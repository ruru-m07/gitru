//! Integration tests for parsers using real git repositories.
//!
//! These tests create actual git repositories and parse real git output,
//! ensuring our parsers work with authentic git data rather than synthetic mocks.

mod common;

use common::TestRepo;
use git::parsers::branch::{BRANCH_STANDARD_FORMAT, parse_branch_records};
use git::parsers::commit::{
    COMMIT_STANDARD_FORMAT, build_commit_authors, extract_co_authors, normalize_email,
    parse_commit_record, parse_shortstat,
};
use git::parsers::history::parse_history_records;
use git::parsers::stash::{parse_stash_file_status, parse_stash_list, parse_stash_stat};
use git::parsers::status::parse_porcelain_v2;
use serial_test::serial;

// ══════════════════════════════════════════════════════════════════════════════
// COMMIT PARSER TESTS - Real Git Output
// ══════════════════════════════════════════════════════════════════════════════

#[test]
#[serial]
fn parse_real_commit_simple() {
    let repo = TestRepo::new();
    repo.commit_file("README.md", "# Hello", "Initial commit");

    let output = repo.git(&["log", "-1", &format!("--format={}", COMMIT_STANDARD_FORMAT)]);

    let commit = parse_commit_record(&output).unwrap();
    assert_eq!(commit.summary, "Initial commit");
    assert_eq!(commit.authors.author.name, "Test User");
    assert_eq!(commit.authors.author.email, "test@example.com");
    assert!(!commit.id.is_empty());
    assert!(commit.timestamp > 0);
}

#[test]
#[serial]
fn parse_real_commit_with_body() {
    let repo = TestRepo::new();
    repo.create_file("file.txt", "content");
    repo.add("file.txt");
    repo.git(&[
        "commit",
        "-m",
        "Add feature\n\nThis is the body.\nWith multiple lines.",
    ]);

    let output = repo.git(&["log", "-1", &format!("--format={}", COMMIT_STANDARD_FORMAT)]);

    let commit = parse_commit_record(&output).unwrap();
    assert_eq!(commit.summary, "Add feature");
    assert!(commit.body.contains("This is the body"));
    assert!(commit.body.contains("multiple lines"));
}

#[test]
#[serial]
fn parse_real_commit_with_co_authors() {
    let repo = TestRepo::new();
    repo.create_file("file.txt", "content");
    repo.add("file.txt");

    let message = "Add feature\n\nImplementation details.\n\nCo-authored-by: Alice <alice@example.com>\nCo-authored-by: Bob <bob@example.com>";
    repo.git(&["commit", "-m", message]);

    let output = repo.git(&["log", "-1", &format!("--format={}", COMMIT_STANDARD_FORMAT)]);

    let commit = parse_commit_record(&output).unwrap();
    assert_eq!(commit.authors.co_authors.len(), 2);
    assert_eq!(commit.authors.co_authors[0].name, "Alice");
    assert_eq!(commit.authors.co_authors[0].email, "alice@example.com");
    assert_eq!(commit.authors.co_authors[1].name, "Bob");
    assert_eq!(commit.authors.co_authors[1].email, "bob@example.com");
}

#[test]
#[serial]
fn parse_real_commit_unicode_message() {
    let repo = TestRepo::new();
    repo.commit_file("日本語.txt", "こんにちは", "日本語のコミットメッセージ 🎉");

    let output = repo.git(&["log", "-1", &format!("--format={}", COMMIT_STANDARD_FORMAT)]);

    let commit = parse_commit_record(&output).unwrap();
    assert!(commit.summary.contains("日本語"));
}

#[test]
#[serial]
fn parse_real_shortstat() {
    let repo = TestRepo::new();
    repo.commit_file("README.md", "# Hello", "Initial commit");

    // Make changes
    repo.create_file("file1.txt", "line1\nline2\nline3");
    repo.create_file("file2.txt", "content");
    repo.add(".");
    repo.commit("Add files");

    let output = repo.git(&["diff", "--shortstat", "HEAD~1", "HEAD"]);
    let stats = parse_shortstat(&output);

    assert_eq!(stats.files_changed, 2);
    assert!(stats.insertions > 0);
}

