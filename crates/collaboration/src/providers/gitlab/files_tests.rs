use super::{
    tests::{response, server},
    *,
};
use crate::providers::pull_files::{continue_request, fixture_request};
use serde_json::{Value, json};
const HEAD: &str = "1111111111111111111111111111111111111111";
const BASE: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const MERGE_BASE: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
fn request() -> PullFileCollectionRequest {
    let mut request = fixture_request(
        commits_tests::request(HEAD.into()),
        PullFileSourceStrategy::GitlabMergeRequestDiffs,
    );
    request.binding.context.merge_base_oid = Some(MERGE_BASE.into());
    request.lease.binding = request.binding.clone();
    request
}
fn token() -> SecretToken {
    SecretToken::new("synthetic_files_token".into()).unwrap()
}
fn row(index: u32) -> Value {
    json!({"old_path":format!("file{index}"),"new_path":format!("file{index}"),"new_file":false,"deleted_file":false,"renamed_file":false,"a_mode":"100644","b_mode":"100755","diff":"@@ -1 +1 @@\n-a\n+b","collapsed":false,"too_large":false,"generated_file":false})
}
fn parent() -> Value {
    json!({"id":999,"iid":67,"project_id":123,"target_project_id":123,"source_project_id":456,"sha":HEAD,"diff_refs":{"head_sha":HEAD,"start_sha":BASE,"base_sha":MERGE_BASE},"changes_count":"1"})
}
#[tokio::test]
async fn divergent_target_and_merge_base_are_distinct_and_flags_are_not_collapsed() {
    let mut a = row(1);
    a["collapsed"] = json!(true);
    a["generated_file"] = json!(true);
    let mut b = row(2);
    b["too_large"] = json!(true);
    let (provider, calls) = server(|_| {
        vec![
            response(200, "", &json!([a, b]).to_string()),
            response(200, "", &parent().to_string()),
        ]
    });
    let input = request();
    let page = provider
        .fetch_pull_files(&token(), input.clone())
        .await
        .unwrap();
    assert_eq!(page.files[0].provider_collapsed, PullFileFlag::Known(true));
    assert_eq!(page.files[0].provider_too_large, PullFileFlag::Known(false));
    assert_eq!(page.files[0].generated, PullFileFlag::Known(true));
    assert_eq!(page.files[0].diff_hint, PullFileDiffHint::Omitted);
    assert_eq!(page.files[1].diff_hint, PullFileDiffHint::Oversized);
    assert_eq!(page.files[1].mode_changed, PullFileFlag::Known(true));
    let validated = provider
        .validate_pull_file_range(&token(), input)
        .await
        .unwrap();
    assert_eq!(validated.validation.base_oid, BASE);
    assert_eq!(
        validated.validation.merge_base_oid.as_deref(),
        Some(MERGE_BASE)
    );
    assert_eq!(validated.expected_file_count, Some(1));
    let calls = calls.join().unwrap();
    assert!(
        calls[0]
            .starts_with("GET /api/v4/projects/123/merge_requests/67/diffs?per_page=100&page=1 ")
    );
}
#[tokio::test]
async fn terminal_count_is_required_and_capped_count_never_proves_complete() {
    for count in [
        Value::Null,
        json!(""),
        json!("1000+"),
        json!("0"),
        json!("01"),
    ] {
        let mut p = parent();
        p["changes_count"] = count.clone();
        let (provider, calls) =
            server(|_| vec![response(200, "RateLimit-Remaining: 0\r\n", &p.to_string())]);
        let result = provider.validate_pull_file_range(&token(), request()).await;
        match count.as_str() {
            Some("1000+") => {
                let result = result.unwrap();
                assert!(result.expected_file_count.is_none());
                assert_eq!(
                    result.collection_cap.unwrap().reason,
                    PullFileCapReason::ProviderFileLimit
                );
            }
            Some("0") => assert_eq!(result.unwrap().expected_file_count, Some(0)),
            _ => assert!(result.unwrap_err().account_cooldown_seconds.is_some()),
        }
        calls.join().unwrap();
    }
}
#[tokio::test]
async fn pages_bind_account_epoch_authorization_view_and_exact_refs() {
    let (provider, calls) = server(|base| {
        vec![
            response(
                200,
                &format!(
                    "Link: <{base}projects/123/merge_requests/67/diffs?per_page=100&page=2>; rel=\"next\"\r\n"
                ),
                &json!([row(1)]).to_string(),
            ),
            response(200, "", &json!([row(2)]).to_string()),
        ]
    });
    let mut input = request();
    let page = provider
        .fetch_pull_files(&token(), input.clone())
        .await
        .unwrap();
    continue_request(&mut input, &page);
    let mut wrong = input.clone();
    wrong.authorization_view = "2".into();
    wrong.lease.authorization_view = "2".into();
    assert!(provider.fetch_pull_files(&token(), wrong).await.is_err());
    let page = provider.fetch_pull_files(&token(), input).await.unwrap();
    assert_eq!(page.start_position, 1);
    assert!(page.next_cursor.is_none());
    assert_eq!(calls.join().unwrap().len(), 2);
}
#[tokio::test]
async fn malformed_flags_parent_drift_and_unsafe_next_preserve_received_quota() {
    let mut malformed = row(1);
    malformed["new_file"] = json!(true);
    malformed["deleted_file"] = json!(true);
    let (provider, calls) = server(|_| {
        vec![response(
            200,
            "RateLimit-Remaining: 0\r\n",
            &json!([malformed]).to_string(),
        )]
    });
    assert!(
        provider
            .fetch_pull_files(&token(), request())
            .await
            .unwrap_err()
            .account_cooldown_seconds
            .is_some()
    );
    calls.join().unwrap();
    for field in ["start_sha", "base_sha", "head_sha"] {
        let mut p = parent();
        p["diff_refs"][field] = json!(format!("{:040x}", 42));
        let (provider, calls) =
            server(|_| vec![response(200, "RateLimit-Remaining: 0\r\n", &p.to_string())]);
        assert!(
            provider
                .validate_pull_file_range(&token(), request())
                .await
                .unwrap_err()
                .account_cooldown_seconds
                .is_some()
        );
        calls.join().unwrap();
    }
    let (provider, calls) = server(|base| {
        vec![response(
            200,
            &format!(
                "Link: <{base}projects/456/merge_requests/67/diffs?per_page=100&page=2>; rel=\"next\"\r\n"
            ),
            &json!([row(1)]).to_string(),
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
#[tokio::test]
async fn page_thirty_is_an_explicit_local_cap() {
    let (provider, calls) = server(|base| {
        (1..=30).map(|page|response(200,&format!("Link: <{base}projects/123/merge_requests/67/diffs?per_page=100&page={}>; rel=\"next\"\r\n",page+1),&json!([row(page)]).to_string())).collect()
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
async fn asynchronous_missing_diff_refs_are_retryable_with_quota() {
    for value in [
        Value::Null,
        json!({}),
        json!({"head_sha":"","start_sha":"","base_sha":""}),
    ] {
        let mut p = parent();
        p["diff_refs"] = value;
        let (provider, calls) =
            server(|_| vec![response(200, "RateLimit-Remaining: 0\r\n", &p.to_string())]);
        let error = provider
            .validate_pull_file_range(&token(), request())
            .await
            .unwrap_err();
        assert_eq!(error.kind, ProviderErrorKind::Unavailable);
        assert!(error.account_cooldown_seconds.is_some());
        calls.join().unwrap();
    }
}

#[tokio::test]
async fn selected_ordinal_preserves_collapsed_oversized_and_omitted_states() {
    for state in ["text", "collapsed", "too_large", "omitted"] {
        let mut observed = row(1);
        match state {
            "collapsed" => observed["collapsed"] = json!(true),
            "too_large" => observed["too_large"] = json!(true),
            "omitted" => observed["diff"] = Value::Null,
            _ => {}
        }
        let (provider, calls) = server(|_| {
            vec![
                response(200, "", &json!([row(1)]).to_string()),
                response(200, "", &json!([observed]).to_string()),
                response(200, "", &parent().to_string()),
            ]
        });
        let input = request();
        let page = provider
            .fetch_pull_files(&token(), input.clone())
            .await
            .unwrap();
        let selected =
            crate::providers::pull_files::selected_fixture(&input, page.files[0].clone(), 129);
        let content = provider
            .fetch_pull_file_artifact(&token(), selected.clone())
            .await
            .unwrap();
        assert_eq!(
            content.content_state,
            match state {
                "text" => PullFileContentState::Text,
                "too_large" => PullFileContentState::Oversized,
                _ => PullFileContentState::Omitted,
            }
        );
        assert_eq!(content.unified_text.is_some(), state == "text");
        provider
            .validate_selected_pull_file_range(&token(), selected)
            .await
            .unwrap();
        let calls = calls.join().unwrap();
        assert!(
            calls[1].starts_with(
                "GET /api/v4/projects/123/merge_requests/67/diffs?per_page=1&page=130 "
            )
        );
    }
}
#[tokio::test]
async fn selected_identity_and_page_limit_are_fenced_before_parent_validation() {
    let (provider, calls) = server(|_| {
        vec![
            response(200, "", &json!([row(1)]).to_string()),
            response(
                200,
                "RateLimit-Remaining: 0\r\n",
                &json!([row(2)]).to_string(),
            ),
        ]
    });
    let input = request();
    let page = provider
        .fetch_pull_files(&token(), input.clone())
        .await
        .unwrap();
    let selected = crate::providers::pull_files::selected_fixture(&input, page.files[0].clone(), 0);
    let mut malformed = selected.clone();
    malformed.file.provider_position = MAX_PULL_FILES;
    assert!(
        provider
            .fetch_pull_file_artifact(&token(), malformed)
            .await
            .is_err()
    );
    let error = provider
        .fetch_pull_file_artifact(&token(), selected)
        .await
        .unwrap_err();
    assert_eq!(error.kind, ProviderErrorKind::InvalidResponse);
    assert!(error.account_cooldown_seconds.is_some());
    assert_eq!(calls.join().unwrap().len(), 2);
}
