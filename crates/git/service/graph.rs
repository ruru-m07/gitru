use std::collections::HashMap;
use std::process::Command;
use std::sync::{Mutex, OnceLock};

use crate::models::commit::{Author, CommitAuthors, CommitStats, FullCommitInfo};
use crate::models::graph::GraphRefKind as GraphRefType;
use crate::models::graph::{CommitActivityQuery, GraphLogEntry, GraphRefKind, HistoryQuery};
use crate::models::history::{
    CommitActivityItem, CommitActivityResponse, GraphPaging, GraphRef as GraphRefDto, GraphRow,
    GraphRowType, HistoryGraphResponse, ParentEdge, Swimlane as SwimlaneDto,
};
use crate::parsers::graph::{LOG_FORMAT, parse_log_entries};
use crate::runner::{git_binary_path, git_path_env};

const DEFAULT_CHUNK_SIZE: usize = 200;

// TODO(phase2): migrate graph git invocations/cache to shared GitCommandRunner + RepoCache.
struct GraphCacheEntry {
    state: GraphState,
    last_cursor: usize,
}

static GRAPH_CACHE: OnceLock<Mutex<HashMap<String, GraphCacheEntry>>> = OnceLock::new();

fn graph_cache() -> &'static Mutex<HashMap<String, GraphCacheEntry>> {
    GRAPH_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// A single swimlane in the graph state.
#[derive(Debug, Clone)]
struct Swimlane {
    /// The commit OID this lane is heading toward.
    id: String,
    /// Color index (rotated through a palette on the frontend).
    color: usize,
    /// Whether this lane carries a stash flow (dashed rendering).
    is_stash: bool,
    /// Branch refs that should continue to the next commit on this lane.
    branch_refs: Vec<GraphRefDto>,
}

const NUM_COLORS: usize = 10;

/// Graph state using the VS Code swimlane-compaction model.
///
/// Swimlanes are stored in a `Vec<Swimlane>` where position = column.
/// When a branch merges and a lane is freed, the lane is **removed** from the
/// vector (everything to the right shifts left).  New branches are always
/// appended at the end.  This keeps the graph narrow while giving each branch
/// its own dedicated column.
#[derive(Debug, Clone)]
struct GraphState {
    /// Current swimlanes — position in this vec = column index.
    lanes: Vec<Swimlane>,
    /// Global color counter — incremented each time a brand-new branch color
    /// is needed, then taken `mod NUM_COLORS`.
    color_counter: usize,
}

/// The result of processing a single commit.
struct ProcessResult {
    /// Column index where this commit's dot should be drawn.
    lane: usize,
    /// Parent edges: (parent_oid, column_in_output_swimlanes).
    parent_edges: Vec<(String, usize)>,
    /// Branch refs that apply to the current commit row.
    branch_refs: Vec<GraphRefDto>,
    /// Swimlanes entering this row (snapshot *before* processing).
    input_swimlanes: Vec<Swimlane>,
    /// Swimlanes leaving this row (snapshot *after* processing).
    output_swimlanes: Vec<Swimlane>,
}

impl GraphState {
    fn new() -> Self {
        Self {
            lanes: Vec::new(),
            color_counter: 0,
        }
    }

    fn next_color(&mut self) -> usize {
        let c = self.color_counter % NUM_COLORS;
        self.color_counter += 1;
        c
    }

    fn merge_branch_refs(
        &self,
        existing: &[GraphRefDto],
        entry_refs: &[GraphRefDto],
        keep_head_flags: bool,
    ) -> Vec<GraphRefDto> {
        let mut refs = Vec::new();
        let mut seen = std::collections::HashSet::new();

        for reference in entry_refs.iter().chain(existing.iter()) {
            let key = (reference.kind, reference.name.as_str());
            if !seen.insert(key) {
                continue;
            }

            refs.push(GraphRefDto {
                name: reference.name.clone(),
                display_name: reference.display_name.clone(),
                kind: reference.kind,
                is_head: keep_head_flags && reference.is_head,
            });
        }

        refs
    }

