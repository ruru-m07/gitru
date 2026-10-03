//! Fair committed-page arbitration and bounded ready admission.
use super::*;
use std::collections::{BTreeSet, HashSet};

const MAX_RECONCILE: usize = 96;
const MAX_INTERACTIVE: usize = 32;
const MAX_MANUAL: usize = 128;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Admission {
    Manual,
    Explicit,
    Foreground,
    Reconcile,
}

impl Scheduler {
    fn interactive(&self, job: &Job) -> bool {
        matches!(job.reason, Admission::Manual | Admission::Explicit)
            || self.explicit_keys.contains(&job.key)
            || (self.foreground_keys.contains(&job.key) && self.demands.interested(job))
    }
    // Account-wide interest does not promote every pre-existing backfill job.
    // A bounded foreground reservation is claimed only on actual admission.
    fn foreground_space(&mut self, kind: &JobKind, adding: bool) -> bool {
        let room = |scheduler: &Self| {
            (!adding || scheduler.queue.len() < MAX_QUEUED_SCOPES)
                && scheduler
                    .queue
                    .iter()
                    .filter(|job| scheduler.interactive(job))
                    .count()
                    < MAX_INTERACTIVE
        };
        if room(self) {
            return true;
        }
        if !matches!(
            kind,
            JobKind::Detail { .. } | JobKind::NotificationSubject { .. }
        ) {
            return false;
        }
        let demote = self
            .queue
            .iter()
            .find(|job| {
                matches!(job.kind, JobKind::Feed(_))
                    && self.foreground_keys.contains(&job.key)
                    && !self.explicit_keys.contains(&job.key)
            })
            .map(|job| job.key.clone());
        if let Some(key) = demote {
            self.foreground_keys.remove(&key);
            // Preserve its origin: a true backfill remains reconciliation,
            // while a foreground-only read still stops when its lease expires.
            // Either can regain priority on a later admission rotation.
        }
        if adding
            && self.queue.len() >= MAX_QUEUED_SCOPES
            && let Some(index) = self.queue.iter().rposition(|job| {
                matches!(job.kind, JobKind::Feed(_))
                    && !self.interactive(job)
                    && !self.explicit_keys.contains(&job.key)
            })
        {
            let job = self.queue.remove(index).expect("observed queue index");
            self.active.remove(&job.key);
            self.foreground_keys.remove(&job.key);
        }
        room(self)
    }
    pub(super) fn requeue(&mut self, job: Job) {
        let interactive = self.interactive(&job);
        if self.ready_space(interactive) {
            self.queue.push_back(job);
        } else if self.manual_keys.contains(&job.key) {
            self.deferred.push_back(job);
        } else {
            // Continuation/explicit intent is already durable; this releases
            // ready capacity without discarding its committed checkpoint.
            self.active.remove(&job.key);
            self.explicit_keys.remove(&job.key);
            self.foreground_keys.remove(&job.key);
        }
    }
    fn blocked(&self, job: &Job, now: Instant) -> bool {
        self.strict_scope_deadlines
            .get(&job.key)
            .is_some_and(|deadline| *deadline > now)
            || self
                .account_cooldowns
                .get(&job.account.id)
                .is_some_and(|deadline| *deadline > now)
    }
    fn ready_space(&self, interactive: bool) -> bool {
        self.queue.len() < MAX_QUEUED_SCOPES
            && self
                .queue
                .iter()
                .filter(|job| self.interactive(job) == interactive)
                .count()
                < if interactive {
                    MAX_INTERACTIVE
                } else {
                    MAX_RECONCILE
                }
    }
    pub(super) fn pick(&mut self, now: Instant) -> Option<Job> {
        self.demands.expire(now);
        let removed: HashSet<String> = self
            .queue
            .iter()
            .filter(|job| {
                job.reason == Admission::Foreground
                    && !self.demands.interested(job)
                    && !self.explicit_keys.contains(&job.key)
            })
            .map(|job| job.key.clone())
            .collect();
        self.queue.retain(|job| !removed.contains(&job.key));
        self.active.retain(|key, _| !removed.contains(key));
        self.foreground_keys.retain(|key| !removed.contains(key));
        let expired: Vec<_> = self
            .queue
            .iter()
            .filter(|job| self.foreground_keys.contains(&job.key) && !self.demands.interested(job))
            .map(|job| job.key.clone())
            .collect();
        for key in expired {
            self.foreground_keys.remove(&key);
        }
        // Blocked automatic intent remains in the lease or durable read-intent
        // table. It must not occupy another account's ready reservation. Manual
        // receipts retain their bounded in-memory job until its barrier opens.
        for _ in 0..self.queue.len() {
            let job = self.queue.pop_front().expect("bounded queue length");
            if self.blocked(&job, now) {
                if self.manual_keys.contains(&job.key) {
                    self.deferred.push_back(job);
                } else {
                    self.active.remove(&job.key);
                    self.explicit_keys.remove(&job.key);
                    self.foreground_keys.remove(&job.key);
                }
            } else {
                self.queue.push_back(job);
            }
        }
        for _ in 0..self.deferred.len() {
            let job = self.deferred.pop_front().expect("bounded deferred length");
            if !self.blocked(&job, now) && self.ready_space(true) {
                self.queue.push_back(job);
            } else {
                self.deferred.push_back(job);
            }
        }
        let eligible = |job: &Job| !self.blocked(job, now);
        let has_interactive = self
            .queue
            .iter()
            .any(|job| eligible(job) && self.interactive(job));
        let has_background = self
            .queue
            .iter()
            .any(|job| eligible(job) && !self.interactive(job));
        let interactive = has_interactive && (!has_background || self.interactive_turns < 3);
        let has_detail = interactive
            && self.queue.iter().any(|job| {
                eligible(job)
                    && self.interactive(job)
                    && matches!(
                        job.kind,
                        JobKind::Detail { .. } | JobKind::NotificationSubject { .. }
                    )
            });
        let has_index = interactive
            && self.queue.iter().any(|job| {
                eligible(job) && self.interactive(job) && matches!(job.kind, JobKind::Feed(_))
            });
        let detail = has_detail && (!has_index || self.detail_turns < 2);
        let selected = |job: &Job| {
            eligible(job)
                && self.interactive(job) == interactive
                && (!interactive
                    || matches!(
                        job.kind,
                        JobKind::Detail { .. } | JobKind::NotificationSubject { .. }
                    ) == detail)
        };
        let accounts: BTreeSet<_> = self
            .queue
            .iter()
            .filter(|job| selected(job))
            .map(|job| job.account.id.clone())
            .collect();
        let last = if interactive {
            &self.interactive_account
        } else {
            &self.reconciliation_account
        };
        let account = accounts
            .iter()
            .find(|id| last.as_ref().is_none_or(|last| *id > last))
            .or_else(|| accounts.first())?
            .clone();
        let index = self
            .queue
            .iter()
            .position(|job| job.account.id == account && selected(job))?;
        if interactive {
            self.detail_turns = if detail {
                self.detail_turns.saturating_add(1).min(2)
            } else {
                0
            };
            self.interactive_turns = self.interactive_turns.saturating_add(1).min(3);
            self.interactive_account = Some(account);
        } else {
            self.interactive_turns = 0;
            self.reconciliation_account = Some(account);
        }
        self.queue.remove(index)
    }
}

