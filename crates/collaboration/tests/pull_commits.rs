use collaboration::providers::ProviderProfile;
use collaboration::storage::retention::CacheRetentionPolicy;
use collaboration::*;

const BASE: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const HEAD: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
const FIRST: &str = "cccccccccccccccccccccccccccccccccccccccc";
const SECOND: &str = "dddddddddddddddddddddddddddddddddddddddd";

fn account(id: &str) -> RemoteAccount {
    RemoteAccount {
        id: id.into(),
        provider: ProviderKind::Github,
        host: "github.com".into(),
        actor_id: format!("actor-{id}"),
        login: id.into(),
        display_name: None,
        authorization_epoch: "1".into(),
        state: AccountState::Active,
        notifications_supported: true,
    }
}

async fn seed(store: &Store, id: &str) -> RemoteAccount {
    let account = store.upsert_account(account(id)).await.unwrap();
    let repository = RemoteRepository {
        id: "repo".into(),
        account_id: id.into(),
        provider_id: "target-1".into(),
        full_name: "owner/project".into(),
        name: "project".into(),
        web_url: "https://github.com/owner/project".into(),
        description: None,
        default_branch: Some("main".into()),
        selected: true,
    };
    let pull = RemoteItem {
        native_inbox: None,
        id: "pull".into(),
        account_id: id.into(),
        repository_id: Some("repo".into()),
        provider_id: "pull-67".into(),
        kind: RemoteItemKind::PullRequest,
        number: Some("67".into()),
        title: "Commit cache fixture".into(),
        body: None,
        body_omitted: true,
        author: None,
        web_url: Some("https://github.com/owner/project/pull/67".into()),
        state: "open".into(),
        updated_at: "2026-10-07T00:00:00Z".into(),
        head_oid: Some(HEAD.into()),
        is_draft: Some(false),
        reason: None,
        unread: None,
    };
    for (scope, repositories, items) in [
        ("repositories", vec![repository], vec![]),
        ("repo:repo:pull_request", vec![], vec![pull]),
    ] {
        let run_id = store
            .begin_sync(id, &account.authorization_epoch, scope)
            .await
            .unwrap();
        store
            .apply_page(PageCommit {
                account_id: id.into(),
                authorization_epoch: account.authorization_epoch.clone(),
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
                observed_at: "2026-10-07T00:00:00Z".into(),
            })
            .await
            .unwrap();
    }
    hydrate_range(store, &account, BASE, HEAD, "fork-2").await;
    account
}

