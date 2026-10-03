use async_trait::async_trait;
use collaboration::{
    credentials::{CredentialError, CredentialVault, SecretToken},
    github_cli::{GithubCli, GithubCliCommand, GithubCliFailure, GithubCliOutput, GithubCliRunner},
    providers::*,
    *,
};
use std::{
    collections::HashMap,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};
use zeroize::Zeroizing;

#[derive(Default)]
struct FakeVault {
    tokens: Mutex<HashMap<String, SecretToken>>,
}

impl CredentialVault for FakeVault {
    fn store(&self, reference: &str, token: &SecretToken) -> Result<(), CredentialError> {
        self.tokens
            .lock()
            .unwrap()
            .insert(reference.into(), token.clone());
        Ok(())
    }
    fn load(&self, reference: &str) -> Result<Option<SecretToken>, CredentialError> {
        Ok(self.tokens.lock().unwrap().get(reference).cloned())
    }
    fn delete(&self, reference: &str) -> Result<(), CredentialError> {
        self.tokens.lock().unwrap().remove(reference);
        Ok(())
    }
}

struct FakeProvider {
    calls: AtomicUsize,
    cursors: Mutex<Vec<Option<String>>>,
    pages: usize,
    error: Mutex<Option<ProviderError>>,
    entered: tokio::sync::Notify,
    hold: Option<Arc<tokio::sync::Notify>>,
}

impl FakeProvider {
    fn new(pages: usize) -> Self {
        Self {
            calls: AtomicUsize::new(0),
            cursors: Mutex::new(Vec::new()),
            pages,
            error: Mutex::new(None),
            entered: tokio::sync::Notify::new(),
            hold: None,
        }
    }
}

#[async_trait]
impl CollaborationProvider for FakeProvider {
    fn kind(&self) -> ProviderKind {
        ProviderKind::Github
    }
    async fn probe(&self, token: &SecretToken) -> Result<VerifiedAccount, ProviderError> {
        Ok(VerifiedAccount {
            actor_id: if token.expose() == "different_actor" {
                "2"
            } else {
                "1"
            }
            .into(),
            login: "actor".into(),
            display_name: None,
            notifications_supported: false,
        })
    }
    async fn fetch_page(
        &self,
        _: &SecretToken,
        request: FeedRequest,
    ) -> Result<FetchPage, ProviderError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.cursors.lock().unwrap().push(request.cursor.clone());
        let error = self.error.lock().unwrap().clone();
        self.entered.notify_one();
        if let Some(hold) = &self.hold {
            hold.notified().await;
        }
        if let Some(error) = error {
            return Err(error);
        }
        let page = request
            .cursor
            .as_deref()
            .and_then(|cursor| cursor.rsplit('=').next())
            .and_then(|page| page.parse::<usize>().ok())
            .unwrap_or(1);
        let repositories = if request.kind == FeedKind::Repositories {
            vec![RemoteRepository {
                id: format!("repo-{page}"),
                account_id: request.account.id,
                provider_id: page.to_string(),
                full_name: format!("actor/repo-{page}"),
                name: format!("repo-{page}"),
                web_url: format!("https://github.com/actor/repo-{page}"),
                description: None,
                default_branch: Some("main".into()),
                selected: false,
            }]
        } else {
            Vec::new()
        };
        Ok(FetchPage {
            repositories,
            items: Vec::new(),
            next_cursor: (page < self.pages)
                .then(|| format!("https://api.github.com/user/repos?page={}", page + 1)),
            etag: Some(format!("etag-{page}")),
            last_modified: None,
            not_modified: false,
            poll_interval_seconds: None,
            cooldown_seconds: None,
        })
    }
}

