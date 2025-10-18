use git2::{BranchType, Repository};

#[tauri::command(rename_all = "snake_case")]
pub fn current_branch(repo_path: &str) {}

#[tauri::command(rename_all = "snake_case")]
pub fn list_branch(repo_path: &str) {}

#[tauri::command(rename_all = "snake_case")]
pub fn switch_branch(repo_path: &str) {}
