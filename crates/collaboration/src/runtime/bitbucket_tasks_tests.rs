//! Actual owned HTTP -> task adapter -> Runtime/SQLite qualification.
//! No provider account, OS vault, background worker or public HTTP is used.
use super::*;
use crate::credentials::CredentialError;
use serde_json::{Value, json};
use std::{
    io::{Read, Write},
    sync::{Condvar, Mutex as StdMutex, atomic::AtomicUsize},
};

const A: &str = "00000000-0000-4000-8000-000000000001";
const B: &str = "00000000-0000-4000-8000-000000000002";
const WORKSPACE: &str = "00000000-0000-4000-8000-000000000010";
const REPO_A: &str = "00000000-0000-4000-8000-000000000020";
const REPO_B: &str = "00000000-0000-4000-8000-000000000021";
fn subject(repository: &str) -> String {
    format!("bitbucket_cloud:pull:{repository}:67")
}
fn repository_id(repository: &str) -> String {
    format!("bitbucket_cloud:repository:{repository}")
}
fn route(repository: &str) -> String {
    format!("/2.0/repositories/%7B%7D/%7B{repository}%7D/pullrequests")
}

#[derive(Default)]
struct Vault {
    tokens: StdMutex<HashMap<String, SecretToken>>,
    loads: AtomicUsize,
    stores: AtomicUsize,
}
impl CredentialVault for Vault {
    fn store(&self, key: &str, token: &SecretToken) -> Result<(), CredentialError> {
        self.stores.fetch_add(1, Ordering::SeqCst);
        self.tokens
            .lock()
            .unwrap()
            .insert(key.into(), token.clone());
        Ok(())
    }
    fn load(&self, key: &str) -> Result<Option<SecretToken>, CredentialError> {
        self.loads.fetch_add(1, Ordering::SeqCst);
        Ok(self.tokens.lock().unwrap().get(key).cloned())
    }
    fn delete(&self, key: &str) -> Result<(), CredentialError> {
        self.tokens.lock().unwrap().remove(key);
        Ok(())
    }
}
struct Clock {
    base: Instant,
    utc: DateTime<Utc>,
    elapsed: std::sync::atomic::AtomicU64,
}
impl Clock {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            base: Instant::now(),
            utc: Utc::now(),
            elapsed: 0.into(),
        })
    }
    fn advance(&self, seconds: u64) {
        self.elapsed.fetch_add(seconds, Ordering::SeqCst);
    }
}
impl clock::Clock for Clock {
    fn now(&self) -> Instant {
        self.base + Duration::from_secs(self.elapsed.load(Ordering::SeqCst))
    }
    fn utc(&self) -> DateTime<Utc> {
        self.utc + chrono::Duration::seconds(self.elapsed.load(Ordering::SeqCst) as i64)
    }
    fn jitter(&self) -> u64 {
        0
    }
}
#[derive(Default)]
struct Held {
    armed: AtomicBool,
    rate_limited: AtomicBool,
    entered: Notify,
    released: StdMutex<bool>,
    release: Condvar,
}
impl Held {
    fn release(&self) {
        *self.released.lock().unwrap() = true;
        self.release.notify_all();
    }
    fn wait(&self) {
        self.entered.notify_one();
        let (released, timeout) = self
            .release
            .wait_timeout_while(
                self.released.lock().unwrap(),
                Duration::from_secs(5),
                |released| !*released,
            )
            .unwrap();
        assert!(
            *released && !timeout.timed_out(),
            "owned singleton release deadline"
        );
    }
}
#[derive(Clone, Copy, Default)]
enum Mode {
    #[default]
    Normal,
    Omitted,
    Oversized,
    FalseNull,
    ResolverChanged,
    Older,
    Invalid,
    Empty,
    Denied,
    Terminal,
    YieldCap,
    Loop,
}
#[derive(Clone, Copy, Default)]
struct Scenario {
    mode: Mode,
    renamed: bool,
    advanced_head: bool,
}
fn response(status: u16, headers: &str, body: &Value) -> String {
    let body = body.to_string();
    format!(
        "HTTP/1.1 {status} Fixture\r\nContent-Length: {}\r\nConnection: close\r\n{headers}\r\n{body}",
        body.len()
    )
}
fn full_name(repository: &str, renamed: bool) -> String {
    format!(
        "{}/{}",
        if renamed { "renamed" } else { "workspace" },
        if repository == REPO_A {
            "first"
        } else {
            "second"
        }
    )
}
fn repository(repository: &str, renamed: bool) -> Value {
    let name = full_name(repository, renamed);
    json!({"type":"repository","uuid":format!("{{{repository}}}"),"scm":"git","name":"fixture","full_name":name,
        "workspace":{"type":"workspace","uuid":format!("{{{WORKSPACE}}}"),"slug":if renamed{"renamed"}else{"workspace"}},
        "links":{"html":{"href":format!("https://bitbucket.org/{name}")},"clone":[{"name":"https","href":format!("https://bitbucket.org/{name}.git")} ]},"description":null,"mainbranch":null})
}
fn person(actor: &str, token: &str) -> Value {
    json!({"type":"participant","user":{"type":"user","uuid":format!("{{{actor}}}"),"nickname":"same"},"approved":true,"role":"REVIEWER","state":"approved","participated_on":"2026-10-04T00:00:00Z","ignored":token})
}
fn pull(repository: &str, token: &str, scenario: Scenario) -> Value {
    json!({"type":"pullrequest","id":67,"title":"cached PR67","state":"OPEN","updated_on":"2026-10-04T00:00:00Z",
        "rendered":{"description":{"raw":format!("Body {repository} {token}")}},"description":format!("Body {repository} {token}"),
        "source":{"branch":{"name":"feature"},"commit":{"hash":if scenario.advanced_head{"c".repeat(40)}else{"a".repeat(40)}}},
        "destination":{"branch":{"name":"main"},"commit":{"hash":"b".repeat(40)},"repository":{"type":"repository","uuid":format!("{{{repository}}}")}},
        "participants":[person(A,token),person(B,token)]})
}
fn task_row(repository: &str, token: &str, scenario: Scenario) -> Value {
    let mut value = json!({"id":7,"content":{"raw":format!("Task {repository} {token}")},"creator":{"type":"user","uuid":format!("{{{A}}}"),"nickname":"same","display_name":"Creator"},"state":"UNRESOLVED","created_on":"2026-10-04T00:00:00Z","updated_on":"2026-10-04T01:00:00Z","pending":true,"resolved_on":"2026-10-04T02:00:00Z","resolved_by":{"type":"team","uuid":format!("{{{B}}}"),"nickname":"same","display_name":"Old resolver"},"comment":{"id":23}});
    match scenario.mode {
        Mode::Omitted | Mode::Oversized => {
            value["content"] = if matches!(scenario.mode, Mode::Omitted) {
                json!({})
            } else {
                json!({"raw":"x".repeat(65_537)})
            };
            value["updated_on"] = "2026-10-04T03:00:00Z".into();
            value["creator"].as_object_mut().unwrap().remove("nickname");
            value["creator"]
                .as_object_mut()
                .unwrap()
                .remove("display_name");
            for key in ["pending", "resolved_on", "resolved_by", "comment"] {
                value.as_object_mut().unwrap().remove(key);
            }
        }
        Mode::FalseNull => {
            value["updated_on"] = "2026-10-04T06:00:00Z".into();
            value["pending"] = false.into();
            value["resolved_on"] = Value::Null;
            value["resolved_by"] = Value::Null;
            value["comment"] = Value::Null;
            value["creator"]["nickname"] = Value::Null;
        }
        Mode::ResolverChanged => {
            value["updated_on"] = "2026-10-04T05:00:00Z".into();
            value["resolved_by"] = json!({"type":"app_user","uuid":format!("{{{WORKSPACE}}}")});
            value["content"]["raw"] = "New content".into();
            value["created_on"] = "2001-01-01T00:00:00Z".into();
            value["resolved_on"] = "2001-01-01T00:00:00Z".into();
        }
        Mode::Older => {
            value["updated_on"] = "2026-10-04T02:00:00Z".into();
            value["resolved_by"]["display_name"] = "Obsolete identity presentation".into();
            value["content"]["raw"] = "Old provider content".into();
            value["pending"] = true.into();
        }
        Mode::Invalid => value["content"]["raw"] = Value::Null,
        _ => {}
    }
    value
}
struct Fixture {
    provider: Arc<providers::bitbucket_cloud::BitbucketCloudProvider>,
    calls: Arc<StdMutex<Vec<(String, String)>>>,
    scenario: Arc<StdMutex<Scenario>>,
    held: Arc<Held>,
    stopped: Arc<AtomicBool>,
    task: Option<std::thread::JoinHandle<()>>,
}
impl Fixture {
    fn new() -> Self {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let base = reqwest::Url::parse(&format!("http://{}/2.0/", listener.local_addr().unwrap()))
            .unwrap();
        let task_base = base.to_string();
        let calls = Arc::new(StdMutex::new(vec![]));
        let scenario = Arc::new(StdMutex::new(Scenario::default()));
        let held = Arc::new(Held::default());
        let stopped = Arc::new(AtomicBool::new(false));
        let task = {
            let calls = calls.clone();
            let scenario = scenario.clone();
            let held = held.clone();
            let stopped = stopped.clone();
            std::thread::spawn(move || {
                let mut connections = vec![];
                while !stopped.load(Ordering::SeqCst) {
                    let mut stream = match listener.accept() {
                        Ok((stream, _)) => stream,
                        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                            std::thread::sleep(Duration::from_millis(1));
                            continue;
                        }
                        Err(error) => panic!("owned accept: {error}"),
                    };
                    assert!(connections.len() < 128, "finite owned request budget");
                    let calls = calls.clone();
                    let task_base = task_base.clone();
                    let scenario = scenario.clone();
                    let held = held.clone();
                    connections.push(std::thread::spawn(move||{
                        stream.set_nonblocking(false).unwrap();stream.set_read_timeout(Some(Duration::from_secs(2))).unwrap();stream.set_write_timeout(Some(Duration::from_secs(2))).unwrap();
                        let mut bytes=vec![];
                        loop{let mut chunk=[0;1024];let count=stream.read(&mut chunk).unwrap();bytes.extend_from_slice(&chunk[..count]);assert!(bytes.len()<=16_384);if count==0||bytes.windows(4).any(|v|v==b"\r\n\r\n"){break;}}
                        let request=String::from_utf8(bytes).unwrap();let target=request.lines().next().unwrap().split_whitespace().nth(1).unwrap();
                        let token=request.lines().find_map(|line|{let (name,value)=line.split_once(':')?;name.eq_ignore_ascii_case("authorization").then(||value.trim())}).and_then(|value|value.strip_prefix("Bearer ")).unwrap();
                        assert!(matches!(token,"actor-a"|"actor-b"|"replacement-a"));assert!(!request.to_ascii_lowercase().contains("if-none-match"));
                        calls.lock().unwrap().push((target.into(),token.into()));let scenario=*scenario.lock().unwrap();
                        let output=if target=="/2.0/user"{response(200,"",&json!({"type":"user","uuid":format!("{{{}}}",if token=="actor-b"{B}else{A}),"nickname":"same-nickname","account_status":"active"}))}
                        else if target=="/2.0/user/workspaces?pagelen=10"{response(200,"",&json!({"values":[{"type":"workspace_access","workspace":{"type":"workspace_base","uuid":format!("{{{WORKSPACE}}}"),"slug":"workspace"}}]}))}
                        else if target==format!("/2.0/repositories/%7B{WORKSPACE}%7D?role=member&pagelen=50"){response(200,"",&json!({"values":[repository(REPO_A,scenario.renamed),repository(REPO_B,scenario.renamed)]}))}
                        else{
                            let repository=[REPO_A,REPO_B].into_iter().find(|repository|target.starts_with(&route(repository))).expect("owned route");
                            if target.starts_with(&format!("{}/67/tasks?",route(repository))){
                                let task_url=reqwest::Url::parse(&format!("http://owned.invalid{target}")).unwrap();let page=task_url.query_pairs().find(|(key,_)|key=="page").map(|(_,value)|value.parse::<usize>().unwrap()).unwrap_or(1);
                                assert!(page<=21 && task_url.query_pairs().all(|(key,_)|key=="pagelen"||key=="page"));
                                if token=="actor-a"&&repository==REPO_A&&held.armed.swap(false,Ordering::SeqCst){let mut old=task_row(repository,token,Scenario::default());old["content"]["raw"]="obsolete held provider data".into();old["updated_on"]="2026-10-05T00:00:00Z".into();held.wait();response(if held.rate_limited.load(Ordering::SeqCst){429}else{200},"Retry-After: 120\r\n",&json!({"values":[old]}))}
                                else if token=="actor-a"&&repository==REPO_A&&matches!(scenario.mode,Mode::Denied){response(403,"",&json!({"error":"synthetic denied"}))}
                                else {let paged=matches!(scenario.mode,Mode::Terminal|Mode::YieldCap|Mode::Loop);let next=if paged&&(!matches!(scenario.mode,Mode::Terminal)||page<3){Some(format!("{}repositories/%7B%7D/%7B{repository}%7D/pullrequests/67/tasks?pagelen=50&page={}",task_base,if matches!(scenario.mode,Mode::Loop)&&page==3{2}else{page+1}))}else{None};let values=if paged||matches!(scenario.mode,Mode::Empty){vec![]}else{vec![task_row(repository,token,scenario)]};response(200,if matches!(scenario.mode,Mode::Invalid){"Retry-After: 120\r\n"}else{""},&json!({"values":values,"next":next}))}
                            }else if target==format!("{}/67",route(repository)){response(200,"",&pull(repository,token,scenario))
                            }else{
                                assert_eq!(target,format!("{}?state=OPEN&state=MERGED&state=DECLINED&state=SUPERSEDED&pagelen=50&sort=id",route(repository)));
                                let normal=Scenario{mode:Mode::Normal,..scenario};response(200,"",&json!({"values":[pull(repository,token,normal)]}))
                            }
                        };stream.write_all(output.as_bytes()).unwrap();
                    }));
                }
                for connection in connections {
                    connection.join().unwrap();
                }
            })
        };
        Self {
            provider: Arc::new(providers::bitbucket_cloud::BitbucketCloudProvider::fixture(
                base,
            )),
            calls,
            scenario,
            held,
            stopped,
            task: Some(task),
        }
    }
    fn count(&self) -> usize {
        self.calls.lock().unwrap().len()
    }
    fn update(&self, update: impl FnOnce(&mut Scenario)) {
        update(&mut self.scenario.lock().unwrap());
    }
    fn task_calls(&self) -> usize {
        self.calls
            .lock()
            .unwrap()
            .iter()
            .filter(|(target, _)| target.contains("/67/tasks?"))
            .count()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        self.held.release();
        self.stopped.store(true, Ordering::SeqCst);
        if let Some(task) = self.task.take() {
            let result = task.join();
            if !std::thread::panicking() {
                result.unwrap();
            }
        }
    }
}
async fn store(path: &std::path::Path) -> Arc<Store> {
    Arc::new(Store::open(path.join("synthetic.sqlite")).await.unwrap())
}
fn runtime(
    database: Arc<Store>,
    vault: Arc<Vault>,
    fixture: &Fixture,
    clock: Arc<Clock>,
) -> CollaborationRuntime {
    let mut runtime = CollaborationRuntime::new(database, vault, fixture.provider.clone());
    runtime.clock = clock;
    runtime
}
fn query(account: &RemoteAccount, repository: &str, facet: DetailFacet) -> DetailQuery {
    DetailQuery {
        account_id: account.id.clone(),
        subject_id: subject(repository),
        facet,
        cursor: None,
        limit: 100,
    }
}
async fn connect(runtime: &CollaborationRuntime, token: &str) -> RemoteAccount {
    let account = runtime.connect_bitbucket_cloud(token.into()).await.unwrap();
    for _ in 0..2 {
        assert!(runtime.run_next().await);
    }
    account
}
async fn select(runtime: &CollaborationRuntime, account: &RemoteAccount, repository: &str) {
    runtime
        .select_repository(&account.id, &repository_id(repository), true)
        .await
        .unwrap();
    assert!(runtime.run_next().await);
}
async fn hydrate(
    runtime: &CollaborationRuntime,
    account: &RemoteAccount,
    repository: &str,
    facet: DetailFacet,
) -> DetailSnapshot {
    runtime
        .hydrate_detail(HydrateDetailRequest {
            account_id: account.id.clone(),
            authorization_epoch: account.authorization_epoch.clone(),
            subject_id: subject(repository),
            facet,
        })
        .await
        .unwrap();
    assert!(runtime.run_next().await);
    runtime
        .store
        .detail(query(account, repository, facet))
        .await
        .unwrap()
}
async fn draft(
    runtime: &CollaborationRuntime,
    account: &RemoteAccount,
    repository: &str,
    text: &str,
) -> LocalDraft {
    runtime
        .save_draft(LocalDraft {
            account_id: account.id.clone(),
            subject_id: subject(repository),
            body: text.into(),
            generation: "0".into(),
        })
        .await
        .unwrap()
}
fn native(entry: &DetailEntry) -> &TaskV1 {
    match entry.native.as_ref().unwrap() {
        NativeDetailPayload::TaskV1(value) => value,
        _ => panic!("task payload"),
    }
}
fn assert_scope_eq(actual: StoredScope, expected: &StoredScope) {
    assert_eq!(
        (
            &actual.run_id,
            &actual.next_cursor,
            &actual.etag,
            &actual.last_modified,
            &actual.coverage,
            &actual.sync
        ),
        (
            &expected.run_id,
            &expected.next_cursor,
            &expected.etag,
            &expected.last_modified,
            &expected.coverage,
            &expected.sync
        ),
        "Every runtime-only checkpoint and validator field survives cold reopen",
    );
}
fn validation(entry: &DetailEntry, field: DetailField) -> &DetailFieldValidation {
    entry
        .field_validations
        .iter()
        .find(|value| value.field == field)
        .unwrap()
}

