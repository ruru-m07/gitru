//! Recovery policy accepts only frozen recognized schemas; historical bytes are
//! never edited or migrated in place. Every fixture database is synthetic.
use collaboration::recovery::{RecoverySession, RestoreChoice};
use collaboration::{AccountState, ProviderKind, RemoteAccount, Store};
use sha2::{Digest, Sha384};
use sqlx::{
    Connection, SqlSafeStr, SqliteConnection,
    migrate::{Migration, MigrationType, Migrator},
    sqlite::SqliteConnectOptions,
};
static CURRENT: Migrator = sqlx::migrate!("./migrations");
const OLD_SQL: [&str; 17] = [
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
    include_str!("fixtures/migrations/v13/0013_command_admission.sql"),
    include_str!("fixtures/migrations/v14/0014_pull_file_generations.sql"),
    include_str!("fixtures/migrations/v15/0015_recovery_quarantine.sql"),
    include_str!("fixtures/migrations/v16/0016_command_delivery.sql"),
    include_str!("fixtures/migrations/v17/0017_effective_intent.sql"),
];
const CHECKSUMS: [&str; 17] = [
    "a0b4863d56b1620dae93b13df7ef2b38074c3ac5a5d5bf639b01899204cb61f6796ba9fb37bfd3b085f79e928e475e3d",
    "2fe47653ace5f705b32a819739da13bd9faf40a56a268da416a9f9d39c770ec74a42377268670c40a5478c898137929b",
    "6f5925a0690563071eeaeeb43bc3eec634c280582b9971e94effe266eedb804fb7773b85a5a4d9ad2492439c575e67e9",
    "36b4ccaf02a8a00eee143d81bd9b8505c5831bbfb6ee4ee02412c3232aef92a8e5b08fdadbdb097a5e0bff3b42f281da",
    "7ad6d3309e04d14db8f65a11954f5310a41cfd960493d69d489abebb9c454463e1e19af5f4200348fd3f7ac700d28435",
    "dcdf2e80cd9c78d32ca1f5c5284635b55ac4e5ffd1f728baf171f964e7e84024702c506a335f5b55775b2261bc337c34",
    "ba94402e7b3671eb73aab3da890a484447b5b4f0d622afccbd2b474fa90f925fe867cb34b6bf379c1964a7e5917b1e8c",
    "fb8de37dba851bbe428aafc8685e6cf572294de30a00f8d52138e6c0acd06a26911d31c19f54c80b12d74bc857290073",
    "d0d0ac8e029c92dbc3ad804518219c5849edb4c8dd2818578202dee4596dcf56d173aef92e3d0c49d5e717685d606f8a",
    "441544b7f7125838a5532096dd40487323537ea7e4f9917c1174498a0b24c5695d07783a2ca7a149500cce9e00a80621",
    "28c0e19477d1d940af1508ace6bf353529f0393bc8d0c27e8529e8fde3764ae9bcafc932e93bd466c04e340095c089c4",
    "65b06409d491935ea19d209a0317f96894e6c3e2cbb69ee03e130bb983ddc28e7aa720521f9963b5957db1d97fe5dd1b",
    "d29215527787e5b66230af7d8f1f1a915c969a52f54b49bace77247936b2138a361b82f3b167142341d914562d3b1109",
    "0e8e926a667a1e02a62edbcdf2886dc25e65a66c02c079ededd5a1db82f263673f4ffcdaab1b81dc5aada67c86281c15",
    "75cedf38449a30de9ec6ea7dae41582e09d34d746909ce6af16ee52c615c6a70f94b0f882c677b65ec40284b4fd6d88f",
    "f29b1e22717899ef34a8159790fde4388d3573ed67a8051d970dad690191ac6daf28c9a0bf8905e0e78bb848dc93c3db",
    "8630ad7104e36c9db4006ceef2516fcea090d99259ffe44efac97fa21df3723fe2e5a75bb23b121fc221df8702f0efd6",
];
fn historical(version: usize) -> Migrator {
    Migrator::with_migrations(
        OLD_SQL
            .iter()
            .take(version)
            .enumerate()
            .map(|(i, sql)| {
                Migration::new(
                    i as i64 + 1,
                    "frozen recovery schema".into(),
                    MigrationType::Simple,
                    (*sql).into_sql_str(),
                    false,
                )
            })
            .collect(),
    )
}
async fn database(path: &std::path::Path) -> SqliteConnection {
    SqliteConnection::connect_with(
        &SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(true)
            .foreign_keys(true),
    )
    .await
    .unwrap()
}
#[test]
fn accepted_historical_sql_and_checksums_are_frozen() {
    for (index, sql) in OLD_SQL.iter().enumerate() {
        assert_eq!(
            format!("{:x}", Sha384::digest(sql.as_bytes())),
            CHECKSUMS[index]
        );
        assert_eq!(CURRENT.iter().nth(index).unwrap().sql.as_ref(), *sql);
        assert_eq!(
            CURRENT.iter().nth(index).unwrap().checksum.as_ref(),
            Sha384::digest(sql.as_bytes()).as_slice()
        );
    }
}
#[tokio::test]
async fn every_recognized_historical_schema_restores_without_modifying_the_selected_file() {
    let dir = tempfile::tempdir().unwrap();
    for version in 1..=17 {
        let target = dir.path().join(format!("target-{version}.db"));
        let source = dir.path().join(format!("v{version}.db"));
        let store = Store::open(&target).await.unwrap();
        store.close().await.unwrap();
        drop(store);
        let mut db = database(&source).await;
        historical(version).run(&mut db).await.unwrap();
        let account = RemoteAccount {
            id: "a".into(),
            provider: ProviderKind::Github,
            host: "github.com".into(),
            actor_id: "actor-a".into(),
            login: "a".into(),
            display_name: None,
            authorization_epoch: "19".into(),
            state: AccountState::Active,
            notifications_supported: true,
        };
        sqlx::query(
            "INSERT INTO accounts VALUES('a','github','github.com','actor-a',19,'active',?)",
        )
        .bind(serde_json::to_string(&account).unwrap())
        .execute(&mut db)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO drafts VALUES('a','missing-subject','Retain authored 雪 text',37)",
        )
        .execute(&mut db)
        .await
        .unwrap();
        if version >= 3 {
            sqlx::raw_sql("INSERT INTO provider_instances VALUES('github:https://github.com/','github','https://github.com/'); INSERT INTO account_instances VALUES('a','github:https://github.com/');").execute(&mut db).await.unwrap();
        }
        db.close().await.unwrap();
        let before = std::fs::read(&source).unwrap();
        let session = RecoverySession::prepare(&target, &source).await.unwrap();
        assert_eq!(session.preview().incoming.schema_version, version as i64);
        assert_eq!(session.preview().quarantined_commands, 0);
        let id = session.preview().confirmation_id.clone();
        session
            .confirm(&id, RestoreChoice::ReplaceCurrentData)
            .unwrap();
        assert_eq!(
            std::fs::read(&source).unwrap(),
            before,
            "source version {version}"
        );
        let store = Store::open(&target).await.unwrap();
        let draft = store.draft("a", "missing-subject").await.unwrap().unwrap();
        assert_eq!(draft.body, "Retain authored 雪 text");
        assert_eq!(draft.generation, "37");
        assert_eq!(store.account("a").await.unwrap().authorization_epoch, "20");
        store.close().await.unwrap();
    }
}
#[tokio::test]
async fn failed_quarantine_migration_rolls_back_existing_schema_and_intent() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("history.db");
    let mut db = database(&path).await;
    historical(14).run(&mut db).await.unwrap();
    let before: Vec<String> =
        sqlx::query_scalar("SELECT sql FROM sqlite_schema WHERE sql IS NOT NULL ORDER BY name")
            .fetch_all(&mut db)
            .await
            .unwrap();
    let mut migrations = historical(14).iter().cloned().collect::<Vec<_>>();
    migrations.push(Migration::new(
        15,
        "failed recovery".into(),
        MigrationType::Simple,
        sqlx::AssertSqlSafe(format!(
            "{}\nSELECT * FROM missing_recovery_table;",
            include_str!("../migrations/0015_recovery_quarantine.sql")
        ))
        .into_sql_str(),
        false,
    ));
    assert!(
        Migrator::with_migrations(migrations)
            .run(&mut db)
            .await
            .is_err()
    );
    let after: Vec<String> =
        sqlx::query_scalar("SELECT sql FROM sqlite_schema WHERE sql IS NOT NULL ORDER BY name")
            .fetch_all(&mut db)
            .await
            .unwrap();
    assert_eq!(before, after);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT max(version) FROM _sqlx_migrations")
            .fetch_one(&mut db)
            .await
            .unwrap(),
        14
    );
    CURRENT.run(&mut db).await.unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT generation FROM recovery_meta")
            .fetch_one(&mut db)
            .await
            .unwrap(),
        0
    );
    db.close().await.unwrap();
}

