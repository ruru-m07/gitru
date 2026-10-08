use super::*;
use crate::review_submission::*;

impl CollaborationRuntime {
    pub async fn review_draft(
        &self,
        key: ReviewDraftKey,
    ) -> Result<ReviewDraftSnapshot, CollaborationError> {
        let _guard = self.acquire_operation()?;
        self.store.review_draft(key).await
    }

    pub async fn review_drafts(
        &self,
        query: ReviewDraftQuery,
    ) -> Result<ReviewDraftPage, CollaborationError> {
        let _guard = self.acquire_operation()?;
        self.store.review_drafts(query).await
    }

    pub async fn save_review_draft(
        &self,
        request: SaveReviewDraftRequest,
    ) -> Result<ReviewDraftSnapshot, CollaborationError> {
        crate::storage::review_submission::validate_save(&request)?;
        self.owned_operation(move |runtime| async move {
            let _lifecycle = runtime.lifecycle.lock().await;
            let snapshot = runtime.store.save_review_draft(request).await?;
            runtime.publish(snapshot.revision.clone());
            Ok(snapshot)
        })
        .await
    }

    pub async fn submit_review(
        &self,
        request: SubmitReviewRequest,
    ) -> Result<ReviewSubmissionReceipt, CollaborationError> {
        crate::review_submission::native::validate_submit(&request)?;
        self.owned_operation(move |runtime| async move {
            let _lifecycle = runtime.lifecycle.lock().await;
            let account = runtime.active_account(&request.context.account_id).await?;
            let instance = ProviderInstance::for_account(&account)?;
            if runtime
                .registry
                .delivery_policy(&instance, crate::review_submission::native::OPERATION, 1)
                .is_none()
            {
                return Err(CollaborationError::new(
                    ErrorCode::Unsupported,
                    "Review submission is unavailable",
                ));
            }
            let receipt = runtime.store.submit_review(request).await?;
            runtime.publish(receipt.admitted_revision.clone());
            runtime.notify.notify_one();
            Ok(receipt)
        })
        .await
    }

    pub async fn submitted_reviews(
        &self,
        query: SubmittedReviewQuery,
    ) -> Result<SubmittedReviewPage, CollaborationError> {
        let _guard = self.acquire_operation()?;
        self.store.submitted_reviews(query).await
    }
}