#[tokio::test]
async fn bitbucket_task_permission_denial_is_facet_and_actor_scoped_preserving_body_and_drafts() {
    let dir = tempfile::tempdir().unwrap();
    let database = store(dir.path()).await;
    let fixture = Fixture::new();
    let engine = runtime(
        database.clone(),
        Arc::new(Vault::default()),
        &fixture,
        Clock::new(),
    );
    let account = connect(&engine, "actor-a").await;
    select(&engine, &account, REPO_A).await;
    select(&engine, &account, REPO_B).await;
    let other = connect(&engine, "actor-b").await;
    select(&engine, &other, REPO_A).await;
    let body = hydrate(&engine, &account, REPO_A, DetailFacet::Body).await;
    hydrate(&engine, &account, REPO_A, DetailFacet::Tasks).await;
    let other_repo = hydrate(&engine, &account, REPO_B, DetailFacet::Tasks).await;
    let other_actor = hydrate(&engine, &other, REPO_A, DetailFacet::Tasks).await;
    let own = draft(&engine, &account, REPO_A, "A private").await;
    let theirs = draft(&engine, &other, REPO_A, "B private").await;
    fixture.update(|scenario| scenario.mode = Mode::Denied);
    let denied = hydrate(&engine, &account, REPO_A, DetailFacet::Tasks).await;
    assert!(denied.entries.is_empty());
    assert_eq!(
        denied.evidence.availability,
        DetailAvailability::Unavailable
    );
    assert_eq!(
        denied.evidence.access_reason,
        Some(CapabilityReason::PermissionDenied)
    );
    for (actor, repository, expected) in [
        (&account, REPO_B, other_repo),
        (&other, REPO_A, other_actor),
    ] {
        let actual = database
            .detail(query(actor, repository, DetailFacet::Tasks))
            .await
            .unwrap();
        assert_eq!(actual.entries, expected.entries);
        assert_eq!(actual.evidence, expected.evidence);
    }
    let remaining_body = database
        .detail(query(&account, REPO_A, DetailFacet::Body))
        .await
        .unwrap();
    assert_eq!(remaining_body.body, body.body);
    assert_eq!(remaining_body.metadata, body.metadata);
    assert_eq!(remaining_body.evidence, body.evidence);
    assert_eq!(
        database
            .repositories(&account.id)
            .await
            .unwrap()
            .repositories
            .len(),
        2
    );
    assert_eq!(
        database.account(&account.id).await.unwrap().state,
        AccountState::Active
    );
    for saved in [own, theirs] {
        assert_eq!(
            database
                .draft(&saved.account_id, &saved.subject_id)
                .await
                .unwrap(),
            Some(saved)
        );
    }
}

