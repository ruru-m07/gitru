use super::super::tests::{ACTOR, REPO, collection, ok, response, server, token};
use super::*;
use serde_json::json;
fn request_for(repository: &str) -> DetailRequest {
    DetailRequest {
        account: super::super::tests::request().account,
        repository: RemoteRepository {
            id: format!("bitbucket_cloud:repository:{repository}"),
            account_id: "fixture-account".into(),
            provider_id: repository.into(),
            full_name: "renamed/project".into(),
            name: "project".into(),
            web_url: "https://bitbucket.org/renamed/project".into(),
            description: None,
            default_branch: None,
            selected: true,
        },
        subject: RemoteItem {
            id: format!("bitbucket_cloud:pull:{repository}:67"),
            account_id: "fixture-account".into(),
            repository_id: Some(format!("bitbucket_cloud:repository:{repository}")),
            provider_id: format!("{repository}:67"),
            kind: RemoteItemKind::PullRequest,
            number: Some("67".into()),
            title: "cached PR".into(),
            body: None,
            body_omitted: true,
            author: None,
            web_url: None,
            state: "open".into(),
            updated_at: "not-a-task-clock".into(),
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

fn request() -> DetailRequest {
    request_for(REPO)
}
fn path() -> String {
    format!("repositories/%7B%7D/%7B{REPO}%7D/pullrequests/67/activity")
}
fn event_row(family: &str, second: u64) -> Value {
    let actor = json!({"uuid":format!("{{{ACTOR}}}"),"nickname":"safe Δ","email":"private email","links":{"avatar":{"href":"https://evil.invalid"}}});
    let mut value = json!({"date":format!("2026-10-02T00:00:{second:02}.123456Z"),"user":actor,"pullrequest":{"id":67}});
    if family == "update" {
        value["author"] = value["user"].take();
        value["state"] = "OPEN".into();
        value["title"] = "Title <img>".into();
        value["description"] = "Historical <script>raw text</script>".into();
        value["destination"] = json!({"repository":{"uuid":format!("{{{REPO}}}")},"branch":{"name":"main"},"commit":{"hash":"abc123def456"}});
    }
    json!({family:value,"pull_request":{"id":67,"title":"mutable ignored title"}})
}
fn comment_row(id: u64) -> Value {
    json!({"pull_request":{"id":67},"comment":{"id":id,"type":"pullrequest_comment","created_on":"2026-10-01T00:00:00Z","updated_on":"2026-10-02T00:00:00Z","deleted":false,"content":{"raw":"hello Δ <script>text</script>","html":"never render"},"user":{"uuid":format!("{{{ACTOR}}}"),"nickname":"actor"},"pullrequest":{"id":67},"inline":{"path":"do not expose"}}})
}
#[tokio::test]
async fn families_use_native_route_partial_safe_text_and_separate_identity_domains() {
    let (provider, calls) = server(|_| {
        vec![ok(collection(
            vec![
                comment_row(1),
                event_row("update", 1),
                event_row("approval", 2),
                event_row("changes_requested", 3),
            ],
            None,
        ))]
    });
    let page = provider.fetch_detail(&token(), request()).await.unwrap();
    assert_eq!(page.reconciliation, DetailReconciliation::default());
    assert_eq!(page.entries.len(), 4);
    assert_eq!(page.source.source, SOURCE);
    assert!(page.source.provider_updated_at.is_none());
    assert!(
        page.entries
            .iter()
            .all(|e| e.title.is_none() && e.state.is_none() && e.head_oid.is_none())
    );
    assert_eq!(
        page.entries
            .iter()
            .map(|e| e.provider_id.clone())
            .collect::<HashSet<_>>()
            .len(),
        4
    );
    let encoded = serde_json::to_string(&page.entries).unwrap();
    for private in [
        "private email",
        "evil.invalid",
        "never render",
        "do not expose",
        "mutable ignored",
    ] {
        assert!(!encoded.contains(private));
    }
    assert!(
        page.entries[0]
            .body
            .text
            .as_ref()
            .unwrap()
            .contains("<script>")
    );
    let calls = calls.join().unwrap();
    assert_eq!(calls.len(), 1);
    assert!(calls[0].starts_with(&format!("GET /2.0/{}?pagelen=50 HTTP/1.1", path())));
    assert!(!calls[0].to_lowercase().contains("if-none-match"));
}
#[tokio::test]
async fn observation_identity_normalizes_dates_cosmetics_and_json_but_preserves_semantic_changes() {
    let a = event_row("update", 1);
    let mut cosmetic = a.clone();
    cosmetic["update"]["author"]["nickname"] = "renamed".into();
    cosmetic["update"]["author"]["uuid"] = ACTOR.to_uppercase().into();
    cosmetic["update"]["date"] = "2026-10-02T05:30:01.123456+05:30".into();
    cosmetic["pull_request"]["title"] = "new present-day title".into();
    cosmetic["update"]["destination"]["repository"]["links"] =
        json!({"html":"https://changed.invalid"});
    let mut changed = a.clone();
    changed["update"]["description"] = "a different historical observation".into();
    let (provider, calls) = server(|_| {
        [a, cosmetic, changed]
            .into_iter()
            .map(|row| ok(collection(vec![row], None)))
            .collect()
    });
    let first = provider
        .fetch_detail(&token(), request())
        .await
        .unwrap()
        .entries
        .remove(0);
    let second = provider
        .fetch_detail(&token(), request())
        .await
        .unwrap()
        .entries
        .remove(0);
    let third = provider
        .fetch_detail(&token(), request())
        .await
        .unwrap()
        .entries
        .remove(0);
    assert_eq!(first.id, second.id);
    assert_eq!(first.updated_at, second.updated_at);
    assert_ne!(first.author, second.author);
    assert_ne!(first.id, third.id);
    assert_eq!(calls.join().unwrap().len(), 3);
}
#[tokio::test]
async fn missing_oversized_deleted_unknown_and_duplicate_observations_are_explicit() {
    let mut missing = event_row("update", 1);
    missing["update"]
        .as_object_mut()
        .unwrap()
        .remove("description");
    let mut huge = event_row("update", 2);
    huge["update"]["description"] = "x".repeat(4097).into();
    let mut deleted = comment_row(1);
    deleted["comment"]["deleted"] = true.into();
    deleted["comment"]["user"] = json!({"malformed":"must not retain"});
    let approval = event_row("approval", 4);
    let (provider, calls) = server(|_| {
        vec![ok(collection(
            vec![
                missing,
                huge,
                deleted,
                approval.clone(),
                approval,
                json!({"pull_request":{"id":67},"future_event":{}}),
            ],
            None,
        ))]
    });
    let page = provider.fetch_detail(&token(), request()).await.unwrap();
    assert_eq!(page.entries.len(), 4);
    assert_eq!(page.entries[0].body.state, DetailValueState::Omitted);
    assert_eq!(page.entries[1].body.state, DetailValueState::Oversized);
    assert_eq!(page.entries[2].body.state, DetailValueState::Known);
    assert!(page.entries[2].body.text.is_none());
    assert!(page.entries[2].author.is_none());
    assert_eq!(page.reconciliation, DetailReconciliation::default());
    calls.join().unwrap();
}
#[tokio::test]
async fn malformed_identity_and_semantics_reject_the_page_and_keep_observed_quota() {
    let mut rows = vec![];
    for (path, value) in [
        ("/pull_request/id", json!(68)),
        ("/update/pullrequest/id", json!(68)),
        ("/update/date", json!("invalid")),
        ("/update/author/uuid", json!("invalid")),
        (
            "/update/destination/repository/uuid",
            json!("{11111111-1111-4111-8111-111111111111}"),
        ),
        ("/update/description", json!(false)),
    ] {
        let mut row = event_row("update", 1);
        *row.pointer_mut(path).unwrap() = value;
        rows.push(collection(vec![row], None));
    }
    let mut both = event_row("approval", 1);
    both["update"] = event_row("update", 1)["update"].clone();
    rows.push(collection(vec![both], None));
    rows.push(collection(vec![comment_row(1), comment_row(1)], None));
    rows.push(collection((1..=51).map(comment_row).collect(), None));
    let count = rows.len();
    let (provider, calls) = server(|_| {
        rows.into_iter()
            .map(|row| response(200, "Retry-After: 120\r\n", &row))
            .collect()
    });
    for _ in 0..count {
        let e = provider
            .fetch_detail(&token(), request())
            .await
            .unwrap_err();
        assert_eq!(e.kind, ProviderErrorKind::InvalidResponse);
        assert_eq!(e.account_cooldown_seconds, Some(120));
    }
    assert_eq!(calls.join().unwrap().len(), count);
}
#[tokio::test]
async fn activity_continuation_binds_actor_epoch_subject_and_has_no_false_terminal_authority() {
    let (provider, calls) = server(|base| {
        vec![
            ok(collection(
                vec![comment_row(1)],
                Some(format!("{base}{}?pagelen=50&page=2", path())),
            )),
            ok(collection(vec![comment_row(2)], None)),
        ]
    });
    let first = provider
        .fetch_detail(&token(), request_for(REPO))
        .await
        .unwrap();
    assert_eq!(first.reconciliation, DetailReconciliation::default());
    let saved = first.next_cursor.unwrap();
    for field in [
        "actor",
        "epoch",
        "subject",
        "account",
        "repository",
        "pull",
        "pages",
    ] {
        let mut value: Value = serde_json::from_str(&saved).unwrap();
        value[field] = if field == "pull" || field == "pages" {
            json!(99)
        } else {
            json!("foreign")
        };
        let mut request = request_for(REPO);
        request.cursor = Some(value.to_string());
        assert_eq!(
            provider
                .fetch_detail(&token(), request)
                .await
                .unwrap_err()
                .kind,
            ProviderErrorKind::InvalidResponse
        );
    }
    let mut request = request_for(REPO);
    request.cursor = Some(saved);
    let last = provider.fetch_detail(&token(), request).await.unwrap();
    assert_eq!(last.reconciliation, DetailReconciliation::default());
    assert!(last.next_cursor.is_none());
    assert_eq!(calls.join().unwrap().len(), 2);
}
#[tokio::test]
async fn activity_reject_hostile_skipped_cyclic_and_mixed_continuations() {
    let (provider, calls) = server(|base| {
        let route = format!("{base}{}?pagelen=50", path());
        [
            format!("{route}&page=3"),
            format!("{route}&page=1"),
            format!("{route}&page=2&after=opaque"),
            format!("{route}&page=2&pagelen=50"),
            format!("{route}&page=2&sort=-id"),
            "https://evil.invalid/comments?pagelen=50&page=2".into(),
        ]
        .into_iter()
        .map(|next| response(200, "Retry-After: 120\r\n", &collection(vec![], Some(next))))
        .collect()
    });
    for _ in 0..6 {
        let e = provider
            .fetch_detail(&token(), request_for(REPO))
            .await
            .unwrap_err();
        assert_eq!(e.kind, ProviderErrorKind::InvalidResponse);
        assert_eq!(e.account_cooldown_seconds, Some(120));
    }
    assert_eq!(calls.join().unwrap().len(), 6);
}
#[tokio::test]
async fn activity_identity_grants_errors_and_no_conditional_requests() {
    let (provider, calls) = server(|_| {
        vec![
            response(401, "Retry-After: 80\r\n", &json!({})),
            response(429, "Retry-After: 90\r\n", &json!({})),
            response(304, "", &json!({})),
            response(302, "Location: https://evil.invalid/\r\n", &json!({})),
        ]
    });
    assert_eq!(
        provider
            .profile(&request_for(REPO).account)
            .facet(ResourceFacet::Comments)
            .state,
        CapabilityState::Supported
    );
    for change in 0..4 {
        let mut request = request_for(REPO);
        match change {
            0 => request.subject.kind = RemoteItemKind::Issue,
            1 => request.repository.selected = false,
            2 => request.subject.provider_id = "other".into(),
            _ => request.etag = Some("invented".into()),
        };
        assert!(provider.fetch_detail(&token(), request).await.is_err());
    }
    for expected in [
        ProviderErrorKind::Authentication,
        ProviderErrorKind::RateLimited,
        ProviderErrorKind::InvalidResponse,
        ProviderErrorKind::InvalidResponse,
    ] {
        assert_eq!(
            provider
                .fetch_detail(&token(), request_for(REPO))
                .await
                .unwrap_err()
                .kind,
            expected
        );
    }
    assert_eq!(calls.join().unwrap().len(), 4);
}

#[tokio::test]
async fn activity_contradictory_count_is_partial_and_paging_metadata_must_match() {
    let mut partial = collection(vec![], None);
    partial["size"] = 20.into();
    let mut wrong_page = collection(vec![], None);
    wrong_page["page"] = 2.into();
    let mut huge_page = collection(vec![], None);
    huge_page["pagelen"] = 100.into();
    let (provider, calls) = server(|_| vec![ok(partial), ok(wrong_page), ok(huge_page)]);
    let page = provider
        .fetch_detail(&token(), request_for(REPO))
        .await
        .unwrap();
    assert_eq!(page.reconciliation, DetailReconciliation::default());
    for _ in 0..2 {
        assert_eq!(
            provider
                .fetch_detail(&token(), request_for(REPO))
                .await
                .unwrap_err()
                .kind,
            ProviderErrorKind::InvalidResponse
        );
    }
    assert_eq!(calls.join().unwrap().len(), 3);
}
#[tokio::test]
async fn activity_opaque_continuations_are_bounded_and_cycles_preserve_quota() {
    let (provider, calls) = server(|base| {
        let a = format!("{base}{}?pagelen=50&cursor=opaque-a", path());
        let b = format!("{base}{}?pagelen=50&cursor=opaque-b", path());
        vec![
            ok(collection(vec![], Some(a.clone()))),
            ok(collection(vec![], Some(b))),
            response(200, "Retry-After: 70\r\n", &collection(vec![], Some(a))),
        ]
    });
    let mut request = request_for(REPO);
    for _ in 0..2 {
        request.cursor = provider
            .fetch_detail(&token(), request.clone())
            .await
            .unwrap()
            .next_cursor;
    }
    let error = provider.fetch_detail(&token(), request).await.unwrap_err();
    assert_eq!(error.kind, ProviderErrorKind::InvalidResponse);
    assert_eq!(error.account_cooldown_seconds, Some(70));
    assert_eq!(calls.join().unwrap().len(), 3);
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
    let mut r = request();
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
        (format!("repo:{}:issue", r.repository.id), vec![], vec![]),
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
async fn next_commit(
    store: &Store,
    r: &DetailRequest,
    provider: &BitbucketCloudProvider,
) -> DetailCommit {
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

/// Bounded script uses the real transport. Its longer acceptance watchdog is for
/// SQLite cold startup and ten-page scheduling, never an HTTP throughput claim.
fn runtime_server(
    responses: impl FnOnce(&str) -> Vec<(String, String)>,
) -> (
    Arc<BitbucketCloudProvider>,
    Arc<Mutex<Vec<String>>>,
    std::thread::JoinHandle<()>,
) {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let base = format!("http://{}/2.0/", listener.local_addr().unwrap());
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
                request.starts_with(&format!("GET /2.0/{expected} ")),
                "unexpected fixture route: {}",
                request.lines().next().unwrap()
            );
            captured.lock().unwrap().push(expected);
            stream.write_all(reply.as_bytes()).unwrap();
        }
    });
    (
        Arc::new(BitbucketCloudProvider::fixture(
            reqwest::Url::parse(&base).unwrap(),
        )),
        calls,
        task,
    )
}
async fn checkpoint(
    runtime: &CollaborationRuntime,
    r: &DetailRequest,
    pages: Option<usize>,
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
async fn activity_runtime_ten_page_yield_cold_resume_and_global_cap_keep_bounded_observations() {
    let directory = tempfile::tempdir().unwrap();
    let (store, r, vault, draft) = saved_activity(directory.path()).await;
    let (provider, calls, task) = runtime_server(|base| {
        (1..=20)
            .map(|page| {
                (
                    format!(
                        "{}?pagelen=50{}",
                        path(),
                        if page == 1 {
                            String::new()
                        } else {
                            format!("&page={page}")
                        }
                    ),
                    ok(collection(
                        vec![event_row("approval", page)],
                        Some(format!("{base}{}?pagelen=50&page={}", path(), page + 1)),
                    )),
                )
            })
            .chain([(
                format!("{}?pagelen=50", path()),
                ok(collection(vec![], None)),
            )])
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
    assert_eq!(cursor.pages, 10);
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
    assert_eq!(calls.lock().unwrap().len(), 21);
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
        let (provider, calls, task) = runtime_server(|base| {
            vec![(
                format!("{}?pagelen=50", path()),
                response(
                    if case == 2 { 503 } else { 200 },
                    "Retry-After: 120\r\n",
                    &match case {
                        1 => json!({"malformed":true}),
                        2 => json!({"message":"unavailable"}),
                        _ => collection(
                            vec![event_row("approval", 1)],
                            Some(format!("{base}{}?pagelen=50&page=2", path())),
                        ),
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
#[tokio::test]
async fn activity_held_same_epoch_denial_and_old_epoch_cannot_publish_or_erase_authored_text() {
    for deny in [true, false] {
        let directory = tempfile::tempdir().unwrap();
        let (store, r, _, draft) = saved_activity(directory.path()).await;
        let (provider, task) = server(|_| {
            vec![response(
                200,
                "Retry-After: 120\r\n",
                &collection(vec![event_row("approval", 1)], None),
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

#[tokio::test]
async fn activity_comments_edit_tombstone_and_local_order_survive_restart_without_pruning() {
    let directory = tempfile::tempdir().unwrap();
    let (store, r, vault, draft) = saved_activity(directory.path()).await;
    let mut edited = comment_row(1);
    edited["comment"]["content"]["raw"] = "new text".into();
    edited["comment"]["updated_on"] = "2026-10-03T00:00:00Z".into();
    let mut deleted = edited.clone();
    deleted["comment"]["deleted"] = true.into();
    deleted["comment"]["updated_on"] = "2026-10-04T00:00:00Z".into();
    let (provider, calls) = server(|_| {
        vec![
            ok(collection(
                vec![
                    event_row("update", 2),
                    comment_row(1),
                    event_row("approval", 1),
                ],
                None,
            )),
            ok(collection(vec![edited], None)),
            ok(collection(vec![deleted], None)),
            ok(collection(vec![], None)),
        ]
    });
    store
        .apply_detail(next_commit(&store, &r, &provider).await)
        .await
        .unwrap();
    let mut query = activity_query(&r);
    query.limit = 1;
    let first = store.detail(query.clone()).await.unwrap();
    assert_eq!(event(&first.entries[0]).kind, "commented");
    query.cursor = first.next_cursor.clone();
    let second = store.detail(query).await.unwrap();
    assert_eq!(event(&second.entries[0]).kind, "approved");
    store
        .apply_detail(next_commit(&store, &r, &provider).await)
        .await
        .unwrap();
    let local = store.detail(activity_query(&r)).await.unwrap();
    assert_eq!(
        local
            .entries
            .iter()
            .find(|e| e.id.contains(":comment:"))
            .unwrap()
            .body
            .text
            .as_deref(),
        Some("new text")
    );
    store
        .apply_detail(next_commit(&store, &r, &provider).await)
        .await
        .unwrap();
    store
        .apply_detail(next_commit(&store, &r, &provider).await)
        .await
        .unwrap();
    assert_eq!(calls.join().unwrap().len(), 4);
    store.close().await.unwrap();
    drop(store);
    let store = Store::open(directory.path().join("activity.sqlite"))
        .await
        .unwrap();
    let loads = vault.loads.load(Ordering::SeqCst);
    let local = store.detail(activity_query(&r)).await.unwrap();
    assert_eq!(local.entries.len(), 3);
    assert_eq!(local.evidence.coverage.state, crate::CoverageState::Partial);
    let comment = local
        .entries
        .iter()
        .find(|e| e.id.contains(":comment:"))
        .unwrap();
    assert!(comment.body.text.is_none());
    assert!(comment.author.is_none());
    assert_eq!(
        event(comment).description.as_deref(),
        Some("Comment removed")
    );
    assert_eq!(vault.loads.load(Ordering::SeqCst), loads);
    assert_eq!(
        store.draft(&r.account.id, &r.subject.id).await.unwrap(),
        Some(draft)
    );
    store.close().await.unwrap();
}

#[tokio::test]
async fn terminal_cap_still_rejects_repeated_opaque_continuation_with_quota() {
    let (provider, calls) = server(|base| {
        (1..=20)
            .map(|page| {
                let next = if page == 20 { 2 } else { page + 1 };
                response(
                    200,
                    if page == 20 {
                        "Retry-After: 120\r\n"
                    } else {
                        ""
                    },
                    &collection(
                        vec![event_row("approval", page)],
                        Some(format!("{base}{}?pagelen=50&cursor={next}", path())),
                    ),
                )
            })
            .collect()
    });
    let mut r = request();
    for _ in 0..19 {
        r.cursor = provider
            .fetch_detail(&token(), r.clone())
            .await
            .unwrap()
            .next_cursor;
    }
    let failure = provider.fetch_detail(&token(), r).await.unwrap_err();
    assert_eq!(failure.kind, ProviderErrorKind::InvalidResponse);
    assert_eq!(failure.account_cooldown_seconds, Some(120));
    assert_eq!(calls.join().unwrap().len(), 20);
}
