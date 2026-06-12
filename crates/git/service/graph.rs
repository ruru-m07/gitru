use std::collections::HashMap;
use std::process::Command;
use std::sync::{Mutex, OnceLock};

use crate::models::commit::{Author, CommitAuthors, CommitStats, FullCommitInfo};
use crate::models::graph::GraphRefKind as GraphRefType;
use crate::models::graph::{
    GraphLogEntry, GraphRefKind, GraphSearchKey, GraphSearchQuery, HistoryQuery,
};
use crate::models::history::{
    CommitOverview, GraphPaging, GraphRef as GraphRefDto, GraphRow, GraphRowType, HistoryGraphResponse, ParentEdge,
    Swimlane as SwimlaneDto,
};
use crate::parsers::graph::{LOG_FORMAT, parse_log_entries, parse_search_query};
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
        let propagated_branch_refs = self.merge_branch_refs(
            &current_lane_branch_refs,
            entry_branch_refs,
            false,
        );
        let row_branch_refs = self.merge_branch_refs(
            &current_lane_branch_refs,
            entry_branch_refs,
            true,
        );

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

#[derive(Debug, Clone)]
struct SearchContext {
    name: Option<String>,
    email: Option<String>,
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

    let search_query = query.search.as_deref().map(parse_search_query);
    let search_context = search_query
        .as_ref()
        .and_then(|_| resolve_search_context(repo_path));
    let remote_refs = list_remote_refs(repo_path)?;

    let mut rows = Vec::with_capacity(query.limit);
    let mut exhausted = false;