#[tokio::test]
async fn bitbucket_task_obsolete_epoch_success_and_quota_error_cannot_cross_replacement() {
    for rate_limited in [false, true] {
        let dir = tempfile::tempdir().unwrap();
        let database = store(dir.path()).await;
        let fixture = Fixture::new();
        let engine = runtime(
            database.clone(),
            Arc::new(Vault::default()),
            &fixture,
            Clock::new(),
        );
        let account = connect(&engine, "actor-a").await;
        select(&engine, &account, REPO_A).await;
        hydrate(&engine, &account, REPO_A, DetailFacet::Tasks).await;
        let other = connect(&engine, "actor-b").await;
        select(&engine, &other, REPO_A).await;
        hydrate(&engine, &other, REPO_A, DetailFacet::Tasks).await;
        let private = draft(&engine, &account, REPO_A, "old grant private text").await;
        fixture
            .held
            .rate_limited
            .store(rate_limited, Ordering::SeqCst);
        fixture.held.armed.store(true, Ordering::SeqCst);
        engine
            .hydrate_detail(HydrateDetailRequest {
                account_id: account.id.clone(),
                authorization_epoch: account.authorization_epoch.clone(),
                subject_id: subject(REPO_A),
                facet: DetailFacet::Tasks,
            })
            .await
            .unwrap();
        let worker = engine.clone();
        let pending = tokio::spawn(async move { worker.run_next().await });
        tokio::time::timeout(Duration::from_secs(2), fixture.held.entered.notified())
            .await
            .unwrap();
        let replacement = engine
            .connect_bitbucket_cloud("replacement-a".into())
            .await
            .unwrap();
        assert_eq!(replacement.id, account.id);
        assert_eq!(replacement.authorization_epoch, "2");
        let revision = database.revision().await.unwrap();
        let current = database
            .detail(query(&replacement, REPO_A, DetailFacet::Tasks))
            .await
            .unwrap();
        let theirs = database
            .detail(query(&other, REPO_A, DetailFacet::Tasks))
            .await
            .unwrap();
        let accounts = database.accounts().await.unwrap();
        fixture.held.release();
        assert!(
            tokio::time::timeout(Duration::from_secs(2), pending)
                .await
                .unwrap()
                .unwrap()
        );
        assert_eq!(database.revision().await.unwrap(), revision);
        assert_eq!(database.accounts().await.unwrap(), accounts);
        assert_eq!(
            database
                .detail(query(&replacement, REPO_A, DetailFacet::Tasks))
                .await
                .unwrap(),
            current
        );
        assert_eq!(
            database
                .detail(query(&other, REPO_A, DetailFacet::Tasks))
                .await
                .unwrap(),
            theirs
        );
        assert!(
            database
                .scope_state(&replacement.id, "provider:rest")
                .await
                .unwrap()
                .is_none()
        );
        assert!(
            database
                .scope_state(&other.id, "provider:rest")
                .await
                .unwrap()
                .is_none()
        );
        assert_eq!(
            database
                .draft(&replacement.id, &private.subject_id)
                .await
                .unwrap(),
            Some(private)
        );
    }
}

