use serde::{Deserialize, Serialize};
use crate::models::commit::FullCommitInfo;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum GraphRowType {
    Commit,
    Stash,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GraphRef {
    pub name: String,
    pub kind: crate::models::graph::GraphRefKind,
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
