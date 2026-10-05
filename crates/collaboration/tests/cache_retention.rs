//! Real SQLite qualification for bounded, rebuildable detail retention.
//! Public Store operations create current observations; literal historical SQL
//! and aborting triggers exercise migration, interrupted indexing and rollback.
use std::{path::Path, time::Duration};

use collaboration::{storage::retention::CacheRetentionPolicy, *};
use sqlx::{
    AssertSqlSafe, Connection, Row, SqliteConnection,
    sqlite::{SqliteConnectOptions, SqliteJournalMode, SqliteSynchronous},
};

mod detail_support;
use detail_support::*;

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
const ORIGINAL_TABLES: &[&str] = &[
    "runtime_meta",
    "accounts",
    "repositories",
    "items",
    "items_fts",
    "sync_scopes",
    "scope_membership",
    "drafts",
    "change_log",
    "account_credentials",
    "credential_cleanup",
    "provider_instances",
    "account_instances",
    "resource_identities",
    "resource_aliases",
    "pending_endpoint_aliases",
    "detail_observations",
    "detail_entries",
    "detail_demand",
    "detail_resource_metadata",
    "local_link_meta",
    "local_transport_bindings",
    "local_repository_links",
    "notification_subject_selectors",
    "notification_subject_discovery",
];
const AUTHORED_AND_IDENTITY_TABLES: &[&str] = &[
    "accounts",
    "repositories",
    "items",
    "items_fts",
    "scope_membership",
    "drafts",
    "account_credentials",
    "credential_cleanup",
    "provider_instances",
    "account_instances",
    "resource_identities",
    "resource_aliases",
    "pending_endpoint_aliases",
    "detail_demand",
    "local_link_meta",
    "local_transport_bindings",
    "local_repository_links",
    "notification_subject_selectors",
    "notification_subject_discovery",
    "cache_pins",
];

fn policy() -> CacheRetentionPolicy {
    CacheRetentionPolicy {
        target_logical_bytes: 0,
        max_index_facets: u32::MAX,
        max_scan_facets: u32::MAX,
        max_evict_facets: u32::MAX,
        max_entry_rows: u32::MAX,
        checkpoint_wal: false,
    }
}

async fn connect(path: &Path) -> SqliteConnection {
    SqliteConnection::connect_with(
        &SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(true)
            .foreign_keys(true)
            .journal_mode(SqliteJournalMode::Wal)
            .synchronous(SqliteSynchronous::Full)
            .busy_timeout(Duration::from_secs(2)),
    )
    .await
    .unwrap()
}

async fn count(connection: &mut SqliteConnection, table: &str) -> i64 {
    // Table names below are fixed test constants, never provider or user input.
    sqlx::query_scalar(AssertSqlSafe(format!("SELECT count(*) FROM {table}")))
        .fetch_one(connection)
        .await
        .unwrap()
}

async fn snapshot(connection: &mut SqliteConnection, tables: &[&str]) -> Vec<Vec<String>> {
    let mut result = Vec::new();
    for table in tables {
        let columns = sqlx::query(AssertSqlSafe(format!("PRAGMA table_info({table})")))
            .fetch_all(&mut *connection)
            .await
            .unwrap()
            .into_iter()
            .map(|row| format!("\"{}\"", row.get::<String, _>("name")))
            .collect::<Vec<_>>()
            .join(",");
        assert!(!columns.is_empty(), "The fixture table {table} exists");
        result.push(
            sqlx::query_scalar(AssertSqlSafe(format!(
                "SELECT json_array({columns}) FROM {table} ORDER BY 1"
            )))
            .fetch_all(&mut *connection)
            .await
            .unwrap(),
        );
    }
    result
}

async fn assert_integrity(connection: &mut SqliteConnection) {
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

async fn frozen_v8(path: &Path) {
    let mut connection = connect(path).await;
    let mut tx = connection.begin().await.unwrap();
    for sql in V8_SQL {
        sqlx::raw_sql(sql).execute(&mut *tx).await.unwrap();
    }
    sqlx::raw_sql(include_str!("fixtures/migrations/v8/seed.sql"))
        .execute(&mut *tx)
        .await
        .unwrap();
    tx.commit().await.unwrap();
    connection.close().await.unwrap();
}

async fn logical_bytes(connection: &mut SqliteConnection) -> u64 {
    // Independent persisted-byte calculation, including UTF-8 rather than chars.
    let bytes: i64 = sqlx::query_scalar(
        "SELECT coalesce((SELECT sum(octet_length(body_json)+octet_length(source_json)+coalesce(octet_length(value_source_json),0)) FROM detail_observations),0)+coalesce((SELECT sum(octet_length(json)) FROM detail_entries),0)+coalesce((SELECT sum(octet_length(metadata_json)+octet_length(source_json)) FROM detail_resource_metadata),0)",
    )
    .fetch_one(connection)
    .await
    .unwrap();
    u64::try_from(bytes).unwrap()
}

async fn assert_accounted(store: &Store, connection: &mut SqliteConnection) {
    let usage = store.cache_usage().await.unwrap();
    assert!(usage.index_complete);
    assert_eq!(usage.logical_bytes, Some(logical_bytes(connection).await));
    assert_eq!(usage.indexed_logical_bytes, usage.logical_bytes.unwrap());
    assert_eq!(
        usage.indexed_facets,
        count(connection, "detail_observations").await as u64
    );
    assert_eq!(
        count(connection, "cache_retention_entries").await as u64,
        usage.indexed_facets
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM cache_retention_entries r JOIN detail_observations o USING(account_id,subject_id,facet) WHERE r.last_observed_revision<>CAST(o.facet_revision AS INTEGER)")
            .fetch_one(connection).await.unwrap(),
        0,
        "Accounting retains the accepted observation revision rather than a string sort key"
    );
}

async fn complete_index(store: &Store) {
    for _ in 0..8 {
        let report = store
            .run_cache_maintenance(CacheRetentionPolicy {
                target_logical_bytes: u64::MAX,
                ..policy()
            })
            .await
            .unwrap();
        assert!(report.indexed_facets <= 128);
        assert_eq!(report.evicted_facets, 0);
        if report.usage_after.index_complete {
            return;
        }
    }
    panic!("The bounded fixture completes historical accounting");
}

async fn add_subjects(store: &Store, actor: &RemoteAccount, ids: &[String]) {
    let template = store.item(&actor.id, "pull").await.unwrap().item.unwrap();
    let scope = "repo:repo:pull_request";
    let run_id = store
        .begin_sync(&actor.id, &actor.authorization_epoch, scope)
        .await
        .unwrap();
    for (page, batch) in ids.chunks(100).enumerate() {
        let items = batch
            .iter()
            .enumerate()
            .map(|(index, id)| {
                let mut item = template.clone();
                item.id = id.clone();
                item.provider_id = (1_000_000 + page * 100 + index).to_string();
                item.number = Some(item.provider_id.clone());
                item
            })
            .collect();
        store
            .apply_page(PageCommit {
                account_id: actor.id.clone(),
                authorization_epoch: actor.authorization_epoch.clone(),
                scope: scope.into(),
                run_id: run_id.clone(),
                repositories: vec![],
                items,
                endpoint_aliases: vec![],
                next_cursor: None,
                etag: None,
                last_modified: None,
                not_modified: false,
                complete: false,
                observed_at: "2026-10-03T00:00:00Z".into(),
            })
            .await
            .unwrap();
    }
}

async fn subject_commit(
    store: &Store,
    actor: &RemoteAccount,
    subject: &str,
    facet: DetailFacet,
) -> DetailCommit {
    let lease = store
        .begin_detail(&actor.id, &actor.authorization_epoch, subject, facet)
        .await
        .unwrap();
    let mut page = from_lease(actor, facet, lease);
    page.subject_id = subject.into();
    page
}

async fn save_bodies(store: &Store, actor: &RemoteAccount, ids: &[String]) {
    for id in ids {
        store
            .apply_detail(subject_commit(store, actor, id, DetailFacet::Body).await)
            .await
            .unwrap();
    }
}

fn resumed_query(function: &str) -> &'static str {
    // Exercise the literal query passed to SQLx by production, rather than an
    // independently copied query that could stay fast while production regresses.
    let source = include_str!("../src/storage/retention.rs");
    source
        .split_once(&format!("async fn {function}("))
        .expect("The production keyset function exists")
        .1
        .split_once("sqlx::query(\"")
        .expect("The resumed branch passes its query to SQLx")
        .1
        .split_once("\")")
        .expect("The resumed query is one SQL string literal")
        .0
}

