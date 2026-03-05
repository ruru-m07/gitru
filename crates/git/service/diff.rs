use std::{
    fs,
    path::{Path, PathBuf},
    sync::Arc,
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use crate::{
    cache::{CachePolicy, TTL_PATCH_BY_FILE_PATH},
    context::RepoContext,
    models::{
        diff::{AssetDiff, AssetDiffEntry, AssetDiffKind, FileDiff},
        status::FileStatusKind,
    },
    parsers::stash::validate_stash_ref,
    runner::GitRunOptions,
};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};

const EMPTY_TREE_HASH: &str = "4b825dc642cb6eb9a060e54bf8d69288fbee4904";
const IMAGE_DIFF_TEMP_DIR: &str = "gitru-image-diff";
/// Maximum image size (in bytes) to include as base64 in the response (~10 MB).
const MAX_ASSET_INLINE_BYTES: usize = 10 * 1024 * 1024;
/// Maximum age of temp files before they are eligible for cleanup (1 hour).
const TEMP_FILE_MAX_AGE_SECS: u64 = 3600;
static TEMP_FILE_COUNTER: AtomicU64 = AtomicU64::new(0);

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
        file_new_path: Option<&str>,
        status: Option<&[FileStatusKind]>,
        stash_reference: Option<&str>,
        commit_hash: Option<&str>,
        parent_index: Option<usize>,
    ) -> Result<FileDiff, String> {
        if stash_reference.is_some() && commit_hash.is_some() {
            return Err("Cannot request stash and commit diff together".to_string());
        }

        if let Some(reference) = stash_reference {
            validate_stash_ref(reference)?;
        }
        if let Some(hash) = commit_hash {
            validate_commit_hash(hash)?;
        }

        let patch = self
            .get_patch_text_by_file_path(file_path, stash_reference, commit_hash, parent_index)
            .await?;

        let asset_diff = self
            .resolve_asset_diff(
                file_path,
                file_new_path,
                status,
                stash_reference,
                commit_hash,
                parent_index,
                &patch,
            )
            .await?;

        Ok(FileDiff { patch, asset_diff })
    }

    async fn get_patch_text_by_file_path(
        &self,
        file_path: &str,
        stash_reference: Option<&str>,
        commit_hash: Option<&str>,
        parent_index: Option<usize>,
    ) -> Result<String, String> {
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

    #[allow(clippy::too_many_arguments)]
    async fn resolve_asset_diff(
        &self,
        file_path: &str,
        file_new_path: Option<&str>,
        status: Option<&[FileStatusKind]>,
        stash_reference: Option<&str>,
        commit_hash: Option<&str>,
        parent_index: Option<usize>,
        patch: &str,
    ) -> Result<Option<AssetDiff>, String> {
        if stash_reference.is_some() {
            return Ok(None);
        }

        let status = status.unwrap_or(&[]);
        let is_new = status
            .iter()
            .any(|s| matches!(s, FileStatusKind::IndexNew | FileStatusKind::WorktreeNew));
        let is_deleted = status.iter().any(|s| {
            matches!(
                s,
                FileStatusKind::IndexDeleted | FileStatusKind::WorktreeDeleted
            )
        });
        let is_renamed = status.iter().any(|s| {
            matches!(
                s,
                FileStatusKind::IndexRenamed | FileStatusKind::WorktreeRenamed
            )
        });

        let logical_before = file_path.to_string();
        let logical_after = file_new_path.unwrap_or(file_path).to_string();
        let extension_path = file_new_path.unwrap_or(file_path);

        let is_image = is_supported_image_extension(extension_path);
        let binary_hint = patch.contains("Binary files") || patch.contains("GIT binary patch");

        if !is_image {
            if binary_hint {
                return Ok(Some(AssetDiff {
                    kind: AssetDiffKind::Binary,
                    before: None,
                    after: None,
                }));
            }
            return Ok(None);
        }

        let before_should_exist = !is_new;
        let after_should_exist = !is_deleted;
        let effective_before_path = if is_renamed {
            logical_before.as_str()
        } else {
            file_path
        };
        let effective_after_path = logical_after.as_str();

        let before = if before_should_exist {
            if let Some(hash) = commit_hash {
                let base = resolve_commit_diff_base(
                    &self.ctx.runner,
                    hash,
                    parent_index.unwrap_or(1).max(1),
                )
                .await?;
                self.load_blob_entry(&base, effective_before_path, &logical_before)
                    .await
            } else {
                self.load_blob_entry("HEAD", effective_before_path, &logical_before)
                    .await
            }
        } else {
            None
        };

        let after = if after_should_exist {
            if let Some(hash) = commit_hash {
                self.load_blob_entry(hash, effective_after_path, &logical_after)
                    .await
            } else {
                self.load_worktree_entry(effective_after_path, &logical_after)
                    .await
            }
        } else {
            None
        };

        if before.is_none() && after.is_none() {
            return Ok(None);
        }

        Ok(Some(AssetDiff {
            kind: AssetDiffKind::Image,
            before,
            after,
        }))
    }

    async fn load_blob_entry(
        &self,
        rev: &str,
        rev_path: &str,
        logical_path: &str,
    ) -> Option<AssetDiffEntry> {
        let spec = format!("{rev}:{rev_path}");
        let bytes = self
            .ctx
            .runner
            .run_with_options_bytes(
                &["show", "--no-ext-diff", "--no-textconv", &spec],
                GitRunOptions::default_read().allow_exit_codes(&[1, 128]),
            )
            .await
            .ok()?;

        if bytes.is_empty() {
            return None;
        }

        if bytes.len() > MAX_ASSET_INLINE_BYTES {
            return None;
        }

        let temp_path = write_temp_asset_file(logical_path, &bytes).ok()?;
        Some(AssetDiffEntry {
            absolute_path: temp_path.to_string_lossy().to_string(),
            mime: mime_for_path(logical_path),
            bytes: bytes.len(),
            logical_path: logical_path.to_string(),
            contents_base64: BASE64_STANDARD.encode(&bytes),
        })
    }

    async fn load_worktree_entry(
        &self,
        repo_relative_path: &str,
        logical_path: &str,
    ) -> Option<AssetDiffEntry> {
        let absolute = Path::new(&self.ctx.repo_path).join(repo_relative_path);
        let bytes = fs::read(&absolute).ok()?;
        if bytes.is_empty() {
            return None;
        }

        Some(AssetDiffEntry {
            absolute_path: absolute.to_string_lossy().to_string(),
            mime: mime_for_path(logical_path),
            bytes: bytes.len(),
            logical_path: logical_path.to_string(),
            contents_base64: BASE64_STANDARD.encode(&bytes),
        })
    }
}

