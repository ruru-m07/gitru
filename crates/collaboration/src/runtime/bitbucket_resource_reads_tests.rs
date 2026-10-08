//! Synthetic HTTP -> Bitbucket adapter -> Runtime/SQLite resource qualification.
//! Tokens and network listeners are owned by these tests; no provider/keyring is used.
use super::*;
use crate::credentials::CredentialError;
use std::{
    io::{Read, Write},
    sync::{Condvar, Mutex as StdMutex, atomic::AtomicUsize},
};

const ACTOR_A: &str = "00000000-0000-4000-8000-000000000001";
const ACTOR_B: &str = "00000000-0000-4000-8000-000000000002";
const WORKSPACE: &str = "00000000-0000-4000-8000-000000000010";
const REPO_A: &str = "00000000-0000-4000-8000-000000000020";
const REPO_B: &str = "00000000-0000-4000-8000-000000000021";

fn subject(repository: &str, number: u64) -> String {
    format!("bitbucket_cloud:pull:{repository}:{number}")
}
fn repository_id(repository: &str) -> String {
    format!("bitbucket_cloud:repository:{repository}")
}
fn route(repository: &str) -> String {
    format!("/2.0/repositories/%7B%7D/%7B{repository}%7D/pullrequests")
}
fn list_query() -> &'static str {
    "state=OPEN&state=MERGED&state=DECLINED&state=SUPERSEDED&pagelen=50&sort=id"
}

#[derive(Default)]
struct Vault {
    tokens: StdMutex<HashMap<String, SecretToken>>,
    stores: AtomicUsize,
    loads: AtomicUsize,
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
            "owned response not released"
        );
    }
}

#[derive(Clone, Copy, Default)]
enum DetailMode {
    #[default]
    Normal,
    Omitted,
    Empty,
    Oversized,
    Conflicting,
    AbbreviatedHead,
    MissingHead,
    DeletedSourceRepository,
    WrongDestination,
    Denied,
    OlderSource,
}
#[derive(Clone, Copy, Default)]
enum ListMode {
    #[default]
    Normal,
    Loop,
    Cap,
    CrossRepository,
    DuplicateFilter,
}
#[derive(Clone, Copy, Default)]
enum CommentMode {
    #[default]
    Normal,
    Edit,
    Deleted,
    Older,
    Partial,
    Empty,
    Paged,
    Denied,
}
#[derive(Clone, Copy, Default)]
struct Scenario {
    comments: CommentMode,
    detail: DetailMode,
    list: ListMode,
    renamed: bool,
    advanced_head: bool,
}

fn response(status: u16, headers: &str, body: &str) -> String {
    format!(
        "HTTP/1.1 {status} Fixture\r\nContent-Length: {}\r\nConnection: close\r\n{headers}\r\n{body}",
        body.len()
    )
}
fn full_name(repository: &str, renamed: bool) -> String {
    let workspace = if renamed {
        "transferred-workspace"
    } else {
        "workspace"
    };
    let name = if repository == REPO_A {
        "first"
    } else {
        "second"
    };
    format!("{workspace}/{name}")
}
fn condensed(repository: &str, renamed: bool) -> serde_json::Value {
    serde_json::json!({
        "type":"repository", "uuid":format!("{{{repository}}}"),
        "full_name":full_name(repository, renamed),
        "links":{"html":{"href":format!("https://bitbucket.org/{}", full_name(repository, renamed))}}
    })
}
fn repository(repository: &str, renamed: bool) -> serde_json::Value {
    let mut value = condensed(repository, renamed);
    value["scm"] = "git".into();
    value["name"] = if repository == REPO_A {
        "first"
    } else {
        "second"
    }
    .into();
    value["description"] = serde_json::Value::Null;
    value["mainbranch"] = serde_json::Value::Null;
    value["workspace"] = serde_json::json!({
        "type":"workspace", "uuid":format!("{{{WORKSPACE}}}"),
        "slug":if renamed { "transferred-workspace" } else { "workspace" }
    });
    value["links"]["clone"] = serde_json::json!([
        {"name":"https","href":format!("https://bitbucket.org/{}.git",full_name(repository,renamed))},
        {"name":"ssh","href":format!("git@bitbucket.org:{}.git",full_name(repository,renamed))}
    ]);
    value
}
fn pull(
    repository: &str,
    number: u64,
    token: &str,
    scenario: Scenario,
    tick: usize,
) -> serde_json::Value {
    let actor = if token == "actor-b" { ACTOR_B } else { ACTOR_A };
    let state = match number {
        68 => "MERGED",
        69 => "DECLINED",
        70 => "SUPERSEDED",
        _ => "OPEN",
    };
    let body = format!("singleton {repository} for {token}\nMarkdown π 🌱");
    let timestamp = DateTime::parse_from_rfc3339("2026-10-04T00:00:00Z").unwrap()
        + chrono::Duration::seconds(tick as i64);
    let mut value = serde_json::json!({
        "type":"pullrequest", "id":number, "title":format!("PR{number} in {repository}"),
        "state":state, "updated_on":timestamp.to_rfc3339(),
        "author":{"type":"user","uuid":format!("{{{actor}}}"),"nickname":"same-nickname"},
        "description":body, "rendered":{"description":{"raw":body,"html":"<p>never local authority</p>"}},
        "links":{"html":{"href":format!("https://bitbucket.org/{}/pull-requests/{number}",full_name(repository,scenario.renamed))}},
        "source":{"branch":{"name":"feature/local"},"commit":{"hash":if scenario.advanced_head { "c".repeat(40) } else { "a".repeat(40) }},"repository":condensed(repository,scenario.renamed)},
        "destination":{"branch":{"name":"main"},"commit":{"hash":"b".repeat(40)},"repository":condensed(repository,scenario.renamed)},
        "draft":true, "merge_commit":{"hash":"d".repeat(40)},
        "participants":[{"role":"REVIEWER","approved":true}], "task_count":9
    });
    match scenario.detail {
        DetailMode::Normal | DetailMode::Denied => {}
        DetailMode::Omitted => {
            value.as_object_mut().unwrap().remove("rendered");
            // A top-level preview and unrelated field presence cannot validate Body/title/head.
            value["description"] = "top-level preview is not singleton raw authority".into();
            value.as_object_mut().unwrap().remove("title");
            value.as_object_mut().unwrap().remove("source");
        }
        DetailMode::Empty => {
            value["rendered"]["description"]["raw"] = serde_json::Value::Null;
            value["description"] = serde_json::Value::Null;
        }
        DetailMode::Oversized => {
            let oversized = "x".repeat(1_048_577);
            value["rendered"]["description"]["raw"] = oversized.clone().into();
            value["description"] = oversized.into();
        }
        DetailMode::Conflicting => {
            value["description"] = "conflicting top-level raw".into();
            value["title"] = "must never commit this title".into();
        }
        DetailMode::AbbreviatedHead => value["source"]["commit"]["hash"] = "abc1234".into(),
        DetailMode::MissingHead => {
            value["source"].as_object_mut().unwrap().remove("commit");
        }
        DetailMode::DeletedSourceRepository => {
            value["source"]
                .as_object_mut()
                .unwrap()
                .remove("repository");
        }
        DetailMode::WrongDestination => {
            value["destination"]["repository"] = condensed(REPO_B, scenario.renamed);
        }
        DetailMode::OlderSource => {
            value["updated_on"] = "2026-10-04T00:00:00Z".into();
            value["title"] = "older endpoint title must not commit".into();
            value["description"] = "older endpoint Body must not commit".into();
            value["rendered"]["description"]["raw"] = "older endpoint Body must not commit".into();
        }
    }
    value
}

