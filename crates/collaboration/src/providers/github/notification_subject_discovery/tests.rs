use super::*;
use crate::{NotificationSubjectSelector, resource_metadata::MetadataField};
use serde_json::json;
use std::{
    io::{Read, Write},
    net::TcpListener,
    thread,
    time::{Duration, Instant},
};

fn request(kind: Kind) -> TrustedNotificationSubjectRequest {
    TrustedNotificationSubjectRequest {
        account: RemoteAccount {
            id: "fixture-account".into(),
            provider: ProviderKind::Github,
            host: "github.com".into(),
            actor_id: "9007199254741023".into(),
            login: "fixture-account-login".into(),
            display_name: None,
            authorization_epoch: "7".into(),
            state: AccountState::Active,
            notifications_supported: true,
        },
        instance_id: ProviderInstance::public(ProviderKind::Github).id,
        notification_id: "github:notification:777".into(),
        selector_generation: "opaque-generation".into(),
        authorization_view: "3".into(),
        repository: RemoteRepository {
            id: "github:repo:9007199254741021".into(),
            account_id: "fixture-account".into(),
            provider_id: "9007199254741021".into(),
            full_name: "fixture/project".into(),
            name: "project".into(),
            web_url: "https://github.com/fixture/project".into(),
            description: None,
            default_branch: None,
            selected: false,
        },
        selector: NotificationSubjectSelector {
            kind,
            repository_provider_id: "9007199254741021".into(),
            number: if kind == Kind::PullRequest {
                "67"
            } else {
                "68"
            }
            .into(),
            repository_path: "fixture/project".into(),
            representation: if kind == Kind::PullRequest {
                Representation::GithubPullRequest
            } else {
                Representation::GithubIssue
            },
        },
    }
}
fn fixture(kind: Kind) -> Value {
    serde_json::from_str(if kind == Kind::PullRequest {
        include_str!("../../../../tests/fixtures/github_notification_discovery_pull.json")
    } else {
        include_str!("../../../../tests/fixtures/github_notification_discovery_issue.json")
    })
    .expect("valid synthetic JSON")
}
fn map(kind: Kind, value: Value) -> Result<NotificationSubjectDiscovery, ProviderError> {
    normalize_response(
        request(kind),
        &serde_json::to_vec(&value).unwrap(),
        Some("\"fixture-validator\"".into()),
        Some(240),
    )
}
fn verified(discovery: NotificationSubjectDiscovery) -> (RemoteItem, DetailPage) {
    match discovery {
        NotificationSubjectDiscovery::Verified {
            subject,
            detail,
            endpoint_aliases,
        } => {
            assert!(endpoint_aliases.is_empty());
            (*subject, *detail)
        }
        NotificationSubjectDiscovery::Unresolved { .. } => panic!("expected verified fixture"),
        NotificationSubjectDiscovery::Failed { .. } => panic!("expected valid fixture"),
    }
}
fn unresolved(discovery: NotificationSubjectDiscovery, expected: Reason) {
    match discovery {
        NotificationSubjectDiscovery::Unresolved {
            reason,
            cooldown_seconds,
        } => {
            assert_eq!(reason, expected);
            assert_eq!(cooldown_seconds, Some(240));
        }
        NotificationSubjectDiscovery::Verified { .. } => {
            panic!("unverified identity must not mint")
        }
        NotificationSubjectDiscovery::Failed { .. } => panic!("expected finite fallback"),
    }
}
fn rejected(kind: Kind, value: Value) {
    let error = map(kind, value).expect_err("invalid response cannot create identity");
    assert_eq!(error.kind, ProviderErrorKind::InvalidResponse);
    assert!(!error.to_string().contains("fixture-secret"));
}

#[test]
fn pull_identity_and_detail_share_one_authoritative_response_and_exact_integer_ids() {
    let (subject, detail) = verified(map(Kind::PullRequest, fixture(Kind::PullRequest)).unwrap());
    assert_eq!(subject.id, "github:pull:9007199254740997");
    assert_eq!(subject.provider_id, "9007199254740997");
    assert_eq!(subject.number.as_deref(), Some("67"));
    assert_eq!(subject.account_id, "fixture-account");
    assert_eq!(
        subject.repository_id,
        Some(request(Kind::PullRequest).repository.id)
    );
    assert_eq!(subject.state, "merged");
    assert_eq!(subject.kind, RemoteItemKind::PullRequest);
    assert_eq!(
        detail.body.text,
        fixture(Kind::PullRequest)["body"]
            .as_str()
            .map(String::from)
    );
    assert_eq!(detail.source.source, PULL_SOURCE);
    assert_eq!(
        detail.source.provider_updated_at.as_deref(),
        Some("2026-10-03T12:00:00Z")
    );
    assert_eq!(detail.source.field_mask, vec![DetailField::Body]);
    let metadata = detail.metadata.unwrap();
    assert_eq!(
        metadata.values.author.unwrap().provider_id,
        "9007199254740999"
    );
    assert_eq!(
        metadata
            .values
            .base
            .unwrap()
            .repository
            .unwrap()
            .provider_id,
        "9007199254741021"
    );
    assert_eq!(detail.etag.as_deref(), Some("\"fixture-validator\""));
    assert_eq!(detail.cooldown_seconds, Some(240));
    assert!(!detail.not_modified);
    assert!(detail.next_cursor.is_none());
}

