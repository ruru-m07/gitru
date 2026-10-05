//! Frozen v8 input qualifies the actual forward rebuild, independently of today's
//! DTO serializers. The injected fault uses the production SQLx transaction and
//! production 0009 statements, not a second migration implementation.
use std::path::Path;

use collaboration::{
    CoverageState, DetailAvailability, DetailFacet, DetailQuery, DetailValueState, ErrorCode,
    LocalDraft, Store, SyncState,
};
use sha2::{Digest, Sha384};
use sqlx::{
    AssertSqlSafe, Connection, SqlSafeStr, SqliteConnection,
    migrate::{MigrateError, Migration, MigrationType, Migrator},
    sqlite::{SqliteConnectOptions, SqliteJournalMode, SqliteSynchronous},
};

const V8_SQL: [&str; 8] = [
    include_str!("fixtures/migrations/v8/0001_local_collaboration.sql"),
    include_str!("fixtures/migrations/v8/0002_credential_cutover.sql"),
    include_str!("fixtures/migrations/v8/0003_provider_identities.sql"),
    include_str!("fixtures/migrations/v8/0004_detail_scopes.sql"),
    include_str!("fixtures/migrations/v8/0005_resource_metadata.sql"),
    include_str!("fixtures/migrations/v8/0006_local_repository_links.sql"),
    include_str!("fixtures/migrations/v8/0007_notification_subjects.sql"),
    include_str!("fixtures/migrations/v8/0008_participant_facets.sql"),
];
const V8_SEED: &str = include_str!("fixtures/migrations/v8/seed.sql");
const V8_CHECKSUMS: &str = include_str!("fixtures/migrations/v8/checksums.sha384");
const V9_SQL: &str = include_str!("../migrations/0009_task_facets.sql");
static CURRENT: Migrator = sqlx::migrate!("./migrations");

// JSON strings are wrapped, not parsed or re-serialized. This compares the
// exact stored source/clock/payload bytes as well as every historical column.
const SNAPSHOT_QUERIES: &[&str] = &[
    "SELECT json_array(singleton,revision,authorization_view,log_floor) FROM runtime_meta ORDER BY singleton",
    "SELECT json_array(id,provider,host,actor_id,authorization_epoch,state,json) FROM accounts ORDER BY id",
    "SELECT json_array(account_id,id,provider_id,full_name,selected,json) FROM repositories ORDER BY account_id,id",
    "SELECT json_array(account_id,id,repository_id,kind,state,updated_at,json) FROM items ORDER BY account_id,id",
    "SELECT json_array(account_id,id,title,body) FROM items_fts ORDER BY account_id,id",
    "SELECT json_array(account_id,scope,run_id,data_revision,completed_run_id,next_cursor,etag,last_modified,access_denied,coverage_json,sync_json) FROM sync_scopes ORDER BY account_id,scope",
    "SELECT json_array(account_id,scope,entity_id,last_seen_run,active,missing_count) FROM scope_membership ORDER BY account_id,scope,entity_id",
    "SELECT json_array(account_id,subject_id,body,generation) FROM drafts ORDER BY account_id,subject_id",
    "SELECT json_array(revision,account_id,authorization_epoch,scope,reset) FROM change_log ORDER BY revision",
    "SELECT json_array(account_id,credential_ref) FROM account_credentials ORDER BY account_id",
    "SELECT json_array(credential_ref,account_id,state,attempts,next_retry_at) FROM credential_cleanup ORDER BY credential_ref",
    "SELECT json_array(id,provider,base_url) FROM provider_instances ORDER BY id",
    "SELECT json_array(account_id,instance_id) FROM account_instances ORDER BY account_id",
    "SELECT json_array(account_id,instance_id,entity_id,kind,provider_id,repository_provider_id,number) FROM resource_identities ORDER BY account_id,instance_id,entity_id",
    "SELECT json_array(account_id,instance_id,kind,alias_kind,value,repository_path,entity_id) FROM resource_aliases ORDER BY account_id,instance_id,kind,alias_kind,value,repository_path,entity_id",
    "SELECT json_array(account_id,instance_id,kind,repository_provider_id,number,native_identity,web_url) FROM pending_endpoint_aliases ORDER BY account_id,instance_id,kind,repository_provider_id,number,native_identity",
    "SELECT json_array(account_id,subject_id,facet,authorization_epoch,facet_revision,body_json,source_json,value_source_json,observed_state,stale_at) FROM detail_observations ORDER BY account_id,subject_id,facet",
    "SELECT json_array(account_id,subject_id,facet,id,json,last_seen_run) FROM detail_entries ORDER BY account_id,subject_id,facet,id",
    "SELECT json_array(account_id,subject_id,facet,authorization_epoch,requested) FROM detail_demand ORDER BY account_id,subject_id,facet",
    "SELECT json_array(account_id,subject_id,facet,authorization_epoch,metadata_json,source_json) FROM detail_resource_metadata ORDER BY account_id,subject_id",
    "SELECT json_array(singleton,bindings_generation) FROM local_link_meta ORDER BY singleton",
    "SELECT json_array(id,instance_id,transport,host,port,path_prefix,layout,generation) FROM local_transport_bindings ORDER BY id",
    "SELECT json_array(id,local_repository_id,endpoint_json,account_id,instance_id,actor_id,repository_id,repository_provider_id,registration_proof,remote_digest,generation) FROM local_repository_links ORDER BY id",
    "SELECT json_array(account_id,notification_id,instance_id,authorization_epoch,selector_generation,mapping_json,kind,repository_provider_id,number) FROM notification_subject_selectors ORDER BY account_id,notification_id",
    "SELECT json_array(account_id,notification_id,authorization_epoch,selector_generation,intent_generation,requested,attempts,run_id,outcome_reason) FROM notification_subject_discovery ORDER BY account_id,notification_id",
];

