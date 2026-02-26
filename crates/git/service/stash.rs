use std::sync::Arc;

use crate::{
    cache::{CachePolicy, TTL_STASH_LIST},
    context::RepoContext,
    models::stash::{BranchStash, StashEntry, StashQuickStat, StashShowResponse},
    parsers::stash::{
        branch_name_matches, parse_gitru_stash_message, parse_stash_file_status, parse_stash_list,
        parse_stash_stat, validate_stash_ref,
    },
    runner::GitRunOptions,
};

const DEFAULT_STASH_REF: &str = "stash@{0}";

pub struct StashService {
    ctx: Arc<RepoContext>,
}

impl StashService {
    pub fn new(ctx: Arc<RepoContext>) -> Self {
        Self { ctx }
    }

    // ── queries ──────────────────────────────────────────────────────

    /// List all stash entries.
    #[logger::logger]
    pub async fn list(&self) -> Result<Vec<StashEntry>, String> {
        let runner = self.ctx.runner.clone();

        self.ctx
            .cache
            .get_or_refresh(
                CachePolicy {
                    namespace: "stash_list",
                    ttl: TTL_STASH_LIST,
                },
                "all".to_string(),
                move || async move {
                    let output = runner
                        .run_with_options(
                            &["stash", "list", "--format=%gd%x1f%gs"],
                            GitRunOptions::default_read(),
                        )
                        .await?;

                    if output.is_empty() {
                        return Ok(Vec::new());
                    }

                    parse_stash_list(&output)
                },
            )
            .await
    }

    /// Quick numeric stat for a stash (`git stash show --stat`).
    #[logger::logger]
    pub async fn quick_stat(&self, reference: &str) -> Result<StashQuickStat, String> {
        validate_stash_ref(reference)?;

        let output = self
            .ctx
            .runner
            .run_with_options(
                &["stash", "show", "--stat", reference],
                GitRunOptions::default_read(),
            )
            .await?;

        parse_stash_stat(&output, reference)
    }

    /// Full stash detail: quick stat + per-file status.
    #[logger::logger]
    pub async fn show(&self, reference: &str) -> Result<StashShowResponse, String> {
        validate_stash_ref(reference)?;

        let stat_output = self
            .ctx
            .runner
            .run_with_options(
                &["stash", "show", "--stat", reference],
                GitRunOptions::default_read(),
            )
            .await?;

        let file_output = self
            .ctx
            .runner
            .run_with_options(
                &["stash", "show", "--name-status", "-z", reference],
                GitRunOptions::default_read(),
            )
            .await?;

        let stat = parse_stash_stat(&stat_output, reference)?;
        let files = parse_stash_file_status(file_output.as_bytes())?;

        Ok(StashShowResponse {
            reference: reference.to_string(),
            stat,
            files,
        })
    }

    // ── mutations ────────────────────────────────────────────────────

    /// Create a new stash entry.
    #[logger::logger]
    pub async fn push(
        &self,
        message: Option<&str>,
        include_untracked: bool,
    ) -> Result<String, String> {
        let mut args = vec!["stash", "push"];

        if include_untracked {
            args.push("-u");
        }

        let msg_flag;
        if let Some(msg) = message {
            args.push("-m");
            msg_flag = msg.to_string();
            args.push(&msg_flag);
        }

        let output = self
            .ctx
            .runner
            .run_with_options(&args, GitRunOptions::default_read())
            .await?;

        if output == "No local changes to save" {
            return Ok("No local changes to save".to_string());
        }

        self.ctx.cache.invalidate_all();
        Ok("Stash created".to_string())
    }

    /// Pop a stash entry (apply + remove).
    ///
    /// Behavior mirrors GitHub Desktop:
    /// - no custom path-based recovery,
    /// - keep stash when apply fails/conflicts,
    /// - treat missing ref after failure as a successful pop.
    #[logger::logger]
    pub async fn pop(&self, reference: Option<&str>) -> Result<String, String> {
        let resolved_ref = self.resolve_reference(reference)?;

        if !self.stash_ref_exists(&resolved_ref).await {
            return Err(format!("Stash '{}' does not exist", resolved_ref));
        }

        let result = self
            .ctx
            .runner
            .run_with_options(
                &["stash", "pop", "--quiet", &resolved_ref],
                GitRunOptions::default_read(),
            )
            .await;

        match result {
            Ok(_) => {
                self.ctx.cache.invalidate_all();
                Ok(format!("Popped {}", resolved_ref))
            }
            Err(err) => {
                // Some git versions can report non-zero exit even when the stash
                // entry has already been removed. If the ref is gone, treat as success.
                if !self.stash_ref_exists(&resolved_ref).await {
                    self.ctx.cache.invalidate_all();
                    return Ok(format!("Popped {}", resolved_ref));
                }

                if Self::is_untracked_conflict_error(&err) {
                    return Err(format!(
                        "Unable to pop {} because untracked files would be overwritten. Resolve file conflicts and retry.",
                        resolved_ref
                    ));
                }

                if Self::is_merge_conflict_error(&err) {
                    return Err(format!(
                        "Stash {} was applied with conflicts and kept in stash list. Resolve conflicts, then drop it when ready.",
                        resolved_ref
                    ));
                }

                Err(err)
            }
        }
    }

