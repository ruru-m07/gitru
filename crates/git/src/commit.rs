use git2::{DiffOptions, Oid, Repository};

use crate::{
    history::extract_all_authors,
    types::{CommitInfo, CommitStats, FullCommitInfo},
};

#[tauri::command]
pub fn last_commit(repo_path: &str) -> Result<CommitInfo, String> {
    let repo = Repository::open(repo_path).map_err(|e| e.to_string())?;

    let head = repo.head().map_err(|e| e.to_string())?;
    let commit = head.peel_to_commit().map_err(|e| e.to_string())?;

    println!("{:?}", commit.summary().unwrap_or("").to_string());

    Ok(CommitInfo {
        id: commit.id().to_string(),
        timestamp: commit.time().seconds(),
        summary: commit.summary().unwrap_or("").to_string(),
        body: commit.body().unwrap_or("").to_string(),
        authors: extract_all_authors(&commit),
    })
}

#[tauri::command]
pub fn commit_by_id(repo_path: &str, hash: &str) -> Result<FullCommitInfo, String> {
    let repo = Repository::open(repo_path).map_err(|e| e.to_string())?;

    let oid = Oid::from_str(hash).map_err(|e| e.to_string())?;
    let commit = repo.find_commit(oid).map_err(|e| e.to_string())?;

    let commit_tree = commit.tree().map_err(|e| e.to_string())?;

    let parent_tree = if commit.parent_count() > 0 {
        Some(
            commit
                .parent(0)
                .map_err(|e| e.to_string())?
                .tree()
                .map_err(|e| e.to_string())?,
        )
    } else {
        None
    };

    let diff = repo
        .diff_tree_to_tree(
            parent_tree.as_ref(),
            Some(&commit_tree),
            Some(&mut DiffOptions::new()),
        )
        .map_err(|e| e.to_string())?;

    let stats = diff.stats().map_err(|e| e.to_string())?;

    let commit_stats = CommitStats {
        insertions: stats.insertions() as usize,
        deletions: stats.deletions() as usize,
        files_changed: stats.files_changed() as usize,
    };

    Ok(FullCommitInfo {
        id: commit.id().to_string(),
        timestamp: commit.time().seconds(),
        summary: commit.summary().unwrap_or("").to_string(),
        body: commit.body().unwrap_or("").to_string(),
        authors: extract_all_authors(&commit),
        stats: commit_stats,
    })
}
