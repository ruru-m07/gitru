use collaboration::*;
mod detail_support;
use detail_support::{account, seed};

fn endpoint() -> LocalRemoteEndpoint {
    LocalRemoteEndpoint {
        remote_name: "origin".into(),
        direction: LinkDirection::Fetch,
        ordinal: 0,
        transport: LinkTransport::Https,
        host: "github.com".into(),
        port: 443,
        path: "owner/project.git".into(),
    }
}
fn query() -> LocalLinkQuery {
    LocalLinkQuery {
        local_repository_id: "durable-local-repo".into(),
        registration_proof: Some("native-worktree-proof".into()),
        remote_digest: Some("safe-semantic-digest".into()),
        endpoints: vec![endpoint()],
    }
}
async fn confirm(
    store: &Store,
    query: LocalLinkQuery,
    account: &str,
    replace: Option<LocalLinkVersion>,
) -> LocalLinkWriteReceipt {
    let snapshot = store.local_link_snapshot(query.clone()).await.unwrap();
    let candidate = snapshot
        .resolutions
        .iter()
        .flat_map(|r| &r.candidates)
        .find(|c| c.account_id == account)
        .unwrap();
    store
        .confirm_local_link(ConfirmLocalLink {
            query,
            candidate_id: candidate.id.clone(),
            expected_authorization_view: snapshot.authorization_view,
            expected_bindings_generation: snapshot.bindings_generation,
            replace,
        })
        .await
        .unwrap()
}
async fn page(store: &Store, id: &str, repositories: Vec<RemoteRepository>) {
    let account = store.account(id).await.unwrap();
    let run_id = store
        .begin_sync(id, &account.authorization_epoch, "repositories")
        .await
        .unwrap();
    store
        .apply_page(PageCommit {
            account_id: id.into(),
            authorization_epoch: account.authorization_epoch,
            scope: "repositories".into(),
            run_id,
            repositories,
            items: vec![],
            endpoint_aliases: vec![],
            next_cursor: None,
            etag: None,
            last_modified: None,
            not_modified: false,
            complete: true,
            observed_at: "2026-10-03T01:00:00Z".into(),
        })
        .await
        .unwrap();
}

#[tokio::test]
async fn explicit_accounts_and_multiple_clones_are_independent_authored_choices() {
    let directory = tempfile::tempdir().unwrap();
    let store = Store::open(directory.path().join("db")).await.unwrap();
    seed(&store, "a").await;
    seed(&store, "b").await;
    let snapshot = store.local_link_snapshot(query()).await.unwrap();
    assert_eq!(snapshot.resolutions[0].candidates.len(), 2);
    let first = confirm(&store, query(), "b", None).await;
    let mut another = query();
    another.local_repository_id = "another-clone".into();
    another.registration_proof = Some("other-worktree".into());
    confirm(&store, another, "b", None).await;
    let instance = store.provider_instance("b").await.unwrap();
    let clones = store
        .local_links_for_resource("b", &instance.id, "repo", "1")
        .await
        .unwrap();
    assert_eq!(clones.len(), 2);
    assert!(clones.iter().all(|l| l.account_id == "b"));
    assert_eq!(
        first.revision,
        store
            .changes_since("0")
            .await
            .unwrap()
            .changes
            .iter()
            .find(|c| c.scope == "local_link:durable-local-repo")
            .unwrap()
            .revision
    );
}

