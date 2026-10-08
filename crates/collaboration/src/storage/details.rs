//! Independent facet observations and short, authorized local snapshots.
use super::facet_reconciliation::{StoredEntry, StoredSource};
use super::*;
use crate::{DetailSubjectBinding, detail::*};

const MAX_DETAIL_ENTRIES: i64 = 5_000;
const MAX_ENTRY_BODY_BYTES: usize = 65_536;

fn is_review_facet(facet: DetailFacet) -> bool {
    matches!(
        facet,
        DetailFacet::ReviewSummaries | DetailFacet::ReviewThreads
    )
}

/// Internal SQL expressions only; all detail facets share authorization resets.
pub(super) fn scope_sql_list(subject_expression: &str) -> String {
    DetailFacet::ALL
        .into_iter()
        .map(|facet| format!("'detail:'||{subject_expression}||':{}'", facet.name()))
        .collect::<Vec<_>>()
        .join(",")
}

pub(super) async fn invalidate_head_scopes_in(
    tx: &mut Transaction<'_, Sqlite>,
    account: &RemoteAccount,
    item: &RemoteItem,
) -> Result<()> {
    invalidate_declared_head_in(tx, account, &item.id, item.head_oid.as_deref()).await
}

pub(super) async fn invalidate_declared_head_in(
    tx: &mut Transaction<'_, Sqlite>,
    account: &RemoteAccount,
    subject: &str,
    head: Option<&str>,
) -> Result<()> {
    super::pull_files::head_observed_in(tx, account, subject, head).await?;
    let rows = sqlx::query("SELECT facet,source_json FROM detail_observations WHERE account_id=? AND subject_id=? AND facet<>'body' AND authorization_epoch=?")
        .bind(&account.id).bind(subject).bind(&account.authorization_epoch).fetch_all(&mut **tx).await.map_err(storage_error)?;
    for row in rows {
        let source: StoredSource = decode(row.get("source_json"))?;
        let Some(proof) = source
            .traversal()
            .filter(|proof| proof.reconciliation.head_scope == DetailHeadScope::CurrentHead)
        else {
            continue;
        };
        let facet: DetailFacet = decode(&format!("\"{}\"", row.get::<String, _>("facet")))?;
        let changed = if facet == DetailFacet::Checks {
            match super::pull_commits::check_context_in(tx, &account.id, subject, false).await {
                Ok(context) => proof.check_context.as_ref() != Some(&context),
                Err(error) if matches!(error.code, ErrorCode::NotFound | ErrorCode::StaleView) => {
                    true
                }
                Err(error) => return Err(error),
            }
        } else if is_review_facet(facet) {
            match super::pull_commits::review_context_in(tx, &account.id, subject, false).await {
                Ok(context) => proof.review_context.as_ref() != Some(&context),
                Err(error) if matches!(error.code, ErrorCode::NotFound | ErrorCode::StaleView) => {
                    true
                }
                Err(error) => return Err(error),
            }
        } else {
            proof.head_oid.as_deref() != head
        };
        if !changed {
            continue;
        }
        let scope = facet.scope(subject);
        let stored = scope_in(tx, &account.id, &scope).await?.ok_or_else(stale)?;
        let mut coverage = stored.coverage;
        coverage.state = CoverageState::Partial;
        coverage.remote_has_more = false;
        sqlx::query("UPDATE detail_observations SET stale_at=? WHERE account_id=? AND subject_id=? AND facet=?")
            .bind(chrono::Utc::now().to_rfc3339()).bind(&account.id).bind(subject).bind(tag(&facet)?).execute(&mut **tx).await.map_err(storage_error)?;
        sqlx::query("UPDATE sync_scopes SET run_id=?,next_cursor=NULL,etag=NULL,coverage_json=?,data_revision=data_revision+1,sync_json=json_set(sync_json,'$.state','idle') WHERE account_id=? AND scope=?")
            .bind(Uuid::new_v4().to_string()).bind(encode(&coverage)?).bind(&account.id).bind(&scope).execute(&mut **tx).await.map_err(storage_error)?;
        record_change(
            tx,
            &account.id,
            positive_revision(&account.authorization_epoch)?,
            &scope,
            false,
        )
        .await?;
    }
    Ok(())
}

async fn validate_declared_head_in(
    tx: &mut Transaction<'_, Sqlite>,
    account: &RemoteAccount,
    subject: &RemoteItem,
    facet: DetailFacet,
    reconciliation: Option<DetailReconciliation>,
) -> Result<()> {
    if facet != DetailFacet::Body
        && reconciliation.is_some_and(|proof| proof.head_scope == DetailHeadScope::CurrentHead)
        && super::resource_metadata::head_conflicts_in(
            tx,
            account,
            &subject.id,
            subject.head_oid.as_deref(),
        )
        .await?
    {
        return Err(stale());
    }
    Ok(())
}

#[derive(Serialize, Deserialize)]
struct DetailCursor {
    account: String,
    subject: String,
    facet: DetailFacet,
    authorization_view: String,
    facet_revision: Option<String>,
    effective_revision: String,
    last_id: String,
    #[serde(default)]
    activity_order: Option<String>,
}

pub(crate) async fn subject_in(
    tx: &mut Transaction<'_, Sqlite>,
    account_id: &str,
    subject_id: &str,
) -> Result<RemoteItem> {
    validate_identifier(subject_id)?;
    let json: Option<String> = sqlx::query_scalar(
        "SELECT json FROM items WHERE account_id=? AND id=? AND kind IN ('pull_request','issue')",
    )
    .bind(account_id)
    .bind(subject_id)
    .fetch_optional(&mut **tx)
    .await
    .map_err(storage_error)?;
    let subject: RemoteItem = json.map(|s| decode(&s)).transpose()?.ok_or_else(|| {
        CollaborationError::new(ErrorCode::NotFound, "Detail subject is not cached")
    })?;
    let kind = match subject.kind {
        RemoteItemKind::PullRequest => ResourceKind::PullRequest,
        RemoteItemKind::Issue => ResourceKind::Issue,
        _ => return Err(invalid_detail()),
    };
    if !identities::accessible(tx, account_id, subject_id, kind).await? {
        return Err(CollaborationError::new(
            ErrorCode::PermissionDenied,
            "Detail subject is inaccessible",
        ));
    }
    Ok(subject)
}

async fn subject_visible_in(
    tx: &mut Transaction<'_, Sqlite>,
    account: &str,
    subject: &str,
) -> Result<bool> {
    validate_identifier(subject)?;
    let kind: Option<String> = sqlx::query_scalar(
        "SELECT kind FROM items WHERE account_id=? AND id=? AND kind IN ('pull_request','issue')",
    )
    .bind(account)
    .bind(subject)
    .fetch_optional(&mut **tx)
    .await
    .map_err(storage_error)?;
    let Some(kind) = kind else {
        return Ok(false);
    };
    identities::accessible(
        tx,
        account,
        subject,
        if kind == "pull_request" {
            ResourceKind::PullRequest
        } else {
            ResourceKind::Issue
        },
    )
    .await
}

/// Metadata only; contextual capabilities reuse their own atomic read snapshot.
pub(crate) async fn detail_evidence_in(
    tx: &mut Transaction<'_, Sqlite>,
    account: &RemoteAccount,
    subject_id: &str,
    facet: DetailFacet,
) -> Result<DetailEvidence> {
    let scope = facet.scope(subject_id);
    let stored = scope_in(tx, &account.id, &scope).await?;
    let mut evidence = DetailEvidence {
        facet,
        availability: DetailAvailability::Missing,
        coverage: missing_coverage(),
        freshness: DetailFreshness::Unknown,
        stale_at: None,
        facet_revision: None,
        authorization_epoch: account.authorization_epoch.clone(),
        access_reason: None,
        source: None,
        value_source: None,
        saved_empty: None,
        observed_state: DetailValueState::NotLoaded,
        sync: stored.as_ref().map(|s| s.sync.clone()).unwrap_or_default(),
    };
    let denial: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM sync_scopes WHERE account_id=? AND scope=? AND access_denied=1)")
        .bind(&account.id).bind(&scope).fetch_one(&mut **tx).await.map_err(storage_error)?;
    let inaccessible = if account.state != AccountState::Active {
        Some(CapabilityReason::AuthenticationRequired)
    } else if denial || !subject_visible_in(tx, &account.id, subject_id).await? {
        Some(CapabilityReason::PermissionDenied)
    } else {
        None
    };
    if let Some(reason) = inaccessible {
        evidence.availability = DetailAvailability::Unavailable;
        evidence.access_reason = Some(reason);
        return Ok(evidence);
    }
    // Do not read provider body JSON while observing contextual metadata.
    let row = sqlx::query("SELECT authorization_epoch,facet_revision,source_json,value_source_json,observed_state,stale_at FROM detail_observations WHERE account_id=? AND subject_id=? AND facet=?")
        .bind(&account.id).bind(subject_id).bind(tag(&facet)?).fetch_optional(&mut **tx).await.map_err(storage_error)?;
    if let Some(row) = row {
        if row.get::<String, _>("authorization_epoch") != account.authorization_epoch {
            return Ok(evidence);
        }
        evidence.facet_revision = Some(row.get("facet_revision"));
        evidence.source = Some(decode(row.get("source_json"))?);
        let native: StoredSource = decode(row.get("source_json"))?;
        let head_changed = if let Some(proof) = native
            .traversal()
            .filter(|proof| proof.reconciliation.head_scope == DetailHeadScope::CurrentHead)
        {
            let head: Option<String> = sqlx::query_scalar(
                "SELECT json_extract(json,'$.head_oid') FROM items WHERE account_id=? AND id=?",
            )
            .bind(&account.id)
            .bind(subject_id)
            .fetch_one(&mut **tx)
            .await
            .map_err(storage_error)?;
            let exact_context_changed = if facet == DetailFacet::Checks {
                match super::pull_commits::check_context_in(tx, &account.id, subject_id, false)
                    .await
                {
                    Ok(context) => proof.check_context.as_ref() != Some(&context),
                    Err(error)
                        if matches!(error.code, ErrorCode::NotFound | ErrorCode::StaleView) =>
                    {
                        true
                    }
                    Err(error) => return Err(error),
                }
            } else if is_review_facet(facet) {
                match super::pull_commits::review_context_in(tx, &account.id, subject_id, false)
                    .await
                {
                    Ok(context) => proof.review_context.as_ref() != Some(&context),
                    Err(error)
                        if matches!(error.code, ErrorCode::NotFound | ErrorCode::StaleView) =>
                    {
                        true
                    }
                    Err(error) => return Err(error),
                }
            } else {
                false
            };
            head != proof.head_oid
                || exact_context_changed
                || super::resource_metadata::head_conflicts_in(
                    tx,
                    account,
                    subject_id,
                    head.as_deref(),
                )
                .await?
        } else {
            false
        };
        evidence.value_source = row
            .get::<Option<String>, _>("value_source_json")
            .map(|json| decode(&json))
            .transpose()?;
        evidence.observed_state =
            decode(&format!("\"{}\"", row.get::<String, _>("observed_state")))?;
        evidence.stale_at = row.get("stale_at");
        evidence.coverage = stored
            .as_ref()
            .map(|s| s.coverage.clone())
            .unwrap_or_else(missing_coverage);
        if facet != DetailFacet::Body && (native.traversal().is_none() || head_changed) {
            evidence.coverage.state = CoverageState::Partial;
        }
        evidence.availability = if evidence.coverage.state == CoverageState::Complete {
            DetailAvailability::Ready
        } else {
            DetailAvailability::Partial
        };
        if evidence.coverage.state == CoverageState::Complete {
            evidence.saved_empty = if facet == DetailFacet::Body {
                sqlx::query_scalar("SELECT CASE WHEN json_extract(body_json,'$.state')='known' THEN (json_extract(body_json,'$.text') IS NULL OR json_extract(body_json,'$.text')='') ELSE NULL END FROM detail_observations WHERE account_id=? AND subject_id=? AND facet='body'")
                    .bind(&account.id).bind(subject_id).fetch_one(&mut **tx).await.map_err(storage_error)?
            } else {
                Some(sqlx::query_scalar("SELECT NOT EXISTS(SELECT 1 FROM detail_entries WHERE account_id=? AND subject_id=? AND facet=?)")
                    .bind(&account.id).bind(subject_id).bind(tag(&facet)?).fetch_one(&mut **tx).await.map_err(storage_error)?)
            };
        }
        if evidence.coverage.validated_at.is_some() {
            evidence.freshness = if !head_changed
                && evidence.stale_at.as_ref().is_some_and(|at| {
                    chrono::DateTime::parse_from_rfc3339(at).is_ok_and(|at| at > chrono::Utc::now())
                }) {
                DetailFreshness::Fresh
            } else {
                DetailFreshness::Stale
            };
        }
    }
    Ok(evidence)
}

