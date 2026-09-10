#![cfg_attr(
    all(not(debug_assertions), not(feature = "e2e")),
    windows_subsystem = "windows"
)]
fn main() {
    gitru_lib::run()
}
