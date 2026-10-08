//! Cache-only GitHub label review and immutable desired-membership admission.
use super::*;
use crate::commands::*;
use crate::detail::*;
use crate::label_sets::{native::*, *};
use crate::storage::command_admission::{self, CommandAdmissionPolicy, CommandProtection};
use crate::{MetadataField, ResourceMetadataSnapshot};
use std::collections::HashSet;

pub(crate) struct Admission;

#[async_trait::async_trait]
impl CommandAdmissionPolicy for Admission {
    const OPERATION_KIND: &'static str = OPERATION;
    const PAYLOAD_VERSION: u32 = 1;

    fn effect(
        &self,
        submission: &CommandSubmission,
    ) -> Result<Option<crate::effective::ItemIntentPatch>> {
        let payload = decode_submission(submission)?;
        Ok(Some(crate::effective::ItemIntentPatch {
            labels: Some(
                target_labels(&payload)?
                    .into_iter()
                    .map(|label| crate::DetailLabel {
                        provider_id: Some(label.provider_id),
                        name: label.name,
                        color: label.color,
                    })
                    .collect(),
            ),
            ..Default::default()
        }))
    }

    async fn validate(
        &self,
        tx: &mut Transaction<'_, Sqlite>,
        account: &RemoteAccount,
        submission: &CommandSubmission,
    ) -> Result<Vec<CommandProtection>> {
        let payload = decode_submission(submission)?;
        let frame = capture_in(tx, account, submission.target().id()).await?;
        if payload.base != frame.base || payload.request.context != context(account, &frame)? {
            return Err(stale());
        }
        let catalog = catalog_in(tx, account, &frame.repository.id).await?;
        if payload
            .request
            .add_labels
            .iter()
            .any(|label| !catalog.labels.iter().any(|known| known == label))
        {
            return Err(stale());
        }
        Ok(vec![CommandProtection::Facet {
            subject_id: submission.target().id().into(),
            facet: DetailFacet::Body,
        }])
    }
}

pub(crate) fn seal(payload: Payload, repository: &str) -> Result<CommandSubmission> {
    validate_payload(&payload)?;
    let kind = match payload.base.source.as_str() {
        "github/pull-detail/2026-03-10" => CommandTargetKind::PullRequest,
        "github/issue-detail/2026-03-10" => CommandTargetKind::Issue,
        _ => return Err(invalid()),
    };
    seal_command(CommandDraft {
        command_id: payload.request.command_id.clone(),
        account_id: payload.request.context.account_id.clone(),
        authorization_epoch: payload.request.context.authorization_epoch.clone(),
        target: CommandTarget::new(
            kind,
            payload.request.context.subject_id.clone(),
            Some(repository.into()),
        )?,
        payload,
        guards: vec![],
        dependencies: vec![],
    })
}

pub(crate) fn admission_error(
    error: command_admission::CommandAdmissionError,
) -> CollaborationError {
    match error {
        command_admission::CommandAdmissionError::Local(error) => error,
        _ => CollaborationError::invalid("Label command ID was already used"),
    }
}

impl Store {
    pub async fn label_set_snapshot(
        &self,
        account_id: &str,
        subject_id: &str,
    ) -> Result<LabelSetSnapshot> {
        validate_identifier(account_id)?;
        validate_identifier(subject_id)?;
        let mut tx = self.inner.readers.begin().await.map_err(storage_error)?;
        let account = account_in(&mut tx, account_id, false).await?;
        let (revision, authorization_view) = metadata(&mut tx).await?;
        let pending = pending_labels_in(&mut tx, account_id, subject_id).await?;
        let mut snapshot = LabelSetSnapshot {
            context: None,
            canonical_labels: vec![],
            effective_labels: vec![],
            available_labels: vec![],
            catalog_complete: false,
            catalog_truncated: false,
            availability: LabelSetAvailability::Unavailable,
            reason: None,
            pending_intent: pending,
            revision,
            authorization_view,
        };
        snapshot.reason = if account.provider != ProviderKind::Github
            || account.host != "github.com"
        {
            Some(LabelSetReason::UnsupportedProvider)
        } else if account.state != AccountState::Active {
            Some(LabelSetReason::AccountUnavailable)
        } else {
            match capture_in(&mut tx, &account, subject_id).await {
                Ok(frame) => {
                    snapshot.canonical_labels = frame.base.labels.clone();
                    snapshot.effective_labels =
                        effective_labels_in(&mut tx, account_id, subject_id, &frame.base.labels)
                            .await?;
                    let catalog = catalog_in(&mut tx, &account, &frame.repository.id).await?;
                    snapshot.available_labels = catalog.labels;
                    snapshot.catalog_truncated = catalog.truncated;
                    if snapshot.pending_intent.is_some() {
                        Some(LabelSetReason::PendingIntent)
                    } else {
                        snapshot.context = Some(context(&account, &frame)?);
                        snapshot.availability = LabelSetAvailability::Available;
                        None
                    }
                }
                Err(error)
                    if matches!(
                        error.code,
                        ErrorCode::NotFound
                            | ErrorCode::StaleView
                            | ErrorCode::PermissionDenied
                            | ErrorCode::InvalidInput
                    ) =>
                {
                    Some(label_unavailable_reason_in(&mut tx, &account, subject_id).await?)
                }
                Err(error) => return Err(error),
            }
        };
        if snapshot.pending_intent.is_some() {
            snapshot.context = None;
            snapshot.availability = LabelSetAvailability::Unavailable;
            snapshot.reason = Some(LabelSetReason::PendingIntent);
        }
        tx.commit().await.map_err(storage_error)?;
        Ok(snapshot)
    }

