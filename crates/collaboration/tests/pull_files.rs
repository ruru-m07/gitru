use ::collaboration;
use collaboration::storage::PullFileCommit;
use collaboration::storage::retention::CacheRetentionPolicy;
use collaboration::*;
use sqlx::Row;
#[path = "support/pull_file_fixture.rs"]
mod fixture;
use fixture::*;

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
    store.close().await.unwrap();
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
    hydrate_range(
        &store,
        &a,
        "cccccccccccccccccccccccccccccccccccccccc",
        HEAD,
        "fork-2",
    )
    .await;
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
    store.close().await.unwrap();
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
    store.close().await.unwrap();
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

#[tokio::test]
async fn repeated_full_generations_cannot_strand_cache_above_the_retention_row_budget() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("cache.db");
    let store = Store::open(&path).await.unwrap();
    let a = seed(&store, "a").await;
    for run in 0..2 {
        let mut lease = store
            .begin_pull_files("a", &a.authorization_epoch, "pull", source())
            .await
            .unwrap();
        for page in 0..30 {
            let paths: Vec<String> = (0..100)
                .map(|i| format!("run-{run}/file-{}", page * 100 + i))
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
            if let Some(next) = store.apply_pull_files(value).await.unwrap().next_lease {
                lease = next;
            }
        }
    }
    let mut db = database(&path).await;
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM pull_file_rows")
            .fetch_one(&mut db)
            .await
            .unwrap(),
        6000
    );
    let active: String = sqlx::query_scalar("SELECT active_generation FROM pull_file_facets")
        .fetch_one(&mut db)
        .await
        .unwrap();
    let policy = CacheRetentionPolicy {
        target_logical_bytes: 0,
        max_evict_facets: 1,
        checkpoint_wal: false,
        ..Default::default()
    };
    for _ in 0..5 {
        let report = store.run_cache_maintenance(policy.clone()).await.unwrap();
        assert!(report.evicted_entry_rows <= policy.max_entry_rows);
        assert!(report.scanned_facets <= policy.max_scan_facets);
        let count: i64 = sqlx::query_scalar("SELECT count(*) FROM pull_file_rows")
            .fetch_one(&mut db)
            .await
            .unwrap();
        if count < 6000 {
            assert_eq!(count, 3000);
            assert_eq!(
                sqlx::query_scalar::<_, String>("SELECT active_generation FROM pull_file_facets")
                    .fetch_one(&mut db)
                    .await
                    .unwrap(),
                active
            );
            return;
        }
    }
    panic!("obsolete generation remained stranded above bounded eviction budget");
}

#[tokio::test]
async fn replacing_native_blob_artifact_reclaims_only_unreferenced_unprotected_objects() {
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
    sqlx::query("INSERT INTO pull_file_blob_objects VALUES('a','native-blob',NULL,'application/octet-stream',x'010203')").execute(&mut db).await.unwrap();
    let mut blob = artifact(&m, "");
    blob.content_state = PullFileContentState::Binary;
    blob.unified_text = None;
    blob.blob_references.new = Some("native-blob".into());
    blob.content_type = Some("application/octet-stream".into());
    blob.logical_bytes = "3".into();
    blob.on_disk_bytes = "3".into();
    store
        .apply_pull_file_artifact(req.clone(), m.clone(), blob)
        .await
        .unwrap();
    store
        .apply_pull_file_artifact(req, m.clone(), artifact(&m, "text"))
        .await
        .unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM pull_file_blob_objects")
            .fetch_one(&mut db)
            .await
            .unwrap(),
        0
    );
}

