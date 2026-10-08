use collaboration::*;
mod detail_support;
use detail_support::*;

#[tokio::test]
async fn new_merge_base_field_requires_observation_after_legacy_snapshot() {
    use sqlx::Connection;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("cache.sqlite");
    let store = Store::open(&path).await.unwrap();
    let account = seed(&store, "merge-base").await;
    let page = observation(
        &store,
        &account,
        ResourceMetadataValues::default(),
        vec![],
        None,
    )
    .await;
    store.apply_detail(page).await.unwrap();
    let mut legacy = saved(&store, &account).await;
    legacy
        .fields
        .retain(|entry| entry.field != MetadataField::MergeBase);
    let mut json = serde_json::to_value(legacy).unwrap();
    json["values"]
        .as_object_mut()
        .unwrap()
        .remove("merge_base_oid");
    let mut connection = sqlx::SqliteConnection::connect_with(
        &sqlx::sqlite::SqliteConnectOptions::new().filename(&path),
    )
    .await
    .unwrap();
    sqlx::query("UPDATE detail_resource_metadata SET metadata_json=? WHERE account_id=? AND subject_id='pull'")
        .bind(json.to_string()).bind(&account.id).execute(&mut connection).await.unwrap();
    connection.close().await.unwrap();

    let mut unchanged = observation(
        &store,
        &account,
        ResourceMetadataValues::default(),
        vec![],
        None,
    )
    .await;
    unchanged.not_modified = true;
    unchanged.metadata = None;
    unchanged.body = DetailValue::default();
    store.apply_detail(unchanged).await.unwrap();
    let snapshot = saved(&store, &account).await;
    assert_eq!(snapshot.values.merge_base_oid, None);
    assert_eq!(
        field(&snapshot, MetadataField::MergeBase).saved_state,
        DetailValueState::NotLoaded
    );
    assert_eq!(
        field(&snapshot, MetadataField::MergeBase).validated_at,
        None
    );

    let oid = "c".repeat(40);
    let page = observation(
        &store,
        &account,
        ResourceMetadataValues {
            merge_base_oid: Some(oid.clone()),
            ..Default::default()
        },
        vec![(MetadataField::MergeBase, DetailValueState::Known)],
        None,
    )
    .await;
    store.apply_detail(page).await.unwrap();
    assert_eq!(
        saved(&store, &account).await.values.merge_base_oid,
        Some(oid)
    );
    let page = observation(
        &store,
        &account,
        ResourceMetadataValues {
            merge_base_oid: Some("invalid".into()),
            ..Default::default()
        },
        vec![(MetadataField::MergeBase, DetailValueState::Known)],
        None,
    )
    .await;
    assert!(store.apply_detail(page).await.is_err());
    store.close().await.unwrap();
}

async fn observation(
    store: &Store,
    a: &RemoteAccount,
    values: ResourceMetadataValues,
    fields: Vec<(MetadataField, DetailValueState)>,
    time: Option<&str>,
) -> DetailCommit {
    let mut page = commit(store, a, DetailFacet::Body).await;
    let subject = store.detail_subject(&a.id, "pull").await.unwrap();
    page.subject_binding = Some(DetailSubjectBinding {
        repository_id: "repo".into(),
        repository_provider_id: "1".into(),
        provider_id: subject.provider_id,
        number: subject.number,
        kind: subject.kind,
        head_oid: subject.head_oid,
    });
    page.source.observed_at = "2099-01-01T00:00:00Z".into();
    page.source.provider_updated_at = time.map(String::from);
    page.metadata = Some(ResourceMetadataObservation {
        kind: RemoteItemKind::PullRequest,
        values,
        fields: fields
            .into_iter()
            .map(|(field, state)| MetadataObservedField { field, state })
            .collect(),
        source: MetadataSource {
            source: page.source.source.clone(),
            adapter_version: page.source.adapter_version,
            provider_updated_at: page.source.provider_updated_at.clone(),
            observed_at: page.source.observed_at.clone(),
        },
    });
    page
}
async fn saved(store: &Store, a: &RemoteAccount) -> ResourceMetadataSnapshot {
    store
        .detail(query(&a.id, DetailFacet::Body))
        .await
        .unwrap()
        .metadata
        .unwrap()
}
fn field(snapshot: &ResourceMetadataSnapshot, field: MetadataField) -> &MetadataFieldEvidence {
    snapshot.fields.iter().find(|f| f.field == field).unwrap()
}

