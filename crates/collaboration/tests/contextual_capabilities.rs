use async_trait::async_trait;
use collaboration::{contextual_capabilities::*, providers::ProviderProfile, *};
use collaboration::{
    credentials::*,
    providers::{CollaborationProvider, FeedRequest, FetchPage, ProviderError, VerifiedAccount},
};
use std::sync::Arc;

const CHECK_HEAD: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

fn account(id: &str) -> RemoteAccount {
    RemoteAccount {
        id: id.into(),
        provider: ProviderKind::Github,
        host: "github.com".into(),
        actor_id: id.into(),
        login: id.into(),
        display_name: None,
        authorization_epoch: "1".into(),
        state: AccountState::Active,
        notifications_supported: true,
    }
}
fn account_target() -> CapabilityTarget {
    CapabilityTarget {
        kind: CapabilityTargetKind::Account,
        instance_id: None,
        repository_id: None,
        resource_id: None,
        resource_kind: None,
    }
}
fn repository_target() -> CapabilityTarget {
    CapabilityTarget {
        kind: CapabilityTargetKind::Repository,
        instance_id: Some(ProviderInstance::public(ProviderKind::Github).id),
        repository_id: Some("repo".into()),
        resource_id: None,
        resource_kind: None,
    }
}
fn resource_target() -> CapabilityTarget {
    CapabilityTarget {
        kind: CapabilityTargetKind::Resource,
        instance_id: Some(ProviderInstance::public(ProviderKind::Github).id),
        repository_id: None,
        resource_id: Some("pull".into()),
        resource_kind: Some(ResourceKind::PullRequest),
    }
}
fn request(id: &str, target: CapabilityTarget) -> ContextCapabilityRequest {
    ContextCapabilityRequest {
        account_id: id.into(),
        authorization_epoch: "1".into(),
        target,
    }
}
fn profile(_: &RemoteAccount, _: &ProviderInstance) -> ProviderProfile {
    ProviderProfile::read_only(InboxSemantics::NativeNotifications, true)
}
fn facet(snapshot: &ContextualCapabilitySnapshot, facet: ResourceFacet) -> &ContextFacetCapability {
    snapshot.facets.iter().find(|f| f.facet == facet).unwrap()
}

async fn fixture() -> (tempfile::TempDir, Store) {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path().join("fixture.sqlite"))
        .await
        .unwrap();
    for id in ["a", "b"] {
        store.upsert_account(account(id)).await.unwrap();
    }
    let repo = RemoteRepository {
        id: "repo".into(),
        account_id: "a".into(),
        provider_id: "9007199254740993".into(),
        full_name: "owner/project".into(),
        name: "project".into(),
        web_url: "https://github.com/owner/project".into(),
        description: None,
        default_branch: None,
        selected: false,
    };
    page(&store, "repositories", vec![repo], vec![]).await;
    store.select_repository("a", "repo", true).await.unwrap();
    page(
        &store,
        "repo:repo:pull_request",
        vec![],
        vec![RemoteItem {
            native_inbox: None,
            id: "pull".into(),
            account_id: "a".into(),
            repository_id: Some("repo".into()),
            provider_id: "9007199254740995".into(),
            kind: RemoteItemKind::PullRequest,
            number: Some("67".into()),
            title: "private fixture title".into(),
            body: Some("private fixture body".into()),
            body_omitted: false,
            author: None,
            web_url: None,
            state: "open".into(),
            updated_at: "2026-10-03T12:00:00Z".into(),
            head_oid: None,
            is_draft: None,
            reason: None,
            unread: None,
        }],
    )
    .await;
    (dir, store)
}
async fn page(
    store: &Store,
    scope: &str,
    repositories: Vec<RemoteRepository>,
    items: Vec<RemoteItem>,
) {
    let run_id = store.begin_sync("a", "1", scope).await.unwrap();
    store
        .apply_page(PageCommit {
            account_id: "a".into(),
            authorization_epoch: "1".into(),
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
            observed_at: "2026-10-03T12:00:00Z".into(),
        })
        .await
        .unwrap();
}

#[tokio::test]
async fn target_identity_and_epoch_are_checked_without_returning_provider_content() {
    let (_dir, store) = fixture().await;
    let snapshot = store
        .contextual_capabilities(request("a", resource_target()), profile)
        .await
        .unwrap();
    assert_eq!(
        facet(&snapshot, ResourceFacet::PullRequests)
            .saved_read
            .state,
        CapabilityState::Supported
    );
    assert_eq!(
        facet(&snapshot, ResourceFacet::Reviews).saved_read.state,
        CapabilityState::Unsupported
    );
    assert!(
        snapshot
            .facets
            .iter()
            .all(|facet| facet.remote_write.state == CapabilityState::Unsupported)
    );
    let json = serde_json::to_string(&snapshot).unwrap();
    assert!(!json.contains("private fixture"));
    assert_eq!(
        store
            .contextual_capabilities(request("b", resource_target()), profile)
            .await
            .unwrap_err()
            .code,
        ErrorCode::NotFound
    );
    let mut wrong = resource_target();
    wrong.resource_kind = Some(ResourceKind::Issue);
    assert_eq!(
        store
            .contextual_capabilities(request("a", wrong), profile)
            .await
            .unwrap_err()
            .code,
        ErrorCode::InvalidInput
    );
    let mut wrong = resource_target();
    wrong.instance_id = Some(ProviderInstance::public(ProviderKind::Gitlab).id);
    assert_eq!(
        store
            .contextual_capabilities(request("a", wrong), profile)
            .await
            .unwrap_err()
            .code,
        ErrorCode::InvalidInput
    );
    let mut wrong = account_target();
    wrong.resource_id = Some("pull".into());
    assert_eq!(
        store
            .contextual_capabilities(request("a", wrong), profile)
            .await
            .unwrap_err()
            .code,
        ErrorCode::InvalidInput
    );
    let mut old = request("a", account_target());
    old.authorization_epoch = "0".into();
    assert_eq!(
        store
            .contextual_capabilities(old, profile)
            .await
            .unwrap_err()
            .code,
        ErrorCode::StaleView
    );
}

