use std::{
    fs,
    path::{Path, PathBuf},
    sync::Arc,
};

use git2::RepositoryState;

use crate::{
    context::RepoContext,
    models::{
        operation::{RebaseEngine, RepoOperation, RepoOperationKind},
        rebase::{
            RebaseAction, RebasePauseReason, RebaseTodoEntry, RebaseTodoStatus,
        },
    },
};

pub const GITRU_REBASE_DIR: &str = "gitru-rebase";

pub struct OperationService {
    ctx: Arc<RepoContext>,
}

impl OperationService {
    pub fn new(ctx: Arc<RepoContext>) -> Self {
        Self { ctx }
    }

    pub fn git_dir(&self) -> Result<PathBuf, String> {
        let repo = git2::Repository::open(&self.ctx.repo_path)
            .map_err(|e| format!("Failed to open repository: {e}"))?;
        Ok(repo.path().to_path_buf())
    }

    pub fn gitru_rebase_dir(&self) -> Result<PathBuf, String> {
        Ok(self.git_dir()?.join(GITRU_REBASE_DIR))
    }

    /// Detect the current in-progress operation (clean, rebase, merge, …).
    pub fn get_repo_operation(&self) -> Result<RepoOperation, String> {
        let repo = git2::Repository::open(&self.ctx.repo_path)
            .map_err(|e| format!("Failed to open repository: {e}"))?;

        let gitru_dir = repo.path().join(GITRU_REBASE_DIR);
        if gitru_dir.is_dir() && gitru_dir.join("onto").is_file() {
            return self.read_gitru_operation(&repo, &gitru_dir);
        }

        let kind = map_repository_state(repo.state());
        if !matches!(
            kind,
            RepoOperationKind::Rebase
                | RepoOperationKind::RebaseInteractive
                | RepoOperationKind::RebaseMerge
        ) {
            let mut op = RepoOperation::clean();
            op.kind = kind;
            if !matches!(op.kind, RepoOperationKind::Clean) {
                op.conflict_paths = conflict_paths_from_index(&repo)?;
            }
            return Ok(op);
        }

        self.read_native_rebase_operation(&repo, kind)
    }

    fn read_gitru_operation(
        &self,
        repo: &git2::Repository,
        dir: &Path,
    ) -> Result<RepoOperation, String> {
        let onto = read_trimmed(dir.join("onto")).ok();
        let head_name = read_trimmed(dir.join("head-name")).ok();
        let orig_head = read_trimmed(dir.join("orig-head")).ok();
        let paused_at = read_trimmed(dir.join("paused-at")).ok();
        let pause_reason = read_trimmed(dir.join("pause-reason"))
            .ok()
            .and_then(|s| parse_pause_reason(&s));

        let todo = parse_gitru_todo(dir)?;
        let current = todo
            .iter()
            .position(|e| e.status == RebaseTodoStatus::Current)
            .map(|i| (i + 1) as u32)
            .or_else(|| {
                let done = todo
                    .iter()
                    .filter(|e| e.status == RebaseTodoStatus::Done)
                    .count();
                if done > 0 && done < todo.len() {
                    Some((done + 1) as u32)
                } else if done == todo.len() && !todo.is_empty() {
                    Some(todo.len() as u32)
                } else {
                    None
                }
            });
        let total = Some(todo.len() as u32);
        let remaining = current.map(|c| total.unwrap_or(0).saturating_sub(c));

        let label = match (&head_name, &onto) {
            (Some(h), Some(o)) => Some(format!(
                "{} onto {}",
                shorten_ref(h),
                short_oid(o)
            )),
            _ => None,
        };

        let conflict_paths = conflict_paths_from_index(repo)?;
        let commit_message = resolve_commit_message(repo, dir, &todo, paused_at.as_deref());

        Ok(RepoOperation {
            kind: RepoOperationKind::RebaseInteractive,
            is_rebasing: true,
            engine: Some(RebaseEngine::Gitru),
            head_name,
            onto,
            paused_at,
            pause_reason,
            current,
            total,
            remaining,
            label,
            orig_head,
            commit_message,
            todo,
            conflict_paths,
        })
    }

