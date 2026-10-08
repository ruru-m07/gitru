use super::*;

impl CollaborationRuntime {
    pub async fn label_set_snapshot(
        &self,
        account_id: &str,
        subject_id: &str,
    ) -> Result<LabelSetSnapshot, CollaborationError> {
        let _operation = self.acquire_operation()?;
        self.store.label_set_snapshot(account_id, subject_id).await
    }

    pub async fn submit_label_set(
        &self,
        request: LabelSetRequest,
    ) -> Result<LabelSetReceipt, CollaborationError> {
        crate::label_sets::native::validate_request(&request)?;
        self.owned_operation(move |runtime| async move {
            let _lifecycle = runtime.lifecycle.lock().await;
            let account = runtime.active_account(&request.context.account_id).await?;
            let instance = ProviderInstance::for_account(&account)?;
            if runtime
                .registry
                .delivery_policy(&instance, crate::label_sets::native::OPERATION, 1)
                .is_none()
            {
                return Err(CollaborationError::new(
                    ErrorCode::Unsupported,
                    "GitHub label delivery is unavailable",
                ));
            }
            let receipt = runtime.store.submit_label_set(request).await?;
            runtime.publish(receipt.admitted_revision.clone());
            runtime.notify.notify_one();
            Ok(receipt)
        })
        .await
    }
}
