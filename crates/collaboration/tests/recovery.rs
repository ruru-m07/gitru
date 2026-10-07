//! Black-box recovery contracts. Every database and vault here is synthetic.
//! Process termination at native mutation boundaries is exercised separately
//! by the recovery module's subprocess tests.
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use async_trait::async_trait;
use collaboration::{
    credentials::{CredentialError, CredentialVault, SecretToken},
    providers::*,
    recovery::{InterruptedRecovery, RecoverySession, RestoreChoice},
    *,
};
use sha2::{Digest, Sha256};
use sqlx::{Connection, SqliteConnection, sqlite::SqliteConnectOptions};

const SUBJECT: &str = "missing-subject";
const A_BODY: &str = "Actor A authored text — café 🌱\nKeep the final newline.\n";
const B_BODY: &str = "Disconnected actor B's independent text\n";
const A_REF: &str = "reference-only-a-physical-redaction-sentinel-429c";
const B_REF: &str = "reference-only-b-physical-redaction-sentinel-c138";
const STAGED_REF: &str = "reference-only-staged-physical-redaction-sentinel-28bd";

fn append(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.as_os_str().to_os_string();
    name.push(suffix);
    PathBuf::from(name)
}

fn digest(path: &Path) -> String {
    format!("{:x}", Sha256::digest(std::fs::read(path).unwrap()))
}

fn account(id: &str, epoch: &str) -> RemoteAccount {
    RemoteAccount {
        id: id.into(),
        provider: ProviderKind::Github,
        host: "github.com".into(),
        actor_id: format!("actor-{id}"),
        login: format!("synthetic-{id}"),
        display_name: None,
        authorization_epoch: epoch.into(),
        state: AccountState::Active,
        notifications_supported: true,
    }
}

async fn fixture(path: &Path) -> Store {
    let store = Store::open(path).await.unwrap();
    for (id, reference, body) in [("a", A_REF, A_BODY), ("b", B_REF, B_BODY)] {
        store.stage_credential(id, reference).await.unwrap();
        store
            .commit_account_credential(account(id, "1"), reference)
            .await
            .unwrap();
        store
            .save_draft(LocalDraft {
                account_id: id.into(),
                subject_id: SUBJECT.into(),
                body: body.into(),
                generation: "0".into(),
            })
            .await
            .unwrap();
    }
    // Disconnected-account cleanup and an abandoned new-account replacement
    // exercise both kinds of native references without calling an OS vault.
    store.disconnect("b").await.unwrap();
    store
        .stage_credential("not-connected", STAGED_REF)
        .await
        .unwrap();
    cache_observation(&store).await;
    store
}

async fn cache_observation(store: &Store) {
    let run_id = store.begin_sync("a", "1", "repositories").await.unwrap();
    store
        .apply_page(PageCommit {
            account_id: "a".into(),
            authorization_epoch: "1".into(),
            scope: "repositories".into(),
            run_id,
            repositories: vec![RemoteRepository {
                id: "repo-a".into(),
                account_id: "a".into(),
                provider_id: "42".into(),
                full_name: "synthetic/project".into(),
                name: "project".into(),
                web_url: "https://github.com/synthetic/project".into(),
                description: None,
                default_branch: Some("main".into()),
                selected: false,
            }],
            items: vec![],
            endpoint_aliases: vec![],
            next_cursor: None,
            etag: Some("repository-validator".into()),
            last_modified: None,
            not_modified: false,
            complete: true,
            observed_at: "2026-10-03T00:00:00Z".into(),
        })
        .await
        .unwrap();
    store.select_repository("a", "repo-a", true).await.unwrap();
    let scope = "repo:repo-a:pull_request";
    let run_id = store.begin_sync("a", "1", scope).await.unwrap();
    store
        .apply_page(PageCommit {
            account_id: "a".into(),
            authorization_epoch: "1".into(),
            scope: scope.into(),
            run_id,
            repositories: vec![],
            items: vec![RemoteItem {
                id: "cached-pr".into(),
                account_id: "a".into(),
                repository_id: Some("repo-a".into()),
                provider_id: "67".into(),
                kind: RemoteItemKind::PullRequest,
                number: Some("67".into()),
                title: "Synthetic cached observation".into(),
                body: Some("Provider data may be exported but must be fenced on restore".into()),
                body_omitted: false,
                author: Some("synthetic".into()),
                web_url: None,
                state: "open".into(),
                updated_at: "2026-10-03T00:00:00Z".into(),
                head_oid: Some("synthetic-head".into()),
                is_draft: Some(false),
                reason: None,
                unread: None,
                native_inbox: None,
            }],
            endpoint_aliases: vec![],
            next_cursor: None,
            etag: Some("item-validator".into()),
            last_modified: None,
            not_modified: false,
            complete: true,
            observed_at: "2026-10-03T00:00:00Z".into(),
        })
        .await
        .unwrap();
}

