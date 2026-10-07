//! Synthetic GitLab HTTP -> native scheduling -> durable offline review reads.
use super::*;
use crate::credentials::CredentialError;
use crate::providers::gitlab::{
    reads_tests::merge_request,
    tests::{response, server},
};
use serde_json::json;
use std::sync::atomic::AtomicUsize;

const HEAD: &str = "1111111111111111111111111111111111111111";
const ACCOUNT: &str = "gitlab-review-account";
const SUBJECT: &str = "gitlab:pull:999";
const REPOSITORY: &str = "gitlab:repository:123";

#[derive(Default)]
struct Vault {
    loads: AtomicUsize,
}
impl CredentialVault for Vault {
    fn load(&self, _: &str) -> Result<Option<SecretToken>, CredentialError> {
        self.loads.fetch_add(1, Ordering::SeqCst);
        Ok(Some(
            SecretToken::new("synthetic_gitlab_review_token".into()).unwrap(),
        ))
    }
    fn store(&self, _: &str, _: &SecretToken) -> Result<(), CredentialError> {
        Ok(())
    }
    fn delete(&self, _: &str) -> Result<(), CredentialError> {
        Ok(())
    }
}
async fn seed(store: &Store) -> RemoteAccount {
    let account = store
        .upsert_account(RemoteAccount {
            id: ACCOUNT.into(),
            provider: ProviderKind::Gitlab,
            host: "gitlab.com".into(),
            actor_id: "42".into(),
            login: "actor".into(),
            display_name: None,
            authorization_epoch: "1".into(),
            state: AccountState::Active,
            notifications_supported: false,
        })
        .await
        .unwrap();
    store
        .stage_credential(ACCOUNT, "synthetic-review-reference")
        .await
        .unwrap();
    let account = store
        .commit_account_credential(
            RemoteAccount {
                authorization_epoch: "2".into(),
                ..account
            },
            "synthetic-review-reference",
        )
        .await
        .unwrap();
    let repository = RemoteRepository {
        id: REPOSITORY.into(),
        account_id: ACCOUNT.into(),
        provider_id: "123".into(),
        full_name: "org/subgroup/project".into(),
        name: "project".into(),
        web_url: "https://gitlab.com/org/subgroup/project".into(),
        description: None,
        default_branch: None,
        selected: true,
    };
    let item = RemoteItem {
        id: SUBJECT.into(),
        account_id: ACCOUNT.into(),
        repository_id: Some(REPOSITORY.into()),
        provider_id: "999".into(),
        kind: RemoteItemKind::PullRequest,
        number: Some("67".into()),
        title: "Native MR".into(),
        body: None,
        body_omitted: true,
        author: None,
        web_url: None,
        state: "open".into(),
        updated_at: "2026-10-04T00:00:00Z".into(),
        head_oid: Some(HEAD.into()),
        is_draft: Some(false),
        reason: None,
        unread: None,
    };
    for (scope, repositories, items) in [
        ("repositories".to_string(), vec![repository], vec![]),
        (
            format!("repo:{REPOSITORY}:pull_request"),
            vec![],
            vec![item],
        ),
    ] {
        let run_id = store
            .begin_sync(ACCOUNT, &account.authorization_epoch, &scope)
            .await
            .unwrap();
        store
            .apply_page(PageCommit {
                account_id: ACCOUNT.into(),
                authorization_epoch: account.authorization_epoch.clone(),
                scope,
                run_id,
                repositories,
                items,
                endpoint_aliases: vec![],
                next_cursor: None,
                etag: None,
                last_modified: None,
                not_modified: false,
                complete: true,
                observed_at: "2026-10-04T00:00:00Z".into(),
            })
            .await
            .unwrap();
    }
    account
}
fn query(facet: DetailFacet) -> DetailQuery {
    DetailQuery {
        account_id: ACCOUNT.into(),
        subject_id: SUBJECT.into(),
        facet,
        cursor: None,
        limit: 100,
    }
}
async fn hydrate(runtime: &CollaborationRuntime, account: &RemoteAccount, facet: DetailFacet) {
    runtime
        .hydrate_detail(HydrateDetailRequest {
            account_id: ACCOUNT.into(),
            authorization_epoch: account.authorization_epoch.clone(),
            subject_id: SUBJECT.into(),
            facet,
        })
        .await
        .unwrap();
    for _ in 0..8 {
        let snapshot = runtime.store.detail(query(facet)).await.unwrap();
        if snapshot.evidence.availability == DetailAvailability::Ready {
            return;
        }
        assert!(
            runtime.run_next().await,
            "bounded fixture review demand did not progress"
        );
    }
    panic!("bounded fixture review demand did not settle");
}
fn approvals() -> String {
    json!({"id":999,"iid":67,"project_id":123,"approved_by":[{"user":{"id":9007199254740993_u64,"username":"approver"}}]}).to_string()
}
fn discussions() -> String {
    json!([{"id":"native-thread","individual_note":false,"notes":[{"id":9007199254741993_u64,"type":"DiffNote","noteable_id":999,"noteable_type":"MergeRequest","project_id":123,"body":"saved native discussion 🌱","author":{"id":42,"username":"reviewer"},"system":false,"created_at":"2026-10-01T00:00:00Z","updated_at":"2026-10-04T00:00:00Z","resolvable":true,"resolved":false,"position":{"position_type":"text","base_sha":"3333333333333333333333333333333333333333","start_sha":"2222222222222222222222222222222222222222","head_sha":HEAD,"old_path":"before.rs","new_path":"after.rs","new_line":8}}]}]).to_string()
}
#[tokio::test]
async fn gitlab_native_reviews_hydrate_exact_body_then_survive_closed_offline_reopen() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("reviews.sqlite");
    let store = Arc::new(Store::open(&path).await.unwrap());
    let account = seed(&store).await;
    let (provider, task) = server(|_| {
        vec![
            response(200, "", &merge_request(999, 123, 67).to_string()),
            response(200, "", &approvals()),
            response(200, "", &discussions()),
        ]
    });
    let vault = Arc::new(Vault::default());
    let runtime = CollaborationRuntime::new(store.clone(), vault.clone(), Arc::new(provider));
    hydrate(&runtime, &account, DetailFacet::ReviewSummaries).await;
    hydrate(&runtime, &account, DetailFacet::ReviewThreads).await;
    let reviews = store
        .detail(query(DetailFacet::ReviewSummaries))
        .await
        .unwrap();
    let threads = store
        .detail(query(DetailFacet::ReviewThreads))
        .await
        .unwrap();
    assert_eq!(reviews.entries.len(), 1);
    assert_eq!(threads.entries.len(), 1);
    let Some(NativeDetailPayload::ReviewV1(review)) = &reviews.entries[0].native else {
        panic!("typed approval missing")
    };
    assert_eq!(review.decision, ReviewDecision::Approved);
    assert!(review.reviewed_commit_oid.is_none());
    assert_eq!(review.context.head_oid, HEAD);
    let Some(NativeDetailPayload::ReviewThreadV1(thread)) = &threads.entries[0].native else {
        panic!("typed thread missing")
    };
    assert!(thread.anchor.is_none());
    assert!(thread.root_comment_id.is_none());
    let Some(native) = thread.native.as_deref() else {
        panic!("GitLab native position missing")
    };
    let ReviewThreadNativeV1::Gitlab(native) = native;
    assert_eq!(
        native.position.as_ref().unwrap().new_path.as_deref(),
        Some("after.rs")
    );
    let calls = task.join().unwrap();
    assert_eq!(calls.len(), 3);
    assert!(calls[0].starts_with("GET /api/v4/projects/123/merge_requests/67 HTTP/1.1"));
    assert!(calls[1].starts_with("GET /api/v4/projects/123/merge_requests/67/approvals HTTP/1.1"));
    assert!(calls[2].starts_with(
        "GET /api/v4/projects/123/merge_requests/67/discussions?per_page=50&page=1 HTTP/1.1"
    ));
    let loads = vault.loads.load(Ordering::SeqCst);
    runtime.shutdown().await.unwrap();
    let offline = Store::open(&path).await.unwrap();
    let saved_reviews = offline
        .detail(query(DetailFacet::ReviewSummaries))
        .await
        .unwrap();
    let saved_threads = offline
        .detail(query(DetailFacet::ReviewThreads))
        .await
        .unwrap();
    assert_eq!(saved_reviews.entries, reviews.entries);
    assert_eq!(saved_threads.entries, threads.entries);
    assert_eq!(vault.loads.load(Ordering::SeqCst), loads);
    offline.close().await.unwrap();
}

