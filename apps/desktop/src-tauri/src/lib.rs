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

#[cfg(feature = "e2e")]
const E2E_RESET_ENV: &str = "GITRU_E2E_RESET";
#[cfg(feature = "e2e")]
const E2E_IDENTIFIER: &str = "com.ruru.gitru.e2e";

#[cfg(target_os = "macos")]
mod app_menu;
mod commands;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = tauri::Builder::default()
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
        .manage(Arc::new(SessionManager::new()));

    #[cfg(target_os = "macos")]
    let builder = builder
        .menu(app_menu::build)
        .on_menu_event(app_menu::handle_event);

    #[cfg(feature = "e2e")]
    let builder = builder
        .plugin(tauri_plugin_wdio::init())
        .plugin(tauri_plugin_wdio_webdriver::init());

    builder
        .setup(|app| {
            #[cfg(feature = "e2e")]
            reset_e2e_state(app)?;
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
            commands::history::commit_activity,
            commands::pickaxe::start_pickaxe,
            commands::pickaxe::cancel_pickaxe,
            commands::origin::repository_origin,
            commands::security::open_external_url,
            commands::commit::last_commit,
            commands::commit::commit_by_id,
            commands::commit::create_commit,
            commands::branch::push,
            commands::branch::publish_branch,
            commands::branch::pull,
            commands::branch::switch_branch,
            commands::branch::create_branch,
            commands::branch::rename_branch,
            commands::branch::delete_local_branch,
            commands::branch::delete_remote_branch,
            commands::branch::set_branch_upstream,
            commands::branch::unset_branch_upstream,
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
            commands::actions::read_worktree_file,
            commands::actions::write_worktree_file,
            commands::rebase::get_repo_operation,
            commands::rebase::rebase_plan,
            commands::rebase::rebase_start,
            commands::rebase::rebase_continue,
            commands::rebase::rebase_skip,
            commands::rebase::rebase_abort,
            commands::rebase::rebase_abort_preview,
            commands::rebase::rebase_update_todo,
            commands::rebase::rebase_set_commit_message,
            commands::rebase::rebase_resolve_conflict,
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

#[cfg(feature = "e2e")]
fn reset_e2e_state(app: &App) -> Result<(), Box<dyn std::error::Error>> {
    if std::env::var(E2E_RESET_ENV).as_deref() != Ok("1") {
        return Ok(());
    }

    if app.config().identifier != E2E_IDENTIFIER {
        return Err(format!(
            "refusing to reset app state for identifier {}; expected {E2E_IDENTIFIER}",
            app.config().identifier
        )
        .into());
    }

    let data_dir = app.path().app_data_dir()?;
    for file_name in [STORE_FILE, "app-state.json"] {
        let path = data_dir.join(file_name);
        match std::fs::remove_file(path) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
    }

    Ok(())
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
