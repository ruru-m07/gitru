use super::*;
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    fs::OpenOptions,
    io::Write,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
};

#[derive(Default)]
struct Vault {
    keys: Mutex<HashMap<String, [u8; 32]>>,
    loads: AtomicUsize,
    stores: AtomicUsize,
    mode: Mutex<Option<&'static str>>,
}
impl DatabaseKeyVault for Vault {
    fn load(
        &self,
        identity: &DatabaseKeyIdentity,
    ) -> Result<Option<DatabaseKey>, DatabaseKeyError> {
        self.loads.fetch_add(1, Ordering::SeqCst);
        if *self.mode.lock().unwrap() == Some("locked") {
            return Err(DatabaseKeyError::VaultLocked);
        }
        Ok(self
            .keys
            .lock()
            .unwrap()
            .get(&identity.vault_reference())
            .copied()
            .map(DatabaseKey::from_bytes))
    }
    fn store_new(
        &self,
        identity: &DatabaseKeyIdentity,
        key: &DatabaseKey,
    ) -> Result<(), DatabaseKeyError> {
        self.stores.fetch_add(1, Ordering::SeqCst);
        let mode = *self.mode.lock().unwrap();
        if mode != Some("no_write") {
            let value = if mode == Some("mismatch") {
                [0; 32]
            } else {
                *key.expose()
            };
            assert!(
                self.keys
                    .lock()
                    .unwrap()
                    .insert(identity.vault_reference(), value)
                    .is_none()
            );
        }
        if matches!(mode, Some("no_write" | "write_then_error")) {
            Err(DatabaseKeyError::VaultUnavailable)
        } else {
            Ok(())
        }
    }
}
fn path(temp: &tempfile::TempDir) -> PathBuf {
    temp.path().join("private/cache.db")
}
fn write(path: &Path, bytes: &[u8]) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let mut file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(path)
        .unwrap();
    file.write_all(bytes).unwrap();
    file.sync_all().unwrap();
}
/// A synthetic identity/key digest, NOT an encrypted database or cipher proof.
fn fixture(identity: &DatabaseKeyIdentity, key: &DatabaseKey) -> Vec<u8> {
    let mut digest = Sha256::new();
    digest.update(b"synthetic bootstrap fixture\0");
    digest.update(identity.vault_reference());
    digest.update(key.expose());
    digest.finalize().to_vec()
}
struct Verifier;
#[async_trait::async_trait]
impl DatabaseKeyVerifier for Verifier {
    async fn verify(
        &self,
        path: &Path,
        identity: &DatabaseKeyIdentity,
        key: &DatabaseKey,
    ) -> Result<(), DatabaseKeyError> {
        if std::fs::read(path).unwrap() == fixture(identity, key) {
            Ok(())
        } else {
            Err(DatabaseKeyError::WrongKeyOrCorrupt)
        }
    }
}
fn create_file(session: &DatabaseKeySession) {
    write(session.path(), &fixture(&session.identity(), session.key()));
}

#[tokio::test]
async fn reserve_resume_verify_and_cold_open_reuse_exact_key() {
    let temp = tempfile::tempdir().unwrap();
    let path = path(&temp);
    let vault = Vault::default();
    assert_eq!(
        DatabaseKeySession::prepare(&path, &vault, DatabaseCreation::ExistingOnly).unwrap_err(),
        DatabaseKeyError::CreationNotAuthorized
    );
    assert_eq!(vault.loads.load(Ordering::SeqCst), 0);
    let first = DatabaseKeySession::prepare(&path, &vault, DatabaseCreation::AllowNew).unwrap();
    let identity = first.identity();
    let key = *first.key().expose();
    assert_ne!(key, [0; 32]);
    assert!(!files::read(&path).unwrap().unwrap().ready);
    assert_eq!(first.mode(), DatabaseKeyMode::CreateNew);
    assert_eq!(
        DatabaseKeySession::prepare(&path, &vault, DatabaseCreation::AllowNew).unwrap_err(),
        DatabaseKeyError::Busy
    );
    let debug = format!("{first:?} {:?}", first.key());
    assert!(debug.contains("REDACTED"));
    assert!(
        !String::from_utf8(std::fs::read(files::append(&path, ".key.json")).unwrap())
            .unwrap()
            .contains("key")
    );
    drop(first);
    let mut resumed =
        DatabaseKeySession::prepare(&path, &vault, DatabaseCreation::ExistingOnly).unwrap();
    assert_eq!(resumed.identity(), identity);
    assert_eq!(resumed.key().expose(), &key);
    assert_eq!(
        resumed.verify(&Verifier).await,
        Err(DatabaseKeyError::MissingDatabase)
    );
    create_file(&resumed);
    resumed.verify(&Verifier).await.unwrap();
    assert_eq!(resumed.mode(), DatabaseKeyMode::Verified);
    assert!(files::read(&path).unwrap().unwrap().ready);
    drop(resumed);
    let mut reopened =
        DatabaseKeySession::prepare(&path, &vault, DatabaseCreation::ExistingOnly).unwrap();
    assert_eq!(reopened.mode(), DatabaseKeyMode::VerifyExisting);
    reopened.verify(&Verifier).await.unwrap();
    assert_eq!(reopened.identity(), identity);
    assert_eq!(vault.stores.load(Ordering::SeqCst), 1);
}

