//! Finite review HTTP fixtures; no provider account, vault or public HTTP.
use super::*;
use crate::NativeDetailPayload;
use serde_json::{Value, json};
use std::io::{Read, Write};

fn context() -> ReviewContext {
    ReviewContext {
        base_oid: "b".repeat(40),
        head_oid: "a".repeat(40),
        base_repository_provider_id: "123".into(),
        source_repository_provider_id: "456".into(),
        metadata_facet_revision: "7".into(),
    }
}

fn request(facet: DetailFacet) -> ReviewRequest {
    ReviewRequest {
        detail: DetailRequest {
            account: RemoteAccount {
                id: "a".into(),
                provider: ProviderKind::Github,
                host: "github.com".into(),
                actor_id: "1".into(),
                login: "actor".into(),
                display_name: None,
                authorization_epoch: "1".into(),
                state: AccountState::Active,
                notifications_supported: false,
            },
            repository: RemoteRepository {
                id: "github:repository:123".into(),
                account_id: "a".into(),
                provider_id: "123".into(),
                full_name: "owner/project".into(),
                name: "project".into(),
                web_url: "https://github.com/owner/project".into(),
                description: None,
                default_branch: None,
                selected: true,
            },
            subject: RemoteItem {
                native_inbox: None,
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
                updated_at: "2026-10-08T00:00:00Z".into(),
                head_oid: Some("a".repeat(40)),
                is_draft: Some(false),
                reason: None,
                unread: None,
            },
            facet,
            cursor: None,
            etag: None,
            source: None,
        },
        context: context(),
    }
}

fn token() -> SecretToken {
    SecretToken::new("synthetic_review_token".into()).unwrap()
}

fn review_row(id: u64, state: &str) -> Value {
    json!({
        "id": id,
        "user": {"id": 88, "login": "reviewer"},
        "body": "review <script> stays text",
        "state": state,
        "submitted_at": "2026-10-08T00:01:00Z",
        "commit_id": "a".repeat(40),
        "pull_request_url": "https://api.github.com/repos/owner/project/pulls/67"
    })
}

