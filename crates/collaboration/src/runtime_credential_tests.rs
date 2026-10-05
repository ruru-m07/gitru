//! Hard process termination around the real runtime's credential boundaries.
//! Every secret belongs to an isolated durable fake vault; no OS vault is used.
use super::*;
use crate::credentials::CredentialError;
use async_trait::async_trait;
use sha2::{Digest, Sha256};
use std::{
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{Mutex as StdMutex, atomic::AtomicUsize},
};

const CHILD_TEST: &str = "runtime::credential_crash_tests::credential_crash_child";

pub(crate) fn checkpoint(name: &str) {
    if std::env::var("GITRU_CREDENTIAL_CRASH_BOUNDARY").as_deref() != Ok(name) {
        return;
    }
    let marker = std::env::var_os("GITRU_CREDENTIAL_CRASH_MARKER").unwrap();
    std::fs::write(marker, name).unwrap();
    // Parent terminates the child, without Rust unwinding or compensation.
    loop {
        std::thread::park();
    }
}

struct DurableVault {
    dir: PathBuf,
    fail_store: AtomicBool,
    fail_delete: AtomicBool,
    delete_calls: AtomicUsize,
    hold_write: StdMutex<Option<Arc<std::sync::Barrier>>>,
    entered: tokio::sync::Notify,
}

impl DurableVault {
    fn new(root: &Path) -> Self {
        let dir = root.join("vault");
        std::fs::create_dir_all(&dir).unwrap();
        Self {
            dir,
            fail_store: AtomicBool::new(false),
            fail_delete: AtomicBool::new(false),
            delete_calls: AtomicUsize::new(0),
            hold_write: StdMutex::new(None),
            entered: tokio::sync::Notify::new(),
        }
    }
    fn path(&self, reference: &str) -> PathBuf {
        // Vault references contain colons; hash them into portable fixture names.
        self.dir
            .join(format!("{:x}", Sha256::digest(reference.as_bytes())))
    }
    fn count(&self) -> usize {
        std::fs::read_dir(&self.dir).unwrap().count()
    }
}

impl CredentialVault for DurableVault {
    fn store(&self, reference: &str, token: &SecretToken) -> Result<(), CredentialError> {
        let barrier = self.hold_write.lock().unwrap().clone();
        self.entered.notify_one();
        if let Some(barrier) = barrier {
            barrier.wait();
        }
        let mut file = std::fs::File::create(self.path(reference)).unwrap();
        file.write_all(token.expose().as_bytes()).unwrap();
        file.sync_all().unwrap();
        if self.fail_store.load(Ordering::SeqCst) {
            Err(CredentialError::Unavailable)
        } else {
            Ok(())
        }
    }
    fn load(&self, reference: &str) -> Result<Option<SecretToken>, CredentialError> {
        match std::fs::read_to_string(self.path(reference)) {
            Ok(value) => SecretToken::new(value).map(Some),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(_) => Err(CredentialError::Unavailable),
        }
    }
    fn delete(&self, reference: &str) -> Result<(), CredentialError> {
        self.delete_calls.fetch_add(1, Ordering::SeqCst);
        if self.fail_delete.load(Ordering::SeqCst) {
            return Err(CredentialError::Unavailable);
        }
        match std::fs::remove_file(self.path(reference)) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(_) => Err(CredentialError::Unavailable),
        }
    }
}

