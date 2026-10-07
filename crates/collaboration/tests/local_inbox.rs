use collaboration::{domain::*, error::ErrorCode, storage::Store};

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
        provider_id: format!("provider-repo-{account_id}"),
        full_name: format!("owner/{account_id}"),
        name: account_id.into(),
        web_url: format!("https://github.com/owner/{account_id}"),
        description: None,
        default_branch: Some("main".into()),
        selected: false,
    }
}

fn notification(account_id: &str, id: &str, updated_at: &str, unread: bool) -> RemoteItem {
    RemoteItem {
        native_inbox: None,
        id: id.into(),
        account_id: account_id.into(),
        repository_id: Some(format!("repo-{account_id}")),
        provider_id: id.into(),
        kind: RemoteItemKind::Notification,
        number: Some("67".into()),
        title: format!("Review requested {id}"),
        body: None,
        body_omitted: false,
        author: Some("reviewer".into()),
        web_url: Some(format!("https://github.com/owner/{account_id}/pull/67")),
        state: "pending".into(),
        updated_at: updated_at.into(),
        head_oid: None,
        is_draft: None,
        reason: Some("review_requested".into()),
        unread: Some(unread),
    }
}

async fn observe(store: &Store, account_id: &str, items: Vec<RemoteItem>) {
    observe_at(store, account_id, "1", items).await;
}