    fn read_native_rebase_operation(
        &self,
        repo: &git2::Repository,
        kind: RepoOperationKind,
    ) -> Result<RepoOperation, String> {
        let git_dir = repo.path();
        let merge_dir = git_dir.join("rebase-merge");
        let apply_dir = git_dir.join("rebase-apply");
        let dir = if merge_dir.is_dir() {
            merge_dir
        } else if apply_dir.is_dir() {
            apply_dir
        } else {
            git_dir.to_path_buf()
        };

        let head_name = read_trimmed(dir.join("head-name")).ok();
        let onto = read_trimmed(dir.join("onto")).ok();
        let orig_head = read_trimmed(git_dir.join("ORIG_HEAD"))
            .ok()
            .or_else(|| read_trimmed(dir.join("orig-head")).ok());

        let msgnum = read_trimmed(dir.join("msgnum"))
            .ok()
            .and_then(|s| s.parse::<u32>().ok());
        let end = read_trimmed(dir.join("end"))
            .ok()
            .and_then(|s| s.parse::<u32>().ok());

        let paused_at = read_trimmed(git_dir.join("REBASE_HEAD"))
            .ok()
            .or_else(|| read_trimmed(dir.join("stopped-sha")).ok())
            .or_else(|| read_trimmed(dir.join("current")).ok());

        let todo = parse_native_todo(&dir, msgnum)?;

        let remaining = match (msgnum, end) {
            (Some(c), Some(t)) => Some(t.saturating_sub(c)),
            _ => None,
        };

        let conflict_paths = conflict_paths_from_index(repo)?;
        let pause_reason = if !conflict_paths.is_empty() {
            Some(RebasePauseReason::Conflict)
        } else if dir.join("message").is_file() || dir.join("amend").is_file() {
            Some(RebasePauseReason::Reword)
        } else {
            Some(RebasePauseReason::Waiting)
        };

        let label = match (&head_name, &onto) {
            (Some(h), Some(o)) => Some(format!(
                "{} onto {}",
                shorten_ref(h),
                short_oid(o)
            )),
            _ => None,
        };

        let commit_message = resolve_commit_message(repo, &dir, &todo, paused_at.as_deref())
            .or_else(|| {
                // Native rebase also writes COMMIT_EDITMSG
                read_message_file(git_dir.join("COMMIT_EDITMSG"))
            });

        Ok(RepoOperation {
            kind,
            is_rebasing: true,
            engine: Some(RebaseEngine::Git),
            head_name,
            onto,
            paused_at,
            pause_reason,
            current: msgnum,
            total: end,
            remaining,
            label,
            orig_head,
            commit_message,
            todo,
            conflict_paths,
        })
    }
}

/// Prefer on-disk rebase message, then current todo subject, then commit object.
fn resolve_commit_message(
    repo: &git2::Repository,
    rebase_dir: &Path,
    todo: &[RebaseTodoEntry],
    paused_at: Option<&str>,
) -> Option<String> {
    if let Some(msg) = read_message_file(rebase_dir.join("message")) {
        return Some(msg);
    }

    let current = todo.iter().find(|e| e.status == RebaseTodoStatus::Current);
    if let Some(entry) = current {
        if !entry.message.trim().is_empty() {
            // Prefer full message from the commit object when possible.
            if let Some(full) = commit_message_for_oid(repo, &entry.commit) {
                return Some(full);
            }
            return Some(entry.message.clone());
        }
        if let Some(full) = commit_message_for_oid(repo, &entry.commit) {
            return Some(full);
        }
    }

    if let Some(oid) = paused_at {
        if let Some(full) = commit_message_for_oid(repo, oid) {
            return Some(full);
        }
    }

    None
}

