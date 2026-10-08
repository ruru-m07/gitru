use super::*;
use collaboration::database_keys::{
    DatabaseCreation, DatabaseKey, DatabaseKeyError, DatabaseKeyIdentity, DatabaseKeyVault,
};
use std::{
    collections::HashMap,
    path::Path,
    sync::{
        Condvar, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};

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
        let mut values = self.0.lock().unwrap();
        if values.contains_key(&identity.vault_reference()) {
            return Err(DatabaseKeyError::VaultAlreadyExists);
        }
        values.insert(identity.vault_reference(), *key.expose());
        Ok(())
    }
}
fn reserve(path: &Path, vault: &Vault) -> DatabaseKeySession {
    DatabaseKeySession::prepare(path, vault, DatabaseCreation::AllowNew).unwrap()
}
fn assert_busy(path: &Path, vault: &Vault) {
    assert!(matches!(
        DatabaseKeySession::prepare(path, vault, DatabaseCreation::ExistingOnly),
        Err(DatabaseKeyError::Busy)
    ));
}
async fn released(path: &Path, vault: &Vault) {
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            match DatabaseKeySession::prepare(path, vault, DatabaseCreation::ExistingOnly) {
                Err(DatabaseKeyError::Busy) => tokio::time::sleep(Duration::from_millis(5)).await,
                Ok(session) => {
                    drop(session);
                    break;
                }
                other => panic!("unexpected session result: {other:?}"),
            }
        }
    })
    .await
    .unwrap();
}
fn canary_absent(path: &Path) {
    for suffix in ["", "-wal", "-journal"] {
        let mut name = path.as_os_str().to_owned();
        name.push(suffix);
        if let Ok(bytes) = std::fs::read(Path::new(&name)) {
            assert!(
                !bytes
                    .windows(CANARY.len())
                    .any(|part| part == CANARY.as_bytes())
            );
        }
    }
}
const CANARY: &str = "GITRU_OWNED_KEYED_CONNECTION_SYNTHETIC_CANARY_8419";

#[tokio::test]
async fn actual_cipher_all_roles_three_readers_replacement_and_cold_reopen() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("data.sqlite");
    let vault = Vault::default();
    let factory = KeyedConnectionFactory::new(reserve(&path, &vault));
    let mut writer = factory.open(HandleRole::Writer).await.unwrap();
    sqlx::raw_sql("CREATE TABLE sample(body TEXT NOT NULL) STRICT; CREATE VIRTUAL TABLE search USING fts5(body); PRAGMA wal_autocheckpoint=0;").execute(writer.connection()).await.unwrap();
    sqlx::query("INSERT INTO sample VALUES(?)")
        .bind(CANARY)
        .execute(writer.connection())
        .await
        .unwrap();
    sqlx::query("INSERT INTO search VALUES(?)")
        .bind(CANARY)
        .execute(writer.connection())
        .await
        .unwrap();
    let pool = factory.reader_pool().await.unwrap();
    let mut readers = vec![];
    for _ in 0..3 {
        readers.push(pool.pool().acquire().await.unwrap());
    }
    for connection in &mut readers {
        let value: String = sqlx::query_scalar("SELECT body FROM sample")
            .fetch_one(&mut **connection)
            .await
            .unwrap();
        assert_eq!(value, CANARY);
        assert!(
            sqlx::query("DELETE FROM sample")
                .execute(&mut **connection)
                .await
                .is_err()
        );
    }
    for connection in readers {
        connection.close().await.unwrap();
    }
    // The pool must authenticate every replacement, not only its first handle.
    let matches: i64 = sqlx::query_scalar("SELECT count(*) FROM search WHERE search MATCH ?")
        .bind(CANARY)
        .fetch_one(pool.pool())
        .await
        .unwrap();
    assert_eq!(matches, 1);
    for role in [
        HandleRole::Reader,
        HandleRole::Maintenance,
        HandleRole::Recovery,
        HandleRole::Verifier,
    ] {
        let mut connection = factory.open(role).await.unwrap();
        let status: String = sqlx::query_scalar("PRAGMA cipher_status")
            .fetch_one(connection.connection())
            .await
            .unwrap();
        assert_eq!(status, "1");
        if role.read_only() {
            assert!(
                sqlx::query("DELETE FROM sample")
                    .execute(connection.connection())
                    .await
                    .is_err()
            );
        } else {
            let _: (i64, i64, i64) = sqlx::query_as("PRAGMA wal_checkpoint(PASSIVE)")
                .fetch_one(connection.connection())
                .await
                .unwrap();
        }
        connection.close().await.unwrap();
    }
    canary_absent(&path);
    pool.close().await.unwrap();
    writer.close().await.unwrap();
    drop(factory);
    released(&path, &vault).await;
    let cold = KeyedConnectionFactory::new(reserve(&path, &vault));
    let mut reader = cold.open(HandleRole::Verifier).await.unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT body FROM sample")
            .fetch_one(reader.connection())
            .await
            .unwrap(),
        CANARY
    );
    reader.close().await.unwrap();
    drop(cold);
    let unkeyed = SqliteConnection::connect_with(
        &SqliteConnectOptions::new().filename(&path).read_only(true),
    )
    .await;
    if let Ok(mut conn) = unkeyed {
        assert!(
            sqlx::query("SELECT * FROM sample")
                .fetch_all(&mut conn)
                .await
                .is_err()
        );
        conn.close().await.unwrap();
    }
}

