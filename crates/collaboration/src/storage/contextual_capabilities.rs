//! All evidence for a contextual policy is read in the same SQLite snapshot.
use super::*;
use crate::contextual_capabilities::*;
use crate::detail::{DetailAvailability, DetailFacet, DetailValueState};
use crate::providers::{FACETS, ProviderProfile};

#[derive(Default)]
struct Evidence {
    observation: Option<CapabilityObservation>,
    reason: Option<ContextCapabilityReason>,
    recheckable: bool,
    sync_denied: bool,
    sync: SyncStatus,
    synchronize_blocked: bool,
}

struct TargetEvidence {
    repository_id: Option<String>,
    resource_saved: bool,
    parent_denied: bool,
    reason: Option<ContextCapabilityReason>,
    notified: bool,
}

impl Store {
    /// The profile factory is pure adapter metadata. It receives the account
    /// and instance captured here, rather than an earlier account snapshot.
    pub async fn contextual_capabilities(
        &self,
        request: ContextCapabilityRequest,
        profile_for: impl FnOnce(&RemoteAccount, &ProviderInstance) -> ProviderProfile,
    ) -> Result<ContextualCapabilitySnapshot> {
        validate_target(&request.target)?;
        let mut tx = self.inner.readers.begin().await.map_err(storage_error)?;
        let account = account_in(&mut tx, &request.account_id, false).await?;
        if account.authorization_epoch != request.authorization_epoch {
            return Err(stale());
        }
        let instance = identities::instance_in(&mut tx, &account).await?;
        if request
            .target
            .instance_id
            .as_ref()
            .is_some_and(|id| id != &instance.id)
        {
            return Err(CollaborationError::invalid(
                "Capability target belongs to another installation",
            ));
        }
        let target = target_evidence(&mut tx, &account, &instance, &request.target).await?;
        let profile = profile_for(&account, &instance);
        let eligibility_time = chrono::Utc::now();
        let (_, provider_sync) =
            presentation(scope_in(&mut tx, &account.id, "provider:rest").await?);
        let mut facets = Vec::with_capacity(FACETS.len());
        for facet in FACETS {
            let relevant = applicable(&request.target, facet);
            let mut evidence = if relevant && account.state == AccountState::Active {
                facet_evidence(&mut tx, &account, &request.target, &target, facet).await?
            } else {
                Evidence::default()
            };
            // Admission also checks this account-wide provider budget. Project
            // it in the same read snapshot, retaining the later local barrier.
            if future_deadline(&provider_sync, &eligibility_time).is_some_and(|deadline| {
                future_deadline(&evidence.sync, &eligibility_time)
                    .is_none_or(|local| deadline > local)
            }) {
                evidence.sync.state = SyncState::RateLimited;
                evidence.sync.next_retry_at = provider_sync.next_retry_at.clone();
                evidence.sync.error = provider_sync.error.clone();
            }
            facets.push(combine(
                &account,
                &profile,
                facet,
                relevant,
                evidence,
                &eligibility_time,
            ));
        }
        let (revision, authorization_view) = metadata(&mut tx).await?;
        tx.commit().await.map_err(storage_error)?;
        Ok(ContextualCapabilitySnapshot {
            account_id: account.id,
            authorization_epoch: account.authorization_epoch,
            instance,
            target: request.target,
            facets,
            inbox_semantics: profile.inbox_semantics,
            revision,
            authorization_view,
        })
    }
}

fn validate_target(target: &CapabilityTarget) -> Result<()> {
    let valid = match target.kind {
        CapabilityTargetKind::Account => {
            target.instance_id.is_none()
                && target.repository_id.is_none()
                && target.resource_id.is_none()
                && target.resource_kind.is_none()
        }
        CapabilityTargetKind::Repository => {
            target.instance_id.is_some()
                && target.repository_id.is_some()
                && target.resource_id.is_none()
                && target.resource_kind.is_none()
        }
        CapabilityTargetKind::Resource => {
            target.instance_id.is_some()
                && target.repository_id.is_none()
                && target.resource_id.is_some()
                && target
                    .resource_kind
                    .is_some_and(|kind| kind != ResourceKind::Repository)
        }
    };
    if !valid {
        return Err(CollaborationError::invalid(
            "Invalid capability target shape",
        ));
    }
    for id in [
        &target.instance_id,
        &target.repository_id,
        &target.resource_id,
    ]
    .into_iter()
    .flatten()
    {
        // Instance URLs are longer than opaque projection IDs.
        if id.is_empty() || id.len() > 4096 || id.chars().any(char::is_control) {
            return Err(CollaborationError::invalid(
                "Invalid capability target identity",
            ));
        }
    }
    Ok(())
}

