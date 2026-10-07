//! Clock-driven scheduler and caller-interest contracts; no live provider or vault.
use super::clock::Clock;
use super::detail_tests::fixtures;
use super::*;
use crate::credentials::CredentialError;
use async_trait::async_trait;
use std::sync::{
    Mutex as StdMutex,
    atomic::{AtomicU64, AtomicUsize},
};

struct ManualClock {
    base: Instant,
    elapsed: AtomicU64,
}
impl ManualClock {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            base: Instant::now(),
            elapsed: AtomicU64::new(0),
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
        DateTime::parse_from_rfc3339("2026-10-03T00:00:00Z")
            .unwrap()
            .with_timezone(&Utc)
            + chrono::Duration::seconds(self.elapsed.load(Ordering::SeqCst) as i64)
    }
    fn jitter(&self) -> u64 {
        0
    }
}
#[derive(Default)]
struct Vault {
    tokens: StdMutex<HashMap<String, SecretToken>>,
    loads: AtomicUsize,
}
impl CredentialVault for Vault {
    fn load(&self, reference: &str) -> Result<Option<SecretToken>, CredentialError> {
        self.loads.fetch_add(1, Ordering::SeqCst);
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
struct Provider {
    requests: StdMutex<Vec<String>>,
    pages: usize,
    error: StdMutex<Option<ProviderError>>,
    hold_first: AtomicBool,
    entered: Notify,
    release: Notify,
}
impl Provider {
    fn new(pages: usize) -> Arc<Self> {
        Arc::new(Self {
            requests: StdMutex::new(vec![]),
            pages,
            error: StdMutex::new(None),
            hold_first: AtomicBool::new(false),
            entered: Notify::new(),
            release: Notify::new(),
        })
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
                ResourceFacet::PullDetails | ResourceFacet::Comments
            ) {
                facet.state = CapabilityState::Supported;
                facet.reason = None;
            }
        }
        profile
    }
    async fn probe(&self, _: &SecretToken) -> Result<VerifiedAccount, ProviderError> {
        panic!("fixture credentials are explicit")
    }
    async fn fetch_page(
        &self,
        _: &SecretToken,
        request: FeedRequest,
    ) -> Result<FetchPage, ProviderError> {
        let index = request
            .cursor
            .as_deref()
            .unwrap_or("1")
            .parse::<usize>()
            .unwrap();
        self.requests.lock().unwrap().push(format!(
            "{}:feed:{}:{index}",
            request.account.id,
            request.kind.facet() as u8
        ));
        if self.hold_first.swap(false, Ordering::SeqCst) {
            self.entered.notify_one();
            self.release.notified().await;
        }
        if let Some(error) = self.error.lock().unwrap().clone() {
            return Err(error);
        }
        Ok(FetchPage {
            repositories: vec![],
            items: vec![],
            endpoint_aliases: vec![],
            notification_subjects: vec![],
            next_cursor: (index < self.pages).then(|| (index + 1).to_string()),
            etag: None,
            last_modified: None,
            not_modified: false,
            poll_interval_seconds: None,
            cooldown_seconds: None,
        })
    }
    async fn fetch_detail(
        &self,
        _: &SecretToken,
        request: DetailRequest,
    ) -> Result<DetailPage, ProviderError> {
        let index = request
            .cursor
            .as_deref()
            .unwrap_or("1")
            .parse::<usize>()
            .unwrap();
        self.requests
            .lock()
            .unwrap()
            .push(format!("{}:detail:{index}", request.account.id));
        if let Some(error) = self.error.lock().unwrap().clone() {
            return Err(error);
        }
        Ok(DetailPage {
            reconciliation: DetailReconciliation::full_history(),
            body: if request.facet == DetailFacet::Body {
                fixtures::known(Some("native cached body"))
            } else {
                DetailValue::default()
            },
            metadata: None,
            entries: if request.facet == DetailFacet::Comments {
                vec![fixtures::entry(&format!("entry-{index}"))]
            } else {
                vec![]
            },
            source: fixtures::source(request.facet),
            next_cursor: (request.facet != DetailFacet::Body && index < self.pages)
                .then(|| (index + 1).to_string()),
            etag: Some("fixture-etag".into()),
            not_modified: false,
            freshness_seconds: 60,
            cooldown_seconds: None,
        })
    }
}
async fn setup(
    path: &std::path::Path,
    provider: Arc<Provider>,
) -> (
    CollaborationRuntime,
    Arc<ManualClock>,
    Arc<Vault>,
    RemoteAccount,
) {
    let store = Arc::new(Store::open(path).await.unwrap());
    let vault = Arc::new(Vault::default());
    let account = add_actor(&store, &vault, "a").await;
    let clock = ManualClock::new();
    let mut runtime = CollaborationRuntime::new(store, vault.clone(), provider);
    runtime.clock = clock.clone();
    (runtime, clock, vault, account)
}
async fn add_actor(store: &Store, vault: &Vault, id: &str) -> RemoteAccount {
    let actor = store.upsert_account(fixtures::account(id)).await.unwrap();
    let reference = format!("fixture-{id}");
    store.stage_credential(id, &reference).await.unwrap();
    vault
        .store(
            &reference,
            &SecretToken::new("native_demand_fixture".into()).unwrap(),
        )
        .unwrap();
    let actor = store
        .commit_account_credential(
            RemoteAccount {
                authorization_epoch: "2".into(),
                ..actor
            },
            &reference,
        )
        .await
        .unwrap();
    fixtures::project(store, &actor).await;
    actor
}
async fn activate(runtime: &CollaborationRuntime, owner: &str) -> DemandOwnerActivity {
    let activity = runtime.demand_owner_activity(owner).await.unwrap();
    runtime
        .set_demand_owner_activity(owner, &activity.generation, true)
        .await
        .unwrap()
}
fn target(kind: DemandTargetKind) -> DemandTarget {
    DemandTarget {
        kind,
        repository_id: None,
        subject_id: None,
        facet: None,
    }
}
fn detail(facet: DetailFacet) -> DemandTarget {
    DemandTarget {
        kind: DemandTargetKind::Detail,
        repository_id: None,
        subject_id: Some("pull".into()),
        facet: Some(facet),
    }
}
async fn acquire(
    runtime: &CollaborationRuntime,
    owner: &str,
    activity: &DemandOwnerActivity,
    account: &RemoteAccount,
    target: DemandTarget,
) -> DemandLeaseReceipt {
    runtime
        .acquire_demand(
            owner,
            AcquireDemandRequest {
                account_id: account.id.clone(),
                authorization_epoch: account.authorization_epoch.clone(),
                owner_generation: activity.generation.clone(),
                target,
            },
        )
        .await
        .unwrap()
}
fn renewal(
    activity: &DemandOwnerActivity,
    account: &RemoteAccount,
    lease: &DemandLeaseReceipt,
) -> RenewDemandRequest {
    RenewDemandRequest {
        owner_generation: activity.generation.clone(),
        leases: vec![DemandLeaseRenewal {
            lease_id: lease.lease_id.clone(),
            account_id: account.id.clone(),
            authorization_epoch: account.authorization_epoch.clone(),
        }],
    }
}

