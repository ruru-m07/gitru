use git;
use ipc;
use tauri_plugin_window_state;
use watcher;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_window_state::Builder::default().build())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(tauri_plugin_opener::init())
        .manage(watcher::commands::WatcherState::default())
        .invoke_handler(tauri::generate_handler![
            ipc::commands::add_local_git_repo,
            git::diff::get_diff,
            git::commands::get_status,
            git::commands::git_add,
            git::commands::git_remove,
            git::commands::git_discard,
            git::commands::commit,
            git::branch::list_branch,
            git::branch::current_branch,
            git::branch::switch_branch,
            watcher::commands::start_watching,
            watcher::commands::stop_watching,
            watcher::commands::rescan_repository,
            watcher::commands::get_watcher_state,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
