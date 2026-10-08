//! Independent reconciliation qualification. Synthetic provider observations and
//! public Store APIs only; no provider HTTP, credentials or migration writes.
use collaboration::*;
use sqlx::Connection as _;
mod detail_support;
use detail_support::*;

const EARLY: &str = "2026-10-03T01:00:00Z";
const LATER: &str = "2026-10-03T02:00:00Z";
const LATEST: &str = "2026-10-03T03:00:00Z";
// Engine validation time is intentionally separate from provider ordering time.
const VALIDATED: &str = "2099-01-01T00:00:00Z";
const REVALIDATED: &str = "2099-01-01T00:01:00Z";
const HEAD_A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const HEAD_B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
const BASE: &str = "cccccccccccccccccccccccccccccccccccccccc";
const COLLECTIONS: [DetailFacet; 2] = [DetailFacet::Comments, DetailFacet::Reviews];

async fn fixture() -> (tempfile::TempDir, Store, RemoteAccount) {
    let directory = tempfile::tempdir().unwrap();
    let store = Store::open(directory.path().join("facet.sqlite"))
        .await
        .unwrap();
    let actor = seed(&store, "a").await;
    (directory, store, actor)
}
async fn authored(store: &Store) -> LocalDraft {
    store
        .save_draft(LocalDraft {
            account_id: "a".into(),
            subject_id: "pull".into(),
            body: "Private draft must survive facet reconciliation".into(),
            generation: "0".into(),
        })
        .await
        .unwrap()
}
fn observed_entry(id: &str, text: &str, at: Option<&str>) -> DetailEntry {
    let mut observed = entry(id);
    observed.body = known(Some(text));
    observed.updated_at = at.map(str::to_owned);
    observed.field_mask.push(DetailField::UpdatedAt);
    observed
}
async fn read(store: &Store, facet: DetailFacet) -> DetailSnapshot {
    store.detail(query("a", facet)).await.unwrap()
}
fn sole_entry(snapshot: &DetailSnapshot) -> &DetailEntry {
    let mut entries = snapshot.entries.iter();
    let only = entries.next().expect("one saved synthetic child");
    assert!(entries.next().is_none(), "one saved synthetic child");
    only
}
async fn parent(store: &Store, actor: &RemoteAccount, state: &str, head: Option<&str>, at: &str) {
    let mut item = store.item("a", "pull").await.unwrap().item.unwrap();
    item.state = state.into();
    item.head_oid = head.map(str::to_owned);
    item.updated_at = at.into();
    let run_id = store
        .begin_sync("a", &actor.authorization_epoch, "repo:repo:pull_request")
        .await
        .unwrap();
    store
        .apply_page(PageCommit {
            account_id: "a".into(),
            authorization_epoch: actor.authorization_epoch.clone(),
            scope: "repo:repo:pull_request".into(),
            run_id,
            repositories: vec![],
            items: vec![item],
            endpoint_aliases: vec![],
            next_cursor: None,
            etag: None,
            last_modified: None,
            not_modified: false,
            complete: true,
            observed_at: VALIDATED.into(),
        })
        .await
        .unwrap();
}
async fn binding(store: &Store) -> DetailSubjectBinding {
    let item = store.detail_subject("a", "pull").await.unwrap();
    let repository = store.repository("a", "repo").await.unwrap();
    DetailSubjectBinding {
        repository_id: repository.id,
        repository_provider_id: repository.provider_id,
        provider_id: item.provider_id,
        number: item.number,
        kind: item.kind,
        head_oid: item.head_oid,
    }
}

async fn detail_head(store: &Store, actor: &RemoteAccount, head: Option<&str>, at: &str) {
    detail_head_source(store, actor, head, "1", at).await;
}

async fn detail_head_source(
    store: &Store,
    actor: &RemoteAccount,
    head: Option<&str>,
    source_repository_provider_id: &str,
    at: &str,
) {
    let mut description = commit(store, actor, DetailFacet::Body).await;
    description.subject_binding = Some(binding(store).await);
    description.source.provider_updated_at = Some(at.into());
    description.source.observed_at = VALIDATED.into();
    description.metadata = Some(ResourceMetadataObservation {
        kind: RemoteItemKind::PullRequest,
        values: ResourceMetadataValues {
            base: Some(DetailBranch {
                name: "main".into(),
                oid: BASE.into(),
                repository: Some(DetailRepositoryRef {
                    provider_id: "1".into(),
                    full_name: "owner/project".into(),
                    web_url: None,
                }),
            }),
            head: head.map(|oid| DetailBranch {
                name: "feature".into(),
                oid: oid.into(),
                repository: Some(DetailRepositoryRef {
                    provider_id: source_repository_provider_id.into(),
                    full_name: "owner/project".into(),
                    web_url: None,
                }),
            }),
            ..Default::default()
        },
        fields: vec![
            MetadataObservedField {
                field: MetadataField::Base,
                state: DetailValueState::Known,
            },
            MetadataObservedField {
                field: MetadataField::Head,
                state: DetailValueState::Known,
            },
        ],
        source: MetadataSource {
            source: description.source.source.clone(),
            adapter_version: description.source.adapter_version,
            provider_updated_at: description.source.provider_updated_at.clone(),
            observed_at: description.source.observed_at.clone(),
        },
    });
    store.apply_detail(description).await.unwrap();
}

fn current_check(head: &str) -> DetailEntry {
    DetailEntry {
        id: "fixture-check-run:1".into(),
        provider_id: "check-run:1".into(),
        author: None,
        title: None,
        state: None,
        body: DetailValue::default(),
        observed_body_state: DetailValueState::NotLoaded,
        updated_at: None,
        head_oid: Some(head.into()),
        native: Some(NativeDetailPayload::CheckV1(CheckV1 {
            kind: CheckKind::CheckRun,
            name: "fixture check".into(),
            state: CheckStateV1::CheckRun {
                status: "completed".into(),
                conclusion: Some("success".into()),
            },
            description: DetailValue::default(),
            producer: Some("fixture".into()),
            started_at: None,
            completed_at: None,
            updated_at: None,
            allow_failure: None,
        })),
        field_mask: vec![DetailField::Check, DetailField::HeadOid],
        field_validations: vec![],
    }
}

fn historical_entry(head: &str) -> DetailEntry {
    let mut value = entry("check-history");
    value.head_oid = Some(head.into());
    value.field_mask.push(DetailField::HeadOid);
    value
}