fn is_supported_image_extension(path: &str) -> bool {
    let ext = Path::new(path)
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase());
    matches!(
        ext.as_deref(),
        Some("png")
            | Some("jpg")
            | Some("jpeg")
            | Some("webp")
            | Some("gif")
            | Some("bmp")
            | Some("ico")
            | Some("avif")
    )
}

fn mime_for_path(path: &str) -> String {
    let ext = Path::new(path)
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase());

    match ext.as_deref() {
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("webp") => "image/webp",
        Some("gif") => "image/gif",
        Some("bmp") => "image/bmp",
        Some("ico") => "image/x-icon",
        Some("avif") => "image/avif",
        _ => "application/octet-stream",
    }
    .to_string()
}

fn write_temp_asset_file(logical_path: &str, bytes: &[u8]) -> Result<PathBuf, String> {
    let root = std::env::temp_dir().join(IMAGE_DIFF_TEMP_DIR);
    fs::create_dir_all(&root).map_err(|e| e.to_string())?;

    // Clean up old temp files to prevent unbounded disk usage.
    if let Ok(entries) = fs::read_dir(&root)
        && let Some(cutoff) =
            SystemTime::now().checked_sub(std::time::Duration::from_secs(TEMP_FILE_MAX_AGE_SECS))
    {
        for entry in entries.flatten() {
            if let Ok(meta) = entry.metadata()
                && meta.is_file()
                && let Ok(modified) = meta.modified()
                && modified < cutoff
            {
                let _ = fs::remove_file(entry.path());
            }
        }
    }

    let ext = Path::new(logical_path)
        .extension()
        .map(|e| e.to_string_lossy().to_string())
        .unwrap_or_else(|| "bin".to_string());
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_nanos();
    let count = TEMP_FILE_COUNTER.fetch_add(1, Ordering::Relaxed);
    let file_name = format!("asset-{now}-{count}.{ext}");
    let output = root.join(file_name);
    fs::write(&output, bytes).map_err(|e| e.to_string())?;
    Ok(output)
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
