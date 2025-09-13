#[tauri::command]
pub fn my_custom_command() {
    println!("I was invoked from JavaScript!");
}

#[tauri::command]
pub fn add(a: i32, b: i32) -> i32 {
    a + b
}
