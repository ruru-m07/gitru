use std::sync::Arc;

use crate::cache::{
    CachePolicy, TTL_BRANCH_INFO, TTL_CURRENT_BRANCH, TTL_HAS_UNCOMMITTED, TTL_REPOSITORY_ORIGIN,
};
use crate::context::RepoContext;
use crate::models::branch::{Branch, BranchInfo};
use crate::models::origin::RepositoryOrigin;
use crate::parsers::branch::{BRANCH_STANDARD_FORMAT, parse_branch_records};
use crate::parsers::origin::parse_remote_url;
use crate::runner::GitRunOptions;

pub struct QueryService {
    ctx: Arc<RepoContext>,
}

impl QueryService {
    pub fn new(ctx: Arc<RepoContext>) -> Self {
        Self { ctx }
    }

    #[logger::logger]
    pub async fn current_branch(&self) -> Result<Branch, String> {
        let runner = self.ctx.runner.clone();
        self.ctx
            .cache
            .get_or_refresh(
                CachePolicy {
                    namespace: "current_branch",
                    ttl: TTL_CURRENT_BRANCH,
                },
                "head".to_string(),
                move || async move {
                    let branch = runner
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
                },
            )
            .await
    }

    #[logger::logger]
    pub async fn branch_info(&self, branch_name: &str) -> Result<BranchInfo, String> {
        let branch_name = branch_name.to_string();
        let runner = self.ctx.runner.clone();

        self.ctx
            .cache
            .get_or_refresh(
                CachePolicy {
                    namespace: "branch_info",
                    ttl: TTL_BRANCH_INFO,
                },
                branch_name.clone(),
                move || async move {
                    let output = runner
                        .run_with_options(
                            &[
                                "for-each-ref",
                                "--format",
                                BRANCH_STANDARD_FORMAT,
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
                },
            )
            .await
    }

    #[logger::logger]
    pub async fn has_uncommitted_changes(&self) -> Result<bool, String> {
        let runner = self.ctx.runner.clone();

        self.ctx
            .cache
            .get_or_refresh(
                CachePolicy {
                    namespace: "has_uncommitted_changes",
                    ttl: TTL_HAS_UNCOMMITTED,
                },
                "porcelain".to_string(),
                move || async move {
                    let output = runner
                        .run_with_options(
                            &["status", "--porcelain", "-z", "--untracked-files=all"],
                            GitRunOptions::default_read(),
                        )
                        .await?;
                    Ok(!output.is_empty())
                },
            )
            .await
    }

    #[logger::logger]
    pub async fn repository_origin(&self) -> Result<RepositoryOrigin, String> {
        let runner = self.ctx.runner.clone();

        self.ctx
            .cache
            .get_or_refresh(
                CachePolicy {
                    namespace: "repository_origin",
                    ttl: TTL_REPOSITORY_ORIGIN,
                },
                "origin".to_string(),
                move || async move {
                    let url = runner
                        .run_with_options(
                            &["remote", "get-url", "origin"],
                            GitRunOptions::default_read(),
                        )
                        .await
                        .map_err(|_| "No origin remote found".to_string())?;

                    let (protocol, host, owner, repo_name, provider) = parse_remote_url(&url);

                    Ok(RepositoryOrigin {
                        remote_name: "origin".into(),
                        remote_url: url,
                        host,
                        provider,
                        owner,
                        repo: repo_name,
                        protocol,
                    })
                },
            )
            .await
    }
}

impl Clone for QueryService {
    fn clone(&self) -> Self {
        Self {
            ctx: self.ctx.clone(),
        }
    }
}