async fn target_evidence(
    tx: &mut Transaction<'_, Sqlite>,
    account: &RemoteAccount,
    instance: &ProviderInstance,
    target: &CapabilityTarget,
) -> Result<TargetEvidence> {
    let mut evidence = TargetEvidence {
        repository_id: target.repository_id.clone(),
        resource_saved: false,
        parent_denied: false,
        reason: None,
        notified: false,
    };
    if target.kind == CapabilityTargetKind::Account {
        return Ok(evidence);
    }
    let id = target
        .repository_id
        .as_ref()
        .or(target.resource_id.as_ref())
        .expect("validated target");
    let identity: Option<(String, String)> = sqlx::query_as("SELECT kind,repository_provider_id FROM resource_identities WHERE account_id=? AND instance_id=? AND entity_id=?")
        .bind(&account.id).bind(&instance.id).bind(id).fetch_optional(&mut **tx).await.map_err(storage_error)?;
    let (kind, repository_provider_id) = identity.ok_or_else(not_found)?;
    let expected_kind = target.resource_kind.unwrap_or(ResourceKind::Repository);
    if kind != tag(&expected_kind)? {
        return Err(CollaborationError::invalid(
            "Capability target kind does not match its identity",
        ));
    }
    if account.state != AccountState::Active {
        return Ok(evidence);
    }
    if target.kind == CapabilityTargetKind::Resource {
        evidence.resource_saved =
            sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM items WHERE account_id=? AND id=?)")
                .bind(&account.id)
                .bind(id)
                .fetch_one(&mut **tx)
                .await
                .map_err(storage_error)?;
        if expected_kind != ResourceKind::Notification {
            evidence.repository_id = sqlx::query_scalar(
                "SELECT id FROM repositories WHERE account_id=? AND provider_id=?",
            )
            .bind(&account.id)
            .bind(&repository_provider_id)
            .fetch_optional(&mut **tx)
            .await
            .map_err(storage_error)?;
            if evidence.repository_id.is_none() {
                evidence.reason = Some(ContextCapabilityReason::NotObserved);
            }
        }
        if evidence.resource_saved {
            let scope = if expected_kind == ResourceKind::Notification {
                Some("notifications".to_owned())
            } else {
                evidence.repository_id.as_ref().map(|repo| {
                    repository_scope(
                        repo,
                        &if expected_kind == ResourceKind::PullRequest {
                            RemoteItemKind::PullRequest
                        } else {
                            RemoteItemKind::Issue
                        },
                    )
                })
            };
            if let Some(scope) = scope {
                let active: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM scope_membership WHERE account_id=? AND scope=? AND entity_id=? AND active=1)")
                    .bind(&account.id).bind(scope).bind(id).fetch_one(&mut **tx).await.map_err(storage_error)?;
                if !active {
                    evidence.reason = Some(ContextCapabilityReason::NotObserved);
                }
            }
        }
    }
    if expected_kind != ResourceKind::Notification
        && let Some(repository_id) = &evidence.repository_id
    {
        let selected: Option<bool> =
            sqlx::query_scalar("SELECT selected FROM repositories WHERE account_id=? AND id=?")
                .bind(&account.id)
                .bind(repository_id)
                .fetch_optional(&mut **tx)
                .await
                .map_err(storage_error)?;
        // Preserve an explicit denial even if the cache was already purged.
        let denied: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM sync_scopes s JOIN scope_membership m ON m.account_id=s.account_id AND m.scope=s.scope WHERE s.account_id=? AND s.scope='repositories' AND s.access_denied=1 AND m.entity_id=?)")
            .bind(&account.id).bind(repository_id).fetch_one(&mut **tx).await.map_err(storage_error)?;
        let mut visible = sqlx::QueryBuilder::<Sqlite>::new(
            "SELECT EXISTS(SELECT 1 FROM repositories r WHERE r.account_id=",
        );
        visible
            .push_bind(&account.id)
            .push(" AND r.id=")
            .push_bind(repository_id)
            .push(" AND ")
            .push(VISIBLE_REPOSITORY)
            .push(")");
        let visible: bool = visible
            .build_query_scalar()
            .fetch_one(&mut **tx)
            .await
            .map_err(storage_error)?;
        evidence.reason = if denied {
            Some(ContextCapabilityReason::PermissionDenied)
        } else if !visible {
            Some(ContextCapabilityReason::NotObserved)
        } else if selected != Some(true) {
            Some(ContextCapabilityReason::RepositoryNotSelected)
        } else {
            evidence.reason
        };
        if target.kind == CapabilityTargetKind::Resource {
            let kind = if expected_kind == ResourceKind::PullRequest {
                RemoteItemKind::PullRequest
            } else {
                RemoteItemKind::Issue
            };
            evidence.parent_denied = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM sync_scopes WHERE account_id=? AND scope=? AND access_denied=1)")
                .bind(&account.id).bind(repository_scope(repository_id, &kind)).fetch_one(&mut **tx).await.map_err(storage_error)?;
        }
    }
    if target.kind == CapabilityTargetKind::Resource
        && matches!(
            expected_kind,
            ResourceKind::PullRequest | ResourceKind::Issue
        )
        && super::notification_subjects::provenance_in(tx, &account.id, id).await?
        && matches!(
            evidence.reason,
            Some(
                ContextCapabilityReason::NotObserved
                    | ContextCapabilityReason::RepositoryNotSelected
            )
        )
    {
        evidence.reason = None;
        evidence.notified = true;
    }
    Ok(evidence)
}