#[tokio::test]
async fn saved_reads_survive_network_and_rate_limits_but_actual_denial_does_not() {
    let (_dir, store) = fixture().await;
    for code in [ErrorCode::Network, ErrorCode::RateLimited] {
        store
            .set_sync_status(
                "a",
                "1",
                "repo:repo:pull_request",
                SyncStatus {
                    state: if code == ErrorCode::Network {
                        SyncState::Offline
                    } else {
                        SyncState::RateLimited
                    },
                    error: Some(CollaborationError::new(code, "fixture failure")),
                    next_retry_at: Some("2099-10-03T12:00:00Z".into()),
                    ..SyncStatus::default()
                },
            )
            .await
            .unwrap();
        let snapshot = store
            .contextual_capabilities(request("a", repository_target()), profile)
            .await
            .unwrap();
        let policy = facet(&snapshot, ResourceFacet::PullRequests);
        assert_eq!(policy.saved_read.state, CapabilityState::Supported);
        assert_eq!(
            policy.synchronize.reason,
            Some(ContextCapabilityReason::TemporarilyUnavailable)
        );
        assert_eq!(policy.observation, CapabilityObservation::Complete);
        assert!(!policy.can_recheck_access);
        assert!(store.item("a", "pull").await.unwrap().item.is_some());
    }
    store
        .set_sync_status(
            "a",
            "1",
            "repo:repo:pull_request",
            SyncStatus {
                state: SyncState::Error,
                error: Some(CollaborationError::new(
                    ErrorCode::PermissionDenied,
                    "fixture denied",
                )),
                ..SyncStatus::default()
            },
        )
        .await
        .unwrap();
    let snapshot = store
        .contextual_capabilities(request("a", resource_target()), profile)
        .await
        .unwrap();
    let policy = facet(&snapshot, ResourceFacet::PullRequests);
    assert_eq!(
        policy.saved_read.reason,
        Some(ContextCapabilityReason::PermissionDenied)
    );
    assert_eq!(policy.observation, CapabilityObservation::Unknown);
    assert!(policy.can_recheck_access);
    assert!(store.item("a", "pull").await.unwrap().item.is_none());
    let detailed = store
        .contextual_capabilities(request("a", resource_target()), |account, instance| {
            let mut profile = profile(account, instance);
            for facet in &mut profile.facets {
                if matches!(
                    facet.facet,
                    ResourceFacet::PullDetails | ResourceFacet::Reviews | ResourceFacet::Checks
                ) {
                    facet.state = CapabilityState::Supported;
                    facet.reason = None;
                }
            }
            profile
        })
        .await
        .unwrap();
    for detail in [
        ResourceFacet::PullDetails,
        ResourceFacet::Reviews,
        ResourceFacet::Checks,
    ] {
        let policy = facet(&detailed, detail);
        assert_eq!(
            policy.saved_read.reason,
            Some(ContextCapabilityReason::PermissionDenied)
        );
        assert!(
            !policy.can_recheck_access,
            "Inherited parent denial cannot admit detail hydration"
        );
    }
}

#[tokio::test]
async fn missing_complete_empty_unsupported_and_not_selected_are_distinct() {
    let (_dir, store) = fixture().await;
    let missing = store
        .contextual_capabilities(request("a", repository_target()), profile)
        .await
        .unwrap();
    assert_eq!(
        facet(&missing, ResourceFacet::Issues).observation,
        CapabilityObservation::NotLoaded
    );
    page(&store, "repo:repo:issue", vec![], vec![]).await;
    let empty = store
        .contextual_capabilities(request("a", repository_target()), profile)
        .await
        .unwrap();
    assert_eq!(
        facet(&empty, ResourceFacet::Issues).observation,
        CapabilityObservation::Empty
    );
    let unsupported = store
        .contextual_capabilities(request("a", account_target()), |_, _| {
            ProviderProfile::read_only(InboxSemantics::None, false)
        })
        .await
        .unwrap();
    assert_eq!(
        facet(&unsupported, ResourceFacet::Inbox).saved_read.reason,
        Some(ContextCapabilityReason::ProviderSemantics)
    );
    store.select_repository("a", "repo", false).await.unwrap();
    let selected = store
        .contextual_capabilities(request("a", resource_target()), profile)
        .await
        .unwrap();
    assert_eq!(
        facet(&selected, ResourceFacet::PullRequests)
            .saved_read
            .reason,
        Some(ContextCapabilityReason::RepositoryNotSelected)
    );
    assert!(!facet(&selected, ResourceFacet::PullRequests).can_recheck_access);
    assert_eq!(
        facet(&selected, ResourceFacet::Inbox).saved_read.reason,
        Some(ContextCapabilityReason::NotApplicable)
    );
}

#[tokio::test]
async fn successful_page_cooldown_without_an_error_keeps_saved_reads_and_blocks_sync() {
    let (_dir, store) = fixture().await;
    store
        .set_sync_status(
            "a",
            "1",
            "repo:repo:pull_request",
            SyncStatus {
                state: SyncState::RateLimited,
                next_retry_at: Some("2099-10-03T12:00:00Z".into()),
                error: None,
                ..SyncStatus::default()
            },
        )
        .await
        .unwrap();
    let snapshot = store
        .contextual_capabilities(request("a", resource_target()), profile)
        .await
        .unwrap();
    let policy = facet(&snapshot, ResourceFacet::PullRequests);
    assert_eq!(policy.saved_read.state, CapabilityState::Supported);
    assert_eq!(
        policy.synchronize.reason,
        Some(ContextCapabilityReason::TemporarilyUnavailable)
    );
    assert_eq!(
        policy.sync.next_retry_at.as_deref(),
        Some("2099-10-03T12:00:00Z")
    );
    assert!(policy.sync.error.is_none());
    assert_eq!(policy.observation, CapabilityObservation::Complete);
}

