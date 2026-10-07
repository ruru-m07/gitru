//! Actual synthetic Bitbucket HTTP -> Runtime -> SQLite qualification.
//! No provider credential, keyring, background worker or public endpoint is used.
use super::*;
use crate::credentials::CredentialError;
use std::{
    io::{Read, Write},
    sync::{Condvar, Mutex as StdMutex, atomic::AtomicUsize},
};

const ACTOR_A: &str = "00000000-0000-4000-8000-000000000001";
const ACTOR_B: &str = "00000000-0000-4000-8000-000000000002";
const WORKSPACE: &str = "00000000-0000-4000-8000-000000000010";
const REPOSITORY: &str = "00000000-0000-4000-8000-000000000020";

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
    successful: AtomicBool,
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
            "fixture response not released"
        );
    }
}

#[derive(Clone, Copy)]
enum Mode {
    Normal = 0,
    Renamed = 1,
    Loop = 2,
    Cap = 3,
    DeniedActorA = 4,
}

fn response(status: u16, headers: &str, body: &str) -> String {
    format!(
        "HTTP/1.1 {status} Fixture\r\nContent-Length: {}\r\nConnection: close\r\n{headers}\r\n{body}",
        body.len()
    )
}
fn workspace() -> serde_json::Value {
    serde_json::json!({
        "type":"workspace_access", "administrator":false,
        "workspace":{"type":"workspace_base", "uuid":format!("{{{WORKSPACE}}}"), "slug":"workspace"}
    })
}
fn repository(renamed: bool) -> serde_json::Value {
    let slug = if renamed {
        "renamed-workspace"
    } else {
        "workspace"
    };
    let name = if renamed { "renamed-repo" } else { "repo" };
    let full_name = format!("{slug}/{name}");
    serde_json::json!({
        "type":"repository", "uuid":format!("{{{REPOSITORY}}}"), "scm":"git",
        "name":name,"full_name":full_name,"description":null,"mainbranch":null,
        "workspace":{"type":"workspace", "uuid":format!("{{{WORKSPACE}}}"),"slug":slug},
        "links":{"html":{"href":format!("https://bitbucket.org/{full_name}")},
            "clone":[{"name":"https","href":format!("https://bitbucket.org/{full_name}.git")},
                {"name":"ssh","href":format!("git@bitbucket.org:{full_name}.git")}]
        }
    })
}

