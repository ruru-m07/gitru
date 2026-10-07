//! Selected provider inbox operations. Local admission is not remote confirmation.
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderInboxAction {
    MarkRead,
    MarkDone,
}

/// Neither selected endpoint exposes server-side activity CAS. The UI must
/// disclose that delivery can apply to provider activity arriving concurrently.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderInboxActivityPolicy {
    BestEffortCurrentItem,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderInboxActionAvailability {
    Available,
    Unavailable,
    Unsupported,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderInboxActionReason {
    NotImplemented,
    ProviderSemantics,
    MissingSourceEvidence,
    AlreadyApplied,
    AuthenticationRequired,
    UnsupportedCredential,
    PendingCommand,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderInboxActionDescriptor {
    pub action: ProviderInboxAction,
    pub availability: ProviderInboxActionAvailability,
    pub reason: Option<ProviderInboxActionReason>,
    pub activity_policy: ProviderInboxActivityPolicy,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderInboxActionsQuery {
    pub account_id: String,
    /// Canonical local notification item UUID, not the linked PR/issue identity.
    pub subject_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderInboxActionsSnapshot {
    pub account_id: String,
    pub subject_id: String,
    pub authorization_epoch: String,
    pub authorization_view: String,
    pub activity_version: String,
    pub actions: Vec<ProviderInboxActionDescriptor>,
    pub revision: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QueueProviderInboxActionRequest {
    pub account_id: String,
    pub authorization_epoch: String,
    pub authorization_view: String,
    /// Canonical local notification item UUID; no provider URL is accepted.
    pub subject_id: String,
    pub expected_activity_version: String,
    /// Retain the same UUID when retrying after a lost local IPC receipt.
    pub command_id: String,
    pub action: ProviderInboxAction,
    pub activity_policy: ProviderInboxActivityPolicy,
}

/// Durable local intent only; query effective state/recovery for remote outcome.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderInboxActionReceipt {
    pub account_id: String,
    pub command_id: String,
    pub revision: String,
    pub duplicate: bool,
}
