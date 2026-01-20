use serde::Serialize;
use std::{path::Path, process::Command};
use uuid::Uuid;

#[derive(Serialize)]
pub struct RepoSitoryStore {
    pub id: String,
    pub name: String,
    pub path: String,
    pub origin: Option<String>,
    pub branch: Option<String>,
}

#[derive(Serialize)]
pub struct GitRepoResponse {
    pub error: Option<String>,
    pub success: Option<RepoSitoryStore>,
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
pub fn add_local_git_repo(repo_path: String) -> Result<GitRepoResponse, String> {
    let path = Path::new(&repo_path);

    if !path.exists() {
        return Ok(GitRepoResponse {
            error: Some(format!("Path does not exist: {}", repo_path)),
            success: None,
        });
    }

    if !path.is_dir() {
        return Ok(GitRepoResponse {
            error: Some(format!("Path is not a directory: {}", repo_path)),
            success: None,
        });
    }

    let git_dir = path.join(".git");

    if !git_dir.exists() || !git_dir.is_dir() {
        return Ok(GitRepoResponse {
            error: Some(format!(
                "Not a valid Git repository 🖕 (no .git folder): {}",
                repo_path
            )),
            success: None,
        });
    }

    let origin = run_git_command(&["remote", "get-url", "origin"], path);
    let branch = run_git_command(&["rev-parse", "--abbrev-ref", "HEAD"], path);

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

    Ok(GitRepoResponse {
        error: None,
        success: Some(RepoSitoryStore {
            id: Uuid::new_v4().to_string(),
            name,
            path: repo_path,
            origin,
            branch,
        }),
    })
}
