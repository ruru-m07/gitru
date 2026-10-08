//! Local reads are separate commands from network refresh intents.
use collaboration::{
    AccountSnapshot, CapabilitySnapshot, ChangePage, CollaborationError, CollaborationRuntime,
    ContextCapabilityRequest, ContextualCapabilitySnapshot, DetailQuery, DetailSnapshot, DraftPage,
    DraftQuery, ErrorCode, GithubCliDiscovery, HydrateDetailRequest, InboxPage, InboxQuery,
    ItemPage, ItemQuery, ItemSnapshot, LocalDraft, LocalInboxWriteReceipt, PullCommitQuery,
    PullCommitSnapshot, RefreshReceipt, RefreshRequest, RemoteAccount, RepositorySnapshot,
    ResourceLocator, ResourceResolution, SetLocalInboxStateRequest, SyncDiagnosticsExportReceipt,
    SyncDiagnosticsSnapshot,
};
use std::sync::Arc;
use tauri::{State, Webview};
use tauri_plugin_dialog::DialogExt;

mod command_recovery;
mod provider_inbox_actions;
pub use provider_inbox_actions::*;
mod diagnostics_export;
mod draft_export;
pub use command_recovery::*;
mod lifecycle;
pub(super) use lifecycle::RecoveryTransition;
use lifecycle::RuntimeSlot;

#[derive(Default)]
pub struct CollaborationState {
    native_database_path: std::sync::OnceLock<std::path::PathBuf>,
    pub(crate) runtime: RuntimeSlot,
    services: std::sync::Mutex<Vec<tokio::task::JoinHandle<()>>>,
    transition: tokio::sync::Mutex<()>,
    draft_export: tokio::sync::Mutex<()>,
    diagnostic_export: tokio::sync::Mutex<()>,
    pub(super) demand_hosts: tokio::sync::Mutex<std::collections::HashMap<String, bool>>,
    pub(super) local_link_previews: super::collaboration_local_links::LocalLinkPreviews,
    pub(super) pull_checkout_plans: super::collaboration_pull_checkout::PullCheckoutPlans,
    pub(super) webview_lifetimes: super::collaboration_local_links::NativeWebviewLifetimes,
}

#[derive(Clone, Copy)]
pub(super) enum Operation {
    Accounts,
    ConnectGithub,
    ConnectGitlab,
    ConnectBitbucketCloud,
    DiscoverGithubCli,
    ConnectGithubCli,
    Disconnect,
    Repositories,
    SelectRepository,
    Items,
    Item,
    Inbox,
    SetLocalInboxState,
    Refresh,
    ChangesSince,
    SaveDraft,
    Draft,
    Drafts,
    ExportDraft,
    Diagnostics,
    ExportDiagnostics,
    CommandRecovery,
    ProviderInboxAction,
    Recovery,
    Capabilities,
    ContextualCapabilities,
    ResolveResource,
    Detail,
    PullCommits,
    PullFiles,
    PullFileArtifact,
    HydratePullFile,
    LocalPullFile,
    HydrateDetail,
    DemandActivity,
    AcquireDemand,
    RenewDemand,
    ReleaseDemand,
    InspectDemandOwner,
    SetDemandOwner,
    DisposeDemandOwner,
    LocalLinks,
    TransportBindings,
    NotificationSubject,
    DiscoverNotificationSubject,
    PullCheckout,
    PullCommitNavigation,
}

impl Operation {
    fn requires_main(self) -> bool {
        matches!(
            self,
            Self::ConnectGithub
                | Self::ConnectGitlab
                | Self::ConnectBitbucketCloud
                | Self::DiscoverGithubCli
                | Self::ConnectGithubCli
                | Self::Disconnect
                | Self::Diagnostics
                | Self::ExportDiagnostics
                | Self::InspectDemandOwner
                | Self::SetDemandOwner
                | Self::DisposeDemandOwner
                | Self::TransportBindings
        )
    }
}

pub(super) fn authorize(view: &Webview, operation: Operation) -> Result<(), CollaborationError> {
    let url = view.url().map_err(|_| denied())?;
    if !caller_allowed(view.label(), &url, operation) {
        return Err(denied());
    }
    Ok(())
}

