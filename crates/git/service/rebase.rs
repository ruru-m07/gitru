use std::{
    fs,
    path::Path,
    sync::Arc,
    time::Duration,
};

use git2::{
    AnnotatedCommit, CherrypickOptions, Oid, Repository, ResetType, Signature,
};
use tauri::Emitter;

use crate::{
    context::RepoContext,
    models::{
        operation::{RebaseEngine, RepoOperation, RepoOperationKind},
        rebase::{
            ConflictResolveRequest, ConflictResolveStrategy, RebaseAbortPreview, RebaseAction,
            RebasePauseReason, RebasePlan, RebasePlanEntry, RebaseProgressEvent,
            RebaseProgressPhase, RebaseStartRequest,
        },
    },
    runner::GitRunOptions,
    service::operation::{
        conflict_paths_from_index, read_trimmed, OperationService, GITRU_REBASE_DIR,
    },
};

/// Noninteractive editors so `git rebase --continue` never waits on a TTY editor.
/// Without these, continue holds the per-repo command lock until timeout and starves
/// status/branch queries (empty Changes panel / broken status bar until restart).
const REBASE_NONINTERACTIVE_ENV: &[(&str, &str)] = &[
    ("GIT_EDITOR", "true"),
    ("GIT_SEQUENCE_EDITOR", "true"),
    ("EDITOR", "true"),
    ("VISUAL", "true"),
];

pub const REBASE_PROGRESS_EVENT: &str = "git://rebase-progress";

pub struct RebaseService {
    ctx: Arc<RepoContext>,
}

impl RebaseService {
    pub fn new(ctx: Arc<RepoContext>) -> Self {
        Self { ctx }
    }

    pub fn operation(&self) -> OperationService {
        OperationService::new(self.ctx.clone())
    }

    pub fn get_repo_operation(&self) -> Result<RepoOperation, String> {
        self.operation().get_repo_operation()
    }

    /// Build a default pick-all plan for commits reachable from HEAD but not from upstream.
    pub fn plan_rebase(&self, onto: &str, upstream: Option<&str>) -> Result<RebasePlan, String> {
        let repo = open_repo(&self.ctx.repo_path)?;
        let upstream_ref = upstream.unwrap_or(onto);
        let onto_oid = revparse_oid(&repo, onto)?;
        let upstream_oid = revparse_oid(&repo, upstream_ref)?;
        let head = repo
            .head()
            .map_err(|e| format!("Failed to read HEAD: {e}"))?;
        let head_oid = head
            .target()
            .ok_or_else(|| "HEAD is unborn".to_string())?;

        let mut revwalk = repo
            .revwalk()
            .map_err(|e| format!("Failed to create revwalk: {e}"))?;
        revwalk
            .push(head_oid)
            .map_err(|e| format!("Failed to push HEAD: {e}"))?;
        revwalk
            .hide(upstream_oid)
            .map_err(|e| format!("Failed to hide upstream: {e}"))?;
        revwalk
            .set_sorting(git2::Sort::TOPOLOGICAL | git2::Sort::REVERSE)
            .map_err(|e| format!("Failed to set sorting: {e}"))?;

        let mut entries = Vec::new();
        for oid_result in revwalk {
            let oid = oid_result.map_err(|e| format!("Revwalk error: {e}"))?;
            let commit = repo
                .find_commit(oid)
                .map_err(|e| format!("Failed to find commit: {e}"))?;
            let message = commit
                .summary()
                .unwrap_or("")
                .to_string();
            entries.push(RebasePlanEntry {
                action: RebaseAction::Pick,
                commit: oid.to_string(),
                message: Some(message),
            });
        }

        Ok(RebasePlan {
            onto: onto_oid.to_string(),
            upstream: Some(upstream_oid.to_string()),
            entries,
            autostash: false,
        })
    }

    pub async fn start(
        &self,
        request: RebaseStartRequest,
        app: Option<tauri::AppHandle>,
    ) -> Result<RepoOperation, String> {
        let path = self.ctx.repo_path.clone();
        let result = tokio::task::spawn_blocking(move || {
            start_rebase_blocking(&path, request, app)
        })
        .await
        .map_err(|e| format!("Rebase task join error: {e}"))?;

        self.ctx.cache.invalidate_all();
        result
    }

    pub async fn continue_rebase(
        &self,
        message: Option<String>,
        app: Option<tauri::AppHandle>,
    ) -> Result<RepoOperation, String> {
        let op = self.get_repo_operation()?;
        match op.engine {
            Some(RebaseEngine::Gitru) => {
                let path = self.ctx.repo_path.clone();
                let result = tokio::task::spawn_blocking(move || {
                    continue_gitru_blocking(&path, message, app)
                })
                .await
                .map_err(|e| format!("Rebase continue join error: {e}"))?;
                self.ctx.cache.invalidate_all();
                result
            }
            Some(RebaseEngine::Git) | None => {
                // Interactive rebases (todo / reword / edit) must use the CLI — libgit2's
                // open_rebase does not drive the interactive sequencer correctly.
                let use_cli = matches!(op.kind, RepoOperationKind::RebaseInteractive)
                    || self.is_native_interactive_rebase();

                if !use_cli {
                    let path = self.ctx.repo_path.clone();
                    let msg = message.clone();
                    let git2_result = tokio::task::spawn_blocking(move || {
                        continue_native_git2_blocking(&path, msg)
                    })
                    .await
                    .map_err(|e| format!("Rebase continue join error: {e}"))?;

                    if git2_result.is_ok() {
                        self.ctx.cache.invalidate_all();
                        return git2_result;
                    }
                }

                // Exit code 1 is normal when continue pauses again on conflicts.
                let cli_result = self
                    .cli_rebase_command(&["rebase", "--continue"], message.as_deref())
                    .await;
                self.ctx.cache.invalidate_all();
                match cli_result {
                    Ok(_) => self.get_repo_operation(),
                    Err(e) if e.contains("timed out") => Err(e),
                    Err(e) => {
                        // Conflict / editor pause: rebase still in progress — surface state.
                        let op = self.get_repo_operation()?;
                        if op.is_rebasing {
                            Ok(op)
                        } else {
                            Err(e)
                        }
                    }
                }
            }
        }
    }

