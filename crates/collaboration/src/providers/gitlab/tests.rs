use super::*;
use std::io::{Read, Write};

fn request() -> FeedRequest {
    FeedRequest {
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
        kind: FeedKind::Repositories,
        repository: None,
        cursor: None,
        etag: Some("ignored-validator".into()),
        last_modified: None,
    }
}
fn token() -> SecretToken {
    SecretToken::new("synthetic_gitlab_token".into()).unwrap()
}
pub(crate) fn project(id: u64, path: &str) -> serde_json::Value {
    serde_json::json!({"id":id,"path_with_namespace":path,"path":path.rsplit('/').next().unwrap(),"name":"Δ repository 🚀","web_url":format!("https://gitlab.com/{path}"),"description":null,"default_branch":null})
}
pub(crate) fn response(status: u16, headers: &str, body: &str) -> String {
    format!(
        "HTTP/1.1 {status} Fixture\r\nContent-Length: {}\r\nConnection: close\r\n{headers}\r\n{body}",
        body.len()
    )
}
pub(crate) fn server(
    responses: impl FnOnce(&str) -> Vec<String>,
) -> (GitlabProvider, std::thread::JoinHandle<Vec<String>>) {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}/api/v4/", listener.local_addr().unwrap());
    let responses = responses(&base);
    let task = std::thread::spawn(move || {
        listener.set_nonblocking(true).unwrap();
        let mut requests = vec![];
        for response in responses {
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
            let mut stream = loop {
                match listener.accept() {
                    Ok((stream, _)) => break stream,
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        assert!(
                            std::time::Instant::now() < deadline,
                            "expected synthetic request"
                        );
                        std::thread::sleep(std::time::Duration::from_millis(1));
                    }
                    Err(e) => panic!("fixture listener failed: {e}"),
                }
            };
            // Darwin inherits listener O_NONBLOCK on accept; exercise the real
            // transport with deterministic blocking reads on every platform.
            stream.set_nonblocking(false).unwrap();
            stream
                .set_read_timeout(Some(std::time::Duration::from_secs(2)))
                .unwrap();
            let mut bytes = vec![];
            loop {
                let mut buffer = [0; 1024];
                let n = stream.read(&mut buffer).unwrap();
                bytes.extend_from_slice(&buffer[..n]);
                if n == 0 || bytes.windows(4).any(|v| v == b"\r\n\r\n") {
                    break;
                }
            }
            requests.push(String::from_utf8(bytes).unwrap());
            stream.write_all(response.as_bytes()).unwrap();
        }
        requests
    });
    (
        GitlabProvider::fixture(reqwest::Url::parse(&base).unwrap()),
        task,
    )
}

#[tokio::test]
async fn operation_probes_accept_exact_actor_and_empty_memberships_without_coverage() {
    let (provider, task) = server(|_| {
        vec![
            response(
                200,
                "",
                r#"{"id":9007199254740993,"username":"mutable-login","name":"名前 🚀","state":"active"}"#,
            ),
            response(200, "", "[]"),
        ]
    });
    let verified = provider.probe(&token()).await.unwrap();
    assert_eq!(verified.actor_id, "9007199254740993");
    assert_eq!(verified.display_name.as_deref(), Some("名前 🚀"));
    assert!(!verified.notifications_supported);
    let calls = task.join().unwrap();
    assert_eq!(calls.len(), 2);
    assert!(calls[0].starts_with("GET /api/v4/user HTTP/1.1"));
    assert!(calls[1].starts_with(&format!(
        "GET /api/v4/projects?{} HTTP/1.1",
        transport::PROJECT_QUERY
    )));
    for call in calls {
        assert!(
            call.to_ascii_lowercase()
                .contains("private-token: synthetic_gitlab_token")
        );
        assert!(!call.to_ascii_lowercase().contains("authorization:"));
        assert!(!call.contains("X-GitHub"));
        assert!(!call.contains("if-none-match"));
    }
}

