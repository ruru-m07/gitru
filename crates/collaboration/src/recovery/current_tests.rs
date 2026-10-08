use super::*;
use crate::commands::{
    CanonicalFields, CommandDraft, CommandPayloadCodec, CommandSubmission, CommandTarget,
    CommandTargetKind, seal_command,
};
use crate::pull_file_fixture as files;
use crate::storage::command_admission::{CommandAdmissionPolicy, CommandProtection};
use crate::{DetailFacet, LocalDraft, Store};

const FIRST: &str = "123e4567-e89b-12d3-a456-426614174000";
const SECOND: &str = "123e4567-e89b-12d3-a456-426614174001";
const TEXT: &str = "Authored opaque operation — 雪\n";
struct Payload;
impl CommandPayloadCodec for Payload {
    const OPERATION_KIND: &'static str = "future.unsupported_create";
    const PAYLOAD_VERSION: u32 = 79;
    fn encode_payload(&self, fields: &mut CanonicalFields) -> Result<()> {
        fields.string(1, TEXT)
    }
}
struct Policy;
#[async_trait::async_trait]
impl CommandAdmissionPolicy for Policy {
    const OPERATION_KIND: &'static str = Payload::OPERATION_KIND;
    const PAYLOAD_VERSION: u32 = Payload::PAYLOAD_VERSION;
    async fn validate(
        &self,
        _: &mut sqlx::Transaction<'_, Sqlite>,
        _: &RemoteAccount,
        _: &CommandSubmission,
    ) -> Result<Vec<CommandProtection>> {
        Ok(vec![
            CommandProtection::Facet {
                subject_id: "pull".into(),
                facet: DetailFacet::Body,
            },
            CommandProtection::Facet {
                subject_id: "pull".into(),
                facet: DetailFacet::Files,
            },
        ])
    }
}
fn command(id: &str, account: &str, dependencies: Vec<String>) -> CommandSubmission {
    seal_command(CommandDraft {
        command_id: id.into(),
        account_id: account.into(),
        authorization_epoch: "1".into(),
        target: CommandTarget::new(CommandTargetKind::PullRequest, "pull", Some("repo".into()))
            .unwrap(),
        payload: Payload,
        guards: vec![],
        dependencies,
    })
    .unwrap()
}
async fn setup(path: &Path) -> Store {
    let store = Store::open(path).await.unwrap();
    let account = files::seed(&store, "a").await;
    files::seed(&store, "b").await;
    store
        .save_draft(LocalDraft {
            account_id: "a".into(),
            subject_id: "pull".into(),
            body: TEXT.into(),
            generation: "0".into(),
        })
        .await
        .unwrap();
    let snapshot = files::publish(&store, &account, &["src/protected.rs"]).await;
    let request = files::request(&snapshot, &account);
    let membership = store
        .verify_pull_file_membership(request.clone())
        .await
        .unwrap();
    store
        .apply_pull_file_artifact(
            request,
            membership.clone(),
            files::artifact(&membership, "@@ -1 +1 @@\n-old\n+new\n"),
        )
        .await
        .unwrap();
    let mut db = connect(path, false).await.unwrap();
    sqlx::raw_sql("INSERT INTO cache_pins VALUES('a','github:https://github.com/','pull','pull_request',1); INSERT INTO local_inbox_state VALUES('a','retained-notification','inbox',1,'2027-01-01T00:00:00Z','2026-10-08T00:00:00Z',12); INSERT INTO local_inbox_projection VALUES('a',29); INSERT INTO local_transport_bindings VALUES('binding','github:https://github.com/','ssh','example.invalid',22,'','owner_repository',4); INSERT INTO local_repository_links VALUES('link','local-repo','{\"remote_name\":\"origin\",\"direction\":\"fetch\",\"ordinal\":0,\"transport\":\"https\",\"host\":\"github.com\",\"port\":443,\"path\":\"owner/project\"}','a','github:https://github.com/','actor-a','repo','target-1','synthetic-proof','synthetic-digest',5);")
        .execute(&mut db).await.unwrap();
    db.close().await.unwrap();
    store
        .admit_command(&command(FIRST, "a", vec![]), &Policy)
        .await
        .unwrap();
    store
        .admit_command(&command(SECOND, "a", vec![FIRST.into()]), &Policy)
        .await
        .unwrap();
    store
        .admit_command(&command(FIRST, "b", vec![]), &Policy)
        .await
        .unwrap();
    store
}
async fn command_rows(path: &Path) -> Vec<String> {
    let mut db = connect(path, true).await.unwrap();
    let rows=sqlx::query_scalar("SELECT json_array(account_id,command_id,authorization_epoch,envelope_version,operation_kind,payload_version,target_kind,target_id,repository_id,hex(canonical_envelope),hex(payload_bytes),hex(guard_bytes),hex(submission_hash),enqueue_order,admitted_revision,admitted_at,state) FROM commands ORDER BY account_id,command_id")
        .fetch_all(&mut db).await.unwrap();
    db.close().await.unwrap();
    rows
}
async fn count(path: &Path, table: &str) -> i64 {
    let mut db = connect(path, true).await.unwrap();
    let result = sqlx::query_scalar(sqlx::AssertSqlSafe(format!("SELECT count(*) FROM {table}")))
        .fetch_one(&mut db)
        .await
        .unwrap();
    db.close().await.unwrap();
    result
}
async fn restore(path: &Path, backup: &Path) -> RestoreReceipt {
    let session = RecoverySession::prepare(path, backup).await.unwrap();
    let id = session.preview().confirmation_id.clone();
    assert!(session.preview().incoming_evidence_retained);
    session
        .confirm(&id, RestoreChoice::ReplaceCurrentData)
        .unwrap()
}

