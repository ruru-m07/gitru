use git2::{BranchType, ErrorCode};

use crate::{
    types_legacy::{Branch, BranchInfo, CommitInfo},
    utils::{extract_all_authors, open_repository},
};

#[derive(serde::Serialize)]
pub struct AheadBehindStatus {
    pub ahead: usize,
    pub behind: usize,

    pub local_branch: String,
    pub local_branch_id: String,

    pub upstream_branch: Option<String>,
    pub upstream_branch_id: Option<String>,

    pub is_published: bool,
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
#[logger::logger]
pub async fn current_branch(repo_path: &str) -> Result<Branch, String> {
    let repo = open_repository(repo_path).map_err(|e| e.to_string())?;

    match repo.head() {
        // ? normal case: repo has commits
        Ok(head) if head.is_branch() => {
            let short = head
                .shorthand()
                .ok_or_else(|| "Failed to read branch name".to_string())?;

            Ok(Branch {
                name: short.to_string(),
                display_name: short.to_string(),
                is_remote: false,
            })
        }

        // ! empty repo: HEAD exists but branch is unborn
        Err(e) if e.code() == ErrorCode::UnbornBranch => {
            let head = repo.find_reference("HEAD").map_err(|e| e.to_string())?;

            let target = head
                .symbolic_target()
                .ok_or_else(|| "HEAD is not symbolic".to_string())?;

            let branch = target
                .strip_prefix("refs/heads/")
                .ok_or_else(|| "Invalid HEAD reference".to_string())?;

            Ok(Branch {
                name: branch.to_string(),
                display_name: branch.to_string(),
                is_remote: false,
            })
        }

        // ! Detached HEAD or other error
        Ok(_) => Err("Repository is in detached HEAD state".into()),
        Err(e) => Err(format!("Failed to get HEAD: {e}")),
    }
}

#[tauri::command]
#[logger::logger]
pub async fn list_branches(repo_path: &str, kind: BranchKind) -> Result<Vec<BranchInfo>, String> {
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

#[tauri::command]
#[logger::logger]
pub async fn status_ahead_behind(repo_path: &str) -> Result<AheadBehindStatus, String> {
    let repo = open_repository(repo_path).map_err(|e| e.to_string())?;

    let head = match repo.head() {
        Ok(h) => h,
        Err(e) if e.code() == ErrorCode::UnbornBranch => {
            // Empty repo → no commits → nothing is ahead/behind
            let head = repo.find_reference("HEAD").map_err(|e| e.to_string())?;

            let target = head
                .symbolic_target()
                .ok_or_else(|| "HEAD is not symbolic".to_string())?;

            let branch = target
                .strip_prefix("refs/heads/")
                .ok_or_else(|| "Invalid HEAD reference".to_string())?;

            return Ok(AheadBehindStatus {
                ahead: 0,
                behind: 0,
                local_branch: branch.to_string(),
                local_branch_id: String::new(),
                upstream_branch: None,
                upstream_branch_id: None,
                is_published: false,
            });
        }
        Err(e) => return Err(format!("Failed to get HEAD: {e}")),
    };

    let local_branch_name = head
        .shorthand()
        .ok_or_else(|| "HEAD does not point to a named branch".to_string())?;

    let branch = repo
        .find_branch(local_branch_name, BranchType::Local)
        .map_err(|_| format!("Failed to find local branch {local_branch_name}"))?;

    // If branch has no target → unborn branch (extra safety)
    let local_oid = match branch.get().target() {
        Some(oid) => oid,
        None => {
            return Ok(AheadBehindStatus {
                ahead: 0,
                behind: 0,
                local_branch: local_branch_name.to_string(),
                local_branch_id: String::new(),
                upstream_branch: None,
                upstream_branch_id: None,
                is_published: false,
            });
        }
    };

    let upstream = branch.upstream().ok();

    if let Some(upstream) = upstream {
        let upstream_oid = match upstream.get().target() {
            Some(oid) => oid,
            None => {
                return Ok(AheadBehindStatus {
                    ahead: 0,
                    behind: 0,
                    local_branch: local_branch_name.to_string(),
                    local_branch_id: local_oid.to_string(),
                    upstream_branch: None,
                    upstream_branch_id: None,
                    is_published: false,
                });
            }
        };

        let upstream_name = upstream
            .get()
            .shorthand()
            .ok_or_else(|| "Failed to read upstream branch name".to_string())?
            .to_string();

        let (ahead, behind) = repo
            .graph_ahead_behind(local_oid, upstream_oid)
            .map_err(|e| e.to_string())?;

        Ok(AheadBehindStatus {
            ahead,
            behind,
            local_branch: local_branch_name.to_string(),
            local_branch_id: local_oid.to_string(),
            upstream_branch: Some(upstream_name),
            upstream_branch_id: Some(upstream_oid.to_string()),
            is_published: true,
        })
    } else {
        Ok(AheadBehindStatus {
            ahead: 0,
            behind: 0,
            local_branch: local_branch_name.to_string(),
            local_branch_id: local_oid.to_string(),
            upstream_branch: None,
            upstream_branch_id: None,
            is_published: false,
        })
    }
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
