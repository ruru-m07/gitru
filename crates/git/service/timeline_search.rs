use std::collections::{HashMap, HashSet};
use std::sync::{
    Arc, Mutex, OnceLock,
    atomic::{AtomicBool, Ordering},
};

use tauri::{AppHandle, Emitter};

use crate::{
    context::RepoContext,
    models::timeline_search::{
        TimelineSearchHit, TimelineSearchMode, TimelineSearchPhase, TimelineSearchProgressEvent,
        TimelineSearchQuery,
    },
    parsers::timeline_search::{parse_grep_hit_line, PickaxeStreamParser},
    runner::{GitCommandRunner, GitRunOptions},
};

pub const TIMELINE_SEARCH_PROGRESS_EVENT: &str = "git://timeline-search-progress";

const DEFAULT_HIT_LIMIT: usize = 500;
const SEARCH_TIMEOUT_SECS: u64 = 600;

static ACTIVE_SEARCH_CANCEL_FLAGS: OnceLock<Mutex<HashMap<String, Arc<AtomicBool>>>> =
    OnceLock::new();

fn active_search_cancel_flags() -> &'static Mutex<HashMap<String, Arc<AtomicBool>>> {
    ACTIVE_SEARCH_CANCEL_FLAGS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn hit_dedupe_key(hit: &TimelineSearchHit) -> String {
    format!("{}:{}", hit.commit_hash, hit.file_path)
}

fn should_emit_hit(seen: &mut HashSet<String>, hit: &TimelineSearchHit) -> bool {
    seen.insert(hit_dedupe_key(hit))
}

pub struct TimelineSearchService {
    ctx: Arc<RepoContext>,
}

impl TimelineSearchService {
    pub fn new(ctx: Arc<RepoContext>) -> Self {
        Self { ctx }
    }

    pub fn cancel_search(operation_id: &str) -> Result<bool, String> {
        let flag = {
            let active = active_search_cancel_flags()
                .lock()
                .map_err(|_| "Failed to access active timeline searches".to_string())?;
            active.get(operation_id).cloned()
        };

        let Some(flag) = flag else {
            return Ok(false);
        };

        flag.store(true, Ordering::Relaxed);
        Ok(true)
    }

    pub async fn start_search(&self, query: TimelineSearchQuery, app: AppHandle) -> Result<(), String> {
        let cancel_flag = Arc::new(AtomicBool::new(false));
        {
            let mut active = active_search_cancel_flags()
                .lock()
                .map_err(|_| "Failed to access active timeline searches".to_string())?;
            active.insert(query.operation_id.clone(), cancel_flag.clone());
        }

        let emit = |event: TimelineSearchProgressEvent| {
            let _ = app.emit(TIMELINE_SEARCH_PROGRESS_EVENT, event);
        };

        emit(TimelineSearchProgressEvent {
            operation_id: query.operation_id.clone(),
            phase: TimelineSearchPhase::Started,
            hit: None,
            commits_scanned: 0,
            hits_found: 0,
            status: Some("Starting timeline search".to_string()),
            error: None,
        });

        let result = match query.mode {
            TimelineSearchMode::Pickaxe => self.run_pickaxe_search(&query, cancel_flag.clone(), emit).await,
            TimelineSearchMode::FullContent => {
                self.run_full_content_search(&query, cancel_flag.clone(), emit)
                    .await
            }
        };

        {
            let mut active = active_search_cancel_flags()
                .lock()
                .map_err(|_| "Failed to access active timeline searches".to_string())?;
            active.remove(&query.operation_id);
        }

        if cancel_flag.load(Ordering::Relaxed) {
            emit(TimelineSearchProgressEvent {
                operation_id: query.operation_id.clone(),
                phase: TimelineSearchPhase::Cancelled,
                hit: None,
                commits_scanned: 0,
                hits_found: 0,
                status: Some("Search cancelled".to_string()),
                error: None,
            });
            return Ok(());
        }

        match result {
            Ok((commits_scanned, hits_found)) => {
                emit(TimelineSearchProgressEvent {
                    operation_id: query.operation_id.clone(),
                    phase: TimelineSearchPhase::Finished,
                    hit: None,
                    commits_scanned,
                    hits_found,
                    status: Some(format!("Found {hits_found} hits in {commits_scanned} commits")),
                    error: None,
                });
                Ok(())
            }
            Err(error) => {
                emit(TimelineSearchProgressEvent {
                    operation_id: query.operation_id,
                    phase: TimelineSearchPhase::Error,
                    hit: None,
                    commits_scanned: 0,
                    hits_found: 0,
                    status: None,
                    error: Some(error.clone()),
                });
                Err(error)
            }
        }
    }