async fn bind_checks(store: &Store, page: &mut DetailCommit, head: &str) {
    page.reconciliation = DetailReconciliation {
        enumeration: DetailEnumeration::FullEnumeration,
        head_scope: DetailHeadScope::CurrentHead,
    };
    page.subject_binding = Some(binding(store).await);
    page.source.field_mask = vec![DetailField::Check, DetailField::HeadOid];
    let body = read(store, DetailFacet::Body).await;
    let source_repository_provider_id = body
        .metadata
        .as_ref()
        .and_then(|metadata| metadata.values.head.as_ref())
        .and_then(|head| head.repository.as_ref())
        .map(|repository| repository.provider_id.clone())
        // Negative fixtures without an authoritative head repository still
        // need a well-formed captured page; Store admission rejects them.
        .unwrap_or_else(|| "1".into());
    page.check_context = Some(CheckContext {
        head_oid: head.into(),
        source_repository_provider_id,
        metadata_facet_revision: body.evidence.facet_revision.unwrap(),
    });
}

#[tokio::test]
async fn collection_304_without_timestamp_preserves_saved_comparable_ordering_boundary() {
    for facet in COLLECTIONS {
        let (_directory, store, actor) = fixture().await;
        let draft = authored(&store).await;
        let mut newer = commit(&store, &actor, facet).await;
        newer.source.provider_updated_at = Some(LATER.into());
        newer.source.observed_at = VALIDATED.into();
        newer.entries = vec![entry("child")];
        store.apply_detail(newer).await.unwrap();

        let mut unchanged = commit(&store, &actor, facet).await;
        unchanged.not_modified = true;
        unchanged.source.provider_updated_at = None;
        unchanged.source.observed_at = REVALIDATED.into();
        store.apply_detail(unchanged).await.unwrap();
        let saved = read(&store, facet).await;
        assert_eq!(
            saved
                .evidence
                .value_source
                .as_ref()
                .unwrap()
                .provider_updated_at
                .as_deref(),
            Some(LATER)
        );

        let mut obsolete = commit(&store, &actor, facet).await;
        obsolete.source.provider_updated_at = Some(EARLY.into());
        obsolete.source.observed_at = REVALIDATED.into();
        let mut old_entry = entry("child");
        old_entry.body = known(Some("older provider response"));
        obsolete.entries = vec![old_entry];
        let before = read(&store, facet).await;
        let revision = store.revision().await.unwrap();
        let result = store.apply_detail(obsolete).await;
        assert_eq!(
            result.unwrap_err().code,
            ErrorCode::Provider,
            "{facet:?}: 304 validates saved truth without deleting its ordering barrier"
        );
        assert_eq!(store.revision().await.unwrap(), revision);
        assert_eq!(read(&store, facet).await, before);
        assert_eq!(store.draft("a", "pull").await.unwrap(), Some(draft));
    }
}

#[tokio::test]
async fn child_null_timestamp_cannot_erase_retained_body_ordering_evidence() {
    for facet in COLLECTIONS {
        let (_directory, store, actor) = fixture().await;
        let draft = authored(&store).await;
        let mut initial = commit(&store, &actor, facet).await;
        initial.source.observed_at = VALIDATED.into();
        initial.entries = vec![observed_entry("child", "newer saved body", Some(LATER))];
        store.apply_detail(initial).await.unwrap();
        let saved_body = sole_entry(&read(&store, facet).await).body.clone();

        let mut partial = commit(&store, &actor, facet).await;
        partial.source.observed_at = REVALIDATED.into();
        let mut metadata = observed_entry("child", "unobserved payload", None);
        metadata.field_mask = vec![DetailField::UpdatedAt, DetailField::Author];
        metadata.author = Some("renamed actor".into());
        partial.entries = vec![metadata];
        store.apply_detail(partial).await.unwrap();
        assert_eq!(sole_entry(&read(&store, facet).await).body, saved_body);

        let mut older = commit(&store, &actor, facet).await;
        older.source.observed_at = REVALIDATED.into();
        older.entries = vec![observed_entry("child", "obsolete body", Some(EARLY))];
        store.apply_detail(older).await.unwrap();
        let saved = read(&store, facet).await;
        assert_eq!(
            sole_entry(&saved).body,
            saved_body,
            "{facet:?}: updating nullable timestamp metadata must not authorize stale body rollback"
        );
        assert_eq!(store.draft("a", "pull").await.unwrap(), Some(draft));
    }
}

#[tokio::test]
async fn continuation_representation_drift_cannot_finish_or_prune_a_different_traversal() {
    for drift in ["endpoint", "version", "mask"] {
        let (_directory, store, actor) = fixture().await;
        let draft = authored(&store).await;
        let mut saved = commit(&store, &actor, DetailFacet::Comments).await;
        saved.entries = vec![entry("retained"), entry("seen")];
        store.apply_detail(saved).await.unwrap();

        let mut first = commit(&store, &actor, DetailFacet::Comments).await;
        first.entries = vec![entry("seen")];
        first.complete = false;
        first.whole_scope = false;
        first.next_cursor = Some("captured-continuation".into());
        store.apply_detail(first).await.unwrap();
        let lease = store
            .begin_detail("a", "1", "pull", DetailFacet::Comments)
            .await
            .unwrap();
        assert_eq!(lease.next_cursor.as_deref(), Some("captured-continuation"));
        let mut final_page = from_lease(&actor, DetailFacet::Comments, lease);
        final_page.whole_scope = false;
        final_page.entries = vec![entry("next")];
        match drift {
            "endpoint" => final_page.source.source = "a-different-collection".into(),
            "version" => final_page.source.adapter_version = 2,
            "mask" => final_page.source.field_mask = vec![DetailField::State],
            _ => unreachable!(),
        }
        let before = read(&store, DetailFacet::Comments).await;
        let revision = store.revision().await.unwrap();
        assert_eq!(
            store.apply_detail(final_page).await.unwrap_err().code,
            ErrorCode::StaleView,
            "{drift}: an exhausted response from a changed representation is not absence evidence"
        );
        assert_eq!(store.revision().await.unwrap(), revision);
        assert_eq!(read(&store, DetailFacet::Comments).await, before);
        assert_eq!(store.draft("a", "pull").await.unwrap(), Some(draft));
    }
}

