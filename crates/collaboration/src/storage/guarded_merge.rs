//! Local merge admission never constructs online authority from stored bytes.
use super::*;
use crate::commands::*;
use crate::guarded_merge::{native::*, *};
use crate::storage::command_admission::{self, CommandAdmissionPolicy, CommandProtection};
use crate::workflow_state::native::NativeFrame;

pub(crate) struct Admission;
#[async_trait::async_trait]
impl CommandAdmissionPolicy for Admission {
    const OPERATION_KIND: &'static str = OPERATION;
    const PAYLOAD_VERSION: u32 = 1;
    async fn validate(
        &self,
        tx: &mut Transaction<'_, Sqlite>,
        account: &RemoteAccount,
        submission: &CommandSubmission,
    ) -> super::Result<Vec<CommandProtection>> {
        let p = decode_submission(submission)?;
        let frame = capture_in(tx, account, submission.target().id()).await?;
        if account.actor_id != p.actor_id
            || !matches_frame(&p, &frame)
            || frame.authorization_view != p.request.context.authorization_view
            || frame.base.head.as_deref() != Some(&p.request.context.expected_head)
            || frame.base.state != "open"
            || pending_in(tx, &account.id, &frame.subject.id).await?
        {
            return Err(stale());
        }
        Ok(vec![CommandProtection::Facet {
            subject_id: frame.subject.id,
            facet: crate::DetailFacet::Body,
        }])
    }
}
pub(crate) fn matches_frame(p: &Payload, f: &NativeFrame) -> bool {
    f.subject.kind == RemoteItemKind::PullRequest
        && f.subject.id == p.request.context.subject_id
        && f.subject.account_id == p.request.context.account_id
        && f.repository.account_id == p.request.context.account_id
        && f.repository.provider_id == p.repository_native_id
        && f.subject.provider_id == p.subject_native_id
        && f.subject.number.as_deref() == Some(p.number.as_str())
        && f.base.source == "github/pull-detail/2026-03-10"
}
pub(crate) async fn capture_in(
    tx: &mut Transaction<'_, Sqlite>,
    account: &RemoteAccount,
    subject: &str,
) -> super::Result<NativeFrame> {
    let f = workflow_state::capture_in(tx, account, subject).await?;
    if f.subject.kind != RemoteItemKind::PullRequest
        || !native_id(&f.repository.provider_id)
        || !native_id(&f.subject.provider_id)
        || f.subject.number.as_deref().is_none_or(|n| !native_id(n))
        || f.base.head.as_deref().is_none_or(|h| !valid_oid(h))
    {
        return Err(stale());
    }
    Ok(f)
}
pub(crate) async fn pending_in(
    tx: &mut Transaction<'_, Sqlite>,
    account: &str,
    subject: &str,
) -> super::Result<bool> {
    sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM commands WHERE account_id=? AND target_id=? AND state IN ('queued','sending','retry_wait','accepted','outcome_unknown'))")
        .bind(account).bind(subject).fetch_one(&mut **tx).await.map_err(storage_error)
}
pub(crate) fn seal(p: Payload, repository: &str) -> super::Result<CommandSubmission> {
    seal_command(CommandDraft {
        command_id: p.request.command_id.clone(),
        account_id: p.request.context.account_id.clone(),
        authorization_epoch: p.request.context.authorization_epoch.clone(),
        target: CommandTarget::new(
            CommandTargetKind::PullRequest,
            p.request.context.subject_id.clone(),
            Some(repository.into()),
        )?,
        payload: p,
        guards: vec![],
        dependencies: vec![],
    })
}
impl Store {
    pub async fn guarded_merge_snapshot(
        &self,
        q: GuardedMergeQuery,
    ) -> super::Result<GuardedMergeSnapshot> {
        validate_query(&q)?;
        let mut tx = self.inner.readers.begin().await.map_err(storage_error)?;
        let account = account_in(&mut tx, &q.account_id, false).await?;
        let (revision, authorization_view) = metadata(&mut tx).await?;
        let mut result = GuardedMergeSnapshot {
            reason: None,
            latest: None,
            revision,
            authorization_view,
        };
        result.reason = if account.provider != ProviderKind::Github || account.host != "github.com"
        {
            Some(MergeUnavailableReason::UnsupportedProvider)
        } else if account.state != AccountState::Active {
            Some(MergeUnavailableReason::AccountUnavailable)
        } else if pending_in(&mut tx, &account.id, &q.subject_id).await? {
            Some(MergeUnavailableReason::PendingCommand)
        } else {
            None
        };
        if matches!(
            result.reason,
            None | Some(MergeUnavailableReason::PendingCommand)
        ) {
            match capture_in(&mut tx, &account, &q.subject_id).await {
                Ok(_) => {}
                Err(e)
                    if matches!(
                        e.code,
                        ErrorCode::NotFound
                            | ErrorCode::StaleView
                            | ErrorCode::PermissionDenied
                            | ErrorCode::InvalidInput
                    ) =>
                {
                    result.reason = Some(MergeUnavailableReason::MissingContext)
                }
                Err(e) => return Err(e),
            }
        }
        // No command details cross a disconnected/denied authorization boundary.
        if matches!(
            result.reason,
            None | Some(MergeUnavailableReason::PendingCommand)
        ) {
            let id:Option<String>=sqlx::query_scalar("SELECT command_id FROM commands WHERE account_id=? AND target_id=? AND operation_kind=? ORDER BY enqueue_order DESC LIMIT 1")
                .bind(&q.account_id).bind(&q.subject_id).bind(OPERATION).fetch_optional(&mut *tx).await.map_err(storage_error)?;
            if let Some(id) = id {
                let c = delivery::load_in(&mut tx, &q.account_id, &id).await?;
                let p = decode_payload(&c)?;
                result.latest = Some(GuardedMergeStatus {
                    command_id: id,
                    state: c.state.name().into(),
                    method: p.request.method,
                    expected_head: p.request.context.expected_head,
                    attempt_count: c.attempt_count.try_into().map_err(|_| invalid())?,
                    attention: c.attention,
                });
            }
        }
        tx.commit().await.map_err(storage_error)?;
        Ok(result)
    }
    pub(crate) async fn guarded_merge_frame(
        &self,
        q: &GuardedMergeQuery,
    ) -> super::Result<(RemoteAccount, NativeFrame)> {
        validate_query(q)?;
        let mut tx = self.inner.readers.begin().await.map_err(storage_error)?;
        let account = account_in(&mut tx, &q.account_id, true).await?;
        if pending_in(&mut tx, &account.id, &q.subject_id).await? {
            return Err(CollaborationError::new(
                ErrorCode::Busy,
                "Resolve pending commands before merging",
            ));
        }
        let frame = capture_in(&mut tx, &account, &q.subject_id).await?;
        tx.commit().await.map_err(storage_error)?;
        Ok((account, frame))
    }
    pub(crate) async fn validate_merge_frame(
        &self,
        account: &RemoteAccount,
        frame: &NativeFrame,
    ) -> super::Result<()> {
        let mut writer = self.inner.writer.acquire().await?;
        let mut tx = writer.begin().await.map_err(storage_error)?;
        workflow_state::validate_frame_in(&mut tx, account, frame).await?;
        tx.commit().await.map_err(storage_error)
    }
    pub(crate) async fn submit_guarded_merge<F>(
        &self,
        request: GuardedMergeRequest,
        grant: Option<&NativeFrame>,
        validate_online: F,
    ) -> super::Result<GuardedMergeReceipt>
    where
        F: Fn() -> super::Result<()> + Send,
    {
        validate_request(&request)?;
        let mut writer = self.inner.writer.acquire().await?;
        let mut tx = writer.begin().await.map_err(storage_error)?;
        epoch_in(
            &mut tx,
            &request.context.account_id,
            &request.context.authorization_epoch,
        )
        .await?;
        let account = account_in(&mut tx, &request.context.account_id, true).await?;
        let (_, view) = metadata(&mut tx).await?;
        if view != request.context.authorization_view {
            return Err(stale());
        }
        let old: Option<String> = sqlx::query_scalar(
            "SELECT command_id FROM commands WHERE account_id=? AND command_id=?",
        )
        .bind(&account.id)
        .bind(&request.command_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(storage_error)?;
        let duplicate = old.is_some();
        let submission = if let Some(id) = old {
            let command = delivery::load_in(&mut tx, &account.id, &id).await?;
            if command.operation_kind != OPERATION || command.payload_version != 1 {
                return Err(invalid());
            }
            let p = decode_payload(&command)?;
            if p.request != request {
                return Err(invalid());
            }
            seal(p, command.repository_id.as_deref().ok_or_else(invalid)?)?
        } else {
            let frame = grant.ok_or_else(|| {
                CollaborationError::new(
                    ErrorCode::NotReady,
                    "Merge preview expired; check online again",
                )
            })?;
            workflow_state::validate_frame_in(&mut tx, &account, frame).await?;
            seal(
                Payload {
                    request,
                    actor_id: account.actor_id.clone(),
                    repository_native_id: frame.repository.provider_id.clone(),
                    subject_native_id: frame.subject.provider_id.clone(),
                    number: frame.subject.number.clone().ok_or_else(invalid)?,
                },
                &frame.repository.id,
            )?
        };
        if !duplicate {
            validate_online()?;
        }
        let receipt = command_admission::admit_in(&mut tx, &submission, &Admission)
            .await
            .map_err(workflow_state::admission_error)?;
        if !duplicate {
            validate_online()?;
        }
        tx.commit().await.map_err(storage_error)?;
        Ok(GuardedMergeReceipt {
            account_id: receipt.account_id,
            command_id: receipt.command_id,
            admitted_revision: receipt.admitted_revision,
            duplicate: receipt.duplicate,
        })
    }
}
pub(crate) async fn prepare_in(
    tx: &mut Transaction<'_, Sqlite>,
    c: &crate::delivery::DeliveryCommand,
    a: &RemoteAccount,
) -> super::Result<Vec<u8>> {
    let mut f = capture_in(tx, a, &c.target_id).await?;
    let p = decode_payload(c)?;
    if !matches_frame(&p, &f) {
        return Err(stale());
    }
    f.run_id = Uuid::new_v4().to_string();
    sqlx::query(
        "UPDATE sync_scopes SET run_id=?,next_cursor=NULL,etag=NULL WHERE account_id=? AND scope=?",
    )
    .bind(&f.run_id)
    .bind(&a.id)
    .bind(crate::DetailFacet::Body.scope(&c.target_id))
    .execute(&mut **tx)
    .await
    .map_err(storage_error)?;
    crate::guarded_merge::native::encode(&f)
}
