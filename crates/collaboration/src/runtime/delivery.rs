//! One bounded delivery turn shares the read scheduler's dispatch, quota and
//! credential-cutover gates. Native completion survives caller cancellation.
use super::*;
use crate::delivery::*;
use crate::storage::delivery::DeliveryCompletion;
use futures_util::FutureExt;
use std::panic::AssertUnwindSafe;

impl CollaborationRuntime {
    pub(crate) async fn run_delivery_next(&self) -> Result<bool, CollaborationError> {
        self.owned_operation(|runtime| async move { runtime.delivery_turn().await })
            .await
    }
    async fn delivery_turn(&self) -> Result<bool, CollaborationError> {
        let Ok(_lane) = self.dispatch.try_lock() else {
            return Ok(false);
        };
        let keys = self.delivery_keys().await?;
        for key in keys {
            {
                let mut scheduler = self.scheduler.lock().await;
                scheduler.delivery_account_cursor = Some(key.0.clone());
                scheduler
                    .delivery_cursors
                    .insert(key.0.clone(), key.1.clone());
            }
            let command = self.store.delivery_command(&key.0, &key.1).await?;
            if command.attention.is_some() {
                continue;
            }
            if command.state == DeliveryState::Sending {
                self.publish(
                    self.store
                        .recover_delivery(&command, &self.now_string())
                        .await?,
                );
                return Ok(true);
            }
            if !self.delivery_due(&command).await {
                continue;
            }
            // Keep lock ordering consistent with read dispatch -> lifecycle.
            // Account replacement waits until this finite provider turn and its
            // durable result finish, so it cannot redirect an in-flight write.
            let _lifecycle = self.lifecycle.lock().await;
            if self.is_stopping() {
                return Ok(false);
            }
            let account = match self.active_account(&command.account_id).await {
                Ok(a) => a,
                Err(_) => continue,
            };
            if !command.reconcile_only()
                && account.authorization_epoch != command.authorization_epoch
            {
                continue;
            }
            let instance = ProviderInstance::for_account(&account)?;
            self.adapter_for_account(&account).await?;
            let Some(policy) = self.registry.delivery_policy(
                &instance,
                &command.operation_kind,
                command.payload_version,
            ) else {
                continue;
            };
            if self.check_provider_budget(&account).await.is_err() {
                continue;
            }
            if command.reconcile_only() {
                let (request, revision) = self
                    .store
                    .claim_reconciliation(&command, &account, &self.delivery_time().await)
                    .await?;
                self.publish(revision);
                let Some(request) = request else {
                    return Ok(true);
                };
                let Some(token) = self.delivery_token(&account, &request.command).await? else {
                    return Ok(true);
                };
                let result = bounded(policy.reconcile(&token, request.clone())).await;
                let (report, error) = match result {
                    Some(Ok(report)) => (report, None),
                    Some(Err(error)) => (DeliveryReport::unknown(), Some(error)),
                    None => (DeliveryReport::unknown(), None),
                };
                // Record unresolved evidence/state first even if saving a shared
                // quota observation subsequently fails.
                self.complete_delivery_turn(
                    &request.command,
                    &account,
                    None,
                    policy.as_ref(),
                    report,
                )
                .await?;
                if let Some(error) = error {
                    self.delivery_provider_error(&account, &error).await?;
                }
                return Ok(true);
            }
            let (request, revision) = self
                .store
                .claim_preparation(&command, &account, &self.delivery_time().await)
                .await?;
            let changed = revision.is_some();
            if let Some(revision) = revision {
                self.publish(revision);
            }
            let Some(request) = request else {
                if changed {
                    return Ok(true);
                }
                continue;
            };
            let command = request.command.clone();
            let Some(token) = self.delivery_token(&account, &command).await? else {
                return Ok(true);
            };
            let preparation = match bounded(policy.prepare(&token, &request)).await {
                Some(Ok(value)) => value,
                Some(Err(error)) => {
                    self.defer_delivery_turn(
                        &command,
                        error.retry_after_seconds.unwrap_or(60).max(1),
                    )
                    .await?;
                    self.delivery_provider_error(&account, &error).await?;
                    return Ok(true);
                }
                None => {
                    self.defer_delivery_turn(&command, 60).await?;
                    return Ok(true);
                }
            };
            self.check_provider_budget(&account).await?;
            let claim = self
                .store
                .claim_delivery(
                    &command,
                    &account,
                    policy.as_ref(),
                    &preparation,
                    &self.delivery_time().await,
                )
                .await?;
            let changed = claim.revision.is_some();
            if let Some(revision) = claim.revision {
                self.publish(revision);
            }
            let Some(request) = claim.request else {
                if changed {
                    return Ok(true);
                }
                continue;
            };
            let report = bounded(policy.dispatch(&token, request.clone()))
                .await
                .unwrap_or_else(DeliveryReport::unknown);
            self.complete_delivery_turn(
                &request.command,
                &account,
                Some(request.attempt),
                policy.as_ref(),
                report,
            )
            .await?;
            return Ok(true);
        }
        Ok(false)
    }
    async fn delivery_keys(&self) -> Result<Vec<(String, String)>, CollaborationError> {
        for _ in 0..2 {
            let after = self.scheduler.lock().await.delivery_account_cursor.clone();
            let accounts = self.store.delivery_accounts(after.as_deref()).await?;
            let mut keys = Vec::new();
            for account in accounts {
                if keys.len() >= 32 {
                    break;
                }
                let command_after = self
                    .scheduler
                    .lock()
                    .await
                    .delivery_cursors
                    .get(&account)
                    .cloned();
                let mut page = self
                    .store
                    .delivery_candidates(&account, command_after.as_deref(), 32 - keys.len())
                    .await?;
                if page.is_empty() && command_after.is_some() {
                    page = self
                        .store
                        .delivery_candidates(&account, None, 32 - keys.len())
                        .await?;
                }
                if page.is_empty() {
                    self.scheduler.lock().await.delivery_account_cursor = Some(account);
                }
                keys.extend(page);
            }
            if !keys.is_empty() || after.is_none() {
                return Ok(keys);
            }
            self.scheduler.lock().await.delivery_account_cursor = None;
        }
        Ok(vec![])
    }
    async fn delivery_token(
        &self,
        account: &RemoteAccount,
        command: &DeliveryCommand,
    ) -> Result<Option<SecretToken>, CollaborationError> {
        if let Some(reference) = self.store.credential_reference(&account.id).await? {
            match self.load_token(&reference).await {
                Ok(Some(token)) => return Ok(Some(token)),
                Err(_) => {
                    self.defer_delivery_turn(command, 60).await?;
                    return Ok(None);
                }
                Ok(None) => {}
            }
        }
        self.delivery_provider_error(
            account,
            &ProviderError::new(ProviderErrorKind::Authentication),
        )
        .await?;
        Ok(None)
    }
    async fn complete_delivery_turn(
        &self,
        command: &DeliveryCommand,
        account: &RemoteAccount,
        attempt: Option<i64>,
        policy: &dyn CommandDeliveryPolicy,
        mut report: DeliveryReport,
    ) -> Result<(), CollaborationError> {
        // Invalid/unbounded adapter claims never certify a side effect or cause
        // resubmission. Preserve the ambiguous attempt with no accepted proof.
        if report.outcome.proof().is_some_and(|(purpose, proof)| {
            !proof.bounded() || !policy.validate_evidence(command, purpose, proof)
        }) {
            report.outcome = DeliveryOutcome::Unknown;
        }
        let delay = report.retry_after_seconds.unwrap_or(60).max(1);
        let revision = self
            .store
            .complete_delivery(
                command,
                policy,
                DeliveryCompletion {
                    account,
                    attempt,
                    report: &report,
                    now: &self.now_string(),
                    next: &self.delivery_future(delay).await,
                },
            )
            .await?;
        self.publish(revision);
        if let Some(cooldown) = report.account_cooldown_seconds {
            self.persist_rate_limit(account, cooldown, None).await?;
        }
        Ok(())
    }
    async fn delivery_provider_error(
        &self,
        account: &RemoteAccount,
        error: &ProviderError,
    ) -> Result<(), CollaborationError> {
        if let Some(cooldown) =
            error
                .account_cooldown_seconds
                .or(if error.kind == ProviderErrorKind::RateLimited {
                    error.retry_after_seconds.or(Some(60))
                } else {
                    None
                })
        {
            self.persist_rate_limit(account, cooldown, None).await?;
        }
        if error.kind == ProviderErrorKind::Authentication {
            let revision = self
                .store
                .set_sync_status(
                    &account.id,
                    &account.authorization_epoch,
                    "repositories",
                    SyncStatus {
                        state: SyncState::AuthRequired,
                        last_success_at: None,
                        next_retry_at: None,
                        error: Some(error.clone().into()),
                    },
                )
                .await?;
            self.publish(revision);
            self.reset_account_scheduler(&account.id).await;
        }
        Ok(())
    }
    async fn defer_delivery_turn(
        &self,
        command: &DeliveryCommand,
        delay: u64,
    ) -> Result<(), CollaborationError> {
        let delay = delay.max(1);
        self.publish(
            self.store
                .defer_delivery(command, &self.delivery_future(delay).await)
                .await?,
        );
        Ok(())
    }
    async fn delivery_clock_utc(&self) -> DateTime<Utc> {
        let mut scheduler = self.scheduler.lock().await;
        let now = self.now();
        let (utc, instant) = *scheduler
            .delivery_clock
            .get_or_insert_with(|| (self.clock.utc(), now));
        let elapsed =
            chrono::Duration::from_std(now.checked_duration_since(instant).unwrap_or_default())
                .unwrap_or(chrono::Duration::MAX);
        utc.checked_add_signed(elapsed)
            .unwrap_or(DateTime::<Utc>::MAX_UTC)
    }
    async fn delivery_time(&self) -> DeliveryTime {
        DeliveryTime {
            now: self.now_string(),
            command_now: self.delivery_future(0).await,
        }
    }
    async fn delivery_future(&self, seconds: u64) -> String {
        let ceiling =
            DateTime::<Utc>::from_timestamp(253_402_300_799, 999_000_000).expect("RFC3339 ceiling");
        self.delivery_clock_utc()
            .await
            .checked_add_signed(chrono::Duration::seconds(
                seconds.min(i64::MAX as u64 / 1000) as i64,
            ))
            .unwrap_or(ceiling)
            .min(ceiling)
            .to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
    }
    async fn delivery_due(&self, command: &DeliveryCommand) -> bool {
        let now = self.delivery_clock_utc().await;
        command.next_action_at.as_deref().is_none_or(|value| {
            DateTime::parse_from_rfc3339(value).is_ok_and(|deadline| deadline <= now)
        })
    }
}
async fn bounded<F: std::future::Future>(future: F) -> Option<F::Output> {
    tokio::time::timeout(
        Duration::from_secs(CALL_TIMEOUT_SECONDS),
        AssertUnwindSafe(future).catch_unwind(),
    )
    .await
    .ok()?
    .ok()
}
#[cfg(test)]
mod tests;
