use super::*;
use crate::providers::gitlab::{
    commits_tests,
    tests::{response, server},
};
use serde_json::{Value, json};
const HEAD: &str = "1111111111111111111111111111111111111111";
const BASE: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
fn token() -> SecretToken {
    SecretToken::new("synthetic_review_token".into()).unwrap()
}
fn request(facet: DetailFacet) -> ReviewRequest {
    let input = commits_tests::request(HEAD.into());
    ReviewRequest {
        context: ReviewContext {
            base_oid: BASE.into(),
            head_oid: HEAD.into(),
            base_repository_provider_id: input.repository.provider_id.clone(),
            source_repository_provider_id: input.context.source_repository_provider_id,
            metadata_facet_revision: "1".into(),
        },
        detail: DetailRequest {
            account: input.account,
            repository: input.repository,
            subject: input.subject,
            facet,
            cursor: None,
            etag: None,
            source: None,
        },
    }
}
fn approval_response() -> Value {
    json!({"id":999,"iid":67,"project_id":123,"approved":true,"approvals_required":0,"approvals_left":0,
    "approved_by":[{"user":{"id":9007199254740993_u64,"username":"reviewer"},"approved_at":"2026-10-08T00:00:00Z"}]})
}
fn note(id: u64) -> Value {
    json!({"id":id,"noteable_id":999,"noteable_iid":67,"noteable_type":"MergeRequest","project_id":123,
    "type":"DiscussionNote","body":"Saved native discussion","author":{"id":17,"username":"author"},"system":false,
    "created_at":"2026-10-07T00:00:00Z","updated_at":"2026-10-08T00:00:00Z"})
}
fn discussion(id: &str, notes: Vec<Value>) -> Value {
    json!({"id":id,"individual_note":false,"notes":notes})
}
fn json_text(value: &Value) -> String {
    serde_json::to_string(value).unwrap()
}

#[tokio::test]
async fn approvals_are_provider_observations_with_unknown_commit_anchors() {
    let (provider, calls) = server(|_| vec![response(200, "", &json_text(&approval_response()))]);
    let page = provider
        .fetch_reviews(&token(), request(DetailFacet::ReviewSummaries))
        .await
        .unwrap();
    assert_eq!(page.entries.len(), 1);
    assert_eq!(
        page.reconciliation.enumeration,
        DetailEnumeration::FullEnumeration
    );
    let NativeDetailPayload::ReviewV1(value) = page.entries[0].native.as_ref().unwrap() else {
        panic!("approval")
    };
    assert_eq!(value.decision, ReviewDecision::Approved);
    assert!(value.reviewed_commit_oid.is_none());
    assert_eq!(value.context.head_oid, HEAD);
    assert_eq!(
        value.reviewer.as_ref().unwrap().provider_id,
        "9007199254740993"
    );
    assert_eq!(page.entries[0].body.state, DetailValueState::Omitted);
    let calls = calls.join().unwrap();
    assert_eq!(calls.len(), 1);
    assert!(calls[0].starts_with("GET /api/v4/projects/123/merge_requests/67/approvals "));
}