fn read_message_file(path: PathBuf) -> Option<String> {
    let raw = fs::read_to_string(path).ok()?;
    let trimmed = raw.trim_end_matches('\n').trim_end_matches('\r');
    // Strip comment lines git adds to COMMIT_EDITMSG templates
    let cleaned: String = trimmed
        .lines()
        .filter(|line| !line.starts_with('#'))
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string();
    if cleaned.is_empty() {
        None
    } else {
        Some(cleaned)
    }
}

fn commit_message_for_oid(repo: &git2::Repository, oid_str: &str) -> Option<String> {
    let oid = git2::Oid::from_str(oid_str.trim()).ok()?;
    let commit = repo.find_commit(oid).ok()?;
    let msg = commit.message()?.trim();
    if msg.is_empty() {
        None
    } else {
        Some(msg.to_string())
    }
}

fn map_repository_state(state: RepositoryState) -> RepoOperationKind {
    match state {
        RepositoryState::Clean => RepoOperationKind::Clean,
        RepositoryState::Merge => RepoOperationKind::Merge,
        RepositoryState::Revert | RepositoryState::RevertSequence => {
            RepoOperationKind::Revert
        }
        RepositoryState::CherryPick | RepositoryState::CherryPickSequence => {
            RepoOperationKind::CherryPick
        }
        RepositoryState::Bisect => RepoOperationKind::Bisect,
        RepositoryState::Rebase => RepoOperationKind::Rebase,
        RepositoryState::RebaseInteractive => RepoOperationKind::RebaseInteractive,
        RepositoryState::RebaseMerge => RepoOperationKind::RebaseMerge,
        RepositoryState::ApplyMailbox | RepositoryState::ApplyMailboxOrRebase => {
            RepoOperationKind::ApplyMailbox
        }
    }
}

pub fn conflict_paths_from_index(repo: &git2::Repository) -> Result<Vec<String>, String> {
    let index = repo
        .index()
        .map_err(|e| format!("Failed to read index: {e}"))?;
    let mut paths = Vec::new();
    let conflicts = index
        .conflicts()
        .map_err(|e| format!("Failed to read conflicts: {e}"))?;
    for conflict in conflicts {
        let conflict = conflict.map_err(|e| format!("Conflict entry error: {e}"))?;
        let path_bytes = conflict
            .our
            .as_ref()
            .or(conflict.their.as_ref())
            .or(conflict.ancestor.as_ref())
            .map(|e| e.path.as_slice())
            .unwrap_or(b"");
        if let Ok(path) = std::str::from_utf8(path_bytes) {
            if !path.is_empty() && !paths.iter().any(|p| p == path) {
                paths.push(path.to_string());
            }
        }
    }
    Ok(paths)
}

pub fn read_trimmed(path: PathBuf) -> Result<String, String> {
    let raw = fs::read_to_string(&path)
        .map_err(|e| format!("Failed to read {}: {e}", path.display()))?;
    Ok(raw.trim().to_string())
}

fn short_oid(oid: &str) -> String {
    oid.chars().take(7).collect()
}

fn shorten_ref(name: &str) -> String {
    name.strip_prefix("refs/heads/")
        .or_else(|| name.strip_prefix("refs/remotes/"))
        .unwrap_or(name)
        .to_string()
}

fn parse_pause_reason(s: &str) -> Option<RebasePauseReason> {
    match s.trim().to_ascii_lowercase().as_str() {
        "conflict" => Some(RebasePauseReason::Conflict),
        "edit" => Some(RebasePauseReason::Edit),
        "reword" => Some(RebasePauseReason::Reword),
        "waiting" => Some(RebasePauseReason::Waiting),
        _ => None,
    }
}

