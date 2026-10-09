//! Ephemeral view interest. Callers and visibility are supplied by the native host.
use crate::DetailFacet;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DemandTargetKind {
    Repositories,
    Inbox,
    PullRequests,
    Issues,
    Detail,
    RepositoryLabels,
    RepositoryAssignees,
    RepositoryMilestones,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DemandTarget {
    pub kind: DemandTargetKind,
    /// None for an index covers selected repositories through bounded rotation.
    pub repository_id: Option<String>,
    pub subject_id: Option<String>,
    pub facet: Option<DetailFacet>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DemandOwnerActivity {
    /// Positive decimal native sequence, increasing on transitions/disposal/reopen.
    pub generation: String,
    pub active: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AcquireDemandRequest {
    pub account_id: String,
    pub authorization_epoch: String,
    pub owner_generation: String,
    pub target: DemandTarget,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DemandLeaseReceipt {
    pub lease_id: String,
    pub owner_generation: String,
    pub expires_in_seconds: u32,
    pub renew_after_seconds: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DemandLeaseRenewal {
    pub lease_id: String,
    pub account_id: String,
    pub authorization_epoch: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenewDemandRequest {
    pub owner_generation: String,
    /// One owner heartbeat; validation is atomic and bounded to sixteen handles.
    pub leases: Vec<DemandLeaseRenewal>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DemandRenewalReceipt {
    pub leases: Vec<DemandLeaseReceipt>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReleaseDemandRequest {
    pub lease_id: String,
}
