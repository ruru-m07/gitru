//! Historical fixtures exercise Store bootstrap without serializing today's DTOs.
//! Synthetic pending migrations inject faults into the same pinned SQLx/SQLite
//! migrator used by Store; they are not additional supported Gitru schemas.
use std::{
    fs::OpenOptions,
    path::Path,
    process::{Child, Command, Stdio},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

use collaboration::{domain::*, error::ErrorCode, storage::Store};
use sha2::{Digest, Sha384};
use sqlx::{
    AssertSqlSafe, Connection, SqlSafeStr, SqliteConnection,
    migrate::{MigrateError, Migration, MigrationType, Migrator},
    sqlite::{SqliteConnectOptions, SqliteJournalMode, SqliteSynchronous},
};

const V1_SCHEMA: &str = include_str!("fixtures/migrations/v1/schema.sql");
const V1_SEED: &str = include_str!("fixtures/migrations/v1/seed.sql");
const V1_CHECKSUM: &str = include_str!("fixtures/migrations/v1/checksum.sha384");
static CURRENT: Migrator = sqlx::migrate!("./migrations");
const PROBE_VERSION: i64 = 9001;

// Only these frozen v1 tables belong to this historical snapshot. Later
// migrations may add tables, but must preserve the existing user observations,
// authorization boundaries, and authored intent unless explicitly converted.
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
];

async fn connect(path: &Path) -> SqliteConnection {
    SqliteConnection::connect_with(
        &SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(true)
            .foreign_keys(true)
            .journal_mode(SqliteJournalMode::Wal)
            .synchronous(SqliteSynchronous::Full),
    )
    .await
    .unwrap()
}

async fn frozen_v1(path: &Path) {
    let mut connection = connect(path).await;
    let mut tx = connection.begin().await.unwrap();
    sqlx::raw_sql(V1_SCHEMA).execute(&mut *tx).await.unwrap();
    sqlx::raw_sql(V1_SEED).execute(&mut *tx).await.unwrap();
    tx.commit().await.unwrap();
    connection.close().await.unwrap();
}

async fn snapshot(connection: &mut SqliteConnection) -> Vec<Vec<String>> {
    let mut snapshot = Vec::new();
    for query in SNAPSHOT_QUERIES {
        snapshot.push(
            sqlx::query_scalar(*query)
                .fetch_all(&mut *connection)
                .await
                .unwrap(),
        );
    }
    snapshot
}

async fn ledger(connection: &mut SqliteConnection) -> Vec<String> {
    sqlx::query_scalar("SELECT json_array(version,description,installed_on,success,hex(checksum),execution_time) FROM _sqlx_migrations ORDER BY version")
        .fetch_all(connection).await.unwrap()
}

async fn credential_metadata(connection: &mut SqliteConnection) -> Vec<Vec<String>> {
    let mut snapshots = Vec::new();
    for query in [
        "SELECT json_array(account_id,credential_ref) FROM account_credentials ORDER BY account_id",
        "SELECT json_array(credential_ref,account_id,state,attempts,next_retry_at) FROM credential_cleanup ORDER BY credential_ref",
    ] {
        snapshots.push(
            sqlx::query_scalar(query)
                .fetch_all(&mut *connection)
                .await
                .unwrap(),
        );
    }
    snapshots
}

fn query(account: &str, kind: RemoteItemKind, search: Option<&str>) -> ItemQuery {
    ItemQuery {
        account_id: account.into(),
        kind,
        repository_id: None,
        state: None,
        search: search.map(str::to_owned),
        cursor: None,
        limit: 100,
    }
}