#[tokio::test]
async fn unchanged_body_range_title_edit_and_304_keep_active_file_and_artifact_authority() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path().join("cache.db")).await.unwrap();
    let a = seed(&store, "a").await;
    let saved = publish(&store, &a, &["a"]).await;
    let req = request(&saved, &a);
    let membership = store
        .verify_pull_file_membership(req.clone())
        .await
        .unwrap();
    store
        .apply_pull_file_artifact(
            req.clone(),
            membership.clone(),
            artifact(&membership, "cached diff"),
        )
        .await
        .unwrap();
    let lease = store
        .begin_pull_files("a", &a.authorization_epoch, "pull", source())
        .await
        .unwrap();
    let mut changed = range_observation(&store, &a, BASE, HEAD, "fork-2").await;
    let metadata = changed.metadata.as_mut().unwrap();
    metadata.values.title = Some("new title".into());
    metadata.fields.push(MetadataObservedField {
        field: MetadataField::Title,
        state: DetailValueState::Known,
    });
    changed.body.text = Some("new description".into());
    store.apply_detail(changed).await.unwrap();
    assert_eq!(snapshot(&store).await.context, saved.context);
    assert_eq!(snapshot(&store).await.facet_revision, saved.facet_revision);
    assert_eq!(
        store
            .resume_pull_files("a", &a.authorization_epoch, "pull")
            .await
            .unwrap(),
        Some(lease.clone())
    );
    let mut unchanged = range_observation(&store, &a, BASE, HEAD, "fork-2").await;
    unchanged.not_modified = true;
    unchanged.body = DetailValue::default();
    unchanged.metadata = None;
    store.apply_detail(unchanged).await.unwrap();
    assert_eq!(snapshot(&store).await.files, saved.files);
    assert_eq!(
        store
            .verify_pull_file_membership(req.clone())
            .await
            .unwrap(),
        membership
    );
    assert_eq!(
        store
            .pull_file_artifact(req)
            .await
            .unwrap()
            .artifact
            .unwrap()
            .unified_text
            .as_deref(),
        Some("cached diff")
    );
    store
        .apply_pull_files(commit(&store, &lease, &["a"], None).await)
        .await
        .unwrap();
}

#[tokio::test]
async fn range_fact_changes_and_omission_then_recovery_do_not_revive_old_generations() {
    for transition in ["base", "source", "merge_base", "omission"] {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open(dir.path().join("cache.db")).await.unwrap();
        let a = seed(&store, "a").await;
        let saved = publish(&store, &a, &["a"]).await;
        let req = request(&saved, &a);
        let lease = store
            .begin_pull_files("a", &a.authorization_epoch, "pull", source())
            .await
            .unwrap();
        let inflight = commit(&store, &lease, &["new"], None).await;
        let mut changed = range_observation(&store, &a, BASE, HEAD, "fork-2").await;
        let metadata = changed.metadata.as_mut().unwrap();
        match transition {
            "base" => {
                metadata.values.base.as_mut().unwrap().oid =
                    "cccccccccccccccccccccccccccccccccccccccc".into()
            }
            "source" => {
                metadata
                    .values
                    .head
                    .as_mut()
                    .unwrap()
                    .repository
                    .as_mut()
                    .unwrap()
                    .provider_id = "different-fork".into()
            }
            "merge_base" => {
                metadata.values.merge_base_oid =
                    Some("dddddddddddddddddddddddddddddddddddddddd".into());
                metadata.fields.push(MetadataObservedField {
                    field: MetadataField::MergeBase,
                    state: DetailValueState::Known,
                });
            }
            _ => {
                metadata.values = ResourceMetadataValues::default();
                for field in &mut metadata.fields {
                    field.state = DetailValueState::Omitted;
                }
            }
        }
        store.apply_detail(changed).await.unwrap();
        // No Files query occurs between the changed and restored observations.
        hydrate_range(&store, &a, BASE, HEAD, "fork-2").await;
        assert!(snapshot(&store).await.files.is_empty(), "{transition}");
        assert!(
            store.verify_pull_file_membership(req).await.is_err(),
            "{transition}"
        );
        assert!(
            store.apply_pull_files(inflight).await.is_err(),
            "{transition}"
        );
        assert_eq!(
            store
                .resume_pull_files("a", &a.authorization_epoch, "pull")
                .await
                .unwrap(),
            None,
            "{transition}"
        );
        assert_eq!(publish(&store, &a, &["fresh"]).await.files.len(), 1);
    }
}

