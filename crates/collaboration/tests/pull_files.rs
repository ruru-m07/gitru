use collaboration::storage::PullFileCommit;
use collaboration::storage::retention::CacheRetentionPolicy;
use collaboration::*;
use sqlx::{Connection, Row, SqliteConnection};

const BASE: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const HEAD: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

fn account(id: &str) -> RemoteAccount {
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

async fn seed(store: &Store, id: &str) -> RemoteAccount {
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

async fn hydrate_range(
    store: &Store,
    account: &RemoteAccount,
    base: &str,
    head: &str,
    source_repository: &str,
) {
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
    store
        .apply_detail(DetailCommit {
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
            etag: None,
            not_modified: false,
            whole_scope: true,
            complete: true,
            freshness_seconds: 3_600,
        })
        .await
        .unwrap();
}

fn source() -> PullFileSource {
    PullFileSource {
        strategy: PullFileSourceStrategy::GithubPullFiles,
        adapter_version: 1,
    }
}
fn file(path: &str) -> ProviderPullFile {
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
fn terminal(context: &PullFileContext) -> PullFileRangeValidation {
    PullFileRangeValidation {
        base_oid: context.base_oid.clone(),
        head_oid: context.head_oid.clone(),
        merge_base_oid: context.merge_base_oid.clone(),
        base_repository_provider_id: context.base_repository_provider_id.clone(),
        source_repository_provider_id: context.source_repository_provider_id.clone(),
    }
}
async fn commit(
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
async fn snapshot(store: &Store) -> PullFileSnapshot {
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
async fn publish(store: &Store, account: &RemoteAccount, paths: &[&str]) -> PullFileSnapshot {
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
fn request(snapshot: &PullFileSnapshot, account: &RemoteAccount) -> PullFileDiffRequest {
    PullFileDiffRequest {
        account_id: account.id.clone(),
        authorization_epoch: account.authorization_epoch.clone(),
        subject_id: "pull".into(),
        file_facet_revision: snapshot.facet_revision.clone().unwrap(),
        context: snapshot.context.clone().unwrap(),
        file_key: snapshot.files[0].file_key.clone(),
    }
}
fn artifact(m: &PullFileMembershipReceipt, text: &str) -> PullFileArtifact {
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
async fn database(path: &std::path::Path) -> SqliteConnection {
    SqliteConnection::connect_with(
        &sqlx::sqlite::SqliteConnectOptions::new()
            .filename(path)
            .foreign_keys(true),
    )
    .await
    .unwrap()
}

#[tokio::test]
async fn pages_stay_invisible_until_exact_terminal_publication_and_resume_after_reopen() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("cache.db");
    let store = Store::open(&path).await.unwrap();
    let account = seed(&store, "a").await;
    let lease = store
        .begin_pull_files("a", &account.authorization_epoch, "pull", source())
        .await
        .unwrap();
    let first = store
        .apply_pull_files(commit(&store, &lease, &["a.txt"], Some("page-2")).await)
        .await
        .unwrap();
    assert!(!first.published);
    assert!(snapshot(&store).await.files.is_empty());
    let next = first.next_lease.unwrap();
    store.close().await;
    drop(store);
    let store = Store::open(&path).await.unwrap();
    assert_eq!(
        store
            .resume_pull_files("a", &account.authorization_epoch, "pull")
            .await
            .unwrap(),
        Some(next.clone())
    );
    let final_page = store
        .apply_pull_files(commit(&store, &next, &["b.txt"], None).await)
        .await
        .unwrap();
    assert!(final_page.published);
    assert!(final_page.next_lease.is_none());
    let saved = snapshot(&store).await;
    assert_eq!(saved.files.len(), 2);
    assert_eq!(saved.completeness, PullFileCompleteness::complete());
    assert_eq!(saved.facet_revision, Some(final_page.revision));
    assert!(
        store
            .apply_pull_files(commit_with_old_lease(&lease))
            .await
            .is_err()
    );
}
fn commit_with_old_lease(lease: &PullFileLease) -> PullFileCommit {
    // Stale request replay carries native-looking fixture data but no current lease.
    let repository = RemoteRepository {
        id: "repo".into(),
        account_id: "a".into(),
        provider_id: "target-1".into(),
        full_name: "owner/project".into(),
        name: "project".into(),
        web_url: "https://github.com/owner/project".into(),
        description: None,
        default_branch: Some("main".into()),
        selected: true,
    };
    let subject = RemoteItem {
        id: "pull".into(),
        account_id: "a".into(),
        repository_id: Some("repo".into()),
        provider_id: "pull-67".into(),
        kind: RemoteItemKind::PullRequest,
        number: Some("67".into()),
        title: "fixture".into(),
        body: None,
        body_omitted: true,
        author: None,
        web_url: None,
        state: "open".into(),
        updated_at: "2026-10-07T00:00:00Z".into(),
        head_oid: Some(HEAD.into()),
        is_draft: None,
        reason: None,
        unread: None,
    };
    let mut account = account("a");
    account.authorization_epoch = lease.authorization_epoch.clone();
    PullFileCommit {
        request: PullFileCollectionRequest {
            account,
            authorization_view: lease.authorization_view.clone(),
            repository,
            subject,
            binding: lease.binding.clone(),
            source: source(),
            cursor: lease.next_cursor.clone(),
            start_position: lease.accepted_row_count,
            lease: lease.clone(),
        },
        page: PullFileProviderPage {
            context: lease.binding.context.clone(),
            files: vec![],
            source: source(),
            start_position: lease.accepted_row_count,
            next_cursor: None,
            cap: None,
            freshness_seconds: 60,
            cooldown_seconds: None,
        },
        terminal_validation: Some(terminal(&lease.binding.context)),
        expected_file_count: None,
        collection_cap: None,
    }
}

#[tokio::test]
async fn duplicates_cycles_wrong_counts_and_missing_terminal_evidence_roll_back_all_effects() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path().join("cache.db")).await.unwrap();
    let a = seed(&store, "a").await;
    let lease = store
        .begin_pull_files("a", &a.authorization_epoch, "pull", source())
        .await
        .unwrap();
    let next = store
        .apply_pull_files(commit(&store, &lease, &["same.txt"], Some("cursor")).await)
        .await
        .unwrap()
        .next_lease
        .unwrap();
    let before = snapshot(&store).await.revision;
    for mut bad in [
        commit(&store, &next, &["same.txt"], None).await,
        commit(&store, &next, &["unique.txt"], Some("cursor")).await,
        commit(&store, &next, &["unique.txt"], None).await,
        commit(&store, &next, &["unique.txt"], None).await,
    ]
    .into_iter()
    .enumerate()
    {
        if bad.0 == 2 {
            bad.1.terminal_validation = None;
        }
        if bad.0 == 3 {
            bad.1.expected_file_count = Some(3);
        }
        assert!(store.apply_pull_files(bad.1).await.is_err());
        assert_eq!(snapshot(&store).await.revision, before);
        assert_eq!(
            store
                .resume_pull_files("a", &a.authorization_epoch, "pull")
                .await
                .unwrap(),
            Some(next.clone())
        );
    }
    let mut final_page = commit(&store, &next, &["unique.txt"], None).await;
    final_page.expected_file_count = Some(3);
    final_page.collection_cap = Some(PullFileCapEvidence {
        provenance: PullFileCapProvenance::Provider,
        reason: PullFileCapReason::ProviderOverflow,
        remote_has_more: PullFileFlag::Known(true),
    });
    store.apply_pull_files(final_page).await.unwrap();
    let saved = snapshot(&store).await;
    assert_eq!(saved.files.len(), 2);
    assert_eq!(saved.completeness.state, PullFileCompletenessState::Capped);
}

#[tokio::test]
async fn local_cursor_membership_is_bound_to_generation_epoch_authview_and_body_revision() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path().join("cache.db")).await.unwrap();
    let a = seed(&store, "a").await;
    let saved = publish(&store, &a, &["a", "b", "c"]).await;
    let selected = request(&saved, &a);
    let old_membership = store
        .verify_pull_file_membership(selected.clone())
        .await
        .unwrap();
    let first = store
        .pull_files(PullFileQuery {
            account_id: "a".into(),
            subject_id: "pull".into(),
            limit: 1,
            cursor: None,
        })
        .await
        .unwrap();
    let query = PullFileQuery {
        account_id: "a".into(),
        subject_id: "pull".into(),
        limit: 1,
        cursor: first.next_cursor,
    };
    let next = store.pull_files(query.clone()).await.unwrap();
    assert_eq!(next.files[0].provider_position, 1);
    let mut forged = query.clone();
    let mut value: serde_json::Value =
        serde_json::from_str(forged.cursor.as_ref().unwrap()).unwrap();
    value["last_key"] = serde_json::json!("forged");
    forged.cursor = Some(value.to_string());
    assert!(store.pull_files(forged).await.is_err());
    publish(&store, &a, &["a"]).await;
    assert_eq!(
        store.pull_files(query).await.unwrap_err().code,
        ErrorCode::StaleView
    );
    assert!(
        store
            .apply_pull_file_artifact(
                selected.clone(),
                old_membership.clone(),
                artifact(&old_membership, "@@ -1 +1 @@\n-a\n+b\n")
            )
            .await
            .is_err()
    );
    let saved = snapshot(&store).await;
    let selected = request(&saved, &a);
    hydrate_range(&store, &a, BASE, HEAD, "fork-2").await;
    assert!(snapshot(&store).await.files.is_empty());
    assert!(store.verify_pull_file_membership(selected).await.is_err());
    let saved = publish(&store, &a, &["new"]).await;
    let selected = request(&saved, &a);
    store.upsert_account(account("b")).await.unwrap();
    assert!(snapshot(&store).await.files.is_empty());
    assert!(store.verify_pull_file_membership(selected).await.is_err());
}

#[tokio::test]
async fn artifact_is_separate_exact_bounded_and_accounted_from_native_bytes() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("cache.db");
    let store = Store::open(&path).await.unwrap();
    let a = seed(&store, "a").await;
    let saved = publish(&store, &a, &["a"]).await;
    let req = request(&saved, &a);
    let membership = store
        .verify_pull_file_membership(req.clone())
        .await
        .unwrap();
    let text = "@@ -1 +1 @@\n-a\n+b\n";
    let mut bad = artifact(&membership, text);
    bad.on_disk_bytes = "999".into();
    assert!(
        store
            .apply_pull_file_artifact(req.clone(), membership.clone(), bad)
            .await
            .is_err()
    );
    let mut bad = artifact(&membership, text);
    bad.source.as_mut().unwrap().strategy = PullFileSourceStrategy::GitlabRawDiffs;
    assert!(
        store
            .apply_pull_file_artifact(req.clone(), membership.clone(), bad)
            .await
            .is_err()
    );
    let mut bad = artifact(&membership, text);
    bad.content_state = PullFileContentState::Binary;
    bad.unified_text = None;
    bad.blob_references.new = Some("renderer-forged".into());
    bad.logical_bytes = "0".into();
    bad.on_disk_bytes = "0".into();
    assert!(
        store
            .apply_pull_file_artifact(req.clone(), membership.clone(), bad)
            .await
            .is_err()
    );
    let revision = store
        .apply_pull_file_artifact(req.clone(), membership.clone(), artifact(&membership, text))
        .await
        .unwrap();
    let result = store
        .pull_file_artifact(req.clone())
        .await
        .unwrap()
        .artifact
        .unwrap();
    assert_eq!(result.unified_text.as_deref(), Some(text));
    assert_eq!(result.last_access_revision, revision);
    let mut db = database(&path).await;
    let row=sqlx::query("SELECT json_extract(metadata_json,'$.unified_text') AS metadata_text,unified_text,logical_bytes FROM pull_file_artifacts").fetch_one(&mut db).await.unwrap();
    assert_eq!(row.get::<Option<String>, _>("metadata_text"), None);
    assert_eq!(row.get::<String, _>("unified_text"), text);
    assert_eq!(row.get::<i64, _>("logical_bytes"), text.len() as i64);
    assert_eq!(snapshot(&store).await.files, saved.files);
    let mut oversized = artifact(&membership, &"a".repeat(MAX_PULL_FILE_TEXT_BYTES + 1));
    oversized.content_type = None;
    assert!(
        store
            .apply_pull_file_artifact(req.clone(), membership.clone(), oversized)
            .await
            .is_err()
    );
    store.close().await;
    drop(store);
    let store = Store::open(&path).await.unwrap();
    assert!(
        store
            .pull_file_artifact(req)
            .await
            .unwrap()
            .artifact
            .is_some()
    );
}

#[tokio::test]
async fn heavy_artifacts_evict_before_summaries_and_selected_demand_or_pin_protects() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path().join("cache.db")).await.unwrap();
    let a = seed(&store, "a").await;
    let saved = publish(&store, &a, &["a"]).await;
    let req = request(&saved, &a);
    let m = store
        .verify_pull_file_membership(req.clone())
        .await
        .unwrap();
    store
        .apply_pull_file_artifact(
            req.clone(),
            m.clone(),
            artifact(&m, "@@ -1 +1 @@\n-a\n+b\n"),
        )
        .await
        .unwrap();
    let policy = CacheRetentionPolicy {
        target_logical_bytes: 0,
        checkpoint_wal: false,
        ..Default::default()
    };
    store.set_cache_pin("a", "pull", true).await.unwrap();
    for _ in 0..3 {
        store.run_cache_maintenance(policy.clone()).await.unwrap();
    }
    assert!(
        store
            .pull_file_artifact(req.clone())
            .await
            .unwrap()
            .artifact
            .is_some()
    );
    store.set_cache_pin("a", "pull", false).await.unwrap();
    let mut db = database(&dir.path().join("cache.db")).await;
    sqlx::query("INSERT INTO detail_demand(account_id,subject_id,facet,authorization_epoch,requested) VALUES('a','pull','files',?,1) ON CONFLICT(account_id,subject_id,facet) DO UPDATE SET requested=1")
        .bind(&a.authorization_epoch).execute(&mut db).await.unwrap();
    for _ in 0..3 {
        store.run_cache_maintenance(policy.clone()).await.unwrap();
    }
    assert!(
        store
            .pull_file_artifact(req.clone())
            .await
            .unwrap()
            .artifact
            .is_some()
    );
    sqlx::query("UPDATE detail_demand SET requested=0 WHERE account_id='a' AND subject_id='pull' AND facet='files'").execute(&mut db).await.unwrap();
    let report = store.run_cache_maintenance(policy).await.unwrap();
    assert!(report.freed_logical_bytes > 0);
    assert!(
        store
            .pull_file_artifact(req)
            .await
            .unwrap()
            .artifact
            .is_none()
    );
    assert_eq!(snapshot(&store).await.files.len(), 1);
}

