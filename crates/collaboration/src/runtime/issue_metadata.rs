//! Local authored reads plus one-page catalog work on the shared native lane.
use super::*;
use crate::{issue_creation::*, issue_metadata::*};
pub(super) fn demand_kind(kind: IssueMetadataKind) -> DemandTargetKind {
    match kind {
        IssueMetadataKind::Labels => DemandTargetKind::RepositoryLabels,
        IssueMetadataKind::Assignees => DemandTargetKind::RepositoryAssignees,
        IssueMetadataKind::Milestones => DemandTargetKind::RepositoryMilestones,
    }
}
pub(super) fn metadata_kind(kind: DemandTargetKind) -> Option<IssueMetadataKind> {
    match kind {
        DemandTargetKind::RepositoryLabels => Some(IssueMetadataKind::Labels),
        DemandTargetKind::RepositoryAssignees => Some(IssueMetadataKind::Assignees),
        DemandTargetKind::RepositoryMilestones => Some(IssueMetadataKind::Milestones),
        _ => None,
    }
}
pub(super) fn scope(repo: &str, kind: IssueMetadataKind) -> String {
    format!(
        "repository_metadata:{repo}:{}",
        match kind {
            IssueMetadataKind::Labels => "labels",
            IssueMetadataKind::Assignees => "assignees",
            IssueMetadataKind::Milestones => "milestones",
        }
    )
}
impl CollaborationRuntime {
    pub async fn issue_draft_v2(
        &self,
        key: IssueDraftKey,
    ) -> Result<IssueDraftV2Snapshot, CollaborationError> {
        let _guard = self.acquire_operation()?;
        self.store.issue_draft_v2(key).await
    }
    pub async fn issue_drafts_v2(
        &self,
        q: IssueDraftQuery,
    ) -> Result<IssueDraftV2Page, CollaborationError> {
        let _guard = self.acquire_operation()?;
        self.store.issue_drafts_v2(q).await
    }
    pub async fn save_issue_draft_v2(
        &self,
        r: SaveIssueDraftV2Request,
    ) -> Result<IssueDraftV2Snapshot, CollaborationError> {
        crate::storage::issue_metadata::validate_save(&r)?;
        self.owned_operation(move |runtime| async move {
            let _lifecycle = runtime.lifecycle.lock().await;
            let s = runtime.store.save_issue_draft_v2(r).await?;
            runtime.publish(s.draft.revision.clone());
            Ok(s)
        })
        .await
    }
    pub async fn submit_issue_v2(
        &self,
        r: SubmitIssueV2Request,
    ) -> Result<IssueSubmissionReceipt, CollaborationError> {
        crate::storage::issue_metadata::validate_send(&r)?;
        self.owned_operation(move |runtime| async move {
            let _lifecycle = runtime.lifecycle.lock().await;
            let account = runtime.active_account(&r.context.account_id).await?;
            let instance = ProviderInstance::for_account(&account)?;
            if runtime
                .registry
                .delivery_policy(&instance, "github.create_issue", 2)
                .is_none()
            {
                return Err(unsupported());
            }
            let receipt = runtime.store.submit_issue_v2(r).await?;
            runtime.publish(receipt.admitted_revision.clone());
            runtime.notify.notify_one();
            Ok(receipt)
        })
        .await
    }
    pub async fn issue_metadata_options(
        &self,
        q: IssueMetadataQuery,
    ) -> Result<IssueMetadataPage, CollaborationError> {
        let _guard = self.acquire_operation()?;
        self.store.issue_metadata_options(q).await
    }
    pub async fn refresh_issue_metadata(
        &self,
        r: RefreshIssueMetadataRequest,
    ) -> Result<RefreshReceipt, CollaborationError> {
        crate::issue_creation::native::identifier(&r.account_id)?;
        crate::issue_creation::native::identifier(&r.repository_id)?;
        crate::issue_creation::native::revision(&r.authorization_epoch, true)?;
        self.owned_operation(move |runtime| async move {
            let _lifecycle = runtime.lifecycle.lock().await;
            let a = runtime
                .validate_demand(
                    &r.account_id,
                    &r.authorization_epoch,
                    &DemandTarget {
                        kind: demand_kind(r.kind),
                        repository_id: Some(r.repository_id.clone()),
                        subject_id: None,
                        facet: None,
                    },
                )
                .await?;
            let repo = runtime.store.repository(&a.id, &r.repository_id).await?;
            let id = runtime
                .enqueue_work(
                    a,
                    Some(repo),
                    JobKind::RepositoryMetadata {
                        repository_id: r.repository_id.clone(),
                        kind: r.kind,
                    },
                    scope(&r.repository_id, r.kind),
                    true,
                )
                .await?;
            runtime.notify.notify_one();
            Ok(RefreshReceipt { job_id: id })
        })
        .await
    }
    pub(super) async fn sync_issue_metadata(
        &self,
        job: &mut Job,
        repo: &str,
        kind: IssueMetadataKind,
    ) -> Result<bool, CollaborationError> {
        self.ensure_demand_dispatch(job).await?;
        let account = self.active_account(&job.account.id).await?;
        if account.authorization_epoch != job.account.authorization_epoch {
            return Err(stale());
        }
        self.ensure_provider_budget(&account, job).await?;
        let lease = match self
            .store
            .resume_issue_metadata(&account.id, &account.authorization_epoch, repo, kind)
            .await?
        {
            Some(v) => v,
            None => {
                self.store
                    .begin_issue_metadata(&account.id, &account.authorization_epoch, repo, kind)
                    .await?
            }
        };
        self.publish(self.store.revision().await?);
        let result: Result<bool, CollaborationError> = async {
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
            self.ensure_demand_dispatch(job).await?;
            let request = self.store.issue_metadata_request(&lease).await?;
            self.ensure_provider_budget(&account, job).await?;
            let provider = self.adapter_for_account(&account).await?;
            let fetched = provider.fetch_issue_metadata_catalog(&token, request).await;
            let cooldown = match &fetched {
                Ok(p) => p.cooldown_seconds,
                Err(e) => e.account_cooldown_seconds,
            };
            let budget = if let Some(seconds) = cooldown.filter(|v| *v > 0) {
                self.persist_rate_limit(&account, seconds, None)
                    .await
                    .map(|_| ())
            } else {
                Ok(())
            };
            match fetched {
                Ok(page) => {
                    budget?;
                    let applied = self
                        .store
                        .apply_issue_metadata(&lease, page, &self.now_string())
                        .await?;
                    self.publish(applied.revision);
                    if let (Some(next), Some(seconds)) =
                        (&applied.next_lease, cooldown.filter(|v| *v > 0))
                    {
                        let revision = self
                            .store
                            .fail_issue_metadata(
                                next,
                                SyncStatus {
                                    state: SyncState::RateLimited,
                                    last_success_at: None,
                                    next_retry_at: Some(self.future_string(seconds)),
                                    error: None,
                                },
                            )
                            .await?;
                        self.publish(revision);
                    }
                    Ok(applied.next_lease.is_some())
                }
                Err(error) => Err(error.into()),
            }
        }
        .await;
        if let Err(error) = &result
            && error.code != ErrorCode::StaleView
        {
            let sync = SyncStatus {
                state: match error.code {
                    ErrorCode::AuthRequired => SyncState::AuthRequired,
                    ErrorCode::RateLimited => SyncState::RateLimited,
                    ErrorCode::Network => SyncState::Offline,
                    _ => SyncState::Error,
                },
                last_success_at: None,
                next_retry_at: error
                    .retry_after_seconds
                    .map(|v| self.future_string(u64::from(v))),
                error: Some(error.clone()),
            };
            if let Ok(revision) = self.store.fail_issue_metadata(&lease, sync).await {
                self.publish(revision);
            }
        }
        result
    }
}