#[tokio::test]
async fn bitbucket_task_subject_history_still_rejects_held_old_head_and_selection_lease() {
    for selection_churn in [false, true] {
        let dir = tempfile::tempdir().unwrap();
        let database = store(dir.path()).await;
        let fixture = Fixture::new();
        let engine = runtime(
            database.clone(),
            Arc::new(Vault::default()),
            &fixture,
            Clock::new(),
        );
        let account = connect(&engine, "actor-a").await;
        select(&engine, &account, REPO_A).await;
        let original = hydrate(&engine, &account, REPO_A, DetailFacet::Tasks).await;
        let private = draft(
            &engine,
            &account,
            REPO_A,
            "retain while captured authority retires",
        )
        .await;
        fixture.held.armed.store(true, Ordering::SeqCst);
        engine
            .hydrate_detail(HydrateDetailRequest {
                account_id: account.id.clone(),
                authorization_epoch: account.authorization_epoch.clone(),
                subject_id: subject(REPO_A),
                facet: DetailFacet::Tasks,
            })
            .await
            .unwrap();
        let worker = engine.clone();
        let pending = tokio::spawn(async move { worker.run_next().await });
        tokio::time::timeout(Duration::from_secs(2), fixture.held.entered.notified())
            .await
            .unwrap();
        if selection_churn {
            let scope = DetailFacet::Tasks.scope(&subject(REPO_A));
            let prior = database
                .scope_state(&account.id, &scope)
                .await
                .unwrap()
                .unwrap();
            engine
                .select_repository(&account.id, &repository_id(REPO_A), false)
                .await
                .unwrap();
            engine
                .select_repository(&account.id, &repository_id(REPO_A), true)
                .await
                .unwrap();
            assert_ne!(
                database
                    .scope_state(&account.id, &scope)
                    .await
                    .unwrap()
                    .unwrap()
                    .run_id,
                prior.run_id,
                "deselection retires the exact task lease even after reselection"
            );
        } else {
            fixture.update(|scenario| scenario.advanced_head = true);
            let repository = database
                .repository(&account.id, &repository_id(REPO_A))
                .await
                .unwrap();
            // Runtime dispatch is serialized. This is an independent actual HTTP
            // producer through the same adapter and Store transaction, not two Runtime jobs.
            let page = fixture
                .provider
                .fetch_page(
                    &SecretToken::new("actor-a".into()).unwrap(),
                    FeedRequest {
                        account: account.clone(),
                        repository: Some(repository.clone()),
                        kind: FeedKind::PullRequests,
                        cursor: None,
                        etag: None,
                        last_modified: None,
                    },
                )
                .await
                .unwrap();
            let scope = scope_name(FeedKind::PullRequests, Some(&repository));
            let run = database
                .begin_sync(&account.id, &account.authorization_epoch, &scope)
                .await
                .unwrap();
            database
                .apply_page(PageCommit {
                    account_id: account.id.clone(),
                    authorization_epoch: account.authorization_epoch.clone(),
                    scope,
                    run_id: run,
                    repositories: page.repositories,
                    items: page.items,
                    endpoint_aliases: page.endpoint_aliases,
                    next_cursor: page.next_cursor,
                    etag: page.etag,
                    last_modified: page.last_modified,
                    not_modified: page.not_modified,
                    complete: true,
                    observed_at: engine.now_string(),
                })
                .await
                .unwrap();
            assert_eq!(
                database
                    .item(&account.id, &subject(REPO_A))
                    .await
                    .unwrap()
                    .item
                    .unwrap()
                    .head_oid,
                Some("c".repeat(40))
            );
        }
        fixture.held.release();
        assert!(
            tokio::time::timeout(Duration::from_secs(2), pending)
                .await
                .unwrap()
                .unwrap()
        );
        let after = database
            .detail(query(&account, REPO_A, DetailFacet::Tasks))
            .await
            .unwrap();
        assert_eq!(after.entries, original.entries);
        assert_eq!(
            after.evidence.facet_revision,
            original.evidence.facet_revision
        );
        assert!(
            after
                .entries
                .iter()
                .all(|entry| native(entry).content.text.as_deref()
                    != Some("obsolete held provider data"))
        );
        assert_eq!(
            database
                .draft(&account.id, &private.subject_id)
                .await
                .unwrap(),
            Some(private)
        );
    }
}

