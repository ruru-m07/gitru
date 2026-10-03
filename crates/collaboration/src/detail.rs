//! Typed local detail observations. Summary endpoints have no authority here.
use crate::{
    CapabilityReason, CollaborationError, Coverage, RemoteItemKind, ResourceFacet, SyncStatus,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DetailFacet {
    Body,
    Comments,
    Reviews,
    Checks,
}

impl DetailFacet {
    pub fn capability(self, kind: &RemoteItemKind) -> Option<ResourceFacet> {
        match (self, kind) {
            (Self::Body, RemoteItemKind::PullRequest) => Some(ResourceFacet::PullDetails),
            (Self::Body, RemoteItemKind::Issue) => Some(ResourceFacet::IssueDetails),
            (Self::Comments, RemoteItemKind::PullRequest | RemoteItemKind::Issue) => {
                Some(ResourceFacet::Comments)
            }
            (Self::Reviews, RemoteItemKind::PullRequest) => Some(ResourceFacet::Reviews),
            (Self::Checks, RemoteItemKind::PullRequest) => Some(ResourceFacet::Checks),
            _ => None,
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Self::Body => "body",
            Self::Comments => "comments",
            Self::Reviews => "reviews",
            Self::Checks => "checks",
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

/// Native-only page transaction. IPC never accepts provider observations.
#[derive(Debug, Clone)]
pub struct DetailCommit {
    pub account_id: String,
    pub authorization_epoch: String,
    pub authorization_view: String,
    pub instance_id: String,
    pub subject_id: String,
    pub facet: DetailFacet,
    pub run_id: String,
    pub request_cursor: Option<String>,
    pub body: DetailValue,
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
