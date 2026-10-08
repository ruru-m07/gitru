//! One native scheduler and credential owner shared by every application webview.

use crate::{
    credentials::{CredentialVault, SecretToken},
    github_cli::GithubCli,
    providers::*,
    *,
};
use chrono::{DateTime, Utc};
use std::{
    collections::{HashMap, VecDeque},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};
use tokio::sync::{Mutex, Notify, broadcast};
#[cfg(test)]
mod bitbucket_participants_tests;
#[cfg(test)]
mod bitbucket_resource_reads_tests;
#[cfg(test)]
mod bitbucket_tasks_tests;
#[cfg(test)]
mod bitbucket_tests;
mod clock;
#[cfg(test)]
mod clock_lifecycle_tests;
mod command_recovery;
mod comment_send;
mod delivery;
mod demand;
#[cfg(test)]
mod demand_tests;
#[cfg(test)]
pub(crate) mod detail_tests;
mod details;
mod feeds;
mod file_artifacts;
#[cfg(test)]
mod github_comments_tests;
#[cfg(test)]
mod gitlab_probe_backoff_tests;
#[cfg(test)]
mod gitlab_tests;
#[cfg(feature = "test-harness")]
mod harness;
#[cfg(test)]
mod notification_subject_tests;
mod notification_subjects;
#[cfg(test)]
mod pull_commit_tests;
mod pull_commits;
#[cfg(test)]
mod pull_file_tests;
mod pull_files;
mod scheduler;
mod shutdown;
mod text_edits;
pub use shutdown::RuntimeOperation;

#[cfg(test)]
mod diagnostics_unit_tests {
    use super::*;

    #[test]
    fn actual_elapsed_histogram_is_constant_space_and_saturating() {
        let mut histogram = LatencyAccumulator::default();
        for milliseconds in [1, 11, 99, 249, 499, 999, 1_999, 2_001] {
            histogram.record(Duration::from_millis(milliseconds));
        }
        let snapshot = histogram.snapshot();
        assert_eq!(snapshot.sample_count, 8);
        assert_eq!(snapshot.total_milliseconds, 5_858);
        assert_eq!(snapshot.maximum_milliseconds, Some(2_001));
        assert_eq!(snapshot.p50_upper_bound_milliseconds, Some(250));
        assert_eq!(snapshot.p95_upper_bound_milliseconds, Some(2_001));
        assert_eq!(snapshot.p99_upper_bound_milliseconds, Some(2_001));
        assert_eq!(histogram.buckets.len(), 8);
    }
}

const MAX_QUEUED_SCOPES: usize = 128;
const MAX_PAGES_PER_REFRESH: usize = 10;

macro_rules! credential_boundary {
    ($name:literal) => {
        #[cfg(test)]
        credential_crash_tests::checkpoint($name);
    };
}

#[derive(Clone)]
enum JobKind {
    PullFileArtifact {
        request: Box<PullFileDiffRequest>,
    },
    NotificationSubject {
        intent: NotificationDiscoveryIntent,
    },
    Feed(FeedKind),
    Detail {
        subject_id: String,
        facet: DetailFacet,
    },
}

#[derive(Clone)]
struct Job {
    key: String,
    account: RemoteAccount,
    repository: Option<RemoteRepository>,
    kind: JobKind,
    scope: String,
    reason: scheduler::Admission,
    pages: usize,
    detail_lease: Option<DetailLease>,
    detail_restarted: bool,
    pull_commit_lease: Option<PullCommitLease>,
    pull_commit_restarted: bool,
    local_budget_refusal: bool,
    enqueued_at: Instant,
}

const DIAGNOSTIC_LATENCY_BOUNDS_MS: [u64; 8] = [10, 30, 100, 250, 500, 1_000, 2_000, u64::MAX];

#[derive(Default)]
struct LatencyAccumulator {
    count: u64,
    total_ms: u64,
    maximum_ms: u64,
    buckets: [u64; DIAGNOSTIC_LATENCY_BOUNDS_MS.len()],
}

impl LatencyAccumulator {
    fn record(&mut self, elapsed: Duration) {
        let milliseconds = u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX);
        self.count = self.count.saturating_add(1);
        self.total_ms = self.total_ms.saturating_add(milliseconds);
        self.maximum_ms = self.maximum_ms.max(milliseconds);
        let index = DIAGNOSTIC_LATENCY_BOUNDS_MS
            .iter()
            .position(|bound| milliseconds <= *bound)
            .unwrap_or(DIAGNOSTIC_LATENCY_BOUNDS_MS.len() - 1);
        self.buckets[index] = self.buckets[index].saturating_add(1);
    }

    fn snapshot(&self) -> SyncLatencyDiagnostics {
        SyncLatencyDiagnostics {
            sample_count: self.count,
            total_milliseconds: self.total_ms,
            maximum_milliseconds: (self.count > 0).then_some(self.maximum_ms),
            p50_upper_bound_milliseconds: self.quantile(50),
            p95_upper_bound_milliseconds: self.quantile(95),
            p99_upper_bound_milliseconds: self.quantile(99),
        }
    }

    fn quantile(&self, percentile: u64) -> Option<u64> {
        if self.count == 0 {
            return None;
        }
        let rank = self.count.saturating_mul(percentile).saturating_add(99) / 100;
        let mut seen = 0u64;
        for (index, count) in self.buckets.iter().enumerate() {
            seen = seen.saturating_add(*count);
            if seen >= rank {
                let bound = DIAGNOSTIC_LATENCY_BOUNDS_MS[index];
                return Some(if bound == u64::MAX {
                    self.maximum_ms
                } else {
                    bound
                });
            }
        }
        Some(self.maximum_ms)
    }
}

#[derive(Default)]
struct Scheduler {
    demands: demand::Demands,
    queue: VecDeque<Job>,
    deferred: VecDeque<Job>,
    active: HashMap<String, String>,
    due: HashMap<String, Instant>,
    strict_scope_deadlines: HashMap<String, Instant>,
    account_cooldowns: HashMap<String, clock::AccountCooldown>,
    failures: HashMap<String, u32>,
    background_cursor: usize,
    interactive_turns: u8,
    detail_turns: u8,
    interactive_account: Option<String>,
    reconciliation_account: Option<String>,
    explicit_keys: std::collections::HashSet<String>,
    manual_keys: std::collections::HashSet<String>,
    foreground_keys: std::collections::HashSet<String>,
    detail_cursor: Option<DetailDemand>,
    delivery_account_cursor: Option<String>,
    delivery_cursors: HashMap<String, String>,
    delivery_clock: Option<(DateTime<Utc>, Instant)>,
}

#[derive(Default)]
struct QueueObservation {
    ready: u32,
    deferred: u32,
    oldest_seconds: Option<u64>,
    cooldown_seconds: Option<u64>,
}

