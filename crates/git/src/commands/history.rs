use std::collections::{HashMap, VecDeque};

use git2::Oid;
use serde::{Deserialize, Serialize};

use crate::{
    types::{CommitAuthors, CommitInfo},
    utils::{extract_all_authors, open_repository},
};

#[tauri::command]
#[logger::logger]
pub async fn history(
    repo_path: &str,
    skip: usize,
    limit: usize,
) -> Result<Vec<CommitInfo>, String> {
    let repo = open_repository(repo_path).map_err(|e| e.to_string())?;

    let mut revwalk = repo.revwalk().map_err(|e| e.to_string())?;

    revwalk.push_head().map_err(|e| e.to_string())?;

    let mut commits = Vec::with_capacity(limit);

    for oid in revwalk.skip(skip).take(limit) {
        let oid = oid.map_err(|e| e.to_string())?;
        let commit = repo.find_commit(oid).map_err(|e| e.to_string())?;

        commits.push(CommitInfo {
            id: oid.to_string(),
            summary: commit.summary().unwrap_or("").to_string(),
            body: commit.body().unwrap_or("").to_string(),
            timestamp: commit.time().seconds(),
            authors: extract_all_authors(&commit),
        });
    }

    Ok(commits)
}
// ============================================================================
// DATA STRUCTURES
// ============================================================================

#[derive(Clone, Serialize, Deserialize)]
pub struct GraphRow {
    pub oid: String,
    pub short_oid: String, // First 7 chars for display
    pub lane: usize,
    pub message: String,
    pub author: String,
    pub authors: CommitAuthors,
    pub timestamp: i64,
    pub parents: Vec<ParentEdge>,
    pub insertions: usize, // Lines added
    pub deletions: usize,  // Lines deleted
    pub branches: Vec<String>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct ParentEdge {
    pub oid: String,
    pub lane: usize,
}

#[derive(Serialize, Deserialize)]
pub struct HistoryResponse {
    pub rows: Vec<GraphRow>,
    pub cursor: Option<String>, // Next commit OID for pagination
    pub has_more: bool,
}

#[derive(Debug, Deserialize)]
pub struct HistoryQuery {
    pub cursor: Option<String>, // Start from this commit
    pub limit: usize,           // Number of commits to fetch
    pub search: Option<String>, // Search in message/description
}

// ============================================================================
// OPTIMIZED GRAPH BUILDER
// ============================================================================

struct LaneManager {
    // Maps OID -> lane index for O(1) lookup
    oid_to_lane: HashMap<Oid, usize>,
    // Tracks which lanes are currently free
    free_lanes: VecDeque<usize>,
    // Next lane to allocate if no free lanes
    next_lane: usize,
}

impl LaneManager {
    fn new() -> Self {
        Self {
            oid_to_lane: HashMap::new(),
            free_lanes: VecDeque::new(),
            next_lane: 0,
        }
    }

    fn get_or_allocate_lane(&mut self, oid: Oid) -> usize {
        if let Some(&lane) = self.oid_to_lane.get(&oid) {
            lane
        } else {
            let lane = self.free_lanes.pop_front().unwrap_or_else(|| {
                let lane = self.next_lane;
                self.next_lane += 1;
                lane
            });
            self.oid_to_lane.insert(oid, lane);
            lane
        }
    }

    fn release_lane(&mut self, oid: Oid) {
        if let Some(lane) = self.oid_to_lane.remove(&oid) {
            // Keep free lanes sorted for better visual consistency
            let pos = self
                .free_lanes
                .iter()
                .position(|&l| l > lane)
                .unwrap_or(self.free_lanes.len());
            self.free_lanes.insert(pos, lane);
        }
    }

