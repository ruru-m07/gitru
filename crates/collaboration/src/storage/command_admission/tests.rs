use super::*;
use crate::commands::{
    CanonicalFields, CommandDraft, CommandPayloadCodec, CommandTarget, CommandTargetKind,
    seal_command,
};
use crate::runtime::detail_tests::fixtures;

const FIRST: &str = "123e4567-e89b-12d3-a456-426614174000";
const SECOND: &str = "123e4567-e89b-12d3-a456-426614174001";
const THIRD: &str = "123e4567-e89b-12d3-a456-426614174002";

#[derive(Clone)]
struct Payload(String);
impl CommandPayloadCodec for Payload {
    const OPERATION_KIND: &'static str = "test.replace_text";
    const PAYLOAD_VERSION: u32 = 1;
    fn encode_payload(&self, fields: &mut CanonicalFields) -> Result<()> {
        fields.string(1, &self.0)
    }
}

#[derive(Default)]
struct Policy {
    reject: bool,
    protections: Vec<CommandProtection>,
}
#[async_trait::async_trait]
impl CommandAdmissionPolicy for Policy {
    const OPERATION_KIND: &'static str = Payload::OPERATION_KIND;
    const PAYLOAD_VERSION: u32 = Payload::PAYLOAD_VERSION;
    async fn validate(
        &self,
        tx: &mut Transaction<'_, Sqlite>,
        account: &RemoteAccount,
        submission: &CommandSubmission,
    ) -> Result<Vec<CommandProtection>> {
        if self.reject {
            return Err(CollaborationError::new(
                ErrorCode::PermissionDenied,
                "Fixture policy denied",
            ));
        }
        let kind: Option<String> =
            sqlx::query_scalar("SELECT kind FROM items WHERE account_id=? AND id=?")
                .bind(&account.id)
                .bind(submission.target().id())
                .fetch_optional(&mut **tx)
                .await
                .map_err(storage_error)?;
        if kind.as_deref() != Some(submission.target().kind().storage_name()) {
            return Err(not_found());
        }
        Ok(self.protections.clone())
    }
}
fn draft(id: &str, account: &str, text: &str) -> CommandDraft<Payload> {
    CommandDraft {
        command_id: id.into(),
        account_id: account.into(),
        authorization_epoch: "1".into(),
        target: CommandTarget::new(CommandTargetKind::PullRequest, "pull", Some("repo".into()))
            .unwrap(),
        payload: Payload(text.into()),
        guards: vec![],
        dependencies: vec![],
    }
}
fn submission(id: &str, account: &str, text: &str) -> CommandSubmission {
    seal_command(draft(id, account, text)).unwrap()
}
async fn setup() -> (tempfile::TempDir, Store) {
    let directory = tempfile::tempdir().unwrap();
    let store = Store::open(directory.path().join("commands.db"))
        .await
        .unwrap();
    fixtures::seed(&store, "a").await;
    fixtures::seed(&store, "b").await;
    (directory, store)
}
async fn row_count(store: &Store, table: &'static str) -> i64 {
    sqlx::query_scalar(sqlx::AssertSqlSafe(format!("SELECT count(*) FROM {table}")))
        .fetch_one(&store.inner.readers)
        .await
        .unwrap()
}
async fn assert_error(
    store: &Store,
    submission: &CommandSubmission,
    policy: &Policy,
    error: CommandAdmissionError,
) {
    let before = store.revision().await.unwrap();
    let rows = row_count(store, "commands").await;
    assert_eq!(
        store.admit_command(submission, policy).await.unwrap_err(),
        error
    );
    assert_eq!(store.revision().await.unwrap(), before);
    assert_eq!(row_count(store, "commands").await, rows);
}

