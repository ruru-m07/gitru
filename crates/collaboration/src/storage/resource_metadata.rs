//! Resource field authority is separate from the description's missingness.
use super::*;
use crate::{detail::*, resource_metadata::*};

const MAX_METADATA_BYTES: usize = 262_144;
// Metadata fields can grow their RFC3339 validation strings during a 304.
const VALIDATION_RESERVE_BYTES: usize = 8_192;

pub(super) async fn read_in(
    tx: &mut Transaction<'_, Sqlite>,
    account: &RemoteAccount,
    subject: &str,
) -> Result<Option<ResourceMetadataSnapshot>> {
    let json: Option<String> = sqlx::query_scalar("SELECT metadata_json FROM detail_resource_metadata WHERE account_id=? AND subject_id=? AND authorization_epoch=?")
        .bind(&account.id).bind(subject).bind(&account.authorization_epoch).fetch_optional(&mut **tx).await.map_err(storage_error)?;
    json.map(|s| decode(&s)).transpose()
}

/// A saved authorized head can contradict the independent list projection.
/// Fail closed for head-scoped evidence; do not elect or rewrite either head.
pub(super) async fn head_conflicts_in(
    tx: &mut Transaction<'_, Sqlite>,
    account: &RemoteAccount,
    subject_id: &str,
    summary_head: Option<&str>,
) -> Result<bool> {
    // Metadata-only projection: do not materialize a description, summary body
    // or the rest of the saved metadata in the contextual evidence accessor.
    sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM detail_resource_metadata m WHERE m.account_id=? AND m.subject_id=? AND m.authorization_epoch=? AND json_extract(m.metadata_json,'$.values.head.oid') IS NOT ? AND EXISTS(SELECT 1 FROM json_each(m.metadata_json,'$.fields') f WHERE json_extract(f.value,'$.field')='head' AND json_extract(f.value,'$.saved_state')='known') AND NOT EXISTS(SELECT 1 FROM sync_scopes s WHERE s.account_id=m.account_id AND s.scope=? AND s.access_denied=1))")
        .bind(&account.id).bind(subject_id).bind(&account.authorization_epoch).bind(summary_head).bind(DetailFacet::Body.scope(subject_id))
        .fetch_one(&mut **tx).await.map_err(storage_error)
}

pub(super) async fn validate_binding_in(
    tx: &mut Transaction<'_, Sqlite>,
    account: &str,
    subject: &RemoteItem,
    binding: &DetailSubjectBinding,
) -> Result<()> {
    let native: Option<String> =
        sqlx::query_scalar("SELECT provider_id FROM repositories WHERE account_id=? AND id=?")
            .bind(account)
            .bind(&binding.repository_id)
            .fetch_optional(&mut **tx)
            .await
            .map_err(storage_error)?;
    if subject.repository_id.as_ref() != Some(&binding.repository_id)
        || native.as_ref() != Some(&binding.repository_provider_id)
        || subject.provider_id != binding.provider_id
        || subject.number != binding.number
        || subject.kind != binding.kind
        || subject.head_oid != binding.head_oid
    {
        return Err(stale());
    }
    Ok(())
}

