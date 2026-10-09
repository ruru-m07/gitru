use crate::domain::ProviderKind;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SyncRecoveryCategory {
    Authentication,
    Permission,
    RateLimit,
    Offline,
    Unavailable,
    Permanent,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SyncRecoveryState {
    pub category: SyncRecoveryCategory,
    pub affected_scopes: u32,
    pub next_retry_at: Option<String>,
    pub retry_after_seconds: Option<u64>,
    pub explicit_retry_eligible: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoverageDiagnostics {
    pub complete_scopes: u32,
    pub partial_scopes: u32,
    pub missing_scopes: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccountSyncDiagnostics {
    /// Contextual UI join key. It is deliberately excluded from export models.
    pub account_id: String,
    pub provider: ProviderKind,
    pub coverage: CoverageDiagnostics,
    pub ready_jobs: u32,
    pub deferred_jobs: u32,
    pub oldest_job_age_seconds: Option<u64>,
    pub cooldown_remaining_seconds: Option<u64>,
    pub recovery: Option<SyncRecoveryState>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SyncLatencyDiagnostics {
    pub sample_count: u64,
    pub total_milliseconds: u64,
    pub maximum_milliseconds: Option<u64>,
    pub p50_upper_bound_milliseconds: Option<u64>,
    pub p95_upper_bound_milliseconds: Option<u64>,
    pub p99_upper_bound_milliseconds: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StorageDiagnostics {
    pub cache_usage_available: bool,
    pub logical_bytes: Option<u64>,
    pub indexed_logical_bytes: Option<u64>,
    pub database_bytes: Option<u64>,
    pub wal_bytes: Option<u64>,
    pub wal_observation_supported: bool,
    pub wal_busy: Option<bool>,
    pub wal_log_frames: Option<u64>,
    pub wal_checkpointed_frames: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SyncDiagnosticsSnapshot {
    pub generated_at: String,
    pub revision: String,
    pub accounts: Vec<AccountSyncDiagnostics>,
    pub ready_jobs: u32,
    pub deferred_jobs: u32,
    pub oldest_job_age_seconds: Option<u64>,
    pub accounts_in_cooldown: u32,
    pub latency: SyncLatencyDiagnostics,
    pub storage: StorageDiagnostics,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecoveryCategoryCount {
    pub category: SyncRecoveryCategory,
    pub account_count: u32,
    pub affected_scopes: u32,
}

/// Aggregate-only support payload. Its schema intentionally cannot carry
/// account/repository identities, provider hosts, remote text or raw errors.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SyncDiagnosticsExport {
    pub format_version: u32,
    pub generated_at: String,
    pub account_count: u32,
    pub coverage: CoverageDiagnostics,
    pub ready_jobs: u32,
    pub deferred_jobs: u32,
    pub oldest_job_age_seconds: Option<u64>,
    pub accounts_in_cooldown: u32,
    pub recovery_categories: Vec<RecoveryCategoryCount>,
    pub latency: SyncLatencyDiagnostics,
    pub storage: StorageDiagnostics,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SyncDiagnosticsExportReceipt {
    pub exported: bool,
}

impl SyncDiagnosticsSnapshot {
    pub fn export(&self) -> SyncDiagnosticsExport {
        let mut recovery = std::collections::BTreeMap::<SyncRecoveryCategory, (u32, u32)>::new();
        let mut coverage = CoverageDiagnostics {
            complete_scopes: 0,
            partial_scopes: 0,
            missing_scopes: 0,
        };
        for account in &self.accounts {
            coverage.complete_scopes = coverage
                .complete_scopes
                .saturating_add(account.coverage.complete_scopes);
            coverage.partial_scopes = coverage
                .partial_scopes
                .saturating_add(account.coverage.partial_scopes);
            coverage.missing_scopes = coverage
                .missing_scopes
                .saturating_add(account.coverage.missing_scopes);
            if let Some(state) = &account.recovery {
                let value = recovery.entry(state.category).or_default();
                value.0 = value.0.saturating_add(1);
                value.1 = value.1.saturating_add(state.affected_scopes);
            }
        }
        SyncDiagnosticsExport {
            format_version: 1,
            generated_at: self.generated_at.clone(),
            account_count: u32::try_from(self.accounts.len()).unwrap_or(u32::MAX),
            coverage,
            ready_jobs: self.ready_jobs,
            deferred_jobs: self.deferred_jobs,
            oldest_job_age_seconds: self.oldest_job_age_seconds,
            accounts_in_cooldown: self.accounts_in_cooldown,
            recovery_categories: recovery
                .into_iter()
                .map(
                    |(category, (account_count, affected_scopes))| RecoveryCategoryCount {
                        category,
                        account_count,
                        affected_scopes,
                    },
                )
                .collect(),
            latency: self.latency.clone(),
            storage: self.storage.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aggregate_export_has_no_contextual_identity_or_remote_text_field() {
        let snapshot = SyncDiagnosticsSnapshot {
            generated_at: "2026-10-08T00:00:00.000Z".into(),
            revision: "9".into(),
            accounts: vec![AccountSyncDiagnostics {
                account_id: "account-canary-token-user-repository-url-title".into(),
                provider: ProviderKind::Github,
                coverage: CoverageDiagnostics {
                    complete_scopes: 2,
                    partial_scopes: 1,
                    missing_scopes: 0,
                },
                ready_jobs: 1,
                deferred_jobs: 0,
                oldest_job_age_seconds: Some(2),
                cooldown_remaining_seconds: None,
                recovery: Some(SyncRecoveryState {
                    category: SyncRecoveryCategory::Permission,
                    affected_scopes: 1,
                    next_retry_at: None,
                    retry_after_seconds: None,
                    explicit_retry_eligible: false,
                }),
            }],
            ready_jobs: 1,
            deferred_jobs: 0,
            oldest_job_age_seconds: Some(2),
            accounts_in_cooldown: 0,
            latency: SyncLatencyDiagnostics {
                sample_count: 1,
                total_milliseconds: 4,
                maximum_milliseconds: Some(4),
                p50_upper_bound_milliseconds: Some(10),
                p95_upper_bound_milliseconds: Some(10),
                p99_upper_bound_milliseconds: Some(10),
            },
            storage: StorageDiagnostics {
                cache_usage_available: true,
                logical_bytes: Some(1),
                indexed_logical_bytes: Some(1),
                database_bytes: Some(1),
                wal_bytes: Some(0),
                wal_observation_supported: true,
                wal_busy: Some(false),
                wal_log_frames: Some(0),
                wal_checkpointed_frames: Some(0),
            },
        };
        let json = serde_json::to_string(&snapshot.export()).unwrap();
        for forbidden in [
            "account-canary",
            "token",
            "user",
            "repository",
            "url",
            "title",
            "github",
            "account_id",
            "provider",
            "next_retry_at",
        ] {
            assert!(!json.contains(forbidden), "leaked {forbidden}: {json}");
        }
    }
}
