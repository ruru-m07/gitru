use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize)]
pub enum UncommittedChangesStrategy {
    /// Stash changes on the current branch before switching (can be recovered later)
    StashOnCurrentBranch,
    /// Bring uncommitted changes to the new branch
    BringChanges,
}

#[derive(Debug, Serialize)]
pub struct FileVersion {
    pub content: String,
    pub encoding: Option<String>,
    pub is_binary: bool,
    pub byte_length: usize,
}

#[derive(Debug, Serialize)]
pub struct GetDiffResponse {
    pub file_path: String,
    pub head: Option<FileVersion>,
    pub workdir: Option<FileVersion>,
    pub patch: Option<String>,
}

#[derive(Debug, Serialize)]
pub enum FileStatusKind {
    IndexNew,
    IndexModified,
    IndexDeleted,
    IndexRenamed,
    IndexTypechange,
    WorktreeNew,
    WorktreeModified,
    WorktreeDeleted,
    WorktreeRenamed,
    WorktreeTypechange,
    WorktreeUnreadable,
    Conflicted,
}

#[derive(Debug, Serialize)]
pub struct FileStatus {
    pub path: String,
    pub new_path: Option<String>,
    pub status: Vec<FileStatusKind>,
}

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
pub struct GetStatusResponse {
    pub files: Vec<FileStatus>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Author {
    pub name: String,
    pub email: String,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct CommitAuthors {
    pub author: Author,
    pub committer: Author,
    pub co_authors: Vec<Author>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct CommitInfo {
    pub id: String,
    pub summary: String,
    pub body: String,
    pub timestamp: i64,
    pub authors: CommitAuthors,
}

// full commit
#[derive(Serialize, Deserialize)]
pub struct CommitStats {
    pub insertions: usize,
    pub deletions: usize,
    pub files_changed: usize,
}

#[derive(Serialize, Deserialize)]
pub struct FullCommitInfo {
    pub id: String,
    pub timestamp: i64,
    pub summary: String,
    pub body: String,
    pub authors: CommitAuthors,
    pub stats: CommitStats,
}

#[derive(Serialize, Deserialize)]
pub struct Branch {
    pub name: String,
    pub display_name: String,
    pub is_remote: bool,
}

#[derive(Serialize, Deserialize, Clone)]
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

#[derive(Serialize, Deserialize, Clone)]
pub enum GraphRowType {
    Commit,
    Stash,
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
pub enum GraphRefType {
    Local,
    Remote,
    Tag,
    Stash,
    Other,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct GraphRef {
    pub name: String,
    pub kind: GraphRefType,
    pub is_head: bool,
}

#[derive(Serialize, Deserialize)]
pub struct GraphRow {
    pub oid: String,
    pub lane: usize,
    pub commit: FullCommitInfo,
    pub parents: Vec<ParentEdge>,
    #[serde(rename = "type")]
    pub r#type: GraphRowType,
    pub refs: Vec<GraphRef>,
    pub heads: Vec<GraphRef>,
    pub remotes: Vec<GraphRef>,
    pub tags: Vec<GraphRef>,
    pub stashes: Vec<GraphRef>,
    /// Swimlanes entering this row from above (before processing).
    pub input_swimlanes: Vec<Swimlane>,
    /// Swimlanes leaving this row going below (after processing).
    pub output_swimlanes: Vec<Swimlane>,
}

/// A single graph swimlane (column) carrying a branch flow.
#[derive(Serialize, Deserialize, Clone)]
pub struct Swimlane {
    /// The commit OID this lane is heading toward.
    pub id: String,
    /// Color index (rotated through a palette on the frontend).
    pub color: usize,
    /// Whether this lane is carrying a stash flow (should be dashed).
    pub is_stash: bool,
}

#[derive(Serialize, Deserialize)]
pub struct ParentEdge {
    pub oid: String,
    pub lane: usize,
}

#[derive(Serialize, Deserialize)]
pub struct HistoryGraphResponse {
    pub rows: Vec<GraphRow>,
    pub cursor: Option<String>,
    pub graph_state: Option<String>,
    pub has_more: bool,
    pub paging: GraphPaging,
}

#[derive(Serialize, Deserialize)]
pub struct GraphPaging {
    pub starting_cursor: Option<String>,
    pub has_more: bool,
}