    while rows.len() < query.limit {
        let batch_limit = DEFAULT_CHUNK_SIZE.min(query.limit * 2).max(50);
        let entries = fetch_log_entries(repo_path, query, cursor, batch_limit, &remote_refs)?;
        let batch_count = entries.len();

        if batch_count == 0 {
            break;
        }

        let mut reached_limit = false;

        let stats_map = fetch_shortstat_map(repo_path, query, cursor, entries.len())?;

        for entry in entries {
            cursor += 1;

            let is_stash = entry
                .refs
                .iter()
                .any(|r| matches!(r.kind, GraphRefKind::Stash));
            let branch_refs = build_branch_refs(&entry);
            let result =
                graph_state.process_commit(&entry.id, &entry.parents, is_stash, &branch_refs);

            if let Some(ref query) = search_query
                && !matches_search(&entry, query, search_context.as_ref())
            {
                continue;
            }

            let stats = stats_map
                .get(entry.id.as_str())
                .map(copy_stats)
                .unwrap_or(CommitStats {
                    insertions: 0,
                    deletions: 0,
                    files_changed: 0,
                });

            let commit_info = build_full_commit_info(&entry, stats);
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

/// Lightweight overview for the commit activity chart/minimap.
/// Returns a compact series (parallel arrays) + metadata.
/// Uses the *same* revision filter as history_graph.
/// When a search is active we apply the identical post-filter (matches_search) so that
/// the chart domain + indices line up with what the list will display.
/// All graph topology / swimlane / heavy per-row work is skipped.
pub fn history_overview(
    repo_path: &str,
    query: &HistoryQuery,
) -> Result<CommitOverview, String> {
    let search_query = query.search.as_deref().map(parse_search_query);
    let search_context = search_query
        .as_ref()
        .and_then(|_| resolve_search_context(repo_path));
    let remote_refs = list_remote_refs(repo_path)?;

    // Fast path when no search: we can use cheap rev-list count + ordered ids.
    // We cap the series at a few thousand points for responsiveness on large histories.
    // The returned `total` is the true count (for scaling / future full overview).
    // The series always covers the most recent commits (index 0..series_len).
    const MAX_OVERVIEW_SERIES: usize = 8192;

    if search_query.is_none() {
        let rev_args = build_revision_args(repo_path, query)?;
        let total = rev_list_count(repo_path, &rev_args)?;
        if total == 0 {
            let _head = current_head_oid(repo_path).unwrap_or_default();
            return Ok(CommitOverview {
                total: 0,
                head_index: 0,
                insertions: Vec::new(),
                deletions: Vec::new(),
                timestamps: None,
                oids: None,
            });
        }

        let series_len = total.min(MAX_OVERVIEW_SERIES);
        let head_oid = current_head_oid(repo_path)?;
        let ids_in_order = fetch_ordered_ids(repo_path, &rev_args, series_len)?;
        let head_index = ids_in_order
            .iter()
            .position(|id| id == &head_oid)
            .unwrap_or(0);

        let stats_map = fetch_shortstat_map(repo_path, query, 0, series_len)?;

        let mut insertions = Vec::with_capacity(series_len);
        let mut deletions = Vec::with_capacity(series_len);
        for id in &ids_in_order {
            let st = stats_map.get(id).cloned().unwrap_or(CommitStats {
                insertions: 0,
                deletions: 0,
                files_changed: 0,
            });
            insertions.push(st.insertions as u32);
            deletions.push(st.deletions as u32);
        }

        return Ok(CommitOverview {
            total,
            head_index,
            insertions,
            deletions,
            timestamps: None,
            oids: None,
        });
    }

    // Search path: must apply the same filter the list uses so indices align.
    // We still avoid GraphState, swimlanes, FullCommitInfo.files, etc.
    let mut ids_in_order: Vec<String> = Vec::new();
    let mut stats_map: std::collections::HashMap<String, CommitStats> = std::collections::HashMap::new();

    // Collect in batches (reuse the chunking style from history_graph).
    let mut cursor: usize = 0;
    const BATCH: usize = 500;

    loop {
        let entries = fetch_log_entries(repo_path, query, cursor, BATCH, &remote_refs)?;
        if entries.is_empty() {
            break;
        }

        // Stats for this batch
        let batch_stats = fetch_shortstat_map(repo_path, query, cursor, entries.len())?;

        for entry in &entries {
            cursor += 1;

            if let Some(ref sq) = search_query {
                if !matches_search(entry, sq, search_context.as_ref()) {
                    continue;
                }
            }

            let st = batch_stats
                .get(&entry.id)
                .cloned()
                .unwrap_or(CommitStats {
                    insertions: 0,
                    deletions: 0,
                    files_changed: 0,
                });

            ids_in_order.push(entry.id.clone());
            stats_map.insert(entry.id.clone(), st);

            // Hard safety cap for pathological cases (search that matches almost everything).
            if ids_in_order.len() >= 200_000 {
                break;
            }
        }

        if entries.len() < BATCH {
            break;
        }
        if ids_in_order.len() >= 200_000 {
            break;
        }
    }

    let total = ids_in_order.len();
    if total == 0 {
        return Ok(CommitOverview {
            total: 0,
            head_index: 0,
            insertions: Vec::new(),
            deletions: Vec::new(),
            timestamps: None,
            oids: None,
        });
    }

    let head_oid = current_head_oid(repo_path).ok();
    let head_index = if let Some(h) = head_oid {
        ids_in_order.iter().position(|id| id == &h).unwrap_or(0)
    } else {
        0
    };

    let mut insertions = Vec::with_capacity(total);
    let mut deletions = Vec::with_capacity(total);
    for id in &ids_in_order {
        let st = stats_map.get(id).cloned().unwrap_or(CommitStats {
            insertions: 0,
            deletions: 0,
            files_changed: 0,
        });
        insertions.push(st.insertions as u32);
        deletions.push(st.deletions as u32);
    }

    Ok(CommitOverview {
        total,
        head_index,
        insertions,
        deletions,
        timestamps: None,
        oids: None,
    })
}

fn current_head_oid(repo_path: &str) -> Result<String, String> {
    let output = run_git_command(repo_path, &["rev-parse", "HEAD"])?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).to_string());
    }
    let oid = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if oid.len() == 40 && oid.chars().all(|c| c.is_ascii_hexdigit()) {
        Ok(oid)
    } else {
        Err("invalid HEAD oid".to_string())
    }
}

