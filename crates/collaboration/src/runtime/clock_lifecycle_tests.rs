//! Independent clock controls at native Runtime boundaries, not live HTTP or OS suspend proof.
use super::detail_tests::fixtures;
use super::*;
use crate::credentials::CredentialError;
use async_trait::async_trait;
use std::{
    future::{Future, poll_fn},
    pin::Pin,
    sync::{
        Condvar, Mutex as StdMutex,
        atomic::{AtomicI64, AtomicU64, AtomicUsize},
    },
    task::Poll,
};

const HANDSHAKE: Duration = Duration::from_secs(10);

struct IndependentClock {
    origin: Instant,
    monotonic_ms: AtomicU64,
    utc_ms: AtomicI64,
    jitter: AtomicU64,
}
impl IndependentClock {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            origin: Instant::now(),
            monotonic_ms: AtomicU64::new(0),
            utc_ms: AtomicI64::new(
                DateTime::parse_from_rfc3339("2026-10-05T00:00:00Z")
                    .unwrap()
                    .timestamp_millis(),
            ),
            jitter: AtomicU64::new(7),
        })
    }
    fn advance_monotonic(&self, seconds: u64) {
        self.monotonic_ms
            .fetch_add(seconds * 1000, Ordering::SeqCst);
    }
    fn move_utc(&self, seconds: i64) {
        self.utc_ms.fetch_add(seconds * 1000, Ordering::SeqCst);
    }
}
impl clock::Clock for IndependentClock {
    fn now(&self) -> Instant {
        self.origin + Duration::from_millis(self.monotonic_ms.load(Ordering::SeqCst))
    }
    fn utc(&self) -> DateTime<Utc> {
        DateTime::from_timestamp_millis(self.utc_ms.load(Ordering::SeqCst)).unwrap()
    }
    fn jitter(&self) -> u64 {
        self.jitter.load(Ordering::SeqCst)
    }
}

#[derive(Default)]
struct AsyncGate {
    entered: Notify,
    released: Notify,
}
impl AsyncGate {
    async fn hold(&self) {
        self.entered.notify_one();
        tokio::time::timeout(HANDSHAKE, self.released.notified())
            .await
            .expect("bounded synthetic adapter release");
    }
    fn release(&self) {
        self.released.notify_one();
    }
}
struct AsyncRelease(Arc<AsyncGate>);
impl Drop for AsyncRelease {
    fn drop(&mut self) {
        self.0.release();
    }
}

#[derive(Default)]
struct BlockingGate {
    entered: Notify,
    released: StdMutex<bool>,
    changed: Condvar,
}
impl BlockingGate {
    fn hold(&self) -> Result<(), CredentialError> {
        self.entered.notify_one();
        let (released, _) = self
            .changed
            .wait_timeout_while(self.released.lock().unwrap(), HANDSHAKE, |released| {
                !*released
            })
            .unwrap();
        if *released {
            Ok(())
        } else {
            Err(CredentialError::Unavailable)
        }
    }
    fn release(&self) {
        *self.released.lock().unwrap() = true;
        self.changed.notify_all();
    }
}
struct BlockingRelease(Arc<BlockingGate>);
impl Drop for BlockingRelease {
    fn drop(&mut self) {
        self.0.release();
    }
}

