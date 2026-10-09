use super::*;
use crate::guarded_merge::native::{validate_query, validate_request};
impl CollaborationRuntime {
    pub async fn guarded_merge_snapshot(
        &self,
        q: GuardedMergeQuery,
    ) -> Result<GuardedMergeSnapshot, CollaborationError> {
        let _operation = self.acquire_operation()?;
        let mut s = self.store.guarded_merge_snapshot(q).await?;
        if self.registry.guarded_merge.is_none() {
            s.reason = Some(MergeUnavailableReason::UnsupportedProvider);
        }
        Ok(s)
    }
    pub async fn preview_guarded_merge(
        &self,
        q: GuardedMergeQuery,
    ) -> Result<GuardedMergePreview, CollaborationError> {
        validate_query(&q)?;
        self.owned_operation(move |runtime| async move {
            let _dispatch = runtime.dispatch.lock().await;
            let _lifecycle = runtime.lifecycle.lock().await;
            if runtime.is_stopping() {
                return Err(shutdown::stopped());
            }
            let policy = runtime.registry.guarded_merge.clone().ok_or_else(|| {
                CollaborationError::new(ErrorCode::Unsupported, "Guarded merge is unavailable")
            })?;
            let (account, frame) = runtime.store.guarded_merge_frame(&q).await?;
            runtime.adapter_for_account(&account).await?;
            runtime.check_provider_budget(&account).await?;
            let reference = runtime
                .store
                .credential_reference(&account.id)
                .await?
                .ok_or_else(|| {
                    CollaborationError::new(
                        ErrorCode::AuthRequired,
                        "Connect this account to preview merge",
                    )
                })?;
            let token = runtime.load_token(&reference).await?.ok_or_else(|| {
                CollaborationError::new(
                    ErrorCode::AuthRequired,
                    "Reconnect this account to preview merge",
                )
            })?;
            // A credential probe can publish a provider cooldown while the
            // native vault is pending. Recheck immediately before provider I/O.
            runtime.check_provider_budget(&account).await?;
            let result = tokio::time::timeout(
                Duration::from_secs(30),
                policy.preview(&token, &account, &frame),
            )
            .await
            .map_err(|_| CollaborationError::new(ErrorCode::Network, "Merge preview timed out"))?;
            match result {
                Ok((preview, cooldown)) => {
                    if let Some(seconds) = cooldown {
                        runtime.persist_rate_limit(&account, seconds, None).await?;
                    }
                    runtime.store.validate_merge_frame(&account, &frame).await?;
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
    pub async fn submit_guarded_merge(
        &self,
        r: GuardedMergeRequest,
    ) -> Result<GuardedMergeReceipt, CollaborationError> {
        validate_request(&r)?;
        self.owned_operation(move |runtime| async move {
            let _lifecycle = runtime.lifecycle.lock().await;
            let account = runtime.active_account(&r.context.account_id).await?;
            let policy = runtime.registry.guarded_merge.clone().ok_or_else(|| {
                CollaborationError::new(ErrorCode::Unsupported, "Guarded merge is unavailable")
            })?;
            let frame = policy.arm(&account, &r)?;
            if frame.is_some() {
                runtime.check_provider_budget(&account).await?;
            }
            let receipt = runtime
                .store
                .submit_guarded_merge(r.clone(), frame.as_ref(), || {
                    if policy.live(&account, &r) {
                        Ok(())
                    } else {
                        Err(CollaborationError::new(
                            ErrorCode::NotReady,
                            "Merge preview expired; check online again",
                        ))
                    }
                })
                .await?;
            runtime.publish(receipt.admitted_revision.clone());
            runtime.notify.notify_one();
            Ok(receipt)
        })
        .await
    }
}