#[tokio::test]
async fn concurrent_page_admission_has_one_winner_and_changed_range_rejects_inflight() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path().join("cache.db")).await.unwrap();
    let a = seed(&store, "a").await;
    let lease = store
        .begin_pull_files("a", &a.authorization_epoch, "pull", source())
        .await
        .unwrap();
    let first = commit(&store, &lease, &["a"], None).await;
    let second = commit(&store, &lease, &["b"], None).await;
    let (a1, a2) = tokio::join!(
        store.apply_pull_files(first),
        store.apply_pull_files(second)
    );
    assert_ne!(a1.is_ok(), a2.is_ok());
    assert_eq!(snapshot(&store).await.files.len(), 1);
    let lease = store
        .begin_pull_files("a", &a.authorization_epoch, "pull", source())
        .await
        .unwrap();
    let inflight = commit(&store, &lease, &["c"], None).await;
    hydrate_range(
        &store,
        &a,
        "cccccccccccccccccccccccccccccccccccccccc",
        HEAD,
        "fork-2",
    )
    .await;
    assert_eq!(
        store.apply_pull_files(inflight).await.unwrap_err().code,
        ErrorCode::StaleView
    );
    assert!(snapshot(&store).await.files.is_empty());
}

#[tokio::test]
async fn refresh_preserves_active_membership_and_storage_fault_rolls_back_publication() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("cache.db");
    let store = Store::open(&path).await.unwrap();
    let a = seed(&store, "a").await;
    let saved = publish(&store, &a, &["old"]).await;
    let req = request(&saved, &a);
    let receipt = store
        .verify_pull_file_membership(req.clone())
        .await
        .unwrap();
    let lease = store
        .begin_pull_files("a", &a.authorization_epoch, "pull", source())
        .await
        .unwrap();
    assert_eq!(snapshot(&store).await.files, saved.files);
    assert_eq!(
        store
            .verify_pull_file_membership(req.clone())
            .await
            .unwrap(),
        receipt
    );
    let mut db = database(&path).await;
    sqlx::query("CREATE TRIGGER injected_file_failure BEFORE UPDATE OF active_generation ON pull_file_facets WHEN NEW.active_generation IS NOT NULL BEGIN SELECT RAISE(ABORT,'injected file failure'); END").execute(&mut db).await.unwrap();
    let before = snapshot(&store).await.revision;
    assert!(
        store
            .apply_pull_files(commit(&store, &lease, &["new"], None).await)
            .await
            .is_err()
    );
    assert_eq!(snapshot(&store).await.revision, before);
    assert_eq!(snapshot(&store).await.files, saved.files);
    assert_eq!(
        store
            .resume_pull_files("a", &a.authorization_epoch, "pull")
            .await
            .unwrap(),
        Some(lease.clone())
    );
    sqlx::query("DROP TRIGGER injected_file_failure")
        .execute(&mut db)
        .await
        .unwrap();
    store
        .apply_pull_files(commit(&store, &lease, &["new"], None).await)
        .await
        .unwrap();
    assert!(store.verify_pull_file_membership(req).await.is_err());
    assert_eq!(
        snapshot(&store).await.files[0]
            .file
            .identity
            .new_path
            .as_deref(),
        Some("new")
    );
}

