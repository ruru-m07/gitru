//! Independent access/evidence tests through the public local storage API.
//! All accounts, subjects, observations, and databases are synthetic; no HTTP
//! client, native vault, runtime scheduler, or personal app data is involved.
use collaboration::*;

const T0: &str = "2025-01-01T00:00:00Z";
const T1: &str = "2025-01-01T01:00:00Z";
const T2: &str = "2025-01-01T02:00:00Z";
const SUBJECT: &str = "pull-67";
const OTHER_SUBJECT: &str = "pull-68";
const REPOSITORY: &str = "repo";

fn account(id: &str, host: &str) -> RemoteAccount {
    RemoteAccount {
        id: id.into(),
        provider: ProviderKind::Github,
        host: host.into(),
        actor_id: format!("actor-{id}"),
        login: format!("fixture-{id}"),
        display_name: None,
        authorization_epoch: "1".into(),
        state: AccountState::Active,
        notifications_supported: true,
    }
}

fn subject(account: &str, id: &str) -> RemoteItem {
    RemoteItem {
        native_inbox: None,
        id: id.into(),
        account_id: account.into(),
        repository_id: Some(REPOSITORY.into()),
        provider_id: format!("native-{id}"),
        kind: RemoteItemKind::PullRequest,
        number: Some(if id == SUBJECT { "67" } else { "68" }.into()),
        title: "Synthetic summary".into(),
        body: Some("A summary body does not authorize detail hydration".into()),
        body_omitted: false,
        author: None,
        web_url: None,
        state: "open".into(),
        // Parent summary time deliberately exceeds every detail observation.
        updated_at: "2029-01-01T00:00:00Z".into(),
        head_oid: None,
        is_draft: None,
        reason: None,
        unread: None,
    }
}

async fn seed(store: &Store, id: &str, host: &str) {
    store.upsert_account(account(id, host)).await.unwrap();
    seed_parent(store, id, host).await;
}

async fn seed_parent(store: &Store, id: &str, host: &str) {
    let epoch = store.account(id).await.unwrap().authorization_epoch;
    let repository = RemoteRepository {
        id: REPOSITORY.into(),
        account_id: id.into(),
        provider_id: "native-repo-42".into(),
        full_name: "fixture/project".into(),
        name: "project".into(),
        web_url: format!("https://{host}/fixture/project"),
        description: None,
        default_branch: None,
        selected: true,
    };
    for (scope, repositories, items) in [
        ("repositories", vec![repository], vec![]),
        (
            "repo:repo:pull_request",
            vec![],
            vec![subject(id, SUBJECT), subject(id, OTHER_SUBJECT)],
        ),
    ] {
        let run_id = store.begin_sync(id, &epoch, scope).await.unwrap();
        store
            .apply_page(PageCommit {
                account_id: id.into(),
                authorization_epoch: epoch.clone(),
                scope: scope.into(),
                run_id,
                repositories,
                items,
                endpoint_aliases: vec![],
                next_cursor: None,
                etag: None,
                last_modified: None,
                not_modified: false,
                complete: true,
                observed_at: T0.into(),
            })
            .await
            .unwrap();
    }
}

fn known(text: Option<&str>) -> DetailValue {
    DetailValue {
        state: DetailValueState::Known,
        text: text.map(str::to_owned),
    }
}

async fn observation(
    store: &Store,
    account: &str,
    subject: &str,
    facet: DetailFacet,
) -> DetailCommit {
    let epoch = store.account(account).await.unwrap().authorization_epoch;
    let lease = store
        .begin_detail(account, &epoch, subject, facet)
        .await
        .unwrap();
    DetailCommit {
        reconciliation: DetailReconciliation::full_history(),
        metadata: None,
        subject_binding: None,
        account_id: account.into(),
        authorization_epoch: epoch,
        authorization_view: lease.authorization_view,
        instance_id: lease.instance_id,
        subject_id: subject.into(),
        facet,
        run_id: lease.run_id,
        request_cursor: lease.next_cursor,
        body: DetailValue::default(),
        entries: vec![],
        source: DetailSource {
            source: format!("fixture-{}-v1", facet.name()),
            adapter_version: 1,
            field_mask: vec![DetailField::Body],
            provider_updated_at: None,
            observed_at: T0.into(),
        },
        next_cursor: None,
        etag: None,
        not_modified: false,
        whole_scope: true,
        complete: true,
        freshness_seconds: 600,
    }
}