#[tokio::test]
async fn accepted_head_change_stales_current_head_checks_and_discards_old_continuation() {
    let (_directory, store, actor) = fixture().await;
    parent(&store, &actor, "open", Some(HEAD_A), EARLY).await;
    detail_head(&store, &actor, Some(HEAD_A), EARLY).await;
    let draft = authored(&store).await;
    let mut first = commit(&store, &actor, DetailFacet::Checks).await;
    bind_checks(&store, &mut first, HEAD_A).await;
    first.source.observed_at = VALIDATED.into();
    first.entries = vec![current_check(HEAD_A)];
    first.next_cursor = Some("head-a-page2".into());
    first.complete = false;
    first.whole_scope = false;
    store.apply_detail(first).await.unwrap();
    let old_lease = store
        .begin_detail("a", "1", "pull", DetailFacet::Checks)
        .await
        .unwrap();
    let mut late = from_lease(&actor, DetailFacet::Checks, old_lease);
    bind_checks(&store, &mut late, HEAD_A).await;
    let old = read(&store, DetailFacet::Checks).await;
    assert_eq!(old.evidence.freshness, DetailFreshness::Fresh);

    parent(&store, &actor, "open", Some(HEAD_B), LATER).await;
    assert_eq!(
        store.apply_detail(late).await.unwrap_err().code,
        ErrorCode::StaleView
    );
    let historical = read(&store, DetailFacet::Checks).await;
    assert_eq!(historical.entries, old.entries, "retain historical checks");
    assert_eq!(
        historical.evidence.freshness,
        DetailFreshness::Stale,
        "head-a check success cannot remain current-head evidence at head-b"
    );
    detail_head(&store, &actor, Some(HEAD_B), LATER).await;
    let replacement = store
        .begin_detail("a", "1", "pull", DetailFacet::Checks)
        .await
        .unwrap();
    assert_eq!(replacement.next_cursor, None);
    assert_eq!(replacement.etag, None);
    assert_eq!(store.draft("a", "pull").await.unwrap(), Some(draft));
}

#[tokio::test]
async fn same_head_body_context_change_retires_queued_and_inflight_check_traversals() {
    for source_repository_provider_id in ["1", "2"] {
        let (_directory, store, actor) = fixture().await;
        parent(&store, &actor, "open", Some(HEAD_A), EARLY).await;
        detail_head_source(&store, &actor, Some(HEAD_A), "1", EARLY).await;
        let draft = authored(&store).await;

        let mut first = commit(&store, &actor, DetailFacet::Checks).await;
        bind_checks(&store, &mut first, HEAD_A).await;
        let old_context = first.check_context.clone();
        first.source.observed_at = VALIDATED.into();
        first.entries = vec![current_check(HEAD_A)];
        first.next_cursor = Some("old-context-page-2".into());
        first.complete = false;
        first.whole_scope = false;
        store.apply_detail(first).await.unwrap();

        let old_lease = store
            .begin_detail("a", "1", "pull", DetailFacet::Checks)
            .await
            .unwrap();
        assert_eq!(old_lease.next_cursor.as_deref(), Some("old-context-page-2"));
        let mut late = from_lease(&actor, DetailFacet::Checks, old_lease);
        late.reconciliation = DetailReconciliation {
            enumeration: DetailEnumeration::FullEnumeration,
            head_scope: DetailHeadScope::CurrentHead,
        };
        late.subject_binding = Some(binding(&store).await);
        late.source.field_mask = vec![DetailField::Check, DetailField::HeadOid];
        late.check_context = old_context;
        late.entries = vec![current_check(HEAD_A)];

        // A Body refresh changes the authoritative metadata facet revision even
        // when the OID is unchanged. The second case also changes the source repo.
        detail_head_source(
            &store,
            &actor,
            Some(HEAD_A),
            source_repository_provider_id,
            LATER,
        )
        .await;
        assert_eq!(
            store.apply_detail(late).await.unwrap_err().code,
            ErrorCode::StaleView,
            "source repository {source_repository_provider_id}: old in-flight page"
        );

        let historical = read(&store, DetailFacet::Checks).await;
        assert_eq!(historical.entries.len(), 1, "retain prior context history");
        assert_eq!(historical.evidence.freshness, DetailFreshness::Stale);
        assert_eq!(historical.evidence.coverage.state, CoverageState::Partial);
        assert_eq!(
            historical.check_aggregate(Some(HEAD_A)),
            Some(CheckAggregate {
                state: CheckAggregateState::Stale,
                authoritative: false,
                total: 1,
            })
        );

        let replacement = store
            .begin_detail("a", "1", "pull", DetailFacet::Checks)
            .await
            .unwrap();
        assert_eq!(replacement.next_cursor, None, "queued cursor is retired");
        assert_eq!(replacement.etag, None, "old validators are retired");
        assert_eq!(store.draft("a", "pull").await.unwrap(), Some(draft));
    }
}

#[tokio::test]
async fn same_head_source_repository_change_replaces_old_row_ordering_clocks() {
    let (_directory, store, actor) = fixture().await;
    parent(&store, &actor, "open", Some(HEAD_A), EARLY).await;
    detail_head_source(&store, &actor, Some(HEAD_A), "source-a", EARLY).await;

    let mut first = commit(&store, &actor, DetailFacet::Checks).await;
    bind_checks(&store, &mut first, HEAD_A).await;
    let mut green = current_check(HEAD_A);
    let Some(NativeDetailPayload::CheckV1(check)) = &mut green.native else {
        panic!("fixture check")
    };
    check.updated_at = Some(LATEST.into());
    first.entries = vec![green];
    first.source.observed_at = VALIDATED.into();
    store.apply_detail(first).await.unwrap();

    detail_head_source(&store, &actor, Some(HEAD_A), "source-b", LATER).await;
    let replacement_lease = store
        .begin_detail("a", "1", "pull", DetailFacet::Checks)
        .await
        .unwrap();
    assert_eq!(replacement_lease.next_cursor, None);
    assert_eq!(replacement_lease.etag, None);
    assert_eq!(replacement_lease.source, None);
    assert_eq!(replacement_lease.reconciliation, None);

    let mut replacement = from_lease(&actor, DetailFacet::Checks, replacement_lease);
    bind_checks(&store, &mut replacement, HEAD_A).await;
    let mut failed = current_check(HEAD_A);
    let Some(NativeDetailPayload::CheckV1(check)) = &mut failed.native else {
        panic!("fixture check")
    };
    check.updated_at = Some(EARLY.into());
    check.state = CheckStateV1::CheckRun {
        status: "completed".into(),
        conclusion: Some("failure".into()),
    };
    replacement.entries = vec![failed];
    replacement.source.observed_at = REVALIDATED.into();
    store.apply_detail(replacement).await.unwrap();

    let saved = read(&store, DetailFacet::Checks).await;
    assert_eq!(saved.evidence.freshness, DetailFreshness::Fresh);
    assert_eq!(
        saved.check_aggregate(Some(HEAD_A)),
        Some(CheckAggregate {
            state: CheckAggregateState::Failed,
            authoritative: true,
            total: 1,
        })
    );
    let Some(NativeDetailPayload::CheckV1(check)) = &sole_entry(&saved).native else {
        panic!("saved check")
    };
    assert_eq!(
        check.state,
        CheckStateV1::CheckRun {
            status: "completed".into(),
            conclusion: Some("failure".into()),
        }
    );
    assert_eq!(check.updated_at.as_deref(), Some(EARLY));
}

