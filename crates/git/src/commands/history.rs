use crate::{
    types::CommitInfo,
    utils::{extract_all_authors, open_repository},
};

#[tauri::command]
pub fn history(repo_path: &str, skip: usize, limit: usize) -> Result<Vec<CommitInfo>, String> {
    let repo = open_repository(repo_path).map_err(|e| e.to_string())?;

    let mut revwalk = repo.revwalk().map_err(|e| e.to_string())?;

    revwalk.push_head().map_err(|e| e.to_string())?;

    let mut commits = Vec::with_capacity(limit);

    for oid in revwalk.skip(skip).take(limit) {
        let oid = oid.map_err(|e| e.to_string())?;
        let commit = repo.find_commit(oid).map_err(|e| e.to_string())?;

        commits.push(CommitInfo {
            id: oid.to_string(),
            summary: commit.summary().unwrap_or("").to_string(),
            body: commit.body().unwrap_or("").to_string(),
            timestamp: commit.time().seconds(),
            authors: extract_all_authors(&commit),
        });
    }

    Ok(commits)
}