#[test]
#[serial]
fn parse_real_shortstat_with_deletions() {
    let repo = TestRepo::new();
    repo.commit_file("file.txt", "line1\nline2\nline3\nline4\nline5", "Initial");

    // Modify - remove some lines
    repo.create_file("file.txt", "line1\nline3");
    repo.add(".");
    repo.commit("Remove lines");

    let output = repo.git(&["diff", "--shortstat", "HEAD~1", "HEAD"]);
    let stats = parse_shortstat(&output);

    assert_eq!(stats.files_changed, 1);
    assert!(stats.deletions > 0);
}

#[test]
#[serial]
fn parse_real_shortstat_insertions_only() {
    let repo = TestRepo::new();
    repo.commit_file("README.md", "# Test", "Initial commit");

    // Add a brand new file with content (no deletions)
    repo.commit_file(
        "new_feature.txt",
        "line1\nline2\nline3\nline4\nline5",
        "Add new feature file",
    );

    let output = repo.git(&["diff", "--shortstat", "HEAD~1", "HEAD"]);
    let stats = parse_shortstat(&output);

    assert_eq!(stats.files_changed, 1);
    assert_eq!(stats.insertions, 5);
    assert_eq!(stats.deletions, 0);
}

#[test]
#[serial]
fn parse_real_shortstat_deletions_only() {
    let repo = TestRepo::new();
    repo.commit_file("to_delete.txt", "line1\nline2\nline3", "Initial");

    // Delete the file entirely
    std::fs::remove_file(repo.path().join("to_delete.txt")).unwrap();
    repo.add(".");
    repo.commit("Delete file");

    let output = repo.git(&["diff", "--shortstat", "HEAD~1", "HEAD"]);
    let stats = parse_shortstat(&output);

    assert_eq!(stats.files_changed, 1);
    assert_eq!(stats.insertions, 0);
    assert_eq!(stats.deletions, 3);
}

#[test]
#[serial]
fn parse_real_shortstat_multiple_files() {
    let repo = TestRepo::new();
    repo.commit_file("README.md", "# Test", "Initial commit");

    // Add multiple files of different sizes
    repo.create_file("small.txt", "a");
    repo.create_file("medium.txt", "line1\nline2\nline3");
    repo.create_file("large.txt", "1\n2\n3\n4\n5\n6\n7\n8\n9\n10");
    repo.add(".");
    repo.commit("Add multiple files");

    let output = repo.git(&["diff", "--shortstat", "HEAD~1", "HEAD"]);
    let stats = parse_shortstat(&output);

    assert_eq!(stats.files_changed, 3);
    assert_eq!(stats.insertions, 14); // 1 + 3 + 10
    assert_eq!(stats.deletions, 0);
}

#[test]
#[serial]
fn parse_real_shortstat_single_insertion() {
    let repo = TestRepo::new();
    repo.commit_file("file.txt", "original", "Initial");

    // Add exactly one line
    repo.create_file("file.txt", "original\nnew line");
    repo.add(".");
    repo.commit("Add one line");

    let output = repo.git(&["diff", "--shortstat", "HEAD~1", "HEAD"]);
    let stats = parse_shortstat(&output);

    assert_eq!(stats.files_changed, 1);
    // Git counts the modified line as changed, so this might be 1 insertion + 1 deletion
    // depending on how git sees the diff
    assert!(stats.insertions >= 1);
}

// ══════════════════════════════════════════════════════════════════════════════
// BRANCH PARSER TESTS - Real Git Output
// ══════════════════════════════════════════════════════════════════════════════

#[test]
#[serial]
fn parse_real_single_branch() {
    let repo = TestRepo::new();
    repo.commit_file("README.md", "# Test", "Initial commit");

    let output = repo.git(&[
        "for-each-ref",
        "refs/heads",
        &format!("--format={}", BRANCH_STANDARD_FORMAT),
    ]);

    let branches = parse_branch_records(&output, false).unwrap();
    assert_eq!(branches.len(), 1);
    assert_eq!(branches[0].name, "main");
    assert!(branches[0].is_head);
    assert!(!branches[0].is_remote);
}