#[tokio::test]
async fn receipt_is_atomic_and_exact_retry_survives_reopen() {
    let (directory, store) = setup().await;
    let request = submission(FIRST, "a", "Unicode 雪 ✅");
    let before = store.revision().await.unwrap().parse::<u64>().unwrap();
    let receipt = store
        .admit_command(&request, &Policy::default())
        .await
        .unwrap();
    assert_eq!(receipt.enqueue_order, 1);
    assert_eq!(receipt.admitted_revision, (before + 1).to_string());
    assert!(!receipt.duplicate);
    assert_eq!(receipt.submission_hash, *request.submission_hash());
    let mut expected = receipt.clone();
    expected.duplicate = true;
    assert_eq!(
        store
            .admit_command(
                &request,
                &Policy {
                    reject: true,
                    ..Default::default()
                }
            )
            .await
            .unwrap(),
        expected
    );
    assert_eq!(row_count(&store, "commands").await, 1);
    assert_eq!(row_count(&store, "command_target_protections").await, 2);
    assert_eq!(row_count(&store, "delivery_attempts").await, 0);
    assert_eq!(row_count(&store, "command_evidence").await, 0);
    assert_eq!(store.revision().await.unwrap(), receipt.admitted_revision);
    store.close().await.unwrap();
    drop(store);
    let store = Store::open(directory.path().join("commands.db"))
        .await
        .unwrap();
    assert_eq!(
        store
            .admit_command(&request, &Policy::default())
            .await
            .unwrap(),
        expected
    );
    assert_eq!(
        store.command_receipt("a", FIRST).await.unwrap(),
        Some(receipt)
    );
    assert_eq!(store.command_receipt("b", FIRST).await.unwrap(), None);
}

#[tokio::test]
async fn changed_intent_conflicts_and_account_partition_is_independent() {
    let (_directory, store) = setup().await;
    let first = submission(FIRST, "a", "one");
    store
        .admit_command(&first, &Policy::default())
        .await
        .unwrap();
    assert_error(
        &store,
        &submission(FIRST, "a", "two"),
        &Policy::default(),
        CommandAdmissionError::IdempotencyConflict,
    )
    .await;
    let mut changed = draft(FIRST, "a", "one");
    changed.target = CommandTarget::new(CommandTargetKind::Issue, "other", None).unwrap();
    assert_error(
        &store,
        &seal_command(changed).unwrap(),
        &Policy::default(),
        CommandAdmissionError::IdempotencyConflict,
    )
    .await;
    let receipt = store
        .admit_command(&submission(FIRST, "b", "two"), &Policy::default())
        .await
        .unwrap();
    assert_eq!(receipt.enqueue_order, 1);
    assert_ne!(receipt.submission_hash, *first.submission_hash());
}

#[tokio::test]
async fn dependencies_capture_exact_predecessors_and_order() {
    let (_directory, store) = setup().await;
    let first = store
        .admit_command(&submission(FIRST, "a", "one"), &Policy::default())
        .await
        .unwrap();
    store
        .admit_command(&submission(SECOND, "a", "two"), &Policy::default())
        .await
        .unwrap();
    let mut pending = draft(THIRD, "a", "three");
    pending.dependencies = vec![FIRST.into(), SECOND.into()];
    let receipt = store
        .admit_command(&seal_command(pending.clone()).unwrap(), &Policy::default())
        .await
        .unwrap();
    assert_eq!(receipt.enqueue_order, 3);
    let hash: Vec<u8>=sqlx::query_scalar("SELECT predecessor_hash FROM command_dependencies WHERE account_id='a' AND command_id=? AND ordinal=0")
        .bind(THIRD).fetch_one(&store.inner.readers).await.unwrap();
    assert_eq!(hash, first.submission_hash);
    assert!(
        sqlx::query("UPDATE command_dependencies SET required=0 WHERE account_id='a'")
            .execute(&mut *store.inner.writer.acquire().await.unwrap())
            .await
            .is_err()
    );
    pending.dependencies.reverse();
    assert_error(
        &store,
        &seal_command(pending).unwrap(),
        &Policy::default(),
        CommandAdmissionError::IdempotencyConflict,
    )
    .await;
    let mut missing = draft(SECOND, "b", "two");
    missing.dependencies = vec![FIRST.into()];
    assert_error(
        &store,
        &seal_command(missing).unwrap(),
        &Policy::default(),
        CommandAdmissionError::MissingPredecessor,
    )
    .await;
}