#[tokio::test]
async fn queued_backup_cannot_redeliver_after_remote_success_restore_and_reauthentication() {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("current.db");
    let backup = dir.path().join("backup.db");
    let store = setup(&target).await;
    let receipt = store.command_receipt("a", FIRST).await.unwrap().unwrap();
    let summary = store.backup_to(&backup).await.unwrap();
    assert_eq!(summary.commands, 3);
    assert_eq!(summary.schema_version, 25);
    assert_eq!(count(&backup, "delivery_attempts").await, 0);
    let original_commands = command_rows(&backup).await;
    // An independent remote side effect and receipt happen after the backup.
    let mut remote_creates = 1;
    let mut db = connect(&target, false).await.unwrap();
    sqlx::raw_sql("INSERT INTO delivery_attempts VALUES('a','123e4567-e89b-12d3-a456-426614174000',1,1,'2026-10-08T00:00:00Z','confirmed','2026-10-08T00:00:01Z'); INSERT INTO command_evidence VALUES('a','123e4567-e89b-12d3-a456-426614174000',0,1,'provider.receipt',1,x'7b7d','2026-10-08T00:00:01Z'); UPDATE commands SET state='confirmed' WHERE account_id='a' AND command_id='123e4567-e89b-12d3-a456-426614174000';").execute(&mut db).await.unwrap();
    db.close().await.unwrap();
    store.close().await.unwrap();
    drop(store);
    let session = RecoverySession::prepare(&target, &backup).await.unwrap();
    assert_eq!(session.preview().quarantined_commands, 3);
    assert_eq!(session.preview().recovery_generation, "1");
    let id = session.preview().confirmation_id.clone();
    let restored = session
        .confirm(&id, RestoreChoice::ReplaceCurrentData)
        .unwrap();
    assert_eq!(command_rows(&target).await, original_commands);
    assert_eq!(
        count(
            &restored.original_bundle.join("original.sqlite"),
            "delivery_attempts"
        )
        .await,
        1
    );
    let evidence = restored.original_bundle.join("incoming-evidence.sqlite");
    assert_eq!(command_rows(&evidence).await, original_commands);
    assert_eq!(count(&evidence, "pull_file_artifacts").await, 1);
    for table in [
        "items",
        "detail_observations",
        "pull_file_generations",
        "pull_file_artifacts",
        "pull_file_blob_objects",
        "sync_scopes",
        "detail_demand",
        "account_credentials",
        "credential_cleanup",
    ] {
        assert_eq!(count(&target, table).await, 0, "{table}");
    }
    for (table, expected) in [
        ("drafts", 1),
        ("local_repository_links", 1),
        ("local_transport_bindings", 1),
        ("cache_pins", 1),
        ("local_inbox_state", 1),
        ("command_dependencies", 1),
    ] {
        assert_eq!(count(&target, table).await, expected, "{table}");
    }
    let store = Store::open(&target).await.unwrap();
    assert_eq!(
        store.command_receipt("a", FIRST).await.unwrap().unwrap(),
        receipt
    );
    let mut account = store.account("a").await.unwrap();
    account.state = AccountState::Active;
    account.authorization_epoch =
        (account.authorization_epoch.parse::<u64>().unwrap() + 1).to_string();
    store.upsert_account(account).await.unwrap();
    let mut db = connect(&target, false).await.unwrap();
    let mut tx = db.begin().await.unwrap();
    if !command_quarantined_in(&mut tx, "a", FIRST).await.unwrap() {
        remote_creates += 1;
    }
    assert!(command_quarantined_in(&mut tx, "a", SECOND).await.unwrap());
    assert!(command_quarantined_in(&mut tx, "b", FIRST).await.unwrap());
    assert!(
        !command_quarantined_in(&mut tx, "missing-account", FIRST)
            .await
            .unwrap()
    );
    // Even a worker that omits the native guard cannot record a dispatch claim
    // after reauthentication. Historical attempt rows remain valid evidence.
    let error = sqlx::query("INSERT INTO delivery_attempts(account_id,command_id,attempt_number,authorization_epoch,started_at,outcome) VALUES('a',?,1,4,'2026-10-08T00:00:03Z','started')")
        .bind(FIRST)
        .execute(&mut *tx)
        .await
        .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("restored command requires reconciliation")
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM delivery_attempts")
            .fetch_one(&mut *tx)
            .await
            .unwrap(),
        0
    );
    tx.rollback().await.unwrap();
    db.close().await.unwrap();
    assert_eq!(remote_creates, 1);
    let again = dir.path().join("again.db");
    store.backup_to(&again).await.unwrap();
    store.close().await.unwrap();
    drop(store);
    assert_eq!(restore(&target, &again).await.recovery_generation, "2");
    assert_eq!(count(&target, "command_recovery_quarantine").await, 6);
    let store = Store::open(&target).await.unwrap();
    assert_eq!(
        store.command_receipt("a", FIRST).await.unwrap().unwrap(),
        receipt
    );
    store.close().await.unwrap();
}