fn pin_lookup_query() -> &'static str {
    include_str!("../src/storage/retention.rs")
        .split_once("let identity = sqlx::query(")
        .expect("The production pin identity query exists")
        .1
        .trim_start()
        .strip_prefix('"')
        .expect("The pin identity query is one SQL string literal")
        .split_once("\",")
        .expect("The pin identity query closes before its SQLx arguments")
        .0
}

#[tokio::test]
async fn resumed_keysets_use_indexed_tuple_range_searches_without_prefix_scans() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("cache.sqlite");
    let store = Store::open(&path).await.unwrap();
    let actor = seed(&store, "a").await;
    let ids = (0..8)
        .map(|i| format!("subject-{i:03}"))
        .collect::<Vec<_>>();
    add_subjects(&store, &actor, &ids).await;
    save_bodies(&store, &actor, &ids).await;
    complete_index(&store).await;
    let mut connection = connect(&path).await;

    let history = resumed_query("index_historical_in");
    assert!(history.contains("WHERE (account_id,subject_id,facet)>(?,?,?)"));
    let rows = sqlx::query(AssertSqlSafe(format!("EXPLAIN QUERY PLAN {history}")))
        .bind("a")
        .bind("subject-003")
        .bind("body")
        .bind(128_i64)
        .fetch_all(&mut connection)
        .await
        .unwrap();
    let plan = rows
        .into_iter()
        .map(|row| row.get::<String, _>("detail").to_ascii_uppercase())
        .collect::<Vec<_>>();
    assert!(
        plan.iter()
            .any(|detail| detail.contains("SEARCH DETAIL_OBSERVATIONS")
                && detail.contains("USING COVERING INDEX")
                && detail.contains("(ACCOUNT_ID,SUBJECT_ID,FACET)>(?,?,?)")),
        "Historical resume must seek the composite primary-key range: {plan:?}"
    );
    assert!(
        plan.iter()
            .all(|detail| !detail.contains("SCAN DETAIL_OBSERVATIONS")
                && !detail.contains("TEMP B-TREE")),
        "No prefix scan or sorting of the historical table: {plan:?}"
    );

    let eviction = resumed_query("evict_in");
    assert!(
        eviction.contains("WHERE (last_observed_revision,account_id,subject_id,facet)>(?,?,?,?)")
    );
    let revision: i64 = sqlx::query_scalar("SELECT last_observed_revision FROM cache_retention_entries WHERE account_id='a' AND subject_id='subject-003' AND facet='body'")
        .fetch_one(&mut connection).await.unwrap();
    let rows = sqlx::query(AssertSqlSafe(format!("EXPLAIN QUERY PLAN {eviction}")))
        .bind(revision)
        .bind("a")
        .bind("subject-003")
        .bind("body")
        .bind(128_i64)
        .fetch_all(&mut connection)
        .await
        .unwrap();
    let plan = rows
        .into_iter()
        .map(|row| row.get::<String, _>("detail").to_ascii_uppercase())
        .collect::<Vec<_>>();
    assert!(
        plan.iter()
            .any(|detail| detail.contains("SEARCH CACHE_RETENTION_ENTRIES")
                && detail.contains("USING INDEX CACHE_RETENTION_EVICTION_ORDER")
                && detail
                    .contains("(LAST_OBSERVED_REVISION,ACCOUNT_ID,SUBJECT_ID,FACET)>(?,?,?,?)")),
        "Eviction resume must seek the numeric revision plus primary-key range: {plan:?}"
    );
    assert!(
        plan.iter()
            .all(|detail| !detail.contains("SCAN CACHE_RETENTION_ENTRIES")
                && !detail.contains("TEMP B-TREE")),
        "No prefix scan or sorting of the accounting ledger: {plan:?}"
    );

    let pin_lookup = pin_lookup_query();
    let rows = sqlx::query(AssertSqlSafe(format!("EXPLAIN QUERY PLAN {pin_lookup}")))
        .bind("a")
        .bind("pull")
        .fetch_all(&mut connection)
        .await
        .unwrap();
    let plan = rows
        .into_iter()
        .map(|row| row.get::<String, _>("detail").to_ascii_uppercase())
        .collect::<Vec<_>>();
    assert!(
        plan.iter().any(|detail| {
            detail.contains("SEARCH RI")
                && detail.contains("ACCOUNT_ID=?")
                && detail.contains("INSTANCE_ID=?")
                && detail.contains("ENTITY_ID=?")
        }),
        "Pin validation must use the exact canonical identity key: {plan:?}"
    );
    assert!(
        plan.iter().all(|detail| !detail.contains("SCAN ")),
        "Pin validation must not scan an account's retained identities: {plan:?}"
    );
}

#[tokio::test]
async fn entry_budget_skips_never_fit_facets_but_defers_facets_that_fit_a_fresh_call() {
    // An oversized collection cannot monopolize a small caller-selected budget.
    {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("cache.sqlite");
        let store = Store::open(&path).await.unwrap();
        let actor = seed(&store, "a").await;
        complete_index(&store).await;
        let mut comments = commit(&store, &actor, DetailFacet::Comments).await;
        comments.entries = vec![entry("one"), entry("two")];
        store.apply_detail(comments).await.unwrap();
        store
            .apply_detail(commit(&store, &actor, DetailFacet::Body).await)
            .await
            .unwrap();
        let first = store
            .run_cache_maintenance(CacheRetentionPolicy {
                max_scan_facets: 1,
                max_entry_rows: 1,
                ..policy()
            })
            .await
            .unwrap();
        assert_eq!(first.scanned_facets, 1);
        assert_eq!(first.evicted_facets, 0);
        assert_eq!(first.evicted_entry_rows, 0);
        assert!(!first.target_met);
        let mut connection = connect(&path).await;
        let cursor: (String, String) = sqlx::query_as("SELECT eviction_cursor_subject_id,eviction_cursor_facet FROM cache_retention_state WHERE singleton=1")
            .fetch_one(&mut connection).await.unwrap();
        assert_eq!(
            cursor,
            ("pull".into(), "comments".into()),
            "A never-fit facet advances the durable scan cursor"
        );
        store.close().await;
        drop(store);
        let store = Store::open(&path).await.unwrap();
        let second = store
            .run_cache_maintenance(CacheRetentionPolicy {
                max_scan_facets: 1,
                max_entry_rows: 1,
                ..policy()
            })
            .await
            .unwrap();
        assert_eq!(second.scanned_facets, 1);
        assert_eq!(second.evicted_facets, 1);
        assert_eq!(second.evicted_entry_rows, 0);
        assert!(!second.target_met);
        assert_eq!(
            store
                .detail(query("a", DetailFacet::Body))
                .await
                .unwrap()
                .evidence
                .availability,
            DetailAvailability::Missing
        );
        assert_eq!(
            store
                .detail(query("a", DetailFacet::Comments))
                .await
                .unwrap()
                .entries
                .len(),
            2
        );
        assert_accounted(&store, &mut connection).await;
    }

    // A collection that fits the total budget stays next when only the current
    // call's remainder is too small, rather than being skipped by its cursor.
    {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("cache.sqlite");
        let store = Store::open(&path).await.unwrap();
        let actor = seed(&store, "a").await;
        complete_index(&store).await;
        for facet in [DetailFacet::Comments, DetailFacet::Reviews] {
            let mut page = commit(&store, &actor, facet).await;
            page.entries = vec![entry("one"), entry("two")];
            store.apply_detail(page).await.unwrap();
        }
        store
            .apply_detail(commit(&store, &actor, DetailFacet::Body).await)
            .await
            .unwrap();
        let first = store
            .run_cache_maintenance(CacheRetentionPolicy {
                max_entry_rows: 3,
                ..policy()
            })
            .await
            .unwrap();
        assert_eq!(first.scanned_facets, 2);
        assert_eq!(first.evicted_facets, 1);
        assert_eq!(first.evicted_entry_rows, 2);
        assert!(!first.target_met);
        let mut connection = connect(&path).await;
        let cursor: (String, String) = sqlx::query_as("SELECT eviction_cursor_subject_id,eviction_cursor_facet FROM cache_retention_state WHERE singleton=1")
            .fetch_one(&mut connection).await.unwrap();
        assert_eq!(
            cursor,
            ("pull".into(), "comments".into()),
            "The remaining-budget refusal keeps Reviews next"
        );
        assert_eq!(
            store
                .detail(query("a", DetailFacet::Reviews))
                .await
                .unwrap()
                .entries
                .len(),
            2
        );
        assert_eq!(
            store
                .detail(query("a", DetailFacet::Body))
                .await
                .unwrap()
                .body
                .state,
            DetailValueState::Known
        );
        let second = store
            .run_cache_maintenance(CacheRetentionPolicy {
                max_entry_rows: 3,
                ..policy()
            })
            .await
            .unwrap();
        assert_eq!(second.scanned_facets, 2);
        assert_eq!(second.evicted_facets, 2);
        assert_eq!(second.evicted_entry_rows, 2);
        assert!(second.target_met);
        assert_eq!(count(&mut connection, "detail_observations").await, 0);
        assert_accounted(&store, &mut connection).await;
    }
}

