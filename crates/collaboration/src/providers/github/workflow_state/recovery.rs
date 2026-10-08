use super::*;
use crate::command_recovery::{policy::*, *};
use crate::storage::command_admission::{self, CommandReceipt};
use crate::{WorkflowState, WorkflowStateRequest};
#[async_trait]
impl CommandRecoveryPolicy for GithubWorkflowStatePolicy {
    fn instance_id(&self) -> &str {
        "github:https://github.com/"
    }
    fn operation_kind(&self) -> &'static str {
        OPERATION
    }
    fn payload_version(&self) -> u32 {
        1
    }
    async fn review_in(
        &self,
        tx: &mut Transaction<'_, Sqlite>,
        command: &DeliveryCommand,
        account: &RemoteAccount,
    ) -> Result<NativeRecoveryReview, CollaborationError> {
        let payload = decode_payload(command)?;
        let frame = match store::capture_in(tx, account, &command.target_id).await {
            Ok(f) => Some(f),
            Err(e)
                if matches!(
                    e.code,
                    crate::ErrorCode::NotFound
                        | crate::ErrorCode::StaleView
                        | crate::ErrorCode::PermissionDenied
                        | crate::ErrorCode::InvalidInput
                ) =>
            {
                None
            }
            Err(e) => return Err(e),
        };
        let known = |s: Option<String>| CommandFieldValue {
            known: true,
            value: s,
        };
        let unknown = || CommandFieldValue {
            known: false,
            value: None,
        };
        let base = known(Some(payload.base.state.clone()));
        let desired = known(Some(payload.request.desired_state.as_str().into()));
        let remote = frame
            .as_ref()
            .map(|f| known(Some(f.base.state.clone())))
            .unwrap_or_else(unknown);
        let mut fields = vec![CommandFieldReview {
            field: CommandReviewField::State,
            comparison: compare_field(CommandReviewField::State, &base, &remote, &desired),
            base,
            remote,
            desired,
            editable: true,
        }];
        if payload.base.head.is_some() {
            let base = known(payload.base.head.clone());
            let desired = base.clone();
            let remote = frame
                .as_ref()
                .map(|f| known(f.base.head.clone()))
                .unwrap_or_else(unknown);
            fields.push(CommandFieldReview {
                field: CommandReviewField::Head,
                comparison: compare_field(CommandReviewField::Head, &base, &remote, &desired),
                base,
                remote,
                desired,
                editable: false,
            });
        }
        Ok(NativeRecoveryReview{fields,can_replace:frame.as_ref().is_some_and(|f|WorkflowState::parse(&f.base.state).is_some()),reason:Some("GitHub close/reopen is best effort. Confirmation proves observed state, not which actor caused it; merged or unknown workflows cannot be replaced.".into()),fence:frame.as_ref().map(encode_bounded).transpose()?.unwrap_or_default()})
    }
    async fn replace_in(
        &self,
        tx: &mut Transaction<'_, Sqlite>,
        command: &DeliveryCommand,
        account: &RemoteAccount,
        request: &CommandRecoveryReplaceRequest,
        _review: &NativeRecoveryReview,
    ) -> Result<CommandReceipt, CollaborationError> {
        let original = decode_payload(command)?;
        let frame = store::capture_in(tx, account, &command.target_id).await?;
        if request.fields.len() != 1 || request.fields[0].field != CommandReviewField::State {
            return Err(invalid());
        }
        let resolution = &request.fields[0];
        let desired_state = match resolution.choice {
            CommandResolutionChoice::KeepDesired => original.request.desired_state,
            CommandResolutionChoice::UseRemote => {
                WorkflowState::parse(&frame.base.state).ok_or_else(invalid)?
            }
            CommandResolutionChoice::Edited => {
                WorkflowState::parse(resolution.value.as_deref().ok_or_else(invalid)?)
                    .ok_or_else(invalid)?
            }
        };
        let submission = store::seal(
            Payload {
                request: WorkflowStateRequest {
                    context: store::context(account, &frame)?,
                    command_id: request.new_command_id.clone(),
                    accept_best_effort: true,
                    desired_state,
                },
                base: frame.base,
            },
            &frame.repository.id,
        )?;
        command_admission::admit_in(tx, &submission, &store::Admission)
            .await
            .map_err(store::admission_error)
    }
}