    pub async fn skip(&self, app: Option<tauri::AppHandle>) -> Result<RepoOperation, String> {
        let op = self.get_repo_operation()?;
        match op.engine {
            Some(RebaseEngine::Gitru) => {
                let path = self.ctx.repo_path.clone();
                let result = tokio::task::spawn_blocking(move || skip_gitru_blocking(&path, app))
                    .await
                    .map_err(|e| format!("Rebase skip join error: {e}"))?;
                self.ctx.cache.invalidate_all();
                result
            }
            Some(RebaseEngine::Git) | None => {
                self.cli_rebase_command(&["rebase", "--skip"], None).await?;
                self.ctx.cache.invalidate_all();
                self.get_repo_operation()
            }
        }
    }

    pub async fn abort(&self, app: Option<tauri::AppHandle>) -> Result<RepoOperation, String> {
        let op = self.get_repo_operation()?;
        match op.engine {
            Some(RebaseEngine::Gitru) => {
                let path = self.ctx.repo_path.clone();
                let result = tokio::task::spawn_blocking(move || abort_gitru_blocking(&path, app))
                    .await
                    .map_err(|e| format!("Rebase abort join error: {e}"))?;
                self.ctx.cache.invalidate_all();
                result
            }
            Some(RebaseEngine::Git) | None => {
                self.cli_rebase_command(&["rebase", "--abort"], None).await?;
                self.ctx.cache.invalidate_all();
                self.get_repo_operation()
            }
        }
    }

    pub fn abort_preview(&self) -> Result<RebaseAbortPreview, String> {
        let op = self.get_repo_operation()?;
        if !op.is_rebasing {
            return Err("No rebase in progress".to_string());
        }
        let commits_applied = op
            .current
            .map(|c| c.saturating_sub(1))
            .unwrap_or(0);
        Ok(RebaseAbortPreview {
            orig_head: op.orig_head.clone(),
            head_name: op.head_name.clone(),
            onto: op.onto.clone(),
            current: op.current,
            total: op.total,
            commits_applied,
            warning: format!(
                "Aborting will discard {} applied commit(s) from this rebase and return to {}.",
                commits_applied,
                op.orig_head
                    .as_deref()
                    .map(|h| {
                        let short: String = h.chars().take(7).collect();
                        short
                    })
                    .unwrap_or_else(|| "the pre-rebase HEAD".to_string())
            ),
        })
    }

    pub fn update_todo(&self, entries: Vec<RebasePlanEntry>) -> Result<RepoOperation, String> {
        let gitru_dir = self.operation().gitru_rebase_dir()?;
        if gitru_dir.is_dir() {
            validate_todo(&entries)?;
            write_gitru_todo(&gitru_dir, &entries)?;
            return self.get_repo_operation();
        }

        let git_dir = self.operation().git_dir()?;
        let merge_dir = git_dir.join("rebase-merge");
        if !merge_dir.join("interactive").is_file() {
            return Err("Todo editing requires an interactive rebase".into());
        }

        // `done` already contains applied commits (including the conflicted current
        // step). Only rewrite the remaining `git-rebase-todo` suffix.
        let done_count = count_native_todo_lines(&merge_dir.join("done"));
        if entries.len() < done_count {
            return Err("Todo entry count is shorter than applied commits".into());
        }
        let remaining = &entries[done_count..];
        if remaining.is_empty() {
            return Err("No remaining commits to edit in the rebase todo".into());
        }
        write_native_todo(&merge_dir, remaining)?;
        self.get_repo_operation()
    }

    pub fn set_commit_message(&self, message: &str) -> Result<(), String> {
        let dir = self.operation().gitru_rebase_dir()?;
        if dir.is_dir() {
            fs::write(dir.join("message"), message)
                .map_err(|e| format!("Failed to write message: {e}"))?;
            return Ok(());
        }
        // Native rebase message file
        let git_dir = self.operation().git_dir()?;
        let merge_msg = git_dir.join("rebase-merge").join("message");
        if merge_msg.parent().is_some_and(|p| p.is_dir()) {
            fs::write(&merge_msg, message)
                .map_err(|e| format!("Failed to write rebase message: {e}"))?;
            return Ok(());
        }
        Err("No rebase message file available".to_string())
    }

