//! Adapters normalize provider resources; scheduling and persistence stay outside.

#[cfg(test)]
mod contract_tests;
pub mod github;
mod registry;
mod transport;

pub use registry::{ProviderProfile, ProviderRegistry};
pub use transport::{ProviderError, ProviderErrorKind};

use crate::{CollaborationError, ErrorCode, credentials::SecretToken, domain::*};
use async_trait::async_trait;

#[derive(Debug, Clone)]
pub struct VerifiedAccount {
    pub actor_id: String,
    pub login: String,
    pub display_name: Option<String>,
    pub notifications_supported: bool,
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
    pub next_cursor: Option<String>,
    pub etag: Option<String>,
    pub last_modified: Option<String>,
    pub not_modified: bool,
    pub poll_interval_seconds: Option<u64>,
    pub cooldown_seconds: Option<u64>,
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
    fn profile(&self, account: &RemoteAccount) -> ProviderProfile {
        ProviderProfile::read_only(
            InboxSemantics::NativeNotifications,
            account.notifications_supported,
        )
    }
    async fn probe(&self, token: &SecretToken) -> Result<VerifiedAccount, ProviderError>;
    async fn fetch_page(
        &self,
        token: &SecretToken,
        request: FeedRequest,
    ) -> Result<FetchPage, ProviderError>;
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
