use std::{path::Path, sync::Arc};

use crate::{context::RepoContext, runner::GitRunOptions};

pub struct DiffService {
    ctx: Arc<RepoContext>,
}

impl DiffService {
    pub fn new(ctx: Arc<RepoContext>) -> Self {
        Self { ctx }
    }

    #[logger::logger]
    pub async fn get_patch_by_file_path(&self, file_path: &str) -> Result<String, String> {
        let out = self
            .ctx
            .runner
            .run_with_options(
                &[
                    "diff",
                    "--no-ext-diff",
                    "--patch-with-raw",
                    "--no-color",
                    "HEAD",
                    "--",
                    file_path,
                ],
                GitRunOptions::default_read().allow_exit_codes(&[1]),
            )
            .await?;

        if !out.is_empty() {
            return Ok(out);
        }

        let abs = Path::new(&self.ctx.repo_path).join(file_path);

        if !abs.exists() {
            return Ok(String::new());
        }

        let out = self
            .ctx
            .runner
            .run_with_options(
                &[
                    "diff",
                    "--no-index",
                    "--patch-with-raw",
                    "--no-color",
                    "/dev/null",
                    "--",
                    file_path,
                ],
                GitRunOptions::default_read().allow_exit_codes(&[1]),
            )
            .await?;

        Ok(out)
    }
}
