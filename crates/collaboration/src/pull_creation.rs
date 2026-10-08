//! Authored PR drafts and explicit online creation; branch references are not remote CAS.
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PullDraftKey {
    pub account_id: String,
    pub draft_id: String,
    pub repository_id: String,
}
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PullDraftValues {
    pub title: String,
    pub body: String,
    pub source_branch: String,
    pub base_branch: String,
    pub local_repository_id: String,
    pub link_id: String,
    pub link_generation: String,
    pub is_draft: bool,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PullCreationReason {
    UnsupportedProvider,
    AccountUnavailable,
    MissingRepository,
    IncompleteDraft,
    PendingSubmission,
    AlreadySubmitted,
    LocalMappingChanged,
    LocalHeadChanged,
    UnpublishedSource,
    SameBranch,
    PermissionUnavailable,
    GrantExpired,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PullSubmissionStatus {
    pub command_id: String,
    pub draft_generation: String,
    pub state: String,
    pub attempt_count: u32,
    pub quarantined: bool,
    pub attention: Option<String>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreatedPullIdentity {
    pub subject_id: String,
    pub provider_id: String,
    pub number: String,
    pub url: String,
    pub command_id: String,
    pub inspected_source_oid: String,
    pub inspected_base_oid: String,
    pub observed_source_oid: String,
    pub observed_base_oid: String,
    pub branches_changed: bool,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PullDraftSnapshot {
    pub key: PullDraftKey,
    pub values: PullDraftValues,
    pub generation: String,
    pub can_preview: bool,
    pub reason: Option<PullCreationReason>,
    pub submission: Option<PullSubmissionStatus>,
    pub published: Option<CreatedPullIdentity>,
    pub revision: String,
    pub authorization_view: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SavePullDraftRequest {
    pub key: PullDraftKey,
    pub authorization_epoch: String,
    pub authorization_view: String,
    pub expected_generation: String,
    pub values: PullDraftValues,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PreviewPullCreationRequest {
    pub key: PullDraftKey,
    pub draft_generation: String,
    pub authorization_epoch: String,
    pub authorization_view: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PullCreationContext {
    pub key: PullDraftKey,
    pub draft_generation: String,
    pub authorization_epoch: String,
    pub authorization_view: String,
    pub grant_id: String,
    pub source_oid: String,
    pub base_oid: String,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PullCreationPolicy {
    BestEffortCurrentBranches,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PullCreationPreview {
    pub context: Option<PullCreationContext>,
    pub reason: Option<PullCreationReason>,
    pub values: PullDraftValues,
    pub local_source_oid: String,
    pub observed_source_oid: Option<String>,
    pub observed_base_oid: Option<String>,
    pub can_push: Option<bool>,
    pub observed_at: String,
    pub expires_in_seconds: u32,
    pub authorization_view: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SubmitPullRequest {
    pub context: PullCreationContext,
    pub command_id: String,
    pub policy: PullCreationPolicy,
    pub confirm_current_branches: bool,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PullSubmissionReceipt {
    pub account_id: String,
    pub command_id: String,
    pub admitted_revision: String,
    pub duplicate: bool,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PullDraftQuery {
    pub account_id: String,
    pub cursor: Option<String>,
    pub limit: u32,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PullDraftSummary {
    pub draft_id: String,
    pub repository_id: String,
    pub title: String,
    pub preview: String,
    pub source_branch: String,
    pub base_branch: String,
    pub generation: String,
    pub submission: Option<PullSubmissionStatus>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PullDraftPage {
    pub account_id: String,
    pub drafts: Vec<PullDraftSummary>,
    pub next_cursor: Option<String>,
    pub revision: String,
    pub authorization_view: String,
}
/// Native-only, freshly observed by the registered local Git caller.
#[derive(Debug, Clone)]
pub struct PullCreationLocalObservation {
    pub query: crate::LocalLinkQuery,
    pub link: crate::LocalLinkVersion,
    pub source_branch: String,
    pub source_oid: String,
}

pub(crate) mod native;
