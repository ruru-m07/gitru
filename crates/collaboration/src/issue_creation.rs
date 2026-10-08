//! Durable local issue drafts resolve to a canonical issue only after validated creation.
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IssueDraftAvailability {
    Available,
    Unavailable,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IssueDraftReason {
    UnsupportedProvider,
    AccountUnavailable,
    MissingRepository,
    EmptyTitle,
    PendingSubmission,
    AlreadySubmitted,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IssueDraftKey {
    pub account_id: String,
    pub draft_id: String,
    pub repository_id: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IssueDraftContext {
    pub account_id: String,
    pub repository_id: String,
    pub authorization_epoch: String,
    pub authorization_view: String,
    pub review_token: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IssueSubmissionStatus {
    pub command_id: String,
    pub draft_generation: String,
    pub state: String,
    pub attempt_count: u32,
    pub quarantined: bool,
    pub attention: Option<String>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreatedIssueIdentity {
    pub subject_id: String,
    pub provider_id: String,
    pub number: String,
    pub url: String,
    pub command_id: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IssueDraftSnapshot {
    pub account_id: String,
    pub draft_id: String,
    pub repository_id: String,
    pub title: String,
    pub body: String,
    pub generation: String,
    pub context: Option<IssueDraftContext>,
    pub availability: IssueDraftAvailability,
    pub reason: Option<IssueDraftReason>,
    pub submission: Option<IssueSubmissionStatus>,
    pub published: Option<CreatedIssueIdentity>,
    pub revision: String,
    pub authorization_view: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SaveIssueDraftRequest {
    pub account_id: String,
    pub draft_id: String,
    pub repository_id: String,
    pub authorization_epoch: String,
    pub authorization_view: String,
    pub expected_generation: String,
    pub title: String,
    pub body: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SubmitIssueRequest {
    pub context: IssueDraftContext,
    pub draft_id: String,
    pub draft_generation: String,
    pub command_id: String,
    pub accept_background_delivery: bool,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IssueSubmissionReceipt {
    pub account_id: String,
    pub command_id: String,
    pub admitted_revision: String,
    pub duplicate: bool,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IssueDraftQuery {
    pub account_id: String,
    pub cursor: Option<String>,
    pub limit: u32,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IssueDraftSummary {
    pub draft_id: String,
    pub repository_id: String,
    pub title: String,
    pub preview: String,
    pub generation: String,
    pub submission: Option<IssueSubmissionStatus>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IssueDraftPage {
    pub account_id: String,
    pub drafts: Vec<IssueDraftSummary>,
    pub next_cursor: Option<String>,
    pub revision: String,
    pub authorization_view: String,
}

pub(crate) mod native;
