use collaboration::{domain::*, error::ErrorCode, storage::Store};
use sqlx::{Connection, Row, sqlite::SqliteConnectOptions};

fn account(id: &str) -> RemoteAccount {
    RemoteAccount {
        id: id.into(),
        provider: ProviderKind::Github,
        host: "github.com".into(),
        actor_id: id.into(),
        login: id.into(),
        display_name: None,
        authorization_epoch: "1".into(),
        state: AccountState::Active,
        notifications_supported: true,
    }
}
fn repository(account_id: &str) -> RemoteRepository {
    RemoteRepository {
        id: format!("repo-{account_id}"),
        account_id: account_id.into(),
        provider_id: "42".into(),
        full_name: "owner/project".into(),
        name: "project".into(),
        web_url: "https://github.com/owner/project".into(),
        description: None,
        default_branch: Some("main".into()),
        selected: false,
    }
}
fn item(account_id: &str, id: &str) -> RemoteItem {
    RemoteItem {
        id: id.into(),
        account_id: account_id.into(),
        repository_id: Some(format!("repo-{account_id}")),
        provider_id: id.into(),
        kind: RemoteItemKind::PullRequest,
        number: Some("67".into()),
        title: format!("Local fast {id}"),
        body: Some("cached confidential body".into()),
        body_omitted: false,
        author: Some("author".into()),
        web_url: None,
        state: "open".into(),
        updated_at: "2026-10-02T12:00:00Z".into(),
        head_oid: Some("head".into()),
        is_draft: Some(false),
        reason: None,
        unread: None,
    }
}
fn query(account_id: &str) -> ItemQuery {
    ItemQuery {
        account_id: account_id.into(),
        kind: RemoteItemKind::PullRequest,
        repository_id: None,
        state: None,
        search: None,
        cursor: None,
        limit: 100,
    }
}
async fn connect(store: &Store, account_id: &str) {
    store.upsert_account(account(account_id)).await.unwrap();
    let run_id = store
        .begin_sync(account_id, "1", "repositories")
        .await
        .unwrap();
    store
        .apply_page(PageCommit {
            account_id: account_id.into(),
            authorization_epoch: "1".into(),
            scope: "repositories".into(),
            run_id,
            repositories: vec![repository(account_id)],
            items: vec![],
            next_cursor: None,
            etag: None,
            last_modified: None,
            not_modified: false,
            complete: true,
            observed_at: "2026-10-02T12:00:00Z".into(),
        })
        .await
        .unwrap();
    store
        .select_repository(account_id, &format!("repo-{account_id}"), true)
        .await
        .unwrap();
}
async fn run(store: &Store, account_id: &str) -> String {
    store
        .begin_sync(
            account_id,
            "1",
            &format!("repo:repo-{account_id}:pull_request"),
        )
        .await
        .unwrap()
}
fn page(account_id: &str, run_id: &str, items: Vec<RemoteItem>, complete: bool) -> PageCommit {
    PageCommit {
        account_id: account_id.into(),
        authorization_epoch: "1".into(),
        scope: format!("repo:repo-{account_id}:pull_request"),
        run_id: run_id.into(),
        repositories: vec![],
        items,
        next_cursor: if complete {
            None
        } else {
            Some("provider-page-2".into())
        },
        etag: Some("validator".into()),
        last_modified: None,
        not_modified: false,
        complete,
        observed_at: "2026-10-02T12:00:00Z".into(),
    }
}

#[tokio::test]
async fn offline_reopen_retains_local_rows_coverage_and_drafts() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("collaboration.db");
    {
        let store = Store::open(&path).await.unwrap();
        connect(&store, "a").await;
        let run_id = run(&store, "a").await;
        store
            .apply_page(page("a", &run_id, vec![item("a", "pr")], true))
            .await
            .unwrap();
        store
            .save_draft(LocalDraft {
                account_id: "a".into(),
                subject_id: "pr".into(),
                body: "unsent intent".into(),
                generation: "0".into(),
            })
            .await
            .unwrap();
        store
            .set_sync_status(
                "a",
                "1",
                "repo:repo-a:pull_request",
                SyncStatus {
                    state: SyncState::Offline,
                    last_success_at: Some("2026-10-02T12:00:00Z".into()),
                    next_retry_at: None,
                    error: None,
                },
            )
            .await
            .unwrap();
        store.close().await;
    }
    let store = Store::open(&path).await.unwrap();
    let snapshot = store.query_items(query("a")).await.unwrap();
    assert_eq!(snapshot.items.len(), 1);
    assert_eq!(snapshot.coverage.state, CoverageState::Complete);
    assert_eq!(snapshot.sync.state, SyncState::Offline);
    assert_eq!(
        store.draft("a", "pr").await.unwrap().unwrap().body,
        "unsent intent"
    );
    let mut connection = sqlx::SqliteConnection::connect_with(
        &SqliteConnectOptions::new().filename(&path).read_only(true),
    )
    .await
    .unwrap();
    let version: String = sqlx::query_scalar("SELECT sqlite_version()")
        .fetch_one(&mut connection)
        .await
        .unwrap();
    assert_eq!(version, "3.51.3");
    let mode: String = sqlx::query_scalar("PRAGMA journal_mode")
        .fetch_one(&mut connection)
        .await
        .unwrap();
    assert_eq!(mode, "wal");
    let foreign_keys: i64 = sqlx::query_scalar("PRAGMA foreign_keys")
        .fetch_one(&mut connection)
        .await
        .unwrap();
    assert_eq!(foreign_keys, 1);
    let check: String = sqlx::query_scalar("PRAGMA integrity_check")
        .fetch_one(&mut connection)
        .await
        .unwrap();
    assert_eq!(check, "ok");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(path).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }
}

