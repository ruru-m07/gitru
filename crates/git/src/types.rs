use serde::Serialize;

#[derive(Serialize)]
pub struct GitResult {
    pub success: bool,
    pub message: Option<String>,
}

impl GitResult {
    pub fn success() -> Self {
        Self {
            success: true,
            message: None,
        }
    }

    pub fn error(msg: impl Into<String>) -> Self {
        Self {
            success: false,
            message: Some(msg.into()),
        }
    }
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

#[derive(Serialize, Clone, Debug)]
pub struct Author {
    pub name: String,
    pub email: String,
}

#[derive(Serialize, Clone)]
pub struct CommitAuthors {
    pub author: Author,
    pub committer: Author,
    pub co_authors: Vec<Author>,
}

#[derive(Serialize, Clone)]
pub struct CommitInfo {
    pub id: String,
    pub summary: String,
    pub body: String,
    pub timestamp: i64,
    pub authors: CommitAuthors,
}

// full commit
#[derive(Serialize)]
pub struct CommitStats {
    pub insertions: usize,
    pub deletions: usize,
    pub files_changed: usize,
}

#[derive(Serialize)]
pub struct FullCommitInfo {
    pub id: String,
    pub timestamp: i64,
    pub summary: String,
    pub body: String,
    pub authors: CommitAuthors,
    pub stats: CommitStats,
}

/// just generating return types
#[tauri::command(rename_all = "snake_case")]
fn _generate() -> FileStatus {
    todo!()
}
