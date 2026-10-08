//! Opt-in native connection qualification. Never selected by application setup.
//!
//! The SQLx adaptation retains the initializer's capture on the actual C handle,
//! including failed startup and cancelled futures. This factory does not publish
//! Ready, migrate application schemas, choose a platform vault, or convert data.
use collaboration::database_keys::{DatabaseKeyMode, DatabaseKeySession};
use sqlx::{
    ConnectOptions, Connection, SqliteConnection, SqlitePool,
    sqlite::{
        SqliteConnectOptions, SqliteJournalMode, SqliteNativeHandleOwner, SqlitePoolOptions,
        SqliteSynchronous,
    },
};
use std::{
    fmt,
    sync::{Arc, Mutex},
    time::Duration,
};

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
}
struct FactoryState {
    session: DatabaseKeySession,
    admission: Mutex<Admission>,
}
struct HandleOwner {
    state: Arc<FactoryState>,
}
impl fmt::Debug for HandleOwner {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("HandleOwner([REDACTED])")
    }
}
impl HandleOwner {
    fn reserve(state: Arc<FactoryState>) -> Result<Self, sqlx::Error> {
        {
            let mut admission = state
                .admission
                .lock()
                .map_err(|_| sqlx::Error::Protocol("Native admission unavailable".into()))?;
            if admission.faulted || admission.open >= MAX_NATIVE_HANDLES {
                return Err(sqlx::Error::Protocol("Native admission refused".into()));
            }
            admission.open += 1;
        }
        Ok(Self { state })
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
        let key = self.state.session.key().expose();
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
        // Driver drops this only after successful close or before native open.
        if let Ok(mut admission) = self.state.admission.lock() {
            admission.open -= 1;
        }
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
                session,
                admission: Mutex::new(Admission::default()),
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
                && self.state.session.mode() == DatabaseKeyMode::CreateNew
                && !admission.creation_consumed;
            if allowed {
                admission.creation_consumed = true;
            }
            allowed
        };
        let state = self.state.clone();
        let session = &state.session;
        let options = SqliteConnectOptions::new()
            .filename(session.path())
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
        if admission.faulted || admission.open >= MAX_NATIVE_HANDLES {
            return Err(KeyedError::Admission);
        }
        Ok(())
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