    /// Process a commit and produce input/output swimlane snapshots.
    ///
    /// This follows the same algorithm as VS Code's
    /// `toISCMHistoryItemViewModelArray`:
    ///
    /// 1. `input = clone(self.lanes)`  (previous output becomes this row's input)
    /// 2. Walk input lanes:
    ///    - If lane targets this commit id → first match: replace with
    ///      first parent (lane reuse); subsequent matches: drop (compaction).
    ///    - Otherwise: pass through.
    /// 3. Append new lanes for any additional (secondary) parents.
    /// 4. `self.lanes = output`
    fn process_commit(
        &mut self,
        oid: &str,
        parents: &[String],
        is_stash: bool,
        entry_branch_refs: &[GraphRefDto],
    ) -> ProcessResult {
        let input_swimlanes = self.lanes.clone();
        let current_lane_branch_refs = self
            .lanes
            .iter()
            .find(|lane| lane.id == oid)
            .map(|lane| lane.branch_refs.clone())
            .unwrap_or_default();

        // Find this commit's position in the input lanes
        let commit_index = self.lanes.iter().position(|l| l.id == oid);
        // If not found: this is a new root/branch tip — will be appended
        let commit_lane_in_input = commit_index.unwrap_or(self.lanes.len());

        let mut output: Vec<Swimlane> = Vec::new();
        let mut first_parent_added = false;
        let mut commit_lane_in_output = commit_lane_in_input;
        let propagated_branch_refs =
            self.merge_branch_refs(&current_lane_branch_refs, entry_branch_refs, false);
        let row_branch_refs =
            self.merge_branch_refs(&current_lane_branch_refs, entry_branch_refs, true);

        if !parents.is_empty() {
            // Walk existing lanes, building the output
            for node in &self.lanes {
                if node.id == oid {
                    if !first_parent_added {
                        // Reuse this lane for the first parent
                        let is_stash_carry = is_stash;
                        commit_lane_in_output = output.len();
                        output.push(Swimlane {
                            id: parents[0].clone(),
                            color: node.color,
                            is_stash: is_stash_carry,
                            branch_refs: propagated_branch_refs.clone(),
                        });
                        first_parent_added = true;
                    }
                    // Subsequent lanes targeting the same commit are dropped
                    // (compaction — this is how merged branches free up columns)
                    continue;
                }

                // Non-stash carry-through: if this lane was a stash carry and
                // we encounter its target commit, clear the stash flag
                let mut cloned = node.clone();
                if cloned.is_stash && cloned.id == parents.first().map(String::as_str).unwrap_or("")
                {
                    cloned.is_stash = false;
                }
                output.push(cloned);
            }
        } else {
            // Root / orphan commit — drop the lane that targeted this commit
            for node in &self.lanes {
                if node.id == oid {
                    continue;
                }
                output.push(node.clone());
            }
        }

        // If this commit wasn't found in any lane (new branch tip),
        // and it has parents, add the first parent as a new lane
        if !first_parent_added && !parents.is_empty() {
            commit_lane_in_output = output.len();
            let color = self.next_color();
            output.push(Swimlane {
                id: parents[0].clone(),
                color,
                is_stash,
                branch_refs: propagated_branch_refs.clone(),
            });
        }

        // Add new lanes for secondary parents (merge parents)
        let mut parent_edges = Vec::new();
        if !parents.is_empty() {
            parent_edges.push((parents[0].clone(), commit_lane_in_output));
        }

        for parent in parents.iter().skip(1) {
            // Check if this parent already has a lane in the output
            let existing = output.iter().position(|l| l.id == *parent);
            if let Some(idx) = existing {
                parent_edges.push((parent.clone(), idx));
            } else {
                let color = self.next_color();
                let lane_idx = output.len();
                output.push(Swimlane {
                    id: parent.clone(),
                    color,
                    is_stash: false,
                    branch_refs: Vec::new(),
                });
                parent_edges.push((parent.clone(), lane_idx));
            }
        }

        // Clear stash flag on lanes whose target is reached by a non-stash commit
        if !is_stash {
            for lane in &mut output {
                if lane.is_stash && lane.id == oid {
                    lane.is_stash = false;
                }
            }
        }

        let output_swimlanes = output.clone();
        self.lanes = output;

        ProcessResult {
            lane: commit_lane_in_output,
            parent_edges,
            branch_refs: row_branch_refs,
            input_swimlanes,
            output_swimlanes,
        }
    }
}