async fn close(store: Store, path: &Path) {
    store.close().await.unwrap();
    drop(store);
    // SQLx's writer drop signals an asynchronous worker close. Explicitly
    // checkpoint through a temporary fixture connection before byte snapshots;
    // otherwise that final worker checkpoint can change the main file later.
    let mut db = connection(path, false).await;
    let (busy, _, _): (i64, i64, i64) = sqlx::query_as("PRAGMA wal_checkpoint(TRUNCATE)")
        .fetch_one(&mut db)
        .await
        .unwrap();
    assert_eq!(busy, 0, "Synthetic fixture must be quiescent");
    db.close().await.unwrap();
}

async fn connection(path: &Path, read_only: bool) -> SqliteConnection {
    SqliteConnection::connect_with(
        &SqliteConnectOptions::new()
            .filename(path)
            .read_only(read_only)
            .foreign_keys(true),
    )
    .await
    .unwrap()
}

async fn execute(path: &Path, sql: &'static str) {
    let mut db = connection(path, false).await;
    sqlx::raw_sql(sql).execute(&mut db).await.unwrap();
    db.close().await.unwrap();
}

async fn count(path: &Path, table: &str) -> i64 {
    let mut db = connection(path, true).await;
    // Table names below are fixed test-owned literals, never user input.
    let value = sqlx::query_scalar(sqlx::AssertSqlSafe(format!("SELECT count(*) FROM {table}")))
        .fetch_one(&mut db)
        .await
        .unwrap();
    db.close().await.unwrap();
    value
}

async fn assert_authored(store: &Store, id: &str, body: &str, generation: &str) {
    let draft = store.draft(id, SUBJECT).await.unwrap().unwrap();
    assert_eq!(draft.account_id, id);
    assert_eq!(draft.subject_id, SUBJECT);
    assert_eq!(draft.body, body);
    assert_eq!(draft.generation, generation);
}

fn assert_references_absent(path: &Path, references: &[&str]) {
    let bytes = std::fs::read(path).unwrap();
    for reference in references {
        assert!(
            !bytes
                .windows(reference.len())
                .any(|part| part == reference.as_bytes()),
            "Native reference remained in exported database bytes"
        );
    }
}