fn rev_list_count(repo_path: &str, rev_args: &[String]) -> Result<usize, String> {
    let mut args = vec!["rev-list".to_string(), "--count".to_string()];
    args.extend(rev_args.iter().cloned());
    let output = run_git(repo_path, &mut args)?;
    let s = output.trim();
    s.parse::<usize>().map_err(|e| format!("failed to parse rev-list --count: {e}"))
}

fn fetch_ordered_ids(
    repo_path: &str,
    rev_args: &[String],
    total: usize,
) -> Result<Vec<String>, String> {
    if total == 0 {
        return Ok(Vec::new());
    }
    let mut args = vec![
        "log".to_string(),
        "--date-order".to_string(),
        "--no-color".to_string(),
        "--pretty=format:%H".to_string(),
    ];
    // -n total (or omit if we trust rev-args to bound; safer with explicit)
    args.push(format!("-n{}", total));
    args.extend(rev_args.iter().cloned());

    let output = run_git(repo_path, &mut args)?;
    let mut ids = Vec::with_capacity(total);
    for line in output.lines() {
        let line = line.trim();
        if line.len() == 40 && line.chars().all(|c| c.is_ascii_hexdigit()) {
            ids.push(line.to_string());
        }
    }
    Ok(ids)
}

fn fetch_log_entries(
    repo_path: &str,
    query: &HistoryQuery,
    skip: usize,
    limit: usize,
    remote_refs: &[String],
) -> Result<Vec<GraphLogEntry>, String> {
    let mut args = build_log_args(repo_path, query, skip, limit)?;
    let output = run_git(repo_path, &mut args)?;
    let mut entries = parse_log_entries(&output);
    normalize_refs(&mut entries, remote_refs);
    Ok(entries)
}

fn fetch_shortstat_map(
    repo_path: &str,
    query: &HistoryQuery,
    skip: usize,
    limit: usize,
) -> Result<HashMap<String, CommitStats>, String> {
    let mut args = build_shortstat_args(repo_path, query, skip, limit)?;
    let output = run_git(repo_path, &mut args)?;
    Ok(parse_shortstat_output(&output))
}

fn build_log_args(
    repo_path: &str,
    query: &HistoryQuery,
    skip: usize,
    limit: usize,
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

    let revs = build_revision_args(repo_path, query)?;
    args.extend(revs);

    Ok(args)
}

