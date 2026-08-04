use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum RebaseAction {
    Pick,
    Reword,
    Edit,
    Squash,
    Fixup,
    Drop,
}

impl RebaseAction {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pick => "pick",
            Self::Reword => "reword",
            Self::Edit => "edit",
            Self::Squash => "squash",
            Self::Fixup => "fixup",
            Self::Drop => "drop",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "pick" | "p" => Some(Self::Pick),
            "reword" | "r" => Some(Self::Reword),
            "edit" | "e" => Some(Self::Edit),
            "squash" | "s" => Some(Self::Squash),
            "fixup" | "f" => Some(Self::Fixup),
            "drop" | "d" => Some(Self::Drop),
            _ => None,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum RebaseTodoStatus {
    Pending,
    Current,
    Done,
    Skipped,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum RebasePauseReason {
    Conflict,
    Edit,
    Reword,
    /// Sequencer is idle between steps / waiting for user continue after edit.
    Waiting,
}

impl RebasePauseReason {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Conflict => "conflict",
            Self::Edit => "edit",
            Self::Reword => "reword",
            Self::Waiting => "waiting",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "conflict" => Some(Self::Conflict),
            "edit" => Some(Self::Edit),
            "reword" => Some(Self::Reword),
            "waiting" => Some(Self::Waiting),
            _ => None,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RebaseTodoEntry {
    pub index: u32,
    pub action: RebaseAction,
    pub commit: String,
    pub short_commit: String,
    pub message: String,
    pub status: RebaseTodoStatus,
    /// Author date as unix seconds (string) when resolvable from the commit object.
    pub authored_at: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RebasePlanEntry {
    pub action: RebaseAction,
    pub commit: String,
    pub message: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RebasePlan {
    /// Upstream / onto commit or ref.
    pub onto: String,
    /// Optional upstream tip used to compute the range (defaults to onto).
    pub upstream: Option<String>,
    pub entries: Vec<RebasePlanEntry>,
    /// When true, stash dirty worktree before starting.
    pub autostash: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RebaseStartRequest {
    /// Onto commit-ish (branch, tag, or oid).
    pub onto: String,
    /// Optional upstream for range computation. Defaults to `onto`.
    pub upstream: Option<String>,
    /// When set, use these interactive actions instead of plain pick-all.
    pub entries: Option<Vec<RebasePlanEntry>>,
    pub autostash: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RebaseUpdateTodoRequest {
    pub entries: Vec<RebasePlanEntry>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ConflictResolveRequest {
    pub path: String,
    pub strategy: ConflictResolveStrategy,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ConflictResolveStrategy {
    Ours,
    Theirs,
    /// Keep both sides concatenated (union-style).
    Union,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RebaseAbortPreview {
    pub orig_head: Option<String>,
    pub head_name: Option<String>,
    pub onto: Option<String>,
    pub current: Option<u32>,
    pub total: Option<u32>,
    pub commits_applied: u32,
    pub warning: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RebaseProgressEvent {
    pub phase: RebaseProgressPhase,
    pub current: Option<u32>,
    pub total: Option<u32>,
    pub message: Option<String>,
    pub commit: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub enum RebaseProgressPhase {
    Preparing,
    Started,
    Applying,
    Paused,
    Finished,
    Error,
    Aborted,
}
