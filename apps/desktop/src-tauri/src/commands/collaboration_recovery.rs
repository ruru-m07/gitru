//! Native file pickers and caller-bound, expiring recovery sessions.
//! Once a transition starts, a native-owned task finishes it even if IPC cancels.
use super::{
    collaboration::{authorize, CollaborationState, Operation, RecoveryTransition},
    collaboration_local_links::CallerProof,
};
use collaboration::{
    recovery::{
        BackupSummary, InterruptedPreview, InterruptedRecovery, RecoverySession, RestoreChoice,
        RestorePreview,
    },
    CollaborationError, ErrorCode,
};
use serde::{Deserialize, Serialize};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};
use tauri::{AppHandle, Manager, State, Webview};
use tauri_plugin_dialog::DialogExt;
use tokio::sync::{Mutex, OwnedMutexGuard};

const PREVIEW_TTL: Duration = Duration::from_secs(10 * 60);

#[derive(Default)]
pub struct RecoveryUiState {
    gate: Arc<Mutex<()>>,
    prepared: Mutex<Option<Prepared>>,
    #[cfg(all(feature = "e2e", not(feature = "collaboration-harness")))]
    test_picker: Mutex<Option<(bool, std::path::PathBuf)>>,
    #[cfg(all(feature = "e2e", not(feature = "collaboration-harness")))]
    test_backup: Mutex<Option<std::path::PathBuf>>,
}

enum Candidate {
    Restore(RecoverySession),
    Interrupted(InterruptedRecovery),
}

