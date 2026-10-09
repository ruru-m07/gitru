//! Local metadata drafts and scheduled native repository catalogs.
use super::{authorize, CollaborationState, Operation};
use collaboration::*;
use tauri::{State, Webview};
#[tauri::command]
pub async fn collaboration_issue_draft_v2(
    key: IssueDraftKey,
    view: Webview,
    state: State<'_, CollaborationState>,
) -> Result<IssueDraftV2Snapshot, CollaborationError> {
    authorize(&view, Operation::IssueCreation)?;
    state.get().await?.issue_draft_v2(key).await
}
#[tauri::command]
pub async fn collaboration_issue_drafts_v2(
    query: IssueDraftQuery,
    view: Webview,
    state: State<'_, CollaborationState>,
) -> Result<IssueDraftV2Page, CollaborationError> {
    authorize(&view, Operation::IssueCreation)?;
    state.get().await?.issue_drafts_v2(query).await
}
#[tauri::command]
pub async fn collaboration_save_issue_draft_v2(
    request: SaveIssueDraftV2Request,
    view: Webview,
    state: State<'_, CollaborationState>,
) -> Result<IssueDraftV2Snapshot, CollaborationError> {
    authorize(&view, Operation::IssueCreation)?;
    state.get().await?.save_issue_draft_v2(request).await
}
#[tauri::command]
pub async fn collaboration_submit_issue_v2(
    request: SubmitIssueV2Request,
    view: Webview,
    state: State<'_, CollaborationState>,
) -> Result<IssueSubmissionReceipt, CollaborationError> {
    authorize(&view, Operation::IssueCreation)?;
    state.get().await?.submit_issue_v2(request).await
}
#[tauri::command]
pub async fn collaboration_issue_metadata_options(
    query: IssueMetadataQuery,
    view: Webview,
    state: State<'_, CollaborationState>,
) -> Result<IssueMetadataPage, CollaborationError> {
    authorize(&view, Operation::IssueCreation)?;
    state.get().await?.issue_metadata_options(query).await
}
#[tauri::command]
pub async fn collaboration_refresh_issue_metadata(
    request: RefreshIssueMetadataRequest,
    view: Webview,
    state: State<'_, CollaborationState>,
) -> Result<RefreshReceipt, CollaborationError> {
    authorize(&view, Operation::IssueCreation)?;
    state.get().await?.refresh_issue_metadata(request).await
}
