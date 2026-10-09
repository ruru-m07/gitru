//! Portable, passphrase-protected collaboration backup envelopes.
//!
//! The outer stream is the interoperable age v1 file format using its scrypt
//! recipient. Gitru does not define encryption, key derivation, nonces, or
//! authentication itself. A small authenticated manifest binds the decrypted
//! SQLite length, digest, and reviewed restore-schema policy before the existing
//! recovery transaction is allowed to inspect or replace storage.

use std::{
    fs::{File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
};

use age::secrecy::SecretString;
use serde::{Deserialize, Serialize};
use sqlx::SqliteConnection;
use tempfile::TempDir;

use crate::{
    CollaborationError, ErrorCode,
    recovery::{BackupSummary, RecoverySession},
};

const MAGIC: &[u8; 16] = b"GITRU-PORTABLE\0\x01";
const FORMAT_VERSION: u32 = 1;
const MAX_MANIFEST_BYTES: usize = 16 * 1024;
const MAX_DATABASE_BYTES: u64 = 512 * 1024 * 1024;

type Result<T> = std::result::Result<T, CollaborationError>;

/// A user-supplied portable restore credential. It has no Debug/text accessor
/// and is never persisted by this module or substituted with a device vault key.
#[derive(Clone)]
pub struct PortableBackupCredential(SecretString);

impl PortableBackupCredential {
    pub fn from_user_passphrase(passphrase: String) -> Result<Self> {
        if passphrase.chars().count() < 12 || passphrase.len() > 1024 {
            return Err(CollaborationError::invalid(
                "Use a portable backup passphrase between 12 and 1024 bytes",
            ));
        }
        Ok(Self(SecretString::from(passphrase)))
    }
}

impl std::fmt::Debug for PortableBackupCredential {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("PortableBackupCredential([REDACTED])")
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    format: u32,
    media_type: String,
    sqlite_bytes: u64,
    sqlite_sha256: String,
    schema_version: i64,
    revision: String,
    accounts: u64,
    drafts: u64,
    commands: u64,
}

/// Encrypt an already verified standalone SQLite backup. Publication is
/// create-new and atomic; interrupted ciphertext stays in private staging.
pub async fn encrypt_verified_backup(
    backup: impl AsRef<Path>,
    destination: impl AsRef<Path>,
    credential: &PortableBackupCredential,
) -> Result<BackupSummary> {
    let backup = backup.as_ref();
    let destination = destination.as_ref();
    if destination.exists() {
        return Err(CollaborationError::invalid("Choose a new backup filename"));
    }
    let summary = crate::recovery::inspect_standalone_backup(backup).await?;
    let metadata = std::fs::metadata(backup).map_err(|_| invalid())?;
    if !metadata.is_file() || metadata.len() == 0 || metadata.len() > MAX_DATABASE_BYTES {
        return Err(invalid());
    }
    let manifest = Manifest {
        format: FORMAT_VERSION,
        media_type: "application/vnd.gitru.collaboration.sqlite".into(),
        sqlite_bytes: metadata.len(),
        sqlite_sha256: summary.sha256.clone(),
        schema_version: summary.schema_version,
        revision: summary.revision.clone(),
        accounts: summary.accounts,
        drafts: summary.drafts,
        commands: summary.commands,
    };
    let manifest = serde_json::to_vec(&manifest).map_err(|_| invalid())?;
    if manifest.len() > MAX_MANIFEST_BYTES {
        return Err(invalid());
    }
    let parent = destination.parent().ok_or_else(invalid)?;
    let stage = private_stage(parent)?;
    let ciphertext = stage.path().join("portable.age");
    let output = private_create(&ciphertext)?;
    let encryptor = age::Encryptor::with_user_passphrase(credential.0.clone());
    let mut writer = encryptor.wrap_output(output).map_err(|_| invalid())?;
    writer.write_all(MAGIC).map_err(|_| storage())?;
    writer
        .write_all(&(manifest.len() as u32).to_be_bytes())
        .map_err(|_| storage())?;
    writer.write_all(&manifest).map_err(|_| storage())?;
    let input = File::open(backup).map_err(|_| invalid())?;
    std::io::copy(&mut input.take(MAX_DATABASE_BYTES + 1), &mut writer).map_err(|_| storage())?;
    let output = writer.finish().map_err(|_| storage())?;
    output.sync_all().map_err(|_| storage())?;
    std::fs::hard_link(&ciphertext, destination).map_err(|_| storage())?;
    sync_directory(parent)?;
    Ok(summary)
}

pub(crate) async fn encrypt_from_connection(
    connection: &mut SqliteConnection,
    destination: &Path,
    credential: &PortableBackupCredential,
) -> Result<BackupSummary> {
    if destination.exists() {
        return Err(CollaborationError::invalid("Choose a new backup filename"));
    }
    let parent = destination.parent().ok_or_else(invalid)?;
    let stage = private_stage(parent)?;
    let snapshot = stage.path().join("verified.sqlite");
    let expected = crate::recovery::backup_from(connection, &snapshot).await?;
    let exported = encrypt_verified_backup(&snapshot, destination, credential).await?;
    if exported != expected {
        return Err(invalid());
    }
    Ok(exported)
}

/// Owns both the verified decrypted staging file and the existing explicit
/// recovery session. Dropping before confirmation removes plaintext staging and
/// leaves active storage untouched.
pub struct PortableRecoverySession {
    _stage: TempDir,
    recovery: RecoverySession,
}

/// Authenticated plaintext owned only for a trusted native rekey/import. The
/// directory is private and removed on drop; no caller can detach the path.
pub struct VerifiedPortableBackup {
    _stage: TempDir,
    path: PathBuf,
    summary: BackupSummary,
}

impl VerifiedPortableBackup {
    pub async fn prepare_for_keyed_import(
        target: impl AsRef<Path>,
        encrypted_backup: impl AsRef<Path>,
        credential: &PortableBackupCredential,
    ) -> Result<Self> {
        let target = target.as_ref();
        let parent = target.parent().ok_or_else(invalid)?;
        let stage = private_stage(parent)?;
        let selected = stage.path().join("selected.sqlite");
        decrypt_and_verify(encrypted_backup.as_ref(), &selected, credential).await?;
        // Reuse the exact reviewed restore policy before crossing the native
        // rekey boundary: migrate only staging, remove provider cache/credential
        // references, quarantine commands and advance authorization fences.
        let path = stage.path().join("verified.sqlite");
        let empty = crate::Store::open(&path).await?;
        empty.close().await?;
        let recovery = RecoverySession::prepare(&path, &selected).await?;
        let id = recovery.preview().confirmation_id.clone();
        recovery.confirm(&id, crate::recovery::RestoreChoice::ReplaceCurrentData)?;
        let summary = crate::recovery::inspect_standalone_backup(&path).await?;
        Ok(Self {
            _stage: stage,
            path,
            summary,
        })
    }

    /// Trusted native codec seam. Callers must not copy, publish, log or expose
    /// this path; the returned borrow cannot outlive the RAII staging owner.
    pub fn native_import_path(&self) -> &Path {
        &self.path
    }

    pub fn summary(&self) -> &BackupSummary {
        &self.summary
    }
}

impl PortableRecoverySession {
    pub async fn prepare(
        target: impl AsRef<Path>,
        encrypted_backup: impl AsRef<Path>,
        credential: &PortableBackupCredential,
    ) -> Result<Self> {
        let target = target.as_ref();
        let parent = target.parent().ok_or_else(invalid)?;
        let stage = private_stage(parent)?;
        let plaintext = stage.path().join("verified.sqlite");
        decrypt_and_verify(encrypted_backup.as_ref(), &plaintext, credential).await?;
        let recovery = RecoverySession::prepare(target, &plaintext).await?;
        Ok(Self {
            _stage: stage,
            recovery,
        })
    }

    pub fn preview(&self) -> &crate::recovery::RestorePreview {
        self.recovery.preview()
    }

    pub fn confirm(
        self,
        confirmation_id: &str,
        choice: crate::recovery::RestoreChoice,
    ) -> Result<crate::recovery::RestoreReceipt> {
        self.recovery.confirm(confirmation_id, choice)
    }
}

async fn decrypt_and_verify(
    encrypted: &Path,
    plaintext: &Path,
    credential: &PortableBackupCredential,
) -> Result<BackupSummary> {
    let input = File::open(encrypted).map_err(|_| invalid())?;
    if !input.metadata().map_err(|_| invalid())?.is_file() {
        return Err(invalid());
    }
    let decryptor = age::Decryptor::new(input).map_err(|_| invalid())?;
    let identity = age::scrypt::Identity::new(credential.0.clone());
    let mut reader = decryptor
        .decrypt(std::iter::once(&identity as &dyn age::Identity))
        .map_err(|_| invalid())?;
    let mut magic = [0_u8; 16];
    reader.read_exact(&mut magic).map_err(|_| invalid())?;
    if &magic != MAGIC {
        return Err(invalid());
    }
    let mut length = [0_u8; 4];
    reader.read_exact(&mut length).map_err(|_| invalid())?;
    let length = u32::from_be_bytes(length) as usize;
    if length == 0 || length > MAX_MANIFEST_BYTES {
        return Err(invalid());
    }
    let mut manifest = vec![0; length];
    reader.read_exact(&mut manifest).map_err(|_| invalid())?;
    let manifest: Manifest = serde_json::from_slice(&manifest).map_err(|_| invalid())?;
    if manifest.format != FORMAT_VERSION
        || manifest.media_type != "application/vnd.gitru.collaboration.sqlite"
        || manifest.sqlite_bytes == 0
        || manifest.sqlite_bytes > MAX_DATABASE_BYTES
    {
        return Err(invalid());
    }
    let mut output = private_create(plaintext)?;
    let mut remaining = reader.take(manifest.sqlite_bytes + 1);
    let copied = std::io::copy(&mut remaining, &mut output).map_err(|_| storage())?;
    if copied != manifest.sqlite_bytes {
        return Err(invalid());
    }
    output.sync_all().map_err(|_| storage())?;
    drop(output);
    let summary = crate::recovery::inspect_standalone_backup(plaintext).await?;
    if summary.sha256 != manifest.sqlite_sha256
        || summary.schema_version != manifest.schema_version
        || summary.revision != manifest.revision
        || summary.accounts != manifest.accounts
        || summary.drafts != manifest.drafts
        || summary.commands != manifest.commands
    {
        return Err(invalid());
    }
    Ok(summary)
}

fn private_stage(parent: &Path) -> Result<TempDir> {
    let stage = tempfile::Builder::new()
        .prefix(".gitru-portable-")
        .tempdir_in(parent)
        .map_err(|_| storage())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(stage.path(), std::fs::Permissions::from_mode(0o700))
            .map_err(|_| storage())?;
    }
    Ok(stage)
}