#[tokio::test]
async fn concurrency_has_one_winner_and_one_revision() {
    let (_directory, store) = setup().await;
    let first = submission(FIRST, "a", "same");
    let before = store.revision().await.unwrap().parse::<u64>().unwrap();
    let policy = Policy::default();
    let (a, b) = tokio::join!(
        store.admit_command(&first, &policy),
        store.admit_command(&first, &policy)
    );
    let (a, b) = (a.unwrap(), b.unwrap());
    assert_ne!(a.duplicate, b.duplicate);
    assert_eq!(a.admitted_revision, b.admitted_revision);
    assert_eq!(store.revision().await.unwrap(), (before + 1).to_string());
    let x = submission(SECOND, "a", "x");
    let y = submission(SECOND, "a", "y");
    let (a, b) = tokio::join!(
        store.admit_command(&x, &policy),
        store.admit_command(&y, &policy)
    );
    assert!(matches!(
        (&a, &b),
        (Ok(_), Err(CommandAdmissionError::IdempotencyConflict))
            | (Err(CommandAdmissionError::IdempotencyConflict), Ok(_))
    ));
    assert_eq!(row_count(&store, "commands").await, 2);
    assert_eq!(store.revision().await.unwrap(), (before + 2).to_string());
}

#[tokio::test]
async fn local_policy_epoch_bounds_and_missing_base_fail_without_writes() {
    let (_directory, store) = setup().await;
    let request = submission(FIRST, "a", "one");
    assert_error(
        &store,
        &request,
        &Policy {
            reject: true,
            ..Default::default()
        },
        CommandAdmissionError::Local(CollaborationError::new(
            ErrorCode::PermissionDenied,
            "Fixture policy denied",
        )),
    )
    .await;
    let mut changed = draft(FIRST, "a", "one");
    changed.authorization_epoch = "2".into();
    assert_error(
        &store,
        &seal_command(changed).unwrap(),
        &Policy::default(),
        CommandAdmissionError::Local(stale()),
    )
    .await;
    let mut missing = draft(FIRST, "a", "one");
    missing.target = CommandTarget::new(CommandTargetKind::PullRequest, "absent", None).unwrap();
    assert_error(
        &store,
        &seal_command(missing).unwrap(),
        &Policy::default(),
        CommandAdmissionError::Local(not_found()),
    )
    .await;
    let many = Policy {
        protections: (0..128)
            .map(|i| CommandProtection::Blob(format!("blob-{i}")))
            .collect(),
        ..Default::default()
    };
    assert_error(
        &store,
        &request,
        &many,
        CommandAdmissionError::Local(CollaborationError::invalid("Too many command protections")),
    )
    .await;
    store.disconnect("a").await.unwrap();
    assert_error(
        &store,
        &request,
        &Policy::default(),
        CommandAdmissionError::Local(stale()),
    )
    .await;
}

#[tokio::test]
async fn aborting_protection_insert_rolls_back_every_admission_fact() {
    let (_directory, store) = setup().await;
    let first = submission(FIRST, "a", "one");
    store
        .admit_command(&first, &Policy::default())
        .await
        .unwrap();
    let mut dependent = draft(SECOND, "a", "two");
    dependent.dependencies = vec![FIRST.into()];
    let dependent = seal_command(dependent).unwrap();
    sqlx::query("CREATE TRIGGER admission_failure BEFORE INSERT ON command_target_protections BEGIN SELECT RAISE(ABORT,'fixture failure'); END")
        .execute(&mut *store.inner.writer.acquire().await.unwrap()).await.unwrap();
    assert_error(
        &store,
        &dependent,
        &Policy::default(),
        CommandAdmissionError::Local(CollaborationError::storage()),
    )
    .await;
    assert_eq!(row_count(&store, "command_dependencies").await, 0);
    assert_eq!(row_count(&store, "command_target_protections").await, 2);
    sqlx::query("DROP TRIGGER admission_failure")
        .execute(&mut *store.inner.writer.acquire().await.unwrap())
        .await
        .unwrap();
    assert_eq!(
        store
            .admit_command(&dependent, &Policy::default())
            .await
            .unwrap()
            .enqueue_order,
        2
    );
}

pub(super) fn crash_checkpoint(point: &str) {
    if std::env::var("GITRU_COMMAND_CRASH_POINT").as_deref() == Ok(point) {
        let marker = std::env::var_os("GITRU_COMMAND_CRASH_MARKER").expect("isolated crash marker");
        std::fs::write(marker, point).unwrap();
        loop {
            std::thread::park();
        }
    }
}