pub fn history_graph(
    repo_path: &str,
    query: &HistoryQuery,
) -> Result<HistoryGraphResponse, String> {
    if query.limit == 0 {
        return Ok(HistoryGraphResponse {
            rows: Vec::new(),
            cursor: None,
            graph_state: None,
            has_more: false,
            paging: GraphPaging {
                starting_cursor: None,
                has_more: false,
            },
        });
    }

    let cache_key = build_cache_key(repo_path, query);
    let mut cursor = query
        .cursor
        .as_deref()
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(0);

    let mut graph_state = if query.cursor.is_none() {
        cursor = 0;
        let mut cache = graph_cache()
            .lock()
            .map_err(|_| "Graph cache lock poisoned")?;
        cache.remove(&cache_key);
        GraphState::new()
    } else {
        let cache = graph_cache()
            .lock()
            .map_err(|_| "Graph cache lock poisoned")?;
        if let Some(entry) = cache.get(&cache_key) {
            cursor = entry.last_cursor;
            entry.state.clone()
        } else {
            GraphState::new()
        }
    };

    if query.cursor.is_some() && graph_state.lanes.is_empty() && cursor > 0 {
        graph_state = rebuild_graph_state(repo_path, query, cursor)?;
    }

    let mut rows = Vec::with_capacity(query.limit);
    let mut exhausted = false;

    while rows.len() < query.limit {
        let batch_limit = query.limit - rows.len();
        let entries = fetch_log_entries(repo_path, query, cursor, batch_limit, true)?;
        let batch_count = entries.len();

        if batch_count == 0 {
            break;
        }

        let mut reached_limit = false;

        for entry in entries {
            cursor += 1;

            let is_stash = entry
                .refs
                .iter()
                .any(|r| matches!(r.kind, GraphRefKind::Stash));
            let branch_refs = build_branch_refs(&entry);
            let result =
                graph_state.process_commit(&entry.id, &entry.parents, is_stash, &branch_refs);

            let commit_info = build_full_commit_info(&entry, entry.stats);
            let refs = build_refs(&entry);
            let heads = filter_refs(&refs, GraphRefType::Local, true);
            let remotes = filter_refs(&refs, GraphRefType::Remote, false);
            let tags = filter_refs(&refs, GraphRefType::Tag, false);
            let stashes = filter_refs(&refs, GraphRefType::Stash, false);
            let row_type = if stashes.is_empty() {
                GraphRowType::Commit
            } else {
                GraphRowType::Stash
            };

            rows.push(GraphRow {
                oid: entry.id.clone(),
                lane: result.lane,
                commit: commit_info,
                parents: result
                    .parent_edges
                    .into_iter()
                    .map(|(parent_id, parent_lane)| ParentEdge {
                        oid: parent_id,
                        lane: parent_lane,
                    })
                    .collect(),
                r#type: row_type,
                refs,
                branch_refs: result.branch_refs,
                heads,
                remotes,
                tags,
                stashes,
                input_swimlanes: result
                    .input_swimlanes
                    .iter()
                    .map(|s| SwimlaneDto {
                        id: s.id.clone(),
                        color: s.color,
                        is_stash: s.is_stash,
                    })
                    .collect(),
                output_swimlanes: result
                    .output_swimlanes
                    .iter()
                    .map(|s| SwimlaneDto {
                        id: s.id.clone(),
                        color: s.color,
                        is_stash: s.is_stash,
                    })
                    .collect(),
            });

            if rows.len() >= query.limit {
                reached_limit = true;
                break;
            }
        }

        if batch_count < batch_limit && !reached_limit {
            exhausted = true;
        }

        if reached_limit || exhausted {
            break;
        }
    }

    let has_more = rows.len() >= query.limit && !exhausted;
    let next_cursor = if has_more {
        Some(cursor.to_string())
    } else {
        None
    };

    let mut cache = graph_cache()
        .lock()
        .map_err(|_| "Graph cache lock poisoned")?;
    if let Some(next_cursor) = next_cursor.as_ref() {
        cache.insert(
            cache_key,
            GraphCacheEntry {
                state: graph_state,
                last_cursor: next_cursor.parse::<usize>().unwrap_or_default(),
            },
        );
    } else {
        cache.remove(&cache_key);
    }

    Ok(HistoryGraphResponse {
        rows,
        cursor: next_cursor.clone(),
        graph_state: None,
        has_more,
        paging: GraphPaging {
            starting_cursor: next_cursor,
            has_more,
        },
    })
}

