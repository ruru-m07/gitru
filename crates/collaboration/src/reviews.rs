//! Provider-independent pull-request review and anchored thread observations.
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReviewContext {
    pub base_oid: String,
    pub head_oid: String,
    pub base_repository_provider_id: String,
    pub source_repository_provider_id: String,
    pub metadata_facet_revision: String,
}

impl ReviewContext {
    pub(crate) fn is_valid(&self) -> bool {
        crate::is_canonical_commit_oid(&self.base_oid)
            && crate::is_canonical_commit_oid(&self.head_oid)
            && bounded_identity(&self.base_repository_provider_id, 512)
            && bounded_identity(&self.source_repository_provider_id, 512)
            && self
                .metadata_facet_revision
                .parse::<u64>()
                .is_ok_and(|value| value > 0 && value.to_string() == self.metadata_facet_revision)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReviewActor {
    pub provider_id: String,
    pub login: Option<String>,
    pub display_name: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewDecision {
    Approved,
    ChangesRequested,
    Commented,
    Dismissed,
    Pending,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReviewV1 {
    pub context: ReviewContext,
    pub reviewer: Option<ReviewActor>,
    pub decision: ReviewDecision,
    pub provider_state: String,
    pub reviewed_commit_oid: Option<String>,
    pub submitted_at: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewAnchorSubject {
    Line,
    File,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewDiffSide {
    Left,
    Right,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReviewAnchor {
    pub path: String,
    pub commit_oid: String,
    pub original_commit_oid: String,
    pub subject: ReviewAnchorSubject,
    pub start_line: Option<u32>,
    pub line: Option<u32>,
    pub start_side: Option<ReviewDiffSide>,
    pub side: Option<ReviewDiffSide>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReviewThreadV1 {
    pub context: ReviewContext,
    pub thread_id: String,
    /// Optional native root identity. Some providers group notes only by a
    /// discussion ID and expose no reply-parent relation.
    pub root_comment_id: Option<String>,
    pub comment_id: String,
    pub parent_comment_id: Option<String>,
    pub review_id: Option<String>,
    pub author: Option<ReviewActor>,
    pub created_at: String,
    pub updated_at: String,
    /// Native file/diff anchor when the provider exposes one. General/system
    /// discussions may truthfully have no anchor.
    pub anchor: Option<ReviewAnchor>,
    /// Provider-reported only. `None` means unavailable or unsupported.
    pub provider_outdated: Option<bool>,
    /// Provider-reported only. `None` means unavailable or unsupported.
    pub provider_resolved: Option<bool>,
}

#[derive(Debug, Clone)]
pub struct ReviewRequest {
    pub detail: crate::providers::DetailRequest,
    pub context: ReviewContext,
}

pub(crate) fn bounded_identity(value: &str, limit: usize) -> bool {
    !value.is_empty() && value.len() <= limit && !value.chars().any(char::is_control)
}

pub(crate) fn bounded_optional(value: &Option<String>, limit: usize) -> bool {
    value
        .as_deref()
        .is_none_or(|value| value.len() <= limit && !value.chars().any(char::is_control))
}

pub(crate) fn actor_valid(actor: &ReviewActor) -> bool {
    bounded_identity(&actor.provider_id, 512)
        && bounded_optional(&actor.login, 255)
        && bounded_optional(&actor.display_name, 1024)
}
