//! Current inbox provenance is derived in the same transaction as every read/write.
use super::*;
use crate::{notification_subjects::*, providers::*};

pub(crate) const PREFIX: &str = "notification_subject:";
const CURRENT: &str = "FROM notification_subject_selectors s JOIN accounts a ON a.id=s.account_id JOIN items n ON n.account_id=s.account_id AND n.id=s.notification_id AND n.kind='notification' JOIN scope_membership m ON m.account_id=n.account_id AND m.scope='notifications' AND m.entity_id=n.id AND m.active=1 JOIN repositories r ON r.account_id=n.account_id AND r.id=n.repository_id AND r.provider_id=s.repository_provider_id WHERE a.state='active' AND CAST(a.authorization_epoch AS TEXT)=s.authorization_epoch AND NOT EXISTS(SELECT 1 FROM sync_scopes d WHERE d.account_id=s.account_id AND d.scope IN ('notifications','repositories') AND d.access_denied=1) AND NOT EXISTS(SELECT 1 FROM scope_membership d WHERE d.account_id=r.account_id AND d.scope='repositories' AND d.entity_id=r.id AND d.active=0)";

// Only immutable Native representation keys participate. Mutable path/web
// aliases and multiple distinct native keys naming one canonical resource do not
// contradict that resource. Retained/hidden canonical claims still participate.
const IMMUTABLE_UNAMBIGUOUS: &str = "NOT EXISTS(SELECT 1 FROM resource_aliases own WHERE own.account_id=i.account_id AND own.instance_id=i.instance_id AND own.kind=i.kind AND own.entity_id=i.entity_id AND own.alias_kind='native' AND (EXISTS(SELECT 1 FROM resource_aliases other WHERE other.account_id=own.account_id AND other.instance_id=own.instance_id AND other.kind=own.kind AND other.alias_kind=own.alias_kind AND other.value=own.value AND other.repository_path=own.repository_path AND other.entity_id<>own.entity_id) OR EXISTS(SELECT 1 FROM pending_endpoint_aliases p JOIN resource_identities other ON other.account_id=p.account_id AND other.instance_id=p.instance_id AND other.kind=p.kind AND other.repository_provider_id=p.repository_provider_id AND other.number=p.number WHERE p.account_id=own.account_id AND p.instance_id=own.instance_id AND p.kind=own.kind AND p.native_identity=own.value AND own.repository_path='' AND other.entity_id<>own.entity_id)))";

fn provenance_sql(select: &str) -> String {
    let detail_scopes = super::details::scope_sql_list("i.entity_id");
    format!(
        "{select} {CURRENT} AND s.account_id=i.account_id AND s.kind=i.kind AND s.instance_id=i.instance_id AND s.repository_provider_id=i.repository_provider_id AND s.number=i.number AND {IMMUTABLE_UNAMBIGUOUS} AND (SELECT count(*) FROM resource_identities c WHERE c.account_id=i.account_id AND c.instance_id=i.instance_id AND c.kind=i.kind AND c.repository_provider_id=i.repository_provider_id AND c.number=i.number)=1 AND NOT EXISTS(SELECT 1 FROM sync_scopes d WHERE d.account_id=i.account_id AND d.access_denied=1 AND d.scope IN ('repo:'||r.id||':'||i.kind,{detail_scopes},'notification_subject:'||s.notification_id))"
    )
}

pub(super) async fn provenance_in(
    tx: &mut Transaction<'_, Sqlite>,
    account: &str,
    subject: &str,
) -> Result<bool> {
    let sql = format!(
        "SELECT EXISTS(SELECT 1 FROM resource_identities i JOIN items cached ON cached.account_id=i.account_id AND cached.id=i.entity_id AND cached.kind=i.kind WHERE i.account_id=? AND i.entity_id=? AND i.kind IN ('pull_request','issue') AND EXISTS({}))",
        provenance_sql("SELECT 1")
    );
    sqlx::QueryBuilder::<Sqlite>::new(sql)
        .build_query_scalar()
        .bind(account)
        .bind(subject)
        .fetch_one(&mut **tx)
        .await
        .map_err(storage_error)
}

/// SQLite holds the prior ID set, without materializing the inbox in native memory.
pub(super) async fn capture_in(tx: &mut Transaction<'_, Sqlite>, account: &str) -> Result<()> {
    sqlx::query(
        "CREATE TEMP TABLE IF NOT EXISTS prior_notification_subjects(id TEXT PRIMARY KEY NOT NULL)",
    )
    .execute(&mut **tx)
    .await
    .map_err(storage_error)?;
    sqlx::query("DELETE FROM prior_notification_subjects")
        .execute(&mut **tx)
        .await
        .map_err(storage_error)?;
    let sql = format!(
        "INSERT INTO prior_notification_subjects SELECT i.entity_id FROM resource_identities i JOIN items cached ON cached.account_id=i.account_id AND cached.id=i.entity_id AND cached.kind=i.kind WHERE i.account_id=? AND i.kind IN ('pull_request','issue') AND EXISTS({})",
        provenance_sql("SELECT 1")
    );
    sqlx::QueryBuilder::<Sqlite>::new(sql)
        .build()
        .bind(account)
        .execute(&mut **tx)
        .await
        .map_err(storage_error)?;
    Ok(())
}

