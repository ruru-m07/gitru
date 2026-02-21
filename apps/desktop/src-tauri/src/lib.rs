use git::{core::RepoServices, AppState};
use ipc::{
    self,
    repo_manager::{RepoManager, SELECTED_REPO_KEY, STORE_FILE},
};
use log::LevelFilter;
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
            services: RwLock::new(None),
        })
        .setup(|app| {
            setup_managers(app);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            ipc::commands::add_local_git_repo,
            ipc::commands::select_repository,
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
            commands::actions::git_version,
            commands::actions::get_status,
            commands::actions::git_fetch,
            commands::actions::git_add,
            commands::actions::git_remove,
            commands::actions::git_discard,
            commands::updater::check_for_update_by_channel,
            commands::updater::install_update_by_channel,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

fn setup_managers(app: &mut App) {
    let app_handle = app.handle().clone();

    let repo_manager = RepoManager::new(app_handle.clone());
    app.manage(Arc::new(Mutex::new(repo_manager)));

    tauri::async_runtime::spawn(async move {
        let store = match app_handle.store(STORE_FILE) {
            Ok(s) => s,
            Err(_) => return,
        };

        let selected_id: Option<String> = store
            .get(SELECTED_REPO_KEY)
            .and_then(|v| serde_json::from_value(v.clone()).ok());

        if let Some(id) = selected_id {
            let manager_state = app_handle.state::<Arc<Mutex<RepoManager>>>();
            let app_state = app_handle.state::<AppState>();

            let repos = {
                let app = {
                    let manager = manager_state.lock().unwrap();
                    manager.app.clone()
                };
                let temp = RepoManager::new(app);
                temp.list_repositories(false).await.ok()
            };

            if let Some(repos) = repos {
                if let Some(repo) = repos.into_iter().find(|r| r.id == id) {
                    if let Ok(services) = RepoServices::new(&repo.path) {
                        let mut lock = app_state.services.write().await;
                        *lock = Some(Arc::new(services));
                    }
                }
            }
        }
    });
}
