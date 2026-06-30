//! Integration tests for HistoryService.
//!
//! Tests commit history retrieval with skip/limit pagination,
//! field correctness and the history-graph path.

mod common;

use common::{TestRepo, run_async};
use git::context::RepoContext;
use git::models::graph::HistoryQuery;
use git::service::history::HistoryService;
use serial_test::serial;
use std::sync::Arc;

fn setup(repo: &TestRepo) -> HistoryService {
    let ctx = Arc::new(RepoContext::new(repo.path_str()).expect("failed to create repo context"));
    HistoryService::new(ctx)
}

fn default_query(limit: usize) -> HistoryQuery {
    HistoryQuery {
        cursor: None,
        limit,
        search: None,
        branch: None,
        graph_state: None,
        include_local: true,
        include_remotes: false,
        include_tags: false,
        include_stash: false,
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// HISTORY – basic retrieval
// ══════════════════════════════════════════════════════════════════════════════

#[test]
#[serial]
fn history_empty_repo_returns_empty() {
    run_async(async {
        let repo = TestRepo::new();
        // No commits – git log on an empty repo exits with code 128
        let svc = setup(&repo);
        let result = svc.history(0, 50).await;
        // Acceptable outcomes: error OR empty vec
        if let Ok(commits) = result {
            assert!(commits.is_empty(), "empty repo should have no commits")
        }
    });
}

#[test]
#[serial]
fn history_single_commit() {
    run_async(async {
        let repo = TestRepo::new();
        repo.commit_file("README.md", "# Test", "Initial commit");

        let svc = setup(&repo);
        let commits = svc.history(0, 50).await.unwrap();

        assert_eq!(commits.len(), 1);
        assert_eq!(commits[0].summary, "Initial commit");
        assert!(!commits[0].id.is_empty(), "commit id must not be empty");
        assert!(commits[0].timestamp > 0, "timestamp must be positive");
        assert_eq!(commits[0].authors.author.name, "Test User");
        assert_eq!(commits[0].authors.author.email, "test@example.com");
    });
}

#[test]
#[serial]
fn history_multiple_commits_newest_first() {
    run_async(async {
        let repo = TestRepo::new();
        repo.commit_file("f1.txt", "1", "First commit");
        repo.commit_file("f2.txt", "2", "Second commit");
        repo.commit_file("f3.txt", "3", "Third commit");

        let svc = setup(&repo);
        let commits = svc.history(0, 50).await.unwrap();

        assert_eq!(commits.len(), 3);
        // git log default order: newest first
        assert_eq!(commits[0].summary, "Third commit");
        assert_eq!(commits[1].summary, "Second commit");
        assert_eq!(commits[2].summary, "First commit");
    });
}

#[test]
#[serial]
fn history_timestamps_descending() {
    run_async(async {
        let repo = TestRepo::new();
        repo.commit_file("f1.txt", "1", "First");
        repo.commit_file("f2.txt", "2", "Second");
        repo.commit_file("f3.txt", "3", "Third");

        let svc = setup(&repo);
        let commits = svc.history(0, 50).await.unwrap();

        assert_eq!(commits.len(), 3);
        // Each commit must have a non-zero timestamp; newest first means timestamps
        // should be non-increasing (they can be equal in rapid test runs on the same second)
        assert!(
            commits[0].timestamp >= commits[2].timestamp,
            "commits should be in reverse chronological order"
        );
    });
}

// ══════════════════════════════════════════════════════════════════════════════
// HISTORY – limit parameter
// ══════════════════════════════════════════════════════════════════════════════

#[test]
#[serial]
fn history_limit_restricts_results() {
    run_async(async {
        let repo = TestRepo::new();
        for i in 1..=5 {
            repo.commit_file(&format!("f{i}.txt"), &i.to_string(), &format!("Commit {i}"));
        }

        let svc = setup(&repo);
        let commits = svc.history(0, 2).await.unwrap();

        assert_eq!(commits.len(), 2);
        assert_eq!(commits[0].summary, "Commit 5");
        assert_eq!(commits[1].summary, "Commit 4");
    });
}

#[test]
#[serial]
fn history_limit_larger_than_total() {
    run_async(async {
        let repo = TestRepo::new();
        repo.commit_file("a.txt", "a", "Only commit");

        let svc = setup(&repo);
        let commits = svc.history(0, 100).await.unwrap();

        // Should return all commits, not error
        assert_eq!(commits.len(), 1);
    });
}

// ══════════════════════════════════════════════════════════════════════════════
// HISTORY – skip parameter
// ══════════════════════════════════════════════════════════════════════════════

#[test]
#[serial]
fn history_skip_skips_newest() {
    run_async(async {
        let repo = TestRepo::new();
        for i in 1..=5 {
            repo.commit_file(&format!("f{i}.txt"), &i.to_string(), &format!("Commit {i}"));
        }

        let svc = setup(&repo);
        // skip 2 newest, then take the rest
        let commits = svc.history(2, 50).await.unwrap();

        assert_eq!(commits.len(), 3);
        assert_eq!(commits[0].summary, "Commit 3");
        assert_eq!(commits[1].summary, "Commit 2");
        assert_eq!(commits[2].summary, "Commit 1");
    });
}

#[test]
#[serial]
fn history_skip_and_limit_pagination() {
    run_async(async {
        let repo = TestRepo::new();
        for i in 1..=6 {
            repo.commit_file(&format!("f{i}.txt"), &i.to_string(), &format!("Commit {i}"));
        }

        let svc = setup(&repo);

        let page1 = svc.history(0, 3).await.unwrap();
        let page2 = svc.history(3, 3).await.unwrap();

        assert_eq!(page1.len(), 3);
        assert_eq!(page2.len(), 3);

        // page1: commits 6, 5, 4
        assert_eq!(page1[0].summary, "Commit 6");
        assert_eq!(page1[2].summary, "Commit 4");

        // page2: commits 3, 2, 1
        assert_eq!(page2[0].summary, "Commit 3");
        assert_eq!(page2[2].summary, "Commit 1");

        // No overlap
        let page1_ids: Vec<&str> = page1.iter().map(|c| c.id.as_str()).collect();
        let page2_ids: Vec<&str> = page2.iter().map(|c| c.id.as_str()).collect();
        assert!(
            page1_ids.iter().all(|id| !page2_ids.contains(id)),
            "pages must not overlap"
        );
    });
}

#[test]
#[serial]
fn history_skip_past_end_returns_empty() {
    run_async(async {
        let repo = TestRepo::new();
        repo.commit_file("a.txt", "a", "Only commit");

        let svc = setup(&repo);
        let commits = svc.history(100, 50).await.unwrap();

        assert!(
            commits.is_empty(),
            "skipping past all commits should return empty"
        );
    });
}

// ══════════════════════════════════════════════════════════════════════════════
// HISTORY – commit field correctness
// ══════════════════════════════════════════════════════════════════════════════

#[test]
#[serial]
fn history_commit_id_matches_git_rev_parse() {
    run_async(async {
        let repo = TestRepo::new();
        repo.commit_file("README.md", "# Test", "Initial commit");
        let expected_id = repo.head_commit();

        let svc = setup(&repo);
        let commits = svc.history(0, 1).await.unwrap();

        assert_eq!(commits.len(), 1);
        assert_eq!(commits[0].id, expected_id, "commit id must match HEAD");
    });
}

#[test]
#[serial]
fn history_commit_with_body() {
    run_async(async {
        let repo = TestRepo::new();
        repo.create_file("file.txt", "content");
        repo.add("file.txt");
        repo.git(&[
            "commit",
            "-m",
            "Add feature\n\nThis is the body.\nMultiple lines.",
        ]);

        let svc = setup(&repo);
        let commits = svc.history(0, 1).await.unwrap();

        assert_eq!(commits.len(), 1);
        assert_eq!(commits[0].summary, "Add feature");
        assert!(
            commits[0].body.contains("This is the body"),
            "body should be preserved"
        );
        assert!(
            commits[0].body.contains("Multiple lines"),
            "multi-line body should be preserved"
        );
    });
}

#[test]
#[serial]
fn history_author_fields_populated() {
    run_async(async {
        let repo = TestRepo::new();
        repo.commit_file("a.txt", "a", "Author test commit");

        let svc = setup(&repo);
        let commits = svc.history(0, 1).await.unwrap();

        let commit = &commits[0];
        assert_eq!(commit.authors.author.name, "Test User");
        assert_eq!(commit.authors.author.email, "test@example.com");
        // committer should also be set
        assert!(!commit.authors.committer.name.is_empty());
    });
}

// ══════════════════════════════════════════════════════════════════════════════
// HISTORY GRAPH – smoke tests
// ══════════════════════════════════════════════════════════════════════════════

#[test]
#[serial]
fn history_graph_basic_returns_rows() {
    run_async(async {
        let repo = TestRepo::new();
        repo.commit_file("f1.txt", "1", "First commit");
        repo.commit_file("f2.txt", "2", "Second commit");

        let svc = setup(&repo);
        let result = svc.history_graph(default_query(50)).await;

        assert!(
            result.is_ok(),
            "history_graph should succeed: {:?}",
            result.err()
        );
        let response = result.unwrap();
        assert!(!response.rows.is_empty(), "should have at least one row");
    });
}

#[test]
#[serial]
fn history_graph_row_count_matches_commits() {
    run_async(async {
        let repo = TestRepo::new();
        for i in 1..=5 {
            repo.commit_file(&format!("f{i}.txt"), &i.to_string(), &format!("Commit {i}"));
        }

        let svc = setup(&repo);
        let response = svc.history_graph(default_query(5)).await.unwrap();

        assert_eq!(
            response.rows.len(),
            5,
            "row count should match number of commits (got {})",
            response.rows.len()
        );
    });
}

#[test]
#[serial]
fn history_graph_limit_respected() {
    run_async(async {
        let repo = TestRepo::new();
        for i in 1..=10 {
            repo.commit_file(&format!("f{i}.txt"), &i.to_string(), &format!("Commit {i}"));
        }

        let svc = setup(&repo);
        let response = svc.history_graph(default_query(3)).await.unwrap();

        assert!(
            response.rows.len() <= 3,
            "graph should respect limit, got {} rows",
            response.rows.len()
        );
    });
}

#[test]
#[serial]
fn history_graph_normalizes_refs_and_marks_current_branch() {
    run_async(async {
        let repo = TestRepo::new();
        repo.commit_file("base.txt", "base", "Initial commit");
        repo.create_branch("feature/foo");
        repo.commit_file("feature.txt", "feature", "Feature only");
        repo.switch_branch("main");
        repo.commit_file("main.txt", "main", "Main only");
        repo.git(&["merge", "--no-ff", "feature/foo", "-m", "Merge feature"]);

        let svc = setup(&repo);
        let response = svc.history_graph(default_query(20)).await.unwrap();

        let current_branch_rows: Vec<_> = response
            .rows
            .iter()
            .filter(|row| row.heads.iter().any(|ref_info| ref_info.is_head))
            .collect();

        assert!(
            !current_branch_rows.is_empty(),
            "expected at least one row to be marked as HEAD"
        );
        assert!(
            current_branch_rows.iter().any(|row| row
                .heads
                .iter()
                .any(|ref_info| ref_info.display_name == "main")),
            "expected the checked-out branch to be labeled as main"
        );
        assert!(
            response.rows.iter().any(|row| row
                .refs
                .iter()
                .any(|ref_info| ref_info.display_name == "feature/foo")),
            "expected a normalized feature branch label"
        );
        assert!(
            response.rows.iter().flat_map(|row| row.refs.iter()).all(|ref_info| {
                !ref_info.display_name.starts_with("refs/")
            }),
            "display labels should not leak refs/ prefixes"
        );
    });
}

#[test]
#[serial]
fn history_graph_populates_branch_refs_across_linear_history() {
    run_async(async {
        let repo = TestRepo::new();
        repo.commit_file("base.txt", "base", "Initial commit");
        repo.commit_file("main.txt", "main-1", "Main one");
        repo.commit_file("main.txt", "main-2", "Main two");

        let svc = setup(&repo);
        let response = svc.history_graph(default_query(20)).await.unwrap();

        assert!(
            response
                .rows
                .iter()
                .all(|row| row.branch_refs.iter().any(|ref_info| ref_info.display_name == "main")),
            "every row in a linear history should carry the current branch label"
        );
    });
}

#[test]
#[serial]
fn history_graph_marks_remote_tracking_refs_as_remote() {
    run_async(async {
        let repo = TestRepo::new();
        repo.commit_file("base.txt", "base", "Initial commit");
        let _remote_dir = repo.setup_remote();

        repo.create_branch("feature/remote");
        repo.commit_file("feature.txt", "feature", "Remote feature");
        repo.push("feature/remote");
        repo.switch_branch("main");
        repo.git(&["branch", "-D", "feature/remote"]);
        repo.fetch();

        let svc = setup(&repo);
        let mut query = default_query(20);
        query.include_local = false;
        query.include_remotes = true;

        let response = svc.history_graph(query).await.unwrap();

        assert!(
            response.rows.iter().any(|row| row.branch_refs.iter().any(|ref_info| {
                ref_info.display_name == "origin/feature/remote"
                    && matches!(ref_info.kind, git::models::graph::GraphRefKind::Remote)
            })),
            "expected remote-tracking branch refs to be marked as Remote"
        );
    });
}

#[test]
#[serial]
fn history_graph_branch_query_limits_to_selected_branch() {
    run_async(async {
        let repo = TestRepo::new();
        repo.commit_file("base.txt", "base", "Initial commit");
        repo.create_branch("feature/only");
        repo.commit_file("feature.txt", "feature", "Feature only");
        repo.switch_branch("main");
        repo.commit_file("main.txt", "main", "Main only");

        let svc = setup(&repo);
        let mut query = default_query(20);
        query.branch = Some("main".to_string());

        let response = svc.history_graph(query).await.unwrap();

        assert!(
            response.rows.iter().any(|row| row.commit.summary == "Main only"),
            "expected main branch history to include the local branch tip"
        );
        assert!(
            response
                .rows
                .iter()
                .all(|row| row.commit.summary != "Feature only"),
            "branch-scoped history should not include commits exclusive to another branch"
        );
    });
}