#[test]
fn issue_parent_requires_same_response_native_proof_and_keeps_null_body_known() {
    let (subject, detail) = verified(map(Kind::Issue, fixture(Kind::Issue)).unwrap());
    assert_eq!(subject.id, "github:issue:9007199254740997");
    assert_eq!(subject.kind, RemoteItemKind::Issue);
    assert_eq!(detail.body.state, DetailValueState::Known);
    assert!(detail.body.text.is_none());
    assert!(!subject.body_omitted);
    let mut native_parent = fixture(Kind::Issue);
    native_parent.as_object_mut().unwrap().remove("repository");
    native_parent["repository_url"] = json!("https://api.github.com/repositories/9007199254741021");
    verified(map(Kind::Issue, native_parent).unwrap());
    let mut named_only = fixture(Kind::Issue);
    named_only.as_object_mut().unwrap().remove("repository");
    unresolved(
        map(Kind::Issue, named_only).unwrap(),
        Reason::IdentityUnverified,
    );
}

#[test]
fn issue_pr_marker_never_mints_an_issue_identity_or_follows_a_second_endpoint() {
    for marker in [
        Value::Null,
        json!({"url":"https://untrusted.test/pull"}),
        json!(false),
    ] {
        let mut value = fixture(Kind::Issue);
        value["pull_request"] = marker;
        unresolved(
            map(Kind::Issue, value).unwrap(),
            Reason::RepresentationMismatch,
        );
    }
}

#[test]
fn conflicting_immutable_parent_proofs_are_rejected_even_when_one_proof_matches() {
    let mut pull = fixture(Kind::PullRequest);
    pull["base"]["repo"]["id"] = json!(9007199254741022_u64);
    rejected(Kind::PullRequest, pull);
    let mut issue = fixture(Kind::Issue);
    issue["repository_url"] = json!("https://api.github.com/repositories/9007199254741021");
    issue["repository"]["id"] = json!(9007199254741022_u64);
    rejected(Kind::Issue, issue);
    for parent in [
        Value::Null,
        json!({}),
        json!({"id": "9007199254741021"}),
        json!({"id":0}),
    ] {
        let mut value = fixture(Kind::Issue);
        value["repository"] = parent;
        rejected(Kind::Issue, value);
    }
}

#[test]
fn response_paths_and_number_cannot_cross_kind_parent_account_or_raw_url_boundaries() {
    for (field, value) in [
        ("url", "https://api.github.com/repos/other/project/pulls/67"),
        (
            "url",
            "https://api.github.com/repositories/9007199254741022/pulls/67",
        ),
        (
            "url",
            "https://api.github.com/repos/fixture/project/issues/67",
        ),
        (
            "url",
            "https://api.github.com/repos/fixture/project/pulls/68",
        ),
        (
            "url",
            "https://api.github.com/repos/fixture/temp/../project/pulls/67",
        ),
        (
            "url",
            "https://api.github.com/repos/fixture/project/pulls/%36%37",
        ),
        (
            "url",
            "https://fixture-secret@api.github.com/repos/fixture/project/pulls/67",
        ),
        (
            "url",
            "https://api.github.com/repos/fixture/project/pulls/67?token=fixture-secret",
        ),
        (
            "url",
            "https://api.github.com.evil.test/repos/fixture/project/pulls/67",
        ),
        ("html_url", "https://github.com/fixture/project/issues/67"),
        ("html_url", "https://github.com/other/project/pull/67"),
        (
            "html_url",
            "https://github.com/fixture/temp/../project/pull/67",
        ),
        (
            "html_url",
            "https://github.com/fixture/project/pull/67#fragment",
        ),
    ] {
        let mut response = fixture(Kind::PullRequest);
        response[field] = json!(value);
        rejected(Kind::PullRequest, response);
    }
    let mut issue = fixture(Kind::Issue);
    issue["repository_url"] = json!("https://api.github.com/repos/other/project");
    rejected(Kind::Issue, issue);
    for field in ["id", "number"] {
        for value in [json!(0), json!(-1), json!("67"), json!(1.5)] {
            let mut response = fixture(Kind::PullRequest);
            response[field] = value;
            rejected(Kind::PullRequest, response);
        }
    }
}

