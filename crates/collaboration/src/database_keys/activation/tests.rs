use super::*;

fn write(path: &Path, bytes: &[u8]) {
    std::fs::write(path, bytes).unwrap();
}

fn fixture() -> (tempfile::TempDir, PathBuf, PathBuf) {
    let directory = tempfile::tempdir().unwrap();
    let target = directory.path().join("collaboration.sqlite");
    let candidate = directory.path().join("candidate.sqlite");
    write(&target, b"SQLite format 3\0plaintext authored intent");
    write(&candidate, b"encrypted keyed candidate authored intent");
    write(
        &append(&candidate, KEY_SUFFIX),
        br#"{"version":1,"database_id":"01234567-89ab-4def-8123-456789abcdef","generation":1,"ready":true}"#,
    );
    (directory, target, candidate)
}

fn verified(path: &Path) -> VerifiedKeyedCandidate {
    // Test fixture stands in for the native SQLCipher verifier.
    unsafe { VerifiedKeyedCandidate::from_native_verification(path.to_owned()) }
}

#[test]
fn exact_confirmation_activates_candidate_and_preserves_plaintext() {
    let (_directory, target, candidate) = fixture();
    let session = KeyedActivationSession::prepare(&target, verified(&candidate)).unwrap();
    let confirmation = session.preview().confirmation_id().to_owned();
    let bundle = session.preview().recovery_bundle().to_owned();
    let receipt = session.confirm(&confirmation).unwrap();
    assert_eq!(receipt.recovery_bundle, bundle);
    assert_eq!(
        std::fs::read(&target).unwrap(),
        b"encrypted keyed candidate authored intent"
    );
    assert!(append(&target, KEY_SUFFIX).exists());
    assert_eq!(
        std::fs::read(bundle.join("plaintext.sqlite")).unwrap(),
        b"SQLite format 3\0plaintext authored intent"
    );
    assert!(bundle.join("manifest.json").exists());
    assert!(!marker_path(&target).exists());
    assert!(!candidate.exists());
}

#[test]
fn wrong_confirmation_and_changed_inputs_leave_every_file_untouched() {
    let (_directory, target, candidate) = fixture();
    let session = KeyedActivationSession::prepare(&target, verified(&candidate)).unwrap();
    assert!(session.confirm("wrong").is_err());
    assert!(target.exists() && candidate.exists());
    assert!(!marker_path(&target).exists());

    let session = KeyedActivationSession::prepare(&target, verified(&candidate)).unwrap();
    write(&candidate, b"changed after preview");
    let confirmation = session.preview().confirmation_id().to_owned();
    assert!(session.confirm(&confirmation).is_err());
    assert!(target.exists() && candidate.exists());
    assert!(!marker_path(&target).exists());
}

#[test]
fn pending_marker_and_sidecars_fail_closed() {
    let (_directory, target, candidate) = fixture();
    write(&append(&target, "-wal"), b"live");
    assert!(KeyedActivationSession::prepare(&target, verified(&candidate)).is_err());
    std::fs::remove_file(append(&target, "-wal")).unwrap();
    write(&marker_path(&target), b"interrupted");
    let error = KeyedActivationSession::prepare(&target, verified(&candidate))
        .err()
        .unwrap();
    assert_eq!(error.code, ErrorCode::NotReady);
    assert!(
        super::super::DatabaseKeySession::prepare(
            &target,
            &NeverVault,
            super::super::DatabaseCreation::AllowNew,
        )
        .is_err()
    );
}

struct NeverVault;
impl super::super::DatabaseKeyVault for NeverVault {
    fn load(
        &self,
        _: &super::super::DatabaseKeyIdentity,
    ) -> std::result::Result<Option<super::super::DatabaseKey>, super::super::DatabaseKeyError>
    {
        panic!("pending activation must block before vault access")
    }
    fn store_new(
        &self,
        _: &super::super::DatabaseKeyIdentity,
        _: &super::super::DatabaseKey,
    ) -> std::result::Result<(), super::super::DatabaseKeyError> {
        panic!("pending activation must block before vault access")
    }
}

#[test]
fn malformed_or_unready_candidate_metadata_is_refused() {
    let (_directory, target, candidate) = fixture();
    write(&append(&candidate, KEY_SUFFIX), b"{}");
    assert!(KeyedActivationSession::prepare(&target, verified(&candidate)).is_err());
    write(&append(&candidate, KEY_SUFFIX), br#"{"version":1,"database_id":"01234567-89ab-4def-8123-456789abcdef","generation":1,"ready":false}"#);
    assert!(KeyedActivationSession::prepare(&target, verified(&candidate)).is_err());
}

#[test]
fn interrupted_after_source_move_rolls_back_and_preserves_candidate() {
    let (_directory, target, candidate) = fixture();
    let session = KeyedActivationSession::prepare(&target, verified(&candidate)).unwrap();
    publish_marker(&marker_path(&target), &session.marker).unwrap();
    std::fs::create_dir(&session.preview.recovery_bundle).unwrap();
    std::fs::rename(
        &target,
        session.preview.recovery_bundle.join("plaintext.sqlite"),
    )
    .unwrap();
    drop(session);

    let interrupted = InterruptedKeyedActivation::open(&target).unwrap();
    let bundle = interrupted.rollback().unwrap();
    assert_eq!(
        std::fs::read(&target).unwrap(),
        b"SQLite format 3\0plaintext authored intent"
    );
    assert_eq!(
        std::fs::read(bundle.join("keyed.sqlite")).unwrap(),
        b"encrypted keyed candidate authored intent"
    );
    assert!(bundle.join("keyed.key.json").exists());
    assert!(!marker_path(&target).exists());
}

#[test]
fn interrupted_after_keyed_install_rolls_back_without_deleting_key_evidence() {
    let (_directory, target, candidate) = fixture();
    let session = KeyedActivationSession::prepare(&target, verified(&candidate)).unwrap();
    publish_marker(&marker_path(&target), &session.marker).unwrap();
    std::fs::create_dir(&session.preview.recovery_bundle).unwrap();
    std::fs::rename(
        &target,
        session.preview.recovery_bundle.join("plaintext.sqlite"),
    )
    .unwrap();
    std::fs::rename(&candidate, &target).unwrap();
    std::fs::rename(append(&candidate, KEY_SUFFIX), append(&target, KEY_SUFFIX)).unwrap();
    drop(session);

    let bundle = InterruptedKeyedActivation::open(&target)
        .unwrap()
        .rollback()
        .unwrap();
    assert_eq!(
        std::fs::read(&target).unwrap(),
        b"SQLite format 3\0plaintext authored intent"
    );
    assert_eq!(
        std::fs::read(bundle.join("keyed.sqlite")).unwrap(),
        b"encrypted keyed candidate authored intent"
    );
    assert!(bundle.join("keyed.key.json").exists());
    assert!(!append(&target, KEY_SUFFIX).exists());
}
