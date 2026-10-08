//! Native-only catalog reads. The runtime owns leases, epochs, budgets and persistence.
use crate::{
    IssueMetadataAvailability, IssueMetadataKind, IssueMetadataOption, IssueMetadataReason,
    IssueMetadataReference, RemoteAccount, RemoteRepository,
};

#[derive(Debug, Clone)]
pub struct IssueMetadataReadContext {
    pub account: RemoteAccount,
    pub repository: RemoteRepository,
    pub authorization_view: String,
}
#[derive(Debug, Clone)]
pub struct IssueMetadataCatalogRequest {
    pub context: IssueMetadataReadContext,
    pub kind: IssueMetadataKind,
    /// Native catalog run identity; unrelated global revisions never retire paging.
    pub catalog_generation: String,
    pub cursor: Option<String>,
}
#[derive(Debug, Clone)]
pub struct IssueMetadataCatalogPage {
    pub options: Vec<IssueMetadataOption>,
    pub next_cursor: Option<String>,
    /// A terminal bounded traversal is still refreshable and cannot prove absence.
    pub truncated: bool,
    /// Mutable enumeration never supplies absence authority.
    pub coverage: crate::CoverageState,
    pub cooldown_seconds: Option<u64>,
}
#[derive(Debug, Clone)]
pub enum IssueMetadataPoint {
    Repository,
    Label(crate::IssueMetadataLabel),
    AssigneeIdentity(crate::IssueMetadataAssignee),
    AssigneeAssignable(crate::IssueMetadataAssignee),
    Milestone(crate::IssueMetadataMilestone),
}
#[derive(Debug, Clone)]
pub struct IssueMetadataPointRequest {
    pub context: IssueMetadataReadContext,
    pub point: IssueMetadataPoint,
}
#[derive(Debug, Clone)]
pub enum IssueMetadataPointValue {
    Repository {
        metadata_access: IssueMetadataAvailability,
    },
    Selection {
        reference: IssueMetadataReference,
        identity_matches: bool,
        availability: IssueMetadataAvailability,
        reason: Option<IssueMetadataReason>,
    },
    Assignable,
}
#[derive(Debug, Clone)]
pub struct IssueMetadataPointRead {
    pub value: IssueMetadataPointValue,
    pub cooldown_seconds: Option<u64>,
}
