use git2::BranchType;
use serde::Serialize;

use crate::utils::open_repository;

#[derive(Debug, Serialize, Clone)]
pub struct Branch {
    pub name: String,
    pub display_name: String,
    pub is_remote: bool,
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
pub fn list_branch(repo_path: &str) -> Result<Vec<Branch>, String> {
    let repo = open_repository(repo_path).map_err(|e| e.to_string())?;

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
