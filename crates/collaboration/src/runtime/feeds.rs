//! One committed feed page per scheduler turn.
use super::*;

impl CollaborationRuntime {
    pub(super) async fn sync_feed_page(&self, job: &mut Job) -> Result<bool, CollaborationError> {
        self.ensure_demand_dispatch(job).await?;
        let kind = match &job.kind {
            JobKind::NotificationSubject { .. } => return Err(unsupported()),
            JobKind::Detail { subject_id, facet } => {
                let _ = (subject_id, facet);
                return Err(unsupported());
            }
            JobKind::Feed(kind) => *kind,
        };
        let (account, token) = {
            let _lifecycle = self.lifecycle.lock().await;
            let account = self.active_account(&job.account.id).await?;
            if account.authorization_epoch != job.account.authorization_epoch {
                return Err(stale());
            }
            self.require_feed(&account, kind).await?;
            if let Some(repository) = &job.repository {
                let current = self.store.repository(&account.id, &repository.id).await?;
                if !current.selected {
                    return Err(stale());
                }
                job.repository = Some(current);
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
            self.ensure_provider_budget(&account, job).await?;
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
        let page_index = 0;
        {
            let current = self.active_account(&account.id).await?;
            if current.authorization_epoch != account.authorization_epoch {
                return Err(stale());
            }
            self.require_feed(&current, kind).await?;
            self.ensure_demand_dispatch(job).await?;
            // A probe may publish a new account budget while this job waits for
            // lifecycle/vault/native reads after the scheduler picked it.
            let adapter = self.adapter_for_account(&current).await?;
            self.ensure_provider_budget(&current, job).await?;
            let fetched = adapter
                .fetch_page(
                    &token,
                    FeedRequest {
                        account: account.clone(),
                        repository: job.repository.clone(),
                        kind,
                        cursor: cursor.clone(),
                        etag: validator.take(),
                        last_modified: modified.take(),
                    },
                )
                .await;
            // A bounded/invalid response can still consume the account quota.
            // Preserve that same-epoch observation before converting its safe
            // error; no rejected page data or old-epoch grant can be published.
            let page = match fetched {
                Ok(page) => page,
                Err(error) => {
                    if let Some(seconds) = error
                        .account_cooldown_seconds
                        .filter(|seconds| *seconds > 0)
                    {
                        self.persist_rate_limit(&account, seconds, None).await?;
                    }
                    return Err(error.into());
                }
            };
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
                .apply_page_with_notification_subjects(
                    PageCommit {
                        account_id: account.id.clone(),
                        authorization_epoch: account.authorization_epoch.clone(),
                        scope: job.scope.clone(),
                        run_id: run_id.clone(),
                        repositories: page.repositories,
                        items: page.items,
                        endpoint_aliases: page.endpoint_aliases,
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
                        observed_at: self.now_string(),
                    },
                    page.notification_subjects,
                )
                .await?;
            self.publish(revision);
            let _lifecycle = self.lifecycle.lock().await;
            if !self.store.account(&account.id).await.is_ok_and(|current| {
                current.state == AccountState::Active
                    && current.authorization_epoch == account.authorization_epoch
            }) {
                return Err(stale());
            }
            let normal_interval = match kind {
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
                    self.deadline_after(if complete {
                        normal_interval.max(poll_interval).max(cooldown)
                    } else {
                        poll_interval.max(cooldown)
                    }),
                );
                scheduler.failures.remove(&job.key);
                if poll_interval > 0 {
                    scheduler
                        .strict_scope_deadlines
                        .entry(job.key.clone())
                        .and_modify(|old| *old = (*old).max(self.deadline_after(poll_interval)))
                        .or_insert_with(|| self.deadline_after(poll_interval));
                }
                if cooldown > 0 {
                    scheduler
                        .account_cooldowns
                        .entry(account.id.clone())
                        .and_modify(|old| *old = (*old).max(self.deadline_after(cooldown)))
                        .or_insert_with(|| self.deadline_after(cooldown));
                }
            }
            {
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
                            last_success_at: Some(self.now_string()),
                            next_retry_at: (strict_seconds > 0)
                                .then(|| self.future_string(strict_seconds)),
                            error: None,
                        },
                    )
                    .await?;
                self.publish(revision);
                if cooldown > 0 {
                    self.persist_rate_limit(&account, cooldown, None).await?;
                }
                Ok(!complete && cooldown == 0)
            }
        }
    }
}
