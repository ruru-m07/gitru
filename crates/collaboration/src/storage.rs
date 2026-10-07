//! The durable, account-scoped read model. All writes share one owner; reads
//! never touch a provider and use short SQLite snapshots.
use std::{
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

use serde::{Deserialize, Serialize, de::DeserializeOwned};
use sha2::{Digest, Sha256};
use sqlx::{
    Connection, Row, Sqlite, SqliteConnection, SqlitePool, Transaction,
    sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous},
};
use tokio::sync::Mutex;
use uuid::Uuid;

pub(crate) mod command_admission;
mod contextual_capabilities;
pub(crate) mod delivery;
pub(crate) mod effective;
mod shutdown;
use shutdown::NativeWriter;
pub(crate) mod details;
pub(crate) mod diagnostics;
pub(crate) mod facet_reconciliation;
mod identities;
mod inbox;
mod local_links;
pub(crate) mod notification_subjects;
mod pull_commits;
mod pull_files;
pub use pull_files::{PullFileApplyReceipt, PullFileCommit, PullFileSelection};
mod resource_metadata;
pub mod retention;
#[cfg(all(test, unix))]
mod writer_lease_tests;

use crate::{
    domain::*,
    error::{CollaborationError, ErrorCode},
};

type Result<T> = std::result::Result<T, CollaborationError>;
const MAX_ITEMS: u32 = 100;
const MAX_CHANGE_PAGE: i64 = 256;
const CHANGE_LOG_LIMIT: i64 = 4096;
const MAX_BODY_BYTES: usize = 1_048_576;
const MAX_FTS_BODY_CHARS: usize = 16_384;
pub(crate) static MIGRATIONS: sqlx::migrate::Migrator = sqlx::migrate!("./migrations");
// Discovery owns picker membership. Notification-only references can appear
// independently, but cannot resurrect a denied or retired discovery member.
const VISIBLE_REPOSITORY: &str = "((EXISTS(SELECT 1 FROM scope_membership m WHERE m.account_id=r.account_id AND m.scope='repositories' AND m.entity_id=r.id AND m.active=1) AND NOT EXISTS(SELECT 1 FROM sync_scopes s WHERE s.account_id=r.account_id AND s.scope='repositories' AND s.access_denied=1)) OR (NOT EXISTS(SELECT 1 FROM scope_membership m WHERE m.account_id=r.account_id AND m.scope='repositories' AND m.entity_id=r.id) AND EXISTS(SELECT 1 FROM items n JOIN scope_membership m ON m.account_id=n.account_id AND m.scope='notifications' AND m.entity_id=n.id AND m.active=1 WHERE n.account_id=r.account_id AND n.repository_id=r.id AND n.kind='notification') AND NOT EXISTS(SELECT 1 FROM sync_scopes s WHERE s.account_id=r.account_id AND s.scope='notifications' AND s.access_denied=1)))";

#[derive(Clone)]
pub struct Store {
    inner: Arc<Inner>,
}

struct Inner {
    writer: NativeWriter,
    readers: SqlitePool,
    path: PathBuf,
    maintenance: Mutex<()>,
    noop_wal_checkpoint_supported: bool,
    // Drop connection handles before releasing the final writer owner's
    // lease. Keeping the file avoids unlink/recreate races between instances.
    writer_lease: Mutex<Option<WriterLease>>,
    shutdown: Mutex<()>,
}

pub(crate) struct WriterLease {
    file: std::fs::File,
}

impl Drop for WriterLease {
    fn drop(&mut self) {
        // Closing only our descriptor can leave a Unix flock held by a
        // concurrent fork until it executes. Explicitly release the lock
        // after actual writer closure, including explicit close on any clone.
        // On failure, File drop still closes our descriptor; the OS also
        // releases the lease on process exit, including crashes.
        let _ = self.file.unlock();
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct Cursor {
    authorization_view: String,
    projection_view: String,
    query: String,
    updated_at: String,
    id: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct DraftCursor {
    version: u32,
    account_id: String,
    subject_id: String,
}

impl Store {
    #[cfg(feature = "test-harness")]
    pub async fn harness_item_count(&self) -> Result<u32> {
        let count: i64 = sqlx::query_scalar("SELECT count(*) FROM items")
            .fetch_one(&self.inner.readers)
            .await
            .map_err(storage_error)?;
        u32::try_from(count).map_err(|_| CollaborationError::invalid("Fixture item count overflow"))
    }

    pub async fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        tokio::spawn(async move { Self::open_owned(&path).await })
            .await
            .map_err(|_| CollaborationError::storage())?
    }
    async fn open_owned(path: &Path) -> Result<Self> {
        prepare_private_path(path)?;
        let writer_lease = acquire_writer_lease(path)?;
        crate::recovery::require_no_pending_restore(path)?;
        let base = SqliteConnectOptions::new()
            .filename(path)
            .foreign_keys(true)
            .synchronous(SqliteSynchronous::Full)
            .busy_timeout(Duration::from_secs(2))
            .optimize_on_close(false, None)
            .statement_cache_capacity(64)
            .row_buffer_size(128)
            .command_buffer_size(32);
        let mut pending = shutdown::PendingWriter {
            connection: None,
            lease: Some(writer_lease),
        };
        let writer = SqliteConnection::connect_with(
            &base
                .clone()
                .create_if_missing(true)
                .journal_mode(SqliteJournalMode::Wal),
        )
        .await
        .map_err(storage_error)?;
        pending.connection = Some(writer);
        let initialized = async {
            let writer = pending.connection.as_mut().expect("new SQLite writer");
            secure_database_files(path)?;
            let version: String = sqlx::query_scalar("SELECT sqlite_version()")
                .fetch_one(&mut *writer)
                .await
                .map_err(storage_error)?;
            if !fixed_sqlite_version(&version) {
                return Err(CollaborationError::new(
                    ErrorCode::Storage,
                    "Collaboration requires SQLite with the WAL-reset fix",
                ));
            }
            let fts: i64 = sqlx::query_scalar("SELECT sqlite_compileoption_used('ENABLE_FTS5')")
                .fetch_one(&mut *writer)
                .await
                .map_err(storage_error)?;
            if fts != 1 {
                return Err(CollaborationError::new(
                    ErrorCode::Storage,
                    "Collaboration requires SQLite FTS5",
                ));
            }
            MIGRATIONS
            .run_direct(None, &mut *writer, false)
            .await
            .map_err(|_| {
                CollaborationError::new(
                    ErrorCode::Storage,
                    "Collaboration database migration failed; the existing database was preserved",
                )
            })?;
            pull_commits::cleanup_abandoned_in(&mut *writer).await?;
            pull_files::cleanup_abandoned_in(&mut *writer).await?;
            secure_database_files(path)?;
            let readers = SqlitePoolOptions::new()
                .max_connections(3)
                .min_connections(1)
                .acquire_timeout(Duration::from_secs(2))
                .connect_with(base.read_only(true).pragma("query_only", "ON"))
                .await
                .map_err(storage_error)?;
            Ok::<_, CollaborationError>((readers, version))
        }
        .await;
        let (readers, version) = match initialized {
            Ok(initialized) => initialized,
            Err(error) => {
                pending.close().await?;
                return Err(error);
            }
        };
        Ok(Self {
            inner: Arc::new(Inner {
                writer: NativeWriter::new(
                    pending
                        .connection
                        .take()
                        .expect("initialized SQLite writer"),
                ),
                readers,
                path: path.to_path_buf(),
                maintenance: Mutex::new(()),
                noop_wal_checkpoint_supported: sqlite_version_at_least(&version, (3, 51, 0)),
                writer_lease: Mutex::new(pending.lease.take()),
                shutdown: Mutex::new(()),
            }),
        })
    }

    /// A verified, WAL-consistent export. Credentials and their vault references
    /// are removed from private staging before publication. Never overwrites.
    pub async fn backup_to(
        &self,
        path: impl AsRef<Path>,
    ) -> Result<crate::recovery::BackupSummary> {
        let mut writer = self.inner.writer.acquire().await?;
        crate::recovery::backup_from(&mut writer, path.as_ref()).await
    }

    pub async fn revision(&self) -> Result<String> {
        let value: i64 = sqlx::query_scalar("SELECT revision FROM runtime_meta WHERE singleton=1")
            .fetch_one(&self.inner.readers)
            .await
            .map_err(storage_error)?;
        Ok(value.to_string())
    }

    pub async fn accounts(&self) -> Result<AccountSnapshot> {
        let mut tx = self.inner.readers.begin().await.map_err(storage_error)?;
        let (revision, authorization_view) = metadata(&mut tx).await?;
        let rows = sqlx::query("SELECT json FROM accounts ORDER BY id LIMIT 101")
            .fetch_all(&mut *tx)
            .await
            .map_err(storage_error)?;
        if rows.len() > 100 {
            return Err(CollaborationError::new(
                ErrorCode::Storage,
                "Too many connected accounts",
            ));
        }
        let accounts = rows
            .iter()
            .map(|r| decode(r.get::<&str, _>("json")))
            .collect::<Result<Vec<_>>>()?;
        tx.commit().await.map_err(storage_error)?;
        Ok(AccountSnapshot {
            accounts,
            revision,
            authorization_view,
        })
    }

    /// Metadata remains available after disconnection so reconnect and draft
    /// recovery do not depend on a provider request.
    pub async fn account(&self, id: &str) -> Result<RemoteAccount> {
        let json: Option<String> = sqlx::query_scalar("SELECT json FROM accounts WHERE id=?")
            .bind(id)
            .fetch_optional(&self.inner.readers)
            .await
            .map_err(storage_error)?;
        decode(&json.ok_or_else(not_found)?)
    }

    /// Account metadata alone is useful for local fixtures. Native authentication
    /// must use commit_account_credential so epoch and reference move together.
    pub async fn upsert_account(&self, account: RemoteAccount) -> Result<RemoteAccount> {
        let mut writer = self.inner.writer.acquire().await?;
        let mut tx = writer.begin().await.map_err(storage_error)?;
        upsert_account_in(&mut tx, &account).await?;
        tx.commit().await.map_err(storage_error)?;
        Ok(account)
    }

    /// Journal the fresh reference before any vault side effect. Failed/crashed
    /// staging never changes the committed account authorization or credential.
    pub async fn stage_credential(&self, account_id: &str, reference: &str) -> Result<()> {
        validate_identifier(account_id)?;
        validate_identifier(reference)?;
        let mut writer = self.inner.writer.acquire().await?;
        let mut tx = writer.begin().await.map_err(storage_error)?;
        let pending: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM credential_cleanup")
            .fetch_one(&mut *tx)
            .await
            .map_err(storage_error)?;
        if pending >= 128 {
            return Err(CollaborationError::new(
                ErrorCode::Busy,
                "Credential cleanup is pending; unlock the credential store before reconnecting",
            ));
        }
        let committed: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM account_credentials WHERE credential_ref=?)",
        )
        .bind(reference)
        .fetch_one(&mut *tx)
        .await
        .map_err(storage_error)?;
        if committed {
            return Err(CollaborationError::invalid(
                "Credential staging requires a fresh reference",
            ));
        }
        sqlx::query(
            "INSERT INTO credential_cleanup(credential_ref,account_id,state) VALUES(?,?,'staged')",
        )
        .bind(reference)
        .bind(account_id)
        .execute(&mut *tx)
        .await
        .map_err(storage_error)?;
        #[cfg(test)]
        crate::runtime::credential_crash_tests::checkpoint("during_stage");
        tx.commit().await.map_err(storage_error)?;
        Ok(())
    }