#[tokio::test]
async fn retained_identity_after_scope_retirement_does_not_grant_current_saved_reads() {
    let (_dir, store) = fixture().await;
    for _ in 0..2 {
        page(&store, "repo:repo:pull_request", vec![], vec![]).await;
    }
    let snapshot = store
        .contextual_capabilities(request("a", resource_target()), profile)
        .await
        .unwrap();
    assert_eq!(
        facet(&snapshot, ResourceFacet::PullRequests)
            .saved_read
            .reason,
        Some(ContextCapabilityReason::NotObserved)
    );
    assert_eq!(
        facet(&snapshot, ResourceFacet::PullRequests).observation,
        CapabilityObservation::Unknown
    );
    assert!(!facet(&snapshot, ResourceFacet::PullRequests).can_recheck_access);
    // The retained canonical identity can still be validated; it cannot imply
    // active scope membership or return stale private provider content.
    assert_eq!(snapshot.target.resource_id.as_deref(), Some("pull"));
    assert!(
        !serde_json::to_string(&snapshot)
            .unwrap()
            .contains("private fixture")
    );
    for _ in 0..2 {
        page(&store, "repositories", vec![], vec![]).await;
    }
    let repo = store
        .contextual_capabilities(request("a", repository_target()), profile)
        .await
        .unwrap();
    assert_eq!(
        facet(&repo, ResourceFacet::Repositories).saved_read.reason,
        Some(ContextCapabilityReason::NotObserved)
    );
    assert_eq!(
        facet(&repo, ResourceFacet::PullRequests).saved_read.reason,
        Some(ContextCapabilityReason::NotObserved)
    );
}

#[tokio::test]
async fn an_undeclared_profile_is_unknown_and_cannot_infer_read_or_write_support() {
    let (_dir, store) = fixture().await;
    let snapshot = store
        .contextual_capabilities(request("a", account_target()), |_, _| ProviderProfile {
            facets: vec![],
            inbox_semantics: InboxSemantics::None,
        })
        .await
        .unwrap();
    for resource_facet in [
        ResourceFacet::Repositories,
        ResourceFacet::PullRequests,
        ResourceFacet::Issues,
        ResourceFacet::Inbox,
    ] {
        let policy = facet(&snapshot, resource_facet);
        assert_eq!(
            policy.saved_read.reason,
            Some(ContextCapabilityReason::NotObserved)
        );
        assert_eq!(policy.saved_read.state, CapabilityState::Unavailable);
        assert_eq!(policy.synchronize.state, CapabilityState::Unavailable);
        assert_eq!(policy.remote_write.state, CapabilityState::Unsupported);
        assert!(!policy.can_recheck_access);
    }
}

fn detail_profile(account: &RemoteAccount, instance: &ProviderInstance) -> ProviderProfile {
    let mut profile = profile(account, instance);
    for capability in &mut profile.facets {
        if matches!(
            capability.facet,
            ResourceFacet::PullDetails | ResourceFacet::Reviews | ResourceFacet::Checks
        ) {
            capability.state = CapabilityState::Supported;
            capability.reason = None;
        }
    }
    profile
}

async fn detail_page(
    store: &Store,
    facet: DetailFacet,
    state: DetailValueState,
    text: Option<&str>,
    complete: bool,
) {
    let lease = store.begin_detail("a", "1", "pull", facet).await.unwrap();
    store
        .apply_detail(DetailCommit {
            reconciliation: DetailReconciliation::full_history(),
            metadata: None,
            subject_binding: None,
            check_context: None,
            review_context: None,
            account_id: "a".into(),
            authorization_epoch: "1".into(),
            authorization_view: lease.authorization_view,
            instance_id: lease.instance_id,
            subject_id: "pull".into(),
            facet,
            run_id: lease.run_id,
            request_cursor: None,
            body: DetailValue {
                state,
                text: text.map(str::to_owned),
            },
            entries: vec![],
            source: DetailSource {
                source: "fixture.detail.v1".into(),
                adapter_version: 1,
                field_mask: if facet == DetailFacet::Body {
                    vec![DetailField::Body]
                } else {
                    vec![]
                },
                provider_updated_at: Some("2026-10-03T12:00:00Z".into()),
                observed_at: "2026-10-03T12:00:00Z".into(),
            },
            next_cursor: if complete { None } else { Some("next".into()) },
            etag: None,
            not_modified: false,
            whole_scope: true,
            complete,
            freshness_seconds: 60,
        })
        .await
        .unwrap();
}