async fn connect(path: &Path) -> SqliteConnection {
    let mut connection = SqliteConnection::connect_with(
        &SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(true)
            .foreign_keys(true)
            .journal_mode(SqliteJournalMode::Wal)
            .synchronous(SqliteSynchronous::Full),
    )
    .await
    .unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT sqlite_version()")
            .fetch_one(&mut connection)
            .await
            .unwrap(),
        "3.51.3",
        "This gate qualifies the bundled pinned SQLite engine"
    );
    connection
}

async fn frozen_v8(path: &Path) {
    let mut connection = connect(path).await;
    let mut tx = connection.begin().await.unwrap();
    for sql in V8_SQL {
        sqlx::raw_sql(sql).execute(&mut *tx).await.unwrap();
    }
    sqlx::raw_sql(V8_SEED).execute(&mut *tx).await.unwrap();
    tx.commit().await.unwrap();
    assert_integrity(&mut connection).await;
    connection.close().await.unwrap();
}

async fn snapshot(connection: &mut SqliteConnection) -> Vec<Vec<String>> {
    let mut result = Vec::new();
    for query in SNAPSHOT_QUERIES {
        result.push(
            sqlx::query_scalar(*query)
                .fetch_all(&mut *connection)
                .await
                .unwrap(),
        );
    }
    result
}

async fn ledger(connection: &mut SqliteConnection) -> Vec<String> {
    sqlx::query_scalar("SELECT json_array(version,description,installed_on,success,hex(checksum),execution_time) FROM _sqlx_migrations ORDER BY version")
        .fetch_all(connection).await.unwrap()
}

async fn schema(connection: &mut SqliteConnection) -> Vec<String> {
    sqlx::query_scalar(
        "SELECT json_array(type,name,tbl_name,sql) FROM sqlite_schema ORDER BY type,name",
    )
    .fetch_all(connection)
    .await
    .unwrap()
}

async fn assert_integrity(connection: &mut SqliteConnection) {
    assert_eq!(
        sqlx::query_scalar::<_, i64>("PRAGMA foreign_keys")
            .fetch_one(&mut *connection)
            .await
            .unwrap(),
        1,
        "The forward rebuild never disables FK enforcement"
    );
    assert_eq!(
        sqlx::query_scalar::<_, String>("PRAGMA integrity_check")
            .fetch_one(&mut *connection)
            .await
            .unwrap(),
        "ok"
    );
    assert!(
        sqlx::query("PRAGMA foreign_key_check")
            .fetch_all(&mut *connection)
            .await
            .unwrap()
            .is_empty()
    );
}