struct TokenProvider {
    observed: StdMutex<Vec<String>>,
    kind: ProviderKind,
}
impl Default for TokenProvider {
    fn default() -> Self {
        Self {
            observed: StdMutex::default(),
            kind: ProviderKind::Github,
        }
    }
}
#[async_trait]
impl CollaborationProvider for TokenProvider {
    fn kind(&self) -> ProviderKind {
        self.kind
    }
    fn profile(&self, account: &RemoteAccount) -> ProviderProfile {
        if self.kind == ProviderKind::Gitlab {
            let mut profile = ProviderProfile::read_only(InboxSemantics::None, false);
            for capability in &mut profile.facets {
                if capability.facet != ResourceFacet::Repositories {
                    capability.state = CapabilityState::Unsupported;
                    capability.reason = Some(CapabilityReason::NotImplemented);
                }
            }
            profile
        } else {
            ProviderProfile::read_only(
                InboxSemantics::NativeNotifications,
                account.notifications_supported,
            )
        }
    }
    async fn probe(&self, token: &SecretToken) -> Result<VerifiedAccount, ProviderError> {
        Ok(VerifiedAccount {
            actor_id: "1".into(),
            login: "fixture-actor".into(),
            display_name: None,
            notifications_supported: self.kind == ProviderKind::Github
                && token.expose() == "replacement_fixture_token",
            cooldown_seconds: (self.kind == ProviderKind::Gitlab
                && token.expose() == "quota_fixture_token")
                .then_some(60),
        })
    }
    async fn fetch_page(
        &self,
        token: &SecretToken,
        _: FeedRequest,
    ) -> Result<FetchPage, ProviderError> {
        self.observed.lock().unwrap().push(token.expose().into());
        Ok(FetchPage {
            repositories: vec![],
            items: vec![],
            endpoint_aliases: Vec::new(),
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

async fn runtime(
    root: &Path,
) -> (
    Arc<Store>,
    Arc<DurableVault>,
    Arc<TokenProvider>,
    CollaborationRuntime,
) {
    runtime_for(root, ProviderKind::Github).await
}
async fn runtime_for(
    root: &Path,
    kind: ProviderKind,
) -> (
    Arc<Store>,
    Arc<DurableVault>,
    Arc<TokenProvider>,
    CollaborationRuntime,
) {
    let store = Arc::new(Store::open(root.join("remote.sqlite")).await.unwrap());
    let vault = Arc::new(DurableVault::new(root));
    let provider = Arc::new(TokenProvider {
        kind,
        ..TokenProvider::default()
    });
    let runtime = CollaborationRuntime::new(store.clone(), vault.clone(), provider.clone());
    (store, vault, provider, runtime)
}

async fn seed_open(
    store: &Store,
    runtime: &CollaborationRuntime,
) -> (RemoteAccount, String, LocalDraft) {
    seed_open_for(store, runtime, ProviderKind::Github).await
}
async fn seed_open_for(
    store: &Store,
    runtime: &CollaborationRuntime,
    kind: ProviderKind,
) -> (RemoteAccount, String, LocalDraft) {
    let account = match kind {
        ProviderKind::Gitlab => runtime.connect_gitlab("old_fixture_token".into()).await,
        _ => runtime.connect_github("old_fixture_token".into()).await,
    }
    .unwrap();
    let reference = store
        .credential_reference(&account.id)
        .await
        .unwrap()
        .unwrap();
    let draft = runtime
        .save_draft(LocalDraft {
            account_id: account.id.clone(),
            subject_id: if kind == ProviderKind::Gitlab {
                "gitlab:issue:123"
            } else {
                "github:issue:123"
            }
            .into(),
            body: "unsent private text".into(),
            generation: "0".into(),
        })
        .await
        .unwrap();
    (account, reference, draft)
}

async fn seed(root: &Path) -> (RemoteAccount, String, LocalDraft) {
    let (store, _, _, runtime) = runtime(root).await;
    let seeded = seed_open(&store, &runtime).await;
    store.close().await;
    drop(runtime);
    assert_eq!(
        Arc::strong_count(&store),
        1,
        "cutover task released storage before completion"
    );
    drop(store);
    seeded
}

async fn kill_at(root: &Path, boundary: &str, operation: &str) {
    kill_at_for(root, boundary, operation, ProviderKind::Github).await
}
async fn kill_at_for(root: &Path, boundary: &str, operation: &str, kind: ProviderKind) {
    let marker = root.join("boundary-reached");
    // Keep the harness independent of concurrent Cargo builds in other worktrees.
    let binary = root.join(if cfg!(windows) {
        "crash-harness.exe"
    } else {
        "crash-harness"
    });
    #[cfg(unix)]
    {
        // Concurrent forks must never inherit a parent descriptor that can
        // write this executable, even after the parent's copy has completed.
        let copied = Command::new("/bin/cp")
            .arg(std::env::current_exe().unwrap())
            .arg(&binary)
            .env_clear()
            .current_dir(root)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .expect("credential fixture snapshot writer starts");
        assert!(
            copied.success(),
            "credential fixture snapshot writer succeeds"
        );
    }
    #[cfg(not(unix))]
    std::fs::copy(std::env::current_exe().unwrap(), &binary).unwrap();
    let mut child = Command::new(binary)
        .args(["--exact", CHILD_TEST, "--ignored", "--nocapture"])
        .env("GITRU_CREDENTIAL_CRASH_ROOT", root)
        .env("GITRU_CREDENTIAL_CRASH_MARKER", &marker)
        .env("GITRU_CREDENTIAL_CRASH_BOUNDARY", boundary)
        .env("GITRU_CREDENTIAL_CRASH_OPERATION", operation)
        .env(
            "GITRU_CREDENTIAL_CRASH_PROVIDER",
            if kind == ProviderKind::Gitlab {
                "gitlab"
            } else {
                "github"
            },
        )
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::inherit())
        .spawn()
        .unwrap();
    let reached = tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            if marker.exists() {
                return;
            }
            assert!(
                child.try_wait().unwrap().is_none(),
                "child exited before {boundary}"
            );
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await;
    let _ = child.kill();
    let status = child.wait().unwrap();
    reached.expect("child reached the actual credential boundary");
    assert!(!status.success(), "hard termination must not unwind");
}

#[test]
#[ignore = "subprocess entry point used only by crash boundary tests"]
fn credential_crash_child() {
    let root = PathBuf::from(std::env::var_os("GITRU_CREDENTIAL_CRASH_ROOT").unwrap());
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            let kind =
                if std::env::var("GITRU_CREDENTIAL_CRASH_PROVIDER").as_deref() == Ok("gitlab") {
                    ProviderKind::Gitlab
                } else {
                    ProviderKind::Github
                };
            let (store, _, _, runtime) = runtime_for(&root, kind).await;
            if std::env::var("GITRU_CREDENTIAL_CRASH_OPERATION").as_deref() == Ok("disconnect") {
                let mut accounts = store.accounts().await.unwrap().accounts.into_iter();
                let account = accounts
                    .next()
                    .expect("disconnect fixture contains an account");
                assert!(
                    accounts.next().is_none(),
                    "disconnect fixture contains exactly one account"
                );
                runtime.disconnect(&account.id).await.unwrap();
            } else {
                let token = if std::env::var("GITRU_CREDENTIAL_CRASH_OPERATION").as_deref()
                    == Ok("quota")
                {
                    "quota_fixture_token"
                } else {
                    "replacement_fixture_token"
                };
                match kind {
                    ProviderKind::Gitlab => runtime.connect_gitlab(token.into()).await,
                    _ => runtime.connect_github(token.into()).await,
                }
                .unwrap();
            }
        });
    panic!("requested crash boundary was not reached");
}

async fn assert_native_token(
    runtime: &CollaborationRuntime,
    provider: &TokenProvider,
    account: &RemoteAccount,
    token: &str,
) {
    runtime
        .sync_feed_page(&mut Job {
            key: "fixture".into(),
            account: account.clone(),
            repository: None,
            kind: JobKind::Feed(FeedKind::Repositories),
            scope: "repositories".into(),
            reason: scheduler::Admission::Manual,
            pages: 0,
            detail_lease: None,
            detail_restarted: false,
            local_budget_refusal: false,
        })
        .await
        .unwrap();
    assert_eq!(&*provider.observed.lock().unwrap(), &[token.to_owned()]);
}

#[tokio::test]
async fn replacement_process_crashes_preserve_the_committed_authorization_and_drafts() {
    for boundary in [
        "before_stage",
        "during_stage",
        "after_stage",
        "before_vault_write",
        "after_vault_write",
        "before_cutover",
        "during_cutover",
        "after_cutover",
        "before_delete",
        "after_delete",
        "after_cleanup",
    ] {
        let directory = tempfile::tempdir().unwrap();
        let (old, old_ref, draft) = seed(directory.path()).await;
        kill_at(directory.path(), boundary, "replace").await;
        let (store, vault, provider, runtime) = runtime(directory.path()).await;
        let committed = store.account(&old.id).await.unwrap();
        let promoted = matches!(
            boundary,
            "after_cutover" | "before_delete" | "after_delete" | "after_cleanup"
        );
        assert_eq!(
            committed.authorization_epoch,
            if promoted { "2" } else { "1" },
            "{boundary}"
        );
        assert_eq!(committed.notifications_supported, promoted, "{boundary}");
        let reference = store.credential_reference(&old.id).await.unwrap().unwrap();
        assert_eq!(reference == old_ref, !promoted, "{boundary}");
        runtime.recover_credentials().await.unwrap();
        assert!(
            store
                .due_credential_cleanup(i64::MAX, 32)
                .await
                .unwrap()
                .is_empty(),
            "{boundary}"
        );
        assert_eq!(vault.count(), 1, "{boundary}");
        assert_eq!(
            store.draft(&old.id, &draft.subject_id).await.unwrap(),
            Some(draft),
            "{boundary}"
        );
        assert_native_token(
            &runtime,
            &provider,
            &committed,
            if promoted {
                "replacement_fixture_token"
            } else {
                "old_fixture_token"
            },
        )
        .await;
        if promoted {
            assert_eq!(
                store
                    .begin_sync(&old.id, &old.authorization_epoch, "repositories")
                    .await
                    .unwrap_err()
                    .code,
                ErrorCode::StaleView
            );
        }
    }
}

#[tokio::test]
async fn first_connection_process_crashes_never_authorize_an_uncommitted_token() {
    for boundary in [
        "before_stage",
        "during_stage",
        "after_stage",
        "before_vault_write",
        "after_vault_write",
        "before_cutover",
        "during_cutover",
        "after_cutover",
    ] {
        let directory = tempfile::tempdir().unwrap();
        kill_at(directory.path(), boundary, "first").await;
        let (store, vault, provider, runtime) = runtime(directory.path()).await;
        runtime.recover_credentials().await.unwrap();
        let accounts = store.accounts().await.unwrap().accounts;
        assert_eq!(
            accounts.len(),
            usize::from(boundary == "after_cutover"),
            "{boundary}"
        );
        assert_eq!(vault.count(), accounts.len(), "{boundary}");
        assert!(
            store
                .due_credential_cleanup(i64::MAX, 32)
                .await
                .unwrap()
                .is_empty()
        );
        if let Some(account) = accounts.first() {
            assert_native_token(&runtime, &provider, account, "replacement_fixture_token").await;
        }
    }
}

#[tokio::test]
async fn disconnect_process_crashes_retry_secret_removal_and_preserve_drafts() {
    for boundary in [
        "before_disconnect",
        "during_disconnect",
        "after_disconnect",
        "before_delete",
        "after_delete",
        "after_cleanup",
    ] {
        let directory = tempfile::tempdir().unwrap();
        let (old, _, draft) = seed(directory.path()).await;
        kill_at(directory.path(), boundary, "disconnect").await;
        let (store, vault, _, runtime) = runtime(directory.path()).await;
        runtime.recover_credentials().await.unwrap();
        let disconnected = !matches!(boundary, "before_disconnect" | "during_disconnect");
        assert_eq!(
            store.account(&old.id).await.unwrap().state,
            if disconnected {
                AccountState::Disconnected
            } else {
                AccountState::Active
            }
        );
        assert_eq!(vault.count(), usize::from(!disconnected), "{boundary}");
        assert_eq!(
            store.credential_reference(&old.id).await.unwrap().is_none(),
            disconnected
        );
        assert_eq!(
            store.draft(&old.id, &draft.subject_id).await.unwrap(),
            Some(draft)
        );
        assert!(
            store
                .due_credential_cleanup(i64::MAX, 32)
                .await
                .unwrap()
                .is_empty()
        );
    }
}

#[tokio::test]
async fn partially_failed_vault_write_and_locked_cleanup_keep_the_old_account_usable() {
    let directory = tempfile::tempdir().unwrap();
    let (store, vault, provider, runtime) = runtime(directory.path()).await;
    let (old, old_ref, draft) = seed_open(&store, &runtime).await;
    vault.fail_store.store(true, Ordering::SeqCst);
    vault.fail_delete.store(true, Ordering::SeqCst);
    assert_eq!(
        runtime
            .connect_github("replacement_fixture_token".into())
            .await
            .unwrap_err()
            .code,
        ErrorCode::CredentialStoreUnavailable
    );
    assert_eq!(store.account(&old.id).await.unwrap(), old);
    assert_eq!(
        store.credential_reference(&old.id).await.unwrap(),
        Some(old_ref)
    );
    let pending = store.due_credential_cleanup(i64::MAX, 32).await.unwrap();
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].attempts, 1);
    let calls = vault.delete_calls.load(Ordering::SeqCst);
    runtime.recover_credentials().await.unwrap();
    assert_eq!(
        vault.delete_calls.load(Ordering::SeqCst),
        calls,
        "persisted backoff avoids a hot retry loop"
    );
    assert_native_token(&runtime, &provider, &old, "old_fixture_token").await;
    assert_eq!(
        store.draft(&old.id, &draft.subject_id).await.unwrap(),
        Some(draft)
    );
    vault.fail_delete.store(false, Ordering::SeqCst);
    store
        .defer_credential_cleanup(&pending[0].reference, 0)
        .await
        .unwrap();
    runtime.recover_credentials().await.unwrap();
    assert_eq!(vault.count(), 1);
}