#[tokio::test]
async fn general_diff_and_system_notes_keep_native_semantics_and_no_invented_parents() {
    let mut diff = note(18);
    diff["type"] = json!("DiffNote");
    diff["resolved"] = json!(false);
    diff["resolvable"] = json!(true);
    diff["position"] = json!({"position_type":"text","base_sha":"2222222222222222222222222222222222222222","start_sha":BASE,"head_sha":HEAD,"old_path":"old path","new_path":"new path","new_line":8});
    let mut system = note(19);
    system["system"] = json!(true);
    system["body"] = json!("x".repeat(65_537));
    let rows = json!([
        discussion("general", vec![note(17)]),
        discussion("diff", vec![diff, system])
    ]);
    let (provider, calls) = server(|_| vec![response(200, "", &json_text(&rows))]);
    let page = provider
        .fetch_reviews(&token(), request(DetailFacet::ReviewThreads))
        .await
        .unwrap();
    assert_eq!(page.entries.len(), 3);
    for entry in &page.entries {
        let NativeDetailPayload::ReviewThreadV1(value) = entry.native.as_ref().unwrap() else {
            panic!("thread")
        };
        assert!(
            value.anchor.is_none()
                && value.root_comment_id.is_none()
                && value.parent_comment_id.is_none()
                && value.provider_outdated.is_none()
        );
        assert!(value.native.as_ref().unwrap().is_valid());
    }
    let NativeDetailPayload::ReviewThreadV1(value) = page.entries[1].native.as_ref().unwrap()
    else {
        panic!("thread")
    };
    assert_eq!(value.provider_resolved, Some(false));
    let ReviewThreadNativeV1::Gitlab(native) = value.native.as_deref().unwrap();
    assert_eq!(
        native.position.as_ref().unwrap().head_oid.as_deref(),
        Some(HEAD)
    );
    assert_ne!(
        native.position.as_ref().unwrap().base_oid,
        native.position.as_ref().unwrap().start_oid
    );
    assert_eq!(page.entries[2].body.state, DetailValueState::Oversized);
    assert!(page.entries[2].body.text.is_none());
    assert_eq!(calls.join().unwrap().len(), 1);
}

#[tokio::test]
async fn nested_page_and_approval_limits_never_claim_complete_coverage() {
    let rows = json!([discussion("long", (1..=51).map(note).collect())]);
    let mut approvals = approval_response();
    approvals["approved_by"] = (1..=51)
        .map(|id| json!({"user":{"id":id,"username":format!("user{id}")}}))
        .collect();
    let (provider, calls) = server(|_| {
        vec![
            response(200, "", &json_text(&rows)),
            response(200, "", &json_text(&approvals)),
        ]
    });
    for facet in [DetailFacet::ReviewThreads, DetailFacet::ReviewSummaries] {
        let page = provider
            .fetch_reviews(&token(), request(facet))
            .await
            .unwrap();
        assert_eq!(page.entries.len(), 50);
        assert_eq!(
            page.reconciliation.enumeration,
            DetailEnumeration::Uncertain
        );
        assert!(page.next_cursor.is_none());
    }
    assert_eq!(calls.join().unwrap().len(), 2);
}

#[tokio::test]
async fn continuation_pins_exact_context_and_resumed_terminal_remains_partial() {
    let (provider, calls) = server(|base| {
        vec![
            response(
                200,
                &format!(
                    "Link: <{base}projects/123/merge_requests/67/discussions?per_page=50&page=2>; rel=\"next\"\r\n"
                ),
                &json_text(&json!([discussion("one", vec![note(1)])])),
            ),
            response(
                200,
                "",
                &json_text(&json!([discussion("two", vec![note(2)])])),
            ),
        ]
    });
    let mut input = request(DetailFacet::ReviewThreads);
    input.detail.cursor = provider
        .fetch_reviews(&token(), input.clone())
        .await
        .unwrap()
        .next_cursor;
    for field in ["account", "epoch", "context", "pages", "url"] {
        let mut forged = input.clone();
        let mut cursor: Value =
            serde_json::from_str(forged.detail.cursor.as_ref().unwrap()).unwrap();
        match field {
            "context" => cursor["context"]["head_oid"] = json!(BASE),
            "pages" => cursor[field] = json!(u64::MAX),
            "url" => cursor[field] = json!("https://other.invalid/private"),
            _ => cursor[field] = json!("changed"),
        }
        forged.detail.cursor = Some(json_text(&cursor));
        assert_eq!(
            provider
                .fetch_reviews(&token(), forged)
                .await
                .unwrap_err()
                .kind,
            ProviderErrorKind::InvalidResponse
        );
    }
    let last = provider.fetch_reviews(&token(), input).await.unwrap();
    assert!(last.next_cursor.is_none());
    assert_eq!(
        last.reconciliation.enumeration,
        DetailEnumeration::Uncertain
    );
    assert_eq!(calls.join().unwrap().len(), 2);
}

