//! Synthetic actual HTTP qualification; no provider accounts or native credentials.
use super::*;
use std::{
    io::{Read, Write},
    time::{Duration, Instant},
};

pub(super) const ACTOR: &str = "11111111-1111-4111-8111-111111111111";
pub(super) const W1: &str = "22222222-2222-4222-8222-222222222222";
pub(super) const W2: &str = "33333333-3333-4333-8333-333333333333";
pub(super) const REPO: &str = "44444444-4444-4444-8444-444444444444";

pub(super) fn token() -> SecretToken {
    SecretToken::new("synthetic_bitbucket_token".into()).unwrap()
}

pub(super) fn request() -> FeedRequest {
    FeedRequest {
        account: RemoteAccount {
            id: "fixture-account".into(),
            provider: ProviderKind::BitbucketCloud,
            host: "bitbucket.org".into(),
            actor_id: ACTOR.into(),
            login: "mutable".into(),
            display_name: None,
            authorization_epoch: "1".into(),
            state: AccountState::Active,
            notifications_supported: false,
        },
        kind: FeedKind::Repositories,
        repository: None,
        cursor: None,
        etag: Some("not-authority".into()),
        last_modified: Some("not-authority".into()),
    }
}

pub(super) fn actor() -> serde_json::Value {
    serde_json::json!({"type":"user","uuid":format!("{{{ACTOR}}}"),"nickname":"same-nickname","display_name":"Δ actor","account_status":"active"})
}

pub(super) fn workspace(id: &str, slug: &str) -> serde_json::Value {
    serde_json::json!({"type":"workspace_access","workspace":{"type":"workspace_base","uuid":format!("{{{id}}}"),"slug":slug}})
}

pub(super) fn repository(workspace: &str, full_name: &str) -> serde_json::Value {
    serde_json::json!({"type":"repository","uuid":format!("{{{REPO}}}"),"scm":"git","name":"Δ repository","full_name":full_name,"workspace":{"type":"workspace","uuid":format!("{{{workspace}}}"),"slug":full_name.split('/').next().unwrap()},"links":{"html":{"href":format!("https://bitbucket.org/{full_name}/")},"clone":[{"name":"https","href":format!("https://username@bitbucket.org/{full_name}.git")},{"name":"ssh","href":format!("git@bitbucket.org:{full_name}.git")}]},"description":"first line\nsecond line","mainbranch":null})
}

pub(super) fn collection(
    values: Vec<serde_json::Value>,
    next: Option<String>,
) -> serde_json::Value {
    serde_json::json!({"values":values,"next":next})
}

pub(super) fn response(status: u16, headers: &str, body: &serde_json::Value) -> String {
    let body = body.to_string();
    format!(
        "HTTP/1.1 {status} Fixture\r\nContent-Length: {}\r\nConnection: close\r\n{headers}\r\n{body}",
        body.len()
    )
}

pub(super) fn ok(body: serde_json::Value) -> String {
    response(200, "", &body)
}

pub(super) fn server(
    responses: impl FnOnce(&str) -> Vec<String>,
) -> (BitbucketCloudProvider, std::thread::JoinHandle<Vec<String>>) {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}/2.0/", listener.local_addr().unwrap());
    let replies = responses(&base);
    let task = std::thread::spawn(move || {
        listener.set_nonblocking(true).unwrap();
        let mut requests = vec![];
        for response in replies {
            let deadline = Instant::now() + Duration::from_secs(3);
            let mut stream = loop {
                match listener.accept() {
                    Ok((stream, _)) => break stream,
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        assert!(
                            Instant::now() < deadline,
                            "expected bounded fixture request"
                        );
                        std::thread::sleep(Duration::from_millis(1));
                    }
                    Err(error) => panic!("fixture failed: {error}"),
                }
            };
            stream.set_nonblocking(false).unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            let mut bytes = vec![];
            loop {
                let mut chunk = [0; 1024];
                let length = stream.read(&mut chunk).unwrap();
                assert!(bytes.len() + length <= 16 * 1024, "bounded request");
                bytes.extend_from_slice(&chunk[..length]);
                if length == 0 || bytes.windows(4).any(|v| v == b"\r\n\r\n") {
                    break;
                }
            }
            requests.push(String::from_utf8(bytes).unwrap());
            stream.write_all(response.as_bytes()).unwrap();
        }
        requests
    });
    (
        BitbucketCloudProvider::fixture(reqwest::Url::parse(&base).unwrap()),
        task,
    )
}

