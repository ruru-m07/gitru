//! Only the retained packaged lane can construct this native controller.
pub mod domain;
pub use domain::*;

use crate::commands::collaboration::CollaborationState;
#[cfg(feature = "native-keyed-storage")]
use collaboration::database_keys::{
    DatabaseCreation, DatabaseKey, DatabaseKeyError, DatabaseKeyIdentity, DatabaseKeyVault,
};
use collaboration::{
    test_harness::*, ChangeHint, CollaborationError, CollaborationRuntime, ErrorCode,
};
use serde::Deserialize;
use std::{
    collections::VecDeque,
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::PathBuf,
    sync::{Arc, LazyLock, Mutex},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use tauri::{
    App, AppHandle, Emitter, EventTarget, Manager, Webview, WebviewBuilder, WebviewUrl, Window,
};
use tokio::sync::{Notify, OnceCell};

const ROOT_ENV: &str = "GITRU_COLLABORATION_HARNESS_ROOT";
const NONCE_ENV: &str = "GITRU_COLLABORATION_HARNESS_RUN_NONCE";
const PERFORMANCE_ENV: &str = "GITRU_COLLABORATION_PERFORMANCE";
const MAX_HINTS: usize = 128;
const MAX_PERFORMANCE_QUERIES: usize = 512;
const READ_TIMEOUT: Duration = Duration::from_secs(15);

pub fn invalid() -> CollaborationError {
    CollaborationError::invalid("Invalid native collaboration harness input")
}
pub fn denied() -> CollaborationError {
    CollaborationError::new(
        ErrorCode::PermissionDenied,
        "The native collaboration harness controller is restricted to its main local view",
    )
}
fn stale() -> CollaborationError {
    CollaborationError::new(
        ErrorCode::StaleView,
        "The native collaboration harness incarnation changed",
    )
}

#[derive(Deserialize)]
struct Marker {
    version: u32,
    application_id: String,
    run_nonce: String,
    #[serde(default)]
    storage_mode: HarnessStorageMode,
}
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
enum HarnessStorageMode {
    #[default]
    Plaintext,
    Keyed,
}
#[derive(Clone)]
struct LaunchRoot {
    root: PathBuf,
    nonce: String,
    storage_mode: HarnessStorageMode,
    #[cfg(unix)]
    identity: (u64, u64),
}
impl LaunchRoot {
    fn open(identifier: &str, root: PathBuf, nonce: String) -> Result<Self, CollaborationError> {
        if identifier != APPLICATION_ID || uuid::Uuid::parse_str(&nonce).is_err() {
            return Err(invalid());
        }
        let metadata = fs::symlink_metadata(&root).map_err(|_| invalid())?;
        if !metadata.is_dir()
            || metadata.file_type().is_symlink()
            || fs::canonicalize(&root).map_err(|_| invalid())? != root
        {
            return Err(invalid());
        }
        #[cfg(unix)]
        let identity = {
            use std::os::unix::fs::MetadataExt;
            if metadata.mode() & 0o077 != 0 {
                return Err(invalid());
            }
            (metadata.dev(), metadata.ino())
        };
        let marker: Marker = serde_json::from_slice(
            &File::open(root.join("run.json"))
                .and_then(|file| {
                    if file.metadata()?.len() > 4096 {
                        return Err(std::io::Error::new(
                            std::io::ErrorKind::InvalidData,
                            "marker",
                        ));
                    }
                    let mut bytes = vec![];
                    file.take(4097).read_to_end(&mut bytes)?;
                    if bytes.len() > 4096 {
                        return Err(std::io::Error::new(
                            std::io::ErrorKind::InvalidData,
                            "marker",
                        ));
                    }
                    Ok(bytes)
                })
                .map_err(|_| invalid())?,
        )
        .map_err(|_| invalid())?;
        if marker.version != 1
            || marker.application_id != APPLICATION_ID
            || marker.run_nonce != nonce
        {
            return Err(invalid());
        }
        let launch = Self {
            root,
            nonce,
            storage_mode: marker.storage_mode,
            #[cfg(unix)]
            identity,
        };
        launch.check()?;
        Ok(launch)
    }
    fn file(&self, name: &str) -> Result<PathBuf, CollaborationError> {
        let path = self.root.join(name);
        match fs::symlink_metadata(&path) {
            Ok(metadata) if !metadata.is_file() || metadata.file_type().is_symlink() => {
                Err(invalid())
            }
            Ok(_) => Ok(path),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(path),
            Err(_) => Err(invalid()),
        }
    }
    fn check(&self) -> Result<(), CollaborationError> {
        let metadata = fs::symlink_metadata(&self.root).map_err(|_| invalid())?;
        if !metadata.is_dir() || metadata.file_type().is_symlink() {
            return Err(invalid());
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            if (metadata.dev(), metadata.ino()) != self.identity || metadata.mode() & 0o077 != 0 {
                return Err(invalid());
            }
        }
        let marker: Marker =
            serde_json::from_slice(&self.read_bounded("run.json")?.ok_or_else(invalid)?)
                .map_err(|_| invalid())?;
        if marker.version != 1
            || marker.application_id != APPLICATION_ID
            || marker.run_nonce != self.nonce
            || marker.storage_mode != self.storage_mode
        {
            return Err(invalid());
        }
        self.file("crash-checkpoint.json")?;
        Ok(())
    }
    fn read_bounded(&self, name: &str) -> Result<Option<Vec<u8>>, CollaborationError> {
        let file = match File::open(self.file(name)?) {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(_) => return Err(invalid()),
        };
        if file.metadata().map_err(|_| invalid())?.len() > 4096 {
            return Err(invalid());
        }
        let mut bytes = vec![];
        file.take(4097)
            .read_to_end(&mut bytes)
            .map_err(|_| invalid())?;
        if bytes.len() > 4096 {
            return Err(invalid());
        }
        Ok(Some(bytes))
    }
    fn retained_checkpoint(&self) -> Result<Option<HarnessCheckpoint>, CollaborationError> {
        self.check()?;
        let Some(bytes) = self.read_bounded("crash-checkpoint.json")? else {
            return Ok(None);
        };
        let checkpoint: HarnessCheckpoint =
            serde_json::from_slice(&bytes).map_err(|_| invalid())?;
        if checkpoint.run_nonce != self.nonce
            || uuid::Uuid::parse_str(&checkpoint.session_id).is_err()
            || checkpoint.scenario_generation.parse::<u64>().is_err()
            || checkpoint.process_id == 0
            || match checkpoint.kind {
                HarnessCheckpointKind::BeforeCommit => {
                    checkpoint
                        .gate_id
                        .as_deref()
                        .is_none_or(|id| uuid::Uuid::parse_str(id).is_err())
                        || checkpoint.committed_phase.is_some()
                        || checkpoint.committed_facet_revision.is_some()
                }
                HarnessCheckpointKind::CommittedBeforeHint => {
                    checkpoint.gate_id.is_some()
                        || !matches!(
                            checkpoint.committed_phase,
                            Some(HarnessPhase::One | HarnessPhase::Two)
                        )
                        || checkpoint
                            .committed_facet_revision
                            .as_deref()
                            .is_none_or(|revision| revision.parse::<u64>().is_err())
                }
            }
        {
            return Err(invalid());
        }
        Ok(Some(checkpoint))
    }
    fn checkpoint(&self, value: &HarnessCheckpoint) -> Result<(), CollaborationError> {
        self.check()?;
        let destination = self.file("crash-checkpoint.json")?;
        let temporary = self.file(&format!(".checkpoint-{}.tmp", uuid::Uuid::new_v4()))?;
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let result = (|| {
            let mut file = options.open(&temporary).map_err(|_| invalid())?;
            file.write_all(&serde_json::to_vec(value).map_err(|_| invalid())?)
                .map_err(|_| invalid())?;
            file.sync_all().map_err(|_| invalid())?;
            drop(file);
            self.check()?;
            fs::rename(&temporary, destination).map_err(|_| invalid())?;
            #[cfg(unix)]
            File::open(&self.root)
                .and_then(|file| file.sync_all())
                .map_err(|_| invalid())?;
            Ok(())
        })();
        if result.is_err() {
            let _ = fs::remove_file(temporary);
        }
        result
    }
}

#[cfg(feature = "native-keyed-storage")]
struct HarnessDatabaseVault(LaunchRoot);

#[cfg(feature = "native-keyed-storage")]
impl HarnessDatabaseVault {
    fn name(identity: &DatabaseKeyIdentity) -> String {
        format!(
            "database-key-{}-{}.bin",
            identity.database_id(),
            identity.generation()
        )
    }
}

#[cfg(feature = "native-keyed-storage")]
impl DatabaseKeyVault for HarnessDatabaseVault {
    fn load(
        &self,
        identity: &DatabaseKeyIdentity,
    ) -> Result<Option<DatabaseKey>, DatabaseKeyError> {
        self.0
            .check()
            .map_err(|_| DatabaseKeyError::VaultUnavailable)?;
        let bytes = match self
            .0
            .read_bounded(&Self::name(identity))
            .map_err(|_| DatabaseKeyError::VaultUnavailable)?
        {
            Some(bytes) => bytes,
            None => return Ok(None),
        };
        Ok(Some(DatabaseKey::from_bytes(
            bytes
                .try_into()
                .map_err(|_| DatabaseKeyError::VaultInvalidKey)?,
        )))
    }

    fn store_new(
        &self,
        identity: &DatabaseKeyIdentity,
        key: &DatabaseKey,
    ) -> Result<(), DatabaseKeyError> {
        self.0
            .check()
            .map_err(|_| DatabaseKeyError::VaultUnavailable)?;
        let path = self
            .0
            .file(&Self::name(identity))
            .map_err(|_| DatabaseKeyError::VaultUnavailable)?;
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(path).map_err(|error| match error.kind() {
            std::io::ErrorKind::AlreadyExists => DatabaseKeyError::VaultAlreadyExists,
            _ => DatabaseKeyError::VaultUnavailable,
        })?;
        file.write_all(key.expose())
            .and_then(|()| file.sync_all())
            .map_err(|_| DatabaseKeyError::VaultWriteUncertain)
    }
}

#[derive(Clone)]
struct ChildSurface {
    label: String,
    window_label: String,
}
struct ReadGate {
    receipt: HarnessLocalReadGate,
    owner: crate::commands::collaboration_harness::CapturedReadOwner,
    notify: Arc<Notify>,
    in_flight: bool,
}
struct Projection {
    child: Option<ChildSurface>,
    hint_mode: HarnessHintMode,
    hints: VecDeque<String>,
    reads: Vec<ReadGate>,
    hydrate_requests: u64,
    checkpoint: Option<HarnessCheckpoint>,
}
impl Default for Projection {
    fn default() -> Self {
        Self {
            child: None,
            hint_mode: HarnessHintMode::Normal,
            hints: VecDeque::new(),
            reads: vec![],
            hydrate_requests: 0,
            checkpoint: None,
        }
    }
}
// The command's timeout future disappears if its task is cancelled. A native
// terminal guard releases the finite slot even when the view itself remains.
struct HeldLocalRead<F: FnOnce()> {
    cancel: Option<F>,
}
impl<F: FnOnce()> Drop for HeldLocalRead<F> {
    fn drop(&mut self) {
        if let Some(cancel) = self.cancel.take() {
            cancel();
        }
    }
}
fn cancel_held_return(receipt: &mut HarnessLocalReadGate) -> bool {
    if receipt.state == HarnessReadState::Held {
        receipt.state = HarnessReadState::Cancelled;
        true
    } else {
        false
    }
}

#[derive(Default)]
pub struct HarnessState {
    ready: OnceCell<Result<Arc<NativeHarness>, CollaborationError>>,
}
impl HarnessState {
    pub async fn get(&self) -> Result<&Arc<NativeHarness>, CollaborationError> {
        for _ in 0..100 {
            if let Some(result) = self.ready.get() {
                return result.as_ref().map_err(Clone::clone);
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
        Err(CollaborationError::new(
            ErrorCode::NotReady,
            "The retained native harness is starting",
        ))
    }
}
pub struct NativeHarness {
    launch: LaunchRoot,
    pub runtime: Arc<CollaborationRuntime>,
    control: HarnessControl,
    projection: Mutex<Projection>,
    operations: tokio::sync::Mutex<()>,
    native_setup_started_epoch_ms: u128,
    runtime_ready_epoch_ms: u128,
    runtime_open_micros: u128,
}

#[derive(Default)]
struct PerformanceQueryState {
    next: u64,
    timings: VecDeque<HarnessQueryTiming>,
}
static PERFORMANCE_QUERIES: LazyLock<Mutex<PerformanceQueryState>> =
    LazyLock::new(|| Mutex::new(PerformanceQueryState::default()));

fn epoch_millis() -> Result<u128, CollaborationError> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .map_err(|_| invalid())
}

pub(crate) fn record_performance_query(
    webview_label: &str,
    kind: HarnessQueryKind,
    elapsed: Duration,
    result_count: usize,
) {
    if std::env::var(PERFORMANCE_ENV).as_deref() != Ok("1") {
        return;
    }
    let Ok(result_count) = u32::try_from(result_count) else {
        return;
    };
    let Ok(mut state) = PERFORMANCE_QUERIES.lock() else {
        return;
    };
    state.next = state.next.saturating_add(1);
    let sequence = state.next.to_string();
    if state.timings.len() == MAX_PERFORMANCE_QUERIES {
        state.timings.pop_front();
    }
    state.timings.push_back(HarnessQueryTiming {
        sequence,
        webview_label: webview_label.into(),
        kind,
        elapsed_micros: elapsed.as_micros().to_string(),
        result_count,
    });
}

fn performance_queries() -> Vec<HarnessQueryTiming> {
    PERFORMANCE_QUERIES
        .lock()
        .map(|state| state.timings.iter().cloned().collect())
        .unwrap_or_default()
}

pub fn setup(app: &App) -> Result<(), Box<dyn std::error::Error>> {
    let setup_started = Instant::now();
    let native_setup_started_epoch_ms = epoch_millis()?;
    // Fail synchronously, before normal managers can use any application data.
    let root = std::env::var_os(ROOT_ENV).ok_or_else(invalid)?;
    let nonce = std::env::var(NONCE_ENV).map_err(|_| invalid())?;
    let launch = LaunchRoot::open(&app.config().identifier, PathBuf::from(root), nonce)?;
    let checkpoint = launch.retained_checkpoint()?;
    app.state::<CollaborationState>()
        .configure_database_path(launch.file("collaboration.sqlite")?)?;
    app.manage(HarnessState::default());
    let handle = app.handle().clone();
    tauri::async_runtime::spawn(async move {
        let visibility_handle = handle.clone();
        let visibility: Arc<dyn Fn(&str) -> bool + Send + Sync> = Arc::new(move |owner| {
            crate::commands::collaboration_demand::owner_window_available(&visibility_handle, owner)
        });
        let session = match launch.storage_mode {
            HarnessStorageMode::Plaintext => {
                HarnessSession::open(&launch.root, &launch.nonce, visibility).await
            }
            HarnessStorageMode::Keyed => {
                #[cfg(feature = "native-keyed-storage")]
                {
                    async {
                        let database = launch.file("collaboration.sqlite")?;
                        HarnessSession::validate_before_store_open(&launch.root, &launch.nonce)?;
                        let vault: Arc<dyn DatabaseKeyVault> =
                            Arc::new(HarnessDatabaseVault(launch.clone()));
                        let store = crate::collaboration_setup::open_keyed_store(
                            database,
                            vault,
                            DatabaseCreation::AllowNew,
                        )
                        .await?;
                        HarnessSession::open_with_store(
                            &launch.root,
                            &launch.nonce,
                            visibility,
                            Arc::new(store),
                        )
                        .await
                    }
                    .await
                }
                #[cfg(not(feature = "native-keyed-storage"))]
                {
                    Err(invalid())
                }
            }
        };
        let result = session.and_then(|session| {
            let runtime_ready_epoch_ms = epoch_millis()?;
            Ok(Arc::new(NativeHarness {
                launch,
                runtime: session.runtime,
                control: session.control,
                projection: Mutex::new(Projection {
                    checkpoint,
                    ..Projection::default()
                }),
                operations: tokio::sync::Mutex::new(()),
                native_setup_started_epoch_ms,
                runtime_ready_epoch_ms,
                runtime_open_micros: setup_started.elapsed().as_micros(),
            }))
        });
        let runtime_result = result
            .as_ref()
            .map(|harness| harness.runtime.clone())
            .map_err(Clone::clone);
        if let Ok(harness) = &result {
            let mut changes = harness.runtime.subscribe();
            let relay = harness.clone();
            let event_handle = handle.clone();
            let relay_task = tokio::spawn(async move {
                loop {
                    match changes.recv().await {
                        Ok(hint) => relay.relay_hint(&event_handle, hint),
                        Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                        Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                    }
                }
            });
            handle.state::<CollaborationState>().own_service(relay_task);
            handle.state::<CollaborationState>().own_service(
                crate::commands::collaboration_demand::observe_window_activity(
                    handle.clone(),
                    harness.runtime.clone(),
                ),
            );
            if harness
                .control
                .status(&harness.launch.nonce)
                .await
                .is_ok_and(|status| {
                    status.prepared && status.fixture != HarnessFixture::Performance
                })
            {
                harness.runtime.clone().start_background();
            }
        }
        let _ = handle
            .state::<CollaborationState>()
            .runtime
            .set(runtime_result);
        let ready = result.as_ref().ok().cloned();
        let _ = handle.state::<HarnessState>().ready.set(result);
        if let Some(harness) = ready {
            if let Ok(revision) = harness.runtime.store().revision().await {
                harness.relay_hint(&handle, ChangeHint { revision });
            }
        } else {
            log::error!("Retained collaboration fixture initialization failed");
        }
    });
    Ok(())
}

pub fn exact_local_origin(address: &url::Url) -> bool {
    address.username().is_empty()
        && address.password().is_none()
        && address.port().is_none()
        && matches!(
            (address.scheme(), address.host_str()),
            ("tauri", Some("localhost"))
                | ("https", Some("tauri.localhost"))
                | ("http", Some("tauri.localhost"))
        )
}

impl NativeHarness {
    fn lock(&self) -> Result<std::sync::MutexGuard<'_, Projection>, CollaborationError> {
        self.projection.lock().map_err(|_| invalid())
    }
    pub fn validate_runtime(&self, app: &AppHandle) -> Result<(), CollaborationError> {
        if app.config().identifier != APPLICATION_ID {
            return Err(denied());
        }
        self.launch.check()?;
        let state = app.state::<CollaborationState>();
        if !state.runtime.get().is_some_and(|result| {
            result
                .as_ref()
                .is_ok_and(|runtime| Arc::ptr_eq(runtime, &self.runtime))
        }) {
            return Err(stale());
        }
        Ok(())
    }
    pub async fn manifest<F>(
        &self,
        app: &AppHandle,
        view: &Webview,
        guard: F,
    ) -> Result<HarnessViewManifest, CollaborationError>
    where
        F: Fn() -> Result<(), CollaborationError>,
    {
        guard()?;
        let status = self.control.status(&self.launch.nonce).await?;
        guard()?;
        let role = if view.label() == "main" {
            HarnessViewRole::Main
        } else {
            let projection = self.lock()?;
            if let Some(child) = projection
                .child
                .as_ref()
                .filter(|child| child.label == view.label())
            {
                if view.window().label() != child.window_label {
                    return Err(denied());
                }
                HarnessViewRole::ConcurrentChild
            } else if view.label().starts_with("tab-webview:") && view.window().label() == "main" {
                HarnessViewRole::NormalTab
            } else {
                return Err(denied());
            }
        };
        self.validate_runtime(app)?;
        Ok(HarnessViewManifest {
            run_nonce: status.run_nonce,
            session_id: status.session_id,
            scenario_generation: status.scenario_generation,
            webview_label: view.label().into(),
            role,
            actors: status.actors,
        })
    }
    pub async fn status<F>(
        &self,
        request: HarnessStatusRequest,
        guard: F,
    ) -> Result<HarnessStatus, CollaborationError>
    where
        F: Fn() -> Result<(), CollaborationError>,
    {
        guard()?;
        let core = self.control.status(&request.run_nonce).await?;
        guard()?;
        let projection = self.lock()?;
        Ok(HarnessStatus {
            core,
            child_label: projection.child.as_ref().map(|child| child.label.clone()),
            hint_mode: projection.hint_mode,
            held_hint_revisions: projection.hints.iter().cloned().collect(),
            local_reads: projection
                .reads
                .iter()
                .map(|gate| gate.receipt.clone())
                .collect(),
            authorized_hydrate_requests: projection.hydrate_requests.to_string(),
            checkpoint: projection.checkpoint.clone(),
            process_id: std::process::id(),
            native_setup_started_epoch_ms: self.native_setup_started_epoch_ms.to_string(),
            runtime_ready_epoch_ms: self.runtime_ready_epoch_ms.to_string(),
            runtime_open_micros: self.runtime_open_micros.to_string(),
            performance_queries: performance_queries(),
        })
    }
    pub async fn execute<F>(
        &self,
        app: &AppHandle,
        request: HarnessControlRequest,
        guard: F,
    ) -> Result<HarnessReceipt, CollaborationError>
    where
        F: Fn() -> Result<(), CollaborationError> + Send + Sync,
    {
        validate_request(&request)?;
        guard()?;
        let _operation = self.operations.lock().await;
        guard()?;
        let core = self.control.status(&request.run_nonce).await?;
        guard()?;
        if core.scenario_generation != request.expected_generation {
            return Err(stale());
        }
        let mut issued = None;
        match request.action {
            HarnessAction::Core => {
                let receipt = self
                    .control
                    .execute_checked(
                        HarnessCoreRequest {
                            run_nonce: request.run_nonce.clone(),
                            expected_generation: request.expected_generation.clone(),
                            action: request.core_action.ok_or_else(invalid)?,
                            gate_id: request.gate_id.clone(),
                        },
                        &guard,
                    )
                    .await?;
                guard()?;
                if receipt.status.prepared && receipt.status.fixture != HarnessFixture::Performance
                {
                    self.runtime.clone().start_background();
                }
                issued = receipt.gate_id;
                if receipt.status.scenario_generation != core.scenario_generation {
                    self.cancel_reads()?;
                }
            }
            HarnessAction::CreateConcurrentChild => self.create_child(app, &guard).await?,
            HarnessAction::CloseConcurrentChild => self.close_child(app, &guard).await?,
            HarnessAction::ReloadConcurrentChild => {
                let child = self.lock()?.child.clone().ok_or_else(invalid)?;
                self.dispose_child_owner(app, &child.label).await?;
                guard()?;
                self.cancel_reads()?;
                let view = app.get_webview(&child.label).ok_or_else(stale)?;
                view.reload().map_err(|_| invalid())?;
            }
            HarnessAction::HoldChildHints | HarnessAction::DropChildHints => {
                let mut projection = self.lock()?;
                if projection.child.is_none() {
                    return Err(invalid());
                }
                projection.hints.clear();
                projection.hint_mode = if request.action == HarnessAction::HoldChildHints {
                    HarnessHintMode::Hold
                } else {
                    HarnessHintMode::Drop
                };
            }
            HarnessAction::DeliverChildHintsReverse => {
                let (label, hints) = {
                    let mut projection = self.lock()?;
                    if projection.hint_mode != HarnessHintMode::Hold {
                        return Err(invalid());
                    }
                    let label = projection.child.as_ref().ok_or_else(invalid)?.label.clone();
                    (label, projection.hints.drain(..).rev().collect::<Vec<_>>())
                };
                for revision in hints {
                    guard()?;
                    emit_hint(app, &label, ChangeHint { revision });
                }
            }
            HarnessAction::ResumeHints => {
                let mut projection = self.lock()?;
                projection.hint_mode = HarnessHintMode::Normal;
                projection.hints.clear();
            }
            HarnessAction::ArmItemRead
            | HarnessAction::ArmBodyRead
            | HarnessAction::ArmDraftRead => {
                let label = self
                    .lock()?
                    .child
                    .as_ref()
                    .map_or_else(|| "main".into(), |child| child.label.clone());
                let view = app.get_webview(&label).ok_or_else(stale)?;
                if !view.url().is_ok_and(|url| exact_local_origin(&url)) {
                    return Err(denied());
                }
                let owner =
                    crate::commands::collaboration_harness::CapturedReadOwner::capture(&view)?;
                let mut projection = self.lock()?;
                for gate in &mut projection.reads {
                    if !gate.owner.current()
                        && matches!(
                            gate.receipt.state,
                            HarnessReadState::Armed | HarnessReadState::Held
                        )
                    {
                        gate.receipt.state = HarnessReadState::Cancelled;
                        gate.notify.notify_waiters();
                    }
                }
                projection.reads.retain(|gate| {
                    matches!(
                        gate.receipt.state,
                        HarnessReadState::Armed | HarnessReadState::Held
                    ) || gate.in_flight
                });
                if projection.reads.len() >= 2 {
                    return Err(CollaborationError::new(
                        ErrorCode::Busy,
                        "The finite local return gates are full",
                    ));
                }
                let id = uuid::Uuid::new_v4().to_string();
                projection.reads.push(ReadGate {
                    receipt: HarnessLocalReadGate {
                        gate_id: id.clone(),
                        scenario_generation: core.scenario_generation.clone(),
                        webview_label: label,
                        kind: match request.action {
                            HarnessAction::ArmItemRead => HarnessReadKind::Item,
                            HarnessAction::ArmBodyRead => HarnessReadKind::Body,
                            _ => HarnessReadKind::Draft,
                        },
                        state: HarnessReadState::Armed,
                    },
                    owner,
                    notify: Arc::new(Notify::new()),
                    in_flight: false,
                });
                issued = Some(id);
            }
            HarnessAction::ReleaseLocalRead => {
                let mut projection = self.lock()?;
                let gate = projection
                    .reads
                    .iter_mut()
                    .find(|gate| {
                        gate.receipt.gate_id == request.gate_id.as_deref().unwrap_or("")
                            && gate.receipt.scenario_generation == core.scenario_generation
                            && matches!(
                                gate.receipt.state,
                                HarnessReadState::Armed | HarnessReadState::Held
                            )
                    })
                    .ok_or_else(stale)?;
                gate.receipt.state = HarnessReadState::Released;
                gate.notify.notify_waiters();
            }
            HarnessAction::CancelLocalReads => self.cancel_reads()?,
            HarnessAction::CheckpointBeforeCommit
            | HarnessAction::CheckpointCommittedBeforeHint => {
                let checkpoint = if request.action == HarnessAction::CheckpointBeforeCommit {
                    let gate = core
                        .gates
                        .iter()
                        .find(|gate| {
                            gate.gate_id == request.gate_id.as_deref().unwrap_or("")
                                && gate.scenario_generation == core.scenario_generation
                                && gate.state == HarnessGateState::Held
                        })
                        .ok_or_else(stale)?;
                    let call = core
                        .calls
                        .iter()
                        .find(|call| {
                            Some(&call.call_id) == gate.call_id.as_ref()
                                && call.state == HarnessCallState::Held
                                && call.scenario_generation == core.scenario_generation
                        })
                        .ok_or_else(stale)?;
                    if call.facet != "body" || call.slot != HarnessActorSlot::Primary {
                        return Err(invalid());
                    }
                    HarnessCheckpoint {
                        run_nonce: core.run_nonce.clone(),
                        session_id: core.session_id.clone(),
                        scenario_generation: core.scenario_generation.clone(),
                        kind: HarnessCheckpointKind::BeforeCommit,
                        gate_id: Some(gate.gate_id.clone()),
                        committed_phase: None,
                        committed_facet_revision: None,
                        process_id: std::process::id(),
                    }
                } else {
                    let projection = self.lock()?;
                    if projection.hint_mode != HarnessHintMode::Hold
                        || projection.child.is_none()
                        || core.committed_phase != Some(core.phase)
                        || core
                            .committed_facet_revision
                            .as_ref()
                            .is_none_or(|facet_revision| !projection.hints.contains(facet_revision))
                    {
                        return Err(stale());
                    }
                    HarnessCheckpoint {
                        run_nonce: core.run_nonce.clone(),
                        session_id: core.session_id.clone(),
                        scenario_generation: core.scenario_generation.clone(),
                        kind: HarnessCheckpointKind::CommittedBeforeHint,
                        gate_id: None,
                        committed_phase: core.committed_phase,
                        committed_facet_revision: core.committed_facet_revision.clone(),
                        process_id: std::process::id(),
                    }
                };
                guard()?;
                self.launch.checkpoint(&checkpoint)?;
                self.lock()?.checkpoint = Some(checkpoint);
            }
        }
        guard()?;
        Ok(HarnessReceipt {
            status: self
                .status(
                    HarnessStatusRequest {
                        run_nonce: request.run_nonce,
                    },
                    &guard,
                )
                .await?,
            gate_id: issued,
        })
    }
    fn cancel_reads(&self) -> Result<(), CollaborationError> {
        for gate in &mut self.lock()?.reads {
            if matches!(
                gate.receipt.state,
                HarnessReadState::Armed | HarnessReadState::Held
            ) {
                gate.receipt.state = HarnessReadState::Cancelled;
                gate.notify.notify_waiters();
            }
        }
        Ok(())
    }
    pub fn record_hydrate(&self) -> Result<(), CollaborationError> {
        let mut projection = self.lock()?;
        projection.hydrate_requests = projection
            .hydrate_requests
            .checked_add(1)
            .ok_or_else(invalid)?;
        Ok(())
    }
    pub async fn hold_read<F>(
        &self,
        view: &Webview,
        kind: HarnessReadKind,
        account: &str,
        subject: &str,
        guard: F,
    ) -> Result<(), CollaborationError>
    where
        F: Fn() -> Result<(), CollaborationError>,
    {
        guard()?;
        if account != PRIMARY_ACCOUNT || subject != SUBJECT_ID {
            return Ok(());
        }
        let status = self.control.status(&self.launch.nonce).await?;
        guard()?;
        let (id, notify) = {
            let mut projection = self.lock()?;
            let Some(gate) = projection.reads.iter_mut().find(|gate| {
                gate.receipt.kind == kind
                    && gate.receipt.state == HarnessReadState::Armed
                    && gate.receipt.webview_label == view.label()
                    && gate.owner.matches(view)
                    && gate.receipt.scenario_generation == status.scenario_generation
            }) else {
                return Ok(());
            };
            gate.receipt.state = HarnessReadState::Held;
            gate.in_flight = true;
            (gate.receipt.gate_id.clone(), gate.notify.clone())
        };
        let _terminal = HeldLocalRead {
            cancel: Some(|| {
                if let Ok(mut projection) = self.lock() {
                    if let Some(gate) = projection
                        .reads
                        .iter_mut()
                        .find(|gate| gate.receipt.gate_id == id)
                    {
                        gate.in_flight = false;
                        if cancel_held_return(&mut gate.receipt) {
                            gate.notify.notify_waiters();
                        }
                    }
                }
            }),
        };
        let deadline = tokio::time::Instant::now() + READ_TIMEOUT;
        loop {
            let notified = notify.notified();
            tokio::pin!(notified);
            notified.as_mut().enable();
            let current = self
                .lock()?
                .reads
                .iter()
                .find(|gate| gate.receipt.gate_id == id)
                .map(|gate| gate.receipt.state)
                .ok_or_else(stale)?;
            match current {
                HarnessReadState::Released => break,
                HarnessReadState::Cancelled | HarnessReadState::TimedOut => return Err(stale()),
                _ => {}
            }
            if tokio::time::timeout_at(deadline, notified).await.is_err() {
                if let Some(gate) = self
                    .lock()?
                    .reads
                    .iter_mut()
                    .find(|gate| gate.receipt.gate_id == id)
                {
                    gate.receipt.state = HarnessReadState::TimedOut;
                }
                return Err(CollaborationError::new(
                    ErrorCode::StaleView,
                    "The finite native local return gate expired",
                ));
            }
        }
        guard()?;
        if self
            .control
            .status(&self.launch.nonce)
            .await?
            .scenario_generation
            != status.scenario_generation
        {
            return Err(stale());
        }
        guard()
    }
    fn relay_hint(&self, app: &AppHandle, hint: ChangeHint) {
        let withheld = {
            let Ok(mut projection) = self.lock() else {
                return;
            };
            let child = projection.child.as_ref().map(|child| child.label.clone());
            match projection.hint_mode {
                HarnessHintMode::Normal => None,
                HarnessHintMode::Hold => {
                    retain_hint(&mut projection.hints, hint.revision.clone());
                    child
                }
                HarnessHintMode::Drop => child,
            }
        };
        for (label, view) in app.webviews() {
            if withheld.as_deref() != Some(label.as_str())
                && (label == "main" || label.starts_with("tab-webview:"))
                && view.url().is_ok_and(|url| exact_local_origin(&url))
            {
                emit_hint(app, &label, hint.clone());
            }
        }
    }
    async fn create_child<F>(&self, app: &AppHandle, guard: &F) -> Result<(), CollaborationError>
    where
        F: Fn() -> Result<(), CollaborationError>,
    {
        guard()?;
        let suffix = uuid::Uuid::new_v4();
        let child = ChildSurface {
            label: format!("tab-webview:ruru103:{suffix}"),
            window_label: format!("ruru103-surface-{suffix}"),
        };
        {
            let mut projection = self.lock()?;
            if projection.child.is_some() {
                return Err(CollaborationError::new(
                    ErrorCode::Busy,
                    "The concurrent fixture surface already exists",
                ));
            }
            projection.child = Some(child.clone());
        }
        let app_handle = app.clone();
        let create = child.clone();
        let child_url = if std::env::var(PERFORMANCE_ENV).as_deref() == Ok("1") {
            "/app/git?embedded=1&collaborationHarness=1&collaborationPerformance=1"
        } else {
            "/app/git?embedded=1&collaborationHarness=1"
        };
        let result = tauri::async_runtime::spawn_blocking(move || {
            let window = Window::builder(&app_handle, &create.window_label)
                .title("Gitru retained collaboration fixture")
                .inner_size(900.0, 700.0)
                .visible(false)
                .build()
                .map_err(|_| invalid())?;
            let view = match window.add_child(
                WebviewBuilder::new(&create.label, WebviewUrl::App(child_url.into())),
                tauri::LogicalPosition::new(0.0, 0.0),
                tauri::LogicalSize::new(900.0, 700.0),
            ) {
                Ok(view) => view,
                Err(_) => {
                    let _ = window.close();
                    return Err(invalid());
                }
            };
            view.show().map_err(|_| invalid())?;
            window.show().map_err(|_| invalid())?;
            if window.is_minimized().map_err(|_| invalid())? {
                window.unminimize().map_err(|_| invalid())?;
            }
            window.set_focus().map_err(|_| invalid())?;
            if !window.is_visible().unwrap_or(false) || window.is_minimized().unwrap_or(true) {
                let _ = window.close();
                return Err(invalid());
            }
            Ok(())
        })
        .await
        .map_err(|_| invalid())
        .and_then(|result| result);
        if let Err(error) = result.and_then(|()| guard()) {
            if let Some(window) = app.get_window(&child.window_label) {
                let _ = window.close();
            }
            self.lock()?.child = None;
            return Err(error);
        }
        Ok(())
    }
    async fn dispose_child_owner(
        &self,
        app: &AppHandle,
        label: &str,
    ) -> Result<(), CollaborationError> {
        crate::commands::collaboration_harness::dispose_harness_child_owner(
            app,
            &self.runtime,
            label,
        )
        .await
    }
    async fn close_child<F>(&self, app: &AppHandle, guard: &F) -> Result<(), CollaborationError>
    where
        F: Fn() -> Result<(), CollaborationError>,
    {
        let child = self.lock()?.child.clone().ok_or_else(invalid)?;
        self.dispose_child_owner(app, &child.label).await?;
        guard()?;
        self.cancel_reads()?;
        if let Some(window) = app.get_window(&child.window_label) {
            window.close().map_err(|_| invalid())?;
        }
        {
            let mut projection = self.lock()?;
            projection.child = None;
            projection.hint_mode = HarnessHintMode::Normal;
            projection.hints.clear();
        }
        guard()?;
        let main = app.get_window("main").ok_or_else(invalid)?;
        main.show().map_err(|_| invalid())?;
        if main.is_minimized().map_err(|_| invalid())? {
            main.unminimize().map_err(|_| invalid())?;
        }
        main.set_focus().map_err(|_| invalid())?;
        if !main.is_visible().unwrap_or(false) || main.is_minimized().unwrap_or(true) {
            return Err(invalid());
        }
        guard()
    }
}
fn retain_hint(hints: &mut VecDeque<String>, revision: String) {
    if hints.len() == MAX_HINTS {
        hints.pop_front();
    }
    hints.push_back(revision);
}
fn emit_hint(app: &AppHandle, label: &str, hint: ChangeHint) {
    let _ = app.emit_to(
        EventTarget::webview(label),
        "gitru:collaboration-change",
        hint,
    );
}
fn validate_request(request: &HarnessControlRequest) -> Result<(), CollaborationError> {
    if request.action == HarnessAction::Core {
        let action = request.core_action.ok_or_else(invalid)?;
        if (action == HarnessCoreAction::ReleaseProviderGate) != request.gate_id.is_some() {
            return Err(invalid());
        }
    } else if request.core_action.is_some()
        || (matches!(
            request.action,
            HarnessAction::ReleaseLocalRead | HarnessAction::CheckpointBeforeCommit
        ) != request.gate_id.is_some())
    {
        return Err(invalid());
    }
    if request
        .gate_id
        .as_deref()
        .is_some_and(|id| uuid::Uuid::parse_str(id).is_err())
    {
        return Err(invalid());
    }
    Ok(())
}
#[cfg(test)]
mod tests;
