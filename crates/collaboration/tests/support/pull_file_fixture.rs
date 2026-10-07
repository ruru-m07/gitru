use super::collaboration;
use collaboration::storage::PullFileCommit;
use collaboration::*;
use sqlx::{Connection, SqliteConnection};

pub(super) const BASE: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
pub(super) const HEAD: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

pub(super) fn account(id: &str) -> RemoteAccount {
    RemoteAccount {
        id: id.into(),
        provider: ProviderKind::Github,
        host: "github.com".into(),
        actor_id: format!("actor-{id}"),
        login: id.into(),
        display_name: None,
        authorization_epoch: "1".into(),
        state: AccountState::Active,
        notifications_supported: true,
    }
}

pub(super) async fn seed(store: &Store, id: &str) -> RemoteAccount {
    let account = store.upsert_account(account(id)).await.unwrap();
    let repository = RemoteRepository {
        id: "repo".into(),
        account_id: id.into(),
        provider_id: "target-1".into(),
        full_name: "owner/project".into(),
        name: "project".into(),
        web_url: "https://github.com/owner/project".into(),
        description: None,
        default_branch: Some("main".into()),
        selected: true,
    };
    let pull = RemoteItem {
        id: "pull".into(),
        account_id: id.into(),
        repository_id: Some("repo".into()),
        provider_id: "pull-67".into(),
        kind: RemoteItemKind::PullRequest,
        number: Some("67".into()),
        title: "File cache fixture".into(),
        body: None,
        body_omitted: true,
        author: None,
        web_url: Some("https://github.com/owner/project/pull/67".into()),
        state: "open".into(),
        updated_at: "2026-10-07T00:00:00Z".into(),
        head_oid: Some(HEAD.into()),
        is_draft: Some(false),
        reason: None,
        unread: None,
    };
    for (scope, repositories, items) in [
        ("repositories", vec![repository], vec![]),
        ("repo:repo:pull_request", vec![], vec![pull]),
    ] {
        let run_id = store
            .begin_sync(id, &account.authorization_epoch, scope)
            .await
            .unwrap();
        store
            .apply_page(PageCommit {
                account_id: id.into(),
                authorization_epoch: account.authorization_epoch.clone(),
                scope: scope.into(),
                run_id,
                repositories,
                items,
                endpoint_aliases: vec![],
                next_cursor: None,
                etag: None,
                last_modified: None,
                not_modified: false,
                complete: true,
                observed_at: "2026-10-07T00:00:00Z".into(),
            })
            .await
            .unwrap();
    }
    hydrate_range(store, &account, BASE, HEAD, "fork-2").await;
    account
}

pub(super) async fn hydrate_range(
    store: &Store,
    account: &RemoteAccount,
    base: &str,
    head: &str,
    source_repository: &str,
) {
    store
        .apply_detail(range_observation(store, account, base, head, source_repository).await)
        .await
        .unwrap();
}

pub(super) async fn range_observation(
    store: &Store,
    account: &RemoteAccount,
    base: &str,
    head: &str,
    source_repository: &str,
) -> DetailCommit {
    let lease = store
        .begin_detail(
            &account.id,
            &account.authorization_epoch,
            "pull",
            DetailFacet::Body,
        )
        .await
        .unwrap();
    let source = DetailSource {
        source: "fixture/pull/v1".into(),
        adapter_version: 1,
        field_mask: vec![DetailField::Body],
        provider_updated_at: None,
        observed_at: "2099-01-01T00:00:00Z".into(),
    };
    DetailCommit {
        reconciliation: DetailReconciliation::full_history(),
        account_id: account.id.clone(),
        authorization_epoch: account.authorization_epoch.clone(),
        authorization_view: lease.authorization_view,
        instance_id: lease.instance_id,
        subject_id: "pull".into(),
        facet: DetailFacet::Body,
        run_id: lease.run_id,
        request_cursor: lease.next_cursor,
        body: DetailValue {
            state: DetailValueState::Known,
            text: Some("body".into()),
        },
        metadata: Some(ResourceMetadataObservation {
            kind: RemoteItemKind::PullRequest,
            values: ResourceMetadataValues {
                base: Some(DetailBranch {
                    name: "main".into(),
                    oid: base.into(),
                    repository: Some(DetailRepositoryRef {
                        provider_id: "target-1".into(),
                        full_name: "owner/project".into(),
                        web_url: None,
                    }),
                }),
                head: Some(DetailBranch {
                    name: "feature".into(),
                    oid: head.into(),
                    repository: Some(DetailRepositoryRef {
                        provider_id: source_repository.into(),
                        full_name: "fork/project".into(),
                        web_url: None,
                    }),
                }),
                ..Default::default()
            },
            fields: vec![
                MetadataObservedField {
                    field: MetadataField::Base,
                    state: DetailValueState::Known,
                },
                MetadataObservedField {
                    field: MetadataField::Head,
                    state: DetailValueState::Known,
                },
            ],
            source: MetadataSource {
                source: source.source.clone(),
                adapter_version: 1,
                provider_updated_at: None,
                observed_at: source.observed_at.clone(),
            },
        }),
        subject_binding: Some(DetailSubjectBinding {
            repository_id: "repo".into(),
            repository_provider_id: "target-1".into(),
            provider_id: "pull-67".into(),
            number: Some("67".into()),
            kind: RemoteItemKind::PullRequest,
            head_oid: Some(head.into()),
        }),
        entries: vec![],
        source,
        next_cursor: None,
        etag: Some("range-v1".into()),
        not_modified: false,
        whole_scope: true,
        complete: true,
        freshness_seconds: 3_600,
    }
}

