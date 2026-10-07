//! Pull-commit hydration reuses the single native scheduler and quota owner.
use super::*;

fn pull_commit_page_is_terminal(lease: &PullCommitLease, page: &PullCommitProviderPage) -> bool {
    let available = MAX_PULL_COMMITS.saturating_sub(lease.row_count) as usize;
    let accepted = page.commits.len().min(available);
    let new_count = lease
        .row_count
        .saturating_add(u32::try_from(accepted).unwrap_or(u32::MAX));
    let local_cap = accepted < page.commits.len()
        || (new_count == MAX_PULL_COMMITS && page.remote_has_more)
        || (lease.page_count.saturating_add(1) >= MAX_PULL_COMMIT_PAGES
            && page.next_cursor.is_some());
    local_cap || page.next_cursor.is_none()
}

fn observed_known(metadata: &ResourceMetadataObservation, field: MetadataField) -> bool {
    let mut observations = metadata
        .fields
        .iter()
        .filter(|observation| observation.field == field);
    observations
        .next()
        .is_some_and(|observation| observation.state == DetailValueState::Known)
        && observations.next().is_none()
}

fn pull_commit_range_validation(page: &DetailPage) -> Option<PullCommitRangeValidation> {
    if page.not_modified || page.next_cursor.is_some() {
        return None;
    }
    let metadata = page.metadata.as_ref()?;
    if metadata.kind != RemoteItemKind::PullRequest
        || !observed_known(metadata, MetadataField::Base)
        || !observed_known(metadata, MetadataField::Head)
    {
        return None;
    }
    let base = metadata.values.base.as_ref()?;
    let head = metadata.values.head.as_ref()?;
    Some(PullCommitRangeValidation {
        base_oid: base.oid.clone(),
        head_oid: head.oid.clone(),
        base_repository_provider_id: base.repository.as_ref()?.provider_id.clone(),
        source_repository_provider_id: head.repository.as_ref()?.provider_id.clone(),
    })
}

fn validation_matches(validation: &PullCommitRangeValidation, binding: &PullCommitBinding) -> bool {
    validation.base_oid == binding.context.base_oid
        && validation.head_oid == binding.context.head_oid
        && validation.base_repository_provider_id == binding.repository_provider_id
        && validation.source_repository_provider_id == binding.context.source_repository_provider_id
}

enum PullCommitPreparation {
    Ready {
        account: RemoteAccount,
        token: SecretToken,
        lease: PullCommitLease,
    },
    NeedsBody {
        account: RemoteAccount,
        repository: RemoteRepository,
        subject: RemoteItem,
    },
}

impl CollaborationRuntime {
    pub(super) async fn request_pull_commit_body_context(
        &self,
        account: &RemoteAccount,
        repository: &RemoteRepository,
        subject: &RemoteItem,
    ) -> Result<(), CollaborationError> {
        self.require_detail(account, subject, DetailFacet::Body)
            .await?;
        let revision = self
            .store
            .request_detail(
                &account.id,
                &account.authorization_epoch,
                &subject.id,
                DetailFacet::Body,
            )
            .await?;
        self.publish(revision);
        match self
            .enqueue_work_reason(
                account.clone(),
                Some(repository.clone()),
                JobKind::Detail {
                    subject_id: subject.id.clone(),
                    facet: DetailFacet::Body,
                },
                DetailFacet::Body.scope(&subject.id),
                scheduler::Admission::Explicit,
            )
            .await
        {
            Ok(_) => self.notify.notify_one(),
            Err(error) if error.code == ErrorCode::Busy => {}
            Err(error) => return Err(error),
        }
        Ok(())
    }

    async fn mark_pull_commit_rate_limited(
        &self,
        account: &RemoteAccount,
        scope: &str,
        cooldown: u64,
    ) -> Result<(), CollaborationError> {
        if cooldown == 0 {
            return Ok(());
        }
        let revision = self
            .store
            .set_sync_status(
                &account.id,
                &account.authorization_epoch,
                scope,
                SyncStatus {
                    state: SyncState::RateLimited,
                    last_success_at: None,
                    next_retry_at: Some(self.future_string(cooldown)),
                    error: None,
                },
            )
            .await?;
        self.publish(revision);
        Ok(())
    }

