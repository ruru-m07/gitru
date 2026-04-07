use git::{
    core::get_services,
    models::stash::{StashEntry, StashQuickStat, StashShowResponse},
    runner::validate_relative_path,
    AppState,
};

#[tauri::command]
pub async fn stash_list(
    context_id: String,
    state: tauri::State<'_, AppState>,
) -> Result<Vec<StashEntry>, String> {
    let services = get_services(state, &context_id).await?;
    services.stash().list().await
}

#[tauri::command]
pub async fn stash_quick_stat(
    context_id: String,
    reference: &str,
    state: tauri::State<'_, AppState>,
) -> Result<StashQuickStat, String> {
    let services = get_services(state, &context_id).await?;
    services.stash().quick_stat(reference).await
}

#[tauri::command]
pub async fn stash_show(
    context_id: String,
    reference: &str,
    state: tauri::State<'_, AppState>,
) -> Result<StashShowResponse, String> {
    let services = get_services(state, &context_id).await?;
    services.stash().show(reference).await
}

#[tauri::command]
pub async fn stash_push(
    context_id: String,
    message: Option<String>,
    include_untracked: Option<bool>,
    state: tauri::State<'_, AppState>,
) -> Result<String, String> {
    let services = get_services(state, &context_id).await?;
    services
        .stash()
        .push(message.as_deref(), include_untracked.unwrap_or(false))
        .await
}

#[tauri::command]
pub async fn stash_pop(
    context_id: String,
    reference: Option<String>,
    state: tauri::State<'_, AppState>,
) -> Result<String, String> {
    let services = get_services(state, &context_id).await?;
    services.stash().pop(reference.as_deref()).await
}

#[tauri::command]
pub async fn stash_apply(
    context_id: String,
    reference: Option<String>,
    state: tauri::State<'_, AppState>,
) -> Result<String, String> {
    let services = get_services(state, &context_id).await?;
    services.stash().apply(reference.as_deref()).await
}

#[tauri::command]
pub async fn stash_drop(
    context_id: String,
    reference: &str,
    state: tauri::State<'_, AppState>,
) -> Result<String, String> {
    let services = get_services(state, &context_id).await?;
    services.stash().drop(reference).await
}

#[tauri::command]
pub async fn stash_clear(
    context_id: String,
    state: tauri::State<'_, AppState>,
) -> Result<String, String> {
    let services = get_services(state, &context_id).await?;
    services.stash().clear().await
}

#[tauri::command]
pub async fn stash_branch(
    context_id: String,
    branch_name: &str,
    reference: Option<String>,
    state: tauri::State<'_, AppState>,
) -> Result<String, String> {
    let services = get_services(state, &context_id).await?;
    services
        .stash()
        .branch(branch_name, reference.as_deref())
        .await
}

#[tauri::command]
pub async fn stash_restore_file(
    context_id: String,
    reference: &str,
    file_path: &str,
    state: tauri::State<'_, AppState>,
) -> Result<String, String> {
    validate_relative_path(file_path)?;

    let services = get_services(state, &context_id).await?;
    services.stash().restore_file(reference, file_path).await
}
