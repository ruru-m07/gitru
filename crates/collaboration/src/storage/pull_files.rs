//! Account/range fenced file generations and separate selected-diff artifacts.
use super::*;
use crate::{detail::*, pull_files::*, resource_metadata::*};

pub struct PullFileCommit {
    pub request: PullFileCollectionRequest,
    pub page: PullFileProviderPage,
    pub terminal_validation: Option<PullFileRangeValidation>,
    pub expected_file_count: Option<u32>,
    pub collection_cap: Option<PullFileCapEvidence>,
}
#[derive(Debug, Clone)]
pub struct PullFileApplyReceipt {
    pub revision: String,
    pub next_lease: Option<PullFileLease>,
    pub published: bool,
    pub row_count: u32,
}
#[derive(Debug, Clone)]
pub struct PullFileSelection {
    pub account: RemoteAccount,
    pub repository: RemoteRepository,
    pub subject: RemoteItem,
    pub binding: PullFileBinding,
    pub membership: PullFileMembershipReceipt,
    pub file: PullFile,
}
struct Captured {
    account: RemoteAccount,
    repository: RemoteRepository,
    subject: RemoteItem,
    binding: PullFileBinding,
    authorization_view: String,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct FileCursor {
    version: u32,
    account_id: String,
    authorization_epoch: String,
    authorization_view: String,
    subject_id: String,
    generation: String,
    facet_revision: String,
    context: PullFileContext,
    last_position: u32,
    last_key: String,
    depth: u32,
}
fn scope(subject: &str) -> String {
    format!("detail:{subject}:files")
}
fn invalid() -> CollaborationError {
    CollaborationError::invalid("Invalid or unbounded pull file observation")
}
fn missing_context() -> CollaborationError {
    CollaborationError::new(
        ErrorCode::NotFound,
        "Pull range metadata must be hydrated before loading files",
    )
}
async fn denied_in(tx: &mut Transaction<'_, Sqlite>, account: &str, subject: &str) -> Result<bool> {
    sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM sync_scopes WHERE account_id=? AND scope IN (?,?) AND access_denied=1)")
        .bind(account).bind(scope(subject)).bind(DetailFacet::Body.scope(subject)).fetch_one(&mut **tx).await.map_err(storage_error)
}
async fn capture_latest_in(
    tx: &mut Transaction<'_, Sqlite>,
    account_id: &str,
    subject_id: &str,
) -> Result<Captured> {
    validate_identifier(account_id)?;
    validate_identifier(subject_id)?;
    let account = account_in(tx, account_id, true).await?;
    let body_denied:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM sync_scopes WHERE account_id=? AND scope=? AND access_denied=1)")
        .bind(account_id).bind(DetailFacet::Body.scope(subject_id)).fetch_one(&mut **tx).await.map_err(storage_error)?;
    if body_denied {
        return Err(stale());
    }
    let subject = details::subject_in(tx, account_id, subject_id).await?;
    if subject.kind != RemoteItemKind::PullRequest {
        return Err(invalid());
    }
    let repository_id = subject.repository_id.as_ref().ok_or_else(missing_context)?;
    let repository_json: String =
        sqlx::query_scalar("SELECT json FROM repositories WHERE account_id=? AND id=?")
            .bind(account_id)
            .bind(repository_id)
            .fetch_optional(&mut **tx)
            .await
            .map_err(storage_error)?
            .ok_or_else(missing_context)?;
    let repository: RemoteRepository = decode(&repository_json)?;
    let metadata = super::resource_metadata::read_in(tx, &account, subject_id)
        .await?
        .ok_or_else(missing_context)?;
    let exact = |field| {
        metadata.fields.iter().any(|e| {
            e.field == field
                && e.saved_state == DetailValueState::Known
                && e.observed_state == DetailValueState::Known
                && e.source.is_some()
        })
    };
    if metadata.kind != RemoteItemKind::PullRequest
        || !exact(MetadataField::Head)
        || !exact(MetadataField::Base)
        || account.provider == ProviderKind::Gitlab
            && (!exact(MetadataField::MergeBase) || metadata.values.merge_base_oid.is_none())
    {
        return Err(missing_context());
    }
    let merge_base_oid = if exact(MetadataField::MergeBase) {
        metadata.values.merge_base_oid.clone()
    } else {
        None
    };
    let base = metadata.values.base.ok_or_else(missing_context)?;
    let head = metadata.values.head.ok_or_else(missing_context)?;
    if subject.head_oid.as_ref() != Some(&head.oid)
        || base
            .repository
            .as_ref()
            .is_some_and(|r| r.provider_id != repository.provider_id)
        || super::resource_metadata::head_conflicts_in(
            tx,
            &account,
            subject_id,
            subject.head_oid.as_deref(),
        )
        .await?
    {
        return Err(stale());
    }
    let body_revision:Option<String>=sqlx::query_scalar("SELECT facet_revision FROM detail_observations WHERE account_id=? AND subject_id=? AND facet='body' AND authorization_epoch=?")
        .bind(account_id).bind(subject_id).bind(&account.authorization_epoch).fetch_optional(&mut **tx).await.map_err(storage_error)?;
    let context = PullFileContext {
        base_oid: base.oid,
        head_oid: head.oid,
        merge_base_oid,
        base_repository_provider_id: repository.provider_id.clone(),
        source_repository_provider_id: head
            .repository
            .map(|r| r.provider_id)
            .ok_or_else(missing_context)?,
        body_metadata_facet_revision: body_revision.ok_or_else(missing_context)?,
    };
    context.validate()?;
    let instance = identities::instance_in(tx, &account).await?;
    let binding = PullFileBinding {
        instance_id: instance.id,
        repository_id: repository.id.clone(),
        repository_provider_id: repository.provider_id.clone(),
        pull_id: subject.id.clone(),
        pull_provider_id: subject.provider_id.clone(),
        number: subject.number.clone(),
        context,
    };
    binding.validate()?;
    Ok(Captured {
        account,
        repository,
        subject,
        binding,
        authorization_view: super::metadata(tx).await?.1,
    })
}
/// A Body validation revision is not itself a range change. Reuse the revision
/// at which the exact range became current only while an active/staging native
/// binding still witnesses that range. The Body commit hook retires both kinds
/// atomically when known facts or authorization change, including an ABA cycle.
async fn capture_in(
    tx: &mut Transaction<'_, Sqlite>,
    account: &str,
    subject: &str,
) -> Result<Captured> {
    let mut captured = capture_latest_in(tx, account, subject).await?;
    let saved:Option<String>=sqlx::query_scalar("SELECT current_context_json FROM pull_file_facets WHERE account_id=? AND subject_id=? AND authorization_epoch=? AND authorization_view=?")
        .bind(account).bind(subject).bind(&captured.account.authorization_epoch).bind(&captured.authorization_view).fetch_optional(&mut **tx).await.map_err(storage_error)?;
    if let Some(saved) = saved {
        let context: PullFileContext = decode(&saved)?;
        let mut candidate = captured.binding.clone();
        candidate.context.body_metadata_facet_revision =
            context.body_metadata_facet_revision.clone();
        if candidate.context == context {
            let witnessed:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM pull_file_generations WHERE account_id=? AND subject_id=? AND state IN ('active','staging') AND authorization_epoch=? AND authorization_view=? AND binding_json=?)")
                .bind(account).bind(subject).bind(&captured.account.authorization_epoch).bind(&captured.authorization_view).bind(encode(&candidate)?).fetch_one(&mut **tx).await.map_err(storage_error)?;
            if witnessed {
                captured.binding = candidate;
            }
        }
    }
    Ok(captured)
}