/// Finite owned HTTP server, concurrent only to let a token replacement probe
/// finish while an already dispatched singleton is held.
struct HttpFixture {
    provider: Arc<providers::bitbucket_cloud::BitbucketCloudProvider>,
    calls: Arc<StdMutex<Vec<(String, String)>>>,
    scenario: Arc<StdMutex<Scenario>>,
    held: Arc<Held>,
    stopped: Arc<AtomicBool>,
    task: Option<std::thread::JoinHandle<()>>,
}
impl HttpFixture {
    fn new() -> Self {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let base = reqwest::Url::parse(&format!("http://{}/2.0/", listener.local_addr().unwrap()))
            .unwrap();
        let calls = Arc::new(StdMutex::new(Vec::new()));
        let scenario = Arc::new(StdMutex::new(Scenario::default()));
        let held = Arc::new(Held::default());
        let stopped = Arc::new(AtomicBool::new(false));
        let task = {
            let calls = calls.clone();
            let scenario = scenario.clone();
            let held = held.clone();
            let stopped = stopped.clone();
            let base = base.clone();
            std::thread::spawn(move || {
                let mut connections = vec![];
                while !stopped.load(Ordering::SeqCst) {
                    let mut stream = match listener.accept() {
                        Ok((stream, _)) => stream,
                        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                            std::thread::sleep(Duration::from_millis(1));
                            continue;
                        }
                        Err(error) => panic!("owned fixture accept failed: {error}"),
                    };
                    assert!(connections.len() < 128, "owned HTTP budget exceeded");
                    let calls = calls.clone();
                    let scenario = scenario.clone();
                    let held = held.clone();
                    let base = base.clone();
                    connections.push(std::thread::spawn(move || {
                        stream.set_nonblocking(false).unwrap();
                        stream.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
                        stream.set_write_timeout(Some(Duration::from_secs(2))).unwrap();
                        let mut bytes = vec![];
                        loop {
                            let mut buffer = [0;1024];
                            let count = stream.read(&mut buffer).unwrap();
                            bytes.extend_from_slice(&buffer[..count]);
                            assert!(bytes.len() <=16_384, "owned header budget exceeded");
                            if count == 0 || bytes.windows(4).any(|value| value == b"\r\n\r\n") { break; }
                        }
                        let request = String::from_utf8(bytes).unwrap();
                        let target = request.lines().next().unwrap().split_whitespace().nth(1).unwrap();
                        let token = request.lines().find_map(|line| {
                            let (name,value) = line.split_once(':')?;
                            name.eq_ignore_ascii_case("authorization").then(||value.trim())
                        }).and_then(|value|value.strip_prefix("Bearer ")).expect("synthetic Bearer required");
                        assert!(matches!(token,"actor-a"|"actor-b"|"replacement-a"));
                        assert!(!request.to_ascii_lowercase().contains("if-none-match:"));
                        calls.lock().unwrap().push((target.into(),token.into()));
                        let scenario = *scenario.lock().unwrap();
                        let tick = calls.lock().unwrap().len();
                        let output = if target == "/2.0/user" {
                            response(200,"",&serde_json::json!({
                                "type":"user","uuid":format!("{{{}}}",if token == "actor-b" {ACTOR_B} else {ACTOR_A}),
                                "nickname":"same-nickname","display_name":"Synthetic account","account_status":"active"
                            }).to_string())
                        } else if target == "/2.0/user/workspaces?pagelen=10" {
                            response(200,"",&serde_json::json!({"values":[{
                                "type":"workspace_access","workspace":{"type":"workspace_base","uuid":format!("{{{WORKSPACE}}}"),"slug":"workspace"}
                            }]}).to_string())
                        } else if target == format!("/2.0/repositories/%7B{WORKSPACE}%7D?role=member&pagelen=50") {
                            response(200,"",&serde_json::json!({"values":[repository(REPO_A,scenario.renamed),repository(REPO_B,scenario.renamed)]}).to_string())
                        } else {
                            let repository = [REPO_A,REPO_B].into_iter().find(|repository| target.starts_with(&route(repository))).expect("unknown fixture route");
                            let path = route(repository);
                            if target.starts_with(&format!("{path}/67/comments?")) {
                                let url = base.join(target).unwrap();
                                assert_eq!(url.query_pairs().find(|(k,_)|k=="pagelen").unwrap().1,"50");
                                let page=url.query_pairs().find(|(k,_)|k=="page").map(|(_,v)|v.parse::<u64>().unwrap()).unwrap_or(1);
                                let mode=scenario.comments;
                                let updated=if matches!(mode,CommentMode::Deleted){"2026-10-05T00:00:00Z"}else if matches!(mode,CommentMode::Edit){"2026-10-04T00:00:00Z"}else{"2026-10-02T00:00:00Z"};
                                let mut value=serde_json::json!({"type":"pullrequest_comment","id":1,"created_on":"2026-10-01T00:00:00Z","updated_on":updated,"deleted":matches!(mode,CommentMode::Deleted),"content":{"raw":if matches!(mode,CommentMode::Edit){"edited text"}else{"original text"}},"user":{"uuid":format!("{{{}}}",if token=="actor-b"{ACTOR_B}else{ACTOR_A}),"nickname":token},"pullrequest":{"id":67}});
                                if token == "actor-a" && repository == REPO_A && held.armed.swap(false,Ordering::SeqCst) {
                                    value["content"]["raw"]="obsolete held comment".into();value["updated_on"]="2026-10-05T00:00:00Z".into();held.wait();
                                    response(if held.rate_limited.load(Ordering::SeqCst){429}else{200},"Retry-After: 120\r\n",&serde_json::json!({"values":[value]}).to_string())
                                } else if matches!(mode,CommentMode::Denied) { response(403,"","synthetic comment grant denied") }
                                else {
                                    let mut rows=if matches!(mode,CommentMode::Empty|CommentMode::Paged){vec![]}else{vec![value]};
                                    if matches!(mode,CommentMode::Partial){rows[0]["inline"]=serde_json::json!({"path":"not a conversation"});}
                                    let mut body=serde_json::json!({"values":rows});
                                    if matches!(mode,CommentMode::Paged)&&page==1 {body["next"]=format!("{}{path}/67/comments?pagelen=50&page=2",base.origin().ascii_serialization()).into();}
                                    response(200,"",&body.to_string())
                                }
                            } else if target == format!("{path}/67") {
                                if token == "actor-a" && repository == REPO_A && held.armed.swap(false,Ordering::SeqCst) {
                                    let mut old = pull(repository,67,token,Scenario::default(),tick);
                                    old["description"] = "obsolete old-epoch Body".into();
                                    old["rendered"]["description"]["raw"] = "obsolete old-epoch Body".into();
                                    old["title"] = "obsolete old-epoch title".into();
                                    held.wait();
                                    if held.rate_limited.load(Ordering::SeqCst) {
                                        response(429,"Retry-After: 120\r\n","synthetic old quota")
                                    } else {
                                        response(200,"Retry-After: 120\r\n",&old.to_string())
                                    }
                                } else if token == "actor-a" && repository == REPO_A && matches!(scenario.detail,DetailMode::Denied) {
                                    response(403,"","synthetic PR-read grant denied")
                                } else {
                                    response(200,if matches!(scenario.detail,DetailMode::Conflicting|DetailMode::WrongDestination) {"Retry-After: 120\r\n"} else {""},&pull(repository,67,token,scenario,tick).to_string())
                                }
                            } else {
                                let url = base.join(target).unwrap();
                                let pairs:Vec<_> = url.query_pairs().into_owned().collect();
                                let mut states:Vec<_> = pairs.iter().filter(|(name,_)|name == "state").map(|(_,value)|value.as_str()).collect();
                                states.sort_unstable();
                                assert_eq!(states,vec!["DECLINED","MERGED","OPEN","SUPERSEDED"]);
                                assert!(pairs.iter().any(|(name,value)|name == "pagelen" && value == "50"));
                                assert!(pairs.iter().any(|(name,value)|name == "sort" && value == "id"));
                                let cursor = pairs.iter().find(|(name,_)|name == "cursor").map(|(_,value)|value.as_str());
                                let mut value = serde_json::json!({"values":[]});
                                let next = |repository:&str,cursor:&str| format!("{}{path}?{}&cursor={cursor}",base.origin().ascii_serialization(),list_query(),path=route(repository));
                                match scenario.list {
                                    ListMode::Normal => {
                                        let normal = Scenario { detail:DetailMode::Normal,..scenario };
                                        value["values"] = serde_json::json!((67..=70).map(|number|pull(repository,number,token,normal,tick)).collect::<Vec<_>>());
                                    }
                                    ListMode::Loop => {
                                        value["next"] = next(repository,if cursor == Some("B") {"A"} else {"B"}).into();
                                        if cursor.is_none() { value["next"] = next(repository,"A").into(); }
                                    }
                                    ListMode::Cap => {
                                        let page = cursor.map(|value|value.parse::<usize>().unwrap()).unwrap_or(0);
                                        value["next"] = next(repository,&(page+1).to_string()).into();
                                    }
                                    ListMode::CrossRepository => value["next"] = next(REPO_B,"foreign").into(),
                                    ListMode::DuplicateFilter => value["next"] = format!("{}&pagelen=50",next(repository,"invalid")).into(),
                                }
                                response(200,"",&value.to_string())
                            }
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
    fn cursor_calls(&self, cursor: &str) -> usize {
        self.calls
            .lock()
            .unwrap()
            .iter()
            .filter(|(target, _)| target.ends_with(&format!("&cursor={cursor}")))
            .count()
    }
}
impl Drop for HttpFixture {
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

async fn store(dir: &std::path::Path) -> Arc<Store> {
    Arc::new(Store::open(dir.join("synthetic.sqlite")).await.unwrap())
}
fn make_runtime(
    database: Arc<Store>,
    vault: Arc<Vault>,
    fixture: &HttpFixture,
    clock: Arc<Clock>,
) -> CollaborationRuntime {
    let mut runtime = CollaborationRuntime::new(database, vault, fixture.provider.clone());
    runtime.clock = clock;
    runtime
}
fn query(account: &RemoteAccount, repository: &str) -> DetailQuery {
    DetailQuery {
        account_id: account.id.clone(),
        subject_id: subject(repository, 67),
        facet: DetailFacet::Body,
        cursor: None,
        limit: 50,
    }
}
fn demand(account: &RemoteAccount, repository: &str) -> HydrateDetailRequest {
    HydrateDetailRequest {
        account_id: account.id.clone(),
        authorization_epoch: account.authorization_epoch.clone(),
        subject_id: subject(repository, 67),
        facet: DetailFacet::Body,
    }
}
fn items(account: &RemoteAccount, repository: &str) -> ItemQuery {
    ItemQuery {
        account_id: account.id.clone(),
        kind: RemoteItemKind::PullRequest,
        repository_id: Some(repository_id(repository)),
        state: None,
        search: None,
        cursor: None,
        limit: 50,
    }
}
fn refresh(account: &RemoteAccount, repository: &str) -> RefreshRequest {
    RefreshRequest {
        account_id: account.id.clone(),
        repository_id: Some(repository_id(repository)),
        kind: Some(RemoteItemKind::PullRequest),
    }
}
async fn connected(runtime: &CollaborationRuntime, token: &str) -> RemoteAccount {
    let account = runtime.connect_bitbucket_cloud(token.into()).await.unwrap();
    for _ in 0..2 {
        assert!(runtime.run_next().await);
    }
    assert_eq!(
        runtime
            .store
            .repositories(&account.id)
            .await
            .unwrap()
            .repositories
            .len(),
        2
    );
    account
}
async fn select(runtime: &CollaborationRuntime, account: &RemoteAccount, repository: &str) {
    runtime
        .select_repository(&account.id, &repository_id(repository), true)
        .await
        .unwrap();
    assert!(runtime.run_next().await);
}
async fn authored(
    runtime: &CollaborationRuntime,
    account: &RemoteAccount,
    repository: &str,
    text: &str,
) -> LocalDraft {
    runtime
        .save_draft(LocalDraft {
            account_id: account.id.clone(),
            subject_id: subject(repository, 67),
            body: text.into(),
            generation: "0".into(),
        })
        .await
        .unwrap()
}
async fn hydrate(
    runtime: &CollaborationRuntime,
    account: &RemoteAccount,
    repository: &str,
) -> DetailSnapshot {
    runtime
        .hydrate_detail(demand(account, repository))
        .await
        .unwrap();
    assert!(runtime.run_next().await);
    runtime
        .store
        .detail(query(account, repository))
        .await
        .unwrap()
}
fn field(snapshot: &DetailSnapshot, name: MetadataField) -> &MetadataFieldEvidence {
    snapshot
        .metadata
        .as_ref()
        .unwrap()
        .fields
        .iter()
        .find(|field| field.field == name)
        .unwrap()
}
fn assert_scope(actual: StoredScope, expected: &StoredScope) {
    assert_eq!(actual.run_id, expected.run_id);
    assert_eq!(actual.next_cursor, expected.next_cursor);
    assert_eq!(actual.etag, expected.etag);
    assert_eq!(actual.last_modified, expected.last_modified);
    assert_eq!(actual.coverage, expected.coverage);
    assert_eq!(actual.sync, expected.sync);
}

#[tokio::test]
async fn bitbucket_compound_pull_identity_all_states_and_cached_body_survive_rename_and_cold_reopen()
 {
    let dir = tempfile::tempdir().unwrap();
    let database = store(dir.path()).await;
    let fixture = HttpFixture::new();
    let vault = Arc::new(Vault::default());
    let clock = Clock::new();
    let runtime = make_runtime(database.clone(), vault.clone(), &fixture, clock.clone());
    let first = connected(&runtime, "actor-a").await;
    for repository in [REPO_A, REPO_B] {
        select(&runtime, &first, repository).await;
    }
    let other = connected(&runtime, "actor-b").await;
    select(&runtime, &other, REPO_A).await;
    assert_ne!(first.id, other.id);
    assert_eq!(
        first.login, other.login,
        "mutable nickname is not an actor identity"
    );

    let draft_a = authored(&runtime, &first, REPO_A, "first repository draft").await;
    let draft_b = authored(&runtime, &first, REPO_B, "second repository draft").await;
    let draft_other = authored(&runtime, &other, REPO_A, "other actor draft").await;
    for (account, repository) in [(&first, REPO_A), (&first, REPO_B), (&other, REPO_A)] {
        let page = database
            .query_items(items(account, repository))
            .await
            .unwrap();
        assert_eq!(page.items.len(), 4);
        assert_eq!(page.coverage.state, CoverageState::Complete);
        for (number, state, reason) in [
            (67, "open", None),
            (68, "merged", None),
            (69, "closed", Some("declined")),
            (70, "closed", Some("superseded")),
        ] {
            let item = page
                .items
                .iter()
                .find(|item| item.number.as_deref() == Some(&number.to_string()))
                .unwrap();
            assert_eq!(item.id, subject(repository, number));
            assert_eq!(item.provider_id, format!("{repository}:{number}"));
            assert_eq!(
                item.repository_id.as_deref(),
                Some(repository_id(repository).as_str())
            );
            assert_eq!(item.account_id, account.id);
            assert_eq!(item.state, state);
            assert_eq!(item.reason.as_deref(), reason);
            assert_eq!(
                item.is_draft, None,
                "unsupported Bitbucket draft flag is not authority"
            );
        }
        let missing = database.detail(query(account, repository)).await.unwrap();
        assert_eq!(
            missing.body.state,
            DetailValueState::NotLoaded,
            "list previews never hydrate singleton Body"
        );
        assert!(missing.metadata.is_none());
        let saved = hydrate(&runtime, account, repository).await;
        assert_eq!(saved.body.state, DetailValueState::Known);
        assert!(saved.body.text.as_deref().unwrap().contains(repository));
        assert_eq!(saved.evidence.availability, DetailAvailability::Ready);
        assert_eq!(saved.evidence.coverage.state, CoverageState::Complete);
        let metadata = saved.metadata.as_ref().unwrap();
        assert_eq!(metadata.values.state.as_deref(), Some("open"));
        assert_eq!(
            metadata.values.author.as_ref().unwrap().provider_id,
            if account.id == first.id {
                ACTOR_A
            } else {
                ACTOR_B
            }
        );
        assert_eq!(metadata.values.head.as_ref().unwrap().oid, "a".repeat(40));
        assert_eq!(metadata.values.base.as_ref().unwrap().oid, "b".repeat(40));
        for name in [
            MetadataField::Labels,
            MetadataField::Assignees,
            MetadataField::Milestone,
            MetadataField::IsDraft,
            MetadataField::MergedAt,
        ] {
            assert_eq!(
                field(&saved, name).observed_state,
                DetailValueState::Omitted
            );
            assert_eq!(field(&saved, name).saved_state, DetailValueState::Omitted);
        }
    }
    let before_list = database.detail(query(&first, REPO_A)).await.unwrap();
    fixture.update(|scenario| scenario.renamed = true);
    runtime
        .refresh(RefreshRequest {
            account_id: first.id.clone(),
            repository_id: None,
            kind: None,
        })
        .await
        .unwrap();
    for _ in 0..4 {
        assert!(runtime.run_next().await);
    }
    let renamed = database
        .repository(&first.id, &repository_id(REPO_A))
        .await
        .unwrap();
    assert_eq!(renamed.provider_id, REPO_A);
    assert_eq!(renamed.full_name, "transferred-workspace/first");
    assert!(renamed.selected);
    let after_list = database.detail(query(&first, REPO_A)).await.unwrap();
    assert_eq!(after_list.body, before_list.body);
    assert_eq!(after_list.metadata, before_list.metadata);
    assert_eq!(
        after_list.evidence.facet_revision,
        before_list.evidence.facet_revision
    );
    let renamed_body = hydrate(&runtime, &first, REPO_A).await;
    assert_eq!(renamed_body.subject_id, subject(REPO_A, 67));
    assert_eq!(
        renamed_body
            .metadata
            .as_ref()
            .unwrap()
            .values
            .head
            .as_ref()
            .unwrap()
            .repository
            .as_ref()
            .unwrap()
            .provider_id,
        REPO_A
    );
    assert_eq!(
        renamed_body
            .metadata
            .as_ref()
            .unwrap()
            .values
            .web_url
            .as_deref(),
        Some("https://bitbucket.org/transferred-workspace/first/pull-requests/67")
    );

    // Capture after all writes: global revision is part of each actual snapshot.
    let own_a = database.detail(query(&first, REPO_A)).await.unwrap();
    let own_b = database.detail(query(&first, REPO_B)).await.unwrap();
    let theirs = database.detail(query(&other, REPO_A)).await.unwrap();
    let saved_items = database.query_items(items(&first, REPO_A)).await.unwrap();
    let saved_accounts = database.accounts().await.unwrap();
    let calls = fixture.count();
    let loads = vault.loads.load(Ordering::SeqCst);
    database.close().await.unwrap();
    drop(runtime);
    drop(database);
    let reopened = store(dir.path()).await;
    let cold = make_runtime(reopened.clone(), vault.clone(), &fixture, clock);
    assert_eq!(reopened.accounts().await.unwrap(), saved_accounts);
    assert_eq!(
        reopened.query_items(items(&first, REPO_A)).await.unwrap(),
        saved_items
    );
    assert_eq!(reopened.detail(query(&first, REPO_A)).await.unwrap(), own_a);
    assert_eq!(reopened.detail(query(&first, REPO_B)).await.unwrap(), own_b);
    assert_eq!(
        reopened.detail(query(&other, REPO_A)).await.unwrap(),
        theirs
    );
    for draft in [draft_a, draft_b, draft_other] {
        assert_eq!(
            reopened
                .draft(&draft.account_id, &draft.subject_id)
                .await
                .unwrap(),
            Some(draft)
        );
    }
    assert!(
        !cold.run_next().await,
        "saved reads do not schedule remote work"
    );
    assert_eq!(fixture.count(), calls);
    assert_eq!(vault.loads.load(Ordering::SeqCst), loads);
    assert!(
        fixture
            .calls
            .lock()
            .unwrap()
            .iter()
            .filter(|(target, _)| target.contains("/pullrequests"))
            .all(|(target, _)| target.starts_with("/2.0/repositories/%7B%7D/%7B")),
        "immutable UUID route includes the literal empty workspace braces"
    );
}

#[tokio::test]
async fn bitbucket_body_and_independent_metadata_keep_authority_on_omission_oversize_and_conflict()
{
    let dir = tempfile::tempdir().unwrap();
    let database = store(dir.path()).await;
    let fixture = HttpFixture::new();
    let vault = Arc::new(Vault::default());
    let clock = Clock::new();
    let runtime = make_runtime(database.clone(), vault, &fixture, clock.clone());
    let account = connected(&runtime, "actor-a").await;
    select(&runtime, &account, REPO_A).await;
    let draft = authored(
        &runtime,
        &account,
        REPO_A,
        "author text must survive raw omission",
    )
    .await;
    let known = hydrate(&runtime, &account, REPO_A).await;
    let title_evidence = field(&known, MetadataField::Title).clone();
    let head_evidence = field(&known, MetadataField::Head).clone();
    let source = known.evidence.value_source.clone();
    clock.advance(1);
    fixture.update(|scenario| scenario.detail = DetailMode::Omitted);
    let omitted = hydrate(&runtime, &account, REPO_A).await;
    assert_eq!(omitted.body, known.body);
    assert_eq!(omitted.evidence.observed_state, DetailValueState::Omitted);
    assert_eq!(
        omitted.evidence.value_source, source,
        "a top-level preview cannot revalidate canonical raw"
    );
    assert_eq!(
        field(&omitted, MetadataField::Title).validated_at,
        title_evidence.validated_at
    );
    assert_eq!(
        field(&omitted, MetadataField::Title).source,
        title_evidence.source
    );
    assert_eq!(
        field(&omitted, MetadataField::Head).validated_at,
        head_evidence.validated_at
    );
    assert_eq!(
        field(&omitted, MetadataField::Head).source,
        head_evidence.source
    );
    assert_eq!(
        field(&omitted, MetadataField::Title).observed_state,
        DetailValueState::Omitted
    );
    assert_ne!(
        field(&omitted, MetadataField::UpdatedAt).validated_at,
        field(&known, MetadataField::UpdatedAt).validated_at,
        "an independently present field still advances"
    );
    clock.advance(1);
    fixture.update(|scenario| scenario.detail = DetailMode::Oversized);
    let oversized = hydrate(&runtime, &account, REPO_A).await;
    assert_eq!(oversized.body, known.body);
    assert_eq!(
        oversized.evidence.observed_state,
        DetailValueState::Oversized
    );
    assert_eq!(oversized.evidence.value_source, source);
    clock.advance(1);
    fixture.update(|scenario| scenario.detail = DetailMode::Empty);
    let empty = hydrate(&runtime, &account, REPO_A).await;
    assert_eq!(
        empty.body,
        DetailValue {
            state: DetailValueState::Known,
            text: None
        }
    );
    assert_eq!(empty.evidence.observed_state, DetailValueState::Known);
    assert_eq!(empty.evidence.saved_empty, Some(true));
    assert_ne!(empty.evidence.value_source, source);
    assert_eq!(
        database
            .draft(&account.id, &draft.subject_id)
            .await
            .unwrap(),
        Some(draft.clone())
    );

    fixture.update(|scenario| scenario.detail = DetailMode::OlderSource);
    clock.advance(1);
    let older = hydrate(&runtime, &account, REPO_A).await;
    assert_eq!(older.body, empty.body);
    assert_eq!(
        older.metadata, empty.metadata,
        "an older comparable singleton source cannot replace any independent field"
    );
    assert_eq!(older.evidence.value_source, empty.evidence.value_source);
    assert_eq!(older.evidence.facet_revision, empty.evidence.facet_revision);

    fixture.update(|scenario| scenario.detail = DetailMode::Conflicting);
    clock.advance(1);
    let rejected = hydrate(&runtime, &account, REPO_A).await;
    assert_eq!(rejected.body, empty.body);
    assert_eq!(
        rejected.metadata, empty.metadata,
        "conflicting raw must reject all page fields atomically"
    );
    assert_eq!(rejected.evidence.value_source, empty.evidence.value_source);
    assert_eq!(
        rejected.evidence.facet_revision,
        empty.evidence.facet_revision
    );
    let cooldown = database
        .scope_state(&account.id, "provider:rest")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        cooldown.sync.state,
        SyncState::RateLimited,
        "mapping failure keeps actual response quota"
    );
    assert!(cooldown.sync.next_retry_at.is_some());
    assert_eq!(
        database
            .draft(&account.id, &draft.subject_id)
            .await
            .unwrap(),
        Some(draft)
    );
}

#[tokio::test]
async fn bitbucket_abbreviated_or_missing_head_cannot_validate_base_and_deleted_fork_keeps_refs() {
    for mode in [
        DetailMode::AbbreviatedHead,
        DetailMode::MissingHead,
        DetailMode::DeletedSourceRepository,
    ] {
        let dir = tempfile::tempdir().unwrap();
        let database = store(dir.path()).await;
        let fixture = HttpFixture::new();
        let runtime = make_runtime(
            database.clone(),
            Arc::new(Vault::default()),
            &fixture,
            Clock::new(),
        );
        let account = connected(&runtime, "actor-a").await;
        select(&runtime, &account, REPO_A).await;
        fixture.update(|scenario| scenario.detail = mode);
        let detail = hydrate(&runtime, &account, REPO_A).await;
        assert_eq!(detail.body.state, DetailValueState::Known);
        let values = &detail.metadata.as_ref().unwrap().values;
        if matches!(mode, DetailMode::DeletedSourceRepository) {
            assert_eq!(values.head.as_ref().unwrap().oid, "a".repeat(40));
            assert!(values.head.as_ref().unwrap().repository.is_none());
            assert_eq!(values.base.as_ref().unwrap().oid, "b".repeat(40));
        } else {
            assert!(values.head.is_none());
            assert!(
                values.base.is_none(),
                "a full destination hash is insufficient without actual full source Head"
            );
            assert_eq!(
                field(&detail, MetadataField::Head).observed_state,
                DetailValueState::Omitted
            );
            assert_eq!(
                field(&detail, MetadataField::Base).observed_state,
                DetailValueState::Omitted
            );
        }
    }
}

#[tokio::test]
async fn bitbucket_pr_scope_denial_preserves_repository_grant_other_resource_actor_and_private_drafts()
 {
    let dir = tempfile::tempdir().unwrap();
    let database = store(dir.path()).await;
    let fixture = HttpFixture::new();
    let runtime = make_runtime(
        database.clone(),
        Arc::new(Vault::default()),
        &fixture,
        Clock::new(),
    );
    let first = connected(&runtime, "actor-a").await;
    for repository in [REPO_A, REPO_B] {
        select(&runtime, &first, repository).await;
    }
    let other = connected(&runtime, "actor-b").await;
    select(&runtime, &other, REPO_A).await;
    let draft = authored(&runtime, &first, REPO_A, "private under PR scope loss").await;
    let theirs = authored(&runtime, &other, REPO_A, "other account private text").await;
    let first_body = hydrate(&runtime, &first, REPO_A).await;
    let sibling = hydrate(&runtime, &first, REPO_B).await;
    let other_body = hydrate(&runtime, &other, REPO_A).await;
    let repositories = database.repositories(&first.id).await.unwrap().repositories;
    fixture.update(|scenario| scenario.detail = DetailMode::Denied);
    let denied = hydrate(&runtime, &first, REPO_A).await;
    assert_eq!(first_body.evidence.availability, DetailAvailability::Ready);
    assert_eq!(
        denied.evidence.availability,
        DetailAvailability::Unavailable
    );
    assert!(denied.body.text.is_none());
    assert!(denied.metadata.is_none());
    assert_eq!(
        database.account(&first.id).await.unwrap(),
        first,
        "PR-read denial must not revoke the verified repository account"
    );
    assert_eq!(
        database.repositories(&first.id).await.unwrap().repositories,
        repositories
    );
    assert_eq!(
        database
            .query_items(items(&first, REPO_A))
            .await
            .unwrap()
            .items
            .len(),
        4
    );
    let sibling_after = database.detail(query(&first, REPO_B)).await.unwrap();
    assert_eq!(sibling_after.body, sibling.body);
    assert_eq!(sibling_after.metadata, sibling.metadata);
    assert_eq!(sibling_after.evidence, sibling.evidence);
    let other_after = database.detail(query(&other, REPO_A)).await.unwrap();
    assert_eq!(other_after.body, other_body.body);
    assert_eq!(other_after.metadata, other_body.metadata);
    assert_eq!(other_after.evidence, other_body.evidence);
    for saved in [draft, theirs] {
        assert_eq!(
            database
                .draft(&saved.account_id, &saved.subject_id)
                .await
                .unwrap(),
            Some(saved)
        );
    }
    let calls = fixture.count();
    let error = runtime
        .hydrate_detail(HydrateDetailRequest {
            facet: DetailFacet::Reviews,
            ..demand(&other, REPO_A)
        })
        .await
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::Unsupported);
    assert!(!runtime.run_next().await);
    assert_eq!(
        fixture.count(),
        calls,
        "unsupported review facets never trigger an invented request"
    );
}

#[tokio::test]
async fn bitbucket_held_old_epoch_body_success_and_quota_error_cannot_mutate_replacement_or_other_actor()
 {
    for rate_limited in [false, true] {
        let dir = tempfile::tempdir().unwrap();
        let database = store(dir.path()).await;
        let fixture = HttpFixture::new();
        let runtime = make_runtime(
            database.clone(),
            Arc::new(Vault::default()),
            &fixture,
            Clock::new(),
        );
        let first = connected(&runtime, "actor-a").await;
        select(&runtime, &first, REPO_A).await;
        let draft = authored(&runtime, &first, REPO_A, "old actor private draft").await;
        hydrate(&runtime, &first, REPO_A).await;
        let other = connected(&runtime, "actor-b").await;
        select(&runtime, &other, REPO_A).await;
        let theirs = authored(&runtime, &other, REPO_A, "other actor private draft").await;
        hydrate(&runtime, &other, REPO_A).await;
        fixture
            .held
            .rate_limited
            .store(rate_limited, Ordering::SeqCst);
        fixture.held.armed.store(true, Ordering::SeqCst);
        runtime
            .hydrate_detail(demand(&first, REPO_A))
            .await
            .unwrap();
        let worker = runtime.clone();
        let pending = tokio::spawn(async move { worker.run_next().await });
        tokio::time::timeout(Duration::from_secs(2), fixture.held.entered.notified())
            .await
            .unwrap();
        let replacement = runtime
            .connect_bitbucket_cloud("replacement-a".into())
            .await
            .unwrap();
        assert_eq!(replacement.id, first.id);
        assert_eq!(replacement.authorization_epoch, "2");
        let accounts = database.accounts().await.unwrap();
        let current = database.detail(query(&replacement, REPO_A)).await.unwrap();
        let other_saved = database.detail(query(&other, REPO_A)).await.unwrap();
        let own_items = database
            .query_items(items(&replacement, REPO_A))
            .await
            .unwrap();
        let other_items = database.query_items(items(&other, REPO_A)).await.unwrap();
        let revision = database.revision().await.unwrap();
        fixture.held.release();
        assert!(
            tokio::time::timeout(Duration::from_secs(2), pending)
                .await
                .unwrap()
                .unwrap()
        );
        assert_eq!(
            database.revision().await.unwrap(),
            revision,
            "an obsolete grant publishes neither data nor failure/quota revisions"
        );
        assert_eq!(database.accounts().await.unwrap(), accounts);
        assert_eq!(
            database.detail(query(&replacement, REPO_A)).await.unwrap(),
            current
        );
        assert_eq!(
            database.detail(query(&other, REPO_A)).await.unwrap(),
            other_saved
        );
        assert_eq!(
            database
                .query_items(items(&replacement, REPO_A))
                .await
                .unwrap(),
            own_items
        );
        assert_eq!(
            database.query_items(items(&other, REPO_A)).await.unwrap(),
            other_items
        );
        for account in [&replacement, &other] {
            assert!(
                database
                    .scope_state(&account.id, "provider:rest")
                    .await
                    .unwrap()
                    .is_none()
            );
        }
        for saved in [draft, theirs] {
            assert_eq!(
                database
                    .draft(&saved.account_id, &saved.subject_id)
                    .await
                    .unwrap(),
                Some(saved)
            );
        }
        assert_eq!(
            runtime
                .hydrate_detail(demand(&first, REPO_A))
                .await
                .unwrap_err()
                .code,
            ErrorCode::StaleView
        );
    }
}

#[tokio::test]
async fn bitbucket_wrong_destination_rejects_body_metadata_atomically_but_keeps_current_actor_quota()
 {
    let dir = tempfile::tempdir().unwrap();
    let database = store(dir.path()).await;
    let fixture = HttpFixture::new();
    let runtime = make_runtime(
        database.clone(),
        Arc::new(Vault::default()),
        &fixture,
        Clock::new(),
    );
    let account = connected(&runtime, "actor-a").await;
    for repository in [REPO_A, REPO_B] {
        select(&runtime, &account, repository).await;
    }
    let saved_a = hydrate(&runtime, &account, REPO_A).await;
    let saved_b = hydrate(&runtime, &account, REPO_B).await;
    let draft = authored(
        &runtime,
        &account,
        REPO_A,
        "same local PR number is not route authority",
    )
    .await;
    fixture.update(|scenario| scenario.detail = DetailMode::WrongDestination);
    let failed = hydrate(&runtime, &account, REPO_A).await;
    assert_eq!(failed.body, saved_a.body);
    assert_eq!(failed.metadata, saved_a.metadata);
    assert_eq!(failed.evidence.value_source, saved_a.evidence.value_source);
    assert_eq!(
        failed.evidence.facet_revision,
        saved_a.evidence.facet_revision
    );
    let unchanged = database.detail(query(&account, REPO_B)).await.unwrap();
    assert_eq!(unchanged.body, saved_b.body);
    assert_eq!(unchanged.metadata, saved_b.metadata);
    assert_eq!(unchanged.evidence, saved_b.evidence);
    assert_eq!(database.account(&account.id).await.unwrap(), account);
    assert_eq!(
        database
            .scope_state(&account.id, "provider:rest")
            .await
            .unwrap()
            .unwrap()
            .sync
            .state,
        SyncState::RateLimited
    );
    assert_eq!(
        database
            .draft(&account.id, &draft.subject_id)
            .await
            .unwrap(),
        Some(draft)
    );
}

#[tokio::test]
async fn bitbucket_opaque_pull_loop_keeps_accepted_cursor_cache_and_backoff_across_cold_reopen() {
    let dir = tempfile::tempdir().unwrap();
    let database = store(dir.path()).await;
    let fixture = HttpFixture::new();
    let vault = Arc::new(Vault::default());
    let clock = Clock::new();
    let runtime = make_runtime(database.clone(), vault.clone(), &fixture, clock.clone());
    let account = connected(&runtime, "actor-a").await;
    select(&runtime, &account, REPO_A).await;
    let complete = database.query_items(items(&account, REPO_A)).await.unwrap();
    let body = hydrate(&runtime, &account, REPO_A).await;
    let draft = authored(
        &runtime,
        &account,
        REPO_A,
        "private under a partial empty traversal",
    )
    .await;
    let scope = format!("repo:{}:pull_request", repository_id(REPO_A));
    fixture.update(|scenario| scenario.list = ListMode::Loop);
    runtime.refresh(refresh(&account, REPO_A)).await.unwrap();
    assert!(runtime.run_next().await); // Initial empty page proposes A.
    let accepted = database
        .scope_state(&account.id, &scope)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(accepted.coverage.state, CoverageState::Partial);
    assert_eq!(
        accepted.coverage.validated_at,
        complete.coverage.validated_at
    );
    assert_eq!(
        database
            .query_items(items(&account, REPO_A))
            .await
            .unwrap()
            .items,
        complete.items
    );
    database.close().await.unwrap();
    drop(runtime);
    drop(database);
    let reopened = store(dir.path()).await;
    let runtime = make_runtime(reopened.clone(), vault.clone(), &fixture, clock.clone());
    assert_scope(
        reopened
            .scope_state(&account.id, &scope)
            .await
            .unwrap()
            .unwrap(),
        &accepted,
    );
    let calls = fixture.count();
    let loads = vault.loads.load(Ordering::SeqCst);
    assert!(!runtime.run_next().await);
    assert_eq!(
        reopened.detail(query(&account, REPO_A)).await.unwrap().body,
        body.body
    );
    assert_eq!(
        reopened
            .draft(&account.id, &draft.subject_id)
            .await
            .unwrap(),
        Some(draft.clone())
    );
    assert_eq!(fixture.count(), calls);
    assert_eq!(vault.loads.load(Ordering::SeqCst), loads);

    runtime.refresh(refresh(&account, REPO_A)).await.unwrap();
    assert!(runtime.run_next().await); // A accepts, proposes B.
    let before = reopened
        .scope_state(&account.id, &scope)
        .await
        .unwrap()
        .unwrap();
    assert!(runtime.run_next().await); // B proposes old A: reject B atomically.
    let failed = reopened
        .scope_state(&account.id, &scope)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(failed.run_id, accepted.run_id);
    assert_eq!(failed.next_cursor, before.next_cursor);
    assert_eq!(failed.coverage, before.coverage);
    assert_eq!(failed.coverage.state, CoverageState::Partial);
    assert_eq!(failed.sync.state, SyncState::Error);
    assert!(failed.sync.next_retry_at.is_some());
    assert_eq!(fixture.cursor_calls("A"), 1);
    assert_eq!(fixture.cursor_calls("B"), 1);
    assert_eq!(
        reopened
            .query_items(items(&account, REPO_A))
            .await
            .unwrap()
            .items,
        complete.items
    );
    reopened.close().await.unwrap();
    drop(runtime);
    drop(reopened);
    let cold = store(dir.path()).await;
    let runtime = make_runtime(cold.clone(), vault.clone(), &fixture, clock.clone());
    assert_scope(
        cold.scope_state(&account.id, &scope)
            .await
            .unwrap()
            .unwrap(),
        &failed,
    );
    let calls = fixture.count();
    let loads = vault.loads.load(Ordering::SeqCst);
    runtime.refresh(refresh(&account, REPO_A)).await.unwrap();
    assert!(
        !runtime.run_next().await,
        "cold admission restores the persisted retry barrier before a job can be picked"
    );
    assert_eq!(
        fixture.count(),
        calls,
        "stored backoff makes zero HTTP requests"
    );
    assert_eq!(
        vault.loads.load(Ordering::SeqCst),
        loads,
        "stored backoff does not open the vault"
    );
    clock.advance(61);
    assert!(
        runtime.run_next().await,
        "the rejected current B may retry after finite backoff"
    );
    assert_eq!(fixture.cursor_calls("B"), 2);
    assert_eq!(
        fixture.cursor_calls("A"),
        1,
        "history never follows the proposed loop back to A"
    );
    assert_eq!(
        cold.query_items(items(&account, REPO_A))
            .await
            .unwrap()
            .items,
        complete.items
    );
    assert_eq!(
        cold.detail(query(&account, REPO_A)).await.unwrap().body,
        body.body
    );
    assert_eq!(
        cold.draft(&account.id, &draft.subject_id).await.unwrap(),
        Some(draft)
    );
}

#[tokio::test]
async fn bitbucket_pull_twenty_page_cap_persists_across_scheduler_yield_and_two_cold_reopens() {
    let dir = tempfile::tempdir().unwrap();
    let database = store(dir.path()).await;
    let fixture = HttpFixture::new();
    let vault = Arc::new(Vault::default());
    let clock = Clock::new();
    let runtime = make_runtime(database.clone(), vault.clone(), &fixture, clock.clone());
    let account = connected(&runtime, "actor-a").await;
    select(&runtime, &account, REPO_A).await;
    let complete = database.query_items(items(&account, REPO_A)).await.unwrap();
    let draft = authored(&runtime, &account, REPO_A, "draft under durable cap").await;
    let scope = format!("repo:{}:pull_request", repository_id(REPO_A));
    fixture.update(|scenario| scenario.list = ListMode::Cap);
    let baseline = fixture.count();
    runtime.refresh(refresh(&account, REPO_A)).await.unwrap();
    for _ in 0..10 {
        assert!(runtime.run_next().await);
    }
    assert!(
        !runtime.run_next().await,
        "one admitted refresh yields after ten pages"
    );
    assert_eq!(fixture.count(), baseline + 10);
    let yielded = database
        .scope_state(&account.id, &scope)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(yielded.coverage.state, CoverageState::Partial);
    assert_eq!(
        yielded.coverage.validated_at,
        complete.coverage.validated_at
    );
    assert_eq!(
        database
            .query_items(items(&account, REPO_A))
            .await
            .unwrap()
            .items,
        complete.items
    );
    database.close().await.unwrap();
    drop(runtime);
    drop(database);
    let reopened = store(dir.path()).await;
    let runtime = make_runtime(reopened.clone(), vault.clone(), &fixture, clock.clone());
    assert_scope(
        reopened
            .scope_state(&account.id, &scope)
            .await
            .unwrap()
            .unwrap(),
        &yielded,
    );
    let loads = vault.loads.load(Ordering::SeqCst);
    assert_eq!(
        reopened
            .draft(&account.id, &draft.subject_id)
            .await
            .unwrap(),
        Some(draft.clone())
    );
    assert!(!runtime.run_next().await);
    assert_eq!(vault.loads.load(Ordering::SeqCst), loads);
    runtime.refresh(refresh(&account, REPO_A)).await.unwrap();
    for _ in 0..10 {
        assert!(runtime.run_next().await);
    }
    assert_eq!(
        fixture.count(),
        baseline + 20,
        "accepted-page budget survives a cold scheduler reset"
    );
    let capped = reopened
        .scope_state(&account.id, &scope)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(capped.run_id, yielded.run_id);
    assert_eq!(capped.coverage.state, CoverageState::Partial);
    assert_eq!(capped.coverage.validated_at, complete.coverage.validated_at);
    assert!(capped.coverage.remote_has_more);
    assert!(capped.next_cursor.as_ref().unwrap().len() <= 4096);
    assert_eq!(
        reopened
            .query_items(items(&account, REPO_A))
            .await
            .unwrap()
            .items,
        complete.items,
        "partial empty pages cannot infer absence of prior authorized PRs"
    );
    reopened.close().await.unwrap();
    drop(runtime);
    drop(reopened);
    let cold = store(dir.path()).await;
    let runtime = make_runtime(cold.clone(), vault.clone(), &fixture, clock.clone());
    assert_scope(
        cold.scope_state(&account.id, &scope)
            .await
            .unwrap()
            .unwrap(),
        &capped,
    );
    for _ in 0..2 {
        runtime.refresh(refresh(&account, REPO_A)).await.unwrap();
        assert!(
            runtime.run_next().await,
            "cap is a bounded error receipt, never a page-one restart"
        );
        let failed = cold
            .scope_state(&account.id, &scope)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(failed.next_cursor, capped.next_cursor);
        assert_eq!(failed.run_id, capped.run_id);
        assert_eq!(failed.coverage, capped.coverage);
        assert_eq!(failed.sync.state, SyncState::Error);
        assert_eq!(
            fixture.count(),
            baseline + 20,
            "a capped cursor rejects before HTTP even after reopen"
        );
        clock.advance(181);
    }
    assert_eq!(
        cold.query_items(items(&account, REPO_A))
            .await
            .unwrap()
            .items,
        complete.items
    );
    assert_eq!(
        cold.draft(&account.id, &draft.subject_id).await.unwrap(),
        Some(draft)
    );
}

#[tokio::test]
async fn bitbucket_hostile_pull_continuations_cannot_cross_repository_or_duplicate_fixed_filters() {
    for mode in [ListMode::CrossRepository, ListMode::DuplicateFilter] {
        let dir = tempfile::tempdir().unwrap();
        let database = store(dir.path()).await;
        let fixture = HttpFixture::new();
        let runtime = make_runtime(
            database.clone(),
            Arc::new(Vault::default()),
            &fixture,
            Clock::new(),
        );
        let account = connected(&runtime, "actor-a").await;
        for repository in [REPO_A, REPO_B] {
            select(&runtime, &account, repository).await;
        }
        let own = database.query_items(items(&account, REPO_A)).await.unwrap();
        let sibling = database.query_items(items(&account, REPO_B)).await.unwrap();
        let draft = authored(
            &runtime,
            &account,
            REPO_A,
            "private under untrusted next link",
        )
        .await;
        fixture.update(|scenario| scenario.list = mode);
        let calls = fixture.count();
        runtime.refresh(refresh(&account, REPO_A)).await.unwrap();
        assert!(runtime.run_next().await);
        assert_eq!(
            fixture.count(),
            calls + 1,
            "only the trusted current repository route was requested"
        );
        assert_eq!(
            database
                .query_items(items(&account, REPO_A))
                .await
                .unwrap()
                .items,
            own.items
        );
        assert_eq!(
            database
                .query_items(items(&account, REPO_B))
                .await
                .unwrap()
                .items,
            sibling.items
        );
        assert_eq!(
            database
                .draft(&account.id, &draft.subject_id)
                .await
                .unwrap(),
            Some(draft)
        );
        let scope = database
            .scope_state(
                &account.id,
                &format!("repo:{}:pull_request", repository_id(REPO_A)),
            )
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            scope.coverage, own.coverage,
            "a failed new first page preserves historical coverage and its old validation clock"
        );
        assert_eq!(scope.sync.state, SyncState::Error);
        assert!(
            scope.next_cursor.is_none(),
            "no rejected next-link checkpoint is published"
        );
    }
}

#[tokio::test]
async fn bitbucket_actual_http_summary_head_change_vetoes_a_held_runtime_singleton_binding() {
    let dir = tempfile::tempdir().unwrap();
    let database = store(dir.path()).await;
    let fixture = HttpFixture::new();
    let runtime = make_runtime(
        database.clone(),
        Arc::new(Vault::default()),
        &fixture,
        Clock::new(),
    );
    let account = connected(&runtime, "actor-a").await;
    select(&runtime, &account, REPO_A).await;
    let original = hydrate(&runtime, &account, REPO_A).await;
    let draft = authored(
        &runtime,
        &account,
        REPO_A,
        "private text while old head read is held",
    )
    .await;
    fixture.held.armed.store(true, Ordering::SeqCst);
    runtime
        .hydrate_detail(demand(&account, REPO_A))
        .await
        .unwrap();
    let worker = runtime.clone();
    let pending = tokio::spawn(async move { worker.run_next().await });
    tokio::time::timeout(Duration::from_secs(2), fixture.held.entered.notified())
        .await
        .unwrap();
    fixture.update(|scenario| scenario.advanced_head = true);
    let repository = database
        .repository(&account.id, &repository_id(REPO_A))
        .await
        .unwrap();
    // Runtime dispatch is serialized. This is an independent actual HTTP
    // producer -> the same adapter -> a native Store feed transaction, not a
    // claim that two provider jobs run simultaneously in one Runtime.
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
            observed_at: runtime.now_string(),
        })
        .await
        .unwrap();
    assert_eq!(
        database
            .item(&account.id, &subject(REPO_A, 67))
            .await
            .unwrap()
            .item
            .unwrap()
            .head_oid,
        Some("c".repeat(40))
    );
    let invalidated = database.detail(query(&account, REPO_A)).await.unwrap();
    fixture.held.release();
    assert!(
        tokio::time::timeout(Duration::from_secs(2), pending)
            .await
            .unwrap()
            .unwrap()
    );
    let after = database.detail(query(&account, REPO_A)).await.unwrap();
    assert_eq!(
        after.body, original.body,
        "old HTTP raw cannot cross the captured current-head binding"
    );
    assert_eq!(after.metadata, invalidated.metadata);
    assert_eq!(after.evidence.value_source, original.evidence.value_source);
    assert_eq!(
        after.evidence.facet_revision,
        invalidated.evidence.facet_revision
    );
    assert_ne!(after.body.text.as_deref(), Some("obsolete old-epoch Body"));
    assert_eq!(
        database
            .draft(&account.id, &draft.subject_id)
            .await
            .unwrap(),
        Some(draft)
    );
}

fn comments_demand(account: &RemoteAccount, repository: &str) -> HydrateDetailRequest {
    HydrateDetailRequest {
        facet: DetailFacet::Comments,
        ..demand(account, repository)
    }
}
fn comments_query(account: &RemoteAccount, repository: &str) -> DetailQuery {
    DetailQuery {
        facet: DetailFacet::Comments,
        ..query(account, repository)
    }
}
async fn hydrate_comments(
    runtime: &CollaborationRuntime,
    account: &RemoteAccount,
    repository: &str,
) -> DetailSnapshot {
    runtime
        .hydrate_detail(comments_demand(account, repository))
        .await
        .unwrap();
    assert!(runtime.run_next().await);
    runtime
        .store
        .detail(comments_query(account, repository))
        .await
        .unwrap()
}
#[tokio::test]
async fn bitbucket_comments_own_clock_tombstone_partial_absence_and_offline_reopen() {
    let dir = tempfile::tempdir().unwrap();
    let database = store(dir.path()).await;
    let fixture = HttpFixture::new();
    let vault = Arc::new(Vault::default());
    let clock = Clock::new();
    let runtime = make_runtime(database.clone(), vault.clone(), &fixture, clock.clone());
    let account = connected(&runtime, "actor-a").await;
    select(&runtime, &account, REPO_A).await;
    let draft = authored(&runtime, &account, REPO_A, "private draft").await;
    let initial = hydrate_comments(&runtime, &account, REPO_A).await;
    assert_eq!(
        initial.entries[0].body.text.as_deref(),
        Some("original text")
    );
    fixture.update(|s| s.comments = CommentMode::Edit);
    let edited = hydrate_comments(&runtime, &account, REPO_A).await;
    assert_eq!(edited.entries[0].body.text.as_deref(), Some("edited text"));
    fixture.update(|s| s.comments = CommentMode::Older);
    let older = hydrate_comments(&runtime, &account, REPO_A).await;
    assert_eq!(older.entries[0].body, edited.entries[0].body);
    fixture.update(|s| s.comments = CommentMode::Deleted);
    let deleted = hydrate_comments(&runtime, &account, REPO_A).await;
    assert_eq!(deleted.entries[0].state.as_deref(), Some("deleted"));
    assert!(deleted.entries[0].body.text.is_none() && deleted.entries[0].author.is_none());
    fixture.update(|s| s.comments = CommentMode::Older);
    let older = hydrate_comments(&runtime, &account, REPO_A).await;
    assert_eq!(older.entries[0].body, deleted.entries[0].body);
    assert_eq!(older.entries[0].state, deleted.entries[0].state);
    fixture.update(|s| s.comments = CommentMode::Partial);
    let partial = hydrate_comments(&runtime, &account, REPO_A).await;
    assert_eq!(partial.entries.len(), 1);
    assert_eq!(partial.evidence.coverage.state, CoverageState::Partial);
    runtime.shutdown().await.unwrap();
    drop(runtime);
    database.close().await.unwrap();
    drop(database);
    let database = store(dir.path()).await;
    let runtime = make_runtime(database.clone(), vault, &fixture, clock);
    let before = fixture.count();
    let saved = database
        .detail(comments_query(&account, REPO_A))
        .await
        .unwrap();
    assert_eq!(saved.entries[0].state.as_deref(), Some("deleted"));
    assert_eq!(fixture.count(), before);
    assert_eq!(
        database
            .draft(&account.id, &draft.subject_id)
            .await
            .unwrap(),
        Some(draft)
    );
    fixture.update(|s| s.comments = CommentMode::Empty);
    let empty = hydrate_comments(&runtime, &account, REPO_A).await;
    assert!(empty.entries.is_empty());
}
#[tokio::test]
async fn bitbucket_comments_continuation_cold_reopen_and_scope_denial_keep_authored_data() {
    let dir = tempfile::tempdir().unwrap();
    let database = store(dir.path()).await;
    let fixture = HttpFixture::new();
    let vault = Arc::new(Vault::default());
    let clock = Clock::new();
    let runtime = make_runtime(database.clone(), vault.clone(), &fixture, clock.clone());
    let account = connected(&runtime, "actor-a").await;
    select(&runtime, &account, REPO_A).await;
    let draft = authored(&runtime, &account, REPO_A, "private draft").await;
    hydrate_comments(&runtime, &account, REPO_A).await;
    fixture.update(|s| s.comments = CommentMode::Paged);
    let first = hydrate_comments(&runtime, &account, REPO_A).await;
    assert_eq!(first.entries.len(), 1);
    assert!(first.evidence.coverage.remote_has_more);
    runtime.shutdown().await.unwrap();
    drop(runtime);
    database.close().await.unwrap();
    drop(database);
    let database = store(dir.path()).await;
    let runtime = make_runtime(database.clone(), vault, &fixture, clock);
    let terminal = hydrate_comments(&runtime, &account, REPO_A).await;
    assert!(!terminal.evidence.coverage.remote_has_more);
    assert_eq!(terminal.entries.len(), 1);
    assert_eq!(terminal.evidence.coverage.state, CoverageState::Partial);
    let calls = fixture.calls.lock().unwrap().clone();
    assert!(
        calls
            .last()
            .unwrap()
            .0
            .ends_with("/comments?pagelen=50&page=2")
    );
    fixture.update(|s| s.comments = CommentMode::Denied);
    let denied = hydrate_comments(&runtime, &account, REPO_A).await;
    assert!(denied.entries.is_empty());
    assert_eq!(
        database.account(&account.id).await.unwrap().state,
        AccountState::Active
    );
    assert_eq!(
        database
            .draft(&account.id, &draft.subject_id)
            .await
            .unwrap(),
        Some(draft)
    );
}
#[tokio::test]
async fn bitbucket_comments_held_old_epoch_success_and_quota_error_cannot_mutate_replacement_or_other_actor()
 {
    for rate_limited in [false, true] {
        let dir = tempfile::tempdir().unwrap();
        let database = store(dir.path()).await;
        let fixture = HttpFixture::new();
        let runtime = make_runtime(
            database.clone(),
            Arc::new(Vault::default()),
            &fixture,
            Clock::new(),
        );
        let first = connected(&runtime, "actor-a").await;
        select(&runtime, &first, REPO_A).await;
        let draft = authored(&runtime, &first, REPO_A, "old actor private draft").await;
        hydrate_comments(&runtime, &first, REPO_A).await;
        let other = connected(&runtime, "actor-b").await;
        select(&runtime, &other, REPO_A).await;
        let theirs = authored(&runtime, &other, REPO_A, "other actor private draft").await;
        hydrate_comments(&runtime, &other, REPO_A).await;
        fixture
            .held
            .rate_limited
            .store(rate_limited, Ordering::SeqCst);
        fixture.held.armed.store(true, Ordering::SeqCst);
        runtime
            .hydrate_detail(comments_demand(&first, REPO_A))
            .await
            .unwrap();
        let worker = runtime.clone();
        let pending = tokio::spawn(async move { worker.run_next().await });
        tokio::time::timeout(Duration::from_secs(2), fixture.held.entered.notified())
            .await
            .unwrap();
        let replacement = runtime
            .connect_bitbucket_cloud("replacement-a".into())
            .await
            .unwrap();
        assert_eq!(replacement.id, first.id);
        assert_eq!(replacement.authorization_epoch, "2");
        let accounts = database.accounts().await.unwrap();
        let current = database
            .detail(comments_query(&replacement, REPO_A))
            .await
            .unwrap();
        let other_saved = database
            .detail(comments_query(&other, REPO_A))
            .await
            .unwrap();
        let own_items = database
            .query_items(items(&replacement, REPO_A))
            .await
            .unwrap();
        let other_items = database.query_items(items(&other, REPO_A)).await.unwrap();
        let revision = database.revision().await.unwrap();
        fixture.held.release();
        assert!(
            tokio::time::timeout(Duration::from_secs(2), pending)
                .await
                .unwrap()
                .unwrap()
        );
        assert_eq!(
            database.revision().await.unwrap(),
            revision,
            "an obsolete grant publishes neither data nor failure/quota revisions"
        );
        assert_eq!(database.accounts().await.unwrap(), accounts);
        assert_eq!(
            database
                .detail(comments_query(&replacement, REPO_A))
                .await
                .unwrap(),
            current
        );
        assert_eq!(
            database
                .detail(comments_query(&other, REPO_A))
                .await
                .unwrap(),
            other_saved
        );
        assert_eq!(
            database
                .query_items(items(&replacement, REPO_A))
                .await
                .unwrap(),
            own_items
        );
        assert_eq!(
            database.query_items(items(&other, REPO_A)).await.unwrap(),
            other_items
        );
        for account in [&replacement, &other] {
            assert!(
                database
                    .scope_state(&account.id, "provider:rest")
                    .await
                    .unwrap()
                    .is_none()
            );
        }
        for saved in [draft, theirs] {
            assert_eq!(
                database
                    .draft(&saved.account_id, &saved.subject_id)
                    .await
                    .unwrap(),
                Some(saved)
            );
        }
        assert_eq!(
            runtime
                .hydrate_detail(comments_demand(&first, REPO_A))
                .await
                .unwrap_err()
                .code,
            ErrorCode::StaleView
        );
    }
}
