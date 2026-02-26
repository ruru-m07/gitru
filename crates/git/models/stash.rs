use serde::{Deserialize, Serialize};

use crate::models::status::FileStatus;

/// A single entry in the stash list.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct StashEntry {
    /// The stash index (e.g. 0 for stash@{0}).
    pub index: usize,
    /// The full stash reference (e.g. "stash@{0}").
    pub reference: String,
    /// The stash message.
    pub message: String,
    /// The branch it was created on (parsed from "On <branch>: …" when available).
    pub branch: Option<String>,
    /// Whether this is a Gitru-managed stash (message matches `!!Gitru<…>`).
    pub is_gitru: bool,
}

/// Quick numeric stat for a stash (from `git stash show --stat`).
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct StashQuickStat {
    pub reference: String,
    pub files_changed: usize,
    pub insertions: usize,
    pub deletions: usize,
}

/// Full stash detail: quick stat + per-file status.
#[derive(Debug, Serialize, Clone)]
pub struct StashShowResponse {
    pub reference: String,
    pub stat: StashQuickStat,
    pub files: Vec<FileStatus>,
}

/// Gitru-managed stash created during branch switching.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct BranchStash {
    pub reference: String,
    pub message: String,
    pub from_branch: String,
    pub to_branch: String,
    pub files_changed: usize,
    pub insertions: usize,
    pub deletions: usize,
}