#[tokio::test]
async fn required_command_files_and_blob_anchors_protect_artifacts_and_authored_rows() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("cache.db");
    let store = Store::open(&path).await.unwrap();
    let a = seed(&store, "a").await;
    let saved = publish(&store, &a, &["a"]).await;
    let req = request(&saved, &a);
    let m = store
        .verify_pull_file_membership(req.clone())
        .await
        .unwrap();
    let mut db = database(&path).await;
    sqlx::query("INSERT INTO commands VALUES('a','11111111-1111-4111-8111-111111111111',1,1,'fixture',1,'pull_request','pull',NULL,x'01',x'02',x'03',zeroblob(32),1,1,'2026-10-08T00:00:00Z','queued')").execute(&mut db).await.unwrap();
    sqlx::query("INSERT INTO command_target_protections(account_id,command_id,reference_kind,reference_id,facet) VALUES('a','11111111-1111-4111-8111-111111111111','facet','pull','files')").execute(&mut db).await.unwrap();
    // These are authenticated native object fixtures; production exposes no
    // renderer registration path. A blob protection cannot be forged by a URL.
    sqlx::query("INSERT INTO pull_file_blob_objects VALUES('a','native-blob',NULL,'application/octet-stream',x'010203')").execute(&mut db).await.unwrap();
    let mut blob = artifact(&m, "");
    blob.content_state = PullFileContentState::Binary;
    blob.unified_text = None;
    blob.blob_references.new = Some("native-blob".into());
    blob.content_type = Some("application/octet-stream".into());
    blob.logical_bytes = "3".into();
    blob.on_disk_bytes = "3".into();
    store
        .apply_pull_file_artifact(req.clone(), m, blob)
        .await
        .unwrap();
    sqlx::query("INSERT INTO command_target_protections(account_id,command_id,reference_kind,reference_id,facet) VALUES('a','11111111-1111-4111-8111-111111111111','blob','native-blob','')").execute(&mut db).await.unwrap();
    let policy = CacheRetentionPolicy {
        target_logical_bytes: 0,
        checkpoint_wal: false,
        ..Default::default()
    };
    for _ in 0..5 {
        store.run_cache_maintenance(policy.clone()).await.unwrap();
    }
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM pull_file_artifacts")
            .fetch_one(&mut db)
            .await
            .unwrap(),
        1
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM pull_file_rows")
            .fetch_one(&mut db)
            .await
            .unwrap(),
        1
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM commands")
            .fetch_one(&mut db)
            .await
            .unwrap(),
        1
    );
    let plan:Vec<sqlx::sqlite::SqliteRow>=sqlx::query("EXPLAIN QUERY PLAN SELECT 1 FROM command_target_protections WHERE account_id='a' AND reference_id='native-blob' AND reference_kind='blob' AND required=1").fetch_all(&mut db).await.unwrap();
    assert!(plan.iter().any(|r| {
        r.get::<String, _>("detail")
            .contains("command_protection_lookup")
    }));
}

