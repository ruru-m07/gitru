//! Typed, best-effort GitHub label membership intent.
use crate::PendingItemIntent;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LabelIdentity {
    pub provider_id: String,
    pub name: String,
    pub color: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LabelSetAvailability {
    Available,
    Unavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LabelSetReason {
    UnsupportedProvider,
    AccountUnavailable,
    MissingLabels,
    OversizedLabels,
    PendingIntent,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LabelSetContext {
    pub account_id: String,
    pub subject_id: String,
    pub authorization_epoch: String,
    pub authorization_view: String,
    pub review_token: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LabelSetSnapshot {
    pub context: Option<LabelSetContext>,
    pub canonical_labels: Vec<LabelIdentity>,
    pub effective_labels: Vec<LabelIdentity>,
    pub available_labels: Vec<LabelIdentity>,
    /// The first slice derives choices from saved resource metadata only.
    pub catalog_complete: bool,
    pub catalog_truncated: bool,
    pub availability: LabelSetAvailability,
    pub reason: Option<LabelSetReason>,
    pub pending_intent: Option<PendingItemIntent>,
    pub revision: String,
    pub authorization_view: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LabelSetRequest {
    pub context: LabelSetContext,
    pub command_id: String,
    pub add_labels: Vec<LabelIdentity>,
    pub remove_labels: Vec<LabelIdentity>,
    pub accept_best_effort: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LabelSetReceipt {
    pub account_id: String,
    pub command_id: String,
    pub admitted_revision: String,
    pub duplicate: bool,
}

pub(crate) mod native;
