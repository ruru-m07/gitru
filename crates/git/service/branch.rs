use crate::cache::{CachePolicy, TTL_AHEAD_BEHIND, TTL_CURRENT_BRANCH_STASH, TTL_LIST_BRANCHES};
use crate::context::RepoContext;
use crate::models::branch::{
    AheadBehindStatus, Branch, BranchInfo, BranchKind, BranchStash, UncommittedChangesStrategy,
};
use crate::parsers::branch::{BRANCH_STANDARD_FORMAT, parse_branch_records};
use crate::runner::GitRunOptions;
use crate::service::query::QueryService;
use std::sync::Arc;
use std::time::Duration;

pub struct BranchService {
    ctx: Arc<RepoContext>,
}

impl BranchService {
    pub fn new(ctx: Arc<RepoContext>) -> Self {
        Self { ctx }
    }

    fn query(&self) -> QueryService {
        QueryService::new(self.ctx.clone())
    }

    #[logger::logger]
    pub async fn get_current_branch(&self) -> Result<Branch, String> {
        self.query().current_branch().await
    }

    #[logger::logger]
    pub async fn get_branch_info(&self, branch_name: &str) -> Result<BranchInfo, String> {
        self.query().branch_info(branch_name).await
    }

    #[logger::logger]
    pub async fn list_branches(&self, kind: BranchKind) -> Result<Vec<BranchInfo>, String> {
        let (refs, is_remote) = match kind {
            BranchKind::Local => ("refs/heads", false),
            BranchKind::Remote => ("refs/remotes", true),
        };
        let refs_key = refs.to_string();
        let runner = self.ctx.runner.clone();

        self.ctx
            .cache
            .get_or_refresh(
                CachePolicy {
                    namespace: "list_branches",
                    ttl: TTL_LIST_BRANCHES,
                },
                refs_key.clone(),
                move || async move {
                    let output = runner
                        .run_with_options(
                            &[
                                "for-each-ref",
                                "--sort=-committerdate",
                                "--format",
                                BRANCH_STANDARD_FORMAT,
                                refs,
                            ],
                            GitRunOptions::default_read().with_timeout(Duration::from_secs(60)),
                        )
                        .await?;

                    let mut branches = parse_branch_records(&output, is_remote)?;
                    branches.sort_by(|a, b| {
                        b.is_head
                            .cmp(&a.is_head)
                            .then(a.is_remote.cmp(&b.is_remote))
                            .then(a.display_name.cmp(&b.display_name))
                    });

                    Ok(branches)
                },
            )
            .await
    }

    #[logger::logger]
    pub async fn status_ahead_behind(&self) -> Result<AheadBehindStatus, String> {
        let runner = self.ctx.runner.clone();
        self.ctx
            .cache
            .get_or_refresh(
                CachePolicy {
                    namespace: "ahead_behind_status",
                    ttl: TTL_AHEAD_BEHIND,
                },
                "head_upstream".to_string(),
                move || async move {
                    let local_branch = self.query().current_branch().await?;
                    let local_branch_info = self.query().branch_info(&local_branch.name).await?;

                    let Some(upstream_branch) = local_branch_info.upstream.clone() else {
                        return Ok(AheadBehindStatus {
                            ahead: 0,
                            behind: 0,
                            local_branch: local_branch.name,
                            local_branch_id: local_branch_info.commit.id,
                            upstream_branch: None,
                            upstream_branch_id: None,
                            is_published: false,
                        });
                    };

                    let upstream_branch_id = runner
                        .run_with_options(
                            &["rev-parse", "@{upstream}"],
                            GitRunOptions::default_read(),
                        )
                        .await
                        .ok();

                    let output = runner
                        .run_with_options(
                            &["rev-list", "--left-right", "--count", "HEAD...@{upstream}"],
                            GitRunOptions::default_read(),
                        )
                        .await?;

                    let parts: Vec<&str> = output.split_whitespace().collect();
                    let (ahead, behind) = if parts.len() >= 2 {
                        (
                            parts[0].parse::<usize>().unwrap_or(0),
                            parts[1].parse::<usize>().unwrap_or(0),
                        )
                    } else {
                        (0, 0)
                    };

                    Ok(AheadBehindStatus {
                        ahead,
                        behind,
                        local_branch: local_branch.name,
                        local_branch_id: local_branch_info.commit.id,
                        upstream_branch: Some(upstream_branch),
                        upstream_branch_id,
                        is_published: true,
                    })
                },
            )
            .await
    }

