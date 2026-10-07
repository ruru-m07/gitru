use super::*;
use crate::commands::{
    CanonicalFields, CommandDraft, CommandPayloadCodec, CommandSubmission, CommandTarget,
    CommandTargetKind, seal_command,
};
use crate::credentials::CredentialError;
use crate::storage::command_admission::{CommandAdmissionPolicy, CommandProtection};
use async_trait::async_trait;
use sqlx::{Connection, Sqlite, SqliteConnection, Transaction};
use std::{
    path::{Path, PathBuf},
    sync::{
        Mutex as StdMutex,
        atomic::{AtomicBool, AtomicU8, AtomicU64, AtomicUsize},
    },
};
const FIRST: &str = "123e4567-e89b-12d3-a456-426614174000";
const SECOND: &str = "123e4567-e89b-12d3-a456-426614174001";
#[derive(Default)]
struct Vault;
impl CredentialVault for Vault {
    fn store(&self, _: &str, _: &SecretToken) -> Result<(), CredentialError> {
        Ok(())
    }
    fn load(&self, _: &str) -> Result<Option<SecretToken>, CredentialError> {
        Ok(Some(SecretToken::new("synthetic_delivery".into()).unwrap()))
    }
    fn delete(&self, _: &str) -> Result<(), CredentialError> {
        Ok(())
    }
}
struct ReadProvider;
#[async_trait]
impl CollaborationProvider for ReadProvider {
    fn kind(&self) -> ProviderKind {
        ProviderKind::Github
    }
    async fn probe(&self, _: &SecretToken) -> Result<VerifiedAccount, ProviderError> {
        panic!("No live probe")
    }
    async fn fetch_page(
        &self,
        _: &SecretToken,
        _: FeedRequest,
    ) -> Result<FetchPage, ProviderError> {
        panic!("No live feed")
    }
}
struct Payload;
impl CommandPayloadCodec for Payload {
    const OPERATION_KIND: &'static str = "fixture.create";
    const PAYLOAD_VERSION: u32 = 1;
    fn encode_payload(&self, fields: &mut CanonicalFields) -> Result<(), CollaborationError> {
        fields.string(1, "authored message")
    }
}
struct Admission;
#[async_trait]
impl CommandAdmissionPolicy for Admission {
    const OPERATION_KIND: &'static str = Payload::OPERATION_KIND;
    const PAYLOAD_VERSION: u32 = 1;
    async fn validate(
        &self,
        _: &mut Transaction<'_, Sqlite>,
        _: &RemoteAccount,
        _: &CommandSubmission,
    ) -> Result<Vec<CommandProtection>, CollaborationError> {
        Ok(vec![])
    }
}
struct TestClock {
    start: Instant,
    utc: DateTime<Utc>,
    elapsed: StdMutex<u64>,
    wall: StdMutex<i64>,
}
impl TestClock {
    fn new() -> Self {
        Self {
            start: Instant::now(),
            utc: Utc::now(),
            elapsed: StdMutex::new(0),
            wall: StdMutex::new(0),
        }
    }
    fn advance(&self, seconds: u64) {
        *self.elapsed.lock().unwrap() += seconds;
        *self.wall.lock().unwrap() += seconds as i64;
    }
}
impl clock::Clock for TestClock {
    fn now(&self) -> Instant {
        self.start + Duration::from_secs(*self.elapsed.lock().unwrap())
    }
    fn utc(&self) -> DateTime<Utc> {
        self.utc + chrono::Duration::seconds(*self.wall.lock().unwrap())
    }
}
struct Policy {
    remote: PathBuf,
    mode: AtomicU8,
    calls: AtomicUsize,
    probes: AtomicUsize,
    entered: Notify,
    hold: AtomicBool,
    release: Notify,
    prepare_hold: AtomicBool,
    prepare_cooldown: AtomicU64,
    result_cooldown: AtomicU64,
    prepare_entered: Notify,
    prepare_release: Notify,
    guard: AtomicBool,
    finalize_fail: AtomicBool,
}
impl Policy {
    fn new(remote: PathBuf) -> Self {
        Self {
            remote,
            mode: AtomicU8::new(0),
            calls: AtomicUsize::new(0),
            probes: AtomicUsize::new(0),
            entered: Notify::new(),
            hold: AtomicBool::new(false),
            release: Notify::new(),
            prepare_hold: AtomicBool::new(false),
            prepare_cooldown: AtomicU64::new(0),
            result_cooldown: AtomicU64::new(0),
            prepare_entered: Notify::new(),
            prepare_release: Notify::new(),
            guard: AtomicBool::new(true),
            finalize_fail: AtomicBool::new(false),
        }
    }
    fn proof(&self, command: &DeliveryCommand, kind: &str) -> OperationEvidence {
        OperationEvidence {
            kind: kind.into(),
            version: 1,
            payload: [command.command_id.as_bytes(), command.hash.as_slice()].concat(),
        }
    }
    fn report(&self, command: &DeliveryCommand, kind: &str) -> DeliveryReport {
        let proof = self.proof(command, kind);
        DeliveryReport {
            outcome: match kind {
                "fixture.confirmed" => DeliveryOutcome::Confirmed(proof),
                "fixture.accepted" => DeliveryOutcome::Accepted(proof),
                "fixture.rejected" => DeliveryOutcome::Rejected(proof),
                "fixture.conflict" => DeliveryOutcome::Conflict(proof),
                "fixture.safe" => DeliveryOutcome::SafeRetry(proof),
                _ => DeliveryOutcome::Unknown,
            },
            retry_after_seconds: Some(1),
            account_cooldown_seconds: match self.result_cooldown.load(Ordering::SeqCst) {
                0 => None,
                value => Some(value),
            },
            provider_error: None,
        }
    }
    fn quota_error(&self) -> ProviderError {
        ProviderError {
            kind: ProviderErrorKind::RateLimited,
            retry_after_seconds: Some(1),
            account_cooldown_seconds: Some(120),
        }
    }
    fn receipt_key(command: &DeliveryCommand) -> String {
        let hash = command
            .hash
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>();
        format!("{}|{}|{}", command.account_id, command.command_id, hash)
    }
    fn exists(&self, command: &DeliveryCommand) -> bool {
        std::fs::read_to_string(&self.remote)
            .unwrap_or_default()
            .lines()
            .any(|line| line == Self::receipt_key(command))
    }
    fn effects(&self) -> Vec<String> {
        std::fs::read_to_string(&self.remote)
            .unwrap_or_default()
            .lines()
            .filter_map(|line| line.split('|').nth(1).map(str::to_owned))
            .collect()
    }
    fn effect(&self, command: &DeliveryCommand) {
        use std::io::Write;
        let mut f = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.remote)
            .unwrap();
        writeln!(f, "{}", Self::receipt_key(command)).unwrap();
        f.sync_all().unwrap();
    }
}
#[async_trait]
impl CommandDeliveryPolicy for Policy {
    fn operation_kind(&self) -> &'static str {
        Payload::OPERATION_KIND
    }
    fn payload_version(&self) -> u32 {
        1
    }
    async fn prepare(
        &self,
        _: &SecretToken,
        _: &ReconcileRequest,
    ) -> Result<DeliveryPreparation, ProviderError> {
        if self.prepare_hold.load(Ordering::SeqCst) {
            self.prepare_entered.notify_one();
            self.prepare_release.notified().await;
        }
        if self.mode.load(Ordering::SeqCst) == 10 {
            return Err(self.quota_error());
        }
        if self.mode.load(Ordering::SeqCst) == 9 {
            return Err(ProviderError::new(ProviderErrorKind::Offline));
        }
        Ok(DeliveryPreparation {
            bytes: vec![1, 2, 3],
            account_cooldown_seconds: match self.prepare_cooldown.load(Ordering::SeqCst) {
                0 => None,
                value => Some(value),
            },
        })
    }
    async fn validate_claim(
        &self,
        tx: &mut Transaction<'_, Sqlite>,
        command: &DeliveryCommand,
        _: &RemoteAccount,
        preparation: &[u8],
    ) -> Result<ClaimDecision, CollaborationError> {
        assert_eq!(preparation, [1, 2, 3]);
        assert_eq!(command.repository_id.as_deref(), Some("repo"));
        assert!(
            command
                .canonical_envelope
                .windows(command.payload.len())
                .any(|w| w == command.payload)
        );
        assert!(command.guards.is_empty());
        let pending:i64=sqlx::query_scalar("SELECT count(*) FROM delivery_attempts WHERE account_id=? AND command_id=? AND outcome='started'").bind(&command.account_id).bind(&command.command_id).fetch_one(&mut **tx).await.unwrap();
        assert_eq!(pending, 0);
        if !self.guard.load(Ordering::SeqCst) {
            return Ok(ClaimDecision::Conflict(
                self.proof(command, "fixture.conflict"),
            ));
        }
        Ok(ClaimDecision::Ready(b"separate execution base".to_vec()))
    }
    fn validate_evidence(
        &self,
        command: &DeliveryCommand,
        purpose: EvidencePurpose,
        evidence: &OperationEvidence,
    ) -> bool {
        let kind = match purpose {
            EvidencePurpose::Confirmed => "fixture.confirmed",
            EvidencePurpose::Accepted => "fixture.accepted",
            EvidencePurpose::Rejected => "fixture.rejected",
            EvidencePurpose::Conflict => "fixture.conflict",
            EvidencePurpose::SafeRetry => "fixture.safe",
        };
        evidence == &self.proof(command, kind)
            && (purpose != EvidencePurpose::Confirmed || self.exists(command))
    }
    async fn finalize_in(
        &self,
        context: &mut crate::storage::effective::finalization::DeliveryFinalization<'_, '_>,
        command: &DeliveryCommand,
        purpose: EvidencePurpose,
        _: &OperationEvidence,
    ) -> Result<(), CollaborationError> {
        let tx = context.transaction();
        if self.finalize_fail.load(Ordering::SeqCst) {
            return Err(CollaborationError::storage());
        }
        if purpose == EvidencePurpose::Confirmed {
            sqlx::query(
                "UPDATE items SET json=json_set(json,'$.body',?) WHERE account_id=? AND id='pull'",
            )
            .bind(&command.command_id)
            .bind(&command.account_id)
            .execute(&mut **tx)
            .await
            .map_err(|_| CollaborationError::storage())?;
        }
        Ok(())
    }
    async fn dispatch(&self, _: &SecretToken, request: DispatchRequest) -> DeliveryReport {
        assert_eq!(request.account.id, request.command.account_id);
        assert_eq!(
            request.instance_id,
            ProviderInstance::for_account(&request.account).unwrap().id
        );
        self.calls.fetch_add(1, Ordering::SeqCst);
        assert_eq!(request.execution_base, b"separate execution base");
        assert_eq!(request.command.state, DeliveryState::Sending);
        let mode = self.mode.load(Ordering::SeqCst);
        self.entered.notify_one();
        if self.hold.load(Ordering::SeqCst) {
            self.release.notified().await;
        }
        if matches!(mode, 0 | 1 | 2 | 6 | 7 | 8) {
            self.effect(&request.command);
        }
        if std::env::var("GITRU_DELIVERY_CRASH").ok().as_deref() == Some("after_remote_effect") {
            std::process::exit(91);
        }
        match mode {
            0 => self.report(&request.command, "fixture.confirmed"),
            1 => self.report(&request.command, "fixture.accepted"),
            2 => DeliveryReport::unknown(),
            3 => self.report(&request.command, "fixture.safe"),
            4 => self.report(&request.command, "fixture.conflict"),
            5 => self.report(&request.command, "fixture.rejected"),
            6 => DeliveryReport {
                outcome: DeliveryOutcome::Confirmed(OperationEvidence::native("forged.2xx")),
                retry_after_seconds: None,
                account_cooldown_seconds: None,
                provider_error: None,
            },
            7 => panic!("synthetic adapter panicked after remote effect"),
            8 => std::future::pending().await,
            11 => DeliveryReport {
                provider_error: Some(ProviderError {
                    kind: ProviderErrorKind::Authentication,
                    retry_after_seconds: None,
                    account_cooldown_seconds: match self.result_cooldown.load(Ordering::SeqCst) {
                        0 => None,
                        value => Some(value),
                    },
                }),
                ..DeliveryReport::unknown()
            },
            _ => panic!("unexpected mode"),
        }
    }
    async fn reconcile(
        &self,
        _: &SecretToken,
        request: ReconcileRequest,
    ) -> Result<DeliveryReport, ProviderError> {
        assert_eq!(request.account.id, request.command.account_id);
        assert_eq!(
            request.instance_id,
            ProviderInstance::for_account(&request.account).unwrap().id
        );
        self.probes.fetch_add(1, Ordering::SeqCst);
        if self.mode.load(Ordering::SeqCst) == 10 {
            return Err(self.quota_error());
        }
        if self.mode.load(Ordering::SeqCst) == 3 {
            return Ok(self.report(&request.command, "fixture.safe"));
        }
        if self.mode.load(Ordering::SeqCst) == 5 {
            return Ok(self.report(&request.command, "fixture.rejected"));
        }
        if self.exists(&request.command) {
            Ok(self.report(&request.command, "fixture.confirmed"))
        } else {
            Ok(DeliveryReport::unknown())
        }
    }
}
async fn runtime(
    path: &Path,
    policy: Arc<Policy>,
    register: bool,
) -> (Arc<CollaborationRuntime>, RemoteAccount, Arc<TestClock>) {
    let store = Arc::new(Store::open(path).await.unwrap());
    let account = match store.account("a").await {
        Ok(a) => a,
        Err(_) => store
            .upsert_account(RemoteAccount {
                id: "a".into(),
                provider: ProviderKind::Github,
                host: "github.com".into(),
                actor_id: "actor-a".into(),
                login: "a".into(),
                display_name: None,
                authorization_epoch: "1".into(),
                state: AccountState::Active,
                notifications_supported: false,
            })
            .await
            .unwrap(),
    };
    let reference = format!("fixture-{}", uuid::Uuid::new_v4());
    store
        .stage_credential(&account.id, &reference)
        .await
        .unwrap();
    let account = store
        .commit_account_credential(
            RemoteAccount {
                authorization_epoch: (account.authorization_epoch.parse::<u64>().unwrap() + 1)
                    .to_string(),
                state: AccountState::Active,
                ..account
            },
            &reference,
        )
        .await
        .unwrap();
    crate::runtime::detail_tests::fixtures::project(&store, &account).await;
    let mut registry = ProviderRegistry::default();
    registry.register(Arc::new(ReadProvider)).unwrap();
    if register {
        registry
            .register_delivery(&ProviderInstance::public(ProviderKind::Github), policy)
            .unwrap();
    }
    let mut runtime = CollaborationRuntime::with_registry(store, Arc::new(Vault), registry);
    let clock = Arc::new(TestClock::new());
    runtime.clock = clock.clone();
    (Arc::new(runtime), account, clock)
}
async fn admit(
    runtime: &CollaborationRuntime,
    account: &RemoteAccount,
    id: &str,
    target: &str,
    deps: Vec<String>,
) {
    let submission = seal_command(CommandDraft {
        command_id: id.into(),
        account_id: account.id.clone(),
        authorization_epoch: account.authorization_epoch.clone(),
        target: CommandTarget::new(CommandTargetKind::Issue, target, Some("repo".into())).unwrap(),
        payload: Payload,
        guards: vec![],
        dependencies: deps,
    })
    .unwrap();
    runtime
        .store
        .admit_command(&submission, &Admission)
        .await
        .unwrap();
}
async fn state(runtime: &CollaborationRuntime, id: &str) -> DeliveryCommand {
    runtime.store.delivery_command("a", id).await.unwrap()
}
async fn sql(path: &Path, statement: &'static str) {
    let mut db = SqliteConnection::connect_with(
        &sqlx::sqlite::SqliteConnectOptions::new()
            .filename(path)
            .foreign_keys(true),
    )
    .await
    .unwrap();
    sqlx::raw_sql(statement).execute(&mut db).await.unwrap();
    db.close().await.unwrap();
}

