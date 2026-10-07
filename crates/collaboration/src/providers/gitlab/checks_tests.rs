//! Finite exact-head status fixtures; no live account or ambient credential.
use super::{
    tests::{response, server},
    *,
};
use crate::{CheckKind, CheckStateV1, NativeDetailPayload};
use serde_json::json;

const HEAD: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

fn request() -> CheckRequest {
    CheckRequest {
        detail: DetailRequest {
            account: RemoteAccount {
                id: "gitlab-account".into(),
                provider: ProviderKind::Gitlab,
                host: "gitlab.com".into(),
                actor_id: "9007199254740993".into(),
                login: "actor".into(),
                display_name: None,
                authorization_epoch: "1".into(),
                state: AccountState::Active,
                notifications_supported: false,
            },
            repository: RemoteRepository {
                id: "gitlab:repository:123".into(),
                account_id: "gitlab-account".into(),
                provider_id: "123".into(),
                full_name: "org/project".into(),
                name: "project".into(),
                web_url: "https://gitlab.com/org/project".into(),
                description: None,
                default_branch: None,
                selected: true,
            },
            subject: RemoteItem {
                id: "gitlab:pull:999".into(),
                account_id: "gitlab-account".into(),
                repository_id: Some("gitlab:repository:123".into()),
                provider_id: "999".into(),
                kind: RemoteItemKind::PullRequest,
                number: Some("67".into()),
                title: "cached merge request".into(),
                body: None,
                body_omitted: true,
                author: None,
                web_url: None,
                state: "open".into(),
                updated_at: "2026-10-07T00:00:00Z".into(),
                head_oid: Some(HEAD.into()),
                is_draft: None,
                reason: None,
                unread: None,
            },
            facet: DetailFacet::Checks,
            cursor: None,
            etag: None,
            source: None,
        },
        context: crate::CheckContext {
            head_oid: HEAD.into(),
            source_repository_provider_id: "456".into(),
            metadata_facet_revision: "17".into(),
        },
    }
}

fn token() -> SecretToken {
    SecretToken::new("synthetic_gitlab_status_token".into()).unwrap()
}

fn row(id: u64, head: &str) -> serde_json::Value {
    json!({
        "id": id,
        "sha": head,
        "status": "success",
        "name": format!("status-{id}"),
        "description": "status summary",
        "author": {"username":"gitlab-bot"},
        "created_at": "2026-10-07T01:00:00Z",
        "started_at": "2026-10-07T00:59:00Z",
        "finished_at": "2026-10-07T01:00:00Z",
        "allow_failure": true,
        "future": {"ignored":true}
    })
}

fn path() -> String {
    format!("projects/123/repository/commits/{HEAD}/statuses")
}

#[tokio::test]
async fn exact_head_status_is_typed_and_complete_only_with_total_evidence() {
    let body = serde_json::to_string(&vec![row(1, HEAD)]).unwrap();
    let (provider, calls) = server(|_| vec![response(200, "X-Total: 1\r\n", &body)]);
    let page = provider.fetch_checks(&token(), request()).await.unwrap();
    assert_eq!(
        page.reconciliation,
        DetailReconciliation {
            enumeration: DetailEnumeration::FullEnumeration,
            head_scope: DetailHeadScope::CurrentHead,
        }
    );
    assert!(page.next_cursor.is_none());
    let entry = &page.entries[0];
    assert_eq!(entry.head_oid.as_deref(), Some(HEAD));
    let Some(NativeDetailPayload::CheckV1(status)) = &entry.native else {
        panic!("typed status")
    };
    assert_eq!(status.kind, CheckKind::CommitStatus);
    assert!(matches!(
        &status.state,
        CheckStateV1::CommitStatus { state } if state == "success"
    ));
    assert_eq!(status.allow_failure, Some(true));
    let calls = calls.join().unwrap();
    assert_eq!(calls.len(), 1);
    assert!(calls[0].starts_with(&format!(
        "GET /api/v4/{}?all=false&order_by=id&sort=asc&per_page=100&page=1 HTTP/1.1",
        path()
    )));
    assert!(
        calls[0]
            .to_ascii_lowercase()
            .contains("private-token: synthetic_gitlab_status_token")
    );
}

#[tokio::test]
async fn known_gitlab_statuses_normalize_without_losing_allow_failure() {
    let mut failed = row(1, HEAD);
    failed["status"] = json!("failed");
    failed["allow_failure"] = json!(true);
    let mut running = row(2, HEAD);
    running["status"] = json!("running");
    running["allow_failure"] = json!(false);
    let mut canceled = row(3, HEAD);
    canceled["status"] = json!("canceled");
    canceled["allow_failure"] = json!(false);
    let body = serde_json::to_string(&vec![failed, running, canceled]).unwrap();
    let (provider, calls) = server(|_| vec![response(200, "X-Total: 3\r\n", &body)]);
    let page = provider.fetch_checks(&token(), request()).await.unwrap();
    let states = page
        .entries
        .iter()
        .map(|entry| match &entry.native {
            Some(NativeDetailPayload::CheckV1(check)) => {
                let CheckStateV1::CommitStatus { state } = &check.state else {
                    panic!("GitLab status kept as commit status")
                };
                (state.as_str(), check.allow_failure)
            }
            _ => panic!("typed status"),
        })
        .collect::<Vec<_>>();
    assert_eq!(
        states,
        vec![
            ("failure", Some(true)),
            ("pending", Some(false)),
            ("failure", Some(false)),
        ]
    );
    assert_eq!(calls.join().unwrap().len(), 1);
}