#[tokio::test]
async fn locked_retirement_does_not_roll_back_a_working_replacement() {
    let directory = tempfile::tempdir().unwrap();
    let (store, vault, provider, runtime) = runtime(directory.path()).await;
    let (old, old_ref, _) = seed_open(&store, &runtime).await;
    vault.fail_delete.store(true, Ordering::SeqCst);
    let replacement = runtime
        .connect_github("replacement_fixture_token".into())
        .await
        .unwrap();
    assert_ne!(
        store.credential_reference(&old.id).await.unwrap(),
        Some(old_ref)
    );
    assert_eq!(replacement.authorization_epoch, "2");
    assert_native_token(
        &runtime,
        &provider,
        &replacement,
        "replacement_fixture_token",
    )
    .await;
    assert_eq!(
        store
            .due_credential_cleanup(i64::MAX, 32)
            .await
            .unwrap()
            .len(),
        1
    );
    assert_eq!(vault.count(), 2);
}

#[tokio::test]
async fn disconnect_cleans_its_own_reference_before_unrelated_background_work() {
    let directory = tempfile::tempdir().unwrap();
    let (store, vault, _, runtime) = runtime(directory.path()).await;
    let (old, _, _) = seed_open(&store, &runtime).await;
    for index in 0..9 {
        store
            .stage_credential("uncommitted-account", &format!("aa-staged:{index}"))
            .await
            .unwrap();
    }
    runtime.disconnect(&old.id).await.unwrap();
    assert_eq!(
        vault.count(),
        0,
        "explicit disconnect is not deferred behind eight other entries"
    );
    assert_eq!(
        store
            .due_credential_cleanup(i64::MAX, 32)
            .await
            .unwrap()
            .len(),
        9
    );
}

