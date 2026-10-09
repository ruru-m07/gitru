use super::{CANARY, KEY, Result, no_canary, open, readable};
use sqlx::{Connection, SqliteConnection};
use std::path::Path;

pub async fn child(path: &Path) -> Result<()> {
    let mut database = open(path, Some(KEY), true).await?;
    sqlx::raw_sql("PRAGMA journal_mode=WAL; PRAGMA synchronous=FULL; PRAGMA wal_autocheckpoint=0; CREATE TABLE spike_probe(body TEXT NOT NULL) STRICT;")
        .execute(&mut database).await?;
    sqlx::query("INSERT INTO spike_probe VALUES(?)")
        .bind(CANARY)
        .execute(&mut database)
        .await?;
    sqlx::raw_sql("BEGIN IMMEDIATE; INSERT INTO spike_probe VALUES('must roll back');")
        .execute(&mut database)
        .await?;
    // Abrupt process exit deliberately skips Rust/SQLx destructors. Synthetic DB only.
    std::process::exit(0);
}

pub async fn crash_reopen(scratch: &Path) -> Result<()> {
    let path = scratch.join("crashed.sqlite");
    let mut command = tokio::process::Command::new(std::env::current_exe()?);
    command.arg("--crash-child").arg(&path).kill_on_drop(true);
    let status =
        tokio::time::timeout(std::time::Duration::from_secs(60), command.status()).await??;
    assert!(status.success());
    assert!(path.with_file_name("crashed.sqlite-wal").exists());
    assert!(no_canary(&path)?);
    assert!(no_canary(&path.with_file_name("crashed.sqlite-wal"))?);
    let mut reopened = open(&path, Some(KEY), false).await?;
    let bodies: Vec<String> = sqlx::query_scalar("SELECT body FROM spike_probe")
        .fetch_all(&mut reopened)
        .await?;
    assert_eq!(bodies, [CANARY]);
    let integrity: String = sqlx::query_scalar("PRAGMA integrity_check")
        .fetch_one(&mut reopened)
        .await?;
    assert_eq!(integrity, "ok");
    reopened.close().await?;
    println!("abrupt_process_wal_reopen_committed_and_rollback=pass");
    Ok(())
}

pub async fn portability(directory: &Path) -> Result<()> {
    let mut producers = std::collections::BTreeSet::new();
    for entry in std::fs::read_dir(directory)? {
        let path = entry?.path();
        if path.extension().is_none_or(|ext| ext != "sqlite") {
            continue;
        }
        let mut database = open(&path, Some(KEY), false).await?;
        let bodies: Vec<String> = sqlx::query_scalar("SELECT body FROM spike_probe")
            .fetch_all(&mut database)
            .await?;
        assert_eq!(bodies, [CANARY]);
        let integrity: String = sqlx::query_scalar("PRAGMA integrity_check")
            .fetch_one(&mut database)
            .await?;
        assert_eq!(integrity, "ok");
        let fts: i64 = sqlx::query_scalar("SELECT count(*) FROM spike_fts WHERE spike_fts MATCH ?")
            .bind(CANARY)
            .fetch_one(&mut database)
            .await?;
        assert_eq!(fts, 1);
        self::integrity(&mut database).await?;
        database.close().await?;
        assert!(!readable(&path, None).await);
        assert!(no_canary(&path)?);
        producers.insert(path.file_name().unwrap().to_string_lossy().into_owned());
        println!(
            "portable_fixture={}; keyed_read_fts_integrity=pass",
            path.file_name().unwrap().to_string_lossy()
        );
    }
    assert_eq!(
        producers,
        ["linux.sqlite", "macos.sqlite", "windows.sqlite"]
            .map(str::to_owned)
            .into_iter()
            .collect(),
        "portability qualification requires all three producer platforms"
    );
    Ok(())
}

pub async fn tamper_and_export(scratch: &Path, backup: &Path, export: &Path) -> Result<()> {
    let tampered = scratch.join("tampered.sqlite");
    let mut bytes = std::fs::read(backup)?;
    assert!(bytes.len() >= 4096);
    bytes[2000] ^= 0x80;
    std::fs::write(&tampered, bytes)?;
    assert!(!readable(&tampered, Some(KEY)).await);
    assert!(readable(backup, Some(KEY)).await);
    std::fs::create_dir_all(export)?;
    std::fs::copy(
        backup,
        export.join(format!("{}.sqlite", std::env::consts::OS)),
    )?;
    println!("tampered_page_refusal=pass; encrypted_portability_fixture=exported");
    Ok(())
}

pub async fn integrity(connection: &mut SqliteConnection) -> Result<()> {
    let rows: Vec<String> = sqlx::query_scalar("PRAGMA cipher_integrity_check")
        .fetch_all(connection)
        .await?;
    assert!(rows.is_empty(), "cipher integrity check failed");
    Ok(())
}