async fn eventually(mut condition: impl AsyncFnMut() -> bool) {
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            if condition().await {
                return;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("runtime made expected progress");
}

#[tokio::test]
async fn reauthentication_preserves_identity_and_isolates_different_actors() {
    let directory = tempfile::tempdir().unwrap();
    let store = Arc::new(
        Store::open(directory.path().join("remote.sqlite"))
            .await
            .unwrap(),
    );
    let vault = Arc::new(FakeVault::default());
    let runtime =
        CollaborationRuntime::new(store.clone(), vault.clone(), Arc::new(FakeProvider::new(1)));
    let first = runtime.connect_github("first_token".into()).await.unwrap();
    let second = runtime
        .connect_github("replacement_token".into())
        .await
        .unwrap();
    let other = runtime
        .connect_github("different_actor".into())
        .await
        .unwrap();
    assert_eq!(first.id, second.id);
    assert_ne!(first.authorization_epoch, second.authorization_epoch);
    assert_ne!(first.id, other.id);
    let second_ref = store
        .credential_reference(&second.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        vault.load(&second_ref).unwrap().unwrap().expose(),
        "replacement_token"
    );
    runtime.disconnect(&second.id).await.unwrap();
    assert!(vault.load(&second_ref).unwrap().is_none());
    assert_eq!(
        store.account(&second.id).await.unwrap().state,
        AccountState::Disconnected
    );
    assert!(
        runtime
            .refresh(RefreshRequest {
                account_id: second.id,
                repository_id: None,
                kind: None
            })
            .await
            .is_err()
    );
    let other_ref = store
        .credential_reference(&other.id)
        .await
        .unwrap()
        .unwrap();
    assert!(vault.load(&other_ref).unwrap().is_some());
}

#[tokio::test]
async fn capped_bootstrap_resumes_committed_cursor_and_run_instead_of_restarting() {
    let directory = tempfile::tempdir().unwrap();
    let store = Arc::new(
        Store::open(directory.path().join("remote.sqlite"))
            .await
            .unwrap(),
    );
    let provider = Arc::new(FakeProvider::new(12));
    let runtime = Arc::new(CollaborationRuntime::new(
        store.clone(),
        Arc::new(FakeVault::default()),
        provider.clone(),
    ));
    let account = runtime.connect_github("token".into()).await.unwrap();
    runtime.clone().start_background();
    eventually(async || {
        store
            .scope_state(&account.id, "repositories")
            .await
            .unwrap()
            .is_some_and(|scope| {
                scope.coverage.state == CoverageState::Partial
                    && scope.sync.state == SyncState::Idle
            })
    })
    .await;
    let partial = store
        .scope_state(&account.id, "repositories")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(provider.calls.load(Ordering::SeqCst), 10);
    assert!(partial.next_cursor.as_ref().unwrap().ends_with("page=11"));
    assert!(
        partial.etag.is_none(),
        "a page validator cannot validate a multi-page feed"
    );
    runtime
        .refresh(RefreshRequest {
            account_id: account.id.clone(),
            repository_id: None,
            kind: None,
        })
        .await
        .unwrap();
    eventually(async || {
        store
            .scope_state(&account.id, "repositories")
            .await
            .unwrap()
            .is_some_and(|scope| {
                scope.coverage.state == CoverageState::Complete
                    && scope.sync.state == SyncState::Idle
            })
    })
    .await;
    let complete = store
        .scope_state(&account.id, "repositories")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(partial.run_id, complete.run_id);
    assert!(
        complete.etag.is_none(),
        "the tail-page validator cannot validate page one"
    );
    assert_eq!(provider.calls.load(Ordering::SeqCst), 12);
    assert_eq!(
        store
            .repositories(&account.id)
            .await
            .unwrap()
            .repositories
            .len(),
        12
    );
}

#[tokio::test]
async fn duplicate_refreshes_share_one_inflight_job_and_disconnect_fences_its_result() {
    let directory = tempfile::tempdir().unwrap();
    let store = Arc::new(
        Store::open(directory.path().join("remote.sqlite"))
            .await
            .unwrap(),
    );
    let release = Arc::new(tokio::sync::Notify::new());
    let mut provider = FakeProvider::new(1);
    provider.hold = Some(release.clone());
    let provider = Arc::new(provider);
    let runtime = Arc::new(CollaborationRuntime::new(
        store.clone(),
        Arc::new(FakeVault::default()),
        provider.clone(),
    ));
    let account = runtime.connect_github("token".into()).await.unwrap();
    let request = RefreshRequest {
        account_id: account.id.clone(),
        repository_id: None,
        kind: None,
    };
    let first = runtime.refresh(request.clone()).await.unwrap();
    let second = runtime.refresh(request).await.unwrap();
    assert_eq!(first.job_id, second.job_id);
    runtime.clone().start_background();
    provider.entered.notified().await;
    assert_eq!(provider.calls.load(Ordering::SeqCst), 1);
    runtime.disconnect(&account.id).await.unwrap();
    release.notify_one();
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert_eq!(
        store.account(&account.id).await.unwrap().state,
        AccountState::Disconnected
    );
    assert_eq!(provider.calls.load(Ordering::SeqCst), 1);
    assert!(store.repositories(&account.id).await.is_err());
}

#[tokio::test]
async fn persisted_account_budget_constrains_manual_refresh_and_survives_restart() {
    let directory = tempfile::tempdir().unwrap();
    let store = Arc::new(
        Store::open(directory.path().join("remote.sqlite"))
            .await
            .unwrap(),
    );
    let provider = Arc::new(FakeProvider::new(1));
    let vault = Arc::new(FakeVault::default());
    let initial = CollaborationRuntime::new(store.clone(), vault.clone(), provider.clone());
    let account = initial.connect_github("token".into()).await.unwrap();
    store
        .set_sync_status(
            &account.id,
            &account.authorization_epoch,
            "provider:rest",
            SyncStatus {
                state: SyncState::RateLimited,
                last_success_at: None,
                next_retry_at: Some((chrono::Utc::now() + chrono::Duration::days(2)).to_rfc3339()),
                error: None,
            },
        )
        .await
        .unwrap();
    drop(initial);
    let runtime = Arc::new(CollaborationRuntime::new(
        store.clone(),
        vault,
        provider.clone(),
    ));
    runtime.clone().start_background();
    runtime
        .refresh(RefreshRequest {
            account_id: account.id.clone(),
            repository_id: None,
            kind: None,
        })
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert_eq!(provider.calls.load(Ordering::SeqCst), 0);
    assert!(
        store
            .scope_state(&account.id, "provider:rest")
            .await
            .unwrap()
            .unwrap()
            .sync
            .next_retry_at
            .is_some()
    );
    // Reading account metadata never invokes provider HTTP.
    store.accounts().await.unwrap();
    assert_eq!(provider.calls.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn late_authentication_failure_cannot_revoke_a_replacement_credential() {
    let directory = tempfile::tempdir().unwrap();
    let store = Arc::new(
        Store::open(directory.path().join("remote.sqlite"))
            .await
            .unwrap(),
    );
    let release = Arc::new(tokio::sync::Notify::new());
    let mut provider = FakeProvider::new(1);
    provider.hold = Some(release.clone());
    *provider.error.lock().unwrap() = Some(ProviderError::new(ProviderErrorKind::Authentication));
    let provider = Arc::new(provider);
    let runtime = Arc::new(CollaborationRuntime::new(
        store.clone(),
        Arc::new(FakeVault::default()),
        provider.clone(),
    ));
    let account = runtime.connect_github("token".into()).await.unwrap();
    runtime.clone().start_background();
    provider.entered.notified().await;
    *provider.error.lock().unwrap() = None;
    let replacement = runtime.connect_github("replacement".into()).await.unwrap();
    release.notify_one();
    eventually(async || provider.calls.load(Ordering::SeqCst) == 2).await;
    let current = store.account(&account.id).await.unwrap();
    assert_eq!(current.state, AccountState::Active);
    assert_eq!(current.authorization_epoch, replacement.authorization_epoch);
    runtime.disconnect(&account.id).await.unwrap();
    release.notify_one();
}

#[tokio::test]
async fn revoked_credentials_fence_all_jobs_and_draft_saves_publish_committed_revisions() {
    let directory = tempfile::tempdir().unwrap();
    let store = Arc::new(
        Store::open(directory.path().join("remote.sqlite"))
            .await
            .unwrap(),
    );
    let provider = Arc::new(FakeProvider::new(1));
    let runtime = Arc::new(CollaborationRuntime::new(
        store.clone(),
        Arc::new(FakeVault::default()),
        provider.clone(),
    ));
    let account = runtime.connect_github("token".into()).await.unwrap();
    let mut hints = runtime.subscribe();
    let draft = runtime
        .save_draft(LocalDraft {
            account_id: account.id.clone(),
            subject_id: "github:issue:123".into(),
            body: "offline draft".into(),
            generation: "0".into(),
        })
        .await
        .unwrap();
    let hint = hints.recv().await.unwrap();
    assert_eq!(hint.revision, store.revision().await.unwrap());
    assert_eq!(
        store
            .draft(&account.id, &draft.subject_id)
            .await
            .unwrap()
            .unwrap(),
        draft
    );
    *provider.error.lock().unwrap() = Some(ProviderError::new(ProviderErrorKind::Authentication));
    runtime.clone().start_background();
    eventually(async || {
        store.account(&account.id).await.unwrap().state == AccountState::AuthRequired
    })
    .await;
    let revoked = store.account(&account.id).await.unwrap();
    assert_ne!(revoked.authorization_epoch, account.authorization_epoch);
    assert!(
        runtime
            .refresh(RefreshRequest {
                account_id: account.id.clone(),
                repository_id: None,
                kind: None
            })
            .await
            .is_err()
    );
    assert!(
        store
            .draft(&account.id, &draft.subject_id)
            .await
            .unwrap()
            .is_some()
    );
}

struct FakeGithubCli {
    login: String,
    calls: Mutex<Vec<GithubCliCommand>>,
}

#[async_trait]
impl GithubCliRunner for FakeGithubCli {
    async fn run(&self, command: GithubCliCommand) -> Result<GithubCliOutput, GithubCliFailure> {
        self.calls.lock().unwrap().push(command.clone());
        let output = match command {
            GithubCliCommand::TokenHelp => "--user string".into(),
            GithubCliCommand::Discover => format!(
                r#"[{{"state":"success","active":true,"host":"github.com","login":"{}"}}]"#,
                self.login
            ),
            GithubCliCommand::Token { .. } => "gho_fixture_secret\n".into(),
        };
        Ok(GithubCliOutput {
            success: true,
            stdout: Zeroizing::new(output.into_bytes()),
            stderr: Zeroizing::new(Vec::new()),
        })
    }
}

#[tokio::test]
async fn cli_discovery_is_metadata_only_and_explicit_import_reuses_native_account_lifecycle() {
    let directory = tempfile::tempdir().unwrap();
    let store = Arc::new(
        Store::open(directory.path().join("remote.sqlite"))
            .await
            .unwrap(),
    );
    let vault = Arc::new(FakeVault::default());
    let provider = Arc::new(FakeProvider::new(1));
    let runner = Arc::new(FakeGithubCli {
        login: "Actor".into(),
        calls: Mutex::new(Vec::new()),
    });
    let runtime = CollaborationRuntime::new(store.clone(), vault.clone(), provider)
        .with_github_cli(GithubCli::with_runner(runner.clone()));
    let discovery = runtime.discover_github_cli().await;
    assert!(store.accounts().await.unwrap().accounts.is_empty());
    assert!(vault.tokens.lock().unwrap().is_empty());
    assert!(
        !runner
            .calls
            .lock()
            .unwrap()
            .iter()
            .any(|call| matches!(call, GithubCliCommand::Token { .. }))
    );
    let account = runtime
        .connect_github_cli(&discovery.accounts[0].id)
        .await
        .unwrap();
    assert_eq!(account.login, "actor");
    assert_eq!(account.state, AccountState::Active);
    assert_eq!(account.authorization_epoch, "1");
    let reference = store
        .credential_reference(&account.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        vault
            .tokens
            .lock()
            .unwrap()
            .get(&reference)
            .unwrap()
            .expose(),
        "gho_fixture_secret"
    );
    let replacement = runtime.connect_github("replacement".into()).await.unwrap();
    assert_eq!(account.id, replacement.id);
    assert_eq!(replacement.authorization_epoch, "2");
    assert!(
        !serde_json::to_string(&discovery)
            .unwrap()
            .contains("fixture_secret")
    );
    assert!(
        !serde_json::to_string(&account)
            .unwrap()
            .contains("fixture_secret")
    );
}

#[tokio::test]
async fn cli_identity_mismatch_never_writes_vault_or_creates_account() {
    let directory = tempfile::tempdir().unwrap();
    let store = Arc::new(
        Store::open(directory.path().join("remote.sqlite"))
            .await
            .unwrap(),
    );
    let vault = Arc::new(FakeVault::default());
    let runner = Arc::new(FakeGithubCli {
        login: "another".into(),
        calls: Mutex::new(Vec::new()),
    });
    let runtime =
        CollaborationRuntime::new(store.clone(), vault.clone(), Arc::new(FakeProvider::new(1)))
            .with_github_cli(GithubCli::with_runner(runner));
    let discovery = runtime.discover_github_cli().await;
    let error = runtime
        .connect_github_cli(&discovery.accounts[0].id)
        .await
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::StaleView);
    assert!(!error.message.contains("fixture_secret"));
    assert!(vault.tokens.lock().unwrap().is_empty());
    assert!(store.accounts().await.unwrap().accounts.is_empty());
}

#[tokio::test]
async fn core_runtime_defaults_to_isolated_cli_disabled_behavior() {
    let directory = tempfile::tempdir().unwrap();
    let store = Arc::new(
        Store::open(directory.path().join("remote.sqlite"))
            .await
            .unwrap(),
    );
    let runtime = CollaborationRuntime::new(
        store,
        Arc::new(FakeVault::default()),
        Arc::new(FakeProvider::new(1)),
    );
    assert_eq!(
        runtime.discover_github_cli().await.status,
        GithubCliStatus::NotInstalled
    );
    assert_eq!(
        runtime
            .connect_github_cli("unknown")
            .await
            .unwrap_err()
            .code,
        ErrorCode::StaleView
    );
}