#[tokio::test]
async fn wrong_key_and_plaintext_refused_without_replacement_or_ready() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("data.sqlite");
    let vault = Vault::default();
    let factory = KeyedConnectionFactory::new(reserve(&path, &vault));
    let reference = factory.state.session.identity().vault_reference();
    let mut writer = factory.open(HandleRole::Writer).await.unwrap();
    sqlx::query("CREATE TABLE sample(v)")
        .execute(writer.connection())
        .await
        .unwrap();
    writer.close().await.unwrap();
    drop(factory);
    let before = std::fs::read(&path).unwrap();
    vault.0.lock().unwrap().insert(reference, [7; 32]);
    let wrong = KeyedConnectionFactory::new(reserve(&path, &vault));
    assert!(wrong.open(HandleRole::Verifier).await.is_err());
    drop(wrong);
    assert_eq!(std::fs::read(&path).unwrap(), before);
    let path = temp.path().join("plain.sqlite");
    let reserved = reserve(&path, &vault);
    let mut plain = SqliteConnection::connect_with(
        &SqliteConnectOptions::new()
            .filename(&path)
            .create_if_missing(true),
    )
    .await
    .unwrap();
    sqlx::query("CREATE TABLE private(v)")
        .execute(&mut plain)
        .await
        .unwrap();
    plain.close().await.unwrap();
    let before = std::fs::read(&path).unwrap();
    let factory = KeyedConnectionFactory::new(reserved);
    assert!(factory.open(HandleRole::Verifier).await.is_err());
    drop(factory);
    assert_eq!(std::fs::read(&path).unwrap(), before);
    let metadata = std::fs::read_to_string(path.with_file_name("plain.sqlite.key.json")).unwrap();
    assert!(metadata.contains("\"ready\":false"));
}

#[derive(Default)]
struct Gate {
    entered: AtomicBool,
    state: Mutex<bool>,
    changed: Condvar,
}
impl Gate {
    fn block(&self) {
        self.entered.store(true, Ordering::Release);
        let guard = self.state.lock().unwrap();
        drop(
            self.changed
                .wait_while(guard, |released| !*released)
                .unwrap(),
        );
    }
    fn release(&self) {
        *self.state.lock().unwrap() = true;
        self.changed.notify_all();
    }
    async fn entered(&self) {
        tokio::time::timeout(Duration::from_secs(5), async {
            while !self.entered.load(Ordering::Acquire) {
                tokio::time::sleep(Duration::from_millis(1)).await;
            }
        })
        .await
        .unwrap();
    }
}

