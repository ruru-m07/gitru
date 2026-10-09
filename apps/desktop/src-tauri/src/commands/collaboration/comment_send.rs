//! Dedicated local comment drafts, durable send admission and creation receipts.
use super::{authorize, CollaborationState, Operation};
use collaboration::{
    CollaborationError, CommentDraftPage, CommentDraftQuery, CommentDraftSnapshot,
    CommentSubmissionReceipt, CreatedCommentPage, CreatedCommentQuery, SaveCommentDraftRequest,
    SendCommentRequest,
};
use tauri::{State, Webview};

/// Reads only the dedicated local comment draft and native send context.
#[tauri::command]
pub async fn collaboration_comment_draft(
    account_id: String,
    subject_id: String,
    view: Webview,
    state: State<'_, CollaborationState>,
) -> Result<CommentDraftSnapshot, CollaborationError> {
    authorize(&view, Operation::CommentSend)?;
    state
        .get()
        .await?
        .comment_draft(&account_id, &subject_id)
        .await
}

/// Lists dedicated comment drafts even when their remote subjects are absent.
#[tauri::command]
pub async fn collaboration_comment_drafts(
    query: CommentDraftQuery,
    view: Webview,
    state: State<'_, CollaborationState>,
) -> Result<CommentDraftPage, CollaborationError> {
    authorize(&view, Operation::CommentSend)?;
    state.get().await?.comment_drafts(query).await
}

/// Saves only the dedicated comment draft. Private notes are a separate store.
#[tauri::command]
pub async fn collaboration_save_comment_draft(
    request: SaveCommentDraftRequest,
    view: Webview,
    state: State<'_, CollaborationState>,
) -> Result<CommentDraftSnapshot, CollaborationError> {
    authorize(&view, Operation::CommentSend)?;
    state.get().await?.save_comment_draft(request).await
}

/// Admits one exact saved draft generation for background delivery.
#[tauri::command]
pub async fn collaboration_send_comment(
    request: SendCommentRequest,
    view: Webview,
    state: State<'_, CollaborationState>,
) -> Result<CommentSubmissionReceipt, CollaborationError> {
    authorize(&view, Operation::CommentSend)?;
    state.get().await?.send_comment(request).await
}

/// Reads validated local creation receipts without provider I/O.
#[tauri::command]
pub async fn collaboration_created_comments(
    query: CreatedCommentQuery,
    view: Webview,
    state: State<'_, CollaborationState>,
) -> Result<CreatedCommentPage, CollaborationError> {
    authorize(&view, Operation::CommentSend)?;
    state.get().await?.created_comments(query).await
}
