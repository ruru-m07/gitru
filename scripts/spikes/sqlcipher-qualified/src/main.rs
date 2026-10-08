//! Source-pinned qualification; fixed synthetic data/keys, never app data.
mod lifecycle;
use sqlx::{
    Connection, SqliteConnection,
    sqlite::{SqliteConnectOptions, SqlitePoolOptions},
};
use std::{
    error::Error,
    path::{Path, PathBuf},
};
type Result<T> = std::result::Result<T, Box<dyn Error>>;
const KEY: &str = "gitru-synthetic-qualification-key-not-a-real-credential";
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
    let arguments: Vec<_> = std::env::args_os().skip(1).collect();
    match arguments.as_slice() {
        [mode, path] if mode == "--crash-child" => return lifecycle::child(Path::new(path)).await,
        [mode, path] if mode == "--portability-dir" => {
            return lifecycle::portability(Path::new(path)).await;
        }
        [] => {}
        _ => return Err("Unknown qualification arguments".into()),
    }
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
    let provider_version: String = sqlx::query_scalar("PRAGMA cipher_provider_version")
        .fetch_one(&mut connection)
        .await?;
    if !cfg!(target_vendor = "apple") {
        assert!(
            provider_version.starts_with("OpenSSL 3.6.5 "),
            "unqualified crypto provider: {provider_version}"
        );
    }
    let fts: i64 = sqlx::query_scalar("SELECT sqlite_compileoption_used('ENABLE_FTS5')")
        .fetch_one(&mut connection)
        .await?;
    assert_eq!(sqlite, "3.53.4", "unexpected native SQLite linkage");
    assert!(cipher.starts_with("4.19.0 "), "unexpected cipher {cipher}");
    assert_eq!(
        provider,
        if cfg!(target_vendor = "apple") {
            "commoncrypto"
        } else {
            "openssl"
        }
    );
    assert_eq!(fts, 1);
    println!("sqlite={sqlite}; cipher={cipher}; crypto_provider={provider}; fts5={fts}");
    println!("crypto_provider_version={provider_version}");
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
    assert!(
        gate,
        "candidate fails the unchanged production WAL safety gate"
    );
    for (pragma, expected) in [
        ("PRAGMA cipher_use_hmac", "1"),
        ("PRAGMA cipher_plaintext_header_size", "0"),
        ("PRAGMA cipher_page_size", "4096"),
        ("PRAGMA kdf_iter", "256000"),
        ("PRAGMA cipher_hmac_algorithm", "HMAC_SHA512"),
        ("PRAGMA cipher_kdf_algorithm", "PBKDF2_HMAC_SHA512"),
    ] {
        let observed: String = sqlx::query_scalar(pragma)
            .fetch_one(&mut connection)
            .await?;
        assert_eq!(
            observed, expected,
            "unexpected cipher settings for {pragma}"
        );
    }
    let source: String = sqlx::query_scalar("SELECT sqlite_source_id()")
        .fetch_one(&mut connection)
        .await?;
    println!("sqlite_source_id={source}");
    let migrations = root.join("target/collaboration-source/crates/collaboration/migrations");
    sqlx::migrate::Migrator::new(migrations.as_path())
        .await?
        .run(&mut connection)
        .await?;
    let version: i64 = sqlx::query_scalar("SELECT max(version) FROM _sqlx_migrations")
        .fetch_one(&mut connection)
        .await?;
    assert_eq!(
        version, 22,
        "pinned current-store migration coverage changed"
    );
    sqlx::raw_sql("CREATE TABLE spike_probe(body TEXT NOT NULL) STRICT; CREATE VIRTUAL TABLE spike_fts USING fts5(body); PRAGMA journal_mode=WAL; PRAGMA synchronous=FULL; PRAGMA wal_autocheckpoint=0;")
        .execute(&mut connection).await?;
    let journal: String = sqlx::query_scalar("PRAGMA journal_mode")
        .fetch_one(&mut connection)
        .await?;
    let synchronous: i64 = sqlx::query_scalar("PRAGMA synchronous")
        .fetch_one(&mut connection)
        .await?;
    assert_eq!(journal, "wal");
    assert_eq!(synchronous, 2);
    let json: i64 = sqlx::query_scalar("SELECT json_extract('{\"value\":42}', '$.value')")
        .fetch_one(&mut connection)
        .await?;
    assert_eq!(json, 42);
    assert!(
        sqlx::query("INSERT INTO spike_probe VALUES(x'00')")
            .execute(&mut connection)
            .await
            .is_err()
    );
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
        .max_connections(3)
        .connect_with(
            options(&db, Some(KEY), false)
                .read_only(true)
                .pragma("query_only", "ON"),
        )
        .await?;
    let mut first = readers.acquire().await?;
    let mut second = readers.acquire().await?;
    let mut third = readers.acquire().await?;
    for reader in [&mut first, &mut second, &mut third] {
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
    drop(third);
    readers.close().await;
    println!("fts_json_strict=pass; three_keyed_read_only_pool_connections=pass");
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
    wal_checkpoint(&db, &mut connection).await?;
    native_backup(&db, temporary.path(), &mut connection).await?;
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
    let mut verified = open(&backup, Some(KEY), false).await?;
    lifecycle::integrity(&mut verified).await?;
    verified.close().await?;
    lifecycle::crash_reopen(temporary.path()).await?;
    lifecycle::tamper_and_export(temporary.path(), &backup, &scratch.join("portability")).await?;
    println!("NATIVE_PROBE_PASS; whole_app_encryption_vault_portable_key_wrap=not_qualified");
    Ok(())
}

