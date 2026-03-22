use git::{
    core::get_services,
    models::{
        diff::{DiffScope, PatchAction, PatchRange},
        status::GetStatusResponse,
    },
    AppState,
};

#[tauri::command]
pub async fn git_version(state: tauri::State<'_, AppState>) -> Result<String, String> {
    let services = get_services(state).await?;
    services.action().git_version().await
}

#[tauri::command]
pub async fn get_status(state: tauri::State<'_, AppState>) -> Result<GetStatusResponse, String> {
    let services = get_services(state).await?;
    services.action().get_status().await
}

#[tauri::command]
pub async fn git_fetch(state: tauri::State<'_, AppState>) -> Result<String, String> {
    let services = get_services(state).await?;
    services.action().git_fetch().await
}

#[tauri::command]
pub async fn git_add(
    file: Option<String>,
    files: Option<Vec<String>>,
    state: tauri::State<'_, AppState>,
) -> Result<String, String> {
    let services = get_services(state).await?;
    services
        .action()
        .git_add(file.as_deref(), files.as_deref())
        .await
}

#[tauri::command]
pub async fn git_remove(
    file: Option<String>,
    files: Option<Vec<String>>,
    state: tauri::State<'_, AppState>,
) -> Result<String, String> {
    let services = get_services(state).await?;
    services
        .action()
        .git_remove(file.as_deref(), files.as_deref())
        .await
}

#[tauri::command]
pub async fn git_discard(
    file: Option<String>,
    files: Option<Vec<String>>,
    all: Option<bool>,
    state: tauri::State<'_, AppState>,
) -> Result<String, String> {
    let services = get_services(state).await?;
    services
        .action()
        .git_discard(file.as_deref(), files.as_deref(), all)
        .await
}

#[tauri::command]
pub async fn git_apply_patch_block(
    file_path: String,
    file_new_path: Option<String>,
    diff_scope: DiffScope,
    additions: PatchRange,
    deletions: PatchRange,
    action: PatchAction,
    state: tauri::State<'_, AppState>,
) -> Result<String, String> {
    let services = get_services(state).await?;
    services
        .action()
        .git_apply_patch_block(
            &file_path,
            file_new_path.as_deref(),
            diff_scope,
            additions,
            deletions,
            action,
        )
        .await
}
