use crate::commands::RepoSitoryStore;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, State};
use tauri_plugin_store::StoreExt;

const REPO_STORE_KEY: &str = "repositories";
const STORE_FILE: &str = "repositories.json";

#[derive(Serialize, Deserialize, Clone)]
pub struct RepositoryInfo {
    pub id: String,
    pub name: String,
    pub path: String,
    pub origin: Option<String>,
    pub current_branch: Option<String>,
    pub ahead_behind: Option<(u32, u32)>,
    pub has_uncommitted_changes: bool,
    pub last_updated: u64,
}

impl From<RepoSitoryStore> for RepositoryInfo {
    fn from(store: RepoSitoryStore) -> Self {
        Self {
            id: store.id,
            name: store.name,
            path: store.path,
            origin: store.origin,
            current_branch: None,
            ahead_behind: None,
            has_uncommitted_changes: false,
            last_updated: 0,
        }
    }
}

pub struct RepoManager {
    app: AppHandle,
}

impl RepoManager {
    pub fn new(app: AppHandle) -> Self {
        Self { app }
    }

    fn get_store(&self) -> Result<Arc<tauri_plugin_store::Store<tauri::Wry>>, String> {
        self.app
            .store(STORE_FILE)
            .map_err(|e| format!("Failed to get store: {}", e))
    }

    fn get_current_timestamp() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
    }

    fn list_repositories_sync(&self) -> Result<Vec<RepositoryInfo>, String> {
        let store = self.get_store()?;
        let repos: Vec<RepositoryInfo> = store
            .get(REPO_STORE_KEY)
            .and_then(|v| serde_json::from_value(v.clone()).ok())
            .unwrap_or_default();
        Ok(repos)
    }

    pub async fn list_repositories(
        &self,
        refresh_stale: bool,
    ) -> Result<Vec<RepositoryInfo>, String> {
        let repos = self.list_repositories_sync()?;

        if !refresh_stale {
            return Ok(repos);
        }

        // ? refresh repositories that are stale (older than 60 seconds)
        let current_time = Self::get_current_timestamp();
        let stale_threshold = 60;

        let mut updated_repos = repos.clone();
        let mut needs_save = false;

        for (index, repo) in repos.iter().enumerate() {
            let age = current_time.saturating_sub(repo.last_updated);

            if age > stale_threshold {
                if let Ok(refreshed) = self.refresh_repository_info_internal(repo).await {
                    updated_repos[index] = refreshed;
                    needs_save = true;
                }
            }
        }

        if needs_save {
            let store = self.get_store()?;
            store.set(
                REPO_STORE_KEY.to_string(),
                serde_json::to_value(&updated_repos).map_err(|e| e.to_string())?,
            );
            store.save().map_err(|e| e.to_string())?;
        }

        Ok(updated_repos)
    }

    pub async fn add_repository(&self, mut repo: RepositoryInfo) -> Result<RepositoryInfo, String> {
        let mut repos = self.list_repositories_sync()?;

        if repos.iter().any(|r| r.path == repo.path) {
            return Err(format!("Repository already exists: {}", repo.path));
        }

        repo = self.refresh_repository_info_internal(&repo).await?;

        repos.push(repo.clone());

        let store = self.get_store()?;
        store.set(
            REPO_STORE_KEY.to_string(),
            serde_json::to_value(&repos).map_err(|e| e.to_string())?,
        );

        store.save().map_err(|e| e.to_string())?;

        Ok(repo)
    }

    pub fn remove_repository(&self, repo_id: &str) -> Result<(), String> {
        let mut repos = self.list_repositories_sync()?;

        repos.retain(|r| r.id != repo_id);

        let store = self.get_store()?;
        store.set(
            REPO_STORE_KEY.to_string(),
            serde_json::to_value(&repos).map_err(|e| e.to_string())?,
        );

        store.save().map_err(|e| e.to_string())?;

        Ok(())
    }

    async fn refresh_repository_info_internal(
        &self,
        repo: &RepositoryInfo,
    ) -> Result<RepositoryInfo, String> {
        let path = Path::new(&repo.path);

        if !path.exists() {
            return Err(format!("Repository path does not exist: {}", repo.path));
        }

        let current_branch = git::commands::branch::current_branch(&repo.path)
            .await
            .ok()
            .map(|b| b.name);

        let ahead_behind = git::commands::branch::status_ahead_behind(&repo.path)
            .await
            .ok()
            .and_then(|status| Some((status.ahead as u32, status.behind as u32)));

        let has_uncommitted_changes = {
            use std::process::Command;
            Command::new("git")
                .args(&["status", "--porcelain"])
                .current_dir(&repo.path)
                .output()
                .ok()
                .map(|output| !output.stdout.is_empty())
                .unwrap_or(false)
        };

        Ok(RepositoryInfo {
            id: repo.id.clone(),
            name: repo.name.clone(),
            path: repo.path.clone(),
            origin: repo.origin.clone(),
            current_branch,
            ahead_behind,
            has_uncommitted_changes,
            last_updated: Self::get_current_timestamp(),
        })
    }

    pub async fn refresh_repository_info(&self, repo_id: &str) -> Result<RepositoryInfo, String> {
        let repos = self.list_repositories_sync()?;

        let repo = repos
            .iter()
            .find(|r| r.id == repo_id)
            .ok_or_else(|| format!("Repository not found: {}", repo_id))?;

        let updated_repo = self.refresh_repository_info_internal(repo).await?;

        let mut repos = self.list_repositories_sync()?;

        if let Some(index) = repos.iter().position(|r| r.id == repo_id) {
            repos[index] = updated_repo.clone();

            let store = self.get_store()?;
            store.set(
                REPO_STORE_KEY.to_string(),
                serde_json::to_value(&repos).map_err(|e| e.to_string())?,
            );

            store.save().map_err(|e| e.to_string())?;
        }

        Ok(updated_repo)
    }
}

#[tauri::command]
#[logger::logger]
pub async fn list_repositories(
    manager: State<'_, Arc<Mutex<RepoManager>>>,
    refresh_stale: Option<bool>,
) -> Result<Vec<RepositoryInfo>, String> {
    let app = {
        let manager = manager.lock().map_err(|e| e.to_string())?;
        manager.app.clone()
    };
    let temp_manager = RepoManager::new(app);
    temp_manager
        .list_repositories(refresh_stale.unwrap_or(true))
        .await
}

#[tauri::command]
#[logger::logger]
pub async fn add_repository(
    manager: State<'_, Arc<Mutex<RepoManager>>>,
    repo: RepositoryInfo,
) -> Result<RepositoryInfo, String> {
    let app = {
        let manager = manager.lock().map_err(|e| e.to_string())?;
        manager.app.clone()
    };
    let temp_manager = RepoManager::new(app);
    temp_manager.add_repository(repo).await
}

#[tauri::command]
#[logger::logger]
pub async fn remove_repository(
    manager: State<'_, Arc<Mutex<RepoManager>>>,
    repo_id: String,
) -> Result<(), String> {
    let manager = manager.lock().map_err(|e| e.to_string())?;
    manager.remove_repository(&repo_id)
}

#[tauri::command]
#[logger::logger]
pub async fn refresh_repository_info(
    manager: State<'_, Arc<Mutex<RepoManager>>>,
    repo_id: String,
) -> Result<RepositoryInfo, String> {
    let app = {
        let manager = manager.lock().map_err(|e| e.to_string())?;
        manager.app.clone()
    };
    let temp_manager = RepoManager::new(app);
    temp_manager.refresh_repository_info(&repo_id).await
}
