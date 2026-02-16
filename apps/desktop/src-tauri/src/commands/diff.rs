use git::models::diff::FileDiff;
use git::service::core::get_services;
use git::service::runner::validate_relative_path;
use git::AppState;

#[tauri::command]
pub async fn get_patch_by_file_path(
    file_path: &str,
    state: tauri::State<'_, AppState>,
) -> Result<FileDiff, String> {
    validate_relative_path(file_path)?;

    let services = get_services(state).await?;

    let patch = services.diff().get_patch_by_file_path(file_path).await?;

    return Ok(FileDiff { patch });
}