fn entry(id: &str, text: &str) -> DetailEntry {
    DetailEntry {
        id: id.into(),
        provider_id: format!("native-{id}"),
        author: Some("saved-author".into()),
        title: Some("saved-title".into()),
        state: Some("approved".into()),
        body: known(Some(text)),
        observed_body_state: DetailValueState::NotLoaded,
        updated_at: Some(T0.into()),
        head_oid: Some("saved-head".into()),
        field_mask: vec![
            DetailField::Body,
            DetailField::Author,
            DetailField::Title,
            DetailField::State,
            DetailField::UpdatedAt,
            DetailField::HeadOid,
        ],
        field_validations: vec![],
        native: None,
    }
}

async fn body(store: &Store, account: &str, subject: &str, value: DetailValue) {
    let mut page = observation(store, account, subject, DetailFacet::Body).await;
    page.body = value;
    store.apply_detail(page).await.unwrap();
}

async fn read(store: &Store, account: &str, subject: &str, facet: DetailFacet) -> DetailSnapshot {
    store
        .detail(DetailQuery {
            account_id: account.into(),
            subject_id: subject.into(),
            facet,
            cursor: None,
            limit: 100,
        })
        .await
        .unwrap()
}

async fn collection(store: &Store, account: &str, facet: DetailFacet, entries: Vec<DetailEntry>) {
    let mut page = observation(store, account, SUBJECT, facet).await;
    page.source.field_mask = vec![
        DetailField::Body,
        DetailField::Author,
        DetailField::Title,
        DetailField::State,
        DetailField::UpdatedAt,
        DetailField::HeadOid,
    ];
    page.entries = entries;
    store.apply_detail(page).await.unwrap();
}

async fn failed_scope(store: &Store, scope: &str, state: SyncState, code: ErrorCode) {
    let epoch = store.account("a").await.unwrap().authorization_epoch;
    let mut status = store
        .scope_state("a", scope)
        .await
        .unwrap()
        .map(|scope| scope.sync)
        .unwrap_or_default();
    status.state = state;
    status.error = Some(CollaborationError::new(code, "Synthetic failure"));
    store
        .set_sync_status("a", &epoch, scope, status)
        .await
        .unwrap();
}

#[tokio::test]
async fn summary_bodies_do_not_hydrate_details_and_local_reads_have_no_side_effects() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path().join("db")).await.unwrap();
    seed(&store, "a", "github.com").await;
    let before = store.revision().await.unwrap();
    for facet in [
        DetailFacet::Body,
        DetailFacet::Comments,
        DetailFacet::Reviews,
        DetailFacet::Checks,
    ] {
        let snapshot = read(&store, "a", SUBJECT, facet).await;
        assert_eq!(snapshot.body, DetailValue::default());
        assert!(snapshot.entries.is_empty());
        assert_eq!(snapshot.evidence.availability, DetailAvailability::Missing);
        assert_eq!(snapshot.evidence.freshness, DetailFreshness::Unknown);
        assert_eq!(snapshot.evidence.coverage.state, CoverageState::Missing);
        assert!(snapshot.evidence.source.is_none());
        assert_eq!(
            store.detail_evidence("a", SUBJECT, facet).await.unwrap(),
            snapshot.evidence
        );
    }
    assert_eq!(store.revision().await.unwrap(), before);
    assert!(store.pending_details().await.unwrap().is_empty());
}

