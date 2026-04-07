use git::AppState;
use ipc::{
    self,
    repo_manager::{RepoManager, STORE_FILE},
    session_manager::SessionManager,
};
use log::LevelFilter;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tauri::{App, Manager};
use tauri_plugin_store::StoreExt;
use tokio::sync::RwLock;

mod commands;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_window_state::Builder::default().build())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(tauri_plugin_opener::init())
        .plugin(
            tauri_plugin_log::Builder::new()
                .level_for("tao", LevelFilter::Off)
                .build(),
        )
        .manage(AppState {
            services: RwLock::new(HashMap::new()),
        })
        .manage(Arc::new(SessionManager::new()))
        .setup(|app| {
            setup_managers(app);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            ipc::commands::add_local_git_repo,
            ipc::commands::clone_repository,
            ipc::commands::cancel_clone_repository,
            ipc::commands::init_repository,
            ipc::commands::create_repo_context,
            ipc::commands::dispose_repo_context,
            ipc::commands::open_with_app,
            ipc::repo_manager::list_repositories,
            ipc::repo_manager::add_repository,
            ipc::repo_manager::remove_repository,
            ipc::repo_manager::refresh_repository_info,
            commands::branch::list_branches,
            commands::branch::current_branch,
            commands::branch::status_ahead_behind,
            commands::branch::get_branch_info,
            commands::diff::get_patch_by_file_path,
            commands::history::history,
            commands::history::history_graph,
            commands::origin::repository_origin,
            commands::commit::last_commit,
            commands::commit::commit_by_id,
            commands::commit::create_commit,
            commands::branch::push,
            commands::branch::publish_branch,
            commands::branch::pull,
            commands::branch::switch_branch,
            commands::branch::create_branch,
            commands::branch::has_uncommitted_changes,
            commands::branch::current_branch_stash,
            commands::branch::pop_current_branch_stash,
            commands::stash::stash_list,
            commands::stash::stash_quick_stat,
            commands::stash::stash_show,
            commands::stash::stash_push,
            commands::stash::stash_pop,
            commands::stash::stash_apply,
            commands::stash::stash_drop,
            commands::stash::stash_clear,
            commands::stash::stash_branch,
            commands::stash::stash_restore_file,
            commands::actions::git_version,
            commands::actions::get_status,
            commands::actions::git_fetch,
            commands::actions::git_add,
            commands::actions::git_remove,
            commands::actions::git_discard,
            commands::actions::git_apply_patch_block,
            commands::updater::check_for_update_by_channel,
            commands::updater::download_and_install_update_by_channel,
            // Session Navigation Commands
            ipc::commands::session_push_to_history,
            ipc::commands::session_go_back,
            ipc::commands::session_go_forward,
            ipc::commands::session_get_navigation_state,
            ipc::commands::session_clear_history,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

fn setup_managers(app: &mut App) {
    let app_handle = app.handle().clone();

    let repo_manager = RepoManager::new(app_handle.clone());
    app.manage(Arc::new(Mutex::new(repo_manager)));

    let session_manager = SessionManager::new();
    app.manage(Arc::new(session_manager));

    tauri::async_runtime::spawn(async move {
        let _ = app_handle.store(STORE_FILE);
    });
}