    /// Promote only a journaled, verified replacement. The old reference is
    /// retired atomically with the authorization epoch and account cache reset.
    pub async fn commit_account_credential(
        &self,
        account: RemoteAccount,
        reference: &str,
    ) -> Result<RemoteAccount> {
        self.commit_account_credential_with_quota(account, reference, None)
            .await
    }

    /// A verified operation may consume the actor's quota before promotion.
    /// Commit its deadline with the grant, so a crash cannot discard it.
    pub(crate) async fn commit_account_credential_with_quota(
        &self,
        account: RemoteAccount,
        reference: &str,
        quota_deadline: Option<String>,
    ) -> Result<RemoteAccount> {
        if let Some(deadline) = &quota_deadline {
            validate_provider_deadline(deadline)?;
        }
        let mut writer = self.inner.writer.acquire().await?;
        let mut tx = writer.begin().await.map_err(storage_error)?;
        let staged: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM credential_cleanup WHERE credential_ref=? AND account_id=? AND state='staged')")
            .bind(reference).bind(&account.id).fetch_one(&mut *tx).await.map_err(storage_error)?;
        if !staged || account.state != AccountState::Active {
            return Err(CollaborationError::invalid(
                "Credential cutover requires a staged reference and active account",
            ));
        }
        let previous: Option<String> =
            sqlx::query_scalar("SELECT credential_ref FROM account_credentials WHERE account_id=?")
                .bind(&account.id)
                .fetch_optional(&mut *tx)
                .await
                .map_err(storage_error)?;
        upsert_account_in(&mut tx, &account).await?;
        sqlx::query("INSERT INTO account_credentials(account_id,credential_ref) VALUES(?,?) ON CONFLICT(account_id) DO UPDATE SET credential_ref=excluded.credential_ref")
            .bind(&account.id).bind(reference).execute(&mut *tx).await.map_err(storage_error)?;
        sqlx::query("DELETE FROM credential_cleanup WHERE credential_ref=?")
            .bind(reference)
            .execute(&mut *tx)
            .await
            .map_err(storage_error)?;
        if let Some(previous) = previous {
            retire_credential_in(&mut tx, &account.id, &previous).await?;
        }
        if let Some(proposed) = quota_deadline {
            provider_budget_in(
                &mut tx,
                &account.id,
                &account.authorization_epoch,
                proposed,
                None,
            )
            .await?;
        }
        #[cfg(test)]
        crate::runtime::credential_crash_tests::checkpoint("during_cutover");
        tx.commit().await.map_err(storage_error)?;
        Ok(account)
    }

    /// Budget consumption is independent of an actor's current authorization.
    /// Merge it atomically, without admitting reads or changing private data.
    pub(crate) async fn merge_provider_budget(
        &self,
        account_id: &str,
        epoch: &str,
        proposed: String,
        error: Option<CollaborationError>,
    ) -> Result<String> {
        validate_provider_deadline(&proposed)?;
        let mut writer = self.inner.writer.acquire().await?;
        let mut tx = writer.begin().await.map_err(storage_error)?;
        let account = account_in(&mut tx, account_id, false).await?;
        if account.authorization_epoch != epoch {
            return Err(stale());
        }
        let revision = provider_budget_in(&mut tx, account_id, epoch, proposed, error).await?;
        tx.commit().await.map_err(storage_error)?;
        Ok(revision)
    }

    /// Native-only metadata. Never add this reference to an IPC/domain DTO.
    pub async fn credential_reference(&self, account_id: &str) -> Result<Option<String>> {
        sqlx::query_scalar("SELECT credential_ref FROM account_credentials WHERE account_id=?")
            .bind(account_id)
            .fetch_optional(&self.inner.readers)
            .await
            .map_err(storage_error)
    }

    pub async fn due_credential_cleanup(
        &self,
        now: i64,
        limit: u32,
    ) -> Result<Vec<crate::credentials::CredentialCleanup>> {
        if !(1..=32).contains(&limit) {
            return Err(CollaborationError::invalid(
                "Invalid credential cleanup batch size",
            ));
        }
        let rows = sqlx::query("SELECT credential_ref,attempts FROM credential_cleanup c WHERE next_retry_at<=? AND NOT EXISTS(SELECT 1 FROM account_credentials a WHERE a.credential_ref=c.credential_ref) ORDER BY next_retry_at,credential_ref LIMIT ?")
            .bind(now).bind(limit).fetch_all(&self.inner.readers).await.map_err(storage_error)?;
        Ok(rows
            .into_iter()
            .map(|row| crate::credentials::CredentialCleanup {
                reference: row.get("credential_ref"),
                attempts: row.get::<i64, _>("attempts") as u32,
            })
            .collect())
    }

    pub async fn finish_credential_cleanup(&self, reference: &str) -> Result<()> {
        let mut writer = self.inner.writer.acquire().await?;
        sqlx::query("DELETE FROM credential_cleanup WHERE credential_ref=? AND NOT EXISTS(SELECT 1 FROM account_credentials WHERE credential_ref=?)")
            .bind(reference).bind(reference).execute(&mut *writer).await.map_err(storage_error)?;
        Ok(())
    }

    pub async fn credential_cleanup(
        &self,
        reference: &str,
    ) -> Result<Option<crate::credentials::CredentialCleanup>> {
        let row = sqlx::query("SELECT credential_ref,attempts FROM credential_cleanup c WHERE credential_ref=? AND NOT EXISTS(SELECT 1 FROM account_credentials a WHERE a.credential_ref=c.credential_ref)")
            .bind(reference).fetch_optional(&self.inner.readers).await.map_err(storage_error)?;
        Ok(row.map(|row| crate::credentials::CredentialCleanup {
            reference: row.get("credential_ref"),
            attempts: row.get::<i64, _>("attempts") as u32,
        }))
    }

    pub async fn defer_credential_cleanup(
        &self,
        reference: &str,
        next_retry_at: i64,
    ) -> Result<()> {
        let mut writer = self.inner.writer.acquire().await?;
        sqlx::query("UPDATE credential_cleanup SET attempts=min(attempts+1,20),next_retry_at=? WHERE credential_ref=?")
            .bind(next_retry_at).bind(reference).execute(&mut *writer).await.map_err(storage_error)?;
        Ok(())
    }

    pub async fn disconnect(&self, account_id: &str) -> Result<String> {
        let mut writer = self.inner.writer.acquire().await?;
        let mut tx = writer.begin().await.map_err(storage_error)?;
        let mut account = account_in(&mut tx, account_id, false).await?;
        let epoch = positive_revision(&account.authorization_epoch)?
            .checked_add(1)
            .ok_or_else(CollaborationError::storage)?;
        account.authorization_epoch = epoch.to_string();
        account.state = AccountState::Disconnected;
        let reference: Option<String> = sqlx::query_scalar(
            "DELETE FROM account_credentials WHERE account_id=? RETURNING credential_ref",
        )
        .bind(account_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(storage_error)?;
        if let Some(reference) = reference {
            retire_credential_in(&mut tx, account_id, &reference).await?;
        }
        clear_remote_cache(&mut tx, account_id).await?;
        sqlx::query(
            "UPDATE accounts SET authorization_epoch=?,state='disconnected',json=? WHERE id=?",
        )
        .bind(epoch)
        .bind(encode(&account)?)
        .bind(account_id)
        .execute(&mut *tx)
        .await
        .map_err(storage_error)?;
        sqlx::query(
            "UPDATE runtime_meta SET authorization_view=authorization_view+1 WHERE singleton=1",
        )
        .execute(&mut *tx)
        .await
        .map_err(storage_error)?;
        let revision = record_change(&mut tx, account_id, epoch, "account", true).await?;
        #[cfg(test)]
        crate::runtime::credential_crash_tests::checkpoint("during_disconnect");
        tx.commit().await.map_err(storage_error)?;
        Ok(revision)
    }

    pub async fn repositories(&self, account_id: &str) -> Result<RepositorySnapshot> {
        let mut tx = self.inner.readers.begin().await.map_err(storage_error)?;
        account_in(&mut tx, account_id, true).await?;
        let (revision, authorization_view) = metadata(&mut tx).await?;
        // Repository discovery is deliberately capped in the initial slice.
        let mut sql = sqlx::QueryBuilder::<Sqlite>::new(
            "SELECT r.json,r.selected FROM repositories r WHERE r.account_id=",
        );
        sql.push_bind(account_id)
            .push(" AND ")
            .push(VISIBLE_REPOSITORY)
            .push(" ORDER BY r.full_name,r.id LIMIT 2001");
        let rows = sql
            .build()
            .fetch_all(&mut *tx)
            .await
            .map_err(storage_error)?;
        if rows.len() > 2000 {
            return Err(CollaborationError::new(
                ErrorCode::Storage,
                "Repository list exceeds the local discovery limit",
            ));
        }
        let repositories = rows
            .iter()
            .map(repository_from_row)
            .collect::<Result<Vec<_>>>()?;
        let stored = scope_in(&mut tx, account_id, "repositories").await?;
        let (coverage, sync) = presentation(stored);
        tx.commit().await.map_err(storage_error)?;
        Ok(RepositorySnapshot {
            repositories,
            revision,
            authorization_view,
            coverage,
            sync,
        })
    }

    pub async fn repository(
        &self,
        account_id: &str,
        repository_id: &str,
    ) -> Result<RemoteRepository> {
        let mut tx = self.inner.readers.begin().await.map_err(storage_error)?;
        account_in(&mut tx, account_id, true).await?;
        let mut sql = sqlx::QueryBuilder::<Sqlite>::new(
            "SELECT r.json,r.selected FROM repositories r WHERE r.account_id=",
        );
        sql.push_bind(account_id)
            .push(" AND r.id=")
            .push_bind(repository_id)
            .push(" AND ")
            .push(VISIBLE_REPOSITORY);
        let row = sql
            .build()
            .fetch_optional(&mut *tx)
            .await
            .map_err(storage_error)?
            .ok_or_else(not_found)?;
        let repository = repository_from_row(&row)?;
        tx.commit().await.map_err(storage_error)?;
        Ok(repository)
    }
    pub(crate) async fn demand_repositories(
        &self,
        account_id: &str,
        after_id: Option<&str>,
    ) -> Result<Vec<RemoteRepository>> {
        let mut tx = self.inner.readers.begin().await.map_err(storage_error)?;
        account_in(&mut tx, account_id, true).await?;
        let mut result = vec![];
        for after in [after_id, None] {
            let mut sql = sqlx::QueryBuilder::<Sqlite>::new(
                "SELECT r.json,r.selected FROM repositories r WHERE r.account_id=",
            );
            sql.push_bind(account_id)
                .push(" AND r.selected=1 AND ")
                .push(VISIBLE_REPOSITORY);
            if let Some(id) = after {
                sql.push(" AND r.id>").push_bind(id);
            }
            sql.push(" ORDER BY r.id LIMIT 16");
            let rows = sql
                .build()
                .fetch_all(&mut *tx)
                .await
                .map_err(storage_error)?;
            result = rows
                .iter()
                .map(repository_from_row)
                .collect::<Result<Vec<_>>>()?;
            if !result.is_empty() || after.is_none() {
                break;
            }
        }
        tx.commit().await.map_err(storage_error)?;
        Ok(result)
    }

    pub async fn select_repository(
        &self,
        account_id: &str,
        repository_id: &str,
        selected: bool,
    ) -> Result<String> {
        let mut writer = self.inner.writer.acquire().await?;
        let mut tx = writer.begin().await.map_err(storage_error)?;
        let account = account_in(&mut tx, account_id, true).await?;
        let result = sqlx::query("UPDATE repositories SET selected=? WHERE account_id=? AND id=?")
            .bind(selected)
            .bind(account_id)
            .bind(repository_id)
            .execute(&mut *tx)
            .await
            .map_err(storage_error)?;
        if result.rows_affected() == 0 {
            return Err(not_found());
        }
        if !selected {
            // Invalidates responses already in flight even when credentials did
            // not change. Cached observations remain available for re-selection.
            for kind in [RemoteItemKind::PullRequest, RemoteItemKind::Issue] {
                sqlx::query("UPDATE sync_scopes SET run_id=? WHERE account_id=? AND scope=?")
                    .bind(Uuid::new_v4().to_string())
                    .bind(account_id)
                    .bind(repository_scope(repository_id, &kind))
                    .execute(&mut *tx)
                    .await
                    .map_err(storage_error)?;
            }
            // Reselecting the parent cannot make a pre-deselection detail lease
            // current again. Retain saved observations, but restart any partial
            // traversal rather than reusing its invalidated membership run.
            sqlx::QueryBuilder::<Sqlite>::new(format!("UPDATE sync_scopes SET run_id=?,next_cursor=NULL,etag=NULL WHERE account_id=? AND EXISTS(SELECT 1 FROM items i WHERE i.account_id=sync_scopes.account_id AND i.repository_id=? AND sync_scopes.scope IN ({}))", details::scope_sql_list("i.id"))).build()
                .bind(Uuid::new_v4().to_string()).bind(account_id).bind(repository_id)
                .execute(&mut *tx).await.map_err(storage_error)?;
        }
        let revision = record_change(
            &mut tx,
            account_id,
            positive_revision(&account.authorization_epoch)?,
            "repositories",
            false,
        )
        .await?;
        tx.commit().await.map_err(storage_error)?;
        Ok(revision)
    }

    pub async fn query_items(&self, query: ItemQuery) -> Result<ItemPage> {
        if query.limit == 0 || query.limit > MAX_ITEMS {
            return Err(CollaborationError::invalid(
                "Item limit must be between 1 and 100",
            ));
        }
        if query.search.as_ref().is_some_and(|s| s.len() > 256) {
            return Err(CollaborationError::invalid("Search text is too long"));
        }
        let mut tx = self.inner.readers.begin().await.map_err(storage_error)?;
        account_in(&mut tx, &query.account_id, true).await?;
        let (revision, authorization_view) = metadata(&mut tx).await?;
        let projection_view = query_projection_view(&mut tx, &query).await?;
        let mut fingerprint = query.clone();
        fingerprint.cursor = None;
        let fingerprint = encode(&fingerprint)?;
        let cursor: Option<Cursor> = query
            .cursor
            .as_ref()
            .map(|s| {
                if s.len() > 4096 {
                    return Err(CollaborationError::invalid("Invalid local item cursor"));
                }
                serde_json::from_str(s)
                    .map_err(|_| CollaborationError::invalid("Invalid local item cursor"))
            })
            .transpose()?;
        if let Some(cursor) = &cursor {
            if cursor.query != fingerprint {
                return Err(CollaborationError::invalid(
                    "Cursor belongs to a different query",
                ));
            }
            if cursor.authorization_view != authorization_view
                || cursor.projection_view != projection_view
            {
                return Err(stale());
            }
        }
        let mut sql = sqlx::QueryBuilder::<Sqlite>::new(
            "SELECT items.json FROM effective_items AS items WHERE items.account_id=",
        );
        push_item_predicates(&mut sql, &query)?;
        let mut count = sqlx::QueryBuilder::<Sqlite>::new(
            "SELECT count(*) FROM effective_items AS items WHERE items.account_id=",
        );
        push_item_predicates(&mut count, &query)?;
        let total_count: i64 = count
            .build_query_scalar()
            .fetch_one(&mut *tx)
            .await
            .map_err(storage_error)?;
        if let Some(cursor) = cursor {
            sql.push(" AND (items.updated_at,items.id)<(")
                .push_bind(cursor.updated_at)
                .push(",")
                .push_bind(cursor.id)
                .push(")");
        }
        sql.push(" ORDER BY items.updated_at DESC,items.id DESC LIMIT ")
            .push_bind(i64::from(query.limit) + 1);
        let rows = sql
            .build()
            .fetch_all(&mut *tx)
            .await
            .map_err(storage_error)?;
        let mut items = rows
            .iter()
            .map(|r| decode(r.get::<&str, _>("json")))
            .collect::<Result<Vec<RemoteItem>>>()?;
        let has_more = items.len() > query.limit as usize;
        items.truncate(query.limit as usize);
        let next_cursor = if has_more {
            items
                .last()
                .map(|item| {
                    encode(&Cursor {
                        authorization_view: authorization_view.clone(),
                        projection_view: projection_view.clone(),
                        query: fingerprint,
                        updated_at: item.updated_at.clone(),
                        id: item.id.clone(),
                    })
                })
                .transpose()?
        } else {
            None
        };
        let (coverage, sync) = query_presentation(&mut tx, &query).await?;
        let ids: Vec<&str> = items.iter().map(|item| item.id.as_str()).collect();
        let pending_intents = effective::pending_in(&mut tx, &query.account_id, &ids).await?;
        tx.commit().await.map_err(storage_error)?;
        Ok(ItemPage {
            items,
            total_count: total_count as u64,
            pending_intents,
            revision,
            authorization_view,
            next_cursor,
            coverage,
            sync,
        })
    }

    pub async fn item(&self, account_id: &str, item_id: &str) -> Result<ItemSnapshot> {
        let mut tx = self.inner.readers.begin().await.map_err(storage_error)?;
        account_in(&mut tx, account_id, true).await?;
        let (revision, authorization_view) = metadata(&mut tx).await?;
        let json: Option<String> =
            sqlx::query_scalar("SELECT json FROM effective_items WHERE account_id=? AND id=?")
                .bind(account_id)
                .bind(item_id)
                .fetch_optional(&mut *tx)
                .await
                .map_err(storage_error)?;
        let mut item: Option<RemoteItem> = json.as_ref().map(|s| decode(s)).transpose()?;
        if let Some(value) = &item {
            let kind = match value.kind {
                RemoteItemKind::PullRequest => ResourceKind::PullRequest,
                RemoteItemKind::Issue => ResourceKind::Issue,
                RemoteItemKind::Notification => ResourceKind::Notification,
            };
            if !identities::accessible(&mut tx, account_id, item_id, kind).await? {
                item = None;
            }
        }
        let pending_intent = if item.is_some() {
            effective::pending_in(&mut tx, account_id, &[item_id])
                .await?
                .pop()
        } else {
            None
        };
        tx.commit().await.map_err(storage_error)?;
        Ok(ItemSnapshot {
            item,
            pending_intent,
            revision,
            authorization_view,
        })
    }

    pub async fn begin_sync(&self, account_id: &str, epoch: &str, scope: &str) -> Result<String> {
        validate_scope(scope)?;
        let mut writer = self.inner.writer.acquire().await?;
        let mut tx = writer.begin().await.map_err(storage_error)?;
        epoch_in(&mut tx, account_id, epoch).await?;
        ensure_selected_scope(&mut tx, account_id, scope).await?;
        let run_id = Uuid::new_v4().to_string();
        let old = scope_in(&mut tx, account_id, scope).await?;
        let coverage = old
            .as_ref()
            .map(|s| s.coverage.clone())
            .unwrap_or_else(missing_coverage);
        let mut sync = old.map(|s| s.sync).unwrap_or_default();
        sync.state = SyncState::Syncing;
        sync.error = None;
        sync.next_retry_at = None;
        sqlx::query("INSERT INTO sync_scopes(account_id,scope,run_id,coverage_json,sync_json) VALUES(?,?,?,?,?) ON CONFLICT(account_id,scope) DO UPDATE SET run_id=excluded.run_id,sync_json=excluded.sync_json")
            .bind(account_id).bind(scope).bind(&run_id).bind(encode(&coverage)?).bind(encode(&sync)?)
            .execute(&mut *tx).await.map_err(storage_error)?;
        record_change(&mut tx, account_id, positive_revision(epoch)?, scope, false).await?;
        tx.commit().await.map_err(storage_error)?;
        Ok(run_id)
    }

    pub async fn scope_state(&self, account_id: &str, scope: &str) -> Result<Option<StoredScope>> {
        let mut tx = self.inner.readers.begin().await.map_err(storage_error)?;
        account_in(&mut tx, account_id, true).await?;
        let scope = scope_in(&mut tx, account_id, scope).await?;
        tx.commit().await.map_err(storage_error)?;
        Ok(scope)
    }

    pub async fn apply_page(&self, page: PageCommit) -> Result<String> {
        self.apply_page_with_notification_subjects(page, vec![])
            .await
    }

    /// Apply trusted native observations directly (also used by store fixtures).
    /// Network feed responses must use `apply_fetched_page` with the revision
    /// captured before sending HTTP.
    pub async fn apply_page_with_notification_subjects(
        &self,
        page: PageCommit,
        observations: Vec<crate::NotificationSubjectObservation>,
    ) -> Result<String> {
        self.apply_page_in_revision(page, observations, None).await
    }

    pub(crate) async fn apply_fetched_page(
        &self,
        page: PageCommit,
        observations: Vec<crate::NotificationSubjectObservation>,
        expected_data_revision: i64,
    ) -> Result<String> {
        self.apply_page_in_revision(page, observations, Some(expected_data_revision))
            .await
    }

    async fn apply_page_in_revision(
        &self,
        page: PageCommit,
        observations: Vec<crate::NotificationSubjectObservation>,
        expected_data_revision: Option<i64>,
    ) -> Result<String> {
        validate_scope(&page.scope)?;
        if page.repositories.len() + page.items.len() + page.endpoint_aliases.len() > 100 {
            return Err(CollaborationError::invalid(
                "Provider page exceeds the write batch limit",
            ));
        }
        if page.complete && page.next_cursor.is_some() {
            return Err(CollaborationError::invalid(
                "Completed traversal cannot have a continuation",
            ));
        }
        if page.not_modified
            && (!page.items.is_empty()
                || !page.repositories.is_empty()
                || !page.endpoint_aliases.is_empty())
        {
            return Err(CollaborationError::invalid(
                "Unmodified response cannot contain observations",
            ));
        }
        let mut writer = self.inner.writer.acquire().await?;
        let mut tx = writer.begin().await.map_err(storage_error)?;
        epoch_in(&mut tx, &page.account_id, &page.authorization_epoch).await?;
        let account = account_in(&mut tx, &page.account_id, true).await?;
        notification_subjects::capture_in(&mut tx, &page.account_id).await?;
        ensure_selected_scope(&mut tx, &page.account_id, &page.scope).await?;
        let stored = scope_in(&mut tx, &page.account_id, &page.scope)
            .await?
            .ok_or_else(stale)?;
        if stored.run_id != page.run_id
            || expected_data_revision.is_some_and(|revision| revision != stored.data_revision)
        {
            return Err(stale());
        }
        if page.not_modified && stored.coverage.state != CoverageState::Complete {
            return Err(CollaborationError::invalid(
                "Unmodified response cannot complete an uncached traversal",
            ));
        }
        for repository in &page.repositories {
            if repository.account_id != page.account_id
                || (page.scope != "repositories" && page.scope != "notifications")
            {
                return Err(CollaborationError::invalid(
                    "Repository observation has the wrong account or scope",
                ));
            }
            validate_identifier(&repository.id)?;
            identities::repository_in(&mut tx, &account, repository).await?;
            sqlx::query("INSERT INTO repositories(account_id,id,provider_id,full_name,selected,json) VALUES(?,?,?,?,?,?) ON CONFLICT(account_id,id) DO UPDATE SET provider_id=excluded.provider_id,full_name=excluded.full_name,json=excluded.json")
                .bind(&page.account_id).bind(&repository.id).bind(&repository.provider_id).bind(&repository.full_name).bind(repository.selected).bind(encode(repository)?)
                .execute(&mut *tx).await.map_err(storage_error)?;
            if page.scope == "repositories" {
                seen(&mut tx, &page, &repository.id).await?;
            }
        }
        for incoming in &page.items {
            if incoming.account_id != page.account_id || query_item_scope(incoming) != page.scope {
                return Err(CollaborationError::invalid(
                    "Item observation has the wrong account or scope",
                ));
            }
            validate_identifier(&incoming.id)?;
            if incoming
                .body
                .as_ref()
                .is_some_and(|s| s.len() > MAX_BODY_BYTES)
                || incoming.title.len() > 16_384
            {
                return Err(CollaborationError::invalid(
                    "Provider observation exceeds the text limit",
                ));
            }
            let mut item = incoming.clone();
            let updated = chrono::DateTime::parse_from_rfc3339(&item.updated_at).map_err(|_| {
                CollaborationError::invalid("Provider observation has an invalid timestamp")
            })?;
            item.updated_at = updated
                .with_timezone(&chrono::Utc)
                .to_rfc3339_opts(chrono::SecondsFormat::Nanos, true);
            let previous: Option<String> =
                sqlx::query_scalar("SELECT json FROM items WHERE account_id=? AND id=?")
                    .bind(&page.account_id)
                    .bind(&item.id)
                    .fetch_optional(&mut *tx)
                    .await
                    .map_err(storage_error)?;
            let mut previous_head = None;
            if let Some(previous) = previous {
                let previous: RemoteItem = decode(&previous)?;
                previous_head = previous.head_oid.clone();
                // Provider timestamps are comparable for this list projection.
                // Missing detail fields from a summary never erase cached detail.
                if timestamp_older(&item.updated_at, &previous.updated_at) {
                    seen(&mut tx, &page, &item.id).await?;
                    continue;
                }
                if item.body_omitted {
                    item.body = previous.body;
                    item.body_omitted = previous.body_omitted;
                }
                if item.head_oid.is_none() {
                    item.head_oid = previous.head_oid;
                }
                if item.is_draft.is_none() {
                    item.is_draft = previous.is_draft;
                }
            }
            identities::item_in(&mut tx, &account, &item).await?;
            resource_metadata::invalidate_head_in(
                &mut tx,
                &account,
                &item,
                previous_head.as_deref(),
            )
            .await?;
            sqlx::query("INSERT INTO items(account_id,id,repository_id,kind,state,updated_at,json) VALUES(?,?,?,?,?,?,?) ON CONFLICT(account_id,id) DO UPDATE SET repository_id=excluded.repository_id,kind=excluded.kind,state=excluded.state,updated_at=excluded.updated_at,json=excluded.json")
                .bind(&page.account_id).bind(&item.id).bind(&item.repository_id).bind(tag(&item.kind)?).bind(&item.state).bind(&item.updated_at).bind(encode(&item)?)
                .execute(&mut *tx).await.map_err(storage_error)?;
            sqlx::query("DELETE FROM items_fts WHERE account_id=? AND id=?")
                .bind(&page.account_id)
                .bind(&item.id)
                .execute(&mut *tx)
                .await
                .map_err(storage_error)?;
            let body: String = item
                .body
                .as_deref()
                .unwrap_or("")
                .chars()
                .take(MAX_FTS_BODY_CHARS)
                .collect();
            sqlx::query("INSERT INTO items_fts(account_id,id,title,body) VALUES(?,?,?,?)")
                .bind(&page.account_id)
                .bind(&item.id)
                .bind(&item.title)
                .bind(body)
                .execute(&mut *tx)
                .await
                .map_err(storage_error)?;
            effective::refresh_target_in(&mut tx, &page.account_id, &item.id).await?;
            seen(&mut tx, &page, &item.id).await?;
        }
        for alias in &page.endpoint_aliases {
            identities::endpoint_in(&mut tx, &account, alias, &page.scope).await?;
        }
        notification_subjects::observe_in(&mut tx, &account, &page, &observations).await?;
        // Two completed traversals establish observed feed absence. Keep the
        // canonical row: a list miss never proves deletion or denied access.
        // Partial traversals cannot hide previous membership. A 304 leaves the
        // previous validated membership intact.
        let completed_run: Option<String> = sqlx::query_scalar(
            "SELECT completed_run_id FROM sync_scopes WHERE account_id=? AND scope=?",
        )
        .bind(&page.account_id)
        .bind(&page.scope)
        .fetch_one(&mut *tx)
        .await
        .map_err(storage_error)?;
        if page.complete && !page.not_modified && completed_run.as_deref() != Some(&page.run_id) {
            sqlx::query("UPDATE scope_membership SET missing_count=missing_count+1,active=CASE WHEN missing_count+1>=2 THEN 0 ELSE active END WHERE account_id=? AND scope=? AND last_seen_run<>?")
                .bind(&page.account_id).bind(&page.scope).bind(&page.run_id).execute(&mut *tx).await.map_err(storage_error)?;
        }
        let coverage = Coverage {
            state: if page.complete {
                CoverageState::Complete
            } else {
                CoverageState::Partial
            },
            validated_at: if page.complete {
                Some(page.observed_at.clone())
            } else {
                stored.coverage.validated_at
            },
            remote_has_more: page.next_cursor.is_some(),
        };
        let sync = SyncStatus {
            state: if page.complete {
                SyncState::Idle
            } else {
                SyncState::Syncing
            },
            last_success_at: if page.complete {
                Some(page.observed_at)
            } else {
                stored.sync.last_success_at
            },
            next_retry_at: None,
            error: None,
        };
        let pending_absence: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM scope_membership WHERE account_id=? AND scope=? AND active=1 AND missing_count>0)")
            .bind(&page.account_id).bind(&page.scope).fetch_one(&mut *tx).await.map_err(storage_error)?;
        // A conditional response cannot supply the second full enumeration
        // required to confirm absence. Force that enumeration before validating
        // subsequent reads conditionally.
        let (etag, last_modified) = if pending_absence {
            (None, None)
        } else {
            (page.etag, page.last_modified)
        };
        sqlx::query("UPDATE sync_scopes SET next_cursor=?,etag=?,last_modified=?,coverage_json=?,sync_json=?,access_denied=0,completed_run_id=CASE WHEN ? THEN ? ELSE completed_run_id END WHERE account_id=? AND scope=?")
            .bind(page.next_cursor).bind(etag).bind(last_modified).bind(encode(&coverage)?).bind(encode(&sync)?).bind(page.complete).bind(&page.run_id).bind(&page.account_id).bind(&page.scope)
            .execute(&mut *tx).await.map_err(storage_error)?;
        notification_subjects::reconcile_in(&mut tx, &account).await?;
        let revision = record_change(
            &mut tx,
            &page.account_id,
            positive_revision(&page.authorization_epoch)?,
            &page.scope,
            false,
        )
        .await?;
        sqlx::query("UPDATE sync_scopes SET data_revision=? WHERE account_id=? AND scope=?")
            .bind(revision_number(&revision)?)
            .bind(&page.account_id)
            .bind(&page.scope)
            .execute(&mut *tx)
            .await
            .map_err(storage_error)?;
        tx.commit().await.map_err(storage_error)?;
        Ok(revision)
    }

    pub async fn set_sync_status(
        &self,
        account_id: &str,
        epoch: &str,
        scope: &str,
        status: SyncStatus,
    ) -> Result<String> {
        validate_scope(scope)?;
        let mut writer = self.inner.writer.acquire().await?;
        let mut tx = writer.begin().await.map_err(storage_error)?;
        epoch_in(&mut tx, account_id, epoch).await?;
        notification_subjects::capture_in(&mut tx, account_id).await?;
        ensure_selected_scope(&mut tx, account_id, scope).await?;
        if status.state == SyncState::AuthRequired {
            // Authentication revocation fences *all* provider content and old
            // responses atomically; durable local drafts survive.
            let mut account = account_in(&mut tx, account_id, true).await?;
            let new_epoch = positive_revision(epoch)?
                .checked_add(1)
                .ok_or_else(CollaborationError::storage)?;
            account.authorization_epoch = new_epoch.to_string();
            account.state = AccountState::AuthRequired;
            clear_remote_cache(&mut tx, account_id).await?;
            sqlx::query(
                "UPDATE accounts SET authorization_epoch=?,state='auth_required',json=? WHERE id=?",
            )
            .bind(new_epoch)
            .bind(encode(&account)?)
            .bind(account_id)
            .execute(&mut *tx)
            .await
            .map_err(storage_error)?;
            sqlx::query(
                "UPDATE runtime_meta SET authorization_view=authorization_view+1 WHERE singleton=1",
            )
            .execute(&mut *tx)
            .await
            .map_err(storage_error)?;
            let revision = record_change(&mut tx, account_id, new_epoch, "account", true).await?;
            tx.commit().await.map_err(storage_error)?;
            return Ok(revision);
        }
        let was_denied: Option<bool> = sqlx::query_scalar(
            "SELECT access_denied FROM sync_scopes WHERE account_id=? AND scope=?",
        )
        .bind(account_id)
        .bind(scope)
        .fetch_optional(&mut *tx)
        .await
        .map_err(storage_error)?;
        let denied = status.error.as_ref().is_some_and(|e| {
            e.code == ErrorCode::PermissionDenied || e.code == ErrorCode::NotFound
        });
        let reset = denied && was_denied != Some(true);
        sqlx::query("INSERT INTO sync_scopes(account_id,scope,run_id,coverage_json,sync_json) VALUES(?,?,?,?,?) ON CONFLICT(account_id,scope) DO UPDATE SET sync_json=excluded.sync_json")
            .bind(account_id).bind(scope).bind(Uuid::new_v4().to_string()).bind(encode(&missing_coverage())?).bind(encode(&status)?)
            .execute(&mut *tx).await.map_err(storage_error)?;
        if denied {
            sqlx::query("UPDATE sync_scopes SET access_denied=1 WHERE account_id=? AND scope=?")
                .bind(account_id)
                .bind(scope)
                .execute(&mut *tx)
                .await
                .map_err(storage_error)?;
        }
        if reset {
            sqlx::query(
                "UPDATE runtime_meta SET authorization_view=authorization_view+1 WHERE singleton=1",
            )
            .execute(&mut *tx)
            .await
            .map_err(storage_error)?;
        }
        let account = account_in(&mut tx, account_id, true).await?;
        notification_subjects::reconcile_in(&mut tx, &account).await?;
        let revision =
            record_change(&mut tx, account_id, positive_revision(epoch)?, scope, reset).await?;
        tx.commit().await.map_err(storage_error)?;
        Ok(revision)
    }

    pub async fn changes_since(&self, after_revision: &str) -> Result<ChangePage> {
        let after = revision_number(after_revision)?;
        let mut tx = self.inner.readers.begin().await.map_err(storage_error)?;
        let (current, authorization_view) = metadata(&mut tx).await?;
        let current_number = revision_number(&current)?;
        let floor: i64 = sqlx::query_scalar("SELECT log_floor FROM runtime_meta WHERE singleton=1")
            .fetch_one(&mut *tx)
            .await
            .map_err(storage_error)?;
        if after < floor || after > current_number {
            tx.commit().await.map_err(storage_error)?;
            return Ok(ChangePage {
                revision: current,
                authorization_view,
                reset_required: true,
                changes: vec![],
                has_more: false,
            });
        }
        let rows = sqlx::query("SELECT l.revision,l.account_id,l.scope,l.reset,l.authorization_epoch,a.authorization_epoch AS current_epoch FROM change_log l JOIN accounts a ON a.id=l.account_id WHERE l.revision>? ORDER BY l.revision LIMIT ?")
            .bind(after).bind(MAX_CHANGE_PAGE).fetch_all(&mut *tx).await.map_err(storage_error)?;
        let mut through = after;
        let mut reset_required = false;
        let mut changes = Vec::with_capacity(rows.len());
        for row in rows {
            through = row.get("revision");
            let reset: bool = row.get("reset");
            if reset
                || row.get::<i64, _>("authorization_epoch") != row.get::<i64, _>("current_epoch")
            {
                reset_required = true;
            }
            // Epoch-obsolete metadata is not needed by a subscriber after reset.
            if row.get::<i64, _>("authorization_epoch") == row.get::<i64, _>("current_epoch") {
                changes.push(CollaborationChange {
                    revision: through.to_string(),
                    account_id: row.get("account_id"),
                    scope: row.get("scope"),
                    reset,
                });
            }
        }
        tx.commit().await.map_err(storage_error)?;
        Ok(ChangePage {
            revision: through.to_string(),
            authorization_view,
            reset_required,
            changes,
            has_more: through < current_number,
        })
    }

    pub async fn save_draft(&self, mut draft: LocalDraft) -> Result<LocalDraft> {
        validate_identifier(&draft.subject_id)?;
        if draft.body.len() > MAX_BODY_BYTES {
            return Err(CollaborationError::invalid("Draft exceeds the text limit"));
        }
        let expected = revision_number(&draft.generation)?;
        let mut writer = self.inner.writer.acquire().await?;
        let mut tx = writer.begin().await.map_err(storage_error)?;
        // Drafts remain editable offline and after account disconnection.
        let account = account_in(&mut tx, &draft.account_id, false).await?;
        let previous: Option<i64> =
            sqlx::query_scalar("SELECT generation FROM drafts WHERE account_id=? AND subject_id=?")
                .bind(&draft.account_id)
                .bind(&draft.subject_id)
                .fetch_optional(&mut *tx)
                .await
                .map_err(storage_error)?;
        if previous.unwrap_or(0) != expected {
            return Err(stale());
        }
        let generation = expected
            .checked_add(1)
            .ok_or_else(CollaborationError::storage)?;
        sqlx::query("INSERT INTO drafts(account_id,subject_id,body,generation) VALUES(?,?,?,?) ON CONFLICT(account_id,subject_id) DO UPDATE SET body=excluded.body,generation=excluded.generation")
            .bind(&draft.account_id).bind(&draft.subject_id).bind(&draft.body).bind(generation).execute(&mut *tx).await.map_err(storage_error)?;
        record_change(
            &mut tx,
            &draft.account_id,
            positive_revision(&account.authorization_epoch)?,
            "drafts",
            false,
        )
        .await?;
        tx.commit().await.map_err(storage_error)?;
        draft.generation = generation.to_string();
        Ok(draft)
    }

    /// No provider join or active-credential check: authored drafts survive
    /// permission changes, missing subjects and disconnected accounts.
    pub async fn query_drafts(&self, query: DraftQuery) -> Result<DraftPage> {
        if query.limit == 0 || query.limit > MAX_ITEMS {
            return Err(CollaborationError::invalid("Invalid local draft page size"));
        }
        let cursor = query
            .cursor
            .as_ref()
            .map(|value| {
                if value.len() > 4096 {
                    return Err(CollaborationError::invalid("Invalid local draft cursor"));
                }
                let cursor: DraftCursor = serde_json::from_str(value)
                    .map_err(|_| CollaborationError::invalid("Invalid local draft cursor"))?;
                if cursor.version != 1 || cursor.account_id != query.account_id {
                    return Err(CollaborationError::invalid(
                        "Cursor belongs to a different account",
                    ));
                }
                validate_identifier(&cursor.subject_id)?;
                Ok(cursor)
            })
            .transpose()?;
        let mut tx = self.inner.readers.begin().await.map_err(storage_error)?;
        account_in(&mut tx, &query.account_id, false).await?;
        let rows = sqlx::query("SELECT subject_id,substr(body,1,160) AS preview,generation FROM drafts WHERE account_id=? AND subject_id>? ORDER BY subject_id ASC LIMIT ?")
            .bind(&query.account_id)
            .bind(cursor.map(|cursor| cursor.subject_id).unwrap_or_default())
            .bind(i64::from(query.limit) + 1)
            .fetch_all(&mut *tx).await.map_err(storage_error)?;
        let mut drafts: Vec<DraftSummary> = rows
            .iter()
            .map(|row| DraftSummary {
                subject_id: row.get("subject_id"),
                preview: row.get("preview"),
                generation: row.get::<i64, _>("generation").to_string(),
            })
            .collect();
        let has_more = drafts.len() > query.limit as usize;
        drafts.truncate(query.limit as usize);
        let next_cursor = if has_more {
            drafts
                .last()
                .map(|draft| {
                    encode(&DraftCursor {
                        version: 1,
                        account_id: query.account_id,
                        subject_id: draft.subject_id.clone(),
                    })
                })
                .transpose()?
        } else {
            None
        };
        tx.commit().await.map_err(storage_error)?;
        Ok(DraftPage {
            drafts,
            next_cursor,
        })
    }

    pub async fn draft(&self, account_id: &str, subject_id: &str) -> Result<Option<LocalDraft>> {
        let mut tx = self.inner.readers.begin().await.map_err(storage_error)?;
        account_in(&mut tx, account_id, false).await?;
        let row =
            sqlx::query("SELECT body,generation FROM drafts WHERE account_id=? AND subject_id=?")
                .bind(account_id)
                .bind(subject_id)
                .fetch_optional(&mut *tx)
                .await
                .map_err(storage_error)?;
        let draft = row.map(|r| LocalDraft {
            account_id: account_id.into(),
            subject_id: subject_id.into(),
            body: r.get("body"),
            generation: r.get::<i64, _>("generation").to_string(),
        });
        tx.commit().await.map_err(storage_error)?;
        Ok(draft)
    }
}

