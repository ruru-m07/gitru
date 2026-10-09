//! Finite HTTP qualification for exact-head Bitbucket build statuses.
use super::{
    tests::{REPO, W2, ok, request as feed_request, response, server, token},
    *,
};
use serde_json::{Value, json};

const HEAD: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

fn request() -> CheckRequest {
    let account = feed_request().account;
    let repository_id = format!("bitbucket_cloud:repository:{REPO}");
    CheckRequest {
        detail: DetailRequest {
            repository: RemoteRepository {
                id: repository_id.clone(),
                account_id: account.id.clone(),
                provider_id: REPO.into(),
                full_name: "team/project".into(),
                name: "project".into(),
                web_url: "https://bitbucket.org/team/project".into(),
                description: None,
                default_branch: None,
                selected: true,
            },
            subject: RemoteItem {
                native_inbox: None,
                id: format!("bitbucket_cloud:pull:{REPO}:67"),
                account_id: account.id.clone(),
                repository_id: Some(repository_id),
                provider_id: format!("{REPO}:67"),
                kind: RemoteItemKind::PullRequest,
                number: Some("67".into()),
                title: "cached pull".into(),
                body: None,
                body_omitted: true,
                author: None,
                web_url: None,
                state: "open".into(),
                updated_at: "2026-10-01T00:00:00Z".into(),
                head_oid: Some(HEAD.into()),
                is_draft: None,
                reason: None,
                unread: None,
            },
            account,
            facet: DetailFacet::Checks,
            cursor: None,
            etag: None,
            source: None,
        },
        context: crate::CheckContext {
            head_oid: HEAD.into(),
            source_repository_provider_id: W2.into(),
            metadata_facet_revision: "7".into(),
        },
    }
}

fn path() -> String {
    format!("repositories/%7B%7D/%7B{W2}%7D/commit/{HEAD}/statuses")
}

fn status(base: &str, index: u32, state: &str) -> Value {
    json!({
        "type":"build",
        "key":format!("pipeline-{index}"),
        "name":format!("Pipeline {index}"),
        "state":state,
        "description":format!("status {index}"),
        "created_on":"2026-10-01T00:01:00Z",
        "updated_on":"2026-10-01T00:02:00Z",
        "links":{"commit":{"href":format!("{base}repositories/%7B%7D/%7B{W2}%7D/commit/{HEAD}")}},
        "future":{"ignored":true}
    })
}

fn collection(values: Vec<Value>, next: Option<String>, size: Option<u64>, page: u64) -> Value {
    json!({"values":values,"next":next,"size":size,"pagelen":100,"page":page})
}

fn check(page: &DetailPage, index: usize) -> &crate::CheckV1 {
    match page.entries[index].native.as_ref().unwrap() {
        crate::NativeDetailPayload::CheckV1(value) => value,
        _ => panic!("expected check payload"),
    }
}

#[tokio::test]
async fn exact_source_uuid_and_head_normalize_statuses_without_merging_unknown_states() {
    let (provider, calls) = server(|base| {
        vec![ok(collection(
            vec![status(base, 1, "SUCCESSFUL"), status(base, 2, "FUTURE")],
            None,
            Some(2),
            1,
        ))]
    });
    let page = provider.fetch_checks(&token(), request()).await.unwrap();
    assert_eq!(
        page.reconciliation.enumeration,
        DetailEnumeration::FullEnumeration
    );
    assert_eq!(page.reconciliation.head_scope, DetailHeadScope::CurrentHead);
    assert!(page.next_cursor.is_none());
    assert_eq!(page.entries.len(), 2);
    assert_eq!(check(&page, 0).kind, crate::CheckKind::CommitStatus);
    assert!(matches!(
        &check(&page, 0).state,
        crate::CheckStateV1::CommitStatus { state } if state == "success"
    ));
    assert!(matches!(
        &check(&page, 1).state,
        crate::CheckStateV1::CommitStatus { state } if state == "FUTURE"
    ));
    assert_eq!(page.entries[0].head_oid.as_deref(), Some(HEAD));
    let calls = calls.join().unwrap();
    assert_eq!(calls.len(), 1);
    assert!(calls[0].starts_with(&format!("GET /2.0/{}?pagelen=100 HTTP/1.1", path())));
}

#[tokio::test]
async fn opaque_paging_is_context_bound_and_terminal_totals_are_complete() {
    let (provider, calls) = server(|base| {
        vec![
            ok(collection(
                (0..100)
                    .map(|index| status(base, index, "INPROGRESS"))
                    .collect(),
                Some(format!(
                    "{base}{}?pagelen=100&cursor=opaque%2B%2F%3D",
                    path()
                )),
                Some(101),
                1,
            )),
            ok(collection(
                vec![status(base, 100, "FAILED")],
                None,
                Some(101),
                2,
            )),
        ]
    });
    let mut input = request();
    let first = provider
        .fetch_checks(&token(), input.clone())
        .await
        .unwrap();
    assert_eq!(first.entries.len(), 100);
    assert!(first.next_cursor.is_some());
    input.detail.cursor = first.next_cursor;
    let second = provider.fetch_checks(&token(), input).await.unwrap();
    assert_eq!(second.entries.len(), 1);
    assert!(second.next_cursor.is_none());
    assert_eq!(
        second.reconciliation.enumeration,
        DetailEnumeration::FullEnumeration
    );
    let calls = calls.join().unwrap();
    assert_eq!(calls.len(), 2);
    assert!(calls[1].contains("cursor=opaque%2B%2F%3D"));

    let (provider, calls) = server(|base| {
        vec![ok(collection(
            (0..100)
                .map(|index| status(base, index, "SUCCESSFUL"))
                .collect(),
            Some(format!("{base}{}?pagelen=100&page=2", path())),
            Some(101),
            1,
        ))]
    });
    let mut original = request();
    original.detail.cursor = provider
        .fetch_checks(&token(), original.clone())
        .await
        .unwrap()
        .next_cursor;
    let mut drift = original;
    drift.context.metadata_facet_revision = "8".into();
    assert_eq!(
        provider
            .fetch_checks(&token(), drift)
            .await
            .unwrap_err()
            .kind,
        ProviderErrorKind::InvalidResponse
    );
    assert_eq!(calls.join().unwrap().len(), 1);
}

