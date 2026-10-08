use super::super::tests::{response, server};
use super::*;
use serde_json::json;
const PATH: &str = "projects/123/issues/67/notes";
const QUERY: &str = "per_page=50&page=1&sort=asc&order_by=updated_at";
fn request(kind: RemoteItemKind) -> DetailRequest {
    let prefix = if kind == RemoteItemKind::PullRequest {
        "pull"
    } else {
        "issue"
    };
    DetailRequest {
        account: RemoteAccount {
            id: "a".into(),
            provider: ProviderKind::Gitlab,
            host: "gitlab.com".into(),
            actor_id: "1".into(),
            login: "actor".into(),
            display_name: None,
            authorization_epoch: "1".into(),
            state: AccountState::Active,
            notifications_supported: false,
        },
        repository: RemoteRepository {
            id: "gitlab:repository:123".into(),
            account_id: "a".into(),
            provider_id: "123".into(),
            full_name: "owner/project".into(),
            name: "project".into(),
            web_url: "https://gitlab.com/owner/project".into(),
            description: None,
            default_branch: None,
            selected: true,
        },
        subject: RemoteItem {
            id: format!("gitlab:{prefix}:999"),
            account_id: "a".into(),
            repository_id: Some("gitlab:repository:123".into()),
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

fn row(id: u64, kind: RemoteItemKind) -> Value {
    json!({"id":id,"project_id":123,"noteable_id":999,"noteable_iid":67,"noteable_type":if kind==RemoteItemKind::Issue{"Issue"}else{"MergeRequest"},"system":false,"type":null,"resolvable":false,"author":{"id":8,"username":"author","email":"never stored"},"body":"body Δ <script>","created_at":"2026-10-01T00:00:00Z","updated_at":"2026-10-08T01:00:00Z"})
}
fn notes(status: u16, headers: &str, rows: Value) -> String {
    response(status, headers, &rows.to_string())
}
#[tokio::test]
async fn gitlab_comments_issue_and_mr_bind_native_identity_without_parent_clock_or_pii() {
    for kind in [RemoteItemKind::Issue, RemoteItemKind::PullRequest] {
        let (provider, task) = server(|_| {
            vec![notes(
                200,
                "ETag: unused\r\n",
                json!([row(u64::MAX, kind.clone())]),
            )]
        });
        let mut r = request(kind.clone());
        r.etag = Some("stale-parent-validator".into());
        let p = provider.fetch_detail(&token(), r).await.unwrap();
        assert_eq!(p.reconciliation, DetailReconciliation::full_history());
        assert_eq!(p.entries[0].provider_id, u64::MAX.to_string());
        assert_eq!(
            p.entries[0].updated_at.as_deref(),
            Some("2026-10-08T01:00:00.000000000Z")
        );
        assert_eq!(p.entries[0].author.as_deref(), Some("author"));
        assert!(p.source.provider_updated_at.is_none());
        assert!(p.etag.is_none());
        assert!(
            !serde_json::to_string(&p.entries[0])
                .unwrap()
                .contains("never stored")
        );
        let calls = task.join().unwrap();
        let segment = if kind == RemoteItemKind::Issue {
            "issues"
        } else {
            "merge_requests"
        };
        assert!(calls[0].starts_with(&format!(
            "GET /api/v4/projects/123/{segment}/67/notes?{QUERY} "
        )));
        assert!(!calls[0].to_lowercase().contains("if-none-match"));
    }
}
#[tokio::test]
async fn gitlab_comments_unrepresented_rows_never_claim_empty_or_full_history() {
    for case in 0..15 {
        let mut value = row(1, RemoteItemKind::Issue);
        match case {
            0 => value["system"] = true.into(),
            1 => value["type"] = "DiffNote".into(),
            2 => value["resolvable"] = true.into(),
            3 => {
                value.as_object_mut().unwrap().remove("id");
            }
            4 => value["updated_at"] = "bad".into(),
            5 => value["created_at"] = "2027-01-01T00:00:00Z".into(),
            6 => value["body"] = Value::Null,
            7 => {
                value.as_object_mut().unwrap().remove("body");
            }
            8 => value["body"] = "x".repeat(65537).into(),
            9 => value["deleted"] = true.into(),
            10 => {
                value.as_object_mut().unwrap().remove("system");
            }
            11 => {
                value.as_object_mut().unwrap().remove("created_at");
            }
            12 => value["resolvable"] = "unknown".into(),
            13 => value["body"] = "invalid\0body".into(),
            _ => value["author"] = json!({"username":"unknown-id"}),
        }
        let (provider, task) = server(|_| vec![notes(200, "", json!([value]))]);
        let p = provider
            .fetch_detail(&token(), request(RemoteItemKind::Issue))
            .await
            .unwrap();
        assert_eq!(p.reconciliation, DetailReconciliation::default(), "{case}");
        if case != 7 && case != 8 {
            assert!(p.entries.is_empty(), "{case}");
        }
        task.join().unwrap();
    }
    let (provider, task) = server(|_| vec![notes(200, "", json!([]))]);
    let p = provider
        .fetch_detail(&token(), request(RemoteItemKind::Issue))
        .await
        .unwrap();
    assert_eq!(p.reconciliation, DetailReconciliation::full_history());
    task.join().unwrap();
}
#[tokio::test]
async fn gitlab_comments_identity_mismatch_duplicate_and_bounds_preserve_quota() {
    for case in 0..6 {
        let mut v = row(1, RemoteItemKind::Issue);
        let rows = match case {
            0 => {
                v["project_id"] = 124.into();
                json!([v])
            }
            1 => {
                v["noteable_id"] = 998.into();
                json!([v])
            }
            2 => {
                v["noteable_iid"] = 68.into();
                json!([v])
            }
            3 => {
                v["noteable_type"] = "MergeRequest".into();
                json!([v])
            }
            4 => json!([v.clone(), v]),
            _ => json!(vec![v; 51]),
        };
        let (provider, task) = server(|_| vec![notes(200, "Retry-After: 120\r\n", rows)]);
        let e = provider
            .fetch_detail(&token(), request(RemoteItemKind::Issue))
            .await
            .unwrap_err();
        assert_eq!(e.kind, ProviderErrorKind::InvalidResponse);
        assert_eq!(e.account_cooldown_seconds, Some(120));
        task.join().unwrap();
    }
}
#[tokio::test]
async fn gitlab_comments_strict_next_links_cannot_expand_authority_or_skip_pages() {
    for suffix in [
        "per_page=50&page=3&sort=asc&order_by=updated_at",
        "per_page=50&page=1&sort=asc&order_by=updated_at",
        "per_page=50&page=2&sort=asc&order_by=created_at",
        "per_page=50&page=2&sort=desc&order_by=updated_at",
        "per_page=50&page=2&sort=asc&order_by=updated_at&sudo=1",
        "per_page=50&page=2&sort=asc&order_by=updated_at&issue_iid=68",
        "per_page=50&page=2&sort=asc&order_by=updated_at&page=2",
    ] {
        let (provider, task) = server(|base| {
            vec![notes(
                200,
                &format!("Link: <{base}{PATH}?{suffix}>; rel=\"next\"\r\nRetry-After: 120\r\n"),
                json!([]),
            )]
        });
        let e = provider
            .fetch_detail(&token(), request(RemoteItemKind::Issue))
            .await
            .unwrap_err();
        assert_eq!(e.kind, ProviderErrorKind::InvalidResponse);
        assert_eq!(e.account_cooldown_seconds, Some(120));
        assert_eq!(task.join().unwrap().len(), 1);
    }
    for path in [
        "https://evil.invalid/api/v4/projects/123/issues/67/notes",
        "projects/124/issues/67/notes",
        "projects/123/merge_requests/67/notes",
        "projects/123/issues/68/notes",
        "projects/123/issues/67/%6eotes",
    ] {
        let (provider, task) = server(|base| {
            let url = if path.starts_with("https:") {
                path.into()
            } else {
                format!("{base}{path}")
            };
            vec![notes(
                200,
                &format!(
                    "Link: <{url}?per_page=50&page=2&sort=asc&order_by=updated_at>; rel=\"next\"\r\n"
                ),
                json!([]),
            )]
        });
        assert!(
            provider
                .fetch_detail(&token(), request(RemoteItemKind::Issue))
                .await
                .is_err()
        );
        assert_eq!(task.join().unwrap().len(), 1);
    }
}
#[tokio::test]
async fn gitlab_comments_cursor_round_trip_is_bound_and_twenty_page_cap_keeps_more_truthful() {
    let (provider, task) = server(|base| {
        (1..=20).map(|page|notes(200,&format!("Link: <{base}{PATH}?per_page=50&page={}&sort=asc&order_by=updated_at>; rel=\"next\"\r\n",page+1),json!([row(page,RemoteItemKind::Issue)]))).collect()
    });
    let mut r = request(RemoteItemKind::Issue);
    for page in 1..=20 {
        let p = provider.fetch_detail(&token(), r.clone()).await.unwrap();
        assert_eq!(p.reconciliation, DetailReconciliation::default());
        let raw = p.next_cursor.unwrap();
        let saved: Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(saved["pages"], page);
        if page == 1 {
            for field in ["actor", "account", "epoch", "repository", "subject"] {
                let mut v = saved.clone();
                v[field] = "foreign".into();
                let mut bad = r.clone();
                bad.cursor = Some(v.to_string());
                assert!(provider.fetch_detail(&token(), bad).await.is_err());
            }
        }
        r.cursor = Some(saved.to_string());
    }
    assert!(provider.fetch_detail(&token(), r.clone()).await.is_err());
    assert!(provider.fetch_detail(&token(), r).await.is_err());
    assert_eq!(task.join().unwrap().len(), 20);
}
#[tokio::test]
async fn gitlab_comments_status_and_cooldown_survive_empty_or_malformed_responses() {
    for (status, kind) in [
        (401, ProviderErrorKind::Authentication),
        (403, ProviderErrorKind::Permission),
        (429, ProviderErrorKind::RateLimited),
        (503, ProviderErrorKind::Unavailable),
        (304, ProviderErrorKind::InvalidResponse),
    ] {
        let (provider, task) = server(|_| {
            vec![notes(
                status,
                "Retry-After: 120\r\n",
                json!({"private":"never persist"}),
            )]
        });
        let e = provider
            .fetch_detail(&token(), request(RemoteItemKind::Issue))
            .await
            .unwrap_err();
        assert_eq!(e.kind, kind);
        assert_eq!(e.account_cooldown_seconds, Some(120));
        task.join().unwrap();
    }
}

async fn saved_store(dir: &std::path::Path) -> (crate::Store, DetailRequest) {
    use crate::*;
    let store = Store::open(dir.join("notes.db")).await.unwrap();
    let mut r = request(RemoteItemKind::Issue);
    r.subject.updated_at = "2026-10-07T00:00:00Z".into();
    r.account = store.upsert_account(r.account).await.unwrap();
    for (scope, repositories, items) in [
        (
            "repositories".to_string(),
            vec![r.repository.clone()],
            vec![],
        ),
        (
            format!("repo:{}:issue", r.repository.id),
            vec![],
            vec![r.subject.clone()],
        ),
    ] {
        let run_id = store
            .begin_sync(&r.account.id, &r.account.authorization_epoch, &scope)
            .await
            .unwrap();
        store
            .apply_page(PageCommit {
                account_id: r.account.id.clone(),
                authorization_epoch: r.account.authorization_epoch.clone(),
                scope,
                run_id,
                repositories,
                items,
                endpoint_aliases: vec![],
                next_cursor: None,
                etag: None,
                last_modified: None,
                not_modified: false,
                complete: true,
                observed_at: "2026-10-07T00:00:00Z".into(),
            })
            .await
            .unwrap();
    }
    (store, r)
}
async fn commit_page(
    store: &crate::Store,
    r: &DetailRequest,
    p: DetailPage,
) -> crate::DetailCommit {
    use crate::*;
    let lease = store
        .begin_detail(
            &r.account.id,
            &r.account.authorization_epoch,
            &r.subject.id,
            DetailFacet::Comments,
        )
        .await
        .unwrap();
    DetailCommit {
        account_id: r.account.id.clone(),
        authorization_epoch: r.account.authorization_epoch.clone(),
        authorization_view: lease.authorization_view,
        instance_id: lease.instance_id,
        subject_id: r.subject.id.clone(),
        facet: DetailFacet::Comments,
        run_id: lease.run_id,
        request_cursor: lease.next_cursor,
        reconciliation: p.reconciliation,
        metadata: None,
        subject_binding: Some(DetailSubjectBinding {
            repository_id: r.repository.id.clone(),
            repository_provider_id: r.repository.provider_id.clone(),
            provider_id: r.subject.provider_id.clone(),
            number: r.subject.number.clone(),
            kind: r.subject.kind.clone(),
            head_oid: r.subject.head_oid.clone(),
        }),
        check_context: None,
        review_context: None,
        body: p.body,
        entries: p.entries,
        source: p.source,
        next_cursor: p.next_cursor.clone(),
        etag: None,
        not_modified: false,
        whole_scope: r.cursor.is_none(),
        complete: p.next_cursor.is_none(),
        freshness_seconds: 180,
    }
}
fn query(r: &DetailRequest) -> crate::DetailQuery {
    crate::DetailQuery {
        account_id: r.account.id.clone(),
        subject_id: r.subject.id.clone(),
        facet: DetailFacet::Comments,
        cursor: None,
        limit: 100,
    }
}
#[tokio::test]
async fn gitlab_comments_sqlite_own_clock_edits_regressions_partial_and_empty_absence() {
    let dir = tempfile::tempdir().unwrap();
    let (store, r) = saved_store(dir.path()).await;
    let old = row(2, RemoteItemKind::Issue);
    let mut edited = old.clone();
    edited["body"] = "edited note".into();
    edited["updated_at"] = "2026-10-08T01:05:00Z".into();
    let mut omitted = edited.clone();
    omitted.as_object_mut().unwrap().remove("body");
    omitted["updated_at"] = "2026-10-08T01:06:00Z".into();
    let mut system = edited.clone();
    system["system"] = true.into();
    let (provider, task) = server(|_| {
        [
            json!([old.clone()]),
            json!([edited]),
            json!([old]),
            json!([omitted]),
            json!([system]),
            json!([]),
        ]
        .into_iter()
        .map(|rows| notes(200, "", rows))
        .collect()
    });
    for (index, want) in [
        "body Δ <script>",
        "edited note",
        "edited note",
        "edited note",
        "edited note",
    ]
    .into_iter()
    .enumerate()
    {
        let p = provider.fetch_detail(&token(), r.clone()).await.unwrap();
        let commit = commit_page(&store, &r, p).await;
        store.apply_detail(commit).await.unwrap();
        let saved = store.detail(query(&r)).await.unwrap();
        assert_eq!(saved.entries[0].body.text.as_deref(), Some(want), "{index}");
        if index >= 3 {
            assert_eq!(saved.evidence.coverage.state, crate::CoverageState::Partial);
        }
    }
    let p = provider.fetch_detail(&token(), r.clone()).await.unwrap();
    store
        .apply_detail(commit_page(&store, &r, p).await)
        .await
        .unwrap();
    assert!(store.detail(query(&r)).await.unwrap().entries.is_empty());
    assert_eq!(task.join().unwrap().len(), 6);
    store.close().await.unwrap();
}
#[tokio::test]
async fn gitlab_comments_sqlite_continuation_survives_restart_and_held_epoch_cannot_publish() {
    let dir = tempfile::tempdir().unwrap();
    let (store, mut r) = saved_store(dir.path()).await;
    let (provider, task) = server(|base| {
        vec![
            notes(
                200,
                &format!(
                    "Link: <{base}{PATH}?per_page=50&page=2&sort=asc&order_by=updated_at>; rel=\"next\"\r\n"
                ),
                json!([row(1, RemoteItemKind::Issue)]),
            ),
            notes(200, "", json!([row(2, RemoteItemKind::Issue)])),
            notes(200, "", json!([row(3, RemoteItemKind::Issue)])),
        ]
    });
    let p = provider.fetch_detail(&token(), r.clone()).await.unwrap();
    r.cursor = p.next_cursor.clone();
    let mut first = r.clone();
    first.cursor = None;
    store
        .apply_detail(commit_page(&store, &first, p).await)
        .await
        .unwrap();
    store.close().await.unwrap();
    let store = crate::Store::open(dir.path().join("notes.db"))
        .await
        .unwrap();
    let lease = store
        .begin_detail(
            &r.account.id,
            &r.account.authorization_epoch,
            &r.subject.id,
            DetailFacet::Comments,
        )
        .await
        .unwrap();
    assert_eq!(lease.next_cursor, r.cursor);
    let p = provider.fetch_detail(&token(), r.clone()).await.unwrap();
    store
        .apply_detail(commit_page(&store, &r, p).await)
        .await
        .unwrap();
    let saved = store.detail(query(&r)).await.unwrap();
    assert_eq!(saved.entries.len(), 2);
    assert_eq!(saved.evidence.coverage.state, crate::CoverageState::Partial);
    r.cursor = None;
    let p = provider.fetch_detail(&token(), r.clone()).await.unwrap();
    let held = commit_page(&store, &r, p).await;
    store.disconnect(&r.account.id).await.unwrap();
    assert!(store.apply_detail(held).await.is_err());
    assert!(store.detail(query(&r)).await.unwrap().entries.is_empty());
    assert_eq!(task.join().unwrap().len(), 3);
    store.close().await.unwrap();
}

#[tokio::test]
async fn gitlab_comments_successful_retry_after_is_account_evidence_without_primary_headers() {
    let (provider, task) = server(|_| vec![notes(200, "Retry-After: 120\r\n", json!([]))]);
    let p = provider
        .fetch_detail(&token(), request(RemoteItemKind::Issue))
        .await
        .unwrap();
    assert_eq!(p.cooldown_seconds, Some(120));
    assert_eq!(p.reconciliation, DetailReconciliation::full_history());
    task.join().unwrap();
}