#[tokio::test]
async fn pull_context_includes_authorized_source_repository_clones() {
    let directory = tempfile::tempdir().unwrap();
    let store = Store::open(directory.path().join("db")).await.unwrap();
    let account = seed(&store, "a").await;
    page(
        &store,
        "a",
        vec![
            RemoteRepository {
                id: "repo".into(),
                account_id: "a".into(),
                provider_id: "1".into(),
                full_name: "owner/project".into(),
                name: "project".into(),
                web_url: "https://github.com/owner/project".into(),
                description: None,
                default_branch: None,
                selected: true,
            },
            RemoteRepository {
                id: "fork-repo".into(),
                account_id: "a".into(),
                provider_id: "2".into(),
                full_name: "fork/project".into(),
                name: "project".into(),
                web_url: "https://github.com/fork/project".into(),
                description: None,
                default_branch: None,
                selected: false,
            },
        ],
    )
    .await;
    let mut fork = query();
    fork.local_repository_id = "fork-clone".into();
    fork.registration_proof = Some("fork-worktree-proof".into());
    fork.remote_digest = Some("fork-semantic-digest".into());
    fork.endpoints[0].path = "fork/project.git".into();
    let source_link = confirm(&store, fork, "a", None).await;
    let instance = store.provider_instance("a").await.unwrap();

    assert!(
        store
            .local_links_for_resource("a", &instance.id, "repo", &account.authorization_epoch)
            .await
            .unwrap()
            .is_empty()
    );
    let range_links = store
        .local_links_for_resource_context(
            "a",
            &instance.id,
            "repo",
            Some("2"),
            &account.authorization_epoch,
        )
        .await
        .unwrap();
    assert_eq!(range_links.len(), 1);
    assert_eq!(range_links[0].id, source_link.link.id);
    assert_eq!(range_links[0].repository_provider_id, "2");
}

#[tokio::test]
async fn grant_cutover_hides_cache_but_preserves_link_and_newer_draft_and_removal() {
    let directory = tempfile::tempdir().unwrap();
    let store = Store::open(directory.path().join("db")).await.unwrap();
    seed(&store, "a").await;
    let created = confirm(&store, query(), "a", None).await;
    store
        .save_draft(LocalDraft {
            account_id: "a".into(),
            subject_id: "pull".into(),
            body: "newest private text".into(),
            generation: "0".into(),
        })
        .await
        .unwrap();
    store.disconnect("a").await.unwrap();
    let snapshot = store.local_link_snapshot(query()).await.unwrap();
    assert_eq!(snapshot.links.len(), 1);
    assert_eq!(snapshot.links[0].state, LocalLinkState::Unavailable);
    assert!(snapshot.links[0].repository.is_none());
    assert!(snapshot.resolutions[0].candidates.is_empty());
    assert!(
        !serde_json::to_string(&snapshot)
            .unwrap()
            .contains("summary body")
    );
    assert_eq!(
        store.draft("a", "pull").await.unwrap().unwrap().body,
        "newest private text"
    );
    store
        .remove_local_link(&created.link.id, &created.link.generation)
        .await
        .unwrap();
    assert!(
        store
            .local_link_snapshot(query())
            .await
            .unwrap()
            .links
            .is_empty()
    );
    assert_eq!(
        store.draft("a", "pull").await.unwrap().unwrap().body,
        "newest private text"
    );
}

#[tokio::test]
async fn stale_actor_and_binding_preview_cannot_commit_or_mutate_revision() {
    let directory = tempfile::tempdir().unwrap();
    let store = Store::open(directory.path().join("db")).await.unwrap();
    seed(&store, "a").await;
    let old = store.local_link_snapshot(query()).await.unwrap();
    let candidate = old.resolutions[0].candidates[0].id.clone();
    store.disconnect("a").await.unwrap();
    let revision = store.revision().await.unwrap();
    let error = store
        .confirm_local_link(ConfirmLocalLink {
            query: query(),
            candidate_id: candidate,
            expected_authorization_view: old.authorization_view,
            expected_bindings_generation: old.bindings_generation,
            replace: None,
        })
        .await
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::StaleView);
    assert_eq!(store.revision().await.unwrap(), revision);
    assert!(
        store
            .local_link_snapshot(query())
            .await
            .unwrap()
            .links
            .is_empty()
    );
}

#[tokio::test]
async fn remote_and_native_worktree_replacement_require_reconfirmation() {
    let directory = tempfile::tempdir().unwrap();
    let store = Store::open(directory.path().join("db")).await.unwrap();
    seed(&store, "a").await;
    confirm(&store, query(), "a", None).await;
    for changed in ["remote", "worktree"] {
        let mut q = query();
        if changed == "remote" {
            q.remote_digest = Some("changed-safe-configuration".into());
        } else {
            q.registration_proof = Some("replacement-worktree".into());
        }
        let link = &store.local_link_snapshot(q).await.unwrap().links[0];
        assert_eq!(link.state, LocalLinkState::RemoteChanged);
        assert!(link.repository.is_none());
    }
    let mut missing = query();
    missing.registration_proof = None;
    missing.remote_digest = None;
    missing.endpoints.clear();
    assert_eq!(
        store.local_link_snapshot(missing).await.unwrap().links[0].state,
        LocalLinkState::LocalRepositoryMissing
    );
}

