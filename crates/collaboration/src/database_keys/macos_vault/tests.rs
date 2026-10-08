//! Actual OS access is opt-in, isolated to an owned fixture subprocess.
use super::*;
use crate::database_keys::{DatabaseCreation, DatabaseKeyMode, DatabaseKeySession};
use core_foundation::base::TCFType;
use security_framework_sys::keychain::SecKeychainCreate;
use sha2::{Digest, Sha256};
use std::{
    ffi::CString,
    fs::OpenOptions,
    io::Write,
    os::unix::{
        ffi::OsStrExt,
        fs::{OpenOptionsExt, PermissionsExt},
    },
    path::Path,
    process::{Command, Stdio},
    sync::Barrier,
    time::{Duration, Instant},
};
#[link(name = "Security", kind = "framework")]
unsafe extern "C" {
    fn SecKeychainLock(keychain: SecKeychainRef) -> i32;
    fn SecKeychainDelete(keychain: SecKeychainRef) -> i32;
}
const CHILD: &str = "database_keys::macos_vault::tests::native_fixture_child";

#[test]
fn public_errors_and_owned_secret_debug_do_not_disclose_values() {
    let key = DatabaseKey::from_bytes([97; 32]);
    assert_eq!(format!("{key:?}"), "DatabaseKey([REDACTED])");
    assert!(!Error::VaultAlreadyExists.to_string().contains("aaaa"));
    assert!(!Error::VaultInvalidKey.to_string().contains("aaaa"));
    assert_eq!(SERVICE, "com.gitru.collaboration.database-key.v1");
}
fn write_private(path: &Path, bytes: &[u8]) {
    let mut f = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
        .unwrap();
    f.write_all(bytes).unwrap();
    f.sync_all().unwrap();
}
fn child(root: &Path, phase: &str, expected: i32) -> Result<(), String> {
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", CHILD, "--ignored", "--test-threads=1"])
        .env("GITRU_DATABASE_VAULT_FIXTURE_ROOT", root)
        .env("GITRU_DATABASE_VAULT_FIXTURE_PHASE", phase)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(60);
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            let stage = std::fs::read_to_string(root.join("stage")).unwrap_or_default();
            return if status.code() == Some(expected) {
                Ok(())
            } else {
                Err(format!("fixture phase {phase}, last stage {stage}"))
            };
        }
        if Instant::now() > deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Err(format!(
                "bounded keychain fixture timed out in phase {phase}"
            ));
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}
#[test]
#[ignore = "explicit opt-in; disposable native keychain subprocess only"]
fn actual_private_keychain_create_only_lock_and_crash_resume() {
    assert_eq!(
        std::env::var("GITRU_TEST_DISPOSABLE_DATABASE_KEYCHAIN").as_deref(),
        Ok("1")
    );
    let root = tempfile::tempdir().unwrap();
    std::fs::set_permissions(root.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
    write_private(
        &root.path().join("authorization"),
        b"gitru-disposable-database-keychain-v1",
    );
    let result = child(root.path(), "create", 43).and_then(|()| child(root.path(), "resume", 0));
    let cleanup = child(root.path(), "cleanup", 0);
    result.unwrap();
    cleanup.unwrap();
    assert!(!root.path().join("fixture.keychain-db").exists());
    assert!(!root.path().join("fixture.keychain").exists());
    assert!(!root.path().join("other.keychain-db").exists());
    assert!(!root.path().join("other.keychain").exists());
}
fn stage(root: &Path, text: &str) {
    std::fs::write(root.join("stage"), text).unwrap();
}
fn create_keychain(file: &Path, password: &str) -> SecKeychain {
    let path = CString::new(file.as_os_str().as_bytes()).unwrap();
    let mut handle = ptr::null_mut();
    // Borrow the zeroizing password without the high-level builder's
    // additional String copy. The OS manages its own internal copies.
    assert_eq!(
        unsafe {
            SecKeychainCreate(
                path.as_ptr(),
                password.len() as u32,
                password.as_ptr().cast(),
                0,
                ptr::null_mut(),
                &mut handle,
            )
        },
        errSecSuccess
    );
    let mut keychain = unsafe { SecKeychain::wrap_under_create_rule(handle) };
    keychain.unlock(Some(password)).unwrap();
    keychain
}

#[test]
#[ignore = "subprocess fixture entrypoint; only generated private paths"]
fn native_fixture_child() {
    let path =
        std::env::var_os("GITRU_DATABASE_VAULT_FIXTURE_ROOT").expect("fixture path required");
    let root = Path::new(&path);
    assert!(
        !std::fs::symlink_metadata(root)
            .unwrap()
            .file_type()
            .is_symlink()
    );
    assert_eq!(
        std::fs::metadata(root).unwrap().permissions().mode() & 0o077,
        0
    );
    assert_eq!(
        std::fs::read(root.join("authorization")).unwrap(),
        b"gitru-disposable-database-keychain-v1"
    );
    let phase = std::env::var("GITRU_DATABASE_VAULT_FIXTURE_PHASE").unwrap();
    // This flag is process-wide, which is precisely why actual calls run only
    // in this dedicated fixture child, never in the application process.
    let _no_ui = SecKeychain::disable_user_interaction().unwrap();
    let file = root.join("fixture.keychain");
    if phase == "create" {
        stage(root, "create-keychain");
        assert!(!file.exists());
        let password = Zeroizing::new(format!("{}{}", uuid::Uuid::new_v4(), uuid::Uuid::new_v4()));
        write_private(&root.join("password"), password.as_bytes());
        let keychain = create_keychain(&file, &password);
        let vault = FileKeychainDatabaseVault::from_keychain(keychain);
        stage(root, "reserve-and-create-key");
        let session =
            DatabaseKeySession::prepare(&root.join("cache.db"), &vault, DatabaseCreation::AllowNew)
                .unwrap();
        assert_eq!(session.mode(), DatabaseKeyMode::CreateNew);
        write_private(
            &root.join("key-digest"),
            &Sha256::digest(session.key().expose()),
        );
        // Simulate death after the real OS vault write, before DB creation or Ready.
        stage(root, "vault-write-complete");
        std::process::exit(43);
    }
    let password = Zeroizing::new(std::fs::read_to_string(root.join("password")).unwrap());
    let mut keychain = SecKeychain::open(&file).unwrap();
    keychain.unlock(Some(&password)).unwrap();
    if phase == "cleanup" {
        stage(root, "delete-owned-keychain");
        assert_eq!(
            unsafe { SecKeychainDelete(keychain.as_concrete_TypeRef()) },
            errSecSuccess
        );
        let other = root.join("other.keychain");
        if other.exists() || root.join("other.keychain-db").exists() {
            let other = SecKeychain::open(&other).unwrap();
            assert_eq!(
                unsafe { SecKeychainDelete(other.as_concrete_TypeRef()) },
                errSecSuccess
            );
        }
        return;
    }
    assert_eq!(phase, "resume");
    let vault = FileKeychainDatabaseVault::from_keychain(keychain.clone());
    stage(root, "resume-exact-reservation");
    let session = DatabaseKeySession::prepare(
        &root.join("cache.db"),
        &vault,
        DatabaseCreation::ExistingOnly,
    )
    .unwrap();
    assert_eq!(session.mode(), DatabaseKeyMode::CreateNew);
    let identity = session.identity();
    assert_eq!(
        Sha256::digest(session.key().expose()).as_slice(),
        std::fs::read(root.join("key-digest")).unwrap()
    );
    assert_eq!(
        vault.store_new(&identity, &DatabaseKey::from_bytes([17; 32])),
        Err(Error::VaultAlreadyExists)
    );
    assert_eq!(
        vault.load(&identity).unwrap().unwrap().expose(),
        session.key().expose()
    );
    stage(root, "exact-keychain-isolation");
    let other = FileKeychainDatabaseVault::from_keychain(create_keychain(
        &root.join("other.keychain"),
        &password,
    ));
    assert!(other.load(&identity).unwrap().is_none());
    other
        .store_new(&identity, &DatabaseKey::from_bytes([99; 32]))
        .unwrap();
    assert_eq!(other.load(&identity).unwrap().unwrap().expose(), &[99; 32]);
    assert_eq!(
        vault.load(&identity).unwrap().unwrap().expose(),
        session.key().expose()
    );
    let other_only = fresh_identity(root, "other-only.db");
    other
        .store_new(&other_only, &DatabaseKey::from_bytes([98; 32]))
        .unwrap();
    assert!(vault.load(&other_only).unwrap().is_none());
    vault
        .store_new(&other_only, &DatabaseKey::from_bytes([97; 32]))
        .unwrap();
    assert_eq!(
        vault.load(&other_only).unwrap().unwrap().expose(),
        &[97; 32]
    );
    assert_eq!(
        other.load(&other_only).unwrap().unwrap().expose(),
        &[98; 32]
    );
    stage(root, "locked-refusal");
    assert_eq!(
        unsafe { SecKeychainLock(keychain.as_concrete_TypeRef()) },
        errSecSuccess
    );
    assert!(matches!(vault.load(&identity), Err(Error::VaultLocked)));
    assert_eq!(
        vault.store_new(&identity, &DatabaseKey::from_bytes([18; 32])),
        Err(Error::VaultLocked)
    );
    keychain.unlock(Some(&password)).unwrap();
    assert_eq!(
        vault.load(&identity).unwrap().unwrap().expose(),
        session.key().expose()
    );
    stage(root, "concurrent-create-only");
    let competing = fresh_identity(root, "competing.db");
    assert!(vault.load(&competing).unwrap().is_none());
    let barrier = Arc::new(Barrier::new(2));
    let outcomes = [41u8, 42]
        .map(|byte| {
            let vault = FileKeychainDatabaseVault::from_keychain(keychain.clone());
            let identity = competing.clone();
            let gate = barrier.clone();
            std::thread::spawn(move || {
                gate.wait();
                (
                    byte,
                    vault.store_new(&identity, &DatabaseKey::from_bytes([byte; 32])),
                )
            })
        })
        .map(|task| task.join().unwrap());
    assert_eq!(
        outcomes.iter().filter(|(_, result)| result.is_ok()).count(),
        1
    );
    assert_eq!(
        outcomes
            .iter()
            .filter(|(_, result)| *result == Err(Error::VaultAlreadyExists))
            .count(),
        1
    );
    let winner = outcomes
        .iter()
        .find(|(_, result)| result.is_ok())
        .unwrap()
        .0;
    assert_eq!(
        vault.load(&competing).unwrap().unwrap().expose(),
        &[winner; 32]
    );
    stage(root, "invalid-format-preserved");
    let malformed = fresh_identity(root, "invalid.db");
    vault.add(&malformed, &[55; 31]).unwrap();
    assert!(matches!(
        vault.load(&malformed),
        Err(Error::VaultInvalidKey)
    ));
    assert_eq!(
        vault.store_new(&malformed, &DatabaseKey::from_bytes([55; 32])),
        Err(Error::VaultAlreadyExists)
    );
    assert!(matches!(
        vault.load(&malformed),
        Err(Error::VaultInvalidKey)
    ));
    stage(root, "complete");
}
fn fresh_identity(root: &Path, name: &str) -> DatabaseKeyIdentity {
    // Generate a real private reservation without introducing a public identity
    // constructor. The capture vault has no OS access and exists only in fixtures.
    struct Capture(std::sync::Mutex<Option<DatabaseKey>>);
    impl DatabaseKeyVault for Capture {
        fn load(&self, _: &DatabaseKeyIdentity) -> Result<Option<DatabaseKey>, Error> {
            Ok(self
                .0
                .lock()
                .unwrap()
                .as_ref()
                .map(|key| DatabaseKey::from_bytes(*key.expose())))
        }
        fn store_new(&self, _: &DatabaseKeyIdentity, key: &DatabaseKey) -> Result<(), Error> {
            *self.0.lock().unwrap() = Some(DatabaseKey::from_bytes(*key.expose()));
            Ok(())
        }
    }
    DatabaseKeySession::prepare(
        &root.join(name),
        &Capture(std::sync::Mutex::new(None)),
        DatabaseCreation::AllowNew,
    )
    .unwrap()
    .identity()
}