/// Withdraw only the derived grant. Ordinary selected eligibility remains separate.
pub(super) async fn reconcile_in(
    tx: &mut Transaction<'_, Sqlite>,
    account: &RemoteAccount,
) -> Result<bool> {
    let sql = format!(
        "DELETE FROM prior_notification_subjects WHERE id IN (SELECT i.entity_id FROM resource_identities i WHERE i.account_id=? AND EXISTS({}))",
        provenance_sql("SELECT 1")
    );
    sqlx::QueryBuilder::<Sqlite>::new(sql)
        .build()
        .bind(&account.id)
        .execute(&mut **tx)
        .await
        .map_err(storage_error)?;
    // This set contains only withdrawn coordinates; apply exactly the shared
    // ordinary eligibility policy before issuing an authorization reset.
    let mut cursor = String::new();
    loop {
        let rows=sqlx::query("SELECT p.id,i.kind FROM prior_notification_subjects p JOIN items i ON i.account_id=? AND i.id=p.id WHERE p.id>? ORDER BY p.id LIMIT 100").bind(&account.id).bind(&cursor).fetch_all(&mut **tx).await.map_err(storage_error)?;
        if rows.is_empty() {
            break;
        }
        for row in rows {
            let id: String = row.get("id");
            cursor = id.clone();
            let kind = if row.get::<String, _>("kind") == "pull_request" {
                ResourceKind::PullRequest
            } else {
                ResourceKind::Issue
            };
            if identities::ordinary_accessible(tx, &account.id, &id, kind).await? {
                sqlx::query("DELETE FROM prior_notification_subjects WHERE id=?")
                    .bind(id)
                    .execute(&mut **tx)
                    .await
                    .map_err(storage_error)?;
            }
        }
    }
    let withdrawn: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM prior_notification_subjects)")
            .fetch_one(&mut **tx)
            .await
            .map_err(storage_error)?;
    if withdrawn {
        sqlx::QueryBuilder::<Sqlite>::new(format!("UPDATE sync_scopes SET run_id=?,next_cursor=NULL,etag=NULL WHERE account_id=? AND EXISTS(SELECT 1 FROM prior_notification_subjects p WHERE sync_scopes.scope IN ({}))", super::details::scope_sql_list("p.id"))).build().bind(Uuid::new_v4().to_string()).bind(&account.id).execute(&mut **tx).await.map_err(storage_error)?;
        sqlx::query("UPDATE detail_demand SET requested=0 WHERE account_id=? AND subject_id IN(SELECT id FROM prior_notification_subjects)").bind(&account.id).execute(&mut **tx).await.map_err(storage_error)?;
        sqlx::query(
            "UPDATE runtime_meta SET authorization_view=authorization_view+1 WHERE singleton=1",
        )
        .execute(&mut **tx)
        .await
        .map_err(storage_error)?;
        record_change(
            tx,
            &account.id,
            positive_revision(&account.authorization_epoch)?,
            "notification_subject:*",
            true,
        )
        .await?;
    }
    // Selector replacement, retirement or known denial cannot revive explicit work.
    let current = format!(
        "UPDATE notification_subject_discovery SET requested=0,run_id=NULL WHERE account_id=? AND (selector_generation NOT IN(SELECT selector_generation {CURRENT} AND s.account_id=?) OR authorization_epoch<>?)"
    );
    sqlx::QueryBuilder::<Sqlite>::new(current)
        .build()
        .bind(&account.id)
        .bind(&account.id)
        .bind(&account.authorization_epoch)
        .execute(&mut **tx)
        .await
        .map_err(storage_error)?;
    Ok(withdrawn)
}

fn reason(mapping: &NotificationSubjectMapping) -> NotificationSubjectReason {
    match mapping {
        NotificationSubjectMapping::Selector(_) => NotificationSubjectReason::NotCached,
        NotificationSubjectMapping::Fallback(
            NotificationSubjectFallbackReason::UnsupportedSubjectType,
        ) => NotificationSubjectReason::UnsupportedSubject,
        NotificationSubjectMapping::Fallback(
            NotificationSubjectFallbackReason::MissingSubjectType
            | NotificationSubjectFallbackReason::MissingSubjectUrl,
        ) => NotificationSubjectReason::MissingSelector,
        NotificationSubjectMapping::Fallback(_) => NotificationSubjectReason::InvalidSelector,
    }
}

fn validate_mapping(
    mapping: &NotificationSubjectMapping,
    repo: &RemoteRepository,
    same_observation: bool,
) -> Result<()> {
    if encode(mapping)?.len() > 4096 {
        return Err(CollaborationError::invalid(
            "Notification selector exceeds its bound",
        ));
    }
    if let NotificationSubjectMapping::Selector(s) = mapping {
        let positive = |s: &str| s.parse::<u64>().is_ok_and(|v| v > 0 && v.to_string() == s);
        if !positive(&s.number)
            || !positive(&s.repository_provider_id)
            || s.repository_provider_id != repo.provider_id
            || (same_observation && s.repository_path != repo.full_name)
            || !matches!(
                (s.kind, s.representation),
                (
                    NotificationSubjectKind::PullRequest,
                    NotificationSubjectRepresentation::GithubPullRequest
                ) | (
                    NotificationSubjectKind::Issue,
                    NotificationSubjectRepresentation::GithubIssue
                )
            )
        {
            return Err(CollaborationError::invalid(
                "Notification selector has the wrong immutable parent",
            ));
        }
    }
    Ok(())
}

pub(super) async fn observe_in(
    tx: &mut Transaction<'_, Sqlite>,
    account: &RemoteAccount,
    page: &PageCommit,
    observations: &[NotificationSubjectObservation],
) -> Result<()> {
    if page.scope != "notifications" {
        if !observations.is_empty() {
            return Err(CollaborationError::invalid(
                "Selectors require an inbox page",
            ));
        }
        return Ok(());
    }
    if page.not_modified {
        if !observations.is_empty() {
            return Err(CollaborationError::invalid(
                "Unmodified inbox cannot carry selectors",
            ));
        }
        return Ok(());
    }
    if observations.len() > page.items.len()
        || observations.iter().enumerate().any(|(i, o)| {
            observations[..i]
                .iter()
                .any(|p| p.notification_id == o.notification_id)
        })
    {
        return Err(CollaborationError::invalid("Invalid inbox selector batch"));
    }
    let instance = identities::instance_in(tx, account).await?;
    for item in &page.items {
        // The list transaction has already rejected older item representations.
        // Their locators must not replace the retained newer notification either.
        let saved: Option<String> =
            sqlx::query_scalar("SELECT json FROM items WHERE account_id=? AND id=?")
                .bind(&account.id)
                .bind(&item.id)
                .fetch_optional(&mut **tx)
                .await
                .map_err(storage_error)?;
        if let Some(saved) = saved {
            let saved: RemoteItem = decode(&saved)?;
            if timestamp_older(&item.updated_at, &saved.updated_at) {
                continue;
            }
        }
        let mapping = observations
            .iter()
            .find(|o| o.notification_id == item.id)
            .map(|o| o.mapping.clone())
            .unwrap_or(NotificationSubjectMapping::Fallback(
                NotificationSubjectFallbackReason::MissingSubjectUrl,
            ));
        let Some(repo_id) = item.repository_id.as_deref() else {
            // Provider-neutral todo/notification rows may have no repository.
            // They remain inbox content but cannot grant subject discovery.
            if matches!(mapping, NotificationSubjectMapping::Selector(_)) {
                return Err(CollaborationError::invalid(
                    "Notification selector requires a captured repository",
                ));
            }
            continue;
        };
        let row = sqlx::query("SELECT json,selected FROM repositories WHERE account_id=? AND id=?")
            .bind(&account.id)
            .bind(repo_id)
            .fetch_one(&mut **tx)
            .await
            .map_err(storage_error)?;
        let repo = repository_from_row(&row)?;
        validate_mapping(&mapping, &repo, true)?;
        let json = encode(&mapping)?;
        let previous:Option<(String,String)>=sqlx::query_as("SELECT mapping_json,selector_generation FROM notification_subject_selectors WHERE account_id=? AND notification_id=?").bind(&account.id).bind(&item.id).fetch_optional(&mut **tx).await.map_err(storage_error)?;
        let generation = previous
            .as_ref()
            .filter(|(prior, _)| prior == &json)
            .map(|(_, generation)| generation.clone())
            .unwrap_or_else(|| Uuid::new_v4().to_string());
        let selector = if let NotificationSubjectMapping::Selector(s) = &mapping {
            Some(s)
        } else {
            None
        };
        sqlx::query("INSERT INTO notification_subject_selectors(account_id,notification_id,instance_id,authorization_epoch,selector_generation,mapping_json,kind,repository_provider_id,number) VALUES(?,?,?,?,?,?,?,?,?) ON CONFLICT(account_id,notification_id) DO UPDATE SET instance_id=excluded.instance_id,authorization_epoch=excluded.authorization_epoch,selector_generation=excluded.selector_generation,mapping_json=excluded.mapping_json,kind=excluded.kind,repository_provider_id=excluded.repository_provider_id,number=excluded.number")
            .bind(&account.id).bind(&item.id).bind(&instance.id).bind(&account.authorization_epoch).bind(&generation).bind(json).bind(selector.map(|s|tag(&s.kind)).transpose()?).bind(selector.map(|s|s.repository_provider_id.as_str())).bind(selector.map(|s|s.number.as_str())).execute(&mut **tx).await.map_err(storage_error)?;
        if previous.as_ref().is_none_or(|(_, old)| old != &generation) {
            sqlx::query("DELETE FROM notification_subject_discovery WHERE account_id=? AND notification_id=?").bind(&account.id).bind(&item.id).execute(&mut **tx).await.map_err(storage_error)?;
            sqlx::query("DELETE FROM sync_scopes WHERE account_id=? AND scope=?")
                .bind(&account.id)
                .bind(format!("{PREFIX}{}", item.id))
                .execute(&mut **tx)
                .await
                .map_err(storage_error)?;
        }
        record_change(
            tx,
            &account.id,
            positive_revision(&account.authorization_epoch)?,
            &format!("{PREFIX}{}", item.id),
            false,
        )
        .await?;
    }
    if observations
        .iter()
        .any(|o| !page.items.iter().any(|i| i.id == o.notification_id))
    {
        return Err(CollaborationError::invalid(
            "Selector does not belong to an observed notification",
        ));
    }
    Ok(())
}

