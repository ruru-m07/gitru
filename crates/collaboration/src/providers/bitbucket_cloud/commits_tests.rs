//! Actual finite Bitbucket Cloud HTTP fixtures for pull-request commits.
use super::{
    tests::{REPO, collection, ok, request as feed_request, response, server, token},
    *,
};
use serde_json::{Value, json};

const BASE: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

fn oid(index: u32) -> String {
    format!("{index:040x}")
}

pub(super) fn request(head: String) -> PullCommitRequest {
    let account = feed_request().account;
    PullCommitRequest {
        repository: RemoteRepository {
            id: format!("bitbucket_cloud:repository:{REPO}"),
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
            repository_id: Some(format!("bitbucket_cloud:repository:{REPO}")),
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
            head_oid: Some(head.clone()),
            is_draft: None,
            reason: None,
            unread: None,
        },
        account,
        context: PullCommitContext {
            base_oid: BASE.into(),
            head_oid: head,
            source_repository_provider_id: REPO.into(),
            metadata_facet_revision: "metadata-1".into(),
        },
        cursor: None,
        start_position: 0,
    }
}

fn row(index: u32) -> Value {
    let commit_oid = oid(index);
    json!({
        "type":"commit",
        "hash":commit_oid,
        "date":"2026-10-01T00:01:00Z",
        "author":{"type":"author","raw":"Author Name <author@example.invalid>","user":null},
        "message":format!("commit {index}\n\nbody"),
        "summary":{"raw":format!("commit {index}"),"markup":"markdown","html":"ignored"},
        "parents":[{"type":"commit","hash":if index == 1 { BASE.into() } else { oid(index - 1) }}],
        "repository":{"type":"repository","uuid":format!("{{{REPO}}}")},
        "links":{"html":{"href":format!("https://bitbucket.org/team/project/commits/{commit_oid}")}},
        "future":{"ignored":true}
    })
}

fn path() -> String {
    format!("repositories/%7B%7D/%7B{REPO}%7D/pullrequests/67/commits")
}

#[tokio::test]
async fn commits_use_exact_uuid_route_and_normalize_chronological_facts() {
    let (provider, calls) = server(|_| vec![ok(collection(vec![row(2), row(1)], None))]);
    let page = provider
        .fetch_pull_commits(&token(), request(oid(2)))
        .await
        .unwrap();
    assert_eq!(page.order, PullCommitProviderOrder::HeadToBase);
    assert_eq!(page.start_position, 0);
    assert_eq!(page.source.source, commits::SOURCE);
    assert_eq!(page.commits.len(), 2);
    assert_eq!(page.commits[0].oid, oid(2));
    assert_eq!(page.commits[0].summary, "commit 2");
    assert_eq!(page.commits[0].author.name, "Author Name");
    assert!(page.commits[0].committer.is_none());
    assert_eq!(
        page.commits[0].committed_at.as_deref(),
        Some("2026-10-01T00:01:00Z")
    );
    assert!(page.commits[0].authored_at.is_none());
    assert!(page.next_cursor.is_none() && page.cap_reason.is_none());
    assert!(!page.remote_has_more);
    let calls = calls.join().unwrap();
    assert_eq!(calls.len(), 1);
    assert!(calls[0].starts_with(&format!("GET /2.0/{}?pagelen=50 HTTP/1.1", path())));
    assert!(
        calls[0]
            .to_ascii_lowercase()
            .contains("authorization: bearer synthetic_bitbucket_token")
    );
}

#[tokio::test]
async fn opaque_continuation_is_route_bound_and_positions_progress() {
    let (provider, calls) = server(|base| {
        vec![
            ok(collection(
                vec![row(3), row(2)],
                Some(format!(
                    "{base}{}?pagelen=50&cursor=opaque%2B%2F%3D",
                    path()
                )),
            )),
            ok(collection(vec![row(1)], None)),
        ]
    });
    let mut input = request(oid(3));
    let first = provider
        .fetch_pull_commits(&token(), input.clone())
        .await
        .unwrap();
    assert_eq!(first.start_position, 0);
    assert!(first.remote_has_more && first.next_cursor.is_some());
    input.cursor = first.next_cursor;
    input.start_position = 2;
    let second = provider.fetch_pull_commits(&token(), input).await.unwrap();
    assert_eq!(second.start_position, 2);
    assert_eq!(second.commits[0].oid, oid(1));
    assert!(second.next_cursor.is_none() && !second.remote_has_more);
    let calls = calls.join().unwrap();
    assert_eq!(calls.len(), 2);
    assert!(calls[1].contains("cursor=opaque%2B%2F%3D"));
}