async fn upsert_account_in(
    tx: &mut Transaction<'_, Sqlite>,
    account: &RemoteAccount,
) -> Result<()> {
    validate_identifier(&account.id)?;
    validate_identifier(&account.actor_id)?;
    let epoch = positive_revision(&account.authorization_epoch)?;
    if let Some(row) =
        sqlx::query("SELECT authorization_epoch, provider, host, actor_id FROM accounts WHERE id=?")
            .bind(&account.id)
            .fetch_optional(&mut **tx)
            .await
            .map_err(storage_error)?
    {
        let current: i64 = row.get("authorization_epoch");
        if epoch <= current {
            return Err(stale());
        }
        if row.get::<String, _>("provider") != tag(&account.provider)?
            || row.get::<String, _>("host") != account.host
            || row.get::<String, _>("actor_id") != account.actor_id
        {
            return Err(CollaborationError::invalid(
                "Account identity cannot be changed",
            ));
        }
        clear_remote_cache(tx, &account.id).await?;
    }
    let provider = tag(&account.provider)?;
    let state = tag(&account.state)?;
    sqlx::query("INSERT INTO accounts(id,provider,host,actor_id,authorization_epoch,state,json) VALUES(?,?,?,?,?,?,?) ON CONFLICT(id) DO UPDATE SET authorization_epoch=excluded.authorization_epoch,state=excluded.state,json=excluded.json")
            .bind(&account.id).bind(provider).bind(&account.host).bind(&account.actor_id).bind(epoch).bind(state).bind(encode(account)?)
            .execute(&mut **tx).await.map_err(storage_error)?;
    identities::bind_account_in(tx, account).await?;
    sqlx::query(
        "UPDATE runtime_meta SET authorization_view=authorization_view+1 WHERE singleton=1",
    )
    .execute(&mut **tx)
    .await
    .map_err(storage_error)?;
    record_change(tx, &account.id, epoch, "account", true).await?;
    Ok(())
}