#[tokio::test]
async fn recovery_quarantine_is_durable_and_generation_is_monotonic() {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("current.db");
    let backup = dir.path().join("backup.db");
    let store = setup(&target).await;
    store.backup_to(&backup).await.unwrap();
    store.close().await.unwrap();
    drop(store);
    restore(&target, &backup).await;
    let mut db = connect(&target, false).await.unwrap();
    for sql in [
        "UPDATE recovery_meta SET generation=0",
        "DELETE FROM recovery_meta",
        "UPDATE command_recovery_quarantine SET recovery_generation=3",
        "DELETE FROM command_recovery_quarantine",
        "INSERT INTO command_recovery_quarantine SELECT account_id,command_id,submission_hash,9 FROM commands LIMIT 1",
    ] {
        assert!(sqlx::query(sql).execute(&mut db).await.is_err(), "{sql}");
    }
    sqlx::query("UPDATE recovery_meta SET generation=21")
        .execute(&mut db)
        .await
        .unwrap();
    db.close().await.unwrap();
    assert_eq!(restore(&target, &backup).await.recovery_generation, "22");
    let before = hash_file(&target).unwrap();
    let mut db = connect(&target, false).await.unwrap();
    sqlx::query("UPDATE recovery_meta SET generation=9223372036854775807")
        .execute(&mut db)
        .await
        .unwrap();
    db.close().await.unwrap();
    let overflow_hash = hash_file(&target).unwrap();
    assert_ne!(overflow_hash, before);
    assert!(RecoverySession::prepare(&target, &backup).await.is_err());
    assert_eq!(hash_file(&target).unwrap(), overflow_hash);
}