fn detail_query(account_id: &str, facet: DetailFacet) -> DetailQuery {
    DetailQuery {
        account_id: account_id.into(),
        subject_id: "pull-request-67".into(),
        facet,
        cursor: None,
        limit: 100,
    }
}

async fn assert_cold_saved_reads(store: &Store) {
    assert_eq!(store.revision().await.unwrap(), "9004");
    let accounts = store.accounts().await.unwrap();
    assert_eq!(accounts.authorization_view, "11");
    assert_eq!(accounts.accounts.len(), 4);
    for (account_id, epoch, login) in [("a", "17", "alice"), ("b", "29", "bob")] {
        let account = store.account(account_id).await.unwrap();
        assert_eq!(account.authorization_epoch, epoch);
        assert_eq!(account.login, login);
        assert!(
            store
                .repository(account_id, "repository-42")
                .await
                .unwrap()
                .selected
        );
        assert_eq!(
            store
                .credential_reference(account_id)
                .await
                .unwrap()
                .unwrap(),
            format!("fixture-reference-{account_id}")
        );
        for facet in [
            DetailFacet::Body,
            DetailFacet::Comments,
            DetailFacet::Reviews,
            DetailFacet::Checks,
        ] {
            let saved = store.detail(detail_query(account_id, facet)).await.unwrap();
            assert_eq!(saved.revision, "9004");
            assert_eq!(saved.authorization_view, "11");
            assert_eq!(saved.evidence.authorization_epoch, epoch);
            assert_eq!(saved.evidence.availability, DetailAvailability::Ready);
            assert_eq!(saved.evidence.coverage.state, CoverageState::Complete);
            assert_eq!(saved.evidence.sync.state, SyncState::Offline);
            assert!(saved.next_cursor.is_none());
            if facet == DetailFacet::Body {
                assert_eq!(saved.body.state, DetailValueState::Known);
                assert_eq!(
                    saved.body.text.as_deref(),
                    Some(format!("{account_id} authoritative saved Body").as_str())
                );
                assert_eq!(saved.evidence.observed_state, DetailValueState::Omitted);
                assert!(saved.entries.is_empty());
                assert_eq!(
                    saved.metadata.unwrap().values.title.as_deref(),
                    Some(format!("{account_id} authoritative metadata title").as_str())
                );
            } else {
                assert_eq!(saved.entries.len(), 1);
                let entry = &saved.entries[0];
                assert_eq!(entry.id, format!("{}-entry-{account_id}", facet.name()));
                assert_eq!(
                    entry.author.as_deref(),
                    Some(format!("{account_id}-reviewer").as_str())
                );
                assert!(
                    entry.native.is_none(),
                    "Missing legacy native JSON defaults to None"
                );
                assert_eq!(entry.field_mask.len(), 6);
                assert_eq!(entry.field_validations.len(), 6);
                assert_eq!(
                    entry.field_validations[0].source,
                    format!("frozen-v7/{}/v1", facet.name())
                );
                assert!(saved.metadata.is_none());
            }
        }
        let participants = store
            .detail(detail_query(account_id, DetailFacet::Participants))
            .await
            .unwrap();
        assert_eq!(
            participants.evidence.availability,
            DetailAvailability::Ready
        );
        assert_eq!(
            participants.evidence.coverage.state,
            CoverageState::Complete
        );
        assert_eq!(participants.evidence.sync.state, SyncState::Offline);
        assert_eq!(participants.entries.len(), 1);
        let record = &participants.entries[0];
        let Some(collaboration::NativeDetailPayload::ParticipantV1(value)) = &record.native else {
            panic!("Frozen v8 typed participant survives normal cold reads")
        };
        assert_eq!(
            value.user.provider_id,
            "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa"
        );
        assert_eq!(value.user.login.as_deref(), Some("same-name"));
        assert_eq!(
            value.user.display_name.as_deref(),
            Some(format!("{account_id} saved participant café 🦀").as_str())
        );
        assert_eq!(value.role.as_deref(), Some("FUTURE_OBSERVER"));
        assert_eq!(value.approved, Some(account_id == "b"));
        assert_eq!(value.state, None);
        assert_eq!(value.participated_at, None);
        assert_eq!(record.field_mask.len(), 6);
        assert_eq!(record.field_validations.len(), 6);
        assert!(
            record
                .field_validations
                .iter()
                .all(|field| field.source == "frozen-v8/participants/v1")
        );
        let tasks = store
            .detail(detail_query(account_id, DetailFacet::Tasks))
            .await
            .unwrap();
        assert_eq!(tasks.evidence.availability, DetailAvailability::Missing);
        assert!(
            tasks.entries.is_empty(),
            "A migration must not invent task observations"
        );
    }
    let pending = store.pending_details().await.unwrap();
    assert_eq!(pending.len(), 10);
    for account in ["a", "b"] {
        for facet in [
            DetailFacet::Body,
            DetailFacet::Comments,
            DetailFacet::Reviews,
            DetailFacet::Checks,
            DetailFacet::Participants,
        ] {
            assert!(pending.iter().any(|demand| {
                demand.account_id == account
                    && demand.subject_id == "pull-request-67"
                    && demand.facet == facet
            }));
        }
    }
    for (account, subject, body, generation) in [
        (
            "a",
            "pull-request-67",
            "Alice unsent draft with a newline\nand Unicode: café 🦀",
            "37",
        ),
        ("b", "pull-request-67", "Bob independent unsent draft", "51"),
        ("c", "missing-subject", "Carol draft after disconnect", "63"),
        (
            "d",
            "missing-subject",
            "Dave draft awaiting reconnection",
            "79",
        ),
    ] {
        let draft = store.draft(account, subject).await.unwrap().unwrap();
        assert_eq!(draft.body, body);
        assert_eq!(draft.generation, generation);
    }
    assert_eq!(
        store.revision().await.unwrap(),
        "9004",
        "Saved reads do not emit sync changes"
    );
    // This helper constructs only Store. There is no Runtime, HTTP transport or
    // vault implementation to contact while restoring these saved snapshots.
}

