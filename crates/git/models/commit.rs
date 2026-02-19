use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Author {
    pub name: String,
    pub email: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CommitAuthors {
    pub author: Author,
    pub committer: Author,
    pub co_authors: Vec<Author>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CommitInfo {
    pub id: String,
    pub summary: String,
    pub body: String,
    pub timestamp: i64,
    pub authors: CommitAuthors,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CommitStats {
    pub insertions: usize,
    pub deletions: usize,
    pub files_changed: usize,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FullCommitInfo {
    pub id: String,
    pub timestamp: i64,
    pub summary: String,
    pub body: String,
    pub authors: CommitAuthors,
    pub stats: CommitStats,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CommitMessage {
    pub title: String,
    pub description: Option<String>,
    pub co_authors: Vec<(String, String)>,
}
