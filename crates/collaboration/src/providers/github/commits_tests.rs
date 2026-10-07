//! Finite HTTP fixtures for typed pull-request commit pages.
use super::*;
use serde_json::{Value, json};
use std::io::{Read, Write};

const BASE: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

fn oid(index: u32) -> String {
    format!("{index:040x}")
}

pub(super) fn request(head: String) -> PullCommitRequest {
    PullCommitRequest {
        account: RemoteAccount {
            id: "github-account".into(),
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
            account_id: "github-account".into(),
            provider_id: "123".into(),
            full_name: "owner/project".into(),
            name: "project".into(),
            web_url: "https://github.com/owner/project".into(),
            description: None,
            default_branch: None,
            selected: true,
        },
        subject: RemoteItem {
            id: "github:pull:999".into(),
            account_id: "github-account".into(),
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
            updated_at: "2026-10-01T00:00:00Z".into(),
            head_oid: Some(head.clone()),
            is_draft: None,
            reason: None,
            unread: None,
        },
        context: PullCommitContext {
            base_oid: BASE.into(),
            head_oid: head,
            source_repository_provider_id: "456".into(),
            metadata_facet_revision: "metadata-1".into(),
        },
        cursor: None,
        start_position: 0,
    }
}

pub(super) fn token() -> SecretToken {
    SecretToken::new("synthetic_commit_token".into()).unwrap()
}

fn row(index: u32) -> Value {
    let commit_oid = oid(index);
    let parents = if index == 1 {
        vec![json!({"sha":BASE})]
    } else {
        vec![json!({"sha":oid(index - 1)})]
    };
    json!({
        "sha": commit_oid,
        "commit": {
            "message": format!("summary {index}\n\nbody"),
            "author": {"name":"Authored Name","date":"2026-10-01T00:00:00Z"},
            "committer": {"name":"Committed Name","date":"2026-10-01T00:01:00Z"}
        },
        "author": {"id":9007199254740993_u64,"login":"author","html_url":"https://github.com/author"},
        "committer": null,
        "parents": parents,
        "html_url": format!("https://github.com/source/project/commit/{commit_oid}"),
        "future": {"ignored":true}
    })
}

pub(super) fn response(status: u16, headers: &str, body: &Value) -> String {
    let body = body.to_string();
    format!(
        "HTTP/1.1 {status} Fixture\r\nContent-Length: {}\r\nConnection: close\r\n{headers}\r\n{body}",
        body.len()
    )
}

pub(super) fn server(
    responses: impl FnOnce(&str) -> Vec<String>,
) -> (GithubProvider, std::thread::JoinHandle<Vec<String>>) {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}/", listener.local_addr().unwrap());
    let replies = responses(&base);
    let task = std::thread::spawn(move || {
        listener.set_nonblocking(true).unwrap();
        let mut calls = vec![];
        for reply in replies {
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
            let mut stream = loop {
                match listener.accept() {
                    Ok((stream, _)) => break stream,
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        assert!(
                            std::time::Instant::now() < deadline,
                            "expected fixture request"
                        );
                        std::thread::sleep(std::time::Duration::from_millis(1));
                    }
                    Err(error) => panic!("fixture listener failed: {error}"),
                }
            };
            stream.set_nonblocking(false).unwrap();
            stream
                .set_read_timeout(Some(std::time::Duration::from_secs(2)))
                .unwrap();
            let mut bytes = vec![];
            while !bytes.windows(4).any(|value| value == b"\r\n\r\n") {
                let mut chunk = [0; 1024];
                let count = stream.read(&mut chunk).unwrap();
                assert!(count > 0 && bytes.len() + count < 16 * 1024);
                bytes.extend_from_slice(&chunk[..count]);
            }
            calls.push(String::from_utf8(bytes).unwrap());
            stream.write_all(reply.as_bytes()).unwrap();
        }
        calls
    });
    (
        GithubProvider::for_test_base(reqwest::Url::parse(&base).unwrap()),
        task,
    )
}

