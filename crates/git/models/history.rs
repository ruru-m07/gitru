use crate::models::{commit::FullCommitInfo, graph::GraphRefKind};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum GraphRowType {
    Commit,
    Stash,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GraphRef {
    pub name: String,
    pub display_name: String,
    pub kind: GraphRefKind,
    pub is_head: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ParentEdge {
    pub oid: String,
    pub lane: usize,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Swimlane {
    pub id: String,
    pub color: usize,
    pub is_stash: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GraphRow {
    pub oid: String,
    pub lane: usize,
    pub commit: FullCommitInfo,
    pub parents: Vec<ParentEdge>,
    #[serde(rename = "type")]
    pub r#type: GraphRowType,
    pub refs: Vec<GraphRef>,
    #[serde(default)]
    pub branch_refs: Vec<GraphRef>,
    pub heads: Vec<GraphRef>,
    pub remotes: Vec<GraphRef>,
    pub tags: Vec<GraphRef>,
    pub stashes: Vec<GraphRef>,
    pub input_swimlanes: Vec<Swimlane>,
    pub output_swimlanes: Vec<Swimlane>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GraphPaging {
    pub starting_cursor: Option<String>,
    pub has_more: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct HistoryGraphResponse {
    pub rows: Vec<GraphRow>,
    pub cursor: Option<String>,
    pub graph_state: Option<String>,
    pub has_more: bool,
    pub paging: GraphPaging,
}

/// Lightweight per-commit series for the history overview / minimap chart.
/// One entry per commit in the filtered rev-list order (index 0 = tip under the filter).
/// Designed for efficient transfer and canvas rendering; omits all graph topology,
/// messages, file lists, etc.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CommitOverview {
    /// Total number of commits matching the filter (domain size for the chart).
    pub total: usize,
    /// Index (0-based, in this series) of the current HEAD commit under the filter.
    /// Usually 0 for the default branch view.
    pub head_index: usize,
    /// Insertions per commit, parallel to the logical series (length == total).
    pub insertions: Vec<u32>,
    /// Deletions per commit (length == total).
    pub deletions: Vec<u32>,
    /// Optional: timestamps (author or committer time) for tooltips / richer UX.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timestamps: Option<Vec<i64>>,
    /// Optional: oids (can be full list, sampled, or empty to save payload).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub oids: Option<Vec<String>>,
}
