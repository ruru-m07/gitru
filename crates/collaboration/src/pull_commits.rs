//! Typed pull-request commit observations and exact local-cache bindings.
use crate::{
    Coverage, DetailActor, DetailFreshness, RemoteAccount, RemoteItem, RemoteRepository, SyncStatus,
};
use serde::{Deserialize, Serialize};

const PULL_COMMIT_DRIFT: &str = "Pull commit traversal representation changed";

pub const MAX_PULL_COMMITS: u32 = 500;
pub const MAX_PULL_COMMIT_PAGES: u32 = 20;
pub const MAX_PULL_COMMITS_PER_PROVIDER_PAGE: usize = 100;
pub const MAX_PULL_COMMITS_PER_LOCAL_PAGE: u32 = 100;
pub const MAX_PULL_COMMIT_LOCAL_PAGE_BYTES: usize = 1_048_576;
pub const MAX_PULL_COMMIT_MESSAGE_BYTES: usize = 65_536;
pub const MAX_PULL_COMMIT_PARENTS: usize = 64;

/// Exact pull-detail facts that authorize one commit-list generation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PullCommitContext {
    pub base_oid: String,
    pub head_oid: String,
    pub source_repository_provider_id: String,
    pub metadata_facet_revision: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PullCommitMessageState {
    Known,
    Omitted,
    Oversized,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PullCommitMessage {
    pub state: PullCommitMessageState,
    pub text: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PullCommitActor {
    pub name: String,
    /// Optional provider presentation. `name` remains the authored Git fact.
    pub provider: Option<DetailActor>,
}

/// Provider-normalized commit facts. The engine assigns range positions only
/// when it atomically publishes a terminal generation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderPullCommit {
    pub oid: String,
    pub summary: String,
    pub message: PullCommitMessage,
    pub author: PullCommitActor,
    /// Providers that do not expose the authored Git committer keep this
    /// unknown. Adapters must never infer it from the author presentation.
    pub committer: Option<PullCommitActor>,
    pub authored_at: Option<String>,
    pub committed_at: Option<String>,
    pub parent_oids: Vec<String>,
    pub web_url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PullCommit {
    pub oid: String,
    pub position: u32,
    pub summary: String,
    pub message: PullCommitMessage,
    pub author: PullCommitActor,
    pub committer: Option<PullCommitActor>,
    pub authored_at: Option<String>,
    pub committed_at: Option<String>,
    pub parent_oids: Vec<String>,
    pub web_url: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PullCommitCapReason {
    ProviderLimit,
    LocalLimit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PullCommitCompletenessState {
    Missing,
    Syncing,
    Complete,
    Capped,
    Partial,
}

/// Local publication state for one exact pull-commit context.
///
/// `reason` is present exactly when `state` is [`PullCommitCompletenessState::Capped`].
/// Use the constructors below when producing values and [`Self::is_valid`] when
/// accepting persisted or otherwise untrusted representations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PullCommitCompleteness {
    pub state: PullCommitCompletenessState,
    pub reason: Option<PullCommitCapReason>,
}

impl PullCommitCompleteness {
    pub const fn missing() -> Self {
        Self {
            state: PullCommitCompletenessState::Missing,
            reason: None,
        }
    }

    pub const fn syncing() -> Self {
        Self {
            state: PullCommitCompletenessState::Syncing,
            reason: None,
        }
    }

    pub const fn complete() -> Self {
        Self {
            state: PullCommitCompletenessState::Complete,
            reason: None,
        }
    }

    pub const fn capped(reason: PullCommitCapReason) -> Self {
        Self {
            state: PullCommitCompletenessState::Capped,
            reason: Some(reason),
        }
    }

    pub const fn partial() -> Self {
        Self {
            state: PullCommitCompletenessState::Partial,
            reason: None,
        }
    }

    pub const fn is_complete(self) -> bool {
        matches!(self.state, PullCommitCompletenessState::Complete)
    }

    pub const fn is_missing(self) -> bool {
        matches!(self.state, PullCommitCompletenessState::Missing)
    }

    pub const fn is_valid(self) -> bool {
        matches!(
            (self.state, self.reason),
            (PullCommitCompletenessState::Capped, Some(_))
                | (
                    PullCommitCompletenessState::Missing
                        | PullCommitCompletenessState::Syncing
                        | PullCommitCompletenessState::Complete
                        | PullCommitCompletenessState::Partial,
                    None
                )
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PullCommitProviderOrder {
    BaseToHead,
    HeadToBase,
}

/// Bounded adapter strategy evidence. It is persisted with the staging
/// generation so pages cannot silently switch endpoints or API semantics.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PullCommitSource {
    pub source: String,
    pub adapter_version: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PullCommitQuery {
    pub account_id: String,
    pub subject_id: String,
    pub cursor: Option<String>,
    pub limit: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PullCommitSnapshot {
    pub subject_id: String,
    pub context: Option<PullCommitContext>,
    pub commits: Vec<PullCommit>,
    pub next_cursor: Option<String>,
    pub completeness: PullCommitCompleteness,
    pub coverage: Coverage,
    pub sync: SyncStatus,
    pub freshness: DetailFreshness,
    pub facet_revision: Option<String>,
    pub revision: String,
    pub authorization_view: String,
}

/// Trusted request assembled from the current account, canonical pull and
/// exact Body metadata. Provider adapters must not derive a looser range.
#[derive(Debug, Clone)]
pub struct PullCommitRequest {
    pub account: RemoteAccount,
    pub repository: RemoteRepository,
    pub subject: RemoteItem,
    pub context: PullCommitContext,
    pub cursor: Option<String>,
    /// Expected zero-based position in the provider's declared traversal order.
    pub start_position: u32,
}

#[derive(Debug, Clone)]
pub struct PullCommitProviderPage {
    /// Echoed exact context; a mismatch is provider drift.
    pub context: PullCommitContext,
    pub commits: Vec<ProviderPullCommit>,
    pub order: PullCommitProviderOrder,
    pub source: PullCommitSource,
    /// Position of the first row in provider traversal order.
    pub start_position: u32,
    pub next_cursor: Option<String>,
    /// A terminal provider limit is explicit even when no continuation exists.
    pub cap_reason: Option<PullCommitCapReason>,
    pub remote_has_more: bool,
    pub freshness_seconds: u32,
    pub cooldown_seconds: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PullCommitBinding {
    pub repository_id: String,
    pub repository_provider_id: String,
    pub provider_id: String,
    pub number: Option<String>,
    pub context: PullCommitContext,
}

/// Fresh provider-backed parent facts checked immediately before a terminal
/// commit generation is eligible for publication.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PullCommitRangeValidation {
    pub base_oid: String,
    pub head_oid: String,
    pub base_repository_provider_id: String,
    pub source_repository_provider_id: String,
}

#[derive(Debug, Clone)]
pub struct PullCommitLease {
    pub run_id: String,
    pub generation: String,
    pub authorization_view: String,
    pub instance_id: String,
    pub binding: PullCommitBinding,
    pub next_cursor: Option<String>,
    pub page_count: u32,
    pub row_count: u32,
}

/// Native-only page transaction. Renderer input can never publish provider data.
#[derive(Debug, Clone)]
pub struct PullCommitCommit {
    pub account_id: String,
    pub authorization_epoch: String,
    pub lease: PullCommitLease,
    pub request_cursor: Option<String>,
    pub page: PullCommitProviderPage,
    pub terminal_validation: Option<PullCommitRangeValidation>,
}

#[derive(Debug, Clone)]
pub struct PullCommitApplyReceipt {
    pub revision: String,
    pub next_cursor: Option<String>,
    pub published: bool,
    pub row_count: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PullCommitMembershipRequest {
    pub account_id: String,
    pub authorization_epoch: String,
    pub subject_id: String,
    pub commit_oid: String,
    pub facet_revision: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PullCommitMembershipReceipt {
    pub repository_id: String,
    pub instance_id: String,
    pub authorization_view: String,
    pub subject_id: String,
    pub commit_oid: String,
    pub active_generation: String,
    pub facet_revision: String,
    pub context: PullCommitContext,
}

pub fn is_canonical_commit_oid(value: &str) -> bool {
    matches!(value.len(), 40 | 64)
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

pub(crate) fn pull_commit_drift() -> crate::CollaborationError {
    crate::CollaborationError::new(crate::ErrorCode::StaleView, PULL_COMMIT_DRIFT)
}

pub(crate) fn is_pull_commit_drift(error: &crate::CollaborationError) -> bool {
    error.code == crate::ErrorCode::StaleView && error.message == PULL_COMMIT_DRIFT
}