#[tokio::test]
async fn actual_probe_uses_sensitive_bearer_uuid_routes_without_requiring_pull_grant() {
    let (provider, calls) = server(|_| {
        vec![
            ok(actor()),
            ok(collection(vec![workspace(W1, "team")], None)),
            ok(collection(vec![repository(W1, "team/project")], None)),
        ]
    });
    let account = provider.probe(&token()).await.unwrap();
    assert_eq!(account.actor_id, ACTOR);
    assert_eq!(account.login, "same-nickname");
    assert!(!account.notifications_supported);
    let profile = provider.profile(&request().account);
    assert_eq!(profile.inbox_semantics, InboxSemantics::None);
    for facet in profile.facets {
        assert_eq!(
            facet.state == CapabilityState::Supported,
            matches!(
                facet.facet,
                ResourceFacet::Repositories
                    | ResourceFacet::PullRequests
                    | ResourceFacet::PullDetails
                    | ResourceFacet::Participants
            )
        );
        if matches!(facet.facet, ResourceFacet::Issues | ResourceFacet::Inbox) {
            assert_eq!(facet.reason, Some(CapabilityReason::ProviderSemantics));
        }
    }
    let calls = calls.join().unwrap();
    assert_eq!(calls.len(), 3);
    assert!(calls[0].starts_with("GET /2.0/user HTTP/1.1"));
    assert!(calls[1].starts_with("GET /2.0/user/workspaces?pagelen=10 HTTP/1.1"));
    assert!(calls[2].starts_with(&format!(
        "GET /2.0/repositories/%7B{W1}%7D?role=member&pagelen=50 HTTP/1.1"
    )));
    for call in calls {
        let call = call.to_ascii_lowercase();
        assert!(call.contains("authorization: bearer synthetic_bitbucket_token"));
        assert!(
            !call.contains("basic ")
                && !call.contains("private-token")
                && !call.contains("if-none-match")
                && !call.contains("if-modified-since")
        );
    }
}

#[tokio::test]
async fn actual_probe_rejects_unproven_identity_types_states_and_uuid_aliases() {
    for patch in [
        serde_json::json!({"type":"team"}),
        serde_json::json!({"account_status":"closed"}),
        serde_json::json!({"uuid":"nickname"}),
        serde_json::json!({"uuid":"urn:uuid:11111111-1111-4111-8111-111111111111"}),
    ] {
        let mut user = actor();
        for (key, value) in patch.as_object().unwrap() {
            user[key] = value.clone();
        }
        let (provider, calls) = server(|_| vec![ok(user)]);
        let failure = provider.probe_with_backoff(&token()).await.unwrap_err();
        assert_eq!(failure.verified_actor_id, None);
        assert!(!failure.error.to_string().contains("synthetic"));
        assert_eq!(calls.join().unwrap().len(), 1);
    }
}

#[tokio::test]
async fn actual_empty_workspace_probe_succeeds_without_inventing_repository_access() {
    let (provider, calls) = server(|_| vec![ok(actor()), ok(collection(vec![], None))]);
    assert_eq!(provider.probe(&token()).await.unwrap().actor_id, ACTOR);
    assert_eq!(calls.join().unwrap().len(), 2);
}