#[test]
fn ambiguous_vault_store_uses_readback_without_overwriting() {
    for (mode, expected) in [
        ("write_then_error", None),
        ("no_write", Some(DatabaseKeyError::VaultUnavailable)),
        ("mismatch", Some(DatabaseKeyError::VaultKeyMismatch)),
    ] {
        let temp = tempfile::tempdir().unwrap();
        let path = path(&temp);
        let vault = Vault::default();
        *vault.mode.lock().unwrap() = Some(mode);
        let result = DatabaseKeySession::prepare(&path, &vault, DatabaseCreation::AllowNew);
        if let Some(error) = expected {
            assert_eq!(result.unwrap_err(), error);
        } else {
            drop(result.unwrap());
        }
        assert_eq!(vault.stores.load(Ordering::SeqCst), 1);
        let identity = files::read(&path).unwrap().unwrap().identity();
        *vault.mode.lock().unwrap() = Some("locked");
        assert_eq!(
            DatabaseKeySession::prepare(&path, &vault, DatabaseCreation::ExistingOnly).unwrap_err(),
            DatabaseKeyError::VaultLocked
        );
        assert_eq!(files::read(&path).unwrap().unwrap().identity(), identity);
        assert_eq!(vault.stores.load(Ordering::SeqCst), 1);
    }
}

#[test]
fn every_existing_data_sidecar_blocks_new_key_even_when_empty() {
    for suffix in ["", "-wal", "-shm", "-journal"] {
        for body in [b"".as_slice(), b"retained authored intent"] {
            let temp = tempfile::tempdir().unwrap();
            let path = path(&temp);
            let vault = Vault::default();
            let existing = files::append(&path, suffix);
            write(&existing, body);
            assert_eq!(
                DatabaseKeySession::prepare(&path, &vault, DatabaseCreation::AllowNew).unwrap_err(),
                DatabaseKeyError::MissingKeyMetadata
            );
            assert_eq!(vault.loads.load(Ordering::SeqCst), 0);
            assert_eq!(vault.stores.load(Ordering::SeqCst), 0);
            assert_eq!(std::fs::read(&existing).unwrap(), body);
            let journal = files::Journal::reserved();
            files::publish(&path, &journal, false).unwrap();
            assert_eq!(
                DatabaseKeySession::prepare(&path, &vault, DatabaseCreation::AllowNew).unwrap_err(),
                DatabaseKeyError::MissingKey
            );
            assert_eq!(vault.stores.load(Ordering::SeqCst), 0);
            assert_eq!(std::fs::read(&existing).unwrap(), body);
        }
    }
    let temp = tempfile::tempdir().unwrap();
    let path = path(&temp);
    write(&path, b"SQLite format 3\0original source");
    assert_eq!(
        DatabaseKeySession::prepare(&path, &Vault::default(), DatabaseCreation::AllowNew)
            .unwrap_err(),
        DatabaseKeyError::PlaintextMigrationRequired
    );
}

