use std::{path::Path, sync::Arc};

use crate::{
    cache::{CachePolicy, TTL_PATCH_BY_FILE_PATH},
    context::RepoContext,
    parsers::stash::validate_stash_ref,
    runner::GitRunOptions,
};

pub struct DiffService {
    ctx: Arc<RepoContext>,
}

impl DiffService {
    pub fn new(ctx: Arc<RepoContext>) -> Self {
        Self { ctx }
    }

    #[logger::logger]
    pub async fn get_patch_by_file_path(
        &self,
        file_path: &str,
        stash_reference: Option<&str>,
    ) -> Result<String, String> {
        if let Some(reference) = stash_reference {
            validate_stash_ref(reference)?;
        }

        let runner = self.ctx.runner.clone();
        let repo_path = self.ctx.repo_path.clone();
        let file_path = file_path.to_string();
        let stash_reference = stash_reference.map(str::to_string);
        let cache_key = match &stash_reference {
            Some(reference) => format!("stash:{reference}:{file_path}"),
            None => format!("worktree:{file_path}"),
        };

        self.ctx
            .cache
            .get_or_refresh(
                CachePolicy {
                    namespace: "patch_by_file_path",
                    ttl: TTL_PATCH_BY_FILE_PATH,
                },
                cache_key,
                move || async move {
                    if let Some(reference) = stash_reference {
                        let out = runner
                            .run_with_options(
                                &[
                                    "stash",
                                    "show",
                                    "-p",
                                    "--no-color",
                                    "--include-untracked",
                                    &reference,
                                ],
                                GitRunOptions::default_read().allow_exit_codes(&[1]),
                            )
                            .await?;

                        return Ok(extract_single_diff_for_path(&out, &file_path));
                    }

                    let out = runner
                        .run_with_options(
                            &[
                                "diff",
                                "--no-ext-diff",
                                "--patch-with-raw",
                                "--no-color",
                                "HEAD",
                                "--",
                                &file_path,
                            ],
                            GitRunOptions::default_read().allow_exit_codes(&[1]),
                        )
                        .await?;

                    if !out.is_empty() {
                        return Ok(out);
                    }

                    let abs = Path::new(&repo_path).join(&file_path);

                    if !abs.exists() {
                        return Ok(String::new());
                    }

                    let out = runner
                        .run_with_options(
                            &[
                                "diff",
                                "--no-index",
                                "--patch-with-raw",
                                "--no-color",
                                "/dev/null",
                                "--",
                                &file_path,
                            ],
                            GitRunOptions::default_read().allow_exit_codes(&[1]),
                        )
                        .await?;

                    Ok(out)
                },
            )
            .await
    }
}

fn extract_single_diff_for_path(output: &str, file_path: &str) -> String {
    let mut starts = Vec::new();

    for (idx, line) in output.lines().enumerate() {
        if line.starts_with("diff --git ") {
            starts.push(idx);
        }
    }

    if starts.len() <= 1 {
        return output.to_string();
    }

    let lines: Vec<&str> = output.lines().collect();
    let mut sections = Vec::new();
    for (i, start) in starts.iter().enumerate() {
        let end = starts.get(i + 1).copied().unwrap_or(lines.len());
        sections.push((*start, end));
    }

    let path_markers = [
        format!("a/{file_path}"),
        format!("b/{file_path}"),
        format!("--- a/{file_path}"),
        format!("+++ b/{file_path}"),
    ];

    let selected = sections
        .iter()
        .find(|(start, end)| {
            lines[*start..*end]
                .iter()
                .any(|line| path_markers.iter().any(|marker| line.contains(marker)))
        })
        .copied()
        .unwrap_or(sections[0]);

    lines[selected.0..selected.1].join("\n")
}
