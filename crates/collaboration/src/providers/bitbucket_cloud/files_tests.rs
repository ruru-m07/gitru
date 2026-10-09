use super::{
    tests::{REPO, response, server, token},
    *,
};
use crate::providers::pull_files::{continue_request, fixture_request};
use serde_json::{Value, json};
const HEAD: &str = "1111111111111111111111111111111111111111";
const BASE: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
fn request() -> PullFileCollectionRequest {
    fixture_request(
        commits_tests::request(HEAD.into()),
        PullFileSourceStrategy::BitbucketCloudDiffstat,
    )
}
fn row(index: u32) -> Value {
    json!({"type":"diffstat","status":"modified","lines_added":2,"lines_removed":1,"old":{"type":"commit_file","path":format!("file{index}")},"new":{"type":"commit_file","path":format!("file{index}")}})
}
fn parent() -> Value {
    json!({"type":"pullrequest","id":67,"destination":{"commit":{"hash":BASE},"repository":{"type":"repository","uuid":format!("{{{REPO}}}"),"full_name":"team/project","name":"project"}},"source":{"commit":{"hash":HEAD},"repository":{"type":"repository","uuid":format!("{{{REPO}}}"),"full_name":"team/project","name":"project"}}})
}
fn path() -> String {
    format!("repositories/%7B%7D/%7B{REPO}%7D/diffstat/{HEAD}..{BASE}")
}
fn url(base: &str, cursor: &str) -> String {
    format!(
        "{base}{}?topic=true&renames=true&pagelen=100&cursor={cursor}",
        path()
    )
}
#[tokio::test]
async fn diffstat_uses_explicit_merge_base_comparison_and_preserves_rename() {
    let mut a = row(1);
    a["status"] = json!("renamed");
    a["old"]["path"] = json!("old/Δ.txt");
    let (provider, calls) = server(|_| {
        vec![
            response(200, "", &json!({"values":[a],"size":1})),
            response(200, "", &parent()),
        ]
    });
    let input = request();
    let page = provider
        .fetch_pull_files(&token(), input.clone())
        .await
        .unwrap();
    assert_eq!(
        page.files[0].identity.old_path.as_deref(),
        Some("old/Δ.txt")
    );
    assert_eq!(
        page.files[0].total_changes,
        PullFileCount::Known("3".into())
    );
    assert_eq!(page.files[0].diff_hint, PullFileDiffHint::Unknown);
    provider
        .validate_pull_file_range(&token(), input)
        .await
        .unwrap();
    let calls = calls.join().unwrap();
    assert!(calls[0].starts_with(&format!(
        "GET /2.0/{}?topic=true&renames=true&pagelen=100 ",
        path()
    )));
    assert!(!calls[0].contains("merge="));
}
#[tokio::test]
async fn opaque_cursor_loop_and_changed_comparison_never_send_a_third_request() {
    let (provider, calls) = server(|base| {
        vec![
            response(
                200,
                "",
                &json!({"values":[row(1)],"next":url(base,"opaqueA")}),
            ),
            response(
                200,
                "",
                &json!({"values":[row(2)],"next":url(base,"opaqueA")}),
            ),
        ]
    });
    let mut input = request();
    let first = provider
        .fetch_pull_files(&token(), input.clone())
        .await
        .unwrap();
    continue_request(&mut input, &first);
    let mut wrong = input.clone();
    wrong.binding.context.base_oid = format!("{:040x}", 42);
    wrong.lease.binding = wrong.binding.clone();
    assert!(provider.fetch_pull_files(&token(), wrong).await.is_err());
    assert!(provider.fetch_pull_files(&token(), input).await.is_err());
    assert_eq!(calls.join().unwrap().len(), 2);
}
#[tokio::test]
async fn terminal_size_mismatch_reports_provider_overflow() {
    let (provider, calls) =
        server(|_| vec![response(200, "", &json!({"values":[row(1)],"size":8}))]);
    let page = provider
        .fetch_pull_files(&token(), request())
        .await
        .unwrap();
    assert_eq!(
        page.cap.unwrap().reason,
        PullFileCapReason::ProviderOverflow
    );
    calls.join().unwrap();
}
#[tokio::test]
async fn hostile_next_and_parent_drift_keep_quota() {
    for next in [
        "https://evil.invalid/steal".into(),
        format!(
            "https://api.bitbucket.org/2.0/{}?topic=false&renames=true&pagelen=100&page=2",
            path()
        ),
    ] {
        let (provider, calls) = server(|_| {
            vec![response(
                200,
                "Retry-After: 45\r\n",
                &json!({"values":[row(1)],"next":next}),
            )]
        });
        let error = provider
            .fetch_pull_files(&token(), request())
            .await
            .unwrap_err();
        assert_eq!(error.account_cooldown_seconds, Some(45));
        calls.join().unwrap();
    }
    let mut p = parent();
    p["source"]["commit"]["hash"] = json!(format!("{:040x}", 42));
    let (provider, calls) = server(|_| vec![response(200, "Retry-After: 45\r\n", &p)]);
    assert_eq!(
        provider
            .validate_pull_file_range(&token(), request())
            .await
            .unwrap_err()
            .account_cooldown_seconds,
        Some(45)
    );
    calls.join().unwrap();
}
#[tokio::test]
async fn bounded_page_count_never_follows_page_thirty_one() {
    let (provider, calls) = server(|base| {
        (1..=30)
            .map(|page| {
                response(
                    200,
                    "",
                    &json!({"values":[row(page)],"next":url(base,&format!("c{page}"))}),
                )
            })
            .collect()
    });
    let mut input = request();
    for index in 0..30 {
        let page = provider
            .fetch_pull_files(&token(), input.clone())
            .await
            .unwrap();
        if index < 29 {
            continue_request(&mut input, &page)
        } else {
            assert_eq!(page.cap.unwrap().reason, PullFileCapReason::LocalPageLimit);
            assert!(page.next_cursor.is_none());
        }
    }
    assert_eq!(calls.join().unwrap().len(), 30);
}

