//! Adapters normalize provider resources; scheduling and persistence stay outside.

#[cfg(test)]
mod contract_tests;
pub mod github;
pub mod gitlab;
mod registry;
mod transport;

pub(crate) use registry::FACETS;
pub use registry::{ProviderProfile, ProviderRegistry};
pub use transport::{ProviderError, ProviderErrorKind};

use crate::{CollaborationError, ErrorCode, credentials::SecretToken, detail::*, domain::*};
use async_trait::async_trait;

#[derive(Debug, Clone)]
pub struct VerifiedAccount {
    pub actor_id: String,
    pub login: String,
    pub display_name: Option<String>,
    pub notifications_supported: bool,
    /// Quota observed while verifying operations, before account promotion.
    pub cooldown_seconds: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FeedKind {
    Repositories,
    PullRequests,
    Issues,
    Notifications,
}

#[derive(Debug, Clone)]
pub struct FeedRequest {
    pub account: RemoteAccount,
    pub kind: FeedKind,
    pub repository: Option<RemoteRepository>,
    pub cursor: Option<String>,
    pub etag: Option<String>,
    pub last_modified: Option<String>,
}

#[derive(Debug, Clone)]
pub struct FetchPage {
    pub repositories: Vec<RemoteRepository>,
    pub items: Vec<RemoteItem>,
    pub endpoint_aliases: Vec<EndpointAlias>,
    pub notification_subjects: Vec<crate::NotificationSubjectObservation>,
    pub next_cursor: Option<String>,
    pub etag: Option<String>,
    pub last_modified: Option<String>,
    pub not_modified: bool,
    pub poll_interval_seconds: Option<u64>,
    pub cooldown_seconds: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct DetailRequest {
    pub account: RemoteAccount,
    pub repository: RemoteRepository,
    pub subject: RemoteItem,
    pub facet: DetailFacet,
    pub cursor: Option<String>,
    pub etag: Option<String>,
    pub source: Option<DetailSource>,
}

#[derive(Debug, Clone)]
pub struct DetailPage {
    pub body: DetailValue,
    pub metadata: Option<crate::ResourceMetadataObservation>,
    pub entries: Vec<DetailEntry>,
    pub source: DetailSource,
    pub next_cursor: Option<String>,
    pub etag: Option<String>,
    pub not_modified: bool,
    pub freshness_seconds: u32,
    pub cooldown_seconds: Option<u64>,
}

/// Native receipts capture the trusted installation and same-notification parent.
/// No command accepts a renderer-authored URL or selector as HTTP authority.
#[derive(Debug, Clone)]
pub struct TrustedNotificationSubjectRequest {
    pub account: RemoteAccount,
    pub instance_id: String,
    pub notification_id: String,
    pub selector_generation: String,
    pub authorization_view: String,
    pub repository: RemoteRepository,
    pub selector: crate::NotificationSubjectSelector,
}

#[derive(Debug, Clone)]
pub enum NotificationSubjectDiscovery {
    Failed {
        error: ProviderError,
        cooldown_seconds: Option<u64>,
    },
    Verified {
        subject: Box<RemoteItem>,
        detail: Box<DetailPage>,
        endpoint_aliases: Vec<EndpointAlias>,
    },
    Unresolved {
        reason: crate::NotificationSubjectReason,
        cooldown_seconds: Option<u64>,
    },
}

/// A failed prospective credential can carry quota evidence only after the
/// adapter has proved its immutable actor. It is never authorization evidence.
#[derive(Debug, Clone)]
pub struct ProbeFailure {
    pub error: ProviderError,
    pub verified_actor_id: Option<String>,
}

/// Read-only contract for the first vertical slice. Writes must eventually use
/// operation-specific durable outbox delivery; adapters do not offer raw HTTP.
#[async_trait]
pub trait CollaborationProvider: Send + Sync + 'static {
    fn kind(&self) -> ProviderKind;
    /// The adapter is constructed for one explicitly trusted installation.
    fn instance(&self) -> ProviderInstance {
        ProviderInstance::public(self.kind())
    }
    fn profile(&self, _: &RemoteAccount) -> ProviderProfile {
        // An adapter must declare its own implementation and inbox semantics.
        // The common layer cannot infer GitHub grants for a future provider.
        ProviderProfile {
            facets: vec![],
            inbox_semantics: InboxSemantics::None,
        }
    }
    async fn probe(&self, token: &SecretToken) -> Result<VerifiedAccount, ProviderError>;
    async fn probe_with_backoff(
        &self,
        token: &SecretToken,
    ) -> Result<VerifiedAccount, ProbeFailure> {
        self.probe(token).await.map_err(|error| ProbeFailure {
            error,
            verified_actor_id: None,
        })
    }
    async fn fetch_page(
        &self,
        token: &SecretToken,
        request: FeedRequest,
    ) -> Result<FetchPage, ProviderError>;
    async fn fetch_detail(
        &self,
        _token: &SecretToken,
        _request: DetailRequest,
    ) -> Result<DetailPage, ProviderError> {
        Err(ProviderError {
            kind: ProviderErrorKind::Unsupported,
            retry_after_seconds: None,
            account_cooldown_seconds: None,
        })
    }
    fn notification_subject_support(
        &self,
        _account: &RemoteAccount,
        _kind: crate::NotificationSubjectKind,
    ) -> CapabilityState {
        CapabilityState::Unsupported
    }
    async fn discover_notification_subject(
        &self,
        _token: &SecretToken,
        _request: TrustedNotificationSubjectRequest,
    ) -> Result<NotificationSubjectDiscovery, ProviderError> {
        Err(ProviderError {
            kind: ProviderErrorKind::Unsupported,
            retry_after_seconds: None,
            account_cooldown_seconds: None,
        })
    }
}

impl From<ProviderError> for CollaborationError {
    fn from(error: ProviderError) -> Self {
        let code = match error.kind {
            ProviderErrorKind::Authentication => ErrorCode::AuthRequired,
            ProviderErrorKind::Permission => ErrorCode::PermissionDenied,
            ProviderErrorKind::NotFound => ErrorCode::NotFound,
            ProviderErrorKind::RateLimited => ErrorCode::RateLimited,
            ProviderErrorKind::Offline => ErrorCode::Network,
            ProviderErrorKind::Unsupported => ErrorCode::Unsupported,
            ProviderErrorKind::Unavailable | ProviderErrorKind::InvalidResponse => {
                ErrorCode::Provider
            }
        };
        Self {
            code,
            message: error.to_string(),
            retry_after_seconds: error
                .retry_after_seconds
                .map(|n| n.min(u32::MAX as u64) as u32),
        }
    }
}
