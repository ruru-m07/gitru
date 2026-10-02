//! Integration tests for CommitService.
//!
//! Tests commit creation, retrieval, history, and co-author handling.

mod common;

use common::{TestRepo, run_async};
use git::context::RepoContext;
use git::models::commit::CommitMessage;
use git::service::commit::CommitService;
use serial_test::serial;
use std::io::Write;
use std::process::{Command, Stdio};
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
        let result = service.create_commit(&msg, false, false, None).await;

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
        let result = service.create_commit(&msg, false, false, None).await;

        assert!(result.is_ok());

        // Verify the canonical trailer is parsed back into structured history data.
        let last = service.last_commit().await.unwrap();
        assert!(
            last.body
                .contains("Co-authored-by: Alice <alice@example.com>")
        );
        assert_eq!(last.authors.co_authors.len(), 1);
        assert_eq!(last.authors.co_authors[0].name, "Alice");
        assert_eq!(last.authors.co_authors[0].email, "alice@example.com");
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
        let result = service.create_commit(&msg, false, false, None).await;

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
        let result = service.create_commit(&msg, false, false, None).await;

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
        let previous_head = repo.head_commit();
        let previous_tree = repo.git(&["rev-parse", "HEAD^{tree}"]);
        repo.create_file("staged.txt", "must remain staged");
        repo.add("staged.txt");
        repo.create_file("unstaged.txt", "must remain uncommitted");

        let service = setup_commit_service(&repo);
        let msg = CommitMessage {
            title: "Allow empty commit".to_string(),
            description: None,
            co_authors: vec![],
        };
        let result = service.create_commit(&msg, true, false, None).await;

        let commit_id = result.expect("allow-empty commit should succeed");
        assert_ne!(commit_id, previous_head);
        assert_eq!(repo.git(&["rev-list", "--count", "HEAD"]), "2");
        assert_eq!(repo.git(&["rev-parse", "HEAD^{tree}"]), previous_tree);
        assert_eq!(
            repo.git(&["ls-tree", "--name-only", "HEAD", "--", "staged.txt"]),
            ""
        );
        assert_eq!(
            repo.git(&["ls-tree", "--name-only", "HEAD", "--", "unstaged.txt"]),
            ""
        );
        assert!(repo.is_staged("staged.txt"));
        assert_eq!(repo.git(&["show", ":staged.txt"]), "must remain staged");
        assert_eq!(
            service.last_commit().await.unwrap().summary,
            "Allow empty commit"
        );
    });
}

#[test]
#[serial]
fn amend_commit_message_only_replaces_head() {
    run_async(async {
        let repo = TestRepo::new();
        repo.commit_file("README.md", "# Test", "Initial commit");
        repo.commit_file("second.txt", "content", "Original message");
        let previous_head = repo.head_commit();
        let previous_tree = repo.git(&["rev-parse", "HEAD^{tree}"]);
        let previous_parent = repo.git(&["rev-parse", "HEAD^"]);

        let service = setup_commit_service(&repo);
        let msg = CommitMessage {
            title: "Updated message".to_string(),
            description: Some("Updated body".to_string()),
            co_authors: vec![],
        };
        let commit_id = service
            .create_commit(&msg, false, true, Some(previous_head.as_str()))
            .await
            .expect("message-only amend should succeed");

        assert_ne!(commit_id, previous_head);
        assert_eq!(repo.git(&["rev-list", "--count", "HEAD"]), "2");
        assert_eq!(repo.git(&["rev-parse", "HEAD^{tree}"]), previous_tree);
        assert_eq!(repo.git(&["rev-parse", "HEAD^"]), previous_parent);
        let last = service.last_commit().await.unwrap();
        assert_eq!(last.summary, "Updated message");
        assert!(last.body.contains("Updated body"));
    });
}