#[tokio::test]
async fn projects_keep_large_ids_subgroups_unicode_and_server_keyset_continuation() {
    let id = 9007199254740993;
    let (provider, task) = server(|base| {
        vec![
            response(
                200,
                &format!(
                    "Link: <{base}projects?{}&id_after={id}>; rel=\"next\"\r\n",
                    transport::PROJECT_QUERY
                ),
                &serde_json::to_string(&vec![project(id, "org/subgroup/project")]).unwrap(),
            ),
            response(
                200,
                "",
                &serde_json::to_string(&vec![project(id + 1, "other/project")]).unwrap(),
            ),
        ]
    });
    let first = provider.fetch_page(&token(), request()).await.unwrap();
    assert_eq!(first.repositories[0].provider_id, id.to_string());
    assert_eq!(first.repositories[0].full_name, "org/subgroup/project");
    assert_eq!(first.repositories[0].name, "Δ repository 🚀");
    assert_eq!(first.repositories[0].description, None);
    assert_eq!(first.repositories[0].default_branch, None);
    assert!(first.etag.is_none());
    let mut next = request();
    next.cursor = first.next_cursor;
    let second = provider.fetch_page(&token(), next).await.unwrap();
    assert!(second.next_cursor.is_none());
    assert_eq!(second.repositories[0].provider_id, (id + 1).to_string());
    let calls = task.join().unwrap();
    assert!(calls[1].contains(&format!("id_after={id}")));
    assert!(!calls[1].to_ascii_lowercase().contains("if-none-match"));
}

#[test]
fn implemented_read_profile_never_infers_future_permissions() {
    let provider = GitlabProvider::new().unwrap();
    let profile = provider.profile(&request().account);
    assert_eq!(profile.inbox_semantics, InboxSemantics::None);
    for facet in super::super::FACETS {
        assert_eq!(
            profile.facet(facet).state,
            if matches!(
                facet,
                ResourceFacet::Repositories
                    | ResourceFacet::PullRequests
                    | ResourceFacet::Issues
                    | ResourceFacet::PullDetails
                    | ResourceFacet::IssueDetails
                    | ResourceFacet::PullCommits
                    | ResourceFacet::PullFiles
            ) {
                CapabilityState::Supported
            } else {
                CapabilityState::Unsupported
            }
        );
    }
    assert_eq!(
        profile.facet(ResourceFacet::Inbox).reason,
        Some(CapabilityReason::ProviderSemantics)
    );
}

#[tokio::test]
async fn unsupported_feed_detail_and_foreign_installation_make_zero_requests() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let provider = GitlabProvider::fixture(
        reqwest::Url::parse(&format!(
            "http://{}/api/v4/",
            listener.local_addr().unwrap()
        ))
        .unwrap(),
    );
    let mut input = request();
    input.kind = FeedKind::Notifications;
    assert_eq!(
        provider.fetch_page(&token(), input).await.unwrap_err().kind,
        ProviderErrorKind::Unsupported
    );
    let mut input = request();
    input.account.host = "gitlab.company.invalid".into();
    assert_eq!(
        provider.fetch_page(&token(), input).await.unwrap_err().kind,
        ProviderErrorKind::Unsupported
    );
    let detail=DetailRequest {
        account:request().account,
        repository:RemoteRepository {id:"gitlab:repository:1".into(),account_id:"gitlab-account".into(),provider_id:"1".into(),full_name:"a/b".into(),name:"b".into(),web_url:"https://gitlab.com/a/b".into(),description:None,default_branch:None,selected:true},
        subject:serde_json::from_value(serde_json::json!({"id":"gitlab:issue:2","account_id":"gitlab-account","provider_id":"2","kind":"issue","title":"future detail","body_omitted":false,"state":"open","updated_at":"2026-10-03T00:00:00Z"})).unwrap(),
        facet:crate::DetailFacet::Comments,cursor:None,etag:None,source:None,
    };
    assert_eq!(
        provider
            .fetch_detail(&token(), detail)
            .await
            .unwrap_err()
            .kind,
        ProviderErrorKind::Unsupported
    );
    assert_eq!(
        provider.notification_subject_support(
            &request().account,
            crate::NotificationSubjectKind::Issue
        ),
        CapabilityState::Unsupported
    );
    assert_eq!(
        listener.accept().unwrap_err().kind(),
        std::io::ErrorKind::WouldBlock
    );
}