    pub fn resolve_conflict(&self, request: ConflictResolveRequest) -> Result<(), String> {
        let repo = open_repo(&self.ctx.repo_path)?;
        let path = request.path.as_str();
        match request.strategy {
            ConflictResolveStrategy::Ours => {
                checkout_stage(&repo, path, 2)?;
                add_path(&repo, path)?;
            }
            ConflictResolveStrategy::Theirs => {
                checkout_stage(&repo, path, 3)?;
                add_path(&repo, path)?;
            }
            ConflictResolveStrategy::Union => {
                let ours = stage_blob_content(&repo, path, 2)?;
                let theirs = stage_blob_content(&repo, path, 3)?;
                let combined = format!("{ours}\n<<<<<<< union >>>>>>>\n{theirs}");
                let abs = Path::new(&self.ctx.repo_path).join(path);
                if let Some(parent) = abs.parent() {
                    fs::create_dir_all(parent).ok();
                }
                fs::write(&abs, combined).map_err(|e| format!("Failed to write union: {e}"))?;
                add_path(&repo, path)?;
            }
        }
        Ok(())
    }

    fn is_native_interactive_rebase(&self) -> bool {
        self.operation()
            .git_dir()
            .map(|git_dir| git_dir.join("rebase-merge").join("interactive").is_file())
            .unwrap_or(false)
    }

    async fn cli_rebase_command(
        &self,
        args: &[&str],
        message: Option<&str>,
    ) -> Result<String, String> {
        if let Some(msg) = message.filter(|m| !m.trim().is_empty()) {
            let git_dir = self.operation().git_dir()?;
            fs::write(git_dir.join("COMMIT_EDITMSG"), msg)
                .map_err(|e| format!("Failed to write commit message: {e}"))?;
            // Native interactive continue reads `.git/rebase-merge/message`.
            let rebase_msg = git_dir.join("rebase-merge").join("message");
            if rebase_msg.parent().is_some_and(|p| p.is_dir()) {
                fs::write(&rebase_msg, msg)
                    .map_err(|e| format!("Failed to write rebase message: {e}"))?;
            }
        }
        self.ctx
            .runner
            .run_with_env(
                args,
                GitRunOptions::default_read()
                    .with_timeout(Duration::from_secs(120))
                    .allow_exit_codes(&[1]),
                REBASE_NONINTERACTIVE_ENV,
            )
            .await
    }
}

fn open_repo(path: &str) -> Result<Repository, String> {
    Repository::open(path).map_err(|e| format!("Failed to open repository: {e}"))
}

fn revparse_oid(repo: &Repository, spec: &str) -> Result<Oid, String> {
    let obj = repo
        .revparse_single(spec)
        .map_err(|e| format!("Invalid revision '{spec}': {e}"))?;
    Ok(obj.id())
}

fn annotated<'a>(repo: &'a Repository, spec: &str) -> Result<AnnotatedCommit<'a>, String> {
    let oid = revparse_oid(repo, spec)?;
    repo.find_annotated_commit(oid)
        .map_err(|e| format!("Failed to annotate commit: {e}"))
}

fn emit_progress(app: &Option<tauri::AppHandle>, event: RebaseProgressEvent) {
    if let Some(app) = app {
        let _ = app.emit(REBASE_PROGRESS_EVENT, event);
    }
}

fn continue_native_git2_blocking(
    repo_path: &str,
    message: Option<String>,
) -> Result<RepoOperation, String> {
    let repo = open_repo(repo_path)?;
    let mut rebase = repo
        .open_rebase(None)
        .map_err(|e| format!("open_rebase: {e}"))?;

    if !conflict_paths_from_index(&repo)?.is_empty() {
        return Err("Resolve conflicted files before continuing".into());
    }

    let sig = signature_for(&repo)?;
    let msg_ref = message.as_deref();
    rebase
        .commit(None, &sig, msg_ref)
        .map_err(|e| format!("Failed to commit rebase step: {e}"))?;

    let total = rebase.len() as u32;
    loop {
        match rebase.next() {
            Some(Ok(_op)) => {
                let index = repo.index().map_err(|e| e.to_string())?;
                if index.has_conflicts() {
                    drop(index);
                    return OperationService::new(Arc::new(RepoContext::new(repo_path)?))
                        .get_repo_operation();
                }
                drop(index);
                rebase
                    .commit(None, &sig, None)
                    .map_err(|e| format!("Failed to commit rebase step: {e}"))?;
                let _ = total;
            }
            None => break,
            Some(Err(e)) => {
                return Err(format!("Rebase failed: {e}"));
            }
        }
    }

    rebase
        .finish(None)
        .map_err(|e| format!("Failed to finish rebase: {e}"))?;

    OperationService::new(Arc::new(RepoContext::new(repo_path)?)).get_repo_operation()
}