#[test]
#[serial]
fn amend_commit_round_trips_mixed_git_trailers() {
    run_async(async {
        let repo = TestRepo::new();
        repo.create_file("README.md", "# Test");
        repo.add("README.md");
        repo.commit(
            "Pair change\n\nExplains the change\n\nSigned-off-by: Ruru <ruru@example.com>\nCo-authored-by: Alice <alice@example.com>",
        );

        let service = setup_commit_service(&repo);
        let original = service.last_commit().await.unwrap();
        assert_eq!(original.authors.co_authors.len(), 1);

        let description = original
            .body
            .lines()
            .filter(|line| !line.starts_with("Co-authored-by:"))
            .collect::<Vec<_>>()
            .join("\n")
            .trim()
            .to_string();
        let co_authors = original
            .authors
            .co_authors
            .iter()
            .map(|author| (author.name.clone(), author.email.clone()))
            .collect();
        let previous_head = repo.head_commit();

        service
            .create_commit(
                &CommitMessage {
                    title: "Amended pair change".to_string(),
                    description: Some(description),
                    co_authors,
                },
                false,
                true,
                Some(previous_head.as_str()),
            )
            .await
            .expect("mixed-trailer amend should succeed");

        let amended_message = repo.git(&["show", "-s", "--format=%B", "HEAD"]);
        assert_eq!(
            amended_message,
            "Amended pair change\n\nExplains the change\n\nSigned-off-by: Ruru <ruru@example.com>\nCo-authored-by: Alice <alice@example.com>"
        );

        let mut parser = Command::new("git")
            .current_dir(repo.path())
            .args(["interpret-trailers", "--parse"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("failed to start git interpret-trailers");
        parser
            .stdin
            .take()
            .expect("interpret-trailers stdin should be piped")
            .write_all(amended_message.as_bytes())
            .expect("failed to write commit message to interpret-trailers");
        let output = parser
            .wait_with_output()
            .expect("failed to wait for git interpret-trailers");
        assert!(
            output.status.success(),
            "git interpret-trailers failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            String::from_utf8(output.stdout).unwrap().trim(),
            "Signed-off-by: Ruru <ruru@example.com>\nCo-authored-by: Alice <alice@example.com>"
        );

        let amended = service.last_commit().await.unwrap();
        assert_eq!(amended.authors.co_authors.len(), 1);
        assert_eq!(amended.authors.co_authors[0].name, "Alice");
        assert_eq!(amended.authors.co_authors[0].email, "alice@example.com");
    });
}

#[test]
#[serial]
fn amend_commit_includes_staged_content() {
    run_async(async {
        let repo = TestRepo::new();
        repo.commit_file("README.md", "# Test", "Original message");
        let previous_head = repo.head_commit();
        repo.create_file("staged.txt", "amended content");
        repo.add("staged.txt");

        let service = setup_commit_service(&repo);
        let msg = CommitMessage {
            title: "Amended with staged content".to_string(),
            description: None,
            co_authors: vec![],
        };
        let commit_id = service
            .create_commit(&msg, false, true, Some(previous_head.as_str()))
            .await
            .expect("staged amend should succeed");

        assert_ne!(commit_id, previous_head);
        assert_eq!(repo.git(&["rev-list", "--count", "HEAD"]), "1");
        assert_eq!(repo.git(&["show", "HEAD:staged.txt"]), "amended content");
    });
}

#[test]
#[serial]
fn create_commit_rejects_amend_with_allow_empty() {
    run_async(async {
        let repo = TestRepo::new();
        repo.commit_file("README.md", "# Test", "Initial");
        let previous_head = repo.head_commit();
        let service = setup_commit_service(&repo);
        let msg = CommitMessage {
            title: "Invalid options".to_string(),
            description: None,
            co_authors: vec![],
        };

        let error = service
            .create_commit(&msg, true, true, Some(previous_head.as_str()))
            .await
            .unwrap_err();

        assert!(error.contains("cannot be used together"));
        assert_eq!(repo.head_commit(), previous_head);
    });
}

#[test]
#[serial]
fn amend_commit_rejects_changed_head_and_preserves_index() {
    run_async(async {
        let repo = TestRepo::new();
        repo.commit_file("README.md", "# Test", "Initial commit");
        let expected_head = repo.head_commit();
        repo.commit_file("second.txt", "content", "Newer commit");
        let current_head = repo.head_commit();
        repo.create_file("staged.txt", "must remain staged");
        repo.add("staged.txt");

        let service = setup_commit_service(&repo);
        let msg = CommitMessage {
            title: "Stale amend attempt".to_string(),
            description: None,
            co_authors: vec![],
        };

        let error = service
            .create_commit(&msg, false, true, Some(expected_head.as_str()))
            .await
            .unwrap_err();

        assert!(error.contains("HEAD changed"));
        assert!(error.contains(&expected_head[..12]));
        assert!(error.contains(&current_head[..12]));
        assert_eq!(repo.head_commit(), current_head);
        assert_eq!(repo.git(&["rev-list", "--count", "HEAD"]), "2");
        assert!(repo.is_staged("staged.txt"));
        assert_eq!(repo.git(&["show", ":staged.txt"]), "must remain staged");
    });
}

#[test]
#[serial]
fn amend_commit_requires_expected_head() {
    run_async(async {
        let repo = TestRepo::new();
        repo.commit_file("README.md", "# Test", "Initial commit");
        let previous_head = repo.head_commit();

        let service = setup_commit_service(&repo);
        let msg = CommitMessage {
            title: "Amend without lease".to_string(),
            description: None,
            co_authors: vec![],
        };

        let error = service
            .create_commit(&msg, false, true, None)
            .await
            .unwrap_err();

        assert!(error.contains("Expected HEAD is required"));
        assert_eq!(repo.head_commit(), previous_head);
    });
}

#[test]
#[serial]
fn create_commit_validates_summary_and_co_authors() {
    run_async(async {
        let repo = TestRepo::new();
        repo.commit_file("README.md", "# Test", "Initial");
        let previous_head = repo.head_commit();
        let service = setup_commit_service(&repo);

        let blank = CommitMessage {
            title: "   ".to_string(),
            description: None,
            co_authors: vec![],
        };
        assert!(
            service
                .create_commit(&blank, false, false, None)
                .await
                .unwrap_err()
                .contains("summary is required")
        );

        let multiline = CommitMessage {
            title: "First line\nSecond line".to_string(),
            description: None,
            co_authors: vec![],
        };
        assert!(
            service
                .create_commit(&multiline, false, false, None)
                .await
                .unwrap_err()
                .contains("single line")
        );

        let invalid_co_author = CommitMessage {
            title: "Valid summary".to_string(),
            description: None,
            co_authors: vec![("Alice".to_string(), "not-an-email".to_string())],
        };
        assert!(
            service
                .create_commit(&invalid_co_author, false, false, None)
                .await
                .unwrap_err()
                .contains("email must contain one @")
        );
        assert_eq!(repo.head_commit(), previous_head);
    });
}

#[cfg(unix)]
#[test]
#[serial]
fn create_commit_propagates_exit_one_hook_failure() {
    use std::os::unix::fs::PermissionsExt;

    run_async(async {
        let repo = TestRepo::new();
        repo.commit_file("README.md", "# Test", "Initial");
        let previous_head = repo.head_commit();
        repo.create_file("staged.txt", "content");
        repo.add("staged.txt");

        let hook = repo.path().join(".git/hooks/pre-commit");
        std::fs::write(&hook, "#!/bin/sh\necho blocked-by-test-hook >&2\nexit 1\n").unwrap();
        let mut permissions = std::fs::metadata(&hook).unwrap().permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&hook, permissions).unwrap();

        let service = setup_commit_service(&repo);
        let msg = CommitMessage {
            title: "Should fail".to_string(),
            description: None,
            co_authors: vec![],
        };
        let error = service
            .create_commit(&msg, false, false, None)
            .await
            .unwrap_err();

        assert!(error.contains("blocked-by-test-hook"));
        assert_eq!(repo.head_commit(), previous_head);
    });
}

#[test]
#[serial]
fn commit_authors_returns_configured_and_recent_authors() {
    run_async(async {
        let repo = TestRepo::new();
        repo.git(&["config", "user.name", "Recent Author"]);
        repo.git(&["config", "user.email", "recent@example.com"]);
        repo.commit_file("README.md", "# Test", "Older commit");

        repo.git(&["config", "user.name", "Configured Author"]);
        repo.git(&["config", "user.email", "configured@example.com"]);
        repo.create_file("collaboration.txt", "content");
        repo.add("collaboration.txt");

        let service = setup_commit_service(&repo);
        let msg = CommitMessage {
            title: "Collaborative commit".to_string(),
            description: None,
            co_authors: vec![
                (
                    "Configured Alias".to_string(),
                    "CONFIGURED@example.com".to_string(),
                ),
                ("Pair Author".to_string(), "pair@example.com".to_string()),
            ],
        };
        service
            .create_commit(&msg, false, false, None)
            .await
            .unwrap();

        let authors = service.commit_authors().await.unwrap();
        let emails: Vec<&str> = authors.iter().map(|author| author.email.as_str()).collect();

        assert_eq!(authors[0].name, "Configured Author");
        assert_eq!(authors[0].email, "configured@example.com");
        assert_eq!(
            emails,
            vec![
                "configured@example.com",
                "pair@example.com",
                "recent@example.com"
            ]
        );
    });
}

#[test]
#[serial]
fn commit_authors_in_unborn_repo_returns_configured_identity() {
    run_async(async {
        let repo = TestRepo::new();
        let service = setup_commit_service(&repo);

        let authors = service.commit_authors().await.unwrap();

        assert_eq!(authors.len(), 1);
        assert_eq!(authors[0].name, "Test User");
        assert_eq!(authors[0].email, "test@example.com");
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
        let result = service
            .commit_by_id("0000000000000000000000000000000000000000")
            .await;

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
            description: Some(
                "This is the body of the commit.\nIt has multiple lines.".to_string(),
            ),
            co_authors: vec![],
        };
        let result = service.create_commit(&msg, false, false, None).await;

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
        let result = service.create_commit(&msg, false, false, None).await;

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
        let result = service.create_commit(&msg, false, false, None).await;

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
        let result = service.create_commit(&msg, false, false, None).await;

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
            description: Some(
                "This adds a detailed feature with lots of context.\n\nMore details here."
                    .to_string(),
            ),
            co_authors: vec![],
        };
        let result = service.create_commit(&msg, false, false, None).await;

        assert!(result.is_ok());

        let last = service.last_commit().await.unwrap();
        assert_eq!(last.summary, "Add detailed feature");
    });
}
