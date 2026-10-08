//! Canonical evidence is issued and consumed inside one delivery transaction.
//! Policies can access the transaction for unrelated canonical work, but only
//! these guarded helpers can attest that optimistic item fields were replaced.
#![cfg_attr(not(test), allow(dead_code))]
use super::*;
use crate::delivery::{DeliveryCommand, EvidencePurpose};
use crate::effective::BodyIntent;
use crate::{DetailCommit, DetailFacet, DetailValue, DetailValueState, IntentField, MetadataField};

type BodyFrame = (String, Option<String>, Option<String>, String);
struct CanonicalWitness {
    item: RemoteItem,
    fields: Vec<IntentField>,
    body_frame: Option<BodyFrame>,
}

/// No public constructor or transferable receipt: the witness belongs to this
/// exact mutable SQLite transaction and command, and is consumed before state
/// transition. Rechecking its saved frame also detects later raw SQL changes.
pub(crate) struct DeliveryFinalization<'a, 'db> {
    tx: &'a mut Transaction<'db, Sqlite>,
    command: &'a DeliveryCommand,
    account: &'a RemoteAccount,
    required: Vec<IntentField>,
    witness: Option<CanonicalWitness>,
}

/// A native adapter's canonical notification response, separate from authored
/// intent. The captured raw base and view must still be exact. The policy must
/// validate operation-specific receipt provenance before this helper is called.
pub(crate) struct CanonicalNotificationObservation {
    pub expected: RemoteItem,
    pub authorization_view: String,
    pub instance_id: String,
    pub source: crate::MetadataSource,
    pub values: ItemIntentPatch,
}

impl<'a, 'db> DeliveryFinalization<'a, 'db> {
    pub(in crate::storage) async fn new(
        tx: &'a mut Transaction<'db, Sqlite>,
        command: &'a DeliveryCommand,
        account: &'a RemoteAccount,
        purpose: EvidencePurpose,
    ) -> Result<Self> {
        let required = if purpose == EvidencePurpose::Confirmed {
            let effect: Option<(i64, String)> = sqlx::query_as(
                "SELECT version,patch_json FROM command_effects WHERE account_id=? AND command_id=? AND submission_hash=?",
            ).bind(&command.account_id).bind(&command.command_id).bind(command.hash.as_slice())
                .fetch_optional(&mut **tx).await.map_err(storage_error)?;
            match effect {
                Some((EFFECT_VERSION, json)) => decode::<ItemIntentPatch>(&json)?.fields(),
                Some(_) => return Err(CollaborationError::storage()),
                None => vec![],
            }
        } else {
            vec![]
        };
        Ok(Self {
            tx,
            command,
            account,
            required,
            witness: None,
        })
    }