#[tokio::test]
async fn forward_migration_preserves_frozen_bytes_and_starts_with_empty_accounting() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("cache.sqlite");
    frozen_v8(&path).await;
    let mut connection = connect(&path).await;
    let before = snapshot(&mut connection, ORIGINAL_TABLES).await;
    let store = Store::open(&path).await.unwrap();
    assert_eq!(snapshot(&mut connection, ORIGINAL_TABLES).await, before);
    assert_eq!(count(&mut connection, "cache_pins").await, 0);
    assert_eq!(count(&mut connection, "cache_retention_entries").await, 0);
    let usage = store.cache_usage().await.unwrap();
    assert!(!usage.index_complete);
    assert_eq!(
        usage.logical_bytes, None,
        "An empty ledger is not an empty historical cache"
    );
    assert_eq!(usage.indexed_facets, 0);
    assert!(usage.database_bytes > 0);
    assert!(usage.page_count > 0);
    assert!(usage.page_size > 0);
    assert_integrity(&mut connection).await;
}

#[tokio::test]
async fn pins_are_kind_checked_account_scoped_idempotent_and_survive_disconnect_and_restart() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("cache.sqlite");
    let store = Store::open(&path).await.unwrap();
    seed(&store, "a").await;
    seed(&store, "b").await;
    let mut issue = store.item("a", "pull").await.unwrap().item.unwrap();
    issue.id = "issue".into();
    issue.provider_id = "123456".into();
    issue.number = Some("68".into());
    issue.kind = RemoteItemKind::Issue;
    let run_id = store.begin_sync("a", "1", "repo:repo:issue").await.unwrap();
    store
        .apply_page(PageCommit {
            account_id: "a".into(),
            authorization_epoch: "1".into(),
            scope: "repo:repo:issue".into(),
            run_id,
            repositories: vec![],
            items: vec![issue],
            endpoint_aliases: vec![],
            next_cursor: None,
            etag: None,
            last_modified: None,
            not_modified: false,
            complete: true,
            observed_at: "2026-10-03T00:00:00Z".into(),
        })
        .await
        .unwrap();
    let before = store.revision().await.unwrap();
    let pinned = store.set_cache_pin("a", "pull", true).await.unwrap();
    assert_ne!(pinned, before);
    assert_eq!(
        store.set_cache_pin("a", "pull", true).await.unwrap(),
        pinned
    );
    assert_eq!(
        store.revision().await.unwrap(),
        pinned,
        "Idempotent pin creates no new revision"
    );
    let changes = store.changes_since(&before).await.unwrap();
    assert_eq!(changes.changes.len(), 1);
    assert_eq!(changes.changes[0].account_id, "a");
    assert_eq!(changes.changes[0].scope, "pins");
    assert!(!changes.changes[0].reset);
    let mut connection = connect(&path).await;
    assert_eq!(count(&mut connection, "cache_pins").await, 1);
    store.set_cache_pin("a", "issue", true).await.unwrap();
    assert_eq!(
        count(&mut connection, "cache_pins").await,
        2,
        "Both canonical PR and issue kinds are pinnable"
    );
    for (account_id, subject_id, kind) in [
        ("b", "pull", "issue"),
        ("a", "repo", "repository"),
        ("missing", "pull", "pull_request"),
    ] {
        assert!(sqlx::query("INSERT INTO cache_pins(account_id,instance_id,entity_id,kind,pinned_revision) VALUES(?,'github:https://github.com/',?,?,1)")
            .bind(account_id).bind(subject_id).bind(kind).execute(&mut connection).await.is_err(),
            "The SQLite constraints reject mismatched kind/account identities");
    }
    assert!(sqlx::query("INSERT INTO cache_pins(account_id,instance_id,entity_id,kind,pinned_revision) VALUES(NULL,'github:https://github.com/','pull','pull_request',1)")
        .execute(&mut connection).await.is_err());
    assert!(
        sqlx::query("UPDATE cache_pins SET kind='issue' WHERE account_id='a' AND entity_id='pull'")
            .execute(&mut connection)
            .await
            .is_err(),
        "The update trigger prevents relabeling an existing canonical identity"
    );
    assert!(
        sqlx::query("UPDATE resource_identities SET kind='repository' WHERE account_id='a' AND entity_id='pull'")
            .execute(&mut connection)
            .await
            .is_err(),
        "The parent trigger prevents relabeling an identity behind its pin"
    );
    assert!(store.set_cache_pin("a", "repo", true).await.is_err());
    assert!(store.set_cache_pin("a", "missing", true).await.is_err());
    let pin_bytes = snapshot(&mut connection, &["cache_pins"]).await;
    store.disconnect("a").await.unwrap();
    assert_eq!(snapshot(&mut connection, &["cache_pins"]).await, pin_bytes);
    store.close().await;
    drop(store);
    connection.close().await.unwrap();
    let reopened = Store::open(&path).await.unwrap();
    let mut connection = connect(&path).await;
    assert_eq!(snapshot(&mut connection, &["cache_pins"]).await, pin_bytes);
    reopened.set_cache_pin("a", "pull", false).await.unwrap();
    reopened.set_cache_pin("a", "issue", false).await.unwrap();
    assert_eq!(
        count(&mut connection, "cache_pins").await,
        0,
        "A disconnected actor can edit durable local pins"
    );
    reopened.set_cache_pin("a", "pull", true).await.unwrap();
    let mut reconnect = account("a");
    reconnect.authorization_epoch = "3".into();
    let reconnected = reopened.upsert_account(reconnect).await.unwrap();
    project(&reopened, &reconnected).await;
    reopened
        .apply_detail(commit(&reopened, &reconnected, DetailFacet::Body).await)
        .await
        .unwrap();
    complete_index(&reopened).await;
    assert_eq!(
        reopened
            .run_cache_maintenance(policy())
            .await
            .unwrap()
            .evicted_facets,
        0,
        "An offline pin protects content rebuilt after the account reconnects"
    );
}

