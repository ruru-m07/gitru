//! Isolated compatibility experiment; fixed synthetic data/keys, never app data.
use sqlx::{
    Connection, SqliteConnection,
    sqlite::{SqliteConnectOptions, SqlitePoolOptions},
};
use std::{
    error::Error,
    path::{Path, PathBuf},
};
type Result<T> = std::result::Result<T, Box<dyn Error>>;
const KEY: &str = "gitru-synthetic-spike-key-not-a-real-credential";
const CANARY: &str = "GITRU_PRIVATE_SYNTHETIC_CANARY_6b3d1f20";
fn options(path: &Path, key: Option<&str>, create: bool) -> SqliteConnectOptions {
    let mut options = SqliteConnectOptions::new()
        .filename(path)
        .create_if_missing(create)
        .foreign_keys(true)
        .pragma("temp_store", "MEMORY");
    if let Some(key) = key {
        // SQLx accepts a PRAGMA expression, so quote this synthetic literal.
        options = options.pragma("key", format!("'{}'", key.replace('\'', "''")));
    }
    options
}
async fn open(path: &Path, key: Option<&str>, create: bool) -> Result<SqliteConnection> {
    Ok(SqliteConnection::connect_with(&options(path, key, create)).await?)
}
async fn readable(path: &Path, key: Option<&str>) -> bool {
    match open(path, key, false).await {
        Ok(mut connection) => {
            let ok = sqlx::query_scalar::<_, i64>("SELECT count(*) FROM spike_probe")
                .fetch_one(&mut connection)
                .await
                .is_ok();
            let _ = connection.close().await;
            ok
        }
        Err(_) => false,
    }
}
fn no_canary(path: &Path) -> Result<bool> {
    let bytes = std::fs::read(path)?;
    Ok(!bytes
        .windows(CANARY.len())
        .any(|window| window == CANARY.as_bytes()))
}
#[tokio::main]
async fn main() -> Result<()> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let scratch = root.join("target");
    std::fs::create_dir_all(&scratch)?;
    let temporary = tempfile::tempdir_in(&scratch)?;
    let db = temporary.path().join("synthetic.sqlite");
    let mut connection = open(&db, Some(KEY), true).await?;
    let sqlite: String = sqlx::query_scalar("SELECT sqlite_version()")
        .fetch_one(&mut connection)
        .await?;
    let cipher: String = sqlx::query_scalar("PRAGMA cipher_version")
        .fetch_one(&mut connection)
        .await?;
    let provider: String = sqlx::query_scalar("PRAGMA cipher_provider")
        .fetch_one(&mut connection)
        .await?;
    let fts: i64 = sqlx::query_scalar("SELECT sqlite_compileoption_used('ENABLE_FTS5')")
        .fetch_one(&mut connection)
        .await?;
    assert!(!cipher.is_empty());
    assert_eq!(fts, 1);
    println!("sqlite={sqlite}; cipher={cipher}; crypto_provider={provider}; fts5={fts}");
    // Production storage.rs requires the upstream WAL-reset fix. Never relax it.
    let parts: Vec<u32> = sqlite
        .split('.')
        .map(str::parse)
        .collect::<std::result::Result<_, _>>()?;
    let gate = parts.len() == 3
        && ((parts[0], parts[1], parts[2]) >= (3, 51, 3)
            || parts[..2] == [3, 50] && parts[2] >= 7
            || parts[..2] == [3, 44] && parts[2] >= 6);
    println!("production_wal_version_gate={gate}");
    let migrations = root.join("../../../crates/collaboration/migrations");
    sqlx::migrate::Migrator::new(migrations.as_path())
        .await?
        .run(&mut connection)
        .await?;
    let version: i64 = sqlx::query_scalar("SELECT max(version) FROM _sqlx_migrations")
        .fetch_one(&mut connection)
        .await?;
    sqlx::raw_sql("CREATE TABLE spike_probe(body TEXT NOT NULL) STRICT; CREATE VIRTUAL TABLE spike_fts USING fts5(body); PRAGMA journal_mode=WAL; PRAGMA wal_autocheckpoint=0;")
        .execute(&mut connection).await?;
    sqlx::query("INSERT INTO spike_probe VALUES(?)")
        .bind(CANARY)
        .execute(&mut connection)
        .await?;
    sqlx::query("INSERT INTO spike_fts VALUES(?)")
        .bind(CANARY)
        .execute(&mut connection)
        .await?;
    let matches: i64 = sqlx::query_scalar("SELECT count(*) FROM spike_fts WHERE spike_fts MATCH ?")
        .bind(CANARY)
        .fetch_one(&mut connection)
        .await?;
    assert_eq!(matches, 1);
    let readers = SqlitePoolOptions::new()
        .max_connections(2)
        .connect_with(
            options(&db, Some(KEY), false)
                .read_only(true)
                .pragma("query_only", "ON"),
        )
        .await?;
    let mut first = readers.acquire().await?;
    let mut second = readers.acquire().await?;
    for reader in [&mut first, &mut second] {
        let count: i64 = sqlx::query_scalar("SELECT count(*) FROM spike_probe")
            .fetch_one(&mut **reader)
            .await?;
        assert_eq!(count, 1);
        assert!(
            sqlx::query("DELETE FROM spike_probe")
                .execute(&mut **reader)
                .await
                .is_err()
        );
    }
    drop(first);
    drop(second);
    readers.close().await;
    println!("fts_query=pass; two_keyed_read_only_pool_connections=pass");
    let mut transaction = connection.begin().await?;
    sqlx::query("INSERT INTO spike_probe VALUES('rolled back')")
        .execute(&mut *transaction)
        .await?;
    transaction.rollback().await?;
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM spike_probe")
        .fetch_one(&mut connection)
        .await?;
    assert_eq!(count, 1);
    assert!(no_canary(&db)?);
    let wal = db.with_file_name("synthetic.sqlite-wal");
    assert!(wal.exists());
    assert!(no_canary(&wal)?);
    println!("migrations_through={version}; rollback=pass; db_and_wal_canary_absent=pass");
    let backup = temporary.path().join("backup.sqlite");
    sqlx::query("VACUUM INTO ?")
        .bind(backup.to_str().ok_or("non-UTF8 synthetic path")?)
        .execute(&mut connection)
        .await?;
    let backup_keyed = readable(&backup, Some(KEY)).await;
    let backup_unkeyed = readable(&backup, None).await;
    println!(
        "vacuum_backup_keyed_read={backup_keyed}; vacuum_backup_unkeyed_read={backup_unkeyed}; backup_canary_absent={}",
        no_canary(&backup)?
    );
    assert!(backup_keyed && !backup_unkeyed);
    assert!(no_canary(&backup)?);
    connection.close().await?;
    assert!(readable(&db, Some(KEY)).await);
    assert!(!readable(&db, Some("wrong-synthetic-key")).await);
    assert!(!readable(&db, None).await);
    let mut restored = open(&backup, Some(KEY), false).await?;
    let restored_body: String = sqlx::query_scalar("SELECT body FROM spike_probe")
        .fetch_one(&mut restored)
        .await?;
    assert_eq!(restored_body, CANARY);
    let integrity: String = sqlx::query_scalar("PRAGMA integrity_check")
        .fetch_one(&mut restored)
        .await?;
    assert_eq!(integrity, "ok");
    restored.close().await?;
    println!(
        "keyed_reopen=pass; wrong_and_missing_key_refusal=pass; synthetic_backup_restore=pass"
    );
    if !gate {
        println!(
            "PROMOTION_BLOCKED: pinned SQLCipher SQLite is below the existing production WAL fix gate"
        );
    }
    Ok(())
}
