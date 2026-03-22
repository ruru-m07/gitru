use crate::{
    cache::CachePolicy,
    cache::TTL_STATUS,
    context::RepoContext,
    models::diff::{DiffScope, PatchAction, PatchRange},
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
    pub async fn git_add(
        &self,
        file: Option<&str>,
        files: Option<&[String]>,
    ) -> Result<String, String> {
        if matches!(file, Some(".")) && files.is_none() {
            self.ctx
                .runner
                .run_with_options(
                    &["add", "-A"],
                    GitRunOptions::default_read().with_timeout(std::time::Duration::from_secs(30)),
                )
                .await?;
        } else {
            let paths = normalize_paths(file, files)?;
            self.run_pathspec_command("add", &["-A"], &paths).await?;
        }

        self.ctx.cache.invalidate_all();
        Ok("Added".to_string())
    }

    #[logger::logger]
    pub async fn git_remove(
        &self,
        file: Option<&str>,
        files: Option<&[String]>,
    ) -> Result<String, String> {
        if matches!(file, Some(".")) && files.is_none() {
            self.ctx
                .runner
                .run_with_options(
                    &["restore", "--staged", "."],
                    GitRunOptions::default_read().with_timeout(std::time::Duration::from_secs(30)),
                )
                .await?;
        } else {
            let paths = normalize_paths(file, files)?;
            self.run_pathspec_command("restore", &["--staged"], &paths)
                .await?;
        }

        self.ctx.cache.invalidate_all();
        Ok("Removed".to_string())
    }

    #[logger::logger]
    pub async fn git_discard(
        &self,
        file: Option<&str>,
        files: Option<&[String]>,
        all: Option<bool>,
    ) -> Result<String, String> {
        if all.unwrap_or(false) {
            self.git_restore_all().await?;
            self.ctx.cache.invalidate_all();
            return Ok("All changes discarded".to_string());
        }

        if matches!(file, Some(".")) && files.is_none() {
            self.git_restore_all().await?;
            self.ctx.cache.invalidate_all();
            return Ok("All changes discarded".to_string());
        }

        let paths = normalize_paths(file, files)?;
        self.git_restore_files(&paths).await?;
        self.ctx.cache.invalidate_all();
        Ok("Changes discarded".to_string())
    }

    #[logger::logger]
    pub async fn git_apply_patch_block(
        &self,
        file_path: &str,
        file_new_path: Option<&str>,
        diff_scope: DiffScope,
        additions: PatchRange,
        deletions: PatchRange,
        action: PatchAction,
    ) -> Result<String, String> {
        validate_relative_path(file_path)?;
        if let Some(new_path) = file_new_path {
            validate_relative_path(new_path)?;
        }

        match action {
            PatchAction::Stage => {
                if diff_scope != DiffScope::Unstaged {
                    return Err("Stage requires diff_scope=unstaged".to_string());
                }
            }
            PatchAction::Unstage => {
                if diff_scope != DiffScope::Staged {
                    return Err("Unstage requires diff_scope=staged".to_string());
                }
            }
            PatchAction::Discard => {
                if diff_scope != DiffScope::Unstaged {
                    return Err("Discard requires diff_scope=unstaged".to_string());
                }
            }
        }

        let diff_output = self
            .build_zero_context_diff(file_path, file_new_path, diff_scope)
            .await?;

        if diff_output.trim().is_empty() {
            return Err("No diff available for selected block".to_string());
        }

        let patch = build_patch_for_range(&diff_output, &additions, &deletions)?;

        let mut args = vec!["apply", "--unidiff-zero"];
        match action {
            PatchAction::Stage => {
                args.push("--cached");
            }
            PatchAction::Unstage => {
                args.push("--cached");
                args.push("--reverse");
            }
            PatchAction::Discard => {
                args.push("--reverse");
            }
        }
        args.push("-");

        self.ctx
            .runner
            .run_with_input(&args, &patch, GitRunOptions::default_read())
            .await?;

        self.ctx.cache.invalidate_all();
        Ok("Patch applied".to_string())
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

    async fn git_restore_files(&self, files: &[String]) -> Result<(), String> {
        let mut errors = Vec::new();

        for file in files {
            if let Err(error) = self.git_restore_file(file).await {
                errors.push(error);
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors.join("; "))
        }
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

    async fn run_pathspec_command(
        &self,
        command: &str,
        fixed_args: &[&str],
        paths: &[String],
    ) -> Result<String, String> {
        let mut args = Vec::with_capacity(2 + fixed_args.len() + paths.len());
        args.push(command.to_string());
        args.extend(fixed_args.iter().map(|arg| (*arg).to_string()));
        args.push("--".to_string());
        args.extend(paths.iter().cloned());

        let arg_refs = args.iter().map(String::as_str).collect::<Vec<_>>();
        self.ctx
            .runner
            .run_with_options(
                &arg_refs,
                GitRunOptions::default_read().with_timeout(std::time::Duration::from_secs(30)),
            )
            .await
    }

    async fn build_zero_context_diff(
        &self,
        file_path: &str,
        file_new_path: Option<&str>,
        diff_scope: DiffScope,
    ) -> Result<String, String> {
        if matches!(diff_scope, DiffScope::Worktree) {
            return Err("diff_scope=worktree is not supported for patch actions".to_string());
        }

        let mut args = vec![
            "diff",
            "-U0",
            "--no-color",
            "--no-ext-diff",
        ];
        if diff_scope == DiffScope::Staged {
            args.push("--cached");
        }
        args.push("--");
        args.push(file_path);
        if let Some(new_path) = file_new_path
            && new_path != file_path
        {
            args.push(new_path);
        }

        self.ctx
            .runner
            .run_with_options(&args, GitRunOptions::default_read())
            .await
    }
}

#[derive(Clone)]
struct ParsedHunk {
    old_start: usize,
    old_count: usize,
    new_start: usize,
    new_count: usize,
    lines: Vec<String>,
}

fn build_patch_for_range(
    diff_output: &str,
    additions: &PatchRange,
    deletions: &PatchRange,
) -> Result<String, String> {
    let (header_lines, hunks) = parse_diff_output(diff_output)?;

    if additions.count > 0 && additions.start.is_none() {
        return Err("Additions start line missing".to_string());
    }
    if deletions.count > 0 && deletions.start.is_none() {
        return Err("Deletions start line missing".to_string());
    }

    let target = hunks
        .iter()
        .find(|hunk| hunk_matches_range(hunk, additions, deletions))
        .ok_or_else(|| "Unable to locate selected hunk in diff".to_string())?;

    let mut lines = Vec::new();
    lines.extend(header_lines);
    lines.extend(target.lines.clone());

    Ok(format!("{}\n", lines.join("\n")))
}

fn hunk_matches_range(
    hunk: &ParsedHunk,
    additions: &PatchRange,
    deletions: &PatchRange,
) -> bool {
    let add_count = additions.count;
    let del_count = deletions.count;

    if add_count > 0 && del_count > 0 {
        return hunk.old_start == deletions.start.unwrap_or(0)
            && hunk.old_count == del_count
            && hunk.new_start == additions.start.unwrap_or(0)
            && hunk.new_count == add_count;
    }

    if add_count > 0 && del_count == 0 {
        return hunk.new_start == additions.start.unwrap_or(0)
            && hunk.new_count == add_count
            && hunk.old_count == 0;
    }

    if del_count > 0 && add_count == 0 {
        return hunk.old_start == deletions.start.unwrap_or(0)
            && hunk.old_count == del_count
            && hunk.new_count == 0;
    }

    false
}

fn parse_diff_output(diff_output: &str) -> Result<(Vec<String>, Vec<ParsedHunk>), String> {
    let mut header_lines: Vec<String> = Vec::new();
    let mut hunks: Vec<ParsedHunk> = Vec::new();
    let mut current_hunk: Option<ParsedHunk> = None;
    let mut started = false;

    for line in diff_output.lines() {
        if line.starts_with("diff --git ") {
            if started {
                break;
            }
            started = true;
            header_lines.push(line.to_string());
            continue;
        }

        if !started {
            continue;
        }

        if line.starts_with("@@ ") {
            if let Some(hunk) = current_hunk.take() {
                hunks.push(hunk);
            }
            let (old_start, old_count, new_start, new_count) =
                parse_hunk_header(line).ok_or_else(|| "Invalid hunk header".to_string())?;
            current_hunk = Some(ParsedHunk {
                old_start,
                old_count,
                new_start,
                new_count,
                lines: vec![line.to_string()],
            });
            continue;
        }

        if let Some(hunk) = current_hunk.as_mut() {
            hunk.lines.push(line.to_string());
        } else {
            header_lines.push(line.to_string());
        }
    }

    if let Some(hunk) = current_hunk {
        hunks.push(hunk);
    }

    if header_lines.is_empty() {
        return Err("No diff header found".to_string());
    }
    if hunks.is_empty() {
        return Err("No hunks found in diff".to_string());
    }

    Ok((header_lines, hunks))
}

fn parse_hunk_header(line: &str) -> Option<(usize, usize, usize, usize)> {
    let parts: Vec<&str> = line.split_whitespace().collect();
    if parts.len() < 3 {
        return None;
    }

    let old_range = parts[1].trim_start_matches('-');
    let new_range = parts[2].trim_start_matches('+');

    let (old_start, old_count) = parse_range(old_range)?;
    let (new_start, new_count) = parse_range(new_range)?;

    Some((old_start, old_count, new_start, new_count))
}

fn parse_range(range: &str) -> Option<(usize, usize)> {
    let mut parts = range.split(',');
    let start = parts.next()?.parse::<usize>().ok()?;
    let count = match parts.next() {
        Some(value) => value.parse::<usize>().ok()?,
        None => 1,
    };
    Some((start, count))
}

fn normalize_paths(file: Option<&str>, files: Option<&[String]>) -> Result<Vec<String>, String> {
    let mut normalized = Vec::new();

    if let Some(file) = file {
        validate_relative_path(file)?;
        normalized.push(file.to_string());
    }

    if let Some(files) = files {
        for file in files {
            validate_relative_path(file)?;
            if !normalized.iter().any(|existing| existing == file) {
                normalized.push(file.clone());
            }
        }
    }

    if normalized.is_empty() {
        return Err("At least one file path is required".to_string());
    }

    Ok(normalized)
}