#[tokio::test]
async fn actual_probe_accepts_fixed_ssh_uri_metadata_without_giving_it_http_authority() {
    let mut repo = repository(W1, "team/project");
    repo["links"]["clone"][1]["href"] = "ssh://git@bitbucket.org/team/project.git".into();
    let (provider, calls) = server(|_| {
        vec![
            ok(actor()),
            ok(collection(vec![workspace(W1, "team")], None)),
            ok(collection(vec![repo], None)),
        ]
    });
    assert_eq!(provider.probe(&token()).await.unwrap().actor_id, ACTOR);
    assert_eq!(calls.join().unwrap().len(), 3);
}

#[tokio::test]
async fn actual_multi_workspace_opaque_paging_keeps_empty_intermediate_pages_partial() {
    let (provider, calls) = server(|base| {
        vec![
            ok(collection(
                vec![workspace(W1, "team"), workspace(W2, "other")],
                Some(format!(
                    "{base}user/workspaces?pagelen=10&page=outer%2Bopaque"
                )),
            )),
            ok(collection(
                vec![],
                Some(format!(
                    "{base}repositories/%7B{W1}%7D?role=member&pagelen=50&cursor=opaque%2B%2F%3D"
                )),
            )),
            ok(collection(vec![repository(W1, "team/project")], None)),
            ok(collection(vec![], None)),
            ok(collection(
                vec![],
                Some(format!("{base}user/workspaces?pagelen=10&page=last")),
            )),
            ok(collection(vec![], None)),
        ]
    });
    let mut req = request();
    let mut rows = vec![];
    for index in 0..6 {
        let page = provider.fetch_page(&token(), req.clone()).await.unwrap();
        assert_eq!(page.next_cursor.is_none(), index == 5);
        assert!(page.etag.is_none() && page.last_modified.is_none() && !page.not_modified);
        rows.extend(page.repositories);
        req.cursor = page.next_cursor;
    }
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].provider_id, REPO);
    assert_eq!(rows[0].account_id, "fixture-account");
    assert_eq!(rows[0].web_url, "https://bitbucket.org/team/project");
    let calls = calls.join().unwrap();
    assert_eq!(calls.len(), 6);
    assert!(calls[2].contains("cursor=opaque%2B%2F%3D"));
    assert!(calls[4].contains("page=outer%2Bopaque"));
}

#[tokio::test]
async fn actual_continuations_cannot_change_origin_filters_paths_or_credentials() {
    for suffix in [
        "foreign",
        "userinfo",
        "password",
        "path",
        "role",
        "size",
        "duplicate",
        "secret",
        "dot",
        "long",
    ] {
        let (provider, calls) = server(|base| {
            let next = match suffix {
                "foreign" => "https://evil.example/2.0/user/workspaces?pagelen=10&page=2".into(),
                "userinfo" => {
                    base.replacen("http://", "http://actor@", 1)
                        + "user/workspaces?pagelen=10&page=2"
                }
                "password" => {
                    base.replacen("http://", "http://actor:secret@", 1)
                        + "user/workspaces?pagelen=10&page=2"
                }
                "path" => format!("{base}user?pagelen=10&page=2"),
                "size" => format!("{base}user/workspaces?pagelen=50&page=2"),
                "duplicate" => format!("{base}user/workspaces?pagelen=10&page=2&page=3"),
                "secret" => format!("{base}user/workspaces?pagelen=10&page=2&access_token=secret"),
                "dot" => format!("{base}user/../user/workspaces?pagelen=10&page=2"),
                "long" => format!("{base}user/workspaces?pagelen=10&page={}", "x".repeat(513)),
                _ => format!("{base}user/workspaces?pagelen=10&role=owner&page=2"),
            };
            vec![ok(collection(vec![], Some(next)))]
        });
        assert_eq!(
            provider
                .fetch_page(&token(), request())
                .await
                .unwrap_err()
                .kind,
            ProviderErrorKind::InvalidResponse,
            "{suffix}"
        );
        assert_eq!(calls.join().unwrap().len(), 1);
    }
}

