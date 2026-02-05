use serde::Serialize;
use std::{
    path::Path,
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};
use uuid::Uuid;

#[derive(Serialize)]
pub struct RepoSitoryStore {
    pub id: String,
    pub name: String,
    pub path: String,
    pub origin: Option<String>,
    pub current_branch: Option<String>,
    pub ahead_behind: Option<(u32, u32)>,
    pub has_uncommitted_changes: bool,
    pub last_updated: u64,
}

fn run_git_command(args: &[&str], repo_path: &Path) -> Option<String> {
    Command::new("git")
        .args(args)
        .current_dir(repo_path)
        .output()
        .ok()
        .and_then(|out| {
            if out.status.success() {
                Some(String::from_utf8_lossy(&out.stdout).trim().to_string())
            } else {
                None
            }
        })
}

#[tauri::command]
#[logger::logger]
pub async fn add_local_git_repo(repo_path: String) -> Result<Option<RepoSitoryStore>, String> {
    let path = Path::new(&repo_path);

    if !path.exists() {
        return Err(format!("Path does not exist: {}", repo_path));
    }

    if !path.is_dir() {
        return Err(format!("Path is not a directory: {}", repo_path));
    }

    let git_dir = path.join(".git");

    if !git_dir.exists() || !git_dir.is_dir() {
        return Err(format!(
            "Not a valid Git repository (no .git folder): {}",
            repo_path
        ));
    }

    let origin = run_git_command(&["remote", "get-url", "origin", "--"], path);

    let name = if let Some(ref o) = origin {
        Path::new(o)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or(o)
            .to_string()
    } else {
        path.file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .to_string()
    };

    let current_branch = git::commands::branch::current_branch(&repo_path)
        .await
        .ok()
        .map(|b| b.name);

    let ahead_behind = git::commands::branch::status_ahead_behind(&repo_path)
        .await
        .ok()
        .and_then(|status| Some((status.ahead as u32, status.behind as u32)));

    let has_uncommitted_changes = Command::new("git")
        .args(&["status", "--porcelain"])
        .current_dir(path)
        .output()
        .ok()
        .map(|output| !output.stdout.is_empty())
        .unwrap_or(false);

    Ok(Some(RepoSitoryStore {
        id: Uuid::new_v4().to_string(),
        name,
        path: repo_path,
        origin,
        current_branch,
        ahead_behind,
        has_uncommitted_changes,
        last_updated: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
    }))
}
