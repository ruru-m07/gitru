use std::process::Command;

use crate::{
    types_legacy::{FileStatus, FileStatusKind, GetStatusResponse, UncommittedChangesStrategy},
    utils::open_repository,
};
use git2::{Status, StatusOptions};
use serde::Serialize;

#[derive(Serialize)]
pub struct CommitResult {
    success: bool,
    message: Option<String>,
}

/* #region // ! command */
#[tauri::command]
#[logger::logger]
pub async fn get_status(repo_path: &str) -> Result<GetStatusResponse, String> {
    let files = collect_status(repo_path)?;
    Ok(GetStatusResponse { files })
}

#[tauri::command]
#[logger::logger]
pub async fn get_file_status(
    repo_path: &str,
    file_path: &str,
) -> Result<Option<FileStatus>, String> {
    collect_single_file_status(repo_path, file_path)
}

#[tauri::command]
#[logger::logger]
pub async fn git_add(repo_path: &str, file: &str) -> Result<String, String> {
    let repo = match open_repository(repo_path) {
        Ok(r) => r,
        Err(e) => {
            return Err(format!("Failed to open repo: {e}"));
        }
    };

    let mut index = match repo.index() {
        Ok(i) => i,
        Err(e) => {
            return Err(format!("Failed to open index: {e}"));
        }
    };

    if file == "." {
        if let Err(e) = index.add_all(["*"].iter(), git2::IndexAddOption::DEFAULT, None) {
            return Err(format!("Failed to add all: {e}"));
        }
    } else {
        let file_path = std::path::Path::new(file);
        let full_path = std::path::Path::new(repo_path).join(file_path);

        if full_path.exists() {
            if let Err(e) = index.add_path(file_path) {
                return Err(format!("Failed to add {file}: {e}"));
            }
        } else {
            // ? File doesn't exist (deleted), remove it from the index
            if let Err(e) = index.remove_path(file_path) {
                return Err(format!("Failed to stage deletion of {file}: {e}"));
            }
        }
    }

    if let Err(e) = index.write() {
        return Err(format!("Failed to write index: {e}"));
    }

    Ok(format!("Added"))
}

#[tauri::command]
#[logger::logger]
pub async fn git_remove(repo_path: &str, file: &str) -> Result<String, String> {
    let repo = match open_repository(repo_path) {
        Ok(r) => r,
        Err(e) => {
            return Err(format!("Failed to open repo: {e}"));
        }
    };

    let mut index = match repo.index() {
        Ok(i) => i,
        Err(e) => {
            return Err(format!("Failed to open index: {e}"));
        }
    };

    // Get HEAD tree
    let head = match repo.head() {
        Ok(h) => h,
        Err(e) => {
            return Err(format!("Failed to get HEAD: {e}"));
        }
    };

    let tree = match head.peel_to_tree() {
        Ok(t) => t,
        Err(e) => {
            return Err(format!("Failed to get tree: {e}"));
        }
    };

    if file == "." {
        // Unstage all files by resetting index to HEAD
        if let Err(e) = index.read_tree(&tree) {
            return Err(format!("Failed to reset index: {e}"));
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
                    return Err(format!("Failed to reset {file} to HEAD: {e}"));
                }
            }
            Err(_) => {
                // File doesn't exist in HEAD (it's a new file), so remove it from index
                if let Err(e) = index.remove_path(std::path::Path::new(file)) {
                    return Err(format!("Failed to unstage {file}: {e}"));
                }
            }
        }
    }

    if let Err(e) = index.write() {
        return Err(format!("Failed to write index: {e}"));
    }

    Ok(format!("Removed"))
}

#[tauri::command]
#[logger::logger]
pub async fn git_discard(repo_path: &str, file: &str, all: Option<bool>) -> Result<String, String> {
    if all.unwrap_or(false) {
        git_restore_all(repo_path)?;
        return Ok(format!("All changes discarded"));
    }

    git_restore_file(repo_path, file)?;
    Ok(format!("Changes discarded"))
}

#[tauri::command]
#[logger::logger]
pub async fn git_fetch(repo_path: &str) -> Result<String, String> {
    match git(repo_path, &["fetch", "--prune"]) {
        Ok(_) => Ok(format!("Fetched successfully")),
        Err(e) => Err(e),
    }
}

#[tauri::command]
#[logger::logger]
pub async fn git_push(repo_path: &str) -> Result<String, String> {
    git(repo_path, &["push"])?;
    Ok(format!("Pushed successfully"))
}

