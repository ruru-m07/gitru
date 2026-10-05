//! Native-only registration and caller authority around authored local links.
use super::collaboration::{authorize, caller_allowed, CollaborationState, Operation};
use collaboration::{local_links::*, ChangeHint, CollaborationError, ErrorCode};
use git::models::remotes::{RemoteObservationError, RemoteSnapshot, RemoteTransport};
use ipc::{
    local_repository::observe_registered_repository, repo_manager::RepoManager,
    repository_watcher::RepoContextRuntime,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    sync::Mutex,
    time::{Duration, Instant},
};
use tauri::{AppHandle, Emitter, Manager, State, Webview};

const PREVIEW_TTL: Duration = Duration::from_secs(120);
const MAX_PREVIEWS: usize = 64;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalLinkInspection {
    pub local_repository_id: String,
    pub remotes: Option<RemoteSnapshot>,
    pub observation_error: Option<RemoteObservationError>,
    pub snapshot: LocalLinkSnapshot,
    pub preview_id: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfirmLocalLinkPreview {
    pub preview_id: String,
    pub candidate_id: String,
    pub replace_link_id: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransportBindingRequest {
    pub instance_id: String,
    pub transport: LinkTransport,
    pub host: String,
    pub port: u16,
    pub path_prefix: String,
    pub layout: RepositoryPathLayout,
    pub expected_bindings_generation: String,
    pub replace: Option<LocalLinkVersion>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalCloneRequest {
    pub account_id: String,
    pub instance_id: String,
    pub repository_id: String,
    pub authorization_epoch: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalCloneRecord {
    pub local_repository_id: String,
    pub local_repository_name: Option<String>,
    pub link_id: String,
    pub generation: String,
    pub state: LocalLinkState,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalCloneSnapshot {
    pub clones: Vec<LocalCloneRecord>,
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LocalNavigationDirection {
    Git,
    Collaboration,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalNavigationRequest {
    pub local_repository_id: String,
    pub link_id: String,
    pub generation: String,
    pub direction: LocalNavigationDirection,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalNavigationReceipt {
    pub local_repository_id: String,
    pub account_id: String,
    pub instance_id: String,
    pub repository_id: String,
    pub authorization_epoch: String,
    pub selected: bool,
}

// A Webview clone owns the same native resource-table allocation; a newly
// attached Webview owns a new one. Retaining the clone prevents identity reuse
// before its ready callback, which Tauri may dispatch asynchronously.
struct Incarnation<T> {
    generation: u64,
    identity: usize,
    allowed: bool,
    ready: bool,
    _owner: T,
}
struct IncarnationRegistry<T> {
    next: u64,
    views: HashMap<String, Incarnation<T>>,
}
impl<T> Default for IncarnationRegistry<T> {
    fn default() -> Self {
        Self {
            next: 0,
            views: HashMap::new(),
        }
    }
}
impl<T> IncarnationRegistry<T> {
    fn rotate(
        &mut self,
        label: &str,
        identity: usize,
        allowed: bool,
        ready: bool,
        owner: T,
    ) -> Result<(), CollaborationError> {
        let Some(next) = self.next.checked_add(1) else {
            self.views.clear();
            return Err(denied());
        };
        if self.views.len() >= 1024 && !self.views.contains_key(label) {
            return Err(denied());
        }
        self.next = next;
        self.views.insert(
            label.into(),
            Incarnation {
                generation: self.next,
                identity,
                allowed,
                ready,
                _owner: owner,
            },
        );
        Ok(())
    }
    fn created(
        &mut self,
        label: &str,
        identity: usize,
        allowed: bool,
        owner: T,
        active: &HashSet<String>,
    ) -> Result<(), CollaborationError> {
        self.views.retain(|label, _| active.contains(label));
        if self.views.len() >= 1024 && !self.views.contains_key(label) {
            return Err(denied());
        }
        let allowed = self
            .views
            .get(label)
            .filter(|entry| entry.identity == identity && !entry.ready)
            .map_or(allowed, |entry| entry.allowed && allowed);
        self.rotate(label, identity, allowed, true, owner)
    }
    fn navigating(
        &mut self,
        label: &str,
        identity: usize,
        allowed: bool,
        owner: T,
    ) -> Result<(), CollaborationError> {
        let ready = self
            .views
            .get(label)
            .is_some_and(|entry| entry.identity == identity && entry.ready);
        self.rotate(label, identity, allowed, ready, owner)
    }
    fn current(&self, label: &str, identity: usize) -> Result<u64, CollaborationError> {
        self.views
            .get(label)
            .filter(|entry| entry.identity == identity && entry.ready && entry.allowed)
            .map(|entry| entry.generation)
            .ok_or_else(denied)
    }
}
#[derive(Default)]
pub(super) struct NativeWebviewLifetimes(Mutex<IncarnationRegistry<Webview>>);
fn native_identity(view: &Webview) -> usize {
    let resources = view.resources_table();
    std::ptr::from_ref(&*resources) as usize
}
pub(crate) fn lifetime_plugin() -> tauri::plugin::TauriPlugin<tauri::Wry> {
    tauri::plugin::Builder::new("local-link-lifetimes")
        .on_webview_ready(|view: Webview| {
            let app = view.app_handle();
            let Some(state) = app.try_state::<CollaborationState>() else {
                return;
            };
            let identity = native_identity(&view);
            // An old delayed ready callback cannot replace the new owner.
            if app
                .get_webview(view.label())
                .is_none_or(|current| native_identity(&current) != identity)
            {
                return;
            }
            let active = app.webviews().into_keys().collect();
            let allowed = view
                .url()
                .is_ok_and(|url| caller_allowed(view.label(), &url, Operation::LocalLinks));
            if let Ok(mut registry) = state.webview_lifetimes.0.lock() {
                let _ = registry.created(view.label(), identity, allowed, view.clone(), &active);
            };
        })
        .on_navigation(|view, url| {
            if let Some(state) = view.app_handle().try_state::<CollaborationState>() {
                if let Ok(mut registry) = state.webview_lifetimes.0.lock() {
                    let _ = registry.navigating(
                        view.label(),
                        native_identity(view),
                        caller_allowed(view.label(), url, Operation::LocalLinks),
                        view.clone(),
                    );
                }
            }
            // Preserve app navigation policy. This hook only revokes old link authority.
            true
        })
        .build()
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct CallerProof {
    label: String,
    url: String,
    owners: Vec<(String, u64)>,
    native_identity: usize,
    incarnation: u64,
}
impl CallerProof {
    pub(super) fn capture(view: &Webview, app: &AppHandle) -> Result<Self, CollaborationError> {
        authorize(view, Operation::LocalLinks)?;
        let native_identity = native_identity(view);
        let incarnation = app
            .state::<CollaborationState>()
            .webview_lifetimes
            .0
            .lock()
            .map_err(|_| denied())?
            .current(view.label(), native_identity)?;
        let proof = Self {
            native_identity,
            incarnation,
            label: view.label().into(),
            url: view.url().map_err(|_| denied())?.to_string(),
            owners: app
                .state::<RepoContextRuntime>()
                .webview_owner_generations(view.label())
                .map_err(|_| denied())?,
        };
        proof.validate(view, app)?;
        Ok(proof)
    }
    pub(super) fn validate(
        &self,
        view: &Webview,
        app: &AppHandle,
    ) -> Result<(), CollaborationError> {
        authorize(view, Operation::LocalLinks)?;
        let current = app.get_webview(&self.label).ok_or_else(denied)?;
        if view.label() != self.label
            || current.url().map_err(|_| denied())?.as_str() != self.url
            || view.url().map_err(|_| denied())?.as_str() != self.url
        {
            return Err(denied());
        }
        self.validate_under_writer(app)
    }
    /// No native URL lookup/UI-thread dispatch while SQLite owns the writer.
    pub(super) fn validate_under_writer(&self, app: &AppHandle) -> Result<(), CollaborationError> {
        let current = app.get_webview(&self.label).ok_or_else(denied)?;
        let identity = native_identity(&current);
        let incarnation = app
            .state::<CollaborationState>()
            .webview_lifetimes
            .0
            .lock()
            .map_err(|_| denied())?
            .current(&self.label, identity)?;
        if identity != self.native_identity
            || incarnation != self.incarnation
            || app
                .state::<RepoContextRuntime>()
                .webview_owner_generations(&self.label)
                .map_err(|_| denied())?
                != self.owners
        {
            return Err(denied());
        }
        Ok(())
    }
}
struct LinkPreview {
    expires: Instant,
    caller: CallerProof,
    query: LocalLinkQuery,
    candidate_ids: HashSet<String>,
    link_versions: HashMap<String, String>,
    authorization_view: String,
    bindings_generation: String,
}
#[derive(Default)]
pub(super) struct LocalLinkPreviews(Mutex<HashMap<String, LinkPreview>>);
impl LocalLinkPreviews {
    fn insert(&self, preview: LinkPreview, now: Instant) -> Result<String, CollaborationError> {
        let mut previews = self.0.lock().map_err(|_| CollaborationError::storage())?;
        previews.retain(|_, p| {
            p.expires > now
                && !(p.caller == preview.caller
                    && p.query.local_repository_id == preview.query.local_repository_id)
        });
        if previews.len() >= MAX_PREVIEWS {
            return Err(CollaborationError::new(
                ErrorCode::NotReady,
                "Too many open link previews",
            ));
        }
        let id = uuid::Uuid::new_v4().to_string();
        previews.insert(id.clone(), preview);
        Ok(id)
    }
    fn take(
        &self,
        id: &str,
        caller: &CallerProof,
        now: Instant,
    ) -> Result<LinkPreview, CollaborationError> {
        if id.len() > 128 {
            return Err(stale());
        }
        let mut previews = self.0.lock().map_err(|_| CollaborationError::storage())?;
        previews.retain(|_, p| p.expires > now);
        let preview = previews.get(id).ok_or_else(stale)?;
        if preview.caller != *caller {
            return Err(denied());
        }
        previews.remove(id).ok_or_else(stale)
    }
}
fn denied() -> CollaborationError {
    CollaborationError::new(
        ErrorCode::PermissionDenied,
        "The requesting Gitru tab is no longer active",
    )
}
fn stale() -> CollaborationError {
    CollaborationError::new(
        ErrorCode::StaleView,
        "Inspect this local repository link again",
    )
}
fn missing_query(id: &str) -> LocalLinkQuery {
    LocalLinkQuery {
        local_repository_id: id.into(),
        registration_proof: None,
        remote_digest: None,
        endpoints: vec![],
    }
}
fn endpoints(remotes: &RemoteSnapshot) -> Vec<LocalRemoteEndpoint> {
    remotes
        .remotes
        .iter()
        .flat_map(|remote| {
            [
                (LinkDirection::Fetch, &remote.fetch_urls),
                (LinkDirection::Push, &remote.push_urls),
            ]
            .into_iter()
            .flat_map(move |(direction, urls)| {
                urls.iter().filter_map(move |url| {
                    url.endpoint.as_ref().map(|e| LocalRemoteEndpoint {
                        remote_name: remote.name.clone(),
                        direction,
                        ordinal: url.ordinal,
                        transport: match e.transport {
                            RemoteTransport::Https => LinkTransport::Https,
                            RemoteTransport::Ssh => LinkTransport::Ssh,
                            RemoteTransport::Scp => LinkTransport::Scp,
                        },
                        host: e.host.clone(),
                        port: e.port,
                        path: e.path.clone(),
                    })
                })
            })
        })
        .collect()
}
async fn observe(
    app: &AppHandle,
    id: &str,
) -> Result<
    (
        LocalLinkQuery,
        Option<RemoteSnapshot>,
        Option<RemoteObservationError>,
    ),
    CollaborationError,
> {
    let manager = RepoManager::new(app.clone());
    let Some(repo) = manager
        .repository_for_link(id)
        .map_err(|_| CollaborationError::invalid("Invalid repository registration"))?
    else {
        return Ok((
            missing_query(id),
            None,
            Some(RemoteObservationError::Unavailable),
        ));
    };
    let observation =
        tokio::time::timeout(Duration::from_secs(8), observe_registered_repository(&repo)).await;
    let observation = match observation {
        Ok(value) => value,
        Err(_) => Err(RemoteObservationError::Timeout),
    };
    match observation {
        Ok(value) => {
            // Registration removal/replacement while Git was running invalidates the result.
            let current = manager
                .repository_for_link(id)
                .map_err(|_| stale())?
                .ok_or_else(stale)?;
            if current.path != repo.path {
                return Err(stale());
            }
            let endpoints = endpoints(&value.remotes);
            if endpoints.len() > 256 {
                return Ok((
                    missing_query(id),
                    Some(value.remotes),
                    Some(RemoteObservationError::LimitExceeded),
                ));
            }
            let query = LocalLinkQuery {
                local_repository_id: id.into(),
                registration_proof: Some(value.proof),
                remote_digest: Some(value.remotes.semantic_digest.clone()),
                endpoints,
            };
            Ok((query, Some(value.remotes), None))
        }
        Err(error) => Ok((missing_query(id), None, Some(error))),
    }
}
fn emit(app: &AppHandle, revision: String) {
    let _ = app.emit("gitru:collaboration-change", ChangeHint { revision });
}

#[tauri::command]
pub async fn collaboration_local_links(
    local_repository_id: String,
    view: Webview,
    app: AppHandle,
    state: State<'_, CollaborationState>,
) -> Result<LocalLinkInspection, CollaborationError> {
    let caller = CallerProof::capture(&view, &app)?;
    let runtime = state.get().await?;
    let (query, remotes, observation_error) = observe(&app, &local_repository_id).await?;
    caller.validate(&view, &app)?;
    let snapshot = runtime.store().local_link_snapshot(query.clone()).await?;
    caller.validate(&view, &app)?;
    let preview_id = if query.registration_proof.is_some() {
        let now = Instant::now();
        Some(
            state.local_link_previews.insert(
                LinkPreview {
                    expires: now + PREVIEW_TTL,
                    caller,
                    query,
                    candidate_ids: snapshot
                        .resolutions
                        .iter()
                        .flat_map(|resolution| &resolution.candidates)
                        .map(|candidate| candidate.id.clone())
                        .collect(),
                    link_versions: snapshot
                        .links
                        .iter()
                        .map(|link| (link.id.clone(), link.generation.clone()))
                        .collect(),
                    authorization_view: snapshot.authorization_view.clone(),
                    bindings_generation: snapshot.bindings_generation.clone(),
                },
                now,
            )?,
        )
    } else {
        None
    };
    Ok(LocalLinkInspection {
        local_repository_id,
        remotes,
        observation_error,
        snapshot,
        preview_id,
    })
}
#[tauri::command]
pub async fn collaboration_confirm_local_link(
    request: ConfirmLocalLinkPreview,
    view: Webview,
    app: AppHandle,
    state: State<'_, CollaborationState>,
) -> Result<LocalLinkWriteReceipt, CollaborationError> {
    if request.candidate_id.len() > 128
        || request
            .replace_link_id
            .as_ref()
            .is_some_and(|id| id.len() > 128)
    {
        return Err(stale());
    }
    let caller = CallerProof::capture(&view, &app)?;
    let preview = state
        .local_link_previews
        .take(&request.preview_id, &caller, Instant::now())?;
    let runtime = state.get().await?;
    let (query, _, error) = observe(&app, &preview.query.local_repository_id).await?;
    caller.validate(&view, &app)?;
    if error.is_some()
        || query.registration_proof != preview.query.registration_proof
        || query.remote_digest != preview.query.remote_digest
        || query.endpoints != preview.query.endpoints
    {
        return Err(stale());
    }
    if !preview.candidate_ids.contains(&request.candidate_id) {
        return Err(stale());
    }
    let replace = match request.replace_link_id {
        Some(id) => Some(LocalLinkVersion {
            generation: preview.link_versions.get(&id).cloned().ok_or_else(stale)?,
            id,
        }),
        None => None,
    };
    let receipt = runtime
        .store()
        .confirm_local_link_checked(
            ConfirmLocalLink {
                query,
                candidate_id: request.candidate_id,
                expected_authorization_view: preview.authorization_view,
                expected_bindings_generation: preview.bindings_generation,
                replace,
            },
            || caller.validate_under_writer(&app),
        )
        .await?;
    emit(&app, receipt.revision.clone());
    Ok(receipt)
}
#[tauri::command]
pub async fn collaboration_remove_local_link(
    id: String,
    generation: String,
    view: Webview,
    app: AppHandle,
    state: State<'_, CollaborationState>,
) -> Result<String, CollaborationError> {
    let caller = CallerProof::capture(&view, &app)?;
    let revision = state
        .get()
        .await?
        .store()
        .remove_local_link_checked(&id, &generation, || caller.validate_under_writer(&app))
        .await?;
    emit(&app, revision.clone());
    Ok(revision)
}
#[tauri::command]
pub async fn collaboration_save_transport_binding(
    request: TransportBindingRequest,
    view: Webview,
    app: AppHandle,
    state: State<'_, CollaborationState>,
) -> Result<LocalTransportBinding, CollaborationError> {
    authorize(&view, Operation::TransportBindings)?;
    let caller = CallerProof::capture(&view, &app)?;
    let runtime = state.get().await?;
    let binding = runtime
        .store()
        .save_transport_binding_checked(
            SaveTransportBinding {
                instance_id: request.instance_id,
                transport: request.transport,
                host: request.host,
                port: request.port,
                path_prefix: request.path_prefix,
                layout: request.layout,
                expected_bindings_generation: request.expected_bindings_generation,
                replace: request.replace,
            },
            || caller.validate_under_writer(&app),
        )
        .await?;
    emit(&app, runtime.store().revision().await?);
    Ok(binding)
}
#[tauri::command]
pub async fn collaboration_remove_transport_binding(
    id: String,
    generation: String,
    expected_bindings_generation: String,
    view: Webview,
    app: AppHandle,
    state: State<'_, CollaborationState>,
) -> Result<String, CollaborationError> {
    authorize(&view, Operation::TransportBindings)?;
    let caller = CallerProof::capture(&view, &app)?;
    let revision = state
        .get()
        .await?
        .store()
        .remove_transport_binding_checked(&id, &generation, &expected_bindings_generation, || {
            caller.validate_under_writer(&app)
        })
        .await?;
    emit(&app, revision.clone());
    Ok(revision)
}
#[tauri::command]
pub async fn collaboration_local_clones(
    request: LocalCloneRequest,
    view: Webview,
    app: AppHandle,
    state: State<'_, CollaborationState>,
) -> Result<LocalCloneSnapshot, CollaborationError> {
    let caller = CallerProof::capture(&view, &app)?;
    let runtime = state.get().await?;
    let links = runtime
        .store()
        .local_links_for_resource(
            &request.account_id,
            &request.instance_id,
            &request.repository_id,
            &request.authorization_epoch,
        )
        .await?;
    // Whole request is bounded. Four probes at a time; unread probes stay unavailable.
    let mut clones: Vec<_> = links
        .iter()
        .map(|l| LocalCloneRecord {
            local_repository_id: l.local_repository_id.clone(),
            local_repository_name: None,
            link_id: l.id.clone(),
            generation: l.generation.clone(),
            state: LocalLinkState::LocalRepositoryMissing,
        })
        .collect();
    let permits = std::sync::Arc::new(tokio::sync::Semaphore::new(4));
    let mut probes = tokio::task::JoinSet::new();
    for (index, link) in links.into_iter().enumerate() {
        let app = app.clone();
        let permits = permits.clone();
        probes.spawn(async move {
            let _permit = permits.acquire_owned().await.ok()?;
            let (query, _, error) = observe(&app, &link.local_repository_id).await.ok()?;
            let name = RepoManager::new(app)
                .repository_for_link(&link.local_repository_id)
                .ok()
                .flatten()
                .map(|r| r.name);
            Some((index, query, error, name))
        });
    }
    let deadline = tokio::time::Instant::now() + Duration::from_secs(12);
    loop {
        let joined = tokio::time::timeout_at(deadline, probes.join_next()).await;
        let Ok(Some(Ok(Some((index, query, error, name))))) = joined else {
            if matches!(joined, Ok(Some(_))) {
                continue;
            }
            break;
        };
        if error.is_some() {
            if name.is_some() {
                clones[index].local_repository_name = name;
                clones[index].state = LocalLinkState::Unavailable;
            }
            continue;
        }
        let snapshot = runtime.store().local_link_snapshot(query).await?;
        let record = &mut clones[index];
        record.local_repository_name = name;
        record.state = snapshot
            .links
            .iter()
            .find(|l| l.id == record.link_id && l.generation == record.generation)
            .map_or(LocalLinkState::RemoteChanged, |l| l.state);
    }
    probes.abort_all();
    caller.validate(&view, &app)?;
    // Access may have changed during filesystem work: never expose the earlier list.
    let authorized = runtime
        .store()
        .local_links_for_resource(
            &request.account_id,
            &request.instance_id,
            &request.repository_id,
            &request.authorization_epoch,
        )
        .await?;
    clones.retain(|c| {
        authorized
            .iter()
            .any(|l| l.id == c.link_id && l.generation == c.generation)
    });
    Ok(LocalCloneSnapshot { clones })
}
#[tauri::command]
pub async fn collaboration_validate_local_navigation(
    request: LocalNavigationRequest,
    view: Webview,
    app: AppHandle,
    state: State<'_, CollaborationState>,
) -> Result<LocalNavigationReceipt, CollaborationError> {
    let caller = CallerProof::capture(&view, &app)?;
    let runtime = state.get().await?;
    let (query, _, error) = observe(&app, &request.local_repository_id).await?;
    caller.validate(&view, &app)?;
    if error.is_some() {
        return Err(stale());
    }
    let snapshot = runtime.store().local_link_snapshot(query).await?;
    let link = snapshot
        .links
        .iter()
        .find(|l| {
            l.id == request.link_id
                && l.generation == request.generation
                && l.state == LocalLinkState::Linked
        })
        .ok_or_else(stale)?;
    let repository = link.repository.as_ref().ok_or_else(denied)?;
    let account = runtime.store().account(&link.account_id).await?;
    // Bind navigation to the same account authorization view as the selected link.
    if runtime.store().accounts().await?.authorization_view != snapshot.authorization_view {
        return Err(stale());
    }
    caller.validate(&view, &app)?;
    Ok(LocalNavigationReceipt {
        local_repository_id: link.local_repository_id.clone(),
        account_id: link.account_id.clone(),
        instance_id: link.instance_id.clone(),
        repository_id: link.repository_id.clone(),
        authorization_epoch: account.authorization_epoch,
        selected: repository.selected,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn caller(label: &str) -> CallerProof {
        CallerProof {
            native_identity: 1,
            incarnation: 1,
            label: label.into(),
            url: "tauri://localhost/app/git".into(),
            owners: vec![("scope".into(), 1)],
        }
    }
    fn preview(now: Instant) -> LinkPreview {
        LinkPreview {
            expires: now + PREVIEW_TTL,
            caller: caller("tab-webview:fixture"),
            query: missing_query("registered"),
            candidate_ids: HashSet::new(),
            link_versions: HashMap::new(),
            authorization_view: "1".into(),
            bindings_generation: "1".into(),
        }
    }
    #[test]
    fn preview_is_bounded_monotonic_single_use_and_caller_bound() {
        let registry = LocalLinkPreviews::default();
        let now = Instant::now();
        let id = registry.insert(preview(now), now).unwrap();
        assert_eq!(
            registry
                .take(&id, &caller("tab-webview:other"), now)
                .err()
                .unwrap()
                .code,
            ErrorCode::PermissionDenied
        );
        let mut retired = caller("tab-webview:fixture");
        retired.owners[0].1 = 2;
        assert!(registry.take(&id, &retired, now).is_err());
        assert!(registry
            .take(&id, &caller("tab-webview:fixture"), now)
            .is_ok());
        assert!(registry
            .take(&id, &caller("tab-webview:fixture"), now)
            .is_err());
        for index in 0..MAX_PREVIEWS {
            let mut value = preview(now);
            value.query.local_repository_id = format!("registered-{index}");
            registry.insert(value, now).unwrap();
        }
        assert!(registry.insert(preview(now), now).is_err());
        assert!(registry
            .insert(preview(now + PREVIEW_TTL), now + PREVIEW_TTL)
            .is_ok());
    }
    #[test]
    fn expired_preview_is_rejected_at_exact_deadline() {
        let registry = LocalLinkPreviews::default();
        let now = Instant::now();
        let id = registry.insert(preview(now), now).unwrap();
        assert_eq!(
            registry
                .take(&id, &caller("tab-webview:fixture"), now + PREVIEW_TTL)
                .err()
                .unwrap()
                .code,
            ErrorCode::StaleView
        );
    }
    #[test]
    fn reinspection_retires_the_prior_token_without_consuming_registry_capacity() {
        let registry = LocalLinkPreviews::default();
        let now = Instant::now();
        let old = registry.insert(preview(now), now).unwrap();
        let mut latest = String::new();
        for _ in 0..(MAX_PREVIEWS * 2) {
            latest = registry.insert(preview(now), now).unwrap();
        }
        assert_eq!(registry.0.lock().unwrap().len(), 1);
        assert!(registry
            .take(&old, &caller("tab-webview:fixture"), now)
            .is_err());
        assert!(registry
            .take(&latest, &caller("tab-webview:fixture"), now)
            .is_ok());
    }
    #[test]
    fn native_incarnation_rejects_same_label_replacement_before_and_after_ready_without_git_contexts(
    ) {
        use std::sync::Arc;
        let old = Arc::new(Mutex::new(tauri::ResourceTable::default()));
        let new = Arc::new(Mutex::new(tauri::ResourceTable::default()));
        let identity = |table: &Arc<Mutex<tauri::ResourceTable>>| {
            std::ptr::from_ref(&*table.lock().unwrap()) as usize
        };
        let label = "tab-webview:collaboration-only";
        let active = HashSet::from([label.into()]);
        let mut registry = IncarnationRegistry::default();
        registry
            .created(label, identity(&old), true, old.clone(), &active)
            .unwrap();
        let captured = registry.current(label, identity(&old)).unwrap();
        // Tauri can register a replacement before dispatching its ready callback.
        assert!(registry.current(label, identity(&new)).is_err());
        registry
            .created(label, identity(&new), true, new.clone(), &active)
            .unwrap();
        assert!(registry.current(label, identity(&old)).is_err());
        assert_ne!(registry.current(label, identity(&new)).unwrap(), captured);
        registry
            .created(
                "main",
                identity(&old),
                true,
                old,
                &HashSet::from(["main".into()]),
            )
            .unwrap();
        assert!(!registry.views.contains_key(label));
    }
    #[test]
    fn native_navigation_revokes_waiting_authority_before_foreign_document_load() {
        let label = "tab-webview:local";
        let mut registry = IncarnationRegistry::default();
        registry
            .created(label, 1, true, (), &HashSet::from([label.into()]))
            .unwrap();
        let captured = registry.current(label, 1).unwrap();
        registry.navigating(label, 1, false, ()).unwrap();
        assert!(registry.current(label, 1).is_err());
        registry.navigating(label, 1, true, ()).unwrap();
        assert_ne!(registry.current(label, 1).unwrap(), captured);
        // A pre-ready foreign navigation must not be revived by the delayed hook.
        registry.navigating(label, 2, false, ()).unwrap();
        registry
            .created(label, 2, true, (), &HashSet::from([label.into()]))
            .unwrap();
        assert!(registry.current(label, 2).is_err());
    }
}
