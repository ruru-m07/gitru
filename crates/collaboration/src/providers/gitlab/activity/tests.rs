use super::super::tests::{response, server};
use super::*;
use serde_json::json;
fn request(kind: RemoteItemKind) -> DetailRequest {
    let prefix = if kind == RemoteItemKind::PullRequest {
        "pull"
    } else {
        "issue"
    };
    DetailRequest {
        account: RemoteAccount {
            id: "a".into(),
            provider: ProviderKind::Gitlab,
            host: "gitlab.com".into(),
            actor_id: "1".into(),
            login: "actor".into(),
            display_name: None,
            authorization_epoch: "1".into(),
            state: AccountState::Active,
            notifications_supported: false,
        },
        repository: RemoteRepository {
            id: "gitlab:repository:123".into(),
            account_id: "a".into(),
            provider_id: "123".into(),
            full_name: "owner/project".into(),
            name: "project".into(),
            web_url: "https://gitlab.com/owner/project".into(),
            description: None,
            default_branch: None,
            selected: true,
        },
        subject: RemoteItem {
            id: format!("gitlab:{prefix}:999"),
            account_id: "a".into(),
            repository_id: Some("gitlab:repository:123".into()),
            provider_id: "999".into(),
            kind,
            number: Some("67".into()),
            title: "cached subject".into(),
            body: None,
            body_omitted: true,
            author: None,
            web_url: None,
            state: "open".into(),
            updated_at: "not-a-comment-clock".into(),
            head_oid: Some("a".repeat(40)),
            is_draft: None,
            reason: None,
            unread: None,
            native_inbox: None,
        },
        facet: DetailFacet::Activity,
        cursor: None,
        etag: None,
        source: None,
    }
}