pub(super) fn caller_allowed(label: &str, url: &url::Url, operation: Operation) -> bool {
    let production = url.port().is_none()
        && matches!(
            (url.scheme(), url.host_str()),
            ("tauri", Some("localhost"))
                | ("https", Some("tauri.localhost"))
                | ("http", Some("tauri.localhost"))
        );
    let development = cfg!(debug_assertions)
        && url.scheme() == "http"
        && matches!(url.host_str(), Some("localhost" | "127.0.0.1"))
        && url.port() == Some(1420);
    url.username().is_empty()
        && url.password().is_none()
        && (production || development)
        && (label == "main" || (!operation.requires_main() && label.starts_with("tab-webview:")))
}

fn denied() -> CollaborationError {
    CollaborationError::new(
        ErrorCode::PermissionDenied,
        "Manage provider accounts in the main Gitru window",
    )
}

#[tauri::command]
pub async fn collaboration_accounts(
    view: Webview,
    state: State<'_, CollaborationState>,
) -> Result<AccountSnapshot, CollaborationError> {
    authorize(&view, Operation::Accounts)?;
    #[cfg(feature = "collaboration-harness")]
    let started = std::time::Instant::now();
    let snapshot = state.get().await?.store().accounts().await?;
    #[cfg(feature = "collaboration-harness")]
    crate::collaboration_harness::record_performance_query(
        view.label(),
        crate::collaboration_harness::HarnessQueryKind::Accounts,
        started.elapsed(),
        snapshot.accounts.len(),
    );
    Ok(snapshot)
}

#[tauri::command]
pub async fn collaboration_connect_github(
    token: String,
    view: Webview,
    state: State<'_, CollaborationState>,
) -> Result<RemoteAccount, CollaborationError> {
    authorize(&view, Operation::ConnectGithub)?;
    state.get().await?.connect_github(token).await
}

#[tauri::command]
pub async fn collaboration_connect_gitlab(
    token: String,
    view: Webview,
    state: State<'_, CollaborationState>,
) -> Result<RemoteAccount, CollaborationError> {
    authorize(&view, Operation::ConnectGitlab)?;
    state.get().await?.connect_gitlab(token).await
}

#[tauri::command]
pub async fn collaboration_connect_bitbucket_cloud(
    token: String,
    view: Webview,
    state: State<'_, CollaborationState>,
) -> Result<RemoteAccount, CollaborationError> {
    authorize(&view, Operation::ConnectBitbucketCloud)?;
    state.get().await?.connect_bitbucket_cloud(token).await
}

#[tauri::command]
pub async fn collaboration_discover_github_cli(
    view: Webview,
    state: State<'_, CollaborationState>,
) -> Result<GithubCliDiscovery, CollaborationError> {
    authorize(&view, Operation::DiscoverGithubCli)?;
    Ok(state.get().await?.discover_github_cli().await)
}

#[tauri::command]
pub async fn collaboration_connect_github_cli(
    candidate_id: String,
    view: Webview,
    state: State<'_, CollaborationState>,
) -> Result<RemoteAccount, CollaborationError> {
    authorize(&view, Operation::ConnectGithubCli)?;
    state.get().await?.connect_github_cli(&candidate_id).await
}

#[tauri::command]
pub async fn collaboration_disconnect(
    account_id: String,
    view: Webview,
    state: State<'_, CollaborationState>,
) -> Result<String, CollaborationError> {
    authorize(&view, Operation::Disconnect)?;
    state.get().await?.disconnect(&account_id).await
}

#[tauri::command]
pub async fn collaboration_repositories(
    account_id: String,
    view: Webview,
    state: State<'_, CollaborationState>,
) -> Result<RepositorySnapshot, CollaborationError> {
    authorize(&view, Operation::Repositories)?;
    #[cfg(feature = "collaboration-harness")]
    let started = std::time::Instant::now();
    let snapshot = state.get().await?.store().repositories(&account_id).await?;
    #[cfg(feature = "collaboration-harness")]
    crate::collaboration_harness::record_performance_query(
        view.label(),
        crate::collaboration_harness::HarnessQueryKind::Repositories,
        started.elapsed(),
        snapshot.repositories.len(),
    );
    Ok(snapshot)
}

