//! Actual finite HTTP qualification, without provider credentials or public HTTP.
use super::{tests::*, *};
use crate::{NativeDetailPayload, TaskV1};
use serde_json::{Value, json};

fn request_for(repository: &str) -> DetailRequest {
    DetailRequest {
        account: request().account,
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
            native_inbox: None,
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
        },
        facet: DetailFacet::Tasks,
        cursor: None,
        etag: None,
        source: None,
    }
}
fn row(id: u64) -> Value {
    json!({"id":id,"content":{"raw":"task text\nΔ","markup":"ignored","html":false},"creator":{"type":"user","uuid":format!("{{{ACTOR}}}"),"nickname":"same","display_name":"Creator"},"state":"UNRESOLVED","created_on":"2026-10-04T00:00:00Z","updated_on":"2026-10-04T01:00:00Z","pending":true,"resolved_on":"2026-10-04T02:00:00Z","resolved_by":{"type":"app_user","kind":"ignored-app-subkind","uuid":format!("{{{W1}}}"),"nickname":"bot","display_name":"App"},"comment":{"id":23,"links":{"self":{"href":"https://evil.invalid/never-follow"}}}})
}
fn native(entry: &DetailEntry) -> &TaskV1 {
    match entry.native.as_ref().unwrap() {
        NativeDetailPayload::TaskV1(value) => value,
        _ => panic!("task payload"),
    }
}
fn path(repository: &str) -> String {
    format!("repositories/%7B%7D/%7B{repository}%7D/pullrequests/67/tasks")
}

#[tokio::test]
async fn actual_tasks_route_native_accounts_false_null_and_independent_own_clock() {
    let mut value = row(i64::MAX as u64);
    value["creator"]["type"] = "future_account_kind".into();
    value["creator"]["nickname"] = Value::Null;
    value["creator"]["display_name"] = "".into();
    value["state"] = "FUTURE_NATIVE_STATE".into();
    value["pending"] = false.into();
    value["resolved_on"] = Value::Null;
    value["resolved_by"] = Value::Null;
    value["comment"] = Value::Null;
    value["content"]["raw"] = "".into();
    let (provider, calls) = server(|_| vec![ok(collection(vec![value], None))]);
    let page = provider
        .fetch_detail(&token(), request_for(REPO))
        .await
        .unwrap();
    let entry = &page.entries[0];
    let task = native(entry);
    assert_eq!(
        entry.id,
        format!("bitbucket_cloud:task:{REPO}:67:{}", i64::MAX)
    );
    assert_eq!(task.creator.kind, "future_account_kind");
    assert_eq!(task.creator.login, None);
    assert_eq!(task.creator.display_name.as_deref(), Some(""));
    assert_eq!(task.content.text.as_deref(), Some(""));
    assert_eq!(task.pending, Some(false));
    assert_eq!(task.state.as_deref(), Some("FUTURE_NATIVE_STATE"));
    assert!(task.resolved_at.is_none() && task.resolved_by.is_none() && task.comment_id.is_none());
    assert_eq!(entry.field_mask.len(), 12);
    assert!(
        entry.author.is_none()
            && entry.title.is_none()
            && entry.state.is_none()
            && entry.updated_at.is_none()
            && entry.head_oid.is_none()
    );
    assert_eq!(page.source.source, "bitbucket.tasks.v1");
    assert!(page.source.provider_updated_at.is_none());
    assert!(page.metadata.is_none() && page.etag.is_none() && !page.not_modified);
    assert_eq!(page.body, DetailValue::default());
    assert_eq!(page.reconciliation, DetailReconciliation::full_history());
    let calls = calls.join().unwrap();
    assert_eq!(calls.len(), 1);
    assert!(calls[0].starts_with(&format!("GET /2.0/{}?pagelen=50 HTTP/1.1", path(REPO))));
    assert!(calls[0].contains("Bearer synthetic_bitbucket_token"));
    assert!(!calls[0].to_ascii_lowercase().contains("if-none-match"));
}

#[tokio::test]
async fn actual_tasks_omission_oversize_and_non_user_accounts_do_not_invent_observations() {
    let mut absent = row(1);
    absent["creator"] = json!({"type":"team","uuid":format!("{{{ACTOR}}}")});
    for key in ["pending", "resolved_on", "resolved_by", "comment"] {
        absent.as_object_mut().unwrap().remove(key);
    }
    absent["content"] = json!({"html":"ignored"});
    let mut oversized = row(2);
    oversized["content"]["raw"] = "x".repeat(65_537).into();
    let mut boundary = row(3);
    boundary["content"]["raw"] = "x".repeat(65_536).into();
    let (provider, calls) =
        server(|_| vec![ok(collection(vec![absent, oversized, boundary], None))]);
    let page = provider
        .fetch_detail(&token(), request_for(REPO))
        .await
        .unwrap();
    assert_eq!(
        native(&page.entries[0]).content.state,
        DetailValueState::Omitted
    );
    assert_eq!(native(&page.entries[0]).creator.kind, "team");
    assert_eq!(page.entries[0].field_mask.len(), 4);
    assert_eq!(
        native(&page.entries[1]).content.state,
        DetailValueState::Oversized
    );
    assert!(native(&page.entries[1]).content.text.is_none());
    assert_eq!(
        native(&page.entries[1]).resolved_by.as_ref().unwrap().kind,
        "app_user"
    );
    assert_eq!(
        native(&page.entries[2])
            .content
            .text
            .as_ref()
            .unwrap()
            .len(),
        65_536
    );
    assert_eq!(calls.join().unwrap().len(), 1);
}