fn applicable(target: &CapabilityTarget, facet: ResourceFacet) -> bool {
    match target.kind {
        CapabilityTargetKind::Account => matches!(
            facet,
            ResourceFacet::Repositories
                | ResourceFacet::PullRequests
                | ResourceFacet::Issues
                | ResourceFacet::Inbox
        ),
        CapabilityTargetKind::Repository => matches!(
            facet,
            ResourceFacet::Repositories | ResourceFacet::PullRequests | ResourceFacet::Issues
        ),
        CapabilityTargetKind::Resource => match target.resource_kind {
            Some(ResourceKind::PullRequest) => matches!(
                facet,
                ResourceFacet::PullRequests
                    | ResourceFacet::PullDetails
                    | ResourceFacet::Comments
                    | ResourceFacet::Reviews
                    | ResourceFacet::Checks
                    | ResourceFacet::Merge
            ),
            Some(ResourceKind::Issue) => matches!(
                facet,
                ResourceFacet::Issues | ResourceFacet::IssueDetails | ResourceFacet::Comments
            ),
            Some(ResourceKind::Notification) => facet == ResourceFacet::Inbox,
            _ => false,
        },
    }
}

async fn scope_evidence(
    tx: &mut Transaction<'_, Sqlite>,
    account_id: &str,
    scope: &str,
    count: i64,
) -> Result<Evidence> {
    let denied: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM sync_scopes WHERE account_id=? AND scope=? AND access_denied=1)")
        .bind(account_id).bind(scope).fetch_one(&mut **tx).await.map_err(storage_error)?;
    let (coverage, sync) = presentation(scope_in(tx, account_id, scope).await?);
    let observation = match coverage.state {
        CoverageState::Missing => CapabilityObservation::NotLoaded,
        CoverageState::Partial => CapabilityObservation::Partial,
        CoverageState::Complete if count == 0 => CapabilityObservation::Empty,
        CoverageState::Complete => CapabilityObservation::Complete,
    };
    Ok(Evidence {
        observation: Some(observation),
        reason: denied.then_some(ContextCapabilityReason::PermissionDenied),
        recheckable: denied,
        sync_denied: denied,
        sync,
        synchronize_blocked: false,
    })
}

