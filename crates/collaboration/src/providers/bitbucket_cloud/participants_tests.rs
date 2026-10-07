//! Actual finite HTTP qualification of singleton-only participant facts.
use super::{tests::*, *};
use crate::NativeDetailPayload;
use serde_json::{Value, json};

fn request_for(repository: &str) -> DetailRequest {
    DetailRequest {
        account: request().account,
        repository: RemoteRepository {
            id: format!("bitbucket_cloud:repository:{repository}"),
            account_id: "fixture-account".into(),
            provider_id: repository.into(),
            full_name: "renamed/project".into(),
            name: "project".into(),
            web_url: "https://bitbucket.org/renamed/project".into(),
            description: None,
            default_branch: None,
            selected: true,
        },
        subject: RemoteItem {
            native_inbox: None,
            id: format!("bitbucket_cloud:pull:{repository}:67"),
            account_id: "fixture-account".into(),
            repository_id: Some(format!("bitbucket_cloud:repository:{repository}")),
            provider_id: format!("{repository}:67"),
            kind: RemoteItemKind::PullRequest,
            number: Some("67".into()),
            title: "saved title".into(),
            body: None,
            body_omitted: true,
            author: None,
            web_url: None,
            state: "open".into(),
            updated_at: "2026-10-04T00:00:00Z".into(),
            head_oid: None,
            is_draft: None,
            reason: None,
            unread: None,
        },
        facet: DetailFacet::Participants,
        cursor: None,
        etag: None,
        source: None,
    }
}
fn person(actor: &str) -> Value {
    json!({"type":"participant","user":{"type":"user","uuid":format!("{{{actor}}}"),"nickname":"same","display_name":"Δ participant"},
        "role":"REVIEWER","approved":true,"state":"approved","participated_on":"2026-10-04T00:00:00Z"})
}
fn singleton(repository: &str, values: Vec<Value>) -> Value {
    json!({"type":"pullrequest","id":67,"destination":{"repository":{"type":"repository","uuid":format!("{{{repository}}}")}},
        "participants":values,
        "updated_on":"not-a-parent-clock","rendered":{"description":{"raw":false}},"description":"unrelated conflict",
        "reviewers":[{"uuid":"never-participant-authority"}],"task_count":42})
}
fn native(entry: &DetailEntry) -> &crate::ParticipantV1 {
    match entry.native.as_ref().unwrap() {
        NativeDetailPayload::ParticipantV1(value) => value,
        NativeDetailPayload::TaskV1(_)
        | NativeDetailPayload::CheckV1(_)
        | NativeDetailPayload::ReviewV1(_)
        | NativeDetailPayload::ReviewThreadV1(_) => {
            panic!("Expected the typed participant payload")
        }
    }
}

#[tokio::test]
async fn actual_participant_singleton_preserves_false_null_unknown_and_no_parent_authority() {
    let mut first = person(ACTOR);
    first["approved"] = false.into();
    first["state"] = Value::Null;
    first["role"] = "FUTURE_ROLE".into();
    first["participated_on"] = Value::Null;
    first["user"]["nickname"] = Value::Null;
    first["user"]["display_name"] = Value::Null;
    let (provider, calls) = server(|_| vec![ok(singleton(REPO, vec![first, person(W1)]))]);
    let page = provider
        .fetch_detail(&token(), request_for(REPO))
        .await
        .unwrap();
    assert_eq!(page.entries.len(), 2);
    assert_eq!(page.reconciliation, DetailReconciliation::full_history());
    assert_eq!(page.body, DetailValue::default());
    assert!(
        page.metadata.is_none()
            && page.next_cursor.is_none()
            && page.etag.is_none()
            && !page.not_modified
    );
    assert_eq!(page.source.source, "bitbucket.participants.v1");
    assert!(page.source.provider_updated_at.is_none());
    let first = &page.entries[0];
    assert_eq!(
        first.id,
        format!("bitbucket_cloud:participant:{REPO}:67:{ACTOR}")
    );
    assert_eq!(native(first).approved, Some(false));
    assert_eq!(native(first).state, None);
    assert_eq!(native(first).role.as_deref(), Some("FUTURE_ROLE"));
    assert_eq!(native(first).user.login, None);
    assert!(native(first).participated_at.is_none());
    assert_eq!(first.field_mask.len(), 6);
    assert!(
        first.author.is_none()
            && first.title.is_none()
            && first.state.is_none()
            && first.updated_at.is_none()
            && first.head_oid.is_none()
    );
    assert_ne!(first.id, page.entries[1].id);
    let calls = calls.join().unwrap();
    assert_eq!(calls.len(), 1);
    assert!(calls[0].starts_with(&format!(
        "GET /2.0/repositories/%7B%7D/%7B{REPO}%7D/pullrequests/67 HTTP/1.1"
    )));
    assert!(!calls[0].to_ascii_lowercase().contains("if-none-match"));
}

