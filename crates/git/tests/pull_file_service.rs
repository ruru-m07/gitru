mod common;

use common::TestRepo;
use git::{
    core::RepoServices,
    models::pull_file::{
        LocalPullFileComparison, LocalPullFileDiff, LocalPullFileDiffRequest,
        LocalPullFileDiffState, LocalPullFileDiffUnavailableReason as Unavailable,
        LocalPullFileDiffUnsupportedReason as Unsupported,
    },
};

fn test_repo() -> TestRepo {
    let repo = TestRepo::new();
    repo.git(&["config", "commit.gpgSign", "false"]);
    repo
}

fn request(
    base_oid: &str,
    head_oid: &str,
    merge_base_oid: Option<&str>,
    old_path: Option<&str>,
    new_path: Option<&str>,
) -> LocalPullFileDiffRequest {
    LocalPullFileDiffRequest {
        base_oid: base_oid.into(),
        head_oid: head_oid.into(),
        merge_base_oid: merge_base_oid.map(str::to_owned),
        old_path: old_path.map(str::to_owned),
        new_path: new_path.map(str::to_owned),
    }
}

async fn selected(repo: &TestRepo, request: LocalPullFileDiffRequest) -> LocalPullFileDiff {
    RepoServices::new(repo.path_str())
        .expect("services")
        .pull_file()
        .selected_diff(request)
        .await
        .expect("valid request")
}

fn text(result: &LocalPullFileDiff) -> &str {
    match &result.state {
        LocalPullFileDiffState::Text { unified_diff } => unified_diff,
        state => panic!("expected text, got {state:?}"),
    }
}

#[tokio::test]
async fn compares_the_unique_merge_base_to_head_on_divergent_branches() {
    let repo = test_repo();
    repo.commit_file("shared.txt", "root\n", "root");
    let merge_base = repo.head_commit();

    repo.create_file("shared.txt", "base-only\n");
    repo.add("shared.txt");
    repo.commit("advance target");
    let base_oid = repo.head_commit();

    repo.git(&["switch", "-c", "feature", &merge_base]);
    repo.create_file("shared.txt", "feature-only\n");
    repo.add("shared.txt");
    repo.commit("advance source");
    let head_oid = repo.head_commit();

    let result = selected(
        &repo,
        request(
            &base_oid,
            &head_oid,
            Some(&merge_base),
            Some("shared.txt"),
            Some("shared.txt"),
        ),
    )
    .await;
    let provenance = result.provenance.as_ref().expect("resolved provenance");
    assert_eq!(
        provenance.comparison,
        LocalPullFileComparison::MergeBaseToHead
    );
    assert_eq!(provenance.resolved_merge_base_oid, merge_base);
    assert!(text(&result).contains("+feature-only"));
    assert!(!text(&result).contains("base-only"));

    let mismatch = selected(
        &repo,
        request(
            &base_oid,
            &head_oid,
            Some(&base_oid),
            Some("shared.txt"),
            Some("shared.txt"),
        ),
    )
    .await;
    assert_eq!(
        mismatch.state,
        LocalPullFileDiffState::Unavailable {
            reason: Unavailable::KnownMergeBaseMismatch
        }
    );
    assert_eq!(
        mismatch
            .provenance
            .expect("mismatch provenance")
            .resolved_merge_base_oid,
        merge_base
    );
}

#[tokio::test]
async fn distinguishes_missing_base_and_head_objects_without_fetching() {
    let repo = test_repo();
    repo.commit_file("file.txt", "root\n", "root");
    let oid = repo.head_commit();
    let missing = format!("{}1", "0".repeat(oid.len() - 1));

    let missing_base = selected(
        &repo,
        request(&missing, &oid, None, Some("file.txt"), Some("file.txt")),
    )
    .await;
    assert_eq!(
        missing_base.state,
        LocalPullFileDiffState::Unavailable {
            reason: Unavailable::BaseObjectMissing
        }
    );

    let missing_head = selected(
        &repo,
        request(&oid, &missing, None, Some("file.txt"), Some("file.txt")),
    )
    .await;
    assert_eq!(
        missing_head.state,
        LocalPullFileDiffState::Unavailable {
            reason: Unavailable::HeadObjectMissing
        }
    );
}

