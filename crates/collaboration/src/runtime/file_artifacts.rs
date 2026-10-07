//! Explicit selected diff reads share the native scheduler and quota lane.
use super::*;

fn provider_request(
    selected: crate::storage::PullFileSelection,
) -> Result<PullFileSelectedRequest, CollaborationError> {
    let source = super::pull_files::file_source(&selected.account)?;
    let request = PullFileSelectedRequest {
        resource: PullFileSourceRequest {
            account: selected.account,
            repository: selected.repository,
            subject: selected.subject,
            binding: selected.binding,
            source,
        },
        membership: selected.membership,
        file: selected.file,
    };
    request.validate()?;
    Ok(request)
}
impl CollaborationRuntime {
    pub async fn hydrate_pull_file(
        &self,
        request: PullFileDiffRequest,
    ) -> Result<RefreshReceipt, CollaborationError> {
        let _operation = self.acquire_operation()?;
        request.validate()?;
        let selected = self.store.pull_file_selection(request.clone()).await?;
        self.require_detail(&selected.account, &selected.subject, DetailFacet::Files)
            .await?;
        self.require_detail(&selected.account, &selected.subject, DetailFacet::Body)
            .await?;
        let job_id = self
            .enqueue_work_reason(
                selected.account,
                Some(selected.repository),
                JobKind::PullFileArtifact {
                    request: Box::new(request.clone()),
                },
                DetailFacet::Files.scope(&request.subject_id),
                scheduler::Admission::Manual,
            )
            .await?;
        self.notify.notify_one();
        Ok(RefreshReceipt { job_id })
    }

    async fn selected_provider_error(
        &self,
        account: &RemoteAccount,
        error: ProviderError,
    ) -> Result<bool, CollaborationError> {
        if let Some(seconds) = error.account_cooldown_seconds.filter(|n| *n > 0) {
            self.persist_rate_limit(account, seconds, None).await?;
        }
        Err(error.into())
    }

    pub(super) async fn sync_pull_file_artifact(
        &self,
        job: &mut Job,
        request: PullFileDiffRequest,
    ) -> Result<bool, CollaborationError> {
        self.ensure_demand_dispatch(job).await?;
        let token = {
            let _lifecycle = self.lifecycle.lock().await;
            let selected = self.store.pull_file_selection(request.clone()).await?;
            if selected.account.authorization_epoch != job.account.authorization_epoch {
                return Err(stale());
            }
            self.require_detail(&selected.account, &selected.subject, DetailFacet::Files)
                .await?;
            self.ensure_provider_budget(&selected.account, job).await?;
            let reference = self
                .store
                .credential_reference(&selected.account.id)
                .await?
                .ok_or_else(|| {
                    CollaborationError::new(
                        ErrorCode::AuthRequired,
                        "Reconnect this provider account",
                    )
                })?;
            self.load_token(&reference).await?.ok_or_else(|| {
                CollaborationError::new(ErrorCode::AuthRequired, "Reconnect this provider account")
            })?
        };
        // Selection is resolved again after credential I/O, then separately
        // before parent validation. A detached grant/generation never advances.
        let selected = provider_request(self.store.pull_file_selection(request.clone()).await?)?;
        self.ensure_demand_dispatch(job).await?;
        self.require_detail(
            &selected.resource.account,
            &selected.resource.subject,
            DetailFacet::Files,
        )
        .await?;
        self.ensure_provider_budget(&selected.resource.account, job)
            .await?;
        let adapter = self.adapter_for_account(&selected.resource.account).await?;
        let content = match adapter
            .fetch_pull_file_artifact(&token, selected.clone())
            .await
        {
            Ok(content) => content,
            Err(error) => {
                return self
                    .selected_provider_error(&selected.resource.account, error)
                    .await;
            }
        };
        if let Some(seconds) = content.cooldown_seconds.filter(|n| *n > 0) {
            self.persist_rate_limit(&selected.resource.account, seconds, None)
                .await?;
        }
        let current = provider_request(self.store.pull_file_selection(request.clone()).await?)?;
        if current.membership != selected.membership {
            return Err(stale());
        }
        self.ensure_demand_dispatch(job).await?;
        self.require_detail(
            &current.resource.account,
            &current.resource.subject,
            DetailFacet::Files,
        )
        .await?;
        self.require_detail(
            &current.resource.account,
            &current.resource.subject,
            DetailFacet::Body,
        )
        .await?;
        // Quota exhausted by a successful patch drops those bytes until a new
        // full read can be paired with a separately budgeted fresh parent read.
        self.ensure_provider_budget(&current.resource.account, job)
            .await?;
        let adapter = self.adapter_for_account(&current.resource.account).await?;
        let validation = match adapter
            .validate_selected_pull_file_range(&token, current.clone())
            .await
        {
            Ok(validation) => validation,
            Err(error) => {
                if matches!(
                    error.kind,
                    ProviderErrorKind::InvalidResponse | ProviderErrorKind::Unavailable
                ) {
                    self.request_pull_file_body_context(
                        &current.resource.account,
                        &current.resource.repository,
                        &current.resource.subject,
                    )
                    .await?;
                }
                return self
                    .selected_provider_error(&current.resource.account, error)
                    .await;
            }
        };
        if let Some(seconds) = validation.cooldown_seconds.filter(|n| *n > 0) {
            self.persist_rate_limit(&current.resource.account, seconds, None)
                .await?;
        }
        if !current
            .membership
            .context
            .matches_range_validation(&validation.validation)
        {
            return Err(stale());
        }
        let retained_bytes = content
            .unified_text
            .as_ref()
            .map_or(0, String::len)
            .to_string();
        let artifact = PullFileArtifact {
            account_id: current.membership.account_id.clone(),
            authorization_epoch: current.membership.authorization_epoch.clone(),
            authorization_view: current.membership.authorization_view.clone(),
            subject_id: current.membership.subject_id.clone(),
            generation: current.membership.generation.clone(),
            file_key: current.membership.file_key.clone(),
            identity: current.membership.identity.clone(),
            context: current.membership.context.clone(),
            source: Some(current.resource.source),
            validation: Some(PullFileArtifactValidation::Provider {
                provider_validated_at: self.future_string(0),
            }),
            content_state: content.content_state,
            unified_text: content.unified_text,
            blob_references: PullFileBlobReferences::default(),
            old_blob_oid: None,
            new_blob_oid: None,
            content_type: None,
            binary_hint: content.binary_hint,
            image_hint: PullFileFlag::Unknown,
            last_access_revision: "1".into(),
            logical_bytes: retained_bytes.clone(),
            on_disk_bytes: retained_bytes,
        };
        artifact.validate()?;
        let revision = self
            .store
            .apply_pull_file_artifact(request, current.membership, artifact)
            .await?;
        self.publish(revision);
        self.scheduler.lock().await.failures.remove(&job.key);
        Ok(false)
    }
}
