//! Independent current-inbox provenance tests. Synthetic local databases only;
//! no runtime, HTTP, credentials, personal configuration or provider writes.
use collaboration::{
    providers::{NotificationSubjectDiscovery, ProviderProfile},
    *,
};
use sqlx::{Connection, SqliteConnection, sqlite::SqliteConnectOptions};
use std::{
    path::Path,
    sync::atomic::{AtomicUsize, Ordering},
};

const T1: &str = "2025-01-01T01:00:00Z";
const T2: &str = "2025-01-01T02:00:00Z";
const T3: &str = "2025-01-01T03:00:00Z";
const REPO: &str = "repo";
const NOTIFICATION: &str = "notification";
const SUBJECT: &str = "pull-67";

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
fn repository(actor: &str, id: &str, native: &str, path: &str) -> RemoteRepository {
    RemoteRepository {
        id: id.into(),
        account_id: actor.into(),
        provider_id: native.into(),
        full_name: path.into(),
        name: path.rsplit('/').next().unwrap().into(),
        web_url: format!("https://github.com/{path}"),
        description: None,
        default_branch: None,
        selected: false,
    }
}
fn selector(number: &str) -> NotificationSubjectMapping {
    NotificationSubjectMapping::Selector(NotificationSubjectSelector {
        kind: NotificationSubjectKind::PullRequest,
        repository_provider_id: "42".into(),
        number: number.into(),
        repository_path: "fixture/project".into(),
        representation: NotificationSubjectRepresentation::GithubPullRequest,
    })
}
fn notification(actor: &str, id: &str, at: &str) -> RemoteItem {
    RemoteItem {
        id: id.into(),
        account_id: actor.into(),
        repository_id: Some(REPO.into()),
        provider_id: format!("thread-{id}"),
        kind: RemoteItemKind::Notification,
        number: None,
        title: "Synthetic notification".into(),
        body: None,
        body_omitted: true,
        author: None,
        web_url: Some("https://github.com/fixture/project".into()),
        state: "PullRequest".into(),
        updated_at: at.into(),
        head_oid: None,
        is_draft: None,
        reason: Some("review_requested".into()),
        unread: Some(true),
    }
}
fn subject(actor: &str, id: &str, native: &str, number: &str) -> RemoteItem {
    RemoteItem {
        id: id.into(),
        account_id: actor.into(),
        repository_id: Some(REPO.into()),
        provider_id: native.into(),
        kind: RemoteItemKind::PullRequest,
        number: Some(number.into()),
        title: format!("Private subject {actor}"),
        body: Some("Saved summary".into()),
        body_omitted: false,
        author: None,
        web_url: Some(format!("https://github.com/fixture/project/pull/{number}")),
        state: "open".into(),
        updated_at: T2.into(),
        head_oid: None,
        is_draft: Some(false),
        reason: None,
        unread: None,
    }
}
async fn page(
    store: &Store,
    actor: &str,
    scope: &str,
    repos: Vec<RemoteRepository>,
    items: Vec<RemoteItem>,
    observations: Vec<NotificationSubjectObservation>,
) {
    let epoch = store.account(actor).await.unwrap().authorization_epoch;
    let run = store.begin_sync(actor, &epoch, scope).await.unwrap();
    store
        .apply_page_with_notification_subjects(
            PageCommit {
                account_id: actor.into(),
                authorization_epoch: epoch,
                scope: scope.into(),
                run_id: run,
                repositories: repos,
                items,
                endpoint_aliases: vec![],
                next_cursor: None,
                etag: Some(format!("etag-{scope}")),
                last_modified: None,
                not_modified: false,
                complete: true,
                observed_at: T3.into(),
            },
            observations,
        )
        .await
        .unwrap();
}
async fn inbox(store: &Store, actor: &str, observations: Vec<(&str, &str, &str)>) {
    page(
        store,
        actor,
        "notifications",
        vec![],
        observations
            .iter()
            .map(|(id, _, at)| notification(actor, id, at))
            .collect(),
        observations
            .iter()
            .map(|(id, number, _)| NotificationSubjectObservation {
                notification_id: (*id).into(),
                mapping: selector(number),
            })
            .collect(),
    )
    .await;
}
async fn seed_actor(store: &Store, actor: &str, host: &str) {
    store.upsert_account(account(actor, host)).await.unwrap();
    page(
        store,
        actor,
        "repositories",
        vec![repository(actor, REPO, "42", "fixture/project")],
        vec![],
        vec![],
    )
    .await;
    inbox(store, actor, vec![(NOTIFICATION, "67", T1)]).await;
}
async fn fixture() -> (tempfile::TempDir, Store) {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path().join("fixture.sqlite"))
        .await
        .unwrap();
    seed_actor(&store, "a", "github.com").await;
    (dir, store)
}
fn query(actor: &str, epoch: &str, id: &str) -> NotificationSubjectQuery {
    NotificationSubjectQuery {
        account_id: actor.into(),
        authorization_epoch: epoch.into(),
        notification_id: id.into(),
    }
}
async fn resolve(store: &Store, actor: &str, id: &str) -> NotificationSubjectSnapshot {
    let epoch = store.account(actor).await.unwrap().authorization_epoch;
    store
        .notification_subject(query(actor, &epoch, id), |_, _, _| {
            CapabilityState::Supported
        })
        .await
        .unwrap()
}
async fn request(store: &Store, actor: &str, id: &str) -> DiscoverNotificationSubjectRequest {
    let snapshot = resolve(store, actor, id).await;
    DiscoverNotificationSubjectRequest {
        account_id: actor.into(),
        authorization_epoch: snapshot.authorization_epoch,
        notification_id: id.into(),
        selector_generation: snapshot.selector_generation.unwrap(),
    }
}
async fn begin(store: &Store, actor: &str, id: &str) -> NotificationDiscoveryLease {
    store
        .request_notification_subject_checked(&request(store, actor, id).await, || Ok(()))
        .await
        .unwrap();
    let intent = store
        .pending_notification_subjects()
        .await
        .unwrap()
        .into_iter()
        .find(|i| i.account_id == actor && i.notification_id == id)
        .unwrap();
    store.begin_notification_subject(&intent).await.unwrap()
}
fn verified(
    lease: &NotificationDiscoveryLease,
    id: &str,
    native: &str,
) -> NotificationSubjectDiscovery {
    let actor = &lease.request.account.id;
    let subject = subject(actor, id, native, &lease.request.selector.number);
    let source = DetailSource {
        source: "fixture/pull-details".into(),
        adapter_version: 1,
        field_mask: vec![DetailField::Body],
        provider_updated_at: Some(T2.into()),
        observed_at: T2.into(),
    };
    let metadata = ResourceMetadataObservation {
        kind: RemoteItemKind::PullRequest,
        values: ResourceMetadataValues {
            title: Some(subject.title.clone()),
            state: Some(subject.state.clone()),
            updated_at: Some(T2.into()),
            ..Default::default()
        },
        fields: vec![
            MetadataObservedField {
                field: MetadataField::Title,
                state: DetailValueState::Known,
            },
            MetadataObservedField {
                field: MetadataField::State,
                state: DetailValueState::Known,
            },
            MetadataObservedField {
                field: MetadataField::UpdatedAt,
                state: DetailValueState::Known,
            },
        ],
        source: MetadataSource {
            source: source.source.clone(),
            adapter_version: 1,
            provider_updated_at: Some(T2.into()),
            observed_at: T2.into(),
        },
    };
    NotificationSubjectDiscovery::Verified {
        subject: Box::new(subject),
        detail: Box::new(collaboration::providers::DetailPage {
            body: DetailValue {
                state: DetailValueState::Known,
                text: Some(format!("Private detail {actor}")),
            },
            metadata: Some(metadata),
            entries: vec![],
            source,
            next_cursor: None,
            etag: Some("body-etag".into()),
            not_modified: false,
            freshness_seconds: 600,
            cooldown_seconds: None,
        }),
        endpoint_aliases: vec![],
    }
}
async fn discover(store: &Store, actor: &str, id: &str, subject_id: &str, native: &str) {
    let lease = begin(store, actor, id).await;
    store
        .apply_notification_subject(&lease, verified(&lease, subject_id, native))
        .await
        .unwrap();
}
async fn detail(store: &Store, actor: &str, id: &str) -> DetailSnapshot {
    store
        .detail(DetailQuery {
            account_id: actor.into(),
            subject_id: id.into(),
            facet: DetailFacet::Body,
            cursor: None,
            limit: 20,
        })
        .await
        .unwrap()
}
fn item_query(actor: &str, search: Option<&str>) -> ItemQuery {
    ItemQuery {
        account_id: actor.into(),
        kind: RemoteItemKind::PullRequest,
        repository_id: None,
        state: None,
        search: search.map(str::to_owned),
        cursor: None,
        limit: 100,
    }
}
async fn context(
    store: &Store,
    actor: &str,
    id: &str,
    instance: &str,
) -> ContextualCapabilitySnapshot {
    let epoch = store.account(actor).await.unwrap().authorization_epoch;
    store
        .contextual_capabilities(
            ContextCapabilityRequest {
                account_id: actor.into(),
                authorization_epoch: epoch,
                target: CapabilityTarget {
                    kind: CapabilityTargetKind::Resource,
                    instance_id: Some(instance.into()),
                    repository_id: None,
                    resource_id: Some(id.into()),
                    resource_kind: Some(ResourceKind::PullRequest),
                },
            },
            detail_profile,
        )
        .await
        .unwrap()
}
fn detail_profile(_: &RemoteAccount, _: &ProviderInstance) -> ProviderProfile {
    let mut profile = ProviderProfile::read_only(InboxSemantics::NativeNotifications, true);
    for facet in &mut profile.facets {
        if matches!(
            facet.facet,
            ResourceFacet::PullDetails | ResourceFacet::IssueDetails
        ) {
            facet.state = CapabilityState::Supported;
            facet.reason = None;
        }
    }
    profile
}