#[tokio::test]
async fn completed_same_representation_multipage_scan_reconciles_only_its_own_children() {
    for facet in COLLECTIONS {
        let (_directory, store, actor) = fixture().await;
        let draft = authored(&store).await;
        let mut initial = commit(&store, &actor, facet).await;
        initial.entries = vec![entry("gone"), entry("edited")];
        store.apply_detail(initial).await.unwrap();
        let mut independent = commit(&store, &actor, DetailFacet::Body).await;
        independent.body = known(Some("independent saved description"));
        store.apply_detail(independent).await.unwrap();

        let mut first = commit(&store, &actor, facet).await;
        first.source.observed_at = VALIDATED.into();
        first.entries = vec![observed_entry("edited", "newer child edit", Some(LATER))];
        first.complete = false;
        first.whole_scope = false;
        first.next_cursor = Some("page2".into());
        store.apply_detail(first).await.unwrap();
        assert_eq!(
            read(&store, facet).await.entries.len(),
            2,
            "partial scan retains unseen children"
        );
        let lease = store.begin_detail("a", "1", "pull", facet).await.unwrap();
        let mut final_page = from_lease(&actor, facet, lease);
        final_page.whole_scope = false;
        final_page.source.observed_at = REVALIDATED.into();
        final_page.entries = vec![
            observed_entry("edited", "older overlapping edit", Some(EARLY)),
            entry("new"),
        ];
        store.apply_detail(final_page).await.unwrap();
        let saved = read(&store, facet).await;
        assert_eq!(saved.evidence.coverage.state, CoverageState::Complete);
        assert_eq!(
            saved
                .entries
                .iter()
                .map(|entry| entry.id.as_str())
                .collect::<Vec<_>>(),
            ["edited", "new"]
        );
        assert_eq!(
            saved.entries[0].body.text.as_deref(),
            Some("newer child edit")
        );
        assert_eq!(
            read(&store, DetailFacet::Body).await.body.text.as_deref(),
            Some("independent saved description")
        );
        assert_eq!(
            store
                .begin_detail("a", "1", "pull", facet)
                .await
                .unwrap()
                .etag,
            None,
            "terminal pagination-page validator never covers the whole collection"
        );
        assert_eq!(store.draft("a", "pull").await.unwrap(), Some(draft));
    }
}

#[tokio::test]
async fn close_merge_and_historical_review_head_do_not_order_unrelated_child_fields() {
    let (_directory, store, actor) = fixture().await;
    parent(&store, &actor, "open", Some(HEAD_A), EARLY).await;
    let draft = authored(&store).await;
    let mut history = commit(&store, &actor, DetailFacet::Reviews).await;
    history.source.observed_at = VALIDATED.into();
    let mut review = observed_entry("review", "historical review", Some(LATER));
    review.head_oid = Some(HEAD_A.into());
    review.field_mask.push(DetailField::HeadOid);
    history.entries = vec![review];
    store.apply_detail(history).await.unwrap();
    let before = read(&store, DetailFacet::Reviews).await;
    parent(&store, &actor, "closed", Some(HEAD_B), LATER).await;
    parent(&store, &actor, "merged", Some(HEAD_B), LATEST).await;
    let after = read(&store, DetailFacet::Reviews).await;
    assert_eq!(after.entries, before.entries);
    assert_eq!(after.evidence.coverage, before.evidence.coverage);
    assert_eq!(after.evidence.freshness, DetailFreshness::Fresh);
    assert_eq!(sole_entry(&after).head_oid.as_deref(), Some(HEAD_A));
    assert_eq!(store.draft("a", "pull").await.unwrap(), Some(draft));
}

#[tokio::test]
async fn terminal_delta_and_uncertain_receipts_do_not_grant_absence_or_validator_authority() {
    assert_eq!(
        DetailReconciliation::default().enumeration,
        DetailEnumeration::Uncertain
    );
    for facet in COLLECTIONS {
        for enumeration in [DetailEnumeration::Incremental, DetailEnumeration::Uncertain] {
            let (_directory, store, actor) = fixture().await;
            let draft = authored(&store).await;
            let mut initial = commit(&store, &actor, facet).await;
            initial.entries = vec![entry("retained"), entry("seen")];
            store.apply_detail(initial).await.unwrap();

            let mut delta = commit(&store, &actor, facet).await;
            delta.reconciliation.enumeration = enumeration;
            delta.source.observed_at = REVALIDATED.into();
            // An exhausted empty delta is not a full empty collection.
            store.apply_detail(delta).await.unwrap();
            let saved = read(&store, facet).await;
            assert_eq!(saved.entries.len(), 2, "{facet:?}/{enumeration:?}");
            assert_eq!(saved.evidence.coverage.state, CoverageState::Partial);
            assert_eq!(saved.evidence.saved_empty, None);
            let lease = store.begin_detail("a", "1", "pull", facet).await.unwrap();
            assert_eq!(lease.etag, None);
            assert_eq!(lease.next_cursor, None);
            let mut invalid_304 = from_lease(&actor, facet, lease);
            invalid_304.reconciliation.enumeration = enumeration;
            invalid_304.not_modified = true;
            let before = read(&store, facet).await;
            let revision = store.revision().await.unwrap();
            assert_eq!(
                store.apply_detail(invalid_304).await.unwrap_err().code,
                ErrorCode::InvalidInput
            );
            assert_eq!(store.revision().await.unwrap(), revision);
            assert_eq!(read(&store, facet).await, before);

            let full_empty = commit(&store, &actor, facet).await;
            store.apply_detail(full_empty).await.unwrap();
            let empty = read(&store, facet).await;
            assert!(empty.entries.is_empty());
            assert_eq!(empty.evidence.coverage.state, CoverageState::Complete);
            assert_eq!(empty.evidence.saved_empty, Some(true));
            assert_eq!(store.draft("a", "pull").await.unwrap(), Some(draft));
        }
    }
}

