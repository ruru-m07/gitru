//! Actual owned HTTP -> participant adapter -> Runtime/SQLite qualification.
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
    FalseNull,
    OlderAction,
    Empty,
    Absent,
    ApprovedNull,
    Denied,
    ConflictingBody,
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
fn person(actor: &str, token: &str, scenario: Scenario) -> Value {
    let mut value = json!({"type":"participant","user":{"type":"user","uuid":format!("{{{actor}}}"),"nickname":"same-nickname",
        "display_name":format!("{} {token}",if scenario.renamed{"Renamed user"}else{"Original user"})},
        "role":"REVIEWER","approved":true,"state":"approved","participated_on":"2026-10-04T00:00:00Z"});
    match scenario.mode {
        Mode::Omitted => {
            value =
                json!({"type":"participant","user":{"type":"user","uuid":format!("{{{actor}}}")}})
        }
        Mode::FalseNull => {
            value["approved"] = false.into();
            value["state"] = Value::Null;
            value["participated_on"] = Value::Null;
            value["user"]["nickname"] = Value::Null;
            value["user"]["display_name"] = Value::Null;
        }
        Mode::OlderAction => {
            value["approved"] = false.into();
            value["role"] = "FUTURE_ROLE".into();
            value["state"] = "future_native_state".into();
            value["participated_on"] = "2001-01-01T00:00:00Z".into();
        }
        Mode::ApprovedNull => value["approved"] = Value::Null,
        _ => {}
    }
    value
}
fn pull(repository: &str, token: &str, scenario: Scenario) -> Value {
    let mut value = json!({"type":"pullrequest","id":67,"title":"cached PR67","state":"OPEN","updated_on":"2026-10-04T00:00:00Z",
        "rendered":{"description":{"raw":format!("Body {repository} {token}")}},"description":format!("Body {repository} {token}"),
        "source":{"branch":{"name":"feature"},"commit":{"hash":if scenario.advanced_head{"c".repeat(40)}else{"a".repeat(40)}}},
        "destination":{"branch":{"name":"main"},"commit":{"hash":"b".repeat(40)},"repository":{"type":"repository","uuid":format!("{{{repository}}}")}},
        "participants":[person(A,token,scenario),person(B,token,scenario)],"reviewers":[{"uuid":"not-authority"}]});
    match scenario.mode {
        Mode::Empty => value["participants"] = json!([]),
        Mode::Absent => {
            value.as_object_mut().unwrap().remove("participants");
        }
        Mode::ConflictingBody => {
            value["rendered"]["description"]["raw"] = false.into();
            value["description"] = "conflicting unrelated raw".into();
            value["updated_on"] = "not-a-parent-clock".into();
        }
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
                            if target==format!("{}/67",route(repository)){
                                if token=="actor-a"&&repository==REPO_A&&held.armed.swap(false,Ordering::SeqCst){
                                    let mut old=pull(repository,token,Scenario::default());old["participants"][0]["user"]["display_name"]="obsolete held provider data".into();held.wait();
                                    response(if held.rate_limited.load(Ordering::SeqCst){429}else{200},"Retry-After: 120\r\n",&old)
                                }else if token=="actor-a"&&repository==REPO_A&&matches!(scenario.mode,Mode::Denied){response(403,"",&json!({"error":"synthetic denied"}))}
                                else{response(200,if matches!(scenario.mode,Mode::Absent|Mode::ApprovedNull){"Retry-After: 120\r\n"}else{""},&pull(repository,token,scenario))}
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
    fn singletons(&self) -> usize {
        self.calls
            .lock()
            .unwrap()
            .iter()
            .filter(|(target, _)| target.ends_with("/67"))
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
fn native(entry: &DetailEntry) -> &ParticipantV1 {
    match entry.native.as_ref().unwrap() {
        NativeDetailPayload::ParticipantV1(value) => value,
    }
}
fn validation(entry: &DetailEntry, field: DetailField) -> &DetailFieldValidation {
    entry
        .field_validations
        .iter()
        .find(|value| value.field == field)
        .unwrap()
}

#[tokio::test]
async fn bitbucket_participants_compound_actor_identity_rename_and_cold_saved_reads_have_no_http_or_vault()
 {
    let dir = tempfile::tempdir().unwrap();
    let database = store(dir.path()).await;
    let fixture = Fixture::new();
    let vault = Arc::new(Vault::default());
    let clock = Clock::new();
    let engine = runtime(database.clone(), vault.clone(), &fixture, clock.clone());
    let first = connect(&engine, "actor-a").await;
    for repository in [REPO_A, REPO_B] {
        select(&engine, &first, repository).await;
    }
    let other = connect(&engine, "actor-b").await;
    select(&engine, &other, REPO_A).await;
    assert_eq!(
        fixture.singletons(),
        0,
        "selected repositories and embedded list participants never hydrate this facet"
    );
    assert_eq!(
        database
            .detail(query(&first, REPO_A, DetailFacet::Participants))
            .await
            .unwrap()
            .evidence
            .availability,
        DetailAvailability::Missing
    );
    let own = hydrate(&engine, &first, REPO_A, DetailFacet::Participants).await;
    let own_b = hydrate(&engine, &first, REPO_B, DetailFacet::Participants).await;
    let theirs = hydrate(&engine, &other, REPO_A, DetailFacet::Participants).await;
    assert_eq!(own.entries.len(), 2);
    assert_ne!(own.entries[0].id, own_b.entries[0].id);
    assert_eq!(own.entries[0].id, theirs.entries[0].id);
    assert_ne!(
        native(&own.entries[0]).user.display_name,
        native(&theirs.entries[0]).user.display_name
    );
    assert_eq!(
        native(&own.entries[0]).user.login,
        native(&own.entries[1]).user.login
    );
    assert_ne!(
        native(&own.entries[0]).user.provider_id,
        native(&own.entries[1]).user.provider_id
    );
    let private_a = draft(&engine, &first, REPO_A, "private Δ A").await;
    let private_b = draft(&engine, &other, REPO_A, "private B").await;
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
    let renamed = hydrate(&engine, &first, REPO_A, DetailFacet::Participants).await;
    assert_eq!(own.entries[0].id, renamed.entries[0].id);
    assert!(
        native(&renamed.entries[0])
            .user
            .display_name
            .as_deref()
            .unwrap()
            .starts_with("Renamed user")
    );
    let snapshots = [
        database
            .detail(query(&first, REPO_A, DetailFacet::Participants))
            .await
            .unwrap(),
        database
            .detail(query(&first, REPO_B, DetailFacet::Participants))
            .await
            .unwrap(),
        database
            .detail(query(&other, REPO_A, DetailFacet::Participants))
            .await
            .unwrap(),
    ];
    let calls = fixture.count();
    let loads = vault.loads.load(Ordering::SeqCst);
    database.close().await;
    drop(engine);
    drop(database);
    let reopened = store(dir.path()).await;
    let cold = runtime(reopened.clone(), vault.clone(), &fixture, clock);
    for ((account, repository), expected) in [(&first, REPO_A), (&first, REPO_B), (&other, REPO_A)]
        .into_iter()
        .zip(snapshots)
    {
        assert_eq!(
            reopened
                .detail(query(account, repository, DetailFacet::Participants))
                .await
                .unwrap(),
            expected
        );
    }
    for saved in [private_a, private_b] {
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
async fn bitbucket_participant_masks_retain_omitted_flags_clear_known_null_and_ignore_parent_action_clocks()
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
    let saved = hydrate(&engine, &account, REPO_A, DetailFacet::Participants).await;
    let old_validation = validation(&saved.entries[0], DetailField::ParticipantApproved).clone();
    clock.advance(1);
    fixture.update(|scenario| scenario.mode = Mode::Omitted);
    let omitted = hydrate(&engine, &account, REPO_A, DetailFacet::Participants).await;
    assert_eq!(native(&omitted.entries[0]), native(&saved.entries[0]));
    assert!(omitted.entries[0].field_mask.is_empty());
    assert_eq!(
        validation(&omitted.entries[0], DetailField::ParticipantApproved),
        &old_validation
    );
    clock.advance(1);
    fixture.update(|scenario| scenario.mode = Mode::FalseNull);
    let known = hydrate(&engine, &account, REPO_A, DetailFacet::Participants).await;
    assert_eq!(native(&known.entries[0]).approved, Some(false));
    assert!(
        native(&known.entries[0]).state.is_none()
            && native(&known.entries[0]).participated_at.is_none()
            && native(&known.entries[0]).user.login.is_none()
    );
    assert!(
        known.entries[0]
            .field_mask
            .contains(&DetailField::ParticipantState)
    );
    assert_ne!(
        validation(&known.entries[0], DetailField::ParticipantApproved),
        &old_validation
    );
    fixture.update(|scenario| scenario.mode = Mode::OlderAction);
    let old_action = hydrate(&engine, &account, REPO_A, DetailFacet::Participants).await;
    assert_eq!(
        native(&old_action.entries[0]).state.as_deref(),
        Some("future_native_state")
    );
    assert_eq!(
        native(&old_action.entries[0]).role.as_deref(),
        Some("FUTURE_ROLE")
    );
    assert_eq!(
        native(&old_action.entries[0]).participated_at.as_deref(),
        Some("2001-01-01T00:00:00Z")
    );
    fixture.update(|scenario| scenario.mode = Mode::ConflictingBody);
    let independent = hydrate(&engine, &account, REPO_A, DetailFacet::Participants).await;
    assert_eq!(native(&independent.entries[0]).approved, Some(true));
    assert!(independent.metadata.is_none());
    assert!(
        independent
            .evidence
            .source
            .as_ref()
            .unwrap()
            .provider_updated_at
            .is_none()
    );
    assert!(
        independent.entries[0].updated_at.is_none() && independent.entries[0].head_oid.is_none()
    );
    let unchanged = database
        .detail(query(&account, REPO_A, DetailFacet::Body))
        .await
        .unwrap();
    assert_eq!(unchanged.body, body.body);
    assert_eq!(unchanged.metadata, body.metadata);
    assert_eq!(unchanged.evidence, body.evidence);
}

#[tokio::test]
async fn bitbucket_participant_invalid_array_or_flag_preserves_cache_and_draft_with_durable_actor_quota()
 {
    for mode in [Mode::Absent, Mode::ApprovedNull] {
        let dir = tempfile::tempdir().unwrap();
        let database = store(dir.path()).await;
        let fixture = Fixture::new();
        let vault = Arc::new(Vault::default());
        let clock = Clock::new();
        let engine = runtime(database.clone(), vault.clone(), &fixture, clock.clone());
        let account = connect(&engine, "actor-a").await;
        select(&engine, &account, REPO_A).await;
        let private = draft(&engine, &account, REPO_A, "do not lose authored text").await;
        let saved = hydrate(&engine, &account, REPO_A, DetailFacet::Participants).await;
        fixture.update(|scenario| scenario.mode = mode);
        let failed = hydrate(&engine, &account, REPO_A, DetailFacet::Participants).await;
        assert_eq!(failed.entries, saved.entries);
        assert_eq!(
            failed.evidence.facet_revision,
            saved.evidence.facet_revision
        );
        assert_eq!(failed.evidence.value_source, saved.evidence.value_source);
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
        assert_eq!(
            database
                .draft(&account.id, &private.subject_id)
                .await
                .unwrap(),
            Some(private.clone())
        );
        let calls = fixture.count();
        let loads = vault.loads.load(Ordering::SeqCst);
        database.close().await;
        drop(engine);
        drop(database);
        let reopened = store(dir.path()).await;
        let cold = runtime(reopened.clone(), vault.clone(), &fixture, clock);
        assert_eq!(
            reopened
                .detail(query(&account, REPO_A, DetailFacet::Participants))
                .await
                .unwrap(),
            failed
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
}

#[tokio::test]
async fn bitbucket_participant_empty_collection_is_authoritative_without_deleting_private_drafts() {
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
    let private = draft(
        &engine,
        &account,
        REPO_A,
        "retain on explicit provider emptiness",
    )
    .await;
    assert_eq!(
        hydrate(&engine, &account, REPO_A, DetailFacet::Participants)
            .await
            .entries
            .len(),
        2
    );
    fixture.update(|scenario| scenario.mode = Mode::Empty);
    let empty = hydrate(&engine, &account, REPO_A, DetailFacet::Participants).await;
    assert!(empty.entries.is_empty());
    assert_eq!(empty.evidence.saved_empty, Some(true));
    assert_eq!(empty.evidence.coverage.state, CoverageState::Complete);
    assert_eq!(empty.evidence.observed_state, DetailValueState::Known);
    assert_eq!(
        database
            .draft(&account.id, &private.subject_id)
            .await
            .unwrap(),
        Some(private)
    );
}

#[tokio::test]
async fn bitbucket_participant_permission_denial_is_facet_and_actor_scoped_preserving_body_and_drafts()
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
    select(&engine, &account, REPO_B).await;
    let other = connect(&engine, "actor-b").await;
    select(&engine, &other, REPO_A).await;
    let body = hydrate(&engine, &account, REPO_A, DetailFacet::Body).await;
    hydrate(&engine, &account, REPO_A, DetailFacet::Participants).await;
    let other_repo = hydrate(&engine, &account, REPO_B, DetailFacet::Participants).await;
    let other_actor = hydrate(&engine, &other, REPO_A, DetailFacet::Participants).await;
    let own = draft(&engine, &account, REPO_A, "A private").await;
    let theirs = draft(&engine, &other, REPO_A, "B private").await;
    fixture.update(|scenario| scenario.mode = Mode::Denied);
    let denied = hydrate(&engine, &account, REPO_A, DetailFacet::Participants).await;
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
            .detail(query(actor, repository, DetailFacet::Participants))
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
async fn bitbucket_participant_obsolete_epoch_success_and_quota_error_cannot_cross_replacement() {
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
        hydrate(&engine, &account, REPO_A, DetailFacet::Participants).await;
        let other = connect(&engine, "actor-b").await;
        select(&engine, &other, REPO_A).await;
        hydrate(&engine, &other, REPO_A, DetailFacet::Participants).await;
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
                facet: DetailFacet::Participants,
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
            .detail(query(&replacement, REPO_A, DetailFacet::Participants))
            .await
            .unwrap();
        let theirs = database
            .detail(query(&other, REPO_A, DetailFacet::Participants))
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
                .detail(query(&replacement, REPO_A, DetailFacet::Participants))
                .await
                .unwrap(),
            current
        );
        assert_eq!(
            database
                .detail(query(&other, REPO_A, DetailFacet::Participants))
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
async fn bitbucket_participant_subject_history_still_rejects_held_old_head_and_selection_lease() {
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
        let original = hydrate(&engine, &account, REPO_A, DetailFacet::Participants).await;
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
                facet: DetailFacet::Participants,
            })
            .await
            .unwrap();
        let worker = engine.clone();
        let pending = tokio::spawn(async move { worker.run_next().await });
        tokio::time::timeout(Duration::from_secs(2), fixture.held.entered.notified())
            .await
            .unwrap();
        if selection_churn {
            let scope = DetailFacet::Participants.scope(&subject(REPO_A));
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
                "deselection retires the exact participant lease even after reselection"
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
            .detail(query(&account, REPO_A, DetailFacet::Participants))
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
                .all(|entry| native(entry).user.display_name.as_deref()
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
