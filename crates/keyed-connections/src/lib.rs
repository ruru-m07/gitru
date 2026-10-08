//! Qualified opt-in native keyed connections.
//!
//! The SQLx adaptation retains the initializer's capture on the actual C handle,
//! including failed startup and cancelled futures. This factory does not publish
//! Ready, migrate application schemas, choose a platform vault, or convert data.
//! Desktop startup may select this factory only after native key metadata or an
//! exact retained-harness marker has selected keyed storage before runtime creation.
use collaboration::database_keys::lifecycle::{
    DatabaseKeyRotation, DatabaseKeyRotationReservation, VerifiedRotatedDatabase,
};
use collaboration::database_keys::{
    DatabaseKeyError, DatabaseKeyIdentity, DatabaseKeyMode, DatabaseKeySession,
};
use collaboration::portable_backup::VerifiedPortableBackup;
use sqlx::{
    ConnectOptions, Connection, SqliteConnection, SqlitePool,
    sqlite::{
        SqliteConnectOptions, SqliteJournalMode, SqliteNativeHandleOwner, SqlitePoolOptions,
        SqliteSynchronous,
    },
};
use std::{
    fmt,
    sync::atomic::{AtomicBool, Ordering},
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::sync::Notify;

#[cfg(test)]
mod tests;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HandleRole {
    Writer,
    Reader,
    Maintenance,
    Recovery,
    Verifier,
}
impl HandleRole {
    fn read_only(self) -> bool {
        matches!(self, Self::Reader | Self::Recovery | Self::Verifier)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyedError {
    Open,
    WrongKeyOrCorrupt,
    Profile,
    Closed,
    Admission,
}
impl fmt::Display for KeyedError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Open => "Keyed database handle could not open; existing files were preserved",
            Self::WrongKeyOrCorrupt => "Database key or encrypted contents could not be verified",
            Self::Profile => "Database cipher build or settings are not qualified",
            Self::Admission => "Keyed factory is faulted or its native handle bound was reached",
            Self::Closed => "Keyed database handle could not close safely",
        })
    }
}
impl std::error::Error for KeyedError {}

/// The key/session lease is captured by every native initializer. No secret is
/// copied into SQLite options, SQL text or logs, and no unkeyed fallback exists.
#[derive(Clone)]
pub struct KeyedConnectionFactory {
    state: Arc<FactoryState>,
}
// Bound includes direct handles, pooled replacements and failed-close owners.
const MAX_NATIVE_HANDLES: usize = 8;
#[derive(Default)]
struct Admission {
    open: usize,
    faulted: bool,
    creation_consumed: bool,
    retiring: bool,
    store_claimed: bool,
    session: Option<Arc<DatabaseKeySession>>,
}
struct FactoryState {
    path: std::path::PathBuf,
    identity: DatabaseKeyIdentity,
    mode: DatabaseKeyMode,
    admission: Mutex<Admission>,
    released: Notify,
}
struct HandleOwner {
    state: Arc<FactoryState>,
    session: Option<Arc<DatabaseKeySession>>,
}
impl fmt::Debug for HandleOwner {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("HandleOwner([REDACTED])")
    }
}
impl HandleOwner {
    fn reserve(state: Arc<FactoryState>) -> Result<Self, sqlx::Error> {
        let session = {
            let mut admission = state
                .admission
                .lock()
                .map_err(|_| sqlx::Error::Protocol("Native admission unavailable".into()))?;
            if admission.faulted || admission.retiring || admission.open >= MAX_NATIVE_HANDLES {
                return Err(sqlx::Error::Protocol("Native admission refused".into()));
            }
            let session = admission
                .session
                .clone()
                .ok_or_else(|| sqlx::Error::Protocol("Native admission closed".into()))?;
            admission.open += 1;
            session
        };
        Ok(Self {
            state,
            session: Some(session),
        })
    }
}
// SAFETY: initialization keys only the exclusive live handle. The reservation
// retains the key+lease; failed close permanently retires admission. No secret
// enters SQL, diagnostics, options Debug or callback Debug. Poison fails closed.
unsafe impl SqliteNativeHandleOwner for HandleOwner {
    fn initialize(
        &self,
        handle: std::ptr::NonNull<libsqlite3_sys::sqlite3>,
    ) -> Result<(), sqlx::Error> {
        if self
            .state
            .admission
            .lock()
            .map_err(|_| sqlx::Error::Protocol("Native admission unavailable".into()))?
            .faulted
        {
            return Err(sqlx::Error::Protocol("Native admission retired".into()));
        }
        let key = self
            .session
            .as_ref()
            .expect("reserved native owner")
            .key()
            .expose();
        // SAFETY: the driver supplies exclusive access before any SQL.
        let result = unsafe {
            libsqlite3_sys::sqlite3_key(handle.as_ptr(), key.as_ptr().cast(), key.len() as i32)
        };
        if result != libsqlite3_sys::SQLITE_OK {
            return Err(sqlx::Error::Protocol(
                "Native database key initialization failed".into(),
            ));
        }
        Ok(())
    }
    fn close_failed(&self) {
        // Poison itself also permanently prevents subsequent admission.
        if let Ok(mut admission) = self.state.admission.lock() {
            admission.faulted = true;
        }
    }
}
impl Drop for HandleOwner {
    fn drop(&mut self) {
        // Acknowledge only AFTER releasing this owner's session reference. The
        // SQLx close message can precede worker option destruction.
        drop(self.session.take());
        if let Ok(mut admission) = self.state.admission.lock() {
            admission.open -= 1;
        }
        self.state.released.notify_waiters();
    }
}
impl fmt::Debug for KeyedConnectionFactory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("KeyedConnectionFactory")
            .finish_non_exhaustive()
    }
}
impl KeyedConnectionFactory {
    pub fn new(session: DatabaseKeySession) -> Self {
        Self {
            state: Arc::new(FactoryState {
                path: session.path().to_owned(),
                identity: session.identity(),
                mode: session.mode(),
                admission: Mutex::new(Admission {
                    session: Some(Arc::new(session)),
                    ..Admission::default()
                }),
                released: Notify::new(),
            }),
        }
    }