#[tauri::command]
#[logger::logger]
pub async fn git_publish_branch(repo_path: &str) -> Result<String, String> {
    let branch = git(repo_path, &["branch", "--show-current"])?;

    git(repo_path, &["push", "-u", "origin", branch.as_str()])?;

    Ok(format!("Published `{}` to origin", branch))
}

#[tauri::command]
#[logger::logger]
pub async fn git_pull(repo_path: &str) -> Result<String, String> {
    match git(repo_path, &["pull"]) {
        Ok(_) => Ok(format!("Pulled successfully")),
        Err(e) => Err(e),
    }
}

#[tauri::command]
#[logger::logger]
pub async fn git_switch_branch(
    repo_path: &str,
    branch: &str,
    strategy: Option<UncommittedChangesStrategy>,
) -> Result<String, String> {
    let repo = open_repository(repo_path).map_err(|e| e.to_string())?;
    let head = repo
        .head()
        .map_err(|e| format!("Failed to get HEAD: {e}"))?;

    if !head.is_branch() {
        return Err("You aren't on a valid HEAD".to_string());
    }

    let current_branch = head
        .shorthand()
        .ok_or_else(|| "Failed to read branch name".to_string())?;

    // Decide how to switch:
    // - local branch  -> git switch <branch>
    // - remote branch -> git switch --track <branch>
    let switch_args: Vec<&str> = if branch.starts_with("origin/") {
        vec!["switch", "--track", branch]
    } else {
        vec!["switch", branch]
    };

    let do_switch = || git(repo_path, &switch_args);

    match strategy {
        Some(UncommittedChangesStrategy::StashOnCurrentBranch) => {
            let stash_msg = format!("!!Gitru<{}> -> <{}>", current_branch, branch);

            git(repo_path, &["stash", "push", "-u", "-m", &stash_msg])
                .map_err(|e| format!("Failed to stash changes: {e}"))?;

            match do_switch() {
                Ok(_) => Ok(format!(
                    "Switched to {} (changes stashed from {})",
                    branch, current_branch
                )),
                Err(err) => {
                    let _ = git(repo_path, &["stash", "pop"]);
                    Err(format!(
                        "Failed to switch to {} even after stashing: {}",
                        branch, err
                    ))
                }
            }
        }

        _ => match do_switch() {
            Ok(_) => Ok(format!("Switched to {}", branch)),
            Err(err) => match strategy {
                Some(UncommittedChangesStrategy::BringChanges) => Err(format!(
                    "Cannot bring uncommitted changes to {}: conflicts detected",
                    branch
                )),
                None => Err(format!("Cannot switch to {}: {}", branch, err)),
                _ => unreachable!(),
            },
        },
    }
}

#[tauri::command]
#[logger::logger]
pub async fn git_create_branch(
    repo_path: &str,
    branch: &str,
    strategy: Option<UncommittedChangesStrategy>,
) -> Result<String, String> {
    let repo = open_repository(repo_path).map_err(|e| e.to_string())?;
    let head = repo
        .head()
        .map_err(|e| format!("Failed to get HEAD: {e}"))?;

    if !head.is_branch() {
        return Err(format!("You aren't on valid HEAD"));
    }

    let current_branch = head
        .shorthand()
        .ok_or_else(|| "Failed to read branch name".to_string())?;

    match strategy {
        // ? If strategy is StashOnCurrentBranch, stash FIRST before creating and switching
        Some(UncommittedChangesStrategy::StashOnCurrentBranch) => {
            let stash_msg = format!("!!Gitru<{}> -> <{}> (new)", current_branch, branch);
            git(repo_path, &["stash", "push", "-u", "-m", &stash_msg])
                .map_err(|e| format!("Failed to stash changes: {}", e))?;

            match git(repo_path, &["switch", "-c", branch]) {
                Ok(_) => Ok(format!(
                    "Created and switched to {} (changes stashed in {})",
                    branch, current_branch
                )),
                Err(err) => {
                    let _ = git(repo_path, &["stash", "pop"]);
                    Err(format!(
                        "Failed to create branch {} even after stashing: {}",
                        branch, err
                    ))
                }
            }
        }

        // ? If strategy is BringChanges or None, try to create and switch directly
        _ => match git(repo_path, &["switch", "-c", branch]) {
            Ok(_) => Ok(format!("Created and switched to {}", branch)),
            Err(err) => match strategy {
                Some(UncommittedChangesStrategy::BringChanges) => Err(format!(
                    "Cannot bring uncommitted changes to new branch {}: {}",
                    branch, err
                )),
                None => Err(format!("Cannot create branch {}: {}", branch, err)),
                _ => unreachable!(),
            },
        },
    }
}

