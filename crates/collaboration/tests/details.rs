use collaboration::*;
mod detail_support;
use detail_support::*;

fn only_fixture_entry(snapshot: DetailSnapshot) -> DetailEntry {
    let mut entries = snapshot.entries.into_iter();
    let entry = entries.next().expect("detail fixture contains an entry");
    assert!(
        entries.next().is_none(),
        "detail fixture contains exactly one entry"
    );
    entry
}

#[tokio::test]
async fn metadata_emptiness_is_known_only_for_complete_authorized_saved_content() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path().join("cache.sqlite")).await.unwrap();
    let actor = seed(&store, "a").await;
    assert_eq!(
        store
            .detail_evidence("a", "pull", DetailFacet::Body)
            .await
            .unwrap()
            .saved_empty,
        None
    );
    for (text, empty) in [
        (None, true),
        (Some(""), true),
        (Some("saved"), false),
        (Some("\0saved"), false),
    ] {
        let mut body = commit(&store, &actor, DetailFacet::Body).await;
        body.body = known(text);
        store.apply_detail(body).await.unwrap();
        assert_eq!(
            store
                .detail_evidence("a", "pull", DetailFacet::Body)
                .await
                .unwrap()
                .saved_empty,
            Some(empty)
        );
    }
    let mut omitted = commit(&store, &actor, DetailFacet::Body).await;
    omitted.body = DetailValue {
        state: DetailValueState::Omitted,
        text: None,
    };
    store.apply_detail(omitted).await.unwrap();
    assert_eq!(
        store
            .detail_evidence("a", "pull", DetailFacet::Body)
            .await
            .unwrap()
            .saved_empty,
        None
    );
    let mut page = commit(&store, &actor, DetailFacet::Comments).await;
    page.next_cursor = Some("next".into());
    page.complete = false;
    page.whole_scope = false;
    store.apply_detail(page).await.unwrap();
    assert_eq!(
        store
            .detail_evidence("a", "pull", DetailFacet::Comments)
            .await
            .unwrap()
            .saved_empty,
        None
    );
    store
        .apply_detail(commit(&store, &actor, DetailFacet::Reviews).await)
        .await
        .unwrap();
    assert_eq!(
        store
            .detail_evidence("a", "pull", DetailFacet::Reviews)
            .await
            .unwrap()
            .saved_empty,
        Some(true)
    );
    let mut populated = commit(&store, &actor, DetailFacet::Reviews).await;
    populated.entries = vec![entry("review")];
    store.apply_detail(populated).await.unwrap();
    assert_eq!(
        store
            .detail_evidence("a", "pull", DetailFacet::Reviews)
            .await
            .unwrap()
            .saved_empty,
        Some(false)
    );
    store
        .set_sync_status(
            "a",
            "1",
            &DetailFacet::Reviews.scope("pull"),
            SyncStatus {
                state: SyncState::Error,
                error: Some(CollaborationError::new(
                    ErrorCode::PermissionDenied,
                    "Denied",
                )),
                ..SyncStatus::default()
            },
        )
        .await
        .unwrap();
    assert_eq!(
        store
            .detail_evidence("a", "pull", DetailFacet::Reviews)
            .await
            .unwrap()
            .saved_empty,
        None
    );
    assert_eq!(
        store
            .detail_evidence("b", "pull", DetailFacet::Reviews)
            .await
            .unwrap_err()
            .code,
        ErrorCode::NotFound
    );
}

#[tokio::test]
async fn facets_and_summary_coverage_are_independent_and_null_is_authoritative() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path().join("cache.sqlite")).await.unwrap();
    let actor = seed(&store, "a").await;
    assert_eq!(
        store
            .detail(query("a", DetailFacet::Body))
            .await
            .unwrap()
            .evidence
            .availability,
        DetailAvailability::Missing
    );
    assert_eq!(
        store
            .item("a", "pull")
            .await
            .unwrap()
            .item
            .unwrap()
            .body
            .as_deref(),
        Some("summary body has no detail authority")
    );
    let mut body = commit(&store, &actor, DetailFacet::Body).await;
    body.body = known(None);
    store.apply_detail(body).await.unwrap();
    let saved = store.detail(query("a", DetailFacet::Body)).await.unwrap();
    assert_eq!(saved.body, known(None));
    assert_eq!(saved.evidence.coverage.state, CoverageState::Complete);
    assert_eq!(
        store
            .detail(query("a", DetailFacet::Comments))
            .await
            .unwrap()
            .evidence
            .availability,
        DetailAvailability::Missing
    );
    store
        .apply_detail(commit(&store, &actor, DetailFacet::Comments).await)
        .await
        .unwrap();
    let empty = store
        .detail(query("a", DetailFacet::Comments))
        .await
        .unwrap();
    assert!(empty.entries.is_empty());
    assert_eq!(empty.evidence.availability, DetailAvailability::Ready);
    assert_eq!(
        store
            .item("a", "pull")
            .await
            .unwrap()
            .item
            .unwrap()
            .body
            .as_deref(),
        Some("summary body has no detail authority")
    );
}