#[tokio::test]
async fn locked_disconnect_fences_authorization_and_keeps_cleanup_evidence_and_drafts() {
    let directory = tempfile::tempdir().unwrap();
    let (store, vault, _, runtime) = runtime(directory.path()).await;
    let (old, old_ref, draft) = seed_open(&store, &runtime).await;
    vault.fail_delete.store(true, Ordering::SeqCst);
    assert_eq!(
        runtime.disconnect(&old.id).await.unwrap_err().code,
        ErrorCode::CredentialStoreUnavailable
    );
    assert_eq!(
        store.account(&old.id).await.unwrap().state,
        AccountState::Disconnected
    );
    assert!(store.credential_reference(&old.id).await.unwrap().is_none());
    assert!(store.repositories(&old.id).await.is_err());
    assert_eq!(
        store.draft(&old.id, &draft.subject_id).await.unwrap(),
        Some(draft)
    );
    assert_eq!(
        store
            .credential_cleanup(&old_ref)
            .await
            .unwrap()
            .unwrap()
            .attempts,
        1
    );
    vault.fail_delete.store(false, Ordering::SeqCst);
    store.defer_credential_cleanup(&old_ref, 0).await.unwrap();
    runtime.recover_credentials().await.unwrap();
    assert_eq!(vault.count(), 0);
}

