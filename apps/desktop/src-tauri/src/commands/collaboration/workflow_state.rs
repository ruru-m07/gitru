//! Cache-only workflow state and durable close/reopen intent admission.
use super::{authorize, CollaborationState, Operation};
use collaboration::{
    CollaborationError, WorkflowStateReceipt, WorkflowStateRequest, WorkflowStateSnapshot,
};
use tauri::{State, Webview};

/// Reads only the native cache. Provider reconciliation remains background-owned.
#[tauri::command]
pub async fn collaboration_workflow_state_snapshot(
    account_id: String,
    subject_id: String,
    view: Webview,
    state: State<'_, CollaborationState>,
) -> Result<WorkflowStateSnapshot, CollaborationError> {
    authorize(&view, Operation::WorkflowState)?;
    state
        .get()
        .await?
        .workflow_state_snapshot(&account_id, &subject_id)
        .await
}

/// Admits immutable local intent. Native policy owns any later provider write.
#[tauri::command]
pub async fn collaboration_submit_workflow_state(
    request: WorkflowStateRequest,
    view: Webview,
    state: State<'_, CollaborationState>,
) -> Result<WorkflowStateReceipt, CollaborationError> {
    authorize(&view, Operation::WorkflowState)?;
    state.get().await?.submit_workflow_state(request).await
}
