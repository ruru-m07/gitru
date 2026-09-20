use git::core::get_services;
use git::models::branch::{
    AheadBehindStatus, Branch, BranchInfo, BranchKind, BranchStash, UncommittedChangesStrategy,
};
use git::AppState;

#[tauri::command]
pub async fn current_branch(
    context_id: String,
    state: tauri::State<'_, AppState>,
) -> Result<Branch, String> {
    let services = get_services(state, &context_id).await?;
    services.branch().get_current_branch().await
}

#[tauri::command]
pub async fn list_branches(
    context_id: String,
    kind: BranchKind,
    state: tauri::State<'_, AppState>,
) -> Result<Vec<BranchInfo>, String> {
    let services = get_services(state, &context_id).await?;
    services.branch().list_branches(kind).await
}

#[tauri::command]
pub async fn status_ahead_behind(
    context_id: String,
    state: tauri::State<'_, AppState>,
) -> Result<AheadBehindStatus, String> {
    let services = get_services(state, &context_id).await?;
    services.branch().status_ahead_behind().await
}

#[tauri::command]
pub async fn get_branch_info(
    context_id: String,
    branch_name: &str,
    state: tauri::State<'_, AppState>,
) -> Result<BranchInfo, String> {
    let services = get_services(state, &context_id).await?;
    services.branch().get_branch_info(branch_name).await
}

#[tauri::command]
pub async fn switch_branch(
    context_id: String,
    branch: &str,
    strategy: Option<UncommittedChangesStrategy>,
    state: tauri::State<'_, AppState>,
) -> Result<String, String> {
    let services = get_services(state, &context_id).await?;
    services.branch().switch(branch, strategy).await
}

#[tauri::command]
pub async fn create_branch(
    context_id: String,
    branch: &str,
    strategy: Option<UncommittedChangesStrategy>,
    state: tauri::State<'_, AppState>,
) -> Result<String, String> {
    let services = get_services(state, &context_id).await?;
    services.branch().create(branch, strategy).await
}

#[tauri::command]
pub async fn rename_branch(
    context_id: String,
    branch: &str,
    new_name: &str,
    state: tauri::State<'_, AppState>,
) -> Result<String, String> {
    let services = get_services(state, &context_id).await?;
    services.branch().rename(branch, new_name).await
}

#[tauri::command]
pub async fn delete_local_branch(
    context_id: String,
    branch: &str,
    force: bool,
    state: tauri::State<'_, AppState>,
) -> Result<String, String> {
    let services = get_services(state, &context_id).await?;
    services.branch().delete_local(branch, force).await
}

#[tauri::command]
pub async fn delete_remote_branch(
    context_id: String,
    branch: &str,
    force: bool,
    state: tauri::State<'_, AppState>,
) -> Result<String, String> {
    let services = get_services(state, &context_id).await?;
    services.branch().delete_remote(branch, force).await
}

#[tauri::command]
pub async fn set_branch_upstream(
    context_id: String,
    branch: &str,
    upstream: &str,
    state: tauri::State<'_, AppState>,
) -> Result<String, String> {
    let services = get_services(state, &context_id).await?;
    services.branch().set_upstream(branch, upstream).await
}

#[tauri::command]
pub async fn unset_branch_upstream(
    context_id: String,
    branch: &str,
    state: tauri::State<'_, AppState>,
) -> Result<String, String> {
    let services = get_services(state, &context_id).await?;
    services.branch().unset_upstream(branch).await
}

#[tauri::command]
pub async fn push(context_id: String, state: tauri::State<'_, AppState>) -> Result<String, String> {
    let services = get_services(state, &context_id).await?;
    services.branch().push().await
}

#[tauri::command]
pub async fn publish_branch(
    context_id: String,
    state: tauri::State<'_, AppState>,
) -> Result<String, String> {
    let services = get_services(state, &context_id).await?;
    services.branch().publish_branch().await
}

#[tauri::command]
pub async fn pull(context_id: String, state: tauri::State<'_, AppState>) -> Result<String, String> {
    let services = get_services(state, &context_id).await?;
    services.branch().pull().await
}

#[tauri::command]
pub async fn has_uncommitted_changes(
    context_id: String,
    state: tauri::State<'_, AppState>,
) -> Result<bool, String> {
    let services = get_services(state, &context_id).await?;
    services.branch().has_uncommitted_changes().await
}

#[tauri::command]
pub async fn current_branch_stash(
    context_id: String,
    state: tauri::State<'_, AppState>,
) -> Result<Option<BranchStash>, String> {
    let services = get_services(state, &context_id).await?;
    services.branch().current_branch_stash().await
}

#[tauri::command]
pub async fn pop_current_branch_stash(
    context_id: String,
    state: tauri::State<'_, AppState>,
) -> Result<String, String> {
    let services = get_services(state, &context_id).await?;
    services.branch().pop_current_branch_stash().await
}
