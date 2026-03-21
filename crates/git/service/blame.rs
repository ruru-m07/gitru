use std::{sync::Arc, time::Duration};

use tokio::time::{Duration as TokioDuration, sleep};

use crate::{
    context::RepoContext,
    models::{
        diff::{BlameDiff, BlameInfo},
        status::FileStatusKind,
    },
    parsers::{blame::parse_blame_porcelain, stash::validate_stash_ref},
    runner::GitRunOptions,
    service::request_queue::{CancellationToken, RequestQueueManager},
};

const EMPTY_TREE_HASH: &str = "4b825dc642cb6eb9a060e54bf8d69288fbee4904";
const BLAME_ALLOW_EXIT_CODES: &[i32] = &[1, 128];
const BLAME_TIMEOUT_SECS: u64 = 6;

pub struct BlameService {
    ctx: Arc<RepoContext>,
    queue_manager: Arc<RequestQueueManager>,
}

impl BlameService {
    pub fn new(ctx: Arc<RepoContext>) -> Self {
        Self {
            ctx,
            queue_manager: Arc::new(RequestQueueManager::new()),
        }
    }

    #[allow(clippy::too_many_arguments)]
    #[logger::logger]
    pub async fn get_blame_by_file_path(
        &self,
        file_path: &str,
        file_new_path: Option<&str>,
        status: Option<&[FileStatusKind]>,
        stash_reference: Option<&str>,
        commit_hash: Option<&str>,
        parent_index: Option<usize>,
    ) -> Result<BlameDiff, String> {
        let token = self.queue_manager.register_request(file_path).await;

        if !self.queue_manager.acquire_worker_slot(&token).await {
            self.queue_manager.cleanup_request(&token).await;
            return Err("Request cancelled (superseded by newer request)".to_string());
        }

        let _guard = WorkerSlotGuard::new(self.queue_manager.clone());

        let result = async {
            if token.is_cancelled() {
                return Err("Request cancelled (superseded by newer request)".to_string());
            }

            if stash_reference.is_some() && commit_hash.is_some() {
                return Err("Cannot request stash and commit blame together".to_string());
            }

            if let Some(reference) = stash_reference {
                validate_stash_ref(reference)?;
            }
            if let Some(hash) = commit_hash {
                validate_commit_hash(hash)?;
            }

            if !should_collect_blame(file_path, file_new_path, status) {
                return Ok(BlameDiff {
                    old_blame: None,
                    new_blame: None,
                });
            }

            let old_path = file_path.to_string();
            let new_path = file_new_path.unwrap_or(file_path).to_string();
            let old_revision: Option<String>;
            let new_revision: Option<String>;

            if let Some(stash_ref) = stash_reference {
                old_revision = Some(format!("{stash_ref}^1"));
                new_revision = Some(stash_ref.to_string());
            } else if let Some(hash) = commit_hash {
                let base = resolve_commit_diff_base(
                    &self.ctx.runner,
                    hash,
                    parent_index.unwrap_or(1).max(1),
                )
                .await?;
                old_revision = Some(base);
                new_revision = Some(hash.to_string());
            } else {
                old_revision = Some("HEAD".to_string());
                new_revision = None;
            }

            let old_fut = self.get_blame_for_revision(old_revision.as_deref(), &old_path, &token);
            let new_fut = self.get_blame_for_revision(new_revision.as_deref(), &new_path, &token);
            let (old_res, new_res) = tokio::join!(old_fut, new_fut);

            if token.is_cancelled() {
                return Ok(BlameDiff {
                    old_blame: None,
                    new_blame: None,
                });
            }

            Ok(BlameDiff {
                old_blame: Some(old_res.unwrap_or_default()),
                new_blame: Some(new_res.unwrap_or_default()),
            })
        }
        .await;

        self.queue_manager.cleanup_request(&token).await;
        result
    }

