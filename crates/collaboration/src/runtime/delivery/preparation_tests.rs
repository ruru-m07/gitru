//! Real SQLite/runtime controls for yielding native preparation chains.
use super::*;
use crate::runtime::clock::Clock;

async fn pending(runtime: &CollaborationRuntime) -> usize {
    runtime.scheduler.lock().await.delivery_preparations.len()
}
#[tokio::test]
async fn fifty_four_reads_yield_each_turn_and_consume_one_durable_preparation_budget() {
    let dir = tempfile::tempdir().unwrap();
    let policy = Arc::new(Policy::new(dir.path().join("remote")));
    policy.step_limit.store(54, Ordering::SeqCst);
    let (runtime, account, _) = runtime(&dir.path().join("db"), policy.clone(), true).await;
    admit(&runtime, &account, FIRST, "issue", vec![]).await;
    for step in 0..54 {
        assert!(runtime.run_delivery_next().await.unwrap());
        assert_eq!(policy.step_trace.lock().unwrap().len(), step + 1);
        assert!(runtime.dispatch.try_lock().is_ok());
        assert!(runtime.lifecycle.try_lock().is_ok());
        let saved = state(&runtime, FIRST).await;
        assert_eq!(saved.reconciliation_count, if step == 53 { 0 } else { 1 });
        assert_eq!(saved.attempt_count, if step == 53 { 1 } else { 0 });
        assert_eq!(pending(&runtime).await, usize::from(step != 53));
    }
    assert_eq!(state(&runtime, FIRST).await.state, DeliveryState::Confirmed);
    assert_eq!(policy.calls.load(Ordering::SeqCst), 1);
    assert_eq!(policy.effects(), [FIRST]);
    runtime.shutdown().await.unwrap();
}