#[tokio::test]
async fn inactive_account_denial_and_changed_epoch_hide_cache_and_clean_staging_on_reopen() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("cache.db");
    let store = Store::open(&path).await.unwrap();
    let a = seed(&store, "a").await;
    publish(&store, &a, &["a"]).await;
    let lease = store
        .begin_pull_files("a", &a.authorization_epoch, "pull", source())
        .await
        .unwrap();
    let mut db = database(&path).await;
    sqlx::query(
        "UPDATE sync_scopes SET access_denied=1 WHERE account_id='a' AND scope='detail:pull:files'",
    )
    .execute(&mut db)
    .await
    .unwrap();
    assert!(snapshot(&store).await.files.is_empty());
    assert!(store.pull_file_request(&lease).await.is_err());
    store.close().await;
    drop(store);
    let store = Store::open(&path).await.unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM pull_file_generations WHERE state='staging'"
        )
        .fetch_one(&mut db)
        .await
        .unwrap(),
        0
    );
    assert!(snapshot(&store).await.files.is_empty());
    store.disconnect("a").await.unwrap();
    let snap = snapshot(&store).await;
    assert!(snap.files.is_empty());
    assert_eq!(snap.sync.state, SyncState::AuthRequired);
}

#[tokio::test]
async fn empty_complete_and_three_thousand_file_cap_are_explicit_and_pages_bounded() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path().join("cache.db")).await.unwrap();
    let a = seed(&store, "a").await;
    let empty = publish(&store, &a, &[]).await;
    assert!(empty.files.is_empty());
    assert_eq!(empty.completeness, PullFileCompleteness::complete());
    let mut lease = store
        .begin_pull_files("a", &a.authorization_epoch, "pull", source())
        .await
        .unwrap();
    for page in 0..30 {
        let paths: Vec<String> = (0..100)
            .map(|i| format!("src/file-{}", page * 100 + i))
            .collect();
        let refs = paths.iter().map(String::as_str).collect::<Vec<_>>();
        let cursor = format!("page-{}", page + 1);
        let mut value = commit(
            &store,
            &lease,
            &refs,
            (page < 29).then_some(cursor.as_str()),
        )
        .await;
        if page == 29 {
            value.page.cap = Some(PullFileCapEvidence {
                provenance: PullFileCapProvenance::Provider,
                reason: PullFileCapReason::ProviderFileLimit,
                remote_has_more: PullFileFlag::Unknown,
            });
        }
        let receipt = store.apply_pull_files(value).await.unwrap();
        if let Some(next) = receipt.next_lease {
            lease = next;
        }
    }
    let mut cursor = None;
    let mut count = 0;
    loop {
        let page = store
            .pull_files(PullFileQuery {
                account_id: "a".into(),
                subject_id: "pull".into(),
                limit: 100,
                cursor,
            })
            .await
            .unwrap();
        assert!(serde_json::to_vec(&page).unwrap().len() <= MAX_PULL_FILE_LOCAL_PAGE_BYTES);
        assert_eq!(page.completeness.state, PullFileCompletenessState::Capped);
        count += page.files.len();
        cursor = page.next_cursor;
        if cursor.is_none() {
            break;
        }
    }
    assert_eq!(count, 3000);
}
