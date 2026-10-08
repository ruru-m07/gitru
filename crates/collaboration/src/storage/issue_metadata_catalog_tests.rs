use super::*;
use crate::runtime::detail_tests::fixtures;

async fn setup() -> (tempfile::TempDir, Store, RemoteAccount) {
    let directory = tempfile::tempdir().unwrap();
    let store = Store::open(directory.path().join("catalog.sqlite"))
        .await
        .unwrap();
    let mut account = fixtures::account("a");
    account.actor_id = "7".into();
    let account = store.upsert_account(account).await.unwrap();
    fixtures::project(&store, &account).await;
    (directory, store, account)
}
fn now() -> String {
    chrono::Utc::now().to_rfc3339()
}
fn query(family: IssueMetadataKind) -> IssueMetadataQuery {
    IssueMetadataQuery {
        account_id: "a".into(),
        repository_id: "repo".into(),
        kind: family,
        search: String::new(),
        cursor: None,
        limit: 50,
    }
}
fn label(id: u64, text: &str) -> IssueMetadataOption {
    IssueMetadataOption {
        reference: IssueMetadataReference::Label(crate::IssueMetadataLabel {
            provider_id: id.to_string(),
            name: text.into(),
            color: Some("0123ab".into()),
        }),
        availability: IssueMetadataAvailability::Available,
        reason: None,
    }
}
fn page(options: Vec<IssueMetadataOption>, next: Option<&str>) -> IssueMetadataCatalogPage {
    IssueMetadataCatalogPage {
        options,
        next_cursor: next.map(str::to_owned),
        truncated: false,
        coverage: CoverageState::Partial,
        cooldown_seconds: None,
    }
}
async fn begin(store: &Store, a: &RemoteAccount) -> CatalogLease {
    store
        .begin_issue_metadata(
            &a.id,
            &a.authorization_epoch,
            "repo",
            IssueMetadataKind::Labels,
        )
        .await
        .unwrap()
}
async fn count(store: &Store, table: &str) -> i64 {
    let sql = match table {
        "options" => "SELECT count(*) FROM repository_metadata_options WHERE account_id='a'",
        "catalogs" => "SELECT count(*) FROM repository_metadata_catalogs WHERE account_id='a'",
        _ => panic!("fixed table only"),
    };
    sqlx::query_scalar(sql)
        .fetch_one(&store.inner.readers)
        .await
        .unwrap()
}
async fn repository(store: &Store, a: &RemoteAccount, id: &str, native: &str) {
    let run = store
        .begin_sync(&a.id, &a.authorization_epoch, "repositories")
        .await
        .unwrap();
    store
        .apply_page(PageCommit {
            account_id: a.id.clone(),
            authorization_epoch: a.authorization_epoch.clone(),
            scope: "repositories".into(),
            run_id: run,
            repositories: vec![RemoteRepository {
                id: id.into(),
                account_id: a.id.clone(),
                provider_id: native.into(),
                full_name: format!("owner/{id}"),
                name: id.into(),
                web_url: format!("https://github.com/owner/{id}"),
                description: None,
                default_branch: None,
                selected: true,
            }],
            items: vec![],
            endpoint_aliases: vec![],
            next_cursor: Some("still-discovering".into()),
            etag: None,
            last_modified: None,
            not_modified: false,
            complete: false,
            observed_at: now(),
        })
        .await
        .unwrap();
}

