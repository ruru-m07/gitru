use crate::error::CollaborationError;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderKind {
    Github,
    Gitlab,
    BitbucketCloud,
    BitbucketDc,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AccountState {
    Active,
    AuthRequired,
    Disconnected,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemoteAccount {
    pub id: String,
    pub provider: ProviderKind,
    pub host: String,
    pub actor_id: String,
    pub login: String,
    pub display_name: Option<String>,
    pub authorization_epoch: String,
    pub state: AccountState,
    pub notifications_supported: bool,
}

/// Discovery exposes account metadata only. Credentials remain native.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GithubCliStatus {
    Available,
    NotInstalled,
    Unsupported,
    Unavailable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GithubCliAccountAvailability {
    Ready,
    AuthRequired,
    Unavailable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GithubCliAccount {
    /// Short-lived opaque candidate ID; never a token or executable path.
    pub id: String,
    pub login: String,
    pub host: String,
    pub active: bool,
    pub availability: GithubCliAccountAvailability,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GithubCliDiscovery {
    pub status: GithubCliStatus,
    pub accounts: Vec<GithubCliAccount>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemoteRepository {
    pub id: String,
    pub account_id: String,
    pub provider_id: String,
    pub full_name: String,
    pub name: String,
    pub web_url: String,
    pub description: Option<String>,
    pub default_branch: Option<String>,
    pub selected: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RemoteItemKind {
    PullRequest,
    Issue,
    Notification,
}

/// A list/detail projection. Provider observation and coverage remain separate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemoteItem {
    pub id: String,
    pub account_id: String,
    pub repository_id: Option<String>,
    pub provider_id: String,
    pub kind: RemoteItemKind,
    pub number: Option<String>,
    pub title: String,
    pub body: Option<String>,
    /// The provider body was not cached (e.g. an oversized summary). This is
    /// distinct from an authoritative null body and must not erase detail.
    pub body_omitted: bool,
    pub author: Option<String>,
    pub web_url: Option<String>,
    pub state: String,
    pub updated_at: String,
    pub head_oid: Option<String>,
    pub is_draft: Option<bool>,
    pub reason: Option<String>,
    pub unread: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CoverageState {
    Missing,
    Partial,
    Complete,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Coverage {
    pub state: CoverageState,
    pub validated_at: Option<String>,
    pub remote_has_more: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SyncState {
    Idle,
    Syncing,
    Offline,
    RateLimited,
    AuthRequired,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SyncStatus {
    pub state: SyncState,
    pub last_success_at: Option<String>,
    pub next_retry_at: Option<String>,
    pub error: Option<CollaborationError>,
}

impl Default for SyncStatus {
    fn default() -> Self {
        Self {
            state: SyncState::Idle,
            last_success_at: None,
            next_retry_at: None,
            error: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccountSnapshot {
    pub accounts: Vec<RemoteAccount>,
    pub revision: String,
    pub authorization_view: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepositorySnapshot {
    pub repositories: Vec<RemoteRepository>,
    pub revision: String,
    pub authorization_view: String,
    pub coverage: Coverage,
    pub sync: SyncStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ItemPage {
    pub items: Vec<RemoteItem>,
    pub revision: String,
    pub authorization_view: String,
    pub next_cursor: Option<String>,
    pub coverage: Coverage,
    pub sync: SyncStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ItemSnapshot {
    pub item: Option<RemoteItem>,
    pub revision: String,
    pub authorization_view: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ItemQuery {
    pub account_id: String,
    pub kind: RemoteItemKind,
    pub repository_id: Option<String>,
    pub state: Option<String>,
    pub search: Option<String>,
    pub cursor: Option<String>,
    pub limit: u32,
}

/// Gitru-owned inbox intent. Provider read/done state remains on `RemoteItem`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LocalInboxDisposition {
    Inbox,
    Done,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LocalInboxEffectiveDisposition {
    Inbox,
    Snoozed,
    Done,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LocalInboxFilter {
    Inbox,
    Snoozed,
    Done,
    Bookmarked,
    All,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LocalInboxMutation {
    Disposition,
    Bookmark,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalInboxState {
    pub disposition: LocalInboxDisposition,
    pub effective_disposition: LocalInboxEffectiveDisposition,
    pub bookmarked: bool,
    pub snoozed_until: Option<String>,
    pub activity_updated_at: String,
    pub superseded_by_activity: bool,
    pub generation: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InboxEntry {
    pub item: RemoteItem,
    pub local: LocalInboxState,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InboxQuery {
    pub account_id: String,
    pub remote_state: Option<String>,
    pub local_state: LocalInboxFilter,
    pub search: Option<String>,
    pub cursor: Option<String>,
    pub limit: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InboxPage {
    pub entries: Vec<InboxEntry>,
    pub revision: String,
    pub authorization_view: String,
    pub next_cursor: Option<String>,
    pub coverage: Coverage,
    pub sync: SyncStatus,
    pub evaluated_at: String,
    pub next_local_change_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SetLocalInboxStateRequest {
    pub account_id: String,
    pub authorization_epoch: String,
    pub notification_id: String,
    pub mutation: LocalInboxMutation,
    pub disposition: Option<LocalInboxDisposition>,
    pub bookmarked: Option<bool>,
    pub snoozed_until: Option<String>,
    pub expected_generation: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalInboxWriteReceipt {
    pub state: LocalInboxState,
    pub revision: String,
    pub authorization_view: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CollaborationChange {
    pub revision: String,
    pub account_id: String,
    pub scope: String,
    pub reset: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChangePage {
    pub revision: String,
    pub authorization_view: String,
    pub has_more: bool,
    pub reset_required: bool,
    pub changes: Vec<CollaborationChange>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChangeHint {
    pub revision: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RefreshRequest {
    pub account_id: String,
    pub repository_id: Option<String>,
    pub kind: Option<RemoteItemKind>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RefreshReceipt {
    pub job_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalDraft {
    pub account_id: String,
    pub subject_id: String,
    pub body: String,
    pub generation: String,
}

/// Recovery lists contain only user-authored text and stable subject identities.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DraftSummary {
    pub subject_id: String,
    pub preview: String,
    pub generation: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DraftQuery {
    pub account_id: String,
    pub cursor: Option<String>,
    pub limit: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DraftPage {
    pub drafts: Vec<DraftSummary>,
    pub next_cursor: Option<String>,
}

/// Runtime-only checkpoint/validator state; no token or response body is stored.
#[derive(Debug, Clone)]
pub struct StoredScope {
    pub run_id: String,
    pub next_cursor: Option<String>,
    pub etag: Option<String>,
    pub last_modified: Option<String>,
    pub coverage: Coverage,
    pub sync: SyncStatus,
}

#[derive(Debug, Clone)]
pub struct PageCommit {
    pub account_id: String,
    pub authorization_epoch: String,
    pub scope: String,
    pub run_id: String,
    pub repositories: Vec<RemoteRepository>,
    pub items: Vec<RemoteItem>,
    pub endpoint_aliases: Vec<EndpointAlias>,
    pub next_cursor: Option<String>,
    pub etag: Option<String>,
    pub last_modified: Option<String>,
    pub not_modified: bool,
    pub complete: bool,
    pub observed_at: String,
}

/// An endpoint representation which is not a separate domain entity. A GitHub
/// issue-side PR must wait for its authoritative pull identity before resolving.
#[derive(Debug, Clone)]
pub struct EndpointAlias {
    pub kind: ResourceKind,
    pub repository_provider_id: String,
    pub number: String,
    pub native_identity: String,
    pub web_url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderInstance {
    pub id: String,
    pub provider: ProviderKind,
    pub base_url: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResourceFacet {
    Repositories,
    PullRequests,
    Issues,
    Inbox,
    PullDetails,
    IssueDetails,
    Comments,
    Reviews,
    Checks,
    Participants,
    Tasks,
    PullCommits,
    Merge,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityState {
    Supported,
    Unsupported,
    Unavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityReason {
    NotImplemented,
    ProviderSemantics,
    AdapterUnavailable,
    AuthenticationRequired,
    MissingScope,
    PermissionDenied,
    TemporarilyUnavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InboxSemantics {
    NativeNotifications,
    Todos,
    None,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FacetCapability {
    pub facet: ResourceFacet,
    pub state: CapabilityState,
    pub reason: Option<CapabilityReason>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapabilitySnapshot {
    pub account_id: String,
    pub instance: ProviderInstance,
    pub facets: Vec<FacetCapability>,
    pub inbox_semantics: InboxSemantics,
    pub revision: String,
    pub authorization_view: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResourceKind {
    Repository,
    PullRequest,
    Issue,
    Notification,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LocatorKind {
    Canonical,
    Native,
    RepositoryPath,
    RepositoryNumber,
    WebUrl,
}

/// Structured locators are explicit about actor, installation, resource kind,
/// and mutable presentation. Native values include a representation namespace.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResourceLocator {
    pub instance_id: String,
    pub kind: ResourceKind,
    pub locator_kind: LocatorKind,
    pub value: String,
    pub repository_path: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CanonicalResource {
    pub account_id: String,
    pub instance_id: String,
    pub id: String,
    pub kind: ResourceKind,
    pub provider_id: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResolutionState {
    Resolved,
    Unresolved,
    Ambiguous,
    Unavailable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResourceResolution {
    pub state: ResolutionState,
    pub resource: Option<CanonicalResource>,
    pub candidates: Vec<CanonicalResource>,
    pub revision: String,
    pub authorization_view: String,
}
