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
            git::commands::git::git_add,
            git::commands::git::get_status,
            git::commands::git::git_remove,
            git::commands::git::git_discard,
            git::commands::git::git_fetch,
            git::commands::git::git_push,
            git::commands::commit::last_commit,
            git::commands::commit::commit_by_id,
            git::commands::commit::create_commit,
            git::commands::commit::create_empty_commit,
            git::commands::branch::list_branches,
            git::commands::branch::current_branch,
            git::commands::diff::get_diff,
            git::commands::history::history,
            git::commands::origin::repository_origin,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