#[derive(Clone)]
pub struct CollaborationRuntime {
    clock: Arc<dyn clock::Clock>,
    demand_visibility: Arc<dyn Fn(&str) -> bool + Send + Sync>,
    store: Arc<Store>,
    vault: Arc<dyn CredentialVault>,
    registry: Arc<ProviderRegistry>,
    github_cli: Arc<GithubCli>,
    lifecycle: Arc<Mutex<()>>,
    dispatch: Arc<Mutex<()>>,
    scheduler: Arc<Mutex<Scheduler>>,
    diagnostic_latency: Arc<std::sync::Mutex<LatencyAccumulator>>,
    notify: Arc<Notify>,
    started: Arc<AtomicBool>,
    lifetime: Arc<shutdown::RuntimeLifetime>,
    changes: broadcast::Sender<ChangeHint>,
}

impl CollaborationRuntime {
    pub fn new(
        store: Arc<Store>,
        vault: Arc<dyn CredentialVault>,
        provider: Arc<dyn CollaborationProvider>,
    ) -> Self {
        let mut registry = ProviderRegistry::default();
        registry
            .register(provider)
            .expect("valid explicitly bound adapter");
        Self::with_registry(store, vault, registry)
    }

    pub fn with_registry(
        store: Arc<Store>,
        vault: Arc<dyn CredentialVault>,
        registry: ProviderRegistry,
    ) -> Self {
        let (changes, _) = broadcast::channel(64);
        Self {
            clock: Arc::new(clock::SystemClock),
            demand_visibility: Arc::new(|_| true),
            store,
            vault,
            registry: Arc::new(registry),
            github_cli: Arc::new(GithubCli::disabled()),
            lifecycle: Arc::new(Mutex::new(())),
            dispatch: Arc::new(Mutex::new(())),
            scheduler: Arc::new(Mutex::new(Scheduler::default())),
            diagnostic_latency: Arc::new(std::sync::Mutex::new(LatencyAccumulator::default())),
            notify: Arc::new(Notify::new()),
            started: Arc::new(AtomicBool::new(false)),
            lifetime: Arc::new(shutdown::RuntimeLifetime::default()),
            changes,
        }
    }

    pub fn with_github_cli(mut self, github_cli: GithubCli) -> Self {
        self.github_cli = Arc::new(github_cli);
        self
    }

    pub async fn discover_github_cli(&self) -> GithubCliDiscovery {
        let Ok(_operation) = self.acquire_operation() else {
            return GithubCliDiscovery {
                status: GithubCliStatus::Unavailable,
                accounts: vec![],
            };
        };
        self.github_cli.discover().await
    }

    pub async fn connect_github_cli(
        &self,
        candidate_id: &str,
    ) -> Result<RemoteAccount, CollaborationError> {
        let _operation = self.acquire_operation()?;
        let (expected_login, token) = self.github_cli.import(candidate_id).await?;
        self.connect_owned_token(
            ProviderInstance::public(ProviderKind::Github),
            token,
            Some(expected_login),
        )
        .await
    }

    pub fn store(&self) -> &Arc<Store> {
        &self.store
    }

    pub fn subscribe(&self) -> broadcast::Receiver<ChangeHint> {
        self.changes.subscribe()
    }

    /// Local-only support state. This path never loads a credential, admits a
    /// job or calls a provider.
    pub async fn diagnostics(&self) -> Result<SyncDiagnosticsSnapshot, CollaborationError> {
        let (revision, saved) = self.store.saved_diagnostics().await?;
        let now = self.now();
        let mut queue_by_account = HashMap::<String, QueueObservation>::new();
        {
            let scheduler = self.scheduler.lock().await;
            for job in &scheduler.queue {
                let observation = queue_by_account.entry(job.account.id.clone()).or_default();
                observation.ready = observation.ready.saturating_add(1);
                let age = now
                    .checked_duration_since(job.enqueued_at)
                    .unwrap_or_default()
                    .as_secs();
                observation.oldest_seconds = Some(observation.oldest_seconds.unwrap_or(0).max(age));
            }
            for job in &scheduler.deferred {
                let observation = queue_by_account.entry(job.account.id.clone()).or_default();
                observation.deferred = observation.deferred.saturating_add(1);
                let age = now
                    .checked_duration_since(job.enqueued_at)
                    .unwrap_or_default()
                    .as_secs();
                observation.oldest_seconds = Some(observation.oldest_seconds.unwrap_or(0).max(age));
            }
            for (account_id, deadline) in &scheduler.account_cooldowns {
                if let Some(remaining) = deadline.remaining(now) {
                    queue_by_account
                        .entry(account_id.clone())
                        .or_default()
                        .cooldown_seconds = Some(remaining.as_secs().saturating_add(1));
                }
            }
        }

        let cache = self.store.cache_usage().await.ok();
        let wal = self.store.wal_status().await.ok();
        let storage = StorageDiagnostics {
            cache_usage_available: cache.as_ref().is_some_and(|usage| usage.available),
            logical_bytes: cache.as_ref().and_then(|usage| usage.logical_bytes),
            indexed_logical_bytes: cache
                .as_ref()
                .filter(|usage| usage.available)
                .map(|usage| usage.indexed_logical_bytes),
            database_bytes: wal
                .as_ref()
                .map(|status| status.database_bytes)
                .or_else(|| cache.as_ref().map(|usage| usage.database_bytes)),
            wal_bytes: wal
                .as_ref()
                .map(|status| status.wal_bytes)
                .or_else(|| cache.as_ref().map(|usage| usage.wal_bytes)),
            wal_observation_supported: wal.as_ref().is_some_and(|status| status.supported),
            wal_busy: wal
                .as_ref()
                .filter(|status| status.supported)
                .and_then(|status| status.busy)
                .map(|busy| busy > 0),
            wal_log_frames: wal
                .as_ref()
                .filter(|status| status.supported)
                .and_then(|status| status.log_frames)
                .and_then(|frames| u64::try_from(frames).ok()),
            wal_checkpointed_frames: wal
                .as_ref()
                .filter(|status| status.supported)
                .and_then(|status| status.checkpointed_frames)
                .and_then(|frames| u64::try_from(frames).ok()),
        };
        let latency = self
            .diagnostic_latency
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .snapshot();

        let mut accounts = Vec::with_capacity(saved.len());
        for saved_account in saved {
            let queue = queue_by_account
                .remove(&saved_account.account.id)
                .unwrap_or_default();
            let recovery = saved_account.recovery.map(|recovery| {
                let retry_after_seconds = recovery
                    .next_retry_at
                    .as_deref()
                    .and_then(|time| self.delay_until(time))
                    .map(|delay| delay.as_secs().saturating_add(1));
                let explicit_retry_eligible = saved_account.account.state == AccountState::Active
                    && retry_after_seconds.is_none()
                    && matches!(
                        recovery.category,
                        SyncRecoveryCategory::RateLimit
                            | SyncRecoveryCategory::Offline
                            | SyncRecoveryCategory::Unavailable
                    );
                SyncRecoveryState {
                    category: recovery.category,
                    affected_scopes: recovery.affected_scopes,
                    next_retry_at: recovery.next_retry_at,
                    retry_after_seconds,
                    explicit_retry_eligible,
                }
            });
            accounts.push(AccountSyncDiagnostics {
                account_id: saved_account.account.id,
                provider: saved_account.account.provider,
                coverage: saved_account.coverage,
                ready_jobs: queue.ready,
                deferred_jobs: queue.deferred,
                oldest_job_age_seconds: queue.oldest_seconds,
                cooldown_remaining_seconds: queue.cooldown_seconds,
                recovery,
            });
        }
        let ready_jobs = accounts
            .iter()
            .fold(0u32, |sum, account| sum.saturating_add(account.ready_jobs));
        let deferred_jobs = accounts.iter().fold(0u32, |sum, account| {
            sum.saturating_add(account.deferred_jobs)
        });
        let oldest_job_age_seconds = accounts
            .iter()
            .filter_map(|account| account.oldest_job_age_seconds)
            .max();
        let accounts_in_cooldown = u32::try_from(
            accounts
                .iter()
                .filter(|account| account.cooldown_remaining_seconds.is_some())
                .count(),
        )
        .unwrap_or(u32::MAX);
        Ok(SyncDiagnosticsSnapshot {
            generated_at: self.now_string(),
            revision,
            accounts,
            ready_jobs,
            deferred_jobs,
            oldest_job_age_seconds,
            accounts_in_cooldown,
            latency,
            storage,
        })
    }

