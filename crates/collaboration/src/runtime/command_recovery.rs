//! Local user actions share the delivery lane so a held response commits before
//! a pause can change its generation. Completion remains runtime-owned.
use super::*;
use crate::command_recovery::policy::CommandRecoveryPolicy;

impl CollaborationRuntime {
    pub async fn command_recovery_list(
        &self,
        query: CommandRecoveryQuery,
    ) -> Result<CommandRecoverySnapshot, CollaborationError> {
        let _operation = self.acquire_operation()?;
        self.store.command_recovery_list(query).await
    }
    pub async fn command_recovery_detail(
        &self,
        account_id: &str,
        command_id: &str,
    ) -> Result<CommandRecoveryDetail, CollaborationError> {
        let _operation = self.acquire_operation()?;
        let policy = self.recovery_policy(account_id, command_id).await?;
        self.store
            .command_recovery_detail(account_id, command_id, policy.as_deref())
            .await
    }
    pub async fn command_recovery_action(
        &self,
        request: CommandRecoveryActionRequest,
    ) -> Result<CommandRecoveryReceipt, CollaborationError> {
        self.owned_operation(move |runtime| async move {
            let _lane = runtime.dispatch.lock().await;
            let _lifecycle = runtime.lifecycle.lock().await;
            let policy = runtime
                .recovery_policy(&request.context.account_id, &request.context.command_id)
                .await?;
            let receipt = runtime
                .store
                .command_recovery_action(request, policy.as_deref())
                .await?;
            runtime.publish(receipt.revision.clone());
            runtime.notify.notify_one();
            Ok(receipt)
        })
        .await
    }
    pub async fn command_recovery_replace(
        &self,
        request: CommandRecoveryReplaceRequest,
    ) -> Result<CommandRecoveryReceipt, CollaborationError> {
        self.owned_operation(move |runtime| async move {
            let _lane = runtime.dispatch.lock().await;
            let _lifecycle = runtime.lifecycle.lock().await;
            let policy = runtime
                .recovery_policy(&request.context.account_id, &request.context.command_id)
                .await?;
            let receipt = runtime
                .store
                .command_recovery_replace(request, policy.as_deref())
                .await?;
            runtime.publish(receipt.revision.clone());
            runtime.notify.notify_one();
            Ok(receipt)
        })
        .await
    }
    pub async fn command_recovery_export(
        &self,
        context: CommandRecoveryContext,
    ) -> Result<CommandRecoveryExport, CollaborationError> {
        let _operation = self.acquire_operation()?;
        let policy = self
            .recovery_policy(&context.account_id, &context.command_id)
            .await?;
        self.store
            .command_recovery_export(context, policy.as_deref())
            .await
    }
    async fn recovery_policy(
        &self,
        account_id: &str,
        command_id: &str,
    ) -> Result<Option<Arc<dyn CommandRecoveryPolicy>>, CollaborationError> {
        let account = self.store.account(account_id).await?;
        let command = self.store.delivery_command(account_id, command_id).await?;
        let instance = ProviderInstance::for_account(&account)?;
        Ok(self.registry.recovery_policy(
            &instance,
            &command.operation_kind,
            command.payload_version,
        ))
    }
}