#[tokio::test]
async fn account_subject_and_facet_partitions_keep_their_own_private_values() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path().join("db")).await.unwrap();
    seed(&store, "a", "github.com").await;
    seed(&store, "b", "github.com").await;
    body(&store, "a", SUBJECT, known(Some("a-only-body"))).await;
    body(&store, "b", SUBJECT, known(Some("b-only-body"))).await;
    body(&store, "a", OTHER_SUBJECT, known(Some("another-subject"))).await;
    collection(
        &store,
        "a",
        DetailFacet::Comments,
        vec![entry("same-native-entry", "comment-only")],
    )
    .await;
    collection(
        &store,
        "a",
        DetailFacet::Reviews,
        vec![entry("same-native-entry", "review-only")],
    )
    .await;
    for (account, subject, expected) in [
        ("a", SUBJECT, "a-only-body"),
        ("b", SUBJECT, "b-only-body"),
        ("a", OTHER_SUBJECT, "another-subject"),
    ] {
        assert_eq!(
            read(&store, account, subject, DetailFacet::Body).await.body,
            known(Some(expected))
        );
    }
    for (facet, expected) in [
        (DetailFacet::Comments, "comment-only"),
        (DetailFacet::Reviews, "review-only"),
    ] {
        let snapshot = read(&store, "a", SUBJECT, facet).await;
        assert_eq!(snapshot.entries.len(), 1);
        assert_eq!(snapshot.entries[0].body, known(Some(expected)));
    }
    assert!(
        read(&store, "b", SUBJECT, DetailFacet::Comments)
            .await
            .entries
            .is_empty()
    );
    assert_eq!(
        read(&store, "a", OTHER_SUBJECT, DetailFacet::Comments)
            .await
            .evidence
            .availability,
        DetailAvailability::Missing
    );
}

#[tokio::test]
async fn copied_job_bindings_cannot_cross_account_instance_subject_or_facet() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path().join("db")).await.unwrap();
    seed(&store, "a", "github.com").await;
    seed(&store, "b", "github.com").await;
    seed(&store, "enterprise", "code.fixture.test").await;
    body(&store, "a", SUBJECT, known(Some("saved-a"))).await;
    for boundary in ["account", "instance", "subject", "facet", "run"] {
        let mut page = observation(&store, "a", SUBJECT, DetailFacet::Body).await;
        page.body = known(Some("must-not-commit"));
        match boundary {
            "account" => {
                observation(&store, "b", SUBJECT, DetailFacet::Body).await;
                page.account_id = "b".into();
            }
            "instance" => {
                page.instance_id = store.provider_instance("enterprise").await.unwrap().id;
            }
            "subject" => {
                observation(&store, "a", OTHER_SUBJECT, DetailFacet::Body).await;
                page.subject_id = OTHER_SUBJECT.into();
            }
            "facet" => {
                observation(&store, "a", SUBJECT, DetailFacet::Comments).await;
                page.facet = DetailFacet::Comments;
                page.body = DetailValue::default();
                page.entries = vec![entry("wrong-facet", "must-not-commit")];
            }
            "run" => {
                observation(&store, "a", SUBJECT, DetailFacet::Body).await;
            }
            _ => unreachable!(),
        }
        let before = store.revision().await.unwrap();
        assert_eq!(
            store.apply_detail(page).await.unwrap_err().code,
            ErrorCode::StaleView,
            "{boundary}"
        );
        assert_eq!(store.revision().await.unwrap(), before, "{boundary}");
        assert_eq!(
            read(&store, "a", SUBJECT, DetailFacet::Body).await.body,
            known(Some("saved-a")),
            "{boundary}"
        );
    }
}

#[tokio::test]
async fn transient_failures_retain_saved_reads_without_freshening_field_evidence() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path().join("db")).await.unwrap();
    seed(&store, "a", "github.com").await;
    body(&store, "a", SUBJECT, known(Some("saved offline body"))).await;
    let before = read(&store, "a", SUBJECT, DetailFacet::Body).await;
    for (state, code) in [
        (SyncState::Offline, ErrorCode::Network),
        (SyncState::RateLimited, ErrorCode::RateLimited),
        (SyncState::Error, ErrorCode::Provider),
    ] {
        failed_scope(
            &store,
            &DetailFacet::Body.scope(SUBJECT),
            state.clone(),
            code,
        )
        .await;
        let snapshot = read(&store, "a", SUBJECT, DetailFacet::Body).await;
        assert_eq!(snapshot.body, before.body);
        assert_eq!(snapshot.evidence.availability, DetailAvailability::Ready);
        assert_eq!(snapshot.evidence.access_reason, None);
        assert_eq!(snapshot.evidence.coverage, before.evidence.coverage);
        assert_eq!(snapshot.evidence.source, before.evidence.source);
        assert_eq!(snapshot.evidence.stale_at, before.evidence.stale_at);
        assert_eq!(
            snapshot.evidence.facet_revision,
            before.evidence.facet_revision
        );
        assert_eq!(snapshot.evidence.sync.state, state);
    }
}

