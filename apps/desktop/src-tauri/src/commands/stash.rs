use git::{
    core::get_services,
    models::stash::{StashEntry, StashQuickStat, StashShowResponse},
    runner::validate_relative_path,
    AppState,
};

#[tauri::command]
pub async fn stash_list(state: tauri::State<'_, AppState>) -> Result<Vec<StashEntry>, String> {
    let services = get_services(state).await?;
    services.stash().list().await
}

#[tauri::command]
pub async fn stash_quick_stat(
    reference: &str,
    state: tauri::State<'_, AppState>,
) -> Result<StashQuickStat, String> {
    let services = get_services(state).await?;
    services.stash().quick_stat(reference).await
}

#[tauri::command]
pub async fn stash_show(
    reference: &str,
    state: tauri::State<'_, AppState>,
) -> Result<StashShowResponse, String> {
    let services = get_services(state).await?;
    services.stash().show(reference).await
}

#[tauri::command]
pub async fn stash_push(
    message: Option<String>,
    include_untracked: Option<bool>,
    state: tauri::State<'_, AppState>,
) -> Result<String, String> {
    let services = get_services(state).await?;
    services
        .stash()
        .push(message.as_deref(), include_untracked.unwrap_or(false))
        .await
}

#[tauri::command]
pub async fn stash_pop(
    reference: Option<String>,
    state: tauri::State<'_, AppState>,
) -> Result<String, String> {
    let services = get_services(state).await?;
    services.stash().pop(reference.as_deref()).await
}

#[tauri::command]
pub async fn stash_apply(
    reference: Option<String>,
    state: tauri::State<'_, AppState>,
) -> Result<String, String> {
    let services = get_services(state).await?;
    services.stash().apply(reference.as_deref()).await
}

#[tauri::command]
pub async fn stash_drop(
    reference: &str,
    state: tauri::State<'_, AppState>,
) -> Result<String, String> {
    let services = get_services(state).await?;
    services.stash().drop(reference).await
}

#[tauri::command]
pub async fn stash_clear(state: tauri::State<'_, AppState>) -> Result<String, String> {
    let services = get_services(state).await?;
    services.stash().clear().await
}

#[tauri::command]
pub async fn stash_branch(
    branch_name: &str,
    reference: Option<String>,
    state: tauri::State<'_, AppState>,
) -> Result<String, String> {
    let services = get_services(state).await?;
    services
        .stash()
        .branch(branch_name, reference.as_deref())
        .await
}

#[tauri::command]
pub async fn stash_restore_file(
    reference: &str,
    file_path: &str,
    state: tauri::State<'_, AppState>,
) -> Result<String, String> {
    validate_relative_path(file_path)?;

    let services = get_services(state).await?;
    services.stash().restore_file(reference, file_path).await
}