    async fn run_pickaxe_search<F>(
        &self,
        query: &TimelineSearchQuery,
        cancel_flag: Arc<AtomicBool>,
        mut emit: F,
    ) -> Result<(u32, u32), String>
    where
        F: FnMut(TimelineSearchProgressEvent),
    {
        let runner = GitCommandRunner::new(&self.ctx.repo_path)?;
        let args = build_pickaxe_log_args(query)?;
        let hit_limit = query.limit.unwrap_or(DEFAULT_HIT_LIMIT);
        let options = GitRunOptions::default_read().with_timeout(std::time::Duration::from_secs(
            SEARCH_TIMEOUT_SECS,
        ));

        let mut parser = PickaxeStreamParser::new();
        let mut seen_hits = HashSet::new();
        let mut commits_scanned = 0u32;
        let mut hits_found = 0u32;
        let mut stop = false;
        let mut last_commit_hash = String::new();

        let exit_code = runner
            .run_streaming(
                &args.iter().map(String::as_str).collect::<Vec<_>>(),
                options,
                cancel_flag,
                |line| {
                    if stop {
                        return false;
                    }

                    if is_commit_hash_line(line) && line != last_commit_hash {
                        last_commit_hash = line.to_string();
                        commits_scanned = commits_scanned.saturating_add(1);
                    }

                    for hit in parser.push_line(line) {
                        if !should_emit_hit(&mut seen_hits, &hit) {
                            continue;
                        }

                        hits_found = hits_found.saturating_add(1);
                        emit(TimelineSearchProgressEvent {
                            operation_id: query.operation_id.clone(),
                            phase: TimelineSearchPhase::Hit,
                            hit: Some(hit),
                            commits_scanned,
                            hits_found,
                            status: None,
                            error: None,
                        });

                        if hits_found as usize >= hit_limit {
                            stop = true;
                            return false;
                        }
                    }

                    if commits_scanned.is_multiple_of(25) {
                        emit(TimelineSearchProgressEvent {
                            operation_id: query.operation_id.clone(),
                            phase: TimelineSearchPhase::Progress,
                            hit: None,
                            commits_scanned,
                            hits_found,
                            status: Some(format!("Scanned {commits_scanned} commits")),
                            error: None,
                        });
                    }

                    true
                },
            )
            .await?;

        if exit_code == -1 {
            return Ok((commits_scanned, hits_found));
        }

        for hit in parser.finish() {
            if !should_emit_hit(&mut seen_hits, &hit) {
                continue;
            }

            hits_found = hits_found.saturating_add(1);
            emit(TimelineSearchProgressEvent {
                operation_id: query.operation_id.clone(),
                phase: TimelineSearchPhase::Hit,
                hit: Some(hit),
                commits_scanned,
                hits_found,
                status: None,
                error: None,
            });

            if hits_found as usize >= hit_limit {
                break;
            }
        }

        Ok((commits_scanned, hits_found))
    }

    async fn run_full_content_search<F>(
        &self,
        query: &TimelineSearchQuery,
        cancel_flag: Arc<AtomicBool>,
        mut emit: F,
    ) -> Result<(u32, u32), String>
    where
        F: FnMut(TimelineSearchProgressEvent),
    {
        let runner = GitCommandRunner::new(&self.ctx.repo_path)?;
        let rev_args = build_rev_list_args(query)?;
        let hit_limit = query.limit.unwrap_or(DEFAULT_HIT_LIMIT);
        let options = GitRunOptions::default_read().with_timeout(std::time::Duration::from_secs(
            SEARCH_TIMEOUT_SECS,
        ));

        let commits_output = runner
            .run_with_options(
                &rev_args.iter().map(String::as_str).collect::<Vec<_>>(),
                options.clone(),
            )
            .await?;

        let mut seen_hits = HashSet::new();
        let mut commits_scanned = 0u32;
        let mut hits_found = 0u32;

        for commit_hash in commits_output.lines().map(str::trim).filter(|line| !line.is_empty()) {
            if cancel_flag.load(Ordering::Relaxed) {
                break;
            }

            commits_scanned = commits_scanned.saturating_add(1);
            let commit_hits = self
                .grep_commit(&runner, query, commit_hash, options.clone())
                .await?;

            for hit in commit_hits {
                if !should_emit_hit(&mut seen_hits, &hit) {
                    continue;
                }

                hits_found = hits_found.saturating_add(1);
                emit(TimelineSearchProgressEvent {
                    operation_id: query.operation_id.clone(),
                    phase: TimelineSearchPhase::Hit,
                    hit: Some(hit),
                    commits_scanned,
                    hits_found,
                    status: None,
                    error: None,
                });

                if hits_found as usize >= hit_limit {
                    return Ok((commits_scanned, hits_found));
                }
            }

            if commits_scanned.is_multiple_of(50) {
                emit(TimelineSearchProgressEvent {
                    operation_id: query.operation_id.clone(),
                    phase: TimelineSearchPhase::Progress,
                    hit: None,
                    commits_scanned,
                    hits_found,
                    status: Some(format!("Scanned {commits_scanned} commits")),
                    error: None,
                });
            }
        }

        Ok((commits_scanned, hits_found))
    }