fn start_rebase_blocking(
    repo_path: &str,
    request: RebaseStartRequest,
    app: Option<tauri::AppHandle>,
) -> Result<RepoOperation, String> {
    emit_progress(
        &app,
        RebaseProgressEvent {
            phase: RebaseProgressPhase::Preparing,
            current: None,
            total: None,
            message: Some("Preparing rebase".into()),
            commit: None,
        },
    );

    let mut repo = open_repo(repo_path)?;
    if repo.state() != git2::RepositoryState::Clean {
        return Err("Repository already has an operation in progress".into());
    }

    let gitru_dir = repo.path().join(GITRU_REBASE_DIR);
    if gitru_dir.exists() {
        return Err("A Gitru rebase state already exists".into());
    }

    if request.autostash {
        let sig = signature_for(&repo)?;
        let _ = repo.stash_save(&sig, "!!Gitru rebase autostash", Some(git2::StashFlags::DEFAULT));
    } else if !is_worktree_clean(&repo)? {
        return Err(
            "You have uncommitted changes. Commit, stash, or enable autostash before rebasing."
                .into(),
        );
    }

    let plan = if let Some(entries) = request.entries {
        validate_todo(&entries)?;
        RebasePlan {
            onto: revparse_oid(&repo, &request.onto)?.to_string(),
            upstream: request
                .upstream
                .as_deref()
                .map(|u| revparse_oid(&repo, u).map(|o| o.to_string()))
                .transpose()?,
            entries,
            autostash: request.autostash,
        }
    } else {
        // Plain rebase: use git2 rebase API for pick-all
        return start_plain_git2_rebase(repo_path, &request, app);
    };

    // Interactive / custom plan → Gitru sequencer
    init_gitru_state(&repo, &plan)?;
    emit_progress(
        &app,
        RebaseProgressEvent {
            phase: RebaseProgressPhase::Started,
            current: Some(1),
            total: Some(plan.entries.len() as u32),
            message: Some("Rebase started".into()),
            commit: None,
        },
    );
    run_gitru_until_pause(repo_path, app)
}

fn start_plain_git2_rebase(
    repo_path: &str,
    request: &RebaseStartRequest,
    app: Option<tauri::AppHandle>,
) -> Result<RepoOperation, String> {
    let repo = open_repo(repo_path)?;
    let onto = annotated(&repo, &request.onto)?;
    let upstream_spec = request.upstream.as_deref().unwrap_or(request.onto.as_str());
    let upstream = annotated(&repo, upstream_spec)?;
    let head_ann = {
        let head = repo
            .head()
            .map_err(|e| format!("Failed to read HEAD: {e}"))?;
        let oid = head
            .target()
            .ok_or_else(|| "HEAD is unborn".to_string())?;
        repo.find_annotated_commit(oid)
            .map_err(|e| format!("Failed to annotate HEAD: {e}"))?
    };

    let mut opts = git2::RebaseOptions::new();
    let mut rebase = repo
        .rebase(Some(&head_ann), Some(&upstream), Some(&onto), Some(&mut opts))
        .map_err(|e| format!("Failed to start rebase: {e}"))?;

    let total = rebase.len() as u32;
    emit_progress(
        &app,
        RebaseProgressEvent {
            phase: RebaseProgressPhase::Started,
            current: Some(0),
            total: Some(total),
            message: Some("Rebase started".into()),
            commit: None,
        },
    );

    let sig = signature_for(&repo)?;
    let mut step = 0u32;
    loop {
        match rebase.next() {
            Some(Ok(op)) => {
                step += 1;
                let commit_id = op.id().to_string();
                emit_progress(
                    &app,
                    RebaseProgressEvent {
                        phase: RebaseProgressPhase::Applying,
                        current: Some(step),
                        total: Some(total),
                        message: Some(format!("Applying {commit_id}")),
                        commit: Some(commit_id.clone()),
                    },
                );

                let index = repo
                    .index()
                    .map_err(|e| format!("Failed to read index: {e}"))?;
                if index.has_conflicts() {
                    drop(index);
                    emit_progress(
                        &app,
                        RebaseProgressEvent {
                            phase: RebaseProgressPhase::Paused,
                            current: Some(step),
                            total: Some(total),
                            message: Some("Conflict — resolve and continue".into()),
                            commit: Some(commit_id),
                        },
                    );
                    // Leave rebase in progress for open_rebase + continue
                    return OperationService::new(Arc::new(RepoContext::new(repo_path)?))
                        .get_repo_operation();
                }
                drop(index);

                rebase
                    .commit(None, &sig, None)
                    .map_err(|e| format!("Failed to commit rebase step: {e}"))?;
            }
            None => break,
            Some(Err(e)) => {
                let _ = rebase.abort();
                return Err(format!("Rebase failed: {e}"));
            }
        }
    }

    rebase
        .finish(None)
        .map_err(|e| format!("Failed to finish rebase: {e}"))?;

    emit_progress(
        &app,
        RebaseProgressEvent {
            phase: RebaseProgressPhase::Finished,
            current: Some(total),
            total: Some(total),
            message: Some("Rebase finished".into()),
            commit: None,
        },
    );

    OperationService::new(Arc::new(RepoContext::new(repo_path)?)).get_repo_operation()
}