#[tokio::test]
async fn whole_collection_304_requires_matching_representation_and_opaque_validator() {
    for mismatch in ["validator", "endpoint", "version", "mask", "enumeration"] {
        let (_directory, store, actor) = fixture().await;
        let draft = authored(&store).await;
        let mut initial = commit(&store, &actor, DetailFacet::Comments).await;
        initial.entries = vec![entry("saved")];
        initial.source.observed_at = VALIDATED.into();
        store.apply_detail(initial).await.unwrap();
        let mut invalid = commit(&store, &actor, DetailFacet::Comments).await;
        invalid.not_modified = true;
        invalid.source.observed_at = REVALIDATED.into();
        match mismatch {
            "validator" => invalid.etag = Some("opaque-unrelated-zzz".into()),
            "endpoint" => invalid.source.source = "another-endpoint".into(),
            "version" => invalid.source.adapter_version = 2,
            "mask" => invalid.source.field_mask = vec![DetailField::State],
            "enumeration" => invalid.reconciliation.enumeration = DetailEnumeration::Uncertain,
            _ => unreachable!(),
        }
        let before = read(&store, DetailFacet::Comments).await;
        let revision = store.revision().await.unwrap();
        assert_eq!(
            store.apply_detail(invalid).await.unwrap_err().code,
            ErrorCode::InvalidInput,
            "{mismatch}"
        );
        assert_eq!(store.revision().await.unwrap(), revision);
        assert_eq!(read(&store, DetailFacet::Comments).await, before);
        assert_eq!(store.draft("a", "pull").await.unwrap(), Some(draft));
    }
}

#[tokio::test]
async fn conditional_validation_preserves_unobserved_child_field_clock_and_validation() {
    let (_directory, store, actor) = fixture().await;
    let mut initial = commit(&store, &actor, DetailFacet::Comments).await;
    initial.source.observed_at = VALIDATED.into();
    initial.entries = vec![observed_entry("child", "saved body", Some(LATER))];
    store.apply_detail(initial).await.unwrap();
    let body_validation = sole_entry(&read(&store, DetailFacet::Comments).await)
        .field_validations
        .iter()
        .find(|v| v.field == DetailField::Body)
        .unwrap()
        .clone();

    let mut omitted = commit(&store, &actor, DetailFacet::Comments).await;
    omitted.source.observed_at = REVALIDATED.into();
    let mut child = entry("child");
    child.field_mask = vec![DetailField::Author];
    child.author = Some("current actor".into());
    omitted.entries = vec![child];
    store.apply_detail(omitted).await.unwrap();
    let mut unchanged = commit(&store, &actor, DetailFacet::Comments).await;
    unchanged.not_modified = true;
    unchanged.source.observed_at = "2099-01-01T00:02:00Z".into();
    store.apply_detail(unchanged).await.unwrap();
    let snapshot = read(&store, DetailFacet::Comments).await;
    let saved = sole_entry(&snapshot);
    assert_eq!(saved.author.as_deref(), Some("current actor"));
    assert_eq!(saved.observed_body_state, DetailValueState::NotLoaded);
    assert_eq!(
        saved
            .field_validations
            .iter()
            .find(|v| v.field == DetailField::Body),
        Some(&body_validation)
    );
    assert_eq!(saved.body.text.as_deref(), Some("saved body"));

    let mut old = commit(&store, &actor, DetailFacet::Comments).await;
    old.entries = vec![observed_entry("child", "obsolete body", Some(EARLY))];
    store.apply_detail(old).await.unwrap();
    assert_eq!(
        sole_entry(&read(&store, DetailFacet::Comments).await)
            .body
            .text
            .as_deref(),
        Some("saved body")
    );
}

#[tokio::test]
async fn persisted_traversal_proof_resumes_after_cold_reopen_and_fences_old_generation() {
    let (directory, store, actor) = fixture().await;
    let draft = authored(&store).await;
    let mut old = commit(&store, &actor, DetailFacet::Comments).await;
    old.entries = vec![entry("gone")];
    store.apply_detail(old).await.unwrap();
    let mut first = commit(&store, &actor, DetailFacet::Comments).await;
    first.entries = vec![entry("first")];
    first.next_cursor = Some("page2".into());
    first.complete = false;
    first.whole_scope = false;
    store.apply_detail(first).await.unwrap();
    let abandoned = store
        .begin_detail("a", "1", "pull", DetailFacet::Comments)
        .await
        .unwrap();
    store.close().await.unwrap();
    drop(store);
    let store = Store::open(directory.path().join("facet.sqlite"))
        .await
        .unwrap();
    let current = store
        .begin_detail("a", "1", "pull", DetailFacet::Comments)
        .await
        .unwrap();
    assert_eq!(current.next_cursor.as_deref(), Some("page2"));
    assert_eq!(
        current.reconciliation,
        Some(DetailReconciliation::full_history())
    );
    let mut late = from_lease(&actor, DetailFacet::Comments, abandoned);
    late.whole_scope = false;
    late.entries = vec![entry("late")];
    let revision = store.revision().await.unwrap();
    assert_eq!(
        store.apply_detail(late).await.unwrap_err().code,
        ErrorCode::StaleView
    );
    assert_eq!(store.revision().await.unwrap(), revision);
    let mut terminal = from_lease(&actor, DetailFacet::Comments, current);
    terminal.whole_scope = false;
    terminal.entries = vec![entry("second")];
    store.apply_detail(terminal).await.unwrap();
    assert_eq!(
        read(&store, DetailFacet::Comments)
            .await
            .entries
            .iter()
            .map(|v| v.id.as_str())
            .collect::<Vec<_>>(),
        ["first", "second"]
    );
    assert_eq!(store.draft("a", "pull").await.unwrap(), Some(draft));
}