fn fetch_log_entries(
    repo_path: &str,
    query: &HistoryQuery,
    skip: usize,
    limit: usize,
    include_stats: bool,
) -> Result<Vec<GraphLogEntry>, String> {
    let mut args = build_log_args(repo_path, query, skip, limit, include_stats)?;
    let output = run_git(repo_path, &mut args)?;
    Ok(parse_log_entries(&output))
}

fn build_log_args(
    repo_path: &str,
    query: &HistoryQuery,
    skip: usize,
    limit: usize,
    include_stats: bool,
) -> Result<Vec<String>, String> {
    let mut args = vec![
        "log".to_string(),
        "--date-order".to_string(),
        "--parents".to_string(),
        "--decorate=full".to_string(),
        "--no-color".to_string(),
        format!("--pretty=format:{}", LOG_FORMAT),
        format!("--skip={}", skip),
        format!("-n{}", limit),
    ];

    if include_stats {
        args.push("--shortstat".to_string());
    }

    let revs = build_revision_args(repo_path, query)?;
    args.extend(revs);

    Ok(args)
}

fn build_revision_args(repo_path: &str, query: &HistoryQuery) -> Result<Vec<String>, String> {
    if let Some(branch) = query.branch.as_ref() {
        return Ok(vec![branch.clone()]);
    }

    let mut args = Vec::new();
    let mut has_any = false;

    if query.include_local {
        args.push("--branches".to_string());
        has_any = true;
    }

    if query.include_remotes {
        args.push("--remotes".to_string());
        has_any = true;
    }

    if query.include_tags {
        args.push("--tags".to_string());
        has_any = true;
    }

    if query.include_stash && has_ref(repo_path, "refs/stash")? {
        args.push("refs/stash".to_string());
        has_any = true;
    }

    if !has_any {
        args.push("HEAD".to_string());
    }

    Ok(args)
}

fn has_ref(repo_path: &str, reference: &str) -> Result<bool, String> {
    let output = run_git_command(repo_path, &["rev-parse", "--verify", reference])?;

    Ok(output.status.success())
}

fn parse_shortstat_output(output: &str) -> HashMap<String, CommitStats> {
    let mut stats_map = HashMap::new();
    let mut current_id: Option<String> = None;

    for line in output.lines() {
        let line = line.trim();
        if line.is_empty() {
            current_id = None;
            continue;
        }

        if is_commit_id(line) {
            current_id = Some(line.to_string());
            stats_map.entry(line.to_string()).or_insert(CommitStats {
                insertions: 0,
                deletions: 0,
                files_changed: 0,
            });
            continue;
        }

        if let Some(id) = current_id.as_ref() {
            let stats = parse_shortstat_line(line);
            stats_map.insert(id.clone(), stats);
        }
    }

    stats_map
}

fn parse_shortstat_line(line: &str) -> CommitStats {
    let mut stats = CommitStats {
        insertions: 0,
        deletions: 0,
        files_changed: 0,
    };

    for part in line.split(',') {
        let part = part.trim();
        let mut iter = part.split_whitespace();
        let count = match iter.next().and_then(|value| value.parse::<usize>().ok()) {
            Some(value) => value,
            None => continue,
        };

        let label = iter.collect::<Vec<_>>().join(" ");
        if label.starts_with("file") {
            stats.files_changed = count;
        } else if label.starts_with("insertion") {
            stats.insertions = count;
        } else if label.starts_with("deletion") {
            stats.deletions = count;
        }
    }

    stats
}

fn is_commit_id(value: &str) -> bool {
    value.len() == 40 && value.chars().all(|ch| ch.is_ascii_hexdigit())
}