async fn assert_historical_reads(store: &Store) {
    let accounts = store.accounts().await.unwrap();
    assert_eq!(accounts.revision, "9004");
    assert_eq!(accounts.authorization_view, "11");
    assert_eq!(accounts.accounts.len(), 4);
    for (id, actor, login, epoch, state) in [
        ("a", "100", "alice", "17", AccountState::Active),
        ("b", "101", "bob", "29", AccountState::Active),
        ("c", "102", "carol", "41", AccountState::Disconnected),
        ("d", "103", "dave", "53", AccountState::AuthRequired),
    ] {
        let account = store.account(id).await.unwrap();
        assert_eq!(account.actor_id, actor);
        assert_eq!(account.login, login);
        assert_eq!(account.authorization_epoch, epoch);
        assert_eq!(account.state, state);
        assert_eq!(account.provider, ProviderKind::Github);
        assert_eq!(account.host, "github.com");
    }
    for (account, own_term, other_term, head) in [
        ("a", "AlphaOnly", "BetaOnly", "alice-head"),
        ("b", "BetaOnly", "AlphaOnly", "bob-head"),
    ] {
        let repository = store.repository(account, "repository-42").await.unwrap();
        assert_eq!(repository.account_id, account);
        assert_eq!(repository.provider_id, "42");
        assert_eq!(repository.full_name, "fixture/project");
        assert!(repository.selected);
        let own = store
            .query_items(query(account, RemoteItemKind::PullRequest, Some(own_term)))
            .await
            .unwrap();
        assert_eq!(own.items.len(), 1);
        assert_eq!(own.items[0].account_id, account);
        assert_eq!(own.items[0].id, "pull-request-67");
        assert_eq!(own.items[0].head_oid.as_deref(), Some(head));
        assert!(
            store
                .query_items(query(
                    account,
                    RemoteItemKind::PullRequest,
                    Some(other_term)
                ))
                .await
                .unwrap()
                .items
                .is_empty(),
            "An overlapping provider ID cannot make another account's FTS row visible"
        );
    }
    assert!(
        store
            .query_items(query("a", RemoteItemKind::Issue, Some("DeniedOnly")))
            .await
            .unwrap()
            .items
            .is_empty()
    );
    assert!(store.item("a", "issue-68").await.unwrap().item.is_none());
    assert_eq!(
        store.item("c", "pull-request-67").await.unwrap_err().code,
        ErrorCode::AuthRequired
    );
    let partial = store
        .scope_state("a", "repo:repository-42:pull_request")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(partial.run_id, "partial-a");
    assert_eq!(partial.next_cursor.as_deref(), Some("page-2-a"));
    assert_eq!(partial.etag.as_deref(), Some("private-validator-a"));
    assert_eq!(partial.coverage.state, CoverageState::Partial);
    assert!(partial.coverage.remote_has_more);
    assert_eq!(partial.sync.state, SyncState::Offline);
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
        assert_eq!(draft.account_id, account);
    }
    let changes = store.changes_since("8998").await.unwrap();
    assert_eq!(changes.revision, "9004");
    assert_eq!(changes.authorization_view, "11");
    assert!(changes.reset_required);
    assert_eq!(changes.changes.len(), 5, "Obsolete epochs remain fenced");
    assert_eq!(changes.changes[0].revision, "9000");
    assert!(!changes.has_more);
    assert!(store.changes_since("8997").await.unwrap().reset_required);
}

async fn assert_integrity(connection: &mut SqliteConnection) {
    let check: String = sqlx::query_scalar("PRAGMA integrity_check")
        .fetch_one(&mut *connection)
        .await
        .unwrap();
    assert_eq!(check, "ok");
    assert!(
        sqlx::query("PRAGMA foreign_key_check")
            .fetch_all(connection)
            .await
            .unwrap()
            .is_empty()
    );
}

#[test]
fn historical_schema_checksum_is_frozen() {
    assert_eq!(
        format!("{:x}", Sha384::digest(V1_SCHEMA)),
        V1_CHECKSUM.trim()
    );
    assert_eq!(
        format!(
            "{:x}",
            Sha384::digest(CURRENT.iter().next().unwrap().sql.as_str())
        ),
        V1_CHECKSUM.trim(),
        "Applied v1 SQL must remain immutable; add a forward migration"
    );
}