fn body_capability(snapshot: &ContextualCapabilitySnapshot) -> &ContextFacetCapability {
    snapshot
        .facets
        .iter()
        .find(|f| f.facet == ResourceFacet::PullDetails)
        .unwrap()
}
async fn draft(store: &Store, actor: &str, id: &str) -> LocalDraft {
    store
        .save_draft(LocalDraft {
            account_id: actor.into(),
            subject_id: id.into(),
            body: format!("Authored draft {actor}"),
            generation: "0".into(),
        })
        .await
        .unwrap()
}
async fn deny(store: &Store, actor: &str, scope: &str) {
    let epoch = store.account(actor).await.unwrap().authorization_epoch;
    store
        .set_sync_status(
            actor,
            &epoch,
            scope,
            SyncStatus {
                state: SyncState::Error,
                last_success_at: None,
                next_retry_at: None,
                error: Some(CollaborationError::new(
                    ErrorCode::PermissionDenied,
                    "Synthetic denial",
                )),
            },
        )
        .await
        .unwrap();
}
async fn body_commit(store: &Store, actor: &str, id: &str) -> DetailCommit {
    let epoch = store.account(actor).await.unwrap().authorization_epoch;
    let lease = store
        .begin_detail(actor, &epoch, id, DetailFacet::Body)
        .await
        .unwrap();
    DetailCommit {
        account_id: actor.into(),
        authorization_epoch: epoch,
        authorization_view: lease.authorization_view,
        instance_id: lease.instance_id,
        subject_id: id.into(),
        facet: DetailFacet::Body,
        run_id: lease.run_id,
        request_cursor: None,
        body: DetailValue {
            state: DetailValueState::Known,
            text: Some("Late response".into()),
        },
        metadata: None,
        subject_binding: None,
        entries: vec![],
        source: DetailSource {
            source: "fixture/pull-details".into(),
            adapter_version: 1,
            field_mask: vec![DetailField::Body],
            provider_updated_at: Some(T3.into()),
            observed_at: T3.into(),
        },
        next_cursor: None,
        etag: None,
        not_modified: false,
        whole_scope: true,
        complete: true,
        freshness_seconds: 600,
    }
}
#[derive(Debug, PartialEq, Eq)]
struct FeedFootprint {
    repositories: Vec<String>,
    membership: Vec<String>,
    scopes: Vec<String>,
}
/// Read-only SQL evidence supplements public queries to prove no hidden feed
/// checkpoint/membership/selection mutation by the atomic point transaction.
async fn feed_footprint(path: &Path, actor: &str) -> FeedFootprint {
    let mut db =
        SqliteConnection::connect_with(&SqliteConnectOptions::new().filename(path).read_only(true))
            .await
            .unwrap();
    let repositories=sqlx::query_scalar("SELECT json_object('id',id,'native',provider_id,'selected',selected,'json',json) FROM repositories WHERE account_id=? ORDER BY id").bind(actor).fetch_all(&mut db).await.unwrap();
    let membership=sqlx::query_scalar("SELECT json_object('scope',scope,'entity',entity_id,'seen',last_seen_run,'active',active,'missing',missing_count) FROM scope_membership WHERE account_id=? ORDER BY scope,entity_id").bind(actor).fetch_all(&mut db).await.unwrap();
    let scopes=sqlx::query_scalar("SELECT json_object('scope',scope,'run',run_id,'data',data_revision,'complete',completed_run_id,'cursor',next_cursor,'etag',etag,'modified',last_modified,'denied',access_denied,'coverage',coverage_json,'sync',sync_json) FROM sync_scopes WHERE account_id=? AND (scope IN ('repositories','notifications') OR scope LIKE 'repo:%') ORDER BY scope").bind(actor).fetch_all(&mut db).await.unwrap();
    db.close().await.unwrap();
    FeedFootprint {
        repositories,
        membership,
        scopes,
    }
}

