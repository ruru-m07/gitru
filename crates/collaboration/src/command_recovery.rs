//! Local review of durable intent. These DTOs never grant provider authority.
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommandRecoveryQuery {
    pub account_id: String,
    pub target_id: Option<String>,
    pub include_terminal: bool,
    pub cursor: Option<String>,
    pub limit: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommandRecoverySummary {
    pub account_id: String,
    pub command_id: String,
    pub target_id: String,
    pub target_kind: String,
    pub operation_kind: String,
    pub payload_version: u32,
    pub state: String,
    pub admitted_at: String,
    pub attempt_count: u32,
    pub paused: bool,
    pub quarantined: bool,
    pub attention: Option<String>,
    pub replacement_id: Option<String>,
    pub blocked_reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommandRecoverySnapshot {
    pub commands: Vec<CommandRecoverySummary>,
    pub next_cursor: Option<String>,
    pub revision: String,
    pub authorization_view: String,
}

/// `known: true, value: None` is a known null (such as an empty body).
/// `known: false` is unavailable evidence and must never imply equality.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommandFieldValue {
    pub known: bool,
    pub value: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CommandReviewField {
    Title,
    Body,
    State,
    Unread,
    Head,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CommandFieldComparison {
    Unchanged,
    Independent,
    Converged,
    Conflict,
    Unknown,
    GuardChanged,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommandFieldReview {
    pub field: CommandReviewField,
    pub base: CommandFieldValue,
    pub remote: CommandFieldValue,
    pub desired: CommandFieldValue,
    pub comparison: CommandFieldComparison,
    pub editable: bool,
}

/// Native snapshot proof carried unchanged by every local action. The writer
/// recomputes the review token; the renderer cannot manufacture a fresh base.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommandRecoveryContext {
    pub account_id: String,
    pub command_id: String,
    pub expected_generation: String,
    pub expected_epoch: String,
    pub authorization_view: String,
    pub review_token: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommandRecoveryDetail {
    pub command: CommandRecoverySummary,
    pub context: CommandRecoveryContext,
    pub fields: Vec<CommandFieldReview>,
    pub can_retry: bool,
    pub can_cancel: bool,
    pub can_pause: bool,
    pub can_replace: bool,
    pub reason: Option<String>,
    pub revision: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CommandRecoveryAction {
    Cancel,
    Pause,
    Resume,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommandRecoveryActionRequest {
    pub context: CommandRecoveryContext,
    pub action_id: String,
    pub action: CommandRecoveryAction,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CommandResolutionChoice {
    KeepDesired,
    UseRemote,
    Edited,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommandFieldResolution {
    pub field: CommandReviewField,
    pub choice: CommandResolutionChoice,
    /// Only `edited` may supply a value. Null is an explicit empty value.
    /// The registered native codec validates field-specific representations.
    pub value: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommandRecoveryReplaceRequest {
    pub context: CommandRecoveryContext,
    pub action_id: String,
    pub new_command_id: String,
    pub fields: Vec<CommandFieldResolution>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommandRecoveryReceipt {
    pub account_id: String,
    pub action_id: String,
    pub command_id: String,
    pub replacement_id: Option<String>,
    pub state: String,
    pub paused: bool,
    pub remote_may_have_happened: bool,
    pub revision: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommandRecoveryExport {
    pub suggested_name: String,
    pub text: String,
}
