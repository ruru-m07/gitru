use git::models::origin::RepositoryOrigin;
use git::service::core::get_services;
use git::AppState;

#[tauri::command]
pub async fn repository_origin(
    state: tauri::State<'_, AppState>,
) -> Result<RepositoryOrigin, String> {
    let services = get_services(state).await?;
    services.origin().repository_origin().await
}
