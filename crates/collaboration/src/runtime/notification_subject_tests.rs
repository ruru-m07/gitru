//! Deterministic native worker tests: synthetic provider/vault, no live accounts.
use super::*;
use crate::credentials::CredentialError;
use async_trait::async_trait;
use std::sync::{
    Condvar, Mutex as StdMutex,
    atomic::{AtomicI64, AtomicU64, AtomicUsize},
};

const NOTIFICATION: &str = "thread";
const SUBJECT: &str = "canonical-pull";
const AT: &str = "2026-10-03T00:00:00Z";

struct ManualClock {
    base: Instant,
    utc: DateTime<Utc>,
    elapsed: AtomicU64,
    utc_offset: AtomicI64,
}
impl ManualClock {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            base: Instant::now(),
            utc: Utc::now(),
            elapsed: AtomicU64::new(0),
            utc_offset: AtomicI64::new(0),
        })
    }
    fn advance(&self, seconds: u64) {
        self.elapsed.fetch_add(seconds, Ordering::SeqCst);
    }
}
impl clock::Clock for ManualClock {
    fn now(&self) -> Instant {
        self.base + Duration::from_secs(self.elapsed.load(Ordering::SeqCst))
    }
    fn utc(&self) -> DateTime<Utc> {
        self.utc
            + chrono::Duration::seconds(
                self.elapsed.load(Ordering::SeqCst) as i64 + self.utc_offset.load(Ordering::SeqCst),
            )
    }
    fn jitter(&self) -> u64 {
        0
    }
}
#[derive(Default)]
struct Vault {
    tokens: StdMutex<HashMap<String, SecretToken>>,
    loads: AtomicUsize,
    hold: AtomicBool,
    entered: Notify,
    release: (StdMutex<bool>, Condvar),
}
impl Vault {
    fn unblock(&self) {
        *self.release.0.lock().unwrap() = true;
        self.release.1.notify_all();
    }
}
impl CredentialVault for Vault {
    fn load(&self, reference: &str) -> Result<Option<SecretToken>, CredentialError> {
        self.loads.fetch_add(1, Ordering::SeqCst);
        if self.hold.swap(false, Ordering::SeqCst) {
            self.entered.notify_one();
            let mut released = self.release.0.lock().unwrap();
            while !*released {
                released = self.release.1.wait(released).unwrap();
            }
        }
        Ok(self.tokens.lock().unwrap().get(reference).cloned())
    }
    fn store(&self, reference: &str, token: &SecretToken) -> Result<(), CredentialError> {
        self.tokens
            .lock()
            .unwrap()
            .insert(reference.into(), token.clone());
        Ok(())
    }
    fn delete(&self, reference: &str) -> Result<(), CredentialError> {
        self.tokens.lock().unwrap().remove(reference);
        Ok(())
    }
}
#[derive(Clone)]
enum Outcome {
    Verified,
    Unresolved,
    Failed,
    Error(ProviderErrorKind),
}
struct Provider {
    calls: AtomicUsize,
    detail_calls: AtomicUsize,
    supported: bool,
    outcome: StdMutex<Outcome>,
    cooldown: Option<u64>,
    hold: AtomicBool,
    entered: Notify,
    release: Notify,
}
impl Provider {
    fn new(outcome: Outcome) -> Self {
        Self {
            calls: AtomicUsize::new(0),
            detail_calls: AtomicUsize::new(0),
            supported: true,
            outcome: StdMutex::new(outcome),
            cooldown: None,
            hold: AtomicBool::new(false),
            entered: Notify::new(),
            release: Notify::new(),
        }
    }
}
fn body_page() -> DetailPage {
    DetailPage {
        reconciliation: Default::default(),
        body: DetailValue {
            state: DetailValueState::Known,
            text: Some("cached point body".into()),
        },
        entries: vec![],
        source: DetailSource {
            source: "fixture/pull-details".into(),
            adapter_version: 1,
            field_mask: vec![DetailField::Body],
            provider_updated_at: Some(AT.into()),
            observed_at: AT.into(),
        },
        metadata: Some(ResourceMetadataObservation {
            kind: RemoteItemKind::PullRequest,
            values: ResourceMetadataValues {
                title: Some("endpoint title".into()),
                ..Default::default()
            },
            fields: vec![MetadataObservedField {
                field: MetadataField::Title,
                state: DetailValueState::Known,
            }],
            source: MetadataSource {
                source: "fixture/pull-details".into(),
                adapter_version: 1,
                provider_updated_at: Some(AT.into()),
                observed_at: AT.into(),
            },
        }),
        next_cursor: None,
        etag: Some("point-v1".into()),
        not_modified: false,
        freshness_seconds: 60,
        cooldown_seconds: None,
    }
}
#[async_trait]
impl CollaborationProvider for Provider {
    fn kind(&self) -> ProviderKind {
        ProviderKind::Github
    }
    fn profile(&self, _: &RemoteAccount) -> ProviderProfile {
        let mut profile = ProviderProfile::read_only(InboxSemantics::NativeNotifications, true);
        for facet in &mut profile.facets {
            if matches!(
                facet.facet,
                ResourceFacet::PullDetails | ResourceFacet::IssueDetails
            ) {
                facet.state = if self.supported {
                    CapabilityState::Supported
                } else {
                    CapabilityState::Unsupported
                };
                facet.reason = None;
            }
        }
        profile
    }
    fn notification_subject_support(
        &self,
        _: &RemoteAccount,
        _: NotificationSubjectKind,
    ) -> CapabilityState {
        if self.supported {
            CapabilityState::Supported
        } else {
            CapabilityState::Unsupported
        }
    }
    async fn probe(&self, _: &SecretToken) -> Result<VerifiedAccount, ProviderError> {
        Err(ProviderError::new(ProviderErrorKind::InvalidResponse))
    }
    async fn probe_with_backoff(&self, _: &SecretToken) -> Result<VerifiedAccount, ProbeFailure> {
        let mut error = ProviderError::new(ProviderErrorKind::InvalidResponse);
        error.account_cooldown_seconds = self.cooldown;
        Err(ProbeFailure {
            error,
            verified_actor_id: Some("actor-a".into()),
        })
    }
    async fn fetch_page(
        &self,
        _: &SecretToken,
        _: FeedRequest,
    ) -> Result<FetchPage, ProviderError> {
        panic!("point discovery must not reconcile repository feeds")
    }
    async fn fetch_detail(
        &self,
        _: &SecretToken,
        request: DetailRequest,
    ) -> Result<DetailPage, ProviderError> {
        assert_eq!(request.subject.id, SUBJECT);
        assert_eq!(request.facet, DetailFacet::Body);
        self.detail_calls.fetch_add(1, Ordering::SeqCst);
        Ok(body_page())
    }
    async fn discover_notification_subject(
        &self,
        token: &SecretToken,
        request: TrustedNotificationSubjectRequest,
    ) -> Result<NotificationSubjectDiscovery, ProviderError> {
        assert_eq!(token.expose(), "synthetic-point-token");
        assert_eq!(request.notification_id, NOTIFICATION);
        assert_eq!(request.repository.provider_id, "42");
        self.calls.fetch_add(1, Ordering::SeqCst);
        let outcome = self.outcome.lock().unwrap().clone();
        if self.hold.swap(false, Ordering::SeqCst) {
            self.entered.notify_one();
            self.release.notified().await;
        }
        match outcome {
            Outcome::Error(kind) => {
                let mut error = ProviderError::new(kind);
                error.account_cooldown_seconds = self.cooldown;
                Err(error)
            }
            Outcome::Unresolved => Ok(NotificationSubjectDiscovery::Unresolved {
                reason: NotificationSubjectReason::IdentityUnverified,
                cooldown_seconds: self.cooldown,
            }),
            Outcome::Failed => Ok(NotificationSubjectDiscovery::Failed {
                error: ProviderError::new(ProviderErrorKind::InvalidResponse),
                cooldown_seconds: self.cooldown,
            }),
            Outcome::Verified => {
                let subject = RemoteItem {
                    native_inbox: None,
                    id: SUBJECT.into(),
                    account_id: request.account.id,
                    repository_id: Some(request.repository.id),
                    provider_id: "9007199254740997".into(),
                    kind: RemoteItemKind::PullRequest,
                    number: Some(request.selector.number),
                    title: "point summary".into(),
                    body: Some("cached point body".into()),
                    body_omitted: false,
                    author: None,
                    web_url: Some("https://github.com/fixture/project/pull/67".into()),
                    state: "open".into(),
                    updated_at: AT.into(),
                    head_oid: None,
                    is_draft: Some(false),
                    reason: None,
                    unread: None,
                };
                let mut detail = body_page();
                detail.cooldown_seconds = self.cooldown;
                Ok(NotificationSubjectDiscovery::Verified {
                    subject: Box::new(subject),
                    detail: Box::new(detail),
                    endpoint_aliases: vec![],
                })
            }
        }
    }
}
async fn inbox(store: &Store, account: &RemoteAccount, number: Option<&str>) {
    let item = RemoteItem {
        native_inbox: None,
        id: NOTIFICATION.into(),
        account_id: account.id.clone(),
        repository_id: Some("repo".into()),
        provider_id: "9007199254740999".into(),
        kind: RemoteItemKind::Notification,
        number: None,
        title: "thread title".into(),
        body: None,
        body_omitted: true,
        author: None,
        web_url: Some("https://github.com/fixture/project".into()),
        state: "PullRequest".into(),
        updated_at: AT.into(),
        head_oid: None,
        is_draft: None,
        reason: Some("review_requested".into()),
        unread: Some(true),
    };
    let observations = number
        .map(|number| {
            vec![NotificationSubjectObservation {
                notification_id: NOTIFICATION.into(),
                mapping: NotificationSubjectMapping::Selector(NotificationSubjectSelector {
                    subject_provider_id: None,
                    kind: NotificationSubjectKind::PullRequest,
                    repository_provider_id: "42".into(),
                    number: number.into(),
                    repository_path: "fixture/project".into(),
                    representation: NotificationSubjectRepresentation::GithubPullRequest,
                }),
            }]
        })
        .unwrap_or_default();
    let run = store
        .begin_sync(&account.id, &account.authorization_epoch, "notifications")
        .await
        .unwrap();
    store
        .apply_page_with_notification_subjects(
            PageCommit {
                account_id: account.id.clone(),
                authorization_epoch: account.authorization_epoch.clone(),
                scope: "notifications".into(),
                run_id: run,
                repositories: vec![],
                items: if number.is_some() { vec![item] } else { vec![] },
                endpoint_aliases: vec![],
                next_cursor: None,
                etag: None,
                last_modified: None,
                not_modified: false,
                complete: true,
                observed_at: AT.into(),
            },
            observations,
        )
        .await
        .unwrap();
}
async fn fixture(
    path: &std::path::Path,
    provider: Arc<Provider>,
) -> (
    Arc<CollaborationRuntime>,
    Arc<Vault>,
    RemoteAccount,
    Arc<ManualClock>,
) {
    let store = Arc::new(Store::open(path).await.unwrap());
    let account = RemoteAccount {
        id: "a".into(),
        provider: ProviderKind::Github,
        host: "github.com".into(),
        actor_id: "actor-a".into(),
        login: "synthetic".into(),
        display_name: None,
        authorization_epoch: "1".into(),
        state: AccountState::Active,
        notifications_supported: true,
    };
    let vault = Arc::new(Vault::default());
    store
        .stage_credential(&account.id, "synthetic:credential")
        .await
        .unwrap();
    vault
        .store(
            "synthetic:credential",
            &SecretToken::new("synthetic-point-token".into()).unwrap(),
        )
        .unwrap();
    let account = store
        .commit_account_credential(account, "synthetic:credential")
        .await
        .unwrap();
    let run = store
        .begin_sync(&account.id, &account.authorization_epoch, "repositories")
        .await
        .unwrap();
    store
        .apply_page(PageCommit {
            account_id: account.id.clone(),
            authorization_epoch: account.authorization_epoch.clone(),
            scope: "repositories".into(),
            run_id: run,
            repositories: vec![RemoteRepository {
                id: "repo".into(),
                account_id: account.id.clone(),
                provider_id: "42".into(),
                full_name: "fixture/project".into(),
                name: "project".into(),
                web_url: "https://github.com/fixture/project".into(),
                description: None,
                default_branch: None,
                selected: false,
            }],
            items: vec![],
            endpoint_aliases: vec![],
            next_cursor: None,
            etag: None,
            last_modified: None,
            not_modified: false,
            complete: true,
            observed_at: AT.into(),
        })
        .await
        .unwrap();
    inbox(&store, &account, Some("67")).await;
    let clock = ManualClock::new();
    let mut runtime = CollaborationRuntime::new(store, vault.clone(), provider);
    runtime.clock = clock.clone();
    (Arc::new(runtime), vault, account, clock)
}
fn query(account: &RemoteAccount) -> NotificationSubjectQuery {
    NotificationSubjectQuery {
        account_id: account.id.clone(),
        authorization_epoch: account.authorization_epoch.clone(),
        notification_id: NOTIFICATION.into(),
    }
}
async fn discover(runtime: &CollaborationRuntime, account: &RemoteAccount) -> RefreshReceipt {
    let snapshot = runtime.notification_subject(query(account)).await.unwrap();
    runtime
        .discover_notification_subject(DiscoverNotificationSubjectRequest {
            account_id: account.id.clone(),
            authorization_epoch: account.authorization_epoch.clone(),
            notification_id: NOTIFICATION.into(),
            selector_generation: snapshot.selector_generation.unwrap(),
        })
        .await
        .unwrap()
}

