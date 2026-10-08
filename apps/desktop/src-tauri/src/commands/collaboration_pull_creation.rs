//! Native registration/ref observations for explicit online PR creation.
use super::{
    collaboration::{CollaborationState, Operation, authorize},
    collaboration_local_links::{CallerProof, observe},
};
use collaboration::{
    CollaborationError, ErrorCode, LocalLinkState, LocalLinkVersion, PreviewPullCreationRequest,
    PullCreationLocalObservation, PullCreationOwner, PullCreationPreview, PullDraftKey,
    PullDraftPage, PullDraftQuery, PullDraftSnapshot, PullSubmissionReceipt, SavePullDraftRequest,
    SubmitPullRequest,
};
use git::{core::RepoServices, service::remotes::NativeWorktreePaths};
use ipc::{
    local_repository::registration_proof,
    repo_manager::{RepoManager, RepositoryInfo},
};
use std::sync::Arc;
use tauri::{AppHandle, State, Webview};

#[tauri::command]
pub async fn collaboration_pull_draft(
    key: PullDraftKey,
    view: Webview,
    state: State<'_, CollaborationState>,
) -> Result<PullDraftSnapshot, CollaborationError> {
    authorize(&view, Operation::PullCreation)?;
    state.get().await?.pull_draft(key).await
}
#[tauri::command]
pub async fn collaboration_pull_drafts(
    query: PullDraftQuery,
    view: Webview,
    state: State<'_, CollaborationState>,
) -> Result<PullDraftPage, CollaborationError> {
    authorize(&view, Operation::PullCreation)?;
    state.get().await?.pull_drafts(query).await
}
#[tauri::command]
pub async fn collaboration_save_pull_draft(
    request: SavePullDraftRequest,
    view: Webview,
    state: State<'_, CollaborationState>,
) -> Result<PullDraftSnapshot, CollaborationError> {
    authorize(&view, Operation::PullCreation)?;
    state.get().await?.save_pull_draft(request).await
}
#[tauri::command]
pub async fn collaboration_preview_pull_creation(
    request: PreviewPullCreationRequest,
    view: Webview,
    app: AppHandle,
    state: State<'_, CollaborationState>,
) -> Result<PullCreationPreview, CollaborationError> {
    authorize(&view, Operation::PullCreation)?;
    let caller = CallerProof::capture(&view, &app)?;
    let (observation, registration) = local_observation(&request, &app, &state).await?;
    caller.validate(&view, &app)?;
    let owner = owner(&caller, &app, Some(registration))?;
    let result = state
        .get()
        .await?
        .preview_pull_creation(request, observation, owner)
        .await?;
    caller.validate(&view, &app)?;
    Ok(result)
}
#[tauri::command]
pub async fn collaboration_submit_pull(
    request: SubmitPullRequest,
    view: Webview,
    app: AppHandle,
    state: State<'_, CollaborationState>,
) -> Result<PullSubmissionReceipt, CollaborationError> {
    authorize(&view, Operation::PullCreation)?;
    let caller = CallerProof::capture(&view, &app)?;
    let runtime = state.get().await?;
    // Receipt recovery neither needs a live grant nor new Git/network authority.
    match runtime
        .submit_pull(request.clone(), None, owner(&caller, &app, None)?)
        .await
    {
        Ok(receipt) => {
            caller.validate(&view, &app)?;
            return Ok(receipt);
        }
        Err(error) if error.code == ErrorCode::NotReady => {}
        Err(error) => return Err(error),
    }
    let r = PreviewPullCreationRequest {
        key: request.context.key.clone(),
        draft_generation: request.context.draft_generation.clone(),
        authorization_epoch: request.context.authorization_epoch.clone(),
        authorization_view: request.context.authorization_view.clone(),
    };
    let (observation, registration) = local_observation(&r, &app, &state).await?;
    caller.validate(&view, &app)?;
    let receipt = runtime
        .submit_pull(
            request,
            Some(observation),
            owner(&caller, &app, Some(registration))?,
        )
        .await?;
    caller.validate(&view, &app)?;
    Ok(receipt)
}

