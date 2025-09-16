use git;
use ipc;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            ipc::commands::add,
            git::commands::get_status,
            // git::commands::get_status_single_file
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