#[tokio::test]
async fn cancelled_or_failed_initializer_retains_actual_worker_key_and_os_lease() {
    for fail in [false, true] {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("cancel.sqlite");
        let vault = Vault::default();
        let factory = KeyedConnectionFactory::new(reserve(&path, &vault));
        let state = factory.state.clone();
        let gate = Arc::new(Gate::default());
        let held = gate.clone();
        // SAFETY: bounded owner keys, then waits for the parent gate. The raw
        // handle is never retained; the cancellation future cannot drop owner.
        let options = unsafe {
            factory
                .options(HandleRole::Writer)
                .unwrap()
                .before_initialize(move || {
                    Ok(Box::new(TestOwner {
                        owner: HandleOwner::reserve(state.clone())?,
                        gate: Some(held.clone()),
                        fail,
                        trace: None,
                    }))
                })
        };
        drop(factory);
        let task = tokio::spawn(async move { SqliteConnection::connect_with(&options).await });
        gate.entered().await;
        task.abort();
        let _ = task.await;
        assert_busy(&path, &vault);
        gate.release();
        released(&path, &vault).await;
    }
}

#[tokio::test]
async fn dropping_factory_and_pool_keeps_lease_until_last_reader_worker_closes() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("reader.sqlite");
    let vault = Vault::default();
    let factory = KeyedConnectionFactory::new(reserve(&path, &vault));
    let mut writer = factory.open(HandleRole::Writer).await.unwrap();
    sqlx::query("CREATE TABLE sample(v)")
        .execute(writer.connection())
        .await
        .unwrap();
    writer.close().await.unwrap();
    let pool = factory.reader_pool().await.unwrap();
    let reader = pool.pool().acquire().await.unwrap();
    drop(factory);
    let observing_pool = pool.pool().clone();
    let closing = tokio::spawn(pool.close());
    tokio::time::timeout(Duration::from_secs(5), async {
        while !observing_pool.is_closed() {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    drop(observing_pool);
    closing.abort();
    let _ = closing.await;
    assert_busy(&path, &vault);
    reader.close().await.unwrap();
    released(&path, &vault).await;
}

#[tokio::test]
async fn hook_runs_before_first_sql_and_options_never_contain_key_text() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("trace.sqlite");
    let vault = Vault::default();
    let factory = KeyedConnectionFactory::new(reserve(&path, &vault));
    let key_hex = factory
        .state
        .session
        .key()
        .expose()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>();
    let options = factory.options(HandleRole::Writer).unwrap();
    assert!(!format!("{options:?} {factory:?}").contains(&key_hex));
    let trace = Arc::new(Mutex::new(Vec::<String>::new()));
    let state = factory.state.clone();
    let observed = trace.clone();
    // SAFETY: bounded owner captures trace data until after native close.
    let options = unsafe {
        options.before_initialize(move || {
            Ok(Box::new(TestOwner {
                owner: HandleOwner::reserve(state.clone())?,
                gate: None,
                fail: false,
                trace: Some(observed.clone()),
            }))
        })
    };
    let mut connection = SqliteConnection::connect_with(&options).await.unwrap();
    authenticate(&mut connection).await.unwrap();
    connection.close().await.unwrap();
    let events = trace.lock().unwrap();
    assert!(!events.is_empty());
    assert!(events[0].starts_with("PRAGMA journal_mode"));
    assert!(
        !events
            .iter()
            .any(|sql| sql.contains(&key_hex) || sql.to_lowercase().contains("pragma key"))
    );
}

struct TestOwner {
    owner: HandleOwner,
    gate: Option<Arc<Gate>>,
    fail: bool,
    trace: Option<Arc<Mutex<Vec<String>>>>,
}
impl fmt::Debug for TestOwner {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("TestOwner([SYNTHETIC])")
    }
}
// SAFETY: fixed synthetic callbacks retain no raw pointers/statements and capture
// trace memory through actual close. Gate is parent-released under a watchdog.
unsafe impl SqliteNativeHandleOwner for TestOwner {
    fn initialize(
        &self,
        handle: std::ptr::NonNull<libsqlite3_sys::sqlite3>,
    ) -> Result<(), sqlx::Error> {
        assert_eq!(
            unsafe { libsqlite3_sys::sqlite3_next_stmt(handle.as_ptr(), std::ptr::null_mut()) },
            std::ptr::null_mut()
        );
        self.owner.initialize(handle)?;
        if let Some(gate) = &self.gate {
            gate.block();
        }
        if self.fail {
            return Err(sqlx::Error::Protocol(
                "synthetic initializer refusal".into(),
            ));
        }
        if let Some(observed) = &self.trace {
            unsafe extern "C" fn record(
                _: u32,
                context: *mut std::ffi::c_void,
                _: *mut std::ffi::c_void,
                text: *mut std::ffi::c_void,
            ) -> i32 {
                // SAFETY: owner retains context until close; trace gives live SQL.
                let events = unsafe { &*context.cast::<Mutex<Vec<String>>>() };
                let sql = unsafe { std::ffi::CStr::from_ptr(text.cast()) }
                    .to_string_lossy()
                    .into_owned();
                events.lock().unwrap().push(sql);
                0
            }
            assert_eq!(
                unsafe {
                    libsqlite3_sys::sqlite3_trace_v2(
                        handle.as_ptr(),
                        libsqlite3_sys::SQLITE_TRACE_STMT,
                        Some(record),
                        Arc::as_ptr(observed).cast_mut().cast(),
                    )
                },
                0
            );
        }
        Ok(())
    }
    fn close_failed(&self) {
        self.owner.close_failed();
    }
}