    fn options(&self, role: HandleRole) -> Result<SqliteConnectOptions, KeyedError> {
        // This process-local creation authority is consumed before any await or
        // native open. A removed database cannot be silently recreated by a
        // later writer/replacement in the same owned factory session.
        let create = {
            let mut admission = self
                .state
                .admission
                .lock()
                .map_err(|_| KeyedError::Admission)?;
            let allowed = role == HandleRole::Writer
                && self.state.mode == DatabaseKeyMode::CreateNew
                && !admission.creation_consumed;
            if allowed {
                admission.creation_consumed = true;
            }
            allowed
        };
        let state = self.state.clone();
        let options = SqliteConnectOptions::new()
            .filename(&state.path)
            .read_only(role.read_only())
            .create_if_missing(create)
            .foreign_keys(true)
            .synchronous(SqliteSynchronous::Full)
            .pragma("temp_store", "MEMORY")
            .pragma("trusted_schema", "OFF")
            .optimize_on_close(false, None)
            .busy_timeout(Duration::from_secs(2))
            .statement_cache_capacity(32)
            .disable_statement_logging();
        let options = if role == HandleRole::Writer {
            options.journal_mode(SqliteJournalMode::Wal)
        } else if role.read_only() {
            options.pragma("query_only", "ON")
        } else {
            options
        };
        // SAFETY: the bounded owner obeys the driver lifecycle contract above.
        Ok(unsafe {
            options.before_initialize(move || Ok(Box::new(HandleOwner::reserve(state.clone())?)))
        })
    }

    fn available(&self) -> Result<(), KeyedError> {
        let admission = self
            .state
            .admission
            .lock()
            .map_err(|_| KeyedError::Admission)?;
        if admission.faulted || admission.retiring || admission.open >= MAX_NATIVE_HANDLES {
            return Err(KeyedError::Admission);
        }
        Ok(())
    }

