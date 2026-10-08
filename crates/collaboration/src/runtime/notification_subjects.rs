//! Explicit finite identity discovery shares the single HTTP worker and budgets.
use super::*;

impl CollaborationRuntime {
    pub async fn notification_subject(
        &self,
        query: NotificationSubjectQuery,
    ) -> Result<NotificationSubjectSnapshot, CollaborationError> {
        self.store
            .notification_subject(query, |account, instance, kind| {
                let Ok(adapter) = self.registry.adapter(instance) else {
                    return CapabilityState::Unavailable;
                };
                let inbox = adapter.profile(account).facet(ResourceFacet::Inbox).state;
                if inbox != CapabilityState::Supported {
                    return inbox;
                }
                kind.map(|kind| adapter.notification_subject_support(account, kind))
                    .unwrap_or(CapabilityState::Unsupported)
            })
            .await
    }

    pub async fn discover_notification_subject(
        &self,
        request: DiscoverNotificationSubjectRequest,
    ) -> Result<RefreshReceipt, CollaborationError> {
        self.discover_notification_subject_checked(request, || Ok(()))
            .await
    }

    pub async fn discover_notification_subject_checked<F>(
        &self,
        request: DiscoverNotificationSubjectRequest,
        guard: F,
    ) -> Result<RefreshReceipt, CollaborationError>
    where
        F: Fn() -> Result<(), CollaborationError>,
    {
        let query = NotificationSubjectQuery {
            account_id: request.account_id.clone(),
            authorization_epoch: request.authorization_epoch.clone(),
            notification_id: request.notification_id.clone(),
        };
        let snapshot = self.notification_subject(query).await?;
        if snapshot.selector_generation.as_ref() != Some(&request.selector_generation) {
            return Err(stale());
        }
        if !snapshot.discovery.admission {
            return Err(CollaborationError::new(
                if snapshot.discovery.support == CapabilityState::Unsupported {
                    ErrorCode::Unsupported
                } else {
                    ErrorCode::PermissionDenied
                },
                "Notification discovery is not available",
            ));
        }
        let job_id = self
            .store
            .request_notification_subject_checked(&request, guard)
            .await?;
        self.publish(self.store.revision().await?);
        self.enqueue_pending_notification_subjects().await?;
        self.notify.notify_one();
        Ok(RefreshReceipt { job_id })
    }

    pub(super) async fn enqueue_pending_notification_subjects(
        &self,
    ) -> Result<(), CollaborationError> {
        for intent in self.store.pending_notification_subjects().await? {
            let key = format!(
                "{}:{}:notification_subject:{}",
                intent.account_id, intent.authorization_epoch, intent.notification_id
            );
            if intent.attempts >= 3 {
                if !self.scheduler.lock().await.active.contains_key(&key)
                    && let Ok(revision) = self.store.exhaust_notification_subject(&intent).await
                {
                    self.publish(revision);
                }
                continue;
            }
            let query = NotificationSubjectQuery {
                account_id: intent.account_id.clone(),
                authorization_epoch: intent.authorization_epoch.clone(),
                notification_id: intent.notification_id.clone(),
            };
            let snapshot = match self.notification_subject(query).await {
                Ok(s) => s,
                Err(error)
                    if matches!(
                        error.code,
                        ErrorCode::AuthRequired | ErrorCode::StaleView | ErrorCode::NotFound
                    ) =>
                {
                    continue;
                }
                Err(error) => return Err(error),
            };
            if matches!(
                snapshot.state,
                NotificationSubjectState::Resolved
                    | NotificationSubjectState::Ambiguous
                    | NotificationSubjectState::Unavailable
                    | NotificationSubjectState::Unsupported
            ) || snapshot.selector_generation.as_ref() != Some(&intent.selector_generation)
            {
                if let Ok(revision) = self.store.retire_notification_subject_intent(&intent).await {
                    self.publish(revision);
                }
                continue;
            }
            if !snapshot.discovery.admission {
                continue;
            }
            let account = self.active_account(&intent.account_id).await?;
            match self
                .enqueue_work_reason(
                    account,
                    None,
                    JobKind::NotificationSubject {
                        intent: intent.clone(),
                    },
                    format!("notification_subject:{}", intent.notification_id),
                    scheduler::Admission::Explicit,
                )
                .await
            {
                Ok(_) => {}
                Err(error) if error.code == ErrorCode::Busy => continue,
                Err(error) => return Err(error),
            }
        }
        Ok(())
    }

