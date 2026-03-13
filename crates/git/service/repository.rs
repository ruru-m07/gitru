use git2::{
    Cred, FetchOptions, Progress, RemoteCallbacks,
    build::{CheckoutBuilder, RepoBuilder},
};
use serde::Serialize;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::process::Stdio;
use std::sync::{
    Arc, Mutex, OnceLock,
    atomic::{AtomicBool, Ordering},
};
use tauri::Emitter;

pub const CLONE_PROGRESS_EVENT: &str = "git://clone-progress";

#[derive(Clone, Serialize)]
pub enum CloneProgressPhase {
    Preparing,
    Started,
    Sideband,
    Transfer,
    Pack,
    RefUpdate,
    Message,
    Finished,
    Error,
    Cancelled,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CloneTransferStats {
    pub total_objects: usize,
    pub received_objects: usize,
    pub indexed_objects: usize,
    pub local_objects: usize,
    pub total_deltas: usize,
    pub indexed_deltas: usize,
    pub received_bytes: usize,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClonePackStats {
    pub stage: String,
    pub current: usize,
    pub total: usize,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CloneRefUpdate {
    pub refname: String,
    pub old_oid: String,
    pub new_oid: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CloneProgressEvent {
    pub operation_id: String,
    pub phase: CloneProgressPhase,
    pub status: Option<String>,
    pub line: Option<String>,
    pub percent: Option<f64>,
    pub current: Option<u64>,
    pub total: Option<u64>,
    pub transfer: Option<CloneTransferStats>,
    pub pack: Option<ClonePackStats>,
    pub ref_update: Option<CloneRefUpdate>,
    pub error_kind: Option<String>,
}

pub struct RepositoryService;

static ACTIVE_CLONE_CANCEL_FLAGS: OnceLock<Mutex<HashMap<String, Arc<AtomicBool>>>> =
    OnceLock::new();

fn active_clone_cancel_flags() -> &'static Mutex<HashMap<String, Arc<AtomicBool>>> {
    ACTIVE_CLONE_CANCEL_FLAGS.get_or_init(|| Mutex::new(HashMap::new()))
}

impl RepositoryService {
    pub fn cancel_clone_repository(operation_id: &str) -> Result<bool, String> {
        let flag = {
            let active = active_clone_cancel_flags()
                .lock()
                .map_err(|_| "Failed to access active clone operations".to_string())?;
            active.get(operation_id).cloned()
        };

        let Some(flag) = flag else {
            return Ok(false);
        };

        flag.store(true, Ordering::Relaxed);
        Ok(true)
    }

    pub async fn clone_repository(
        url: &str,
        destination_path: &str,
        operation_id: &str,
        app: &tauri::AppHandle,
    ) -> Result<(), String> {
        app.emit(
            CLONE_PROGRESS_EVENT,
            CloneProgressEvent {
                operation_id: operation_id.to_string(),
                phase: CloneProgressPhase::Preparing,
                status: Some("Preparing clone session".to_string()),
                line: Some(format!(
                    "Preparing clone session {operation_id} for {destination_path}"
                )),
                percent: Some(0.0),
                current: Some(0),
                total: None,
                transfer: None,
                pack: None,
                ref_update: None,
                error_kind: None,
            },
        )
        .map_err(|e| format!("Failed to emit clone preparing event: {e}"))?;

        Self::validate_clone_destination(destination_path)?;

        app.emit(
            CLONE_PROGRESS_EVENT,
            CloneProgressEvent {
                operation_id: operation_id.to_string(),
                phase: CloneProgressPhase::Started,
                status: Some("Cloning repository".to_string()),
                line: Some(format!("git clone {url} {destination_path}")),
                percent: Some(0.0),
                current: Some(0),
                total: None,
                transfer: None,
                pack: None,
                ref_update: None,
                error_kind: None,
            },
        )
        .map_err(|e| format!("Failed to emit clone start event: {e}"))?;

        let cancel_flag = Arc::new(AtomicBool::new(false));
        {
            let mut active = active_clone_cancel_flags()
                .lock()
                .map_err(|_| "Failed to register active clone operation".to_string())?;
            active.insert(operation_id.to_string(), cancel_flag.clone());
        }

        let url_owned = url.to_string();
        let destination_owned = destination_path.to_string();
        let operation_id_owned = operation_id.to_string();
        let app_handle = app.clone();

        let clone_result = tokio::task::spawn_blocking(move || {
            run_git2_clone(
                &url_owned,
                &destination_owned,
                &operation_id_owned,
                &app_handle,
                cancel_flag,
            )
        })
        .await
        .map_err(|e| format!("Clone task failed to join: {e}"))?;

        if let Ok(mut active) = active_clone_cancel_flags().lock() {
            active.remove(operation_id);
        }

        clone_result
    }

    pub async fn init_repository(repo_path: &str) -> Result<String, String> {
        Self::validate_init_path(repo_path)?;

        let output = tokio::process::Command::new("git")
            .arg("init")
            .arg(repo_path)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await
            .map_err(|e| format!("Failed to run git init: {e}"))?;

        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if stdout.is_empty() {
                Ok("Initialized empty Git repository".to_string())
            } else {
                Ok(stdout)
            }
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            if stderr.is_empty() {
                Err("Failed to initialize git repository".to_string())
            } else {
                Err(stderr)
            }
        }
    }

    fn validate_clone_destination(destination_path: &str) -> Result<(), String> {
        let path = Path::new(destination_path);

        if path.exists() {
            if !path.is_dir() {
                return Err(format!(
                    "Destination must be a directory: {destination_path}"
                ));
            }

            let mut entries = path
                .read_dir()
                .map_err(|e| format!("Unable to read destination directory: {e}"))?;

            if entries.next().is_some() {
                return Err(
                    "Destination folder is not empty. Choose a new or empty folder.".to_string(),
                );
            }
        } else if let Some(parent) = path.parent() {
            if !parent.exists() {
                return Err(format!(
                    "Destination parent folder does not exist: {}",
                    parent.display()
                ));
            }
        }

        Ok(())
    }

    fn validate_init_path(repo_path: &str) -> Result<(), String> {
        let path = Path::new(repo_path);

        if !path.exists() {
            let Some(parent) = path.parent() else {
                return Err(format!("Parent folder does not exist: {repo_path}"));
            };

            if !parent.exists() {
                return Err(format!(
                    "Parent folder does not exist: {}",
                    parent.display()
                ));
            }

            if !parent.is_dir() {
                return Err(format!(
                    "Parent path is not a directory: {}",
                    parent.display()
                ));
            }

            return Ok(());
        }

        if !path.is_dir() {
            return Err(format!("Path is not a directory: {repo_path}"));
        }

        let git_dir = path.join(".git");
        if git_dir.exists() {
            return Err(format!("Folder is already a Git repository: {repo_path}"));
        }

        Ok(())
    }
}

fn run_git2_clone(
    repo_url: &str,
    destination_path: &str,
    operation_id: &str,
    app: &tauri::AppHandle,
    cancel_flag: Arc<AtomicBool>,
) -> Result<(), String> {
    let destination = Path::new(destination_path).to_path_buf();

    let mut callbacks = RemoteCallbacks::new();

    callbacks.credentials(|_url, username_from_url, allowed| {
        if allowed.is_ssh_key() {
            return Cred::ssh_key_from_agent(username_from_url.unwrap_or("git"));
        }

        if allowed.is_username() {
            return Cred::username(username_from_url.unwrap_or("git"));
        }

        if allowed.is_default() {
            return Cred::default();
        }

        Err(git2::Error::from_str("No valid authentication method"))
    });

    {
        let operation_id = operation_id.to_string();
        let app = app.clone();
        callbacks.sideband_progress(move |data| {
            let line = String::from_utf8_lossy(data).trim().to_string();
            if line.is_empty() {
                return true;
            }

            let _ = app.emit(
                CLONE_PROGRESS_EVENT,
                CloneProgressEvent {
                    operation_id: operation_id.clone(),
                    phase: CloneProgressPhase::Sideband,
                    status: Some("Remote output".to_string()),
                    line: Some(line),
                    percent: None,
                    current: None,
                    total: None,
                    transfer: None,
                    pack: None,
                    ref_update: None,
                    error_kind: None,
                },
            );

            true
        });
    }

    {
        let operation_id = operation_id.to_string();
        let app = app.clone();
        let cancel_flag = cancel_flag.clone();
        callbacks.transfer_progress(move |stats| {
            if cancel_flag.load(Ordering::Relaxed) {
                let _ = app.emit(
                    CLONE_PROGRESS_EVENT,
                    CloneProgressEvent {
                        operation_id: operation_id.clone(),
                        phase: CloneProgressPhase::Cancelled,
                        status: Some("Clone cancelled by user".to_string()),
                        line: Some("Cancellation requested. Stopping clone...".to_string()),
                        percent: None,
                        current: None,
                        total: None,
                        transfer: None,
                        pack: None,
                        ref_update: None,
                        error_kind: Some("cancelled".to_string()),
                    },
                );
                return false;
            }

            emit_transfer_event(&app, &operation_id, &stats);
            true
        });
    }

    {
        let operation_id = operation_id.to_string();
        let app = app.clone();
        callbacks.pack_progress(move |stage, current, total| {
            let _ = app.emit(
                CLONE_PROGRESS_EVENT,
                CloneProgressEvent {
                    operation_id: operation_id.clone(),
                    phase: CloneProgressPhase::Pack,
                    status: Some("Pack progress".to_string()),
                    line: Some(format!("Pack progress: {stage:?} {current}/{total}")),
                    percent: if total > 0 {
                        Some((current as f64 / total as f64 * 100.0).clamp(0.0, 100.0))
                    } else {
                        None
                    },
                    current: Some(current as u64),
                    total: Some(total as u64),
                    transfer: None,
                    pack: Some(ClonePackStats {
                        stage: format!("{stage:?}"),
                        current,
                        total,
                    }),
                    ref_update: None,
                    error_kind: None,
                },
            );
        });
    }

    {
        let operation_id = operation_id.to_string();
        let app = app.clone();
        callbacks.update_tips(move |refname, old_oid, new_oid| {
            let _ = app.emit(
                CLONE_PROGRESS_EVENT,
                CloneProgressEvent {
                    operation_id: operation_id.clone(),
                    phase: CloneProgressPhase::RefUpdate,
                    status: Some("Reference updated".to_string()),
                    line: Some(format!("Ref updated: {refname} {old_oid} -> {new_oid}",)),
                    percent: None,
                    current: None,
                    total: None,
                    transfer: None,
                    pack: None,
                    ref_update: Some(CloneRefUpdate {
                        refname: refname.to_string(),
                        old_oid: old_oid.to_string(),
                        new_oid: new_oid.to_string(),
                    }),
                    error_kind: None,
                },
            );
            true
        });
    }

    let mut fetch_options = FetchOptions::new();
    fetch_options.remote_callbacks(callbacks);

    let mut checkout = CheckoutBuilder::new();
    checkout.safe();

    let mut builder = RepoBuilder::new();
    builder.fetch_options(fetch_options);
    builder.with_checkout(checkout);

    let clone_result = builder.clone(repo_url, &destination);

    if cancel_flag.load(Ordering::Relaxed) {
        let _ = clean_partial_clone(&destination);
        let message = "Clone cancelled by user".to_string();
        let _ = app.emit(
            CLONE_PROGRESS_EVENT,
            CloneProgressEvent {
                operation_id: operation_id.to_string(),
                phase: CloneProgressPhase::Cancelled,
                status: Some("Clone cancelled".to_string()),
                line: Some(message.clone()),
                percent: None,
                current: None,
                total: None,
                transfer: None,
                pack: None,
                ref_update: None,
                error_kind: Some("cancelled".to_string()),
            },
        );
        return Err(message);
    }

    match clone_result {
        Ok(_) => {
            let _ = app.emit(
                CLONE_PROGRESS_EVENT,
                CloneProgressEvent {
                    operation_id: operation_id.to_string(),
                    phase: CloneProgressPhase::Finished,
                    status: Some("Clone completed".to_string()),
                    line: None,
                    percent: Some(100.0),
                    current: None,
                    total: None,
                    transfer: None,
                    pack: None,
                    ref_update: None,
                    error_kind: None,
                },
            );
            Ok(())
        }
        Err(error) => {
            let _ = clean_partial_clone(&destination);
            let raw = error.message().to_string();
            let kind = classify_clone_error(&raw);
            let message = build_clone_error_message(&raw, &kind);

            let _ = app.emit(
                CLONE_PROGRESS_EVENT,
                CloneProgressEvent {
                    operation_id: operation_id.to_string(),
                    phase: CloneProgressPhase::Error,
                    status: Some("Clone failed".to_string()),
                    line: Some(message.clone()),
                    percent: None,
                    current: None,
                    total: None,
                    transfer: None,
                    pack: None,
                    ref_update: None,
                    error_kind: Some(kind),
                },
            );

            Err(message)
        }
    }
}

fn emit_transfer_event(app: &tauri::AppHandle, operation_id: &str, stats: &Progress<'_>) {
    let transfer = CloneTransferStats {
        total_objects: stats.total_objects(),
        received_objects: stats.received_objects(),
        indexed_objects: stats.indexed_objects(),
        local_objects: stats.local_objects(),
        total_deltas: stats.total_deltas(),
        indexed_deltas: stats.indexed_deltas(),
        received_bytes: stats.received_bytes(),
    };

    let (percent, current, total) = transfer_percent_and_fraction(&transfer);

    let _ = app.emit(
        CLONE_PROGRESS_EVENT,
        CloneProgressEvent {
            operation_id: operation_id.to_string(),
            phase: CloneProgressPhase::Transfer,
            status: Some("Receiving objects".to_string()),
            line: Some(format!(
                "Receiving objects: {}/{} ({} bytes)",
                transfer.received_objects, transfer.total_objects, transfer.received_bytes
            )),
            percent,
            current,
            total,
            transfer: Some(transfer),
            pack: None,
            ref_update: None,
            error_kind: None,
        },
    );
}

fn transfer_percent_and_fraction(
    transfer: &CloneTransferStats,
) -> (Option<f64>, Option<u64>, Option<u64>) {
    if transfer.total_objects == 0 {
        return (None, Some(transfer.received_objects as u64), None);
    }

    let percent = (transfer.received_objects as f64 / transfer.total_objects as f64 * 100.0)
        .clamp(0.0, 100.0);
    (
        Some(percent),
        Some(transfer.received_objects as u64),
        Some(transfer.total_objects as u64),
    )
}

fn clean_partial_clone(destination: &Path) -> Result<(), String> {
    if destination.exists() {
        fs::remove_dir_all(destination).map_err(|e| {
            format!(
                "Failed to clean up partial clone at {}: {e}",
                destination.display()
            )
        })?;
    }
    Ok(())
}

fn classify_clone_error(raw: &str) -> String {
    let normalized = raw.to_ascii_lowercase();

    if normalized.contains("authentication failed")
        || normalized.contains("could not read username")
        || normalized.contains("could not read password")
        || normalized.contains("permission denied (publickey)")
    {
        return "auth".to_string();
    }

    if normalized.contains("repository not found") || normalized.contains("not found") {
        return "not_found".to_string();
    }

    if normalized.contains("could not resolve host")
        || normalized.contains("failed to connect")
        || normalized.contains("operation timed out")
        || normalized.contains("connection timed out")
    {
        return "network".to_string();
    }

    if normalized.contains("cancel") {
        return "cancelled".to_string();
    }

    "unknown".to_string()
}

fn build_clone_error_message(raw: &str, kind: &str) -> String {
    match kind {
        "auth" => {
            "Authentication failed while cloning. Check credentials or SSH key access, then retry."
                .to_string()
        }
        "not_found" => {
            "Repository not found. Verify the URL and your access permissions.".to_string()
        }
        "network" => "Network error while cloning. Check connectivity and try again.".to_string(),
        "cancelled" => "Clone cancelled by user".to_string(),
        _ => {
            let trimmed = raw.trim();
            if trimmed.is_empty() {
                "Clone failed due to an unknown error.".to_string()
            } else {
                trimmed.to_string()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn computes_transfer_percent() {
        let transfer = CloneTransferStats {
            total_objects: 772_251,
            received_objects: 52_308,
            indexed_objects: 0,
            local_objects: 0,
            total_deltas: 0,
            indexed_deltas: 0,
            received_bytes: 31_040_000,
        };

        let (percent, current, total) = transfer_percent_and_fraction(&transfer);
        assert_eq!(current, Some(52_308));
        assert_eq!(total, Some(772_251));
        assert_eq!(
            percent,
            Some((52_308.0_f64 / 772_251.0_f64 * 100.0).clamp(0.0, 100.0))
        );
    }

    #[test]
    fn classifies_auth_errors() {
        let error = "fatal: Authentication failed for https://example.com/repo.git";
        assert_eq!(classify_clone_error(error), "auth");
    }

    #[test]
    fn classifies_not_found_errors() {
        let error = "fatal: repository 'https://example.com/nope.git' not found";
        assert_eq!(classify_clone_error(error), "not_found");
    }
}