    #[logger::logger]
    pub async fn switch(
        &self,
        branch: &str,
        strategy: Option<UncommittedChangesStrategy>,
    ) -> Result<String, String> {
        let current_branch = self.get_current_branch().await?;

        // Decide how to switch:
        // - local branch  -> git switch <branch>
        // - remote branch -> git switch --track <branch>
        let switch_args: Vec<&str> = if branch.starts_with("origin/") {
            vec!["switch", "--track", branch]
        } else {
            vec!["switch", branch]
        };

        let do_switch = || {
            self.ctx
                .runner
                .run_with_options(&switch_args, GitRunOptions::default_read())
        };

        match strategy {
            Some(UncommittedChangesStrategy::StashOnCurrentBranch) => {
                let stash_msg = format!("!!gitru<{}> -> <{}>", current_branch.name, branch);

                self.ctx
                    .runner
                    .run_with_options(
                        &["stash", "push", "-u", "-m", &stash_msg],
                        GitRunOptions::default_read(),
                    )
                    .await
                    .map_err(|e| format!("Failed to stash changes: {e}"))?;

                match do_switch().await {
                    Ok(_) => {
                        self.ctx.cache.invalidate_all();
                        Ok(format!(
                            "Switched to {} (changes stashed from {})",
                            branch, current_branch.name
                        ))
                    }
                    Err(err) => {
                        let _ = self
                            .ctx
                            .runner
                            .run_with_options(&["stash", "pop"], GitRunOptions::default_read());
                        Err(format!(
                            "Failed to switch to {} even after stashing: {}",
                            branch, err
                        ))
                    }
                }
            }

            _ => match do_switch().await {
                Ok(_) => {
                    self.ctx.cache.invalidate_all();
                    Ok(format!("Switched to {}", branch))
                }
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

    #[logger::logger]
    pub async fn create(
        &self,
        branch: &str,
        strategy: Option<UncommittedChangesStrategy>,
    ) -> Result<String, String> {
        let current_branch = self.get_current_branch().await?;

        match strategy {
            // If strategy is StashOnCurrentBranch, stash FIRST before creating and switching.
            Some(UncommittedChangesStrategy::StashOnCurrentBranch) => {
                let stash_msg = format!("!!gitru<{}> -> <{}> (new)", current_branch.name, branch);
                self.ctx
                    .runner
                    .run_with_options(
                        &["stash", "push", "-u", "-m", &stash_msg],
                        GitRunOptions::default_read(),
                    )
                    .await
                    .map_err(|e| format!("Failed to stash changes: {}", e))?;

                match self
                    .ctx
                    .runner
                    .run_with_options(&["switch", "-c", branch], GitRunOptions::default_read())
                    .await
                {
                    Ok(_) => {
                        self.ctx.cache.invalidate_all();
                        Ok(format!(
                            "Created and switched to {} (changes stashed in {})",
                            branch, current_branch.name
                        ))
                    }
                    Err(err) => {
                        let _ = self
                            .ctx
                            .runner
                            .run_with_options(&["stash", "pop"], GitRunOptions::default_read());
                        Err(format!(
                            "Failed to create branch {} even after stashing: {}",
                            branch, err
                        ))
                    }
                }
            }

            // If strategy is BringChanges or None, try to create and switch directly.
            _ => match self
                .ctx
                .runner
                .run_with_options(&["switch", "-c", branch], GitRunOptions::default_read())
                .await
            {
                Ok(_) => {
                    self.ctx.cache.invalidate_all();
                    Ok(format!("Created and switched to {}", branch))
                }
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

    #[logger::logger]
    pub async fn push(&self) -> Result<String, String> {
        self.ctx
            .runner
            .run_with_options(&["push"], GitRunOptions::default_read())
            .await?;
        self.ctx.cache.invalidate_all();
        Ok("Pushed successfully".to_string())
    }

    #[logger::logger]
    pub async fn publish_branch(&self) -> Result<String, String> {
        let branch = self.get_current_branch().await?;

        self.ctx
            .runner
            .run_with_options(
                &["push", "-u", "origin", format!("@{}", branch.name).as_str()],
                GitRunOptions::default_read().with_timeout(Duration::from_secs(60)),
            )
            .await?;

        self.ctx.cache.invalidate_all();
        Ok(format!("Published `{}` to origin", branch.name))
    }

    #[logger::logger]
    pub async fn pull(&self) -> Result<String, String> {
        self.ctx
            .runner
            .run_with_options(&["pull"], GitRunOptions::default_read())
            .await?;

        self.ctx.cache.invalidate_all();
        Ok("Pulled successfully".to_string())
    }

    pub async fn has_uncommitted_changes(&self) -> Result<bool, String> {
        self.query().has_uncommitted_changes().await
    }

    #[logger::logger]
    pub async fn current_branch_stash(&self) -> Result<Option<BranchStash>, String> {
        let current_branch = self.get_current_branch().await?;
        let branch_name = current_branch.name.clone();
        let runner = self.ctx.runner.clone();

        self.ctx
            .cache
            .get_or_refresh(
                CachePolicy {
                    namespace: "current_branch_stash",
                    ttl: TTL_CURRENT_BRANCH_STASH,
                },
                branch_name.clone(),
                move || async move {
                    let output = runner
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

                        let Some((from_branch, to_branch)) =
                            Self::parse_gitru_stash_message(message)
                        else {
                            continue;
                        };

                        if Self::branch_name_matches(&from_branch, &branch_name) {
                            return Ok(Some(BranchStash {
                                reference: reference.to_string(),
                                message: message.to_string(),
                                from_branch,
                                to_branch,
                            }));
                        }
                    }

                    Ok(None)
                },
            )
            .await
    }

    #[logger::logger]
    pub async fn pop_current_branch_stash(&self) -> Result<String, String> {
        let current_branch = self.get_current_branch().await?;
        let stash = self
            .find_stash_for_branch(&current_branch.name)
            .await?
            .ok_or_else(|| {
                format!(
                    "No !!gitru stash found for branch '{}'",
                    current_branch.name
                )
            })?;

        self.ctx
            .runner
            .run_with_options(
                &["stash", "pop", &stash.reference],
                GitRunOptions::default_read(),
            )
            .await?;

        self.ctx.cache.invalidate_all();

        Ok(format!(
            "Popped {} onto branch '{}'",
            stash.reference, current_branch.name
        ))
    }

    async fn find_stash_for_branch(&self, branch: &str) -> Result<Option<BranchStash>, String> {
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

            let Some((from_branch, to_branch)) = Self::parse_gitru_stash_message(message) else {
                continue;
            };

            if Self::branch_name_matches(&from_branch, branch) {
                return Ok(Some(BranchStash {
                    reference: reference.to_string(),
                    message: message.to_string(),
                    from_branch,
                    to_branch,
                }));
            }
        }

        Ok(None)
    }

    fn parse_gitru_stash_message(message: &str) -> Option<(String, String)> {
        let marker = "!!gitru<";
        let lower = message.to_ascii_lowercase();
        let marker_start = lower.find(marker)?;
        let after_prefix = &message[marker_start + marker.len()..];
        let from_end = after_prefix.find('>')?;
        let from_branch = after_prefix[..from_end].trim();

        let after_from = &after_prefix[from_end + 1..];
        let after_arrow = after_from.strip_prefix(" -> <")?;
        let to_end = after_arrow.find('>')?;
        let to_branch = after_arrow[..to_end].trim();

        if from_branch.is_empty() || to_branch.is_empty() {
            return None;
        }

        Some((from_branch.to_string(), to_branch.to_string()))
    }

    fn branch_name_matches(stash_target: &str, branch: &str) -> bool {
        stash_target == branch
            || stash_target
                .strip_prefix("origin/")
                .is_some_and(|s| s == branch)
            || branch
                .strip_prefix("origin/")
                .is_some_and(|s| s == stash_target)
    }
}
