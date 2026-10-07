//! Bounded accounting and eviction for rebuildable collaboration detail data.
use super::*;

const MAX_INDEX_FACETS: u32 = 128;
const MAX_SCAN_FACETS: u32 = 128;
const MAX_EVICT_FACETS: u32 = 32;
const MAX_ENTRY_ROWS: u32 = 5_000;
const DEFAULT_TARGET_LOGICAL_BYTES: u64 = 512 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CacheRetentionPolicy {
    pub target_logical_bytes: u64,
    pub max_index_facets: u32,
    pub max_scan_facets: u32,
    pub max_evict_facets: u32,
    pub max_entry_rows: u32,
    pub checkpoint_wal: bool,
}

impl Default for CacheRetentionPolicy {
    fn default() -> Self {
        Self {
            target_logical_bytes: DEFAULT_TARGET_LOGICAL_BYTES,
            max_index_facets: MAX_INDEX_FACETS,
            max_scan_facets: MAX_SCAN_FACETS,
            max_evict_facets: MAX_EVICT_FACETS,
            max_entry_rows: MAX_ENTRY_ROWS,
            checkpoint_wal: true,
        }
    }
}

impl CacheRetentionPolicy {
    fn bounded(&self) -> Self {
        Self {
            target_logical_bytes: self.target_logical_bytes,
            max_index_facets: self.max_index_facets.clamp(1, MAX_INDEX_FACETS),
            max_scan_facets: self.max_scan_facets.clamp(1, MAX_SCAN_FACETS),
            max_evict_facets: self.max_evict_facets.clamp(1, MAX_EVICT_FACETS),
            max_entry_rows: self.max_entry_rows.clamp(1, MAX_ENTRY_ROWS),
            checkpoint_wal: self.checkpoint_wal,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CacheUsage {
    /// False only when nonblocking maintenance admission could not inspect SQLite.
    pub available: bool,
    pub logical_bytes: Option<u64>,
    pub indexed_logical_bytes: u64,
    pub indexed_facets: u64,
    pub index_complete: bool,
    pub database_bytes: u64,
    pub wal_bytes: u64,
    pub page_size: u64,
    pub page_count: u64,
    pub free_pages: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CacheMaintenanceReport {
    pub usage_before: CacheUsage,
    pub usage_after: CacheUsage,
    pub indexed_facets: u32,
    pub scanned_facets: u32,
    pub evicted_facets: u32,
    pub evicted_entry_rows: u32,
    pub freed_logical_bytes: u64,
    pub target_met: bool,
    pub skipped_busy: bool,
    pub checkpoint: Option<WalCheckpointResult>,
    /// A checkpoint failure after the retention transaction committed.
    pub checkpoint_error: Option<CollaborationError>,
    /// A fresh usage-read failure after commit. `usage_after` then contains the
    /// committed transaction snapshot, including authoritative logical totals.
    pub usage_refresh_error: Option<CollaborationError>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WalStatus {
    pub supported: bool,
    pub database_bytes: u64,
    pub wal_bytes: u64,
    pub busy: Option<i64>,
    pub log_frames: Option<i64>,
    pub checkpointed_frames: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WalCheckpointResult {
    pub skipped_busy: bool,
    pub busy: Option<i64>,
    pub log_frames: Option<i64>,
    pub checkpointed_frames: Option<i64>,
}

#[derive(Debug)]
struct RetentionState {
    index_complete: bool,
    index_cursor: Option<(String, String, String)>,
    eviction_cursor: Option<(i64, String, String, String)>,
    indexed_logical_bytes: u64,
}

#[derive(Debug)]
struct RetentionCandidate {
    account_id: String,
    subject_id: String,
    facet: String,
    logical_bytes: u64,
    revision: i64,
}

impl Store {
    /// Pins authored local intent. It remains available while an account is disconnected.
    pub async fn set_cache_pin(
        &self,
        account_id: &str,
        subject_id: &str,
        pinned: bool,
    ) -> Result<String> {
        validate_identifier(account_id)?;
        validate_identifier(subject_id)?;
        let mut writer = self.inner.writer.lock().await;
        let mut tx = writer.begin().await.map_err(storage_error)?;
        let account = account_in(&mut tx, account_id, false).await?;
        let identity = sqlx::query(
            "SELECT ri.instance_id,ri.kind FROM account_instances ai JOIN resource_identities ri ON ri.account_id=ai.account_id AND ri.instance_id=ai.instance_id WHERE ai.account_id=? AND ri.entity_id=? AND ri.kind IN ('pull_request','issue')",
        )
        .bind(account_id)
        .bind(subject_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(storage_error)?
        .ok_or_else(not_found)?;
        let instance_id: String = identity.get("instance_id");
        let kind: String = identity.get("kind");
        let exists: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM cache_pins WHERE account_id=? AND instance_id=? AND entity_id=?)",
        )
        .bind(account_id)
        .bind(&instance_id)
        .bind(subject_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(storage_error)?;
        if exists == pinned {
            let revision = metadata(&mut tx).await?.0;
            tx.commit().await.map_err(storage_error)?;
            return Ok(revision);
        }
        let revision = record_change(
            &mut tx,
            account_id,
            positive_revision(&account.authorization_epoch)?,
            "pins",
            false,
        )
        .await?;
        if pinned {
            sqlx::query("INSERT INTO cache_pins(account_id,instance_id,entity_id,kind,pinned_revision) VALUES(?,?,?,?,?)")
                .bind(account_id)
                .bind(instance_id)
                .bind(subject_id)
                .bind(kind)
                .bind(revision_number(&revision)?)
                .execute(&mut *tx)
                .await
                .map_err(storage_error)?;
        } else {
            sqlx::query(
                "DELETE FROM cache_pins WHERE account_id=? AND instance_id=? AND entity_id=?",
            )
            .bind(account_id)
            .bind(instance_id)
            .bind(subject_id)
            .execute(&mut *tx)
            .await
            .map_err(storage_error)?;
        }
        tx.commit().await.map_err(storage_error)?;
        Ok(revision)
    }

    pub async fn cache_usage(&self) -> Result<CacheUsage> {
        let mut tx = self.inner.readers.begin().await.map_err(storage_error)?;
        let usage = cache_usage_in(&mut tx, &self.inner.path).await?;
        tx.commit().await.map_err(storage_error)?;
        Ok(usage)
    }

    pub async fn run_cache_maintenance(
        &self,
        policy: CacheRetentionPolicy,
    ) -> Result<CacheMaintenanceReport> {
        let policy = policy.bounded();
        let _maintenance = match self.inner.maintenance.try_lock() {
            Ok(guard) => guard,
            Err(_) => {
                return Ok(busy_report(&self.inner.path));
            }
        };
        let mut writer = match self.inner.writer.try_lock() {
            Ok(guard) => guard,
            Err(_) => {
                return Ok(busy_report(&self.inner.path));
            }
        };
        let mut tx = writer.begin().await.map_err(storage_error)?;
        let usage_before = cache_usage_in(&mut tx, &self.inner.path).await?;
        let state = retention_state_in(&mut tx).await?;
        let mut indexed_facets = 0;
        let mut outcome = EvictionOutcome::default();
        if !state.index_complete {
            indexed_facets = index_historical_in(&mut tx, &state, policy.max_index_facets).await?;
        } else if state.indexed_logical_bytes > policy.target_logical_bytes {
            let (artifacts, sweep_complete) =
                super::pull_files::evict_artifacts_in(&mut tx, &policy).await?;
            outcome = artifacts;
            // A bounded artifact sweep always precedes summary eviction. If
            // there are more artifacts, the next maintenance turn resumes it.
            if sweep_complete
                && outcome.evicted_facets == 0
                && outcome.scanned_facets < policy.max_scan_facets
            {
                let state = retention_state_in(&mut tx).await?;
                let mut remaining_policy = policy.clone();
                remaining_policy.max_scan_facets -= outcome.scanned_facets;
                let summaries = evict_in(&mut tx, &state, &remaining_policy).await?;
                outcome.scanned_facets += summaries.scanned_facets;
                outcome.evicted_facets += summaries.evicted_facets;
                outcome.evicted_entry_rows += summaries.evicted_entry_rows;
                outcome.freed_logical_bytes += summaries.freed_logical_bytes;
            }
        }
        // Capture an authoritative post-mutation snapshot before commit. Once
        // commit succeeds, later checkpoint/reporting failures must not turn a
        // durable eviction into an all-or-nothing error for the caller.
        let committed_usage = cache_usage_in(&mut tx, &self.inner.path).await?;
        tx.commit().await.map_err(storage_error)?;
        drop(writer);

        let (checkpoint, checkpoint_error) = if policy.checkpoint_wal {
            match self.checkpoint_wal_passive_if_writer_idle().await {
                Ok(checkpoint) => (Some(checkpoint), None),
                Err(error) => (None, Some(error)),
            }
        } else {
            (None, None)
        };
        let (usage_after, usage_refresh_error) = match self.cache_usage().await {
            Ok(usage) => (usage, None),
            Err(error) => (committed_usage, Some(error)),
        };
        let target_met = usage_after
            .logical_bytes
            .is_some_and(|bytes| bytes <= policy.target_logical_bytes);
        Ok(CacheMaintenanceReport {
            usage_before,
            usage_after,
            indexed_facets,
            scanned_facets: outcome.scanned_facets,
            evicted_facets: outcome.evicted_facets,
            evicted_entry_rows: outcome.evicted_entry_rows,
            freed_logical_bytes: outcome.freed_logical_bytes,
            target_met,
            skipped_busy: false,
            checkpoint,
            checkpoint_error,
            usage_refresh_error,
        })
    }

    pub async fn wal_status(&self) -> Result<WalStatus> {
        let database_bytes = file_size(&self.inner.path);
        let wal_bytes = file_size(&sidecar_path(&self.inner.path, "-wal"));
        if !self.inner.noop_wal_checkpoint_supported {
            return Ok(WalStatus {
                supported: false,
                database_bytes,
                wal_bytes,
                busy: None,
                log_frames: None,
                checkpointed_frames: None,
            });
        }
        let mut connection = self.maintenance_connection().await?;
        let (busy, log_frames, checkpointed_frames) =
            wal_checkpoint(&mut connection, "NOOP").await?;
        Ok(WalStatus {
            supported: true,
            database_bytes,
            wal_bytes,
            busy,
            log_frames,
            checkpointed_frames,
        })
    }

    pub async fn checkpoint_wal_passive(&self) -> Result<WalCheckpointResult> {
        let _maintenance = match self.inner.maintenance.try_lock() {
            Ok(guard) => guard,
            Err(_) => return Ok(skipped_checkpoint()),
        };
        self.checkpoint_wal_passive_if_writer_idle().await
    }

    async fn checkpoint_wal_passive_if_writer_idle(&self) -> Result<WalCheckpointResult> {
        let writer = match self.inner.writer.try_lock() {
            Ok(guard) => guard,
            Err(_) => return Ok(skipped_checkpoint()),
        };
        drop(writer);
        self.checkpoint_wal_passive_inner().await
    }

    async fn checkpoint_wal_passive_inner(&self) -> Result<WalCheckpointResult> {
        let mut connection = self.maintenance_connection().await?;
        let (busy, log_frames, checkpointed_frames) =
            wal_checkpoint(&mut connection, "PASSIVE").await?;
        Ok(WalCheckpointResult {
            skipped_busy: false,
            busy,
            log_frames,
            checkpointed_frames,
        })
    }

    async fn maintenance_connection(&self) -> Result<SqliteConnection> {
        SqliteConnection::connect_with(
            &SqliteConnectOptions::new()
                .filename(&self.inner.path)
                .foreign_keys(true)
                .create_if_missing(false)
                .busy_timeout(Duration::ZERO)
                .statement_cache_capacity(8),
        )
        .await
        .map_err(storage_error)
    }
}

/// Refresh one accepted facet after all entry and metadata reconciliation.
pub(super) async fn refresh_detail_accounting_in(
    tx: &mut Transaction<'_, Sqlite>,
    account_id: &str,
    subject_id: &str,
    facet: &str,
) -> Result<()> {
    let row = sqlx::query(
        "SELECT o.facet_revision,octet_length(o.body_json)+octet_length(o.source_json)+coalesce(octet_length(o.value_source_json),0)+coalesce((SELECT sum(octet_length(e.json)) FROM detail_entries e WHERE e.account_id=o.account_id AND e.subject_id=o.subject_id AND e.facet=o.facet),0)+coalesce((SELECT octet_length(m.metadata_json)+octet_length(m.source_json) FROM detail_resource_metadata m WHERE m.account_id=o.account_id AND m.subject_id=o.subject_id AND m.facet=o.facet),0) AS logical_bytes FROM detail_observations o WHERE o.account_id=? AND o.subject_id=? AND o.facet=?",
    )
    .bind(account_id)
    .bind(subject_id)
    .bind(facet)
    .fetch_optional(&mut **tx)
    .await
    .map_err(storage_error)?;
    let Some(row) = row else {
        return Ok(());
    };
    let revision = revision_number(row.get::<&str, _>("facet_revision"))?;
    let logical_bytes: i64 = row.get("logical_bytes");
    if logical_bytes < 0 {
        return Err(CollaborationError::storage());
    }
    sqlx::query("INSERT INTO cache_retention_entries(account_id,subject_id,facet,logical_bytes,last_observed_revision) VALUES(?,?,?,?,?) ON CONFLICT(account_id,subject_id,facet) DO UPDATE SET logical_bytes=excluded.logical_bytes,last_observed_revision=excluded.last_observed_revision")
        .bind(account_id)
        .bind(subject_id)
        .bind(facet)
        .bind(logical_bytes)
        .bind(revision)
        .execute(&mut **tx)
        .await
        .map_err(storage_error)?;
    Ok(())
}

async fn cache_usage_in(tx: &mut Transaction<'_, Sqlite>, path: &Path) -> Result<CacheUsage> {
    let row = sqlx::query("SELECT index_complete,indexed_logical_bytes,indexed_facet_count FROM cache_retention_state WHERE singleton=1")
        .fetch_one(&mut **tx)
        .await
        .map_err(storage_error)?;
    let page_size: i64 = sqlx::query_scalar("PRAGMA main.page_size")
        .fetch_one(&mut **tx)
        .await
        .map_err(storage_error)?;
    let page_count: i64 = sqlx::query_scalar("PRAGMA main.page_count")
        .fetch_one(&mut **tx)
        .await
        .map_err(storage_error)?;
    let free_pages: i64 = sqlx::query_scalar("PRAGMA main.freelist_count")
        .fetch_one(&mut **tx)
        .await
        .map_err(storage_error)?;
    let index_complete = row.get::<i64, _>("index_complete") != 0;
    let indexed_logical_bytes = nonnegative_u64(row.get("indexed_logical_bytes"));
    Ok(CacheUsage {
        available: true,
        logical_bytes: index_complete.then_some(indexed_logical_bytes),
        indexed_logical_bytes,
        indexed_facets: nonnegative_u64(row.get("indexed_facet_count")),
        index_complete,
        database_bytes: file_size(path),
        wal_bytes: file_size(&sidecar_path(path, "-wal")),
        page_size: nonnegative_u64(page_size),
        page_count: nonnegative_u64(page_count),
        free_pages: nonnegative_u64(free_pages),
    })
}

async fn retention_state_in(tx: &mut Transaction<'_, Sqlite>) -> Result<RetentionState> {
    let row = sqlx::query("SELECT index_complete,index_cursor_account_id,index_cursor_subject_id,index_cursor_facet,eviction_cursor_revision,eviction_cursor_account_id,eviction_cursor_subject_id,eviction_cursor_facet,indexed_logical_bytes FROM cache_retention_state WHERE singleton=1")
        .fetch_one(&mut **tx)
        .await
        .map_err(storage_error)?;
    let index_account: Option<String> = row.get("index_cursor_account_id");
    let eviction_revision: Option<i64> = row.get("eviction_cursor_revision");
    Ok(RetentionState {
        index_complete: row.get::<i64, _>("index_complete") != 0,
        index_cursor: index_account.map(|account| {
            (
                account,
                row.get("index_cursor_subject_id"),
                row.get("index_cursor_facet"),
            )
        }),
        eviction_cursor: eviction_revision.map(|revision| {
            (
                revision,
                row.get("eviction_cursor_account_id"),
                row.get("eviction_cursor_subject_id"),
                row.get("eviction_cursor_facet"),
            )
        }),
        indexed_logical_bytes: nonnegative_u64(row.get("indexed_logical_bytes")),
    })
}

async fn index_historical_in(
    tx: &mut Transaction<'_, Sqlite>,
    state: &RetentionState,
    limit: u32,
) -> Result<u32> {
    let rows = if let Some((account, subject, facet)) = &state.index_cursor {
        sqlx::query("SELECT account_id,subject_id,facet FROM detail_observations WHERE (account_id,subject_id,facet)>(?,?,?) ORDER BY account_id,subject_id,facet LIMIT ?")
            .bind(account)
            .bind(subject)
            .bind(facet)
            .bind(i64::from(limit))
            .fetch_all(&mut **tx)
            .await
            .map_err(storage_error)?
    } else {
        sqlx::query("SELECT account_id,subject_id,facet FROM detail_observations ORDER BY account_id,subject_id,facet LIMIT ?")
            .bind(i64::from(limit))
            .fetch_all(&mut **tx)
            .await
            .map_err(storage_error)?
    };
    for row in &rows {
        refresh_detail_accounting_in(
            tx,
            row.get::<&str, _>("account_id"),
            row.get::<&str, _>("subject_id"),
            row.get::<&str, _>("facet"),
        )
        .await?;
    }
    if rows.len() < limit as usize {
        sqlx::query("UPDATE cache_retention_state SET index_complete=1,index_cursor_account_id=NULL,index_cursor_subject_id=NULL,index_cursor_facet=NULL WHERE singleton=1")
            .execute(&mut **tx)
            .await
            .map_err(storage_error)?;
    } else if let Some(last) = rows.last() {
        sqlx::query("UPDATE cache_retention_state SET index_cursor_account_id=?,index_cursor_subject_id=?,index_cursor_facet=? WHERE singleton=1")
            .bind(last.get::<&str, _>("account_id"))
            .bind(last.get::<&str, _>("subject_id"))
            .bind(last.get::<&str, _>("facet"))
            .execute(&mut **tx)
            .await
            .map_err(storage_error)?;
    }
    Ok(rows.len() as u32)
}

#[derive(Default)]
pub(super) struct EvictionOutcome {
    pub(super) scanned_facets: u32,
    pub(super) evicted_facets: u32,
    pub(super) evicted_entry_rows: u32,
    pub(super) freed_logical_bytes: u64,
}

async fn evict_in(
    tx: &mut Transaction<'_, Sqlite>,
    state: &RetentionState,
    policy: &CacheRetentionPolicy,
) -> Result<EvictionOutcome> {
    let rows = if let Some((revision, account, subject, facet)) = &state.eviction_cursor {
        // Keep each arm as an indexed range query so SQLite can merge the two
        // ordered streams. Wrapping the union in a subquery forces a temporary
        // B-tree and turns every resumed maintenance pass into a prefix scan.
        sqlx::query("SELECT account_id,subject_id,facet,logical_bytes,last_observed_revision FROM cache_retention_entries WHERE (last_observed_revision,account_id,subject_id,facet)>(?,?,?,?) UNION ALL SELECT account_id,subject_id,facet,logical_bytes,last_observed_revision FROM pull_commit_retention WHERE (last_observed_revision,account_id,subject_id,facet)>(?,?,?,?) UNION ALL SELECT account_id,subject_id,facet,logical_bytes,last_observed_revision FROM pull_file_retention WHERE (last_observed_revision,account_id,subject_id,facet)>(?,?,?,?) ORDER BY last_observed_revision,account_id,subject_id,facet LIMIT ?")
            .bind(revision)
            .bind(account)
            .bind(subject)
            .bind(facet)
            .bind(revision)
            .bind(account)
            .bind(subject)
            .bind(facet)
            .bind(revision)
            .bind(account)
            .bind(subject)
            .bind(facet)
            .bind(i64::from(policy.max_scan_facets))
            .fetch_all(&mut **tx)
            .await
            .map_err(storage_error)?
    } else {
        sqlx::query("SELECT account_id,subject_id,facet,logical_bytes,last_observed_revision FROM cache_retention_entries UNION ALL SELECT account_id,subject_id,facet,logical_bytes,last_observed_revision FROM pull_commit_retention UNION ALL SELECT account_id,subject_id,facet,logical_bytes,last_observed_revision FROM pull_file_retention ORDER BY last_observed_revision,account_id,subject_id,facet LIMIT ?")
            .bind(i64::from(policy.max_scan_facets))
            .fetch_all(&mut **tx)
            .await
            .map_err(storage_error)?
    };
    if rows.is_empty() {
        persist_eviction_cursor(tx, None).await?;
        return Ok(EvictionOutcome::default());
    }
    let candidates = rows
        .iter()
        .map(|row| RetentionCandidate {
            account_id: row.get("account_id"),
            subject_id: row.get("subject_id"),
            facet: row.get("facet"),
            logical_bytes: nonnegative_u64(row.get("logical_bytes")),
            revision: row.get("last_observed_revision"),
        })
        .collect::<Vec<_>>();
    let mut outcome = EvictionOutcome::default();
    let mut remaining_bytes = state.indexed_logical_bytes;
    let mut last_cursor = state.eviction_cursor.clone();
    let mut processed_all = true;
    for candidate in &candidates {
        if remaining_bytes <= policy.target_logical_bytes
            || outcome.evicted_facets >= policy.max_evict_facets
        {
            processed_all = false;
            break;
        }
        outcome.scanned_facets += 1;
        let scope = format!("detail:{}:{}", candidate.subject_id, candidate.facet);
        let eligible: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM accounts a JOIN items i ON i.account_id=a.id AND i.id=? AND i.kind IN ('pull_request','issue') JOIN account_instances ai ON ai.account_id=a.id JOIN resource_identities ri ON ri.account_id=a.id AND ri.instance_id=ai.instance_id AND ri.entity_id=i.id AND ri.kind=i.kind JOIN sync_scopes s ON s.account_id=a.id AND s.scope=? WHERE a.id=? AND json_extract(s.sync_json,'$.state')<>'syncing' AND NOT EXISTS(SELECT 1 FROM detail_demand f WHERE f.account_id=a.id AND f.subject_id=i.id AND f.facet='files' AND f.requested=1) AND NOT EXISTS(SELECT 1 FROM cache_pins p WHERE p.account_id=a.id AND p.instance_id=ai.instance_id AND p.entity_id=i.id) AND NOT EXISTS(SELECT 1 FROM detail_demand d WHERE d.account_id=a.id AND d.subject_id=i.id AND d.facet=? AND d.requested=1) AND NOT EXISTS(SELECT 1 FROM command_target_protections p WHERE p.account_id=a.id AND p.reference_id=i.id AND p.required=1 AND (p.reference_kind='entity' OR (p.reference_kind='facet' AND p.facet=?))))")
            .bind(&candidate.subject_id)
            .bind(&scope)
            .bind(&candidate.account_id)
            .bind(&candidate.facet)
            .bind(&candidate.facet)
            .fetch_one(&mut **tx)
            .await
            .map_err(storage_error)?;
        let eligible = eligible
            && (candidate.facet != "files"
                || !super::pull_files::protected_content_in(
                    tx,
                    &candidate.account_id,
                    &candidate.subject_id,
                )
                .await?);
        if eligible {
            let entry_rows: i64 = if candidate.facet == "files" {
                sqlx::query_scalar(
                    "SELECT count(*) FROM pull_file_rows WHERE account_id=? AND subject_id=?",
                )
                .bind(&candidate.account_id)
                .bind(&candidate.subject_id)
                .fetch_one(&mut **tx)
                .await
                .map_err(storage_error)?
            } else if candidate.facet == "commits" {
                sqlx::query_scalar(
                    "SELECT count(*) FROM pull_commit_rows WHERE account_id=? AND subject_id=?",
                )
                .bind(&candidate.account_id)
                .bind(&candidate.subject_id)
                .fetch_one(&mut **tx)
                .await
                .map_err(storage_error)?
            } else {
                sqlx::query_scalar("SELECT count(*) FROM detail_entries WHERE account_id=? AND subject_id=? AND facet=?")
                    .bind(&candidate.account_id)
                    .bind(&candidate.subject_id)
                    .bind(&candidate.facet)
                    .fetch_one(&mut **tx)
                    .await
                    .map_err(storage_error)?
            };
            let entry_rows =
                u32::try_from(entry_rows).map_err(|_| CollaborationError::storage())?;
            if entry_rows > policy.max_entry_rows {
                last_cursor = Some((
                    candidate.revision,
                    candidate.account_id.clone(),
                    candidate.subject_id.clone(),
                    candidate.facet.clone(),
                ));
                continue;
            }
            if outcome.evicted_entry_rows.saturating_add(entry_rows) > policy.max_entry_rows {
                processed_all = false;
                break;
            }
            let epoch: i64 =
                sqlx::query_scalar("SELECT authorization_epoch FROM accounts WHERE id=?")
                    .bind(&candidate.account_id)
                    .fetch_one(&mut **tx)
                    .await
                    .map_err(storage_error)?;
            let revision = record_change(tx, &candidate.account_id, epoch, &scope, false).await?;
            if candidate.facet == "files" {
                super::pull_files::evict_subject_in(
                    tx,
                    &candidate.account_id,
                    &candidate.subject_id,
                )
                .await?;
            } else if candidate.facet == "commits" {
                sqlx::query("UPDATE pull_commit_facets SET active_generation=NULL,facet_revision=NULL WHERE account_id=? AND subject_id=?")
                    .bind(&candidate.account_id).bind(&candidate.subject_id).execute(&mut **tx).await.map_err(storage_error)?;
                sqlx::query(
                    "DELETE FROM pull_commit_generations WHERE account_id=? AND subject_id=?",
                )
                .bind(&candidate.account_id)
                .bind(&candidate.subject_id)
                .execute(&mut **tx)
                .await
                .map_err(storage_error)?;
                sqlx::query("DELETE FROM pull_commit_facets WHERE account_id=? AND subject_id=?")
                    .bind(&candidate.account_id)
                    .bind(&candidate.subject_id)
                    .execute(&mut **tx)
                    .await
                    .map_err(storage_error)?;
                let result = sqlx::query(
                    "DELETE FROM pull_commit_retention WHERE account_id=? AND subject_id=?",
                )
                .bind(&candidate.account_id)
                .bind(&candidate.subject_id)
                .execute(&mut **tx)
                .await
                .map_err(storage_error)?;
                if result.rows_affected() != 1 {
                    return Err(CollaborationError::storage());
                }
            } else {
                let result = sqlx::query(
                    "DELETE FROM detail_observations WHERE account_id=? AND subject_id=? AND facet=?",
                )
                .bind(&candidate.account_id)
                .bind(&candidate.subject_id)
                .bind(&candidate.facet)
                .execute(&mut **tx)
                .await
                .map_err(storage_error)?;
                if result.rows_affected() != 1 {
                    return Err(CollaborationError::storage());
                }
            }
            let result = sqlx::query("UPDATE sync_scopes SET run_id=?,data_revision=?,completed_run_id=NULL,next_cursor=NULL,etag=NULL,last_modified=NULL,coverage_json=? WHERE account_id=? AND scope=?")
                .bind(Uuid::new_v4().to_string())
                .bind(revision_number(&revision)?)
                .bind(encode(&missing_coverage())?)
                .bind(&candidate.account_id)
                .bind(&scope)
                .execute(&mut **tx)
                .await
                .map_err(storage_error)?;
            if result.rows_affected() != 1 {
                return Err(CollaborationError::storage());
            }
            outcome.evicted_facets += 1;
            outcome.evicted_entry_rows += entry_rows;
            outcome.freed_logical_bytes = outcome
                .freed_logical_bytes
                .saturating_add(candidate.logical_bytes);
            remaining_bytes = remaining_bytes.saturating_sub(candidate.logical_bytes);
        }
        last_cursor = Some((
            candidate.revision,
            candidate.account_id.clone(),
            candidate.subject_id.clone(),
            candidate.facet.clone(),
        ));
    }
    if processed_all && candidates.len() < policy.max_scan_facets as usize {
        last_cursor = None;
    }
    persist_eviction_cursor(tx, last_cursor.as_ref()).await?;
    Ok(outcome)
}

async fn persist_eviction_cursor(
    tx: &mut Transaction<'_, Sqlite>,
    cursor: Option<&(i64, String, String, String)>,
) -> Result<()> {
    if let Some((revision, account, subject, facet)) = cursor {
        sqlx::query("UPDATE cache_retention_state SET eviction_cursor_revision=?,eviction_cursor_account_id=?,eviction_cursor_subject_id=?,eviction_cursor_facet=? WHERE singleton=1")
            .bind(revision)
            .bind(account)
            .bind(subject)
            .bind(facet)
            .execute(&mut **tx)
            .await
            .map_err(storage_error)?;
    } else {
        sqlx::query("UPDATE cache_retention_state SET eviction_cursor_revision=NULL,eviction_cursor_account_id=NULL,eviction_cursor_subject_id=NULL,eviction_cursor_facet=NULL WHERE singleton=1")
            .execute(&mut **tx)
            .await
            .map_err(storage_error)?;
    }
    Ok(())
}

async fn wal_checkpoint(
    connection: &mut SqliteConnection,
    mode: &str,
) -> Result<(Option<i64>, Option<i64>, Option<i64>)> {
    let sql = match mode {
        "NOOP" => "PRAGMA main.wal_checkpoint(NOOP)",
        "PASSIVE" => "PRAGMA main.wal_checkpoint(PASSIVE)",
        _ => return Err(CollaborationError::storage()),
    };
    let row = sqlx::query(sql)
        .fetch_one(connection)
        .await
        .map_err(storage_error)?;
    Ok((
        nonnegative(row.get(0)),
        nonnegative(row.get(1)),
        nonnegative(row.get(2)),
    ))
}

fn busy_report(path: &Path) -> CacheMaintenanceReport {
    let usage = CacheUsage {
        available: false,
        logical_bytes: None,
        indexed_logical_bytes: 0,
        indexed_facets: 0,
        index_complete: false,
        database_bytes: file_size(path),
        wal_bytes: file_size(&sidecar_path(path, "-wal")),
        page_size: 0,
        page_count: 0,
        free_pages: 0,
    };
    CacheMaintenanceReport {
        usage_before: usage.clone(),
        usage_after: usage,
        indexed_facets: 0,
        scanned_facets: 0,
        evicted_facets: 0,
        evicted_entry_rows: 0,
        freed_logical_bytes: 0,
        target_met: false,
        skipped_busy: true,
        checkpoint: None,
        checkpoint_error: None,
        usage_refresh_error: None,
    }
}

fn skipped_checkpoint() -> WalCheckpointResult {
    WalCheckpointResult {
        skipped_busy: true,
        busy: None,
        log_frames: None,
        checkpointed_frames: None,
    }
}

fn nonnegative(value: i64) -> Option<i64> {
    (value >= 0).then_some(value)
}

fn nonnegative_u64(value: i64) -> u64 {
    u64::try_from(value).unwrap_or_default()
}

fn file_size(path: &Path) -> u64 {
    std::fs::metadata(path)
        .map(|metadata| metadata.len())
        .unwrap_or(0)
}

fn sidecar_path(path: &Path, suffix: &str) -> PathBuf {
    let mut value = path.as_os_str().to_os_string();
    value.push(suffix);
    PathBuf::from(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn maintenance_and_checkpoint_skip_an_owned_application_writer() {
        let directory = tempfile::tempdir().unwrap();
        let store = Store::open(directory.path().join("cache.sqlite"))
            .await
            .unwrap();
        let _reader_a = store.inner.readers.acquire().await.unwrap();
        let _reader_b = store.inner.readers.acquire().await.unwrap();
        let _reader_c = store.inner.readers.acquire().await.unwrap();
        let writer = store.inner.writer.lock().await;
        let report = tokio::time::timeout(
            Duration::from_millis(500),
            store.run_cache_maintenance(CacheRetentionPolicy {
                checkpoint_wal: true,
                ..CacheRetentionPolicy::default()
            }),
        )
        .await
        .expect("Busy admission never waits for a saturated reader pool")
        .unwrap();
        assert!(report.skipped_busy);
        assert!(!report.usage_before.available);
        assert_eq!(report.indexed_facets, 0);
        assert_eq!(
            store.checkpoint_wal_passive().await.unwrap(),
            skipped_checkpoint()
        );
        drop(writer);
    }

    #[test]
    fn version_and_frame_guards_are_explicit() {
        assert!(!sqlite_version_at_least("3.50.7", (3, 51, 0)));
        assert!(sqlite_version_at_least("3.51.0", (3, 51, 0)));
        assert!(sqlite_version_at_least("3.51.3", (3, 51, 0)));
        assert!(!sqlite_version_at_least("3.51", (3, 51, 0)));
        assert_eq!(nonnegative(-1), None);
        assert_eq!(nonnegative(0), Some(0));
    }
}