    fn assign_parent(&mut self, parent_oid: Oid, preferred_lane: Option<usize>) -> usize {
        if let Some(lane) = preferred_lane {
            // First parent continues in same lane
            if let Some(existing_lane) = self.oid_to_lane.get(&parent_oid) {
                // Already has a lane assignment, keep it
                *existing_lane
            } else {
                // Try to use preferred lane if available
                if self.free_lanes.contains(&lane) {
                    self.free_lanes.retain(|&l| l != lane);
                }
                self.oid_to_lane.insert(parent_oid, lane);
                lane
            }
        } else {
            // Secondary parents get new lanes
            self.get_or_allocate_lane(parent_oid)
        }
    }
}

// ============================================================================
// MAIN API FUNCTIONS
// ============================================================================

#[tauri::command]
#[logger::logger]
pub async fn history_graph(
    repo_path: &str,
    query: HistoryQuery,
) -> Result<HistoryResponse, String> {
    let repo = open_repository(repo_path).map_err(|e| e.to_string())?;

    let branch_map = collect_branch_refs(&repo).map_err(|e| e.to_string())?;

    let mut revwalk = repo.revwalk().map_err(|e| e.to_string())?;

    // Start from cursor or HEAD
    if let Some(ref cursor) = query.cursor {
        let oid = Oid::from_str(cursor).map_err(|e| format!("Invalid cursor: {}", e))?;
        revwalk.push(oid).map_err(|e| e.to_string())?;
    } else {
        revwalk.push_head().map_err(|e| e.to_string())?;
    }

    revwalk
        .set_sorting(git2::Sort::TOPOLOGICAL | git2::Sort::TIME)
        .map_err(|e| e.to_string())?;

    let mut lane_manager = LaneManager::new();
    let mut rows = Vec::with_capacity(query.limit);
    let mut iter = revwalk.enumerate();
    let mut next_cursor = None;

    // Fetch one extra to determine if there are more
    // If cursor is provided, we skip first commit, so fetch one more
    let fetch_count = if query.cursor.is_some() {
        query.limit + 2
    } else {
        query.limit + 1
    };

    while let Some((idx, oid_result)) = iter.next() {
        if idx >= fetch_count {
            break;
        }

        let oid = oid_result.map_err(|e| e.to_string())?;
        let commit = repo.find_commit(oid).map_err(|e| e.to_string())?;

        // Skip first commit if we're continuing from a cursor
        if idx == 0 && query.cursor.is_some() {
            continue;
        }

        // Apply search filter
        if let Some(ref search_term) = query.search {
            let message = commit.message().unwrap_or("");
            if !message.to_lowercase().contains(&search_term.to_lowercase()) {
                continue;
            }
        }

        // Check if we've collected enough
        if rows.len() >= query.limit {
            next_cursor = Some(oid.to_string());
            break;
        }

        // Get or allocate lane for this commit
        let lane = lane_manager.get_or_allocate_lane(oid);

        // Build parent edges
        let parent_ids: Vec<Oid> = commit.parent_ids().collect();
        let mut parents = Vec::with_capacity(parent_ids.len());

        // First parent continues in the same lane
        if let Some(&first_parent) = parent_ids.first() {
            let parent_lane = lane_manager.assign_parent(first_parent, Some(lane));
            parents.push(ParentEdge {
                oid: first_parent.to_string(),
                lane: parent_lane,
            });
        }

        // Additional parents get new lanes
        for &parent_oid in parent_ids.iter().skip(1) {
            let parent_lane = lane_manager.assign_parent(parent_oid, None);
            parents.push(ParentEdge {
                oid: parent_oid.to_string(),
                lane: parent_lane,
            });
        }

        // Release this commit's lane
        lane_manager.release_lane(oid);

        // Calculate diff stats (insertions/deletions)
        let (insertions, deletions) = calculate_diff_stats(&repo, &commit);

        // Build the row
        let author = commit.author();
        let full_oid = oid.to_string();
        let mut branches = branch_map.get(&oid).cloned().unwrap_or_default();
        branches.sort();
        branches.dedup();

        rows.push(GraphRow {
            short_oid: full_oid[..7].to_string(),
            oid: full_oid,
            lane,
            message: commit.summary().unwrap_or("").to_string(),
            author: author.name().unwrap_or("Unknown").to_string(),
            authors: extract_all_authors(&commit),
            timestamp: commit.time().seconds(),
            parents,
            insertions,
            deletions,
            branches,
        });
    }

    Ok(HistoryResponse {
        rows,
        cursor: next_cursor.clone(),
        has_more: next_cursor.is_some(),
    })
}

// ============================================================================
// SEARCH FUNCTION (Optimized for large repos)
// ============================================================================

#[tauri::command]
#[logger::logger]
pub async fn search_commits(
    repo_path: &str,
    search_term: &str,
    limit: usize,
) -> Result<Vec<SearchResult>, String> {
    let repo = open_repository(repo_path).map_err(|e| e.to_string())?;
    let mut revwalk = repo.revwalk().map_err(|e| e.to_string())?;

    revwalk.push_head().map_err(|e| e.to_string())?;
    revwalk
        .set_sorting(git2::Sort::TIME)
        .map_err(|e| e.to_string())?;

    let search_lower = search_term.to_lowercase();
    let mut results = Vec::new();

    for oid in revwalk {
        if results.len() >= limit {
            break;
        }

        let oid = oid.map_err(|e| e.to_string())?;
        let commit = repo.find_commit(oid).map_err(|e| e.to_string())?;

        let message = commit.message().unwrap_or("");
        if message.to_lowercase().contains(&search_lower) {
            let author = commit.author();
            results.push(SearchResult {
                oid: oid.to_string(),
                short_oid: oid.to_string()[..7].to_string(),
                message: commit.summary().unwrap_or("").to_string(),
                author: author.name().unwrap_or("Unknown").to_string(),
                timestamp: commit.time().seconds(),
            });
        }
    }

    Ok(results)
}

#[derive(Debug, Serialize)]
pub struct SearchResult {
    pub oid: String,
    pub short_oid: String,
    pub message: String,
    pub author: String,
    pub timestamp: i64,
}

// ============================================================================
// HELPER FUNCTIONS
// ============================================================================

/// Calculate insertions and deletions for a commit compared to its parent
fn calculate_diff_stats(repo: &git2::Repository, commit: &git2::Commit) -> (usize, usize) {
    let tree = match commit.tree() {
        Ok(t) => t,
        Err(_) => return (0, 0),
    };

    let parent_tree = commit.parent(0).ok().and_then(|p| p.tree().ok());

    let diff = match parent_tree {
        Some(parent) => repo.diff_tree_to_tree(Some(&parent), Some(&tree), None),
        None => repo.diff_tree_to_tree(None, Some(&tree), None),
    };

    let diff = match diff {
        Ok(d) => d,
        Err(_) => return (0, 0),
    };

    let stats = match diff.stats() {
        Ok(s) => s,
        Err(_) => return (0, 0),
    };

    (stats.insertions(), stats.deletions())
}

fn collect_branch_refs(repo: &git2::Repository) -> Result<HashMap<Oid, Vec<String>>, git2::Error> {
    let mut map: HashMap<Oid, Vec<String>> = HashMap::new();

    for reference in repo.references_glob("refs/heads/*")? {
        let reference = reference?;
        let name = match reference.shorthand() {
            Some(n) => n,
            None => continue,
        };

        if let Some(oid) = reference.target() {
            map.entry(oid).or_default().push(name.to_string());
        }
    }

    for reference in repo.references_glob("refs/remotes/*")? {
        let reference = reference?;
        let name = match reference.shorthand() {
            Some(n) => n,
            None => continue,
        };

        if name.ends_with("/HEAD") {
            continue;
        }

        if let Some(oid) = reference.target() {
            map.entry(oid).or_default().push(name.to_string());
        }
    }

    Ok(map)
}