#[tokio::test]
async fn failed_sqlite_cutover_rolls_back_epoch_reference_and_cache_invalidation() {
    use sqlx::{Connection, sqlite::SqliteConnectOptions};
    let directory = tempfile::tempdir().unwrap();
    let (store, vault, provider, runtime) = runtime(directory.path()).await;
    let (old, old_ref, draft) = seed_open(&store, &runtime).await;
    assert_native_token(&runtime, &provider, &old, "old_fixture_token").await;
    let run = store
        .scope_state(&old.id, "repositories")
        .await
        .unwrap()
        .unwrap()
        .run_id;
    let mut fault = sqlx::SqliteConnection::connect_with(
        &SqliteConnectOptions::new().filename(directory.path().join("remote.sqlite")),
    )
    .await
    .unwrap();
    sqlx::query("CREATE TRIGGER fail_credential_cutover BEFORE INSERT ON account_credentials BEGIN SELECT RAISE(ABORT,'fixture failure'); END")
        .execute(&mut fault).await.unwrap();
    let error = runtime
        .connect_github("replacement_fixture_token".into())
        .await
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::Storage);
    assert_eq!(store.account(&old.id).await.unwrap(), old);
    assert_eq!(
        store.credential_reference(&old.id).await.unwrap(),
        Some(old_ref)
    );
    assert_eq!(
        store
            .scope_state(&old.id, "repositories")
            .await
            .unwrap()
            .unwrap()
            .run_id,
        run
    );
    assert_eq!(
        store.draft(&old.id, &draft.subject_id).await.unwrap(),
        Some(draft)
    );
    assert_eq!(
        vault.count(),
        1,
        "failed replacement is cleaned, old credential survives"
    );
    assert!(
        store
            .due_credential_cleanup(i64::MAX, 32)
            .await
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn cancelling_the_requester_cannot_abandon_a_delayed_vault_write() {
    let directory = tempfile::tempdir().unwrap();
    let (store, vault, _, runtime) = runtime(directory.path()).await;
    let (old, _, _) = seed_open(&store, &runtime).await;
    vault.entered.notified().await; // consume the initial credential write
    let barrier = Arc::new(std::sync::Barrier::new(2));
    *vault.hold_write.lock().unwrap() = Some(barrier.clone());
    let requester_runtime = runtime.clone();
    let requester = tokio::spawn(async move {
        requester_runtime
            .connect_github("replacement_fixture_token".into())
            .await
    });
    vault.entered.notified().await;
    requester.abort();
    let cleanup_runtime = runtime.clone();
    let cleanup = tokio::spawn(async move { cleanup_runtime.recover_credentials().await });
    tokio::task::spawn_blocking(move || barrier.wait())
        .await
        .unwrap();
    cleanup.await.unwrap().unwrap();
    assert_eq!(
        store.account(&old.id).await.unwrap().authorization_epoch,
        "2"
    );
    assert_eq!(vault.count(), 1);
    assert!(
        store
            .due_credential_cleanup(i64::MAX, 32)
            .await
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn gitlab_replacement_process_crashes_preserve_exact_grant_reference_and_private_drafts() {
    for boundary in [
        "before_stage",
        "during_stage",
        "after_stage",
        "before_vault_write",
        "after_vault_write",
        "before_cutover",
        "during_cutover",
        "after_cutover",
        "before_delete",
        "after_delete",
        "after_cleanup",
    ] {
        let directory = tempfile::tempdir().unwrap();
        let (store, _, _, runtime) = runtime_for(directory.path(), ProviderKind::Gitlab).await;
        let (old, old_ref, draft) = seed_open_for(&store, &runtime, ProviderKind::Gitlab).await;
        store.close().await;
        drop(runtime);
        drop(store);
        kill_at_for(directory.path(), boundary, "replace", ProviderKind::Gitlab).await;
        let (store, vault, provider, runtime) =
            runtime_for(directory.path(), ProviderKind::Gitlab).await;
        let committed = store.account(&old.id).await.unwrap();
        let promoted = matches!(
            boundary,
            "after_cutover" | "before_delete" | "after_delete" | "after_cleanup"
        );
        assert_eq!(committed.provider, ProviderKind::Gitlab);
        assert_eq!(committed.host, "gitlab.com");
        assert_eq!(
            committed.authorization_epoch,
            if promoted { "2" } else { "1" },
            "{boundary}"
        );
        assert!(!committed.notifications_supported);
        assert_eq!(
            store.credential_reference(&old.id).await.unwrap().unwrap() == old_ref,
            !promoted,
            "{boundary}"
        );
        runtime.recover_credentials().await.unwrap();
        assert_eq!(vault.count(), 1, "{boundary}");
        assert!(
            store
                .due_credential_cleanup(i64::MAX, 32)
                .await
                .unwrap()
                .is_empty(),
            "{boundary}"
        );
        assert_eq!(
            store.draft(&old.id, &draft.subject_id).await.unwrap(),
            Some(draft),
            "{boundary}"
        );
        assert_native_token(
            &runtime,
            &provider,
            &committed,
            if promoted {
                "replacement_fixture_token"
            } else {
                "old_fixture_token"
            },
        )
        .await;
        if promoted {
            assert_eq!(
                store
                    .begin_sync(&old.id, &old.authorization_epoch, "repositories")
                    .await
                    .unwrap_err()
                    .code,
                ErrorCode::StaleView
            );
        }
    }
}

#[tokio::test]
async fn gitlab_first_connection_crashes_never_promote_an_uncommitted_secret() {
    for boundary in [
        "before_stage",
        "during_stage",
        "after_stage",
        "before_vault_write",
        "after_vault_write",
        "before_cutover",
        "during_cutover",
        "after_cutover",
    ] {
        let directory = tempfile::tempdir().unwrap();
        kill_at_for(directory.path(), boundary, "first", ProviderKind::Gitlab).await;
        let (store, vault, provider, runtime) =
            runtime_for(directory.path(), ProviderKind::Gitlab).await;
        runtime.recover_credentials().await.unwrap();
        let accounts = store.accounts().await.unwrap().accounts;
        assert_eq!(
            accounts.len(),
            usize::from(boundary == "after_cutover"),
            "{boundary}"
        );
        assert_eq!(vault.count(), accounts.len(), "{boundary}");
        assert!(
            store
                .due_credential_cleanup(i64::MAX, 32)
                .await
                .unwrap()
                .is_empty()
        );
        if let Some(account) = accounts.first() {
            assert_native_token(&runtime, &provider, account, "replacement_fixture_token").await;
        }
    }
}

#[tokio::test]
async fn gitlab_cancelled_requester_and_locked_cleanup_preserve_owned_cutover() {
    let directory = tempfile::tempdir().unwrap();
    let (store, vault, _, runtime) = runtime_for(directory.path(), ProviderKind::Gitlab).await;
    let (old, old_ref, draft) = seed_open_for(&store, &runtime, ProviderKind::Gitlab).await;
    vault.entered.notified().await;
    vault.fail_delete.store(true, Ordering::SeqCst);
    let barrier = Arc::new(std::sync::Barrier::new(2));
    *vault.hold_write.lock().unwrap() = Some(barrier.clone());
    let requester_runtime = runtime.clone();
    let requester = tokio::spawn(async move {
        requester_runtime
            .connect_gitlab("replacement_fixture_token".into())
            .await
    });
    vault.entered.notified().await;
    requester.abort();
    let cleanup_runtime = runtime.clone();
    let cleanup = tokio::spawn(async move { cleanup_runtime.recover_credentials().await });
    tokio::task::spawn_blocking(move || barrier.wait())
        .await
        .unwrap();
    cleanup.await.unwrap().unwrap();
    assert_eq!(
        store.account(&old.id).await.unwrap().authorization_epoch,
        "2"
    );
    assert_ne!(
        store.credential_reference(&old.id).await.unwrap(),
        Some(old_ref.clone())
    );
    assert_eq!(
        store.draft(&old.id, &draft.subject_id).await.unwrap(),
        Some(draft)
    );
    assert_eq!(vault.count(), 2);
    let journal = store.credential_cleanup(&old_ref).await.unwrap().unwrap();
    assert!(journal.attempts >= 1);
    vault.fail_delete.store(false, Ordering::SeqCst);
    store.defer_credential_cleanup(&old_ref, 0).await.unwrap();
    runtime.recover_credentials().await.unwrap();
    assert_eq!(vault.count(), 1);
}

#[tokio::test]
async fn gitlab_probe_quota_is_atomic_with_cutover_and_never_shortens_retained_budget() {
    for replacement in [false, true] {
        let directory = tempfile::tempdir().unwrap();
        let retained_deadline = if replacement {
            let (store, _, _, runtime) = runtime_for(directory.path(), ProviderKind::Gitlab).await;
            let (account, _, _) = seed_open_for(&store, &runtime, ProviderKind::Gitlab).await;
            let deadline = runtime.future_string(3600);
            store
                .set_sync_status(
                    &account.id,
                    &account.authorization_epoch,
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
            store.close().await;
            drop(runtime);
            drop(store);
            Some(deadline)
        } else {
            None
        };
        kill_at_for(
            directory.path(),
            "after_cutover",
            "quota",
            ProviderKind::Gitlab,
        )
        .await;
        let (store, vault, provider, runtime) =
            runtime_for(directory.path(), ProviderKind::Gitlab).await;
        let mut accounts = store.accounts().await.unwrap().accounts.into_iter();
        let account = accounts.next().expect("promoted quota fixture account");
        assert!(accounts.next().is_none());
        assert_eq!(
            account.authorization_epoch,
            if replacement { "2" } else { "1" }
        );
        assert!(
            store
                .credential_reference(&account.id)
                .await
                .unwrap()
                .is_some()
        );
        let status = store
            .scope_state(&account.id, "provider:rest")
            .await
            .unwrap()
            .unwrap()
            .sync;
        assert_eq!(status.state, SyncState::RateLimited);
        assert!(status.next_retry_at.is_some());
        if let Some(retained) = retained_deadline {
            assert_eq!(status.next_retry_at, Some(retained));
        }
        runtime.recover_credentials().await.unwrap();
        assert_eq!(vault.count(), 1);
        runtime
            .refresh(RefreshRequest {
                account_id: account.id,
                repository_id: None,
                kind: None,
            })
            .await
            .unwrap();
        assert!(!runtime.run_next().await);
        assert!(!runtime.run_next().await);
        assert!(
            provider.observed.lock().unwrap().is_empty(),
            "post-crash refresh obeys the committed probe quota"
        );
    }
}

#[tokio::test]
async fn failed_gitlab_quota_publication_rolls_back_grant_epoch_reference_and_draft() {
    use sqlx::{Connection, sqlite::SqliteConnectOptions};
    let directory = tempfile::tempdir().unwrap();
    let (store, vault, _, runtime) = runtime_for(directory.path(), ProviderKind::Gitlab).await;
    let (old, old_ref, draft) = seed_open_for(&store, &runtime, ProviderKind::Gitlab).await;
    let mut fault = sqlx::SqliteConnection::connect_with(
        &SqliteConnectOptions::new().filename(directory.path().join("remote.sqlite")),
    )
    .await
    .unwrap();
    sqlx::query("CREATE TRIGGER fail_probe_quota BEFORE INSERT ON sync_scopes WHEN NEW.scope='provider:rest' BEGIN SELECT RAISE(ABORT,'synthetic quota failure'); END").execute(&mut fault).await.unwrap();
    assert_eq!(
        runtime
            .connect_gitlab("quota_fixture_token".into())
            .await
            .unwrap_err()
            .code,
        ErrorCode::Storage
    );
    assert_eq!(store.account(&old.id).await.unwrap(), old);
    assert_eq!(
        store.credential_reference(&old.id).await.unwrap(),
        Some(old_ref)
    );
    assert_eq!(
        store.draft(&old.id, &draft.subject_id).await.unwrap(),
        Some(draft)
    );
    assert!(
        store
            .scope_state(&old.id, "provider:rest")
            .await
            .unwrap()
            .is_none()
    );
    assert_eq!(vault.count(), 1);
    assert!(
        store
            .due_credential_cleanup(i64::MAX, 32)
            .await
            .unwrap()
            .is_empty()
    );
}