#[tokio::test]
async fn actual_interrupt_and_full_disk_leave_v14_retryable() {
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };
    for interrupt in [true, false] {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("fault.db");
        let mut db = database(&path).await;
        historical(14).run(&mut db).await.unwrap();
        let observed = Arc::new(AtomicBool::new(false));
        if interrupt {
            let mut handle = db.lock_handle().await.unwrap();
            let signal = observed.clone();
            handle.set_update_hook(move |update| {
                if update.table == "recovery_meta" {
                    signal.store(true, Ordering::SeqCst);
                }
            });
            let signal = observed.clone();
            let mut fired = false;
            handle.set_progress_handler(1, move || {
                if signal.load(Ordering::SeqCst) && !fired {
                    fired = true;
                    false
                } else {
                    true
                }
            });
        } else {
            let pages: i64 = sqlx::query_scalar("PRAGMA page_count")
                .fetch_one(&mut db)
                .await
                .unwrap();
            sqlx::query(sqlx::AssertSqlSafe(format!(
                "PRAGMA max_page_count={pages}"
            )))
            .execute(&mut db)
            .await
            .unwrap();
        }
        let error = CURRENT.run_direct(None, &mut db, false).await.unwrap_err();
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
            Some(if interrupt { "9" } else { "13" }),
            "{error:?}"
        );
        if interrupt {
            assert!(observed.load(Ordering::SeqCst));
            let mut handle = db.lock_handle().await.unwrap();
            handle.remove_progress_handler();
            handle.remove_update_hook();
        }
        sqlx::query("PRAGMA max_page_count=1073741823")
            .execute(&mut db)
            .await
            .unwrap();
        assert_eq!(
            sqlx::query_scalar::<_, i64>(
                "SELECT count(*) FROM sqlite_schema WHERE name='recovery_meta'"
            )
            .fetch_one(&mut db)
            .await
            .unwrap(),
            0
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT max(version) FROM _sqlx_migrations")
                .fetch_one(&mut db)
                .await
                .unwrap(),
            14
        );
        db.close().await.unwrap();
        let store = Store::open(&path).await.unwrap();
        store.close().await.unwrap();
    }
}

