use std::{path::Path, sync::Arc};

use crate::{
    cache::{CachePolicy, TTL_PATCH_BY_FILE_PATH},
    context::RepoContext,
    parsers::stash::validate_stash_ref,
    runner::GitRunOptions,
};

const EMPTY_TREE_HASH: &str = "4b825dc642cb6eb9a060e54bf8d69288fbee4904";

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
        commit_hash: Option<&str>,
        parent_index: Option<usize>,
    ) -> Result<String, String> {
        if stash_reference.is_some() && commit_hash.is_some() {
            return Err("Cannot request stash and commit diff together".to_string());
        }

        if let Some(reference) = stash_reference {
            validate_stash_ref(reference)?;
        }
        if let Some(hash) = commit_hash {
            validate_commit_hash(hash)?;
        }

        let runner = self.ctx.runner.clone();
        let repo_path = self.ctx.repo_path.clone();
        let file_path = file_path.to_string();
        let stash_reference = stash_reference.map(str::to_string);
        let commit_hash = commit_hash.map(str::to_string);
        let parent_index = parent_index.unwrap_or(1).max(1);
        let cache_key = match &stash_reference {
            Some(reference) => format!("stash:{reference}:{file_path}"),
            None => match &commit_hash {
                Some(hash) => format!("commit:{hash}:p{parent_index}:{file_path}"),
                None => format!("worktree:{file_path}"),
            },
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

                    if let Some(hash) = commit_hash {
                        let base = resolve_commit_diff_base(&runner, &hash, parent_index).await?;
                        let out = runner
                            .run_with_options(
                                &[
                                    "diff",
                                    "--no-ext-diff",
                                    "--patch-with-raw",
                                    "--no-color",
                                    &base,
                                    &hash,
                                    "--",
                                    &file_path,
                                ],
                                GitRunOptions::default_read().allow_exit_codes(&[1]),
                            )
                            .await?;

                        return Ok(out);
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

async fn resolve_commit_diff_base(
    runner: &crate::runner::GitCommandRunner,
    commit_hash: &str,
    parent_index: usize,
) -> Result<String, String> {
    let parents = runner
        .run_with_options(
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