#[tokio::test]
async fn ledger_constraints_and_cascades_keep_the_constant_time_aggregate_consistent() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("cache.sqlite");
    let store = Store::open(&path).await.unwrap();
    let actor = seed(&store, "a").await;
    complete_index(&store).await;
    store
        .apply_detail(commit(&store, &actor, DetailFacet::Body).await)
        .await
        .unwrap();
    let mut connection = connect(&path).await;
    for statement in [
        "INSERT INTO cache_retention_entries VALUES('a','missing','body',1,1)",
        "INSERT INTO cache_retention_entries VALUES(NULL,'pull','body',1,1)",
        "UPDATE cache_retention_entries SET logical_bytes=-1",
        "UPDATE cache_retention_entries SET last_observed_revision=0",
        "INSERT INTO cache_retention_entries SELECT * FROM cache_retention_entries",
    ] {
        assert!(
            sqlx::query(statement)
                .execute(&mut connection)
                .await
                .is_err()
        );
    }
    assert_accounted(&store, &mut connection).await;
    sqlx::query("DELETE FROM detail_observations WHERE account_id='a' AND subject_id='pull' AND facet='body'")
        .execute(&mut connection).await.unwrap();
    assert_eq!(count(&mut connection, "cache_retention_entries").await, 0);
    assert_accounted(&store, &mut connection).await;
    let usage = store.cache_usage().await.unwrap();
    assert_eq!(usage.logical_bytes, Some(0));
    assert_eq!(usage.indexed_facets, 0);
    assert_integrity(&mut connection).await;
}

#[tokio::test]
async fn notification_point_discovery_accounts_the_shared_detail_transaction() {
    use collaboration::providers::{DetailPage, NotificationSubjectDiscovery};
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("cache.sqlite");
    let store = Store::open(&path).await.unwrap();
    let actor = seed(&store, "a").await;
    complete_index(&store).await;
    let mut subject = store.item("a", "pull").await.unwrap().item.unwrap();
    subject.id = "discovered-pull".into();
    subject.provider_id = "12345".into();
    subject.number = Some("68".into());
    let mut notification = subject.clone();
    notification.id = "notification".into();
    notification.provider_id = "thread-1".into();
    notification.kind = RemoteItemKind::Notification;
    notification.number = None;
    notification.body = None;
    notification.body_omitted = true;
    notification.reason = Some("review_requested".into());
    notification.unread = Some(true);
    let run_id = store
        .begin_sync("a", &actor.authorization_epoch, "notifications")
        .await
        .unwrap();
    store
        .apply_page_with_notification_subjects(
            PageCommit {
                account_id: "a".into(),
                authorization_epoch: actor.authorization_epoch.clone(),
                scope: "notifications".into(),
                run_id,
                repositories: vec![],
                items: vec![notification],
                endpoint_aliases: vec![],
                next_cursor: None,
                etag: None,
                last_modified: None,
                not_modified: false,
                complete: true,
                observed_at: "2026-10-03T00:00:00Z".into(),
            },
            vec![NotificationSubjectObservation {
                notification_id: "notification".into(),
                mapping: NotificationSubjectMapping::Selector(NotificationSubjectSelector {
                    kind: NotificationSubjectKind::PullRequest,
                    repository_provider_id: "1".into(),
                    number: "68".into(),
                    repository_path: "owner/project".into(),
                    representation: NotificationSubjectRepresentation::GithubPullRequest,
                }),
            }],
        )
        .await
        .unwrap();
    let snapshot = store
        .notification_subject(
            NotificationSubjectQuery {
                account_id: "a".into(),
                authorization_epoch: actor.authorization_epoch.clone(),
                notification_id: "notification".into(),
            },
            |_, _, _| CapabilityState::Supported,
        )
        .await
        .unwrap();
    store
        .request_notification_subject_checked(
            &DiscoverNotificationSubjectRequest {
                account_id: "a".into(),
                authorization_epoch: actor.authorization_epoch.clone(),
                notification_id: "notification".into(),
                selector_generation: snapshot.selector_generation.unwrap(),
            },
            || Ok(()),
        )
        .await
        .unwrap();
    let intent = store
        .pending_notification_subjects()
        .await
        .unwrap()
        .into_iter()
        .next()
        .unwrap();
    let lease = store.begin_notification_subject(&intent).await.unwrap();
    let detail_source = source(DetailFacet::Body);
    store
        .apply_notification_subject(
            &lease,
            NotificationSubjectDiscovery::Verified {
                subject: Box::new(subject),
                detail: Box::new(DetailPage {
                    reconciliation: DetailReconciliation::full_history(),
                    body: known(Some("point-discovered café 🦀")),
                    metadata: None,
                    entries: vec![],
                    source: detail_source,
                    next_cursor: None,
                    etag: Some("point-body".into()),
                    not_modified: false,
                    freshness_seconds: 60,
                    cooldown_seconds: None,
                }),
                endpoint_aliases: vec![],
            },
        )
        .await
        .unwrap();
    let mut connection = connect(&path).await;
    assert_accounted(&store, &mut connection).await;
    let mut q = query("a", DetailFacet::Body);
    q.subject_id = "discovered-pull".into();
    assert_eq!(
        store.detail(q).await.unwrap().body.text.as_deref(),
        Some("point-discovered café 🦀")
    );
    assert_eq!(store.cache_usage().await.unwrap().indexed_facets, 1);
}

#[tokio::test]
async fn accepted_reconciliation_retained_values_304_and_summary_metadata_refresh_accounting() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("cache.sqlite");
    let store = Store::open(&path).await.unwrap();
    let actor = seed(&store, "a").await;
    complete_index(&store).await;
    let mut connection = connect(&path).await;
    let mut page = commit(&store, &actor, DetailFacet::Comments).await;
    page.entries = vec![entry("one"), entry("two")];
    page.entries[0].body = known(Some("retained café 🦀"));
    store.apply_detail(page).await.unwrap();
    assert_accounted(&store, &mut connection).await;
    let mut page = commit(&store, &actor, DetailFacet::Comments).await;
    page.entries = vec![entry("one")];
    page.entries[0].body = DetailValue {
        state: DetailValueState::Omitted,
        text: None,
    };
    page.source.observed_at = "2026-10-03T00:01:00.123456789Z".into();
    store.apply_detail(page).await.unwrap();
    let saved = store
        .detail(query("a", DetailFacet::Comments))
        .await
        .unwrap();
    assert_eq!(
        saved.entries.len(),
        1,
        "Complete enumeration prunes the missing member"
    );
    assert_eq!(
        saved.entries[0].body.text.as_deref(),
        Some("retained café 🦀")
    );
    assert_eq!(
        saved.entries[0].observed_body_state,
        DetailValueState::Omitted
    );
    assert_accounted(&store, &mut connection).await;
    let before_304 = store.cache_usage().await.unwrap().indexed_logical_bytes;
    let mut page = commit(&store, &actor, DetailFacet::Comments).await;
    page.not_modified = true;
    page.source.observed_at = "2026-10-03T00:02:00Z".into();
    store.apply_detail(page).await.unwrap();
    assert_accounted(&store, &mut connection).await;
    assert_ne!(
        store.cache_usage().await.unwrap().indexed_logical_bytes,
        before_304,
        "Validation updates persisted clock byte lengths"
    );

    let mut body = commit(&store, &actor, DetailFacet::Body).await;
    body.subject_binding = Some(DetailSubjectBinding {
        repository_id: "repo".into(),
        repository_provider_id: "1".into(),
        provider_id: "9007199254740997".into(),
        number: Some("67".into()),
        kind: RemoteItemKind::PullRequest,
        head_oid: None,
    });
    body.metadata = Some(ResourceMetadataObservation {
        kind: RemoteItemKind::PullRequest,
        values: ResourceMetadataValues {
            title: Some("saved metadata café 🦀".into()),
            head: Some(DetailBranch {
                name: "topic".into(),
                oid: "old-head".into(),
                repository: None,
            }),
            ..ResourceMetadataValues::default()
        },
        fields: vec![
            MetadataObservedField {
                field: MetadataField::Title,
                state: DetailValueState::Known,
            },
            MetadataObservedField {
                field: MetadataField::Head,
                state: DetailValueState::Known,
            },
        ],
        source: MetadataSource {
            source: body.source.source.clone(),
            adapter_version: 1,
            provider_updated_at: None,
            observed_at: body.source.observed_at.clone(),
        },
    });
    store.apply_detail(body).await.unwrap();
    assert_accounted(&store, &mut connection).await;
    let metadata_before_head = snapshot(&mut connection, &["detail_resource_metadata"]).await;
    let mut item = store.item("a", "pull").await.unwrap().item.unwrap();
    item.head_oid = Some("new-head".into());
    item.updated_at = "2026-10-03T00:03:00Z".into();
    let run_id = store
        .begin_sync("a", &actor.authorization_epoch, "repo:repo:pull_request")
        .await
        .unwrap();
    store
        .apply_page(PageCommit {
            account_id: "a".into(),
            authorization_epoch: actor.authorization_epoch.clone(),
            scope: "repo:repo:pull_request".into(),
            run_id,
            repositories: vec![],
            items: vec![item],
            endpoint_aliases: vec![],
            next_cursor: None,
            etag: None,
            last_modified: None,
            not_modified: false,
            complete: true,
            observed_at: "2026-10-03T00:03:00Z".into(),
        })
        .await
        .unwrap();
    assert_accounted(&store, &mut connection).await;
    assert_ne!(
        snapshot(&mut connection, &["detail_resource_metadata"]).await,
        metadata_before_head,
        "Summary head invalidation rewrites the persisted metadata represented by accounting"
    );
}