#[tokio::test]
async fn actual_tasks_empty_and_fifty_rows_are_single_page_full_enumerations() {
    let (provider, calls) = server(|_| {
        vec![
            ok(collection(vec![], None)),
            ok(collection((1..=50).map(row).collect(), None)),
        ]
    });
    for count in [0, 50] {
        let page = provider
            .fetch_detail(&token(), request_for(REPO))
            .await
            .unwrap();
        assert_eq!(page.entries.len(), count);
        assert_eq!(page.reconciliation, DetailReconciliation::full_history());
    }
    assert_eq!(calls.join().unwrap().len(), 2);
}

#[tokio::test]
async fn actual_tasks_invalid_envelopes_required_values_and_native_identity_preserve_received_quota()
 {
    let mut invalids = vec![
        json!({}),
        json!({"values":null}),
        json!({"values":false}),
        collection(vec![row(1), row(1)], None),
        collection((1..=51).map(row).collect(), None),
    ];
    // Each malformed observation is an actual independent HTTP receipt.
    for (key, replacement) in [
        ("id", json!(0)),
        ("id", json!(-1)),
        ("id", json!((i64::MAX as u64) + 1)),
        ("id", json!("1")),
        ("content", Value::Null),
        ("content", json!({"raw":null})),
        ("content", json!({"raw":true})),
        ("creator", Value::Null),
        ("state", Value::Null),
        ("state", json!("")),
        ("state", json!("x".repeat(129))),
        ("pending", Value::Null),
        ("pending", json!("false")),
        ("created_on", json!("bad")),
        ("updated_on", Value::Null),
        ("resolved_on", json!("bad")),
        ("comment", json!({"id":0})),
        ("comment", json!({"id":(i64::MAX as u64)+1})),
        ("resolved_by", json!({"type":"user","uuid":"nil"})),
    ] {
        let mut value = row(1);
        value[key] = replacement;
        invalids.push(collection(vec![value], None));
    }
    for (key, replacement) in [
        ("uuid", json!("00000000-0000-0000-0000-000000000000")),
        ("uuid", json!("same-nickname")),
        ("type", json!("")),
        ("type", json!("future\nkind")),
        ("nickname", json!("x".repeat(256))),
        ("display_name", json!("x".repeat(1025))),
    ] {
        let mut value = row(1);
        value["creator"][key] = replacement;
        invalids.push(collection(vec![value], None));
    }
    for key in [
        "id",
        "content",
        "creator",
        "state",
        "created_on",
        "updated_on",
    ] {
        let mut value = row(1);
        value.as_object_mut().unwrap().remove(key);
        invalids.push(collection(vec![value], None));
    }
    for value in invalids {
        let (provider, calls) = server(|_| vec![response(200, "Retry-After: 120\r\n", &value)]);
        let error = provider
            .fetch_detail(&token(), request_for(REPO))
            .await
            .unwrap_err();
        assert_eq!(error.kind, ProviderErrorKind::InvalidResponse);
        assert_eq!(error.account_cooldown_seconds, Some(120));
        assert_eq!(calls.join().unwrap().len(), 1);
    }
}

#[tokio::test]
async fn actual_tasks_multipage_terminal_and_empty_intermediate_never_upgrade_absence_authority() {
    let (provider, calls) = server(|base| {
        vec![
            ok(collection(
                vec![row(8)],
                Some(format!("{base}{}?pagelen=50&cursor=opaque-A", path(REPO))),
            )),
            ok(collection(
                vec![],
                Some(format!("{base}{}?cursor=opaque-B&pagelen=50", path(REPO))),
            )),
            ok(collection(vec![row(2)], None)),
        ]
    });
    let mut request = request_for(REPO);
    for count in [1, 0, 1] {
        let page = provider
            .fetch_detail(&token(), request.clone())
            .await
            .unwrap();
        assert_eq!(page.entries.len(), count);
        assert_eq!(page.reconciliation, DetailReconciliation::default());
        request.cursor = page.next_cursor;
    }
    assert!(request.cursor.is_none());
    assert_eq!(calls.join().unwrap().len(), 3);
}