fn init_gitru_state(repo: &Repository, plan: &RebasePlan) -> Result<(), String> {
    let dir = repo.path().join(GITRU_REBASE_DIR);
    fs::create_dir_all(&dir).map_err(|e| format!("Failed to create gitru-rebase dir: {e}"))?;

    let head = repo
        .head()
        .map_err(|e| format!("Failed to read HEAD: {e}"))?;
    let head_name = if head.is_branch() {
        head.name().unwrap_or("detached").to_string()
    } else {
        "detached HEAD".to_string()
    };
    let orig_head = head
        .target()
        .ok_or_else(|| "HEAD is unborn".to_string())?
        .to_string();

    fs::write(dir.join("onto"), &plan.onto).map_err(|e| e.to_string())?;
    fs::write(dir.join("orig-head"), &orig_head).map_err(|e| e.to_string())?;
    fs::write(dir.join("head-name"), &head_name).map_err(|e| e.to_string())?;
    fs::write(dir.join("current-index"), "0").map_err(|e| e.to_string())?;
    write_gitru_todo(&dir, &plan.entries)?;

    // Detach and reset onto base
    let onto_oid = Oid::from_str(&plan.onto).map_err(|e| format!("Invalid onto oid: {e}"))?;
    let onto_commit = repo
        .find_commit(onto_oid)
        .map_err(|e| format!("Failed to find onto commit: {e}"))?;
    repo.reset(onto_commit.as_object(), ResetType::Hard, None)
        .map_err(|e| format!("Failed to reset onto base: {e}"))?;

    Ok(())
}

fn write_gitru_todo(dir: &Path, entries: &[RebasePlanEntry]) -> Result<(), String> {
    let mut out = String::new();
    for e in entries {
        let msg = e.message.as_deref().unwrap_or("");
        out.push_str(&format!("{} {} {}\n", e.action.as_str(), e.commit, msg));
    }
    fs::write(dir.join("todo"), out).map_err(|e| format!("Failed to write todo: {e}"))
}

fn count_native_todo_lines(path: &Path) -> usize {
    let Ok(raw) = fs::read_to_string(path) else {
        return 0;
    };
    raw.lines()
        .filter(|line| {
            let line = line.trim();
            !(line.is_empty() || line.starts_with('#') || line.starts_with("exec"))
                && line
                    .split_whitespace()
                    .next()
                    .and_then(RebaseAction::parse)
                    .is_some()
        })
        .count()
}

fn write_native_todo(dir: &Path, entries: &[RebasePlanEntry]) -> Result<(), String> {
    let mut out = String::new();
    for e in entries {
        let msg = e.message.as_deref().unwrap_or("").trim();
        if msg.is_empty() {
            out.push_str(&format!("{} {}\n", e.action.as_str(), e.commit));
        } else {
            out.push_str(&format!("{} {} # {}\n", e.action.as_str(), e.commit, msg));
        }
    }
    fs::write(dir.join("git-rebase-todo"), out)
        .map_err(|e| format!("Failed to write git-rebase-todo: {e}"))
}

fn validate_todo(entries: &[RebasePlanEntry]) -> Result<(), String> {
    if entries.is_empty() {
        return Err("Rebase plan has no commits".into());
    }
    let first = &entries[0].action;
    if matches!(first, RebaseAction::Squash | RebaseAction::Fixup) {
        return Err("First commit cannot be squash or fixup".into());
    }
    Ok(())
}