fn run_git(repo_path: &str, args: &mut [String]) -> Result<String, String> {
    let ref_args = args.iter().map(String::as_str).collect::<Vec<_>>();
    let output = run_git_command(repo_path, &ref_args)?;

    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).to_string());
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

fn build_cache_key(repo_path: &str, query: &HistoryQuery) -> String {
    format!(
        "{}|{}|l:{}|r:{}|t:{}|s:{}",
        repo_path,
        query.branch.as_deref().unwrap_or("all"),
        query.include_local,
        query.include_remotes,
        query.include_tags,
        query.include_stash
    )
}

fn rebuild_graph_state(
    repo_path: &str,
    query: &HistoryQuery,
    cursor: usize,
) -> Result<GraphState, String> {
    let mut state = GraphState::new();
    let mut skip = 0usize;

    while skip < cursor {
        let limit = DEFAULT_CHUNK_SIZE.min(cursor - skip).max(50);
        let entries = fetch_log_entries(repo_path, query, skip, limit, false)?;
        if entries.is_empty() {
            break;
        }

        for entry in entries.iter() {
            let is_stash = entry
                .refs
                .iter()
                .any(|r| matches!(r.kind, GraphRefKind::Stash));
            let branch_refs = build_branch_refs(entry);
            let _ = state.process_commit(&entry.id, &entry.parents, is_stash, &branch_refs);
            skip += 1;
            if skip >= cursor {
                break;
            }
        }

        if entries.len() < limit {
            break;
        }
    }

    Ok(state)
}

fn build_full_commit_info(entry: &GraphLogEntry, stats: CommitStats) -> FullCommitInfo {
    let authors = CommitAuthors {
        author: Author {
            name: entry.author_name.clone(),
            email: entry.author_email.clone(),
        },
        committer: Author {
            name: entry.committer_name.clone(),
            email: entry.committer_email.clone(),
        },
        co_authors: extract_co_authors(&entry.body),
    };

    FullCommitInfo {
        id: entry.id.clone(),
        summary: entry.summary.clone(),
        body: entry.body.clone(),
        timestamp: entry.committer_time,
        authors,
        stats,
        files: Vec::new(),
    }
}

fn build_refs(entry: &GraphLogEntry) -> Vec<GraphRefDto> {
    entry
        .refs
        .iter()
        .map(|reference| GraphRefDto {
            name: reference.name.clone(),
            display_name: reference.display_name.clone(),
            kind: map_ref_kind(reference.kind),
            is_head: reference.is_head,
        })
        .collect()
}

fn build_branch_refs(entry: &GraphLogEntry) -> Vec<GraphRefDto> {
    entry
        .refs
        .iter()
        .filter(|reference| matches!(reference.kind, GraphRefKind::Local | GraphRefKind::Remote))
        .map(|reference| GraphRefDto {
            name: reference.name.clone(),
            display_name: reference.display_name.clone(),
            kind: map_ref_kind(reference.kind),
            is_head: reference.is_head,
        })
        .collect()
}

fn map_ref_kind(kind: GraphRefKind) -> GraphRefType {
    match kind {
        GraphRefKind::Local => GraphRefType::Local,
        GraphRefKind::Remote => GraphRefType::Remote,
        GraphRefKind::Tag => GraphRefType::Tag,
        GraphRefKind::Stash => GraphRefType::Stash,
        GraphRefKind::Other => GraphRefType::Other,
    }
}

fn filter_refs(refs: &[GraphRefDto], kind: GraphRefType, only_head: bool) -> Vec<GraphRefDto> {
    refs.iter()
        .filter(|reference| reference.kind == kind && (!only_head || reference.is_head))
        .cloned()
        .collect()
}

fn extract_co_authors(body: &str) -> Vec<Author> {
    let mut co_authors = Vec::new();

    for line in body.lines() {
        let line = line.trim();
        if (line.starts_with("Co-authored-by:") || line.starts_with("Co-Authored-By:"))
            && let Some(author) = parse_author_line(line)
        {
            co_authors.push(author);
        }
    }

    co_authors
}