/// Called inside every successful Body transaction, after metadata reconciliation.
/// This makes omission and away-and-back changes durable fences even when no
/// file query ran between those Body observations.
pub(super) async fn body_observed_in(
    tx: &mut Transaction<'_, Sqlite>,
    account: &str,
    subject: &str,
    body_revision: &str,
) -> Result<()> {
    let facet=sqlx::query("SELECT authorization_epoch,authorization_view,current_context_json FROM pull_file_facets WHERE account_id=? AND subject_id=?")
        .bind(account).bind(subject).fetch_optional(&mut **tx).await.map_err(storage_error)?;
    let Some(facet) = facet else { return Ok(()) };
    let mut old_context: PullFileContext = decode(facet.get("current_context_json"))?;
    let account_state = account_in(tx, account, true).await?;
    let current_view = metadata(tx).await?.1;
    let current = match capture_in(tx, account, subject).await {
        Ok(captured) => Some(captured),
        Err(error) if matches!(error.code, ErrorCode::NotFound | ErrorCode::StaleView) => None,
        Err(error) => return Err(error),
    };
    if current.as_ref().is_some_and(|current| {
        current.binding.context == old_context
            && facet.get::<String, _>("authorization_epoch") == current.account.authorization_epoch
            && facet.get::<String, _>("authorization_view") == current.authorization_view
    }) {
        return Ok(());
    }
    old_context.body_metadata_facet_revision = body_revision.into();
    let context = current
        .map(|value| value.binding.context)
        .unwrap_or(old_context);
    invalidate_current_in(tx, &account_state, subject, context, &current_view).await
}

/// Summary head transitions fence a Files generation even if the provider moves
/// away and back before the next Body refresh or selected-artifact completion.
pub(super) async fn head_observed_in(
    tx: &mut Transaction<'_, Sqlite>,
    account: &RemoteAccount,
    subject: &str,
    head: Option<&str>,
) -> Result<()> {
    let context:Option<String>=sqlx::query_scalar("SELECT current_context_json FROM pull_file_facets f WHERE account_id=? AND subject_id=? AND EXISTS(SELECT 1 FROM pull_file_generations g WHERE g.account_id=f.account_id AND g.subject_id=f.subject_id AND g.state IN ('active','staging'))")
        .bind(&account.id).bind(subject).fetch_optional(&mut **tx).await.map_err(storage_error)?;
    let Some(context) = context else {
        return Ok(());
    };
    let context: PullFileContext = decode(&context)?;
    if Some(context.head_oid.as_str()) == head {
        return Ok(());
    };
    let authorization_view = metadata(tx).await?.1;
    invalidate_current_in(tx, account, subject, context, &authorization_view).await
}
async fn invalidate_current_in(
    tx: &mut Transaction<'_, Sqlite>,
    account_state: &RemoteAccount,
    subject: &str,
    context: PullFileContext,
    current_view: &str,
) -> Result<()> {
    let account = &account_state.id;
    sqlx::query("UPDATE pull_file_facets SET authorization_epoch=?,authorization_view=?,current_context_json=?,active_generation=NULL,facet_revision=NULL,stale_at=NULL WHERE account_id=? AND subject_id=?")
        .bind(&account_state.authorization_epoch).bind(current_view).bind(encode(&context)?).bind(account).bind(subject).execute(&mut **tx).await.map_err(storage_error)?;
    sqlx::query("UPDATE pull_file_generations SET state='superseded' WHERE account_id=? AND subject_id=? AND state='active'").bind(account).bind(subject).execute(&mut **tx).await.map_err(storage_error)?;
    sqlx::query(
        "DELETE FROM pull_file_generations WHERE account_id=? AND subject_id=? AND state='staging'",
    )
    .bind(account)
    .bind(subject)
    .execute(&mut **tx)
    .await
    .map_err(storage_error)?;
    sqlx::query("UPDATE sync_scopes SET run_id=?,next_cursor=NULL,coverage_json=?,sync_json=json_set(sync_json,'$.state','idle') WHERE account_id=? AND scope=?")
        .bind(Uuid::new_v4().to_string()).bind(encode(&missing_coverage())?).bind(account).bind(scope(subject)).execute(&mut **tx).await.map_err(storage_error)?;
    record_change(
        tx,
        account,
        positive_revision(&account_state.authorization_epoch)?,
        &scope(subject),
        false,
    )
    .await?;
    refresh_retention_in(tx, account, subject).await
}

