//! Finite owned HTTP fixtures; no public provider account or ambient credential.
use super::*;
use crate::{CheckKind, CheckStateV1, NativeDetailPayload};
use serde_json::{Value, json};
use std::io::{Read, Write};

const HEAD: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

fn request() -> CheckRequest {
    CheckRequest {
        detail: DetailRequest {
            account: RemoteAccount {
                id: "a".into(),
                provider: ProviderKind::Github,
                host: "github.com".into(),
                actor_id: "1".into(),
                login: "actor".into(),
                display_name: None,
                authorization_epoch: "2".into(),
                state: AccountState::Active,
                notifications_supported: false,
            },
            repository: RemoteRepository {
                id: "github:repository:123".into(),
                account_id: "a".into(),
                provider_id: "123".into(),
                full_name: "target/project".into(),
                name: "project".into(),
                web_url: "https://github.com/target/project".into(),
                description: None,
                default_branch: None,
                selected: true,
            },
            subject: RemoteItem {
                id: "github:pull:999".into(),
                account_id: "a".into(),
                repository_id: Some("github:repository:123".into()),
                provider_id: "999".into(),
                kind: RemoteItemKind::PullRequest,
                number: Some("67".into()),
                title: "cached pull".into(),
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
    SecretToken::new("synthetic_checks_token".into()).unwrap()
}

fn status(id: u64) -> Value {
    json!({
        "id": id,
        "state": "success",
        "context": format!("status-{id}"),
        "description": "legacy status",
        "creator": {"login":"status-bot"},
        "updated_at": "2026-10-07T01:00:00Z"
    })
}

fn run(id: u64, head: &str) -> Value {
    json!({
        "id": id,
        "name": format!("check-{id}"),
        "head_sha": head,
        "status": "completed",
        "conclusion": "failure",
        "started_at": "2026-10-07T00:59:00Z",
        "completed_at": "2026-10-07T01:00:00Z",
        "app": {"name":"checks-app"},
        "output": {"title":"title", "summary":"check summary"}
    })
}

fn response(status: u16, headers: &str, body: &Value) -> String {
    let body = body.to_string();
    format!(
        "HTTP/1.1 {status} Fixture\r\nContent-Length: {}\r\nConnection: close\r\n{headers}\r\n{body}",
        body.len()
    )
}

fn server(
    responses: impl FnOnce(&str) -> Vec<String>,
) -> (GithubProvider, std::thread::JoinHandle<Vec<String>>) {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}/", listener.local_addr().unwrap());
    let responses = responses(&base);
    let task = std::thread::spawn(move || {
        listener.set_nonblocking(true).unwrap();
        let mut calls = vec![];
        for response in responses {
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
            let mut stream = loop {
                match listener.accept() {
                    Ok((stream, _)) => break stream,
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        assert!(
                            std::time::Instant::now() < deadline,
                            "finite expected request"
                        );
                        std::thread::sleep(std::time::Duration::from_millis(1));
                    }
                    Err(error) => panic!("owned accept failed: {error}"),
                }
            };
            stream.set_nonblocking(false).unwrap();
            stream
                .set_read_timeout(Some(std::time::Duration::from_secs(2)))
                .unwrap();
            let mut bytes = vec![];
            loop {
                let mut chunk = [0; 1024];
                let count = stream.read(&mut chunk).unwrap();
                bytes.extend_from_slice(&chunk[..count]);
                assert!(bytes.len() <= 16_384);
                if count == 0 || bytes.windows(4).any(|value| value == b"\r\n\r\n") {
                    break;
                }
            }
            calls.push(String::from_utf8(bytes).unwrap());
            stream.write_all(response.as_bytes()).unwrap();
        }
        calls
    });
    (
        GithubProvider::for_test_base(reqwest::Url::parse(&base).unwrap()),
        task,
    )
}

fn status_path() -> String {
    format!("repositories/456/commits/{HEAD}/status")
}
fn checks_path() -> String {
    format!("repositories/456/commits/{HEAD}/check-runs")
}

#[tokio::test]
async fn invalid_initial_check_context_is_rejected_before_http() {
    let (provider, calls) = server(|_| vec![]);
    let mut req = request();
    req.context.metadata_facet_revision = "01".into();
    assert_eq!(
        provider.fetch_checks(&token(), req).await.unwrap_err().kind,
        ProviderErrorKind::InvalidResponse
    );
    assert!(calls.join().unwrap().is_empty());
}

#[tokio::test]
async fn exact_fork_head_keeps_check_run_and_commit_status_states_distinct() {
    let (provider, calls) = server(|_| {
        vec![
            response(
                200,
                "",
                &json!({"sha":HEAD,"total_count":1,"statuses":[status(7)]}),
            ),
            response(
                200,
                "",
                &json!({"total_count":1,"check_runs":[run(8, HEAD)]}),
            ),
        ]
    });
    let page = provider.fetch_checks(&token(), request()).await.unwrap();
    assert_eq!(
        page.reconciliation,
        DetailReconciliation {
            enumeration: DetailEnumeration::FullEnumeration,
            head_scope: DetailHeadScope::CurrentHead,
        }
    );
    assert!(page.next_cursor.is_none());
    assert_eq!(page.entries.len(), 2);
    assert_eq!(page.entries[0].head_oid.as_deref(), Some(HEAD));
    assert_eq!(page.entries[1].head_oid.as_deref(), Some(HEAD));
    let Some(NativeDetailPayload::CheckV1(status)) = &page.entries[0].native else {
        panic!("typed commit status")
    };
    assert_eq!(status.kind, CheckKind::CommitStatus);
    assert!(matches!(
        &status.state,
        CheckStateV1::CommitStatus { state } if state == "success"
    ));
    let Some(NativeDetailPayload::CheckV1(run)) = &page.entries[1].native else {
        panic!("typed check run")
    };
    assert_eq!(run.kind, CheckKind::CheckRun);
    assert!(matches!(
        &run.state,
        CheckStateV1::CheckRun { status, conclusion }
            if status == "completed" && conclusion.as_deref() == Some("failure")
    ));
    let calls = calls.join().unwrap();
    assert_eq!(calls.len(), 2);
    assert!(calls[0].starts_with(&format!("GET /{}?per_page=50 HTTP/1.1", status_path())));
    assert!(calls[1].starts_with(&format!(
        "GET /{}?filter=latest&per_page=50 HTTP/1.1",
        checks_path()
    )));
    assert!(calls.iter().all(|call| {
        call.to_ascii_lowercase()
            .contains("authorization: bearer synthetic_checks_token")
    }));
}

#[tokio::test]
async fn full_multipage_status_traversal_preserves_exact_context_in_cursor() {
    let (provider, calls) = server(|base| {
        vec![
            response(
                200,
                &format!(
                    "Link: <{base}{}?per_page=50&page=2>; rel=\"next\"\r\n",
                    status_path()
                ),
                &json!({"sha":HEAD,"total_count":51,"statuses":(1..=50).map(status).collect::<Vec<_>>() }),
            ),
            response(200, "", &json!({"total_count":0,"check_runs":[]})),
            response(
                200,
                &format!(
                    "Link: <{base}{}?per_page=50&page=1>; rel=\"first\", <{base}{}?per_page=50&page=1>; rel=\"prev\", <{base}{}?per_page=50&page=2>; rel=\"last\"\r\n",
                    status_path(),
                    status_path(),
                    status_path()
                ),
                &json!({"sha":HEAD,"total_count":51,"statuses":[status(51)]}),
            ),
        ]
    });
    let mut req = request();
    let first = provider.fetch_checks(&token(), req.clone()).await.unwrap();
    assert_eq!(first.entries.len(), 50);
    assert_eq!(
        first.reconciliation.enumeration,
        DetailEnumeration::FullEnumeration
    );
    req.detail.cursor = first.next_cursor;
    let second = provider.fetch_checks(&token(), req).await.unwrap();
    assert_eq!(second.entries.len(), 1);
    assert!(second.next_cursor.is_none());
    assert_eq!(
        second.reconciliation.enumeration,
        DetailEnumeration::FullEnumeration
    );
    let calls = calls.join().unwrap();
    assert_eq!(calls.len(), 3);
    assert!(calls[2].starts_with(&format!(
        "GET /{}?per_page=50&page=2 HTTP/1.1",
        status_path()
    )));
}

#[tokio::test]
async fn continuation_rejects_a_changed_family_total() {
    let (provider, calls) = server(|base| {
        vec![
            response(
                200,
                &format!(
                    "Link: <{base}{}?per_page=50&page=2>; rel=\"next\"\r\n",
                    status_path()
                ),
                &json!({"sha":HEAD,"total_count":51,"statuses":(1..=50).map(status).collect::<Vec<_>>() }),
            ),
            response(200, "", &json!({"total_count":0,"check_runs":[]})),
            response(
                200,
                "",
                &json!({"sha":HEAD,"total_count":52,"statuses":[status(51)]}),
            ),
        ]
    });
    let mut req = request();
    let first = provider.fetch_checks(&token(), req.clone()).await.unwrap();
    req.detail.cursor = first.next_cursor;
    assert_eq!(
        provider.fetch_checks(&token(), req).await.unwrap_err().kind,
        ProviderErrorKind::InvalidResponse
    );
    assert_eq!(calls.join().unwrap().len(), 3);
}

#[tokio::test]
async fn check_run_pagination_pins_latest_filter_and_total() {
    let (provider, calls) = server(|base| {
        vec![
            response(200, "", &json!({"sha":HEAD,"total_count":0,"statuses":[]})),
            response(
                200,
                &format!(
                    "Link: <{base}{}?filter=latest&per_page=50&page=2>; rel=\"next\"\r\n",
                    checks_path()
                ),
                &json!({"total_count":51,"check_runs":(1..=50).map(|id| run(id, HEAD)).collect::<Vec<_>>() }),
            ),
            response(
                200,
                "",
                &json!({"total_count":52,"check_runs":[run(51, HEAD)]}),
            ),
        ]
    });
    let mut req = request();
    let first = provider.fetch_checks(&token(), req.clone()).await.unwrap();
    req.detail.cursor = first.next_cursor;
    assert_eq!(
        provider.fetch_checks(&token(), req).await.unwrap_err().kind,
        ProviderErrorKind::InvalidResponse
    );
    let calls = calls.join().unwrap();
    assert_eq!(calls.len(), 3);
    assert!(calls[2].starts_with(&format!(
        "GET /{}?filter=latest&per_page=50&page=2 HTTP/1.1",
        checks_path()
    )));
}

#[tokio::test]
async fn changed_check_run_filter_is_rejected_before_a_continuation_can_be_saved() {
    let (provider, calls) = server(|base| {
        vec![
            response(200, "", &json!({"sha":HEAD,"total_count":0,"statuses":[]})),
            response(
                200,
                &format!(
                    "Link: <{base}{}?filter=all&per_page=50&page=2>; rel=\"next\"\r\n",
                    checks_path()
                ),
                &json!({"total_count":51,"check_runs":(1..=50).map(|id| run(id, HEAD)).collect::<Vec<_>>() }),
            ),
        ]
    });
    assert_eq!(
        provider
            .fetch_checks(&token(), request())
            .await
            .unwrap_err()
            .kind,
        ProviderErrorKind::InvalidResponse
    );
    assert_eq!(calls.join().unwrap().len(), 2);
}

#[tokio::test]
async fn exhausted_quota_stops_before_the_second_family_request() {
    let (provider, calls) = server(|_| {
        vec![response(
            200,
            "Retry-After: 60\r\n",
            &json!({"sha":HEAD,"total_count":0,"statuses":[]}),
        )]
    });
    let error = provider
        .fetch_checks(&token(), request())
        .await
        .unwrap_err();
    assert_eq!(error.kind, ProviderErrorKind::RateLimited);
    assert_eq!(error.retry_after_seconds, Some(60));
    assert_eq!(error.account_cooldown_seconds, Some(60));
    assert_eq!(calls.join().unwrap().len(), 1);
}

#[tokio::test]
async fn malformed_success_pages_preserve_observed_provider_cooldown() {
    let (provider, calls) = server(|_| {
        vec![
            response(200, "", &json!({"sha":HEAD,"total_count":0,"statuses":[]})),
            response(200, "Retry-After: 120\r\n", &json!({"malformed":true})),
        ]
    });
    let error = provider
        .fetch_checks(&token(), request())
        .await
        .unwrap_err();
    assert_eq!(error.kind, ProviderErrorKind::InvalidResponse);
    assert_eq!(error.account_cooldown_seconds, Some(120));
    assert_eq!(calls.join().unwrap().len(), 2);

    let (provider, calls) = server(|base| {
        vec![
            response(
                200,
                &format!(
                    "Link: <{base}{}?per_page=50&page=2>; rel=\"next\"\r\n",
                    status_path()
                ),
                &json!({"sha":HEAD,"total_count":51,"statuses":(1..=50).map(status).collect::<Vec<_>>() }),
            ),
            response(200, "", &json!({"total_count":0,"check_runs":[]})),
            response(200, "Retry-After: 90\r\n", &json!({"malformed":true})),
        ]
    });
    let mut req = request();
    req.detail.cursor = provider
        .fetch_checks(&token(), req.clone())
        .await
        .unwrap()
        .next_cursor;
    let error = provider.fetch_checks(&token(), req).await.unwrap_err();
    assert_eq!(error.kind, ProviderErrorKind::InvalidResponse);
    assert_eq!(error.account_cooldown_seconds, Some(90));
    assert_eq!(calls.join().unwrap().len(), 3);
}

#[tokio::test]
async fn empty_unknown_oversized_duplicate_and_malformed_pages_fail_closed() {
    let (provider, calls) = server(|_| {
        vec![
            response(200, "", &json!({"sha":HEAD,"total_count":0,"statuses":[]})),
            response(200, "", &json!({"total_count":0,"check_runs":[]})),
        ]
    });
    let page = provider.fetch_checks(&token(), request()).await.unwrap();
    assert!(page.entries.is_empty() && page.next_cursor.is_none());
    assert_eq!(
        page.reconciliation.enumeration,
        DetailEnumeration::FullEnumeration
    );
    assert_eq!(calls.join().unwrap().len(), 2);

    let (provider, calls) = server(|_| {
        let mut future = status(1);
        future["state"] = json!("future_state");
        future["description"] = json!("x".repeat(65_537));
        vec![
            response(
                200,
                "",
                &json!({"sha":HEAD,"total_count":1,"statuses":[future]}),
            ),
            response(200, "", &json!({"total_count":0,"check_runs":[]})),
        ]
    });
    let page = provider.fetch_checks(&token(), request()).await.unwrap();
    let Some(NativeDetailPayload::CheckV1(observed)) = &page.entries[0].native else {
        panic!("typed status")
    };
    assert!(matches!(
        &observed.state,
        CheckStateV1::CommitStatus { state } if state == "future_state"
    ));
    assert_eq!(
        observed.description.state,
        crate::DetailValueState::Oversized
    );
    assert_eq!(calls.join().unwrap().len(), 2);

    let (provider, calls) = server(|_| {
        vec![
            response(
                200,
                "",
                &json!({"sha":HEAD,"total_count":2,"statuses":[status(1),status(1)]}),
            ),
            response(200, "", &json!({"total_count":0,"check_runs":[]})),
        ]
    });
    assert_eq!(
        provider
            .fetch_checks(&token(), request())
            .await
            .unwrap_err()
            .kind,
        ProviderErrorKind::InvalidResponse
    );
    assert_eq!(calls.join().unwrap().len(), 2);

    let (provider, calls) = server(|_| {
        let mut malformed = run(1, HEAD);
        malformed["id"] = json!(0);
        vec![
            response(200, "", &json!({"sha":HEAD,"total_count":0,"statuses":[]})),
            response(200, "", &json!({"total_count":1,"check_runs":[malformed]})),
        ]
    });
    assert_eq!(
        provider
            .fetch_checks(&token(), request())
            .await
            .unwrap_err()
            .kind,
        ProviderErrorKind::InvalidResponse
    );
    assert_eq!(calls.join().unwrap().len(), 2);
}

#[tokio::test]
async fn documented_or_local_cap_is_explicitly_uncertain() {
    let (provider, calls) = server(|base| {
        vec![
            response(
                200,
                &format!(
                    "Link: <{base}{}?per_page=50&page=2>; rel=\"next\"\r\n",
                    status_path()
                ),
                &json!({"sha":HEAD,"total_count":501,"statuses":(1..=50).map(status).collect::<Vec<_>>() }),
            ),
            response(200, "", &json!({"total_count":0,"check_runs":[]})),
        ]
    });
    let page = provider.fetch_checks(&token(), request()).await.unwrap();
    assert_eq!(
        page.reconciliation.enumeration,
        DetailEnumeration::Uncertain
    );
    assert_eq!(page.reconciliation.head_scope, DetailHeadScope::CurrentHead);
    assert!(page.next_cursor.is_some());
    assert_eq!(calls.join().unwrap().len(), 2);
}

#[tokio::test]
async fn foreign_head_and_permission_denial_never_become_empty_authority() {
    let (provider, calls) = server(|_| {
        vec![
            response(200, "", &json!({"sha":HEAD,"total_count":0,"statuses":[]})),
            response(
                200,
                "",
                &json!({"total_count":1,"check_runs":[run(1, &"b".repeat(40))]}),
            ),
        ]
    });
    let error = provider
        .fetch_checks(&token(), request())
        .await
        .unwrap_err();
    assert_eq!(error.kind, ProviderErrorKind::InvalidResponse);
    assert_eq!(calls.join().unwrap().len(), 2);

    let (provider, calls) = server(|_| {
        vec![response(
            403,
            "",
            &json!({"message":"Resource not accessible by personal access token"}),
        )]
    });
    let error = provider
        .fetch_checks(&token(), request())
        .await
        .unwrap_err();
    assert_eq!(error.kind, ProviderErrorKind::Permission);
    assert_eq!(calls.join().unwrap().len(), 1);
}