#[tokio::test]
async fn refuses_ambiguous_and_unrelated_histories() {
    let repo = test_repo();
    repo.commit_file("file.txt", "root\n", "root");
    let root = repo.head_commit();
    let tree = repo.git(&["rev-parse", "HEAD^{tree}"]);
    let a1 = repo.git(&["commit-tree", &tree, "-p", &root, "-m", "a1"]);
    let b1 = repo.git(&["commit-tree", &tree, "-p", &root, "-m", "b1"]);
    let a2 = repo.git(&["commit-tree", &tree, "-p", &a1, "-p", &b1, "-m", "a2"]);
    let b2 = repo.git(&["commit-tree", &tree, "-p", &b1, "-p", &a1, "-m", "b2"]);
    let ambiguous = selected(
        &repo,
        request(&a2, &b2, None, Some("file.txt"), Some("file.txt")),
    )
    .await;
    assert_eq!(
        ambiguous.state,
        LocalPullFileDiffState::Unavailable {
            reason: Unavailable::AmbiguousMergeBase
        }
    );

    let unrelated = repo.git(&["commit-tree", &tree, "-m", "unrelated"]);
    let no_ancestor = selected(
        &repo,
        request(&root, &unrelated, None, Some("file.txt"), Some("file.txt")),
    )
    .await;
    assert_eq!(
        no_ancestor.state,
        LocalPullFileDiffState::Unsupported {
            reason: Unsupported::NoCommonAncestor
        }
    );
}

#[tokio::test]
async fn treats_special_unicode_paths_as_one_literal_identity() {
    let repo = test_repo();
    let selected_path = ":(glob)[x]*雪?.txt";
    let unrelated_path = "x-unrelated-雪a.txt";
    repo.create_file(selected_path, "selected root\n");
    repo.create_file(unrelated_path, "unrelated root\n");
    repo.git(&[
        "--literal-pathspecs",
        "add",
        "--",
        selected_path,
        unrelated_path,
    ]);
    repo.commit("root");
    let base_oid = repo.head_commit();

    repo.create_file(selected_path, "selected head\n");
    repo.create_file(unrelated_path, "unrelated head marker\n");
    repo.git(&[
        "--literal-pathspecs",
        "add",
        "--",
        selected_path,
        unrelated_path,
    ]);
    repo.commit("both change");
    let head_oid = repo.head_commit();

    let result = selected(
        &repo,
        request(
            &base_oid,
            &head_oid,
            None,
            Some(selected_path),
            Some(selected_path),
        ),
    )
    .await;
    assert!(text(&result).contains("selected head"));
    assert!(!text(&result).contains("unrelated head marker"));
}

#[tokio::test]
async fn verifies_rename_path_pairs_before_returning_content() {
    let repo = test_repo();
    repo.commit_file("old name.txt", "content\n", "root");
    let base_oid = repo.head_commit();
    repo.git(&["mv", "old name.txt", "new name.txt"]);
    repo.commit("rename");
    let head_oid = repo.head_commit();

    let result = selected(
        &repo,
        request(
            &base_oid,
            &head_oid,
            None,
            Some("old name.txt"),
            Some("new name.txt"),
        ),
    )
    .await;
    assert!(text(&result).contains("similarity index 100%"));

    let wrong_pair = selected(
        &repo,
        request(
            &base_oid,
            &head_oid,
            None,
            Some("old name.txt"),
            Some("wrong name.txt"),
        ),
    )
    .await;
    assert_eq!(
        wrong_pair.state,
        LocalPullFileDiffState::Unavailable {
            reason: Unavailable::ChangeIdentityMismatch
        }
    );
}

#[tokio::test]
async fn preserves_an_unchanged_copy_source_in_the_selected_pair() {
    let repo = test_repo();
    repo.commit_file("source.txt", "copied content\n", "root");
    let base_oid = repo.head_commit();
    repo.create_file("copy.txt", "copied content\n");
    repo.add("copy.txt");
    repo.commit("copy");
    let head_oid = repo.head_commit();

    let result = selected(
        &repo,
        request(
            &base_oid,
            &head_oid,
            None,
            Some("source.txt"),
            Some("copy.txt"),
        ),
    )
    .await;
    assert!(text(&result).contains("similarity index 100%"));
    assert!(text(&result).contains("copy from source.txt"));
    assert!(text(&result).contains("copy to copy.txt"));
}

#[tokio::test]
async fn ignores_replace_refs_and_repository_grafts() {
    let repo = test_repo();
    repo.commit_file("file.txt", "base\n", "root");
    let base_oid = repo.head_commit();
    repo.create_file("file.txt", "head\n");
    repo.add("file.txt");
    repo.commit("change");
    let head_oid = repo.head_commit();

    repo.git(&["replace", &head_oid, &base_oid]);
    std::fs::create_dir_all(repo.path().join(".git/info")).expect("git info directory");
    std::fs::write(
        repo.path().join(".git/info/grafts"),
        format!("{head_oid}\n"),
    )
    .expect("write malicious graft");

    let result = selected(
        &repo,
        request(
            &base_oid,
            &head_oid,
            None,
            Some("file.txt"),
            Some("file.txt"),
        ),
    )
    .await;
    assert!(text(&result).contains("+head"));
}