#[tokio::test]
async fn immutable_facts_and_collision_defense_are_enforced_by_storage() {
    let (_directory, store) = setup().await;
    let request = submission(FIRST, "a", "one");
    store
        .admit_command(&request, &Policy::default())
        .await
        .unwrap();
    let mut writer = store.inner.writer.acquire().await.unwrap();
    assert!(
        sqlx::query("UPDATE commands SET payload_bytes=x'00' WHERE account_id='a'")
            .execute(&mut *writer)
            .await
            .is_err()
    );
    assert!(
        sqlx::query("DELETE FROM commands WHERE account_id='a'")
            .execute(&mut *writer)
            .await
            .is_err()
    );
    for mutation in [
        "UPDATE command_target_protections SET reference_id='other'",
        "UPDATE command_target_protections SET required=0",
        "UPDATE command_target_protections SET account_id='b'",
        "UPDATE command_target_protections SET command_id='123e4567-e89b-12d3-a456-426614174001'",
        "DELETE FROM command_target_protections",
    ] {
        assert!(sqlx::query(mutation).execute(&mut *writer).await.is_err());
    }
    // Simulate a corrupted decomposed field while retaining the exact hash and
    // envelope. The digest must never authorize a different stored intent.
    sqlx::query("DROP TRIGGER commands_immutable")
        .execute(&mut *writer)
        .await
        .unwrap();
    sqlx::query("UPDATE commands SET payload_bytes=x'00' WHERE account_id='a'")
        .execute(&mut *writer)
        .await
        .unwrap();
    drop(writer);
    assert_error(
        &store,
        &request,
        &Policy::default(),
        CommandAdmissionError::IdempotencyConflict,
    )
    .await;
}

#[tokio::test]
async fn pending_states_protect_cached_targets_and_authored_evidence() {
    use crate::storage::retention::CacheRetentionPolicy;
    for state in [
        "queued",
        "sending",
        "retry_wait",
        "accepted",
        "outcome_unknown",
        "conflict",
    ] {
        let (_directory, store) = setup().await;
        for account in ["a", "b"] {
            let account = store.account(account).await.unwrap();
            store
                .apply_detail(fixtures::commit(&store, &account, DetailFacet::Body).await)
                .await
                .unwrap();
        }
        let policy = Policy {
            protections: vec![
                CommandProtection::Blob("intent-attachment".into()),
                CommandProtection::Facet {
                    subject_id: "pull".into(),
                    facet: DetailFacet::Body,
                },
            ],
            ..Default::default()
        };
        let request = submission(FIRST, "a", "private authored text");
        let receipt = store.admit_command(&request, &policy).await.unwrap();
        {
            let mut writer = store.inner.writer.acquire().await.unwrap();
            sqlx::query("UPDATE commands SET state=? WHERE account_id='a' AND command_id=?")
                .bind(state)
                .bind(FIRST)
                .execute(&mut *writer)
                .await
                .unwrap();
            sqlx::query("UPDATE detail_demand SET requested=0")
                .execute(&mut *writer)
                .await
                .unwrap();
            sqlx::query("INSERT INTO delivery_attempts VALUES('a',?,1,1,'2026-10-07T00:00:00Z','outcome_unknown',NULL)").bind(FIRST).execute(&mut *writer).await.unwrap();
            sqlx::query("INSERT INTO command_evidence VALUES('a',?,0,1,'test.receipt',1,x'010203','2026-10-07T00:00:00Z')").bind(FIRST).execute(&mut *writer).await.unwrap();
        }
        for _ in 0..4 {
            let report = store
                .run_cache_maintenance(CacheRetentionPolicy {
                    target_logical_bytes: 0,
                    checkpoint_wal: false,
                    ..Default::default()
                })
                .await
                .unwrap();
            assert!(report.scanned_facets <= 128);
        }
        let rows: Vec<String> = sqlx::query_scalar(
            "SELECT account_id FROM detail_observations WHERE facet='body' ORDER BY account_id",
        )
        .fetch_all(&store.inner.readers)
        .await
        .unwrap();
        assert_eq!(rows, ["a"], "{state}");
        assert_eq!(row_count(&store, "delivery_attempts").await, 1);
        assert_eq!(row_count(&store, "command_evidence").await, 1);
        assert_eq!(row_count(&store, "command_target_protections").await, 4);
        assert_eq!(
            store.command_receipt("a", FIRST).await.unwrap(),
            Some(receipt.clone())
        );
        store.disconnect("a").await.unwrap();
        assert_eq!(
            store.command_receipt("a", FIRST).await.unwrap(),
            Some(receipt)
        );
        assert_eq!(row_count(&store, "command_evidence").await, 1);
        assert_eq!(row_count(&store, "command_target_protections").await, 4);
        assert_eq!(
            sqlx::query_scalar::<_, Vec<u8>>(
                "SELECT canonical_envelope FROM commands WHERE account_id='a'"
            )
            .fetch_one(&store.inner.readers)
            .await
            .unwrap(),
            request.canonical_envelope()
        );
    }
}