struct Prepared {
    id: String,
    owner: CallerProof,
    expires: Instant,
    transition: RecoveryTransition,
    candidate: Candidate,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollaborationRecoveryPreview {
    pub session_id: String,
    pub restore: Option<RestorePreview>,
    pub interrupted: Option<InterruptedPreview>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CollaborationRecoveryAction {
    ReplaceCurrentData,
    KeepOriginalData,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfirmCollaborationRecovery {
    pub session_id: String,
    pub confirmation_id: String,
    pub action: CollaborationRecoveryAction,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollaborationRecoveryResult {
    pub runtime_ready: bool,
    pub originals_preserved: bool,
}

fn busy() -> CollaborationError {
    CollaborationError::new(
        ErrorCode::Busy,
        "Another backup or recovery is already open",
    )
}
fn stale() -> CollaborationError {
    CollaborationError::new(
        ErrorCode::StaleView,
        "Inspect the backup again before continuing",
    )
}
fn require_fresh_deadline(expires: Instant, now: Instant) -> Result<(), CollaborationError> {
    if now >= expires {
        return Err(stale());
    }
    Ok(())
}
fn session_matches(prepared: &Prepared, id: &str, owner: &CallerProof) -> bool {
    owns_preview(
        &prepared.id,
        &prepared.owner,
        prepared.expires,
        id,
        owner,
        Instant::now(),
    )
}
fn owns_preview(
    expected_id: &str,
    expected_owner: &CallerProof,
    expires: Instant,
    id: &str,
    owner: &CallerProof,
    now: Instant,
) -> bool {
    expected_id == id && expected_owner == owner && now < expires
}

impl RecoveryUiState {
    fn acquire(&self) -> Result<OwnedMutexGuard<()>, CollaborationError> {
        self.gate.clone().try_lock_owned().map_err(|_| busy())
    }
    async fn require_empty(&self) -> Result<(), CollaborationError> {
        if self.prepared.lock().await.is_some() {
            return Err(busy());
        }
        Ok(())
    }
}

async fn resume(app: &AppHandle, transition: RecoveryTransition) -> bool {
    app.state::<CollaborationState>()
        .resume(app, transition)
        .await
        .is_ok()
}

async fn pick_file(
    view: &Webview,
    state: &RecoveryUiState,
    restore: bool,
) -> Result<Option<std::path::PathBuf>, CollaborationError> {
    #[cfg(all(feature = "e2e", not(feature = "collaboration-harness")))]
    if let Some((expected_restore, path)) = state.test_picker.lock().await.take() {
        if expected_restore != restore {
            return Err(stale());
        }
        return Ok(Some(path));
    }
    #[cfg(not(all(feature = "e2e", not(feature = "collaboration-harness"))))]
    let _ = state;
    let (send, receive) = tokio::sync::oneshot::channel();
    let dialog = view
        .dialog()
        .file()
        .set_parent(&view.window())
        .add_filter("Gitru backup", &["sqlite3", "sqlite", "db"]);
    if restore {
        dialog
            .set_title("Choose a collaboration backup")
            .pick_file(move |path| {
                let _ = send.send(path);
            });
    } else {
        dialog
            .set_title("Back up collaboration data")
            .set_file_name("gitru-collaboration.sqlite3")
            .save_file(move |path| {
                let _ = send.send(path);
            });
    }
    receive
        .await
        .map_err(|_| CollaborationError::storage())?
        .map(|path| path.into_path().map_err(|_| CollaborationError::storage()))
        .transpose()
}

/// The ordinary packaged E2E application substitutes one fixed native picker
/// result. It accepts no paths and never exists in production or the harness.
#[cfg(all(feature = "e2e", not(feature = "collaboration-harness")))]
#[tauri::command]
pub async fn collaboration_e2e_recovery_picker(
    restore: bool,
    view: Webview,
    app: AppHandle,
    state: State<'_, RecoveryUiState>,
) -> Result<(), CollaborationError> {
    authorize(&view, Operation::Recovery)?;
    if view.label() != "main" || app.config().identifier != "com.ruru.gitru.e2e" {
        return Err(stale());
    }
    CallerProof::capture(&view, &app)?.validate_under_writer(&app)?;
    let _gate = state.acquire()?;
    state.require_empty().await?;
    let mut selected = state.test_backup.lock().await;
    let path = if restore {
        selected.as_ref().ok_or_else(stale)?.clone()
    } else {
        let path = app
            .path()
            .app_data_dir()
            .map_err(|_| CollaborationError::storage())?
            .join(format!(
                "recovery-picker-fixture-{}.sqlite3",
                uuid::Uuid::new_v4()
            ));
        *selected = Some(path.clone());
        path
    };
    *state.test_picker.lock().await = Some((restore, path));
    Ok(())
}

// A vanished tab must not leave the entire collaboration runtime paused. The
// native session retains authority; a renderer timer cannot extend its lease.
fn watch_preview(app: AppHandle, id: String) {
    tauri::async_runtime::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(2)).await;
            let state = app.state::<RecoveryUiState>();
            let Ok(_gate) = state.acquire() else {
                continue;
            };
            let mut slot = state.prepared.lock().await;
            let Some(prepared) = slot.as_ref().filter(|value| value.id == id) else {
                return;
            };
            if Instant::now() < prepared.expires
                && prepared.owner.validate_under_writer(&app).is_ok()
            {
                continue;
            }
            let prepared = slot.take().expect("checked native recovery session");
            drop(slot);
            drop(prepared.candidate);
            let _ = resume(&app, prepared.transition).await;
            return;
        }
    });
}

async fn prepare(
    app: AppHandle,
    owner: CallerProof,
    selected: Option<std::path::PathBuf>,
    _gate: OwnedMutexGuard<()>,
) -> Result<CollaborationRecoveryPreview, CollaborationError> {
    let ui = app.state::<RecoveryUiState>();
    ui.require_empty().await?;
    owner.validate_under_writer(&app)?;
    let transition = app
        .state::<CollaborationState>()
        .enter_recovery(&app)
        .await?;
    let candidate = if let Some(path) = selected {
        RecoverySession::prepare(transition.target_path(), path)
            .await
            .map(Candidate::Restore)
    } else {
        let path = transition.target_path().to_owned();
        tokio::task::spawn_blocking(move || InterruptedRecovery::inspect(path))
            .await
            .map_err(|_| CollaborationError::storage())
            .and_then(|result| result)
            .map(Candidate::Interrupted)
    };
    let candidate = match candidate {
        Ok(candidate) => candidate,
        Err(error) => {
            let _ = resume(&app, transition).await;
            return Err(error);
        }
    };
    if let Err(error) = owner.validate_under_writer(&app) {
        drop(candidate);
        let _ = resume(&app, transition).await;
        return Err(error);
    }
    let id = uuid::Uuid::new_v4().to_string();
    let preview = CollaborationRecoveryPreview {
        session_id: id.clone(),
        restore: match &candidate {
            Candidate::Restore(session) => Some(session.preview().clone()),
            _ => None,
        },
        interrupted: match &candidate {
            Candidate::Interrupted(session) => Some(session.preview().clone()),
            _ => None,
        },
    };
    *ui.prepared.lock().await = Some(Prepared {
        id: id.clone(),
        owner,
        expires: Instant::now() + PREVIEW_TTL,
        transition,
        candidate,
    });
    watch_preview(app.clone(), id);
    Ok(preview)
}

