//! Offline text admission and fresh native execution capture.
use super::*;
use crate::commands::*;
use crate::detail::*;
use crate::storage::command_admission::{self, CommandAdmissionPolicy, CommandProtection};
use crate::text_edits::{native::*, *};

pub(crate) struct Admission;
#[async_trait::async_trait]
impl CommandAdmissionPolicy for Admission {
    const OPERATION_KIND: &'static str = OPERATION;
    const PAYLOAD_VERSION: u32 = 1;
    fn effect(
        &self,
        submission: &CommandSubmission,
    ) -> Result<Option<crate::effective::ItemIntentPatch>> {
        let p = decode_submission(submission)?;
        Ok(Some(crate::effective::ItemIntentPatch {
            title: p.request.title,
            body: p
                .request
                .body
                .map(|text| crate::effective::BodyIntent { text: Some(text) }),
            ..Default::default()
        }))
    }
    async fn validate(
        &self,
        tx: &mut Transaction<'_, Sqlite>,
        account: &RemoteAccount,
        submission: &CommandSubmission,
    ) -> Result<Vec<CommandProtection>> {
        let p = decode_submission(submission)?;
        let frame = capture_in(tx, account, submission.target().id()).await?;
        if p.base != frame.base || p.request.context != context(account, &frame)? {
            return Err(stale());
        }
        if p.request.title.as_ref().is_some_and(|v| v == &p.base.title)
            || p.request
                .body
                .as_deref()
                .is_some_and(|v| body_equal(Some(v), p.base.body.as_deref()))
        {
            return Err(CollaborationError::invalid(
                "Only changed text fields may be submitted",
            ));
        }
        Ok(vec![CommandProtection::Facet {
            subject_id: submission.target().id().into(),
            facet: DetailFacet::Body,
        }])
    }
}
pub(crate) fn seal(payload: Payload, repository: &str) -> Result<CommandSubmission> {
    let kind = if payload.base.source == "github/pull-detail/2026-03-10" {
        CommandTargetKind::PullRequest
    } else if payload.base.source == "github/issue-detail/2026-03-10" {
        CommandTargetKind::Issue
    } else {
        return Err(invalid());
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
pub(crate) fn admission_error(e: command_admission::CommandAdmissionError) -> CollaborationError {
    match e {
        command_admission::CommandAdmissionError::Local(e) => e,
        _ => CollaborationError::invalid("Text edit command ID was already used"),
    }
}
impl Store {
    pub async fn text_edit_snapshot(
        &self,
        account_id: &str,
        subject_id: &str,
    ) -> Result<TextEditSnapshot> {
        let mut tx = self.inner.readers.begin().await.map_err(storage_error)?;
        let account = account_in(&mut tx, account_id, false).await?;
        let (revision, authorization_view) = metadata(&mut tx).await?;
        let pending = effective::pending_in(&mut tx, account_id, &[subject_id])
            .await?
            .pop();
        let mut snapshot = TextEditSnapshot {
            context: None,
            title: None,
            body: None,
            availability: TextEditAvailability::Unavailable,
            reason: None,
            pending_intent: pending,
            revision,
            authorization_view,
        };
        snapshot.reason = if account.provider != ProviderKind::Github
            || account.host != "github.com"
        {
            Some(TextEditReason::UnsupportedProvider)
        } else if account.state != AccountState::Active {
            Some(TextEditReason::AccountUnavailable)
        } else if snapshot.pending_intent.is_some() {
            Some(TextEditReason::PendingIntent)
        } else {
            match capture_in(&mut tx, &account, subject_id).await {
                Ok(frame) => {
                    snapshot.context = Some(context(&account, &frame)?);
                    snapshot.title = Some(frame.base.title);
                    snapshot.body = frame.base.body;
                    snapshot.availability = TextEditAvailability::Available;
                    None
                }
                Err(error)
                    if matches!(
                        error.code,
                        ErrorCode::NotFound | ErrorCode::StaleView | ErrorCode::PermissionDenied
                    ) =>
                {
                    Some(TextEditReason::MissingBase)
                }
                Err(error) if error.code == ErrorCode::InvalidInput => {
                    Some(TextEditReason::OversizedText)
                }
                Err(error) => return Err(error),
            }
        };
        tx.commit().await.map_err(storage_error)?;
        Ok(snapshot)
    }
    pub(crate) async fn submit_text_edit(
        &self,
        request: TextEditRequest,
    ) -> Result<TextEditReceipt> {
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
            if !effective::pending_in(&mut tx, &account.id, &[&request.context.subject_id])
                .await?
                .is_empty()
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
        Ok(TextEditReceipt {
            account_id: receipt.account_id,
            command_id: receipt.command_id,
            admitted_revision: receipt.admitted_revision,
            duplicate: receipt.duplicate,
        })
    }
}
pub(crate) fn context(account: &RemoteAccount, frame: &NativeFrame) -> Result<TextEditContext> {
    let mut hash = Sha256::new();
    hash.update(b"gitru.github.text-edit-review.v1");
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
    Ok(TextEditContext {
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
    let json: String =
        sqlx::query_scalar("SELECT json FROM repositories WHERE account_id=? AND id=?")
            .bind(&account.id)
            .bind(repository_id)
            .fetch_one(&mut **tx)
            .await
            .map_err(storage_error)?;
    let repository: RemoteRepository = decode(&json)?;
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
    let row=sqlx::query("SELECT body_json,value_source_json,facet_revision,observed_state FROM detail_observations WHERE account_id=? AND subject_id=? AND facet='body' AND authorization_epoch=?").bind(&account.id).bind(subject).bind(&account.authorization_epoch).fetch_optional(&mut **tx).await.map_err(storage_error)?.ok_or_else(stale)?;
    let body: DetailValue = decode(row.get("body_json"))?;
    let source: DetailSource = decode(
        row.get::<Option<&str>, _>("value_source_json")
            .ok_or_else(stale)?,
    )?;
    let observed: String = row.get("observed_state");
    let meta = resource_metadata::read_in(tx, account, subject)
        .await?
        .ok_or_else(stale)?;
    let expected_source = match item.kind {
        RemoteItemKind::Issue => "github/issue-detail/2026-03-10",
        RemoteItemKind::PullRequest => "github/pull-detail/2026-03-10",
        _ => return Err(stale()),
    };
    let known = |field| {
        meta.fields.iter().any(|f| {
            f.field == field
                && f.saved_state == DetailValueState::Known
                && f.observed_state == DetailValueState::Known
                && f.source
                    .as_ref()
                    .is_some_and(|s| s.source == expected_source && s.adapter_version == 1)
        })
    };
    if body.state != DetailValueState::Known
        || observed != "known"
        || source.source != expected_source
        || source.adapter_version != 1
        || !known(crate::MetadataField::Title)
        || !known(crate::MetadataField::State)
        || !known(crate::MetadataField::UpdatedAt)
    {
        return Err(stale());
    }
    let head_known = known(crate::MetadataField::Head);
    let title = meta.values.title.ok_or_else(stale)?;
    if title.len() > 1024
        || title.chars().count() > 256
        || body.text.as_ref().is_some_and(|v| v.len() > MAX_BODY)
    {
        return Err(invalid());
    }
    let state = meta.values.state.ok_or_else(stale)?;
    let head = meta.values.head.as_ref().map(|h| h.oid.clone());
    if item.kind == RemoteItemKind::PullRequest
        && (!head_known || head.is_none() || head != item.head_oid)
    {
        return Err(stale());
    }
    let base = TextBase {
        title,
        body: body.text,
        state,
        updated_at: meta.values.updated_at.ok_or_else(stale)?,
        head,
        source: expected_source.into(),
        repository_native_id: repository.provider_id.clone(),
        subject_native_id: item.provider_id.clone(),
        number: item.number.clone().ok_or_else(stale)?,
    };
    // Keep the captured identity small; descriptions and provider JSON are not
    // request authority and must not amplify persisted execution context.
    item.body = None;
    let frame = NativeFrame {
        repository,
        subject: item,
        base,
        authorization_view,
        body_revision: row.get::<String, _>("facet_revision"),
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