async fn facet_evidence(
    tx: &mut Transaction<'_, Sqlite>,
    account: &RemoteAccount,
    target: &CapabilityTarget,
    bound: &TargetEvidence,
    facet: ResourceFacet,
) -> Result<Evidence> {
    if facet == ResourceFacet::Repositories {
        if target.kind == CapabilityTargetKind::Repository
            && matches!(
                bound.reason,
                Some(
                    ContextCapabilityReason::NotObserved
                        | ContextCapabilityReason::PermissionDenied
                )
            )
        {
            return Ok(Evidence {
                reason: bound.reason,
                ..Evidence::default()
            });
        }
        // Repository metadata can be read to enable selection; selection is
        // required for provider item reads, rather than for the picker itself.
        let count: i64 = sqlx::query_scalar("SELECT count(*) FROM repositories r JOIN scope_membership m ON m.account_id=r.account_id AND m.scope='repositories' AND m.entity_id=r.id AND m.active=1 WHERE r.account_id=?")
            .bind(&account.id).fetch_one(&mut **tx).await.map_err(storage_error)?;
        return scope_evidence(tx, &account.id, "repositories", count).await;
    }
    let kind = match facet {
        ResourceFacet::PullRequests => Some(RemoteItemKind::PullRequest),
        ResourceFacet::Issues => Some(RemoteItemKind::Issue),
        ResourceFacet::Inbox => Some(RemoteItemKind::Notification),
        _ => None,
    };
    if let Some(kind) = kind {
        if bound.notified {
            return Ok(Evidence {
                observation: Some(CapabilityObservation::Partial),
                synchronize_blocked: true,
                ..Evidence::default()
            });
        }
        if let Some(reason) = bound.reason {
            return Ok(Evidence {
                reason: Some(reason),
                ..Evidence::default()
            });
        }
        let query = ItemQuery {
            account_id: account.id.clone(),
            repository_id: bound.repository_id.clone(),
            kind: kind.clone(),
            state: None,
            search: None,
            cursor: None,
            limit: 1,
        };
        if kind != RemoteItemKind::Notification && target.kind == CapabilityTargetKind::Account {
            let selected: bool = sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM repositories WHERE account_id=? AND selected=1)",
            )
            .bind(&account.id)
            .fetch_one(&mut **tx)
            .await
            .map_err(storage_error)?;
            if !selected {
                return Ok(Evidence {
                    reason: Some(ContextCapabilityReason::RepositoryNotSelected),
                    ..Evidence::default()
                });
            }
            let mut visible = sqlx::QueryBuilder::<Sqlite>::new(
                "SELECT EXISTS(SELECT 1 FROM repositories r WHERE r.account_id=",
            );
            visible
                .push_bind(&account.id)
                .push(" AND r.selected=1 AND ")
                .push(VISIBLE_REPOSITORY)
                .push(")");
            let visible: bool = visible
                .build_query_scalar()
                .fetch_one(&mut **tx)
                .await
                .map_err(storage_error)?;
            if !visible {
                let denied: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM sync_scopes WHERE account_id=? AND scope='repositories' AND access_denied=1)")
                    .bind(&account.id).fetch_one(&mut **tx).await.map_err(storage_error)?;
                // Recheck belongs to repository discovery, not an inaccessible
                // child feed that native admission must refuse.
                return Ok(Evidence {
                    reason: Some(if denied {
                        ContextCapabilityReason::PermissionDenied
                    } else {
                        ContextCapabilityReason::NotObserved
                    }),
                    ..Evidence::default()
                });
            }
        }
        let (coverage, sync) = query_presentation(tx, &query).await?;
        let count: i64 = if target.kind == CapabilityTargetKind::Resource {
            i64::from(bound.resource_saved)
        } else {
            let mut sql = sqlx::QueryBuilder::<Sqlite>::new(
                "SELECT count(*) FROM items i WHERE i.account_id=",
            );
            sql.push_bind(&account.id)
                .push(" AND i.kind=")
                .push_bind(tag(&kind)?);
            sql.push(" AND EXISTS(SELECT 1 FROM scope_membership m WHERE m.account_id=i.account_id AND m.entity_id=i.id AND m.active=1 AND m.scope=CASE WHEN i.kind='notification' THEN 'notifications' ELSE 'repo:'||i.repository_id||':'||i.kind END) AND NOT EXISTS(SELECT 1 FROM sync_scopes s WHERE s.account_id=i.account_id AND s.scope=CASE WHEN i.kind='notification' THEN 'notifications' ELSE 'repo:'||i.repository_id||':'||i.kind END AND s.access_denied=1)");
            if kind != RemoteItemKind::Notification {
                sql.push(" AND EXISTS(SELECT 1 FROM repositories r WHERE r.account_id=i.account_id AND r.id=i.repository_id AND r.selected=1 AND ").push(VISIBLE_REPOSITORY).push(")");
            }
            if let Some(repository_id) = &bound.repository_id {
                sql.push(" AND i.repository_id=").push_bind(repository_id);
            }
            sql.build_query_scalar()
                .fetch_one(&mut **tx)
                .await
                .map_err(storage_error)?
        };
        let scope = query_scope(&query);
        let denied = if kind == RemoteItemKind::Notification || bound.repository_id.is_some() {
            sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM sync_scopes WHERE account_id=? AND scope=? AND access_denied=1)")
                .bind(&account.id).bind(scope).fetch_one(&mut **tx).await.map_err(storage_error)?
        } else {
            false
        };
        let (aggregate_denied, any_authorized) = if kind != RemoteItemKind::Notification
            && target.kind == CapabilityTargetKind::Account
        {
            let mut sql = sqlx::QueryBuilder::<Sqlite>::new(
                "SELECT count(*) AS selected,coalesce(sum(CASE WHEN s.access_denied=1 THEN 1 ELSE 0 END),0) AS denied FROM repositories r LEFT JOIN sync_scopes s ON s.account_id=r.account_id AND s.scope='repo:'||r.id||':'||",
            );
            sql.push_bind(tag(&kind)?)
                .push(" WHERE r.account_id=")
                .push_bind(&account.id)
                .push(" AND r.selected=1 AND ")
                .push(VISIBLE_REPOSITORY);
            let row = sql
                .build()
                .fetch_one(&mut **tx)
                .await
                .map_err(storage_error)?;
            let selected: i64 = row.get("selected");
            let denied: i64 = row.get("denied");
            (denied > 0, selected > denied)
        } else {
            (false, true)
        };
        return Ok(Evidence {
            reason: (denied || (aggregate_denied && !any_authorized))
                .then_some(ContextCapabilityReason::PermissionDenied),
            recheckable: denied || aggregate_denied,
            sync_denied: denied || aggregate_denied,
            sync,
            synchronize_blocked: false,
            observation: Some(
                if target.kind == CapabilityTargetKind::Resource && !bound.resource_saved {
                    CapabilityObservation::NotLoaded
                } else {
                    match coverage.state {
                        CoverageState::Missing => CapabilityObservation::NotLoaded,
                        CoverageState::Partial => CapabilityObservation::Partial,
                        CoverageState::Complete if count == 0 => CapabilityObservation::Empty,
                        CoverageState::Complete => CapabilityObservation::Complete,
                    }
                },
            ),
        });
    }
    if let Some(reason) = bound.reason {
        return Ok(Evidence {
            reason: Some(reason),
            ..Evidence::default()
        });
    }
    if bound.parent_denied {
        return Ok(Evidence {
            reason: Some(ContextCapabilityReason::PermissionDenied),
            ..Evidence::default()
        });
    }
    let detail = match facet {
        ResourceFacet::PullDetails | ResourceFacet::IssueDetails => Some(DetailFacet::Body),
        ResourceFacet::Comments => Some(DetailFacet::Comments),
        ResourceFacet::Reviews => Some(DetailFacet::Reviews),
        ResourceFacet::Checks => Some(DetailFacet::Checks),
        _ => None,
    };
    if let Some(detail) = detail
        && let Some(subject_id) = &target.resource_id
    {
        let detail = super::details::detail_evidence_in(tx, account, subject_id, detail).await?;
        return Ok(Evidence {
            reason: detail.access_reason.map(reason),
            recheckable: detail.access_reason == Some(CapabilityReason::PermissionDenied)
                && bound.resource_saved,
            sync_denied: detail.access_reason == Some(CapabilityReason::PermissionDenied),
            observation: Some(if detail.observed_state == DetailValueState::Omitted {
                CapabilityObservation::Omitted
            } else if detail.observed_state == DetailValueState::Oversized {
                CapabilityObservation::Oversized
            } else if detail.saved_empty == Some(true) {
                CapabilityObservation::Empty
            } else {
                match detail.availability {
                    DetailAvailability::Missing => CapabilityObservation::NotLoaded,
                    DetailAvailability::Partial => CapabilityObservation::Partial,
                    DetailAvailability::Ready => CapabilityObservation::Complete,
                    DetailAvailability::Unavailable => CapabilityObservation::Unknown,
                }
            }),
            sync: detail.sync,
            synchronize_blocked: false,
        });
    }
    Ok(Evidence::default())
}