#[tokio::test]
async fn diffstat_total_cannot_change_or_disappear_to_fabricate_completion() {
    for terminal_size in [Some(2_u64), None] {
        let (provider, calls) = server(|base| {
            vec![
                response(
                    200,
                    "",
                    &json!({"values":[row(1)],"size":100,"next":url(base,"next")}),
                ),
                response(200, "", &json!({"values":[row(2)],"size":terminal_size})),
            ]
        });
        let mut input = request();
        let first = provider
            .fetch_pull_files(&token(), input.clone())
            .await
            .unwrap();
        continue_request(&mut input, &first);
        let result = provider.fetch_pull_files(&token(), input).await;
        if terminal_size.is_some() {
            assert!(result.is_err());
        } else {
            assert_eq!(
                result.unwrap().cap.unwrap().reason,
                PullFileCapReason::ProviderOverflow
            );
        }
        assert_eq!(calls.join().unwrap().len(), 2);
    }
}

fn text_response(content_type: &str, text: &str) -> String {
    format!(
        "HTTP/1.1 200 Fixture\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{text}",
        text.len()
    )
}
#[tokio::test]
async fn selected_topic_path_requires_exact_single_diff_section() {
    let patch = "diff --git a/file1 b/file1\n--- a/file1\n+++ b/file1\n@@ -1 +1 @@\n-old\n+new\n";
    for (text, accepted) in [
        (patch.to_string(), true),
        (patch.replace("file1", "file2"), false),
        (format!("{patch}{patch}"), false),
    ] {
        let (provider, calls) = server(|_| {
            vec![
                response(200, "", &json!({"values":[row(1)],"size":1})),
                text_response("text/plain; charset=utf-8", &text),
            ]
        });
        let input = request();
        let page = provider
            .fetch_pull_files(&token(), input.clone())
            .await
            .unwrap();
        let selected =
            crate::providers::pull_files::selected_fixture(&input, page.files[0].clone(), 0);
        let content = provider.fetch_pull_file_artifact(&token(), selected).await;
        assert_eq!(content.is_ok(), accepted);
        if let Ok(content) = content {
            assert_eq!(content.unified_text.as_deref(), Some(patch));
        }
        let calls = calls.join().unwrap();
        assert!(calls[1].contains(&format!(
            "/diff/{HEAD}..{BASE}?topic=true&renames=true&context=3&binary=false&path=file1 "
        )));
        assert!(!calls[1].contains("merge="));
    }
}
#[tokio::test]
async fn selected_diff_content_type_and_size_are_bounded_without_partial_text() {
    for (mime, text, state) in [
        ("text/html", "<html>login</html>".into(), None),
        (
            "text/plain",
            "x".repeat(MAX_PULL_FILE_LINE_BYTES + 1),
            Some(PullFileContentState::Oversized),
        ),
        (
            "text/plain",
            "diff --git a/file1 b/file1\nBinary files a/file1 and b/file1 differ\n".into(),
            Some(PullFileContentState::Omitted),
        ),
    ] {
        let (provider, calls) = server(|_| {
            vec![
                response(200, "", &json!({"values":[row(1)],"size":1})),
                text_response(mime, &text),
            ]
        });
        let input = request();
        let page = provider
            .fetch_pull_files(&token(), input.clone())
            .await
            .unwrap();
        let selected =
            crate::providers::pull_files::selected_fixture(&input, page.files[0].clone(), 0);
        let result = provider.fetch_pull_file_artifact(&token(), selected).await;
        if let Some(state) = state {
            let content = result.unwrap();
            assert_eq!(content.content_state, state);
            assert!(content.unified_text.is_none());
        } else {
            assert!(result.is_err());
        }
        calls.join().unwrap();
    }
}
