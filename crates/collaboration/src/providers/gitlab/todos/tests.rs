use super::super::tests::{project, response, server};
use super::*;
use serde_json::json;
fn account() -> RemoteAccount {
    RemoteAccount {
        id: "gitlab-account".into(),
        provider: ProviderKind::Gitlab,
        host: "gitlab.com".into(),
        actor_id: "9".into(),
        login: "actor".into(),
        display_name: None,
        authorization_epoch: "7".into(),
        state: AccountState::Active,
        notifications_supported: false,
    }
}
fn request() -> FeedRequest {
    FeedRequest {
        account: account(),
        kind: FeedKind::Notifications,
        repository: None,
        cursor: None,
        etag: Some("ignored".into()),
        last_modified: None,
    }
}
fn token() -> SecretToken {
    SecretToken::new("synthetic_gitlab_token".into()).unwrap()
}
fn fixture(id: u64, done: bool) -> Value {
    json!({"id":id,"project":project(2,"group/subgroup/repo"),"author":{"id":3,"username":"author"},"action_name":"mentioned","target_type":"MergeRequest","target":{"id":9007199254740993_u64,"iid":7,"project_id":2,"target_project_id":2,"title":"Exact cached MR"},"target_url":"https://gitlab.com/group/subgroup/repo/-/merge_requests/7","body":"Please review","state":if done {"done"} else {"pending"},"updated_at":"2026-10-07T12:00:00Z"})
}
#[tokio::test]
async fn pending_done_phases_preserve_native_identity_and_no_unread() {
    let (provider, task) = server(|base| {
        vec![
            response(
                200,
                &format!("Link: <{base}todos?state=pending&per_page=50&page=2>; rel=\"next\"\r\n"),
                &json!([fixture(9007199254741001, false)]).to_string(),
            ),
            response(
                200,
                "",
                &json!([fixture(9007199254741002, false)]).to_string(),
            ),
            response(
                200,
                "",
                &json!([fixture(9007199254741003, true)]).to_string(),
            ),
        ]
    });
    let first = provider.fetch_page(&token(), request()).await.unwrap();
    assert_eq!(first.items[0].provider_id, "9007199254741001");
    assert_eq!(first.items[0].unread, None);
    assert!(matches!(
        first.items[0].native_inbox,
        Some(NativeInboxState::Todo {
            completion: TodoCompletion::Pending,
            ..
        })
    ));
    let NotificationSubjectMapping::Selector(selector) = &first.notification_subjects[0].mapping
    else {
        panic!("selector")
    };
    assert_eq!(
        selector.subject_provider_id.as_deref(),
        Some("9007199254740993")
    );
    assert_eq!(selector.repository_provider_id, "2");
    assert_eq!(selector.number, "7");
    let mut second = request();
    second.cursor = first.next_cursor;
    let second = provider.fetch_page(&token(), second).await.unwrap();
    let mut third = request();
    third.cursor = second.next_cursor;
    assert!(third.cursor.as_ref().unwrap().contains("\"done\":true"));
    let third = provider.fetch_page(&token(), third).await.unwrap();
    assert!(third.next_cursor.is_none());
    assert_eq!(third.items[0].state, "done");
    let requests = task.join().unwrap();
    for (req, suffix) in requests.iter().zip([
        "state=pending&per_page=50&page=1",
        "state=pending&per_page=50&page=2",
        "state=done&per_page=50&page=1",
    ]) {
        assert!(req.starts_with(&format!("GET /api/v4/todos?{suffix} ")));
        assert!(!req.to_lowercase().contains("if-none-match"));
    }
}

#[tokio::test]
async fn empty_pending_requires_done_traversal_before_complete() {
    let (provider, task) = server(|_| vec![response(200, "", "[]"), response(200, "", "[]")]);
    let pending = provider.fetch_page(&token(), request()).await.unwrap();
    assert!(pending.items.is_empty());
    assert!(pending.next_cursor.is_some());
    let mut req = request();
    req.cursor = pending.next_cursor;
    assert!(
        provider
            .fetch_page(&token(), req)
            .await
            .unwrap()
            .next_cursor
            .is_none()
    );
    assert_eq!(task.join().unwrap().len(), 2);
}