#[tokio::test]
async fn revocation_rejects_old_responses_hides_content_and_preserves_draft() {
    let temp = tempfile::tempdir().unwrap();
    let store = Store::open(temp.path().join("db")).await.unwrap();
    connect(&store, "a").await;
    let run_id = run(&store, "a").await;
    store
        .apply_page(page("a", &run_id, vec![item("a", "pr")], true))
        .await
        .unwrap();
    let before = store.query_items(query("a")).await.unwrap();
    store
        .save_draft(LocalDraft {
            account_id: "a".into(),
            subject_id: "pr".into(),
            body: "draft survives".into(),
            generation: "0".into(),
        })
        .await
        .unwrap();
    store.disconnect("a").await.unwrap();
    let revision = store.revision().await.unwrap();
    assert_eq!(
        store
            .apply_page(page("a", &run_id, vec![item("a", "late")], true))
            .await
            .unwrap_err()
            .code,
        ErrorCode::StaleView
    );
    assert_eq!(revision, store.revision().await.unwrap());
    assert_eq!(
        store.query_items(query("a")).await.unwrap_err().code,
        ErrorCode::AuthRequired
    );
    let changes = store.changes_since(&before.revision).await.unwrap();
    assert!(changes.reset_required);
    assert_ne!(changes.authorization_view, before.authorization_view);
    let mut draft = store.draft("a", "pr").await.unwrap().unwrap();
    draft.body = "offline edit".into();
    assert_eq!(store.save_draft(draft).await.unwrap().generation, "2");
    let mut reconnect = account("a");
    reconnect.authorization_epoch = "3".into();
    store.upsert_account(reconnect).await.unwrap();
    assert!(
        store
            .query_items(query("a"))
            .await
            .unwrap()
            .items
            .is_empty()
    );
    assert_eq!(
        store.draft("a", "pr").await.unwrap().unwrap().body,
        "offline edit"
    );
}

#[tokio::test]
async fn pages_commit_rows_and_checkpoints_atomically_without_inferred_deletes() {
    let temp = tempfile::tempdir().unwrap();
    let store = Store::open(temp.path().join("db")).await.unwrap();
    connect(&store, "a").await;
    let run_id = run(&store, "a").await;
    store
        .apply_page(page("a", &run_id, vec![item("a", "first")], false))
        .await
        .unwrap();
    let checkpoint = store
        .scope_state("a", "repo:repo-a:pull_request")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(checkpoint.next_cursor.as_deref(), Some("provider-page-2"));
    assert_eq!(checkpoint.coverage.state, CoverageState::Partial);
    assert_eq!(checkpoint.coverage.validated_at, None);
    let before = store.revision().await.unwrap();
    let mut wrong = item("a", "cross-account");
    wrong.account_id = "b".into();
    assert_eq!(
        store
            .apply_page(page(
                "a",
                &run_id,
                vec![item("a", "would-write"), wrong],
                true
            ))
            .await
            .unwrap_err()
            .code,
        ErrorCode::InvalidInput
    );
    assert_eq!(before, store.revision().await.unwrap());
    assert_eq!(store.query_items(query("a")).await.unwrap().items.len(), 1);
    assert_eq!(
        store
            .scope_state("a", "repo:repo-a:pull_request")
            .await
            .unwrap()
            .unwrap()
            .coverage
            .state,
        CoverageState::Partial
    );
    store
        .apply_page(page("a", &run_id, vec![item("a", "second")], true))
        .await
        .unwrap();
    assert_eq!(
        store.query_items(query("a")).await.unwrap().coverage.state,
        CoverageState::Complete
    );
    let next_run = run(&store, "a").await;
    assert_eq!(
        store
            .apply_page(page("a", &run_id, vec![item("a", "obsolete-run")], true))
            .await
            .unwrap_err()
            .code,
        ErrorCode::StaleView
    );
    store
        .apply_page(page("a", &next_run, vec![item("a", "second")], true))
        .await
        .unwrap();
    store
        .apply_page(page("a", &next_run, vec![item("a", "second")], true))
        .await
        .unwrap();
    assert_eq!(
        store.query_items(query("a")).await.unwrap().items.len(),
        2,
        "One mutable traversal miss preserves membership"
    );
    let next_run = run(&store, "a").await;
    store
        .apply_page(page("a", &next_run, vec![item("a", "second")], true))
        .await
        .unwrap();
    // Replaying a committed final page is not a second absent traversal.
    store
        .apply_page(page("a", &next_run, vec![item("a", "second")], true))
        .await
        .unwrap();
    assert_eq!(
        store.query_items(query("a")).await.unwrap().items.len(),
        1,
        "Two complete traversals exclude absent scope membership"
    );
    assert!(
        store.item("a", "first").await.unwrap().item.is_some(),
        "A list miss cannot delete a provider object"
    );
    let next_run = run(&store, "a").await;
    store
        .apply_page(page(
            "a",
            &next_run,
            vec![item("a", "first"), item("a", "second")],
            true,
        ))
        .await
        .unwrap();
    assert_eq!(
        store.query_items(query("a")).await.unwrap().items.len(),
        2,
        "A new observation reactivates membership"
    );
}

