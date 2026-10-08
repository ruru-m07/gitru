//! Authored metadata is independent of provider catalog freshness and send authority.
use crate::{
    Coverage, DetailFreshness, IssueDraftContext, IssueDraftSnapshot, IssueDraftSummary, SyncStatus,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IssueMetadataLabel {
    pub provider_id: String,
    pub name: String,
    pub color: Option<String>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IssueMetadataAssignee {
    pub provider_id: String,
    pub login: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IssueMetadataMilestone {
    pub provider_id: String,
    pub number: String,
    pub title: String,
}
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IssueMetadataSelection {
    pub labels: Vec<IssueMetadataLabel>,
    pub assignees: Vec<IssueMetadataAssignee>,
    pub milestone: Option<IssueMetadataMilestone>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IssueMetadataKind {
    Labels,
    Assignees,
    Milestones,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IssueMetadataAvailability {
    Available,
    Unavailable,
    Unknown,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IssueMetadataReason {
    Archived,
    Closed,
    Missing,
    ChangedIdentity,
    ChangedName,
    Permission,
    Unobserved,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum IssueMetadataReference {
    Label(IssueMetadataLabel),
    Assignee(IssueMetadataAssignee),
    Milestone(IssueMetadataMilestone),
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IssueMetadataOption {
    pub reference: IssueMetadataReference,
    /// Catalog observation only; selection always needs dispatch-time point reads.
    pub availability: IssueMetadataAvailability,
    pub reason: Option<IssueMetadataReason>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IssueMetadataQuery {
    pub account_id: String,
    pub repository_id: String,
    pub kind: IssueMetadataKind,
    pub search: String,
    pub cursor: Option<String>,
    pub limit: u32,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IssueMetadataPage {
    pub account_id: String,
    pub repository_id: String,
    pub kind: IssueMetadataKind,
    pub options: Vec<IssueMetadataOption>,
    pub next_cursor: Option<String>,
    pub coverage: Coverage,
    pub freshness: DetailFreshness,
    pub sync: SyncStatus,
    pub revision: String,
    pub authorization_view: String,
    pub catalog_revision: Option<String>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IssueMetadataField {
    Labels,
    Assignees,
    Milestone,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IssueMetadataResult {
    NotRequested,
    Applied,
    Different,
    Unobserved,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IssueMetadataUnobservedReason {
    Missing,
    Oversized,
    Malformed,
    IdentityUnavailable,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IssueMetadataFieldOutcome {
    pub field: IssueMetadataField,
    pub result: IssueMetadataResult,
    pub reason: Option<IssueMetadataUnobservedReason>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IssueMetadataOutcome {
    pub command_id: String,
    pub fields: Vec<IssueMetadataFieldOutcome>,
    pub needs_attention: bool,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SaveIssueDraftV2Request {
    pub account_id: String,
    pub draft_id: String,
    pub repository_id: String,
    pub authorization_epoch: String,
    pub authorization_view: String,
    pub expected_generation: String,
    pub title: String,
    pub body: String,
    pub metadata: IssueMetadataSelection,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SubmitIssueV2Request {
    pub context: IssueDraftContext,
    pub draft_id: String,
    pub draft_generation: String,
    pub command_id: String,
    pub accept_background_delivery: bool,
    pub accept_metadata_best_effort: bool,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IssueDraftV2Snapshot {
    pub draft: IssueDraftSnapshot,
    pub metadata: IssueMetadataSelection,
    /// Historical confirmed201 outcome; hidden with provider authority on reset.
    pub metadata_outcome: Option<IssueMetadataOutcome>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IssueDraftV2Summary {
    pub draft: IssueDraftSummary,
    pub metadata: IssueMetadataSelection,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IssueDraftV2Page {
    pub account_id: String,
    pub drafts: Vec<IssueDraftV2Summary>,
    pub next_cursor: Option<String>,
    pub revision: String,
    pub authorization_view: String,
}

pub(crate) mod native;