#[test]
#[serial]
fn parse_real_multiple_branches() {
    let repo = TestRepo::new();
    repo.commit_file("README.md", "# Test", "Initial commit");

    // Create branches
    repo.git(&["branch", "feature/one"]);
    repo.git(&["branch", "feature/two"]);
    repo.git(&["branch", "bugfix/issue-123"]);

    let output = repo.git(&[
        "for-each-ref",
        "refs/heads",
        &format!("--format={}", BRANCH_STANDARD_FORMAT),
    ]);

    let branches = parse_branch_records(&output, false).unwrap();
    assert_eq!(branches.len(), 4);

    let names: Vec<&str> = branches.iter().map(|b| b.name.as_str()).collect();
    assert!(names.contains(&"main"));
    assert!(names.contains(&"feature/one"));
    assert!(names.contains(&"feature/two"));
    assert!(names.contains(&"bugfix/issue-123"));
}

#[test]
#[serial]
fn parse_real_branch_after_switch() {
    let repo = TestRepo::new();
    repo.commit_file("README.md", "# Test", "Initial commit");
    repo.create_branch("feature/current");

    let output = repo.git(&[
        "for-each-ref",
        "refs/heads",
        &format!("--format={}", BRANCH_STANDARD_FORMAT),
    ]);

    let branches = parse_branch_records(&output, false).unwrap();

    // Find the current branch
    let current = branches.iter().find(|b| b.is_head);
    assert!(current.is_some());
    assert_eq!(current.unwrap().name, "feature/current");
}

#[test]
#[serial]
fn parse_real_branch_with_commit_info() {
    let repo = TestRepo::new();
    repo.commit_file("README.md", "# Test", "Initial commit on main");
    repo.create_branch("feature");
    repo.commit_file("feature.txt", "feature", "Feature commit");

    let output = repo.git(&[
        "for-each-ref",
        "refs/heads",
        &format!("--format={}", BRANCH_STANDARD_FORMAT),
    ]);

    let branches = parse_branch_records(&output, false).unwrap();

    let feature_branch = branches.iter().find(|b| b.name == "feature").unwrap();
    assert_eq!(feature_branch.commit.summary, "Feature commit");
    assert!(!feature_branch.commit.id.is_empty());
}

#[test]
#[serial]
fn parse_real_branch_unicode_name() {
    let repo = TestRepo::new();
    repo.commit_file("README.md", "# Test", "Initial commit");

    // Create branch with unicode characters
    repo.git(&["branch", "feature/新機能"]);

    let output = repo.git(&[
        "for-each-ref",
        "refs/heads",
        &format!("--format={}", BRANCH_STANDARD_FORMAT),
    ]);

    let branches = parse_branch_records(&output, false).unwrap();
    let names: Vec<&str> = branches.iter().map(|b| b.name.as_str()).collect();
    assert!(names.contains(&"feature/新機能"));
}

// ══════════════════════════════════════════════════════════════════════════════
// STATUS PARSER TESTS - Real Git Output
// ══════════════════════════════════════════════════════════════════════════════

#[test]
#[serial]
fn parse_real_status_clean() {
    let repo = TestRepo::new();
    repo.commit_file("README.md", "# Test", "Initial commit");

    let output = repo.git(&["status", "--porcelain=v2", "-z"]);
    let statuses = parse_porcelain_v2(output.as_bytes()).unwrap();

    assert!(statuses.is_empty());
}

#[test]
#[serial]
fn parse_real_status_untracked() {
    let repo = TestRepo::new();
    repo.commit_file("README.md", "# Test", "Initial commit");
    repo.create_file("untracked.txt", "new file");

    let output = repo.git(&["status", "--porcelain=v2", "-z"]);
    let statuses = parse_porcelain_v2(output.as_bytes()).unwrap();

    assert_eq!(statuses.len(), 1);
    assert_eq!(statuses[0].path, "untracked.txt");
    assert!(
        statuses[0]
            .status
            .iter()
            .any(|s| matches!(s, git::models::status::FileStatusKind::WorktreeNew))
    );
}

#[test]
#[serial]
fn parse_real_status_staged_new() {
    let repo = TestRepo::new();
    repo.commit_file("README.md", "# Test", "Initial commit");
    repo.create_file("new.txt", "content");
    repo.add("new.txt");

    let output = repo.git(&["status", "--porcelain=v2", "-z"]);
    let statuses = parse_porcelain_v2(output.as_bytes()).unwrap();

    assert_eq!(statuses.len(), 1);
    assert!(
        statuses[0]
            .status
            .iter()
            .any(|s| matches!(s, git::models::status::FileStatusKind::IndexNew))
    );
}