    pub async fn save_draft(&self, draft: LocalDraft) -> Result<LocalDraft, CollaborationError> {
        let _operation = self.acquire_operation()?;
        let draft = self.store.save_draft(draft).await?;
        self.publish(self.store.revision().await?);
        Ok(draft)
    }

    pub async fn set_local_inbox_state(
        &self,
        request: SetLocalInboxStateRequest,
    ) -> Result<LocalInboxWriteReceipt, CollaborationError> {
        let _operation = self.acquire_operation()?;
        let receipt = self.store.set_local_inbox_state(request).await?;
        self.publish(receipt.revision.clone());
        Ok(receipt)
    }

    /// Idempotent process-level startup. A weak owner permits normal application
    /// teardown; the background loop does not keep an otherwise dropped runtime.
    pub fn start_background(self: Arc<Self>) {
        let Ok(mut background) = self.lifetime.background.lock() else {
            return;
        };
        if self.is_stopping() {
            return;
        }
        if self.started.swap(true, Ordering::AcqRel) {
            return;
        }
        let owner = Arc::downgrade(&self);
        let notify = self.notify.clone();
        *background = Some(tokio::spawn(async move {
            let mut timer = tokio::time::interval(Duration::from_secs(10));
            timer.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            loop {
                tokio::select! { _ = timer.tick() => {}, _ = notify.notified() => {} }
                let Some(runtime) = owner.upgrade() else {
                    break;
                };
                if runtime.is_stopping() {
                    break;
                }
                // A damaged database does not terminate scheduling permanently;
                // a later tick can recover, and local calls report their errors.
                let _ = runtime.recover_credentials().await;
                let _ = runtime.enqueue_due().await;
                loop {
                    let delivered = runtime.run_delivery_next().await.unwrap_or(false);
                    let read = runtime.run_next().await;
                    if !delivered && !read {
                        break;
                    }
                    let _ = runtime.enqueue_due().await;
                    tokio::task::yield_now().await;
                }
            }
        }));
    }

    pub async fn connect_github(&self, token: String) -> Result<RemoteAccount, CollaborationError> {
        let token = SecretToken::new(token).map_err(|_| {
            CollaborationError::invalid("Enter a valid GitHub personal access token")
        })?;
        self.connect_owned_token(ProviderInstance::public(ProviderKind::Github), token, None)
            .await
    }

    pub async fn connect_gitlab(&self, token: String) -> Result<RemoteAccount, CollaborationError> {
        let token = SecretToken::new(token).map_err(|_| {
            CollaborationError::invalid("Enter a valid GitLab personal access token")
        })?;
        self.connect_owned_token(ProviderInstance::public(ProviderKind::Gitlab), token, None)
            .await
    }

    pub async fn connect_bitbucket_cloud(
        &self,
        token: String,
    ) -> Result<RemoteAccount, CollaborationError> {
        let token = SecretToken::new(token)
            .map_err(|_| CollaborationError::invalid("Enter a valid Bitbucket Cloud API token"))?;
        self.connect_owned_token(
            ProviderInstance::public(ProviderKind::BitbucketCloud),
            token,
            None,
        )
        .await
    }

    async fn connect_owned_token(
        &self,
        instance: ProviderInstance,
        token: SecretToken,
        expected_login: Option<String>,
    ) -> Result<RemoteAccount, CollaborationError> {
        self.owned_operation(move |runtime| async move {
            runtime
                .connect_provider_token(instance, token, expected_login.as_deref())
                .await
        })
        .await
    }

