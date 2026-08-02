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
                    status: Some(format!(
                        "Found {hits_found} hits in {commits_scanned} commits"
                    )),
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
        let options = GitRunOptions::default_read()
            .with_timeout(std::time::Duration::from_secs(SEARCH_TIMEOUT_SECS));

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

fn wrap_git_regex_as_whole_word(pattern: String) -> String {
    format!(r"(^|[^[:alnum:]_]){pattern}([^[:alnum:]_]|$)")
}

enum PickaxeSearchMode {
    String,
    Regex,
}

struct PickaxeSearchSpec {
    mode: PickaxeSearchMode,
    ignore_case: bool,
    pattern: String,
}

fn resolve_pickaxe_search_spec(query: &PickaxeQuery) -> Result<PickaxeSearchSpec, String> {
    let text = query.query.trim();
    if text.is_empty() {
        return Err("Search query cannot be empty".to_string());
    }

    // Plain text without whole-word always uses -S (literal string pickaxe).
    // Never regex-escape here: -S matches the pattern literally, so escaping
    // would search for backslashes (e.g. "file.txt" → "file\.txt").
    if !query.is_regex && !query.match_whole_word {
        return Ok(PickaxeSearchSpec {
            mode: PickaxeSearchMode::String,
            ignore_case: !query.match_case,
            pattern: text.to_string(),
        });
    }

    let pattern = if query.is_regex {
        let mut pattern = text.to_string();
        if query.match_whole_word {
            pattern = wrap_git_regex_as_whole_word(pattern);
        }
        pattern
    } else {
        // Plain text + whole-word: escape for -G, then wrap word boundaries.
        let mut pattern = escape_regex_literal(text);
        pattern = wrap_git_regex_as_whole_word(pattern);
        pattern
    };

    Ok(PickaxeSearchSpec {
        mode: PickaxeSearchMode::Regex,
        ignore_case: !query.match_case,
        pattern,
    })
}

fn build_pickaxe_log_args(query: &PickaxeQuery) -> Result<Vec<String>, String> {
    let spec = resolve_pickaxe_search_spec(query)?;

    let mut args = vec![
        "log".to_string(),
        "-p".to_string(),
        "--all".to_string(),
        "--date-order".to_string(),
        "--pretty=format:%H%n%an%n%ae%n%at%n%s".to_string(),
    ];

    if spec.ignore_case {
        args.push("-i".to_string());
    }

    args.push(
        match spec.mode {
            PickaxeSearchMode::String => "-S",
            PickaxeSearchMode::Regex => "-G",
        }
        .to_string(),
    );

    args.push(spec.pattern);

    append_revision_filters(&mut args, query);
    args.push("--".to_string());
    append_pathspecs(&mut args, &query.file_patterns);

    Ok(args)
}

fn append_revision_filters(args: &mut Vec<String>, query: &PickaxeQuery) {
    if let Some(author) = query
        .author
        .as_ref()
        .map(|value| value.trim())
        .filter(|v| !v.is_empty())
    {
        args.push(format!("--author={author}"));
    }
    if let Some(since) = query
        .since
        .as_ref()
        .map(|value| value.trim())
        .filter(|v| !v.is_empty())
    {
        args.push(format!("--since={since}"));
    }
    if let Some(until) = query
        .until
        .as_ref()
        .map(|value| value.trim())
        .filter(|v| !v.is_empty())
    {
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

#[cfg(test)]
mod tests {
    use super::*;

    fn query(
        query: &str,
        is_regex: bool,
        match_case: bool,
        match_whole_word: bool,
    ) -> PickaxeQuery {
        PickaxeQuery {
            query: query.to_string(),
            is_regex,
            match_case,
            match_whole_word,
            author: None,
            since: None,
            until: None,
            file_patterns: vec![],
            limit: None,
            operation_id: "op-1".to_string(),
        }
    }

    #[test]
    fn plain_text_case_sensitive_uses_pickaxe_s() {
        let spec = resolve_pickaxe_search_spec(&query("AssetSuffix::Inferred", false, true, false))
            .expect("expected spec");

        assert!(matches!(spec.mode, PickaxeSearchMode::String));
        assert!(!spec.ignore_case);
        assert_eq!(spec.pattern, "AssetSuffix::Inferred");
    }

    #[test]
    fn plain_text_case_insensitive_uses_pickaxe_s_with_ignore_case() {
        let spec =
            resolve_pickaxe_search_spec(&query("AssetSuffix::Inferred", false, false, false))
                .expect("expected spec");

        assert!(matches!(spec.mode, PickaxeSearchMode::String));
        assert!(spec.ignore_case);
        assert_eq!(spec.pattern, "AssetSuffix::Inferred");

        let args = build_pickaxe_log_args(&query("AssetSuffix::Inferred", false, false, false))
            .expect("expected args");

        assert!(
            args.windows(2).any(|window| window == ["-i", "-S"]),
            "case-insensitive searches should use git's string pickaxe with -i"
        );
    }

    #[test]
    fn plain_text_case_insensitive_does_not_escape_regex_metacharacters() {
        // Regression: escaping for -S made queries like "file.txt" search for "file\.txt".
        let spec = resolve_pickaxe_search_spec(&query("file.txt", false, false, false))
            .expect("expected spec");

        assert!(matches!(spec.mode, PickaxeSearchMode::String));
        assert!(spec.ignore_case);
        assert_eq!(spec.pattern, "file.txt");

        let spec = resolve_pickaxe_search_spec(&query("foo()", false, false, false))
            .expect("expected spec");
        assert_eq!(spec.pattern, "foo()");

        let args =
            build_pickaxe_log_args(&query("a+b*c?", false, false, false)).expect("expected args");
        assert!(args.windows(2).any(|window| window == ["-i", "-S"]));
        assert!(args.iter().any(|arg| arg == "a+b*c?"));
        assert!(!args.iter().any(|arg| arg.contains('\\')));
    }

    #[test]
    fn whole_word_search_uses_portable_boundaries() {
        let spec = resolve_pickaxe_search_spec(&query("AssetSuffix::Inferred", false, false, true))
            .expect("expected spec");

        assert!(matches!(spec.mode, PickaxeSearchMode::Regex));
        assert!(spec.ignore_case);
        assert_eq!(
            spec.pattern,
            r"(^|[^[:alnum:]_])AssetSuffix::Inferred([^[:alnum:]_]|$)"
        );
    }

    #[test]
    fn whole_word_plain_text_escapes_metacharacters_for_regex() {
        let spec = resolve_pickaxe_search_spec(&query("file.txt", false, true, true))
            .expect("expected spec");

        assert!(matches!(spec.mode, PickaxeSearchMode::Regex));
        assert!(!spec.ignore_case);
        assert_eq!(spec.pattern, r"(^|[^[:alnum:]_])file\.txt([^[:alnum:]_]|$)");
    }

    #[test]
    fn regex_case_insensitive_keeps_pattern_and_uses_ignore_case() {
        let spec = resolve_pickaxe_search_spec(&query("AssetSuffix::Inferred", true, false, false))
            .expect("expected spec");

        assert!(matches!(spec.mode, PickaxeSearchMode::Regex));
        assert!(spec.ignore_case);
        assert_eq!(spec.pattern, "AssetSuffix::Inferred");
    }
}