fn mask_valid(facet: DetailFacet, mask: &[DetailField]) -> bool {
    mask.len() <= facet.field_limit()
        && mask.iter().all(|field| field.valid_for(facet))
        && mask
            .iter()
            .enumerate()
            .all(|(i, field)| !mask[..i].contains(field))
}
fn bounded(value: &Option<String>, limit: usize) -> bool {
    value.as_ref().is_none_or(|v| v.len() <= limit)
}
fn validate_value(value: &DetailValue) -> Result<()> {
    if value.state != DetailValueState::Known && value.text.is_some() {
        return Err(invalid_detail());
    }
    Ok(())
}
fn timestamp_valid(value: &str) -> bool {
    value.len() <= 128 && chrono::DateTime::parse_from_rfc3339(value).is_ok()
}

fn participant_text(value: &Option<String>, limit: usize, nonempty: bool) -> bool {
    value.as_ref().is_none_or(|value| {
        value.len() <= limit
            && (!nonempty || !value.is_empty())
            && !value.chars().any(char::is_control)
    })
}

fn validate_native(facet: DetailFacet, entry: &DetailEntry) -> Result<()> {
    if facet == DetailFacet::Activity {
        let Some(crate::NativeDetailPayload::ActivityV1(event)) = &entry.native else {
            return Err(invalid_detail());
        };
        if !event.valid()
            || entry.title.is_some()
            || entry.state.is_some()
            || entry.head_oid.is_some()
            || entry
                .body
                .text
                .as_ref()
                .is_some_and(|s| s.len() > 4096 || s.contains('\0'))
            || entry
                .author
                .as_ref()
                .is_some_and(|s| s.len() > 256 || s.chars().any(char::is_control))
            || entry
                .updated_at
                .as_ref()
                .is_some_and(|s| !timestamp_valid(s))
            || entry.observed_body_state != entry.body.state
        {
            return Err(invalid_detail());
        }
        return Ok(());
    }
    if facet == DetailFacet::Checks {
        return validate_check(entry);
    }
    if is_review_facet(facet) {
        return validate_review(facet, entry);
    }
    if facet == DetailFacet::Tasks {
        return validate_task(entry);
    }
    if facet != DetailFacet::Participants {
        return if entry.native.is_none() {
            Ok(())
        } else {
            Err(invalid_detail())
        };
    }
    let Some(crate::NativeDetailPayload::ParticipantV1(value)) = &entry.native else {
        return Err(invalid_detail());
    };
    validate_identifier(&value.user.provider_id)?;
    if value.user.provider_id.chars().any(char::is_control)
        || entry.author.is_some()
        || entry.title.is_some()
        || entry.state.is_some()
        || entry.body != DetailValue::default()
        || entry.observed_body_state != DetailValueState::NotLoaded
        || entry.updated_at.is_some()
        || entry.head_oid.is_some()
        || !participant_text(&value.user.login, 255, false)
        || !participant_text(&value.user.display_name, 1024, false)
        || !participant_text(&value.role, 128, true)
        || !participant_text(&value.state, 128, true)
        || value
            .participated_at
            .as_ref()
            .is_some_and(|at| !timestamp_valid(at))
        || entry.field_mask.contains(&DetailField::ParticipantApproved) && value.approved.is_none()
        || entry.field_mask.contains(&DetailField::ParticipantRole) && value.role.is_none()
    {
        return Err(invalid_detail());
    }
    Ok(())
}

fn review_actor_valid(actor: &Option<crate::ReviewActor>) -> bool {
    actor.as_ref().is_none_or(crate::reviews::actor_valid)
}

fn review_id_valid(value: &str) -> bool {
    crate::reviews::bounded_identity(value, 512)
}

fn validate_review(facet: DetailFacet, entry: &DetailEntry) -> Result<()> {
    let valid = match (&entry.native, facet) {
        (Some(crate::NativeDetailPayload::ReviewV1(review)), DetailFacet::ReviewSummaries) => {
            review.context.is_valid()
                && review_actor_valid(&review.reviewer)
                && crate::reviews::bounded_identity(&review.provider_state, 256)
                && review
                    .reviewed_commit_oid
                    .as_deref()
                    .is_none_or(crate::is_canonical_commit_oid)
                && review.submitted_at.as_deref().is_none_or(timestamp_valid)
                && entry.state.as_ref() == Some(&review.provider_state)
                && entry.updated_at == review.submitted_at
                && entry.author
                    == review
                        .reviewer
                        .as_ref()
                        .and_then(|actor| actor.login.clone())
                && entry.head_oid.as_ref() == Some(&review.context.head_oid)
        }
        (Some(crate::NativeDetailPayload::ReviewThreadV1(thread)), DetailFacet::ReviewThreads) => {
            thread.context.is_valid()
                && review_id_valid(&thread.thread_id)
                && thread
                    .root_comment_id
                    .as_deref()
                    .is_none_or(review_id_valid)
                && review_id_valid(&thread.comment_id)
                && thread
                    .parent_comment_id
                    .as_deref()
                    .is_none_or(review_id_valid)
                && thread.review_id.as_deref().is_none_or(review_id_valid)
                && thread.parent_comment_id.as_ref() != Some(&thread.comment_id)
                && (thread.parent_comment_id.is_some()
                    || thread
                        .root_comment_id
                        .as_ref()
                        .is_none_or(|root| root == &thread.comment_id))
        }
        _ => false,
    };
    if !valid {
        return Err(invalid_detail());
    }
    if let Some(crate::NativeDetailPayload::ReviewThreadV1(thread)) = &entry.native {
        let anchor_valid = thread.anchor.as_ref().is_none_or(|anchor| {
            crate::reviews::bounded_identity(&anchor.path, 4096)
                && crate::is_canonical_commit_oid(&anchor.commit_oid)
                && crate::is_canonical_commit_oid(&anchor.original_commit_oid)
                && [anchor.start_line, anchor.line]
                    .into_iter()
                    .flatten()
                    .all(|line| line > 0)
        });
        if !review_actor_valid(&thread.author)
            || !timestamp_valid(&thread.created_at)
            || !timestamp_valid(&thread.updated_at)
            || !anchor_valid
            || thread
                .native
                .as_deref()
                .is_some_and(|native| !native.is_valid())
            || entry.author != thread.author.as_ref().and_then(|actor| actor.login.clone())
            || entry.updated_at.as_ref() != Some(&thread.updated_at)
            || entry.head_oid.as_ref() != Some(&thread.context.head_oid)
        {
            return Err(invalid_detail());
        }
    }
    if entry.title.is_some()
        || entry.body.state == DetailValueState::NotLoaded
        || entry.observed_body_state != entry.body.state
    {
        return Err(invalid_detail());
    }
    Ok(())
}

fn validate_review_input(facet: DetailFacet, entry: &DetailEntry) -> Result<()> {
    let expected: &[DetailField] = match facet {
        DetailFacet::ReviewSummaries => &[
            DetailField::Body,
            DetailField::Author,
            DetailField::State,
            DetailField::UpdatedAt,
            DetailField::HeadOid,
            DetailField::Review,
        ],
        DetailFacet::ReviewThreads => &[
            DetailField::Body,
            DetailField::Author,
            DetailField::UpdatedAt,
            DetailField::HeadOid,
            DetailField::ReviewThread,
        ],
        _ => return Err(invalid_detail()),
    };
    if entry.field_mask.len() != expected.len()
        || expected
            .iter()
            .any(|field| !entry.field_mask.contains(field))
        || !entry.field_validations.is_empty()
    {
        return Err(invalid_detail());
    }
    Ok(())
}

fn check_text(value: &str, limit: usize, nonempty: bool) -> bool {
    value.len() <= limit && (!nonempty || !value.is_empty()) && !value.chars().any(char::is_control)
}

fn validate_check(entry: &DetailEntry) -> Result<()> {
    let Some(crate::NativeDetailPayload::CheckV1(check)) = &entry.native else {
        return Err(invalid_detail());
    };
    validate_value(&check.description)?;
    let state_valid = match (&check.kind, &check.state) {
        (crate::CheckKind::CheckRun, crate::CheckStateV1::CheckRun { status, conclusion }) => {
            check_text(status, 256, true)
                && conclusion
                    .as_deref()
                    .is_none_or(|value| check_text(value, 256, true))
                && check.allow_failure.is_none()
        }
        (crate::CheckKind::CommitStatus, crate::CheckStateV1::CommitStatus { state }) => {
            check_text(state, 256, true)
        }
        _ => false,
    };
    if !state_valid
        || !check_text(&check.name, 16_384, true)
        || check
            .description
            .text
            .as_ref()
            .is_some_and(|value| value.len() > MAX_ENTRY_BODY_BYTES)
        || check
            .producer
            .as_deref()
            .is_some_and(|value| !check_text(value, 1024, false))
        || [&check.started_at, &check.completed_at, &check.updated_at]
            .into_iter()
            .any(|value| value.as_deref().is_some_and(|at| !timestamp_valid(at)))
        || entry.author.is_some()
        || entry.title.is_some()
        || entry.state.is_some()
        || entry.body != DetailValue::default()
        || entry.observed_body_state != DetailValueState::NotLoaded
        || entry.updated_at.is_some()
        || entry.head_oid.is_none()
    {
        return Err(invalid_detail());
    }
    Ok(())
}

fn validate_check_input(entry: &DetailEntry) -> Result<()> {
    if entry.field_mask.len() != 2
        || !entry.field_mask.contains(&DetailField::Check)
        || !entry.field_mask.contains(&DetailField::HeadOid)
        || !entry.field_validations.is_empty()
    {
        return Err(invalid_detail());
    }
    Ok(())
}

fn validate_task_actor(actor: &crate::TaskActor) -> Result<()> {
    validate_identifier(&actor.provider_id)?;
    if actor.provider_id.chars().any(char::is_control)
        || actor.kind.is_empty()
        || actor.kind.len() > 128
        || actor.kind.chars().any(char::is_control)
        || !participant_text(&actor.login, 255, false)
        || !participant_text(&actor.display_name, 1024, false)
    {
        return Err(invalid_detail());
    }
    Ok(())
}