#[tokio::test]
async fn active_wal_backup_preserves_authored_data_and_physically_redacts_references() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("source.sqlite");
    let backup = dir.path().join("backup.sqlite");
    let store = fixture(&source).await;
    assert!(std::fs::metadata(append(&source, "-wal")).unwrap().len() > 32);
    let before = store.accounts().await.unwrap();
    let summary = store.backup_to(&backup).await.unwrap();
    assert_eq!(summary.revision, before.revision);
    assert_eq!(summary.schema_version, 19);
    assert_eq!((summary.accounts, summary.drafts), (2, 2));
    assert_eq!(
        summary.sha256,
        format!("{:x}", Sha256::digest(std::fs::read(&backup).unwrap()))
    );
    assert_eq!(count(&backup, "account_credentials").await, 0);
    assert_eq!(count(&backup, "credential_cleanup").await, 0);
    assert_eq!(count(&backup, "items").await, 1);
    assert_references_absent(&backup, &[A_REF, B_REF, STAGED_REF]);
    assert_authored(&store, "a", A_BODY, "1").await;
    assert_authored(&store, "b", B_BODY, "1").await;
    assert_eq!(
        store.credential_reference("a").await.unwrap().as_deref(),
        Some(A_REF)
    );
    assert_eq!(
        store
            .due_credential_cleanup(i64::MAX, 8)
            .await
            .unwrap()
            .len(),
        2
    );
    assert_eq!(store.accounts().await.unwrap().revision, before.revision);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(&backup).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }
    close(store, &source).await;
    let exported = Store::open(&backup).await.unwrap();
    assert_authored(&exported, "a", A_BODY, "1").await;
    assert_authored(&exported, "b", B_BODY, "1").await;
    close(exported, &backup).await;
}

#[tokio::test]
async fn backup_refuses_existing_destination_and_live_main_file_selection() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("source.sqlite");
    let target = dir.path().join("target.sqlite");
    let destination = dir.path().join("already-exists");
    let source_store = fixture(&source).await;
    let target_store = fixture(&target).await;
    close(target_store, &target).await;
    std::fs::write(&destination, b"existing user-selected bytes").unwrap();
    assert!(source_store.backup_to(&destination).await.is_err());
    assert_eq!(
        std::fs::read(&destination).unwrap(),
        b"existing user-selected bytes"
    );
    assert!(source_store.backup_to(&source).await.is_err());
    assert!(RecoverySession::prepare(&target, &source).await.is_err());
    assert_authored(&source_store, "a", A_BODY, "1").await;
    close(source_store, &source).await;
}

#[tokio::test]
async fn preview_requires_exclusive_lease_and_cancel_or_wrong_confirmation_preserves_target() {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("target.sqlite");
    let backup = dir.path().join("backup.sqlite");
    let store = fixture(&target).await;
    store.backup_to(&backup).await.unwrap();
    let error = RecoverySession::prepare(&target, &backup)
        .await
        .err()
        .unwrap();
    assert_eq!(error.code, ErrorCode::Busy);
    close(store, &target).await;
    let original = digest(&target);
    let session = RecoverySession::prepare(&target, &backup).await.unwrap();
    let error = Store::open(&target).await.err().unwrap();
    assert_eq!(error.code, ErrorCode::Busy);
    drop(session);
    assert_eq!(digest(&target), original);
    assert!(!append(&target, ".restore-pending").exists());
    let session = RecoverySession::prepare(&target, &backup).await.unwrap();
    assert_eq!(
        session
            .confirm("uninspected-preview", RestoreChoice::ReplaceCurrentData)
            .unwrap_err()
            .code,
        ErrorCode::InvalidInput
    );
    let session = RecoverySession::prepare(&target, &backup).await.unwrap();
    let id = session.preview().confirmation_id.clone();
    assert_eq!(
        session
            .confirm(&id, RestoreChoice::KeepOriginalData)
            .unwrap_err()
            .code,
        ErrorCode::InvalidInput
    );
    let reopened = Store::open(&target).await.unwrap();
    assert_authored(&reopened, "a", A_BODY, "1").await;
    close(reopened, &target).await;
}