#[tokio::test]
async fn account_partition_search_selection_and_cursor_fences() {
    let temp = tempfile::tempdir().unwrap();
    let store = Store::open(temp.path().join("db")).await.unwrap();
    connect(&store, "a").await;
    connect(&store, "b").await;
    for id in ["a", "b"] {
        let run_id = run(&store, id).await;
        store
            .apply_page(page(
                id,
                &run_id,
                vec![item(id, "same-id"), item(id, "second")],
                true,
            ))
            .await
            .unwrap();
    }
    let mut first = query("a");
    first.limit = 1;
    let snapshot = store.query_items(first.clone()).await.unwrap();
    assert_eq!(snapshot.items.len(), 1);
    first.cursor = snapshot.next_cursor.clone();
    let second = store.query_items(first.clone()).await.unwrap();
    assert_eq!(second.items.len(), 1);
    assert_ne!(second.items[0].id, snapshot.items[0].id);
    assert!(second.next_cursor.is_none());
    first.account_id = "b".into();
    assert_eq!(
        store.query_items(first).await.unwrap_err().code,
        ErrorCode::InvalidInput
    );
    let mut search = query("a");
    search.search = Some("confidential".into());
    let matches = store.query_items(search).await.unwrap();
    assert_eq!(matches.items.len(), 2);
    assert!(matches.items.iter().all(|i| i.account_id == "a"));
    let late_run = run(&store, "a").await;
    store.select_repository("a", "repo-a", false).await.unwrap();
    assert!(
        store
            .query_items(query("a"))
            .await
            .unwrap()
            .items
            .is_empty()
    );
    assert!(store.item("a", "same-id").await.unwrap().item.is_none());
    assert_eq!(
        store
            .apply_page(page("a", &late_run, vec![item("a", "late")], true))
            .await
            .unwrap_err()
            .code,
        ErrorCode::StaleView
    );
    assert_eq!(store.query_items(query("b")).await.unwrap().items.len(), 2);
    let mut continuation = query("b");
    continuation.limit = 1;
    let snapshot = store.query_items(continuation.clone()).await.unwrap();
    continuation.cursor = snapshot.next_cursor;
    store.disconnect("a").await.unwrap();
    assert_eq!(
        store.query_items(continuation).await.unwrap_err().code,
        ErrorCode::StaleView
    );
}