#[tokio::test]
async fn frozen_v1_opens_with_latest_schema_and_preserves_private_intent() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("collaboration.db");
    frozen_v1(&path).await;
    let mut connection = connect(&path).await;
    let before = snapshot(&mut connection).await;
    connection.close().await.unwrap();

    for _ in 0..2 {
        let store = Store::open(&path).await.unwrap();
        assert_historical_reads(&store).await;
        store.close().await;
        drop(store);
        let mut connection = connect(&path).await;
        assert_eq!(snapshot(&mut connection).await, before);
        let applied: Vec<i64> =
            sqlx::query_scalar("SELECT version FROM _sqlx_migrations ORDER BY version")
                .fetch_all(&mut connection)
                .await
                .unwrap();
        let expected: Vec<_> = CURRENT.iter().map(|migration| migration.version).collect();
        assert_eq!(applied, expected, "All current forward migrations must run");
        let local_inbox_rows: i64 = sqlx::query_scalar(
            "SELECT (SELECT count(*) FROM local_inbox_state) + (SELECT count(*) FROM local_inbox_projection)",
        )
        .fetch_one(&mut connection)
        .await
        .unwrap();
        assert_eq!(
            local_inbox_rows, 0,
            "An upgrade must not invent user-authored inbox intent"
        );
        assert_integrity(&mut connection).await;
        connection.close().await.unwrap();
    }
}

#[tokio::test]
async fn frozen_v1_upgrade_preserves_legacy_vault_references_and_cleanup_work() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("collaboration.db");
    frozen_v1(&path).await;
    for _ in 0..2 {
        let store = Store::open(&path).await.unwrap();
        for account in ["a", "b", "d"] {
            assert_eq!(
                store
                    .credential_reference(account)
                    .await
                    .unwrap()
                    .as_deref(),
                Some(account)
            );
        }
        assert!(store.credential_reference("c").await.unwrap().is_none());
        let due = store.due_credential_cleanup(0, 32).await.unwrap();
        assert_eq!(
            due.len(),
            1,
            "Legacy active references must never enter cleanup"
        );
        assert_eq!(due[0].reference, "c");
        assert_eq!(due[0].attempts, 0);
        store.close().await;
        drop(store);
        let mut connection = connect(&path).await;
        assert_eq!(
            credential_metadata(&mut connection).await,
            [
                vec!["[\"a\",\"a\"]", "[\"b\",\"b\"]", "[\"d\",\"d\"]"],
                vec!["[\"c\",\"c\",\"retired\",0,0]"],
            ]
        );
        assert_integrity(&mut connection).await;
        connection.close().await.unwrap();
    }
}

#[tokio::test]
async fn historical_v1_migrator_refuses_upgraded_storage_without_down_migrating_intent() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("collaboration.db");
    frozen_v1(&path).await;
    let store = Store::open(&path).await.unwrap();
    store.close().await;
    drop(store);
    let mut connection = connect(&path).await;
    let before = snapshot(&mut connection).await;
    let before_ledger = ledger(&mut connection).await;
    let before_credentials = credential_metadata(&mut connection).await;
    let historical = Migrator::with_migrations(vec![Migration::new(
        1,
        "local collaboration".into(),
        MigrationType::Simple,
        V1_SCHEMA.into_sql_str(),
        false,
    )]);
    assert!(matches!(
        historical.run_direct(None, &mut connection, false).await,
        Err(MigrateError::VersionMissing(2))
    ));
    assert_eq!(snapshot(&mut connection).await, before);
    assert_eq!(ledger(&mut connection).await, before_ledger);
    assert_eq!(
        credential_metadata(&mut connection).await,
        before_credentials
    );
    assert_integrity(&mut connection).await;
    connection.close().await.unwrap();
    let store = Store::open(&path).await.unwrap();
    assert_historical_reads(&store).await;
    store.close().await;
}

#[derive(Clone, Copy, Debug)]
enum IncompatibleLedger {
    Future,
    Dirty,
    ChangedChecksum,
}