#[tokio::test]
async fn cache_only_reads_are_inert_and_explicit_point_coalesces_atomic_native_detail_without_feeds()
 {
    let dir = tempfile::tempdir().unwrap();
    let provider = Arc::new(Provider::new(Outcome::Verified));
    let (runtime, vault, account, clock) =
        fixture(&dir.path().join("cache.sqlite"), provider.clone()).await;
    let before = runtime.store.revision().await.unwrap();
    for _ in 0..3 {
        let missing = runtime.notification_subject(query(&account)).await.unwrap();
        assert_eq!(missing.state, NotificationSubjectState::NotCached);
        assert!(missing.discovery.admission);
    }
    assert_eq!(runtime.store.revision().await.unwrap(), before);
    assert_eq!(vault.loads.load(Ordering::SeqCst), 0);
    assert_eq!(provider.calls.load(Ordering::SeqCst), 0);
    let first = discover(&runtime, &account).await;
    let second = discover(&runtime, &account).await;
    assert_eq!(first.job_id, second.job_id);
    assert_eq!(runtime.scheduler.lock().await.queue.len(), 1);
    let mut hints = runtime.subscribe();
    assert!(runtime.run_next().await);
    assert_eq!(provider.calls.load(Ordering::SeqCst), 1);
    let snapshot = runtime.notification_subject(query(&account)).await.unwrap();
    assert_eq!(snapshot.state, NotificationSubjectState::Resolved);
    assert_eq!(snapshot.subject.unwrap().id, SUBJECT);
    assert!(!snapshot.discovery.admission);
    let saved = runtime
        .store
        .detail(DetailQuery {
            account_id: account.id.clone(),
            subject_id: SUBJECT.into(),
            facet: DetailFacet::Body,
            cursor: None,
            limit: 50,
        })
        .await
        .unwrap();
    assert_eq!(saved.body.text.as_deref(), Some("cached point body"));
    assert_eq!(
        saved.metadata.unwrap().values.title.as_deref(),
        Some("endpoint title")
    );
    let repo = runtime.store.repository(&account.id, "repo").await.unwrap();
    assert!(!repo.selected);
    assert!(
        runtime
            .store
            .scope_state(&account.id, "repo:repo:pull_request")
            .await
            .unwrap()
            .is_none()
    );
    assert!(runtime.store.pending_details().await.unwrap().is_empty());
    let original = runtime
        .store
        .item(&account.id, NOTIFICATION)
        .await
        .unwrap()
        .item
        .unwrap();
    assert_eq!(original.unread, Some(true));
    assert_eq!(original.reason.as_deref(), Some("review_requested"));
    assert_eq!(original.title, "thread title");
    let mut last = None;
    while let Ok(hint) = hints.try_recv() {
        last = Some(hint.revision);
    }
    assert_eq!(last, Some(saved.revision));
    clock.advance(61);
    let owner = runtime
        .demand_owner_activity("inbox-detail-view")
        .await
        .unwrap();
    let owner = runtime
        .set_demand_owner_activity("inbox-detail-view", &owner.generation, true)
        .await
        .unwrap();
    runtime
        .acquire_demand(
            "inbox-detail-view",
            AcquireDemandRequest {
                account_id: account.id.clone(),
                authorization_epoch: account.authorization_epoch.clone(),
                owner_generation: owner.generation,
                target: DemandTarget {
                    kind: DemandTargetKind::Detail,
                    repository_id: None,
                    subject_id: Some(SUBJECT.into()),
                    facet: Some(DetailFacet::Body),
                },
            },
        )
        .await
        .unwrap();
    assert!(runtime.run_next().await);
    assert_eq!(provider.detail_calls.load(Ordering::SeqCst), 1);
    assert_eq!(provider.calls.load(Ordering::SeqCst), 1);
    assert!(runtime.store.pending_details().await.unwrap().is_empty());
}

