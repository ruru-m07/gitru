//! Native caller-owned ephemeral interest; renewal never submits refresh intent.
use super::*;

pub(super) const LEASE_SECONDS: u32 = 45;
pub(super) const RENEW_SECONDS: u32 = 15;
const MAX_LEASES: usize = 128;
const MAX_OWNER_LEASES: usize = 16;
const MAX_OWNERS: usize = 128;

#[derive(Clone)]
pub(super) struct DemandLease {
    pub owner: String,
    pub generation: String,
    pub account: RemoteAccount,
    pub target: DemandTarget,
    pub expires: Instant,
    pub repository_cursor: Option<String>,
}

#[derive(Default)]
pub(super) struct Demands {
    pub owners: HashMap<String, DemandOwnerActivity>,
    pub leases: HashMap<String, DemandLease>,
    sequence: u64,
    pub admission_cursor: usize,
    pub coverage_attempted: std::collections::HashSet<String>,
}

impl Demands {
    fn generation(&mut self) -> Result<String, CollaborationError> {
        self.sequence = self.sequence.checked_add(1).ok_or_else(busy)?;
        Ok(self.sequence.to_string())
    }
    pub fn expire(&mut self, now: Instant) {
        self.leases.retain(|_, lease| lease.expires > now);
    }
    fn activity(&mut self, owner: &str) -> Result<DemandOwnerActivity, CollaborationError> {
        if owner.is_empty() || owner.len() > 256 || owner.chars().any(char::is_control) {
            return Err(CollaborationError::invalid("Invalid native demand owner"));
        }
        if let Some(activity) = self.owners.get(owner) {
            return Ok(activity.clone());
        }
        if self.owners.len() >= MAX_OWNERS {
            return Err(busy());
        }
        let activity = DemandOwnerActivity {
            generation: self.generation()?,
            active: false,
        };
        self.owners.insert(owner.into(), activity.clone());
        Ok(activity)
    }
    fn require_owner(&self, owner: &str, generation: &str) -> Result<(), CollaborationError> {
        if self
            .owners
            .get(owner)
            .is_none_or(|activity| !activity.active || activity.generation != generation)
        {
            return Err(stale());
        }
        Ok(())
    }
    pub fn interested(&self, job: &Job) -> bool {
        self.leases.values().any(|lease| {
            lease.account.id == job.account.id
                && lease.account.authorization_epoch == job.account.authorization_epoch
                && match (lease.target.kind, &job.kind) {
                    (DemandTargetKind::Repositories, JobKind::Feed(FeedKind::Repositories))
                    | (DemandTargetKind::Inbox, JobKind::Feed(FeedKind::Notifications)) => true,
                    (DemandTargetKind::PullRequests, JobKind::Feed(FeedKind::PullRequests))
                    | (DemandTargetKind::Issues, JobKind::Feed(FeedKind::Issues)) => {
                        lease.target.repository_id.as_ref().is_none_or(|id| {
                            job.repository.as_ref().is_some_and(|repo| &repo.id == id)
                        })
                    }
                    (DemandTargetKind::Detail, JobKind::Detail { subject_id, facet }) => {
                        lease.target.subject_id.as_ref() == Some(subject_id)
                            && lease.target.facet == Some(*facet)
                    }
                    _ => false,
                }
        })
    }
}

fn busy() -> CollaborationError {
    CollaborationError::new(ErrorCode::Busy, "Foreground collaboration demand is full")
}
fn foreign() -> CollaborationError {
    CollaborationError::new(
        ErrorCode::PermissionDenied,
        "Demand lease belongs to another view",
    )
}
fn receipt(id: &str, generation: &str) -> DemandLeaseReceipt {
    DemandLeaseReceipt {
        lease_id: id.into(),
        owner_generation: generation.into(),
        expires_in_seconds: LEASE_SECONDS,
        renew_after_seconds: RENEW_SECONDS,
    }
}