fn parse_author_line(line: &str) -> Option<Author> {
    let line = line.split(':').nth(1)?.trim();

    if let Some(email_start) = line.rfind('<')
        && let Some(email_end) = line.rfind('>')
    {
        let name = line[..email_start].trim().to_string();
        let email = line[email_start + 1..email_end].trim().to_string();
        return Some(Author { name, email });
    }

    Some(Author {
        name: line.to_string(),
        email: String::new(),
    })
}

fn run_git_command(repo_path: &str, args: &[&str]) -> Result<std::process::Output, String> {
    let git_binary = git_binary_path()?;
    let git_path_env = git_path_env()?;
    Command::new(git_binary)
        .current_dir(repo_path)
        .env("PATH", git_path_env)
        .args(args)
        .output()
        .map_err(|e| e.to_string())
}

pub fn commit_activity(
    repo_path: &str,
    query: &CommitActivityQuery,
) -> Result<CommitActivityResponse, String> {
    let limit_str = query.limit.to_string();
    let revs = build_activity_revision_args(repo_path, query)?;

    // Pass 1: hashes + timestamps (no diff, very fast)
    let mut log_args = vec![
        "log".to_string(),
        "--date-order".to_string(),
        "--no-color".to_string(),
        "--pretty=format:%H %ct".to_string(),
        format!("-n{}", limit_str),
    ];
    log_args.extend(revs.clone());
    let log_output = run_git(repo_path, &mut log_args)?;

    let mut oid_timestamps: Vec<(String, i64)> = Vec::new();
    for line in log_output.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let mut parts = line.splitn(2, ' ');
        let oid = parts.next().unwrap_or("").to_string();
        let ts: i64 = parts.next().and_then(|s| s.parse().ok()).unwrap_or(0);
        oid_timestamps.push((oid, ts));
    }

    if oid_timestamps.is_empty() {
        return Ok(CommitActivityResponse {
            items: Vec::new(),
            head_index: None,
            total: 0,
        });
    }

    // Pass 2: shortstat for insertions/deletions
    let mut stat_args = vec![
        "log".to_string(),
        "--date-order".to_string(),
        "--no-color".to_string(),
        "--pretty=format:%H".to_string(),
        "--shortstat".to_string(),
        format!("-n{}", limit_str),
    ];
    stat_args.extend(revs);
    let stat_output = run_git(repo_path, &mut stat_args)?;
    let stats_map = parse_shortstat_output(&stat_output);

    // Pass 3: HEAD oid for marker
    let head_oid = get_head_oid(repo_path).ok();

    let mut head_index: Option<usize> = None;
    let items: Vec<CommitActivityItem> = oid_timestamps
        .iter()
        .enumerate()
        .map(|(i, (oid, ts))| {
            if head_oid.as_deref() == Some(oid.as_str()) {
                head_index = Some(i);
            }
            let (ins, del) = stats_map
                .get(oid.as_str())
                .map(|s| (s.insertions as u32, s.deletions as u32))
                .unwrap_or((0, 0));
            CommitActivityItem {
                oid: oid.clone(),
                timestamp: *ts,
                insertions: ins,
                deletions: del,
            }
        })
        .collect();

    let total = items.len();
    Ok(CommitActivityResponse {
        items,
        head_index,
        total,
    })
}

fn get_head_oid(repo_path: &str) -> Result<String, String> {
    let output = run_git_command(repo_path, &["rev-parse", "HEAD"])?;
    if !output.status.success() {
        return Err("Failed to get HEAD".to_string());
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn build_activity_revision_args(
    repo_path: &str,
    query: &CommitActivityQuery,
) -> Result<Vec<String>, String> {
    let mut args = Vec::new();
    let mut has_any = false;

    if query.include_local {
        args.push("--branches".to_string());
        has_any = true;
    }
    if query.include_remotes {
        args.push("--remotes".to_string());
        has_any = true;
    }
    if query.include_tags {
        args.push("--tags".to_string());
        has_any = true;
    }
    if query.include_stash && has_ref(repo_path, "refs/stash")? {
        args.push("refs/stash".to_string());
        has_any = true;
    }
    if !has_any {
        args.push("HEAD".to_string());
    }

    Ok(args)
}