#[derive(Clone)]
struct Current {
    account: RemoteAccount,
    instance: ProviderInstance,
    repository: RemoteRepository,
    mapping: NotificationSubjectMapping,
    generation: String,
    claims: Vec<CanonicalResource>,
    immutable_alias_ambiguous: bool,
}

async fn current_in(
    tx: &mut Transaction<'_, Sqlite>,
    query: &NotificationSubjectQuery,
) -> Result<std::result::Result<Current, NotificationSubjectReason>> {
    validate_identifier(&query.notification_id)?;
    let account = account_in(tx, &query.account_id, false).await?;
    if account.authorization_epoch != query.authorization_epoch {
        return Err(stale());
    }
    if account.state != AccountState::Active {
        return Ok(Err(NotificationSubjectReason::AuthenticationRequired));
    }
    let denied:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM sync_scopes WHERE account_id=? AND scope IN ('notifications','repositories') AND access_denied=1)").bind(&account.id).fetch_one(&mut **tx).await.map_err(storage_error)?;
    if denied {
        return Ok(Err(NotificationSubjectReason::PermissionDenied));
    }
    let denied: Option<String> = sqlx::query_scalar(
        "SELECT sync_json FROM sync_scopes WHERE account_id=? AND scope=? AND access_denied=1",
    )
    .bind(&account.id)
    .bind(format!("{PREFIX}{}", query.notification_id))
    .fetch_optional(&mut **tx)
    .await
    .map_err(storage_error)?;
    if let Some(status) = denied {
        let status: SyncStatus = decode(&status)?;
        let reason = if status.error.is_some_and(|e| e.code == ErrorCode::NotFound) {
            NotificationSubjectReason::NotFound
        } else {
            NotificationSubjectReason::PermissionDenied
        };
        return Ok(Err(reason));
    }
    let row=sqlx::query("SELECT r.json,r.selected,s.mapping_json,s.selector_generation,s.instance_id,s.authorization_epoch FROM items n JOIN scope_membership m ON m.account_id=n.account_id AND m.scope='notifications' AND m.entity_id=n.id AND m.active=1 LEFT JOIN repositories r ON r.account_id=n.account_id AND r.id=n.repository_id LEFT JOIN notification_subject_selectors s ON s.account_id=n.account_id AND s.notification_id=n.id WHERE n.account_id=? AND n.id=? AND n.kind='notification' AND NOT EXISTS(SELECT 1 FROM scope_membership d WHERE d.account_id=r.account_id AND d.scope='repositories' AND d.entity_id=r.id AND d.active=0)")
        .bind(&account.id).bind(&query.notification_id).fetch_optional(&mut **tx).await.map_err(storage_error)?;
    let Some(row) = row else {
        return Ok(Err(NotificationSubjectReason::InactiveMembership));
    };
    // The notification remains a current inbox row, but no captured parent can
    // ever grant canonical subject access or become point HTTP authority.
    if row.get::<Option<String>, _>("json").is_none() {
        return Ok(Err(NotificationSubjectReason::MissingSelector));
    }
    let instance = identities::instance_in(tx, &account).await?;
    let json = row.get::<Option<String>, _>("mapping_json");
    if json.is_some()
        && (row.get::<String, _>("instance_id") != instance.id
            || row.get::<String, _>("authorization_epoch") != account.authorization_epoch)
    {
        return Ok(Err(NotificationSubjectReason::InactiveMembership));
    }
    let repository = repository_from_row(&row)?;
    let mapping: NotificationSubjectMapping =
        json.map(|s| decode(&s))
            .transpose()?
            .unwrap_or(NotificationSubjectMapping::Fallback(
                NotificationSubjectFallbackReason::MissingSubjectUrl,
            ));
    validate_mapping(&mapping, &repository, false)?;
    let mut claims = Vec::new();
    let mut immutable_alias_ambiguous = false;
    if let NotificationSubjectMapping::Selector(s) = &mapping {
        let denied:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM sync_scopes WHERE account_id=? AND scope=? AND access_denied=1)").bind(&account.id).bind(repository_scope(&repository.id,&s.kind.item_kind())).fetch_one(&mut **tx).await.map_err(storage_error)?;
        if denied {
            return Ok(Err(NotificationSubjectReason::PermissionDenied));
        }
        let sql = format!(
            "SELECT entity_id,provider_id,{IMMUTABLE_UNAMBIGUOUS} AS immutable_unambiguous FROM resource_identities i WHERE account_id=? AND instance_id=? AND kind=? AND repository_provider_id=? AND number=? LIMIT 2"
        );
        for row in sqlx::QueryBuilder::<Sqlite>::new(sql)
            .build()
            .bind(&account.id)
            .bind(&instance.id)
            .bind(tag(&s.kind.resource_kind())?)
            .bind(&s.repository_provider_id)
            .bind(&s.number)
            .fetch_all(&mut **tx)
            .await
            .map_err(storage_error)?
        {
            immutable_alias_ambiguous |= !row.get::<bool, _>("immutable_unambiguous");
            claims.push(CanonicalResource {
                account_id: account.id.clone(),
                instance_id: instance.id.clone(),
                id: row.get("entity_id"),
                kind: s.kind.resource_kind(),
                provider_id: row.get("provider_id"),
            });
        }
    }
    Ok(Ok(Current {
        account,
        instance,
        repository,
        mapping,
        generation: row
            .get::<Option<String>, _>("selector_generation")
            .unwrap_or_default(),
        claims,
        immutable_alias_ambiguous,
    }))
}