impl CollaborationRuntime {
    pub(super) async fn ensure_demand_dispatch(&self, job: &Job) -> Result<(), CollaborationError> {
        self.prune_demands().await?;
        let scheduler = self.scheduler.lock().await;
        let automatic = job.reason == scheduler::Admission::Foreground
            && !scheduler.explicit_keys.contains(&job.key);
        if automatic && !scheduler.demands.interested(job) {
            return Err(stale());
        }
        drop(scheduler);
        if automatic {
            let target = match &job.kind {
                JobKind::NotificationSubject { .. } => return Err(stale()),
                JobKind::Detail { subject_id, facet } => DemandTarget {
                    kind: DemandTargetKind::Detail,
                    repository_id: None,
                    subject_id: Some(subject_id.clone()),
                    facet: Some(*facet),
                },
                JobKind::Feed(kind) => DemandTarget {
                    kind: match kind {
                        FeedKind::Repositories => DemandTargetKind::Repositories,
                        FeedKind::Notifications => DemandTargetKind::Inbox,
                        FeedKind::PullRequests => DemandTargetKind::PullRequests,
                        FeedKind::Issues => DemandTargetKind::Issues,
                    },
                    repository_id: job
                        .repository
                        .as_ref()
                        .map(|repository| repository.id.clone()),
                    subject_id: None,
                    facet: None,
                },
            };
            self.validate_demand(&job.account.id, &job.account.authorization_epoch, &target)
                .await?;
        }
        Ok(())
    }

    pub fn with_demand_visibility_probe(
        mut self,
        probe: std::sync::Arc<dyn Fn(&str) -> bool + Send + Sync>,
    ) -> Self {
        self.demand_visibility = probe;
        self
    }

    pub(super) async fn prune_demands(&self) -> Result<(), CollaborationError> {
        let owners: Vec<_> = self
            .scheduler
            .lock()
            .await
            .demands
            .owners
            .iter()
            .filter(|(_, activity)| activity.active)
            .map(|(owner, activity)| (owner.clone(), activity.generation.clone()))
            .collect();
        // Probe native state outside the scheduler lock; it may inspect host locks.
        let hidden: Vec<_> = owners
            .into_iter()
            .filter(|(owner, _)| !(self.demand_visibility)(owner))
            .collect();
        let mut scheduler = self.scheduler.lock().await;
        scheduler.demands.expire(self.now());
        for (owner, generation) in hidden {
            if scheduler
                .demands
                .owners
                .get(&owner)
                .is_some_and(|activity| activity.active && activity.generation == generation)
            {
                let generation = scheduler.demands.generation()?;
                scheduler.demands.owners.insert(
                    owner.clone(),
                    DemandOwnerActivity {
                        generation,
                        active: false,
                    },
                );
                scheduler
                    .demands
                    .leases
                    .retain(|_, lease| lease.owner != owner);
            }
        }
        Ok(())
    }