fn run_gitru_until_pause(
    repo_path: &str,
    app: Option<tauri::AppHandle>,
) -> Result<RepoOperation, String> {
    let repo = open_repo(repo_path)?;
    let dir = repo.path().join(GITRU_REBASE_DIR);
    let entries = read_plan_entries(&dir)?;
    let mut idx = read_trimmed(dir.join("current-index"))
        .ok()
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(0);

    let total = entries.len() as u32;
    let sig = signature_for(&repo)?;

    while idx < entries.len() {
        let entry = &entries[idx];
        fs::write(dir.join("current-index"), idx.to_string()).ok();
        fs::write(dir.join("paused-at"), &entry.commit).ok();

        emit_progress(
            &app,
            RebaseProgressEvent {
                phase: RebaseProgressPhase::Applying,
                current: Some((idx + 1) as u32),
                total: Some(total),
                message: Some(format!(
                    "{} {}",
                    entry.action.as_str(),
                    entry.commit.chars().take(7).collect::<String>()
                )),
                commit: Some(entry.commit.clone()),
            },
        );

        match entry.action {
            RebaseAction::Drop => {
                idx += 1;
                continue;
            }
            RebaseAction::Pick | RebaseAction::Reword | RebaseAction::Edit => {
                let oid = Oid::from_str(&entry.commit)
                    .map_err(|e| format!("Invalid commit oid: {e}"))?;
                let commit = repo
                    .find_commit(oid)
                    .map_err(|e| format!("Failed to find commit: {e}"))?;

                let mut opts = CherrypickOptions::new();
                repo.cherrypick(&commit, Some(&mut opts))
                    .map_err(|e| format!("Cherry-pick failed: {e}"))?;

                if conflict_paths_from_index(&repo)?.is_empty() {
                    // ok
                } else {
                    set_pause(&dir, RebasePauseReason::Conflict)?;
                    emit_paused(&app, idx, total, &entry.commit, "Conflict");
                    return OperationService::new(Arc::new(RepoContext::new(repo_path)?))
                        .get_repo_operation();
                }

                match entry.action {
                    RebaseAction::Reword => {
                        let msg = commit.message().unwrap_or("").to_string();
                        fs::write(dir.join("message"), &msg).ok();
                        set_pause(&dir, RebasePauseReason::Reword)?;
                        emit_paused(&app, idx, total, &entry.commit, "Reword");
                        return OperationService::new(Arc::new(RepoContext::new(repo_path)?))
                            .get_repo_operation();
                    }
                    RebaseAction::Edit => {
                        commit_cherrypick(&repo, &commit, &sig, None)?;
                        set_pause(&dir, RebasePauseReason::Edit)?;
                        emit_paused(&app, idx, total, &entry.commit, "Edit");
                        return OperationService::new(Arc::new(RepoContext::new(repo_path)?))
                            .get_repo_operation();
                    }
                    _ => {
                        commit_cherrypick(&repo, &commit, &sig, None)?;
                    }
                }
            }
            RebaseAction::Squash | RebaseAction::Fixup => {
                let oid = Oid::from_str(&entry.commit)
                    .map_err(|e| format!("Invalid commit oid: {e}"))?;
                let commit = repo
                    .find_commit(oid)
                    .map_err(|e| format!("Failed to find commit: {e}"))?;

                let mut opts = CherrypickOptions::new();
                repo.cherrypick(&commit, Some(&mut opts))
                    .map_err(|e| format!("Cherry-pick failed: {e}"))?;

                if !conflict_paths_from_index(&repo)?.is_empty() {
                    set_pause(&dir, RebasePauseReason::Conflict)?;
                    emit_paused(&app, idx, total, &entry.commit, "Conflict");
                    return OperationService::new(Arc::new(RepoContext::new(repo_path)?))
                        .get_repo_operation();
                }

                // Soft-reset to parent (previous HEAD before cherry-pick tree) then amend
                let head = repo
                    .head()
                    .map_err(|e| format!("Failed to read HEAD: {e}"))?
                    .peel_to_commit()
                    .map_err(|e| format!("Failed to peel HEAD: {e}"))?;
                // After cherry-pick without commit, HEAD is still previous; index has new tree.
                // Create amended commit: reuse parent message for fixup, combine for squash.
                let tree_id = {
                    let mut index = repo.index().map_err(|e| e.to_string())?;
                    let tid = index.write_tree().map_err(|e| e.to_string())?;
                    tid
                };
                let tree = repo.find_tree(tree_id).map_err(|e| e.to_string())?;
                let parents: Vec<_> = head.parents().collect();
                let parent_refs: Vec<&git2::Commit> = parents.iter().collect();

                let message = if matches!(entry.action, RebaseAction::Fixup) {
                    head.message().unwrap_or("").to_string()
                } else {
                    let prev = head.message().unwrap_or("").trim().to_string();
                    let cur = commit.message().unwrap_or("").trim().to_string();
                    format!("{prev}\n\n{cur}")
                };

                // Replace HEAD commit (amend)
                repo.commit(
                    Some("HEAD"),
                    &sig,
                    &sig,
                    &message,
                    &tree,
                    &parent_refs,
                )
                .map_err(|e| format!("Failed to squash commit: {e}"))?;

                // Clear cherry-pick state
                let _ = repo.cleanup_state();
            }
        }

        idx += 1;
        fs::write(dir.join("current-index"), idx.to_string()).ok();
    }

    finish_gitru(&repo, &dir, &app)?;
    OperationService::new(Arc::new(RepoContext::new(repo_path)?)).get_repo_operation()
}

fn continue_gitru_blocking(
    repo_path: &str,
    message: Option<String>,
    app: Option<tauri::AppHandle>,
) -> Result<RepoOperation, String> {
    let repo = open_repo(repo_path)?;
    let dir = repo.path().join(GITRU_REBASE_DIR);
    if !dir.is_dir() {
        return Err("No Gitru rebase in progress".into());
    }

    let conflicts = conflict_paths_from_index(&repo)?;
    if !conflicts.is_empty() {
        return Err(format!(
            "Resolve {} conflicted file(s) before continuing",
            conflicts.len()
        ));
    }

    let entries = read_plan_entries(&dir)?;
    let idx = read_trimmed(dir.join("current-index"))
        .ok()
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(0);
    let pause = read_trimmed(dir.join("pause-reason")).ok();
    let sig = signature_for(&repo)?;

    if idx < entries.len() {
        let entry = &entries[idx];
        let oid = Oid::from_str(&entry.commit).ok();
        let original = oid.and_then(|o| repo.find_commit(o).ok());

        match pause.as_deref() {
            Some("conflict") | Some("reword") | None => {
                // Need to create the commit for pick/reword/conflict resolution
                if matches!(
                    entry.action,
                    RebaseAction::Pick | RebaseAction::Reword | RebaseAction::Squash | RebaseAction::Fixup
                ) || pause.as_deref() == Some("conflict")
                {
                    let msg = message
                        .or_else(|| read_trimmed(dir.join("message")).ok())
                        .or_else(|| {
                            original
                                .as_ref()
                                .and_then(|c| c.message().map(|m| m.to_string()))
                        })
                        .unwrap_or_else(|| entry.message.clone().unwrap_or_default());

                    if let Some(commit) = original.as_ref() {
                        // If cherry-pick state exists or index dirty relative to HEAD
                        let _ = commit_from_index(&repo, commit, &sig, Some(&msg));
                    } else {
                        commit_from_index_simple(&repo, &sig, &msg)?;
                    }
                }
            }
            Some("edit") => {
                // User already has a commit; just continue
            }
            _ => {}
        }
    }

    let _ = repo.cleanup_state();
    clear_pause(&dir)?;
    let next = idx + 1;
    fs::write(dir.join("current-index"), next.to_string()).ok();
    run_gitru_until_pause(repo_path, app)
}

