use std::path::PathBuf;

use git2::{Repository, Status, StatusOptions};
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
    Clean,
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

#[tauri::command(rename_all = "snake_case")]
pub fn get_status(repo_path: &str) -> Result<GetStatusResponse, String> {
    let mut opts = default_status_options();
    let files = collect_statuses(repo_path, &mut opts)?;

    Ok(GetStatusResponse { files })
}

pub fn collect_statuses(
    repo_path: &str,
    opts: &mut StatusOptions,
) -> Result<Vec<FileStatus>, String> {
    let repo = Repository::open(repo_path).map_err(|e| format!("Failed to open repo: {}", e))?;

    let statuses = repo
        .statuses(Some(opts))
        .map_err(|e| format!("Failed to get statuses: {}", e))?;

    let result = statuses
        .iter()
        .filter_map(|entry| {
            let path = entry.path()?.into();
            let s = entry.status();

            Some(FileStatus {
                path: path,
                status: human_readable_status(s),
            })
        })
        .collect();

    Ok(result)
}

fn default_status_options() -> StatusOptions {
    let mut opts = StatusOptions::new();
    opts.include_ignored(true)
        .include_unmodified(true)
        .include_unreadable(true)
        .include_unreadable_as_untracked(true)
        .include_untracked(true)
        .renames_index_to_workdir(true)
        .renames_head_to_index(true)
        .recurse_untracked_dirs(true);
    opts
}

fn human_readable_status(status: Status) -> Vec<FileStatusKind> {
    let mut parts = Vec::new();
    if status.contains(Status::INDEX_NEW) {
        parts.push(FileStatusKind::IndexNew);
    }
    if status.contains(Status::INDEX_MODIFIED) {
        parts.push(FileStatusKind::IndexModified);
    }
    if status.contains(Status::INDEX_DELETED) {
        parts.push(FileStatusKind::IndexDeleted);
    }
    if status.contains(Status::INDEX_RENAMED) {
        parts.push(FileStatusKind::IndexRenamed);
    }
    if status.contains(Status::INDEX_TYPECHANGE) {
        parts.push(FileStatusKind::IndexTypechange);
    }
    if status.contains(Status::WT_NEW) {
        parts.push(FileStatusKind::WorktreeNew);
    }
    if status.contains(Status::WT_MODIFIED) {
        parts.push(FileStatusKind::WorktreeModified);
    }
    if status.contains(Status::WT_DELETED) {
        parts.push(FileStatusKind::WorktreeDeleted);
    }
    if status.contains(Status::WT_RENAMED) {
        parts.push(FileStatusKind::WorktreeRenamed);
    }
    if status.contains(Status::WT_TYPECHANGE) {
        parts.push(FileStatusKind::WorktreeTypechange);
    }
    if status.contains(Status::WT_UNREADABLE) {
        parts.push(FileStatusKind::WorktreeUnreadable);
    }
    if parts.is_empty() {
        parts.push(FileStatusKind::Clean);
    }
    parts
}

#[tauri::command(rename_all = "snake_case")]
pub fn generate_file_status() -> FileStatus {
    todo!()
}

#[tauri::command(rename_all = "snake_case")]
pub fn generate_file_status_kind() -> FileStatusKind {
    todo!()
}
