use std::process::Command;

use crate::{
    types::{
        FileStatus, FileStatusKind, GetStatusResponse, GitResult, SwitchBranchResult,
        UncommittedChangesStrategy,
    },
    utils::open_repository,
};
use git2::{
    BranchType, Cred, FetchOptions, FetchPrune, PushOptions, RemoteCallbacks, Signature,
    StashFlags, Status, StatusOptions,
};
use serde::Serialize;
use std::sync::{Arc, Mutex};

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
pub async fn git_push(repo_path: &str) -> Result<String, String> {
    let repo = match open_repository(repo_path) {
        Ok(r) => r,
        Err(e) => {
            return Err(format!("Failed to open repository: {e}"));
        }
    };

    let head = match repo.head() {
        Ok(h) => h,
        Err(e) => {
            return Err(format!("Failed to read HEAD: {e}"));
        }
    };

    if !head.is_branch() {
        return Err("HEAD is detached, cannot push".into());
    }

    let branch_name = match head.shorthand() {
        Some(b) => b.to_string(),
        None => {
            return Err("Invalid branch name".into());
        }
    };

    let mut branch = match repo.find_branch(&branch_name, BranchType::Local) {
        Ok(b) => b,
        Err(e) => {
            return Err(format!("Failed to resolve local branch: {e}"));
        }
    };

    let attempt_count = Arc::new(Mutex::new(0));
    let attempt_count_clone = Arc::clone(&attempt_count);

    let mut callbacks = RemoteCallbacks::new();

    callbacks.credentials(move |url, username_from_url, allowed| {
        let mut count = attempt_count_clone.lock().unwrap();
        *count += 1;

        println!("Credentials attempt #{} for URL: {}", *count, url);
        println!("Allowed types: {:?}", allowed);

        // Prevent infinite retry loop
        if *count > 3 {
            println!("Too many authentication attempts, giving up");
            return Err(git2::Error::from_str(
                "Authentication failed after 3 attempts. Please check:\n\
                1. SSH agent is running (eval `ssh-agent`)\n\
                2. Key is added (ssh-add ~/.ssh/id_rsa)\n\
                3. Key has correct permissions (chmod 600 ~/.ssh/id_rsa)",
            ));
        }

        if allowed.is_ssh_key() {
            let user = username_from_url.unwrap_or("git");
            println!("Attempting SSH key auth for user: {}", user);

            // Try SSH agent first
            match Cred::ssh_key_from_agent(user) {
                Ok(cred) => {
                    println!("SSH agent authentication successful");
                    return Ok(cred);
                }
                Err(e) => {
                    println!("SSH agent failed: {:?}", e);

                    // Fallback: try default SSH key locations
                    let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
                    let possible_keys = vec![
                        format!("{}/.ssh/id_rsa", home),
                        format!("{}/.ssh/id_ed25519", home),
                        format!("{}/.ssh/id_ecdsa", home),
                    ];

                    for key_path in possible_keys {
                        println!("Trying key: {}", key_path);
                        if std::path::Path::new(&key_path).exists() {
                            match Cred::ssh_key(
                                user,
                                None, // public key (optional)
                                std::path::Path::new(&key_path),
                                None, // passphrase
                            ) {
                                Ok(cred) => {
                                    println!("SSH key authentication successful with {}", key_path);
                                    return Ok(cred);
                                }
                                Err(key_err) => {
                                    println!("Failed to use {}: {:?}", key_path, key_err);
                                }
                            }
                        }
                    }

                    return Err(git2::Error::from_str(&format!(
                        "SSH authentication failed: {}\n\
                        Make sure:\n\
                        1. SSH agent is running: eval `ssh-agent`\n\
                        2. Add your key: ssh-add ~/.ssh/id_rsa\n\
                        3. Or ensure key exists at ~/.ssh/id_rsa",
                        e
                    )));
                }
            }
        }

        Err(git2::Error::from_str("No supported authentication method"))
    });

    callbacks.transfer_progress(|progress| {
        println!(
            "Transfer progress: {}/{} objects, {} bytes",
            progress.received_objects(),
            progress.total_objects(),
            progress.received_bytes()
        );
        true
    });

    callbacks.push_transfer_progress(|current, total, bytes| {
        println!("Push progress: {}/{} ({} bytes)", current, total, bytes);
    });

    callbacks.push_update_reference(|refname, status| {
        println!("Push update for {}: {:?}", refname, status);
        if let Some(s) = status {
            println!("Push rejected: {}", s);
            return Err(git2::Error::from_str(s));
        }
        Ok(())
    });

    let mut push_opts = PushOptions::new();
    push_opts.remote_callbacks(callbacks);

    let mut remote = match repo.find_remote("origin") {
        Ok(r) => r,
        Err(e) => {
            return Err(format!("Failed to find remote 'origin': {e}"));
        }
    };

    println!("Remote URL: {:?}", remote.url());

    let has_upstream = branch.upstream().is_ok();
    println!("has_upstream: {}", has_upstream);

    let remote_branch_exists = repo
        .find_branch(&format!("origin/{}", branch_name), BranchType::Remote)
        .is_ok();
    println!("remote_branch_exists: {}", remote_branch_exists);

    let refspec = format!("refs/heads/{}:refs/heads/{}", branch_name, branch_name);

    println!("refspec: {}", refspec);
    println!("Starting push...");

    if let Err(e) = remote.push(&[refspec.as_str()], Some(&mut push_opts)) {
        println!("Push error: {:?}", e);
        return Err(format!("Push failed: {}", e));
    }

    println!("Push completed successfully");

    if !has_upstream {
        println!("Setting upstream to origin/{}", branch_name);
        if let Err(e) = branch.set_upstream(Some(&format!("origin/{}", branch_name))) {
            println!("Warning: Failed to set upstream: {}", e);
            return Err(format!(
                "Pushed `{}` to origin (warning: upstream not set)",
                branch_name
            ));
        }
    }

    let action = if remote_branch_exists {
        "Pushed"
    } else {
        "Published"
    };

    Ok(format!("{} `{}` to origin", action, branch_name))
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

#[tauri::command]
#[logger::logger]
pub async fn git_switch_branch(
    repo_path: &str,
    branch_name: &str,
    strategy: UncommittedChangesStrategy,
) -> Result<SwitchBranchResult, String> {
    let repo = match open_repository(repo_path) {
        Ok(r) => r,
        Err(e) => {
            return Ok(SwitchBranchResult {
                success: false,
                message: Some(format!("Failed to open repo: {e}")),
                stash_name: None,
                from_branch: None,
                to_branch: None,
            });
        }
    };

    // Get current branch name
    let current_branch = match repo.head() {
        Ok(h) => h.shorthand().map(|s| s.to_string()),
        Err(_) => None,
    };

    // Check if we're already on the target branch
    if let Some(ref current) = current_branch {
        if current == branch_name {
            return Ok(SwitchBranchResult {
                success: true,
                message: Some(format!("Already on branch `{}`", branch_name)),
                stash_name: None,
                from_branch: current_branch.clone(),
                to_branch: Some(branch_name.to_string()),
            });
        }
    }

    // Check for uncommitted changes
    let has_changes = has_uncommitted_changes_internal(&repo)?;
    let mut stash_name: Option<String> = None;

    // Handle uncommitted changes based on strategy
    if has_changes {
        match strategy {
            UncommittedChangesStrategy::StashOnCurrentBranch => {
                // Create stash with descriptive message
                let stash_message = format!(
                    "Auto-stash on `{}` before switching to `{}`",
                    current_branch.as_deref().unwrap_or("detached"),
                    branch_name
                );

                let sig = match repo.signature() {
                    Ok(s) => s,
                    Err(_) => Signature::now("gitru", "gitru@local").unwrap(),
                };

                let mut repo_mut = repo;
                match repo_mut.stash_save(&sig, &stash_message, Some(StashFlags::INCLUDE_UNTRACKED))
                {
                    Ok(_oid) => {
                        stash_name = Some(stash_message.clone());
                    }
                    Err(e) => {
                        return Ok(SwitchBranchResult {
                            success: false,
                            message: Some(format!("Failed to stash changes: {e}")),
                            stash_name: None,
                            from_branch: current_branch,
                            to_branch: None,
                        });
                    }
                }

                // Now perform the checkout
                let result = perform_checkout(&mut repo_mut, branch_name);

                if !result.success {
                    // Checkout failed, try to recover the stash
                    if let Err(pop_err) = repo_mut.stash_pop(0, None) {
                        return Ok(SwitchBranchResult {
                            success: false,
                            message: Some(format!(
                                "Checkout failed: {}. Also failed to recover stash: {}",
                                result.message.unwrap_or_default(),
                                pop_err
                            )),
                            stash_name: None,
                            from_branch: current_branch,
                            to_branch: None,
                        });
                    }
                    return Ok(SwitchBranchResult {
                        success: false,
                        message: result.message,
                        stash_name: None,
                        from_branch: current_branch,
                        to_branch: None,
                    });
                }

                return Ok(SwitchBranchResult {
                    success: true,
                    message: Some(format!(
                        "Switched to `{}`. Changes stashed on `{}`",
                        branch_name,
                        current_branch.as_deref().unwrap_or("previous branch")
                    )),
                    stash_name,
                    from_branch: current_branch,
                    to_branch: Some(branch_name.to_string()),
                });
            }
            UncommittedChangesStrategy::BringChanges => {
                // Try checkout directly - git will bring changes if possible
                let result = perform_checkout(&repo, branch_name);
                return Ok(SwitchBranchResult {
                    success: result.success,
                    message: if result.success {
                        Some(format!(
                            "Switched to `{}` with uncommitted changes",
                            branch_name
                        ))
                    } else {
                        result.message
                    },
                    stash_name: None,
                    from_branch: current_branch,
                    to_branch: if result.success {
                        Some(branch_name.to_string())
                    } else {
                        None
                    },
                });
            }
        }
    }

    // No uncommitted changes, just checkout
    let result = perform_checkout(&repo, branch_name);
    Ok(SwitchBranchResult {
        success: result.success,
        message: if result.success {
            Some(format!("Switched to `{}`", branch_name))
        } else {
            result.message
        },
        stash_name: None,
        from_branch: current_branch,
        to_branch: if result.success {
            Some(branch_name.to_string())
        } else {
            None
        },
    })
}

// ? git checkout -b <branch_name>
#[tauri::command]
#[logger::logger]
pub async fn git_create_branch(
    repo_path: &str,
    branch_name: &str,
    strategy: UncommittedChangesStrategy,
) -> Result<SwitchBranchResult, String> {
    let mut repo = match open_repository(repo_path) {
        Ok(r) => r,
        Err(e) => {
            return Ok(SwitchBranchResult {
                success: false,
                message: Some(format!("Failed to open repo: {e}")),
                stash_name: None,
                from_branch: None,
                to_branch: None,
            });
        }
    };

    // Get current branch name
    let current_branch = match repo.head() {
        Ok(h) => h.shorthand().map(|s| s.to_string()),
        Err(_) => None,
    };

    // Check if branch already exists
    if repo.find_branch(branch_name, BranchType::Local).is_ok() {
        return Ok(SwitchBranchResult {
            success: false,
            message: Some(format!("Branch `{}` already exists", branch_name)),
            stash_name: None,
            from_branch: current_branch,
            to_branch: None,
        });
    }

    // Check for uncommitted changes
    let has_changes = has_uncommitted_changes_internal(&repo)?;
    let mut stash_name: Option<String> = None;

    // Handle uncommitted changes based on strategy
    if has_changes {
        match strategy {
            UncommittedChangesStrategy::StashOnCurrentBranch => {
                let stash_message = format!(
                    "Auto-stash on `{}` before creating `{}`",
                    current_branch.as_deref().unwrap_or("detached"),
                    branch_name
                );

                let sig = match repo.signature() {
                    Ok(s) => s,
                    Err(_) => Signature::now("gitru", "gitru@local").unwrap(),
                };

                match repo.stash_save(&sig, &stash_message, Some(StashFlags::INCLUDE_UNTRACKED)) {
                    Ok(_oid) => {
                        stash_name = Some(stash_message.clone());
                    }
                    Err(e) => {
                        return Ok(SwitchBranchResult {
                            success: false,
                            message: Some(format!("Failed to stash changes: {e}")),
                            stash_name: None,
                            from_branch: current_branch,
                            to_branch: None,
                        });
                    }
                }

                // Get HEAD OID and create branch
                let create_result = create_branch_from_head(&repo, branch_name);

                match create_result {
                    Ok(()) => {}
                    Err(e) => {
                        // Failed to create branch, recover stash
                        if let Err(pop_err) = repo.stash_pop(0, None) {
                            return Ok(SwitchBranchResult {
                                success: false,
                                message: Some(format!(
                                    "Failed to create branch: {}. Also failed to recover stash: {}",
                                    e, pop_err
                                )),
                                stash_name: None,
                                from_branch: current_branch,
                                to_branch: None,
                            });
                        }
                        return Ok(SwitchBranchResult {
                            success: false,
                            message: Some(e),
                            stash_name: None,
                            from_branch: current_branch,
                            to_branch: None,
                        });
                    }
                }

                // Checkout the new branch
                let result = perform_checkout(&repo, branch_name);

                if !result.success {
                    // Checkout failed, recover stash
                    if let Err(pop_err) = repo.stash_pop(0, None) {
                        return Ok(SwitchBranchResult {
                            success: false,
                            message: Some(format!(
                                "Checkout failed: {}. Also failed to recover stash: {}",
                                result.message.unwrap_or_default(),
                                pop_err
                            )),
                            stash_name: None,
                            from_branch: current_branch,
                            to_branch: None,
                        });
                    }
                    return Ok(SwitchBranchResult {
                        success: false,
                        message: result.message,
                        stash_name: None,
                        from_branch: current_branch,
                        to_branch: None,
                    });
                }

                return Ok(SwitchBranchResult {
                    success: true,
                    message: Some(format!(
                        "Created and switched to `{}`. Changes stashed on `{}`",
                        branch_name,
                        current_branch.as_deref().unwrap_or("previous branch")
                    )),
                    stash_name,
                    from_branch: current_branch,
                    to_branch: Some(branch_name.to_string()),
                });
            }
            UncommittedChangesStrategy::BringChanges => {
                // Create the new branch
                if let Err(e) = create_branch_from_head(&repo, branch_name) {
                    return Ok(SwitchBranchResult {
                        success: false,
                        message: Some(e),
                        stash_name: None,
                        from_branch: current_branch,
                        to_branch: None,
                    });
                }

                // Checkout - changes will come along
                let result = perform_checkout(&repo, branch_name);
                return Ok(SwitchBranchResult {
                    success: result.success,
                    message: if result.success {
                        Some(format!(
                            "Created and switched to `{}` with uncommitted changes",
                            branch_name
                        ))
                    } else {
                        result.message
                    },
                    stash_name: None,
                    from_branch: current_branch,
                    to_branch: if result.success {
                        Some(branch_name.to_string())
                    } else {
                        None
                    },
                });
            }
        }
    }

    // No uncommitted changes, just create and checkout
    if let Err(e) = create_branch_from_head(&repo, branch_name) {
        return Ok(SwitchBranchResult {
            success: false,
            message: Some(e),
            stash_name: None,
            from_branch: current_branch,
            to_branch: None,
        });
    }

    let result = perform_checkout(&repo, branch_name);
    Ok(SwitchBranchResult {
        success: result.success,
        message: if result.success {
            Some(format!("Created and switched to `{}`", branch_name))
        } else {
            result.message
        },
        stash_name: None,
        from_branch: current_branch,
        to_branch: if result.success {
            Some(branch_name.to_string())
        } else {
            None
        },
    })
}

/// Helper to create a branch from HEAD
fn create_branch_from_head(repo: &git2::Repository, branch_name: &str) -> Result<(), String> {
    let head = repo
        .head()
        .map_err(|e| format!("Failed to get HEAD: {e}"))?;
    let commit = head
        .peel_to_commit()
        .map_err(|e| format!("Failed to get HEAD commit: {e}"))?;
    repo.branch(branch_name, &commit, false)
        .map_err(|e| format!("Failed to create branch: {e}"))?;
    Ok(())
}

// ? Check if there are uncommitted changes
#[tauri::command]
#[logger::logger]
pub async fn has_uncommitted_changes(repo_path: &str) -> Result<bool, String> {
    let repo = open_repository(repo_path).map_err(|e| format!("Failed to open repo: {e}"))?;
    has_uncommitted_changes_internal(&repo)
}

/* #endregion // ! command */

/* #region // ? helpers */

/// Internal helper to check for uncommitted changes
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

/// Helper to perform the actual checkout operation
fn perform_checkout(repo: &git2::Repository, branch_name: &str) -> GitResult {
    // First, try to find local branch
    let branch = match repo.find_branch(branch_name, BranchType::Local) {
        Ok(b) => b,
        Err(_) => {
            // Try to find remote branch and create local tracking branch
            let remote_branch_name = format!("origin/{}", branch_name);
            match repo.find_branch(&remote_branch_name, BranchType::Remote) {
                Ok(remote_branch) => {
                    // Create local branch tracking the remote
                    let commit = match remote_branch.get().peel_to_commit() {
                        Ok(c) => c,
                        Err(e) => {
                            return GitResult {
                                success: false,
                                message: Some(format!("Failed to get remote branch commit: {e}")),
                            };
                        }
                    };

                    match repo.branch(branch_name, &commit, false) {
                        Ok(mut local_branch) => {
                            // Set upstream
                            let _ = local_branch.set_upstream(Some(&remote_branch_name));
                            local_branch
                        }
                        Err(e) => {
                            return GitResult {
                                success: false,
                                message: Some(format!("Failed to create local branch: {e}")),
                            };
                        }
                    }
                }
                Err(_) => {
                    return GitResult {
                        success: false,
                        message: Some(format!("Branch `{}` not found", branch_name)),
                    };
                }
            }
        }
    };

    // Get the commit to checkout
    let commit = match branch.get().peel_to_commit() {
        Ok(c) => c,
        Err(e) => {
            return GitResult {
                success: false,
                message: Some(format!("Failed to get branch commit: {e}")),
            };
        }
    };

    // Set HEAD to the branch
    let refname = format!("refs/heads/{}", branch_name);
    if let Err(e) = repo.set_head(&refname) {
        return GitResult {
            success: false,
            message: Some(format!("Failed to set HEAD: {e}")),
        };
    }

    // Checkout the tree
    let tree = match commit.tree() {
        Ok(t) => t,
        Err(e) => {
            return GitResult {
                success: false,
                message: Some(format!("Failed to get tree: {e}")),
            };
        }
    };

    let mut checkout_builder = git2::build::CheckoutBuilder::new();
    checkout_builder.safe();

    if let Err(e) = repo.checkout_tree(tree.as_object(), Some(&mut checkout_builder)) {
        return GitResult {
            success: false,
            message: Some(format!("Failed to checkout tree: {e}")),
        };
    }

    GitResult {
        success: true,
        message: None,
    }
}
// fn collect_status(repo_path: &str) -> Result<Vec<FileStatus>, String> {
//     let out = Command::new("git")
//         .current_dir(repo_path)
//         .args(["status", "--porcelain=v2", "--untracked-files=all", "-z"])
//         .output()
//         .map_err(|e| e.to_string())?;

//     if !out.status.success() {
//         return Err(String::from_utf8_lossy(&out.stderr).to_string());
//     }

//     parse_porcelain_v2(&out.stdout)
// }

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

/* #endregion // ? helpers */