fn query_for(request: &DiscoverNotificationSubjectRequest) -> NotificationSubjectQuery {
    NotificationSubjectQuery {
        account_id: request.account_id.clone(),
        authorization_epoch: request.authorization_epoch.clone(),
        notification_id: request.notification_id.clone(),
    }
}
fn denied() -> CollaborationError {
    CollaborationError::new(
        ErrorCode::PermissionDenied,
        "Notification subject is inaccessible",
    )
}

#[derive(Debug, Clone)]
pub struct NotificationDiscoveryIntent {
    pub account_id: String,
    pub authorization_epoch: String,
    pub notification_id: String,
    pub selector_generation: String,
    pub intent_generation: String,
    pub attempts: u32,
}

#[derive(Debug, Clone)]
pub struct NotificationDiscoveryLease {
    pub request: TrustedNotificationSubjectRequest,
    pub intent_generation: String,
    pub run_id: String,
    pub expected_claims: Vec<CanonicalResource>,
    pub expected_subject: Option<RemoteItem>,
    pub attempts: u32,
}

impl Store {
    /// Pure factory uses exactly the actor/instance/kind captured in this snapshot.
    pub async fn notification_subject(
        &self,
        query: NotificationSubjectQuery,
        support_for: impl FnOnce(
            &RemoteAccount,
            &ProviderInstance,
            Option<NotificationSubjectKind>,
        ) -> CapabilityState,
    ) -> Result<NotificationSubjectSnapshot> {
        let mut tx = self.inner.readers.begin().await.map_err(storage_error)?;
        let (revision, authorization_view) = metadata(&mut tx).await?;
        let mut result = NotificationSubjectSnapshot {
            revision,
            authorization_view,
            authorization_epoch: query.authorization_epoch.clone(),
            state: NotificationSubjectState::Unavailable,
            reason: None,
            selector_generation: None,
            subject: None,
            fallback_web_url: None,
            discovery: NotificationSubjectDiscoveryPolicy {
                support: CapabilityState::Unavailable,
                admission: false,
                paused: false,
                retry_at: None,
                attempts: 0,
                sync: SyncStatus::default(),
            },
        };
        let current = match current_in(&mut tx, &query).await? {
            Ok(c) => c,
            Err(reason) => {
                result.reason = Some(reason);
                if reason == NotificationSubjectReason::MissingSelector {
                    result.state = NotificationSubjectState::Unsupported;
                    result.discovery.support = CapabilityState::Unsupported;
                }
                return Ok(result);
            }
        };
        let selector = if let NotificationSubjectMapping::Selector(s) = &current.mapping {
            Some(s)
        } else {
            None
        };
        let support = support_for(
            &current.account,
            &current.instance,
            selector.map(|s| s.kind),
        );
        result.discovery.support = support;
        result.selector_generation =
            (!current.generation.is_empty()).then(|| current.generation.clone());
        result.fallback_web_url = safe_fallback(&current);
        result.reason = Some(reason(&current.mapping));
        result.state = if selector.is_some() {
            NotificationSubjectState::NotCached
        } else {
            NotificationSubjectState::Unsupported
        };
        if current.claims.len() > 1 || current.immutable_alias_ambiguous {
            result.state = NotificationSubjectState::Ambiguous;
            result.reason = Some(NotificationSubjectReason::AmbiguousIdentity);
        } else if let Some(claim) = current.claims.first() {
            if identities::accessible(&mut tx, &current.account.id, &claim.id, claim.kind).await? {
                result.state = NotificationSubjectState::Resolved;
                result.reason = None;
                result.subject = Some(claim.clone());
            } else {
                let saved: bool = sqlx::query_scalar(
                    "SELECT EXISTS(SELECT 1 FROM items WHERE account_id=? AND id=?)",
                )
                .bind(&current.account.id)
                .bind(&claim.id)
                .fetch_one(&mut *tx)
                .await
                .map_err(storage_error)?;
                if saved {
                    result.state = NotificationSubjectState::Unavailable;
                    result.reason = Some(NotificationSubjectReason::PermissionDenied);
                    result.selector_generation = None;
                    result.fallback_web_url = None;
                }
            }
        }
        let intent=sqlx::query("SELECT attempts,outcome_reason FROM notification_subject_discovery WHERE account_id=? AND notification_id=? AND selector_generation=?").bind(&query.account_id).bind(&query.notification_id).bind(&current.generation).fetch_optional(&mut *tx).await.map_err(storage_error)?;
        if let Some(row) = intent {
            result.discovery.attempts = row.get::<i64, _>("attempts") as u32;
            if result.state == NotificationSubjectState::NotCached
                && let Some(reason) = row.get::<Option<String>, _>("outcome_reason")
            {
                let reason: NotificationSubjectReason = decode(&reason)?;
                result.reason = Some(reason);
                if reason == NotificationSubjectReason::IdentityUnverified {
                    result.state = NotificationSubjectState::IdentityUnverified;
                } else if reason == NotificationSubjectReason::AttemptsExhausted {
                    result.state = NotificationSubjectState::Unavailable;
                }
            }
        }
        result.discovery.sync = scope_in(
            &mut tx,
            &query.account_id,
            &format!("{PREFIX}{}", query.notification_id),
        )
        .await?
        .map(|s| s.sync)
        .unwrap_or_default();
        let (_, provider) =
            presentation(scope_in(&mut tx, &query.account_id, "provider:rest").await?);
        let now = chrono::Utc::now();
        let deadline = |s: &SyncStatus| {
            s.next_retry_at
                .as_ref()
                .and_then(|t| chrono::DateTime::parse_from_rfc3339(t).ok())
                .filter(|t| *t > now)
        };
        if deadline(&provider)
            .is_some_and(|at| deadline(&result.discovery.sync).is_none_or(|own| at > own))
        {
            result.discovery.sync = provider;
        }
        result.discovery.retry_at = deadline(&result.discovery.sync).map(|t| t.to_rfc3339());
        result.discovery.paused = result.discovery.retry_at.is_some();
        result.discovery.admission = support == CapabilityState::Supported
            && selector.is_some()
            && (matches!(
                result.state,
                NotificationSubjectState::NotCached | NotificationSubjectState::IdentityUnverified
            ) || (result.state == NotificationSubjectState::Unavailable
                && result.reason == Some(NotificationSubjectReason::AttemptsExhausted)));
        if support != CapabilityState::Supported {
            result.discovery.admission = false;
            if support == CapabilityState::Unavailable {
                result.state = NotificationSubjectState::Unavailable;
                result.subject = None;
                result.selector_generation = None;
                result.fallback_web_url = None;
            }
        }
        tx.commit().await.map_err(storage_error)?;
        Ok(result)
    }