async fn hydrate_range(
    store: &Store,
    account: &RemoteAccount,
    base: &str,
    head: &str,
    source_repository: &str,
) {
    let lease = store
        .begin_detail(
            &account.id,
            &account.authorization_epoch,
            "pull",
            DetailFacet::Body,
        )
        .await
        .unwrap();
    let source = DetailSource {
        source: "fixture/pull/v1".into(),
        adapter_version: 1,
        field_mask: vec![DetailField::Body],
        provider_updated_at: None,
        observed_at: "2099-01-01T00:00:00Z".into(),
    };
    store
        .apply_detail(DetailCommit {
            reconciliation: DetailReconciliation::full_history(),
            account_id: account.id.clone(),
            authorization_epoch: account.authorization_epoch.clone(),
            authorization_view: lease.authorization_view,
            instance_id: lease.instance_id,
            subject_id: "pull".into(),
            facet: DetailFacet::Body,
            run_id: lease.run_id,
            request_cursor: lease.next_cursor,
            body: DetailValue {
                state: DetailValueState::Known,
                text: Some("body".into()),
            },
            metadata: Some(ResourceMetadataObservation {
                kind: RemoteItemKind::PullRequest,
                values: ResourceMetadataValues {
                    base: Some(DetailBranch {
                        name: "main".into(),
                        oid: base.into(),
                        repository: Some(DetailRepositoryRef {
                            provider_id: "target-1".into(),
                            full_name: "owner/project".into(),
                            web_url: None,
                        }),
                    }),
                    head: Some(DetailBranch {
                        name: "feature".into(),
                        oid: head.into(),
                        repository: Some(DetailRepositoryRef {
                            provider_id: source_repository.into(),
                            full_name: "fork/project".into(),
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
                    adapter_version: 1,
                    provider_updated_at: None,
                    observed_at: source.observed_at.clone(),
                },
            }),
            subject_binding: Some(DetailSubjectBinding {
                repository_id: "repo".into(),
                repository_provider_id: "target-1".into(),
                provider_id: "pull-67".into(),
                number: Some("67".into()),
                kind: RemoteItemKind::PullRequest,
                head_oid: Some(head.into()),
            }),
            entries: vec![],
            source,
            next_cursor: None,
            etag: None,
            not_modified: false,
            whole_scope: true,
            complete: true,
            freshness_seconds: 3_600,
        })
        .await
        .unwrap();
}

async fn omit_range_metadata(store: &Store, account: &RemoteAccount) {
    let lease = store
        .begin_detail(
            &account.id,
            &account.authorization_epoch,
            "pull",
            DetailFacet::Body,
        )
        .await
        .unwrap();
    let source = DetailSource {
        source: "fixture/pull/v1".into(),
        adapter_version: 1,
        field_mask: vec![DetailField::Body],
        provider_updated_at: None,
        observed_at: "2099-01-02T00:00:00Z".into(),
    };
    store
        .apply_detail(DetailCommit {
            reconciliation: DetailReconciliation::full_history(),
            account_id: account.id.clone(),
            authorization_epoch: account.authorization_epoch.clone(),
            authorization_view: lease.authorization_view,
            instance_id: lease.instance_id,
            subject_id: "pull".into(),
            facet: DetailFacet::Body,
            run_id: lease.run_id,
            request_cursor: None,
            body: DetailValue {
                state: DetailValueState::Known,
                text: Some("body without current range facts".into()),
            },
            metadata: Some(ResourceMetadataObservation {
                kind: RemoteItemKind::PullRequest,
                values: ResourceMetadataValues::default(),
                fields: vec![
                    MetadataObservedField {
                        field: MetadataField::Base,
                        state: DetailValueState::Omitted,
                    },
                    MetadataObservedField {
                        field: MetadataField::Head,
                        state: DetailValueState::Omitted,
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
                repository_provider_id: "target-1".into(),
                provider_id: "pull-67".into(),
                number: Some("67".into()),
                kind: RemoteItemKind::PullRequest,
                head_oid: Some(HEAD.into()),
            }),
            entries: vec![],
            source,
            next_cursor: None,
            etag: None,
            not_modified: false,
            whole_scope: true,
            complete: true,
            freshness_seconds: 3_600,
        })
        .await
        .unwrap();
}

fn provider_commit(oid: &str) -> ProviderPullCommit {
    ProviderPullCommit {
        oid: oid.into(),
        summary: format!("commit {}", &oid[..8]),
        message: PullCommitMessage {
            state: PullCommitMessageState::Known,
            text: Some(format!("commit {}\n", &oid[..8])),
        },
        author: PullCommitActor {
            name: "Fixture Author".into(),
            provider: Some(DetailActor {
                provider_id: "actor-1".into(),
                login: "fixture".into(),
                web_url: Some("https://github.com/fixture".into()),
            }),
        },
        committer: Some(PullCommitActor {
            name: "Fixture Committer".into(),
            provider: None,
        }),
        authored_at: Some("2026-10-07T00:00:00Z".into()),
        committed_at: Some("2026-10-07T00:00:01Z".into()),
        parent_oids: vec![BASE.into()],
        web_url: Some(format!("https://github.com/owner/project/commit/{oid}")),
    }
}

fn page(
    lease: &PullCommitLease,
    commits: Vec<ProviderPullCommit>,
    next_cursor: Option<&str>,
) -> PullCommitProviderPage {
    PullCommitProviderPage {
        context: lease.binding.context.clone(),
        commits,
        order: PullCommitProviderOrder::BaseToHead,
        source: PullCommitSource {
            source: "github/pull-commits/v1".into(),
            adapter_version: 1,
        },
        start_position: lease.row_count,
        next_cursor: next_cursor.map(str::to_owned),
        cap_reason: None,
        remote_has_more: next_cursor.is_some(),
        freshness_seconds: 3_600,
        cooldown_seconds: None,
    }
}

fn range_validation(lease: &PullCommitLease) -> PullCommitRangeValidation {
    PullCommitRangeValidation {
        base_oid: lease.binding.context.base_oid.clone(),
        head_oid: lease.binding.context.head_oid.clone(),
        base_repository_provider_id: lease.binding.repository_provider_id.clone(),
        source_repository_provider_id: lease.binding.context.source_repository_provider_id.clone(),
    }
}

async fn apply(
    store: &Store,
    account: &RemoteAccount,
    lease: &PullCommitLease,
    page: PullCommitProviderPage,
) -> PullCommitApplyReceipt {
    store
        .apply_pull_commits(PullCommitCommit {
            account_id: account.id.clone(),
            authorization_epoch: account.authorization_epoch.clone(),
            lease: lease.clone(),
            request_cursor: lease.next_cursor.clone(),
            page,
            terminal_validation: Some(range_validation(lease)),
        })
        .await
        .unwrap()
}

fn query(account: &str, cursor: Option<String>, limit: u32) -> PullCommitQuery {
    PullCommitQuery {
        account_id: account.into(),
        subject_id: "pull".into(),
        cursor,
        limit,
    }
}

fn pull_commit_capability_profile(_: &RemoteAccount, _: &ProviderInstance) -> ProviderProfile {
    let mut profile = ProviderProfile::read_only(InboxSemantics::NativeNotifications, true);
    let commits = profile
        .facets
        .iter_mut()
        .find(|capability| capability.facet == ResourceFacet::PullCommits)
        .unwrap();
    commits.state = CapabilityState::Supported;
    commits.reason = None;
    profile
}

fn pull_commit_capability_request() -> ContextCapabilityRequest {
    ContextCapabilityRequest {
        account_id: "a".into(),
        authorization_epoch: "1".into(),
        target: CapabilityTarget {
            kind: CapabilityTargetKind::Resource,
            instance_id: Some(ProviderInstance::public(ProviderKind::Github).id),
            repository_id: None,
            resource_id: Some("pull".into()),
            resource_kind: Some(ResourceKind::PullRequest),
        },
    }
}

fn commit_capability(snapshot: &ContextualCapabilitySnapshot) -> &ContextFacetCapability {
    snapshot
        .facets
        .iter()
        .find(|capability| capability.facet == ResourceFacet::PullCommits)
        .unwrap()
}

fn retention_policy(target_logical_bytes: u64) -> CacheRetentionPolicy {
    CacheRetentionPolicy {
        target_logical_bytes,
        max_index_facets: u32::MAX,
        max_scan_facets: u32::MAX,
        max_evict_facets: u32::MAX,
        max_entry_rows: u32::MAX,
        checkpoint_wal: false,
    }
}

async fn complete_retention_index(store: &Store) {
    for _ in 0..8 {
        let report = store
            .run_cache_maintenance(retention_policy(u64::MAX))
            .await
            .unwrap();
        if report.usage_after.index_complete {
            return;
        }
    }
    panic!("bounded retention indexing did not complete");
}

fn numbered_oid(index: u64) -> String {
    format!("{index:040x}")
}

#[test]
fn canonical_commit_identity_accepts_only_lowercase_sha1_and_sha256() {
    assert!(is_canonical_commit_oid(&"a".repeat(40)));
    assert!(is_canonical_commit_oid(&"b".repeat(64)));
    assert!(!is_canonical_commit_oid(&"a".repeat(39)));
    assert!(!is_canonical_commit_oid(&"a".repeat(65)));
    assert!(!is_canonical_commit_oid(&"A".repeat(40)));
    assert!(!is_canonical_commit_oid(&format!("{}g", "a".repeat(39))));
}

#[test]
fn completeness_has_a_stable_object_shape_and_exact_cap_reason_invariant() {
    assert_eq!(
        serde_json::to_value(PullCommitCompleteness::complete()).unwrap(),
        serde_json::json!({ "state": "complete", "reason": null })
    );
    assert_eq!(
        serde_json::to_value(PullCommitCompleteness::capped(
            PullCommitCapReason::ProviderLimit
        ))
        .unwrap(),
        serde_json::json!({ "state": "capped", "reason": "provider_limit" })
    );
    assert!(PullCommitCompleteness::missing().is_valid());
    assert!(PullCommitCompleteness::syncing().is_valid());
    assert!(PullCommitCompleteness::complete().is_valid());
    assert!(PullCommitCompleteness::partial().is_valid());
    assert!(PullCommitCompleteness::capped(PullCommitCapReason::LocalLimit).is_valid());
    assert!(
        !PullCommitCompleteness {
            state: PullCommitCompletenessState::Capped,
            reason: None,
        }
        .is_valid()
    );
    assert!(
        !PullCommitCompleteness {
            state: PullCommitCompletenessState::Complete,
            reason: Some(PullCommitCapReason::ProviderLimit),
        }
        .is_valid()
    );
}

#[tokio::test]
async fn publishes_terminal_generation_atomically_in_base_to_head_order() {
    let directory = tempfile::tempdir().unwrap();
    let store = Store::open(directory.path().join("cache.sqlite"))
        .await
        .unwrap();
    let account = seed(&store, "a").await;
    let mut lease = store
        .begin_pull_commits(&account.id, &account.authorization_epoch, "pull")
        .await
        .unwrap();
    let first = apply(
        &store,
        &account,
        &lease,
        page(&lease, vec![provider_commit(FIRST)], Some("page-2")),
    )
    .await;
    assert!(!first.published);
    let unpublished = store.pull_commits(query("a", None, 100)).await.unwrap();
    assert!(unpublished.commits.is_empty());
    assert_eq!(unpublished.completeness, PullCommitCompleteness::syncing());
    lease.next_cursor = first.next_cursor;
    lease.page_count += 1;
    lease.row_count = first.row_count;
    let terminal = apply(
        &store,
        &account,
        &lease,
        page(
            &lease,
            vec![provider_commit(SECOND), provider_commit(HEAD)],
            None,
        ),
    )
    .await;
    assert!(terminal.published);
    let first_page = store.pull_commits(query("a", None, 2)).await.unwrap();
    assert_eq!(first_page.completeness, PullCommitCompleteness::complete());
    assert_eq!(first_page.coverage.state, CoverageState::Complete);
    assert_eq!(
        first_page
            .commits
            .iter()
            .map(|commit| (commit.position, commit.oid.as_str()))
            .collect::<Vec<_>>(),
        vec![(0, FIRST), (1, SECOND)]
    );
    let second_page = store
        .pull_commits(query("a", first_page.next_cursor, 2))
        .await
        .unwrap();
    assert_eq!(second_page.commits[0].position, 2);
    assert_eq!(second_page.commits[0].oid, HEAD);
    let receipt = store
        .verify_pull_commit_membership(PullCommitMembershipRequest {
            account_id: "a".into(),
            authorization_epoch: "1".into(),
            subject_id: "pull".into(),
            commit_oid: SECOND.into(),
            facet_revision: second_page.facet_revision.unwrap(),
        })
        .await
        .unwrap();
    assert_eq!(receipt.repository_id, "repo");
    assert_eq!(receipt.commit_oid, SECOND);
    assert_eq!(receipt.context.head_oid, HEAD);
}

#[tokio::test]
async fn missing_syncing_complete_empty_and_terminal_partial_remain_distinct() {
    let directory = tempfile::tempdir().unwrap();
    let store = Store::open(directory.path().join("cache.sqlite"))
        .await
        .unwrap();
    let account = seed(&store, "a").await;
    assert_eq!(
        store
            .pull_commits(query("a", None, 100))
            .await
            .unwrap()
            .completeness,
        PullCommitCompleteness::missing()
    );
    let empty = store.begin_pull_commits("a", "1", "pull").await.unwrap();
    assert_eq!(
        store
            .pull_commits(query("a", None, 100))
            .await
            .unwrap()
            .completeness,
        PullCommitCompleteness::syncing()
    );
    apply(&store, &account, &empty, page(&empty, vec![], None)).await;
    let complete_empty = store.pull_commits(query("a", None, 100)).await.unwrap();
    assert_eq!(
        complete_empty.completeness,
        PullCommitCompleteness::complete()
    );
    assert!(complete_empty.commits.is_empty());
    assert_eq!(complete_empty.coverage.state, CoverageState::Complete);

    let partial = store.begin_pull_commits("a", "1", "pull").await.unwrap();
    let mut partial_page = page(&partial, vec![provider_commit(FIRST)], None);
    partial_page.remote_has_more = true;
    apply(&store, &account, &partial, partial_page).await;
    let saved_partial = store.pull_commits(query("a", None, 100)).await.unwrap();
    assert_eq!(
        saved_partial.completeness,
        PullCommitCompleteness::partial()
    );
    assert_eq!(saved_partial.coverage.state, CoverageState::Partial);
    assert!(saved_partial.coverage.remote_has_more);
    assert_eq!(saved_partial.commits[0].oid, FIRST);
}

#[tokio::test]
async fn contextual_capability_uses_exact_commit_generation_and_commit_scope_evidence() {
    let directory = tempfile::tempdir().unwrap();
    let store = Store::open(directory.path().join("cache.sqlite"))
        .await
        .unwrap();
    let account = seed(&store, "a").await;

    let missing = store
        .contextual_capabilities(
            pull_commit_capability_request(),
            pull_commit_capability_profile,
        )
        .await
        .unwrap();
    let policy = commit_capability(&missing);
    assert_eq!(policy.saved_read.state, CapabilityState::Supported);
    assert_eq!(policy.synchronize.state, CapabilityState::Supported);
    assert_eq!(policy.observation, CapabilityObservation::NotLoaded);

    let empty = store.begin_pull_commits("a", "1", "pull").await.unwrap();
    apply(&store, &account, &empty, page(&empty, vec![], None)).await;
    let complete_empty = store
        .contextual_capabilities(
            pull_commit_capability_request(),
            pull_commit_capability_profile,
        )
        .await
        .unwrap();
    assert_eq!(
        commit_capability(&complete_empty).observation,
        CapabilityObservation::Empty
    );

    let complete = store.begin_pull_commits("a", "1", "pull").await.unwrap();
    apply(
        &store,
        &account,
        &complete,
        page(
            &complete,
            vec![provider_commit(FIRST), provider_commit(HEAD)],
            None,
        ),
    )
    .await;
    let complete_snapshot = store
        .contextual_capabilities(
            pull_commit_capability_request(),
            pull_commit_capability_profile,
        )
        .await
        .unwrap();
    assert_eq!(
        commit_capability(&complete_snapshot).observation,
        CapabilityObservation::Complete
    );

    let capped = store.begin_pull_commits("a", "1", "pull").await.unwrap();
    let mut capped_page = page(
        &capped,
        vec![provider_commit(FIRST), provider_commit(HEAD)],
        None,
    );
    capped_page.cap_reason = Some(PullCommitCapReason::ProviderLimit);
    capped_page.remote_has_more = true;
    apply(&store, &account, &capped, capped_page).await;
    let capped_snapshot = store
        .contextual_capabilities(
            pull_commit_capability_request(),
            pull_commit_capability_profile,
        )
        .await
        .unwrap();
    assert_eq!(
        commit_capability(&capped_snapshot).observation,
        CapabilityObservation::Partial
    );

    store
        .set_sync_status(
            "a",
            "1",
            "detail:pull:commits",
            SyncStatus {
                state: SyncState::Offline,
                error: Some(CollaborationError::new(
                    ErrorCode::Network,
                    "fixture offline",
                )),
                next_retry_at: Some("2099-10-07T00:00:00Z".into()),
                ..SyncStatus::default()
            },
        )
        .await
        .unwrap();
    let offline = store
        .contextual_capabilities(
            pull_commit_capability_request(),
            pull_commit_capability_profile,
        )
        .await
        .unwrap();
    let policy = commit_capability(&offline);
    assert_eq!(policy.saved_read.state, CapabilityState::Supported);
    assert_eq!(policy.observation, CapabilityObservation::Partial);
    assert_eq!(policy.sync.state, SyncState::Offline);
    assert_eq!(
        policy.synchronize.reason,
        Some(ContextCapabilityReason::TemporarilyUnavailable)
    );

    store
        .set_sync_status(
            "a",
            "1",
            "detail:pull:commits",
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
        .contextual_capabilities(
            pull_commit_capability_request(),
            pull_commit_capability_profile,
        )
        .await
        .unwrap();
    let policy = commit_capability(&denied);
    assert_eq!(policy.saved_read.state, CapabilityState::Unavailable);
    assert_eq!(
        policy.saved_read.reason,
        Some(ContextCapabilityReason::PermissionDenied)
    );
    assert_eq!(policy.observation, CapabilityObservation::Unknown);
    assert!(policy.can_recheck_access);
}

#[tokio::test]
async fn latest_omitted_range_facts_hide_saved_oids_and_block_a_new_traversal() {
    let directory = tempfile::tempdir().unwrap();
    let store = Store::open(directory.path().join("cache.sqlite"))
        .await
        .unwrap();
    let account = seed(&store, "a").await;
    let lease = store.begin_pull_commits("a", "1", "pull").await.unwrap();
    apply(
        &store,
        &account,
        &lease,
        page(
            &lease,
            vec![provider_commit(FIRST), provider_commit(HEAD)],
            None,
        ),
    )
    .await;
    let facet_revision = store
        .pull_commits(query("a", None, 100))
        .await
        .unwrap()
        .facet_revision
        .unwrap();

    omit_range_metadata(&store, &account).await;

    let hidden = store.pull_commits(query("a", None, 100)).await.unwrap();
    assert!(hidden.context.is_none());
    assert!(hidden.commits.is_empty());
    assert_eq!(hidden.completeness, PullCommitCompleteness::missing());
    assert_eq!(
        store
            .begin_pull_commits("a", "1", "pull")
            .await
            .unwrap_err()
            .code,
        ErrorCode::NotFound
    );
    assert_eq!(
        store
            .verify_pull_commit_membership(PullCommitMembershipRequest {
                account_id: "a".into(),
                authorization_epoch: "1".into(),
                subject_id: "pull".into(),
                commit_oid: FIRST.into(),
                facet_revision,
            })
            .await
            .unwrap_err()
            .code,
        ErrorCode::NotFound
    );
}

#[tokio::test]
async fn terminal_publication_requires_fresh_exact_parent_range_validation() {
    let directory = tempfile::tempdir().unwrap();
    let store = Store::open(directory.path().join("cache.sqlite"))
        .await
        .unwrap();
    let account = seed(&store, "a").await;
    let missing = store.begin_pull_commits("a", "1", "pull").await.unwrap();
    assert_eq!(
        store
            .apply_pull_commits(PullCommitCommit {
                account_id: "a".into(),
                authorization_epoch: "1".into(),
                lease: missing.clone(),
                request_cursor: None,
                page: page(
                    &missing,
                    vec![provider_commit(FIRST), provider_commit(HEAD)],
                    None,
                ),
                terminal_validation: None,
            })
            .await
            .unwrap_err()
            .code,
        ErrorCode::StaleView
    );
    assert!(
        store
            .pull_commits(query("a", None, 100))
            .await
            .unwrap()
            .commits
            .is_empty()
    );

    let mismatched = store.begin_pull_commits("a", "1", "pull").await.unwrap();
    let mut validation = range_validation(&mismatched);
    validation.source_repository_provider_id = "other-fork".into();
    assert_eq!(
        store
            .apply_pull_commits(PullCommitCommit {
                account_id: "a".into(),
                authorization_epoch: "1".into(),
                lease: mismatched.clone(),
                request_cursor: None,
                page: page(
                    &mismatched,
                    vec![provider_commit(FIRST), provider_commit(HEAD)],
                    None,
                ),
                terminal_validation: Some(validation),
            })
            .await
            .unwrap_err()
            .code,
        ErrorCode::StaleView
    );

    let valid = store.begin_pull_commits("a", "1", "pull").await.unwrap();
    apply(
        &store,
        &account,
        &valid,
        page(
            &valid,
            vec![provider_commit(FIRST), provider_commit(HEAD)],
            None,
        ),
    )
    .await;
    assert_eq!(
        store
            .pull_commits(query("a", None, 100))
            .await
            .unwrap()
            .commits
            .len(),
        2
    );
}

#[tokio::test]
async fn drift_and_restart_never_replace_the_readable_generation() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("cache.sqlite");
    let store = Store::open(&path).await.unwrap();
    let account = seed(&store, "a").await;
    let lease = store.begin_pull_commits("a", "1", "pull").await.unwrap();
    apply(
        &store,
        &account,
        &lease,
        page(
            &lease,
            vec![provider_commit(FIRST), provider_commit(HEAD)],
            None,
        ),
    )
    .await;
    let before = store.pull_commits(query("a", None, 100)).await.unwrap();
    let staging = store.begin_pull_commits("a", "1", "pull").await.unwrap();
    let error = store
        .apply_pull_commits(PullCommitCommit {
            account_id: "a".into(),
            authorization_epoch: "1".into(),
            lease: staging.clone(),
            request_cursor: None,
            terminal_validation: Some(range_validation(&staging)),
            page: page(
                &staging,
                vec![provider_commit(SECOND), provider_commit(SECOND)],
                None,
            ),
        })
        .await
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::StaleView);
    let retained = store.pull_commits(query("a", None, 100)).await.unwrap();
    assert_eq!(retained.commits, before.commits);
    store.close().await;
    drop(store);
    let reopened = Store::open(&path).await.unwrap();
    let after_restart = reopened.pull_commits(query("a", None, 100)).await.unwrap();
    assert_eq!(after_restart.commits, before.commits);
    reopened.begin_pull_commits("a", "1", "pull").await.unwrap();
}

#[tokio::test]
async fn context_and_authorization_changes_hide_membership_and_stale_cursors() {
    let directory = tempfile::tempdir().unwrap();
    let store = Store::open(directory.path().join("cache.sqlite"))
        .await
        .unwrap();
    let account = seed(&store, "a").await;
    let other = seed(&store, "b").await;
    let lease = store.begin_pull_commits("a", "1", "pull").await.unwrap();
    apply(
        &store,
        &account,
        &lease,
        page(
            &lease,
            vec![provider_commit(FIRST), provider_commit(HEAD)],
            None,
        ),
    )
    .await;
    let saved = store.pull_commits(query("a", None, 1)).await.unwrap();
    let cursor = saved.next_cursor.clone().unwrap();
    let facet_revision = saved.facet_revision.clone().unwrap();
    assert_eq!(
        store
            .verify_pull_commit_membership(PullCommitMembershipRequest {
                account_id: other.id,
                authorization_epoch: "1".into(),
                subject_id: "pull".into(),
                commit_oid: FIRST.into(),
                facet_revision: facet_revision.clone(),
            })
            .await
            .unwrap_err()
            .code,
        ErrorCode::StaleView
    );
    // A new metadata observation changes the context revision even if the OIDs
    // are unchanged; old membership is hidden before replacement publication.
    hydrate_range(&store, &account, BASE, HEAD, "fork-2").await;
    assert!(
        store
            .pull_commits(query("a", None, 100))
            .await
            .unwrap()
            .commits
            .is_empty()
    );
    let replacement = store.begin_pull_commits("a", "1", "pull").await.unwrap();
    apply(
        &store,
        &account,
        &replacement,
        page(
            &replacement,
            vec![provider_commit(FIRST), provider_commit(HEAD)],
            None,
        ),
    )
    .await;
    assert_eq!(
        store
            .pull_commits(query("a", Some(cursor), 1))
            .await
            .unwrap_err()
            .code,
        ErrorCode::StaleView
    );
    assert_eq!(
        store
            .verify_pull_commit_membership(PullCommitMembershipRequest {
                account_id: "a".into(),
                authorization_epoch: "1".into(),
                subject_id: "pull".into(),
                commit_oid: FIRST.into(),
                facet_revision,
            })
            .await
            .unwrap_err()
            .code,
        ErrorCode::StaleView
    );
}

#[tokio::test]
async fn reverses_head_first_pages_and_preserves_explicit_cap_and_oversized_evidence() {
    let directory = tempfile::tempdir().unwrap();
    let store = Store::open(directory.path().join("cache.sqlite"))
        .await
        .unwrap();
    let account = seed(&store, "a").await;
    let mut lease = store.begin_pull_commits("a", "1", "pull").await.unwrap();
    let mut first_page = page(
        &lease,
        vec![provider_commit(HEAD), provider_commit(SECOND)],
        Some("older"),
    );
    first_page.order = PullCommitProviderOrder::HeadToBase;
    let first = apply(&store, &account, &lease, first_page).await;
    lease.next_cursor = first.next_cursor;
    lease.page_count += 1;
    lease.row_count = first.row_count;
    let mut last_commit = provider_commit(FIRST);
    last_commit.message.text = Some("x".repeat(MAX_PULL_COMMIT_MESSAGE_BYTES + 1));
    let mut last_page = page(&lease, vec![last_commit], None);
    last_page.order = PullCommitProviderOrder::HeadToBase;
    apply(&store, &account, &lease, last_page).await;
    let reversed = store.pull_commits(query("a", None, 100)).await.unwrap();
    assert_eq!(
        reversed
            .commits
            .iter()
            .map(|commit| (commit.position, commit.oid.as_str()))
            .collect::<Vec<_>>(),
        vec![(0, FIRST), (1, SECOND), (2, HEAD)]
    );
    assert_eq!(
        reversed.commits[0].message,
        PullCommitMessage {
            state: PullCommitMessageState::Oversized,
            text: None,
        }
    );

    let capped = store.begin_pull_commits("a", "1", "pull").await.unwrap();
    let mut capped_page = page(&capped, vec![provider_commit(FIRST)], None);
    capped_page.cap_reason = Some(PullCommitCapReason::ProviderLimit);
    capped_page.remote_has_more = true;
    apply(&store, &account, &capped, capped_page).await;
    let snapshot = store.pull_commits(query("a", None, 100)).await.unwrap();
    assert_eq!(
        snapshot.completeness,
        PullCommitCompleteness::capped(PullCommitCapReason::ProviderLimit)
    );
    assert_eq!(snapshot.coverage.state, CoverageState::Partial);
    assert!(snapshot.coverage.remote_has_more);

    let wrong_head = store.begin_pull_commits("a", "1", "pull").await.unwrap();
    let mut wrong_head_page = page(&wrong_head, vec![provider_commit(FIRST)], None);
    wrong_head_page.order = PullCommitProviderOrder::HeadToBase;
    wrong_head_page.cap_reason = Some(PullCommitCapReason::ProviderLimit);
    wrong_head_page.remote_has_more = true;
    assert_eq!(
        store
            .apply_pull_commits(PullCommitCommit {
                account_id: account.id.clone(),
                authorization_epoch: account.authorization_epoch.clone(),
                lease: wrong_head.clone(),
                request_cursor: None,
                terminal_validation: Some(range_validation(&wrong_head)),
                page: wrong_head_page,
            })
            .await
            .unwrap_err()
            .code,
        ErrorCode::StaleView
    );
    assert_eq!(
        store
            .pull_commits(query("a", None, 100))
            .await
            .unwrap()
            .completeness,
        PullCommitCompleteness::capped(PullCommitCapReason::ProviderLimit)
    );

    let adapter_capped = store.begin_pull_commits("a", "1", "pull").await.unwrap();
    let mut adapter_capped_page = page(&adapter_capped, vec![provider_commit(FIRST)], None);
    adapter_capped_page.cap_reason = Some(PullCommitCapReason::LocalLimit);
    adapter_capped_page.remote_has_more = true;
    apply(&store, &account, &adapter_capped, adapter_capped_page).await;
    assert_eq!(
        store
            .pull_commits(query("a", None, 100))
            .await
            .unwrap()
            .completeness,
        PullCommitCompleteness::capped(PullCommitCapReason::LocalLimit)
    );
}

#[tokio::test]
async fn authentication_loss_hides_rows_and_rejects_old_epoch_membership() {
    let directory = tempfile::tempdir().unwrap();
    let store = Store::open(directory.path().join("cache.sqlite"))
        .await
        .unwrap();
    let account = seed(&store, "a").await;
    let lease = store.begin_pull_commits("a", "1", "pull").await.unwrap();
    apply(
        &store,
        &account,
        &lease,
        page(
            &lease,
            vec![provider_commit(FIRST), provider_commit(HEAD)],
            None,
        ),
    )
    .await;
    let revision = store.pull_commits(query("a", None, 100)).await.unwrap();
    let facet_revision = revision.facet_revision.unwrap();
    store
        .set_sync_status(
            "a",
            "1",
            &DetailFacet::Commits.scope("pull"),
            SyncStatus {
                state: SyncState::AuthRequired,
                error: Some(CollaborationError::new(
                    ErrorCode::AuthRequired,
                    "fixture credential expired",
                )),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    let hidden = store.pull_commits(query("a", None, 100)).await.unwrap();
    assert!(hidden.commits.is_empty());
    assert_eq!(hidden.sync.state, SyncState::AuthRequired);
    assert_eq!(
        store
            .verify_pull_commit_membership(PullCommitMembershipRequest {
                account_id: "a".into(),
                authorization_epoch: "1".into(),
                subject_id: "pull".into(),
                commit_oid: FIRST.into(),
                facet_revision,
            })
            .await
            .unwrap_err()
            .code,
        ErrorCode::StaleView
    );
}

#[tokio::test]
async fn offline_keeps_same_context_rows_but_access_denial_fences_them() {
    let directory = tempfile::tempdir().unwrap();
    let store = Store::open(directory.path().join("cache.sqlite"))
        .await
        .unwrap();
    let account = seed(&store, "a").await;
    let lease = store.begin_pull_commits("a", "1", "pull").await.unwrap();
    apply(
        &store,
        &account,
        &lease,
        page(
            &lease,
            vec![provider_commit(FIRST), provider_commit(HEAD)],
            None,
        ),
    )
    .await;
    let saved = store.pull_commits(query("a", None, 100)).await.unwrap();
    let facet_revision = saved.facet_revision.unwrap();
    let scope = DetailFacet::Commits.scope("pull");
    store
        .set_sync_status(
            "a",
            "1",
            &scope,
            SyncStatus {
                state: SyncState::Offline,
                error: Some(CollaborationError::new(
                    ErrorCode::Network,
                    "fixture offline",
                )),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    let offline = store.pull_commits(query("a", None, 100)).await.unwrap();
    assert_eq!(offline.sync.state, SyncState::Offline);
    assert_eq!(offline.commits.len(), 2);

    store
        .set_sync_status(
            "a",
            "1",
            &scope,
            SyncStatus {
                state: SyncState::Error,
                error: Some(CollaborationError::new(
                    ErrorCode::PermissionDenied,
                    "fixture access denied",
                )),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    let denied = store.pull_commits(query("a", None, 100)).await.unwrap();
    assert!(denied.commits.is_empty());
    assert_eq!(
        store
            .verify_pull_commit_membership(PullCommitMembershipRequest {
                account_id: "a".into(),
                authorization_epoch: "1".into(),
                subject_id: "pull".into(),
                commit_oid: FIRST.into(),
                facet_revision,
            })
            .await
            .unwrap_err()
            .code,
        ErrorCode::StaleView
    );
}

#[tokio::test]
async fn unrelated_account_denial_rebinds_reads_without_hiding_active_commits() {
    let directory = tempfile::tempdir().unwrap();
    let store = Store::open(directory.path().join("cache.sqlite"))
        .await
        .unwrap();
    let account_a = seed(&store, "a").await;
    let account_b = seed(&store, "b").await;
    for account in [&account_a, &account_b] {
        let lease = store
            .begin_pull_commits(&account.id, &account.authorization_epoch, "pull")
            .await
            .unwrap();
        apply(
            &store,
            account,
            &lease,
            page(
                &lease,
                vec![
                    provider_commit(FIRST),
                    provider_commit(SECOND),
                    provider_commit(HEAD),
                ],
                None,
            ),
        )
        .await;
    }

    let before = store.pull_commits(query("a", None, 1)).await.unwrap();
    let old_view = before.authorization_view.clone();
    let old_cursor = before.next_cursor.unwrap();
    let a_facet_revision = before.facet_revision.unwrap();
    let b_facet_revision = store
        .pull_commits(query("b", None, 1))
        .await
        .unwrap()
        .facet_revision
        .unwrap();
    let held_a = store.begin_pull_commits("a", "1", "pull").await.unwrap();

    store
        .set_sync_status(
            "b",
            "1",
            &DetailFacet::Commits.scope("pull"),
            SyncStatus {
                state: SyncState::Error,
                error: Some(CollaborationError::new(
                    ErrorCode::PermissionDenied,
                    "fixture account b access denied",
                )),
                ..Default::default()
            },
        )
        .await
        .unwrap();

    let rebound = store.pull_commits(query("a", None, 1)).await.unwrap();
    assert_ne!(rebound.authorization_view, old_view);
    assert_eq!(rebound.commits[0].oid, FIRST);
    assert_eq!(rebound.completeness, PullCommitCompleteness::complete());
    assert_eq!(
        rebound.facet_revision.as_deref(),
        Some(a_facet_revision.as_str())
    );
    let new_cursor = rebound.next_cursor.unwrap();
    let cursor_json: serde_json::Value = serde_json::from_str(&new_cursor).unwrap();
    assert_eq!(
        cursor_json["authorization_view"].as_str(),
        Some(rebound.authorization_view.as_str())
    );
    assert_eq!(
        store
            .pull_commits(query("a", Some(old_cursor), 1))
            .await
            .unwrap_err()
            .code,
        ErrorCode::StaleView
    );
    assert_eq!(
        store
            .pull_commits(query("a", Some(new_cursor), 1))
            .await
            .unwrap()
            .commits[0]
            .oid,
        SECOND
    );
    let membership = store
        .verify_pull_commit_membership(PullCommitMembershipRequest {
            account_id: "a".into(),
            authorization_epoch: "1".into(),
            subject_id: "pull".into(),
            commit_oid: FIRST.into(),
            facet_revision: a_facet_revision.clone(),
        })
        .await
        .unwrap();
    assert_eq!(membership.authorization_view, rebound.authorization_view);

    assert_eq!(
        store
            .apply_pull_commits(PullCommitCommit {
                account_id: "a".into(),
                authorization_epoch: "1".into(),
                lease: held_a.clone(),
                request_cursor: held_a.next_cursor.clone(),
                terminal_validation: Some(range_validation(&held_a)),
                page: page(
                    &held_a,
                    vec![provider_commit(FIRST), provider_commit(HEAD)],
                    None,
                ),
            })
            .await
            .unwrap_err()
            .code,
        ErrorCode::StaleView
    );
    let replacement_a = store.begin_pull_commits("a", "1", "pull").await.unwrap();
    assert_ne!(replacement_a.authorization_view, old_view);
    let readable_during_replacement = store.pull_commits(query("a", None, 1)).await.unwrap();
    assert_eq!(readable_during_replacement.commits[0].oid, FIRST);
    assert_eq!(
        readable_during_replacement.facet_revision.as_deref(),
        Some(a_facet_revision.as_str())
    );

    let denied_b = store.pull_commits(query("b", None, 1)).await.unwrap();
    assert!(denied_b.commits.is_empty());
    assert_eq!(denied_b.completeness, PullCommitCompleteness::missing());
    assert_eq!(
        store
            .verify_pull_commit_membership(PullCommitMembershipRequest {
                account_id: "b".into(),
                authorization_epoch: "1".into(),
                subject_id: "pull".into(),
                commit_oid: FIRST.into(),
                facet_revision: b_facet_revision,
            })
            .await
            .unwrap_err()
            .code,
        ErrorCode::StaleView
    );
}

#[tokio::test]
async fn rejects_overlap_reordering_source_changes_and_cursor_loops_without_poisoning_staging() {
    let directory = tempfile::tempdir().unwrap();
    let store = Store::open(directory.path().join("cache.sqlite"))
        .await
        .unwrap();
    let account = seed(&store, "a").await;
    let mut lease = store.begin_pull_commits("a", "1", "pull").await.unwrap();
    let first = apply(
        &store,
        &account,
        &lease,
        page(&lease, vec![provider_commit(FIRST)], Some("page-2")),
    )
    .await;
    lease.next_cursor = first.next_cursor;
    lease.page_count += 1;
    lease.row_count = first.row_count;

    let mut reordered = page(&lease, vec![provider_commit(SECOND)], Some("page-3"));
    reordered.order = PullCommitProviderOrder::HeadToBase;
    assert_eq!(
        store
            .apply_pull_commits(PullCommitCommit {
                account_id: "a".into(),
                authorization_epoch: "1".into(),
                lease: lease.clone(),
                request_cursor: lease.next_cursor.clone(),
                terminal_validation: Some(range_validation(&lease)),
                page: reordered,
            })
            .await
            .unwrap_err()
            .code,
        ErrorCode::StaleView
    );

    let mut changed_source = page(&lease, vec![provider_commit(SECOND)], Some("page-3"));
    changed_source.source.source = "github/other-route/v1".into();
    assert_eq!(
        store
            .apply_pull_commits(PullCommitCommit {
                account_id: "a".into(),
                authorization_epoch: "1".into(),
                lease: lease.clone(),
                request_cursor: lease.next_cursor.clone(),
                terminal_validation: Some(range_validation(&lease)),
                page: changed_source,
            })
            .await
            .unwrap_err()
            .code,
        ErrorCode::StaleView
    );

    let mut overlap = page(&lease, vec![provider_commit(FIRST)], Some("page-3"));
    overlap.start_position = lease.row_count;
    assert_eq!(
        store
            .apply_pull_commits(PullCommitCommit {
                account_id: "a".into(),
                authorization_epoch: "1".into(),
                lease: lease.clone(),
                request_cursor: lease.next_cursor.clone(),
                terminal_validation: Some(range_validation(&lease)),
                page: overlap,
            })
            .await
            .unwrap_err()
            .code,
        ErrorCode::StaleView
    );

    let second = apply(
        &store,
        &account,
        &lease,
        page(&lease, vec![provider_commit(SECOND)], Some("page-3")),
    )
    .await;
    lease.next_cursor = second.next_cursor;
    lease.page_count += 1;
    lease.row_count = second.row_count;
    let looped = page(&lease, vec![provider_commit(HEAD)], Some("page-2"));
    assert_eq!(
        store
            .apply_pull_commits(PullCommitCommit {
                account_id: "a".into(),
                authorization_epoch: "1".into(),
                lease: lease.clone(),
                request_cursor: lease.next_cursor.clone(),
                terminal_validation: Some(range_validation(&lease)),
                page: looped,
            })
            .await
            .unwrap_err()
            .code,
        ErrorCode::StaleView
    );

    let wrong_head = page(&lease, vec![provider_commit(&numbered_oid(99))], None);
    assert_eq!(
        store
            .apply_pull_commits(PullCommitCommit {
                account_id: "a".into(),
                authorization_epoch: "1".into(),
                lease: lease.clone(),
                request_cursor: lease.next_cursor.clone(),
                terminal_validation: Some(range_validation(&lease)),
                page: wrong_head,
            })
            .await
            .unwrap_err()
            .code,
        ErrorCode::StaleView
    );

    apply(
        &store,
        &account,
        &lease,
        page(&lease, vec![provider_commit(HEAD)], None),
    )
    .await;
    assert_eq!(
        store
            .pull_commits(query("a", None, 100))
            .await
            .unwrap()
            .commits
            .iter()
            .map(|commit| commit.oid.as_str())
            .collect::<Vec<_>>(),
        vec![FIRST, SECOND, HEAD]
    );
}

#[tokio::test]
async fn local_pages_obey_byte_bounds_and_reject_forged_keyset_membership() {
    let directory = tempfile::tempdir().unwrap();
    let store = Store::open(directory.path().join("cache.sqlite"))
        .await
        .unwrap();
    let account = seed(&store, "a").await;
    let lease = store.begin_pull_commits("a", "1", "pull").await.unwrap();
    let mut commits = (1..=20)
        .map(|index| {
            let mut commit = provider_commit(&numbered_oid(index));
            commit.message.text = Some("x".repeat(MAX_PULL_COMMIT_MESSAGE_BYTES));
            commit
        })
        .collect::<Vec<_>>();
    commits.last_mut().unwrap().oid = HEAD.into();
    apply(&store, &account, &lease, page(&lease, commits, None)).await;

    let first = store.pull_commits(query("a", None, 100)).await.unwrap();
    assert!(!first.commits.is_empty());
    assert!(first.commits.len() < 20);
    assert!(first.next_cursor.is_some());
    let continuation = store
        .pull_commits(query("a", first.next_cursor.clone(), 100))
        .await
        .unwrap();
    assert_eq!(first.commits.len() + continuation.commits.len(), 20);

    let mut forged: serde_json::Value =
        serde_json::from_str(first.next_cursor.as_deref().unwrap()).unwrap();
    forged["last_position"] = serde_json::json!(499);
    assert_eq!(
        store
            .pull_commits(query("a", Some(forged.to_string()), 100))
            .await
            .unwrap_err()
            .code,
        ErrorCode::StaleView
    );
}

#[tokio::test]
async fn exact_local_limit_publishes_a_truthful_cap_without_an_extra_page() {
    let directory = tempfile::tempdir().unwrap();
    let store = Store::open(directory.path().join("cache.sqlite"))
        .await
        .unwrap();
    let account = seed(&store, "a").await;
    let mut lease = store.begin_pull_commits("a", "1", "pull").await.unwrap();
    for page_index in 0..5_u64 {
        let commits = (1..=100_u64)
            .map(|offset| provider_commit(&numbered_oid(page_index * 100 + offset)))
            .collect::<Vec<_>>();
        let receipt = apply(
            &store,
            &account,
            &lease,
            page(&lease, commits, Some(&format!("page-{}", page_index + 2))),
        )
        .await;
        if page_index < 4 {
            assert!(!receipt.published);
            lease.next_cursor = receipt.next_cursor;
            lease.page_count += 1;
            lease.row_count = receipt.row_count;
        } else {
            assert!(receipt.published);
            assert_eq!(receipt.row_count, MAX_PULL_COMMITS);
        }
    }
    let snapshot = store.pull_commits(query("a", None, 100)).await.unwrap();
    assert_eq!(
        snapshot.completeness,
        PullCommitCompleteness::capped(PullCommitCapReason::LocalLimit)
    );
    assert!(snapshot.coverage.remote_has_more);
}

#[tokio::test]
async fn retention_counts_staging_and_active_rows_and_honors_authored_pins() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("cache.sqlite");
    let store = Store::open(&path).await.unwrap();
    let account = seed(&store, "a").await;
    complete_retention_index(&store).await;
    let baseline = store.cache_usage().await.unwrap().indexed_logical_bytes;
    let lease = store.begin_pull_commits("a", "1", "pull").await.unwrap();
    let staging = store.cache_usage().await.unwrap().indexed_logical_bytes;
    assert!(staging > baseline);
    apply(
        &store,
        &account,
        &lease,
        page(
            &lease,
            vec![provider_commit(FIRST), provider_commit(HEAD)],
            None,
        ),
    )
    .await;
    let accounted = store.cache_usage().await.unwrap();
    assert!(accounted.indexed_logical_bytes > baseline);

    store.set_cache_pin("a", "pull", true).await.unwrap();
    let pinned = store
        .run_cache_maintenance(retention_policy(0))
        .await
        .unwrap();
    assert_eq!(pinned.evicted_facets, 0);
    assert_eq!(
        store
            .pull_commits(query("a", None, 100))
            .await
            .unwrap()
            .commits
            .len(),
        2
    );

    store.set_cache_pin("a", "pull", false).await.unwrap();
    let evicted = store
        .run_cache_maintenance(retention_policy(0))
        .await
        .unwrap();
    assert!(evicted.evicted_facets >= 1);
    assert!(
        store
            .pull_commits(query("a", None, 100))
            .await
            .unwrap()
            .commits
            .is_empty()
    );
    store.close().await;
    drop(store);
    let reopened = Store::open(&path).await.unwrap();
    assert!(
        reopened
            .pull_commits(query("a", None, 100))
            .await
            .unwrap()
            .commits
            .is_empty()
    );
}