#[tokio::test]
async fn bounded_gitlab_threads_publish_partial_and_malformed_native_replacement_is_atomic() {
    let dir = tempfile::tempdir().unwrap();
    let store = Arc::new(
        Store::open(dir.path().join("bounded.sqlite"))
            .await
            .unwrap(),
    );
    let account = seed(&store).await;
    let mut rows: serde_json::Value = serde_json::from_str(&discussions()).unwrap();
    let note = rows[0]["notes"][0].clone();
    rows[0]["notes"] = (1..=51)
        .map(|id| {
            let mut value = note.clone();
            value["id"] = json!(id);
            value
        })
        .collect();
    let (provider, task) = server(|_| {
        vec![
            response(200, "", &merge_request(999, 123, 67).to_string()),
            response(200, "", &rows.to_string()),
        ]
    });
    let runtime = CollaborationRuntime::new(
        store.clone(),
        Arc::new(Vault::default()),
        Arc::new(provider),
    );
    runtime
        .hydrate_detail(HydrateDetailRequest {
            account_id: ACCOUNT.into(),
            authorization_epoch: account.authorization_epoch.clone(),
            subject_id: SUBJECT.into(),
            facet: DetailFacet::ReviewThreads,
        })
        .await
        .unwrap();
    for _ in 0..4 {
        if !runtime.run_next().await {
            break;
        }
    }
    let saved = store
        .detail(query(DetailFacet::ReviewThreads))
        .await
        .unwrap();
    assert_eq!(saved.entries.len(), 50);
    assert_eq!(saved.evidence.availability, DetailAvailability::Partial);
    assert_eq!(saved.evidence.coverage.state, CoverageState::Partial);
    assert_eq!(task.join().unwrap().len(), 2);
    for change in 0..3 {
        let lease = store
            .begin_detail(
                ACCOUNT,
                &account.authorization_epoch,
                SUBJECT,
                DetailFacet::ReviewThreads,
            )
            .await
            .unwrap();
        let mut entry = saved.entries[0].clone();
        entry.field_validations.clear();
        let Some(NativeDetailPayload::ReviewThreadV1(thread)) = &mut entry.native else {
            panic!("native thread")
        };
        let context = thread.context.clone();
        let Some(native) = thread.native.as_deref_mut() else {
            panic!("native GL")
        };
        let ReviewThreadNativeV1::Gitlab(native) = native;
        match change {
            0 => native.retained_note_count = 52,
            1 => native.position.as_mut().unwrap().head_oid = Some("not-an-oid".into()),
            _ => native.resolved_at = Some("invalid-date".into()),
        }
        let subject = store.detail_subject(ACCOUNT, SUBJECT).await.unwrap();
        let error = store
            .apply_detail(DetailCommit {
                reconciliation: DetailReconciliation {
                    enumeration: DetailEnumeration::FullEnumeration,
                    head_scope: DetailHeadScope::CurrentHead,
                },
                account_id: ACCOUNT.into(),
                authorization_epoch: account.authorization_epoch.clone(),
                authorization_view: lease.authorization_view,
                instance_id: lease.instance_id,
                subject_id: SUBJECT.into(),
                facet: DetailFacet::ReviewThreads,
                run_id: lease.run_id,
                request_cursor: lease.next_cursor,
                body: DetailValue::default(),
                metadata: None,
                subject_binding: Some(DetailSubjectBinding {
                    repository_id: REPOSITORY.into(),
                    repository_provider_id: "123".into(),
                    provider_id: subject.provider_id,
                    number: subject.number,
                    kind: subject.kind,
                    head_oid: subject.head_oid,
                }),
                check_context: None,
                review_context: Some(context),
                entries: vec![entry],
                source: saved.evidence.source.clone().unwrap(),
                next_cursor: None,
                etag: None,
                not_modified: false,
                whole_scope: true,
                complete: true,
                freshness_seconds: 120,
            })
            .await
            .unwrap_err();
        assert_eq!(error.code, ErrorCode::InvalidInput);
        assert_eq!(
            store
                .detail(query(DetailFacet::ReviewThreads))
                .await
                .unwrap()
                .entries,
            saved.entries
        );
    }
    runtime.shutdown().await.unwrap();
}

