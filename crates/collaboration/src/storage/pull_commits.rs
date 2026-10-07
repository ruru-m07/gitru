//! Exact-context, generation-based pull-request commit persistence.
use super::*;
use crate::{
    DetailFacet, DetailFreshness, DetailValueState, MetadataField, PullCommitMessageState,
    pull_commits::*,
};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

const MAX_CURSOR_BYTES: usize = 8_192;
const MAX_LOCAL_CURSOR_BYTES: usize = 4_096;
const MAX_SUMMARY_BYTES: usize = 4_096;
const MAX_SOURCE_BYTES: usize = 256;

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PullCommitCursor {
    version: u32,
    account_id: String,
    authorization_view: String,
    subject_id: String,
    generation: String,
    facet_revision: String,
    context: PullCommitContext,
    last_position: u32,
    last_oid: String,
}

struct CapturedPull {
    instance: ProviderInstance,
    binding: PullCommitBinding,
}

fn invalid_commits() -> CollaborationError {
    CollaborationError::invalid("Invalid or unbounded pull commit observation")
}

fn missing_context() -> CollaborationError {
    CollaborationError::new(
        ErrorCode::NotFound,
        "Pull request range metadata must be hydrated before loading commits",
    )
}

fn valid_timestamp(value: &str) -> bool {
    value.len() <= 128 && chrono::DateTime::parse_from_rfc3339(value).is_ok()
}

fn valid_url(value: &str) -> bool {
    if value.len() > 2_048 || value.trim() != value {
        return false;
    }
    url::Url::parse(value).is_ok_and(|url| {
        url.scheme() == "https"
            && url.host_str().is_some()
            && url.username().is_empty()
            && url.password().is_none()
    })
}

fn url_in_instance(base_url: &str, value: &str) -> bool {
    url::Url::parse(base_url)
        .ok()
        .zip(url::Url::parse(value).ok())
        .is_some_and(|(base, value)| {
            let base_path = base.path().trim_end_matches('/');
            base.origin() == value.origin()
                && (base_path.is_empty()
                    || value.path() == base_path
                    || value
                        .path()
                        .strip_prefix(base_path)
                        .is_some_and(|suffix| suffix.starts_with('/')))
        })
}

fn valid_actor(actor: &PullCommitActor) -> bool {
    !actor.name.is_empty()
        && actor.name.len() <= 1_024
        && !actor.name.chars().any(char::is_control)
        && actor.provider.as_ref().is_none_or(|provider| {
            !provider.provider_id.is_empty()
                && provider.provider_id.len() <= 256
                && provider.login.len() <= 255
                && !provider.provider_id.chars().any(char::is_control)
                && !provider.login.chars().any(char::is_control)
                && provider.web_url.as_deref().is_none_or(valid_url)
        })
}

fn valid_context(context: &PullCommitContext) -> bool {
    is_canonical_commit_oid(&context.base_oid)
        && is_canonical_commit_oid(&context.head_oid)
        && !context.source_repository_provider_id.is_empty()
        && context.source_repository_provider_id.len() <= 512
        && !context
            .source_repository_provider_id
            .chars()
            .any(char::is_control)
        && positive_revision(&context.metadata_facet_revision).is_ok()
}

fn normalize_commit(mut commit: ProviderPullCommit) -> Result<ProviderPullCommit> {
    if !is_canonical_commit_oid(&commit.oid)
        || commit.summary.len() > MAX_SUMMARY_BYTES
        || !valid_actor(&commit.author)
        || commit
            .committer
            .as_ref()
            .is_some_and(|actor| !valid_actor(actor))
        || commit.parent_oids.len() > MAX_PULL_COMMIT_PARENTS
        || commit
            .parent_oids
            .iter()
            .any(|oid| !is_canonical_commit_oid(oid))
        || commit
            .authored_at
            .as_deref()
            .is_some_and(|value| !valid_timestamp(value))
        || commit
            .committed_at
            .as_deref()
            .is_some_and(|value| !valid_timestamp(value))
        || commit.web_url.as_deref().is_some_and(|url| !valid_url(url))
    {
        return Err(invalid_commits());
    }
    match commit.message.state {
        PullCommitMessageState::Known => {
            let Some(text) = commit.message.text.as_ref() else {
                return Err(invalid_commits());
            };
            if text.len() > MAX_PULL_COMMIT_MESSAGE_BYTES {
                commit.message.state = PullCommitMessageState::Oversized;
                commit.message.text = None;
            }
        }
        PullCommitMessageState::Omitted | PullCommitMessageState::Oversized => {
            if commit.message.text.is_some() {
                return Err(invalid_commits());
            }
        }
    }
    Ok(commit)
}

fn row_to_commit(row: &sqlx::sqlite::SqliteRow) -> Result<PullCommit> {
    let value: ProviderPullCommit = decode(row.get("json"))?;
    let position =
        u32::try_from(row.get::<i64, _>("position")).map_err(|_| CollaborationError::storage())?;
    Ok(PullCommit {
        oid: value.oid,
        position,
        summary: value.summary,
        message: value.message,
        author: value.author,
        committer: value.committer,
        authored_at: value.authored_at,
        committed_at: value.committed_at,
        parent_oids: value.parent_oids,
        web_url: value.web_url,
    })
}

async fn capture_in(
    tx: &mut Transaction<'_, Sqlite>,
    account_id: &str,
    require_active: bool,
) -> Result<RemoteAccount> {
    account_in(tx, account_id, require_active).await
}

