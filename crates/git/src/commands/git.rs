use std::process::Command;

use crate::{
    types::{FileStatus, FileStatusKind, GetStatusResponse, GitResult},
    utils::open_repository,
};
use git2::{BranchType, Cred, FetchOptions, FetchPrune, PushOptions, RemoteCallbacks};
use serde::Serialize;

#[derive(Serialize)]
pub struct CommitResult {
    success: bool,
    message: Option<String>,
}
/* #region // ! command */
// ? git status
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

// ? git add <file>
#[tauri::command]
#[logger::logger]
pub async fn git_add(repo_path: &str, file: &str) -> Result<GitResult, String> {
    let repo = match open_repository(repo_path) {
        Ok(r) => r,
        Err(e) => {
            return Ok(GitResult {
                success: false,
                message: Some(format!("Failed to open repo: {e}")),
            });
        }
    };

    let mut index = match repo.index() {
        Ok(i) => i,
        Err(e) => {
            return Ok(GitResult {
                success: false,
                message: Some(format!("Failed to open index: {e}")),
            });
        }
    };

    if file == "." {
        if let Err(e) = index.add_all(["*"].iter(), git2::IndexAddOption::DEFAULT, None) {
            return Ok(GitResult {
                success: false,
                message: Some(format!("Failed to add all: {e}")),
            });
        }
    } else {
        let file_path = std::path::Path::new(file);
        let full_path = std::path::Path::new(repo_path).join(file_path);

        if full_path.exists() {
            if let Err(e) = index.add_path(file_path) {
                return Ok(GitResult {
                    success: false,
                    message: Some(format!("Failed to add {file}: {e}")),
                });
            }
        } else {
            // ? File doesn't exist (deleted), remove it from the index
            if let Err(e) = index.remove_path(file_path) {
                return Ok(GitResult {
                    success: false,
                    message: Some(format!("Failed to stage deletion of {file}: {e}")),
                });
            }
        }
    }

    if let Err(e) = index.write() {
        return Ok(GitResult {
            success: false,
            message: Some(format!("Failed to write index: {e}")),
        });
    }

    Ok(GitResult {
        success: true,
        message: None,
    })
}

// ? git restore --staged <file>
#[tauri::command]
#[logger::logger]
pub async fn git_remove(repo_path: &str, file: &str) -> Result<GitResult, String> {
    let repo = match open_repository(repo_path) {
        Ok(r) => r,
        Err(e) => {
            return Ok(GitResult {
                success: false,
                message: Some(format!("Failed to open repo: {e}")),
            });
        }
    };

    let mut index = match repo.index() {
        Ok(i) => i,
        Err(e) => {
            return Ok(GitResult {
                success: false,
                message: Some(format!("Failed to open index: {e}")),
            });
        }
    };

    // Get HEAD tree
    let head = match repo.head() {
        Ok(h) => h,
        Err(e) => {
            return Ok(GitResult {
                success: false,
                message: Some(format!("Failed to get HEAD: {e}")),
            });
        }
    };

    let tree = match head.peel_to_tree() {
        Ok(t) => t,
        Err(e) => {
            return Ok(GitResult {
                success: false,
                message: Some(format!("Failed to get tree: {e}")),
            });
        }
    };

    if file == "." {
        // Unstage all files by resetting index to HEAD
        if let Err(e) = index.read_tree(&tree) {
            return Ok(GitResult {
                success: false,
                message: Some(format!("Failed to reset index: {e}")),
            });
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
                    return Ok(GitResult {
                        success: false,
                        message: Some(format!("Failed to reset {file} to HEAD: {e}")),
                    });
                }
            }
            Err(_) => {
                // File doesn't exist in HEAD (it's a new file), so remove it from index
                if let Err(e) = index.remove_path(std::path::Path::new(file)) {
                    return Ok(GitResult {
                        success: false,
                        message: Some(format!("Failed to unstage {file}: {e}")),
                    });
                }
            }
        }
    }

    if let Err(e) = index.write() {
        return Ok(GitResult {
            success: false,
            message: Some(format!("Failed to write index: {e}")),
        });
    }

    Ok(GitResult {
        success: true,
        message: None,
    })
}