async fn retire_credential_in(
    tx: &mut Transaction<'_, Sqlite>,
    account_id: &str,
    reference: &str,
) -> Result<()> {
    sqlx::query("INSERT INTO credential_cleanup(credential_ref,account_id,state) VALUES(?,?,'retired') ON CONFLICT(credential_ref) DO UPDATE SET state='retired',attempts=0,next_retry_at=0")
        .bind(reference).bind(account_id).execute(&mut **tx).await.map_err(storage_error)?;
    Ok(())
}

fn storage_error(_: sqlx::Error) -> CollaborationError {
    CollaborationError::storage()
}
pub(crate) fn acquire_writer_lease(path: &Path) -> Result<WriterLease> {
    let mut name = path.as_os_str().to_os_string();
    name.push(".lock");
    let lock_path = std::path::PathBuf::from(name);
    if std::fs::symlink_metadata(&lock_path).is_ok_and(|m| m.file_type().is_symlink()) {
        return Err(CollaborationError::storage());
    }
    let mut options = std::fs::OpenOptions::new();
    options.read(true).write(true).create(true).truncate(false);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let lease = options
        .open(&lock_path)
        .map_err(|_| CollaborationError::storage())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        lease
            .set_permissions(std::fs::Permissions::from_mode(0o600))
            .map_err(|_| CollaborationError::storage())?;
    }
    match lease.try_lock() {
        Ok(()) => Ok(WriterLease { file: lease }),
        Err(std::fs::TryLockError::WouldBlock) => Err(CollaborationError::new(
            ErrorCode::Busy,
            "Collaboration storage is already open in another application instance",
        )),
        Err(_) => Err(CollaborationError::storage()),
    }
}
pub(crate) fn prepare_private_path(path: &Path) -> Result<()> {
    if std::fs::symlink_metadata(path).is_ok_and(|m| m.file_type().is_symlink()) {
        return Err(CollaborationError::storage());
    }
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        let mut directory = std::fs::DirBuilder::new();
        directory.recursive(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            directory.mode(0o700);
        }
        directory
            .create(parent)
            .map_err(|_| CollaborationError::storage())?;
    }
    Ok(())
}
fn secure_database_files(path: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        for suffix in ["", "-wal", "-shm"] {
            let mut name = path.as_os_str().to_os_string();
            name.push(suffix);
            let file = std::path::PathBuf::from(name);
            match std::fs::metadata(&file) {
                Ok(_) => std::fs::set_permissions(file, std::fs::Permissions::from_mode(0o600))
                    .map_err(|_| CollaborationError::storage())?,
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(_) => return Err(CollaborationError::storage()),
            }
        }
    }
    #[cfg(not(unix))]
    let _ = path;
    Ok(())
}
fn stale() -> CollaborationError {
    CollaborationError::new(
        ErrorCode::StaleView,
        "The account or local view changed; reload before retrying",
    )
}
fn not_found() -> CollaborationError {
    CollaborationError::new(
        ErrorCode::NotFound,
        "Local collaboration record was not found",
    )
}
fn encode(value: &impl Serialize) -> Result<String> {
    serde_json::to_string(value).map_err(|_| CollaborationError::storage())
}
fn decode<T: DeserializeOwned>(json: &str) -> Result<T> {
    serde_json::from_str(json).map_err(|_| CollaborationError::storage())
}
fn tag(value: &impl Serialize) -> Result<String> {
    decode::<String>(&encode(value)?)
}
fn revision_number(value: &str) -> Result<i64> {
    if value.is_empty() || value.len() > 19 || !value.bytes().all(|c| c.is_ascii_digit()) {
        return Err(CollaborationError::invalid("Invalid local revision"));
    }
    value
        .parse::<i64>()
        .map_err(|_| CollaborationError::invalid("Invalid local revision"))
}
fn positive_revision(value: &str) -> Result<i64> {
    let revision = revision_number(value)?;
    if revision == 0 {
        Err(CollaborationError::invalid(
            "Authorization epoch must be positive",
        ))
    } else {
        Ok(revision)
    }
}
pub(crate) fn validate_identifier(value: &str) -> Result<()> {
    if value.is_empty() || value.len() > 1024 || value.contains('\0') {
        Err(CollaborationError::invalid("Invalid local identifier"))
    } else {
        Ok(())
    }
}
fn fixed_sqlite_version(version: &str) -> bool {
    let parts: Vec<u32> = version.split('.').filter_map(|p| p.parse().ok()).collect();
    parts.len() == 3
        && ((parts[0], parts[1], parts[2]) >= (3, 51, 3)
            || parts[..2] == [3, 50] && parts[2] >= 7
            || parts[..2] == [3, 44] && parts[2] >= 6)
}
fn sqlite_version_at_least(version: &str, minimum: (u32, u32, u32)) -> bool {
    let parts: Vec<u32> = version
        .split('.')
        .filter_map(|part| part.parse().ok())
        .collect();
    parts.len() == 3 && (parts[0], parts[1], parts[2]) >= minimum
}
fn missing_coverage() -> Coverage {
    Coverage {
        state: CoverageState::Missing,
        validated_at: None,
        remote_has_more: false,
    }
}
fn presentation(stored: Option<StoredScope>) -> (Coverage, SyncStatus) {
    stored
        .map(|s| (s.coverage, s.sync))
        .unwrap_or_else(|| (missing_coverage(), SyncStatus::default()))
}
fn repository_scope(repository: &str, kind: &RemoteItemKind) -> String {
    format!(
        "repo:{repository}:{}",
        if *kind == RemoteItemKind::PullRequest {
            "pull_request"
        } else {
            "issue"
        }
    )
}
fn query_scope(query: &ItemQuery) -> String {
    if query.kind == RemoteItemKind::Notification {
        "notifications".into()
    } else {
        query
            .repository_id
            .as_ref()
            .map(|id| repository_scope(id, &query.kind))
            .unwrap_or_else(|| {
                format!(
                    "all:{}",
                    if query.kind == RemoteItemKind::PullRequest {
                        "pull_request"
                    } else {
                        "issue"
                    }
                )
            })
    }
}
async fn query_projection_view(
    tx: &mut Transaction<'_, Sqlite>,
    query: &ItemQuery,
) -> Result<String> {
    // Status, drafts and other accounts can advance the global catch-up log
    // without changing this list. Only selected visible feed content participates
    // in its cursor fence. Missing scopes count as zero, and selection changes
    // alter the sorted vector itself.
    let scopes: Vec<(String, i64)> = if query.kind == RemoteItemKind::Notification {
        let revision: Option<i64> = sqlx::query_scalar(
            "SELECT data_revision FROM sync_scopes WHERE account_id=? AND scope='notifications'",
        )
        .bind(&query.account_id)
        .fetch_optional(&mut **tx)
        .await
        .map_err(storage_error)?;
        vec![("notifications".into(), revision.unwrap_or(0))]
    } else {
        let mut sql = sqlx::QueryBuilder::<Sqlite>::new(
            "SELECT r.id,COALESCE(s.data_revision,0) AS data_revision FROM repositories r LEFT JOIN sync_scopes s ON s.account_id=r.account_id AND s.scope='repo:'||r.id||':'||",
        );
        sql.push_bind(tag(&query.kind)?)
            .push(" WHERE r.account_id=")
            .push_bind(&query.account_id)
            .push(" AND r.selected=1 AND ")
            .push(VISIBLE_REPOSITORY);
        if let Some(repository_id) = &query.repository_id {
            sql.push(" AND r.id=").push_bind(repository_id);
        }
        sql.push(" ORDER BY r.id LIMIT 2001");
        let rows = sql
            .build()
            .fetch_all(&mut **tx)
            .await
            .map_err(storage_error)?;
        if rows.len() > 2000 {
            return Err(CollaborationError::storage());
        }
        rows.into_iter()
            .map(|row| {
                let repository: String = row.get("id");
                (
                    repository_scope(&repository, &query.kind),
                    row.get("data_revision"),
                )
            })
            .collect()
    };
    // Keep cursors small regardless of the working-set size; JSON provides an
    // unambiguous tuple encoding before the digest is computed.
    Ok(format!(
        "{:x}",
        Sha256::digest(
            encode(&(scopes, effective::query_revision_in(tx, query).await?))?.as_bytes()
        )
    ))
}
async fn query_presentation(
    tx: &mut Transaction<'_, Sqlite>,
    query: &ItemQuery,
) -> Result<(Coverage, SyncStatus)> {
    if query.kind != RemoteItemKind::Notification
        && let Some(repository_id) = &query.repository_id
    {
        let mut sql = sqlx::QueryBuilder::<Sqlite>::new(
            "SELECT EXISTS(SELECT 1 FROM repositories r WHERE r.account_id=",
        );
        sql.push_bind(&query.account_id)
            .push(" AND r.id=")
            .push_bind(repository_id)
            .push(" AND r.selected=1 AND ")
            .push(VISIBLE_REPOSITORY)
            .push(")");
        let visible: bool = sql
            .build_query_scalar()
            .fetch_one(&mut **tx)
            .await
            .map_err(storage_error)?;
        if !visible {
            return Ok((missing_coverage(), SyncStatus::default()));
        }
    }
    if query.kind == RemoteItemKind::Notification || query.repository_id.is_some() {
        return Ok(presentation(
            scope_in(tx, &query.account_id, &query_scope(query)).await?,
        ));
    }
    let mut sql = sqlx::QueryBuilder::<Sqlite>::new(
        "SELECT s.coverage_json,s.sync_json FROM repositories r LEFT JOIN sync_scopes s ON s.account_id=r.account_id AND s.scope='repo:'||r.id||':'||",
    );
    sql.push_bind(tag(&query.kind)?)
        .push(" WHERE r.account_id=")
        .push_bind(&query.account_id)
        .push(" AND r.selected=1 AND ")
        .push(VISIBLE_REPOSITORY)
        .push(" ORDER BY r.id LIMIT 2001");
    let rows = sql
        .build()
        .fetch_all(&mut **tx)
        .await
        .map_err(storage_error)?;
    if rows.len() > 2000 {
        return Err(CollaborationError::storage());
    }
    if rows.is_empty() {
        return Ok((missing_coverage(), SyncStatus::default()));
    }
    let mut states: Vec<(Coverage, SyncStatus)> = Vec::with_capacity(rows.len());
    for row in rows {
        let coverage: Option<String> = row.get("coverage_json");
        let sync: Option<String> = row.get("sync_json");
        states.push((
            coverage
                .as_ref()
                .map(|s| decode::<Coverage>(s))
                .transpose()?
                .unwrap_or_else(missing_coverage),
            sync.as_ref()
                .map(|s| decode::<SyncStatus>(s))
                .transpose()?
                .unwrap_or_default(),
        ));
    }
    let state = if states
        .iter()
        .all(|(c, _)| c.state == CoverageState::Complete)
    {
        CoverageState::Complete
    } else if states
        .iter()
        .all(|(c, _)| c.state == CoverageState::Missing)
    {
        CoverageState::Missing
    } else {
        CoverageState::Partial
    };
    let coverage = Coverage {
        state,
        validated_at: states
            .iter()
            .filter_map(|(c, _)| c.validated_at.clone())
            .min(),
        remote_has_more: states.iter().any(|(c, _)| c.remote_has_more),
    };
    let sync = states
        .into_iter()
        .map(|(_, s)| s)
        .max_by_key(|s| sync_priority(&s.state))
        .unwrap_or_default();
    Ok((coverage, sync))
}
fn sync_priority(state: &SyncState) -> u8 {
    match state {
        SyncState::Idle => 0,
        SyncState::Syncing => 1,
        SyncState::Offline => 2,
        SyncState::RateLimited => 3,
        SyncState::Error => 4,
        SyncState::AuthRequired => 5,
    }
}
fn query_item_scope(item: &RemoteItem) -> String {
    if item.kind == RemoteItemKind::Notification {
        "notifications".into()
    } else {
        item.repository_id
            .as_ref()
            .map(|id| repository_scope(id, &item.kind))
            .unwrap_or_default()
    }
}
fn validate_scope(scope: &str) -> Result<()> {
    if scope == "repositories"
        || scope == "notifications"
        || scope == "provider:rest"
        || scope
            .strip_prefix(notification_subjects::PREFIX)
            .is_some_and(|id| validate_identifier(id).is_ok())
        || repository_from_scope(scope).is_some()
        || crate::DetailFacet::from_scope(scope).is_some()
    {
        Ok(())
    } else {
        Err(CollaborationError::invalid(
            "Unknown collaboration sync scope",
        ))
    }
}
fn repository_from_scope(scope: &str) -> Option<&str> {
    scope
        .strip_prefix("repo:")
        .and_then(|s| {
            s.strip_suffix(":pull_request")
                .or_else(|| s.strip_suffix(":issue"))
        })
        .filter(|s| !s.is_empty())
}
fn timestamp_older(candidate: &str, current: &str) -> bool {
    match (
        chrono::DateTime::parse_from_rfc3339(candidate),
        chrono::DateTime::parse_from_rfc3339(current),
    ) {
        (Ok(candidate), Ok(current)) => candidate < current,
        _ => false,
    }
}