#[tokio::test]
async fn durable_claim_precedes_dispatch_and_confirmed_result_materializes_atomically() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("state.db");
    let policy = Arc::new(Policy::new(dir.path().join("remote")));
    policy.hold.store(true, Ordering::SeqCst);
    let (runtime, account, _) = runtime(&path, policy.clone(), true).await;
    admit(&runtime, &account, FIRST, "issue", vec![]).await;
    let receipt = runtime
        .store
        .command_receipt("a", FIRST)
        .await
        .unwrap()
        .unwrap();
    let task = tokio::spawn({
        let r = runtime.clone();
        async move { r.run_delivery_next().await }
    });
    policy.entered.notified().await;
    let pending = state(&runtime, FIRST).await;
    assert_eq!(pending.state, DeliveryState::Sending);
    assert_eq!(pending.attempt_count, 1);
    assert!(policy.effects().is_empty());
    policy.release.notify_one();
    assert!(task.await.unwrap().unwrap());
    let done = state(&runtime, FIRST).await;
    assert_eq!(done.state, DeliveryState::Confirmed);
    assert_eq!(done.evidence.len(), 1);
    assert_eq!(
        done.evidence[0].attempt,
        (done.attempt_count > 0).then_some(1)
    );
    assert_eq!(policy.effects(), [FIRST]);
    assert_eq!(
        receipt,
        runtime
            .store
            .command_receipt("a", FIRST)
            .await
            .unwrap()
            .unwrap()
    );
    assert_eq!(
        runtime
            .store
            .item("a", "pull")
            .await
            .unwrap()
            .item
            .unwrap()
            .body
            .as_deref(),
        Some(FIRST)
    );
    runtime.shutdown().await.unwrap();
}
#[tokio::test]
async fn accepted_is_not_confirmation_and_reconciliation_never_resubmits() {
    for mode in [1, 2, 6, 7] {
        let dir = tempfile::tempdir().unwrap();
        let policy = Arc::new(Policy::new(dir.path().join("remote")));
        policy.mode.store(mode, Ordering::SeqCst);
        let (runtime, account, clock) =
            runtime(&dir.path().join("state.db"), policy.clone(), true).await;
        admit(&runtime, &account, FIRST, "issue", vec![]).await;
        assert!(runtime.run_delivery_next().await.unwrap());
        assert_eq!(
            state(&runtime, FIRST).await.state,
            if mode == 1 {
                DeliveryState::Accepted
            } else {
                DeliveryState::Unknown
            }
        );
        assert!(!runtime.run_delivery_next().await.unwrap());
        clock.advance(61);
        assert!(runtime.run_delivery_next().await.unwrap());
        assert_eq!(state(&runtime, FIRST).await.state, DeliveryState::Confirmed);
        assert_eq!(policy.calls.load(Ordering::SeqCst), 1);
        assert_eq!(policy.effects(), [FIRST]);
        runtime.shutdown().await.unwrap();
    }
}
#[tokio::test]
async fn only_policy_proven_safe_retry_dispatches_again_and_bound_is_durable() {
    let dir = tempfile::tempdir().unwrap();
    let policy = Arc::new(Policy::new(dir.path().join("remote")));
    policy.mode.store(3, Ordering::SeqCst);
    let (runtime, account, clock) =
        runtime(&dir.path().join("state.db"), policy.clone(), true).await;
    admit(&runtime, &account, FIRST, "issue", vec![]).await;
    for _ in 0..8 {
        assert!(runtime.run_delivery_next().await.unwrap());
        clock.advance(2);
    }
    assert!(runtime.run_delivery_next().await.unwrap());
    let command = state(&runtime, FIRST).await;
    assert_eq!(command.attempt_count, 8);
    assert_eq!(command.attention.as_deref(), Some("attempt_limit"));
    assert_eq!(policy.calls.load(Ordering::SeqCst), 8);
    assert!(policy.effects().is_empty());
    assert!(!runtime.run_delivery_next().await.unwrap());
    runtime.shutdown().await.unwrap();
}
#[tokio::test]
async fn unsupported_operation_and_stale_epoch_never_reach_adapter() {
    for supported in [false, true] {
        let dir = tempfile::tempdir().unwrap();
        let policy = Arc::new(Policy::new(dir.path().join("remote")));
        let (runtime, account, _) =
            runtime(&dir.path().join("state.db"), policy.clone(), supported).await;
        admit(&runtime, &account, FIRST, "issue", vec![]).await;
        if supported {
            runtime
                .store
                .upsert_account(RemoteAccount {
                    authorization_epoch: "3".into(),
                    ..account
                })
                .await
                .unwrap();
        }
        assert!(!runtime.run_delivery_next().await.unwrap());
        assert_eq!(state(&runtime, FIRST).await.attempt_count, 0);
        assert_eq!(policy.calls.load(Ordering::SeqCst), 0);
        runtime.shutdown().await.unwrap();
    }
}
#[tokio::test]
async fn dependencies_and_same_target_order_require_proven_confirmation() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("state.db");
    let policy = Arc::new(Policy::new(dir.path().join("remote")));
    policy.mode.store(1, Ordering::SeqCst);
    let (runtime, account, clock) = runtime(&path, policy.clone(), true).await;
    admit(&runtime, &account, FIRST, "issue", vec![]).await;
    admit(&runtime, &account, SECOND, "other", vec![FIRST.into()]).await;
    assert!(runtime.run_delivery_next().await.unwrap());
    assert!(!runtime.run_delivery_next().await.unwrap());
    assert_eq!(state(&runtime, SECOND).await.attempt_count, 0);
    clock.advance(2);
    assert!(runtime.run_delivery_next().await.unwrap());
    policy.mode.store(0, Ordering::SeqCst);
    assert!(runtime.run_delivery_next().await.unwrap());
    assert_eq!(policy.effects(), [FIRST, SECOND]);
    runtime.shutdown().await.unwrap();
}
#[tokio::test]
async fn local_guard_conflict_has_no_attempt_and_finalization_failure_rolls_back_result() {
    let dir = tempfile::tempdir().unwrap();
    let policy = Arc::new(Policy::new(dir.path().join("remote")));
    policy.guard.store(false, Ordering::SeqCst);
    let (runtime, account, _) = runtime(&dir.path().join("state.db"), policy.clone(), true).await;
    admit(&runtime, &account, FIRST, "issue", vec![]).await;
    assert!(runtime.run_delivery_next().await.unwrap());
    assert_eq!(state(&runtime, FIRST).await.state, DeliveryState::Conflict);
    assert_eq!(state(&runtime, FIRST).await.attempt_count, 0);
    policy.guard.store(true, Ordering::SeqCst);
    policy.finalize_fail.store(true, Ordering::SeqCst);
    admit(&runtime, &account, SECOND, "other", vec![]).await;
    assert!(runtime.run_delivery_next().await.is_err());
    let command = state(&runtime, SECOND).await;
    assert_eq!(command.state, DeliveryState::Sending);
    assert!(command.evidence.is_empty());
    policy.finalize_fail.store(false, Ordering::SeqCst);
    assert!(runtime.run_delivery_next().await.unwrap());
    assert_eq!(state(&runtime, SECOND).await.state, DeliveryState::Unknown);
    assert!(runtime.run_delivery_next().await.unwrap());
    assert_eq!(
        state(&runtime, SECOND).await.state,
        DeliveryState::Confirmed
    );
    assert_eq!(policy.effects(), [SECOND]);
    runtime.shutdown().await.unwrap();
}
#[tokio::test]
async fn cancellation_and_shutdown_drain_native_result_before_closing_writer() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("state.db");
    let policy = Arc::new(Policy::new(dir.path().join("remote")));
    policy.hold.store(true, Ordering::SeqCst);
    let (runtime, account, _) = runtime(&path, policy.clone(), true).await;
    admit(&runtime, &account, FIRST, "issue", vec![]).await;
    let caller = tokio::spawn({
        let r = runtime.clone();
        async move { r.run_delivery_next().await }
    });
    policy.entered.notified().await;
    caller.abort();
    let shutdown = tokio::spawn({
        let r = runtime.clone();
        async move { r.shutdown().await }
    });
    tokio::task::yield_now().await;
    assert!(!shutdown.is_finished());
    assert!(Store::open(&path).await.is_err());
    policy.release.notify_one();
    shutdown.await.unwrap().unwrap();
    let store = Store::open(&path).await.unwrap();
    assert_eq!(
        store.delivery_command("a", FIRST).await.unwrap().state,
        DeliveryState::Confirmed
    );
    store.close().await.unwrap();
    assert_eq!(policy.effects(), [FIRST]);
}