#[tokio::test]
async fn old_backup_restore_preserves_newer_original_and_advances_authorization_fences() {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("target.sqlite");
    let backup = dir.path().join("old-backup.sqlite");
    let store = fixture(&target).await;
    let incoming = store.backup_to(&backup).await.unwrap();
    store.upsert_account(account("a", "80")).await.unwrap();
    let mut newer = store.draft("a", SUBJECT).await.unwrap().unwrap();
    newer.body = "Newer unsent current draft, preserve in original bundle\n".into();
    store.save_draft(newer.clone()).await.unwrap();
    let current = store.accounts().await.unwrap();
    close(store, &target).await;
    let session = RecoverySession::prepare(&target, &backup).await.unwrap();
    let preview = session.preview();
    assert_eq!(preview.incoming, incoming);
    assert_eq!(
        preview.current_revision.as_deref(),
        Some(current.revision.as_str())
    );
    assert_eq!(preview.current_drafts, Some(2));
    assert_eq!(preview.current_accounts, Some(2));
    assert!(preview.newer_current_drafts_remain_in_original_bundle);
    assert!(preview.reauthentication_required && preview.cached_provider_data_will_be_removed);
    let id = preview.confirmation_id.clone();
    let receipt = session
        .confirm(&id, RestoreChoice::ReplaceCurrentData)
        .unwrap();
    assert!(receipt.original_bundle.join("candidate.sqlite").is_file());
    assert!(!append(&target, ".restore-pending").exists());
    let restored = Store::open(&target).await.unwrap();
    let after = restored.accounts().await.unwrap();
    assert!(after.revision.parse::<u64>().unwrap() > current.revision.parse::<u64>().unwrap());
    assert_eq!(after.revision, receipt.revision);
    assert!(
        after.authorization_view.parse::<u64>().unwrap()
            > current.authorization_view.parse::<u64>().unwrap()
    );
    assert_eq!(
        restored.account("a").await.unwrap().authorization_epoch,
        "81"
    );
    for actor in ["a", "b"] {
        assert_eq!(
            restored.account(actor).await.unwrap().state,
            AccountState::AuthRequired
        );
    }
    assert_authored(&restored, "a", A_BODY, "1").await;
    assert_authored(&restored, "b", B_BODY, "1").await;
    for table in [
        "items",
        "items_fts",
        "repositories",
        "sync_scopes",
        "scope_membership",
        "change_log",
        "account_credentials",
        "credential_cleanup",
    ] {
        assert_eq!(
            count(&target, table).await,
            0,
            "Restored provider or credential state remained in {table}"
        );
    }
    close(restored, &target).await;
    let original = Store::open(receipt.original_bundle.join("original.sqlite"))
        .await
        .unwrap();
    assert_authored(&original, "a", &newer.body, "2").await;
    assert_authored(&original, "b", B_BODY, "1").await;
    assert_eq!(
        original.credential_reference("a").await.unwrap().as_deref(),
        Some(A_REF)
    );
    close(original, &receipt.original_bundle.join("original.sqlite")).await;
}

#[derive(Default)]
struct RecordingVault {
    tokens: Mutex<HashMap<String, SecretToken>>,
    deletes: Mutex<Vec<String>>,
}

impl CredentialVault for RecordingVault {
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
        self.deletes.lock().unwrap().push(reference.into());
        self.tokens.lock().unwrap().remove(reference);
        Ok(())
    }
}

struct NoNetwork;

#[async_trait]
impl CollaborationProvider for NoNetwork {
    fn kind(&self) -> ProviderKind {
        ProviderKind::Github
    }
    async fn probe(&self, _: &SecretToken) -> Result<VerifiedAccount, ProviderError> {
        panic!("Recovery must not probe a provider")
    }
    async fn fetch_page(
        &self,
        _: &SecretToken,
        _: FeedRequest,
    ) -> Result<FetchPage, ProviderError> {
        panic!("Recovery must not fetch provider data")
    }
}