fn token() -> SecretToken {
    SecretToken::new("synthetic_activity_token".into()).unwrap()
}
fn row(source: ActivitySource, id: u64, kind: RemoteItemKind) -> Value {
    let name = if kind == RemoteItemKind::Issue {
        "Issue"
    } else {
        "MergeRequest"
    };
    let author = json!({"id":8,"username":"actor","email":"never persist"});
    match source {
        ActivitySource::Notes => {
            json!({"id":id,"project_id":123,"noteable_id":999,"noteable_iid":67,"noteable_type":name,"system":true,"body":"system Δ <script>raw text</script>","created_at":"2026-10-01T00:00:00Z","updated_at":"2026-10-02T00:00:00Z","author":author})
        }
        ActivitySource::State => {
            json!({"id":id,"resource_id":999,"resource_type":name,"state":"closed","created_at":"2026-10-02T00:00:00Z","user":author})
        }
        ActivitySource::Labels => {
            json!({"id":id,"resource_id":999,"resource_type":name,"action":"add","label":{"id":9,"name":"bug <img>"},"created_at":"2026-10-03T00:00:00Z","user":author})
        }
    }
}
fn path(source: ActivitySource, page: u64, kind: RemoteItemKind) -> String {
    let route = if kind == RemoteItemKind::Issue {
        "issues"
    } else {
        "merge_requests"
    };
    let suffix = match source {
        ActivitySource::Notes => "notes",
        ActivitySource::State => "resource_state_events",
        ActivitySource::Labels => "resource_label_events",
    };
    let mut s = format!("projects/123/{route}/67/{suffix}?per_page=50&page={page}");
    if source == ActivitySource::Notes {
        s.push_str("&sort=desc&order_by=updated_at&activity_filter=only_activity");
    }
    s
}
#[tokio::test]
async fn independent_families_bind_issue_and_mr_identity_and_never_claim_full_history() {
    for kind in [RemoteItemKind::Issue, RemoteItemKind::PullRequest] {
        let (provider, task) = server(|_| {
            ActivitySource::ALL
                .into_iter()
                .map(|source| {
                    response(
                        200,
                        "",
                        &json!([row(source, u64::MAX, kind.clone())]).to_string(),
                    )
                })
                .collect()
        });
        let mut r = request(kind.clone());
        r.etag = Some("unrelated-parent".into());
        let mut entries = Vec::new();
        for _ in 0..3 {
            let page = provider.fetch_detail(&token(), r.clone()).await.unwrap();
            assert_eq!(page.reconciliation, DetailReconciliation::default());
            assert!(page.etag.is_none());
            assert!(!page.not_modified);
            assert!(page.source.provider_updated_at.is_none());
            r.cursor = page.next_cursor;
            entries.extend(page.entries);
        }
        assert!(r.cursor.is_none());
        assert_eq!(entries.len(), 3);
        assert_eq!(
            entries.iter().map(|v| &v.id).collect::<HashSet<_>>().len(),
            3
        );
        assert_eq!(
            entries
                .iter()
                .map(|v| &v.provider_id)
                .collect::<HashSet<_>>()
                .len(),
            3
        );
        assert!(entries[0].body.text.as_ref().unwrap().contains("<script>"));
        assert!(
            !serde_json::to_string(&entries)
                .unwrap()
                .contains("never persist")
        );
        let calls = task.join().unwrap();
        for (index, source) in ActivitySource::ALL.into_iter().enumerate() {
            assert!(
                calls[index]
                    .starts_with(&format!("GET /api/v4/{} ", path(source, 1, kind.clone())))
            );
            assert!(!calls[index].to_lowercase().contains("if-none-match"));
        }
    }
}
#[tokio::test]
async fn terminal_empty_sources_do_not_restart_and_nonterminal_notes_cannot_starve_peers() {
    let (provider, task) = server(|base| {
        vec![
            response(
                200,
                &format!(
                    "Link: <{base}{}>; rel=\"next\"\r\n",
                    path(ActivitySource::Notes, 2, RemoteItemKind::Issue)
                ),
                "[]",
            ),
            response(200, "", "[]"),
            response(200, "", "[]"),
            response(200, "", "[]"),
        ]
    });
    let mut r = request(RemoteItemKind::Issue);
    for n in 0..4 {
        let page = provider.fetch_detail(&token(), r.clone()).await.unwrap();
        assert!(page.entries.is_empty());
        assert_eq!(page.reconciliation, DetailReconciliation::default());
        assert_eq!(page.next_cursor.is_none(), n == 3);
        r.cursor = page.next_cursor;
        if n == 2 {
            let cursor: Cursor = serde_json::from_str(r.cursor.as_ref().unwrap()).unwrap();
            assert!(cursor.feeds[1].next.is_none() && cursor.feeds[2].next.is_none());
            assert_eq!(cursor.turn, 0);
        }
    }
    let calls = task.join().unwrap();
    assert_eq!(calls.len(), 4);
    assert!(calls[1].contains("resource_state_events"));
    assert!(calls[2].contains("resource_label_events"));
    assert!(calls[3].contains("page=2"));
}
#[tokio::test]
async fn page_budget_survives_serialized_round_robin_and_stops_before_twenty_first_http() {
    let (provider, task) = server(|base| {
        (0..20)
            .map(|n| {
                let source = ActivitySource::ALL[n % 3];
                let page = (n / 3 + 1) as u64;
                response(
                    200,
                    &format!(
                        "Link: <{base}{}>; rel=\"next\"\r\n",
                        path(source, page + 1, RemoteItemKind::Issue)
                    ),
                    &json!([row(source, page, RemoteItemKind::Issue)]).to_string(),
                )
            })
            .collect()
    });
    let mut r = request(RemoteItemKind::Issue);
    for n in 0..20 {
        let page = provider.fetch_detail(&token(), r.clone()).await.unwrap();
        if n == 19 {
            assert!(
                page.next_cursor.is_none(),
                "cap is terminal, not a poisoned resume cursor"
            );
            assert_eq!(
                page.reconciliation.enumeration,
                crate::DetailEnumeration::Truncated
            );
        } else {
            let cursor: Cursor = serde_json::from_str(page.next_cursor.as_ref().unwrap()).unwrap();
            assert_eq!(cursor.pages, n + 1);
            assert_eq!(cursor.turn, (n as usize + 1) % 3);
            assert!(page.next_cursor.as_ref().unwrap().len() <= MAX_CURSOR);
        }
        r.cursor = page.next_cursor;
    }
    assert_eq!(task.join().unwrap().len(), 20);
}
#[tokio::test]
async fn ordinary_notes_unknown_events_and_missing_text_remain_distinct() {
    let mut regular = row(ActivitySource::Notes, 1, RemoteItemKind::Issue);
    regular["system"] = false.into();
    let mut missing = row(ActivitySource::Notes, 2, RemoteItemKind::Issue);
    missing.as_object_mut().unwrap().remove("body");
    let mut oversized = row(ActivitySource::Notes, 3, RemoteItemKind::Issue);
    oversized["body"] = "x".repeat(4097).into();
    let mut unknown = row(ActivitySource::State, 1, RemoteItemKind::Issue);
    unknown["state"] = "future-state".into();
    let (provider, task) = server(|_| {
        vec![
            response(200, "", &json!([regular, missing, oversized]).to_string()),
            response(200, "", &json!([unknown]).to_string()),
        ]
    });
    let mut r = request(RemoteItemKind::Issue);
    let first = provider.fetch_detail(&token(), r.clone()).await.unwrap();
    assert_eq!(first.entries.len(), 2);
    assert_eq!(first.entries[0].body.state, DetailValueState::Omitted);
    assert_eq!(first.entries[1].body.state, DetailValueState::Oversized);
    r.cursor = first.next_cursor;
    let second = provider.fetch_detail(&token(), r).await.unwrap();
    let Some(crate::NativeDetailPayload::ActivityV1(event)) = &second.entries[0].native else {
        panic!("activity")
    };
    assert!(!event.supported);
    assert_eq!(event.kind, "unknown_state");
    task.join().unwrap();
}
#[tokio::test]
async fn foreign_rows_malformed_data_duplicate_ids_and_conditional_results_keep_quota() {
    for case in 0..7 {
        let mut value = row(ActivitySource::Notes, 1, RemoteItemKind::Issue);
        let (code, body) = match case {
            0 => {
                value["project_id"] = 124.into();
                (200, json!([value]).to_string())
            }
            1 => {
                value["noteable_id"] = 998.into();
                (200, json!([value]).to_string())
            }
            2 => {
                value["noteable_type"] = "MergeRequest".into();
                (200, json!([value]).to_string())
            }
            3 => (200, json!([value.clone(), value]).to_string()),
            4 => (200, "{".into()),
            5 => (304, "[]".into()),
            _ => (200, json!(vec![value; 51]).to_string()),
        };
        let (provider, task) = server(|_| vec![response(code, "Retry-After: 120\r\n", &body)]);
        let error = provider
            .fetch_detail(&token(), request(RemoteItemKind::Issue))
            .await
            .unwrap_err();
        assert_eq!(error.kind, ProviderErrorKind::InvalidResponse, "{case}");
        assert_eq!(error.account_cooldown_seconds, Some(120));
        assert_eq!(task.join().unwrap().len(), 1);
    }
}
#[tokio::test]
async fn activity_filter_is_pinned_against_comment_continuations_and_redirects() {
    for kind in [RemoteItemKind::Issue, RemoteItemKind::PullRequest] {
        for redirect in [false, true] {
            let (provider, task) = server(|base| {
                let next = path(
                    ActivitySource::Notes,
                    if redirect { 1 } else { 2 },
                    kind.clone(),
                )
                .replace("&activity_filter=only_activity", "");
                vec![response(
                    if redirect { 302 } else { 200 },
                    &if redirect {
                        format!("Location: {base}{next}\r\n")
                    } else {
                        format!("Link: <{base}{next}>; rel=\"next\"\r\n")
                    },
                    "[]",
                )]
            });
            assert_eq!(
                provider
                    .fetch_detail(&token(), request(kind.clone()))
                    .await
                    .unwrap_err()
                    .kind,
                ProviderErrorKind::InvalidResponse
            );
            assert_eq!(task.join().unwrap().len(), 1);
        }
    }
}
#[tokio::test]
async fn mutated_identity_terminal_cursor_and_foreign_unused_family_are_rejected_before_http() {
    let (provider, task) = server(|_| vec![response(200, "", "[]")]);
    let page = provider
        .fetch_detail(&token(), request(RemoteItemKind::Issue))
        .await
        .unwrap();
    task.join().unwrap();
    let raw: Value = serde_json::from_str(page.next_cursor.as_ref().unwrap()).unwrap();
    for case in 0..8 {
        let mut v = raw.clone();
        match case {
            0=>v["epoch"]="2".into(),1=>v["actor"]="2".into(),2=>v["native"]=998.into(),
            3=>v["turn"]=0.into(),4=>v["pages"]=2.into(),5=>v["unexpected"]=true.into(),
            6=>v["feeds"][2]["next"]="https://evil.invalid/api/v4/projects/123/issues/67/resource_label_events?per_page=50&page=1".into(),
            _=>v["feeds"][1]["next"]=Value::Null,
        }
        let mut r = request(RemoteItemKind::Issue);
        r.cursor = Some(v.to_string());
        assert_eq!(
            provider.fetch_detail(&token(), r).await.unwrap_err().kind,
            ProviderErrorKind::InvalidResponse,
            "{case}"
        );
    }
}