#[tokio::test]
async fn actual_interrupt_and_full_disk_leave_v15_retryable() {
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };
    for interrupt in [true, false] {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("fault.db");
        let mut db = database(&path).await;
        historical(15).run(&mut db).await.unwrap();
        let observed = Arc::new(AtomicBool::new(false));
        if interrupt {
            let mut handle = db.lock_handle().await.unwrap();
            let signal = observed.clone();
            handle.set_update_hook(move |update| {
                if update.table == "_sqlx_migrations" {
                    signal.store(true, Ordering::SeqCst);
                }
            });
            let signal = observed.clone();
            let mut fired = false;
            handle.set_progress_handler(1, move || {
                if signal.load(Ordering::SeqCst) && !fired {
                    fired = true;
                    false
                } else {
                    true
                }
            });
        } else {
            let pages: i64 = sqlx::query_scalar("PRAGMA page_count")
                .fetch_one(&mut db)
                .await
                .unwrap();
            sqlx::query(sqlx::AssertSqlSafe(format!(
                "PRAGMA max_page_count={pages}"
            )))
            .execute(&mut db)
            .await
            .unwrap();
        }
        let error = CURRENT.run_direct(None, &mut db, false).await.unwrap_err();
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
            Some(if interrupt { "9" } else { "13" }),
            "{error:?}"
        );
        if interrupt {
            assert!(observed.load(Ordering::SeqCst));
            let mut handle = db.lock_handle().await.unwrap();
            handle.remove_progress_handler();
            handle.remove_update_hook();
        }
        sqlx::query("PRAGMA max_page_count=1073741823")
            .execute(&mut db)
            .await
            .unwrap();
        assert_eq!(
            sqlx::query_scalar::<_, i64>(
                "SELECT count(*) FROM sqlite_schema WHERE name='command_delivery'"
            )
            .fetch_one(&mut db)
            .await
            .unwrap(),
            0
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT max(version) FROM _sqlx_migrations")
                .fetch_one(&mut db)
                .await
                .unwrap(),
            15
        );
        db.close().await.unwrap();
        let store = Store::open(&path).await.unwrap();
        store.close().await.unwrap();
    }
}

