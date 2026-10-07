//! Cache-only edit bases and durable title/body intent admission.
use super::{authorize, CollaborationState, Operation};
use collaboration::{CollaborationError, TextEditReceipt, TextEditRequest, TextEditSnapshot};
use tauri::{State, Webview};

/// Reads only the native cache. Provider reconciliation remains background-owned.
#[tauri::command]
pub async fn collaboration_text_edit_snapshot(
    account_id: String,
    subject_id: String,
    view: Webview,
    state: State<'_, CollaborationState>,
) -> Result<TextEditSnapshot, CollaborationError> {
    authorize(&view, Operation::TextEdit)?;
    state
        .get()
        .await?
        .text_edit_snapshot(&account_id, &subject_id)
        .await
}

/// Admits immutable local intent. Native policy owns any later provider write.
#[tauri::command]
pub async fn collaboration_submit_text_edit(
    request: TextEditRequest,
    view: Webview,
    state: State<'_, CollaborationState>,
) -> Result<TextEditReceipt, CollaborationError> {
    authorize(&view, Operation::TextEdit)?;
    state.get().await?.submit_text_edit(request).await
}