    pub(super) async fn enqueue_foreground(&self) -> Result<(), CollaborationError> {
        self.prune_demands().await?;
        let mut leases = {
            let mut scheduler = self.scheduler.lock().await;
            scheduler.demands.expire(self.now());
            let mut leases: Vec<_> = scheduler
                .demands
                .leases
                .iter()
                .map(|(id, lease)| (id.clone(), lease.clone()))
                .collect();
            leases.sort_by(|a, b| a.0.cmp(&b.0));
            if !leases.is_empty() {
                let start = scheduler.demands.admission_cursor % leases.len();
                leases.rotate_left(start);
                scheduler.demands.admission_cursor = (start + 1) % leases.len();
            }
            let alive: std::collections::HashSet<_> = leases
                .iter()
                .filter(|(_, lease)| lease.target.kind == DemandTargetKind::Detail)
                .map(|(_, lease)| {
                    format!(
                        "{}:{}:{}",
                        lease.account.id,
                        lease.account.authorization_epoch,
                        lease.target.facet.expect("validated facet").scope(
                            lease
                                .target
                                .subject_id
                                .as_deref()
                                .expect("validated subject")
                        )
                    )
                })
                .collect();
            scheduler
                .demands
                .coverage_attempted
                .retain(|key| alive.contains(key));
            leases
        };
        for (id, lease) in leases.drain(..) {
            if let Err(error) = self
                .validate_demand(
                    &lease.account.id,
                    &lease.account.authorization_epoch,
                    &lease.target,
                )
                .await
            {
                if matches!(
                    error.code,
                    ErrorCode::AuthRequired
                        | ErrorCode::PermissionDenied
                        | ErrorCode::Unsupported
                        | ErrorCode::StaleView
                        | ErrorCode::NotFound
                ) {
                    self.scheduler.lock().await.demands.leases.remove(&id);
                    continue;
                }
                return Err(error);
            }
            let target = lease.target;
            let jobs = match target.kind {
                DemandTargetKind::Detail => {
                    let subject_id = target.subject_id.ok_or_else(stale)?;
                    let facet = target.facet.ok_or_else(stale)?;
                    let subject = self
                        .store
                        .detail_subject(&lease.account.id, &subject_id)
                        .await?;
                    let repository = self
                        .store
                        .repository(
                            &lease.account.id,
                            subject.repository_id.as_deref().ok_or_else(stale)?,
                        )
                        .await?;
                    vec![(
                        Some(repository),
                        JobKind::Detail {
                            subject_id: subject_id.clone(),
                            facet,
                        },
                        facet.scope(&subject_id),
                    )]
                }
                DemandTargetKind::Repositories | DemandTargetKind::Inbox => {
                    let kind = if target.kind == DemandTargetKind::Repositories {
                        FeedKind::Repositories
                    } else {
                        FeedKind::Notifications
                    };
                    vec![(None, JobKind::Feed(kind), scope_name(kind, None))]
                }
                DemandTargetKind::PullRequests | DemandTargetKind::Issues => {
                    let kind = if target.kind == DemandTargetKind::PullRequests {
                        FeedKind::PullRequests
                    } else {
                        FeedKind::Issues
                    };
                    let repositories = if let Some(id) = target.repository_id {
                        vec![self.store.repository(&lease.account.id, &id).await?]
                    } else {
                        let repositories = self
                            .store
                            .demand_repositories(
                                &lease.account.id,
                                lease.repository_cursor.as_deref(),
                            )
                            .await?;
                        if let Some(current) =
                            self.scheduler.lock().await.demands.leases.get_mut(&id)
                        {
                            current.repository_cursor =
                                repositories.last().map(|repository| repository.id.clone());
                        }
                        repositories
                    };
                    repositories
                        .into_iter()
                        .filter(|repository| repository.selected)
                        .map(|repository| {
                            let scope = scope_name(kind, Some(&repository));
                            (Some(repository), JobKind::Feed(kind), scope)
                        })
                        .collect()
                }
            };
            for (repository, kind, scope) in jobs {
                match self
                    .enqueue_work_reason(
                        lease.account.clone(),
                        repository,
                        kind,
                        scope,
                        scheduler::Admission::Foreground,
                    )
                    .await
                {
                    Ok(_) => {}
                    Err(error) if error.code == ErrorCode::Busy => break,
                    Err(error) => return Err(error),
                }
            }
        }
        Ok(())
    }

    pub async fn demand_owner_activity(
        &self,
        owner: &str,
    ) -> Result<DemandOwnerActivity, CollaborationError> {
        self.prune_demands().await?;
        self.scheduler.lock().await.demands.activity(owner)
    }

    pub async fn set_demand_owner_activity(
        &self,
        owner: &str,
        expected_generation: &str,
        active: bool,
    ) -> Result<DemandOwnerActivity, CollaborationError> {
        let mut scheduler = self.scheduler.lock().await;
        let old = scheduler.demands.activity(owner)?;
        if old.generation != expected_generation {
            return Err(stale());
        }
        if old.active == active {
            return Ok(old);
        }
        let activity = DemandOwnerActivity {
            generation: scheduler.demands.generation()?,
            active,
        };
        scheduler
            .demands
            .leases
            .retain(|_, lease| lease.owner != owner);
        scheduler
            .demands
            .owners
            .insert(owner.into(), activity.clone());
        drop(scheduler);
        self.notify.notify_one();
        Ok(activity)
    }

    pub async fn dispose_demand_owner(
        &self,
        owner: &str,
        expected_generation: &str,
    ) -> Result<(), CollaborationError> {
        let mut scheduler = self.scheduler.lock().await;
        let Some(activity) = scheduler.demands.owners.get(owner) else {
            return Ok(());
        };
        if activity.generation != expected_generation {
            return Err(stale());
        }
        scheduler.demands.owners.remove(owner);
        scheduler
            .demands
            .leases
            .retain(|_, lease| lease.owner != owner);
        drop(scheduler);
        self.notify.notify_one();
        Ok(())
    }

