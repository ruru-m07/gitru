//! Finite GitLab commit collection fixtures; no provider account is used.
use super::{
    tests::{response, server},
    *,
};
use serde_json::{Value, json};

const PROJECT: u64 = 123;
const BASE: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

fn oid(index: u32) -> String {
    format!("{index:040x}")
}

fn account() -> RemoteAccount {
    RemoteAccount {
        id: "gitlab-account".into(),
        provider: ProviderKind::Gitlab,
        host: "gitlab.com".into(),
        actor_id: "9007199254740993".into(),
        login: "actor".into(),
        display_name: None,
        authorization_epoch: "1".into(),
        state: AccountState::Active,
        notifications_supported: false,
    }
}

fn request(head: String) -> PullCommitRequest {
    PullCommitRequest {
        account: account(),
        repository: RemoteRepository {
            id: format!("gitlab:repository:{PROJECT}"),
            account_id: "gitlab-account".into(),
            provider_id: PROJECT.to_string(),
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
            repository_id: Some(format!("gitlab:repository:{PROJECT}")),
            provider_id: "999".into(),
            kind: RemoteItemKind::PullRequest,
            number: Some("67".into()),
            title: "cached merge request".into(),
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

fn token() -> SecretToken {
    SecretToken::new("synthetic_gitlab_commit_token".into()).unwrap()
}

fn row(index: u32) -> Value {
    let commit_oid = oid(index);
    json!({
        "id":commit_oid,
        "short_id":&commit_oid[..12],
        "title":format!("commit {index}"),
        "message":format!("commit {index}\n\nbody"),
        "author_name":"Author Name",
        "author_email":"author@example.invalid",
        "authored_date":"2026-10-01T00:00:00Z",
        "committer_name":"Committer Name",
        "committer_email":"committer@example.invalid",
        "committed_date":"2026-10-01T00:01:00Z",
        "created_at":"2026-10-01T00:01:00Z",
        "parent_ids":[if index < 1000 { oid(index + 1) } else { BASE.into() }],
        "web_url":format!("https://gitlab.com/fork/project/-/commit/{commit_oid}"),
        "trailers":{},
        "future":{"ignored":true}
    })
}

fn rows(start: u32, count: u32) -> String {
    serde_json::to_string(&(start..start + count).map(row).collect::<Vec<_>>()).unwrap()
}

fn next(base: &str, page: u32) -> String {
    format!(
        "Link: <{base}projects/123/merge_requests/67/commits?per_page=100&page={page}>; rel=\"next\"\r\n"
    )
}

fn live_next(base: &str, page: u32) -> String {
    format!(
        "Link: <{base}projects/123/merge_requests/67/commits?id=123&merge_request_iid=67&page={page}&per_page=100>; rel=\"next\"\r\n"
    )
}

#[tokio::test]
async fn bounded_pages_follow_exact_monotonic_links_with_head_to_base_evidence() {
    let (provider, calls) = server(|base| {
        vec![
            response(200, &live_next(base, 2), &rows(1, 100)),
            response(200, &live_next(base, 3), &rows(101, 100)),
            response(200, "", &rows(201, 5)),
        ]
    });
    let mut input = request(oid(1));
    for (start, count) in [(0, 100), (100, 100), (200, 5)] {
        let page = provider
            .fetch_pull_commits(&token(), input.clone())
            .await
            .unwrap();
        assert_eq!(page.start_position, start);
        assert_eq!(page.commits.len(), count);
        assert_eq!(page.order, PullCommitProviderOrder::HeadToBase);
        assert_eq!(page.source.source, commits::SOURCE);
        assert_eq!(page.commits[0].oid, oid(start + 1));
        assert_eq!(page.commits[0].author.name, "Author Name");
        assert!(page.commits[0].author.provider.is_none());
        assert_eq!(
            page.commits[0]
                .committer
                .as_ref()
                .map(|actor| actor.name.as_str()),
            Some("Committer Name")
        );
        if start < 200 {
            assert!(page.next_cursor.is_some());
            assert!(page.remote_has_more);
            input.cursor = page.next_cursor;
            input.start_position += count as u32;
        } else {
            assert!(page.next_cursor.is_none() && page.cap_reason.is_none());
            assert!(!page.remote_has_more);
        }
    }
    let calls = calls.join().unwrap();
    assert_eq!(calls.len(), 3);
    assert!(calls[0].starts_with(
        "GET /api/v4/projects/123/merge_requests/67/commits?per_page=100&page=1 HTTP/1.1"
    ));
    for (index, call) in calls.iter().enumerate().skip(1) {
        assert!(call.starts_with(&format!(
            "GET /api/v4/projects/123/merge_requests/67/commits?id=123&merge_request_iid=67&page={}&per_page=100 HTTP/1.1",
            index + 1
        )));
    }
    for call in calls {
        assert!(
            call.to_ascii_lowercase()
                .contains("private-token: synthetic_gitlab_commit_token")
        );
    }
}

#[tokio::test]
async fn five_hundred_of_a_larger_response_end_with_explicit_local_cap() {
    let (provider, calls) = server(|base| {
        (0..5)
            .map(|page| response(200, &next(base, page + 2), &rows(page * 100 + 1, 100)))
            .collect::<Vec<_>>()
    });
    let mut input = request(oid(1));
    for index in 0..5 {
        let page = provider
            .fetch_pull_commits(&token(), input.clone())
            .await
            .unwrap();
        assert_eq!(page.start_position, index * 100);
        assert_eq!(page.commits.len(), 100);
        if index < 4 {
            input.cursor = page.next_cursor;
            input.start_position += 100;
            assert!(input.cursor.is_some() && page.cap_reason.is_none());
        } else {
            assert!(page.next_cursor.is_none());
            assert_eq!(page.cap_reason, Some(PullCommitCapReason::LocalLimit));
            assert!(page.remote_has_more);
        }
    }
    assert_eq!(calls.join().unwrap().len(), 5);
}

#[tokio::test]
async fn cursor_drift_duplicates_and_malicious_links_are_rejected() {
    let (provider, calls) = server(|base| vec![response(200, &next(base, 2), &rows(1, 100))]);
    let mut input = request(oid(1));
    let first = provider
        .fetch_pull_commits(&token(), input.clone())
        .await
        .unwrap();
    input.cursor = first.next_cursor.clone();
    input.start_position = 99;
    assert_eq!(
        provider
            .fetch_pull_commits(&token(), input.clone())
            .await
            .unwrap_err()
            .kind,
        ProviderErrorKind::InvalidResponse
    );
    input.start_position = 100;
    input.context.head_oid = oid(2);
    assert_eq!(
        provider
            .fetch_pull_commits(&token(), input)
            .await
            .unwrap_err()
            .kind,
        ProviderErrorKind::InvalidResponse
    );
    assert_eq!(calls.join().unwrap().len(), 1);

    let duplicated = serde_json::to_string(&vec![row(1), row(1)]).unwrap();
    let (provider, calls) = server(|_| vec![response(200, "", &duplicated)]);
    assert_eq!(
        provider
            .fetch_pull_commits(&token(), request(oid(1)))
            .await
            .unwrap_err()
            .kind,
        ProviderErrorKind::InvalidResponse
    );
    assert_eq!(calls.join().unwrap().len(), 1);

    let body = rows(1, 100);
    let (provider, calls) = server(|base| {
        vec![response(
            200,
            &format!(
                "Link: <{base}projects/123/merge_requests/67/commits?per_page=100&page=3>; rel=\"next\"\r\n"
            ),
            &body,
        )]
    });
    assert_eq!(
        provider
            .fetch_pull_commits(&token(), request(oid(1)))
            .await
            .unwrap_err()
            .kind,
        ProviderErrorKind::InvalidResponse
    );
    assert_eq!(calls.join().unwrap().len(), 1);

    for query in [
        "id=124&merge_request_iid=67&page=2&per_page=100",
        "id=123&merge_request_iid=68&page=2&per_page=100",
        "id=123&id=123&merge_request_iid=67&page=2&per_page=100",
        "id=123&merge_request_iid=67&page=2&per_page=100&sudo=1",
    ] {
        let (provider, calls) = server(|base| {
            vec![response(
                200,
                &format!(
                    "Link: <{base}projects/123/merge_requests/67/commits?{query}>; rel=\"next\"\r\n"
                ),
                &body,
            )]
        });
        assert_eq!(
            provider
                .fetch_pull_commits(&token(), request(oid(1)))
                .await
                .unwrap_err()
                .kind,
            ProviderErrorKind::InvalidResponse,
            "query should fail closed: {query}"
        );
        assert_eq!(calls.join().unwrap().len(), 1);
    }
}