fn validate_task(entry: &DetailEntry) -> Result<()> {
    let Some(crate::NativeDetailPayload::TaskV1(task)) = &entry.native else {
        return Err(invalid_detail());
    };
    validate_task_actor(&task.creator)?;
    if let Some(resolver) = &task.resolved_by {
        validate_task_actor(resolver)?;
    }
    validate_value(&task.content)?;
    if entry.author.is_some()
        || entry.title.is_some()
        || entry.state.is_some()
        || entry.body != DetailValue::default()
        || entry.observed_body_state != DetailValueState::NotLoaded
        || entry.updated_at.is_some()
        || entry.head_oid.is_some()
        || task.content.state == DetailValueState::Known && task.content.text.is_none()
        || task
            .content
            .text
            .as_ref()
            .is_some_and(|text| text.len() > MAX_ENTRY_BODY_BYTES)
        || !participant_text(&task.state, 128, true)
        || [&task.created_at, &task.updated_at, &task.resolved_at]
            .into_iter()
            .any(|at| at.as_ref().is_some_and(|at| !timestamp_valid(at)))
        || task.comment_id.as_ref().is_some_and(|id| {
            id.parse::<i64>()
                .ok()
                .is_none_or(|value| value <= 0 || value.to_string() != *id)
        })
        || entry.field_mask.contains(&DetailField::TaskState) && task.state.is_none()
        || entry.field_mask.contains(&DetailField::TaskCreatedAt) && task.created_at.is_none()
        || entry.field_mask.contains(&DetailField::TaskUpdatedAt) && task.updated_at.is_none()
        || entry.field_mask.contains(&DetailField::TaskPending) && task.pending.is_none()
    {
        return Err(invalid_detail());
    }
    Ok(())
}

fn validate_task_input(entry: &DetailEntry) -> Result<()> {
    let Some(crate::NativeDetailPayload::TaskV1(task)) = &entry.native else {
        return Err(invalid_detail());
    };
    // Normalize oversized Known text only after its incoming value shape is
    // valid; a huge text member on Omitted must not become a valid Oversized.
    validate_value(&task.content)?;
    if ![
        DetailField::TaskContent,
        DetailField::TaskState,
        DetailField::TaskCreatedAt,
        DetailField::TaskUpdatedAt,
    ]
    .into_iter()
    .all(|field| entry.field_mask.contains(&field))
        || task.content.state == DetailValueState::NotLoaded
        || task.observed_content_state != task.content.state
        || task.resolved_by.is_some() && !entry.field_mask.contains(&DetailField::TaskResolver)
        || [
            DetailField::TaskResolverLogin,
            DetailField::TaskResolverDisplayName,
        ]
        .into_iter()
        .any(|field| entry.field_mask.contains(&field))
            && !entry.field_mask.contains(&DetailField::TaskResolver)
        || entry.field_mask.contains(&DetailField::TaskResolver)
            && task.resolved_by.is_none()
            && ![
                DetailField::TaskResolverLogin,
                DetailField::TaskResolverDisplayName,
            ]
            .into_iter()
            .all(|field| entry.field_mask.contains(&field))
        || !entry.field_validations.is_empty()
    {
        return Err(invalid_detail());
    }
    Ok(())
}

fn blank_native(native: &Option<crate::NativeDetailPayload>) -> Option<crate::NativeDetailPayload> {
    native.as_ref().map(|native| match native {
        crate::NativeDetailPayload::ActivityV1(value) => {
            crate::NativeDetailPayload::ActivityV1(value.clone())
        }
        crate::NativeDetailPayload::ParticipantV1(value) => {
            crate::NativeDetailPayload::ParticipantV1(crate::ParticipantV1 {
                user: crate::ParticipantUser {
                    provider_id: value.user.provider_id.clone(),
                    login: None,
                    display_name: None,
                },
                role: None,
                approved: None,
                state: None,
                participated_at: None,
            })
        }
        crate::NativeDetailPayload::TaskV1(value) => {
            crate::NativeDetailPayload::TaskV1(crate::TaskV1 {
                content: DetailValue::default(),
                observed_content_state: DetailValueState::NotLoaded,
                creator: crate::TaskActor {
                    provider_id: value.creator.provider_id.clone(),
                    kind: value.creator.kind.clone(),
                    login: None,
                    display_name: None,
                },
                state: None,
                created_at: None,
                updated_at: None,
                pending: None,
                resolved_at: None,
                resolved_by: None,
                comment_id: None,
            })
        }
        crate::NativeDetailPayload::CheckV1(value) => {
            crate::NativeDetailPayload::CheckV1(value.clone())
        }
        crate::NativeDetailPayload::ReviewV1(value) => {
            crate::NativeDetailPayload::ReviewV1(value.clone())
        }
        crate::NativeDetailPayload::ReviewThreadV1(value) => {
            crate::NativeDetailPayload::ReviewThreadV1(value.clone())
        }
    })
}

fn merge_native_field(
    saved: &mut DetailEntry,
    incoming: &DetailEntry,
    field: DetailField,
) -> Result<()> {
    let (
        Some(crate::NativeDetailPayload::ParticipantV1(saved)),
        Some(crate::NativeDetailPayload::ParticipantV1(incoming)),
    ) = (&mut saved.native, &incoming.native)
    else {
        return Err(invalid_detail());
    };
    match field {
        DetailField::ParticipantLogin => saved.user.login = incoming.user.login.clone(),
        DetailField::ParticipantDisplayName => {
            saved.user.display_name = incoming.user.display_name.clone()
        }
        DetailField::ParticipantRole => saved.role = incoming.role.clone(),
        DetailField::ParticipantApproved => saved.approved = incoming.approved,
        DetailField::ParticipantState => saved.state = incoming.state.clone(),
        DetailField::ParticipantParticipatedAt => {
            saved.participated_at = incoming.participated_at.clone()
        }
        _ => return Err(invalid_detail()),
    }
    Ok(())
}

fn task_actor_identity(actor: &Option<crate::TaskActor>) -> Option<(&str, &str)> {
    actor
        .as_ref()
        .map(|actor| (actor.provider_id.as_str(), actor.kind.as_str()))
}

fn merge_task_field(
    saved: &mut DetailEntry,
    incoming: &DetailEntry,
    field: DetailField,
) -> Result<()> {
    let (
        Some(crate::NativeDetailPayload::TaskV1(saved)),
        Some(crate::NativeDetailPayload::TaskV1(incoming)),
    ) = (&mut saved.native, &incoming.native)
    else {
        return Err(invalid_detail());
    };
    match field {
        DetailField::TaskContent => saved.content = incoming.content.clone(),
        DetailField::TaskCreatorLogin => saved.creator.login = incoming.creator.login.clone(),
        DetailField::TaskCreatorDisplayName => {
            saved.creator.display_name = incoming.creator.display_name.clone()
        }
        DetailField::TaskState => saved.state = incoming.state.clone(),
        DetailField::TaskCreatedAt => saved.created_at = incoming.created_at.clone(),
        DetailField::TaskUpdatedAt => saved.updated_at = incoming.updated_at.clone(),
        DetailField::TaskPending => saved.pending = incoming.pending,
        DetailField::TaskResolvedAt => saved.resolved_at = incoming.resolved_at.clone(),
        DetailField::TaskResolver => {
            if task_actor_identity(&saved.resolved_by) != task_actor_identity(&incoming.resolved_by)
            {
                saved.resolved_by = incoming.resolved_by.as_ref().map(|actor| crate::TaskActor {
                    provider_id: actor.provider_id.clone(),
                    kind: actor.kind.clone(),
                    login: None,
                    display_name: None,
                });
            }
        }
        DetailField::TaskResolverLogin | DetailField::TaskResolverDisplayName => {
            if let Some(saved) = &mut saved.resolved_by {
                let Some(incoming) = &incoming.resolved_by else {
                    return Err(invalid_detail());
                };
                if field == DetailField::TaskResolverLogin {
                    saved.login = incoming.login.clone();
                } else {
                    saved.display_name = incoming.display_name.clone();
                }
            }
        }
        DetailField::TaskCommentId => saved.comment_id = incoming.comment_id.clone(),
        _ => return Err(invalid_detail()),
    }
    Ok(())
}

fn merge_check_field(saved: &mut DetailEntry, incoming: &DetailEntry) -> Result<()> {
    let (
        Some(crate::NativeDetailPayload::CheckV1(saved)),
        Some(crate::NativeDetailPayload::CheckV1(incoming)),
    ) = (&mut saved.native, &incoming.native)
    else {
        return Err(invalid_detail());
    };
    *saved = incoming.clone();
    Ok(())
}

fn merge_review_field(
    saved: &mut DetailEntry,
    incoming: &DetailEntry,
    field: DetailField,
) -> Result<()> {
    match (&mut saved.native, &incoming.native, field) {
        (
            Some(crate::NativeDetailPayload::ReviewV1(saved)),
            Some(crate::NativeDetailPayload::ReviewV1(incoming)),
            DetailField::Review,
        ) => *saved = incoming.clone(),
        (
            Some(crate::NativeDetailPayload::ReviewThreadV1(saved)),
            Some(crate::NativeDetailPayload::ReviewThreadV1(incoming)),
            DetailField::ReviewThread,
        ) => *saved = incoming.clone(),
        _ => return Err(invalid_detail()),
    }
    Ok(())
}