async fn metadata(tx: &mut Transaction<'_, Sqlite>) -> Result<(String, String)> {
    let row = sqlx::query("SELECT revision,authorization_view FROM runtime_meta WHERE singleton=1")
        .fetch_one(&mut **tx)
        .await
        .map_err(storage_error)?;
    Ok((
        row.get::<i64, _>("revision").to_string(),
        row.get::<i64, _>("authorization_view").to_string(),
    ))
}
async fn account_in(
    tx: &mut Transaction<'_, Sqlite>,
    account_id: &str,
    require_active: bool,
) -> Result<RemoteAccount> {
    let json: Option<String> = sqlx::query_scalar("SELECT json FROM accounts WHERE id=?")
        .bind(account_id)
        .fetch_optional(&mut **tx)
        .await
        .map_err(storage_error)?;
    let account: RemoteAccount = decode(&json.ok_or_else(not_found)?)?;
    if require_active && account.state != AccountState::Active {
        return Err(CollaborationError::new(
            ErrorCode::AuthRequired,
            "Reconnect the provider account to read remote data",
        ));
    }
    Ok(account)
}
fn validate_provider_deadline(deadline: &str) -> Result<()> {
    if deadline.len() > 128 || chrono::DateTime::parse_from_rfc3339(deadline).is_err() {
        return Err(CollaborationError::invalid(
            "Invalid provider quota deadline",
        ));
    }
    Ok(())
}

