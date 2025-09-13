use git2::{Repository, Status, StatusOptions};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct FileStatus {
    pub path: String,
    pub status: String,
}

#[derive(Serialize)]
pub struct GetStatusResponse {
    pub files: Vec<FileStatus>,
}

#[tauri::command(rename_all = "snake_case")]
pub fn get_status(repo_path: String) -> Result<GetStatusResponse, String> {
    // Open the repository
    let repo = Repository::open(&repo_path).map_err(|e| format!("Failed to open repo: {}", e))?;

    // Configure status options (similar to `git status`)
    let mut opts = StatusOptions::new();
    opts.include_untracked(true)
        .recurse_untracked_dirs(true)
        .renames_head_to_index(true)
        .renames_index_to_workdir(true);

    let statuses = repo
        .statuses(Some(&mut opts))
        .map_err(|e| format!("Failed to get statuses: {}", e))?;

    // Map each status entry to our struct
    let result = statuses
        .iter()
        .filter_map(|entry| {
            let path = entry.path()?.to_string();
            let s = entry.status();
            Some(FileStatus {
                path,
                status: human_readable_status(s),
            })
        })
        .collect();

    Ok(GetStatusResponse { files: result })
}

#[tauri::command(rename_all = "snake_case")]
pub async fn get_status_single_file(
    repo_path: String,
    file_path: String,
) -> Result<FileStatus, String> {
    // Open the repository
    let repo = Repository::open(&repo_path).map_err(|e| format!("Failed to open repo: {}", e))?;

    // Configure status options
    let mut opts = StatusOptions::new();
    opts.include_untracked(true)
        .recurse_untracked_dirs(true)
        .renames_head_to_index(true)
        .renames_index_to_workdir(true)
        .pathspec(&file_path); // Only check this file

    let statuses = repo
        .statuses(Some(&mut opts))
        .map_err(|e| format!("Failed to get status: {}", e))?;

    // Find the status entry for the file
    let file_status = statuses
        .iter()
        .find_map(|entry| {
            let path = entry.path()?.to_string();
            if path == file_path {
                let s = entry.status();
                Some(FileStatus {
                    path,
                    status: human_readable_status(s),
                })
            } else {
                None
            }
        })
        .ok_or_else(|| "File not found in repository status".to_string())?;

    Ok(file_status)
}

fn human_readable_status(status: Status) -> String {
    let mut parts = Vec::new();
    if status.contains(Status::INDEX_NEW) {
        parts.push("INDEX_NEW");
    }
    if status.contains(Status::INDEX_MODIFIED) {
        parts.push("INDEX_MODIFIED");
    }
    if status.contains(Status::INDEX_DELETED) {
        parts.push("INDEX_DELETED");
    }
    if status.contains(Status::WT_NEW) {
        parts.push("WT_NEW");
    }
    if status.contains(Status::WT_MODIFIED) {
        parts.push("WT_MODIFIED");
    }
    if status.contains(Status::WT_DELETED) {
        parts.push("WT_DELETED");
    }
    if parts.is_empty() {
        parts.push("CLEAN");
    }
    parts.join(" | ")
}