#[tokio::test]
async fn summary_keeps_detail_and_permission_failure_hides_only_affected_scope() {
    let temp = tempfile::tempdir().unwrap();
    let store = Store::open(temp.path().join("db")).await.unwrap();
    connect(&store, "a").await;
    connect(&store, "b").await;
    for id in ["a", "b"] {
        let run_id = run(&store, id).await;
        store
            .apply_page(page(id, &run_id, vec![item(id, "pr")], true))
            .await
            .unwrap();
    }
    let run_id = run(&store, "a").await;
    let mut summary = item("a", "pr");
    summary.body = None;
    summary.body_omitted = true;
    summary.head_oid = None;
    store
        .apply_page(page("a", &run_id, vec![summary], true))
        .await
        .unwrap();
    assert_eq!(
        store
            .item("a", "pr")
            .await
            .unwrap()
            .item
            .unwrap()
            .body
            .as_deref(),
        Some("cached confidential body")
    );
    let run_id = run(&store, "a").await;
    let mut cleared = item("a", "pr");
    cleared.body = None;
    cleared.body_omitted = false;
    store
        .apply_page(page("a", &run_id, vec![cleared], true))
        .await
        .unwrap();
    assert!(
        store
            .item("a", "pr")
            .await
            .unwrap()
            .item
            .unwrap()
            .body
            .is_none(),
        "An authoritative null body clears old text"
    );
    store
        .set_sync_status(
            "a",
            "1",
            "repo:repo-a:pull_request",
            SyncStatus {
                state: SyncState::Error,
                last_success_at: None,
                next_retry_at: None,
                error: Some(collaboration::error::CollaborationError::new(
                    ErrorCode::PermissionDenied,
                    "Access needs revalidation",
                )),
            },
        )
        .await
        .unwrap();
    let snapshot = store.query_items(query("a")).await.unwrap();
    assert!(snapshot.items.is_empty());
    assert_eq!(snapshot.sync.state, SyncState::Error);
    assert!(store.item("a", "pr").await.unwrap().item.is_none());
    assert_eq!(store.query_items(query("b")).await.unwrap().items.len(), 1);
    let run_id = run(&store, "a").await;
    assert!(
        store
            .query_items(query("a"))
            .await
            .unwrap()
            .items
            .is_empty(),
        "Revalidation does not expose denied cache before a successful observation"
    );
    store
        .apply_page(page("a", &run_id, vec![item("a", "pr")], true))
        .await
        .unwrap();
    assert_eq!(store.query_items(query("a")).await.unwrap().items.len(), 1);
}

#[tokio::test]
async fn notifications_use_remote_disposition_and_accept_repository_references() {
    let temp = tempfile::tempdir().unwrap();
    let store = Store::open(temp.path().join("db")).await.unwrap();
    connect(&store, "a").await;
    let mut referenced = repository("a");
    referenced.id = "referenced-only".into();
    referenced.provider_id = "another-provider-repo".into();
    let mut unread = item("a", "notification-unread");
    unread.kind = RemoteItemKind::Notification;
    unread.repository_id = Some(referenced.id.clone());
    unread.state = "PullRequest".into();
    unread.unread = Some(true);
    let mut read = unread.clone();
    read.id = "notification-read".into();
    read.provider_id = "notification-read".into();
    read.unread = Some(false);
    let run_id = store.begin_sync("a", "1", "notifications").await.unwrap();
    store
        .apply_page(PageCommit {
            account_id: "a".into(),
            authorization_epoch: "1".into(),
            scope: "notifications".into(),
            run_id,
            repositories: vec![referenced],
            items: vec![unread, read],
            next_cursor: None,
            etag: None,
            last_modified: None,
            not_modified: false,
            complete: true,
            observed_at: "2026-10-02T12:00:00Z".into(),
        })
        .await
        .unwrap();
    let mut inbox = query("a");
    inbox.kind = RemoteItemKind::Notification;
    inbox.state = Some("unread".into());
    let snapshot = store.query_items(inbox.clone()).await.unwrap();
    assert_eq!(snapshot.items.len(), 1);
    assert_eq!(snapshot.items[0].id, "notification-unread");
    inbox.state = Some("read".into());
    assert_eq!(
        store.query_items(inbox.clone()).await.unwrap().items.len(),
        1
    );
    inbox.state = Some("open".into());
    assert_eq!(
        store.query_items(inbox).await.unwrap_err().code,
        ErrorCode::InvalidInput
    );
    assert_eq!(store.repositories("a").await.unwrap().repositories.len(), 2);
}

#[tokio::test]
async fn catchup_advances_bounded_scanned_revisions_and_draft_generation_is_guarded() {
    let temp = tempfile::tempdir().unwrap();
    let store = Store::open(temp.path().join("db")).await.unwrap();
    store.upsert_account(account("a")).await.unwrap();
    let baseline = store.revision().await.unwrap();
    for generation in 0..270 {
        store
            .save_draft(LocalDraft {
                account_id: "a".into(),
                subject_id: "draft".into(),
                body: format!("draft{generation}"),
                generation: generation.to_string(),
            })
            .await
            .unwrap();
    }
    let first = store.changes_since(&baseline).await.unwrap();
    assert_eq!(first.changes.len(), 256);
    assert!(first.has_more);
    assert!(!first.reset_required);
    let last = store.changes_since(&first.revision).await.unwrap();
    assert_eq!(last.changes.len(), 14);
    assert!(!last.has_more);
    assert_eq!(last.revision, store.revision().await.unwrap());
    let baseline = store.revision().await.unwrap();
    assert_eq!(
        store
            .save_draft(LocalDraft {
                account_id: "a".into(),
                subject_id: "draft".into(),
                body: "stale edit".into(),
                generation: "1".into()
            })
            .await
            .unwrap_err()
            .code,
        ErrorCode::StaleView
    );
    assert_eq!(baseline, store.revision().await.unwrap());
    assert_eq!(
        store.draft("a", "draft").await.unwrap().unwrap().body,
        "draft269"
    );
}

