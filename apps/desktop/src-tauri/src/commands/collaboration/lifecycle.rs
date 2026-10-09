//! One owned desktop runtime generation, including failed startup and recovery.
use super::*;
use std::{
    ops::Deref,
    path::{Path, PathBuf},
    sync::Mutex as StdMutex,
};
use tauri::{AppHandle, Emitter, Manager};

pub(crate) struct RuntimeSlot {
    phase: StdMutex<Phase>,
    changed: tokio::sync::Notify,
}
enum Phase {
    Starting,
    Running(Arc<CollaborationRuntime>),
    Failed(CollaborationError),
    Quiescing,
    Recovery {
        id: String,
        previous: Option<Arc<CollaborationRuntime>>,
    },
}
impl Default for RuntimeSlot {
    fn default() -> Self {
        Self {
            phase: StdMutex::new(Phase::Starting),
            changed: tokio::sync::Notify::new(),
        }
    }
}
impl RuntimeSlot {
    pub fn get(&self) -> Option<Result<Arc<CollaborationRuntime>, CollaborationError>> {
        Some(match &*self.phase.lock().ok()? {
            Phase::Starting => return None,
            Phase::Running(runtime) => Ok(runtime.clone()),
            Phase::Failed(error) => Err(error.clone()),
            Phase::Quiescing | Phase::Recovery { .. } => Err(paused()),
        })
    }
    pub fn set(
        &self,
        result: Result<Arc<CollaborationRuntime>, CollaborationError>,
    ) -> Result<(), CollaborationError> {
        let mut phase = self
            .phase
            .lock()
            .map_err(|_| CollaborationError::storage())?;
        if !matches!(*phase, Phase::Starting) {
            return Err(paused());
        }
        *phase = match result {
            Ok(runtime) => Phase::Running(runtime),
            Err(error) => Phase::Failed(error),
        };
        self.changed.notify_waiters();
        Ok(())
    }
}
/// Held by every normal IPC through its final await. No borrowed OnceCell
/// reference or unleased runtime escapes `CollaborationState::get`.
pub(crate) struct RuntimeLease {
    runtime: Arc<CollaborationRuntime>,
    _operation: collaboration::runtime::RuntimeOperation,
}
impl Deref for RuntimeLease {
    type Target = Arc<CollaborationRuntime>;
    fn deref(&self) -> &Self::Target {
        &self.runtime
    }
}
pub(crate) struct RecoveryTransition {
    id: String,
    target: PathBuf,
}
impl RecoveryTransition {
    pub fn target_path(&self) -> &Path {
        &self.target
    }
}
fn paused() -> CollaborationError {
    CollaborationError::new(
        ErrorCode::NotReady,
        "Collaboration is paused for storage recovery",
    )
}
impl CollaborationState {
    pub(crate) fn configured_database_path(&self) -> Option<&Path> {
        self.native_database_path.get().map(PathBuf::as_path)
    }
    pub(crate) fn configure_database_path(&self, path: PathBuf) -> Result<(), CollaborationError> {
        if let Some(configured) = self.native_database_path.get() {
            return if configured == &path {
                Ok(())
            } else {
                Err(paused())
            };
        }
        match self.native_database_path.set(path) {
            Ok(()) => Ok(()),
            Err(path) if self.native_database_path.get() == Some(&path) => Ok(()),
            Err(_) => Err(paused()),
        }
    }