#[tokio::test]
async fn pagination_resume_fences_old_requests_and_local_cursors_and_keeps_membership() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path().join("cache.sqlite")).await.unwrap();
    let actor = seed(&store, "a").await;
    store
        .request_detail("a", "1", "pull", DetailFacet::Comments)
        .await
        .unwrap();
    let mut first = commit(&store, &actor, DetailFacet::Comments).await;
    first.entries = vec![entry("a"), entry("b")];
    first.next_cursor = Some("page2".into());
    first.complete = false;
    first.whole_scope = false;
    let old_request = first.clone();
    store.apply_detail(first).await.unwrap();
    let mut q = query("a", DetailFacet::Comments);
    q.limit = 1;
    let page = store.detail(q.clone()).await.unwrap();
    assert_eq!(page.entries.len(), 1);
    assert!(page.next_cursor.is_some());
    assert_eq!(page.evidence.coverage.state, CoverageState::Partial);
    let lease = store
        .begin_detail("a", "1", "pull", DetailFacet::Comments)
        .await
        .unwrap();
    assert_eq!(lease.next_cursor.as_deref(), Some("page2"));
    assert_ne!(lease.run_id, old_request.run_id);
    assert_eq!(
        store.apply_detail(old_request).await.unwrap_err().code,
        ErrorCode::StaleView
    );
    let mut last = from_lease(&actor, DetailFacet::Comments, lease);
    last.entries = vec![entry("c")];
    last.whole_scope = false;
    store.apply_detail(last).await.unwrap();
    q.cursor = page.next_cursor;
    assert_eq!(
        store.detail(q).await.unwrap_err().code,
        ErrorCode::StaleView
    );
    let saved = store
        .detail(query("a", DetailFacet::Comments))
        .await
        .unwrap();
    assert_eq!(
        saved
            .entries
            .iter()
            .map(|e| e.id.as_str())
            .collect::<Vec<_>>(),
        ["a", "b", "c"]
    );
    assert!(store.pending_details().await.unwrap().is_empty());
    assert!(
        store
            .begin_detail("a", "1", "pull", DetailFacet::Comments)
            .await
            .unwrap()
            .etag
            .is_none(),
        "a later-page validator cannot validate the whole collection"
    );
}

#[tokio::test]
async fn partial_entry_fields_preserve_saved_values_and_validation_times() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path().join("cache.sqlite")).await.unwrap();
    let actor = seed(&store, "a").await;
    let mut page = commit(&store, &actor, DetailFacet::Comments).await;
    page.entries = vec![entry("comment")];
    store.apply_detail(page).await.unwrap();
    let saved = only_fixture_entry(
        store
            .detail(query("a", DetailFacet::Comments))
            .await
            .unwrap(),
    );
    let body_validation = saved
        .field_validations
        .iter()
        .find(|v| v.field == DetailField::Body)
        .unwrap()
        .clone();
    let mut page = commit(&store, &actor, DetailFacet::Comments).await;
    let mut partial = entry("comment");
    partial.body = DetailValue {
        state: DetailValueState::Omitted,
        text: None,
    };
    partial.author = Some("renamed actor".into());
    page.entries = vec![partial];
    page.source.observed_at = "2026-10-03T00:01:00Z".into();
    store.apply_detail(page).await.unwrap();
    let saved = only_fixture_entry(
        store
            .detail(query("a", DetailFacet::Comments))
            .await
            .unwrap(),
    );
    assert_eq!(saved.body.text.as_deref(), Some("saved comment"));
    assert_eq!(saved.observed_body_state, DetailValueState::Omitted);
    assert_eq!(saved.author.as_deref(), Some("renamed actor"));
    assert_eq!(
        saved
            .field_validations
            .iter()
            .find(|v| v.field == DetailField::Body),
        Some(&body_validation)
    );
    assert_ne!(
        saved
            .field_validations
            .iter()
            .find(|v| v.field == DetailField::Author)
            .unwrap()
            .validated_at,
        body_validation.validated_at
    );
}

