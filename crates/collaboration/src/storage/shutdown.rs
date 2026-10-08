//! Fallible writer admission and actual SQLite worker shutdown.
use super::*;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::sync::{MappedMutexGuard, MutexGuard};

pub(super) struct NativeWriter {
    connection: Mutex<Option<SqliteConnection>>,
    closing: AtomicBool,
}
impl NativeWriter {
    pub fn new(connection: SqliteConnection) -> Self {
        Self {
            connection: Mutex::new(Some(connection)),
            closing: AtomicBool::new(false),
        }
    }
    pub async fn acquire(&self) -> Result<MappedMutexGuard<'_, SqliteConnection>> {
        if self.closing.load(Ordering::Acquire) {
            return Err(closed());
        }
        let guard = self.connection.lock().await;
        if self.closing.load(Ordering::Acquire) {
            return Err(closed());
        }
        MutexGuard::try_map(guard, Option::as_mut).map_err(|_| closed())
    }
    pub fn try_acquire(&self) -> Result<MappedMutexGuard<'_, SqliteConnection>> {
        if self.closing.load(Ordering::Acquire) {
            return Err(closed());
        }
        let guard = self.connection.try_lock().map_err(|_| {
            CollaborationError::new(ErrorCode::Busy, "Collaboration writer is busy")
        })?;
        if self.closing.load(Ordering::Acquire) {
            return Err(closed());
        }
        MutexGuard::try_map(guard, Option::as_mut).map_err(|_| closed())
    }
}
fn closed() -> CollaborationError {
    CollaborationError::new(
        ErrorCode::NotReady,
        "Collaboration storage is closed for recovery",
    )
}
impl Store {
    /// Cancellation of a caller never releases the OS lease before the SQLx
    /// writer thread has acknowledged shutdown. Every clone shares this fence.
    pub async fn close(&self) -> Result<()> {
        self.inner.writer.closing.store(true, Ordering::Release);
        #[cfg(feature = "native-keyed-store")]
        if let Some(factory) = &self.inner.keyed {
            factory.stop_admission()?;
        }
        let owned = self.clone();
        tokio::spawn(async move { owned.close_owned().await })
            .await
            .map_err(|_| CollaborationError::storage())?
    }
    async fn close_owned(&self) -> Result<()> {
        let _closing = self.inner.shutdown.lock().await;
        self.inner.readers.close().await;
        let connection = self.inner.writer.connection.lock().await.take();
        if let Some(connection) = connection {
            // optimize_on_close is disabled, so close unconditionally awaits
            // SQLx's worker-termination acknowledgement, including checkpoint.
            connection.close().await.map_err(storage_error)?;
        }
        #[cfg(feature = "native-keyed-store")]
        if let Some(factory) = &self.inner.keyed {
            factory.retire().await?;
        }
        self.inner.writer_lease.lock().await.take();
        Ok(())
    }
}
// If final-owner cleanup cannot prove writer termination, retain the lock until
// process exit. Failing closed is preferable to reopening under a live worker.
struct FinalLease(Option<WriterLease>);
impl Drop for FinalLease {
    fn drop(&mut self) {
        if let Some(lease) = self.0.take() {
            std::mem::forget(lease);
        }
    }
}
pub(super) struct PendingWriter {
    pub connection: Option<SqliteConnection>,
    pub lease: Option<WriterLease>,
}
impl PendingWriter {
    pub async fn close(&mut self) -> Result<()> {
        if let Some(connection) = self.connection.take() {
            connection.close().await.map_err(storage_error)?;
        }
        self.lease.take();
        Ok(())
    }
}
impl Drop for PendingWriter {
    fn drop(&mut self) {
        close_after_drop(
            None,
            self.connection.take(),
            self.lease.take(),
            #[cfg(feature = "native-keyed-store")]
            None,
        );
    }
}
impl Drop for Inner {
    fn drop(&mut self) {
        close_after_drop(
            Some(self.readers.clone()),
            self.writer.connection.get_mut().take(),
            self.writer_lease.get_mut().take(),
            #[cfg(feature = "native-keyed-store")]
            self.keyed.take(),
        );
    }
}
fn close_after_drop(
    readers: Option<SqlitePool>,
    connection: Option<SqliteConnection>,
    lease: Option<WriterLease>,
    #[cfg(feature = "native-keyed-store")] keyed: Option<Arc<dyn super::keyed::KeyedStoreFactory>>,
) {
    #[cfg(feature = "native-keyed-store")]
    if let Some(factory) = &keyed {
        let _ = factory.stop_admission();
    }
    let no_keyed = true;
    #[cfg(feature = "native-keyed-store")]
    let no_keyed = no_keyed && keyed.is_none();
    if connection.is_none() && lease.is_none() && no_keyed {
        return;
    }
    // A dedicated thread also works while the originating runtime shuts down.
    // Any setup/worker failure retains the lease until this process exits.
    let mut lease = FinalLease(lease);
    let _ = std::thread::Builder::new()
        .name("collaboration-close".into())
        .spawn(move || {
            if let Ok(runtime) = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            {
                runtime.block_on(async move {
                    if let Some(readers) = readers {
                        readers.close().await;
                    }
                    let closed = match connection {
                        Some(connection) => connection.close().await.is_ok(),
                        None => true,
                    };
                    #[cfg(feature = "native-keyed-store")]
                    let closed = match keyed {
                        Some(factory) if closed => factory.retire().await.is_ok(),
                        _ => closed,
                    };
                    if closed {
                        drop(lease.0.take());
                    }
                });
            }
        });
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    async fn wait_closing(store: &Store) {
        tokio::time::timeout(Duration::from_secs(2), async {
            while !store.inner.writer.closing.load(Ordering::Acquire) {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
    }

    async fn reopen(path: &std::path::Path) -> Store {
        tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                match Store::open(path).await {
                    Ok(store) => break store,
                    Err(error) if error.code == ErrorCode::Busy => {
                        tokio::time::sleep(Duration::from_millis(5)).await;
                    }
                    Err(error) => panic!("unexpected reopen failure: {error:?}"),
                }
            }
        })
        .await
        .unwrap()
    }

    #[tokio::test]
    async fn close_drains_writer_before_unlock_and_rejects_queued_writers() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("cache.sqlite");
        let store = Store::open(&path).await.unwrap();
        let mut active = store.inner.writer.acquire().await.unwrap();
        let mut transaction = active.begin().await.unwrap();
        sqlx::query("UPDATE runtime_meta SET revision=7 WHERE singleton=1")
            .execute(&mut *transaction)
            .await
            .unwrap();
        let queued_store = store.clone();
        let queued =
            tokio::spawn(async move { queued_store.inner.writer.acquire().await.map(|_| ()) });
        tokio::task::yield_now().await;
        let closing_store = store.clone();
        let closing = tokio::spawn(async move { closing_store.close().await });
        wait_closing(&store).await;
        assert_eq!(
            Store::open(&path).await.err().unwrap().code,
            ErrorCode::Busy
        );
        assert!(!closing.is_finished());
        transaction.commit().await.unwrap();
        drop(active);
        assert_eq!(queued.await.unwrap().unwrap_err().code, ErrorCode::NotReady);
        closing.await.unwrap().unwrap();
        assert!(store.revision().await.is_err());
        assert_eq!(
            store.wal_status().await.unwrap_err().code,
            ErrorCode::NotReady
        );
        assert_eq!(
            store.checkpoint_wal_passive().await.unwrap_err().code,
            ErrorCode::NotReady
        );
        assert_eq!(
            store.inner.writer.acquire().await.err().unwrap().code,
            ErrorCode::NotReady
        );
        let reopened = Store::open(&path).await.unwrap();
        assert_eq!(reopened.revision().await.unwrap(), "7");
        reopened.close().await.unwrap();
    }

    #[tokio::test]
    async fn cancelled_close_still_drains_readers_and_releases_writer() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("cache.sqlite");
        let store = Store::open(&path).await.unwrap();
        let reader = store.inner.readers.begin().await.unwrap();
        let closing_store = store.clone();
        let requester = tokio::spawn(async move { closing_store.close().await });
        wait_closing(&store).await;
        requester.abort();
        let _ = requester.await;
        assert_eq!(
            Store::open(&path).await.err().unwrap().code,
            ErrorCode::Busy
        );
        assert_eq!(
            store.inner.writer.acquire().await.err().unwrap().code,
            ErrorCode::NotReady
        );
        reader.commit().await.unwrap();
        let reopened = reopen(&path).await;
        assert!(store.revision().await.is_err());
        reopened.close().await.unwrap();
    }

    #[tokio::test]
    async fn migration_failure_closes_startup_writer_before_returning() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("cache.sqlite");
        let store = Store::open(&path).await.unwrap();
        store.close().await.unwrap();
        let mut fixture =
            SqliteConnection::connect_with(&SqliteConnectOptions::new().filename(&path))
                .await
                .unwrap();
        sqlx::query("UPDATE _sqlx_migrations SET checksum=x'00' WHERE version=1")
            .execute(&mut fixture)
            .await
            .unwrap();
        fixture.close().await.unwrap();
        assert_eq!(
            Store::open(&path).await.err().unwrap().code,
            ErrorCode::Storage
        );
        // The returned failure owns no lingering writer, so recovery can take
        // the exact existing lease inode without a race against final Drop.
        let recovery_lease = acquire_writer_lease(&path).unwrap();
        assert_eq!(
            Store::open(&path).await.err().unwrap().code,
            ErrorCode::Busy
        );
        drop(recovery_lease);
        assert_eq!(
            Store::open(&path).await.err().unwrap().code,
            ErrorCode::Storage
        );
    }

    #[tokio::test]
    async fn final_drop_retains_lease_until_outstanding_reader_finishes() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("cache.sqlite");
        let store = Store::open(&path).await.unwrap();
        let reader = store.inner.readers.begin().await.unwrap();
        drop(store);
        assert_eq!(
            Store::open(&path).await.err().unwrap().code,
            ErrorCode::Busy
        );
        reader.commit().await.unwrap();
        let reopened = reopen(&path).await;
        reopened.close().await.unwrap();
    }
}
