import type { FileStatus, GetStatusResponse } from "@gitru/commands";

/**
 * Path on disk / for `git add` and worktree restore.
 * For renames, `path` is the old location and `new_path` is the current file.
 */
export function getGitStagePathspec(file: FileStatus): string {
  return file.new_path ?? file.path;
}

export function getGitStagePathspecs(
  files: GetStatusResponse["files"],
): string[] {
  return Array.from(new Set(files.map(getGitStagePathspec)));
}

/**
 * Paths for `git restore --staged`. Renames are two index entries (delete old,
 * add new), so both paths must be passed to fully unstage.
 */
export function getGitUnstagePathspecs(file: FileStatus): string[] {
  if (file.new_path) {
    return [file.path, file.new_path];
  }
  return [file.path];
}

export function getGitUnstagePathspecsForFiles(
  files: GetStatusResponse["files"],
): string[] {
  return Array.from(new Set(files.flatMap(getGitUnstagePathspecs)));
}

/** @deprecated Use getGitStagePathspec for staging/discard or getGitUnstagePathspecs for unstaging. */
export function getGitPathspec(file: FileStatus): string {
  return getGitStagePathspec(file);
}

/** @deprecated Use getGitStagePathspecs or getGitUnstagePathspecsForFiles. */
export function getGitPathspecs(files: GetStatusResponse["files"]): string[] {
  return getGitStagePathspecs(files);
}
