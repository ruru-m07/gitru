//! Selected, best-effort GitHub text edits. Native authority never crosses IPC.
use crate::PendingItemIntent;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TextEditAvailability {
    Available,
    Unavailable,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TextEditReason {
    UnsupportedProvider,
    AccountUnavailable,
    MissingBase,
    OversizedText,
    PendingIntent,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TextEditContext {
    pub account_id: String,
    pub subject_id: String,
    pub authorization_epoch: String,
    pub authorization_view: String,
    pub review_token: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TextEditSnapshot {
    pub context: Option<TextEditContext>,
    pub title: Option<String>,
    /// Known null is preserved; a missing context means this is not an edit base.
    pub body: Option<String>,
    pub availability: TextEditAvailability,
    pub reason: Option<TextEditReason>,
    pub pending_intent: Option<PendingItemIntent>,
    pub revision: String,
    pub authorization_view: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TextEditRequest {
    pub context: TextEditContext,
    pub command_id: String,
    pub accept_best_effort: bool,
    /// None omits this field from the provider request.
    pub title: Option<String>,
    /// Some("") explicitly clears the body; None leaves it unchanged.
    pub body: Option<String>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TextEditReceipt {
    pub account_id: String,
    pub command_id: String,
    pub admitted_revision: String,
    pub duplicate: bool,
}

pub(crate) mod native;