fn skip_gitru_blocking(
    repo_path: &str,
    app: Option<tauri::AppHandle>,
) -> Result<RepoOperation, String> {
    let repo = open_repo(repo_path)?;
    let dir = repo.path().join(GITRU_REBASE_DIR);
    if !dir.is_dir() {
        return Err("No Gitru rebase in progress".into());
    }

    // Reset any partial cherry-pick
    let head = repo
        .head()
        .map_err(|e| e.to_string())?
        .peel_to_commit()
        .map_err(|e| e.to_string())?;
    repo.reset(head.as_object(), ResetType::Hard, None)
        .map_err(|e| format!("Failed to reset for skip: {e}"))?;
    let _ = repo.cleanup_state();

    let idx = read_trimmed(dir.join("current-index"))
        .ok()
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(0);
    clear_pause(&dir)?;
    fs::write(dir.join("current-index"), (idx + 1).to_string()).ok();
    run_gitru_until_pause(repo_path, app)
}

fn abort_gitru_blocking(
    repo_path: &str,
    app: Option<tauri::AppHandle>,
) -> Result<RepoOperation, String> {
    let repo = open_repo(repo_path)?;
    let dir = repo.path().join(GITRU_REBASE_DIR);
    if !dir.is_dir() {
        // Try native abort via open_rebase
        if let Ok(mut rebase) = repo.open_rebase(None) {
            rebase.abort().map_err(|e| format!("Abort failed: {e}"))?;
        }
        return OperationService::new(Arc::new(RepoContext::new(repo_path)?)).get_repo_operation();
    }

    let orig = read_trimmed(dir.join("orig-head"))?;
    let head_name = read_trimmed(dir.join("head-name")).ok();
    let oid = Oid::from_str(&orig).map_err(|e| format!("Invalid orig-head: {e}"))?;
    let commit = repo
        .find_commit(oid)
        .map_err(|e| format!("Failed to find orig-head: {e}"))?;

    repo.reset(commit.as_object(), ResetType::Hard, None)
        .map_err(|e| format!("Failed to reset to orig-head: {e}"))?;

    if let Some(name) = head_name {
        if name.starts_with("refs/heads/") {
            // Move branch ref back and checkout
            repo.reference(&name, oid, true, "gitru rebase abort")
                .map_err(|e| format!("Failed to restore branch ref: {e}"))?;
            let obj = repo
                .revparse_single(&name)
                .map_err(|e| e.to_string())?;
            repo.checkout_tree(&obj, None)
                .map_err(|e| format!("Failed to checkout branch: {e}"))?;
            repo.set_head(&name)
                .map_err(|e| format!("Failed to set HEAD: {e}"))?;
        }
    }

    let _ = repo.cleanup_state();
    let _ = fs::remove_dir_all(&dir);

    emit_progress(
        &app,
        RebaseProgressEvent {
            phase: RebaseProgressPhase::Aborted,
            current: None,
            total: None,
            message: Some("Rebase aborted".into()),
            commit: None,
        },
    );

    OperationService::new(Arc::new(RepoContext::new(repo_path)?)).get_repo_operation()
}

fn finish_gitru(
    repo: &Repository,
    dir: &Path,
    app: &Option<tauri::AppHandle>,
) -> Result<(), String> {
    let head_name = read_trimmed(dir.join("head-name")).ok();
    let head_oid = repo
        .head()
        .ok()
        .and_then(|h| h.target());

    if let (Some(name), Some(oid)) = (head_name, head_oid) {
        if name.starts_with("refs/heads/") {
            repo.reference(&name, oid, true, "gitru rebase finish")
                .map_err(|e| format!("Failed to update branch: {e}"))?;
            repo.set_head(&name)
                .map_err(|e| format!("Failed to set HEAD: {e}"))?;
        }
    }

    let _ = fs::remove_dir_all(dir);
    emit_progress(
        app,
        RebaseProgressEvent {
            phase: RebaseProgressPhase::Finished,
            current: None,
            total: None,
            message: Some("Rebase finished".into()),
            commit: None,
        },
    );
    Ok(())
}

fn read_plan_entries(dir: &Path) -> Result<Vec<RebasePlanEntry>, String> {
    let raw = fs::read_to_string(dir.join("todo")).map_err(|e| e.to_string())?;
    let mut entries = Vec::new();
    for line in raw.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut parts = line.splitn(3, char::is_whitespace);
        let action = RebaseAction::parse(parts.next().unwrap_or(""))
            .ok_or_else(|| format!("Invalid action in todo: {line}"))?;
        let commit = parts
            .next()
            .ok_or_else(|| format!("Missing commit in todo: {line}"))?
            .to_string();
        let message = parts.next().map(|s| s.to_string());
        entries.push(RebasePlanEntry {
            action,
            commit,
            message,
        });
    }
    Ok(entries)
}

fn set_pause(dir: &Path, reason: RebasePauseReason) -> Result<(), String> {
    let s = match reason {
        RebasePauseReason::Conflict => "conflict",
        RebasePauseReason::Edit => "edit",
        RebasePauseReason::Reword => "reword",
        RebasePauseReason::Waiting => "waiting",
    };
    fs::write(dir.join("pause-reason"), s).map_err(|e| e.to_string())
}