#[tokio::test]
async fn change_and_remove_compare_exact_generation_without_destroying_old_intent() {
    let directory = tempfile::tempdir().unwrap();
    let store = Store::open(directory.path().join("db")).await.unwrap();
    seed(&store, "a").await;
    let first = confirm(&store, query(), "a", None).await;
    let second = confirm(
        &store,
        query(),
        "a",
        Some(LocalLinkVersion {
            id: first.link.id.clone(),
            generation: first.link.generation.clone(),
        }),
    )
    .await;
    assert_eq!(second.link.id, first.link.id);
    assert_eq!(second.link.generation, "2");
    let revision = store.revision().await.unwrap();
    assert_eq!(
        store
            .remove_local_link(&first.link.id, &first.link.generation)
            .await
            .unwrap_err()
            .code,
        ErrorCode::StaleView
    );
    assert_eq!(store.revision().await.unwrap(), revision);
    assert_eq!(
        store.local_link_snapshot(query()).await.unwrap().links[0],
        second.link
    );
}

#[tokio::test]
async fn renamed_repository_retains_identity_and_historical_path_reuse_blocks_navigation() {
    let directory = tempfile::tempdir().unwrap();
    let store = Store::open(directory.path().join("db")).await.unwrap();
    seed(&store, "a").await;
    confirm(&store, query(), "a", None).await;
    let mut renamed = store.repositories("a").await.unwrap().repositories[0].clone();
    renamed.full_name = "owner/renamed".into();
    renamed.name = "renamed".into();
    renamed.web_url = "https://github.com/owner/renamed".into();
    page(&store, "a", vec![renamed.clone()]).await;
    let snapshot = store.local_link_snapshot(query()).await.unwrap();
    assert_eq!(snapshot.links[0].state, LocalLinkState::Linked);
    assert_eq!(
        snapshot.links[0].repository.as_ref().unwrap().full_name,
        "owner/renamed"
    );
    let mut reused = renamed.clone();
    reused.id = "new-repo".into();
    reused.provider_id = "9007199254740999".into();
    reused.full_name = "owner/project".into();
    reused.name = "project".into();
    reused.web_url = "https://github.com/owner/project".into();
    page(&store, "a", vec![renamed, reused]).await;
    let snapshot = store.local_link_snapshot(query()).await.unwrap();
    assert_eq!(snapshot.resolutions[0].state, LocalLinkState::Ambiguous);
    assert!(snapshot.resolutions[0].candidates.is_empty());
    assert_eq!(snapshot.links[0].state, LocalLinkState::Ambiguous);
    assert!(snapshot.links[0].repository.is_none());
}

#[tokio::test]
async fn explicit_alias_base_path_and_port_are_exact_and_persisted() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("db");
    let store = Store::open(&path).await.unwrap();
    seed(&store, "a").await;
    let generation = store
        .local_link_snapshot(query())
        .await
        .unwrap()
        .bindings_generation;
    let instance = store.provider_instance("a").await.unwrap();
    let binding = store
        .save_transport_binding(SaveTransportBinding {
            instance_id: instance.id,
            transport: LinkTransport::Ssh,
            host: "github-work".into(),
            port: 2222,
            path_prefix: "installation".into(),
            layout: RepositoryPathLayout::OwnerRepository,
            expected_bindings_generation: generation,
            replace: None,
        })
        .await
        .unwrap();
    let mut q = query();
    q.endpoints[0].transport = LinkTransport::Ssh;
    q.endpoints[0].host = "github-work".into();
    q.endpoints[0].port = 2222;
    q.endpoints[0].path = "installation/owner/project.git".into();
    assert_eq!(
        store
            .local_link_snapshot(q.clone())
            .await
            .unwrap()
            .resolutions[0]
            .candidates
            .len(),
        1
    );
    for invalid in [
        "installation-other/owner/project.git",
        "installation/group/owner/project.git",
    ] {
        let mut invalidq = q.clone();
        invalidq.endpoints[0].path = invalid.into();
        assert!(
            store
                .local_link_snapshot(invalidq)
                .await
                .unwrap()
                .resolutions[0]
                .candidates
                .is_empty()
        );
    }
    let mut wrong = q.clone();
    wrong.endpoints[0].port = 443;
    assert!(
        store.local_link_snapshot(wrong).await.unwrap().resolutions[0]
            .candidates
            .is_empty()
    );
    let linked = confirm(&store, q.clone(), "a", None).await;
    store.close().await.unwrap();
    drop(store);
    let reopened = Store::open(path).await.unwrap();
    assert_eq!(
        reopened.local_link_snapshot(q.clone()).await.unwrap().links[0],
        linked.link
    );
    let generation = reopened
        .local_link_snapshot(q.clone())
        .await
        .unwrap()
        .bindings_generation;
    reopened
        .remove_transport_binding(&binding.id, &binding.generation, &generation)
        .await
        .unwrap();
    assert_eq!(
        reopened.local_link_snapshot(q).await.unwrap().links[0].state,
        LocalLinkState::UnconfiguredInstance
    );
}