#[tokio::test]
async fn unselected_subject_has_narrow_local_access_without_feed_truth_or_query_intents() {
    let (dir, store) = fixture().await;
    let before = feed_footprint(&dir.path().join("fixture.sqlite"), "a").await;
    let empty = store.query_items(item_query("a", None)).await.unwrap();
    assert!(empty.items.is_empty());
    let rev = store.revision().await.unwrap();
    for _ in 0..3 {
        let snapshot = resolve(&store, "a", NOTIFICATION).await;
        assert_eq!(snapshot.state, NotificationSubjectState::NotCached);
        assert!(snapshot.subject.is_none());
    }
    assert_eq!(store.revision().await.unwrap(), rev);
    assert!(
        store
            .pending_notification_subjects()
            .await
            .unwrap()
            .is_empty()
    );
    discover(&store, "a", NOTIFICATION, SUBJECT, "9007199254740995").await;
    let snapshot = resolve(&store, "a", NOTIFICATION).await;
    assert_eq!(snapshot.state, NotificationSubjectState::Resolved);
    let canonical = snapshot.subject.unwrap();
    assert_eq!(canonical.id, SUBJECT);
    assert_eq!(canonical.provider_id, "9007199254740995");
    assert_eq!(
        store.item("a", SUBJECT).await.unwrap().item.unwrap().title,
        "Private subject a"
    );
    let saved = detail(&store, "a", SUBJECT).await;
    assert_eq!(saved.body.text.as_deref(), Some("Private detail a"));
    assert_eq!(saved.evidence.availability, DetailAvailability::Ready);
    assert_eq!(
        body_capability(&context(&store, "a", SUBJECT, &canonical.instance_id).await)
            .saved_read
            .state,
        CapabilityState::Supported
    );
    assert!(
        store
            .query_items(item_query("a", Some("Private")))
            .await
            .unwrap()
            .items
            .is_empty()
    );
    assert_eq!(
        store
            .query_items(item_query("a", None))
            .await
            .unwrap()
            .coverage,
        empty.coverage
    );
    assert_eq!(
        feed_footprint(&dir.path().join("fixture.sqlite"), "a").await,
        before
    );
    assert!(
        !store
            .repositories("a")
            .await
            .unwrap()
            .repositories
            .iter()
            .find(|r| r.id == REPO)
            .unwrap()
            .selected
    );
    assert!(
        store
            .pending_notification_subjects()
            .await
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn completed_inbox_retirement_withdraws_only_provider_access_and_fences_body_response() {
    let (_dir, store) = fixture().await;
    discover(&store, "a", NOTIFICATION, SUBJECT, "9001").await;
    let authored = draft(&store, "a", SUBJECT).await;
    let late = body_commit(&store, "a", SUBJECT).await;
    let instance = resolve(&store, "a", NOTIFICATION)
        .await
        .subject
        .unwrap()
        .instance_id;
    inbox(&store, "a", vec![]).await;
    assert!(
        store.item("a", SUBJECT).await.unwrap().item.is_some(),
        "one missing traversal is not authoritative withdrawal"
    );
    inbox(&store, "a", vec![]).await;
    let snapshot = resolve(&store, "a", NOTIFICATION).await;
    assert_eq!(
        snapshot.reason,
        Some(NotificationSubjectReason::InactiveMembership)
    );
    assert!(snapshot.subject.is_none());
    assert!(store.item("a", SUBJECT).await.unwrap().item.is_none());
    let hidden = detail(&store, "a", SUBJECT).await;
    assert_eq!(
        hidden.evidence.availability,
        DetailAvailability::Unavailable
    );
    assert!(hidden.body.text.is_none() && hidden.metadata.is_none());
    assert_eq!(
        body_capability(&context(&store, "a", SUBJECT, &instance).await)
            .saved_read
            .state,
        CapabilityState::Unavailable
    );
    assert!(matches!(
        store.apply_detail(late.clone()).await.unwrap_err().code,
        ErrorCode::StaleView | ErrorCode::PermissionDenied
    ));
    assert_eq!(
        store.draft("a", SUBJECT).await.unwrap(),
        Some(authored.clone())
    );
    inbox(&store, "a", vec![(NOTIFICATION, "67", T2)]).await;
    assert_eq!(
        resolve(&store, "a", NOTIFICATION).await.state,
        NotificationSubjectState::Resolved
    );
    assert_eq!(
        detail(&store, "a", SUBJECT).await.body.text.as_deref(),
        Some("Private detail a")
    );
    assert_eq!(
        store.apply_detail(late).await.unwrap_err().code,
        ErrorCode::StaleView
    );
    assert_eq!(store.draft("a", SUBJECT).await.unwrap(), Some(authored));
}

#[tokio::test]
async fn union_of_current_notifications_survives_one_selector_change_then_withdraws_last() {
    let (_dir, store) = fixture().await;
    inbox(
        &store,
        "a",
        vec![(NOTIFICATION, "67", T1), ("other-notification", "67", T1)],
    )
    .await;
    discover(&store, "a", NOTIFICATION, SUBJECT, "9001").await;
    let authored = draft(&store, "a", SUBJECT).await;
    inbox(
        &store,
        "a",
        vec![(NOTIFICATION, "68", T2), ("other-notification", "67", T2)],
    )
    .await;
    assert_eq!(
        resolve(&store, "a", NOTIFICATION).await.state,
        NotificationSubjectState::NotCached
    );
    assert_eq!(
        resolve(&store, "a", "other-notification").await.state,
        NotificationSubjectState::Resolved
    );
    assert!(store.item("a", SUBJECT).await.unwrap().item.is_some());
    inbox(
        &store,
        "a",
        vec![(NOTIFICATION, "68", T3), ("other-notification", "68", T3)],
    )
    .await;
    assert!(store.item("a", SUBJECT).await.unwrap().item.is_none());
    assert_eq!(
        detail(&store, "a", SUBJECT).await.evidence.availability,
        DetailAvailability::Unavailable
    );
    assert_eq!(store.draft("a", SUBJECT).await.unwrap(), Some(authored));
}

#[tokio::test]
async fn selector_replacement_rejects_late_point_success_and_error_without_changing_new_intent() {
    let (_dir, store) = fixture().await;
    let lease = begin(&store, "a", NOTIFICATION).await;
    inbox(&store, "a", vec![(NOTIFICATION, "68", T2)]).await;
    let fresh = begin(&store, "a", NOTIFICATION).await;
    let pending = store.pending_notification_subjects().await.unwrap();
    let revision = store.revision().await.unwrap();
    assert_eq!(
        store
            .apply_notification_subject(&lease, verified(&lease, SUBJECT, "9001"))
            .await
            .unwrap_err()
            .code,
        ErrorCode::StaleView
    );
    assert_eq!(
        store
            .fail_notification_subject(
                &lease,
                CollaborationError::new(ErrorCode::PermissionDenied, "Late denial"),
                None
            )
            .await
            .unwrap_err()
            .code,
        ErrorCode::StaleView
    );
    assert_eq!(store.revision().await.unwrap(), revision);
    let after = store.pending_notification_subjects().await.unwrap();
    assert_eq!(after.len(), pending.len());
    assert_eq!(
        after.first().unwrap().intent_generation,
        pending.first().unwrap().intent_generation
    );
    assert!(store.item("a", SUBJECT).await.unwrap().item.is_none());
    store
        .apply_notification_subject(&fresh, verified(&fresh, "pull-68", "9002"))
        .await
        .unwrap();
    assert_eq!(
        resolve(&store, "a", NOTIFICATION).await.subject.unwrap().id,
        "pull-68"
    );
}

#[tokio::test]
async fn older_notification_representation_cannot_rollback_selector_authority() {
    let (_dir, store) = fixture().await;
    inbox(&store, "a", vec![(NOTIFICATION, "67", T2)]).await;
    discover(&store, "a", NOTIFICATION, SUBJECT, "9001").await;
    let previous = resolve(&store, "a", NOTIFICATION).await;
    inbox(&store, "a", vec![(NOTIFICATION, "68", T1)]).await;
    let current = resolve(&store, "a", NOTIFICATION).await;
    assert_eq!(
        current.selector_generation, previous.selector_generation,
        "an older rejected notification must not publish newer-looking selector authority"
    );
    assert_eq!(current.state, NotificationSubjectState::Resolved);
    assert_eq!(current.subject, previous.subject);
    assert!(store.item("a", SUBJECT).await.unwrap().item.is_some());
}

#[tokio::test]
async fn hidden_competing_immutable_claims_are_ambiguous_before_any_visibility_selection() {
    let (_dir, store) = fixture().await;
    store.select_repository("a", REPO, true).await.unwrap();
    page(
        &store,
        "a",
        "repo:repo:pull_request",
        vec![],
        vec![
            subject("a", SUBJECT, "9001", "67"),
            subject("a", "hidden-claim", "9002", "67"),
        ],
        vec![],
    )
    .await;
    store.select_repository("a", REPO, false).await.unwrap();
    let snapshot = resolve(&store, "a", NOTIFICATION).await;
    assert_eq!(snapshot.state, NotificationSubjectState::Ambiguous);
    assert_eq!(
        snapshot.reason,
        Some(NotificationSubjectReason::AmbiguousIdentity)
    );
    assert!(snapshot.subject.is_none() && !snapshot.discovery.admission);
    assert!(store.item("a", SUBJECT).await.unwrap().item.is_none());
    assert!(
        store
            .item("a", "hidden-claim")
            .await
            .unwrap()
            .item
            .is_none()
    );
    assert_eq!(
        detail(&store, "a", SUBJECT).await.evidence.availability,
        DetailAvailability::Unavailable
    );
    assert_eq!(
        store
            .request_notification_subject_checked(&request(&store, "a", NOTIFICATION).await, || Ok(
                ()
            ))
            .await
            .unwrap_err()
            .code,
        ErrorCode::Unsupported
    );
    assert!(
        store
            .pending_notification_subjects()
            .await
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn authoritative_rename_preserves_immutable_resolution_and_uses_current_request_coordinates()
{
    let (_dir, store) = fixture().await;
    discover(&store, "a", NOTIFICATION, SUBJECT, "9001").await;
    inbox(
        &store,
        "a",
        vec![(NOTIFICATION, "67", T2), ("missing-target", "68", T2)],
    )
    .await;
    let previous = resolve(&store, "a", NOTIFICATION).await;
    page(
        &store,
        "a",
        "repositories",
        vec![repository("a", REPO, "42", "renamed/project")],
        vec![],
        vec![],
    )
    .await;
    let current = resolve(&store, "a", NOTIFICATION).await;
    assert_eq!(current.state, NotificationSubjectState::Resolved);
    assert_eq!(current.subject, previous.subject);
    assert_eq!(
        current.fallback_web_url.as_deref(),
        Some("https://github.com/renamed/project/pull/67")
    );
    let lease = begin(&store, "a", "missing-target").await;
    assert_eq!(lease.request.repository.provider_id, "42");
    assert_eq!(lease.request.repository.full_name, "renamed/project");
    assert_eq!(lease.request.selector.repository_path, "renamed/project");
    assert_eq!(lease.request.selector.repository_provider_id, "42");
}

#[tokio::test]
async fn path_reuse_with_a_different_immutable_parent_never_crosses_current_provenance() {
    let (_dir, store) = fixture().await;
    discover(&store, "a", NOTIFICATION, SUBJECT, "9001").await;
    page(
        &store,
        "a",
        "repositories",
        vec![
            repository("a", REPO, "42", "renamed/project"),
            repository("a", "replacement-repo", "43", "fixture/project"),
        ],
        vec![],
        vec![],
    )
    .await;
    store
        .select_repository("a", "replacement-repo", true)
        .await
        .unwrap();
    let mut wrong = subject("a", "replacement-pull", "9010", "67");
    wrong.repository_id = Some("replacement-repo".into());
    page(
        &store,
        "a",
        "repo:replacement-repo:pull_request",
        vec![],
        vec![wrong],
        vec![],
    )
    .await;
    let canonical = resolve(&store, "a", NOTIFICATION).await.subject.unwrap();
    assert_eq!(canonical.id, SUBJECT);
    assert_eq!(canonical.provider_id, "9001");
    assert_eq!(
        detail(&store, "a", SUBJECT).await.body.text.as_deref(),
        Some("Private detail a")
    );
}

#[tokio::test]
async fn account_and_provider_instance_partitions_do_not_share_claims_or_subject_content() {
    let (_dir, store) = fixture().await;
    seed_actor(&store, "b", "github.com").await;
    seed_actor(&store, "c", "git.example.com").await;
    discover(&store, "a", NOTIFICATION, SUBJECT, "9001").await;
    assert_eq!(
        resolve(&store, "b", NOTIFICATION).await.state,
        NotificationSubjectState::NotCached
    );
    assert_eq!(
        resolve(&store, "c", NOTIFICATION).await.state,
        NotificationSubjectState::NotCached
    );
    assert!(store.item("b", SUBJECT).await.unwrap().item.is_none());
    assert!(store.item("c", SUBJECT).await.unwrap().item.is_none());
    discover(&store, "b", NOTIFICATION, SUBJECT, "9001").await;
    discover(&store, "c", NOTIFICATION, SUBJECT, "9001").await;
    let a = resolve(&store, "a", NOTIFICATION).await.subject.unwrap();
    let b = resolve(&store, "b", NOTIFICATION).await.subject.unwrap();
    let c = resolve(&store, "c", NOTIFICATION).await.subject.unwrap();
    assert_eq!(a.instance_id, b.instance_id);
    assert_ne!(a.instance_id, c.instance_id);
    for actor in ["a", "b", "c"] {
        assert_eq!(
            detail(&store, actor, SUBJECT).await.body.text,
            Some(format!("Private detail {actor}"))
        );
    }
    let mut wrong = context_request("c", &a.instance_id, SUBJECT);
    wrong.authorization_epoch = "1".into();
    assert_eq!(
        store
            .contextual_capabilities(wrong, |_, _| ProviderProfile::read_only(
                InboxSemantics::NativeNotifications,
                true
            ))
            .await
            .unwrap_err()
            .code,
        ErrorCode::InvalidInput
    );
}
fn context_request(actor: &str, instance: &str, id: &str) -> ContextCapabilityRequest {
    ContextCapabilityRequest {
        account_id: actor.into(),
        authorization_epoch: "1".into(),
        target: CapabilityTarget {
            kind: CapabilityTargetKind::Resource,
            instance_id: Some(instance.into()),
            repository_id: None,
            resource_id: Some(id.into()),
            resource_kind: Some(ResourceKind::PullRequest),
        },
    }
}

#[tokio::test]
async fn replacement_grant_rejects_old_queries_intents_success_and_failure_but_preserves_drafts() {
    let (_dir, store) = fixture().await;
    seed_actor(&store, "b", "github.com").await;
    discover(&store, "b", NOTIFICATION, SUBJECT, "9001").await;
    let b_before = detail(&store, "b", SUBJECT).await;
    let authored = draft(&store, "a", SUBJECT).await;
    let old_request = request(&store, "a", NOTIFICATION).await;
    let lease = begin(&store, "a", NOTIFICATION).await;
    let mut replaced = store.account("a").await.unwrap();
    replaced.authorization_epoch = "2".into();
    store.upsert_account(replaced).await.unwrap();
    let revision = store.revision().await.unwrap();
    assert_eq!(
        store
            .notification_subject(query("a", "1", NOTIFICATION), |_, _, _| {
                CapabilityState::Supported
            })
            .await
            .unwrap_err()
            .code,
        ErrorCode::StaleView
    );
    assert_eq!(
        store
            .request_notification_subject_checked(&old_request, || Ok(()))
            .await
            .unwrap_err()
            .code,
        ErrorCode::StaleView
    );
    assert_eq!(
        store
            .apply_notification_subject(&lease, verified(&lease, SUBJECT, "9001"))
            .await
            .unwrap_err()
            .code,
        ErrorCode::StaleView
    );
    assert_eq!(
        store
            .fail_notification_subject(
                &lease,
                CollaborationError::new(ErrorCode::AuthRequired, "Old authorization"),
                None
            )
            .await
            .unwrap_err()
            .code,
        ErrorCode::StaleView
    );
    assert_eq!(store.revision().await.unwrap(), revision);
    assert!(
        store
            .pending_notification_subjects()
            .await
            .unwrap()
            .is_empty()
    );
    assert_eq!(store.draft("a", SUBJECT).await.unwrap(), Some(authored));
    assert_eq!(detail(&store, "b", SUBJECT).await.body, b_before.body);
    assert_eq!(
        resolve(&store, "b", NOTIFICATION).await.state,
        NotificationSubjectState::Resolved
    );
}

#[tokio::test]
async fn known_denial_withdraws_narrow_grant_and_preserves_saved_draft_for_every_gate() {
    for scope in [
        "notifications",
        "repositories",
        "repo:repo:pull_request",
        "detail:pull-67:body",
        "notification_subject:notification",
    ] {
        let (_dir, store) = fixture().await;
        discover(&store, "a", NOTIFICATION, SUBJECT, "9001").await;
        let authored = draft(&store, "a", SUBJECT).await;
        let late = body_commit(&store, "a", SUBJECT).await;
        if scope == "repo:repo:pull_request" {
            // This administrative error API intentionally requires selection
            // for feed scopes. The existing grant is still independently denied.
            store.select_repository("a", REPO, true).await.unwrap();
        }
        deny(&store, "a", scope).await;
        let snapshot = resolve(&store, "a", NOTIFICATION).await;
        assert_eq!(
            snapshot.state,
            NotificationSubjectState::Unavailable,
            "{scope}"
        );
        assert_eq!(
            snapshot.reason,
            Some(NotificationSubjectReason::PermissionDenied),
            "{scope}"
        );
        assert!(
            snapshot.subject.is_none()
                && snapshot.fallback_web_url.is_none()
                && snapshot.selector_generation.is_none(),
            "{scope}"
        );
        assert!(!snapshot.discovery.admission, "{scope}");
        assert!(
            store.item("a", SUBJECT).await.unwrap().item.is_none(),
            "{scope}"
        );
        assert_eq!(
            detail(&store, "a", SUBJECT).await.evidence.availability,
            DetailAvailability::Unavailable,
            "{scope}"
        );
        assert!(
            matches!(
                store.apply_detail(late).await.unwrap_err().code,
                ErrorCode::StaleView | ErrorCode::PermissionDenied
            ),
            "{scope}"
        );
        assert_eq!(
            store.draft("a", SUBJECT).await.unwrap(),
            Some(authored),
            "{scope}"
        );
    }
}

#[tokio::test]
async fn transient_offline_and_quota_keep_authorized_saved_reads_and_query_admission_separate() {
    let (_dir, store) = fixture().await;
    discover(&store, "a", NOTIFICATION, SUBJECT, "9001").await;
    store
        .set_sync_status(
            "a",
            "1",
            "notifications",
            SyncStatus {
                state: SyncState::Offline,
                last_success_at: Some(T1.into()),
                next_retry_at: Some("2099-01-01T00:00:00Z".into()),
                error: Some(CollaborationError::new(
                    ErrorCode::Network,
                    "Synthetic offline",
                )),
            },
        )
        .await
        .unwrap();
    store
        .set_sync_status(
            "a",
            "1",
            "provider:rest",
            SyncStatus {
                state: SyncState::RateLimited,
                last_success_at: None,
                next_retry_at: Some("2099-02-01T00:00:00Z".into()),
                error: Some(CollaborationError::new(
                    ErrorCode::RateLimited,
                    "Synthetic quota",
                )),
            },
        )
        .await
        .unwrap();
    let snapshot = resolve(&store, "a", NOTIFICATION).await;
    assert_eq!(snapshot.state, NotificationSubjectState::Resolved);
    assert!(snapshot.discovery.paused);
    assert_eq!(
        snapshot.discovery.retry_at.as_deref(),
        Some("2099-02-01T00:00:00+00:00")
    );
    assert_eq!(
        detail(&store, "a", SUBJECT).await.body.text.as_deref(),
        Some("Private detail a")
    );
    assert!(store.item("a", SUBJECT).await.unwrap().item.is_some());
    assert!(
        store
            .pending_notification_subjects()
            .await
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn current_provenance_and_retained_private_authority_survive_cold_reopen() {
    let (dir, store) = fixture().await;
    discover(&store, "a", NOTIFICATION, SUBJECT, "9001").await;
    let authored = draft(&store, "a", SUBJECT).await;
    let canonical = resolve(&store, "a", NOTIFICATION).await.subject.unwrap();
    store.close().await;
    drop(store);
    let reopened = Store::open(dir.path().join("fixture.sqlite"))
        .await
        .unwrap();
    assert_eq!(
        resolve(&reopened, "a", NOTIFICATION).await.subject,
        Some(canonical)
    );
    assert_eq!(
        detail(&reopened, "a", SUBJECT).await.body.text.as_deref(),
        Some("Private detail a")
    );
    assert_eq!(
        reopened.draft("a", SUBJECT).await.unwrap(),
        Some(authored.clone())
    );
    inbox(&reopened, "a", vec![]).await;
    inbox(&reopened, "a", vec![]).await;
    reopened.close().await;
    drop(reopened);
    let final_store = Store::open(dir.path().join("fixture.sqlite"))
        .await
        .unwrap();
    assert!(final_store.item("a", SUBJECT).await.unwrap().item.is_none());
    assert_eq!(
        detail(&final_store, "a", SUBJECT)
            .await
            .evidence
            .availability,
        DetailAvailability::Unavailable
    );
    assert_eq!(
        final_store.draft("a", SUBJECT).await.unwrap(),
        Some(authored)
    );
}

#[tokio::test]
async fn admission_checks_caller_again_before_commit_and_rolls_back_every_intent_change() {
    let (_dir, store) = fixture().await;
    let request = request(&store, "a", NOTIFICATION).await;
    let revision = store.revision().await.unwrap();
    let calls = AtomicUsize::new(0);
    let error = store
        .request_notification_subject_checked(&request, || {
            if calls.fetch_add(1, Ordering::SeqCst) == 0 {
                Ok(())
            } else {
                Err(CollaborationError::new(
                    ErrorCode::PermissionDenied,
                    "Retired caller",
                ))
            }
        })
        .await
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::PermissionDenied);
    assert_eq!(calls.load(Ordering::SeqCst), 2);
    assert!(
        store
            .pending_notification_subjects()
            .await
            .unwrap()
            .is_empty()
    );
    assert_eq!(store.revision().await.unwrap(), revision);
    assert_eq!(
        resolve(&store, "a", NOTIFICATION).await.discovery.attempts,
        0
    );
    let first = store
        .request_notification_subject_checked(&request, || Ok(()))
        .await
        .unwrap();
    let coalesced = store
        .request_notification_subject_checked(&request, || Ok(()))
        .await
        .unwrap();
    assert_eq!(first, coalesced);
    assert_eq!(
        store.pending_notification_subjects().await.unwrap().len(),
        1
    );
}

#[tokio::test]
async fn invalid_point_detail_rolls_back_summary_identity_and_feed_effects_atomically() {
    let (dir, store) = fixture().await;
    let lease = begin(&store, "a", NOTIFICATION).await;
    let before = feed_footprint(&dir.path().join("fixture.sqlite"), "a").await;
    let revision = store.revision().await.unwrap();
    let mut result = verified(&lease, SUBJECT, "9001");
    if let NotificationSubjectDiscovery::Verified { detail, .. } = &mut result {
        detail.source.field_mask = vec![DetailField::Body, DetailField::Body];
    }
    assert_eq!(
        store
            .apply_notification_subject(&lease, result)
            .await
            .unwrap_err()
            .code,
        ErrorCode::InvalidInput
    );
    assert_eq!(store.revision().await.unwrap(), revision);
    assert_eq!(
        feed_footprint(&dir.path().join("fixture.sqlite"), "a").await,
        before
    );
    assert!(store.item("a", SUBJECT).await.unwrap().item.is_none());
    assert_eq!(
        resolve(&store, "a", NOTIFICATION).await.state,
        NotificationSubjectState::NotCached
    );
    assert_eq!(
        store.pending_notification_subjects().await.unwrap().len(),
        1
    );
    store
        .apply_notification_subject(&lease, verified(&lease, SUBJECT, "9001"))
        .await
        .unwrap();
    assert_eq!(
        resolve(&store, "a", NOTIFICATION).await.state,
        NotificationSubjectState::Resolved
    );
}

#[tokio::test]
async fn retired_notification_rows_cannot_recreate_discovery_or_provider_authority() {
    let (_dir, store) = fixture().await;
    let old_request = request(&store, "a", NOTIFICATION).await;
    let lease = begin(&store, "a", NOTIFICATION).await;
    inbox(&store, "a", vec![]).await;
    inbox(&store, "a", vec![]).await;
    let revision = store.revision().await.unwrap();
    assert_eq!(
        store
            .request_notification_subject_checked(&old_request, || Ok(()))
            .await
            .unwrap_err()
            .code,
        ErrorCode::PermissionDenied
    );
    assert_eq!(
        store
            .apply_notification_subject(&lease, verified(&lease, SUBJECT, "9001"))
            .await
            .unwrap_err()
            .code,
        ErrorCode::StaleView
    );
    assert_eq!(
        store
            .fail_notification_subject(
                &lease,
                CollaborationError::new(ErrorCode::AuthRequired, "Late auth"),
                None
            )
            .await
            .unwrap_err()
            .code,
        ErrorCode::StaleView
    );
    assert_eq!(store.revision().await.unwrap(), revision);
    assert!(
        store
            .pending_notification_subjects()
            .await
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        store.account("a").await.unwrap().state,
        AccountState::Active
    );
}

#[tokio::test]
async fn rediscovery_retains_existing_canonical_id_and_inspected_private_draft_generation() {
    let (_dir, store) = fixture().await;
    store.select_repository("a", REPO, true).await.unwrap();
    page(
        &store,
        "a",
        "repo:repo:pull_request",
        vec![],
        vec![subject("a", "authored-canonical-id", "9001", "67")],
        vec![],
    )
    .await;
    let authored = draft(&store, "a", "authored-canonical-id").await;
    let mut replacement = store.account("a").await.unwrap();
    replacement.authorization_epoch = "2".into();
    store.upsert_account(replacement).await.unwrap();
    page(
        &store,
        "a",
        "repositories",
        vec![repository("a", REPO, "42", "fixture/project")],
        vec![],
        vec![],
    )
    .await;
    store.select_repository("a", REPO, false).await.unwrap();
    inbox(&store, "a", vec![(NOTIFICATION, "67", T2)]).await;
    assert_eq!(
        resolve(&store, "a", NOTIFICATION).await.state,
        NotificationSubjectState::NotCached
    );
    let lease = begin(&store, "a", NOTIFICATION).await;
    assert_eq!(
        lease.expected_claims.first().unwrap().id,
        "authored-canonical-id"
    );
    // The adapter can only supply its fresh normalized ID; native identity
    // authority chooses the preexisting canonical ID in the point transaction.
    store
        .apply_notification_subject(&lease, verified(&lease, SUBJECT, "9001"))
        .await
        .unwrap();
    assert_eq!(
        resolve(&store, "a", NOTIFICATION).await.subject.unwrap().id,
        "authored-canonical-id"
    );
    assert!(store.item("a", SUBJECT).await.unwrap().item.is_none());
    assert_eq!(
        detail(&store, "a", "authored-canonical-id")
            .await
            .body
            .text
            .as_deref(),
        Some("Private detail a")
    );
    assert_eq!(
        store.draft("a", "authored-canonical-id").await.unwrap(),
        Some(authored.clone())
    );
    assert!(store.draft("a", SUBJECT).await.unwrap().is_none());
    assert_eq!(
        store
            .save_draft(LocalDraft {
                generation: "0".into(),
                ..authored
            })
            .await
            .unwrap_err()
            .code,
        ErrorCode::StaleView
    );
}

#[tokio::test]
async fn issue_and_pull_same_number_have_independent_typed_canonical_provenance() {
    let (_dir, store) = fixture().await;
    store.select_repository("a", REPO, true).await.unwrap();
    page(
        &store,
        "a",
        "repo:repo:pull_request",
        vec![],
        vec![subject("a", SUBJECT, "9001", "67")],
        vec![],
    )
    .await;
    store.select_repository("a", REPO, false).await.unwrap();
    let mapping = NotificationSubjectMapping::Selector(NotificationSubjectSelector {
        kind: NotificationSubjectKind::Issue,
        repository_provider_id: "42".into(),
        number: "67".into(),
        repository_path: "fixture/project".into(),
        representation: NotificationSubjectRepresentation::GithubIssue,
    });
    page(
        &store,
        "a",
        "notifications",
        vec![],
        vec![notification("a", NOTIFICATION, T2)],
        vec![NotificationSubjectObservation {
            notification_id: NOTIFICATION.into(),
            mapping,
        }],
    )
    .await;
    assert_eq!(
        resolve(&store, "a", NOTIFICATION).await.state,
        NotificationSubjectState::NotCached
    );
    assert!(store.item("a", SUBJECT).await.unwrap().item.is_none());
    let lease = begin(&store, "a", NOTIFICATION).await;
    let mut receipt = verified(&lease, "issue-67", "9002");
    if let NotificationSubjectDiscovery::Verified {
        subject, detail, ..
    } = &mut receipt
    {
        subject.kind = RemoteItemKind::Issue;
        subject.is_draft = None;
        subject.web_url = Some("https://github.com/fixture/project/issues/67".into());
        detail.metadata.as_mut().unwrap().kind = RemoteItemKind::Issue;
    }
    store
        .apply_notification_subject(&lease, receipt)
        .await
        .unwrap();
    let canonical = resolve(&store, "a", NOTIFICATION).await.subject.unwrap();
    assert_eq!(canonical.id, "issue-67");
    assert_eq!(canonical.kind, ResourceKind::Issue);
    assert_eq!(canonical.provider_id, "9002");
    assert!(store.item("a", SUBJECT).await.unwrap().item.is_none());
    assert_eq!(
        detail(&store, "a", "issue-67").await.body.text.as_deref(),
        Some("Private detail a")
    );
    assert!(
        store
            .query_items(ItemQuery {
                kind: RemoteItemKind::Issue,
                ..item_query("a", None)
            })
            .await
            .unwrap()
            .items
            .is_empty()
    );
}

#[tokio::test]
async fn caller_retired_while_waiting_for_writer_is_checked_before_durable_admission() {
    use std::sync::{Arc, Condvar, Mutex, atomic::AtomicBool};
    use std::task::Poll;

    struct ReleaseOnDrop(Arc<(Mutex<bool>, Condvar)>);
    impl ReleaseOnDrop {
        fn release(&self) {
            let (ready, wake) = &*self.0;
            *ready.lock().unwrap() = true;
            wake.notify_all();
        }
    }
    impl Drop for ReleaseOnDrop {
        fn drop(&mut self) {
            self.release();
        }
    }

    let (_dir, store) = fixture().await;
    inbox(
        &store,
        "a",
        vec![(NOTIFICATION, "67", T1), ("other-notification", "68", T1)],
    )
    .await;
    let first_request = request(&store, "a", NOTIFICATION).await;
    let second_request = request(&store, "a", "other-notification").await;
    let revision = store.revision().await.unwrap();
    let (entered_tx, entered_rx) = tokio::sync::oneshot::channel();
    let entered = Mutex::new(Some(entered_tx));
    let barrier = Arc::new((Mutex::new(false), Condvar::new()));
    let release = ReleaseOnDrop(barrier.clone());
    let first_store = store.clone();
    let handle = tokio::runtime::Handle::current();
    // Hold the actual Store writer using the public initial authority guard;
    // no artificial SQLite locks, wall-clock sleeps or production hooks.
    let first = tokio::task::spawn_blocking(move || {
        handle.block_on(
            first_store.request_notification_subject_checked(&first_request, || {
                if let Some(sender) = entered.lock().unwrap().take() {
                    sender.send(()).unwrap();
                }
                let (ready, wake) = &*barrier;
                let mut ready = ready.lock().unwrap();
                while !*ready {
                    ready = wake.wait(ready).unwrap();
                }
                Err(CollaborationError::new(
                    ErrorCode::Busy,
                    "Synthetic writer owner released",
                ))
            }),
        )
    });
    entered_rx.await.unwrap();
    let alive = AtomicBool::new(true);
    let calls = AtomicUsize::new(0);
    let pending = store.request_notification_subject_checked(&second_request, || {
        calls.fetch_add(1, Ordering::SeqCst);
        if alive.load(Ordering::SeqCst) {
            Ok(())
        } else {
            Err(CollaborationError::new(
                ErrorCode::PermissionDenied,
                "Retired waiting caller",
            ))
        }
    });
    tokio::pin!(pending);
    assert!(matches!(futures_util::poll!(&mut pending), Poll::Pending));
    assert_eq!(
        calls.load(Ordering::SeqCst),
        0,
        "authority must be inspected after obtaining the held writer"
    );
    alive.store(false, Ordering::SeqCst);
    release.release();
    assert_eq!(pending.await.unwrap_err().code, ErrorCode::PermissionDenied);
    assert_eq!(first.await.unwrap().unwrap_err().code, ErrorCode::Busy);
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert!(
        store
            .pending_notification_subjects()
            .await
            .unwrap()
            .is_empty()
    );
    assert_eq!(store.revision().await.unwrap(), revision);
}

#[tokio::test]
async fn discovery_absence_withdraws_only_narrow_grant_and_preserves_independent_selected_cache() {
    for selected in [false, true] {
        let (_dir, store) = fixture().await;
        discover(&store, "a", NOTIFICATION, SUBJECT, "9001").await;
        if selected {
            store.select_repository("a", REPO, true).await.unwrap();
        }
        let authored = draft(&store, "a", SUBJECT).await;
        let late = body_commit(&store, "a", SUBJECT).await;
        let prior_run = store
            .scope_state("a", "detail:pull-67:body")
            .await
            .unwrap()
            .unwrap()
            .run_id;
        for _ in 0..2 {
            page(&store, "a", "repositories", vec![], vec![], vec![]).await;
        }
        assert!(
            store
                .repositories("a")
                .await
                .unwrap()
                .repositories
                .is_empty()
        );
        assert!(
            store
                .query_items(item_query("a", None))
                .await
                .unwrap()
                .items
                .is_empty()
        );
        assert_eq!(
            resolve(&store, "a", NOTIFICATION).await.reason,
            Some(NotificationSubjectReason::InactiveMembership)
        );
        if selected {
            assert!(
                store.item("a", SUBJECT).await.unwrap().item.is_some(),
                "independent selection retains existing ordinary saved reads"
            );
            assert_eq!(
                detail(&store, "a", SUBJECT).await.body.text.as_deref(),
                Some("Private detail a")
            );
            assert_eq!(
                store
                    .scope_state("a", "detail:pull-67:body")
                    .await
                    .unwrap()
                    .unwrap()
                    .run_id,
                prior_run,
                "narrow grant withdrawal must not rotate an independently authorized detail run"
            );
            store.apply_detail(late).await.unwrap();
            assert_eq!(
                detail(&store, "a", SUBJECT).await.body.text.as_deref(),
                Some("Late response")
            );
        } else {
            assert!(store.item("a", SUBJECT).await.unwrap().item.is_none());
            assert_eq!(
                detail(&store, "a", SUBJECT).await.evidence.availability,
                DetailAvailability::Unavailable
            );
            assert!(matches!(
                store.apply_detail(late).await.unwrap_err().code,
                ErrorCode::PermissionDenied | ErrorCode::StaleView
            ));
        }
        assert_eq!(
            store.draft("a", SUBJECT).await.unwrap(),
            Some(authored.clone())
        );
        // An actual discovery denial, unlike mere feed absence, withdraws both
        // ordinary selected reads and current notification-derived reads.
        page(
            &store,
            "a",
            "repositories",
            vec![repository("a", REPO, "42", "fixture/project")],
            vec![],
            vec![],
        )
        .await;
        deny(&store, "a", "repositories").await;
        assert!(store.item("a", SUBJECT).await.unwrap().item.is_none());
        assert_eq!(
            detail(&store, "a", SUBJECT).await.evidence.availability,
            DetailAvailability::Unavailable
        );
        assert_eq!(store.draft("a", SUBJECT).await.unwrap(), Some(authored));
    }
}

async fn endpoint_page(
    store: &Store,
    repo: &str,
    native_repo: &str,
    number: &str,
    native_alias: &str,
) {
    let epoch = store.account("a").await.unwrap().authorization_epoch;
    let scope = format!("repo:{repo}:issue");
    let run = store.begin_sync("a", &epoch, &scope).await.unwrap();
    store
        .apply_page(PageCommit {
            account_id: "a".into(),
            authorization_epoch: epoch,
            scope,
            run_id: run,
            repositories: vec![],
            items: vec![],
            endpoint_aliases: vec![EndpointAlias {
                kind: ResourceKind::PullRequest,
                repository_provider_id: native_repo.into(),
                number: number.into(),
                native_identity: native_alias.into(),
                web_url: None,
            }],
            next_cursor: None,
            etag: None,
            last_modified: None,
            not_modified: false,
            complete: true,
            observed_at: T3.into(),
        })
        .await
        .unwrap();
}

#[tokio::test]
async fn same_immutable_representation_alias_with_hidden_competing_target_blocks_new_inbox_grant() {
    let (_dir, store) = fixture().await;
    store.select_repository("a", REPO, true).await.unwrap();
    page(
        &store,
        "a",
        "repo:repo:pull_request",
        vec![],
        vec![subject("a", SUBJECT, "9001", "67")],
        vec![],
    )
    .await;
    endpoint_page(&store, REPO, "42", "67", "issue:1000").await;
    let observed = body_commit(&store, "a", SUBJECT).await;
    store.apply_detail(observed).await.unwrap();
    let authored = draft(&store, "a", SUBJECT).await;
    store.select_repository("a", REPO, false).await.unwrap();
    assert_eq!(
        resolve(&store, "a", NOTIFICATION).await.state,
        NotificationSubjectState::Resolved
    );
    let late = body_commit(&store, "a", SUBJECT).await;
    // Each endpoint observation is properly bound to its own parent/feed. The
    // contradiction is the SAME immutable namespace+native ID claiming two
    // DIFFERENT canonical PRs, not multiple distinct aliases for one PR.
    page(
        &store,
        "a",
        "repositories",
        vec![
            repository("a", REPO, "42", "fixture/project"),
            repository("a", "other-repo", "43", "other/project"),
        ],
        vec![],
        vec![],
    )
    .await;
    store
        .select_repository("a", "other-repo", true)
        .await
        .unwrap();
    let mut other = subject("a", "hidden-competitor", "9002", "68");
    other.repository_id = Some("other-repo".into());
    other.web_url = Some("https://github.com/other/project/pull/68".into());
    page(
        &store,
        "a",
        "repo:other-repo:pull_request",
        vec![],
        vec![other],
        vec![],
    )
    .await;
    endpoint_page(&store, "other-repo", "43", "68", "issue:1000").await;
    store
        .select_repository("a", "other-repo", false)
        .await
        .unwrap();
    let generic = store
        .resolve_resource(
            "a",
            ResourceLocator {
                instance_id: ProviderInstance::public(ProviderKind::Github).id,
                kind: ResourceKind::PullRequest,
                locator_kind: LocatorKind::Native,
                value: "issue:1000".into(),
                repository_path: None,
            },
        )
        .await
        .unwrap();
    // Generic resolution reports only accessible candidates. The new inbox
    // guard may now hide both retained claims before its ambiguity projection.
    assert!(matches!(
        generic.state,
        ResolutionState::Ambiguous | ResolutionState::Unavailable
    ));
    let snapshot = resolve(&store, "a", NOTIFICATION).await;
    assert_eq!(
        snapshot.state,
        NotificationSubjectState::Ambiguous,
        "one canonical number match cannot bypass contradictory immutable representation authority"
    );
    assert_eq!(
        snapshot.reason,
        Some(NotificationSubjectReason::AmbiguousIdentity)
    );
    assert!(snapshot.subject.is_none() && !snapshot.discovery.admission);
    assert!(store.item("a", SUBJECT).await.unwrap().item.is_none());
    assert!(
        store
            .item("a", "hidden-competitor")
            .await
            .unwrap()
            .item
            .is_none()
    );
    assert_eq!(
        detail(&store, "a", SUBJECT).await.evidence.availability,
        DetailAvailability::Unavailable
    );
    let instance = ProviderInstance::public(ProviderKind::Github).id;
    assert_eq!(
        body_capability(&context(&store, "a", SUBJECT, &instance).await)
            .saved_read
            .state,
        CapabilityState::Unavailable
    );
    assert!(matches!(
        store.apply_detail(late).await.unwrap_err().code,
        ErrorCode::PermissionDenied | ErrorCode::StaleView
    ));
    assert_eq!(
        store.detail_subject("a", SUBJECT).await.unwrap_err().code,
        ErrorCode::PermissionDenied
    );
    assert_eq!(
        store
            .request_detail("a", "1", SUBJECT, DetailFacet::Body)
            .await
            .unwrap_err()
            .code,
        ErrorCode::PermissionDenied
    );
    assert_eq!(store.draft("a", SUBJECT).await.unwrap(), Some(authored));
    assert!(
        store
            .pending_notification_subjects()
            .await
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn distinct_representation_native_aliases_bound_to_one_canonical_are_not_ambiguous() {
    let (_dir, store) = fixture().await;
    store.select_repository("a", REPO, true).await.unwrap();
    endpoint_page(&store, REPO, "42", "67", "issue:1000").await;
    endpoint_page(&store, REPO, "42", "67", "issue:1001").await;
    page(
        &store,
        "a",
        "repo:repo:pull_request",
        vec![],
        vec![subject("a", SUBJECT, "9001", "67")],
        vec![],
    )
    .await;
    let observed = body_commit(&store, "a", SUBJECT).await;
    store.apply_detail(observed).await.unwrap();
    let authored = draft(&store, "a", SUBJECT).await;
    store.select_repository("a", REPO, false).await.unwrap();
    let snapshot = resolve(&store, "a", NOTIFICATION).await;
    assert_eq!(snapshot.state, NotificationSubjectState::Resolved);
    assert_eq!(snapshot.subject.unwrap().id, SUBJECT);
    assert!(store.item("a", SUBJECT).await.unwrap().item.is_some());
    assert_eq!(
        detail(&store, "a", SUBJECT).await.body.text.as_deref(),
        Some("Late response")
    );
    for alias in ["issue:1000", "issue:1001"] {
        let resolved = store
            .resolve_resource(
                "a",
                ResourceLocator {
                    instance_id: ProviderInstance::public(ProviderKind::Github).id,
                    kind: ResourceKind::PullRequest,
                    locator_kind: LocatorKind::Native,
                    value: alias.into(),
                    repository_path: None,
                },
            )
            .await
            .unwrap();
        assert_eq!(resolved.state, ResolutionState::Resolved);
        assert_eq!(resolved.resource.unwrap().id, SUBJECT);
    }
    assert_eq!(store.draft("a", SUBJECT).await.unwrap(), Some(authored));
}

#[tokio::test]
async fn point_representation_alias_commits_uniquely_or_rolls_back_conflict_atomically() {
    for conflict in [false, true] {
        let (dir, store) = fixture().await;
        page(
            &store,
            "a",
            "repositories",
            vec![
                repository("a", REPO, "42", "fixture/project"),
                repository("a", "other-repo", "43", "other/project"),
            ],
            vec![],
            vec![],
        )
        .await;
        store
            .select_repository("a", "other-repo", true)
            .await
            .unwrap();
        let mut other = subject("a", "hidden-competitor", "9002", "68");
        other.repository_id = Some("other-repo".into());
        other.web_url = Some("https://github.com/other/project/pull/68".into());
        page(
            &store,
            "a",
            "repo:other-repo:pull_request",
            vec![],
            vec![other],
            vec![],
        )
        .await;
        endpoint_page(
            &store,
            "other-repo",
            "43",
            "68",
            if conflict { "issue:1000" } else { "issue:1001" },
        )
        .await;
        store
            .select_repository("a", "other-repo", false)
            .await
            .unwrap();

        let authored = draft(&store, "a", SUBJECT).await;
        let lease = begin(&store, "a", NOTIFICATION).await;
        let before = feed_footprint(&dir.path().join("fixture.sqlite"), "a").await;
        let revision = store.revision().await.unwrap();
        let mut receipt = verified(&lease, SUBJECT, "9001");
        if let NotificationSubjectDiscovery::Verified {
            endpoint_aliases, ..
        } = &mut receipt
        {
            endpoint_aliases.push(EndpointAlias {
                kind: ResourceKind::PullRequest,
                repository_provider_id: "42".into(),
                number: "67".into(),
                native_identity: "issue:1000".into(),
                web_url: None,
            });
        }
        let committed = store.apply_notification_subject(&lease, receipt).await;
        assert_eq!(
            feed_footprint(&dir.path().join("fixture.sqlite"), "a").await,
            before,
            "a point receipt must never rewrite repository or feed authority"
        );
        assert_eq!(store.draft("a", SUBJECT).await.unwrap(), Some(authored));
        if conflict {
            assert_eq!(committed.unwrap_err().code, ErrorCode::StaleView);
            assert_eq!(store.revision().await.unwrap(), revision);
            assert_eq!(
                resolve(&store, "a", NOTIFICATION).await.state,
                NotificationSubjectState::NotCached
            );
            assert!(store.item("a", SUBJECT).await.unwrap().item.is_none());
            assert_eq!(
                store.pending_notification_subjects().await.unwrap().len(),
                1
            );
            // Prove the failed receipt did not leave hidden canonical, alias or
            // detail evidence behind, even though private authored state stays.
            let mut db = SqliteConnection::connect_with(
                &SqliteConnectOptions::new()
                    .filename(dir.path().join("fixture.sqlite"))
                    .read_only(true),
            )
            .await
            .unwrap();
            let rows: i64 = sqlx::query_scalar("SELECT (SELECT COUNT(*) FROM items WHERE account_id='a' AND id=?) + (SELECT COUNT(*) FROM resource_identities WHERE account_id='a' AND entity_id=?) + (SELECT COUNT(*) FROM detail_observations WHERE account_id='a' AND subject_id=?) + (SELECT COUNT(*) FROM detail_resource_metadata WHERE account_id='a' AND subject_id=?) + (SELECT COUNT(*) FROM resource_aliases WHERE account_id='a' AND entity_id=?) + (SELECT COUNT(*) FROM pending_endpoint_aliases WHERE account_id='a' AND repository_provider_id='42' AND number='67' AND native_identity='issue:1000')")
                .bind(SUBJECT).bind(SUBJECT).bind(SUBJECT).bind(SUBJECT).bind(SUBJECT)
                .fetch_one(&mut db).await.unwrap();
            db.close().await.unwrap();
            assert_eq!(rows, 0, "the entire provider receipt must roll back");
        } else {
            committed.unwrap();
            assert_eq!(
                resolve(&store, "a", NOTIFICATION).await.state,
                NotificationSubjectState::Resolved
            );
            assert_eq!(
                detail(&store, "a", SUBJECT).await.body.text.as_deref(),
                Some("Private detail a")
            );
            let native = store
                .resolve_resource(
                    "a",
                    ResourceLocator {
                        instance_id: ProviderInstance::public(ProviderKind::Github).id,
                        kind: ResourceKind::PullRequest,
                        locator_kind: LocatorKind::Native,
                        value: "issue:1000".into(),
                        repository_path: None,
                    },
                )
                .await
                .unwrap();
            assert_eq!(native.state, ResolutionState::Resolved);
            assert_eq!(native.resource.unwrap().id, SUBJECT);
        }
    }
}