async fn rejected_upgrade_preserves_data_and_releases_writer_lease(
    fault: IncompatibleLedger,
    upgraded: bool,
) {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("collaboration.db");
    frozen_v1(&path).await;
    if upgraded {
        let store = Store::open(&path).await.unwrap();
        store.close().await;
        drop(store);
    }
    let mut connection = connect(&path).await;
    match fault {
        IncompatibleLedger::Future => {
            sqlx::query("INSERT INTO _sqlx_migrations VALUES (9001,'future','2026-10-03 00:00:00',1,x'00',0)")
                .execute(&mut connection).await.unwrap();
        }
        IncompatibleLedger::Dirty => {
            sqlx::query("UPDATE _sqlx_migrations SET success=0 WHERE version=1")
                .execute(&mut connection)
                .await
                .unwrap();
        }
        IncompatibleLedger::ChangedChecksum => {
            sqlx::query("UPDATE _sqlx_migrations SET checksum=x'00' WHERE version=1")
                .execute(&mut connection)
                .await
                .unwrap();
        }
    }
    let before = snapshot(&mut connection).await;
    let before_ledger = ledger(&mut connection).await;
    let before_credentials = if upgraded {
        Some(credential_metadata(&mut connection).await)
    } else {
        None
    };
    connection.close().await.unwrap();
    let error = Store::open(&path)
        .await
        .err()
        .expect("Unsupported schema must be refused");
    assert_eq!(error.code, ErrorCode::Storage, "{fault:?}");
    assert!(error.message.contains("existing database was preserved"));
    // Check the OS lease itself: a generic Storage error alone could otherwise
    // mask a failed open that permanently retained the single-writer lock.
    let lease = OpenOptions::new()
        .read(true)
        .write(true)
        .open(temp.path().join("collaboration.db.lock"))
        .unwrap();
    lease
        .try_lock()
        .expect("A failed bootstrap must release its writer lease");
    drop(lease);
    let mut connection = connect(&path).await;
    assert_eq!(snapshot(&mut connection).await, before);
    assert_eq!(ledger(&mut connection).await, before_ledger);
    if let Some(before_credentials) = before_credentials {
        assert_eq!(
            credential_metadata(&mut connection).await,
            before_credentials
        );
    }
    assert_integrity(&mut connection).await;

    // Deliberate test-fixture repair is not a production recovery/reset path.
    // Production must retain the unsupported file until a compatible binary or
    // explicit user recovery is available.
    sqlx::query("DELETE FROM _sqlx_migrations WHERE version=9001")
        .execute(&mut connection)
        .await
        .unwrap();
    sqlx::query("UPDATE _sqlx_migrations SET success=1,checksum=? WHERE version=1")
        .bind(Sha384::digest(V1_SCHEMA).to_vec())
        .execute(&mut connection)
        .await
        .unwrap();
    connection.close().await.unwrap();
    let store = Store::open(&path).await.unwrap();
    assert_historical_reads(&store).await;
    store.close().await;
}

#[tokio::test]
async fn newer_schema_is_recoverable_without_silent_reset() {
    for upgraded in [false, true] {
        rejected_upgrade_preserves_data_and_releases_writer_lease(
            IncompatibleLedger::Future,
            upgraded,
        )
        .await;
    }
}

#[tokio::test]
async fn dirty_migration_is_recoverable_without_silent_reset() {
    for upgraded in [false, true] {
        rejected_upgrade_preserves_data_and_releases_writer_lease(
            IncompatibleLedger::Dirty,
            upgraded,
        )
        .await;
    }
}

#[tokio::test]
async fn changed_applied_migration_is_recoverable_without_silent_reset() {
    for upgraded in [false, true] {
        rejected_upgrade_preserves_data_and_releases_writer_lease(
            IncompatibleLedger::ChangedChecksum,
            upgraded,
        )
        .await;
    }
}

