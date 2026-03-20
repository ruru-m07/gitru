use git::AppState;
use git::core::get_services;
use git::models::diff::BlameDiff;
use git::models::status::FileStatusKind;
use git::runner::validate_relative_path;

#[tauri::command]
pub async fn get_blame_by_file_path(
    file_path: &str,
    file_new_path: Option<String>,
    status: Option<Vec<FileStatusKind>>,
    stash_reference: Option<String>,
    commit_hash: Option<String>,
    parent_index: Option<usize>,
    state: tauri::State<'_, AppState>,
) -> Result<BlameDiff, String> {
    validate_relative_path(file_path)?;
    if let Some(ref new_path) = file_new_path {
        validate_relative_path(new_path)?;
    }

    let services = get_services(state).await?;

    services
        .blame()
        .get_blame_by_file_path(
            file_path,
            file_new_path.as_deref(),
            status.as_deref(),
            stash_reference.as_deref(),
            commit_hash.as_deref(),
            parent_index,
        )
        .await
}