fn copy_field(
    target: &mut ResourceMetadataValues,
    source: &ResourceMetadataValues,
    field: MetadataField,
) {
    match field {
        MetadataField::Title => target.title = source.title.clone(),
        MetadataField::State => target.state = source.state.clone(),
        MetadataField::StateReason => target.state_reason = source.state_reason.clone(),
        MetadataField::Author => target.author = source.author.clone(),
        MetadataField::WebUrl => target.web_url = source.web_url.clone(),
        MetadataField::UpdatedAt => target.updated_at = source.updated_at.clone(),
        MetadataField::Labels => target.labels = source.labels.clone(),
        MetadataField::Assignees => target.assignees = source.assignees.clone(),
        MetadataField::Milestone => target.milestone = source.milestone.clone(),
        MetadataField::IsDraft => target.is_draft = source.is_draft,
        MetadataField::Head => target.head = source.head.clone(),
        MetadataField::Base => target.base = source.base.clone(),
        MetadataField::MergeBase => target.merge_base_oid = source.merge_base_oid.clone(),
        MetadataField::MergedAt => target.merged_at = source.merged_at.clone(),
    }
}
fn valid_time(value: &str) -> bool {
    value.len() <= 128 && chrono::DateTime::parse_from_rfc3339(value).is_ok()
}
fn text(value: &Option<String>, max: usize) -> bool {
    value.as_ref().is_none_or(|s| s.len() <= max)
}
fn actor(value: &DetailActor) -> bool {
    !value.provider_id.is_empty()
        && value.provider_id.len() <= 256
        && value.login.len() <= 255
        && text(&value.web_url, 2048)
}
fn branch(value: &DetailBranch) -> bool {
    value.name.len() <= 1024
        && !value.oid.is_empty()
        && value.oid.len() <= 128
        && value.repository.as_ref().is_none_or(|r| {
            !r.provider_id.is_empty()
                && r.provider_id.len() <= 256
                && r.full_name.len() <= 1024
                && text(&r.web_url, 2048)
        })
}
fn validate(observation: &ResourceMetadataObservation) -> Result<()> {
    let v = &observation.values;
    if !matches!(
        observation.kind,
        RemoteItemKind::PullRequest | RemoteItemKind::Issue
    ) || observation.fields.len() > MetadataField::COMMON.len() + MetadataField::PULL.len()
        || observation.fields.iter().enumerate().any(|(i, f)| {
            !f.field.supports(&observation.kind)
                || observation.fields[..i]
                    .iter()
                    .any(|old| old.field == f.field)
                || f.state == DetailValueState::NotLoaded
        })
        || observation.source.source.is_empty()
        || observation.source.source.len() > 256
        || observation.source.adapter_version == 0
        || !valid_time(&observation.source.observed_at)
        || observation
            .source
            .provider_updated_at
            .as_ref()
            .is_some_and(|s| !valid_time(s))
        || !text(&v.title, 16384)
        || !text(&v.state, 128)
        || !text(&v.state_reason, 128)
        || !text(&v.web_url, 2048)
        || v.updated_at.as_ref().is_some_and(|s| !valid_time(s))
        || v.merged_at.as_ref().is_some_and(|s| !valid_time(s))
        || v.author.as_ref().is_some_and(|v| !actor(v))
        || v.assignees.len() > 100
        || v.assignees.iter().any(|v| !actor(v))
        || v.labels.len() > 100
        || v.labels
            .iter()
            .any(|l| l.name.len() > 1024 || !text(&l.provider_id, 256) || !text(&l.color, 32))
        || v.milestone.as_ref().is_some_and(|m| {
            m.provider_id.is_empty()
                || m.provider_id.len() > 256
                || !text(&m.number, 256)
                || m.title.len() > 16384
                || !text(&m.state, 128)
                || !text(&m.web_url, 2048)
        })
        || v.head.as_ref().is_some_and(|b| !branch(b))
        || v.base.as_ref().is_some_and(|b| !branch(b))
        || v.merge_base_oid
            .as_ref()
            .is_some_and(|oid| !crate::is_canonical_commit_oid(oid))
        || encode(v)?.len() > MAX_METADATA_BYTES
    {
        return Err(invalid_detail());
    }
    Ok(())
}

