//! Authored conversation comments are separate from private notes and cached lists.
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CommentSendAvailability {
    Available,
    Unavailable,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CommentSendReason {
    UnsupportedProvider,
    AccountUnavailable,
    MissingTarget,
    EmptyDraft,
    AlreadySubmitted,
    PendingSubmission,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommentSendContext {
    pub account_id: String,
    pub subject_id: String,
    pub authorization_epoch: String,
    pub authorization_view: String,
    pub review_token: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommentSubmissionStatus {
    pub command_id: String,
    pub draft_generation: String,
    pub state: String,
    pub attempt_count: u32,
    pub quarantined: bool,
    pub attention: Option<String>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommentDraftSnapshot {
    pub account_id: String,
    pub subject_id: String,
    /// Dedicated comment draft only; never copied from the private note.
    pub body: String,
    /// Zero denotes an unsaved draft. Unchanged saves retain the generation.
    pub generation: String,
    pub context: Option<CommentSendContext>,
    pub availability: CommentSendAvailability,
    pub reason: Option<CommentSendReason>,
    /// Current generation's receipt, or a blocking outstanding older submission.
    pub submission: Option<CommentSubmissionStatus>,
    pub revision: String,
    pub authorization_view: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SaveCommentDraftRequest {
    pub account_id: String,
    pub subject_id: String,
    pub authorization_epoch: String,
    pub authorization_view: String,
    pub expected_generation: String,
    pub body: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SendCommentRequest {
    pub context: CommentSendContext,
    pub draft_generation: String,
    pub command_id: String,
    /// Explicitly authorizes durable queuing for later connectivity.
    pub accept_background_delivery: bool,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommentSubmissionReceipt {
    pub account_id: String,
    pub command_id: String,
    pub admitted_revision: String,
    pub duplicate: bool,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreatedCommentQuery {
    pub account_id: String,
    pub subject_id: String,
    pub cursor: Option<String>,
    pub limit: u32,
}
/// Historical validated creation receipt, not current comment-list coverage.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreatedCommentReceipt {
    pub command_id: String,
    pub draft_generation: String,
    pub provider_id: String,
    pub url: String,
    pub body: String,
    pub author: String,
    pub created_at: String,
    pub observed_at: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreatedCommentPage {
    pub account_id: String,
    pub subject_id: String,
    pub comments: Vec<CreatedCommentReceipt>,
    pub next_cursor: Option<String>,
    pub revision: String,
    pub authorization_view: String,
}