#[tokio::test]
async fn known_null_body_preserves_rich_metadata_restart_and_partition_access() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("cache.sqlite");
    let store = Store::open(&path).await.unwrap();
    let a = seed(&store, "a").await;
    let b = seed(&store, "b").await;
    let mut page = observation(
        &store,
        &a,
        ResourceMetadataValues {
            title: Some("Authoritative title".into()),
            state: Some("future-state".into()),
            labels: vec![],
            ..Default::default()
        },
        vec![
            (MetadataField::Title, DetailValueState::Known),
            (MetadataField::State, DetailValueState::Known),
            (MetadataField::Labels, DetailValueState::Known),
        ],
        Some("2026-10-02T00:00:00Z"),
    )
    .await;
    page.body = known(None);
    store.apply_detail(page).await.unwrap();
    let local = store.detail(query("a", DetailFacet::Body)).await.unwrap();
    assert_eq!(local.evidence.saved_empty, Some(true));
    assert_eq!(
        local.metadata.unwrap().values.title.as_deref(),
        Some("Authoritative title")
    );
    assert!(
        store
            .detail(query(&b.id, DetailFacet::Body))
            .await
            .unwrap()
            .metadata
            .is_none()
    );
    assert!(store.pending_details().await.unwrap().is_empty());
    store
        .save_draft(LocalDraft {
            account_id: a.id.clone(),
            subject_id: "pull".into(),
            body: "Authored private text".into(),
            generation: "0".into(),
        })
        .await
        .unwrap();
    store.close().await.unwrap();
    drop(store);
    let store = Store::open(&path).await.unwrap();
    assert_eq!(
        saved(&store, &a).await.values.state.as_deref(),
        Some("future-state")
    );
    store
        .set_sync_status(
            "a",
            "1",
            &DetailFacet::Body.scope("pull"),
            SyncStatus {
                state: SyncState::Error,
                error: Some(CollaborationError {
                    code: ErrorCode::PermissionDenied,
                    message: "denied".into(),
                    retry_after_seconds: None,
                }),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    let denied = store.detail(query("a", DetailFacet::Body)).await.unwrap();
    assert!(denied.metadata.is_none());
    assert_eq!(denied.body, DetailValue::default());
    assert_eq!(
        store.draft("a", "pull").await.unwrap().unwrap().body,
        "Authored private text"
    );
}

#[tokio::test]
async fn omitted_fields_keep_their_saved_clock_and_are_not_freshened_by_304() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path().join("cache.sqlite")).await.unwrap();
    let a = seed(&store, "a").await;
    let page = observation(
        &store,
        &a,
        ResourceMetadataValues {
            title: Some("first".into()),
            labels: vec![DetailLabel {
                provider_id: None,
                name: "kept".into(),
                color: None,
            }],
            ..Default::default()
        },
        vec![
            (MetadataField::Title, DetailValueState::Known),
            (MetadataField::Labels, DetailValueState::Known),
        ],
        Some("2026-10-02T00:00:00Z"),
    )
    .await;
    store.apply_detail(page).await.unwrap();
    let first = saved(&store, &a).await;
    let label_clock = field(&first, MetadataField::Labels).clone();
    let mut page = observation(
        &store,
        &a,
        ResourceMetadataValues {
            title: Some("new title".into()),
            ..Default::default()
        },
        vec![
            (MetadataField::Title, DetailValueState::Known),
            (MetadataField::Labels, DetailValueState::Omitted),
        ],
        Some("2026-10-03T00:00:00Z"),
    )
    .await;
    page.source.observed_at = "2099-01-02T00:00:00Z".into();
    page.metadata.as_mut().unwrap().source.observed_at = page.source.observed_at.clone();
    store.apply_detail(page).await.unwrap();
    let mut page = commit(&store, &a, DetailFacet::Body).await;
    page.source.observed_at = "2099-01-03T00:00:00Z".into();
    page.body = DetailValue::default();
    page.not_modified = true;
    store.apply_detail(page).await.unwrap();
    let current = saved(&store, &a).await;
    let labels = field(&current, MetadataField::Labels);
    assert_eq!(labels.validated_at, label_clock.validated_at);
    assert_eq!(labels.source, label_clock.source);
    assert_eq!(labels.observed_state, DetailValueState::Omitted);
    assert_eq!(current.values.labels[0].name, "kept");
    assert_eq!(
        field(&current, MetadataField::Title)
            .validated_at
            .as_deref(),
        Some("2099-01-03T00:00:00Z")
    );
}