/// Both credential cutover and same-epoch observations hold the writer here.
async fn provider_budget_in(
    tx: &mut Transaction<'_, Sqlite>,
    account_id: &str,
    epoch: &str,
    proposed: String,
    error: Option<CollaborationError>,
) -> Result<String> {
    let prior: Option<String> = sqlx::query_scalar(
        "SELECT sync_json FROM sync_scopes WHERE account_id=? AND scope='provider:rest'",
    )
    .bind(account_id)
    .fetch_optional(&mut **tx)
    .await
    .map_err(storage_error)?;
    let prior: Option<SyncStatus> = prior.as_deref().map(decode).transpose()?;
    let retained = prior
        .as_ref()
        .and_then(|status| status.next_retry_at.as_ref())
        .filter(|old| {
            chrono::DateTime::parse_from_rfc3339(old)
                .ok()
                .zip(chrono::DateTime::parse_from_rfc3339(&proposed).ok())
                .is_some_and(|(old, new)| old > new)
        })
        .cloned();
    let status = SyncStatus {
        state: SyncState::RateLimited,
        last_success_at: prior
            .as_ref()
            .and_then(|status| status.last_success_at.clone()),
        next_retry_at: Some(retained.clone().unwrap_or(proposed)),
        // Retaining a longer existing barrier also retains its diagnostic when
        // the new observation carries no error. Neither field revokes access.
        error: error.or_else(|| retained.and_then(|_| prior.and_then(|status| status.error))),
    };
    sqlx::query("INSERT INTO sync_scopes(account_id,scope,run_id,coverage_json,sync_json) VALUES(?,'provider:rest',?,?,?) ON CONFLICT(account_id,scope) DO UPDATE SET sync_json=excluded.sync_json")
        .bind(account_id).bind(Uuid::new_v4().to_string()).bind(encode(&missing_coverage())?).bind(encode(&status)?)
        .execute(&mut **tx).await.map_err(storage_error)?;
    record_change(
        tx,
        account_id,
        positive_revision(epoch)?,
        "provider:rest",
        false,
    )
    .await
}

