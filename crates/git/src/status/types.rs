use std::path::PathBuf;

use serde::Serialize;

#[derive(Debug, Serialize)]
// #[serde(rename_all = "SCREAMING_SNAKE_CASE")]
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
    pub path: PathBuf,
    pub status: Vec<FileStatusKind>,
}

#[derive(Serialize)]
pub struct GetStatusResponse {
    pub files: Vec<FileStatus>,
}
