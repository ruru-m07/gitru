//! Synthetic second-provider qualification; no production credential or API.
#[path = "gitlab_resource_reads_tests.rs"]
mod resource_reads_tests;

use super::*;
use crate::{
    credentials::CredentialError,
    providers::gitlab::tests::{project, response, server},
};
use async_trait::async_trait;
use std::sync::{Mutex as StdMutex, atomic::AtomicUsize};

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
    fn advance(&self, n: u64) {
        self.elapsed.fetch_add(n, Ordering::SeqCst);
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
async fn store(dir: &std::path::Path) -> Arc<Store> {
    Arc::new(Store::open(dir.join("synthetic.sqlite")).await.unwrap())
}
fn refresh(account: &RemoteAccount) -> RefreshRequest {
    RefreshRequest {
        account_id: account.id.clone(),
        repository_id: None,
        kind: None,
    }
}

struct ActorProvider {
    kind: ProviderKind,
    calls: AtomicUsize,
    held: Option<Arc<Notify>>,
    entered: Notify,
    error: StdMutex<Option<ProviderError>>,
}
impl ActorProvider {
    fn new(kind: ProviderKind) -> Self {
        Self {
            kind,
            calls: AtomicUsize::new(0),
            held: None,
            entered: Notify::new(),
            error: StdMutex::new(None),
        }
    }
}
#[async_trait]
impl CollaborationProvider for ActorProvider {
    fn kind(&self) -> ProviderKind {
        self.kind
    }
    fn profile(&self, _: &RemoteAccount) -> ProviderProfile {
        ProviderProfile {
            inbox_semantics: InboxSemantics::None,
            facets: providers::FACETS
                .into_iter()
                .map(|facet| FacetCapability {
                    facet,
                    state: if facet == ResourceFacet::Repositories {
                        CapabilityState::Supported
                    } else {
                        CapabilityState::Unsupported
                    },
                    reason: None,
                })
                .collect(),
        }
    }
    async fn probe(&self, token: &SecretToken) -> Result<VerifiedAccount, ProviderError> {
        if token.expose() == "denied" {
            return Err(ProviderError::new(ProviderErrorKind::Permission));
        }
        Ok(VerifiedAccount {
            actor_id: if token.expose() == "other" { "2" } else { "1" }.into(),
            login: if token.expose() == "renamed" {
                "renamed"
            } else {
                "same-login"
            }
            .into(),
            display_name: None,
            notifications_supported: false,
            cooldown_seconds: None,
        })
    }
    async fn fetch_page(
        &self,
        _: &SecretToken,
        request: FeedRequest,
    ) -> Result<FetchPage, ProviderError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.entered.notify_one();
        if let Some(held) = &self.held {
            held.notified().await;
        }
        if let Some(error) = self.error.lock().unwrap().clone() {
            return Err(error);
        }
        Ok(FetchPage {
            repositories: vec![RemoteRepository {
                id: format!("{:?}:repository:42", self.kind),
                account_id: request.account.id,
                provider_id: "42".into(),
                full_name: "group/sub/project".into(),
                name: "Project".into(),
                web_url: if self.kind == ProviderKind::Gitlab {
                    "https://gitlab.com/group/sub/project"
                } else {
                    "https://github.com/group/project"
                }
                .into(),
                description: None,
                default_branch: None,
                selected: false,
            }],
            items: vec![],
            endpoint_aliases: vec![],
            notification_subjects: vec![],
            next_cursor: None,
            etag: None,
            last_modified: None,
            not_modified: false,
            poll_interval_seconds: None,
            cooldown_seconds: None,
        })
    }
}

