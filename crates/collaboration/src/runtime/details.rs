//! Explicit detail demand uses the single engine queue, quota and credential owner.
use super::*;

impl CollaborationRuntime {
    pub(super) async fn require_detail(
        &self,
        account: &RemoteAccount,
        subject: &RemoteItem,
        facet: DetailFacet,
    ) -> Result<(), CollaborationError> {
        let facet = facet.capability(&subject.kind).ok_or_else(unsupported)?;
        let capability = self
            .adapter_for_account(account)
            .await?
            .profile(account)
            .facet(facet);
        match capability.state {
            CapabilityState::Supported => Ok(()),
            CapabilityState::Unsupported => Err(unsupported()),
            CapabilityState::Unavailable => Err(CollaborationError::new(
                ErrorCode::PermissionDenied,
                "This detail capability is currently unavailable",
            )),
        }
    }
    pub async fn hydrate_detail(
        &self,
        request: HydrateDetailRequest,
    ) -> Result<RefreshReceipt, CollaborationError> {
        let account = self.active_account(&request.account_id).await?;
        if account.authorization_epoch != request.authorization_epoch {
            return Err(stale());
        }
        let subject = self
            .store
            .detail_subject(&account.id, &request.subject_id)
            .await?;
        self.require_detail(&account, &subject, request.facet)
            .await?;
        let repository = self
            .store
            .repository(
                &account.id,
                subject.repository_id.as_deref().ok_or_else(unsupported)?,
            )
            .await?;
        // Durable read intent precedes dispatch. Queue pressure can defer it to
        // the next background admission pass, without dropping explicit demand.
        let revision = self
            .store
            .request_detail(
                &account.id,
                &account.authorization_epoch,
                &subject.id,
                request.facet,
            )
            .await?;
        self.publish(revision);
        let job_id = self
            .enqueue_work(
                account,
                Some(repository),
                JobKind::Detail {
                    subject_id: request.subject_id.clone(),
                    facet: request.facet,
                },
                request.facet.scope(&request.subject_id),
                true,
            )
            .await?;
        self.notify.notify_one();
        Ok(RefreshReceipt { job_id })
    }
    pub(super) async fn enqueue_pending_details(&self) -> Result<(), CollaborationError> {
        let cursor = self.scheduler.lock().await.detail_cursor.clone();
        for demand in self.store.pending_detail_batch(cursor.as_ref()).await? {
            // Advance over blocked and already fresh subjects too; a sorted
            // prefix must not permanently hide later explicit read intent.
            self.scheduler.lock().await.detail_cursor = Some(demand.clone());
            let account = match self.active_account(&demand.account_id).await {
                Ok(account) => account,
                Err(error)
                    if matches!(error.code, ErrorCode::AuthRequired | ErrorCode::Unsupported) =>
                {
                    continue;
                }
                Err(error) => return Err(error),
            };
            let subject = match self
                .store
                .detail_subject(&account.id, &demand.subject_id)
                .await
            {
                Ok(subject) => subject,
                Err(error)
                    if matches!(
                        error.code,
                        ErrorCode::PermissionDenied | ErrorCode::NotFound
                    ) =>
                {
                    continue;
                }
                Err(error) => return Err(error),
            };
            if let Err(error) = self.require_detail(&account, &subject, demand.facet).await {
                if matches!(
                    error.code,
                    ErrorCode::Unsupported | ErrorCode::PermissionDenied
                ) {
                    continue;
                }
                return Err(error);
            }
            let repository = self
                .store
                .repository(
                    &account.id,
                    subject.repository_id.as_deref().ok_or_else(unsupported)?,
                )
                .await?;
            match self
                .enqueue_work_reason(
                    account,
                    Some(repository),
                    JobKind::Detail {
                        subject_id: demand.subject_id.clone(),
                        facet: demand.facet,
                    },
                    demand.facet.scope(&demand.subject_id),
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
    pub(super) async fn sync_detail_page(
        &self,
        job: &mut Job,
        subject_id: &str,
        facet: DetailFacet,
    ) -> Result<bool, CollaborationError> {
        self.ensure_demand_dispatch(job).await?;
        let (account, token, mut lease) = {
            let _lifecycle = self.lifecycle.lock().await;
            let account = self.active_account(&job.account.id).await?;
            if account.authorization_epoch != job.account.authorization_epoch {
                return Err(stale());
            }
            let subject = self.store.detail_subject(&account.id, subject_id).await?;
            self.require_detail(&account, &subject, facet).await?;
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
            let lease = if let Some(lease) = &job.detail_lease {
                lease.clone()
            } else {
                let lease = self
                    .store
                    .begin_detail(&account.id, &account.authorization_epoch, subject_id, facet)
                    .await?;
                self.publish(self.store.revision().await?);
                lease
            };
            (account, token, lease)
        };
        let starts_at_beginning = lease.next_cursor.is_none();
        let conditional = lease.etag.is_some();
        let page_index = job.pages;
        {
            let current = self.active_account(&account.id).await?;
            if current.authorization_epoch != account.authorization_epoch {
                return Err(stale());
            }
            let subject = self.store.detail_subject(&account.id, subject_id).await?;
            self.require_detail(&current, &subject, facet).await?;
            let repository = self
                .store
                .repository(
                    &account.id,
                    subject.repository_id.as_deref().ok_or_else(unsupported)?,
                )
                .await?;
            let binding = DetailSubjectBinding {
                repository_id: repository.id.clone(),
                repository_provider_id: repository.provider_id.clone(),
                provider_id: subject.provider_id.clone(),
                number: subject.number.clone(),
                kind: subject.kind.clone(),
                head_oid: subject.head_oid.clone(),
            };
            let adapter = self.adapter_for_account(&current).await?;
            self.ensure_demand_dispatch(job).await?;
            self.store
                .validate_detail_dispatch(
                    &account.id,
                    &account.authorization_epoch,
                    subject_id,
                    facet,
                    &lease,
                    &binding,
                )
                .await?;
            let fetched = adapter
                .fetch_detail(
                    &token,
                    DetailRequest {
                        account: current,
                        repository,
                        subject,
                        facet,
                        cursor: lease.next_cursor.clone(),
                        etag: lease.etag.clone(),
                        source: lease.source.clone(),
                    },
                )
                .await;
            // A rejected singleton can consume account quota without proving
            // any Body, metadata or access. Keep that captured-epoch evidence
            // before converting the safe provider error, as feed reads do.
            let mut page = match fetched {
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
            // Receipt/validation time is engine owned, never the provider clock.
            page.source.observed_at = self.now_string();
            if let Some(metadata) = &mut page.metadata {
                metadata.source.observed_at = page.source.observed_at.clone();
            }
            if page.not_modified && (!conditional || page_index != 0) {
                return Err(CollaborationError::new(
                    ErrorCode::Provider,
                    "Unexpected detail conditional response",
                ));
            }
            let complete = page.not_modified || page.next_cursor.is_none();
            let cooldown = page.cooldown_seconds.unwrap_or(0);
            let result = self
                .store
                .apply_detail(DetailCommit {
                    reconciliation: page.reconciliation,
                    account_id: account.id.clone(),
                    authorization_epoch: account.authorization_epoch.clone(),
                    authorization_view: lease.authorization_view.clone(),
                    instance_id: lease.instance_id.clone(),
                    subject_id: subject_id.into(),
                    facet,
                    run_id: lease.run_id.clone(),
                    request_cursor: lease.next_cursor.clone(),
                    body: page.body,
                    metadata: page.metadata,
                    subject_binding: Some(binding),
                    entries: page.entries,
                    source: page.source.clone(),
                    next_cursor: page.next_cursor.clone(),
                    etag: page.etag,
                    not_modified: page.not_modified,
                    whole_scope: starts_at_beginning && page_index == 0 && complete,
                    complete,
                    freshness_seconds: page.freshness_seconds,
                })
                .await;
            let revision = match result {
                Ok(revision) => revision,
                Err(error)
                    if crate::storage::facet_reconciliation::is_drift(&error)
                        && !job.detail_restarted =>
                {
                    // The rejected page never committed provider truth. Reset
                    // only this exact live traversal, once per admitted job.
                    let revision = self
                        .store
                        .restart_detail_traversal(
                            &account.id,
                            &account.authorization_epoch,
                            subject_id,
                            facet,
                            &lease,
                        )
                        .await?;
                    self.publish(revision);
                    job.detail_restarted = true;
                    job.detail_lease = None;
                    if cooldown > 0 {
                        self.persist_rate_limit(&account, cooldown, None).await?;
                    }
                    return Ok(cooldown == 0);
                }
                Err(error) if crate::storage::facet_reconciliation::is_drift(&error) => {
                    // A broken adapter must not spin through representation
                    // restarts. Later independent reads still respect the
                    // ordinary strict retry barriers.
                    self.store
                        .stop_detail_reconciliation(
                            &account.id,
                            &account.authorization_epoch,
                            subject_id,
                            facet,
                            &lease,
                        )
                        .await?;
                    if cooldown > 0 {
                        self.persist_rate_limit(&account, cooldown, None).await?;
                    }
                    return Err(CollaborationError::new(
                        ErrorCode::Provider,
                        "Detail traversal changed repeatedly",
                    ));
                }
                Err(error) => return Err(error),
            };
            self.publish(revision);
            lease.next_cursor = page.next_cursor;
            lease.source = Some(page.source);
            lease.reconciliation = Some(page.reconciliation);
            lease.etag = None;
            job.detail_lease = Some(lease);
            {
                let mut scheduler = self.scheduler.lock().await;
                scheduler.failures.remove(&job.key);
                // Partial bounded reads keep their durable intent and yield.
                scheduler.due.insert(
                    job.key.clone(),
                    self.deadline_after(if complete {
                        u64::from(page.freshness_seconds.max(1))
                    } else {
                        0
                    }),
                );
                if cooldown > 0 {
                    scheduler
                        .account_cooldowns
                        .entry(account.id.clone())
                        .and_modify(|old| *old = (*old).max(self.deadline_after(cooldown)))
                        .or_insert_with(|| self.deadline_after(cooldown));
                }
            }
            if cooldown > 0 {
                self.persist_rate_limit(&account, cooldown, None).await?;
                let revision = self
                    .store
                    .set_sync_status(
                        &account.id,
                        &account.authorization_epoch,
                        &job.scope,
                        SyncStatus {
                            state: SyncState::RateLimited,
                            last_success_at: Some(self.now_string()),
                            next_retry_at: Some(self.future_string(cooldown)),
                            error: None,
                        },
                    )
                    .await?;
                self.publish(revision);
            }
            Ok(!complete && cooldown == 0)
        }
    }
}