    /// Raw canonical work cannot produce item-effect coverage. A later raw edit
    /// after materialization invalidates the scoped witness during finish().
    pub(crate) fn transaction(&mut self) -> &mut Transaction<'db, Sqlite> {
        self.tx
    }

    /// Apply one already captured authoritative PR/issue Body observation through
    /// all ordinary lease, authorization, identity, source and head fences. Known
    /// canonical scalar values are copied to the summary/FTS only after acceptance;
    /// they may differ from the submitted values after provider normalization.
    pub(crate) async fn observe_body(&mut self, page: DetailCommit) -> Result<()> {
        if self.witness.is_some()
            || page.account_id != self.account.id
            || page.authorization_epoch != self.account.authorization_epoch
            || page.subject_id != self.command.target_id
            || page.facet != DetailFacet::Body
            || page.subject_binding.is_none()
            || !page.whole_scope
            || !page.complete
            || page.request_cursor.is_some()
            || page.next_cursor.is_some()
            || !page.entries.is_empty()
        {
            return Err(invalid());
        }
        let mut item = raw_item_in(self.tx, &self.account.id, &self.command.target_id).await?;
        if !matches!(
            item.kind,
            RemoteItemKind::PullRequest | RemoteItemKind::Issue
        ) {
            return Err(invalid());
        }
        if self.command.target_kind != tag(&item.kind)? {
            return Err(invalid());
        }
        let provider_updated_at = page
            .source
            .provider_updated_at
            .clone()
            .ok_or_else(invalid)?;
        let canonical_updated = canonical_time(&provider_updated_at, &item.updated_at)?;
        let incoming_body = page.body.clone();
        let incoming_metadata = page.metadata.clone();
        let not_modified = page.not_modified;
        let observed = page.source.observed_at.clone();
        let source = page.source.source.clone();
        let adapter_version = page.source.adapter_version;
        super::super::details::apply_detail_in(self.tx, page).await?;
        let mut values = ItemIntentPatch::default();
        let metadata =
            super::super::resource_metadata::read_in(self.tx, self.account, &item.id).await?;
        if let Some(metadata) = metadata {
            let known = |field| {
                metadata.fields.iter().any(|e| {
                    e.field == field
                        && e.saved_state == DetailValueState::Known
                        && e.observed_state == DetailValueState::Known
                        && e.validated_at.as_deref() == Some(observed.as_str())
                        && e.source.as_ref().is_some_and(|s| {
                            s.source == source
                                && s.adapter_version == adapter_version
                                && s.observed_at == observed
                                && s.provider_updated_at.as_deref()
                                    == Some(provider_updated_at.as_str())
                        })
                })
            };
            let explicitly_observed = |field| {
                not_modified
                    || incoming_metadata.as_ref().is_some_and(|m| {
                        m.fields
                            .iter()
                            .any(|f| f.field == field && f.state == DetailValueState::Known)
                    })
            };
            if known(MetadataField::Title)
                && explicitly_observed(MetadataField::Title)
                && (not_modified
                    || incoming_metadata
                        .as_ref()
                        .is_some_and(|m| m.values.title == metadata.values.title))
            {
                values.title = metadata.values.title.clone();
            }
            if known(MetadataField::State)
                && explicitly_observed(MetadataField::State)
                && (not_modified
                    || incoming_metadata
                        .as_ref()
                        .is_some_and(|m| m.values.state == metadata.values.state))
            {
                values.state = metadata.values.state.clone();
            }
            if known(MetadataField::Labels)
                && explicitly_observed(MetadataField::Labels)
                && (not_modified
                    || incoming_metadata
                        .as_ref()
                        .is_some_and(|incoming| incoming.values.labels == metadata.values.labels))
            {
                values.labels = Some(metadata.values.labels.clone());
            }
        }
        let frame = body_frame_in(self.tx, &self.account.id, &item.id).await?;
        if let Some((body, source_json, _, _)) = &frame {
            let body: DetailValue = decode(body)?;
            let value_source = source_json
                .as_deref()
                .map(decode::<crate::DetailSource>)
                .transpose()?;
            if body.state == DetailValueState::Known
                && (not_modified
                    || incoming_body.state == DetailValueState::Known
                        && incoming_body.text == body.text)
                && value_source.is_some_and(|s| {
                    s.source == source
                        && s.adapter_version == adapter_version
                        && s.observed_at == observed
                        && s.provider_updated_at.as_deref() == Some(provider_updated_at.as_str())
                })
            {
                values.body = Some(BodyIntent { text: body.text });
            }
        }
        values.validate(&item.kind)?;
        let fields = values.fields();
        if !self.required.iter().all(|field| fields.contains(field)) {
            return Err(invalid());
        }
        values.apply(&mut item);
        item.updated_at = canonical_updated;
        write_item_in(self.tx, &item).await?;
        self.witness = Some(CanonicalWitness {
            item,
            fields,
            body_frame: frame,
        });
        Ok(())
    }

    /// Notification fields have no Body facet. Require exact captured native
    /// summary, current visibility/view/instance and a non-regressing provider
    /// activity timestamp before copying this response's bounded scalar values.
    pub(crate) async fn observe_notification(
        &mut self,
        observation: CanonicalNotificationObservation,
    ) -> Result<()> {
        if self.witness.is_some()
            || observation.expected.kind != RemoteItemKind::Notification
            || observation.expected.account_id != self.account.id
            || observation.expected.id != self.command.target_id
            || self.command.target_kind != "notification"
            || observation.values.body.is_some()
            || observation.source.source.is_empty()
            || observation.source.source.len() > 256
            || observation.source.adapter_version == 0
            || observation.source.observed_at.len() > 128
            || chrono::DateTime::parse_from_rfc3339(&observation.source.observed_at).is_err()
        {
            return Err(invalid());
        }
        epoch_in(self.tx, &self.account.id, &self.account.authorization_epoch).await?;
        let account = account_in(self.tx, &self.account.id, true).await?;
        if identities::instance_in(self.tx, &account).await?.id != observation.instance_id
            || metadata(self.tx).await?.1 != observation.authorization_view
            || !identities::accessible(
                self.tx,
                &self.account.id,
                &self.command.target_id,
                ResourceKind::Notification,
            )
            .await?
        {
            return Err(stale());
        }
        let mut item = raw_item_in(self.tx, &self.account.id, &self.command.target_id).await?;
        if item != observation.expected {
            return Err(stale());
        }
        let updated = observation
            .source
            .provider_updated_at
            .as_deref()
            .ok_or_else(invalid)?;
        let canonical_updated = canonical_time(updated, &item.updated_at)?;
        observation.values.validate(&item.kind)?;
        let fields = observation.values.fields();
        if !self.required.iter().all(|field| fields.contains(field)) {
            return Err(invalid());
        }
        observation.values.apply(&mut item);
        item.updated_at = canonical_updated;
        write_item_in(self.tx, &item).await?;
        self.witness = Some(CanonicalWitness {
            item,
            fields,
            body_frame: None,
        });
        Ok(())
    }

    pub(in crate::storage) async fn finish(self) -> Result<()> {
        if self.required.is_empty() {
            return Ok(());
        }
        let witness = self.witness.ok_or_else(invalid)?;
        let search: Vec<(String, String)> =
            sqlx::query_as("SELECT title,body FROM items_fts WHERE account_id=? AND id=? LIMIT 2")
                .bind(&self.account.id)
                .bind(&self.command.target_id)
                .fetch_all(&mut **self.tx)
                .await
                .map_err(storage_error)?;
        let search_body: String = witness
            .item
            .body
            .as_deref()
            .unwrap_or("")
            .chars()
            .take(MAX_FTS_BODY_CHARS)
            .collect();
        if search != [(witness.item.title.clone(), search_body)] {
            return Err(invalid());
        }
        if !self
            .required
            .iter()
            .all(|field| witness.fields.contains(field))
            || raw_item_in(self.tx, &self.account.id, &self.command.target_id).await?
                != witness.item
            || body_frame_in(self.tx, &self.account.id, &self.command.target_id).await?
                != witness.body_frame
        {
            return Err(invalid());
        }
        Ok(())
    }
}