/// Each request runs on a bounded owned connection thread so an old epoch's
/// response can remain held while real replacement probes complete.
struct HttpFixture {
    provider: Arc<providers::bitbucket_cloud::BitbucketCloudProvider>,
    calls: Arc<StdMutex<Vec<(String, String)>>>,
    mode: Arc<AtomicUsize>,
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
        let mode = Arc::new(AtomicUsize::new(Mode::Normal as usize));
        let held = Arc::new(Held::default());
        let stopped = Arc::new(AtomicBool::new(false));
        let task = {
            let calls = calls.clone();
            let mode = mode.clone();
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
                        Err(error) => panic!("fixture accept failed: {error}"),
                    };
                    assert!(connections.len() < 96, "fixture request budget exceeded");
                    let calls = calls.clone();
                    let mode = mode.clone();
                    let held = held.clone();
                    let base = base.clone();
                    connections.push(std::thread::spawn(move || {
                        stream.set_nonblocking(false).unwrap();
                        stream.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
                        stream.set_write_timeout(Some(Duration::from_secs(2))).unwrap();
                        let mut bytes = vec![];
                        loop {
                            let mut buffer = [0; 1024];
                            let size = stream.read(&mut buffer).unwrap();
                            bytes.extend_from_slice(&buffer[..size]);
                            assert!(bytes.len() <= 16_384, "fixture header budget exceeded");
                            if size == 0 || bytes.windows(4).any(|part| part == b"\r\n\r\n") {
                                break;
                            }
                        }
                        let request = String::from_utf8(bytes).unwrap();
                        let target = request.lines().next().unwrap().split_whitespace().nth(1).unwrap();
                        let token = request.lines().find_map(|line| {
                            let (name, value) = line.split_once(':')?;
                            name.eq_ignore_ascii_case("authorization")
                                .then(|| value.trim().strip_prefix("Bearer "))
                                .flatten()
                        }).expect("the real adapter must send the synthetic Bearer token");
                        assert!(!request.to_ascii_lowercase().contains("private-token:"));
                        assert!(!request.to_ascii_lowercase().contains("if-none-match:"));
                        calls.lock().unwrap().push((target.into(), token.into()));
                        let url = base.join(target).unwrap();
                        let query: HashMap<_, _> = url.query_pairs().into_owned().collect();
                        let output = if url.path() == "/2.0/user" {
                            if token == "denied-user" {
                                response(401, "", "private provider message")
                            } else {
                                let mut actor = serde_json::json!({
                                    "type":"user", "uuid":format!("{{{}}}", if token == "actor-b" {ACTOR_B} else {ACTOR_A}),
                                    "nickname":"same-mutable-nickname", "display_name":"Fixture Δ",
                                    "account_status":"active"
                                });
                                let headers = match token {
                                    "oversized-nickname-a" => {
                                        actor["nickname"] = "n".repeat(256).into();
                                        "Retry-After: 120\r\n"
                                    }
                                    "oversized-display-a" => {
                                        actor["display_name"] = "d".repeat(1025).into();
                                        "Retry-After: 120\r\n"
                                    }
                                    _ => "",
                                };
                                response(200, headers, &actor.to_string())
                            }
                        } else if url.path() == "/2.0/user/workspaces" {
                            assert_eq!(query.get("pagelen").map(String::as_str), Some("10"));
                            if token == "denied-workspace" {
                                response(403, "", "private provider message")
                            } else {
                                let mut item = workspace();
                                if mode.load(Ordering::SeqCst) == Mode::Renamed as usize {
                                    item["workspace"]["slug"] = "renamed-workspace".into();
                                }
                                response(200, "", &serde_json::json!({"values":[item]}).to_string())
                            }
                        } else if url.path() == format!("/2.0/repositories/%7B%7D/%7B{REPOSITORY}%7D/pullrequests") {
                            // Selection now legitimately admits the PR feed. These
                            // historical account/discovery cases use an actual
                            // empty all-state terminal response, without disabling
                            // the provider capability or changing discovery quotas.
                            let mut states: Vec<_> = url.query_pairs().filter(|(key,_)| key == "state").map(|(_,value)|value.into_owned()).collect();
                            states.sort_unstable();
                            assert_eq!(states,["DECLINED","MERGED","OPEN","SUPERSEDED"]);
                            assert_eq!(query.get("pagelen").map(String::as_str),Some("50"));
                            assert_eq!(query.get("sort").map(String::as_str),Some("id"));
                            response(200,"",r#"{"values":[]}"#)
                        } else if url.path().starts_with("/2.0/repositories/") {
                            assert!(target.to_ascii_lowercase().contains(&format!("%7b{WORKSPACE}%7d")));
                            assert_eq!(query.get("role").map(String::as_str), Some("member"));
                            assert_eq!(query.get("pagelen").map(String::as_str), Some("50"));
                            if token == "denied-repository"
                                || (token == "actor-a"
                                    && mode.load(Ordering::SeqCst) == Mode::DeniedActorA as usize)
                            {
                                response(403, "", "private provider message")
                            } else if token == "invalid-repository" {
                                response(200, "", r#"{"values":[{"type":"repository","uuid":"not-a-uuid"}]}"#)
                            } else if token == "actor-a" && held.armed.swap(false, Ordering::SeqCst) {
                                held.wait();
                                if held.successful.load(Ordering::SeqCst) {
                                    let mut obsolete = repository(true);
                                    obsolete["description"] = "obsolete grant private description".into();
                                    response(200, "Retry-After: 120\r\n", &serde_json::json!({"values":[obsolete]}).to_string())
                                } else {
                                    response(429, "Retry-After: 120\r\n", "private obsolete quota")
                                }
                            } else {
                                let current_mode = mode.load(Ordering::SeqCst);
                                let mut body = serde_json::json!({"values":[]});
                                let cursor = query.get("cursor").map(String::as_str);
                                if cursor.is_none()
                                    && current_mode != Mode::Loop as usize
                                    && current_mode != Mode::Cap as usize
                                {
                                    body["values"] = serde_json::json!([repository(current_mode == Mode::Renamed as usize)]);
                                }
                                if current_mode == Mode::Loop as usize || current_mode == Mode::Cap as usize {
                                    let next = match cursor {
                                        None => "step2".to_string(),
                                        Some("A") => "B".to_string(),
                                        Some("B") => "A".to_string(),
                                        Some(step) => {
                                            let number = step.strip_prefix("step").unwrap().parse::<u32>().unwrap();
                                            if current_mode == Mode::Loop as usize && number == 9 {
                                                "A".into()
                                            } else {
                                                format!("step{}", number + 1)
                                            }
                                        }
                                    };
                                    let mut continuation = url.clone();
                                    continuation.set_query(Some(&format!("role=member&pagelen=50&cursor={next}")));
                                    body["next"] = continuation.to_string().into();
                                }
                                response(200, "", &body.to_string())
                            }
                        } else {
                            panic!("unexpected fixture target: {target}");
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
            mode,
            held,
            stopped,
            task: Some(task),
        }
    }
    fn count(&self) -> usize {
        self.calls.lock().unwrap().len()
    }
    fn mode(&self, mode: Mode) {
        self.mode.store(mode as usize, Ordering::SeqCst);
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
fn assert_scope(actual: StoredScope, expected: &StoredScope) {
    assert_eq!(actual.run_id, expected.run_id);
    assert_eq!(actual.next_cursor, expected.next_cursor);
    assert_eq!(actual.etag, expected.etag);
    assert_eq!(actual.last_modified, expected.last_modified);
    assert_eq!(actual.coverage, expected.coverage);
    assert_eq!(actual.sync, expected.sync);
}
fn refresh(account: &RemoteAccount) -> RefreshRequest {
    RefreshRequest {
        account_id: account.id.clone(),
        repository_id: None,
        kind: None,
    }
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
async fn draft(runtime: &CollaborationRuntime, account: &RemoteAccount) -> LocalDraft {
    runtime
        .save_draft(LocalDraft {
            account_id: account.id.clone(),
            subject_id: format!("bitbucket:pull:{REPOSITORY}:67"),
            body: "private offline draft Δ".into(),
            generation: "0".into(),
        })
        .await
        .unwrap()
}

#[tokio::test]
async fn actual_bitbucket_probe_failures_do_not_stage_account_or_secret() {
    for (token, expected_calls) in [
        ("denied-user", 1),
        ("denied-workspace", 2),
        ("denied-repository", 3),
        ("invalid-repository", 3),
    ] {
        let dir = tempfile::tempdir().unwrap();
        let database = store(dir.path()).await;
        let vault = Arc::new(Vault::default());
        let fixture = HttpFixture::new();
        let runtime = make_runtime(database.clone(), vault.clone(), &fixture, Clock::new());
        let error = runtime
            .connect_bitbucket_cloud(token.into())
            .await
            .unwrap_err();
        assert!(!error.message.contains(token));
        assert!(!error.message.contains("private"));
        assert!(database.accounts().await.unwrap().accounts.is_empty());
        assert!(
            database
                .due_credential_cleanup(i64::MAX, 32)
                .await
                .unwrap()
                .is_empty()
        );
        assert_eq!(vault.stores.load(Ordering::SeqCst), 0);
        assert_eq!(vault.loads.load(Ordering::SeqCst), 0);
        assert_eq!(fixture.count(), expected_calls);
    }
}

#[tokio::test]
async fn bitbucket_known_actor_invalid_presentation_keeps_quota_without_replacing_grant_or_cache() {
    for token in ["oversized-nickname-a", "oversized-display-a"] {
        let dir = tempfile::tempdir().unwrap();
        let database = store(dir.path()).await;
        let vault = Arc::new(Vault::default());
        let fixture = HttpFixture::new();
        let clock = Clock::new();
        let runtime = make_runtime(database.clone(), vault.clone(), &fixture, clock.clone());
        let account = runtime
            .connect_bitbucket_cloud("actor-a".into())
            .await
            .unwrap();
        for _ in 0..2 {
            assert!(runtime.run_next().await);
        }
        let original = database
            .repositories(&account.id)
            .await
            .unwrap()
            .repositories
            .pop()
            .unwrap();
        runtime
            .select_repository(&account.id, &original.id, true)
            .await
            .unwrap();
        assert!(runtime.run_next().await); // Actual terminal all-state PR feed.
        let saved = draft(&runtime, &account).await;
        let rows_before = database.repositories(&account.id).await.unwrap();
        let scope_before = database
            .scope_state(&account.id, "repositories")
            .await
            .unwrap()
            .unwrap();
        let reference = database.credential_reference(&account.id).await.unwrap();
        let stores = vault.stores.load(Ordering::SeqCst);
        let calls = fixture.count();
        let error = runtime
            .connect_bitbucket_cloud(token.into())
            .await
            .unwrap_err();
        assert_eq!(error.code, ErrorCode::Provider);
        assert!(!error.message.contains(token));
        assert_eq!(
            fixture.count(),
            calls + 1,
            "only the invalid-presentation user probe runs"
        );
        assert_eq!(database.account(&account.id).await.unwrap(), account);
        assert_eq!(
            database.credential_reference(&account.id).await.unwrap(),
            reference
        );
        assert_eq!(vault.stores.load(Ordering::SeqCst), stores);
        assert_eq!(vault.tokens.lock().unwrap().len(), 1);
        assert!(
            database
                .due_credential_cleanup(i64::MAX, 32)
                .await
                .unwrap()
                .is_empty()
        );
        let after = database.repositories(&account.id).await.unwrap();
        assert_eq!(after.repositories, rows_before.repositories);
        assert_eq!(after.authorization_view, rows_before.authorization_view);
        assert_eq!(after.coverage, rows_before.coverage);
        assert_eq!(after.sync, rows_before.sync);
        assert_scope(
            database
                .scope_state(&account.id, "repositories")
                .await
                .unwrap()
                .unwrap(),
            &scope_before,
        );
        assert_eq!(
            database
                .draft(&account.id, &saved.subject_id)
                .await
                .unwrap(),
            Some(saved.clone())
        );
        let quota = database
            .scope_state(&account.id, "provider:rest")
            .await
            .unwrap()
            .expect("proven actor quota survives invalid mutable presentation");
        assert_eq!(quota.sync.state, SyncState::RateLimited);
        let deadline =
            DateTime::parse_from_rfc3339(quota.sync.next_retry_at.as_deref().unwrap()).unwrap();
        assert_eq!(
            deadline.timestamp_millis() - runtime.clock.utc().timestamp_millis(),
            120_000
        );
        let loads = vault.loads.load(Ordering::SeqCst);
        runtime.refresh(refresh(&account)).await.unwrap();
        assert!(!runtime.run_next().await);
        assert_eq!(vault.loads.load(Ordering::SeqCst), loads);
        assert_eq!(fixture.count(), calls + 1);
        database.close().await.unwrap();
        drop(runtime);
        drop(database);
        let reopened = store(dir.path()).await;
        vault.loads.store(0, Ordering::SeqCst);
        let cold = make_runtime(reopened.clone(), vault.clone(), &fixture, clock);
        assert_scope(
            reopened
                .scope_state(&account.id, "provider:rest")
                .await
                .unwrap()
                .unwrap(),
            &quota,
        );
        assert_eq!(reopened.account(&account.id).await.unwrap(), account);
        assert_eq!(
            reopened
                .draft(&account.id, &saved.subject_id)
                .await
                .unwrap(),
            Some(saved)
        );
        assert_eq!(
            reopened
                .repositories(&account.id)
                .await
                .unwrap()
                .repositories,
            rows_before.repositories
        );
        cold.refresh(refresh(&account)).await.unwrap();
        assert!(!cold.run_next().await);
        assert_eq!(vault.loads.load(Ordering::SeqCst), 0);
        assert_eq!(
            fixture.count(),
            calls + 1,
            "cold quota gate blocks provider access without loading a token"
        );
    }
}

#[tokio::test]
async fn bitbucket_uuid_rename_selection_and_private_draft_survive_inert_cold_reads() {
    let dir = tempfile::tempdir().unwrap();
    let database = store(dir.path()).await;
    let vault = Arc::new(Vault::default());
    let fixture = HttpFixture::new();
    let clock = Clock::new();
    let runtime = make_runtime(database.clone(), vault.clone(), &fixture, clock.clone());
    let account = runtime
        .connect_bitbucket_cloud("actor-a".into())
        .await
        .unwrap();
    assert_eq!(account.actor_id, ACTOR_A);
    assert_eq!(fixture.count(), 3);
    assert!(
        database
            .scope_state(&account.id, "repositories")
            .await
            .unwrap()
            .is_none(),
        "probe pages establish no cache coverage"
    );
    assert!(runtime.run_next().await);
    let empty = database.repositories(&account.id).await.unwrap();
    assert!(empty.repositories.is_empty());
    assert_eq!(
        empty.coverage.state,
        CoverageState::Partial,
        "workspace enumeration is an intermediate page"
    );
    assert!(runtime.run_next().await);
    let original = database
        .repositories(&account.id)
        .await
        .unwrap()
        .repositories
        .pop()
        .unwrap();
    assert_eq!(original.provider_id, REPOSITORY);
    runtime
        .select_repository(&account.id, &original.id, true)
        .await
        .unwrap();
    assert!(runtime.run_next().await); // Actual terminal all-state PR feed.
    let saved = draft(&runtime, &account).await;
    fixture.mode(Mode::Renamed);
    let refresh_calls = fixture.count();
    runtime.refresh(refresh(&account)).await.unwrap();
    for _ in 0..3 {
        assert!(runtime.run_next().await);
    }
    assert_eq!(
        fixture.count(),
        refresh_calls + 3,
        "account refresh admits two discovery pages plus the selected PR feed"
    );
    let renamed = database
        .repository(&account.id, &original.id)
        .await
        .unwrap();
    assert_eq!(renamed.id, original.id);
    assert_eq!(renamed.provider_id, original.provider_id);
    assert_eq!(renamed.full_name, "renamed-workspace/renamed-repo");
    assert!(renamed.selected);
    let capabilities = runtime.capabilities(&account.id).await.unwrap();
    assert_eq!(capabilities.inbox_semantics, InboxSemantics::None);
    assert!(!account.notifications_supported);
    let snapshot = database.repositories(&account.id).await.unwrap();
    assert_eq!(snapshot.coverage.state, CoverageState::Complete);
    let calls = fixture.count();
    database.close().await.unwrap();
    drop(runtime);
    drop(database);
    let reopened = store(dir.path()).await;
    vault.loads.store(0, Ordering::SeqCst);
    let cold = make_runtime(reopened.clone(), vault.clone(), &fixture, clock);
    assert_eq!(reopened.repositories(&account.id).await.unwrap(), snapshot);
    assert_eq!(reopened.account(&account.id).await.unwrap(), account);
    assert_eq!(cold.capabilities(&account.id).await.unwrap(), capabilities);
    assert_eq!(
        reopened
            .draft(&account.id, &saved.subject_id)
            .await
            .unwrap(),
        Some(saved)
    );
    assert_eq!(fixture.count(), calls, "cold projections perform no HTTP");
    assert_eq!(
        vault.loads.load(Ordering::SeqCst),
        0,
        "cold projections do not open the vault"
    );
    assert!(
        !cold.run_next().await,
        "construction does not schedule provider work"
    );
}

#[tokio::test]
async fn bitbucket_same_nickname_actors_and_held_old_epoch_keep_private_rows_and_quota_partitioned()
{
    for successful in [false, true] {
        let dir = tempfile::tempdir().unwrap();
        let database = store(dir.path()).await;
        let vault = Arc::new(Vault::default());
        let fixture = HttpFixture::new();
        fixture.held.successful.store(successful, Ordering::SeqCst);
        let runtime = make_runtime(database.clone(), vault.clone(), &fixture, Clock::new());
        let first = runtime
            .connect_bitbucket_cloud("actor-a".into())
            .await
            .unwrap();
        let saved = draft(&runtime, &first).await;
        assert!(runtime.run_next().await); // Commit the old epoch's intermediate workspace cursor.
        fixture.held.armed.store(true, Ordering::SeqCst);
        let worker = runtime.clone();
        let read = tokio::spawn(async move { worker.run_next().await });
        tokio::time::timeout(Duration::from_secs(3), fixture.held.entered.notified())
            .await
            .unwrap();
        let other = runtime
            .connect_bitbucket_cloud("actor-b".into())
            .await
            .unwrap();
        let old_reference = database
            .credential_reference(&first.id)
            .await
            .unwrap()
            .unwrap();
        let replacement = runtime
            .connect_bitbucket_cloud("replacement-a".into())
            .await
            .unwrap();
        assert_eq!(first.id, replacement.id);
        assert_eq!(replacement.authorization_epoch, "2");
        assert_eq!(first.login, other.login);
        assert_eq!(other.actor_id, ACTOR_B);
        assert_ne!(replacement.id, other.id);
        assert!(!vault.tokens.lock().unwrap().contains_key(&old_reference));
        assert_eq!(vault.tokens.lock().unwrap().len(), 2);
        assert_eq!(
            database
                .draft(&replacement.id, &saved.subject_id)
                .await
                .unwrap(),
            Some(saved.clone())
        );
        assert!(
            database
                .draft(&other.id, &saved.subject_id)
                .await
                .unwrap()
                .is_none()
        );
        let accounts_before = database.accounts().await.unwrap();
        let replacement_before = database.repositories(&replacement.id).await.unwrap();
        let other_before = database.repositories(&other.id).await.unwrap();
        fixture.held.release();
        assert!(
            tokio::time::timeout(Duration::from_secs(3), read)
                .await
                .unwrap()
                .unwrap()
        );
        assert_eq!(database.accounts().await.unwrap(), accounts_before);
        assert_eq!(
            database.repositories(&replacement.id).await.unwrap(),
            replacement_before
        );
        assert_eq!(
            database.repositories(&other.id).await.unwrap(),
            other_before
        );
        assert_eq!(
            database.account(&replacement.id).await.unwrap(),
            replacement
        );
        assert!(
            database
                .scope_state(&replacement.id, "provider:rest")
                .await
                .unwrap()
                .is_none(),
            "obsolete200 or429 cannot cool a replacement grant"
        );
        assert!(
            database
                .repositories(&replacement.id)
                .await
                .unwrap()
                .repositories
                .is_empty()
        );
        for _ in 0..4 {
            assert!(runtime.run_next().await);
        }
        let own = database
            .repositories(&replacement.id)
            .await
            .unwrap()
            .repositories
            .pop()
            .unwrap();
        assert_eq!(own.full_name, "workspace/repo");
        assert_ne!(
            own.description.as_deref(),
            Some("obsolete grant private description")
        );
        let theirs = database
            .repositories(&other.id)
            .await
            .unwrap()
            .repositories
            .pop()
            .unwrap();
        assert_eq!(own.provider_id, theirs.provider_id);
        assert_eq!(
            own.id, theirs.id,
            "canonical provider repository UUID is account-scoped in storage"
        );
        assert_eq!(own.account_id, replacement.id);
        assert_eq!(theirs.account_id, other.id);
        assert_eq!(
            database
                .draft(&replacement.id, &saved.subject_id)
                .await
                .unwrap(),
            Some(saved)
        );
    }
}

#[tokio::test]
async fn bitbucket_actual_permission_loss_and_replacement_never_cross_account_or_private_draft() {
    let dir = tempfile::tempdir().unwrap();
    let database = store(dir.path()).await;
    let vault = Arc::new(Vault::default());
    let fixture = HttpFixture::new();
    let clock = Clock::new();
    let runtime = make_runtime(database.clone(), vault.clone(), &fixture, clock.clone());
    let first = runtime
        .connect_bitbucket_cloud("actor-a".into())
        .await
        .unwrap();
    for _ in 0..2 {
        assert!(runtime.run_next().await);
    }
    let other = runtime
        .connect_bitbucket_cloud("actor-b".into())
        .await
        .unwrap();
    for _ in 0..2 {
        assert!(runtime.run_next().await);
    }
    let repository = database
        .repositories(&first.id)
        .await
        .unwrap()
        .repositories
        .pop()
        .unwrap();
    runtime
        .select_repository(&first.id, &repository.id, true)
        .await
        .unwrap();
    assert!(runtime.run_next().await); // Actual terminal all-state PR feed.
    let saved = draft(&runtime, &first).await;
    let theirs = runtime
        .save_draft(LocalDraft {
            account_id: other.id.clone(),
            subject_id: saved.subject_id.clone(),
            body: "other actor private draft λ".into(),
            generation: "0".into(),
        })
        .await
        .unwrap();
    fixture.mode(Mode::DeniedActorA);
    runtime.refresh(refresh(&first)).await.unwrap();
    for _ in 0..3 {
        assert!(runtime.run_next().await);
    }
    let denied = database.repositories(&first.id).await.unwrap();
    assert!(denied.repositories.is_empty());
    assert_eq!(
        denied.sync.error.as_ref().unwrap().code,
        ErrorCode::PermissionDenied
    );
    assert_eq!(
        database.account(&first.id).await.unwrap(),
        first,
        "resource403 does not revoke a verified actor"
    );
    assert_eq!(
        database
            .repositories(&other.id)
            .await
            .unwrap()
            .repositories
            .len(),
        1
    );
    assert_eq!(
        database.draft(&first.id, &saved.subject_id).await.unwrap(),
        Some(saved.clone())
    );
    assert_eq!(
        database.draft(&other.id, &theirs.subject_id).await.unwrap(),
        Some(theirs.clone())
    );
    let stores = vault.stores.load(Ordering::SeqCst);
    assert!(
        runtime
            .connect_bitbucket_cloud("denied-repository".into())
            .await
            .is_err()
    );
    assert_eq!(
        vault.stores.load(Ordering::SeqCst),
        stores,
        "rejected replacement cannot stage a new grant"
    );
    assert_eq!(database.repositories(&first.id).await.unwrap(), denied);
    fixture.mode(Mode::Normal);
    let replacement = runtime
        .connect_bitbucket_cloud("replacement-a".into())
        .await
        .unwrap();
    assert_eq!(replacement.id, first.id);
    assert_eq!(replacement.authorization_epoch, "2");
    assert!(
        database
            .repositories(&replacement.id)
            .await
            .unwrap()
            .repositories
            .is_empty(),
        "a new grant revalidates private cached membership"
    );
    for _ in 0..2 {
        assert!(runtime.run_next().await);
    }
    let restored = database.repositories(&replacement.id).await.unwrap();
    assert_eq!(restored.repositories.len(), 1);
    assert_eq!(restored.repositories[0].provider_id, repository.provider_id);
    assert_eq!(restored.coverage.state, CoverageState::Complete);
    assert_eq!(
        database
            .draft(&replacement.id, &saved.subject_id)
            .await
            .unwrap(),
        Some(saved.clone())
    );
    let other_rows = database.repositories(&other.id).await.unwrap();
    let calls = fixture.count();
    database.close().await.unwrap();
    drop(runtime);
    drop(database);
    let reopened = store(dir.path()).await;
    vault.loads.store(0, Ordering::SeqCst);
    let cold = make_runtime(reopened.clone(), vault.clone(), &fixture, clock);
    assert_eq!(
        reopened.repositories(&replacement.id).await.unwrap(),
        restored
    );
    assert_eq!(reopened.repositories(&other.id).await.unwrap(), other_rows);
    assert_eq!(
        reopened
            .draft(&replacement.id, &saved.subject_id)
            .await
            .unwrap(),
        Some(saved)
    );
    assert_eq!(
        reopened.draft(&other.id, &theirs.subject_id).await.unwrap(),
        Some(theirs)
    );
    assert!(!cold.run_next().await);
    assert_eq!(fixture.count(), calls);
    assert_eq!(vault.loads.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn bitbucket_opaque_loop_history_survives_job_yield_reopen_and_rejected_page_retry() {
    let dir = tempfile::tempdir().unwrap();
    let database = store(dir.path()).await;
    let vault = Arc::new(Vault::default());
    let fixture = HttpFixture::new();
    let clock = Clock::new();
    let runtime = make_runtime(database.clone(), vault.clone(), &fixture, clock.clone());
    let account = runtime
        .connect_bitbucket_cloud("actor-a".into())
        .await
        .unwrap();
    for _ in 0..2 {
        assert!(runtime.run_next().await);
    }
    let complete = database.repositories(&account.id).await.unwrap();
    assert_eq!(complete.coverage.state, CoverageState::Complete);
    assert_eq!(complete.repositories.len(), 1);
    let calls_before = fixture.count();
    fixture.mode(Mode::Loop);
    runtime.refresh(refresh(&account)).await.unwrap();
    let saved = draft(&runtime, &account).await;
    for _ in 0..10 {
        assert!(runtime.run_next().await);
    }
    assert_eq!(fixture.count(), calls_before + 10);
    let accepted = database
        .scope_state(&account.id, "repositories")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(accepted.coverage.state, CoverageState::Partial);
    assert!(accepted.next_cursor.as_ref().unwrap().len() <= 4096);
    let rows = database
        .repositories(&account.id)
        .await
        .unwrap()
        .repositories;
    assert_eq!(
        rows, complete.repositories,
        "Partial pages do not infer historical absence"
    );
    database.close().await.unwrap();
    drop(runtime);
    drop(database);
    let reopened = store(dir.path()).await;
    let runtime = make_runtime(reopened.clone(), vault.clone(), &fixture, clock.clone());
    assert_scope(
        reopened
            .scope_state(&account.id, "repositories")
            .await
            .unwrap()
            .unwrap(),
        &accepted,
    );
    runtime.refresh(refresh(&account)).await.unwrap();
    assert!(runtime.run_next().await); // A is accepted, with B as its opaque next.
    let before_rejection = reopened
        .scope_state(&account.id, "repositories")
        .await
        .unwrap()
        .unwrap();
    assert!(runtime.run_next().await); // B proposes already accepted A; B is rejected atomically.
    let rejected = reopened
        .scope_state(&account.id, "repositories")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(rejected.next_cursor, before_rejection.next_cursor);
    assert_eq!(rejected.run_id, accepted.run_id);
    assert_eq!(rejected.coverage.state, CoverageState::Partial);
    assert_eq!(
        rejected.coverage.validated_at,
        complete.coverage.validated_at
    );
    assert_eq!(rejected.sync.state, SyncState::Error);
    assert!(rejected.sync.next_retry_at.is_some());
    assert_eq!(
        reopened
            .repositories(&account.id)
            .await
            .unwrap()
            .repositories,
        rows
    );
    let calls = fixture.count();
    let loads = vault.loads.load(Ordering::SeqCst);
    runtime.refresh(refresh(&account)).await.unwrap();
    assert!(!runtime.run_next().await);
    assert_eq!(fixture.count(), calls);
    assert_eq!(vault.loads.load(Ordering::SeqCst), loads);
    clock.advance(61);
    assert!(runtime.run_next().await); // A rejected receipt may refetch still-current B after backoff.
    assert_eq!(
        fixture.cursor_calls("A"),
        1,
        "an opaque loop is never followed back to A"
    );
    assert_eq!(
        fixture.cursor_calls("B"),
        2,
        "only the rejected current B can retry"
    );
    assert_eq!(
        reopened
            .repositories(&account.id)
            .await
            .unwrap()
            .repositories,
        rows
    );
    assert_eq!(
        reopened
            .draft(&account.id, &saved.subject_id)
            .await
            .unwrap(),
        Some(saved)
    );
    assert_eq!(
        reopened
            .scope_state(&account.id, "repositories")
            .await
            .unwrap()
            .unwrap()
            .coverage
            .state,
        CoverageState::Partial
    );
}

#[tokio::test]
async fn bitbucket_twenty_page_cap_is_durable_across_jobs_reopen_and_manual_refresh() {
    let dir = tempfile::tempdir().unwrap();
    let database = store(dir.path()).await;
    let vault = Arc::new(Vault::default());
    let fixture = HttpFixture::new();
    let clock = Clock::new();
    let runtime = make_runtime(database.clone(), vault.clone(), &fixture, clock.clone());
    let account = runtime
        .connect_bitbucket_cloud("actor-a".into())
        .await
        .unwrap();
    for _ in 0..2 {
        assert!(runtime.run_next().await);
    }
    let complete = database.repositories(&account.id).await.unwrap();
    assert_eq!(complete.coverage.state, CoverageState::Complete);
    assert_eq!(complete.repositories.len(), 1);
    let calls_before = fixture.count();
    fixture.mode(Mode::Cap);
    runtime.refresh(refresh(&account)).await.unwrap();
    let saved = draft(&runtime, &account).await;
    for _ in 0..10 {
        assert!(runtime.run_next().await);
    }
    runtime.refresh(refresh(&account)).await.unwrap();
    for _ in 0..10 {
        assert!(runtime.run_next().await);
    }
    assert_eq!(
        fixture.count(),
        calls_before + 20,
        "twenty accepted empty enumeration pages follow the historical complete cache"
    );
    let exhausted = database
        .scope_state(&account.id, "repositories")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(exhausted.coverage.state, CoverageState::Partial);
    assert_eq!(
        exhausted.coverage.validated_at,
        complete.coverage.validated_at
    );
    assert!(exhausted.coverage.remote_has_more);
    assert!(exhausted.next_cursor.as_ref().unwrap().len() <= 4096);
    let rows = database
        .repositories(&account.id)
        .await
        .unwrap()
        .repositories;
    assert_eq!(
        rows, complete.repositories,
        "Partial pages do not infer historical absence"
    );
    database.close().await.unwrap();
    drop(runtime);
    drop(database);
    let reopened = store(dir.path()).await;
    let runtime = make_runtime(reopened.clone(), vault.clone(), &fixture, clock.clone());
    assert_scope(
        reopened
            .scope_state(&account.id, "repositories")
            .await
            .unwrap()
            .unwrap(),
        &exhausted,
    );
    let calls = fixture.count();
    for _ in 0..2 {
        runtime.refresh(refresh(&account)).await.unwrap();
        assert!(
            runtime.run_next().await,
            "cap becomes a finite error receipt without HTTP"
        );
        let scope = reopened
            .scope_state(&account.id, "repositories")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(scope.next_cursor, exhausted.next_cursor);
        assert_eq!(scope.run_id, exhausted.run_id);
        assert_eq!(scope.coverage.state, CoverageState::Partial);
        assert_eq!(scope.sync.state, SyncState::Error);
        runtime.refresh(refresh(&account)).await.unwrap();
        assert!(
            !runtime.run_next().await,
            "manual admission cannot erase persisted backoff"
        );
        assert_eq!(
            fixture.count(),
            calls,
            "even a new runtime cannot resume past the accepted-page cap"
        );
        clock.advance(181);
    }
    assert_eq!(
        reopened
            .repositories(&account.id)
            .await
            .unwrap()
            .repositories,
        rows
    );
    assert_eq!(
        reopened
            .draft(&account.id, &saved.subject_id)
            .await
            .unwrap(),
        Some(saved)
    );
}
