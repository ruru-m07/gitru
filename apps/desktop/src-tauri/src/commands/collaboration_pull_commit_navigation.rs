//! Read-only, caller-bound navigation from cached pull membership to local Git.
use super::{
    collaboration::{authorize, CollaborationState, Operation},
    collaboration_local_links::{observe, CallerProof},
};
use collaboration::{
    is_canonical_commit_oid, local_links::LocalLinkState, CollaborationError, ErrorCode,
    LocalLinkQuery, PullCommitMembershipReceipt, PullCommitMembershipRequest,
};
use git::core::RepoServices;
use ipc::repo_manager::{RepoManager, RepositoryInfo};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, State, Webview};

const MAX_OPAQUE_ID_BYTES: usize = 1_024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OpenLocalPullCommitRequest {
    pub account_id: String,
    pub authorization_epoch: String,
    pub subject_id: String,
    pub commit_oid: String,
    pub facet_revision: String,
    pub local_repository_id: String,
    pub link_id: String,
    pub link_generation: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OpenLocalPullCommitReceipt {
    pub local_repository_id: String,
    pub oid: String,
}

struct ResolvedLocalCommit {
    query: LocalLinkQuery,
    repository: RepositoryInfo,
}

fn stale() -> CollaborationError {
    CollaborationError::new(
        ErrorCode::StaleView,
        "Inspect this saved commit and local clone again",
    )
}

fn unavailable() -> CollaborationError {
    CollaborationError::new(
        ErrorCode::NotFound,
        "This commit object is not available in the selected local clone",
    )
}

fn validate_request(request: &OpenLocalPullCommitRequest) -> Result<(), CollaborationError> {
    if !is_canonical_commit_oid(&request.commit_oid)
        || [
            &request.account_id,
            &request.authorization_epoch,
            &request.subject_id,
            &request.facet_revision,
            &request.local_repository_id,
            &request.link_id,
            &request.link_generation,
        ]
        .iter()
        .any(|value| {
            value.is_empty()
                || value.len() > MAX_OPAQUE_ID_BYTES
                || value.chars().any(char::is_control)
        })
    {
        return Err(CollaborationError::invalid(
            "Invalid saved commit navigation request",
        ));
    }
    Ok(())
}

fn membership_request(request: &OpenLocalPullCommitRequest) -> PullCommitMembershipRequest {
    PullCommitMembershipRequest {
        account_id: request.account_id.clone(),
        authorization_epoch: request.authorization_epoch.clone(),
        subject_id: request.subject_id.clone(),
        commit_oid: request.commit_oid.clone(),
        facet_revision: request.facet_revision.clone(),
    }
}

fn repository_matches_membership(
    repository_id: &str,
    repository_provider_id: &str,
    membership: &PullCommitMembershipReceipt,
) -> bool {
    repository_id == membership.repository_id
        || repository_provider_id == membership.context.source_repository_provider_id
}

async fn resolve_local_commit(
    request: &OpenLocalPullCommitRequest,
    membership: &PullCommitMembershipReceipt,
    app: &AppHandle,
    state: &CollaborationState,
) -> Result<ResolvedLocalCommit, CollaborationError> {
    let manager = RepoManager::new(app.clone());
    let before = manager
        .repository_for_link(&request.local_repository_id)
        .map_err(|_| stale())?
        .ok_or_else(stale)?;
    let (query, _, observation_error) = observe(app, &request.local_repository_id).await?;
    if observation_error.is_some() || query.registration_proof.is_none() {
        return Err(stale());
    }
    let repository = manager
        .repository_for_link(&request.local_repository_id)
        .map_err(|_| stale())?
        .ok_or_else(stale)?;
    if before.id != repository.id || before.path != repository.path {
        return Err(stale());
    }

    let snapshot = state
        .get()
        .await?
        .store()
        .local_link_snapshot(query.clone())
        .await?;
    if snapshot.authorization_view != membership.authorization_view {
        return Err(stale());
    }
    let linked = snapshot.links.iter().any(|link| {
        link.id == request.link_id
            && link.generation == request.link_generation
            && link.state == LocalLinkState::Linked
            && link.local_repository_id == request.local_repository_id
            && link.account_id == request.account_id
            && link.instance_id == membership.instance_id
            && repository_matches_membership(
                &link.repository_id,
                &link.repository_provider_id,
                membership,
            )
            && link.repository.is_some()
    });
    if !linked {
        return Err(stale());
    }
    Ok(ResolvedLocalCommit { query, repository })
}

#[tauri::command]
pub async fn collaboration_open_local_pull_commit(
    request: OpenLocalPullCommitRequest,
    view: Webview,
    app: AppHandle,
    state: State<'_, CollaborationState>,
) -> Result<OpenLocalPullCommitReceipt, CollaborationError> {
    authorize(&view, Operation::PullCommitNavigation)?;
    validate_request(&request)?;
    let caller = CallerProof::capture(&view, &app)?;
    let runtime = state.get().await?;
    let membership_request = membership_request(&request);
    let membership = runtime
        .store()
        .verify_pull_commit_membership(membership_request.clone())
        .await?;
    caller.validate(&view, &app)?;
    let resolved = resolve_local_commit(&request, &membership, &app, &state).await?;
    caller.validate(&view, &app)?;

    let services = RepoServices::new(&resolved.repository.path).map_err(|_| stale())?;
    services.validate_worktree().await.map_err(|_| stale())?;
    let exists = services
        .commit()
        .has_exact_commit_object(&request.commit_oid)
        .await
        .map_err(|_| stale())?;
    if !exists {
        return Err(unavailable());
    }

    // Native Git and authorization/link state can change independently. Repeat
    // all proofs before releasing an opaque navigation receipt to the renderer.
    caller.validate(&view, &app)?;
    let current_membership = runtime
        .store()
        .verify_pull_commit_membership(membership_request)
        .await?;
    if current_membership != membership {
        return Err(stale());
    }
    let current = resolve_local_commit(&request, &current_membership, &app, &state).await?;
    caller.validate(&view, &app)?;
    if current.query != resolved.query
        || current.repository.id != resolved.repository.id
        || current.repository.path != resolved.repository.path
    {
        return Err(stale());
    }

    Ok(OpenLocalPullCommitReceipt {
        local_repository_id: request.local_repository_id,
        oid: request.commit_oid,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use collaboration::PullCommitContext;

    fn request() -> OpenLocalPullCommitRequest {
        OpenLocalPullCommitRequest {
            account_id: "account".into(),
            authorization_epoch: "7".into(),
            subject_id: "github:pull:repo:67".into(),
            commit_oid: "0123456789abcdef0123456789abcdef01234567".into(),
            facet_revision: "11".into(),
            local_repository_id: "11111111-1111-4111-8111-111111111111".into(),
            link_id: "link".into(),
            link_generation: "generation".into(),
        }
    }

    #[test]
    fn request_requires_canonical_oid_and_bounded_opaque_ids() {
        assert!(validate_request(&request()).is_ok());
        let mut invalid = request();
        invalid.commit_oid = invalid.commit_oid.to_ascii_uppercase();
        assert!(validate_request(&invalid).is_err());
        let mut invalid = request();
        invalid.subject_id = "x".repeat(MAX_OPAQUE_ID_BYTES + 1);
        assert!(validate_request(&invalid).is_err());
        let mut invalid = request();
        invalid.link_id = "contains\0control".into();
        assert!(validate_request(&invalid).is_err());
    }

    #[test]
    fn navigation_accepts_only_the_exact_target_or_source_repository() {
        let membership = PullCommitMembershipReceipt {
            repository_id: "target-internal".into(),
            instance_id: "github:https://github.com/".into(),
            authorization_view: "3".into(),
            subject_id: "github:pull:repo:67".into(),
            commit_oid: "0123456789abcdef0123456789abcdef01234567".into(),
            active_generation: "generation".into(),
            facet_revision: "11".into(),
            context: PullCommitContext {
                base_oid: "1111111111111111111111111111111111111111".into(),
                head_oid: "0123456789abcdef0123456789abcdef01234567".into(),
                source_repository_provider_id: "source-provider".into(),
                metadata_facet_revision: "10".into(),
            },
        };

        assert!(repository_matches_membership(
            "target-internal",
            "target-provider",
            &membership,
        ));
        assert!(repository_matches_membership(
            "source-internal",
            "source-provider",
            &membership,
        ));
        assert!(!repository_matches_membership(
            "other-internal",
            "other-provider",
            &membership,
        ));
    }
}