#[tokio::test]
async fn one_storage_owner_is_enforced_and_process_lease_releases_on_drop() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("db");
    let owner = Store::open(&path).await.unwrap();
    assert_eq!(
        Store::open(&path).await.err().unwrap().code,
        ErrorCode::Busy
    );
    let clone = owner.clone();
    drop(owner);
    assert_eq!(
        Store::open(&path).await.err().unwrap().code,
        ErrorCode::Busy
    );
    clone.close().await;
    drop(clone);
    let reopened = Store::open(&path).await.unwrap();
    assert_eq!(reopened.revision().await.unwrap(), "0");
    assert!(
        path.with_file_name("db.lock").exists(),
        "The lock inode remains stable across application instances"
    );
}

#[tokio::test]
async fn credential_lifecycle_keeps_observed_account_rate_cooldown() {
    let temp = tempfile::tempdir().unwrap();
    let store = Store::open(temp.path().join("db")).await.unwrap();
    store.upsert_account(account("a")).await.unwrap();
    store
        .set_sync_status(
            "a",
            "1",
            "provider:rest",
            SyncStatus {
                state: SyncState::RateLimited,
                last_success_at: None,
                next_retry_at: Some("2026-10-03T12:00:00Z".into()),
                error: None,
            },
        )
        .await
        .unwrap();
    let mut replacement = account("a");
    replacement.authorization_epoch = "2".into();
    store.upsert_account(replacement).await.unwrap();
    assert_eq!(
        store
            .scope_state("a", "provider:rest")
            .await
            .unwrap()
            .unwrap()
            .sync
            .next_retry_at
            .as_deref(),
        Some("2026-10-03T12:00:00Z")
    );
    store.disconnect("a").await.unwrap();
    let mut replacement = account("a");
    replacement.authorization_epoch = "4".into();
    store.upsert_account(replacement).await.unwrap();
    assert_eq!(
        store
            .scope_state("a", "provider:rest")
            .await
            .unwrap()
            .unwrap()
            .sync
            .state,
        SyncState::RateLimited
    );
}

#[tokio::test]
async fn newer_schema_is_refused_without_erasing_user_intent() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("db");
    {
        let store = Store::open(&path).await.unwrap();
        store.upsert_account(account("a")).await.unwrap();
        store
            .save_draft(LocalDraft {
                account_id: "a".into(),
                subject_id: "draft".into(),
                body: "preserved".into(),
                generation: "0".into(),
            })
            .await
            .unwrap();
        store.close().await;
    }
    let mut connection =
        sqlx::SqliteConnection::connect_with(&SqliteConnectOptions::new().filename(&path))
            .await
            .unwrap();
    sqlx::query("INSERT INTO _sqlx_migrations(version,description,installed_on,success,checksum,execution_time) VALUES(999,'future',CURRENT_TIMESTAMP,1,x'00',0)").execute(&mut connection).await.unwrap();
    connection.close().await.unwrap();
    assert_eq!(
        Store::open(&path).await.err().unwrap().code,
        ErrorCode::Storage
    );
    let mut connection = sqlx::SqliteConnection::connect_with(
        &SqliteConnectOptions::new().filename(&path).read_only(true),
    )
    .await
    .unwrap();
    let row = sqlx::query(
        "SELECT body,generation FROM drafts WHERE account_id='a' AND subject_id='draft'",
    )
    .fetch_one(&mut connection)
    .await
    .unwrap();
    assert_eq!(row.get::<String, _>("body"), "preserved");
    assert_eq!(row.get::<i64, _>("generation"), 1);
}