#[tokio::test]
async fn expiry_owner_reopen_foreign_ids_and_atomic_batch_are_fenced_without_io() {
    let dir = tempfile::tempdir().unwrap();
    let provider = Provider::new(1);
    let (runtime, clock, vault, account) =
        setup(&dir.path().join("cache.sqlite"), provider.clone()).await;
    let a = activate(&runtime, "view-a").await;
    let b = activate(&runtime, "view-b").await;
    let first = acquire(&runtime, "view-a", &a, &account, detail(DetailFacet::Body)).await;
    let second = acquire(
        &runtime,
        "view-a",
        &a,
        &account,
        target(DemandTargetKind::Inbox),
    )
    .await;
    assert_eq!(
        runtime
            .release_demand(
                "view-b",
                ReleaseDemandRequest {
                    lease_id: first.lease_id.clone()
                }
            )
            .await
            .unwrap_err()
            .code,
        ErrorCode::PermissionDenied
    );
    clock.advance(30);
    let mut batch = renewal(&a, &account, &first);
    batch.leases.push(DemandLeaseRenewal {
        lease_id: "missing".into(),
        account_id: account.id.clone(),
        authorization_epoch: account.authorization_epoch.clone(),
    });
    assert_eq!(
        runtime
            .renew_demand("view-a", batch)
            .await
            .unwrap_err()
            .code,
        ErrorCode::StaleView
    );
    clock.advance(15);
    assert_eq!(
        runtime
            .renew_demand("view-a", renewal(&a, &account, &first))
            .await
            .unwrap_err()
            .code,
        ErrorCode::StaleView
    );
    assert_eq!(
        runtime
            .renew_demand("view-a", renewal(&a, &account, &second))
            .await
            .unwrap_err()
            .code,
        ErrorCode::StaleView
    );
    runtime
        .dispose_demand_owner("view-a", &a.generation)
        .await
        .unwrap();
    let reopened = activate(&runtime, "view-a").await;
    assert!(reopened.generation.parse::<u64>().unwrap() > a.generation.parse::<u64>().unwrap());
    assert_eq!(
        runtime
            .set_demand_owner_activity("view-a", &a.generation, false)
            .await
            .unwrap_err()
            .code,
        ErrorCode::StaleView
    );
    assert!(b.active);
    assert_eq!(provider.requests.lock().unwrap().len(), 0);
    assert_eq!(vault.loads.load(Ordering::SeqCst), 0);
    assert!(runtime.store.pending_details().await.unwrap().is_empty());
    assert!(!runtime.run_next().await);
}

