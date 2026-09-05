//! Integration tests for BranchService.
//!
//! Tests branch creation, switching, and edge cases with uncommitted changes.

mod common;

use common::{TestRepo, run_async};
use git::context::RepoContext;
use git::models::branch::{BranchKind, UncommittedChangesStrategy};
use git::service::branch::BranchService;
use serial_test::serial;
use std::sync::Arc;

fn setup_branch_service(repo: &TestRepo) -> BranchService {
    let ctx = Arc::new(RepoContext::new(repo.path_str()).expect("failed to create repo context"));
    BranchService::new(ctx)
}

// ══════════════════════════════════════════════════════════════════════════════
// GET CURRENT BRANCH TESTS
// ══════════════════════════════════════════════════════════════════════════════

#[test]
#[serial]
fn get_current_branch_on_main() {
    run_async(async {
        let repo = TestRepo::new();
        repo.commit_file("README.md", "# Test", "Initial commit");

        let service = setup_branch_service(&repo);
        let branch = service.get_current_branch().await.unwrap();

        // Default branch is either main or master depending on git config
        assert!(branch.name == "main" || branch.name == "master");
    });
}

#[test]
#[serial]
fn get_current_branch_after_switch() {
    run_async(async {
        let repo = TestRepo::new();
        repo.commit_file("README.md", "# Test", "Initial commit");
        repo.create_branch("feature/test");

        let service = setup_branch_service(&repo);
        let branch = service.get_current_branch().await.unwrap();

        assert_eq!(branch.name, "feature/test");
    });
}

#[test]
#[serial]
fn get_current_branch_when_detached() {
    run_async(async {
        let repo = TestRepo::new();
        repo.commit_file("README.md", "# Test", "Initial commit");
        let head = repo.git(&["rev-parse", "HEAD"]);
        repo.git(&["checkout", "--detach", &head]);

        let service = setup_branch_service(&repo);
        let branch = service.get_current_branch().await.unwrap();

        assert!(branch.is_detached);
        assert!(head.starts_with(&branch.name));
        assert_eq!(branch.name, branch.display_name);
    });
}

#[test]
#[serial]
fn ahead_behind_when_detached() {
    run_async(async {
        let repo = TestRepo::new();
        repo.commit_file("README.md", "# Test", "Initial commit");
        let head = repo.git(&["rev-parse", "HEAD"]);
        repo.git(&["checkout", "--detach", &head]);

        let service = setup_branch_service(&repo);
        let status = service.status_ahead_behind().await.unwrap();

        assert!(status.is_detached);
        assert!(!status.is_published);
        assert_eq!(status.ahead, 0);
        assert_eq!(status.behind, 0);
    });
}

#[test]
#[serial]
fn publish_branch_pushes_current_branch_and_sets_upstream() {
    run_async(async {
        let repo = TestRepo::new();
        repo.commit_file("README.md", "# Test", "Initial commit");
        let _remote = repo.setup_remote();
        repo.create_branch("feature/publish-me");

        let service = setup_branch_service(&repo);
        let message = service.publish_branch().await.unwrap();

        assert_eq!(message, "Published `feature/publish-me` to origin");
        assert_eq!(
            repo.git(&["rev-parse", "--abbrev-ref", "@{upstream}"]),
            "origin/feature/publish-me"
        );
        assert_eq!(
            repo.git(&["rev-parse", "HEAD"]),
            repo.git(&["rev-parse", "refs/remotes/origin/feature/publish-me"])
        );
    });
}

// ══════════════════════════════════════════════════════════════════════════════
// LIST BRANCHES TESTS
// ══════════════════════════════════════════════════════════════════════════════

#[test]
#[serial]
fn list_branches_single_branch() {
    run_async(async {
        let repo = TestRepo::new();
        repo.commit_file("README.md", "# Test", "Initial commit");

        let service = setup_branch_service(&repo);
        let branches = service.list_branches(BranchKind::Local).await.unwrap();

        assert_eq!(branches.len(), 1);
    });
}