#[tokio::test]
async fn cached_catalog_is_sorted_bounded_searchable_and_independent_of_global_revision() {
    let (_dir, store, a) = setup().await;
    let missing = store
        .issue_metadata_options(query(IssueMetadataKind::Labels))
        .await
        .unwrap();
    assert_eq!(missing.coverage.state, CoverageState::Missing);
    let l = begin(&store, &a).await;
    store
        .apply_issue_metadata(
            &l,
            page(
                vec![label(3, "Zebra"), label(2, "beta"), label(1, "Alpha")],
                None,
            ),
            &now(),
        )
        .await
        .unwrap();
    let mut q = query(IssueMetadataKind::Labels);
    q.limit = 1;
    let first = store.issue_metadata_options(q.clone()).await.unwrap();
    assert_eq!(first.options, vec![label(1, "Alpha")]);
    assert_eq!(first.coverage.state, CoverageState::Partial);
    assert_eq!(first.freshness, DetailFreshness::Fresh);
    q.cursor = first.next_cursor;
    store
        .save_draft(LocalDraft {
            account_id: "a".into(),
            subject_id: "pull".into(),
            body: "Unrelated authored change".into(),
            generation: "0".into(),
        })
        .await
        .unwrap();
    let second = store.issue_metadata_options(q.clone()).await.unwrap();
    assert_eq!(second.options, vec![label(2, "beta")]);
    assert_ne!(first.revision, second.revision);
    assert_eq!(first.catalog_revision, second.catalog_revision);
    q.cursor = second.next_cursor;
    assert_eq!(
        store.issue_metadata_options(q).await.unwrap().options,
        vec![label(3, "Zebra")]
    );
    let mut q = query(IssueMetadataKind::Labels);
    q.search = "ALP".into();
    assert_eq!(
        store.issue_metadata_options(q).await.unwrap().options,
        vec![label(1, "Alpha")]
    );
    assert!(
        store
            .issue_metadata_options(query(IssueMetadataKind::Assignees))
            .await
            .unwrap()
            .options
            .is_empty()
    );
}

#[tokio::test]
async fn cold_resume_preserves_exact_generation_cursor_and_retires_old_lease_revision() {
    let (dir, store, a) = setup().await;
    let l = begin(&store, &a).await;
    let old = store
        .apply_issue_metadata(
            &l,
            page(vec![label(1, "one")], Some("native-page-two")),
            &now(),
        )
        .await
        .unwrap()
        .next_lease
        .unwrap();
    store.close().await.unwrap();
    let store = Store::open(dir.path().join("catalog.sqlite"))
        .await
        .unwrap();
    let resumed = store
        .resume_issue_metadata(
            "a",
            &a.authorization_epoch,
            "repo",
            IssueMetadataKind::Labels,
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        resumed.request.catalog_generation,
        old.request.catalog_generation
    );
    assert_eq!(resumed.request.cursor, old.request.cursor);
    assert_eq!(resumed.page_count, 1);
    assert_ne!(resumed.catalog_revision, old.catalog_revision);
    assert_eq!(
        store.issue_metadata_request(&old).await.unwrap_err().code,
        ErrorCode::StaleView
    );
    assert_eq!(
        store
            .issue_metadata_options(query(IssueMetadataKind::Labels))
            .await
            .unwrap()
            .sync
            .state,
        SyncState::Idle
    );
    store
        .apply_issue_metadata(&resumed, page(vec![label(2, "two")], None), &now())
        .await
        .unwrap();
    assert_eq!(
        store
            .issue_metadata_options(query(IssueMetadataKind::Labels))
            .await
            .unwrap()
            .options
            .len(),
        2
    );
}