#[tokio::test]
async fn native_cursor_rejects_cross_account_epoch_extra_fields_and_zero_page_without_http() {
    let (provider, task) = server(|_| vec![]);
    for cursor in [
        json!({"version":1,"account":"gitlab-account","epoch":"7","done":false,"page":u64::MAX}),
        json!({"version":1,"account":"gitlab-account","epoch":"7","done":false,"page":1,"resume_done_page":MAX_TODO_PAGE + 1}),
        json!({"version":1,"account":"another","epoch":"7","done":false,"page":2}),
        json!({"version":1,"account":"gitlab-account","epoch":"6","done":false,"page":2}),
        json!({"version":1,"account":"gitlab-account","epoch":"7","done":false,"page":0}),
        json!({"version":1,"account":"gitlab-account","epoch":"7","done":false,"page":2,"url":"https://attacker.invalid"}),
        json!({"version":2,"account":"gitlab-account","epoch":"7","done":false,"page":2}),
        json!({"version":1,"account":"gitlab-account","epoch":"7","done":true,"page":2,"done_pages_since_pending":0}),
        json!({"version":1,"account":"gitlab-account","epoch":"7","done":true,"page":6,"done_pages_since_pending":5}),
        json!({"version":1,"account":"gitlab-account","epoch":"7","done":false,"page":1,"resume_done_page":2}),
        json!({"version":1,"account":"gitlab-account","epoch":"7","done":true,"page":1,"resume_done_page":6}),
        json!({"version":1,"account":"gitlab-account","epoch":"7","done":false,"page":1,"resume_done_page":7}),
        json!({"version":1,"account":"gitlab-account","epoch":"7","done":false,"page":1,"resume_done_page":6,"done_pages_since_pending":1}),
    ] {
        let mut req = request();
        req.cursor = Some(cursor.to_string());
        assert_eq!(
            provider.fetch_page(&token(), req).await.unwrap_err().kind,
            ProviderErrorKind::InvalidResponse
        );
    }
    assert!(task.join().unwrap().is_empty());
}

#[tokio::test]
async fn continuation_cannot_change_state_route_account_or_skip_pages() {
    for suffix in [
        "todos?state=done&per_page=50&page=2",
        "todos?state=pending&per_page=100&page=2",
        "todos?state=pending&per_page=50&page=3",
        "todos?state=pending&per_page=50&page=1",
        "todos?state=pending&per_page=50&page=2&author_id=99",
        "todos?state=pending&per_page=50&page=2&page=2",
        "user?state=pending&per_page=50&page=2",
    ] {
        let (provider, task) = server(|base| {
            vec![response(
                200,
                &format!("Link: <{base}{suffix}>; rel=\"next\"\r\nRateLimit-Remaining: 0\r\n"),
                &json!([fixture(1, false)]).to_string(),
            )]
        });
        let error = provider.fetch_page(&token(), request()).await.unwrap_err();
        assert_eq!(error.kind, ProviderErrorKind::InvalidResponse);
        assert_eq!(error.account_cooldown_seconds, Some(60));
        assert_eq!(task.join().unwrap().len(), 1);
    }
    let (provider, task) = server(|_| {
        vec![response(
            200,
            "Link: <https://attacker.invalid/api/v4/todos?state=pending&per_page=50&page=2>; rel=\"next\"\r\n",
            &json!([fixture(1, false)]).to_string(),
        )]
    });
    assert!(provider.fetch_page(&token(), request()).await.is_err());
    assert_eq!(task.join().unwrap().len(), 1);
}