#[tokio::test]
async fn actual_participant_optional_omission_is_not_false_null_or_invented_review() {
    let value = json!({"type":"participant","user":{"type":"user","uuid":format!("{{{ACTOR}}}")}});
    let (provider, calls) = server(|_| vec![ok(singleton(REPO, vec![value]))]);
    let page = provider
        .fetch_detail(&token(), request_for(REPO))
        .await
        .unwrap();
    assert!(page.entries[0].field_mask.is_empty());
    let value = native(&page.entries[0]);
    assert_eq!(value.user.provider_id, ACTOR);
    assert!(
        value.role.is_none()
            && value.approved.is_none()
            && value.state.is_none()
            && value.participated_at.is_none()
    );
    assert_eq!(calls.join().unwrap().len(), 1);
}

#[tokio::test]
async fn actual_participant_uuid_authority_ignores_unrelated_destination_presentation() {
    let mut value = singleton(REPO, vec![person(ACTOR)]);
    value["destination"]["repository"]["full_name"] = json!({"not": "a name"});
    value["destination"]["repository"]["links"] = json!({
        "html": {"href": "https://credential:secret@evil.invalid/never-follow"},
        "clone": false,
    });
    let (provider, calls) = server(|_| vec![ok(value)]);
    let page = provider
        .fetch_detail(&token(), request_for(REPO))
        .await
        .unwrap();
    assert_eq!(native(&page.entries[0]).user.provider_id, ACTOR);
    assert_eq!(page.reconciliation, DetailReconciliation::full_history());
    assert!(page.metadata.is_none());
    let calls = calls.join().unwrap();
    assert_eq!(calls.len(), 1);
    assert!(calls[0].starts_with(&format!(
        "GET /2.0/repositories/%7B%7D/%7B{REPO}%7D/pullrequests/67 HTTP/1.1"
    )));
}

#[tokio::test]
async fn actual_participant_empty_and_hundred_entry_bound_are_real_complete_arrays() {
    let values = (1..=100)
        .map(|n| person(&format!("00000000-0000-4000-8000-{n:012x}")))
        .collect();
    let (provider, calls) =
        server(|_| vec![ok(singleton(REPO, vec![])), ok(singleton(REPO, values))]);
    let empty = provider
        .fetch_detail(&token(), request_for(REPO))
        .await
        .unwrap();
    assert!(empty.entries.is_empty());
    assert_eq!(empty.reconciliation, DetailReconciliation::full_history());
    assert_eq!(
        provider
            .fetch_detail(&token(), request_for(REPO))
            .await
            .unwrap()
            .entries
            .len(),
        100
    );
    assert_eq!(calls.join().unwrap().len(), 2);
}