#[tokio::test]
async fn discovery_membership_and_denial_hide_projections_without_erasing_canonical_rows() {
    let temp = tempfile::tempdir().unwrap();
    let store = Store::open(temp.path().join("db")).await.unwrap();
    connect(&store, "a").await;
    let run_id = run(&store, "a").await;
    store
        .apply_page(page("a", &run_id, vec![item("a", "pr")], true))
        .await
        .unwrap();
    let pending = run(&store, "a").await;
    for enumeration in 0..2 {
        let run_id = store.begin_sync("a", "1", "repositories").await.unwrap();
        store
            .apply_page(PageCommit {
                account_id: "a".into(),
                authorization_epoch: "1".into(),
                scope: "repositories".into(),
                run_id,
                repositories: vec![],
                items: vec![],
                next_cursor: None,
                etag: Some("empty".into()),
                last_modified: None,
                not_modified: false,
                complete: true,
                observed_at: "2026-10-02T12:00:00Z".into(),
            })
            .await
            .unwrap();
        assert_eq!(
            store.repositories("a").await.unwrap().repositories.len(),
            if enumeration == 0 { 1 } else { 0 }
        );
    }
    assert_eq!(
        store.repository("a", "repo-a").await.unwrap_err().code,
        ErrorCode::NotFound
    );
    assert!(
        store
            .query_items(query("a"))
            .await
            .unwrap()
            .items
            .is_empty()
    );
    assert_eq!(
        store.query_items(query("a")).await.unwrap().coverage.state,
        CoverageState::Missing
    );
    assert!(
        store.item("a", "pr").await.unwrap().item.is_some(),
        "Feed absence retains canonical detail"
    );
    assert_eq!(
        store
            .apply_page(page("a", &pending, vec![item("a", "late")], true))
            .await
            .unwrap_err()
            .code,
        ErrorCode::StaleView
    );

    let run_id = store.begin_sync("a", "1", "repositories").await.unwrap();
    store
        .apply_page(PageCommit {
            account_id: "a".into(),
            authorization_epoch: "1".into(),
            scope: "repositories".into(),
            run_id,
            repositories: vec![repository("a")],
            items: vec![],
            next_cursor: None,
            etag: None,
            last_modified: None,
            not_modified: false,
            complete: true,
            observed_at: "2026-10-02T12:00:00Z".into(),
        })
        .await
        .unwrap();
    assert!(
        store.repository("a", "repo-a").await.unwrap().selected,
        "Reappearing discovery retains local selection"
    );
    store
        .set_sync_status("a", "1", "repositories", denied())
        .await
        .unwrap();
    assert!(
        store
            .repositories("a")
            .await
            .unwrap()
            .repositories
            .is_empty()
    );
    assert!(
        store
            .query_items(query("a"))
            .await
            .unwrap()
            .items
            .is_empty()
    );
    assert!(
        store.item("a", "pr").await.unwrap().item.is_none(),
        "Denied discovery blocks discovered canonical detail"
    );
    let mut referenced = repository("a");
    referenced.id = "notification-only".into();
    referenced.provider_id = "notification-repository".into();
    let mut notification = item("a", "notification");
    notification.kind = RemoteItemKind::Notification;
    notification.repository_id = Some(referenced.id.clone());
    notification.unread = Some(true);
    let run_id = store.begin_sync("a", "1", "notifications").await.unwrap();
    store
        .apply_page(PageCommit {
            account_id: "a".into(),
            authorization_epoch: "1".into(),
            scope: "notifications".into(),
            run_id,
            repositories: vec![referenced],
            items: vec![notification],
            next_cursor: None,
            etag: None,
            last_modified: None,
            not_modified: false,
            complete: true,
            observed_at: "2026-10-02T12:00:00Z".into(),
        })
        .await
        .unwrap();
    let visible = store.repositories("a").await.unwrap().repositories;
    assert_eq!(visible.len(), 1);
    assert_eq!(
        visible[0].id, "notification-only",
        "An independently authorized reference remains visible"
    );
    store
        .set_sync_status("a", "1", "notifications", denied())
        .await
        .unwrap();
    assert!(
        store
            .repositories("a")
            .await
            .unwrap()
            .repositories
            .is_empty()
    );
}

fn denied() -> SyncStatus {
    SyncStatus {
        state: SyncState::Error,
        last_success_at: None,
        next_retry_at: None,
        error: Some(collaboration::CollaborationError::new(
            ErrorCode::PermissionDenied,
            "Access needs revalidation",
        )),
    }
}

#[tokio::test]
async fn pending_absence_disables_conditionals_until_a_second_full_enumeration() {
    let temp = tempfile::tempdir().unwrap();
    let store = Store::open(temp.path().join("db")).await.unwrap();
    connect(&store, "a").await;
    let run_id = run(&store, "a").await;
    store
        .apply_page(page("a", &run_id, vec![item("a", "pr")], true))
        .await
        .unwrap();
    assert!(
        store
            .scope_state("a", "repo:repo-a:pull_request")
            .await
            .unwrap()
            .unwrap()
            .etag
            .is_some()
    );
    let run_id = run(&store, "a").await;
    store
        .apply_page(page("a", &run_id, vec![], true))
        .await
        .unwrap();
    assert!(
        store
            .scope_state("a", "repo:repo-a:pull_request")
            .await
            .unwrap()
            .unwrap()
            .etag
            .is_none()
    );
    assert_eq!(store.query_items(query("a")).await.unwrap().items.len(), 1);
    let run_id = run(&store, "a").await;
    let mut unchanged = page("a", &run_id, vec![], true);
    unchanged.not_modified = true;
    store.apply_page(unchanged).await.unwrap();
    assert!(
        store
            .scope_state("a", "repo:repo-a:pull_request")
            .await
            .unwrap()
            .unwrap()
            .etag
            .is_none()
    );
    assert_eq!(
        store.query_items(query("a")).await.unwrap().items.len(),
        1,
        "304 is not a second absence observation"
    );
    let run_id = run(&store, "a").await;
    store
        .apply_page(page("a", &run_id, vec![], true))
        .await
        .unwrap();
    assert!(
        store
            .query_items(query("a"))
            .await
            .unwrap()
            .items
            .is_empty()
    );
    assert!(
        store
            .scope_state("a", "repo:repo-a:pull_request")
            .await
            .unwrap()
            .unwrap()
            .etag
            .is_some()
    );
    assert!(store.item("a", "pr").await.unwrap().item.is_some());
}

