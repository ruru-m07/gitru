use git::AppState;
use git::core::RepoServices;
use serde::Deserialize;
use serde::Serialize;
use std::path::Path;
use std::process::Command;
use std::sync::Mutex;
use std::{
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

#[tauri::command]
#[logger::logger]
pub async fn add_local_git_repo(repo_path: String) -> Result<Option<RepoSitoryStore>, String> {
    let path = Path::new(&repo_path);

    if !path.exists() {
        return Err(format!("Path does not exist: {repo_path}"));
    }

    if !path.is_dir() {
        return Err(format!("Path is not a directory: {repo_path}"));
    }

    let git_dir = path.join(".git");

    if !git_dir.exists() || !git_dir.is_dir() {
        return Err(format!(
            "Not a valid Git repository (no .git folder): {repo_path}"
        ));
    }

    let (origin, current_branch, ahead_behind, has_uncommitted_changes) =
        match RepoServices::new(&repo_path) {
            Ok(services) => {
                let origin = services
                    .origin()
                    .repository_origin()
                    .await
                    .ok()
                    .map(|o| o.remote_url);
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
                let has_uncommitted_changes = services
                    .branch()
                    .has_uncommitted_changes()
                    .await
                    .unwrap_or(false);
                (
                    origin,
                    current_branch,
                    ahead_behind,
                    has_uncommitted_changes,
                )
            }
            Err(_) => (None, None, None, false),
        };

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
        .map_err(|e| format!("Failed to get store: {e}"))?;

    store.set(SELECTED_REPO_KEY, repo_id);
    store.save().map_err(|e| e.to_string())?;

    Ok(true)
}

#[tauri::command]
#[logger::logger]
pub async fn open_with_app(
    file_path: String,
    line: Option<u32>,
    app: Option<String>,
) -> Result<(), String> {
    let opener = ExternalOpener::from_input(app.as_deref())?;

    #[cfg(target_os = "macos")]
    {
        return open_on_macos(opener, &file_path, line);
    }

    #[cfg(target_os = "windows")]
    {
        return open_on_windows(opener, &file_path, line);
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        return open_on_linux(opener, &file_path, line);
    }
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
enum ExternalOpener {
    Vscode,
    Cursor,
    Finder,
    Terminal,
    Ghostty,
}

impl ExternalOpener {
    fn from_input(input: Option<&str>) -> Result<Self, String> {
        match input.unwrap_or("vscode").to_ascii_lowercase().as_str() {
            "vscode" => Ok(Self::Vscode),
            "cursor" => Ok(Self::Cursor),
            "finder" => Ok(Self::Finder),
            "terminal" => Ok(Self::Terminal),
            "ghostty" | "ghosty" => Ok(Self::Ghostty),
            other => Err(format!(
                "Unsupported opener '{other}'. Use one of: vscode, cursor, finder, terminal, ghostty"
            )),
        }
    }
}

fn path_for_directory_apps(file_path: &str) -> String {
    let path = Path::new(file_path);
    if path.is_dir() {
        return file_path.to_string();
    }

    match path.parent() {
        Some(parent) => parent.to_string_lossy().to_string(),
        None => file_path.to_string(),
    }
}

#[cfg(target_os = "macos")]
fn open_on_macos(opener: ExternalOpener, file_path: &str, line: Option<u32>) -> Result<(), String> {
    let mut command = Command::new("open");

    match opener {
        ExternalOpener::Vscode => {
            command.arg("-a").arg("Visual Studio Code");
            if let Some(line) = line {
                command
                    .arg("--args")
                    .arg("--goto")
                    .arg(format!("{file_path}:{line}"));
            } else {
                command.arg(file_path);
            }
        }
        ExternalOpener::Cursor => {
            command.arg("-a").arg("Cursor");
            if let Some(line) = line {
                command
                    .arg("--args")
                    .arg("--goto")
                    .arg(format!("{file_path}:{line}"));
            } else {
                command.arg(file_path);
            }
        }
        ExternalOpener::Finder => {
            command.arg("-a").arg("Finder").arg(file_path);
        }
        ExternalOpener::Terminal => {
            command
                .arg("-a")
                .arg("Terminal")
                .arg(path_for_directory_apps(file_path));
        }
        ExternalOpener::Ghostty => {
            command
                .arg("-a")
                .arg("Ghostty")
                .arg(path_for_directory_apps(file_path));
        }
    }

    command
        .spawn()
        .map_err(|e| format!("Failed to launch opener on macOS: {e}"))?;

    Ok(())
}

#[cfg(target_os = "windows")]
fn open_on_windows(
    opener: ExternalOpener,
    file_path: &str,
    line: Option<u32>,
) -> Result<(), String> {
    let target_with_line = line
        .map(|line| format!("{}:{}", file_path, line))
        .unwrap_or_else(|| file_path.to_string());

    let mut command = match opener {
        ExternalOpener::Vscode => {
            let mut cmd = Command::new("code");
            cmd.arg("-g").arg(target_with_line);
            cmd
        }
        ExternalOpener::Cursor => {
            let mut cmd = Command::new("cursor");
            cmd.arg("-g").arg(target_with_line);
            cmd
        }
        ExternalOpener::Finder => {
            let mut cmd = Command::new("explorer");
            cmd.arg(file_path);
            cmd
        }
        ExternalOpener::Terminal => {
            let mut cmd = Command::new("wt");
            cmd.arg("-d").arg(path_for_directory_apps(file_path));
            cmd
        }
        ExternalOpener::Ghostty => {
            let mut cmd = Command::new("ghostty");
            cmd.arg(path_for_directory_apps(file_path));
            cmd
        }
    };

    command
        .spawn()
        .map_err(|e| format!("Failed to launch opener on Windows: {}", e))?;

    Ok(())
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn open_on_linux(opener: ExternalOpener, file_path: &str, line: Option<u32>) -> Result<(), String> {
    let target_with_line = line
        .map(|line| format!("{}:{}", file_path, line))
        .unwrap_or_else(|| file_path.to_string());

    let mut command = match opener {
        ExternalOpener::Vscode => {
            let mut cmd = Command::new("code");
            cmd.arg("-g").arg(target_with_line);
            cmd
        }
        ExternalOpener::Cursor => {
            let mut cmd = Command::new("cursor");
            cmd.arg("-g").arg(target_with_line);
            cmd
        }
        ExternalOpener::Finder => {
            let mut cmd = Command::new("xdg-open");
            cmd.arg(file_path);
            cmd
        }
        ExternalOpener::Terminal => {
            let mut cmd = Command::new("xdg-open");
            cmd.arg(path_for_directory_apps(file_path));
            cmd
        }
        ExternalOpener::Ghostty => {
            let mut cmd = Command::new("ghostty");
            cmd.arg(path_for_directory_apps(file_path));
            cmd
        }
    };

    command
        .spawn()
        .map_err(|e| format!("Failed to launch opener on Linux: {}", e))?;

    Ok(())
}
