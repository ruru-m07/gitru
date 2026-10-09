use super::*;
use crate::command_recovery::{policy::*, *};
use crate::storage::command_admission::{self, CommandReceipt};
use crate::text_edits::TextEditRequest;
#[async_trait]
impl CommandRecoveryPolicy for GithubTextEditPolicy {
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
            Ok(frame) => Some(frame),
            Err(error)
                if matches!(
                    error.code,
                    crate::ErrorCode::NotFound
                        | crate::ErrorCode::StaleView
                        | crate::ErrorCode::PermissionDenied
                        | crate::ErrorCode::InvalidInput
                ) =>
            {
                None
            }
            Err(error) => return Err(error),
        };
        let mut fields = vec![];
        let value = |s: Option<String>| CommandFieldValue {
            known: true,
            value: s,
        };
        let unknown = || CommandFieldValue {
            known: false,
            value: None,
        };
        let mut push = |field, base, desired, remote, editable| {
            let comparison = compare_field(field, &base, &remote, &desired);
            fields.push(CommandFieldReview {
                field,
                base,
                remote,
                desired,
                comparison,
                editable,
            });
        };
        if let Some(title) = &payload.request.title {
            push(
                CommandReviewField::Title,
                value(Some(payload.base.title.clone())),
                value(Some(title.clone())),
                frame
                    .as_ref()
                    .map(|f| value(Some(f.base.title.clone())))
                    .unwrap_or_else(unknown),
                true,
            );
        }
        if let Some(body) = &payload.request.body {
            push(
                CommandReviewField::Body,
                value(Some(payload.base.body.clone().unwrap_or_default())),
                value(Some(body.clone())),
                frame
                    .as_ref()
                    .map(|f| value(Some(f.base.body.clone().unwrap_or_default())))
                    .unwrap_or_else(unknown),
                true,
            );
        }
        push(
            CommandReviewField::State,
            value(Some(payload.base.state.clone())),
            value(Some(payload.base.state.clone())),
            frame
                .as_ref()
                .map(|f| value(Some(f.base.state.clone())))
                .unwrap_or_else(unknown),
            false,
        );
        if payload.base.head.is_some() {
            push(
                CommandReviewField::Head,
                value(payload.base.head.clone()),
                value(payload.base.head.clone()),
                frame
                    .as_ref()
                    .map(|f| value(f.base.head.clone()))
                    .unwrap_or_else(unknown),
                false,
            );
        }
        Ok(NativeRecoveryReview{fields,can_replace:frame.is_some(),reason:Some(if frame.is_some(){"GitHub title/body updates are best effort. Confirmed means the desired fields were observed, not proof of which actor caused them."}else{"Refresh the authorized resource Body before reviewing this edit."}.into()),fence:frame.as_ref().map(encode_bounded).transpose()?.unwrap_or_default()})
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
        let mut title = None;
        let mut body = None;
        for resolution in &request.fields {
            let next = match resolution.field {
                CommandReviewField::Title if original.request.title.is_some() => {
                    match resolution.choice {
                        CommandResolutionChoice::KeepDesired => original.request.title.clone(),
                        CommandResolutionChoice::UseRemote => None,
                        CommandResolutionChoice::Edited => {
                            Some(resolution.value.clone().ok_or_else(invalid)?)
                        }
                    }
                }
                CommandReviewField::Body if original.request.body.is_some() => {
                    match resolution.choice {
                        CommandResolutionChoice::KeepDesired => original.request.body.clone(),
                        CommandResolutionChoice::UseRemote => None,
                        CommandResolutionChoice::Edited => {
                            Some(resolution.value.clone().unwrap_or_default())
                        }
                    }
                }
                _ => return Err(invalid()),
            };
            if resolution.field == CommandReviewField::Title {
                title = next.filter(|v| v != &frame.base.title);
            } else {
                body = next.filter(|v| !body_equal(Some(v), frame.base.body.as_deref()));
            }
        }
        let request = TextEditRequest {
            context: store::context(account, &frame)?,
            command_id: request.new_command_id.clone(),
            accept_best_effort: true,
            title,
            body,
        };
        let submission = store::seal(
            Payload {
                request,
                base: frame.base,
            },
            &frame.repository.id,
        )?;
        command_admission::admit_in(tx, &submission, &store::Admission)
            .await
            .map_err(store::admission_error)
    }
}