#[tokio::test]
async fn bitbucket_tasks_compound_actors_rename_and_offline_cold_reads_preserve_private_drafts() {
    let dir = tempfile::tempdir().unwrap();
    let database = store(dir.path()).await;
    let fixture = Fixture::new();
    let vault = Arc::new(Vault::default());
    let clock = Clock::new();
    let engine = runtime(database.clone(), vault.clone(), &fixture, clock.clone());
    let first = connect(&engine, "actor-a").await;
    select(&engine, &first, REPO_A).await;
    select(&engine, &first, REPO_B).await;
    let other = connect(&engine, "actor-b").await;
    select(&engine, &other, REPO_A).await;
    assert_eq!(
        fixture.task_calls(),
        0,
        "selected feeds and unopened local queries never hydrate Tasks"
    );
    assert_eq!(
        database
            .detail(query(&first, REPO_A, DetailFacet::Tasks))
            .await
            .unwrap()
            .evidence
            .availability,
        DetailAvailability::Missing
    );
    let own = hydrate(&engine, &first, REPO_A, DetailFacet::Tasks).await;
    let other_repo = hydrate(&engine, &first, REPO_B, DetailFacet::Tasks).await;
    let theirs = hydrate(&engine, &other, REPO_A, DetailFacet::Tasks).await;
    assert_eq!(own.entries.len(), 1);
    assert_ne!(own.entries[0].id, other_repo.entries[0].id);
    assert_eq!(own.entries[0].id, theirs.entries[0].id);
    assert_ne!(
        native(&own.entries[0]).content,
        native(&theirs.entries[0]).content
    );
    assert_eq!(first.login, other.login);
    assert_ne!(first.id, other.id);
    let own_draft = draft(&engine, &first, REPO_A, "private A").await;
    let theirs_draft = draft(&engine, &other, REPO_A, "private B").await;
    fixture.update(|scenario| scenario.renamed = true);
    engine
        .refresh(RefreshRequest {
            account_id: first.id.clone(),
            repository_id: None,
            kind: None,
        })
        .await
        .unwrap();
    for _ in 0..4 {
        assert!(engine.run_next().await);
    }
    assert_eq!(
        database
            .repository(&first.id, &repository_id(REPO_A))
            .await
            .unwrap()
            .full_name,
        "renamed/first"
    );
    let renamed = hydrate(&engine, &first, REPO_A, DetailFacet::Tasks).await;
    assert_eq!(own.entries[0].id, renamed.entries[0].id);
    assert_eq!(native(&own.entries[0]), native(&renamed.entries[0]));
    let snapshots = [
        database
            .detail(query(&first, REPO_A, DetailFacet::Tasks))
            .await
            .unwrap(),
        database
            .detail(query(&first, REPO_B, DetailFacet::Tasks))
            .await
            .unwrap(),
        database
            .detail(query(&other, REPO_A, DetailFacet::Tasks))
            .await
            .unwrap(),
    ];
    let calls = fixture.count();
    let loads = vault.loads.load(Ordering::SeqCst);
    database.close().await.unwrap();
    drop(engine);
    drop(database);
    let reopened = store(dir.path()).await;
    let cold = runtime(reopened.clone(), vault.clone(), &fixture, clock);
    for ((account, repo), expected) in [(&first, REPO_A), (&first, REPO_B), (&other, REPO_A)]
        .into_iter()
        .zip(snapshots)
    {
        assert_eq!(
            reopened
                .detail(query(account, repo, DetailFacet::Tasks))
                .await
                .unwrap(),
            expected
        );
    }
    for saved in [own_draft, theirs_draft] {
        assert_eq!(
            reopened
                .draft(&saved.account_id, &saved.subject_id)
                .await
                .unwrap(),
            Some(saved)
        );
    }
    assert!(!cold.run_next().await);
    assert_eq!(fixture.count(), calls);
    assert_eq!(vault.loads.load(Ordering::SeqCst), loads);
}

