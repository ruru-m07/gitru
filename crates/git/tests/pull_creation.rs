mod common;
use common::TestRepo;
use git::{core::RepoServices, service::pull_creation::PullCreationInspectionError as Error};

#[tokio::test]
async fn source_observation_is_uncached_and_never_modifies_dirty_work() {
    let r = TestRepo::new();
    r.commit_file("file", "base", "base");
    r.git(&["branch", "feature"]);
    let s = RepoServices::new(r.path_str()).unwrap();
    let first = s.pull_creation().inspect("feature").await.unwrap();
    r.commit_file("file", "next", "next");
    let current = r.head_commit();
    r.git(&["update-ref", "refs/heads/feature", &current]);
    r.create_file("file", "private unstaged text");
    r.create_file("untracked", "private untracked text");
    let index = std::fs::read(r.path().join(".git/index")).unwrap();
    let status = r.git(&["status", "--porcelain=v1"]);
    let next = s.pull_creation().inspect("feature").await.unwrap();
    assert_ne!(first.source_oid, next.source_oid);
    assert_eq!(next.source_oid, current);
    assert_eq!(next.current_branch, "main");
    assert_eq!(next.current_head_oid, current);
    assert_eq!(index, std::fs::read(r.path().join(".git/index")).unwrap());
    assert_eq!(status, r.git(&["status", "--porcelain=v1"]));
    assert_eq!(
        std::fs::read_to_string(r.path().join("file")).unwrap(),
        "private unstaged text"
    );
}

#[tokio::test]
async fn detached_unborn_missing_and_revision_expressions_are_refused() {
    let r = TestRepo::new();
    let s = RepoServices::new(r.path_str()).unwrap();
    assert_eq!(
        s.pull_creation().inspect("main").await.unwrap_err(),
        Error::Unborn
    );
    r.commit_file("file", "base", "base");
    for value in [
        "",
        "-x",
        "main~1",
        "main^{commit}",
        "main..x",
        "main\0x",
        "refs/heads/main",
        "main\nx",
    ] {
        assert_eq!(
            s.pull_creation().inspect(value).await.unwrap_err(),
            Error::InvalidBranch,
            "{value:?}"
        );
    }
    assert_eq!(
        s.pull_creation().inspect("missing").await.unwrap_err(),
        Error::MissingSource
    );
    r.git(&["checkout", "--detach"]);
    assert_eq!(
        s.pull_creation().inspect("main").await.unwrap_err(),
        Error::Detached
    );
}

#[tokio::test]
async fn literal_unicode_packed_branch_and_linked_worktree_are_supported() {
    let r = TestRepo::new();
    r.commit_file("file", "base", "base");
    r.git(&["branch", "feature/雪"]);
    r.git(&["pack-refs", "--all", "--prune"]);
    let linked = tempfile::tempdir().unwrap();
    let path = linked.path().join("linked");
    r.git(&["worktree", "add", path.to_str().unwrap(), "feature/雪"]);
    let s = RepoServices::new(path.to_str().unwrap()).unwrap();
    let value = s.pull_creation().inspect("feature/雪").await.unwrap();
    assert_eq!(value.source_oid, r.head_commit());
    assert_eq!(value.current_branch, "feature/雪");
    assert_eq!(value.paths.worktree, path.canonicalize().unwrap());
    assert_eq!(
        value.paths.common_dir,
        r.path().join(".git").canonicalize().unwrap()
    );
    assert_ne!(value.paths.git_dir, value.paths.common_dir);
}

#[tokio::test]
async fn replacement_objects_do_not_change_literal_tip_authority() {
    let r = TestRepo::new();
    r.commit_file("file", "base", "base");
    let first = r.head_commit();
    r.git(&["branch", "feature"]);
    r.commit_file("file", "next", "next");
    let second = r.head_commit();
    r.git(&["replace", &first, &second]);
    let s = RepoServices::new(r.path_str()).unwrap();
    assert_eq!(
        s.pull_creation()
            .inspect("feature")
            .await
            .unwrap()
            .source_oid,
        first
    );
    assert_eq!(r.git(&["replace", "-l"]), first);
}

#[tokio::test]
async fn partial_clone_inspection_does_not_hydrate_blobs() {
    let r = TestRepo::new();
    r.commit_file("private", "a missing promisor blob", "base");
    let blob = r.git(&["rev-parse", "HEAD:private"]);
    let clone = TestRepo::blobless_clone(&r);
    assert!(!clone.has_local_object(&blob));
    let s = RepoServices::new(clone.path_str()).unwrap();
    assert_eq!(
        s.pull_creation().inspect("main").await.unwrap().source_oid,
        r.head_commit()
    );
    assert!(!clone.has_local_object(&blob));
}

#[tokio::test]
async fn unsupported_sha256_identity_is_not_coerced_to_github_sha1() {
    let Some(r) = TestRepo::new_sha256() else {
        return;
    };
    r.commit_file("file", "base", "base");
    let s = RepoServices::new(r.path_str()).unwrap();
    assert_eq!(
        s.pull_creation().inspect("main").await.unwrap_err(),
        Error::InvalidObject
    );
}