#[tokio::test]
async fn imported_cleanup_cannot_delete_current_installation_vault_entries() {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("target.sqlite");
    let backup = dir.path().join("imported.sqlite");
    let store = fixture(&target).await;
    store.backup_to(&backup).await.unwrap();
    close(store, &target).await;
    let imported_mapping = "foreign-installation-mapping-0323";
    let current_reference = "current-installation-only-vault-entry-7120";
    let mut db = connection(&backup, false).await;
    sqlx::query("INSERT INTO account_credentials VALUES('a',?)")
        .bind(imported_mapping)
        .execute(&mut db)
        .await
        .unwrap();
    sqlx::query("INSERT INTO credential_cleanup VALUES(?,'a','retired',0,0)")
        .bind(current_reference)
        .execute(&mut db)
        .await
        .unwrap();
    db.close().await.unwrap();
    let vault = Arc::new(RecordingVault::default());
    vault
        .store(
            current_reference,
            &SecretToken::new("synthetic_local_token".into()).unwrap(),
        )
        .unwrap();
    let session = RecoverySession::prepare(&target, &backup).await.unwrap();
    let id = session.preview().confirmation_id.clone();
    let receipt = session
        .confirm(&id, RestoreChoice::ReplaceCurrentData)
        .unwrap();
    assert_references_absent(&target, &[imported_mapping, current_reference]);
    assert_references_absent(
        &receipt.original_bundle.join("incoming-evidence.sqlite"),
        &[imported_mapping, current_reference],
    );
    let store = Arc::new(Store::open(&target).await.unwrap());
    let runtime = CollaborationRuntime::new(store.clone(), vault.clone(), Arc::new(NoNetwork));
    runtime.recover_credentials().await.unwrap();
    assert!(vault.deletes.lock().unwrap().is_empty());
    assert!(vault.load(current_reference).unwrap().is_some());
    assert!(store.credential_reference("a").await.unwrap().is_none());
    assert_authored(&store, "a", A_BODY, "1").await;
    store.close().await.unwrap();
}

#[tokio::test]
async fn target_change_after_preview_is_rejected_without_losing_the_new_text() {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("target.sqlite");
    let backup = dir.path().join("backup.sqlite");
    let store = fixture(&target).await;
    store.backup_to(&backup).await.unwrap();
    close(store, &target).await;
    let session = RecoverySession::prepare(&target, &backup).await.unwrap();
    let id = session.preview().confirmation_id.clone();
    // An external fixture connection deliberately bypasses the application
    // lease to model changed files between user inspection and confirmation.
    let main_before = digest(&target);
    let mut external = connection(&target, false).await;
    sqlx::query(
        "UPDATE drafts SET body='External newer authored text',generation=2 WHERE account_id='a'",
    )
    .execute(&mut external)
    .await
    .unwrap();
    assert!(std::fs::metadata(append(&target, "-wal")).unwrap().len() > 32);
    assert_eq!(
        digest(&target),
        main_before,
        "This case must exercise WAL-only stale confirmation"
    );
    assert_eq!(
        session
            .confirm(&id, RestoreChoice::ReplaceCurrentData)
            .unwrap_err()
            .code,
        ErrorCode::StaleView
    );
    assert!(!append(&target, ".restore-pending").exists());
    external.close().await.unwrap();
    let reopened = Store::open(&target).await.unwrap();
    assert_authored(&reopened, "a", "External newer authored text", "2").await;
    close(reopened, &target).await;
}

#[tokio::test]
async fn changing_selected_file_after_inspection_never_changes_the_confirmed_draft() {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("target.sqlite");
    let backup = dir.path().join("backup.sqlite");
    let store = fixture(&target).await;
    store.backup_to(&backup).await.unwrap();
    close(store, &target).await;
    let session = RecoverySession::prepare(&target, &backup).await.unwrap();
    let id = session.preview().confirmation_id.clone();
    execute(&backup, "UPDATE drafts SET body='Different backup selected after preview',generation=2 WHERE account_id='a'").await;
    session
        .confirm(&id, RestoreChoice::ReplaceCurrentData)
        .unwrap();
    let restored = Store::open(&target).await.unwrap();
    assert_authored(&restored, "a", A_BODY, "1").await;
    close(restored, &target).await;
}