#[tauri::command]
pub async fn collaboration_backup(
    view: Webview,
    app: AppHandle,
    state: State<'_, RecoveryUiState>,
) -> Result<Option<BackupSummary>, CollaborationError> {
    authorize(&view, Operation::Recovery)?;
    let owner = CallerProof::capture(&view, &app)?;
    let gate = state.acquire()?;
    state.require_empty().await?;
    let Some(path) = pick_file(&view, &state, false).await? else {
        return Ok(None);
    };
    owner.validate(&view, &app)?;
    tauri::async_runtime::spawn(async move {
        let _gate = gate;
        owner.validate_under_writer(&app)?;
        let state = app.state::<CollaborationState>();
        let runtime = state.get().await?;
        runtime.store().backup_to(path).await.map(Some)
    })
    .await
    .map_err(|_| CollaborationError::storage())?
}

#[tauri::command]
pub async fn collaboration_prepare_restore(
    view: Webview,
    app: AppHandle,
    state: State<'_, RecoveryUiState>,
) -> Result<Option<CollaborationRecoveryPreview>, CollaborationError> {
    authorize(&view, Operation::Recovery)?;
    let owner = CallerProof::capture(&view, &app)?;
    let gate = state.acquire()?;
    state.require_empty().await?;
    let Some(path) = pick_file(&view, &state, true).await? else {
        return Ok(None);
    };
    owner.validate(&view, &app)?;
    tauri::async_runtime::spawn(prepare(app, owner, Some(path), gate))
        .await
        .map_err(|_| CollaborationError::storage())?
        .map(Some)
}

#[tauri::command]
pub async fn collaboration_inspect_interrupted_recovery(
    view: Webview,
    app: AppHandle,
    state: State<'_, RecoveryUiState>,
) -> Result<CollaborationRecoveryPreview, CollaborationError> {
    authorize(&view, Operation::Recovery)?;
    let owner = CallerProof::capture(&view, &app)?;
    let gate = state.acquire()?;
    state.require_empty().await?;
    tauri::async_runtime::spawn(prepare(app, owner, None, gate))
        .await
        .map_err(|_| CollaborationError::storage())?
}

#[tauri::command]
pub async fn collaboration_cancel_recovery(
    session_id: String,
    view: Webview,
    app: AppHandle,
    state: State<'_, RecoveryUiState>,
) -> Result<CollaborationRecoveryResult, CollaborationError> {
    authorize(&view, Operation::Recovery)?;
    let owner = CallerProof::capture(&view, &app)?;
    let gate = state.acquire()?;
    tauri::async_runtime::spawn(async move {
        let _gate = gate;
        let state = app.state::<RecoveryUiState>();
        let mut slot = state.prepared.lock().await;
        if !slot
            .as_ref()
            .is_some_and(|value| session_matches(value, &session_id, &owner))
        {
            return Err(stale());
        }
        let prepared = slot.take().ok_or_else(stale)?;
        drop(slot);
        drop(prepared.candidate);
        Ok(CollaborationRecoveryResult {
            runtime_ready: resume(&app, prepared.transition).await,
            originals_preserved: true,
        })
    })
    .await
    .map_err(|_| CollaborationError::storage())?
}