#[tokio::test]
async fn accounting_abort_rolls_back_content_coverage_ledger_and_revision() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("cache.sqlite");
    let store = Store::open(&path).await.unwrap();
    let actor = seed(&store, "a").await;
    complete_index(&store).await;
    store
        .apply_detail(commit(&store, &actor, DetailFacet::Comments).await)
        .await
        .unwrap();
    let mut page = commit(&store, &actor, DetailFacet::Comments).await;
    page.entries = vec![entry("new")];
    let mut connection = connect(&path).await;
    let mut tables = ORIGINAL_TABLES.to_vec();
    tables.extend(["cache_retention_entries", "cache_retention_state"]);
    let before = snapshot(&mut connection, &tables).await;
    sqlx::raw_sql("CREATE TRIGGER abort_retention_insert BEFORE INSERT ON cache_retention_entries BEGIN SELECT RAISE(ABORT,'retention fixture abort'); END; CREATE TRIGGER abort_retention_update BEFORE UPDATE ON cache_retention_entries BEGIN SELECT RAISE(ABORT,'retention fixture abort'); END;")
        .execute(&mut connection).await.unwrap();
    assert_eq!(
        store.apply_detail(page).await.unwrap_err().code,
        ErrorCode::Storage
    );
    assert_eq!(snapshot(&mut connection, &tables).await, before);
    sqlx::raw_sql("DROP TRIGGER abort_retention_insert; DROP TRIGGER abort_retention_update;")
        .execute(&mut connection)
        .await
        .unwrap();
    assert_accounted(&store, &mut connection).await;
}

#[tokio::test]
async fn historical_index_is_bounded_reopens_and_admits_new_rows_on_both_sides_of_cursor() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("cache.sqlite");
    frozen_v8(&path).await;
    let mut connection = connect(&path).await;
    for index in 0..260 {
        sqlx::query("INSERT INTO detail_observations SELECT account_id,?,facet,authorization_epoch,facet_revision,body_json,source_json,value_source_json,observed_state,stale_at FROM detail_observations WHERE account_id='a' AND subject_id='pull-request-67' AND facet='body'")
            .bind(format!("legacy-{index:04}")).execute(&mut connection).await.unwrap();
    }
    let store = Store::open(&path).await.unwrap();
    let total = count(&mut connection, "detail_observations").await;
    let first = store.run_cache_maintenance(policy()).await.unwrap();
    assert_eq!(first.indexed_facets, 128);
    assert_eq!(first.evicted_facets, 0);
    assert!(!first.usage_after.index_complete);
    assert_eq!(first.usage_after.logical_bytes, None);
    assert_eq!(count(&mut connection, "detail_observations").await, total);
    store.close().await;
    drop(store);
    connection.close().await.unwrap();

    let store = Store::open(&path).await.unwrap();
    let actor = store.account("a").await.unwrap();
    project(&store, &actor).await;
    let ids = vec!["000-new-before".into(), "zzz-new-after".into()];
    add_subjects(&store, &actor, &ids).await;
    save_bodies(&store, &actor, &ids).await;
    let mut connection = connect(&path).await;
    let zero_limit = store
        .run_cache_maintenance(CacheRetentionPolicy {
            max_index_facets: 0,
            ..policy()
        })
        .await
        .unwrap();
    assert!(
        zero_limit.indexed_facets > 0,
        "Zero indexing limit cannot disable progress"
    );
    assert!(zero_limit.indexed_facets <= 128);
    assert_eq!(zero_limit.evicted_facets, 0);
    let mut completed = false;
    for _ in 0..4 {
        let report = store.run_cache_maintenance(policy()).await.unwrap();
        assert!(report.indexed_facets <= 128);
        assert_eq!(
            report.evicted_facets, 0,
            "The completing call preserves the indexing/eviction phase boundary"
        );
        if report.usage_after.index_complete {
            completed = true;
            break;
        }
        assert_eq!(report.usage_after.logical_bytes, None);
    }
    assert!(completed);
    assert_eq!(
        count(&mut connection, "detail_observations").await,
        total + 2
    );
    assert_accounted(&store, &mut connection).await;
}

#[tokio::test]
async fn oversized_and_zero_limits_never_exceed_parent_scan_or_eviction_caps() {
    for zero_limits in [false, true] {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("cache.sqlite");
        let store = Store::open(&path).await.unwrap();
        let actor = seed(&store, "a").await;
        let ids = (0..40)
            .map(|i| format!("subject-{i:03}"))
            .collect::<Vec<_>>();
        add_subjects(&store, &actor, &ids).await;
        save_bodies(&store, &actor, &ids).await;
        let mut requested_policy = policy();
        if zero_limits {
            requested_policy.max_index_facets = 0;
            requested_policy.max_scan_facets = 0;
            requested_policy.max_evict_facets = 0;
            requested_policy.max_entry_rows = 0;
        }
        let index = store
            .run_cache_maintenance(CacheRetentionPolicy {
                target_logical_bytes: u64::MAX,
                ..policy()
            })
            .await
            .unwrap();
        assert!(index.usage_after.index_complete);
        assert!(index.indexed_facets <= 128);
        assert_eq!(index.evicted_facets, 0);
        let eviction = store.run_cache_maintenance(requested_policy).await.unwrap();
        assert!(!eviction.skipped_busy);
        assert!(eviction.scanned_facets <= 128);
        assert!(
            eviction.evicted_facets > 0,
            "Zero values cannot disable maintenance work"
        );
        assert!(eviction.evicted_facets <= 32);
        if !zero_limits {
            assert_eq!(eviction.evicted_facets, 32);
        }
        assert_eq!(eviction.evicted_entry_rows, 0);
        assert!(!eviction.target_met);
        assert_eq!(
            eviction.usage_after.indexed_facets,
            40 - u64::from(eviction.evicted_facets)
        );
        assert!(eviction.freed_logical_bytes > 0);
        let mut connection = connect(&path).await;
        assert_accounted(&store, &mut connection).await;
    }
}

