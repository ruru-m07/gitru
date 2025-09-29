use tauri::command;

use crate::{
    diff::FileVersion,
    status::{
        FileStatus, FileStatusKind, collect_statuses, default_status_options,
        types::GetStatusResponse,
    },
};

#[command(rename_all = "snake_case")]
pub fn get_status(repo_path: &str) -> Result<GetStatusResponse, String> {
    let mut opts = default_status_options();
    let files = collect_statuses(repo_path, &mut opts)?;
    Ok(GetStatusResponse { files })
}

#[tauri::command(rename_all = "snake_case")]
pub fn generate_file_status() -> FileStatus {
    todo!()
}

#[tauri::command(rename_all = "snake_case")]
pub fn generate_file_status_kind() -> FileStatusKind {
    todo!()
}

#[tauri::command(rename_all = "snake_case")]
pub fn generate_file_version() -> FileVersion {
    todo!()
}
