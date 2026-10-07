//! Native local-Git pull-file diff facts.
//!
//! These models describe an optional accelerator. Provider metadata remains
//! authoritative for the pull request and the selected `(old_path, new_path)`
//! identity.

use serde::{Deserialize, Serialize};

pub const MAX_LOCAL_PULL_FILE_PATH_BYTES: usize = 4_096;
pub const MAX_LOCAL_PULL_FILE_TEXT_BYTES: usize = 4 * 1_048_576;
pub const MAX_LOCAL_PULL_FILE_TEXT_LINES: usize = 50_000;
pub const MAX_LOCAL_PULL_FILE_LINE_BYTES: usize = 256 * 1_024;
/// Refuse a single selected blob before Git begins rename detection or diffing.
/// The local path is an optional accelerator, so bounded refusal is preferable
/// to letting an untrusted repository make the Git child consume unbounded RAM.
pub const MAX_LOCAL_PULL_FILE_INPUT_BLOB_BYTES: u64 = 8 * 1_048_576;
/// Both sides of a selected change share one bounded native diff budget.
pub const MAX_LOCAL_PULL_FILE_COMBINED_BLOB_BYTES: u64 = 12 * 1_048_576;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalPullFileDiffRequest {
    /// Current target-branch tip observed by the provider.
    pub base_oid: String,
    /// Exact source-branch tip observed by the provider.
    pub head_oid: String,
    /// Provider-observed merge base when that provider exposes one.
    pub merge_base_oid: Option<String>,
    /// Exact provider-selected source path for a delete/rename/copy.
    pub old_path: Option<String>,
    /// Exact provider-selected destination path for an add/rename/copy.
    pub new_path: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LocalPullFileComparison {
    MergeBaseToHead,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalPullFileDiffProvenance {
    pub comparison: LocalPullFileComparison,
    pub base_oid: String,
    pub head_oid: String,
    pub resolved_merge_base_oid: String,
    pub known_merge_base_oid: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LocalPullFileDiffUnsupportedReason {
    NoCommonAncestor,
    UnsupportedChangeKind,
    NonUtf8Text,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LocalPullFileDiffUnavailableReason {
    BaseObjectMissing,
    HeadObjectMissing,
    AmbiguousMergeBase,
    KnownMergeBaseMismatch,
    ChangeIdentityMismatch,
    SelectedObjectMissing,
    MetadataLimitExceeded,
    MalformedGitOutput,
    GitUnavailable,
    TimedOut,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum LocalPullFileDiffState {
    Text {
        unified_diff: String,
    },
    Binary,
    Oversized,
    Unsupported {
        reason: LocalPullFileDiffUnsupportedReason,
    },
    Unavailable {
        reason: LocalPullFileDiffUnavailableReason,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalPullFileDiff {
    /// Present only after Git resolves one unique merge base. Historical or
    /// caller-supplied branch tips are never reported as that resolved base.
    pub provenance: Option<LocalPullFileDiffProvenance>,
    pub state: LocalPullFileDiffState,
}