fn private_create(path: &Path) -> Result<File> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options.open(path).map_err(|_| storage())
}

fn sync_directory(path: &Path) -> Result<()> {
    #[cfg(unix)]
    File::open(path)
        .and_then(|directory| directory.sync_all())
        .map_err(|_| storage())?;
    Ok(())
}

fn invalid() -> CollaborationError {
    CollaborationError::new(
        ErrorCode::InvalidInput,
        "Portable backup credential or encrypted data could not be verified; files were preserved",
    )
}
fn storage() -> CollaborationError {
    CollaborationError::storage()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AccountState, ProviderKind, RemoteAccount, Store, recovery::RestoreChoice};
    use sqlx::Connection;

    const CANARY: &str = "GITRU_PORTABLE_PRIVATE_CANARY_905173";

    async fn backup_fixture(root: &Path) -> (PathBuf, BackupSummary) {
        let source = root.join("source.sqlite");
        let backup = root.join("source.backup.sqlite");
        let store = Store::open(&source).await.unwrap();
        store
            .upsert_account(RemoteAccount {
                id: "portable-account".into(),
                provider: ProviderKind::Github,
                host: "github.com".into(),
                actor_id: "905173".into(),
                login: "portable-test".into(),
                display_name: Some(CANARY.into()),
                authorization_epoch: "1".into(),
                state: AccountState::Active,
                notifications_supported: true,
            })
            .await
            .unwrap();
        let summary = store.backup_to(&backup).await.unwrap();
        store.close().await.unwrap();
        (backup, summary)
    }

    async fn empty_target(root: &Path, name: &str) -> PathBuf {
        let target = root.join(name);
        let store = Store::open(&target).await.unwrap();
        store.close().await.unwrap();
        target
    }

    fn credential(value: &str) -> PortableBackupCredential {
        PortableBackupCredential::from_user_passphrase(value.to_owned()).unwrap()
    }

    fn contains(path: &Path, needle: &[u8]) -> bool {
        std::fs::read(path)
            .unwrap()
            .windows(needle.len())
            .any(|part| part == needle)
    }

    #[tokio::test]
    async fn portable_envelope_round_trips_through_existing_quarantine_policy() {
        let temp = tempfile::tempdir().unwrap();
        let encrypted = temp.path().join("portable.gitru-age");
        let key = credential("correct horse battery staple for Gitru");
        let source = temp.path().join("source.sqlite");
        let store = Store::open(&source).await.unwrap();
        store
            .upsert_account(RemoteAccount {
                id: "portable-account".into(),
                provider: ProviderKind::Github,
                host: "github.com".into(),
                actor_id: "905173".into(),
                login: "portable-test".into(),
                display_name: Some(CANARY.into()),
                authorization_epoch: "1".into(),
                state: AccountState::Active,
                notifications_supported: true,
            })
            .await
            .unwrap();
        let exported = store.portable_backup_to(&encrypted, &key).await.unwrap();
        store.close().await.unwrap();
        assert!(!contains(&encrypted, CANARY.as_bytes()));
        assert!(!contains(&encrypted, b"SQLite format 3\0"));

        let target = empty_target(temp.path(), "target.sqlite").await;
        let session = PortableRecoverySession::prepare(&target, &encrypted, &key)
            .await
            .unwrap();
        assert_eq!(session.preview().incoming, exported);
        let id = session.preview().confirmation_id.clone();
        let receipt = session
            .confirm(&id, RestoreChoice::ReplaceCurrentData)
            .unwrap();
        assert!(receipt.original_bundle.is_dir());
        let restored = Store::open(&target).await.unwrap();
        assert_eq!(
            restored.accounts().await.unwrap().accounts[0]
                .display_name
                .as_deref(),
            Some(CANARY)
        );
        restored.close().await.unwrap();
        assert!(std::fs::read_dir(temp.path()).unwrap().all(|entry| {
            !entry
                .unwrap()
                .file_name()
                .to_string_lossy()
                .starts_with(".gitru-portable-")
        }));
    }

    #[tokio::test]
    async fn wrong_key_tamper_and_truncation_preserve_target_and_leave_no_plaintext() {
        let temp = tempfile::tempdir().unwrap();
        let (backup, _) = backup_fixture(temp.path()).await;
        let encrypted = temp.path().join("portable.gitru-age");
        let key = credential("correct horse battery staple for Gitru");
        encrypt_verified_backup(&backup, &encrypted, &key)
            .await
            .unwrap();
        let original_ciphertext = std::fs::read(&encrypted).unwrap();

        for (name, bytes, candidate_key) in [
            (
                "wrong-key",
                original_ciphertext.clone(),
                credential("different explicit restore credential"),
            ),
            (
                "tampered",
                {
                    let mut value = original_ciphertext.clone();
                    let middle = value.len() / 2;
                    value[middle] ^= 0x40;
                    value
                },
                key.clone(),
            ),
            (
                "truncated",
                original_ciphertext[..original_ciphertext.len() - 17].to_vec(),
                key.clone(),
            ),
        ] {
            let candidate = temp.path().join(format!("{name}.gitru-age"));
            std::fs::write(&candidate, bytes).unwrap();
            let target = empty_target(temp.path(), &format!("{name}.sqlite")).await;
            let before = std::fs::read(&target).unwrap();
            assert!(
                PortableRecoverySession::prepare(&target, &candidate, &candidate_key)
                    .await
                    .is_err(),
                "{name} was accepted"
            );
            assert_eq!(
                std::fs::read(&target).unwrap(),
                before,
                "{name} changed target"
            );
            assert!(!contains(&candidate, CANARY.as_bytes()));
        }
        assert!(std::fs::read_dir(temp.path()).unwrap().all(|entry| {
            !entry
                .unwrap()
                .file_name()
                .to_string_lossy()
                .starts_with(".gitru-portable-")
        }));
    }

    #[tokio::test]
    async fn publication_is_create_new_and_rejects_live_wal_or_unreviewed_schema() {
        let temp = tempfile::tempdir().unwrap();
        let (backup, _) = backup_fixture(temp.path()).await;
        let destination = temp.path().join("portable.gitru-age");
        std::fs::write(&destination, b"existing user bytes").unwrap();
        let key = credential("correct horse battery staple for Gitru");
        assert!(
            encrypt_verified_backup(&backup, &destination, &key)
                .await
                .is_err()
        );
        assert_eq!(std::fs::read(&destination).unwrap(), b"existing user bytes");

        let sidecar = PathBuf::from(format!("{}-wal", backup.display()));
        std::fs::write(&sidecar, b"uncheckpointed frames").unwrap();
        assert!(
            encrypt_verified_backup(&backup, temp.path().join("wal.age"), &key)
                .await
                .is_err()
        );
        std::fs::remove_file(sidecar).unwrap();

        let mut connection =
            sqlx::SqliteConnection::connect(backup.to_str().expect("UTF-8 test path"))
                .await
                .unwrap();
        sqlx::query("UPDATE _sqlx_migrations SET version=999 WHERE version=(SELECT max(version) FROM _sqlx_migrations)")
            .execute(&mut connection)
            .await
            .unwrap();
        connection.close().await.unwrap();
        assert!(
            encrypt_verified_backup(&backup, temp.path().join("unknown-schema.age"), &key)
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn recognized_old_schema_is_encrypted_then_migrated_only_in_restore_staging() {
        let temp = tempfile::tempdir().unwrap();
        let historical = temp.path().join("historical-v1.sqlite");
        let mut db = sqlx::SqliteConnection::connect_with(
            &sqlx::sqlite::SqliteConnectOptions::new()
                .filename(&historical)
                .create_if_missing(true),
        )
        .await
        .unwrap();
        sqlx::raw_sql(include_str!("../tests/fixtures/migrations/v1/schema.sql"))
            .execute(&mut db)
            .await
            .unwrap();
        sqlx::raw_sql(include_str!("../tests/fixtures/migrations/v1/seed.sql"))
            .execute(&mut db)
            .await
            .unwrap();
        db.close().await.unwrap();
        let selected_before = std::fs::read(&historical).unwrap();
        let encrypted = temp.path().join("historical-v1.age");
        let key = credential("portable historical schema credential");
        let summary = encrypt_verified_backup(&historical, &encrypted, &key)
            .await
            .unwrap();
        assert_eq!(summary.schema_version, 1);
        assert_eq!(std::fs::read(&historical).unwrap(), selected_before);

        let target = empty_target(temp.path(), "old-schema-target.sqlite").await;
        let session = PortableRecoverySession::prepare(&target, &encrypted, &key)
            .await
            .unwrap();
        assert_eq!(session.preview().incoming.schema_version, 1);
        let id = session.preview().confirmation_id.clone();
        session
            .confirm(&id, RestoreChoice::ReplaceCurrentData)
            .unwrap();
        let restored = Store::open(&target).await.unwrap();
        assert_eq!(restored.accounts().await.unwrap().accounts.len(), 4);
        restored.close().await.unwrap();
        assert_eq!(std::fs::read(&historical).unwrap(), selected_before);
    }
}
