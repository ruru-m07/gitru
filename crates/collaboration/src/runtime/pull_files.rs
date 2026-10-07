//! File collection hydration shares the native scheduler, budgets and vault.
//! Staging leases remain durable store authority across every scheduler turn.
use super::*;
use crate::storage::PullFileCommit;

fn file_source(account: &RemoteAccount) -> Result<PullFileSource, CollaborationError> {
    let strategy = match account.provider {
        ProviderKind::Github => PullFileSourceStrategy::GithubPullFiles,
        ProviderKind::Gitlab => PullFileSourceStrategy::GitlabMergeRequestDiffs,
        ProviderKind::BitbucketCloud => PullFileSourceStrategy::BitbucketCloudDiffstat,
        ProviderKind::BitbucketDc => return Err(unsupported()),
    };
    Ok(PullFileSource {
        strategy,
        adapter_version: 1,
    })
}

enum Preparation {
    Ready {
        request: Box<PullFileCollectionRequest>,
        token: SecretToken,
    },
    NeedsBody {
        account: Box<RemoteAccount>,
        repository: Box<RemoteRepository>,
        subject: Box<RemoteItem>,
    },
}

impl CollaborationRuntime {
    /// Admit an already bounded native local-Git observation through the same
    /// current-generation and authorization fence used by provider artifacts.
    pub async fn save_local_pull_file_artifact(
        &self,
        request: PullFileDiffRequest,
        membership: PullFileMembershipReceipt,
        artifact: PullFileArtifact,
    ) -> Result<String, CollaborationError> {
        let revision = self
            .store
            .apply_pull_file_artifact(request, membership, artifact)
            .await?;
        self.publish(revision.clone());
        Ok(revision)
    }

