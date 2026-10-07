//! Notification locators and optional immutable subject evidence never grant HTTP authority.
use crate::{CanonicalResource, CapabilityState, RemoteItemKind, ResourceKind, SyncStatus};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NotificationSubjectKind {
    PullRequest,
    Issue,
}

impl NotificationSubjectKind {
    pub fn resource_kind(self) -> ResourceKind {
        match self {
            Self::PullRequest => ResourceKind::PullRequest,
            Self::Issue => ResourceKind::Issue,
        }
    }

    pub fn item_kind(self) -> RemoteItemKind {
        match self {
            Self::PullRequest => RemoteItemKind::PullRequest,
            Self::Issue => RemoteItemKind::Issue,
        }
    }
}

/// Closed adapter representation namespaces. An issue-side PR is not a true issue.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NotificationSubjectRepresentation {
    GithubPullRequest,
    GithubIssue,
    GitlabMergeRequest,
    GitlabIssue,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NotificationSubjectSelector {
    pub kind: NotificationSubjectKind,
    /// Immutable parent identity captured from the same notification observation.
    pub repository_provider_id: String,
    /// Scoped display number; this is never the notification thread or resource ID.
    pub number: String,
    /// Bounded presentation coordinates, checked against that notification's parent.
    pub repository_path: String,
    pub representation: NotificationSubjectRepresentation,
    /// Immutable native subject observed alongside the parent (required for GitLab).
    #[serde(default)]
    pub subject_provider_id: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NotificationSubjectFallbackReason {
    MissingSubjectType,
    InvalidSubjectType,
    UnsupportedSubjectType,
    MissingSubjectUrl,
    InvalidSubjectUrl,
    InvalidApiConfiguration,
    InvalidRepository,
    RepositoryMismatch,
    RepresentationMismatch,
}

/// Per-item fallback does not invalidate an otherwise usable notification page.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", content = "value", rename_all = "snake_case")]
pub enum NotificationSubjectMapping {
    Selector(NotificationSubjectSelector),
    Fallback(NotificationSubjectFallbackReason),
}

/// Native page-observation seam; persistence must later bind page/account/run proof.
/// No serialized command accepts this as renderer-supplied authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotificationSubjectObservation {
    pub notification_id: String,
    pub mapping: NotificationSubjectMapping,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NotificationSubjectState {
    Resolved,
    NotCached,
    Unsupported,
    Ambiguous,
    Unavailable,
    IdentityUnverified,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NotificationSubjectReason {
    MissingSelector,
    UnsupportedSubject,
    InvalidSelector,
    AmbiguousIdentity,
    AuthenticationRequired,
    PermissionDenied,
    InactiveMembership,
    NotCached,
    IdentityUnverified,
    RepresentationMismatch,
    NotFound,
    AttemptsExhausted,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NotificationSubjectQuery {
    pub account_id: String,
    pub authorization_epoch: String,
    pub notification_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiscoverNotificationSubjectRequest {
    pub account_id: String,
    pub authorization_epoch: String,
    pub notification_id: String,
    pub selector_generation: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NotificationSubjectDiscoveryPolicy {
    pub support: CapabilityState,
    /// An explicit local read intent may be accepted; dispatch is separately paused.
    pub admission: bool,
    pub paused: bool,
    pub retry_at: Option<String>,
    pub attempts: u32,
    pub sync: SyncStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NotificationSubjectSnapshot {
    pub revision: String,
    pub authorization_view: String,
    pub authorization_epoch: String,
    pub state: NotificationSubjectState,
    pub reason: Option<NotificationSubjectReason>,
    pub selector_generation: Option<String>,
    pub subject: Option<CanonicalResource>,
    pub fallback_web_url: Option<String>,
    pub discovery: NotificationSubjectDiscoveryPolicy,
}