fn merge_entry(
    facet: DetailFacet,
    mut incoming: DetailEntry,
    previous: Option<StoredEntry>,
    source: &DetailSource,
    head: Option<&str>,
) -> Result<StoredEntry> {
    validate_identifier(&incoming.id)?;
    validate_identifier(&incoming.provider_id)?;
    validate_value(&incoming.body)?;
    if facet == DetailFacet::Tasks {
        validate_task_input(&incoming)?;
        if let Some(crate::NativeDetailPayload::TaskV1(task)) = &mut incoming.native
            && task
                .content
                .text
                .as_ref()
                .is_some_and(|value| value.len() > MAX_ENTRY_BODY_BYTES)
        {
            task.content = DetailValue {
                state: DetailValueState::Oversized,
                text: None,
            };
            task.observed_content_state = DetailValueState::Oversized;
        }
    }
    if facet == DetailFacet::Checks {
        validate_check_input(&incoming)?;
    }
    if is_review_facet(facet) {
        validate_review_input(facet, &incoming)?;
    }
    if facet == DetailFacet::Participants && !incoming.field_validations.is_empty() {
        return Err(invalid_detail());
    }
    validate_native(facet, &incoming)?;
    if !mask_valid(facet, &incoming.field_mask)
        || !bounded(&incoming.author, 1024)
        || !bounded(&incoming.title, 16_384)
        || !bounded(&incoming.state, 256)
        || !bounded(&incoming.head_oid, 256)
        || incoming
            .updated_at
            .as_ref()
            .is_some_and(|s| !timestamp_valid(s))
    {
        return Err(invalid_detail());
    }
    if incoming
        .body
        .text
        .as_ref()
        .is_some_and(|v| v.len() > MAX_ENTRY_BODY_BYTES)
    {
        incoming.body = DetailValue {
            state: DetailValueState::Oversized,
            text: None,
        };
    }
    let mut saved = previous.unwrap_or_else(|| {
        if is_review_facet(facet) {
            // A review native payload and its generic projection are one typed
            // observation. Seed a new row from that internally consistent
            // value, then let the ordinary field loop attach source clocks.
            let mut entry = incoming.clone();
            entry.field_mask.clear();
            entry.field_validations.clear();
            return StoredEntry::from_entry(entry);
        }
        StoredEntry::from_entry(DetailEntry {
            id: incoming.id.clone(),
            provider_id: incoming.provider_id.clone(),
            author: None,
            title: None,
            state: None,
            body: DetailValue::default(),
            observed_body_state: DetailValueState::NotLoaded,
            updated_at: None,
            head_oid: if facet == DetailFacet::Checks {
                incoming.head_oid.clone()
            } else {
                None
            },
            native: blank_native(&incoming.native),
            field_mask: vec![],
            field_validations: vec![],
        })
    });
    validate_native(facet, &saved.entry)?;
    if !mask_valid(facet, &saved.entry.field_mask)
        || saved.entry.field_validations.len() > facet.field_limit()
        || saved
            .entry
            .field_validations
            .iter()
            .enumerate()
            .any(|(index, validation)| {
                !validation.field.valid_for(facet)
                    || saved.entry.field_validations[..index]
                        .iter()
                        .any(|old| old.field == validation.field)
            })
    {
        return Err(invalid_detail());
    }
    saved.initialize_legacy(facet);
    let same_native_identity = match (&saved.entry.native, &incoming.native) {
        (
            Some(crate::NativeDetailPayload::ActivityV1(old)),
            Some(crate::NativeDetailPayload::ActivityV1(new)),
        ) => old.kind == new.kind,
        (None, None) => true,
        (
            Some(crate::NativeDetailPayload::ParticipantV1(old)),
            Some(crate::NativeDetailPayload::ParticipantV1(new)),
        ) => old.user.provider_id == new.user.provider_id,
        (
            Some(crate::NativeDetailPayload::TaskV1(old)),
            Some(crate::NativeDetailPayload::TaskV1(new)),
        ) => {
            old.creator.provider_id == new.creator.provider_id
                && old.creator.kind == new.creator.kind
        }
        (
            Some(crate::NativeDetailPayload::CheckV1(old)),
            Some(crate::NativeDetailPayload::CheckV1(new)),
        ) => old.kind == new.kind,
        (
            Some(crate::NativeDetailPayload::ReviewV1(_)),
            Some(crate::NativeDetailPayload::ReviewV1(_)),
        ) => true,
        (
            Some(crate::NativeDetailPayload::ReviewThreadV1(old)),
            Some(crate::NativeDetailPayload::ReviewThreadV1(new)),
        ) => {
            old.thread_id == new.thread_id
                && old.root_comment_id == new.root_comment_id
                && old.comment_id == new.comment_id
        }
        _ => false,
    };
    if saved.entry.provider_id != incoming.provider_id || !same_native_identity {
        return Err(invalid_detail());
    }
    let comparable = if let Some(crate::NativeDetailPayload::TaskV1(task)) = &incoming.native {
        task.updated_at.clone()
    } else if let Some(crate::NativeDetailPayload::CheckV1(check)) = &incoming.native {
        check.updated_at.clone()
    } else if let Some(crate::NativeDetailPayload::ReviewV1(review)) = &incoming.native {
        review.submitted_at.clone()
    } else if let Some(crate::NativeDetailPayload::ReviewThreadV1(thread)) = &incoming.native {
        Some(thread.updated_at.clone())
    } else if incoming.field_mask.contains(&DetailField::UpdatedAt) {
        incoming
            .updated_at
            .clone()
            .or_else(|| source.provider_updated_at.clone())
    } else {
        source.provider_updated_at.clone()
    };
    saved.entry.observed_body_state = if incoming.field_mask.contains(&DetailField::Body) {
        incoming.body.state
    } else {
        DetailValueState::NotLoaded
    };
    if facet == DetailFacet::Tasks {
        let (
            Some(crate::NativeDetailPayload::TaskV1(old)),
            Some(crate::NativeDetailPayload::TaskV1(new)),
        ) = (&mut saved.entry.native, &incoming.native)
        else {
            return Err(invalid_detail());
        };
        old.observed_content_state = new.content.state;
        // Resolver identity must be reconciled before presentations, regardless
        // of the adapter's mask order. Old-person clocks cannot authorize names.
        if incoming.field_mask.contains(&DetailField::TaskResolver)
            && !saved.older(
                DetailField::TaskResolver,
                comparable.as_deref(),
                source,
                head,
            )
        {
            let changed = match (&saved.entry.native, &incoming.native) {
                (
                    Some(crate::NativeDetailPayload::TaskV1(old)),
                    Some(crate::NativeDetailPayload::TaskV1(new)),
                ) => task_actor_identity(&old.resolved_by) != task_actor_identity(&new.resolved_by),
                _ => return Err(invalid_detail()),
            };
            merge_task_field(&mut saved.entry, &incoming, DetailField::TaskResolver)?;
            if changed {
                saved.forget(&[
                    DetailField::TaskResolverLogin,
                    DetailField::TaskResolverDisplayName,
                ]);
            }
            saved.observed(
                DetailField::TaskResolver,
                comparable.as_deref(),
                source,
                head,
            );
            saved
                .entry
                .field_validations
                .retain(|v| v.field != DetailField::TaskResolver);
            saved.entry.field_validations.push(DetailFieldValidation {
                field: DetailField::TaskResolver,
                validated_at: source.observed_at.clone(),
                source: source.source.clone(),
                adapter_version: source.adapter_version,
            });
        }
        let same_resolver = match (&saved.entry.native, &incoming.native) {
            (
                Some(crate::NativeDetailPayload::TaskV1(old)),
                Some(crate::NativeDetailPayload::TaskV1(new)),
            ) => task_actor_identity(&old.resolved_by) == task_actor_identity(&new.resolved_by),
            _ => return Err(invalid_detail()),
        };
        if !same_resolver {
            incoming.field_mask.retain(|field| {
                !matches!(
                    field,
                    DetailField::TaskResolver
                        | DetailField::TaskResolverLogin
                        | DetailField::TaskResolverDisplayName
                )
            });
        }
    }
    for field in &incoming.field_mask {
        if facet == DetailFacet::Tasks && *field == DetailField::TaskResolver {
            continue;
        }
        if saved.older(*field, comparable.as_deref(), source, head) {
            continue;
        }
        match field {
            DetailField::Body if incoming.body.state == DetailValueState::Known => {
                saved.entry.body = incoming.body.clone()
            }
            DetailField::Body => {
                if saved.entry.body.state != DetailValueState::Known {
                    saved.entry.body = incoming.body.clone();
                }
                continue;
            }
            DetailField::Author => saved.entry.author = incoming.author.clone(),
            DetailField::Title => saved.entry.title = incoming.title.clone(),
            DetailField::State => saved.entry.state = incoming.state.clone(),
            DetailField::UpdatedAt => saved.entry.updated_at = incoming.updated_at.clone(),
            DetailField::HeadOid => saved.entry.head_oid = incoming.head_oid.clone(),
            field if field.is_participant() => {
                merge_native_field(&mut saved.entry, &incoming, *field)?
            }
            DetailField::TaskContent => {
                let Some(crate::NativeDetailPayload::TaskV1(task)) = &mut saved.entry.native else {
                    return Err(invalid_detail());
                };
                let Some(crate::NativeDetailPayload::TaskV1(observed)) = &incoming.native else {
                    return Err(invalid_detail());
                };
                if observed.content.state != DetailValueState::Known {
                    if task.content.state != DetailValueState::Known {
                        task.content = observed.content.clone();
                    }
                    continue;
                }
                merge_task_field(&mut saved.entry, &incoming, *field)?;
            }
            field if field.is_task() => merge_task_field(&mut saved.entry, &incoming, *field)?,
            DetailField::Activity => {
                let (
                    Some(crate::NativeDetailPayload::ActivityV1(saved)),
                    Some(crate::NativeDetailPayload::ActivityV1(observed)),
                ) = (&mut saved.entry.native, &incoming.native)
                else {
                    return Err(invalid_detail());
                };
                *saved = observed.clone();
            }
            DetailField::Check => merge_check_field(&mut saved.entry, &incoming)?,
            DetailField::Review | DetailField::ReviewThread => {
                merge_review_field(&mut saved.entry, &incoming, *field)?
            }
            _ => return Err(invalid_detail()),
        }
        saved.observed(*field, comparable.as_deref(), source, head);
        saved.entry.field_validations.retain(|v| v.field != *field);
        saved.entry.field_validations.push(DetailFieldValidation {
            field: *field,
            validated_at: source.observed_at.clone(),
            source: source.source.clone(),
            adapter_version: source.adapter_version,
        });
    }
    saved.entry.field_mask = incoming.field_mask;
    Ok(saved)
}

/// A native read/cleanup belongs to this exact committed traversal state. Check
/// after opening the transaction (and, for writes, acquiring the writer), so an
/// obsolete page cannot be dispatched or clear an independently replaced intent.
async fn validate_lease_in(
    tx: &mut Transaction<'_, Sqlite>,
    account_id: &str,
    epoch: &str,
    subject_id: &str,
    facet: DetailFacet,
    lease: &DetailLease,
) -> Result<RemoteItem> {
    epoch_in(tx, account_id, epoch).await?;
    let account = account_in(tx, account_id, true).await?;
    let subject = subject_in(tx, account_id, subject_id).await?;
    if facet.capability(&subject.kind).is_none() {
        return Err(invalid_detail());
    }
    if metadata(tx).await?.1 != lease.authorization_view
        || identities::instance_in(tx, &account).await?.id != lease.instance_id
    {
        return Err(stale());
    }
    let scope = facet.scope(subject_id);
    let stored = scope_in(tx, account_id, &scope).await?.ok_or_else(stale)?;
    let denied: bool =
        sqlx::query_scalar("SELECT access_denied FROM sync_scopes WHERE account_id=? AND scope=?")
            .bind(account_id)
            .bind(&scope)
            .fetch_one(&mut **tx)
            .await
            .map_err(storage_error)?;
    if denied
        || stored.run_id != lease.run_id
        || stored.next_cursor != lease.next_cursor
        || stored.etag != lease.etag
    {
        return Err(stale());
    }
    let source: Option<String> = sqlx::query_scalar("SELECT source_json FROM detail_observations WHERE account_id=? AND subject_id=? AND facet=? AND authorization_epoch=?")
        .bind(account_id).bind(subject_id).bind(tag(&facet)?).bind(epoch).fetch_optional(&mut **tx).await.map_err(storage_error)?;
    let source: Option<StoredSource> = source.map(|json| decode(&json)).transpose()?;
    let proof = source.as_ref().and_then(StoredSource::traversal);
    // begin_detail drops old traversal tokens when the retained observation is
    // outside the current head/context. The replacement still has to match the
    // durable run/cursor/authorization above, but the historical proof must not
    // veto the fresh traversal it caused.
    let replaces_incomparable_scope = source.is_some()
        && lease.next_cursor.is_none()
        && lease.etag.is_none()
        && lease.source.is_none()
        && lease.reconciliation.is_none();
    let reconciliation = lease.reconciliation.map(|value| {
        if facet == DetailFacet::Body {
            DetailReconciliation {
                enumeration: DetailEnumeration::Uncertain,
                ..value
            }
        } else {
            value
        }
    });
    let exact_context_changed = if !replaces_incomparable_scope && proof.is_some() {
        if facet == DetailFacet::Checks {
            match super::pull_commits::check_context_in(tx, account_id, subject_id, false).await {
                Ok(context) => {
                    proof.and_then(|proof| proof.check_context.as_ref()) != Some(&context)
                }
                Err(error) if matches!(error.code, ErrorCode::NotFound | ErrorCode::StaleView) => {
                    true
                }
                Err(error) => return Err(error),
            }
        } else if is_review_facet(facet) {
            match super::pull_commits::review_context_in(tx, account_id, subject_id, false).await {
                Ok(context) => {
                    proof.and_then(|proof| proof.review_context.as_ref()) != Some(&context)
                }
                Err(error) if matches!(error.code, ErrorCode::NotFound | ErrorCode::StaleView) => {
                    true
                }
                Err(error) => return Err(error),
            }
        } else {
            false
        }
    } else {
        false
    };
    if !replaces_incomparable_scope
        && (source.as_ref().map(|source| &source.source) != lease.source.as_ref()
            || proof.map(|proof| proof.reconciliation) != reconciliation
            || exact_context_changed
            || proof.is_some_and(|proof| {
                proof.reconciliation.head_scope == DetailHeadScope::CurrentHead
                    && proof.head_oid != subject.head_oid
            }))
    {
        return Err(stale());
    }
    validate_declared_head_in(tx, &account, &subject, facet, reconciliation).await?;
    Ok(subject)
}