    pub async fn request_notification_subject_checked(
        &self,
        request: &DiscoverNotificationSubjectRequest,
        guard: impl Fn() -> Result<()>,
    ) -> Result<String> {
        let mut writer = self.inner.writer.lock().await;
        guard()?;
        let mut tx = writer.begin().await.map_err(storage_error)?;
        let current = current_in(&mut tx, &query_for(request))
            .await?
            .map_err(|_| denied())?;
        if current.generation != request.selector_generation {
            return Err(stale());
        }
        if !matches!(current.mapping, NotificationSubjectMapping::Selector(_))
            || current.claims.len() > 1
            || current.immutable_alias_ambiguous
        {
            return Err(CollaborationError::new(
                ErrorCode::Unsupported,
                "Notification subject cannot be discovered",
            ));
        }
        if let Some(claim) = current.claims.first()
            && identities::accessible(&mut tx, &current.account.id, &claim.id, claim.kind).await?
        {
            return Err(CollaborationError::invalid(
                "The notification subject is already cached",
            ));
        }
        if let Some(claim) = current.claims.first() {
            let saved: bool = sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM items WHERE account_id=? AND id=?)",
            )
            .bind(&current.account.id)
            .bind(&claim.id)
            .fetch_one(&mut *tx)
            .await
            .map_err(storage_error)?;
            if saved {
                return Err(denied());
            }
        }
        let existing:Option<(String,bool)>=sqlx::query_as("SELECT intent_generation,requested FROM notification_subject_discovery WHERE account_id=? AND notification_id=? AND selector_generation=?").bind(&request.account_id).bind(&request.notification_id).bind(&request.selector_generation).fetch_optional(&mut *tx).await.map_err(storage_error)?;
        if let Some((id, true)) = existing {
            guard()?;
            tx.commit().await.map_err(storage_error)?;
            return Ok(id);
        }
        let counts:(i64,i64)=sqlx::query_as("SELECT coalesce(sum(CASE WHEN account_id=? THEN 1 ELSE 0 END),0),count(*) FROM notification_subject_discovery WHERE requested=1").bind(&request.account_id).fetch_one(&mut *tx).await.map_err(storage_error)?;
        if counts.0 >= 16 || counts.1 >= 64 {
            return Err(CollaborationError::new(
                ErrorCode::Busy,
                "Notification discovery intent is full",
            ));
        }
        let generation = Uuid::new_v4().to_string();
        sqlx::query("INSERT INTO notification_subject_discovery(account_id,notification_id,authorization_epoch,selector_generation,intent_generation,requested,attempts,run_id,outcome_reason) VALUES(?,?,?,?,?,1,0,NULL,NULL) ON CONFLICT(account_id,notification_id) DO UPDATE SET authorization_epoch=excluded.authorization_epoch,selector_generation=excluded.selector_generation,intent_generation=excluded.intent_generation,requested=1,attempts=0,run_id=NULL,outcome_reason=NULL")
            .bind(&request.account_id).bind(&request.notification_id).bind(&request.authorization_epoch).bind(&request.selector_generation).bind(&generation).execute(&mut *tx).await.map_err(storage_error)?;
        record_change(
            &mut tx,
            &request.account_id,
            positive_revision(&request.authorization_epoch)?,
            &format!("{PREFIX}{}", request.notification_id),
            false,
        )
        .await?;
        guard()?;
        tx.commit().await.map_err(storage_error)?;
        Ok(generation)
    }

    pub async fn pending_notification_subjects(&self) -> Result<Vec<NotificationDiscoveryIntent>> {
        let rows=sqlx::query("SELECT account_id,authorization_epoch,notification_id,selector_generation,intent_generation,attempts FROM notification_subject_discovery WHERE requested=1 ORDER BY account_id,notification_id LIMIT 64").fetch_all(&self.inner.readers).await.map_err(storage_error)?;
        Ok(rows
            .into_iter()
            .map(|r| NotificationDiscoveryIntent {
                account_id: r.get("account_id"),
                authorization_epoch: r.get("authorization_epoch"),
                notification_id: r.get("notification_id"),
                selector_generation: r.get("selector_generation"),
                intent_generation: r.get("intent_generation"),
                attempts: r.get::<i64, _>("attempts") as u32,
            })
            .collect())
    }

    pub async fn begin_notification_subject(
        &self,
        intent: &NotificationDiscoveryIntent,
    ) -> Result<NotificationDiscoveryLease> {
        let mut writer = self.inner.writer.lock().await;
        let mut tx = writer.begin().await.map_err(storage_error)?;
        let query = NotificationSubjectQuery {
            account_id: intent.account_id.clone(),
            authorization_epoch: intent.authorization_epoch.clone(),
            notification_id: intent.notification_id.clone(),
        };
        let current = current_in(&mut tx, &query).await?.map_err(|_| denied())?;
        if current.generation != intent.selector_generation
            || current.claims.len() > 1
            || current.immutable_alias_ambiguous
        {
            return Err(stale());
        }
        if let Some(claim) = current.claims.first() {
            let saved: bool = sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM items WHERE account_id=? AND id=?)",
            )
            .bind(&current.account.id)
            .bind(&claim.id)
            .fetch_one(&mut *tx)
            .await
            .map_err(storage_error)?;
            if saved {
                return Err(stale());
            }
        }
        let NotificationSubjectMapping::Selector(mut selector) = current.mapping else {
            return Err(stale());
        };
        // A cached authoritative rename updates request presentation coordinates,
        // never the immutable parent captured from the notification.
        selector.repository_path = current.repository.full_name.clone();
        let row:Option<(String,bool,i64)>=sqlx::query_as("SELECT intent_generation,requested,attempts FROM notification_subject_discovery WHERE account_id=? AND notification_id=? AND selector_generation=?").bind(&intent.account_id).bind(&intent.notification_id).bind(&intent.selector_generation).fetch_optional(&mut *tx).await.map_err(storage_error)?;
        if row.is_none_or(|(generation, requested, attempts)| {
            generation != intent.intent_generation || !requested || attempts >= 3
        }) {
            return Err(stale());
        }
        let run = Uuid::new_v4().to_string();
        sqlx::query("UPDATE notification_subject_discovery SET attempts=attempts+1,run_id=? WHERE account_id=? AND notification_id=?").bind(&run).bind(&intent.account_id).bind(&intent.notification_id).execute(&mut *tx).await.map_err(storage_error)?;
        let expected_subject = if let Some(claim) = current.claims.first() {
            sqlx::query_scalar::<_, String>("SELECT json FROM items WHERE account_id=? AND id=?")
                .bind(&intent.account_id)
                .bind(&claim.id)
                .fetch_optional(&mut *tx)
                .await
                .map_err(storage_error)?
                .map(|s| decode(&s))
                .transpose()?
        } else {
            None
        };
        let attempts:i64=sqlx::query_scalar("SELECT attempts FROM notification_subject_discovery WHERE account_id=? AND notification_id=?").bind(&intent.account_id).bind(&intent.notification_id).fetch_one(&mut *tx).await.map_err(storage_error)?;
        let lease = NotificationDiscoveryLease {
            request: TrustedNotificationSubjectRequest {
                account: current.account,
                instance_id: current.instance.id,
                notification_id: intent.notification_id.clone(),
                selector_generation: intent.selector_generation.clone(),
                authorization_view: metadata(&mut tx).await?.1,
                repository: current.repository,
                selector,
            },
            intent_generation: intent.intent_generation.clone(),
            run_id: run,
            expected_claims: current.claims,
            expected_subject,
            attempts: attempts as u32,
        };
        record_change(
            &mut tx,
            &intent.account_id,
            positive_revision(&intent.authorization_epoch)?,
            &format!("{PREFIX}{}", intent.notification_id),
            false,
        )
        .await?;
        tx.commit().await.map_err(storage_error)?;
        Ok(lease)
    }
}