#[tokio::test]
async fn actual_participant_rejects_incomplete_duplicate_oversize_and_malformed_observations_with_quota()
 {
    let mut invalids = vec![];
    for array in [
        Value::Null,
        json!({}),
        json!([person(ACTOR), person(ACTOR)]),
        json!(
            (1..=101)
                .map(|n| person(&format!("00000000-0000-4000-8000-{n:012x}")))
                .collect::<Vec<_>>()
        ),
    ] {
        let mut value = singleton(REPO, vec![]);
        value["participants"] = array;
        invalids.push(value);
    }
    let mut absent = singleton(REPO, vec![]);
    absent.as_object_mut().unwrap().remove("participants");
    invalids.push(absent);
    for (key, value) in [
        ("approved", Value::Null),
        ("approved", json!("false")),
        ("role", Value::Null),
        ("state", json!(true)),
        ("participated_on", json!("not-a-date")),
        ("role", json!("x".repeat(129))),
    ] {
        let mut row = person(ACTOR);
        row[key] = value;
        invalids.push(singleton(REPO, vec![row]));
    }
    for (key, value) in [
        ("uuid", json!("nickname")),
        ("type", json!("team")),
        ("nickname", json!("x".repeat(256))),
        ("display_name", json!("x".repeat(1025))),
    ] {
        let mut row = person(ACTOR);
        row["user"][key] = value;
        invalids.push(singleton(REPO, vec![row]));
    }
    for invalid in invalids {
        let (provider, calls) = server(|_| vec![response(200, "Retry-After: 120\r\n", &invalid)]);
        let error = provider
            .fetch_detail(&token(), request_for(REPO))
            .await
            .unwrap_err();
        assert_eq!(error.kind, ProviderErrorKind::InvalidResponse);
        assert_eq!(error.account_cooldown_seconds, Some(120));
        assert_eq!(calls.join().unwrap().len(), 1);
    }
}

#[tokio::test]
async fn actual_participant_wrong_singleton_and_destination_never_receive_authority() {
    for patch in [
        json!({"id":68}),
        json!({"type":"issue"}),
        json!({"destination":{"repository":{"type":"repository","uuid":format!("{{{W2}}}")}}}),
        json!({"destination":{"repository":{"type":"team","uuid":format!("{{{REPO}}}")}}}),
        json!({"destination":{"repository":{"type":"repository","uuid":"mutable-nickname"}}}),
    ] {
        let mut value = singleton(REPO, vec![person(ACTOR)]);
        for (key, patch) in patch.as_object().unwrap() {
            value[key] = patch.clone();
        }
        let (provider, calls) = server(|_| vec![ok(value)]);
        assert_eq!(
            provider
                .fetch_detail(&token(), request_for(REPO))
                .await
                .unwrap_err()
                .kind,
            ProviderErrorKind::InvalidResponse
        );
        assert_eq!(calls.join().unwrap().len(), 1);
    }
}

#[tokio::test]
async fn actual_participant_pre_dispatch_guard_rejects_foreign_compound_cursor_validator_and_selection()
 {
    let mut requests = vec![];
    let mut req = request_for(REPO);
    req.cursor = Some("https://evil.invalid/participants".into());
    requests.push(req);
    let mut req = request_for(REPO);
    req.etag = Some("never conditional".into());
    requests.push(req);
    let mut req = request_for(REPO);
    req.repository.selected = false;
    requests.push(req);
    let mut req = request_for(REPO);
    req.subject.provider_id = format!("{W2}:67");
    requests.push(req);
    let mut req = request_for(REPO);
    req.subject.account_id = "other-account".into();
    requests.push(req);
    let mut req = request_for(REPO);
    req.subject.kind = RemoteItemKind::Issue;
    requests.push(req);
    let (provider, calls) = server(|_| vec![]);
    for req in requests {
        assert!(provider.fetch_detail(&token(), req).await.is_err());
    }
    assert!(calls.join().unwrap().is_empty());
}

#[tokio::test]
async fn actual_participant_permissions_and_rate_limit_are_native_safe_errors() {
    for (status, kind, wait) in [
        (403, ProviderErrorKind::Permission, None),
        (429, ProviderErrorKind::RateLimited, Some(120)),
    ] {
        let (provider, calls) = server(|_| {
            vec![response(
                status,
                if wait.is_some() {
                    "Retry-After: 120\r\n"
                } else {
                    ""
                },
                &json!({"error":"synthetic fixture"}),
            )]
        });
        let error = provider
            .fetch_detail(&token(), request_for(REPO))
            .await
            .unwrap_err();
        assert_eq!(error.kind, kind);
        assert_eq!(error.account_cooldown_seconds, wait);
        assert_eq!(calls.join().unwrap().len(), 1);
    }
}
