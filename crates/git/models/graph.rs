use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GraphRefKind {
	Local,
	Remote,
	Tag,
	Stash,
	Other,
}

#[derive(Debug, Clone)]
pub struct GraphRef {
	pub name: String,
	pub kind: GraphRefKind,
	pub is_head: bool,
}

#[derive(Debug, Clone)]
pub struct GraphLogEntry {
	pub id: String,
	pub parents: Vec<String>,
	pub author_name: String,
	pub author_email: String,
	pub author_time: i64,
	pub committer_name: String,
	pub committer_email: String,
	pub committer_time: i64,
	pub refs: Vec<GraphRef>,
	pub summary: String,
	pub body: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GraphSearchKey {
	Author,
	Committer,
	Message,
	Ref,
	Id,
}

#[derive(Debug, Clone)]
pub struct GraphSearchTerm {
	pub key: Option<GraphSearchKey>,
	pub value: String,
	pub negated: bool,
}

#[derive(Debug, Clone)]
pub struct GraphSearchQuery {
	pub terms: Vec<GraphSearchTerm>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct HistoryQuery {
	pub cursor: Option<String>,
	pub limit: usize,
	pub search: Option<String>,
	pub branch: Option<String>,
	pub graph_state: Option<String>,

	#[serde(default = "default_true")]
	pub include_local: bool,
	#[serde(default)]
	pub include_remotes: bool,
	#[serde(default)]
	pub include_tags: bool,
	#[serde(default)]
	pub include_stash: bool,
}

fn default_true() -> bool {
	true
}

#[derive(Debug, Serialize)]
pub struct SearchResult {
	pub oid: String,
	pub short_oid: String,
	pub message: String,
	pub author: String,
	pub timestamp: i64,
}


