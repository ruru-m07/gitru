use crate::{
    types::{FileStatus, FileStatusKind, GetStatusResponse, GitResult},
    utils::open_repository,
};
use git2::{
    BranchType, Cred, FetchOptions, FetchPrune, PushOptions, RemoteCallbacks, Status, StatusOptions,
};
use serde::Serialize;

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

// ? git add <file>
#[tauri::command]
pub fn git_add(repo_path: &str, file: &str) -> GitResult {
    let repo = match open_repository(repo_path) {
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

// ? git restore --staged <file>
#[tauri::command]
pub fn git_remove(repo_path: &str, file: &str) -> GitResult {
    let repo = match open_repository(repo_path) {
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

// ? git restore <file>
#[tauri::command]
pub fn git_discard(repo_path: &str, file: &str) -> GitResult {
    let repo = match open_repository(repo_path) {
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

// ? git fetch
#[tauri::command]
pub fn git_fetch(repo_path: &str) -> GitResult {
    let repo = match open_repository(repo_path) {
        Ok(r) => r,
        Err(e) => {
            return GitResult {
                success: false,
                message: Some(format!("Failed to open repo: {e}")),
            };
        }
    };

    // ? resolve remote (origin or fallback)
    let mut remote = match repo.find_remote("origin") {
        Ok(r) => r,
        Err(_) => {
            let remotes = match repo.remotes() {
                Ok(r) => r,
                Err(e) => {
                    return GitResult {
                        success: false,
                        message: Some(format!("Failed to list remotes: {e}")),
                    };
                }
            };

            let name = match remotes.get(0) {
                Some(n) => n,
                None => {
                    return GitResult {
                        success: false,
                        message: Some("No remotes configured".into()),
                    };
                }
            };

            match repo.find_remote(name) {
                Ok(r) => r,
                Err(e) => {
                    return GitResult {
                        success: false,
                        message: Some(format!("Failed to open remote `{name}`: {e}")),
                    };
                }
            }
        }
    };

    // ? auth callbacks
    let mut callbacks = RemoteCallbacks::new();
    callbacks.credentials(|_url, username_from_url, allowed| {
        if allowed.is_ssh_key() {
            let user = username_from_url.unwrap_or("git");
            return Cred::ssh_key_from_agent(user);
        }

        Err(git2::Error::from_str("No supported authentication method"))
    });

    let mut fo = FetchOptions::new();
    fo.remote_callbacks(callbacks);
    fo.prune(FetchPrune::On); // ? equivalent to `git fetch --prune`

    // ? fetch using configured refspecs
    match remote.fetch(&[] as &[&str], Some(&mut fo), None) {
        Ok(_) => GitResult {
            success: true,
            message: Some("Fetched successfully".into()),
        },
        Err(e) => GitResult {
            success: false,
            message: Some(format!("Failed to fetch: {e}")),
        },
    }
}

// ? git push ...
#[tauri::command]
pub fn git_push(repo_path: &str) -> GitResult {
    let repo = match open_repository(repo_path) {
        Ok(r) => r,
        Err(e) => {
            return GitResult {
                success: false,
                message: Some(format!("Failed to open repository: {e}")),
            };
        }
    };

    // ? HEAD validation
    let head = match repo.head() {
        Ok(h) => h,
        Err(e) => {
            return GitResult {
                success: false,
                message: Some(format!("Failed to read HEAD: {e}")),
            };
        }
    };

    if !head.is_branch() {
        return GitResult {
            success: false,
            message: Some("HEAD is detached, cannot push".into()),
        };
    }

    let branch_name = match head.shorthand() {
        Some(b) => b.to_string(),
        None => {
            return GitResult {
                success: false,
                message: Some("Invalid branch name".into()),
            };
        }
    };

    // ? resolve local branch
    // * usually the current checkout branch
    // * if it;s detached
    let mut branch = match repo.find_branch(&branch_name, BranchType::Local) {
        Ok(b) => b,
        Err(e) => {
            return GitResult {
                success: false,
                message: Some(format!("Failed to resolve local branch: {e}")),
            };
        }
    };

    // ? auth callbacks (SSH + HTTPS)
    let mut callbacks = RemoteCallbacks::new();
    callbacks.credentials(|url, username_from_url, allowed| {
        // TODO(ruru): will support more auth option, rn it's via ssh only
        println!("pushing to: {}", url);

        // ! ssh
        if allowed.is_ssh_key() {
            let user = username_from_url.unwrap_or("git");
            return Cred::ssh_key_from_agent(user);
        }

        Err(git2::Error::from_str("No supported authentication method"))
    });

    let mut push_opts = PushOptions::new();
    push_opts.remote_callbacks(callbacks);

    // ? resolve remote
    let mut remote = match repo.find_remote("origin") {
        Ok(r) => r,
        Err(e) => {
            return GitResult {
                success: false,
                message: Some(format!("Failed to find remote 'origin': {e}")),
            };
        }
    };

    // ? check upstream
    let has_upstream = branch.upstream().is_ok();

    let refspec = if has_upstream {
        format!("refs/heads/{0}:refs/heads/{0}", branch_name)
    } else {
        // * it's same as saying `git push -u origin {branch}`
        format!("+refs/heads/{0}:refs/heads/{0}", branch_name)
    };

    // ? pushhhhh
    if let Err(e) = remote.push(&[refspec.as_str()], Some(&mut push_opts)) {
        return GitResult {
            success: false,
            message: Some(format!("Push failed: {e}")),
        };
    }

    // ? set upstream if missing
    if !has_upstream {
        if let Err(e) = branch.set_upstream(Some(&branch_name)) {
            return GitResult {
                success: false,
                message: Some(format!("Push succeeded but failed to set upstream: {e}")),
            };
        }
    }

    GitResult {
        success: true,
        message: Some(format!("Pushed `{branch_name}` to origin")),
    }
}

fn collect_statuses(repo_path: &str, opts: &mut StatusOptions) -> Result<Vec<FileStatus>, String> {
    let repo = open_repository(repo_path).map_err(|e| format!("Failed to open repo: {}", e))?;

    let statuses = repo
        .statuses(Some(opts))
        .map_err(|e| format!("Failed to get statuses: {}", e))?;

    let result = statuses
        .iter()
        .filter_map(|entry| {
            let s = entry.status();
            let status = human_readable_status(s);

            if status.len() == 0 {
                return None;
            }

            // For renamed files, get both old and new paths
            let (path, new_path) = if s.is_index_renamed() {
                // Index rename: HEAD -> Index
                if let Some(diff) = entry.head_to_index() {
                    let old_path = diff.old_file().path().map(|p| p.to_string_lossy().into());
                    let new_path = diff.new_file().path().map(|p| p.to_string_lossy().into());
                    (old_path?, new_path)
                } else {
                    (entry.path()?.into(), None)
                }
            } else if s.is_wt_renamed() {
                // Working tree rename: Index -> Workdir
                if let Some(diff) = entry.index_to_workdir() {
                    let old_path = diff.old_file().path().map(|p| p.to_string_lossy().into());
                    let new_path = diff.new_file().path().map(|p| p.to_string_lossy().into());
                    (old_path?, new_path)
                } else {
                    (entry.path()?.into(), None)
                }
            } else {
                (entry.path()?.into(), None)
            };

            Some(FileStatus {
                path,
                new_path,
                status,
            })
        })
        .collect();

    Ok(result)
}

fn human_readable_status(status: Status) -> Vec<FileStatusKind> {
    let mut parts = Vec::new();
    if status.contains(Status::INDEX_NEW) {
        parts.push(FileStatusKind::IndexNew);
    }
    if status.contains(Status::INDEX_MODIFIED) {
        parts.push(FileStatusKind::IndexModified);
    }
    if status.contains(Status::INDEX_DELETED) {
        parts.push(FileStatusKind::IndexDeleted);
    }
    if status.contains(Status::INDEX_RENAMED) {
        parts.push(FileStatusKind::IndexRenamed);
    }
    if status.contains(Status::INDEX_TYPECHANGE) {
        parts.push(FileStatusKind::IndexTypechange);
    }
    if status.contains(Status::WT_NEW) {
        parts.push(FileStatusKind::WorktreeNew);
    }
    if status.contains(Status::WT_MODIFIED) {
        parts.push(FileStatusKind::WorktreeModified);
    }
    if status.contains(Status::WT_DELETED) {
        parts.push(FileStatusKind::WorktreeDeleted);
    }
    if status.contains(Status::WT_RENAMED) {
        parts.push(FileStatusKind::WorktreeRenamed);
    }
    if status.contains(Status::WT_TYPECHANGE) {
        parts.push(FileStatusKind::WorktreeTypechange);
    }
    if status.contains(Status::WT_UNREADABLE) {
        parts.push(FileStatusKind::WorktreeUnreadable);
    }
    if parts.is_empty() {
        // ! nothing
    }
    parts
}

fn default_status_options() -> StatusOptions {
    let mut opts = StatusOptions::new();
    opts.include_ignored(true)
        .include_unmodified(true)
        .include_unreadable(true)
        .include_unreadable_as_untracked(true)
        .include_untracked(true)
        .renames_index_to_workdir(true)
        .renames_head_to_index(true)
        .recurse_untracked_dirs(true);
    opts
}