async fn lease_in(
    tx: &mut Transaction<'_, Sqlite>,
    account: &str,
    subject: &str,
) -> Result<Option<PullFileLease>> {
    let row=sqlx::query("SELECT generation,run_id,authorization_epoch,authorization_view,binding_json,source_json,page_count,row_count,expected_cursor FROM pull_file_generations WHERE account_id=? AND subject_id=? AND state='staging'")
        .bind(account).bind(subject).fetch_optional(&mut **tx).await.map_err(storage_error)?;
    let Some(row) = row else { return Ok(None) };
    let generation: String = row.get("generation");
    let cursors:Vec<String>=sqlx::query_scalar("SELECT cursor FROM pull_file_cursors WHERE account_id=? AND subject_id=? AND generation=? ORDER BY ordinal")
        .bind(account).bind(subject).bind(&generation).fetch_all(&mut **tx).await.map_err(storage_error)?;
    let lease = PullFileLease {
        run_id: row.get("run_id"),
        generation,
        account_id: account.into(),
        authorization_epoch: row.get("authorization_epoch"),
        authorization_view: row.get("authorization_view"),
        binding: decode(row.get("binding_json"))?,
        source: decode(row.get("source_json"))?,
        provider_page_count: u32::try_from(row.get::<i64, _>("page_count"))
            .map_err(|_| CollaborationError::storage())?,
        accepted_row_count: u32::try_from(row.get::<i64, _>("row_count"))
            .map_err(|_| CollaborationError::storage())?,
        next_cursor: row.get("expected_cursor"),
        seen_cursors: cursors,
    };
    lease.validate()?;
    Ok(Some(lease))
}
fn request_from(captured: Captured, lease: PullFileLease) -> Result<PullFileCollectionRequest> {
    if captured.binding != lease.binding
        || captured.authorization_view != lease.authorization_view
        || captured.account.authorization_epoch != lease.authorization_epoch
    {
        return Err(stale());
    }
    let request = PullFileCollectionRequest {
        account: captured.account,
        authorization_view: captured.authorization_view,
        repository: captured.repository,
        subject: captured.subject,
        binding: captured.binding,
        source: lease.source.clone(),
        cursor: lease.next_cursor.clone(),
        start_position: lease.accepted_row_count,
        lease,
    };
    request.validate()?;
    Ok(request)
}
impl Store {
    pub async fn begin_pull_files(
        &self,
        account_id: &str,
        epoch: &str,
        subject_id: &str,
        source: PullFileSource,
    ) -> Result<PullFileLease> {
        source.validate()?;
        let mut writer = self.inner.writer.lock().await;
        let mut tx = writer.begin().await.map_err(storage_error)?;
        epoch_in(&mut tx, account_id, epoch).await?;
        let captured = capture_in(&mut tx, account_id, subject_id).await?;
        let lease = PullFileLease {
            run_id: Uuid::new_v4().to_string(),
            generation: Uuid::new_v4().to_string(),
            account_id: account_id.into(),
            authorization_epoch: epoch.into(),
            authorization_view: captured.authorization_view.clone(),
            binding: captured.binding.clone(),
            source,
            provider_page_count: 0,
            accepted_row_count: 0,
            next_cursor: None,
            seen_cursors: vec![],
        };
        request_from(captured, lease.clone())?;
        let old = scope_in(&mut tx, account_id, &scope(subject_id)).await?;
        let same:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM pull_file_facets WHERE account_id=? AND subject_id=? AND authorization_epoch=? AND authorization_view=? AND current_context_json=? AND active_generation IS NOT NULL)")
            .bind(account_id).bind(subject_id).bind(epoch).bind(&lease.authorization_view).bind(encode(&lease.binding.context)?).fetch_one(&mut *tx).await.map_err(storage_error)?;
        let same = same && !denied_in(&mut tx, account_id, subject_id).await?;
        if !same {
            sqlx::query("UPDATE pull_file_facets SET active_generation=NULL,facet_revision=NULL WHERE account_id=? AND subject_id=?").bind(account_id).bind(subject_id).execute(&mut *tx).await.map_err(storage_error)?;
            sqlx::query("UPDATE pull_file_generations SET state='superseded' WHERE account_id=? AND subject_id=? AND state='active'").bind(account_id).bind(subject_id).execute(&mut *tx).await.map_err(storage_error)?;
        }
        sqlx::query("DELETE FROM pull_file_generations WHERE account_id=? AND subject_id=? AND state='staging'").bind(account_id).bind(subject_id).execute(&mut *tx).await.map_err(storage_error)?;
        let revision = record_change(
            &mut tx,
            account_id,
            positive_revision(epoch)?,
            &scope(subject_id),
            false,
        )
        .await?;
        let mut sync = old.as_ref().map(|s| s.sync.clone()).unwrap_or_default();
        sync.state = SyncState::Syncing;
        sync.error = None;
        sync.next_retry_at = None;
        let coverage = if same {
            old.as_ref()
                .map(|s| s.coverage.clone())
                .unwrap_or_else(missing_coverage)
        } else {
            missing_coverage()
        };
        sqlx::query("INSERT INTO sync_scopes(account_id,scope,run_id,coverage_json,sync_json) VALUES(?,?,?,?,?) ON CONFLICT(account_id,scope) DO UPDATE SET run_id=excluded.run_id,next_cursor=NULL,etag=NULL,last_modified=NULL,coverage_json=excluded.coverage_json,sync_json=excluded.sync_json,access_denied=0")
            .bind(account_id).bind(scope(subject_id)).bind(&lease.run_id).bind(encode(&coverage)?).bind(encode(&sync)?).execute(&mut *tx).await.map_err(storage_error)?;
        sqlx::query("INSERT INTO pull_file_generations(account_id,subject_id,generation,run_id,authorization_epoch,authorization_view,binding_json,context_json,source_json,state,created_revision) VALUES(?,?,?,?,?,?,?,?,?,'staging',?)")
            .bind(account_id).bind(subject_id).bind(&lease.generation).bind(&lease.run_id).bind(epoch).bind(&lease.authorization_view).bind(encode(&lease.binding)?).bind(encode(&lease.binding.context)?).bind(encode(&lease.source)?).bind(positive_revision(&revision)?).execute(&mut *tx).await.map_err(storage_error)?;
        sqlx::query("INSERT INTO pull_file_facets(account_id,subject_id,authorization_epoch,authorization_view,current_context_json) VALUES(?,?,?,?,?) ON CONFLICT(account_id,subject_id) DO UPDATE SET authorization_epoch=excluded.authorization_epoch,authorization_view=excluded.authorization_view,current_context_json=excluded.current_context_json")
            .bind(account_id).bind(subject_id).bind(epoch).bind(&lease.authorization_view).bind(encode(&lease.binding.context)?).execute(&mut *tx).await.map_err(storage_error)?;
        refresh_retention_in(&mut tx, account_id, subject_id).await?;
        tx.commit().await.map_err(storage_error)?;
        Ok(lease)
    }
    pub async fn resume_pull_files(
        &self,
        account: &str,
        epoch: &str,
        subject: &str,
    ) -> Result<Option<PullFileLease>> {
        let mut tx = self.inner.readers.begin().await.map_err(storage_error)?;
        epoch_in(&mut tx, account, epoch).await?;
        if denied_in(&mut tx, account, subject).await? {
            return Err(stale());
        }
        let lease = lease_in(&mut tx, account, subject).await?;
        if let Some(lease) = &lease {
            request_from(capture_in(&mut tx, account, subject).await?, lease.clone())?;
        }
        tx.commit().await.map_err(storage_error)?;
        Ok(lease)
    }
    pub async fn pull_file_request(
        &self,
        lease: &PullFileLease,
    ) -> Result<PullFileCollectionRequest> {
        lease.validate()?;
        let mut tx = self.inner.readers.begin().await.map_err(storage_error)?;
        epoch_in(&mut tx, &lease.account_id, &lease.authorization_epoch).await?;
        if denied_in(&mut tx, &lease.account_id, &lease.binding.pull_id).await? {
            return Err(stale());
        }
        if lease_in(&mut tx, &lease.account_id, &lease.binding.pull_id)
            .await?
            .as_ref()
            != Some(lease)
        {
            return Err(stale());
        }
        let request = request_from(
            capture_in(&mut tx, &lease.account_id, &lease.binding.pull_id).await?,
            lease.clone(),
        )?;
        tx.commit().await.map_err(storage_error)?;
        Ok(request)
    }
}