#[tokio::test]
async fn body_only_representation_omits_metadata_before_304_and_restart() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("cache.sqlite");
    let store = Store::open(&path).await.unwrap();
    let a = seed(&store, "a").await;
    let page = observation(
        &store,
        &a,
        ResourceMetadataValues {
            labels: vec![DetailLabel {
                provider_id: Some("42".into()),
                name: "retained label".into(),
                color: None,
            }],
            ..Default::default()
        },
        vec![(MetadataField::Labels, DetailValueState::Known)],
        Some("2026-10-02T00:00:00Z"),
    )
    .await;
    store.apply_detail(page).await.unwrap();
    let old = field(&saved(&store, &a).await, MetadataField::Labels).clone();
    let mut page = commit(&store, &a, DetailFacet::Body).await;
    page.body = known(Some("body from a metadata-omitting adapter"));
    page.source.observed_at = "2099-01-02T00:00:00Z".into();
    page.source.provider_updated_at = Some("2026-10-03T00:00:00Z".into());
    page.etag = Some("\"body-only\"".into());
    store.apply_detail(page).await.unwrap();
    let omitted = saved(&store, &a).await;
    assert!(
        omitted
            .fields
            .iter()
            .all(|f| f.observed_state == DetailValueState::Omitted)
    );
    assert_eq!(field(&omitted, MetadataField::Labels).source, old.source);
    let mut page = commit(&store, &a, DetailFacet::Body).await;
    page.source.observed_at = "2099-01-03T00:00:00Z".into();
    page.body = DetailValue::default();
    page.not_modified = true;
    page.etag = Some("\"body-only\"".into());
    store.apply_detail(page).await.unwrap();
    store.close().await.unwrap();
    drop(store);
    let store = Store::open(&path).await.unwrap();
    let current = store.detail(query(&a.id, DetailFacet::Body)).await.unwrap();
    assert_eq!(
        current.body.text.as_deref(),
        Some("body from a metadata-omitting adapter")
    );
    let metadata = current.metadata.unwrap();
    assert_eq!(metadata.values.labels[0].name, "retained label");
    let labels = field(&metadata, MetadataField::Labels);
    assert_eq!(labels.saved_state, DetailValueState::Known);
    assert_eq!(labels.observed_state, DetailValueState::Omitted);
    assert_eq!(labels.validated_at, old.validated_at);
    assert_eq!(labels.stale_at, old.stale_at);
    assert_eq!(labels.source, old.source);
}

#[tokio::test]
async fn timestamp_less_observations_cannot_erase_metadata_or_body_ordering_barriers() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path().join("cache.sqlite")).await.unwrap();
    let a = seed(&store, "a").await;
    let page = observation(
        &store,
        &a,
        ResourceMetadataValues {
            title: Some("new".into()),
            ..Default::default()
        },
        vec![(MetadataField::Title, DetailValueState::Known)],
        Some("2026-10-02T00:00:00Z"),
    )
    .await;
    store.apply_detail(page).await.unwrap();
    let page = observation(
        &store,
        &a,
        ResourceMetadataValues {
            title: Some("still new".into()),
            ..Default::default()
        },
        vec![(MetadataField::Title, DetailValueState::Known)],
        None,
    )
    .await;
    store.apply_detail(page).await.unwrap();
    let before = store.detail(query("a", DetailFacet::Body)).await.unwrap();
    assert_eq!(
        before
            .evidence
            .value_source
            .unwrap()
            .provider_updated_at
            .as_deref(),
        Some("2026-10-02T00:00:00Z")
    );
    assert_eq!(
        field(before.metadata.as_ref().unwrap(), MetadataField::Title)
            .source
            .as_ref()
            .unwrap()
            .provider_updated_at
            .as_deref(),
        Some("2026-10-02T00:00:00Z")
    );
    let mut page = observation(
        &store,
        &a,
        ResourceMetadataValues {
            title: Some("old".into()),
            ..Default::default()
        },
        vec![(MetadataField::Title, DetailValueState::Known)],
        Some("2026-10-01T00:00:00Z"),
    )
    .await;
    page.body = known(Some("old body"));
    assert_eq!(
        store.apply_detail(page).await.unwrap_err().code,
        ErrorCode::Provider
    );
    assert_eq!(
        saved(&store, &a).await.values.title.as_deref(),
        Some("still new")
    );
}