#[tokio::test]
async fn empty_refresh_and_terminal_cap_preserve_observations_and_private_draft() {
    let (_dir, store, a) = setup().await;
    let draft = store
        .save_draft(LocalDraft {
            account_id: "a".into(),
            subject_id: "pull".into(),
            body: "retained private note".into(),
            generation: "0".into(),
        })
        .await
        .unwrap();
    let l = begin(&store, &a).await;
    store
        .apply_issue_metadata(&l, page(vec![label(1, "retained")], None), &now())
        .await
        .unwrap();
    let l = begin(&store, &a).await;
    store
        .apply_issue_metadata(&l, page(vec![], None), &now())
        .await
        .unwrap();
    let saved = store
        .issue_metadata_options(query(IssueMetadataKind::Labels))
        .await
        .unwrap();
    assert_eq!(saved.options, vec![label(1, "retained")]);
    assert_eq!(saved.freshness, DetailFreshness::Stale);
    let mut l = begin(&store, &a).await;
    for i in 1..20 {
        l = store
            .apply_issue_metadata(&l, page(vec![], Some(&format!("page-{i}"))), &now())
            .await
            .unwrap()
            .next_lease
            .unwrap();
    }
    assert_eq!(
        store
            .apply_issue_metadata(&l, page(vec![], Some("page-21")), &now())
            .await
            .unwrap_err()
            .code,
        ErrorCode::InvalidInput
    );
    let mut last = page(vec![], None);
    last.truncated = true;
    assert!(
        store
            .apply_issue_metadata(&l, last, &now())
            .await
            .unwrap()
            .next_lease
            .is_none()
    );
    let saved = store
        .issue_metadata_options(query(IssueMetadataKind::Labels))
        .await
        .unwrap();
    assert!(saved.coverage.remote_has_more);
    assert_eq!(saved.coverage.state, CoverageState::Partial);
    assert!(
        store
            .resume_issue_metadata(
                "a",
                &a.authorization_epoch,
                "repo",
                IssueMetadataKind::Labels
            )
            .await
            .unwrap()
            .is_none()
    );
    let fresh = begin(&store, &a).await;
    assert!(fresh.request.cursor.is_none());
    assert_eq!(fresh.page_count, 0);
    assert_ne!(
        fresh.request.catalog_generation,
        l.request.catalog_generation
    );
    assert_eq!(store.draft("a", "pull").await.unwrap(), Some(draft));
}

#[tokio::test]
async fn family_view_epoch_and_repository_binding_fence_cursors_and_held_pages() {
    let (_dir, store, a) = setup().await;
    let old = begin(&store, &a).await;
    let next = store
        .apply_issue_metadata(
            &old,
            page(vec![label(1, "a"), label(2, "b")], Some("p2")),
            &now(),
        )
        .await
        .unwrap()
        .next_lease
        .unwrap();
    let mut q = query(IssueMetadataKind::Labels);
    q.limit = 1;
    q.cursor = store
        .issue_metadata_options(q.clone())
        .await
        .unwrap()
        .next_cursor;
    let mut foreign = q.clone();
    foreign.kind = IssueMetadataKind::Milestones;
    assert_eq!(
        store
            .issue_metadata_options(foreign)
            .await
            .unwrap_err()
            .code,
        ErrorCode::StaleView
    );
    let mut foreign = q.clone();
    foreign.search = "different".into();
    assert_eq!(
        store
            .issue_metadata_options(foreign)
            .await
            .unwrap_err()
            .code,
        ErrorCode::StaleView
    );
    let mut writer = store.inner.writer.acquire().await.unwrap();
    sqlx::query(
        "UPDATE runtime_meta SET authorization_view=authorization_view+1 WHERE singleton=1",
    )
    .execute(&mut *writer)
    .await
    .unwrap();
    drop(writer);
    assert_eq!(
        store
            .apply_issue_metadata(&next, page(vec![label(3, "late")], None), &now())
            .await
            .unwrap_err()
            .code,
        ErrorCode::StaleView
    );
    assert_eq!(
        store.issue_metadata_options(q).await.unwrap_err().code,
        ErrorCode::StaleView
    );
    assert!(
        store
            .issue_metadata_options(query(IssueMetadataKind::Labels))
            .await
            .unwrap()
            .options
            .is_empty()
    );
    let l = begin(&store, &a).await;
    let mut writer = store.inner.writer.acquire().await.unwrap();
    sqlx::query("UPDATE repositories SET json=json_set(json,'$.full_name','owner/renamed') WHERE account_id='a' AND id='repo'").execute(&mut *writer).await.unwrap();
    drop(writer);
    assert_eq!(
        store.issue_metadata_request(&l).await.unwrap_err().code,
        ErrorCode::StaleView
    );
    assert!(
        store
            .resume_issue_metadata(
                "a",
                &a.authorization_epoch,
                "repo",
                IssueMetadataKind::Labels
            )
            .await
            .unwrap()
            .is_none()
    );
    let l = begin(&store, &a).await;
    store.disconnect("a").await.unwrap();
    assert!(
        store
            .apply_issue_metadata(&l, page(vec![label(4, "late epoch")], None), &now())
            .await
            .is_err()
    );
    assert_eq!(
        store
            .issue_metadata_options(query(IssueMetadataKind::Labels))
            .await
            .unwrap_err()
            .code,
        ErrorCode::AuthRequired
    );
}