#[tokio::test]
async fn new_head_empty_check_set_is_complete_without_ordering_old_head_or_review_history() {
    let (_directory, store, actor) = fixture().await;
    parent(&store, &actor, "open", Some(HEAD_A), EARLY).await;
    detail_head(&store, &actor, Some(HEAD_A), EARLY).await;
    let draft = authored(&store).await;
    let mut initial = commit(&store, &actor, DetailFacet::Checks).await;
    bind_checks(&store, &mut initial, HEAD_A).await;
    initial.source.provider_updated_at = Some(LATER.into());
    initial.source.observed_at = VALIDATED.into();
    initial.entries = vec![current_check(HEAD_A)];
    store.apply_detail(initial).await.unwrap();
    parent(&store, &actor, "open", Some(HEAD_B), LATEST).await;
    detail_head(&store, &actor, Some(HEAD_B), LATEST).await;
    let mut replacement = commit(&store, &actor, DetailFacet::Checks).await;
    bind_checks(&store, &mut replacement, HEAD_B).await;
    replacement.source.provider_updated_at = Some(EARLY.into());
    replacement.source.observed_at = REVALIDATED.into();
    // Different head has an independent ordering domain, even if its clock is older.
    store.apply_detail(replacement).await.unwrap();
    let empty = read(&store, DetailFacet::Checks).await;
    assert!(empty.entries.is_empty());
    assert_eq!(empty.evidence.saved_empty, Some(true));
    assert_eq!(empty.evidence.freshness, DetailFreshness::Fresh);
    let mut matching = commit(&store, &actor, DetailFacet::Checks).await;
    bind_checks(&store, &mut matching, HEAD_B).await;
    matching.not_modified = true;
    matching.source.provider_updated_at = None;
    matching.source.observed_at = "2099-01-01T00:02:00Z".into();
    store.apply_detail(matching).await.unwrap();
    assert_eq!(
        read(&store, DetailFacet::Checks).await.evidence.saved_empty,
        Some(true)
    );
    assert_eq!(store.draft("a", "pull").await.unwrap(), Some(draft));
}

#[tokio::test]
async fn newer_authoritative_description_head_prevents_summary_head_checks_from_staying_fresh() {
    let (_directory, store, actor) = fixture().await;
    parent(&store, &actor, "open", Some(HEAD_A), EARLY).await;
    detail_head(&store, &actor, Some(HEAD_A), EARLY).await;
    let draft = authored(&store).await;
    let mut checks = commit(&store, &actor, DetailFacet::Checks).await;
    bind_checks(&store, &mut checks, HEAD_A).await;
    checks.source.observed_at = VALIDATED.into();
    checks.entries = vec![current_check(HEAD_A)];
    store.apply_detail(checks).await.unwrap();

    let mut description = commit(&store, &actor, DetailFacet::Body).await;
    description.subject_binding = Some(binding(&store).await);
    description.source.provider_updated_at = Some(LATEST.into());
    description.source.observed_at = VALIDATED.into();
    description.metadata = Some(ResourceMetadataObservation {
        kind: RemoteItemKind::PullRequest,
        values: ResourceMetadataValues {
            head: Some(DetailBranch {
                name: "feature".into(),
                oid: HEAD_B.into(),
                repository: None,
            }),
            ..Default::default()
        },
        fields: vec![MetadataObservedField {
            field: MetadataField::Head,
            state: DetailValueState::Known,
        }],
        source: MetadataSource {
            source: description.source.source.clone(),
            adapter_version: description.source.adapter_version,
            provider_updated_at: description.source.provider_updated_at.clone(),
            observed_at: description.source.observed_at.clone(),
        },
    });
    store.apply_detail(description).await.unwrap();
    // R77 intentionally keeps the list projection independent: an intermediate
    // summary cannot overwrite the newer authoritative detail head.
    parent(&store, &actor, "open", Some(HEAD_A), LATER).await;
    assert_eq!(
        store
            .item("a", "pull")
            .await
            .unwrap()
            .item
            .unwrap()
            .head_oid
            .as_deref(),
        Some(HEAD_A)
    );
    assert_eq!(
        read(&store, DetailFacet::Body)
            .await
            .metadata
            .unwrap()
            .values
            .head
            .unwrap()
            .oid,
        HEAD_B
    );
    let saved = read(&store, DetailFacet::Checks).await;
    assert_eq!(sole_entry(&saved).head_oid.as_deref(), Some(HEAD_A));
    assert_eq!(
        saved.evidence.freshness,
        DetailFreshness::Stale,
        "known newer head-b detail excludes fresh current-head evidence for summary head-a"
    );
    assert_eq!(store.draft("a", "pull").await.unwrap(), Some(draft));
}

#[tokio::test]
async fn legacy_and_unknown_traversal_evidence_keep_saved_data_without_resume_or_absence_authority()
{
    for evidence in ["legacy", "future-version", "future-strategy"] {
        for complete in [true, false] {
            let (directory, store, actor) = fixture().await;
            let draft = authored(&store).await;
            let mut initial = commit(&store, &actor, DetailFacet::Comments).await;
            initial.entries = vec![entry("saved")];
            initial.source.observed_at = VALIDATED.into();
            initial.complete = complete;
            initial.whole_scope = complete;
            initial.next_cursor = (!complete).then(|| "old-cursor".into());
            store.apply_detail(initial).await.unwrap();
            store.close().await.unwrap();
            drop(store);

            // Native-only synthetic historical/future JSON fixture. SQL schema
            // and provider/public values are unchanged, with no live runtime.
            let path = directory.path().join("facet.sqlite");
            let mut connection = sqlx::SqliteConnection::connect_with(
                &sqlx::sqlite::SqliteConnectOptions::new().filename(&path),
            )
            .await
            .unwrap();
            let raw: String = sqlx::query_scalar("SELECT source_json FROM detail_observations WHERE account_id='a' AND subject_id='pull' AND facet='comments'").fetch_one(&mut connection).await.unwrap();
            let mut json: serde_json::Value = serde_json::from_str(&raw).unwrap();
            match evidence {
                "legacy" => {
                    json.as_object_mut().unwrap().remove("traversal_evidence");
                }
                "future-version" => json["traversal_evidence"]["version"] = 99.into(),
                "future-strategy" => {
                    json["traversal_evidence"]["reconciliation"]["enumeration"] =
                        "future-proof".into()
                }
                _ => unreachable!(),
            }
            sqlx::query("UPDATE detail_observations SET source_json=? WHERE account_id='a' AND subject_id='pull' AND facet='comments'").bind(json.to_string()).execute(&mut connection).await.unwrap();
            connection.close().await.unwrap();

            let store = Store::open(&path).await.unwrap();
            let retained = read(&store, DetailFacet::Comments).await;
            assert_eq!(sole_entry(&retained).id, "saved");
            assert_eq!(
                retained.evidence.coverage.state,
                CoverageState::Partial,
                "{evidence}/{complete}"
            );
            assert_eq!(retained.evidence.saved_empty, None);
            let lease = store
                .begin_detail(
                    "a",
                    &actor.authorization_epoch,
                    "pull",
                    DetailFacet::Comments,
                )
                .await
                .unwrap();
            assert_eq!(lease.next_cursor, None);
            assert_eq!(lease.etag, None);
            assert_eq!(lease.reconciliation, None);
            assert_eq!(store.draft("a", "pull").await.unwrap(), Some(draft));
            // A new qualified full traversal re-establishes known evidence.
            let mut fresh = from_lease(&actor, DetailFacet::Comments, lease);
            fresh.entries = vec![entry("replacement")];
            store.apply_detail(fresh).await.unwrap();
            let saved = read(&store, DetailFacet::Comments).await;
            assert_eq!(sole_entry(&saved).id, "replacement");
            assert_eq!(saved.evidence.coverage.state, CoverageState::Complete);
        }
    }
}

