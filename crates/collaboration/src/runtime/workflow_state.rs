use super::*;
impl CollaborationRuntime {
    pub async fn workflow_state_snapshot(
        &self,
        account_id: &str,
        subject_id: &str,
    ) -> Result<WorkflowStateSnapshot, CollaborationError> {
        let _operation = self.acquire_operation()?;
        self.store
            .workflow_state_snapshot(account_id, subject_id)
            .await
    }
    pub async fn submit_workflow_state(
        &self,
        request: WorkflowStateRequest,
    ) -> Result<WorkflowStateReceipt, CollaborationError> {
        crate::workflow_state::native::validate_request(&request)?;
        self.owned_operation(move |runtime| async move {
            let _lifecycle = runtime.lifecycle.lock().await;
            let account = runtime.active_account(&request.context.account_id).await?;
            let instance = ProviderInstance::for_account(&account)?;
            if runtime
                .registry
                .delivery_policy(&instance, crate::workflow_state::native::OPERATION, 1)
                .is_none()
            {
                return Err(CollaborationError::new(
                    ErrorCode::Unsupported,
                    "Workflow state delivery is unavailable",
                ));
            }
            let receipt = runtime.store.submit_workflow_state(request).await?;
            runtime.publish(receipt.admitted_revision.clone());
            runtime.notify.notify_one();
            Ok(receipt)
        })
        .await
    }
}