#[tokio::test]
async fn command_tampering_fails_even_with_known_schema_and_ledger() {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("current.db");
    let backup = dir.path().join("backup.db");
    let store = setup(&target).await;
    store.backup_to(&backup).await.unwrap();
    store.close().await.unwrap();
    drop(store);
    let target_hash = hash_file(&target).unwrap();
    for (i, mutation) in [
        "UPDATE commands SET operation_kind='other.valid_kind' WHERE account_id='a'",
        "UPDATE commands SET canonical_envelope=x'00010000000400000001' WHERE account_id='a'",
        "UPDATE commands SET guard_bytes=x'000100000000' WHERE account_id='a'",
        "UPDATE commands SET admitted_at='not-a-timestamp' WHERE account_id='a'",
    ]
    .into_iter()
    .enumerate()
    {
        let bad = dir.path().join(format!("bad-{i}.db"));
        copy_private(&backup, &bad).unwrap();
        let mut db = connect(&bad, false).await.unwrap();
        let trigger: String =
            sqlx::query_scalar("SELECT sql FROM sqlite_schema WHERE name='commands_immutable'")
                .fetch_one(&mut db)
                .await
                .unwrap();
        sqlx::query("DROP TRIGGER commands_immutable")
            .execute(&mut db)
            .await
            .unwrap();
        sqlx::query(mutation).execute(&mut db).await.unwrap();
        sqlx::raw_sql(sqlx::AssertSqlSafe(trigger))
            .execute(&mut db)
            .await
            .unwrap();
        db.close().await.unwrap();
        let bad_hash = hash_file(&bad).unwrap();
        assert!(
            RecoverySession::prepare(&target, &bad).await.is_err(),
            "{mutation}"
        );
        assert_eq!(hash_file(&bad).unwrap(), bad_hash);
        assert_eq!(hash_file(&target).unwrap(), target_hash);
    }
}

#[tokio::test]
async fn tampered_protected_evidence_refuses_confirmation_before_target_mutation() {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("current.db");
    let backup = dir.path().join("backup.db");
    let store = setup(&target).await;
    store.backup_to(&backup).await.unwrap();
    store.close().await.unwrap();
    drop(store);
    let original = hash_file(&target).unwrap();
    let session = RecoverySession::prepare(&target, &backup).await.unwrap();
    let id = session.preview().confirmation_id.clone();
    std::fs::write(&session.incoming_evidence, b"tampered").unwrap();
    assert!(
        session
            .confirm(&id, RestoreChoice::ReplaceCurrentData)
            .is_err()
    );
    assert_eq!(hash_file(&target).unwrap(), original);
    assert!(!pending_path(&target).exists());
}