    async fn get_blame_for_revision(
        &self,
        revision: Option<&str>,
        file_path: &str,
        token: &CancellationToken,
    ) -> Result<Vec<BlameInfo>, String> {
        if token.is_cancelled() {
            return Ok(Vec::new());
        }

        let mut args = vec!["blame".to_string(), "--line-porcelain".to_string()];
        if let Some(rev) = revision {
            args.push(rev.to_string());
        }
        args.push("--".to_string());
        args.push(file_path.to_string());

        if token.is_cancelled() {
            return Ok(Vec::new());
        }

        let runner = self.ctx.runner.clone();
        let blame_args = args;
        let mut command_task = tokio::spawn(async move {
            let refs: Vec<&str> = blame_args.iter().map(String::as_str).collect();
            runner
                .run_with_options_unlocked(
                    &refs,
                    GitRunOptions::default_read()
                        .allow_exit_codes(BLAME_ALLOW_EXIT_CODES)
                        .with_timeout(Duration::from_secs(BLAME_TIMEOUT_SECS)),
                )
                .await
        });

        let out = match tokio::select! {
            result = &mut command_task => {
                match result {
                    Ok(result) => result,
                    Err(err) => Err(format!("Failed to join git blame task: {err}")),
                }
            }
            _ = wait_for_cancellation(token) => {
                command_task.abort();
                return Ok(Vec::new());
            }
        } {
            Ok(out) => out,
            Err(err) if err.contains("timed out") => return Ok(Vec::new()),
            Err(err) => return Err(err),
        };

        parse_blame_porcelain(&out)
    }
}

fn should_collect_blame(
    file_path: &str,
    file_new_path: Option<&str>,
    status: Option<&[FileStatusKind]>,
) -> bool {
    let status = status.unwrap_or(&[]);
    let is_deleted = status.iter().any(|s| {
        matches!(
            s,
            FileStatusKind::IndexDeleted | FileStatusKind::WorktreeDeleted
        )
    });

    let before_path = file_path;
    let after_path = file_new_path.unwrap_or(file_path);

    // Skip known image files to avoid expensive blame on binary-like content.
    if is_supported_image_extension(before_path) || is_supported_image_extension(after_path) {
        return false;
    }

    // Deleted files can still have old blame but never new blame; we still return old blame.
    !is_deleted || !file_path.is_empty()
}

fn is_supported_image_extension(path: &str) -> bool {
    let ext = std::path::Path::new(path)
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase());
    matches!(
        ext.as_deref(),
        Some("png")
            | Some("jpg")
            | Some("jpeg")
            | Some("webp")
            | Some("gif")
            | Some("bmp")
            | Some("ico")
            | Some("avif")
    )
}

async fn resolve_commit_diff_base(
    runner: &crate::runner::GitCommandRunner,
    commit_hash: &str,
    parent_index: usize,
) -> Result<String, String> {
    let parents = runner
        .run_with_options_unlocked(
            &["show", "-s", "--format=%P", commit_hash],
            GitRunOptions::default_read(),
        )
        .await?;

    let parent_oids: Vec<&str> = parents.split_whitespace().collect();
    if parent_oids.is_empty() {
        return Ok(EMPTY_TREE_HASH.to_string());
    }

    parent_oids
        .get(parent_index.saturating_sub(1))
        .map(|oid| (*oid).to_string())
        .ok_or_else(|| format!("Parent #{parent_index} not found for commit {commit_hash}"))
}

fn validate_commit_hash(hash: &str) -> Result<(), String> {
    let is_valid = (4..=64).contains(&hash.len())
        && hash
            .chars()
            .all(|ch| ch.is_ascii_hexdigit() || ch == '^' || ch == '~');

    if is_valid {
        return Ok(());
    }

    Err(format!(
        "Invalid commit hash '{hash}': expected abbreviated or full git oid"
    ))
}

async fn wait_for_cancellation(token: &CancellationToken) {
    while !token.is_cancelled() {
        sleep(TokioDuration::from_millis(4)).await;
    }
}

struct WorkerSlotGuard {
    queue_manager: Arc<RequestQueueManager>,
}

impl WorkerSlotGuard {
    fn new(queue_manager: Arc<RequestQueueManager>) -> Self {
        Self { queue_manager }
    }
}

impl Drop for WorkerSlotGuard {
    fn drop(&mut self) {
        self.queue_manager.release_worker_slot();
    }
}
