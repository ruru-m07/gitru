use super::*;
use crate::diagnostics::{CoverageDiagnostics, SyncRecoveryCategory};

const MAX_DIAGNOSTIC_ACCOUNTS: usize = 100;
const MAX_RECOVERY_GROUPS: usize = 700;

#[derive(Debug)]
pub(crate) struct SavedAccountDiagnostics {
    pub account: RemoteAccount,
    pub coverage: CoverageDiagnostics,
    pub recovery: Option<SavedRecoveryDiagnostics>,
}

#[derive(Debug, Clone)]
pub(crate) struct SavedRecoveryDiagnostics {
    pub category: SyncRecoveryCategory,
    pub affected_scopes: u32,
    pub next_retry_at: Option<String>,
}

impl Store {
    pub(crate) async fn saved_diagnostics(&self) -> Result<(String, Vec<SavedAccountDiagnostics>)> {
        let mut tx = self.inner.readers.begin().await.map_err(storage_error)?;
        let revision = metadata(&mut tx).await?.0;
        let coverage_rows = sqlx::query(
            "SELECT a.json,\
             COALESCE(SUM(CASE WHEN json_extract(s.coverage_json,'$.state')='complete' THEN 1 ELSE 0 END),0) complete_scopes,\
             COALESCE(SUM(CASE WHEN json_extract(s.coverage_json,'$.state')='partial' THEN 1 ELSE 0 END),0) partial_scopes,\
             COALESCE(SUM(CASE WHEN json_extract(s.coverage_json,'$.state')='missing' THEN 1 ELSE 0 END),0) missing_scopes \
             FROM accounts a LEFT JOIN sync_scopes s ON s.account_id=a.id \
             GROUP BY a.id ORDER BY a.id LIMIT 101",
        )
        .fetch_all(&mut *tx)
        .await
        .map_err(storage_error)?;
        if coverage_rows.len() > MAX_DIAGNOSTIC_ACCOUNTS {
            return Err(CollaborationError::new(
                ErrorCode::Storage,
                "Too many connected accounts for sync diagnostics",
            ));
        }
        let recovery_rows = sqlx::query(
            "SELECT account_id,\
             json_extract(sync_json,'$.state') sync_state,\
             json_extract(sync_json,'$.error.code') error_code,\
             COUNT(*) affected_scopes,\
             MAX(json_extract(sync_json,'$.next_retry_at')) next_retry_at \
             FROM sync_scopes \
             WHERE json_extract(sync_json,'$.error.code') IS NOT NULL \
                OR json_extract(sync_json,'$.state') IN ('offline','rate_limited','auth_required','error') \
             GROUP BY account_id,sync_state,error_code \
             ORDER BY account_id,sync_state,error_code LIMIT 701",
        )
        .fetch_all(&mut *tx)
        .await
        .map_err(storage_error)?;
        if recovery_rows.len() > MAX_RECOVERY_GROUPS {
            return Err(CollaborationError::new(
                ErrorCode::Storage,
                "Too many saved retry states for sync diagnostics",
            ));
        }

        let mut recoveries: std::collections::HashMap<String, SavedRecoveryDiagnostics> =
            std::collections::HashMap::new();
        for row in recovery_rows {
            let account_id: String = row.get("account_id");
            let state: Option<String> = row.get("sync_state");
            let code: Option<String> = row.get("error_code");
            let category = recovery_category(code.as_deref(), state.as_deref());
            let affected_scopes = bounded_count(row.get::<i64, _>("affected_scopes"))?;
            let next_retry_at: Option<String> = row.get("next_retry_at");
            recoveries
                .entry(account_id)
                .and_modify(|saved| {
                    if category < saved.category {
                        saved.category = category;
                        saved.affected_scopes = affected_scopes;
                        saved.next_retry_at = next_retry_at.clone();
                    } else if category == saved.category {
                        saved.affected_scopes =
                            saved.affected_scopes.saturating_add(affected_scopes);
                        saved.next_retry_at = saved
                            .next_retry_at
                            .iter()
                            .chain(next_retry_at.iter())
                            .max()
                            .cloned();
                    }
                })
                .or_insert(SavedRecoveryDiagnostics {
                    category,
                    affected_scopes,
                    next_retry_at,
                });
        }

        let mut accounts = Vec::with_capacity(coverage_rows.len());
        for row in coverage_rows {
            let account: RemoteAccount = decode(row.get::<&str, _>("json"))?;
            let recovery = recoveries.remove(&account.id);
            accounts.push(SavedAccountDiagnostics {
                account,
                coverage: CoverageDiagnostics {
                    complete_scopes: bounded_count(row.get("complete_scopes"))?,
                    partial_scopes: bounded_count(row.get("partial_scopes"))?,
                    missing_scopes: bounded_count(row.get("missing_scopes"))?,
                },
                recovery,
            });
        }
        tx.commit().await.map_err(storage_error)?;
        Ok((revision, accounts))
    }
}