#[tokio::test]
async fn summary_head_away_and_back_is_fenced_without_a_body_or_files_query_between() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path().join("cache.db")).await.unwrap();
    let a = seed(&store, "a").await;
    let saved = publish(&store, &a, &["a"]).await;
    let req = request(&saved, &a);
    let mut subject = store
        .pull_file_selection(req.clone())
        .await
        .unwrap()
        .subject;
    let lease = store
        .begin_pull_files("a", &a.authorization_epoch, "pull", source())
        .await
        .unwrap();
    let inflight = commit(&store, &lease, &["old run"], None).await;
    for (head, at) in [
        (
            "cccccccccccccccccccccccccccccccccccccccc",
            "2099-01-01T01:00:00Z",
        ),
        (HEAD, "2099-01-01T02:00:00Z"),
    ] {
        subject.head_oid = Some(head.into());
        subject.updated_at = at.into();
        let run_id = store
            .begin_sync("a", &a.authorization_epoch, "repo:repo:pull_request")
            .await
            .unwrap();
        store
            .apply_page(PageCommit {
                account_id: "a".into(),
                authorization_epoch: a.authorization_epoch.clone(),
                scope: "repo:repo:pull_request".into(),
                run_id,
                repositories: vec![],
                items: vec![subject.clone()],
                endpoint_aliases: vec![],
                next_cursor: None,
                etag: None,
                last_modified: None,
                not_modified: false,
                complete: true,
                observed_at: at.into(),
            })
            .await
            .unwrap();
    }
    assert!(snapshot(&store).await.files.is_empty());
    assert!(store.verify_pull_file_membership(req).await.is_err());
    assert!(store.apply_pull_files(inflight).await.is_err());
}

#[tokio::test]
async fn body_access_denial_recovery_cannot_rebind_old_generation_to_the_new_view() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path().join("cache.db")).await.unwrap();
    let a = seed(&store, "a").await;
    let saved = publish(&store, &a, &["a"]).await;
    let req = request(&saved, &a);
    let lease = store
        .begin_pull_files("a", &a.authorization_epoch, "pull", source())
        .await
        .unwrap();
    store
        .set_sync_status(
            "a",
            &a.authorization_epoch,
            "detail:pull:body",
            SyncStatus {
                state: SyncState::Error,
                last_success_at: None,
                next_retry_at: None,
                error: Some(CollaborationError::new(
                    ErrorCode::PermissionDenied,
                    "Denied fixture",
                )),
            },
        )
        .await
        .unwrap();
    hydrate_range(&store, &a, BASE, HEAD, "fork-2").await;
    assert!(snapshot(&store).await.files.is_empty());
    assert!(store.verify_pull_file_membership(req).await.is_err());
    assert!(store.pull_file_request(&lease).await.is_err());
    assert_eq!(publish(&store, &a, &["recovered"]).await.files.len(), 1);
}

#[tokio::test]
async fn independent_terminal_cap_evidence_keeps_page_reason_and_stronger_known_more() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path().join("cache.db")).await.unwrap();
    let a = seed(&store, "a").await;
    let lease = store
        .begin_pull_files("a", &a.authorization_epoch, "pull", source())
        .await
        .unwrap();
    let mut value = commit(&store, &lease, &["a"], None).await;
    value.page.cap = Some(PullFileCapEvidence {
        provenance: PullFileCapProvenance::Local,
        reason: PullFileCapReason::LocalPageLimit,
        remote_has_more: PullFileFlag::Unknown,
    });
    value.collection_cap = Some(PullFileCapEvidence {
        provenance: PullFileCapProvenance::Provider,
        reason: PullFileCapReason::ProviderOverflow,
        remote_has_more: PullFileFlag::Known(true),
    });
    store.apply_pull_files(value).await.unwrap();
    let cap = snapshot(&store).await.completeness.cap.unwrap();
    assert_eq!(cap.reason, PullFileCapReason::LocalPageLimit);
    assert_eq!(cap.remote_has_more, PullFileFlag::Known(true));
}