#[tokio::test]
async fn durable_quota_and_monotonic_deadlines_block_dispatch_without_consuming_attempts() {
    let dir = tempfile::tempdir().unwrap();
    let policy = Arc::new(Policy::new(dir.path().join("remote")));
    let (runtime, account, clock) =
        runtime(&dir.path().join("state.db"), policy.clone(), true).await;
    admit(&runtime, &account, FIRST, "issue", vec![]).await;
    runtime
        .persist_rate_limit(&account, 120, None)
        .await
        .unwrap();
    assert!(!runtime.run_delivery_next().await.unwrap());
    assert_eq!(state(&runtime, FIRST).await.attempt_count, 0);
    *clock.wall.lock().unwrap() += 3600;
    assert!(!runtime.run_delivery_next().await.unwrap());
    assert_eq!(policy.calls.load(Ordering::SeqCst), 0);
    clock.advance(121);
    assert!(runtime.run_delivery_next().await.unwrap());
    policy.mode.store(3, Ordering::SeqCst);
    admit(&runtime, &account, SECOND, "other", vec![]).await;
    assert!(runtime.run_delivery_next().await.unwrap());
    *clock.wall.lock().unwrap() += 3600;
    assert!(!runtime.run_delivery_next().await.unwrap());
    clock.advance(2);
    assert!(runtime.run_delivery_next().await.unwrap());
    assert_eq!(state(&runtime, SECOND).await.attempt_count, 2);
    runtime.shutdown().await.unwrap();
}
#[tokio::test]
async fn preparation_revalidates_epoch_before_claim_and_offline_keeps_zero_attempts() {
    let dir = tempfile::tempdir().unwrap();
    let policy = Arc::new(Policy::new(dir.path().join("remote")));
    let (runtime, account, clock) =
        runtime(&dir.path().join("state.db"), policy.clone(), true).await;
    admit(&runtime, &account, FIRST, "issue", vec![]).await;
    policy.mode.store(9, Ordering::SeqCst);
    assert!(runtime.run_delivery_next().await.unwrap());
    assert_eq!(state(&runtime, FIRST).await.attempt_count, 0);
    assert!(!runtime.run_delivery_next().await.unwrap());
    clock.advance(61);
    policy.mode.store(0, Ordering::SeqCst);
    policy.prepare_hold.store(true, Ordering::SeqCst);
    let task = tokio::spawn({
        let r = runtime.clone();
        async move { r.run_delivery_next().await }
    });
    policy.prepare_entered.notified().await;
    runtime
        .store
        .upsert_account(RemoteAccount {
            authorization_epoch: "3".into(),
            ..account
        })
        .await
        .unwrap();
    policy.prepare_release.notify_one();
    assert!(task.await.unwrap().is_err());
    assert_eq!(state(&runtime, FIRST).await.attempt_count, 0);
    assert!(policy.effects().is_empty());
    runtime.shutdown().await.unwrap();
}
#[tokio::test]
async fn unproven_confirmed_state_cannot_unlock_a_dependency_and_conflict_blocks_target_order() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("state.db");
    let policy = Arc::new(Policy::new(dir.path().join("remote")));
    let (runtime, account, _) = runtime(&path, policy.clone(), true).await;
    admit(&runtime, &account, FIRST, "issue", vec![]).await;
    admit(&runtime, &account, SECOND, "issue", vec![FIRST.into()]).await;
    sql(&path,"UPDATE commands SET state='confirmed' WHERE command_id='123e4567-e89b-12d3-a456-426614174000'").await;
    assert!(!runtime.run_delivery_next().await.unwrap());
    assert_eq!(state(&runtime, SECOND).await.attempt_count, 0);
    sql(&path,"UPDATE commands SET state='conflict' WHERE command_id='123e4567-e89b-12d3-a456-426614174000'").await;
    assert!(!runtime.run_delivery_next().await.unwrap());
    assert!(policy.effects().is_empty());
    runtime.shutdown().await.unwrap();
}
#[tokio::test]
async fn restore_queued_backup_then_reauthenticate_reconciles_remote_success_without_dispatch() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("state.db");
    let backup = dir.path().join("backup.db");
    let policy = Arc::new(Policy::new(dir.path().join("remote")));
    let (before, account, _) = runtime(&path, policy.clone(), true).await;
    admit(&before, &account, FIRST, "issue", vec![]).await;
    let receipt = before
        .store
        .command_receipt("a", FIRST)
        .await
        .unwrap()
        .unwrap();
    before.store.backup_to(&backup).await.unwrap();
    assert!(before.run_delivery_next().await.unwrap());
    assert_eq!(policy.effects(), [FIRST]);
    before.shutdown().await.unwrap();
    drop(before);
    let session = crate::recovery::RecoverySession::prepare(&path, &backup)
        .await
        .unwrap();
    let id = session.preview().confirmation_id.clone();
    session
        .confirm(&id, crate::recovery::RestoreChoice::ReplaceCurrentData)
        .unwrap();
    let (after, _, _) = runtime(&path, policy.clone(), true).await;
    let restored = state(&after, FIRST).await;
    assert!(restored.quarantine_generation > 0);
    assert_eq!(restored.attempt_count, 0);
    assert!(after.run_delivery_next().await.unwrap());
    let done = state(&after, FIRST).await;
    assert_eq!(done.state, DeliveryState::Confirmed);
    assert_eq!(done.attempt_count, 0);
    assert_eq!(done.evidence.len(), 1);
    assert_eq!(
        done.evidence[0].attempt,
        (done.attempt_count > 0).then_some(1)
    );
    assert!(done.quarantine_generation > 0);
    assert_eq!(
        after
            .store
            .command_receipt("a", FIRST)
            .await
            .unwrap()
            .unwrap(),
        receipt
    );
    assert_eq!(policy.calls.load(Ordering::SeqCst), 1);
    assert_eq!(policy.effects(), [FIRST]);
    after.shutdown().await.unwrap();
}
#[tokio::test]
async fn restored_safe_non_delivery_evidence_never_releases_quarantine_and_probes_are_bounded() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("state.db");
    let backup = dir.path().join("backup.db");
    let policy = Arc::new(Policy::new(dir.path().join("remote")));
    let (before, account, _) = runtime(&path, policy.clone(), true).await;
    admit(&before, &account, FIRST, "issue", vec![]).await;
    before.store.backup_to(&backup).await.unwrap();
    before.shutdown().await.unwrap();
    let session = crate::recovery::RecoverySession::prepare(&path, &backup)
        .await
        .unwrap();
    let id = session.preview().confirmation_id.clone();
    session
        .confirm(&id, crate::recovery::RestoreChoice::ReplaceCurrentData)
        .unwrap();
    policy.mode.store(3, Ordering::SeqCst);
    let (after, _, clock) = runtime(&path, policy.clone(), true).await;
    for _ in 0..8 {
        assert!(after.run_delivery_next().await.unwrap());
        clock.advance(2);
    }
    assert!(after.run_delivery_next().await.unwrap());
    let command = state(&after, FIRST).await;
    assert_eq!(command.state, DeliveryState::Unknown);
    assert_eq!(command.attempt_count, 0);
    assert_eq!(command.attention.as_deref(), Some("reconciliation_limit"));
    assert_eq!(policy.calls.load(Ordering::SeqCst), 0);
    assert_eq!(policy.probes.load(Ordering::SeqCst), 8);
    assert!(policy.effects().is_empty());
    after.shutdown().await.unwrap();
}