#[tokio::test]
async fn equivalent_views_coalesce_hidden_native_probe_removes_priority_and_same_label_can_reactivate()
 {
    let dir = tempfile::tempdir().unwrap();
    let provider = Provider::new(1);
    let (runtime, _, vault, account) =
        setup(&dir.path().join("cache.sqlite"), provider.clone()).await;
    let visible = Arc::new(AtomicBool::new(true));
    let probe = visible.clone();
    let runtime =
        runtime.with_demand_visibility_probe(Arc::new(move |_| probe.load(Ordering::SeqCst)));
    let a = activate(&runtime, "one").await;
    let b = activate(&runtime, "two").await;
    acquire(&runtime, "one", &a, &account, detail(DetailFacet::Body)).await;
    acquire(&runtime, "two", &b, &account, detail(DetailFacet::Body)).await;
    runtime.enqueue_foreground().await.unwrap();
    assert_eq!(runtime.scheduler.lock().await.queue.len(), 1);
    visible.store(false, Ordering::SeqCst);
    assert!(!runtime.run_next().await);
    assert_eq!(vault.loads.load(Ordering::SeqCst), 0);
    assert!(runtime.scheduler.lock().await.demands.leases.is_empty());
    assert!(!runtime.demand_owner_activity("one").await.unwrap().active);
    visible.store(true, Ordering::SeqCst);
    let new = activate(&runtime, "one").await;
    acquire(&runtime, "one", &new, &account, detail(DetailFacet::Body)).await;
    assert!(runtime.run_next().await);
    assert_eq!(provider.requests.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn idle_renewal_is_not_refresh_and_due_lease_continues_without_durable_intent() {
    let dir = tempfile::tempdir().unwrap();
    let provider = Provider::new(1);
    let (runtime, clock, _, account) =
        setup(&dir.path().join("cache.sqlite"), provider.clone()).await;
    let activity = activate(&runtime, "view").await;
    let lease = acquire(
        &runtime,
        "view",
        &activity,
        &account,
        detail(DetailFacet::Body),
    )
    .await;
    assert!(runtime.run_next().await);
    for _ in 0..3 {
        clock.advance(15);
        runtime
            .renew_demand("view", renewal(&activity, &account, &lease))
            .await
            .unwrap();
        assert!(!runtime.run_next().await);
    }
    clock.advance(15);
    runtime
        .renew_demand("view", renewal(&activity, &account, &lease))
        .await
        .unwrap();
    assert!(runtime.run_next().await);
    assert_eq!(provider.requests.lock().unwrap().len(), 2);
    assert!(runtime.store.pending_details().await.unwrap().is_empty());
    runtime
        .release_demand(
            "view",
            ReleaseDemandRequest {
                lease_id: lease.lease_id,
            },
        )
        .await
        .unwrap();
    clock.advance(60);
    assert!(!runtime.run_next().await);
    let saved = runtime
        .store
        .detail(fixtures::query("a", DetailFacet::Body))
        .await
        .unwrap();
    assert_eq!(saved.body.text.as_deref(), Some("native cached body"));
}

#[tokio::test]
async fn reconnect_and_deselection_reject_old_interest_without_reading_credentials() {
    let dir = tempfile::tempdir().unwrap();
    let provider = Provider::new(1);
    let (runtime, _, vault, account) =
        setup(&dir.path().join("cache.sqlite"), provider.clone()).await;
    let activity = activate(&runtime, "view").await;
    let lease = acquire(
        &runtime,
        "view",
        &activity,
        &account,
        detail(DetailFacet::Body),
    )
    .await;
    runtime.enqueue_foreground().await.unwrap();
    runtime
        .store
        .upsert_account(RemoteAccount {
            authorization_epoch: "3".into(),
            ..account.clone()
        })
        .await
        .unwrap();
    assert_eq!(
        runtime
            .renew_demand("view", renewal(&activity, &account, &lease))
            .await
            .unwrap_err()
            .code,
        ErrorCode::StaleView
    );
    assert!(!runtime.run_next().await);
    assert_eq!(vault.loads.load(Ordering::SeqCst), 0);
    let current = runtime.store.account("a").await.unwrap();
    fixtures::project(&runtime.store, &current).await;
    acquire(
        &runtime,
        "view",
        &activity,
        &current,
        detail(DetailFacet::Body),
    )
    .await;
    runtime.enqueue_foreground().await.unwrap();
    runtime.select_repository("a", "repo", false).await.unwrap();
    assert!(!runtime.run_next().await);
    assert_eq!(provider.requests.lock().unwrap().len(), 0);
    assert_eq!(vault.loads.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn quota_and_offline_renewal_preserve_barriers_and_retry_history() {
    let dir = tempfile::tempdir().unwrap();
    let provider = Provider::new(1);
    let (runtime, clock, _, account) =
        setup(&dir.path().join("cache.sqlite"), provider.clone()).await;
    let activity = activate(&runtime, "view").await;
    let lease = acquire(
        &runtime,
        "view",
        &activity,
        &account,
        detail(DetailFacet::Body),
    )
    .await;
    *provider.error.lock().unwrap() = Some(ProviderError::new(ProviderErrorKind::Offline));
    assert!(runtime.run_next().await);
    let key = format!("a:2:{}", DetailFacet::Body.scope("pull"));
    let retry = runtime
        .store
        .scope_state("a", &DetailFacet::Body.scope("pull"))
        .await
        .unwrap()
        .unwrap()
        .sync
        .next_retry_at;
    for _ in 0..3 {
        clock.advance(15);
        runtime
            .renew_demand("view", renewal(&activity, &account, &lease))
            .await
            .unwrap();
        assert!(!runtime.run_next().await);
        assert_eq!(runtime.scheduler.lock().await.failures[&key], 1);
    }
    assert_eq!(
        runtime
            .store
            .scope_state("a", &DetailFacet::Body.scope("pull"))
            .await
            .unwrap()
            .unwrap()
            .sync
            .next_retry_at,
        retry
    );
    clock.advance(15);
    runtime
        .renew_demand("view", renewal(&activity, &account, &lease))
        .await
        .unwrap();
    assert!(runtime.run_next().await);
    assert_eq!(runtime.scheduler.lock().await.failures[&key], 2);
    *provider.error.lock().unwrap() = None;
    runtime
        .persist_rate_limit(&account, 300, None)
        .await
        .unwrap();
    runtime.persist_rate_limit(&account, 1, None).await.unwrap();
    for _ in 0..19 {
        clock.advance(15);
        runtime
            .renew_demand("view", renewal(&activity, &account, &lease))
            .await
            .unwrap();
        assert!(!runtime.run_next().await);
    }
    assert_eq!(provider.requests.lock().unwrap().len(), 2);
    clock.advance(15);
    runtime
        .renew_demand("view", renewal(&activity, &account, &lease))
        .await
        .unwrap();
    assert!(runtime.run_next().await);
    assert_eq!(provider.requests.lock().unwrap().len(), 3);
    assert!(!runtime.scheduler.lock().await.failures.contains_key(&key));
}

#[tokio::test]
async fn foreground_detail_runs_between_committed_background_pages_without_changing_membership_run()
{
    let dir = tempfile::tempdir().unwrap();
    let provider = Provider::new(12);
    let (runtime, clock, _, account) =
        setup(&dir.path().join("cache.sqlite"), provider.clone()).await;
    clock.advance(180);
    let runtime = Arc::new(runtime);
    let repo = runtime.store.repository("a", "repo").await.unwrap();
    runtime
        .enqueue_work_reason(
            account.clone(),
            Some(repo),
            JobKind::Feed(FeedKind::PullRequests),
            "repo:repo:pull_request".into(),
            scheduler::Admission::Reconcile,
        )
        .await
        .unwrap();
    provider.hold_first.store(true, Ordering::SeqCst);
    let first = {
        let runtime = runtime.clone();
        tokio::spawn(async move { runtime.run_next().await })
    };
    tokio::time::timeout(Duration::from_secs(5), provider.entered.notified())
        .await
        .unwrap();
    let activity = activate(&runtime, "view").await;
    acquire(
        &runtime,
        "view",
        &activity,
        &account,
        detail(DetailFacet::Body),
    )
    .await;
    // A concurrent wake does not occupy a second provider lane.
    assert!(!runtime.run_next().await);
    provider.release.notify_one();
    assert!(first.await.unwrap());
    let checkpoint = runtime
        .store
        .scope_state("a", "repo:repo:pull_request")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(checkpoint.next_cursor.as_deref(), Some("2"));
    assert!(runtime.run_next().await);
    assert!(runtime.run_next().await);
    let after = runtime
        .store
        .scope_state("a", "repo:repo:pull_request")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(checkpoint.run_id, after.run_id);
    assert_eq!(after.next_cursor.as_deref(), Some("3"));
    let requests = provider.requests.lock().unwrap();
    assert_eq!(requests.len(), 3);
    assert!(requests[0].contains(":feed:"));
    assert_eq!(requests[1], "a:detail:1");
    assert!(requests[2].ends_with(":2"));
}

#[tokio::test]
async fn expired_detail_yields_no_continuation_and_new_interest_resumes_saved_checkpoint() {
    let dir = tempfile::tempdir().unwrap();
    let provider = Provider::new(3);
    let (runtime, clock, _, account) =
        setup(&dir.path().join("cache.sqlite"), provider.clone()).await;
    let activity = activate(&runtime, "view").await;
    acquire(
        &runtime,
        "view",
        &activity,
        &account,
        detail(DetailFacet::Comments),
    )
    .await;
    assert!(runtime.run_next().await);
    clock.advance(45);
    assert!(!runtime.run_next().await);
    assert_eq!(provider.requests.lock().unwrap().len(), 1);
    assert_eq!(
        runtime
            .store
            .detail(fixtures::query("a", DetailFacet::Comments))
            .await
            .unwrap()
            .entries
            .len(),
        1
    );
    acquire(
        &runtime,
        "view",
        &activity,
        &account,
        detail(DetailFacet::Comments),
    )
    .await;
    assert!(runtime.run_next().await);
    assert_eq!(
        provider.requests.lock().unwrap().last().unwrap(),
        "a:detail:2"
    );
    assert!(runtime.run_next().await);
    assert_eq!(
        runtime
            .store
            .detail(fixtures::query("a", DetailFacet::Comments))
            .await
            .unwrap()
            .entries
            .len(),
        3
    );
    assert!(runtime.store.pending_details().await.unwrap().is_empty());
}

fn queued(account: &RemoteAccount, index: usize, reason: scheduler::Admission) -> Job {
    let scope = format!("test-{index}");
    Job {
        key: format!("{}:{}:{scope}", account.id, account.authorization_epoch),
        account: account.clone(),
        repository: None,
        kind: JobKind::Feed(FeedKind::Notifications),
        scope,
        reason,
        pages: 0,
        detail_lease: None,
        detail_restarted: false,
        pull_commit_lease: None,
        pull_commit_restarted: false,
        local_budget_refusal: false,
        enqueued_at: Instant::now(),
    }
}

#[tokio::test]
async fn fair_arbitration_reserves_background_turns_and_rotates_accounts_and_scopes() {
    let clock = ManualClock::new();
    let a = fixtures::account("a");
    let b = fixtures::account("b");
    let c = fixtures::account("c");
    let mut scheduler = Scheduler::default();
    for i in 0..2 {
        for account in [&a, &b] {
            scheduler
                .queue
                .push_back(queued(account, i, scheduler::Admission::Manual));
        }
    }
    scheduler
        .queue
        .push_back(queued(&c, 0, scheduler::Admission::Reconcile));
    let mut order = vec![];
    for _ in 0..12 {
        let job = scheduler.pick(clock.now()).unwrap();
        order.push((job.account.id.clone(), job.scope.clone()));
        scheduler.queue.push_back(job);
    }
    assert_eq!(order.iter().filter(|(id, _)| id == "c").count(), 3);
    for i in [3, 7, 11] {
        assert_eq!(order[i].0, "c");
    }
    assert_eq!(order[0], ("a".into(), "test-0".into()));
    assert_eq!(order[1], ("b".into(), "test-0".into()));
    assert_eq!(order[2], ("a".into(), "test-1".into()));
    assert_eq!(order[4], ("b".into(), "test-1".into()));
}

#[tokio::test]
async fn blocked_accounts_do_not_consume_ready_capacity_and_manual_receipts_resume_at_exact_deadline()
 {
    let dir = tempfile::tempdir().unwrap();
    let provider = Provider::new(1);
    let (runtime, clock, vault, account) =
        setup(&dir.path().join("cache.sqlite"), provider.clone()).await;
    let other = add_actor(&runtime.store, &vault, "b").await;
    runtime
        .persist_rate_limit(&account, 60, None)
        .await
        .unwrap();
    for i in 0..40 {
        assert!(
            runtime
                .enqueue_work_reason(
                    account.clone(),
                    None,
                    JobKind::Feed(FeedKind::Notifications),
                    format!("blocked-{i}"),
                    scheduler::Admission::Foreground
                )
                .await
                .unwrap()
                .is_empty()
        );
    }
    let id = runtime
        .enqueue_work_reason(
            account.clone(),
            None,
            JobKind::Feed(FeedKind::Notifications),
            "notifications".into(),
            scheduler::Admission::Manual,
        )
        .await
        .unwrap();
    assert!(!id.is_empty());
    assert_eq!(runtime.scheduler.lock().await.queue.len(), 0);
    assert_eq!(runtime.scheduler.lock().await.deferred.len(), 1);
    runtime
        .enqueue_work_reason(
            other,
            None,
            JobKind::Feed(FeedKind::Notifications),
            "notifications".into(),
            scheduler::Admission::Reconcile,
        )
        .await
        .unwrap();
    assert!(runtime.run_next().await);
    assert_eq!(provider.requests.lock().unwrap().len(), 1);
    assert!(provider.requests.lock().unwrap()[0].starts_with("b:"));
    clock.advance(59);
    assert!(!runtime.run_next().await);
    clock.advance(1);
    assert!(runtime.run_next().await);
    assert_eq!(provider.requests.lock().unwrap().len(), 2);
    assert!(runtime.scheduler.lock().await.deferred.is_empty());
}

#[tokio::test]
async fn ready_reservations_and_lease_bounds_hold_under_saturation() {
    let dir = tempfile::tempdir().unwrap();
    let provider = Provider::new(1);
    let (runtime, _, vault, account) =
        setup(&dir.path().join("cache.sqlite"), provider.clone()).await;
    for i in 0..96 {
        runtime
            .enqueue_work_reason(
                account.clone(),
                None,
                JobKind::Feed(FeedKind::Repositories),
                format!("background-{i}"),
                scheduler::Admission::Reconcile,
            )
            .await
            .unwrap();
    }
    let activity = activate(&runtime, "owner-0").await;
    acquire(
        &runtime,
        "owner-0",
        &activity,
        &account,
        detail(DetailFacet::Body),
    )
    .await;
    runtime.enqueue_foreground().await.unwrap();
    for i in 0..31 {
        runtime
            .enqueue_work_reason(
                account.clone(),
                None,
                JobKind::Feed(FeedKind::Notifications),
                format!("manual-{i}"),
                scheduler::Admission::Manual,
            )
            .await
            .unwrap();
    }
    assert_eq!(runtime.scheduler.lock().await.queue.len(), 128);
    assert_eq!(
        runtime
            .enqueue_work_reason(
                account.clone(),
                None,
                JobKind::Feed(FeedKind::Notifications),
                "overflow".into(),
                scheduler::Admission::Manual
            )
            .await
            .unwrap_err()
            .code,
        ErrorCode::Busy
    );
    for owner_index in 0..8 {
        let owner = format!("owner-{owner_index}");
        let activity = activate(&runtime, &owner).await;
        let existing = usize::from(owner_index == 0);
        for _ in existing..16 {
            acquire(
                &runtime,
                &owner,
                &activity,
                &account,
                detail(DetailFacet::Body),
            )
            .await;
        }
        assert_eq!(
            runtime
                .acquire_demand(
                    &owner,
                    AcquireDemandRequest {
                        account_id: account.id.clone(),
                        authorization_epoch: account.authorization_epoch.clone(),
                        owner_generation: activity.generation,
                        target: detail(DetailFacet::Body)
                    }
                )
                .await
                .unwrap_err()
                .code,
            ErrorCode::Busy
        );
    }
    let activity = activate(&runtime, "ninth-owner").await;
    assert_eq!(
        runtime
            .acquire_demand(
                "ninth-owner",
                AcquireDemandRequest {
                    account_id: account.id.clone(),
                    authorization_epoch: account.authorization_epoch.clone(),
                    owner_generation: activity.generation,
                    target: detail(DetailFacet::Body)
                }
            )
            .await
            .unwrap_err()
            .code,
        ErrorCode::Busy
    );
    assert_eq!(runtime.scheduler.lock().await.demands.leases.len(), 128);
    assert_eq!(provider.requests.lock().unwrap().len(), 0);
    assert_eq!(vault.loads.load(Ordering::SeqCst), 0);
    let first = runtime.scheduler.lock().await.pick(runtime.now()).unwrap();
    assert!(matches!(first.kind, JobKind::Detail { .. }));
}

#[tokio::test]
async fn rotating_explicit_admission_reaches_intent_beyond_a_fresh_128_subject_prefix() {
    let dir = tempfile::tempdir().unwrap();
    let provider = Provider::new(1);
    let (runtime, _, _, account) = setup(&dir.path().join("cache.sqlite"), provider.clone()).await;
    let original = runtime.store.detail_subject("a", "pull").await.unwrap();
    let mut subjects = vec![];
    for i in 0..130 {
        let mut item = original.clone();
        item.id = format!("subject-{i:03}");
        item.provider_id = (10000 + i).to_string();
        item.number = Some((1000 + i).to_string());
        subjects.push(item);
    }
    let run = runtime
        .store
        .begin_sync("a", &account.authorization_epoch, "repo:repo:pull_request")
        .await
        .unwrap();
    for (index, chunk) in subjects.chunks(65).enumerate() {
        runtime
            .store
            .apply_page(PageCommit {
                account_id: "a".into(),
                authorization_epoch: account.authorization_epoch.clone(),
                scope: "repo:repo:pull_request".into(),
                run_id: run.clone(),
                repositories: vec![],
                items: chunk.to_vec(),
                endpoint_aliases: vec![],
                next_cursor: (index == 0).then(|| "fixture-page-two".into()),
                etag: None,
                last_modified: None,
                not_modified: false,
                complete: index == 1,
                observed_at: runtime.now_string(),
            })
            .await
            .unwrap();
    }
    for (index, item) in subjects.iter().enumerate() {
        if index < 129 {
            let lease = runtime
                .store
                .begin_detail(
                    "a",
                    &account.authorization_epoch,
                    &item.id,
                    DetailFacet::Body,
                )
                .await
                .unwrap();
            let mut page = fixtures::from_lease(&account, DetailFacet::Body, lease);
            page.subject_id = item.id.clone();
            runtime.store.apply_detail(page).await.unwrap();
        }
        runtime
            .store
            .request_detail(
                "a",
                &account.authorization_epoch,
                &item.id,
                DetailFacet::Body,
            )
            .await
            .unwrap();
    }
    for _ in 0..9 {
        runtime.enqueue_pending_details().await.unwrap();
    }
    let scheduler = runtime.scheduler.lock().await;
    assert_eq!(scheduler.queue.len(), 1);
    assert!(
        matches!(&scheduler.queue[0].kind,JobKind::Detail{subject_id,..} if subject_id=="subject-129")
    );
    drop(scheduler);
    assert!(runtime.run_next().await);
    assert_eq!(provider.requests.lock().unwrap().len(), 1);
    assert_eq!(
        runtime
            .store
            .detail(DetailQuery {
                account_id: "a".into(),
                subject_id: "subject-129".into(),
                facet: DetailFacet::Body,
                cursor: None,
                limit: 10
            })
            .await
            .unwrap()
            .body
            .text
            .as_deref(),
        Some("native cached body")
    );
}

#[tokio::test]
async fn global_index_interest_pages_local_repository_keys_without_copying_an_unbounded_plan() {
    let dir = tempfile::tempdir().unwrap();
    let provider = Provider::new(1);
    let (runtime, clock, _, account) =
        setup(&dir.path().join("cache.sqlite"), provider.clone()).await;
    let original = runtime.store.repository("a", "repo").await.unwrap();
    let mut repositories = vec![];
    for i in 0..40 {
        let mut repository = original.clone();
        repository.id = format!("repo-{i:03}");
        repository.provider_id = (100 + i).to_string();
        repository.full_name = format!("owner/project-{i}");
        repository.name = format!("project-{i}");
        repository.web_url = format!("https://github.com/owner/project-{i}");
        repositories.push(repository);
    }
    let run = runtime
        .store
        .begin_sync("a", &account.authorization_epoch, "repositories")
        .await
        .unwrap();
    runtime
        .store
        .apply_page(PageCommit {
            account_id: "a".into(),
            authorization_epoch: account.authorization_epoch.clone(),
            scope: "repositories".into(),
            run_id: run,
            repositories,
            items: vec![],
            endpoint_aliases: vec![],
            next_cursor: None,
            etag: None,
            last_modified: None,
            not_modified: false,
            complete: true,
            observed_at: runtime.now_string(),
        })
        .await
        .unwrap();
    clock.advance(60);
    let activity = activate(&runtime, "view").await;
    let lease = acquire(
        &runtime,
        "view",
        &activity,
        &account,
        target(DemandTargetKind::PullRequests),
    )
    .await;
    runtime.enqueue_foreground().await.unwrap();
    assert_eq!(runtime.scheduler.lock().await.queue.len(), 16);
    assert!(runtime.run_next().await);
    runtime.enqueue_foreground().await.unwrap();
    let scheduler = runtime.scheduler.lock().await;
    assert!(scheduler.queue.len() <= 32);
    assert!(scheduler.queue.iter().any(|job| {
        job.repository
            .as_ref()
            .is_some_and(|repo| repo.id.as_str() > "repo-016")
    }));
    assert!(
        scheduler.demands.leases[&lease.lease_id]
            .repository_cursor
            .is_some()
    );
}

#[tokio::test]
async fn denial_after_admission_prevents_vault_and_provider_access() {
    let dir = tempfile::tempdir().unwrap();
    let provider = Provider::new(1);
    let (runtime, _, vault, account) =
        setup(&dir.path().join("cache.sqlite"), provider.clone()).await;
    let activity = activate(&runtime, "view").await;
    let lease = acquire(
        &runtime,
        "view",
        &activity,
        &account,
        detail(DetailFacet::Body),
    )
    .await;
    runtime.enqueue_foreground().await.unwrap();
    runtime
        .store
        .set_sync_status(
            "a",
            &account.authorization_epoch,
            &DetailFacet::Body.scope("pull"),
            SyncStatus {
                state: SyncState::Error,
                last_success_at: None,
                next_retry_at: None,
                error: Some(CollaborationError::new(
                    ErrorCode::PermissionDenied,
                    "fixture scope denied",
                )),
            },
        )
        .await
        .unwrap();
    assert_eq!(
        runtime
            .renew_demand("view", renewal(&activity, &account, &lease))
            .await
            .unwrap_err()
            .code,
        ErrorCode::PermissionDenied
    );
    assert!(!runtime.run_next().await);
    assert_eq!(vault.loads.load(Ordering::SeqCst), 0);
    assert_eq!(provider.requests.lock().unwrap().len(), 0);
    assert!(runtime.store.pending_details().await.unwrap().is_empty());
}

#[tokio::test]
async fn diagnostics_observe_saved_recovery_and_monotonic_queue_age_without_io() {
    let dir = tempfile::tempdir().unwrap();
    let database = dir.path().join("cache.sqlite");
    let provider = Provider::new(1);
    let (runtime, clock, vault, account) = setup(&database, provider.clone()).await;
    let future_retry = runtime.future_string(30);
    runtime
        .store
        .set_sync_status(
            &account.id,
            &account.authorization_epoch,
            "repositories",
            SyncStatus {
                state: SyncState::Offline,
                last_success_at: None,
                next_retry_at: Some(future_retry.clone()),
                error: Some(CollaborationError::new(
                    ErrorCode::Network,
                    "fixture raw provider url token must never leave storage",
                )),
            },
        )
        .await
        .unwrap();
    {
        let mut scheduler = runtime.scheduler.lock().await;
        let mut ready = queued(&account, 1, scheduler::Admission::Manual);
        ready.enqueued_at = clock.now();
        let mut deferred = queued(&account, 2, scheduler::Admission::Manual);
        deferred.enqueued_at = clock.now();
        scheduler.queue.push_back(ready);
        scheduler.deferred.push_back(deferred);
        scheduler
            .account_cooldowns
            .insert(account.id.clone(), clock.now() + Duration::from_secs(30));
    }
    clock.advance(12);

    let snapshot = runtime.diagnostics().await.unwrap();
    let observed = snapshot
        .accounts
        .iter()
        .find(|candidate| candidate.account_id == account.id)
        .unwrap();
    assert_eq!(snapshot.ready_jobs, 1);
    assert_eq!(snapshot.deferred_jobs, 1);
    assert_eq!(snapshot.oldest_job_age_seconds, Some(12));
    assert_eq!(observed.cooldown_remaining_seconds, Some(19));
    let recovery = observed.recovery.as_ref().unwrap();
    assert_eq!(recovery.category, SyncRecoveryCategory::Offline);
    assert_eq!(recovery.retry_after_seconds, Some(19));
    assert!(!recovery.explicit_retry_eligible);
    assert_eq!(vault.loads.load(Ordering::SeqCst), 0);
    assert!(provider.requests.lock().unwrap().is_empty());

    runtime
        .store
        .set_sync_status(
            &account.id,
            &account.authorization_epoch,
            "repositories",
            SyncStatus {
                state: SyncState::Offline,
                last_success_at: None,
                next_retry_at: Some("2026-10-02T00:00:00Z".into()),
                error: Some(CollaborationError::new(
                    ErrorCode::Network,
                    "another raw diagnostic that stays native",
                )),
            },
        )
        .await
        .unwrap();
    runtime.scheduler.lock().await.account_cooldowns.clear();
    let eligible = runtime.diagnostics().await.unwrap();
    let recovery = eligible.accounts[0].recovery.as_ref().unwrap();
    assert_eq!(recovery.retry_after_seconds, None);
    assert!(recovery.explicit_retry_eligible);
    assert_eq!(vault.loads.load(Ordering::SeqCst), 0);
    assert!(provider.requests.lock().unwrap().is_empty());

    drop(runtime);
    let store = Arc::new(Store::open(&database).await.unwrap());
    let mut reopened = CollaborationRuntime::new(store, vault.clone(), provider.clone());
    reopened.clock = clock;
    let cold = reopened.diagnostics().await.unwrap();
    assert_eq!(cold.ready_jobs, 0);
    assert_eq!(cold.deferred_jobs, 0);
    assert_eq!(cold.oldest_job_age_seconds, None);
    assert_eq!(cold.latency.sample_count, 0);
    assert_eq!(
        cold.accounts[0].recovery.as_ref().unwrap().category,
        SyncRecoveryCategory::Offline
    );
    assert!(
        cold.accounts[0]
            .recovery
            .as_ref()
            .unwrap()
            .explicit_retry_eligible
    );
    assert_eq!(vault.loads.load(Ordering::SeqCst), 0);
    assert!(provider.requests.lock().unwrap().is_empty());
}

#[tokio::test]
async fn diagnostics_latency_counts_attempted_runtime_work_without_counting_reads() {
    let dir = tempfile::tempdir().unwrap();
    let provider = Provider::new(1);
    let (runtime, _, vault, account) =
        setup(&dir.path().join("cache.sqlite"), provider.clone()).await;
    assert_eq!(runtime.diagnostics().await.unwrap().latency.sample_count, 0);
    runtime
        .refresh(RefreshRequest {
            account_id: account.id,
            repository_id: None,
            kind: Some(RemoteItemKind::Notification),
        })
        .await
        .unwrap();
    assert!(runtime.run_next().await);
    let loads = vault.loads.load(Ordering::SeqCst);
    let requests = provider.requests.lock().unwrap().len();
    let measured = runtime.diagnostics().await.unwrap();
    assert_eq!(measured.latency.sample_count, 1);
    let maximum = measured.latency.maximum_milliseconds.unwrap();
    let upper_bound = measured.latency.p50_upper_bound_milliseconds.unwrap();
    assert!(upper_bound >= maximum);
    assert_eq!(
        measured.latency.p95_upper_bound_milliseconds,
        Some(upper_bound)
    );
    assert_eq!(
        measured.latency.p99_upper_bound_milliseconds,
        Some(upper_bound)
    );
    assert_eq!(runtime.diagnostics().await.unwrap().latency.sample_count, 1);
    assert_eq!(vault.loads.load(Ordering::SeqCst), loads);
    assert_eq!(provider.requests.lock().unwrap().len(), requests);
}

#[tokio::test]
async fn ten_page_activation_yields_a_bounded_gap_then_resumes_without_manual_retry() {
    let dir = tempfile::tempdir().unwrap();
    let provider = Provider::new(12);
    let (runtime, clock, _, account) =
        setup(&dir.path().join("cache.sqlite"), provider.clone()).await;
    let activity = activate(&runtime, "view").await;
    acquire(
        &runtime,
        "view",
        &activity,
        &account,
        detail(DetailFacet::Comments),
    )
    .await;
    for _ in 0..10 {
        assert!(runtime.run_next().await);
    }
    assert_eq!(provider.requests.lock().unwrap().len(), 10);
    assert!(!runtime.run_next().await);
    clock.advance(9);
    assert!(!runtime.run_next().await);
    clock.advance(1);
    assert!(runtime.run_next().await);
    assert!(runtime.run_next().await);
    let snapshot = runtime
        .store
        .detail(fixtures::query("a", DetailFacet::Comments))
        .await
        .unwrap();
    assert_eq!(snapshot.entries.len(), 12);
    assert_eq!(snapshot.evidence.coverage.state, CoverageState::Complete);
    assert!(runtime.store.pending_details().await.unwrap().is_empty());
}

#[tokio::test]
async fn closed_offline_interest_does_not_survive_restart_but_saved_values_and_draft_cas_do() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("cache.sqlite");
    let provider = Provider::new(1);
    let (runtime, clock, vault, account) = setup(&path, provider.clone()).await;
    let draft = runtime
        .save_draft(LocalDraft {
            account_id: "a".into(),
            subject_id: "pull".into(),
            body: "private authored body".into(),
            generation: "0".into(),
        })
        .await
        .unwrap();
    let activity = activate(&runtime, "view").await;
    let lease = acquire(
        &runtime,
        "view",
        &activity,
        &account,
        detail(DetailFacet::Body),
    )
    .await;
    assert!(runtime.run_next().await);
    clock.advance(60);
    assert_eq!(
        runtime
            .renew_demand("view", renewal(&activity, &account, &lease))
            .await
            .unwrap_err()
            .code,
        ErrorCode::StaleView
    );
    let lease = acquire(
        &runtime,
        "view",
        &activity,
        &account,
        detail(DetailFacet::Body),
    )
    .await;
    *provider.error.lock().unwrap() = Some(ProviderError::new(ProviderErrorKind::Offline));
    assert!(runtime.run_next().await);
    runtime
        .dispose_demand_owner("view", &activity.generation)
        .await
        .unwrap();
    clock.advance(120);
    assert!(!runtime.run_next().await);
    assert_eq!(provider.requests.lock().unwrap().len(), 2);
    assert_eq!(
        runtime.store.draft("a", "pull").await.unwrap().unwrap(),
        draft
    );
    assert_eq!(
        runtime
            .release_demand(
                "view",
                ReleaseDemandRequest {
                    lease_id: lease.lease_id
                }
            )
            .await,
        Ok(())
    );
    runtime.store.close().await;
    drop(runtime);
    let store = Arc::new(Store::open(&path).await.unwrap());
    let mut reopened = CollaborationRuntime::new(store, vault, provider.clone());
    reopened.clock = clock;
    assert!(!reopened.run_next().await);
    assert!(reopened.scheduler.lock().await.demands.leases.is_empty());
    assert!(reopened.store.pending_details().await.unwrap().is_empty());
    assert_eq!(
        reopened
            .store
            .detail(fixtures::query("a", DetailFacet::Body))
            .await
            .unwrap()
            .body
            .text
            .as_deref(),
        Some("native cached body")
    );
    assert_eq!(
        reopened.store.draft("a", "pull").await.unwrap().unwrap(),
        draft
    );
    assert_eq!(provider.requests.lock().unwrap().len(), 2);
}

#[tokio::test]
async fn account_wide_promotion_saturation_admits_selected_body_next_and_preserves_backfill_checkpoints()
 {
    let dir = tempfile::tempdir().unwrap();
    let provider = Provider::new(12);
    let (runtime, clock, _, account) =
        setup(&dir.path().join("cache.sqlite"), provider.clone()).await;
    let original = runtime.store.repository("a", "repo").await.unwrap();
    let mut repositories = vec![];
    for i in 0..95 {
        let mut repo = original.clone();
        repo.id = format!("repo-{i:03}");
        repo.provider_id = (100 + i).to_string();
        repo.full_name = format!("owner/repo-{i}");
        repo.name = format!("repo-{i}");
        repo.web_url = format!("https://github.com/owner/repo-{i}");
        repositories.push(repo);
    }
    let run = runtime
        .store
        .begin_sync("a", &account.authorization_epoch, "repositories")
        .await
        .unwrap();
    runtime
        .store
        .apply_page(PageCommit {
            account_id: "a".into(),
            authorization_epoch: account.authorization_epoch.clone(),
            scope: "repositories".into(),
            run_id: run,
            repositories: repositories.clone(),
            items: vec![],
            endpoint_aliases: vec![],
            next_cursor: None,
            etag: None,
            last_modified: None,
            not_modified: false,
            complete: true,
            observed_at: runtime.now_string(),
        })
        .await
        .unwrap();
    repositories.push(original);
    clock.advance(180);
    let mut checkpoints = HashMap::new();
    for repo in &repositories {
        let scope = scope_name(FeedKind::PullRequests, Some(repo));
        let run = runtime
            .store
            .begin_sync("a", &account.authorization_epoch, &scope)
            .await
            .unwrap();
        runtime
            .store
            .apply_page(PageCommit {
                account_id: "a".into(),
                authorization_epoch: account.authorization_epoch.clone(),
                scope: scope.clone(),
                run_id: run.clone(),
                repositories: vec![],
                items: vec![],
                endpoint_aliases: vec![],
                next_cursor: Some("2".into()),
                etag: None,
                last_modified: None,
                not_modified: false,
                complete: false,
                observed_at: runtime.now_string(),
            })
            .await
            .unwrap();
        checkpoints.insert(scope.clone(), run);
        runtime
            .enqueue_work_reason(
                account.clone(),
                Some(repo.clone()),
                JobKind::Feed(FeedKind::PullRequests),
                scope,
                scheduler::Admission::Reconcile,
            )
            .await
            .unwrap();
    }
    assert_eq!(runtime.scheduler.lock().await.queue.len(), 96);
    let activity = activate(&runtime, "view").await;
    acquire(
        &runtime,
        "view",
        &activity,
        &account,
        target(DemandTargetKind::PullRequests),
    )
    .await;
    for _ in 0..6 {
        runtime.enqueue_foreground().await.unwrap();
    }
    assert_eq!(runtime.scheduler.lock().await.foreground_keys.len(), 32);
    acquire(
        &runtime,
        "view",
        &activity,
        &account,
        detail(DetailFacet::Body),
    )
    .await;
    runtime.enqueue_foreground().await.unwrap();
    let scheduler = runtime.scheduler.lock().await;
    assert_eq!(scheduler.queue.len(), 97);
    assert_eq!(scheduler.foreground_keys.len(), 32);
    drop(scheduler);
    assert!(runtime.run_next().await);
    assert_eq!(provider.requests.lock().unwrap()[0], "a:detail:1");
    for (scope, run) in checkpoints {
        let checkpoint = runtime
            .store
            .scope_state("a", &scope)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(checkpoint.run_id, run);
        assert_eq!(checkpoint.next_cursor.as_deref(), Some("2"));
    }
    assert!(runtime.run_next().await);
    assert!(provider.requests.lock().unwrap()[1].ends_with(":2"));
}
