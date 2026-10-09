//! Actual native GitLab transport → scheduler → SQLite, synthetic credentials only.
use super::*;
use serde_json::{Value, json};

fn todo(id: u64, done: bool, target_type: &str) -> Value {
    json!({"id":id,"project":project(42,"group/sub/project"),"author":{"id":5,"username":"reviewer"},"action_name":"mentioned","target_type":target_type,"target":{"id":8001,"iid":67,"project_id":42,"target_project_id":42,"title":"Saved subject"},"target_url":"https://gitlab.com/group/sub/project/-/merge_requests/67","body":"Please review","state":if done {"done"} else {"pending"},"updated_at":"2026-10-07T00:00:00Z"})
}
fn query(account: &RemoteAccount, state: Option<&str>) -> ItemQuery {
    ItemQuery {
        account_id: account.id.clone(),
        kind: RemoteItemKind::Notification,
        repository_id: None,
        state: state.map(str::to_string),
        search: None,
        cursor: None,
        limit: 100,
    }
}
fn subject_query(account: &RemoteAccount, id: u64) -> NotificationSubjectQuery {
    NotificationSubjectQuery {
        account_id: account.id.clone(),
        authorization_epoch: account.authorization_epoch.clone(),
        notification_id: format!("gitlab:todo:{id}"),
    }
}
async fn seed_subject(database: &Store, account: &RemoteAccount, native: &str) {
    let repository = "gitlab:repository:42";
    database
        .select_repository(&account.id, repository, true)
        .await
        .unwrap();
    let scope = format!("repo:{repository}:pull_request");
    let run_id = database
        .begin_sync(&account.id, &account.authorization_epoch, &scope)
        .await
        .unwrap();
    database
        .apply_page(PageCommit {
            account_id: account.id.clone(),
            authorization_epoch: account.authorization_epoch.clone(),
            scope,
            run_id,
            repositories: vec![],
            endpoint_aliases: vec![],
            items: vec![RemoteItem {
                native_inbox: None,
                id: format!("gitlab:pull:{native}"),
                account_id: account.id.clone(),
                repository_id: Some(repository.into()),
                provider_id: native.into(),
                kind: RemoteItemKind::PullRequest,
                number: Some("67".into()),
                title: "Cached native merge request".into(),
                body: Some("private saved summary".into()),
                body_omitted: false,
                author: None,
                web_url: None,
                state: "open".into(),
                updated_at: "2026-10-07T00:00:00Z".into(),
                head_oid: None,
                is_draft: None,
                reason: None,
                unread: None,
            }],
            next_cursor: None,
            etag: None,
            last_modified: None,
            not_modified: false,
            complete: true,
            observed_at: "2026-10-07T00:00:00Z".into(),
        })
        .await
        .unwrap();
    database
        .select_repository(&account.id, repository, false)
        .await
        .unwrap();
}
#[tokio::test]
async fn actual_todos_are_local_read_only_and_resolve_exact_cached_subject_after_cold_reopen() {
    let dir = tempfile::tempdir().unwrap();
    let database = store(dir.path()).await;
    let vault = Arc::new(Vault::default());
    let (provider, task) = server(|_| {
        vec![
            response(200, "", r#"{"id":1,"username":"actor"}"#),
            response(200, "", "[]"),
            response(200, "", "[]"),
            response(
                200,
                "",
                &json!([todo(101, false, "MergeRequest")]).to_string(),
            ),
            response(
                200,
                "",
                &json!([todo(102, true, "MergeRequest")]).to_string(),
            ),
        ]
    });
    let provider = Arc::new(provider);
    let runtime = CollaborationRuntime::new(database.clone(), vault.clone(), provider.clone());
    let account = runtime
        .connect_gitlab("synthetic_gitlab_token".into())
        .await
        .unwrap();
    assert!(runtime.run_next().await); // Repository discovery.
    assert!(runtime.run_next().await); // Pending is committed, not a complete inbox.
    let partial = database.query_items(query(&account, None)).await.unwrap();
    assert_eq!(partial.coverage.state, CoverageState::Partial);
    assert_eq!(partial.items.len(), 1);
    assert!(runtime.run_next().await); // Done completes both phases.
    let all = database.query_items(query(&account, None)).await.unwrap();
    assert_eq!(all.coverage.state, CoverageState::Complete);
    assert_eq!(all.items.len(), 2);
    assert!(all.items.iter().all(|item| item.unread.is_none()
        && matches!(item.native_inbox, Some(NativeInboxState::Todo { .. }))));
    assert_eq!(
        database
            .query_items(query(&account, Some("pending")))
            .await
            .unwrap()
            .items[0]
            .id,
        "gitlab:todo:101"
    );
    assert_eq!(
        database
            .query_items(query(&account, Some("done")))
            .await
            .unwrap()
            .items[0]
            .id,
        "gitlab:todo:102"
    );
    seed_subject(&database, &account, "8001").await;
    let resolved = runtime
        .notification_subject(subject_query(&account, 101))
        .await
        .unwrap();
    assert_eq!(resolved.state, NotificationSubjectState::Resolved);
    assert_eq!(resolved.subject.as_ref().unwrap().provider_id, "8001");
    assert_eq!(resolved.discovery.support, CapabilityState::Unsupported);
    assert!(!resolved.discovery.admission);
    assert_eq!(
        resolved.fallback_web_url.as_deref(),
        Some("https://gitlab.com/group/sub/project/-/merge_requests/67")
    );
    let capability = runtime
        .contextual_capabilities(ContextCapabilityRequest {
            account_id: account.id.clone(),
            authorization_epoch: account.authorization_epoch.clone(),
            target: CapabilityTarget {
                kind: CapabilityTargetKind::Account,
                instance_id: None,
                repository_id: None,
                resource_id: None,
                resource_kind: None,
            },
        })
        .await
        .unwrap();
    let inbox = capability
        .facets
        .iter()
        .find(|f| f.facet == ResourceFacet::Inbox)
        .unwrap();
    assert_eq!(capability.inbox_semantics, InboxSemantics::Todos);
    assert_eq!(inbox.saved_read.state, CapabilityState::Supported);
    assert_eq!(inbox.remote_write.state, CapabilityState::Unsupported);
    let calls = task.join().unwrap();
    assert_eq!(calls.len(), 5);
    assert!(calls.iter().all(|c| c.starts_with("GET ")));
    let loads = vault.loads.load(Ordering::SeqCst);
    database.close().await.unwrap();
    drop(runtime);
    drop(database);
    let reopened = store(dir.path()).await;
    let runtime = CollaborationRuntime::new(reopened.clone(), vault.clone(), provider);
    assert_eq!(
        reopened
            .query_items(query(&account, None))
            .await
            .unwrap()
            .items,
        all.items
    );
    assert_eq!(
        runtime
            .notification_subject(subject_query(&account, 101))
            .await
            .unwrap()
            .subject,
        resolved.subject
    );
    assert_eq!(
        vault.loads.load(Ordering::SeqCst),
        loads,
        "offline local reads never load provider credentials"
    );
    // A failed online refresh retains both already-cached states.
    runtime
        .refresh(RefreshRequest {
            account_id: account.id.clone(),
            repository_id: None,
            kind: Some(RemoteItemKind::Notification),
        })
        .await
        .unwrap();
    assert!(runtime.run_next().await);
    let offline = reopened.query_items(query(&account, None)).await.unwrap();
    assert_eq!(offline.items, all.items);
    assert_eq!(offline.sync.state, SyncState::Offline);
}

#[tokio::test]
async fn depleted_pending_phase_persists_and_resumes_done_after_cold_reopen() {
    let dir = tempfile::tempdir().unwrap();
    let database = store(dir.path()).await;
    let vault = Arc::new(Vault::default());
    let clock = Clock::new();
    let (provider, task) = server(|_| {
        vec![
            response(200, "", r#"{"id":1,"username":"actor"}"#),
            response(200, "", "[]"),
            response(200, "", "[]"),
            response(
                200,
                "RateLimit-Remaining: 0\r\n",
                &json!([todo(101, false, "MergeRequest")]).to_string(),
            ),
        ]
    });
    let mut runtime =
        CollaborationRuntime::new(database.clone(), vault.clone(), Arc::new(provider));
    runtime.clock = clock.clone();
    let account = runtime
        .connect_gitlab("synthetic_gitlab_token".into())
        .await
        .unwrap();
    assert!(runtime.run_next().await);
    assert!(runtime.run_next().await);
    assert!(!runtime.run_next().await);
    let checkpoint = database
        .scope_state(&account.id, "notifications")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(checkpoint.coverage.state, CoverageState::Partial);
    assert!(checkpoint.next_cursor.unwrap().contains("\"done\":true"));
    assert_eq!(task.join().unwrap().len(), 4);
    database.close().await.unwrap();
    drop(runtime);
    drop(database);
    let reopened = store(dir.path()).await;
    clock.advance(61);
    let (provider, task) = server(|_| {
        vec![response(
            200,
            "",
            &json!([todo(102, true, "MergeRequest")]).to_string(),
        )]
    });
    let mut runtime = CollaborationRuntime::new(reopened.clone(), vault, Arc::new(provider));
    runtime.clock = clock;
    runtime
        .refresh(RefreshRequest {
            account_id: account.id.clone(),
            repository_id: None,
            kind: Some(RemoteItemKind::Notification),
        })
        .await
        .unwrap();
    assert!(runtime.run_next().await);
    let page = reopened.query_items(query(&account, None)).await.unwrap();
    assert_eq!(page.items.len(), 2);
    assert_eq!(page.coverage.state, CoverageState::Complete);
    let calls = task.join().unwrap();
    assert_eq!(calls.len(), 1);
    assert!(calls[0].starts_with("GET /api/v4/todos?state=done&per_page=50&page=1 "));
}

#[tokio::test]
async fn long_todo_scan_yields_at_native_page_budget_without_claiming_complete() {
    let dir = tempfile::tempdir().unwrap();
    let database = store(dir.path()).await;
    let (provider, task) = server(|base| {
        let mut responses = vec![
            response(200, "", r#"{"id":1,"username":"actor"}"#),
            response(200, "", "[]"),
            response(200, "", "[]"),
        ];
        for page in 1..=11 {
            let next = if page < 11 {
                format!(
                    "Link: <{base}todos?state=pending&per_page=50&page={}>; rel=\"next\"\r\n",
                    page + 1
                )
            } else {
                String::new()
            };
            responses.push(response(
                200,
                &next,
                &json!([todo(page, false, "MergeRequest")]).to_string(),
            ));
        }
        responses.push(response(200, "", "[]"));
        responses
    });
    let runtime = CollaborationRuntime::new(
        database.clone(),
        Arc::new(Vault::default()),
        Arc::new(provider),
    );
    let account = runtime
        .connect_gitlab("synthetic_gitlab_token".into())
        .await
        .unwrap();
    assert!(runtime.run_next().await);
    for _ in 0..MAX_PAGES_PER_REFRESH {
        assert!(runtime.run_next().await);
    }
    assert!(
        !runtime.run_next().await,
        "a refresh must yield after ten committed pages"
    );
    let partial = database.query_items(query(&account, None)).await.unwrap();
    assert_eq!(partial.items.len(), 10);
    assert_eq!(partial.coverage.state, CoverageState::Partial);
    assert!(partial.coverage.remote_has_more);
    runtime
        .refresh(RefreshRequest {
            account_id: account.id.clone(),
            repository_id: None,
            kind: Some(RemoteItemKind::Notification),
        })
        .await
        .unwrap();
    assert!(runtime.run_next().await);
    assert!(runtime.run_next().await);
    let complete = database.query_items(query(&account, None)).await.unwrap();
    assert_eq!(complete.items.len(), 11);
    assert_eq!(complete.coverage.state, CoverageState::Complete);
    let calls = task.join().unwrap();
    assert_eq!(calls.len(), 15);
    assert!(calls[13].starts_with("GET /api/v4/todos?state=pending&per_page=50&page=11 "));
    assert!(calls[14].starts_with("GET /api/v4/todos?state=done"));
}

#[test]
fn legacy_cache_json_decodes_without_inventing_native_inbox_evidence() {
    let old = json!({"id":"legacy","account_id":"a","repository_id":null,"provider_id":"4","kind":"notification","number":null,"title":"legacy","body":null,"body_omitted":false,"author":null,"web_url":null,"state":"PullRequest","updated_at":"2026-10-07T00:00:00Z","head_oid":null,"is_draft":null,"reason":null,"unread":true});
    let decoded: RemoteItem = serde_json::from_value(old).unwrap();
    assert_eq!(decoded.native_inbox, None);
    assert_eq!(decoded.unread, Some(true));
    let selector:NotificationSubjectSelector=serde_json::from_value(json!({"kind":"pull_request","repository_provider_id":"42","number":"67","repository_path":"fixture/project","representation":"github_pull_request"})).unwrap();
    assert_eq!(selector.subject_provider_id, None);
}

#[tokio::test]
async fn pending_sweeps_interleave_done_history_and_failure_restart_keeps_exact_done_checkpoint() {
    let dir = tempfile::tempdir().unwrap();
    let database = store(dir.path()).await;
    let vault = Arc::new(Vault::default());
    let clock = Clock::new();
    let (provider, task) = server(|base| {
        let mut responses = vec![
            response(200, "", r#"{"id":1,"username":"actor"}"#),
            response(200, "", "[]"),
            response(200, "", "[]"),
            response(
                200,
                "",
                &json!([todo(101, false, "MergeRequest")]).to_string(),
            ),
        ];
        for page in 1..=5 {
            responses.push(response(
                200,
                &format!(
                    "Link: <{base}todos?state=done&per_page=50&page={}>; rel=\"next\"\r\n",
                    page + 1
                ),
                &json!([todo(200 + page, true, "MergeRequest")]).to_string(),
            ));
        }
        let mut fresh = todo(101, false, "MergeRequest");
        fresh["target"]["title"] = json!("Fresh pending observation during old done history");
        fresh["updated_at"] = json!("2026-10-07T00:00:01Z");
        responses.push(response(
            200,
            &format!("Link: <{base}todos?state=pending&per_page=50&page=2>; rel=\"next\"\r\n"),
            &json!([fresh]).to_string(),
        ));
        responses.push(response(503, "Retry-After: 2\r\n", "synthetic retry"));
        responses
    });
    let mut runtime =
        CollaborationRuntime::new(database.clone(), vault.clone(), Arc::new(provider));
    runtime.clock = clock.clone();
    let account = runtime
        .connect_gitlab("synthetic_gitlab_token".into())
        .await
        .unwrap();
    for _ in 0..7 {
        assert!(runtime.run_next().await);
    }
    // Initial pending + five done pages, then pending starts again before Done6.
    let saved = database
        .scope_state(&account.id, "notifications")
        .await
        .unwrap()
        .unwrap();
    let next = saved.next_cursor.unwrap();
    assert!(next.contains("\"done\":false"));
    assert!(next.contains("\"resume_done_page\":6"));
    assert_eq!(saved.coverage.state, CoverageState::Partial);
    assert!(runtime.run_next().await);
    let fresh = database
        .query_items(query(&account, Some("pending")))
        .await
        .unwrap();
    assert_eq!(
        fresh.items[0].title,
        "Fresh pending observation during old done history"
    );
    assert_eq!(fresh.coverage.state, CoverageState::Partial);
    let checkpoint = database
        .scope_state(&account.id, "notifications")
        .await
        .unwrap()
        .unwrap()
        .next_cursor;
    assert!(runtime.run_next().await); // The pending continuation fails.
    let failed = database
        .scope_state(&account.id, "notifications")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(failed.next_cursor, checkpoint);
    assert_eq!(failed.coverage.state, CoverageState::Partial);
    assert_eq!(
        database
            .query_items(query(&account, None))
            .await
            .unwrap()
            .items
            .len(),
        6,
        "partial pending failure cannot infer absence or done"
    );
    let calls = task.join().unwrap();
    assert_eq!(calls.len(), 11);
    assert!(calls[9].starts_with("GET /api/v4/todos?state=pending&per_page=50&page=1 "));
    assert!(calls[10].starts_with("GET /api/v4/todos?state=pending&per_page=50&page=2 "));
    database.close().await.unwrap();
    drop(runtime);
    drop(database);
    clock.advance(3);
    let reopened = store(dir.path()).await;
    assert_eq!(
        reopened
            .scope_state(&account.id, "notifications")
            .await
            .unwrap()
            .unwrap()
            .next_cursor,
        checkpoint
    );
    let (provider, task) = server(|_| {
        vec![
            response(
                200,
                "",
                &json!([todo(102, false, "MergeRequest")]).to_string(),
            ),
            response(
                200,
                "",
                &json!([todo(206, true, "MergeRequest")]).to_string(),
            ),
        ]
    });
    let mut runtime = CollaborationRuntime::new(reopened.clone(), vault, Arc::new(provider));
    runtime.clock = clock;
    runtime
        .refresh(RefreshRequest {
            account_id: account.id.clone(),
            repository_id: None,
            kind: Some(RemoteItemKind::Notification),
        })
        .await
        .unwrap();
    assert!(runtime.run_next().await);
    assert!(runtime.run_next().await);
    let complete = reopened.query_items(query(&account, None)).await.unwrap();
    assert_eq!(complete.coverage.state, CoverageState::Complete);
    assert_eq!(complete.items.len(), 8);
    let calls = task.join().unwrap();
    assert_eq!(calls.len(), 2);
    assert!(calls[0].starts_with("GET /api/v4/todos?state=pending&per_page=50&page=2 "));
    assert!(calls[1].starts_with("GET /api/v4/todos?state=done&per_page=50&page=6 "));
}

#[tokio::test]
async fn exact_todo_grant_hydrates_unselected_subject_and_withdrawal_fences_queued_work() {
    let dir = tempfile::tempdir().unwrap();
    let database = store(dir.path()).await;
    let (provider, task) = server(|_| {
        vec![
        response(200, "", r#"{"id":1,"username":"actor"}"#),
        response(200, "", "[]"), response(200, "", "[]"),
        response(200, "", &json!([todo(101, false, "MergeRequest")]).to_string()),
        response(200, "", "[]"),
        response(200, "", &json!({"id":8001,"iid":67,"project_id":42,"target_project_id":42,
            "title":"Cached native merge request","description":"Fresh body through current todo grant",
            "state":"opened","updated_at":"2026-10-07T00:00:01Z",
            "web_url":"https://gitlab.com/group/sub/project/-/merge_requests/67"}).to_string()),
    ]
    });
    let runtime = CollaborationRuntime::new(
        database.clone(),
        Arc::new(Vault::default()),
        Arc::new(provider),
    );
    let account = runtime
        .connect_gitlab("synthetic_gitlab_token".into())
        .await
        .unwrap();
    for _ in 0..3 {
        assert!(runtime.run_next().await);
    }
    seed_subject(&database, &account, "8001").await;
    assert!(
        !database
            .repository(&account.id, "gitlab:repository:42")
            .await
            .unwrap()
            .selected
    );
    let demand = HydrateDetailRequest {
        account_id: account.id.clone(),
        authorization_epoch: account.authorization_epoch.clone(),
        subject_id: "gitlab:pull:8001".into(),
        facet: DetailFacet::Body,
    };
    runtime.hydrate_detail(demand.clone()).await.unwrap();
    assert!(runtime.run_next().await);
    let saved = database
        .detail(DetailQuery {
            account_id: account.id.clone(),
            subject_id: demand.subject_id.clone(),
            facet: DetailFacet::Body,
            cursor: None,
            limit: 50,
        })
        .await
        .unwrap();
    assert_eq!(
        saved.body.text.as_deref(),
        Some("Fresh body through current todo grant")
    );
    let calls = task.join().unwrap();
    assert_eq!(calls.len(), 6);
    assert!(calls[5].starts_with("GET /api/v4/projects/42/merge_requests/67 "));

    // Current exact todo provenance, not the repository's selection bit, owns
    // point-read authority. Removing it also retires already queued demand.
    runtime.hydrate_detail(demand.clone()).await.unwrap();
    let mut original = database
        .query_items(query(&account, None))
        .await
        .unwrap()
        .items
        .remove(0);
    original.updated_at = "2026-10-07T00:00:02Z".into();
    let run = database
        .begin_sync(&account.id, &account.authorization_epoch, "notifications")
        .await
        .unwrap();
    database
        .apply_page_with_notification_subjects(
            PageCommit {
                account_id: account.id.clone(),
                authorization_epoch: account.authorization_epoch.clone(),
                scope: "notifications".into(),
                run_id: run,
                repositories: vec![],
                endpoint_aliases: vec![],
                items: vec![original],
                next_cursor: None,
                etag: None,
                last_modified: None,
                not_modified: false,
                complete: true,
                observed_at: "2026-10-07T00:00:02Z".into(),
            },
            vec![NotificationSubjectObservation {
                notification_id: "gitlab:todo:101".into(),
                mapping: NotificationSubjectMapping::Fallback(
                    NotificationSubjectFallbackReason::UnsupportedSubjectType,
                ),
            }],
        )
        .await
        .unwrap();
    assert!(runtime.run_next().await);
    assert!(matches!(
        runtime.hydrate_detail(demand).await.unwrap_err().code,
        ErrorCode::PermissionDenied | ErrorCode::NotFound
    ));
    let state = database
        .scope_state(&account.id, &DetailFacet::Body.scope("gitlab:pull:8001"))
        .await
        .unwrap();
    assert!(
        state.is_none_or(|s| s.sync.state != SyncState::Offline),
        "withdrawn queued work must not contact the closed synthetic provider"
    );
}
