use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
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

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FileStatus {
    pub path: String,
    pub new_path: Option<String>,
    pub status: Vec<FileStatusKind>,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "snake_case")]
pub struct GetStatusResponse {
    pub files: Vec<FileStatus>,
}