#[tokio::test]
async fn unsupported_or_malformed_selected_data_never_mutates_the_original() {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("target.sqlite");
    let good = dir.path().join("good.sqlite");
    let bad = dir.path().join("bad.sqlite");
    let store = fixture(&target).await;
    store.backup_to(&good).await.unwrap();
    close(store, &target).await;
    let original = digest(&target);
    for mutation in [
        "UPDATE _sqlx_migrations SET version=9001 WHERE version=2",
        "UPDATE _sqlx_migrations SET success=0 WHERE version=2",
        "UPDATE _sqlx_migrations SET checksum=x'00' WHERE version=2",
        "UPDATE _sqlx_migrations SET checksum='unexpected-text-type' WHERE version=2",
        "UPDATE _sqlx_migrations SET version='unexpected-text-type' WHERE version=2",
        "CREATE TABLE outbox(command TEXT NOT NULL); INSERT INTO outbox VALUES('remote mutation')",
        "CREATE TRIGGER unexpected_draft_trigger AFTER UPDATE ON drafts BEGIN UPDATE runtime_meta SET revision=revision+1; END",
        "UPDATE accounts SET actor_id='actor-column-json-mismatch' WHERE id='a'",
        "UPDATE drafts SET body=x'00ff' WHERE account_id='a'",
        "UPDATE drafts SET subject_id='' WHERE account_id='a'",
        "UPDATE drafts SET body=printf('%.*c',1048577,'x') WHERE account_id='a'",
    ] {
        std::fs::copy(&good, &bad).unwrap();
        execute(&bad, mutation).await;
        let selected = digest(&bad);
        let result = RecoverySession::prepare(&target, &bad).await;
        assert!(
            result.is_err(),
            "Unreviewed recovery data was accepted: {mutation}"
        );
        assert_eq!(digest(&bad), selected, "Selected backup was modified");
        assert_eq!(digest(&target), original, "Original database was modified");
        assert!(!append(&target, ".restore-pending").exists());
    }
    let reopened = Store::open(&target).await.unwrap();
    assert_authored(&reopened, "a", A_BODY, "1").await;
    close(reopened, &target).await;
}

#[tokio::test]
async fn truncated_source_is_rejected_but_corrupt_target_is_preserved_as_raw_evidence() {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("target.sqlite");
    let backup = dir.path().join("backup.sqlite");
    let truncated = dir.path().join("truncated.sqlite");
    let store = fixture(&target).await;
    store.backup_to(&backup).await.unwrap();
    close(store, &target).await;
    let valid_target = digest(&target);
    let bytes = std::fs::read(&backup).unwrap();
    for length in [0, 37, bytes.len() / 2] {
        std::fs::write(&truncated, &bytes[..length]).unwrap();
        assert!(RecoverySession::prepare(&target, &truncated).await.is_err());
        assert_eq!(digest(&target), valid_target);
        assert!(std::fs::read(&truncated).unwrap() == bytes[..length]);
    }
    let corrupt = b"Synthetic corrupt original database, preserve these exact bytes";
    std::fs::write(&target, corrupt).unwrap();
    let session = RecoverySession::prepare(&target, &backup).await.unwrap();
    assert_eq!(session.preview().current_drafts, None);
    assert_eq!(session.preview().current_revision, None);
    let id = session.preview().confirmation_id.clone();
    let receipt = session
        .confirm(&id, RestoreChoice::ReplaceCurrentData)
        .unwrap();
    assert_eq!(
        std::fs::read(receipt.original_bundle.join("original.sqlite")).unwrap(),
        corrupt
    );
    let restored = Store::open(&target).await.unwrap();
    assert_authored(&restored, "a", A_BODY, "1").await;
    close(restored, &target).await;
}