    /// Permanently stop all admission, then release the key/lease only after
    /// every native owner acknowledges destruction. Closed option clones do not
    /// retain the session. Failure keeps it retained and never reopens admission.
    pub async fn retire(&self) -> Result<(), KeyedError> {
        {
            let mut admission = self
                .state
                .admission
                .lock()
                .map_err(|_| KeyedError::Closed)?;
            admission.retiring = true;
        }
        tokio::time::timeout(Duration::from_secs(30), async {
            loop {
                let notified = self.state.released.notified();
                tokio::pin!(notified);
                notified.as_mut().enable();
                let released = {
                    let mut admission = self
                        .state
                        .admission
                        .lock()
                        .map_err(|_| KeyedError::Closed)?;
                    if admission.faulted {
                        return Err(KeyedError::Closed);
                    }
                    if admission.open == 0 {
                        Some(admission.session.take())
                    } else {
                        None
                    }
                };
                if let Some(session) = released {
                    drop(session);
                    return Ok(());
                }
                notified.await;
            }
        })
        .await
        .map_err(|_| KeyedError::Closed)?
    }

    pub async fn open(&self, role: HandleRole) -> Result<KeyedConnection, KeyedError> {
        self.available()?;
        let mut connection = SqliteConnection::connect_with(&self.options(role)?)
            .await
            .map_err(|_| KeyedError::Open)?;
        if let Err(error) = authenticate(&mut connection).await {
            connection.close().await.map_err(|_| KeyedError::Closed)?;
            return Err(error);
        }
        Ok(KeyedConnection {
            connection: Some(connection),
        })
    }

    pub async fn reader_pool(&self) -> Result<KeyedReaderPool, KeyedError> {
        self.available()?;
        let pool = SqlitePoolOptions::new()
            .max_connections(3)
            .min_connections(0)
            .acquire_timeout(Duration::from_secs(5))
            .after_connect(|connection, _| {
                Box::pin(async move {
                    authenticate(connection).await.map_err(|_| {
                        sqlx::Error::Protocol("Keyed reader authentication failed".into())
                    })
                })
            })
            .connect_with(self.options(HandleRole::Reader)?)
            .await
            .map_err(|_| KeyedError::Open)?;
        Ok(KeyedReaderPool {
            pool,
            state: self.state.clone(),
        })
    }
}

struct PortableImportState {
    key: Mutex<[u8; 32]>,
    faulted: AtomicBool,
}
impl Drop for PortableImportState {
    fn drop(&mut self) {
        use zeroize::Zeroize;
        if let Ok(key) = self.key.get_mut() {
            key.zeroize();
        }
    }
}
struct PortableImportOwner(Arc<PortableImportState>);
impl fmt::Debug for PortableImportOwner {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("PortableImportOwner([REDACTED])")
    }
}
// SAFETY: the key is applied to the exclusive new handle before SQL, retained
// through actual close and zeroized with the final owner. A close failure
// permanently faults this one-shot import state.
unsafe impl SqliteNativeHandleOwner for PortableImportOwner {
    fn initialize(
        &self,
        handle: std::ptr::NonNull<libsqlite3_sys::sqlite3>,
    ) -> Result<(), sqlx::Error> {
        if self.0.faulted.load(Ordering::Acquire) {
            return Err(sqlx::Error::Protocol("Portable import retired".into()));
        }
        let key = self
            .0
            .key
            .lock()
            .map_err(|_| sqlx::Error::Protocol("Portable import retired".into()))?;
        let status = unsafe {
            libsqlite3_sys::sqlite3_key(handle.as_ptr(), key.as_ptr().cast(), key.len() as i32)
        };
        if status == libsqlite3_sys::SQLITE_OK {
            Ok(())
        } else {
            self.0.faulted.store(true, Ordering::Release);
            Err(sqlx::Error::Protocol(
                "Portable import keying failed".into(),
            ))
        }
    }
    fn close_failed(&self) {
        self.0.faulted.store(true, Ordering::Release);
    }
}