#[derive(Clone)]
struct Registration {
    repository: RepositoryInfo,
    proof: String,
}
fn stale() -> CollaborationError {
    CollaborationError::new(
        ErrorCode::StaleView,
        "Local repository, source branch or account changed; preview again",
    )
}
fn owner(
    caller: &CallerProof,
    app: &AppHandle,
    registration: Option<Registration>,
) -> Result<PullCreationOwner, CollaborationError> {
    // The native registry rotates incarnation on navigation/recreation. Its exact
    // callback also rechecks every context owner generation; no renderer token.
    let identity = format!(
        "{}:{}:{}:{}",
        caller.label.len(),
        caller.label,
        caller.native_identity,
        caller.incarnation
    );
    let caller = caller.clone();
    let app = app.clone();
    PullCreationOwner::new(
        identity,
        Arc::new(move || {
            caller.validate_under_writer(&app)?;
            if let Some(expected) = &registration {
                let current = RepoManager::new(app.clone())
                    .repository_for_link(&expected.repository.id)
                    .map_err(|_| stale())?
                    .ok_or_else(stale)?;
                validate_registration(expected, &current)?;
            }
            Ok(())
        }),
    )
}
fn validate_registration(
    expected: &Registration,
    current: &RepositoryInfo,
) -> Result<(), CollaborationError> {
    if current.id != expected.repository.id || current.path != expected.repository.path {
        return Err(stale());
    }
    // Rediscover the current .git pointer, not only the previously captured paths.
    // This is local filesystem work only: no Git child/UI dispatch while writer-held.
    let paths = RepoServices::new(&current.path)
        .map_err(|_| stale())?
        .watch_paths()
        .map_err(|_| stale())?;
    let paths = NativeWorktreePaths {
        worktree: paths.worktree,
        git_dir: paths.git_dir,
        common_dir: paths.common_dir,
    };
    if registration_proof(current, &paths).map_err(|_| stale())? != expected.proof {
        return Err(stale());
    }
    Ok(())
}
async fn local_observation(
    r: &PreviewPullCreationRequest,
    app: &AppHandle,
    state: &CollaborationState,
) -> Result<(PullCreationLocalObservation, Registration), CollaborationError> {
    let runtime = state.get().await?;
    // Bounded native DTO validation precedes any registration or path lookup.
    let draft = runtime.pull_draft(r.key.clone()).await?;
    if draft.generation != r.draft_generation
        || draft.authorization_view != r.authorization_view
        || !draft.can_preview
    {
        return Err(stale());
    }
    let account = runtime.store().account(&r.key.account_id).await?;
    if account.authorization_epoch != r.authorization_epoch {
        return Err(stale());
    }
    let v = &draft.values;
    let manager = RepoManager::new(app.clone());
    let repository = manager
        .repository_for_link(&v.local_repository_id)
        .map_err(|_| stale())?
        .ok_or_else(stale)?;
    let (before, _, error) = observe(app, &v.local_repository_id).await?;
    if error.is_some() {
        return Err(stale());
    }
    let inspection = RepoServices::new(&repository.path)
        .map_err(|_| stale())?
        .pull_creation()
        .inspect(&v.source_branch)
        .await
        .map_err(|_| stale())?;
    let proof = registration_proof(&repository, &inspection.paths).map_err(|_| stale())?;
    if before.registration_proof.as_deref() != Some(proof.as_str()) {
        return Err(stale());
    }
    let (after, _, error) = observe(app, &v.local_repository_id).await?;
    if error.is_some() || before != after {
        return Err(stale());
    }
    let links = runtime.store().local_link_snapshot(after.clone()).await?;
    if links.authorization_view != r.authorization_view
        || !links.links.iter().any(|link| {
            link.id == v.link_id
                && link.generation == v.link_generation
                && link.local_repository_id == v.local_repository_id
                && link.account_id == r.key.account_id
                && link.repository_id == r.key.repository_id
                && link.state == LocalLinkState::Linked
                && link.repository.is_some()
        })
    {
        return Err(stale());
    }
    let registration = Registration { repository, proof };
    let current = manager
        .repository_for_link(&v.local_repository_id)
        .map_err(|_| stale())?
        .ok_or_else(stale)?;
    validate_registration(&registration, &current)?;
    Ok((
        PullCreationLocalObservation {
            query: after,
            link: LocalLinkVersion {
                id: v.link_id.clone(),
                generation: v.link_generation.clone(),
            },
            source_branch: inspection.source_branch,
            source_oid: inspection.source_oid,
        },
        registration,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{path::Path, process::Command};
    fn init(path: &Path) {
        assert!(
            Command::new("git")
                .args(["init", "-b", "main"])
                .arg(path)
                .output()
                .unwrap()
                .status
                .success()
        );
    }
    fn info(path: &Path) -> RepositoryInfo {
        RepositoryInfo {
            id: "registered".into(),
            name: "Local".into(),
            path: path.to_str().unwrap().into(),
            origin: None,
            current_branch: None,
            ahead_behind: None,
            has_uncommitted_changes: false,
            last_updated: 0,
        }
    }
    fn expected(repo: RepositoryInfo) -> Registration {
        let p = RepoServices::new(&repo.path)
            .unwrap()
            .watch_paths()
            .unwrap();
        let paths = NativeWorktreePaths {
            worktree: p.worktree,
            git_dir: p.git_dir,
            common_dir: p.common_dir,
        };
        Registration {
            proof: registration_proof(&repo, &paths).unwrap(),
            repository: repo,
        }
    }
    #[test]
    fn owner_registration_rejects_path_and_directory_replacement() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("repo");
        init(&path);
        let current = info(&path);
        let saved = expected(current.clone());
        validate_registration(&saved, &current).unwrap();
        let mut display_rename = current.clone();
        display_rename.name = "renamed".into();
        validate_registration(&saved, &display_rename).unwrap();
        let mut wrong_path = current.clone();
        wrong_path.path = root.path().join("elsewhere").to_str().unwrap().into();
        assert!(validate_registration(&saved, &wrong_path).is_err());
        std::fs::rename(&path, root.path().join("retained")).unwrap();
        init(&path);
        assert!(validate_registration(&saved, &current).is_err());
    }
    #[test]
    fn owner_rediscovers_changed_git_pointer_even_if_original_directories_remain() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("repo");
        let metadata = root.path().join("original-metadata");
        assert!(
            Command::new("git")
                .arg("init")
                .arg("--separate-git-dir")
                .arg(&metadata)
                .arg(&path)
                .output()
                .unwrap()
                .status
                .success()
        );
        let current = info(&path);
        let saved = expected(current.clone());
        let alternate = root.path().join("alternate");
        init(&alternate);
        std::fs::write(
            path.join(".git"),
            format!("gitdir: {}\n", alternate.join(".git").display()),
        )
        .unwrap();
        assert!(metadata.is_dir());
        assert!(validate_registration(&saved, &current).is_err());
    }
}