async fn capture_pull_in(
    tx: &mut Transaction<'_, Sqlite>,
    account_id: &str,
    subject_id: &str,
    require_active: bool,
) -> Result<CapturedPull> {
    let account = capture_in(tx, account_id, require_active).await?;
    let subject = details::subject_in(tx, account_id, subject_id).await?;
    if subject.kind != RemoteItemKind::PullRequest {
        return Err(invalid_commits());
    }
    let repository_id = subject.repository_id.clone().ok_or_else(missing_context)?;
    let repository_provider_id: Option<String> =
        sqlx::query_scalar("SELECT provider_id FROM repositories WHERE account_id=? AND id=?")
            .bind(account_id)
            .bind(&repository_id)
            .fetch_optional(&mut **tx)
            .await
            .map_err(storage_error)?;
    let repository_provider_id = repository_provider_id.ok_or_else(missing_context)?;
    let metadata = super::resource_metadata::read_in(tx, &account, subject_id)
        .await?
        .ok_or_else(missing_context)?;
    if metadata.kind != RemoteItemKind::PullRequest {
        return Err(invalid_commits());
    }
    let exact = |field| {
        metadata.fields.iter().any(|evidence| {
            evidence.field == field
                && evidence.saved_state == DetailValueState::Known
                && evidence.observed_state == DetailValueState::Known
                && evidence.source.is_some()
        })
    };
    if !exact(MetadataField::Base) || !exact(MetadataField::Head) {
        return Err(missing_context());
    }
    let base = metadata.values.base.ok_or_else(missing_context)?;
    let head = metadata.values.head.ok_or_else(missing_context)?;
    if base
        .repository
        .as_ref()
        .is_some_and(|repository| repository.provider_id != repository_provider_id)
        || subject.head_oid.as_ref() != Some(&head.oid)
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
    let source_repository_provider_id = head
        .repository
        .as_ref()
        .map(|repository| repository.provider_id.clone())
        .filter(|value| !value.is_empty())
        .ok_or_else(missing_context)?;
    let metadata_facet_revision: Option<String> = sqlx::query_scalar(
        "SELECT facet_revision FROM detail_observations WHERE account_id=? AND subject_id=? AND facet='body' AND authorization_epoch=?",
    )
    .bind(account_id)
    .bind(subject_id)
    .bind(&account.authorization_epoch)
    .fetch_optional(&mut **tx)
    .await
    .map_err(storage_error)?;
    let context = PullCommitContext {
        base_oid: base.oid,
        head_oid: head.oid,
        source_repository_provider_id,
        metadata_facet_revision: metadata_facet_revision.ok_or_else(missing_context)?,
    };
    if !valid_context(&context) {
        return Err(invalid_commits());
    }
    let binding = PullCommitBinding {
        repository_id,
        repository_provider_id,
        provider_id: subject.provider_id.clone(),
        number: subject.number.clone(),
        context,
    };
    Ok(CapturedPull {
        instance: identities::instance_in(tx, &account).await?,
        binding,
    })
}

pub(super) async fn check_context_in(
    tx: &mut Transaction<'_, Sqlite>,
    account_id: &str,
    subject_id: &str,
    require_active: bool,
) -> Result<crate::CheckContext> {
    let captured = capture_pull_in(tx, account_id, subject_id, require_active).await?;
    Ok(crate::CheckContext {
        head_oid: captured.binding.context.head_oid,
        source_repository_provider_id: captured.binding.context.source_repository_provider_id,
        metadata_facet_revision: captured.binding.context.metadata_facet_revision,
    })
}

pub(super) async fn review_context_in(
    tx: &mut Transaction<'_, Sqlite>,
    account_id: &str,
    subject_id: &str,
    require_active: bool,
) -> Result<crate::ReviewContext> {
    let captured = capture_pull_in(tx, account_id, subject_id, require_active).await?;
    Ok(crate::ReviewContext {
        base_oid: captured.binding.context.base_oid,
        head_oid: captured.binding.context.head_oid,
        base_repository_provider_id: captured.binding.repository_provider_id,
        source_repository_provider_id: captured.binding.context.source_repository_provider_id,
        metadata_facet_revision: captured.binding.context.metadata_facet_revision,
    })
}

async fn refresh_retention_in(
    tx: &mut Transaction<'_, Sqlite>,
    account_id: &str,
    subject_id: &str,
) -> Result<()> {
    let logical_bytes: i64 = sqlx::query_scalar(
        "SELECT coalesce((SELECT sum(octet_length(binding_json)+octet_length(context_json)+coalesce(octet_length(source_json),0)+coalesce(octet_length(expected_cursor),0)+coalesce(octet_length(completeness_json),0)) FROM pull_commit_generations WHERE account_id=? AND subject_id=?),0)+coalesce((SELECT sum(octet_length(json)+octet_length(oid)) FROM pull_commit_rows WHERE account_id=? AND subject_id=?),0)+coalesce((SELECT sum(octet_length(cursor)) FROM pull_commit_cursors WHERE account_id=? AND subject_id=?),0)+coalesce((SELECT sum(octet_length(current_context_json)+coalesce(octet_length(active_generation),0)+coalesce(octet_length(facet_revision),0)+coalesce(octet_length(stale_at),0)) FROM pull_commit_facets WHERE account_id=? AND subject_id=?),0)",
    )
    .bind(account_id)
    .bind(subject_id)
    .bind(account_id)
    .bind(subject_id)
    .bind(account_id)
    .bind(subject_id)
    .bind(account_id)
    .bind(subject_id)
    .fetch_one(&mut **tx)
    .await
    .map_err(storage_error)?;
    if logical_bytes == 0 {
        sqlx::query("DELETE FROM pull_commit_retention WHERE account_id=? AND subject_id=?")
            .bind(account_id)
            .bind(subject_id)
            .execute(&mut **tx)
            .await
            .map_err(storage_error)?;
        return Ok(());
    }
    let revision: i64 =
        sqlx::query_scalar("SELECT max(revision,1) FROM runtime_meta WHERE singleton=1")
            .fetch_one(&mut **tx)
            .await
            .map_err(storage_error)?;
    sqlx::query("INSERT INTO pull_commit_retention(account_id,subject_id,logical_bytes,last_observed_revision) VALUES(?,?,?,?) ON CONFLICT(account_id,subject_id) DO UPDATE SET logical_bytes=excluded.logical_bytes,last_observed_revision=excluded.last_observed_revision")
        .bind(account_id).bind(subject_id).bind(logical_bytes).bind(revision).execute(&mut **tx).await.map_err(storage_error)?;
    Ok(())
}