#[tokio::test]
async fn mixed_provider_actor_ids_and_login_renames_keep_exact_account_draft_and_vault_partitions()
{
    let dir = tempfile::tempdir().unwrap();
    let store = store(dir.path()).await;
    let vault = Arc::new(Vault::default());
    let gitlab = Arc::new(ActorProvider::new(ProviderKind::Gitlab));
    let github = Arc::new(ActorProvider::new(ProviderKind::Github));
    let mut registry = ProviderRegistry::default();
    registry.register(gitlab).unwrap();
    registry.register(github).unwrap();
    let runtime = CollaborationRuntime::with_registry(store.clone(), vault.clone(), registry);
    let first = runtime.connect_gitlab("first".into()).await.unwrap();
    let draft = runtime
        .save_draft(LocalDraft {
            account_id: first.id.clone(),
            subject_id: "gitlab:issue:123".into(),
            body: "private authored Δ".into(),
            generation: "0".into(),
        })
        .await
        .unwrap();
    let github = runtime.connect_github("first".into()).await.unwrap();
    let other = runtime.connect_gitlab("other".into()).await.unwrap();
    let old_ref = store
        .credential_reference(&first.id)
        .await
        .unwrap()
        .unwrap();
    let renamed = runtime.connect_gitlab("renamed".into()).await.unwrap();
    assert_eq!(first.id, renamed.id);
    assert_eq!(renamed.authorization_epoch, "2");
    assert_eq!(renamed.login, "renamed");
    assert_ne!(first.id, github.id);
    assert_ne!(first.id, other.id);
    assert_eq!(first.actor_id, github.actor_id);
    assert_eq!(store.accounts().await.unwrap().accounts.len(), 3);
    assert_eq!(
        store.draft(&renamed.id, &draft.subject_id).await.unwrap(),
        Some(draft.clone())
    );
    assert!(
        store
            .draft(&github.id, &draft.subject_id)
            .await
            .unwrap()
            .is_none()
    );
    assert!(!vault.tokens.lock().unwrap().contains_key(&old_ref));
    assert_eq!(vault.tokens.lock().unwrap().len(), 3);
    assert_eq!(
        runtime
            .capabilities(&renamed.id)
            .await
            .unwrap()
            .inbox_semantics,
        InboxSemantics::None
    );
    let deadline = runtime.future_string(3600);
    store
        .set_sync_status(
            &renamed.id,
            &renamed.authorization_epoch,
            "provider:rest",
            SyncStatus {
                state: SyncState::RateLimited,
                last_success_at: None,
                next_retry_at: Some(deadline.clone()),
                error: None,
            },
        )
        .await
        .unwrap();
    assert!(runtime.connect_gitlab("denied".into()).await.is_err());
    assert_eq!(store.account(&renamed.id).await.unwrap(), renamed);
    assert_eq!(vault.tokens.lock().unwrap().len(), 3);
    assert_eq!(
        store
            .scope_state(&renamed.id, "provider:rest")
            .await
            .unwrap()
            .unwrap()
            .sync
            .next_retry_at,
        Some(deadline)
    );
}