#[tokio::test]
async fn explicit_facet_denial_suppresses_saved_content_and_transient_errors_do_not_restore_it() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path().join("db")).await.unwrap();
    seed(&store, "a", "github.com").await;
    body(&store, "a", SUBJECT, known(Some("private denied body"))).await;
    collection(
        &store,
        "a",
        DetailFacet::Comments,
        vec![entry("c", "visible comment")],
    )
    .await;
    let scope = DetailFacet::Body.scope(SUBJECT);
    failed_scope(
        &store,
        &scope,
        SyncState::Error,
        ErrorCode::PermissionDenied,
    )
    .await;
    failed_scope(&store, &scope, SyncState::Offline, ErrorCode::Network).await;
    let snapshot = read(&store, "a", SUBJECT, DetailFacet::Body).await;
    assert_eq!(snapshot.body, DetailValue::default());
    assert!(snapshot.entries.is_empty());
    assert_eq!(
        snapshot.evidence.availability,
        DetailAvailability::Unavailable
    );
    assert_eq!(
        snapshot.evidence.access_reason,
        Some(CapabilityReason::PermissionDenied)
    );
    assert!(snapshot.evidence.source.is_none());
    assert!(snapshot.evidence.facet_revision.is_none());
    assert_eq!(snapshot.evidence.coverage.state, CoverageState::Missing);
    assert_eq!(
        read(&store, "a", SUBJECT, DetailFacet::Comments)
            .await
            .entries
            .len(),
        1
    );
}

#[tokio::test]
async fn denied_parent_hides_every_detail_facet_and_metadata_source() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path().join("db")).await.unwrap();
    seed(&store, "a", "github.com").await;
    body(&store, "a", SUBJECT, known(Some("private subject body"))).await;
    collection(
        &store,
        "a",
        DetailFacet::Comments,
        vec![entry("c", "private comment")],
    )
    .await;
    failed_scope(
        &store,
        "repo:repo:pull_request",
        SyncState::Error,
        ErrorCode::NotFound,
    )
    .await;
    for facet in [DetailFacet::Body, DetailFacet::Comments] {
        let snapshot = read(&store, "a", SUBJECT, facet).await;
        assert_eq!(
            snapshot.evidence.availability,
            DetailAvailability::Unavailable
        );
        assert_eq!(snapshot.body, DetailValue::default());
        assert!(snapshot.entries.is_empty());
        assert!(snapshot.evidence.source.is_none());
        assert!(snapshot.evidence.facet_revision.is_none());
    }
    assert_eq!(
        store.detail_subject("a", SUBJECT).await.unwrap_err().code,
        ErrorCode::PermissionDenied
    );
}

#[tokio::test]
async fn deselection_suppresses_reads_and_reselection_never_revives_an_old_detail_lease() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path().join("db")).await.unwrap();
    seed(&store, "a", "github.com").await;
    body(&store, "a", SUBJECT, known(Some("saved selected body"))).await;
    let mut delayed = observation(&store, "a", SUBJECT, DetailFacet::Body).await;
    delayed.body = known(Some("stale response before deselection"));
    store
        .select_repository("a", REPOSITORY, false)
        .await
        .unwrap();
    let hidden = read(&store, "a", SUBJECT, DetailFacet::Body).await;
    assert_eq!(
        hidden.evidence.availability,
        DetailAvailability::Unavailable
    );
    assert_eq!(hidden.body, DetailValue::default());
    store
        .select_repository("a", REPOSITORY, true)
        .await
        .unwrap();
    assert_eq!(
        read(&store, "a", SUBJECT, DetailFacet::Body).await.body,
        known(Some("saved selected body"))
    );
    let before = store.revision().await.unwrap();
    assert_eq!(
        store.apply_detail(delayed).await.unwrap_err().code,
        ErrorCode::StaleView
    );
    assert_eq!(store.revision().await.unwrap(), before);
    assert_eq!(
        read(&store, "a", SUBJECT, DetailFacet::Body).await.body,
        known(Some("saved selected body"))
    );
}

