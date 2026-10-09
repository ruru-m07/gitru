//! Authored review drafts and exact durable GitHub review submission.
use super::{authorize, CollaborationState, Operation};
use collaboration::{
    CollaborationError, ReviewDraftKey, ReviewDraftPage, ReviewDraftQuery, ReviewDraftSnapshot,
    ReviewSubmissionReceipt, SaveReviewDraftRequest, SubmitReviewRequest, SubmittedReviewPage,
    SubmittedReviewQuery,
};
use tauri::{State, Webview};

/// Reads one local review draft and any current native submission authority.
#[tauri::command]
pub async fn collaboration_review_draft(
    key: ReviewDraftKey,
    view: Webview,
    state: State<'_, CollaborationState>,
) -> Result<ReviewDraftSnapshot, CollaborationError> {
    authorize(&view, Operation::ReviewSubmission)?;
    state.get().await?.review_draft(key).await
}

/// Lists authored review drafts even when their provider pull request is absent.
#[tauri::command]
pub async fn collaboration_review_drafts(
    query: ReviewDraftQuery,
    view: Webview,
    state: State<'_, CollaborationState>,
) -> Result<ReviewDraftPage, CollaborationError> {
    authorize(&view, Operation::ReviewSubmission)?;
    state.get().await?.review_drafts(query).await
}

/// Saves authored review content locally without contacting the provider.
#[tauri::command]
pub async fn collaboration_save_review_draft(
    request: SaveReviewDraftRequest,
    view: Webview,
    state: State<'_, CollaborationState>,
) -> Result<ReviewDraftSnapshot, CollaborationError> {
    authorize(&view, Operation::ReviewSubmission)?;
    state.get().await?.save_review_draft(request).await
}

/// Admits one exact saved review draft generation for background submission.
#[tauri::command]
pub async fn collaboration_submit_review(
    request: SubmitReviewRequest,
    view: Webview,
    state: State<'_, CollaborationState>,
) -> Result<ReviewSubmissionReceipt, CollaborationError> {
    authorize(&view, Operation::ReviewSubmission)?;
    state.get().await?.submit_review(request).await
}

/// Reads validated accepted or confirmed receipts without provider I/O.
#[tauri::command]
pub async fn collaboration_submitted_reviews(
    query: SubmittedReviewQuery,
    view: Webview,
    state: State<'_, CollaborationState>,
) -> Result<SubmittedReviewPage, CollaborationError> {
    authorize(&view, Operation::ReviewSubmission)?;
    state.get().await?.submitted_reviews(query).await
}
