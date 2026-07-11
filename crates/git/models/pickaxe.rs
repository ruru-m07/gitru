use crate::models::commit::FullCommitInfo;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PickaxeQuery {
    pub query: String,
    pub is_regex: bool,
    pub match_case: bool,
    pub match_whole_word: bool,
    pub author: Option<String>,
    pub since: Option<String>,
    pub until: Option<String>,
    pub file_patterns: Vec<String>,
    pub limit: Option<usize>,
    pub operation_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PickaxeHit {
    pub commit: FullCommitInfo,
    pub commit_hash: String,
    pub commit_subject: String,
    pub author_name: String,
    pub author_email: String,
    pub commit_time: i64,
    pub file_path: String,
    pub file_new_path: Option<String>,
    pub match_line: Option<u32>,
    pub patch: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PickaxePhase {
    Started,
    Hit,
    Progress,
    Finished,
    Error,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PickaxeProgressEvent {
    pub operation_id: String,
    pub phase: PickaxePhase,
    pub hit: Option<PickaxeHit>,
    pub commits_scanned: u32,
    pub hits_found: u32,
    pub status: Option<String>,
    pub error: Option<String>,
}