#[tokio::test]
async fn grant_cutover_clears_private_details_and_fences_old_observations_while_preserving_drafts()
{
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path().join("db")).await.unwrap();
    seed(&store, "a", "github.com").await;
    body(&store, "a", SUBJECT, known(Some("old grant body"))).await;
    let draft = store
        .save_draft(LocalDraft {
            account_id: "a".into(),
            subject_id: SUBJECT.into(),
            body: "authored draft survives".into(),
            generation: "0".into(),
        })
        .await
        .unwrap();
    let mut old = observation(&store, "a", SUBJECT, DetailFacet::Body).await;
    old.body = known(Some("old grant response"));
    let mut reauthorized = store.account("a").await.unwrap();
    reauthorized.authorization_epoch =
        (reauthorized.authorization_epoch.parse::<u64>().unwrap() + 1).to_string();
    store.upsert_account(reauthorized).await.unwrap();
    seed_parent(&store, "a", "github.com").await;
    assert_ne!(
        store.account("a").await.unwrap().authorization_epoch,
        old.authorization_epoch
    );
    let snapshot = read(&store, "a", SUBJECT, DetailFacet::Body).await;
    assert_eq!(snapshot.evidence.availability, DetailAvailability::Missing);
    assert_eq!(snapshot.body, DetailValue::default());
    let before = store.revision().await.unwrap();
    assert_eq!(
        store.apply_detail(old).await.unwrap_err().code,
        ErrorCode::StaleView
    );
    assert_eq!(store.revision().await.unwrap(), before);
    assert_eq!(store.draft("a", SUBJECT).await.unwrap(), Some(draft));
}

#[tokio::test]
async fn omitted_and_oversized_observations_retain_known_text_and_its_validation_time() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path().join("db")).await.unwrap();
    seed(&store, "a", "github.com").await;
    body(&store, "a", SUBJECT, known(Some("saved exact body"))).await;
    let before = read(&store, "a", SUBJECT, DetailFacet::Body).await;
    for (incoming, observed_state) in [
        (
            DetailValue {
                state: DetailValueState::Omitted,
                text: None,
            },
            DetailValueState::Omitted,
        ),
        // This has fewer than 1Mi characters but exceeds the byte ceiling.
        (
            DetailValue {
                state: DetailValueState::Known,
                text: Some("😀".repeat(262_145)),
            },
            DetailValueState::Oversized,
        ),
    ] {
        let mut page = observation(&store, "a", SUBJECT, DetailFacet::Body).await;
        page.body = incoming;
        page.source.observed_at = T2.into();
        store.apply_detail(page).await.unwrap();
        let snapshot = read(&store, "a", SUBJECT, DetailFacet::Body).await;
        assert_eq!(snapshot.body, before.body);
        assert_eq!(snapshot.evidence.observed_state, observed_state);
        assert_eq!(snapshot.evidence.coverage.state, CoverageState::Partial);
        assert_eq!(
            snapshot.evidence.coverage.validated_at,
            before.evidence.coverage.validated_at
        );
        assert_eq!(snapshot.evidence.stale_at, before.evidence.stale_at);
        assert_eq!(
            snapshot.evidence.sync.last_success_at,
            before.evidence.sync.last_success_at
        );
    }
}