fn bounded_count(value: i64) -> Result<u32> {
    u32::try_from(value).map_err(|_| {
        CollaborationError::new(ErrorCode::Storage, "Sync diagnostic count is unavailable")
    })
}

fn recovery_category(code: Option<&str>, state: Option<&str>) -> SyncRecoveryCategory {
    match code {
        Some("auth_required") => SyncRecoveryCategory::Authentication,
        Some("permission_denied") => SyncRecoveryCategory::Permission,
        Some("rate_limited") => SyncRecoveryCategory::RateLimit,
        Some("network") => SyncRecoveryCategory::Offline,
        Some("provider" | "not_ready" | "busy" | "credential_store_unavailable") => {
            SyncRecoveryCategory::Unavailable
        }
        Some(_) => SyncRecoveryCategory::Permanent,
        None => match state {
            Some("auth_required") => SyncRecoveryCategory::Authentication,
            Some("rate_limited") => SyncRecoveryCategory::RateLimit,
            Some("offline") => SyncRecoveryCategory::Offline,
            Some("error") => SyncRecoveryCategory::Unavailable,
            _ => SyncRecoveryCategory::Permanent,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_codes_map_to_bounded_recovery_categories() {
        assert_eq!(
            recovery_category(Some("auth_required"), None),
            SyncRecoveryCategory::Authentication
        );
        assert_eq!(
            recovery_category(Some("permission_denied"), None),
            SyncRecoveryCategory::Permission
        );
        assert_eq!(
            recovery_category(Some("rate_limited"), None),
            SyncRecoveryCategory::RateLimit
        );
        assert_eq!(
            recovery_category(Some("network"), None),
            SyncRecoveryCategory::Offline
        );
        assert_eq!(
            recovery_category(Some("provider"), None),
            SyncRecoveryCategory::Unavailable
        );
        assert_eq!(
            recovery_category(Some("invalid_input"), None),
            SyncRecoveryCategory::Permanent
        );
    }

    #[tokio::test]
    async fn saved_diagnostics_aggregate_scope_state_without_identity_text() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open(dir.path().join("diagnostics.db"))
            .await
            .unwrap();
        store
            .upsert_account(RemoteAccount {
                id: "account-secret-canary".into(),
                provider: ProviderKind::Gitlab,
                host: "provider-url-canary.invalid".into(),
                actor_id: "username-canary".into(),
                login: "login-canary".into(),
                display_name: Some("remote-text-canary".into()),
                authorization_epoch: "1".into(),
                state: AccountState::Active,
                notifications_supported: false,
            })
            .await
            .unwrap();
        store
            .set_sync_status(
                "account-secret-canary",
                "1",
                "repositories",
                SyncStatus {
                    state: SyncState::Error,
                    last_success_at: None,
                    next_retry_at: None,
                    error: Some(CollaborationError::new(
                        ErrorCode::PermissionDenied,
                        "token-url-repository-title-canary",
                    )),
                },
            )
            .await
            .unwrap();
        let (_, accounts) = store.saved_diagnostics().await.unwrap();
        assert_eq!(accounts.len(), 1);
        assert_eq!(accounts[0].account.provider, ProviderKind::Gitlab);
        assert_eq!(accounts[0].coverage.missing_scopes, 1);
        let recovery = accounts[0].recovery.as_ref().unwrap();
        assert_eq!(recovery.category, SyncRecoveryCategory::Permission);
        assert_eq!(recovery.affected_scopes, 1);
        let safe = serde_json::to_string(&CoverageDiagnostics {
            complete_scopes: accounts[0].coverage.complete_scopes,
            partial_scopes: accounts[0].coverage.partial_scopes,
            missing_scopes: accounts[0].coverage.missing_scopes,
        })
        .unwrap();
        for forbidden in ["secret", "username", "provider-url", "remote-text", "token"] {
            assert!(!safe.contains(forbidden));
        }
    }
}