impl Store {
    pub async fn apply_pull_files(&self, commit: PullFileCommit) -> Result<PullFileApplyReceipt> {
        commit.request.validate()?;
        let lease = &commit.request.lease;
        let account = &lease.account_id;
        let subject = &lease.binding.pull_id;
        let mut writer = self.inner.writer.lock().await;
        let mut tx = writer.begin().await.map_err(storage_error)?;
        epoch_in(&mut tx, account, &lease.authorization_epoch).await?;
        if denied_in(&mut tx, account, subject).await?
            || lease_in(&mut tx, account, subject).await?.as_ref() != Some(lease)
        {
            return Err(stale());
        }
        let native = request_from(capture_in(&mut tx, account, subject).await?, lease.clone())?;
        commit.page.validate_for(&native)?;
        let published = commit.page.next_cursor.is_none();
        let row_count = lease.accepted_row_count + commit.page.files.len() as u32;
        let cap = commit.page.cap.or(commit.collection_cap);
        if commit
            .page
            .cap
            .zip(commit.collection_cap)
            .is_some_and(|(a, b)| a != b)
            || lease.source.strategy == PullFileSourceStrategy::LocalExactRange
                && cap.is_some_and(|cap| cap.provenance == PullFileCapProvenance::Provider)
            || cap.is_some_and(|cap| !cap.is_valid())
            || !published
                && (commit.terminal_validation.is_some()
                    || commit.expected_file_count.is_some()
                    || commit.collection_cap.is_some())
        {
            return Err(invalid());
        }
        if published {
            let validation = commit.terminal_validation.as_ref().ok_or_else(invalid)?;
            if !lease.binding.context.matches_range_validation(validation) {
                return Err(stale());
            }
            if commit
                .expected_file_count
                .is_some_and(|count| row_count > count || cap.is_none() && row_count != count)
            {
                return Err(stale());
            }
        }
        for (offset, file) in commit.page.files.iter().enumerate() {
            let json = encode(file)?;
            if json.len() > 65_536 {
                return Err(invalid());
            }
            let key = format!(
                "file:{:x}",
                Sha256::digest(encode(&file.identity)?.as_bytes())
            );
            let duplicate:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM pull_file_rows WHERE account_id=? AND subject_id=? AND generation=? AND old_path=? AND new_path=?)")
                .bind(account).bind(subject).bind(&lease.generation).bind(file.identity.old_path.as_deref().unwrap_or("")).bind(file.identity.new_path.as_deref().unwrap_or("")).fetch_one(&mut *tx).await.map_err(storage_error)?;
            if duplicate {
                return Err(invalid());
            }
            sqlx::query("INSERT INTO pull_file_rows(account_id,subject_id,generation,file_key,position,old_path,new_path,summary_json) VALUES(?,?,?,?,?,?,?,?)")
                .bind(account).bind(subject).bind(&lease.generation).bind(key).bind(i64::from(lease.accepted_row_count)+offset as i64).bind(file.identity.old_path.as_deref().unwrap_or("")).bind(file.identity.new_path.as_deref().unwrap_or("")).bind(json).execute(&mut *tx).await.map_err(storage_error)?;
        }
        let next_lease = commit.page.next_lease(&native)?;
        if let Some(cursor) = &commit.page.next_cursor {
            sqlx::query("INSERT INTO pull_file_cursors(account_id,subject_id,generation,ordinal,cursor) VALUES(?,?,?,?,?)")
                .bind(account).bind(subject).bind(&lease.generation).bind(i64::from(lease.provider_page_count)).bind(cursor).execute(&mut *tx).await.map_err(storage_error)?;
        }
        sqlx::query("UPDATE pull_file_generations SET page_count=page_count+1,row_count=?,expected_cursor=? WHERE account_id=? AND subject_id=? AND generation=?")
            .bind(i64::from(row_count)).bind(&commit.page.next_cursor).bind(account).bind(subject).bind(&lease.generation).execute(&mut *tx).await.map_err(storage_error)?;
        let revision = record_change(
            &mut tx,
            account,
            positive_revision(&lease.authorization_epoch)?,
            &scope(subject),
            false,
        )
        .await?;
        if published {
            let completeness = cap
                .map(PullFileCompleteness::capped)
                .unwrap_or_else(PullFileCompleteness::complete);
            sqlx::query("UPDATE pull_file_facets SET active_generation=NULL,facet_revision=NULL WHERE account_id=? AND subject_id=?").bind(account).bind(subject).execute(&mut *tx).await.map_err(storage_error)?;
            sqlx::query("UPDATE pull_file_generations SET state='superseded' WHERE account_id=? AND subject_id=? AND state='active'").bind(account).bind(subject).execute(&mut *tx).await.map_err(storage_error)?;
            sqlx::query("UPDATE pull_file_generations SET state='active',completeness_json=? WHERE account_id=? AND subject_id=? AND generation=?").bind(encode(&completeness)?).bind(account).bind(subject).bind(&lease.generation).execute(&mut *tx).await.map_err(storage_error)?;
            let now = chrono::Utc::now();
            let stale_at = now
                + chrono::Duration::seconds(i64::from(
                    commit.page.freshness_seconds.clamp(1, 86_400),
                ));
            sqlx::query("UPDATE pull_file_facets SET active_generation=?,facet_revision=?,stale_at=? WHERE account_id=? AND subject_id=?")
                .bind(&lease.generation).bind(&revision).bind(stale_at.to_rfc3339()).bind(account).bind(subject).execute(&mut *tx).await.map_err(storage_error)?;
            let coverage = Coverage {
                state: if cap.is_none() {
                    CoverageState::Complete
                } else {
                    CoverageState::Partial
                },
                validated_at: Some(now.to_rfc3339()),
                remote_has_more: cap
                    .is_some_and(|c| c.remote_has_more != PullFileFlag::Known(false)),
            };
            let sync = SyncStatus {
                state: SyncState::Idle,
                last_success_at: Some(now.to_rfc3339()),
                next_retry_at: None,
                error: None,
            };
            sqlx::query("UPDATE sync_scopes SET completed_run_id=?,next_cursor=NULL,coverage_json=?,sync_json=?,data_revision=data_revision+1 WHERE account_id=? AND scope=? AND run_id=?")
                .bind(&lease.run_id).bind(encode(&coverage)?).bind(encode(&sync)?).bind(account).bind(scope(subject)).bind(&lease.run_id).execute(&mut *tx).await.map_err(storage_error)?;
            sqlx::query("UPDATE detail_demand SET requested=0 WHERE account_id=? AND subject_id=? AND facet='files' AND authorization_epoch=?").bind(account).bind(subject).bind(&lease.authorization_epoch).execute(&mut *tx).await.map_err(storage_error)?;
        } else {
            sqlx::query(
                "UPDATE sync_scopes SET next_cursor=?,sync_json=json_set(sync_json,'$.state','syncing') WHERE account_id=? AND scope=? AND run_id=?",
            )
            .bind(&commit.page.next_cursor)
            .bind(account)
            .bind(scope(subject))
            .bind(&lease.run_id)
            .execute(&mut *tx)
            .await
            .map_err(storage_error)?;
        }
        refresh_retention_in(&mut tx, account, subject).await?;
        tx.commit().await.map_err(storage_error)?;
        Ok(PullFileApplyReceipt {
            revision,
            next_lease,
            published,
            row_count,
        })
    }

    pub async fn pull_files(&self, query: PullFileQuery) -> Result<PullFileSnapshot> {
        query.validate()?;
        let mut tx = self.inner.readers.begin().await.map_err(storage_error)?;
        let account = account_in(&mut tx, &query.account_id, false).await?;
        let (revision, authorization_view) = metadata(&mut tx).await?;
        let (_, sync) =
            presentation(scope_in(&mut tx, &query.account_id, &scope(&query.subject_id)).await?);
        let mut snapshot = PullFileSnapshot {
            subject_id: query.subject_id.clone(),
            context: None,
            files: vec![],
            next_cursor: None,
            completeness: if sync.state == SyncState::Syncing {
                PullFileCompleteness::syncing()
            } else {
                PullFileCompleteness::missing()
            },
            coverage: missing_coverage(),
            sync,
            freshness: DetailFreshness::Unknown,
            facet_revision: None,
            revision,
            authorization_view,
        };
        if account.state != AccountState::Active {
            snapshot.sync.state = SyncState::AuthRequired;
            return Ok(snapshot);
        }
        if denied_in(&mut tx, &query.account_id, &query.subject_id).await? {
            snapshot.completeness = PullFileCompleteness::missing();
            return Ok(snapshot);
        }
        let captured = match capture_in(&mut tx, &query.account_id, &query.subject_id).await {
            Ok(value) => value,
            Err(error) if matches!(error.code, ErrorCode::NotFound | ErrorCode::StaleView) => {
                if query.cursor.is_some() {
                    return Err(stale());
                }
                return Ok(snapshot);
            }
            Err(error) => return Err(error),
        };
        snapshot.context = Some(captured.binding.context.clone());
        let active = active_in(&mut tx, &captured).await?;
        let Some(active) = active else {
            if query.cursor.is_some() {
                return Err(stale());
            }
            return Ok(snapshot);
        };
        let mut after = -1i64;
        let mut depth = 0;
        if let Some(cursor) = query.cursor {
            let cursor: FileCursor = serde_json::from_str(&cursor).map_err(|_| invalid())?;
            if cursor.version != 1
                || cursor.account_id != account.id
                || cursor.authorization_epoch != account.authorization_epoch
                || cursor.authorization_view != captured.authorization_view
                || cursor.subject_id != query.subject_id
                || cursor.generation != active.generation
                || cursor.facet_revision != active.revision
                || cursor.context != captured.binding.context
                || cursor.depth == 0
                || cursor.depth >= MAX_PULL_FILE_LOCAL_CURSOR_DEPTH
            {
                return Err(stale());
            }
            let exists:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM pull_file_rows WHERE account_id=? AND subject_id=? AND generation=? AND position=? AND file_key=?)")
                .bind(&account.id).bind(&query.subject_id).bind(&active.generation).bind(i64::from(cursor.last_position)).bind(&cursor.last_key).fetch_one(&mut *tx).await.map_err(storage_error)?;
            if !exists {
                return Err(stale());
            }
            after = i64::from(cursor.last_position);
            depth = cursor.depth;
        }
        let rows=sqlx::query("SELECT file_key,position,summary_json FROM pull_file_rows WHERE account_id=? AND subject_id=? AND generation=? AND position>? ORDER BY position LIMIT ?")
            .bind(&account.id).bind(&query.subject_id).bind(&active.generation).bind(after).bind(i64::from(query.limit)+1).fetch_all(&mut *tx).await.map_err(storage_error)?;
        // Reserve bounded snapshot/cursor metadata as well as serialized row bytes.
        let mut bytes = 16_384usize;
        let mut more = false;
        for row in rows {
            let file = row_file(&row, &captured.binding.context)?;
            let size = encode(&file)?.len();
            if snapshot.files.len() >= query.limit as usize
                || bytes + size > MAX_PULL_FILE_LOCAL_PAGE_BYTES
            {
                more = true;
                break;
            }
            bytes += size;
            snapshot.files.push(file);
        }
        if more {
            let last = snapshot.files.last().ok_or_else(invalid)?;
            let cursor = encode(&FileCursor {
                version: 1,
                account_id: account.id,
                authorization_epoch: account.authorization_epoch,
                authorization_view: captured.authorization_view,
                subject_id: query.subject_id,
                generation: active.generation,
                facet_revision: active.revision.clone(),
                context: captured.binding.context,
                last_position: last.provider_position,
                last_key: last.file_key.clone(),
                depth: depth + 1,
            })?;
            if cursor.len() > MAX_PULL_FILE_LOCAL_CURSOR_BYTES {
                return Err(invalid());
            }
            snapshot.next_cursor = Some(cursor);
        }
        snapshot.completeness = active.completeness;
        snapshot.facet_revision = Some(active.revision);
        snapshot.freshness = freshness(active.stale_at.as_deref());
        snapshot.coverage =
            presentation(scope_in(&mut tx, &query.account_id, &scope(&snapshot.subject_id)).await?)
                .0;
        tx.commit().await.map_err(storage_error)?;
        Ok(snapshot)
    }

    pub async fn pull_file_selection(
        &self,
        request: PullFileDiffRequest,
    ) -> Result<PullFileSelection> {
        let mut tx = self.inner.readers.begin().await.map_err(storage_error)?;
        let selection = selection_in(&mut tx, &request).await?;
        tx.commit().await.map_err(storage_error)?;
        Ok(selection)
    }
    pub async fn verify_pull_file_membership(
        &self,
        request: PullFileDiffRequest,
    ) -> Result<PullFileMembershipReceipt> {
        Ok(self.pull_file_selection(request).await?.membership)
    }
}

