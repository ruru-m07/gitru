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
    /// Canonical local notification item identity, not the linked PR/issue identity.
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
    /// Canonical local notification item identity; no provider URL is accepted.
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

pub(crate) mod native;

fn bounded_id(value: &str) -> Result<(), crate::CollaborationError> {
    if value.is_empty() || value.len() > 1024 || value.chars().any(char::is_control) {
        return Err(crate::CollaborationError::invalid(
            "Invalid bounded inbox identity",
        ));
    }
    Ok(())
}
fn canonical_revision(value: &str) -> Result<(), crate::CollaborationError> {
    if value.is_empty()
        || value.len() > 19
        || !value.bytes().all(|b| b.is_ascii_digit())
        || !value
            .parse::<i64>()
            .is_ok_and(|n| n > 0 && n.to_string() == value)
    {
        return Err(crate::CollaborationError::invalid(
            "Invalid inbox authorization revision",
        ));
    }
    Ok(())
}
impl ProviderInboxActionsQuery {
    pub(crate) fn validate(&self) -> Result<(), crate::CollaborationError> {
        bounded_id(&self.account_id)?;
        bounded_id(&self.subject_id)
    }
}
impl QueueProviderInboxActionRequest {
    pub(crate) fn validate(&self) -> Result<(), crate::CollaborationError> {
        bounded_id(&self.account_id)?;
        bounded_id(&self.subject_id)?;
        canonical_revision(&self.authorization_epoch)?;
        canonical_revision(&self.authorization_view)?;
        if self.command_id.len() != 36
            || !uuid::Uuid::parse_str(&self.command_id)
                .is_ok_and(|id| id.hyphenated().to_string() == self.command_id)
            || self.expected_activity_version.len() != 64
            || !self
                .expected_activity_version
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err(crate::CollaborationError::invalid(
                "Invalid inbox action proof or command identity",
            ));
        }
        Ok(())
    }
}