#[tokio::test]
async fn one_maximum_child_facet_consumes_the_entire_entry_row_budget() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("cache.sqlite");
    let store = Store::open(&path).await.unwrap();
    let actor = seed(&store, "a").await;
    complete_index(&store).await;
    let mut comments = commit(&store, &actor, DetailFacet::Comments).await;
    comments.entries = vec![entry("entry-0000")];
    store.apply_detail(comments).await.unwrap();
    let mut connection = connect(&path).await;
    sqlx::query("WITH RECURSIVE n(i) AS (SELECT 1 UNION ALL SELECT i+1 FROM n WHERE i<4999) INSERT INTO detail_entries SELECT e.account_id,e.subject_id,e.facet,printf('entry-%04d',n.i),json_set(e.json,'$.id',printf('entry-%04d',n.i),'$.provider_id',printf('entry-%04d',n.i)),e.last_seen_run FROM detail_entries e,n WHERE e.account_id='a' AND e.subject_id='pull' AND e.facet='comments' AND e.id='entry-0000'")
        .execute(&mut connection).await.unwrap();
    // A genuine accepted validation accounts the enlarged historical collection.
    let mut validation = commit(&store, &actor, DetailFacet::Comments).await;
    validation.not_modified = true;
    store.apply_detail(validation).await.unwrap();
    let mut reviews = commit(&store, &actor, DetailFacet::Reviews).await;
    reviews.entries = vec![entry("review")];
    store.apply_detail(reviews).await.unwrap();
    assert_eq!(count(&mut connection, "detail_entries").await, 5001);
    assert_accounted(&store, &mut connection).await;
    let first = store.run_cache_maintenance(policy()).await.unwrap();
    assert_eq!(first.evicted_facets, 1);
    assert_eq!(first.evicted_entry_rows, 5000);
    assert_eq!(count(&mut connection, "detail_entries").await, 1);
    assert!(!first.target_met);
    let second = store.run_cache_maintenance(policy()).await.unwrap();
    assert_eq!(second.evicted_facets, 1);
    assert_eq!(second.evicted_entry_rows, 1);
    assert!(second.target_met);
    assert_accounted(&store, &mut connection).await;
}

#[tokio::test]
async fn protected_prefix_advances_a_bounded_cursor_and_wrap_reconsiders_unpinned_rows() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("cache.sqlite");
    let store = Store::open(&path).await.unwrap();
    let actor = seed(&store, "a").await;
    let ids = (0..130)
        .map(|i| format!("subject-{i:03}"))
        .collect::<Vec<_>>();
    add_subjects(&store, &actor, &ids).await;
    save_bodies(&store, &actor, &ids).await;
    for id in &ids[..129] {
        store.set_cache_pin("a", id, true).await.unwrap();
    }
    complete_index(&store).await;
    let first = store.run_cache_maintenance(policy()).await.unwrap();
    assert_eq!(first.scanned_facets, 128);
    assert_eq!(first.evicted_facets, 0);
    assert!(!first.target_met);
    let mut connection = connect(&path).await;
    let cursor_subject: String = sqlx::query_scalar(
        "SELECT eviction_cursor_subject_id FROM cache_retention_state WHERE singleton=1",
    )
    .fetch_one(&mut connection)
    .await
    .unwrap();
    assert_eq!(
        cursor_subject, ids[127],
        "The saved cursor stops at the bounded protected prefix"
    );
    assert_eq!(count(&mut connection, "detail_observations").await, 130);
    store.close().await;
    drop(store);
    let store = Store::open(&path).await.unwrap();
    let second = store.run_cache_maintenance(policy()).await.unwrap();
    assert!(second.scanned_facets <= 128);
    assert_eq!(
        second.evicted_facets, 1,
        "Protected prefix cannot hide the eligible suffix forever"
    );
    store.set_cache_pin("a", &ids[0], false).await.unwrap();
    let mut evicted = false;
    for _ in 0..3 {
        let report = store.run_cache_maintenance(policy()).await.unwrap();
        assert!(report.scanned_facets <= 128);
        assert!(report.evicted_facets <= 32);
        if report.evicted_facets == 1 {
            evicted = true;
            break;
        }
    }
    assert!(
        evicted,
        "Reaching the end wraps the cursor to newly unpinned older rows"
    );
    assert_eq!(count(&mut connection, "detail_observations").await, 128);
    assert_accounted(&store, &mut connection).await;
}

#[tokio::test]
async fn eviction_orders_revision_numbers_before_primary_key_ties() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("cache.sqlite");
    let store = Store::open(&path).await.unwrap();
    let actor = seed(&store, "a").await;
    let ids = vec![
        "z-nine".into(),
        "b-ten".into(),
        "a-ten".into(),
        "a-hundred".into(),
    ];
    add_subjects(&store, &actor, &ids).await;
    save_bodies(&store, &actor, &ids).await;
    complete_index(&store).await;
    let mut connection = connect(&path).await;
    for (subject, revision) in [
        ("z-nine", 9),
        ("b-ten", 10),
        ("a-ten", 10),
        ("a-hundred", 100),
    ] {
        // Historical revision spellings deliberately cross decimal widths.
        sqlx::query(
            "UPDATE detail_observations SET facet_revision=? WHERE account_id='a' AND subject_id=?",
        )
        .bind(revision.to_string())
        .bind(subject)
        .execute(&mut connection)
        .await
        .unwrap();
        sqlx::query("UPDATE cache_retention_entries SET last_observed_revision=? WHERE account_id='a' AND subject_id=?")
            .bind(revision).bind(subject).execute(&mut connection).await.unwrap();
    }
    for subject in ["z-nine", "a-ten", "b-ten", "a-hundred"] {
        let report = store
            .run_cache_maintenance(CacheRetentionPolicy {
                max_evict_facets: 1,
                ..policy()
            })
            .await
            .unwrap();
        assert_eq!(report.evicted_facets, 1);
        assert_eq!(
            sqlx::query_scalar::<_, i64>(
                "SELECT count(*) FROM detail_observations WHERE account_id='a' AND subject_id=?"
            )
            .bind(subject)
            .fetch_one(&mut connection)
            .await
            .unwrap(),
            0,
            "The next eviction is the oldest numeric revision, with stable primary-key ordering"
        );
        assert_accounted(&store, &mut connection).await;
    }
}

#[tokio::test]
async fn pins_requested_demand_and_syncing_are_exact_protections_across_accounts() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("cache.sqlite");
    let store = Store::open(&path).await.unwrap();
    let a = seed(&store, "a").await;
    let b = seed(&store, "b").await;
    complete_index(&store).await;
    for actor in [&a, &b] {
        for facet in [
            DetailFacet::Body,
            DetailFacet::Comments,
            DetailFacet::Reviews,
        ] {
            store
                .apply_detail(commit(&store, actor, facet).await)
                .await
                .unwrap();
        }
    }
    store.set_cache_pin("a", "pull", true).await.unwrap();
    store
        .request_detail("b", &b.authorization_epoch, "pull", DetailFacet::Comments)
        .await
        .unwrap();
    store
        .begin_detail("b", &b.authorization_epoch, "pull", DetailFacet::Reviews)
        .await
        .unwrap();
    let mut connection = connect(&path).await;
    let before = snapshot(&mut connection, AUTHORED_AND_IDENTITY_TABLES).await;
    let report = store.run_cache_maintenance(policy()).await.unwrap();
    assert_eq!(
        report.evicted_facets, 1,
        "Account a's pin does not protect account b's Body"
    );
    assert!(!report.target_met);
    assert_eq!(
        snapshot(&mut connection, AUTHORED_AND_IDENTITY_TABLES).await,
        before
    );
    assert_eq!(
        store
            .detail(query("b", DetailFacet::Body))
            .await
            .unwrap()
            .evidence
            .availability,
        DetailAvailability::Missing
    );
    for (account_id, facet) in [
        ("a", DetailFacet::Body),
        ("a", DetailFacet::Comments),
        ("a", DetailFacet::Reviews),
        ("b", DetailFacet::Comments),
        ("b", DetailFacet::Reviews),
    ] {
        assert_ne!(
            store
                .detail(query(account_id, facet))
                .await
                .unwrap()
                .evidence
                .availability,
            DetailAvailability::Missing
        );
    }
    assert_accounted(&store, &mut connection).await;
}