#[test]
#[serial]
fn parse_real_status_modified_worktree() {
    let repo = TestRepo::new();
    repo.commit_file("file.txt", "original", "Initial commit");
    repo.create_file("file.txt", "modified");

    let output = repo.git(&["status", "--porcelain=v2", "-z"]);
    let statuses = parse_porcelain_v2(output.as_bytes()).unwrap();

    assert_eq!(statuses.len(), 1);
    assert!(
        statuses[0]
            .status
            .iter()
            .any(|s| matches!(s, git::models::status::FileStatusKind::WorktreeModified))
    );
}

#[test]
#[serial]
fn parse_real_status_modified_staged() {
    let repo = TestRepo::new();
    repo.commit_file("file.txt", "original", "Initial commit");
    repo.create_file("file.txt", "modified");
    repo.add("file.txt");

    let output = repo.git(&["status", "--porcelain=v2", "-z"]);
    let statuses = parse_porcelain_v2(output.as_bytes()).unwrap();

    assert_eq!(statuses.len(), 1);
    assert!(
        statuses[0]
            .status
            .iter()
            .any(|s| matches!(s, git::models::status::FileStatusKind::IndexModified))
    );
}

#[test]
#[serial]
fn parse_real_status_deleted() {
    let repo = TestRepo::new();
    repo.commit_file("file.txt", "content", "Initial commit");
    std::fs::remove_file(repo.path().join("file.txt")).unwrap();

    let output = repo.git(&["status", "--porcelain=v2", "-z"]);
    let statuses = parse_porcelain_v2(output.as_bytes()).unwrap();

    assert_eq!(statuses.len(), 1);
    assert!(
        statuses[0]
            .status
            .iter()
            .any(|s| matches!(s, git::models::status::FileStatusKind::WorktreeDeleted))
    );
}

#[test]
#[serial]
fn parse_real_status_renamed() {
    let repo = TestRepo::new();
    repo.commit_file("old_name.txt", "content", "Initial commit");
    repo.git(&["mv", "old_name.txt", "new_name.txt"]);

    let output = repo.git(&["status", "--porcelain=v2", "-z"]);
    let statuses = parse_porcelain_v2(output.as_bytes()).unwrap();

    assert_eq!(statuses.len(), 1);
    assert_eq!(statuses[0].path, "old_name.txt");
    assert_eq!(statuses[0].new_path, Some("new_name.txt".to_string()));
    assert!(
        statuses[0]
            .status
            .iter()
            .any(|s| matches!(s, git::models::status::FileStatusKind::IndexRenamed))
    );
}

#[test]
#[serial]
fn parse_real_status_multiple_files() {
    let repo = TestRepo::new();
    repo.commit_file("README.md", "# Test", "Initial commit");

    repo.create_file("untracked.txt", "untracked");
    repo.create_file("staged.txt", "staged");
    repo.add("staged.txt");
    repo.create_file("README.md", "# Modified");

    let output = repo.git(&["status", "--porcelain=v2", "-z"]);
    let statuses = parse_porcelain_v2(output.as_bytes()).unwrap();

    assert_eq!(statuses.len(), 3);
}

#[test]
#[serial]
fn parse_real_status_file_with_spaces() {
    let repo = TestRepo::new();
    repo.commit_file("README.md", "# Test", "Initial commit");
    repo.create_file("file with spaces.txt", "content");

    let output = repo.git(&["status", "--porcelain=v2", "-z"]);
    let statuses = parse_porcelain_v2(output.as_bytes()).unwrap();

    assert_eq!(statuses.len(), 1);
    assert_eq!(statuses[0].path, "file with spaces.txt");
}

#[test]
#[serial]
fn parse_real_status_nested_directory() {
    let repo = TestRepo::new();
    repo.commit_file("README.md", "# Test", "Initial commit");
    repo.create_file("src/nested/deep/file.txt", "content");

    // Use -uall to show individual files instead of directory summaries
    let output = repo.git(&["status", "--porcelain=v2", "-z", "-uall"]);
    let statuses = parse_porcelain_v2(output.as_bytes()).unwrap();

    assert_eq!(statuses.len(), 1);
    assert_eq!(statuses[0].path, "src/nested/deep/file.txt");
}