/// Import an authenticated portable backup into a fresh next-generation
/// SQLCipher database and return the existing crash-recoverable rotation.
/// The original device key is not read or copied into the portable artifact.
pub async fn import_portable_backup(
    portable: &VerifiedPortableBackup,
    reservation: DatabaseKeyRotationReservation,
) -> Result<DatabaseKeyRotation, DatabaseKeyError> {
    let candidate = reservation.target().parent().unwrap().join(format!(
        ".gitru-portable-import-{}.sqlite",
        uuid::Uuid::new_v4()
    ));
    let state = Arc::new(PortableImportState {
        key: Mutex::new(*reservation.key().expose()),
        faulted: AtomicBool::new(false),
    });
    {
        let mut create = std::fs::OpenOptions::new();
        create.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            create.mode(0o600);
        }
        create
            .open(&candidate)
            .map_err(|_| DatabaseKeyError::Storage)?;
    }
    let owner = state.clone();
    let options = SqliteConnectOptions::new()
        .filename(&candidate)
        .create_if_missing(false)
        .foreign_keys(true)
        .journal_mode(SqliteJournalMode::Delete)
        .synchronous(SqliteSynchronous::Full)
        .pragma("temp_store", "MEMORY")
        .pragma("trusted_schema", "OFF")
        .disable_statement_logging();
    let options = unsafe {
        options.before_initialize(move || {
            if owner.faulted.load(Ordering::Acquire) {
                return Err(sqlx::Error::Protocol("Portable import retired".into()));
            }
            Ok(Box::new(PortableImportOwner(owner.clone())))
        })
    };
    let result = async {
        let mut source = SqliteConnection::connect_with(
            &SqliteConnectOptions::new()
                .filename(portable.native_import_path())
                .read_only(false)
                .pragma("trusted_schema", "OFF"),
        )
        .await
        .map_err(|_| DatabaseKeyError::WrongKeyOrCorrupt)?;
        sqlx::query("ATTACH DATABASE ? AS portable KEY ''")
            .bind(candidate.to_str().ok_or(DatabaseKeyError::Storage)?)
            .execute(&mut source)
            .await
            .map_err(|_| DatabaseKeyError::Storage)?;
        {
            let mut source_handle = source
                .lock_handle()
                .await
                .map_err(|_| DatabaseKeyError::Storage)?;
            let key = reservation.key().expose();
            let status = unsafe {
                libsqlite3_sys::sqlite3_key_v2(
                    source_handle.as_raw_handle().as_ptr(),
                    c"portable".as_ptr(),
                    key.as_ptr().cast(),
                    key.len() as i32,
                )
            };
            if status != libsqlite3_sys::SQLITE_OK {
                return Err(DatabaseKeyError::Storage);
            }
        }
        sqlx::raw_sql(
            "PRAGMA portable.cipher_page_size=4096;
             PRAGMA portable.kdf_iter=256000;
             PRAGMA portable.cipher_hmac_algorithm=HMAC_SHA512;
             PRAGMA portable.cipher_kdf_algorithm=PBKDF2_HMAC_SHA512;
             SELECT sqlcipher_export('portable');",
        )
        .execute(&mut source)
        .await
        .map_err(|_| DatabaseKeyError::Storage)?;
        let identity = reservation.identity();
        sqlx::query("DELETE FROM portable.database_storage_identity")
            .execute(&mut source)
            .await
            .map_err(|_| DatabaseKeyError::Storage)?;
        sqlx::query("INSERT INTO portable.database_storage_identity VALUES(1,1,?,?,?)")
            .bind(identity.database_id())
            .bind(i64::try_from(identity.generation()).map_err(|_| DatabaseKeyError::Storage)?)
            .bind(collaboration::storage::keyed::CIPHER_PROFILE)
            .execute(&mut source)
            .await
            .map_err(|_| DatabaseKeyError::Storage)?;
        sqlx::raw_sql("PRAGMA portable.journal_mode=DELETE; DETACH DATABASE portable;")
            .execute(&mut source)
            .await
            .map_err(|_| DatabaseKeyError::Storage)?;
        source
            .close()
            .await
            .map_err(|_| DatabaseKeyError::Storage)?;

        let mut destination = SqliteConnection::connect_with(&options)
            .await
            .map_err(|_| DatabaseKeyError::WrongKeyOrCorrupt)?;
        authenticate(&mut destination)
            .await
            .map_err(|_| DatabaseKeyError::WrongKeyOrCorrupt)?;
        collaboration::storage::keyed::verify_portable_import(
            &mut destination,
            &identity,
            portable.summary(),
        )
        .await
        .map_err(|_| DatabaseKeyError::WrongKeyOrCorrupt)?;
        destination
            .close()
            .await
            .map_err(|_| DatabaseKeyError::Storage)?;
        if state.faulted.load(Ordering::Acquire) {
            return Err(DatabaseKeyError::Storage);
        }
        reservation.publish_imported_candidate_metadata(&candidate)?;
        let proof = unsafe {
            VerifiedRotatedDatabase::from_native_verification(candidate.clone(), identity)
        };
        reservation.prepare_verified_candidate(proof)
    }
    .await;
    if result.is_err() {
        for suffix in [
            "",
            "-wal",
            "-shm",
            "-journal",
            ".key.json",
            ".key.json.pending",
        ] {
            let mut path = candidate.as_os_str().to_os_string();
            path.push(suffix);
            let _ = std::fs::remove_file(std::path::PathBuf::from(path));
        }
    }
    result
}