    async fn apply_pull_commit_parent_observation(
        &self,
        account: &RemoteAccount,
        repository: &RemoteRepository,
        subject: &RemoteItem,
        authorization_view: &str,
        mut page: DetailPage,
    ) -> Result<(), CollaborationError> {
        if page.not_modified || page.next_cursor.is_some() {
            return Err(CollaborationError::new(
                ErrorCode::Provider,
                "Pull commit range validation was not a fresh singleton response",
            ));
        }
        let lease = self
            .store
            .begin_detail_at_authorization_view(
                &account.id,
                &account.authorization_epoch,
                &subject.id,
                DetailFacet::Body,
                authorization_view,
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
        page.source.observed_at = self.now_string();
        if let Some(metadata) = &mut page.metadata {
            metadata.source.observed_at = page.source.observed_at.clone();
        }
        let revision = self
            .store
            .apply_detail(DetailCommit {
                reconciliation: page.reconciliation,
                account_id: account.id.clone(),
                authorization_epoch: account.authorization_epoch.clone(),
                authorization_view: lease.authorization_view,
                instance_id: lease.instance_id,
                subject_id: subject.id.clone(),
                facet: DetailFacet::Body,
                run_id: lease.run_id,
                request_cursor: None,
                body: page.body,
                metadata: page.metadata,
                subject_binding: Some(binding),
                check_context: None,
                review_context: None,
                entries: page.entries,
                source: page.source,
                next_cursor: None,
                etag: page.etag,
                not_modified: false,
                whole_scope: true,
                complete: true,
                freshness_seconds: page.freshness_seconds,
            })
            .await?;
        self.publish(revision);
        self.request_pull_commit_body_context(account, repository, subject)
            .await
    }

    pub(super) async fn sync_pull_commit_page(
        &self,
        job: &mut Job,
        subject_id: &str,
    ) -> Result<bool, CollaborationError> {
        self.ensure_demand_dispatch(job).await?;
        let preparation = async {
            let _lifecycle = self.lifecycle.lock().await;
            let account = self.active_account(&job.account.id).await?;
            if account.authorization_epoch != job.account.authorization_epoch {
                return Err(stale());
            }
            let subject = self.store.detail_subject(&account.id, subject_id).await?;
            self.require_detail(&account, &subject, DetailFacet::Commits)
                .await?;
            let repository = self
                .store
                .repository(
                    &account.id,
                    subject.repository_id.as_deref().ok_or_else(unsupported)?,
                )
                .await?;
            let lease = if let Some(lease) = &job.pull_commit_lease {
                lease.clone()
            } else {
                match self
                    .store
                    .begin_pull_commits(&account.id, &account.authorization_epoch, subject_id)
                    .await
                {
                    Ok(lease) => {
                        self.publish(self.store.revision().await?);
                        lease
                    }
                    Err(error)
                        if matches!(error.code, ErrorCode::NotFound | ErrorCode::StaleView) =>
                    {
                        return Ok(PullCommitPreparation::NeedsBody {
                            account,
                            repository,
                            subject,
                        });
                    }
                    Err(error) => return Err(error),
                }
            };
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
            Ok(PullCommitPreparation::Ready {
                account,
                token,
                lease,
            })
        }
        .await;
        let (account, token, lease) = match preparation? {
            PullCommitPreparation::Ready {
                account,
                token,
                lease,
            } => (account, token, lease),
            PullCommitPreparation::NeedsBody {
                account,
                repository,
                subject,
            } => {
                self.request_pull_commit_body_context(&account, &repository, &subject)
                    .await?;
                return Ok(false);
            }
        };
        let current = self.active_account(&account.id).await?;
        if current.authorization_epoch != account.authorization_epoch {
            return Err(stale());
        }
        let subject = self.store.detail_subject(&account.id, subject_id).await?;
        self.require_detail(&current, &subject, DetailFacet::Commits)
            .await?;
        let repository = self
            .store
            .repository(
                &account.id,
                subject.repository_id.as_deref().ok_or_else(unsupported)?,
            )
            .await?;
        let adapter = self.adapter_for_account(&current).await?;
        self.ensure_demand_dispatch(job).await?;
        self.ensure_provider_budget(&current, job).await?;
        let fetched = adapter
            .fetch_pull_commits(
                &token,
                PullCommitRequest {
                    account: current.clone(),
                    repository: repository.clone(),
                    subject: subject.clone(),
                    context: lease.binding.context.clone(),
                    cursor: lease.next_cursor.clone(),
                    start_position: lease.row_count,
                },
            )
            .await;
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
        let terminal = pull_commit_page_is_terminal(&lease, &page);
        let mut cooldown = page.cooldown_seconds.unwrap_or(0);
        if cooldown > 0 {
            self.persist_rate_limit(&account, cooldown, None).await?;
        }
        if terminal && cooldown > 0 {
            self.mark_pull_commit_rate_limited(&account, &job.scope, cooldown)
                .await?;
            return Ok(false);
        }

        let terminal_validation = if terminal {
            self.ensure_demand_dispatch(job).await?;
            let validation_account = self.active_account(&account.id).await?;
            if validation_account.authorization_epoch != account.authorization_epoch {
                return Err(stale());
            }
            let validation_subject = self
                .store
                .detail_subject(&validation_account.id, subject_id)
                .await?;
            self.require_detail(&validation_account, &validation_subject, DetailFacet::Body)
                .await?;
            let validation_repository = self
                .store
                .repository(
                    &validation_account.id,
                    validation_subject
                        .repository_id
                        .as_deref()
                        .ok_or_else(unsupported)?,
                )
                .await?;
            if validation_repository.id != lease.binding.repository_id
                || validation_repository.provider_id != lease.binding.repository_provider_id
                || validation_subject.provider_id != lease.binding.provider_id
                || validation_subject.number != lease.binding.number
            {
                return Err(stale());
            }
            let validation_adapter = self.adapter_for_account(&validation_account).await?;
            self.ensure_provider_budget(&validation_account, job)
                .await?;
            let validation_page = match validation_adapter
                .fetch_detail(
                    &token,
                    DetailRequest {
                        account: validation_account.clone(),
                        repository: validation_repository.clone(),
                        subject: validation_subject.clone(),
                        facet: DetailFacet::Body,
                        cursor: None,
                        etag: None,
                        source: None,
                    },
                )
                .await
            {
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
            let validation_cooldown = validation_page.cooldown_seconds.unwrap_or(0);
            if validation_cooldown > 0 {
                cooldown = cooldown.max(validation_cooldown);
                self.persist_rate_limit(&account, validation_cooldown, None)
                    .await?;
            }
            let validation = pull_commit_range_validation(&validation_page);
            if validation
                .as_ref()
                .is_some_and(|validation| validation_matches(validation, &lease.binding))
            {
                validation
            } else {
                self.apply_pull_commit_parent_observation(
                    &validation_account,
                    &validation_repository,
                    &validation_subject,
                    &lease.authorization_view,
                    validation_page,
                )
                .await?;
                self.mark_pull_commit_rate_limited(&account, &job.scope, cooldown)
                    .await?;
                return Ok(false);
            }
        } else {
            None
        };
        let result = self
            .store
            .apply_pull_commits(PullCommitCommit {
                account_id: account.id.clone(),
                authorization_epoch: account.authorization_epoch.clone(),
                lease: lease.clone(),
                request_cursor: lease.next_cursor.clone(),
                page,
                terminal_validation,
            })
            .await;
        let receipt = match result {
            Ok(receipt) => receipt,
            Err(error) if is_pull_commit_drift(&error) && !job.pull_commit_restarted => {
                let replacement = self
                    .store
                    .begin_pull_commits(&account.id, &account.authorization_epoch, subject_id)
                    .await?;
                self.publish(self.store.revision().await?);
                job.pull_commit_restarted = true;
                job.pull_commit_lease = Some(replacement);
                return Ok(cooldown == 0);
            }
            Err(error) if is_pull_commit_drift(&error) => {
                self.store
                    .stop_detail_demand(
                        &account.id,
                        &account.authorization_epoch,
                        subject_id,
                        DetailFacet::Commits,
                    )
                    .await?;
                return Err(CollaborationError::new(
                    ErrorCode::Provider,
                    "Pull commit traversal changed repeatedly",
                ));
            }
            Err(error) => return Err(error),
        };
        self.publish(receipt.revision.clone());
        if receipt.published {
            job.pull_commit_lease = None;
        } else {
            let mut next = lease;
            next.next_cursor = receipt.next_cursor.clone();
            next.page_count = next.page_count.saturating_add(1);
            next.row_count = receipt.row_count;
            job.pull_commit_lease = Some(next);
        }
        {
            let mut scheduler = self.scheduler.lock().await;
            scheduler.failures.remove(&job.key);
            scheduler.due.insert(
                job.key.clone(),
                self.deadline_after(if receipt.published { 60 } else { 0 }),
            );
            if cooldown > 0 {
                scheduler
                    .account_cooldowns
                    .entry(account.id.clone())
                    .and_modify(|old| *old = (*old).max(self.deadline_after(cooldown)))
                    .or_insert_with(|| self.deadline_after(cooldown));
            }
        }
        self.mark_pull_commit_rate_limited(&account, &job.scope, cooldown)
            .await?;
        Ok(!receipt.published && receipt.next_cursor.is_some() && cooldown == 0)
    }
}