async fn materialize_v14_detail_schema(db: &mut SqliteConnection) {
    // Migration 0018 deliberately rebuilds these tables to extend the facet
    // constraints. This test constructs a byte-faithful v14 backup from a
    // current store, so restore the earlier constraints and the original
    // cache-retention SQL before removing the later migration ledger rows.
    sqlx::raw_sql(
        r#"
CREATE TABLE detail_observations_v14_restore (
    account_id TEXT NOT NULL REFERENCES accounts(id) ON DELETE RESTRICT,
    subject_id TEXT NOT NULL,
    facet TEXT NOT NULL CHECK(facet IN ('body','comments','reviews','checks','participants','tasks')),
    authorization_epoch TEXT NOT NULL,
    facet_revision TEXT NOT NULL,
    body_json TEXT NOT NULL,
    source_json TEXT NOT NULL,
    value_source_json TEXT,
    observed_state TEXT NOT NULL,
    stale_at TEXT,
    PRIMARY KEY(account_id,subject_id,facet)
);
CREATE TABLE detail_entries_v14_restore (
    account_id TEXT NOT NULL,
    subject_id TEXT NOT NULL,
    facet TEXT NOT NULL,
    id TEXT NOT NULL,
    json TEXT NOT NULL,
    last_seen_run TEXT NOT NULL,
    PRIMARY KEY(account_id,subject_id,facet,id),
    FOREIGN KEY(account_id,subject_id,facet) REFERENCES detail_observations_v14_restore(account_id,subject_id,facet) ON DELETE CASCADE
);
CREATE TABLE detail_resource_metadata_v14_restore (
    account_id TEXT NOT NULL,
    subject_id TEXT NOT NULL,
    facet TEXT NOT NULL DEFAULT 'body' CHECK(facet='body'),
    authorization_epoch TEXT NOT NULL,
    metadata_json TEXT NOT NULL,
    source_json TEXT NOT NULL,
    PRIMARY KEY(account_id,subject_id),
    FOREIGN KEY(account_id,subject_id,facet) REFERENCES detail_observations_v14_restore(account_id,subject_id,facet) ON DELETE CASCADE
);
CREATE TABLE detail_demand_v14_restore (
    account_id TEXT NOT NULL REFERENCES accounts(id) ON DELETE RESTRICT,
    subject_id TEXT NOT NULL,
    facet TEXT NOT NULL CHECK(facet IN ('body','comments','reviews','checks','participants','tasks','commits','files')),
    authorization_epoch TEXT NOT NULL,
    requested INTEGER NOT NULL DEFAULT 1 CHECK(requested IN (0,1)),
    PRIMARY KEY(account_id,subject_id,facet)
);
CREATE TEMP TABLE cache_retention_entries_v14_restore AS SELECT * FROM cache_retention_entries;
INSERT INTO detail_observations_v14_restore SELECT * FROM detail_observations;
INSERT INTO detail_entries_v14_restore SELECT * FROM detail_entries;
INSERT INTO detail_resource_metadata_v14_restore SELECT * FROM detail_resource_metadata;
INSERT INTO detail_demand_v14_restore SELECT * FROM detail_demand;
DROP TRIGGER cache_retention_entries_aggregate_insert;
DROP TRIGGER cache_retention_entries_aggregate_update;
DROP TRIGGER cache_retention_entries_aggregate_delete;
DROP TABLE cache_retention_entries;
DROP TABLE detail_resource_metadata;
DROP TABLE detail_entries;
DROP TABLE detail_demand;
DROP TABLE detail_observations;
ALTER TABLE detail_observations_v14_restore RENAME TO detail_observations;
ALTER TABLE detail_entries_v14_restore RENAME TO detail_entries;
ALTER TABLE detail_resource_metadata_v14_restore RENAME TO detail_resource_metadata;
ALTER TABLE detail_demand_v14_restore RENAME TO detail_demand;
CREATE TABLE cache_retention_entries (
    account_id TEXT NOT NULL,
    subject_id TEXT NOT NULL,
    facet TEXT NOT NULL,
    logical_bytes INTEGER NOT NULL CHECK(logical_bytes >= 0),
    last_observed_revision INTEGER NOT NULL CHECK(last_observed_revision > 0),
    PRIMARY KEY(account_id,subject_id,facet),
    FOREIGN KEY(account_id,subject_id,facet)
        REFERENCES detail_observations(account_id,subject_id,facet)
        ON DELETE CASCADE
);
INSERT INTO cache_retention_entries SELECT * FROM cache_retention_entries_v14_restore;
DROP TABLE cache_retention_entries_v14_restore;
CREATE INDEX cache_retention_eviction_order
ON cache_retention_entries(last_observed_revision,account_id,subject_id,facet);
CREATE TRIGGER cache_retention_entries_aggregate_insert
AFTER INSERT ON cache_retention_entries
BEGIN
    UPDATE cache_retention_state
    SET indexed_logical_bytes=indexed_logical_bytes+NEW.logical_bytes,
        indexed_facet_count=indexed_facet_count+1
    WHERE singleton=1;
END;
CREATE TRIGGER cache_retention_entries_aggregate_update
AFTER UPDATE OF logical_bytes ON cache_retention_entries
BEGIN
    UPDATE cache_retention_state
    SET indexed_logical_bytes=indexed_logical_bytes-OLD.logical_bytes+NEW.logical_bytes
    WHERE singleton=1;
END;
CREATE TRIGGER cache_retention_entries_aggregate_delete
AFTER DELETE ON cache_retention_entries
BEGIN
    UPDATE cache_retention_state
    SET indexed_logical_bytes=indexed_logical_bytes-OLD.logical_bytes,
        indexed_facet_count=indexed_facet_count-1
    WHERE singleton=1;
END;
"#,
    )
    .execute(&mut *db)
    .await
    .unwrap();
}