#[tokio::test]
async fn actual_gitlab_failed_operation_probes_never_stage_a_secret_or_account() {
    for (first, second) in [
        (response(401, "", "private"), None),
        (
            response(200, "", r#"{"id":1,"username":"actor"}"#),
            Some(response(403, "", "private")),
        ),
        (
            response(200, "", r#"{"id":1,"username":"actor"}"#),
            Some(response(429, "Retry-After: 90\r\n", "private")),
        ),
        (
            response(200, "", r#"{"id":1,"username":"actor"}"#),
            Some(response(200, "", "invalid-json")),
        ),
    ] {
        let dir = tempfile::tempdir().unwrap();
        let store = store(dir.path()).await;
        let vault = Arc::new(Vault::default());
        let (provider, task) = server(|_| std::iter::once(first).chain(second).collect());
        let runtime = CollaborationRuntime::new(store.clone(), vault.clone(), Arc::new(provider));
        let error = runtime
            .connect_gitlab("synthetic_token".into())
            .await
            .unwrap_err();
        assert!(!error.message.contains("private"));
        assert!(!error.message.contains("synthetic_token"));
        assert!(store.accounts().await.unwrap().accounts.is_empty());
        assert!(
            store
                .due_credential_cleanup(i64::MAX, 32)
                .await
                .unwrap()
                .is_empty()
        );
        assert_eq!(vault.stores.load(Ordering::SeqCst), 0);
        task.join().unwrap();
    }
}

#[tokio::test]
async fn probe_quota_commits_before_refresh_and_survives_cold_runtime_restart() {
    let dir = tempfile::tempdir().unwrap();
    let database = store(dir.path()).await;
    let vault = Arc::new(Vault::default());
    let (provider, task) = server(|_| {
        vec![
            response(200, "", r#"{"id":1,"username":"actor"}"#),
            response(200, "RateLimit-Remaining: 0\r\n", "[]"),
        ]
    });
    let provider = Arc::new(provider);
    let runtime = CollaborationRuntime::new(database.clone(), vault.clone(), provider.clone());
    let account = runtime
        .connect_gitlab("synthetic_token".into())
        .await
        .unwrap();
    assert!(
        database
            .repositories(&account.id)
            .await
            .unwrap()
            .repositories
            .is_empty()
    );
    assert!(
        database
            .scope_state(&account.id, "repositories")
            .await
            .unwrap()
            .is_none()
    );
    let quota = database
        .scope_state(&account.id, "provider:rest")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(quota.sync.state, SyncState::RateLimited);
    assert!(quota.sync.next_retry_at.is_some());
    // Admission loads the committed account barrier before the ready picker.
    assert!(!runtime.run_next().await);
    assert!(!runtime.run_next().await);
    assert_eq!(vault.loads.load(Ordering::SeqCst), 0);
    assert_eq!(task.join().unwrap().len(), 2);
    database.close().await.unwrap();
    drop(runtime);
    drop(database);
    let reopened = store(dir.path()).await;
    let runtime = CollaborationRuntime::new(reopened.clone(), vault.clone(), provider);
    runtime.refresh(refresh(&account)).await.unwrap();
    assert!(!runtime.run_next().await);
    assert!(!runtime.run_next().await);
    assert_eq!(vault.loads.load(Ordering::SeqCst), 0);
    assert_eq!(
        reopened
            .scope_state(&account.id, "provider:rest")
            .await
            .unwrap()
            .unwrap()
            .sync
            .next_retry_at,
        quota.sync.next_retry_at
    );
}

#[tokio::test]
async fn actual_keyset_resume_rename_selection_and_complete_absence_use_shared_storage_policy() {
    let dir = tempfile::tempdir().unwrap();
    let database = store(dir.path()).await;
    let vault = Arc::new(Vault::default());
    let (provider, task) = server(|base| {
        vec![
            response(200, "", r#"{"id":1,"username":"actor"}"#),
            response(200, "", "[]"),
            response(
                200,
                &format!(
                    "Link: <{base}projects?membership=true&pagination=keyset&order_by=id&sort=asc&per_page=50&id_after=1>; rel=\"next\"\r\n"
                ),
                &serde_json::json!([project(1, "group/sub/project")]).to_string(),
            ),
            response(
                200,
                "",
                &serde_json::json!([project(2, "group/second")]).to_string(),
            ),
            // Selecting a repository now admits its actual common MR/issue
            // feeds. Drain those before this repository-only reconciliation.
            response(200, "", "[]"),
            response(200, "", "[]"),
            response(
                200,
                "",
                &serde_json::json!([
                    project(1, "renamed/sub/project"),
                    project(2, "group/second")
                ])
                .to_string(),
            ),
            response(200, "", "[]"),
            response(200, "", "[]"),
        ]
    });
    let provider = Arc::new(provider);
    let runtime = CollaborationRuntime::new(database.clone(), vault.clone(), provider.clone());
    let account = runtime
        .connect_gitlab("synthetic_token".into())
        .await
        .unwrap();
    assert!(runtime.run_next().await);
    let partial = database
        .scope_state(&account.id, "repositories")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(partial.coverage.state, CoverageState::Partial);
    assert!(partial.next_cursor.is_some());
    database.close().await.unwrap();
    drop(runtime);
    drop(database);
    let database = store(dir.path()).await;
    let runtime = CollaborationRuntime::new(database.clone(), vault, provider);
    runtime.refresh(refresh(&account)).await.unwrap();
    assert!(runtime.run_next().await);
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
        database
            .scope_state(&account.id, "repositories")
            .await
            .unwrap()
            .unwrap()
            .coverage
            .state,
        CoverageState::Complete
    );
    let repository = database
        .repositories(&account.id)
        .await
        .unwrap()
        .repositories
        .into_iter()
        .find(|r| r.provider_id == "1")
        .unwrap();
    runtime
        .select_repository(&account.id, &repository.id, true)
        .await
        .unwrap();
    assert!(runtime.run_next().await);
    assert!(runtime.run_next().await);
    let before = database
        .scope_state(&account.id, "repositories")
        .await
        .unwrap()
        .unwrap();
    for kind in ["pull_request", "issue"] {
        let scope = database
            .scope_state(&account.id, &format!("repo:{}:{kind}", repository.id))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(scope.coverage.state, CoverageState::Complete);
    }
    runtime
        .enqueue(account.clone(), None, FeedKind::Repositories, true)
        .await
        .unwrap();
    assert!(runtime.run_next().await);
    let renamed = database
        .repository(&account.id, &repository.id)
        .await
        .unwrap();
    assert!(renamed.selected);
    assert_eq!(renamed.full_name, "renamed/sub/project");
    assert_eq!(renamed.provider_id, "1");
    assert_eq!(before.coverage.state, CoverageState::Complete);
    runtime
        .enqueue(account.clone(), None, FeedKind::Repositories, true)
        .await
        .unwrap();
    assert!(runtime.run_next().await);
    assert_eq!(
        database
            .repositories(&account.id)
            .await
            .unwrap()
            .repositories
            .len(),
        2,
        "one complete absence is not retirement"
    );
    runtime
        .enqueue(account.clone(), None, FeedKind::Repositories, true)
        .await
        .unwrap();
    assert!(runtime.run_next().await);
    assert!(
        database
            .repositories(&account.id)
            .await
            .unwrap()
            .repositories
            .is_empty()
    );
    assert!(
        database
            .repository(&account.id, &repository.id)
            .await
            .is_err()
    );
    let calls = task.join().unwrap();
    assert_eq!(calls.len(), 9);
    assert!(calls[3].contains("id_after=1"));
    assert!(calls[4].starts_with("GET /api/v4/projects/1/merge_requests?"));
    assert!(calls[5].starts_with("GET /api/v4/projects/1/issues?"));
    assert!(calls[6].starts_with("GET /api/v4/projects?"));
}

#[tokio::test]
async fn successful_invalid_feed_quota_survives_error_and_manual_refresh_cannot_reset_it() {
    let dir = tempfile::tempdir().unwrap();
    let database = store(dir.path()).await;
    let vault = Arc::new(Vault::default());
    let provider = Arc::new(ActorProvider::new(ProviderKind::Gitlab));
    let mut error = ProviderError::new(ProviderErrorKind::InvalidResponse);
    error.account_cooldown_seconds = Some(120);
    *provider.error.lock().unwrap() = Some(error);
    let clock = Clock::new();
    let mut runtime = CollaborationRuntime::new(database.clone(), vault, provider.clone());
    runtime.clock = clock.clone();
    let account = runtime
        .connect_gitlab("synthetic_token".into())
        .await
        .unwrap();
    assert!(runtime.run_next().await);
    assert_eq!(provider.calls.load(Ordering::SeqCst), 1);
    let barrier = database
        .scope_state(&account.id, "provider:rest")
        .await
        .unwrap()
        .unwrap()
        .sync
        .next_retry_at;
    runtime.refresh(refresh(&account)).await.unwrap();
    assert!(!runtime.run_next().await);
    assert_eq!(provider.calls.load(Ordering::SeqCst), 1);
    assert_eq!(
        database
            .scope_state(&account.id, "provider:rest")
            .await
            .unwrap()
            .unwrap()
            .sync
            .next_retry_at,
        barrier
    );
    clock.advance(121);
    *provider.error.lock().unwrap() = None;
    assert!(runtime.run_next().await);
    assert_eq!(provider.calls.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn delayed_gitlab_read_cannot_cross_replacement_epoch_or_mutate_its_quota() {
    let dir = tempfile::tempdir().unwrap();
    let database = store(dir.path()).await;
    let vault = Arc::new(Vault::default());
    let release = Arc::new(Notify::new());
    let provider = Arc::new(ActorProvider {
        held: Some(release.clone()),
        ..ActorProvider::new(ProviderKind::Gitlab)
    });
    let runtime = CollaborationRuntime::new(database.clone(), vault, provider.clone());
    let old = runtime.connect_gitlab("first".into()).await.unwrap();
    let worker = runtime.clone();
    let read = tokio::spawn(async move { worker.run_next().await });
    provider.entered.notified().await;
    let replacement = runtime.connect_gitlab("renamed".into()).await.unwrap();
    assert_eq!(replacement.id, old.id);
    assert_eq!(replacement.authorization_epoch, "2");
    let mut error = ProviderError::new(ProviderErrorKind::InvalidResponse);
    error.account_cooldown_seconds = Some(120);
    *provider.error.lock().unwrap() = Some(error);
    release.notify_one();
    assert!(read.await.unwrap());
    assert!(
        database
            .repositories(&replacement.id)
            .await
            .unwrap()
            .repositories
            .is_empty()
    );
    assert!(
        database
            .scope_state(&replacement.id, "provider:rest")
            .await
            .unwrap()
            .is_none()
    );
    assert_eq!(
        database.account(&replacement.id).await.unwrap(),
        replacement
    );
}

#[tokio::test]
async fn unrepresentable_quota_never_panics_or_becomes_a_short_fallback_retry() {
    let dir = tempfile::tempdir().unwrap();
    let database = store(dir.path()).await;
    let vault = Arc::new(Vault::default());
    let provider = Arc::new(ActorProvider::new(ProviderKind::Gitlab));
    let mut error = ProviderError::new(ProviderErrorKind::RateLimited);
    error.account_cooldown_seconds = Some(u64::MAX);
    error.retry_after_seconds = Some(u64::MAX);
    *provider.error.lock().unwrap() = Some(error);
    let clock = Clock::new();
    let mut runtime = CollaborationRuntime::new(database.clone(), vault, provider.clone());
    runtime.clock = clock.clone();
    let account = runtime
        .connect_gitlab("synthetic_token".into())
        .await
        .unwrap();
    assert!(runtime.run_next().await);
    let quota = database
        .scope_state(&account.id, "provider:rest")
        .await
        .unwrap()
        .unwrap();
    let deadline =
        DateTime::parse_from_rfc3339(quota.sync.next_retry_at.as_deref().unwrap()).unwrap();
    assert_eq!(
        deadline.with_timezone(&Utc),
        DateTime::<Utc>::from_timestamp(253_402_300_799, 999_000_000).unwrap(),
        "native evidence saturates at the durable clock boundary"
    );
    clock.advance(172801);
    runtime.refresh(refresh(&account)).await.unwrap();
    for _ in 0..2 {
        let _ = runtime.run_next().await;
    }
    assert_eq!(
        provider.calls.load(Ordering::SeqCst),
        1,
        "bounded wakeups still consult the full persisted deadline"
    );
}

#[tokio::test]
async fn unavailable_retry_after_survives_manual_admission_and_runtime_restart() {
    let dir = tempfile::tempdir().unwrap();
    let database = store(dir.path()).await;
    let vault = Arc::new(Vault::default());
    let provider = Arc::new(ActorProvider::new(ProviderKind::Gitlab));
    let mut error = ProviderError::new(ProviderErrorKind::Unavailable);
    error.retry_after_seconds = Some(172800);
    *provider.error.lock().unwrap() = Some(error);
    let clock = Clock::new();
    let mut runtime = CollaborationRuntime::new(database.clone(), vault.clone(), provider.clone());
    runtime.clock = clock.clone();
    let account = runtime
        .connect_gitlab("synthetic_token".into())
        .await
        .unwrap();
    assert!(runtime.run_next().await);
    assert_eq!(provider.calls.load(Ordering::SeqCst), 1);
    let deadline = database
        .scope_state(&account.id, "repositories")
        .await
        .unwrap()
        .unwrap()
        .sync
        .next_retry_at;
    runtime.refresh(refresh(&account)).await.unwrap();
    assert!(!runtime.run_next().await);
    database.close().await.unwrap();
    drop(runtime);
    drop(database);
    let database = store(dir.path()).await;
    let mut runtime = CollaborationRuntime::new(database.clone(), vault, provider.clone());
    runtime.clock = clock.clone();
    runtime.refresh(refresh(&account)).await.unwrap();
    clock.advance(86401);
    for _ in 0..2 {
        let _ = runtime.run_next().await;
    }
    assert_eq!(provider.calls.load(Ordering::SeqCst), 1);
    assert_eq!(
        database
            .scope_state(&account.id, "repositories")
            .await
            .unwrap()
            .unwrap()
            .sync
            .next_retry_at,
        deadline
    );
}

#[tokio::test]
async fn failed_replacement_probe_preserves_proven_actor_backoff_without_promoting_or_revoking_grant()
 {
    for (status, headers, body) in [
        (429, "Retry-After: 120\r\n", "private"),
        (503, "Retry-After: 172800\r\n", "private"),
        (200, "RateLimit-Remaining: 0\r\n", "invalid-json"),
    ] {
        let dir = tempfile::tempdir().unwrap();
        let database = store(dir.path()).await;
        let vault = Arc::new(Vault::default());
        let (provider, task) = server(|_| {
            vec![
                response(200, "", r#"{"id":1,"username":"actor"}"#),
                response(200, "", "[]"),
                response(200, "", r#"{"id":1,"username":"actor"}"#),
                response(status, headers, body),
            ]
        });
        let provider = Arc::new(provider);
        let runtime = CollaborationRuntime::new(database.clone(), vault.clone(), provider.clone());
        let account = runtime.connect_gitlab("initial".into()).await.unwrap();
        let reference = database.credential_reference(&account.id).await.unwrap();
        let draft = runtime
            .save_draft(LocalDraft {
                account_id: account.id.clone(),
                subject_id: "gitlab:issue:5".into(),
                body: "retained private Δ".into(),
                generation: "0".into(),
            })
            .await
            .unwrap();
        let error = runtime
            .connect_gitlab("prospective-replacement".into())
            .await
            .unwrap_err();
        assert_eq!(
            error.code,
            if status == 429 {
                ErrorCode::RateLimited
            } else {
                ErrorCode::Provider
            }
        );
        assert_eq!(database.account(&account.id).await.unwrap(), account);
        assert_eq!(
            database.credential_reference(&account.id).await.unwrap(),
            reference
        );
        assert_eq!(
            database
                .draft(&account.id, &draft.subject_id)
                .await
                .unwrap(),
            Some(draft)
        );
        assert_eq!(vault.stores.load(Ordering::SeqCst), 1);
        let quota = database
            .scope_state(&account.id, "provider:rest")
            .await
            .unwrap();
        assert!(
            quota.is_some(),
            "a proven old actor retains newly observed backoff after failed prospective probe"
        );
        for _ in 0..2 {
            let _ = runtime.run_next().await;
        }
        assert_eq!(vault.loads.load(Ordering::SeqCst), 0);
        assert_eq!(task.join().unwrap().len(), 4);
        database.close().await.unwrap();
        drop(runtime);
        drop(database);
        let database = store(dir.path()).await;
        let runtime = CollaborationRuntime::new(database.clone(), vault.clone(), provider);
        runtime.refresh(refresh(&account)).await.unwrap();
        for _ in 0..2 {
            let _ = runtime.run_next().await;
        }
        assert_eq!(vault.loads.load(Ordering::SeqCst), 0);
    }
}

#[tokio::test]
async fn concurrent_provider_budget_observations_preserve_maximum_and_reject_old_epochs() {
    let directory = tempfile::tempdir().unwrap();
    let database = store(directory.path()).await;
    let vault = Arc::new(Vault::default());
    let provider = Arc::new(ActorProvider::new(ProviderKind::Gitlab));
    let runtime = CollaborationRuntime::new(database.clone(), vault, provider);
    let account = runtime
        .connect_gitlab("synthetic_token".into())
        .await
        .unwrap();
    let success = runtime.now_string();
    database
        .set_sync_status(
            &account.id,
            &account.authorization_epoch,
            "provider:rest",
            SyncStatus {
                state: SyncState::Idle,
                last_success_at: Some(success.clone()),
                next_retry_at: None,
                error: None,
            },
        )
        .await
        .unwrap();
    let long = runtime.future_string(172800);
    let short = runtime.future_string(60);
    let (a, b) = tokio::join!(
        database.merge_provider_budget(
            &account.id,
            &account.authorization_epoch,
            long.clone(),
            None
        ),
        database.merge_provider_budget(&account.id, &account.authorization_epoch, short, None),
    );
    a.unwrap();
    b.unwrap();
    let budget = database
        .scope_state(&account.id, "provider:rest")
        .await
        .unwrap()
        .unwrap()
        .sync;
    assert_eq!(budget.next_retry_at, Some(long.clone()));
    assert_eq!(budget.last_success_at, Some(success));
    assert!(budget.error.is_none());
    let replacement = runtime
        .connect_gitlab("synthetic_replacement_token".into())
        .await
        .unwrap();
    let revision = database.revision().await.unwrap();
    assert_eq!(
        database
            .merge_provider_budget(
                &account.id,
                &account.authorization_epoch,
                runtime.future_string(360000),
                None
            )
            .await
            .unwrap_err()
            .code,
        ErrorCode::StaleView
    );
    assert_eq!(database.revision().await.unwrap(), revision);
    assert_eq!(
        database
            .scope_state(&replacement.id, "provider:rest")
            .await
            .unwrap()
            .unwrap()
            .sync
            .next_retry_at,
        Some(long)
    );
    assert!(
        database
            .merge_provider_budget(
                &replacement.id,
                &replacement.authorization_epoch,
                "+262142-12-31T23:59:59Z".into(),
                None
            )
            .await
            .is_err()
    );
    assert_eq!(database.revision().await.unwrap(), revision);
}