#[tokio::test]
async fn bitbucket_tasks_own_clock_masks_content_and_resolver_context_never_overwrite_other_facets()
{
    let dir = tempfile::tempdir().unwrap();
    let database = store(dir.path()).await;
    let fixture = Fixture::new();
    let clock = Clock::new();
    let engine = runtime(
        database.clone(),
        Arc::new(Vault::default()),
        &fixture,
        clock.clone(),
    );
    let account = connect(&engine, "actor-a").await;
    select(&engine, &account, REPO_A).await;
    let body = hydrate(&engine, &account, REPO_A, DetailFacet::Body).await;
    let participants = hydrate(&engine, &account, REPO_A, DetailFacet::Participants).await;
    let private = draft(
        &engine,
        &account,
        REPO_A,
        "authored content survives native observations",
    )
    .await;
    let original = hydrate(&engine, &account, REPO_A, DetailFacet::Tasks).await;
    let content_validation = validation(&original.entries[0], DetailField::TaskContent).clone();
    let pending_validation = validation(&original.entries[0], DetailField::TaskPending).clone();
    for mode in [Mode::Omitted, Mode::Oversized] {
        clock.advance(1);
        fixture.update(|scenario| scenario.mode = mode);
        let retained = hydrate(&engine, &account, REPO_A, DetailFacet::Tasks).await;
        assert_eq!(
            native(&retained.entries[0]).content,
            native(&original.entries[0]).content
        );
        assert_eq!(
            native(&retained.entries[0]).observed_content_state,
            if matches!(mode, Mode::Omitted) {
                DetailValueState::Omitted
            } else {
                DetailValueState::Oversized
            }
        );
        assert_eq!(
            validation(&retained.entries[0], DetailField::TaskContent),
            &content_validation
        );
        assert_eq!(
            validation(&retained.entries[0], DetailField::TaskPending),
            &pending_validation
        );
        assert_eq!(native(&retained.entries[0]).pending, Some(true));
        assert_eq!(
            native(&retained.entries[0]).creator.login.as_deref(),
            Some("same")
        );
        assert!(
            !retained.entries[0]
                .field_mask
                .contains(&DetailField::TaskPending)
        );
    }
    fixture.update(|scenario| scenario.mode = Mode::ResolverChanged);
    let changed = hydrate(&engine, &account, REPO_A, DetailFacet::Tasks).await;
    let resolver = native(&changed.entries[0]).resolved_by.as_ref().unwrap();
    assert_eq!(resolver.provider_id, WORKSPACE);
    assert_eq!(resolver.kind, "app_user");
    assert!(resolver.login.is_none() && resolver.display_name.is_none());
    assert!(
        !changed.entries[0]
            .field_validations
            .iter()
            .any(|v| matches!(
                v.field,
                DetailField::TaskResolverLogin | DetailField::TaskResolverDisplayName
            )),
        "new resolver cannot inherit old actor presentation proof"
    );
    assert_eq!(
        native(&changed.entries[0]).resolved_at.as_deref(),
        Some("2001-01-01T00:00:00Z"),
        "action time does not order fields"
    );
    fixture.update(|scenario| scenario.mode = Mode::FalseNull);
    let cleared = hydrate(&engine, &account, REPO_A, DetailFacet::Tasks).await;
    let value = native(&cleared.entries[0]);
    assert_eq!(value.pending, Some(false));
    assert!(
        value.resolved_by.is_none()
            && value.resolved_at.is_none()
            && value.comment_id.is_none()
            && value.creator.login.is_none()
    );
    assert_eq!(cleared.entries[0].field_mask.len(), 12);
    fixture.update(|scenario| scenario.mode = Mode::Older);
    clock.advance(1);
    let older = hydrate(&engine, &account, REPO_A, DetailFacet::Tasks).await;
    assert_eq!(native(&older.entries[0]), native(&cleared.entries[0]));
    assert_eq!(
        older.entries[0].field_validations, cleared.entries[0].field_validations,
        "new local receipt does not make old updated_on authoritative"
    );
    assert!(
        older
            .evidence
            .source
            .as_ref()
            .unwrap()
            .provider_updated_at
            .is_none()
    );
    for (facet, saved) in [
        (DetailFacet::Body, body),
        (DetailFacet::Participants, participants),
    ] {
        let actual = database
            .detail(query(&account, REPO_A, facet))
            .await
            .unwrap();
        assert_eq!(actual.body, saved.body);
        assert_eq!(actual.metadata, saved.metadata);
        assert_eq!(actual.entries, saved.entries);
        assert_eq!(actual.evidence, saved.evidence);
    }
    assert_eq!(
        database
            .draft(&account.id, &private.subject_id)
            .await
            .unwrap(),
        Some(private)
    );
}