#[tauri::command]
pub async fn collaboration_select_repository(
    account_id: String,
    repository_id: String,
    selected: bool,
    view: Webview,
    state: State<'_, CollaborationState>,
) -> Result<String, CollaborationError> {
    authorize(&view, Operation::SelectRepository)?;
    state
        .get()
        .await?
        .select_repository(&account_id, &repository_id, selected)
        .await
}

#[tauri::command]
pub async fn collaboration_items(
    query: ItemQuery,
    view: Webview,
    state: State<'_, CollaborationState>,
) -> Result<ItemPage, CollaborationError> {
    authorize(&view, Operation::Items)?;
    #[cfg(feature = "collaboration-harness")]
    let started = std::time::Instant::now();
    let snapshot = state.get().await?.store().query_items(query).await?;
    #[cfg(feature = "collaboration-harness")]
    crate::collaboration_harness::record_performance_query(
        view.label(),
        crate::collaboration_harness::HarnessQueryKind::Items,
        started.elapsed(),
        snapshot.items.len(),
    );
    Ok(snapshot)
}

#[tauri::command]
pub async fn collaboration_item(
    account_id: String,
    item_id: String,
    view: Webview,
    state: State<'_, CollaborationState>,
) -> Result<ItemSnapshot, CollaborationError> {
    authorize(&view, Operation::Item)?;
    #[cfg(feature = "collaboration-harness")]
    let proof = super::collaboration_harness::LocalReturnProof::capture(&view)?;
    #[cfg(feature = "collaboration-harness")]
    let started = std::time::Instant::now();
    let snapshot = state
        .get()
        .await?
        .store()
        .item(&account_id, &item_id)
        .await?;
    #[cfg(feature = "collaboration-harness")]
    crate::collaboration_harness::record_performance_query(
        view.label(),
        crate::collaboration_harness::HarnessQueryKind::Item,
        started.elapsed(),
        usize::from(snapshot.item.is_some()),
    );
    #[cfg(feature = "collaboration-harness")]
    proof
        .hold(
            crate::collaboration_harness::HarnessReadKind::Item,
            &account_id,
            &item_id,
        )
        .await?;
    Ok(snapshot)
}

#[tauri::command]
pub async fn collaboration_inbox(
    query: InboxQuery,
    view: Webview,
    state: State<'_, CollaborationState>,
) -> Result<InboxPage, CollaborationError> {
    authorize(&view, Operation::Inbox)?;
    state.get().await?.store().inbox(query).await
}

#[tauri::command]
pub async fn collaboration_set_local_inbox_state(
    request: SetLocalInboxStateRequest,
    view: Webview,
    state: State<'_, CollaborationState>,
) -> Result<LocalInboxWriteReceipt, CollaborationError> {
    authorize(&view, Operation::SetLocalInboxState)?;
    state.get().await?.set_local_inbox_state(request).await
}

#[tauri::command]
pub async fn collaboration_refresh(
    request: RefreshRequest,
    view: Webview,
    state: State<'_, CollaborationState>,
) -> Result<RefreshReceipt, CollaborationError> {
    authorize(&view, Operation::Refresh)?;
    state.get().await?.refresh(request).await
}

/// Cache/scheduler observation only. It does not load credentials, admit work
/// or contact a provider.
#[tauri::command]
pub async fn collaboration_diagnostics(
    view: Webview,
    state: State<'_, CollaborationState>,
) -> Result<SyncDiagnosticsSnapshot, CollaborationError> {
    authorize(&view, Operation::Diagnostics)?;
    state.get().await?.diagnostics().await
}

/// The destination and report bytes are both native-owned. IPC callers cannot
/// supply a path or contextual data to this aggregate-only export.
#[tauri::command]
pub async fn collaboration_export_diagnostics(
    view: Webview,
    state: State<'_, CollaborationState>,
) -> Result<SyncDiagnosticsExportReceipt, CollaborationError> {
    authorize(&view, Operation::ExportDiagnostics)?;
    let _lease = state.diagnostic_export.try_lock().map_err(|_| {
        CollaborationError::new(
            ErrorCode::NotReady,
            "A sync diagnostics export dialog is already open",
        )
    })?;
    let report = state.get().await?.diagnostics().await?.export();
    let (send, receive) = tokio::sync::oneshot::channel();
    view.dialog()
        .file()
        .set_parent(&view.window())
        .set_title("Export sync diagnostics")
        .set_file_name("gitru-sync-diagnostics.json")
        .add_filter("JSON", &["json"])
        .save_file(move |path| {
            let _ = send.send(path);
        });
    diagnostics_export::export(report, async move {
        receive
            .await
            .map_err(|_| CollaborationError::storage())?
            .map(|path| path.into_path().map_err(|_| CollaborationError::storage()))
            .transpose()
    })
    .await
}

