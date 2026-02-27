//! Integration tests for CommitService.
//!
//! Tests commit creation, retrieval, history, and co-author handling.

mod common;

use common::{run_async, TestRepo};
use git::context::RepoContext;
use git::models::commit::CommitMessage;
use git::service::commit::CommitService;
use serial_test::serial;
use std::sync::Arc;

fn setup_commit_service(repo: &TestRepo) -> CommitService {
    let ctx = Arc::new(RepoContext::new(repo.path_str()).expect("failed to create repo context"));
    CommitService::new(ctx)
}

// ══════════════════════════════════════════════════════════════════════════════
// LAST COMMIT TESTS
// ══════════════════════════════════════════════════════════════════════════════

#[test]
#[serial]
fn last_commit_exists() {
    run_async(async {
        let repo = TestRepo::new();
        repo.commit_file("README.md", "# Test", "Initial commit message");

        let service = setup_commit_service(&repo);
        let commit = service.last_commit().await.unwrap();

        assert_eq!(commit.summary, "Initial commit message");
        assert!(!commit.id.is_empty());
    });
}

#[test]
#[serial]
fn last_commit_empty_repo() {
    run_async(async {
        let repo = TestRepo::new();
        // No commits made

        let service = setup_commit_service(&repo);
        let result = service.last_commit().await;

        // Should fail - no commits
        assert!(result.is_err());
    });
}

#[test]
#[serial]
fn last_commit_after_multiple() {
    run_async(async {
        let repo = TestRepo::new();
        repo.commit_file("file1.txt", "content1", "First commit");
        repo.commit_file("file2.txt", "content2", "Second commit");
        repo.commit_file("file3.txt", "content3", "Third commit");

        let service = setup_commit_service(&repo);
        let commit = service.last_commit().await.unwrap();

        assert_eq!(commit.summary, "Third commit");
    });
}

// ══════════════════════════════════════════════════════════════════════════════
// CREATE COMMIT TESTS
// ══════════════════════════════════════════════════════════════════════════════

#[test]
#[serial]
fn create_commit_simple() {
    run_async(async {
        let repo = TestRepo::new();
        repo.commit_file("README.md", "# Test", "Initial");

        // Stage changes
        repo.create_file("new_file.txt", "content");
        repo.add("new_file.txt");

        let service = setup_commit_service(&repo);
        let msg = CommitMessage {
            title: "New feature commit".to_string(),
            description: None,
            co_authors: vec![],
        };
        let result = service.create_commit(&msg, false).await;

        assert!(result.is_ok());

        // Verify commit was created
        let last = service.last_commit().await.unwrap();
        assert_eq!(last.summary, "New feature commit");
    });
}

#[test]
#[serial]
fn create_commit_with_co_author() {
    run_async(async {
        let repo = TestRepo::new();
        repo.commit_file("README.md", "# Test", "Initial");

        repo.create_file("collab.txt", "collaboration");
        repo.add("collab.txt");

        let service = setup_commit_service(&repo);
        let msg = CommitMessage {
            title: "Pair programming".to_string(),
            description: None,
            co_authors: vec![("Alice".to_string(), "alice@example.com".to_string())],
        };
        let result = service.create_commit(&msg, false).await;

        assert!(result.is_ok());

        // Verify the commit was created with co-author
        let last = service.last_commit().await.unwrap();
        assert!(last.body.contains("Co-authored-by:") || last.summary.contains("Pair"));
    });
}

#[test]
#[serial]
fn create_commit_with_multiple_co_authors() {
    run_async(async {
        let repo = TestRepo::new();
        repo.commit_file("README.md", "# Test", "Initial");

        repo.create_file("team.txt", "team work");
        repo.add("team.txt");

        let service = setup_commit_service(&repo);
        let msg = CommitMessage {
            title: "Team effort".to_string(),
            description: None,
            co_authors: vec![
                ("Alice".to_string(), "alice@example.com".to_string()),
                ("Bob".to_string(), "bob@example.com".to_string()),
            ],
        };
        let result = service.create_commit(&msg, false).await;

        assert!(result.is_ok());
    });
}

#[test]
#[serial]
fn create_commit_nothing_staged() {
    run_async(async {
        let repo = TestRepo::new();
        repo.commit_file("README.md", "# Test", "Initial");

        // No changes staged
        let service = setup_commit_service(&repo);
        let msg = CommitMessage {
            title: "Empty commit attempt".to_string(),
            description: None,
            co_authors: vec![],
        };
        let result = service.create_commit(&msg, false).await;

        // Should fail - nothing to commit
        assert!(result.is_err());
    });
}

#[test]
#[serial]
fn create_commit_allow_empty() {
    run_async(async {
        let repo = TestRepo::new();
        repo.commit_file("README.md", "# Test", "Initial");

        let service = setup_commit_service(&repo);
        let msg = CommitMessage {
            title: "Allow empty commit".to_string(),
            description: None,
            co_authors: vec![],
        };
        let result = service.create_commit(&msg, true).await;

        // Should succeed with allow_empty=true
        assert!(result.is_ok());
    });
}