    pub(super) async fn request_pull_file_body_context(
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

    async fn mark_pull_file_rate_limited(
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

    pub(super) async fn sync_pull_file_page(
        &self,
        job: &mut Job,
        subject_id: &str,
    ) -> Result<bool, CollaborationError> {
        self.ensure_demand_dispatch(job).await?;
        let preparation = {
            let _lifecycle = self.lifecycle.lock().await;
            let account = self.active_account(&job.account.id).await?;
            if account.authorization_epoch != job.account.authorization_epoch {
                return Err(stale());
            }
            let subject = self.store.detail_subject(&account.id, subject_id).await?;
            self.require_detail(&account, &subject, DetailFacet::Files)
                .await?;
            let repository = self
                .store
                .repository(
                    &account.id,
                    subject.repository_id.as_deref().ok_or_else(unsupported)?,
                )
                .await?;
            let resumed = match self
                .store
                .resume_pull_files(&account.id, &account.authorization_epoch, subject_id)
                .await
            {
                Ok(lease) => lease,
                Err(error) if matches!(error.code, ErrorCode::StaleView | ErrorCode::NotFound) => {
                    None
                }
                Err(error) => return Err(error),
            };
            let source = file_source(&account)?;
            let resumed_current = resumed.filter(|lease| lease.source == source);
            let beginning = resumed_current.is_none();
            let lease = match resumed_current {
                Some(lease) => Ok(lease),
                None => {
                    self.store
                        .begin_pull_files(
                            &account.id,
                            &account.authorization_epoch,
                            subject_id,
                            source,
                        )
                        .await
                }
            };
            match lease {
                Err(error) if matches!(error.code, ErrorCode::StaleView | ErrorCode::NotFound) => {
                    Preparation::NeedsBody {
                        account: Box::new(account),
                        repository: Box::new(repository),
                        subject: Box::new(subject),
                    }
                }
                Err(error) => return Err(error),
                Ok(lease) => {
                    if beginning {
                        self.publish(self.store.revision().await?);
                    }
                    // Resolve every route and revalidate the persisted lease
                    // before a credential lookup or provider dispatch.
                    let request = self.store.pull_file_request(&lease).await?;
                    if request.source != file_source(&account)? {
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
                    self.ensure_provider_budget(&account, job).await?;
                    let token = self.load_token(&reference).await?.ok_or_else(|| {
                        CollaborationError::new(
                            ErrorCode::AuthRequired,
                            "Reconnect this provider account",
                        )
                    })?;
                    Preparation::Ready {
                        request: Box::new(request),
                        token,
                    }
                }
            }
        };
        let (request, token) = match preparation {
            Preparation::Ready { request, token } => (request, token),
            Preparation::NeedsBody {
                account,
                repository,
                subject,
            } => {
                self.request_pull_file_body_context(&account, &repository, &subject)
                    .await?;
                return Ok(false);
            }
        };
        self.ensure_demand_dispatch(job).await?;
        let request = self.store.pull_file_request(&request.lease).await?;
        self.require_detail(&request.account, &request.subject, DetailFacet::Files)
            .await?;
        self.ensure_provider_budget(&request.account, job).await?;
        let adapter = self.adapter_for_account(&request.account).await?;
        let fetched = adapter.fetch_pull_files(&token, request.clone()).await;
        let page = match fetched {
            Ok(page) => page,
            Err(error) => {
                if let Some(seconds) = error.account_cooldown_seconds.filter(|s| *s > 0) {
                    self.persist_rate_limit(&request.account, seconds, None)
                        .await?;
                }
                return Err(error.into());
            }
        };
        // Budget observations survive malformed pages and retired leases.
        let mut cooldown = page.cooldown_seconds.unwrap_or(0);
        if cooldown > 0 {
            self.persist_rate_limit(&request.account, cooldown, None)
                .await?;
        }
        page.validate_for(&request)?;
        let terminal = page.next_cursor.is_none();
        if terminal && cooldown > 0 {
            // Keep the preceding durable checkpoint. A terminal page cannot
            // publish without a second, freshly budgeted parent validation.
            self.mark_pull_file_rate_limited(&request.account, &job.scope, cooldown)
                .await?;
            return Ok(false);
        }
        let validation = if terminal {
            self.ensure_demand_dispatch(job).await?;
            let validation_request = self.store.pull_file_request(&request.lease).await?;
            self.require_detail(
                &validation_request.account,
                &validation_request.subject,
                DetailFacet::Files,
            )
            .await?;
            self.require_detail(
                &validation_request.account,
                &validation_request.subject,
                DetailFacet::Body,
            )
            .await?;
            self.ensure_provider_budget(&validation_request.account, job)
                .await?;
            let validation_adapter = self
                .adapter_for_account(&validation_request.account)
                .await?;
            match validation_adapter
                .validate_pull_file_range(&token, validation_request)
                .await
            {
                Ok(validation) => {
                    if let Some(seconds) = validation.cooldown_seconds.filter(|s| *s > 0) {
                        cooldown = cooldown.max(seconds);
                        self.persist_rate_limit(&request.account, seconds, None)
                            .await?;
                    }
                    Some(validation)
                }
                Err(error) => {
                    if let Some(seconds) = error.account_cooldown_seconds.filter(|s| *s > 0) {
                        self.persist_rate_limit(&request.account, seconds, None)
                            .await?;
                    }
                    // The parent may have advanced while enumeration ran. Body
                    // demand refreshes native context; existing active rows stay
                    // untouched while the provider error observes normal backoff.
                    if matches!(
                        error.kind,
                        ProviderErrorKind::InvalidResponse | ProviderErrorKind::Unavailable
                    ) {
                        self.request_pull_file_body_context(
                            &request.account,
                            &request.repository,
                            &request.subject,
                        )
                        .await?;
                    }
                    return Err(error.into());
                }
            }
        } else {
            None
        };
        let receipt = self
            .store
            .apply_pull_files(PullFileCommit {
                request: request.clone(),
                page,
                terminal_validation: validation.as_ref().map(|v| v.validation.clone()),
                expected_file_count: validation.as_ref().and_then(|v| v.expected_file_count),
                collection_cap: validation.as_ref().and_then(|v| v.collection_cap),
            })
            .await?;
        self.publish(receipt.revision);
        {
            let mut scheduler = self.scheduler.lock().await;
            scheduler.failures.remove(&job.key);
            scheduler.due.insert(
                job.key.clone(),
                self.deadline_after(if receipt.published { 60 } else { 0 }),
            );
        }
        self.mark_pull_file_rate_limited(&request.account, &job.scope, cooldown)
            .await?;
        Ok(!receipt.published && receipt.next_lease.is_some() && cooldown == 0)
    }
}