async fn pull_commit_scope_denied_in(
    tx: &mut Transaction<'_, Sqlite>,
    account_id: &str,
    subject_id: &str,
) -> Result<bool> {
    let denied: Option<bool> =
        sqlx::query_scalar("SELECT access_denied FROM sync_scopes WHERE account_id=? AND scope=?")
            .bind(account_id)
            .bind(DetailFacet::Commits.scope(subject_id))
            .fetch_optional(&mut **tx)
            .await
            .map_err(storage_error)?;
    Ok(denied.unwrap_or(false))
}

pub(super) struct PullCommitCapabilityEvidence {
    pub completeness: PullCommitCompleteness,
    pub row_count: i64,
    pub denied: bool,
    pub sync: SyncStatus,
}

/// Read the renderer-facing capability evidence in the caller's SQLite
/// snapshot. This deliberately revalidates the exact current pull context;
/// an older active generation must not keep the Commits panel reachable after
/// the summary or Body range changes.
pub(super) async fn capability_evidence_in(
    tx: &mut Transaction<'_, Sqlite>,
    account: &RemoteAccount,
    subject_id: &str,
) -> Result<PullCommitCapabilityEvidence> {
    let scope = DetailFacet::Commits.scope(subject_id);
    let (_, sync) = presentation(scope_in(tx, &account.id, &scope).await?);
    let denied = pull_commit_scope_denied_in(tx, &account.id, subject_id).await?;
    let missing = || PullCommitCapabilityEvidence {
        completeness: if sync.state == SyncState::Syncing {
            PullCommitCompleteness::syncing()
        } else {
            PullCommitCompleteness::missing()
        },
        row_count: 0,
        denied,
        sync: sync.clone(),
    };
    if denied {
        return Ok(missing());
    }
    let captured = match capture_pull_in(tx, &account.id, subject_id, true).await {
        Ok(captured) => captured,
        Err(error) if matches!(error.code, ErrorCode::NotFound | ErrorCode::StaleView) => {
            return Ok(missing());
        }
        Err(error) => return Err(error),
    };
    let row = sqlx::query(
        "SELECT g.completeness_json,(SELECT count(*) FROM pull_commit_rows r WHERE r.account_id=g.account_id AND r.subject_id=g.subject_id AND r.generation=g.generation AND r.position IS NOT NULL) AS row_count FROM pull_commit_facets f JOIN pull_commit_generations g ON g.account_id=f.account_id AND g.subject_id=f.subject_id AND g.generation=f.active_generation WHERE f.account_id=? AND f.subject_id=? AND f.authorization_epoch=? AND f.current_context_json=? AND f.facet_revision IS NOT NULL AND g.state='active' AND g.authorization_epoch=? AND g.context_json=?",
    )
    .bind(&account.id)
    .bind(subject_id)
    .bind(&account.authorization_epoch)
    .bind(encode(&captured.binding.context)?)
    .bind(&account.authorization_epoch)
    .bind(encode(&captured.binding.context)?)
    .fetch_optional(&mut **tx)
    .await
    .map_err(storage_error)?;
    let Some(row) = row else {
        return Ok(missing());
    };
    let completeness_json: Option<String> = row.get("completeness_json");
    let Some(completeness_json) = completeness_json else {
        return Ok(missing());
    };
    let completeness: PullCommitCompleteness = decode(&completeness_json)?;
    if !completeness.is_valid()
        || matches!(
            completeness.state,
            PullCommitCompletenessState::Missing | PullCommitCompletenessState::Syncing
        )
    {
        return Err(CollaborationError::storage());
    }
    Ok(PullCommitCapabilityEvidence {
        completeness,
        row_count: row.get("row_count"),
        denied: false,
        sync,
    })
}

pub(super) async fn cleanup_abandoned_in(connection: &mut SqliteConnection) -> Result<()> {
    let mut tx = connection.begin().await.map_err(storage_error)?;
    loop {
        // Bound each allocation while still reclaiming every abandoned
        // generation. Refusing to open a large valid cache would leave all of
        // its unpublished bytes permanently stranded.
        let subjects = sqlx::query(
            "SELECT DISTINCT account_id,subject_id FROM pull_commit_generations WHERE state='staging' ORDER BY account_id,subject_id LIMIT 500",
        )
        .fetch_all(&mut *tx)
        .await
        .map_err(storage_error)?;
        if subjects.is_empty() {
            break;
        }
        for row in &subjects {
            let account: &str = row.get("account_id");
            let subject: &str = row.get("subject_id");
            sqlx::query("DELETE FROM pull_commit_generations WHERE account_id=? AND subject_id=? AND state='staging'")
                .bind(account).bind(subject).execute(&mut *tx).await.map_err(storage_error)?;
            let scope = DetailFacet::Commits.scope(subject);
            sqlx::query("UPDATE sync_scopes SET run_id=?,next_cursor=NULL,etag=NULL,sync_json=json_set(sync_json,'$.state','idle') WHERE account_id=? AND scope=? AND json_extract(sync_json,'$.state')='syncing'")
                .bind(Uuid::new_v4().to_string()).bind(account).bind(&scope).execute(&mut *tx).await.map_err(storage_error)?;
            refresh_retention_in(&mut tx, account, subject).await?;
        }
    }
    tx.commit().await.map_err(storage_error)
}