#[tokio::test]
async fn intact_unknown_target_cannot_be_treated_as_corruption_or_disposable_cache() {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("target.sqlite");
    let backup = dir.path().join("backup.sqlite");
    let store = fixture(&target).await;
    store.backup_to(&backup).await.unwrap();
    close(store, &target).await;
    execute(&target, "CREATE TABLE outbox(command TEXT NOT NULL); INSERT INTO outbox VALUES('newer durable intent')").await;
    let original = digest(&target);
    assert!(RecoverySession::prepare(&target, &backup).await.is_err());
    assert_eq!(digest(&target), original);
    assert_eq!(count(&target, "outbox").await, 1);
    assert!(!append(&target, ".restore-pending").exists());
}

#[tokio::test]
async fn pending_recovery_blocks_bootstrap_even_when_main_database_is_missing() {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("not-yet-present.sqlite");
    let marker = append(&target, ".restore-pending");
    std::fs::create_dir(&marker).unwrap();
    std::fs::write(marker.join("original.sqlite"), b"private original evidence").unwrap();
    assert_eq!(
        Store::open(&target).await.err().unwrap().code,
        ErrorCode::Storage
    );
    assert!(
        !target.exists(),
        "Bootstrap created an empty database over interrupted recovery"
    );
    assert!(InterruptedRecovery::inspect(&target).is_err());
    assert_eq!(
        std::fs::read(marker.join("original.sqlite")).unwrap(),
        b"private original evidence"
    );
    assert!(marker.is_dir());
}

async fn interrupted_fixture(directory: &Path) -> PathBuf {
    let target = directory.join("target.sqlite");
    let backup = directory.join("old.sqlite");
    let store = fixture(&target).await;
    store.backup_to(&backup).await.unwrap();
    let mut original = store.draft("a", SUBJECT).await.unwrap().unwrap();
    original.body = "Newer original authored text retained for explicit recovery".into();
    store.save_draft(original).await.unwrap();
    close(store, &target).await;
    let session = RecoverySession::prepare(&target, &backup).await.unwrap();
    let id = session.preview().confirmation_id.clone();
    let receipt = session
        .confirm(&id, RestoreChoice::ReplaceCurrentData)
        .unwrap();
    // Use a real published bundle and its native manifest. This recreates the
    // durable state just before final archival; module subprocess tests kill
    // actual processes at those mutation boundaries separately.
    std::fs::rename(receipt.original_bundle, append(&target, ".restore-pending")).unwrap();
    target
}

#[tokio::test]
async fn interrupted_recovery_requires_fresh_confirmation_and_preserves_restored_work() {
    let dir = tempfile::tempdir().unwrap();
    let target = interrupted_fixture(dir.path()).await;
    let pending = append(&target, ".restore-pending");
    let before = digest(&target);
    let session = InterruptedRecovery::inspect(&target).unwrap();
    assert!(session.preview().original_files >= 1);
    assert!(
        session
            .preview()
            .current_data_will_remain_in_recovery_bundle
    );
    drop(session);
    assert_eq!(digest(&target), before);
    assert!(pending.is_dir());
    let session = InterruptedRecovery::inspect(&target).unwrap();
    assert_eq!(
        session
            .confirm("uninspected-recovery", RestoreChoice::KeepOriginalData)
            .unwrap_err()
            .code,
        ErrorCode::InvalidInput
    );
    assert_eq!(digest(&target), before);
    assert_eq!(
        Store::open(&target).await.err().unwrap().code,
        ErrorCode::Storage
    );
    let session = InterruptedRecovery::inspect(&target).unwrap();
    let id = session.preview().confirmation_id.clone();
    execute(&target, "UPDATE drafts SET body='Work written after the first restore',generation=2 WHERE account_id='a'").await;
    assert_eq!(
        session
            .confirm(&id, RestoreChoice::KeepOriginalData)
            .unwrap_err()
            .code,
        ErrorCode::StaleView
    );
    assert!(pending.is_dir());
    let session = InterruptedRecovery::inspect(&target).unwrap();
    let id = session.preview().confirmation_id.clone();
    let archive = session
        .confirm(&id, RestoreChoice::KeepOriginalData)
        .unwrap();
    assert!(!pending.exists());
    let discarded = std::fs::read_dir(&archive)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .find(|path| {
            path.file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("discarded-")
        })
        .unwrap();
    let preserved_work = Store::open(discarded.join("current.sqlite")).await.unwrap();
    assert_authored(
        &preserved_work,
        "a",
        "Work written after the first restore",
        "2",
    )
    .await;
    close(preserved_work, &discarded.join("current.sqlite")).await;
    let original = Store::open(&target).await.unwrap();
    assert_authored(
        &original,
        "a",
        "Newer original authored text retained for explicit recovery",
        "2",
    )
    .await;
    assert_authored(&original, "b", B_BODY, "1").await;
    close(original, &target).await;
}

