//! Notification selectors are locators, never native subject IDs or HTTP authority.
use crate::{RemoteItemKind, ResourceKind};
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