#[test]
fn body_and_optional_metadata_missingness_are_not_fabricated_or_truncated() {
    for (body, expected) in [
        (None, DetailValueState::Omitted),
        (Some(Value::Null), DetailValueState::Known),
        (Some(json!("")), DetailValueState::Known),
        (
            Some(json!("x".repeat(1_048_577))),
            DetailValueState::Oversized,
        ),
    ] {
        let mut response = fixture(Kind::Issue);
        response.as_object_mut().unwrap().remove("body");
        if let Some(body) = body {
            response["body"] = body;
        }
        response["labels"] = json!(vec!["fixture"; 101]);
        response["user"] = json!({"id": 9007199254740999_u64, "login": "x".repeat(256)});
        let (subject, detail) = verified(map(Kind::Issue, response).unwrap());
        assert_eq!(detail.body.state, expected);
        assert_eq!(subject.body_omitted, expected != DetailValueState::Known);
        let metadata = detail.metadata.unwrap();
        for field in [MetadataField::Labels, MetadataField::Author] {
            assert_eq!(
                metadata
                    .fields
                    .iter()
                    .find(|e| e.field == field)
                    .unwrap()
                    .state,
                DetailValueState::Oversized
            );
        }
        assert!(metadata.values.labels.is_empty());
        assert!(metadata.values.author.is_none());
    }
}

fn response(status: &str, headers: &str, body: &str) -> String {
    format!(
        "HTTP/1.1 {status}\r\n{headers}Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
}
fn server(
    responses: impl FnOnce(&str) -> Vec<String>,
) -> (GithubProvider, thread::JoinHandle<Vec<String>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let base = format!("http://{}/", listener.local_addr().unwrap());
    let responses = responses(&base);
    let worker = thread::spawn(move || {
        let mut requests = vec![];
        for response in responses {
            let deadline = Instant::now() + Duration::from_secs(3);
            let mut stream = loop {
                match listener.accept() {
                    Ok((stream, _)) => break stream,
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        assert!(Instant::now() < deadline, "fixture request deadline");
                        thread::sleep(Duration::from_millis(2));
                    }
                    Err(_) => panic!("fixture listener error"),
                }
            };
            stream
                .set_read_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            let mut bytes = vec![];
            while !bytes.windows(4).any(|window| window == b"\r\n\r\n") {
                let mut buffer = [0; 4096];
                let count = stream.read(&mut buffer).unwrap();
                assert!(count > 0 && bytes.len() + count <= 16384);
                bytes.extend_from_slice(&buffer[..count]);
            }
            requests.push(String::from_utf8(bytes).unwrap());
            stream.write_all(response.as_bytes()).unwrap();
        }
        requests
    });
    (
        GithubProvider::for_test_base(reqwest::Url::parse(&base).unwrap()),
        worker,
    )
}
fn token() -> SecretToken {
    SecretToken::new("fixture-not-a-personal-token".into()).unwrap()
}

#[tokio::test]
async fn documented_named_get_has_no_conditionals_and_returns_saved_data_with_provider_cooldown() {
    let body = fixture(Kind::PullRequest).to_string();
    let (provider, worker) = server(|_| {
        vec![response(
            "200 OK",
            "ETag: \"fixture\"\r\nX-RateLimit-Remaining: 0\r\n",
            &body,
        )]
    });
    let (_, detail) = verified(
        provider
            .request_notification_subject_discovery(&token(), request(Kind::PullRequest))
            .await
            .unwrap(),
    );
    assert_eq!(detail.cooldown_seconds, Some(60));
    assert_eq!(detail.etag.as_deref(), Some("\"fixture\""));
    let requests = worker.join().unwrap();
    assert_eq!(requests.len(), 1);
    let raw = &requests[0];
    assert!(raw.starts_with("GET /repos/fixture/project/pulls/67 HTTP/1.1\r\n"));
    let headers = raw.to_ascii_lowercase();
    assert!(headers.contains("x-github-api-version: 2026-03-10"));
    assert!(!headers.contains("if-none-match"));
    assert!(!headers.contains("if-modified-since"));
    assert!(!raw.contains("777"));
}