#[tokio::test]
async fn empty_unknown_oversized_duplicate_cap_and_quota_evidence_is_explicit() {
    let (provider, calls) = server(|_| vec![response(200, "X-Total: 0\r\n", "[]")]);
    let page = provider.fetch_checks(&token(), request()).await.unwrap();
    assert!(page.entries.is_empty() && page.next_cursor.is_none());
    assert_eq!(
        page.reconciliation.enumeration,
        DetailEnumeration::FullEnumeration
    );
    assert_eq!(calls.join().unwrap().len(), 1);

    let mut future = row(1, HEAD);
    future["status"] = json!("future_state");
    future["description"] = json!("x".repeat(65_537));
    let body = serde_json::to_string(&vec![future]).unwrap();
    let (provider, calls) = server(|_| vec![response(200, "X-Total: 1\r\n", &body)]);
    let page = provider.fetch_checks(&token(), request()).await.unwrap();
    let Some(NativeDetailPayload::CheckV1(status)) = &page.entries[0].native else {
        panic!("typed status")
    };
    assert!(matches!(
        &status.state,
        CheckStateV1::CommitStatus { state } if state == "future_state"
    ));
    assert_eq!(status.description.state, crate::DetailValueState::Oversized);
    assert_eq!(calls.join().unwrap().len(), 1);

    let duplicate = serde_json::to_string(&vec![row(1, HEAD), row(1, HEAD)]).unwrap();
    let (provider, calls) = server(|_| vec![response(200, "X-Total: 2\r\n", &duplicate)]);
    assert_eq!(
        provider
            .fetch_checks(&token(), request())
            .await
            .unwrap_err()
            .kind,
        ProviderErrorKind::InvalidResponse
    );
    assert_eq!(calls.join().unwrap().len(), 1);

    let first =
        serde_json::to_string(&(1..=100).map(|id| row(id, HEAD)).collect::<Vec<_>>()).unwrap();
    let (provider, calls) = server(|base| {
        vec![response(
            200,
            &format!(
                "X-Total: 1001\r\nLink: <{base}{}?all=false&order_by=id&sort=asc&per_page=100&page=2&id=123&sha={HEAD}>; rel=\"next\"\r\n",
                path()
            ),
            &first,
        )]
    });
    let page = provider.fetch_checks(&token(), request()).await.unwrap();
    assert_eq!(page.entries.len(), 100);
    assert!(page.next_cursor.is_some());
    assert_eq!(
        page.reconciliation.enumeration,
        DetailEnumeration::Uncertain
    );
    assert_eq!(calls.join().unwrap().len(), 1);

    let (provider, calls) = server(|_| {
        vec![response(
            429,
            "Retry-After: 60\r\n",
            r#"{"message":"rate limited"}"#,
        )]
    });
    let error = provider
        .fetch_checks(&token(), request())
        .await
        .unwrap_err();
    assert_eq!(error.kind, ProviderErrorKind::RateLimited);
    assert_eq!(error.account_cooldown_seconds, Some(60));
    assert_eq!(calls.join().unwrap().len(), 1);
}

#[tokio::test]
async fn absent_total_keeps_terminal_statuses_explicitly_uncertain() {
    let body = serde_json::to_string(&vec![row(1, HEAD)]).unwrap();
    let (provider, calls) = server(|_| vec![response(200, "", &body)]);
    let page = provider.fetch_checks(&token(), request()).await.unwrap();
    assert_eq!(
        page.reconciliation.enumeration,
        DetailEnumeration::Uncertain
    );
    assert_eq!(page.reconciliation.head_scope, DetailHeadScope::CurrentHead);
    assert!(page.next_cursor.is_none());
    assert_eq!(calls.join().unwrap().len(), 1);
}

#[tokio::test]
async fn exact_pagination_and_foreign_head_permission_fail_closed() {
    let first =
        serde_json::to_string(&(1..=100).map(|id| row(id, HEAD)).collect::<Vec<_>>()).unwrap();
    let last = serde_json::to_string(&vec![row(101, HEAD)]).unwrap();
    let (provider, calls) = server(|base| {
        vec![
            response(
                200,
                &format!(
                    "X-Total: 101\r\nLink: <{base}{}?all=false&order_by=id&sort=asc&per_page=100&page=2&id=123&sha={HEAD}>; rel=\"next\"\r\n",
                    path()
                ),
                &first,
            ),
            response(200, "X-Total: 101\r\n", &last),
        ]
    });
    let mut req = request();
    let first = provider.fetch_checks(&token(), req.clone()).await.unwrap();
    req.detail.cursor = first.next_cursor;
    let second = provider.fetch_checks(&token(), req).await.unwrap();
    assert!(second.next_cursor.is_none());
    assert_eq!(second.entries[0].provider_id, "commit-status:101");
    assert_eq!(calls.join().unwrap().len(), 2);

    let foreign = serde_json::to_string(&vec![row(1, &"b".repeat(40))]).unwrap();
    let (provider, calls) = server(|_| vec![response(200, "X-Total: 1\r\n", &foreign)]);
    assert_eq!(
        provider
            .fetch_checks(&token(), request())
            .await
            .unwrap_err()
            .kind,
        ProviderErrorKind::InvalidResponse
    );
    assert_eq!(calls.join().unwrap().len(), 1);

    let (provider, calls) = server(|_| vec![response(403, "", r#"{"message":"forbidden"}"#)]);
    assert_eq!(
        provider
            .fetch_checks(&token(), request())
            .await
            .unwrap_err()
            .kind,
        ProviderErrorKind::Permission
    );
    assert_eq!(calls.join().unwrap().len(), 1);
}
