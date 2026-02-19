use git::core::get_services;
use git::models::branch::{
    AheadBehindStatus, Branch, BranchInfo, BranchKind, BranchStash, UncommittedChangesStrategy,
};
use git::AppState;

#[tauri::command]
pub async fn current_branch(state: tauri::State<'_, AppState>) -> Result<Branch, String> {
    let services = get_services(state).await?;
    services.branch().get_current_branch().await
}

#[tauri::command]
pub async fn list_branches(
    kind: BranchKind,
    state: tauri::State<'_, AppState>,
) -> Result<Vec<BranchInfo>, String> {
    let services = get_services(state).await?;
    services.branch().list_branches(kind).await
}

#[tauri::command]
pub async fn status_ahead_behind(
    state: tauri::State<'_, AppState>,
) -> Result<AheadBehindStatus, String> {
    let services = get_services(state).await?;
    services.branch().status_ahead_behind().await
}

#[tauri::command]
pub async fn get_branch_info(
    branch_name: &str,
    state: tauri::State<'_, AppState>,
) -> Result<BranchInfo, String> {
    let services = get_services(state).await?;
    services.branch().get_branch_info(branch_name).await
}

#[tauri::command]
pub async fn switch_branch(
    branch: &str,
    strategy: Option<UncommittedChangesStrategy>,
    state: tauri::State<'_, AppState>,
) -> Result<String, String> {
    let services = get_services(state).await?;
    services.branch().switch(branch, strategy).await
}

#[tauri::command]
pub async fn create_branch(
    branch: &str,
    strategy: Option<UncommittedChangesStrategy>,
    state: tauri::State<'_, AppState>,
) -> Result<String, String> {
    let services = get_services(state).await?;
    services.branch().create(branch, strategy).await
}

#[tauri::command]
pub async fn push(state: tauri::State<'_, AppState>) -> Result<String, String> {
    let services = get_services(state).await?;
    services.branch().push().await
}

#[tauri::command]
pub async fn publish_branch(state: tauri::State<'_, AppState>) -> Result<String, String> {
    let services = get_services(state).await?;
    services.branch().publish_branch().await
}

#[tauri::command]
pub async fn pull(state: tauri::State<'_, AppState>) -> Result<String, String> {
    let services = get_services(state).await?;
    services.branch().pull().await
}

#[tauri::command]
pub async fn has_uncommitted_changes(state: tauri::State<'_, AppState>) -> Result<bool, String> {
    let services = get_services(state).await?;
    services.branch().has_uncommitted_changes().await
}

#[tauri::command]
pub async fn current_branch_stash(
    state: tauri::State<'_, AppState>,
) -> Result<Option<BranchStash>, String> {
    let services = get_services(state).await?;
    services.branch().current_branch_stash().await
}

#[tauri::command]
pub async fn pop_current_branch_stash(state: tauri::State<'_, AppState>) -> Result<String, String> {
    let services = get_services(state).await?;
    services.branch().pop_current_branch_stash().await
}