#[tokio::test]
async fn production_initial_migration_rolls_back_when_a_late_statement_fails() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("collaboration.db");
    let mut connection = connect(&path).await;
    // Deliberately unsupported storage with an existing authored-intent table
    // makes the production migration fail at CREATE TABLE drafts, after it has
    // already created accounts, FTS and sync tables in its transaction.
    sqlx::raw_sql("CREATE TABLE drafts(account_id TEXT,subject_id TEXT,body TEXT,generation INTEGER); INSERT INTO drafts VALUES ('a','private','Existing authored text',23);")
        .execute(&mut connection).await.unwrap();
    connection.close().await.unwrap();
    assert_eq!(
        Store::open(&path).await.err().unwrap().code,
        ErrorCode::Storage
    );
    let mut connection = connect(&path).await;
    let draft: (String, i64) = sqlx::query_as("SELECT body,generation FROM drafts")
        .fetch_one(&mut connection)
        .await
        .unwrap();
    assert_eq!(draft, ("Existing authored text".into(), 23));
    let tables: Vec<String> =
        sqlx::query_scalar("SELECT name FROM sqlite_schema WHERE type='table' ORDER BY name")
            .fetch_all(&mut connection)
            .await
            .unwrap();
    assert_eq!(
        tables,
        ["_sqlx_migrations", "drafts"],
        "Transactional DDL must roll back including FTS shadow tables"
    );
    assert!(ledger(&mut connection).await.is_empty());
    assert_integrity(&mut connection).await;
}

const PARTIAL_FAILURE: &str = "
    UPDATE drafts SET body='would erase intent' WHERE account_id='a';
    CREATE TABLE migration_backfill(id INTEGER PRIMARY KEY,payload BLOB NOT NULL);
    INSERT INTO migration_backfill VALUES (1,x'01');
    INSERT INTO nonexistent_migration_target VALUES (1);
";
const LARGE_BACKFILL: &str = "
    UPDATE drafts SET body='would erase intent' WHERE account_id='a';
    CREATE TABLE migration_backfill(id INTEGER PRIMARY KEY,payload BLOB NOT NULL);
    WITH RECURSIVE rows(id) AS (VALUES(1) UNION ALL SELECT id+1 FROM rows WHERE id<1000)
    INSERT INTO migration_backfill SELECT id,zeroblob(4096) FROM rows;
";
const RECOVERED_MIGRATION: &str = "
    CREATE TABLE migration_backfill(id INTEGER PRIMARY KEY,payload BLOB NOT NULL);
    INSERT INTO migration_backfill VALUES (1,x'01');
";

fn with_probe(sql: &'static str) -> Migrator {
    let mut migrations: Vec<_> = CURRENT.iter().cloned().collect();
    assert!(
        migrations
            .iter()
            .all(|migration| migration.version < PROBE_VERSION)
    );
    migrations.push(Migration::new(
        PROBE_VERSION,
        "synthetic recovery probe".into(),
        MigrationType::Simple,
        sql.into_sql_str(),
        false,
    ));
    Migrator::with_migrations(migrations)
}

fn sqlite_error_code(error: &MigrateError) -> Option<String> {
    match error {
        MigrateError::Execute(error) | MigrateError::ExecuteMigration(error, _) => error
            .as_database_error()
            .and_then(|error| error.code())
            .map(|code| code.into_owned()),
        _ => None,
    }
}

#[derive(Clone, Copy, Debug)]
enum Fault {
    InvalidStatement,
    InterruptedBackfill,
    FullDatabase,
}

