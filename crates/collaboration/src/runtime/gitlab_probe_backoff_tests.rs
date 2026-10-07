//! Independent prospective-token failure tests using only synthetic actors/HTTP.
use super::*;
use crate::{
    credentials::CredentialError,
    providers::gitlab::tests::{response, server},
};
use async_trait::async_trait;
use std::sync::{Mutex as StdMutex, atomic::AtomicUsize};

const ACTOR: &str = "9007199254740993";

#[derive(Default)]
struct Vault {
    tokens: StdMutex<HashMap<String, SecretToken>>,
    stores: AtomicUsize,
    loads: AtomicUsize,
    deletes: AtomicUsize,
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
        self.deletes.fetch_add(1, Ordering::SeqCst);
        self.tokens.lock().unwrap().remove(key);
        Ok(())
    }
}

fn account(provider: ProviderKind, host: &str, actor: &str) -> RemoteAccount {
    RemoteAccount {
        id: "existing-account".into(),
        provider,
        host: host.into(),
        actor_id: actor.into(),
        login: "same-mutable-login".into(),
        display_name: Some("Old account 名前".into()),
        authorization_epoch: "7".into(),
        state: AccountState::Active,
        notifications_supported: false,
    }
}
fn refresh(account: &RemoteAccount) -> RefreshRequest {
    RefreshRequest {
        account_id: account.id.clone(),
        repository_id: None,
        kind: None,
    }
}
fn user(actor: &str) -> String {
    format!(r#"{{"id":{actor},"username":"same-mutable-login","state":"active"}}"#)
}

struct ExistingGrant {
    account: RemoteAccount,
    reference: String,
    repositories: Vec<RemoteRepository>,
    draft: LocalDraft,
    authorization_view: String,
}
async fn seed(store: &Store, vault: &Vault, account: RemoteAccount) -> ExistingGrant {
    let reference = "credential:synthetic-old-grant".to_string();
    store
        .stage_credential(&account.id, &reference)
        .await
        .unwrap();
    vault
        .store(
            &reference,
            &SecretToken::new("synthetic_old_token".into()).unwrap(),
        )
        .unwrap();
    let account = store
        .commit_account_credential(account, &reference)
        .await
        .unwrap();
    let repository = RemoteRepository {
        id: "gitlab:repository:42".into(),
        account_id: account.id.clone(),
        provider_id: "42".into(),
        full_name: "group/sub/project".into(),
        name: "Cached project Δ".into(),
        web_url: format!("https://{}/group/sub/project", account.host),
        description: Some("Retained provider description".into()),
        default_branch: Some("main".into()),
        selected: true,
    };
    let run_id = store
        .begin_sync(&account.id, &account.authorization_epoch, "repositories")
        .await
        .unwrap();
    store
        .apply_page(PageCommit {
            account_id: account.id.clone(),
            authorization_epoch: account.authorization_epoch.clone(),
            scope: "repositories".into(),
            run_id,
            repositories: vec![repository.clone()],
            items: vec![],
            endpoint_aliases: vec![],
            next_cursor: None,
            etag: Some("saved-validator".into()),
            last_modified: None,
            not_modified: false,
            complete: true,
            observed_at: Utc::now().to_rfc3339(),
        })
        .await
        .unwrap();
    let draft = store
        .save_draft(LocalDraft {
            account_id: account.id.clone(),
            subject_id: "private-unresolved-subject".into(),
            body: "Saved private draft 日本語 Δ\n".into(),
            generation: "0".into(),
        })
        .await
        .unwrap();
    ExistingGrant {
        account,
        reference,
        repositories: vec![repository],
        draft,
        authorization_view: store.accounts().await.unwrap().authorization_view,
    }
}

async fn preserved(store: &Store, vault: &Vault, old: &ExistingGrant) {
    assert_eq!(store.account(&old.account.id).await.unwrap(), old.account);
    assert_eq!(
        store.credential_reference(&old.account.id).await.unwrap(),
        Some(old.reference.clone())
    );
    assert_eq!(
        store
            .repositories(&old.account.id)
            .await
            .unwrap()
            .repositories,
        old.repositories
    );
    assert_eq!(
        store
            .draft(&old.account.id, &old.draft.subject_id)
            .await
            .unwrap(),
        Some(old.draft.clone())
    );
    assert_eq!(
        store.accounts().await.unwrap().authorization_view,
        old.authorization_view
    );
    assert!(
        store
            .due_credential_cleanup(i64::MAX, 32)
            .await
            .unwrap()
            .is_empty()
    );
    assert_eq!(vault.stores.load(Ordering::SeqCst), 1);
    assert_eq!(vault.deletes.load(Ordering::SeqCst), 0);
    assert_eq!(vault.tokens.lock().unwrap().len(), 1);
    assert!(vault.tokens.lock().unwrap().contains_key(&old.reference));
}

#[tokio::test]
async fn actual_partial_probe_preserves_old_grant_cache_draft_and_restart_backoff() {
    for (status, headers, body, minimum_seconds) in [
        (429, "Retry-After: 120\r\n", "private fixture", 120),
        (503, "Retry-After: 172800\r\n", "private fixture", 172800),
        (200, "RateLimit-Remaining: 0\r\n", "invalid-json", 60),
    ] {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("independent.sqlite");
        let database = Arc::new(Store::open(&path).await.unwrap());
        let vault = Arc::new(Vault::default());
        let old = seed(
            &database,
            &vault,
            account(ProviderKind::Gitlab, "gitlab.com", ACTOR),
        )
        .await;
        let (provider, task) = server(|_| {
            vec![
                response(200, "", &user(ACTOR)),
                response(status, headers, body),
            ]
        });
        let provider = Arc::new(provider);
        let runtime = CollaborationRuntime::new(database.clone(), vault.clone(), provider.clone());
        let before = Utc::now();
        let error = runtime
            .connect_gitlab("synthetic_prospective_token".into())
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
        preserved(&database, &vault, &old).await;
        let quota = database
            .scope_state(&old.account.id, "provider:rest")
            .await
            .unwrap()
            .expect("verified actor quota must survive prospective-token rejection");
        assert_eq!(quota.sync.state, SyncState::RateLimited);
        assert!(
            quota.sync.error.is_none(),
            "prospective failure cannot revoke old authorized reads"
        );
        let deadline = quota.sync.next_retry_at.clone().unwrap();
        let until = DateTime::parse_from_rfc3339(&deadline).unwrap();
        assert!(
            until
                >= before + chrono::Duration::seconds(minimum_seconds)
                    - chrono::Duration::milliseconds(1)
        );
        runtime.refresh(refresh(&old.account)).await.unwrap();
        let _ = runtime.run_next().await;
        assert_eq!(vault.loads.load(Ordering::SeqCst), 0);
        let requests = task.join().unwrap();
        assert_eq!(requests.len(), 2);
        assert!(requests[0].starts_with("GET /api/v4/user "));
        assert!(requests[1].starts_with("GET /api/v4/projects?"));
        database.close().await.unwrap();
        drop(runtime);
        drop(database);
        let reopened = Arc::new(Store::open(&path).await.unwrap());
        let restarted = CollaborationRuntime::new(reopened.clone(), vault.clone(), provider);
        restarted.refresh(refresh(&old.account)).await.unwrap();
        let _ = restarted.run_next().await;
        assert_eq!(
            vault.loads.load(Ordering::SeqCst),
            0,
            "restart/manual refresh cannot bypass the durable deadline"
        );
        assert_eq!(
            reopened
                .scope_state(&old.account.id, "provider:rest")
                .await
                .unwrap()
                .unwrap()
                .sync
                .next_retry_at,
            Some(deadline)
        );
        preserved(&reopened, &vault, &old).await;
    }
}

#[tokio::test]
async fn actual_partial_probe_cannot_shorten_an_existing_longer_deadline() {
    let dir = tempfile::tempdir().unwrap();
    let database = Arc::new(
        Store::open(dir.path().join("independent.sqlite"))
            .await
            .unwrap(),
    );
    let vault = Arc::new(Vault::default());
    let old = seed(
        &database,
        &vault,
        account(ProviderKind::Gitlab, "gitlab.com", ACTOR),
    )
    .await;
    let (provider, task) = server(|_| {
        vec![
            response(200, "", &user(ACTOR)),
            response(429, "Retry-After: 120\r\n", "fixture"),
        ]
    });
    let runtime = CollaborationRuntime::new(database.clone(), vault.clone(), Arc::new(provider));
    let existing_deadline = runtime.future_string(3600);
    database
        .set_sync_status(
            &old.account.id,
            &old.account.authorization_epoch,
            "provider:rest",
            SyncStatus {
                state: SyncState::RateLimited,
                last_success_at: None,
                next_retry_at: Some(existing_deadline.clone()),
                error: None,
            },
        )
        .await
        .unwrap();
    assert_eq!(
        runtime
            .connect_gitlab("synthetic_prospective_token".into())
            .await
            .unwrap_err()
            .code,
        ErrorCode::RateLimited
    );
    assert_eq!(
        database
            .scope_state(&old.account.id, "provider:rest")
            .await
            .unwrap()
            .unwrap()
            .sync
            .next_retry_at,
        Some(existing_deadline)
    );
    preserved(&database, &vault, &old).await;
    assert_eq!(task.join().unwrap().len(), 2);
}

#[tokio::test]
async fn actual_partial_probe_never_assigns_quota_to_a_matching_login_or_other_installation() {
    for (kind, host, stored_actor) in [
        (ProviderKind::Gitlab, "gitlab.com", "2"),
        (ProviderKind::Github, "github.com", ACTOR),
        (ProviderKind::Gitlab, "gitlab.example.test", ACTOR),
    ] {
        let dir = tempfile::tempdir().unwrap();
        let database = Arc::new(
            Store::open(dir.path().join("independent.sqlite"))
                .await
                .unwrap(),
        );
        let vault = Arc::new(Vault::default());
        let old = seed(&database, &vault, account(kind, host, stored_actor)).await;
        let revision = database.accounts().await.unwrap().revision;
        let (provider, task) = server(|_| {
            vec![
                response(200, "", &user(ACTOR)),
                response(429, "Retry-After: 120\r\n", "fixture"),
            ]
        });
        let runtime =
            CollaborationRuntime::new(database.clone(), vault.clone(), Arc::new(provider));
        assert_eq!(
            runtime
                .connect_gitlab("synthetic_prospective_token".into())
                .await
                .unwrap_err()
                .code,
            ErrorCode::RateLimited
        );
        assert!(
            database
                .scope_state(&old.account.id, "provider:rest")
                .await
                .unwrap()
                .is_none()
        );
        assert_eq!(database.accounts().await.unwrap().revision, revision);
        preserved(&database, &vault, &old).await;
        assert_eq!(task.join().unwrap().len(), 2);
    }
}

#[tokio::test]
async fn actual_unproven_user_response_cannot_poison_a_known_actor_or_stage_a_token() {
    for (status, body) in [
        (401, "unauthorized fixture"),
        (429, "rate limited fixture"),
        (200, "invalid-json"),
        (
            200,
            r#"{"id":9007199254740993,"username":"same-mutable-login","state":"blocked"}"#,
        ),
    ] {
        let dir = tempfile::tempdir().unwrap();
        let database = Arc::new(
            Store::open(dir.path().join("independent.sqlite"))
                .await
                .unwrap(),
        );
        let vault = Arc::new(Vault::default());
        let old = seed(
            &database,
            &vault,
            account(ProviderKind::Gitlab, "gitlab.com", ACTOR),
        )
        .await;
        let revision = database.accounts().await.unwrap().revision;
        let (provider, task) = server(|_| {
            vec![response(
                status,
                "Retry-After: 120\r\nRateLimit-Remaining: 0\r\n",
                body,
            )]
        });
        let runtime =
            CollaborationRuntime::new(database.clone(), vault.clone(), Arc::new(provider));
        assert!(
            runtime
                .connect_gitlab("synthetic_prospective_token".into())
                .await
                .is_err()
        );
        assert!(
            database
                .scope_state(&old.account.id, "provider:rest")
                .await
                .unwrap()
                .is_none()
        );
        assert_eq!(database.accounts().await.unwrap().revision, revision);
        preserved(&database, &vault, &old).await;
        assert_eq!(task.join().unwrap().len(), 1);
    }
}

#[tokio::test]
async fn verified_inactive_actor_retains_backoff_without_reactivation_or_credential_staging() {
    for state in [AccountState::Disconnected, AccountState::AuthRequired] {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("independent.sqlite");
        let database = Arc::new(Store::open(&path).await.unwrap());
        let vault = Arc::new(Vault::default());
        let mut old = account(ProviderKind::Gitlab, "gitlab.com", ACTOR);
        old.state = state;
        database.upsert_account(old.clone()).await.unwrap();
        let draft = database
            .save_draft(LocalDraft {
                account_id: old.id.clone(),
                subject_id: "private-inactive-subject".into(),
                body: "Inactive private text 日本語".into(),
                generation: "0".into(),
            })
            .await
            .unwrap();
        let authorization_view = database.accounts().await.unwrap().authorization_view;
        let (provider, task) = server(|_| {
            vec![
                response(200, "", &user(ACTOR)),
                response(503, "Retry-After: 120\r\n", "fixture"),
            ]
        });
        let runtime =
            CollaborationRuntime::new(database.clone(), vault.clone(), Arc::new(provider));
        assert_eq!(
            runtime
                .connect_gitlab("synthetic_prospective_token".into())
                .await
                .unwrap_err()
                .code,
            ErrorCode::Provider
        );
        assert_eq!(database.account(&old.id).await.unwrap(), old);
        assert_eq!(
            database.accounts().await.unwrap().authorization_view,
            authorization_view
        );
        assert_eq!(
            database.draft(&old.id, &draft.subject_id).await.unwrap(),
            Some(draft)
        );
        assert!(
            database
                .credential_reference(&old.id)
                .await
                .unwrap()
                .is_none()
        );
        assert!(
            database
                .due_credential_cleanup(i64::MAX, 32)
                .await
                .unwrap()
                .is_empty()
        );
        // Public scope reads remain authorization-gated. Check only the durable
        // native quota metadata through a read-only connection for this actor.
        use sqlx::Connection;
        let mut inspection = sqlx::SqliteConnection::connect_with(
            &sqlx::sqlite::SqliteConnectOptions::new()
                .filename(&path)
                .read_only(true),
        )
        .await
        .unwrap();
        let json: String = sqlx::query_scalar(
            "SELECT sync_json FROM sync_scopes WHERE account_id=? AND scope='provider:rest'",
        )
        .bind(&old.id)
        .fetch_one(&mut inspection)
        .await
        .unwrap();
        let quota: SyncStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(quota.state, SyncState::RateLimited);
        assert!(quota.next_retry_at.is_some());
        assert!(quota.error.is_none());
        inspection.close().await.unwrap();
        assert_eq!(vault.stores.load(Ordering::SeqCst), 0);
        assert_eq!(vault.loads.load(Ordering::SeqCst), 0);
        assert_eq!(task.join().unwrap().len(), 2);
    }
}

struct GatedProbe {
    actual: Arc<gitlab::GitlabProvider>,
    finished_http: Notify,
    release: Notify,
    feeds: AtomicUsize,
}
#[async_trait]
impl CollaborationProvider for GatedProbe {
    fn kind(&self) -> ProviderKind {
        ProviderKind::Gitlab
    }
    fn profile(&self, account: &RemoteAccount) -> ProviderProfile {
        self.actual.profile(account)
    }
    async fn probe(&self, token: &SecretToken) -> Result<VerifiedAccount, ProviderError> {
        self.actual.probe(token).await
    }
    async fn probe_with_backoff(
        &self,
        token: &SecretToken,
    ) -> Result<VerifiedAccount, ProbeFailure> {
        let result = self.actual.probe_with_backoff(token).await;
        self.finished_http.notify_one();
        self.release.notified().await;
        result
    }
    async fn fetch_page(
        &self,
        _: &SecretToken,
        _: FeedRequest,
    ) -> Result<FetchPage, ProviderError> {
        self.feeds.fetch_add(1, Ordering::SeqCst);
        Err(ProviderError::new(ProviderErrorKind::Unavailable))
    }
}

#[tokio::test]
async fn already_picked_feed_waiting_on_probe_rechecks_budget_before_loading_old_token() {
    let dir = tempfile::tempdir().unwrap();
    let database = Arc::new(
        Store::open(dir.path().join("independent.sqlite"))
            .await
            .unwrap(),
    );
    let vault = Arc::new(Vault::default());
    let old = seed(
        &database,
        &vault,
        account(ProviderKind::Gitlab, "gitlab.com", ACTOR),
    )
    .await;
    let (actual, task) = server(|_| {
        vec![
            response(200, "", &user(ACTOR)),
            response(429, "Retry-After: 120\r\n", "fixture"),
        ]
    });
    let provider = Arc::new(GatedProbe {
        actual: Arc::new(actual),
        finished_http: Notify::new(),
        release: Notify::new(),
        feeds: AtomicUsize::new(0),
    });
    let runtime = CollaborationRuntime::new(database.clone(), vault.clone(), provider.clone());
    runtime.refresh(refresh(&old.account)).await.unwrap();
    // Pick while the persisted quota is absent, exactly like run_next's first
    // check. A later probe cannot rely only on that earlier scheduler decision.
    let mut job = runtime.scheduler.lock().await.pick(runtime.now()).unwrap();
    assert!(
        database
            .scope_state(&old.account.id, "provider:rest")
            .await
            .unwrap()
            .is_none()
    );
    let connector = runtime.clone();
    let prospective = tokio::spawn(async move {
        connector
            .connect_gitlab("synthetic_prospective_token".into())
            .await
    });
    tokio::time::timeout(Duration::from_secs(3), provider.finished_http.notified())
        .await
        .unwrap();
    let worker = runtime.sync_feed_page(&mut job);
    tokio::pin!(worker);
    // No database/timer sleep precedes this lifecycle lock for a manual job.
    // Polling proves this worker is suspended behind the prospective probe.
    assert!(futures_util::poll!(&mut worker).is_pending());
    assert_eq!(vault.loads.load(Ordering::SeqCst), 0);
    provider.release.notify_one();
    assert_eq!(
        prospective.await.unwrap().unwrap_err().code,
        ErrorCode::RateLimited
    );
    assert_eq!(worker.await.unwrap_err().code, ErrorCode::RateLimited);
    assert_eq!(vault.loads.load(Ordering::SeqCst), 0);
    assert_eq!(provider.feeds.load(Ordering::SeqCst), 0);
    preserved(&database, &vault, &old).await;
    assert_eq!(task.join().unwrap().len(), 2);
}