struct Active {
    generation: String,
    revision: String,
    completeness: PullFileCompleteness,
    stale_at: Option<String>,
}
async fn active_in(
    tx: &mut Transaction<'_, Sqlite>,
    captured: &Captured,
) -> Result<Option<Active>> {
    let row=sqlx::query("SELECT g.generation,g.completeness_json,f.facet_revision,f.stale_at FROM pull_file_facets f JOIN pull_file_generations g ON g.account_id=f.account_id AND g.subject_id=f.subject_id AND g.generation=f.active_generation WHERE f.account_id=? AND f.subject_id=? AND f.authorization_epoch=? AND f.authorization_view=? AND f.current_context_json=? AND g.authorization_epoch=f.authorization_epoch AND g.authorization_view=f.authorization_view AND g.context_json=f.current_context_json AND g.binding_json=? AND g.state='active' AND f.facet_revision IS NOT NULL")
        .bind(&captured.account.id).bind(&captured.subject.id).bind(&captured.account.authorization_epoch).bind(&captured.authorization_view).bind(encode(&captured.binding.context)?).bind(encode(&captured.binding)?).fetch_optional(&mut **tx).await.map_err(storage_error)?;
    let Some(row) = row else { return Ok(None) };
    let completeness: PullFileCompleteness = decode(row.get("completeness_json"))?;
    if !completeness.is_valid()
        || !matches!(
            completeness.state,
            PullFileCompletenessState::Complete
                | PullFileCompletenessState::Capped
                | PullFileCompletenessState::Partial
        )
    {
        return Err(CollaborationError::storage());
    }
    Ok(Some(Active {
        generation: row.get("generation"),
        revision: row.get("facet_revision"),
        completeness,
        stale_at: row.get("stale_at"),
    }))
}
fn row_file(row: &sqlx::sqlite::SqliteRow, context: &PullFileContext) -> Result<PullFile> {
    let file = PullFile {
        file_key: row.get("file_key"),
        context: context.clone(),
        provider_position: u32::try_from(row.get::<i64, _>("position"))
            .map_err(|_| CollaborationError::storage())?,
        file: decode(row.get("summary_json"))?,
    };
    file.validate()?;
    Ok(file)
}
fn freshness(stale_at: Option<&str>) -> DetailFreshness {
    if stale_at.is_some_and(|value| {
        chrono::DateTime::parse_from_rfc3339(value).is_ok_and(|value| value > chrono::Utc::now())
    }) {
        DetailFreshness::Fresh
    } else {
        DetailFreshness::Stale
    }
}
async fn selection_in(
    tx: &mut Transaction<'_, Sqlite>,
    request: &PullFileDiffRequest,
) -> Result<PullFileSelection> {
    request.validate()?;
    epoch_in(tx, &request.account_id, &request.authorization_epoch).await?;
    if denied_in(tx, &request.account_id, &request.subject_id).await? {
        return Err(stale());
    }
    let captured = capture_in(tx, &request.account_id, &request.subject_id).await?;
    if captured.binding.context != request.context {
        return Err(stale());
    }
    let active = active_in(tx, &captured).await?.ok_or_else(stale)?;
    if active.revision != request.file_facet_revision {
        return Err(stale());
    }
    let row=sqlx::query("SELECT file_key,position,summary_json FROM pull_file_rows WHERE account_id=? AND subject_id=? AND generation=? AND file_key=?")
        .bind(&request.account_id).bind(&request.subject_id).bind(&active.generation).bind(&request.file_key).fetch_optional(&mut **tx).await.map_err(storage_error)?.ok_or_else(stale)?;
    let file = row_file(&row, &request.context)?;
    let membership = PullFileMembershipReceipt {
        account_id: request.account_id.clone(),
        authorization_epoch: request.authorization_epoch.clone(),
        authorization_view: captured.authorization_view,
        generation: active.generation,
        subject_id: request.subject_id.clone(),
        file_facet_revision: active.revision,
        context: request.context.clone(),
        file_key: request.file_key.clone(),
        identity: file.file.identity.clone(),
    };
    membership.validate()?;
    Ok(PullFileSelection {
        account: captured.account,
        repository: captured.repository,
        subject: captured.subject,
        binding: captured.binding,
        membership,
        file,
    })
}

