//! Schema 18 expands rebuildable review facets while preserving historical
//! cache bytes, retention accounting, and authored command effects.
use std::path::Path;

use sha2::{Digest, Sha256};
use sqlx::{Connection, SqliteConnection, migrate::Migrator, sqlite::SqliteConnectOptions};

static CURRENT: Migrator = sqlx::migrate!("./migrations");

async fn connect(path: &Path) -> SqliteConnection {
    SqliteConnection::connect_with(
        &SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(true)
            .foreign_keys(true),
    )
    .await
    .unwrap()
}

#[tokio::test]
async fn review_facet_rebuild_preserves_historical_rows_retention_and_authored_effects() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("collaboration.sqlite");
    let mut db = connect(&path).await;
    let historical = Migrator::with_migrations(
        CURRENT
            .iter()
            .filter(|migration| migration.version < 18)
            .cloned()
            .collect(),
    );
    historical.run(&mut db).await.unwrap();
    sqlx::raw_sql(
        r#"
        INSERT INTO accounts VALUES('a','github','github.com','actor',1,'active','{}');
        INSERT INTO detail_observations VALUES('a','pull','reviews','1','7','{"state":"not_loaded","text":null}','{"source":"legacy"}',NULL,'known',NULL);
        INSERT INTO detail_entries VALUES('a','pull','reviews','legacy-row','{"opaque":"historical"}','run-1');
        INSERT INTO detail_demand VALUES('a','pull','reviews','1',1);
        INSERT INTO cache_retention_entries VALUES('a','pull','reviews',73,9);
        INSERT INTO commands(account_id,command_id,authorization_epoch,envelope_version,operation_kind,payload_version,target_kind,target_id,repository_id,canonical_envelope,payload_bytes,guard_bytes,submission_hash,enqueue_order,admitted_revision,admitted_at,state)
        VALUES('a','123e4567-e89b-12d3-a456-426614174000',1,1,'pull.edit',1,'pull_request','pull',NULL,x'01',x'',x'',zeroblob(32),1,1,'2026-10-08T00:00:00Z','queued');
        INSERT INTO command_effects VALUES('a','123e4567-e89b-12d3-a456-426614174000',zeroblob(32),1,'{"title":"Preserved title"}');
        "#,
    )
    .execute(&mut db)
    .await
    .unwrap();
    let before: Vec<String> = sqlx::query_scalar(
        "SELECT json_array(account_id,subject_id,facet,authorization_epoch,facet_revision,body_json,source_json,value_source_json,observed_state,stale_at) FROM detail_observations UNION ALL SELECT json_array(account_id,subject_id,facet,id,json,last_seen_run,NULL,NULL,NULL,NULL) FROM detail_entries ORDER BY 1",
    )
    .fetch_all(&mut db)
    .await
    .unwrap();
    let effect_before: String = sqlx::query_scalar("SELECT patch_json FROM command_effects")
        .fetch_one(&mut db)
        .await
        .unwrap();
    let effect_hash = Sha256::digest(effect_before.as_bytes());

    CURRENT.run(&mut db).await.unwrap();
    let after: Vec<String> = sqlx::query_scalar(
        "SELECT json_array(account_id,subject_id,facet,authorization_epoch,facet_revision,body_json,source_json,value_source_json,observed_state,stale_at) FROM detail_observations UNION ALL SELECT json_array(account_id,subject_id,facet,id,json,last_seen_run,NULL,NULL,NULL,NULL) FROM detail_entries ORDER BY 1",
    )
    .fetch_all(&mut db)
    .await
    .unwrap();
    assert_eq!(after, before);
    let effect_after: String = sqlx::query_scalar("SELECT patch_json FROM command_effects")
        .fetch_one(&mut db)
        .await
        .unwrap();
    assert_eq!(Sha256::digest(effect_after.as_bytes()), effect_hash);
    assert_eq!(
        sqlx::query_as::<_, (i64, i64)>(
            "SELECT indexed_logical_bytes,indexed_facet_count FROM cache_retention_state WHERE singleton=1",
        )
        .fetch_one(&mut db)
        .await
        .unwrap(),
        (73, 1)
    );
    sqlx::raw_sql(
        "INSERT INTO detail_observations VALUES('a','pull','review_summaries','1','8','{}','{}',NULL,'known',NULL); INSERT INTO detail_observations VALUES('a','pull','review_threads','1','9','{}','{}',NULL,'known',NULL); INSERT INTO detail_demand VALUES('a','pull','review_summaries','1',1); INSERT INTO detail_demand VALUES('a','pull','review_threads','1',1);",
    )
    .execute(&mut db)
    .await
    .unwrap();
    assert!(
        sqlx::query("INSERT INTO detail_observations VALUES('a','pull','future_reviews','1','10','{}','{}',NULL,'known',NULL)")
            .execute(&mut db)
            .await
            .is_err()
    );
    assert_eq!(
        sqlx::query_scalar::<_, String>("PRAGMA integrity_check")
            .fetch_one(&mut db)
            .await
            .unwrap(),
        "ok"
    );
    assert!(
        sqlx::query("PRAGMA foreign_key_check")
            .fetch_all(&mut db)
            .await
            .unwrap()
            .is_empty()
    );
}