#[tokio::test]
async fn per_field_clocks_never_invent_order_across_different_representations() {
    for difference in ["endpoint", "version"] {
        let (_directory, store, actor) = fixture().await;
        let mut first = commit(&store, &actor, DetailFacet::Comments).await;
        first.entries = vec![observed_entry("child", "first representation", Some(LATER))];
        store.apply_detail(first).await.unwrap();
        let mut other = commit(&store, &actor, DetailFacet::Comments).await;
        match difference {
            "endpoint" => other.source.source = "independent-representation".into(),
            "version" => other.source.adapter_version = 2,
            _ => unreachable!(),
        }
        other.entries = vec![observed_entry("child", "other representation", Some(EARLY))];
        let other_source = other.source.clone();
        store.apply_detail(other).await.unwrap();
        assert_eq!(
            sole_entry(&read(&store, DetailFacet::Comments).await)
                .body
                .text
                .as_deref(),
            Some("other representation")
        );
        let mut older = commit(&store, &actor, DetailFacet::Comments).await;
        older.source = other_source;
        older.entries = vec![observed_entry(
            "child",
            "older in comparable representation",
            Some("2026-10-03T00:00:00Z"),
        )];
        store.apply_detail(older).await.unwrap();
        assert_eq!(
            sole_entry(&read(&store, DetailFacet::Comments).await)
                .body
                .text
                .as_deref(),
            Some("other representation")
        );
    }
}

#[tokio::test]
async fn known_head_conflict_revokes_old_validator_and_admission_but_preserves_review_history() {
    let (_directory, store, actor) = fixture().await;
    parent(&store, &actor, "open", Some(HEAD_A), EARLY).await;
    detail_head(&store, &actor, Some(HEAD_A), EARLY).await;
    let draft = authored(&store).await;
    let mut checks = commit(&store, &actor, DetailFacet::Checks).await;
    bind_checks(&store, &mut checks, HEAD_A).await;
    checks.source.observed_at = VALIDATED.into();
    checks.entries = vec![current_check(HEAD_A)];
    store.apply_detail(checks).await.unwrap();
    let mut history = commit(&store, &actor, DetailFacet::Reviews).await;
    history.source.observed_at = VALIDATED.into();
    history.entries = vec![historical_entry(HEAD_A)];
    store.apply_detail(history).await.unwrap();
    let before_history = read(&store, DetailFacet::Reviews).await;
    let mut old_304 = commit(&store, &actor, DetailFacet::Checks).await;
    bind_checks(&store, &mut old_304, HEAD_A).await;
    old_304.not_modified = true;
    detail_head(&store, &actor, Some(HEAD_B), LATEST).await;
    let revision = store.revision().await.unwrap();
    assert_eq!(
        store.apply_detail(old_304).await.unwrap_err().code,
        ErrorCode::StaleView
    );
    assert_eq!(
        store
            .begin_detail("a", "1", "pull", DetailFacet::Checks)
            .await
            .unwrap_err()
            .code,
        ErrorCode::StaleView
    );
    assert_eq!(
        store
            .request_detail("a", "1", "pull", DetailFacet::Checks)
            .await
            .unwrap_err()
            .code,
        ErrorCode::StaleView
    );
    assert_eq!(store.revision().await.unwrap(), revision);
    assert!(store.pending_details().await.unwrap().is_empty());
    let stale = read(&store, DetailFacet::Checks).await;
    assert_eq!(stale.evidence.freshness, DetailFreshness::Stale);
    assert_eq!(stale.entries.len(), 1);
    let history = read(&store, DetailFacet::Reviews).await;
    assert_eq!(history.entries, before_history.entries);
    assert_eq!(history.evidence.coverage, before_history.evidence.coverage);
    assert_eq!(history.evidence.freshness, DetailFreshness::Fresh);

    // Agreement restores admission, without projecting either source over the
    // other or recovering a validator that was invalidated during the conflict.
    detail_head(&store, &actor, Some(HEAD_A), "2026-10-03T04:00:00Z").await;
    let current = store
        .begin_detail("a", "1", "pull", DetailFacet::Checks)
        .await
        .unwrap();
    assert_eq!(current.etag, None);
    let mut fresh = from_lease(&actor, DetailFacet::Checks, current);
    bind_checks(&store, &mut fresh, HEAD_A).await;
    fresh.source.observed_at = REVALIDATED.into();
    store.apply_detail(fresh).await.unwrap();
    assert_eq!(
        read(&store, DetailFacet::Checks).await.evidence.saved_empty,
        Some(true)
    );
    assert_eq!(store.draft("a", "pull").await.unwrap(), Some(draft));
}

#[tokio::test]
async fn unclassified_first_head_receipt_is_fenced_without_inventing_pre_dispatch_strategy() {
    let (_directory, store, actor) = fixture().await;
    parent(&store, &actor, "open", Some(HEAD_A), EARLY).await;
    detail_head(&store, &actor, Some(HEAD_B), LATEST).await;
    let lease = store
        .begin_detail("a", "1", "pull", DetailFacet::Checks)
        .await
        .unwrap();
    assert_eq!(
        lease.reconciliation, None,
        "future adapter has not declared head scope yet"
    );
    let mut first = from_lease(&actor, DetailFacet::Checks, lease);
    bind_checks(&store, &mut first, HEAD_A).await;
    first.entries = vec![current_check(HEAD_A)];
    let revision = store.revision().await.unwrap();
    assert_eq!(
        store.apply_detail(first).await.unwrap_err().code,
        ErrorCode::StaleView
    );
    assert_eq!(store.revision().await.unwrap(), revision);
    assert!(read(&store, DetailFacet::Checks).await.entries.is_empty());

    // Head history remains usable with the same summary/description mismatch.
    let mut history = commit(&store, &actor, DetailFacet::Reviews).await;
    history.entries = vec![historical_entry(HEAD_A)];
    store.apply_detail(history).await.unwrap();
    assert_eq!(read(&store, DetailFacet::Reviews).await.entries.len(), 1);
}

