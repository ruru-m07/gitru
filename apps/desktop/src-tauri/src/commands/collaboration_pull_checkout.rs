//! Explicit, caller-bound planning for checking a saved PR head out locally.
use super::{
    collaboration::{authorize, CollaborationState, Operation},
    collaboration_local_links::{observe, CallerProof},
};
use collaboration::{
    AccountState, CollaborationError, DetailFacet, DetailFreshness, DetailQuery, DetailValueState,
    ErrorCode, LocalLinkQuery, LocalLinkVersion, MetadataField, MetadataFieldEvidence,
    PullCheckoutLinkRequest, RemoteItemKind,
};
use git::{
    core::RepoServices,
    models::{
        operation::RepoOperationKind,
        pull_checkout::{
            PullCheckoutAction, PullCheckoutBlocker, PullCheckoutError, PullCheckoutInspection,
            PullCheckoutReceipt as GitCheckoutReceipt, PullCheckoutTarget,
        },
        remotes::{RemoteEndpoint, RemoteTransport},
    },
    AppState as GitAppState,
};
use ipc::repo_manager::{RepoManager, RepositoryInfo};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    sync::Mutex,
    time::{Duration, Instant},
};
use tauri::{AppHandle, State, Webview};
use uuid::Uuid;

const PLAN_TTL: Duration = Duration::from_secs(120);
const MAX_PLANS: usize = 64;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PullCheckoutPlanRequest {
    pub account_id: String,
    pub authorization_epoch: String,
    pub instance_id: String,
    pub subject_id: String,
    pub local_repository_id: String,
    pub link_id: String,
    pub link_generation: String,
    pub local_branch: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PullCheckoutPlan {
    pub plan_id: Option<String>,
    pub local_repository_id: String,
    pub local_repository_name: String,
    pub source_repository: String,
    pub source_remote: String,
    pub source_branch: String,
    pub expected_oid: String,
    pub local_branch: String,
    pub metadata_validated_at: Option<String>,
    pub metadata_stale: bool,
    pub inspection: CheckoutPlanInspection,
}

/// IPC-owned representation. Keeping the wire enum at the command boundary
/// makes its serialized spelling part of this command's generated contract.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum CheckoutPlanAction {
    #[serde(rename = "already_checked_out")]
    AlreadyCheckedOut,
    #[serde(rename = "switch_existing")]
    SwitchExisting,
    #[serde(rename = "create_branch")]
    CreateBranch,
    #[serde(rename = "fetch_and_create_branch")]
    FetchAndCreateBranch,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum CheckoutPlanBlocker {
    #[serde(rename = "dirty_worktree")]
    DirtyWorktree,
    #[serde(rename = "active_operation")]
    ActiveOperation,
    #[serde(rename = "existing_branch_diverged")]
    ExistingBranchDiverged,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckoutPlanInspection {
    pub current_branch: Option<String>,
    pub current_head_oid: String,
    pub detached: bool,
    pub dirty: bool,
    pub operation: RepoOperationKind,
    pub target_branch_oid: Option<String>,
    pub object_available: bool,
    pub action: Option<CheckoutPlanAction>,
    pub blocker: Option<CheckoutPlanBlocker>,
}

impl From<&PullCheckoutInspection> for CheckoutPlanInspection {
    fn from(value: &PullCheckoutInspection) -> Self {
        Self {
            current_branch: value.current_branch.clone(),
            current_head_oid: value.current_head_oid.clone(),
            detached: value.detached,
            dirty: value.dirty,
            operation: value.operation.clone(),
            target_branch_oid: value.target_branch_oid.clone(),
            object_available: value.object_available,
            action: value.action.map(|action| match action {
                PullCheckoutAction::AlreadyCheckedOut => CheckoutPlanAction::AlreadyCheckedOut,
                PullCheckoutAction::SwitchExisting => CheckoutPlanAction::SwitchExisting,
                PullCheckoutAction::CreateBranch => CheckoutPlanAction::CreateBranch,
                PullCheckoutAction::FetchAndCreateBranch => {
                    CheckoutPlanAction::FetchAndCreateBranch
                }
            }),
            blocker: value.blocker.map(|blocker| match blocker {
                PullCheckoutBlocker::DirtyWorktree => CheckoutPlanBlocker::DirtyWorktree,
                PullCheckoutBlocker::ActiveOperation => CheckoutPlanBlocker::ActiveOperation,
                PullCheckoutBlocker::ExistingBranchDiverged => {
                    CheckoutPlanBlocker::ExistingBranchDiverged
                }
            }),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutePullCheckoutRequest {
    pub plan_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollaborationPullCheckoutReceipt {
    pub local_repository_id: String,
    pub branch: String,
    pub oid: String,
    pub fetched: bool,
    pub git_reported_failure: bool,
}

#[derive(Clone, PartialEq, Eq)]
struct MetadataBinding {
    authorization_view: String,
    facet_revision: Option<String>,
    head_evidence: MetadataFieldEvidence,
    base_evidence: MetadataFieldEvidence,
    head_repository_provider_id: String,
    head_repository_full_name: String,
    head_branch: String,
    head_oid: String,
    base_repository_provider_id: String,
    summary_head_oid: String,
}

#[derive(Clone, PartialEq, Eq)]
struct ResolvedCheckout {
    account_id: String,
    actor_id: String,
    authorization_epoch: String,
    instance_id: String,
    subject_id: String,
    repository_id: String,
    link: LocalLinkVersion,
    registration_proof: String,
    registration_path: String,
    local_query: LocalLinkQuery,
    metadata: MetadataBinding,
    target: PullCheckoutTarget,
}

struct CheckoutPlanEntry {
    expires: Instant,
    caller: CallerProof,
    request: PullCheckoutPlanRequest,
    resolved: ResolvedCheckout,
    inspection: PullCheckoutInspection,
}

#[derive(Default)]
pub(super) struct PullCheckoutPlans(Mutex<HashMap<String, CheckoutPlanEntry>>);

impl PullCheckoutPlans {
    fn retire(
        &self,
        caller: &CallerProof,
        subject_id: &str,
        local_repository_id: &str,
        now: Instant,
    ) -> Result<(), CollaborationError> {
        let mut plans = self.0.lock().map_err(|_| CollaborationError::storage())?;
        plans.retain(|_, plan| {
            plan.expires > now
                && !(plan.caller == *caller
                    && plan.request.subject_id == subject_id
                    && plan.request.local_repository_id == local_repository_id)
        });
        Ok(())
    }

    fn insert(&self, entry: CheckoutPlanEntry, now: Instant) -> Result<String, CollaborationError> {
        let mut plans = self.0.lock().map_err(|_| CollaborationError::storage())?;
        plans.retain(|_, plan| {
            plan.expires > now
                && !(plan.caller == entry.caller
                    && plan.request.subject_id == entry.request.subject_id
                    && plan.request.local_repository_id == entry.request.local_repository_id)
        });
        if plans.len() >= MAX_PLANS {
            return Err(CollaborationError::new(
                ErrorCode::NotReady,
                "Too many pull request checkout plans are open",
            ));
        }
        let id = uuid::Uuid::new_v4().to_string();
        plans.insert(id.clone(), entry);
        Ok(id)
    }

    fn take(
        &self,
        id: &str,
        caller: &CallerProof,
        now: Instant,
    ) -> Result<CheckoutPlanEntry, CollaborationError> {
        if id.len() > 128 {
            return Err(stale());
        }
        let mut plans = self.0.lock().map_err(|_| CollaborationError::storage())?;
        plans.retain(|_, plan| plan.expires > now);
        let plan = plans.get(id).ok_or_else(stale)?;
        if plan.caller != *caller {
            return Err(denied());
        }
        plans.remove(id).ok_or_else(stale)
    }
}

#[tauri::command]
pub async fn collaboration_plan_pull_checkout(
    request: PullCheckoutPlanRequest,
    view: Webview,
    app: AppHandle,
    state: State<'_, CollaborationState>,
) -> Result<PullCheckoutPlan, CollaborationError> {
    authorize(&view, Operation::PullCheckout)?;
    let caller = CallerProof::capture(&view, &app)?;
    validate_request(&request)?;
    state.pull_checkout_plans.retire(
        &caller,
        &request.subject_id,
        &request.local_repository_id,
        Instant::now(),
    )?;
    let runtime = state.get().await?;
    let (resolved, repository, metadata_validated_at, metadata_stale) =
        resolve(&request, &app, runtime.store()).await?;
    caller.validate(&view, &app)?;
    let services = RepoServices::new(&repository.path).map_err(|_| unavailable())?;
    services
        .validate_worktree()
        .await
        .map_err(|_| unavailable())?;
    let inspection = services
        .pull_checkout()
        .inspect(&resolved.target)
        .await
        .map_err(map_git_error)?;
    caller.validate(&view, &app)?;
    let plan_id = if inspection.action.is_some() {
        let now = Instant::now();
        Some(state.pull_checkout_plans.insert(
            CheckoutPlanEntry {
                expires: now + PLAN_TTL,
                caller,
                request: request.clone(),
                resolved: resolved.clone(),
                inspection: inspection.clone(),
            },
            now,
        )?)
    } else {
        None
    };
    Ok(PullCheckoutPlan {
        plan_id,
        local_repository_id: request.local_repository_id,
        local_repository_name: repository.name,
        source_repository: resolved.metadata.head_repository_full_name,
        source_remote: resolved.target.remote_name,
        source_branch: resolved.target.source_branch,
        expected_oid: resolved.target.expected_oid,
        local_branch: resolved.target.local_branch,
        metadata_validated_at,
        metadata_stale,
        inspection: (&inspection).into(),
    })
}

#[tauri::command]
pub async fn collaboration_execute_pull_checkout(
    request: ExecutePullCheckoutRequest,
    view: Webview,
    app: AppHandle,
    state: State<'_, CollaborationState>,
    git_state: State<'_, GitAppState>,
) -> Result<CollaborationPullCheckoutReceipt, CollaborationError> {
    authorize(&view, Operation::PullCheckout)?;
    let caller = CallerProof::capture(&view, &app)?;
    let plan = state
        .pull_checkout_plans
        .take(&request.plan_id, &caller, Instant::now())?;
    let runtime = state.get().await?;
    let current = RepoManager::new(app.clone())
        .repository_for_link(&plan.request.local_repository_id)
        .map_err(|_| stale())?
        .ok_or_else(stale)?;
    if current.path != plan.resolved.registration_path {
        return Err(stale());
    }
    let services = RepoServices::new(&current.path).map_err(|_| unavailable())?;
    services
        .validate_worktree()
        .await
        .map_err(|_| unavailable())?;
    // The nonce pins a native-only source alias; it never crosses IPC or
    // becomes a repository ref.
    let operation_id = Uuid::new_v4().to_string();
    let mut prepared = services
        .pull_checkout()
        .prepare_execution(&plan.resolved.target, &plan.inspection, &operation_id)
        .await
        .map_err(map_git_error)?;
    revalidate_authority(
        &plan,
        &caller,
        &view,
        &app,
        runtime.store(),
        prepared.worktree_paths(),
    )
    .await?;
    if prepared.needs_fetch() {
        prepared.fetch().await.map_err(map_git_error)?;
    }
    revalidate_authority(
        &plan,
        &caller,
        &view,
        &app,
        runtime.store(),
        prepared.worktree_paths(),
    )
    .await?;
    let GitCheckoutReceipt {
        branch,
        oid,
        fetched,
        git_reported_failure,
    } = {
        let result = prepared.finish().await;
        // The checkout service above has its own isolated RepoServices cache.
        // Invalidate every currently mounted context after any possible switch
        // attempt so immediate navigation cannot observe an older branch.
        let live_services = git_state
            .services
            .read()
            .await
            .values()
            .cloned()
            .collect::<Vec<_>>();
        for live in live_services {
            live.invalidate_cache();
        }
        result.map_err(map_git_error)?
    };
    Ok(CollaborationPullCheckoutReceipt {
        local_repository_id: plan.request.local_repository_id,
        branch,
        oid,
        fetched,
        git_reported_failure,
    })
}

async fn revalidate_authority(
    plan: &CheckoutPlanEntry,
    caller: &CallerProof,
    view: &Webview,
    app: &AppHandle,
    store: &collaboration::Store,
    paths: &git::service::remotes::NativeWorktreePaths,
) -> Result<(), CollaborationError> {
    caller.validate(view, app)?;
    let (resolved, repository, _, _) = resolve_with_query(
        &plan.request,
        app,
        store,
        Some(plan.resolved.local_query.clone()),
    )
    .await?;
    if resolved != plan.resolved {
        return Err(stale());
    }
    let proof =
        ipc::local_repository::registration_proof(&repository, paths).map_err(|_| stale())?;
    if proof != plan.resolved.registration_proof {
        return Err(stale());
    }
    caller.validate(view, app)
}

async fn resolve(
    request: &PullCheckoutPlanRequest,
    app: &AppHandle,
    store: &collaboration::Store,
) -> Result<(ResolvedCheckout, RepositoryInfo, Option<String>, bool), CollaborationError> {
    resolve_with_query(request, app, store, None).await
}

async fn resolve_with_query(
    request: &PullCheckoutPlanRequest,
    app: &AppHandle,
    store: &collaboration::Store,
    trusted_query: Option<LocalLinkQuery>,
) -> Result<(ResolvedCheckout, RepositoryInfo, Option<String>, bool), CollaborationError> {
    validate_request(request)?;
    let manager = RepoManager::new(app.clone());
    let repository = manager
        .repository_for_link(&request.local_repository_id)
        .map_err(|_| invalid())?
        .ok_or_else(stale)?;
    let query = match trusted_query {
        Some(query) => query,
        None => {
            let (query, _, observation_error) = observe(app, &request.local_repository_id).await?;
            if observation_error.is_some() {
                return Err(stale());
            }
            query
        }
    };
    let registration_proof = query.registration_proof.clone().ok_or_else(stale)?;
    let account = store.account(&request.account_id).await?;
    if account.state != AccountState::Active
        || account.authorization_epoch != request.authorization_epoch
    {
        return Err(stale());
    }
    let item_snapshot = store.item(&request.account_id, &request.subject_id).await?;
    let item = item_snapshot.item.ok_or_else(|| {
        CollaborationError::new(ErrorCode::NotFound, "The saved pull request is unavailable")
    })?;
    let repository_id = item.repository_id.clone().ok_or_else(invalid)?;
    if item.kind != RemoteItemKind::PullRequest {
        return Err(CollaborationError::new(
            ErrorCode::Unsupported,
            "Only saved pull requests can be checked out",
        ));
    }
    let detail = store
        .detail(DetailQuery {
            account_id: request.account_id.clone(),
            subject_id: request.subject_id.clone(),
            facet: DetailFacet::Body,
            cursor: None,
            limit: 1,
        })
        .await?;
    if item_snapshot.authorization_view != detail.authorization_view {
        return Err(stale());
    }
    let metadata = detail.metadata.ok_or_else(missing_head)?;
    let head_evidence = known_field(&metadata.fields, MetadataField::Head)?;
    let base_evidence = known_field(&metadata.fields, MetadataField::Base)?;
    let head = metadata.values.head.ok_or_else(missing_head)?;
    let base = metadata.values.base.ok_or_else(missing_head)?;
    let head_repository = head.repository.clone().ok_or_else(|| {
        CollaborationError::new(
            ErrorCode::NotReady,
            "The saved pull request source repository is unavailable",
        )
    })?;
    let base_repository = base.repository.ok_or_else(missing_head)?;
    let summary_head_oid = item.head_oid.ok_or_else(|| {
        CollaborationError::new(
            ErrorCode::NotReady,
            "Refresh the pull request head before checking it out",
        )
    })?;
    if !valid_oid(&head.oid) || summary_head_oid != head.oid {
        return Err(stale());
    }
    let source = store
        .pull_checkout_link_source(PullCheckoutLinkRequest {
            query: query.clone(),
            account_id: request.account_id.clone(),
            authorization_epoch: request.authorization_epoch.clone(),
            instance_id: request.instance_id.clone(),
            repository_id: repository_id.clone(),
            link: LocalLinkVersion {
                id: request.link_id.clone(),
                generation: request.link_generation.clone(),
            },
            head_repository: head_repository.clone(),
        })
        .await?;
    if source.authorization_view != detail.authorization_view
        || source.base_repository.provider_id != base_repository.provider_id
    {
        return Err(stale());
    }
    let local_branch = request
        .local_branch
        .clone()
        .unwrap_or_else(|| default_branch(item.number.as_deref(), &head.oid));
    let target = PullCheckoutTarget {
        remote_name: source.source_endpoint.remote_name.clone(),
        remote_ordinal: source.source_endpoint.ordinal,
        remote_endpoint: RemoteEndpoint {
            transport: match source.source_endpoint.transport {
                collaboration::LinkTransport::Https => RemoteTransport::Https,
                collaboration::LinkTransport::Ssh => RemoteTransport::Ssh,
                collaboration::LinkTransport::Scp => RemoteTransport::Scp,
            },
            host: source.source_endpoint.host,
            port: source.source_endpoint.port,
            path: source.source_endpoint.path,
        },
        remote_digest: source.remote_digest,
        source_branch: head.name.clone(),
        expected_oid: head.oid.clone(),
        local_branch,
    };
    let metadata_validated_at = head_evidence.validated_at.clone();
    let metadata_stale = detail.evidence.freshness == DetailFreshness::Stale;
    Ok((
        ResolvedCheckout {
            account_id: account.id,
            actor_id: account.actor_id,
            authorization_epoch: account.authorization_epoch,
            instance_id: request.instance_id.clone(),
            subject_id: request.subject_id.clone(),
            repository_id,
            link: LocalLinkVersion {
                id: request.link_id.clone(),
                generation: request.link_generation.clone(),
            },
            registration_proof,
            registration_path: repository.path.clone(),
            local_query: query,
            metadata: MetadataBinding {
                authorization_view: detail.authorization_view,
                facet_revision: detail.evidence.facet_revision,
                head_evidence,
                base_evidence,
                head_repository_provider_id: head_repository.provider_id,
                head_repository_full_name: head_repository.full_name,
                head_branch: head.name,
                head_oid: head.oid,
                base_repository_provider_id: base_repository.provider_id,
                summary_head_oid,
            },
            target,
        },
        repository,
        metadata_validated_at,
        metadata_stale,
    ))
}

fn known_field(
    fields: &[MetadataFieldEvidence],
    field: MetadataField,
) -> Result<MetadataFieldEvidence, CollaborationError> {
    fields
        .iter()
        .find(|evidence| evidence.field == field && evidence.saved_state == DetailValueState::Known)
        .cloned()
        .ok_or_else(missing_head)
}

fn default_branch(number: Option<&str>, oid: &str) -> String {
    let suffix = number
        .filter(|value| !value.is_empty() && value.bytes().all(|byte| byte.is_ascii_digit()))
        .unwrap_or_else(|| &oid[..12]);
    format!("pr/{suffix}")
}

fn valid_oid(value: &str) -> bool {
    matches!(value.len(), 40 | 64)
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
}

fn validate_request(request: &PullCheckoutPlanRequest) -> Result<(), CollaborationError> {
    let fields = [
        &request.account_id,
        &request.authorization_epoch,
        &request.instance_id,
        &request.subject_id,
        &request.local_repository_id,
        &request.link_id,
        &request.link_generation,
    ];
    if fields.iter().any(|value| {
        value.is_empty()
            || value.len() > 256
            || value.trim() != value.as_str()
            || value.chars().any(char::is_control)
    }) || request.local_branch.as_ref().is_some_and(|branch| {
        branch.is_empty()
            || branch.len() > 1024
            || branch.trim() != branch
            || branch.chars().any(char::is_control)
    }) {
        return Err(invalid());
    }
    Ok(())
}

fn map_git_error(error: PullCheckoutError) -> CollaborationError {
    let code = match error {
        PullCheckoutError::InvalidTarget => ErrorCode::InvalidInput,
        PullCheckoutError::RemoteChanged
        | PullCheckoutError::StalePlan
        | PullCheckoutError::HeadMoved => ErrorCode::StaleView,
        PullCheckoutError::Blocked => ErrorCode::Busy,
        PullCheckoutError::FetchFailed => ErrorCode::Network,
        PullCheckoutError::CheckoutFailed | PullCheckoutError::VerificationFailed => {
            ErrorCode::LocalStateChanged
        }
        PullCheckoutError::UnsupportedCredentials | PullCheckoutError::InspectionFailed => {
            ErrorCode::NotReady
        }
    };
    CollaborationError::new(code, error.to_string())
}

fn invalid() -> CollaborationError {
    CollaborationError::invalid("Invalid pull request checkout request")
}
fn missing_head() -> CollaborationError {
    CollaborationError::new(
        ErrorCode::NotReady,
        "Sync the pull request head and source repository before checking it out",
    )
}
fn stale() -> CollaborationError {
    CollaborationError::new(
        ErrorCode::StaleView,
        "The pull request checkout plan changed; inspect it again",
    )
}
fn denied() -> CollaborationError {
    CollaborationError::new(
        ErrorCode::PermissionDenied,
        "The requesting Gitru tab is no longer active",
    )
}
fn unavailable() -> CollaborationError {
    CollaborationError::new(
        ErrorCode::NotReady,
        "The linked local Git worktree is unavailable",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn caller(label: &str) -> CallerProof {
        CallerProof {
            native_identity: 1,
            incarnation: 1,
            label: label.into(),
            url: "tauri://localhost/app/pulls".into(),
            owners: vec![("scope".into(), 1)],
        }
    }
    fn request(subject: &str) -> PullCheckoutPlanRequest {
        PullCheckoutPlanRequest {
            account_id: "account".into(),
            authorization_epoch: "1".into(),
            instance_id: "instance".into(),
            subject_id: subject.into(),
            local_repository_id: "local".into(),
            link_id: "link".into(),
            link_generation: "1".into(),
            local_branch: None,
        }
    }
    fn entry(now: Instant, subject: &str) -> CheckoutPlanEntry {
        let endpoint = RemoteEndpoint {
            transport: RemoteTransport::Https,
            host: "example.invalid".into(),
            port: 443,
            path: "owner/repo.git".into(),
        };
        CheckoutPlanEntry {
            expires: now + PLAN_TTL,
            caller: caller("main"),
            request: request(subject),
            resolved: ResolvedCheckout {
                account_id: "account".into(),
                actor_id: "actor".into(),
                authorization_epoch: "1".into(),
                instance_id: "instance".into(),
                subject_id: subject.into(),
                repository_id: "repo".into(),
                link: LocalLinkVersion {
                    id: "link".into(),
                    generation: "1".into(),
                },
                registration_proof: "proof".into(),
                registration_path: "/fixture/repository".into(),
                local_query: LocalLinkQuery {
                    local_repository_id: "local".into(),
                    registration_proof: Some("proof".into()),
                    remote_digest: Some("a".repeat(64)),
                    endpoints: Vec::new(),
                },
                metadata: MetadataBinding {
                    authorization_view: "1".into(),
                    facet_revision: Some("1".into()),
                    head_evidence: evidence(MetadataField::Head),
                    base_evidence: evidence(MetadataField::Base),
                    head_repository_provider_id: "fork".into(),
                    head_repository_full_name: "fork/repo".into(),
                    head_branch: "feature".into(),
                    head_oid: "a".repeat(40),
                    base_repository_provider_id: "base".into(),
                    summary_head_oid: "a".repeat(40),
                },
                target: PullCheckoutTarget {
                    remote_name: "fork".into(),
                    remote_ordinal: 0,
                    remote_endpoint: endpoint,
                    remote_digest: "a".repeat(64),
                    source_branch: "feature".into(),
                    expected_oid: "a".repeat(40),
                    local_branch: "pr/1".into(),
                },
            },
            inspection: PullCheckoutInspection {
                current_branch: Some("main".into()),
                current_head_oid: "b".repeat(40),
                detached: false,
                dirty: false,
                operation: git::models::operation::RepoOperationKind::Clean,
                target_branch_oid: None,
                object_available: false,
                action: Some(git::models::pull_checkout::PullCheckoutAction::FetchAndCreateBranch),
                blocker: None,
                remote_identity: Some("c".repeat(64)),
            },
        }
    }
    fn evidence(field: MetadataField) -> MetadataFieldEvidence {
        MetadataFieldEvidence {
            field,
            saved_state: DetailValueState::Known,
            observed_state: DetailValueState::Known,
            validated_at: Some("2026-10-05T00:00:00Z".into()),
            stale_at: None,
            source: None,
        }
    }

    #[test]
    fn plan_tokens_are_single_use_caller_bound_and_replace_same_target() {
        let plans = PullCheckoutPlans::default();
        let now = Instant::now();
        let first = plans.insert(entry(now, "pr"), now).unwrap();
        let latest = plans.insert(entry(now, "pr"), now).unwrap();
        assert!(plans.take(&first, &caller("main"), now).is_err());
        assert!(plans.take(&latest, &caller("other"), now).is_err());
        assert!(plans.take(&latest, &caller("main"), now).is_ok());
        assert!(plans.take(&latest, &caller("main"), now).is_err());
    }

    #[test]
    fn blocked_replan_retires_the_prior_actionable_token() {
        let plans = PullCheckoutPlans::default();
        let now = Instant::now();
        let actionable = plans.insert(entry(now, "pr"), now).unwrap();

        plans.retire(&caller("main"), "pr", "local", now).unwrap();

        assert!(plans.take(&actionable, &caller("main"), now).is_err());
    }

    #[test]
    fn expired_plan_and_invalid_branch_input_fail_closed() {
        let plans = PullCheckoutPlans::default();
        let now = Instant::now();
        let id = plans.insert(entry(now, "pr"), now).unwrap();
        assert!(plans.take(&id, &caller("main"), now + PLAN_TTL).is_err());
        let mut value = request("pr");
        value.local_branch = Some("--upload-pack=credential".into());
        // Renderer validation is only a bound; native Git check-ref-format is
        // the final authority and rejects option-like refs during inspection.
        assert!(validate_request(&value).is_ok());
    }

    #[test]
    fn checkout_wire_uses_bounded_snake_case_states() {
        let value = CheckoutPlanInspection::from(&entry(Instant::now(), "pr").inspection);
        let json = serde_json::to_value(value).unwrap();
        assert_eq!(json["action"], "fetch_and_create_branch");
        assert!(json["blocker"].is_null());

        let blocked = CheckoutPlanInspection {
            action: None,
            blocker: Some(CheckoutPlanBlocker::ExistingBranchDiverged),
            ..CheckoutPlanInspection::from(&entry(Instant::now(), "pr").inspection)
        };
        assert_eq!(
            serde_json::to_value(blocked).unwrap()["blocker"],
            "existing_branch_diverged"
        );
        assert_eq!(
            map_git_error(PullCheckoutError::CheckoutFailed).code,
            ErrorCode::LocalStateChanged
        );
        assert_eq!(
            map_git_error(PullCheckoutError::VerificationFailed).code,
            ErrorCode::LocalStateChanged
        );
    }

    #[test]
    fn retained_known_metadata_remains_eligible_after_an_omitted_observation() {
        let mut retained = evidence(MetadataField::Head);
        retained.observed_state = DetailValueState::Omitted;
        retained.stale_at = Some("2026-10-05T00:00:00Z".into());
        assert_eq!(
            known_field(&[retained.clone()], MetadataField::Head).unwrap(),
            retained
        );

        retained.saved_state = DetailValueState::NotLoaded;
        assert!(known_field(&[retained], MetadataField::Head).is_err());
    }

    #[test]
    fn provider_object_ids_must_already_be_canonical_lowercase() {
        assert!(valid_oid(&"a1".repeat(20)));
        assert!(valid_oid(&"a1".repeat(32)));
        assert!(!valid_oid(&"A1".repeat(20)));
        assert!(!valid_oid(&"A1".repeat(32)));
    }
}