async fn validate_lease_in(
    tx: &mut Transaction<'_, Sqlite>,
    lease: &NotificationDiscoveryLease,
) -> Result<Current> {
    let request = &lease.request;
    let query = NotificationSubjectQuery {
        account_id: request.account.id.clone(),
        authorization_epoch: request.account.authorization_epoch.clone(),
        notification_id: request.notification_id.clone(),
    };
    let current = current_in(tx, &query).await?.map_err(|_| stale())?;
    if current.immutable_alias_ambiguous
        || current.generation != request.selector_generation
        || current.instance.id != request.instance_id
        || metadata(tx).await?.1 != request.authorization_view
        || current.claims != lease.expected_claims
        || current.repository.id != request.repository.id
        || current.repository.provider_id != request.repository.provider_id
        || current.repository.full_name != request.repository.full_name
    {
        return Err(stale());
    }
    let row:Option<(String,String,String,bool)>=sqlx::query_as("SELECT selector_generation,intent_generation,run_id,requested FROM notification_subject_discovery WHERE account_id=? AND notification_id=?")
        .bind(&request.account.id).bind(&request.notification_id).fetch_optional(&mut **tx).await.map_err(storage_error)?;
    if row.is_none_or(|(selector, intent, run, requested)| {
        selector != request.selector_generation
            || intent != lease.intent_generation
            || run != lease.run_id
            || !requested
    }) {
        return Err(stale());
    }
    if let Some(expected) = &lease.expected_subject {
        let json: Option<String> =
            sqlx::query_scalar("SELECT json FROM items WHERE account_id=? AND id=?")
                .bind(&request.account.id)
                .bind(&expected.id)
                .fetch_optional(&mut **tx)
                .await
                .map_err(storage_error)?;
        let subject: RemoteItem = json.map(|s| decode(&s)).transpose()?.ok_or_else(stale)?;
        if subject.repository_id != expected.repository_id
            || subject.provider_id != expected.provider_id
            || subject.kind != expected.kind
            || subject.number != expected.number
            || subject.head_oid != expected.head_oid
            || timestamp_older(&expected.updated_at, &subject.updated_at)
        {
            return Err(stale());
        }
    } else if let Some(claim) = current.claims.first() {
        let saved: bool =
            sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM items WHERE account_id=? AND id=?)")
                .bind(&request.account.id)
                .bind(&claim.id)
                .fetch_one(&mut **tx)
                .await
                .map_err(storage_error)?;
        if saved {
            return Err(stale());
        }
    }
    Ok(current)
}

impl Store {
    /// Recheck the accepted lease and strict budgets after asynchronous credential
    /// access, immediately before the single coordinator dispatches provider I/O.
    pub(crate) async fn notification_subject_dispatchable(
        &self,
        lease: &NotificationDiscoveryLease,
        eligibility_at: &str,
    ) -> Result<()> {
        let mut tx = self.inner.readers.begin().await.map_err(storage_error)?;
        validate_lease_in(&mut tx, lease).await?;
        let now = chrono::DateTime::parse_from_rfc3339(eligibility_at)
            .map_err(|_| CollaborationError::invalid("Invalid native eligibility clock"))?;
        for scope in [
            "provider:rest".to_owned(),
            format!("{PREFIX}{}", lease.request.notification_id),
        ] {
            if scope_in(&mut tx, &lease.request.account.id, &scope)
                .await?
                .and_then(|s| s.sync.next_retry_at)
                .and_then(|t| chrono::DateTime::parse_from_rfc3339(&t).ok())
                .is_some_and(|at| at > now)
            {
                return Err(stale());
            }
        }
        tx.commit().await.map_err(storage_error)?;
        Ok(())
    }

    pub async fn retire_notification_subject_intent(
        &self,
        intent: &NotificationDiscoveryIntent,
    ) -> Result<String> {
        let mut writer = self.inner.writer.lock().await;
        let mut tx = writer.begin().await.map_err(storage_error)?;
        epoch_in(&mut tx, &intent.account_id, &intent.authorization_epoch).await?;
        let changed=sqlx::query("UPDATE notification_subject_discovery SET requested=0,run_id=NULL WHERE account_id=? AND notification_id=? AND selector_generation=? AND intent_generation=? AND requested=1")
            .bind(&intent.account_id).bind(&intent.notification_id).bind(&intent.selector_generation).bind(&intent.intent_generation).execute(&mut *tx).await.map_err(storage_error)?;
        if changed.rows_affected() == 0 {
            return Err(stale());
        }
        let revision = record_change(
            &mut tx,
            &intent.account_id,
            positive_revision(&intent.authorization_epoch)?,
            &format!("{PREFIX}{}", intent.notification_id),
            false,
        )
        .await?;
        tx.commit().await.map_err(storage_error)?;
        Ok(revision)
    }

