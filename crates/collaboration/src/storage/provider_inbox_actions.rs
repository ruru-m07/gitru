//! Native inbox admission and final local claim share the writer snapshot.
use super::command_admission::{CommandAdmissionPolicy, CommandProtection};
use super::*;
use crate::commands::*;
use crate::effective::ItemIntentPatch;
use crate::provider_inbox_actions::{native::*, *};

type StoredAdmission = (String, i64, Vec<u8>, Option<String>, String);

pub(crate) async fn frame_in(
    tx: &mut Transaction<'_, Sqlite>,
    account: &RemoteAccount,
    id: &str,
) -> Result<Frame> {
    if !identities::accessible(tx, &account.id, id, ResourceKind::Notification).await? {
        return Err(CollaborationError::new(
            ErrorCode::NotFound,
            "Notification is unavailable",
        ));
    }
    let json: String = sqlx::query_scalar(
        "SELECT json FROM items WHERE account_id=? AND id=? AND kind='notification'",
    )
    .bind(&account.id)
    .bind(id)
    .fetch_one(&mut **tx)
    .await
    .map_err(storage_error)?;
    let item: RemoteItem = decode(&json)?;
    Ok(Frame {
        item,
        view: metadata(tx).await?.1,
        instance: identities::instance_in(tx, account).await?.id,
    })
}
async fn project_in(tx: &mut Transaction<'_, Sqlite>, item: &RemoteItem) -> Result<String> {
    let id = item.repository_id.as_deref().ok_or_else(invalid)?;
    sqlx::query_scalar("SELECT provider_id FROM repositories WHERE account_id=? AND id=?")
        .bind(&item.account_id)
        .bind(id)
        .fetch_optional(&mut **tx)
        .await
        .map_err(storage_error)?
        .ok_or_else(invalid)
}
fn descriptors(
    account: &RemoteAccount,
    item: &RemoteItem,
    registered: bool,
    pending: bool,
) -> Vec<ProviderInboxActionDescriptor> {
    use ProviderInboxActionAvailability as A;
    use ProviderInboxActionReason as R;
    [ProviderInboxAction::MarkRead, ProviderInboxAction::MarkDone]
        .into_iter()
        .map(|action| {
            let mut reason = match (account.provider, action) {
                (ProviderKind::Github, ProviderInboxAction::MarkRead)
                | (ProviderKind::Gitlab, ProviderInboxAction::MarkDone) => None,
                (ProviderKind::Github, ProviderInboxAction::MarkDone) => Some(R::NotImplemented),
                _ => Some(R::ProviderSemantics),
            };
            let mut availability = if reason.is_some() {
                A::Unsupported
            } else {
                A::Available
            };
            if reason.is_none() {
                reason = if !registered {
                    Some(R::NotImplemented)
                } else if !account.notifications_supported {
                    Some(R::UnsupportedCredential)
                } else if account.state != AccountState::Active {
                    Some(R::AuthenticationRequired)
                } else if item.repository_id.is_none()
                    || !item
                        .provider_id
                        .parse::<u64>()
                        .is_ok_and(|n| n > 0 && n.to_string() == item.provider_id)
                {
                    Some(R::MissingSourceEvidence)
                } else if pending {
                    Some(R::PendingCommand)
                } else {
                    match (&item.native_inbox, action) {
                        (
                            Some(NativeInboxState::Notification { unread: true }),
                            ProviderInboxAction::MarkRead,
                        ) if item.unread == Some(true) => None,
                        (
                            Some(NativeInboxState::Notification { unread: false }),
                            ProviderInboxAction::MarkRead,
                        ) => Some(R::AlreadyApplied),
                        (
                            Some(NativeInboxState::Todo {
                                completion: TodoCompletion::Pending,
                                ..
                            }),
                            ProviderInboxAction::MarkDone,
                        ) if item.unread.is_none() && item.state == "pending" => None,
                        (
                            Some(NativeInboxState::Todo {
                                completion: TodoCompletion::Done,
                                ..
                            }),
                            ProviderInboxAction::MarkDone,
                        ) => Some(R::AlreadyApplied),
                        _ => Some(R::MissingSourceEvidence),
                    }
                };
                if reason.is_some() {
                    availability = A::Unavailable;
                }
            }
            ProviderInboxActionDescriptor {
                action,
                availability,
                reason,
                activity_policy: ProviderInboxActivityPolicy::BestEffortCurrentItem,
            }
        })
        .collect()
}
async fn pending_in(tx: &mut Transaction<'_, Sqlite>, account: &str, id: &str) -> Result<bool> {
    sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM commands c JOIN command_effects e USING(account_id,command_id) WHERE c.account_id=? AND c.target_id=? AND c.state IN ('queued','sending','retry_wait','accepted','outcome_unknown','conflict'))").bind(account).bind(id).fetch_one(&mut **tx).await.map_err(storage_error)
}
impl Store {
    pub(crate) async fn provider_inbox_actions(
        &self,
        query: ProviderInboxActionsQuery,
        registered: bool,
    ) -> Result<ProviderInboxActionsSnapshot> {
        query.validate()?;
        let mut tx = self.inner.readers.begin().await.map_err(storage_error)?;
        let account = account_in(&mut tx, &query.account_id, true).await?;
        let frame = frame_in(&mut tx, &account, &query.subject_id).await?;
        let pending = pending_in(&mut tx, &account.id, &frame.item.id).await?;
        let revision = metadata(&mut tx).await?.0;
        let actions = descriptors(&account, &frame.item, registered, pending);
        let activity_version = activity(&frame.item)?;
        tx.commit().await.map_err(storage_error)?;
        Ok(ProviderInboxActionsSnapshot {
            account_id: account.id,
            subject_id: query.subject_id,
            authorization_epoch: account.authorization_epoch,
            authorization_view: frame.view,
            activity_version,
            actions,
            revision,
        })
    }
    pub(crate) async fn queue_provider_inbox_action(
        &self,
        request: QueueProviderInboxActionRequest,
    ) -> Result<ProviderInboxActionReceipt> {
        request.validate()?;
        let mut writer = self.inner.writer.acquire().await?;
        let mut tx = writer.begin().await.map_err(storage_error)?;
        epoch_in(&mut tx, &request.account_id, &request.authorization_epoch).await?;
        let account = account_in(&mut tx, &request.account_id, true).await?;
        // Rebuild an exact existing sealed intent before mutable cache checks.
        // This preserves native UUID replay after a lost receipt or later sync.
        let prior:Option<StoredAdmission>=sqlx::query_as("SELECT operation_kind,payload_version,payload_bytes,repository_id,target_id FROM commands WHERE account_id=? AND command_id=?").bind(&account.id).bind(&request.command_id).fetch_optional(&mut *tx).await.map_err(storage_error)?;
        let (payload, repository) = if let Some((kind, version, bytes, repository, target)) = prior
        {
            let payload = Payload::parse(&bytes)?;
            if kind != KIND
                || version != 1
                || target != request.subject_id
                || !payload.matches_request(&request)
            {
                return Err(invalid());
            }
            (payload, repository)
        } else {
            let frame = frame_in(&mut tx, &account, &request.subject_id).await?;
            if frame.view != request.authorization_view
                || activity(&frame.item)? != request.expected_activity_version
            {
                return Err(stale());
            }
            let payload = payload_in(&mut tx, &account, &frame, request.action).await?;
            (payload, frame.item.repository_id)
        };
        let sealed = seal(&request, payload, repository)?;
        let receipt = super::command_admission::admit_in(&mut tx, &sealed, &Admission)
            .await
            .map_err(|e| match e {
                super::command_admission::CommandAdmissionError::Local(e) => e,
                _ => invalid(),
            })?;
        tx.commit().await.map_err(storage_error)?;
        Ok(ProviderInboxActionReceipt {
            account_id: receipt.account_id,
            command_id: receipt.command_id,
            revision: receipt.admitted_revision,
            duplicate: receipt.duplicate,
        })
    }
}
pub(crate) async fn payload_in(
    tx: &mut Transaction<'_, Sqlite>,
    account: &RemoteAccount,
    frame: &Frame,
    action: ProviderInboxAction,
) -> Result<Payload> {
    let pending = pending_in(tx, &account.id, &frame.item.id).await?;
    if !descriptors(account, &frame.item, true, pending)
        .iter()
        .any(|d| d.action == action && d.availability == ProviderInboxActionAvailability::Available)
    {
        return Err(CollaborationError::new(
            ErrorCode::Unsupported,
            "This provider inbox action is unavailable",
        ));
    }
    let (subject_type, native_action) = match &frame.item.native_inbox {
        Some(NativeInboxState::Notification { .. }) => (frame.item.state.clone(), String::new()),
        Some(NativeInboxState::Todo {
            action,
            target_type,
            ..
        }) => (target_type.clone(), action.clone()),
        None => return Err(invalid()),
    };
    let payload = Payload {
        instance: frame.instance.clone(),
        actor: account.actor_id.clone(),
        native_id: frame.item.provider_id.clone(),
        project: project_in(tx, &frame.item).await?,
        updated_at: frame.item.updated_at.clone(),
        activity: activity(&frame.item)?,
        view: frame.view.clone(),
        action,
        subject_type,
        native_action,
    };
    payload.validate()?;
    Ok(payload)
}
pub(crate) struct Admission;
#[async_trait::async_trait]
impl CommandAdmissionPolicy for Admission {
    const OPERATION_KIND: &'static str = KIND;
    const PAYLOAD_VERSION: u32 = 1;
    fn effect(&self, submission: &CommandSubmission) -> Result<Option<ItemIntentPatch>> {
        Ok(Some(Payload::parse(submission.payload_bytes())?.patch()))
    }
    async fn validate(
        &self,
        tx: &mut Transaction<'_, Sqlite>,
        account: &RemoteAccount,
        submission: &CommandSubmission,
    ) -> Result<Vec<CommandProtection>> {
        let payload = Payload::parse(submission.payload_bytes())?;
        let frame = frame_in(tx, account, submission.target().id()).await?;
        let expected = payload_in(tx, account, &frame, payload.action).await?;
        if payload != expected
            || submission.target().kind() != CommandTargetKind::Notification
            || submission.target().repository_id() != frame.item.repository_id.as_deref()
        {
            return Err(stale());
        }
        Ok(vec![])
    }
}
pub(crate) async fn current_claim(
    tx: &mut Transaction<'_, Sqlite>,
    account: &RemoteAccount,
    command: &crate::delivery::DeliveryCommand,
    payload: &Payload,
) -> Result<Frame> {
    let frame = frame_in(tx, account, &command.target_id).await?;
    if frame.view != payload.view
        || frame.instance != payload.instance
        || account.actor_id != payload.actor
        || activity(&frame.item)? != payload.activity
        || project_in(tx, &frame.item).await? != payload.project
        || frame.item.provider_id != payload.native_id
        || command.authorization_epoch != account.authorization_epoch
    {
        return Err(stale());
    }
    Ok(frame)
}
fn stale() -> CollaborationError {
    CollaborationError::new(
        ErrorCode::StaleView,
        "The notification changed; refresh before applying this action",
    )
}
#[cfg(test)]
mod tests;