fn clear_pause(dir: &Path) -> Result<(), String> {
    let _ = fs::remove_file(dir.join("pause-reason"));
    let _ = fs::remove_file(dir.join("paused-at"));
    Ok(())
}

fn emit_paused(
    app: &Option<tauri::AppHandle>,
    idx: usize,
    total: u32,
    commit: &str,
    reason: &str,
) {
    emit_progress(
        app,
        RebaseProgressEvent {
            phase: RebaseProgressPhase::Paused,
            current: Some((idx + 1) as u32),
            total: Some(total),
            message: Some(format!("{reason} at {}", &commit[..commit.len().min(7)])),
            commit: Some(commit.to_string()),
        },
    );
}

fn commit_cherrypick(
    repo: &Repository,
    original: &git2::Commit,
    sig: &Signature,
    message_override: Option<&str>,
) -> Result<Oid, String> {
    let msg = message_override
        .map(|s| s.to_string())
        .unwrap_or_else(|| original.message().unwrap_or("").to_string());
    commit_from_index(repo, original, sig, Some(&msg))
}

fn commit_from_index(
    repo: &Repository,
    original: &git2::Commit,
    sig: &Signature,
    message: Option<&str>,
) -> Result<Oid, String> {
    let mut index = repo.index().map_err(|e| e.to_string())?;
    let tree_id = index.write_tree().map_err(|e| e.to_string())?;
    index.write().map_err(|e| e.to_string())?;
    let tree = repo.find_tree(tree_id).map_err(|e| e.to_string())?;
    let parent = repo
        .head()
        .ok()
        .and_then(|h| h.peel_to_commit().ok());
    let parents: Vec<&git2::Commit> = match &parent {
        Some(p) => vec![p],
        None => vec![],
    };
    let msg = message.unwrap_or_else(|| original.message().unwrap_or(""));
    let author = original.author();
    let oid = repo
        .commit(
            Some("HEAD"),
            &author,
            sig,
            msg,
            &tree,
            &parents,
        )
        .map_err(|e| format!("Failed to create commit: {e}"))?;
    let _ = repo.cleanup_state();
    Ok(oid)
}

fn commit_from_index_simple(
    repo: &Repository,
    sig: &Signature,
    message: &str,
) -> Result<Oid, String> {
    let mut index = repo.index().map_err(|e| e.to_string())?;
    let tree_id = index.write_tree().map_err(|e| e.to_string())?;
    index.write().map_err(|e| e.to_string())?;
    let tree = repo.find_tree(tree_id).map_err(|e| e.to_string())?;
    let parent = repo
        .head()
        .ok()
        .and_then(|h| h.peel_to_commit().ok());
    let parents: Vec<&git2::Commit> = match &parent {
        Some(p) => vec![p],
        None => vec![],
    };
    let oid = repo
        .commit(Some("HEAD"), sig, sig, message, &tree, &parents)
        .map_err(|e| format!("Failed to create commit: {e}"))?;
    let _ = repo.cleanup_state();
    Ok(oid)
}

fn signature_for(repo: &Repository) -> Result<Signature<'static>, String> {
    let sig = repo
        .signature()
        .or_else(|_| Signature::now("Gitru", "gitru@local"))
        .map_err(|e| format!("Failed to create signature: {e}"))?;
    // Leak into owned via to_owned pattern
    Signature::now(
        sig.name().unwrap_or("Gitru"),
        sig.email().unwrap_or("gitru@local"),
    )
    .map_err(|e| format!("Failed to create signature: {e}"))
}

fn is_worktree_clean(repo: &Repository) -> Result<bool, String> {
    let statuses = repo
        .statuses(None)
        .map_err(|e| format!("Failed to get status: {e}"))?;
    Ok(statuses.is_empty())
}

fn checkout_stage(repo: &Repository, path: &str, stage: i32) -> Result<(), String> {
    let index = repo.index().map_err(|e| e.to_string())?;
    let entry = index
        .get_path(Path::new(path), stage)
        .ok_or_else(|| format!("No stage {stage} for {path}"))?;
    let blob = repo
        .find_blob(entry.id)
        .map_err(|e| format!("Failed to find blob: {e}"))?;
    let abs = repo
        .workdir()
        .ok_or_else(|| "Bare repository".to_string())?
        .join(path);
    if let Some(parent) = abs.parent() {
        fs::create_dir_all(parent).ok();
    }
    fs::write(&abs, blob.content()).map_err(|e| format!("Failed to write file: {e}"))?;
    Ok(())
}

fn stage_blob_content(repo: &Repository, path: &str, stage: i32) -> Result<String, String> {
    let index = repo.index().map_err(|e| e.to_string())?;
    let entry = index
        .get_path(Path::new(path), stage)
        .ok_or_else(|| format!("No stage {stage} for {path}"))?;
    let blob = repo
        .find_blob(entry.id)
        .map_err(|e| format!("Failed to find blob: {e}"))?;
    String::from_utf8(blob.content().to_vec())
        .or_else(|_| Ok(String::from_utf8_lossy(blob.content()).to_string()))
}

fn add_path(repo: &Repository, path: &str) -> Result<(), String> {
    let mut index = repo.index().map_err(|e| e.to_string())?;
    index
        .add_path(Path::new(path))
        .map_err(|e| format!("Failed to stage {path}: {e}"))?;
    index.write().map_err(|e| e.to_string())?;
    Ok(())
}