use crate::{
    CollaborationRuntime, DetailCommit, DetailLease, DetailQuery, DetailSubjectBinding, ErrorCode,
    HydrateDetailRequest, LocalDraft, PageCommit, Store, SyncState, SyncStatus,
    credentials::{CredentialError, CredentialVault},
};
use std::{
    collections::HashMap,
    io::{Read, Write},
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

#[derive(Default)]
struct ActivityVault {
    tokens: Mutex<HashMap<String, SecretToken>>,
    loads: AtomicUsize,
}
impl CredentialVault for ActivityVault {
    fn load(&self, reference: &str) -> Result<Option<SecretToken>, CredentialError> {
        self.loads.fetch_add(1, Ordering::SeqCst);
        Ok(self.tokens.lock().unwrap().get(reference).cloned())
    }
    fn store(&self, reference: &str, token: &SecretToken) -> Result<(), CredentialError> {
        self.tokens
            .lock()
            .unwrap()
            .insert(reference.into(), token.clone());
        Ok(())
    }
    fn delete(&self, reference: &str) -> Result<(), CredentialError> {
        self.tokens.lock().unwrap().remove(reference);
        Ok(())
    }
}
async fn saved_activity(
    directory: &std::path::Path,
) -> (Arc<Store>, DetailRequest, Arc<ActivityVault>, LocalDraft) {
    let store = Arc::new(
        Store::open(directory.join("activity.sqlite"))
            .await
            .unwrap(),
    );
    let vault = Arc::new(ActivityVault::default());
    let mut r = request(RemoteItemKind::Issue);
    r.subject.updated_at = "2026-10-07T00:00:00Z".into();
    r.account = store.upsert_account(r.account).await.unwrap();
    store
        .stage_credential(&r.account.id, "activity-test-reference")
        .await
        .unwrap();
    vault.store("activity-test-reference", &token()).unwrap();
    r.account.authorization_epoch = "2".into();
    r.account = store
        .commit_account_credential(r.account, "activity-test-reference")
        .await
        .unwrap();
    for (scope, repositories, items) in [
        ("repositories".into(), vec![r.repository.clone()], vec![]),
        ("notifications".into(), vec![], vec![]),
        (
            format!("repo:{}:issue", r.repository.id),
            vec![],
            vec![r.subject.clone()],
        ),
        (
            format!("repo:{}:pull_request", r.repository.id),
            vec![],
            vec![],
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
                // These already-read summary scopes are not the subject of this
                // fixture. The real background scheduler must only dispatch Activity.
                observed_at: "2099-01-01T00:00:00Z".into(),
            })
            .await
            .unwrap();
    }
    let draft = store
        .save_draft(LocalDraft {
            account_id: r.account.id.clone(),
            subject_id: r.subject.id.clone(),
            body: "Private note Δ must survive history".into(),
            generation: "0".into(),
        })
        .await
        .unwrap();
    (store, r, vault, draft)
}
fn activity_query(r: &DetailRequest) -> DetailQuery {
    DetailQuery {
        account_id: r.account.id.clone(),
        subject_id: r.subject.id.clone(),
        facet: DetailFacet::Activity,
        cursor: None,
        limit: 100,
    }
}
fn commit(r: &DetailRequest, lease: DetailLease, p: DetailPage) -> DetailCommit {
    DetailCommit {
        account_id: r.account.id.clone(),
        authorization_epoch: r.account.authorization_epoch.clone(),
        authorization_view: lease.authorization_view,
        instance_id: lease.instance_id,
        subject_id: r.subject.id.clone(),
        facet: DetailFacet::Activity,
        run_id: lease.run_id,
        whole_scope: lease.next_cursor.is_none() && p.next_cursor.is_none(),
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
        complete: p.next_cursor.is_none(),
        next_cursor: p.next_cursor,
        etag: None,
        not_modified: false,
        freshness_seconds: p.freshness_seconds,
    }
}
async fn next_commit(store: &Store, r: &DetailRequest, provider: &GitlabProvider) -> DetailCommit {
    let lease = store
        .begin_detail(
            &r.account.id,
            &r.account.authorization_epoch,
            &r.subject.id,
            DetailFacet::Activity,
        )
        .await
        .unwrap();
    let mut request = r.clone();
    request.cursor = lease.next_cursor.clone();
    let page = provider.fetch_detail(&token(), request).await.unwrap();
    commit(r, lease, page)
}
fn event(entry: &DetailEntry) -> &crate::ActivityEvent {
    match entry.native.as_ref().unwrap() {
        crate::NativeDetailPayload::ActivityV1(event) => event,
        _ => panic!("activity payload"),
    }
}

