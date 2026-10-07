use super::{
    commits_tests::{request as commit_request, response, server, token},
    *,
};
use crate::providers::pull_files::{continue_request, fixture_request};
use serde_json::{Value, json};

fn request() -> PullFileCollectionRequest {
    fixture_request(
        commit_request(format!("{:040x}", 1)),
        PullFileSourceStrategy::GithubPullFiles,
    )
}
fn row(index: u32) -> Value {
    json!({"sha":format!("{index:040x}"),"filename":format!("src/file{index}.rs"),"status":"modified","additions":2,"deletions":1,"changes":3,"patch":"@@ -1 +1 @@\n-old\n+new"})
}
fn parent() -> Value {
    json!({"id":999,"number":67,"changed_files":1,"base":{"sha":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","repo":{"id":123}},"head":{"sha":format!("{:040x}",1),"repo":{"id":456}}})
}
#[tokio::test]
async fn summaries_preserve_rename_pair_unknown_binary_and_omission() {
    let mut renamed = row(1);
    renamed["status"] = json!("renamed");
    renamed["previous_filename"] = json!("old/Δ.rs");
    let mut omitted = row(2);
    omitted.as_object_mut().unwrap().remove("patch");
    let (provider, calls) = server(|_| {
        vec![
            response(200, "", &json!([renamed, omitted])),
            response(200, "", &parent()),
        ]
    });
    let input = request();
    let page = provider
        .fetch_pull_files(&token(), input.clone())
        .await
        .unwrap();
    assert_eq!(page.files[0].identity.old_path.as_deref(), Some("old/Δ.rs"));
    assert_eq!(page.files[1].diff_hint, PullFileDiffHint::Omitted);
    assert_eq!(page.files[1].binary, PullFileFlag::Unknown);
    assert!(page.next_cursor.is_none() && page.cap.is_none());
    let validation = provider
        .validate_pull_file_range(&token(), input)
        .await
        .unwrap();
    assert_eq!(validation.expected_file_count, Some(1));
    let calls = calls.join().unwrap();
    assert!(calls[0].starts_with("GET /repositories/123/pulls/67/files?per_page=100 "));
    assert!(calls[1].starts_with("GET /repositories/123/pulls/67 "));
    assert!(
        calls
            .iter()
            .all(|s| !s.to_ascii_lowercase().contains("if-none-match"))
    );
}
#[tokio::test]
async fn full_three_thousand_rows_always_publish_provider_cap() {
    let (provider, calls) = server(|base| {
        (1..=30).map(|page| {
        let headers=if page<30 {format!("Link: <{base}repositories/123/pulls/67/files?per_page=100&page={}>; rel=\"next\"\r\n",page+1)} else {String::new()};
        response(200,&headers,&json!(((page-1)*100..page*100).map(row).collect::<Vec<_>>()))
    }).collect()
    });
    let mut input = request();
    for index in 0..30 {
        let page = provider
            .fetch_pull_files(&token(), input.clone())
            .await
            .unwrap();
        assert_eq!(page.start_position, index * 100);
        if index < 29 {
            continue_request(&mut input, &page)
        } else {
            assert_eq!(
                page.cap.unwrap().reason,
                PullFileCapReason::ProviderFileLimit
            );
            assert_eq!(page.cap.unwrap().remote_has_more, PullFileFlag::Unknown);
            assert!(page.next_cursor.is_none());
        }
    }
    assert_eq!(calls.join().unwrap().len(), 30);
}
#[tokio::test]
async fn cursor_fences_and_hostile_continuations_fail_before_another_http() {
    let (provider, calls) = server(|base| {
        vec![response(
            200,
            &format!(
                "Link: <{base}repositories/123/pulls/67/files?per_page=100&page=2>; rel=\"next\"\r\n"
            ),
            &json!([row(1)]),
        )]
    });
    let mut input = request();
    let page = provider
        .fetch_pull_files(&token(), input.clone())
        .await
        .unwrap();
    continue_request(&mut input, &page);
    for mutate in 0..5 {
        let mut changed = input.clone();
        match mutate {
            0 => {
                changed.authorization_view = "2".into();
                changed.lease.authorization_view = "2".into();
            }
            1 => {
                changed.account.authorization_epoch = "2".into();
                changed.lease.authorization_epoch = "2".into();
            }
            2 => {
                changed.binding.context.head_oid = format!("{:040x}", 2);
                changed.lease.binding = changed.binding.clone();
            }
            3 => changed.lease.generation = uuid::Uuid::new_v4().to_string(),
            _ => changed.account.actor_id = "2".into(),
        }
        assert!(provider.fetch_pull_files(&token(), changed).await.is_err());
    }
    assert_eq!(calls.join().unwrap().len(), 1);
    for next in [
        "https://evil.invalid/files?per_page=100&page=2",
        "http://127.0.0.1/other?per_page=100&page=2",
    ] {
        let (provider, calls) = server(|_| {
            vec![response(
                200,
                &format!("Link: <{next}>; rel=\"next\"\r\n"),
                &json!([row(1)]),
            )]
        });
        assert!(
            provider
                .fetch_pull_files(&token(), request())
                .await
                .is_err()
        );
        assert_eq!(calls.join().unwrap().len(), 1);
    }
}
#[tokio::test]
async fn malformed_paths_duplicates_and_range_drift_keep_quota() {
    let mut traversal = row(1);
    traversal["filename"] = json!("../secret");
    for body in [
        json!([traversal]),
        json!([row(1), row(1)]),
        json!({"files":[]}),
    ] {
        let (provider, calls) =
            server(|_| vec![response(200, "X-RateLimit-Remaining: 0\r\n", &body)]);
        let error = provider
            .fetch_pull_files(&token(), request())
            .await
            .unwrap_err();
        assert_eq!(error.kind, ProviderErrorKind::InvalidResponse);
        assert!(error.account_cooldown_seconds.is_some());
        calls.join().unwrap();
    }
    for field in ["base", "head"] {
        let mut body = parent();
        body[field]["sha"] = json!(format!("{:040x}", 42));
        let (provider, calls) =
            server(|_| vec![response(200, "X-RateLimit-Remaining: 0\r\n", &body)]);
        let error = provider
            .validate_pull_file_range(&token(), request())
            .await
            .unwrap_err();
        assert_eq!(error.kind, ProviderErrorKind::InvalidResponse);
        assert!(error.account_cooldown_seconds.is_some());
        calls.join().unwrap();
    }
}
#[tokio::test]
async fn collection_only_dispatches_one_request_even_when_quota_is_exhausted() {
    let (provider, calls) =
        server(|_| vec![response(200, "X-RateLimit-Remaining: 0\r\n", &json!([]))]);
    let page = provider
        .fetch_pull_files(&token(), request())
        .await
        .unwrap();
    assert!(page.files.is_empty() && page.cooldown_seconds.is_some());
    assert_eq!(calls.join().unwrap().len(), 1);
}
