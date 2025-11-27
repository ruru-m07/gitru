use git;
use ipc;
use tauri_plugin_updater;
use tauri_plugin_window_state;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_window_state::Builder::default().build())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            ipc::commands::add_local_git_repo,
            git::commands::git_add,
            git::commands::get_status,
            git::commands::git_remove,
            git::commands::git_discard,
            git::commands::commit,
            git::diff::get_diff,
            git::branch::list_branch,
            git::branch::current_branch,
            git::branch::switch_branch,
            git::history::history,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