#[tokio::test]
async fn five_hundred_rows_with_continuation_end_at_explicit_local_cap() {
    let (provider, calls) = server(|base| {
        (0_u32..10)
            .map(|page| {
                let high = 500 - page * 50;
                ok(collection(
                    (high - 49..=high).rev().map(row).collect(),
                    Some(format!(
                        "{base}{}?pagelen=50&cursor=opaque-{}",
                        path(),
                        page + 1
                    )),
                ))
            })
            .collect()
    });
    let mut input = request(oid(500));
    for index in 0..10 {
        let page = provider
            .fetch_pull_commits(&token(), input.clone())
            .await
            .unwrap();
        assert_eq!(page.start_position, index * 50);
        assert_eq!(page.commits.len(), 50);
        if index < 9 {
            input.cursor = page.next_cursor;
            input.start_position += 50;
            assert!(input.cursor.is_some() && page.cap_reason.is_none());
        } else {
            assert!(page.next_cursor.is_none());
            assert_eq!(page.cap_reason, Some(PullCommitCapReason::LocalLimit));
            assert!(page.remote_has_more);
        }
    }
    assert_eq!(calls.join().unwrap().len(), 10);
}

#[tokio::test]
async fn terminal_merged_prefix_before_captured_source_head_is_rejected_without_skipping() {
    let source_head = oid(2);
    let (provider, calls) = server(|_| {
        vec![response(
            200,
            "Retry-After: 120\r\n",
            &collection(vec![row(3), row(2), row(1)], None),
        )]
    });
    let error = provider
        .fetch_pull_commits(&token(), request(source_head))
        .await
        .unwrap_err();
    assert_eq!(error.kind, ProviderErrorKind::InvalidResponse);
    assert_eq!(error.account_cooldown_seconds, Some(120));
    assert_eq!(calls.join().unwrap().len(), 1);
}

#[tokio::test]
async fn duplicate_rows_foreign_next_and_cursor_position_drift_are_rejected() {
    let (provider, calls) = server(|_| vec![ok(collection(vec![row(1), row(1)], None))]);
    assert_eq!(
        provider
            .fetch_pull_commits(&token(), request(oid(1)))
            .await
            .unwrap_err()
            .kind,
        ProviderErrorKind::InvalidResponse
    );
    assert_eq!(calls.join().unwrap().len(), 1);

    let (provider, calls) = server(|base| {
        vec![ok(collection(
            vec![row(1)],
            Some(format!(
                "{base}repositories/%7B%7D/%7B{REPO}%7D/pullrequests/67/tasks?pagelen=50&cursor=foreign"
            )),
        ))]
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

    let (provider, calls) = server(|base| {
        vec![ok(collection(
            vec![row(1)],
            Some(format!("{base}{}?pagelen=50&cursor=next", path())),
        ))]
    });
    let first = provider
        .fetch_pull_commits(&token(), request(oid(1)))
        .await
        .unwrap();
    let mut drift = request(oid(1));
    drift.cursor = first.next_cursor;
    drift.start_position = 2;
    assert_eq!(
        provider
            .fetch_pull_commits(&token(), drift)
            .await
            .unwrap_err()
            .kind,
        ProviderErrorKind::InvalidResponse
    );
    assert_eq!(calls.join().unwrap().len(), 1);

    // Preserve quota evidence even when the received identity is malformed.
    let mut malformed = row(1);
    malformed["hash"] = "ABC".into();
    let (provider, calls) = server(|_| {
        vec![response(
            200,
            "Retry-After: 120\r\n",
            &collection(vec![malformed], None),
        )]
    });
    let error = provider
        .fetch_pull_commits(&token(), request(oid(1)))
        .await
        .unwrap_err();
    assert_eq!(error.kind, ProviderErrorKind::InvalidResponse);
    assert_eq!(error.account_cooldown_seconds, Some(120));
    assert_eq!(calls.join().unwrap().len(), 1);
}
