use git2::{Repository, Status, StatusOptions};

use crate::types::{FileStatus, FileStatusKind};

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
            let s = entry.status();
            let status = human_readable_status(s);

            if status.len() == 0 {
                return None;
            }

            // For renamed files, get both old and new paths
            let (path, new_path) = if s.is_index_renamed() {
                // Index rename: HEAD -> Index
                if let Some(diff) = entry.head_to_index() {
                    let old_path = diff.old_file().path().map(|p| p.to_string_lossy().into());
                    let new_path = diff.new_file().path().map(|p| p.to_string_lossy().into());
                    (old_path?, new_path)
                } else {
                    (entry.path()?.into(), None)
                }
            } else if s.is_wt_renamed() {
                // Working tree rename: Index -> Workdir
                if let Some(diff) = entry.index_to_workdir() {
                    let old_path = diff.old_file().path().map(|p| p.to_string_lossy().into());
                    let new_path = diff.new_file().path().map(|p| p.to_string_lossy().into());
                    (old_path?, new_path)
                } else {
                    (entry.path()?.into(), None)
                }
            } else {
                (entry.path()?.into(), None)
            };

            Some(FileStatus {
                path,
                new_path,
                status,
            })
        })
        .collect();

    Ok(result)
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
        // ! nothing
    }
    parts
}
