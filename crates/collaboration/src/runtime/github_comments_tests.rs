//! Finite synthetic HTTP -> GitHub Comments -> Runtime -> SQLite qualification.
//! Uses no public HTTP, real credentials, OS vault or background worker.
use super::*;
use crate::credentials::CredentialError;
use serde_json::{Value, json};
use std::{
    io::{Read, Write},
    sync::{Condvar, Mutex as StdMutex, atomic::AtomicUsize},
};

const REPO: &str = "9007199254741021";
const PULL: &str = "9007199254740997";
const ISSUE: &str = "9007199254740998";
const SOURCE: &str = "github/comments/2026-03-10";
fn repository_id() -> String {
    format!("github:repository:{REPO}")
}
fn subject(number: u64) -> String {
    if number == 67 {
        format!("github:pull:{PULL}")
    } else {
        assert_eq!(number, 68);
        format!("github:issue:{ISSUE}")
    }
}
fn full_name(renamed: bool) -> &'static str {
    if renamed {
        "renamed/project"
    } else {
        "owner/project"
    }
}
fn comment_path(number: u64) -> String {
    format!("/repositories/{REPO}/issues/{number}/comments")
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
    Edit,
    Older,
    Omitted,
    Oversized,
    NullUser,
    Empty,
    Terminal,
    YieldCap,
    Invalid,
    Denied403,
    Denied404,
    MalformedLink,
    ContradictoryLink,
    ManyRows,
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
fn repository(scenario: Scenario) -> Value {
    let name = full_name(scenario.renamed);
    json!({"id":9007199254741021_u64,"full_name":name,"name":"project","html_url":format!("https://github.com/{name}"),"description":null,"default_branch":"main"})
}
fn item(number: u64, token: &str, scenario: Scenario) -> Value {
    let name = full_name(scenario.renamed);
    let pull = number == 67;
    let resource = if pull { "pulls" } else { "issues" };
    let mut value = json!({"id":if pull{9007199254740997_u64}else{9007199254740998_u64},"number":number,"url":format!("https://api.github.com/repos/{name}/{resource}/{number}"),"html_url":format!("https://github.com/{name}/{}/{number}",if pull{"pull"}else{"issues"}),"title":"Independent parent title","body":format!("Parent Body {token} {number}"),"state":"open","updated_at":"2026-10-04T20:00:00Z","user":{"id":101,"login":"parent-author","html_url":"https://github.com/parent-author"},"labels":[],"assignees":[],"milestone":null});
    if pull {
        value["draft"] = false.into();
        value["head"] = json!({"ref":"feature","sha":if scenario.advanced_head{"c".repeat(40)}else{"a".repeat(40)},"repo":null});
        value["base"] = json!({"ref":"main","sha":"b".repeat(40),"repo":null});
    }
    value
}
fn comment(id: u64, number: u64, token: &str, scenario: Scenario) -> Value {
    let mut value = json!({"id":id,"issue_url":format!("https://api.github.com/repos/{}/issues/{number}",full_name(scenario.renamed)),"body":format!("Comment {number}/{id} {token} <script>raw text</script> café 🦀"),"user":{"id":101,"login":format!("comment-{token}")},"updated_at":if id==2{"2026-10-04T00:00:00Z"}else{"2026-10-04T12:00:00Z"},"created_at":"2001-01-01T00:00:00Z","author_association":"IGNORED_FUTURE_VALUE"});
    if id == 2 {
        match scenario.mode {
            Mode::Edit => {
                value["body"] = "Edited own comment clock".into();
                value["updated_at"] = "2026-10-04T01:00:00Z".into();
            }
            Mode::Older => {
                value["body"] = "Obsolete own comment".into();
                value["user"]["login"] = "obsolete-author".into();
                value["updated_at"] = "2026-10-04T00:30:00Z".into();
            }
            Mode::Omitted => {
                value.as_object_mut().unwrap().remove("body");
                value["updated_at"] = "2026-10-04T13:00:00Z".into();
            }
            Mode::Oversized => {
                value["body"] = "x".repeat(65_537).into();
                value["updated_at"] = "2026-10-04T14:00:00Z".into();
            }
            Mode::NullUser => {
                value["user"] = Value::Null;
                value["body"] = "".into();
                value["updated_at"] = "2026-10-04T15:00:00Z".into();
            }
            Mode::Invalid => value["body"] = Value::Null,
            _ => {}
        }
    }
    value
}
struct Fixture {
    provider: Arc<providers::github::GithubProvider>,
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
        let base =
            reqwest::Url::parse(&format!("http://{}/", listener.local_addr().unwrap())).unwrap();
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
                    assert!(connections.len() < 256, "finite synthetic request budget");
                    let calls = calls.clone();
                    let scenario = scenario.clone();
                    let held = held.clone();
                    let task_base = task_base.clone();
                    connections.push(std::thread::spawn(move||{
                        stream.set_nonblocking(false).unwrap();stream.set_read_timeout(Some(Duration::from_secs(2))).unwrap();stream.set_write_timeout(Some(Duration::from_secs(2))).unwrap();
                        let mut bytes=vec![];
                        loop {let mut chunk=[0;1024];let count=stream.read(&mut chunk).unwrap();bytes.extend_from_slice(&chunk[..count]);assert!(bytes.len()<=16_384);if count==0||bytes.windows(4).any(|v|v==b"\r\n\r\n"){break}}
                        let request=String::from_utf8(bytes).unwrap();let target=request.lines().next().unwrap().split_whitespace().nth(1).unwrap();
                        let token=request.lines().find_map(|line|{let(name,value)=line.split_once(':')?;name.eq_ignore_ascii_case("authorization").then(||value.trim())}).and_then(|value|value.strip_prefix("Bearer ")).unwrap();
                        assert!(matches!(token,"actor-a"|"actor-b"|"replacement-a"));
                        calls.lock().unwrap().push((target.into(),token.into()));let scenario=*scenario.lock().unwrap();
                        let output=if target=="/user" {response(200,"",&json!({"id":if token=="actor-b"{102}else{101},"login":"same-login","name":null}))}
                        else if target.starts_with("/user/repos?"){response(200,"",&json!([repository(scenario)]))}
                        else if let Some(number)=[67,68].into_iter().find(|number|target.starts_with(&format!("{}?",comment_path(*number)))) {
                            assert!(request.to_ascii_lowercase().contains("x-github-api-version: 2026-03-10"));
                            assert!(!request.to_ascii_lowercase().contains("if-none-match")&&!request.to_ascii_lowercase().contains("if-modified-since"));
                            let url=reqwest::Url::parse(&format!("http://fixture.invalid{target}")).unwrap();
                            let pairs=url.query_pairs().collect::<Vec<_>>();
                            assert!(pairs.iter().all(|(key,_)|key=="per_page"||key=="page"));assert_eq!(pairs.iter().find(|(key,_)|key=="per_page").unwrap().1,"50");
                            let page=pairs.iter().find(|(key,_)|key=="page").map(|(_,value)|value.parse::<u64>().unwrap()).unwrap_or(1);assert!(page<=21);
                            if token=="actor-a"&&number==67&&held.armed.swap(false,Ordering::SeqCst) {
                                let mut row=comment(2,number,token,Scenario::default());row["body"]="obsolete held comment".into();row["updated_at"]="2026-10-05T00:00:00Z".into();
                                held.wait();response(if held.rate_limited.load(Ordering::SeqCst){429}else{200},"Retry-After: 120\r\n",&json!([row]))
                            } else if token=="actor-a"&&number==67&&matches!(scenario.mode,Mode::Denied403|Mode::Denied404) {response(if matches!(scenario.mode,Mode::Denied403){403}else{404},"",&json!({"message":"synthetic denied"}))}
                            else {
                                let paged=matches!(scenario.mode,Mode::Terminal|Mode::YieldCap|Mode::ManyRows);
                                let has_next=paged&&match scenario.mode{Mode::Terminal=>page<3,Mode::ManyRows=>page<2,_=>true};
                                let headers=if matches!(scenario.mode,Mode::MalformedLink){"Link: <truncated\r\nRetry-After: 120\r\n".into()}
                                else if matches!(scenario.mode,Mode::ContradictoryLink){format!("Link: <{}{}?per_page=50&page=9>; rel=\"last\"\r\nRetry-After: 120\r\n",task_base,comment_path(number).trim_start_matches('/').trim_end_matches('/'))}
                                else if has_next {format!("Link: <{}{}?per_page=50&page={}>; rel=\"next\"\r\n",task_base,comment_path(number).trim_start_matches('/'),page+1)}
                                else if matches!(scenario.mode,Mode::Invalid){"Retry-After: 120\r\n".into()}else{String::new()};
                                let rows=if matches!(scenario.mode,Mode::ManyRows){if page==1{(1..=50).rev().map(|id|comment(id,number,token,scenario)).collect()}else{vec![comment(51,number,token,scenario)]}}
                                else if paged||matches!(scenario.mode,Mode::Empty){vec![]}else{vec![comment(10,number,token,scenario),comment(2,number,token,scenario)]};
                                response(200,&headers,&json!(rows))
                            }
                        } else {
                            let name=full_name(scenario.renamed);
                            if target==format!("/repos/{name}/pulls/67"){response(200,"",&item(67,token,scenario))}
                            else if target==format!("/repos/{name}/issues/68"){response(200,"",&item(68,token,scenario))}
                            else if target.starts_with(&format!("/repos/{name}/pulls?")){response(200,"",&json!([item(67,token,scenario)]))}
                            else {assert!(target.starts_with(&format!("/repos/{name}/issues?")),"unexpected owned route {target}");response(200,"",&json!([item(68,token,scenario)]))}
                        };
                        stream.write_all(output.as_bytes()).unwrap();
                    }));
                }
                for connection in connections {
                    connection.join().unwrap();
                }
            })
        };
        Self {
            provider: Arc::new(providers::github::GithubProvider::for_test_base(base)),
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
    fn comment_calls(&self) -> usize {
        self.calls
            .lock()
            .unwrap()
            .iter()
            .filter(|(target, _)| target.contains("/comments?"))
            .count()
    }
    fn update(&self, f: impl FnOnce(&mut Scenario)) {
        f(&mut self.scenario.lock().unwrap());
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
    let mut engine = CollaborationRuntime::new(database, vault, fixture.provider.clone());
    engine.clock = clock;
    engine
}
async fn connect(engine: &CollaborationRuntime, token: &str) -> RemoteAccount {
    let account = engine.connect_github(token.into()).await.unwrap();
    assert!(engine.run_next().await);
    account
}
async fn select(engine: &CollaborationRuntime, account: &RemoteAccount) {
    engine
        .select_repository(&account.id, &repository_id(), true)
        .await
        .unwrap();
    for _ in 0..2 {
        assert!(engine.run_next().await);
    }
}
fn query(account: &RemoteAccount, number: u64, facet: DetailFacet) -> DetailQuery {
    DetailQuery {
        account_id: account.id.clone(),
        subject_id: subject(number),
        facet,
        cursor: None,
        limit: 100,
    }
}
fn demand(account: &RemoteAccount, number: u64, facet: DetailFacet) -> HydrateDetailRequest {
    HydrateDetailRequest {
        account_id: account.id.clone(),
        authorization_epoch: account.authorization_epoch.clone(),
        subject_id: subject(number),
        facet,
    }
}
async fn hydrate(
    engine: &CollaborationRuntime,
    account: &RemoteAccount,
    number: u64,
    facet: DetailFacet,
) -> DetailSnapshot {
    engine
        .hydrate_detail(demand(account, number, facet))
        .await
        .unwrap();
    assert!(engine.run_next().await);
    engine
        .store
        .detail(query(account, number, facet))
        .await
        .unwrap()
}
async fn draft(
    engine: &CollaborationRuntime,
    account: &RemoteAccount,
    number: u64,
    text: &str,
) -> LocalDraft {
    engine
        .save_draft(LocalDraft {
            account_id: account.id.clone(),
            subject_id: subject(number),
            body: text.into(),
            generation: "0".into(),
        })
        .await
        .unwrap()
}
fn validation(entry: &DetailEntry, field: DetailField) -> &DetailFieldValidation {
    entry
        .field_validations
        .iter()
        .find(|value| value.field == field)
        .unwrap()
}
fn assert_scope(actual: StoredScope, expected: &StoredScope) {
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
        )
    );
}
async fn saved_native_facet(
    database: &Store,
    account: &RemoteAccount,
    facet: DetailFacet,
) -> DetailSnapshot {
    // Synthetic storage admission establishes independent native facet bytes;
    // it makes no claim that GitHub supports Participants or Tasks HTTP.
    let lease = database
        .begin_detail(
            &account.id,
            &account.authorization_epoch,
            &subject(67),
            facet,
        )
        .await
        .unwrap();
    let mut commit = detail_tests::fixtures::from_lease(account, facet, lease);
    commit.subject_id = subject(67);
    commit.etag = None;
    let (fields, payload) = if facet == DetailFacet::Participants {
        (
            vec![DetailField::ParticipantLogin],
            NativeDetailPayload::ParticipantV1(ParticipantV1 {
                user: ParticipantUser {
                    provider_id: "synthetic-participant".into(),
                    login: Some("independent participant".into()),
                    display_name: None,
                },
                role: None,
                approved: None,
                state: None,
                participated_at: None,
            }),
        )
    } else {
        (
            vec![
                DetailField::TaskContent,
                DetailField::TaskState,
                DetailField::TaskCreatedAt,
                DetailField::TaskUpdatedAt,
            ],
            NativeDetailPayload::TaskV1(TaskV1 {
                content: DetailValue {
                    state: DetailValueState::Known,
                    text: Some("independent task".into()),
                },
                observed_content_state: DetailValueState::Known,
                creator: TaskActor {
                    provider_id: "synthetic-creator".into(),
                    kind: "user".into(),
                    login: None,
                    display_name: None,
                },
                state: Some("UNRESOLVED".into()),
                created_at: Some("2026-10-04T00:00:00Z".into()),
                updated_at: Some("2026-10-04T01:00:00Z".into()),
                pending: None,
                resolved_at: None,
                resolved_by: None,
                comment_id: None,
            }),
        )
    };
    let item = database
        .detail_subject(&account.id, &subject(67))
        .await
        .unwrap();
    let repository = database
        .repository(&account.id, &repository_id())
        .await
        .unwrap();
    commit.subject_binding = Some(DetailSubjectBinding {
        repository_id: repository.id,
        repository_provider_id: repository.provider_id,
        provider_id: item.provider_id,
        number: item.number,
        kind: item.kind,
        head_oid: item.head_oid,
    });
    // Source declares the complete native family; each entry carries only its
    // actually observed fields. Existing Store admission requires both this
    // family declaration and the exact captured subject binding.
    commit.source.field_mask = if facet == DetailFacet::Participants {
        vec![
            DetailField::ParticipantLogin,
            DetailField::ParticipantDisplayName,
            DetailField::ParticipantRole,
            DetailField::ParticipantApproved,
            DetailField::ParticipantState,
            DetailField::ParticipantParticipatedAt,
        ]
    } else {
        vec![
            DetailField::TaskContent,
            DetailField::TaskCreatorLogin,
            DetailField::TaskCreatorDisplayName,
            DetailField::TaskState,
            DetailField::TaskCreatedAt,
            DetailField::TaskUpdatedAt,
            DetailField::TaskPending,
            DetailField::TaskResolvedAt,
            DetailField::TaskResolver,
            DetailField::TaskResolverLogin,
            DetailField::TaskResolverDisplayName,
            DetailField::TaskCommentId,
        ]
    };
    commit.entries = vec![DetailEntry {
        id: format!("independent-{}", facet.name()),
        provider_id: format!("independent-{}", facet.name()),
        author: None,
        title: None,
        state: None,
        body: DetailValue::default(),
        observed_body_state: DetailValueState::NotLoaded,
        updated_at: None,
        head_oid: None,
        field_mask: fields,
        field_validations: vec![],
        native: Some(payload),
    }];
    database.apply_detail(commit).await.unwrap();
    database.detail(query(account, 67, facet)).await.unwrap()
}