#[tokio::test]
async fn conditional_validation_requires_known_matching_source_and_whole_scope() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path().join("cache.sqlite")).await.unwrap();
    let actor = seed(&store, "a").await;
    let mut unknown = commit(&store, &actor, DetailFacet::Body).await;
    unknown.body = DetailValue::default();
    unknown.not_modified = true;
    assert_eq!(
        store.apply_detail(unknown).await.unwrap_err().code,
        ErrorCode::InvalidInput
    );
    store
        .apply_detail(commit(&store, &actor, DetailFacet::Body).await)
        .await
        .unwrap();
    let mut unchanged = commit(&store, &actor, DetailFacet::Body).await;
    unchanged.body = DetailValue::default();
    unchanged.not_modified = true;
    let mut wrong = unchanged.clone();
    wrong.source.source = "other endpoint".into();
    assert_eq!(
        store.apply_detail(wrong).await.unwrap_err().code,
        ErrorCode::InvalidInput
    );
    let mut wrong = unchanged.clone();
    wrong.whole_scope = false;
    assert_eq!(
        store.apply_detail(wrong).await.unwrap_err().code,
        ErrorCode::InvalidInput
    );
    let mut wrong = unchanged.clone();
    wrong.etag = Some("different".into());
    assert_eq!(
        store.apply_detail(wrong).await.unwrap_err().code,
        ErrorCode::InvalidInput
    );
    unchanged.source.observed_at = "2026-10-03T00:01:00Z".into();
    store.apply_detail(unchanged).await.unwrap();
    let current = store.detail(query("a", DetailFacet::Body)).await.unwrap();
    assert_eq!(
        current.body.text.as_deref(),
        Some("saved authoritative body")
    );
    assert_eq!(
        current.evidence.coverage.validated_at.as_deref(),
        Some("2026-10-03T00:01:00Z")
    );
}

#[tokio::test]
async fn invalid_entries_and_foreign_instance_roll_back_the_whole_transaction() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path().join("cache.sqlite")).await.unwrap();
    let actor = seed(&store, "a").await;
    let mut invalid = commit(&store, &actor, DetailFacet::Comments).await;
    invalid.entries = vec![entry("valid"), entry("")];
    let revision = store.revision().await.unwrap();
    assert_eq!(
        store.apply_detail(invalid.clone()).await.unwrap_err().code,
        ErrorCode::InvalidInput
    );
    assert_eq!(store.revision().await.unwrap(), revision);
    assert!(
        store
            .detail(query("a", DetailFacet::Comments))
            .await
            .unwrap()
            .entries
            .is_empty()
    );
    invalid.entries = vec![entry("valid")];
    invalid.instance_id = "github:https://other.example/".into();
    assert_eq!(
        store.apply_detail(invalid).await.unwrap_err().code,
        ErrorCode::StaleView
    );
}

#[tokio::test]
async fn cache_and_read_intent_survive_restart_without_new_http_or_summary_authority() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("cache.sqlite");
    let store = Store::open(&path).await.unwrap();
    let actor = seed(&store, "a").await;
    store
        .apply_detail(commit(&store, &actor, DetailFacet::Body).await)
        .await
        .unwrap();
    store
        .request_detail("a", "1", "pull", DetailFacet::Reviews)
        .await
        .unwrap();
    store.close().await;
    drop(store);
    let store = Store::open(path).await.unwrap();
    assert_eq!(
        store
            .detail(query("a", DetailFacet::Body))
            .await
            .unwrap()
            .body
            .text
            .as_deref(),
        Some("saved authoritative body")
    );
    assert_eq!(
        store.pending_details().await.unwrap()[0].facet,
        DetailFacet::Reviews
    );
    assert_eq!(
        store
            .detail(query("a", DetailFacet::Reviews))
            .await
            .unwrap()
            .evidence
            .availability,
        DetailAvailability::Missing
    );
}