async fn observe_at(store: &Store, account_id: &str, epoch: &str, items: Vec<RemoteItem>) {
    let run_id = store
        .begin_sync(account_id, epoch, "notifications")
        .await
        .unwrap();
    store
        .apply_page(PageCommit {
            account_id: account_id.into(),
            authorization_epoch: epoch.into(),
            scope: "notifications".into(),
            run_id,
            repositories: vec![repository(account_id)],
            items,
            endpoint_aliases: Vec::new(),
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

async fn observe_resource(store: &Store, account_id: &str, kind: RemoteItemKind, id: &str) {
    store
        .select_repository(account_id, &format!("repo-{account_id}"), true)
        .await
        .unwrap();
    let scope = format!(
        "repo:repo-{account_id}:{}",
        match kind {
            RemoteItemKind::PullRequest => "pull_request",
            RemoteItemKind::Issue => "issue",
            RemoteItemKind::Notification => panic!("resource helper requires PR or issue"),
        }
    );
    let run_id = store.begin_sync(account_id, "1", &scope).await.unwrap();
    let mut item = notification(account_id, id, "2026-10-07T10:00:00Z", false);
    item.kind = kind;
    item.state = "open".into();
    item.unread = None;
    store
        .apply_page(PageCommit {
            account_id: account_id.into(),
            authorization_epoch: "1".into(),
            scope,
            run_id,
            repositories: Vec::new(),
            items: vec![item],
            endpoint_aliases: Vec::new(),
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

fn denied() -> SyncStatus {
    SyncStatus {
        state: SyncState::Error,
        last_success_at: None,
        next_retry_at: None,
        error: Some(collaboration::CollaborationError::new(
            ErrorCode::PermissionDenied,
            "Inbox access needs revalidation",
        )),
    }
}

fn query(account_id: &str, local_state: LocalInboxFilter) -> InboxQuery {
    InboxQuery {
        account_id: account_id.into(),
        remote_state: None,
        local_state,
        search: None,
        cursor: None,
        limit: 100,
    }
}

fn disposition_request(
    account_id: &str,
    notification_id: &str,
    generation: &str,
    disposition: LocalInboxDisposition,
) -> SetLocalInboxStateRequest {
    SetLocalInboxStateRequest {
        account_id: account_id.into(),
        authorization_epoch: "1".into(),
        notification_id: notification_id.into(),
        expected_activity_updated_at: "2026-10-07T10:00:00Z".into(),
        mutation: LocalInboxMutation::Disposition,
        disposition: Some(disposition),
        bookmarked: None,
        snoozed_until: None,
        expected_generation: generation.into(),
    }
}

fn bookmark_request(
    account_id: &str,
    notification_id: &str,
    generation: &str,
    bookmarked: bool,
) -> SetLocalInboxStateRequest {
    SetLocalInboxStateRequest {
        account_id: account_id.into(),
        authorization_epoch: "1".into(),
        notification_id: notification_id.into(),
        expected_activity_updated_at: "2026-10-07T10:00:00Z".into(),
        mutation: LocalInboxMutation::Bookmark,
        disposition: None,
        bookmarked: Some(bookmarked),
        snoozed_until: None,
        expected_generation: generation.into(),
    }
}

#[tokio::test]
async fn local_done_and_bookmark_survive_restart_and_new_activity_resurfaces() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("collaboration.sqlite");
    let store = Store::open(&path).await.unwrap();
    store.upsert_account(account("a")).await.unwrap();
    observe(
        &store,
        "a",
        vec![notification("a", "thread-1", "2026-10-07T10:00:00Z", true)],
    )
    .await;
    let initial = store
        .inbox(query("a", LocalInboxFilter::Inbox))
        .await
        .unwrap();
    assert_eq!(initial.entries.len(), 1);
    assert_eq!(initial.entries[0].local.generation, "0");

    let receipt = store
        .set_local_inbox_state(disposition_request(
            "a",
            "thread-1",
            "0",
            LocalInboxDisposition::Done,
        ))
        .await
        .unwrap();
    assert_eq!(receipt.state.generation, "1");
    assert_eq!(
        receipt.state.effective_disposition,
        LocalInboxEffectiveDisposition::Done
    );
    assert!(!receipt.state.bookmarked);
    let receipt = store
        .set_local_inbox_state(bookmark_request("a", "thread-1", "1", true))
        .await
        .unwrap();
    assert_eq!(receipt.state.generation, "2");
    assert!(receipt.state.bookmarked);
    assert_eq!(
        receipt.state.effective_disposition,
        LocalInboxEffectiveDisposition::Done
    );
    assert!(
        store
            .inbox(query("a", LocalInboxFilter::Inbox))
            .await
            .unwrap()
            .entries
            .is_empty()
    );
    assert_eq!(
        store
            .inbox(query("a", LocalInboxFilter::Done))
            .await
            .unwrap()
            .entries
            .len(),
        1
    );
    let all = store
        .inbox(query("a", LocalInboxFilter::All))
        .await
        .unwrap();
    assert_eq!(all.entries[0].item.unread, Some(true));
    assert_eq!(all.entries[0].item.state, "pending");
    assert_eq!(
        store
            .inbox(query("a", LocalInboxFilter::Bookmarked))
            .await
            .unwrap()
            .entries
            .len(),
        1
    );
    assert_eq!(
        store
            .set_local_inbox_state(disposition_request(
                "a",
                "thread-1",
                "0",
                LocalInboxDisposition::Inbox,
            ))
            .await
            .unwrap_err()
            .code,
        ErrorCode::StaleView
    );
    store.close().await.unwrap();
    drop(store);

    let reopened = Store::open(&path).await.unwrap();
    let done = reopened
        .inbox(query("a", LocalInboxFilter::Done))
        .await
        .unwrap();
    assert_eq!(done.entries[0].local.generation, "2");
    assert!(done.entries[0].local.bookmarked);

    observe(
        &reopened,
        "a",
        vec![notification("a", "thread-1", "2026-10-07T11:00:00Z", false)],
    )
    .await;
    let resurfaced = reopened
        .inbox(query("a", LocalInboxFilter::Inbox))
        .await
        .unwrap();
    assert_eq!(resurfaced.entries.len(), 1);
    assert!(resurfaced.entries[0].local.superseded_by_activity);
    assert!(resurfaced.entries[0].local.bookmarked);
    assert_eq!(resurfaced.entries[0].item.unread, Some(false));
    assert_eq!(
        resurfaced.entries[0].local.effective_disposition,
        LocalInboxEffectiveDisposition::Inbox
    );
    let mut toggle = bookmark_request("a", "thread-1", "2", false);
    toggle.expected_activity_updated_at = "2026-10-07T11:00:00Z".into();
    let toggled = reopened.set_local_inbox_state(toggle).await.unwrap();
    assert!(!toggled.state.bookmarked);
    assert!(toggled.state.superseded_by_activity);
    assert_eq!(
        toggled.state.effective_disposition,
        LocalInboxEffectiveDisposition::Inbox,
        "bookmarking must not reapply the superseded done intent"
    );
}

#[tokio::test]
async fn unseen_provider_activity_fences_stale_local_actions() {
    let temp = tempfile::tempdir().unwrap();
    let store = Store::open(temp.path().join("collaboration.sqlite"))
        .await
        .unwrap();
    store.upsert_account(account("a")).await.unwrap();
    observe(
        &store,
        "a",
        vec![notification("a", "thread", "2026-10-07T10:00:00Z", true)],
    )
    .await;
    let inspected = store
        .inbox(query("a", LocalInboxFilter::Inbox))
        .await
        .unwrap()
        .entries
        .remove(0);

    observe(
        &store,
        "a",
        vec![notification("a", "thread", "2026-10-07T11:00:00Z", false)],
    )
    .await;
    let mut stale_done = disposition_request(
        "a",
        "thread",
        &inspected.local.generation,
        LocalInboxDisposition::Done,
    );
    stale_done.expected_activity_updated_at = inspected.item.updated_at.clone();
    assert_eq!(
        store
            .set_local_inbox_state(stale_done)
            .await
            .unwrap_err()
            .code,
        ErrorCode::StaleView
    );
    let mut stale_bookmark = bookmark_request("a", "thread", &inspected.local.generation, true);
    stale_bookmark.expected_activity_updated_at = inspected.item.updated_at;
    assert_eq!(
        store
            .set_local_inbox_state(stale_bookmark)
            .await
            .unwrap_err()
            .code,
        ErrorCode::StaleView
    );

    let current = store
        .inbox(query("a", LocalInboxFilter::Inbox))
        .await
        .unwrap();
    assert_eq!(current.entries.len(), 1);
    assert_eq!(current.entries[0].local.generation, "0");
    assert!(!current.entries[0].local.bookmarked);
    assert_eq!(
        current.entries[0].item.updated_at,
        "2026-10-07T11:00:00.000000000Z"
    );
}

#[tokio::test]
async fn filters_search_and_cursor_share_local_projection_authority() {
    let temp = tempfile::tempdir().unwrap();
    let store = Store::open(temp.path().join("collaboration.sqlite"))
        .await
        .unwrap();
    store.upsert_account(account("a")).await.unwrap();
    let mut second = notification("a", "thread-2", "2026-10-07T09:00:00Z", false);
    second.title = "A distinct search needle".into();
    observe(
        &store,
        "a",
        vec![
            notification("a", "thread-1", "2026-10-07T10:00:00Z", true),
            second,
        ],
    )
    .await;
    let mut paged = query("a", LocalInboxFilter::All);
    paged.limit = 1;
    let first = store.inbox(paged.clone()).await.unwrap();
    assert_eq!(first.entries.len(), 1);
    assert!(first.next_cursor.is_some());
    paged.cursor = first.next_cursor.clone();

    let mut mark_second_done =
        disposition_request("a", "thread-2", "0", LocalInboxDisposition::Done);
    mark_second_done.expected_activity_updated_at = "2026-10-07T09:00:00Z".into();
    store.set_local_inbox_state(mark_second_done).await.unwrap();
    assert_eq!(
        store.inbox(paged).await.unwrap_err().code,
        ErrorCode::StaleView
    );
    let mut search = query("a", LocalInboxFilter::Done);
    search.search = Some("distinct search needle".into());
    let result = store.inbox(search).await.unwrap();
    assert_eq!(result.entries.len(), 1);
    assert_eq!(result.entries[0].item.id, "thread-2");
    let mut unread = query("a", LocalInboxFilter::Inbox);
    unread.remote_state = Some("unread".into());
    assert_eq!(store.inbox(unread).await.unwrap().entries.len(), 1);
    let mut read_done = query("a", LocalInboxFilter::Done);
    read_done.remote_state = Some("read".into());
    assert_eq!(store.inbox(read_done).await.unwrap().entries.len(), 1);
    let mut pending = query("a", LocalInboxFilter::All);
    pending.remote_state = Some("pending".into());
    assert_eq!(store.inbox(pending).await.unwrap().entries.len(), 2);
    let mut remote_done = query("a", LocalInboxFilter::All);
    remote_done.remote_state = Some("done".into());
    assert!(store.inbox(remote_done).await.unwrap().entries.is_empty());
}

#[tokio::test]
async fn snooze_and_access_validation_are_local_account_scoped() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("collaboration.sqlite");
    let store = Store::open(&path).await.unwrap();
    for id in ["a", "b"] {
        store.upsert_account(account(id)).await.unwrap();
        observe(
            &store,
            id,
            vec![notification(
                id,
                "overlapping",
                "2026-10-07T10:00:00Z",
                true,
            )],
        )
        .await;
    }
    let deadline = (chrono::Utc::now() + chrono::Duration::hours(2)).to_rfc3339();
    let mut snooze = disposition_request("a", "overlapping", "0", LocalInboxDisposition::Inbox);
    snooze.snoozed_until = Some(deadline.clone());
    let saved = store.set_local_inbox_state(snooze).await.unwrap();
    let page = store
        .inbox(query("a", LocalInboxFilter::Snoozed))
        .await
        .unwrap();
    assert_eq!(page.entries.len(), 1);
    assert_eq!(page.next_local_change_at, saved.state.snoozed_until);
    assert_eq!(
        store
            .inbox(query("b", LocalInboxFilter::Inbox))
            .await
            .unwrap()
            .entries
            .len(),
        1
    );

    let mut stale_epoch = disposition_request("b", "overlapping", "0", LocalInboxDisposition::Done);
    stale_epoch.authorization_epoch = "2".into();
    assert_eq!(
        store
            .set_local_inbox_state(stale_epoch)
            .await
            .unwrap_err()
            .code,
        ErrorCode::StaleView
    );
    let mut invalid = disposition_request("b", "overlapping", "0", LocalInboxDisposition::Done);
    invalid.snoozed_until = Some(deadline);
    assert_eq!(
        store.set_local_inbox_state(invalid).await.unwrap_err().code,
        ErrorCode::InvalidInput
    );
    for invalid_deadline in [
        "not-a-timestamp".into(),
        (chrono::Utc::now() - chrono::Duration::seconds(1)).to_rfc3339(),
        (chrono::Utc::now() + chrono::Duration::days(31)).to_rfc3339(),
    ] {
        let mut invalid =
            disposition_request("b", "overlapping", "0", LocalInboxDisposition::Inbox);
        invalid.snoozed_until = Some(invalid_deadline);
        assert_eq!(
            store.set_local_inbox_state(invalid).await.unwrap_err().code,
            ErrorCode::InvalidInput
        );
    }
    let missing = disposition_request("b", "missing", "0", LocalInboxDisposition::Done);
    assert_eq!(
        store.set_local_inbox_state(missing).await.unwrap_err().code,
        ErrorCode::NotFound
    );
    store.close().await.unwrap();
    drop(store);
    let reopened = Store::open(&path).await.unwrap();
    let persisted = reopened
        .inbox(query("a", LocalInboxFilter::Snoozed))
        .await
        .unwrap();
    assert_eq!(persisted.entries.len(), 1);
    assert_eq!(
        persisted.entries[0].local.snoozed_until,
        saved.state.snoozed_until
    );
}

#[tokio::test]
async fn equal_or_older_activity_keeps_intent_but_newer_activity_supersedes_snooze() {
    let temp = tempfile::tempdir().unwrap();
    let store = Store::open(temp.path().join("collaboration.sqlite"))
        .await
        .unwrap();
    store.upsert_account(account("a")).await.unwrap();
    observe(
        &store,
        "a",
        vec![notification("a", "thread", "2026-10-07T10:00:00Z", true)],
    )
    .await;
    let receipt = store
        .set_local_inbox_state(disposition_request(
            "a",
            "thread",
            "0",
            LocalInboxDisposition::Done,
        ))
        .await
        .unwrap();
    observe(
        &store,
        "a",
        vec![notification("a", "thread", "2026-10-07T10:00:00Z", false)],
    )
    .await;
    let equal = store
        .inbox(query("a", LocalInboxFilter::Done))
        .await
        .unwrap();
    assert_eq!(equal.entries.len(), 1);
    assert!(!equal.entries[0].local.superseded_by_activity);
    assert_eq!(equal.entries[0].item.unread, Some(false));
    observe(
        &store,
        "a",
        vec![notification("a", "thread", "2026-10-07T09:59:59Z", true)],
    )
    .await;
    assert_eq!(
        store
            .inbox(query("a", LocalInboxFilter::Done))
            .await
            .unwrap()
            .entries
            .len(),
        1
    );

    let mut snooze = disposition_request(
        "a",
        "thread",
        &receipt.state.generation,
        LocalInboxDisposition::Inbox,
    );
    snooze.snoozed_until = Some((chrono::Utc::now() + chrono::Duration::hours(2)).to_rfc3339());
    let snoozed = store.set_local_inbox_state(snooze).await.unwrap();
    let bookmarked = store
        .set_local_inbox_state(bookmark_request(
            "a",
            "thread",
            &snoozed.state.generation,
            true,
        ))
        .await
        .unwrap();
    assert_eq!(
        bookmarked.state.effective_disposition,
        LocalInboxEffectiveDisposition::Snoozed
    );
    observe(
        &store,
        "a",
        vec![notification("a", "thread", "2026-10-07T11:00:00Z", false)],
    )
    .await;
    let resurfaced = store
        .inbox(query("a", LocalInboxFilter::Inbox))
        .await
        .unwrap();
    assert_eq!(resurfaced.entries.len(), 1);
    assert!(resurfaced.entries[0].local.bookmarked);
    assert!(resurfaced.entries[0].local.superseded_by_activity);
}

#[tokio::test]
async fn non_notifications_retired_and_denied_memberships_cannot_be_mutated() {
    let temp = tempfile::tempdir().unwrap();
    let store = Store::open(temp.path().join("collaboration.sqlite"))
        .await
        .unwrap();

    store.upsert_account(account("resources")).await.unwrap();
    observe(
        &store,
        "resources",
        vec![notification(
            "resources",
            "seed",
            "2026-10-07T10:00:00Z",
            true,
        )],
    )
    .await;
    for (kind, id) in [
        (RemoteItemKind::PullRequest, "pull"),
        (RemoteItemKind::Issue, "issue"),
    ] {
        observe_resource(&store, "resources", kind, id).await;
        assert_eq!(
            store
                .set_local_inbox_state(disposition_request(
                    "resources",
                    id,
                    "0",
                    LocalInboxDisposition::Done,
                ))
                .await
                .unwrap_err()
                .code,
            ErrorCode::NotFound
        );
    }

    store.upsert_account(account("retired")).await.unwrap();
    observe(
        &store,
        "retired",
        vec![notification(
            "retired",
            "thread",
            "2026-10-07T10:00:00Z",
            true,
        )],
    )
    .await;
    observe(&store, "retired", Vec::new()).await;
    observe(&store, "retired", Vec::new()).await;
    assert_eq!(
        store
            .set_local_inbox_state(disposition_request(
                "retired",
                "thread",
                "0",
                LocalInboxDisposition::Done,
            ))
            .await
            .unwrap_err()
            .code,
        ErrorCode::NotFound
    );

    store.upsert_account(account("denied")).await.unwrap();
    observe(
        &store,
        "denied",
        vec![notification(
            "denied",
            "thread",
            "2026-10-07T10:00:00Z",
            true,
        )],
    )
    .await;
    store
        .set_sync_status("denied", "1", "notifications", denied())
        .await
        .unwrap();
    assert!(
        store
            .inbox(query("denied", LocalInboxFilter::All))
            .await
            .unwrap()
            .entries
            .is_empty()
    );
    assert_eq!(
        store
            .set_local_inbox_state(disposition_request(
                "denied",
                "thread",
                "0",
                LocalInboxDisposition::Done,
            ))
            .await
            .unwrap_err()
            .code,
        ErrorCode::NotFound
    );
}

#[tokio::test]
async fn authored_state_survives_remote_cache_clear_and_requires_fresh_membership() {
    let temp = tempfile::tempdir().unwrap();
    let store = Store::open(temp.path().join("collaboration.sqlite"))
        .await
        .unwrap();
    store.upsert_account(account("a")).await.unwrap();
    observe(
        &store,
        "a",
        vec![notification("a", "thread", "2026-10-07T10:00:00Z", true)],
    )
    .await;
    let done = store
        .set_local_inbox_state(disposition_request(
            "a",
            "thread",
            "0",
            LocalInboxDisposition::Done,
        ))
        .await
        .unwrap();
    store
        .set_local_inbox_state(bookmark_request(
            "a",
            "thread",
            &done.state.generation,
            true,
        ))
        .await
        .unwrap();
    store
        .set_sync_status(
            "a",
            "1",
            "notifications",
            SyncStatus {
                state: SyncState::AuthRequired,
                last_success_at: None,
                next_retry_at: None,
                error: None,
            },
        )
        .await
        .unwrap();
    assert_eq!(
        store
            .inbox(query("a", LocalInboxFilter::All))
            .await
            .unwrap_err()
            .code,
        ErrorCode::AuthRequired
    );
    let mut reconnected = account("a");
    reconnected.authorization_epoch = "3".into();
    store.upsert_account(reconnected).await.unwrap();
    assert!(
        store
            .inbox(query("a", LocalInboxFilter::All))
            .await
            .unwrap()
            .entries
            .is_empty()
    );
    observe_at(
        &store,
        "a",
        "3",
        vec![notification("a", "thread", "2026-10-07T11:00:00Z", false)],
    )
    .await;
    let restored = store
        .inbox(query("a", LocalInboxFilter::Inbox))
        .await
        .unwrap();
    assert_eq!(restored.entries.len(), 1);
    assert!(restored.entries[0].local.bookmarked);
    assert!(restored.entries[0].local.superseded_by_activity);
    assert_eq!(restored.entries[0].local.generation, "2");
}