#[tokio::test]
async fn unsupported_and_old_epoch_queries_never_load_credentials_or_admit_discovery() {
    let dir = tempfile::tempdir().unwrap();
    let mut adapter = Provider::new(Outcome::Verified);
    adapter.supported = false;
    let provider = Arc::new(adapter);
    let (runtime, vault, account, _) =
        fixture(&dir.path().join("cache.sqlite"), provider.clone()).await;
    let snapshot = runtime.notification_subject(query(&account)).await.unwrap();
    assert_eq!(snapshot.discovery.support, CapabilityState::Unsupported);
    assert!(!snapshot.discovery.admission);
    let mut request = DiscoverNotificationSubjectRequest {
        account_id: account.id.clone(),
        authorization_epoch: account.authorization_epoch.clone(),
        notification_id: NOTIFICATION.into(),
        selector_generation: snapshot.selector_generation.unwrap(),
    };
    assert_eq!(
        runtime
            .discover_notification_subject(request.clone())
            .await
            .unwrap_err()
            .code,
        ErrorCode::Unsupported
    );
    request.authorization_epoch = "99".into();
    assert_eq!(
        runtime
            .discover_notification_subject(request)
            .await
            .unwrap_err()
            .code,
        ErrorCode::StaleView
    );
    assert!(
        runtime
            .store
            .pending_notification_subjects()
            .await
            .unwrap()
            .is_empty()
    );
    assert_eq!(provider.calls.load(Ordering::SeqCst), 0);
    assert_eq!(vault.loads.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn unresolved_and_invalid_success_preserve_account_quota_without_automatic_discovery_loop() {
    for outcome in [Outcome::Unresolved, Outcome::Failed] {
        let dir = tempfile::tempdir().unwrap();
        let mut adapter = Provider::new(outcome.clone());
        adapter.cooldown = Some(120);
        let provider = Arc::new(adapter);
        let (runtime, _, account, clock) =
            fixture(&dir.path().join("cache.sqlite"), provider.clone()).await;
        let first = discover(&runtime, &account).await;
        assert!(runtime.run_next().await);
        let snapshot = runtime.notification_subject(query(&account)).await.unwrap();
        assert!(snapshot.subject.is_none());
        assert!(snapshot.discovery.paused);
        assert!(snapshot.discovery.admission);
        assert_eq!(provider.calls.load(Ordering::SeqCst), 1);
        let budget = runtime
            .store
            .scope_state(&account.id, "provider:rest")
            .await
            .unwrap()
            .unwrap()
            .sync
            .next_retry_at
            .unwrap();
        let second = discover(&runtime, &account).await;
        if matches!(outcome, Outcome::Unresolved) {
            assert_ne!(first.job_id, second.job_id);
            assert_eq!(snapshot.state, NotificationSubjectState::IdentityUnverified);
        } else {
            assert_eq!(first.job_id, second.job_id);
        }
        runtime
            .enqueue_pending_notification_subjects()
            .await
            .unwrap();
        assert!(!runtime.run_next().await);
        assert_eq!(provider.calls.load(Ordering::SeqCst), 1);
        assert_eq!(
            runtime
                .store
                .scope_state(&account.id, "provider:rest")
                .await
                .unwrap()
                .unwrap()
                .sync
                .next_retry_at
                .as_deref(),
            Some(budget.as_str())
        );
        clock.advance(121);
        runtime
            .enqueue_pending_notification_subjects()
            .await
            .unwrap();
        assert!(runtime.run_next().await);
        assert_eq!(provider.calls.load(Ordering::SeqCst), 2);
    }
}

#[tokio::test]
async fn three_transient_attempts_survive_restart_then_require_explicit_new_intent_without_clearing_cooldown()
 {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("cache.sqlite");
    let provider = Arc::new(Provider::new(Outcome::Error(ProviderErrorKind::Offline)));
    let (runtime, vault, account, clock) = fixture(&path, provider.clone()).await;
    let first = discover(&runtime, &account).await;
    assert!(runtime.run_next().await);
    assert_eq!(provider.calls.load(Ordering::SeqCst), 1);
    assert_eq!(
        runtime.store.pending_notification_subjects().await.unwrap()[0].attempts,
        1
    );
    let store = runtime.store.clone();
    drop(runtime);
    let mut resumed = CollaborationRuntime::new(store, vault, provider.clone());
    resumed.clock = clock.clone();
    let resumed = Arc::new(resumed);
    resumed
        .enqueue_pending_notification_subjects()
        .await
        .unwrap();
    assert!(!resumed.run_next().await);
    assert_eq!(provider.calls.load(Ordering::SeqCst), 1);
    for seconds in [61, 121] {
        clock.advance(seconds);
        resumed
            .enqueue_pending_notification_subjects()
            .await
            .unwrap();
        assert!(resumed.run_next().await);
    }
    assert_eq!(provider.calls.load(Ordering::SeqCst), 3);
    assert!(
        resumed
            .store
            .pending_notification_subjects()
            .await
            .unwrap()
            .is_empty()
    );
    let exhausted = resumed.notification_subject(query(&account)).await.unwrap();
    assert_eq!(exhausted.state, NotificationSubjectState::Unavailable);
    assert_eq!(
        exhausted.reason,
        Some(NotificationSubjectReason::AttemptsExhausted)
    );
    assert!(exhausted.discovery.admission);
    clock.advance(1000);
    resumed
        .enqueue_pending_notification_subjects()
        .await
        .unwrap();
    assert!(!resumed.run_next().await);
    assert_eq!(provider.calls.load(Ordering::SeqCst), 3);
    let deadline = resumed
        .store
        .scope_state(&account.id, "notification_subject:thread")
        .await
        .unwrap()
        .unwrap()
        .sync
        .next_retry_at;
    let next = discover(&resumed, &account).await;
    assert_ne!(first.job_id, next.job_id);
    assert_eq!(
        resumed.store.pending_notification_subjects().await.unwrap()[0].attempts,
        0
    );
    assert_eq!(
        resumed
            .store
            .scope_state(&account.id, "notification_subject:thread")
            .await
            .unwrap()
            .unwrap()
            .sync
            .next_retry_at,
        deadline
    );
    assert!(resumed.run_next().await);
    assert_eq!(provider.calls.load(Ordering::SeqCst), 4);
}

#[tokio::test]
async fn abandoned_third_attempt_on_recovery_cannot_issue_a_fourth_get() {
    let dir = tempfile::tempdir().unwrap();
    let provider = Arc::new(Provider::new(Outcome::Verified));
    let (runtime, _, account, _) =
        fixture(&dir.path().join("cache.sqlite"), provider.clone()).await;
    discover(&runtime, &account).await;
    let intent = runtime
        .store
        .pending_notification_subjects()
        .await
        .unwrap()
        .pop()
        .unwrap();
    for _ in 0..3 {
        runtime
            .store
            .begin_notification_subject(&intent)
            .await
            .unwrap();
    }
    *runtime.scheduler.lock().await = Scheduler::default();
    runtime
        .enqueue_pending_notification_subjects()
        .await
        .unwrap();
    assert!(
        runtime
            .store
            .pending_notification_subjects()
            .await
            .unwrap()
            .is_empty()
    );
    assert!(!runtime.run_next().await);
    assert_eq!(provider.calls.load(Ordering::SeqCst), 0);
    let snapshot = runtime.notification_subject(query(&account)).await.unwrap();
    assert_eq!(
        snapshot.reason,
        Some(NotificationSubjectReason::AttemptsExhausted)
    );
    assert!(snapshot.discovery.admission);
}

#[tokio::test]
async fn withdrawal_or_budget_during_awaited_vault_access_prevents_provider_dispatch() {
    for withdraw in [true, false] {
        let dir = tempfile::tempdir().unwrap();
        let provider = Arc::new(Provider::new(Outcome::Verified));
        let (runtime, vault, account, _) =
            fixture(&dir.path().join("cache.sqlite"), provider.clone()).await;
        vault.hold.store(true, Ordering::SeqCst);
        discover(&runtime, &account).await;
        let engine = runtime.clone();
        let worker = tokio::spawn(async move { engine.run_next().await });
        vault.entered.notified().await;
        if withdraw {
            // The inherited feed contract confirms absence only after two
            // complete independent enumerations; one omission is provisional.
            inbox(&runtime.store, &account, None).await;
            inbox(&runtime.store, &account, None).await;
        } else {
            runtime
                .store
                .set_sync_status(
                    &account.id,
                    &account.authorization_epoch,
                    "provider:rest",
                    SyncStatus {
                        state: SyncState::RateLimited,
                        last_success_at: None,
                        next_retry_at: Some(runtime.future_string(120)),
                        error: None,
                    },
                )
                .await
                .unwrap();
        }
        vault.unblock();
        assert!(worker.await.unwrap());
        assert_eq!(provider.calls.load(Ordering::SeqCst), 0);
        assert!(
            runtime
                .store
                .item(&account.id, SUBJECT)
                .await
                .unwrap()
                .item
                .is_none()
        );
    }
}

#[tokio::test]
async fn held_success_and_denial_cannot_publish_under_replaced_notification_selector() {
    for outcome in [
        Outcome::Verified,
        Outcome::Error(ProviderErrorKind::Permission),
    ] {
        let dir = tempfile::tempdir().unwrap();
        let provider = Arc::new(Provider::new(outcome));
        provider.hold.store(true, Ordering::SeqCst);
        let (runtime, _, account, _) =
            fixture(&dir.path().join("cache.sqlite"), provider.clone()).await;
        discover(&runtime, &account).await;
        let engine = runtime.clone();
        let worker = tokio::spawn(async move { engine.run_next().await });
        provider.entered.notified().await;
        inbox(&runtime.store, &account, Some("68")).await;
        let fresh = runtime.notification_subject(query(&account)).await.unwrap();
        provider.release.notify_one();
        assert!(worker.await.unwrap());
        let after = runtime.notification_subject(query(&account)).await.unwrap();
        assert_eq!(after.selector_generation, fresh.selector_generation);
        assert_eq!(after.state, NotificationSubjectState::NotCached);
        assert!(after.discovery.sync.error.is_none());
        assert!(
            runtime
                .store
                .item(&account.id, SUBJECT)
                .await
                .unwrap()
                .item
                .is_none()
        );
        assert!(
            runtime
                .store
                .scope_state(&account.id, "notification_subject:thread")
                .await
                .unwrap()
                .is_none()
        );
    }
}

#[tokio::test]
async fn missing_subject_denial_hides_locators_and_cannot_be_cleared_by_manual_retry() {
    for (kind, reason) in [
        (
            ProviderErrorKind::Permission,
            NotificationSubjectReason::PermissionDenied,
        ),
        (
            ProviderErrorKind::NotFound,
            NotificationSubjectReason::NotFound,
        ),
    ] {
        let dir = tempfile::tempdir().unwrap();
        let provider = Arc::new(Provider::new(Outcome::Error(kind)));
        let (runtime, _, account, _) =
            fixture(&dir.path().join("cache.sqlite"), provider.clone()).await;
        let snapshot = runtime.notification_subject(query(&account)).await.unwrap();
        let request = DiscoverNotificationSubjectRequest {
            account_id: account.id.clone(),
            authorization_epoch: account.authorization_epoch.clone(),
            notification_id: NOTIFICATION.into(),
            selector_generation: snapshot.selector_generation.unwrap(),
        };
        runtime
            .discover_notification_subject(request.clone())
            .await
            .unwrap();
        assert!(runtime.run_next().await);
        let denied = runtime.notification_subject(query(&account)).await.unwrap();
        assert_eq!(denied.state, NotificationSubjectState::Unavailable);
        assert_eq!(denied.reason, Some(reason));
        assert!(denied.selector_generation.is_none());
        assert!(denied.subject.is_none());
        assert!(denied.fallback_web_url.is_none());
        assert!(!denied.discovery.admission);
        assert!(
            runtime
                .discover_notification_subject(request)
                .await
                .is_err()
        );
        assert!(
            runtime
                .store
                .pending_notification_subjects()
                .await
                .unwrap()
                .is_empty()
        );
        assert_eq!(provider.calls.load(Ordering::SeqCst), 1);
    }
}

#[tokio::test]
async fn late_selector_receipt_preserves_only_consumed_account_quota_evidence() {
    let dir = tempfile::tempdir().unwrap();
    let mut adapter = Provider::new(Outcome::Verified);
    adapter.cooldown = Some(120);
    let provider = Arc::new(adapter);
    provider.hold.store(true, Ordering::SeqCst);
    let (runtime, _, account, _) =
        fixture(&dir.path().join("cache.sqlite"), provider.clone()).await;
    discover(&runtime, &account).await;
    let engine = runtime.clone();
    let worker = tokio::spawn(async move { engine.run_next().await });
    provider.entered.notified().await;
    inbox(&runtime.store, &account, Some("68")).await;
    let fresh = runtime.notification_subject(query(&account)).await.unwrap();
    provider.release.notify_one();
    assert!(worker.await.unwrap());
    let current = runtime.notification_subject(query(&account)).await.unwrap();
    assert_eq!(current.selector_generation, fresh.selector_generation);
    assert_eq!(current.state, NotificationSubjectState::NotCached);
    assert!(current.discovery.sync.error.is_none());
    assert!(current.discovery.paused);
    assert!(
        runtime
            .store
            .item(&account.id, SUBJECT)
            .await
            .unwrap()
            .item
            .is_none()
    );
    assert!(
        runtime
            .store
            .scope_state(&account.id, "notification_subject:thread")
            .await
            .unwrap()
            .is_none()
    );
    assert_eq!(
        runtime
            .store
            .scope_state(&account.id, "provider:rest")
            .await
            .unwrap()
            .unwrap()
            .sync
            .state,
        SyncState::RateLimited
    );
}

#[tokio::test]
async fn durable_point_intent_limits_are_per_actor_and_global_without_query_or_provider_work() {
    let dir = tempfile::tempdir().unwrap();
    let provider = Arc::new(Provider::new(Outcome::Verified));
    let (runtime, vault, original, _) =
        fixture(&dir.path().join("cache.sqlite"), provider.clone()).await;
    for actor in ["a", "b", "c", "d", "e"] {
        let account = if actor == "a" {
            original.clone()
        } else {
            runtime
                .store
                .upsert_account(RemoteAccount {
                    id: actor.into(),
                    actor_id: format!("actor-{actor}"),
                    ..original.clone()
                })
                .await
                .unwrap()
        };
        if actor != "a" {
            let mut repository = runtime.store.repository("a", "repo").await.unwrap();
            repository.account_id = actor.into();
            let run = runtime
                .store
                .begin_sync(actor, &account.authorization_epoch, "repositories")
                .await
                .unwrap();
            runtime
                .store
                .apply_page(PageCommit {
                    account_id: actor.into(),
                    authorization_epoch: account.authorization_epoch.clone(),
                    scope: "repositories".into(),
                    run_id: run,
                    repositories: vec![repository],
                    items: vec![],
                    endpoint_aliases: vec![],
                    next_cursor: None,
                    etag: None,
                    last_modified: None,
                    not_modified: false,
                    complete: true,
                    observed_at: AT.into(),
                })
                .await
                .unwrap();
        }
        let notification = runtime
            .store
            .item("a", NOTIFICATION)
            .await
            .unwrap()
            .item
            .unwrap();
        let ids: Vec<_> = (0..17).map(|n| format!("bound-{n:02}")).collect();
        let items = ids
            .iter()
            .map(|id| RemoteItem {
                id: id.clone(),
                account_id: actor.into(),
                provider_id: format!("thread-{id}"),
                ..notification.clone()
            })
            .collect();
        let observations = ids
            .iter()
            .map(|id| NotificationSubjectObservation {
                notification_id: id.clone(),
                mapping: NotificationSubjectMapping::Selector(NotificationSubjectSelector {
                    subject_provider_id: None,
                    kind: NotificationSubjectKind::PullRequest,
                    repository_provider_id: "42".into(),
                    number: "67".into(),
                    repository_path: "fixture/project".into(),
                    representation: NotificationSubjectRepresentation::GithubPullRequest,
                }),
            })
            .collect();
        let run = runtime
            .store
            .begin_sync(actor, &account.authorization_epoch, "notifications")
            .await
            .unwrap();
        runtime
            .store
            .apply_page_with_notification_subjects(
                PageCommit {
                    account_id: actor.into(),
                    authorization_epoch: account.authorization_epoch.clone(),
                    scope: "notifications".into(),
                    run_id: run,
                    repositories: vec![],
                    items,
                    endpoint_aliases: vec![],
                    next_cursor: None,
                    etag: None,
                    last_modified: None,
                    not_modified: false,
                    complete: true,
                    observed_at: AT.into(),
                },
                observations,
            )
            .await
            .unwrap();
        for (index, id) in ids.iter().enumerate() {
            let snapshot = runtime
                .notification_subject(NotificationSubjectQuery {
                    account_id: actor.into(),
                    authorization_epoch: account.authorization_epoch.clone(),
                    notification_id: id.clone(),
                })
                .await
                .unwrap();
            let request = DiscoverNotificationSubjectRequest {
                account_id: actor.into(),
                authorization_epoch: account.authorization_epoch.clone(),
                notification_id: id.clone(),
                selector_generation: snapshot.selector_generation.unwrap(),
            };
            let admitted = runtime
                .store
                .request_notification_subject_checked(&request, || Ok(()))
                .await;
            if actor == "e" || index == 16 {
                assert_eq!(admitted.unwrap_err().code, ErrorCode::Busy);
            } else {
                let receipt = admitted.unwrap();
                assert_eq!(
                    runtime
                        .store
                        .request_notification_subject_checked(&request, || Ok(()))
                        .await
                        .unwrap(),
                    receipt
                );
            }
        }
    }
    assert_eq!(
        runtime
            .store
            .pending_notification_subjects()
            .await
            .unwrap()
            .len(),
        64
    );
    assert_eq!(provider.calls.load(Ordering::SeqCst), 0);
    assert_eq!(vault.loads.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn repository_less_active_inbox_row_is_unsupported_without_false_retirement_or_subject_authority()
 {
    let dir = tempfile::tempdir().unwrap();
    let provider = Arc::new(Provider::new(Outcome::Verified));
    let (runtime, vault, account, _) =
        fixture(&dir.path().join("cache.sqlite"), provider.clone()).await;
    let mut notification = runtime
        .store
        .item(&account.id, NOTIFICATION)
        .await
        .unwrap()
        .item
        .unwrap();
    notification.repository_id = None;
    let run = runtime
        .store
        .begin_sync(&account.id, &account.authorization_epoch, "notifications")
        .await
        .unwrap();
    runtime
        .store
        .apply_page(PageCommit {
            account_id: account.id.clone(),
            authorization_epoch: account.authorization_epoch.clone(),
            scope: "notifications".into(),
            run_id: run,
            repositories: vec![],
            items: vec![notification],
            endpoint_aliases: vec![],
            next_cursor: None,
            etag: None,
            last_modified: None,
            not_modified: false,
            complete: true,
            observed_at: AT.into(),
        })
        .await
        .unwrap();
    let snapshot = runtime.notification_subject(query(&account)).await.unwrap();
    assert_eq!(snapshot.state, NotificationSubjectState::Unsupported);
    assert_eq!(
        snapshot.reason,
        Some(NotificationSubjectReason::MissingSelector)
    );
    assert_eq!(snapshot.discovery.support, CapabilityState::Unsupported);
    assert!(!snapshot.discovery.admission);
    assert!(snapshot.selector_generation.is_none());
    assert!(snapshot.subject.is_none());
    assert!(snapshot.fallback_web_url.is_none());
    assert!(
        runtime
            .store
            .item(&account.id, NOTIFICATION)
            .await
            .unwrap()
            .item
            .is_some()
    );
    runtime
        .store
        .set_sync_status(
            &account.id,
            &account.authorization_epoch,
            "notifications",
            SyncStatus {
                state: SyncState::Error,
                last_success_at: None,
                next_retry_at: None,
                error: Some(CollaborationError::new(
                    ErrorCode::PermissionDenied,
                    "fixture permission denied",
                )),
            },
        )
        .await
        .unwrap();
    let denied = runtime.notification_subject(query(&account)).await.unwrap();
    assert_eq!(denied.state, NotificationSubjectState::Unavailable);
    assert_eq!(
        denied.reason,
        Some(NotificationSubjectReason::PermissionDenied)
    );
    assert!(
        runtime
            .store
            .item(&account.id, NOTIFICATION)
            .await
            .unwrap()
            .item
            .is_none()
    );
    assert_eq!(provider.calls.load(Ordering::SeqCst), 0);
    assert_eq!(vault.loads.load(Ordering::SeqCst), 0);
}

// A genuine synthetic adapter observation enters the accepted-epoch Runtime seam;
// this does not manually populate the scheduler or claim a concurrent live probe.
async fn clock_quota_receipt(
    runtime: &CollaborationRuntime,
    account: &RemoteAccount,
    provider: &Provider,
) {
    let observation = provider
        .probe_with_backoff(&SecretToken::new("synthetic_clock_receipt".into()).unwrap())
        .await
        .unwrap_err();
    assert_eq!(
        observation.verified_actor_id.as_deref(),
        Some(account.actor_id.as_str())
    );
    runtime
        .persist_rate_limit(
            account,
            observation.error.account_cooldown_seconds.unwrap(),
            None,
        )
        .await
        .unwrap();
}

struct ClockVaultRelease(Arc<Vault>);
impl Drop for ClockVaultRelease {
    fn drop(&mut self) {
        self.0.unblock();
    }
}

async fn discovery_clock_gate(before_vault: bool) {
    use std::future::{Future, poll_fn};
    use std::task::Poll;
    {
        let dir = tempfile::tempdir().unwrap();
        let mut configured = Provider::new(Outcome::Verified);
        configured.cooldown = Some(120);
        let provider = Arc::new(configured);
        let (runtime, vault, account, clock) =
            fixture(&dir.path().join("cache.sqlite"), provider.clone()).await;
        let _release = ClockVaultRelease(vault.clone());
        discover(&runtime, &account).await;
        let lifecycle = if before_vault {
            Some(runtime.lifecycle.lock().await)
        } else {
            None
        };
        if !before_vault {
            vault.hold.store(true, Ordering::SeqCst);
        }
        let mut worker = Box::pin(runtime.run_next());
        tokio::time::timeout(Duration::from_secs(10), async {
            if before_vault {
                loop {
                    poll_fn(|cx| {
                        assert!(worker.as_mut().poll(cx).is_pending());
                        Poll::Ready(())
                    })
                    .await;
                    let s = runtime.scheduler.lock().await;
                    let key = s
                        .active
                        .iter()
                        .find(|(key, _)| key.ends_with(":notification_subject:thread"))
                        .map(|(key, _)| key)
                        .unwrap();
                    if s.queue.iter().chain(&s.deferred).all(|job| &job.key != key) {
                        assert!(runtime.dispatch.try_lock().is_err());
                        break;
                    }
                    drop(s);
                    tokio::task::yield_now().await;
                }
            } else {
                tokio::select! {
                    _ = vault.entered.notified() => {},
                    _ = &mut worker => panic!("worker escaped held vault"),
                }
            }
        })
        .await
        .unwrap();
        clock_quota_receipt(&runtime, &account, &provider).await;
        let original = runtime
            .store
            .scope_state(&account.id, "provider:rest")
            .await
            .unwrap()
            .unwrap()
            .sync
            .next_retry_at;
        clock.utc_offset.fetch_add(600, Ordering::SeqCst);
        drop(lifecycle);
        vault.unblock();
        assert!(
            tokio::time::timeout(Duration::from_secs(10), worker)
                .await
                .unwrap()
        );
        assert_eq!(
            vault.loads.load(Ordering::SeqCst),
            usize::from(!before_vault),
            "discovery must not add a vault read after quota acceptance"
        );
        assert_eq!(
            provider.calls.load(Ordering::SeqCst),
            0,
            "picked discovery must retain the live lower bound"
        );
        assert_eq!(
            runtime
                .store
                .scope_state(&account.id, "provider:rest")
                .await
                .unwrap()
                .unwrap()
                .sync
                .next_retry_at,
            original,
            "local refusal cannot invent a provider observation at shifted UTC"
        );
        assert!(!runtime.run_next().await);
        assert!(
            runtime
                .store
                .item(&account.id, SUBJECT)
                .await
                .unwrap()
                .item
                .is_none()
        );
    }
}

#[tokio::test]
async fn discovery_clock_live_budget_blocks_picked_lifecycle_before_vault() {
    discovery_clock_gate(true).await;
}
#[tokio::test]
async fn discovery_clock_live_budget_blocks_after_native_vault() {
    discovery_clock_gate(false).await;
}
