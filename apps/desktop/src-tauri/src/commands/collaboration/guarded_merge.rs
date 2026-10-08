//! Online preview and explicit short-lived consent use the same native caller gate.
use super::{authorize, CollaborationState, Operation};
use collaboration::{
    CollaborationError, GuardedMergePreview, GuardedMergeQuery, GuardedMergeReceipt,
    GuardedMergeRequest, GuardedMergeSnapshot,
};
use tauri::{State, Webview};

#[tauri::command]
pub async fn collaboration_guarded_merge_snapshot(
    query: GuardedMergeQuery,
    view: Webview,
    state: State<'_, CollaborationState>,
) -> Result<GuardedMergeSnapshot, CollaborationError> {
    authorize(&view, Operation::GuardedMerge)?;
    state.get().await?.guarded_merge_snapshot(query).await
}
#[tauri::command]
pub async fn collaboration_preview_guarded_merge(
    query: GuardedMergeQuery,
    view: Webview,
    state: State<'_, CollaborationState>,
) -> Result<GuardedMergePreview, CollaborationError> {
    authorize(&view, Operation::GuardedMerge)?;
    state.get().await?.preview_guarded_merge(query).await
}
#[tauri::command]
pub async fn collaboration_submit_guarded_merge(
    request: GuardedMergeRequest,
    view: Webview,
    state: State<'_, CollaborationState>,
) -> Result<GuardedMergeReceipt, CollaborationError> {
    authorize(&view, Operation::GuardedMerge)?;
    state.get().await?.submit_guarded_merge(request).await
}