#[tokio::test]
async fn distinguishes_binary_and_oversized_content() {
    let binary_repo = test_repo();
    binary_repo.create_file("seed.txt", "seed\n");
    binary_repo.create_file(".gitattributes", "binary.dat diff=forced-text\n");
    binary_repo.add_all();
    binary_repo.commit("root");
    let binary_base = binary_repo.head_commit();
    std::fs::write(binary_repo.path().join("binary.dat"), b"a\0b\0c").expect("write binary");
    binary_repo.add("binary.dat");
    binary_repo.commit("binary");
    let binary_head = binary_repo.head_commit();
    binary_repo.git(&["config", "diff.forced-text.binary", "false"]);
    let binary = selected(
        &binary_repo,
        request(&binary_base, &binary_head, None, None, Some("binary.dat")),
    )
    .await;
    assert_eq!(binary.state, LocalPullFileDiffState::Binary);

    let large_repo = test_repo();
    large_repo.commit_file("large.txt", "root\n", "root");
    let large_base = large_repo.head_commit();
    let large = "0123456789abcdef\n".repeat(270_000);
    large_repo.create_file("large.txt", &large);
    large_repo.add("large.txt");
    large_repo.commit("large");
    let large_head = large_repo.head_commit();
    let oversized = selected(
        &large_repo,
        request(
            &large_base,
            &large_head,
            None,
            Some("large.txt"),
            Some("large.txt"),
        ),
    )
    .await;
    assert_eq!(oversized.state, LocalPullFileDiffState::Oversized);
}

#[tokio::test]
async fn missing_promisor_blob_is_not_hydrated() {
    let source = test_repo();
    source.commit_file("large.txt", &"base\n".repeat(100_000), "root");
    let base_oid = source.head_commit();
    source.create_file("large.txt", &"head\n".repeat(100_000));
    source.add("large.txt");
    source.commit("change");
    let head_oid = source.head_commit();
    let head_blob = source.git(&["rev-parse", "HEAD:large.txt"]);

    let clone = TestRepo::blobless_clone(&source);
    assert!(
        !clone.has_local_object(&head_blob),
        "blob unexpectedly present"
    );
    let result = selected(
        &clone,
        request(
            &base_oid,
            &head_oid,
            None,
            Some("large.txt"),
            Some("large.txt"),
        ),
    )
    .await;
    assert_eq!(
        result.state,
        LocalPullFileDiffState::Unavailable {
            reason: Unavailable::GitUnavailable
        }
    );
    assert!(
        !clone.has_local_object(&head_blob),
        "local diff must never hydrate a missing blob"
    );
}

#[cfg(unix)]
#[tokio::test]
async fn configured_external_diff_textconv_and_hooks_never_execute() {
    use std::os::unix::fs::PermissionsExt;

    let repo = test_repo();
    repo.commit_file("file.txt", "base\n", "root");
    let base_oid = repo.head_commit();
    repo.create_file("file.txt", "head\n");
    repo.add("file.txt");
    repo.commit("change");
    let head_oid = repo.head_commit();

    let marker = repo.path().join("external-ran");
    let script = repo.path().join("external.sh");
    std::fs::write(
        &script,
        format!("#!/bin/sh\nprintf ran > '{}'\nexit 99\n", marker.display()),
    )
    .expect("write external command");
    let mut permissions = std::fs::metadata(&script).unwrap().permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(&script, permissions).unwrap();

    let hooks = repo.path().join("hooks");
    std::fs::create_dir(&hooks).expect("hooks dir");
    let hook = hooks.join("post-checkout");
    std::fs::write(
        &hook,
        format!("#!/bin/sh\nprintf hook > '{}'\n", marker.display()),
    )
    .expect("write hook");
    let mut permissions = std::fs::metadata(&hook).unwrap().permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(&hook, permissions).unwrap();

    repo.create_file(".gitattributes", "file.txt diff=evil\n");
    repo.git(&["config", "diff.external", script.to_str().unwrap()]);
    repo.git(&["config", "diff.evil.command", script.to_str().unwrap()]);
    repo.git(&["config", "diff.evil.textconv", script.to_str().unwrap()]);
    repo.git(&["config", "core.hooksPath", hooks.to_str().unwrap()]);

    let result = selected(
        &repo,
        request(
            &base_oid,
            &head_oid,
            None,
            Some("file.txt"),
            Some("file.txt"),
        ),
    )
    .await;
    assert!(text(&result).contains("+head"));
    assert!(!marker.exists(), "repository-configured program executed");
}

#[tokio::test]
async fn supports_sha256_commit_authority_when_git_does() {
    let Some(repo) = TestRepo::new_sha256() else {
        return;
    };
    repo.git(&["config", "commit.gpgSign", "false"]);
    repo.commit_file("file.txt", "base\n", "root");
    let base_oid = repo.head_commit();
    repo.create_file("file.txt", "head\n");
    repo.add("file.txt");
    repo.commit("change");
    let head_oid = repo.head_commit();
    assert_eq!(base_oid.len(), 64);

    let result = selected(
        &repo,
        request(
            &base_oid,
            &head_oid,
            None,
            Some("file.txt"),
            Some("file.txt"),
        ),
    )
    .await;
    assert!(text(&result).contains("+head"));
}