impl Store {
    pub async fn apply_pull_file_artifact(
        &self,
        request: PullFileDiffRequest,
        membership: PullFileMembershipReceipt,
        mut artifact: PullFileArtifact,
    ) -> Result<String> {
        if !artifact.is_exact_for(&request, &membership) {
            return Err(invalid());
        }
        let mut writer = self.inner.writer.lock().await;
        let mut tx = writer.begin().await.map_err(storage_error)?;
        let selected = selection_in(&mut tx, &request).await?;
        if selected.membership != membership {
            return Err(stale());
        }
        if artifact
            .source
            .as_ref()
            .and_then(|s| s.strategy.provider())
            .is_some_and(|p| p != selected.account.provider)
        {
            return Err(invalid());
        }
        if artifact.identity.old_path.is_none() && artifact.blob_references.old.is_some()
            || artifact.identity.new_path.is_none() && artifact.blob_references.new.is_some()
        {
            return Err(invalid());
        }
        let old_blobs = artifact_blob_ids_in(
            &mut tx,
            &request.account_id,
            &request.subject_id,
            &membership.generation,
            Some(&request.file_key),
        )
        .await?;
        let mut bytes = artifact.unified_text.as_ref().map_or(0, |s| s.len() as i64);
        for (reference, oid) in [
            (&artifact.blob_references.old, &artifact.old_blob_oid),
            (&artifact.blob_references.new, &artifact.new_blob_oid),
        ] {
            if let Some(reference) = reference {
                let blob=sqlx::query("SELECT oid,content_type,length(bytes) AS size FROM pull_file_blob_objects WHERE account_id=? AND blob_id=?")
                    .bind(&request.account_id).bind(reference).fetch_optional(&mut *tx).await.map_err(storage_error)?.ok_or_else(invalid)?;
                if oid
                    .as_ref()
                    .is_some_and(|oid| blob.get::<Option<String>, _>("oid").as_ref() != Some(oid))
                    || artifact
                        .content_type
                        .as_ref()
                        .is_some_and(|value| value != blob.get::<&str, _>("content_type"))
                {
                    return Err(invalid());
                }
                bytes += blob.get::<i64, _>("size");
            }
        }
        if bytes > MAX_PULL_FILE_TEXT_BYTES as i64
            || artifact.logical_bytes != bytes.to_string()
            || artifact.on_disk_bytes != bytes.to_string()
        {
            return Err(invalid());
        }
        let revision = record_change(
            &mut tx,
            &request.account_id,
            positive_revision(&request.authorization_epoch)?,
            &scope(&request.subject_id),
            false,
        )
        .await?;
        artifact.last_access_revision = revision.clone();
        let text = artifact.unified_text.take();
        let json = encode(&artifact)?;
        if json.len() > 65_536 {
            return Err(invalid());
        }
        sqlx::query("INSERT INTO pull_file_artifacts(account_id,subject_id,generation,file_key,metadata_json,unified_text,logical_bytes,last_access_revision) VALUES(?,?,?,?,?,?,?,?) ON CONFLICT(account_id,subject_id,generation,file_key) DO UPDATE SET metadata_json=excluded.metadata_json,unified_text=excluded.unified_text,logical_bytes=excluded.logical_bytes,last_access_revision=excluded.last_access_revision")
            .bind(&request.account_id).bind(&request.subject_id).bind(&membership.generation).bind(&request.file_key).bind(json).bind(text).bind(bytes).bind(positive_revision(&revision)?).execute(&mut *tx).await.map_err(storage_error)?;
        sqlx::query("DELETE FROM pull_file_blob_references WHERE account_id=? AND subject_id=? AND generation=? AND file_key=?")
            .bind(&request.account_id).bind(&request.subject_id).bind(&membership.generation).bind(&request.file_key).execute(&mut *tx).await.map_err(storage_error)?;
        for (side, reference) in [
            ("old", artifact.blob_references.old),
            ("new", artifact.blob_references.new),
        ] {
            if let Some(reference) = reference {
                sqlx::query("INSERT INTO pull_file_blob_references(account_id,subject_id,generation,file_key,side,blob_id) VALUES(?,?,?,?,?,?)")
                    .bind(&request.account_id).bind(&request.subject_id).bind(&membership.generation).bind(&request.file_key).bind(side).bind(reference).execute(&mut *tx).await.map_err(storage_error)?;
            }
        }
        cleanup_blob_ids_in(&mut tx, &request.account_id, &old_blobs).await?;
        refresh_retention_in(&mut tx, &request.account_id, &request.subject_id).await?;
        tx.commit().await.map_err(storage_error)?;
        Ok(revision)
    }
    pub async fn pull_file_artifact(
        &self,
        request: PullFileDiffRequest,
    ) -> Result<PullFileArtifactSnapshot> {
        let mut tx = self.inner.readers.begin().await.map_err(storage_error)?;
        let selection = selection_in(&mut tx, &request).await?;
        let (revision, authorization_view) = metadata(&mut tx).await?;
        let row=sqlx::query("SELECT metadata_json,unified_text FROM pull_file_artifacts WHERE account_id=? AND subject_id=? AND generation=? AND file_key=?")
            .bind(&request.account_id).bind(&request.subject_id).bind(&selection.membership.generation).bind(&request.file_key).fetch_optional(&mut *tx).await.map_err(storage_error)?;
        let artifact = if let Some(row) = row {
            let mut artifact: PullFileArtifact = decode(row.get("metadata_json"))?;
            artifact.unified_text = row.get("unified_text");
            if !artifact.is_exact_for(&request, &selection.membership) {
                return Err(stale());
            }
            Some(artifact)
        } else {
            None
        };
        let stale_at: Option<String> = sqlx::query_scalar(
            "SELECT stale_at FROM pull_file_facets WHERE account_id=? AND subject_id=?",
        )
        .bind(&request.account_id)
        .bind(&request.subject_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(storage_error)?
        .flatten();
        let freshness = if artifact.is_some() {
            freshness(stale_at.as_deref())
        } else {
            DetailFreshness::Unknown
        };
        tx.commit().await.map_err(storage_error)?;
        Ok(PullFileArtifactSnapshot {
            request,
            membership: selection.membership,
            artifact,
            revision,
            authorization_view,
            freshness,
        })
    }
}

pub(super) async fn refresh_retention_in(
    tx: &mut Transaction<'_, Sqlite>,
    account: &str,
    subject: &str,
) -> Result<()> {
    let logical_bytes:i64=sqlx::query_scalar("SELECT coalesce((SELECT sum(octet_length(binding_json)+octet_length(context_json)+octet_length(source_json)+coalesce(octet_length(expected_cursor),0)+coalesce(octet_length(completeness_json),0)) FROM pull_file_generations WHERE account_id=? AND subject_id=?),0)+coalesce((SELECT sum(octet_length(summary_json)+octet_length(file_key)+octet_length(old_path)+octet_length(new_path)) FROM pull_file_rows WHERE account_id=? AND subject_id=?),0)+coalesce((SELECT sum(octet_length(cursor)) FROM pull_file_cursors WHERE account_id=? AND subject_id=?),0)+coalesce((SELECT sum(octet_length(current_context_json)+coalesce(octet_length(active_generation),0)+coalesce(octet_length(facet_revision),0)+coalesce(octet_length(stale_at),0)) FROM pull_file_facets WHERE account_id=? AND subject_id=?),0)+coalesce((SELECT sum(octet_length(metadata_json)+logical_bytes) FROM pull_file_artifacts WHERE account_id=? AND subject_id=?),0)")
        .bind(account).bind(subject).bind(account).bind(subject).bind(account).bind(subject).bind(account).bind(subject).fetch_one(&mut **tx).await.map_err(storage_error)?;
    if logical_bytes == 0 {
        sqlx::query("DELETE FROM pull_file_retention WHERE account_id=? AND subject_id=?")
            .bind(account)
            .bind(subject)
            .execute(&mut **tx)
            .await
            .map_err(storage_error)?;
    } else {
        let revision: i64 =
            sqlx::query_scalar("SELECT max(revision,1) FROM runtime_meta WHERE singleton=1")
                .fetch_one(&mut **tx)
                .await
                .map_err(storage_error)?;
        sqlx::query("INSERT INTO pull_file_retention(account_id,subject_id,logical_bytes,last_observed_revision) VALUES(?,?,?,?) ON CONFLICT(account_id,subject_id) DO UPDATE SET logical_bytes=excluded.logical_bytes,last_observed_revision=excluded.last_observed_revision")
            .bind(account).bind(subject).bind(logical_bytes).bind(revision).execute(&mut **tx).await.map_err(storage_error)?;
    }
    Ok(())
}

pub(super) async fn cleanup_abandoned_in(connection: &mut SqliteConnection) -> Result<()> {
    let mut tx = connection.begin().await.map_err(storage_error)?;
    let mut after = (String::new(), String::new());
    loop {
        let rows=sqlx::query("SELECT account_id,subject_id FROM pull_file_generations WHERE state='staging' AND (account_id,subject_id)>(?,?) ORDER BY account_id,subject_id LIMIT 128")
            .bind(&after.0).bind(&after.1).fetch_all(&mut *tx).await.map_err(storage_error)?;
        if rows.is_empty() {
            break;
        }
        for row in rows {
            let account: String = row.get("account_id");
            let subject: String = row.get("subject_id");
            let lease = lease_in(&mut tx, &account, &subject)
                .await?
                .ok_or_else(CollaborationError::storage)?;
            let captured = capture_in(&mut tx, &account, &subject).await;
            let valid = match captured {
                Ok(captured) => {
                    request_from(captured, lease).is_ok()
                        && !denied_in(&mut tx, &account, &subject).await?
                }
                Err(error)
                    if matches!(
                        error.code,
                        ErrorCode::NotFound | ErrorCode::StaleView | ErrorCode::AuthRequired
                    ) =>
                {
                    false
                }
                Err(error) => return Err(error),
            };
            if !valid {
                sqlx::query("DELETE FROM pull_file_generations WHERE account_id=? AND subject_id=? AND state='staging'").bind(&account).bind(&subject).execute(&mut *tx).await.map_err(storage_error)?;
                sqlx::query("UPDATE sync_scopes SET run_id=?,next_cursor=NULL WHERE account_id=? AND scope=?").bind(Uuid::new_v4().to_string()).bind(&account).bind(scope(&subject)).execute(&mut *tx).await.map_err(storage_error)?;
                refresh_retention_in(&mut tx, &account, &subject).await?;
            }
            sqlx::query("UPDATE sync_scopes SET sync_json=json_set(sync_json,'$.state','idle') WHERE account_id=? AND scope=? AND json_extract(sync_json,'$.state')='syncing'").bind(&account).bind(scope(&subject)).execute(&mut *tx).await.map_err(storage_error)?;
            after = (account, subject);
        }
    }
    tx.commit().await.map_err(storage_error)
}

/// Required authored blob anchors prevent a summary cascade from deleting
/// the last reference to an authored base.
pub(super) async fn protected_content_in(
    tx: &mut Transaction<'_, Sqlite>,
    account: &str,
    subject: &str,
) -> Result<bool> {
    sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM pull_file_blob_references r JOIN command_target_protections p ON p.account_id=r.account_id AND p.reference_id=r.blob_id AND p.reference_kind='blob' AND p.required=1 WHERE r.account_id=? AND r.subject_id=?)")
        .bind(account).bind(subject).fetch_one(&mut **tx).await.map_err(storage_error)
}
pub(super) async fn evict_subject_in(
    tx: &mut Transaction<'_, Sqlite>,
    account: &str,
    subject: &str,
) -> Result<()> {
    let blobs:Vec<String>=sqlx::query_scalar("SELECT DISTINCT blob_id FROM pull_file_blob_references WHERE account_id=? AND subject_id=?")
        .bind(account).bind(subject).fetch_all(&mut **tx).await.map_err(storage_error)?;
    sqlx::query("UPDATE pull_file_facets SET active_generation=NULL,facet_revision=NULL WHERE account_id=? AND subject_id=?").bind(account).bind(subject).execute(&mut **tx).await.map_err(storage_error)?;
    sqlx::query("DELETE FROM pull_file_generations WHERE account_id=? AND subject_id=?")
        .bind(account)
        .bind(subject)
        .execute(&mut **tx)
        .await
        .map_err(storage_error)?;
    sqlx::query("DELETE FROM pull_file_facets WHERE account_id=? AND subject_id=?")
        .bind(account)
        .bind(subject)
        .execute(&mut **tx)
        .await
        .map_err(storage_error)?;
    cleanup_blob_ids_in(tx, account, &blobs).await?;
    refresh_retention_in(tx, account, subject).await
}

