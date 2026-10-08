use super::*;
use crate::command_recovery::{policy::*, *};
use crate::guarded_merge::native::encode;
#[async_trait]
impl CommandRecoveryPolicy for GithubGuardedMergePolicy {
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
        c: &DeliveryCommand,
        a: &RemoteAccount,
    ) -> Result<NativeRecoveryReview, CollaborationError> {
        let p = decode_payload(c)?;
        let frame = match store::capture_in(tx, a, &c.target_id).await {
            Ok(frame) => Some(frame),
            Err(error)
                if matches!(
                    error.code,
                    ErrorCode::NotFound
                        | ErrorCode::StaleView
                        | ErrorCode::PermissionDenied
                        | ErrorCode::InvalidInput
                ) =>
            {
                None
            }
            Err(error) => return Err(error),
        };
        let base = CommandFieldValue {
            known: true,
            value: Some(p.request.context.expected_head),
        };
        let remote = CommandFieldValue {
            known: frame.is_some(),
            value: frame.as_ref().and_then(|f| f.base.head.clone()),
        };
        let desired = base.clone();
        Ok(NativeRecoveryReview{fields:vec![CommandFieldReview{field:CommandReviewField::Head,comparison:compare_field(CommandReviewField::Head,&base,&remote,&desired),base,remote,desired,editable:false}],can_replace:false,
            reason:Some("Merge requires a new online preview and explicit head/method consent. Accepted or uncertain attempts are read-only reconciled and cannot be resent.".into()),fence:frame.as_ref().map(encode).transpose()?.unwrap_or_default()})
    }
    async fn replace_in(
        &self,
        _tx: &mut Transaction<'_, Sqlite>,
        _c: &DeliveryCommand,
        _a: &RemoteAccount,
        _r: &CommandRecoveryReplaceRequest,
        _review: &NativeRecoveryReview,
    ) -> Result<crate::storage::command_admission::CommandReceipt, CollaborationError> {
        Err(CollaborationError::new(
            ErrorCode::Unsupported,
            "Merges cannot be replaced through offline recovery",
        ))
    }
}
