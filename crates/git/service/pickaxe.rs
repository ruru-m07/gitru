use std::collections::{HashMap, HashSet};
use std::sync::{
    Arc, Mutex, OnceLock,
    atomic::{AtomicBool, Ordering},
};

use tauri::{AppHandle, Emitter};

use crate::{
    context::RepoContext,
    models::pickaxe::{PickaxeHit, PickaxePhase, PickaxeProgressEvent, PickaxeQuery},
    parsers::pickaxe::PickaxeStreamParser,
    runner::{GitCommandRunner, GitRunOptions},
};

pub const PICKAXE_PROGRESS_EVENT: &str = "git://pickaxe-progress";

const DEFAULT_HIT_LIMIT: usize = 500;
const SEARCH_TIMEOUT_SECS: u64 = 600;

static ACTIVE_SEARCH_CANCEL_FLAGS: OnceLock<Mutex<HashMap<String, Arc<AtomicBool>>>> =
    OnceLock::new();

fn active_search_cancel_flags() -> &'static Mutex<HashMap<String, Arc<AtomicBool>>> {
    ACTIVE_SEARCH_CANCEL_FLAGS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn hit_dedupe_key(hit: &PickaxeHit) -> String {
    format!("{}:{}", hit.commit_hash, hit.file_path)
}

fn should_emit_hit(seen: &mut HashSet<String>, hit: &PickaxeHit) -> bool {
    seen.insert(hit_dedupe_key(hit))
}

pub struct PickaxeService {
    ctx: Arc<RepoContext>,
}

impl PickaxeService {
    pub fn new(ctx: Arc<RepoContext>) -> Self {
        Self { ctx }
    }

    pub fn cancel_search(operation_id: &str) -> Result<bool, String> {
        let flag = {
            let active = active_search_cancel_flags()
                .lock()
                .map_err(|_| "Failed to access active pickaxe searches".to_string())?;
            active.get(operation_id).cloned()
        };

        let Some(flag) = flag else {
            return Ok(false);
        };

        flag.store(true, Ordering::Relaxed);
        Ok(true)
    }

    pub async fn start_search(&self, query: PickaxeQuery, app: AppHandle) -> Result<(), String> {
        let cancel_flag = Arc::new(AtomicBool::new(false));
        {
            let mut active = active_search_cancel_flags()
                .lock()
                .map_err(|_| "Failed to access active pickaxe searches".to_string())?;
            active.insert(query.operation_id.clone(), cancel_flag.clone());
        }

        let emit = |event: PickaxeProgressEvent| {
            let _ = app.emit(PICKAXE_PROGRESS_EVENT, event);
        };

        emit(PickaxeProgressEvent {
            operation_id: query.operation_id.clone(),
            phase: PickaxePhase::Started,
            hit: None,
            commits_scanned: 0,
            hits_found: 0,
            status: Some("Starting pickaxe".to_string()),
            error: None,
        });

        let result = self
            .run_pickaxe_search(&query, cancel_flag.clone(), emit)
            .await;

        {
            let mut active = active_search_cancel_flags()
                .lock()
                .map_err(|_| "Failed to access active pickaxe searches".to_string())?;
            active.remove(&query.operation_id);
        }

        if cancel_flag.load(Ordering::Relaxed) {
            emit(PickaxeProgressEvent {
                operation_id: query.operation_id.clone(),
                phase: PickaxePhase::Cancelled,
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
                emit(PickaxeProgressEvent {
                    operation_id: query.operation_id.clone(),
                    phase: PickaxePhase::Finished,
                    hit: None,
                    commits_scanned,
                    hits_found,
                    status: Some(format!("Found {hits_found} hits in {commits_scanned} commits")),
                    error: None,
                });
                Ok(())
            }
            Err(error) => {
                emit(PickaxeProgressEvent {
                    operation_id: query.operation_id,
                    phase: PickaxePhase::Error,
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
        query: &PickaxeQuery,
        cancel_flag: Arc<AtomicBool>,
        mut emit: F,
    ) -> Result<(u32, u32), String>
    where
        F: FnMut(PickaxeProgressEvent),
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
                        emit(PickaxeProgressEvent {
                            operation_id: query.operation_id.clone(),
                            phase: PickaxePhase::Hit,
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
                        emit(PickaxeProgressEvent {
                            operation_id: query.operation_id.clone(),
                            phase: PickaxePhase::Progress,
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
            emit(PickaxeProgressEvent {
                operation_id: query.operation_id.clone(),
                phase: PickaxePhase::Hit,
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

}

fn escape_regex_literal(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '\\' | '.' | '+' | '*' | '?' | '|' | '(' | ')' | '[' | ']' | '{' | '}' | '^' | '$' => {
                out.push('\\');
                out.push(ch);
            }
            _ => out.push(ch),
        }
    }
    out
}

fn resolve_pickaxe_search_spec(query: &PickaxeQuery) -> Result<(&'static str, String), String> {
    let text = query.query.trim();
    if text.is_empty() {
        return Err("Search query cannot be empty".to_string());
    }

    let can_use_pickaxe_s = !query.is_regex && query.match_case && !query.match_whole_word;
    if can_use_pickaxe_s {
        return Ok(("-S", text.to_string()));
    }

    let pattern = if query.is_regex {
        let mut pattern = text.to_string();
        if !query.match_case && !pattern.contains("(?") {
            pattern = format!("(?i){pattern}");
        }
        if query.match_whole_word {
            pattern = format!(r"\b(?:{pattern})\b");
        }
        pattern
    } else {
        let mut pattern = escape_regex_literal(text);
        if query.match_whole_word {
            pattern = format!(r"\b{pattern}\b");
        }
        if !query.match_case {
            pattern = format!("(?i){pattern}");
        }
        pattern
    };

    Ok(("-G", pattern))
}

fn build_pickaxe_log_args(query: &PickaxeQuery) -> Result<Vec<String>, String> {
    let (flag, pattern) = resolve_pickaxe_search_spec(query)?;

    let mut args = vec![
        "log".to_string(),
        "-p".to_string(),
        "--all".to_string(),
        "--date-order".to_string(),
        "--pretty=format:%H%n%an%n%ae%n%at%n%s".to_string(),
        flag.to_string(),
        pattern,
    ];

    append_revision_filters(&mut args, query);
    args.push("--".to_string());
    append_pathspecs(&mut args, &query.file_patterns);

    Ok(args)
}

fn append_revision_filters(args: &mut Vec<String>, query: &PickaxeQuery) {
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