pub(super) fn source() -> PullFileSource {
    PullFileSource {
        strategy: PullFileSourceStrategy::GithubPullFiles,
        adapter_version: 1,
    }
}
pub(super) fn file(path: &str) -> ProviderPullFile {
    ProviderPullFile {
        identity: PullFileIdentity {
            old_path: Some(path.into()),
            new_path: Some(path.into()),
        },
        provider_file_id: None,
        change_kind: PullFileChangeKind::Modified,
        provider_change_kind: "modified".into(),
        additions: PullFileCount::Known("1".into()),
        deletions: PullFileCount::Known("1".into()),
        total_changes: PullFileCount::Known("2".into()),
        old_mode: None,
        new_mode: None,
        mode_changed: PullFileFlag::Unknown,
        binary: PullFileFlag::Unknown,
        generated: PullFileFlag::Unknown,
        provider_collapsed: PullFileFlag::Unknown,
        provider_too_large: PullFileFlag::Unknown,
        diff_hint: PullFileDiffHint::Candidate,
    }
}
pub(super) fn terminal(context: &PullFileContext) -> PullFileRangeValidation {
    PullFileRangeValidation {
        base_oid: context.base_oid.clone(),
        head_oid: context.head_oid.clone(),
        merge_base_oid: context.merge_base_oid.clone(),
        base_repository_provider_id: context.base_repository_provider_id.clone(),
        source_repository_provider_id: context.source_repository_provider_id.clone(),
    }
}
pub(super) async fn commit(
    store: &Store,
    lease: &PullFileLease,
    paths: &[&str],
    next: Option<&str>,
) -> PullFileCommit {
    let request = store.pull_file_request(lease).await.unwrap();
    PullFileCommit {
        page: PullFileProviderPage {
            context: lease.binding.context.clone(),
            files: paths.iter().map(|p| file(p)).collect(),
            source: source(),
            start_position: lease.accepted_row_count,
            next_cursor: next.map(str::to_owned),
            cap: None,
            freshness_seconds: 3600,
            cooldown_seconds: None,
        },
        terminal_validation: next.is_none().then(|| terminal(&lease.binding.context)),
        expected_file_count: None,
        collection_cap: None,
        request,
    }
}
pub(super) async fn snapshot(store: &Store) -> PullFileSnapshot {
    store
        .pull_files(PullFileQuery {
            account_id: "a".into(),
            subject_id: "pull".into(),
            cursor: None,
            limit: 100,
        })
        .await
        .unwrap()
}
pub(super) async fn publish(
    store: &Store,
    account: &RemoteAccount,
    paths: &[&str],
) -> PullFileSnapshot {
    let lease = store
        .begin_pull_files(&account.id, &account.authorization_epoch, "pull", source())
        .await
        .unwrap();
    store
        .apply_pull_files(commit(store, &lease, paths, None).await)
        .await
        .unwrap();
    snapshot(store).await
}
pub(super) fn request(snapshot: &PullFileSnapshot, account: &RemoteAccount) -> PullFileDiffRequest {
    PullFileDiffRequest {
        account_id: account.id.clone(),
        authorization_epoch: account.authorization_epoch.clone(),
        subject_id: "pull".into(),
        file_facet_revision: snapshot.facet_revision.clone().unwrap(),
        context: snapshot.context.clone().unwrap(),
        file_key: snapshot.files[0].file_key.clone(),
    }
}
pub(super) fn artifact(m: &PullFileMembershipReceipt, text: &str) -> PullFileArtifact {
    PullFileArtifact {
        account_id: m.account_id.clone(),
        authorization_epoch: m.authorization_epoch.clone(),
        authorization_view: m.authorization_view.clone(),
        subject_id: m.subject_id.clone(),
        generation: m.generation.clone(),
        file_key: m.file_key.clone(),
        identity: m.identity.clone(),
        context: m.context.clone(),
        source: Some(PullFileSource {
            strategy: PullFileSourceStrategy::GithubPullDiff,
            adapter_version: 1,
        }),
        validation: Some(PullFileArtifactValidation::Provider {
            provider_validated_at: "2026-10-08T00:00:00Z".into(),
        }),
        content_state: PullFileContentState::Text,
        unified_text: Some(text.into()),
        blob_references: PullFileBlobReferences::default(),
        old_blob_oid: None,
        new_blob_oid: None,
        content_type: Some("text/plain".into()),
        binary_hint: PullFileFlag::Unknown,
        image_hint: PullFileFlag::Unknown,
        last_access_revision: "1".into(),
        logical_bytes: text.len().to_string(),
        on_disk_bytes: text.len().to_string(),
    }
}
pub(super) async fn database(path: &std::path::Path) -> SqliteConnection {
    SqliteConnection::connect_with(
        &sqlx::sqlite::SqliteConnectOptions::new()
            .filename(path)
            .foreign_keys(true),
    )
    .await
    .unwrap()
}