#[test]
fn frozen_v8_sql_and_applied_checksums_remain_immutable() {
    let checksums: Vec<_> = V8_CHECKSUMS
        .lines()
        .map(|line| line.split_once("  ").unwrap().0)
        .collect();
    assert_eq!(checksums.len(), 9);
    for (index, sql) in V8_SQL.iter().enumerate() {
        let historical = format!("{:x}", Sha384::digest(sql));
        assert_eq!(historical, checksums[index]);
        let production = CURRENT
            .iter()
            .find(|migration| migration.version == index as i64 + 1)
            .unwrap();
        assert_eq!(
            format!("{:x}", Sha384::digest(production.sql.as_str())),
            historical,
            "Applied migration {} must remain unchanged",
            index + 1
        );
    }
    assert_eq!(format!("{:x}", Sha384::digest(V8_SEED)), checksums[8]);
}

#[tokio::test]
async fn frozen_v8_schema_rejects_tasks_before_the_forward_migration() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("historical-v8.db");
    frozen_v8(&path).await;
    let mut connection = connect(&path).await;
    let before = snapshot(&mut connection).await;
    let before_ledger = ledger(&mut connection).await;
    assert_eq!(before_ledger.len(), 8);
    for sql in [
        "INSERT INTO detail_observations VALUES ('a','historical-task','tasks','17','9110','{}','{}',NULL,'known',NULL)",
        "INSERT INTO detail_demand VALUES ('a','historical-task','tasks','17',1)",
    ] {
        assert!(sqlx::query(sql).execute(&mut connection).await.is_err());
    }
    assert_eq!(snapshot(&mut connection).await, before);
    assert_eq!(ledger(&mut connection).await, before_ledger);
    assert_integrity(&mut connection).await;
    connection.close().await.unwrap();
}