#[tokio::test]
#[ignore = "subprocess fixture; launched by delivery_process_death_boundaries"]
async fn delivery_crash_child() {
    let path = PathBuf::from(std::env::var_os("GITRU_DELIVERY_DB").unwrap());
    let remote = PathBuf::from(std::env::var_os("GITRU_DELIVERY_REMOTE").unwrap());
    let policy = Arc::new(Policy::new(remote));
    let (runtime, account, _) = runtime(&path, policy, true).await;
    admit(&runtime, &account, FIRST, "issue", vec![]).await;
    let _ = runtime.run_delivery_next().await;
    panic!("crash boundary was not reached");
}
#[tokio::test]
async fn delivery_process_death_boundaries_preserve_intent_and_never_duplicate_ambiguous_creates() {
    for boundary in [
        "before_claim_commit",
        "after_claim_commit",
        "after_remote_effect",
        "before_result_commit",
        "after_result_commit",
    ] {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("state.db");
        let remote = dir.path().join("remote");
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "runtime::delivery::tests::delivery_crash_child",
                "--ignored",
                "--nocapture",
            ])
            .env("GITRU_DELIVERY_DB", &path)
            .env("GITRU_DELIVERY_REMOTE", &remote)
            .env("GITRU_DELIVERY_CRASH", boundary)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .unwrap();
        assert_eq!(status.code(), Some(91), "{boundary}");
        let policy = Arc::new(Policy::new(remote));
        // Reopen with the persisted actor epoch; a process restart is not a new
        // credential admission and does not replace the original authorization.
        let store = Arc::new(Store::open(&path).await.unwrap());
        let mut registry = ProviderRegistry::default();
        registry.register(Arc::new(ReadProvider)).unwrap();
        registry
            .register_delivery(
                &ProviderInstance::public(ProviderKind::Github),
                policy.clone(),
            )
            .unwrap();
        let runtime = CollaborationRuntime::with_registry(store, Arc::new(Vault), registry);
        let cold = state(&runtime, FIRST).await;
        let expected = match boundary {
            "before_claim_commit" => DeliveryState::Queued,
            "after_result_commit" => DeliveryState::Confirmed,
            _ => DeliveryState::Sending,
        };
        assert_eq!(cold.state, expected, "{boundary}");
        for _ in 0..3 {
            let _ = runtime.run_delivery_next().await.unwrap();
        }
        let settled = state(&runtime, FIRST).await;
        if boundary == "after_claim_commit" {
            assert_eq!(settled.state, DeliveryState::Unknown);
            assert!(policy.effects().is_empty());
            assert_eq!(policy.calls.load(Ordering::SeqCst), 0);
        } else {
            assert_eq!(settled.state, DeliveryState::Confirmed, "{boundary}");
            assert_eq!(policy.effects(), [FIRST], "{boundary}");
        }
        assert_eq!(settled.attempt_count, 1);
        runtime.shutdown().await.unwrap();
    }
}

