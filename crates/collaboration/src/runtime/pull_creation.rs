use super::*;
use crate::pull_creation::{native as n, *};
impl CollaborationRuntime {
    pub async fn pull_draft(
        &self,
        key: PullDraftKey,
    ) -> Result<PullDraftSnapshot, CollaborationError> {
        let _operation = self.acquire_operation()?;
        self.store.pull_draft(key).await
    }
    pub async fn pull_drafts(
        &self,
        q: PullDraftQuery,
    ) -> Result<PullDraftPage, CollaborationError> {
        let _operation = self.acquire_operation()?;
        self.store.pull_drafts(q).await
    }
    pub async fn save_pull_draft(
        &self,
        r: SavePullDraftRequest,
    ) -> Result<PullDraftSnapshot, CollaborationError> {
        n::validate_save(&r)?;
        self.owned_operation(move |runtime| async move {
            let _lifecycle = runtime.lifecycle.lock().await;
            let snapshot = runtime.store.save_pull_draft(r).await?;
            runtime.publish(snapshot.revision.clone());
            Ok(snapshot)
        })
        .await
    }
    pub async fn preview_pull_creation(
        &self,
        r: PreviewPullCreationRequest,
        local: PullCreationLocalObservation,
        owner: PullCreationOwner,
    ) -> Result<PullCreationPreview, CollaborationError> {
        n::validate_preview(&r)?;
        owner.validate()?;
        let proof = n::LocalProof::from_observation(&local)?;
        self.owned_operation(move |runtime| async move {
            let _dispatch = runtime.dispatch.lock().await;
            let _lifecycle = runtime.lifecycle.lock().await;
            if runtime.is_stopping() {
                return Err(shutdown::stopped());
            }
            let policy = runtime.registry.pull_creation.clone().ok_or_else(|| {
                CollaborationError::new(
                    ErrorCode::Unsupported,
                    "Pull request creation is unavailable",
                )
            })?;
            let (account, frame) = runtime.store.pull_creation_frame(&r, &proof).await?;
            runtime.adapter_for_account(&account).await?;
            runtime.check_provider_budget(&account).await?;
            let reference = runtime
                .store
                .credential_reference(&account.id)
                .await?
                .ok_or_else(|| {
                    CollaborationError::new(
                        ErrorCode::AuthRequired,
                        "Connect this account to preview creation",
                    )
                })?;
            let token = runtime.load_token(&reference).await?.ok_or_else(|| {
                CollaborationError::new(
                    ErrorCode::AuthRequired,
                    "Reconnect this account to preview creation",
                )
            })?;
            runtime.check_provider_budget(&account).await?;
            owner.validate()?;
            let result = tokio::time::timeout(
                Duration::from_secs(30),
                policy.preview(&token, &account, &r.key, &frame, &owner),
            )
            .await
            .map_err(|_| {
                CollaborationError::new(ErrorCode::Network, "Pull request preview timed out")
            })?;
            match result {
                Ok((preview, cooldown)) => {
                    if let Some(seconds) = cooldown {
                        runtime.persist_rate_limit(&account, seconds, None).await?;
                    }
                    runtime
                        .store
                        .validate_pull_creation_frame(&account, &frame, &r.key, || owner.validate())
                        .await?;
                    Ok(preview)
                }
                Err(error) => {
                    runtime.delivery_provider_error(&account, &error).await?;
                    Err(error.into())
                }
            }
        })
        .await
    }
    pub async fn submit_pull(
        &self,
        r: SubmitPullRequest,
        local: Option<PullCreationLocalObservation>,
        owner: PullCreationOwner,
    ) -> Result<PullSubmissionReceipt, CollaborationError> {
        n::validate_send(&r)?;
        owner.validate()?;
        let proof = local
            .as_ref()
            .map(n::LocalProof::from_observation)
            .transpose()?;
        self.owned_operation(move |runtime| async move {
            let _lifecycle = runtime.lifecycle.lock().await;
            let account = runtime.active_account(&r.context.key.account_id).await?;
            let policy = runtime.registry.pull_creation.clone().ok_or_else(|| {
                CollaborationError::new(
                    ErrorCode::Unsupported,
                    "Pull request creation is unavailable",
                )
            })?;
            // A lost IPC response does not require another Git/vault/network preview.
            // Native storage still checks the exact UUID/payload/account/view and current owner.
            match runtime
                .store
                .submit_pull(r.clone(), None, || owner.validate(), || Err(n::invalid()))
                .await
            {
                Ok(receipt) => {
                    runtime.publish(receipt.admitted_revision.clone());
                    return Ok(receipt);
                }
                Err(error) if error.code == ErrorCode::NotReady => {}
                Err(error) => return Err(error),
            }
            let frame = policy.arm(&account, &r, &owner)?;
            // An expired or missing grant can only recover an already durable exact receipt.
            // A live new grant needs a fresh native local observation matching its original proof.
            if let Some(f) = &frame {
                if proof.as_ref() != Some(&f.local) {
                    return Err(CollaborationError::new(
                        ErrorCode::StaleView,
                        "Local branch or repository mapping changed; preview again",
                    ));
                }
                runtime.check_provider_budget(&account).await?;
            }
            let receipt = runtime
                .store
                .submit_pull(
                    r.clone(),
                    frame.as_ref(),
                    || owner.validate(),
                    || {
                        if policy.live(&account, &r) {
                            Ok(())
                        } else {
                            Err(CollaborationError::new(
                                ErrorCode::NotReady,
                                "Creation preview expired; check online again",
                            ))
                        }
                    },
                )
                .await?;
            runtime.publish(receipt.admitted_revision.clone());
            runtime.notify.notify_one();
            Ok(receipt)
        })
        .await
    }
}