#[tokio::test]
async fn structurally_ineligible_observations_are_retained_without_a_full_eligibility_scan() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("cache.sqlite");
    let store = Store::open(&path).await.unwrap();
    let actor = seed(&store, "a").await;
    let ids = vec![
        "missing-item".into(),
        "missing-identity".into(),
        "missing-scope".into(),
        "eligible".into(),
    ];
    add_subjects(&store, &actor, &ids).await;
    save_bodies(&store, &actor, &ids).await;
    complete_index(&store).await;
    let mut connection = connect(&path).await;
    sqlx::query("DELETE FROM items WHERE account_id='a' AND id='missing-item'")
        .execute(&mut connection)
        .await
        .unwrap();
    sqlx::query(
        "DELETE FROM resource_aliases WHERE account_id='a' AND entity_id='missing-identity'",
    )
    .execute(&mut connection)
    .await
    .unwrap();
    sqlx::query(
        "DELETE FROM resource_identities WHERE account_id='a' AND entity_id='missing-identity'",
    )
    .execute(&mut connection)
    .await
    .unwrap();
    sqlx::query(
        "DELETE FROM sync_scopes WHERE account_id='a' AND scope='detail:missing-scope:body'",
    )
    .execute(&mut connection)
    .await
    .unwrap();
    let report = store.run_cache_maintenance(policy()).await.unwrap();
    assert_eq!(report.scanned_facets, 4);
    assert_eq!(report.evicted_facets, 1);
    assert!(!report.target_met);
    let remaining: Vec<String> =
        sqlx::query_scalar("SELECT subject_id FROM detail_observations ORDER BY subject_id")
            .fetch_all(&mut connection)
            .await
            .unwrap();
    assert_eq!(
        remaining,
        ["missing-identity", "missing-item", "missing-scope"]
    );
    assert_accounted(&store, &mut connection).await;
}

#[tokio::test]
async fn eviction_preserves_authored_frozen_state_and_exact_error_metadata() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("cache.sqlite");
    frozen_v8(&path).await;
    let store = Store::open(&path).await.unwrap();
    let mut connection = connect(&path).await;
    sqlx::query("UPDATE detail_demand SET requested=0")
        .execute(&mut connection)
        .await
        .unwrap();
    store
        .set_sync_status(
            "a",
            "17",
            "provider:rest",
            SyncStatus {
                state: SyncState::RateLimited,
                last_success_at: Some("2026-10-02T12:00:00Z".into()),
                next_retry_at: Some("2099-01-01T00:00:00Z".into()),
                error: Some(CollaborationError::new(
                    ErrorCode::RateLimited,
                    "Synthetic conserved provider quota",
                )),
            },
        )
        .await
        .unwrap();
    let draft_before = store.draft("a", "pull-request-67").await.unwrap().unwrap();
    let before = snapshot(&mut connection, AUTHORED_AND_IDENTITY_TABLES).await;
    let sync_before: Vec<String> = sqlx::query_scalar("SELECT json_array(account_id,scope,access_denied,sync_json) FROM sync_scopes ORDER BY account_id,scope")
        .fetch_all(&mut connection).await.unwrap();
    complete_index(&store).await;
    let report = store.run_cache_maintenance(policy()).await.unwrap();
    assert!(report.evicted_facets > 0);
    assert_eq!(
        snapshot(&mut connection, AUTHORED_AND_IDENTITY_TABLES).await,
        before
    );
    let sync_after: Vec<String> = sqlx::query_scalar("SELECT json_array(account_id,scope,access_denied,sync_json) FROM sync_scopes ORDER BY account_id,scope")
        .fetch_all(&mut connection).await.unwrap();
    assert_eq!(
        sync_after, sync_before,
        "Retention preserves existing scope error/success/quota metadata"
    );
    assert_eq!(
        store.draft("a", "pull-request-67").await.unwrap(),
        Some(draft_before.clone())
    );
    let mut replacement = draft_before.clone();
    replacement
        .body
        .push_str("\nAn authored edit after eviction");
    let saved = store.save_draft(replacement).await.unwrap();
    assert_ne!(saved.generation, draft_before.generation);
    assert_eq!(
        store.save_draft(draft_before).await.unwrap_err().code,
        ErrorCode::StaleView,
        "Retention does not reset the private draft CAS generation"
    );
    assert_accounted(&store, &mut connection).await;
    assert_integrity(&mut connection).await;
}

#[tokio::test]
async fn eviction_rotates_run_clears_proofs_fences_receipts_and_allows_fresh_hydration() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("cache.sqlite");
    let store = Store::open(&path).await.unwrap();
    let actor = seed(&store, "a").await;
    complete_index(&store).await;
    let mut first = commit(&store, &actor, DetailFacet::Comments).await;
    first.entries = vec![entry("one"), entry("two")];
    first.complete = false;
    first.whole_scope = false;
    first.next_cursor = Some("page-two".into());
    store.apply_detail(first).await.unwrap();
    let mut limited = query("a", DetailFacet::Comments);
    limited.limit = 1;
    let local_cursor = store
        .detail(limited.clone())
        .await
        .unwrap()
        .next_cursor
        .unwrap();
    let obsolete = commit(&store, &actor, DetailFacet::Comments).await;
    store
        .set_sync_status(
            "a",
            &actor.authorization_epoch,
            "detail:pull:comments",
            SyncStatus {
                state: SyncState::Offline,
                ..SyncStatus::default()
            },
        )
        .await
        .unwrap();
    let mut connection = connect(&path).await;
    sqlx::query("UPDATE sync_scopes SET etag='obsolete-validator',last_modified='Fri, 02 Oct 2026 12:00:00 GMT',completed_run_id=run_id WHERE account_id='a' AND scope='detail:pull:comments'")
        .execute(&mut connection).await.unwrap();
    let old_run = obsolete.run_id.clone();
    let before_revision = store.revision().await.unwrap();
    let report = store.run_cache_maintenance(policy()).await.unwrap();
    assert_eq!(report.evicted_facets, 1);
    let revision = store.revision().await.unwrap();
    assert_ne!(revision, before_revision);
    let scope = store
        .scope_state("a", "detail:pull:comments")
        .await
        .unwrap()
        .unwrap();
    assert_ne!(scope.run_id, old_run);
    assert_eq!(scope.coverage.state, CoverageState::Missing);
    assert_eq!(scope.coverage.validated_at, None);
    assert!(!scope.coverage.remote_has_more);
    assert_eq!(scope.next_cursor, None);
    assert_eq!(scope.etag, None);
    assert_eq!(scope.last_modified, None);
    let (data_revision, completed): (i64, Option<String>) = sqlx::query_as("SELECT data_revision,completed_run_id FROM sync_scopes WHERE account_id='a' AND scope='detail:pull:comments'")
        .fetch_one(&mut connection).await.unwrap();
    assert_eq!(data_revision.to_string(), revision);
    assert_eq!(completed, None);
    let changes = store.changes_since(&before_revision).await.unwrap();
    assert_eq!(changes.changes.len(), 1);
    assert_eq!(changes.changes[0].scope, "detail:pull:comments");
    let missing = store
        .detail(query("a", DetailFacet::Comments))
        .await
        .unwrap();
    assert_eq!(missing.evidence.availability, DetailAvailability::Missing);
    assert!(missing.entries.is_empty());
    store.close().await;
    drop(store);
    let store = Store::open(&path).await.unwrap();
    assert_eq!(store.revision().await.unwrap(), revision);
    assert_eq!(
        store.apply_detail(obsolete).await.unwrap_err().code,
        ErrorCode::StaleView
    );
    limited.cursor = Some(local_cursor);
    assert_eq!(
        store.detail(limited).await.unwrap_err().code,
        ErrorCode::StaleView
    );
    store
        .request_detail(
            "a",
            &actor.authorization_epoch,
            "pull",
            DetailFacet::Comments,
        )
        .await
        .unwrap();
    let lease = store
        .begin_detail(
            "a",
            &actor.authorization_epoch,
            "pull",
            DetailFacet::Comments,
        )
        .await
        .unwrap();
    assert_eq!(lease.next_cursor, None);
    assert_eq!(
        lease.etag, None,
        "An evicted validator cannot drive a 304 rehydration"
    );
    let mut fresh = from_lease(&actor, DetailFacet::Comments, lease);
    fresh.entries = vec![entry("fresh")];
    store.apply_detail(fresh).await.unwrap();
    let saved = store
        .detail(query("a", DetailFacet::Comments))
        .await
        .unwrap();
    assert_eq!(saved.evidence.coverage.state, CoverageState::Complete);
    assert_eq!(saved.entries[0].id, "fresh");
    assert_accounted(&store, &mut connection).await;
}