#[tokio::test]
async fn malformed_cross_subject_page_fails_atomically_with_quota() {
    let mut other = note(2);
    other["noteable_id"] = json!(1000);
    let (provider, calls) = server(|_| {
        vec![response(
            200,
            "RateLimit-Remaining: 0\r\n",
            &json_text(&json!([discussion("native", vec![note(1), other])])),
        )]
    });
    let error = provider
        .fetch_reviews(&token(), request(DetailFacet::ReviewThreads))
        .await
        .unwrap_err();
    assert_eq!(error.kind, ProviderErrorKind::InvalidResponse);
    assert!(error.account_cooldown_seconds.is_some());
    assert_eq!(calls.join().unwrap().len(), 1);
}

#[tokio::test]
async fn authentication_permission_plan_absence_and_quota_do_not_become_empty_success() {
    for (status, kind) in [
        (401, ProviderErrorKind::Authentication),
        (403, ProviderErrorKind::Permission),
        (404, ProviderErrorKind::NotFound),
        (429, ProviderErrorKind::RateLimited),
        (503, ProviderErrorKind::Unavailable),
    ] {
        let (provider, calls) = server(|_| vec![response(status, "Retry-After: 7\r\n", "{}")]);
        let error = provider
            .fetch_reviews(&token(), request(DetailFacet::ReviewSummaries))
            .await
            .unwrap_err();
        assert_eq!(error.kind, kind);
        assert_eq!(calls.join().unwrap().len(), 1);
    }
}

#[tokio::test]
async fn invalid_account_and_context_are_refused_before_http() {
    let (provider, calls) = server(|_| vec![]);
    for n in 0..4 {
        let mut input = request(DetailFacet::ReviewThreads);
        match n {
            0 => input.detail.subject.account_id = "other".into(),
            1 => input.context.base_repository_provider_id = "999".into(),
            2 => input.detail.subject.number = Some("67/../../other".into()),
            _ => input.context.head_oid = BASE.into(),
        };
        assert_eq!(
            provider
                .fetch_reviews(&token(), input)
                .await
                .unwrap_err()
                .kind,
            ProviderErrorKind::InvalidResponse
        );
    }
    assert!(calls.join().unwrap().is_empty());
}

#[tokio::test]
async fn twentieth_page_stops_with_partial_evidence_even_when_provider_has_more() {
    let (provider, calls) = server(|base| {
        (1..=20).map(|page| response(200,
        &format!("Link: <{base}projects/123/merge_requests/67/discussions?per_page=50&page={}>; rel=\"next\"\r\n",page+1),
        &json_text(&json!([discussion(&format!("thread-{page}"),vec![note(page)])])))).collect()
    });
    let mut input = request(DetailFacet::ReviewThreads);
    for page_number in 1..=20 {
        let page = provider
            .fetch_reviews(&token(), input.clone())
            .await
            .unwrap();
        assert_eq!(
            page.reconciliation.enumeration,
            DetailEnumeration::Uncertain
        );
        assert_eq!(page.entries.len(), 1);
        if page_number < 20 {
            assert!(page.next_cursor.is_some());
        } else {
            assert!(page.next_cursor.is_none());
        }
        input.detail.cursor = page.next_cursor;
    }
    assert_eq!(calls.join().unwrap().len(), 20);
}

#[tokio::test]
async fn authoritative_empty_lists_do_not_invent_approval_or_resolution() {
    let mut approvals = approval_response();
    approvals["approved_by"] = json!([]);
    // A provider aggregate can be true with no approver; do not emit a synthetic approval.
    approvals["approved"] = json!(true);
    let (provider, calls) = server(|_| {
        vec![
            response(200, "", &json_text(&approvals)),
            response(200, "", "[]"),
        ]
    });
    for facet in [DetailFacet::ReviewSummaries, DetailFacet::ReviewThreads] {
        let page = provider
            .fetch_reviews(&token(), request(facet))
            .await
            .unwrap();
        assert!(page.entries.is_empty());
        assert!(page.next_cursor.is_none());
        assert_eq!(
            page.reconciliation.enumeration,
            DetailEnumeration::FullEnumeration
        );
    }
    assert_eq!(calls.join().unwrap().len(), 2);
}
