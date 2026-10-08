//! Cache-only label review and durable best-effort label intent admission.
use super::{CollaborationState, Operation, authorize};
use collaboration::{
    CollaborationError, LabelSetReceipt, LabelSetRequest, LabelSetSnapshot,
};
use tauri::{State, Webview};

/// Reads only the native cache. Provider reconciliation remains background-owned.
#[tauri::command]
pub async fn collaboration_label_set_snapshot(
    account_id: String,
    subject_id: String,
    view: Webview,
    state: State<'_, CollaborationState>,
) -> Result<LabelSetSnapshot, CollaborationError> {
    authorize(&view, Operation::LabelSet)?;
    state
        .get()
        .await?
        .label_set_snapshot(&account_id, &subject_id)
        .await
}

/// Admits immutable local intent. Native policy owns every later provider write.
#[tauri::command]
pub async fn collaboration_submit_label_set(
    request: LabelSetRequest,
    view: Webview,
    state: State<'_, CollaborationState>,
) -> Result<LabelSetReceipt, CollaborationError> {
    authorize(&view, Operation::LabelSet)?;
    state.get().await?.submit_label_set(request).await
}
