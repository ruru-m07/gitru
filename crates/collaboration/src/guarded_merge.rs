//! Online-only guarded merge; a local receipt is never an assertion of merging.
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MergeMethod {
    Merge,
    Squash,
    Rebase,
}
impl MergeMethod {
    pub(crate) fn wire(self) -> &'static str {
        match self {
            Self::Merge => "merge",
            Self::Squash => "squash",
            Self::Rebase => "rebase",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MergeUnavailableReason {
    UnsupportedProvider,
    AccountUnavailable,
    MissingContext,
    PendingCommand,
    HeadChanged,
    NotOpen,
    Draft,
    PermissionUnavailable,
    MergeabilityUnavailable,
    MethodsUnavailable,
    AutomaticMergeUnsupported,
    ConsentExpired,
    ProviderConflict,
    ProviderRejected,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GuardedMergeQuery {
    pub account_id: String,
    pub subject_id: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GuardedMergeContext {
    pub account_id: String,
    pub subject_id: String,
    pub authorization_epoch: String,
    pub authorization_view: String,
    pub expected_head: String,
    pub grant_id: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GuardedMergePreview {
    pub context: Option<GuardedMergeContext>,
    pub expected_head: Option<String>,
    pub methods: Vec<MergeMethod>,
    pub can_push: Option<bool>,
    pub mergeable: Option<bool>,
    pub provider_mergeability: Option<String>,
    pub reason: Option<MergeUnavailableReason>,
    pub observed_at: Option<String>,
    pub expires_in_seconds: u32,
    pub authorization_view: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GuardedMergeStatus {
    pub command_id: String,
    pub state: String,
    pub method: MergeMethod,
    pub expected_head: String,
    pub attempt_count: u32,
    pub attention: Option<String>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GuardedMergeSnapshot {
    pub reason: Option<MergeUnavailableReason>,
    pub latest: Option<GuardedMergeStatus>,
    pub revision: String,
    pub authorization_view: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GuardedMergeRequest {
    pub context: GuardedMergeContext,
    pub command_id: String,
    pub method: MergeMethod,
    pub confirm_inspected_head: bool,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GuardedMergeReceipt {
    pub account_id: String,
    pub command_id: String,
    pub admitted_revision: String,
    pub duplicate: bool,
}

pub(crate) mod native;