fn build_shortstat_args(
    repo_path: &str,
    query: &HistoryQuery,
    skip: usize,
    limit: usize,
) -> Result<Vec<String>, String> {
    let mut args = vec![
        "log".to_string(),
        "--date-order".to_string(),
        "--no-color".to_string(),
        format!("--pretty=format:%H"),
        "--shortstat".to_string(),
        format!("--skip={}", skip),
        format!("-n{}", limit),
    ];

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

fn copy_stats(stats: &CommitStats) -> CommitStats {
    CommitStats {
        insertions: stats.insertions,
        deletions: stats.deletions,
        files_changed: stats.files_changed,
    }
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
        "{}|{}|{}|l:{}|r:{}|t:{}|s:{}",
        repo_path,
        query.branch.as_deref().unwrap_or("all"),
        query.search.as_deref().unwrap_or(""),
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
    let remote_refs = list_remote_refs(repo_path)?;

    while skip < cursor {
        let limit = DEFAULT_CHUNK_SIZE.min(cursor - skip).max(50);
        let entries = fetch_log_entries(repo_path, query, skip, limit, &remote_refs)?;
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
        .filter(|reference| {
            matches!(reference.kind, GraphRefKind::Local | GraphRefKind::Remote)
        })
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

fn matches_search(
    entry: &GraphLogEntry,
    query: &GraphSearchQuery,
    context: Option<&SearchContext>,
) -> bool {
    if query.terms.is_empty() {
        return true;
    }

    for term in &query.terms {
        let matched = matches_term(entry, term, context);
        if term.negated {
            if matched {
                return false;
            }
        } else if !matched {
            return false;
        }
    }

    true
}

fn matches_term(
    entry: &GraphLogEntry,
    term: &crate::models::graph::GraphSearchTerm,
    context: Option<&SearchContext>,
) -> bool {
    let value = term.value.to_ascii_lowercase();

    if value == "@me"
        && let Some(context) = context
    {
        return matches_me(entry, context);
    }

    match term.key {
        Some(GraphSearchKey::Author) => matches_author(entry, &value),
        Some(GraphSearchKey::Committer) => matches_committer(entry, &value),
        Some(GraphSearchKey::Message) => matches_message(entry, &value),
        Some(GraphSearchKey::Ref) => matches_ref(entry, &value),
        Some(GraphSearchKey::Id) => entry.id.to_ascii_lowercase().starts_with(&value),
        None => {
            matches_message(entry, &value)
                || matches_author(entry, &value)
                || matches_committer(entry, &value)
                || matches_ref(entry, &value)
                || entry.id.to_ascii_lowercase().starts_with(&value)
        }
    }
}

fn matches_author(entry: &GraphLogEntry, value: &str) -> bool {
    entry.author_name.to_ascii_lowercase().contains(value)
        || entry.author_email.to_ascii_lowercase().contains(value)
}

fn matches_committer(entry: &GraphLogEntry, value: &str) -> bool {
    entry.committer_name.to_ascii_lowercase().contains(value)
        || entry.committer_email.to_ascii_lowercase().contains(value)
}

fn matches_message(entry: &GraphLogEntry, value: &str) -> bool {
    entry.summary.to_ascii_lowercase().contains(value)
        || entry.body.to_ascii_lowercase().contains(value)
}

fn matches_ref(entry: &GraphLogEntry, value: &str) -> bool {
    entry
        .refs
        .iter()
        .any(|reference| reference.name.to_ascii_lowercase().contains(value))
}

fn matches_me(entry: &GraphLogEntry, context: &SearchContext) -> bool {
    if let Some(name) = context.name.as_ref() {
        let name = name.to_ascii_lowercase();
        if matches_author(entry, &name) || matches_committer(entry, &name) {
            return true;
        }
    }

    if let Some(email) = context.email.as_ref() {
        let email = email.to_ascii_lowercase();
        if matches_author(entry, &email) || matches_committer(entry, &email) {
            return true;
        }
    }

    false
}

fn resolve_search_context(repo_path: &str) -> Option<SearchContext> {
    let name = git_config_value(repo_path, "user.name");
    let email = git_config_value(repo_path, "user.email");

    if name.is_none() && email.is_none() {
        return None;
    }

    Some(SearchContext { name, email })
}

fn git_config_value(repo_path: &str, key: &str) -> Option<String> {
    let output = run_git_command(repo_path, &["config", "--get", key]).ok()?;

    if !output.status.success() {
        return None;
    }

    let value = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if value.is_empty() { None } else { Some(value) }
}

fn list_remote_refs(repo_path: &str) -> Result<Vec<String>, String> {
    let output = run_git_command(
        repo_path,
        &["for-each-ref", "--format=%(refname)", "refs/remotes"],
    )?;

    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).to_string());
    }

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    Ok(stdout
        .lines()
        .map(|line| line.trim().to_string())
        .filter(|line| !line.is_empty())
        .collect())
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

fn normalize_refs(entries: &mut [GraphLogEntry], remote_refs: &[String]) {
    for entry in entries.iter_mut() {
        for reference in entry.refs.iter_mut() {
            if reference.kind != GraphRefKind::Other {
                continue;
            }

            let is_remote = remote_refs.iter().any(|remote_ref| reference.name == *remote_ref);

            reference.kind = if is_remote {
                GraphRefKind::Remote
            } else {
                GraphRefKind::Local
            };
        }
    }
}
