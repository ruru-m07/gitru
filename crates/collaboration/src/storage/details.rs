//! Independent facet observations and short, authorized local snapshots.
use super::*;
use crate::detail::*;

const MAX_DETAIL_ENTRIES: i64 = 5_000;
const MAX_ENTRY_BODY_BYTES: usize = 65_536;

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
            evidence.freshness = if evidence.stale_at.as_ref().is_some_and(|at| {
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

fn mask_valid(mask: &[DetailField]) -> bool {
    mask.len() <= 6
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
    chrono::DateTime::parse_from_rfc3339(value).is_ok()
}

fn merge_entry(
    mut incoming: DetailEntry,
    previous: Option<DetailEntry>,
    source: &DetailSource,
) -> Result<DetailEntry> {
    validate_identifier(&incoming.id)?;
    validate_identifier(&incoming.provider_id)?;
    validate_value(&incoming.body)?;
    if !mask_valid(&incoming.field_mask)
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
    let mut result = previous.unwrap_or(DetailEntry {
        id: incoming.id.clone(),
        provider_id: incoming.provider_id.clone(),
        author: None,
        title: None,
        state: None,
        body: DetailValue::default(),
        observed_body_state: DetailValueState::NotLoaded,
        updated_at: None,
        head_oid: None,
        field_mask: vec![],
        field_validations: vec![],
    });
    if result.provider_id != incoming.provider_id {
        return Err(invalid_detail());
    }
    if incoming.field_mask.contains(&DetailField::UpdatedAt)
        && result
            .updated_at
            .as_ref()
            .zip(incoming.updated_at.as_ref())
            .is_some_and(|(old, new)| timestamp_older(new, old))
        && result.field_validations.iter().any(|v| {
            v.field == DetailField::UpdatedAt
                && v.source == source.source
                && v.adapter_version == source.adapter_version
        })
    {
        return Ok(result);
    }
    result.observed_body_state = if incoming.field_mask.contains(&DetailField::Body) {
        incoming.body.state
    } else {
        DetailValueState::NotLoaded
    };
    for field in &incoming.field_mask {
        match field {
            DetailField::Body if incoming.body.state == DetailValueState::Known => {
                result.body = incoming.body.clone()
            }
            DetailField::Body => {
                if result.body.state != DetailValueState::Known {
                    result.body = incoming.body.clone();
                }
                continue;
            }
            DetailField::Author => result.author = incoming.author.clone(),
            DetailField::Title => result.title = incoming.title.clone(),
            DetailField::State => result.state = incoming.state.clone(),
            DetailField::UpdatedAt => result.updated_at = incoming.updated_at.clone(),
            DetailField::HeadOid => result.head_oid = incoming.head_oid.clone(),
        }
        result.field_validations.retain(|v| v.field != *field);
        result.field_validations.push(DetailFieldValidation {
            field: *field,
            validated_at: source.observed_at.clone(),
            source: source.source.clone(),
            adapter_version: source.adapter_version,
        });
    }
    result.field_mask = incoming.field_mask;
    Ok(result)
}

impl Store {
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
        let resume = old
            .as_ref()
            .is_some_and(|s| s.coverage.state == CoverageState::Partial && s.next_cursor.is_some());
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
        sqlx::query("INSERT INTO sync_scopes(account_id,scope,run_id,coverage_json,sync_json) VALUES(?,?,?,?,?) ON CONFLICT(account_id,scope) DO UPDATE SET run_id=excluded.run_id,sync_json=excluded.sync_json")
            .bind(account_id).bind(&scope).bind(&run_id).bind(encode(&coverage)?).bind(encode(&sync)?).execute(&mut *tx).await.map_err(storage_error)?;
        let source:Option<String>=sqlx::query_scalar("SELECT source_json FROM detail_observations WHERE account_id=? AND subject_id=? AND facet=?").bind(account_id).bind(subject_id).bind(tag(&facet)?).fetch_optional(&mut *tx).await.map_err(storage_error)?;
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
                .filter(|s| s.coverage.state == CoverageState::Complete && !resume)
                .and_then(|s| s.etag.clone()),
            source: source.map(|s| decode(&s)).transpose()?,
        };
        tx.commit().await.map_err(storage_error)?;
        Ok(result)
    }

    pub async fn apply_detail(&self, mut page: DetailCommit) -> Result<String> {
        if page.entries.len() > 100
            || page.source.source.is_empty()
            || page.source.source.len() > 256
            || page.source.adapter_version == 0
            || !mask_valid(&page.source.field_mask)
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
        let mut writer = self.inner.writer.lock().await;
        let mut tx = writer.begin().await.map_err(storage_error)?;
        epoch_in(&mut tx, &page.account_id, &page.authorization_epoch).await?;
        let account = account_in(&mut tx, &page.account_id, true).await?;
        let subject = subject_in(&mut tx, &page.account_id, &page.subject_id).await?;
        if let Some(binding) = &page.subject_binding {
            super::resource_metadata::validate_binding_in(
                &mut tx,
                &page.account_id,
                &subject,
                binding,
            )
            .await?;
        } else if page.metadata.is_some() {
            return Err(invalid_detail());
        }
        if page.facet.capability(&subject.kind).is_none() {
            return Err(invalid_detail());
        }
        if identities::instance_in(&mut tx, &account).await?.id != page.instance_id
            || metadata(&mut tx).await?.1 != page.authorization_view
        {
            return Err(stale());
        }
        let stored = scope_in(&mut tx, &page.account_id, &scope)
            .await?
            .ok_or_else(stale)?;
        if stored.run_id != page.run_id || stored.next_cursor != page.request_cursor {
            return Err(stale());
        }
        let previous=sqlx::query("SELECT body_json,source_json,value_source_json,stale_at FROM detail_observations WHERE account_id=? AND subject_id=? AND facet=?")
            .bind(&page.account_id).bind(&page.subject_id).bind(tag(&page.facet)?).fetch_optional(&mut *tx).await.map_err(storage_error)?;
        let previous_source: Option<DetailSource> = previous
            .as_ref()
            .map(|r| decode(r.get("source_json")))
            .transpose()?;
        let previous_value_source: Option<DetailSource> = previous
            .as_ref()
            .and_then(|r| r.get::<Option<String>, _>("value_source_json"))
            .map(|json| decode(&json))
            .transpose()?;
        let ordering_source = if page.facet == DetailFacet::Body {
            previous_value_source.as_ref()
        } else {
            previous_source.as_ref()
        };
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
                || previous_source.as_ref().is_none_or(|old| {
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
                    old.source == source.source && old.adapter_version == source.adapter_version
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
            state: if page.complete && validates {
                CoverageState::Complete
            } else {
                CoverageState::Partial
            },
            validated_at,
            remote_has_more: page.next_cursor.is_some(),
        };
        let revision = record_change(
            &mut tx,
            &page.account_id,
            positive_revision(&page.authorization_epoch)?,
            &scope,
            false,
        )
        .await?;
        sqlx::query("INSERT INTO detail_observations(account_id,subject_id,facet,authorization_epoch,facet_revision,body_json,source_json,value_source_json,observed_state,stale_at) VALUES(?,?,?,?,?,?,?,?,?,?) ON CONFLICT(account_id,subject_id,facet) DO UPDATE SET authorization_epoch=excluded.authorization_epoch,facet_revision=excluded.facet_revision,body_json=excluded.body_json,source_json=excluded.source_json,value_source_json=excluded.value_source_json,observed_state=excluded.observed_state,stale_at=excluded.stale_at")
            .bind(&page.account_id).bind(&page.subject_id).bind(tag(&page.facet)?).bind(&page.authorization_epoch).bind(&revision).bind(encode(&body)?).bind(encode(&page.source)?).bind(value_source.map(|s|encode(&s)).transpose()?).bind(tag(&observed_state)?).bind(stale_at).execute(&mut *tx).await.map_err(storage_error)?;
        if page.facet == DetailFacet::Body {
            super::resource_metadata::apply_in(&mut tx, &page, &subject.kind).await?;
        } else if page.metadata.is_some() {
            return Err(invalid_detail());
        }
        for incoming in page.entries {
            let old:Option<String>=sqlx::query_scalar("SELECT json FROM detail_entries WHERE account_id=? AND subject_id=? AND facet=? AND id=?")
                .bind(&page.account_id).bind(&page.subject_id).bind(tag(&page.facet)?).bind(&incoming.id).fetch_optional(&mut *tx).await.map_err(storage_error)?;
            let entry = merge_entry(incoming, old.map(|s| decode(&s)).transpose()?, &page.source)?;
            sqlx::query("INSERT INTO detail_entries(account_id,subject_id,facet,id,json,last_seen_run) VALUES(?,?,?,?,?,?) ON CONFLICT(account_id,subject_id,facet,id) DO UPDATE SET json=excluded.json,last_seen_run=excluded.last_seen_run")
                .bind(&page.account_id).bind(&page.subject_id).bind(tag(&page.facet)?).bind(&entry.id).bind(encode(&entry)?).bind(&page.run_id).execute(&mut *tx).await.map_err(storage_error)?;
        }
        if page.complete && !page.not_modified && page.facet != DetailFacet::Body {
            sqlx::query("DELETE FROM detail_entries WHERE account_id=? AND subject_id=? AND facet=? AND last_seen_run<>?")
                .bind(&page.account_id).bind(&page.subject_id).bind(tag(&page.facet)?).bind(&page.run_id).execute(&mut *tx).await.map_err(storage_error)?;
        }
        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM detail_entries WHERE account_id=? AND subject_id=? AND facet=?",
        )
        .bind(&page.account_id)
        .bind(&page.subject_id)
        .bind(tag(&page.facet)?)
        .fetch_one(&mut *tx)
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
        let retain_validator = page.complete && page.whole_scope && validates;
        sqlx::query("UPDATE sync_scopes SET next_cursor=?,etag=?,coverage_json=?,sync_json=?,access_denied=0,data_revision=data_revision+1 WHERE account_id=? AND scope=? AND run_id=?")
            .bind(&page.next_cursor).bind(if retain_validator { page.etag.or(if page.not_modified {stored.etag}else{None}) } else {None}).bind(encode(&coverage)?).bind(encode(&sync)?).bind(&page.account_id).bind(&scope).bind(&page.run_id).execute(&mut *tx).await.map_err(storage_error)?;
        if page.complete {
            sqlx::query("UPDATE detail_demand SET requested=0 WHERE account_id=? AND subject_id=? AND facet=? AND authorization_epoch=?")
                .bind(&page.account_id).bind(&page.subject_id).bind(tag(&page.facet)?).bind(&page.authorization_epoch).execute(&mut *tx).await.map_err(storage_error)?;
        }
        tx.commit().await.map_err(storage_error)?;
        Ok(revision)
    }
}
