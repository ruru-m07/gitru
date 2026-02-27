use crate::cache::{CachePolicy, TTL_AHEAD_BEHIND, TTL_LIST_BRANCHES};
use crate::context::RepoContext;
use crate::models::branch::{
    AheadBehindStatus, Branch, BranchInfo, BranchKind, UncommittedChangesStrategy,
};
use crate::models::stash::BranchStash;
use crate::parsers::branch::{BRANCH_STANDARD_FORMAT, parse_branch_records};
use crate::runner::GitRunOptions;
use crate::service::query::QueryService;
use crate::service::stash::StashService;
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

    fn stash(&self) -> StashService {
        StashService::new(self.ctx.clone())
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
                let did_create_stash = self
                    .stash()
                    .push_gitru_stash(&current_branch.name, branch, false)
                    .await?;

                match do_switch().await {
                    Ok(_) => {
                        self.ctx.cache.invalidate_all();
                        Ok(format!(
                            "Switched to {} (changes stashed from {})",
                            branch, current_branch.name
                        ))
                    }
                    Err(err) => {
                        if did_create_stash {
                            let _ = self.stash().pop(None).await;
                        }
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
                let did_create_stash = self
                    .stash()
                    .push_gitru_stash(&current_branch.name, branch, true)
                    .await?;

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
                        if did_create_stash {
                            let _ = self.stash().pop(None).await;
                        }
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
        self.stash()
            .find_gitru_stash_for_branch(&current_branch.name)
            .await
    }

    #[logger::logger]
    pub async fn pop_current_branch_stash(&self) -> Result<String, String> {
        let current_branch = self.get_current_branch().await?;
        self.stash()
            .pop_gitru_stash_for_branch(&current_branch.name)
            .await
    }
}