#[tokio::test]
async fn delivery_keysets_seek_partial_indexes_without_terminal_history_or_temp_sort() {
    let dir = tempfile::tempdir().unwrap();
    let mut db = database(&dir.path().join("plans.db")).await;
    CURRENT.run(&mut db).await.unwrap();
    for (sql, index) in [
        (
            "EXPLAIN QUERY PLAN SELECT id FROM accounts INDEXED BY command_delivery_active_accounts WHERE state='active' AND id>'a' ORDER BY id LIMIT 32",
            "command_delivery_active_accounts",
        ),
        (
            "EXPLAIN QUERY PLAN SELECT account_id,command_id FROM commands INDEXED BY command_delivery_pending WHERE account_id='a' AND command_id>'123e4567-e89b-12d3-a456-426614174000' AND state IN ('queued','sending','retry_wait','accepted','outcome_unknown') ORDER BY command_id LIMIT 32",
            "command_delivery_pending",
        ),
        (
            "EXPLAIN QUERY PLAN SELECT 1 FROM commands WHERE account_id='a' AND target_kind='issue' AND target_id='issue' AND enqueue_order<90 AND state IN ('queued','sending','retry_wait','accepted','outcome_unknown','conflict') LIMIT 1",
            "command_delivery_target_order",
        ),
    ] {
        use sqlx::Row;
        let rows = sqlx::query(sql).fetch_all(&mut db).await.unwrap();
        let plan = rows
            .iter()
            .map(|r| r.get::<String, _>("detail"))
            .collect::<Vec<_>>()
            .join("\n");
        assert!(plan.contains(index), "{plan}");
        assert!(plan.contains("SEARCH"), "{plan}");
        assert!(!plan.contains("TEMP B-TREE"), "{plan}");
    }
    db.close().await.unwrap();
}

#[tokio::test]
async fn failed_effect_migration_preserves_v16_authored_rows_and_retries_cleanly() {
    let dir = tempfile::tempdir().unwrap();
    let mut db = database(&dir.path().join("v16.db")).await;
    historical(16).run(&mut db).await.unwrap();
    let account = RemoteAccount {
        id: "a".into(),
        provider: ProviderKind::Github,
        host: "github.com".into(),
        actor_id: "actor-a".into(),
        login: "a".into(),
        display_name: None,
        authorization_epoch: "17".into(),
        state: AccountState::Active,
        notifications_supported: true,
    };
    sqlx::query("INSERT INTO accounts VALUES('a','github','github.com','actor-a',17,'active',?)")
        .bind(serde_json::to_string(&account).unwrap())
        .execute(&mut db)
        .await
        .unwrap();
    sqlx::raw_sql("INSERT INTO commands VALUES ('a','11111111-1111-4111-8111-111111111111',17,1,'fixture.comment',1,'pull_request','pull',NULL,x'010203',x'0405',x'0607',zeroblob(32),1,9004,'2026-10-08T00:00:00Z','queued');
INSERT INTO command_target_protections(account_id,command_id,reference_kind,reference_id,facet) VALUES ('a','11111111-1111-4111-8111-111111111111','facet','pull','files');
INSERT INTO command_evidence(account_id,command_id,ordinal,kind,version,payload,recorded_at) VALUES ('a','11111111-1111-4111-8111-111111111111',0,'validation',1,x'0102','2026-10-08T00:00:00Z');")
        .execute(&mut db)
        .await
        .unwrap();
    let before:Vec<String>=sqlx::query_scalar("SELECT json_array(account_id,command_id,hex(canonical_envelope),hex(submission_hash),state) FROM commands ORDER BY account_id,command_id").fetch_all(&mut db).await.unwrap();
    let mut migrations = historical(16).iter().cloned().collect::<Vec<_>>();
    migrations.push(Migration::new(
        17,
        "failed effects".into(),
        MigrationType::Simple,
        sqlx::AssertSqlSafe(format!(
            "{}\nSELECT * FROM missing_effect_migration;",
            include_str!("../migrations/0017_effective_intent.sql")
        ))
        .into_sql_str(),
        false,
    ));
    assert!(
        Migrator::with_migrations(migrations)
            .run(&mut db)
            .await
            .is_err()
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT max(version) FROM _sqlx_migrations")
            .fetch_one(&mut db)
            .await
            .unwrap(),
        16
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM sqlite_schema WHERE name='command_effects'"
        )
        .fetch_one(&mut db)
        .await
        .unwrap(),
        0
    );
    let after:Vec<String>=sqlx::query_scalar("SELECT json_array(account_id,command_id,hex(canonical_envelope),hex(submission_hash),state) FROM commands ORDER BY account_id,command_id").fetch_all(&mut db).await.unwrap();
    assert_eq!(before, after);
    CURRENT.run(&mut db).await.unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM command_effects")
            .fetch_one(&mut db)
            .await
            .unwrap(),
        0
    );
    db.close().await.unwrap();
}
