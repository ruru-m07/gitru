use super::*;
use crate::issue_creation::*;
impl CollaborationRuntime {
    pub async fn issue_draft(
        &self,
        key: IssueDraftKey,
    ) -> Result<IssueDraftSnapshot, CollaborationError> {
        let _guard = self.acquire_operation()?;
        self.store.issue_draft(key).await
    }
    pub async fn save_issue_draft(
        &self,
        r: SaveIssueDraftRequest,
    ) -> Result<IssueDraftSnapshot, CollaborationError> {
        crate::storage::issue_creation::validate_save(&r)?;
        self.owned_operation(move |runtime| async move {
            let _lifecycle = runtime.lifecycle.lock().await;
            let s = runtime.store.save_issue_draft(r).await?;
            runtime.publish(s.revision.clone());
            Ok(s)
        })
        .await
    }
    pub async fn submit_issue(
        &self,
        r: SubmitIssueRequest,
    ) -> Result<IssueSubmissionReceipt, CollaborationError> {
        crate::issue_creation::native::validate_send(&r)?;
        self.owned_operation(move |runtime| async move {
            let _lifecycle = runtime.lifecycle.lock().await;
            let account = runtime.active_account(&r.context.account_id).await?;
            let instance = ProviderInstance::for_account(&account)?;
            if runtime
                .registry
                .delivery_policy(&instance, crate::issue_creation::native::OPERATION, 1)
                .is_none()
            {
                return Err(CollaborationError::new(
                    ErrorCode::Unsupported,
                    "Issue creation is unavailable",
                ));
            }
            let receipt = runtime.store.submit_issue(r).await?;
            runtime.publish(receipt.admitted_revision.clone());
            runtime.notify.notify_one();
            Ok(receipt)
        })
        .await
    }
}
impl CollaborationRuntime {
    pub async fn issue_drafts(
        &self,
        q: IssueDraftQuery,
    ) -> Result<IssueDraftPage, CollaborationError> {
        let _guard = self.acquire_operation()?;
        self.store.issue_drafts(q).await
    }
}