#[test]
#[serial]
fn list_branches_multiple_branches() {
    run_async(async {
        let repo = TestRepo::new();
        repo.commit_file("README.md", "# Test", "Initial commit");
        repo.create_branch("feature/one");
        repo.git(&["switch", "-c", "feature/two"]);
        repo.git(&["switch", "-c", "bugfix/issue-123"]);

        let service = setup_branch_service(&repo);
        let branches = service.list_branches(BranchKind::Local).await.unwrap();

        // Should have main + 3 feature branches
        assert!(branches.len() >= 4);

        let branch_names: Vec<&str> = branches.iter().map(|b| b.name.as_str()).collect();
        assert!(branch_names.contains(&"feature/one"));
        assert!(branch_names.contains(&"feature/two"));
        assert!(branch_names.contains(&"bugfix/issue-123"));
    });
}

#[test]
#[serial]
fn list_branches_marks_head_correctly() {
    run_async(async {
        let repo = TestRepo::new();
        repo.commit_file("README.md", "# Test", "Initial commit");
        repo.create_branch("feature/current");

        let service = setup_branch_service(&repo);
        let branches = service.list_branches(BranchKind::Local).await.unwrap();

        // At least one branch should be marked as HEAD (the current branch)
        let head_branches: Vec<_> = branches.iter().filter(|b| b.is_head).collect();
        assert!(
            !head_branches.is_empty(),
            "Expected at least one HEAD branch"
        );

        // The current branch should be feature/current since we just created and switched to it
        let current = service.get_current_branch().await.unwrap();
        assert_eq!(current.name, "feature/current");
    });
}

// ══════════════════════════════════════════════════════════════════════════════
// CREATE BRANCH TESTS
// ══════════════════════════════════════════════════════════════════════════════

#[test]
#[serial]
fn create_branch_simple() {
    run_async(async {
        let repo = TestRepo::new();
        repo.commit_file("README.md", "# Test", "Initial commit");

        let service = setup_branch_service(&repo);
        let result = service.create("feature/new-branch", None).await;

        assert!(result.is_ok());
        assert!(result.unwrap().contains("feature/new-branch"));

        // Verify we're now on the new branch
        let current = service.get_current_branch().await.unwrap();
        assert_eq!(current.name, "feature/new-branch");
    });
}

#[test]
#[serial]
fn create_branch_already_exists() {
    run_async(async {
        let repo = TestRepo::new();
        repo.commit_file("README.md", "# Test", "Initial commit");
        repo.create_branch("existing-branch");
        repo.switch_branch("main");

        let service = setup_branch_service(&repo);
        let result = service.create("existing-branch", None).await;

        assert!(result.is_err());
        let err = result.unwrap_err();
        // Git error message should mention the branch already exists
        assert!(
            err.contains("exists") || err.contains("already") || err.contains("fatal"),
            "Expected error about existing branch, got: {err}"
        );
    });
}

#[test]
#[serial]
fn create_branch_with_slash_in_name() {
    run_async(async {
        let repo = TestRepo::new();
        repo.commit_file("README.md", "# Test", "Initial commit");

        let service = setup_branch_service(&repo);
        let result = service.create("feature/deeply/nested/branch", None).await;

        assert!(result.is_ok());

        let current = service.get_current_branch().await.unwrap();
        assert_eq!(current.name, "feature/deeply/nested/branch");
    });
}

#[test]
#[serial]
fn create_branch_with_uncommitted_changes_bring_changes() {
    run_async(async {
        let repo = TestRepo::new();
        repo.commit_file("README.md", "# Test", "Initial commit");
        repo.create_file("uncommitted.txt", "uncommitted content");

        assert!(repo.has_changes());

        let service = setup_branch_service(&repo);
        let result = service
            .create(
                "feature/with-changes",
                Some(UncommittedChangesStrategy::BringChanges),
            )
            .await;

        // Should succeed and bring changes along
        assert!(result.is_ok());

        // Should still have uncommitted changes
        assert!(repo.has_changes());

        let current = service.get_current_branch().await.unwrap();
        assert_eq!(current.name, "feature/with-changes");
    });
}