    async fn connect_provider_token(
        &self,
        instance: ProviderInstance,
        token: SecretToken,
        expected_login: Option<&str>,
    ) -> Result<RemoteAccount, CollaborationError> {
        let _lifecycle = self.lifecycle.lock().await;
        let _ = self.cleanup_credentials_locked().await;
        // Only native fixed-provider wrappers supply this installation. The
        // registry validates its exact binding; there is no family fallback.
        let provider = self.registry.adapter(&instance)?;
        let verified = match provider.probe_with_backoff(&token).await {
            Ok(verified) => verified,
            Err(failure) => {
                if let Some(actor_id) = failure.verified_actor_id.as_deref()
                    && let Some(delay) = failure
                        .error
                        .account_cooldown_seconds
                        .into_iter()
                        .chain(failure.error.retry_after_seconds)
                        .filter(|seconds| *seconds > 0)
                        .max()
                    && let Some(known) =
                        self.store
                            .accounts()
                            .await?
                            .accounts
                            .into_iter()
                            .find(|account| {
                                account.provider == instance.provider
                                    && ProviderInstance::for_account(account)
                                        .is_ok_and(|old| old == instance)
                                    && account.actor_id == actor_id
                            })
                {
                    // Consumption evidence belongs to this proven actor's
                    // current epoch, even while disconnected/auth-required.
                    // A rejected prospective token cannot revoke its grant.
                    self.persist_rate_limit(&known, delay, None).await?;
                }
                return Err(failure.error.into());
            }
        };
        let quota_observation = verified
            .cooldown_seconds
            .filter(|seconds| *seconds > 0)
            .map(|seconds| self.capture_provider_budget(seconds));
        if expected_login.is_some_and(|expected| !verified.login.eq_ignore_ascii_case(expected)) {
            return Err(CollaborationError::new(
                ErrorCode::StaleView,
                "GitHub CLI credentials changed; check accounts again before connecting",
            ));
        }
        let previous = self
            .store
            .accounts()
            .await?
            .accounts
            .into_iter()
            .find(|account| {
                account.provider == instance.provider
                    && ProviderInstance::for_account(account).is_ok_and(|old| old == instance)
                    && account.actor_id == verified.actor_id
            });
        let id = previous
            .as_ref()
            .map(|account| account.id.clone())
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
        let epoch = match &previous {
            Some(account) => next_epoch(&account.authorization_epoch)?,
            None => "1".to_string(),
        };
        let reference = format!("credential:{}", uuid::Uuid::new_v4());
        credential_boundary!("before_stage");
        self.store.stage_credential(&id, &reference).await?;
        credential_boundary!("after_stage");
        credential_boundary!("before_vault_write");
        if let Err(error) = self.store_token(&reference, &token).await {
            // A vault may report failure after a partial side effect. The
            // journal remains durable until deletion is confirmed.
            let _ = self.cleanup_credentials_locked().await;
            return Err(error);
        }
        credential_boundary!("after_vault_write");
        let account = RemoteAccount {
            id: id.clone(),
            provider: instance.provider,
            host: instance
                .base_url
                .strip_prefix("https://")
                .expect("registered HTTPS installation")
                .trim_end_matches('/')
                .to_string(),
            actor_id: verified.actor_id,
            login: verified.login,
            display_name: verified.display_name,
            authorization_epoch: epoch,
            state: AccountState::Active,
            notifications_supported: verified.notifications_supported,
        };
        credential_boundary!("before_cutover");
        let committed = {
            // Publish the accepted probe budget to live dispatch checks before
            // releasing scheduler serialization. Vault work is already done.
            let mut scheduler = self.scheduler.lock().await;
            let result = self
                .store
                .commit_account_credential_with_quota(
                    account,
                    &reference,
                    quota_observation
                        .as_ref()
                        .map(|(deadline, _)| deadline.clone()),
                )
                .await;
            if let Ok(account) = &result
                && let Some((_, deadline)) = &quota_observation
            {
                Self::install_provider_cooldown(&mut scheduler, &account.id, *deadline);
            }
            result
        };
        let account = match committed {
            Ok(account) => account,
            Err(error) => {
                let _ = self.cleanup_credentials_locked().await;
                return Err(error);
            }
        };
        credential_boundary!("after_cutover");
        self.reset_account_scheduler(&account.id).await;
        self.publish(self.store.revision().await?);
        if let Err(error) = self
            .refresh(RefreshRequest {
                account_id: account.id.clone(),
                repository_id: None,
                kind: None,
            })
            .await
        {
            // Authentication has committed. A saturated refresh queue must not
            // make a successfully connected account look like a failed login.
            if error.code != ErrorCode::Busy
                && let Ok(revision) = self
                    .store
                    .set_sync_status(
                        &account.id,
                        &account.authorization_epoch,
                        "repositories",
                        SyncStatus {
                            state: SyncState::Error,
                            last_success_at: None,
                            next_retry_at: Some(self.future_string(60)),
                            error: Some(error),
                        },
                    )
                    .await
            {
                self.publish(revision);
            }
        }
        // Promotion has committed even if retirement cleanup fails. Do not
        // present a working replacement as a failed connection.
        let _ = self.cleanup_credentials_locked().await;
        Ok(account)
    }

    pub async fn disconnect(&self, account_id: &str) -> Result<String, CollaborationError> {
        let account_id = account_id.to_string();
        self.owned_operation(
            move |runtime| async move { runtime.disconnect_inner(&account_id).await },
        )
        .await
    }
    async fn disconnect_inner(&self, account_id: &str) -> Result<String, CollaborationError> {
        let _lifecycle = self.lifecycle.lock().await;
        credential_boundary!("before_disconnect");
        let reference = self.store.credential_reference(account_id).await?;
        let revision = self.store.disconnect(account_id).await?;
        credential_boundary!("after_disconnect");
        self.reset_account_scheduler(account_id).await;
        // Epoch invalidation commits before removal, so in-flight observations
        // cannot become visible even if the native vault is temporarily locked.
        self.publish(revision.clone());
        // The explicit disconnect cleans its own secret first, even when the
        // bounded background batch contains unrelated retired references.
        if let Some(reference) = reference
            && let Some(entry) = self.store.credential_cleanup(&reference).await?
        {
            self.cleanup_credential_locked(entry).await?;
        }
        Ok(revision)
    }

    pub async fn select_repository(
        &self,
        account_id: &str,
        repository_id: &str,
        selected: bool,
    ) -> Result<String, CollaborationError> {
        let _operation = self.acquire_operation()?;
        let _lifecycle = self.lifecycle.lock().await;
        let account = self.active_account(account_id).await?;
        let revision = self
            .store
            .select_repository(account_id, repository_id, selected)
            .await?;
        self.publish(revision.clone());
        if selected {
            let repository = self.store.repository(account_id, repository_id).await?;
            for kind in [FeedKind::PullRequests, FeedKind::Issues] {
                if self.require_feed(&account, kind).await.is_err() {
                    continue;
                }
                self.enqueue(account.clone(), Some(repository.clone()), kind, true)
                    .await?;
            }
        } else {
            let mut scheduler = self.scheduler.lock().await;
            let removed: Vec<String> = scheduler
                .queue
                .iter()
                .chain(scheduler.deferred.iter())
                .filter(|job| {
                    job.account.id == account_id
                        && job
                            .repository
                            .as_ref()
                            .is_some_and(|repo| repo.id == repository_id)
                })
                .map(|job| job.key.clone())
                .collect();
            scheduler.queue.retain(|job| !removed.contains(&job.key));
            scheduler.deferred.retain(|job| !removed.contains(&job.key));
            for key in removed {
                scheduler.active.remove(&key);
                scheduler.explicit_keys.remove(&key);
                scheduler.manual_keys.remove(&key);
                scheduler.foreground_keys.remove(&key);
            }
        }
        self.notify.notify_one();
        Ok(revision)
    }

