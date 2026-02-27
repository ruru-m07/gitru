//! Integration tests for StashService.
//!
//! Tests stash push/pop, apply, drop, and gitru-specific stash features.

mod common;

use common::{TestRepo, run_async};
use git::context::RepoContext;
use git::service::stash::StashService;
use serial_test::serial;
use std::sync::Arc;

fn setup_stash_service(repo: &TestRepo) -> StashService {
    let ctx = Arc::new(RepoContext::new(repo.path_str()).expect("failed to create repo context"));
    StashService::new(ctx)
}

// ══════════════════════════════════════════════════════════════════════════════
// STASH LIST TESTS
// ══════════════════════════════════════════════════════════════════════════════

#[test]
#[serial]
fn stash_list_empty() {
    run_async(async {
        let repo = TestRepo::new();
        repo.commit_file("README.md", "# Test", "Initial commit");

        let service = setup_stash_service(&repo);
        let stashes = service.list().await.unwrap();

        assert!(stashes.is_empty());
    });
}

#[test]
#[serial]
fn stash_list_single_stash() {
    run_async(async {
        let repo = TestRepo::new();
        repo.commit_file("README.md", "# Test", "Initial commit");
        repo.create_file("changes.txt", "content");
        repo.stash_push(Some("Test stash"));

        let service = setup_stash_service(&repo);
        let stashes = service.list().await.unwrap();

        assert_eq!(stashes.len(), 1);
        assert!(stashes[0].message.contains("Test stash"));
        assert_eq!(stashes[0].index, 0);
        assert_eq!(stashes[0].reference, "stash@{0}");
    });
}

#[test]
#[serial]
fn stash_list_multiple_stashes() {
    run_async(async {
        let repo = TestRepo::new();
        repo.commit_file("README.md", "# Test", "Initial commit");

        // Create first stash
        repo.create_file("file1.txt", "content1");
        repo.stash_push(Some("First stash"));

        // Create second stash
        repo.create_file("file2.txt", "content2");
        repo.stash_push(Some("Second stash"));

        // Create third stash
        repo.create_file("file3.txt", "content3");
        repo.stash_push(Some("Third stash"));

        let service = setup_stash_service(&repo);
        let stashes = service.list().await.unwrap();

        assert_eq!(stashes.len(), 3);
        // Most recent stash is at index 0
        assert!(stashes[0].message.contains("Third stash"));
        assert!(stashes[1].message.contains("Second stash"));
        assert!(stashes[2].message.contains("First stash"));
    });
}

// ══════════════════════════════════════════════════════════════════════════════
// STASH PUSH TESTS
// ══════════════════════════════════════════════════════════════════════════════

#[test]
#[serial]
fn stash_push_simple() {
    run_async(async {
        let repo = TestRepo::new();
        repo.commit_file("README.md", "# Test", "Initial commit");
        // Create a tracked change (not untracked) by modifying a committed file
        repo.create_file("README.md", "# Test - Modified");

        let service = setup_stash_service(&repo);
        let result = service.push(Some("My stash"), false).await;

        assert!(result.is_ok());

        // Working tree should be clean
        assert!(!repo.has_changes());

        // Should have one stash
        let stashes = service.list().await.unwrap();
        assert_eq!(stashes.len(), 1);
    });
}

#[test]
#[serial]
fn stash_push_with_untracked() {
    run_async(async {
        let repo = TestRepo::new();
        repo.commit_file("README.md", "# Test", "Initial commit");
        repo.create_file("untracked.txt", "untracked content");

        let service = setup_stash_service(&repo);
        let result = service.push(Some("With untracked"), true).await;

        assert!(result.is_ok());

        // Should have stashed the untracked file
        assert!(!repo.path().join("untracked.txt").exists());
    });
}

#[test]
#[serial]
fn stash_push_without_untracked() {
    run_async(async {
        let repo = TestRepo::new();
        repo.commit_file("README.md", "# Test", "Initial commit");
        repo.create_file("untracked.txt", "untracked content");

        let service = setup_stash_service(&repo);
        let result = service.push(Some("Without untracked"), false).await;

        // Should succeed but say no changes to save (tracked files are clean)
        assert!(result.is_ok());

        // Untracked file should still exist
        assert!(repo.path().join("untracked.txt").exists());
    });
}

#[test]
#[serial]
fn stash_push_no_changes() {
    run_async(async {
        let repo = TestRepo::new();
        repo.commit_file("README.md", "# Test", "Initial commit");

        let service = setup_stash_service(&repo);
        let result = service.push(Some("No changes"), false).await;

        assert!(result.is_ok());
        assert!(result.unwrap().contains("No local changes"));
    });
}

#[test]
#[serial]
fn stash_push_staged_changes() {
    run_async(async {
        let repo = TestRepo::new();
        repo.commit_file("README.md", "# Test", "Initial commit");
        repo.create_file("staged.txt", "content");
        repo.add("staged.txt");

        let service = setup_stash_service(&repo);
        let result = service.push(Some("Staged"), false).await;

        assert!(result.is_ok());
        assert!(!repo.has_changes());
    });
}

