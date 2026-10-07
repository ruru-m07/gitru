use super::*;
impl CollaborationRuntime {
    pub async fn text_edit_snapshot(
        &self,
        account_id: &str,
        subject_id: &str,
    ) -> Result<TextEditSnapshot, CollaborationError> {
        let _operation = self.acquire_operation()?;
        self.store.text_edit_snapshot(account_id, subject_id).await
    }
    pub async fn submit_text_edit(
        &self,
        request: TextEditRequest,
    ) -> Result<TextEditReceipt, CollaborationError> {
        self.owned_operation(move |runtime| async move {
            let _lifecycle = runtime.lifecycle.lock().await;
            let account = runtime.active_account(&request.context.account_id).await?;
            let instance = ProviderInstance::for_account(&account)?;
            if runtime
                .registry
                .delivery_policy(&instance, crate::text_edits::native::OPERATION, 1)
                .is_none()
            {
                return Err(CollaborationError::new(
                    ErrorCode::Unsupported,
                    "Text edit delivery is unavailable",
                ));
            }
            let receipt = runtime.store.submit_text_edit(request).await?;
            runtime.publish(receipt.admitted_revision.clone());
            runtime.notify.notify_one();
            Ok(receipt)
        })
        .await
    }
}