#[tokio::test]
async fn activity_sqlite_cold_chronological_paging_and_empty_refresh_never_prune_or_touch_draft() {
    let directory = tempfile::tempdir().unwrap();
    let (store, r, _, draft) = saved_activity(directory.path()).await;
    let mut note = row(ActivitySource::Notes, 1, RemoteItemKind::Issue);
    note["created_at"] = "2026-10-03T00:00:00Z".into();
    note["updated_at"] = "2026-10-08T00:00:00Z".into();
    let mut label = row(ActivitySource::Labels, 1, RemoteItemKind::Issue);
    label["created_at"] = "2026-10-01T00:00:00Z".into();
    let (provider, task) = server(|_| {
        vec![
            response(200, "", &json!([note]).to_string()),
            response(
                200,
                "",
                &json!([row(ActivitySource::State, 1, RemoteItemKind::Issue)]).to_string(),
            ),
            response(200, "", &json!([label]).to_string()),
        ]
    });
    for _ in 0..3 {
        store
            .apply_detail(next_commit(&store, &r, &provider).await)
            .await
            .unwrap();
    }
    assert_eq!(task.join().unwrap().len(), 3);
    let before = store.detail(activity_query(&r)).await.unwrap();
    assert_eq!(
        before.evidence.coverage.state,
        crate::CoverageState::Partial
    );
    assert_eq!(before.entries.len(), 3);
    assert_eq!(
        before
            .entries
            .iter()
            .map(|e| event(e).occurred_at.as_deref().unwrap())
            .collect::<Vec<_>>(),
        [
            "2026-10-01T00:00:00.000000000Z",
            "2026-10-02T00:00:00.000000000Z",
            "2026-10-03T00:00:00.000000000Z"
        ]
    );
    assert!(
        before.entries.iter().all(|e| e.state.is_none()),
        "history is not current workflow state"
    );
    store.close().await.unwrap();
    let store = Store::open(directory.path().join("activity.sqlite"))
        .await
        .unwrap();
    let mut query = activity_query(&r);
    query.limit = 1;
    let mut entries = vec![];
    loop {
        let page = store.detail(query.clone()).await.unwrap();
        entries.extend(page.entries);
        if page.next_cursor.is_none() {
            break;
        }
        query.cursor = page.next_cursor;
    }
    assert_eq!(entries, before.entries);
    assert_eq!(
        store.draft(&r.account.id, &r.subject.id).await.unwrap(),
        Some(draft.clone())
    );
    let mut partial = row(ActivitySource::Notes, 2, RemoteItemKind::Issue);
    partial.as_object_mut().unwrap().remove("created_at");
    let (provider, task) = server(|_| {
        vec![
            response(200, "", &json!([partial]).to_string()),
            response(200, "", "[]"),
            response(200, "", "[]"),
        ]
    });
    for _ in 0..3 {
        store
            .apply_detail(next_commit(&store, &r, &provider).await)
            .await
            .unwrap();
    }
    let after = store.detail(activity_query(&r)).await.unwrap();
    assert_eq!(after.entries, before.entries);
    assert_eq!(after.evidence.coverage.state, crate::CoverageState::Partial);
    assert_eq!(
        after.evidence.saved_empty, None,
        "uncertain history does not assert saved-empty authority"
    );
    assert_eq!(
        store.draft(&r.account.id, &r.subject.id).await.unwrap(),
        Some(draft)
    );
    assert_eq!(
        store
            .item(&r.account.id, &r.subject.id)
            .await
            .unwrap()
            .item
            .unwrap()
            .state,
        "open"
    );
    assert_eq!(task.join().unwrap().len(), 3);
    store.close().await.unwrap();
}