#[test]
fn continuation_rejects_host_paths_duplicate_filters_sudo_and_normalization() {
    let provider = GitlabProvider::new().unwrap();
    let good = format!(
        "https://gitlab.com/api/v4/projects?{}&id_after=9007199254740993",
        transport::PROJECT_QUERY
    );
    assert!(provider.http.continuation(&good).is_ok());
    for bad in [
        good.replace("gitlab.com", "evil.invalid"),
        good.replace("https:", "http:"),
        good.replace("gitlab.com/", "gitlab.com:8443/"),
        good.replace("gitlab.com/", "user@gitlab.com/"),
        good.replace("/projects?", "/user?"),
        format!("{good}&sudo=actor"),
        format!("{good}&private_token=secret"),
        format!("{good}&membership=false"),
        format!("{good}&id_after=42"),
        format!("{good}#fragment"),
        good.replace("9007199254740993", "01"),
        good.replace("9007199254740993", "0"),
        good.replace("9007199254740993", "18446744073709551616"),
        good.replace("api/v4", "api/../api/v4"),
        good.replace("projects", "%70rojects"),
        good.replace("projects", "projects/"),
    ] {
        assert!(
            provider.http.continuation(&bad).is_err(),
            "untrusted continuation accepted"
        );
    }
}

#[tokio::test]
async fn hostile_or_malformed_next_links_fail_without_claiming_completion() {
    for suffix in ["&sudo=actor", "&membership=false", "&id_after=02"] {
        let (provider, task) = server(|base| {
            vec![response(
                200,
                &format!(
                    "Link: <{base}projects?{}&id_after=1{suffix}>; rel=\"next\"\r\n",
                    transport::PROJECT_QUERY
                ),
                &serde_json::to_string(&vec![project(1, "a/b")]).unwrap(),
            )]
        });
        assert_eq!(
            provider
                .fetch_page(&token(), request())
                .await
                .unwrap_err()
                .kind,
            ProviderErrorKind::InvalidResponse
        );
        assert_eq!(task.join().unwrap().len(), 1);
    }
    for link in [
        "broken; rel=\"next\"",
        "<https://gitlab.com/api/v4/projects>; rel=\"next",
        "<https://gitlab.com/api/v4/projects>; rel=\"next\", <https://gitlab.com/api/v4/projects>; rel=\"next\"",
    ] {
        let (provider, task) = server(|_| vec![response(200, &format!("Link: {link}\r\n"), "[]")]);
        assert_eq!(
            provider
                .fetch_page(&token(), request())
                .await
                .unwrap_err()
                .kind,
            ProviderErrorKind::InvalidResponse
        );
        task.join().unwrap();
    }
}

#[tokio::test]
async fn unsafe_redirects_never_forward_the_token_or_change_operation() {
    for path in [
        "/api/v4/user",
        "/api/v4/../v4/projects",
        "/api/v4/%70rojects",
        "https://evil.invalid/api/v4/projects",
        "//evil.invalid/api/v4/projects",
    ] {
        let (provider, task) =
            server(|_| vec![response(302, &format!("Location: {path}\r\n"), "")]);
        assert_eq!(
            provider
                .fetch_page(&token(), request())
                .await
                .unwrap_err()
                .kind,
            ProviderErrorKind::InvalidResponse
        );
        assert_eq!(task.join().unwrap().len(), 1);
    }
}

#[tokio::test]
async fn exact_redirects_are_bounded_and_quota_prevents_another_request() {
    let (provider, task) = server(|base| {
        (0..4)
            .map(|_| {
                response(
                    302,
                    &format!("Location: {base}projects?{}\r\n", transport::PROJECT_QUERY),
                    "",
                )
            })
            .collect()
    });
    assert_eq!(
        provider
            .fetch_page(&token(), request())
            .await
            .unwrap_err()
            .kind,
        ProviderErrorKind::InvalidResponse
    );
    assert_eq!(task.join().unwrap().len(), 4);
    let (provider, task) = server(|base| {
        vec![response(
            302,
            &format!(
                "Location: {base}projects?{}\r\nRateLimit-Remaining: 0\r\n",
                transport::PROJECT_QUERY
            ),
            "",
        )]
    });
    let error = provider.fetch_page(&token(), request()).await.unwrap_err();
    assert_eq!(error.kind, ProviderErrorKind::RateLimited);
    assert_eq!(error.account_cooldown_seconds, Some(60));
    assert_eq!(task.join().unwrap().len(), 1);
}