#[tokio::test]
async fn bitbucket_tasks_invalid_content_keeps_cache_and_draft_with_cold_durable_actor_cooldown() {
    let dir = tempfile::tempdir().unwrap();
    let database = store(dir.path()).await;
    let fixture = Fixture::new();
    let vault = Arc::new(Vault::default());
    let clock = Clock::new();
    let engine = runtime(database.clone(), vault.clone(), &fixture, clock.clone());
    let account = connect(&engine, "actor-a").await;
    select(&engine, &account, REPO_A).await;
    let private = draft(
        &engine,
        &account,
        REPO_A,
        "keep authored text on malformed task",
    )
    .await;
    let saved = hydrate(&engine, &account, REPO_A, DetailFacet::Tasks).await;
    fixture.update(|scenario| scenario.mode = Mode::Invalid);
    let failed = hydrate(&engine, &account, REPO_A, DetailFacet::Tasks).await;
    assert_eq!(failed.entries, saved.entries);
    assert_eq!(
        failed.evidence.facet_revision,
        saved.evidence.facet_revision
    );
    assert_eq!(failed.evidence.coverage, saved.evidence.coverage);
    assert!(failed.evidence.sync.error.is_some());
    let rest = database
        .scope_state(&account.id, "provider:rest")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(rest.sync.state, SyncState::RateLimited);
    assert!(rest.sync.next_retry_at.is_some());
    assert_eq!(database.account(&account.id).await.unwrap(), account);
    let calls = fixture.count();
    let loads = vault.loads.load(Ordering::SeqCst);
    database.close().await.unwrap();
    drop(engine);
    drop(database);
    let reopened = store(dir.path()).await;
    let cold = runtime(reopened.clone(), vault.clone(), &fixture, clock);
    assert_eq!(
        reopened
            .detail(query(&account, REPO_A, DetailFacet::Tasks))
            .await
            .unwrap(),
        failed,
    );
    assert_eq!(
        reopened
            .draft(&account.id, &private.subject_id)
            .await
            .unwrap(),
        Some(private)
    );
    assert!(!cold.run_next().await);
    assert_eq!(fixture.count(), calls);
    assert_eq!(vault.loads.load(Ordering::SeqCst), loads);
}

#[tokio::test]
async fn bitbucket_tasks_terminal_multipage_uncertain_retains_history_but_single_empty_reconciles_absence()
 {
    let dir = tempfile::tempdir().unwrap();
    let database = store(dir.path()).await;
    let fixture = Fixture::new();
    let engine = runtime(
        database.clone(),
        Arc::new(Vault::default()),
        &fixture,
        Clock::new(),
    );
    let account = connect(&engine, "actor-a").await;
    select(&engine, &account, REPO_A).await;
    let saved = hydrate(&engine, &account, REPO_A, DetailFacet::Tasks).await;
    let private = draft(&engine, &account, REPO_A, "survive absence reconciliation").await;
    fixture.update(|scenario| scenario.mode = Mode::Terminal);
    let first = hydrate(&engine, &account, REPO_A, DetailFacet::Tasks).await;
    assert_eq!(first.entries, saved.entries);
    assert_eq!(first.evidence.coverage.state, CoverageState::Partial);
    assert!(engine.run_next().await);
    assert!(engine.run_next().await);
    let terminal = database
        .detail(query(&account, REPO_A, DetailFacet::Tasks))
        .await
        .unwrap();
    assert_eq!(terminal.entries, saved.entries);
    assert_eq!(terminal.evidence.coverage.state, CoverageState::Partial);
    assert!(!terminal.evidence.coverage.remote_has_more);
    assert_eq!(fixture.task_calls(), 4);
    fixture.update(|scenario| scenario.mode = Mode::Empty);
    let empty = hydrate(&engine, &account, REPO_A, DetailFacet::Tasks).await;
    assert!(empty.entries.is_empty());
    assert_eq!(empty.evidence.saved_empty, Some(true));
    assert_eq!(empty.evidence.coverage.state, CoverageState::Complete);
    assert_eq!(
        database
            .draft(&account.id, &private.subject_id)
            .await
            .unwrap(),
        Some(private)
    );
}