#[tokio::test]
async fn activity_held_same_epoch_denial_and_old_epoch_cannot_publish_or_erase_authored_text() {
    for deny in [true, false] {
        let directory = tempfile::tempdir().unwrap();
        let (store, r, _, draft) = saved_activity(directory.path()).await;
        let (provider, task) = server(|_| {
            vec![response(
                200,
                "Retry-After: 120\r\n",
                &json!([row(ActivitySource::Notes, 1, RemoteItemKind::Issue)]).to_string(),
            )]
        });
        let held = next_commit(&store, &r, &provider).await;
        if deny {
            store
                .set_sync_status(
                    &r.account.id,
                    &r.account.authorization_epoch,
                    &DetailFacet::Activity.scope(&r.subject.id),
                    SyncStatus {
                        state: SyncState::Error,
                        last_success_at: None,
                        next_retry_at: None,
                        error: Some(crate::CollaborationError::new(
                            ErrorCode::PermissionDenied,
                            "synthetic facet denial",
                        )),
                    },
                )
                .await
                .unwrap();
            assert_eq!(
                store
                    .account(&r.account.id)
                    .await
                    .unwrap()
                    .authorization_epoch,
                r.account.authorization_epoch
            );
        } else {
            store.disconnect(&r.account.id).await.unwrap();
        }
        assert_eq!(
            store.apply_detail(held).await.unwrap_err().code,
            ErrorCode::StaleView
        );
        let local = store.detail(activity_query(&r)).await.unwrap();
        assert!(local.entries.is_empty());
        assert_eq!(
            local.evidence.availability,
            crate::DetailAvailability::Unavailable
        );
        assert_eq!(
            store.draft(&r.account.id, &r.subject.id).await.unwrap(),
            Some(draft)
        );
        assert_eq!(task.join().unwrap().len(), 1);
        store.close().await.unwrap();
    }
}

