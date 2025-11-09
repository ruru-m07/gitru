use git2::{BranchType, ObjectType, Repository};
use serde::Serialize;

use crate::commands::GitResult;

#[derive(Debug, Serialize, Clone)]
pub struct Branch {
    pub name: String,
    pub display_name: String,
    pub is_remote: bool,
}

fn open_repo(repo_path: &str) -> Result<Repository, String> {
    Repository::open(repo_path).map_err(|e| format!("Failed to open repo: {e}"))
}

#[tauri::command(rename_all = "snake_case")]
pub fn current_branch(repo_path: &str) -> Result<Branch, String> {
    let repo = open_repo(repo_path)?;
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

#[tauri::command(rename_all = "snake_case")]
pub fn list_branch(repo_path: &str) -> Result<Vec<Branch>, String> {
    let repo = open_repo(repo_path)?;

    let current_name = repo.head().ok().and_then(|h| {
        if h.is_branch() {
            h.shorthand().map(|s| s.to_string())
        } else {
            None
        }
    });

    let mut result: Vec<Branch> = Vec::new();

    // Local branches
    let iter = repo
        .branches(Some(BranchType::Local))
        .map_err(|e| format!("Failed to list local branches: {e}"))?;
    for item in iter {
        let (branch, _kind) = item.map_err(|e| format!("Failed to read branch: {e}"))?;
        let name = branch
            .name()
            .map_err(|e| format!("Failed to get branch name: {e}"))?
            .unwrap_or_default()
            .to_string();
        result.push(Branch {
            name: name.clone(),
            display_name: name,
            is_remote: false,
        });
    }

    let iter = repo
        .branches(Some(BranchType::Remote))
        .map_err(|e| format!("Failed to list remote branches: {e}"))?;
    for item in iter {
        let (branch, _kind) = item.map_err(|e| format!("Failed to read remote branch: {e}"))?;
        let full = branch
            .name()
            .map_err(|e| format!("Failed to get remote branch name: {e}"))?
            .unwrap_or_default()
            .to_string();

        let mut parts = full.splitn(2, '/');
        let remote = parts.next().unwrap_or("").to_string();
        let short = parts.next().unwrap_or("").to_string();
        if remote != "origin" {
            continue;
        }

        if short == "HEAD" {
            continue;
        }
        let display_name = format!("{remote}/{short}");
        result.push(Branch {
            name: if short.is_empty() {
                full.clone()
            } else {
                short
            },
            display_name,
            is_remote: true,
        });
    }

    result.sort_by(|a, b| a.display_name.cmp(&b.display_name));
    result.dedup_by(|a, b| a.display_name == b.display_name && a.is_remote == b.is_remote);

    if let Some(cur) = current_name {
        let mut current: Vec<Branch> = Vec::new();
        let mut others: Vec<Branch> = Vec::new();
        for b in result.into_iter() {
            if !b.is_remote && b.name == cur {
                current.push(b);
            } else {
                others.push(b);
            }
        }
        current.extend(others);
        Ok(current)
    } else {
        Ok(result)
    }
}

#[tauri::command(rename_all = "snake_case")]
pub fn switch_branch(repo_path: &str, branch_name: &str) -> GitResult {
    let repo = match open_repo(repo_path) {
        Ok(r) => r,
        Err(e) => return crate::commands::GitResult::error(e),
    };

    let local_branch = repo.find_branch(branch_name, BranchType::Local).ok();

    let refname = if let Some(branch) = local_branch {
        // Use existing local branch
        branch
            .get()
            .name()
            .map(|name| name.to_string())
            .unwrap_or_else(|| format!("refs/heads/{branch_name}"))
    } else {
        let remote_ref = format!("origin/{branch_name}");
        let remote_branch = match repo.find_branch(&remote_ref, BranchType::Remote) {
            Ok(b) => b,
            Err(e) => {
                return crate::commands::GitResult::error(format!(
                    "Branch '{branch_name}' not found locally or on origin: {e}"
                ));
            }
        };

        let target_oid = match remote_branch.get().target() {
            Some(oid) => oid,
            None => match remote_branch.get().peel(ObjectType::Commit) {
                Ok(obj) => obj.id(),
                Err(e) => {
                    return crate::commands::GitResult::error(format!(
                        "Failed to resolve remote branch target: {e}"
                    ));
                }
            },
        };
        let commit = match repo.find_commit(target_oid) {
            Ok(c) => c,
            Err(e) => {
                return crate::commands::GitResult::error(format!(
                    "Failed to find commit {target_oid}: {e}"
                ));
            }
        };

        if let Err(e) = repo.branch(branch_name, &commit, false) {
            return crate::commands::GitResult::error(format!(
                "Failed to create local branch '{branch_name}': {e}"
            ));
        }

        if let Ok(mut lb) = repo.find_branch(branch_name, BranchType::Local) {
            let _ = lb.set_upstream(Some(&remote_ref));
        }

        format!("refs/heads/{branch_name}")
    };

    if let Err(e) = repo.set_head(&refname) {
        return crate::commands::GitResult::error(format!("Failed to set HEAD to {refname}: {e}"));
    }

    // Checkout files to match the new HEAD
    let obj = match repo.revparse_single(&refname) {
        Ok(o) => o,
        Err(e) => {
            return crate::commands::GitResult::error(format!(
                "Failed to resolve target {refname}: {e}"
            ));
        }
    };

    let mut opts = git2::build::CheckoutBuilder::new();
    opts.safe()
        .allow_conflicts(true)
        .remove_untracked(true)
        .remove_ignored(true);
    if let Err(e) = repo.checkout_tree(&obj, Some(&mut opts)) {
        return crate::commands::GitResult::error(format!("Failed to checkout working tree: {e}"));
    }

    crate::commands::GitResult::success()
}