    /// Admission is local and bounded. A receipt identifies coalesced refresh
    /// work; it is not a claim that the remote provider has been contacted.
    pub async fn refresh(
        &self,
        request: RefreshRequest,
    ) -> Result<RefreshReceipt, CollaborationError> {
        let _operation = self.acquire_operation()?;
        let account = self.active_account(&request.account_id).await?;
        let mut first_job = None;
        if let Some(repository_id) = request.repository_id {
            let repository = self.store.repository(&account.id, &repository_id).await?;
            if !repository.selected {
                return Err(CollaborationError::invalid(
                    "Select this repository before synchronizing it",
                ));
            }
            let kinds = match request.kind {
                Some(RemoteItemKind::PullRequest) => vec![FeedKind::PullRequests],
                Some(RemoteItemKind::Issue) => vec![FeedKind::Issues],
                Some(RemoteItemKind::Notification) => {
                    return Err(CollaborationError::invalid(
                        "Notifications use an account inbox scope",
                    ));
                }
                None => vec![FeedKind::PullRequests, FeedKind::Issues],
            };
            for kind in kinds {
                if request.kind.is_none() && self.require_feed(&account, kind).await.is_err() {
                    continue;
                }
                first_job.get_or_insert(
                    self.enqueue(account.clone(), Some(repository.clone()), kind, true)
                        .await?,
                );
            }
        } else {
            if request.kind.is_none() {
                first_job.get_or_insert(
                    self.enqueue(account.clone(), None, FeedKind::Repositories, true)
                        .await?,
                );
            }
            if request.kind.is_none() || request.kind == Some(RemoteItemKind::Notification) {
                if self
                    .require_feed(&account, FeedKind::Notifications)
                    .await
                    .is_ok()
                {
                    first_job.get_or_insert(
                        self.enqueue(account.clone(), None, FeedKind::Notifications, true)
                            .await?,
                    );
                } else if request.kind.is_some() {
                    self.require_feed(&account, FeedKind::Notifications).await?;
                }
            }
            if request.kind != Some(RemoteItemKind::Notification) {
                for repository in self
                    .store
                    .repositories(&account.id)
                    .await?
                    .repositories
                    .into_iter()
                    .filter(|repo| repo.selected)
                {
                    for kind in [FeedKind::PullRequests, FeedKind::Issues] {
                        if request.kind.is_none()
                            && self.require_feed(&account, kind).await.is_err()
                        {
                            continue;
                        }
                        if request.kind.is_none()
                            || (kind == FeedKind::PullRequests
                                && request.kind == Some(RemoteItemKind::PullRequest))
                            || (kind == FeedKind::Issues
                                && request.kind == Some(RemoteItemKind::Issue))
                        {
                            first_job.get_or_insert(
                                self.enqueue(account.clone(), Some(repository.clone()), kind, true)
                                    .await?,
                            );
                        }
                    }
                }
            }
        }
        self.notify.notify_one();
        Ok(RefreshReceipt {
            job_id: first_job.unwrap_or_else(|| uuid::Uuid::new_v4().to_string()),
        })
    }

    async fn active_account(&self, account_id: &str) -> Result<RemoteAccount, CollaborationError> {
        let account = self.store.account(account_id).await?;
        if account.state != AccountState::Active {
            return Err(CollaborationError::new(
                ErrorCode::AuthRequired,
                "Reconnect this provider account",
            ));
        }
        self.adapter_for_account(&account).await?;
        Ok(account)
    }

    async fn adapter_for_account(
        &self,
        account: &RemoteAccount,
    ) -> Result<Arc<dyn CollaborationProvider>, CollaborationError> {
        let instance = self.store.provider_instance(&account.id).await?;
        if instance.provider != account.provider
            || instance != ProviderInstance::for_account(account)?
        {
            return Err(stale());
        }
        self.registry.adapter(&instance)
    }

    async fn require_feed(
        &self,
        account: &RemoteAccount,
        kind: FeedKind,
    ) -> Result<(), CollaborationError> {
        let capability = self
            .adapter_for_account(account)
            .await?
            .profile(account)
            .facet(kind.facet());
        match capability.state {
            CapabilityState::Supported => Ok(()),
            CapabilityState::Unsupported => Err(unsupported()),
            CapabilityState::Unavailable => Err(CollaborationError::new(
                if account.state != AccountState::Active {
                    ErrorCode::AuthRequired
                } else {
                    ErrorCode::PermissionDenied
                },
                "This provider capability is currently unavailable",
            )),
        }
    }

    /// Atomic local contextual policy, with the captured actor/installation.
    pub async fn contextual_capabilities(
        &self,
        request: ContextCapabilityRequest,
    ) -> Result<ContextualCapabilitySnapshot, CollaborationError> {
        let _operation = self.acquire_operation()?;
        self.store
            .contextual_capabilities(request, |account, instance| {
                match self.registry.adapter(instance) {
                    Ok(provider) => provider.profile(account),
                    Err(_) => ProviderProfile::unavailable(CapabilityReason::AdapterUnavailable),
                }
            })
            .await
    }

    /// Local capability observation. Dispatch rechecks the adapter/current actor.
    pub async fn capabilities(
        &self,
        account_id: &str,
    ) -> Result<CapabilitySnapshot, CollaborationError> {
        let _operation = self.acquire_operation()?;
        let snapshot = self.store.accounts().await?;
        let account = snapshot
            .accounts
            .into_iter()
            .find(|a| a.id == account_id)
            .ok_or_else(|| {
                CollaborationError::new(ErrorCode::NotFound, "Provider account was not found")
            })?;
        let instance = self.store.provider_instance(account_id).await?;
        let mut profile = match self.registry.adapter(&instance) {
            Ok(provider) => provider.profile(&account),
            Err(_) => ProviderProfile::unavailable(CapabilityReason::AdapterUnavailable),
        };
        for capability in &mut profile.facets {
            if account.state != AccountState::Active {
                if capability.state != CapabilityState::Unsupported
                    && capability.reason != Some(CapabilityReason::AdapterUnavailable)
                {
                    capability.state = CapabilityState::Unavailable;
                    capability.reason = Some(CapabilityReason::AuthenticationRequired);
                }
            } else {
                if capability.state != CapabilityState::Supported {
                    continue;
                }
                let scope = match capability.facet {
                    ResourceFacet::Repositories => Some("repositories"),
                    ResourceFacet::Inbox => Some("notifications"),
                    _ => None,
                };
                if let Some(scope) = scope {
                    let status = self.store.scope_state(account_id, scope).await?;
                    if let Some(status) = status.and_then(|s| s.sync.error)
                        && matches!(
                            status.code,
                            ErrorCode::PermissionDenied
                                | ErrorCode::Provider
                                | ErrorCode::Network
                                | ErrorCode::RateLimited
                        )
                    {
                        capability.state = CapabilityState::Unavailable;
                        capability.reason = Some(if status.code == ErrorCode::PermissionDenied {
                            CapabilityReason::PermissionDenied
                        } else {
                            CapabilityReason::TemporarilyUnavailable
                        });
                    }
                }
            }
        }
        Ok(CapabilitySnapshot {
            account_id: account.id,
            instance,
            facets: profile.facets,
            inbox_semantics: profile.inbox_semantics,
            revision: snapshot.revision,
            authorization_view: snapshot.authorization_view,
        })
    }

