//! Revalidate process-local preparation authority without consuming another read-chain budget.
use super::*;
impl Store {
    pub(crate) async fn recheck_preparation(
        &self,
        expected: &DeliveryCommand,
        account: &RemoteAccount,
        policy: &dyn CommandDeliveryPolicy,
        context: &[u8],
        authorization_view: Option<&str>,
        now: &DeliveryTime,
    ) -> Result<Option<String>> {
        if context.len() > MAX_EVIDENCE_BYTES {
            return Err(stale());
        }
        let mut writer = self.inner.writer.acquire().await?;
        let mut tx = writer.begin().await.map_err(storage_error)?;
        let command = exact_in(&mut tx, expected).await?;
        policy_matches(&command, policy)?;
        authorize_in(&mut tx, &command, account, now).await?;
        let (_, view) = metadata(&mut tx).await?;
        if authorization_view.is_some_and(|expected| expected != view)
            || command.authorization_epoch != account.authorization_epoch
            || command.reconcile_only()
            || command.attention.is_some()
            || !matches!(
                command.state,
                DeliveryState::Queued | DeliveryState::RetryWait
            )
            || age_exceeded(&command.admitted_at, &now.command_now)?
            || command.attempt_count >= MAX_ATTEMPTS
            || evidence_full(&command)
            || !policy
                .preparation_context_matches_in(&mut tx, &command, account, context)
                .await?
        {
            return Err(stale());
        }
        if blocked_in(&mut tx, &command).await? {
            return Ok(None);
        }
        tx.commit().await.map_err(storage_error)?;
        Ok(Some(view))
    }
}

/// Process-local preparation authority is checked while the claim writer is held.
/// The callback contains only native lifecycle/monotonic checks, never I/O.
pub(crate) struct PreparationAuthority<'a> {
    pub view: &'a str,
    pub context: &'a [u8],
    pub live: &'a (dyn Fn() -> bool + Send + Sync),
}
impl PreparationAuthority<'_> {
    pub(super) async fn validate_in(
        &self,
        tx: &mut Transaction<'_, Sqlite>,
        command: &DeliveryCommand,
        account: &RemoteAccount,
        policy: &dyn CommandDeliveryPolicy,
    ) -> Result<()> {
        if self.context.len() > MAX_EVIDENCE_BYTES
            || !(self.live)()
            || metadata(tx).await?.1 != self.view
            || !policy
                .preparation_context_matches_in(tx, command, account, self.context)
                .await?
            || !(self.live)()
        {
            return Err(stale());
        }
        Ok(())
    }
}

#[cfg(test)]
pub(crate) async fn blocked_claim_test(
    store: &Store,
    request: ReconcileRequest,
    policy: std::sync::Arc<dyn CommandDeliveryPolicy>,
    change_view: bool,
    live: std::sync::Arc<dyn Fn() -> bool + Send + Sync>,
    mutate: impl FnOnce(),
) -> Result<bool> {
    let now = chrono::Utc::now().to_rfc3339();
    let now = DeliveryTime {
        command_now: now.clone(),
        now,
    };
    let view = store
        .recheck_preparation(
            &request.command,
            &request.account,
            policy.as_ref(),
            &request.native_context,
            None,
            &now,
        )
        .await?
        .unwrap();
    let mut writer = store.inner.writer.acquire().await?;
    let entered = std::sync::Arc::new(tokio::sync::Notify::new());
    let task = tokio::spawn({
        let store = store.clone();
        let entered = entered.clone();
        async move {
            entered.notify_one();
            store
                .claim_delivery_guarded(
                    &request.command,
                    &request.account,
                    policy.as_ref(),
                    &[1, 2, 3],
                    &now,
                    Some(PreparationAuthority {
                        view: &view,
                        context: &request.native_context,
                        live: live.as_ref(),
                    }),
                )
                .await
        }
    });
    entered.notified().await;
    tokio::task::yield_now().await;
    assert!(
        !task.is_finished(),
        "final claim is held behind the actual writer"
    );
    if change_view {
        sqlx::query(
            "UPDATE runtime_meta SET authorization_view=authorization_view+1 WHERE singleton=1",
        )
        .execute(&mut *writer)
        .await
        .unwrap();
    }
    mutate();
    drop(writer);
    task.await.unwrap().map(|claim| claim.request.is_some())
}
