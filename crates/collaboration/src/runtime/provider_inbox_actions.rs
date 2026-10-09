use super::*;
impl CollaborationRuntime {
    pub async fn provider_inbox_actions(
        &self,
        query: ProviderInboxActionsQuery,
    ) -> Result<ProviderInboxActionsSnapshot, CollaborationError> {
        query.validate()?;
        let _operation = self.acquire_operation()?;
        let account = self.store.account(&query.account_id).await?;
        let registered = self
            .registry
            .delivery_policy(
                &ProviderInstance::for_account(&account)?,
                crate::provider_inbox_actions::native::KIND,
                1,
            )
            .is_some();
        self.store.provider_inbox_actions(query, registered).await
    }
    pub async fn queue_provider_inbox_action(
        &self,
        request: QueueProviderInboxActionRequest,
    ) -> Result<ProviderInboxActionReceipt, CollaborationError> {
        request.validate()?;
        self.owned_operation(move |runtime| async move {
            let _lifecycle = runtime.lifecycle.lock().await;
            let account = runtime.store.account(&request.account_id).await?;
            if runtime
                .registry
                .delivery_policy(
                    &ProviderInstance::for_account(&account)?,
                    crate::provider_inbox_actions::native::KIND,
                    1,
                )
                .is_none()
            {
                return Err(CollaborationError::new(
                    ErrorCode::Unsupported,
                    "Provider inbox action is not installed",
                ));
            }
            let receipt = runtime.store.queue_provider_inbox_action(request).await?;
            runtime.publish(receipt.revision.clone());
            runtime.notify.notify_one();
            Ok(receipt)
        })
        .await
    }
}