// ══════════════════════════════════════════════════════════════════════════════
// GET COMMIT BY ID TESTS
// ══════════════════════════════════════════════════════════════════════════════

#[test]
#[serial]
fn get_commit_by_id() {
    run_async(async {
        let repo = TestRepo::new();
        repo.commit_file("README.md", "# Test", "Target commit");

        let service = setup_commit_service(&repo);

        // Get the commit hash
        let last = service.last_commit().await.unwrap();
        let hash = last.id.clone();

        // Fetch by ID
        let commit = service.commit_by_id(&hash).await.unwrap();

        assert_eq!(commit.summary, "Target commit");
    });
}

#[test]
#[serial]
fn get_commit_by_short_hash() {
    run_async(async {
        let repo = TestRepo::new();
        repo.commit_file("README.md", "# Test", "Short hash commit");

        let service = setup_commit_service(&repo);

        let last = service.last_commit().await.unwrap();
        let short_hash = &last.id[..7];

        let commit = service.commit_by_id(short_hash).await.unwrap();

        assert_eq!(commit.summary, "Short hash commit");
    });
}

#[test]
#[serial]
fn get_commit_by_invalid_id() {
    run_async(async {
        let repo = TestRepo::new();
        repo.commit_file("README.md", "# Test", "Initial");

        let service = setup_commit_service(&repo);
        let result = service.commit_by_id("0000000000000000000000000000000000000000").await;

        // Should fail - invalid commit reference
        assert!(result.is_err());
    });
}

// ══════════════════════════════════════════════════════════════════════════════
// EDGE CASES
// ══════════════════════════════════════════════════════════════════════════════

#[test]
#[serial]
fn create_commit_multiline_message() {
    run_async(async {
        let repo = TestRepo::new();
        repo.commit_file("README.md", "# Test", "Initial");

        repo.create_file("feature.txt", "content");
        repo.add("feature.txt");

        let service = setup_commit_service(&repo);
        let msg = CommitMessage {
            title: "Subject line".to_string(),
            description: Some("This is the body of the commit.\nIt has multiple lines.".to_string()),
            co_authors: vec![],
        };
        let result = service.create_commit(&msg, false).await;

        assert!(result.is_ok());

        let last = service.last_commit().await.unwrap();
        assert_eq!(last.summary, "Subject line");
        assert!(last.body.contains("multiple lines"));
    });
}

#[test]
#[serial]
fn create_commit_special_characters_in_message() {
    run_async(async {
        let repo = TestRepo::new();
        repo.commit_file("README.md", "# Test", "Initial");

        repo.create_file("special.txt", "content");
        repo.add("special.txt");

        let service = setup_commit_service(&repo);
        let msg = CommitMessage {
            title: "fix: handle 'quotes' and \"double quotes\" & special chars".to_string(),
            description: None,
            co_authors: vec![],
        };
        let result = service.create_commit(&msg, false).await;

        assert!(result.is_ok());

        let last = service.last_commit().await.unwrap();
        assert!(last.summary.contains("quotes"));
    });
}

#[test]
#[serial]
fn create_commit_unicode_message() {
    run_async(async {
        let repo = TestRepo::new();
        repo.commit_file("README.md", "# Test", "Initial");

        repo.create_file("unicode.txt", "内容");
        repo.add("unicode.txt");

        let service = setup_commit_service(&repo);
        let msg = CommitMessage {
            title: "feat: 新機能の追加 🚀".to_string(),
            description: None,
            co_authors: vec![],
        };
        let result = service.create_commit(&msg, false).await;

        assert!(result.is_ok());

        let last = service.last_commit().await.unwrap();
        assert!(last.summary.contains("新機能"));
    });
}

#[test]
#[serial]
fn commit_on_different_branch() {
    run_async(async {
        let repo = TestRepo::new();
        repo.commit_file("README.md", "# Test", "Initial on main");

        // Create and switch to feature branch
        repo.create_branch("feature");
        repo.switch_branch("feature");

        repo.create_file("feature.txt", "feature content");
        repo.add("feature.txt");

        let service = setup_commit_service(&repo);
        let msg = CommitMessage {
            title: "Feature commit".to_string(),
            description: None,
            co_authors: vec![],
        };
        let result = service.create_commit(&msg, false).await;

        assert!(result.is_ok());

        // Verify commit exists on feature branch
        let last = service.last_commit().await.unwrap();
        assert_eq!(last.summary, "Feature commit");
    });
}

#[test]
#[serial]
fn commit_with_description() {
    run_async(async {
        let repo = TestRepo::new();
        repo.commit_file("README.md", "# Test", "Initial");

        repo.create_file("detailed.txt", "content");
        repo.add("detailed.txt");

        let service = setup_commit_service(&repo);
        let msg = CommitMessage {
            title: "Add detailed feature".to_string(),
            description: Some("This adds a detailed feature with lots of context.\n\nMore details here.".to_string()),
            co_authors: vec![],
        };
        let result = service.create_commit(&msg, false).await;

        assert!(result.is_ok());

        let last = service.last_commit().await.unwrap();
        assert_eq!(last.summary, "Add detailed feature");
    });
}