#[tokio::test]
async fn pending_successor_keeps_terminal_predecessor_reference_protection() {
    let (_directory, store) = setup().await;
    let account = store.account("a").await.unwrap();
    store
        .apply_detail(fixtures::commit(&store, &account, DetailFacet::Body).await)
        .await
        .unwrap();
    store
        .admit_command(&submission(FIRST, "a", "one"), &Policy::default())
        .await
        .unwrap();
    let mut next = draft(SECOND, "a", "two");
    next.dependencies = vec![FIRST.into()];
    store
        .admit_command(&seal_command(next).unwrap(), &Policy::default())
        .await
        .unwrap();
    {
        let mut writer = store.inner.writer.acquire().await.unwrap();
        sqlx::query("UPDATE commands SET state='confirmed' WHERE account_id='a' AND command_id=?")
            .bind(FIRST)
            .execute(&mut *writer)
            .await
            .unwrap();
        let required: i64 = sqlx::query_scalar("SELECT count(*) FROM command_target_protections WHERE account_id='a' AND command_id=? AND required=1")
            .bind(FIRST).fetch_one(&mut *writer).await.unwrap();
        assert_eq!(
            required, 2,
            "the confirmed predecessor remains protected solely by its pending dependent"
        );
        sqlx::query("UPDATE detail_demand SET requested=0")
            .execute(&mut *writer)
            .await
            .unwrap();
    }
    for _ in 0..4 {
        store
            .run_cache_maintenance(crate::storage::retention::CacheRetentionPolicy {
                target_logical_bytes: 0,
                checkpoint_wal: false,
                ..Default::default()
            })
            .await
            .unwrap();
    }
    assert_eq!(row_count(&store, "detail_observations").await, 1);
    assert_eq!(row_count(&store, "command_dependencies").await, 1);
    let source = include_str!("../retention.rs");
    let query = source
        .split_once("let eligible: bool = sqlx::query_scalar(\"")
        .unwrap()
        .1
        .split_once("\")")
        .unwrap()
        .0;
    let plan = sqlx::query(sqlx::AssertSqlSafe(format!("EXPLAIN QUERY PLAN {query}")))
        .bind("pull")
        .bind("detail:pull:body")
        .bind("a")
        .bind("body")
        .bind("body")
        .fetch_all(&store.inner.readers)
        .await
        .unwrap();
    let details = plan
        .iter()
        .map(|row| row.get::<String, _>("detail"))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(details.contains("command_protection_lookup"), "{details}");
    assert!(!details.contains("SCAN p"), "{details}");
    // Terminal history remains durable but leaves the partial protection index.
    sqlx::query("UPDATE commands SET state='cancelled' WHERE account_id='a' AND command_id=?")
        .bind(SECOND)
        .execute(&mut *store.inner.writer.acquire().await.unwrap())
        .await
        .unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM command_target_protections WHERE required=1"
        )
        .fetch_one(&store.inner.readers)
        .await
        .unwrap(),
        0
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM command_dependencies WHERE required=1")
            .fetch_one(&store.inner.readers)
            .await
            .unwrap(),
        0
    );
    for _ in 0..4 {
        store
            .run_cache_maintenance(crate::storage::retention::CacheRetentionPolicy {
                target_logical_bytes: 0,
                checkpoint_wal: false,
                ..Default::default()
            })
            .await
            .unwrap();
    }
    assert_eq!(row_count(&store, "detail_observations").await, 0);
    assert_eq!(row_count(&store, "commands").await, 2);
    assert_eq!(row_count(&store, "command_dependencies").await, 1);
}