#[tokio::test]
async fn eviction_sqlite_abort_rolls_back_cascades_scope_fence_and_change_revision() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("cache.sqlite");
    let store = Store::open(&path).await.unwrap();
    let actor = seed(&store, "a").await;
    complete_index(&store).await;
    let mut page = commit(&store, &actor, DetailFacet::Comments).await;
    page.entries = vec![entry("saved")];
    store.apply_detail(page).await.unwrap();
    let mut connection = connect(&path).await;
    let mut tables = ORIGINAL_TABLES.to_vec();
    tables.extend(["cache_retention_entries", "cache_retention_state"]);
    let before = snapshot(&mut connection, &tables).await;
    sqlx::query("CREATE TRIGGER abort_retention_delete AFTER DELETE ON detail_observations BEGIN SELECT RAISE(ABORT,'retention delete fixture abort'); END")
        .execute(&mut connection).await.unwrap();
    assert_eq!(
        store
            .run_cache_maintenance(policy())
            .await
            .unwrap_err()
            .code,
        ErrorCode::Storage
    );
    assert_eq!(snapshot(&mut connection, &tables).await, before);
    sqlx::query("DROP TRIGGER abort_retention_delete")
        .execute(&mut connection)
        .await
        .unwrap();
    assert_eq!(
        store
            .run_cache_maintenance(policy())
            .await
            .unwrap()
            .evicted_facets,
        1
    );
    assert_accounted(&store, &mut connection).await;
}

#[tokio::test]
async fn committed_eviction_reports_a_post_commit_usage_refresh_failure() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("cache.sqlite");
    let store = Store::open(&path).await.unwrap();
    let actor = seed(&store, "a").await;
    complete_index(&store).await;
    store
        .apply_detail(commit(&store, &actor, DetailFacet::Body).await)
        .await
        .unwrap();
    let before_revision = store.revision().await.unwrap().parse::<i64>().unwrap();
    let mut connection = connect(&path).await;

    // Closing the read pool leaves the owned writer available and deterministically
    // fails only the fresh usage read after the maintenance transaction commits.
    store.close().await;
    let report = store
        .run_cache_maintenance(CacheRetentionPolicy {
            checkpoint_wal: false,
            ..policy()
        })
        .await
        .expect("A post-commit reporting failure must not hide durable eviction");

    assert_eq!(report.evicted_facets, 1);
    assert!(report.target_met);
    assert_eq!(report.usage_after.logical_bytes, Some(0));
    assert!(report.checkpoint.is_none());
    assert!(report.checkpoint_error.is_none());
    assert_eq!(
        report.usage_refresh_error.as_ref().map(|error| &error.code),
        Some(&ErrorCode::Storage)
    );
    assert_eq!(count(&mut connection, "detail_observations").await, 0);
    assert_eq!(count(&mut connection, "cache_retention_entries").await, 0);
    let (revision, coverage): (i64, String) = sqlx::query_as(
        "SELECT (SELECT revision FROM runtime_meta WHERE singleton=1),json_extract(coverage_json,'$.state') FROM sync_scopes WHERE account_id='a' AND scope='detail:pull:body'",
    )
    .fetch_one(&mut connection)
    .await
    .unwrap();
    assert!(revision > before_revision);
    assert_eq!(coverage, "missing");
    assert_integrity(&mut connection).await;
}

#[tokio::test]
async fn noop_is_observation_and_passive_preserves_a_held_reader_then_makes_progress() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("cache.sqlite");
    let store = Store::open(&path).await.unwrap();
    let mut writer = connect(&path).await;
    sqlx::query("PRAGMA wal_autocheckpoint=0")
        .execute(&mut writer)
        .await
        .unwrap();
    sqlx::query("CREATE TABLE retention_wal_fixture(id INTEGER PRIMARY KEY,payload BLOB NOT NULL)")
        .execute(&mut writer)
        .await
        .unwrap();
    sqlx::query("INSERT INTO retention_wal_fixture VALUES(1,zeroblob(1))")
        .execute(&mut writer)
        .await
        .unwrap();
    let frames_before: (i64, i64, i64) = sqlx::query_as("PRAGMA main.wal_checkpoint(NOOP)")
        .fetch_one(&mut writer)
        .await
        .unwrap();
    assert!(
        frames_before.1 > frames_before.2,
        "The observation control begins with checkpointable pending frames"
    );
    let untouched = store.wal_status().await.unwrap();
    assert!(untouched.supported);
    let frames_after: (i64, i64, i64) = sqlx::query_as("PRAGMA main.wal_checkpoint(NOOP)")
        .fetch_one(&mut writer)
        .await
        .unwrap();
    assert_eq!(
        frames_after, frames_before,
        "Observation must not disguise a PASSIVE mutation as NOOP"
    );
    assert_eq!(untouched.log_frames, Some(frames_before.1));
    assert_eq!(untouched.checkpointed_frames, Some(frames_before.2));
    let baseline = store.checkpoint_wal_passive().await.unwrap();
    assert!(!baseline.skipped_busy);
    let mut reader = connect(&path).await;
    let mut held = reader.begin().await.unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT length(payload) FROM retention_wal_fixture WHERE id=1"
        )
        .fetch_one(&mut *held)
        .await
        .unwrap(),
        1
    );
    sqlx::query("UPDATE retention_wal_fixture SET payload=zeroblob(262144) WHERE id=1")
        .execute(&mut writer)
        .await
        .unwrap();
    let observed = store.wal_status().await.unwrap();
    assert!(
        observed.supported,
        "Bundled SQLite supports NOOP without substituting a mutating checkpoint"
    );
    assert!(observed.wal_bytes > 0);
    let repeated = store.wal_status().await.unwrap();
    assert_eq!(repeated.log_frames, observed.log_frames);
    assert_eq!(repeated.checkpointed_frames, observed.checkpointed_frames);
    let partial = tokio::time::timeout(Duration::from_secs(5), store.checkpoint_wal_passive())
        .await
        .expect("PASSIVE does not wait for the held reader")
        .unwrap();
    assert!(!partial.skipped_busy);
    let log = partial.log_frames.expect("Actual available log frames");
    let checkpointed = partial
        .checkpointed_frames
        .expect("Actual available checkpoint progress");
    assert!(
        log > checkpointed,
        "Held snapshot leaves frames pending even if SQLite reports busy=0"
    );
    assert!(checkpointed >= 0);
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT length(payload) FROM retention_wal_fixture WHERE id=1"
        )
        .fetch_one(&mut *held)
        .await
        .unwrap(),
        1,
        "The old read snapshot remains usable after PASSIVE"
    );
    held.commit().await.unwrap();
    let complete = store.checkpoint_wal_passive().await.unwrap();
    assert!(!complete.skipped_busy);
    assert_eq!(complete.log_frames, complete.checkpointed_frames);
    assert!(complete.checkpointed_frames.unwrap() > checkpointed);
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT length(payload) FROM retention_wal_fixture WHERE id=1"
        )
        .fetch_one(&mut reader)
        .await
        .unwrap(),
        262144
    );
}
