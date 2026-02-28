use crate::{
    cache::CachePolicy,
    cache::TTL_STATUS,
    context::RepoContext,
    models::status::GetStatusResponse,
    parsers::status::parse_porcelain_v2,
    runner::{GitRunOptions, validate_relative_path},
};
use std::sync::Arc;

pub struct ActionService {
    ctx: Arc<RepoContext>,
}

impl ActionService {
    pub fn new(ctx: Arc<RepoContext>) -> Self {
        Self { ctx }
    }

    #[logger::logger]
    pub async fn git_version(&self) -> Result<String, String> {
        let output = self
            .ctx
            .runner
            .run_with_options(&["--version"], GitRunOptions::default_read())
            .await?;

        Ok(output.trim().to_string())
    }

    #[logger::logger]
    pub async fn get_status(&self) -> Result<GetStatusResponse, String> {
        let runner = self.ctx.runner.clone();

        self.ctx
            .cache
            .get_or_refresh(
                CachePolicy {
                    namespace: "status",
                    ttl: TTL_STATUS,
                },
                "porcelain_v2".to_string(),
                move || async move {
                    let output = runner
                        .run_with_options(
                            &["status", "--porcelain=v2", "--untracked-files=all", "-z"],
                            GitRunOptions::default_read(),
                        )
                        .await?;

                    let files = parse_porcelain_v2(output.as_bytes())?;
                    Ok(GetStatusResponse { files })
                },
            )
            .await
    }

    #[logger::logger]
    pub async fn git_fetch(&self) -> Result<String, String> {
        self.ctx
            .runner
            .run_with_options(
                &["fetch", "--prune"],
                GitRunOptions::default_read().with_timeout(std::time::Duration::from_secs(60)),
            )
            .await?;
        self.ctx.cache.invalidate_all();
        Ok("Fetched successfully".to_string())
    }

    #[logger::logger]
    pub async fn git_add(&self, file: &str) -> Result<String, String> {
        if file == "." {
            self.ctx
                .runner
                .run_with_options(
                    &["add", "-A"],
                    GitRunOptions::default_read().with_timeout(std::time::Duration::from_secs(30)),
                )
                .await?;
        } else {
            validate_relative_path(file)?;
            self.ctx
                .runner
                .run_with_options(
                    &["add", "-A", "--", file],
                    GitRunOptions::default_read().with_timeout(std::time::Duration::from_secs(30)),
                )
                .await?;
        }

        self.ctx.cache.invalidate_all();
        Ok("Added".to_string())
    }

    #[logger::logger]
    pub async fn git_remove(&self, file: &str) -> Result<String, String> {
        if file == "." {
            self.ctx
                .runner
                .run_with_options(
                    &["restore", "--staged", "."],
                    GitRunOptions::default_read().with_timeout(std::time::Duration::from_secs(30)),
                )
                .await?;
        } else {
            validate_relative_path(file)?;
            self.ctx
                .runner
                .run_with_options(
                    &["restore", "--staged", "--", file],
                    GitRunOptions::default_read().with_timeout(std::time::Duration::from_secs(30)),
                )
                .await?;
        }

        self.ctx.cache.invalidate_all();
        Ok("Removed".to_string())
    }

    #[logger::logger]
    pub async fn git_discard(&self, file: &str, all: Option<bool>) -> Result<String, String> {
        if all.unwrap_or(false) {
            self.git_restore_all().await?;
            self.ctx.cache.invalidate_all();
            return Ok("All changes discarded".to_string());
        }

        self.git_restore_file(file).await?;
        self.ctx.cache.invalidate_all();
        Ok("Changes discarded".to_string())
    }

    async fn git_restore_all(&self) -> Result<(), String> {
        self.ctx
            .runner
            .run_with_options(
                &["restore", "--source=HEAD", "--staged", "--worktree", "."],
                GitRunOptions::default_read().with_timeout(std::time::Duration::from_secs(30)),
            )
            .await?;

        self.ctx
            .runner
            .run_with_options(
                &["clean", "-fd"],
                GitRunOptions::default_read().with_timeout(std::time::Duration::from_secs(30)),
            )
            .await?;

        Ok(())
    }

    async fn git_restore_file(&self, file: &str) -> Result<(), String> {
        validate_relative_path(file)?;
        let restore_result = self
            .ctx
            .runner
            .run_with_options(
                &[
                    "restore",
                    "--source=HEAD",
                    "--staged",
                    "--worktree",
                    "--",
                    file,
                ],
                GitRunOptions::default_read().with_timeout(std::time::Duration::from_secs(30)),
            )
            .await;

        let clean_result = self
            .ctx
            .runner
            .run_with_options(
                &["clean", "-fd", "--", file],
                GitRunOptions::default_read().with_timeout(std::time::Duration::from_secs(30)),
            )
            .await;

        match (restore_result, clean_result) {
            (Ok(_), _) | (Err(_), Ok(_)) => Ok(()),
            (Err(restore_err), Err(clean_err)) => Err(format!(
                "Failed to discard changes for '{file}': restore failed ({restore_err}) and clean failed ({clean_err})"
            )),
        }
    }
}