#[tokio::test]
async fn retained_fields_cannot_grow_the_saved_metadata_past_its_budget() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path().join("cache.sqlite")).await.unwrap();
    let a = seed(&store, "a").await;
    let labels = (0..100)
        .map(|_| DetailLabel {
            provider_id: None,
            name: "L".repeat(1024),
            color: None,
        })
        .collect();
    let page = observation(
        &store,
        &a,
        ResourceMetadataValues {
            labels,
            ..Default::default()
        },
        vec![(MetadataField::Labels, DetailValueState::Known)],
        Some("2026-10-01T00:00:00Z"),
    )
    .await;
    store.apply_detail(page).await.unwrap();
    let assignees = (0..100)
        .map(|n| DetailActor {
            provider_id: (n + 1).to_string(),
            login: "A".repeat(255),
            web_url: Some(format!("https://github.com/{}", "x".repeat(2000))),
        })
        .collect::<Vec<_>>();
    let mut page = observation(
        &store,
        &a,
        ResourceMetadataValues {
            assignees: assignees.clone(),
            ..Default::default()
        },
        vec![
            (MetadataField::Labels, DetailValueState::Omitted),
            (MetadataField::Assignees, DetailValueState::Known),
        ],
        Some("2026-10-02T00:00:00Z"),
    )
    .await;
    page.body = known(Some("Valid body remains committed"));
    store.apply_detail(page).await.unwrap();
    let current = saved(&store, &a).await;
    assert_eq!(current.values.labels.len(), 100);
    assert!(current.values.assignees.is_empty());
    assert_eq!(
        field(&current, MetadataField::Assignees).observed_state,
        DetailValueState::Oversized
    );
    assert!(serde_json::to_vec(&current.values).unwrap().len() <= 262144);
    assert_eq!(
        store
            .detail(query("a", DetailFacet::Body))
            .await
            .unwrap()
            .body
            .text
            .as_deref(),
        Some("Valid body remains committed")
    );
    let page = observation(
        &store,
        &a,
        ResourceMetadataValues {
            assignees,
            ..Default::default()
        },
        vec![
            (MetadataField::Labels, DetailValueState::Known),
            (MetadataField::Assignees, DetailValueState::Known),
        ],
        Some("2026-10-03T00:00:00Z"),
    )
    .await;
    store.apply_detail(page).await.unwrap();
    let current = saved(&store, &a).await;
    assert!(current.values.labels.is_empty());
    assert_eq!(current.values.assignees.len(), 100);
}

async fn new_head(store: &Store, a: &RemoteAccount, head: &str) {
    let mut item = store.detail_subject(&a.id, "pull").await.unwrap();
    item.head_oid = Some(head.into());
    item.updated_at = "2026-10-03T00:00:00Z".into();
    let scope = "repo:repo:pull_request";
    let run_id = store
        .begin_sync(&a.id, &a.authorization_epoch, scope)
        .await
        .unwrap();
    store
        .apply_page(PageCommit {
            account_id: a.id.clone(),
            authorization_epoch: a.authorization_epoch.clone(),
            scope: scope.into(),
            run_id,
            repositories: vec![],
            items: vec![item],
            endpoint_aliases: vec![],
            next_cursor: None,
            etag: None,
            last_modified: None,
            not_modified: false,
            complete: true,
            observed_at: "2026-10-03T00:00:00Z".into(),
        })
        .await
        .unwrap();
}