    async fn enqueue_due(&self) -> Result<(), CollaborationError> {
        self.enqueue_pending_notification_subjects().await?;
        self.enqueue_pending_details().await?;
        let accounts: Vec<_> = self
            .store
            .accounts()
            .await?
            .accounts
            .into_iter()
            .filter(|account| account.state == AccountState::Active)
            .collect();
        let start = self.scheduler.lock().await.background_cursor;
        let mut total = 0;
        // Iterate one account's local repository snapshot at a time. Two bounded
        // passes implement a rotating admission cursor without cloning every
        // account/repository combination before the 128-job limit is applied.
        for pass in 0..2 {
            total = 0;
            for account in &accounts {
                if self.adapter_for_account(account).await.is_err() {
                    continue;
                }
                let repositories = self.store.repositories(&account.id).await?.repositories;
                let scopes = std::iter::once((None, FeedKind::Repositories))
                    .chain(std::iter::once((None, FeedKind::Notifications)))
                    .chain(
                        repositories
                            .into_iter()
                            .filter(|repo| repo.selected)
                            .flat_map(|repo| {
                                [
                                    (Some(repo.clone()), FeedKind::PullRequests),
                                    (Some(repo), FeedKind::Issues),
                                ]
                            }),
                    );
                for (repository, kind) in scopes {
                    let index = total;
                    total += 1;
                    if (pass == 0 && index < start) || (pass == 1 && index >= start) {
                        continue;
                    }
                    if self.require_feed(account, kind).await.is_err() {
                        continue;
                    }
                    match self.enqueue(account.clone(), repository, kind, false).await {
                        Ok(_) => {}
                        Err(error) if error.code == ErrorCode::Busy => {
                            self.scheduler.lock().await.background_cursor = index;
                            return Ok(());
                        }
                        Err(error) => return Err(error),
                    }
                }
            }
            if start == 0 {
                break;
            }
        }
        self.scheduler.lock().await.background_cursor =
            if total > 0 { (start + 1) % total } else { 0 };
        Ok(())
    }

    async fn enqueue(
        &self,
        account: RemoteAccount,
        repository: Option<RemoteRepository>,
        kind: FeedKind,
        manual: bool,
    ) -> Result<String, CollaborationError> {
        self.require_feed(&account, kind).await?;
        let scope = scope_name(kind, repository.as_ref());
        self.enqueue_work(account, repository, JobKind::Feed(kind), scope, manual)
            .await
    }

    async fn enqueue_work(
        &self,
        account: RemoteAccount,
        repository: Option<RemoteRepository>,
        kind: JobKind,
        scope: String,
        manual: bool,
    ) -> Result<String, CollaborationError> {
        self.enqueue_work_reason(
            account,
            repository,
            kind,
            scope,
            if manual {
                scheduler::Admission::Manual
            } else {
                scheduler::Admission::Reconcile
            },
        )
        .await
    }

    async fn run_next(&self) -> bool {
        let Ok(_operation) = self.acquire_operation() else {
            return false;
        };
        // Public actions can wake the engine concurrently, but provider reads
        // remain one lane. The next wake handles any newly admitted interest.
        let Ok(_dispatch) = self.dispatch.try_lock() else {
            return false;
        };
        let _ = self.enqueue_foreground().await;
        let Some(mut job) = self.scheduler.lock().await.pick(self.now()) else {
            return false;
        };
        job.local_budget_refusal = false;
        for scope in ["provider:rest".to_string(), job.scope.clone()] {
            if let Ok(Some(state)) = self.store.scope_state(&job.account.id, &scope).await
                && let Some(delay) = state
                    .sync
                    .next_retry_at
                    .as_deref()
                    .and_then(|time| self.delay_until(time))
            {
                let mut scheduler = self.scheduler.lock().await;
                let deadline = self.now() + delay;
                if scope == "provider:rest" || state.sync.state == SyncState::RateLimited {
                    scheduler
                        .account_cooldowns
                        .entry(job.account.id.clone())
                        .and_modify(|old| *old = (*old).max(deadline.into()))
                        .or_insert(deadline.into());
                } else {
                    scheduler
                        .strict_scope_deadlines
                        .entry(job.key.clone())
                        .and_modify(|old| *old = (*old).max(deadline))
                        .or_insert(deadline);
                }
                if scheduler.manual_keys.contains(&job.key) {
                    scheduler.deferred.push_back(job);
                } else {
                    scheduler.active.remove(&job.key);
                    scheduler.explicit_keys.remove(&job.key);
                }
                return true;
            }
        }
        let diagnostic_started = Instant::now();
        let result = match job.kind.clone() {
            JobKind::PullFileArtifact { request } => {
                self.sync_pull_file_artifact(&mut job, *request).await
            }
            JobKind::NotificationSubject { intent } => {
                self.sync_notification_subject(&intent, &mut job).await
            }
            JobKind::Feed(_) => self.sync_feed_page(&mut job).await,
            JobKind::Detail { subject_id, facet } => {
                self.sync_detail_page(&mut job, &subject_id, facet).await
            }
        };
        self.diagnostic_latency
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .record(diagnostic_started.elapsed());
        if let Err(error) = &result
            && !matches!(job.kind, JobKind::NotificationSubject { .. })
        {
            self.record_error(&job, error.clone()).await;
        }
        let _lifecycle = self.lifecycle.lock().await;
        let epoch_is_current = self
            .store
            .account(&job.account.id)
            .await
            .is_ok_and(|account| {
                account.state == AccountState::Active
                    && account.authorization_epoch == job.account.authorization_epoch
            });
        let mut scheduler = self.scheduler.lock().await;
        scheduler.demands.expire(self.now());
        job.pages += 1;
        let retry_selected = matches!(job.kind, JobKind::PullFileArtifact { .. })
            && epoch_is_current
            && job.pages < 5
            && result.as_ref().is_err_and(|error| {
                matches!(
                    error.code,
                    ErrorCode::Network | ErrorCode::Provider | ErrorCode::RateLimited
                )
            });
        if retry_selected {
            scheduler.deferred.push_back(job);
            return true;
        }
        let continue_page = result.is_ok_and(|more| more) && epoch_is_current;
        let interested = scheduler.demands.interested(&job);
        let explicit = scheduler.explicit_keys.contains(&job.key);
        let page_limit = match job.kind {
            JobKind::Detail {
                facet: DetailFacet::Commits,
                ..
            } => MAX_PULL_COMMIT_PAGES as usize,
            JobKind::Detail {
                facet: DetailFacet::Files,
                ..
            } => MAX_PULL_FILE_PROVIDER_PAGES as usize,
            _ => MAX_PAGES_PER_REFRESH,
        };
        if continue_page
            && job.pages < page_limit
            && (matches!(job.kind, JobKind::Feed(_)) || interested || explicit)
        {
            scheduler.requeue(job);
            return true;
        }
        if continue_page && job.pages >= page_limit {
            scheduler
                .due
                .insert(job.key.clone(), self.deadline_after(10));
        }
        scheduler.active.remove(&job.key);
        scheduler.explicit_keys.remove(&job.key);
        scheduler.manual_keys.remove(&job.key);
        scheduler.foreground_keys.remove(&job.key);
        if epoch_is_current {
            scheduler
                .due
                .entry(job.key)
                .or_insert_with(|| self.deadline_after(60));
        }
        true
    }

