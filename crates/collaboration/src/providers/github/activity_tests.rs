use super::comments_tests::{request as comments_request, response, server, token};
use super::*;
use serde_json::{Value, json};
fn request() -> DetailRequest {
    let mut r = comments_request(RemoteItemKind::PullRequest);
    r.facet = DetailFacet::Activity;
    r
}
fn row(id: u64) -> Value {
    json!({"id":id,"event":"labeled","actor":{"login":"actor"},"created_at":"2026-10-08T00:00:00Z","label":{"name":"safe <script>text</script>"}})
}
async fn page(rows: Value) -> DetailPage {
    let (provider, calls) = server(|_| vec![response(200, "", &rows)]);
    let page = provider.fetch_detail(&token(), request()).await.unwrap();
    assert!(
        calls.join().unwrap()[0]
            .starts_with("GET /repositories/123/issues/67/timeline?per_page=50 ")
    );
    page
}
#[tokio::test]
async fn activity_heterogeneous_ids_unknown_types_and_commit_time_stay_explicit() {
    let p=page(json!([
        row(9007199254740995_u64),
        {"event":"committed","sha":"a".repeat(40),"author":{"date":"2020-01-01T00:00:00Z"},"message":"commit body"},
        {"id":2,"event":"future_event","created_at":"2026-10-08T01:00:00Z","opaque_secret":"not retained"},
        {"event":"future_event","node_id":"stable-node"}
    ])).await;
    assert_eq!(p.entries.len(), 4);
    assert_eq!(p.reconciliation, DetailReconciliation::full_history());
    assert!(p.entries[0].provider_id.contains("9007199254740995"));
    let Some(crate::NativeDetailPayload::ActivityV1(commit)) = &p.entries[1].native else {
        panic!()
    };
    assert!(commit.supported);
    assert!(commit.occurred_at.is_none());
    assert!(p.entries[1].id.contains(":committed:sha:"));
    let Some(crate::NativeDetailPayload::ActivityV1(unknown)) = &p.entries[2].native else {
        panic!()
    };
    assert!(!unknown.supported);
    assert!(
        !serde_json::to_string(&p.entries)
            .unwrap()
            .contains("opaque_secret")
    );
}
#[tokio::test]
async fn activity_missing_id_or_invalid_kind_cannot_claim_complete_empty() {
    for rows in [
        json!([{ "event":"future_event" }]),
        json!([{ "event":"bad\nkind","id":1 }]),
        json!([{ "event":"committed","sha":"invalid" }]),
    ] {
        let p = page(rows).await;
        assert!(p.entries.is_empty());
        assert_eq!(p.reconciliation.enumeration, DetailEnumeration::Uncertain);
    }
    assert_eq!(
        page(json!([])).await.reconciliation,
        DetailReconciliation::full_history()
    );
}
#[tokio::test]
async fn activity_own_edit_clocks_and_bounded_text_do_not_change_event_identity() {
    let event = json!({"event":"commented","id":1,"created_at":"2026-10-07T00:00:00Z","updated_at":"2026-10-08T00:00:00Z","body":"first","user":{"login":"author"}});
    let p1 = page(json!([event])).await;
    let mut changed = event;
    changed["updated_at"] = json!("2026-10-08T01:00:00Z");
    changed["body"] = json!("x".repeat(4097));
    let p2 = page(json!([changed])).await;
    assert_eq!(p1.entries[0].id, p2.entries[0].id);
    assert_ne!(p1.entries[0].updated_at, p2.entries[0].updated_at);
    assert_eq!(p2.entries[0].body.state, DetailValueState::Oversized);
    assert!(p2.entries[0].body.text.is_none());
}
#[tokio::test]
async fn activity_cursor_is_bound_and_repeated_payload_is_refused() {
    let (provider, calls) = server(|base| {
        vec![
            response(
                200,
                &format!(
                    "Link: <{base}repositories/123/issues/67/timeline?per_page=50&page=2>; rel=\"next\"\r\n"
                ),
                &json!([row(1)]),
            ),
            response(200, "", &json!([row(1)])),
        ]
    });
    let mut r = request();
    let p = provider.fetch_detail(&token(), r.clone()).await.unwrap();
    assert_eq!(p.reconciliation.enumeration, DetailEnumeration::Uncertain);
    r.cursor = p.next_cursor;
    let mut wrong = r.clone();
    wrong.account.actor_id = "2".into();
    assert!(provider.fetch_detail(&token(), wrong).await.is_err());
    assert_eq!(
        provider.fetch_detail(&token(), r).await.unwrap_err().kind,
        ProviderErrorKind::InvalidResponse
    );
    assert_eq!(calls.join().unwrap().len(), 2);
}
#[tokio::test]
async fn activity_twenty_page_bound_retains_uncertain_terminal_without_another_cursor() {
    let (provider, calls) = server(|base| {
        (1..=20).map(|p|response(200,&format!("Link: <{base}repositories/123/issues/67/timeline?per_page=50&page={}>; rel=\"next\"\r\n",p+1),&json!([row(p)]))).collect()
    });
    let mut r = request();
    for index in 1..=20 {
        let p = provider.fetch_detail(&token(), r.clone()).await.unwrap();
        assert_eq!(p.reconciliation.enumeration, DetailEnumeration::Uncertain);
        assert_eq!(p.next_cursor.is_some(), index < 20);
        r.cursor = p.next_cursor;
    }
    assert_eq!(calls.join().unwrap().len(), 20);
}
#[tokio::test]
async fn activity_hostile_links_duplicate_ids_and_permission_errors_fail_closed() {
    for rows in [
        json!([row(1), row(1)]),
        json!((0..51).map(row).collect::<Vec<_>>()),
    ] {
        let (provider, calls) = server(|_| vec![response(200, "", &rows)]);
        assert_eq!(
            provider
                .fetch_detail(&token(), request())
                .await
                .unwrap_err()
                .kind,
            ProviderErrorKind::InvalidResponse
        );
        calls.join().unwrap();
    }
    let (provider, calls) = server(|_| {
        vec![response(
            200,
            "Link: <https://attacker.invalid/timeline?page=2>; rel=\"next\"\r\n",
            &json!([row(1)]),
        )]
    });
    assert_eq!(
        provider
            .fetch_detail(&token(), request())
            .await
            .unwrap_err()
            .kind,
        ProviderErrorKind::InvalidResponse
    );
    calls.join().unwrap();
    for (status, kind) in [
        (401, ProviderErrorKind::Authentication),
        (403, ProviderErrorKind::Permission),
    ] {
        let (provider, calls) =
            server(|_| vec![response(status, "", &json!({"message":"denied"}))]);
        assert_eq!(
            provider
                .fetch_detail(&token(), request())
                .await
                .unwrap_err()
                .kind,
            kind
        );
        calls.join().unwrap();
    }
}
#[tokio::test]
async fn activity_successful_and_rejected_pages_preserve_observed_quota() {
    for rows in [json!([row(1)]), json!([row(1), row(1)])] {
        let (provider, calls) = server(|_| {
            vec![response(
                200,
                "Retry-After: 120\r\nX-RateLimit-Remaining: 0\r\n",
                &rows,
            )]
        });
        match provider.fetch_detail(&token(), request()).await {
            Ok(p) => assert!(p.cooldown_seconds.is_some_and(|n| n >= 120)),
            Err(e) => assert!(e.account_cooldown_seconds.is_some_and(|n| n >= 120)),
        }
        calls.join().unwrap();
    }
}