#[tokio::test]
async fn list_head_changes_dirty_even_body_only_cache_and_reject_dispatched_response() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path().join("cache.sqlite")).await.unwrap();
    let a = seed(&store, "a").await;
    let mut first = commit(&store, &a, DetailFacet::Body).await;
    first.source.observed_at = "2099-01-01T00:00:00Z".into();
    store.apply_detail(first).await.unwrap();
    let pending = commit(&store, &a, DetailFacet::Body).await;
    new_head(&store, &a, "new-head").await;
    assert_eq!(
        store.apply_detail(pending).await.unwrap_err().code,
        ErrorCode::StaleView
    );
    let local = store.detail(query("a", DetailFacet::Body)).await.unwrap();
    assert_eq!(local.body.text.as_deref(), Some("saved authoritative body"));
    assert_eq!(local.evidence.freshness, DetailFreshness::Stale);
    assert_eq!(local.evidence.sync.state, SyncState::Idle);
    assert!(
        store
            .begin_detail("a", "1", "pull", DetailFacet::Body)
            .await
            .unwrap()
            .etag
            .is_none()
    );
    assert!(store.pending_details().await.unwrap().is_empty());
}

#[tokio::test]
async fn bad_binding_rolls_back_body_and_metadata_together() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path().join("cache.sqlite")).await.unwrap();
    let a = seed(&store, "a").await;
    let mut page = observation(
        &store,
        &a,
        ResourceMetadataValues {
            title: Some("must not publish".into()),
            ..Default::default()
        },
        vec![(MetadataField::Title, DetailValueState::Known)],
        None,
    )
    .await;
    page.subject_binding
        .as_mut()
        .unwrap()
        .repository_provider_id = "different".into();
    assert_eq!(
        store.apply_detail(page).await.unwrap_err().code,
        ErrorCode::StaleView
    );
    let local = store.detail(query("a", DetailFacet::Body)).await.unwrap();
    assert!(local.metadata.is_none());
    assert_eq!(local.body, DetailValue::default());
}

#[tokio::test]
async fn metadata_clock_is_preserved_when_the_body_does_not_supply_an_ordering_barrier() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path().join("cache.sqlite")).await.unwrap();
    let a = seed(&store, "a").await;
    let mut first = commit(&store, &a, DetailFacet::Body).await;
    first.source.provider_updated_at = Some("2026-10-01T00:00:00Z".into());
    store.apply_detail(first).await.unwrap();
    for timestamp in [Some("2026-10-02T00:00:00Z"), None] {
        let mut page = observation(
            &store,
            &a,
            ResourceMetadataValues {
                title: Some("new".into()),
                ..Default::default()
            },
            vec![(MetadataField::Title, DetailValueState::Known)],
            timestamp,
        )
        .await;
        page.body = DetailValue {
            state: DetailValueState::Omitted,
            text: None,
        };
        store.apply_detail(page).await.unwrap();
    }
    let page = observation(
        &store,
        &a,
        ResourceMetadataValues {
            title: Some("old".into()),
            ..Default::default()
        },
        vec![(MetadataField::Title, DetailValueState::Known)],
        Some("2026-10-01T00:00:00Z"),
    )
    .await;
    assert_eq!(
        store.apply_detail(page).await.unwrap_err().code,
        ErrorCode::Provider
    );
    assert_eq!(saved(&store, &a).await.values.title.as_deref(), Some("new"));
}

#[tokio::test]
async fn head_hints_dirty_older_head_fields_without_staling_newer_omitting_fields() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path().join("cache.sqlite")).await.unwrap();
    let a = seed(&store, "a").await;
    let page = observation(
        &store,
        &a,
        ResourceMetadataValues {
            head: Some(DetailBranch {
                name: "feature".into(),
                oid: "old-head".into(),
                repository: None,
            }),
            ..Default::default()
        },
        vec![(MetadataField::Head, DetailValueState::Known)],
        Some("2026-10-02T00:00:00Z"),
    )
    .await;
    store.apply_detail(page).await.unwrap();
    let page = observation(
        &store,
        &a,
        ResourceMetadataValues {
            title: Some("newer title".into()),
            ..Default::default()
        },
        vec![
            (MetadataField::Head, DetailValueState::Omitted),
            (MetadataField::Title, DetailValueState::Known),
        ],
        Some("2026-10-04T00:00:00Z"),
    )
    .await;
    store.apply_detail(page).await.unwrap();
    new_head(&store, &a, "changed-head").await;
    let current = saved(&store, &a).await;
    assert!(
        chrono::DateTime::parse_from_rfc3339(
            field(&current, MetadataField::Head)
                .stale_at
                .as_ref()
                .unwrap()
        )
        .unwrap()
            <= chrono::Utc::now()
    );
    assert_eq!(
        field(&current, MetadataField::Title).stale_at.as_deref(),
        Some("2099-01-01T00:01:00+00:00")
    );
    assert_eq!(
        store
            .detail(query("a", DetailFacet::Body))
            .await
            .unwrap()
            .evidence
            .freshness,
        DetailFreshness::Fresh
    );
    assert_eq!(current.values.head.unwrap().oid, "old-head");
}

