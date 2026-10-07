use crate::models::{operation::RepoOperationKind, remotes::RemoteEndpoint};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PullCheckoutTarget {
    pub remote_name: String,
    pub remote_ordinal: u32,
    pub remote_endpoint: RemoteEndpoint,
    pub remote_digest: String,
    pub source_branch: String,
    pub expected_oid: String,
    pub local_branch: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PullCheckoutAction {
    AlreadyCheckedOut,
    SwitchExisting,
    CreateBranch,
    FetchAndCreateBranch,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PullCheckoutBlocker {
    DirtyWorktree,
    ActiveOperation,
    ExistingBranchDiverged,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PullCheckoutInspection {
    pub current_branch: Option<String>,
    pub current_head_oid: String,
    pub detached: bool,
    pub dirty: bool,
    pub operation: RepoOperationKind,
    pub target_branch_oid: Option<String>,
    pub object_available: bool,
    pub action: Option<PullCheckoutAction>,
    pub blocker: Option<PullCheckoutBlocker>,
    /// Native-only digest of the credential-free effective fetch URL. It binds
    /// transport details such as an SSH username without crossing IPC.
    #[serde(skip)]
    pub remote_identity: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PullCheckoutReceipt {
    pub branch: String,
    pub oid: String,
    pub fetched: bool,
    /// Git returned a failure after the requested branch and commit became
    /// authoritative. The caller must present this as a completed checkout
    /// with a local Git warning rather than offering the consumed plan again.
    pub git_reported_failure: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PullCheckoutError {
    InvalidTarget,
    UnsupportedCredentials,
    InspectionFailed,
    RemoteChanged,
    StalePlan,
    Blocked,
    FetchFailed,
    HeadMoved,
    CheckoutFailed,
    VerificationFailed,
}

impl std::fmt::Display for PullCheckoutError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::InvalidTarget => "The pull request checkout target is invalid",
            Self::UnsupportedCredentials => {
                "Inline remote credentials are unsupported; configure a credential helper or SSH key"
            }
            Self::InspectionFailed => "The local Git worktree could not be inspected",
            Self::RemoteChanged => "The local Git remotes changed; inspect the checkout again",
            Self::StalePlan => "The local Git worktree changed; inspect the checkout again",
            Self::Blocked => "The local Git worktree is not ready for this checkout",
            Self::FetchFailed => "Git could not fetch the pull request branch",
            Self::HeadMoved => "The pull request head moved; refresh it before checking out",
            Self::CheckoutFailed => "Git could not check out the pull request branch",
            Self::VerificationFailed => "Git could not verify the checked out branch",
        })
    }
}

impl std::error::Error for PullCheckoutError {}