#[tokio::test]
async fn actual_tasks_hostile_continuation_never_leaves_exact_child_route() {
    for suffix in [
        "?pagelen=50&page=2&q=state",
        "?pagelen=50&page=2&sort=id",
        "?pagelen=50&page=2&page=3",
        "?pagelen=10&page=2",
        "?pagelen=50",
        "?pagelen=50&cursor=",
        "?pagelen=50&page=2#fragment",
    ] {
        let (provider, calls) = server(|base| {
            vec![ok(collection(
                vec![row(1)],
                Some(format!("{base}{}{suffix}", path(REPO))),
            ))]
        });
        assert_eq!(
            provider
                .fetch_detail(&token(), request_for(REPO))
                .await
                .unwrap_err()
                .kind,
            ProviderErrorKind::InvalidResponse
        );
        assert_eq!(calls.join().unwrap().len(), 1);
    }
    for kind in 0..5 {
        let (provider, calls) = server(|base| {
            let url = match kind {
                0 => "https://evil.invalid/2.0/tasks?pagelen=50&page=2".into(),
                1 => format!("{base}{}?pagelen=50&page=2", path(W2)),
                2 => format!(
                    "{base}{}?pagelen=50&page=2",
                    path(REPO).replace("/67/", "/68/")
                ),
                3 => format!(
                    "{base}{}?pagelen=50&page=2",
                    path(REPO).replace("/tasks", "/comments")
                ),
                _ => format!(
                    "{}credential:secret@{}{}?pagelen=50&page=2",
                    base.split_once("://").unwrap().0.to_owned() + "://",
                    base.split_once("://").unwrap().1,
                    path(REPO)
                ),
            };
            vec![ok(collection(vec![], Some(url)))]
        });
        assert!(
            provider
                .fetch_detail(&token(), request_for(REPO))
                .await
                .is_err()
        );
        assert_eq!(calls.join().unwrap().len(), 1);
    }
}

#[tokio::test]
async fn actual_tasks_private_cursor_rejects_cross_scope_cap_and_alias_loop_before_extra_http() {
    let (provider, calls) = server(|base| {
        vec![
            ok(collection(
                vec![row(1)],
                Some(format!("{base}{}?pagelen=50&page=2", path(REPO))),
            )),
            ok(collection(
                vec![row(2)],
                Some(format!("{base}{}?page=2&pagelen=50", path(REPO))),
            )),
        ]
    });
    let first = provider
        .fetch_detail(&token(), request_for(REPO))
        .await
        .unwrap();
    let mut req = request_for(REPO);
    req.cursor = first.next_cursor;
    for key in ["account", "epoch", "subject", "repository", "strategy"] {
        let mut bad = req.clone();
        let mut cursor: Value = serde_json::from_str(bad.cursor.as_ref().unwrap()).unwrap();
        cursor[key] = "foreign".into();
        bad.cursor = Some(cursor.to_string());
        assert!(provider.fetch_detail(&token(), bad).await.is_err());
    }
    let error = provider.fetch_detail(&token(), req).await.unwrap_err();
    assert_eq!(error.kind, ProviderErrorKind::InvalidResponse);
    assert_eq!(calls.join().unwrap().len(), 2);
    let (provider, calls) = server(|base| {
        (1..=20)
            .map(|n| {
                ok(collection(
                    vec![],
                    Some(format!("{base}{}?pagelen=50&page={}", path(REPO), n + 1)),
                ))
            })
            .collect()
    });
    let mut req = request_for(REPO);
    for _ in 0..20 {
        let page = provider.fetch_detail(&token(), req.clone()).await.unwrap();
        assert_eq!(page.reconciliation, DetailReconciliation::default());
        assert!(page.next_cursor.as_ref().unwrap().len() <= 4096);
        req.cursor = page.next_cursor;
    }
    assert!(provider.fetch_detail(&token(), req).await.is_err());
    assert_eq!(calls.join().unwrap().len(), 20);
}

#[tokio::test]
async fn actual_tasks_pre_dispatch_binding_validator_and_safe_native_errors() {
    let mut requests = vec![];
    let mut req = request_for(REPO);
    req.etag = Some("unqualified validator".into());
    requests.push(req);
    let mut req = request_for(REPO);
    req.repository.selected = false;
    requests.push(req);
    let mut req = request_for(REPO);
    req.subject.provider_id = format!("{W2}:67");
    requests.push(req);
    let mut req = request_for(REPO);
    req.subject.account_id = "other".into();
    requests.push(req);
    let mut req = request_for(REPO);
    req.subject.number = Some(((i64::MAX as u64) + 1).to_string());
    requests.push(req);
    let mut req = request_for(REPO);
    req.cursor = Some("x".repeat(4097));
    requests.push(req);
    let mut req = request_for(REPO);
    req.subject.kind = RemoteItemKind::Issue;
    requests.push(req);
    let (provider, calls) = server(|_| vec![]);
    for req in requests {
        assert!(provider.fetch_detail(&token(), req).await.is_err());
    }
    assert!(calls.join().unwrap().is_empty());
    for (status, kind, wait) in [
        (403, ProviderErrorKind::Permission, None),
        (429, ProviderErrorKind::RateLimited, Some(120)),
    ] {
        let (provider, calls) = server(|_| {
            vec![response(
                status,
                if wait.is_some() {
                    "Retry-After: 120\r\n"
                } else {
                    ""
                },
                &json!({"error":"synthetic"}),
            )]
        });
        let error = provider
            .fetch_detail(&token(), request_for(REPO))
            .await
            .unwrap_err();
        assert_eq!(error.kind, kind);
        assert_eq!(error.account_cooldown_seconds, wait);
        assert_eq!(calls.join().unwrap().len(), 1);
    }
}
