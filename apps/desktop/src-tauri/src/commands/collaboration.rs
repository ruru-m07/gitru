//! Local reads are separate commands from network refresh intents.
use collaboration::{
    AccountSnapshot, CapabilitySnapshot, ChangePage, CollaborationError, CollaborationRuntime,
    ContextCapabilityRequest, ContextualCapabilitySnapshot, DetailQuery, DetailSnapshot, ErrorCode,
    GithubCliDiscovery, HydrateDetailRequest, ItemPage, ItemQuery, ItemSnapshot, LocalDraft,
    RefreshReceipt, RefreshRequest, RemoteAccount, RepositorySnapshot, ResourceLocator,
    ResourceResolution,
};
use std::sync::Arc;
use tauri::{State, Webview};
use tokio::sync::OnceCell;

#[derive(Default)]
pub struct CollaborationState {
    pub runtime: OnceCell<Result<Arc<CollaborationRuntime>, CollaborationError>>,
}

impl CollaborationState {
    async fn get(&self) -> Result<&Arc<CollaborationRuntime>, CollaborationError> {
        // Initialization starts during setup. A bounded wait makes startup reads
        // resilient without blocking the app shell or requiring network access.
        for _ in 0..100 {
            if let Some(result) = self.runtime.get() {
                return result.as_ref().map_err(Clone::clone);
            }
            tokio::time::sleep(std::time::Duration::from_millis(25)).await;
        }
        Err(CollaborationError::new(
            ErrorCode::NotReady,
            "Collaboration storage is still starting",
        ))
    }
}

#[derive(Clone, Copy)]
enum Operation {
    Accounts,
    ConnectGithub,
    DiscoverGithubCli,
    ConnectGithubCli,
    Disconnect,
    Repositories,
    SelectRepository,
    Items,
    Item,
    Refresh,
    ChangesSince,
    SaveDraft,
    Draft,
    Capabilities,
    ContextualCapabilities,
    ResolveResource,
    Detail,
    HydrateDetail,
}

impl Operation {
    fn requires_main(self) -> bool {
        matches!(
            self,
            Self::ConnectGithub
                | Self::DiscoverGithubCli
                | Self::ConnectGithubCli
                | Self::Disconnect
        )
    }
}

fn authorize(view: &Webview, operation: Operation) -> Result<(), CollaborationError> {
    let url = view.url().map_err(|_| denied())?;
    if !caller_allowed(view.label(), &url, operation) {
        return Err(denied());
    }
    Ok(())
}

fn caller_allowed(label: &str, url: &url::Url, operation: Operation) -> bool {
    let local = matches!(
        (url.scheme(), url.host_str()),
        ("tauri", Some("localhost"))
            | ("https", Some("tauri.localhost"))
            | ("http", Some("tauri.localhost"))
    ) || (cfg!(debug_assertions)
        && url.scheme() == "http"
        && matches!(url.host_str(), Some("localhost" | "127.0.0.1"))
        && url.port() == Some(1420));
    local && (label == "main" || (!operation.requires_main() && label.starts_with("tab-webview:")))
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
    state.get().await?.store().accounts().await
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
    state.get().await?.store().repositories(&account_id).await
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
    state.get().await?.store().query_items(query).await
}

#[tauri::command]
pub async fn collaboration_item(
    account_id: String,
    item_id: String,
    view: Webview,
    state: State<'_, CollaborationState>,
) -> Result<ItemSnapshot, CollaborationError> {
    authorize(&view, Operation::Item)?;
    state.get().await?.store().item(&account_id, &item_id).await
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
    state
        .get()
        .await?
        .store()
        .draft(&account_id, &subject_id)
        .await
}

#[tauri::command]
pub async fn collaboration_contextual_capabilities(
    request: ContextCapabilityRequest,
    view: Webview,
    state: State<'_, CollaborationState>,
) -> Result<ContextualCapabilitySnapshot, CollaborationError> {
    authorize(&view, Operation::ContextualCapabilities)?;
    state.get().await?.contextual_capabilities(request).await
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
pub async fn collaboration_detail(
    query: DetailQuery,
    view: Webview,
    state: State<'_, CollaborationState>,
) -> Result<DetailSnapshot, CollaborationError> {
    authorize(&view, Operation::Detail)?;
    state.get().await?.store().detail(query).await
}

#[tauri::command]
pub async fn collaboration_hydrate_detail(
    request: HydrateDetailRequest,
    view: Webview,
    state: State<'_, CollaborationState>,
) -> Result<RefreshReceipt, CollaborationError> {
    authorize(&view, Operation::HydrateDetail)?;
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
        Operation::Refresh,
        Operation::ChangesSince,
        Operation::SaveDraft,
        Operation::Draft,
        Operation::Capabilities,
        Operation::ContextualCapabilities,
        Operation::ResolveResource,
        Operation::Detail,
        Operation::HydrateDetail,
    ];
    const CREDENTIAL_OPERATIONS: &[Operation] = &[
        Operation::ConnectGithub,
        Operation::DiscoverGithubCli,
        Operation::ConnectGithubCli,
        Operation::Disconnect,
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
        for &operation in CREDENTIAL_OPERATIONS {
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
        ] {
            let url = url::Url::parse(url).unwrap();
            for &operation in DOMAIN_OPERATIONS.iter().chain(CREDENTIAL_OPERATIONS) {
                assert!(!caller_allowed("main", &url, operation));
                assert!(!caller_allowed("tab-webview:1", &url, operation));
            }
        }
    }
}