#[tokio::test]
async fn repeated_pre_dispatch_offline_errors_stop_at_durable_probe_budget() {
    let dir = tempfile::tempdir().unwrap();
    let policy = Arc::new(Policy::new(dir.path().join("remote")));
    policy.mode.store(9, Ordering::SeqCst);
    let (runtime, account, clock) =
        runtime(&dir.path().join("state.db"), policy.clone(), true).await;
    admit(&runtime, &account, FIRST, "issue", vec![]).await;
    for _ in 0..8 {
        assert!(runtime.run_delivery_next().await.unwrap());
        clock.advance(61);
    }
    assert!(runtime.run_delivery_next().await.unwrap());
    let command = state(&runtime, FIRST).await;
    assert_eq!(command.state, DeliveryState::Queued);
    assert_eq!(command.attempt_count, 0);
    assert_eq!(command.attention.as_deref(), Some("reconciliation_limit"));
    assert!(!runtime.run_delivery_next().await.unwrap());
    assert!(policy.effects().is_empty());
    runtime.shutdown().await.unwrap();
}
#[tokio::test]
async fn pending_response_times_out_to_unknown_then_reconciles_without_a_second_create() {
    let dir = tempfile::tempdir().unwrap();
    let policy = Arc::new(Policy::new(dir.path().join("remote")));
    policy.mode.store(8, Ordering::SeqCst);
    let (runtime, account, clock) =
        runtime(&dir.path().join("state.db"), policy.clone(), true).await;
    admit(&runtime, &account, FIRST, "issue", vec![]).await;
    assert!(
        tokio::time::timeout(Duration::from_secs(35), runtime.run_delivery_next())
            .await
            .unwrap()
            .unwrap()
    );
    assert_eq!(state(&runtime, FIRST).await.state, DeliveryState::Unknown);
    clock.advance(61);
    assert!(runtime.run_delivery_next().await.unwrap());
    assert_eq!(state(&runtime, FIRST).await.state, DeliveryState::Confirmed);
    assert_eq!(policy.effects(), [FIRST]);
    runtime.shutdown().await.unwrap();
}
#[tokio::test]
async fn account_budget_and_evidence_are_isolated_for_identical_command_ids() {
    let dir = tempfile::tempdir().unwrap();
    let policy = Arc::new(Policy::new(dir.path().join("remote")));
    let (runtime, account, _) = runtime(&dir.path().join("state.db"), policy.clone(), true).await;
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
    let b = runtime
        .store
        .commit_account_credential(
            RemoteAccount {
                authorization_epoch: "2".into(),
                ..b
            },
            "fixture-b",
        )
        .await
        .unwrap();
    admit(&runtime, &account, FIRST, "issue", vec![]).await;
    admit(&runtime, &b, FIRST, "issue", vec![]).await;
    runtime
        .persist_rate_limit(&account, 120, None)
        .await
        .unwrap();
    assert!(runtime.run_delivery_next().await.unwrap());
    assert_eq!(state(&runtime, FIRST).await.state, DeliveryState::Queued);
    let b = runtime.store.delivery_command("b", FIRST).await.unwrap();
    assert_eq!(b.state, DeliveryState::Confirmed);
    assert!(policy.exists(&b));
    assert!(!policy.exists(&state(&runtime, FIRST).await));
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn sqlite_claim_and_result_faults_rollback_without_guessing_remote_outcomes() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("state.db");
    let policy = Arc::new(Policy::new(dir.path().join("remote")));
    let (runtime, account, _) = runtime(&path, policy.clone(), true).await;
    admit(&runtime, &account, FIRST, "issue", vec![]).await;
    sql(&path,"CREATE TRIGGER fail_attempt BEFORE INSERT ON delivery_attempts BEGIN SELECT RAISE(ABORT,'synthetic claim disk fault'); END;").await;
    assert!(runtime.run_delivery_next().await.is_err());
    assert_eq!(state(&runtime, FIRST).await.attempt_count, 0);
    assert_eq!(state(&runtime, FIRST).await.state, DeliveryState::Queued);
    assert!(policy.effects().is_empty());
    sql(&path,"DROP TRIGGER fail_attempt; CREATE TRIGGER fail_evidence BEFORE INSERT ON command_evidence BEGIN SELECT RAISE(ABORT,'synthetic result disk fault'); END;").await;
    assert!(runtime.run_delivery_next().await.is_err());
    assert_eq!(policy.effects(), [FIRST]);
    assert_eq!(state(&runtime, FIRST).await.state, DeliveryState::Sending);
    assert!(state(&runtime, FIRST).await.evidence.is_empty());
    sql(&path, "DROP TRIGGER fail_evidence;").await;
    assert!(runtime.run_delivery_next().await.unwrap());
    assert_eq!(state(&runtime, FIRST).await.state, DeliveryState::Unknown);
    assert!(runtime.run_delivery_next().await.unwrap());
    assert_eq!(state(&runtime, FIRST).await.state, DeliveryState::Confirmed);
    assert_eq!(policy.calls.load(Ordering::SeqCst), 1);
    runtime.shutdown().await.unwrap();
}
#[tokio::test]
async fn accepted_can_be_finally_rejected_without_another_mutation() {
    let dir = tempfile::tempdir().unwrap();
    let policy = Arc::new(Policy::new(dir.path().join("remote")));
    policy.mode.store(1, Ordering::SeqCst);
    let (runtime, account, clock) =
        runtime(&dir.path().join("state.db"), policy.clone(), true).await;
    admit(&runtime, &account, FIRST, "issue", vec![]).await;
    assert!(runtime.run_delivery_next().await.unwrap());
    policy.mode.store(5, Ordering::SeqCst);
    clock.advance(2);
    assert!(runtime.run_delivery_next().await.unwrap());
    let command = state(&runtime, FIRST).await;
    assert_eq!(command.state, DeliveryState::Rejected);
    assert_eq!(command.evidence.len(), 2);
    assert_eq!(policy.calls.load(Ordering::SeqCst), 1);
    assert!(!runtime.run_delivery_next().await.unwrap());
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn many_waiting_commands_do_not_consume_another_accounts_timer_capacity() {
    let dir = tempfile::tempdir().unwrap();
    let policy = Arc::new(Policy::new(dir.path().join("remote")));
    policy.mode.store(3, Ordering::SeqCst);
    let (runtime, account, _) = runtime(&dir.path().join("state.db"), policy.clone(), true).await;
    for n in 0..129 {
        let id = format!("123e4567-e89b-12d3-a456-{n:012}");
        admit(&runtime, &account, &id, &format!("issue-{n}"), vec![]).await;
        assert!(runtime.run_delivery_next().await.unwrap(), "command {n}");
    }
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
    let b = runtime
        .store
        .commit_account_credential(
            RemoteAccount {
                authorization_epoch: "2".into(),
                ..b
            },
            "fixture-b",
        )
        .await
        .unwrap();
    admit(&runtime, &b, FIRST, "issue", vec![]).await;
    policy.mode.store(0, Ordering::SeqCst);
    assert!(runtime.run_delivery_next().await.unwrap());
    assert_eq!(
        runtime
            .store
            .delivery_command("b", FIRST)
            .await
            .unwrap()
            .state,
        DeliveryState::Confirmed
    );
    assert_eq!(policy.effects(), [FIRST]);
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn late_reconciliation_cannot_overwrite_a_cancelled_command() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("state.db");
    let policy = Arc::new(Policy::new(dir.path().join("remote")));
    policy.mode.store(2, Ordering::SeqCst);
    let (runtime, account, clock) = runtime(&path, policy.clone(), true).await;
    admit(&runtime, &account, FIRST, "issue", vec![]).await;
    runtime.run_delivery_next().await.unwrap();
    clock.advance(61);
    let command = state(&runtime, FIRST).await;
    let (request, _) = runtime
        .store
        .claim_reconciliation(&command, &account, &runtime.delivery_time().await)
        .await
        .unwrap();
    let request = request.unwrap();
    sql(&path,"UPDATE commands SET state='cancelled' WHERE command_id='123e4567-e89b-12d3-a456-426614174000'").await;
    let report = policy.report(&command, "fixture.confirmed");
    let error = runtime
        .store
        .complete_delivery(
            &request.command,
            policy.as_ref(),
            DeliveryCompletion {
                account: &account,
                attempt: None,
                report: &report,
                now: &runtime.now_string(),
                next: &runtime.delivery_future(60).await,
            },
        )
        .await
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::StaleView);
    assert_eq!(state(&runtime, FIRST).await.state, DeliveryState::Cancelled);
    assert!(state(&runtime, FIRST).await.evidence.is_empty());
    assert_eq!(policy.effects(), [FIRST]);
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn successful_preparation_quota_is_durable_before_attempt_claim_and_survives_restart() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("state.db");
    let policy = Arc::new(Policy::new(dir.path().join("remote")));
    policy.prepare_cooldown.store(120, Ordering::SeqCst);
    let (before, account, _) = runtime(&path, policy.clone(), true).await;
    admit(&before, &account, FIRST, "issue", vec![]).await;
    assert!(before.run_delivery_next().await.unwrap());
    let command = state(&before, FIRST).await;
    assert_eq!(command.state, DeliveryState::Queued);
    assert_eq!(command.attempt_count, 0);
    assert!(
        before
            .store
            .scope_state("a", "provider:rest")
            .await
            .unwrap()
            .unwrap()
            .sync
            .next_retry_at
            .is_some()
    );
    assert!(policy.effects().is_empty());
    before.shutdown().await.unwrap();
    policy.prepare_cooldown.store(0, Ordering::SeqCst);
    let store = Arc::new(Store::open(&path).await.unwrap());
    let mut registry = ProviderRegistry::default();
    registry.register(Arc::new(ReadProvider)).unwrap();
    registry
        .register_delivery(
            &ProviderInstance::public(ProviderKind::Github),
            policy.clone(),
        )
        .unwrap();
    let mut after = CollaborationRuntime::with_registry(store, Arc::new(Vault), registry);
    let clock = Arc::new(TestClock::new());
    after.clock = clock.clone();
    assert!(!after.run_delivery_next().await.unwrap());
    assert_eq!(state(&after, FIRST).await.attempt_count, 0);
    clock.advance(121);
    assert!(after.run_delivery_next().await.unwrap());
    assert_eq!(state(&after, FIRST).await.state, DeliveryState::Confirmed);
    assert_eq!(policy.calls.load(Ordering::SeqCst), 1);
    assert_eq!(policy.effects(), [FIRST]);
    after.shutdown().await.unwrap();
}