#[tokio::test]
async fn auth_permission_quota_and_unavailable_stay_distinct_and_sanitized() {
    for (status, kind, headers) in [
        (401, ProviderErrorKind::Authentication, ""),
        (403, ProviderErrorKind::Permission, ""),
        (429, ProviderErrorKind::RateLimited, ""),
        (429, ProviderErrorKind::RateLimited, "Retry-After: 120\r\n"),
        (503, ProviderErrorKind::Unavailable, ""),
    ] {
        let (provider, task) = server(|_| {
            vec![response(
                status,
                headers,
                "synthetic_gitlab_token raw private error",
            )]
        });
        let error = provider.fetch_page(&token(), request()).await.unwrap_err();
        assert_eq!(error.kind, kind);
        assert!(!error.to_string().contains("synthetic_gitlab_token"));
        assert!(!format!("{error:?}").contains("raw private"));
        if status == 429 {
            assert_eq!(
                error.retry_after_seconds,
                Some(if headers.is_empty() { 60 } else { 120 })
            );
        }
        task.join().unwrap();
    }
}

#[tokio::test]
async fn long_observed_server_cooldowns_are_preserved_without_an_earlier_retry() {
    let (provider, task) = server(|_| vec![response(429, "Retry-After: 172800\r\n", "")]);
    let error = provider.fetch_page(&token(), request()).await.unwrap_err();
    assert_eq!(error.kind, ProviderErrorKind::RateLimited);
    assert_eq!(error.retry_after_seconds, Some(172800));
    assert_eq!(error.account_cooldown_seconds, Some(172800));
    task.join().unwrap();

    let reset = chrono::Utc::now().timestamp() as u64 + 172800;
    let (provider, task) = server(|_| {
        vec![response(
            200,
            &format!("RateLimit-Remaining: 0\r\nRateLimit-Reset: {reset}\r\n"),
            "[]",
        )]
    });
    let page = provider.fetch_page(&token(), request()).await.unwrap();
    assert!(page.cooldown_seconds.is_some_and(|seconds| seconds > 86400));
    task.join().unwrap();

    let (provider, task) =
        server(|_| vec![response(429, "Retry-After: 18446744073709551616\r\n", "")]);
    let error = provider.fetch_page(&token(), request()).await.unwrap_err();
    assert_eq!(error.retry_after_seconds, Some(u64::MAX));
    assert_eq!(error.account_cooldown_seconds, Some(u64::MAX));
    task.join().unwrap();
}

#[tokio::test]
async fn unavailable_retry_after_is_not_discarded() {
    let (provider, task) = server(|_| {
        vec![response(
            503,
            "Retry-After: 172800\r\n",
            "private maintenance body",
        )]
    });
    let error = provider.fetch_page(&token(), request()).await.unwrap_err();
    assert_eq!(error.kind, ProviderErrorKind::Unavailable);
    assert_eq!(error.retry_after_seconds, Some(172800));
    assert!(!error.to_string().contains("private"));
    task.join().unwrap();
}

#[tokio::test]
async fn redirect_retry_after_is_not_followed_early() {
    let (provider, task) = server(|base| {
        vec![response(
            302,
            &format!(
                "Retry-After: 120\r\nLocation: {base}projects?{}\r\n",
                transport::PROJECT_QUERY
            ),
            "",
        )]
    });
    let error = provider.fetch_page(&token(), request()).await.unwrap_err();
    assert_eq!(error.kind, ProviderErrorKind::Unavailable);
    assert_eq!(error.retry_after_seconds, Some(120));
    assert_eq!(
        task.join().unwrap().len(),
        1,
        "delayed redirect is left to scheduling"
    );
}

