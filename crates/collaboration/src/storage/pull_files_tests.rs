//! Native writer-admission races share the public storage fixture.
use super::*;
use crate as collaboration;
use crate::local_links::*;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
#[path = "../../tests/support/pull_file_fixture.rs"]
mod fixture;
use fixture::*;

async fn linked_fixture() -> (
    tempfile::TempDir,
    Store,
    PullFileDiffRequest,
    PullFileMembershipReceipt,
    PullFileArtifact,
    LocalLinkQuery,
    LocalLinkVersion,
) {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path().join("cache.db")).await.unwrap();
    let a = seed(&store, "a").await;
    let saved = publish(&store, &a, &["a"]).await;
    let request = request(&saved, &a);
    let membership = store
        .verify_pull_file_membership(request.clone())
        .await
        .unwrap();
    let mut artifact = artifact(&membership, "local diff");
    artifact.source = Some(PullFileSource {
        strategy: PullFileSourceStrategy::LocalExactRange,
        adapter_version: 1,
    });
    artifact.validation = Some(PullFileArtifactValidation::LocalExactRange {
        local_validated_at: "2026-10-08T00:00:00Z".into(),
        resolved_merge_base_oid: BASE.into(),
    });
    let query = LocalLinkQuery {
        local_repository_id: "local".into(),
        registration_proof: Some("registered-clone".into()),
        remote_digest: Some("native-remotes".into()),
        endpoints: vec![LocalRemoteEndpoint {
            remote_name: "origin".into(),
            direction: LinkDirection::Fetch,
            ordinal: 0,
            transport: LinkTransport::Https,
            host: "github.com".into(),
            port: 443,
            path: "owner/project.git".into(),
        }],
    };
    let snapshot = store.local_link_snapshot(query.clone()).await.unwrap();
    let receipt = store
        .confirm_local_link(ConfirmLocalLink {
            query: query.clone(),
            candidate_id: snapshot.resolutions[0].candidates[0].id.clone(),
            expected_authorization_view: snapshot.authorization_view,
            expected_bindings_generation: snapshot.bindings_generation,
            replace: None,
        })
        .await
        .unwrap();
    let link = LocalLinkVersion {
        id: receipt.link.id,
        generation: receipt.link.generation,
    };
    (dir, store, request, membership, artifact, query, link)
}

#[tokio::test]
async fn retired_local_diff_caller_waiting_for_writer_cannot_admit_artifact() {
    let (_dir, store, request, membership, artifact, query, link) = linked_fixture().await;
    let before = store.pull_file_artifact(request.clone()).await.unwrap();
    let writer = store.inner.writer.lock().await;
    let alive = Arc::new(AtomicBool::new(true));
    let owner = alive.clone();
    let worker = store.clone();
    let task = tokio::spawn(async move {
        worker
            .apply_local_pull_file_artifact_checked(
                request,
                membership,
                artifact,
                query,
                link,
                || {
                    if owner.load(Ordering::SeqCst) {
                        Ok(())
                    } else {
                        Err(stale())
                    }
                },
            )
            .await
    });
    tokio::task::yield_now().await;
    assert!(!task.is_finished());
    alive.store(false, Ordering::SeqCst);
    drop(writer);
    assert_eq!(task.await.unwrap().unwrap_err().code, ErrorCode::StaleView);
    assert_eq!(
        store
            .pull_file_artifact(before.request.clone())
            .await
            .unwrap(),
        before
    );
}

#[tokio::test]
async fn local_diff_caller_retiring_after_mutation_rolls_back_rows_revision_and_accounting() {
    let (dir, store, request, membership, artifact, query, link) = linked_fixture().await;
    let before = store.pull_file_artifact(request.clone()).await.unwrap();
    let usage = store.cache_usage().await.unwrap().indexed_logical_bytes;
    let count = AtomicUsize::new(0);
    assert!(
        store
            .apply_local_pull_file_artifact_checked(
                request.clone(),
                membership,
                artifact,
                query,
                link,
                || if count.fetch_add(1, Ordering::SeqCst) == 0 {
                    Ok(())
                } else {
                    Err(stale())
                }
            )
            .await
            .is_err()
    );
    let mut db = database(&dir.path().join("cache.db")).await;
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM pull_file_artifacts")
            .fetch_one(&mut db)
            .await
            .unwrap(),
        0
    );
    assert_eq!(count.load(Ordering::SeqCst), 2);
    assert_eq!(store.pull_file_artifact(request).await.unwrap(), before);
    assert_eq!(
        store.cache_usage().await.unwrap().indexed_logical_bytes,
        usage
    );
}

#[tokio::test]
async fn removed_or_replaced_link_while_local_commit_waits_is_rechecked_in_writer_snapshot() {
    for replace in [false, true] {
        let (_dir, store, request, membership, artifact, query, link) = linked_fixture().await;
        let before = store.pull_file_artifact(request.clone()).await.unwrap();
        let mut writer = store.inner.writer.lock().await;
        let worker = store.clone();
        let saved_request = request.clone();
        let saved_link = link.clone();
        let task = tokio::spawn(async move {
            worker
                .apply_local_pull_file_artifact_checked(
                    saved_request,
                    membership,
                    artifact,
                    query,
                    saved_link,
                    || Ok(()),
                )
                .await
        });
        tokio::task::yield_now().await;
        assert!(!task.is_finished());
        if replace {
            sqlx::query("UPDATE local_repository_links SET generation=generation+1 WHERE id=?")
                .bind(&link.id)
                .execute(&mut *writer)
                .await
                .unwrap();
        } else {
            sqlx::query("DELETE FROM local_repository_links WHERE id=?")
                .bind(&link.id)
                .execute(&mut *writer)
                .await
                .unwrap();
        }
        drop(writer);
        assert_eq!(task.await.unwrap().unwrap_err().code, ErrorCode::StaleView);
        assert_eq!(store.pull_file_artifact(request).await.unwrap(), before);
    }
}

#[tokio::test]
async fn local_artifact_requires_registered_link_and_cannot_use_provider_admission() {
    let (_dir, store, request, membership, artifact, query, link) = linked_fixture().await;
    assert!(
        store
            .apply_pull_file_artifact(request.clone(), membership.clone(), artifact.clone())
            .await
            .is_err()
    );
    let mut changed = query.clone();
    changed.remote_digest = Some("changed-remotes".into());
    assert!(
        store
            .apply_local_pull_file_artifact_checked(
                request.clone(),
                membership.clone(),
                artifact.clone(),
                changed,
                link.clone(),
                || Ok(())
            )
            .await
            .is_err()
    );
    let mut changed = query.clone();
    changed.registration_proof = Some("replacement-clone".into());
    assert!(
        store
            .apply_local_pull_file_artifact_checked(
                request.clone(),
                membership.clone(),
                artifact.clone(),
                changed,
                link.clone(),
                || Ok(())
            )
            .await
            .is_err()
    );
    store
        .apply_local_pull_file_artifact_checked(
            request.clone(),
            membership,
            artifact,
            query,
            link,
            || Ok(()),
        )
        .await
        .unwrap();
    assert!(
        store
            .pull_file_artifact(request)
            .await
            .unwrap()
            .artifact
            .is_some()
    );
}