#[tauri::command]
pub async fn collaboration_changes_since(
    after_revision: String,
    view: Webview,
    state: State<'_, CollaborationState>,
) -> Result<ChangePage, CollaborationError> {
    authorize(&view, Operation::ChangesSince)?;
    state
        .get()
        .await?
        .store()
        .changes_since(&after_revision)
        .await
}

#[tauri::command]
pub async fn collaboration_save_draft(
    draft: LocalDraft,
    view: Webview,
    state: State<'_, CollaborationState>,
) -> Result<LocalDraft, CollaborationError> {
    authorize(&view, Operation::SaveDraft)?;
    state.get().await?.save_draft(draft).await
}

#[tauri::command]
pub async fn collaboration_draft(
    account_id: String,
    subject_id: String,
    view: Webview,
    state: State<'_, CollaborationState>,
) -> Result<Option<LocalDraft>, CollaborationError> {
    authorize(&view, Operation::Draft)?;
    #[cfg(feature = "collaboration-harness")]
    let proof = super::collaboration_harness::LocalReturnProof::capture(&view)?;
    #[cfg(feature = "collaboration-harness")]
    let started = std::time::Instant::now();
    let snapshot = state
        .get()
        .await?
        .store()
        .draft(&account_id, &subject_id)
        .await?;
    #[cfg(feature = "collaboration-harness")]
    crate::collaboration_harness::record_performance_query(
        view.label(),
        crate::collaboration_harness::HarnessQueryKind::Draft,
        started.elapsed(),
        usize::from(snapshot.is_some()),
    );
    #[cfg(feature = "collaboration-harness")]
    proof
        .hold(
            crate::collaboration_harness::HarnessReadKind::Draft,
            &account_id,
            &subject_id,
        )
        .await?;
    Ok(snapshot)
}

#[tauri::command]
pub async fn collaboration_contextual_capabilities(
    request: ContextCapabilityRequest,
    view: Webview,
    state: State<'_, CollaborationState>,
) -> Result<ContextualCapabilitySnapshot, CollaborationError> {
    authorize(&view, Operation::ContextualCapabilities)?;
    #[cfg(feature = "collaboration-harness")]
    let started = std::time::Instant::now();
    let snapshot = state.get().await?.contextual_capabilities(request).await?;
    #[cfg(feature = "collaboration-harness")]
    crate::collaboration_harness::record_performance_query(
        view.label(),
        crate::collaboration_harness::HarnessQueryKind::ContextualCapabilities,
        started.elapsed(),
        snapshot.facets.len(),
    );
    Ok(snapshot)
}

#[tauri::command]
pub async fn collaboration_capabilities(
    account_id: String,
    view: Webview,
    state: State<'_, CollaborationState>,
) -> Result<CapabilitySnapshot, CollaborationError> {
    authorize(&view, Operation::Capabilities)?;
    state.get().await?.capabilities(&account_id).await
}

#[tauri::command]
pub async fn collaboration_resolve_resource(
    account_id: String,
    locator: ResourceLocator,
    view: Webview,
    state: State<'_, CollaborationState>,
) -> Result<ResourceResolution, CollaborationError> {
    authorize(&view, Operation::ResolveResource)?;
    state
        .get()
        .await?
        .store()
        .resolve_resource(&account_id, locator)
        .await
}

#[tauri::command]
pub async fn collaboration_drafts(
    query: DraftQuery,
    view: Webview,
    state: State<'_, CollaborationState>,
) -> Result<DraftPage, CollaborationError> {
    authorize(&view, Operation::Drafts)?;
    state.get().await?.store().query_drafts(query).await
}

