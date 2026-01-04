use git2::{DiffOptions, Oid};
use serde::{Deserialize, Serialize};

use crate::{
    types::{CommitInfo, CommitStats, FullCommitInfo},
    utils::{extract_all_authors, open_repository},
};

#[derive(Serialize, Deserialize)]
pub struct CommitMessage {
    title: String,
    description: Option<String>,
    co_authors: Vec<(String, String)>,
}

/* #region // ! commands  */
#[tauri::command]
pub fn last_commit(repo_path: &str) -> Result<CommitInfo, String> {
    let repo = open_repository(repo_path).map_err(|e| e.to_string())?;

    let head = repo.head().map_err(|e| e.to_string())?;
    let commit = head.peel_to_commit().map_err(|e| e.to_string())?;

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
    let repo = open_repository(repo_path).map_err(|e| e.to_string())?;

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

#[tauri::command]
pub fn create_commit(repo_path: &str, commit_meta: CommitMessage) -> Result<String, String> {
    commit_internal(repo_path, &commit_meta, false)
}

#[tauri::command]
pub fn create_empty_commit(repo_path: &str, commit_meta: CommitMessage) -> Result<String, String> {
    commit_internal(repo_path, &commit_meta, true)
}
/* #endregion  // ! commands */

/* #region // ? utils  */
fn commit_internal(
    repo_path: &str,
    commit_meta: &CommitMessage,
    allow_empty: bool,
) -> Result<String, String> {
    let repo = open_repository(repo_path).map_err(|e| e.to_string())?;

    if repo.is_bare() {
        return Err("Cannot commit in a bare repository".into());
    }

    let mut index = repo.index().map_err(|e| e.to_string())?;

    if index.has_conflicts() {
        return Err("Index has conflicts. Resolve them first.".into());
    }

    // ! IMPORTANT:
    // ? We do NOT call add_all here.
    // ? This commits ONLY what is already staged.
    // TODO(ruru): may be in feature - support for patch based commit, which no one ask for
    index.write().map_err(|e| e.to_string())?;

    let parent_commit = repo.head().ok().and_then(|h| h.peel_to_commit().ok());

    let tree_id = index.write_tree().map_err(|e| e.to_string())?;
    let tree = repo.find_tree(tree_id).map_err(|e| e.to_string())?;

    if !allow_empty {
        if let Some(ref parent) = parent_commit {
            let parent_tree = parent.tree().map_err(|e| e.to_string())?;
            if parent_tree.id() == tree.id() {
                return Err("Nothing to commit (index matches HEAD)".into());
            }
        }
    }

    let sig = repo.signature().map_err(|e| e.to_string())?;
    let message = build_commit_message(commit_meta);

    let commit_oid = match parent_commit {
        Some(ref parent) => repo
            .commit(Some("HEAD"), &sig, &sig, &message, &tree, &[parent])
            .map_err(|e| e.to_string())?,
        None => repo
            .commit(Some("HEAD"), &sig, &sig, &message, &tree, &[])
            .map_err(|e| e.to_string())?,
    };

    Ok(commit_oid.to_string())
}

fn build_commit_message(commit_meta: &CommitMessage) -> String {
    let mut msg = String::new();

    msg.push_str(commit_meta.title.trim());
    msg.push('\n');

    if let Some(desc) = commit_meta.description.as_deref() {
        msg.push('\n');
        msg.push_str(desc.trim());
        msg.push('\n');
    }

    if !commit_meta.co_authors.is_empty() {
        msg.push('\n');
        for (name, email) in &commit_meta.co_authors {
            msg.push_str(&format!("Co-authored-by: {} <{}>\n", name, email));
        }
    }

    msg
}
/* #endregion  // ? utils */
