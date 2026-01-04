use serde::Serialize;

use crate::{
    types::CommitInfo,
    utils::{extract_all_authors, open_repository},
};

#[derive(Debug, Serialize, Clone)]
pub struct Branch {
    pub name: String,
    pub display_name: String,
    pub is_remote: bool,
}

#[derive(Serialize, Clone)]
pub struct BranchInfo {
    pub name: String,
    pub display_name: String,

    pub is_remote: bool,
    pub is_head: bool,

    pub commit: CommitInfo,

    pub upstream: Option<String>,
    pub ahead: Option<usize>,
    pub behind: Option<usize>,
}

#[derive(serde::Deserialize)]
pub enum BranchKind {
    Local,
    Remote,
}

impl From<BranchKind> for git2::BranchType {
    fn from(v: BranchKind) -> Self {
        match v {
            BranchKind::Local => git2::BranchType::Local,
            BranchKind::Remote => git2::BranchType::Remote,
        }
    }
}

#[tauri::command]
pub fn current_branch(repo_path: &str) -> Result<Branch, String> {
    let repo = open_repository(repo_path).map_err(|e| e.to_string())?;
    let head = repo
        .head()
        .map_err(|e| format!("Failed to get HEAD: {e}"))?;

    if head.is_branch() {
        let short = head
            .shorthand()
            .ok_or_else(|| "Failed to read branch name".to_string())?;
        Ok(Branch {
            name: short.to_string(),
            display_name: short.to_string(),
            is_remote: false,
        })
    } else {
        Err("Repository is in detached HEAD state or has no current branch".into())
    }
}

#[tauri::command]
pub fn list_branches(repo_path: &str, kind: BranchKind) -> Result<Vec<BranchInfo>, String> {
    let repo: git2::Repository = open_repository(repo_path).map_err(|e| e.to_string())?;

    let kind = match kind {
        BranchKind::Local => git2::BranchType::Local,
        BranchKind::Remote => git2::BranchType::Remote,
    };

    let mut branches = collect_branches(&repo, kind).map_err(|e| e.to_string())?;

    // ? Sort: current first, then local, then remote, alphabetical
    branches.sort_by(|a, b| {
        b.is_head
            .cmp(&a.is_head)
            .then(a.is_remote.cmp(&b.is_remote))
            .then(a.display_name.cmp(&b.display_name))
    });

    Ok(branches)
}

/* #region // ? Helpers */
fn upstream_info(
    repo: &git2::Repository,
    branch: &git2::Branch,
) -> (Option<String>, Option<usize>, Option<usize>) {
    let upstream = branch.upstream().ok();

    let upstream = match upstream {
        Some(u) => u,
        None => return (None, None, None),
    };

    let upstream_name = upstream.name().ok().flatten().map(|s| s.to_string());

    let local_oid = branch.get().target();
    let upstream_oid = upstream.get().target();

    match (local_oid, upstream_oid) {
        (Some(local), Some(up)) => {
            if let Ok((ahead, behind)) = repo.graph_ahead_behind(local, up) {
                (upstream_name, Some(ahead), Some(behind))
            } else {
                (upstream_name, None, None)
            }
        }
        _ => (upstream_name, None, None),
    }
}

fn collect_branches(
    repo: &git2::Repository,
    kind: git2::BranchType,
) -> Result<Vec<BranchInfo>, git2::Error> {
    let mut out = Vec::new();

    for item in repo.branches(Some(kind))? {
        let (branch, _) = item?;
        let reference = branch.get();

        let name = branch.name()?.unwrap_or("").to_string();

        // Skip origin/HEAD
        if kind == git2::BranchType::Remote && name.ends_with("/HEAD") {
            continue;
        }

        let display_name = name.clone();
        let is_remote = kind == git2::BranchType::Remote;
        let is_head = branch.is_head();

        let commit = reference.peel_to_commit()?;
        let commit_details = CommitInfo {
            id: commit.id().to_string(),
            timestamp: commit.time().seconds(),
            summary: commit.summary().unwrap_or("").to_string(),
            body: commit.body().unwrap_or("").to_string(),
            authors: extract_all_authors(&commit),
        };

        let (upstream, ahead, behind) = if !is_remote {
            upstream_info(repo, &branch)
        } else {
            (None, None, None)
        };

        out.push(BranchInfo {
            name,
            display_name,
            is_remote,
            is_head,
            commit: commit_details,
            upstream,
            ahead,
            behind,
        });
    }

    Ok(out)
}

/* #endregion // ? Helpers */