impl Store {
    pub(crate) async fn check_context(
        &self,
        account_id: &str,
        subject_id: &str,
    ) -> Result<crate::CheckContext> {
        let mut tx = self.inner.readers.begin().await.map_err(storage_error)?;
        let context =
            super::pull_commits::check_context_in(&mut tx, account_id, subject_id, true).await?;
        tx.commit().await.map_err(storage_error)?;
        Ok(context)
    }

    pub(crate) async fn validate_check_dispatch(
        &self,
        account_id: &str,
        epoch: &str,
        subject_id: &str,
        lease: &DetailLease,
        binding: &DetailSubjectBinding,
    ) -> Result<crate::CheckContext> {
        let mut tx = self.inner.readers.begin().await.map_err(storage_error)?;
        let subject = validate_lease_in(
            &mut tx,
            account_id,
            epoch,
            subject_id,
            DetailFacet::Checks,
            lease,
        )
        .await?;
        super::resource_metadata::validate_binding_in(&mut tx, account_id, &subject, binding)
            .await?;
        let context =
            super::pull_commits::check_context_in(&mut tx, account_id, subject_id, true).await?;
        if binding.head_oid.as_ref() != Some(&context.head_oid) {
            return Err(stale());
        }
        tx.commit().await.map_err(storage_error)?;
        Ok(context)
    }

    pub(crate) async fn review_context(
        &self,
        account_id: &str,
        subject_id: &str,
    ) -> Result<crate::ReviewContext> {
        let mut tx = self.inner.readers.begin().await.map_err(storage_error)?;
        let context =
            super::pull_commits::review_context_in(&mut tx, account_id, subject_id, true).await?;
        tx.commit().await.map_err(storage_error)?;
        Ok(context)
    }

    pub(crate) async fn validate_review_dispatch(
        &self,
        account_id: &str,
        epoch: &str,
        subject_id: &str,
        facet: DetailFacet,
        lease: &DetailLease,
        binding: &DetailSubjectBinding,
    ) -> Result<crate::ReviewContext> {
        if !is_review_facet(facet) {
            return Err(invalid_detail());
        }
        let mut tx = self.inner.readers.begin().await.map_err(storage_error)?;
        let subject =
            validate_lease_in(&mut tx, account_id, epoch, subject_id, facet, lease).await?;
        super::resource_metadata::validate_binding_in(&mut tx, account_id, &subject, binding)
            .await?;
        let context =
            super::pull_commits::review_context_in(&mut tx, account_id, subject_id, true).await?;
        if binding.head_oid.as_ref() != Some(&context.head_oid) {
            return Err(stale());
        }
        tx.commit().await.map_err(storage_error)?;
        Ok(context)
    }

