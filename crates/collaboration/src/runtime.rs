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

const MAX_QUEUED_SCOPES: usize = 128;
const MAX_PAGES_PER_REFRESH: usize = 10;

macro_rules! credential_boundary {
    ($name:literal) => {
        #[cfg(test)]
        credential_crash_tests::checkpoint($name);
    };
}

#[derive(Clone)]
struct Job {
    key: String,
    account: RemoteAccount,
    repository: Option<RemoteRepository>,
    kind: FeedKind,
    scope: String,
}

#[derive(Default)]
struct Scheduler {
    queue: VecDeque<Job>,
    active: HashMap<String, String>,
    due: HashMap<String, Instant>,
    strict_scope_deadlines: HashMap<String, Instant>,
    account_cooldowns: HashMap<String, Instant>,
    failures: HashMap<String, u32>,
    background_cursor: usize,
}

#[derive(Clone)]
pub struct CollaborationRuntime {
    store: Arc<Store>,
    vault: Arc<dyn CredentialVault>,
    provider: Arc<dyn CollaborationProvider>,
    github_cli: Arc<GithubCli>,
    lifecycle: Arc<Mutex<()>>,
    scheduler: Arc<Mutex<Scheduler>>,
    notify: Arc<Notify>,
    started: Arc<AtomicBool>,
    changes: broadcast::Sender<ChangeHint>,
}

impl CollaborationRuntime {
    pub fn new(
        store: Arc<Store>,
        vault: Arc<dyn CredentialVault>,
        provider: Arc<dyn CollaborationProvider>,
    ) -> Self {
        let (changes, _) = broadcast::channel(64);
        Self {
            store,
            vault,
            provider,
            github_cli: Arc::new(GithubCli::disabled()),
            lifecycle: Arc::new(Mutex::new(())),
            scheduler: Arc::new(Mutex::new(Scheduler::default())),
            notify: Arc::new(Notify::new()),
            started: Arc::new(AtomicBool::new(false)),
            changes,
        }
    }

    pub fn with_github_cli(mut self, github_cli: GithubCli) -> Self {
        self.github_cli = Arc::new(github_cli);
        self
    }

    pub async fn discover_github_cli(&self) -> GithubCliDiscovery {
        self.github_cli.discover().await
    }

    pub async fn connect_github_cli(
        &self,
        candidate_id: &str,
    ) -> Result<RemoteAccount, CollaborationError> {
        let (expected_login, token) = self.github_cli.import(candidate_id).await?;
        self.connect_owned_token(token, Some(expected_login)).await
    }

    pub fn store(&self) -> &Arc<Store> {
        &self.store
    }

    pub fn subscribe(&self) -> broadcast::Receiver<ChangeHint> {
        self.changes.subscribe()
    }

    pub async fn save_draft(&self, draft: LocalDraft) -> Result<LocalDraft, CollaborationError> {
        let draft = self.store.save_draft(draft).await?;
        self.publish(self.store.revision().await?);
        Ok(draft)
    }

    /// Idempotent process-level startup. A weak owner permits normal application
    /// teardown; the background loop does not keep an otherwise dropped runtime.
    pub fn start_background(self: Arc<Self>) {
        if self.started.swap(true, Ordering::AcqRel) {
            return;
        }
        let owner = Arc::downgrade(&self);
        let notify = self.notify.clone();
        tokio::spawn(async move {
            let mut timer = tokio::time::interval(Duration::from_secs(10));
            timer.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            loop {
                tokio::select! { _ = timer.tick() => {}, _ = notify.notified() => {} }
                let Some(runtime) = owner.upgrade() else {
                    break;
                };
                // A damaged database does not terminate scheduling permanently;
                // a later tick can recover, and local calls report their errors.
                let _ = runtime.recover_credentials().await;
                let _ = runtime.enqueue_due().await;
                while runtime.run_next().await {}
            }
        });
    }

    pub async fn connect_github(&self, token: String) -> Result<RemoteAccount, CollaborationError> {
        let token = SecretToken::new(token).map_err(|_| {
            CollaborationError::invalid("Enter a valid GitHub personal access token")
        })?;
        self.connect_owned_token(token, None).await
    }