// ══════════════════════════════════════════════════════════════════════════════
// STASH POP TESTS
// ══════════════════════════════════════════════════════════════════════════════

#[test]
#[serial]
fn stash_pop_latest() {
    run_async(async {
        let repo = TestRepo::new();
        repo.commit_file("README.md", "# Test", "Initial commit");
        repo.create_file("changes.txt", "content");
        repo.stash_push(Some("To pop"));

        assert!(!repo.has_changes());

        let service = setup_stash_service(&repo);
        let result = service.pop(None).await;

        assert!(result.is_ok());

        // Changes should be restored
        assert!(repo.has_changes());
        assert!(repo.path().join("changes.txt").exists());

        // Stash list should be empty
        let stashes = service.list().await.unwrap();
        assert!(stashes.is_empty());
    });
}

#[test]
#[serial]
fn stash_pop_specific() {
    run_async(async {
        let repo = TestRepo::new();
        repo.commit_file("README.md", "# Test", "Initial commit");

        // Create first stash
        repo.create_file("first.txt", "first");
        repo.stash_push(Some("First"));

        // Create second stash
        repo.create_file("second.txt", "second");
        repo.stash_push(Some("Second"));

        let service = setup_stash_service(&repo);

        // Pop the older stash (index 1)
        let result = service.pop(Some("stash@{1}")).await;

        assert!(result.is_ok());
        assert!(repo.path().join("first.txt").exists());

        // Should have one stash left
        let stashes = service.list().await.unwrap();
        assert_eq!(stashes.len(), 1);
    });
}

#[test]
#[serial]
fn stash_pop_empty_stack() {
    run_async(async {
        let repo = TestRepo::new();
        repo.commit_file("README.md", "# Test", "Initial commit");

        let service = setup_stash_service(&repo);
        let result = service.pop(None).await;

        // Should fail - no stash to pop
        assert!(result.is_err());
    });
}

// ══════════════════════════════════════════════════════════════════════════════
// STASH APPLY TESTS
// ══════════════════════════════════════════════════════════════════════════════

#[test]
#[serial]
fn stash_apply_keeps_stash() {
    run_async(async {
        let repo = TestRepo::new();
        repo.commit_file("README.md", "# Test", "Initial commit");
        repo.create_file("changes.txt", "content");
        repo.stash_push(Some("To apply"));

        let service = setup_stash_service(&repo);
        let result = service.apply(None).await;

        assert!(result.is_ok());

        // Changes should be restored
        assert!(repo.has_changes());

        // Stash should still exist (unlike pop)
        let stashes = service.list().await.unwrap();
        assert_eq!(stashes.len(), 1);
    });
}

#[test]
#[serial]
fn stash_apply_specific() {
    run_async(async {
        let repo = TestRepo::new();
        repo.commit_file("README.md", "# Test", "Initial commit");

        repo.create_file("first.txt", "first");
        repo.stash_push(Some("First"));

        repo.create_file("second.txt", "second");
        repo.stash_push(Some("Second"));

        let service = setup_stash_service(&repo);
        let result = service.apply(Some("stash@{1}")).await;

        assert!(result.is_ok());
        assert!(repo.path().join("first.txt").exists());

        // Both stashes should still exist
        let stashes = service.list().await.unwrap();
        assert_eq!(stashes.len(), 2);
    });
}

// ══════════════════════════════════════════════════════════════════════════════
// STASH DROP TESTS
// ══════════════════════════════════════════════════════════════════════════════

#[test]
#[serial]
fn stash_drop_latest() {
    run_async(async {
        let repo = TestRepo::new();
        repo.commit_file("README.md", "# Test", "Initial commit");
        repo.create_file("changes.txt", "content");
        repo.stash_push(Some("To drop"));

        let service = setup_stash_service(&repo);
        let result = service.drop("stash@{0}").await;

        assert!(result.is_ok());

        // Stash should be gone
        let stashes = service.list().await.unwrap();
        assert!(stashes.is_empty());

        // Changes should NOT be restored
        assert!(!repo.has_changes());
    });
}

#[test]
#[serial]
fn stash_drop_specific() {
    run_async(async {
        let repo = TestRepo::new();
        repo.commit_file("README.md", "# Test", "Initial commit");

        repo.create_file("first.txt", "first");
        repo.stash_push(Some("First"));

        repo.create_file("second.txt", "second");
        repo.stash_push(Some("Second"));

        let service = setup_stash_service(&repo);
        let result = service.drop("stash@{0}").await;

        assert!(result.is_ok());

        // Should have one stash left (the first one)
        let stashes = service.list().await.unwrap();
        assert_eq!(stashes.len(), 1);
        assert!(stashes[0].message.contains("First"));
    });
}

// ══════════════════════════════════════════════════════════════════════════════
// STASH CLEAR TESTS
// ══════════════════════════════════════════════════════════════════════════════

