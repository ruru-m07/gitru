use git::core::get_services;
use git::models::diff::{DiffScope, FileDiff};
use git::models::status::FileStatusKind;
use git::runner::validate_relative_path;
use git::AppState;

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn get_patch_by_file_path(
    context_id: String,
    file_path: &str,
    file_new_path: Option<String>,
    status: Option<Vec<FileStatusKind>>,
    stash_reference: Option<String>,
    commit_hash: Option<String>,
    parent_index: Option<usize>,
    diff_scope: Option<DiffScope>,
    state: tauri::State<'_, AppState>,
) -> Result<FileDiff, String> {
    validate_relative_path(file_path)?;
    if let Some(ref new_path) = file_new_path {
        validate_relative_path(new_path)?;
    }

    let services = get_services(state, &context_id).await?;

    services
        .diff()
        .get_patch_by_file_path(
            file_path,
            file_new_path.as_deref(),
            status.as_deref(),
            stash_reference.as_deref(),
            commit_hash.as_deref(),
            parent_index,
            diff_scope,
        )
        .await
}
