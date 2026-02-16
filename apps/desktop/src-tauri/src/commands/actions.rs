use git::{core::get_services, models::status::GetStatusResponse, AppState};

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
pub async fn git_add(file: &str, state: tauri::State<'_, AppState>) -> Result<String, String> {
    let services = get_services(state).await?;
    services.action().git_add(file).await
}

#[tauri::command]
pub async fn git_remove(file: &str, state: tauri::State<'_, AppState>) -> Result<String, String> {
    let services = get_services(state).await?;
    services.action().git_remove(file).await
}

#[tauri::command]
pub async fn git_discard(
    file: &str,
    all: Option<bool>,
    state: tauri::State<'_, AppState>,
) -> Result<String, String> {
    let services = get_services(state).await?;
    services.action().git_discard(file, all).await
}