struct Foreground(Arc<AtomicUsize>);
#[async_trait]
impl CollaborationProvider for Foreground {
    fn kind(&self) -> ProviderKind {
        ProviderKind::Github
    }
    fn profile(&self, _: &RemoteAccount) -> ProviderProfile {
        ProviderProfile::read_only(InboxSemantics::None, false)
    }
    async fn probe(&self, _: &SecretToken) -> Result<VerifiedAccount, ProviderError> {
        panic!("no probe")
    }
    async fn fetch_page(
        &self,
        _: &SecretToken,
        _: FeedRequest,
    ) -> Result<FetchPage, ProviderError> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Ok(FetchPage {
            repositories: vec![],
            items: vec![],
            endpoint_aliases: vec![],
            notification_subjects: vec![],
            next_cursor: None,
            etag: None,
            last_modified: None,
            not_modified: false,
            poll_interval_seconds: None,
            cooldown_seconds: None,
        })
    }
}
fn registry(policy: Arc<Policy>, read: Arc<dyn CollaborationProvider>) -> ProviderRegistry {
    let mut registry = ProviderRegistry::default();
    registry.register(read).unwrap();
    registry
        .register_delivery(
            &ProviderInstance::public(ProviderKind::Github),
            policy.clone(),
        )
        .unwrap();
    registry
        .register_recovery(&ProviderInstance::public(ProviderKind::Github), policy)
        .unwrap();
    registry
}
async fn peer(runtime: &CollaborationRuntime, account: &RemoteAccount) -> RemoteAccount {
    let b = runtime
        .store
        .upsert_account(RemoteAccount {
            id: "b".into(),
            actor_id: "actor-b".into(),
            login: "b".into(),
            authorization_epoch: "1".into(),
            ..account.clone()
        })
        .await
        .unwrap();
    runtime
        .store
        .stage_credential("b", "fixture-b")
        .await
        .unwrap();
    runtime
        .store
        .commit_account_credential(
            RemoteAccount {
                authorization_epoch: "2".into(),
                ..b
            },
            "fixture-b",
        )
        .await
        .unwrap()
}
#[tokio::test]
async fn foreground_and_peer_account_are_served_between_preparation_steps() {
    let dir = tempfile::tempdir().unwrap();
    let policy = Arc::new(Policy::new(dir.path().join("remote")));
    policy.step_limit.store(3, Ordering::SeqCst);
    let (mut runtime, account, _) = runtime(&dir.path().join("db"), policy.clone(), true).await;
    let reads = Arc::new(AtomicUsize::new(0));
    Arc::get_mut(&mut runtime).unwrap().registry = Arc::new(registry(
        policy.clone(),
        Arc::new(Foreground(reads.clone())),
    ));
    let b = peer(&runtime, &account).await;
    admit(&runtime, &account, FIRST, "issue", vec![]).await;
    admit(&runtime, &b, FIRST, "issue", vec![]).await;
    assert!(runtime.run_delivery_next().await.unwrap());
    runtime
        .refresh(RefreshRequest {
            account_id: "a".into(),
            repository_id: Some("repo".into()),
            kind: Some(RemoteItemKind::Issue),
        })
        .await
        .unwrap();
    assert!(runtime.run_next().await);
    assert_eq!(reads.load(Ordering::SeqCst), 1);
    assert!(runtime.run_delivery_next().await.unwrap());
    assert_eq!(
        *policy.step_trace.lock().unwrap(),
        [("a".into(), 0), ("b".into(), 0)]
    );
    assert_eq!(policy.calls.load(Ordering::SeqCst), 0);
    assert!(runtime.run_delivery_next().await.unwrap());
    assert_eq!(policy.step_trace.lock().unwrap()[2], ("a".into(), 1));
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn quota_on_success_or_error_at_each_step_is_durable_and_drops_partial_authority() {
    for (position, failed) in
        (1..=3).flat_map(|position| [false, true].map(|failed| (position, failed)))
    {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("db");
        let policy = Arc::new(Policy::new(dir.path().join("remote")));
        policy.step_limit.store(3, Ordering::SeqCst);
        let (runtime, account, clock) = runtime(&path, policy.clone(), true).await;
        admit(&runtime, &account, FIRST, "issue", vec![]).await;
        for _ in 1..position {
            assert!(runtime.run_delivery_next().await.unwrap());
        }
        if failed {
            policy.mode.store(10, Ordering::SeqCst)
        } else {
            policy.prepare_cooldown.store(120, Ordering::SeqCst)
        };
        assert!(runtime.run_delivery_next().await.unwrap());
        assert_eq!(pending(&runtime).await, 0);
        assert!(!runtime.run_delivery_next().await.unwrap());
        assert_eq!(policy.step_trace.lock().unwrap().len(), position);
        assert_eq!(state(&runtime, FIRST).await.attempt_count, 0);
        runtime.shutdown().await.unwrap();
        drop(runtime);
        let store = Arc::new(Store::open(&path).await.unwrap());
        let mut reopened = CollaborationRuntime::with_registry(
            store,
            Arc::new(Vault),
            registry(policy.clone(), Arc::new(ReadProvider)),
        );
        reopened.clock = clock.clone();
        assert!(!reopened.run_delivery_next().await.unwrap());
        clock.advance(121);
        policy.mode.store(0, Ordering::SeqCst);
        policy.prepare_cooldown.store(0, Ordering::SeqCst);
        assert!(reopened.run_delivery_next().await.unwrap());
        assert_eq!(
            policy.step_trace.lock().unwrap().last().unwrap().1,
            0,
            "no cached continuation survives quota or restart"
        );
        assert_eq!(state(&reopened, FIRST).await.attempt_count, 0);
        reopened.shutdown().await.unwrap();
    }
}
#[tokio::test]
async fn stale_native_context_view_or_epoch_refuses_the_next_read() {
    for changed in ["context", "view", "epoch"] {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("db");
        let policy = Arc::new(Policy::new(dir.path().join("remote")));
        policy.step_limit.store(2, Ordering::SeqCst);
        let (runtime, account, _) = runtime(&path, policy.clone(), true).await;
        admit(&runtime, &account, FIRST, "issue", vec![]).await;
        assert!(runtime.run_delivery_next().await.unwrap());
        match changed {
            "context" => {
                policy.context_version.store(1, Ordering::SeqCst);
            }
            "view" => {
                peer(&runtime, &account).await;
            }
            _ => {
                runtime
                    .store
                    .upsert_account(RemoteAccount {
                        authorization_epoch: "3".into(),
                        ..account
                    })
                    .await
                    .unwrap();
            }
        }
        let _ = runtime.run_delivery_next().await;
        assert_eq!(policy.step_trace.lock().unwrap().len(), 1, "{changed}");
        assert_eq!(pending(&runtime).await, 0);
        assert_eq!(state(&runtime, FIRST).await.attempt_count, 0);
        assert_eq!(policy.calls.load(Ordering::SeqCst), 0);
        runtime.shutdown().await.unwrap();
    }
}
#[tokio::test]
async fn cancellation_discards_pending_preparation_and_never_dispatches() {
    {
        let action = crate::CommandRecoveryAction::Cancel;
        let dir = tempfile::tempdir().unwrap();
        let policy = Arc::new(Policy::new(dir.path().join("remote")));
        policy.step_limit.store(2, Ordering::SeqCst);
        let (runtime, account, _) = runtime(&dir.path().join("db"), policy.clone(), true).await;
        admit(&runtime, &account, FIRST, "issue", vec![]).await;
        runtime.run_delivery_next().await.unwrap();
        let detail = runtime.command_recovery_detail("a", FIRST).await.unwrap();
        runtime
            .command_recovery_action(crate::CommandRecoveryActionRequest {
                context: detail.context,
                action_id: uuid::Uuid::new_v4().to_string(),
                action,
            })
            .await
            .unwrap();
        assert_eq!(pending(&runtime).await, 0);
        assert!(!runtime.run_delivery_next().await.unwrap());
        assert_eq!(policy.step_trace.lock().unwrap().len(), 1);
        assert_eq!(policy.calls.load(Ordering::SeqCst), 0);
        runtime.shutdown().await.unwrap();
    }
}
#[tokio::test]
async fn expired_cycle_oversized_and_excessive_chains_cannot_record_attempts() {
    for failure in ["expiry", "cycle", "oversized", "steps"] {
        let dir = tempfile::tempdir().unwrap();
        let policy = Arc::new(Policy::new(dir.path().join("remote")));
        policy.step_limit.store(65, Ordering::SeqCst);
        let (runtime, account, clock) = runtime(&dir.path().join("db"), policy.clone(), true).await;
        admit(&runtime, &account, FIRST, "issue", vec![]).await;
        if failure == "cycle" {
            policy.step_cycle.store(true, Ordering::SeqCst)
        }
        if failure == "oversized" {
            policy.step_oversized.store(true, Ordering::SeqCst)
        }
        assert!(runtime.run_delivery_next().await.unwrap());
        match failure {
            "expiry" => {
                clock.advance(120);
                assert!(runtime.run_delivery_next().await.unwrap());
            }
            "cycle" => {
                assert!(runtime.run_delivery_next().await.unwrap());
            }
            "steps" => {
                for _ in 1..64 {
                    assert!(runtime.run_delivery_next().await.unwrap());
                }
            }
            _ => {}
        }
        assert_eq!(pending(&runtime).await, 0, "{failure}");
        assert!(
            !runtime.run_delivery_next().await.unwrap(),
            "bounded defer after {failure}"
        );
        assert_eq!(state(&runtime, FIRST).await.attempt_count, 0);
        assert_eq!(state(&runtime, FIRST).await.reconciliation_count, 1);
        assert_eq!(policy.calls.load(Ordering::SeqCst), 0);
        runtime.shutdown().await.unwrap();
    }
}
#[tokio::test]
async fn held_final_read_cannot_dispatch_after_expiry_or_shutdown_but_retains_quota() {
    for stop in [false, true] {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("db");
        let policy = Arc::new(Policy::new(dir.path().join("remote")));
        policy.step_limit.store(2, Ordering::SeqCst);
        let (runtime, account, clock) = runtime(&path, policy.clone(), true).await;
        admit(&runtime, &account, FIRST, "issue", vec![]).await;
        runtime.run_delivery_next().await.unwrap();
        policy.prepare_hold.store(true, Ordering::SeqCst);
        let turn = tokio::spawn({
            let r = runtime.clone();
            async move { r.run_delivery_next().await }
        });
        tokio::time::timeout(Duration::from_secs(10), policy.prepare_entered.notified())
            .await
            .unwrap();
        let shutdown = if stop {
            Some(tokio::spawn({
                let r = runtime.clone();
                async move { r.shutdown().await }
            }))
        } else {
            None
        };
        if stop {
            while !runtime.is_stopping() {
                tokio::task::yield_now().await;
            }
        } else {
            clock.advance(120);
        }
        policy.prepare_cooldown.store(300, Ordering::SeqCst);
        policy.prepare_release.notify_one();
        turn.await.unwrap().unwrap();
        if let Some(task) = shutdown {
            task.await.unwrap().unwrap();
        } else {
            runtime.shutdown().await.unwrap();
        }
        assert_eq!(policy.calls.load(Ordering::SeqCst), 0);
        let store = Store::open(&path).await.unwrap();
        assert_eq!(
            store
                .delivery_command("a", FIRST)
                .await
                .unwrap()
                .attempt_count,
            0
        );
        assert!(
            store
                .scope_state(&account.id, "provider:rest")
                .await
                .unwrap()
                .is_some()
        );
        store.close().await.unwrap();
    }
}

struct HeldVault {
    entered: Notify,
    gate: (StdMutex<bool>, std::sync::Condvar),
}
impl HeldVault {
    fn release(&self) {
        *self.gate.0.lock().unwrap() = true;
        self.gate.1.notify_all();
    }
}
impl CredentialVault for HeldVault {
    fn load(&self, _: &str) -> Result<Option<SecretToken>, CredentialError> {
        self.entered.notify_one();
        let mut open = self.gate.0.lock().unwrap();
        while !*open {
            open = self.gate.1.wait(open).unwrap();
        }
        Ok(Some(SecretToken::new("synthetic_delivery".into()).unwrap()))
    }
    fn store(&self, _: &str, _: &SecretToken) -> Result<(), CredentialError> {
        Ok(())
    }
    fn delete(&self, _: &str) -> Result<(), CredentialError> {
        Ok(())
    }
}
#[tokio::test]
async fn held_vault_quota_or_expiry_refuses_the_provider_read() {
    for quota in [false, true] {
        let dir = tempfile::tempdir().unwrap();
        let policy = Arc::new(Policy::new(dir.path().join("remote")));
        policy.step_limit.store(2, Ordering::SeqCst);
        let (mut runtime, account, clock) =
            runtime(&dir.path().join("db"), policy.clone(), true).await;
        let vault = Arc::new(HeldVault {
            entered: Notify::new(),
            gate: (StdMutex::new(false), std::sync::Condvar::new()),
        });
        Arc::get_mut(&mut runtime).unwrap().vault = vault.clone();
        admit(&runtime, &account, FIRST, "issue", vec![]).await;
        let turn = tokio::spawn({
            let r = runtime.clone();
            async move { r.run_delivery_next().await }
        });
        tokio::time::timeout(Duration::from_secs(10), vault.entered.notified())
            .await
            .unwrap();
        if quota {
            runtime
                .persist_rate_limit(&account, 300, None)
                .await
                .unwrap();
        } else {
            clock.advance(120);
        }
        vault.release();
        turn.await.unwrap().unwrap();
        assert!(policy.step_trace.lock().unwrap().is_empty());
        assert_eq!(state(&runtime, FIRST).await.attempt_count, 0);
        assert_eq!(policy.calls.load(Ordering::SeqCst), 0);
        runtime.shutdown().await.unwrap();
    }
}

#[tokio::test]
async fn chain_entries_are_globally_bounded_and_account_reset_retires_them() {
    let dir = tempfile::tempdir().unwrap();
    let policy = Arc::new(Policy::new(dir.path().join("remote")));
    policy.step_limit.store(3, Ordering::SeqCst);
    let (runtime, account, _) = runtime(&dir.path().join("db"), policy.clone(), true).await;
    for index in 0..9 {
        let id = format!("123e4567-e89b-12d3-a456-42661417400{index}");
        admit(&runtime, &account, &id, &format!("target-{index}"), vec![]).await;
    }
    for index in 0..9 {
        assert!(runtime.run_delivery_next().await.unwrap());
        assert_eq!(pending(&runtime).await, (index + 1).min(8));
    }
    let ninth = runtime
        .store
        .delivery_command("a", "123e4567-e89b-12d3-a456-426614174008")
        .await
        .unwrap();
    assert!(ninth.next_action_at.is_some());
    assert_eq!(ninth.attempt_count, 0);
    assert_eq!(policy.calls.load(Ordering::SeqCst), 0);
    runtime.disconnect("a").await.unwrap();
    assert_eq!(pending(&runtime).await, 0);
    assert!(!runtime.run_delivery_next().await.unwrap());
    runtime.shutdown().await.unwrap();
}
#[tokio::test]
async fn cold_restart_repeats_fresh_read_zero_and_never_reuses_partial_permission() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("db");
    let policy = Arc::new(Policy::new(dir.path().join("remote")));
    policy.step_limit.store(2, Ordering::SeqCst);
    let (before, account, _) = runtime(&path, policy.clone(), true).await;
    admit(&before, &account, FIRST, "issue", vec![]).await;
    before.run_delivery_next().await.unwrap();
    before.shutdown().await.unwrap();
    drop(before);
    let store = Arc::new(Store::open(&path).await.unwrap());
    let after = CollaborationRuntime::with_registry(
        store,
        Arc::new(Vault),
        registry(policy.clone(), Arc::new(ReadProvider)),
    );
    assert!(after.run_delivery_next().await.unwrap());
    assert_eq!(
        *policy.step_trace.lock().unwrap(),
        [("a".into(), 0), ("a".into(), 0)]
    );
    assert_eq!(state(&after, FIRST).await.reconciliation_count, 2);
    assert_eq!(state(&after, FIRST).await.attempt_count, 0);
    assert!(after.run_delivery_next().await.unwrap());
    assert_eq!(policy.calls.load(Ordering::SeqCst), 1);
    after.shutdown().await.unwrap();
}
#[tokio::test]
async fn authentication_on_a_continuation_preserves_quota_and_prevents_next_read() {
    let dir = tempfile::tempdir().unwrap();
    let policy = Arc::new(Policy::new(dir.path().join("remote")));
    policy.step_limit.store(3, Ordering::SeqCst);
    let (runtime, account, _) = runtime(&dir.path().join("db"), policy.clone(), true).await;
    admit(&runtime, &account, FIRST, "issue", vec![]).await;
    runtime.run_delivery_next().await.unwrap();
    policy.mode.store(12, Ordering::SeqCst);
    runtime.run_delivery_next().await.unwrap();
    assert_eq!(
        runtime.store.account("a").await.unwrap().state,
        AccountState::AuthRequired
    );
    let mut db = SqliteConnection::connect_with(
        &sqlx::sqlite::SqliteConnectOptions::new().filename(dir.path().join("db")),
    )
    .await
    .unwrap();
    let json: String = sqlx::query_scalar(
        "SELECT sync_json FROM sync_scopes WHERE account_id='a' AND scope='provider:rest'",
    )
    .fetch_one(&mut db)
    .await
    .unwrap();
    assert!(
        serde_json::from_str::<SyncStatus>(&json)
            .unwrap()
            .next_retry_at
            .is_some()
    );
    db.close().await.unwrap();
    assert_eq!(pending(&runtime).await, 0);
    assert!(!runtime.run_delivery_next().await.unwrap());
    assert_eq!(policy.calls.load(Ordering::SeqCst), 0);
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn blocked_final_writer_rechecks_same_epoch_view_context_and_monotonic_deadline() {
    for change in ["none", "view", "context", "expiry"] {
        let dir = tempfile::tempdir().unwrap();
        let policy = Arc::new(Policy::new(dir.path().join("remote")));
        policy.step_limit.store(2, Ordering::SeqCst);
        let (runtime, account, clock) = runtime(&dir.path().join("db"), policy.clone(), true).await;
        admit(&runtime, &account, FIRST, "issue", vec![]).await;
        runtime.run_delivery_next().await.unwrap();
        let command = state(&runtime, FIRST).await;
        let request = ReconcileRequest {
            command,
            account,
            instance_id: ProviderInstance::public(ProviderKind::Github).id,
            native_context: 0u64.to_be_bytes().to_vec(),
        };
        let start = clock.now();
        let live = Arc::new({
            let clock = clock.clone();
            move || clock.now().duration_since(start) < Duration::from_secs(120)
        });
        let result = crate::storage::delivery::blocked_claim_test(
            &runtime.store,
            request,
            policy.clone(),
            change == "view",
            live,
            || {
                if change == "context" {
                    policy.context_version.store(1, Ordering::SeqCst)
                }
                if change == "expiry" {
                    clock.advance(120)
                }
            },
        )
        .await;
        if change == "none" {
            assert!(result.unwrap());
            assert_eq!(
                state(&runtime, FIRST).await.attempt_count,
                1,
                "valid writer-held authority can claim"
            );
        } else {
            assert_eq!(result.unwrap_err().code, ErrorCode::StaleView, "{change}");
            assert_eq!(state(&runtime, FIRST).await.attempt_count, 0, "{change}");
        }
        assert_eq!(
            policy.calls.load(Ordering::SeqCst),
            0,
            "claim helper never dispatches"
        );
        runtime.shutdown().await.unwrap();
    }
}