    pub(crate) async fn validate_detail_dispatch(
        &self,
        account_id: &str,
        epoch: &str,
        subject_id: &str,
        facet: DetailFacet,
        lease: &DetailLease,
        binding: &DetailSubjectBinding,
    ) -> Result<()> {
        let mut tx = self.inner.readers.begin().await.map_err(storage_error)?;
        let subject =
            validate_lease_in(&mut tx, account_id, epoch, subject_id, facet, lease).await?;
        super::resource_metadata::validate_binding_in(&mut tx, account_id, &subject, binding)
            .await?;
        tx.commit().await.map_err(storage_error)?;
        Ok(())
    }
    pub async fn detail_subject(&self, account_id: &str, subject_id: &str) -> Result<RemoteItem> {
        let mut tx = self.inner.readers.begin().await.map_err(storage_error)?;
        account_in(&mut tx, account_id, true).await?;
        let item = subject_in(&mut tx, account_id, subject_id).await?;
        tx.commit().await.map_err(storage_error)?;
        Ok(item)
    }
    pub async fn detail_evidence(
        &self,
        account_id: &str,
        subject_id: &str,
        facet: DetailFacet,
    ) -> Result<DetailEvidence> {
        let mut tx = self.inner.readers.begin().await.map_err(storage_error)?;
        let account = account_in(&mut tx, account_id, false).await?;
        let result = detail_evidence_in(&mut tx, &account, subject_id, facet).await?;
        tx.commit().await.map_err(storage_error)?;
        Ok(result)
    }
    pub(crate) async fn demand_detail_state(
        &self,
        account_id: &str,
        subject_id: &str,
        facet: DetailFacet,
    ) -> Result<(DetailEvidence, bool, Vec<String>)> {
        let mut tx = self.inner.readers.begin().await.map_err(storage_error)?;
        let account = account_in(&mut tx, account_id, false).await?;
        let evidence = detail_evidence_in(&mut tx, &account, subject_id, facet).await?;
        let mut present = false;
        let mut deadlines = vec![];
        if facet == DetailFacet::Body && evidence.availability != DetailAvailability::Unavailable {
            present = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM detail_resource_metadata WHERE account_id=? AND subject_id=? AND authorization_epoch=?)")
                .bind(account_id).bind(subject_id).bind(&account.authorization_epoch).fetch_one(&mut *tx).await.map_err(storage_error)?;
            deadlines = sqlx::query_scalar("SELECT json_extract(f.value,'$.stale_at') FROM detail_resource_metadata m,json_each(m.metadata_json,'$.fields') f WHERE m.account_id=? AND m.subject_id=? AND m.authorization_epoch=? AND json_extract(f.value,'$.observed_state')='known' AND json_extract(f.value,'$.saved_state')='known' AND json_extract(f.value,'$.stale_at') IS NOT NULL LIMIT 13")
                .bind(account_id).bind(subject_id).bind(&account.authorization_epoch).fetch_all(&mut *tx).await.map_err(storage_error)?;
        }
        tx.commit().await.map_err(storage_error)?;
        Ok((evidence, present, deadlines))
    }
    pub async fn detail(&self, query: DetailQuery) -> Result<DetailSnapshot> {
        if query.limit == 0
            || query.limit > 100
            || query.cursor.as_ref().is_some_and(|c| c.len() > 4096)
        {
            return Err(invalid_detail());
        }
        validate_identifier(&query.subject_id)?;
        let mut tx = self.inner.readers.begin().await.map_err(storage_error)?;
        let account = account_in(&mut tx, &query.account_id, false).await?;
        let (revision, authorization_view) = metadata(&mut tx).await?;
        let evidence =
            detail_evidence_in(&mut tx, &account, &query.subject_id, query.facet).await?;
        let mut result = DetailSnapshot {
            subject_id: query.subject_id.clone(),
            pending_intent: None,
            body: DetailValue::default(),
            metadata: None,
            entries: vec![],
            next_cursor: None,
            evidence,
            revision,
            authorization_view,
        };
        if result.evidence.availability == DetailAvailability::Unavailable {
            tx.commit().await.map_err(storage_error)?;
            return Ok(result);
        }
        if query.facet == DetailFacet::Body {
            result.metadata =
                super::resource_metadata::read_in(&mut tx, &account, &query.subject_id).await?;
        }
        let effective_revision = if query.facet == DetailFacet::Body {
            super::effective::subject_revision_in(&mut tx, &query.account_id, &query.subject_id)
                .await?
        } else {
            "0".into()
        };
        let mut after = String::new();
        let mut activity_after = String::new();
        if let Some(cursor) = query.cursor {
            let cursor: DetailCursor =
                serde_json::from_str(&cursor).map_err(|_| invalid_detail())?;
            if cursor.account != query.account_id
                || cursor.subject != query.subject_id
                || cursor.facet != query.facet
                || cursor.authorization_view != result.authorization_view
                || cursor.facet_revision != result.evidence.facet_revision
                || cursor.effective_revision != effective_revision
            {
                return Err(stale());
            }
            validate_identifier(&cursor.last_id)?;
            if query.facet == DetailFacet::Activity {
                let order = cursor.activity_order.ok_or_else(invalid_detail)?;
                if order != "~" && (!timestamp_valid(&order) || order.len() > 64) {
                    return Err(invalid_detail());
                }
                activity_after = order;
            } else if cursor.activity_order.is_some() {
                return Err(invalid_detail());
            }
            after = cursor.last_id;
        }
        if let Some(json)=sqlx::query_scalar::<_,String>("SELECT body_json FROM detail_observations WHERE account_id=? AND subject_id=? AND facet=?")
            .bind(&query.account_id).bind(&query.subject_id).bind(tag(&query.facet)?).fetch_optional(&mut *tx).await.map_err(storage_error)? { result.body=decode(&json)?; }
        let rows = if query.facet == DetailFacet::Activity {
            sqlx::query("SELECT id,json,coalesce(json_extract(json,'$.native.value.occurred_at'),'~') AS activity_order FROM detail_entries WHERE account_id=? AND subject_id=? AND facet='activity' AND (coalesce(json_extract(json,'$.native.value.occurred_at'),'~'),id)>(?,?) ORDER BY coalesce(json_extract(json,'$.native.value.occurred_at'),'~'),id LIMIT ?")
                .bind(&query.account_id).bind(&query.subject_id).bind(activity_after).bind(after).bind(i64::from(query.limit)+1).fetch_all(&mut *tx).await.map_err(storage_error)?
        } else {
            sqlx::query("SELECT id,json,NULL AS activity_order FROM detail_entries WHERE account_id=? AND subject_id=? AND facet=? AND id>? ORDER BY id LIMIT ?")
                .bind(&query.account_id).bind(&query.subject_id).bind(tag(&query.facet)?).bind(after).bind(i64::from(query.limit)+1).fetch_all(&mut *tx).await.map_err(storage_error)?
        };
        let has_more = rows.len() > query.limit as usize;
        let mut last_activity_order = None;
        for row in rows.into_iter().take(query.limit as usize) {
            last_activity_order = row.get::<Option<String>, _>("activity_order");
            result.entries.push(decode(row.get("json"))?);
        }
        if query.facet == DetailFacet::Body {
            result.pending_intent =
                super::effective::pending_in(&mut tx, &query.account_id, &[&query.subject_id])
                    .await?
                    .pop();
            if let Some(patch) =
                super::effective::patch_in(&mut tx, &query.account_id, &query.subject_id).await?
            {
                if let Some(body) = patch.body {
                    result.body = DetailValue {
                        state: crate::DetailValueState::Known,
                        text: body.text,
                    };
                }
                if let Some(metadata) = &mut result.metadata {
                    if let Some(title) = patch.title {
                        metadata.values.title = Some(title);
                    }
                    if let Some(state) = patch.state {
                        metadata.values.state = Some(state);
                    }
                }
            }
        }
        if has_more {
            result.next_cursor = Some(encode(&DetailCursor {
                account: query.account_id,
                subject: query.subject_id,
                facet: query.facet,
                authorization_view: result.authorization_view.clone(),
                facet_revision: result.evidence.facet_revision.clone(),
                effective_revision,
                last_id: result.entries.last().ok_or_else(invalid_detail)?.id.clone(),
                activity_order: last_activity_order,
            })?);
        }
        tx.commit().await.map_err(storage_error)?;
        Ok(result)
    }
    pub async fn request_detail(
        &self,
        account_id: &str,
        epoch: &str,
        subject_id: &str,
        facet: DetailFacet,
    ) -> Result<String> {
        let mut writer = self.inner.writer.acquire().await?;
        let mut tx = writer.begin().await.map_err(storage_error)?;
        epoch_in(&mut tx, account_id, epoch).await?;
        let subject = subject_in(&mut tx, account_id, subject_id).await?;
        if facet.capability(&subject.kind).is_none() {
            return Err(invalid_detail());
        }
        let account = account_in(&mut tx, account_id, true).await?;
        let source: Option<String> = sqlx::query_scalar("SELECT source_json FROM detail_observations WHERE account_id=? AND subject_id=? AND facet=?")
            .bind(account_id).bind(subject_id).bind(tag(&facet)?).fetch_optional(&mut *tx).await.map_err(storage_error)?;
        let source: Option<StoredSource> = source.map(|json| decode(&json)).transpose()?;
        validate_declared_head_in(
            &mut tx,
            &account,
            &subject,
            facet,
            source
                .as_ref()
                .and_then(StoredSource::traversal)
                .map(|proof| proof.reconciliation),
        )
        .await?;
        sqlx::query("INSERT INTO detail_demand(account_id,subject_id,facet,authorization_epoch) VALUES(?,?,?,?) ON CONFLICT(account_id,subject_id,facet) DO UPDATE SET authorization_epoch=excluded.authorization_epoch,requested=1")
            .bind(account_id).bind(subject_id).bind(tag(&facet)?).bind(epoch).execute(&mut *tx).await.map_err(storage_error)?;
        let revision = record_change(
            &mut tx,
            account_id,
            positive_revision(epoch)?,
            &facet.scope(subject_id),
            false,
        )
        .await?;
        tx.commit().await.map_err(storage_error)?;
        Ok(revision)
    }
    pub async fn pending_details(&self) -> Result<Vec<DetailDemand>> {
        let rows=sqlx::query("SELECT d.account_id,d.subject_id,d.facet FROM detail_demand d JOIN accounts a ON a.id=d.account_id AND CAST(a.authorization_epoch AS TEXT)=d.authorization_epoch WHERE d.requested=1 AND a.state='active' ORDER BY d.account_id,d.subject_id,d.facet LIMIT 128")
            .fetch_all(&self.inner.readers).await.map_err(storage_error)?;
        rows.into_iter()
            .map(|row| {
                Ok(DetailDemand {
                    account_id: row.get("account_id"),
                    subject_id: row.get("subject_id"),
                    facet: decode(&format!("\"{}\"", row.get::<String, _>("facet")))?,
                })
            })
            .collect()
    }
    pub(crate) async fn pending_detail_batch(
        &self,
        after: Option<&DetailDemand>,
    ) -> Result<Vec<DetailDemand>> {
        let mut tx = self.inner.readers.begin().await.map_err(storage_error)?;
        let mut rows = Vec::new();
        for pass in 0..2 {
            let mut sql = sqlx::QueryBuilder::<Sqlite>::new(
                "SELECT d.account_id,d.subject_id,d.facet FROM detail_demand d JOIN accounts a ON a.id=d.account_id AND CAST(a.authorization_epoch AS TEXT)=d.authorization_epoch WHERE d.requested=1 AND a.state='active'",
            );
            if pass == 0
                && let Some(after) = after
            {
                sql.push(" AND (d.account_id,d.subject_id,d.facet) > (")
                    .push_bind(&after.account_id)
                    .push(",")
                    .push_bind(&after.subject_id)
                    .push(",")
                    .push_bind(tag(&after.facet)?)
                    .push(")");
            }
            sql.push(" ORDER BY d.account_id,d.subject_id,d.facet LIMIT 16");
            rows = sql
                .build()
                .fetch_all(&mut *tx)
                .await
                .map_err(storage_error)?;
            if !rows.is_empty() || after.is_none() {
                break;
            }
        }
        let demands = rows
            .into_iter()
            .map(|row| {
                Ok(DetailDemand {
                    account_id: row.get("account_id"),
                    subject_id: row.get("subject_id"),
                    facet: decode(&format!("\"{}\"", row.get::<String, _>("facet")))?,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        tx.commit().await.map_err(storage_error)?;
        Ok(demands)
    }
    pub async fn stop_detail_demand(
        &self,
        account_id: &str,
        epoch: &str,
        subject_id: &str,
        facet: DetailFacet,
    ) -> Result<()> {
        let mut writer = self.inner.writer.acquire().await?;
        let mut tx = writer.begin().await.map_err(storage_error)?;
        epoch_in(&mut tx, account_id, epoch).await?;
        sqlx::query("UPDATE detail_demand SET requested=0 WHERE account_id=? AND subject_id=? AND facet=? AND authorization_epoch=?")
            .bind(account_id).bind(subject_id).bind(tag(&facet)?).bind(epoch).execute(&mut *tx).await.map_err(storage_error)?;
        tx.commit().await.map_err(storage_error)?;
        Ok(())
    }
    /// Stop only the captured repeatedly drifting job, never a replacement read.
    pub(crate) async fn stop_detail_reconciliation(
        &self,
        account_id: &str,
        epoch: &str,
        subject_id: &str,
        facet: DetailFacet,
        lease: &DetailLease,
    ) -> Result<()> {
        let mut writer = self.inner.writer.acquire().await?;
        let mut tx = writer.begin().await.map_err(storage_error)?;
        validate_lease_in(&mut tx, account_id, epoch, subject_id, facet, lease).await?;
        sqlx::query("UPDATE detail_demand SET requested=0 WHERE account_id=? AND subject_id=? AND facet=? AND authorization_epoch=?")
            .bind(account_id).bind(subject_id).bind(tag(&facet)?).bind(epoch).execute(&mut *tx).await.map_err(storage_error)?;
        tx.commit().await.map_err(storage_error)?;
        Ok(())
    }
    pub async fn begin_detail(
        &self,
        account_id: &str,
        epoch: &str,
        subject_id: &str,
        facet: DetailFacet,
    ) -> Result<DetailLease> {
        self.begin_detail_with_authorization_view(account_id, epoch, subject_id, facet, None)
            .await
    }
    pub(crate) async fn begin_detail_at_authorization_view(
        &self,
        account_id: &str,
        epoch: &str,
        subject_id: &str,
        facet: DetailFacet,
        authorization_view: &str,
    ) -> Result<DetailLease> {
        self.begin_detail_with_authorization_view(
            account_id,
            epoch,
            subject_id,
            facet,
            Some(authorization_view),
        )
        .await
    }
    async fn begin_detail_with_authorization_view(
        &self,
        account_id: &str,
        epoch: &str,
        subject_id: &str,
        facet: DetailFacet,
        expected_authorization_view: Option<&str>,
    ) -> Result<DetailLease> {
        if matches!(facet, DetailFacet::Commits | DetailFacet::Files) {
            return Err(invalid_detail());
        }
        let mut writer = self.inner.writer.acquire().await?;
        let mut tx = writer.begin().await.map_err(storage_error)?;
        epoch_in(&mut tx, account_id, epoch).await?;
        let (_, authorization_view) = metadata(&mut tx).await?;
        if expected_authorization_view
            .is_some_and(|expected| expected != authorization_view.as_str())
        {
            return Err(stale());
        }
        let account = account_in(&mut tx, account_id, true).await?;
        let subject = subject_in(&mut tx, account_id, subject_id).await?;
        if facet.capability(&subject.kind).is_none() {
            return Err(invalid_detail());
        }
        let scope = facet.scope(subject_id);
        let old = scope_in(&mut tx, account_id, &scope).await?;
        let source: Option<String> = sqlx::query_scalar("SELECT source_json FROM detail_observations WHERE account_id=? AND subject_id=? AND facet=?")
            .bind(account_id).bind(subject_id).bind(tag(&facet)?).fetch_optional(&mut *tx).await.map_err(storage_error)?;
        let source: Option<StoredSource> = source.map(|s| decode(&s)).transpose()?;
        validate_declared_head_in(
            &mut tx,
            &account,
            &subject,
            facet,
            source
                .as_ref()
                .and_then(StoredSource::traversal)
                .map(|proof| proof.reconciliation),
        )
        .await?;
        let current_check_context = if facet == DetailFacet::Checks {
            match super::pull_commits::check_context_in(&mut tx, account_id, subject_id, false)
                .await
            {
                Ok(context) => Some(context),
                Err(error) if matches!(error.code, ErrorCode::NotFound | ErrorCode::StaleView) => {
                    None
                }
                Err(error) => return Err(error),
            }
        } else {
            None
        };
        let current_review_context = if is_review_facet(facet) {
            match super::pull_commits::review_context_in(&mut tx, account_id, subject_id, false)
                .await
            {
                Ok(context) => Some(context),
                Err(error) if matches!(error.code, ErrorCode::NotFound | ErrorCode::StaleView) => {
                    None
                }
                Err(error) => return Err(error),
            }
        } else {
            None
        };
        let comparable_scope = source
            .as_ref()
            .and_then(StoredSource::traversal)
            .is_some_and(|proof| {
                if facet == DetailFacet::Checks {
                    current_check_context
                        .as_ref()
                        .is_some_and(|context| proof.check_context.as_ref() == Some(context))
                } else if is_review_facet(facet) {
                    current_review_context
                        .as_ref()
                        .is_some_and(|context| proof.review_context.as_ref() == Some(context))
                } else {
                    proof.reconciliation.head_scope != DetailHeadScope::CurrentHead
                        || proof.head_oid == subject.head_oid
                }
            });
        let resume = old
            .as_ref()
            .is_some_and(|s| s.coverage.state == CoverageState::Partial && s.next_cursor.is_some())
            && comparable_scope;
        let run_id = Uuid::new_v4().to_string();
        if resume {
            // Carry traversal membership into a new request generation. Any old
            // lease still in flight is fenced even when resuming the same cursor.
            sqlx::query("UPDATE detail_entries SET last_seen_run=? WHERE account_id=? AND subject_id=? AND facet=? AND last_seen_run=?")
                .bind(&run_id).bind(account_id).bind(subject_id).bind(tag(&facet)?).bind(&old.as_ref().ok_or_else(stale)?.run_id)
                .execute(&mut *tx).await.map_err(storage_error)?;
        }
        let mut sync = old.as_ref().map(|s| s.sync.clone()).unwrap_or_default();
        sync.state = SyncState::Syncing;
        sync.error = None;
        sync.next_retry_at = None;
        let coverage = old
            .as_ref()
            .map(|s| s.coverage.clone())
            .unwrap_or_else(missing_coverage);
        sqlx::query("INSERT INTO sync_scopes(account_id,scope,run_id,coverage_json,sync_json) VALUES(?,?,?,?,?) ON CONFLICT(account_id,scope) DO UPDATE SET run_id=excluded.run_id,sync_json=excluded.sync_json,next_cursor=?,etag=?")
            .bind(account_id).bind(&scope).bind(&run_id).bind(encode(&coverage)?).bind(encode(&sync)?)
            .bind(if resume {old.as_ref().and_then(|s|s.next_cursor.clone())} else {None})
            .bind(if comparable_scope {old.as_ref().and_then(|s|s.etag.clone())} else {None})
            .execute(&mut *tx).await.map_err(storage_error)?;
        record_change(
            &mut tx,
            account_id,
            positive_revision(epoch)?,
            &scope,
            false,
        )
        .await?;
        let result = DetailLease {
            run_id,
            authorization_view,
            instance_id: identities::instance_in(&mut tx, &account).await?.id,
            next_cursor: old
                .as_ref()
                .filter(|_| resume)
                .and_then(|s| s.next_cursor.clone()),
            etag: old
                .as_ref()
                .filter(|s| {
                    s.coverage.state == CoverageState::Complete && !resume && comparable_scope
                })
                .and_then(|s| s.etag.clone()),
            reconciliation: source
                .as_ref()
                .filter(|_| comparable_scope)
                .and_then(StoredSource::traversal)
                .map(|p| p.reconciliation),
            source: source.filter(|_| comparable_scope).map(|s| s.source),
        };
        tx.commit().await.map_err(storage_error)?;
        Ok(result)
    }

    pub async fn apply_detail(&self, page: DetailCommit) -> Result<String> {
        let mut writer = self.inner.writer.acquire().await?;
        let mut tx = writer.begin().await.map_err(storage_error)?;
        let revision = apply_detail_in(&mut tx, page).await?;
        tx.commit().await.map_err(storage_error)?;
        Ok(revision)
    }

    /// A rejected representation cannot mutate truth. The runtime may separately
    /// restart its exact still-current lease once, retaining cache and drafts.
    pub(crate) async fn restart_detail_traversal(
        &self,
        account_id: &str,
        epoch: &str,
        subject_id: &str,
        facet: DetailFacet,
        lease: &DetailLease,
    ) -> Result<String> {
        let mut writer = self.inner.writer.acquire().await?;
        let mut tx = writer.begin().await.map_err(storage_error)?;
        validate_lease_in(&mut tx, account_id, epoch, subject_id, facet, lease).await?;
        let scope = facet.scope(subject_id);
        let stored = scope_in(&mut tx, account_id, &scope)
            .await?
            .ok_or_else(stale)?;
        let mut coverage = stored.coverage;
        coverage.state = CoverageState::Partial;
        coverage.remote_has_more = false;
        sqlx::query("UPDATE sync_scopes SET run_id=?,next_cursor=NULL,etag=NULL,coverage_json=? WHERE account_id=? AND scope=? AND run_id=?")
            .bind(Uuid::new_v4().to_string()).bind(encode(&coverage)?).bind(account_id).bind(&scope).bind(&lease.run_id).execute(&mut *tx).await.map_err(storage_error)?;
        let revision = record_change(
            &mut tx,
            account_id,
            positive_revision(epoch)?,
            &scope,
            false,
        )
        .await?;
        tx.commit().await.map_err(storage_error)?;
        Ok(revision)
    }
}

pub(super) async fn apply_detail_in(
    tx: &mut Transaction<'_, Sqlite>,
    mut page: DetailCommit,
) -> Result<String> {
    if matches!(page.facet, DetailFacet::Commits | DetailFacet::Files)
        || page.entries.len() > 100
        || page.source.source.is_empty()
        || page.source.source.len() > 256
        || page.source.adapter_version == 0
        || !mask_valid(page.facet, &page.source.field_mask)
        || !timestamp_valid(&page.source.observed_at)
        || page
            .source
            .provider_updated_at
            .as_ref()
            .is_some_and(|s| !timestamp_valid(s))
        || page
            .next_cursor
            .as_ref()
            .is_some_and(|s| s.is_empty() || s.len() > 8192)
        || page.etag.as_ref().is_some_and(|s| s.len() > 8192)
        || page.complete && page.next_cursor.is_some()
    {
        return Err(invalid_detail());
    }
    if page.facet == DetailFacet::Activity {
        let fields = [
            DetailField::Body,
            DetailField::Author,
            DetailField::UpdatedAt,
            DetailField::Activity,
        ];
        if page.entries.len() > 50
            || page.source.field_mask.len() != fields.len()
            || !fields.iter().all(|f| page.source.field_mask.contains(f))
            || page.source.provider_updated_at.is_some()
            || page.reconciliation.head_scope != DetailHeadScope::SubjectHistory
            || !matches!(
                page.reconciliation.enumeration,
                DetailEnumeration::Uncertain | DetailEnumeration::FullEnumeration
            )
            || page.reconciliation.enumeration == DetailEnumeration::FullEnumeration
                && (page.request_cursor.is_some()
                    || page.next_cursor.is_some()
                    || !page.complete
                    || !page.whole_scope)
            || page.complete != page.next_cursor.is_none()
            || page.subject_binding.is_none()
            || page.metadata.is_some()
            || page.etag.is_some()
            || page.not_modified
            || [&page.request_cursor, &page.next_cursor]
                .into_iter()
                .any(|c| c.as_ref().is_some_and(|s| s.len() > 4096))
        {
            return Err(invalid_detail());
        }
        for (i, entry) in page.entries.iter().enumerate() {
            validate_native(page.facet, entry)?;
            if entry.field_mask.len() != fields.len()
                || !fields.iter().all(|f| entry.field_mask.contains(f))
                || !entry.field_validations.is_empty()
                || page.entries[..i]
                    .iter()
                    .any(|prior| prior.id == entry.id || prior.provider_id == entry.provider_id)
            {
                return Err(invalid_detail());
            }
        }
    }
    if page.facet == DetailFacet::Participants
        && (page.source.field_mask.len() != 6
            || page.source.provider_updated_at.is_some()
            || page.reconciliation != DetailReconciliation::full_history()
            || page.subject_binding.is_none()
            || page.metadata.is_some()
            || page.request_cursor.is_some()
            || page.next_cursor.is_some()
            || page.etag.is_some()
            || page.not_modified
            || !page.whole_scope
            || !page.complete)
    {
        return Err(invalid_detail());
    }
    if page.facet == DetailFacet::Tasks
        && (page.entries.len() > 50
            || page.source.field_mask.len() != 12
            || page.source.provider_updated_at.is_some()
            || page.reconciliation.head_scope != DetailHeadScope::SubjectHistory
            || !matches!(
                page.reconciliation.enumeration,
                DetailEnumeration::Uncertain | DetailEnumeration::FullEnumeration
            )
            || page.reconciliation.enumeration == DetailEnumeration::FullEnumeration
                && (page.request_cursor.is_some()
                    || page.next_cursor.is_some()
                    || !page.complete
                    || !page.whole_scope)
            || page.complete != page.next_cursor.is_none()
            || page.subject_binding.is_none()
            || page.metadata.is_some()
            || page.etag.is_some()
            || page.not_modified
            || [&page.request_cursor, &page.next_cursor]
                .into_iter()
                .any(|cursor| cursor.as_ref().is_some_and(|cursor| cursor.len() > 4096)))
    {
        return Err(invalid_detail());
    }
    if is_review_facet(page.facet)
        && (page.entries.len() > 50
            || page.reconciliation.head_scope != DetailHeadScope::CurrentHead
            || !matches!(
                page.reconciliation.enumeration,
                DetailEnumeration::Uncertain | DetailEnumeration::FullEnumeration
            )
            || page.reconciliation.enumeration == DetailEnumeration::FullEnumeration
                && (page.request_cursor.is_some()
                    || page.next_cursor.is_some()
                    || !page.complete
                    || !page.whole_scope)
            || page.complete != page.next_cursor.is_none()
            || page.subject_binding.is_none()
            || page.review_context.is_none()
            || page.metadata.is_some()
            || page.etag.is_some()
            || page.not_modified
            || [&page.request_cursor, &page.next_cursor]
                .into_iter()
                .any(|cursor| cursor.as_ref().is_some_and(|cursor| cursor.len() > 4096)))
    {
        return Err(invalid_detail());
    }
    if page.facet == DetailFacet::Participants {
        for (index, entry) in page.entries.iter().enumerate() {
            validate_native(page.facet, entry)?;
            let Some(crate::NativeDetailPayload::ParticipantV1(value)) = &entry.native else {
                return Err(invalid_detail());
            };
            if page.entries[..index].iter().any(|prior| {
                prior.id == entry.id
                    || prior.provider_id == entry.provider_id
                    || matches!(&prior.native, Some(crate::NativeDetailPayload::ParticipantV1(old))
                        if old.user.provider_id == value.user.provider_id)
            }) {
                return Err(invalid_detail());
            }
        }
    }
    if page.facet == DetailFacet::Tasks {
        for (index, entry) in page.entries.iter().enumerate() {
            validate_task_input(entry)?;
            if page.entries[..index]
                .iter()
                .any(|prior| prior.id == entry.id || prior.provider_id == entry.provider_id)
            {
                return Err(invalid_detail());
            }
        }
    }
    if is_review_facet(page.facet) {
        for (index, entry) in page.entries.iter().enumerate() {
            validate_review_input(page.facet, entry)?;
            validate_review(page.facet, entry)?;
            if page.entries[..index]
                .iter()
                .any(|prior| prior.id == entry.id || prior.provider_id == entry.provider_id)
            {
                return Err(invalid_detail());
            }
        }
    }
    validate_value(&page.body)?;
    if page
        .body
        .text
        .as_ref()
        .is_some_and(|v| v.len() > MAX_BODY_BYTES)
    {
        page.body = DetailValue {
            state: DetailValueState::Oversized,
            text: None,
        };
    }
    if page.facet == DetailFacet::Body {
        if !page.entries.is_empty()
            || page.next_cursor.is_some()
            || (!page.not_modified && !page.source.field_mask.contains(&DetailField::Body))
        {
            return Err(invalid_detail());
        }
    } else if page.body != DetailValue::default() {
        return Err(invalid_detail());
    }
    let scope = page.facet.scope(&page.subject_id);
    epoch_in(tx, &page.account_id, &page.authorization_epoch).await?;
    let account = account_in(tx, &page.account_id, true).await?;
    if account.provider != ProviderKind::Gitlab
        && page.entries.iter().any(|entry| {
            matches!(&entry.native,
            Some(crate::NativeDetailPayload::ReviewThreadV1(thread)) if thread.native.is_some())
        })
    {
        return Err(invalid_detail());
    }

    let subject = subject_in(tx, &page.account_id, &page.subject_id).await?;
    if let Some(binding) = &page.subject_binding {
        super::resource_metadata::validate_binding_in(tx, &page.account_id, &subject, binding)
            .await?;
    } else if page.metadata.is_some() {
        return Err(invalid_detail());
    }
    if page.facet.capability(&subject.kind).is_none() {
        return Err(invalid_detail());
    }
    if page.facet == DetailFacet::Checks {
        let current =
            super::pull_commits::check_context_in(tx, &page.account_id, &page.subject_id, true)
                .await?;
        if page.check_context.as_ref() != Some(&current) {
            return Err(stale());
        }
    } else if is_review_facet(page.facet) {
        let current =
            super::pull_commits::review_context_in(tx, &page.account_id, &page.subject_id, true)
                .await?;
        if page.review_context.as_ref() != Some(&current) {
            return Err(stale());
        }
    }
    if identities::instance_in(tx, &account).await?.id != page.instance_id
        || metadata(tx).await?.1 != page.authorization_view
    {
        return Err(stale());
    }
    let stored = scope_in(tx, &page.account_id, &scope)
        .await?
        .ok_or_else(stale)?;
    if stored.run_id != page.run_id || stored.next_cursor != page.request_cursor {
        return Err(stale());
    }
    let previous=sqlx::query("SELECT body_json,source_json,value_source_json,stale_at FROM detail_observations WHERE account_id=? AND subject_id=? AND facet=?")
            .bind(&page.account_id).bind(&page.subject_id).bind(tag(&page.facet)?).fetch_optional(&mut **tx).await.map_err(storage_error)?;
    let previous_native_source: Option<StoredSource> = previous
        .as_ref()
        .map(|r| decode(r.get("source_json")))
        .transpose()?;
    let previous_source = previous_native_source.as_ref().map(|s| &s.source);
    let native_source =
        super::facet_reconciliation::source_for(&page, previous_native_source.as_ref())?;
    validate_declared_head_in(
        tx,
        &account,
        &subject,
        page.facet,
        Some(page.reconciliation),
    )
    .await?;
    let previous_value_source: Option<DetailSource> = previous
        .as_ref()
        .and_then(|r| r.get::<Option<String>, _>("value_source_json"))
        .map(|json| decode(&json))
        .transpose()?;
    let same_traversal_context = if page.facet == DetailFacet::Checks {
        native_source
            .traversal()
            .and_then(|proof| proof.check_context.as_ref())
            == previous_native_source
                .as_ref()
                .and_then(StoredSource::traversal)
                .and_then(|proof| proof.check_context.as_ref())
    } else if is_review_facet(page.facet) {
        native_source
            .traversal()
            .and_then(|proof| proof.review_context.as_ref())
            == previous_native_source
                .as_ref()
                .and_then(StoredSource::traversal)
                .and_then(|proof| proof.review_context.as_ref())
    } else {
        native_source
            .traversal()
            .and_then(|proof| proof.head_oid.as_ref())
            == previous_native_source
                .as_ref()
                .and_then(StoredSource::traversal)
                .and_then(|proof| proof.head_oid.as_ref())
    };
    let ordering_source = previous_value_source
        .as_ref()
        .filter(|_| same_traversal_context);
    if ordering_source.is_some_and(|old| {
        old.source == page.source.source
            && old.adapter_version == page.source.adapter_version
            && old
                .provider_updated_at
                .as_ref()
                .zip(page.source.provider_updated_at.as_ref())
                .is_some_and(|(old, new)| timestamp_older(new, old))
    }) {
        return Err(stale());
    }
    if page.not_modified
        && (previous.is_none()
            || !page.whole_scope
            || !page.complete
            || !page.entries.is_empty()
            || page.body != DetailValue::default()
            || stored.coverage.state != CoverageState::Complete
            || stored.etag.is_none()
            || previous_source.is_none_or(|old| {
                old.source != page.source.source
                    || old.adapter_version != page.source.adapter_version
                    || old.field_mask != page.source.field_mask
            })
            || page
                .etag
                .as_ref()
                .is_some_and(|etag| Some(etag) != stored.etag.as_ref()))
    {
        return Err(invalid_detail());
    }
    let observes_body =
        page.facet == DetailFacet::Body && page.body.state == DetailValueState::Known;
    let validates = page.not_modified || page.facet != DetailFacet::Body || observes_body;
    let value_source = if page.not_modified {
        previous_value_source.map(|mut source| {
            source.observed_at = page.source.observed_at.clone();
            source
        })
    } else if validates {
        let mut source = page.source.clone();
        if source.provider_updated_at.is_none()
            && let Some(old) = previous_value_source.as_ref().filter(|old| {
                same_traversal_context
                    && old.source == source.source
                    && old.adapter_version == source.adapter_version
            })
        {
            source.provider_updated_at = old.provider_updated_at.clone();
        }
        Some(source)
    } else {
        previous_value_source
    };
    let mut body: DetailValue = previous
        .as_ref()
        .map(|r| decode(r.get("body_json")))
        .transpose()?
        .unwrap_or_default();
    if observes_body || body.state != DetailValueState::Known && !page.not_modified {
        body = page.body.clone();
    }
    let observed_state = if page.not_modified {
        DetailValueState::Known
    } else if page.facet == DetailFacet::Body {
        page.body.state
    } else {
        DetailValueState::Known
    };
    let validated_at = if validates {
        Some(page.source.observed_at.clone())
    } else {
        stored.coverage.validated_at.clone()
    };
    let stale_at = if validates {
        Some(
            (chrono::DateTime::parse_from_rfc3339(&page.source.observed_at)
                .map_err(|_| invalid_detail())?
                + chrono::Duration::seconds(i64::from(page.freshness_seconds.min(86_400))))
            .to_rfc3339(),
        )
    } else {
        previous.as_ref().and_then(|r| r.get("stale_at"))
    };
    let coverage = Coverage {
        state: if page.complete
            && validates
            && (page.facet == DetailFacet::Body
                || native_source.traversal().is_some_and(|proof| {
                    proof.starts_at_beginning
                        && proof.reconciliation.enumeration == DetailEnumeration::FullEnumeration
                }))
        {
            CoverageState::Complete
        } else {
            CoverageState::Partial
        },
        validated_at,
        remote_has_more: page.next_cursor.is_some(),
    };
    let revision = record_change(
        tx,
        &page.account_id,
        positive_revision(&page.authorization_epoch)?,
        &scope,
        false,
    )
    .await?;
    sqlx::query("INSERT INTO detail_observations(account_id,subject_id,facet,authorization_epoch,facet_revision,body_json,source_json,value_source_json,observed_state,stale_at) VALUES(?,?,?,?,?,?,?,?,?,?) ON CONFLICT(account_id,subject_id,facet) DO UPDATE SET authorization_epoch=excluded.authorization_epoch,facet_revision=excluded.facet_revision,body_json=excluded.body_json,source_json=excluded.source_json,value_source_json=excluded.value_source_json,observed_state=excluded.observed_state,stale_at=excluded.stale_at")
            .bind(&page.account_id).bind(&page.subject_id).bind(tag(&page.facet)?).bind(&page.authorization_epoch).bind(&revision).bind(encode(&body)?).bind(encode(&native_source)?).bind(value_source.map(|s|encode(&s)).transpose()?).bind(tag(&observed_state)?).bind(stale_at).execute(&mut **tx).await.map_err(storage_error)?;
    if page.facet == DetailFacet::Body {
        super::resource_metadata::apply_in(tx, &page, &subject.kind).await?;
        super::pull_files::body_observed_in(tx, &page.account_id, &page.subject_id, &revision)
            .await?;
        let saved_metadata = super::resource_metadata::read_in(tx, &account, &subject.id).await?;
        let saved_head = saved_metadata.as_ref().and_then(|metadata| {
            metadata
                .fields
                .iter()
                .find(|field| field.field == crate::MetadataField::Head)
                .filter(|field| field.saved_state == DetailValueState::Known)
                .and(metadata.values.head.as_ref())
                .map(|head| head.oid.as_str())
        });
        invalidate_declared_head_in(tx, &account, &subject.id, saved_head).await?;
    } else if page.metadata.is_some() {
        return Err(invalid_detail());
    }
    for incoming in page.entries {
        let old=sqlx::query("SELECT json,last_seen_run FROM detail_entries WHERE account_id=? AND subject_id=? AND facet=? AND id=?")
                .bind(&page.account_id).bind(&page.subject_id).bind(tag(&page.facet)?).bind(&incoming.id).fetch_optional(&mut **tx).await.map_err(storage_error)?;
        if page.facet == DetailFacet::Checks
            && old
                .as_ref()
                .is_some_and(|row| row.get::<String, _>("last_seen_run") == page.run_id)
        {
            return Err(invalid_detail());
        }
        let previous_entry: Option<StoredEntry> =
            old.map(|row| decode(row.get("json"))).transpose()?;
        // Check keys may be reused by a different fork/source repository at
        // the same SHA. Its provider clock has no ordering authority in the
        // replacement CheckContext, so start that row's clocks from the fresh
        // observation instead of allowing old green data to survive.
        let previous_entry =
            previous_entry.filter(|_| page.facet != DetailFacet::Checks || same_traversal_context);
        let entry = merge_entry(
            page.facet,
            incoming,
            previous_entry,
            &page.source,
            native_source
                .traversal()
                .and_then(|proof| proof.head_oid.as_deref()),
        )?;
        sqlx::query("INSERT INTO detail_entries(account_id,subject_id,facet,id,json,last_seen_run) VALUES(?,?,?,?,?,?) ON CONFLICT(account_id,subject_id,facet,id) DO UPDATE SET json=excluded.json,last_seen_run=excluded.last_seen_run")
                .bind(&page.account_id).bind(&page.subject_id).bind(tag(&page.facet)?).bind(&entry.entry.id).bind(encode(&entry)?).bind(&page.run_id).execute(&mut **tx).await.map_err(storage_error)?;
    }
    if page.complete
        && !page.not_modified
        && page.facet != DetailFacet::Body
        && native_source.traversal().is_some_and(|proof| {
            proof.starts_at_beginning
                && proof.reconciliation.enumeration == DetailEnumeration::FullEnumeration
        })
    {
        sqlx::query("DELETE FROM detail_entries WHERE account_id=? AND subject_id=? AND facet=? AND last_seen_run<>?")
                .bind(&page.account_id).bind(&page.subject_id).bind(tag(&page.facet)?).bind(&page.run_id).execute(&mut **tx).await.map_err(storage_error)?;
    }
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM detail_entries WHERE account_id=? AND subject_id=? AND facet=?",
    )
    .bind(&page.account_id)
    .bind(&page.subject_id)
    .bind(tag(&page.facet)?)
    .fetch_one(&mut **tx)
    .await
    .map_err(storage_error)?;
    if count > MAX_DETAIL_ENTRIES {
        return Err(CollaborationError::new(
            ErrorCode::Busy,
            "The detail collection cache limit was reached",
        ));
    }
    let mut sync = stored.sync;
    sync.state = SyncState::Idle;
    sync.error = None;
    sync.next_retry_at = None;
    if validates {
        sync.last_success_at = Some(page.source.observed_at);
    }
    let retain_validator =
        page.complete && page.whole_scope && validates && coverage.state == CoverageState::Complete;
    sqlx::query("UPDATE sync_scopes SET next_cursor=?,etag=?,coverage_json=?,sync_json=?,access_denied=0,data_revision=data_revision+1 WHERE account_id=? AND scope=? AND run_id=?")
            .bind(&page.next_cursor).bind(if retain_validator { page.etag.or(if page.not_modified {stored.etag}else{None}) } else {None}).bind(encode(&coverage)?).bind(encode(&sync)?).bind(&page.account_id).bind(&scope).bind(&page.run_id).execute(&mut **tx).await.map_err(storage_error)?;
    if page.complete {
        sqlx::query("UPDATE detail_demand SET requested=0 WHERE account_id=? AND subject_id=? AND facet=? AND authorization_epoch=?")
                .bind(&page.account_id).bind(&page.subject_id).bind(tag(&page.facet)?).bind(&page.authorization_epoch).execute(&mut **tx).await.map_err(storage_error)?;
    }
    super::retention::refresh_detail_accounting_in(
        tx,
        &page.account_id,
        &page.subject_id,
        &tag(&page.facet)?,
    )
    .await?;
    Ok(revision)
}