#[tokio::test]
async fn denial_hides_repository_without_disabling_local_removal() {
    let directory = tempfile::tempdir().unwrap();
    let store = Store::open(directory.path().join("db")).await.unwrap();
    seed(&store, "a").await;
    let link = confirm(&store, query(), "a", None).await;
    let status = SyncStatus {
        state: SyncState::Error,
        error: Some(CollaborationError::new(
            ErrorCode::PermissionDenied,
            "Fixture denial",
        )),
        ..SyncStatus::default()
    };
    store
        .set_sync_status("a", "1", "repositories", status)
        .await
        .unwrap();
    let snapshot = store.local_link_snapshot(query()).await.unwrap();
    assert!(snapshot.links[0].repository.is_none());
    assert!(snapshot.resolutions[0].candidates.is_empty());
    assert_eq!(
        store
            .local_links_for_resource(
                "a",
                &store.provider_instance("a").await.unwrap().id,
                "repo",
                "1"
            )
            .await
            .unwrap_err()
            .code,
        ErrorCode::PermissionDenied
    );
    store
        .remove_local_link(&link.link.id, &link.link.generation)
        .await
        .unwrap();
}

#[tokio::test]
async fn unselected_saved_repository_is_metadata_only_and_linking_does_not_select_it() {
    let directory = tempfile::tempdir().unwrap();
    let store = Store::open(directory.path().join("db")).await.unwrap();
    seed(&store, "a").await;
    store.select_repository("a", "repo", false).await.unwrap();
    let link = confirm(&store, query(), "a", None).await;
    assert!(!link.link.repository.unwrap().selected);
    assert!(!store.repositories("a").await.unwrap().repositories[0].selected);
}

#[tokio::test]
async fn subgroup_provider_fixture_uses_full_path_without_qualifying_live_adapter() {
    let directory = tempfile::tempdir().unwrap();
    let store = Store::open(directory.path().join("db")).await.unwrap();
    let mut fixture = account("gitlab-fixture");
    fixture.provider = ProviderKind::Gitlab;
    fixture.host = "gitlab.com".into();
    store.upsert_account(fixture).await.unwrap();
    page(
        &store,
        "gitlab-fixture",
        vec![RemoteRepository {
            id: "nested".into(),
            account_id: "gitlab-fixture".into(),
            provider_id: "9007199254740997".into(),
            full_name: "group/subgroup/project".into(),
            name: "project".into(),
            web_url: "https://gitlab.com/group/subgroup/project".into(),
            description: None,
            default_branch: None,
            selected: true,
        }],
    )
    .await;
    let mut q = query();
    q.endpoints[0].host = "gitlab.com".into();
    q.endpoints[0].path = "group/subgroup/project.git".into();
    let link = confirm(&store, q, "gitlab-fixture", None).await;
    assert_eq!(link.link.repository_provider_id, "9007199254740997");
}