#[tokio::test]
async fn frozen_v14_command_history_migrates_and_preserves_terminal_receipts() {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("current.db");
    let backup = dir.path().join("v14.db");
    let store = setup(&target).await;
    let mut db = connect(&target, false).await.unwrap();
    sqlx::raw_sql("INSERT INTO delivery_attempts VALUES('a','123e4567-e89b-12d3-a456-426614174000',1,1,'2026-10-08T00:00:00Z','confirmed','2026-10-08T00:00:01Z'); INSERT INTO command_evidence VALUES('a','123e4567-e89b-12d3-a456-426614174000',0,1,'provider.receipt',1,x'010203','2026-10-08T00:00:01Z'); UPDATE commands SET state='confirmed' WHERE account_id='a' AND command_id='123e4567-e89b-12d3-a456-426614174000';").execute(&mut db).await.unwrap();
    db.close().await.unwrap();
    store.backup_to(&backup).await.unwrap();
    store.close().await.unwrap();
    drop(store);
    // Materialize exactly the pre-0015 schema with production-sealed v1 bytes.
    // The separate migration fixture pins all 0001..0014 SQL/checksums.
    let mut db = connect(&backup, false).await.unwrap();
    materialize_v14_detail_schema(&mut db).await;
    sqlx::raw_sql("DROP TABLE repository_metadata_options; DROP TABLE repository_metadata_catalogs; DROP TABLE issue_draft_metadata; DROP TRIGGER github_review_submitted_requires_resolution; DROP INDEX github_review_submitted_receipt_id; DROP INDEX github_review_accepted_receipt_id; DROP TABLE review_confirmations; DROP TABLE review_resolutions; DROP TABLE review_submissions; DROP TABLE review_draft_authority; DROP TABLE review_draft_comments; DROP TABLE review_drafts; DROP INDEX pull_created_receipt_id; DROP TABLE pull_creation_visibility; DROP TABLE pull_resolutions; DROP TABLE pull_submissions; DROP TABLE pull_drafts; DROP INDEX issue_created_receipt_id; DROP TABLE issue_creation_visibility; DROP TABLE issue_resolutions; DROP TABLE issue_submissions; DROP TABLE issue_drafts; DROP INDEX comment_created_receipt_id; DROP TABLE comment_submissions; DROP TABLE comment_drafts; DROP TABLE command_supersessions; DROP TABLE command_recovery_actions; DROP TABLE command_user_controls; DROP INDEX command_recovery_pending; DROP INDEX command_recovery_target_history; DROP INDEX command_recovery_target_pending; DROP VIEW effective_items; DROP TABLE effective_items_fts; DROP TABLE effective_item_overrides; DROP TABLE effective_item_revisions; DROP TABLE command_effects; DROP INDEX command_effect_targets; DROP INDEX command_delivery_pending; DROP INDEX command_delivery_target_order; DROP INDEX command_delivery_active_accounts; DROP TRIGGER command_delivery_admitted; DROP TABLE delivery_resolutions; DROP TABLE delivery_attempt_context; DROP TABLE command_delivery; DROP TRIGGER quarantined_attempt_refused; DROP TABLE command_recovery_quarantine; DROP TABLE recovery_meta; DELETE FROM _sqlx_migrations WHERE version>=15;").execute(&mut db).await.unwrap();
    assert_eq!(verify(&mut db).await.unwrap(), 14);
    db.close().await.unwrap();
    let original = command_rows(&backup).await;
    let hash = hash_file(&backup).unwrap();
    let session = RecoverySession::prepare(&target, &backup).await.unwrap();
    assert_eq!(session.preview().incoming.schema_version, 14);
    assert_eq!(session.preview().quarantined_commands, 2);
    let id = session.preview().confirmation_id.clone();
    session
        .confirm(&id, RestoreChoice::ReplaceCurrentData)
        .unwrap();
    assert_eq!(hash_file(&backup).unwrap(), hash);
    assert_eq!(command_rows(&target).await, original);
    assert_eq!(count(&target, "delivery_attempts").await, 1);
    assert_eq!(count(&target, "command_evidence").await, 1);
    let mut db = connect(&target, false).await.unwrap();
    let mut tx = db.begin().await.unwrap();
    assert!(!command_quarantined_in(&mut tx, "a", FIRST).await.unwrap());
    assert!(command_quarantined_in(&mut tx, "a", SECOND).await.unwrap());
    tx.rollback().await.unwrap();
    db.close().await.unwrap();
}

#[tokio::test]
async fn cache_reset_failure_rolls_back_recovery_generation_and_every_quarantine_row() {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("current.db");
    let store = setup(&target).await;
    store.close().await.unwrap();
    drop(store);
    let original_commands = command_rows(&target).await;
    let mut db = connect(&target, false).await.unwrap();
    sqlx::raw_sql("CREATE TRIGGER fail_recovery_reset BEFORE DELETE ON items BEGIN SELECT RAISE(ABORT,'synthetic reset failure'); END;").execute(&mut db).await.unwrap();
    assert!(fence_restored_data(&mut db, None).await.is_err());
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT generation FROM recovery_meta")
            .fetch_one(&mut db)
            .await
            .unwrap(),
        0
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM command_recovery_quarantine")
            .fetch_one(&mut db)
            .await
            .unwrap(),
        0
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM pull_file_artifacts")
            .fetch_one(&mut db)
            .await
            .unwrap(),
        1
    );
    db.close().await.unwrap();
    assert_eq!(command_rows(&target).await, original_commands);
}

