//! Frozen pre-command bytes and transactional migration faults. Credentials are
//! synthetic metadata from historical fixtures; no provider/vault is involved.
use collaboration::Store;
use sqlx::{
    AssertSqlSafe, Connection, Row, SqlSafeStr, SqliteConnection,
    migrate::{Migration, MigrationType, Migrator},
    sqlite::{SqliteConnectOptions, SqliteJournalMode, SqliteSynchronous},
};
use std::{
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

static CURRENT: Migrator = sqlx::migrate!("./migrations");
const OLD_SQL: [&str; 12] = [
    include_str!("fixtures/migrations/v8/0001_local_collaboration.sql"),
    include_str!("fixtures/migrations/v8/0002_credential_cutover.sql"),
    include_str!("fixtures/migrations/v8/0003_provider_identities.sql"),
    include_str!("fixtures/migrations/v8/0004_detail_scopes.sql"),
    include_str!("fixtures/migrations/v8/0005_resource_metadata.sql"),
    include_str!("fixtures/migrations/v8/0006_local_repository_links.sql"),
    include_str!("fixtures/migrations/v8/0007_notification_subjects.sql"),
    include_str!("fixtures/migrations/v8/0008_participant_facets.sql"),
    include_str!("fixtures/migrations/v12/0009_task_facets.sql"),
    include_str!("fixtures/migrations/v12/0010_cache_retention.sql"),
    include_str!("fixtures/migrations/v12/0011_pull_commit_generations.sql"),
    include_str!("fixtures/migrations/v12/0012_local_inbox_state.sql"),
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
async fn frozen(path: &Path) -> SqliteConnection {
    let mut connection = connect(path).await;
    let mut tx = connection.begin().await.unwrap();
    for sql in OLD_SQL {
        sqlx::raw_sql(sql).execute(&mut *tx).await.unwrap();
    }
    sqlx::raw_sql(include_str!("fixtures/migrations/v8/seed.sql"))
        .execute(&mut *tx)
        .await
        .unwrap();
    sqlx::raw_sql(include_str!("fixtures/migrations/v12/seed.sql"))
        .execute(&mut *tx)
        .await
        .unwrap();
    tx.commit().await.unwrap();
    connection
}
async fn old_tables(connection: &mut SqliteConnection) -> Vec<String> {
    sqlx::query_scalar("SELECT name FROM sqlite_schema WHERE type='table' AND name NOT LIKE 'sqlite_%' AND name NOT LIKE 'items_fts_%' ORDER BY name").fetch_all(connection).await.unwrap()
}
async fn snapshot(connection: &mut SqliteConnection, tables: &[String]) -> Vec<Vec<String>> {
    let mut output = vec![];
    for table in tables {
        let columns = sqlx::query(AssertSqlSafe(format!("PRAGMA table_info(\"{table}\")")))
            .fetch_all(&mut *connection)
            .await
            .unwrap();
        let columns = columns
            .into_iter()
            .map(|row| {
                let name: String = row.get("name");
                format!("quote(\"{name}\")")
            })
            .collect::<Vec<_>>()
            .join(",");
        output.push(
            sqlx::query_scalar(AssertSqlSafe(format!(
                "SELECT json_array({columns}) FROM \"{table}\" ORDER BY 1"
            )))
            .fetch_all(&mut *connection)
            .await
            .unwrap(),
        );
    }
    output
}
async fn integrity(connection: &mut SqliteConnection) {
    assert_eq!(
        sqlx::query_scalar::<_, String>("PRAGMA integrity_check")
            .fetch_one(&mut *connection)
            .await
            .unwrap(),
        "ok"
    );
    assert!(
        sqlx::query("PRAGMA foreign_key_check")
            .fetch_all(connection)
            .await
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn frozen_v12_upgrade_preserves_all_old_rows_and_original_ledger_bytes() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("migration.db");
    let mut connection = frozen(&path).await;
    let mut tables = old_tables(&mut connection).await;
    tables.retain(|table| table != "_sqlx_migrations");
    let before = snapshot(&mut connection, &tables).await;
    let ledger:Vec<String>=sqlx::query_scalar("SELECT json_array(version,description,installed_on,success,hex(checksum),execution_time) FROM _sqlx_migrations ORDER BY version").fetch_all(&mut connection).await.unwrap();
    connection.close().await.unwrap();
    let store = Store::open(&path).await.unwrap();
    store.close().await.unwrap();
    drop(store);
    let mut connection = connect(&path).await;
    assert_eq!(snapshot(&mut connection, &tables).await, before);
    let after:Vec<String>=sqlx::query_scalar("SELECT json_array(version,description,installed_on,success,hex(checksum),execution_time) FROM _sqlx_migrations WHERE version<=12 ORDER BY version").fetch_all(&mut connection).await.unwrap();
    assert_eq!(after, ledger);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM commands")
            .fetch_one(&mut connection)
            .await
            .unwrap(),
        0
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM _sqlx_migrations WHERE version=13 AND success=1"
        )
        .fetch_one(&mut connection)
        .await
        .unwrap(),
        1
    );
    integrity(&mut connection).await;
}

#[derive(Debug, Clone, Copy)]
enum Fault {
    Statement,
    Interrupt,
    Full,
}
async fn fault_case(fault: Fault) {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("migration.db");
    let mut connection = frozen(&path).await;
    let tables = old_tables(&mut connection).await;
    let before = snapshot(&mut connection, &tables).await;
    let changed = Arc::new(AtomicBool::new(false));
    let mut migrations = CURRENT
        .iter()
        .filter(|migration| migration.version <= 12)
        .cloned()
        .collect::<Vec<_>>();
    let sql = match fault {
        Fault::Statement => format!(
            "{}\nUPDATE drafts SET body='uncommitted'; SELECT * FROM missing_migration_table;",
            include_str!("../migrations/0013_command_admission.sql")
        ),
        Fault::Interrupt => format!(
            "{}\nUPDATE drafts SET body='uncommitted'; WITH RECURSIVE numbers(n) AS (VALUES(1) UNION ALL SELECT n+1 FROM numbers WHERE n<100000) SELECT sum(n) FROM numbers;",
            include_str!("../migrations/0013_command_admission.sql")
        ),
        Fault::Full => include_str!("../migrations/0013_command_admission.sql").to_owned(),
    };
    match fault {
        Fault::Interrupt => {
            let mut handle = connection.lock_handle().await.unwrap();
            let observed = changed.clone();
            handle.set_update_hook(move |update| {
                if update.table == "drafts" {
                    observed.store(true, Ordering::SeqCst);
                }
            });
            let observed = changed.clone();
            let mut fired = false;
            handle.set_progress_handler(1, move || {
                if observed.load(Ordering::SeqCst) && !fired {
                    fired = true;
                    false
                } else {
                    true
                }
            });
        }
        Fault::Full => {
            let pages: i64 = sqlx::query_scalar("PRAGMA page_count")
                .fetch_one(&mut connection)
                .await
                .unwrap();
            sqlx::query(AssertSqlSafe(format!("PRAGMA max_page_count={pages}")))
                .execute(&mut connection)
                .await
                .unwrap();
        }
        Fault::Statement => {}
    }
    migrations.push(Migration::new(
        13,
        "command admission fault probe".into(),
        MigrationType::Simple,
        AssertSqlSafe(sql).into_sql_str(),
        false,
    ));
    let error = Migrator::with_migrations(migrations)
        .run_direct(None, &mut connection, false)
        .await
        .unwrap_err();
    {
        let mut handle = connection.lock_handle().await.unwrap();
        handle.remove_update_hook();
        handle.remove_progress_handler();
    }
    let code = match &error {
        sqlx::migrate::MigrateError::Execute(error)
        | sqlx::migrate::MigrateError::ExecuteMigration(error, _) => error
            .as_database_error()
            .and_then(|error| error.code())
            .map(|code| code.into_owned()),
        _ => None,
    };
    assert_eq!(
        code.as_deref(),
        Some(match fault {
            Fault::Statement => "1",
            Fault::Interrupt => "9",
            Fault::Full => "13",
        }),
        "{fault:?}: {error}"
    );
    if matches!(fault, Fault::Interrupt) {
        assert!(changed.load(Ordering::SeqCst));
    }
    sqlx::query("PRAGMA max_page_count=1073741823")
        .execute(&mut connection)
        .await
        .unwrap();
    assert_eq!(
        snapshot(&mut connection, &tables).await,
        before,
        "{fault:?}"
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM sqlite_schema WHERE name='commands'")
            .fetch_one(&mut connection)
            .await
            .unwrap(),
        0
    );
    integrity(&mut connection).await;
    connection.close().await.unwrap();
    let store = Store::open(&path).await.unwrap();
    store.close().await.unwrap();
    drop(store);
}
#[tokio::test]
async fn partial_schema_failure_rolls_back_and_retries() {
    fault_case(Fault::Statement).await;
}
#[tokio::test]
async fn interrupted_command_migration_rolls_back_and_retries() {
    fault_case(Fault::Interrupt).await;
}
#[tokio::test]
async fn full_disk_command_migration_rolls_back_and_retries() {
    fault_case(Fault::Full).await;
}

#[tokio::test]
async fn newer_schema_and_checksum_mismatch_refuse_to_open_without_mutating_intent() {
    for mutation in [
        "INSERT INTO _sqlx_migrations VALUES(9001,'future','2026-10-07',1,x'00',1)",
        "UPDATE _sqlx_migrations SET checksum=x'00' WHERE version=12",
    ] {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("migration.db");
        let mut connection = frozen(&path).await;
        sqlx::query(mutation)
            .execute(&mut connection)
            .await
            .unwrap();
        let tables = old_tables(&mut connection).await;
        let before = snapshot(&mut connection, &tables).await;
        connection.close().await.unwrap();
        assert!(Store::open(&path).await.is_err());
        let mut connection = connect(&path).await;
        assert_eq!(snapshot(&mut connection, &tables).await, before);
        integrity(&mut connection).await;
    }
}
