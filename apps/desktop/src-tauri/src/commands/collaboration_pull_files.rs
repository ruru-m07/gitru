//! Local queries and explicit, caller-bound selected-file hydration.
use super::{
    collaboration::{authorize, CollaborationState, Operation},
    collaboration_local_links::{observe, CallerProof},
};
use collaboration::{
    local_links::LocalLinkState, storage::PullFileSelection, CollaborationError, ErrorCode,
    LocalLinkQuery, LocalLinkVersion, PullFileArtifact, PullFileArtifactSnapshot,
    PullFileArtifactValidation, PullFileBlobReferences, PullFileContentState, PullFileContext,
    PullFileDiffRequest, PullFileFlag, PullFileMembershipReceipt, PullFileQuery, PullFileSnapshot,
    PullFileSource, PullFileSourceStrategy, RefreshReceipt,
};
use git::{
    core::RepoServices,
    models::pull_file::{LocalPullFileDiff, LocalPullFileDiffRequest, LocalPullFileDiffState},
};
use ipc::repo_manager::{RepoManager, RepositoryInfo};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, State, Webview};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LoadLocalPullFileRequest {
    pub account_id: String,
    pub authorization_epoch: String,
    pub subject_id: String,
    pub file_facet_revision: String,
    pub context: PullFileContext,
    pub file_key: String,
    pub local_repository_id: String,
    pub link_id: String,
    pub link_generation: String,
}

impl LoadLocalPullFileRequest {
    fn selection(&self) -> Result<PullFileDiffRequest, CollaborationError> {
        if [
            &self.local_repository_id,
            &self.link_id,
            &self.link_generation,
        ]
        .iter()
        .any(|id| id.is_empty() || id.len() > 1_024 || id.chars().any(char::is_control))
        {
            return Err(CollaborationError::invalid("Invalid selected local clone"));
        }
        let request = PullFileDiffRequest {
            account_id: self.account_id.clone(),
            authorization_epoch: self.authorization_epoch.clone(),
            subject_id: self.subject_id.clone(),
            file_facet_revision: self.file_facet_revision.clone(),
            context: self.context.clone(),
            file_key: self.file_key.clone(),
        };
        request.validate()?;
        Ok(request)
    }
}

fn stale() -> CollaborationError {
    CollaborationError::new(
        ErrorCode::StaleView,
        "Inspect this saved file and local clone again",
    )
}

#[tauri::command]
pub async fn collaboration_pull_files(
    query: PullFileQuery,
    view: Webview,
    state: State<'_, CollaborationState>,
) -> Result<PullFileSnapshot, CollaborationError> {
    authorize(&view, Operation::PullFiles)?;
    state.get().await?.store().pull_files(query).await
}

#[tauri::command]
pub async fn collaboration_pull_file_artifact(
    request: PullFileDiffRequest,
    view: Webview,
    state: State<'_, CollaborationState>,
) -> Result<PullFileArtifactSnapshot, CollaborationError> {
    authorize(&view, Operation::PullFileArtifact)?;
    state.get().await?.store().pull_file_artifact(request).await
}

#[tauri::command]
pub async fn collaboration_hydrate_pull_file(
    request: PullFileDiffRequest,
    view: Webview,
    state: State<'_, CollaborationState>,
) -> Result<RefreshReceipt, CollaborationError> {
    authorize(&view, Operation::HydratePullFile)?;
    state.get().await?.hydrate_pull_file(request).await
}

struct ResolvedClone {
    query: LocalLinkQuery,
    repository: RepositoryInfo,
}

