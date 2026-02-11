use git;
use ipc;
use log::LevelFilter;
use std::sync::{Arc, Mutex};
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_window_state::Builder::default().build())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(tauri_plugin_opener::init())
        .plugin(
            tauri_plugin_log::Builder::new()
                .level_for("tao", LevelFilter::Off)
                .build(),
        )
        .setup(|app| {
            let repo_manager = ipc::repo_manager::RepoManager::new(app.handle().clone());
            app.manage(Arc::new(Mutex::new(repo_manager)));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            ipc::commands::add_local_git_repo,
            ipc::repo_manager::list_repositories,
            ipc::repo_manager::add_repository,
            ipc::repo_manager::remove_repository,
            ipc::repo_manager::refresh_repository_info,
            git::commands::git::git_add,
            git::commands::git::get_status,
            git::commands::git::get_file_status,
            git::commands::git::git_remove,
            git::commands::git::git_discard,
            git::commands::git::git_fetch,
            git::commands::git::git_push,
            git::commands::git::git_pull,
            git::commands::git::git_switch_branch,
            git::commands::git::git_create_branch,
            git::commands::git::git_publish_branch,
            git::commands::git::git_version,
            git::commands::git::has_uncommitted_changes,
            git::commands::commit::last_commit,
            git::commands::commit::commit_by_id,
            git::commands::commit::create_commit,
            git::commands::commit::create_empty_commit,
            git::commands::branch::list_branches,
            git::commands::branch::current_branch,
            git::commands::branch::status_ahead_behind,
            git::commands::diff::get_patch_by_file_path,
            git::commands::history::history,
            git::commands::history::history_graph,
            git::commands::origin::repository_origin,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