pub struct KeyedConnection {
    connection: Option<SqliteConnection>,
}
impl KeyedConnection {
    /// Native execution only; the key and session are not exposed by this API.
    pub fn connection(&mut self) -> &mut SqliteConnection {
        self.connection.as_mut().expect("owned keyed connection")
    }
    pub async fn close(mut self) -> Result<(), KeyedError> {
        self.connection
            .take()
            .expect("owned keyed connection")
            .close()
            .await
            .map_err(|_| KeyedError::Closed)
    }
}
impl fmt::Debug for KeyedConnection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("KeyedConnection([NATIVE])")
    }
}

pub struct KeyedReaderPool {
    pool: SqlitePool,
    state: Arc<FactoryState>,
}
impl KeyedReaderPool {
    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }
    pub async fn close(self) -> Result<(), KeyedError> {
        self.pool.close().await;
        let admission = self
            .state
            .admission
            .lock()
            .map_err(|_| KeyedError::Closed)?;
        if admission.faulted {
            return Err(KeyedError::Closed);
        }
        Ok(())
    }
}
impl fmt::Debug for KeyedReaderPool {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("KeyedReaderPool([NATIVE])")
    }
}

async fn authenticate(connection: &mut SqliteConnection) -> Result<(), KeyedError> {
    // Successful sqlite3_key alone is not authentication. Read the encrypted
    // schema before exposing a connection to its caller.
    let _: i64 = sqlx::query_scalar("SELECT count(*) FROM sqlite_master")
        .fetch_one(&mut *connection)
        .await
        .map_err(|_| KeyedError::WrongKeyOrCorrupt)?;
    let status: String = sqlx::query_scalar("PRAGMA cipher_status")
        .fetch_one(&mut *connection)
        .await
        .map_err(|_| KeyedError::Profile)?;
    if status != "1" {
        return Err(KeyedError::Profile);
    }
    for (pragma, expected) in [
        ("PRAGMA cipher_use_hmac", "1"),
        ("PRAGMA cipher_plaintext_header_size", "0"),
        ("PRAGMA cipher_page_size", "4096"),
        ("PRAGMA kdf_iter", "256000"),
        ("PRAGMA cipher_hmac_algorithm", "HMAC_SHA512"),
        ("PRAGMA cipher_kdf_algorithm", "PBKDF2_HMAC_SHA512"),
        ("SELECT sqlite_version()", "3.53.4"),
        (
            "SELECT sqlite_source_id()",
            "2026-07-24 19:02:57 bf7c7f30031888f4e796e429ab3978879485813aaca6f641c7b33e4e0945alt1",
        ),
        (
            "SELECT CAST(sqlite_compileoption_used('ENABLE_FTS5') AS TEXT)",
            "1",
        ),
        (
            "PRAGMA cipher_provider",
            if cfg!(target_vendor = "apple") {
                "commoncrypto"
            } else {
                "openssl"
            },
        ),
    ] {
        let actual: String = sqlx::query_scalar(sqlx::AssertSqlSafe(pragma))
            .fetch_one(&mut *connection)
            .await
            .map_err(|_| KeyedError::Profile)?;
        if actual != expected {
            return Err(KeyedError::Profile);
        }
    }
    let cipher: String = sqlx::query_scalar("PRAGMA cipher_version")
        .fetch_one(&mut *connection)
        .await
        .map_err(|_| KeyedError::Profile)?;
    if !cipher.starts_with("4.19.0 ") {
        return Err(KeyedError::Profile);
    }
    if !cfg!(target_vendor = "apple") {
        let provider: String = sqlx::query_scalar("PRAGMA cipher_provider_version")
            .fetch_one(&mut *connection)
            .await
            .map_err(|_| KeyedError::Profile)?;
        if !provider.starts_with("OpenSSL 3.6.5 ") {
            return Err(KeyedError::Profile);
        }
    }
    Ok(())
}