#[tokio::test]
async fn bitbucket_tasks_twenty_page_budget_survives_ten_page_yield_and_two_cold_reopens() {
    let dir = tempfile::tempdir().unwrap();
    let database = store(dir.path()).await;
    let fixture = Fixture::new();
    let vault = Arc::new(Vault::default());
    let clock = Clock::new();
    let engine = runtime(database.clone(), vault.clone(), &fixture, clock.clone());
    let account = connect(&engine, "actor-a").await;
    select(&engine, &account, REPO_A).await;
    let saved = hydrate(&engine, &account, REPO_A, DetailFacet::Tasks).await;
    let private = draft(&engine, &account, REPO_A, "private under capped traversal").await;
    fixture.update(|scenario| scenario.mode = Mode::YieldCap);
    engine
        .hydrate_detail(HydrateDetailRequest {
            account_id: account.id.clone(),
            authorization_epoch: account.authorization_epoch.clone(),
            subject_id: subject(REPO_A),
            facet: DetailFacet::Tasks,
        })
        .await
        .unwrap();
    for _ in 0..10 {
        assert!(engine.run_next().await);
    }
    assert!(!engine.run_next().await);
    assert_eq!(fixture.task_calls(), 11);
    let scope = DetailFacet::Tasks.scope(&subject(REPO_A));
    let yielded = database
        .scope_state(&account.id, &scope)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(yielded.coverage.state, CoverageState::Partial);
    assert!(yielded.next_cursor.is_some());
    let yielded_cursor: serde_json::Value =
        serde_json::from_str(yielded.next_cursor.as_ref().unwrap()).unwrap();
    assert_eq!(yielded_cursor["pages"], 10);
    let yielded_history = yielded_cursor["seen_pages"].as_array().unwrap();
    assert_eq!(yielded_history.len(), 10);
    assert_eq!(
        database
            .detail(query(&account, REPO_A, DetailFacet::Tasks))
            .await
            .unwrap()
            .entries,
        saved.entries
    );
    let calls = fixture.count();
    let loads = vault.loads.load(Ordering::SeqCst);
    database.close().await.unwrap();
    drop(engine);
    drop(database);
    let reopened = store(dir.path()).await;
    let engine = runtime(reopened.clone(), vault.clone(), &fixture, clock.clone());
    assert_scope_eq(
        reopened
            .scope_state(&account.id, &scope)
            .await
            .unwrap()
            .unwrap(),
        &yielded,
    );
    assert!(!engine.run_next().await);
    assert_eq!(fixture.count(), calls);
    assert_eq!(vault.loads.load(Ordering::SeqCst), loads);
    engine
        .hydrate_detail(HydrateDetailRequest {
            account_id: account.id.clone(),
            authorization_epoch: account.authorization_epoch.clone(),
            subject_id: subject(REPO_A),
            facet: DetailFacet::Tasks,
        })
        .await
        .unwrap();
    for _ in 0..10 {
        assert!(engine.run_next().await);
    }
    assert_eq!(fixture.task_calls(), 21);
    let capped = reopened
        .scope_state(&account.id, &scope)
        .await
        .unwrap()
        .unwrap();
    assert_ne!(
        capped.run_id, yielded.run_id,
        "resumption fences old leases with a fresh request generation"
    );
    assert_eq!(capped.coverage.state, CoverageState::Partial);
    assert!(capped.coverage.remote_has_more);
    assert!(capped.next_cursor.as_ref().unwrap().len() <= 4096);
    let capped_cursor: serde_json::Value =
        serde_json::from_str(capped.next_cursor.as_ref().unwrap()).unwrap();
    assert_eq!(capped_cursor["pages"], 20);
    let capped_history = capped_cursor["seen_pages"].as_array().unwrap();
    assert_eq!(capped_history.len(), 20);
    assert_eq!(&capped_history[..10], yielded_history.as_slice());
    reopened.close().await.unwrap();
    drop(engine);
    drop(reopened);
    let cold = store(dir.path()).await;
    let engine = runtime(cold.clone(), vault, &fixture, clock.clone());
    assert_scope_eq(
        cold.scope_state(&account.id, &scope)
            .await
            .unwrap()
            .unwrap(),
        &capped,
    );
    let mut previous_run = capped.run_id.clone();
    for _ in 0..2 {
        engine
            .hydrate_detail(HydrateDetailRequest {
                account_id: account.id.clone(),
                authorization_epoch: account.authorization_epoch.clone(),
                subject_id: subject(REPO_A),
                facet: DetailFacet::Tasks,
            })
            .await
            .unwrap();
        assert!(engine.run_next().await);
        let failed = cold
            .scope_state(&account.id, &scope)
            .await
            .unwrap()
            .unwrap();
        assert_ne!(failed.run_id, previous_run);
        previous_run = failed.run_id.clone();
        assert_eq!(failed.next_cursor, capped.next_cursor);
        assert_eq!(failed.coverage, capped.coverage);
        assert_eq!(failed.sync.state, SyncState::Error);
        assert_eq!(
            fixture.task_calls(),
            21,
            "capped resume rejects before HTTP"
        );
        clock.advance(181);
    }
    assert_eq!(
        cold.detail(query(&account, REPO_A, DetailFacet::Tasks))
            .await
            .unwrap()
            .entries,
        saved.entries
    );
    assert_eq!(
        cold.draft(&account.id, &private.subject_id).await.unwrap(),
        Some(private)
    );
}

#[tokio::test]
async fn bitbucket_tasks_loop_rejection_is_atomic_and_cold_retry_never_refollows_seen_target() {
    let dir = tempfile::tempdir().unwrap();
    let database = store(dir.path()).await;
    let fixture = Fixture::new();
    let vault = Arc::new(Vault::default());
    let clock = Clock::new();
    let engine = runtime(database.clone(), vault.clone(), &fixture, clock.clone());
    let account = connect(&engine, "actor-a").await;
    select(&engine, &account, REPO_A).await;
    let saved = hydrate(&engine, &account, REPO_A, DetailFacet::Tasks).await;
    let private = draft(&engine, &account, REPO_A, "loop preserves authored data").await;
    fixture.update(|scenario| scenario.mode = Mode::Loop);
    hydrate(&engine, &account, REPO_A, DetailFacet::Tasks).await;
    assert!(engine.run_next().await);
    let scope = DetailFacet::Tasks.scope(&subject(REPO_A));
    let before = database
        .scope_state(&account.id, &scope)
        .await
        .unwrap()
        .unwrap();
    assert!(engine.run_next().await);
    let failed = database
        .scope_state(&account.id, &scope)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(failed.next_cursor, before.next_cursor);
    assert_eq!(failed.run_id, before.run_id);
    assert_eq!(failed.coverage, before.coverage);
    assert_eq!(failed.coverage.state, CoverageState::Partial);
    assert_eq!(failed.sync.state, SyncState::Error);
    assert!(failed.sync.next_retry_at.is_some());
    assert_eq!(fixture.task_calls(), 4);
    let calls = fixture.count();
    let loads = vault.loads.load(Ordering::SeqCst);
    database.close().await.unwrap();
    drop(engine);
    drop(database);
    let cold = store(dir.path()).await;
    let engine = runtime(cold.clone(), vault.clone(), &fixture, clock.clone());
    assert_scope_eq(
        cold.scope_state(&account.id, &scope)
            .await
            .unwrap()
            .unwrap(),
        &failed,
    );
    engine
        .hydrate_detail(HydrateDetailRequest {
            account_id: account.id.clone(),
            authorization_epoch: account.authorization_epoch.clone(),
            subject_id: subject(REPO_A),
            facet: DetailFacet::Tasks,
        })
        .await
        .unwrap();
    assert!(!engine.run_next().await);
    assert_eq!(fixture.count(), calls);
    assert_eq!(vault.loads.load(Ordering::SeqCst), loads);
    clock.advance(61);
    assert!(engine.run_next().await);
    assert_eq!(fixture.task_calls(), 5);
    {
        let seen = fixture.calls.lock().unwrap();
        assert_eq!(
            seen.iter()
                .filter(|(target, _)| target.ends_with("tasks?pagelen=50&page=2"))
                .count(),
            1
        );
        assert_eq!(
            seen.iter()
                .filter(|(target, _)| target.ends_with("tasks?pagelen=50&page=3"))
                .count(),
            2
        );
    }
    assert_eq!(
        cold.detail(query(&account, REPO_A, DetailFacet::Tasks))
            .await
            .unwrap()
            .entries,
        saved.entries
    );
    assert_eq!(
        cold.draft(&account.id, &private.subject_id).await.unwrap(),
        Some(private)
    );
}
