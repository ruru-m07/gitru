use super::*;
use crate::comment_send::*;
impl CollaborationRuntime {
    pub async fn comment_draft(
        &self,
        account: &str,
        subject: &str,
    ) -> Result<CommentDraftSnapshot, CollaborationError> {
        let _guard = self.acquire_operation()?;
        self.store.comment_draft(account, subject).await
    }
    pub async fn save_comment_draft(
        &self,
        r: SaveCommentDraftRequest,
    ) -> Result<CommentDraftSnapshot, CollaborationError> {
        crate::storage::comment_send::validate_save(&r)?;
        self.owned_operation(move |runtime| async move {
            let _lifecycle = runtime.lifecycle.lock().await;
            let s = runtime.store.save_comment_draft(r).await?;
            runtime.publish(s.revision.clone());
            Ok(s)
        })
        .await
    }
    pub async fn send_comment(
        &self,
        r: SendCommentRequest,
    ) -> Result<CommentSubmissionReceipt, CollaborationError> {
        crate::comment_send::native::validate_send(&r)?;
        self.owned_operation(move |runtime| async move {
            let _lifecycle = runtime.lifecycle.lock().await;
            let account = runtime.active_account(&r.context.account_id).await?;
            let instance = ProviderInstance::for_account(&account)?;
            if runtime
                .registry
                .delivery_policy(&instance, crate::comment_send::native::OPERATION, 1)
                .is_none()
            {
                return Err(CollaborationError::new(
                    ErrorCode::Unsupported,
                    "Comment delivery is unavailable",
                ));
            }
            let receipt = runtime.store.send_comment(r).await?;
            runtime.publish(receipt.admitted_revision.clone());
            runtime.notify.notify_one();
            Ok(receipt)
        })
        .await
    }
    pub async fn created_comments(
        &self,
        q: CreatedCommentQuery,
    ) -> Result<CreatedCommentPage, CollaborationError> {
        let _guard = self.acquire_operation()?;
        self.store.created_comments(q).await
    }
}
impl CollaborationRuntime {
    pub async fn comment_drafts(
        &self,
        q: CommentDraftQuery,
    ) -> Result<CommentDraftPage, CollaborationError> {
        let _guard = self.acquire_operation()?;
        self.store.comment_drafts(q).await
    }
}
