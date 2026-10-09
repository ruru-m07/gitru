//! A local policy projection. Support, access and observation are independent;
//! this metadata never authorizes a remote command or reads a credential.
use serde::{Deserialize, Serialize};

use crate::{
    CapabilityState, InboxSemantics, ProviderInstance, ResourceFacet, ResourceKind, SyncStatus,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityTargetKind {
    Account,
    Repository,
    Resource,
}

/// The redundant instance and kind are checked against immutable identities.
/// Unused fields must be absent, rather than silently changing the target.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapabilityTarget {
    pub kind: CapabilityTargetKind,
    pub instance_id: Option<String>,
    pub repository_id: Option<String>,
    pub resource_id: Option<String>,
    pub resource_kind: Option<ResourceKind>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextCapabilityRequest {
    pub account_id: String,
    pub authorization_epoch: String,
    pub target: CapabilityTarget,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContextCapabilityReason {
    NotImplemented,
    ProviderSemantics,
    AdapterUnavailable,
    AuthenticationRequired,
    MissingScope,
    PermissionDenied,
    TemporarilyUnavailable,
    NotObserved,
    NotApplicable,
    RepositoryNotSelected,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextCapabilityAccess {
    pub state: CapabilityState,
    pub reason: Option<ContextCapabilityReason>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityObservation {
    Unknown,
    NotLoaded,
    Partial,
    Complete,
    Empty,
    Omitted,
    Oversized,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextFacetCapability {
    pub facet: ResourceFacet,
    pub saved_read: ContextCapabilityAccess,
    pub synchronize: ContextCapabilityAccess,
    /// Read support and PAT grants cannot imply a delivery implementation.
    pub remote_write: ContextCapabilityAccess,
    pub observation: CapabilityObservation,
    pub sync: SyncStatus,
    /// A separate user intent, still checked/coalesced/limited by the runtime.
    pub can_recheck_access: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextualCapabilitySnapshot {
    pub account_id: String,
    pub authorization_epoch: String,
    pub instance: ProviderInstance,
    pub target: CapabilityTarget,
    pub facets: Vec<ContextFacetCapability>,
    pub inbox_semantics: InboxSemantics,
    pub revision: String,
    pub authorization_view: String,
}