#[tokio::test]
async fn github_comments_pr_issue_two_actors_immutable_rename_and_cold_reads_keep_private_cas() {
    let dir = tempfile::tempdir().unwrap();
    let database = store(dir.path()).await;
    let fixture = Fixture::new();
    let vault = Arc::new(Vault::default());
    let clock = Clock::new();
    let engine = runtime(database.clone(), vault.clone(), &fixture, clock.clone());
    let first = connect(&engine, "actor-a").await;
    select(&engine, &first).await;
    let loads = vault.loads.load(Ordering::SeqCst);
    let calls = fixture.count();
    assert_eq!(
        database
            .detail(query(&first, 67, DetailFacet::Comments))
            .await
            .unwrap()
            .evidence
            .availability,
        DetailAvailability::Missing
    );
    assert_eq!(fixture.comment_calls(), 0);
    assert_eq!(fixture.count(), calls);
    assert_eq!(vault.loads.load(Ordering::SeqCst), loads);
    hydrate(&engine, &first, 67, DetailFacet::Body).await;
    saved_native_facet(&database, &first, DetailFacet::Participants).await;
    saved_native_facet(&database, &first, DetailFacet::Tasks).await;
    let pull = hydrate(&engine, &first, 67, DetailFacet::Comments).await;
    let issue = hydrate(&engine, &first, 68, DetailFacet::Comments).await;
    let other = connect(&engine, "actor-b").await;
    select(&engine, &other).await;
    let theirs = hydrate(&engine, &other, 67, DetailFacet::Comments).await;
    assert_eq!(first.login, other.login);
    assert_ne!(first.id, other.id);
    for saved in [&pull, &issue, &theirs] {
        assert_eq!(
            saved
                .entries
                .iter()
                .map(|entry| entry.provider_id.as_str())
                .collect::<Vec<_>>(),
            vec!["2", "10"]
        );
        assert_eq!(saved.entries[0].id, "github-comment:00000000000000000002");
        assert_eq!(saved.entries[1].id, "github-comment:00000000000000000010");
        let source = saved.evidence.source.as_ref().unwrap();
        assert_eq!(source.source, SOURCE);
        assert_eq!(source.adapter_version, 1);
        assert!(source.provider_updated_at.is_none());
        assert_eq!(
            source.field_mask,
            vec![
                DetailField::Body,
                DetailField::Author,
                DetailField::UpdatedAt
            ]
        );
        for row in &saved.entries {
            assert!(
                row.title.is_none()
                    && row.state.is_none()
                    && row.head_oid.is_none()
                    && row.native.is_none()
            );
            assert_eq!(row.field_validations.len(), 3);
        }
    }
    assert_eq!(pull.entries[0].id, theirs.entries[0].id);
    assert_ne!(pull.entries[0].body, theirs.entries[0].body);
    assert_ne!(pull.entries[0].body, issue.entries[0].body);
    let own = draft(&engine, &first, 67, "Alice private editor café 🦀").await;
    let other_draft = draft(&engine, &other, 67, "Bob separate editor").await;
    fixture.update(|scenario| scenario.renamed = true);
    let refused_rename = hydrate(&engine, &first, 67, DetailFacet::Comments).await;
    assert_eq!(
        refused_rename.entries, pull.entries,
        "a renamed named-parent response needs matching repository metadata"
    );
    assert!(refused_rename.evidence.sync.error.is_some());
    clock.advance(61);
    engine
        .refresh(RefreshRequest {
            account_id: first.id.clone(),
            repository_id: None,
            kind: None,
        })
        .await
        .unwrap();
    assert!(engine.run_next().await);
    assert_eq!(
        database
            .repository(&first.id, &repository_id())
            .await
            .unwrap()
            .full_name,
        "renamed/project"
    );
    let renamed = hydrate(&engine, &first, 67, DetailFacet::Comments).await;
    assert_eq!(renamed.entries[0].id, pull.entries[0].id);
    assert_eq!(renamed.entries[0].provider_id, "2");
    let comment_targets = fixture
        .calls
        .lock()
        .unwrap()
        .iter()
        .filter(|(target, _)| target.contains("/comments?"))
        .map(|(target, _)| target.clone())
        .collect::<Vec<_>>();
    assert!(
        comment_targets
            .iter()
            .all(|target| target.starts_with(&format!("/repositories/{REPO}/issues/")))
    );
    assert!(
        comment_targets
            .iter()
            .all(|target| !target.contains("/repos/") && !target.contains("renamed"))
    );
    let stale = engine
        .save_draft(LocalDraft {
            generation: "0".into(),
            body: "obsolete editor must fail".into(),
            ..own.clone()
        })
        .await
        .unwrap_err();
    assert_eq!(stale.code, ErrorCode::StaleView);
    assert_eq!(
        database.draft(&first.id, &own.subject_id).await.unwrap(),
        Some(own.clone())
    );
    let updated = engine
        .save_draft(LocalDraft {
            body: "Alice explicit edit".into(),
            ..own
        })
        .await
        .unwrap();
    let saved = database
        .detail(query(&first, 67, DetailFacet::Comments))
        .await
        .unwrap();
    let saved_issue = database
        .detail(query(&first, 68, DetailFacet::Comments))
        .await
        .unwrap();
    let saved_other = database
        .detail(query(&other, 67, DetailFacet::Comments))
        .await
        .unwrap();
    let mut independent = vec![];
    for facet in [
        DetailFacet::Body,
        DetailFacet::Participants,
        DetailFacet::Tasks,
    ] {
        independent.push((
            facet,
            database.detail(query(&first, 67, facet)).await.unwrap(),
        ));
    }
    let calls = fixture.count();
    let loads = vault.loads.load(Ordering::SeqCst);
    database.close().await.unwrap();
    drop(engine);
    drop(database);
    let reopened = store(dir.path()).await;
    let cold = runtime(reopened.clone(), vault.clone(), &fixture, clock);
    assert!(!cold.run_next().await);
    for (actor, number, expected) in [
        (&first, 67, saved),
        (&first, 68, saved_issue),
        (&other, 67, saved_other),
    ] {
        assert_eq!(
            reopened
                .detail(query(actor, number, DetailFacet::Comments))
                .await
                .unwrap(),
            expected
        );
    }
    for (facet, saved) in independent {
        assert_eq!(
            reopened.detail(query(&first, 67, facet)).await.unwrap(),
            saved
        );
    }
    for saved in [updated, other_draft] {
        assert_eq!(
            reopened
                .draft(&saved.account_id, &saved.subject_id)
                .await
                .unwrap(),
            Some(saved)
        );
    }
    assert_eq!(fixture.count(), calls);
    assert_eq!(vault.loads.load(Ordering::SeqCst), loads);
}