#[derive(Default)]
struct Vault {
    tokens: StdMutex<HashMap<String, SecretToken>>,
    loads: AtomicUsize,
    load_gate: StdMutex<Option<Arc<BlockingGate>>>,
    store_gate: StdMutex<Option<Arc<BlockingGate>>>,
}
impl CredentialVault for Vault {
    fn load(&self, reference: &str) -> Result<Option<SecretToken>, CredentialError> {
        self.loads.fetch_add(1, Ordering::SeqCst);
        let gate = self.load_gate.lock().unwrap().take();
        if let Some(gate) = gate {
            gate.hold()?;
        }
        Ok(self.tokens.lock().unwrap().get(reference).cloned())
    }
    fn store(&self, reference: &str, token: &SecretToken) -> Result<(), CredentialError> {
        let gate = self.store_gate.lock().unwrap().take();
        if let Some(gate) = gate {
            gate.hold()?;
        }
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

struct ProbePlan {
    actor: String,
    quota: Option<u64>,
    reject: bool,
}
#[derive(Default)]
struct Adapter {
    probes: AtomicUsize,
    probe_plans: StdMutex<VecDeque<ProbePlan>>,
    probe_gate: StdMutex<Option<Arc<AsyncGate>>>,
    reads: StdMutex<Vec<(String, &'static str)>>,
    read_gate: StdMutex<Option<Arc<AsyncGate>>>,
    read_errors: StdMutex<HashMap<String, ProviderError>>,
    read_quota: AtomicU64,
}
impl Adapter {
    fn plan(&self, actor: &str, quota: Option<u64>, reject: bool) {
        self.probe_plans.lock().unwrap().push_back(ProbePlan {
            actor: actor.into(),
            quota,
            reject,
        });
    }
    fn read_count(&self) -> usize {
        self.reads.lock().unwrap().len()
    }
    async fn read(
        &self,
        account: &RemoteAccount,
        kind: &'static str,
    ) -> Result<Option<u64>, ProviderError> {
        self.reads.lock().unwrap().push((account.id.clone(), kind));
        let error = self.read_errors.lock().unwrap().get(&account.id).cloned();
        let quota = self.read_quota.load(Ordering::SeqCst);
        let gate = self.read_gate.lock().unwrap().take();
        if let Some(gate) = gate {
            gate.hold().await;
        }
        if let Some(error) = error {
            return Err(error);
        }
        Ok((quota > 0).then_some(quota))
    }
}
#[async_trait]
impl CollaborationProvider for Adapter {
    fn kind(&self) -> ProviderKind {
        ProviderKind::Github
    }
    fn profile(&self, _: &RemoteAccount) -> ProviderProfile {
        let mut profile = ProviderProfile::read_only(InboxSemantics::None, false);
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
    async fn probe(&self, token: &SecretToken) -> Result<VerifiedAccount, ProviderError> {
        self.probe_with_backoff(token)
            .await
            .map_err(|failure| failure.error)
    }
    async fn probe_with_backoff(&self, _: &SecretToken) -> Result<VerifiedAccount, ProbeFailure> {
        self.probes.fetch_add(1, Ordering::SeqCst);
        let plan = self
            .probe_plans
            .lock()
            .unwrap()
            .pop_front()
            .expect("planned synthetic probe");
        let gate = self.probe_gate.lock().unwrap().take();
        if let Some(gate) = gate {
            gate.hold().await;
        }
        if plan.reject {
            let mut error = ProviderError::new(ProviderErrorKind::InvalidResponse);
            error.account_cooldown_seconds = plan.quota;
            return Err(ProbeFailure {
                error,
                verified_actor_id: Some(plan.actor),
            });
        }
        Ok(VerifiedAccount {
            actor_id: plan.actor.clone(),
            login: plan.actor,
            display_name: None,
            notifications_supported: false,
            cooldown_seconds: plan.quota,
        })
    }
    async fn fetch_page(
        &self,
        _: &SecretToken,
        request: FeedRequest,
    ) -> Result<FetchPage, ProviderError> {
        let cooldown = self.read(&request.account, "feed").await?;
        Ok(FetchPage {
            repositories: vec![],
            items: vec![],
            endpoint_aliases: vec![],
            notification_subjects: vec![],
            next_cursor: None,
            etag: None,
            last_modified: None,
            not_modified: false,
            poll_interval_seconds: None,
            cooldown_seconds: cooldown,
        })
    }
    async fn fetch_detail(
        &self,
        _: &SecretToken,
        request: DetailRequest,
    ) -> Result<DetailPage, ProviderError> {
        assert_eq!(request.facet, DetailFacet::Body);
        let cooldown = self.read(&request.account, "body").await?;
        Ok(DetailPage {
            reconciliation: DetailReconciliation::full_history(),
            body: fixtures::known(Some("synthetic native body")),
            metadata: None,
            entries: vec![],
            source: fixtures::source(request.facet),
            next_cursor: None,
            etag: Some("native-clock-body-v1".into()),
            not_modified: false,
            freshness_seconds: 60,
            cooldown_seconds: cooldown,
        })
    }
}

struct Fixture {
    runtime: CollaborationRuntime,
    clock: Arc<IndependentClock>,
    vault: Arc<Vault>,
    adapter: Arc<Adapter>,
    account: RemoteAccount,
}
async fn add_actor(store: &Store, vault: &Vault, id: &str) -> RemoteAccount {
    let account = store.upsert_account(fixtures::account(id)).await.unwrap();
    let reference = format!("synthetic-clock-{id}");
    store.stage_credential(id, &reference).await.unwrap();
    vault
        .store(
            &reference,
            &SecretToken::new("synthetic_clock_token".into()).unwrap(),
        )
        .unwrap();
    let account = store
        .commit_account_credential(
            RemoteAccount {
                authorization_epoch: "2".into(),
                ..account
            },
            &reference,
        )
        .await
        .unwrap();
    fixtures::project(store, &account).await;
    account
}
async fn setup(path: &std::path::Path) -> Fixture {
    let store = Arc::new(Store::open(path).await.unwrap());
    let vault = Arc::new(Vault::default());
    let adapter = Arc::new(Adapter::default());
    let clock = IndependentClock::new();
    let account = add_actor(&store, &vault, "a").await;
    let mut runtime = CollaborationRuntime::new(store, vault.clone(), adapter.clone());
    runtime.clock = clock.clone();
    Fixture {
        runtime,
        clock,
        vault,
        adapter,
        account,
    }
}
async fn admit(runtime: &CollaborationRuntime, account: &RemoteAccount, detail: bool) -> String {
    if detail {
        runtime
            .hydrate_detail(HydrateDetailRequest {
                account_id: account.id.clone(),
                authorization_epoch: account.authorization_epoch.clone(),
                subject_id: "pull".into(),
                facet: DetailFacet::Body,
            })
            .await
            .unwrap()
            .job_id
    } else {
        runtime
            .refresh(RefreshRequest {
                account_id: account.id.clone(),
                repository_id: Some("repo".into()),
                kind: Some(RemoteItemKind::Issue),
            })
            .await
            .unwrap()
            .job_id
    }
}
async fn deadline(runtime: &CollaborationRuntime, account: &RemoteAccount) -> String {
    let scope = runtime
        .store
        .scope_state(&account.id, "provider:rest")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(scope.sync.state, SyncState::RateLimited);
    scope.sync.next_retry_at.unwrap()
}
// This is a synthetic adapter receipt through the shared captured-epoch Runtime
// persistence boundary. It does not claim a concurrent public probe under lifecycle.
async fn boundary_receipt(
    runtime: &CollaborationRuntime,
    adapter: &Adapter,
    account: &RemoteAccount,
    seconds: u64,
) -> Result<(), CollaborationError> {
    adapter.plan(&account.actor_id, Some(seconds), true);
    let failure = adapter
        .probe_with_backoff(&SecretToken::new("synthetic_receipt".into()).unwrap())
        .await
        .unwrap_err();
    assert_eq!(
        failure.verified_actor_id.as_deref(),
        Some(account.actor_id.as_str())
    );
    runtime
        .persist_rate_limit(
            account,
            failure.error.account_cooldown_seconds.unwrap(),
            None,
        )
        .await
}
async fn wait_picked<F: Future<Output = bool>>(
    runtime: &CollaborationRuntime,
    id: &str,
    mut worker: Pin<&mut F>,
) {
    tokio::time::timeout(HANDSHAKE, async {
        loop {
            poll_fn(|cx| {
                assert!(
                    worker.as_mut().poll(cx).is_pending(),
                    "held worker must remain Pending"
                );
                Poll::Ready(())
            })
            .await;
            let scheduler = runtime.scheduler.lock().await;
            let key = scheduler
                .active
                .iter()
                .find(|(_, receipt)| receipt.as_str() == id)
                .map(|(key, _)| key.clone())
                .expect("real admitted receipt remains active");
            let picked = scheduler
                .queue
                .iter()
                .chain(&scheduler.deferred)
                .all(|job| job.key != key);
            drop(scheduler);
            if picked {
                assert!(
                    runtime.dispatch.try_lock().is_err(),
                    "actual single worker owns dispatch"
                );
                return;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("bounded actual scheduler pick");
}
async fn saved_private(
    runtime: &CollaborationRuntime,
    account: &RemoteAccount,
) -> (DetailSnapshot, LocalDraft) {
    let commit = fixtures::commit(&runtime.store, account, DetailFacet::Body).await;
    runtime.store.apply_detail(commit).await.unwrap();
    let saved = runtime
        .store
        .detail(fixtures::query(&account.id, DetailFacet::Body))
        .await
        .unwrap();
    let draft = runtime
        .save_draft(LocalDraft {
            account_id: account.id.clone(),
            subject_id: "pull".into(),
            body: "private clock-control draft".into(),
            generation: "0".into(),
        })
        .await
        .unwrap();
    (saved, draft)
}
async fn assert_private(
    runtime: &CollaborationRuntime,
    account: &RemoteAccount,
    saved: &DetailSnapshot,
    draft: &LocalDraft,
) {
    let current = runtime
        .store
        .detail(fixtures::query(&account.id, DetailFacet::Body))
        .await
        .unwrap();
    assert_eq!(current.body, saved.body);
    assert_eq!(
        current.evidence.facet_revision,
        saved.evidence.facet_revision
    );
    assert_eq!(
        runtime.store.draft(&account.id, "pull").await.unwrap(),
        Some(draft.clone())
    );
}

#[tokio::test]
async fn forward_utc_and_shorter_receipts_preserve_live_lower_bound_and_original_deadline() {
    let dir = tempfile::tempdir().unwrap();
    let f = setup(&dir.path().join("cache.sqlite")).await;
    boundary_receipt(&f.runtime, &f.adapter, &f.account, 120)
        .await
        .unwrap();
    let original = deadline(&f.runtime, &f.account).await;
    f.clock.advance_monotonic(10);
    f.clock.move_utc(-30);
    boundary_receipt(&f.runtime, &f.adapter, &f.account, 10)
        .await
        .unwrap();
    assert_eq!(deadline(&f.runtime, &f.account).await, original);
    f.clock.move_utc(630);
    admit(&f.runtime, &f.account, false).await;
    admit(&f.runtime, &f.account, true).await;
    assert!(
        !f.runtime.run_next().await,
        "forward UTC cannot remove accepted short live quota"
    );
    assert_eq!(f.vault.loads.load(Ordering::SeqCst), 0);
    assert_eq!(f.adapter.read_count(), 0);
    assert_eq!(deadline(&f.runtime, &f.account).await, original);
    f.clock.advance_monotonic(109);
    assert!(!f.runtime.run_next().await);
    f.clock.advance_monotonic(1);
    assert!(f.runtime.run_next().await);
    assert!(f.runtime.run_next().await);
    assert_eq!(f.adapter.read_count(), 2);
    assert_eq!(f.vault.loads.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn picked_feed_and_body_waiting_on_rejected_probe_recheck_live_quota_before_vault() {
    for detail in [false, true] {
        let dir = tempfile::tempdir().unwrap();
        let f = setup(&dir.path().join("cache.sqlite")).await;
        let (saved, draft) = saved_private(&f.runtime, &f.account).await;
        let id = admit(&f.runtime, &f.account, detail).await;
        let gate = Arc::new(AsyncGate::default());
        let _release = AsyncRelease(gate.clone());
        *f.adapter.probe_gate.lock().unwrap() = Some(gate.clone());
        f.adapter.plan(&f.account.actor_id, Some(120), true);
        let connector_runtime = f.runtime.clone();
        let connector = tokio::spawn(async move {
            connector_runtime
                .connect_github("synthetic_rejected_probe".into())
                .await
        });
        tokio::time::timeout(HANDSHAKE, gate.entered.notified())
            .await
            .unwrap();
        let mut worker = Box::pin(f.runtime.run_next());
        wait_picked(&f.runtime, &id, worker.as_mut()).await;
        assert_eq!(f.vault.loads.load(Ordering::SeqCst), 0);
        assert_eq!(f.adapter.read_count(), 0);
        gate.release();
        assert_eq!(
            tokio::time::timeout(HANDSHAKE, connector)
                .await
                .unwrap()
                .unwrap()
                .unwrap_err()
                .code,
            ErrorCode::Provider
        );
        assert_eq!(
            f.runtime.store.account(&f.account.id).await.unwrap(),
            f.account
        );
        let original = deadline(&f.runtime, &f.account).await;
        f.clock.move_utc(600);
        assert!(tokio::time::timeout(HANDSHAKE, worker).await.unwrap());
        assert_eq!(
            f.vault.loads.load(Ordering::SeqCst),
            0,
            "picked {detail:?} read must refuse before vault"
        );
        assert_eq!(f.adapter.read_count(), 0);
        assert_eq!(
            deadline(&f.runtime, &f.account).await,
            original,
            "local refusal cannot invent quota at shifted UTC"
        );
        assert_private(&f.runtime, &f.account, &saved, &draft).await;
        assert!(!f.runtime.run_next().await);
    }
}

#[tokio::test]
async fn picked_feed_and_body_after_native_vault_gate_recheck_quota_before_adapter_read() {
    for detail in [false, true] {
        let dir = tempfile::tempdir().unwrap();
        let f = setup(&dir.path().join("cache.sqlite")).await;
        let (saved, draft) = saved_private(&f.runtime, &f.account).await;
        let id = admit(&f.runtime, &f.account, detail).await;
        let gate = Arc::new(BlockingGate::default());
        let _release = BlockingRelease(gate.clone());
        *f.vault.load_gate.lock().unwrap() = Some(gate.clone());
        let mut worker = Box::pin(f.runtime.run_next());
        tokio::time::timeout(HANDSHAKE, async {
            tokio::select! {
                _ = gate.entered.notified() => {},
                _ = &mut worker => panic!("real worker completed before held vault"),
            }
        })
        .await
        .unwrap();
        wait_picked(&f.runtime, &id, worker.as_mut()).await;
        assert_eq!(f.vault.loads.load(Ordering::SeqCst), 1);
        boundary_receipt(&f.runtime, &f.adapter, &f.account, 120)
            .await
            .unwrap();
        let original = deadline(&f.runtime, &f.account).await;
        f.clock.move_utc(600);
        gate.release();
        assert!(tokio::time::timeout(HANDSHAKE, worker).await.unwrap());
        assert_eq!(
            f.vault.loads.load(Ordering::SeqCst),
            1,
            "one previously admitted native load may complete"
        );
        assert_eq!(
            f.adapter.read_count(),
            0,
            "new quota must stop the {detail:?} read after native work"
        );
        assert_eq!(deadline(&f.runtime, &f.account).await, original);
        assert_private(&f.runtime, &f.account, &saved, &draft).await;
        admit(&f.runtime, &f.account, !detail).await;
        assert!(!f.runtime.run_next().await);
        assert_eq!(
            f.vault.loads.load(Ordering::SeqCst),
            1,
            "sibling cannot add an early load"
        );
        assert_eq!(f.adapter.read_count(), 0);
    }
}

#[tokio::test]
async fn positive_successful_probe_captures_live_quota_before_native_cutover_awaits() {
    for replacement in [false, true] {
        let dir = tempfile::tempdir().unwrap();
        let store = Arc::new(Store::open(dir.path().join("cache.sqlite")).await.unwrap());
        let vault = Arc::new(Vault::default());
        let adapter = Arc::new(Adapter::default());
        let clock = IndependentClock::new();
        let mut runtime = CollaborationRuntime::new(store, vault.clone(), adapter.clone());
        runtime.clock = clock.clone();
        let previous = if replacement {
            adapter.plan("a", None, false);
            Some(
                runtime
                    .connect_github("synthetic_first_grant".into())
                    .await
                    .unwrap(),
            )
        } else {
            None
        };
        let gate = Arc::new(BlockingGate::default());
        let _release = BlockingRelease(gate.clone());
        *vault.store_gate.lock().unwrap() = Some(gate.clone());
        adapter.plan("a", Some(120), false);
        let proposed = runtime.future_string(120);
        let receipt_live_deadline = runtime.now() + Duration::from_secs(120);
        let mut connector = Box::pin(runtime.connect_github("synthetic_quota_grant".into()));
        tokio::time::timeout(HANDSHAKE, async {
            tokio::select! {
                _ = gate.entered.notified() => {},
                _ = &mut connector => panic!("connect completed before native write gate"),
            }
        })
        .await
        .unwrap();
        poll_fn(|cx| {
            assert!(connector.as_mut().poll(cx).is_pending());
            Poll::Ready(())
        })
        .await;
        clock.advance_monotonic(37);
        clock.move_utc(600);
        gate.release();
        let account = tokio::time::timeout(HANDSHAKE, connector)
            .await
            .unwrap()
            .unwrap();
        if let Some(previous) = previous {
            assert_eq!(account.id, previous.id);
            assert_ne!(account.authorization_epoch, previous.authorization_epoch);
        }
        assert_eq!(deadline(&runtime, &account).await, proposed);
        assert!(
            !runtime.run_next().await,
            "accepted successful probe quota must retain its captured live bound"
        );
        assert_eq!(adapter.read_count(), 0);
        assert_eq!(vault.loads.load(Ordering::SeqCst), 0);
        let scheduler = runtime.scheduler.lock().await;
        assert_eq!(scheduler.account_cooldowns.len(), 1);
        assert!(scheduler.account_cooldowns.contains_key(&account.id));
        assert_eq!(
            scheduler.account_cooldowns[&account.id],
            receipt_live_deadline.into(),
            "the 37s native write wait must consume the original 120s bound, not start it again at cutover"
        );
    }
}

#[tokio::test]
async fn backward_utc_keeps_full_durable_quota_and_serves_an_eligible_peer_without_spinning() {
    let dir = tempfile::tempdir().unwrap();
    let f = setup(&dir.path().join("cache.sqlite")).await;
    let peer = add_actor(&f.runtime.store, &f.vault, "b").await;
    boundary_receipt(&f.runtime, &f.adapter, &f.account, 120)
        .await
        .unwrap();
    let original = deadline(&f.runtime, &f.account).await;
    f.clock.move_utc(-3600);
    f.clock.advance_monotonic(120);
    admit(&f.runtime, &f.account, false).await;
    admit(&f.runtime, &peer, false).await;
    assert!(f.runtime.run_next().await);
    assert_eq!(
        *f.adapter.reads.lock().unwrap(),
        vec![(peer.id.clone(), "feed")]
    );
    assert_eq!(f.vault.loads.load(Ordering::SeqCst), 1);
    for _ in 0..4 {
        assert!(!f.runtime.run_next().await);
    }
    assert_eq!(deadline(&f.runtime, &f.account).await, original);
    f.clock.move_utc(3719);
    f.clock.advance_monotonic(3719);
    assert!(!f.runtime.run_next().await);
    f.clock.move_utc(1);
    f.clock.advance_monotonic(1);
    assert!(f.runtime.run_next().await);
    assert_eq!(f.adapter.read_count(), 2);
}

#[tokio::test]
async fn authentication_and_permanent_detail_errors_do_not_spin_after_clock_changes() {
    for kind in [
        ProviderErrorKind::Authentication,
        ProviderErrorKind::Permission,
        ProviderErrorKind::Unsupported,
    ] {
        let dir = tempfile::tempdir().unwrap();
        let f = setup(&dir.path().join("cache.sqlite")).await;
        f.adapter
            .read_errors
            .lock()
            .unwrap()
            .insert(f.account.id.clone(), ProviderError::new(kind));
        admit(&f.runtime, &f.account, true).await;
        assert!(f.runtime.run_next().await);
        f.clock.move_utc(-3600);
        f.clock.advance_monotonic(3600);
        for _ in 0..4 {
            f.runtime.enqueue_pending_details().await.unwrap();
            assert!(!f.runtime.run_next().await);
        }
        assert_eq!(f.adapter.read_count(), 1);
        assert_eq!(f.vault.loads.load(Ordering::SeqCst), 1);
        assert!(f.runtime.store.pending_details().await.unwrap().is_empty());
    }
}

#[tokio::test]
async fn reconnect_and_obsolete_receipts_preserve_actor_budget_with_one_map_slot() {
    let dir = tempfile::tempdir().unwrap();
    let f = setup(&dir.path().join("cache.sqlite")).await;
    let peer = add_actor(&f.runtime.store, &f.vault, "b").await;
    let (_, draft) = saved_private(&f.runtime, &f.account).await;
    boundary_receipt(&f.runtime, &f.adapter, &f.account, 120)
        .await
        .unwrap();
    let original = deadline(&f.runtime, &f.account).await;
    f.clock.move_utc(600);
    let mut replacement = f.account.clone();
    for _ in 0..12 {
        f.adapter.plan(&f.account.actor_id, None, false);
        replacement = f
            .runtime
            .connect_github("synthetic_same_actor".into())
            .await
            .unwrap();
        assert_eq!(replacement.id, f.account.id);
        assert_eq!(deadline(&f.runtime, &replacement).await, original);
        let scheduler = f.runtime.scheduler.lock().await;
        assert_eq!(scheduler.account_cooldowns.len(), 1);
        assert!(scheduler.account_cooldowns.contains_key(&replacement.id));
    }
    let live = f.runtime.scheduler.lock().await.account_cooldowns[&replacement.id];
    assert_eq!(
        boundary_receipt(&f.runtime, &f.adapter, &f.account, 3600)
            .await
            .unwrap_err()
            .code,
        ErrorCode::StaleView
    );
    assert_eq!(
        f.runtime.scheduler.lock().await.account_cooldowns[&replacement.id],
        live
    );
    assert_eq!(deadline(&f.runtime, &replacement).await, original);
    assert!(
        f.runtime
            .store
            .scope_state(&peer.id, "provider:rest")
            .await
            .unwrap()
            .is_none()
    );
    assert_eq!(f.runtime.store.account(&peer.id).await.unwrap(), peer);
    assert_eq!(
        f.runtime
            .store
            .draft(&replacement.id, "pull")
            .await
            .unwrap(),
        Some(draft)
    );
    assert!(!f.runtime.run_next().await);
    assert_eq!(f.adapter.read_count(), 0);
    assert_eq!(f.vault.loads.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn obsolete_held_adapter_reply_installs_no_replacement_or_peer_budget() {
    let dir = tempfile::tempdir().unwrap();
    let f = setup(&dir.path().join("cache.sqlite")).await;
    let peer = add_actor(&f.runtime.store, &f.vault, "b").await;
    let (_, draft) = saved_private(&f.runtime, &f.account).await;
    f.adapter.read_quota.store(120, Ordering::SeqCst);
    let gate = Arc::new(AsyncGate::default());
    let _release = AsyncRelease(gate.clone());
    *f.adapter.read_gate.lock().unwrap() = Some(gate.clone());
    admit(&f.runtime, &f.account, true).await;
    let mut worker = Box::pin(f.runtime.run_next());
    tokio::time::timeout(HANDSHAKE, async {
        tokio::select! {
            _ = gate.entered.notified() => {},
            _ = &mut worker => panic!("worker completed before held receipt"),
        }
    })
    .await
    .unwrap();
    f.adapter.plan(&f.account.actor_id, None, false);
    let replacement = f
        .runtime
        .connect_github("synthetic_replacement".into())
        .await
        .unwrap();
    gate.release();
    assert!(tokio::time::timeout(HANDSHAKE, worker).await.unwrap());
    assert!(
        f.runtime
            .store
            .scope_state(&replacement.id, "provider:rest")
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        f.runtime
            .store
            .scope_state(&peer.id, "provider:rest")
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        f.runtime
            .scheduler
            .lock()
            .await
            .account_cooldowns
            .is_empty()
    );
    assert_eq!(
        f.runtime.store.account(&replacement.id).await.unwrap(),
        replacement
    );
    assert_eq!(f.runtime.store.account(&peer.id).await.unwrap(), peer);
    assert_eq!(
        f.runtime
            .store
            .draft(&replacement.id, "pull")
            .await
            .unwrap(),
        Some(draft)
    );
    let body = f
        .runtime
        .store
        .detail(fixtures::query(&replacement.id, DetailFacet::Body))
        .await;
    assert!(body.is_err() || body.unwrap().body.text.as_deref() != Some("synthetic native body"));
}

#[tokio::test]
async fn failed_sqlite_budget_acceptance_installs_no_live_barrier() {
    use sqlx::{Connection, sqlite::SqliteConnectOptions};
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("cache.sqlite");
    let f = setup(&path).await;
    let mut fault =
        sqlx::SqliteConnection::connect_with(&SqliteConnectOptions::new().filename(&path))
            .await
            .unwrap();
    sqlx::query("CREATE TRIGGER reject_clock_budget BEFORE INSERT ON sync_scopes WHEN NEW.scope='provider:rest' BEGIN SELECT RAISE(ABORT,'synthetic budget rejection'); END")
        .execute(&mut fault).await.unwrap();
    assert_eq!(
        boundary_receipt(&f.runtime, &f.adapter, &f.account, 120)
            .await
            .unwrap_err()
            .code,
        ErrorCode::Storage
    );
    assert!(
        f.runtime
            .store
            .scope_state(&f.account.id, "provider:rest")
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        f.runtime
            .scheduler
            .lock()
            .await
            .account_cooldowns
            .is_empty()
    );
    sqlx::query("DROP TRIGGER reject_clock_budget")
        .execute(&mut fault)
        .await
        .unwrap();
    fault.close().await.unwrap();
    f.clock.move_utc(600);
    admit(&f.runtime, &f.account, false).await;
    assert!(f.runtime.run_next().await);
    assert_eq!(f.adapter.read_count(), 1);
    assert_eq!(f.vault.loads.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn cold_valid_utc_recovers_full_deadline_private_cache_and_draft_cas() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("cache.sqlite");
    let f = setup(&path).await;
    let (saved, draft) = saved_private(&f.runtime, &f.account).await;
    let peer = add_actor(&f.runtime.store, &f.vault, "b").await;
    boundary_receipt(&f.runtime, &f.adapter, &f.account, 120)
        .await
        .unwrap();
    let original = deadline(&f.runtime, &f.account).await;
    f.clock.move_utc(30);
    f.clock.advance_monotonic(30);
    let Fixture {
        runtime,
        clock,
        vault,
        adapter,
        account,
    } = f;
    runtime.store.close().await.unwrap();
    drop(runtime);
    let store = Arc::new(Store::open(&path).await.unwrap());
    let mut cold = CollaborationRuntime::new(store, vault.clone(), adapter.clone());
    cold.clock = clock.clone();
    assert_eq!(deadline(&cold, &account).await, original);
    assert_private(&cold, &account, &saved, &draft).await;
    admit(&cold, &account, true).await;
    assert!(!cold.run_next().await);
    assert_eq!(adapter.read_count(), 0);
    assert_eq!(vault.loads.load(Ordering::SeqCst), 0);
    admit(&cold, &peer, false).await;
    assert!(cold.run_next().await);
    assert_eq!(
        *adapter.reads.lock().unwrap(),
        vec![(peer.id.clone(), "feed")]
    );
    assert_private(&cold, &account, &saved, &draft).await;
    let changed = cold
        .save_draft(LocalDraft {
            body: "private after cold recovery".into(),
            ..draft.clone()
        })
        .await
        .unwrap();
    assert_ne!(changed.generation, draft.generation);
    assert_eq!(
        cold.save_draft(draft).await.unwrap_err().code,
        ErrorCode::StaleView
    );
    clock.move_utc(89);
    clock.advance_monotonic(89);
    assert!(!cold.run_next().await);
    assert_eq!(adapter.read_count(), 1);
    assert_eq!(vault.loads.load(Ordering::SeqCst), 1);
    clock.move_utc(1);
    clock.advance_monotonic(1);
    assert!(cold.run_next().await);
    assert_eq!(adapter.read_count(), 2);
    assert_eq!(vault.loads.load(Ordering::SeqCst), 2);
    assert_eq!(
        cold.store.draft(&account.id, "pull").await.unwrap(),
        Some(changed)
    );
}

async fn wait_stopping(runtime: &CollaborationRuntime) {
    tokio::time::timeout(HANDSHAKE, async {
        while !runtime.is_stopping() {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
}

#[tokio::test]
async fn shutdown_joins_active_background_dispatch_and_a_native_operation_lease() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("cache.sqlite");
    let f = setup(&path).await;
    let gate = Arc::new(AsyncGate::default());
    let _release = AsyncRelease(gate.clone());
    *f.adapter.read_gate.lock().unwrap() = Some(gate.clone());
    admit(&f.runtime, &f.account, false).await;
    let runtime = Arc::new(f.runtime);
    let ipc = runtime.acquire_operation().unwrap();
    runtime.clone().start_background();
    tokio::time::timeout(HANDSHAKE, gate.entered.notified())
        .await
        .unwrap();
    let closing_runtime = runtime.clone();
    let closing = tokio::spawn(async move { closing_runtime.shutdown().await });
    wait_stopping(&runtime).await;
    assert!(runtime.acquire_operation().is_err());
    assert_eq!(
        Store::open(&path).await.err().unwrap().code,
        ErrorCode::Busy
    );
    assert!(!closing.is_finished());
    gate.release();
    // The background's completion also takes lifecycle after dispatch. Holding
    // the IPC lease until after it completes catches shutdown lock inversion.
    let dispatch = tokio::time::timeout(HANDSHAKE, runtime.dispatch.lock())
        .await
        .unwrap();
    drop(dispatch);
    assert!(!closing.is_finished());
    drop(ipc);
    tokio::time::timeout(HANDSHAKE, closing)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    let reads = f.adapter.read_count();
    assert!(!runtime.run_next().await);
    assert_eq!(f.adapter.read_count(), reads);
    assert!(runtime.store.revision().await.is_err());
    let reopened = Store::open(&path).await.unwrap();
    reopened.close().await.unwrap();
}

#[tokio::test]
async fn cancelled_shutdown_still_drains_and_replacement_preserves_native_configuration() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("cache.sqlite");
    let f = setup(&path).await;
    let old_owner = f
        .runtime
        .demand_owner_activity("retained-content")
        .await
        .unwrap();
    let ipc = f.runtime.acquire_operation().unwrap();
    let closing_runtime = f.runtime.clone();
    let requester = tokio::spawn(async move { closing_runtime.shutdown().await });
    wait_stopping(&f.runtime).await;
    requester.abort();
    let _ = requester.await;
    assert_eq!(
        Store::open(&path).await.err().unwrap().code,
        ErrorCode::Busy
    );
    drop(ipc);
    // A second waiter shares completion; cancelling the first never cancels it.
    tokio::time::timeout(HANDSHAKE, f.runtime.shutdown())
        .await
        .unwrap()
        .unwrap();
    let reopened = Arc::new(Store::open(&path).await.unwrap());
    let replacement = f.runtime.replacement(reopened).unwrap();
    assert!(replacement.acquire_operation().is_ok());
    let new_owner = replacement
        .demand_owner_activity("retained-content")
        .await
        .unwrap();
    assert_ne!(new_owner.generation, old_owner.generation);
    assert_eq!(
        replacement
            .set_demand_owner_activity("retained-content", &old_owner.generation, true)
            .await
            .unwrap_err()
            .code,
        ErrorCode::StaleView
    );
    assert!(f.runtime.acquire_operation().is_err());
    assert!(Arc::ptr_eq(&replacement.vault, &f.runtime.vault));
    assert!(Arc::ptr_eq(&replacement.registry, &f.runtime.registry));
    assert!(replacement.scheduler.lock().await.queue.is_empty());
    admit(&replacement, &f.account, false).await;
    assert!(replacement.run_next().await);
    assert_eq!(f.adapter.read_count(), 1);
    replacement.shutdown().await.unwrap();
}

#[tokio::test]
async fn shutdown_waits_for_cancelled_requesters_owned_vault_cutover() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("cache.sqlite");
    let f = setup(&path).await;
    f.adapter.plan(&f.account.actor_id, None, false);
    let gate = Arc::new(BlockingGate::default());
    let _release = BlockingRelease(gate.clone());
    *f.vault.store_gate.lock().unwrap() = Some(gate.clone());
    let connecting_runtime = f.runtime.clone();
    let connecting = tokio::spawn(async move {
        connecting_runtime
            .connect_github("replacement_fixture_token".into())
            .await
    });
    tokio::time::timeout(HANDSHAKE, gate.entered.notified())
        .await
        .unwrap();
    connecting.abort();
    let _ = connecting.await;
    let closing_runtime = f.runtime.clone();
    let closing = tokio::spawn(async move { closing_runtime.shutdown().await });
    wait_stopping(&f.runtime).await;
    assert_eq!(
        f.runtime
            .connect_github("rejected_fixture_token".into())
            .await
            .unwrap_err()
            .code,
        ErrorCode::NotReady
    );
    assert_eq!(f.adapter.probes.load(Ordering::SeqCst), 1);
    assert_eq!(
        Store::open(&path).await.err().unwrap().code,
        ErrorCode::Busy
    );
    assert!(!closing.is_finished());
    gate.release();
    tokio::time::timeout(HANDSHAKE, closing)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    let reopened = Store::open(&path).await.unwrap();
    let account = reopened.account(&f.account.id).await.unwrap();
    assert_ne!(account.authorization_epoch, f.account.authorization_epoch);
    let reference = reopened
        .credential_reference(&account.id)
        .await
        .unwrap()
        .unwrap();
    assert!(f.vault.load(&reference).unwrap().is_some());
    assert_eq!(f.vault.tokens.lock().unwrap().len(), 1);
    assert!(
        reopened
            .due_credential_cleanup(i64::MAX, 32)
            .await
            .unwrap()
            .is_empty()
    );
    reopened.close().await.unwrap();
}

#[tokio::test]
async fn long_clock_live_observation_survives_forward_jump_and_shorter_receipt() {
    let dir = tempfile::tempdir().unwrap();
    let f = setup(&dir.path().join("cache.sqlite")).await;
    boundary_receipt(&f.runtime, &f.adapter, &f.account, 48 * 3600)
        .await
        .unwrap();
    let original = deadline(&f.runtime, &f.account).await;
    let peer = add_actor(&f.runtime.store, &f.vault, "b").await;
    f.clock.advance_monotonic(25 * 3600);
    f.clock.move_utc(72 * 3600);
    admit(&f.runtime, &f.account, true).await;
    admit(&f.runtime, &peer, false).await;
    assert!(f.runtime.run_next().await);
    assert_eq!(
        *f.adapter.reads.lock().unwrap(),
        vec![(peer.id.clone(), "feed")]
    );
    assert!(
        !f.runtime.run_next().await,
        "a bounded24h wake cannot release the original48h live observation"
    );
    assert_eq!(f.vault.loads.load(Ordering::SeqCst), 1);
    assert_eq!(deadline(&f.runtime, &f.account).await, original);
    f.clock.move_utc(-72 * 3600);
    boundary_receipt(&f.runtime, &f.adapter, &f.account, 30)
        .await
        .unwrap();
    assert_eq!(deadline(&f.runtime, &f.account).await, original);
    f.clock.move_utc(72 * 3600);
    f.clock.advance_monotonic(23 * 3600 - 1);
    assert!(!f.runtime.run_next().await);
    f.clock.advance_monotonic(1);
    assert!(f.runtime.run_next().await);
    assert_eq!(f.adapter.read_count(), 2);
    assert_eq!(f.vault.loads.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn long_clock_cold_valid_wall_preserves_full_deadline_and_peer_progress() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("cache.sqlite");
    let f = setup(&path).await;
    let (saved, draft) = saved_private(&f.runtime, &f.account).await;
    let peer = add_actor(&f.runtime.store, &f.vault, "b").await;
    boundary_receipt(&f.runtime, &f.adapter, &f.account, 48 * 3600)
        .await
        .unwrap();
    let original = deadline(&f.runtime, &f.account).await;
    f.clock.move_utc(3600);
    f.clock.advance_monotonic(3600);
    let Fixture {
        runtime,
        clock,
        vault,
        adapter,
        account,
    } = f;
    runtime.store.close().await.unwrap();
    drop(runtime);
    let mut cold = CollaborationRuntime::new(
        Arc::new(Store::open(&path).await.unwrap()),
        vault.clone(),
        adapter.clone(),
    );
    cold.clock = clock.clone();
    admit(&cold, &account, true).await;
    admit(&cold, &peer, false).await;
    assert!(cold.run_next().await);
    assert_eq!(*adapter.reads.lock().unwrap(), vec![(peer.id, "feed")]);
    assert!(!cold.run_next().await);
    clock.move_utc(24 * 3600);
    clock.advance_monotonic(24 * 3600);
    let _ = cold.run_next().await; // A bounded wake may only recheck durable quota.
    assert_eq!(adapter.read_count(), 1);
    assert_eq!(vault.loads.load(Ordering::SeqCst), 1);
    assert_eq!(deadline(&cold, &account).await, original);
    assert_private(&cold, &account, &saved, &draft).await;
    clock.move_utc(23 * 3600 - 1);
    clock.advance_monotonic(23 * 3600 - 1);
    assert!(!cold.run_next().await);
    clock.move_utc(1);
    clock.advance_monotonic(1);
    assert!(cold.run_next().await);
    assert_eq!(adapter.read_count(), 2);
    assert_eq!(vault.loads.load(Ordering::SeqCst), 2);
    assert_eq!(
        cold.store.draft(&account.id, "pull").await.unwrap(),
        Some(draft)
    );
}

#[tokio::test]
async fn cold_diagnostics_report_the_full_saved_provider_wait_without_io() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("cache.sqlite");
    let f = setup(&path).await;
    boundary_receipt(&f.runtime, &f.adapter, &f.account, 48 * 3600)
        .await
        .unwrap();
    f.clock.move_utc(3600);
    f.clock.advance_monotonic(3600);
    let Fixture {
        runtime,
        clock,
        vault,
        adapter,
        account,
    } = f;
    runtime.store.close().await.unwrap();
    drop(runtime);
    let mut cold = CollaborationRuntime::new(
        Arc::new(Store::open(&path).await.unwrap()),
        vault.clone(),
        adapter.clone(),
    );
    cold.clock = clock;

    let snapshot = cold.diagnostics().await.unwrap();
    let observed = snapshot
        .accounts
        .iter()
        .find(|candidate| candidate.account_id == account.id)
        .unwrap();
    let recovery = observed.recovery.as_ref().unwrap();
    assert_eq!(recovery.category, SyncRecoveryCategory::RateLimit);
    assert_eq!(recovery.retry_after_seconds, Some(47 * 3600 + 1));
    assert!(!recovery.explicit_retry_eligible);
    assert_eq!(vault.loads.load(Ordering::SeqCst), 0);
    assert_eq!(adapter.read_count(), 0);
}

#[tokio::test]
async fn cold_backward_wall_keeps_authored_state_blocks_own_io_and_serves_a_peer() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("cache.sqlite");
    let f = setup(&path).await;
    let (saved, draft) = saved_private(&f.runtime, &f.account).await;
    let peer = add_actor(&f.runtime.store, &f.vault, "b").await;
    boundary_receipt(&f.runtime, &f.adapter, &f.account, 120)
        .await
        .unwrap();
    let original = deadline(&f.runtime, &f.account).await;
    f.clock.move_utc(-3600);
    f.clock.advance_monotonic(120);
    let Fixture {
        runtime,
        clock,
        vault,
        adapter,
        account,
    } = f;
    runtime.store.close().await.unwrap();
    drop(runtime);
    let mut cold = CollaborationRuntime::new(
        Arc::new(Store::open(&path).await.unwrap()),
        vault.clone(),
        adapter.clone(),
    );
    cold.clock = clock;
    admit(&cold, &account, false).await;
    admit(&cold, &peer, false).await;

    assert!(cold.run_next().await);
    assert_eq!(
        *adapter.reads.lock().unwrap(),
        vec![(peer.id.clone(), "feed")]
    );
    for _ in 0..4 {
        assert!(!cold.run_next().await);
    }
    assert_eq!(deadline(&cold, &account).await, original);
    assert_eq!(vault.loads.load(Ordering::SeqCst), 1);
    assert_private(&cold, &account, &saved, &draft).await;
    let changed = cold
        .save_draft(LocalDraft {
            body: "private after backward-wall recovery".into(),
            ..draft.clone()
        })
        .await
        .unwrap();
    assert_ne!(changed.generation, draft.generation);
    assert_eq!(
        cold.save_draft(draft).await.unwrap_err().code,
        ErrorCode::StaleView
    );
}

#[tokio::test]
async fn cold_forward_wall_accepts_one_fresh_limit_then_stops_io_and_serves_a_peer() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("cache.sqlite");
    let f = setup(&path).await;
    let peer = add_actor(&f.runtime.store, &f.vault, "b").await;
    boundary_receipt(&f.runtime, &f.adapter, &f.account, 120)
        .await
        .unwrap();
    let original = deadline(&f.runtime, &f.account).await;
    let mut fresh_limit = ProviderError::new(ProviderErrorKind::RateLimited);
    fresh_limit.retry_after_seconds = Some(120);
    fresh_limit.account_cooldown_seconds = Some(120);
    f.adapter
        .read_errors
        .lock()
        .unwrap()
        .insert(f.account.id.clone(), fresh_limit);
    f.clock.move_utc(3600);
    f.clock.advance_monotonic(120);
    let Fixture {
        runtime,
        clock,
        vault,
        adapter,
        account,
    } = f;
    runtime.store.close().await.unwrap();
    drop(runtime);
    let mut cold = CollaborationRuntime::new(
        Arc::new(Store::open(&path).await.unwrap()),
        vault.clone(),
        adapter.clone(),
    );
    cold.clock = clock;
    admit(&cold, &account, false).await;
    admit(&cold, &peer, false).await;

    for _ in 0..4 {
        if adapter.read_count() == 2 {
            break;
        }
        assert!(cold.run_next().await);
    }
    let reads = adapter.reads.lock().unwrap().clone();
    assert_eq!(
        reads.iter().filter(|(id, _)| id == &account.id).count(),
        1,
        "an unknowable forward jump permits at most one fresh account attempt"
    );
    assert_eq!(reads.iter().filter(|(id, _)| id == &peer.id).count(), 1);
    let refreshed = deadline(&cold, &account).await;
    assert_ne!(refreshed, original);
    for _ in 0..4 {
        assert!(!cold.run_next().await);
    }
    assert_eq!(adapter.read_count(), 2);
    assert_eq!(vault.loads.load(Ordering::SeqCst), 2);

    admit(&cold, &peer, true).await;
    assert!(cold.run_next().await);
    assert_eq!(
        adapter.reads.lock().unwrap().last(),
        Some(&(peer.id, "body"))
    );
    assert_eq!(adapter.read_count(), 3);
    assert_eq!(vault.loads.load(Ordering::SeqCst), 3);
}

#[tokio::test]
async fn replacement_background_wake_seeds_a_valid_saved_floor_before_native_io() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("cache.sqlite");
    let f = setup(&path).await;
    let (saved, draft) = saved_private(&f.runtime, &f.account).await;
    boundary_receipt(&f.runtime, &f.adapter, &f.account, 120)
        .await
        .unwrap();
    f.clock.move_utc(30);
    f.clock.advance_monotonic(30);
    f.runtime.shutdown().await.unwrap();
    let replacement = Arc::new(
        f.runtime
            .replacement(Arc::new(Store::open(&path).await.unwrap()))
            .unwrap(),
    );
    replacement.clone().start_background();
    admit(&replacement, &f.account, false).await;

    tokio::time::timeout(HANDSHAKE, async {
        loop {
            if replacement
                .scheduler
                .lock()
                .await
                .account_cooldowns
                .contains_key(&f.account.id)
            {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("background wake seeds the durable account floor");
    assert_eq!(f.adapter.read_count(), 0);
    assert_eq!(f.vault.loads.load(Ordering::SeqCst), 0);
    assert_private(&replacement, &f.account, &saved, &draft).await;
    replacement.shutdown().await.unwrap();
}

#[tokio::test]
async fn long_clock_direct_cold_admission_seeds_full_wait_without_enqueue() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("cache.sqlite");
    let f = setup(&path).await;
    let peer = add_actor(&f.runtime.store, &f.vault, "b").await;
    // Retain genuine admitted work, then exercise the shared admission boundary
    // directly after reopening. The new scheduler never enqueues these jobs.
    admit(&f.runtime, &f.account, true).await;
    admit(&f.runtime, &peer, false).await;
    let (mut own_job, mut peer_job) = {
        let scheduler = f.runtime.scheduler.lock().await;
        let job = |account_id: &str| {
            scheduler
                .queue
                .iter()
                .find(|job| job.account.id == account_id)
                .unwrap()
                .clone()
        };
        (job(&f.account.id), job(&peer.id))
    };
    boundary_receipt(&f.runtime, &f.adapter, &f.account, 48 * 3600)
        .await
        .unwrap();
    let original = deadline(&f.runtime, &f.account).await;
    f.clock.move_utc(3600);
    f.clock.advance_monotonic(3600);
    let Fixture {
        runtime,
        clock,
        vault,
        adapter,
        account,
    } = f;
    runtime.store.close().await.unwrap();
    drop(runtime);
    let mut cold = CollaborationRuntime::new(
        Arc::new(Store::open(&path).await.unwrap()),
        vault.clone(),
        adapter.clone(),
    );
    cold.clock = clock.clone();
    assert_eq!(
        cold.ensure_provider_budget(&account, &mut own_job)
            .await
            .unwrap_err()
            .code,
        ErrorCode::RateLimited
    );
    cold.ensure_provider_budget(&peer, &mut peer_job)
        .await
        .unwrap();
    clock.advance_monotonic(25 * 3600);
    clock.move_utc(72 * 3600);
    assert_eq!(
        cold.ensure_provider_budget(&account, &mut own_job)
            .await
            .expect_err("cold admission must retain all 47 remaining hours")
            .code,
        ErrorCode::RateLimited
    );
    cold.ensure_provider_budget(&peer, &mut peer_job)
        .await
        .unwrap();
    assert_eq!(deadline(&cold, &account).await, original);
    {
        let scheduler = cold.scheduler.lock().await;
        assert!(scheduler.queue.is_empty());
        assert_eq!(scheduler.account_cooldowns.len(), 1);
    }
    clock.advance_monotonic(22 * 3600 - 1);
    assert!(
        cold.ensure_provider_budget(&account, &mut own_job)
            .await
            .is_err()
    );
    clock.advance_monotonic(1);
    cold.ensure_provider_budget(&account, &mut own_job)
        .await
        .unwrap();
    assert_eq!(deadline(&cold, &account).await, original);
    assert_eq!(adapter.read_count(), 0);
    assert_eq!(vault.loads.load(Ordering::SeqCst), 0);
}