#[tokio::test]
async fn actual_repository_continuation_cannot_drop_membership_or_cross_workspace() {
    for suffix in [
        "role=owner&pagelen=50&page=2",
        "pagelen=50&page=2",
        "role=member&pagelen=10&page=2",
    ] {
        let (provider, calls) = server(|base| {
            vec![
                ok(collection(vec![workspace(W1, "team")], None)),
                ok(collection(
                    vec![],
                    Some(format!("{base}repositories/%7B{W1}%7D?{suffix}")),
                )),
            ]
        });
        let mut req = request();
        req.cursor = provider
            .fetch_page(&token(), req.clone())
            .await
            .unwrap()
            .next_cursor;
        assert_eq!(
            provider.fetch_page(&token(), req).await.unwrap_err().kind,
            ProviderErrorKind::InvalidResponse
        );
        assert_eq!(calls.join().unwrap().len(), 2);
    }
    let (provider, calls) = server(|base| {
        vec![
            ok(collection(vec![workspace(W1, "team")], None)),
            ok(collection(
                vec![],
                Some(format!(
                    "{base}repositories/%7B{W2}%7D?role=member&pagelen=50&page=2"
                )),
            )),
        ]
    });
    let mut req = request();
    req.cursor = provider
        .fetch_page(&token(), req.clone())
        .await
        .unwrap()
        .next_cursor;
    assert_eq!(
        provider.fetch_page(&token(), req).await.unwrap_err().kind,
        ProviderErrorKind::InvalidResponse
    );
    assert_eq!(calls.join().unwrap().len(), 2);
}

#[tokio::test]
async fn actual_cycle_and_query_reordering_are_rejected_before_following_old_page() {
    let (provider, calls) = server(|base| {
        vec![
            ok(collection(
                vec![],
                Some(format!("{base}user/workspaces?pagelen=10&page=A")),
            )),
            ok(collection(
                vec![],
                Some(format!("{base}user/workspaces?pagelen=10&page=B")),
            )),
            ok(collection(
                vec![],
                Some(format!("{base}user/workspaces?page=A&pagelen=10")),
            )),
        ]
    });
    let mut req = request();
    for _ in 0..2 {
        req.cursor = provider
            .fetch_page(&token(), req.clone())
            .await
            .unwrap()
            .next_cursor;
    }
    assert_eq!(
        provider.fetch_page(&token(), req).await.unwrap_err().kind,
        ProviderErrorKind::InvalidResponse
    );
    assert_eq!(calls.join().unwrap().len(), 3);
}

#[tokio::test]
async fn actual_cursor_is_account_epoch_kind_stage_and_size_bound_before_http() {
    let (provider, calls) = server(|_| vec![ok(collection(vec![workspace(W1, "team")], None))]);
    let mut req = request();
    req.cursor = provider
        .fetch_page(&token(), req.clone())
        .await
        .unwrap()
        .next_cursor;
    assert_eq!(calls.join().unwrap().len(), 1); // Listener is now closed.
    let raw = req.cursor.clone().unwrap();
    for key in ["account", "epoch", "kind", "version", "pages"] {
        let mut cursor: serde_json::Value = serde_json::from_str(&raw).unwrap();
        cursor[key] = match key {
            "version" => 2.into(),
            "pages" => 0.into(),
            _ => "foreign".into(),
        };
        let mut invalid_req = req.clone();
        invalid_req.cursor = Some(cursor.to_string());
        assert_eq!(
            provider
                .fetch_page(&token(), invalid_req)
                .await
                .unwrap_err()
                .kind,
            ProviderErrorKind::InvalidResponse
        );
    }
    req.cursor = Some("x".repeat(4097));
    assert_eq!(
        provider.fetch_page(&token(), req).await.unwrap_err().kind,
        ProviderErrorKind::InvalidResponse
    );
}