struct HeldReview {
    inner: providers::gitlab::GitlabProvider,
    entered: Notify,
    release: Notify,
}
#[async_trait::async_trait]
impl CollaborationProvider for HeldReview {
    fn kind(&self) -> ProviderKind {
        ProviderKind::Gitlab
    }
    fn profile(&self, account: &RemoteAccount) -> ProviderProfile {
        self.inner.profile(account)
    }
    async fn probe(&self, _: &SecretToken) -> Result<VerifiedAccount, ProviderError> {
        panic!("fixture must not probe")
    }
    async fn fetch_page(
        &self,
        _: &SecretToken,
        _: FeedRequest,
    ) -> Result<FetchPage, ProviderError> {
        panic!("fixture must not dispatch feeds")
    }
    async fn fetch_detail(
        &self,
        token: &SecretToken,
        request: DetailRequest,
    ) -> Result<DetailPage, ProviderError> {
        self.inner.fetch_detail(token, request).await
    }
    async fn fetch_reviews(
        &self,
        token: &SecretToken,
        request: ReviewRequest,
    ) -> Result<DetailPage, ProviderError> {
        let page = self.inner.fetch_reviews(token, request).await?;
        self.entered.notify_one();
        self.release.notified().await;
        Ok(page)
    }
}
#[tokio::test]
async fn held_gitlab_review_cannot_cross_summary_head_or_authorization_epoch() {
    for replace_epoch in [false, true] {
        let dir = tempfile::tempdir().unwrap();
        let store = Arc::new(Store::open(dir.path().join("held.sqlite")).await.unwrap());
        let account = seed(&store).await;
        let (provider, http) = server(|_| {
            vec![
                response(200, "", &merge_request(999, 123, 67).to_string()),
                response(200, "RateLimit-Remaining: 0\r\n", &discussions()),
            ]
        });
        let provider = Arc::new(HeldReview {
            inner: provider,
            entered: Notify::new(),
            release: Notify::new(),
        });
        let runtime = Arc::new(CollaborationRuntime::new(
            store.clone(),
            Arc::new(Vault::default()),
            provider.clone(),
        ));
        runtime
            .hydrate_detail(HydrateDetailRequest {
                account_id: ACCOUNT.into(),
                authorization_epoch: account.authorization_epoch.clone(),
                subject_id: SUBJECT.into(),
                facet: DetailFacet::ReviewThreads,
            })
            .await
            .unwrap();
        assert!(runtime.run_next().await); // Body context first.
        let job = {
            let runtime = runtime.clone();
            tokio::spawn(async move { runtime.run_next().await })
        };
        tokio::time::timeout(Duration::from_secs(5), provider.entered.notified())
            .await
            .unwrap();
        if replace_epoch {
            store
                .stage_credential(ACCOUNT, "synthetic-replacement-reference")
                .await
                .unwrap();
            store
                .commit_account_credential(
                    RemoteAccount {
                        authorization_epoch: "3".into(),
                        ..account.clone()
                    },
                    "synthetic-replacement-reference",
                )
                .await
                .unwrap();
        } else {
            let mut subject = store.detail_subject(ACCOUNT, SUBJECT).await.unwrap();
            subject.head_oid = Some("e".repeat(40));
            subject.updated_at = "2026-10-09T00:00:00Z".into();
            let scope = format!("repo:{REPOSITORY}:pull_request");
            let run_id = store
                .begin_sync(ACCOUNT, &account.authorization_epoch, &scope)
                .await
                .unwrap();
            store
                .apply_page(PageCommit {
                    account_id: ACCOUNT.into(),
                    authorization_epoch: account.authorization_epoch.clone(),
                    scope,
                    run_id,
                    repositories: vec![],
                    items: vec![subject],
                    endpoint_aliases: vec![],
                    next_cursor: None,
                    etag: None,
                    last_modified: None,
                    not_modified: false,
                    complete: true,
                    observed_at: "2026-10-09T00:00:00Z".into(),
                })
                .await
                .unwrap();
        }
        let after_change = store.revision().await.unwrap();
        provider.release.notify_one();
        assert!(
            tokio::time::timeout(Duration::from_secs(5), job)
                .await
                .unwrap()
                .unwrap()
        );
        let budget = store.scope_state(ACCOUNT, "provider:rest").await.unwrap();
        if replace_epoch {
            assert_eq!(store.revision().await.unwrap(), after_change);
            assert!(budget.is_none_or(|scope| scope.sync.next_retry_at.is_none()));
            assert!(store.detail_subject(ACCOUNT, SUBJECT).await.is_err());
        } else {
            assert!(
                store
                    .detail(query(DetailFacet::ReviewThreads))
                    .await
                    .unwrap()
                    .entries
                    .is_empty()
            );
            assert!(
                budget.unwrap().sync.next_retry_at.is_some(),
                "same-epoch rejected read still consumed provider quota"
            );
        }
        assert_eq!(http.join().unwrap().len(), 2);
        runtime.shutdown().await.unwrap();
    }
}