/// The native dialog is the only source of the destination path. IPC callers
/// cannot supply arbitrary paths or text to this narrowly scoped export.
#[tauri::command]
pub async fn collaboration_export_draft(
    account_id: String,
    subject_id: String,
    generation: String,
    view: Webview,
    state: State<'_, CollaborationState>,
) -> Result<bool, CollaborationError> {
    authorize(&view, Operation::ExportDraft)?;
    let _lease = state.draft_export.try_lock().map_err(|_| {
        CollaborationError::new(ErrorCode::NotReady, "A draft export dialog is already open")
    })?;
    let draft = draft_export::saved(
        state.get().await?.store(),
        &account_id,
        &subject_id,
        &generation,
    )
    .await?;
    let (send, receive) = tokio::sync::oneshot::channel();
    view.dialog()
        .file()
        .set_parent(&view.window())
        .set_title("Export private draft")
        .set_file_name("gitru-draft.txt")
        .add_filter("Text", &["txt"])
        .save_file(move |path| {
            let _ = send.send(path);
        });
    draft_export::export(draft, async move {
        receive
            .await
            .map_err(|_| CollaborationError::storage())?
            .map(|path| path.into_path().map_err(|_| CollaborationError::storage()))
            .transpose()
    })
    .await
}

#[tauri::command]
pub async fn collaboration_detail(
    query: DetailQuery,
    view: Webview,
    state: State<'_, CollaborationState>,
) -> Result<DetailSnapshot, CollaborationError> {
    authorize(&view, Operation::Detail)?;
    #[cfg(feature = "collaboration-harness")]
    let proof = super::collaboration_harness::LocalReturnProof::capture(&view)?;
    #[cfg(feature = "collaboration-harness")]
    let started = std::time::Instant::now();
    #[cfg(feature = "collaboration-harness")]
    let held_target = (query.facet == collaboration::DetailFacet::Body)
        .then(|| (query.account_id.clone(), query.subject_id.clone()));
    let snapshot = state.get().await?.store().detail(query).await?;
    #[cfg(feature = "collaboration-harness")]
    crate::collaboration_harness::record_performance_query(
        view.label(),
        crate::collaboration_harness::HarnessQueryKind::Detail,
        started.elapsed(),
        snapshot.entries.len(),
    );
    #[cfg(feature = "collaboration-harness")]
    if let Some((account, subject)) = held_target {
        proof
            .hold(
                crate::collaboration_harness::HarnessReadKind::Body,
                &account,
                &subject,
            )
            .await?;
    }
    Ok(snapshot)
}

/// Cache-only ordered pull-request commit read. Provider hydration remains an
/// explicit detail intent and never occurs on this query path.
#[tauri::command]
pub async fn collaboration_pull_commits(
    query: PullCommitQuery,
    view: Webview,
    state: State<'_, CollaborationState>,
) -> Result<PullCommitSnapshot, CollaborationError> {
    authorize(&view, Operation::PullCommits)?;
    state.get().await?.store().pull_commits(query).await
}

#[tauri::command]
pub async fn collaboration_hydrate_detail(
    request: HydrateDetailRequest,
    view: Webview,
    state: State<'_, CollaborationState>,
) -> Result<RefreshReceipt, CollaborationError> {
    authorize(&view, Operation::HydrateDetail)?;
    #[cfg(feature = "collaboration-harness")]
    super::collaboration_harness::LocalReturnProof::capture(&view)?
        .record_hydrate()
        .await?;
    state.get().await?.hydrate_detail(request).await
}

#[cfg(test)]
mod tests {
    use super::{caller_allowed, Operation};