pub(super) async fn evict_artifacts_in(
    tx: &mut Transaction<'_, Sqlite>,
    policy: &retention::CacheRetentionPolicy,
) -> Result<(retention::EvictionOutcome, bool)> {
    let cursor=sqlx::query("SELECT last_access_revision,account_id,subject_id,generation,file_key FROM pull_file_artifact_retention_cursor WHERE singleton=1").fetch_one(&mut **tx).await.map_err(storage_error)?;
    let after: Option<i64> = cursor.get("last_access_revision");
    let rows = if let Some(after) = after {
        sqlx::query("SELECT account_id,subject_id,generation,file_key,last_access_revision,octet_length(metadata_json)+logical_bytes AS size FROM pull_file_artifacts WHERE (last_access_revision,account_id,subject_id,generation,file_key)>(?,?,?,?,?) ORDER BY last_access_revision,account_id,subject_id,generation,file_key LIMIT ?")
            .bind(after).bind(cursor.get::<&str,_>("account_id")).bind(cursor.get::<&str,_>("subject_id")).bind(cursor.get::<&str,_>("generation")).bind(cursor.get::<&str,_>("file_key")).bind(i64::from(policy.max_scan_facets)).fetch_all(&mut **tx).await.map_err(storage_error)?
    } else {
        sqlx::query("SELECT account_id,subject_id,generation,file_key,last_access_revision,octet_length(metadata_json)+logical_bytes AS size FROM pull_file_artifacts ORDER BY last_access_revision,account_id,subject_id,generation,file_key LIMIT ?")
            .bind(i64::from(policy.max_scan_facets)).fetch_all(&mut **tx).await.map_err(storage_error)?
    };
    let mut outcome = retention::EvictionOutcome::default();
    let mut processed_all = true;
    let mut remaining: i64 = sqlx::query_scalar(
        "SELECT indexed_logical_bytes FROM cache_retention_state WHERE singleton=1",
    )
    .fetch_one(&mut **tx)
    .await
    .map_err(storage_error)?;
    for row in &rows {
        if remaining.max(0) as u64 <= policy.target_logical_bytes
            || outcome.evicted_facets >= policy.max_evict_facets
            || outcome.evicted_entry_rows >= policy.max_entry_rows
        {
            processed_all = false;
            break;
        }
        outcome.scanned_facets += 1;
        let account: &str = row.get("account_id");
        let subject: &str = row.get("subject_id");
        let generation: &str = row.get("generation");
        let file: &str = row.get("file_key");
        let protected:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM cache_pins WHERE account_id=? AND entity_id=?) OR EXISTS(SELECT 1 FROM detail_demand WHERE account_id=? AND subject_id=? AND facet='files' AND requested=1) OR EXISTS(SELECT 1 FROM command_target_protections WHERE account_id=? AND reference_id=? AND required=1 AND (reference_kind='entity' OR(reference_kind='facet' AND facet='files'))) OR EXISTS(SELECT 1 FROM pull_file_blob_references r JOIN command_target_protections p ON p.account_id=r.account_id AND p.reference_id=r.blob_id AND p.reference_kind='blob' AND p.required=1 WHERE r.account_id=? AND r.subject_id=? AND r.generation=? AND r.file_key=?)")
            .bind(account).bind(subject).bind(account).bind(subject).bind(account).bind(subject).bind(account).bind(subject).bind(generation).bind(file).fetch_one(&mut **tx).await.map_err(storage_error)?;
        if !protected {
            let epoch: i64 =
                sqlx::query_scalar("SELECT authorization_epoch FROM accounts WHERE id=?")
                    .bind(account)
                    .fetch_one(&mut **tx)
                    .await
                    .map_err(storage_error)?;
            record_change(tx, account, epoch, &scope(subject), false).await?;
            let blobs = artifact_blob_ids_in(tx, account, subject, generation, Some(file)).await?;
            sqlx::query("DELETE FROM pull_file_artifacts WHERE account_id=? AND subject_id=? AND generation=? AND file_key=?").bind(account).bind(subject).bind(generation).bind(file).execute(&mut **tx).await.map_err(storage_error)?;
            cleanup_blob_ids_in(tx, account, &blobs).await?;
            refresh_retention_in(tx, account, subject).await?;
            let size: i64 = row.get("size");
            remaining = remaining.saturating_sub(size);
            outcome.freed_logical_bytes += size as u64;
            outcome.evicted_facets += 1;
            outcome.evicted_entry_rows += 1;
        }
        sqlx::query("UPDATE pull_file_artifact_retention_cursor SET last_access_revision=?,account_id=?,subject_id=?,generation=?,file_key=? WHERE singleton=1")
            .bind(row.get::<i64,_>("last_access_revision")).bind(account).bind(subject).bind(generation).bind(file).execute(&mut **tx).await.map_err(storage_error)?;
    }
    let complete = processed_all && rows.len() < policy.max_scan_facets as usize;
    if complete {
        sqlx::query("UPDATE pull_file_artifact_retention_cursor SET last_access_revision=NULL,account_id=NULL,subject_id=NULL,generation=NULL,file_key=NULL WHERE singleton=1").execute(&mut **tx).await.map_err(storage_error)?;
    }
    Ok((outcome, complete))
}

