//! Independent late-cleanup source-state races through synthetic public Store
//! receipts. No credentials, HTTP, sleeps, migrations or global test locking.
use super::*;
use crate::{DetailSubjectBinding, runtime::detail_tests::fixtures};

async fn binding(store: &Store) -> DetailSubjectBinding {
    let subject = store.detail_subject("a", "pull").await.unwrap();
    DetailSubjectBinding {
        repository_id: subject.repository_id.unwrap(),
        repository_provider_id: "1".into(),
        provider_id: subject.provider_id,
        number: subject.number,
        kind: subject.kind,
        head_oid: subject.head_oid,
    }
}

async fn obsolete_terminal_cleanup_keeps_new_intent(change: &str) {
    let directory = tempfile::tempdir().unwrap();
    let store = Store::open(directory.path().join("fences.sqlite"))
        .await
        .unwrap();
    let actor = fixtures::seed(&store, "a").await;
    let draft = store
        .save_draft(LocalDraft {
            account_id: "a".into(),
            subject_id: "pull".into(),
            body: "Authored text survives an obsolete cleanup".into(),
            generation: "0".into(),
        })
        .await
        .unwrap();
    store
        .request_detail("a", "1", "pull", DetailFacet::Comments)
        .await
        .unwrap();
    let mut first = fixtures::commit(&store, &actor, DetailFacet::Comments).await;
    first.entries = vec![fixtures::entry("retained")];
    first.complete = false;
    first.whole_scope = false;
    first.next_cursor = Some("page2".into());
    store.apply_detail(first).await.unwrap();
    let obsolete = store
        .begin_detail("a", "1", "pull", DetailFacet::Comments)
        .await
        .unwrap();
    let mut drift = fixtures::from_lease(&actor, DetailFacet::Comments, obsolete.clone());
    drift.source.source = "changed-representation".into();
    drift.whole_scope = false;
    assert!(is_drift(&store.apply_detail(drift).await.unwrap_err()));
    match change {
        "view" => {
            store
                .set_sync_status(
                    "a",
                    "1",
                    "notifications",
                    SyncStatus {
                        state: SyncState::Error,
                        error: Some(CollaborationError::new(
                            ErrorCode::PermissionDenied,
                            "synthetic unrelated scope denial",
                        )),
                        ..Default::default()
                    },
                )
                .await
                .unwrap();
        }
        "run" => {
            let replacement = store
                .begin_detail("a", "1", "pull", DetailFacet::Comments)
                .await
                .unwrap();
            assert_ne!(replacement.run_id, obsolete.run_id);
        }
        "cursor" | "source" => {
            let mut continuation =
                fixtures::from_lease(&actor, DetailFacet::Comments, obsolete.clone());
            continuation.complete = false;
            continuation.whole_scope = false;
            continuation.entries = vec![fixtures::entry("current")];
            continuation.next_cursor =
                Some(if change == "source" { "page2" } else { "page3" }.into());
            if change == "source" {
                continuation.source.observed_at = "2026-10-03T00:00:01Z".into();
            }
            store.apply_detail(continuation).await.unwrap();
        }
        _ => unreachable!(),
    }
    // A new explicit read was accepted under the current state before the old
    // rejected job resumes its cleanup; its account epoch intentionally matches.
    store
        .request_detail("a", "1", "pull", DetailFacet::Comments)
        .await
        .unwrap();
    let before = store
        .detail(fixtures::query("a", DetailFacet::Comments))
        .await
        .unwrap();
    let revision = store.revision().await.unwrap();
    let error = store
        .validate_detail_dispatch(
            "a",
            "1",
            "pull",
            DetailFacet::Comments,
            &obsolete,
            &binding(&store).await,
        )
        .await
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::StaleView);
    let error = store
        .stop_detail_reconciliation("a", "1", "pull", DetailFacet::Comments, &obsolete)
        .await
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::StaleView);
    assert_eq!(
        store.pending_details().await.unwrap().len(),
        1,
        "obsolete {change} lease cannot clear current explicit intent"
    );
    assert_eq!(
        store
            .detail(fixtures::query("a", DetailFacet::Comments))
            .await
            .unwrap(),
        before
    );
    assert_eq!(store.revision().await.unwrap(), revision);
    assert_eq!(store.draft("a", "pull").await.unwrap(), Some(draft));
}

#[tokio::test]
async fn terminal_cleanup_cannot_clear_intent_after_authorization_view_changes() {
    obsolete_terminal_cleanup_keeps_new_intent("view").await;
}
#[tokio::test]
async fn terminal_cleanup_cannot_clear_intent_after_run_replacement() {
    obsolete_terminal_cleanup_keeps_new_intent("run").await;
}
#[tokio::test]
async fn terminal_cleanup_cannot_clear_intent_after_cursor_advances() {
    obsolete_terminal_cleanup_keeps_new_intent("cursor").await;
}

#[tokio::test]
async fn terminal_cleanup_cannot_clear_intent_after_same_cursor_source_advances() {
    obsolete_terminal_cleanup_keeps_new_intent("source").await;
}

#[tokio::test]
async fn matched_terminal_cleanup_stops_only_read_intent_and_preserves_authored_cache() {
    let directory = tempfile::tempdir().unwrap();
    let store = Store::open(directory.path().join("matched.sqlite"))
        .await
        .unwrap();
    let actor = fixtures::seed(&store, "a").await;
    let draft = store
        .save_draft(LocalDraft {
            account_id: "a".into(),
            subject_id: "pull".into(),
            body: "Keep authored cache".into(),
            generation: "0".into(),
        })
        .await
        .unwrap();
    store
        .request_detail("a", "1", "pull", DetailFacet::Comments)
        .await
        .unwrap();
    let mut first = fixtures::commit(&store, &actor, DetailFacet::Comments).await;
    first.entries = vec![fixtures::entry("retained")];
    first.complete = false;
    first.whole_scope = false;
    first.next_cursor = Some("page2".into());
    store.apply_detail(first).await.unwrap();
    let lease = store
        .begin_detail("a", "1", "pull", DetailFacet::Comments)
        .await
        .unwrap();
    let before = store
        .detail(fixtures::query("a", DetailFacet::Comments))
        .await
        .unwrap();
    let revision = store.revision().await.unwrap();
    let mut request_binding = binding(&store).await;
    store
        .validate_detail_dispatch(
            "a",
            "1",
            "pull",
            DetailFacet::Comments,
            &lease,
            &request_binding,
        )
        .await
        .unwrap();
    request_binding.provider_id = "foreign-native-subject".into();
    assert_eq!(
        store
            .validate_detail_dispatch(
                "a",
                "1",
                "pull",
                DetailFacet::Comments,
                &lease,
                &request_binding
            )
            .await
            .unwrap_err()
            .code,
        ErrorCode::StaleView
    );
    store
        .stop_detail_reconciliation("a", "1", "pull", DetailFacet::Comments, &lease)
        .await
        .unwrap();
    assert!(store.pending_details().await.unwrap().is_empty());
    assert_eq!(
        store
            .detail(fixtures::query("a", DetailFacet::Comments))
            .await
            .unwrap(),
        before
    );
    assert_eq!(store.revision().await.unwrap(), revision);
    assert_eq!(store.draft("a", "pull").await.unwrap(), Some(draft));
}