    async fn grep_commit(
        &self,
        runner: &GitCommandRunner,
        query: &TimelineSearchQuery,
        commit_hash: &str,
        options: GitRunOptions,
    ) -> Result<Vec<TimelineSearchHit>, String> {
        let mut args = vec!["grep".to_string(), "-n".to_string(), "-I".to_string()];
        if query.is_regex {
            args.push("-E".to_string());
        } else {
            args.push("-F".to_string());
        }
        args.push(query.query.clone());
        args.push(commit_hash.to_string());
        args.push("--".to_string());
        append_pathspecs(&mut args, &query.file_patterns);

        let output = runner
            .run_with_options(
                &args.iter().map(String::as_str).collect::<Vec<_>>(),
                options.allow_exit_codes(&[1]),
            )
            .await?;

        if output.trim().is_empty() {
            return Ok(Vec::new());
        }

        let header = load_commit_header(runner, commit_hash).await?;
        let mut hits = Vec::new();
        for line in output.lines() {
            if let Some(hit) = parse_grep_hit_line(line, commit_hash, &header) {
                hits.push(hit);
            }
        }

        Ok(hits)
    }
}

async fn load_commit_header(
    runner: &GitCommandRunner,
    commit_hash: &str,
) -> Result<crate::parsers::timeline_search::CommitHeader, String> {
    use crate::parsers::timeline_search::CommitHeader;

    let args = [
        "log",
        "-1",
        "--pretty=format:%H%n%an%n%ae%n%at%n%s",
        commit_hash,
    ];
    let output = runner
        .run_with_options(&args, GitRunOptions::default_read())
        .await?;

    let mut lines = output.lines();
    Ok(CommitHeader {
        hash: lines.next().unwrap_or_default().to_string(),
        author_name: lines.next().unwrap_or_default().to_string(),
        author_email: lines.next().unwrap_or_default().to_string(),
        commit_time: lines
            .next()
            .and_then(|value| value.parse().ok())
            .unwrap_or(0),
        subject: lines.next().unwrap_or_default().to_string(),
    })
}

fn build_pickaxe_log_args(query: &TimelineSearchQuery) -> Result<Vec<String>, String> {
    if query.query.trim().is_empty() {
        return Err("Search query cannot be empty".to_string());
    }

    let mut args = vec![
        "log".to_string(),
        "-p".to_string(),
        "--all".to_string(),
        "--date-order".to_string(),
        "--pretty=format:%H%n%an%n%ae%n%at%n%s".to_string(),
    ];

    if query.is_regex {
        args.push("-G".to_string());
    } else {
        args.push("-S".to_string());
    }
    args.push(query.query.clone());

    append_revision_filters(&mut args, query);
    args.push("--".to_string());
    append_pathspecs(&mut args, &query.file_patterns);

    Ok(args)
}

fn build_rev_list_args(query: &TimelineSearchQuery) -> Result<Vec<String>, String> {
    if query.query.trim().is_empty() {
        return Err("Search query cannot be empty".to_string());
    }

    let mut args = vec!["rev-list".to_string(), "--all".to_string()];
    append_revision_filters(&mut args, query);
    Ok(args)
}

fn append_revision_filters(args: &mut Vec<String>, query: &TimelineSearchQuery) {
    if let Some(author) = query.author.as_ref().map(|value| value.trim()).filter(|v| !v.is_empty())
    {
        args.push(format!("--author={author}"));
    }
    if let Some(since) = query.since.as_ref().map(|value| value.trim()).filter(|v| !v.is_empty()) {
        args.push(format!("--since={since}"));
    }
    if let Some(until) = query.until.as_ref().map(|value| value.trim()).filter(|v| !v.is_empty()) {
        args.push(format!("--until={until}"));
    }
}

fn is_commit_hash_line(line: &str) -> bool {
    let trimmed = line.trim();
    trimmed.len() == 40 && trimmed.chars().all(|ch| ch.is_ascii_hexdigit())
}

fn append_pathspecs(args: &mut Vec<String>, patterns: &[String]) {
    let normalized: Vec<String> = patterns
        .iter()
        .map(|pattern| pattern.trim())
        .filter(|pattern| !pattern.is_empty())
        .map(|pattern| pattern.to_string())
        .collect();

    if normalized.is_empty() {
        args.push(".".to_string());
        return;
    }

    args.extend(normalized);
}