    pub async fn apply_notification_subject(
        &self,
        lease: &NotificationDiscoveryLease,
        result: NotificationSubjectDiscovery,
    ) -> Result<String> {
        let mut writer = self.inner.writer.lock().await;
        let mut tx = writer.begin().await.map_err(storage_error)?;
        let current = validate_lease_in(&mut tx, lease).await?;
        let account = &current.account;
        let scope = format!("{PREFIX}{}", lease.request.notification_id);
        match result {
            NotificationSubjectDiscovery::Failed { .. } => {
                return Err(CollaborationError::invalid(
                    "Failed discovery requires error publication",
                ));
            }
            NotificationSubjectDiscovery::Unresolved { reason, .. } => {
                if !matches!(
                    reason,
                    NotificationSubjectReason::IdentityUnverified
                        | NotificationSubjectReason::RepresentationMismatch
                ) {
                    return Err(CollaborationError::invalid("Invalid discovery outcome"));
                }
                sqlx::query("UPDATE notification_subject_discovery SET requested=0,outcome_reason=? WHERE account_id=? AND notification_id=?").bind(encode(&reason)?).bind(&account.id).bind(&lease.request.notification_id).execute(&mut *tx).await.map_err(storage_error)?;
            }
            NotificationSubjectDiscovery::Verified {
                subject,
                detail,
                endpoint_aliases,
            } => {
                let mut subject = *subject;
                let detail = *detail;
                if detail.not_modified
                    || detail.next_cursor.is_some()
                    || !detail.entries.is_empty()
                    || subject.account_id != account.id
                    || subject.repository_id.as_ref() != Some(&current.repository.id)
                    || subject.kind != lease.request.selector.kind.item_kind()
                    || subject.number.as_ref() != Some(&lease.request.selector.number)
                    || subject.title.len() > 16_384
                    || subject.state.len() > 128
                    || subject
                        .body
                        .as_ref()
                        .is_some_and(|b| b.len() > MAX_BODY_BYTES)
                    || subject.reason.is_some()
                    || subject.unread.is_some()
                {
                    return Err(CollaborationError::invalid("Invalid point observation"));
                }
                validate_identifier(&subject.provider_id)?;
                let updated = chrono::DateTime::parse_from_rfc3339(&subject.updated_at)
                    .map_err(|_| CollaborationError::invalid("Invalid point observation clock"))?;
                subject.updated_at = updated
                    .with_timezone(&chrono::Utc)
                    .to_rfc3339_opts(chrono::SecondsFormat::Nanos, true);
                if let Some(prior) = &lease.expected_subject {
                    if timestamp_older(&subject.updated_at, &prior.updated_at) {
                        return Err(stale());
                    }
                    if subject.body_omitted {
                        subject.body = prior.body.clone();
                        subject.body_omitted = prior.body_omitted;
                    }
                }
                let existing:Option<(String,String,Option<String>)>=sqlx::query_as("SELECT entity_id,repository_provider_id,number FROM resource_identities WHERE account_id=? AND instance_id=? AND kind=? AND provider_id=?")
                    .bind(&account.id).bind(&current.instance.id).bind(tag(&subject.kind)?).bind(&subject.provider_id).fetch_optional(&mut *tx).await.map_err(storage_error)?;
                if let Some((id, parent, number)) = existing {
                    if parent != current.repository.provider_id || number != subject.number {
                        return Err(stale());
                    }
                    subject.id = id;
                }
                if current.claims.first().is_some_and(|claim| {
                    claim.id != subject.id || claim.provider_id != subject.provider_id
                }) {
                    return Err(stale());
                }
                validate_identifier(&subject.id)?;
                identities::item_in(&mut tx, account, &subject).await?;
                resource_metadata::invalidate_head_in(
                    &mut tx,
                    account,
                    &subject,
                    lease
                        .expected_subject
                        .as_ref()
                        .and_then(|s| s.head_oid.as_deref()),
                )
                .await?;
                sqlx::query("INSERT INTO items(account_id,id,repository_id,kind,state,updated_at,json) VALUES(?,?,?,?,?,?,?) ON CONFLICT(account_id,id) DO UPDATE SET repository_id=excluded.repository_id,kind=excluded.kind,state=excluded.state,updated_at=excluded.updated_at,json=excluded.json")
                    .bind(&account.id).bind(&subject.id).bind(&subject.repository_id).bind(tag(&subject.kind)?).bind(&subject.state).bind(&subject.updated_at).bind(encode(&subject)?).execute(&mut *tx).await.map_err(storage_error)?;
                if !provenance_in(&mut tx, &account.id, &subject.id).await? {
                    return Err(stale());
                }
                // Endpoint representation aliases must already name the verified
                // parent/kind/number; never manufacture an issue-side ID.
                if endpoint_aliases.len() > 1 {
                    return Err(CollaborationError::invalid(
                        "Point alias batch exceeds its bound",
                    ));
                }
                for alias in &endpoint_aliases {
                    if alias.kind != lease.request.selector.kind.resource_kind()
                        || alias.repository_provider_id != current.repository.provider_id
                        || alias.number != lease.request.selector.number
                    {
                        return Err(CollaborationError::invalid(
                            "Point alias has the wrong parent",
                        ));
                    }
                    identities::endpoint_in(
                        &mut tx,
                        account,
                        alias,
                        &repository_scope(&current.repository.id, &RemoteItemKind::Issue),
                    )
                    .await?;
                }
                if !provenance_in(&mut tx, &account.id, &subject.id).await? {
                    return Err(stale());
                }
                let detail_scope = crate::DetailFacet::Body.scope(&subject.id);
                let old = scope_in(&mut tx, &account.id, &detail_scope).await?;
                sqlx::query("INSERT INTO sync_scopes(account_id,scope,run_id,coverage_json,sync_json) VALUES(?,?,?,?,?) ON CONFLICT(account_id,scope) DO UPDATE SET run_id=excluded.run_id,next_cursor=NULL")
                    .bind(&account.id).bind(&detail_scope).bind(&lease.run_id).bind(encode(&old.as_ref().map(|s|s.coverage.clone()).unwrap_or_else(missing_coverage))?).bind(encode(&old.map(|s|s.sync).unwrap_or_default())?).execute(&mut *tx).await.map_err(storage_error)?;
                details::apply_detail_in(
                    &mut tx,
                    crate::DetailCommit {
                        reconciliation: detail.reconciliation,
                        account_id: account.id.clone(),
                        authorization_epoch: account.authorization_epoch.clone(),
                        authorization_view: lease.request.authorization_view.clone(),
                        instance_id: current.instance.id.clone(),
                        subject_id: subject.id.clone(),
                        facet: crate::DetailFacet::Body,
                        run_id: lease.run_id.clone(),
                        body: detail.body,
                        metadata: detail.metadata,
                        subject_binding: Some(crate::DetailSubjectBinding {
                            repository_id: current.repository.id.clone(),
                            repository_provider_id: current.repository.provider_id.clone(),
                            provider_id: subject.provider_id.clone(),
                            number: subject.number.clone(),
                            kind: subject.kind.clone(),
                            head_oid: subject.head_oid.clone(),
                        }),
                        entries: detail.entries,
                        source: detail.source,
                        next_cursor: None,
                        etag: detail.etag,
                        not_modified: false,
                        complete: true,
                        whole_scope: true,
                        request_cursor: None,
                        freshness_seconds: detail.freshness_seconds,
                    },
                )
                .await?;
                sqlx::query("UPDATE notification_subject_discovery SET requested=0,outcome_reason=NULL WHERE account_id=? AND notification_id=?").bind(&account.id).bind(&lease.request.notification_id).execute(&mut *tx).await.map_err(storage_error)?;
            }
        }
        let status = SyncStatus::default();
        sqlx::query("INSERT INTO sync_scopes(account_id,scope,run_id,coverage_json,sync_json) VALUES(?,?,?,?,?) ON CONFLICT(account_id,scope) DO UPDATE SET sync_json=excluded.sync_json")
            .bind(&account.id).bind(&scope).bind(&lease.run_id).bind(encode(&missing_coverage())?).bind(encode(&status)?).execute(&mut *tx).await.map_err(storage_error)?;
        let revision = record_change(
            &mut tx,
            &account.id,
            positive_revision(&account.authorization_epoch)?,
            &scope,
            false,
        )
        .await?;
        tx.commit().await.map_err(storage_error)?;
        Ok(revision)
    }

