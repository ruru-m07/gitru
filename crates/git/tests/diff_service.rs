//! Integration tests for DiffService image asset diff metadata.

mod common;

use common::{TestRepo, run_async};
use git::context::RepoContext;
use git::models::diff::AssetDiffKind;
use git::models::status::FileStatusKind;
use git::service::diff::DiffService;
use serial_test::serial;
use std::sync::Arc;

fn setup_diff_service(repo: &TestRepo) -> DiffService {
    let ctx = Arc::new(RepoContext::new(repo.path_str()).expect("failed to create repo context"));
    DiffService::new(ctx)
}

fn write_binary_file(repo: &TestRepo, path: &str, bytes: &[u8]) {
    let target = repo.path().join(path);
    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent).expect("failed to create parent directories");
    }
    std::fs::write(target, bytes).expect("failed to write binary file");
}

#[test]
#[serial]
fn image_diff_worktree_modified_has_before_and_after() {
    run_async(async {
        let repo = TestRepo::new();
        write_binary_file(&repo, "logo.png", &[1, 2, 3, 0, 4]);
        repo.add("logo.png");
        repo.commit("add logo");

        write_binary_file(&repo, "logo.png", &[8, 9, 10, 0, 11]);

        let service = setup_diff_service(&repo);
        let diff = service
            .get_patch_by_file_path(
                "logo.png",
                None,
                Some(&[FileStatusKind::WorktreeModified]),
                None,
                None,
                None,
            )
            .await
            .expect("failed to get diff");

        let asset = diff.asset_diff.expect("expected asset diff");
        assert!(matches!(asset.kind, AssetDiffKind::Image));
        assert_eq!(
            asset.before.as_ref().map(|e| e.logical_path.as_str()),
            Some("logo.png")
        );
        assert_eq!(
            asset.after.as_ref().map(|e| e.logical_path.as_str()),
            Some("logo.png")
        );
    });
}

#[test]
#[serial]
fn image_diff_rename_maps_old_and_new_paths() {
    run_async(async {
        let repo = TestRepo::new();
        write_binary_file(&repo, "old-name.png", &[1, 2, 3, 4, 5]);
        repo.add("old-name.png");
        repo.commit("add old image");

        repo.git(&["mv", "old-name.png", "new-name.png"]);

        let service = setup_diff_service(&repo);
        let diff = service
            .get_patch_by_file_path(
                "old-name.png",
                Some("new-name.png"),
                Some(&[FileStatusKind::IndexRenamed]),
                None,
                None,
                None,
            )
            .await
            .expect("failed to get diff");

        let asset = diff.asset_diff.expect("expected asset diff");
        assert_eq!(
            asset.before.as_ref().map(|e| e.logical_path.as_str()),
            Some("old-name.png")
        );
        assert_eq!(
            asset.after.as_ref().map(|e| e.logical_path.as_str()),
            Some("new-name.png")
        );
    });
}

#[test]
#[serial]
fn image_diff_history_commit_has_before_and_after() {
    run_async(async {
        let repo = TestRepo::new();
        write_binary_file(&repo, "history.png", &[1, 2, 3, 4]);
        repo.add("history.png");
        repo.commit("v1");

        write_binary_file(&repo, "history.png", &[9, 8, 7, 6]);
        repo.add("history.png");
        repo.commit("v2");

        let head = repo.head_commit();

        let service = setup_diff_service(&repo);
        let diff = service
            .get_patch_by_file_path(
                "history.png",
                None,
                Some(&[FileStatusKind::IndexModified]),
                None,
                Some(&head),
                Some(1),
            )
            .await
            .expect("failed to get history diff");

        let asset = diff.asset_diff.expect("expected asset diff");
        assert!(asset.before.is_some());
        assert!(asset.after.is_some());
    });
}

#[test]
#[serial]
fn image_diff_new_file_has_only_after() {
    run_async(async {
        let repo = TestRepo::new();
        repo.commit_file("README.md", "init", "init");
        write_binary_file(&repo, "new.png", &[5, 6, 7, 8]);

        let service = setup_diff_service(&repo);
        let diff = service
            .get_patch_by_file_path(
                "new.png",
                None,
                Some(&[FileStatusKind::WorktreeNew]),
                None,
                None,
                None,
            )
            .await
            .expect("failed to get diff for new image");

        let asset = diff.asset_diff.expect("expected asset diff");
        assert!(asset.before.is_none());
        assert!(asset.after.is_some());
    });
}

#[test]
#[serial]
fn non_image_binary_returns_binary_or_none_asset_diff() {
    run_async(async {
        let repo = TestRepo::new();
        write_binary_file(&repo, "data.bin", &[0, 1, 2, 3, 4, 5]);
        repo.add("data.bin");
        repo.commit("add binary");

        write_binary_file(&repo, "data.bin", &[9, 0, 8, 0, 7, 0]);

        let service = setup_diff_service(&repo);
        let diff = service
            .get_patch_by_file_path(
                "data.bin",
                None,
                Some(&[FileStatusKind::WorktreeModified]),
                None,
                None,
                None,
            )
            .await
            .expect("failed to get binary diff");

        if let Some(asset) = diff.asset_diff {
            assert!(matches!(asset.kind, AssetDiffKind::Binary));
        }
    });
}