fn parse_gitru_todo(dir: &Path) -> Result<Vec<RebaseTodoEntry>, String> {
    let todo_path = dir.join("todo");
    if !todo_path.is_file() {
        return Ok(Vec::new());
    }
    let raw = fs::read_to_string(&todo_path).map_err(|e| e.to_string())?;
    let current_idx = read_trimmed(dir.join("current-index"))
        .ok()
        .and_then(|s| s.parse::<u32>().ok());

    let mut entries = Vec::new();
    for (i, line) in raw.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some(entry) = parse_todo_line(i as u32, line) else {
            continue;
        };
        let status = if current_idx == Some(i as u32) {
            RebaseTodoStatus::Current
        } else if current_idx.is_some_and(|c| (i as u32) < c) {
            RebaseTodoStatus::Done
        } else {
            RebaseTodoStatus::Pending
        };
        entries.push(RebaseTodoEntry {
            status,
            ..entry
        });
    }
    Ok(entries)
}

fn parse_native_todo(
    dir: &Path,
    msgnum: Option<u32>,
) -> Result<Vec<RebaseTodoEntry>, String> {
    let mut entries = Vec::new();
    let mut index = 0u32;

    let done_path = dir.join("done");
    if done_path.is_file() {
        let raw = fs::read_to_string(&done_path).unwrap_or_default();
        for line in raw.lines() {
            if let Some(mut entry) = parse_todo_line(index, line) {
                entry.status = RebaseTodoStatus::Done;
                entries.push(entry);
                index += 1;
            }
        }
    }

    let todo_path = dir.join("git-rebase-todo");
    if todo_path.is_file() {
        let raw = fs::read_to_string(&todo_path).unwrap_or_default();
        for line in raw.lines() {
            if let Some(mut entry) = parse_todo_line(index, line) {
                let is_current = msgnum.is_some_and(|m| m == index + 1);
                entry.status = if is_current {
                    RebaseTodoStatus::Current
                } else {
                    RebaseTodoStatus::Pending
                };
                entries.push(entry);
                index += 1;
            }
        }
    }

    Ok(entries)
}

fn parse_todo_line(index: u32, line: &str) -> Option<RebaseTodoEntry> {
    let line = line.trim();
    if line.is_empty() || line.starts_with('#') || line.starts_with("exec") {
        return None;
    }
    let mut parts = line.splitn(3, char::is_whitespace);
    let action_str = parts.next()?;
    let action = RebaseAction::parse(action_str)?;
    let commit = parts.next()?.to_string();
    // Native `done` lines often look like: `pick <oid> # <subject>`
    let message = parts
        .next()
        .unwrap_or("")
        .trim()
        .trim_start_matches('#')
        .trim()
        .to_string();
    let short_commit: String = commit.chars().take(7).collect();
    Some(RebaseTodoEntry {
        index,
        action,
        commit,
        short_commit,
        message,
        status: RebaseTodoStatus::Pending,
        authored_at: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_todo_line_pick() {
        let e = parse_todo_line(0, "pick abcdef1 feat: hello world").unwrap();
        assert_eq!(e.action, RebaseAction::Pick);
        assert_eq!(e.commit, "abcdef1");
        assert_eq!(e.message, "feat: hello world");
    }

    #[test]
    fn parse_todo_line_strips_hash_comment() {
        let e = parse_todo_line(
            0,
            "pick abcdef1 # feat: friendlier greet on feature",
        )
        .unwrap();
        assert_eq!(e.message, "feat: friendlier greet on feature");
    }

    #[test]
    fn read_message_file_strips_conflict_comments() {
        let dir = std::env::temp_dir().join(format!(
            "gitru-msg-test-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("message");
        fs::write(
            &path,
            "feat: friendlier greet on feature\n\n# Conflicts:\n#\tapp.js\n",
        )
        .unwrap();
        let msg = read_message_file(path).unwrap();
        assert_eq!(msg, "feat: friendlier greet on feature");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn map_states() {
        assert_eq!(
            map_repository_state(RepositoryState::Clean),
            RepoOperationKind::Clean
        );
        assert_eq!(
            map_repository_state(RepositoryState::RebaseInteractive),
            RepoOperationKind::RebaseInteractive
        );
    }
}
