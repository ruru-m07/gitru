//! Integration tests for ActionService.
//!
//! Tests git_add, git_remove, git_discard, get_status, and related actions.

mod common;

use common::{run_async, TestRepo};
use git::context::RepoContext;
use git::service::actions::ActionService;
use serial_test::serial;
use std::sync::Arc;

fn setup_action_service(repo: &TestRepo) -> ActionService {
    let ctx = Arc::new(RepoContext::new(repo.path_str()).expect("failed to create repo context"));
    ActionService::new(ctx)
}

// ══════════════════════════════════════════════════════════════════════════════
// GIT ADD TESTS
// ══════════════════════════════════════════════════════════════════════════════

#[test]
#[serial]
fn git_add_single_file() {
    run_async(async {
        let repo = TestRepo::new();
        repo.commit_file("README.md", "# Test", "Initial commit");
        repo.create_file("new_file.txt", "content");

        let service = setup_action_service(&repo);
        let result = service.git_add("new_file.txt").await;

        assert!(result.is_ok());

        // File should now be staged
        assert!(repo.is_staged("new_file.txt"));
    });
}

#[test]
#[serial]
fn git_add_all() {
    run_async(async {
        let repo = TestRepo::new();
        repo.commit_file("README.md", "# Test", "Initial commit");
        repo.create_file("a.txt", "a");
        repo.create_file("b.txt", "b");
        repo.create_file("subdir/c.txt", "c");

        let service = setup_action_service(&repo);
        let result = service.git_add(".").await;

        assert!(result.is_ok());
    });
}

#[test]
#[serial]
fn git_add_modified_file() {
    run_async(async {
        let repo = TestRepo::new();
        repo.commit_file("README.md", "# Original", "Initial commit");

        // Modify the tracked file
        repo.create_file("README.md", "# Modified");

        let service = setup_action_service(&repo);
        let result = service.git_add("README.md").await;

        assert!(result.is_ok());
        assert!(repo.is_staged("README.md"));
    });
}

#[test]
#[serial]
fn git_add_nonexistent_file() {
    run_async(async {
        let repo = TestRepo::new();
        repo.commit_file("README.md", "# Test", "Initial commit");

        let service = setup_action_service(&repo);
        let result = service.git_add("nonexistent.txt").await;

        // git add on nonexistent files fails
        assert!(result.is_err());
    });
}

#[test]
#[serial]
fn git_add_directory() {
    run_async(async {
        let repo = TestRepo::new();
        repo.commit_file("README.md", "# Test", "Initial commit");
        repo.create_file("src/a.txt", "a");
        repo.create_file("src/b.txt", "b");
        repo.create_file("src/nested/c.txt", "c");

        let service = setup_action_service(&repo);
        let result = service.git_add("src").await;

        assert!(result.is_ok());
    });
}

// ══════════════════════════════════════════════════════════════════════════════
// GIT REMOVE TESTS (Unstage)
// ══════════════════════════════════════════════════════════════════════════════

#[test]
#[serial]
fn git_remove_unstages_file() {
    run_async(async {
        let repo = TestRepo::new();
        repo.commit_file("README.md", "# Test", "Initial commit");
        repo.create_file("staged.txt", "content");
        repo.add("staged.txt");

        assert!(repo.is_staged("staged.txt"));

        let service = setup_action_service(&repo);
        let result = service.git_remove("staged.txt").await;

        assert!(result.is_ok());
        assert!(!repo.is_staged("staged.txt"));
    });
}

#[test]
#[serial]
fn git_remove_not_staged() {
    run_async(async {
        let repo = TestRepo::new();
        repo.commit_file("README.md", "# Test", "Initial commit");
        repo.create_file("untracked.txt", "content");

        let service = setup_action_service(&repo);
        let result = service.git_remove("untracked.txt").await;

        // git restore --staged on an untracked file fails
        // This is expected behavior
        assert!(result.is_err());
    });
}

// ══════════════════════════════════════════════════════════════════════════════
// GIT DISCARD TESTS
// ══════════════════════════════════════════════════════════════════════════════

#[test]
#[serial]
fn git_discard_modified_file() {
    run_async(async {
        let repo = TestRepo::new();
        repo.commit_file("README.md", "# Original", "Initial commit");
        repo.create_file("README.md", "# Modified");

        assert!(repo.has_changes());

        let service = setup_action_service(&repo);
        let result = service.git_discard("README.md", None).await;

        assert!(result.is_ok());

        // File should be restored to original
        let content = std::fs::read_to_string(repo.path().join("README.md")).unwrap();
        assert_eq!(content, "# Original");
    });
}

#[test]
#[serial]
fn git_discard_untracked_file() {
    run_async(async {
        let repo = TestRepo::new();
        repo.commit_file("README.md", "# Test", "Initial");
        repo.create_file("untracked.txt", "content");

        let service = setup_action_service(&repo);
        let result = service.git_discard("untracked.txt", None).await;

        // For untracked files, discard removes the file
        // Just verify it doesn't panic
        let _ = result;
    });
}

#[test]
#[serial]
fn git_discard_staged_changes() {
    run_async(async {
        let repo = TestRepo::new();
        repo.commit_file("README.md", "# Original", "Initial");
        repo.create_file("README.md", "# Modified");
        repo.add("README.md");

        let service = setup_action_service(&repo);
        let result = service.git_discard("README.md", None).await;

        assert!(result.is_ok());

        let content = std::fs::read_to_string(repo.path().join("README.md")).unwrap();
        assert_eq!(content, "# Original");
    });
}