async fn save_check_body_context(store: &Store) -> CheckContext {
    let mut pull = store.item("a", "pull").await.unwrap().item.unwrap();
    pull.head_oid = Some(CHECK_HEAD.into());
    page(store, "repo:repo:pull_request", vec![], vec![pull.clone()]).await;
    let lease = store
        .begin_detail("a", "1", "pull", DetailFacet::Body)
        .await
        .unwrap();
    let source = DetailSource {
        source: "fixture.body.v1".into(),
        adapter_version: 1,
        field_mask: vec![DetailField::Body],
        provider_updated_at: None,
        observed_at: "2026-10-03T12:00:00Z".into(),
    };
    store
        .apply_detail(DetailCommit {
            reconciliation: DetailReconciliation::full_history(),
            metadata: Some(ResourceMetadataObservation {
                kind: RemoteItemKind::PullRequest,
                values: ResourceMetadataValues {
                    base: Some(DetailBranch {
                        name: "main".into(),
                        oid: "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".into(),
                        repository: Some(DetailRepositoryRef {
                            provider_id: "9007199254740993".into(),
                            full_name: "owner/project".into(),
                            web_url: None,
                        }),
                    }),
                    head: Some(DetailBranch {
                        name: "feature".into(),
                        oid: CHECK_HEAD.into(),
                        repository: Some(DetailRepositoryRef {
                            provider_id: "9007199254740993".into(),
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
                    source: source.source.clone(),
                    adapter_version: source.adapter_version,
                    provider_updated_at: None,
                    observed_at: source.observed_at.clone(),
                },
            }),
            subject_binding: Some(DetailSubjectBinding {
                repository_id: "repo".into(),
                repository_provider_id: "9007199254740993".into(),
                provider_id: pull.provider_id,
                number: pull.number,
                kind: pull.kind,
                head_oid: pull.head_oid,
            }),
            check_context: None,
            review_context: None,
            account_id: "a".into(),
            authorization_epoch: "1".into(),
            authorization_view: lease.authorization_view,
            instance_id: lease.instance_id,
            subject_id: "pull".into(),
            facet: DetailFacet::Body,
            run_id: lease.run_id,
            request_cursor: lease.next_cursor,
            body: DetailValue {
                state: DetailValueState::Known,
                text: Some("saved body".into()),
            },
            entries: vec![],
            source,
            next_cursor: None,
            etag: None,
            not_modified: false,
            whole_scope: true,
            complete: true,
            freshness_seconds: 60,
        })
        .await
        .unwrap();
    let body = store
        .detail(DetailQuery {
            account_id: "a".into(),
            subject_id: "pull".into(),
            facet: DetailFacet::Body,
            cursor: None,
            limit: 1,
        })
        .await
        .unwrap();
    CheckContext {
        head_oid: CHECK_HEAD.into(),
        source_repository_provider_id: "9007199254740993".into(),
        metadata_facet_revision: body.evidence.facet_revision.unwrap(),
    }
}

async fn save_checks_page(
    store: &Store,
    context: &CheckContext,
    include_entry: bool,
    complete: bool,
) {
    let lease = store
        .begin_detail("a", "1", "pull", DetailFacet::Checks)
        .await
        .unwrap();
    store
        .apply_detail(DetailCommit {
            reconciliation: DetailReconciliation {
                enumeration: DetailEnumeration::FullEnumeration,
                head_scope: DetailHeadScope::CurrentHead,
            },
            metadata: None,
            subject_binding: Some(DetailSubjectBinding {
                repository_id: "repo".into(),
                repository_provider_id: "9007199254740993".into(),
                provider_id: "9007199254740995".into(),
                number: Some("67".into()),
                kind: RemoteItemKind::PullRequest,
                head_oid: Some(CHECK_HEAD.into()),
            }),
            check_context: Some(context.clone()),
            review_context: None,
            account_id: "a".into(),
            authorization_epoch: "1".into(),
            authorization_view: lease.authorization_view,
            instance_id: lease.instance_id,
            subject_id: "pull".into(),
            facet: DetailFacet::Checks,
            run_id: lease.run_id,
            request_cursor: lease.next_cursor,
            body: DetailValue::default(),
            entries: if include_entry {
                vec![DetailEntry {
                    id: "github-check-run:1".into(),
                    provider_id: "check-run:1".into(),
                    author: None,
                    title: None,
                    state: None,
                    body: DetailValue::default(),
                    observed_body_state: DetailValueState::NotLoaded,
                    updated_at: None,
                    head_oid: Some(CHECK_HEAD.into()),
                    native: Some(NativeDetailPayload::CheckV1(CheckV1 {
                        kind: CheckKind::CheckRun,
                        name: "build".into(),
                        state: CheckStateV1::CheckRun {
                            status: "completed".into(),
                            conclusion: Some("success".into()),
                        },
                        description: DetailValue::default(),
                        producer: Some("fixture-ci".into()),
                        started_at: None,
                        completed_at: None,
                        updated_at: None,
                        allow_failure: None,
                    })),
                    field_mask: vec![DetailField::Check, DetailField::HeadOid],
                    field_validations: vec![],
                }]
            } else {
                vec![]
            },
            source: DetailSource {
                source: "fixture.checks.v1".into(),
                adapter_version: 1,
                field_mask: vec![DetailField::Check, DetailField::HeadOid],
                provider_updated_at: None,
                observed_at: "2026-10-03T12:00:00Z".into(),
            },
            next_cursor: (!complete).then(|| "page-2".into()),
            etag: None,
            not_modified: false,
            whole_scope: false,
            complete,
            freshness_seconds: 60,
        })
        .await
        .unwrap();
}

fn review_context(context: &CheckContext) -> ReviewContext {
    ReviewContext {
        base_oid: "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".into(),
        head_oid: context.head_oid.clone(),
        base_repository_provider_id: "9007199254740993".into(),
        source_repository_provider_id: context.source_repository_provider_id.clone(),
        metadata_facet_revision: context.metadata_facet_revision.clone(),
    }
}

async fn save_review_page(
    store: &Store,
    context: &ReviewContext,
    include_entry: bool,
    complete: bool,
) {
    let lease = store
        .begin_detail("a", "1", "pull", DetailFacet::ReviewSummaries)
        .await
        .unwrap();
    let fields = vec![
        DetailField::Body,
        DetailField::Author,
        DetailField::State,
        DetailField::UpdatedAt,
        DetailField::HeadOid,
        DetailField::Review,
    ];
    store
        .apply_detail(DetailCommit {
            reconciliation: DetailReconciliation {
                enumeration: if complete {
                    DetailEnumeration::FullEnumeration
                } else {
                    DetailEnumeration::Uncertain
                },
                head_scope: DetailHeadScope::CurrentHead,
            },
            metadata: None,
            subject_binding: Some(DetailSubjectBinding {
                repository_id: "repo".into(),
                repository_provider_id: "9007199254740993".into(),
                provider_id: "9007199254740995".into(),
                number: Some("67".into()),
                kind: RemoteItemKind::PullRequest,
                head_oid: Some(CHECK_HEAD.into()),
            }),
            check_context: None,
            review_context: Some(context.clone()),
            account_id: "a".into(),
            authorization_epoch: "1".into(),
            authorization_view: lease.authorization_view,
            instance_id: lease.instance_id,
            subject_id: "pull".into(),
            facet: DetailFacet::ReviewSummaries,
            run_id: lease.run_id,
            request_cursor: lease.next_cursor,
            body: DetailValue::default(),
            entries: if include_entry {
                vec![DetailEntry {
                    id: "github-review:00000000000000000001".into(),
                    provider_id: "1".into(),
                    author: Some("reviewer".into()),
                    title: None,
                    state: Some("APPROVED".into()),
                    body: DetailValue {
                        state: DetailValueState::Known,
                        text: Some("looks good".into()),
                    },
                    observed_body_state: DetailValueState::Known,
                    updated_at: Some("2026-10-03T12:00:00Z".into()),
                    head_oid: Some(CHECK_HEAD.into()),
                    native: Some(NativeDetailPayload::ReviewV1(ReviewV1 {
                        context: context.clone(),
                        reviewer: Some(ReviewActor {
                            provider_id: "8".into(),
                            login: Some("reviewer".into()),
                            display_name: None,
                        }),
                        decision: ReviewDecision::Approved,
                        provider_state: "APPROVED".into(),
                        reviewed_commit_oid: Some(CHECK_HEAD.into()),
                        submitted_at: Some("2026-10-03T12:00:00Z".into()),
                    })),
                    field_mask: fields.clone(),
                    field_validations: vec![],
                }]
            } else {
                vec![]
            },
            source: DetailSource {
                source: "fixture.reviews.v1".into(),
                adapter_version: 1,
                field_mask: fields,
                provider_updated_at: None,
                observed_at: "2026-10-03T12:00:01Z".into(),
            },
            next_cursor: (!complete).then(|| "page-2".into()),
            etag: None,
            not_modified: false,
            whole_scope: complete,
            complete,
            freshness_seconds: 60,
        })
        .await
        .unwrap();
}

#[tokio::test]
async fn detail_evidence_distinguishes_authoritative_empty_omission_and_oversize_without_body_copy()
{
    for (state, text, expected) in [
        (DetailValueState::Known, None, CapabilityObservation::Empty),
        (
            DetailValueState::Known,
            Some(""),
            CapabilityObservation::Empty,
        ),
        (
            DetailValueState::Known,
            Some("private independent detail body"),
            CapabilityObservation::Complete,
        ),
        (
            DetailValueState::Omitted,
            None,
            CapabilityObservation::Omitted,
        ),
        (
            DetailValueState::Oversized,
            None,
            CapabilityObservation::Oversized,
        ),
    ] {
        let (_dir, store) = fixture().await;
        detail_page(&store, DetailFacet::Body, state, text, true).await;
        let snapshot = store
            .contextual_capabilities(request("a", resource_target()), detail_profile)
            .await
            .unwrap();
        assert_eq!(
            facet(&snapshot, ResourceFacet::PullDetails).observation,
            expected
        );
        assert!(
            !serde_json::to_string(&snapshot)
                .unwrap()
                .contains("private independent detail body")
        );
    }
    let (_dir, store) = fixture().await;
    let missing = store
        .contextual_capabilities(request("a", resource_target()), detail_profile)
        .await
        .unwrap();
    assert_eq!(
        facet(&missing, ResourceFacet::Reviews).observation,
        CapabilityObservation::NotLoaded
    );
    let context = review_context(&save_check_body_context(&store).await);
    save_review_page(&store, &context, true, false).await;
    let partial = store
        .contextual_capabilities(request("a", resource_target()), detail_profile)
        .await
        .unwrap();
    assert_eq!(
        facet(&partial, ResourceFacet::Reviews).observation,
        CapabilityObservation::Partial
    );
    let (_empty_dir, empty_store) = fixture().await;
    let empty_context = review_context(&save_check_body_context(&empty_store).await);
    save_review_page(&empty_store, &empty_context, false, true).await;
    let empty = empty_store
        .contextual_capabilities(request("a", resource_target()), detail_profile)
        .await
        .unwrap();
    assert_eq!(
        facet(&empty, ResourceFacet::Reviews).observation,
        CapabilityObservation::Empty
    );
}

#[tokio::test]
async fn checks_capability_reports_empty_partial_complete_sync_and_access_evidence() {
    let (_dir, store) = fixture().await;
    let missing = store
        .contextual_capabilities(request("a", resource_target()), detail_profile)
        .await
        .unwrap();
    assert_eq!(
        facet(&missing, ResourceFacet::Checks).observation,
        CapabilityObservation::NotLoaded
    );

    let context = save_check_body_context(&store).await;
    save_checks_page(&store, &context, true, false).await;
    let partial = store
        .contextual_capabilities(request("a", resource_target()), detail_profile)
        .await
        .unwrap();
    assert_eq!(
        facet(&partial, ResourceFacet::Checks).observation,
        CapabilityObservation::Partial
    );

    save_checks_page(&store, &context, false, true).await;
    let complete = store
        .contextual_capabilities(request("a", resource_target()), detail_profile)
        .await
        .unwrap();
    let checks = facet(&complete, ResourceFacet::Checks);
    assert_eq!(checks.saved_read.state, CapabilityState::Supported);
    assert_eq!(checks.observation, CapabilityObservation::Complete);

    store
        .set_sync_status(
            "a",
            "1",
            "detail:pull:checks",
            SyncStatus {
                state: SyncState::Syncing,
                ..SyncStatus::default()
            },
        )
        .await
        .unwrap();
    let syncing = store
        .contextual_capabilities(request("a", resource_target()), detail_profile)
        .await
        .unwrap();
    let checks = facet(&syncing, ResourceFacet::Checks);
    assert_eq!(checks.saved_read.state, CapabilityState::Supported);
    assert_eq!(checks.observation, CapabilityObservation::Complete);
    assert_eq!(checks.sync.state, SyncState::Syncing);

    store
        .set_sync_status(
            "a",
            "1",
            "detail:pull:checks",
            SyncStatus {
                state: SyncState::Offline,
                error: Some(CollaborationError::new(
                    ErrorCode::Network,
                    "fixture offline",
                )),
                next_retry_at: Some("2099-10-03T12:00:00Z".into()),
                ..SyncStatus::default()
            },
        )
        .await
        .unwrap();
    let offline = store
        .contextual_capabilities(request("a", resource_target()), detail_profile)
        .await
        .unwrap();
    let checks = facet(&offline, ResourceFacet::Checks);
    assert_eq!(checks.saved_read.state, CapabilityState::Supported);
    assert_eq!(checks.observation, CapabilityObservation::Complete);
    assert_eq!(checks.sync.state, SyncState::Offline);
    assert_eq!(checks.synchronize.state, CapabilityState::Unavailable);
    assert_eq!(
        checks.synchronize.reason,
        Some(ContextCapabilityReason::TemporarilyUnavailable)
    );

    store
        .set_sync_status(
            "a",
            "1",
            "detail:pull:checks",
            SyncStatus {
                state: SyncState::RateLimited,
                error: Some(CollaborationError::new(
                    ErrorCode::RateLimited,
                    "fixture rate limit",
                )),
                next_retry_at: Some("2099-10-03T12:00:00Z".into()),
                ..SyncStatus::default()
            },
        )
        .await
        .unwrap();
    let limited = store
        .contextual_capabilities(request("a", resource_target()), detail_profile)
        .await
        .unwrap();
    let checks = facet(&limited, ResourceFacet::Checks);
    assert_eq!(checks.saved_read.state, CapabilityState::Supported);
    assert_eq!(checks.observation, CapabilityObservation::Complete);
    assert_eq!(checks.synchronize.state, CapabilityState::Unavailable);
    assert_eq!(
        checks.synchronize.reason,
        Some(ContextCapabilityReason::TemporarilyUnavailable)
    );

    store
        .set_sync_status(
            "a",
            "1",
            "detail:pull:checks",
            SyncStatus {
                state: SyncState::Error,
                error: Some(CollaborationError::new(
                    ErrorCode::PermissionDenied,
                    "fixture denied",
                )),
                ..SyncStatus::default()
            },
        )
        .await
        .unwrap();
    let denied = store
        .contextual_capabilities(request("a", resource_target()), detail_profile)
        .await
        .unwrap();
    let checks = facet(&denied, ResourceFacet::Checks);
    assert_eq!(checks.saved_read.state, CapabilityState::Unavailable);
    assert_eq!(
        checks.saved_read.reason,
        Some(ContextCapabilityReason::PermissionDenied)
    );
    assert_eq!(checks.observation, CapabilityObservation::Unknown);
    assert!(checks.can_recheck_access);

    let (_empty_dir, empty_store) = fixture().await;
    let empty_context = save_check_body_context(&empty_store).await;
    save_checks_page(&empty_store, &empty_context, false, true).await;
    let empty = empty_store
        .contextual_capabilities(request("a", resource_target()), detail_profile)
        .await
        .unwrap();
    let checks = facet(&empty, ResourceFacet::Checks);
    assert_eq!(checks.saved_read.state, CapabilityState::Supported);
    assert_eq!(checks.observation, CapabilityObservation::Empty);
}

#[tokio::test]
async fn concurrent_account_cutover_cannot_mix_actor_policy_with_newer_metadata() {
    let (_dir, store) = fixture().await;
    let before = store.accounts().await.unwrap();
    let (start_tx, start_rx) = tokio::sync::oneshot::channel();
    let (done_tx, done_rx) = std::sync::mpsc::channel();
    let writer = store.clone();
    let mutation = std::thread::spawn(move || {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(async move {
                start_rx.await.unwrap();
                let mut next = account("a");
                next.authorization_epoch = "2".into();
                writer.upsert_account(next).await.unwrap();
                done_tx.send(()).unwrap();
            })
    });
    let snapshot = store
        .contextual_capabilities(request("a", account_target()), move |captured, _| {
            assert_eq!(captured.authorization_epoch, "1");
            start_tx.send(()).unwrap();
            done_rx
                .recv_timeout(std::time::Duration::from_secs(5))
                .unwrap();
            profile(captured, &ProviderInstance::public(ProviderKind::Github))
        })
        .await
        .unwrap();
    mutation.join().unwrap();
    assert_eq!(snapshot.authorization_epoch, "1");
    assert_eq!(snapshot.revision, before.revision);
    assert_eq!(snapshot.authorization_view, before.authorization_view);
    let after = store.accounts().await.unwrap();
    assert_ne!(after.revision, snapshot.revision);
    assert_ne!(after.authorization_view, snapshot.authorization_view);
    assert_eq!(
        store
            .contextual_capabilities(request("a", account_target()), profile)
            .await
            .unwrap_err()
            .code,
        ErrorCode::StaleView
    );
}

#[tokio::test]
async fn account_feeds_do_not_grant_reads_or_child_rechecks_through_denied_discovery() {
    let (_dir, store) = fixture().await;
    store
        .set_sync_status(
            "a",
            "1",
            "repositories",
            SyncStatus {
                state: SyncState::Error,
                error: Some(CollaborationError::new(
                    ErrorCode::PermissionDenied,
                    "fixture denied",
                )),
                ..SyncStatus::default()
            },
        )
        .await
        .unwrap();
    let snapshot = store
        .contextual_capabilities(request("a", account_target()), profile)
        .await
        .unwrap();
    for resource_facet in [ResourceFacet::PullRequests, ResourceFacet::Issues] {
        let policy = facet(&snapshot, resource_facet);
        assert_eq!(
            policy.saved_read.reason,
            Some(ContextCapabilityReason::PermissionDenied)
        );
        assert_eq!(policy.synchronize.state, CapabilityState::Unavailable);
        assert!(
            !policy.can_recheck_access,
            "Discovery must be rechecked before inaccessible child admission"
        );
    }
    assert!(facet(&snapshot, ResourceFacet::Repositories).can_recheck_access);
    assert!(
        store
            .query_items(ItemQuery {
                account_id: "a".into(),
                repository_id: None,
                kind: RemoteItemKind::PullRequest,
                state: None,
                search: None,
                cursor: None,
                limit: 10
            })
            .await
            .unwrap()
            .items
            .is_empty()
    );
}

#[tokio::test]
async fn expired_or_absent_retry_barriers_allow_explicit_retry_without_erasing_saved_evidence() {
    let (_dir, store) = fixture().await;
    for (state, error) in [
        (SyncState::RateLimited, Some(ErrorCode::RateLimited)),
        (SyncState::Offline, Some(ErrorCode::Network)),
        (SyncState::Error, Some(ErrorCode::Provider)),
        (SyncState::RateLimited, None),
    ] {
        for deadline in [Some("2000-01-01T00:00:00Z".to_owned()), None] {
            store
                .set_sync_status(
                    "a",
                    "1",
                    "repo:repo:pull_request",
                    SyncStatus {
                        state: state.clone(),
                        next_retry_at: deadline,
                        error: error
                            .clone()
                            .map(|code| CollaborationError::new(code, "fixture transient")),
                        ..SyncStatus::default()
                    },
                )
                .await
                .unwrap();
            let snapshot = store
                .contextual_capabilities(request("a", resource_target()), profile)
                .await
                .unwrap();
            let policy = facet(&snapshot, ResourceFacet::PullRequests);
            assert_eq!(policy.saved_read.state, CapabilityState::Supported);
            assert_eq!(policy.synchronize.state, CapabilityState::Supported);
            assert_eq!(policy.observation, CapabilityObservation::Complete);
            assert_eq!(
                policy.sync.state, state,
                "Status remains descriptive; admission is native checked on explicit retry"
            );
        }
    }
}

#[tokio::test]
async fn account_quota_blocks_an_idle_completed_detail_until_the_later_barrier_expires() {
    let (_dir, store) = fixture().await;
    detail_page(
        &store,
        DetailFacet::Body,
        DetailValueState::Known,
        Some("saved private detail"),
        true,
    )
    .await;
    for deadline in ["2099-10-03T12:00:00Z", "2000-01-01T00:00:00Z"] {
        store
            .set_sync_status(
                "a",
                "1",
                "provider:rest",
                SyncStatus {
                    state: SyncState::RateLimited,
                    next_retry_at: Some(deadline.into()),
                    ..SyncStatus::default()
                },
            )
            .await
            .unwrap();
        let snapshot = store
            .contextual_capabilities(request("a", resource_target()), detail_profile)
            .await
            .unwrap();
        let policy = facet(&snapshot, ResourceFacet::PullDetails);
        assert_eq!(policy.saved_read.state, CapabilityState::Supported);
        assert_eq!(policy.observation, CapabilityObservation::Complete);
        assert_eq!(
            policy.synchronize.state,
            if deadline.starts_with("2099") {
                CapabilityState::Unavailable
            } else {
                CapabilityState::Supported
            }
        );
        if deadline.starts_with("2099") {
            assert_eq!(policy.sync.next_retry_at.as_deref(), Some(deadline));
            assert!(policy.sync.error.is_none());
        }
    }
    store
        .set_sync_status(
            "a",
            "1",
            "provider:rest",
            SyncStatus {
                state: SyncState::RateLimited,
                next_retry_at: Some("2099-10-03T12:00:00Z".into()),
                ..SyncStatus::default()
            },
        )
        .await
        .unwrap();
    store
        .set_sync_status(
            "a",
            "1",
            "repo:repo:pull_request",
            SyncStatus {
                state: SyncState::RateLimited,
                next_retry_at: Some("2099-10-04T12:00:00Z".into()),
                ..SyncStatus::default()
            },
        )
        .await
        .unwrap();
    let snapshot = store
        .contextual_capabilities(request("a", resource_target()), detail_profile)
        .await
        .unwrap();
    assert_eq!(
        facet(&snapshot, ResourceFacet::PullRequests)
            .sync
            .next_retry_at
            .as_deref(),
        Some("2099-10-04T12:00:00Z")
    );
}

struct NoVault;
impl CredentialVault for NoVault {
    fn store(&self, _: &str, _: &SecretToken) -> std::result::Result<(), CredentialError> {
        panic!("Capability reads/unsupported intents must not access credentials")
    }
    fn load(&self, _: &str) -> std::result::Result<Option<SecretToken>, CredentialError> {
        panic!("Capability reads/unsupported intents must not access credentials")
    }
    fn delete(&self, _: &str) -> std::result::Result<(), CredentialError> {
        panic!("Capability reads/unsupported intents must not access credentials")
    }
}
struct UndeclaredProvider;
#[async_trait]
impl CollaborationProvider for UndeclaredProvider {
    fn kind(&self) -> ProviderKind {
        ProviderKind::Github
    }
    async fn probe(&self, _: &SecretToken) -> std::result::Result<VerifiedAccount, ProviderError> {
        panic!("No capability probe HTTP")
    }
    async fn fetch_page(
        &self,
        _: &SecretToken,
        _: FeedRequest,
    ) -> std::result::Result<FetchPage, ProviderError> {
        panic!("No undeclared feed HTTP")
    }
}
#[tokio::test]
async fn undeclared_adapter_default_issues_zero_http_vault_or_durable_hydration_calls() {
    let (_dir, store) = fixture().await;
    let runtime = CollaborationRuntime::new(
        Arc::new(store.clone()),
        Arc::new(NoVault),
        Arc::new(UndeclaredProvider),
    );
    let before = store.accounts().await.unwrap();
    let snapshot = runtime
        .contextual_capabilities(request("a", account_target()))
        .await
        .unwrap();
    assert_eq!(snapshot.inbox_semantics, InboxSemantics::None);
    assert_eq!(
        facet(&snapshot, ResourceFacet::Repositories)
            .saved_read
            .reason,
        Some(ContextCapabilityReason::NotObserved)
    );
    assert_eq!(
        runtime
            .refresh(RefreshRequest {
                account_id: "a".into(),
                repository_id: None,
                kind: Some(RemoteItemKind::PullRequest)
            })
            .await
            .unwrap_err()
            .code,
        ErrorCode::Unsupported
    );
    assert_eq!(
        runtime
            .hydrate_detail(HydrateDetailRequest {
                account_id: "a".into(),
                authorization_epoch: "1".into(),
                subject_id: "pull".into(),
                facet: DetailFacet::Body
            })
            .await
            .unwrap_err()
            .code,
        ErrorCode::Unsupported
    );
    assert_eq!(
        store.accounts().await.unwrap().revision,
        before.revision,
        "Unsupported hydration cannot persist demand or status"
    );
}

#[tokio::test]
async fn bitbucket_comments_are_pr_only_and_unknown_primary_support_is_not_semantic_absence() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path().join("bitbucket.sqlite"))
        .await
        .unwrap();
    let mut actor = account("a");
    actor.provider = ProviderKind::BitbucketCloud;
    actor.host = "bitbucket.org".into();
    actor.actor_id = "11111111-1111-4111-8111-111111111111".into();
    actor.notifications_supported = false;
    store.upsert_account(actor).await.unwrap();
    page(
        &store,
        "repositories",
        vec![RemoteRepository {
            id: "repo".into(),
            account_id: "a".into(),
            provider_id: "44444444-4444-4444-8444-444444444444".into(),
            full_name: "owner/repo".into(),
            name: "repo".into(),
            web_url: "https://bitbucket.org/owner/repo".into(),
            description: None,
            default_branch: None,
            selected: false,
        }],
        vec![],
    )
    .await;
    store.select_repository("a", "repo", true).await.unwrap();
    let provider =
        collaboration::providers::bitbucket_cloud::BitbucketCloudProvider::new().unwrap();
    for (kind, resource_kind, id, number) in [
        (
            RemoteItemKind::PullRequest,
            ResourceKind::PullRequest,
            "pull",
            "67",
        ),
        (RemoteItemKind::Issue, ResourceKind::Issue, "issue", "68"),
    ] {
        let scope = if resource_kind == ResourceKind::PullRequest {
            "repo:repo:pull_request"
        } else {
            "repo:repo:issue"
        };
        page(
            &store,
            scope,
            vec![],
            vec![RemoteItem {
                id: id.into(),
                account_id: "a".into(),
                repository_id: Some("repo".into()),
                provider_id: format!("44444444-4444-4444-8444-444444444444:{number}"),
                kind,
                number: Some(number.into()),
                title: "synthetic cached resource".into(),
                body: None,
                body_omitted: true,
                author: None,
                web_url: None,
                state: "open".into(),
                updated_at: "2026-10-08T00:00:00Z".into(),
                head_oid: None,
                is_draft: None,
                reason: None,
                unread: None,
                native_inbox: None,
            }],
        )
        .await;
        let target = CapabilityTarget {
            kind: CapabilityTargetKind::Resource,
            instance_id: Some(ProviderInstance::public(ProviderKind::BitbucketCloud).id),
            repository_id: None,
            resource_id: Some(id.into()),
            resource_kind: Some(resource_kind),
        };
        let snapshot = store
            .contextual_capabilities(request("a", target.clone()), |account, _| {
                provider.profile(account)
            })
            .await
            .unwrap();
        let comments = facet(&snapshot, ResourceFacet::Comments);
        if resource_kind == ResourceKind::PullRequest {
            assert_eq!(comments.saved_read.state, CapabilityState::Supported);
            assert_eq!(comments.synchronize.state, CapabilityState::Supported);
        } else {
            assert_eq!(comments.saved_read.state, CapabilityState::Unsupported);
            assert_eq!(
                comments.saved_read.reason,
                Some(ContextCapabilityReason::NotApplicable)
            );
            assert_eq!(comments.synchronize, comments.saved_read);
            for state in [Some(CapabilityState::Unavailable), None] {
                let snapshot = store
                    .contextual_capabilities(request("a", target.clone()), |account, _| {
                        let mut profile = provider.profile(account);
                        if let Some(state) = state {
                            profile
                                .facets
                                .iter_mut()
                                .find(|f| f.facet == ResourceFacet::Issues)
                                .unwrap()
                                .state = state;
                        } else {
                            profile.facets.retain(|f| f.facet != ResourceFacet::Issues);
                        }
                        profile
                    })
                    .await
                    .unwrap();
                assert_ne!(
                    facet(&snapshot, ResourceFacet::Comments).saved_read.reason,
                    Some(ContextCapabilityReason::NotApplicable)
                );
            }
        }
        assert_eq!(comments.remote_write.state, CapabilityState::Unsupported);
    }
    store.close().await.unwrap();
}