#[tokio::test]
async fn native_handle_bound_refuses_before_open_and_releases_after_close() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("bound.sqlite");
    let vault = Vault::default();
    let factory = KeyedConnectionFactory::new(reserve(&path, &vault));
    let mut writer = factory.open(HandleRole::Writer).await.unwrap();
    sqlx::query("CREATE TABLE sample(v)")
        .execute(writer.connection())
        .await
        .unwrap();
    let mut readers = Vec::new();
    for _ in 1..MAX_NATIVE_HANDLES {
        readers.push(factory.open(HandleRole::Reader).await.unwrap());
    }
    assert_eq!(
        factory.state.admission.lock().unwrap().open,
        MAX_NATIVE_HANDLES
    );
    assert!(matches!(
        factory.open(HandleRole::Reader).await,
        Err(KeyedError::Admission)
    ));
    readers.pop().unwrap().close().await.unwrap();
    let replacement = factory.open(HandleRole::Reader).await.unwrap();
    replacement.close().await.unwrap();
    for reader in readers {
        reader.close().await.unwrap();
    }
    writer.close().await.unwrap();
    assert_eq!(factory.state.admission.lock().unwrap().open, 0);
}

#[test]
fn native_close_failure_retires_factory_and_retains_key_lease_until_process_exit() {
    let temp = tempfile::tempdir().unwrap();
    let mut child = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "tests::native_close_failure_child",
            "--ignored",
            "--nocapture",
        ])
        .env("GITRU_KEYED_CLOSE_FAILURE_FIXTURE", temp.path())
        .stdout(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::inherit())
        .spawn()
        .unwrap();
    let deadline = std::time::Instant::now() + Duration::from_secs(30);
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            assert!(
                status.success(),
                "native close failure child exited with {status}"
            );
            break;
        }
        if std::time::Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            panic!("native close failure fixture exceeded deadline");
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(
        std::fs::read_to_string(temp.path().join("evidence.txt")).unwrap(),
        "faulted;one retained owner;zero subsequent admissions;lease busy"
    );
}