#[tokio::test]
async fn initial_omission_and_oversize_remain_distinct_from_authoritative_null_and_empty() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path().join("db")).await.unwrap();
    seed(&store, "a", "github.com").await;
    for (subject, state) in [
        (SUBJECT, DetailValueState::Omitted),
        (OTHER_SUBJECT, DetailValueState::Oversized),
    ] {
        body(&store, "a", subject, DetailValue { state, text: None }).await;
        let snapshot = read(&store, "a", subject, DetailFacet::Body).await;
        assert_eq!(snapshot.body, DetailValue { state, text: None });
        assert_eq!(snapshot.evidence.availability, DetailAvailability::Partial);
        assert_eq!(snapshot.evidence.freshness, DetailFreshness::Unknown);
        assert!(snapshot.evidence.coverage.validated_at.is_none());
        for authoritative in [known(Some("")), known(None)] {
            body(&store, "a", subject, authoritative.clone()).await;
            let snapshot = read(&store, "a", subject, DetailFacet::Body).await;
            assert_eq!(snapshot.body, authoritative);
            assert_eq!(snapshot.evidence.observed_state, DetailValueState::Known);
            assert_eq!(snapshot.evidence.availability, DetailAvailability::Ready);
            assert_eq!(snapshot.evidence.coverage.state, CoverageState::Complete);
        }
    }
}

#[tokio::test]
async fn only_a_complete_empty_traversal_removes_that_facets_entries() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path().join("db")).await.unwrap();
    seed(&store, "a", "github.com").await;
    body(&store, "a", SUBJECT, known(Some("independent body"))).await;
    collection(
        &store,
        "a",
        DetailFacet::Comments,
        vec![entry("c", "saved comment")],
    )
    .await;
    collection(
        &store,
        "a",
        DetailFacet::Reviews,
        vec![entry("r", "saved review")],
    )
    .await;
    let mut partial = observation(&store, "a", SUBJECT, DetailFacet::Comments).await;
    partial.complete = false;
    store.apply_detail(partial).await.unwrap();
    let snapshot = read(&store, "a", SUBJECT, DetailFacet::Comments).await;
    assert_eq!(snapshot.evidence.coverage.state, CoverageState::Partial);
    assert_eq!(snapshot.entries.len(), 1);
    collection(&store, "a", DetailFacet::Comments, vec![]).await;
    let empty = read(&store, "a", SUBJECT, DetailFacet::Comments).await;
    assert!(empty.entries.is_empty());
    assert_eq!(empty.evidence.coverage.state, CoverageState::Complete);
    assert_eq!(empty.evidence.availability, DetailAvailability::Ready);
    assert_eq!(
        read(&store, "a", SUBJECT, DetailFacet::Reviews)
            .await
            .entries[0]
            .body,
        known(Some("saved review"))
    );
    assert_eq!(
        read(&store, "a", SUBJECT, DetailFacet::Body).await.body,
        known(Some("independent body"))
    );
}

#[tokio::test]
async fn older_comparable_facet_time_is_rejected_without_using_parent_time_or_lexical_order() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path().join("db")).await.unwrap();
    seed(&store, "a", "github.com").await;
    let mut initial = observation(&store, "a", SUBJECT, DetailFacet::Body).await;
    initial.body = known(Some("newer facet body"));
    initial.source.provider_updated_at = Some(T0.into());
    store.apply_detail(initial).await.unwrap();
    let expected = read(&store, "a", SUBJECT, DetailFacet::Body).await;
    let mut older = observation(&store, "a", SUBJECT, DetailFacet::Body).await;
    older.body = known(Some("older facet body"));
    older.source.observed_at = T2.into();
    // Its lexical prefix is larger but its actual instant is earlier than T0.
    older.source.provider_updated_at = Some("2025-01-01T02:30:00+03:00".into());
    let revision = store.revision().await.unwrap();
    assert_eq!(
        store.apply_detail(older).await.unwrap_err().code,
        ErrorCode::StaleView
    );
    let actual = read(&store, "a", SUBJECT, DetailFacet::Body).await;
    assert_eq!(actual.body, expected.body);
    assert_eq!(
        actual.evidence.facet_revision,
        expected.evidence.facet_revision
    );
    assert_eq!(actual.evidence.source, expected.evidence.source);
    assert_eq!(actual.evidence.coverage, expected.evidence.coverage);
    assert_eq!(actual.evidence.stale_at, expected.evidence.stale_at);
    assert_eq!(store.revision().await.unwrap(), revision);
}