#[tokio::test]
async fn unresolved_success_keeps_provider_cooldown_and_does_not_follow_identity_urls() {
    let mut value = fixture(Kind::Issue);
    value.as_object_mut().unwrap().remove("repository");
    let body = value.to_string();
    let (provider, worker) =
        server(|_| vec![response("200 OK", "X-RateLimit-Remaining: 0\r\n", &body)]);
    match provider
        .request_notification_subject_discovery(&token(), request(Kind::Issue))
        .await
        .unwrap()
    {
        NotificationSubjectDiscovery::Unresolved {
            reason,
            cooldown_seconds,
        } => {
            assert_eq!(reason, Reason::IdentityUnverified);
            assert_eq!(cooldown_seconds, Some(60));
        }
        _ => panic!("named URL cannot prove native parent"),
    }
    assert_eq!(worker.join().unwrap().len(), 1);
}

#[tokio::test]
async fn malformed_success_keeps_invalid_response_and_observed_provider_quota() {
    let (provider, worker) = server(|_| {
        vec![response(
            "200 OK",
            "X-RateLimit-Remaining: 0\r\n",
            "{malformed fixture-secret",
        )]
    });
    match provider
        .request_notification_subject_discovery(&token(), request(Kind::PullRequest))
        .await
        .unwrap()
    {
        NotificationSubjectDiscovery::Failed {
            error,
            cooldown_seconds,
        } => {
            assert_eq!(error.kind, ProviderErrorKind::InvalidResponse);
            assert_eq!(cooldown_seconds, Some(60));
            assert!(!error.to_string().contains("fixture-secret"));
        }
        _ => panic!("invalid response cannot mint identity or discard observed quota"),
    }
    assert_eq!(worker.join().unwrap().len(), 1);
}

#[tokio::test]
async fn unsupported_or_malformed_native_binding_dispatches_zero_http() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let provider = GithubProvider::for_test_base(
        reqwest::Url::parse(&format!("http://{}/", listener.local_addr().unwrap())).unwrap(),
    );
    for change in 0..9 {
        let mut invalid = request(Kind::PullRequest);
        match change {
            0 => invalid.repository.account_id = "other-account".into(),
            1 => invalid.instance_id = ProviderInstance::public(ProviderKind::Gitlab).id,
            2 => invalid.selector.repository_provider_id = "9007199254741022".into(),
            3 => invalid.selector.repository_path = "fixture/other".into(),
            4 => invalid.selector.number = "067".into(),
            5 => invalid.selector.representation = Representation::GithubIssue,
            6 => invalid.repository.full_name = "fixture/project/extra".into(),
            7 => invalid.account.state = AccountState::Disconnected,
            _ => invalid.account.notifications_supported = false,
        }
        tokio::time::timeout(
            Duration::from_millis(500),
            provider.request_notification_subject_discovery(&token(), invalid),
        )
        .await
        .expect("request binding must fail locally")
        .expect_err("invalid binding must not dispatch");
        assert_eq!(
            listener.accept().unwrap_err().kind(),
            std::io::ErrorKind::WouldBlock
        );
    }
}

#[tokio::test]
async fn unsolicited_304_or_pagination_never_bootstraps_identity_or_empty_detail() {
    for paginated in [false, true] {
        let (provider, worker) = server(|base| {
            vec![if paginated {
                response(
                    "200 OK",
                    &format!(
                        "Link: <{base}repos/fixture/project/pulls/67?page=2>; rel=\"next\"\r\n"
                    ),
                    &fixture(Kind::PullRequest).to_string(),
                )
            } else {
                response("304 Not Modified", "ETag: \"fixture\"\r\n", "")
            }]
        });
        let result = provider
            .request_notification_subject_discovery(&token(), request(Kind::PullRequest))
            .await;
        match result {
            Ok(NotificationSubjectDiscovery::Failed { error, .. }) | Err(error) => {
                assert_eq!(error.kind, ProviderErrorKind::InvalidResponse);
            }
            _ => panic!("point response cannot be unobserved or paginated"),
        }
        assert_eq!(worker.join().unwrap().len(), 1);
    }
}

