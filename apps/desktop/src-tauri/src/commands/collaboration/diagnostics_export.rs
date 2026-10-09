use std::{future::Future, path::PathBuf};

use collaboration::{CollaborationError, SyncDiagnosticsExport, SyncDiagnosticsExportReceipt};

pub(super) async fn export(
    report: SyncDiagnosticsExport,
    choice: impl Future<Output = Result<Option<PathBuf>, CollaborationError>>,
) -> Result<SyncDiagnosticsExportReceipt, CollaborationError> {
    let Some(path) = choice.await? else {
        return Ok(SyncDiagnosticsExportReceipt { exported: false });
    };
    let body = serde_json::to_string_pretty(&report).map_err(|_| CollaborationError::storage())?;
    tokio::task::spawn_blocking(move || super::draft_export::write(&path, &body))
        .await
        .map_err(|_| CollaborationError::storage())??;
    Ok(SyncDiagnosticsExportReceipt { exported: true })
}

#[cfg(test)]
mod tests {
    use super::export;
    use collaboration::{
        CoverageDiagnostics, StorageDiagnostics, SyncDiagnosticsExport, SyncLatencyDiagnostics,
    };

    fn report() -> SyncDiagnosticsExport {
        SyncDiagnosticsExport {
            format_version: 1,
            generated_at: "2026-10-08T00:00:00.000Z".into(),
            account_count: 2,
            coverage: CoverageDiagnostics {
                complete_scopes: 3,
                partial_scopes: 1,
                missing_scopes: 0,
            },
            ready_jobs: 1,
            deferred_jobs: 0,
            oldest_job_age_seconds: Some(4),
            accounts_in_cooldown: 0,
            recovery_categories: vec![],
            latency: SyncLatencyDiagnostics {
                sample_count: 1,
                total_milliseconds: 2,
                maximum_milliseconds: Some(2),
                p50_upper_bound_milliseconds: Some(10),
                p95_upper_bound_milliseconds: Some(10),
                p99_upper_bound_milliseconds: Some(10),
            },
            storage: StorageDiagnostics {
                cache_usage_available: true,
                logical_bytes: Some(3),
                indexed_logical_bytes: Some(3),
                database_bytes: Some(4),
                wal_bytes: Some(0),
                wal_observation_supported: true,
                wal_busy: Some(false),
                wal_log_frames: Some(0),
                wal_checkpointed_frames: Some(0),
            },
        }
    }

    #[tokio::test]
    async fn cancellation_writes_nothing_and_success_is_private_json() {
        let dir = tempfile::tempdir().unwrap();
        assert!(!export(report(), async { Ok(None) }).await.unwrap().exported);
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
        let chosen = dir.path().join("diagnostics.json");
        assert!(
            export(report(), async { Ok(Some(chosen.clone())) })
                .await
                .unwrap()
                .exported
        );
        let value: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&chosen).unwrap()).unwrap();
        assert_eq!(value["format_version"], 1);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(chosen).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
    }
}