// ? git restore <file>
#[tauri::command]
#[logger::logger]
pub async fn git_discard(repo_path: &str, file: &str) -> Result<GitResult, String> {
    let repo = match open_repository(repo_path) {
        Ok(r) => r,
        Err(e) => {
            return Ok(GitResult {
                success: false,
                message: Some(format!("Failed to open repo: {e}")),
            });
        }
    };

    let obj = match repo.head() {
        Ok(head) => match head.peel(git2::ObjectType::Commit) {
            Ok(obj) => obj,
            Err(e) => {
                return Ok(GitResult {
                    success: false,
                    message: Some(format!("Failed to peel HEAD: {e}")),
                });
            }
        },
        Err(_) => {
            return Ok(GitResult {
                success: false,
                message: Some("No HEAD to discard from".into()),
            });
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
        return Ok(GitResult {
            success: false,
            message: Some(format!("Failed to discard {file}: {e}")),
        });
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

    Ok(GitResult {
        success: true,
        message: None,
    })
}

// ? git fetch ...
#[tauri::command]
#[logger::logger]
pub async fn git_fetch(repo_path: &str) -> Result<GitResult, String> {
    let repo = match open_repository(repo_path) {
        Ok(r) => r,
        Err(e) => {
            return Ok(GitResult {
                success: false,
                message: Some(format!("Failed to open repo: {e}")),
            });
        }
    };

    // ? resolve remote (origin or fallback)
    let mut remote = match repo.find_remote("origin") {
        Ok(r) => r,
        Err(_) => {
            let remotes = match repo.remotes() {
                Ok(r) => r,
                Err(e) => {
                    return Ok(GitResult {
                        success: false,
                        message: Some(format!("Failed to list remotes: {e}")),
                    });
                }
            };

            let name = match remotes.get(0) {
                Some(n) => n,
                None => {
                    return Ok(GitResult {
                        success: false,
                        message: Some("No remotes configured".into()),
                    });
                }
            };

            match repo.find_remote(name) {
                Ok(r) => r,
                Err(e) => {
                    return Ok(GitResult {
                        success: false,
                        message: Some(format!("Failed to open remote `{name}`: {e}")),
                    });
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
        Ok(_) => {
            return Ok(GitResult {
                success: true,
                message: Some("Fetched successfully".into()),
            });
        }
        Err(e) => {
            return Ok(GitResult {
                success: false,
                message: Some(format!("Failed to fetch: {e}")),
            });
        }
    }
}

// ? git push ...
#[tauri::command]
#[logger::logger]
pub async fn git_push(repo_path: &str) -> Result<GitResult, String> {
    let repo = match open_repository(repo_path) {
        Ok(r) => r,
        Err(e) => {
            return Ok(GitResult {
                success: false,
                message: Some(format!("Failed to open repository: {e}")),
            });
        }
    };

    // ? HEAD validation
    let head = match repo.head() {
        Ok(h) => h,
        Err(e) => {
            return Ok(GitResult {
                success: false,
                message: Some(format!("Failed to read HEAD: {e}")),
            });
        }
    };

    if !head.is_branch() {
        return Ok(GitResult {
            success: false,
            message: Some("HEAD is detached, cannot push".into()),
        });
    }

    let branch_name = match head.shorthand() {
        Some(b) => b.to_string(),
        None => {
            return Ok(GitResult {
                success: false,
                message: Some("Invalid branch name".into()),
            });
        }
    };

    // ? resolve local branch
    // * usually the current checkout branch
    // * if it;s detached
    let mut branch = match repo.find_branch(&branch_name, BranchType::Local) {
        Ok(b) => b,
        Err(e) => {
            return Ok(GitResult {
                success: false,
                message: Some(format!("Failed to resolve local branch: {e}")),
            });
        }
    };

    // ? auth callbacks
    let mut callbacks = RemoteCallbacks::new();
    callbacks.credentials(|_url, username_from_url, allowed| {
        // TODO(ruru): will support more auth option, rn it's via ssh only
        // println!("pushing to: {}", url);

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
            return Ok(GitResult {
                success: false,
                message: Some(format!("Failed to find remote 'origin': {e}")),
            });
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
        return Ok(GitResult {
            success: false,
            message: Some(format!("Push failed: {e}")),
        });
    }

    // ? set upstream if missing
    if !has_upstream {
        if let Err(e) = branch.set_upstream(Some(&branch_name)) {
            return Ok(GitResult {
                success: false,
                message: Some(format!("Push succeeded but failed to set upstream: {e}")),
            });
        }
    }

    Ok(GitResult {
        success: true,
        message: Some(format!("Pushed `{branch_name}` to origin")),
    })
}

// ? git pull ...
#[tauri::command]
#[logger::logger]
pub async fn git_pull(repo_path: &str) -> Result<GitResult, String> {
    let repo = match open_repository(repo_path) {
        Ok(r) => r,
        Err(e) => {
            return Ok(GitResult {
                success: false,
                message: Some(format!("Failed to open repository: {e}")),
            });
        }
    };

    // ! HEAD must be a branch
    let head = match repo.head() {
        Ok(h) if h.is_branch() => h,
        _ => {
            return Ok(GitResult {
                success: false,
                message: Some("HEAD is detached, cannot pull".into()),
            });
        }
    };

    let branch_name = match head.shorthand() {
        Some(b) => b.to_string(),
        None => {
            return Ok(GitResult {
                success: false,
                message: Some("Invalid branch name".into()),
            });
        }
    };

    let branch = match repo.find_branch(&branch_name, BranchType::Local) {
        Ok(b) => b,
        Err(e) => {
            return Ok(GitResult {
                success: false,
                message: Some(format!("Failed to resolve local branch: {e}")),
            });
        }
    };

    // ? resolve upstream (required for pull)
    let upstream = match branch.upstream() {
        Ok(u) => u,
        Err(_) => {
            return Ok(GitResult {
                success: false,
                message: Some("No upstream configured for this branch".into()),
            });
        }
    };

    // ? fetch
    let mut remote = match repo.find_remote("origin") {
        Ok(r) => r,
        Err(e) => {
            return Ok(GitResult {
                success: false,
                message: Some(format!("Failed to find remote 'origin': {e}")),
            });
        }
    };

    let mut callbacks = RemoteCallbacks::new();
    callbacks.credentials(|_, username_from_url, allowed| {
        if allowed.is_ssh_key() {
            Cred::ssh_key_from_agent(username_from_url.unwrap_or("git"))
        } else {
            Err(git2::Error::from_str("Unsupported auth method"))
        }
    });

    let mut fo = FetchOptions::new();
    fo.remote_callbacks(callbacks);
    fo.prune(FetchPrune::On);

    if let Err(e) = remote.fetch(&[] as &[&str], Some(&mut fo), None) {
        return Ok(GitResult {
            success: false,
            message: Some(format!("Fetch failed: {e}")),
        });
    }

    // ? merge analysis
    let upstream_ref = upstream.into_reference();

    let upstream_commit = match repo.reference_to_annotated_commit(&upstream_ref) {
        Ok(c) => c,
        Err(e) => {
            return Ok(GitResult {
                success: false,
                message: Some(format!("Failed to resolve upstream commit: {e}")),
            });
        }
    };

    let (analysis, _) = match repo.merge_analysis(&[&upstream_commit]) {
        Ok(a) => a,
        Err(e) => {
            return Ok(GitResult {
                success: false,
                message: Some(format!("Merge analysis failed: {e}")),
            });
        }
    };

    // ? already up to date
    if analysis.is_up_to_date() {
        return Ok(GitResult {
            success: true,
            message: Some("Already up to date".into()),
        });
    }

    // ? fast-forward
    if analysis.is_fast_forward() {
        let refname = match branch.get().name() {
            Some(r) => r.to_string(),
            None => {
                return Ok(GitResult {
                    success: false,
                    message: Some("Invalid branch reference".into()),
                });
            }
        };

        let mut reference = match repo.find_reference(&refname) {
            Ok(r) => r,
            Err(e) => {
                return Ok(GitResult {
                    success: false,
                    message: Some(format!("Failed to find branch ref: {e}")),
                });
            }
        };

        if let Err(e) = reference.set_target(upstream_commit.id(), "Fast-forward") {
            return Ok(GitResult {
                success: false,
                message: Some(format!("Fast-forward failed: {e}")),
            });
        }

        repo.set_head(&refname).ok();
        repo.checkout_head(None).ok();

        return Ok(GitResult {
            success: true,
            message: Some(format!("Fast-forwarded `{branch_name}`")),
        });
    }

    // ? normal merge (non-ff)
    if analysis.is_normal() {
        repo.merge(&[&upstream_commit], None, None).ok();

        let mut index = match repo.index() {
            Ok(i) => i,
            Err(e) => {
                return Ok(GitResult {
                    success: false,
                    message: Some(format!("Failed to read index: {e}")),
                });
            }
        };

        if index.has_conflicts() {
            return Ok(GitResult {
                success: false,
                message: Some("Merge conflicts detected".into()),
            });
        }

        let tree_oid = index.write_tree().unwrap();
        let tree = repo.find_tree(tree_oid).unwrap();

        let head_commit = repo.find_commit(head.target().unwrap()).unwrap();
        let upstream_commit = repo.find_commit(upstream_commit.id()).unwrap();

        let sig = repo.signature().unwrap();

        repo.commit(
            Some("HEAD"),
            &sig,
            &sig,
            "Merge remote changes",
            &tree,
            &[&head_commit, &upstream_commit],
        )
        .unwrap();

        repo.checkout_head(None).ok();

        return Ok(GitResult {
            success: true,
            message: Some(format!("Merged into `{branch_name}`")),
        });
    }

    Ok(GitResult {
        success: false,
        message: Some("Unsupported merge state".into()),
    })
}

/* #endregion // ! command */

/* #region // ? helpers */
fn collect_status(repo_path: &str) -> Result<Vec<FileStatus>, String> {
    let out = Command::new("git")
        .current_dir(repo_path)
        .args(["status", "--porcelain=v2", "--untracked-files=all", "-z"])
        .output()
        .map_err(|e| e.to_string())?;

    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).to_string());
    }

    parse_porcelain_v2(&out.stdout)
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

    for entry in buf.split(|b| *b == 0) {
        if entry.is_empty() {
            continue;
        }

        let line = std::str::from_utf8(entry).map_err(|e| e.to_string())?;
        let mut chars = line.chars();

        match chars.next() {
            Some('1') => {
                // 1 <XY> <sub> <mH> <mI> <mW> <hH> <hI> <path>
                let mut parts = line.split_whitespace();
                parts.next(); // "1"

                let xy = parts.next().ok_or("missing XY")?;
                let path = parts.last().ok_or("missing path")?.to_string();

                let mut status = Vec::new();
                let x = xy.as_bytes()[0];
                let y = xy.as_bytes()[1];

                push_xy_status(&mut status, x, y);

                result.push(FileStatus {
                    path,
                    new_path: None,
                    status,
                });
            }

            Some('2') => {
                // 2 <XY> <sub> <mH> <mI> <mW> <hH> <hI> <score> <old> <new>
                let mut parts = line.split_whitespace();
                parts.next(); // "2"

                let xy = parts.next().ok_or("missing XY")?;
                let x = xy.as_bytes()[0];
                let y = xy.as_bytes()[1];

                // skip to <old>
                let old_path = parts.nth(7).ok_or("missing old path")?.to_string();
                let new_path = parts.next().ok_or("missing new path")?.to_string();

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

/* #endregion // ? helpers */