    pub(crate) async fn submit_label_set(
        &self,
        request: LabelSetRequest,
    ) -> Result<LabelSetReceipt> {
        validate_request(&request)?;
        let mut writer = self.inner.writer.acquire().await?;
        let mut tx = writer.begin().await.map_err(storage_error)?;
        epoch_in(
            &mut tx,
            &request.context.account_id,
            &request.context.authorization_epoch,
        )
        .await?;
        let old: Option<String> = sqlx::query_scalar(
            "SELECT command_id FROM commands WHERE account_id=? AND command_id=?",
        )
        .bind(&request.context.account_id)
        .bind(&request.command_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(storage_error)?;
        let submission = if old.is_some() {
            let command =
                delivery::load_in(&mut tx, &request.context.account_id, &request.command_id)
                    .await?;
            if command.operation_kind != OPERATION || command.payload_version != 1 {
                return Err(invalid());
            }
            let payload = decode_payload(&command)?;
            if payload.request != request {
                return Err(invalid());
            }
            seal(
                payload,
                command.repository_id.as_deref().ok_or_else(invalid)?,
            )?
        } else {
            let account = account_in(&mut tx, &request.context.account_id, true).await?;
            if pending_labels_in(&mut tx, &account.id, &request.context.subject_id)
                .await?
                .is_some()
            {
                return Err(stale());
            }
            let frame = capture_in(&mut tx, &account, &request.context.subject_id).await?;
            seal(
                Payload {
                    request,
                    base: frame.base,
                },
                &frame.repository.id,
            )?
        };
        let receipt = command_admission::admit_in(&mut tx, &submission, &Admission)
            .await
            .map_err(admission_error)?;
        tx.commit().await.map_err(storage_error)?;
        Ok(LabelSetReceipt {
            account_id: receipt.account_id,
            command_id: receipt.command_id,
            admitted_revision: receipt.admitted_revision,
            duplicate: receipt.duplicate,
        })
    }
}

pub(crate) fn context(account: &RemoteAccount, frame: &NativeFrame) -> Result<LabelSetContext> {
    let mut hash = Sha256::new();
    hash.update(b"gitru.github.label-set-review.v1");
    hash.update(
        encode(&(
            account.id.as_str(),
            account.authorization_epoch.as_str(),
            frame.authorization_view.as_str(),
            &frame.base,
            &frame.repository.id,
            &frame.subject.id,
            &frame.repository.full_name,
            &frame.body_revision,
        ))?
        .as_bytes(),
    );
    Ok(LabelSetContext {
        account_id: account.id.clone(),
        subject_id: frame.subject.id.clone(),
        authorization_epoch: account.authorization_epoch.clone(),
        authorization_view: frame.authorization_view.clone(),
        review_token: format!("{:x}", hash.finalize()),
    })
}

pub(crate) async fn capture_in(
    tx: &mut Transaction<'_, Sqlite>,
    account: &RemoteAccount,
    subject: &str,
) -> Result<NativeFrame> {
    if account.provider != ProviderKind::Github
        || account.host != "github.com"
        || account.state != AccountState::Active
    {
        return Err(stale());
    }
    epoch_in(tx, &account.id, &account.authorization_epoch).await?;
    let (_, authorization_view) = metadata(tx).await?;
    let mut item = details::subject_in(tx, &account.id, subject).await?;
    let repository_id = item.repository_id.as_ref().ok_or_else(stale)?;
    if !identities::accessible(tx, &account.id, repository_id, ResourceKind::Repository).await? {
        return Err(stale());
    }
    let repository_json: String =
        sqlx::query_scalar("SELECT json FROM repositories WHERE account_id=? AND id=?")
            .bind(&account.id)
            .bind(repository_id)
            .fetch_one(&mut **tx)
            .await
            .map_err(storage_error)?;
    let repository: RemoteRepository = decode(&repository_json)?;
    let scope = scope_in(tx, &account.id, &DetailFacet::Body.scope(subject))
        .await?
        .ok_or_else(stale)?;
    let denied: bool =
        sqlx::query_scalar("SELECT access_denied FROM sync_scopes WHERE account_id=? AND scope=?")
            .bind(&account.id)
            .bind(DetailFacet::Body.scope(subject))
            .fetch_one(&mut **tx)
            .await
            .map_err(storage_error)?;
    if denied {
        return Err(stale());
    }
    let row = sqlx::query("SELECT source_json,facet_revision FROM detail_observations WHERE account_id=? AND subject_id=? AND facet='body' AND authorization_epoch=?")
        .bind(&account.id)
        .bind(subject)
        .bind(&account.authorization_epoch)
        .fetch_optional(&mut **tx)
        .await
        .map_err(storage_error)?
        .ok_or_else(stale)?;
    let source: DetailSource = decode(row.get("source_json"))?;
    let metadata = resource_metadata::read_in(tx, account, subject)
        .await?
        .ok_or_else(stale)?;
    let expected_source = match item.kind {
        RemoteItemKind::Issue => "github/issue-detail/2026-03-10",
        RemoteItemKind::PullRequest => "github/pull-detail/2026-03-10",
        _ => return Err(stale()),
    };
    let known = |field| {
        metadata.fields.iter().any(|evidence| {
            evidence.field == field
                && evidence.saved_state == DetailValueState::Known
                && evidence.observed_state == DetailValueState::Known
                && evidence.source.as_ref().is_some_and(|source| {
                    source.source == expected_source && source.adapter_version == 1
                })
        })
    };
    if source.source != expected_source
        || source.adapter_version != 1
        || !known(MetadataField::Labels)
        || !known(MetadataField::UpdatedAt)
    {
        return Err(stale());
    }
    let labels = metadata
        .values
        .labels
        .into_iter()
        .map(|label| {
            Ok(LabelIdentity {
                provider_id: label.provider_id.ok_or_else(stale)?,
                name: label.name,
                color: label.color,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    let labels = sorted(labels);
    if !labels_valid(&labels, MAX_LABELS) {
        return Err(stale());
    }
    let base = LabelBase {
        labels,
        updated_at: metadata.values.updated_at.ok_or_else(stale)?,
        source: expected_source.into(),
        repository_native_id: repository.provider_id.clone(),
        subject_native_id: item.provider_id.clone(),
        number: item.number.clone().ok_or_else(stale)?,
    };
    item.body = None;
    item.title.clear();
    let frame = NativeFrame {
        repository,
        subject: item,
        base,
        authorization_view,
        body_revision: row.get("facet_revision"),
        run_id: scope.run_id,
    };
    encode_bounded(&frame)?;
    Ok(frame)
}

pub(crate) async fn prepare_in(
    tx: &mut Transaction<'_, Sqlite>,
    command: &crate::delivery::DeliveryCommand,
    account: &RemoteAccount,
) -> Result<Vec<u8>> {
    let mut frame = capture_in(tx, account, &command.target_id).await?;
    let payload = decode_payload(command)?;
    if frame.base.repository_native_id != payload.base.repository_native_id
        || frame.base.subject_native_id != payload.base.subject_native_id
        || frame.base.number != payload.base.number
        || frame.base.source != payload.base.source
    {
        return Err(stale());
    }
    frame.run_id = Uuid::new_v4().to_string();
    sqlx::query(
        "UPDATE sync_scopes SET run_id=?,next_cursor=NULL,etag=NULL WHERE account_id=? AND scope=?",
    )
    .bind(&frame.run_id)
    .bind(&account.id)
    .bind(DetailFacet::Body.scope(&command.target_id))
    .execute(&mut **tx)
    .await
    .map_err(storage_error)?;
    encode_bounded(&frame)
}

pub(crate) async fn validate_frame_in(
    tx: &mut Transaction<'_, Sqlite>,
    account: &RemoteAccount,
    frame: &NativeFrame,
) -> Result<()> {
    let current = capture_in(tx, account, &frame.subject.id).await?;
    if context(account, &current)? != context(account, frame)? || current.run_id != frame.run_id {
        return Err(stale());
    }
    Ok(())
}

pub(crate) async fn validate_completion_in(
    tx: &mut Transaction<'_, Sqlite>,
    evidence: &Evidence,
) -> Result<()> {
    let account = account_in(tx, &evidence.account_id, true).await?;
    if account.actor_id != evidence.actor_id
        || account.authorization_epoch != evidence.authorization_epoch
    {
        return Err(stale());
    }
    validate_frame_in(tx, &account, &evidence.frame).await
}

async fn pending_labels_in(
    tx: &mut Transaction<'_, Sqlite>,
    account: &str,
    subject: &str,
) -> Result<Option<crate::PendingItemIntent>> {
    let mut pending = effective::pending_in(tx, account, &[subject]).await?.pop();
    if let Some(pending) = &mut pending {
        pending
            .commands
            .retain(|command| command.fields.contains(&crate::IntentField::Labels));
    }
    Ok(pending.filter(|pending| !pending.commands.is_empty()))
}

async fn effective_labels_in(
    tx: &mut Transaction<'_, Sqlite>,
    account: &str,
    subject: &str,
    canonical: &[LabelIdentity],
) -> Result<Vec<LabelIdentity>> {
    let Some(labels) = effective::patch_in(tx, account, subject)
        .await?
        .and_then(|patch| patch.labels)
    else {
        return Ok(canonical.to_vec());
    };
    let labels = labels
        .into_iter()
        .map(|label| {
            Ok(LabelIdentity {
                provider_id: label.provider_id.ok_or_else(stale)?,
                name: label.name,
                color: label.color,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    if !labels_valid(&labels, MAX_LABELS) {
        return Err(CollaborationError::storage());
    }
    Ok(sorted(labels))
}

struct Catalog {
    labels: Vec<LabelIdentity>,
    truncated: bool,
}

async fn catalog_in(
    tx: &mut Transaction<'_, Sqlite>,
    account: &RemoteAccount,
    repository: &str,
) -> Result<Catalog> {
    let rows: Vec<String> = sqlx::query_scalar(
        "SELECT m.metadata_json FROM detail_resource_metadata m JOIN items i ON i.account_id=m.account_id AND i.id=m.subject_id WHERE m.account_id=? AND m.authorization_epoch=? AND i.repository_id=? ORDER BY i.id LIMIT 201",
    )
    .bind(&account.id)
    .bind(&account.authorization_epoch)
    .bind(repository)
    .fetch_all(&mut **tx)
    .await
    .map_err(storage_error)?;
    let row_truncated = rows.len() == 201;
    let mut observed = Vec::new();
    for json in rows {
        let metadata: ResourceMetadataSnapshot = decode(&json)?;
        let source = match metadata.kind {
            RemoteItemKind::Issue => "github/issue-detail/2026-03-10",
            RemoteItemKind::PullRequest => "github/pull-detail/2026-03-10",
            _ => continue,
        };
        let authoritative = metadata.fields.iter().any(|field| {
            field.field == MetadataField::Labels
                && field.saved_state == DetailValueState::Known
                && field.observed_state == DetailValueState::Known
                && field
                    .source
                    .as_ref()
                    .is_some_and(|value| value.source == source && value.adapter_version == 1)
        });
        if !authoritative {
            continue;
        }
        for label in metadata.values.labels {
            let Some(provider_id) = label.provider_id else {
                continue;
            };
            let identity = LabelIdentity {
                provider_id,
                name: label.name,
                color: label.color,
            };
            if label_valid(&identity) {
                observed.push(identity);
            }
        }
    }
    observed = sorted(observed);
    observed.dedup();
    let ambiguous_ids: HashSet<String> = observed
        .iter()
        .filter(|candidate| {
            observed
                .iter()
                .any(|other| other.provider_id == candidate.provider_id && other != *candidate)
        })
        .map(|label| label.provider_id.clone())
        .collect();
    let ambiguous_names: HashSet<String> = observed
        .iter()
        .filter(|candidate| {
            observed.iter().any(|other| {
                other.name.to_lowercase() == candidate.name.to_lowercase() && other != *candidate
            })
        })
        .map(|label| label.name.to_lowercase())
        .collect();
    observed.retain(|label| {
        !ambiguous_ids.contains(&label.provider_id)
            && !ambiguous_names.contains(&label.name.to_lowercase())
    });
    let truncated = row_truncated || observed.len() > MAX_LABELS;
    observed.truncate(MAX_LABELS);
    Ok(Catalog {
        labels: observed,
        truncated,
    })
}

async fn label_unavailable_reason_in(
    tx: &mut Transaction<'_, Sqlite>,
    account: &RemoteAccount,
    subject: &str,
) -> Result<LabelSetReason> {
    let metadata = resource_metadata::read_in(tx, account, subject).await?;
    Ok(
        if metadata.as_ref().is_some_and(|metadata| {
            metadata.fields.iter().any(|field| {
                field.field == MetadataField::Labels
                    && matches!(
                        (field.saved_state, field.observed_state),
                        (DetailValueState::Oversized, _) | (_, DetailValueState::Oversized)
                    )
            })
        }) {
            LabelSetReason::OversizedLabels
        } else {
            LabelSetReason::MissingLabels
        },
    )
}
