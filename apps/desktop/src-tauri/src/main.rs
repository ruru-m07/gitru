#![cfg_attr(
    all(not(debug_assertions), not(feature = "e2e")),
    windows_subsystem = "windows"
)]
#[tauri_runtime_cef::cef_entry_point]
fn main() {
    gitru_lib::run()
}
