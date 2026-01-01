use git2::Repository;
use serde::Serialize;

use crate::{
    status::{collect_statuses, default_status_options},
    types::{GetStatusResponse, GitResult},
};

#[derive(Serialize)]
pub struct CommitResult {
    success: bool,
    message: Option<String>,
}

#[tauri::command]
pub fn get_status(repo_path: &str) -> Result<GetStatusResponse, String> {
    let mut opts = default_status_options();
    let files = collect_statuses(repo_path, &mut opts)?;
    Ok(GetStatusResponse { files })
}

// git add <file>
#[tauri::command]
pub fn git_add(repo_path: &str, file: &str) -> GitResult {
    let repo = match Repository::open(repo_path) {
        Ok(r) => r,
        Err(e) => {
            return GitResult {
                success: false,
                message: Some(format!("Failed to open repo: {e}")),
            };
        }
    };

    let mut index = match repo.index() {
        Ok(i) => i,
        Err(e) => {
            return GitResult {
                success: false,
                message: Some(format!("Failed to open index: {e}")),
            };
        }
    };

    if file == "." {
        if let Err(e) = index.add_all(["*"].iter(), git2::IndexAddOption::DEFAULT, None) {
            return GitResult {
                success: false,
                message: Some(format!("Failed to add all: {e}")),
            };
        }
    } else {
        let file_path = std::path::Path::new(file);
        let full_path = std::path::Path::new(repo_path).join(file_path);

        if full_path.exists() {
            if let Err(e) = index.add_path(file_path) {
                return GitResult {
                    success: false,
                    message: Some(format!("Failed to add {file}: {e}")),
                };
            }
        } else {
            // ? File doesn't exist (deleted), remove it from the index
            if let Err(e) = index.remove_path(file_path) {
                return GitResult {
                    success: false,
                    message: Some(format!("Failed to stage deletion of {file}: {e}")),
                };
            }
        }
    }

    if let Err(e) = index.write() {
        return GitResult {
            success: false,
            message: Some(format!("Failed to write index: {e}")),
        };
    }

    GitResult {
        success: true,
        message: None,
    }
}

// git restore --staged <file>
#[tauri::command]
pub fn git_remove(repo_path: &str, file: &str) -> GitResult {
    println!("{:?} --- {:?}", repo_path, file);

    let repo = match Repository::open(repo_path) {
        Ok(r) => r,
        Err(e) => {
            return GitResult {
                success: false,
                message: Some(format!("Failed to open repo: {e}")),
            };
        }
    };

    let mut index = match repo.index() {
        Ok(i) => i,
        Err(e) => {
            return GitResult {
                success: false,
                message: Some(format!("Failed to open index: {e}")),
            };
        }
    };

    // Get HEAD tree
    let head = match repo.head() {
        Ok(h) => h,
        Err(e) => {
            return GitResult {
                success: false,
                message: Some(format!("Failed to get HEAD: {e}")),
            };
        }
    };

    let tree = match head.peel_to_tree() {
        Ok(t) => t,
        Err(e) => {
            return GitResult {
                success: false,
                message: Some(format!("Failed to get tree: {e}")),
            };
        }
    };

    if file == "." {
        // Unstage all files by resetting index to HEAD
        if let Err(e) = index.read_tree(&tree) {
            return GitResult {
                success: false,
                message: Some(format!("Failed to reset index: {e}")),
            };
        }
    } else {
        // For a specific file, we need to reset it to the HEAD version
        let tree_entry = tree.get_path(std::path::Path::new(file));

        match tree_entry {
            Ok(entry) => {
                // File exists in HEAD, restore it to that version
                let index_entry = git2::IndexEntry {
                    ctime: git2::IndexTime::new(0, 0),
                    mtime: git2::IndexTime::new(0, 0),
                    dev: 0,
                    ino: 0,
                    mode: entry.filemode() as u32,
                    uid: 0,
                    gid: 0,
                    file_size: 0,
                    id: entry.id(),
                    flags: 0,
                    flags_extended: 0,
                    path: file.as_bytes().to_vec(),
                };

                if let Err(e) = index.add(&index_entry) {
                    return GitResult {
                        success: false,
                        message: Some(format!("Failed to reset {file} to HEAD: {e}")),
                    };
                }
            }
            Err(_) => {
                // File doesn't exist in HEAD (it's a new file), so remove it from index
                if let Err(e) = index.remove_path(std::path::Path::new(file)) {
                    return GitResult {
                        success: false,
                        message: Some(format!("Failed to unstage {file}: {e}")),
                    };
                }
            }
        }
    }

    if let Err(e) = index.write() {
        return GitResult {
            success: false,
            message: Some(format!("Failed to write index: {e}")),
        };
    }

    GitResult {
        success: true,
        message: None,
    }
}

// git restore <file>
#[tauri::command]
pub fn git_discard(repo_path: &str, file: &str) -> GitResult {
    let repo = match Repository::open(repo_path) {
        Ok(r) => r,
        Err(e) => {
            return GitResult {
                success: false,
                message: Some(format!("Failed to open repo: {e}")),
            };
        }
    };

    let obj = match repo.head() {
        Ok(head) => match head.peel(git2::ObjectType::Commit) {
            Ok(obj) => obj,
            Err(e) => {
                return GitResult {
                    success: false,
                    message: Some(format!("Failed to peel HEAD: {e}")),
                };
            }
        },
        Err(_) => {
            return GitResult {
                success: false,
                message: Some("No HEAD to discard from".into()),
            };
        }
    };

    let res = if file == "." {
        repo.checkout_tree(&obj, None)
    } else {
        let mut opts = git2::build::CheckoutBuilder::new();
        opts.path(std::path::Path::new(file)).force();
        repo.checkout_tree(&obj, Some(&mut opts))
    };

    if let Err(e) = res {
        return GitResult {
            success: false,
            message: Some(format!("Failed to discard {file}: {e}")),
        };
    }

    // Remove untracked files
    if file == "." {
        // Get all untracked files
        let mut opts = git2::StatusOptions::new();
        opts.include_untracked(true);

        if let Ok(statuses) = repo.statuses(Some(&mut opts)) {
            for entry in statuses.iter() {
                if entry.status().is_wt_new() {
                    if let Some(path) = entry.path() {
                        let full_path = std::path::Path::new(repo_path).join(path);
                        let _ = std::fs::remove_file(full_path);
                    }
                }
            }
        }
    } else {
        // Check if specific file is untracked
        let file_path = std::path::Path::new(repo_path).join(file);
        if let Ok(status) = repo.status_file(std::path::Path::new(file)) {
            if status.is_wt_new() {
                let _ = std::fs::remove_file(file_path);
            }
        }
    }

    GitResult {
        success: true,
        message: None,
    }
}
