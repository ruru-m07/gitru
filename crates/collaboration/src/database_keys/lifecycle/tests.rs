use super::*;
use crate::database_keys::requires_keyed_open;
use std::{collections::HashMap, sync::Mutex};

#[derive(Default)]
struct Vault(Mutex<HashMap<String, [u8; 32]>>);
impl DatabaseKeyVault for Vault {
    fn load(
        &self,
        identity: &DatabaseKeyIdentity,
    ) -> Result<Option<DatabaseKey>, DatabaseKeyError> {
        Ok(self
            .0
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
        let mut keys = self.0.lock().unwrap();
        if keys.contains_key(&identity.vault_reference()) {
            return Err(DatabaseKeyError::VaultAlreadyExists);
        }
        keys.insert(identity.vault_reference(), *key.expose());
        Ok(())
    }
}

fn target(temp: &tempfile::TempDir) -> PathBuf {
    let directory = temp.path().join("private");
    std::fs::create_dir(&directory).unwrap();
    directory.join("cache.sqlite")
}
fn write(path: &Path, bytes: &[u8]) {
    std::fs::write(path, bytes).unwrap();
    File::open(path).unwrap().sync_all().unwrap();
}
fn ready_database(path: &Path, vault: &Vault) -> DatabaseKeyIdentity {
    let session = super::super::DatabaseKeySession::prepare(
        path,
        vault,
        super::super::DatabaseCreation::AllowNew,
    )
    .unwrap();
    let identity = session.identity();
    write(path, b"encrypted-current-authored-drafts");
    let mut journal = files::read(path).unwrap().unwrap();
    journal.ready = true;
    files::publish(path, &journal, true).unwrap();
    drop(session);
    identity
}
fn candidate(path: &Path, journal: &files::Journal, bytes: &[u8]) {
    write(path, bytes);
    files::publish(path, journal, false).unwrap();
}

#[test]
fn rotation_keeps_old_vault_generation_and_recovery_bytes() {
    let temp = tempfile::tempdir().unwrap();
    let target = target(&temp);
    let vault = Vault::default();
    let old_identity = ready_database(&target, &vault);
    let old_bytes = std::fs::read(&target).unwrap();
    let reservation = DatabaseKeyRotationReservation::prepare(&target, &vault).unwrap();
    assert_eq!(
        reservation.identity().database_id(),
        old_identity.database_id()
    );
    assert_eq!(
        reservation.identity().generation(),
        old_identity.generation() + 1
    );
    assert!(vault.load(&old_identity).unwrap().is_some());
    let candidate_path = target.with_file_name("rotated.sqlite");
    candidate(
        &candidate_path,
        &reservation.next,
        b"encrypted-next-generation-authored-drafts",
    );
    let proof = unsafe {
        VerifiedRotatedDatabase::from_native_verification(candidate_path, reservation.identity())
    };
    let rotation = reservation.prepare_verified_candidate(proof).unwrap();
    assert_eq!(
        rotation.confirm("wrong"),
        Err(DatabaseKeyError::ConfirmationRequired)
    );
    // A failed confirmation consumes only the in-memory session; all durable bytes remain.
    assert_eq!(std::fs::read(&target).unwrap(), old_bytes);
    // Resume reuses the exact next-generation vault entry without replacing it.
    let reservation = DatabaseKeyRotationReservation::prepare(&target, &vault).unwrap();
    let next = reservation.next.clone();
    let proof = unsafe {
        VerifiedRotatedDatabase::from_native_verification(
            target.with_file_name("rotated.sqlite"),
            reservation.identity(),
        )
    };
    let rotation = reservation.prepare_verified_candidate(proof).unwrap();
    let confirmation = rotation.preview().confirmation_id().to_owned();
    let quarantine = rotation.confirm(&confirmation).unwrap();
    assert_eq!(
        std::fs::read(&target).unwrap(),
        b"encrypted-next-generation-authored-drafts"
    );
    assert_eq!(
        std::fs::read(quarantine.join("database.sqlite")).unwrap(),
        old_bytes
    );
    assert_eq!(files::read(&target).unwrap().unwrap(), next);
    assert!(vault.load(&old_identity).unwrap().is_some());
}

#[test]
fn wrong_candidate_identity_and_sidecars_preserve_every_byte() {
    let temp = tempfile::tempdir().unwrap();
    let target = target(&temp);
    let vault = Vault::default();
    ready_database(&target, &vault);
    let reservation = DatabaseKeyRotationReservation::prepare(&target, &vault).unwrap();
    let original = std::fs::read(&target).unwrap();
    let metadata = std::fs::read(files::append(&target, KEY_SUFFIX)).unwrap();
    let candidate_path = target.with_file_name("candidate.sqlite");
    candidate(&candidate_path, &reservation.next, b"candidate");
    write(&files::append(&candidate_path, "-wal"), b"uncheckpointed");
    let proof = unsafe {
        VerifiedRotatedDatabase::from_native_verification(
            candidate_path.clone(),
            reservation.identity(),
        )
    };
    assert!(matches!(
        reservation.prepare_verified_candidate(proof),
        Err(DatabaseKeyError::Busy)
    ));
    assert_eq!(std::fs::read(&target).unwrap(), original);
    assert_eq!(
        std::fs::read(files::append(&target, KEY_SUFFIX)).unwrap(),
        metadata
    );
    assert_eq!(
        std::fs::read(files::append(&candidate_path, "-wal")).unwrap(),
        b"uncheckpointed"
    );
}

#[test]
fn explicit_reset_quarantines_database_and_metadata_without_touching_vault() {
    let temp = tempfile::tempdir().unwrap();
    let target = target(&temp);
    let vault = Vault::default();
    let identity = ready_database(&target, &vault);
    let original_key = *vault.load(&identity).unwrap().unwrap().expose();
    let reset = DatabaseReset::prepare(&target).unwrap();
    let confirmation = reset.preview().confirmation_id().to_owned();
    assert!(requires_keyed_open(&target));
    let quarantine = reset.confirm(&confirmation).unwrap();
    assert!(!target.exists());
    assert!(!files::append(&target, KEY_SUFFIX).exists());
    assert_eq!(
        std::fs::read(quarantine.join("database.sqlite")).unwrap(),
        b"encrypted-current-authored-drafts"
    );
    assert_eq!(
        vault.load(&identity).unwrap().unwrap().expose(),
        &original_key
    );
    // A new installation requires a separate AllowNew decision after quarantine.
    assert_eq!(
        super::super::DatabaseKeySession::prepare(
            &target,
            &vault,
            super::super::DatabaseCreation::ExistingOnly
        )
        .unwrap_err(),
        DatabaseKeyError::CreationNotAuthorized
    );
}

#[test]
fn interrupted_reset_blocks_startup_and_rolls_back_exact_bytes() {
    let temp = tempfile::tempdir().unwrap();
    let target = target(&temp);
    let vault = Vault::default();
    ready_database(&target, &vault);
    let original = std::fs::read(&target).unwrap();
    let original_meta = std::fs::read(files::append(&target, KEY_SUFFIX)).unwrap();
    let reset = DatabaseReset::prepare(&target).unwrap();
    publish_marker(&target, &reset.marker).unwrap();
    std::fs::create_dir(&reset.preview.quarantine).unwrap();
    move_file(&target, &reset.preview.quarantine.join("database.sqlite")).unwrap();
    move_file(
        &files::append(&target, KEY_SUFFIX),
        &reset.preview.quarantine.join("database.key.json"),
    )
    .unwrap();
    drop(reset);
    assert!(requires_keyed_open(&target));
    assert_eq!(
        super::super::DatabaseKeySession::prepare(
            &target,
            &vault,
            super::super::DatabaseCreation::AllowNew
        )
        .unwrap_err(),
        DatabaseKeyError::InterruptedRestore
    );
    let interrupted = InterruptedDatabaseKeyLifecycle::open(&target).unwrap();
    let quarantine = interrupted.rollback().unwrap();
    assert_eq!(std::fs::read(&target).unwrap(), original);
    assert_eq!(
        std::fs::read(files::append(&target, KEY_SUFFIX)).unwrap(),
        original_meta
    );
    assert!(quarantine.is_dir());
    assert!(!marker_path(&target).exists());
}