    async fn record_error(&self, job: &Job, error: CollaborationError) {
        if error.code == ErrorCode::StaleView {
            return;
        }
        if matches!(
            error.code,
            ErrorCode::Unsupported
                | ErrorCode::PermissionDenied
                | ErrorCode::NotFound
                | ErrorCode::Busy
        ) && let JobKind::Detail { subject_id, facet } = &job.kind
        {
            let _ = self
                .store
                .stop_detail_demand(
                    &job.account.id,
                    &job.account.authorization_epoch,
                    subject_id,
                    *facet,
                )
                .await;
        }
        let _lifecycle = self.lifecycle.lock().await;
        if !self
            .store
            .account(&job.account.id)
            .await
            .is_ok_and(|account| {
                account.state == AccountState::Active
                    && account.authorization_epoch == job.account.authorization_epoch
            })
        {
            return;
        }
        // Authentication revocation is a lifecycle operation. Serialize it with
        // connect/disconnect so a late 401 cannot replace a newly verified grant.
        if error.code == ErrorCode::AuthRequired {
            if self
                .store
                .account(&job.account.id)
                .await
                .is_ok_and(|account| {
                    account.state == AccountState::Active
                        && account.authorization_epoch == job.account.authorization_epoch
                })
            {
                if let Ok(revision) = self
                    .store
                    .set_sync_status(
                        &job.account.id,
                        &job.account.authorization_epoch,
                        // Authentication revocation is account-wide even if
                        // its original repository was deselected meanwhile.
                        "repositories",
                        SyncStatus {
                            state: SyncState::AuthRequired,
                            last_success_at: None,
                            next_retry_at: None,
                            error: Some(error),
                        },
                    )
                    .await
                {
                    self.publish(revision);
                }
                self.reset_account_scheduler(&job.account.id).await;
            }
            return;
        }
        let delay = if let Some(delay) = error.retry_after_seconds {
            delay as u64
        } else if matches!(error.code, ErrorCode::Network | ErrorCode::Provider) {
            let mut scheduler = self.scheduler.lock().await;
            let failures = scheduler.failures.entry(job.key.clone()).or_default();
            *failures = failures.saturating_add(1).min(5);
            // Read retries are delayed jobs, with bounded exponential backoff
            // and jitter. No worker sleeps while occupying the HTTP slot.
            let jitter = self.clock.jitter() % 15;
            (30 * (1u64 << *failures)).min(900) + jitter
        } else {
            180
        };
        let state = match error.code {
            ErrorCode::AuthRequired => SyncState::AuthRequired,
            ErrorCode::RateLimited => SyncState::RateLimited,
            ErrorCode::Network => SyncState::Offline,
            _ => SyncState::Error,
        };
        if let Ok(revision) = self
            .store
            .set_sync_status(
                &job.account.id,
                &job.account.authorization_epoch,
                &job.scope,
                SyncStatus {
                    state,
                    last_success_at: self
                        .store
                        .scope_state(&job.account.id, &job.scope)
                        .await
                        .ok()
                        .flatten()
                        .and_then(|scope| scope.sync.last_success_at),
                    next_retry_at: Some(self.future_string(delay)),
                    error: Some(error.clone()),
                },
            )
            .await
        {
            self.publish(revision);
        }
        if error.code == ErrorCode::RateLimited && !job.local_budget_refusal {
            let _ = self
                .persist_rate_limit(&job.account, delay, Some(error.clone()))
                .await;
        }
        {
            let mut scheduler = self.scheduler.lock().await;
            let deadline = self.deadline_after(delay);
            scheduler
                .strict_scope_deadlines
                .entry(job.key.clone())
                .and_modify(|old| *old = (*old).max(deadline))
                .or_insert(deadline);
            scheduler.due.insert(job.key.clone(), deadline);
            if error.code == ErrorCode::RateLimited {
                scheduler
                    .account_cooldowns
                    .entry(job.account.id.clone())
                    .and_modify(|old| *old = (*old).max(deadline.into()))
                    .or_insert(deadline.into());
            }
        }
    }

    async fn reset_account_scheduler(&self, account_id: &str) {
        let mut scheduler = self.scheduler.lock().await;
        scheduler.delivery_cursors.remove(account_id);
        let prefix = format!("{account_id}:");
        scheduler.queue.retain(|job| job.account.id != account_id);
        scheduler
            .deferred
            .retain(|job| job.account.id != account_id);
        scheduler
            .demands
            .leases
            .retain(|_, lease| lease.account.id != account_id);
        scheduler
            .demands
            .coverage_attempted
            .retain(|key| !key.starts_with(&prefix));
        scheduler
            .explicit_keys
            .retain(|key| !key.starts_with(&prefix));
        scheduler
            .manual_keys
            .retain(|key| !key.starts_with(&prefix));
        scheduler
            .foreground_keys
            .retain(|key| !key.starts_with(&prefix));
        scheduler.active.retain(|key, _| !key.starts_with(&prefix));
        scheduler.due.retain(|key, _| !key.starts_with(&prefix));
        scheduler
            .strict_scope_deadlines
            .retain(|key, _| !key.starts_with(&prefix));
        scheduler
            .failures
            .retain(|key, _| !key.starts_with(&prefix));
        // Provider budgets belong to the actor/account. Credential replacement
        // and disconnect cannot reset an exhausted rate-limit window.
    }

    fn publish(&self, revision: String) {
        let _ = self.changes.send(ChangeHint { revision });
    }

    async fn ensure_provider_budget(
        &self,
        account: &RemoteAccount,
        job: &mut Job,
    ) -> Result<(), CollaborationError> {
        let result = self.check_provider_budget(account).await;
        if result.is_err() {
            job.local_budget_refusal = true;
        }
        result
    }