#[tokio::test]
async fn matching_or_unobserved_description_head_keeps_declared_current_head_evidence_usable() {
    for known_head in [true, false] {
        let (_directory, store, actor) = fixture().await;
        parent(&store, &actor, "open", Some(HEAD_A), EARLY).await;
        if known_head {
            detail_head(&store, &actor, Some(HEAD_A), LATEST).await;
        } else {
            let mut description = commit(&store, &actor, DetailFacet::Body).await;
            description.subject_binding = Some(binding(&store).await);
            description.source.observed_at = VALIDATED.into();
            description.metadata = Some(ResourceMetadataObservation {
                kind: RemoteItemKind::PullRequest,
                values: ResourceMetadataValues {
                    title: Some("Known title without head authority".into()),
                    ..Default::default()
                },
                fields: vec![MetadataObservedField {
                    field: MetadataField::Title,
                    state: DetailValueState::Known,
                }],
                source: MetadataSource {
                    source: description.source.source.clone(),
                    adapter_version: description.source.adapter_version,
                    provider_updated_at: description.source.provider_updated_at.clone(),
                    observed_at: description.source.observed_at.clone(),
                },
            });
            store.apply_detail(description).await.unwrap();
        }
        let mut checks = commit(&store, &actor, DetailFacet::Checks).await;
        bind_checks(&store, &mut checks, HEAD_A).await;
        checks.source.observed_at = VALIDATED.into();
        checks.entries = vec![current_check(HEAD_A)];
        if !known_head {
            assert_eq!(
                store.apply_detail(checks).await.unwrap_err().code,
                ErrorCode::NotFound
            );
            assert!(read(&store, DetailFacet::Checks).await.entries.is_empty());
            continue;
        }
        store.apply_detail(checks).await.unwrap();
        let saved = read(&store, DetailFacet::Checks).await;
        assert_eq!(
            saved.evidence.freshness,
            DetailFreshness::Fresh,
            "knownHead={known_head}"
        );
        assert_eq!(saved.evidence.coverage.state, CoverageState::Complete);
        let mut matching = commit(&store, &actor, DetailFacet::Checks).await;
        bind_checks(&store, &mut matching, HEAD_A).await;
        matching.not_modified = true;
        store.apply_detail(matching).await.unwrap();
        assert_eq!(read(&store, DetailFacet::Checks).await.entries.len(), 1);
    }
}

async fn activity_page(store: &Store, actor: &RemoteAccount) -> DetailCommit {
    let mut page = commit(store, actor, DetailFacet::Activity).await;
    page.reconciliation = DetailReconciliation::default();
    page.subject_binding = Some(binding(store).await);
    page.source.field_mask = vec![
        DetailField::Body,
        DetailField::Author,
        DetailField::UpdatedAt,
        DetailField::Activity,
    ];
    page.source.provider_updated_at = None;
    page.etag = None;
    page
}

#[tokio::test]
async fn terminal_activity_cap_preserves_unread_evidence_history_and_cold_refresh() {
    let (directory, store, actor) = fixture().await;
    let draft = authored(&store).await;
    let mut first = activity_page(&store, &actor).await;
    let mut event = observed_entry("event", "retained event", Some(EARLY));
    event.title = None;
    event.state = None;
    event.head_oid = None;
    event.observed_body_state = event.body.state;
    event.native = Some(NativeDetailPayload::ActivityV1(ActivityEvent {
        kind: "closed".into(),
        supported: true,
        occurred_at: Some(EARLY.into()),
        description: None,
    }));
    event.field_mask = first.source.field_mask.clone();
    first.entries = vec![event];
    first.complete = false;
    first.next_cursor = Some("next-activity-page".into());
    store.apply_detail(first).await.unwrap();

    for invalid in 0..6 {
        let mut page = activity_page(&store, &actor).await;
        page.reconciliation.enumeration = DetailEnumeration::Truncated;
        match invalid {
            0 => {
                page.complete = false;
                page.next_cursor = Some("more".into());
            }
            1 => page.not_modified = true,
            2 => page.facet = DetailFacet::Comments,
            3 => page.reconciliation.head_scope = DetailHeadScope::CurrentHead,
            4 => page.source.source = "foreign/source".into(),
            5 => page.source.adapter_version += 1,
            _ => unreachable!(),
        }
        let revision = store.revision().await.unwrap();
        let before = read(&store, DetailFacet::Activity).await;
        let error = store.apply_detail(page).await.unwrap_err();
        assert_eq!(
            error.code,
            if invalid >= 4 {
                ErrorCode::StaleView
            } else {
                ErrorCode::InvalidInput
            }
        );
        assert_eq!(store.revision().await.unwrap(), revision);
        let after = read(&store, DetailFacet::Activity).await;
        assert_eq!(after.entries, before.entries);
        assert_eq!(after.evidence.coverage, before.evidence.coverage);
    }
    let mut final_page = activity_page(&store, &actor).await;
    final_page.reconciliation.enumeration = DetailEnumeration::Truncated;
    store.apply_detail(final_page).await.unwrap();
    let capped = read(&store, DetailFacet::Activity).await;
    assert_eq!(capped.evidence.coverage.state, CoverageState::Partial);
    assert!(capped.evidence.coverage.remote_has_more);
    assert_eq!(
        capped.entries.len(),
        1,
        "A cap cannot delete absent saved events"
    );
    assert_eq!(store.draft("a", "pull").await.unwrap(), Some(draft.clone()));
    store.close().await.unwrap();
    drop(store);
    let store = Store::open(directory.path().join("facet.sqlite"))
        .await
        .unwrap();
    assert_eq!(
        read(&store, DetailFacet::Activity).await.evidence.coverage,
        capped.evidence.coverage
    );
    let refresh = activity_page(&store, &actor).await;
    assert!(
        refresh.request_cursor.is_none(),
        "A cold refresh starts a new bounded window"
    );
    store.apply_detail(refresh).await.unwrap();
    let refreshed = read(&store, DetailFacet::Activity).await;
    assert_eq!(refreshed.evidence.coverage.state, CoverageState::Partial);
    assert!(!refreshed.evidence.coverage.remote_has_more);
    assert_eq!(refreshed.entries, capped.entries);
    assert_eq!(store.draft("a", "pull").await.unwrap(), Some(draft.clone()));
    store.close().await.unwrap();
}