#[tokio::test]
async fn schema_rejects_cross_account_forward_edges_and_unbounded_evidence() {
    let (_directory, store) = setup().await;
    store
        .admit_command(&submission(FIRST, "a", "one"), &Policy::default())
        .await
        .unwrap();
    let second = store
        .admit_command(&submission(SECOND, "a", "two"), &Policy::default())
        .await
        .unwrap();
    let mut writer = store.inner.writer.acquire().await.unwrap();
    for (account, command, predecessor) in [
        ("b", SECOND, FIRST),
        ("a", FIRST, SECOND),
        ("a", FIRST, FIRST),
    ] {
        assert!(
            sqlx::query("INSERT INTO command_dependencies(account_id,command_id,ordinal,predecessor_id,predecessor_hash) VALUES(?,?,0,?,?)")
                .bind(account)
                .bind(command)
                .bind(predecessor)
                .bind(second.submission_hash.as_slice())
                .execute(&mut *writer)
                .await
                .is_err()
        );
    }
    assert!(
        sqlx::query("INSERT INTO delivery_attempts VALUES('a',?,2,1,'now','started',NULL)")
            .bind(FIRST)
            .execute(&mut *writer)
            .await
            .is_err()
    );
    sqlx::query("INSERT INTO delivery_attempts VALUES('a',?,1,1,'now','started',NULL)")
        .bind(FIRST)
        .execute(&mut *writer)
        .await
        .unwrap();
    assert!(
        sqlx::query(
            "INSERT INTO command_evidence VALUES('a',?,0,1,'test',1,zeroblob(65537),'now')"
        )
        .bind(FIRST)
        .execute(&mut *writer)
        .await
        .is_err()
    );
    assert!(
        sqlx::query("INSERT INTO command_evidence VALUES('a',?,128,1,'test',1,x'00','now')")
            .bind(FIRST)
            .execute(&mut *writer)
            .await
            .is_err()
    );
    assert!(
        sqlx::query("INSERT INTO command_evidence VALUES('a',?,0,2,'test',1,x'00','now')")
            .bind(FIRST)
            .execute(&mut *writer)
            .await
            .is_err()
    );
    sqlx::query("INSERT INTO command_evidence VALUES('a',?,0,1,'test',1,zeroblob(65536),'now')")
        .bind(FIRST)
        .execute(&mut *writer)
        .await
        .unwrap();
    assert!(
        sqlx::query("UPDATE command_evidence SET payload=x'00'")
            .execute(&mut *writer)
            .await
            .is_err()
    );
}

struct CrashWorker(std::process::Child);
impl Drop for CrashWorker {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

#[tokio::test]
async fn hard_crash_before_or_after_commit_has_one_cold_receipt() {
    for point in ["before_commit", "after_commit"] {
        let (directory, store) = setup().await;
        let before = store.revision().await.unwrap().parse::<u64>().unwrap();
        store.close().await.unwrap();
        drop(store);
        let path = directory.path().join("commands.db");
        let marker = directory.path().join("checkpoint");
        let mut worker = CrashWorker(
            std::process::Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "storage::command_admission::tests::command_crash_worker",
                    "--ignored",
                    "--nocapture",
                ])
                .env("GITRU_COMMAND_CRASH_POINT", point)
                .env("GITRU_COMMAND_CRASH_MARKER", &marker)
                .env("GITRU_COMMAND_CRASH_DB", &path)
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::inherit())
                .spawn()
                .unwrap(),
        );
        let deadline = std::time::Instant::now() + Duration::from_secs(30);
        while !marker.exists() {
            assert!(
                worker.0.try_wait().unwrap().is_none(),
                "child exited before {point}"
            );
            assert!(
                std::time::Instant::now() < deadline,
                "child did not reach {point}"
            );
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        assert_eq!(
            Store::open(&path).await.err().unwrap().code,
            ErrorCode::Busy
        );
        worker.0.kill().unwrap();
        assert!(!worker.0.wait().unwrap().success());
        let store = Store::open(&path).await.unwrap();
        assert_eq!(
            row_count(&store, "commands").await,
            if point == "after_commit" { 1 } else { 0 }
        );
        assert_eq!(
            store.revision().await.unwrap(),
            (before + u64::from(point == "after_commit")).to_string()
        );
        let receipt = store
            .admit_command(&submission(FIRST, "a", "crash-safe"), &Policy::default())
            .await
            .unwrap();
        assert_eq!(receipt.duplicate, point == "after_commit");
        assert_eq!(receipt.admitted_revision, (before + 1).to_string());
        assert_eq!(receipt.enqueue_order, 1);
        assert_eq!(row_count(&store, "commands").await, 1);
        assert_eq!(row_count(&store, "command_target_protections").await, 2);
        assert_eq!(store.revision().await.unwrap(), receipt.admitted_revision);
    }
}