#[allow(dead_code)] // Consumed by the separately integrated Files capability slice.
pub(super) struct PullFileCapabilityEvidence {
    pub completeness: PullFileCompleteness,
    pub row_count: i64,
    pub denied: bool,
    pub sync: SyncStatus,
}
/// Capability resolution uses the same current account/body/range authority as
/// list reads, never just the presence of a stale cached generation.
#[allow(dead_code)] // Consumed by the separately integrated Files capability slice.
pub(super) async fn capability_evidence_in(
    tx: &mut Transaction<'_, Sqlite>,
    account: &RemoteAccount,
    subject_id: &str,
) -> Result<PullFileCapabilityEvidence> {
    let (_, sync) = presentation(scope_in(tx, &account.id, &scope(subject_id)).await?);
    let denied = denied_in(tx, &account.id, subject_id).await?;
    let missing = || PullFileCapabilityEvidence {
        completeness: if sync.state == SyncState::Syncing {
            PullFileCompleteness::syncing()
        } else {
            PullFileCompleteness::missing()
        },
        row_count: 0,
        denied,
        sync: sync.clone(),
    };
    if denied || account.state != AccountState::Active {
        return Ok(missing());
    }
    let captured = match capture_in(tx, &account.id, subject_id).await {
        Ok(captured) => captured,
        Err(error) if matches!(error.code, ErrorCode::NotFound | ErrorCode::StaleView) => {
            return Ok(missing());
        }
        Err(error) => return Err(error),
    };
    let Some(active) = active_in(tx, &captured).await? else {
        return Ok(missing());
    };
    let row_count:i64=sqlx::query_scalar("SELECT row_count FROM pull_file_generations WHERE account_id=? AND subject_id=? AND generation=?")
        .bind(&account.id).bind(subject_id).bind(&active.generation).fetch_one(&mut **tx).await.map_err(storage_error)?;
    Ok(PullFileCapabilityEvidence {
        completeness: active.completeness,
        row_count,
        denied: false,
        sync,
    })
}

/// Remove at most one bounded, unpublished historical generation. Keeping this
/// distinct from whole-subject eviction prevents two full 3,000-file runs from
/// exceeding the maintenance row budget forever.
pub(super) async fn evict_superseded_in(
    tx: &mut Transaction<'_, Sqlite>,
    account: &str,
    subject: &str,
    max_rows: u32,
) -> Result<Option<(u32, u64)>> {
    let row=sqlx::query("SELECT generation,row_count FROM pull_file_generations WHERE state='superseded' AND account_id=? AND subject_id=? ORDER BY generation LIMIT 1")
        .bind(account).bind(subject).fetch_optional(&mut **tx).await.map_err(storage_error)?;
    let Some(row) = row else { return Ok(None) };
    let count =
        u32::try_from(row.get::<i64, _>("row_count")).map_err(|_| CollaborationError::storage())?;
    if count > max_rows {
        return Ok(None);
    }
    let generation: &str = row.get("generation");
    let before: i64 = sqlx::query_scalar(
        "SELECT logical_bytes FROM pull_file_retention WHERE account_id=? AND subject_id=?",
    )
    .bind(account)
    .bind(subject)
    .fetch_one(&mut **tx)
    .await
    .map_err(storage_error)?;
    let blobs = artifact_blob_ids_in(tx, account, subject, generation, None).await?;
    let epoch: i64 = sqlx::query_scalar("SELECT authorization_epoch FROM accounts WHERE id=?")
        .bind(account)
        .fetch_one(&mut **tx)
        .await
        .map_err(storage_error)?;
    record_change(tx, account, epoch, &scope(subject), false).await?;
    sqlx::query("DELETE FROM pull_file_generations WHERE account_id=? AND subject_id=? AND generation=? AND state='superseded'").bind(account).bind(subject).bind(generation).execute(&mut **tx).await.map_err(storage_error)?;
    cleanup_blob_ids_in(tx, account, &blobs).await?;
    refresh_retention_in(tx, account, subject).await?;
    let after: i64 = sqlx::query_scalar(
        "SELECT logical_bytes FROM pull_file_retention WHERE account_id=? AND subject_id=?",
    )
    .bind(account)
    .bind(subject)
    .fetch_one(&mut **tx)
    .await
    .map_err(storage_error)?;
    Ok(Some((
        count,
        u64::try_from(before - after).map_err(|_| CollaborationError::storage())?,
    )))
}
async fn artifact_blob_ids_in(
    tx: &mut Transaction<'_, Sqlite>,
    account: &str,
    subject: &str,
    generation: &str,
    file: Option<&str>,
) -> Result<Vec<String>> {
    if let Some(file) = file {
        sqlx::query_scalar("SELECT blob_id FROM pull_file_blob_references WHERE account_id=? AND subject_id=? AND generation=? AND file_key=?")
            .bind(account).bind(subject).bind(generation).bind(file).fetch_all(&mut **tx).await.map_err(storage_error)
    } else {
        sqlx::query_scalar("SELECT DISTINCT blob_id FROM pull_file_blob_references WHERE account_id=? AND subject_id=? AND generation=?")
            .bind(account).bind(subject).bind(generation).fetch_all(&mut **tx).await.map_err(storage_error)
    }
}
async fn cleanup_blob_ids_in(
    tx: &mut Transaction<'_, Sqlite>,
    account: &str,
    blobs: &[String],
) -> Result<()> {
    for blob in blobs {
        sqlx::query("DELETE FROM pull_file_blob_objects WHERE account_id=? AND blob_id=? AND NOT EXISTS(SELECT 1 FROM pull_file_blob_references r WHERE r.account_id=pull_file_blob_objects.account_id AND r.blob_id=pull_file_blob_objects.blob_id) AND NOT EXISTS(SELECT 1 FROM command_target_protections p WHERE p.account_id=pull_file_blob_objects.account_id AND p.reference_id=pull_file_blob_objects.blob_id AND p.reference_kind='blob' AND p.required=1)")
            .bind(account).bind(blob).execute(&mut **tx).await.map_err(storage_error)?;
    }
    Ok(())
}