fn thread_row(id: u64, parent: Option<u64>) -> Value {
    let mut value = json!({
        "id": id,
        "pull_request_review_id": 41,
        "user": {"id": 89, "login": "commenter"},
        "body": "anchored Δ",
        "created_at": "2026-10-08T00:02:00Z",
        "updated_at": "2026-10-08T00:03:00Z",
        "path": "src/space and [literal].rs",
        "commit_id": "c".repeat(40),
        "original_commit_id": "d".repeat(40),
        "subject_type": "line",
        "start_line": 4,
        "line": 7,
        "start_side": "RIGHT",
        "side": "RIGHT",
        "url": format!("https://api.github.com/repos/owner/project/pulls/comments/{id}"),
        "pull_request_url": "https://api.github.com/repos/owner/project/pulls/67"
    });
    if let Some(parent) = parent {
        value["in_reply_to_id"] = parent.into();
    }
    value
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
                        assert!(std::time::Instant::now() < deadline);
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

#[tokio::test]
async fn reviews_preserve_decision_and_actual_review_commit_separate_from_observation_head() {
    let mut unknown = review_row(9, "FUTURE_STATE");
    unknown["user"] = Value::Null;
    unknown["commit_id"] = Value::Null;
    unknown.as_object_mut().unwrap().remove("submitted_at");
    let (provider, calls) = server(|_| {
        vec![response(
            200,
            "",
            &json!([review_row(2, "APPROVED"), unknown]),
        )]
    });
    let page = provider
        .fetch_reviews(&token(), request(DetailFacet::ReviewSummaries))
        .await
        .unwrap();
    assert_eq!(
        page.reconciliation.enumeration,
        DetailEnumeration::FullEnumeration
    );
    assert_eq!(page.reconciliation.head_scope, DetailHeadScope::CurrentHead);
    assert_eq!(page.entries.len(), 2);
    let NativeDetailPayload::ReviewV1(first) = page.entries[0].native.as_ref().unwrap() else {
        panic!()
    };
    assert_eq!(first.decision, ReviewDecision::Approved);
    assert_eq!(
        first.reviewed_commit_oid.as_deref(),
        Some("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
    );
    assert_eq!(
        page.entries[0].head_oid.as_deref(),
        Some("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
    );
    let NativeDetailPayload::ReviewV1(second) = page.entries[1].native.as_ref().unwrap() else {
        panic!()
    };
    assert_eq!(second.decision, ReviewDecision::Unknown);
    assert!(
        second.reviewer.is_none()
            && second.reviewed_commit_oid.is_none()
            && second.submitted_at.is_none()
    );
    assert_eq!(page.entries[1].state.as_deref(), Some("FUTURE_STATE"));
    let calls = calls.join().unwrap();
    assert!(calls[0].starts_with("GET /repositories/123/pulls/67/reviews?per_page=50 HTTP/1.1"));
    assert!(
        calls[0]
            .to_ascii_lowercase()
            .contains("authorization: bearer synthetic_review_token")
    );
}

#[tokio::test]
async fn review_comments_group_root_and_reply_with_literal_anchor_and_unknown_resolution() {
    let (provider, calls) = server(|_| {
        vec![response(
            200,
            "",
            &json!([thread_row(12, None), thread_row(13, Some(12))]),
        )]
    });
    let page = provider
        .fetch_reviews(&token(), request(DetailFacet::ReviewThreads))
        .await
        .unwrap();
    assert_eq!(
        page.entries[0].id,
        "github-review-thread:00000000000000000012:00000000000000000012"
    );
    assert_eq!(
        page.entries[1].id,
        "github-review-thread:00000000000000000012:00000000000000000013"
    );
    let NativeDetailPayload::ReviewThreadV1(reply) = page.entries[1].native.as_ref().unwrap()
    else {
        panic!()
    };
    assert_eq!(reply.thread_id, "12");
    assert_eq!(reply.root_comment_id.as_deref(), Some("12"));
    assert_eq!(reply.parent_comment_id.as_deref(), Some("12"));
    assert!(reply.provider_outdated.is_none() && reply.provider_resolved.is_none());
    let anchor = reply.anchor.as_ref().unwrap();
    assert_eq!(anchor.path, "src/space and [literal].rs");
    assert_eq!(anchor.line, Some(7));
    assert_eq!(anchor.side, Some(ReviewDiffSide::Right));
    assert_ne!(anchor.commit_oid, reply.context.head_oid);
    let calls = calls.join().unwrap();
    assert!(calls[0].starts_with("GET /repositories/123/pulls/67/comments?per_page=50 HTTP/1.1"));
}

#[tokio::test]
async fn review_facets_page_independently_and_multipage_completion_stays_partial() {
    for facet in [DetailFacet::ReviewSummaries, DetailFacet::ReviewThreads] {
        let collection = if facet == DetailFacet::ReviewSummaries {
            "reviews"
        } else {
            "comments"
        };
        let body = if facet == DetailFacet::ReviewSummaries {
            json!([review_row(1, "COMMENTED")])
        } else {
            json!([thread_row(1, None)])
        };
        let (provider, calls) = server(|base| {
            vec![
                response(
                    200,
                    &format!(
                        "Link: <{base}repositories/123/pulls/67/{collection}?per_page=50&page=2>; rel=\"next\"\r\n"
                    ),
                    &body,
                ),
                response(200, "", &json!([])),
            ]
        });
        let first = provider
            .fetch_reviews(&token(), request(facet))
            .await
            .unwrap();
        assert_eq!(
            first.reconciliation.enumeration,
            DetailEnumeration::Uncertain
        );
        let mut next = request(facet);
        next.detail.cursor = first.next_cursor;
        let terminal = provider.fetch_reviews(&token(), next).await.unwrap();
        assert_eq!(
            terminal.reconciliation.enumeration,
            DetailEnumeration::Uncertain
        );
        assert!(terminal.next_cursor.is_none());
        let calls = calls.join().unwrap();
        assert_eq!(calls.len(), 2);
        assert!(calls[1].lines().next().unwrap().contains("page=2"));
    }
}

#[tokio::test]
async fn review_pages_reject_duplicate_and_cross_pull_identity_without_publishing_rows() {
    let mut wrong = review_row(1, "APPROVED");
    wrong["pull_request_url"] = "https://api.github.com/repos/owner/project/pulls/68".into();
    for body in [
        json!([review_row(1, "APPROVED"), review_row(1, "APPROVED")]),
        json!([wrong]),
    ] {
        let (provider, calls) = server(|_| {
            vec![response(
                200,
                "X-RateLimit-Remaining: 0\r\nX-RateLimit-Reset: 4102444800\r\n",
                &body,
            )]
        });
        let error = provider
            .fetch_reviews(&token(), request(DetailFacet::ReviewSummaries))
            .await
            .unwrap_err();
        assert_eq!(error.kind, ProviderErrorKind::InvalidResponse);
        assert!(error.account_cooldown_seconds.is_some());
        assert_eq!(calls.join().unwrap().len(), 1);
    }
}