#[tokio::test]
async fn result_finalization_failure_still_persists_observed_quota() {
    let dir = tempfile::tempdir().unwrap();
    let policy = Arc::new(Policy::new(dir.path().join("remote")));
    policy.result_cooldown.store(120, Ordering::SeqCst);
    policy.finalize_fail.store(true, Ordering::SeqCst);
    let (runtime, account, clock) =
        runtime(&dir.path().join("state.db"), policy.clone(), true).await;
    admit(&runtime, &account, FIRST, "issue", vec![]).await;
    assert!(runtime.run_delivery_next().await.is_err());
    assert_eq!(state(&runtime, FIRST).await.state, DeliveryState::Sending);
    assert!(state(&runtime, FIRST).await.evidence.is_empty());
    assert!(
        runtime
            .store
            .scope_state("a", "provider:rest")
            .await
            .unwrap()
            .unwrap()
            .sync
            .next_retry_at
            .is_some()
    );
    policy.finalize_fail.store(false, Ordering::SeqCst);
    assert!(runtime.run_delivery_next().await.unwrap()); // Native crash recovery is local.
    assert_eq!(state(&runtime, FIRST).await.state, DeliveryState::Unknown);
    assert!(!runtime.run_delivery_next().await.unwrap());
    assert_eq!(policy.probes.load(Ordering::SeqCst), 0);
    clock.advance(121);
    assert!(runtime.run_delivery_next().await.unwrap());
    assert_eq!(state(&runtime, FIRST).await.state, DeliveryState::Confirmed);
    assert_eq!(policy.calls.load(Ordering::SeqCst), 1);
    assert_eq!(policy.effects(), [FIRST]);
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn preparation_and_reconciliation_errors_preserve_quota_after_scheduling_failure() {
    for reconcile in [false, true] {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("state.db");
        let policy = Arc::new(Policy::new(dir.path().join("remote")));
        let (runtime, account, clock) = runtime(&path, policy.clone(), true).await;
        admit(&runtime, &account, FIRST, "issue", vec![]).await;
        if reconcile {
            policy.mode.store(2, Ordering::SeqCst);
            assert!(runtime.run_delivery_next().await.unwrap());
            assert_eq!(state(&runtime, FIRST).await.state, DeliveryState::Unknown);
            clock.advance(61);
        }
        policy.mode.store(10, Ordering::SeqCst);
        // Claiming the finite read is allowed; its subsequent outcome/defer fails.
        sql(&path, "CREATE TRIGGER fail_schedule BEFORE UPDATE ON command_delivery WHEN NEW.next_action_at IS NOT NULL BEGIN SELECT RAISE(ABORT,'synthetic schedule fault'); END;").await;
        assert!(runtime.run_delivery_next().await.is_err());
        assert!(
            runtime
                .store
                .scope_state("a", "provider:rest")
                .await
                .unwrap()
                .unwrap()
                .sync
                .next_retry_at
                .is_some()
        );
        sql(&path, "DROP TRIGGER fail_schedule;").await;
        let probes = policy.probes.load(Ordering::SeqCst);
        assert!(!runtime.run_delivery_next().await.unwrap());
        assert_eq!(policy.probes.load(Ordering::SeqCst), probes);
        assert_eq!(
            state(&runtime, FIRST).await.attempt_count,
            if reconcile { 1 } else { 0 }
        );
        clock.advance(121);
        policy.mode.store(0, Ordering::SeqCst);
        assert!(runtime.run_delivery_next().await.unwrap());
        assert_eq!(state(&runtime, FIRST).await.state, DeliveryState::Confirmed);
        assert_eq!(policy.calls.load(Ordering::SeqCst), 1);
        runtime.shutdown().await.unwrap();
    }
}

#[tokio::test]
async fn dispatch_authentication_failure_retains_unknown_intent_and_stops_account_delivery() {
    let dir = tempfile::tempdir().unwrap();
    let policy = Arc::new(Policy::new(dir.path().join("remote")));
    policy.mode.store(11, Ordering::SeqCst);
    let (runtime, account, _) = runtime(&dir.path().join("state.db"), policy.clone(), true).await;
    admit(&runtime, &account, FIRST, "issue", vec![]).await;
    admit(&runtime, &account, SECOND, "other", vec![]).await;
    let original = state(&runtime, FIRST).await;
    assert!(runtime.run_delivery_next().await.unwrap());
    let command = state(&runtime, FIRST).await;
    assert_eq!(command.state, DeliveryState::Unknown);
    assert_eq!(command.attempt_count, 1);
    assert!(command.evidence.is_empty());
    assert_eq!(command.canonical_envelope, original.canonical_envelope);
    assert_eq!(command.authorization_epoch, original.authorization_epoch);
    assert_eq!(
        runtime.store.account("a").await.unwrap().state,
        AccountState::AuthRequired
    );
    assert!(!runtime.run_delivery_next().await.unwrap());
    assert_eq!(state(&runtime, SECOND).await.attempt_count, 0);
    assert_eq!(policy.calls.load(Ordering::SeqCst), 1);
    assert_eq!(policy.probes.load(Ordering::SeqCst), 0);
    assert!(policy.effects().is_empty());
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn dispatch_auth_observation_survives_independent_result_and_quota_faults() {
    for quota_fault in [false, true] {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("state.db");
        let policy = Arc::new(Policy::new(dir.path().join("remote")));
        policy.mode.store(11, Ordering::SeqCst);
        policy.result_cooldown.store(120, Ordering::SeqCst);
        let (runtime, account, _) = runtime(&path, policy.clone(), true).await;
        admit(&runtime, &account, FIRST, "issue", vec![]).await;
        admit(&runtime, &account, SECOND, "other", vec![]).await;
        if quota_fault {
            sql(&path, "CREATE TRIGGER fail_quota BEFORE INSERT ON sync_scopes WHEN NEW.scope='provider:rest' BEGIN SELECT RAISE(ABORT,'synthetic quota fault'); END;").await;
        } else {
            sql(&path, "CREATE TRIGGER fail_result BEFORE UPDATE ON delivery_attempts BEGIN SELECT RAISE(ABORT,'synthetic result fault'); END;").await;
        }
        assert!(runtime.run_delivery_next().await.is_err());
        let command = state(&runtime, FIRST).await;
        assert_eq!(
            command.state,
            if quota_fault {
                DeliveryState::Unknown
            } else {
                DeliveryState::Sending
            }
        );
        assert_eq!(command.attempt_count, 1);
        assert!(command.evidence.is_empty());
        assert_eq!(
            runtime.store.account("a").await.unwrap().state,
            AccountState::AuthRequired
        );
        if quota_fault {
            sql(&path, "DROP TRIGGER fail_quota;").await;
        } else {
            sql(&path, "DROP TRIGGER fail_result;").await;
        }
        // Auth-required accounts leave the candidate query altogether.
        assert!(!runtime.run_delivery_next().await.unwrap());
        assert_eq!(state(&runtime, SECOND).await.attempt_count, 0);
        assert_eq!(policy.calls.load(Ordering::SeqCst), 1);
        assert_eq!(policy.probes.load(Ordering::SeqCst), 0);
        runtime.shutdown().await.unwrap();
    }
}
