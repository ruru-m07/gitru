use crate::{BatchUpdate, RepoWatcher, WatcherConfig, WatcherError as WatcherCrateError};
use serde::Serialize;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, State};

#[derive(Default)]
pub struct WatcherState {
    watchers: Arc<Mutex<HashMap<String, Arc<RepoWatcher>>>>,
}

impl WatcherState {
    pub fn new() -> Self {
        Self {
            watchers: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn insert(&self, id: String, watcher: RepoWatcher) {
        self.watchers.lock().unwrap().insert(id, Arc::new(watcher));
    }

    pub fn get(&self, id: &str) -> Option<Arc<RepoWatcher>> {
        self.watchers.lock().unwrap().get(id).cloned()
    }

    pub fn remove(&self, id: &str) -> Option<Arc<RepoWatcher>> {
        self.watchers.lock().unwrap().remove(id)
    }
}

#[derive(Debug, Serialize)]
pub struct StartWatchingResponse {
    pub success: bool,
    pub watcher_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct StopWatchingResponse {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct RescanResponse {
    pub success: bool,
    pub files_found: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct WatcherStateInfo {
    pub is_active: bool,
    pub repo_path: String,
    pub uptime_seconds: u64,
}

#[tauri::command]
pub async fn start_watching(
    repo_path: String,
    config: Option<WatcherConfig>,
    app_handle: AppHandle,
    state: State<'_, WatcherState>,
) -> Result<StartWatchingResponse, String> {
    let watcher_id = uuid::Uuid::new_v4().to_string();
    let config = config.unwrap_or_default();

    let watcher = match RepoWatcher::new(&repo_path, config) {
        Ok(w) => w,
        Err(e) => {
            return Ok(StartWatchingResponse {
                success: false,
                watcher_id: String::new(),
                error: Some(format!("Failed to create watcher: {}", e)),
            });
        }
    };

    let app_handle_clone = app_handle.clone();

    watcher.set_callback(move |batch: BatchUpdate| {
        if let Err(e) = app_handle_clone.emit("watcher:batch-update", &batch) {
            eprintln!("Failed to emit batch update: {}", e);
        }
    });

    if let Err(e) = watcher.start() {
        return Ok(StartWatchingResponse {
            success: false,
            watcher_id: String::new(),
            error: Some(format!("Failed to start watcher: {}", e)),
        });
    }

    state.insert(watcher_id.clone(), watcher);

    let _ = app_handle.emit(
        "watcher:state-changed",
        serde_json::json!({
            "watcher_id": watcher_id,
            "old_state": "stopped",
            "new_state": "active",
            "timestamp": chrono::Utc::now().to_rfc3339(),
        }),
    );

    Ok(StartWatchingResponse {
        success: true,
        watcher_id,
        error: None,
    })
}

#[tauri::command]
pub async fn stop_watching(
    watcher_id: String,
    app_handle: AppHandle,
    state: State<'_, WatcherState>,
) -> Result<StopWatchingResponse, String> {
    if let Some(watcher) = state.remove(&watcher_id) {
        if let Err(e) = watcher.stop() {
            return Ok(StopWatchingResponse {
                success: false,
                error: Some(format!("Failed to stop watcher: {}", e)),
            });
        }

        let _ = app_handle.emit(
            "watcher:state-changed",
            serde_json::json!({
                "watcher_id": watcher_id,
                "old_state": "active",
                "new_state": "stopped",
                "timestamp": chrono::Utc::now().to_rfc3339(),
            }),
        );

        Ok(StopWatchingResponse {
            success: true,
            error: None,
        })
    } else {
        Ok(StopWatchingResponse {
            success: false,
            error: Some("Watcher not found".to_string()),
        })
    }
}

#[tauri::command]
pub async fn get_watcher_state(
    watcher_id: String,
    state: State<'_, WatcherState>,
) -> Result<WatcherStateInfo, String> {
    if let Some(watcher) = state.get(&watcher_id) {
        Ok(WatcherStateInfo {
            is_active: watcher.is_running(),
            repo_path: watcher.repo_path().to_string_lossy().to_string(),
            uptime_seconds: watcher.uptime(),
        })
    } else {
        Err("Watcher not found".to_string())
    }
}

#[tauri::command]
pub async fn rescan_repository(
    watcher_id: String,
    state: State<'_, WatcherState>,
) -> Result<RescanResponse, String> {
    if let Some(watcher) = state.get(&watcher_id) {
        if let Err(e) = watcher.reload_gitignore() {
            return Ok(RescanResponse {
                success: false,
                files_found: 0,
                error: Some(format!("Failed to reload gitignore: {}", e)),
            });
        }

        Ok(RescanResponse {
            success: true,
            files_found: 0,
            error: None,
        })
    } else {
        Ok(RescanResponse {
            success: false,
            files_found: 0,
            error: Some("Watcher not found".to_string()),
        })
    }
}

pub fn emit_watcher_error(
    app_handle: &AppHandle,
    watcher_id: &str,
    error: WatcherCrateError,
    recoverable: bool,
) {
    let error_type = match &error {
        WatcherCrateError::PermissionDenied(_) => "permission_denied",
        WatcherCrateError::PathNotFound(_) => "path_not_found",
        WatcherCrateError::GitignoreError(_) => "gitignore_parse_error",
        WatcherCrateError::BackpressureExceeded(_) => "backpressure_exceeded",
        _ => "unknown",
    };

    let _ = app_handle.emit(
        "watcher:error",
        serde_json::json!({
            "watcher_id": watcher_id,
            "error_type": error_type,
            "message": error.to_string(),
            "recoverable": recoverable,
            "timestamp": chrono::Utc::now().to_rfc3339(),
        }),
    );
}