fn canonical_time(candidate: &str, saved: &str) -> Result<String> {
    if candidate.len() > 128 {
        return Err(invalid());
    }
    let next = chrono::DateTime::parse_from_rfc3339(candidate).map_err(|_| invalid())?;
    let old = chrono::DateTime::parse_from_rfc3339(saved).map_err(|_| invalid())?;
    if next < old {
        return Err(stale());
    }
    Ok(next
        .with_timezone(&chrono::Utc)
        .to_rfc3339_opts(chrono::SecondsFormat::Nanos, true))
}
fn invalid() -> CollaborationError {
    CollaborationError::invalid(
        "Confirmation requires current canonical observation for every authored field",
    )
}
fn stale() -> CollaborationError {
    CollaborationError::new(
        ErrorCode::StaleView,
        "Canonical delivery observation is no longer current",
    )
}
async fn raw_item_in(
    tx: &mut Transaction<'_, Sqlite>,
    account: &str,
    subject: &str,
) -> Result<RemoteItem> {
    let json: String = sqlx::query_scalar("SELECT json FROM items WHERE account_id=? AND id=?")
        .bind(account)
        .bind(subject)
        .fetch_one(&mut **tx)
        .await
        .map_err(storage_error)?;
    decode(&json)
}
async fn body_frame_in(
    tx: &mut Transaction<'_, Sqlite>,
    account: &str,
    subject: &str,
) -> Result<Option<BodyFrame>> {
    sqlx::query_as("SELECT d.body_json,d.value_source_json,m.metadata_json,d.facet_revision FROM detail_observations d LEFT JOIN detail_resource_metadata m ON m.account_id=d.account_id AND m.subject_id=d.subject_id AND m.authorization_epoch=d.authorization_epoch WHERE d.account_id=? AND d.subject_id=? AND d.facet='body'")
        .bind(account).bind(subject).fetch_optional(&mut **tx).await.map_err(storage_error)
}
async fn write_item_in(tx: &mut Transaction<'_, Sqlite>, item: &RemoteItem) -> Result<()> {
    sqlx::query("UPDATE items SET state=?,updated_at=?,json=? WHERE account_id=? AND id=?")
        .bind(&item.state)
        .bind(&item.updated_at)
        .bind(encode(item)?)
        .bind(&item.account_id)
        .bind(&item.id)
        .execute(&mut **tx)
        .await
        .map_err(storage_error)?;
    sqlx::query("DELETE FROM items_fts WHERE account_id=? AND id=?")
        .bind(&item.account_id)
        .bind(&item.id)
        .execute(&mut **tx)
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
        .bind(&item.account_id)
        .bind(&item.id)
        .bind(&item.title)
        .bind(body)
        .execute(&mut **tx)
        .await
        .map_err(storage_error)?;
    let scope = if item.kind == RemoteItemKind::Notification {
        "notifications".to_owned()
    } else {
        format!(
            "repo:{}:{}",
            item.repository_id.as_deref().ok_or_else(invalid)?,
            tag(&item.kind)?
        )
    };
    let account = account_in(tx, &item.account_id, true).await?;
    // Fence both local list cursors and network pages captured before this
    // canonical observation, including equal provider timestamps. Preserve
    // accepted pagination/membership; only conditional validators are obsolete.
    sqlx::query(
        "UPDATE sync_scopes SET data_revision=data_revision+1,etag=NULL,last_modified=NULL WHERE account_id=? AND scope=?",
    )
    .bind(&item.account_id)
    .bind(&scope)
    .execute(&mut **tx)
    .await
    .map_err(storage_error)?;
    record_change(
        tx,
        &item.account_id,
        positive_revision(&account.authorization_epoch)?,
        &scope,
        false,
    )
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests;