#[test]
#[serial]
fn stash_clear_all() {
    run_async(async {
        let repo = TestRepo::new();
        repo.commit_file("README.md", "# Test", "Initial commit");

        // Create multiple stashes
        for i in 1..=3 {
            repo.create_file(&format!("file{}.txt", i), "content");
            repo.stash_push(Some(&format!("Stash {}", i)));
        }

        let service = setup_stash_service(&repo);

        // Verify we have stashes
        let before = service.list().await.unwrap();
        assert_eq!(before.len(), 3);

        // Clear all
        let result = service.clear().await;
        assert!(result.is_ok());

        // Should be empty
        let after = service.list().await.unwrap();
        assert!(after.is_empty());
    });
}

// ══════════════════════════════════════════════════════════════════════════════
// STASH SHOW/QUICK_STAT TESTS
// ══════════════════════════════════════════════════════════════════════════════

#[test]
#[serial]
fn stash_quick_stat() {
    run_async(async {
        let repo = TestRepo::new();
        repo.commit_file("README.md", "# Test", "Initial commit");
        // Create a staged new file so stash show includes it in stats
        repo.create_file("new_file.txt", "line1\nline2\nline3");
        repo.add("new_file.txt");
        repo.stash_push(Some("With stats"));

        let service = setup_stash_service(&repo);
        let stat = service.quick_stat("stash@{0}").await.unwrap();

        assert_eq!(stat.reference, "stash@{0}");
        // New staged files should show in stash stats
        assert!(stat.files_changed >= 1);
    });
}

#[test]
#[serial]
fn stash_show_full() {
    run_async(async {
        let repo = TestRepo::new();
        repo.commit_file("README.md", "# Test", "Initial commit");
        // Create a staged new file
        repo.create_file("new_file.txt", "content");
        repo.add("new_file.txt");
        repo.stash_push(Some("Full show"));

        let service = setup_stash_service(&repo);
        let show = service.show("stash@{0}").await.unwrap();

        assert_eq!(show.reference, "stash@{0}");
        assert!(!show.files.is_empty());
    });
}

#[test]
#[serial]
fn stash_quick_stat_invalid_ref() {
    run_async(async {
        let repo = TestRepo::new();
        repo.commit_file("README.md", "# Test", "Initial commit");

        let service = setup_stash_service(&repo);
        let result = service.quick_stat("invalid-ref").await;

        assert!(result.is_err());
    });
}

// ══════════════════════════════════════════════════════════════════════════════
// GITRU STASH TESTS (Branch-aware stashing)
// ══════════════════════════════════════════════════════════════════════════════

#[test]
#[serial]
fn push_gitru_stash() {
    run_async(async {
        let repo = TestRepo::new();
        repo.commit_file("README.md", "# Test", "Initial commit");
        // Create staged change so stash has content
        repo.create_file("changes.txt", "content");
        repo.add("changes.txt");

        let service = setup_stash_service(&repo);
        let result = service
            .push_gitru_stash("main", "feature/target", false)
            .await;

        assert!(result.is_ok());

        // Should have created a stash with gitru marker
        let stashes = service.list().await.unwrap();
        assert_eq!(stashes.len(), 1);
        assert!(stashes[0].is_gitru);
    });
}

#[test]
#[serial]
fn find_gitru_stash_for_branch() {
    run_async(async {
        let repo = TestRepo::new();
        repo.commit_file("README.md", "# Test", "Initial commit");
        // Create staged change
        repo.create_file("changes.txt", "content");
        repo.add("changes.txt");

        let service = setup_stash_service(&repo);

        // Create a gitru stash: from "main" to "feature/target"
        service
            .push_gitru_stash("main", "feature/target", false)
            .await
            .unwrap();

        // find_gitru_stash_for_branch matches against from_branch
        // So we look for stashes created FROM "main"
        let found = service.find_gitru_stash_for_branch("main").await.unwrap();

        assert!(found.is_some());
        let branch_stash = found.unwrap();
        assert_eq!(branch_stash.from_branch, "main");
        assert_eq!(branch_stash.to_branch, "feature/target");
    });
}

#[test]
#[serial]
fn find_gitru_stash_not_found() {
    run_async(async {
        let repo = TestRepo::new();
        repo.commit_file("README.md", "# Test", "Initial commit");

        let service = setup_stash_service(&repo);

        // No stash exists
        let found = service
            .find_gitru_stash_for_branch("nonexistent")
            .await
            .unwrap();

        assert!(found.is_none());
    });
}

#[test]
#[serial]
fn pop_gitru_stash_for_branch() {
    run_async(async {
        let repo = TestRepo::new();
        repo.commit_file("README.md", "# Test", "Initial commit");
        // Create staged change
        repo.create_file("changes.txt", "content to stash");
        repo.add("changes.txt");

        let service = setup_stash_service(&repo);

        // Create a gitru stash: from "main" to "feature/target"
        service
            .push_gitru_stash("main", "feature/target", false)
            .await
            .unwrap();
        assert!(!repo.has_changes());

        // Pop it - find by from_branch
        let result = service.pop_gitru_stash_for_branch("main").await;

        assert!(result.is_ok());
        assert!(repo.has_changes());

        // Stash should be gone
        let stashes = service.list().await.unwrap();
        assert!(stashes.is_empty());
    });
}