/// Bounded script uses the real transport. Its longer acceptance watchdog is for
/// SQLite cold startup and ten-page scheduling, never an HTTP throughput claim.
fn runtime_server(
    responses: impl FnOnce(&str) -> Vec<(String, String)>,
) -> (
    Arc<GitlabProvider>,
    Arc<Mutex<Vec<String>>>,
    std::thread::JoinHandle<()>,
) {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let base = format!("http://{}/api/v4/", listener.local_addr().unwrap());
    let responses = responses(&base);
    assert!(responses.len() <= 23);
    let calls = Arc::new(Mutex::new(vec![]));
    let captured = calls.clone();
    let task = std::thread::spawn(move || {
        for (expected, reply) in responses {
            let deadline = std::time::Instant::now() + Duration::from_secs(60);
            let mut stream = loop {
                match listener.accept() {
                    Ok((stream, _)) => break stream,
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        assert!(
                            std::time::Instant::now() < deadline,
                            "expected Activity request {expected}"
                        );
                        std::thread::sleep(Duration::from_millis(2));
                    }
                    Err(error) => panic!("activity fixture accept: {error}"),
                }
            };
            stream.set_nonblocking(false).unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            stream
                .set_write_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut bytes = vec![];
            loop {
                let mut buffer = [0; 1024];
                let n = stream.read(&mut buffer).unwrap();
                bytes.extend_from_slice(&buffer[..n]);
                assert!(bytes.len() <= 16384);
                if n == 0 || bytes.windows(4).any(|b| b == b"\r\n\r\n") {
                    break;
                }
            }
            let request = String::from_utf8(bytes).unwrap();
            assert!(
                request.starts_with(&format!("GET /api/v4/{expected} ")),
                "unexpected fixture route: {}",
                request.lines().next().unwrap()
            );
            captured.lock().unwrap().push(expected);
            stream.write_all(reply.as_bytes()).unwrap();
        }
    });
    (
        Arc::new(GitlabProvider::fixture(reqwest::Url::parse(&base).unwrap())),
        calls,
        task,
    )
}
async fn checkpoint(
    runtime: &CollaborationRuntime,
    r: &DetailRequest,
    pages: Option<u64>,
    state: SyncState,
    previous_run: Option<&str>,
) -> crate::StoredScope {
    let mut changes = runtime.subscribe();
    let mut last = String::new();
    tokio::time::timeout(Duration::from_secs(45), async {
        loop {
            if let Some(scope) = runtime
                .store()
                .scope_state(&r.account.id, &DetailFacet::Activity.scope(&r.subject.id))
                .await
                .unwrap()
            {
                let accepted = scope
                    .next_cursor
                    .as_ref()
                    .and_then(|raw| serde_json::from_str::<Cursor>(raw).ok())
                    .map(|cursor| cursor.pages);
                last = format!(
                    "state={:?},pages={accepted:?},coverage={:?}",
                    scope.sync.state, scope.coverage.state
                );
                if accepted == pages
                    && scope.sync.state == state
                    && previous_run.is_none_or(|previous| scope.run_id != previous)
                {
                    break scope;
                }
            }
            match changes.recv().await {
                Ok(_) | Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => (),
                Err(error) => panic!("activity revision stream: {error}"),
            }
        }
    })
    .await
    .unwrap_or_else(|_| panic!("activity checkpoint timed out: {last}"))
}
fn hydration(r: &DetailRequest) -> HydrateDetailRequest {
    HydrateDetailRequest {
        account_id: r.account.id.clone(),
        authorization_epoch: r.account.authorization_epoch.clone(),
        subject_id: r.subject.id.clone(),
        facet: DetailFacet::Activity,
    }
}