#[tokio::test]
async fn frozen_v8_upgrade_preserves_every_row_and_immediate_cold_reads() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("collaboration.db");
    frozen_v8(&path).await;
    let mut connection = connect(&path).await;
    let before = snapshot(&mut connection).await;
    let before_ledger = ledger(&mut connection).await;
    assert_eq!(before_ledger.len(), 8);
    connection.close().await.unwrap();

    for _ in 0..2 {
        let store = Store::open(&path).await.unwrap();
        assert_cold_saved_reads(&store).await;
        store.close().await;
        drop(store);
        let mut connection = connect(&path).await;
        assert_eq!(snapshot(&mut connection).await, before);
        let after_ledger = ledger(&mut connection).await;
        assert_eq!(&after_ledger[..8], &before_ledger);
        let applied: Vec<i64> =
            sqlx::query_scalar("SELECT version FROM _sqlx_migrations ORDER BY version")
                .fetch_all(&mut connection)
                .await
                .unwrap();
        assert_eq!(
            applied,
            CURRENT
                .iter()
                .map(|migration| migration.version)
                .collect::<Vec<_>>()
        );
        let checksum: Vec<u8> =
            sqlx::query_scalar("SELECT checksum FROM _sqlx_migrations WHERE version=9")
                .fetch_one(&mut connection)
                .await
                .unwrap();
        assert_eq!(checksum, Sha384::digest(V9_SQL).to_vec());
        let leftovers: i64 = sqlx::query_scalar("SELECT count(*) FROM sqlite_schema WHERE type='table' AND (name LIKE 'detail%_v9' OR name='detail_facets_v9_guard')")
            .fetch_one(&mut connection).await.unwrap();
        assert_eq!(leftovers, 0);
        assert_integrity(&mut connection).await;
        connection.close().await.unwrap();
    }
}

#[tokio::test]
async fn actual_task_rebuild_rolls_back_after_old_parent_drop() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("collaboration.db");
    frozen_v8(&path).await;
    let mut connection = connect(&path).await;
    let before = snapshot(&mut connection).await;
    let before_schema = schema(&mut connection).await;
    let before_ledger = ledger(&mut connection).await;
    let boundary = "DROP TABLE detail_observations;";
    assert_eq!(V9_SQL.matches(boundary).count(), 1);
    let fault_sql = V9_SQL.replacen(
        boundary,
        "DROP TABLE detail_observations;\nINSERT INTO nonexistent_task_migration_target VALUES (1);",
        1,
    );
    let mut migrations: Vec<_> = CURRENT
        .iter()
        .filter(|migration| migration.version < 9)
        .cloned()
        .collect();
    migrations.push(Migration::new(
        9,
        "task facets injected boundary failure".into(),
        MigrationType::Simple,
        AssertSqlSafe(fault_sql).into_sql_str(),
        false,
    ));
    let error = Migrator::with_migrations(migrations)
        .run_direct(None, &mut connection, false)
        .await
        .unwrap_err();
    assert!(matches!(&error, MigrateError::ExecuteMigration(_, 9)));
    assert!(
        error
            .to_string()
            .contains("nonexistent_task_migration_target")
    );
    assert_eq!(schema(&mut connection).await, before_schema);
    assert_eq!(snapshot(&mut connection).await, before);
    assert_eq!(ledger(&mut connection).await, before_ledger);
    assert_integrity(&mut connection).await;
    connection.close().await.unwrap();

    // The same intact historical file can immediately bootstrap normally.
    let store = Store::open(&path).await.unwrap();
    assert_cold_saved_reads(&store).await;
    store.close().await;
    drop(store);
    let mut connection = connect(&path).await;
    assert_eq!(snapshot(&mut connection).await, before);
    assert_integrity(&mut connection).await;
    connection.close().await.unwrap();
}