// ══════════════════════════════════════════════════════════════════════════════
// HISTORY PARSER TESTS - Real Git Output
// ══════════════════════════════════════════════════════════════════════════════

#[test]
#[serial]
fn parse_real_history_single_commit() {
    let repo = TestRepo::new();
    repo.commit_file("README.md", "# Test", "Initial commit");

    let output = repo.git(&["log", &format!("--format={}", COMMIT_STANDARD_FORMAT)]);

    let commits = parse_history_records(&output).unwrap();
    assert_eq!(commits.len(), 1);
    assert_eq!(commits[0].summary, "Initial commit");
}

#[test]
#[serial]
fn parse_real_history_multiple_commits() {
    let repo = TestRepo::new();
    repo.commit_file("file1.txt", "1", "First commit");
    repo.commit_file("file2.txt", "2", "Second commit");
    repo.commit_file("file3.txt", "3", "Third commit");

    let output = repo.git(&["log", &format!("--format={}", COMMIT_STANDARD_FORMAT)]);

    let commits = parse_history_records(&output).unwrap();
    assert_eq!(commits.len(), 3);

    // Git log shows newest first
    assert_eq!(commits[0].summary, "Third commit");
    assert_eq!(commits[1].summary, "Second commit");
    assert_eq!(commits[2].summary, "First commit");
}

#[test]
#[serial]
fn parse_real_history_preserves_order() {
    let repo = TestRepo::new();
    for i in 1..=5 {
        repo.commit_file(
            &format!("file{}.txt", i),
            &i.to_string(),
            &format!("Commit {}", i),
        );
    }

    let output = repo.git(&["log", &format!("--format={}", COMMIT_STANDARD_FORMAT)]);

    let commits = parse_history_records(&output).unwrap();
    assert_eq!(commits.len(), 5);

    // Verify reverse chronological order
    assert_eq!(commits[0].summary, "Commit 5");
    assert_eq!(commits[4].summary, "Commit 1");
}

#[test]
#[serial]
fn parse_real_history_with_limit() {
    let repo = TestRepo::new();
    for i in 1..=10 {
        repo.commit_file(
            &format!("file{}.txt", i),
            &i.to_string(),
            &format!("Commit {}", i),
        );
    }

    let output = repo.git(&["log", "-5", &format!("--format={}", COMMIT_STANDARD_FORMAT)]);

    let commits = parse_history_records(&output).unwrap();
    assert_eq!(commits.len(), 5);
}

// ══════════════════════════════════════════════════════════════════════════════
// STASH PARSER TESTS - Real Git Output
// ══════════════════════════════════════════════════════════════════════════════

#[test]
#[serial]
fn parse_real_stash_list_empty() {
    let repo = TestRepo::new();
    repo.commit_file("README.md", "# Test", "Initial commit");

    let output = repo.git(&["stash", "list", "--format=%gd%x1f%gs"]);
    let stashes = parse_stash_list(&output).unwrap();

    assert!(stashes.is_empty());
}

#[test]
#[serial]
fn parse_real_stash_list_single() {
    let repo = TestRepo::new();
    repo.commit_file("README.md", "# Test", "Initial commit");

    repo.create_file("changes.txt", "uncommitted");
    repo.add("changes.txt");
    repo.git(&["stash", "push", "-m", "Work in progress"]);

    let output = repo.git(&["stash", "list", "--format=%gd%x1f%gs"]);
    let stashes = parse_stash_list(&output).unwrap();

    assert_eq!(stashes.len(), 1);
    assert_eq!(stashes[0].index, 0);
    assert_eq!(stashes[0].reference, "stash@{0}");
    assert!(stashes[0].message.contains("Work in progress"));
}