impl CollaborationRuntime {
    pub(super) async fn enqueue_work_reason(
        &self,
        account: RemoteAccount,
        repository: Option<RemoteRepository>,
        kind: JobKind,
        scope: String,
        reason: Admission,
    ) -> Result<String, CollaborationError> {
        let key = format!("{}:{}:{scope}", account.id, account.authorization_epoch);
        let state = self.store.scope_state(&account.id, &scope).await?;
        let strict = state
            .as_ref()
            .and_then(|state| state.sync.next_retry_at.as_deref())
            .and_then(|time| self.delay_until(time));
        let provider = self
            .store
            .scope_state(&account.id, "provider:rest")
            .await?
            .and_then(|state| state.sync.next_retry_at)
            .as_deref()
            .and_then(|time| self.delay_until(time));
        let due = self
            .work_due(&account, &kind, &scope, reason, state.as_ref())
            .await?;
        let mut scheduler = self.scheduler.lock().await;
        scheduler.demands.expire(self.now());
        if let Some(delay) = provider {
            let deadline = self.now() + delay;
            scheduler
                .account_cooldowns
                .entry(account.id.clone())
                .and_modify(|old| *old = (*old).max(deadline))
                .or_insert(deadline);
        }
        if let Some(delay) = strict {
            let deadline = self.now() + delay;
            scheduler
                .strict_scope_deadlines
                .entry(key.clone())
                .and_modify(|old| *old = (*old).max(deadline))
                .or_insert(deadline);
            if state
                .as_ref()
                .is_some_and(|state| state.sync.state == SyncState::RateLimited)
            {
                scheduler
                    .account_cooldowns
                    .entry(account.id.clone())
                    .and_modify(|old| *old = (*old).max(deadline))
                    .or_insert(deadline);
            }
        }
        if reason == Admission::Manual
            && !scheduler.manual_keys.contains(&key)
            && scheduler.manual_keys.len() >= MAX_MANUAL
        {
            return Err(CollaborationError::new(
                ErrorCode::Busy,
                "The collaboration refresh queue is full",
            ));
        }
        if let Some(id) = scheduler.active.get(&key).cloned() {
            if reason == Admission::Foreground
                && !scheduler.explicit_keys.contains(&key)
                && !scheduler.foreground_keys.contains(&key)
                && !scheduler.foreground_space(&kind, false)
            {
                return Err(CollaborationError::new(
                    ErrorCode::Busy,
                    "The collaboration foreground queue is full",
                ));
            }
            if matches!(reason, Admission::Manual | Admission::Explicit)
                && !scheduler.explicit_keys.contains(&key)
                && !scheduler.foreground_space(&kind, false)
            {
                return Err(CollaborationError::new(
                    ErrorCode::Busy,
                    "The collaboration foreground queue is full",
                ));
            }
            if matches!(reason, Admission::Manual | Admission::Explicit) {
                scheduler.explicit_keys.insert(key.clone());
            }
            if reason == Admission::Manual {
                scheduler.manual_keys.insert(key.clone());
            }
            if reason == Admission::Foreground {
                scheduler.foreground_keys.insert(key.clone());
                scheduler.demands.coverage_attempted.insert(key);
            }
            return Ok(id);
        }
        if reason != Admission::Manual && due > self.now() {
            return Ok(String::new());
        }
        let interactive = reason != Admission::Reconcile;
        let blocked = scheduler
            .strict_scope_deadlines
            .get(&key)
            .is_some_and(|deadline| *deadline > self.now())
            || scheduler
                .account_cooldowns
                .get(&account.id)
                .is_some_and(|deadline| *deadline > self.now());
        if blocked && reason != Admission::Manual {
            return Ok(String::new());
        }
        let space = blocked
            || if interactive {
                scheduler.foreground_space(&kind, true)
            } else {
                scheduler.ready_space(false)
            };
        if !blocked && !space {
            return Err(CollaborationError::new(
                ErrorCode::Busy,
                "The collaboration refresh queue is full",
            ));
        }
        if reason == Admission::Manual {
            scheduler.failures.remove(&key);
        }
        let id = uuid::Uuid::new_v4().to_string();
        scheduler.active.insert(key.clone(), id.clone());
        if matches!(reason, Admission::Manual | Admission::Explicit) {
            scheduler.explicit_keys.insert(key.clone());
        }
        if reason == Admission::Manual {
            scheduler.manual_keys.insert(key.clone());
        }
        if reason == Admission::Foreground {
            scheduler.foreground_keys.insert(key.clone());
            scheduler.demands.coverage_attempted.insert(key.clone());
        }
        let job = Job {
            key,
            account,
            repository,
            kind,
            scope,
            reason,
            pages: 0,
            detail_lease: None,
        };
        if blocked {
            scheduler.deferred.push_back(job);
        } else {
            scheduler.queue.push_back(job);
        }
        Ok(id)
    }