    const DOMAIN_OPERATIONS: &[Operation] = &[
        Operation::Accounts,
        Operation::Repositories,
        Operation::SelectRepository,
        Operation::Items,
        Operation::Item,
        Operation::Inbox,
        Operation::SetLocalInboxState,
        Operation::Refresh,
        Operation::ChangesSince,
        Operation::SaveDraft,
        Operation::Draft,
        Operation::Drafts,
        Operation::ExportDraft,
        Operation::ProviderInboxAction,
        Operation::CommandRecovery,
        Operation::Recovery,
        Operation::Capabilities,
        Operation::ContextualCapabilities,
        Operation::ResolveResource,
        Operation::Detail,
        Operation::PullCommits,
        Operation::PullFiles,
        Operation::PullFileArtifact,
        Operation::HydratePullFile,
        Operation::LocalPullFile,
        Operation::HydrateDetail,
        Operation::DemandActivity,
        Operation::AcquireDemand,
        Operation::RenewDemand,
        Operation::ReleaseDemand,
        Operation::LocalLinks,
        Operation::NotificationSubject,
        Operation::DiscoverNotificationSubject,
        Operation::PullCheckout,
        Operation::PullCommitNavigation,
    ];
    const CREDENTIAL_OPERATIONS: &[Operation] = &[
        Operation::ConnectGithub,
        Operation::ConnectGitlab,
        Operation::ConnectBitbucketCloud,
        Operation::DiscoverGithubCli,
        Operation::ConnectGithubCli,
        Operation::Disconnect,
        Operation::Diagnostics,
        Operation::ExportDiagnostics,
        Operation::TransportBindings,
    ];
    const HOST_OPERATIONS: &[Operation] = &[
        Operation::InspectDemandOwner,
        Operation::SetDemandOwner,
        Operation::DisposeDemandOwner,
    ];

    #[test]
    fn local_children_can_select_repositories_and_read_domain_data() {
        let app = url::Url::parse("tauri://localhost/app/pulls").unwrap();
        for &operation in DOMAIN_OPERATIONS {
            assert!(caller_allowed("main", &app, operation));
            assert!(caller_allowed("tab-webview:1", &app, operation));
            assert!(!caller_allowed("other", &app, operation));
        }
    }

    #[test]
    fn credential_commands_remain_restricted_to_the_main_local_webview() {
        let app = url::Url::parse("tauri://localhost/app/pulls").unwrap();
        for &operation in CREDENTIAL_OPERATIONS.iter().chain(HOST_OPERATIONS) {
            assert!(caller_allowed("main", &app, operation));
            assert!(!caller_allowed("tab-webview:1", &app, operation));
            assert!(!caller_allowed("other", &app, operation));
        }
    }

    #[test]
    fn remote_origins_cannot_read_select_repositories_or_manage_credentials() {
        for url in [
            "https://github.com",
            "https://tauri.localhost.evil.com",
            "file:///tmp/page.html",
            "http://localhost:3000",
            "tauri://localhost:1420/app/pulls",
            "https://tauri.localhost:4445/app/pulls",
            "http://tauri.localhost:1420/app/pulls",
            "tauri://actor@localhost/app/pulls",
            "https://actor:secret@tauri.localhost/app/pulls",
            "http://actor@localhost:1420/app/pulls",
        ] {
            let url = url::Url::parse(url).unwrap();
            for &operation in DOMAIN_OPERATIONS
                .iter()
                .chain(CREDENTIAL_OPERATIONS)
                .chain(HOST_OPERATIONS)
            {
                assert!(!caller_allowed("main", &url, operation));
                assert!(!caller_allowed("tab-webview:1", &url, operation));
            }
        }
    }

    #[test]
    fn configured_native_origins_and_the_explicit_debug_origin_remain_usable() {
        for address in [
            "tauri://localhost/app/pulls",
            "https://tauri.localhost/app/pulls",
            "http://tauri.localhost/app/pulls",
        ] {
            let app = url::Url::parse(address).unwrap();
            for operation in [Operation::SetDemandOwner, Operation::TransportBindings] {
                assert!(caller_allowed("main", &app, operation));
                assert!(!caller_allowed("tab-webview:1", &app, operation));
            }
            for operation in [Operation::AcquireDemand, Operation::LocalLinks] {
                assert!(caller_allowed("tab-webview:1", &app, operation));
            }
        }
        for address in [
            "http://localhost:1420/app/pulls",
            "http://127.0.0.1:1420/app/pulls",
        ] {
            let app = url::Url::parse(address).unwrap();
            for operation in [Operation::SetDemandOwner, Operation::TransportBindings] {
                assert_eq!(
                    caller_allowed("main", &app, operation),
                    cfg!(debug_assertions)
                );
            }
            for operation in [Operation::AcquireDemand, Operation::LocalLinks] {
                assert_eq!(
                    caller_allowed("tab-webview:1", &app, operation),
                    cfg!(debug_assertions)
                );
            }
        }
    }
}
