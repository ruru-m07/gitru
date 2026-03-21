//! Integration tests for BlameService.

mod common;

use common::{TestRepo, run_async};
use git::context::RepoContext;
use git::models::status::FileStatusKind;
use git::service::blame::BlameService;
use serial_test::serial;
use std::sync::Arc;

fn setup_blame_service(repo: &TestRepo) -> BlameService {
    let ctx = Arc::new(RepoContext::new(repo.path_str()).expect("failed to create repo context"));
    BlameService::new(ctx)
}

#[test]
#[serial]
fn text_blame_worktree_modified_returns_old_and_new_blame() {
    run_async(async {
        let repo = TestRepo::new();
        repo.commit_file("note.txt", "line one\nline two\n", "add note");
        repo.create_file("note.txt", "line one changed\nline two\n");

        let service = setup_blame_service(&repo);
        let blame = service
            .get_blame_by_file_path(
                "note.txt",
                None,
                Some(&[FileStatusKind::WorktreeModified]),
                None,
                None,
                None,
            )
            .await
            .expect("failed to get blame");

        let old_blame = blame.old_blame.expect("old blame should be present");
        let new_blame = blame.new_blame.expect("new blame should be present");
        assert!(!old_blame.is_empty());
        assert!(!new_blame.is_empty());
    });
}

#[test]
#[serial]
fn text_blame_deleted_file_returns_empty_new_blame() {
    run_async(async {
        let repo = TestRepo::new();
        repo.commit_file("gone.txt", "line\n", "add file");
        std::fs::remove_file(repo.path().join("gone.txt")).expect("failed to remove file");

        let service = setup_blame_service(&repo);
        let blame = service
            .get_blame_by_file_path(
                "gone.txt",
                None,
                Some(&[FileStatusKind::WorktreeDeleted]),
                None,
                None,
                None,
            )
            .await
            .expect("failed to get blame");

        let old_blame = blame.old_blame.expect("old blame should be present");
        let new_blame = blame.new_blame.expect("new blame should be present");
        assert!(!old_blame.is_empty());
        assert!(new_blame.is_empty());
    });
}

#[test]
#[serial]
fn text_blame_commit_mode_returns_old_and_new_blame() {
    run_async(async {
        let repo = TestRepo::new();
        repo.commit_file("commit-note.txt", "v1\n", "v1");
        repo.commit_file("commit-note.txt", "v2\n", "v2");
        let head = repo.head_commit();

        let service = setup_blame_service(&repo);
        let blame = service
            .get_blame_by_file_path(
                "commit-note.txt",
                None,
                Some(&[FileStatusKind::IndexModified]),
                None,
                Some(&head),
                Some(1),
            )
            .await
            .expect("failed to get commit blame");

        let old_blame = blame.old_blame.expect("old blame should be present");
        let new_blame = blame.new_blame.expect("new blame should be present");
        assert!(!old_blame.is_empty());
        assert!(!new_blame.is_empty());
    });
}