    pub(crate) async fn get(&self) -> Result<RuntimeLease, CollaborationError> {
        for _ in 0..100 {
            if let Some(result) = self.runtime.get() {
                let runtime = result?;
                let operation = runtime.acquire_operation()?;
                return Ok(RuntimeLease {
                    runtime,
                    _operation: operation,
                });
            }
            tokio::time::sleep(std::time::Duration::from_millis(25)).await;
        }
        Err(CollaborationError::new(
            ErrorCode::NotReady,
            "Collaboration storage is still starting",
        ))
    }
    pub(crate) fn own_service(&self, service: tokio::task::JoinHandle<()>) {
        if let Ok(mut services) = self.services.lock() {
            services.push(service);
        } else {
            service.abort();
        }
    }
    pub(crate) async fn enter_recovery(
        &self,
        app: &AppHandle,
    ) -> Result<RecoveryTransition, CollaborationError> {
        let app = app.clone();
        tokio::spawn(async move {
            app.state::<CollaborationState>()
                .enter_recovery_owned(&app)
                .await
        })
        .await
        .map_err(|_| CollaborationError::storage())?
    }
    async fn enter_recovery_owned(
        &self,
        app: &AppHandle,
    ) -> Result<RecoveryTransition, CollaborationError> {
        let _transition = self.transition.lock().await;
        let target = crate::collaboration_setup::database_path(app)?;
        // Startup owns its connection until it publishes success/failure. Never
        // abort migrations or race recovery against a pending startup writer.
        loop {
            let changed = self.runtime.changed.notified();
            tokio::pin!(changed);
            changed.as_mut().enable();
            if !matches!(
                *self
                    .runtime
                    .phase
                    .lock()
                    .map_err(|_| CollaborationError::storage())?,
                Phase::Starting
            ) {
                break;
            }
            changed.await;
        }
        let previous = {
            let mut phase = self
                .runtime
                .phase
                .lock()
                .map_err(|_| CollaborationError::storage())?;
            let previous = match &*phase {
                Phase::Running(runtime) => Some(runtime.clone()),
                Phase::Failed(_) => None,
                _ => return Err(paused()),
            };
            if let Some(runtime) = &previous {
                runtime.stop_admission();
            }
            *phase = Phase::Quiescing;
            previous
        };
        let _ = app.emit("gitru:collaboration-runtime-reset", ());
        let services = std::mem::take(
            &mut *self
                .services
                .lock()
                .map_err(|_| CollaborationError::storage())?,
        );
        for service in &services {
            service.abort();
        }
        for service in services {
            let _ = service.await;
        }
        self.demand_hosts.lock().await.clear();
        if let Some(runtime) = &previous {
            if let Err(error) = runtime.shutdown().await {
                *self
                    .runtime
                    .phase
                    .lock()
                    .map_err(|_| CollaborationError::storage())? = Phase::Failed(error.clone());
                return Err(error);
            }
        }
        let id = uuid::Uuid::new_v4().to_string();
        *self
            .runtime
            .phase
            .lock()
            .map_err(|_| CollaborationError::storage())? = Phase::Recovery {
            id: id.clone(),
            previous,
        };
        Ok(RecoveryTransition { id, target })
    }
    pub(crate) async fn resume(
        &self,
        app: &AppHandle,
        transition: RecoveryTransition,
    ) -> Result<(), CollaborationError> {
        let app = app.clone();
        tokio::spawn(async move {
            app.state::<CollaborationState>()
                .resume_owned(&app, transition)
                .await
        })
        .await
        .map_err(|_| CollaborationError::storage())?
    }
    async fn resume_owned(
        &self,
        app: &AppHandle,
        transition: RecoveryTransition,
    ) -> Result<(), CollaborationError> {
        let _transition = self.transition.lock().await;
        if transition.target != crate::collaboration_setup::database_path(app)? {
            return Err(paused());
        }
        let previous = {
            let mut phase = self
                .runtime
                .phase
                .lock()
                .map_err(|_| CollaborationError::storage())?;
            let previous = match &*phase {
                Phase::Recovery { id, previous } if id == &transition.id => previous.clone(),
                _ => return Err(paused()),
            };
            *phase = Phase::Starting;
            previous
        };
        let result = crate::collaboration_setup::build_runtime(app, previous.as_deref()).await;
        if let Ok(runtime) = &result {
            crate::collaboration_setup::start_services(app, runtime.clone());
        }
        let ready = result.as_ref().map(|_| ()).map_err(Clone::clone);
        self.runtime.set(result)?;
        if ready.is_ok() {
            let _ = app.emit("gitru:collaboration-runtime-reset", ());
        }
        ready
    }
}