#[tokio::test]
async fn near_budget_metadata_stays_bounded_when_304_validation_precision_grows() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path().join("cache.sqlite")).await.unwrap();
    let a = seed(&store, "a").await;
    let assignees = (0..100)
        .map(|n| DetailActor {
            provider_id: (n + 1).to_string(),
            login: "A".repeat(255),
            web_url: Some(format!("https://github.com/{}", "x".repeat(2000))),
        })
        .collect();
    let mut page = observation(
        &store,
        &a,
        ResourceMetadataValues {
            title: Some("T".repeat(14000)),
            assignees,
            ..Default::default()
        },
        MetadataField::COMMON
            .into_iter()
            .chain(MetadataField::PULL)
            .map(|field| (field, DetailValueState::Known))
            .collect(),
        Some("2026-10-01T00:00:00Z"),
    )
    .await;
    page.source.observed_at = "2099-01-01T00:00:00Z".into();
    page.metadata.as_mut().unwrap().source.observed_at = page.source.observed_at.clone();
    store.apply_detail(page).await.unwrap();
    let before = saved(&store, &a).await;
    let size = serde_json::to_vec(&before).unwrap().len();
    assert!(size > 240000);
    assert_eq!(before.values.assignees.len(), 100);
    // Build a valid whole-field candidate that the former 64-byte reserve
    // admitted, but whose legitimate 304 timestamp growth crosses the ceiling.
    let mut tight = before.clone();
    let target = 262144 - 96;
    while serde_json::to_vec(&tight).unwrap().len() < target {
        tight.values.labels.push(DetailLabel {
            provider_id: None,
            name: String::new(),
            color: None,
        });
        let empty_size = serde_json::to_vec(&tight).unwrap().len();
        if empty_size > target {
            tight.values.labels.pop();
            let gap = target - serde_json::to_vec(&tight).unwrap().len();
            tight
                .values
                .title
                .as_mut()
                .unwrap()
                .push_str(&"T".repeat(gap));
            break;
        }
        tight.values.labels.last_mut().unwrap().name = "L".repeat((target - empty_size).min(1024));
    }
    assert_eq!(serde_json::to_vec(&tight).unwrap().len(), target);
    assert!(serde_json::to_vec(&tight.values).unwrap().len() <= 262144);
    let later = "2099-01-02T00:00:00.123456789+00:00";
    let mut unbounded_304 = tight.clone();
    for evidence in &mut unbounded_304.fields {
        evidence.validated_at = Some(later.into());
        evidence.stale_at = Some("2099-01-02T00:01:00.123456789+00:00".into());
        evidence.source.as_mut().unwrap().observed_at = later.into();
    }
    assert!(serde_json::to_vec(&unbounded_304).unwrap().len() > 262144);
    let page = observation(
        &store,
        &a,
        tight.values,
        MetadataField::COMMON
            .into_iter()
            .chain(MetadataField::PULL)
            .map(|field| (field, DetailValueState::Known))
            .collect(),
        Some("2026-10-01T00:00:00Z"),
    )
    .await;
    store.apply_detail(page).await.unwrap();
    let bounded = saved(&store, &a).await;
    assert_eq!(
        field(&bounded, MetadataField::Labels).observed_state,
        DetailValueState::Oversized
    );
    assert_eq!(bounded.values.labels, before.values.labels);
    assert_eq!(bounded.values.assignees, before.values.assignees);
    let mut page = commit(&store, &a, DetailFacet::Body).await;
    page.body = DetailValue::default();
    page.not_modified = true;
    page.source.observed_at = later.into();
    store.apply_detail(page).await.unwrap();
    let after = saved(&store, &a).await;
    let after_size = serde_json::to_vec(&after).unwrap().len();
    assert!(after_size > size);
    assert!(after_size <= 262144);
    assert_eq!(after.values, bounded.values);
}
