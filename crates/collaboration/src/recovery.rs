//! Native-only, explicit recovery. No provider or vault is touched here.
//! Desktop restore integration must first own runtime shutdown; a running Store
//! deliberately prevents construction of a RecoverySession through its lease.
use std::{
    collections::HashMap,
    fs::{File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
};

use futures_util::TryStreamExt;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::{
    Connection, Decode, Row, Sqlite, SqliteConnection, Type,
    sqlite::{SqliteConnectOptions, SqliteRow},
};
use tempfile::{TempDir, TempPath};
use uuid::Uuid;

use crate::{
    AccountState, CollaborationError, ErrorCode, RemoteAccount,
    storage::{MIGRATIONS, WriterLease, acquire_writer_lease, prepare_private_path, validate_identifier},
};

type Result<T> = std::result::Result<T, CollaborationError>;
// Raising this requires a reviewed restore policy, especially for future outbox
// tables. Merely adding a migration does not authorize replay of imported data.
const RESTORE_SCHEMA_POLICY: i64 = 2;
const MAX_DATABASE_BYTES: u64 = 512 * 1024 * 1024;
const SIDECARS: [&str; 3] = ["", "-wal", "-shm"];

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackupSummary {
    pub sha256: String,
    pub revision: String,
    pub schema_version: i64,
    pub accounts: u64,
    pub drafts: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RestorePreview {
    pub confirmation_id: String,
    pub incoming: BackupSummary,
    pub current_revision: Option<String>,
    pub current_drafts: Option<u64>,
    pub current_accounts: Option<u64>,
    /// Incoming drafts replace the active database's drafts. Newer current text
    /// remains in the original recovery bundle; it is not automatically merged.
    pub newer_current_drafts_remain_in_original_bundle: bool,
    pub reauthentication_required: bool,
    pub cached_provider_data_will_be_removed: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RestoreChoice {
    ReplaceCurrentData,
    KeepOriginalData,
}

#[derive(Debug)]
pub struct RestoreReceipt {
    pub original_bundle: PathBuf,
    pub revision: String,
}

pub struct RecoverySession {
    _lease: WriterLease,
    stage: TempDir,
    target: PathBuf,
    candidate: PathBuf,
    target_fingerprint: String,
    candidate_fingerprint: String,
    preview: RestorePreview,
    restored_revision: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct PreservedFile {
    suffix: String,
    sha256: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct Manifest {
    format: u32,
    confirmation_id: String,
    candidate_sha256: String,
    original: Vec<PreservedFile>,
}

struct CurrentState {
    summary: BackupSummary,
    authorization_view: i64,
    epochs: HashMap<String, (String, String, String, i64)>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct InterruptedPreview {
    pub confirmation_id: String,
    pub original_files: u32,
    pub current_data_will_remain_in_recovery_bundle: bool,
}

pub struct InterruptedRecovery {
    _lease: WriterLease,
    target: PathBuf,
    pending: PathBuf,
    manifest: Manifest,
    fingerprint: String,
    preview: InterruptedPreview,
}

impl RecoverySession {
    /// Inspect and stage a selected backup without modifying either input.
    /// Paths stay native; callers must not expose arbitrary-path IPC commands.
    pub async fn prepare(target: impl AsRef<Path>, backup: impl AsRef<Path>) -> Result<Self> {
        let target = target.as_ref();
        require_regular(target)?;
        prepare_private_path(target)?;
        let lease = acquire_writer_lease(target)?;
        require_no_pending_restore(target)?;
        let fingerprint = database_fingerprint(target)?;
        let current = read_current(target).await?;
        // A selected export is a standalone snapshot, never the main file of a
        // live SQLite database whose committed WAL frames would be omitted.
        if append(backup.as_ref(), "-wal").exists()
            && std::fs::metadata(append(backup.as_ref(), "-wal"))
                .map_err(|_| invalid_backup())?
                .len()
                > 0
        {
            return Err(invalid_backup());
        }
        let stage = private_stage(parent(target))?;
        let staged = stage.path().join("selected.sqlite");
        copy_private(backup.as_ref(), &staged)?;
        let mut connection = connect(&staged, false).await?;
        let version = verify(&mut connection).await?;
        let incoming = summary(&mut connection, version, hash_file(&staged)?).await?;
        MIGRATIONS
            .run_direct(None, &mut connection, false)
            .await
            .map_err(|_| invalid_backup())?;
        verify(&mut connection).await?;
        let restored_revision = fence_restored_data(&mut connection, current.as_ref()).await?;
        let candidate = stage.path().join("candidate.sqlite");
        vacuum_into(&mut connection, &candidate).await?;
        connection.close().await.map_err(|_| storage())?;
        let mut checked = connect(&candidate, true).await?;
        verify(&mut checked).await?;
        checked.close().await.map_err(|_| storage())?;
        sync_file(&candidate)?;
        let candidate_fingerprint = hash_file(&candidate)?;
        if fingerprint != database_fingerprint(target)? {
            return Err(stale());
        }
        let preview = RestorePreview {
            confirmation_id: Uuid::new_v4().to_string(),
            incoming,
            current_revision: current.as_ref().map(|s| s.summary.revision.clone()),
            current_drafts: current.as_ref().map(|s| s.summary.drafts),
            current_accounts: current.as_ref().map(|s| s.summary.accounts),
            newer_current_drafts_remain_in_original_bundle: true,
            reauthentication_required: true,
            cached_provider_data_will_be_removed: true,
        };
        Ok(Self {
            _lease: lease,
            stage,
            target: target.to_owned(),
            candidate,
            target_fingerprint: fingerprint,
            candidate_fingerprint,
            preview,
            restored_revision,
        })
    }

    pub fn preview(&self) -> &RestorePreview {
        &self.preview
    }

    /// Consume one exact preview. Cancellation/dropping the session does not
    /// replace storage. After the durable marker, errors require explicit
    /// interrupted recovery; bootstrap never guesses or silently resets.
    pub fn confirm(self, confirmation_id: &str, choice: RestoreChoice) -> Result<RestoreReceipt> {
        if choice != RestoreChoice::ReplaceCurrentData
            || confirmation_id != self.preview.confirmation_id
        {
            return Err(CollaborationError::invalid(
                "Confirm the inspected restore preview",
            ));
        }
        if database_fingerprint(&self.target)? != self.target_fingerprint
            || hash_file(&self.candidate)? != self.candidate_fingerprint
        {
            return Err(stale());
        }
        let pending = pending_path(&self.target);
        require_no_pending_restore(&self.target)?;
        let bundle = private_stage(parent(&self.target))?;
        let bundle_path = bundle.path();
        let mut originals = Vec::new();
        // A killed partial copy stays private staging, never a pending marker.
        // Publish only a complete, verified manifest before changing the target.
        (|| {
            for suffix in SIDECARS {
                let source = append(&self.target, suffix);
                if !source.exists() {
                    continue;
                }
                let destination = append(&bundle_path.join("original.sqlite"), suffix);
                copy_private(&source, &destination)?;
                let checksum = hash_file(&source)?;
                if checksum != hash_file(&destination)? {
                    return Err(stale());
                }
                originals.push(PreservedFile {
                    suffix: suffix.into(),
                    sha256: checksum,
                });
            }
            if database_fingerprint(&self.target)? != self.target_fingerprint {
                return Err(stale());
            }
            copy_private(&self.candidate, &bundle_path.join("candidate.sqlite"))?;
            if hash_file(&bundle_path.join("candidate.sqlite"))? != self.candidate_fingerprint {
                return Err(stale());
            }
            let manifest = Manifest {
                format: 1,
                confirmation_id: self.preview.confirmation_id.clone(),
                candidate_sha256: self.candidate_fingerprint.clone(),
                original: originals,
            };
            write_manifest(bundle_path, &manifest)?;
            sync_directory(bundle_path)?;
            sync_directory(parent(&self.target))?;
            Ok(())
        })()?;
        checkpoint("restore_bundle_verified");
        std::fs::rename(bundle_path, &pending).map_err(|_| storage())?;
        sync_directory(parent(&self.target))?;
        checkpoint("restore_prepared");
        // Old WAL frames must never be replayed against a replacement main DB.
        remove_sidecars(&self.target)?;
        checkpoint("restore_sidecars_removed");
        atomic_install(&self.candidate, &self.target, &self.candidate_fingerprint)?;
        sync_directory(parent(&self.target))?;
        checkpoint("restore_installed");
        let archive = archive_pending(&self.target, &pending)?;
        // Keep the stage alive until every operation completes; its RAII cleanup
        // cannot remove the durable original bundle.
        drop(self.stage);
        Ok(RestoreReceipt {
            original_bundle: archive,
            revision: self.restored_revision,
        })
    }
}

impl InterruptedRecovery {
    /// A separate preview/choice is required after any interrupted replacement.
    pub fn inspect(target: impl AsRef<Path>) -> Result<Self> {
        let target = target.as_ref();
        prepare_private_path(target)?;
        let lease = acquire_writer_lease(target)?;
        let pending = pending_path(target);
        let manifest = read_manifest(&pending)?;
        verify_original(&pending, &manifest)?;
        let fingerprint = interrupted_fingerprint(target, &pending)?;
        let preview = InterruptedPreview {
            confirmation_id: Uuid::new_v4().to_string(),
            original_files: manifest.original.len() as u32,
            current_data_will_remain_in_recovery_bundle: true,
        };
        Ok(Self {
            _lease: lease,
            target: target.to_owned(),
            pending,
            manifest,
            fingerprint,
            preview,
        })
    }

    pub fn preview(&self) -> &InterruptedPreview {
        &self.preview
    }

    pub fn confirm(self, confirmation_id: &str, choice: RestoreChoice) -> Result<PathBuf> {
        if choice != RestoreChoice::KeepOriginalData
            || confirmation_id != self.preview.confirmation_id
        {
            return Err(CollaborationError::invalid(
                "Confirm the inspected interrupted recovery",
            ));
        }
        if self.fingerprint != interrupted_fingerprint(&self.target, &self.pending)? {
            return Err(stale());
        }
        verify_original(&self.pending, &self.manifest)?;
        let stage = private_stage(parent(&self.target))?;
        for original in &self.manifest.original {
            copy_private(
                &append(&self.pending.join("original.sqlite"), &original.suffix),
                &append(&stage.path().join("original.sqlite"), &original.suffix),
            )?;
        }
        // Preserve the present candidate/evidence too, without overwriting an
        // earlier interrupted rollback attempt's evidence.
        let discarded = self.pending.join(format!("discarded-{}", Uuid::new_v4()));
        create_private_directory(&discarded)?;
        for suffix in SIDECARS {
            let source = append(&self.target, suffix);
            if source.exists() {
                copy_private(&source, &append(&discarded.join("current.sqlite"), suffix))?;
            }
        }
        sync_directory(&discarded)?;
        sync_directory(&self.pending)?;
        remove_sidecars(&self.target)?;
        checkpoint("rollback_sidecars_removed");
        let main_checksum = &self
            .manifest
            .original
            .iter()
            .find(|f| f.suffix.is_empty())
            .ok_or_else(invalid_backup)?
            .sha256;
        atomic_install(
            &stage.path().join("original.sqlite"),
            &self.target,
            main_checksum,
        )?;
        checkpoint("rollback_main_installed");
        for original in &self.manifest.original {
            if original.suffix.is_empty() {
                continue;
            }
            atomic_install(
                &append(&stage.path().join("original.sqlite"), &original.suffix),
                &append(&self.target, &original.suffix),
                &original.sha256,
            )?;
            checkpoint("rollback_sidecar_installed");
        }
        sync_directory(parent(&self.target))?;
        archive_pending(&self.target, &self.pending)
    }
}

pub(crate) async fn backup_from(
    connection: &mut SqliteConnection,
    destination: &Path,
) -> Result<BackupSummary> {
    if destination.exists() {
        return Err(CollaborationError::invalid("Choose a new backup filename"));
    }
    prepare_private_path(destination)?;
    let stage = private_stage(parent(destination))?;
    let snapshot = stage.path().join("snapshot.sqlite");
    vacuum_into(connection, &snapshot).await?;
    checkpoint("backup_snapshot");
    let mut snapshot_connection = connect(&snapshot, false).await?;
    verify(&mut snapshot_connection).await?;
    sqlx::raw_sql("DELETE FROM account_credentials; DELETE FROM credential_cleanup;")
        .execute(&mut snapshot_connection)
        .await
        .map_err(|_| storage())?;
    let export = stage.path().join("export.sqlite");
    vacuum_into(&mut snapshot_connection, &export).await?;
    snapshot_connection.close().await.map_err(|_| storage())?;
    let mut checked = connect(&export, true).await?;
    let version = verify(&mut checked).await?;
    let summary = summary(&mut checked, version, hash_file(&export)?).await?;
    checked.close().await.map_err(|_| storage())?;
    sync_file(&export)?;
    checkpoint("backup_verified");
    // Same-filesystem hard link publishes an already complete file atomically,
    // with create-new semantics on all supported desktop platforms.
    std::fs::hard_link(&export, destination).map_err(|_| storage())?;
    sync_directory(parent(destination))?;
    Ok(summary)
}

pub(crate) fn require_no_pending_restore(path: &Path) -> Result<()> {
    if std::fs::symlink_metadata(pending_path(path)).is_ok() {
        return Err(CollaborationError::new(
            ErrorCode::Storage,
            "Collaboration restore was interrupted; inspect the preserved recovery bundle before opening storage",
        ));
    }
    Ok(())
}

async fn connect(path: &Path, read_only: bool) -> Result<SqliteConnection> {
    require_regular(path)?;
    SqliteConnection::connect_with(
        &SqliteConnectOptions::new()
            .filename(path)
            .read_only(read_only)
            .foreign_keys(true)
            .pragma("trusted_schema", "OFF")
            .pragma("synchronous", "FULL"),
    )
    .await
    .map_err(|_| invalid_backup())
}

async fn read_current(path: &Path) -> Result<Option<CurrentState>> {
    let Ok(mut connection) = connect(path, true).await else {
        // Physical corruption is recoverable through explicit replacement; the
        // original bytes and sidecars are retained without guessing vault refs.
        return Ok(None);
    };
    let intact = sqlx::query_scalar::<_, String>("PRAGMA integrity_check(1)")
        .fetch_all(&mut connection)
        .await
        .is_ok_and(|rows| rows == ["ok"]);
    if !intact {
        connection.close().await.map_err(|_| storage())?;
        return Ok(None);
    }
    // An intact newer/unknown database cannot be treated as disposable cache or
    // as physical corruption. Its durable intent requires a compatible policy.
    let version = verify(&mut connection).await?;
    let summary = summary(&mut connection, version, String::new()).await?;
    let authorization_view =
        sqlx::query_scalar("SELECT authorization_view FROM runtime_meta WHERE singleton=1")
            .fetch_one(&mut connection)
            .await
            .map_err(|_| invalid_backup())?;
    let rows = sqlx::query("SELECT id,provider,host,actor_id,authorization_epoch FROM accounts")
        .fetch_all(&mut connection)
        .await
        .map_err(|_| invalid_backup())?;
    let epochs = rows
        .into_iter()
        .map(|row| {
            Ok((
                column(&row, "id")?,
                (
                    column(&row, "provider")?,
                    column(&row, "host")?,
                    column(&row, "actor_id")?,
                    column(&row, "authorization_epoch")?,
                ),
            ))
        })
        .collect::<Result<_>>()?;
    connection.close().await.map_err(|_| storage())?;
    Ok(Some(CurrentState {
        summary,
        authorization_view,
        epochs,
    }))
}

async fn summary(
    connection: &mut SqliteConnection,
    version: i64,
    checksum: String,
) -> Result<BackupSummary> {
    let revision: i64 = sqlx::query_scalar("SELECT revision FROM runtime_meta WHERE singleton=1")
        .fetch_one(&mut *connection)
        .await
        .map_err(|_| invalid_backup())?;
    let accounts: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM accounts")
        .fetch_one(&mut *connection)
        .await
        .map_err(|_| invalid_backup())?;
    let drafts: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM drafts")
        .fetch_one(connection)
        .await
        .map_err(|_| invalid_backup())?;
    Ok(BackupSummary {
        sha256: checksum,
        revision: revision.to_string(),
        schema_version: version,
        accounts: accounts as u64,
        drafts: drafts as u64,
    })
}

async fn verify(connection: &mut SqliteConnection) -> Result<i64> {
    let integrity: Vec<String> = sqlx::query_scalar("PRAGMA integrity_check(1)")
        .fetch_all(&mut *connection)
        .await
        .map_err(|_| invalid_backup())?;
    if integrity != ["ok"] {
        return Err(invalid_backup());
    }
    if !sqlx::query("PRAGMA foreign_key_check")
        .fetch_all(&mut *connection)
        .await
        .map_err(|_| invalid_backup())?
        .is_empty()
    {
        return Err(invalid_backup());
    }
    let rows =
        sqlx::query("SELECT version,success,checksum FROM _sqlx_migrations ORDER BY version")
            .fetch_all(&mut *connection)
            .await
            .map_err(|_| invalid_backup())?;
    let Some(last) = rows.last() else {
        return Err(invalid_backup());
    };
    let version: i64 = column(last, "version")?;
    if !(1..=RESTORE_SCHEMA_POLICY).contains(&version) || rows.len() != version as usize {
        return Err(invalid_backup());
    }
    for (row, migration) in rows.iter().zip(MIGRATIONS.iter()) {
        if column::<i64>(row, "version")? != migration.version
            || !column::<bool>(row, "success")?
            || column::<Vec<u8>>(row, "checksum")?.as_slice() != migration.checksum.as_ref()
        {
            return Err(invalid_backup());
        }
    }
    // Compare structural SQL against a clean known schema, not only its ledger.
    // Unknown tables/triggers and outbox payloads are denied until policy review.
    let mut expected = SqliteConnection::connect(":memory:")
        .await
        .map_err(|_| storage())?;
    for migration in MIGRATIONS.iter().filter(|m| m.version <= version) {
        sqlx::raw_sql(migration.sql.as_ref())
            .execute(&mut expected)
            .await
            .map_err(|_| storage())?;
    }
    let schema = "SELECT json_array(type,name,tbl_name,sql) FROM sqlite_schema WHERE name NOT LIKE 'sqlite_%' AND name<>'_sqlx_migrations' ORDER BY name";
    let actual: Vec<String> = sqlx::query_scalar(schema)
        .fetch_all(&mut *connection)
        .await
        .map_err(|_| invalid_backup())?;
    let known: Vec<String> = sqlx::query_scalar(schema)
        .fetch_all(&mut expected)
        .await
        .map_err(|_| storage())?;
    expected.close().await.map_err(|_| storage())?;
    if actual != known {
        return Err(invalid_backup());
    }
    // Validate authored records and account/JSON identity before any candidate is
    // admitted. The low account bound matches Store's published contract.
    let accounts = sqlx::query(
        "SELECT id,provider,host,actor_id,authorization_epoch,state,json FROM accounts LIMIT 101",
    )
    .fetch_all(&mut *connection)
    .await
    .map_err(|_| invalid_backup())?;
    if accounts.len() > 100 {
        return Err(invalid_backup());
    }
    for row in accounts {
        let account: RemoteAccount =
            serde_json::from_str(&column::<String>(&row, "json")?).map_err(|_| invalid_backup())?;
        if account.id != column::<String>(&row, "id")?
            || account.host != column::<String>(&row, "host")?
            || account.actor_id != column::<String>(&row, "actor_id")?
            || serde_json::to_value(&account.provider)
                .map_err(|_| invalid_backup())?
                .as_str()
                != Some(column::<String>(&row, "provider")?.as_str())
            || serde_json::to_value(&account.state)
                .map_err(|_| invalid_backup())?
                .as_str()
                != Some(column::<String>(&row, "state")?.as_str())
            || account.authorization_epoch.parse::<i64>().ok()
                != Some(column(&row, "authorization_epoch")?)
        {
            return Err(invalid_backup());
        }
    }
    let mut drafts = sqlx::query("SELECT account_id,subject_id,body,generation FROM drafts")
        .fetch(&mut *connection);
    while let Some(row) = drafts.try_next().await.map_err(|_| invalid_backup())? {
        let account: String = column(&row, "account_id")?;
        let subject: String = column(&row, "subject_id")?;
        let body: String = column(&row, "body")?;
        let generation: i64 = column(&row, "generation")?;
        if validate_identifier(&account).is_err()
            || validate_identifier(&subject).is_err()
            || body.len() > 1_048_576
            || generation <= 0
        {
            return Err(invalid_backup());
        }
    }
    Ok(version)
}

async fn fence_restored_data(
    connection: &mut SqliteConnection,
    current: Option<&CurrentState>,
) -> Result<String> {
    let mut tx = connection.begin().await.map_err(|_| storage())?;
    for sql in [
        "DELETE FROM account_credentials",
        "DELETE FROM credential_cleanup",
        "DELETE FROM items_fts",
        "DELETE FROM items",
        "DELETE FROM scope_membership",
        "DELETE FROM sync_scopes",
        "DELETE FROM repositories",
        "DELETE FROM change_log",
    ] {
        sqlx::query(sql)
            .execute(&mut *tx)
            .await
            .map_err(|_| storage())?;
    }
    let accounts = sqlx::query("SELECT json FROM accounts")
        .fetch_all(&mut *tx)
        .await
        .map_err(|_| storage())?;
    for row in accounts {
        let mut account: RemoteAccount =
            serde_json::from_str(&column::<String>(&row, "json")?).map_err(|_| invalid_backup())?;
        let previous_epoch = account
            .authorization_epoch
            .parse::<i64>()
            .map_err(|_| invalid_backup())?;
        let target_epoch = current
            .and_then(|state| state.epochs.get(&account.id))
            .filter(|(provider, host, actor, _)| {
                account.host == *host
                    && account.actor_id == *actor
                    && serde_json::to_value(&account.provider)
                        .ok()
                        .and_then(|v| v.as_str().map(str::to_owned))
                        .as_deref()
                        == Some(provider.as_str())
            })
            .map(|(_, _, _, epoch)| *epoch)
            .unwrap_or(previous_epoch);
        let epoch = previous_epoch
            .max(target_epoch)
            .checked_add(1)
            .ok_or_else(invalid_backup)?;
        account.authorization_epoch = epoch.to_string();
        account.state = AccountState::AuthRequired;
        sqlx::query(
            "UPDATE accounts SET authorization_epoch=?,state='auth_required',json=? WHERE id=?",
        )
        .bind(epoch)
        .bind(serde_json::to_string(&account).map_err(|_| invalid_backup())?)
        .bind(account.id)
        .execute(&mut *tx)
        .await
        .map_err(|_| storage())?;
    }
    let previous: i64 = sqlx::query_scalar("SELECT revision FROM runtime_meta WHERE singleton=1")
        .fetch_one(&mut *tx)
        .await
        .map_err(|_| storage())?;
    let current_revision = current
        .and_then(|state| state.summary.revision.parse::<i64>().ok())
        .unwrap_or(previous);
    let revision = previous
        .max(current_revision)
        .checked_add(1)
        .ok_or_else(invalid_backup)?;
    let source_view: i64 =
        sqlx::query_scalar("SELECT authorization_view FROM runtime_meta WHERE singleton=1")
            .fetch_one(&mut *tx)
            .await
            .map_err(|_| storage())?;
    let view = source_view
        .max(
            current
                .map(|state| state.authorization_view)
                .unwrap_or(source_view),
        )
        .checked_add(1)
        .ok_or_else(invalid_backup)?;
    let changed = sqlx::query(
        "UPDATE runtime_meta SET revision=?,authorization_view=?,log_floor=? WHERE singleton=1",
    )
    .bind(revision)
    .bind(view)
    .bind(revision)
    .execute(&mut *tx)
    .await
    .map_err(|_| storage())?;
    if changed.rows_affected() != 1 {
        return Err(invalid_backup());
    }
    tx.commit().await.map_err(|_| storage())?;
    Ok(revision.to_string())
}

async fn vacuum_into(connection: &mut SqliteConnection, path: &Path) -> Result<()> {
    let path_text = path
        .to_str()
        .ok_or_else(|| CollaborationError::invalid("Choose a UTF-8 recovery path"))?;
    sqlx::query("VACUUM INTO ?")
        .bind(path_text)
        .execute(connection)
        .await
        .map_err(|_| storage())?;
    require_regular(path)?;
    set_private_permissions(path)?;
    Ok(())
}

fn parent(path: &Path) -> &Path {
    path.parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."))
}
fn append(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.as_os_str().to_os_string();
    name.push(suffix);
    PathBuf::from(name)
}
fn pending_path(path: &Path) -> PathBuf {
    append(path, ".restore-pending")
}
fn private_stage(directory: &Path) -> Result<TempDir> {
    tempfile::Builder::new()
        .prefix(".gitru-recovery-")
        .tempdir_in(directory)
        .map_err(|_| storage())
}
fn create_private_directory(path: &Path) -> Result<()> {
    let mut builder = std::fs::DirBuilder::new();
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    builder.create(path).map_err(|_| storage())
}
fn require_regular(path: &Path) -> Result<()> {
    let metadata = std::fs::symlink_metadata(path).map_err(|_| invalid_backup())?;
    if !metadata.is_file()
        || metadata.file_type().is_symlink()
        || metadata.len() > MAX_DATABASE_BYTES
    {
        return Err(invalid_backup());
    }
    Ok(())
}
fn set_private_permissions(path: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
            .map_err(|_| storage())?;
    }
    #[cfg(not(unix))]
    let _ = path;
    Ok(())
}
fn copy_private(source: &Path, destination: &Path) -> Result<()> {
    require_regular(source)?;
    let mut input = File::open(source).map_err(|_| storage())?;
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut output = options.open(destination).map_err(|_| storage())?;
    std::io::copy(&mut input, &mut output).map_err(|_| storage())?;
    output.sync_all().map_err(|_| storage())
}
fn hash_file(path: &Path) -> Result<String> {
    require_regular(path)?;
    let mut file = File::open(path).map_err(|_| storage())?;
    let mut hash = Sha256::new();
    let mut buffer = [0; 16 * 1024];
    loop {
        let read = file.read(&mut buffer).map_err(|_| storage())?;
        if read == 0 {
            break;
        }
        hash.update(&buffer[..read]);
    }
    Ok(format!("{:x}", hash.finalize()))
}
fn database_fingerprint(path: &Path) -> Result<String> {
    let mut hash = Sha256::new();
    // SHM is a derived coordination index and may change during a read-only
    // preview. Main+WAL bind all durable content without false stale decisions.
    for suffix in ["", "-wal"] {
        let file = append(path, suffix);
        hash.update(suffix.as_bytes());
        // Opening a WAL-mode database for read-only inspection may create an
        // empty WAL. Empty and absent WALs both contain no durable frames.
        if file.exists()
            && (suffix.is_empty() || std::fs::metadata(&file).map_err(|_| storage())?.len() > 0)
        {
            hash.update(hash_file(&file)?.as_bytes());
        } else {
            hash.update(b"missing");
        }
    }
    Ok(format!("{:x}", hash.finalize()))
}
fn interrupted_fingerprint(target: &Path, pending: &Path) -> Result<String> {
    let mut hash = Sha256::new();
    hash.update(database_fingerprint(target)?.as_bytes());
    hash.update(hash_file(&pending.join("manifest.json"))?.as_bytes());
    for suffix in SIDECARS {
        let file = append(&pending.join("original.sqlite"), suffix);
        if file.exists() {
            hash.update(suffix.as_bytes());
            hash.update(hash_file(&file)?.as_bytes());
        }
    }
    Ok(format!("{:x}", hash.finalize()))
}
fn write_manifest(pending: &Path, manifest: &Manifest) -> Result<()> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options
        .open(pending.join("manifest.json"))
        .map_err(|_| storage())?;
    file.write_all(&serde_json::to_vec(manifest).map_err(|_| storage())?)
        .map_err(|_| storage())?;
    file.sync_all().map_err(|_| storage())
}
fn read_manifest(pending: &Path) -> Result<Manifest> {
    let metadata = std::fs::symlink_metadata(pending).map_err(|_| invalid_backup())?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(invalid_backup());
    }
    let manifest_path = pending.join("manifest.json");
    require_regular(&manifest_path)?;
    if std::fs::metadata(&manifest_path)
        .map_err(|_| storage())?
        .len()
        > 16_384
    {
        return Err(invalid_backup());
    }
    let manifest: Manifest =
        serde_json::from_reader(File::open(manifest_path).map_err(|_| storage())?)
            .map_err(|_| invalid_backup())?;
    if manifest.format != 1
        || manifest.original.is_empty()
        || manifest.original.len() > 3
        || !manifest.original.iter().any(|f| f.suffix.is_empty())
        || manifest
            .original
            .iter()
            .any(|f| !SIDECARS.contains(&f.suffix.as_str()))
    {
        return Err(invalid_backup());
    }
    for (i, file) in manifest.original.iter().enumerate() {
        if manifest.original[..i]
            .iter()
            .any(|f| f.suffix == file.suffix)
        {
            return Err(invalid_backup());
        }
    }
    Ok(manifest)
}
fn verify_original(pending: &Path, manifest: &Manifest) -> Result<()> {
    for original in &manifest.original {
        if hash_file(&append(&pending.join("original.sqlite"), &original.suffix))?
            != original.sha256
        {
            return Err(invalid_backup());
        }
    }
    Ok(())
}
fn remove_sidecars(path: &Path) -> Result<()> {
    for suffix in ["-wal", "-shm"] {
        let file = append(path, suffix);
        match std::fs::symlink_metadata(&file) {
            Ok(metadata) if metadata.is_file() && !metadata.file_type().is_symlink() => {
                std::fs::remove_file(file).map_err(|_| storage())?
            }
            Ok(_) => return Err(storage()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => return Err(storage()),
        }
    }
    Ok(())
}
fn atomic_install(source: &Path, destination: &Path, expected_checksum: &str) -> Result<()> {
    // Source already lives in private same-filesystem staging. Consume that
    // synced file by atomic rename; do not allocate another full DB copy after
    // deleting old sidecars, which would create an avoidable disk-full boundary.
    sync_file(source)?;
    if hash_file(source)? != expected_checksum {
        return Err(invalid_backup());
    }
    TempPath::try_from_path(source.to_owned())
        .map_err(|_| storage())?
        .persist(destination)
        .map_err(|_| storage())?;
    sync_file(destination)
}
fn archive_pending(target: &Path, pending: &Path) -> Result<PathBuf> {
    let archive = append(target, &format!(".restore-original-{}", Uuid::new_v4()));
    std::fs::rename(pending, &archive).map_err(|_| storage())?;
    if let Err(error) = sync_directory(parent(target)) {
        // Keep the conservative startup blocker if final archive publication
        // cannot be acknowledged. The installed DB and original bundle were
        // already synced before the rename, even if rollback rename also fails.
        let _ = std::fs::rename(&archive, pending);
        let _ = sync_directory(parent(target));
        return Err(error);
    }
    Ok(archive)
}
fn sync_file(path: &Path) -> Result<()> {
    OpenOptions::new()
        .write(true)
        .open(path)
        .and_then(|f| f.sync_all())
        .map_err(|_| storage())
}
fn sync_directory(path: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        File::open(path)
            .and_then(|f| f.sync_all())
            .map_err(|_| storage())
    }
    // Windows file FlushFileBuffers is used above; directory fsync has no std
    // equivalent. Power-loss/rename durability remains a platform release gate.
    #[cfg(not(unix))]
    {
        let _ = path;
        Ok(())
    }
}
fn invalid_backup() -> CollaborationError {
    CollaborationError::new(
        ErrorCode::Storage,
        "The selected recovery data is corrupt or uses an unsupported schema; existing data was preserved",
    )
}
fn stale() -> CollaborationError {
    CollaborationError::new(
        ErrorCode::StaleView,
        "Recovery data changed after inspection; inspect a new preview",
    )
}
fn storage() -> CollaborationError {
    CollaborationError::storage()
}

fn column<T>(row: &SqliteRow, name: &str) -> Result<T>
where
    T: for<'r> Decode<'r, Sqlite> + Type<Sqlite>,
{
    row.try_get(name).map_err(|_| invalid_backup())
}

fn checkpoint(_name: &str) {
    #[cfg(test)]
    tests::checkpoint(_name);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{LocalDraft, Store};
    use std::{
        process::{Child, Command, Stdio},
        time::{Duration, Instant},
    };

    const CHILD: &str = "recovery::tests::recovery_crash_child";

    pub(super) fn checkpoint(name: &str) {
        if std::env::var("GITRU_RECOVERY_BOUNDARY").as_deref() != Ok(name) {
            return;
        }
        let marker = std::env::var_os("GITRU_RECOVERY_MARKER").unwrap();
        std::fs::write(marker, name).unwrap();
        loop {
            std::thread::park();
        }
    }

    async fn fixture(path: &Path) {
        let mut connection = SqliteConnection::connect_with(
            &SqliteConnectOptions::new()
                .filename(path)
                .create_if_missing(true),
        )
        .await
        .unwrap();
        sqlx::raw_sql(include_str!("../tests/fixtures/migrations/v1/schema.sql"))
            .execute(&mut connection)
            .await
            .unwrap();
        sqlx::raw_sql(include_str!("../tests/fixtures/migrations/v1/seed.sql"))
            .execute(&mut connection)
            .await
            .unwrap();
        connection.close().await.unwrap();
        let store = Store::open(path).await.unwrap();
        store.close().await;
        drop(store);
    }

    async fn scenario(root: &Path) {
        let target = root.join("target.sqlite");
        fixture(&target).await;
        let store = Store::open(&target).await.unwrap();
        store.backup_to(root.join("backup.sqlite")).await.unwrap();
        store
            .save_draft(LocalDraft {
                account_id: "a".into(),
                subject_id: "pull-request-67".into(),
                body: "Newer current draft must survive every interrupted restore 🦀".into(),
                generation: "37".into(),
            })
            .await
            .unwrap();
        store.close().await;
        drop(store);
    }

    struct ChildGuard(Child);
    impl Drop for ChildGuard {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }

    async fn kill_at(root: &Path, boundary: &str, mode: &str) {
        let executable = root.join(format!("child-{}.exe", Uuid::new_v4()));
        #[cfg(unix)]
        {
            // Only the waited child may open the executable for writing;
            // concurrent parent forks cannot inherit its writable descriptor.
            let copied = Command::new("/bin/cp")
                .arg(std::env::current_exe().unwrap())
                .arg(&executable)
                .env_clear()
                .current_dir(root)
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .expect("recovery fixture snapshot writer starts");
            assert!(
                copied.success(),
                "recovery fixture snapshot writer succeeds"
            );
        }
        #[cfg(not(unix))]
        std::fs::copy(std::env::current_exe().unwrap(), &executable).unwrap();
        let marker = root.join(format!("reached-{}.txt", Uuid::new_v4()));
        let log_path = root.join(format!("child-{}.log", Uuid::new_v4()));
        let log = File::create(&log_path).unwrap();
        let mut child = ChildGuard(
            Command::new(executable)
                .args(["--exact", CHILD, "--ignored", "--nocapture"])
                .env("GITRU_RECOVERY_CHILD_ROOT", root)
                .env("GITRU_RECOVERY_BOUNDARY", boundary)
                .env("GITRU_RECOVERY_MARKER", &marker)
                .env("GITRU_RECOVERY_CHILD_MODE", mode)
                .stdin(Stdio::null())
                .stdout(log.try_clone().unwrap())
                .stderr(log)
                .spawn()
                .unwrap(),
        );
        let deadline = Instant::now() + Duration::from_secs(10);
        while !marker.exists() {
            assert!(
                child.0.try_wait().unwrap().is_none(),
                "child exited before {boundary}: {}",
                std::fs::read_to_string(&log_path).unwrap()
            );
            assert!(
                Instant::now() < deadline,
                "child did not reach {boundary}: {}",
                std::fs::read_to_string(&log_path).unwrap()
            );
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        assert!(
            matches!(Store::open(root.join("target.sqlite")).await,
            Err(error) if error.code == ErrorCode::Busy),
            "child owns the real writer lease"
        );
        child.0.kill().unwrap();
        child.0.wait().unwrap();
    }

    async fn assert_newer_draft(path: &Path) {
        let store = Store::open(path).await.unwrap();
        let draft = store.draft("a", "pull-request-67").await.unwrap().unwrap();
        assert_eq!(draft.generation, "38");
        assert_eq!(
            draft.body,
            "Newer current draft must survive every interrupted restore 🦀"
        );
        store.close().await;
        drop(store);
    }

    #[tokio::test]
    #[ignore = "subprocess entry point invoked by hard-termination recovery cases"]
    async fn recovery_crash_child() {
        let root = PathBuf::from(std::env::var_os("GITRU_RECOVERY_CHILD_ROOT").unwrap());
        let target = root.join("target.sqlite");
        match std::env::var("GITRU_RECOVERY_CHILD_MODE").unwrap().as_str() {
            "backup" => {
                let store = Store::open(target).await.unwrap();
                store
                    .backup_to(root.join("interrupted-backup.sqlite"))
                    .await
                    .unwrap();
            }
            "restore" => {
                let session = RecoverySession::prepare(target, root.join("backup.sqlite"))
                    .await
                    .unwrap();
                let id = session.preview().confirmation_id.clone();
                session
                    .confirm(&id, RestoreChoice::ReplaceCurrentData)
                    .unwrap();
            }
            "rollback" => {
                let session = InterruptedRecovery::inspect(target).unwrap();
                let id = session.preview().confirmation_id.clone();
                session
                    .confirm(&id, RestoreChoice::KeepOriginalData)
                    .unwrap();
            }
            _ => panic!("unknown synthetic recovery mode"),
        }
    }

    #[tokio::test]
    async fn interrupted_backups_never_publish_partial_exports_or_change_original_drafts() {
        for boundary in ["backup_snapshot", "backup_verified"] {
            let directory = tempfile::tempdir().unwrap();
            scenario(directory.path()).await;
            kill_at(directory.path(), boundary, "backup").await;
            assert!(!directory.path().join("interrupted-backup.sqlite").exists());
            assert_newer_draft(&directory.path().join("target.sqlite")).await;
        }
    }

    #[tokio::test]
    async fn process_crashes_during_restore_and_rollback_keep_original_data_recoverable() {
        for boundary in [
            "restore_bundle_verified",
            "restore_prepared",
            "restore_sidecars_removed",
            "restore_installed",
        ] {
            let directory = tempfile::tempdir().unwrap();
            scenario(directory.path()).await;
            let target = directory.path().join("target.sqlite");
            kill_at(directory.path(), boundary, "restore").await;
            if boundary == "restore_bundle_verified" {
                assert_newer_draft(&target).await;
                continue;
            }
            assert!(
                matches!(Store::open(&target).await, Err(error) if error.code == ErrorCode::Storage)
            );
            // A second process dies while restoring the original main DB. The
            // retained originals remain immutable; a new explicit choice works.
            kill_at(directory.path(), "rollback_main_installed", "rollback").await;
            assert!(
                matches!(Store::open(&target).await, Err(error) if error.code == ErrorCode::Storage)
            );
            let session = InterruptedRecovery::inspect(&target).unwrap();
            let id = session.preview().confirmation_id.clone();
            let archive = session
                .confirm(&id, RestoreChoice::KeepOriginalData)
                .unwrap();
            assert!(archive.join("original.sqlite").exists());
            assert_newer_draft(&target).await;
        }
    }

    #[tokio::test]
    async fn sqlite_interruption_during_snapshot_preserves_source_and_allows_retry() {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("source.sqlite");
        fixture(&source).await;
        let mut connection = connect(&source, false).await.unwrap();
        {
            let mut handle = connection.lock_handle().await.unwrap();
            let mut interrupted = false;
            handle.set_progress_handler(50, move || {
                if interrupted {
                    true
                } else {
                    interrupted = true;
                    false
                }
            });
        }
        let destination = directory.path().join("backup.sqlite");
        assert_eq!(
            backup_from(&mut connection, &destination)
                .await
                .unwrap_err()
                .code,
            ErrorCode::Storage
        );
        connection
            .lock_handle()
            .await
            .unwrap()
            .remove_progress_handler();
        assert!(!destination.exists());
        let report = backup_from(&mut connection, &destination).await.unwrap();
        assert_eq!(report.drafts, 4);
        connection.close().await.unwrap();
        let store = Store::open(source).await.unwrap();
        assert_eq!(
            store
                .draft("a", "pull-request-67")
                .await
                .unwrap()
                .unwrap()
                .generation,
            "37"
        );
        store.close().await;
        drop(store);
    }

    #[tokio::test]
    async fn backup_after_real_sqlite_full_exports_only_committed_authored_text() {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("source.sqlite");
        fixture(&source).await;
        let mut connection = connect(&source, false).await.unwrap();
        let pages: i64 = sqlx::query_scalar("PRAGMA page_count")
            .fetch_one(&mut connection)
            .await
            .unwrap();
        let limit: i64 = sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
            "PRAGMA max_page_count={pages}"
        )))
        .fetch_one(&mut connection)
        .await
        .unwrap();
        assert_eq!(limit, pages);
        let error =
            sqlx::query("UPDATE drafts SET body=body||zeroblob(2097152) WHERE account_id='a'")
                .execute(&mut connection)
                .await
                .unwrap_err();
        assert_eq!(
            error.as_database_error().unwrap().code().as_deref(),
            Some("13")
        );
        sqlx::query("PRAGMA max_page_count=1073741823")
            .execute(&mut connection)
            .await
            .unwrap();
        let destination = directory.path().join("backup.sqlite");
        assert_eq!(
            backup_from(&mut connection, &destination)
                .await
                .unwrap()
                .drafts,
            4
        );
        connection.close().await.unwrap();
        let restored = Store::open(destination).await.unwrap();
        assert_eq!(
            restored
                .draft("a", "pull-request-67")
                .await
                .unwrap()
                .unwrap()
                .body,
            "Alice unsent draft with a newline\nand Unicode: café 🦀"
        );
        restored.close().await;
        drop(restored);
    }
}