    async fn check_provider_budget(
        &self,
        account: &RemoteAccount,
    ) -> Result<(), CollaborationError> {
        // Serialize the check with accepted durable writes and live installation.
        // The full persisted deadline is reread after every bounded live wake.
        let mut scheduler = self.scheduler.lock().await;
        let durable = self
            .store
            .scope_state(&account.id, "provider:rest")
            .await?
            .and_then(|scope| scope.sync.next_retry_at)
            .as_deref()
            .and_then(|time| self.provider_delay_until(time));
        // Direct admission can be the first request after a cold open, without
        // passing through scheduler enqueue. Preserve this observed remaining
        // wait monotonically before a later wall-clock jump can erase it.
        if let Some(delay) = durable {
            Self::install_provider_cooldown(
                &mut scheduler,
                &account.id,
                clock::AccountCooldown::after(self.now(), delay),
            );
        }
        let live = scheduler
            .account_cooldowns
            .get(&account.id)
            .and_then(|deadline| deadline.remaining(self.now()));
        let delay = live.into_iter().chain(durable).max();
        drop(scheduler);
        if let Some(delay) = delay {
            let mut error = CollaborationError::new(
                ErrorCode::RateLimited,
                "The provider account is waiting for its next permitted request",
            );
            error.retry_after_seconds = Some(
                delay
                    .as_secs()
                    .saturating_add(u64::from(delay.subsec_nanos() > 0))
                    .min(u32::MAX as u64) as u32,
            );
            // This is a local admission refusal, not another quota observation.
            return Err(error);
        }
        Ok(())
    }

    fn capture_provider_budget(&self, delay: u64) -> (String, clock::AccountCooldown) {
        let live = clock::AccountCooldown::after(self.now(), Duration::from_secs(delay));
        (self.future_string(delay), live)
    }

    fn install_provider_cooldown(
        scheduler: &mut Scheduler,
        account_id: &str,
        deadline: clock::AccountCooldown,
    ) {
        scheduler
            .account_cooldowns
            .entry(account_id.to_string())
            .and_modify(|old| *old = (*old).max(deadline))
            .or_insert(deadline);
    }

    async fn persist_rate_limit(
        &self,
        account: &RemoteAccount,
        delay: u64,
        error: Option<CollaborationError>,
    ) -> Result<(), CollaborationError> {
        let (proposed, deadline) = self.capture_provider_budget(delay);
        let revision = {
            let mut scheduler = self.scheduler.lock().await;
            let revision = self
                .store
                .merge_provider_budget(&account.id, &account.authorization_epoch, proposed, error)
                .await?;
            Self::install_provider_cooldown(&mut scheduler, &account.id, deadline);
            revision
        };
        self.publish(revision);
        Ok(())
    }

    /// Restart recovery and periodic cleanup use the same bounded durable queue.
    /// Staged replacements are abandoned, never promoted without a live probe.
    pub async fn recover_credentials(&self) -> Result<(), CollaborationError> {
        self.owned_operation(|runtime| async move { runtime.recover_credentials_inner().await })
            .await
    }
    async fn recover_credentials_inner(&self) -> Result<(), CollaborationError> {
        let _lifecycle = self.lifecycle.lock().await;
        self.cleanup_credentials_locked().await
    }

    async fn cleanup_credentials_locked(&self) -> Result<(), CollaborationError> {
        let pending = self
            .store
            .due_credential_cleanup(Utc::now().timestamp(), 8)
            .await?;
        let mut failed = false;
        for entry in pending {
            if let Err(error) = self.cleanup_credential_locked(entry).await {
                if error.code != ErrorCode::CredentialStoreUnavailable {
                    return Err(error);
                }
                failed = true;
            }
        }
        if failed { Err(vault_error()) } else { Ok(()) }
    }

    async fn cleanup_credential_locked(
        &self,
        entry: crate::credentials::CredentialCleanup,
    ) -> Result<(), CollaborationError> {
        credential_boundary!("before_delete");
        if self.delete_token(&entry.reference).await.is_ok() {
            credential_boundary!("after_delete");
            self.store
                .finish_credential_cleanup(&entry.reference)
                .await?;
            credential_boundary!("after_cleanup");
            Ok(())
        } else {
            // Keep evidence when the vault is locked. Retries never change the
            // committed account reference or shorten a previous authorization.
            let delay = (30_i64 * (1_i64 << entry.attempts.min(5))).min(900);
            self.store
                .defer_credential_cleanup(&entry.reference, Utc::now().timestamp() + delay)
                .await?;
            Err(vault_error())
        }
    }

    async fn load_token(&self, reference: &str) -> Result<Option<SecretToken>, CollaborationError> {
        let vault = self.vault.clone();
        let reference = reference.to_string();
        tokio::task::spawn_blocking(move || vault.load(&reference))
            .await
            .map_err(|_| vault_error())?
            .map_err(|_| vault_error())
    }

    async fn store_token(
        &self,
        reference: &str,
        token: &SecretToken,
    ) -> Result<(), CollaborationError> {
        let vault = self.vault.clone();
        let reference = reference.to_string();
        let token = token.clone();
        tokio::task::spawn_blocking(move || vault.store(&reference, &token))
            .await
            .map_err(|_| vault_error())?
            .map_err(|_| vault_error())
    }

    async fn delete_token(&self, reference: &str) -> Result<(), CollaborationError> {
        let vault = self.vault.clone();
        let reference = reference.to_string();
        tokio::task::spawn_blocking(move || vault.delete(&reference))
            .await
            .map_err(|_| vault_error())?
            .map_err(|_| vault_error())
    }
}

fn scope_name(kind: FeedKind, repository: Option<&RemoteRepository>) -> String {
    match kind {
        FeedKind::Repositories => "repositories".to_string(),
        FeedKind::Notifications => "notifications".to_string(),
        FeedKind::PullRequests => format!(
            "repo:{}:pull_request",
            repository.expect("repository feed").id
        ),
        FeedKind::Issues => format!("repo:{}:issue", repository.expect("repository feed").id),
    }
}

fn next_epoch(epoch: &str) -> Result<String, CollaborationError> {
    epoch
        .parse::<u64>()
        .ok()
        .and_then(|n| n.checked_add(1))
        .map(|n| n.to_string())
        .ok_or_else(CollaborationError::storage)
}
fn unsupported() -> CollaborationError {
    CollaborationError::new(
        ErrorCode::Unsupported,
        "This operation is not supported by the connected provider credential",
    )
}
fn stale() -> CollaborationError {
    CollaborationError::new(
        ErrorCode::StaleView,
        "This provider account authorization changed",
    )
}
fn vault_error() -> CollaborationError {
    CollaborationError::new(
        ErrorCode::CredentialStoreUnavailable,
        "The operating system credential store is unavailable",
    )
}

#[cfg(test)]
#[path = "runtime_credential_tests.rs"]
pub(crate) mod credential_crash_tests;
