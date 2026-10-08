//! Finite local HTTP, synthetic tokens only.
use super::{tests::*, *};
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
        facet: DetailFacet::Comments,
        cursor: None,
        etag: None,
        source: None,
    }
}

fn row(id: u64) -> Value {
    json!({"type":"pullrequest_comment","id":id,"created_on":"2026-10-01T00:00:00Z","updated_on":"2026-10-02T00:00:00Z","deleted":false,"pending":false,"content":{"raw":"hello\nΔ","html":"<ignored>"},"user":{"uuid":format!("{{{ACTOR}}}"),"nickname":"author","email":"never retain"},"pullrequest":{"id":67},"links":{"self":{"href":"https://evil.invalid/never"}}})
}
fn path() -> String {
    format!("repositories/%7B%7D/%7B{REPO}%7D/pullrequests/67/comments")
}
#[tokio::test]
async fn comments_native_route_empty_body_deleted_and_own_clock() {
    let mut empty = row(2);
    empty["content"]["raw"] = "".into();
    let mut deleted = row(3);
    deleted["deleted"] = true.into();
    deleted["user"] = json!({"bad":"private actor"});
    let (provider, calls) = server(|_| vec![ok(collection(vec![row(1), empty, deleted], None))]);
    let page = provider
        .fetch_detail(&token(), request_for(REPO))
        .await
        .unwrap();
    assert_eq!(page.reconciliation, DetailReconciliation::full_history());
    assert_eq!(page.source.source, "bitbucket.comments.v1");
    assert!(page.source.provider_updated_at.is_none());
    assert_eq!(
        page.entries[0].updated_at.as_deref(),
        Some("2026-10-02T00:00:00Z")
    );
    assert_eq!(page.entries[0].body.text.as_deref(), Some("hello\nΔ"));
    assert_eq!(page.entries[1].body.text.as_deref(), Some(""));
    assert_eq!(
        page.entries[2].body,
        DetailValue {
            state: DetailValueState::Known,
            text: None
        }
    );
    assert!(page.entries[2].author.is_none());
    assert_eq!(page.entries[2].state.as_deref(), Some("deleted"));
    assert!(
        !serde_json::to_string(&page.entries)
            .unwrap()
            .contains("never")
    );
    let calls = calls.join().unwrap();
    assert_eq!(calls.len(), 1);
    assert!(calls[0].starts_with(&format!("GET /2.0/{}?pagelen=50 HTTP/1.1", path())));
    assert!(!calls[0].to_lowercase().contains("if-none-match"));
}
#[tokio::test]
async fn comments_missing_skipped_and_oversized_are_partial_not_empty_authority() {
    let mut rows = vec![];
    for (key, value) in [
        ("inline", json!({"path":"private"})),
        ("parent", json!({"id":2})),
        ("system", json!(true)),
        ("pending", json!(true)),
        ("type", json!("future_comment")),
    ] {
        let mut v = row(rows.len() as u64 + 1);
        v[key] = value;
        rows.push(v);
    }
    let mut omitted = row(6);
    omitted["content"] = json!({"html":"ignored"});
    rows.push(omitted);
    let mut oversized = row(7);
    oversized["content"]["raw"] = "x".repeat(65_537).into();
    rows.push(oversized);
    let mut no_author = row(8);
    no_author.as_object_mut().unwrap().remove("user");
    rows.push(no_author);
    let (provider, calls) =
        server(|_| vec![ok(collection(rows, None)), ok(collection(vec![], None))]);
    let page = provider
        .fetch_detail(&token(), request_for(REPO))
        .await
        .unwrap();
    assert_eq!(page.reconciliation, DetailReconciliation::default());
    assert_eq!(page.entries.len(), 3);
    assert_eq!(page.entries[0].body.state, DetailValueState::Omitted);
    assert_eq!(page.entries[1].body.state, DetailValueState::Oversized);
    assert!(!page.entries[2].field_mask.contains(&DetailField::Author));
    let page = provider
        .fetch_detail(&token(), request_for(REPO))
        .await
        .unwrap();
    assert_eq!(page.reconciliation, DetailReconciliation::full_history());
    assert!(page.entries.is_empty());
    assert_eq!(calls.join().unwrap().len(), 2);
}
#[tokio::test]
async fn comments_invalid_rows_and_envelopes_preserve_quota() {
    let mut invalids = vec![
        json!({}),
        collection(vec![row(1), row(1)], None),
        collection((1..=51).map(row).collect(), None),
    ];
    for (key, value) in [
        ("id", json!(0)),
        ("updated_on", json!("invalid")),
        ("updated_on", Value::Null),
        ("created_on", json!("2099-01-01T00:00:00Z")),
        ("deleted", Value::Null),
        ("pullrequest", json!({"id":68})),
        ("content", json!({"raw":false})),
    ] {
        let mut v = row(1);
        v[key] = value;
        invalids.push(collection(vec![v], None));
    }
    let count = invalids.len();
    let (provider, calls) = server(|_| {
        invalids
            .into_iter()
            .map(|v| response(200, "Retry-After: 120\r\n", &v))
            .collect()
    });
    for _ in 0..count {
        let e = provider
            .fetch_detail(&token(), request_for(REPO))
            .await
            .unwrap_err();
        assert_eq!(e.kind, ProviderErrorKind::InvalidResponse);
        assert_eq!(e.account_cooldown_seconds, Some(120));
    }
    assert_eq!(calls.join().unwrap().len(), count);
}
#[tokio::test]
async fn comments_continuation_binds_actor_epoch_subject_and_has_no_false_terminal_authority() {
    let (provider, calls) = server(|base| {
        vec![
            ok(collection(
                vec![row(1)],
                Some(format!("{base}{}?pagelen=50&page=2", path())),
            )),
            ok(collection(vec![row(2)], None)),
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
async fn comments_reject_hostile_skipped_cyclic_and_mixed_continuations() {
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
async fn comments_twenty_page_checkpoint_is_bounded_and_not_complete() {
    let (provider, calls) = server(|base| {
        (1..=20)
            .map(|page| {
                ok(collection(
                    vec![],
                    Some(format!("{base}{}?pagelen=50&page={}", path(), page + 1)),
                ))
            })
            .collect()
    });
    let mut request = request_for(REPO);
    for _ in 0..20 {
        let page = provider
            .fetch_detail(&token(), request.clone())
            .await
            .unwrap();
        assert_eq!(page.reconciliation, DetailReconciliation::default());
        request.cursor = page.next_cursor;
    }
    assert_eq!(
        provider
            .fetch_detail(&token(), request)
            .await
            .unwrap_err()
            .kind,
        ProviderErrorKind::InvalidResponse
    );
    assert_eq!(calls.join().unwrap().len(), 20);
}
#[tokio::test]
async fn comments_identity_grants_errors_and_no_conditional_requests() {
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
async fn comments_contradictory_count_is_partial_and_paging_metadata_must_match() {
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
async fn comments_opaque_continuations_are_bounded_and_cycles_preserve_quota() {
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