#[test]
#[serial]
fn parse_real_stash_list_multiple() {
    let repo = TestRepo::new();
    repo.commit_file("README.md", "# Test", "Initial commit");

    // Create multiple stashes
    repo.create_file("file1.txt", "1");
    repo.add("file1.txt");
    repo.git(&["stash", "push", "-m", "First stash"]);

    repo.create_file("file2.txt", "2");
    repo.add("file2.txt");
    repo.git(&["stash", "push", "-m", "Second stash"]);

    repo.create_file("file3.txt", "3");
    repo.add("file3.txt");
    repo.git(&["stash", "push", "-m", "Third stash"]);

    let output = repo.git(&["stash", "list", "--format=%gd%x1f%gs"]);
    let stashes = parse_stash_list(&output).unwrap();

    assert_eq!(stashes.len(), 3);
    assert_eq!(stashes[0].index, 0);
    assert_eq!(stashes[1].index, 1);
    assert_eq!(stashes[2].index, 2);

    // Most recent stash is first
    assert!(stashes[0].message.contains("Third stash"));
    assert!(stashes[2].message.contains("First stash"));
}

#[test]
#[serial]
fn parse_real_stash_stat() {
    let repo = TestRepo::new();
    repo.commit_file("README.md", "# Test", "Initial commit");

    repo.create_file("new_file.txt", "line1\nline2\nline3");
    repo.add("new_file.txt");
    repo.git(&["stash", "push", "-m", "Stash with stats"]);

    let output = repo.git(&["stash", "show", "--stat", "stash@{0}"]);
    let stat = parse_stash_stat(&output, "stash@{0}").unwrap();

    assert_eq!(stat.reference, "stash@{0}");
    assert_eq!(stat.files_changed, 1);
    assert!(stat.insertions > 0);
}

#[test]
#[serial]
fn parse_real_stash_file_status() {
    let repo = TestRepo::new();
    repo.commit_file("README.md", "# Test", "Initial commit");

    repo.create_file("added.txt", "new");
    repo.add("added.txt");
    repo.git(&["stash", "push", "-m", "Test stash"]);

    let output_str = repo.git(&["stash", "show", "--name-status", "-z", "stash@{0}"]);
    let statuses = parse_stash_file_status(output_str.as_bytes()).unwrap();

    assert!(!statuses.is_empty());
}

#[test]
#[serial]
fn parse_real_stash_with_modifications() {
    let repo = TestRepo::new();
    repo.commit_file("file.txt", "original", "Initial commit");

    // Modify and stash
    repo.create_file("file.txt", "modified\nwith extra lines");
    repo.add("file.txt");
    repo.git(&["stash", "push", "-m", "Modified file"]);

    let output = repo.git(&["stash", "show", "--stat", "stash@{0}"]);
    let stat = parse_stash_stat(&output, "stash@{0}").unwrap();

    assert_eq!(stat.files_changed, 1);
    assert!(stat.insertions > 0 || stat.deletions > 0);
}

// ══════════════════════════════════════════════════════════════════════════════
// HELPER FUNCTION TESTS - These can remain with synthetic data
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn normalize_email_removes_brackets() {
    assert_eq!(normalize_email("<alice@example.com>"), "alice@example.com");
    assert_eq!(normalize_email("  <bob@test.org>  "), "bob@test.org");
    assert_eq!(normalize_email("plain@email.com"), "plain@email.com");
}

#[test]
fn extract_co_authors_from_message() {
    let message = "Feature\n\nCo-authored-by: Alice <alice@example.com>\nCo-authored-by: Bob <bob@example.com>";
    let authors = extract_co_authors(message);

    assert_eq!(authors.len(), 2);
    assert_eq!(authors[0].name, "Alice");
    assert_eq!(authors[1].name, "Bob");
}

#[test]
fn extract_co_authors_empty() {
    let message = "Just a regular commit";
    let authors = extract_co_authors(message);
    assert!(authors.is_empty());
}

#[test]
fn build_commit_authors_simple() {
    let authors = build_commit_authors(
        "Alice",
        "alice@example.com",
        "Alice",
        "alice@example.com",
        "Simple commit",
    );

    assert_eq!(authors.author.name, "Alice");
    assert_eq!(authors.committer.name, "Alice");
    assert!(authors.co_authors.is_empty());
}

#[test]
fn build_commit_authors_with_co_authors() {
    let message = "Feature\n\nCo-authored-by: Bob <bob@example.com>";
    let authors = build_commit_authors(
        "Alice",
        "alice@example.com",
        "Alice",
        "alice@example.com",
        message,
    );

    assert_eq!(authors.co_authors.len(), 1);
    assert_eq!(authors.co_authors[0].name, "Bob");
}
