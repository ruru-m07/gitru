//! Explicit provider inbox intent. The renderer never supplies provider routes.
use super::{authorize, CollaborationState, Operation};
use collaboration::{
    CollaborationError, ProviderInboxActionReceipt, ProviderInboxActionsQuery,
    ProviderInboxActionsSnapshot, QueueProviderInboxActionRequest,
};
use tauri::{State, Webview};

#[tauri::command]
pub async fn collaboration_provider_inbox_actions(
    query: ProviderInboxActionsQuery,
    view: Webview,
    state: State<'_, CollaborationState>,
) -> Result<ProviderInboxActionsSnapshot, CollaborationError> {
    authorize(&view, Operation::ProviderInboxAction)?;
    state.get().await?.provider_inbox_actions(query).await
}

#[tauri::command]
pub async fn collaboration_queue_provider_inbox_action(
    request: QueueProviderInboxActionRequest,
    view: Webview,
    state: State<'_, CollaborationState>,
) -> Result<ProviderInboxActionReceipt, CollaborationError> {
    authorize(&view, Operation::ProviderInboxAction)?;
    state
        .get()
        .await?
        .queue_provider_inbox_action(request)
        .await
}