#[test]
#[serial]
fn create_branch_with_uncommitted_changes_stash() {
    run_async(async {
        let repo = TestRepo::new();
        repo.commit_file("README.md", "# Test", "Initial commit");
        repo.create_file("uncommitted.txt", "uncommitted content");

        assert!(repo.has_changes());

        let service = setup_branch_service(&repo);
        let result = service
            .create(
                "feature/stashed",
                Some(UncommittedChangesStrategy::StashOnCurrentBranch),
            )
            .await;

        assert!(result.is_ok());

        // Changes should be stashed (working tree clean)
        assert!(!repo.has_changes());

        // Should have a stash
        let stash_list = repo.stash_list();
        assert!(!stash_list.is_empty());
    });
}

// ══════════════════════════════════════════════════════════════════════════════
// SWITCH BRANCH TESTS
// ══════════════════════════════════════════════════════════════════════════════

#[test]
#[serial]
fn switch_branch_simple() {
    run_async(async {
        let repo = TestRepo::new();
        repo.commit_file("README.md", "# Test", "Initial commit");
        repo.create_branch("feature/target");
        repo.switch_branch("main");

        let service = setup_branch_service(&repo);
        let result = service.switch("feature/target", None).await;

        assert!(result.is_ok());

        let current = service.get_current_branch().await.unwrap();
        assert_eq!(current.name, "feature/target");
    });
}

#[test]
#[serial]
fn switch_branch_nonexistent() {
    run_async(async {
        let repo = TestRepo::new();
        repo.commit_file("README.md", "# Test", "Initial commit");

        let service = setup_branch_service(&repo);
        let result = service.switch("nonexistent-branch", None).await;

        assert!(result.is_err());
    });
}

#[test]
#[serial]
fn switch_branch_with_uncommitted_changes_no_conflict() {
    run_async(async {
        let repo = TestRepo::new();
        repo.commit_file("README.md", "# Test", "Initial commit");
        repo.create_branch("feature/target");
        repo.switch_branch("main");

        // Create a new file (not conflicting)
        repo.create_file("new-file.txt", "content");

        let service = setup_branch_service(&repo);
        let result = service
            .switch(
                "feature/target",
                Some(UncommittedChangesStrategy::BringChanges),
            )
            .await;

        // Should succeed since the new file doesn't conflict
        assert!(result.is_ok());
    });
}

#[test]
#[serial]
fn switch_branch_with_uncommitted_changes_stash_strategy() {
    run_async(async {
        let repo = TestRepo::new();
        repo.commit_file("README.md", "# Test", "Initial commit");
        repo.create_branch("feature/target");
        repo.switch_branch("main");

        // Create uncommitted changes
        repo.create_file("changes.txt", "some changes");

        let service = setup_branch_service(&repo);
        let result = service
            .switch(
                "feature/target",
                Some(UncommittedChangesStrategy::StashOnCurrentBranch),
            )
            .await;

        assert!(result.is_ok());

        // Working tree should be clean
        assert!(!repo.has_changes());

        // Should be on target branch
        let current = service.get_current_branch().await.unwrap();
        assert_eq!(current.name, "feature/target");
    });
}