#[tokio::test]
async fn actual_workspace_rows_are_wrapped_unique_uuid_scoped_and_capped() {
    for values in [
        vec![workspace(W1, "team"), workspace(W1, "team")],
        vec![workspace(W1, "team"); 11],
        vec![serde_json::json!({"type":"workspace","uuid":format!("{{{W1}}}"),"slug":"team"})],
    ] {
        let (provider, calls) = server(|_| vec![ok(collection(values, None))]);
        assert_eq!(
            provider
                .fetch_page(&token(), request())
                .await
                .unwrap_err()
                .kind,
            ProviderErrorKind::InvalidResponse
        );
        assert_eq!(calls.join().unwrap().len(), 1);
    }
}

#[tokio::test]
async fn actual_repository_rows_preserve_uuid_identity_and_reject_untrusted_links() {
    for fault in [
        "password",
        "host",
        "ssh-password",
        "ssh-host",
        "ssh-port",
        "web-user",
        "workspace",
        "uuid",
        "duplicate",
        "cap",
    ] {
        let mut repo = repository(W1, "team/project");
        match fault {
            "password" => {
                repo["links"]["clone"][0]["href"] =
                    "https://actor:secret@bitbucket.org/team/project.git".into()
            }
            "host" => {
                repo["links"]["clone"][0]["href"] = "https://evil.example/team/project.git".into()
            }
            "ssh-password" => {
                repo["links"]["clone"][1]["href"] =
                    "ssh://git:secret@bitbucket.org/team/project.git".into()
            }
            "ssh-host" => {
                repo["links"]["clone"][1]["href"] = "ssh://git@evil.example/team/project.git".into()
            }
            "ssh-port" => {
                repo["links"]["clone"][1]["href"] =
                    "ssh://git@bitbucket.org:22/team/project.git".into()
            }
            "web-user" => {
                repo["links"]["html"]["href"] = "https://actor@bitbucket.org/team/project".into()
            }
            "workspace" => repo["workspace"]["uuid"] = format!("{{{W2}}}").into(),
            "uuid" => repo["uuid"] = "mutable-name".into(),
            _ => {}
        }
        let rows = vec![
            repo;
            if fault == "cap" {
                51
            } else if fault == "duplicate" {
                2
            } else {
                1
            }
        ];
        let (provider, calls) = server(|_| {
            vec![
                ok(collection(vec![workspace(W1, "team")], None)),
                ok(collection(rows, None)),
            ]
        });
        let mut req = request();
        req.cursor = provider
            .fetch_page(&token(), req.clone())
            .await
            .unwrap()
            .next_cursor;
        assert_eq!(
            provider.fetch_page(&token(), req).await.unwrap_err().kind,
            ProviderErrorKind::InvalidResponse,
            "{fault}"
        );
        assert_eq!(calls.join().unwrap().len(), 2);
    }
}

#[tokio::test]
async fn actual_rate_and_access_signals_are_redacted_and_capacity_is_not_remaining() {
    for (status, expected) in [
        (401, ProviderErrorKind::Authentication),
        (403, ProviderErrorKind::Permission),
        (404, ProviderErrorKind::NotFound),
        (429, ProviderErrorKind::RateLimited),
        (503, ProviderErrorKind::Unavailable),
    ] {
        let (provider, calls) = server(|_| {
            vec![response(
                status,
                "Retry-After: 172800\r\n",
                &serde_json::json!({"error":"sensitive body"}),
            )]
        });
        let error = provider.fetch_page(&token(), request()).await.unwrap_err();
        assert_eq!(error.kind, expected);
        assert_eq!(error.account_cooldown_seconds, Some(172800));
        assert!(!error.to_string().contains("sensitive"));
        assert_eq!(calls.join().unwrap().len(), 1);
    }
    let (provider, calls) = server(|_| {
        vec![response(
            200,
            "X-RateLimit-Limit: 0\r\nX-RateLimit-NearLimit: true\r\nX-RateLimit-Remaining: 0\r\n",
            &collection(vec![], None),
        )]
    });
    assert_eq!(
        provider
            .fetch_page(&token(), request())
            .await
            .unwrap()
            .cooldown_seconds,
        None
    );
    assert_eq!(calls.join().unwrap().len(), 1);
}