    pub async fn acquire_demand(
        &self,
        owner: &str,
        request: AcquireDemandRequest,
    ) -> Result<DemandLeaseReceipt, CollaborationError> {
        self.prune_demands().await?;
        let _lifecycle = self.lifecycle.lock().await;
        {
            let mut scheduler = self.scheduler.lock().await;
            scheduler.demands.expire(self.now());
            scheduler
                .demands
                .require_owner(owner, &request.owner_generation)?;
        }
        let account = self
            .validate_demand(
                &request.account_id,
                &request.authorization_epoch,
                &request.target,
            )
            .await?;
        let mut scheduler = self.scheduler.lock().await;
        scheduler.demands.expire(self.now());
        scheduler
            .demands
            .require_owner(owner, &request.owner_generation)?;
        if scheduler.demands.leases.len() >= MAX_LEASES
            || scheduler
                .demands
                .leases
                .values()
                .filter(|lease| lease.owner == owner)
                .count()
                >= MAX_OWNER_LEASES
        {
            return Err(busy());
        }
        let id = uuid::Uuid::new_v4().to_string();
        scheduler.demands.leases.insert(
            id.clone(),
            DemandLease {
                owner: owner.into(),
                generation: request.owner_generation.clone(),
                account,
                target: request.target,
                expires: self.now() + Duration::from_secs(u64::from(LEASE_SECONDS)),
                repository_cursor: None,
            },
        );
        drop(scheduler);
        self.notify.notify_one();
        Ok(receipt(&id, &request.owner_generation))
    }

    pub async fn renew_demand(
        &self,
        owner: &str,
        request: RenewDemandRequest,
    ) -> Result<DemandRenewalReceipt, CollaborationError> {
        self.prune_demands().await?;
        if request.leases.is_empty()
            || request.leases.len() > MAX_OWNER_LEASES
            || request.leases.iter().enumerate().any(|(index, lease)| {
                request.leases[..index]
                    .iter()
                    .any(|other| other.lease_id == lease.lease_id)
            })
        {
            return Err(CollaborationError::invalid("Invalid demand renewal batch"));
        }
        let _lifecycle = self.lifecycle.lock().await;
        let leases = {
            let mut scheduler = self.scheduler.lock().await;
            scheduler.demands.expire(self.now());
            scheduler
                .demands
                .require_owner(owner, &request.owner_generation)?;
            request
                .leases
                .iter()
                .map(|request| {
                    let lease = scheduler
                        .demands
                        .leases
                        .get(&request.lease_id)
                        .ok_or_else(stale)?;
                    if lease.owner != owner {
                        return Err(foreign());
                    }
                    if lease.generation != scheduler.demands.owners[owner].generation
                        || lease.account.id != request.account_id
                        || lease.account.authorization_epoch != request.authorization_epoch
                    {
                        return Err(stale());
                    }
                    Ok(lease.clone())
                })
                .collect::<Result<Vec<_>, CollaborationError>>()?
        };
        for lease in &leases {
            self.validate_demand(
                &lease.account.id,
                &lease.account.authorization_epoch,
                &lease.target,
            )
            .await?;
        }
        let mut scheduler = self.scheduler.lock().await;
        scheduler.demands.expire(self.now());
        scheduler
            .demands
            .require_owner(owner, &request.owner_generation)?;
        if request
            .leases
            .iter()
            .any(|lease| !scheduler.demands.leases.contains_key(&lease.lease_id))
        {
            return Err(stale());
        }
        let expires = self.now() + Duration::from_secs(u64::from(LEASE_SECONDS));
        for request in &request.leases {
            scheduler
                .demands
                .leases
                .get_mut(&request.lease_id)
                .ok_or_else(stale)?
                .expires = expires;
        }
        drop(scheduler);
        self.notify.notify_one();
        Ok(DemandRenewalReceipt {
            leases: request
                .leases
                .iter()
                .map(|lease| receipt(&lease.lease_id, &request.owner_generation))
                .collect(),
        })
    }