    pub async fn fail_notification_subject(
        &self,
        lease: &NotificationDiscoveryLease,
        error: CollaborationError,
        next_retry_at: Option<String>,
    ) -> Result<String> {
        let mut writer = self.inner.writer.lock().await;
        let mut tx = writer.begin().await.map_err(storage_error)?;
        let current = validate_lease_in(&mut tx, lease).await?;
        let mut account = current.account;
        if error.code == ErrorCode::AuthRequired {
            let epoch = positive_revision(&account.authorization_epoch)?
                .checked_add(1)
                .ok_or_else(CollaborationError::storage)?;
            account.authorization_epoch = epoch.to_string();
            account.state = AccountState::AuthRequired;
            clear_remote_cache(&mut tx, &account.id).await?;
            sqlx::query(
                "UPDATE accounts SET authorization_epoch=?,state='auth_required',json=? WHERE id=?",
            )
            .bind(epoch)
            .bind(encode(&account)?)
            .bind(&account.id)
            .execute(&mut *tx)
            .await
            .map_err(storage_error)?;
            sqlx::query(
                "UPDATE runtime_meta SET authorization_view=authorization_view+1 WHERE singleton=1",
            )
            .execute(&mut *tx)
            .await
            .map_err(storage_error)?;
            let revision = record_change(&mut tx, &account.id, epoch, "account", true).await?;
            tx.commit().await.map_err(storage_error)?;
            return Ok(revision);
        }
        capture_in(&mut tx, &account.id).await?;
        let attempts:i64=sqlx::query_scalar("SELECT attempts FROM notification_subject_discovery WHERE account_id=? AND notification_id=?").bind(&account.id).bind(&lease.request.notification_id).fetch_one(&mut *tx).await.map_err(storage_error)?;
        let transient = matches!(
            error.code,
            ErrorCode::Network | ErrorCode::Provider | ErrorCode::RateLimited
        );
        let denied = matches!(
            error.code,
            ErrorCode::PermissionDenied | ErrorCode::NotFound
        );
        let reason = if attempts >= 3 && transient {
            Some(NotificationSubjectReason::AttemptsExhausted)
        } else if denied {
            Some(if error.code == ErrorCode::NotFound {
                NotificationSubjectReason::NotFound
            } else {
                NotificationSubjectReason::PermissionDenied
            })
        } else {
            None
        };
        sqlx::query("UPDATE notification_subject_discovery SET requested=?,outcome_reason=? WHERE account_id=? AND notification_id=?").bind(transient && attempts<3).bind(reason.map(|r|encode(&r)).transpose()?).bind(&account.id).bind(&lease.request.notification_id).execute(&mut *tx).await.map_err(storage_error)?;
        let state = match error.code {
            ErrorCode::Network => SyncState::Offline,
            ErrorCode::RateLimited => SyncState::RateLimited,
            _ => SyncState::Error,
        };
        let sync = SyncStatus {
            state,
            last_success_at: None,
            next_retry_at,
            error: Some(error),
        };
        let scope = format!("{PREFIX}{}", lease.request.notification_id);
        sqlx::query("INSERT INTO sync_scopes(account_id,scope,run_id,coverage_json,sync_json,access_denied) VALUES(?,?,?,?,?,?) ON CONFLICT(account_id,scope) DO UPDATE SET sync_json=excluded.sync_json,access_denied=excluded.access_denied")
            .bind(&account.id).bind(&scope).bind(&lease.run_id).bind(encode(&missing_coverage())?).bind(encode(&sync)?).bind(denied).execute(&mut *tx).await.map_err(storage_error)?;
        reconcile_in(&mut tx, &account).await?;
        let revision = record_change(
            &mut tx,
            &account.id,
            positive_revision(&account.authorization_epoch)?,
            &scope,
            denied,
        )
        .await?;
        tx.commit().await.map_err(storage_error)?;
        Ok(revision)
    }

    /// Native worker recovery calls this only when the intent has no live queued
    /// or dispatched owner. An abandoned third attempt cannot become a fourth.
    pub async fn exhaust_notification_subject(
        &self,
        intent: &NotificationDiscoveryIntent,
    ) -> Result<String> {
        let mut writer = self.inner.writer.lock().await;
        let mut tx = writer.begin().await.map_err(storage_error)?;
        epoch_in(&mut tx, &intent.account_id, &intent.authorization_epoch).await?;
        let changed=sqlx::query("UPDATE notification_subject_discovery SET requested=0,outcome_reason=? WHERE account_id=? AND notification_id=? AND selector_generation=? AND intent_generation=? AND attempts>=3 AND requested=1")
            .bind(encode(&NotificationSubjectReason::AttemptsExhausted)?).bind(&intent.account_id).bind(&intent.notification_id).bind(&intent.selector_generation).bind(&intent.intent_generation).execute(&mut *tx).await.map_err(storage_error)?;
        if changed.rows_affected() == 0 {
            return Err(stale());
        }
        let revision = record_change(
            &mut tx,
            &intent.account_id,
            positive_revision(&intent.authorization_epoch)?,
            &format!("{PREFIX}{}", intent.notification_id),
            false,
        )
        .await?;
        tx.commit().await.map_err(storage_error)?;
        Ok(revision)
    }
}

fn safe_fallback(current: &Current) -> Option<String> {
    // Public GitHub is the implemented adapter; future instances must own their routes.
    if current.account.provider != ProviderKind::Github || current.account.host != "github.com" {
        return None;
    }
    let path = &current.repository.full_name;
    if path.split('/').count() != 2
        || path.split('/').any(|s| {
            s.is_empty()
                || s == "."
                || s == ".."
                || !s
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b))
        })
    {
        return None;
    }
    Some(
        if let NotificationSubjectMapping::Selector(s) = &current.mapping {
            format!(
                "https://github.com/{path}/{}/{}",
                if s.kind == NotificationSubjectKind::PullRequest {
                    "pull"
                } else {
                    "issues"
                },
                s.number
            )
        } else {
            format!("https://github.com/{path}")
        },
    )
}