impl Store {
    pub async fn begin_pull_commits(
        &self,
        account_id: &str,
        epoch: &str,
        subject_id: &str,
    ) -> Result<PullCommitLease> {
        let mut writer = self.inner.writer.acquire().await?;
        let mut tx = writer.begin().await.map_err(storage_error)?;
        epoch_in(&mut tx, account_id, epoch).await?;
        let captured = capture_pull_in(&mut tx, account_id, subject_id, true).await?;
        let (_, authorization_view) = metadata(&mut tx).await?;
        let context_json = encode(&captured.binding.context)?;
        let scope_denied = pull_commit_scope_denied_in(&mut tx, account_id, subject_id).await?;
        let facet = sqlx::query("SELECT authorization_epoch,current_context_json,active_generation FROM pull_commit_facets WHERE account_id=? AND subject_id=?")
            .bind(account_id).bind(subject_id).fetch_optional(&mut *tx).await.map_err(storage_error)?;
        let active_matches = !scope_denied
            && facet.as_ref().is_some_and(|row| {
                row.get::<String, _>("authorization_epoch") == epoch
                    && row.get::<String, _>("current_context_json") == context_json
                    && row.get::<Option<String>, _>("active_generation").is_some()
            });
        if !active_matches {
            sqlx::query("UPDATE pull_commit_facets SET active_generation=NULL,facet_revision=NULL,authorization_epoch=?,authorization_view=?,current_context_json=?,stale_at=NULL WHERE account_id=? AND subject_id=?")
                .bind(epoch).bind(&authorization_view).bind(&context_json).bind(account_id).bind(subject_id).execute(&mut *tx).await.map_err(storage_error)?;
            sqlx::query("UPDATE pull_commit_generations SET state='superseded' WHERE account_id=? AND subject_id=? AND state='active'")
                .bind(account_id).bind(subject_id).execute(&mut *tx).await.map_err(storage_error)?;
        }
        sqlx::query("DELETE FROM pull_commit_generations WHERE account_id=? AND subject_id=? AND state='staging'")
            .bind(account_id).bind(subject_id).execute(&mut *tx).await.map_err(storage_error)?;
        let run_id = Uuid::new_v4().to_string();
        let generation = Uuid::new_v4().to_string();
        let scope = DetailFacet::Commits.scope(subject_id);
        let old = scope_in(&mut tx, account_id, &scope).await?;
        let mut sync = old
            .as_ref()
            .map(|scope| scope.sync.clone())
            .unwrap_or_default();
        sync.state = SyncState::Syncing;
        sync.error = None;
        sync.next_retry_at = None;
        let coverage = if active_matches {
            old.as_ref()
                .map(|scope| scope.coverage.clone())
                .unwrap_or_else(missing_coverage)
        } else {
            missing_coverage()
        };
        sqlx::query("INSERT INTO sync_scopes(account_id,scope,run_id,next_cursor,coverage_json,sync_json,access_denied) VALUES(?,?,?,?,?,?,0) ON CONFLICT(account_id,scope) DO UPDATE SET run_id=excluded.run_id,next_cursor=NULL,etag=NULL,last_modified=NULL,coverage_json=excluded.coverage_json,sync_json=excluded.sync_json,access_denied=0")
            .bind(account_id).bind(&scope).bind(&run_id).bind(Option::<String>::None).bind(encode(&coverage)?).bind(encode(&sync)?).execute(&mut *tx).await.map_err(storage_error)?;
        let revision = record_change(
            &mut tx,
            account_id,
            positive_revision(epoch)?,
            &scope,
            false,
        )
        .await?;
        sqlx::query("INSERT INTO pull_commit_generations(account_id,subject_id,generation,run_id,authorization_epoch,authorization_view,binding_json,context_json,state,created_revision) VALUES(?,?,?,?,?,?,?,?, 'staging',?)")
            .bind(account_id).bind(subject_id).bind(&generation).bind(&run_id).bind(epoch).bind(&authorization_view).bind(encode(&captured.binding)?).bind(&context_json).bind(revision_number(&revision)?).execute(&mut *tx).await.map_err(storage_error)?;
        sqlx::query("INSERT INTO pull_commit_facets(account_id,subject_id,authorization_epoch,authorization_view,current_context_json) VALUES(?,?,?,?,?) ON CONFLICT(account_id,subject_id) DO UPDATE SET authorization_epoch=excluded.authorization_epoch,authorization_view=excluded.authorization_view,current_context_json=excluded.current_context_json")
            .bind(account_id).bind(subject_id).bind(epoch).bind(&authorization_view).bind(&context_json).execute(&mut *tx).await.map_err(storage_error)?;
        refresh_retention_in(&mut tx, account_id, subject_id).await?;
        let lease = PullCommitLease {
            run_id,
            generation,
            authorization_view,
            instance_id: captured.instance.id,
            binding: captured.binding,
            next_cursor: None,
            page_count: 0,
            row_count: 0,
        };
        tx.commit().await.map_err(storage_error)?;
        Ok(lease)
    }

