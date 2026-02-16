use crate::context::RepoContext;
use crate::models::branch::{
    AheadBehindStatus, Branch, BranchInfo, BranchKind, UncommittedChangesStrategy,
};
use crate::parsers::branch::{BRANCH_STANDARD_FORMAT, parse_branch_records};
use crate::runner::{GitRunOptions, throttle_command};
use std::sync::Arc;
use std::time::Duration;

pub struct BranchService {
    ctx: Arc<RepoContext>,
}

impl BranchService {
    pub fn new(ctx: Arc<RepoContext>) -> Self {
        Self { ctx }
    }

    #[logger::logger]
    pub async fn get_current_branch(&self) -> Result<Branch, String> {
        throttle_command(
            &format!("{}:current_branch", self.ctx.repo_path),
            std::time::Duration::from_millis(500),
        )?;

        let branch = self
            .ctx
            .runner
            .run_with_options(
                &["symbolic-ref", "--short", "-q", "HEAD"],
                GitRunOptions::default_read(),
            )
            .await
            .map_err(|e| {
                if e.contains("not a symbolic ref") {
                    "Repository is in detached HEAD state".to_string()
                } else {
                    format!("Failed to read HEAD: {e}")
                }
            })?;

        if branch.is_empty() {
            return Err("Repository is in detached HEAD state".to_string());
        }

        Ok(Branch {
            name: branch.clone(),
            display_name: branch,
            is_remote: false,
        })
    }

    #[logger::logger]
    pub async fn get_branch_info(&self, branch_name: &str) -> Result<BranchInfo, String> {
        let output = self
            .ctx
            .runner
            .run_with_options(
                &[
                    "for-each-ref",
                    "--format",
                    &BRANCH_STANDARD_FORMAT,
                    &format!("refs/heads/{branch_name}"),
                ],
                GitRunOptions::default_read(),
            )
            .await?;

        let branches = parse_branch_records(&output, false)?;
        branches
            .into_iter()
            .next()
            .ok_or_else(|| format!("Branch '{branch_name}' not found"))
    }

    #[logger::logger]
    pub async fn list_branches(&self, kind: BranchKind) -> Result<Vec<BranchInfo>, String> {
        let (refs, is_remote) = match kind {
            BranchKind::Local => ("refs/heads", false),
            BranchKind::Remote => ("refs/remotes", true),
        };

        throttle_command(
            &format!("{}:list_branches:{}", self.ctx.repo_path, refs),
            std::time::Duration::from_millis(500),
        )?;

        let output = self
            .ctx
            .runner
            .run_with_options(
                &[
                    "for-each-ref",
                    "--sort=-committerdate",
                    "--format",
                    &BRANCH_STANDARD_FORMAT,
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
    }

    #[logger::logger]
    pub async fn status_ahead_behind(&self) -> Result<AheadBehindStatus, String> {
        throttle_command(
            &format!("{}:status_ahead_behind", self.ctx.repo_path),
            std::time::Duration::from_millis(500),
        )?;

        let local_branch = self.get_current_branch().await?;
        let local_branch_info = self.get_branch_info(&local_branch.name).await?;

        let upstream_branch = local_branch_info.upstream;

        let Some(upstream_branch) = upstream_branch else {
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

        let upstream_branch_id = self
            .ctx
            .runner
            .run_with_options(&["rev-parse", "@{upstream}"], GitRunOptions::default_read())
            .await
            .ok();

        let (ahead, behind) = self
            ._ahead_behind_for("HEAD", "@{upstream}")
            .await?
            .unwrap_or((0, 0));

        Ok(AheadBehindStatus {
            ahead,
            behind,
            local_branch: local_branch.name,
            local_branch_id: local_branch_info.commit.id,
            upstream_branch: Some(upstream_branch),
            upstream_branch_id,
            is_published: true,
        })
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
                let stash_msg = format!("!!Gitru<{}> -> <{}>", current_branch.name, branch);

                self.ctx
                    .runner
                    .run_with_options(
                        &["stash", "push", "-u", "-m", &stash_msg],
                        GitRunOptions::default_read(),
                    )
                    .await
                    .map_err(|e| format!("Failed to stash changes: {e}"))?;

                match do_switch().await {
                    Ok(_) => Ok(format!(
                        "Switched to {} (changes stashed from {})",
                        branch, current_branch.name
                    )),
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

    #[logger::logger]
    pub async fn create(
        &self,
        branch: &str,
        strategy: Option<UncommittedChangesStrategy>,
    ) -> Result<String, String> {
        let current_branch = self.get_current_branch().await?;

        match strategy {
            // ? If strategy is StashOnCurrentBranch, stash FIRST before creating and switching
            Some(UncommittedChangesStrategy::StashOnCurrentBranch) => {
                let stash_msg = format!("!!Gitru<{}> -> <{}> (new)", current_branch.name, branch);
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
                    Ok(_) => Ok(format!(
                        "Created and switched to {} (changes stashed in {})",
                        branch, current_branch.name
                    )),
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

            // ? If strategy is BringChanges or None, try to create and switch directly
            _ => match self
                .ctx
                .runner
                .run_with_options(&["switch", "-c", branch], GitRunOptions::default_read())
                .await
            {
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

    #[logger::logger]
    pub async fn push(&self) -> Result<String, String> {
        self.ctx
            .runner
            .run_with_options(&["push"], GitRunOptions::default_read())
            .await?;
        Ok(format!("Pushed successfully"))
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

        Ok(format!("Published `{}` to origin", branch.name))
    }

    #[logger::logger]
    pub async fn pull(&self) -> Result<String, String> {
        self.ctx
            .runner
            .run_with_options(&["pull"], GitRunOptions::default_read())
            .await?;

        Ok(format!("Pulled successfully"))
    }

    pub async fn has_uncommitted_changes(&self) -> Result<bool, String> {
        let output = self
            .ctx
            .runner
            .run_with_options(
                &["status", "--porcelain", "-z", "--untracked-files=all"],
                GitRunOptions::default_read(),
            )
            .await?;
        Ok(!output.is_empty())
    }

    async fn _ahead_behind_for(
        &self,
        local: &str,
        upstream: &str,
    ) -> Result<Option<(usize, usize)>, String> {
        let output = self
            .ctx
            .runner
            .run_with_options(
                &[
                    "rev-list",
                    "--left-right",
                    "--count",
                    &format!("{local}...{upstream}"),
                ],
                GitRunOptions::default_read(),
            )
            .await?;

        let parts: Vec<&str> = output.split_whitespace().collect();
        if parts.len() < 2 {
            return Ok(None);
        }

        let ahead = parts[0].parse::<usize>().unwrap_or(0);
        let behind = parts[1].parse::<usize>().unwrap_or(0);

        Ok(Some((ahead, behind)))
    }
}