#[tokio::test]
async fn actual_overflow_and_successful_minimum_waits_preserve_native_quota() {
    let (provider, calls) = server(|_| {
        vec![response(
            429,
            "Retry-After: 184467440737095516160\r\n",
            &serde_json::json!({}),
        )]
    });
    assert_eq!(
        provider
            .fetch_page(&token(), request())
            .await
            .unwrap_err()
            .account_cooldown_seconds,
        Some(u64::MAX)
    );
    assert_eq!(calls.join().unwrap().len(), 1);
    let (provider, calls) = server(|_| vec![response(200, "Retry-After: 90\r\n", &actor())]);
    let failure = provider.probe_with_backoff(&token()).await.unwrap_err();
    assert_eq!(failure.verified_actor_id, Some(ACTOR.into()));
    assert_eq!(failure.error.kind, ProviderErrorKind::RateLimited);
    assert_eq!(calls.join().unwrap().len(), 1);
}

#[tokio::test]
async fn actual_invalid_probe_metadata_retains_quota_after_actor_proof() {
    for repository_probe in [false, true] {
        let (provider, calls) = server(|_| {
            let mut replies = vec![ok(actor())];
            if repository_probe {
                replies.push(ok(collection(vec![workspace(W1, "team")], None)));
            }
            replies.push(response(
                200,
                "Retry-After: 172800\r\n",
                &collection(
                    vec![serde_json::json!({"type":"untrusted","uuid":"invalid"})],
                    None,
                ),
            ));
            replies
        });
        let failure = provider.probe_with_backoff(&token()).await.unwrap_err();
        assert_eq!(failure.verified_actor_id, Some(ACTOR.into()));
        assert_eq!(failure.error.kind, ProviderErrorKind::InvalidResponse);
        assert_eq!(failure.error.account_cooldown_seconds, Some(172800));
        assert_eq!(
            calls.join().unwrap().len(),
            if repository_probe { 3 } else { 2 }
        );
    }
}

#[tokio::test]
async fn actual_bounded_presentation_failure_keeps_already_proven_actor_quota() {
    for (field, length) in [("nickname", 256), ("display_name", 1025)] {
        let mut user = actor();
        user[field] = "x".repeat(length).into();
        let (provider, calls) = server(|_| vec![response(200, "Retry-After: 120\r\n", &user)]);
        let failure = provider.probe_with_backoff(&token()).await.unwrap_err();
        assert_eq!(failure.error.kind, ProviderErrorKind::InvalidResponse);
        assert_eq!(failure.error.account_cooldown_seconds, Some(120));
        assert_eq!(failure.verified_actor_id, Some(ACTOR.into()));
        assert_eq!(calls.join().unwrap().len(), 1);
    }
}

#[tokio::test]
async fn actual_redirects_conditional_responses_and_body_bounds_do_not_expand_http() {
    for status in [302, 304] {
        let (provider, calls) = server(|base| {
            vec![response(
                status,
                &format!("Location: {base}user/workspaces?pagelen=10&page=2\r\n"),
                &serde_json::json!({}),
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
        assert_eq!(calls.join().unwrap().len(), 1);
    }
    let (provider, calls) = server(|_| {
        vec![format!(
            "HTTP/1.1 200 Fixture\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            4 * 1024 * 1024 + 1
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
    assert_eq!(calls.join().unwrap().len(), 1);
}

#[tokio::test]
async fn resource_features_remain_explicitly_unsupported_without_http() {
    let (provider, calls) = server(|_| vec![]);
    calls.join().unwrap();
    for kind in [FeedKind::Issues, FeedKind::Notifications] {
        let mut req = request();
        req.kind = kind;
        assert_eq!(
            provider.fetch_page(&token(), req).await.unwrap_err().kind,
            ProviderErrorKind::Unsupported
        );
    }
}
