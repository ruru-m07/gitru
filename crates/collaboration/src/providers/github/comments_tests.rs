//! Finite owned HTTP tests; no provider account, OS vault or public HTTP.
use super::*;
use serde_json::{Value, json};
use std::io::{Read, Write};

const PATH: &str = "repositories/123/issues/67/comments";

fn request(kind: RemoteItemKind) -> DetailRequest {
    let prefix = if kind == RemoteItemKind::PullRequest {
        "pull"
    } else {
        "issue"
    };
    DetailRequest {
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
            id: format!("github:{prefix}:999"),
            account_id: "a".into(),
            repository_id: Some("github:repository:123".into()),
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
        },
        facet: DetailFacet::Comments,
        cursor: None,
        etag: None,
        source: None,
    }
}

fn token() -> SecretToken {
    SecretToken::new("synthetic_comment_token".into()).unwrap()
}

fn row(id: u64) -> Value {
    json!({"id":id,"body":"plain <script>text</script>\nΔ","user":{"id":9007199254740997_u64,"login":"author"},
        "updated_at":"2026-10-05T01:00:00Z","issue_url":"https://api.github.com/repos/owner/project/issues/67",
        "created_at":"ignored-invalid-action-date","body_html":false,"author_association":false,"minimized":false})
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
    assert!(responses.len() <= 20);
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
            stream
                .set_write_timeout(Some(std::time::Duration::from_secs(2)))
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
async fn actual_comments_pr_and_issue_use_immutable_routes_native_ids_and_own_clocks() {
    for kind in [RemoteItemKind::PullRequest, RemoteItemKind::Issue] {
        let mut edited = row(2);
        edited["updated_at"] = "2026-10-05T02:00:00Z".into();
        let mut absent_actor = row(u64::MAX);
        absent_actor["user"] = Value::Null;
        absent_actor["body"] = "".into();
        absent_actor["issue_url"] = "https://api.github.com/repositories/123/issues/67".into();
        let (provider, calls) = server(|_| {
            vec![response(
                200,
                "ETag: \"ignored\"\r\n",
                &json!([edited, row(10), absent_actor]),
            )]
        });
        let page = provider
            .fetch_detail(&token(), request(kind))
            .await
            .unwrap();
        assert_eq!(page.reconciliation, DetailReconciliation::full_history());
        assert_eq!(page.source.source, comments::SOURCE);
        assert_eq!(page.source.adapter_version, 1);
        assert_eq!(
            page.source.field_mask,
            vec![
                DetailField::Body,
                DetailField::Author,
                DetailField::UpdatedAt
            ]
        );
        assert!(page.source.provider_updated_at.is_none());
        assert_eq!(page.entries[0].id, "github-comment:00000000000000000002");
        assert_eq!(page.entries[1].id, "github-comment:00000000000000000010");
        assert_eq!(page.entries[2].provider_id, u64::MAX.to_string());
        assert!(page.entries[0].id < page.entries[1].id && page.entries[1].id < page.entries[2].id);
        assert_eq!(
            page.entries[0].updated_at.as_deref(),
            Some("2026-10-05T02:00:00Z")
        );
        assert_eq!(
            page.entries[1].updated_at.as_deref(),
            Some("2026-10-05T01:00:00Z")
        );
        assert_eq!(page.entries[2].author, None);
        assert_eq!(page.entries[2].body.text.as_deref(), Some(""));
        for entry in &page.entries {
            assert!(
                entry.title.is_none()
                    && entry.state.is_none()
                    && entry.head_oid.is_none()
                    && entry.native.is_none()
            );
            assert!(entry.field_validations.is_empty());
            assert_eq!(entry.observed_body_state, DetailValueState::Known);
        }
        assert_eq!(page.body, DetailValue::default());
        assert!(page.metadata.is_none() && page.etag.is_none() && !page.not_modified);
        let calls = calls.join().unwrap();
        assert_eq!(calls.len(), 1);
        assert!(calls[0].starts_with(&format!("GET /{PATH}?per_page=50 HTTP/1.1")));
        let headers = calls[0].to_ascii_lowercase();
        assert!(headers.contains("authorization: bearer synthetic_comment_token"));
        assert!(headers.contains("x-github-api-version: 2026-03-10"));
        assert!(!headers.contains("if-none-match") && !headers.contains("if-modified-since"));
    }
}

#[tokio::test]
async fn actual_comments_body_omission_and_oversize_keep_observed_body_mask_and_empty_fifty_complete()
 {
    let mut missing = row(1);
    missing.as_object_mut().unwrap().remove("body");
    let mut oversized = row(2);
    oversized["body"] = "x".repeat(65_537).into();
    let mut boundary = row(3);
    boundary["body"] = "x".repeat(65_536).into();
    let (provider, calls) =
        server(|_| vec![response(200, "", &json!([missing, oversized, boundary]))]);
    let page = provider
        .fetch_detail(&token(), request(RemoteItemKind::Issue))
        .await
        .unwrap();
    for (entry, state) in page.entries.iter().zip([
        DetailValueState::Omitted,
        DetailValueState::Oversized,
        DetailValueState::Known,
    ]) {
        assert_eq!(entry.body.state, state);
        assert_eq!(entry.observed_body_state, state);
        assert!(entry.field_mask.contains(&DetailField::Body));
    }
    assert!(page.entries[0].body.text.is_none() && page.entries[1].body.text.is_none());
    assert_eq!(page.entries[2].body.text.as_ref().unwrap().len(), 65_536);
    assert_eq!(calls.join().unwrap().len(), 1);
    for count in [0, 50] {
        let (provider, calls) = server(|_| {
            vec![response(
                200,
                "",
                &Value::Array((1..=count).map(row).collect()),
            )]
        });
        let page = provider
            .fetch_detail(&token(), request(RemoteItemKind::PullRequest))
            .await
            .unwrap();
        assert_eq!(page.entries.len(), count as usize);
        assert_eq!(page.reconciliation, DetailReconciliation::full_history());
        assert!(page.next_cursor.is_none());
        assert_eq!(calls.join().unwrap().len(), 1);
    }
}

#[tokio::test]
async fn actual_comments_invalid_envelopes_rows_identity_and_nullable_shapes_keep_quota() {
    let mut invalids = vec![
        Value::Null,
        json!({"values":[]}),
        json!(false),
        json!([null]),
        json!([row(1), row(1)]),
        Value::Array((1..=51).map(row).collect()),
    ];
    for (key, value) in [
        ("id", json!(0)),
        ("id", json!(-1)),
        ("id", json!("1")),
        ("body", Value::Null),
        ("body", json!(false)),
        ("user", json!(false)),
        ("user", json!({"id":0,"login":"author"})),
        ("user", json!({"id":1,"login":""})),
        ("user", json!({"id":1,"login":"bad\nlogin"})),
        ("user", json!({"id":1,"login":"x".repeat(256)})),
        ("user", json!({"id":1})),
        ("user", json!({"login":"author"})),
        ("user", json!({"id":1,"login":null})),
        ("updated_at", Value::Null),
        ("updated_at", json!("invalid")),
        ("updated_at", json!("x".repeat(129))),
        (
            "issue_url",
            json!("https://api.github.com/repos/other/project/issues/67"),
        ),
        (
            "issue_url",
            json!("https://api.github.com/repositories/124/issues/67"),
        ),
        (
            "issue_url",
            json!("https://api.github.com/repos/owner/project/issues/68"),
        ),
        (
            "issue_url",
            json!("https://api.github.com/repos/owner/project/pulls/67"),
        ),
        (
            "issue_url",
            json!("https://api.github.com/repos/owner/project/issues/67?token=secret"),
        ),
        (
            "issue_url",
            json!("https://secret@api.github.com/repos/owner/project/issues/67"),
        ),
        (
            "issue_url",
            json!("http://api.github.com/repos/owner/project/issues/67"),
        ),
        (
            "issue_url",
            json!("https://api.github.com/repos/owner/project/issues/%36%37"),
        ),
    ] {
        let mut value_row = row(1);
        value_row[key] = value;
        invalids.push(json!([value_row]));
    }
    for key in ["id", "user", "updated_at", "issue_url"] {
        let mut value = row(1);
        value.as_object_mut().unwrap().remove(key);
        invalids.push(json!([value]));
    }
    for body in invalids {
        let (provider, calls) = server(|_| vec![response(200, "Retry-After: 120\r\n", &body)]);
        let error = provider
            .fetch_detail(&token(), request(RemoteItemKind::Issue))
            .await
            .unwrap_err();
        assert_eq!(error.kind, ProviderErrorKind::InvalidResponse);
        assert_eq!(error.account_cooldown_seconds, Some(120));
        assert!(!error.to_string().contains("secret"));
        assert_eq!(calls.join().unwrap().len(), 1);
    }
}

#[tokio::test]
async fn actual_comments_all_headers_and_empty_intermediate_remain_uncertain_through_terminal() {
    let (provider, calls) = server(|base| {
        vec![
            response(
                200,
                &format!(
                    "Link: <{base}{PATH}?page=2&per_page=50>; rel=\"next\"\r\nLink: <{base}{PATH}?per_page=50&page=3>; rel=\"last\"\r\n"
                ),
                &json!([row(1)]),
            ),
            response(
                200,
                &format!(
                    "Link: <{base}{PATH}?per_page=50&page=1>; rel=\"first\", <{base}{PATH}?per_page=50&page=1>; rel=\"prev\"\r\nLink: <{base}{PATH}?per_page=50&page=3>; rel=\"next\", <{base}{PATH}?per_page=50&page=3>; rel=\"last\"\r\n"
                ),
                &json!([]),
            ),
            response(
                200,
                &format!(
                    "Link: <{base}{PATH}?per_page=50&page=1>; rel=\"first\", <{base}{PATH}?per_page=50&page=2>; rel=\"prev\", <{base}{PATH}?per_page=50&page=3>; rel=\"last\"\r\n"
                ),
                &json!([row(3)]),
            ),
        ]
    });
    let mut req = request(RemoteItemKind::PullRequest);
    for (index, count) in [1, 0, 1].into_iter().enumerate() {
        let page = provider.fetch_detail(&token(), req.clone()).await.unwrap();
        assert_eq!(page.entries.len(), count);
        assert_eq!(page.reconciliation, DetailReconciliation::default());
        if let Some(cursor) = &page.next_cursor {
            let cursor: Value = serde_json::from_str(cursor).unwrap();
            assert_eq!(cursor["pages"], index as u64 + 1);
        }
        // Serialization is the same bounded cursor boundary used by cold resume.
        req.cursor = page
            .next_cursor
            .map(|cursor| serde_json::from_str::<Value>(&cursor).unwrap().to_string());
    }
    assert!(req.cursor.is_none());
    let calls = calls.join().unwrap();
    assert_eq!(calls.len(), 3);
    assert!(
        calls[1].starts_with(&format!("GET /{PATH}?page=2&per_page=50 HTTP/1.1")),
        "validated raw target is retained"
    );
}

#[tokio::test]
async fn actual_comments_malformed_or_ambiguous_link_evidence_never_becomes_full_absence() {
    for case in 0..16 {
        let (provider, calls) = server(|base| {
            let next = format!("<{base}{PATH}?per_page=50&page=2>; rel=\"next\"");
            let header = match case {
                0 => "Link: garbage\r\n".into(),
                1 => "Link: \r\n".into(),
                2 => format!("Link: <{base}{PATH}?per_page=50&page=2; rel=\"next\"\r\n"),
                3 => format!("Link: {next}, {next}\r\n"),
                4 => format!("Link: {next}\r\nLink: {next}\r\n"),
                5 => format!("Link: {next}\r\nLink: invalid second header\r\n"),
                6 => format!("Link: <{base}{PATH}?per_page=50&page=2>; rel=next\r\n"),
                7 => format!(
                    "Link: <{base}{PATH}?per_page=50&page=2>; rel=\"next\"; rel=\"last\"\r\n"
                ),
                8 => format!("Link: <{base}{PATH}?per_page=50&page=2>; rel=\"last\"\r\n"),
                9 => format!("Link: <{base}{PATH}?per_page=50&page=1>; rel=\"prev\"\r\n"),
                10 => format!("Link: <{base}{PATH}?per_page=50&page=2>; rel=\"first\"\r\n"),
                11 => format!("Link: {next}, <{base}{PATH}?per_page=50&page=1>; rel=\"last\"\r\n"),
                12 => format!("Link: {next}, \r\n"),
                13 => "Link: ÿ\r\n".into(),
                14 => format!("Link: {}\r\n", "x".repeat(8193)),
                _ => format!("Link: <{base}{PATH}?per_page=50&page=2>; rel=\"unknown\"\r\n"),
            };
            vec![response(
                200,
                &(header + "Retry-After: 120\r\n"),
                &json!([]),
            )]
        });
        let error = provider
            .fetch_detail(&token(), request(RemoteItemKind::Issue))
            .await
            .unwrap_err();
        assert_eq!(
            error.kind,
            ProviderErrorKind::InvalidResponse,
            "case {case}"
        );
        assert_eq!(error.account_cooldown_seconds, Some(120), "case {case}");
        assert_eq!(calls.join().unwrap().len(), 1);
    }
}

#[tokio::test]
async fn actual_comments_hostile_next_paths_queries_aliases_and_redirects_are_refused() {
    for suffix in [
        "?per_page=50&page=1",
        "?per_page=50&page=3",
        "?per_page=50&page=02",
        "?per_page=50&page=0",
        "?per_page=50&page=2&page=3",
        "?per_page=50&per_page=50&page=2",
        "?per_page=100&page=2",
        "?page=2",
        "?per_page=50&page=2&since=2026-01-01",
        "?per_page=50&page=2&sort=id",
        "?per_page=50&page=2&token=secret",
        "?per_page=50&page=%32",
        "?per_page=50&page=2#fragment",
        "?per_page=50&page=2&",
        "?per_page=50&&page=2",
    ] {
        let (provider, calls) = server(|base| {
            vec![response(
                200,
                &format!("Link: <{base}{PATH}{suffix}>; rel=\"next\"\r\nRetry-After: 120\r\n"),
                &json!([]),
            )]
        });
        let error = provider
            .fetch_detail(&token(), request(RemoteItemKind::Issue))
            .await
            .unwrap_err();
        assert_eq!(error.kind, ProviderErrorKind::InvalidResponse);
        assert_eq!(error.account_cooldown_seconds, Some(120));
        assert_eq!(calls.join().unwrap().len(), 1);
    }
    for case in 0..7 {
        let (provider, calls) = server(|base| {
            let target = match case {
                0 => "https://evil.invalid/repositories/123/issues/67/comments?per_page=50&page=2"
                    .into(),
                1 => format!("{base}repositories/124/issues/67/comments?per_page=50&page=2"),
                2 => format!("{base}repositories/123/issues/68/comments?per_page=50&page=2"),
                3 => format!("{base}repos/owner/project/issues/67/comments?per_page=50&page=2"),
                4 => format!("{base}repositories/123/issues/67/../67/comments?per_page=50&page=2"),
                5 => format!("{base}repositories/123/issues/67/%63omments?per_page=50&page=2"),
                _ => format!("{base}repositories\\123\\issues\\67\\comments?per_page=50&page=2"),
            };
            vec![response(
                200,
                &format!("Link: <{target}>; rel=\"next\"\r\nRetry-After: 120\r\n"),
                &json!([]),
            )]
        });
        let error = provider
            .fetch_detail(&token(), request(RemoteItemKind::Issue))
            .await
            .unwrap_err();
        assert_eq!(error.kind, ProviderErrorKind::InvalidResponse);
        assert_eq!(error.account_cooldown_seconds, Some(120));
        assert_eq!(calls.join().unwrap().len(), 1);
    }
    for status in [301, 302, 303, 307, 308, 304] {
        let (provider, calls) = server(|base| {
            vec![response(
                status,
                &format!(
                    "Location: {base}{PATH}?per_page=50&since=2026-01-01\r\nRetry-After: 120\r\n"
                ),
                &json!([]),
            )]
        });
        let error = provider
            .fetch_detail(&token(), request(RemoteItemKind::Issue))
            .await
            .unwrap_err();
        assert_eq!(error.kind, ProviderErrorKind::InvalidResponse);
        assert_eq!(error.account_cooldown_seconds, Some(120));
        assert_eq!(calls.join().unwrap().len(), 1);
    }
}

#[tokio::test]
async fn actual_comments_context_cursor_tampering_and_invalid_request_never_dispatch() {
    let (provider, calls) = server(|base| {
        vec![response(
            200,
            &format!("Link: <{base}{PATH}?per_page=50&page=2>; rel=\"next\"\r\n"),
            &json!([row(1)]),
        )]
    });
    let page = provider
        .fetch_detail(&token(), request(RemoteItemKind::Issue))
        .await
        .unwrap();
    let mut req = request(RemoteItemKind::Issue);
    req.cursor = page.next_cursor;
    for key in [
        "account",
        "actor",
        "epoch",
        "repository",
        "repository_native",
        "subject",
        "subject_native",
        "strategy",
        "kind",
    ] {
        let mut bad = req.clone();
        let mut cursor: Value = serde_json::from_str(bad.cursor.as_ref().unwrap()).unwrap();
        cursor[key] = "foreign".into();
        bad.cursor = Some(cursor.to_string());
        assert_eq!(
            provider.fetch_detail(&token(), bad).await.unwrap_err().kind,
            ProviderErrorKind::InvalidResponse
        );
    }
    for (key, value) in [
        ("version", 2),
        ("number", 68),
        ("pages", 0),
        ("pages", 20),
        ("pages", 2),
    ] {
        let mut bad = req.clone();
        let mut cursor: Value = serde_json::from_str(bad.cursor.as_ref().unwrap()).unwrap();
        cursor[key] = value.into();
        bad.cursor = Some(cursor.to_string());
        assert_eq!(
            provider.fetch_detail(&token(), bad).await.unwrap_err().kind,
            ProviderErrorKind::InvalidResponse
        );
    }
    let mut bad = req.clone();
    bad.cursor = Some("x".repeat(4097));
    assert_eq!(
        provider.fetch_detail(&token(), bad).await.unwrap_err().kind,
        ProviderErrorKind::InvalidResponse
    );
    for case in 0..12 {
        let mut bad = request(RemoteItemKind::Issue);
        match case {
            0 => bad.account.host = "enterprise.invalid".into(),
            1 => bad.account.provider = ProviderKind::Gitlab,
            2 => bad.account.actor_id = "0".into(),
            3 => bad.repository.selected = false,
            4 => bad.repository.provider_id = "0123".into(),
            5 => bad.repository.account_id = "other".into(),
            6 => bad.repository.full_name = "owner/project?query".into(),
            7 => bad.subject.provider_id = "0".into(),
            8 => bad.subject.number = Some("067".into()),
            9 => bad.subject.account_id = "other".into(),
            10 => bad.subject.repository_id = Some("other".into()),
            _ => bad.subject.id = "".into(),
        }
        assert_eq!(
            provider.fetch_detail(&token(), bad).await.unwrap_err().kind,
            ProviderErrorKind::InvalidResponse
        );
    }
    assert_eq!(calls.join().unwrap().len(), 1);
}

#[tokio::test]
async fn actual_comments_opaque_local_ids_and_carried_legacy_validator_use_unconditional_200() {
    let (provider, calls) = server(|_| vec![response(200, "", &json!([row(1)]))]);
    let mut req = request(RemoteItemKind::Issue);
    req.repository.id = "opaque-repository".into();
    req.subject.id = "opaque-subject".into();
    req.subject.repository_id = Some(req.repository.id.clone());
    req.etag = Some("old-unrelated-representation".into());
    let page = provider.fetch_detail(&token(), req).await.unwrap();
    assert_eq!(page.entries.len(), 1);
    assert!(page.etag.is_none());
    let calls = calls.join().unwrap();
    assert_eq!(calls.len(), 1);
    assert!(!calls[0].to_ascii_lowercase().contains("if-none-match"));
}

#[tokio::test]
async fn actual_comments_serialized_twenty_page_budget_keeps_inert_next_twenty_one() {
    let (provider, calls) = server(|base| {
        (1..=20)
            .map(|page| {
                response(
                    200,
                    &format!(
                        "Link: <{base}{PATH}?per_page=50&page={}>; rel=\"next\"\r\n",
                        page + 1
                    ),
                    &json!([]),
                )
            })
            .collect()
    });
    let mut req = request(RemoteItemKind::PullRequest);
    for count in 1..=20 {
        let page = provider.fetch_detail(&token(), req.clone()).await.unwrap();
        assert_eq!(page.reconciliation, DetailReconciliation::default());
        let raw = page.next_cursor.unwrap();
        assert!(raw.len() <= 4096);
        let cursor: Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(cursor["pages"], count);
        assert!(
            cursor["url"]
                .as_str()
                .unwrap()
                .ends_with(&format!("page={}", count + 1))
        );
        req.cursor = Some(cursor.to_string());
    }
    for _ in 0..2 {
        assert_eq!(
            provider
                .fetch_detail(&token(), req.clone())
                .await
                .unwrap_err()
                .kind,
            ProviderErrorKind::InvalidResponse
        );
    }
    assert_eq!(calls.join().unwrap().len(), 20);
}

#[tokio::test]
async fn actual_comments_later_page_loop_and_regression_fail_before_any_returned_cursor() {
    for target in [1, 2, 4] {
        let (provider, calls) = server(|base| {
            vec![
                response(
                    200,
                    &format!("Link: <{base}{PATH}?per_page=50&page=2>; rel=\"next\"\r\n"),
                    &json!([row(1)]),
                ),
                response(
                    200,
                    &format!(
                        "Link: <{base}{PATH}?per_page=50&page={target}>; rel=\"next\"\r\nRetry-After: 120\r\n"
                    ),
                    &json!([row(2)]),
                ),
            ]
        });
        let mut req = request(RemoteItemKind::Issue);
        req.cursor = provider
            .fetch_detail(&token(), req.clone())
            .await
            .unwrap()
            .next_cursor;
        let saved_cursor = req.cursor.clone();
        let error = provider
            .fetch_detail(&token(), req.clone())
            .await
            .unwrap_err();
        assert_eq!(error.kind, ProviderErrorKind::InvalidResponse);
        assert_eq!(error.account_cooldown_seconds, Some(120));
        assert_eq!(req.cursor, saved_cursor);
        assert_eq!(calls.join().unwrap().len(), 2);
    }
}

#[tokio::test]
async fn actual_comments_native_failures_preserve_status_and_quota_without_false_empty() {
    for (status, headers, body, expected, wait) in [
        (
            403,
            "",
            json!({"message":"denied"}),
            ProviderErrorKind::Permission,
            None,
        ),
        (
            403,
            "",
            json!({"message":"secondary rate limit"}),
            ProviderErrorKind::RateLimited,
            Some(60),
        ),
        (
            429,
            "Retry-After: 120\r\n",
            json!({}),
            ProviderErrorKind::RateLimited,
            Some(120),
        ),
        (
            401,
            "Retry-After: 120\r\n",
            json!({}),
            ProviderErrorKind::Authentication,
            Some(120),
        ),
        (404, "", json!({}), ProviderErrorKind::NotFound, None),
        (410, "", json!({}), ProviderErrorKind::NotFound, None),
        (
            500,
            "Retry-After: 120\r\n",
            json!({}),
            ProviderErrorKind::Unavailable,
            Some(120),
        ),
        (204, "", json!([]), ProviderErrorKind::InvalidResponse, None),
    ] {
        let (provider, calls) = server(|_| vec![response(status, headers, &body)]);
        let error = provider
            .fetch_detail(&token(), request(RemoteItemKind::Issue))
            .await
            .unwrap_err();
        assert_eq!(error.kind, expected);
        assert_eq!(error.account_cooldown_seconds, wait);
        assert_eq!(calls.join().unwrap().len(), 1);
    }
    let (provider, calls) = server(|_| {
        vec!["HTTP/1.1 200 OK\r\nContent-Length: 4194305\r\nRetry-After: 120\r\nConnection: close\r\n\r\n".into()]
    });
    let error = provider
        .fetch_detail(&token(), request(RemoteItemKind::Issue))
        .await
        .unwrap_err();
    assert_eq!(error.kind, ProviderErrorKind::InvalidResponse);
    assert_eq!(error.account_cooldown_seconds, Some(120));
    assert_eq!(calls.join().unwrap().len(), 1);
}

#[test]
fn comments_capability_is_github_com_read_only_and_other_facets_remain_explicit() {
    let provider = GithubProvider::new().unwrap();
    let account = request(RemoteItemKind::Issue).account;
    assert_eq!(
        provider
            .profile(&account)
            .facet(ResourceFacet::Comments)
            .state,
        CapabilityState::Supported
    );
    for facet in [
        ResourceFacet::Reviews,
        ResourceFacet::Checks,
        ResourceFacet::Participants,
        ResourceFacet::Tasks,
        ResourceFacet::Merge,
    ] {
        assert_eq!(
            provider.profile(&account).facet(facet).state,
            CapabilityState::Unsupported
        );
    }
    let mut foreign = account;
    foreign.host = "enterprise.invalid".into();
    assert_eq!(
        provider
            .profile(&foreign)
            .facet(ResourceFacet::Comments)
            .state,
        CapabilityState::Unsupported
    );
}