async fn migration_fault_rolls_back_and_can_retry(fault: Fault) {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("collaboration.db");
    frozen_v1(&path).await;
    let store = Store::open(&path).await.unwrap();
    store.close().await;
    drop(store);
    let mut connection = connect(&path).await;
    let before = snapshot(&mut connection).await;
    let before_ledger = ledger(&mut connection).await;
    let before_credentials = credential_metadata(&mut connection).await;
    let inserted = Arc::new(AtomicBool::new(false));
    match fault {
        Fault::InvalidStatement => {}
        Fault::InterruptedBackfill => {
            let mut handle = connection.lock_handle().await.unwrap();
            let observed = inserted.clone();
            handle.set_update_hook(move |change| {
                if change.table == "migration_backfill" {
                    observed.store(true, Ordering::SeqCst);
                }
            });
            let observed = inserted.clone();
            let mut interrupted = false;
            handle.set_progress_handler(1, move || {
                if observed.load(Ordering::SeqCst) && !interrupted {
                    interrupted = true;
                    false
                } else {
                    true // Permit SQLite/SQLx to execute rollback after the fault.
                }
            });
        }
        Fault::FullDatabase => {
            let pages: i64 = sqlx::query_scalar("PRAGMA page_count")
                .fetch_one(&mut connection)
                .await
                .unwrap();
            // This is a real SQLite allocation limit, not a mocked I/O error.
            // Static numeric test data is the only dynamic SQL component.
            let maximum: i64 = sqlx::query_scalar(AssertSqlSafe(format!(
                "PRAGMA max_page_count={}",
                pages + 2
            )))
            .fetch_one(&mut connection)
            .await
            .unwrap();
            assert_eq!(maximum, pages + 2);
        }
    }
    let sql = if matches!(fault, Fault::InvalidStatement) {
        PARTIAL_FAILURE
    } else {
        LARGE_BACKFILL
    };
    let error = with_probe(sql)
        .run_direct(None, &mut connection, false)
        .await
        .unwrap_err();
    {
        let mut handle = connection.lock_handle().await.unwrap();
        handle.remove_progress_handler();
        handle.remove_update_hook();
    }
    match fault {
        Fault::InvalidStatement => assert_eq!(sqlite_error_code(&error).as_deref(), Some("1")),
        Fault::InterruptedBackfill => {
            assert!(
                inserted.load(Ordering::SeqCst),
                "Interrupt only after partial migration work"
            );
            assert_eq!(sqlite_error_code(&error).as_deref(), Some("9"));
        }
        Fault::FullDatabase => assert_eq!(sqlite_error_code(&error).as_deref(), Some("13")),
    }
    sqlx::query("PRAGMA max_page_count=1073741823")
        .execute(&mut connection)
        .await
        .unwrap();
    assert_eq!(snapshot(&mut connection).await, before, "{fault:?}");
    assert_eq!(ledger(&mut connection).await, before_ledger, "{fault:?}");
    assert_eq!(
        credential_metadata(&mut connection).await,
        before_credentials,
        "{fault:?}"
    );
    let table_count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM sqlite_schema WHERE name='migration_backfill'")
            .fetch_one(&mut connection)
            .await
            .unwrap();
    assert_eq!(table_count, 0, "Uncommitted schema and data must roll back");
    assert_integrity(&mut connection).await;
    connection.close().await.unwrap();
    let store = Store::open(&path).await.unwrap();
    assert_historical_reads(&store).await;
    store.close().await;
    drop(store);

    // Retry the unapplied version after correcting the fault fixture. The first
    // successful run commits once; reopening and re-running cannot duplicate it.
    for _ in 0..2 {
        let mut connection = connect(&path).await;
        with_probe(RECOVERED_MIGRATION)
            .run_direct(None, &mut connection, false)
            .await
            .unwrap();
        assert_eq!(snapshot(&mut connection).await, before);
        assert_eq!(
            credential_metadata(&mut connection).await,
            before_credentials
        );
        let rows: i64 = sqlx::query_scalar("SELECT count(*) FROM migration_backfill")
            .fetch_one(&mut connection)
            .await
            .unwrap();
        assert_eq!(rows, 1);
        let applied: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM _sqlx_migrations WHERE version=9001 AND success=1",
        )
        .fetch_one(&mut connection)
        .await
        .unwrap();
        assert_eq!(applied, 1);
        assert_integrity(&mut connection).await;
        connection.close().await.unwrap();
    }
}