#[tokio::test]
async fn successful_quota_survives_invalid_json_links_and_body_bounds() {
    for body in ["not-json", "{}", r#"[{"id":0}]"#] {
        let (provider, task) = server(|_| vec![response(200, "RateLimit-Remaining: 0\r\n", body)]);
        let error = provider.fetch_page(&token(), request()).await.unwrap_err();
        assert_eq!(error.kind, ProviderErrorKind::InvalidResponse);
        assert_eq!(error.account_cooldown_seconds, Some(60));
        task.join().unwrap();
    }
    let (provider, task) = server(|_| {
        vec!["HTTP/1.1 200 OK\r\nContent-Length: 4194305\r\nRateLimit-Remaining: 0\r\nConnection: close\r\n\r\n".into()]
    });
    let error = provider.fetch_page(&token(), request()).await.unwrap_err();
    assert_eq!(error.kind, ProviderErrorKind::InvalidResponse);
    assert_eq!(error.account_cooldown_seconds, Some(60));
    task.join().unwrap();
}

#[tokio::test]
async fn actor_and_membership_probes_fail_closed_before_success() {
    for actor in [
        r#"{"id":0,"username":"actor"}"#,
        r#"{"id":1,"username":"actor","state":"blocked"}"#,
        r#"{"id":1.5,"username":"actor"}"#,
        r#"{"id":1,"username":""}"#,
    ] {
        let (provider, task) = server(|_| vec![response(200, "", actor)]);
        assert!(provider.probe(&token()).await.is_err());
        assert_eq!(task.join().unwrap().len(), 1);
    }
    for status in [401, 403, 429] {
        let (provider, task) = server(|_| {
            vec![
                response(200, "", r#"{"id":1,"username":"actor","state":"active"}"#),
                response(status, "", "private"),
            ]
        });
        assert!(provider.probe(&token()).await.is_err());
        assert_eq!(task.join().unwrap().len(), 2);
    }
    let (provider, task) = server(|_| {
        vec![response(
            200,
            "RateLimit-Remaining: 0\r\n",
            r#"{"id":1,"username":"actor"}"#,
        )]
    });
    assert_eq!(
        provider.probe(&token()).await.unwrap_err().kind,
        ProviderErrorKind::RateLimited
    );
    assert_eq!(task.join().unwrap().len(), 1);
    let (provider, task) = server(|_| {
        vec![
            response(200, "", r#"{"id":1,"username":"actor"}"#),
            response(200, "RateLimit-Remaining: 0\r\n", "[]"),
        ]
    });
    assert_eq!(
        provider.probe(&token()).await.unwrap().cooldown_seconds,
        Some(60)
    );
    task.join().unwrap();
}

#[tokio::test]
async fn invalid_repository_shapes_and_non_progressing_pages_are_not_coverage() {
    for body in [
        serde_json::json!([project(1, "a/b"), project(1, "a/b")]),
        serde_json::json!([project(2, "a/b"), project(1, "a/c")]),
        serde_json::json!((0..51).map(|n| project(n + 1, "a/b")).collect::<Vec<_>>()),
    ] {
        let (provider, task) = server(|_| vec![response(200, "", &body.to_string())]);
        assert_eq!(
            provider
                .fetch_page(&token(), request())
                .await
                .unwrap_err()
                .kind,
            ProviderErrorKind::InvalidResponse
        );
        task.join().unwrap();
    }
    for path in ["a/../b", "a/b?token=secret", "a//b"] {
        let (provider, task) = server(|_| {
            vec![response(
                200,
                "",
                &serde_json::json!([project(1, path)]).to_string(),
            )]
        });
        assert_eq!(
            provider
                .fetch_page(&token(), request())
                .await
                .unwrap_err()
                .kind,
            ProviderErrorKind::InvalidResponse
        );
        task.join().unwrap();
    }
    let (provider, task) = server(|base| {
        vec![response(
            200,
            &format!(
                "Link: <{base}projects?{}&id_after=2>; rel=\"next\"\r\n",
                transport::PROJECT_QUERY
            ),
            &serde_json::json!([project(1, "a/b")]).to_string(),
        )]
    });
    assert_eq!(
        provider
            .fetch_page(&token(), request())
            .await
            .unwrap_err()
            .kind,
        ProviderErrorKind::InvalidResponse
    );
    task.join().unwrap();
}