async fn saved_store(dir: &std::path::Path) -> (crate::Store, DetailRequest) {
    use crate::*;
    let store = Store::open(dir.join("activity.db")).await.unwrap();
    let mut r = request();
    r.subject.updated_at = "2026-10-07T00:00:00Z".into();
    r.account = store.upsert_account(r.account).await.unwrap();
    for (scope, repositories, items) in [
        (
            "repositories".to_string(),
            vec![r.repository.clone()],
            vec![],
        ),
        (
            format!("repo:{}:pull_request", r.repository.id),
            vec![],
            vec![r.subject.clone()],
        ),
    ] {
        let run_id = store
            .begin_sync(&r.account.id, &r.account.authorization_epoch, &scope)
            .await
            .unwrap();
        store
            .apply_page(PageCommit {
                account_id: r.account.id.clone(),
                authorization_epoch: r.account.authorization_epoch.clone(),
                scope,
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
    (store, r)
}
async fn commit_page(
    store: &crate::Store,
    r: &DetailRequest,
    p: DetailPage,
) -> crate::DetailCommit {
    use crate::*;
    let lease = store
        .begin_detail(
            &r.account.id,
            &r.account.authorization_epoch,
            &r.subject.id,
            DetailFacet::Activity,
        )
        .await
        .unwrap();
    DetailCommit {
        account_id: r.account.id.clone(),
        authorization_epoch: r.account.authorization_epoch.clone(),
        authorization_view: lease.authorization_view,
        instance_id: lease.instance_id,
        subject_id: r.subject.id.clone(),
        facet: DetailFacet::Activity,
        run_id: lease.run_id,
        request_cursor: lease.next_cursor,
        reconciliation: p.reconciliation,
        metadata: None,
        subject_binding: Some(DetailSubjectBinding {
            repository_id: r.repository.id.clone(),
            repository_provider_id: r.repository.provider_id.clone(),
            provider_id: r.subject.provider_id.clone(),
            number: r.subject.number.clone(),
            kind: r.subject.kind.clone(),
            head_oid: r.subject.head_oid.clone(),
        }),
        check_context: None,
        review_context: None,
        body: p.body,
        entries: p.entries,
        source: p.source,
        next_cursor: p.next_cursor.clone(),
        etag: None,
        not_modified: false,
        whole_scope: true,
        complete: p.next_cursor.is_none(),
        freshness_seconds: 180,
    }
}
fn query(r: &DetailRequest) -> crate::DetailQuery {
    crate::DetailQuery {
        account_id: r.account.id.clone(),
        subject_id: r.subject.id.clone(),
        facet: DetailFacet::Activity,
        cursor: None,
        limit: 100,
    }
}
#[tokio::test]
async fn activity_native_paging_cold_reopen_and_independent_comments_keep_authored_text() {
    use crate::*;
    let dir = tempfile::tempdir().unwrap();
    let (store, r) = saved_store(dir.path()).await;
    store
        .save_draft(LocalDraft {
            account_id: r.account.id.clone(),
            subject_id: r.subject.id.clone(),
            body: "private authored note".into(),
            generation: "0".into(),
        })
        .await
        .unwrap();
    let p = page(json!([row(2),row(1),{"id":3,"event":"future_event"}])).await;
    let commit = commit_page(&store, &r, p).await;
    store.apply_detail(commit).await.unwrap();
    let mut q = query(&r);
    q.limit = 1;
    let first = store.detail(q.clone()).await.unwrap();
    assert_eq!(first.evidence.coverage.state, CoverageState::Complete);
    assert!(
        first.entries[0]
            .provider_id
            .ends_with("00000000000000000001")
    );
    q.cursor = first.next_cursor.clone();
    let second = store.detail(q.clone()).await.unwrap();
    assert_ne!(first.entries[0].id, second.entries[0].id);
    assert!(
        second.entries[0]
            .provider_id
            .ends_with("00000000000000000002")
    );
    q.cursor = second.next_cursor;
    let third = store.detail(q).await.unwrap();
    assert_eq!(third.entries.len(), 1);
    let Some(NativeDetailPayload::ActivityV1(unknown)) = &third.entries[0].native else {
        panic!()
    };
    assert!(unknown.occurred_at.is_none());
    assert!(third.next_cursor.is_none());
    let mut comments = query(&r);
    comments.facet = DetailFacet::Comments;
    assert_eq!(
        store.detail(comments).await.unwrap().evidence.availability,
        DetailAvailability::Missing
    );
    store.close().await.unwrap();
    let cold = Store::open(dir.path().join("activity.db")).await.unwrap();
    assert_eq!(cold.detail(query(&r)).await.unwrap().entries.len(), 3);
    let draft = cold.draft(&r.account.id, &r.subject.id).await.unwrap();
    assert_eq!(draft.unwrap().body, "private authored note");
    cold.close().await.unwrap();
}
#[tokio::test]
async fn activity_incomplete_absence_cannot_delete_but_full_refresh_edits_and_removes() {
    use crate::*;
    let dir = tempfile::tempdir().unwrap();
    let (store, r) = saved_store(dir.path()).await;
    let original = json!({"id":1,"event":"commented","created_at":"2026-10-07T00:00:00Z","updated_at":"2026-10-08T00:00:00Z","body":"old"});
    let commit = commit_page(&store, &r, page(json!([original, row(2)])).await).await;
    store.apply_detail(commit).await.unwrap();
    let commit = commit_page(
        &store,
        &r,
        page(json!([{ "event":"unknown_missing_id" }])).await,
    )
    .await;
    store.apply_detail(commit).await.unwrap();
    let partial = store.detail(query(&r)).await.unwrap();
    assert_eq!(partial.entries.len(), 2);
    assert_eq!(partial.evidence.coverage.state, CoverageState::Partial);
    let mut edited = original;
    edited["updated_at"] = json!("2026-10-08T01:00:00Z");
    edited["body"] = json!("edited");
    let commit = commit_page(&store, &r, page(json!([edited])).await).await;
    store.apply_detail(commit).await.unwrap();
    let saved = store.detail(query(&r)).await.unwrap();
    assert_eq!(saved.entries.len(), 1);
    assert_eq!(saved.entries[0].body.text.as_deref(), Some("edited"));
    let commit = commit_page(&store, &r, page(json!([])).await).await;
    store.apply_detail(commit).await.unwrap();
    let empty = store.detail(query(&r)).await.unwrap();
    assert!(empty.entries.is_empty());
    assert_eq!(empty.evidence.saved_empty, Some(true));
    store.close().await.unwrap();
}
#[tokio::test]
async fn activity_held_page_after_access_loss_and_old_pager_cannot_publish_or_read() {
    let dir = tempfile::tempdir().unwrap();
    let (store, r) = saved_store(dir.path()).await;
    let commit = commit_page(&store, &r, page(json!([row(1), row(2)])).await).await;
    store.apply_detail(commit).await.unwrap();
    let mut q = query(&r);
    q.limit = 1;
    let before = store.detail(q.clone()).await.unwrap();
    q.cursor = before.next_cursor;
    let held = commit_page(&store, &r, page(json!([row(3)])).await).await;
    store
        .set_sync_status(
            &r.account.id,
            &r.account.authorization_epoch,
            &format!("repo:{}:pull_request", r.repository.id),
            crate::SyncStatus {
                state: crate::SyncState::Error,
                last_success_at: None,
                next_retry_at: None,
                error: Some(crate::CollaborationError::new(
                    crate::ErrorCode::PermissionDenied,
                    "synthetic denial",
                )),
            },
        )
        .await
        .unwrap();
    assert!(store.apply_detail(held).await.is_err());
    assert!(store.detail(q).await.unwrap().entries.is_empty());
    let denied = store.detail(query(&r)).await.unwrap();
    assert!(denied.entries.is_empty());
    assert_eq!(
        denied.evidence.availability,
        crate::DetailAvailability::Unavailable
    );
    store.close().await.unwrap();
}

#[tokio::test]
async fn activity_missing_time_becoming_known_updates_one_identity_and_retires_old_cursor() {
    use crate::*;
    let dir = tempfile::tempdir().unwrap();
    let (store, r) = saved_store(dir.path()).await;
    let unknown = json!({"id":1,"event":"labeled","label":{"name":"old"}});
    let commit = commit_page(&store, &r, page(json!([unknown, row(2)])).await).await;
    store.apply_detail(commit).await.unwrap();
    let mut q = query(&r);
    q.limit = 1;
    let old = store.detail(q.clone()).await.unwrap();
    assert!(old.entries[0].provider_id.ends_with("00000000000000000002"));
    q.cursor = old.next_cursor;
    let changed = json!({"id":1,"event":"labeled","created_at":"2026-10-07T00:00:00Z","label":{"name":"new"}});
    let mut p = page(json!([changed])).await;
    p.reconciliation = DetailReconciliation::default();
    let commit = commit_page(&store, &r, p).await;
    store.apply_detail(commit).await.unwrap();
    assert_eq!(
        store.detail(q).await.unwrap_err().code,
        ErrorCode::StaleView
    );
    let current = store.detail(query(&r)).await.unwrap();
    assert_eq!(current.entries.len(), 2);
    assert!(
        current.entries[0]
            .provider_id
            .ends_with("00000000000000000001")
    );
    let Some(NativeDetailPayload::ActivityV1(value)) = &current.entries[0].native else {
        panic!()
    };
    assert_eq!(value.description.as_deref(), Some("new"));
    assert_eq!(current.evidence.coverage.state, CoverageState::Partial);
    store.close().await.unwrap();
    use sqlx::{Connection, Row};
    let mut db = sqlx::SqliteConnection::connect_with(
        &sqlx::sqlite::SqliteConnectOptions::new()
            .filename(dir.path().join("activity.db"))
            .read_only(true),
    )
    .await
    .unwrap();
    let plan=sqlx::query("EXPLAIN QUERY PLAN SELECT id,json FROM detail_entries WHERE account_id=? AND subject_id=? AND facet='activity' AND (coalesce(json_extract(json,'$.native.value.occurred_at'),'~'),id)>(?,?) ORDER BY coalesce(json_extract(json,'$.native.value.occurred_at'),'~'),id LIMIT ?")
        .bind(&r.account.id).bind(&r.subject.id).bind("").bind("").bind(51_i64).fetch_all(&mut db).await.unwrap();
    let plan = plan
        .iter()
        .map(|row| row.get::<String, _>(3))
        .collect::<Vec<_>>()
        .join(" ");
    assert!(plan.contains("detail_activity_order"), "{plan}");
    assert!(!plan.contains("TEMP B-TREE"), "{plan}");
    db.close().await.unwrap();
}