#[test]
#[serial]
fn git_discard_all() {
    run_async(async {
        let repo = TestRepo::new();
        repo.commit_file("a.txt", "a", "Initial");
        repo.commit_file("b.txt", "b", "Add b");

        repo.create_file("a.txt", "modified-a");
        repo.create_file("b.txt", "modified-b");

        let service = setup_action_service(&repo);
        let result = service.git_discard(".", Some(true)).await;

        assert!(result.is_ok());
        assert!(!repo.has_changes());
    });
}

// ══════════════════════════════════════════════════════════════════════════════
// GET STATUS TESTS
// ══════════════════════════════════════════════════════════════════════════════

#[test]
#[serial]
fn get_status_clean() {
    run_async(async {
        let repo = TestRepo::new();
        repo.commit_file("README.md", "# Test", "Initial commit");

        let service = setup_action_service(&repo);
        let status = service.get_status().await.unwrap();

        assert!(status.files.is_empty());
    });
}

#[test]
#[serial]
fn get_status_untracked() {
    run_async(async {
        let repo = TestRepo::new();
        repo.commit_file("README.md", "# Test", "Initial commit");
        repo.create_file("untracked.txt", "content");

        let service = setup_action_service(&repo);
        let status = service.get_status().await.unwrap();

        assert!(!status.files.is_empty());
        assert!(status.files.iter().any(|f| f.path == "untracked.txt"));
    });
}

#[test]
#[serial]
fn get_status_staged_new_file() {
    run_async(async {
        let repo = TestRepo::new();
        repo.commit_file("README.md", "# Test", "Initial commit");
        repo.create_file("new.txt", "content");
        repo.add("new.txt");

        let service = setup_action_service(&repo);
        let status = service.get_status().await.unwrap();

        assert!(!status.files.is_empty());
        assert!(status.files.iter().any(|f| f.path == "new.txt"));
    });
}

#[test]
#[serial]
fn get_status_modified() {
    run_async(async {
        let repo = TestRepo::new();
        repo.commit_file("README.md", "# Original", "Initial commit");
        repo.create_file("README.md", "# Modified");

        let service = setup_action_service(&repo);
        let status = service.get_status().await.unwrap();

        // Modified file should appear
        assert!(!status.files.is_empty());
    });
}

#[test]
#[serial]
fn get_status_deleted() {
    run_async(async {
        let repo = TestRepo::new();
        repo.commit_file("to_delete.txt", "content", "Add file");

        // Delete the file
        std::fs::remove_file(repo.path().join("to_delete.txt")).unwrap();

        let service = setup_action_service(&repo);
        let status = service.get_status().await.unwrap();

        // Deleted file should appear
        assert!(!status.files.is_empty());
    });
}

#[test]
#[serial]
fn get_status_renamed() {
    run_async(async {
        let repo = TestRepo::new();
        repo.commit_file("old_name.txt", "content", "Add file");

        // Use git mv to rename
        repo.git(&["mv", "old_name.txt", "new_name.txt"]);

        let service = setup_action_service(&repo);
        let status = service.get_status().await.unwrap();

        // Renamed file should appear
        assert!(!status.files.is_empty());
    });
}

#[test]
#[serial]
fn get_status_mixed() {
    run_async(async {
        let repo = TestRepo::new();
        repo.commit_file("tracked.txt", "original", "Initial");

        // Staged new file
        repo.create_file("new.txt", "new");
        repo.add("new.txt");

        // Unstaged modified
        repo.create_file("tracked.txt", "modified");

        // Untracked
        repo.create_file("untracked.txt", "untracked");

        let service = setup_action_service(&repo);
        let status = service.get_status().await.unwrap();

        // Should have multiple files
        assert!(status.files.len() >= 2);
    });
}

// ══════════════════════════════════════════════════════════════════════════════
// EDGE CASES
// ══════════════════════════════════════════════════════════════════════════════

#[test]
#[serial]
fn git_add_file_with_spaces() {
    run_async(async {
        let repo = TestRepo::new();
        repo.commit_file("README.md", "# Test", "Initial commit");
        repo.create_file("file with spaces.txt", "content");

        let service = setup_action_service(&repo);
        let result = service.git_add("file with spaces.txt").await;

        assert!(result.is_ok());
    });
}

#[test]
#[serial]
fn git_add_file_with_unicode() {
    run_async(async {
        let repo = TestRepo::new();
        repo.commit_file("README.md", "# Test", "Initial commit");
        repo.create_file("文件.txt", "内容");

        let service = setup_action_service(&repo);
        let result = service.git_add("文件.txt").await;

        assert!(result.is_ok());
    });
}

#[test]
#[serial]
fn git_version() {
    run_async(async {
        let repo = TestRepo::new();
        repo.commit_file("README.md", "# Test", "Initial commit");

        let service = setup_action_service(&repo);
        let version = service.git_version().await.unwrap();

        assert!(version.contains("git version"));
    });
}

#[test]
#[serial]
fn git_fetch_no_remote() {
    run_async(async {
        let repo = TestRepo::new();
        repo.commit_file("README.md", "# Test", "Initial commit");

        let service = setup_action_service(&repo);
        let result = service.git_fetch().await;

        // Should succeed even without remote (no-op)
        assert!(result.is_ok());
    });
}