#[tauri::command]
#[logger::logger]
pub async fn has_uncommitted_changes(repo_path: &str) -> Result<bool, String> {
    let repo = open_repository(repo_path).map_err(|e| format!("Failed to open repo: {e}"))?;
    has_uncommitted_changes_internal(&repo)
}

#[tauri::command]
#[logger::logger]
pub async fn git_version() -> Result<String, String> {
    let output = Command::new("git")
        .args(["--version"])
        .output()
        .map_err(|e| e.to_string())?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).trim().to_string())
    }
}

/* #endregion // ! command */

/* #region // ? helpers */

fn git(repo_path: &str, args: &[&str]) -> Result<String, String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(repo_path)
        .output()
        .map_err(|e| e.to_string())?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).trim().to_string())
    }
}

fn has_uncommitted_changes_internal(repo: &git2::Repository) -> Result<bool, String> {
    let mut opts = StatusOptions::new();
    opts.include_untracked(true)
        .include_ignored(false)
        .exclude_submodules(true);

    let statuses = repo
        .statuses(Some(&mut opts))
        .map_err(|e| format!("Failed to get status: {e}"))?;

    Ok(!statuses.is_empty())
}

fn collect_single_file_status(
    repo_path: &str,
    file_path: &str,
) -> Result<Option<FileStatus>, String> {
    let out = Command::new("git")
        .current_dir(repo_path)
        .args([
            "status",
            "--porcelain=v2",
            "--untracked-files=all",
            "-z",
            "--",
            file_path,
        ])
        .output()
        .map_err(|e| e.to_string())?;

    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).to_string());
    }

    if out.stdout.is_empty() {
        return Ok(None);
    }

    let mut parsed = parse_porcelain_v2(&out.stdout)?;

    Ok(parsed.pop())
}

fn parse_porcelain_v2(buf: &[u8]) -> Result<Vec<FileStatus>, String> {
    let mut result = Vec::new();
    let mut iter = buf.split(|b| *b == 0).peekable();

    while let Some(entry) = iter.next() {
        if entry.is_empty() {
            continue;
        }

        let line = std::str::from_utf8(entry).map_err(|e| e.to_string())?;
        let mut chars = line.chars();

        match chars.next() {
            Some('1') => {
                // 1 <XY> <sub> <mH> <mI> <mW> <hH> <hI> <path>
                let parts: Vec<&str> = line.splitn(9, ' ').collect();

                if parts.len() < 9 {
                    return Err("invalid type 1 entry".to_string());
                }

                let xy = parts[1];
                let x = xy.as_bytes()[0];
                let y = xy.as_bytes()[1];
                let path = parts[8].to_string();

                let mut status = Vec::new();
                push_xy_status(&mut status, x, y);

                result.push(FileStatus {
                    path,
                    new_path: None,
                    status,
                });
            }

            Some('2') => {
                // 2 <XY> <sub> <mH> <mI> <mW> <hH> <hI> <X><score> <path>\0<origPath>\0
                let mut parts = line.split_whitespace();
                parts.next(); // "2"

                let xy = parts.next().ok_or("missing XY")?;
                let x = xy.as_bytes()[0];
                let y = xy.as_bytes()[1];

                // Paths come from iterator for type 2
                let new_path = iter.next().ok_or("missing new path")?;
                let old_path = iter.next().ok_or("missing old path")?;

                let new_path = std::str::from_utf8(new_path)
                    .map_err(|e| e.to_string())?
                    .to_string();
                let old_path = std::str::from_utf8(old_path)
                    .map_err(|e| e.to_string())?
                    .to_string();

                let mut status = Vec::new();
                push_xy_status(&mut status, x, y);

                result.push(FileStatus {
                    path: old_path,
                    new_path: Some(new_path),
                    status,
                });
            }

            Some('?') => {
                // ? <path>  (untracked)
                let path = line[2..].to_string();
                result.push(FileStatus {
                    path,
                    new_path: None,
                    status: vec![FileStatusKind::WorktreeNew],
                });
            }

            Some('!') => {
                // ! <path> (ignored) — usually safe to skip, but included if needed
                let path = line[2..].to_string();
                result.push(FileStatus {
                    path,
                    new_path: None,
                    status: Vec::new(),
                });
            }

            _ => {}
        }
    }

    Ok(result)
}