#[tokio::test]
async fn denial_retires_held_success_and_refresh_does_not_restore_access_before_observation() {
    let (_dir, store, a) = setup().await;
    let old = begin(&store, &a).await;
    let held = store
        .apply_issue_metadata(
            &old,
            page(vec![label(1, "secret option")], Some("p2")),
            &now(),
        )
        .await
        .unwrap()
        .next_lease
        .unwrap();
    let status = SyncStatus {
        state: SyncState::Error,
        error: Some(CollaborationError::new(
            ErrorCode::PermissionDenied,
            "Catalog access denied",
        )),
        ..SyncStatus::default()
    };
    store.fail_issue_metadata(&held, status).await.unwrap();
    assert_eq!(
        store
            .apply_issue_metadata(&held, page(vec![label(2, "late")], None), &now())
            .await
            .unwrap_err()
            .code,
        ErrorCode::StaleView
    );
    assert!(
        store
            .issue_metadata_options(query(IssueMetadataKind::Labels))
            .await
            .unwrap()
            .options
            .is_empty()
    );
    let fresh = begin(&store, &a).await;
    assert!(
        store
            .issue_metadata_options(query(IssueMetadataKind::Labels))
            .await
            .unwrap()
            .options
            .is_empty()
    );
    store
        .apply_issue_metadata(&fresh, page(vec![label(1, "visible again")], None), &now())
        .await
        .unwrap();
    assert_eq!(
        store
            .issue_metadata_options(query(IssueMetadataKind::Labels))
            .await
            .unwrap()
            .options,
        vec![label(1, "visible again")]
    );
}

#[tokio::test]
async fn blocked_writer_rechecks_view_before_any_page_changes() {
    let (_dir, store, a) = setup().await;
    let l = begin(&store, &a).await;
    let mut writer = store.inner.writer.acquire().await.unwrap();
    let entered = Arc::new(tokio::sync::Notify::new());
    let task = tokio::spawn({
        let store = store.clone();
        let entered = entered.clone();
        async move {
            entered.notify_one();
            store
                .apply_issue_metadata(&l, page(vec![label(1, "late")], None), &now())
                .await
        }
    });
    entered.notified().await;
    tokio::task::yield_now().await;
    assert!(!task.is_finished());
    sqlx::query(
        "UPDATE runtime_meta SET authorization_view=authorization_view+1 WHERE singleton=1",
    )
    .execute(&mut *writer)
    .await
    .unwrap();
    drop(writer);
    assert_eq!(task.await.unwrap().unwrap_err().code, ErrorCode::StaleView);
    assert_eq!(count(&store, "options").await, 0);
}

#[tokio::test]
async fn bounds_reject_invalid_pages_atomically_and_keep_non_authoritative_unknowns() {
    let (_dir, store, a) = setup().await;
    let l = begin(&store, &a).await;
    for p in [
        page(vec![label(1, "a"), label(1, "duplicate")], None),
        page((1..=101).map(|i| label(i, "large")).collect(), None),
        page(
            vec![IssueMetadataOption {
                reference: IssueMetadataReference::Assignee(crate::IssueMetadataAssignee {
                    provider_id: "1".into(),
                    login: "user".into(),
                }),
                availability: IssueMetadataAvailability::Available,
                reason: None,
            }],
            None,
        ),
    ] {
        assert_eq!(
            store
                .apply_issue_metadata(&l, p, &now())
                .await
                .unwrap_err()
                .code,
            ErrorCode::InvalidInput
        );
        assert_eq!(count(&store, "options").await, 0);
    }
    let mut complete = page(vec![], None);
    complete.coverage = CoverageState::Complete;
    assert_eq!(
        store
            .apply_issue_metadata(&l, complete, &now())
            .await
            .unwrap_err()
            .code,
        ErrorCode::InvalidInput
    );
    let mut archived = label(1, "archived");
    archived.availability = IssueMetadataAvailability::Unavailable;
    archived.reason = Some(IssueMetadataReason::Archived);
    let mut unknown = label(2, "unknown");
    unknown.availability = IssueMetadataAvailability::Unknown;
    unknown.reason = Some(IssueMetadataReason::Unobserved);
    store
        .apply_issue_metadata(
            &l,
            page(vec![archived.clone(), unknown.clone()], None),
            &now(),
        )
        .await
        .unwrap();
    assert_eq!(
        store
            .issue_metadata_options(query(IssueMetadataKind::Labels))
            .await
            .unwrap()
            .options,
        vec![archived, unknown]
    );
    for bad in ["\0".to_owned(), "x".repeat(1025)] {
        let mut q = query(IssueMetadataKind::Labels);
        q.account_id = bad;
        assert_eq!(
            store.issue_metadata_options(q).await.unwrap_err().code,
            ErrorCode::InvalidInput
        );
    }
}