#[tokio::test]
async fn native_remote_paths_longer_than_identity_keys_remain_recoverable() {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("current.db");
    let backup = dir.path().join("backup.db");
    let store = setup(&target).await;
    let path = format!("{}repo", "subgroup/".repeat(150));
    assert!(path.len() > 1024 && path.len() < 2048);
    let mut db = connect(&target, false).await.unwrap();
    sqlx::query(
        "UPDATE local_repository_links SET endpoint_json=json_set(endpoint_json,'$.path',?)",
    )
    .bind(&path)
    .execute(&mut db)
    .await
    .unwrap();
    sqlx::query("UPDATE local_transport_bindings SET path_prefix=?")
        .bind(&path)
        .execute(&mut db)
        .await
        .unwrap();
    db.close().await.unwrap();
    store.backup_to(&backup).await.unwrap();
    store.close().await.unwrap();
    drop(store);
    restore(&target, &backup).await;
    let mut db = connect(&target, true).await.unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT path_prefix FROM local_transport_bindings")
            .fetch_one(&mut db)
            .await
            .unwrap(),
        path
    );
    db.close().await.unwrap();
}

#[tokio::test]
async fn delivery_context_and_resolution_survive_restore_but_old_schedule_cannot_run() {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("delivery.db");
    let backup = dir.path().join("delivery-backup.db");
    let store = setup(&target).await;
    let mut db = connect(&target, false).await.unwrap();
    sqlx::raw_sql("INSERT INTO delivery_attempts VALUES('a','123e4567-e89b-12d3-a456-426614174000',1,1,'2026-10-08T00:00:00Z','accepted','2026-10-08T00:00:01Z'); INSERT INTO delivery_attempt_context VALUES('a','123e4567-e89b-12d3-a456-426614174000',1,'github:https://github.com/',x'006f7061717565ff'); INSERT INTO command_evidence VALUES('a','123e4567-e89b-12d3-a456-426614174000',0,1,'future.receipt',1,x'0070726f6f66ff','2026-10-08T00:00:01Z'); UPDATE command_delivery SET generation=2,next_action_at='2026-10-09T00:00:00Z',reconciliation_count=7,attention='reconciliation_limit' WHERE account_id='a' AND command_id='123e4567-e89b-12d3-a456-426614174000'; INSERT INTO delivery_resolutions VALUES('a','123e4567-e89b-12d3-a456-426614174000',2,0,'accepted'); UPDATE commands SET state='accepted' WHERE account_id='a' AND command_id='123e4567-e89b-12d3-a456-426614174000';").execute(&mut db).await.unwrap();
    let context: Vec<u8> =
        sqlx::query_scalar("SELECT execution_base FROM delivery_attempt_context")
            .fetch_one(&mut db)
            .await
            .unwrap();
    for sql in [
        "DELETE FROM delivery_attempt_context",
        "UPDATE delivery_attempt_context SET execution_base=x''",
        "DELETE FROM delivery_resolutions",
        "UPDATE delivery_resolutions SET purpose='confirmed'",
    ] {
        assert!(sqlx::query(sql).execute(&mut db).await.is_err());
    }
    db.close().await.unwrap();
    store.backup_to(&backup).await.unwrap();
    store.close().await.unwrap();
    restore(&target, &backup).await;
    let mut db = connect(&target, true).await.unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, Vec<u8>>("SELECT execution_base FROM delivery_attempt_context")
            .fetch_one(&mut db)
            .await
            .unwrap(),
        context
    );
    let schedule:(i64,Option<String>,i64,Option<String>)=sqlx::query_as("SELECT generation,next_action_at,reconciliation_count,attention FROM command_delivery WHERE account_id='a' AND command_id='123e4567-e89b-12d3-a456-426614174000'").fetch_one(&mut db).await.unwrap();
    assert_eq!(schedule, (3, None, 0, None));
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT purpose FROM delivery_resolutions")
            .fetch_one(&mut db)
            .await
            .unwrap(),
        "accepted"
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM command_recovery_quarantine")
            .fetch_one(&mut db)
            .await
            .unwrap(),
        3
    );
    db.close().await.unwrap();
}