#[tokio::test]
#[ignore = "isolated exceptional native close failure; parent owns watchdog"]
async fn native_close_failure_child() {
    let Some(directory) = std::env::var_os("GITRU_KEYED_CLOSE_FAILURE_FIXTURE") else {
        return;
    };
    let directory = std::path::PathBuf::from(directory);
    eprintln!("native close fixture: entered");
    let path = directory.join("busy.sqlite");
    let vault = Vault::default();
    let factory = KeyedConnectionFactory::new(reserve(&path, &vault));
    let mut writer = factory.open(HandleRole::Writer).await.unwrap();
    sqlx::query("CREATE TABLE sample(v)")
        .execute(writer.connection())
        .await
        .unwrap();
    {
        let mut locked = writer.connection().lock_handle().await.unwrap();
        let mut statement = std::ptr::null_mut();
        // Deliberate fixture-only invariant violation: leave a native statement
        // alive to force sqlite3_close to return SQLITE_BUSY. Process exit owns
        // cleanup; this path is never available in the factory API.
        assert_eq!(
            unsafe {
                libsqlite3_sys::sqlite3_prepare_v2(
                    locked.as_raw_handle().as_ptr(),
                    c"SELECT * FROM sample".as_ptr(),
                    -1,
                    &mut statement,
                    std::ptr::null_mut(),
                )
            },
            0
        );
        assert!(!statement.is_null());
    }
    eprintln!("native close fixture: native statement retained");
    assert!(writer.close().await.is_err());
    eprintln!("native close fixture: worker close error observed");
    {
        let admission = factory.state.admission.lock().unwrap();
        assert!(admission.faulted);
        assert_eq!(admission.open, 1);
    }
    for _ in 0..16 {
        assert!(matches!(
            factory.open(HandleRole::Reader).await,
            Err(KeyedError::Admission)
        ));
    }
    let state = Arc::downgrade(&factory.state);
    drop(factory);
    assert!(state.upgrade().is_some());
    assert_busy(&path, &vault);
    eprintln!("native close fixture: faulted admission and retained lease verified");
    std::fs::write(
        directory.join("evidence.txt"),
        "faulted;one retained owner;zero subsequent admissions;lease busy",
    )
    .unwrap();
}

#[cfg(unix)]
#[tokio::test]
async fn native_open_refuses_replaced_database_symlink() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("identity.sqlite");
    let vault = Vault::default();
    let factory = KeyedConnectionFactory::new(reserve(&path, &vault));
    let mut writer = factory.open(HandleRole::Writer).await.unwrap();
    sqlx::query("CREATE TABLE retained(v)")
        .execute(writer.connection())
        .await
        .unwrap();
    writer.close().await.unwrap();
    let retained = temp.path().join("retained.sqlite");
    std::fs::rename(&path, &retained).unwrap();
    let before = std::fs::read(&retained).unwrap();
    std::os::unix::fs::symlink(&retained, &path).unwrap();
    assert!(factory.open(HandleRole::Verifier).await.is_err());
    assert_eq!(before, std::fs::read(retained).unwrap());
    assert_eq!(factory.state.admission.lock().unwrap().open, 0);
}

#[tokio::test]
async fn consumed_creation_never_recreates_a_removed_database() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("created-once.sqlite");
    let vault = Vault::default();
    let factory = KeyedConnectionFactory::new(reserve(&path, &vault));
    let mut writer = factory.open(HandleRole::Writer).await.unwrap();
    sqlx::query("CREATE TABLE authored(v)")
        .execute(writer.connection())
        .await
        .unwrap();
    writer.close().await.unwrap();
    let retained = temp.path().join("retained.sqlite");
    std::fs::rename(&path, &retained).unwrap();
    let before = std::fs::read(&retained).unwrap();
    assert!(factory.open(HandleRole::Writer).await.is_err());
    assert!(!path.exists());
    assert_eq!(before, std::fs::read(retained).unwrap());
    assert_eq!(factory.state.admission.lock().unwrap().open, 0);
}