#[tokio::test]
async fn activity_runtime_ten_page_yield_cold_resume_and_global_cap_keep_composite_state() {
    let directory = tempfile::tempdir().unwrap();
    let (store, r, vault, draft) = saved_activity(directory.path()).await;
    let (provider, calls, task) = runtime_server(|base| {
        (0..20)
            .map(|n| {
                let source = ActivitySource::ALL[n % 3];
                let page = (n / 3 + 1) as u64;
                (
                    path(source, page, RemoteItemKind::Issue),
                    response(
                        200,
                        &format!(
                            "Link: <{base}{}>; rel=\"next\"\r\n",
                            path(source, page + 1, RemoteItemKind::Issue)
                        ),
                        &json!([row(source, page, RemoteItemKind::Issue)]).to_string(),
                    ),
                )
            })
            .chain(ActivitySource::ALL.into_iter().map(|source| {
                (
                    path(source, 1, RemoteItemKind::Issue),
                    response(200, "", "[]"),
                )
            }))
            .collect()
    });
    let runtime = Arc::new(CollaborationRuntime::new(
        store,
        vault.clone(),
        provider.clone(),
    ));
    runtime.hydrate_detail(hydration(&r)).await.unwrap();
    runtime.clone().start_background();
    let first = checkpoint(&runtime, &r, Some(10), SyncState::Idle, None).await;
    assert_eq!(calls.lock().unwrap().len(), 10);
    assert_eq!(first.coverage.state, crate::CoverageState::Partial);
    let cursor: Cursor = serde_json::from_str(first.next_cursor.as_ref().unwrap()).unwrap();
    assert_eq!(
        cursor
            .feeds
            .iter()
            .map(|feed| feed.accepted)
            .collect::<Vec<_>>(),
        [4, 3, 3]
    );
    assert_eq!(cursor.turn, 1);
    runtime.shutdown().await.unwrap();
    drop(runtime);
    let store = Arc::new(
        Store::open(directory.path().join("activity.sqlite"))
            .await
            .unwrap(),
    );
    let persisted = store
        .scope_state(&r.account.id, &DetailFacet::Activity.scope(&r.subject.id))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(persisted.next_cursor, first.next_cursor);
    let loads = vault.loads.load(Ordering::SeqCst);
    let local = store.detail(activity_query(&r)).await.unwrap();
    assert_eq!(local.entries.len(), 10);
    assert_eq!(vault.loads.load(Ordering::SeqCst), loads);
    let runtime = Arc::new(CollaborationRuntime::new(
        store,
        vault.clone(),
        provider.clone(),
    ));
    runtime.hydrate_detail(hydration(&r)).await.unwrap();
    runtime.clone().start_background();
    let second = checkpoint(&runtime, &r, None, SyncState::Idle, None).await;
    assert_eq!(calls.lock().unwrap().len(), 20);
    assert_ne!(first.run_id, second.run_id);
    assert_eq!(second.coverage.state, crate::CoverageState::Partial);
    assert!(second.coverage.remote_has_more);
    assert!(second.next_cursor.is_none());
    let revision = runtime.store().revision().await.unwrap();
    let loads = vault.loads.load(Ordering::SeqCst);
    tokio::time::sleep(Duration::from_millis(150)).await;
    assert_eq!(runtime.store().revision().await.unwrap(), revision);
    assert_eq!(vault.loads.load(Ordering::SeqCst), loads);
    assert_eq!(
        calls.lock().unwrap().len(),
        20,
        "terminal cap does not queue a 21st read"
    );
    assert_eq!(
        runtime
            .store()
            .detail(activity_query(&r))
            .await
            .unwrap()
            .entries
            .len(),
        20
    );
    runtime.shutdown().await.unwrap();
    drop(runtime);
    let store = Arc::new(
        Store::open(directory.path().join("activity.sqlite"))
            .await
            .unwrap(),
    );
    let runtime = Arc::new(CollaborationRuntime::new(store, vault.clone(), provider));
    runtime.hydrate_detail(hydration(&r)).await.unwrap();
    runtime.clone().start_background();
    let refreshed = checkpoint(&runtime, &r, None, SyncState::Idle, Some(&second.run_id)).await;
    assert_ne!(refreshed.run_id, second.run_id);
    assert_eq!(refreshed.coverage.state, crate::CoverageState::Partial);
    assert_eq!(calls.lock().unwrap().len(), 23);
    assert_eq!(
        runtime
            .store()
            .detail(activity_query(&r))
            .await
            .unwrap()
            .entries
            .len(),
        20,
        "empty bounded refresh must not prune previously saved history"
    );
    task.join().unwrap();
    assert_eq!(
        runtime
            .store()
            .draft(&r.account.id, &r.subject.id)
            .await
            .unwrap(),
        Some(draft)
    );
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn activity_runtime_success_malformed_and_http_error_cooldown_survive_cold_reopen() {
    for case in 0..3 {
        let failed = case != 0;
        let directory = tempfile::tempdir().unwrap();
        let (store, r, vault, draft) = saved_activity(directory.path()).await;
        let (provider, calls, task) = runtime_server(|_| {
            vec![(
                path(ActivitySource::Notes, 1, RemoteItemKind::Issue),
                response(
                    if case == 2 { 503 } else { 200 },
                    "Retry-After: 120\r\nRateLimit-Remaining: 20\r\n",
                    &match case {
                        1 => "{".into(),
                        2 => r#"{"message":"temporarily unavailable"}"#.into(),
                        _ => json!([row(ActivitySource::Notes, 1, RemoteItemKind::Issue)])
                            .to_string(),
                    },
                ),
            )]
        });
        let runtime = Arc::new(CollaborationRuntime::new(
            store,
            vault.clone(),
            provider.clone(),
        ));
        runtime.hydrate_detail(hydration(&r)).await.unwrap();
        runtime.clone().start_background();
        let scope = checkpoint(
            &runtime,
            &r,
            if failed { None } else { Some(1) },
            if failed {
                SyncState::Error
            } else {
                SyncState::RateLimited
            },
            None,
        )
        .await;
        assert!(scope.sync.next_retry_at.is_some());
        assert_eq!(calls.lock().unwrap().len(), 1);
        let budget = runtime
            .store()
            .scope_state(&r.account.id, "provider:rest")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(budget.sync.state, SyncState::RateLimited);
        runtime.shutdown().await.unwrap();
        drop(runtime);
        task.join().unwrap();
        let store = Arc::new(
            Store::open(directory.path().join("activity.sqlite"))
                .await
                .unwrap(),
        );
        assert_eq!(
            store
                .scope_state(&r.account.id, "provider:rest")
                .await
                .unwrap()
                .unwrap()
                .sync
                .next_retry_at,
            budget.sync.next_retry_at
        );
        let loads = vault.loads.load(Ordering::SeqCst);
        let runtime = Arc::new(CollaborationRuntime::new(store, vault.clone(), provider));
        runtime.hydrate_detail(hydration(&r)).await.unwrap();
        runtime.clone().start_background();
        tokio::time::sleep(Duration::from_millis(150)).await;
        assert_eq!(calls.lock().unwrap().len(), 1);
        assert_eq!(
            vault.loads.load(Ordering::SeqCst),
            loads,
            "cold quota admission must precede vault access"
        );
        let local = runtime.store().detail(activity_query(&r)).await.unwrap();
        assert_eq!(local.entries.len(), usize::from(!failed));
        assert_eq!(
            runtime
                .store()
                .draft(&r.account.id, &r.subject.id)
                .await
                .unwrap(),
            Some(draft)
        );
        runtime.shutdown().await.unwrap();
    }
}