#[tokio::test]
async fn redirects_cannot_request_native_undocumented_paths_or_other_parents_origins() {
    for location in [
        "/repositories/9007199254741021/pulls/67",
        "/repos/fixture/other/pulls/67",
        "https://untrusted.example.test/repos/fixture/project/pulls/67",
        "https://fixture-secret@api.github.com/repos/fixture/project/pulls/67",
        "/repos/fixture/project/pulls/67?token=fixture-secret",
        "/repos/fixture/project/pulls/67#fragment",
        "/repos/fixture/temp/../project/pulls/67",
        "/repos/fixture/%70roject/pulls/67",
        "/repos\\fixture\\project\\pulls\\67",
    ] {
        let (provider, worker) = server(|_| {
            vec![response(
                "301 Moved Permanently",
                &format!("Location: {location}\r\n"),
                "",
            )]
        });
        let error = provider
            .request_notification_subject_discovery(&token(), request(Kind::PullRequest))
            .await
            .expect_err("redirect cannot widen point authority");
        assert_eq!(error.kind, ProviderErrorKind::InvalidResponse);
        assert_eq!(worker.join().unwrap().len(), 1);
    }
}

#[tokio::test]
async fn same_operation_redirect_is_accepted_but_never_exceeds_transport_redirect_bound() {
    let body = fixture(Kind::PullRequest).to_string();
    let (provider, worker) = server(|_| {
        vec![
            response(
                "301 Moved Permanently",
                "Location: /repos/fixture/project/pulls/67\r\n",
                "",
            ),
            response("200 OK", "", &body),
        ]
    });
    verified(
        provider
            .request_notification_subject_discovery(&token(), request(Kind::PullRequest))
            .await
            .unwrap(),
    );
    let requests = worker.join().unwrap();
    assert_eq!(requests.len(), 2);
    assert!(
        requests
            .iter()
            .all(|request| request.starts_with("GET /repos/fixture/project/pulls/67 HTTP/1.1\r\n"))
    );
    let (provider, worker) = server(|_| {
        (0..4)
            .map(|_| {
                response(
                    "301 Moved Permanently",
                    "Location: /repos/fixture/project/pulls/67\r\n",
                    "",
                )
            })
            .collect()
    });
    let error = provider
        .request_notification_subject_discovery(&token(), request(Kind::PullRequest))
        .await
        .expect_err("redirect traversal must terminate");
    assert_eq!(error.kind, ProviderErrorKind::InvalidResponse);
    assert_eq!(worker.join().unwrap().len(), 4);
}

#[tokio::test]
async fn provider_errors_keep_truthful_type_and_retry_without_body_or_token_diagnostics() {
    for (status, headers, expected, retry) in [
        (
            "401 Unauthorized",
            "",
            ProviderErrorKind::Authentication,
            None,
        ),
        ("403 Forbidden", "", ProviderErrorKind::Permission, None),
        ("404 Not Found", "", ProviderErrorKind::NotFound, None),
        ("410 Gone", "", ProviderErrorKind::NotFound, None),
        (
            "429 Too Many Requests",
            "Retry-After: 321\r\n",
            ProviderErrorKind::RateLimited,
            Some(321),
        ),
    ] {
        let (provider, worker) = server(|_| vec![response(status, headers, "fixture-secret")]);
        let error = provider
            .request_notification_subject_discovery(&token(), request(Kind::Issue))
            .await
            .expect_err("provider failure must be typed");
        assert_eq!(error.kind, expected);
        assert_eq!(error.retry_after_seconds, retry);
        assert!(!error.to_string().contains("fixture-secret"));
        assert!(!error.to_string().contains("fixture-not-a-personal-token"));
        assert_eq!(worker.join().unwrap().len(), 1);
    }
}

#[tokio::test]
async fn point_http_response_bound_fails_before_a_partial_body_can_mint_identity() {
    let (provider, worker) = server(|_| {
        vec!["HTTP/1.1 200 OK\r\nContent-Length: 4194305\r\nx-ratelimit-remaining: 0\r\nConnection: close\r\n\r\n".into()]
    });
    let error = provider
        .request_notification_subject_discovery(&token(), request(Kind::PullRequest))
        .await
        .expect_err("oversized HTTP representation cannot create identity");
    assert_eq!(error.kind, ProviderErrorKind::InvalidResponse);
    assert_eq!(error.account_cooldown_seconds, Some(60));
    assert_eq!(worker.join().unwrap().len(), 1);
}

#[tokio::test]
async fn offline_named_point_fetch_is_typed_without_fabricating_a_missing_subject() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let provider = GithubProvider::for_test_base(
        reqwest::Url::parse(&format!("http://{}/", listener.local_addr().unwrap())).unwrap(),
    );
    drop(listener);
    let error = provider
        .request_notification_subject_discovery(&token(), request(Kind::Issue))
        .await
        .expect_err("offline cannot resolve or imply subject deletion");
    assert_eq!(error.kind, ProviderErrorKind::Offline);
    assert!(error.retry_after_seconds.is_none());
}