#[tokio::test]
async fn upgraded_schema_admits_tasks_and_preserves_body_only_metadata_and_fks() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("collaboration.db");
    frozen_v8(&path).await;
    let store = Store::open(&path).await.unwrap();
    store.close().await;
    drop(store);
    let mut connection = connect(&path).await;
    // These direct writes exercise schema admission only; provider eligibility
    // and typed native payload admission are qualified by the core/runtime tests.
    sqlx::raw_sql("INSERT INTO detail_observations VALUES ('a','schema-task-subject','tasks','17','9109','{}','{}',NULL,'known',NULL); INSERT INTO detail_entries VALUES ('a','schema-task-subject','tasks','schema-task-entry','{}','schema-run'); INSERT INTO detail_demand VALUES ('a','schema-task-subject','tasks','17',1);")
        .execute(&mut connection).await.unwrap();
    for statement in [
        "INSERT INTO detail_observations VALUES ('a','schema-future-subject','future_task','17','9110','{}','{}',NULL,'known',NULL)",
        "INSERT INTO detail_demand VALUES ('a','schema-future-subject','future_task','17',1)",
        "INSERT INTO detail_demand VALUES ('a','bad-requested-subject','participants','17',2)",
        "INSERT INTO detail_resource_metadata VALUES ('a','schema-task-subject','tasks','17','{}','{}')",
        "INSERT INTO detail_entries VALUES ('a','missing-parent','participants','orphan-entry','{}','schema-run')",
        "INSERT INTO detail_observations VALUES ('missing-account','schema-subject','participants','17','9110','{}','{}',NULL,'known',NULL)",
    ] {
        assert!(
            sqlx::query(statement)
                .execute(&mut connection)
                .await
                .is_err()
        );
    }
    assert!(
        sqlx::query("DELETE FROM accounts WHERE id='a'")
            .execute(&mut connection)
            .await
            .is_err(),
        "Account RESTRICT relationships survive the rebuild"
    );
    sqlx::query(
        "DELETE FROM detail_observations WHERE account_id='a' AND subject_id='schema-task-subject'",
    )
    .execute(&mut connection)
    .await
    .unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM detail_entries WHERE subject_id='schema-task-subject'"
        )
        .fetch_one(&mut connection)
        .await
        .unwrap(),
        0,
        "The renamed child FK still cascades from its new public parent"
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM detail_demand WHERE subject_id='schema-task-subject'"
        )
        .fetch_one(&mut connection)
        .await
        .unwrap(),
        1,
        "Retryable intent remains independent of the rebuildable observation"
    );
    sqlx::query("DELETE FROM detail_observations WHERE account_id='a' AND subject_id='pull-request-67' AND facet='body'")
        .execute(&mut connection).await.unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM detail_resource_metadata WHERE account_id='a'"
        )
        .fetch_one(&mut connection)
        .await
        .unwrap(),
        0,
        "Body metadata retains its parent cascade"
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM detail_resource_metadata WHERE account_id='b'"
        )
        .fetch_one(&mut connection)
        .await
        .unwrap(),
        1,
        "Another actor's metadata is independent"
    );
    assert_integrity(&mut connection).await;
    connection.close().await.unwrap();
}

#[tokio::test]
async fn frozen_draft_generations_keep_cas_after_upgrade_and_cold_reopen() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("collaboration.db");
    frozen_v8(&path).await;
    let store = Store::open(&path).await.unwrap();
    assert_cold_saved_reads(&store).await;
    let original = store.draft("a", "pull-request-67").await.unwrap().unwrap();
    let error = store
        .save_draft(LocalDraft {
            account_id: "a".into(),
            subject_id: "pull-request-67".into(),
            body: "An obsolete editor must not replace authored text".into(),
            generation: "36".into(),
        })
        .await
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::StaleView);
    assert_eq!(store.revision().await.unwrap(), "9004");
    assert_eq!(
        store.draft("a", "pull-request-67").await.unwrap().unwrap(),
        original
    );
    let saved = store.save_draft(original).await.unwrap();
    assert_eq!(saved.generation, "38");
    assert_eq!(store.revision().await.unwrap(), "9005");
    store.close().await;
    drop(store);
    let reopened = Store::open(&path).await.unwrap();
    assert_eq!(reopened.revision().await.unwrap(), "9005");
    assert_eq!(
        reopened
            .draft("a", "pull-request-67")
            .await
            .unwrap()
            .unwrap(),
        saved
    );
    assert_eq!(
        reopened
            .draft("b", "pull-request-67")
            .await
            .unwrap()
            .unwrap()
            .generation,
        "51"
    );
    reopened.close().await;
}