#[tokio::test]
async fn pull_commits_use_immutable_route_and_normalize_base_to_head_facts() {
    let (provider, calls) = server(|_| vec![response(200, "", &json!([row(1), row(2)]))]);
    let expected = request(oid(2)).context;
    let page = provider
        .fetch_pull_commits(&token(), request(oid(2)))
        .await
        .unwrap();
    assert_eq!(page.context, expected);
    assert_eq!(page.order, PullCommitProviderOrder::BaseToHead);
    assert_eq!(page.start_position, 0);
    assert_eq!(page.source.source, commits::SOURCE);
    assert_eq!(page.commits.len(), 2);
    assert_eq!(page.commits[0].oid, oid(1));
    assert_eq!(page.commits[1].oid, oid(2));
    assert_eq!(page.commits[1].summary, "summary 2");
    assert_eq!(page.commits[0].parent_oids, vec![BASE]);
    assert_eq!(page.commits[0].author.name, "Authored Name");
    assert_eq!(
        page.commits[0]
            .author
            .provider
            .as_ref()
            .map(|actor| actor.provider_id.as_str()),
        Some("9007199254740993")
    );
    assert!(
        page.commits[0]
            .committer
            .as_ref()
            .is_some_and(|committer| committer.provider.is_none())
    );
    assert!(page.next_cursor.is_none() && page.cap_reason.is_none());
    assert!(!page.remote_has_more);
    let calls = calls.join().unwrap();
    assert_eq!(calls.len(), 1);
    assert!(calls[0].starts_with("GET /repositories/123/pulls/67/commits?per_page=50 HTTP/1.1"));
    assert!(
        calls[0]
            .to_ascii_lowercase()
            .contains("authorization: bearer synthetic_commit_token")
    );
}

#[tokio::test]
async fn exactly_two_hundred_fifty_commits_are_provider_capped() {
    let (provider, calls) = server(|base| {
        (0..5)
            .map(|page| {
                let start = page * 50 + 1;
                let rows: Vec<_> = (start..start + 50).map(row).collect();
                let link = if page < 4 {
                    format!(
                        "Link: <{base}repositories/123/pulls/67/commits?per_page=50&page={}>; rel=\"next\"\r\n",
                        page + 2
                    )
                } else {
                    String::new()
                };
                response(200, &link, &json!(rows))
            })
            .collect()
    });
    let mut input = request(oid(250));
    for page_index in 0..5 {
        let page = provider
            .fetch_pull_commits(&token(), input.clone())
            .await
            .unwrap();
        assert_eq!(page.start_position, page_index * 50);
        assert_eq!(page.commits.len(), 50);
        if page_index < 4 {
            assert!(page.next_cursor.is_some() && page.cap_reason.is_none());
            assert!(page.remote_has_more);
            input.cursor = page.next_cursor;
            input.start_position += 50;
        } else {
            assert!(page.next_cursor.is_none());
            assert_eq!(page.cap_reason, Some(PullCommitCapReason::ProviderLimit));
            // The documented 250-row provider boundary cannot prove remote
            // exhaustion even when GitHub omits a next Link.
            assert!(page.remote_has_more);
        }
    }
    assert_eq!(calls.join().unwrap().len(), 5);
}

#[tokio::test]
async fn duplicate_rows_and_context_or_position_cursor_drift_are_rejected() {
    let (provider, calls) = server(|base| {
        let link = format!(
            "Link: <{base}repositories/123/pulls/67/commits?per_page=50&page=2>; rel=\"next\"\r\n"
        );
        vec![response(
            200,
            &link,
            &json!((1..=50).map(row).collect::<Vec<_>>()),
        )]
    });
    let first = provider
        .fetch_pull_commits(&token(), request(oid(100)))
        .await
        .unwrap();
    let mut drift = request(oid(100));
    drift.cursor = first.next_cursor;
    drift.start_position = 49;
    assert_eq!(
        provider
            .fetch_pull_commits(&token(), drift)
            .await
            .unwrap_err()
            .kind,
        ProviderErrorKind::InvalidResponse
    );
    assert_eq!(calls.join().unwrap().len(), 1);

    let (provider, calls) = server(|_| vec![response(200, "", &json!([row(1), row(1)]))]);
    assert_eq!(
        provider
            .fetch_pull_commits(&token(), request(oid(2)))
            .await
            .unwrap_err()
            .kind,
        ProviderErrorKind::InvalidResponse
    );
    assert_eq!(calls.join().unwrap().len(), 1);
}

#[test]
fn pull_commits_capability_is_explicitly_scoped_to_github_com() {
    let provider = GithubProvider::new().unwrap();
    let mut account = request(oid(1)).account;
    assert_eq!(
        provider
            .profile(&account)
            .facet(ResourceFacet::PullCommits)
            .state,
        CapabilityState::Supported
    );

    account.host = "enterprise.invalid".into();
    assert_eq!(
        provider
            .profile(&account)
            .facet(ResourceFacet::PullCommits)
            .state,
        CapabilityState::Unsupported
    );
}