#[test]
#[ignore = "Subprocess helper invoked by hard_crash_before_or_after_commit_has_one_cold_receipt"]
fn command_crash_worker() {
    let path = std::env::var_os("GITRU_COMMAND_CRASH_DB").expect("isolated crash database");
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            let store = Store::open(PathBuf::from(path)).await.unwrap();
            store
                .admit_command(&submission(FIRST, "a", "crash-safe"), &Policy::default())
                .await
                .unwrap();
            panic!("worker must be killed before returning receipt");
        });
}

struct ChangedPayload;
impl CommandPayloadCodec for ChangedPayload {
    const OPERATION_KIND: &'static str = "test.changed_operation";
    const PAYLOAD_VERSION: u32 = 2;
    fn encode_payload(&self, fields: &mut CanonicalFields) -> Result<()> {
        fields.string(1, "one")
    }
}
struct ChangedPolicy;
#[async_trait::async_trait]
impl CommandAdmissionPolicy for ChangedPolicy {
    const OPERATION_KIND: &'static str = ChangedPayload::OPERATION_KIND;
    const PAYLOAD_VERSION: u32 = ChangedPayload::PAYLOAD_VERSION;
    async fn validate(
        &self,
        _tx: &mut Transaction<'_, Sqlite>,
        _account: &RemoteAccount,
        _submission: &CommandSubmission,
    ) -> Result<Vec<CommandProtection>> {
        Ok(vec![])
    }
}
struct Guard(u64);
impl crate::commands::CommandGuardCodec for Guard {
    const GUARD_KIND: &'static str = "test.base_revision";
    const GUARD_VERSION: u32 = 1;
    fn encode_guard(&self, fields: &mut CanonicalFields) -> Result<()> {
        fields.u64(1, self.0)
    }
}
#[tokio::test]
async fn changed_guards_and_operation_conflict_while_unknown_policy_fails_closed() {
    let (_directory, store) = setup().await;
    let mut initial = draft(FIRST, "a", "one");
    initial.guards = vec![crate::commands::seal_guard(&Guard(1)).unwrap()];
    store
        .admit_command(&seal_command(initial.clone()).unwrap(), &Policy::default())
        .await
        .unwrap();
    initial.guards = vec![crate::commands::seal_guard(&Guard(2)).unwrap()];
    assert_error(
        &store,
        &seal_command(initial).unwrap(),
        &Policy::default(),
        CommandAdmissionError::IdempotencyConflict,
    )
    .await;
    let different = seal_command(CommandDraft {
        command_id: FIRST.into(),
        account_id: "a".into(),
        authorization_epoch: "1".into(),
        target: CommandTarget::new(CommandTargetKind::PullRequest, "pull", Some("repo".into()))
            .unwrap(),
        payload: ChangedPayload,
        guards: vec![],
        dependencies: vec![],
    })
    .unwrap();
    let before = store.revision().await.unwrap();
    assert_eq!(
        store
            .admit_command(&different, &ChangedPolicy)
            .await
            .unwrap_err(),
        CommandAdmissionError::IdempotencyConflict
    );
    assert_eq!(
        store
            .admit_command(&different, &Policy::default())
            .await
            .unwrap_err(),
        CommandAdmissionError::Local(CollaborationError::new(
            ErrorCode::Unsupported,
            "Unsupported collaboration command version"
        ))
    );
    assert_eq!(store.revision().await.unwrap(), before);
}

#[tokio::test]
async fn credential_replacement_keeps_receipt_but_fences_resubmission() {
    let (_directory, store) = setup().await;
    let request = submission(FIRST, "a", "one");
    let receipt = store
        .admit_command(&request, &Policy::default())
        .await
        .unwrap();
    let mut account = fixtures::account("a");
    account.authorization_epoch = "2".into();
    store.upsert_account(account).await.unwrap();
    assert_eq!(
        store.command_receipt("a", FIRST).await.unwrap(),
        Some(receipt)
    );
    assert_error(
        &store,
        &request,
        &Policy::default(),
        CommandAdmissionError::Local(stale()),
    )
    .await;
    let mut newer = draft(FIRST, "a", "one");
    newer.authorization_epoch = "2".into();
    assert_error(
        &store,
        &seal_command(newer).unwrap(),
        &Policy::default(),
        CommandAdmissionError::IdempotencyConflict,
    )
    .await;
}