fn push_xy_status(status: &mut Vec<FileStatusKind>, x: u8, y: u8) {
    match x {
        b'A' => status.push(FileStatusKind::IndexNew),
        b'M' => status.push(FileStatusKind::IndexModified),
        b'D' => status.push(FileStatusKind::IndexDeleted),
        b'R' => status.push(FileStatusKind::IndexRenamed),
        b'T' => status.push(FileStatusKind::IndexTypechange),
        _ => {}
    }

    match y {
        b'A' => status.push(FileStatusKind::WorktreeNew),
        b'M' => status.push(FileStatusKind::WorktreeModified),
        b'D' => status.push(FileStatusKind::WorktreeDeleted),
        b'R' => status.push(FileStatusKind::WorktreeRenamed),
        b'T' => status.push(FileStatusKind::WorktreeTypechange),
        b'X' => status.push(FileStatusKind::WorktreeUnreadable),
        _ => {}
    }
}

fn collect_status(repo_path: &str) -> Result<Vec<FileStatus>, String> {
    let repo =
        open_repository(repo_path).map_err(|e| format!("Failed to open repository: {}", e))?;

    let mut opts = StatusOptions::new();
    opts.include_untracked(true)
        .recurse_untracked_dirs(true)
        .include_ignored(false)
        .exclude_submodules(true)
        .renames_head_to_index(true)
        .renames_index_to_workdir(true);

    let statuses = repo
        .statuses(Some(&mut opts))
        .map_err(|e| format!("Failed to get status: {}", e))?;

    let mut files = Vec::with_capacity(statuses.len());

    for entry in statuses.iter() {
        let status = entry.status();
        let path = entry
            .path()
            .ok_or_else(|| "Invalid UTF-8 in path".to_string())?
            .to_string();

        let mut status_kinds = Vec::with_capacity(2);

        if status.contains(Status::CONFLICTED) {
            status_kinds.push(FileStatusKind::Conflicted);
        }

        if status.contains(Status::INDEX_NEW) {
            status_kinds.push(FileStatusKind::IndexNew);
        }
        if status.contains(Status::INDEX_MODIFIED) {
            status_kinds.push(FileStatusKind::IndexModified);
        }
        if status.contains(Status::INDEX_DELETED) {
            status_kinds.push(FileStatusKind::IndexDeleted);
        }
        if status.contains(Status::INDEX_RENAMED) {
            status_kinds.push(FileStatusKind::IndexRenamed);
        }
        if status.contains(Status::INDEX_TYPECHANGE) {
            status_kinds.push(FileStatusKind::IndexTypechange);
        }

        if status.contains(Status::WT_NEW) {
            status_kinds.push(FileStatusKind::WorktreeNew);
        }
        if status.contains(Status::WT_MODIFIED) {
            status_kinds.push(FileStatusKind::WorktreeModified);
        }
        if status.contains(Status::WT_DELETED) {
            status_kinds.push(FileStatusKind::WorktreeDeleted);
        }
        if status.contains(Status::WT_RENAMED) {
            status_kinds.push(FileStatusKind::WorktreeRenamed);
        }
        if status.contains(Status::WT_TYPECHANGE) {
            status_kinds.push(FileStatusKind::WorktreeTypechange);
        }

        let new_path =
            if status.contains(Status::INDEX_RENAMED) || status.contains(Status::WT_RENAMED) {
                entry
                    .head_to_index()
                    .and_then(|diff| diff.new_file().path())
                    .or_else(|| {
                        entry
                            .index_to_workdir()
                            .and_then(|diff| diff.new_file().path())
                    })
                    .map(|p| p.to_string_lossy().to_string())
            } else {
                None
            };

        files.push(FileStatus {
            path,
            new_path,
            status: status_kinds,
        });
    }

    Ok(files)
}

fn git_restore_all(repo_path: &str) -> Result<(), String> {
    let restore = Command::new("git")
        .current_dir(repo_path)
        .args(["restore", "--source=HEAD", "--staged", "--worktree", "."])
        .output()
        .map_err(|e| e.to_string())?;

    if !restore.status.success() {
        return Err(String::from_utf8_lossy(&restore.stderr).to_string());
    }

    let clean = Command::new("git")
        .current_dir(repo_path)
        .args(["clean", "-fd"])
        .output()
        .map_err(|e| e.to_string())?;

    if !clean.status.success() {
        return Err(String::from_utf8_lossy(&clean.stderr).to_string());
    }

    Ok(())
}

fn git_restore_file(repo_path: &str, file: &str) -> Result<(), String> {
    let out = Command::new("git")
        .current_dir(repo_path)
        .args([
            "restore",
            "--source=HEAD",
            "--staged",
            "--worktree",
            "--",
            file,
        ])
        .output()
        .map_err(|e| e.to_string())?;

    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).to_string());
    }

    let _ = Command::new("git")
        .current_dir(repo_path)
        .args(["clean", "-f", "--", file])
        .output();

    Ok(())
}

/* #endregion // ? helpers */