fn reason(reason: CapabilityReason) -> ContextCapabilityReason {
    match reason {
        CapabilityReason::NotImplemented => ContextCapabilityReason::NotImplemented,
        CapabilityReason::ProviderSemantics => ContextCapabilityReason::ProviderSemantics,
        CapabilityReason::AdapterUnavailable => ContextCapabilityReason::AdapterUnavailable,
        CapabilityReason::AuthenticationRequired => ContextCapabilityReason::AuthenticationRequired,
        CapabilityReason::MissingScope => ContextCapabilityReason::MissingScope,
        CapabilityReason::PermissionDenied => ContextCapabilityReason::PermissionDenied,
        CapabilityReason::TemporarilyUnavailable => ContextCapabilityReason::TemporarilyUnavailable,
    }
}

fn unavailable(reason: ContextCapabilityReason) -> ContextCapabilityAccess {
    ContextCapabilityAccess {
        state: CapabilityState::Unavailable,
        reason: Some(reason),
    }
}
fn unsupported(reason: ContextCapabilityReason) -> ContextCapabilityAccess {
    ContextCapabilityAccess {
        state: CapabilityState::Unsupported,
        reason: Some(reason),
    }
}

fn combine(
    account: &RemoteAccount,
    profile: &ProviderProfile,
    facet: ResourceFacet,
    applicable: bool,
    evidence: Evidence,
    eligibility_time: &chrono::DateTime<chrono::Utc>,
) -> ContextFacetCapability {
    let declared = profile.facet(facet);
    let saved_read = if !applicable {
        unsupported(ContextCapabilityReason::NotApplicable)
    } else if account.state != AccountState::Active {
        unavailable(ContextCapabilityReason::AuthenticationRequired)
    } else if !profile
        .facets
        .iter()
        .any(|declaration| declaration.facet == facet)
    {
        unavailable(ContextCapabilityReason::NotObserved)
    } else if declared.state != CapabilityState::Supported {
        ContextCapabilityAccess {
            state: declared.state,
            reason: declared
                .reason
                .map(reason)
                .or(Some(ContextCapabilityReason::NotObserved)),
        }
    } else if let Some(reason) = evidence.reason {
        unavailable(reason)
    } else {
        ContextCapabilityAccess {
            state: CapabilityState::Supported,
            reason: None,
        }
    };
    let mut synchronize = saved_read.clone();
    if synchronize.state == CapabilityState::Supported && evidence.synchronize_blocked {
        synchronize = unavailable(ContextCapabilityReason::RepositoryNotSelected);
    }
    if synchronize.state == CapabilityState::Supported && evidence.sync_denied {
        synchronize = unavailable(ContextCapabilityReason::PermissionDenied);
    }
    if synchronize.state == CapabilityState::Supported
        && future_deadline(&evidence.sync, eligibility_time).is_some()
    {
        synchronize = unavailable(ContextCapabilityReason::TemporarilyUnavailable);
    }
    ContextFacetCapability {
        facet,
        can_recheck_access: applicable
            && account.state == AccountState::Active
            && declared.state == CapabilityState::Supported
            && evidence.recheckable,
        observation: if saved_read.state == CapabilityState::Supported {
            evidence
                .observation
                .unwrap_or(CapabilityObservation::NotLoaded)
        } else {
            CapabilityObservation::Unknown
        },
        saved_read,
        synchronize,
        remote_write: unsupported(ContextCapabilityReason::NotImplemented),
        sync: evidence.sync,
    }
}

fn future_deadline(
    sync: &SyncStatus,
    eligibility_time: &chrono::DateTime<chrono::Utc>,
) -> Option<chrono::DateTime<chrono::Utc>> {
    sync.next_retry_at
        .as_deref()
        .and_then(|date| chrono::DateTime::parse_from_rfc3339(date).ok())
        .map(|date| date.with_timezone(&chrono::Utc))
        .filter(|date| date > eligibility_time)
}