    pub(super) async fn sync_notification_subject(
        &self,
        intent: &NotificationDiscoveryIntent,
        job: &mut Job,
    ) -> Result<bool, CollaborationError> {
        let account = self.active_account(&intent.account_id).await?;
        if account.authorization_epoch != intent.authorization_epoch {
            return Err(stale());
        }
        let query = NotificationSubjectQuery {
            account_id: intent.account_id.clone(),
            authorization_epoch: intent.authorization_epoch.clone(),
            notification_id: intent.notification_id.clone(),
        };
        let snapshot = self.notification_subject(query).await?;
        if !snapshot.discovery.admission
            || snapshot.selector_generation.as_ref() != Some(&intent.selector_generation)
        {
            return Err(stale());
        }
        let lease = self.store.begin_notification_subject(intent).await?;
        self.publish(self.store.revision().await?);
        let result = async {
            let _lifecycle = self.lifecycle.lock().await;
            let current = self.active_account(&account.id).await?;
            if current.authorization_epoch != account.authorization_epoch {
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
            self.ensure_provider_budget(&current, job).await?;
            let token = self.load_token(&reference).await?.ok_or_else(|| {
                CollaborationError::new(ErrorCode::AuthRequired, "Reconnect this provider account")
            })?;
            let adapter = self.adapter_for_account(&current).await?;
            if adapter.notification_subject_support(&current, lease.request.selector.kind)
                != CapabilityState::Supported
            {
                return Err(unsupported());
            }
            drop(_lifecycle);
            self.store
                .notification_subject_dispatchable(&lease, &self.now_string())
                .await
                .inspect_err(|error| {
                    // This Store refusal also consumes no provider request.
                    if error.code == ErrorCode::RateLimited {
                        job.local_budget_refusal = true;
                    }
                })?;
            self.ensure_provider_budget(&current, job).await?;
            match adapter
                .discover_notification_subject(&token, lease.request.clone())
                .await
            {
                Ok(value) => Ok(value),
                Err(error) => {
                    if let Some(seconds) = error.account_cooldown_seconds.filter(|s| *s > 0) {
                        self.persist_rate_limit(&account, seconds, None).await?;
                    }
                    Err(error.into())
                }
            }
        }
        .await;
        let mut result = match result {
            Ok(result) => result,
            Err(error) => {
                self.record_notification_subject_error(&lease, error, job.local_budget_refusal)
                    .await?;
                return Ok(false);
            }
        };
        let cooldown = match &result {
            NotificationSubjectDiscovery::Verified { detail, .. } => detail.cooldown_seconds,
            NotificationSubjectDiscovery::Unresolved {
                cooldown_seconds, ..
            }
            | NotificationSubjectDiscovery::Failed {
                cooldown_seconds, ..
            } => *cooldown_seconds,
        };
        if let Some(seconds) = cooldown.filter(|s| *s > 0) {
            self.persist_rate_limit(&account, seconds, None).await?;
        }
        if let NotificationSubjectDiscovery::Failed { error, .. } = result {
            self.record_notification_subject_error(&lease, error.into(), false)
                .await?;
            return Ok(false);
        }
        if let NotificationSubjectDiscovery::Verified { detail, .. } = &mut result {
            detail.source.observed_at = self.now_string();
            if let Some(metadata) = &mut detail.metadata {
                metadata.source.observed_at = detail.source.observed_at.clone();
            }
        }
        match self.store.apply_notification_subject(&lease, result).await {
            Ok(revision) => self.publish(revision),
            Err(error) if error.code == ErrorCode::StaleView => return Err(error),
            Err(error) => {
                self.record_notification_subject_error(&lease, error, job.local_budget_refusal)
                    .await?
            }
        }
        Ok(false)
    }

    async fn record_notification_subject_error(
        &self,
        lease: &NotificationDiscoveryLease,
        error: CollaborationError,
        local_budget_refusal: bool,
    ) -> Result<(), CollaborationError> {
        if error.code == ErrorCode::StaleView {
            return Err(error);
        }
        let delay = error.retry_after_seconds.map(u64::from).unwrap_or_else(|| {
            (60u64.saturating_mul(1u64 << lease.attempts.saturating_sub(1).min(3))).min(900)
                + self.clock.jitter() % 15
        });
        let transient = matches!(
            error.code,
            ErrorCode::Network | ErrorCode::Provider | ErrorCode::RateLimited
        );
        let revision = self
            .store
            .fail_notification_subject(
                lease,
                error.clone(),
                transient.then(|| self.future_string(delay)),
            )
            .await?;
        self.publish(revision);
        if error.code == ErrorCode::RateLimited && !local_budget_refusal {
            self.persist_rate_limit(&lease.request.account, delay, Some(error.clone()))
                .await?;
        }
        if error.code == ErrorCode::AuthRequired {
            self.reset_account_scheduler(&lease.request.account.id)
                .await;
        }
        Ok(())
    }
}