#[tokio::test]
async fn changing_account_invalidates_both_reverse_query_scopes() {
    let directory = tempfile::tempdir().unwrap();
    let store = Store::open(directory.path().join("db")).await.unwrap();
    seed(&store, "a").await;
    seed(&store, "b").await;
    let first = confirm(&store, query(), "a", None).await;
    let after = first.revision.clone();
    let changed = confirm(
        &store,
        query(),
        "b",
        Some(LocalLinkVersion {
            id: first.link.id,
            generation: first.link.generation,
        }),
    )
    .await;
    let changes = store.changes_since(&after).await.unwrap();
    assert!(
        changes
            .changes
            .iter()
            .any(|c| c.account_id == "a" && c.scope == "local_link:durable-local-repo")
    );
    assert!(
        changes
            .changes
            .iter()
            .any(|c| c.account_id == "b" && c.scope == "local_link:durable-local-repo")
    );
    assert_eq!(changes.revision, changed.revision);
    let instance = store.provider_instance("a").await.unwrap();
    assert!(
        store
            .local_links_for_resource("a", &instance.id, "repo", "1")
            .await
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn binding_generation_fences_preview_and_builtin_trust_cannot_be_redefined() {
    let directory = tempfile::tempdir().unwrap();
    let store = Store::open(directory.path().join("db")).await.unwrap();
    seed(&store, "a").await;
    let old = store.local_link_snapshot(query()).await.unwrap();
    let candidate = old.resolutions[0].candidates[0].clone();
    let binding = SaveTransportBinding {
        instance_id: candidate.instance_id.clone(),
        transport: LinkTransport::Scp,
        host: "github-alias".into(),
        port: 22,
        path_prefix: String::new(),
        layout: RepositoryPathLayout::OwnerRepository,
        expected_bindings_generation: old.bindings_generation.clone(),
        replace: None,
    };
    store.save_transport_binding(binding.clone()).await.unwrap();
    let revision = store.revision().await.unwrap();
    let error = store
        .confirm_local_link(ConfirmLocalLink {
            query: query(),
            candidate_id: candidate.id,
            expected_authorization_view: old.authorization_view,
            expected_bindings_generation: old.bindings_generation,
            replace: None,
        })
        .await
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::StaleView);
    assert_eq!(store.revision().await.unwrap(), revision);
    let mut wrong = binding;
    wrong.host = "github.com".into();
    wrong.expected_bindings_generation = store
        .local_link_snapshot(query())
        .await
        .unwrap()
        .bindings_generation;
    assert_eq!(
        store.save_transport_binding(wrong).await.unwrap_err().code,
        ErrorCode::InvalidInput
    );
    assert_eq!(store.revision().await.unwrap(), revision);
}

#[tokio::test]
async fn an_inactive_competing_alias_still_blocks_path_reuse_without_private_projection() {
    let directory = tempfile::tempdir().unwrap();
    let store = Store::open(directory.path().join("db")).await.unwrap();
    seed(&store, "a").await;
    let mut reused = store.repositories("a").await.unwrap().repositories[0].clone();
    reused.id = "new-identity".into();
    reused.provider_id = "9007199254740999".into();
    page(&store, "a", vec![reused.clone()]).await;
    page(&store, "a", vec![reused]).await;
    let visible = store.repositories("a").await.unwrap().repositories;
    assert_eq!(visible.len(), 1);
    assert_eq!(visible[0].id, "new-identity");
    let snapshot = store.local_link_snapshot(query()).await.unwrap();
    assert_eq!(snapshot.resolutions[0].state, LocalLinkState::Ambiguous);
    assert!(snapshot.resolutions[0].candidates.is_empty());
}

#[tokio::test]
async fn transient_network_or_quota_evidence_does_not_disable_authorized_saved_resolution() {
    let directory = tempfile::tempdir().unwrap();
    let store = Store::open(directory.path().join("db")).await.unwrap();
    seed(&store, "a").await;
    for (state, code) in [
        (SyncState::Offline, ErrorCode::Network),
        (SyncState::RateLimited, ErrorCode::RateLimited),
    ] {
        store
            .set_sync_status(
                "a",
                "1",
                "repositories",
                SyncStatus {
                    state,
                    error: Some(CollaborationError::new(code, "Synthetic transient")),
                    last_success_at: None,
                    next_retry_at: Some("2099-01-01T00:00:00Z".into()),
                },
            )
            .await
            .unwrap();
        let snapshot = store.local_link_snapshot(query()).await.unwrap();
        assert_eq!(snapshot.resolutions[0].candidates.len(), 1);
    }
}