#[tokio::test]
async fn changed_preserved_original_is_rejected_without_removing_the_pending_blocker() {
    let dir = tempfile::tempdir().unwrap();
    let target = interrupted_fixture(dir.path()).await;
    let pending = append(&target, ".restore-pending");
    let original = pending.join("original.sqlite");
    let target_before = digest(&target);
    // Appending evidence changes the bound checksum even when SQLite itself
    // might ignore bytes beyond its page count.
    use std::io::Write;
    std::fs::OpenOptions::new()
        .append(true)
        .open(&original)
        .unwrap()
        .write_all(b"Changed preserved original evidence")
        .unwrap();
    let changed = digest(&original);
    assert!(InterruptedRecovery::inspect(&target).is_err());
    assert_eq!(
        Store::open(&target).await.err().unwrap().code,
        ErrorCode::Storage
    );
    assert_eq!(digest(&target), target_before);
    assert_eq!(digest(&original), changed);
    assert!(pending.is_dir());
}

#[tokio::test]
async fn recognized_historical_v1_restore_migrates_staging_and_preserves_actor_drafts() {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("target.sqlite");
    let historical = dir.path().join("v1.sqlite");
    let current = fixture(&target).await;
    close(current, &target).await;
    let mut db = SqliteConnection::connect_with(
        &SqliteConnectOptions::new()
            .filename(&historical)
            .create_if_missing(true),
    )
    .await
    .unwrap();
    sqlx::raw_sql(include_str!("fixtures/migrations/v1/schema.sql"))
        .execute(&mut db)
        .await
        .unwrap();
    sqlx::raw_sql(include_str!("fixtures/migrations/v1/seed.sql"))
        .execute(&mut db)
        .await
        .unwrap();
    db.close().await.unwrap();
    let selected = digest(&historical);
    let session = RecoverySession::prepare(&target, &historical)
        .await
        .unwrap();
    assert_eq!(session.preview().incoming.schema_version, 1);
    assert_eq!(
        (
            session.preview().incoming.accounts,
            session.preview().incoming.drafts
        ),
        (4, 4)
    );
    let id = session.preview().confirmation_id.clone();
    session
        .confirm(&id, RestoreChoice::ReplaceCurrentData)
        .unwrap();
    assert_eq!(digest(&historical), selected);
    let restored = Store::open(&target).await.unwrap();
    let a = restored
        .draft("a", "pull-request-67")
        .await
        .unwrap()
        .unwrap();
    let b = restored
        .draft("b", "pull-request-67")
        .await
        .unwrap()
        .unwrap();
    assert!(a.body.starts_with("Alice unsent draft with a newline\n"));
    assert_eq!(a.generation, "37");
    assert_eq!(b.body, "Bob independent unsent draft");
    assert_eq!(b.generation, "51");
    assert_authored(&restored, "c", "Carol draft after disconnect", "63").await;
    assert_authored(&restored, "d", "Dave draft awaiting reconnection", "79").await;
    assert_eq!(restored.accounts().await.unwrap().accounts.len(), 4);
    assert_eq!(count(&target, "_sqlx_migrations").await, 19);
    assert_eq!(count(&target, "account_credentials").await, 0);
    assert_eq!(count(&target, "credential_cleanup").await, 0);
    close(restored, &target).await;
}