#[tokio::test]
async fn older_comparable_child_observations_cannot_replace_newer_fields() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path().join("cache.sqlite")).await.unwrap();
    let actor = seed(&store, "a").await;
    let mut first = commit(&store, &actor, DetailFacet::Comments).await;
    let mut newer = entry("comment");
    newer.updated_at = Some("2026-10-03T00:02:00Z".into());
    newer.field_mask.push(DetailField::UpdatedAt);
    first.entries = vec![newer];
    store.apply_detail(first).await.unwrap();
    let mut last = commit(&store, &actor, DetailFacet::Comments).await;
    let mut older = entry("comment");
    older.body = known(Some("obsolete body"));
    older.updated_at = Some("2026-10-03T00:01:00Z".into());
    older.field_mask.push(DetailField::UpdatedAt);
    last.entries = vec![older];
    store.apply_detail(last).await.unwrap();
    let saved = store
        .detail(query("a", DetailFacet::Comments))
        .await
        .unwrap();
    assert_eq!(saved.entries[0].body.text.as_deref(), Some("saved comment"));
    assert_eq!(
        saved.entries[0].updated_at.as_deref(),
        Some("2026-10-03T00:02:00Z")
    );
}

#[tokio::test]
async fn oversized_entry_body_and_batch_read_bounds_are_explicit() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path().join("cache.sqlite")).await.unwrap();
    let actor = seed(&store, "a").await;
    let mut first = commit(&store, &actor, DetailFacet::Comments).await;
    let mut huge = entry("comment");
    huge.body = known(Some(&"x".repeat(65_537)));
    first.entries = vec![huge];
    store.apply_detail(first).await.unwrap();
    let saved = store
        .detail(query("a", DetailFacet::Comments))
        .await
        .unwrap();
    assert_eq!(saved.entries[0].body.state, DetailValueState::Oversized);
    assert!(saved.entries[0].body.text.is_none());
    assert!(
        !saved.entries[0]
            .field_validations
            .iter()
            .any(|v| v.field == DetailField::Body)
    );
    let mut too_many = commit(&store, &actor, DetailFacet::Comments).await;
    too_many.entries = (0..101).map(|id| entry(&id.to_string())).collect();
    assert_eq!(
        store.apply_detail(too_many).await.unwrap_err().code,
        ErrorCode::InvalidInput
    );
    let mut invalid = query("a", DetailFacet::Comments);
    invalid.limit = 101;
    assert_eq!(
        store.detail(invalid).await.unwrap_err().code,
        ErrorCode::InvalidInput
    );
}

#[tokio::test]
async fn omitted_oversized_and_304_observations_preserve_the_saved_values_ordering_source() {
    for intermediate in [
        DetailValueState::Omitted,
        DetailValueState::Oversized,
        DetailValueState::NotLoaded,
    ] {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open(dir.path().join("cache.sqlite")).await.unwrap();
        let actor = seed(&store, "a").await;
        let mut newer = commit(&store, &actor, DetailFacet::Body).await;
        newer.body = known(Some("newer authoritative value"));
        newer.source.provider_updated_at = Some("2026-10-03T00:02:00Z".into());
        store.apply_detail(newer).await.unwrap();
        let mut metadata = commit(&store, &actor, DetailFacet::Body).await;
        metadata.body = DetailValue {
            state: intermediate,
            text: None,
        };
        metadata.source.provider_updated_at = None;
        if intermediate == DetailValueState::NotLoaded {
            metadata.not_modified = true;
        }
        store.apply_detail(metadata).await.unwrap();
        let retained = store.detail(query("a", DetailFacet::Body)).await.unwrap();
        assert_eq!(
            retained.body.text.as_deref(),
            Some("newer authoritative value")
        );
        assert_eq!(
            retained
                .evidence
                .value_source
                .unwrap()
                .provider_updated_at
                .as_deref(),
            Some("2026-10-03T00:02:00Z")
        );
        let mut older = commit(&store, &actor, DetailFacet::Body).await;
        older.body = known(Some("obsolete value"));
        older.source.provider_updated_at = Some("2026-10-03T00:01:00Z".into());
        assert_eq!(
            store.apply_detail(older).await.unwrap_err().code,
            ErrorCode::StaleView
        );
        assert_eq!(
            store
                .detail(query("a", DetailFacet::Body))
                .await
                .unwrap()
                .body
                .text
                .as_deref(),
            Some("newer authoritative value")
        );
    }
}