    pub async fn apply_pull_commits(
        &self,
        mut commit: PullCommitCommit,
    ) -> Result<PullCommitApplyReceipt> {
        if commit.page.next_cursor.is_some() && commit.page.next_cursor == commit.request_cursor {
            return Err(pull_commit_drift());
        }
        if commit.page.commits.len() > MAX_PULL_COMMITS_PER_PROVIDER_PAGE
            || commit.page.source.source.is_empty()
            || commit.page.source.source.len() > MAX_SOURCE_BYTES
            || commit.page.source.adapter_version == 0
            || commit
                .page
                .next_cursor
                .as_ref()
                .is_some_and(|cursor| cursor.is_empty() || cursor.len() > MAX_CURSOR_BYTES)
            || commit.page.next_cursor.is_some() && !commit.page.remote_has_more
            || commit.page.commits.is_empty() && commit.page.next_cursor.is_some()
            || commit.page.cap_reason.is_some() && commit.page.next_cursor.is_some()
            || commit.page.cap_reason.is_some() && !commit.page.remote_has_more
        {
            return Err(invalid_commits());
        }
        if commit.page.context != commit.lease.binding.context {
            return Err(pull_commit_drift());
        }
        let mut page_oids = HashSet::with_capacity(commit.page.commits.len());
        let mut normalized = Vec::with_capacity(commit.page.commits.len());
        for value in commit.page.commits {
            let value = normalize_commit(value)?;
            if !page_oids.insert(value.oid.clone()) {
                return Err(pull_commit_drift());
            }
            normalized.push(value);
        }
        commit.page.commits = normalized;
        let mut writer = self.inner.writer.acquire().await?;
        let mut tx = writer.begin().await.map_err(storage_error)?;
        epoch_in(&mut tx, &commit.account_id, &commit.authorization_epoch).await?;
        let row = sqlx::query("SELECT subject_id,authorization_epoch,authorization_view,binding_json,context_json,source_json,provider_order,expected_cursor,page_count,row_count FROM pull_commit_generations WHERE account_id=? AND generation=? AND run_id=? AND state='staging'")
            .bind(&commit.account_id).bind(&commit.lease.generation).bind(&commit.lease.run_id).fetch_optional(&mut *tx).await.map_err(storage_error)?.ok_or_else(stale)?;
        let subject_id: String = row.get("subject_id");
        let current = capture_pull_in(&mut tx, &commit.account_id, &subject_id, true).await?;
        if commit.page.commits.iter().any(|value| {
            value
                .web_url
                .as_deref()
                .is_some_and(|url| !url_in_instance(&current.instance.base_url, url))
                || std::iter::once(&value.author)
                    .chain(value.committer.as_ref())
                    .any(|actor| {
                        actor
                            .provider
                            .as_ref()
                            .and_then(|provider| provider.web_url.as_deref())
                            .is_some_and(|url| !url_in_instance(&current.instance.base_url, url))
                    })
        }) {
            return Err(invalid_commits());
        }
        let stored_binding: PullCommitBinding = decode(row.get("binding_json"))?;
        let stored_context: PullCommitContext = decode(row.get("context_json"))?;
        let stored_source: Option<PullCommitSource> = row
            .get::<Option<String>, _>("source_json")
            .map(|value| decode(&value))
            .transpose()?;
        let stored_order: Option<PullCommitProviderOrder> = row
            .get::<Option<String>, _>("provider_order")
            .map(|value| decode(&format!("\"{value}\"")))
            .transpose()?;
        let expected_cursor: Option<String> = row.get("expected_cursor");
        let page_count = u32::try_from(row.get::<i64, _>("page_count"))
            .map_err(|_| CollaborationError::storage())?;
        let row_count = u32::try_from(row.get::<i64, _>("row_count"))
            .map_err(|_| CollaborationError::storage())?;
        if row.get::<String, _>("authorization_epoch") != commit.authorization_epoch
            || row.get::<String, _>("authorization_view") != commit.lease.authorization_view
            || metadata(&mut tx).await?.1 != commit.lease.authorization_view
            || current.instance.id != commit.lease.instance_id
            || current.binding != commit.lease.binding
            || stored_binding != commit.lease.binding
            || stored_context != commit.page.context
            || expected_cursor != commit.request_cursor
            || expected_cursor != commit.lease.next_cursor
            || page_count != commit.lease.page_count
            || row_count != commit.lease.row_count
            || page_count >= MAX_PULL_COMMIT_PAGES
        {
            return Err(stale());
        }
        if stored_source
            .as_ref()
            .is_some_and(|source| source != &commit.page.source)
            || stored_order.is_some_and(|order| order != commit.page.order)
            || commit.page.start_position != row_count
        {
            return Err(pull_commit_drift());
        }
        if let Some(cursor) = &commit.page.next_cursor {
            let seen: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM pull_commit_cursors WHERE account_id=? AND subject_id=? AND generation=? AND cursor=?)")
                .bind(&commit.account_id).bind(&subject_id).bind(&commit.lease.generation).bind(cursor).fetch_one(&mut *tx).await.map_err(storage_error)?;
            if seen {
                return Err(pull_commit_drift());
            }
        }
        let available = MAX_PULL_COMMITS.saturating_sub(row_count) as usize;
        let accepted = commit.page.commits.len().min(available);
        let new_count = row_count
            .checked_add(u32::try_from(accepted).map_err(|_| CollaborationError::storage())?)
            .ok_or_else(CollaborationError::storage)?;
        let local_cap = accepted < commit.page.commits.len()
            || (new_count == MAX_PULL_COMMITS && commit.page.remote_has_more)
            || (page_count + 1 >= MAX_PULL_COMMIT_PAGES && commit.page.next_cursor.is_some());
        for (offset, value) in commit.page.commits.into_iter().take(accepted).enumerate() {
            let sequence = row_count
                .checked_add(u32::try_from(offset).map_err(|_| CollaborationError::storage())?)
                .ok_or_else(CollaborationError::storage)?;
            let seen: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM pull_commit_rows WHERE account_id=? AND subject_id=? AND generation=? AND oid=?)")
                .bind(&commit.account_id).bind(&subject_id).bind(&commit.lease.generation).bind(&value.oid).fetch_one(&mut *tx).await.map_err(storage_error)?;
            if seen {
                return Err(pull_commit_drift());
            }
            sqlx::query("INSERT INTO pull_commit_rows(account_id,subject_id,generation,provider_sequence,oid,json) VALUES(?,?,?,?,?,?)")
                .bind(&commit.account_id).bind(&subject_id).bind(&commit.lease.generation).bind(i64::from(sequence)).bind(&value.oid).bind(encode(&value)?).execute(&mut *tx).await.map_err(storage_error)?;
        }
        if commit.page.order == PullCommitProviderOrder::HeadToBase && new_count > 0 {
            let first_oid: Option<String> = sqlx::query_scalar("SELECT oid FROM pull_commit_rows WHERE account_id=? AND subject_id=? AND generation=? ORDER BY provider_sequence LIMIT 1")
                .bind(&commit.account_id).bind(&subject_id).bind(&commit.lease.generation).fetch_optional(&mut *tx).await.map_err(storage_error)?;
            if first_oid.as_ref() != Some(&commit.lease.binding.context.head_oid) {
                return Err(pull_commit_drift());
            }
        }
        if let Some(cursor) = &commit.page.next_cursor {
            sqlx::query("INSERT INTO pull_commit_cursors(account_id,subject_id,generation,cursor) VALUES(?,?,?,?)")
                .bind(&commit.account_id).bind(&subject_id).bind(&commit.lease.generation).bind(cursor).execute(&mut *tx).await.map_err(storage_error)?;
        }
        let cap = if local_cap {
            Some(PullCommitCapReason::LocalLimit)
        } else {
            commit.page.cap_reason
        };
        let terminal = local_cap || commit.page.next_cursor.is_none();
        if terminal
            && commit
                .terminal_validation
                .as_ref()
                .is_none_or(|validation| {
                    validation.base_oid != commit.lease.binding.context.base_oid
                        || validation.head_oid != commit.lease.binding.context.head_oid
                        || validation.base_repository_provider_id
                            != commit.lease.binding.repository_provider_id
                        || validation.source_repository_provider_id
                            != commit.lease.binding.context.source_repository_provider_id
                })
        {
            return Err(pull_commit_drift());
        }
        let next_cursor = (!terminal)
            .then(|| commit.page.next_cursor.clone())
            .flatten();
        sqlx::query("UPDATE pull_commit_generations SET source_json=?,provider_order=?,expected_cursor=?,page_count=?,row_count=? WHERE account_id=? AND subject_id=? AND generation=? AND run_id=? AND state='staging'")
            .bind(encode(&commit.page.source)?).bind(tag(&commit.page.order)?).bind(&next_cursor).bind(i64::from(page_count+1)).bind(i64::from(new_count)).bind(&commit.account_id).bind(&subject_id).bind(&commit.lease.generation).bind(&commit.lease.run_id).execute(&mut *tx).await.map_err(storage_error)?;
        let scope = DetailFacet::Commits.scope(&subject_id);
        if !terminal {
            sqlx::query("UPDATE sync_scopes SET next_cursor=?,data_revision=data_revision+1 WHERE account_id=? AND scope=? AND run_id=?")
                .bind(&next_cursor).bind(&commit.account_id).bind(&scope).bind(&commit.lease.run_id).execute(&mut *tx).await.map_err(storage_error)?;
            refresh_retention_in(&mut tx, &commit.account_id, &subject_id).await?;
            tx.commit().await.map_err(storage_error)?;
            return Ok(PullCommitApplyReceipt {
                revision: self.revision().await?,
                next_cursor,
                published: false,
                row_count: new_count,
            });
        }
        let completeness = if let Some(reason) = cap {
            PullCommitCompleteness::capped(reason)
        } else if commit.page.remote_has_more {
            PullCommitCompleteness::partial()
        } else {
            PullCommitCompleteness::complete()
        };
        if completeness.is_complete()
            && new_count > 0
            && commit.page.order == PullCommitProviderOrder::BaseToHead
        {
            let terminal_oid: Option<String> = sqlx::query_scalar("SELECT oid FROM pull_commit_rows WHERE account_id=? AND subject_id=? AND generation=? ORDER BY provider_sequence DESC LIMIT 1")
                .bind(&commit.account_id).bind(&subject_id).bind(&commit.lease.generation).fetch_optional(&mut *tx).await.map_err(storage_error)?;
            if terminal_oid.as_ref() != Some(&commit.lease.binding.context.head_oid) {
                return Err(pull_commit_drift());
            }
        }
        sqlx::query("UPDATE pull_commit_rows SET position=CASE WHEN ?='base_to_head' THEN provider_sequence ELSE ?-1-provider_sequence END WHERE account_id=? AND subject_id=? AND generation=?")
            .bind(tag(&commit.page.order)?).bind(i64::from(new_count)).bind(&commit.account_id).bind(&subject_id).bind(&commit.lease.generation).execute(&mut *tx).await.map_err(storage_error)?;
        sqlx::query("UPDATE pull_commit_facets SET active_generation=NULL,facet_revision=NULL WHERE account_id=? AND subject_id=?")
            .bind(&commit.account_id).bind(&subject_id).execute(&mut *tx).await.map_err(storage_error)?;
        sqlx::query("DELETE FROM pull_commit_generations WHERE account_id=? AND subject_id=? AND state IN ('active','superseded')")
            .bind(&commit.account_id).bind(&subject_id).execute(&mut *tx).await.map_err(storage_error)?;
        sqlx::query("UPDATE pull_commit_generations SET state='active',completeness_json=?,expected_cursor=NULL WHERE account_id=? AND subject_id=? AND generation=? AND run_id=? AND state='staging'")
            .bind(encode(&completeness)?).bind(&commit.account_id).bind(&subject_id).bind(&commit.lease.generation).bind(&commit.lease.run_id).execute(&mut *tx).await.map_err(storage_error)?;
        let now = chrono::Utc::now();
        let stale_at = now
            .checked_add_signed(chrono::Duration::seconds(i64::from(
                commit.page.freshness_seconds.max(1),
            )))
            .ok_or_else(CollaborationError::storage)?
            .to_rfc3339();
        let coverage = Coverage {
            state: if completeness.is_complete() {
                CoverageState::Complete
            } else {
                CoverageState::Partial
            },
            validated_at: Some(now.to_rfc3339()),
            remote_has_more: !completeness.is_complete(),
        };
        let sync = SyncStatus {
            state: SyncState::Idle,
            last_success_at: Some(now.to_rfc3339()),
            next_retry_at: None,
            error: None,
        };
        let revision = record_change(
            &mut tx,
            &commit.account_id,
            positive_revision(&commit.authorization_epoch)?,
            &scope,
            false,
        )
        .await?;
        sqlx::query("UPDATE pull_commit_facets SET authorization_epoch=?,authorization_view=?,current_context_json=?,active_generation=?,facet_revision=?,stale_at=? WHERE account_id=? AND subject_id=?")
            .bind(&commit.authorization_epoch).bind(&commit.lease.authorization_view).bind(encode(&commit.lease.binding.context)?).bind(&commit.lease.generation).bind(&revision).bind(stale_at).bind(&commit.account_id).bind(&subject_id).execute(&mut *tx).await.map_err(storage_error)?;
        sqlx::query("UPDATE sync_scopes SET completed_run_id=?,next_cursor=NULL,coverage_json=?,sync_json=?,access_denied=0,data_revision=data_revision+1 WHERE account_id=? AND scope=? AND run_id=?")
            .bind(&commit.lease.run_id).bind(encode(&coverage)?).bind(encode(&sync)?).bind(&commit.account_id).bind(&scope).bind(&commit.lease.run_id).execute(&mut *tx).await.map_err(storage_error)?;
        sqlx::query("UPDATE detail_demand SET requested=0 WHERE account_id=? AND subject_id=? AND facet='commits' AND authorization_epoch=?")
            .bind(&commit.account_id).bind(&subject_id).bind(&commit.authorization_epoch).execute(&mut *tx).await.map_err(storage_error)?;
        refresh_retention_in(&mut tx, &commit.account_id, &subject_id).await?;
        tx.commit().await.map_err(storage_error)?;
        Ok(PullCommitApplyReceipt {
            revision,
            next_cursor: None,
            published: true,
            row_count: new_count,
        })
    }

    pub async fn pull_commits(&self, query: PullCommitQuery) -> Result<PullCommitSnapshot> {
        if query.limit == 0
            || query.limit > MAX_PULL_COMMITS_PER_LOCAL_PAGE
            || query
                .cursor
                .as_ref()
                .is_some_and(|cursor| cursor.len() > MAX_LOCAL_CURSOR_BYTES)
        {
            return Err(invalid_commits());
        }
        validate_identifier(&query.subject_id)?;
        let mut tx = self.inner.readers.begin().await.map_err(storage_error)?;
        let account = account_in(&mut tx, &query.account_id, false).await?;
        let (revision, authorization_view) = metadata(&mut tx).await?;
        let scope = DetailFacet::Commits.scope(&query.subject_id);
        let stored = scope_in(&mut tx, &query.account_id, &scope).await?;
        let (coverage, sync) = presentation(stored);
        let mut snapshot = PullCommitSnapshot {
            subject_id: query.subject_id.clone(),
            context: None,
            commits: vec![],
            next_cursor: None,
            completeness: if sync.state == SyncState::Syncing {
                PullCommitCompleteness::syncing()
            } else {
                PullCommitCompleteness::missing()
            },
            coverage,
            sync,
            freshness: DetailFreshness::Unknown,
            facet_revision: None,
            revision,
            authorization_view: authorization_view.clone(),
        };
        if account.state != AccountState::Active {
            snapshot.sync.state = SyncState::AuthRequired;
            tx.commit().await.map_err(storage_error)?;
            return Ok(snapshot);
        }
        if pull_commit_scope_denied_in(&mut tx, &query.account_id, &query.subject_id).await? {
            snapshot.coverage = missing_coverage();
            snapshot.completeness = PullCommitCompleteness::missing();
            tx.commit().await.map_err(storage_error)?;
            return Ok(snapshot);
        }
        let captured = capture_pull_in(&mut tx, &query.account_id, &query.subject_id, true).await;
        let captured = match captured {
            Ok(captured) => captured,
            Err(error) if matches!(error.code, ErrorCode::NotFound | ErrorCode::StaleView) => {
                snapshot.coverage = missing_coverage();
                snapshot.completeness = PullCommitCompleteness::missing();
                tx.commit().await.map_err(storage_error)?;
                return Ok(snapshot);
            }
            Err(error) => return Err(error),
        };
        snapshot.context = Some(captured.binding.context.clone());
        let facet = sqlx::query("SELECT authorization_epoch,current_context_json,active_generation,facet_revision,stale_at FROM pull_commit_facets WHERE account_id=? AND subject_id=?")
            .bind(&query.account_id).bind(&query.subject_id).fetch_optional(&mut *tx).await.map_err(storage_error)?;
        let Some(facet) = facet else {
            tx.commit().await.map_err(storage_error)?;
            return Ok(snapshot);
        };
        let current_context: PullCommitContext = decode(facet.get("current_context_json"))?;
        let active_generation: Option<String> = facet.get("active_generation");
        let facet_revision: Option<String> = facet.get("facet_revision");
        if facet.get::<String, _>("authorization_epoch") != account.authorization_epoch
            || current_context != captured.binding.context
            || active_generation.is_none()
            || facet_revision.is_none()
        {
            snapshot.coverage = missing_coverage();
            snapshot.completeness = if snapshot.sync.state == SyncState::Syncing {
                PullCommitCompleteness::syncing()
            } else {
                PullCommitCompleteness::missing()
            };
            tx.commit().await.map_err(storage_error)?;
            return Ok(snapshot);
        }
        let generation = active_generation.ok_or_else(stale)?;
        let facet_revision = facet_revision.ok_or_else(stale)?;
        let completeness_json: Option<String> = sqlx::query_scalar("SELECT completeness_json FROM pull_commit_generations WHERE account_id=? AND subject_id=? AND generation=? AND state='active' AND authorization_epoch=? AND context_json=?")
            .bind(&query.account_id).bind(&query.subject_id).bind(&generation).bind(&account.authorization_epoch).bind(encode(&captured.binding.context)?).fetch_optional(&mut *tx).await.map_err(storage_error)?.flatten();
        let Some(completeness_json) = completeness_json else {
            tx.commit().await.map_err(storage_error)?;
            return Ok(snapshot);
        };
        snapshot.completeness = decode(&completeness_json)?;
        if !snapshot.completeness.is_valid() {
            return Err(CollaborationError::storage());
        }
        snapshot.facet_revision = Some(facet_revision.clone());
        let stale_at: Option<String> = facet.get("stale_at");
        snapshot.freshness = if stale_at.as_deref().is_some_and(|value| {
            chrono::DateTime::parse_from_rfc3339(value)
                .is_ok_and(|value| value > chrono::Utc::now())
        }) {
            DetailFreshness::Fresh
        } else {
            DetailFreshness::Stale
        };
        let mut after_position = None;
        let mut after_oid = None;
        if let Some(cursor) = query.cursor {
            let cursor: PullCommitCursor =
                serde_json::from_str(&cursor).map_err(|_| invalid_commits())?;
            if cursor.version != 1
                || cursor.account_id != query.account_id
                || cursor.authorization_view != authorization_view
                || cursor.subject_id != query.subject_id
                || cursor.generation != generation
                || cursor.facet_revision != facet_revision
                || cursor.context != captured.binding.context
                || !is_canonical_commit_oid(&cursor.last_oid)
            {
                return Err(stale());
            }
            let cursor_present: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM pull_commit_rows WHERE account_id=? AND subject_id=? AND generation=? AND position=? AND oid=?)")
                .bind(&query.account_id)
                .bind(&query.subject_id)
                .bind(&generation)
                .bind(i64::from(cursor.last_position))
                .bind(&cursor.last_oid)
                .fetch_one(&mut *tx)
                .await
                .map_err(storage_error)?;
            if !cursor_present {
                return Err(stale());
            }
            after_position = Some(cursor.last_position);
            after_oid = Some(cursor.last_oid);
        }
        let mut sql = sqlx::QueryBuilder::<Sqlite>::new(
            "SELECT position,json FROM pull_commit_rows WHERE account_id=",
        );
        sql.push_bind(&query.account_id)
            .push(" AND subject_id=")
            .push_bind(&query.subject_id)
            .push(" AND generation=")
            .push_bind(&generation)
            .push(" AND position IS NOT NULL");
        if let (Some(position), Some(oid)) = (after_position, after_oid.as_ref()) {
            sql.push(" AND (position,oid)>(")
                .push_bind(i64::from(position))
                .push(",")
                .push_bind(oid)
                .push(")");
        }
        sql.push(" ORDER BY position,oid LIMIT ")
            .push_bind(i64::from(query.limit) + 1);
        let rows = sql
            .build()
            .fetch_all(&mut *tx)
            .await
            .map_err(storage_error)?;
        let mut page_bytes = 0_usize;
        for row in &rows {
            if snapshot.commits.len() >= query.limit as usize {
                break;
            }
            let row_bytes = row.get::<&str, _>("json").len();
            if !snapshot.commits.is_empty()
                && page_bytes.saturating_add(row_bytes) > MAX_PULL_COMMIT_LOCAL_PAGE_BYTES
            {
                break;
            }
            snapshot.commits.push(row_to_commit(row)?);
            page_bytes = page_bytes.saturating_add(row_bytes);
        }
        let has_more = snapshot.commits.len() < rows.len();
        if has_more {
            let last = snapshot
                .commits
                .last()
                .ok_or_else(CollaborationError::storage)?;
            snapshot.next_cursor = Some(encode(&PullCommitCursor {
                version: 1,
                account_id: query.account_id,
                authorization_view,
                subject_id: query.subject_id,
                generation,
                facet_revision,
                context: captured.binding.context,
                last_position: last.position,
                last_oid: last.oid.clone(),
            })?);
        }
        tx.commit().await.map_err(storage_error)?;
        Ok(snapshot)
    }

    pub async fn verify_pull_commit_membership(
        &self,
        request: PullCommitMembershipRequest,
    ) -> Result<PullCommitMembershipReceipt> {
        if !is_canonical_commit_oid(&request.commit_oid) {
            return Err(invalid_commits());
        }
        let mut tx = self.inner.readers.begin().await.map_err(storage_error)?;
        epoch_in(&mut tx, &request.account_id, &request.authorization_epoch).await?;
        if pull_commit_scope_denied_in(&mut tx, &request.account_id, &request.subject_id).await? {
            return Err(stale());
        }
        let captured =
            capture_pull_in(&mut tx, &request.account_id, &request.subject_id, true).await?;
        let (_, authorization_view) = metadata(&mut tx).await?;
        let generation: Option<String> = sqlx::query_scalar("SELECT active_generation FROM pull_commit_facets WHERE account_id=? AND subject_id=? AND authorization_epoch=? AND current_context_json=? AND facet_revision=?")
            .bind(&request.account_id).bind(&request.subject_id).bind(&request.authorization_epoch).bind(encode(&captured.binding.context)?).bind(&request.facet_revision).fetch_optional(&mut *tx).await.map_err(storage_error)?.flatten();
        let generation = generation.ok_or_else(stale)?;
        let present: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM pull_commit_generations g JOIN pull_commit_rows r ON r.account_id=g.account_id AND r.subject_id=g.subject_id AND r.generation=g.generation WHERE g.account_id=? AND g.subject_id=? AND g.generation=? AND g.state='active' AND g.authorization_epoch=? AND g.context_json=? AND r.oid=? AND r.position IS NOT NULL)")
            .bind(&request.account_id).bind(&request.subject_id).bind(&generation).bind(&request.authorization_epoch).bind(encode(&captured.binding.context)?).bind(&request.commit_oid).fetch_one(&mut *tx).await.map_err(storage_error)?;
        if !present {
            return Err(CollaborationError::new(
                ErrorCode::NotFound,
                "Commit is not a member of the active cached pull range",
            ));
        }
        let receipt = PullCommitMembershipReceipt {
            repository_id: captured.binding.repository_id,
            instance_id: captured.instance.id,
            authorization_view,
            subject_id: request.subject_id,
            commit_oid: request.commit_oid,
            active_generation: generation,
            facet_revision: request.facet_revision,
            context: captured.binding.context,
        };
        tx.commit().await.map_err(storage_error)?;
        Ok(receipt)
    }

    pub(crate) async fn pull_commit_demand_state(
        &self,
        account_id: &str,
        subject_id: &str,
    ) -> Result<PullCommitSnapshot> {
        self.pull_commits(PullCommitQuery {
            account_id: account_id.into(),
            subject_id: subject_id.into(),
            cursor: None,
            limit: 1,
        })
        .await
    }
}