    async fn work_due(
        &self,
        account: &RemoteAccount,
        kind: &JobKind,
        scope: &str,
        reason: Admission,
        state: Option<&StoredScope>,
    ) -> Result<Instant, CollaborationError> {
        if reason == Admission::Manual {
            return Ok(self.now());
        }
        let key = format!("{}:{}:{scope}", account.id, account.authorization_epoch);
        let cached = self.scheduler.lock().await.due.get(&key).copied();
        if state.is_some_and(|state| {
            state.coverage.state == CoverageState::Partial && state.next_cursor.is_some()
        }) {
            return Ok(cached.unwrap_or_else(|| self.now()));
        }
        let seconds = match kind {
            JobKind::NotificationSubject { .. } => return Ok(self.now()),
            JobKind::Feed(FeedKind::Repositories) => {
                if reason == Admission::Foreground {
                    120
                } else {
                    600
                }
            }
            JobKind::Feed(FeedKind::Notifications) => 60,
            JobKind::Feed(_) => {
                if reason == Admission::Foreground {
                    60
                } else {
                    180
                }
            }
            JobKind::Detail { subject_id, facet } => {
                let (evidence, metadata, deadlines) = self
                    .store
                    .demand_detail_state(&account.id, subject_id, *facet)
                    .await?;
                if evidence.availability == DetailAvailability::Unavailable {
                    return Err(CollaborationError::new(
                        ErrorCode::PermissionDenied,
                        "Detail demand is inaccessible",
                    ));
                }
                if reason == Admission::Foreground
                    && *facet == DetailFacet::Body
                    && !metadata
                    && !self
                        .scheduler
                        .lock()
                        .await
                        .demands
                        .coverage_attempted
                        .contains(&key)
                {
                    return Ok(self.now());
                }
                let mut clocks = deadlines;
                if evidence.observed_state == DetailValueState::Known
                    && let Some(time) = evidence.stale_at
                {
                    clocks.push(time);
                }
                if let Some(delay) = clocks
                    .iter()
                    .filter_map(|time| {
                        DateTime::parse_from_rfc3339(time).ok().map(|time| {
                            time.signed_duration_since(self.clock.utc())
                                .num_milliseconds()
                                .max(0) as u64
                        })
                    })
                    .min()
                {
                    return Ok(self.now() + Duration::from_millis(delay.min(86400000)));
                }
                if *facet == DetailFacet::Body { 180 } else { 60 }
            }
        };
        let persisted = state
            .and_then(|state| state.sync.last_success_at.as_deref())
            .and_then(|time| DateTime::parse_from_rfc3339(time).ok())
            .and_then(|time| time.checked_add_signed(chrono::Duration::seconds(seconds)))
            .and_then(|time| self.delay_until(&time.to_rfc3339()))
            .map(|delay| self.now() + delay)
            .unwrap_or_else(|| self.now());
        Ok(if reason == Admission::Foreground {
            persisted
        } else {
            cached.unwrap_or(persisted)
        })
    }
}
