use serde::Serialize;

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
