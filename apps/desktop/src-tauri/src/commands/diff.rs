use git::core::get_services;
use git::models::diff::FileDiff;
use git::runner::validate_relative_path;
use git::AppState;

#[tauri::command]
pub async fn get_patch_by_file_path(
    file_path: &str,
    stash_reference: Option<String>,
    state: tauri::State<'_, AppState>,
) -> Result<FileDiff, String> {
    validate_relative_path(file_path)?;

    let services = get_services(state).await?;

    let patch = services
        .diff()
        .get_patch_by_file_path(file_path, stash_reference.as_deref())
        .await?;

    return Ok(FileDiff { patch });
}
