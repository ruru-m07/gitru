use serde::{Deserialize, Serialize};

use crate::models::rebase::{RebasePauseReason, RebaseTodoEntry};

/// High-level in-progress git operation detected from repository state.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum RepoOperationKind {
    Clean,
    Merge,
    Revert,
    CherryPick,
    Bisect,
    Rebase,
    RebaseInteractive,
    RebaseMerge,
    ApplyMailbox,
    Other,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum RebaseEngine {
    /// Native git rebase (merge / apply / interactive from CLI).
    Git,
    /// Gitru-owned interactive sequencer persisted under `.git/gitru-rebase/`.
    Gitru,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RepoOperation {
    pub kind: RepoOperationKind,
    /// True when the repo is in any rebase-related state (including Gitru sequencer).
    pub is_rebasing: bool,
    pub engine: Option<RebaseEngine>,
    /// Branch or detached HEAD name being rebased (e.g. `refs/heads/feature`).
    pub head_name: Option<String>,
    /// Onto commit short/full oid when known.
    pub onto: Option<String>,
    /// Short oid of the commit currently being applied (paused at).
    pub paused_at: Option<String>,
    pub pause_reason: Option<RebasePauseReason>,
    /// 1-based index of the current step (when known).
    pub current: Option<u32>,
    /// Total steps in the rebase todo.
    pub total: Option<u32>,
    pub remaining: Option<u32>,
    /// Human label like `feature onto abc1234`.
    pub label: Option<String>,
    /// Original HEAD before rebase started.
    pub orig_head: Option<String>,
    /// Prefill for continue/reword — from rebase message file or current commit.
    pub commit_message: Option<String>,
    /// Todo entries (done + remaining) when available.
    pub todo: Vec<RebaseTodoEntry>,
    /// Conflicted paths detected from the index / status.
    pub conflict_paths: Vec<String>,
}

impl RepoOperation {
    pub fn clean() -> Self {
        Self {
            kind: RepoOperationKind::Clean,
            is_rebasing: false,
            engine: None,
            head_name: None,
            onto: None,
            paused_at: None,
            pause_reason: None,
            current: None,
            total: None,
            remaining: None,
            label: None,
            orig_head: None,
            commit_message: None,
            todo: Vec::new(),
            conflict_paths: Vec::new(),
        }
    }
}