#[tokio::test]
async fn failed_forward_migration_rolls_back_authored_intent_and_schema() {
    migration_fault_rolls_back_and_can_retry(Fault::InvalidStatement).await;
}

#[tokio::test]
async fn interrupted_forward_migration_rolls_back_authored_intent_and_schema() {
    migration_fault_rolls_back_and_can_retry(Fault::InterruptedBackfill).await;
}

#[tokio::test]
async fn sqlite_full_during_forward_migration_rolls_back_authored_intent_and_schema() {
    migration_fault_rolls_back_and_can_retry(Fault::FullDatabase).await;
}

// Always kill/reap the bounded child if an assertion or timeout fails, so a
// failing recovery test cannot leave a blocked writer behind on the host.
struct CrashWorker(Child);

impl Drop for CrashWorker {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

#[tokio::test]
async fn process_crash_during_migration_releases_lease_and_preserves_intent() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("collaboration.db");
    frozen_v1(&path).await;
    let store = Store::open(&path).await.unwrap();
    store.close().await;
    drop(store);
    let mut connection = connect(&path).await;
    let before = snapshot(&mut connection).await;
    let before_ledger = ledger(&mut connection).await;
    let before_credentials = credential_metadata(&mut connection).await;
    connection.close().await.unwrap();

    let mut worker = CrashWorker(
        Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "migration_crash_worker",
                "--ignored",
                "--nocapture",
            ])
            .env("GITRU_MIGRATION_CRASH_DB", &path)
            .stdout(Stdio::null())
            .stderr(Stdio::inherit())
            .spawn()
            .unwrap(),
    );
    let marker = path.with_extension("migration-started");
    let deadline = Instant::now() + Duration::from_secs(10);
    while !marker.exists() {
        assert!(
            worker.0.try_wait().unwrap().is_none(),
            "Migration worker exited before reaching the uncommitted backfill"
        );
        assert!(
            Instant::now() < deadline,
            "Migration worker did not reach its checkpoint"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(
        Store::open(&path).await.err().unwrap().code,
        ErrorCode::Busy,
        "The child owns the real Store writer lease before the crash"
    );
    worker.0.kill().unwrap();
    assert!(!worker.0.wait().unwrap().success());

    // Hard termination runs no Rust destructors. SQLite must roll back the
    // unfinished migration and the OS must release the native Store lease.
    let store = Store::open(&path).await.unwrap();
    assert_historical_reads(&store).await;
    store.close().await;
    drop(store);
    let mut connection = connect(&path).await;
    assert_eq!(snapshot(&mut connection).await, before);
    assert_eq!(ledger(&mut connection).await, before_ledger);
    assert_eq!(
        credential_metadata(&mut connection).await,
        before_credentials
    );
    let table_count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM sqlite_schema WHERE name='migration_backfill'")
            .fetch_one(&mut connection)
            .await
            .unwrap();
    assert_eq!(table_count, 0);
    assert_integrity(&mut connection).await;
}

#[test]
#[ignore = "Subprocess helper; invoked only by process_crash_during_migration_releases_lease_and_preserves_intent"]
fn migration_crash_worker() {
    let path = std::env::var_os("GITRU_MIGRATION_CRASH_DB")
        .expect("The parent recovery test must provide an isolated database");
    let path = std::path::PathBuf::from(path);
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            let _store = Store::open(&path).await.unwrap();
            let mut connection = connect(&path).await;
            let marker = path.with_extension("migration-started");
            {
                let mut handle = connection.lock_handle().await.unwrap();
                handle.set_update_hook(move |change| {
                    if change.table == "migration_backfill" {
                        // Signal only after an actual backfill insert, then
                        // hold the transaction open until the parent kills us.
                        std::fs::write(&marker, "uncommitted backfill").unwrap();
                        loop {
                            std::thread::park();
                        }
                    }
                });
            }
            with_probe(LARGE_BACKFILL)
                .run_direct(None, &mut connection, false)
                .await
                .expect("The child must be killed before this migration commits");
        });
}