async fn epoch_in(tx: &mut Transaction<'_, Sqlite>, account_id: &str, epoch: &str) -> Result<()> {
    let account = account_in(tx, account_id, false).await?;
    if account.authorization_epoch != epoch || account.state != AccountState::Active {
        return Err(stale());
    }
    Ok(())
}
async fn ensure_selected_scope(
    tx: &mut Transaction<'_, Sqlite>,
    account_id: &str,
    scope: &str,
) -> Result<()> {
    if let Some((subject, _)) = crate::DetailFacet::from_scope(scope) {
        details::subject_in(tx, account_id, subject).await?;
    }
    // Storage enforces actor/epoch/scope visibility. Provider functionality is
    // checked by the registry at admission and dispatch, including non-native
    // inboxes such as to-dos which have no GitHub notification grant flag.
    if let Some(repository_id) = repository_from_scope(scope) {
        let mut sql = sqlx::QueryBuilder::<Sqlite>::new(
            "SELECT r.selected FROM repositories r WHERE r.account_id=",
        );
        sql.push_bind(account_id)
            .push(" AND r.id=")
            .push_bind(repository_id)
            .push(" AND ")
            .push(VISIBLE_REPOSITORY);
        let selected: Option<bool> = sql
            .build_query_scalar()
            .fetch_optional(&mut **tx)
            .await
            .map_err(storage_error)?;
        if selected != Some(true) {
            return Err(stale());
        }
    }
    Ok(())
}
async fn scope_in(
    tx: &mut Transaction<'_, Sqlite>,
    account_id: &str,
    scope: &str,
) -> Result<Option<StoredScope>> {
    let row = sqlx::query("SELECT run_id,data_revision,next_cursor,etag,last_modified,coverage_json,sync_json FROM sync_scopes WHERE account_id=? AND scope=?")
        .bind(account_id).bind(scope).fetch_optional(&mut **tx).await.map_err(storage_error)?;
    row.map(|r| {
        Ok(StoredScope {
            run_id: r.get("run_id"),
            data_revision: r.get("data_revision"),
            next_cursor: r.get("next_cursor"),
            etag: r.get("etag"),
            last_modified: r.get("last_modified"),
            coverage: decode(r.get::<&str, _>("coverage_json"))?,
            sync: decode(r.get::<&str, _>("sync_json"))?,
        })
    })
    .transpose()
}
fn repository_from_row(row: &sqlx::sqlite::SqliteRow) -> Result<RemoteRepository> {
    let mut repository: RemoteRepository = decode(row.get::<&str, _>("json"))?;
    repository.selected = row.get("selected");
    Ok(repository)
}
async fn clear_remote_cache(tx: &mut Transaction<'_, Sqlite>, account_id: &str) -> Result<()> {
    for sql in [
        "DELETE FROM notification_subject_discovery WHERE account_id=?",
        "DELETE FROM notification_subject_selectors WHERE account_id=?",
        "DELETE FROM detail_demand WHERE account_id=?",
        "DELETE FROM detail_observations WHERE account_id=?",
        "DELETE FROM effective_items_fts WHERE account_id=?",
        "DELETE FROM items_fts WHERE account_id=?",
        "DELETE FROM items WHERE account_id=?",
        // Provider quota is metadata, not private provider content. A reconnect
        // cannot bypass a deadline already observed for this actor/account.
        "DELETE FROM sync_scopes WHERE account_id=? AND scope<>'provider:rest'",
        "DELETE FROM repositories WHERE account_id=?",
    ] {
        sqlx::query(sql)
            .bind(account_id)
            .execute(&mut **tx)
            .await
            .map_err(storage_error)?;
    }
    Ok(())
}
async fn seen(tx: &mut Transaction<'_, Sqlite>, page: &PageCommit, entity_id: &str) -> Result<()> {
    sqlx::query("INSERT INTO scope_membership(account_id,scope,entity_id,last_seen_run) VALUES(?,?,?,?) ON CONFLICT(account_id,scope,entity_id) DO UPDATE SET last_seen_run=excluded.last_seen_run,active=1,missing_count=0")
        .bind(&page.account_id).bind(&page.scope).bind(entity_id).bind(&page.run_id).execute(&mut **tx).await.map_err(storage_error)?;
    Ok(())
}
async fn record_change(
    tx: &mut Transaction<'_, Sqlite>,
    account_id: &str,
    epoch: i64,
    scope: &str,
    reset: bool,
) -> Result<String> {
    let revision: i64 = sqlx::query_scalar(
        "UPDATE runtime_meta SET revision=revision+1 WHERE singleton=1 RETURNING revision",
    )
    .fetch_one(&mut **tx)
    .await
    .map_err(storage_error)?;
    sqlx::query("INSERT INTO change_log(revision,account_id,authorization_epoch,scope,reset) VALUES(?,?,?,?,?)")
        .bind(revision).bind(account_id).bind(epoch).bind(scope).bind(reset).execute(&mut **tx).await.map_err(storage_error)?;
    let floor = revision.saturating_sub(CHANGE_LOG_LIMIT);
    if floor > 0 {
        sqlx::query("DELETE FROM change_log WHERE revision<=?")
            .bind(floor)
            .execute(&mut **tx)
            .await
            .map_err(storage_error)?;
        sqlx::query("UPDATE runtime_meta SET log_floor=? WHERE singleton=1")
            .bind(floor)
            .execute(&mut **tx)
            .await
            .map_err(storage_error)?;
    }
    Ok(revision.to_string())
}

