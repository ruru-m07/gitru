use crate::models::commit::CommitInfo;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Branch {
    pub name: String,
    pub display_name: String,
    pub is_remote: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct BranchInfo {
    pub name: String,
    pub display_name: String,
    pub is_remote: bool,
    pub is_head: bool,
    pub commit: CommitInfo,
    pub upstream: Option<String>,
    pub ahead: Option<usize>,
    pub behind: Option<usize>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AheadBehindStatus {
    pub ahead: usize,
    pub behind: usize,
    pub local_branch: String,
    pub local_branch_id: String,
    pub upstream_branch: Option<String>,
    pub upstream_branch_id: Option<String>,
    pub is_published: bool,
}

// Re-export BranchStash from stash module for backward compatibility.
pub use crate::models::stash::BranchStash;

#[derive(Debug, Deserialize)]
pub enum BranchKind {
    Local,
    Remote,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub enum UncommittedChangesStrategy {
    StashOnCurrentBranch,
    BringChanges,
}