    /// Apply a stash entry without removing it.
    #[logger::logger]
    pub async fn apply(&self, reference: Option<&str>) -> Result<String, String> {
        let resolved_ref = self.resolve_reference(reference)?;

        self.ctx
            .runner
            .run_with_options(
                &["stash", "apply", "--quiet", &resolved_ref],
                GitRunOptions::default_read(),
            )
            .await?;

        self.ctx.cache.invalidate_all();
        Ok(format!("Applied {}", resolved_ref))
    }

    /// Drop a single stash entry.
    #[logger::logger]
    pub async fn drop(&self, reference: &str) -> Result<String, String> {
        validate_stash_ref(reference)?;

        self.ctx
            .runner
            .run_with_options(
                &["stash", "drop", "--quiet", reference],
                GitRunOptions::default_read(),
            )
            .await?;

        self.ctx.cache.invalidate_all();
        Ok(format!("Dropped {}", reference))
    }

    /// Clear all stash entries.
    #[logger::logger]
    pub async fn clear(&self) -> Result<String, String> {
        self.ctx
            .runner
            .run_with_options(&["stash", "clear"], GitRunOptions::default_read())
            .await?;

        self.ctx.cache.invalidate_all();
        Ok("All stashes cleared".to_string())
    }

    /// Create a new branch from a stash entry.
    #[logger::logger]
    pub async fn branch(
        &self,
        branch_name: &str,
        reference: Option<&str>,
    ) -> Result<String, String> {
        let resolved_ref = self.resolve_reference(reference)?;

        self.ctx
            .runner
            .run_with_options(
                &["stash", "branch", branch_name, &resolved_ref],
                GitRunOptions::default_read(),
            )
            .await?;

        self.ctx.cache.invalidate_all();
        Ok(format!(
            "Created branch '{}' from {}",
            branch_name, resolved_ref
        ))
    }

    // ── Gitru branch-stash helpers ───────────────────────────────────

    /// Push a Gitru-managed stash during branch switching.
    ///
    /// Creates a stash with the `!!Gitru<from> -> <to>` message pattern.
    #[logger::logger]
    pub async fn push_gitru_stash(
        &self,
        from_branch: &str,
        to_branch: &str,
        is_new_branch: bool,
    ) -> Result<String, String> {
        let suffix = if is_new_branch { " (new)" } else { "" };
        let stash_msg = format!("!!Gitru<{}> -> <{}>{}", from_branch, to_branch, suffix);

        self.ctx
            .runner
            .run_with_options(
                &["stash", "push", "-u", "-m", &stash_msg],
                GitRunOptions::default_read(),
            )
            .await
            .map_err(|e| format!("Failed to stash changes: {e}"))?;

        self.ctx.cache.invalidate_all();
        Ok(stash_msg)
    }

    /// Find a Gitru-managed stash for a specific branch.
    #[logger::logger]
    pub async fn find_gitru_stash_for_branch(
        &self,
        branch: &str,
    ) -> Result<Option<BranchStash>, String> {
        let output = self
            .ctx
            .runner
            .run_with_options(
                &["stash", "list", "--format=%gd%x1f%gs"],
                GitRunOptions::default_read(),
            )
            .await?;

        if output.is_empty() {
            return Ok(None);
        }

        for line in output.lines() {
            let mut parts = line.splitn(2, '\x1f');
            let Some(reference) = parts.next() else {
                continue;
            };
            let Some(message) = parts.next() else {
                continue;
            };

            let Some((from_branch, to_branch)) = parse_gitru_stash_message(message) else {
                continue;
            };

            if branch_name_matches(&from_branch, branch) {
                let stat = self.quick_stat(reference).await.unwrap_or(StashQuickStat {
                    reference: reference.to_string(),
                    files_changed: 0,
                    insertions: 0,
                    deletions: 0,
                });

                return Ok(Some(BranchStash {
                    reference: reference.to_string(),
                    message: message.to_string(),
                    from_branch,
                    to_branch,
                    files_changed: stat.files_changed,
                    insertions: stat.insertions,
                    deletions: stat.deletions,
                }));
            }
        }

        Ok(None)
    }

    /// Pop the Gitru-managed stash for the current branch.
    #[logger::logger]
    pub async fn pop_gitru_stash_for_branch(&self, branch: &str) -> Result<String, String> {
        let stash = self
            .find_gitru_stash_for_branch(branch)
            .await?
            .ok_or_else(|| format!("No !!Gitru stash found for branch '{}'", branch))?;

        self.pop(Some(&stash.reference)).await
    }

    // ── internal helpers ─────────────────────────────────────────────

    fn resolve_reference(&self, reference: Option<&str>) -> Result<String, String> {
        let resolved_ref = reference.unwrap_or(DEFAULT_STASH_REF);
        validate_stash_ref(resolved_ref)?;
        Ok(resolved_ref.to_string())
    }

    async fn stash_ref_exists(&self, reference: &str) -> bool {
        self.ctx
            .runner
            .run_with_options(
                &["rev-parse", "--verify", "--quiet", reference],
                GitRunOptions::default_read(),
            )
            .await
            .is_ok()
    }

    fn is_untracked_conflict_error(err: &str) -> bool {
        err.contains("could not restore untracked files from stash")
            || err.contains("untracked working tree files would be overwritten")
    }

    fn is_merge_conflict_error(err: &str) -> bool {
        err.contains("CONFLICT")
            || err.contains("Merge conflict")
            || err.contains("needs merge")
            || err.contains("could not apply")
    }
}