    async fn connect_owned_token(
        &self,
        token: SecretToken,
        expected_login: Option<String>,
    ) -> Result<RemoteAccount, CollaborationError> {
        // Once staging can begin, the runtime owns completion. Dropping the IPC
        // requester cannot race janitor deletion against a delayed vault write.
        let runtime = self.clone();
        tokio::spawn(async move {
            let result = runtime
                .connect_github_token(token, expected_login.as_deref())
                .await;
            // Release the owned storage/runtime before notifying the requester
            // that completion has finished (also relevant to reopen tests).
            drop(runtime);
            result
        })
        .await
        .map_err(|_| vault_error())?
    }

    async fn connect_github_token(
        &self,
        token: SecretToken,
        expected_login: Option<&str>,
    ) -> Result<RemoteAccount, CollaborationError> {
        let _lifecycle = self.lifecycle.lock().await;
        let _ = self.cleanup_credentials_locked().await;
        if self.provider.kind() != ProviderKind::Github {
            return Err(unsupported());
        }
        let verified = self.provider.probe(&token).await?;
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
                account.provider == ProviderKind::Github
                    && account.host == "github.com"
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
            provider: ProviderKind::Github,
            host: "github.com".to_string(),
            actor_id: verified.actor_id,
            login: verified.login,
            display_name: verified.display_name,
            authorization_epoch: epoch,
            state: AccountState::Active,
            notifications_supported: verified.notifications_supported,
        };
        credential_boundary!("before_cutover");
        let account = match self
            .store
            .commit_account_credential(account, &reference)
            .await
        {
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
                            next_retry_at: Some(future_string(60)),
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
                self.enqueue(account.clone(), Some(repository.clone()), kind, true)
                    .await?;
            }
        } else {
            let mut scheduler = self.scheduler.lock().await;
            let removed: Vec<String> = scheduler
                .queue
                .iter()
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
            for key in removed {
                scheduler.active.remove(&key);
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
                if account.notifications_supported {
                    first_job.get_or_insert(
                        self.enqueue(account.clone(), None, FeedKind::Notifications, true)
                            .await?,
                    );
                } else if request.kind.is_some() {
                    return Err(unsupported());
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
        if account.provider != self.provider.kind() {
            return Err(unsupported());
        }
        Ok(account)
    }

    async fn enqueue_due(&self) -> Result<(), CollaborationError> {
        let accounts: Vec<_> = self
            .store
            .accounts()
            .await?
            .accounts
            .into_iter()
            .filter(|account| {
                account.state == AccountState::Active && account.provider == self.provider.kind()
            })
            .collect();
        let start = self.scheduler.lock().await.background_cursor;
        let mut total = 0;
        // Iterate one account's local repository snapshot at a time. Two bounded
        // passes implement a rotating admission cursor without cloning every
        // account/repository combination before the 128-job limit is applied.
        for pass in 0..2 {
            total = 0;
            for account in &accounts {
                let repositories = self.store.repositories(&account.id).await?.repositories;
                let scopes = std::iter::once((None, FeedKind::Repositories))
                    .chain(
                        account
                            .notifications_supported
                            .then_some((None, FeedKind::Notifications)),
                    )
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
        let scope = scope_name(kind, repository.as_ref());
        let key = format!("{}:{}:{scope}", account.id, account.authorization_epoch);
        // Persisted cooldowns survive restarts and constrain manual refresh too.
        let persisted_state = self.store.scope_state(&account.id, &scope).await?;
        let persisted_deadline = persisted_state
            .as_ref()
            .and_then(|state| state.sync.next_retry_at.as_deref())
            .and_then(delay_until);
        let provider_deadline = self
            .store
            .scope_state(&account.id, "provider:rest")
            .await?
            .and_then(|state| state.sync.next_retry_at)
            .as_deref()
            .and_then(delay_until);
        let mut scheduler = self.scheduler.lock().await;
        if let Some(delay) = provider_deadline {
            scheduler
                .account_cooldowns
                .insert(account.id.clone(), Instant::now() + delay);
        }
        if let Some(delay) = persisted_deadline {
            let deadline = Instant::now() + delay;
            scheduler
                .strict_scope_deadlines
                .insert(key.clone(), deadline);
            if persisted_state
                .as_ref()
                .is_some_and(|state| state.sync.state == SyncState::RateLimited)
            {
                scheduler
                    .account_cooldowns
                    .entry(account.id.clone())
                    .and_modify(|existing| *existing = (*existing).max(deadline))
                    .or_insert(deadline);
            }
        }
        if let Some(id) = scheduler.active.get(&key).cloned() {
            if manual
                && let Some(index) = scheduler.queue.iter().position(|job| job.key == key)
                && let Some(job) = scheduler.queue.remove(index)
            {
                scheduler.queue.push_front(job);
            }
            return Ok(id);
        }
        if !manual
            && scheduler
                .due
                .get(&key)
                .is_some_and(|time| *time > Instant::now())
        {
            return Ok(String::new());
        }
        if scheduler.queue.len() >= MAX_QUEUED_SCOPES {
            return Err(CollaborationError::new(
                ErrorCode::Busy,
                "The collaboration refresh queue is full",
            ));
        }
        if manual {
            scheduler.failures.remove(&key);
        }
        let id = uuid::Uuid::new_v4().to_string();
        scheduler.active.insert(key.clone(), id.clone());
        let job = Job {
            key,
            account,
            repository,
            kind,
            scope,
        };
        if manual {
            scheduler.queue.push_front(job);
        } else {
            scheduler.queue.push_back(job);
        }
        Ok(id)
    }

    async fn run_next(&self) -> bool {
        let job = {
            let mut scheduler = self.scheduler.lock().await;
            let now = Instant::now();
            let index = scheduler.queue.iter().position(|job| {
                scheduler
                    .strict_scope_deadlines
                    .get(&job.key)
                    .is_none_or(|time| *time <= now)
                    && scheduler
                        .account_cooldowns
                        .get(&job.account.id)
                        .is_none_or(|time| *time <= now)
            });
            index.and_then(|index| scheduler.queue.remove(index))
        };
        let Some(job) = job else {
            return false;
        };
        if let Ok(Some(state)) = self
            .store
            .scope_state(&job.account.id, "provider:rest")
            .await
            && let Some(delay) = state.sync.next_retry_at.as_deref().and_then(delay_until)
        {
            let mut scheduler = self.scheduler.lock().await;
            scheduler
                .account_cooldowns
                .insert(job.account.id.clone(), Instant::now() + delay);
            scheduler.queue.push_back(job);
            return true;
        }
        // Instant deadlines are deliberately bounded segments. Recheck the
        // persisted absolute deadline before dispatch, so a very long provider
        // wait is deferred repeatedly rather than shortened by a local cap.
        if let Ok(Some(state)) = self.store.scope_state(&job.account.id, &job.scope).await
            && let Some(delay) = state.sync.next_retry_at.as_deref().and_then(delay_until)
        {
            let mut scheduler = self.scheduler.lock().await;
            let deadline = Instant::now() + delay;
            scheduler
                .strict_scope_deadlines
                .insert(job.key.clone(), deadline);
            if state.sync.state == SyncState::RateLimited {
                scheduler
                    .account_cooldowns
                    .insert(job.account.id.clone(), deadline);
            }
            scheduler.queue.push_back(job);
            return true;
        }
        let result = self.sync_job(&job).await;
        if let Err(error) = result {
            self.record_error(&job, error).await;
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
        scheduler.active.remove(&job.key);
        if epoch_is_current {
            scheduler
                .due
                .entry(job.key)
                .or_insert_with(|| Instant::now() + Duration::from_secs(60));
        }
        true
    }

    async fn sync_job(&self, job: &Job) -> Result<(), CollaborationError> {
        let (account, token) = {
            let _lifecycle = self.lifecycle.lock().await;
            let account = self.active_account(&job.account.id).await?;
            if account.authorization_epoch != job.account.authorization_epoch {
                return Err(stale());
            }
            let reference = self
                .store
                .credential_reference(&account.id)
                .await?
                .ok_or_else(|| {
                    CollaborationError::new(
                        ErrorCode::AuthRequired,
                        "Reconnect this provider account",
                    )
                })?;
            let token = self.load_token(&reference).await?.ok_or_else(|| {
                CollaborationError::new(ErrorCode::AuthRequired, "Reconnect this provider account")
            })?;
            (account, token)
        };
        let previous = self.store.scope_state(&account.id, &job.scope).await?;
        let resumed = previous.as_ref().filter(|scope| {
            scope.coverage.state == CoverageState::Partial && scope.next_cursor.is_some()
        });
        let run_id = if let Some(scope) = resumed {
            let revision = self
                .store
                .set_sync_status(
                    &account.id,
                    &account.authorization_epoch,
                    &job.scope,
                    SyncStatus {
                        state: SyncState::Syncing,
                        last_success_at: scope.sync.last_success_at.clone(),
                        next_retry_at: None,
                        error: None,
                    },
                )
                .await?;
            self.publish(revision);
            scope.run_id.clone()
        } else {
            let run_id = self
                .store
                .begin_sync(&account.id, &account.authorization_epoch, &job.scope)
                .await?;
            self.publish(self.store.revision().await?);
            run_id
        };
        // A bounded bootstrap continues its committed next-link on the next
        // refresh. After reaching its end, the next reconciliation starts at
        // page one again; absence is never interpreted as deletion.
        let mut cursor = previous
            .as_ref()
            .filter(|scope| scope.coverage.state == CoverageState::Partial)
            .and_then(|scope| scope.next_cursor.clone());
        let starts_at_beginning = cursor.is_none();
        let conditional = previous.as_ref().filter(|scope| {
            scope.coverage.state == CoverageState::Complete
                && scope.next_cursor.is_none()
                && (scope.etag.is_some() || scope.last_modified.is_some())
        });
        let mut validator = conditional.and_then(|scope| scope.etag.clone());
        let mut modified = conditional.and_then(|scope| scope.last_modified.clone());
        for page_index in 0..MAX_PAGES_PER_REFRESH {
            let current = self.active_account(&account.id).await?;
            if current.authorization_epoch != account.authorization_epoch {
                return Err(stale());
            }
            let page = self
                .provider
                .fetch_page(
                    &token,
                    FeedRequest {
                        account: account.clone(),
                        repository: job.repository.clone(),
                        kind: job.kind,
                        cursor: cursor.clone(),
                        etag: validator.take(),
                        last_modified: modified.take(),
                    },
                )
                .await?;
            let not_modified = page.not_modified;
            if not_modified && (page_index != 0 || conditional.is_none()) {
                return Err(CollaborationError::new(
                    ErrorCode::Provider,
                    "The provider returned an unexpected conditional response",
                ));
            }
            let complete = not_modified || page.next_cursor.is_none();
            cursor = page.next_cursor.clone();
            // Only a complete single-page feed gets a feed validator. A page-one
            // 304 cannot prove that page two or older items remain unchanged.
            let retain_validator = starts_at_beginning && page_index == 0 && complete;
            let revision = self
                .store
                .apply_page(PageCommit {
                    account_id: account.id.clone(),
                    authorization_epoch: account.authorization_epoch.clone(),
                    scope: job.scope.clone(),
                    run_id: run_id.clone(),
                    repositories: page.repositories,
                    items: page.items,
                    next_cursor: cursor.clone(),
                    etag: if retain_validator {
                        page.etag.or_else(|| {
                            conditional
                                .filter(|_| not_modified)
                                .and_then(|scope| scope.etag.clone())
                        })
                    } else {
                        None
                    },
                    last_modified: if retain_validator {
                        page.last_modified.or_else(|| {
                            conditional
                                .filter(|_| not_modified)
                                .and_then(|scope| scope.last_modified.clone())
                        })
                    } else {
                        None
                    },
                    not_modified,
                    complete,
                    observed_at: now_string(),
                })
                .await?;
            self.publish(revision);
            let _lifecycle = self.lifecycle.lock().await;
            if !self.store.account(&account.id).await.is_ok_and(|current| {
                current.state == AccountState::Active
                    && current.authorization_epoch == account.authorization_epoch
            }) {
                return Err(stale());
            }
            let normal_interval = match job.kind {
                FeedKind::Repositories => 600,
                FeedKind::Notifications => 60,
                FeedKind::PullRequests | FeedKind::Issues => 180,
            };
            let poll_interval = page.poll_interval_seconds.unwrap_or(0);
            let cooldown = page.cooldown_seconds.unwrap_or(0);
            {
                let mut scheduler = self.scheduler.lock().await;
                scheduler.due.insert(
                    job.key.clone(),
                    deadline_after(normal_interval.max(poll_interval).max(cooldown)),
                );
                scheduler.failures.remove(&job.key);
                if poll_interval > 0 {
                    scheduler
                        .strict_scope_deadlines
                        .insert(job.key.clone(), deadline_after(poll_interval));
                }
                if cooldown > 0 {
                    scheduler
                        .account_cooldowns
                        .insert(account.id.clone(), deadline_after(cooldown));
                }
            }
            if complete || cooldown > 0 || page_index + 1 == MAX_PAGES_PER_REFRESH {
                let strict_seconds = poll_interval.max(cooldown);
                let revision = self
                    .store
                    .set_sync_status(
                        &account.id,
                        &account.authorization_epoch,
                        &job.scope,
                        SyncStatus {
                            state: if cooldown > 0 {
                                SyncState::RateLimited
                            } else {
                                SyncState::Idle
                            },
                            last_success_at: Some(now_string()),
                            next_retry_at: (strict_seconds > 0)
                                .then(|| future_string(strict_seconds)),
                            error: None,
                        },
                    )
                    .await?;
                self.publish(revision);
                if cooldown > 0 {
                    self.persist_rate_limit(&account, cooldown, None).await?;
                }
                return Ok(());
            }
        }
        Ok(())
    }

    async fn record_error(&self, job: &Job, error: CollaborationError) {
        if error.code == ErrorCode::StaleView {
            return;
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
            let jitter = u64::from(uuid::Uuid::new_v4().as_bytes()[0]) % 15;
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
                    next_retry_at: Some(future_string(delay)),
                    error: Some(error.clone()),
                },
            )
            .await
        {
            self.publish(revision);
        }
        if error.code == ErrorCode::RateLimited {
            let _ = self
                .persist_rate_limit(&job.account, delay, Some(error.clone()))
                .await;
        }
        {
            let mut scheduler = self.scheduler.lock().await;
            let deadline = deadline_after(delay);
            scheduler
                .strict_scope_deadlines
                .insert(job.key.clone(), deadline);
            scheduler.due.insert(job.key.clone(), deadline);
            if error.code == ErrorCode::RateLimited {
                scheduler
                    .account_cooldowns
                    .insert(job.account.id.clone(), deadline);
            }
        }
    }

    async fn reset_account_scheduler(&self, account_id: &str) {
        let mut scheduler = self.scheduler.lock().await;
        let prefix = format!("{account_id}:");
        scheduler.queue.retain(|job| job.account.id != account_id);
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

    async fn persist_rate_limit(
        &self,
        account: &RemoteAccount,
        delay: u64,
        error: Option<CollaborationError>,
    ) -> Result<(), CollaborationError> {
        let revision = self
            .store
            .set_sync_status(
                &account.id,
                &account.authorization_epoch,
                "provider:rest",
                SyncStatus {
                    state: SyncState::RateLimited,
                    last_success_at: None,
                    next_retry_at: Some(future_string(delay)),
                    error,
                },
            )
            .await?;
        self.publish(revision);
        Ok(())
    }

    /// Restart recovery and periodic cleanup use the same bounded durable queue.
    /// Staged replacements are abandoned, never promoted without a live probe.
    pub async fn recover_credentials(&self) -> Result<(), CollaborationError> {
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
fn now_string() -> String {
    Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}
fn future_string(seconds: u64) -> String {
    Utc::now()
        .checked_add_signed(chrono::Duration::seconds(
            seconds.min(i64::MAX as u64 / 1000) as i64,
        ))
        .unwrap_or(DateTime::<Utc>::MAX_UTC)
        .to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}
fn delay_until(time: &str) -> Option<Duration> {
    let delay = DateTime::parse_from_rfc3339(time)
        .ok()?
        .signed_duration_since(Utc::now())
        .num_milliseconds();
    (delay > 0).then(|| Duration::from_millis((delay as u64).min(86400000)))
}
fn deadline_after(seconds: u64) -> Instant {
    Instant::now() + Duration::from_secs(seconds.min(86400))
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
