//! Explicit native Store connection seam. No application setup selects it.
use super::*;
use crate::database_keys::DatabaseKeyIdentity;

pub const CIPHER_PROFILE: &str = "sqlcipher-v4-p4096-k256000-hmacsha512";

/// A trusted native factory owns the key and the exclusive database lease.
///
/// # Safety
/// Every returned connection must be keyed before SQLx initialization and must
/// retain the same key/lease until actual C-handle destruction, including failed
/// initialization and cancellation. `retire` must refuse admission permanently
/// and acknowledge final owner release before returning success. Failure retains
/// the lease. The exact cipher/source profile and read-only roles are mandatory.
#[async_trait::async_trait]
pub unsafe trait KeyedStoreFactory: Send + Sync {
    fn path(&self) -> &Path;
    fn identity(&self) -> DatabaseKeyIdentity;
    fn creates_new(&self) -> bool;
    fn claim_store(&self) -> Result<()>;
    fn stop_admission(&self) -> Result<()>;
    async fn writer(&self) -> Result<SqliteConnection>;
    async fn readers(&self) -> Result<SqlitePool>;
    async fn maintenance(&self) -> Result<SqliteConnection>;
    /// Authenticated read-only schema/identity verification on a retained owner,
    /// followed by the fenced key-journal Ready publication.
    async fn verify_ready(&self) -> Result<()>;
    async fn retire(&self) -> Result<()>;
}

pub(super) fn keyed_recovery_required() -> CollaborationError {
    CollaborationError::new(
        ErrorCode::NotReady,
        "Keyed storage requires explicit key-aware recovery; files were preserved",
    )
}

pub async fn verify_identity(
    connection: &mut SqliteConnection,
    identity: &DatabaseKeyIdentity,
) -> Result<()> {
    let row: (i64, String, i64, String) = sqlx::query_as("SELECT format_version,database_id,key_generation,cipher_profile FROM database_storage_identity WHERE singleton=1")
        .fetch_one(&mut *connection).await.map_err(|_| keyed_recovery_required())?;
    if row.0 != 1
        || row.1 != identity.database_id()
        || u64::try_from(row.2).ok() != Some(identity.generation())
        || row.3 != CIPHER_PROFILE
    {
        return Err(keyed_recovery_required());
    }
    Ok(())
}

/// Validate the native import after its destination identity has been rebound.
/// The ciphertext digest differs by design, so compare semantic durable counts
/// and the reviewed current schema while the authenticated keyed handle is live.
pub async fn verify_portable_import(
    connection: &mut SqliteConnection,
    identity: &DatabaseKeyIdentity,
    expected: &crate::recovery::BackupSummary,
) -> Result<()> {
    verify_identity(connection, identity).await?;
    let version = crate::recovery::verify_keyed(connection).await?;
    let actual = crate::recovery::summary(connection, version, String::new()).await?;
    if actual.schema_version != expected.schema_version
        || actual.revision != expected.revision
        || actual.accounts != expected.accounts
        || actual.drafts != expected.drafts
        || actual.commands != expected.commands
    {
        return Err(keyed_recovery_required());
    }
    Ok(())
}

impl Store {
    /// Qualification/native embedding only. No default cipher selection changes.
    pub async fn open_keyed(factory: Arc<dyn KeyedStoreFactory>) -> Result<Self> {
        tokio::spawn(async move {
            factory.claim_store()?;
            match Self::open_keyed_owned(factory.clone()).await {
                Ok(store) => Ok(store),
                Err(error) => {
                    factory.retire().await?;
                    Err(error)
                }
            }
        })
        .await
        .map_err(|_| CollaborationError::storage())?
    }
    async fn open_keyed_owned(factory: Arc<dyn KeyedStoreFactory>) -> Result<Self> {
        let path = factory.path().to_path_buf();
        crate::recovery::require_no_pending_restore(&path)?;
        let mut pending = shutdown::PendingWriter {
            connection: None,
            lease: None,
        };
        pending.connection = Some(factory.writer().await?);
        let initialized = async {
            let writer = pending.connection.as_mut().expect("new keyed writer");
            if factory.creates_new() {
                let count: i64 = sqlx::query_scalar("SELECT count(*) FROM sqlite_schema")
                    .fetch_one(&mut *writer)
                    .await
                    .map_err(storage_error)?;
                if count != 0 {
                    return Err(keyed_recovery_required());
                }
            } else {
                // Refuse an unrelated/missing identity before migrations modify it.
                verify_identity(writer, &factory.identity()).await?;
            }
            let version = initialize_writer(writer, &path).await?;
            if factory.creates_new() {
                let identity = factory.identity();
                sqlx::query("INSERT INTO database_storage_identity VALUES(1,1,?,?,?)")
                    .bind(identity.database_id())
                    .bind(
                        i64::try_from(identity.generation())
                            .map_err(|_| keyed_recovery_required())?,
                    )
                    .bind(CIPHER_PROFILE)
                    .execute(&mut *writer)
                    .await
                    .map_err(storage_error)?;
            }
            verify_identity(writer, &factory.identity()).await?;
            crate::recovery::verify_keyed(writer).await?;
            factory.verify_ready().await?;
            let readers = factory.readers().await?;
            Ok::<_, CollaborationError>((readers, version))
        }
        .await;
        let (readers, version) = match initialized {
            Ok(value) => value,
            Err(error) => {
                pending.close().await?;
                return Err(error);
            }
        };
        Ok(Self {
            inner: Arc::new(Inner {
                writer: NativeWriter::new(
                    pending.connection.take().expect("initialized keyed writer"),
                ),
                readers,
                path,
                maintenance: Mutex::new(()),
                noop_wal_checkpoint_supported: sqlite_version_at_least(&version, (3, 51, 0)),
                writer_lease: Mutex::new(None),
                shutdown: Mutex::new(()),
                keyed: Some(factory),
            }),
        })
    }
}
