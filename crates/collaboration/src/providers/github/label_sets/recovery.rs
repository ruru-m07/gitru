use super::*;
use crate::command_recovery::{policy::*, *};

#[async_trait]
impl CommandRecoveryPolicy for GithubLabelSetPolicy {
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
        let known =
            |labels: &[crate::LabelIdentity]| -> Result<CommandFieldValue, CollaborationError> {
                Ok(CommandFieldValue {
                    known: true,
                    value: Some(serde_json::to_string(labels).map_err(|_| invalid())?),
                })
            };
        let unknown = || CommandFieldValue {
            known: false,
            value: None,
        };
        let base = known(&payload.base.labels)?;
        let desired = known(&target_labels(&payload)?)?;
        let remote = frame
            .as_ref()
            .map(|frame| known(&frame.base.labels))
            .transpose()?
            .unwrap_or_else(unknown);
        Ok(NativeRecoveryReview {
            fields: vec![CommandFieldReview {
                field: CommandReviewField::Labels,
                comparison: compare_field(CommandReviewField::Labels, &base, &remote, &desired),
                base,
                remote,
                desired,
                editable: false,
            }],
            can_replace: false,
            reason: Some("GitHub label changes are name-addressed best-effort writes. Cancel this command and review the latest saved labels before creating a new request.".into()),
            fence: frame.as_ref().map(encode_bounded).transpose()?.unwrap_or_default(),
        })
    }

    async fn replace_in(
        &self,
        _: &mut Transaction<'_, Sqlite>,
        _: &DeliveryCommand,
        _: &RemoteAccount,
        _: &CommandRecoveryReplaceRequest,
        _: &NativeRecoveryReview,
    ) -> Result<crate::storage::command_admission::CommandReceipt, CollaborationError> {
        Err(CollaborationError::new(
            crate::ErrorCode::Unsupported,
            "Review the latest labels before creating a new request",
        ))
    }
}
