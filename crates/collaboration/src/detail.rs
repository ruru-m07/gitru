//! Typed local detail observations. Summary endpoints have no authority here.
use crate::resource_metadata::ResourceMetadataSnapshot;
use crate::{
    CapabilityReason, CollaborationError, Coverage, NativeDetailPayload, RemoteItemKind,
    ResourceFacet, SyncStatus,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DetailFacet {
    Body,
    Comments,
    Reviews,
    Checks,
    Participants,
    Tasks,
    Commits,
    Files,
}

impl DetailFacet {
    pub(crate) const ALL: [Self; 8] = [
        Self::Body,
        Self::Comments,
        Self::Reviews,
        Self::Checks,
        Self::Participants,
        Self::Tasks,
        Self::Commits,
        Self::Files,
    ];
    pub(crate) fn field_limit(self) -> usize {
        if self == Self::Tasks { 12 } else { 6 }
    }
    pub fn capability(self, kind: &RemoteItemKind) -> Option<ResourceFacet> {
        match (self, kind) {
            (Self::Body, RemoteItemKind::PullRequest) => Some(ResourceFacet::PullDetails),
            (Self::Body, RemoteItemKind::Issue) => Some(ResourceFacet::IssueDetails),
            (Self::Comments, RemoteItemKind::PullRequest | RemoteItemKind::Issue) => {
                Some(ResourceFacet::Comments)
            }
            (Self::Reviews, RemoteItemKind::PullRequest) => Some(ResourceFacet::Reviews),
            (Self::Checks, RemoteItemKind::PullRequest) => Some(ResourceFacet::Checks),
            (Self::Participants, RemoteItemKind::PullRequest) => Some(ResourceFacet::Participants),
            (Self::Tasks, RemoteItemKind::PullRequest) => Some(ResourceFacet::Tasks),
            (Self::Commits, RemoteItemKind::PullRequest) => Some(ResourceFacet::PullCommits),
            (Self::Files, RemoteItemKind::PullRequest) => Some(ResourceFacet::PullFiles),
            _ => None,
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Self::Body => "body",
            Self::Comments => "comments",
            Self::Reviews => "reviews",
            Self::Checks => "checks",
            Self::Participants => "participants",
            Self::Tasks => "tasks",
            Self::Commits => "commits",
            Self::Files => "files",
        }
    }
    pub fn scope(self, subject_id: &str) -> String {
        format!("detail:{subject_id}:{}", self.name())
    }
    pub(crate) fn from_scope(scope: &str) -> Option<(&str, Self)> {
        let (subject, facet) = scope.strip_prefix("detail:")?.rsplit_once(':')?;
        if subject.is_empty() {
            return None;
        }
        Some((
            subject,
            match facet {
                "body" => Self::Body,
                "comments" => Self::Comments,
                "reviews" => Self::Reviews,
                "checks" => Self::Checks,
                "participants" => Self::Participants,
                "tasks" => Self::Tasks,
                "commits" => Self::Commits,
                "files" => Self::Files,
                _ => return None,
            },
        ))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DetailValueState {
    NotLoaded,
    Known,
    Omitted,
    Oversized,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DetailValue {
    pub state: DetailValueState,
    /// Known null means an authoritative absence; an empty string is also known.
    pub text: Option<String>,
}
impl Default for DetailValue {
    fn default() -> Self {
        Self {
            state: DetailValueState::NotLoaded,
            text: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DetailField {
    Body,
    Author,
    Title,
    State,
    UpdatedAt,
    HeadOid,
    ParticipantLogin,
    ParticipantDisplayName,
    ParticipantRole,
    ParticipantApproved,
    ParticipantState,
    ParticipantParticipatedAt,
    TaskContent,
    TaskCreatorLogin,
    TaskCreatorDisplayName,
    TaskState,
    TaskCreatedAt,
    TaskUpdatedAt,
    TaskPending,
    TaskResolvedAt,
    TaskResolver,
    TaskResolverLogin,
    TaskResolverDisplayName,
    TaskCommentId,
}

impl DetailField {
    pub(crate) fn is_participant(self) -> bool {
        matches!(
            self,
            Self::ParticipantLogin
                | Self::ParticipantDisplayName
                | Self::ParticipantRole
                | Self::ParticipantApproved
                | Self::ParticipantState
                | Self::ParticipantParticipatedAt
        )
    }
    pub(crate) fn is_task(self) -> bool {
        matches!(
            self,
            Self::TaskContent
                | Self::TaskCreatorLogin
                | Self::TaskCreatorDisplayName
                | Self::TaskState
                | Self::TaskCreatedAt
                | Self::TaskUpdatedAt
                | Self::TaskPending
                | Self::TaskResolvedAt
                | Self::TaskResolver
                | Self::TaskResolverLogin
                | Self::TaskResolverDisplayName
                | Self::TaskCommentId
        )
    }
    pub(crate) fn valid_for(self, facet: DetailFacet) -> bool {
        match facet {
            DetailFacet::Participants => self.is_participant(),
            DetailFacet::Tasks => self.is_task(),
            DetailFacet::Commits | DetailFacet::Files => false,
            _ => !self.is_participant() && !self.is_task(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DetailFieldValidation {
    pub field: DetailField,
    pub validated_at: String,
    pub source: String,
    pub adapter_version: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DetailEntry {
    /// Stable adapter-owned identity within this subject/facet (not a list index).
    pub id: String,
    pub provider_id: String,
    pub author: Option<String>,
    pub title: Option<String>,
    /// Preserve native review/check state strings, including unknown future values.
    pub state: Option<String>,
    pub body: DetailValue,
    pub observed_body_state: DetailValueState,
    pub updated_at: Option<String>,
    pub head_oid: Option<String>,
    /// Versioned native facts. Missing legacy JSON decodes as no native payload.
    #[serde(default)]
    pub native: Option<NativeDetailPayload>,
    pub field_mask: Vec<DetailField>,
    /// Engine-owned per-field validation; omitted fields retain their old time.
    pub field_validations: Vec<DetailFieldValidation>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DetailSource {
    /// Endpoint/API-version identifier, never a token or arbitrary response dump.
    pub source: String,
    pub adapter_version: u32,
    pub field_mask: Vec<DetailField>,
    /// Only an adapter-declared comparable facet timestamp, never its parent's.
    pub provider_updated_at: Option<String>,
    pub observed_at: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DetailAvailability {
    Missing,
    Partial,
    Ready,
    Unavailable,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DetailFreshness {
    Unknown,
    Fresh,
    Stale,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DetailEvidence {
    pub facet: DetailFacet,
    pub availability: DetailAvailability,
    pub coverage: Coverage,
    pub freshness: DetailFreshness,
    pub stale_at: Option<String>,
    pub facet_revision: Option<String>,
    pub authorization_epoch: String,
    pub access_reason: Option<CapabilityReason>,
    pub source: Option<DetailSource>,
    /// Source authority for the saved value; omission cannot replace its clock.
    pub value_source: Option<DetailSource>,
    /// Authorized, complete saved content emptiness; never description text.
    pub saved_empty: Option<bool>,
    /// Most recent endpoint observation, distinct from a retained saved value.
    pub observed_state: DetailValueState,
    pub sync: SyncStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DetailQuery {
    pub account_id: String,
    pub subject_id: String,
    pub facet: DetailFacet,
    pub cursor: Option<String>,
    pub limit: u32,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DetailSnapshot {
    pub subject_id: String,
    pub body: DetailValue,
    pub metadata: Option<ResourceMetadataSnapshot>,
    pub entries: Vec<DetailEntry>,
    pub next_cursor: Option<String>,
    pub evidence: DetailEvidence,
    pub revision: String,
    pub authorization_view: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HydrateDetailRequest {
    pub account_id: String,
    pub authorization_epoch: String,
    pub subject_id: String,
    pub facet: DetailFacet,
}

/// Native adapter evidence. Exhausting a delta is not full absence evidence.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DetailEnumeration {
    FullEnumeration,
    Incremental,
    #[default]
    Uncertain,
}

/// Historical reviews/comments and a current-head check set have different scopes.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DetailHeadScope {
    #[default]
    SubjectHistory,
    CurrentHead,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DetailReconciliation {
    pub enumeration: DetailEnumeration,
    pub head_scope: DetailHeadScope,
}
impl DetailReconciliation {
    /// Adapter declaration for a complete subject-history enumeration.
    pub const fn full_history() -> Self {
        Self {
            enumeration: DetailEnumeration::FullEnumeration,
            head_scope: DetailHeadScope::SubjectHistory,
        }
    }
}

/// Native-only page transaction. IPC never accepts provider observations.
#[derive(Debug, Clone)]
pub struct DetailCommit {
    pub reconciliation: DetailReconciliation,
    pub account_id: String,
    pub authorization_epoch: String,
    pub authorization_view: String,
    pub instance_id: String,
    pub subject_id: String,
    pub facet: DetailFacet,
    pub run_id: String,
    pub request_cursor: Option<String>,
    pub body: DetailValue,
    pub metadata: Option<crate::ResourceMetadataObservation>,
    pub subject_binding: Option<crate::DetailSubjectBinding>,
    pub entries: Vec<DetailEntry>,
    pub source: DetailSource,
    pub next_cursor: Option<String>,
    pub etag: Option<String>,
    pub not_modified: bool,
    /// Validator covers this whole facet, not a later pagination page.
    pub whole_scope: bool,
    pub complete: bool,
    pub freshness_seconds: u32,
}

#[derive(Debug, Clone)]
pub struct DetailLease {
    pub run_id: String,
    pub authorization_view: String,
    pub instance_id: String,
    pub next_cursor: Option<String>,
    pub etag: Option<String>,
    pub source: Option<DetailSource>,
    pub reconciliation: Option<DetailReconciliation>,
}

#[derive(Debug, Clone)]
pub struct DetailDemand {
    pub account_id: String,
    pub subject_id: String,
    pub facet: DetailFacet,
}

pub(crate) fn invalid_detail() -> CollaborationError {
    CollaborationError::invalid("Invalid or unbounded detail observation")
}