pub(super) async fn apply_in(
    tx: &mut Transaction<'_, Sqlite>,
    page: &DetailCommit,
    kind: &RemoteItemKind,
) -> Result<()> {
    if !valid_time(&page.source.observed_at) {
        return Err(invalid_detail());
    }
    let row = sqlx::query("SELECT metadata_json,source_json FROM detail_resource_metadata WHERE account_id=? AND subject_id=? AND authorization_epoch=?")
        .bind(&page.account_id).bind(&page.subject_id).bind(&page.authorization_epoch).fetch_optional(&mut **tx).await.map_err(storage_error)?;
    let mut saved: ResourceMetadataSnapshot = row
        .as_ref()
        .map(|r| decode(r.get("metadata_json")))
        .transpose()?
        .unwrap_or(ResourceMetadataSnapshot {
            kind: kind.clone(),
            values: ResourceMetadataValues::default(),
            fields: MetadataField::COMMON
                .into_iter()
                .chain(if *kind == RemoteItemKind::PullRequest {
                    MetadataField::PULL.to_vec()
                } else {
                    vec![]
                })
                .map(|field| MetadataFieldEvidence {
                    field,
                    saved_state: DetailValueState::NotLoaded,
                    observed_state: DetailValueState::NotLoaded,
                    validated_at: None,
                    stale_at: None,
                    source: None,
                })
                .collect(),
        });
    // Add newly understood fields to older snapshots with no authority. A 304
    // cannot bless them; only a later explicit field observation can do so.
    for field in MetadataField::COMMON
        .into_iter()
        .chain(if *kind == RemoteItemKind::PullRequest {
            MetadataField::PULL.to_vec()
        } else {
            vec![]
        })
    {
        if !saved.fields.iter().any(|evidence| evidence.field == field) {
            saved.fields.push(MetadataFieldEvidence {
                field,
                saved_state: DetailValueState::NotLoaded,
                observed_state: DetailValueState::NotLoaded,
                validated_at: None,
                stale_at: None,
                source: None,
            });
        }
    }
    let previous_source: Option<MetadataSource> = row
        .as_ref()
        .map(|r| decode(r.get("source_json")))
        .transpose()?;
    let stale_at = (chrono::DateTime::parse_from_rfc3339(&page.source.observed_at)
        .map_err(|_| invalid_detail())?
        + chrono::Duration::seconds(i64::from(page.freshness_seconds.min(86400))))
    .to_rfc3339();
    let source = if page.not_modified {
        if page.metadata.is_some() {
            return Err(invalid_detail());
        }
        let Some(mut source) = previous_source else {
            return Ok(());
        };
        if source.source != page.source.source
            || source.adapter_version != page.source.adapter_version
        {
            return Err(invalid_detail());
        }
        source.observed_at = page.source.observed_at.clone();
        for field in &mut saved.fields {
            if field.saved_state == DetailValueState::Known
                && field.observed_state == DetailValueState::Known
            {
                field.validated_at = Some(source.observed_at.clone());
                field.stale_at = Some(stale_at.clone());
                if let Some(authority) = &mut field.source {
                    authority.observed_at = source.observed_at.clone();
                }
            }
        }
        source
    } else {
        let omitted;
        let observation = match &page.metadata {
            Some(observation) => observation,
            None => {
                if row.is_none() {
                    return Ok(());
                }
                // A new body representation without metadata cannot leave the
                // old representation's observed-known mask eligible for 304.
                omitted = ResourceMetadataObservation {
                    kind: kind.clone(),
                    values: ResourceMetadataValues::default(),
                    fields: vec![],
                    source: MetadataSource {
                        source: page.source.source.clone(),
                        adapter_version: page.source.adapter_version,
                        provider_updated_at: page.source.provider_updated_at.clone(),
                        observed_at: page.source.observed_at.clone(),
                    },
                };
                &omitted
            }
        };
        validate(observation)?;
        if observation.kind != *kind
            || observation.source.source != page.source.source
            || observation.source.adapter_version != page.source.adapter_version
            || observation.source.observed_at != page.source.observed_at
            || observation.source.provider_updated_at != page.source.provider_updated_at
        {
            return Err(invalid_detail());
        }
        // Reject a comparable older representation before updating ANY field.
        if saved
            .fields
            .iter()
            .filter_map(|f| f.source.as_ref())
            .any(|old| {
                old.source == observation.source.source
                    && old.adapter_version == observation.source.adapter_version
                    && old
                        .provider_updated_at
                        .as_ref()
                        .zip(observation.source.provider_updated_at.as_ref())
                        .is_some_and(|(old, new)| timestamp_older(new, old))
            })
        {
            return Err(stale());
        }
        for index in 0..saved.fields.len() {
            let field = saved.fields[index].field;
            let mut state = observation
                .fields
                .iter()
                .find(|f| f.field == field)
                .map(|f| f.state)
                .unwrap_or(DetailValueState::Omitted);
            if state == DetailValueState::Known {
                let mut candidate = saved.clone();
                copy_field(&mut candidate.values, &observation.values, field);
                let evidence = &mut candidate.fields[index];
                evidence.saved_state = state;
                evidence.observed_state = state;
                evidence.validated_at = Some(observation.source.observed_at.clone());
                evidence.stale_at = Some(stale_at.clone());
                let mut authority = observation.source.clone();
                if authority.provider_updated_at.is_none()
                    && let Some(old) = evidence.source.as_ref().filter(|old| {
                        old.source == authority.source
                            && old.adapter_version == authority.adapter_version
                    })
                {
                    authority.provider_updated_at = old.provider_updated_at.clone();
                }
                evidence.source = Some(authority);
                // Preserve room for bounded 304 timestamp and state-name growth.
                if encode(&candidate)?.len() > MAX_METADATA_BYTES - VALIDATION_RESERVE_BYTES {
                    state = DetailValueState::Oversized;
                } else {
                    saved = candidate;
                }
            }
            let evidence = &mut saved.fields[index];
            evidence.observed_state = state;
            if state != DetailValueState::Known && evidence.saved_state != DetailValueState::Known {
                evidence.saved_state = state;
            }
        }
        observation.source.clone()
    };
    let json = encode(&saved)?;
    if json.len() > MAX_METADATA_BYTES {
        return Err(invalid_detail());
    }
    sqlx::query("INSERT INTO detail_resource_metadata(account_id,subject_id,authorization_epoch,metadata_json,source_json) VALUES(?,?,?,?,?) ON CONFLICT(account_id,subject_id) DO UPDATE SET authorization_epoch=excluded.authorization_epoch,metadata_json=excluded.metadata_json,source_json=excluded.source_json")
        .bind(&page.account_id).bind(&page.subject_id).bind(&page.authorization_epoch).bind(json).bind(encode(&source)?).execute(&mut **tx).await.map_err(storage_error)?;
    Ok(())
}