#[tokio::test]
async fn deselection_and_reselection_cannot_revive_old_lease() {
    let (_dir, store, a) = setup().await;
    let old = begin(&store, &a).await;
    store.select_repository("a", "repo", false).await.unwrap();
    assert_eq!(
        store.issue_metadata_request(&old).await.unwrap_err().code,
        ErrorCode::StaleView
    );
    store.select_repository("a", "repo", true).await.unwrap();
    assert_eq!(
        store
            .apply_issue_metadata(&old, page(vec![label(1, "late")], None), &now())
            .await
            .unwrap_err()
            .code,
        ErrorCode::StaleView
    );
}

#[tokio::test]
async fn cache_retention_caps_rows_and_headers_without_reusing_run_ids_or_local_cursors() {
    let (_dir, store, a) = setup().await;
    let mut initial_cursor = None;
    for round in 0..2 {
        let mut l = begin(&store, &a).await;
        for index in 0..20 {
            let start = round * 2000 + index * 100 + 1;
            let options = (start..start + 100)
                .map(|i| label(i, "same-name"))
                .collect();
            let next = (index < 19).then(|| format!("{round}-{index}"));
            let receipt = store
                .apply_issue_metadata(&l, page(options, next.as_deref()), &now())
                .await
                .unwrap();
            if let Some(next) = receipt.next_lease {
                l = next;
            }
        }
        if round == 0 {
            let mut q = query(IssueMetadataKind::Labels);
            q.limit = 1;
            initial_cursor = store.issue_metadata_options(q).await.unwrap().next_cursor;
        }
    }
    assert_eq!(count(&store, "options").await, 2000);
    let mut old = query(IssueMetadataKind::Labels);
    old.cursor = initial_cursor;
    assert_eq!(
        store.issue_metadata_options(old).await.unwrap_err().code,
        ErrorCode::StaleView
    );
    assert!(
        store
            .resume_issue_metadata(
                "a",
                &a.authorization_epoch,
                "repo",
                IssueMetadataKind::Labels
            )
            .await
            .unwrap()
            .is_none()
    );
    assert_eq!(
        store
            .issue_metadata_options(query(IssueMetadataKind::Labels))
            .await
            .unwrap()
            .freshness,
        DetailFreshness::Stale
    );
    let original = begin(&store, &a).await;
    for i in 1..=48 {
        let id = format!("repo{i}");
        repository(&store, &a, &id, &(i + 1).to_string()).await;
        store
            .begin_issue_metadata("a", &a.authorization_epoch, &id, IssueMetadataKind::Labels)
            .await
            .unwrap();
    }
    assert_eq!(count(&store, "catalogs").await, 48);
    assert_eq!(
        store
            .issue_metadata_request(&original)
            .await
            .unwrap_err()
            .code,
        ErrorCode::StaleView
    );
    let recreated = begin(&store, &a).await;
    assert_ne!(
        recreated.request.catalog_generation,
        original.request.catalog_generation
    );
    assert_eq!(count(&store, "catalogs").await, 48);
}