#[tokio::test]
async fn incomparable_sources_update_only_observed_fields_and_preserve_saved_field_truth() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path().join("db")).await.unwrap();
    seed(&store, "a", "github.com").await;
    let mut initial = observation(&store, "a", SUBJECT, DetailFacet::Reviews).await;
    initial.source.field_mask = entry("r", "saved review body").field_mask;
    initial.source.provider_updated_at = Some(T2.into());
    initial.entries = vec![entry("r", "saved review body")];
    store.apply_detail(initial).await.unwrap();
    let saved = read(&store, "a", SUBJECT, DetailFacet::Reviews)
        .await
        .entries
        .into_iter()
        .next()
        .expect("The fixture has one saved review");
    for (endpoint, version, provider_time) in [
        ("another-review-endpoint", 1, Some(T0)),
        ("another-review-endpoint", 2, Some(T0)),
        ("another-review-endpoint", 2, None),
    ] {
        let mut page = observation(&store, "a", SUBJECT, DetailFacet::Reviews).await;
        page.source.source = endpoint.into();
        page.source.adapter_version = version;
        page.source.provider_updated_at = provider_time.map(str::to_owned);
        page.source.observed_at = T1.into();
        page.source.field_mask = vec![DetailField::State];
        let mut state_only = entry("r", "unobserved body must not replace saved text");
        state_only.state = Some("future-provider-state".into());
        state_only.author = None;
        state_only.title = None;
        state_only.updated_at = None;
        state_only.head_oid = None;
        state_only.field_mask = vec![DetailField::State];
        page.entries = vec![state_only];
        store.apply_detail(page).await.unwrap();
        let actual = read(&store, "a", SUBJECT, DetailFacet::Reviews)
            .await
            .entries
            .into_iter()
            .next()
            .expect("The saved review remains available");
        assert_eq!(actual.state.as_deref(), Some("future-provider-state"));
        assert_eq!(actual.body, saved.body);
        assert_eq!(actual.author, saved.author);
        assert_eq!(actual.title, saved.title);
        assert_eq!(actual.updated_at, saved.updated_at);
        assert_eq!(actual.head_oid, saved.head_oid);
        for field in [
            DetailField::Body,
            DetailField::Author,
            DetailField::Title,
            DetailField::UpdatedAt,
            DetailField::HeadOid,
        ] {
            assert_eq!(
                actual.field_validations.iter().find(|v| v.field == field),
                saved.field_validations.iter().find(|v| v.field == field)
            );
        }
    }
}

#[tokio::test]
async fn detail_page_cursors_cannot_be_reused_across_account_subject_or_facet() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path().join("db")).await.unwrap();
    seed(&store, "a", "github.com").await;
    seed(&store, "b", "github.com").await;
    collection(
        &store,
        "a",
        DetailFacet::Comments,
        vec![entry("c1", "first"), entry("c2", "second")],
    )
    .await;
    let cursor = store
        .detail(DetailQuery {
            account_id: "a".into(),
            subject_id: SUBJECT.into(),
            facet: DetailFacet::Comments,
            cursor: None,
            limit: 1,
        })
        .await
        .unwrap()
        .next_cursor
        .unwrap();
    let before = store.revision().await.unwrap();
    for (account, subject, facet) in [
        ("b", SUBJECT, DetailFacet::Comments),
        ("a", OTHER_SUBJECT, DetailFacet::Comments),
        ("a", SUBJECT, DetailFacet::Reviews),
    ] {
        let error = store
            .detail(DetailQuery {
                account_id: account.into(),
                subject_id: subject.into(),
                facet,
                cursor: Some(cursor.clone()),
                limit: 1,
            })
            .await
            .unwrap_err();
        assert_eq!(error.code, ErrorCode::StaleView);
    }
    assert_eq!(store.revision().await.unwrap(), before);
}