/// Summary observations dirty a cached resource; they never become its fields.
pub(super) async fn invalidate_head_in(
    tx: &mut Transaction<'_, Sqlite>,
    account: &RemoteAccount,
    item: &RemoteItem,
    previous_head: Option<&str>,
) -> Result<()> {
    if item.kind != RemoteItemKind::PullRequest {
        return Ok(());
    }
    super::details::invalidate_head_scopes_in(tx, account, item).await?;
    if item.head_oid.is_none() {
        return Ok(());
    }
    let mut saved = read_in(tx, account, &item.id).await?;
    if let Some(saved) = &saved {
        if saved
            .values
            .head
            .as_ref()
            .is_some_and(|head| Some(&head.oid) == item.head_oid.as_ref())
        {
            return Ok(());
        }
        if saved
            .fields
            .iter()
            .find(|field| field.field == MetadataField::Head)
            .and_then(|field| field.source.as_ref())
            .is_some_and(|authority| {
                authority
                    .provider_updated_at
                    .as_ref()
                    .is_some_and(|at| timestamp_older(&item.updated_at, at))
            })
        {
            return Ok(());
        }
    }
    if previous_head == item.head_oid.as_deref()
        && saved
            .as_ref()
            .is_none_or(|saved| saved.values.head.is_none())
    {
        return Ok(());
    }
    let exists:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM detail_observations WHERE account_id=? AND subject_id=? AND facet='body' AND authorization_epoch=?)").bind(&account.id).bind(&item.id).bind(&account.authorization_epoch).fetch_one(&mut **tx).await.map_err(storage_error)?;
    if !exists {
        return Ok(());
    }
    let now = chrono::Utc::now().to_rfc3339();
    if let Some(saved) = &mut saved {
        for field in &mut saved.fields {
            if field.saved_state == DetailValueState::Known
                && field
                    .source
                    .as_ref()
                    .and_then(|s| s.provider_updated_at.as_ref())
                    .is_none_or(|at| !timestamp_older(&item.updated_at, at))
            {
                field.stale_at = Some(now.clone());
            }
        }
        sqlx::query(
        "UPDATE detail_resource_metadata SET metadata_json=? WHERE account_id=? AND subject_id=?",
    )
    .bind(encode(&saved)?)
    .bind(&account.id)
    .bind(&item.id)
    .execute(&mut **tx)
    .await
    .map_err(storage_error)?;
    }
    let body_source:Option<String>=sqlx::query_scalar("SELECT value_source_json FROM detail_observations WHERE account_id=? AND subject_id=? AND facet='body'").bind(&account.id).bind(&item.id).fetch_one(&mut **tx).await.map_err(storage_error)?;
    let body_source: Option<DetailSource> =
        body_source.map(|source| decode(&source)).transpose()?;
    if body_source
        .as_ref()
        .and_then(|source| source.provider_updated_at.as_ref())
        .is_none_or(|at| !timestamp_older(&item.updated_at, at))
    {
        sqlx::query("UPDATE detail_observations SET stale_at=? WHERE account_id=? AND subject_id=? AND facet='body'").bind(&now).bind(&account.id).bind(&item.id).execute(&mut **tx).await.map_err(storage_error)?;
    }
    let scope = DetailFacet::Body.scope(&item.id);
    sqlx::query("UPDATE sync_scopes SET etag=NULL,run_id=?,data_revision=data_revision+1 WHERE account_id=? AND scope=?").bind(Uuid::new_v4().to_string()).bind(&account.id).bind(&scope).execute(&mut **tx).await.map_err(storage_error)?;
    sqlx::query("UPDATE sync_scopes SET sync_json=json_set(sync_json,'$.state','idle') WHERE account_id=? AND scope=? AND json_extract(sync_json,'$.state')='syncing'").bind(&account.id).bind(&scope).execute(&mut **tx).await.map_err(storage_error)?;
    record_change(
        tx,
        &account.id,
        positive_revision(&account.authorization_epoch)?,
        &scope,
        false,
    )
    .await?;
    super::retention::refresh_detail_accounting_in(tx, &account.id, &item.id, "body").await?;
    Ok(())
}