#[tauri::command]
pub async fn collaboration_confirm_recovery(
    request: ConfirmCollaborationRecovery,
    view: Webview,
    app: AppHandle,
    state: State<'_, RecoveryUiState>,
) -> Result<CollaborationRecoveryResult, CollaborationError> {
    authorize(&view, Operation::Recovery)?;
    let owner = CallerProof::capture(&view, &app)?;
    let gate = state.acquire()?;
    tauri::async_runtime::spawn(async move {
        let _gate = gate;
        let state = app.state::<RecoveryUiState>();
        let mut slot = state.prepared.lock().await;
        let prepared = slot
            .as_ref()
            .filter(|value| session_matches(value, &request.session_id, &owner))
            .ok_or_else(stale)?;
        let (confirmation, action) = match &prepared.candidate {
            Candidate::Restore(value) => (
                &value.preview().confirmation_id,
                CollaborationRecoveryAction::ReplaceCurrentData,
            ),
            Candidate::Interrupted(value) => (
                &value.preview().confirmation_id,
                CollaborationRecoveryAction::KeepOriginalData,
            ),
        };
        if confirmation != &request.confirmation_id || action != request.action {
            return Err(stale());
        }
        owner.validate_under_writer(&app)?;
        let prepared = slot.take().ok_or_else(stale)?;
        drop(slot);
        let transition = prepared.transition;
        let blocking_app = app.clone();
        let result = tokio::task::spawn_blocking(move || {
            // Revalidate immediately before the first confirmed mutation. Once
            // it starts, finish the recovery protocol despite renderer closure.
            require_fresh_deadline(prepared.expires, Instant::now())?;
            owner.validate_under_writer(&blocking_app)?;
            match prepared.candidate {
                Candidate::Restore(value) => value
                    .confirm(&request.confirmation_id, RestoreChoice::ReplaceCurrentData)
                    .map(|_| ()),
                Candidate::Interrupted(value) => value
                    .confirm(&request.confirmation_id, RestoreChoice::KeepOriginalData)
                    .map(|_| ()),
            }
        });
        // Keep a native app handle outside the blocking closure for restart.
        let result = result
            .await
            .map_err(|_| CollaborationError::storage())
            .and_then(|value| value);
        let ready = resume(&app, transition).await;
        result?;
        Ok(CollaborationRecoveryResult {
            runtime_ready: ready,
            originals_preserved: true,
        })
    })
    .await
    .map_err(|_| CollaborationError::storage())?
}

#[cfg(test)]
mod tests {
    use super::*;

    fn owner() -> CallerProof {
        CallerProof {
            label: "tab-webview:one".into(),
            url: "tauri://localhost/app/issues".into(),
            owners: vec![("repo".into(), 1)],
            native_identity: 42,
            incarnation: 3,
        }
    }

    #[test]
    fn recovery_preview_rejects_another_tab_recreated_owner_or_expired_proof() {
        let now = Instant::now();
        let proof = owner();
        let until = now + PREVIEW_TTL;
        assert!(owns_preview("one", &proof, until, "one", &proof, now));
        assert!(!owns_preview("one", &proof, until, "two", &proof, now));
        assert!(!owns_preview("one", &proof, until, "one", &proof, until));
        let mut other = proof.clone();
        other.label = "tab-webview:two".into();
        assert!(!owns_preview("one", &proof, until, "one", &other, now));
        let mut replaced = proof.clone();
        replaced.incarnation += 1;
        assert!(!owns_preview("one", &proof, until, "one", &replaced, now));
        let mut detached = proof.clone();
        detached.owners[0].1 += 1;
        assert!(!owns_preview("one", &proof, until, "one", &detached, now));
    }

    #[test]
    fn queued_confirmation_rechecks_its_deadline_before_mutation() {
        let accepted_at = Instant::now();
        let expires = accepted_at + Duration::from_secs(1);
        assert!(require_fresh_deadline(expires, accepted_at).is_ok());
        assert_eq!(
            require_fresh_deadline(expires, expires).unwrap_err().code,
            ErrorCode::StaleView
        );
        assert_eq!(
            require_fresh_deadline(expires, expires + Duration::from_secs(1))
                .unwrap_err()
                .code,
            ErrorCode::StaleView
        );
    }

    #[tokio::test]
    async fn one_native_gate_serializes_picker_preparation_and_confirmation_across_views() {
        let state = RecoveryUiState::default();
        let first = state.acquire().unwrap();
        assert_eq!(state.acquire().unwrap_err().code, ErrorCode::Busy);
        drop(first);
        assert!(state.acquire().is_ok());
    }
}