#[tokio::test]
async fn account_retention_evicts_cache_only_and_invalidates_the_affected_family_cursor() {
    let (_dir, store, a) = setup().await;
    let draft = store
        .save_draft(LocalDraft {
            account_id: "a".into(),
            subject_id: "pull".into(),
            body: "authored data never participates in catalog eviction".into(),
            generation: "0".into(),
        })
        .await
        .unwrap();
    let mut old_cursor = None;
    for index in 0..4 {
        let repo = if index == 0 {
            "repo".to_owned()
        } else {
            format!("more{index}")
        };
        if index > 0 {
            repository(&store, &a, &repo, &(index + 10).to_string()).await;
        }
        let mut l = store
            .begin_issue_metadata(
                "a",
                &a.authorization_epoch,
                &repo,
                IssueMetadataKind::Labels,
            )
            .await
            .unwrap();
        for p in 0..20 {
            let rows = (p * 100 + 1..p * 100 + 101)
                .map(|n| label(n, "cached"))
                .collect();
            let next = (p < 19).then(|| format!("next{p}"));
            let result = store
                .apply_issue_metadata(&l, page(rows, next.as_deref()), &now())
                .await
                .unwrap();
            if let Some(next) = result.next_lease {
                l = next;
            }
        }
        if index == 0 {
            let mut q = query(IssueMetadataKind::Labels);
            q.limit = 1;
            old_cursor = store.issue_metadata_options(q).await.unwrap().next_cursor;
        }
    }
    assert_eq!(count(&store, "options").await, 6000);
    let mut q = query(IssueMetadataKind::Labels);
    q.cursor = old_cursor;
    assert_eq!(
        store.issue_metadata_options(q).await.unwrap_err().code,
        ErrorCode::StaleView
    );
    let saved = store
        .issue_metadata_options(query(IssueMetadataKind::Labels))
        .await
        .unwrap();
    assert!(saved.options.is_empty());
    assert_eq!(saved.coverage.state, CoverageState::Partial);
    assert_eq!(saved.freshness, DetailFreshness::Stale);
    assert!(
        store
            .resume_issue_metadata(
                "a",
                &a.authorization_epoch,
                "repo",
                IssueMetadataKind::Labels
            )
            .await
            .unwrap()
            .is_none()
    );
    assert_eq!(store.draft("a", "pull").await.unwrap(), Some(draft));
}

#[tokio::test]
async fn failed_status_cold_reopen_keeps_checkpoint_and_never_invents_success() {
    let (dir, store, a) = setup().await;
    let l = begin(&store, &a).await;
    let next = store
        .apply_issue_metadata(&l, page(vec![label(1, "saved")], Some("page-two")), &now())
        .await
        .unwrap()
        .next_lease
        .unwrap();
    let retry = (chrono::Utc::now() + chrono::Duration::seconds(60)).to_rfc3339();
    store
        .fail_issue_metadata(
            &next,
            SyncStatus {
                state: SyncState::RateLimited,
                last_success_at: Some("bad caller claim".into()),
                next_retry_at: Some(retry.clone()),
                error: Some(CollaborationError::new(
                    ErrorCode::RateLimited,
                    "Fixture quota",
                )),
            },
        )
        .await
        .unwrap();
    store.close().await.unwrap();
    let store = Store::open(dir.path().join("catalog.sqlite"))
        .await
        .unwrap();
    let saved = store
        .issue_metadata_options(query(IssueMetadataKind::Labels))
        .await
        .unwrap();
    assert_eq!(saved.sync.state, SyncState::RateLimited);
    assert_eq!(saved.sync.next_retry_at, Some(retry));
    assert_ne!(
        saved.sync.last_success_at.as_deref(),
        Some("bad caller claim")
    );
    let resumed = store
        .resume_issue_metadata(
            "a",
            &a.authorization_epoch,
            "repo",
            IssueMetadataKind::Labels,
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(resumed.request.cursor, next.request.cursor);
    assert_eq!(
        resumed.request.catalog_generation,
        next.request.catalog_generation
    );
    assert_ne!(resumed.catalog_revision, next.catalog_revision);
}