#[tokio::test]
async fn github_comments_own_row_clock_edits_older_omitted_oversized_and_nullable_author_keep_other_facets()
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
    select(&engine, &account).await;
    let body = hydrate(&engine, &account, 67, DetailFacet::Body).await;
    let participants = saved_native_facet(&database, &account, DetailFacet::Participants).await;
    let tasks = saved_native_facet(&database, &account, DetailFacet::Tasks).await;
    let private = draft(
        &engine,
        &account,
        67,
        "authored text is independent of comment observations",
    )
    .await;
    let initial = hydrate(&engine, &account, 67, DetailFacet::Comments).await;
    assert_eq!(
        initial.entries[0].updated_at.as_deref(),
        Some("2026-10-04T00:00:00Z")
    );
    assert_eq!(
        initial.entries[1].updated_at.as_deref(),
        Some("2026-10-04T12:00:00Z")
    );
    clock.advance(1);
    fixture.update(|scenario| scenario.mode = Mode::Edit);
    let edited = hydrate(&engine, &account, 67, DetailFacet::Comments).await;
    assert_eq!(
        edited.entries[0].body.text.as_deref(),
        Some("Edited own comment clock")
    );
    assert_eq!(
        edited.entries[0].updated_at.as_deref(),
        Some("2026-10-04T01:00:00Z")
    );
    assert_eq!(edited.entries[1].body, initial.entries[1].body);
    let body_validation = validation(&edited.entries[0], DetailField::Body).clone();
    for mode in [Mode::Omitted, Mode::Oversized] {
        clock.advance(1);
        fixture.update(|scenario| scenario.mode = mode);
        let retained = hydrate(&engine, &account, 67, DetailFacet::Comments).await;
        assert_eq!(retained.entries[0].body, edited.entries[0].body);
        assert_eq!(
            retained.entries[0].observed_body_state,
            if matches!(mode, Mode::Omitted) {
                DetailValueState::Omitted
            } else {
                DetailValueState::Oversized
            }
        );
        assert_eq!(
            validation(&retained.entries[0], DetailField::Body),
            &body_validation
        );
        assert!(retained.entries[0].field_mask.contains(&DetailField::Body));
        assert!(
            retained
                .evidence
                .source
                .as_ref()
                .unwrap()
                .provider_updated_at
                .is_none()
        );
    }
    fixture.update(|scenario| scenario.mode = Mode::NullUser);
    clock.advance(1);
    let cleared = hydrate(&engine, &account, 67, DetailFacet::Comments).await;
    assert_eq!(
        cleared.entries[0].body,
        DetailValue {
            state: DetailValueState::Known,
            text: Some(String::new())
        }
    );
    assert!(cleared.entries[0].author.is_none());
    assert_eq!(
        cleared.entries[0].observed_body_state,
        DetailValueState::Known
    );
    fixture.update(|scenario| scenario.mode = Mode::Older);
    clock.advance(1);
    let older = hydrate(&engine, &account, 67, DetailFacet::Comments).await;
    assert_eq!(older.entries[0].body, cleared.entries[0].body);
    assert_eq!(older.entries[0].author, cleared.entries[0].author);
    assert_eq!(older.entries[0].updated_at, cleared.entries[0].updated_at);
    assert_eq!(
        older.entries[0].field_validations,
        cleared.entries[0].field_validations
    );
    for (facet, saved) in [
        (DetailFacet::Body, body),
        (DetailFacet::Participants, participants),
        (DetailFacet::Tasks, tasks),
    ] {
        let current = database.detail(query(&account, 67, facet)).await.unwrap();
        assert_eq!(current.body, saved.body);
        assert_eq!(current.metadata, saved.metadata);
        assert_eq!(current.entries, saved.entries);
        assert_eq!(current.evidence, saved.evidence);
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
async fn github_comments_invalid_200_and_bad_link_quota_keep_cache_and_cold_durable_actor_cooldown()
{
    for mode in [Mode::Invalid, Mode::MalformedLink, Mode::ContradictoryLink] {
        let dir = tempfile::tempdir().unwrap();
        let database = store(dir.path()).await;
        let fixture = Fixture::new();
        let vault = Arc::new(Vault::default());
        let clock = Clock::new();
        let engine = runtime(database.clone(), vault.clone(), &fixture, clock.clone());
        let account = connect(&engine, "actor-a").await;
        select(&engine, &account).await;
        let other = connect(&engine, "actor-b").await;
        select(&engine, &other).await;
        let private = draft(&engine, &account, 67, "preserved on invalid collection").await;
        let saved = hydrate(&engine, &account, 67, DetailFacet::Comments).await;
        let theirs = hydrate(&engine, &other, 67, DetailFacet::Comments).await;
        fixture.update(|scenario| scenario.mode = mode);
        let calls = fixture.comment_calls();
        let failed = hydrate(&engine, &account, 67, DetailFacet::Comments).await;
        assert_eq!(
            fixture.comment_calls(),
            calls + 1,
            "invalid first page cannot publish or follow a next URL"
        );
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
        assert!(
            database
                .scope_state(&other.id, "provider:rest")
                .await
                .unwrap()
                .is_none()
        );
        assert_eq!(
            database
                .detail(query(&other, 67, DetailFacet::Comments))
                .await
                .unwrap()
                .entries,
            theirs.entries
        );
        let calls = fixture.count();
        let loads = vault.loads.load(Ordering::SeqCst);
        database.close().await.unwrap();
        drop(engine);
        drop(database);
        let reopened = store(dir.path()).await;
        let cold = runtime(reopened.clone(), vault.clone(), &fixture, clock);
        assert_eq!(
            reopened
                .detail(query(&account, 67, DetailFacet::Comments))
                .await
                .unwrap(),
            failed
        );
        assert_scope(
            reopened
                .scope_state(&account.id, "provider:rest")
                .await
                .unwrap()
                .unwrap(),
            &rest,
        );
        assert_eq!(
            reopened
                .draft(&account.id, &private.subject_id)
                .await
                .unwrap(),
            Some(private)
        );
        cold.hydrate_detail(demand(&account, 67, DetailFacet::Comments))
            .await
            .unwrap();
        assert!(
            !cold.run_next().await,
            "persisted account barrier refuses explicit cold work before vault or HTTP"
        );
        assert_eq!(fixture.count(), calls);
        assert_eq!(vault.loads.load(Ordering::SeqCst), loads);
    }
}

async fn stored_entry_json(
    path: &std::path::Path,
    account: &RemoteAccount,
    number: u64,
) -> Vec<String> {
    use sqlx::Connection;
    let mut connection = sqlx::SqliteConnection::connect_with(
        &sqlx::sqlite::SqliteConnectOptions::new()
            .filename(path.join("synthetic.sqlite"))
            .read_only(true),
    )
    .await
    .unwrap();
    let rows = sqlx::query_scalar("SELECT json FROM detail_entries WHERE account_id=? AND subject_id=? AND facet='comments' ORDER BY id")
        .bind(&account.id).bind(subject(number)).fetch_all(&mut connection).await.unwrap();
    connection.close().await.unwrap();
    rows
}

#[tokio::test]
async fn github_comments_403_404_are_facet_denial_and_never_successful_empty_history() {
    for mode in [Mode::Denied403, Mode::Denied404] {
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
        select(&engine, &account).await;
        let other = connect(&engine, "actor-b").await;
        select(&engine, &other).await;
        let body = hydrate(&engine, &account, 67, DetailFacet::Body).await;
        let participants = saved_native_facet(&database, &account, DetailFacet::Participants).await;
        let tasks = saved_native_facet(&database, &account, DetailFacet::Tasks).await;
        let saved = hydrate(&engine, &account, 67, DetailFacet::Comments).await;
        let issue = hydrate(&engine, &account, 68, DetailFacet::Comments).await;
        let theirs = hydrate(&engine, &other, 67, DetailFacet::Comments).await;
        let private = draft(&engine, &account, 67, "private despite facet denial").await;
        let stored = stored_entry_json(dir.path(), &account, 67).await;
        assert_eq!(stored.len(), 2);
        fixture.update(|scenario| scenario.mode = mode);
        let denied = hydrate(&engine, &account, 67, DetailFacet::Comments).await;
        assert_eq!(
            stored_entry_json(dir.path(), &account, 67).await,
            stored,
            "denial hides retained historical bytes; it is never an empty enumeration"
        );
        assert!(denied.entries.is_empty());
        assert_eq!(
            denied.evidence.availability,
            DetailAvailability::Unavailable
        );
        assert_eq!(
            denied.evidence.access_reason,
            Some(CapabilityReason::PermissionDenied)
        );
        assert_ne!(denied.evidence.saved_empty, Some(true));
        assert_ne!(
            denied.evidence.facet_revision,
            saved.evidence.facet_revision
        );
        for (actor, number, facet, expected) in [
            (&account, 67, DetailFacet::Body, body),
            (&account, 67, DetailFacet::Participants, participants),
            (&account, 67, DetailFacet::Tasks, tasks),
            (&account, 68, DetailFacet::Comments, issue),
            (&other, 67, DetailFacet::Comments, theirs),
        ] {
            let actual = database.detail(query(actor, number, facet)).await.unwrap();
            assert_eq!(actual.body, expected.body);
            assert_eq!(actual.metadata, expected.metadata);
            assert_eq!(actual.entries, expected.entries);
            assert_eq!(actual.evidence, expected.evidence);
        }
        assert_eq!(
            database.account(&account.id).await.unwrap().state,
            AccountState::Active
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
async fn github_comments_terminal_uncertain_keeps_history_but_initial_full_empty_removes_it() {
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
    select(&engine, &account).await;
    let saved = hydrate(&engine, &account, 67, DetailFacet::Comments).await;
    let private = draft(
        &engine,
        &account,
        67,
        "draft survives mutable history paging",
    )
    .await;
    fixture.update(|scenario| scenario.mode = Mode::Terminal);
    let first = hydrate(&engine, &account, 67, DetailFacet::Comments).await;
    assert_eq!(first.entries, saved.entries);
    assert_eq!(first.evidence.coverage.state, CoverageState::Partial);
    assert!(engine.run_next().await);
    assert!(engine.run_next().await);
    let terminal = database
        .detail(query(&account, 67, DetailFacet::Comments))
        .await
        .unwrap();
    assert_eq!(terminal.entries, saved.entries);
    assert_eq!(terminal.evidence.coverage.state, CoverageState::Partial);
    assert!(!terminal.evidence.coverage.remote_has_more);
    assert_eq!(fixture.comment_calls(), 4);
    fixture.update(|scenario| scenario.mode = Mode::Empty);
    let empty = hydrate(&engine, &account, 67, DetailFacet::Comments).await;
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
async fn github_comments_obsolete_epoch_held_200_and_429_cannot_change_replacement_or_other_actor()
{
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
        select(&engine, &account).await;
        hydrate(&engine, &account, 67, DetailFacet::Comments).await;
        let other = connect(&engine, "actor-b").await;
        select(&engine, &other).await;
        hydrate(&engine, &other, 67, DetailFacet::Comments).await;
        let private = draft(&engine, &account, 67, "old epoch private text").await;
        fixture
            .held
            .rate_limited
            .store(rate_limited, Ordering::SeqCst);
        fixture.held.armed.store(true, Ordering::SeqCst);
        engine
            .hydrate_detail(demand(&account, 67, DetailFacet::Comments))
            .await
            .unwrap();
        let worker = engine.clone();
        let pending = tokio::spawn(async move { worker.run_next().await });
        tokio::time::timeout(Duration::from_secs(2), fixture.held.entered.notified())
            .await
            .unwrap();
        let replacement = engine.connect_github("replacement-a".into()).await.unwrap();
        assert_eq!(replacement.id, account.id);
        assert_eq!(replacement.authorization_epoch, "2");
        let revision = database.revision().await.unwrap();
        let current = database
            .detail(query(&replacement, 67, DetailFacet::Comments))
            .await
            .unwrap();
        let theirs = database
            .detail(query(&other, 67, DetailFacet::Comments))
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
                .detail(query(&replacement, 67, DetailFacet::Comments))
                .await
                .unwrap(),
            current
        );
        assert_eq!(
            database
                .detail(query(&other, 67, DetailFacet::Comments))
                .await
                .unwrap(),
            theirs
        );
        for actor in [&replacement, &other] {
            assert!(
                database
                    .scope_state(&actor.id, "provider:rest")
                    .await
                    .unwrap()
                    .is_none()
            );
        }
        assert_eq!(
            database
                .draft(&replacement.id, &private.subject_id)
                .await
                .unwrap(),
            Some(private)
        );
    }
}

async fn advance_summary(
    engine: &CollaborationRuntime,
    fixture: &Fixture,
    account: &RemoteAccount,
) {
    fixture.update(|scenario| scenario.advanced_head = true);
    let repository = engine
        .store
        .repository(&account.id, &repository_id())
        .await
        .unwrap();
    // Independent actual HTTP producer into a native feed transaction. One
    // Runtime serializes dispatch; this does not pretend it runs two jobs at once.
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
    let run_id = engine
        .store
        .begin_sync(&account.id, &account.authorization_epoch, &scope)
        .await
        .unwrap();
    engine
        .store
        .apply_page(PageCommit {
            account_id: account.id.clone(),
            authorization_epoch: account.authorization_epoch.clone(),
            scope,
            run_id,
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
        engine
            .store
            .item(&account.id, &subject(67))
            .await
            .unwrap()
            .item
            .unwrap()
            .head_oid,
        Some("c".repeat(40))
    );
}

#[tokio::test]
async fn github_comments_subject_history_held_reply_respects_head_selection_and_facet_denial_fences()
 {
    for fence in ["head", "selection", "denial"] {
        let dir = tempfile::tempdir().unwrap();
        let database = store(dir.path()).await;
        let fixture = Fixture::new();
        let vault = Arc::new(Vault::default());
        let clock = Clock::new();
        let engine = runtime(database.clone(), vault.clone(), &fixture, clock.clone());
        let account = connect(&engine, "actor-a").await;
        select(&engine, &account).await;
        let original = hydrate(&engine, &account, 67, DetailFacet::Comments).await;
        let private = draft(
            &engine,
            &account,
            67,
            "private while captured comment authority retires",
        )
        .await;
        fixture.held.armed.store(true, Ordering::SeqCst);
        engine
            .hydrate_detail(demand(&account, 67, DetailFacet::Comments))
            .await
            .unwrap();
        let worker = engine.clone();
        let pending = tokio::spawn(async move { worker.run_next().await });
        tokio::time::timeout(Duration::from_secs(2), fixture.held.entered.notified())
            .await
            .unwrap();
        let scope = DetailFacet::Comments.scope(&subject(67));
        match fence {
            "head" => advance_summary(&engine, &fixture, &account).await,
            "selection" => {
                let prior = database
                    .scope_state(&account.id, &scope)
                    .await
                    .unwrap()
                    .unwrap();
                engine
                    .select_repository(&account.id, &repository_id(), false)
                    .await
                    .unwrap();
                engine
                    .select_repository(&account.id, &repository_id(), true)
                    .await
                    .unwrap();
                assert_ne!(
                    database
                        .scope_state(&account.id, &scope)
                        .await
                        .unwrap()
                        .unwrap()
                        .run_id,
                    prior.run_id
                );
            }
            "denial" => {
                database
                    .set_sync_status(
                        &account.id,
                        &account.authorization_epoch,
                        &scope,
                        SyncStatus {
                            state: SyncState::Error,
                            last_success_at: None,
                            next_retry_at: None,
                            error: Some(CollaborationError::new(
                                ErrorCode::PermissionDenied,
                                "Synthetic independent access loss",
                            )),
                        },
                    )
                    .await
                    .unwrap();
            }
            _ => unreachable!(),
        }
        let before = database
            .detail(query(&account, 67, DetailFacet::Comments))
            .await
            .unwrap();
        fixture.held.release();
        assert!(
            tokio::time::timeout(Duration::from_secs(2), pending)
                .await
                .unwrap()
                .unwrap()
        );
        let after = database
            .detail(query(&account, 67, DetailFacet::Comments))
            .await
            .unwrap();
        assert_eq!(after.entries, before.entries);
        assert_eq!(
            after.evidence.facet_revision,
            before.evidence.facet_revision
        );
        assert!(
            after
                .entries
                .iter()
                .all(|entry| entry.body.text.as_deref() != Some("obsolete held comment"))
        );
        if fence == "denial" {
            assert_eq!(after.evidence.availability, DetailAvailability::Unavailable);
            assert_eq!(
                after.evidence.access_reason,
                Some(CapabilityReason::PermissionDenied)
            );
        } else {
            assert_eq!(after.entries, original.entries);
        }
        assert_eq!(
            database
                .draft(&account.id, &private.subject_id)
                .await
                .unwrap(),
            Some(private.clone())
        );
        let budget = database
            .scope_state(&account.id, "provider:rest")
            .await
            .unwrap()
            .expect("held same-epoch 200 quota survives rejection by captured authority");
        assert_eq!(budget.sync.state, SyncState::RateLimited, "fence {fence}");
        assert_eq!(
            budget.sync.next_retry_at,
            Some(engine.future_string(120)),
            "positive observed quota remains independent of rejected comment data"
        );
        let calls = fixture.count();
        let loads = vault.loads.load(Ordering::SeqCst);
        database.close().await.unwrap();
        drop(engine);
        drop(database);
        let reopened = store(dir.path()).await;
        let cold = runtime(reopened.clone(), vault.clone(), &fixture, clock);
        assert_scope(
            reopened
                .scope_state(&account.id, "provider:rest")
                .await
                .unwrap()
                .unwrap(),
            &budget,
        );
        cold.hydrate_detail(demand(&account, 68, DetailFacet::Body))
            .await
            .unwrap();
        assert!(
            !cold.run_next().await,
            "cold account quota fences sibling Body before credential or HTTP dispatch"
        );
        let restored = reopened
            .detail(query(&account, 67, DetailFacet::Comments))
            .await
            .unwrap();
        assert_eq!(restored.entries, after.entries);
        assert_eq!(restored.evidence, after.evidence);
        assert_eq!(
            reopened
                .draft(&account.id, &private.subject_id)
                .await
                .unwrap(),
            Some(private)
        );
        assert_eq!(fixture.count(), calls, "quota admission performs no HTTP");
        assert_eq!(
            vault.loads.load(Ordering::SeqCst),
            loads,
            "quota admission never opens the vault"
        );
    }
}

#[tokio::test]
async fn github_comments_twenty_page_budget_survives_ten_page_yield_and_two_cold_reopens() {
    let dir = tempfile::tempdir().unwrap();
    let database = store(dir.path()).await;
    let fixture = Fixture::new();
    let vault = Arc::new(Vault::default());
    let clock = Clock::new();
    let engine = runtime(database.clone(), vault.clone(), &fixture, clock.clone());
    let account = connect(&engine, "actor-a").await;
    select(&engine, &account).await;
    let saved = hydrate(&engine, &account, 67, DetailFacet::Comments).await;
    let private = draft(&engine, &account, 67, "private under durable comment cap").await;
    fixture.update(|scenario| scenario.mode = Mode::YieldCap);
    engine
        .hydrate_detail(demand(&account, 67, DetailFacet::Comments))
        .await
        .unwrap();
    for _ in 0..10 {
        assert!(engine.run_next().await);
    }
    assert!(
        !engine.run_next().await,
        "native admission yields after ten pages"
    );
    assert_eq!(fixture.comment_calls(), 11);
    let scope = DetailFacet::Comments.scope(&subject(67));
    let yielded = database
        .scope_state(&account.id, &scope)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(yielded.coverage.state, CoverageState::Partial);
    assert!(yielded.coverage.remote_has_more);
    let cursor: Value = serde_json::from_str(yielded.next_cursor.as_ref().unwrap()).unwrap();
    assert_eq!(cursor["pages"], 10);
    assert!(cursor["url"].as_str().unwrap().ends_with("page=11"));
    assert_eq!(cursor["actor"], account.actor_id);
    assert_eq!(cursor["epoch"], account.authorization_epoch);
    assert_eq!(
        database
            .detail(query(&account, 67, DetailFacet::Comments))
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
    assert_scope(
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
        .hydrate_detail(demand(&account, 67, DetailFacet::Comments))
        .await
        .unwrap();
    for _ in 0..10 {
        assert!(engine.run_next().await);
    }
    assert_eq!(
        fixture.comment_calls(),
        21,
        "accepted count survives a cold scheduler reset"
    );
    let capped = reopened
        .scope_state(&account.id, &scope)
        .await
        .unwrap()
        .unwrap();
    assert_ne!(
        capped.run_id, yielded.run_id,
        "manual resume captures a fresh lease while carrying accepted-page history"
    );
    assert_eq!(capped.coverage.state, CoverageState::Partial);
    assert!(capped.coverage.remote_has_more);
    assert!(capped.next_cursor.as_ref().unwrap().len() <= 4096);
    let cursor: Value = serde_json::from_str(capped.next_cursor.as_ref().unwrap()).unwrap();
    assert_eq!(cursor["pages"], 20);
    assert!(cursor["url"].as_str().unwrap().ends_with("page=21"));
    reopened.close().await.unwrap();
    drop(engine);
    drop(reopened);
    let cold = store(dir.path()).await;
    let engine = runtime(cold.clone(), vault.clone(), &fixture, clock.clone());
    assert_scope(
        cold.scope_state(&account.id, &scope)
            .await
            .unwrap()
            .unwrap(),
        &capped,
    );
    let calls = fixture.count();
    let loads = vault.loads.load(Ordering::SeqCst);
    assert!(!engine.run_next().await);
    assert_eq!(fixture.count(), calls);
    assert_eq!(vault.loads.load(Ordering::SeqCst), loads);
    let mut previous_run = capped.run_id.clone();
    for _ in 0..2 {
        engine
            .hydrate_detail(demand(&account, 67, DetailFacet::Comments))
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
            fixture.comment_calls(),
            21,
            "cap rejects before HTTP under fresh manual leases after cold reopen"
        );
        clock.advance(181);
    }
    assert_eq!(
        cold.detail(query(&account, 67, DetailFacet::Comments))
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
async fn github_comments_fifty_one_saved_rows_use_numeric_local_keysets_without_http_or_vault() {
    let dir = tempfile::tempdir().unwrap();
    let database = store(dir.path()).await;
    let fixture = Fixture::new();
    let vault = Arc::new(Vault::default());
    let engine = runtime(database.clone(), vault.clone(), &fixture, Clock::new());
    let account = connect(&engine, "actor-a").await;
    select(&engine, &account).await;
    fixture.update(|scenario| scenario.mode = Mode::ManyRows);
    hydrate(&engine, &account, 67, DetailFacet::Comments).await;
    assert!(engine.run_next().await);
    let calls = fixture.count();
    let loads = vault.loads.load(Ordering::SeqCst);
    let mut page = query(&account, 67, DetailFacet::Comments);
    page.limit = 50;
    let first = database.detail(page.clone()).await.unwrap();
    assert_eq!(first.entries.len(), 50);
    assert_eq!(
        first
            .entries
            .iter()
            .map(|row| row.provider_id.parse::<u64>().unwrap())
            .collect::<Vec<_>>(),
        (1..=50).collect::<Vec<_>>()
    );
    page.cursor = first.next_cursor.clone();
    assert!(page.cursor.is_some());
    let last = database.detail(page.clone()).await.unwrap();
    assert_eq!(last.entries.len(), 1);
    assert_eq!(last.entries[0].provider_id, "51");
    assert!(last.next_cursor.is_none());
    assert_eq!(last.evidence.facet_revision, first.evidence.facet_revision);
    assert_eq!(fixture.count(), calls);
    assert_eq!(vault.loads.load(Ordering::SeqCst), loads);
    fixture.update(|scenario| scenario.mode = Mode::Edit);
    hydrate(&engine, &account, 67, DetailFacet::Comments).await;
    assert_eq!(
        database.detail(page).await.unwrap_err().code,
        ErrorCode::StaleView,
        "local keyset cannot cross a new Comments facet revision"
    );
}