#[tokio::test]
async fn local_cap_stays_partial_and_empty_exact_total_is_complete() {
    let (provider, calls) = server(|base| {
        (0..10)
            .map(|page| {
                ok(collection(
                    (0..100)
                        .map(|offset| status(base, page * 100 + offset, "SUCCESSFUL"))
                        .collect(),
                    Some(format!("{base}{}?pagelen=100&page={}", path(), page + 2)),
                    Some(1_001),
                    page as u64 + 1,
                ))
            })
            .collect()
    });
    let mut input = request();
    for page_number in 0..10 {
        let page = provider
            .fetch_checks(&token(), input.clone())
            .await
            .unwrap();
        assert_eq!(
            page.reconciliation.enumeration,
            DetailEnumeration::Uncertain
        );
        if page_number < 9 {
            input.detail.cursor = page.next_cursor;
            assert!(input.detail.cursor.is_some());
        } else {
            assert!(page.next_cursor.is_none());
        }
    }
    assert_eq!(calls.join().unwrap().len(), 10);

    let (provider, calls) = server(|_| vec![ok(collection(vec![], None, Some(0), 1))]);
    let page = provider.fetch_checks(&token(), request()).await.unwrap();
    assert!(page.entries.is_empty() && page.next_cursor.is_none());
    assert_eq!(
        page.reconciliation.enumeration,
        DetailEnumeration::FullEnumeration
    );
    assert_eq!(calls.join().unwrap().len(), 1);
}

#[tokio::test]
async fn duplicates_foreign_commit_links_permission_and_quota_fail_closed() {
    let (provider, calls) = server(|base| {
        vec![ok(collection(
            vec![status(base, 1, "SUCCESSFUL"), status(base, 1, "FAILED")],
            None,
            Some(2),
            1,
        ))]
    });
    assert_eq!(
        provider
            .fetch_checks(&token(), request())
            .await
            .unwrap_err()
            .kind,
        ProviderErrorKind::InvalidResponse
    );
    assert_eq!(calls.join().unwrap().len(), 1);

    let (provider, calls) = server(|base| {
        let mut row = status(base, 1, "SUCCESSFUL");
        row["links"]["commit"]["href"] =
            format!("{base}repositories/team/project/commit/{}", "b".repeat(40)).into();
        vec![response(
            200,
            "Retry-After: 120\r\n",
            &collection(vec![row], None, Some(1), 1),
        )]
    });
    let error = provider
        .fetch_checks(&token(), request())
        .await
        .unwrap_err();
    assert_eq!(error.kind, ProviderErrorKind::InvalidResponse);
    assert_eq!(error.account_cooldown_seconds, Some(120));
    assert_eq!(calls.join().unwrap().len(), 1);

    for (status_code, expected) in [
        (403, ProviderErrorKind::Permission),
        (429, ProviderErrorKind::RateLimited),
    ] {
        let (provider, calls) =
            server(|_| vec![response(status_code, "Retry-After: 60\r\n", &json!({}))]);
        let error = provider
            .fetch_checks(&token(), request())
            .await
            .unwrap_err();
        assert_eq!(error.kind, expected);
        assert_eq!(error.account_cooldown_seconds, Some(60));
        assert_eq!(calls.join().unwrap().len(), 1);
    }
}

#[tokio::test]
async fn oversized_description_is_explicit_and_malformed_status_is_atomic() {
    let (provider, calls) = server(|base| {
        let mut row = status(base, 1, "FUTURE");
        row["description"] = json!("x".repeat(65_537));
        vec![ok(collection(vec![row], None, Some(1), 1))]
    });
    let page = provider.fetch_checks(&token(), request()).await.unwrap();
    let value = check(&page, 0);
    assert!(matches!(
        &value.state,
        crate::CheckStateV1::CommitStatus { state } if state == "FUTURE"
    ));
    assert_eq!(value.description.state, crate::DetailValueState::Oversized);
    assert_eq!(calls.join().unwrap().len(), 1);

    let (provider, calls) = server(|base| {
        let mut row = status(base, 1, "SUCCESSFUL");
        row["type"] = json!("deployment");
        vec![ok(collection(vec![row], None, Some(1), 1))]
    });
    assert_eq!(
        provider
            .fetch_checks(&token(), request())
            .await
            .unwrap_err()
            .kind,
        ProviderErrorKind::InvalidResponse
    );
    assert_eq!(calls.join().unwrap().len(), 1);
}
