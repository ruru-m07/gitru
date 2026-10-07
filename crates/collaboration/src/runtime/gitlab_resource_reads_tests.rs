//! Actual GitLab HTTP -> runtime -> SQLite qualification, with synthetic tokens only.
use super::*;
use std::{
    io::{Read, Write},
    sync::Condvar,
};

const PROJECT: u64 = 9_007_199_254_740_993;
const PULL: u64 = 9_007_199_254_741_993;
const ISSUE: u64 = 9_007_199_254_742_993;
const COPIED_PROJECT: u64 = 9_007_199_254_744_993;
const COPIED_ISSUE: u64 = 9_007_199_254_745_993;
const IID: u64 = 67;

fn subject(kind: RemoteItemKind) -> String {
    match kind {
        RemoteItemKind::PullRequest => format!("gitlab:pull:{PULL}"),
        RemoteItemKind::Issue => format!("gitlab:issue:{ISSUE}"),
        RemoteItemKind::Notification => panic!("fixture has no GitLab inbox"),
    }
}

fn resource(kind: RemoteItemKind, body: &str) -> serde_json::Value {
    let pull = kind == RemoteItemKind::PullRequest;
    let route = if pull { "merge_requests" } else { "issues" };
    let mut value = serde_json::json!({
        "id": if pull { PULL } else { ISSUE }, "iid": IID, "project_id": PROJECT,
        "title": if pull { "Draft: native MR Δ" } else { "Native issue λ" },
        "state": "opened", "created_at": "2026-10-01T00:00:00Z",
        "updated_at": "2026-10-04T00:00:00Z", "description": body,
        "web_url": format!("https://gitlab.com/group/sub/project/-/{route}/{IID}"),
        "author": { "id": 9_007_199_254_743_993_u64, "username": "mutable-author" },
        "labels": ["area::collaboration", "Δ"], "assignees": [], "milestone": null
    });
    if pull {
        value["draft"] = true.into();
        value["source_branch"] = "feature/local".into();
        value["target_branch"] = "main".into();
        value["sha"] = "a".repeat(40).into();
        // Async diff refs may be absent even though the summary has a SHA.
        value["diff_refs"] = serde_json::Value::Null;
    }
    value
}

fn moved_issue(body: &str, copied: bool) -> serde_json::Value {
    let mut value = resource(RemoteItemKind::Issue, body);
    value["updated_at"] = "2026-10-04T01:00:00Z".into();
    if copied {
        value["id"] = COPIED_ISSUE.into();
        value["project_id"] = COPIED_PROJECT.into();
        value["web_url"] = format!("https://gitlab.com/group/sub/copied/-/issues/{IID}").into();
    } else {
        value["state"] = "closed".into();
    }
    value
}

#[derive(Default)]
struct HeldBody {
    active: AtomicBool,
    entered: Notify,
    released: StdMutex<bool>,
    release: Condvar,
}
impl HeldBody {
    fn release(&self) {
        *self.released.lock().unwrap() = true;
        self.release.notify_all();
    }
    fn hold(&self) {
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
            "synthetic Body was not released"
        );
    }
}

