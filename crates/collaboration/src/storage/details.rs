//! Independent facet observations and short, authorized local snapshots.
use super::facet_reconciliation::{StoredEntry, StoredSource};
use super::*;
use crate::{DetailSubjectBinding, detail::*};

const MAX_DETAIL_ENTRIES: i64 = 5_000;
const MAX_ENTRY_BODY_BYTES: usize = 65_536;

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
    let rows = sqlx::query("SELECT facet,source_json FROM detail_observations WHERE account_id=? AND subject_id=? AND facet<>'body' AND authorization_epoch=?")
        .bind(&account.id).bind(subject).bind(&account.authorization_epoch).fetch_all(&mut **tx).await.map_err(storage_error)?;
    for row in rows {
        let source: StoredSource = decode(row.get("source_json"))?;
        if source.traversal().is_none_or(|proof| {
            proof.reconciliation.head_scope != DetailHeadScope::CurrentHead
                || proof.head_oid.as_deref() == head
        }) {
            continue;
        }
        let facet: DetailFacet = decode(&format!("\"{}\"", row.get::<String, _>("facet")))?;
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
    last_id: String,
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
            head != proof.head_oid
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
    mask.len() <= 6
        && mask
            .iter()
            .all(|field| field.is_participant() == (facet == DetailFacet::Participants))
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

fn blank_native(native: &Option<crate::NativeDetailPayload>) -> Option<crate::NativeDetailPayload> {
    native.as_ref().map(|native| match native {
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
        StoredEntry::from_entry(DetailEntry {
            id: incoming.id.clone(),
            provider_id: incoming.provider_id.clone(),
            author: None,
            title: None,
            state: None,
            body: DetailValue::default(),
            observed_body_state: DetailValueState::NotLoaded,
            updated_at: None,
            head_oid: None,
            native: blank_native(&incoming.native),
            field_mask: vec![],
            field_validations: vec![],
        })
    });
    validate_native(facet, &saved.entry)?;
    if !mask_valid(facet, &saved.entry.field_mask)
        || saved.entry.field_validations.len() > 6
        || saved
            .entry
            .field_validations
            .iter()
            .enumerate()
            .any(|(index, validation)| {
                validation.field.is_participant() != (facet == DetailFacet::Participants)
                    || saved.entry.field_validations[..index]
                        .iter()
                        .any(|old| old.field == validation.field)
            })
    {
        return Err(invalid_detail());
    }
    saved.initialize_legacy(facet);
    let same_native_identity = match (&saved.entry.native, &incoming.native) {
        (None, None) => true,
        (
            Some(crate::NativeDetailPayload::ParticipantV1(old)),
            Some(crate::NativeDetailPayload::ParticipantV1(new)),
        ) => old.user.provider_id == new.user.provider_id,
        _ => false,
    };
    if saved.entry.provider_id != incoming.provider_id || !same_native_identity {
        return Err(invalid_detail());
    }
    let comparable = if incoming.field_mask.contains(&DetailField::UpdatedAt) {
        incoming
            .updated_at
            .as_deref()
            .or(source.provider_updated_at.as_deref())
    } else {
        source.provider_updated_at.as_deref()
    };
    saved.entry.observed_body_state = if incoming.field_mask.contains(&DetailField::Body) {
        incoming.body.state
    } else {
        DetailValueState::NotLoaded
    };
    for field in &incoming.field_mask {
        if saved.older(*field, comparable, source, head) {
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
            _ => return Err(invalid_detail()),
        }
        saved.observed(*field, comparable, source, head);
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
    if source.as_ref().map(|source| &source.source) != lease.source.as_ref()
        || proof.map(|proof| proof.reconciliation) != reconciliation
        || proof.is_some_and(|proof| {
            proof.reconciliation.head_scope == DetailHeadScope::CurrentHead
                && proof.head_oid != subject.head_oid
        })
    {
        return Err(stale());
    }
    validate_declared_head_in(tx, &account, &subject, facet, reconciliation).await?;
    Ok(subject)
}

impl Store {
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
        let mut after = String::new();
        if let Some(cursor) = query.cursor {
            let cursor: DetailCursor =
                serde_json::from_str(&cursor).map_err(|_| invalid_detail())?;
            if cursor.account != query.account_id
                || cursor.subject != query.subject_id
                || cursor.facet != query.facet
                || cursor.authorization_view != result.authorization_view
                || cursor.facet_revision != result.evidence.facet_revision
            {
                return Err(stale());
            }
            validate_identifier(&cursor.last_id)?;
            after = cursor.last_id;
        }
        if let Some(json)=sqlx::query_scalar::<_,String>("SELECT body_json FROM detail_observations WHERE account_id=? AND subject_id=? AND facet=?")
            .bind(&query.account_id).bind(&query.subject_id).bind(tag(&query.facet)?).fetch_optional(&mut *tx).await.map_err(storage_error)? { result.body=decode(&json)?; }
        let rows=sqlx::query("SELECT id,json FROM detail_entries WHERE account_id=? AND subject_id=? AND facet=? AND id>? ORDER BY id LIMIT ?")
            .bind(&query.account_id).bind(&query.subject_id).bind(tag(&query.facet)?).bind(after).bind(i64::from(query.limit)+1).fetch_all(&mut *tx).await.map_err(storage_error)?;
        let has_more = rows.len() > query.limit as usize;
        for row in rows.into_iter().take(query.limit as usize) {
            result.entries.push(decode(row.get("json"))?);
        }
        if has_more {
            result.next_cursor = Some(encode(&DetailCursor {
                account: query.account_id,
                subject: query.subject_id,
                facet: query.facet,
                authorization_view: result.authorization_view.clone(),
                facet_revision: result.evidence.facet_revision.clone(),
                last_id: result.entries.last().ok_or_else(invalid_detail)?.id.clone(),
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
        let mut writer = self.inner.writer.lock().await;
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
        let mut writer = self.inner.writer.lock().await;
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
        let mut writer = self.inner.writer.lock().await;
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
        let mut writer = self.inner.writer.lock().await;
        let mut tx = writer.begin().await.map_err(storage_error)?;
        epoch_in(&mut tx, account_id, epoch).await?;
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
        let comparable_scope = source
            .as_ref()
            .and_then(StoredSource::traversal)
            .is_some_and(|proof| {
                proof.reconciliation.head_scope != DetailHeadScope::CurrentHead
                    || proof.head_oid == subject.head_oid
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
        let (_, authorization_view) = metadata(&mut tx).await?;
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
                .and_then(StoredSource::traversal)
                .map(|p| p.reconciliation),
            source: source.map(|s| s.source),
        };
        tx.commit().await.map_err(storage_error)?;
        Ok(result)
    }

    pub async fn apply_detail(&self, page: DetailCommit) -> Result<String> {
        let mut writer = self.inner.writer.lock().await;
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
        let mut writer = self.inner.writer.lock().await;
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
    if page.entries.len() > 100
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
    let same_head_context = native_source
        .traversal()
        .and_then(|proof| proof.head_oid.as_ref())
        == previous_native_source
            .as_ref()
            .and_then(StoredSource::traversal)
            .and_then(|proof| proof.head_oid.as_ref());
    let ordering_source = previous_value_source.as_ref().filter(|_| same_head_context);
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
                same_head_context
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
    } else if page.metadata.is_some() {
        return Err(invalid_detail());
    }
    for incoming in page.entries {
        let old:Option<String>=sqlx::query_scalar("SELECT json FROM detail_entries WHERE account_id=? AND subject_id=? AND facet=? AND id=?")
                .bind(&page.account_id).bind(&page.subject_id).bind(tag(&page.facet)?).bind(&incoming.id).fetch_optional(&mut **tx).await.map_err(storage_error)?;
        let entry = merge_entry(
            page.facet,
            incoming,
            old.map(|s| decode(&s)).transpose()?,
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
    Ok(revision)
}