fn store_error(_: KeyedError) -> collaboration::CollaborationError {
    collaboration::CollaborationError::new(
        collaboration::ErrorCode::Storage,
        "Native keyed Store could not complete; files were preserved",
    )
}
// SAFETY: every returned handle uses the reviewed C owner hook; factory retirement
// rejects all new owners and waits for actual session-reference release.
#[async_trait::async_trait]
unsafe impl collaboration::storage::keyed::KeyedStoreFactory for KeyedConnectionFactory {
    fn path(&self) -> &std::path::Path {
        &self.state.path
    }
    fn identity(&self) -> DatabaseKeyIdentity {
        self.state.identity.clone()
    }
    fn creates_new(&self) -> bool {
        self.state.mode == DatabaseKeyMode::CreateNew
    }
    fn claim_store(&self) -> Result<(), collaboration::CollaborationError> {
        let mut state = self
            .state
            .admission
            .lock()
            .map_err(|_| store_error(KeyedError::Admission))?;
        if state.store_claimed || state.retiring || state.faulted || state.open != 0 {
            return Err(store_error(KeyedError::Admission));
        }
        state.store_claimed = true;
        Ok(())
    }
    fn stop_admission(&self) -> Result<(), collaboration::CollaborationError> {
        self.state
            .admission
            .lock()
            .map_err(|_| store_error(KeyedError::Closed))?
            .retiring = true;
        Ok(())
    }
    async fn writer(&self) -> Result<SqliteConnection, collaboration::CollaborationError> {
        Ok(self
            .open(HandleRole::Writer)
            .await
            .map_err(store_error)?
            .connection
            .take()
            .expect("owned writer"))
    }
    async fn readers(&self) -> Result<SqlitePool, collaboration::CollaborationError> {
        Ok(self.reader_pool().await.map_err(store_error)?.pool)
    }
    async fn maintenance(&self) -> Result<SqliteConnection, collaboration::CollaborationError> {
        Ok(self
            .open(HandleRole::Maintenance)
            .await
            .map_err(store_error)?
            .connection
            .take()
            .expect("owned maintenance"))
    }
    async fn verify_ready(&self) -> Result<(), collaboration::CollaborationError> {
        let session = self
            .state
            .admission
            .lock()
            .map_err(|_| store_error(KeyedError::Admission))?
            .session
            .clone()
            .ok_or_else(|| store_error(KeyedError::Closed))?;
        session
            .verify_retained(self)
            .await
            .map_err(|_| store_error(KeyedError::WrongKeyOrCorrupt))
    }
    async fn retire(&self) -> Result<(), collaboration::CollaborationError> {
        KeyedConnectionFactory::retire(self)
            .await
            .map_err(store_error)
    }
}
#[async_trait::async_trait]
impl collaboration::database_keys::RetainedDatabaseKeyVerifier for KeyedConnectionFactory {
    async fn verify(
        &self,
        session: Arc<DatabaseKeySession>,
    ) -> Result<(), collaboration::database_keys::DatabaseKeyError> {
        use collaboration::database_keys::DatabaseKeyError;
        let mut connection = self
            .open(HandleRole::Verifier)
            .await
            .map_err(|_| DatabaseKeyError::WrongKeyOrCorrupt)?;
        let result = async {
            collaboration::storage::keyed::verify_identity(
                connection.connection(),
                &session.identity(),
            )
            .await
            .map_err(|_| DatabaseKeyError::WrongKeyOrCorrupt)?;
            let integrity: String = sqlx::query_scalar("PRAGMA quick_check")
                .fetch_one(connection.connection())
                .await
                .map_err(|_| DatabaseKeyError::WrongKeyOrCorrupt)?;
            if integrity != "ok" {
                return Err(DatabaseKeyError::WrongKeyOrCorrupt);
            }
            if sqlx::query("PRAGMA foreign_key_check")
                .fetch_optional(connection.connection())
                .await
                .map_err(|_| DatabaseKeyError::WrongKeyOrCorrupt)?
                .is_some()
            {
                return Err(DatabaseKeyError::WrongKeyOrCorrupt);
            }
            Ok(())
        }
        .await;
        connection
            .close()
            .await
            .map_err(|_| DatabaseKeyError::Storage)?;
        // The native owner's session Arc persists until the actual C close. The
        // passed session also keeps the lease through the outer Ready checks.
        result
    }
}