#[tokio::test]
async fn unknown_projectless_and_missing_targets_remain_truthful_read_only_rows() {
    let mut unknown = fixture(1, false);
    unknown["target_type"] = json!("Namespace");
    unknown["project"] = Value::Null;
    unknown["target"] = Value::Null;
    let mut missing = fixture(2, false);
    missing["target"] = Value::Null;
    let mut unsafe_web = fixture(3, false);
    unsafe_web["target_url"] = json!("https://attacker.invalid/steal");
    unsafe_web["target"]["project_id"] = json!(99);
    let mut issue = fixture(4, false);
    issue["target_type"] = json!("Issue");
    issue["target"]
        .as_object_mut()
        .unwrap()
        .remove("target_project_id");
    issue["target"]["issue_type"] = json!("issue");
    let (provider, task) = server(|_| {
        vec![response(
            200,
            "",
            &json!([unknown, missing, unsafe_web, issue]).to_string(),
        )]
    });
    let page = provider.fetch_page(&token(), request()).await.unwrap();
    assert_eq!(page.items.len(), 4);
    assert!(page.items.iter().all(|i| i.unread.is_none()));
    assert_eq!(page.items[0].repository_id, None);
    assert_eq!(page.items[2].web_url, None);
    assert!(
        page.notification_subjects[..3]
            .iter()
            .all(|o| matches!(o.mapping, NotificationSubjectMapping::Fallback(_)))
    );
    assert!(matches!(
        page.notification_subjects[3].mapping,
        NotificationSubjectMapping::Selector(NotificationSubjectSelector {
            representation: NotificationSubjectRepresentation::GitlabIssue,
            ..
        })
    ));
    assert_eq!(
        provider.profile(&account()).inbox_semantics,
        InboxSemantics::Todos
    );
    assert_eq!(
        provider
            .profile(&account())
            .facet(ResourceFacet::Inbox)
            .state,
        CapabilityState::Supported
    );
    assert_eq!(
        provider.notification_subject_support(&account(), NotificationSubjectKind::Issue),
        CapabilityState::Unsupported
    );
    assert_eq!(task.join().unwrap().len(), 1);
}

#[tokio::test]
async fn malformed_duplicate_and_oversized_pages_preserve_quota_without_success() {
    let mut wrong = fixture(1, true);
    wrong["state"] = json!("unknown");
    let mut malformed = fixture(1, false);
    malformed["id"] = json!(0);
    let mut oversized = fixture(1, false);
    oversized["body"] = json!("x".repeat(1024 * 1024 + 1));
    for body in [
        json!([wrong]),
        json!([malformed]),
        json!([oversized]),
        json!([fixture(1, false), fixture(1, false)]),
        json!((1..=51).map(|id| fixture(id, false)).collect::<Vec<_>>()),
    ] {
        let (provider, task) = server(|_| {
            vec![response(
                200,
                "RateLimit-Remaining: 0\r\n",
                &body.to_string(),
            )]
        });
        let error = provider.fetch_page(&token(), request()).await.unwrap_err();
        assert_eq!(error.kind, ProviderErrorKind::InvalidResponse);
        assert_eq!(error.account_cooldown_seconds, Some(60));
        assert_eq!(task.join().unwrap().len(), 1);
    }
}

#[tokio::test]
async fn forbidden_rate_limit_and_depleted_success_stay_distinct() {
    for (status, expected) in [
        (403, ProviderErrorKind::Permission),
        (429, ProviderErrorKind::RateLimited),
    ] {
        let (provider, task) = server(|_| vec![response(status, "Retry-After: 25\r\n", "{}")]);
        let error = provider.fetch_page(&token(), request()).await.unwrap_err();
        assert_eq!(error.kind, expected);
        assert_eq!(task.join().unwrap().len(), 1);
    }
    let (provider, task) = server(|_| {
        vec![response(
            200,
            "RateLimit-Remaining: 0\r\n",
            &json!([fixture(1, false)]).to_string(),
        )]
    });
    let page = provider.fetch_page(&token(), request()).await.unwrap();
    assert_eq!(page.cooldown_seconds, Some(60));
    assert!(page.next_cursor.is_some());
    assert_eq!(task.join().unwrap().len(), 1);
}