    pub async fn release_demand(
        &self,
        owner: &str,
        request: ReleaseDemandRequest,
    ) -> Result<(), CollaborationError> {
        let mut scheduler = self.scheduler.lock().await;
        if let Some(lease) = scheduler.demands.leases.get(&request.lease_id)
            && lease.owner != owner
        {
            return Err(foreign());
        }
        scheduler.demands.leases.remove(&request.lease_id);
        drop(scheduler);
        self.notify.notify_one();
        Ok(())
    }

    pub(super) async fn validate_demand(
        &self,
        account_id: &str,
        epoch: &str,
        target: &DemandTarget,
    ) -> Result<RemoteAccount, CollaborationError> {
        for id in [
            Some(account_id),
            Some(epoch),
            target.repository_id.as_deref(),
            target.subject_id.as_deref(),
        ]
        .into_iter()
        .flatten()
        {
            if id.is_empty() || id.len() > 256 || id.chars().any(char::is_control) {
                return Err(CollaborationError::invalid(
                    "Invalid foreground demand identity",
                ));
            }
        }
        let shape = match target.kind {
            DemandTargetKind::Repositories | DemandTargetKind::Inbox => {
                target.repository_id.is_none()
                    && target.subject_id.is_none()
                    && target.facet.is_none()
            }
            DemandTargetKind::PullRequests | DemandTargetKind::Issues => {
                target.subject_id.is_none() && target.facet.is_none()
            }
            DemandTargetKind::Detail => {
                target.repository_id.is_none()
                    && target.subject_id.is_some()
                    && target.facet.is_some()
            }
        };
        if !shape {
            return Err(CollaborationError::invalid(
                "Invalid foreground demand target shape",
            ));
        }
        let account = self.active_account(account_id).await?;
        if account.authorization_epoch != epoch {
            return Err(stale());
        }
        let instance = self.store.provider_instance(account_id).await?;
        let (context_target, facet) = match target.kind {
            DemandTargetKind::Detail => {
                let subject = self
                    .store
                    .detail_subject(account_id, target.subject_id.as_deref().ok_or_else(stale)?)
                    .await?;
                let detail = target.facet.ok_or_else(stale)?;
                self.require_detail(&account, &subject, detail).await?;
                (
                    CapabilityTarget {
                        kind: CapabilityTargetKind::Resource,
                        instance_id: Some(instance.id),
                        repository_id: None,
                        resource_id: Some(subject.id),
                        resource_kind: Some(if subject.kind == RemoteItemKind::PullRequest {
                            ResourceKind::PullRequest
                        } else {
                            ResourceKind::Issue
                        }),
                    },
                    detail.capability(&subject.kind).ok_or_else(unsupported)?,
                )
            }
            _ => {
                let facet = match target.kind {
                    DemandTargetKind::Repositories => ResourceFacet::Repositories,
                    DemandTargetKind::Inbox => ResourceFacet::Inbox,
                    DemandTargetKind::PullRequests => ResourceFacet::PullRequests,
                    DemandTargetKind::Issues => ResourceFacet::Issues,
                    DemandTargetKind::Detail => unreachable!(),
                };
                (
                    if let Some(repository_id) = &target.repository_id {
                        CapabilityTarget {
                            kind: CapabilityTargetKind::Repository,
                            instance_id: Some(instance.id),
                            repository_id: Some(repository_id.clone()),
                            resource_id: None,
                            resource_kind: None,
                        }
                    } else {
                        CapabilityTarget {
                            kind: CapabilityTargetKind::Account,
                            instance_id: None,
                            repository_id: None,
                            resource_id: None,
                            resource_kind: None,
                        }
                    },
                    facet,
                )
            }
        };
        let context = self
            .contextual_capabilities(ContextCapabilityRequest {
                account_id: account_id.into(),
                authorization_epoch: epoch.into(),
                target: context_target,
            })
            .await?;
        let capability = context
            .facets
            .iter()
            .find(|capability| capability.facet == facet)
            .ok_or_else(unsupported)?;
        if capability.saved_read.state == CapabilityState::Unsupported {
            return Err(unsupported());
        }
        if capability.saved_read.state != CapabilityState::Supported {
            return Err(CollaborationError::new(
                ErrorCode::PermissionDenied,
                "Foreground demand is unavailable for this resource",
            ));
        }
        Ok(account)
    }
}
