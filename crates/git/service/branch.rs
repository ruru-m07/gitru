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
use std::collections::HashSet;
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
                    if is_remote {
                        branches.retain(|branch| !branch.name.ends_with("/HEAD"));
                    }

                    let merged_output = runner
                        .run_with_options(
                            &[
                                "for-each-ref",
                                "--merged=HEAD",
                                "--format=%(refname:short)",
                                refs,
                            ],
                            GitRunOptions::default_read(),
                        )
                        .await?;
                    let merged: HashSet<&str> = merged_output.lines().collect();

                    let protected_output = runner
                        .run_with_options(
                            &["for-each-ref", "--format=%(symref:short)", "refs/remotes"],
                            GitRunOptions::default_read(),
                        )
                        .await?;
                    let protected: HashSet<&str> = protected_output
                        .lines()
                        .filter(|name| !name.is_empty())
                        .collect();

                    for branch in &mut branches {
                        branch.is_merged = merged.contains(branch.name.as_str());
                        branch.is_protected = if branch.is_remote {
                            protected.contains(branch.name.as_str())
                        } else {
                            branch
                                .upstream
                                .as_deref()
                                .is_some_and(|upstream| protected.contains(upstream))
                                || protected.iter().any(|remote_default| {
                                    remote_default
                                        .split_once('/')
                                        .is_some_and(|(_, name)| name == branch.name)
                                })
                        };
                    }
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

                    // Detached HEAD (rebase / bisect / oid checkout) has no upstream.
                    if local_branch.is_detached {
                        return Ok(AheadBehindStatus {
                            ahead: 0,
                            behind: 0,
                            local_branch: local_branch.name.clone(),
                            local_branch_id: local_branch.name,
                            upstream_branch: None,
                            upstream_branch_id: None,
                            is_published: false,
                            is_detached: true,
                        });
                    }

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
                            is_detached: false,
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
                        is_detached: false,
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

        let local_ref = format!("refs/heads/{branch}");
        let remote_ref = format!("refs/remotes/{branch}");
        let is_local = self.reference_exists(&local_ref).await;
        let is_remote = self.reference_exists(&remote_ref).await;

        if branch.ends_with("/HEAD") && is_remote {
            return Err("Choose a concrete remote branch instead of its HEAD alias".to_string());
        }

        // Prefer an exact local branch when names overlap. Any remote name is
        // accepted, not only origin/*, and Git derives the tracking local name.
        let switch_args: Vec<&str> = if !is_local && is_remote {
            vec!["switch", "--track", branch]
        } else {
            vec!["switch", "--", branch]
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
                            "Failed to switch to {branch} even after stashing: {err}"
                        ))
                    }
                }
            }

            _ => match do_switch().await {
                Ok(_) => {
                    self.ctx.cache.invalidate_all();
                    Ok(format!("Switched to {branch}"))
                }
                Err(err) => match strategy {
                    Some(UncommittedChangesStrategy::BringChanges) => Err(format!(
                        "Cannot bring uncommitted changes to {branch}: conflicts detected"
                    )),
                    None => Err(format!("Cannot switch to {branch}: {err}")),
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
                            "Failed to create branch {branch} even after stashing: {err}"
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
                    Ok(format!("Created and switched to {branch}"))
                }
                Err(err) => match strategy {
                    Some(UncommittedChangesStrategy::BringChanges) => Err(format!(
                        "Cannot bring uncommitted changes to new branch {branch}: {err}"
                    )),
                    None => Err(format!("Cannot create branch {branch}: {err}")),
                    _ => unreachable!(),
                },
            },
        }
    }

    #[logger::logger]
    pub async fn rename(&self, branch: &str, new_name: &str) -> Result<String, String> {
        self.validate_local_branch(branch).await?;
        self.validate_branch_name(new_name).await?;

        self.ctx
            .runner
            .run_with_options(
                &["branch", "-m", "--", branch, new_name],
                GitRunOptions::default_read(),
            )
            .await?;
        self.ctx.cache.invalidate_all();
        Ok(format!("Renamed `{branch}` to `{new_name}`"))
    }

    #[logger::logger]
    pub async fn delete_local(&self, branch: &str, force: bool) -> Result<String, String> {
        let info = self.local_branch_info(branch).await?;
        if info.is_head {
            return Err(format!(
                "Cannot delete the current branch `{branch}`. Check out another branch first."
            ));
        }
        if info.is_protected {
            return Err(format!(
                "Cannot delete protected branch `{branch}`. Change the remote default branch first."
            ));
        }
        if !info.is_merged && !force {
            return Err(format!(
                "Branch `{branch}` contains commits not merged into HEAD. Review them before force deleting."
            ));
        }

        let delete_flag = if force { "-D" } else { "-d" };
        self.ctx
            .runner
            .run_with_options(
                &["branch", delete_flag, "--", branch],
                GitRunOptions::default_read(),
            )
            .await?;
        self.ctx.cache.invalidate_all();
        Ok(format!("Deleted local branch `{branch}`"))
    }

    #[logger::logger]
    pub async fn delete_remote(&self, branch: &str, force: bool) -> Result<String, String> {
        let info = self.remote_branch_info(branch).await?;
        if info.is_protected {
            return Err(format!(
                "Cannot delete protected remote branch `{branch}`. Change the remote default branch first."
            ));
        }
        if !info.is_merged && !force {
            return Err(format!(
                "Remote branch `{branch}` contains commits not merged into HEAD. Review them before force deleting."
            ));
        }

        let current = self.get_current_branch().await?;
        if !current.is_detached {
            let current_info = self.get_branch_info(&current.name).await?;
            if current_info.upstream.as_deref() == Some(branch) {
                return Err(format!(
                    "Cannot delete `{branch}` while it is the upstream of current branch `{}`. Unset or change the upstream first.",
                    current.name
                ));
            }
        }

        let (remote, remote_branch) = self.split_remote_branch(branch).await?;
        self.ctx
            .runner
            .run_with_options(
                &["push", remote.as_str(), "--delete", remote_branch],
                GitRunOptions::default_read().with_timeout(Duration::from_secs(60)),
            )
            .await?;
        self.ctx.cache.invalidate_all();
        Ok(format!("Deleted remote branch `{branch}`"))
    }

    #[logger::logger]
    pub async fn set_upstream(&self, branch: &str, upstream: &str) -> Result<String, String> {
        self.validate_local_branch(branch).await?;
        self.remote_branch_info(upstream).await?;

        self.ctx
            .runner
            .run_with_options(
                &["branch", "--set-upstream-to", upstream, branch],
                GitRunOptions::default_read(),
            )
            .await?;
        self.ctx.cache.invalidate_all();
        Ok(format!("Set `{upstream}` as the upstream of `{branch}`"))
    }

    #[logger::logger]
    pub async fn unset_upstream(&self, branch: &str) -> Result<String, String> {
        let info = self.local_branch_info(branch).await?;
        if info.upstream.is_none() {
            return Err(format!("Branch `{branch}` does not have an upstream"));
        }

        self.ctx
            .runner
            .run_with_options(
                &["branch", "--unset-upstream", branch],
                GitRunOptions::default_read(),
            )
            .await?;
        self.ctx.cache.invalidate_all();
        Ok(format!("Unset the upstream of `{branch}`"))
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

        if branch.is_detached {
            return Err(
                "HEAD is detached — finish or abort the current operation before publishing"
                    .to_string(),
            );
        }

        self.ctx
            .runner
            .run_with_options(
                &["push", "-u", "origin", branch.name.as_str()],
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

    async fn reference_exists(&self, reference: &str) -> bool {
        self.ctx
            .runner
            .run_with_options(
                &["show-ref", "--verify", "--quiet", reference],
                GitRunOptions::default_read(),
            )
            .await
            .is_ok()
    }

    async fn validate_branch_name(&self, branch: &str) -> Result<(), String> {
        self.ctx
            .runner
            .run_with_options(
                &["check-ref-format", "--branch", branch],
                GitRunOptions::default_read(),
            )
            .await
            .map(|_| ())
            .map_err(|_| format!("Invalid branch name `{branch}`"))
    }

    async fn validate_local_branch(&self, branch: &str) -> Result<(), String> {
        self.validate_branch_name(branch).await?;
        if self.reference_exists(&format!("refs/heads/{branch}")).await {
            Ok(())
        } else {
            Err(format!("Local branch `{branch}` does not exist"))
        }
    }

    async fn local_branch_info(&self, branch: &str) -> Result<BranchInfo, String> {
        self.validate_local_branch(branch).await?;
        self.list_branches(BranchKind::Local)
            .await?
            .into_iter()
            .find(|candidate| candidate.name == branch)
            .ok_or_else(|| format!("Local branch `{branch}` does not exist"))
    }

    async fn remote_branch_info(&self, branch: &str) -> Result<BranchInfo, String> {
        if branch.ends_with("/HEAD") {
            return Err("Choose a concrete remote branch instead of its HEAD alias".to_string());
        }
        self.list_branches(BranchKind::Remote)
            .await?
            .into_iter()
            .find(|candidate| candidate.name == branch)
            .ok_or_else(|| format!("Remote branch `{branch}` does not exist"))
    }

    async fn split_remote_branch<'a>(&self, branch: &'a str) -> Result<(String, &'a str), String> {
        let remotes = self
            .ctx
            .runner
            .run_with_options(&["remote"], GitRunOptions::default_read())
            .await?;
        let mut matching: Vec<&str> = remotes
            .lines()
            .filter(|remote| branch.starts_with(&format!("{remote}/")))
            .collect();
        matching.sort_by_key(|remote| std::cmp::Reverse(remote.len()));
        let remote = matching
            .first()
            .ok_or_else(|| format!("Cannot determine the remote for `{branch}`"))?;
        let remote_branch = &branch[remote.len() + 1..];
        if remote_branch.is_empty() {
            return Err(format!("Remote branch `{branch}` is invalid"));
        }
        Ok(((*remote).to_string(), remote_branch))
    }
}
