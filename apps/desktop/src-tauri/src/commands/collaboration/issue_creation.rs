//! Dedicated local issue drafts and durable GitHub issue creation admission.
use super::{authorize, CollaborationState, Operation};
use collaboration::{
    CollaborationError, IssueDraftKey, IssueDraftPage, IssueDraftQuery, IssueDraftSnapshot,
    IssueSubmissionReceipt, SaveIssueDraftRequest, SubmitIssueRequest,
};
use tauri::{State, Webview};

/// Reads one local issue draft and its current native submission context.
#[tauri::command]
pub async fn collaboration_issue_draft(
    key: IssueDraftKey,
    view: Webview,
    state: State<'_, CollaborationState>,
) -> Result<IssueDraftSnapshot, CollaborationError> {
    authorize(&view, Operation::IssueCreation)?;
    state.get().await?.issue_draft(key).await
}

/// Lists authored issue drafts even when their provider repository is absent.
#[tauri::command]
pub async fn collaboration_issue_drafts(
    query: IssueDraftQuery,
    view: Webview,
    state: State<'_, CollaborationState>,
) -> Result<IssueDraftPage, CollaborationError> {
    authorize(&view, Operation::IssueCreation)?;
    state.get().await?.issue_drafts(query).await
}

/// Saves authored title/body locally without contacting the provider.
#[tauri::command]
pub async fn collaboration_save_issue_draft(
    request: SaveIssueDraftRequest,
    view: Webview,
    state: State<'_, CollaborationState>,
) -> Result<IssueDraftSnapshot, CollaborationError> {
    authorize(&view, Operation::IssueCreation)?;
    state.get().await?.save_issue_draft(request).await
}

/// Admits one exact saved issue draft generation for background submission.
#[tauri::command]
pub async fn collaboration_submit_issue(
    request: SubmitIssueRequest,
    view: Webview,
    state: State<'_, CollaborationState>,
) -> Result<IssueSubmissionReceipt, CollaborationError> {
    authorize(&view, Operation::IssueCreation)?;
    state.get().await?.submit_issue(request).await
}
