use git::AppState;
use git::service::core::RepoServices;
use serde::Serialize;
use std::sync::Mutex;
use std::{
    path::Path,
    process::Command,
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};
use uuid::Uuid;

use crate::repo_manager::{RepoManager, SELECTED_REPO_KEY};

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

    let (current_branch, ahead_behind) = match RepoServices::new(&repo_path) {
        Ok(services) => {
            let current_branch = services
                .branch()
                .get_current_branch()
                .await
                .ok()
                .map(|b| b.name);
            let ahead_behind = services
                .branch()
                .status_ahead_behind()
                .await
                .ok()
                .map(|status| (status.ahead as u32, status.behind as u32));
            (current_branch, ahead_behind)
        }
        Err(_) => (None, None),
    };

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

#[tauri::command]
pub async fn select_repository(
    repo_id: String,
    state: tauri::State<'_, AppState>,
    manager: tauri::State<'_, Arc<Mutex<RepoManager>>>,
) -> Result<bool, String> {
    let repos = {
        let app = {
            let manager_guard = manager.lock().map_err(|e| e.to_string())?;
            manager_guard.app.clone()
        };
        let temp = RepoManager::new(app);
        temp.list_repositories(false).await?
    };

    let repo = repos
        .into_iter()
        .find(|r| r.id == repo_id)
        .ok_or("Repository not found")?;

    let services = Arc::new(RepoServices::new(&repo.path)?);

    {
        let mut lock = state.services.write().await;
        *lock = Some(services);
    }

    let manager_guard = manager.lock().map_err(|e| e.to_string())?;
    let store = manager_guard
        .get_store()
        .map_err(|e| format!("Failed to get store: {}", e))?;

    store.set(SELECTED_REPO_KEY, repo_id);
    store.save().map_err(|e| e.to_string())?;

    Ok(true)
}