#[tokio::test]
async fn a_new_scope_denial_advances_authorization_view_once_and_publishes_a_reset() {
    let temp = tempfile::tempdir().unwrap();
    let store = Store::open(temp.path().join("db")).await.unwrap();
    connect(&store, "a").await;
    let run_id = run(&store, "a").await;
    store
        .apply_page(page("a", &run_id, vec![item("a", "pr")], true))
        .await
        .unwrap();
    let before = store.query_items(query("a")).await.unwrap();
    store
        .set_sync_status("a", "1", "repo:repo-a:pull_request", denied())
        .await
        .unwrap();
    let hidden = store.query_items(query("a")).await.unwrap();
    assert!(hidden.items.is_empty());
    assert_ne!(before.authorization_view, hidden.authorization_view);
    let changes = store.changes_since(&before.revision).await.unwrap();
    assert!(changes.reset_required);
    assert!(changes.changes.iter().any(|change| change.reset));
    assert_eq!(store.account("a").await.unwrap().authorization_epoch, "1");
    store
        .set_sync_status("a", "1", "repo:repo-a:pull_request", denied())
        .await
        .unwrap();
    assert_eq!(
        hidden.authorization_view,
        store
            .query_items(query("a"))
            .await
            .unwrap()
            .authorization_view
    );
    assert!(
        !store
            .changes_since(&hidden.revision)
            .await
            .unwrap()
            .reset_required
    );
    let run_id = run(&store, "a").await;
    store
        .apply_page(page("a", &run_id, vec![item("a", "pr")], true))
        .await
        .unwrap();
    assert_eq!(store.query_items(query("a")).await.unwrap().items.len(), 1);
    let before_second_denial = store.accounts().await.unwrap();
    store
        .set_sync_status("a", "1", "repo:repo-a:pull_request", denied())
        .await
        .unwrap();
    assert_ne!(
        before_second_denial.authorization_view,
        store.accounts().await.unwrap().authorization_view
    );
}

#[tokio::test]
async fn cursor_depends_on_the_selected_projection_instead_of_unrelated_runtime_revisions() {
    let temp = tempfile::tempdir().unwrap();
    let store = Store::open(temp.path().join("db")).await.unwrap();
    connect(&store, "a").await;
    connect(&store, "b").await;
    let run_id = run(&store, "a").await;
    store
        .apply_page(page(
            "a",
            &run_id,
            vec![item("a", "first"), item("a", "second")],
            true,
        ))
        .await
        .unwrap();
    let mut continuation = query("a");
    continuation.limit = 1;
    let first = store.query_items(continuation.clone()).await.unwrap();
    continuation.cursor = first.next_cursor;
    store
        .save_draft(LocalDraft {
            account_id: "a".into(),
            subject_id: "first".into(),
            body: "local draft".into(),
            generation: "0".into(),
        })
        .await
        .unwrap();
    store
        .set_sync_status(
            "a",
            "1",
            "repo:repo-a:pull_request",
            SyncStatus {
                state: SyncState::Syncing,
                ..SyncStatus::default()
            },
        )
        .await
        .unwrap();
    let run_id = run(&store, "b").await;
    store
        .apply_page(page("b", &run_id, vec![item("b", "unrelated")], true))
        .await
        .unwrap();
    assert_eq!(
        store
            .query_items(continuation.clone())
            .await
            .unwrap()
            .items
            .len(),
        1
    );
    let run_id = run(&store, "a").await;
    store
        .apply_page(page(
            "a",
            &run_id,
            vec![item("a", "first"), item("a", "second")],
            true,
        ))
        .await
        .unwrap();
    assert_eq!(
        store.query_items(continuation).await.unwrap_err().code,
        ErrorCode::StaleView
    );
    let mut continuation = query("a");
    continuation.limit = 1;
    continuation.cursor = store
        .query_items(continuation.clone())
        .await
        .unwrap()
        .next_cursor;
    store.select_repository("a", "repo-a", false).await.unwrap();
    assert_eq!(
        store.query_items(continuation).await.unwrap_err().code,
        ErrorCode::StaleView,
        "Changing the selected projection invalidates its cursor"
    );
}