async fn wal_checkpoint(db: &Path, writer: &mut SqliteConnection) -> Result<()> {
    let mut reader = open(db, Some(KEY), false).await?;
    sqlx::query("BEGIN").execute(&mut reader).await?;
    let _: i64 = sqlx::query_scalar("SELECT count(*) FROM spike_probe")
        .fetch_one(&mut reader)
        .await?;
    sqlx::query("INSERT INTO spike_probe VALUES('checkpoint fixture')")
        .execute(&mut *writer)
        .await?;
    let (busy, log, copied): (i64, i64, i64) = sqlx::query_as("PRAGMA wal_checkpoint(PASSIVE)")
        .fetch_one(&mut *writer)
        .await?;
    assert_eq!(busy, 0);
    assert!(
        log > copied,
        "held snapshot must prevent complete checkpoint"
    );
    sqlx::query("ROLLBACK").execute(&mut reader).await?;
    reader.close().await?;
    let (busy, log, copied): (i64, i64, i64) = sqlx::query_as("PRAGMA wal_checkpoint(TRUNCATE)")
        .fetch_one(&mut *writer)
        .await?;
    assert_eq!((busy, log, copied), (0, 0, 0));
    sqlx::query("DELETE FROM spike_probe WHERE body='checkpoint fixture'")
        .execute(&mut *writer)
        .await?;
    println!("independent_reader_checkpoint_and_wal_restart=pass");
    Ok(())
}

async fn native_backup(db: &Path, scratch: &Path, source: &mut SqliteConnection) -> Result<()> {
    let path = scratch.join("native-backup.sqlite");
    let mut destination = open(&path, Some(KEY), true).await?;
    let mut source_guard = source.lock_handle().await?;
    let mut destination_guard = destination.lock_handle().await?;
    // SAFETY: both SQLx worker handles are exclusively locked. Neither connection
    // is used during backup; every initialized backup is finished before unlock.
    let (step, finish) = unsafe {
        let backup = libsqlite3_sys::sqlite3_backup_init(
            destination_guard.as_raw_handle().as_ptr(),
            c"main".as_ptr(),
            source_guard.as_raw_handle().as_ptr(),
            c"main".as_ptr(),
        );
        assert!(
            !backup.is_null(),
            "native backup initialization refused keyed databases"
        );
        let step = libsqlite3_sys::sqlite3_backup_step(backup, -1);
        let finish = libsqlite3_sys::sqlite3_backup_finish(backup);
        (step, finish)
    };
    drop(destination_guard);
    drop(source_guard);
    assert_eq!(step, libsqlite3_sys::SQLITE_DONE);
    assert_eq!(finish, libsqlite3_sys::SQLITE_OK);
    destination.close().await?;
    assert!(readable(&path, Some(KEY)).await);
    assert!(!readable(&path, None).await);
    assert!(no_canary(&path)?);
    assert!(readable(db, Some(KEY)).await);
    println!("native_keyed_backup_api=pass");
    Ok(())
}