#[test]
#[serial]
fn switch_branch_with_conflicting_changes() {
    run_async(async {
        let repo = TestRepo::new();
        repo.commit_file("shared.txt", "original content", "Initial commit");

        // Create target branch with different content
        repo.create_branch("feature/target");
        std::fs::write(repo.path().join("shared.txt"), "target content").unwrap();
        repo.add_all();
        repo.commit("Change on target");

        // Go back to main and modify the same file
        repo.switch_branch("main");
        std::fs::write(repo.path().join("shared.txt"), "main content").unwrap();

        let service = setup_branch_service(&repo);

        // Try to switch with BringChanges - should fail due to conflict
        let result = service
            .switch(
                "feature/target",
                Some(UncommittedChangesStrategy::BringChanges),
            )
            .await;

        // Should fail because of conflicting changes
        assert!(result.is_err());
    });
}

#[test]
#[serial]
fn switch_branch_stash_rollback_on_failure() {
    run_async(async {
        let repo = TestRepo::new();
        repo.commit_file("README.md", "# Test", "Initial commit");

        // Create uncommitted changes
        repo.create_file("changes.txt", "some changes");

        let service = setup_branch_service(&repo);

        // Try to switch to a non-existent branch with stash strategy
        let result = service
            .switch(
                "nonexistent-branch",
                Some(UncommittedChangesStrategy::StashOnCurrentBranch),
            )
            .await;

        // Should fail
        assert!(result.is_err());

        // The stash should be rolled back (popped), so changes should still be present
        // This tests the rollback logic in the switch method
        assert!(repo.has_changes());
    });
}

// ══════════════════════════════════════════════════════════════════════════════
// BRANCH INFO TESTS
// ══════════════════════════════════════════════════════════════════════════════

#[test]
#[serial]
fn get_branch_info() {
    run_async(async {
        let repo = TestRepo::new();
        repo.commit_file("README.md", "# Test", "Initial commit");

        let service = setup_branch_service(&repo);
        let info = service.get_branch_info("main").await.unwrap();

        assert_eq!(info.name, "main");
        assert!(!info.commit.id.is_empty());
        assert!(!info.commit.summary.is_empty());
    });
}

#[test]
#[serial]
fn get_branch_info_nonexistent() {
    run_async(async {
        let repo = TestRepo::new();
        repo.commit_file("README.md", "# Test", "Initial commit");

        let service = setup_branch_service(&repo);
        let result = service.get_branch_info("nonexistent").await;

        assert!(result.is_err());
    });
}

// ══════════════════════════════════════════════════════════════════════════════
// HAS UNCOMMITTED CHANGES TESTS
// ══════════════════════════════════════════════════════════════════════════════

#[test]
#[serial]
fn has_uncommitted_changes_false_when_clean() {
    run_async(async {
        let repo = TestRepo::new();
        repo.commit_file("README.md", "# Test", "Initial commit");

        let service = setup_branch_service(&repo);
        let has_changes = service.has_uncommitted_changes().await.unwrap();

        assert!(!has_changes);
    });
}

#[test]
#[serial]
fn has_uncommitted_changes_true_with_modified() {
    run_async(async {
        let repo = TestRepo::new();
        repo.commit_file("README.md", "# Test", "Initial commit");
        std::fs::write(repo.path().join("README.md"), "modified").unwrap();

        let service = setup_branch_service(&repo);
        let has_changes = service.has_uncommitted_changes().await.unwrap();

        assert!(has_changes);
    });
}

#[test]
#[serial]
fn has_uncommitted_changes_true_with_untracked() {
    run_async(async {
        let repo = TestRepo::new();
        repo.commit_file("README.md", "# Test", "Initial commit");
        repo.create_file("untracked.txt", "content");

        let service = setup_branch_service(&repo);
        let has_changes = service.has_uncommitted_changes().await.unwrap();

        assert!(has_changes);
    });
}

#[test]
#[serial]
fn has_uncommitted_changes_true_with_staged() {
    run_async(async {
        let repo = TestRepo::new();
        repo.commit_file("README.md", "# Test", "Initial commit");
        repo.create_file("new.txt", "content");
        repo.add("new.txt");

        let service = setup_branch_service(&repo);
        let has_changes = service.has_uncommitted_changes().await.unwrap();

        assert!(has_changes);
    });
}