#[tokio::test]
async fn recovery_enumerates_only_authored_drafts_after_disconnect_and_restart() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("recovery.db");
    let store = Store::open(&path).await.unwrap();
    connect(&store, "a").await;
    connect(&store, "b").await;
    for (account_id, body) in [("a", "My private text 🪴"), ("b", "Another actor's text")] {
        store
            .save_draft(LocalDraft {
                account_id: account_id.into(),
                subject_id: "missing-subject".into(),
                body: body.into(),
                generation: "0".into(),
            })
            .await
            .unwrap();
    }
    let run_id = run(&store, "a").await;
    store
        .apply_page(page("a", &run_id, vec![item("a", "provider-only")], true))
        .await
        .unwrap();
    store.disconnect("a").await.unwrap();
    store.close().await;
    drop(store);
    let store = Store::open(&path).await.unwrap();
    let recovered = store
        .query_drafts(DraftQuery {
            account_id: "a".into(),
            cursor: None,
            limit: 50,
        })
        .await
        .unwrap();
    assert_eq!(
        recovered.drafts,
        vec![DraftSummary {
            subject_id: "missing-subject".into(),
            preview: "My private text 🪴".into(),
            generation: "1".into()
        }]
    );
    assert_eq!(recovered.next_cursor, None);
    let mut draft = store.draft("a", "missing-subject").await.unwrap().unwrap();
    let old = draft.clone();
    draft.body = "Recovered and edited offline".into();
    assert_eq!(store.save_draft(draft).await.unwrap().generation, "2");
    assert_eq!(
        store.save_draft(old).await.unwrap_err().code,
        ErrorCode::StaleView
    );
    assert_eq!(
        store
            .draft("a", "missing-subject")
            .await
            .unwrap()
            .unwrap()
            .body,
        "Recovered and edited offline"
    );
    assert_eq!(
        store
            .draft("b", "missing-subject")
            .await
            .unwrap()
            .unwrap()
            .body,
        "Another actor's text"
    );
}

#[tokio::test]
async fn recovery_pages_are_bounded_and_cursors_cannot_cross_actor_partitions() {
    let temp = tempfile::tempdir().unwrap();
    let store = Store::open(temp.path().join("recovery.db")).await.unwrap();
    store.upsert_account(account("a")).await.unwrap();
    store.upsert_account(account("b")).await.unwrap();
    for index in 0..105 {
        store
            .save_draft(LocalDraft {
                account_id: "a".into(),
                subject_id: format!("subject-{index:03}"),
                body: "界".repeat(200),
                generation: "0".into(),
            })
            .await
            .unwrap();
    }
    let query = DraftQuery {
        account_id: "a".into(),
        cursor: None,
        limit: 50,
    };
    let first = store.query_drafts(query.clone()).await.unwrap();
    assert_eq!(first.drafts.len(), 50);
    assert_eq!(first.drafts[0].preview.chars().count(), 160);
    let second = store
        .query_drafts(DraftQuery {
            cursor: first.next_cursor.clone(),
            ..query.clone()
        })
        .await
        .unwrap();
    assert_eq!(second.drafts.len(), 50);
    assert_eq!(second.drafts[0].subject_id, "subject-050");
    let last = store
        .query_drafts(DraftQuery {
            cursor: second.next_cursor,
            ..query.clone()
        })
        .await
        .unwrap();
    assert_eq!(last.drafts.len(), 5);
    assert_eq!(last.next_cursor, None);
    let crossed = DraftQuery {
        account_id: "b".into(),
        cursor: first.next_cursor,
        limit: 50,
    };
    assert_eq!(
        store.query_drafts(crossed).await.unwrap_err().code,
        ErrorCode::InvalidInput
    );
    for limit in [0, 101, u32::MAX] {
        assert_eq!(
            store
                .query_drafts(DraftQuery {
                    limit,
                    ..query.clone()
                })
                .await
                .unwrap_err()
                .code,
            ErrorCode::InvalidInput
        );
    }
    for cursor in [
        "not-json".to_owned(),
        "x".repeat(4097),
        r#"{"version":2,"account_id":"a","subject_id":"s"}"#.into(),
    ] {
        assert_eq!(
            store
                .query_drafts(DraftQuery {
                    cursor: Some(cursor),
                    ..query.clone()
                })
                .await
                .unwrap_err()
                .code,
            ErrorCode::InvalidInput
        );
    }
}