fn push_item_predicates(sql: &mut sqlx::QueryBuilder<Sqlite>, query: &ItemQuery) -> Result<()> {
    sql.push_bind(query.account_id.clone())
        .push(" AND items.kind=")
        .push_bind(tag(&query.kind)?);
    if query.kind != RemoteItemKind::Notification {
        sql.push(" AND EXISTS(SELECT 1 FROM repositories r WHERE r.account_id=items.account_id AND r.id=items.repository_id AND r.selected=1 AND ")
                .push(VISIBLE_REPOSITORY)
                .push(")");
    }
    sql.push(" AND NOT EXISTS(SELECT 1 FROM sync_scopes s WHERE s.account_id=items.account_id AND s.scope=CASE WHEN items.kind='notification' THEN 'notifications' ELSE 'repo:'||items.repository_id||':'||items.kind END AND s.access_denied=1)");
    sql.push(" AND EXISTS(SELECT 1 FROM scope_membership m WHERE m.account_id=items.account_id AND m.entity_id=items.id AND m.active=1 AND m.scope=CASE WHEN items.kind='notification' THEN 'notifications' ELSE 'repo:'||items.repository_id||':'||items.kind END)");
    if let Some(repository_id) = &query.repository_id {
        sql.push(" AND items.repository_id=")
            .push_bind(repository_id.clone());
    }
    if let Some(state) = &query.state {
        if query.kind == RemoteItemKind::Notification {
            match state.as_str() {
                "unread" => {
                    sql.push(" AND json_extract(items.json,'$.unread')=1");
                }
                "read" => {
                    sql.push(" AND json_extract(items.json,'$.unread')=0");
                }
                "pending" | "done" => {
                    sql.push(" AND items.state=").push_bind(state.clone());
                }
                _ => {
                    return Err(CollaborationError::invalid(
                        "Unsupported inbox disposition filter",
                    ));
                }
            }
        } else {
            sql.push(" AND items.state=").push_bind(state.clone());
        }
    }
    if let Some(search) = query.search.as_ref().filter(|s| !s.trim().is_empty()) {
        effective::push_search(sql, "items", &query.account_id, search);
    }
    Ok(())
}