/// Bounded concurrent server: replacement probes can complete while an old
/// singleton response is held. It never connects to a provider or keyring.
struct HttpFixture {
    provider: Arc<providers::gitlab::GitlabProvider>,
    calls: Arc<StdMutex<Vec<String>>>,
    held: Arc<HeldBody>,
    denied: Arc<AtomicBool>,
    moved: Arc<AtomicBool>,
    stopped: Arc<AtomicBool>,
    task: Option<std::thread::JoinHandle<()>>,
}
impl HttpFixture {
    fn new() -> Self {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let base = reqwest::Url::parse(&format!(
            "http://{}/api/v4/",
            listener.local_addr().unwrap()
        ))
        .unwrap();
        let calls = Arc::new(StdMutex::new(Vec::new()));
        let held = Arc::new(HeldBody::default());
        let denied = Arc::new(AtomicBool::new(false));
        let moved = Arc::new(AtomicBool::new(false));
        let stopped = Arc::new(AtomicBool::new(false));
        let task = {
            let calls = calls.clone();
            let held = held.clone();
            let denied = denied.clone();
            let moved = moved.clone();
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
                        Err(error) => panic!("fixture accept failed: {error}"),
                    };
                    assert!(connections.len() < 64, "fixture request budget exceeded");
                    let calls = calls.clone();
                    let held = held.clone();
                    let denied = denied.clone();
                    let moved = moved.clone();
                    connections.push(std::thread::spawn(move || {
                        stream.set_nonblocking(false).unwrap();
                        stream
                            .set_read_timeout(Some(Duration::from_secs(2)))
                            .unwrap();
                        stream
                            .set_write_timeout(Some(Duration::from_secs(2)))
                            .unwrap();
                        let mut bytes = vec![];
                        loop {
                            let mut buffer = [0; 1024];
                            let size = stream.read(&mut buffer).unwrap();
                            bytes.extend_from_slice(&buffer[..size]);
                            assert!(bytes.len() <= 16_384, "fixture header bound exceeded");
                            if size == 0 || bytes.windows(4).any(|value| value == b"\r\n\r\n") {
                                break;
                            }
                        }
                        let request = String::from_utf8(bytes).unwrap();
                        calls.lock().unwrap().push(request.clone());
                        let target = request
                            .lines()
                            .next()
                            .unwrap()
                            .split_whitespace()
                            .nth(1)
                            .unwrap();
                        let token = request
                            .lines()
                            .find_map(|line| {
                                let (name, value) = line.split_once(':')?;
                                name.eq_ignore_ascii_case("private-token")
                                    .then(|| value.trim())
                            })
                            .expect("actual adapter must send the synthetic PAT header");
                        let output = if target == "/api/v4/user" {
                            response(
                                200,
                                "",
                                &serde_json::json!({
                                    "id": if token == "actor-b" { 2 } else { 1 },
                                    "username": "same-mutable-login"
                                })
                                .to_string(),
                            )
                        } else if target.starts_with("/api/v4/projects?") {
                            let mut projects = vec![project(PROJECT, "group/sub/project")];
                            if moved.load(Ordering::SeqCst) {
                                projects.push(project(COPIED_PROJECT, "group/sub/copied"));
                            }
                            response(200, "", &serde_json::json!(projects).to_string())
                        } else if target
                            == format!("/api/v4/projects/{PROJECT}/merge_requests/{IID}")
                        {
                            if token == "actor-a" && held.active.load(Ordering::SeqCst) {
                                held.hold();
                                response(
                                    200,
                                    "RateLimit-Remaining: 0\r\n",
                                    &resource(
                                        RemoteItemKind::PullRequest,
                                        "obsolete old-epoch Body",
                                    )
                                    .to_string(),
                                )
                            } else if token == "actor-a" && denied.load(Ordering::SeqCst) {
                                response(404, "", "concealed synthetic project")
                            } else {
                                response(
                                    200,
                                    "",
                                    &resource(
                                        RemoteItemKind::PullRequest,
                                        &format!("singleton Body for {token} π 🌱"),
                                    )
                                    .to_string(),
                                )
                            }
                        } else if target
                            .starts_with(&format!("/api/v4/projects/{PROJECT}/merge_requests?"))
                        {
                            response(
                                200,
                                "",
                                &serde_json::json!([resource(
                                    RemoteItemKind::PullRequest,
                                    "summary is not independent Body"
                                )])
                                .to_string(),
                            )
                        } else if target == format!("/api/v4/projects/{PROJECT}/issues/{IID}") {
                            response(
                                200,
                                "",
                                &resource(
                                    RemoteItemKind::Issue,
                                    "independent singleton issue Body λ",
                                )
                                .to_string(),
                            )
                        } else if target.starts_with(&format!("/api/v4/projects/{PROJECT}/issues?"))
                        {
                            let issue = if moved.load(Ordering::SeqCst) {
                                moved_issue("summary issue text", false)
                            } else {
                                resource(RemoteItemKind::Issue, "summary issue text")
                            };
                            response(200, "", &serde_json::json!([issue]).to_string())
                        } else if moved.load(Ordering::SeqCst)
                            && target.starts_with(&format!(
                                "/api/v4/projects/{COPIED_PROJECT}/merge_requests?"
                            ))
                        {
                            response(200, "", "[]")
                        } else if moved.load(Ordering::SeqCst)
                            && target
                                .starts_with(&format!("/api/v4/projects/{COPIED_PROJECT}/issues?"))
                        {
                            response(
                                200,
                                "",
                                &serde_json::json!([moved_issue("summary issue text", true)])
                                    .to_string(),
                            )
                        } else if moved.load(Ordering::SeqCst)
                            && target == format!("/api/v4/projects/{COPIED_PROJECT}/issues/{IID}")
                        {
                            response(
                                200,
                                "",
                                &moved_issue("independent copied singleton Body λ", true)
                                    .to_string(),
                            )
                        } else {
                            panic!("unexpected fixture route: {target}")
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
            provider: Arc::new(providers::gitlab::GitlabProvider::fixture(base)),
            calls,
            held,
            denied,
            moved,
            stopped,
            task: Some(task),
        }
    }
    fn count(&self) -> usize {
        self.calls.lock().unwrap().len()
    }
}
impl Drop for HttpFixture {
    fn drop(&mut self) {
        self.held.release();
        self.stopped.store(true, Ordering::SeqCst);
        if let Some(task) = self.task.take() {
            // Preserve the first assertion when unwinding instead of a double panic.
            let result = task.join();
            if !std::thread::panicking() {
                result.unwrap();
            }
        }
    }
}

fn query(account: &RemoteAccount, kind: RemoteItemKind) -> DetailQuery {
    DetailQuery {
        account_id: account.id.clone(),
        subject_id: subject(kind),
        facet: DetailFacet::Body,
        cursor: None,
        limit: 50,
    }
}
fn demand(account: &RemoteAccount, kind: RemoteItemKind) -> HydrateDetailRequest {
    HydrateDetailRequest {
        account_id: account.id.clone(),
        authorization_epoch: account.authorization_epoch.clone(),
        subject_id: subject(kind),
        facet: DetailFacet::Body,
    }
}
fn items(
    account: &RemoteAccount,
    repository: &RemoteRepository,
    kind: RemoteItemKind,
) -> ItemQuery {
    ItemQuery {
        account_id: account.id.clone(),
        kind,
        repository_id: Some(repository.id.clone()),
        state: None,
        search: None,
        cursor: None,
        limit: 50,
    }
}
fn repository_target(
    account: &RemoteAccount,
    repository: &RemoteRepository,
) -> ContextCapabilityRequest {
    ContextCapabilityRequest {
        account_id: account.id.clone(),
        authorization_epoch: account.authorization_epoch.clone(),
        target: CapabilityTarget {
            kind: CapabilityTargetKind::Repository,
            instance_id: Some(ProviderInstance::public(ProviderKind::Gitlab).id),
            repository_id: Some(repository.id.clone()),
            resource_id: None,
            resource_kind: None,
        },
    }
}
fn resource_target(account: &RemoteAccount, kind: RemoteItemKind) -> ContextCapabilityRequest {
    ContextCapabilityRequest {
        account_id: account.id.clone(),
        authorization_epoch: account.authorization_epoch.clone(),
        target: CapabilityTarget {
            kind: CapabilityTargetKind::Resource,
            instance_id: Some(ProviderInstance::public(ProviderKind::Gitlab).id),
            repository_id: None,
            resource_id: Some(subject(kind.clone())),
            resource_kind: Some(if kind == RemoteItemKind::PullRequest {
                ResourceKind::PullRequest
            } else {
                ResourceKind::Issue
            }),
        },
    }
}
async fn selected(
    runtime: &CollaborationRuntime,
    token: &str,
) -> (RemoteAccount, RemoteRepository) {
    let account = runtime.connect_gitlab(token.into()).await.unwrap();
    assert!(runtime.run_next().await);
    let repository = runtime
        .store
        .repositories(&account.id)
        .await
        .unwrap()
        .repositories
        .pop()
        .unwrap();
    let before = runtime
        .contextual_capabilities(repository_target(&account, &repository))
        .await
        .unwrap();
    let pulls = before
        .facets
        .iter()
        .find(|facet| facet.facet == ResourceFacet::PullRequests)
        .unwrap();
    assert_eq!(pulls.synchronize.state, CapabilityState::Unavailable);
    assert_eq!(
        pulls.synchronize.reason,
        Some(ContextCapabilityReason::RepositoryNotSelected)
    );
    runtime
        .select_repository(&account.id, &repository.id, true)
        .await
        .unwrap();
    assert!(runtime.run_next().await);
    assert!(runtime.run_next().await);
    let repository = runtime
        .store
        .repository(&account.id, &repository.id)
        .await
        .unwrap();
    (account, repository)
}
async fn authored(
    runtime: &CollaborationRuntime,
    account: &RemoteAccount,
    text: &str,
) -> LocalDraft {
    runtime
        .save_draft(LocalDraft {
            account_id: account.id.clone(),
            subject_id: subject(RemoteItemKind::PullRequest),
            body: text.into(),
            generation: "0".into(),
        })
        .await
        .unwrap()
}
async fn hydrate(
    runtime: &CollaborationRuntime,
    account: &RemoteAccount,
    kind: RemoteItemKind,
) -> DetailSnapshot {
    runtime
        .hydrate_detail(demand(account, kind.clone()))
        .await
        .unwrap();
    assert!(runtime.run_next().await);
    runtime.store.detail(query(account, kind)).await.unwrap()
}

#[tokio::test]
async fn actual_gitlab_common_reads_capabilities_and_independent_body_survive_cold_sqlite_reopen() {
    let dir = tempfile::tempdir().unwrap();
    let database = store(dir.path()).await;
    let fixture = HttpFixture::new();
    let vault = Arc::new(Vault::default());
    let runtime =
        CollaborationRuntime::new(database.clone(), vault.clone(), fixture.provider.clone());
    let (account, repository) = selected(&runtime, "actor-a").await;
    let draft = authored(&runtime, &account, "private authored draft Δ").await;
    for kind in [RemoteItemKind::PullRequest, RemoteItemKind::Issue] {
        let list = database
            .query_items(items(&account, &repository, kind.clone()))
            .await
            .unwrap();
        assert_eq!(list.items.len(), 1);
        assert_eq!(list.items[0].id, subject(kind.clone()));
        assert_eq!(list.items[0].number.as_deref(), Some("67"));
        assert_eq!(list.items[0].state, "open");
        let missing = database
            .detail(query(&account, kind.clone()))
            .await
            .unwrap();
        assert_eq!(missing.evidence.availability, DetailAvailability::Missing);
        assert_eq!(missing.body.state, DetailValueState::NotLoaded);
        let saved = hydrate(&runtime, &account, kind.clone()).await;
        assert_eq!(saved.body.state, DetailValueState::Known);
        assert_eq!(saved.evidence.availability, DetailAvailability::Ready);
        assert_eq!(saved.evidence.coverage.state, CoverageState::Complete);
        let metadata = saved.metadata.unwrap();
        assert_eq!(metadata.values.state.as_deref(), Some("open"));
        assert_eq!(metadata.values.labels.len(), 2);
        assert_eq!(metadata.values.labels[0].provider_id, None);
        if kind == RemoteItemKind::PullRequest {
            assert_eq!(metadata.values.is_draft, Some(true));
            assert_eq!(metadata.values.head.as_ref().unwrap().oid, "a".repeat(40));
            assert!(
                metadata.values.base.is_none(),
                "async diff refs must not invent a base OID"
            );
        }
    }
    let before_refresh = database
        .detail(query(&account, RemoteItemKind::PullRequest))
        .await
        .unwrap();
    let refresh_calls = fixture.count();
    runtime.refresh(refresh(&account)).await.unwrap();
    for _ in 0..3 {
        assert!(runtime.run_next().await);
    }
    assert_eq!(
        fixture.count(),
        refresh_calls + 3,
        "account refresh admits discovery and both selected feeds"
    );
    let body = database
        .detail(query(&account, RemoteItemKind::PullRequest))
        .await
        .unwrap();
    assert_eq!(body.body, before_refresh.body);
    assert_eq!(
        body.evidence.facet_revision, before_refresh.evidence.facet_revision,
        "list refresh cannot validate or replace singleton Body"
    );
    let issue = database
        .detail(query(&account, RemoteItemKind::Issue))
        .await
        .unwrap();
    let calls = fixture.count();
    let loads = vault.loads.load(Ordering::SeqCst);
    database.close().await;
    drop(runtime);
    drop(database);
    let reopened = store(dir.path()).await;
    let runtime =
        CollaborationRuntime::new(reopened.clone(), vault.clone(), fixture.provider.clone());
    assert_eq!(
        reopened
            .detail(query(&account, RemoteItemKind::PullRequest))
            .await
            .unwrap(),
        body
    );
    assert_eq!(
        reopened
            .detail(query(&account, RemoteItemKind::Issue))
            .await
            .unwrap(),
        issue
    );
    assert_eq!(
        reopened
            .draft(&account.id, &draft.subject_id)
            .await
            .unwrap(),
        Some(draft)
    );
    assert!(
        reopened
            .repository(&account.id, &repository.id)
            .await
            .unwrap()
            .selected
    );
    let capabilities = runtime
        .contextual_capabilities(resource_target(&account, RemoteItemKind::PullRequest))
        .await
        .unwrap();
    let detail = capabilities
        .facets
        .iter()
        .find(|facet| facet.facet == ResourceFacet::PullDetails)
        .unwrap();
    assert_eq!(detail.saved_read.state, CapabilityState::Supported);
    assert_eq!(detail.observation, CapabilityObservation::Complete);
    assert_eq!(detail.remote_write.state, CapabilityState::Unsupported);
    for facet in [
        ResourceFacet::Comments,
        ResourceFacet::Reviews,
        ResourceFacet::Merge,
    ] {
        assert_eq!(
            capabilities
                .facets
                .iter()
                .find(|entry| entry.facet == facet)
                .unwrap()
                .saved_read
                .state,
            CapabilityState::Unsupported
        );
    }
    assert_eq!(
        capabilities
            .facets
            .iter()
            .find(|entry| entry.facet == ResourceFacet::Checks)
            .unwrap()
            .saved_read
            .state,
        CapabilityState::Supported
    );
    assert!(
        !runtime.run_next().await,
        "cold local reads do not admit provider work"
    );
    assert_eq!(fixture.count(), calls);
    assert_eq!(vault.loads.load(Ordering::SeqCst), loads);
    let calls = fixture.calls.lock().unwrap();
    assert!(calls.iter().any(|call| call.starts_with(&format!(
        "GET /api/v4/projects/{PROJECT}/merge_requests/{IID} HTTP/1.1"
    ))));
    assert!(
        !calls
            .iter()
            .any(|call| call.to_ascii_lowercase().contains("if-none-match:"))
    );
}

#[tokio::test]
async fn actual_gitlab_late_singleton_cannot_cross_replacement_epoch_or_other_actor_draft() {
    let dir = tempfile::tempdir().unwrap();
    let database = store(dir.path()).await;
    let fixture = HttpFixture::new();
    let vault = Arc::new(Vault::default());
    let runtime = CollaborationRuntime::new(database.clone(), vault, fixture.provider.clone());
    let (first, _) = selected(&runtime, "actor-a").await;
    let first_draft = authored(&runtime, &first, "actor A authored text").await;
    let (other, _) = selected(&runtime, "actor-b").await;
    let other_draft = authored(&runtime, &other, "actor B authored text").await;
    let other_body = hydrate(&runtime, &other, RemoteItemKind::PullRequest).await;
    assert_ne!(first.id, other.id);
    assert_eq!(first.login, other.login);
    fixture.held.active.store(true, Ordering::SeqCst);
    runtime
        .hydrate_detail(demand(&first, RemoteItemKind::PullRequest))
        .await
        .unwrap();
    let worker = runtime.clone();
    let pending = tokio::spawn(async move { worker.run_next().await });
    tokio::time::timeout(Duration::from_secs(2), fixture.held.entered.notified())
        .await
        .unwrap();
    let replacement = runtime
        .connect_gitlab("actor-a-replaced".into())
        .await
        .unwrap();
    assert_eq!(replacement.id, first.id);
    assert_eq!(replacement.authorization_epoch, "2");
    fixture.held.release();
    assert!(
        tokio::time::timeout(Duration::from_secs(2), pending)
            .await
            .unwrap()
            .unwrap()
    );
    let current = database
        .detail(query(&replacement, RemoteItemKind::PullRequest))
        .await
        .unwrap();
    assert_ne!(
        current.body.text.as_deref(),
        Some("obsolete old-epoch Body")
    );
    assert!(
        current.metadata.is_none(),
        "old singleton metadata must share the epoch fence"
    );
    assert!(
        database
            .scope_state(&replacement.id, "provider:rest")
            .await
            .unwrap()
            .is_none(),
        "old response quota must not mutate the replacement grant"
    );
    assert_eq!(
        database
            .detail(query(&other, RemoteItemKind::PullRequest))
            .await
            .unwrap()
            .body,
        other_body.body
    );
    assert_eq!(
        database
            .draft(&replacement.id, &first_draft.subject_id)
            .await
            .unwrap(),
        Some(first_draft)
    );
    assert_eq!(
        database
            .draft(&other.id, &other_draft.subject_id)
            .await
            .unwrap(),
        Some(other_draft)
    );
    assert_eq!(
        runtime
            .hydrate_detail(demand(&first, RemoteItemKind::PullRequest))
            .await
            .unwrap_err()
            .code,
        ErrorCode::StaleView
    );
}

#[tokio::test]
async fn actual_gitlab_concealed_singleton_denial_hides_only_that_facet_and_preserves_private_drafts()
 {
    let dir = tempfile::tempdir().unwrap();
    let database = store(dir.path()).await;
    let fixture = HttpFixture::new();
    let vault = Arc::new(Vault::default());
    let runtime = CollaborationRuntime::new(database.clone(), vault, fixture.provider.clone());
    let (first, repository) = selected(&runtime, "actor-a").await;
    let draft = authored(&runtime, &first, "private under concealed access loss").await;
    let body = hydrate(&runtime, &first, RemoteItemKind::PullRequest).await;
    let issue = hydrate(&runtime, &first, RemoteItemKind::Issue).await;
    let (other, _) = selected(&runtime, "actor-b").await;
    let other_body = hydrate(&runtime, &other, RemoteItemKind::PullRequest).await;
    fixture.denied.store(true, Ordering::SeqCst);
    // Admission is explicit; it must not validate the previously cached Body.
    let hidden = hydrate(&runtime, &first, RemoteItemKind::PullRequest).await;
    assert_eq!(
        hidden.evidence.availability,
        DetailAvailability::Unavailable
    );
    assert!(hidden.body.text.is_none());
    assert!(hidden.metadata.is_none());
    assert_eq!(
        database
            .query_items(items(&first, &repository, RemoteItemKind::PullRequest))
            .await
            .unwrap()
            .items
            .len(),
        1
    );
    assert_eq!(
        database
            .detail(query(&first, RemoteItemKind::Issue))
            .await
            .unwrap()
            .body,
        issue.body
    );
    assert_eq!(
        database
            .detail(query(&other, RemoteItemKind::PullRequest))
            .await
            .unwrap()
            .body,
        other_body.body
    );
    assert_eq!(
        database.draft(&first.id, &draft.subject_id).await.unwrap(),
        Some(draft)
    );
    let capability = runtime
        .contextual_capabilities(resource_target(&first, RemoteItemKind::PullRequest))
        .await
        .unwrap();
    let detail = capability
        .facets
        .iter()
        .find(|facet| facet.facet == ResourceFacet::PullDetails)
        .unwrap();
    assert_eq!(
        detail.saved_read.reason,
        Some(ContextCapabilityReason::PermissionDenied)
    );
    assert!(detail.can_recheck_access);
    assert_eq!(body.evidence.availability, DetailAvailability::Ready);
}

#[tokio::test]
async fn actual_gitlab_cross_project_issue_copy_keeps_native_identity_and_private_draft_generations()
 {
    let dir = tempfile::tempdir().unwrap();
    let database = store(dir.path()).await;
    let fixture = HttpFixture::new();
    let vault = Arc::new(Vault::default());
    let runtime =
        CollaborationRuntime::new(database.clone(), vault.clone(), fixture.provider.clone());
    let (account, original_repository) = selected(&runtime, "actor-a").await;
    let original_subject = subject(RemoteItemKind::Issue);
    let original_draft = runtime
        .save_draft(LocalDraft {
            account_id: account.id.clone(),
            subject_id: original_subject.clone(),
            body: "private original issue text Δ".into(),
            generation: "0".into(),
        })
        .await
        .unwrap();
    assert_eq!(original_draft.generation, "1");

    // A provider issue move is observed as a closed original and a copied issue
    // in a different project. Shared text and a project-local IID do not identify
    // either item or transfer private authored state.
    fixture.moved.store(true, Ordering::SeqCst);
    runtime.refresh(refresh(&account)).await.unwrap();
    for _ in 0..3 {
        assert!(runtime.run_next().await);
    }
    let target_repository = database
        .repositories(&account.id)
        .await
        .unwrap()
        .repositories
        .into_iter()
        .find(|repository| repository.provider_id == COPIED_PROJECT.to_string())
        .unwrap();
    assert!(!target_repository.selected);
    assert_ne!(target_repository.id, original_repository.id);
    runtime
        .select_repository(&account.id, &target_repository.id, true)
        .await
        .unwrap();
    assert!(runtime.run_next().await);
    assert!(runtime.run_next().await);

    let original = database
        .query_items(items(&account, &original_repository, RemoteItemKind::Issue))
        .await
        .unwrap()
        .items;
    let copied = database
        .query_items(items(&account, &target_repository, RemoteItemKind::Issue))
        .await
        .unwrap()
        .items;
    assert_eq!(original.len(), 1);
    assert_eq!(copied.len(), 1);
    let original = &original[0];
    let copied = &copied[0];
    let copied_subject = format!("gitlab:issue:{COPIED_ISSUE}");
    assert_eq!(original.id, original_subject);
    assert_eq!(copied.id, copied_subject);
    assert_ne!(original.id, copied.id);
    assert_eq!(
        original.repository_id.as_deref(),
        Some(original_repository.id.as_str())
    );
    assert_eq!(
        copied.repository_id.as_deref(),
        Some(target_repository.id.as_str())
    );
    assert_eq!(original.state, "closed");
    assert_eq!(copied.state, "open");
    assert_eq!(original.number, copied.number);
    assert_eq!(copied.number.as_deref(), Some("67"));
    assert_eq!(original.title, copied.title);
    assert_eq!(
        database.draft(&account.id, &original.id).await.unwrap(),
        Some(original_draft.clone())
    );
    assert_eq!(database.draft(&account.id, &copied.id).await.unwrap(), None);

    let mut copied_query = query(&account, RemoteItemKind::Issue);
    copied_query.subject_id = copied_subject.clone();
    let before = database.detail(copied_query.clone()).await.unwrap();
    assert_eq!(before.body.state, DetailValueState::NotLoaded);
    let mut copied_demand = demand(&account, RemoteItemKind::Issue);
    copied_demand.subject_id = copied_subject.clone();
    runtime.hydrate_detail(copied_demand).await.unwrap();
    assert!(runtime.run_next().await);
    let body = database.detail(copied_query.clone()).await.unwrap();
    assert_eq!(body.body.state, DetailValueState::Known);
    assert_eq!(
        body.body.text.as_deref(),
        Some("independent copied singleton Body λ")
    );
    let copied_draft = runtime
        .save_draft(LocalDraft {
            account_id: account.id.clone(),
            subject_id: copied_subject,
            body: "private copied issue text λ".into(),
            generation: "0".into(),
        })
        .await
        .unwrap();
    assert_eq!(copied_draft.generation, "1");
    let after_draft = database.detail(copied_query.clone()).await.unwrap();
    assert_eq!(after_draft.body, body.body);
    assert_eq!(after_draft.metadata, body.metadata);
    assert_eq!(
        after_draft.evidence.facet_revision,
        body.evidence.facet_revision
    );

    let calls = fixture.count();
    let loads = vault.loads.load(Ordering::SeqCst);
    database.close().await;
    drop(runtime);
    drop(database);
    let reopened = store(dir.path()).await;
    assert_eq!(reopened.detail(copied_query).await.unwrap(), after_draft);
    assert_eq!(
        reopened.draft(&account.id, &original.id).await.unwrap(),
        Some(original_draft)
    );
    assert_eq!(
        reopened.draft(&account.id, &copied.id).await.unwrap(),
        Some(copied_draft)
    );
    assert_eq!(fixture.count(), calls);
    assert_eq!(vault.loads.load(Ordering::SeqCst), loads);
    assert!(
        fixture
            .calls
            .lock()
            .unwrap()
            .iter()
            .any(|call| call.starts_with(&format!(
                "GET /api/v4/projects/{COPIED_PROJECT}/issues/{IID} HTTP/1.1"
            )))
    );
}