async fn resolve_clone(
    request: &LoadLocalPullFileRequest,
    selection: &PullFileSelection,
    app: &AppHandle,
    state: &CollaborationState,
) -> Result<ResolvedClone, CollaborationError> {
    let manager = RepoManager::new(app.clone());
    let before = manager
        .repository_for_link(&request.local_repository_id)
        .map_err(|_| stale())?
        .ok_or_else(stale)?;
    let (query, _, error) = observe(app, &request.local_repository_id).await?;
    if error.is_some() || query.registration_proof.is_none() {
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
    if snapshot.authorization_view != selection.membership.authorization_view
        || !snapshot.links.iter().any(|link| {
            link.id == request.link_id
                && link.generation == request.link_generation
                && link.state == LocalLinkState::Linked
                && link.local_repository_id == request.local_repository_id
                && link.account_id == request.account_id
                && link.instance_id == selection.binding.instance_id
                && (link.repository_id == selection.binding.repository_id
                    || link.repository_provider_id
                        == selection.membership.context.source_repository_provider_id)
                && link.repository.is_some()
        })
    {
        return Err(stale());
    }
    Ok(ResolvedClone { query, repository })
}

fn local_artifact(
    membership: &PullFileMembershipReceipt,
    diff: LocalPullFileDiff,
) -> Result<PullFileArtifact, CollaborationError> {
    let provenance = diff.provenance.ok_or_else(|| {
        CollaborationError::new(
            ErrorCode::NotFound,
            "This clone cannot resolve the saved pull range from local objects",
        )
    })?;
    let context = &membership.context;
    if provenance.base_oid != context.base_oid
        || provenance.head_oid != context.head_oid
        || provenance.known_merge_base_oid != context.merge_base_oid
        || context
            .merge_base_oid
            .as_ref()
            .is_some_and(|known| known != &provenance.resolved_merge_base_oid)
    {
        return Err(stale());
    }
    let (content_state, unified_text, binary_hint) = match diff.state {
        LocalPullFileDiffState::Text { unified_diff } => (
            PullFileContentState::Text,
            Some(unified_diff),
            PullFileFlag::Known(false),
        ),
        // No blob is retained by the local accelerator. Report the known binary
        // fact without inventing an available binary/image artifact.
        LocalPullFileDiffState::Binary => (
            PullFileContentState::Omitted,
            None,
            PullFileFlag::Known(true),
        ),
        LocalPullFileDiffState::Oversized => {
            (PullFileContentState::Oversized, None, PullFileFlag::Unknown)
        }
        LocalPullFileDiffState::Unsupported { .. } => (
            PullFileContentState::Unsupported,
            None,
            PullFileFlag::Unknown,
        ),
        LocalPullFileDiffState::Unavailable { .. } => {
            return Err(CollaborationError::new(
                ErrorCode::NotFound,
                "This saved file diff is unavailable in the selected local clone",
            ));
        }
    };
    let bytes = unified_text.as_ref().map_or(0, String::len).to_string();
    let artifact = PullFileArtifact {
        account_id: membership.account_id.clone(),
        authorization_epoch: membership.authorization_epoch.clone(),
        authorization_view: membership.authorization_view.clone(),
        subject_id: membership.subject_id.clone(),
        generation: membership.generation.clone(),
        file_key: membership.file_key.clone(),
        identity: membership.identity.clone(),
        context: membership.context.clone(),
        source: Some(PullFileSource {
            strategy: PullFileSourceStrategy::LocalExactRange,
            adapter_version: 1,
        }),
        validation: Some(PullFileArtifactValidation::LocalExactRange {
            local_validated_at: chrono::Utc::now()
                .to_rfc3339_opts(chrono::SecondsFormat::AutoSi, true),
            resolved_merge_base_oid: provenance.resolved_merge_base_oid,
        }),
        content_state,
        content_type: unified_text.as_ref().map(|_| "text/x-diff".into()),
        unified_text,
        blob_references: PullFileBlobReferences::default(),
        old_blob_oid: None,
        new_blob_oid: None,
        binary_hint,
        image_hint: PullFileFlag::Unknown,
        last_access_revision: membership.file_facet_revision.clone(),
        logical_bytes: bytes.clone(),
        on_disk_bytes: bytes,
    };
    artifact.validate()?;
    Ok(artifact)
}

#[tauri::command]
pub async fn collaboration_load_local_pull_file(
    request: LoadLocalPullFileRequest,
    view: Webview,
    app: AppHandle,
    state: State<'_, CollaborationState>,
) -> Result<PullFileArtifactSnapshot, CollaborationError> {
    authorize(&view, Operation::LocalPullFile)?;
    let selected_request = request.selection()?;
    let caller = CallerProof::capture(&view, &app)?;
    let runtime = state.get().await?;
    let selected = runtime
        .store()
        .pull_file_selection(selected_request.clone())
        .await?;
    caller.validate(&view, &app)?;
    let resolved = resolve_clone(&request, &selected, &app, &state).await?;
    caller.validate(&view, &app)?;
    let services = RepoServices::new(&resolved.repository.path).map_err(|_| stale())?;
    services.validate_worktree().await.map_err(|_| stale())?;
    let diff = services
        .pull_file()
        .selected_diff(LocalPullFileDiffRequest {
            base_oid: selected.membership.context.base_oid.clone(),
            head_oid: selected.membership.context.head_oid.clone(),
            merge_base_oid: selected.membership.context.merge_base_oid.clone(),
            old_path: selected.membership.identity.old_path.clone(),
            new_path: selected.membership.identity.new_path.clone(),
        })
        .await
        .map_err(|_| stale())?;
    let artifact = local_artifact(&selected.membership, diff)?;

    // Git, link, window, and account lifetimes can advance independently.
    // Recheck each before publishing, then let storage atomically fence rows.
    caller.validate(&view, &app)?;
    let current = runtime
        .store()
        .pull_file_selection(selected_request.clone())
        .await?;
    if current.membership != selected.membership || current.binding != selected.binding {
        return Err(stale());
    }
    let clone = resolve_clone(&request, &current, &app, &state).await?;
    caller.validate(&view, &app)?;
    if clone.query != resolved.query
        || clone.repository.id != resolved.repository.id
        || clone.repository.path != resolved.repository.path
    {
        return Err(stale());
    }
    runtime
        .save_local_pull_file_artifact(
            selected_request.clone(),
            selected.membership,
            artifact,
            clone.query,
            LocalLinkVersion {
                id: request.link_id,
                generation: request.link_generation,
            },
            || caller.validate_under_writer(&app),
        )
        .await?;
    let snapshot = runtime.store().pull_file_artifact(selected_request).await?;
    caller.validate(&view, &app)?;
    Ok(snapshot)
}

#[cfg(test)]
mod tests {
    use super::*;
    use collaboration::PullFileIdentity;
    use git::models::pull_file::{LocalPullFileComparison, LocalPullFileDiffProvenance};

    fn membership() -> PullFileMembershipReceipt {
        PullFileMembershipReceipt {
            account_id: "account".into(),
            authorization_epoch: "2".into(),
            authorization_view: "3".into(),
            subject_id: "pull".into(),
            generation: "123e4567-e89b-12d3-a456-426614174000".into(),
            file_facet_revision: "5".into(),
            file_key: "file:abc".into(),
            identity: PullFileIdentity {
                old_path: Some("src/a.rs".into()),
                new_path: Some("src/a.rs".into()),
            },
            context: PullFileContext {
                base_oid: "a".repeat(40),
                head_oid: "b".repeat(40),
                merge_base_oid: None,
                base_repository_provider_id: "base".into(),
                source_repository_provider_id: "source".into(),
                body_metadata_facet_revision: "4".into(),
            },
        }
    }
    fn diff(state: LocalPullFileDiffState) -> LocalPullFileDiff {
        LocalPullFileDiff {
            provenance: Some(LocalPullFileDiffProvenance {
                comparison: LocalPullFileComparison::MergeBaseToHead,
                base_oid: "a".repeat(40),
                head_oid: "b".repeat(40),
                resolved_merge_base_oid: "c".repeat(40),
                known_merge_base_oid: None,
            }),
            state,
        }
    }

    #[test]
    fn local_diff_keeps_merge_base_provenance_without_provider_validation() {
        let saved = local_artifact(
            &membership(),
            diff(LocalPullFileDiffState::Text {
                unified_diff: "@@ -1 +1 @@\n-old\n+new\n".into(),
            }),
        )
        .unwrap();
        assert_eq!(
            saved.source.unwrap().strategy,
            PullFileSourceStrategy::LocalExactRange
        );
        assert!(
            matches!(saved.validation, Some(PullFileArtifactValidation::LocalExactRange { resolved_merge_base_oid, .. }) if resolved_merge_base_oid == "c".repeat(40))
        );
        assert_eq!(
            saved.logical_bytes,
            saved.unified_text.unwrap().len().to_string()
        );
        assert_eq!(saved.context.merge_base_oid, None);
    }

    #[test]
    fn binary_and_oversized_local_content_never_invents_retained_bytes() {
        let binary = local_artifact(&membership(), diff(LocalPullFileDiffState::Binary)).unwrap();
        assert_eq!(binary.content_state, PullFileContentState::Omitted);
        assert_eq!(binary.binary_hint, PullFileFlag::Known(true));
        assert!(binary.unified_text.is_none());
        assert_eq!(binary.blob_references, PullFileBlobReferences::default());
        let oversized =
            local_artifact(&membership(), diff(LocalPullFileDiffState::Oversized)).unwrap();
        assert_eq!(oversized.content_state, PullFileContentState::Oversized);
        assert_eq!(oversized.on_disk_bytes, "0");
    }

    #[test]
    fn mismatched_or_missing_local_range_cannot_become_a_cached_diff() {
        let mut changed = membership();
        changed.context.head_oid = "d".repeat(40);
        assert_eq!(
            local_artifact(&changed, diff(LocalPullFileDiffState::Binary))
                .unwrap_err()
                .code,
            ErrorCode::StaleView
        );
        let mut changed = membership();
        changed.context.merge_base_oid = Some("d".repeat(40));
        assert_eq!(
            local_artifact(&changed, diff(LocalPullFileDiffState::Binary))
                .unwrap_err()
                .code,
            ErrorCode::StaleView
        );
        assert_eq!(
            local_artifact(
                &membership(),
                LocalPullFileDiff {
                    provenance: None,
                    state: LocalPullFileDiffState::Binary
                }
            )
            .unwrap_err()
            .code,
            ErrorCode::NotFound
        );
    }
}