#[tokio::test]
async fn ready_missing_database_and_missing_key_are_distinct_and_preserve_data() {
    let temp = tempfile::tempdir().unwrap();
    let path = path(&temp);
    let vault = Vault::default();
    let mut session =
        DatabaseKeySession::prepare(&path, &vault, DatabaseCreation::AllowNew).unwrap();
    create_file(&session);
    session.verify(&Verifier).await.unwrap();
    drop(session);
    let original = std::fs::read(&path).unwrap();
    let metadata = std::fs::read(files::append(&path, ".key.json")).unwrap();
    let saved = vault.keys.lock().unwrap().clone();
    vault.keys.lock().unwrap().clear();
    assert_eq!(
        DatabaseKeySession::prepare(&path, &vault, DatabaseCreation::AllowNew).unwrap_err(),
        DatabaseKeyError::MissingKey
    );
    assert_eq!(std::fs::read(&path).unwrap(), original);
    *vault.keys.lock().unwrap() = saved;
    std::fs::remove_file(&path).unwrap();
    assert_eq!(
        DatabaseKeySession::prepare(&path, &vault, DatabaseCreation::AllowNew).unwrap_err(),
        DatabaseKeyError::MissingDatabase
    );
    assert_eq!(
        std::fs::read(files::append(&path, ".key.json")).unwrap(),
        metadata
    );
    assert_eq!(vault.stores.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn interrupted_creation_verifies_before_ready_and_wrong_key_preserves_everything() {
    let temp = tempfile::tempdir().unwrap();
    let path = path(&temp);
    let vault = Vault::default();
    let session = DatabaseKeySession::prepare(&path, &vault, DatabaseCreation::AllowNew).unwrap();
    create_file(&session);
    let identity = session.identity();
    drop(session);
    let original = std::fs::read(&path).unwrap();
    let metadata = std::fs::read(files::append(&path, ".key.json")).unwrap();
    let correct = {
        let mut bad = vault.keys.lock().unwrap();
        bad.insert(identity.vault_reference(), [8; 32]).unwrap()
    };
    let mut session =
        DatabaseKeySession::prepare(&path, &vault, DatabaseCreation::ExistingOnly).unwrap();
    assert_eq!(session.mode(), DatabaseKeyMode::VerifyInterruptedCreation);
    assert_eq!(
        session.verify(&Verifier).await,
        Err(DatabaseKeyError::WrongKeyOrCorrupt)
    );
    drop(session);
    assert_eq!(std::fs::read(&path).unwrap(), original);
    assert_eq!(
        std::fs::read(files::append(&path, ".key.json")).unwrap(),
        metadata
    );
    vault
        .keys
        .lock()
        .unwrap()
        .insert(identity.vault_reference(), correct);
    let mut session =
        DatabaseKeySession::prepare(&path, &vault, DatabaseCreation::ExistingOnly).unwrap();
    session.verify(&Verifier).await.unwrap();
    assert_eq!(vault.stores.load(Ordering::SeqCst), 1);
}

#[test]
fn metadata_bounds_versions_pending_and_restore_fail_closed() {
    for bytes in [b"{}".to_vec(),vec![b'x';1025],serde_json::to_vec(&serde_json::json!({"version":2,"database_id":uuid::Uuid::new_v4().to_string(),"generation":1,"ready":false})).unwrap(),serde_json::to_vec(&serde_json::json!({"version":1,"database_id":uuid::Uuid::new_v4().to_string(),"generation":2,"ready":false})).unwrap()] {
        let temp=tempfile::tempdir().unwrap(); let path=path(&temp); let vault=Vault::default();
        write(&files::append(&path,".key.json"),&bytes);
        assert_eq!(DatabaseKeySession::prepare(&path,&vault,DatabaseCreation::AllowNew).unwrap_err(),DatabaseKeyError::InvalidMetadata);
        assert_eq!(vault.loads.load(Ordering::SeqCst),0); assert_eq!(std::fs::read(files::append(&path,".key.json")).unwrap(),bytes);
    }
    let temp = tempfile::tempdir().unwrap();
    let path = path(&temp);
    let vault = Vault::default();
    write(&files::append(&path, ".key.json.pending"), b"partial");
    assert_eq!(
        DatabaseKeySession::prepare(&path, &vault, DatabaseCreation::AllowNew).unwrap_err(),
        DatabaseKeyError::InterruptedMetadata
    );
    std::fs::remove_file(files::append(&path, ".key.json.pending")).unwrap();
    std::fs::create_dir(files::append(&path, ".restore-pending")).unwrap();
    assert_eq!(
        DatabaseKeySession::prepare(&path, &vault, DatabaseCreation::AllowNew).unwrap_err(),
        DatabaseKeyError::InterruptedRestore
    );
    assert_eq!(vault.loads.load(Ordering::SeqCst), 0);
}

#[cfg(unix)]
#[test]
fn metadata_and_sidecar_symlinks_are_never_followed() {
    use std::os::unix::fs::symlink;
    for suffix in [".key.json", ".key.json.pending", "-wal", "-shm", "-journal"] {
        let temp = tempfile::tempdir().unwrap();
        let path = path(&temp);
        let outside = temp.path().join("outside");
        write(&outside, b"do not read or write");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        symlink(&outside, files::append(&path, suffix)).unwrap();
        let vault = Vault::default();
        assert_eq!(
            DatabaseKeySession::prepare(&path, &vault, DatabaseCreation::AllowNew).unwrap_err(),
            DatabaseKeyError::InvalidMetadata
        );
        assert_eq!(vault.loads.load(Ordering::SeqCst), 0);
        assert_eq!(std::fs::read(&outside).unwrap(), b"do not read or write");
    }
}

struct HeldVerifier {
    started: tokio::sync::Notify,
    release: tokio::sync::Notify,
}
#[async_trait::async_trait]
impl DatabaseKeyVerifier for HeldVerifier {
    async fn verify(
        &self,
        path: &Path,
        identity: &DatabaseKeyIdentity,
        key: &DatabaseKey,
    ) -> Result<(), DatabaseKeyError> {
        Verifier.verify(path, identity, key).await?;
        self.started.notify_one();
        self.release.notified().await;
        Ok(())
    }
}
#[tokio::test]
async fn held_verification_refuses_same_presence_replacement_in_place_write_and_metadata_aba() {
    for action in ["replace", "modify", "same_length", "metadata"] {
        let temp = tempfile::tempdir().unwrap();
        let path = path(&temp);
        let vault = Vault::default();
        let mut session =
            DatabaseKeySession::prepare(&path, &vault, DatabaseCreation::AllowNew).unwrap();
        create_file(&session);
        let gate = Arc::new(HeldVerifier {
            started: tokio::sync::Notify::new(),
            release: tokio::sync::Notify::new(),
        });
        let child = gate.clone();
        let task = tokio::spawn(async move {
            let result = session.verify(child.as_ref()).await;
            (result, session)
        });
        tokio::time::timeout(std::time::Duration::from_secs(30), gate.started.notified())
            .await
            .unwrap();
        match action {
            "replace" => {
                let old = files::append(&path, ".removed");
                std::fs::rename(&path, old).unwrap();
                write(&path, b"replacement");
            }
            "modify" => write(&path, b"in place modification after keyed verification"),
            "same_length" => {
                let length = std::fs::metadata(&path).unwrap().len() as usize;
                write(&path, &vec![7; length]);
                std::fs::File::options()
                    .write(true)
                    .open(&path)
                    .unwrap()
                    .set_modified(std::time::SystemTime::now() + std::time::Duration::from_secs(2))
                    .unwrap();
            }
            "metadata" => {
                let metadata = files::append(&path, ".key.json");
                let bytes = std::fs::read(&metadata).unwrap();
                std::fs::rename(&metadata, files::append(&path, ".old-metadata")).unwrap();
                write(&metadata, &bytes);
            }
            _ => unreachable!(),
        }
        gate.release.notify_one();
        let (result, session) = task.await.unwrap();
        assert_eq!(result, Err(DatabaseKeyError::StaleFilesystem));
        assert_ne!(session.mode(), DatabaseKeyMode::Verified);
        assert!(!files::read(&path).unwrap().unwrap().ready);
    }
}

struct MutatingVault<'a> {
    vault: &'a Vault,
    path: PathBuf,
}
impl DatabaseKeyVault for MutatingVault<'_> {
    fn load(
        &self,
        identity: &DatabaseKeyIdentity,
    ) -> Result<Option<DatabaseKey>, DatabaseKeyError> {
        let value = self.vault.load(identity)?;
        std::fs::rename(&self.path, files::append(&self.path, ".previous")).unwrap();
        write(&self.path, b"different db");
        Ok(value)
    }
    fn store_new(&self, _: &DatabaseKeyIdentity, _: &DatabaseKey) -> Result<(), DatabaseKeyError> {
        panic!("must not generate")
    }
}
#[test]
fn held_vault_file_replacement_cannot_return_a_key_session() {
    let temp = tempfile::tempdir().unwrap();
    let path = path(&temp);
    let vault = Vault::default();
    let session = DatabaseKeySession::prepare(&path, &vault, DatabaseCreation::AllowNew).unwrap();
    create_file(&session);
    drop(session);
    let mutating = MutatingVault {
        vault: &vault,
        path: path.clone(),
    };
    assert_eq!(
        DatabaseKeySession::prepare(&path, &mutating, DatabaseCreation::ExistingOnly).unwrap_err(),
        DatabaseKeyError::StaleFilesystem
    );
    assert_eq!(vault.stores.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn metadata_publication_faults_preserve_db_and_stop_restart() {
    for point in [
        "after_write",
        "after_file_sync",
        "before_publish",
        "before_directory_sync",
    ] {
        let temp = tempfile::tempdir().unwrap();
        let path = path(&temp);
        let vault = Vault::default();
        let mut session =
            DatabaseKeySession::prepare(&path, &vault, DatabaseCreation::AllowNew).unwrap();
        create_file(&session);
        let original = std::fs::read(&path).unwrap();
        let identity = session.identity();
        files::FAIL_PUBLICATION.with(|fault| fault.set(Some(point)));
        let result = session.verify(&Verifier).await;
        files::FAIL_PUBLICATION.with(|fault| fault.set(None));
        assert_eq!(result, Err(DatabaseKeyError::Storage));
        assert_ne!(session.mode(), DatabaseKeyMode::Verified);
        drop(session);
        assert_eq!(std::fs::read(&path).unwrap(), original);
        assert_eq!(files::read(&path).unwrap().unwrap().identity(), identity);
        if point == "before_directory_sync" {
            // A published Ready record may be durable despite an ambiguous final
            // directory sync; startup still requires verification of existing data.
            let mut retry =
                DatabaseKeySession::prepare(&path, &vault, DatabaseCreation::ExistingOnly).unwrap();
            retry.verify(&Verifier).await.unwrap();
        } else {
            assert_eq!(
                DatabaseKeySession::prepare(&path, &vault, DatabaseCreation::ExistingOnly)
                    .unwrap_err(),
                DatabaseKeyError::InterruptedMetadata
            );
            assert!(!files::read(&path).unwrap().unwrap().ready);
        }
        assert_eq!(vault.stores.load(Ordering::SeqCst), 1);
    }
    for point in [
        "after_write",
        "after_file_sync",
        "before_publish",
        "after_link",
        "before_directory_sync",
    ] {
        let temp = tempfile::tempdir().unwrap();
        let path = path(&temp);
        let vault = Vault::default();
        files::FAIL_PUBLICATION.with(|fault| fault.set(Some(point)));
        let result = DatabaseKeySession::prepare(&path, &vault, DatabaseCreation::AllowNew);
        files::FAIL_PUBLICATION.with(|fault| fault.set(None));
        assert_eq!(result.unwrap_err(), DatabaseKeyError::Storage);
        assert_eq!(vault.loads.load(Ordering::SeqCst), 0);
        assert_eq!(vault.stores.load(Ordering::SeqCst), 0);
    }
}

struct FileVault {
    directory: PathBuf,
    crash: Option<String>,
}
impl DatabaseKeyVault for FileVault {
    fn load(
        &self,
        identity: &DatabaseKeyIdentity,
    ) -> Result<Option<DatabaseKey>, DatabaseKeyError> {
        if self.crash.as_deref() == Some("reserved") {
            std::process::exit(77);
        }
        let path = self.directory.join(identity.vault_reference());
        match std::fs::read(path) {
            Ok(bytes) => Ok(Some(DatabaseKey::from_bytes(bytes.try_into().unwrap()))),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(_) => Err(DatabaseKeyError::VaultUnavailable),
        }
    }
    fn store_new(
        &self,
        identity: &DatabaseKeyIdentity,
        key: &DatabaseKey,
    ) -> Result<(), DatabaseKeyError> {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(self.directory.join(identity.vault_reference()))
            .unwrap();
        file.write_all(key.expose()).unwrap();
        file.sync_all().unwrap();
        if self.crash.as_deref() == Some("stored") {
            std::process::exit(77);
        }
        Ok(())
    }
}
#[test]
#[ignore = "child process helper; parent owns all synthetic fixture paths"]
fn independent_key_crash_child() {
    let Some(directory) = std::env::var_os("GITRU_KEY_TEST_DIRECTORY") else {
        return;
    };
    let directory = PathBuf::from(directory);
    let phase = std::env::var("GITRU_KEY_TEST_PHASE").unwrap();
    let vault = FileVault {
        directory: directory.clone(),
        crash: Some(phase.clone()),
    };
    let mut session = DatabaseKeySession::prepare(
        &directory.join("private/cache.db"),
        &vault,
        DatabaseCreation::AllowNew,
    )
    .unwrap();
    create_file(&session);
    if phase == "ready" {
        tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap()
            .block_on(session.verify(&Verifier))
            .unwrap();
    }
    std::process::exit(77);
}
#[tokio::test]
async fn independent_process_death_recovers_exact_reservation_before_and_after_vault_write() {
    for phase in ["reserved", "stored", "database", "ready"] {
        let temp = tempfile::tempdir().unwrap();
        let mut child = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "database_keys::tests::independent_key_crash_child",
                "--ignored",
                "--nocapture",
            ])
            .env("GITRU_KEY_TEST_DIRECTORY", temp.path())
            .env("GITRU_KEY_TEST_PHASE", phase)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
        let status = loop {
            if let Some(status) = child.try_wait().unwrap() {
                break status;
            }
            if std::time::Instant::now() >= deadline {
                child.kill().unwrap();
                panic!("key fixture child did not reach {phase}");
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        };
        assert_eq!(status.code(), Some(77), "child phase {phase}");
        let path = path(&temp);
        let journal = files::read(&path).unwrap().unwrap();
        let identity = journal.identity();
        let vault = FileVault {
            directory: temp.path().into(),
            crash: None,
        };
        let old = vault.load(&identity).unwrap();
        assert_eq!(old.is_none(), phase == "reserved");
        let mut session =
            DatabaseKeySession::prepare(&path, &vault, DatabaseCreation::ExistingOnly).unwrap();
        assert_eq!(session.identity(), identity);
        if let Some(old) = old {
            assert!(old.same(session.key()));
        }
        assert_eq!(
            session.mode(),
            match phase {
                "ready" => DatabaseKeyMode::VerifyExisting,
                "database" => DatabaseKeyMode::VerifyInterruptedCreation,
                _ => DatabaseKeyMode::CreateNew,
            }
        );
        if !path.exists() {
            create_file(&session);
        }
        session.verify(&Verifier).await.unwrap();
        assert_eq!(
            std::fs::read_dir(temp.path())
                .unwrap()
                .filter_map(Result::ok)
                .filter(|entry| entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with("gitru.collaboration.database.v1."))
                .count(),
            1
        );
    }
}

#[tokio::test]
async fn failed_reverification_revokes_prior_verified_mode() {
    for action in ["data", "journal", "missing"] {
        let temp = tempfile::tempdir().unwrap();
        let path = path(&temp);
        let vault = Vault::default();
        let mut session =
            DatabaseKeySession::prepare(&path, &vault, DatabaseCreation::AllowNew).unwrap();
        create_file(&session);
        session.verify(&Verifier).await.unwrap();
        assert_eq!(session.mode(), DatabaseKeyMode::Verified);
        let expected = match action {
            "data" => {
                write(&path, b"modified after success");
                DatabaseKeyError::WrongKeyOrCorrupt
            }
            "journal" => {
                let metadata = files::append(&path, ".key.json");
                let mut replacement = files::Journal::reserved();
                replacement.ready = true;
                write(&metadata, &serde_json::to_vec(&replacement).unwrap());
                DatabaseKeyError::StaleFilesystem
            }
            "missing" => {
                std::fs::remove_file(&path).unwrap();
                DatabaseKeyError::MissingDatabase
            }
            _ => unreachable!(),
        };
        assert_eq!(session.verify(&Verifier).await, Err(expected));
        assert_eq!(session.mode(), DatabaseKeyMode::VerifyExisting);
    }
}
