use super::*;
use crate::providers::github::issue_metadata_delivery::GithubIssueMetadataCreationPolicy;
use crate::runtime::detail_tests::fixtures;

use crate::{delivery::*, issue_creation::*, issue_metadata::*};
use serde_json::json;
use std::io::{Read, Write};
fn now() -> String {
    chrono::Utc::now().to_rfc3339()
}
fn time() -> DeliveryTime {
    DeliveryTime {
        now: now(),
        command_now: now(),
    }
}
fn token() -> crate::credentials::SecretToken {
    crate::credentials::SecretToken::new("synthetic_comment".into()).unwrap()
}
fn target() -> serde_json::Value {
    json!({"id":1,"full_name":"owner/project","has_issues":true,"archived":false,"permissions":{"pull":true,"push":false}})
}
fn created(body: &str) -> serde_json::Value {
    json!({"id":9007199254740995_u64,"number":91,"url":"https://api.github.com/repos/owner/project/issues/91","html_url":"https://github.com/owner/project/issues/91","repository_url":"https://api.github.com/repos/owner/project","title":"New issue","body":body,"state":"open","user":{"id":7,"login":"author"},"created_at":"2026-10-08T00:00:00Z","updated_at":"2026-10-08T00:00:00Z"})
}
fn key() -> IssueDraftKey {
    IssueDraftKey {
        account_id: "a".into(),
        draft_id: "11111111-1111-4111-8111-111111111111".into(),
        repository_id: "repo".into(),
    }
}
fn server_headers(
    responses: Vec<(u16, Option<serde_json::Value>, String)>,
) -> (
    GithubIssueMetadataCreationPolicy,
    std::thread::JoinHandle<Vec<String>>,
) {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let url = reqwest::Url::parse(&format!("http://{}/", listener.local_addr().unwrap())).unwrap();
    let handle = std::thread::spawn(move || {
        let mut requests = vec![];
        for (status, response, headers) in responses {
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
            let mut stream = loop {
                match listener.accept() {
                    Ok((s, _)) => break s,
                    Err(e)
                        if e.kind() == std::io::ErrorKind::WouldBlock
                            && std::time::Instant::now() < deadline =>
                    {
                        std::thread::sleep(std::time::Duration::from_millis(5))
                    }
                    Err(e) => panic!("finite server accept: {e}"),
                }
            };
            stream.set_nonblocking(false).unwrap();
            stream
                .set_read_timeout(Some(std::time::Duration::from_secs(5)))
                .unwrap();
            let mut bytes = vec![];
            let mut chunk = [0; 4096];
            loop {
                let n = stream.read(&mut chunk).unwrap();
                if n == 0 {
                    break;
                }
                bytes.extend_from_slice(&chunk[..n]);
                if let Some(end) = bytes.windows(4).position(|w| w == b"\r\n\r\n") {
                    let headers = String::from_utf8_lossy(&bytes[..end]);
                    let len = headers
                        .lines()
                        .find_map(|l| {
                            l.to_ascii_lowercase()
                                .strip_prefix("content-length:")
                                .and_then(|s| s.trim().parse::<usize>().ok())
                        })
                        .unwrap_or(0);
                    if bytes.len() >= end + 4 + len {
                        break;
                    }
                }
                assert!(bytes.len() < 100_000);
            }
            requests.push(String::from_utf8(bytes).unwrap());
            if let Some(response) = response {
                let body = response.to_string();
                write!(stream,"HTTP/1.1 {} Fixture\r\nContent-Length: {}\r\nContent-Type: application/json\r\nConnection: close\r\n{}\r\n{}",status,body.len(),headers,body).unwrap();
            }
        }
        requests
    });
    (
        GithubIssueMetadataCreationPolicy::for_test_base(url),
        handle,
    )
}
async fn setup() -> (tempfile::TempDir, Store, RemoteAccount) {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path().join("comments.db")).await.unwrap();
    let mut account = fixtures::account("a");
    account.actor_id = "7".into();
    let account = store.upsert_account(account).await.unwrap();
    fixtures::project(&store, &account).await;
    (dir, store, account)
}
fn selected() -> IssueMetadataSelection {
    IssueMetadataSelection {
        labels: vec![IssueMetadataLabel {
            provider_id: "11".into(),
            name: "bug".into(),
            color: None,
        }],
        assignees: vec![IssueMetadataAssignee {
            provider_id: "7".into(),
            login: "author".into(),
        }],
        milestone: Some(IssueMetadataMilestone {
            provider_id: "33".into(),
            number: "3".into(),
            title: "Next".into(),
        }),
    }
}
async fn save(store: &Store, selection: IssueMetadataSelection) -> IssueDraftV2Snapshot {
    let k = key();
    let old = store.issue_draft_v2(k.clone()).await.unwrap();
    let a = store.account("a").await.unwrap();
    store
        .save_issue_draft_v2(SaveIssueDraftV2Request {
            account_id: k.account_id,
            draft_id: k.draft_id,
            repository_id: k.repository_id,
            authorization_epoch: a.authorization_epoch,
            authorization_view: old.draft.authorization_view,
            expected_generation: old.draft.generation,
            title: "New issue".into(),
            body: "body".into(),
            metadata: selection,
        })
        .await
        .unwrap()
}
fn request(s: &IssueDraftV2Snapshot) -> SubmitIssueV2Request {
    SubmitIssueV2Request {
        context: s.draft.context.clone().unwrap(),
        draft_id: s.draft.draft_id.clone(),
        draft_generation: s.draft.generation.clone(),
        command_id: Uuid::new_v4().to_string(),
        accept_background_delivery: true,
        accept_metadata_best_effort: true,
    }
}
fn reads() -> Vec<(u16, Option<serde_json::Value>, String)> {
    vec![
        (
            200,
            Some(
                json!({"id":1,"full_name":"owner/project","has_issues":true,"archived":false,"permissions":{"push":true}}),
            ),
            String::new(),
        ),
        (
            200,
            Some(json!({"id":11,"name":"bug","archived_at":null})),
            String::new(),
        ),
        (200, Some(json!({"id":7,"login":"author"})), String::new()),
        (204, Some(json!(null)), String::new()),
        (
            200,
            Some(json!({"id":33,"number":3,"title":"Renamed title","state":"open"})),
            String::new(),
        ),
    ]
}
async fn prepared(
    store: &Store,
    a: &RemoteAccount,
    p: &GithubIssueMetadataCreationPolicy,
    id: &str,
) -> (ReconcileRequest, DeliveryPreparation) {
    let c = store.delivery_command("a", id).await.unwrap();
    let (r, _) = store.claim_preparation(&c, a, p, &time()).await.unwrap();
    let r = r.unwrap();
    let mut continuation = None;
    for step in 0..54 {
        assert_eq!(
            store.delivery_command("a", id).await.unwrap().attempt_count,
            0
        );
        let next = p
            .prepare_step(&token(), &r, continuation.as_deref())
            .await
            .unwrap();
        match next {
            PreparationStep::Continue(v) => {
                assert!(step < 53);
                continuation = Some(v.bytes);
            }
            PreparationStep::Complete(v) => return (r, v),
        }
    }
    panic!("bounded preflight")
}
async fn claim(
    store: &Store,
    a: &RemoteAccount,
    p: &GithubIssueMetadataCreationPolicy,
    id: &str,
) -> DispatchRequest {
    let (r, d) = prepared(store, a, p, id).await;
    store
        .claim_delivery(&r.command, a, p, &d.bytes, &time())
        .await
        .unwrap()
        .request
        .unwrap()
}
async fn complete(
    store: &Store,
    a: &RemoteAccount,
    p: &GithubIssueMetadataCreationPolicy,
    r: &DispatchRequest,
    report: &DeliveryReport,
) {
    store
        .complete_delivery(
            &r.command,
            p,
            super::delivery::DeliveryCompletion {
                account: a,
                attempt: Some(r.attempt),
                report,
                now: &now(),
                next: &now(),
            },
        )
        .await
        .unwrap();
}
#[tokio::test]
async fn metadata_authorship_is_atomic_offline_cas_and_legacy_cannot_drop_selection() {
    let (dir, store, a) = setup().await;
    let first = save(&store, selected()).await;
    assert_eq!(first, save(&store, selected()).await);
    let legacy = store.issue_draft(key()).await.unwrap();
    let error = store
        .save_issue_draft(SaveIssueDraftRequest {
            account_id: "a".into(),
            draft_id: key().draft_id,
            repository_id: "repo".into(),
            authorization_epoch: a.authorization_epoch.clone(),
            authorization_view: legacy.authorization_view.clone(),
            expected_generation: legacy.generation.clone(),
            title: "lost".into(),
            body: "lost".into(),
        })
        .await
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::InvalidInput);
    let legacy_request = SubmitIssueRequest {
        context: legacy.context.unwrap(),
        draft_id: key().draft_id,
        draft_generation: legacy.generation,
        command_id: Uuid::new_v4().to_string(),
        accept_background_delivery: true,
    };
    assert!(store.submit_issue(legacy_request).await.is_err());
    store.disconnect("a").await.unwrap();
    let offline = save(&store, selected()).await;
    assert!(offline.draft.context.is_none());
    assert_eq!(offline.metadata, selected());
    store.close().await.unwrap();
    let store = Store::open(dir.path().join("comments.db")).await.unwrap();
    assert_eq!(
        store.issue_draft_v2(key()).await.unwrap().metadata,
        selected()
    );
    assert_eq!(
        store
            .issue_drafts_v2(IssueDraftQuery {
                account_id: "a".into(),
                cursor: None,
                limit: 1
            })
            .await
            .unwrap()
            .drafts[0]
            .metadata,
        selected()
    );
    store.close().await.unwrap();
}
#[tokio::test]
async fn metadata_core201_confirms_even_when_optional_fields_differ_or_are_malformed() {
    for malformed in [false, true] {
        let (_dir, store, a) = setup().await;
        let r = request(&save(&store, selected()).await);
        store.submit_issue_v2(r.clone()).await.unwrap();
        let mut response = created("body");
        response["labels"] = if malformed {
            json!([{"name":"only"}])
        } else {
            json!([])
        };
        response["assignees"] = json!([{ "id":7,"login":"author"}]);
        response["milestone"] = json!(null);
        let mut responses = reads();
        responses.push((201, Some(response), String::new()));
        let (p, server) = server_headers(responses);
        let dispatch = claim(&store, &a, &p, &r.command_id).await;
        let report = p.dispatch(&token(), dispatch.clone()).await;
        assert!(matches!(report.outcome, DeliveryOutcome::Confirmed(_)));
        complete(&store, &a, &p, &dispatch, &report).await;
        let s = store.issue_draft_v2(key()).await.unwrap();
        assert_eq!(s.draft.reason, Some(IssueDraftReason::AlreadySubmitted));
        let outcome = s.metadata_outcome.unwrap();
        assert_eq!(outcome.command_id, s.draft.published.unwrap().command_id);
        assert!(outcome.needs_attention);
        assert_eq!(
            outcome.fields[0].result,
            if malformed {
                IssueMetadataResult::Unobserved
            } else {
                IssueMetadataResult::Different
            }
        );
        assert_eq!(outcome.fields[1].result, IssueMetadataResult::Applied);
        assert_eq!(outcome.fields[2].result, IssueMetadataResult::Different);
        let paths = server.join().unwrap();
        assert_eq!(paths.len(), 6);
        assert!(paths[3].starts_with("GET /repositories/1/assignees/author "));
        assert!(paths[5].starts_with("POST /repositories/1/issues "));
        assert!(paths[5].contains("\"milestone\":3"));
        assert!(store.submit_issue_v2(r).await.unwrap().duplicate);
        store.close().await.unwrap();
    }
}
#[tokio::test]
async fn metadata_permission_and_identity_declines_record_no_attempt_or_post() {
    for changed in [false, true] {
        let (_dir, store, a) = setup().await;
        let r = request(&save(&store, selected()).await);
        store.submit_issue_v2(r.clone()).await.unwrap();
        let responses = if changed {
            vec![
                reads().remove(0),
                (
                    200,
                    Some(json!({"id":12,"name":"bug","archived_at":null})),
                    String::new(),
                ),
            ]
        } else {
            vec![(200, Some(target()), String::new())]
        };
        let (p, server) = server_headers(responses);
        let (req, prep) = prepared(&store, &a, &p, &r.command_id).await;
        let claimed = store
            .claim_delivery(&req.command, &a, &p, &prep.bytes, &time())
            .await
            .unwrap();
        assert!(claimed.request.is_none());
        let c = store.delivery_command("a", &r.command_id).await.unwrap();
        assert_eq!(c.state, DeliveryState::Conflict);
        assert_eq!(c.attempt_count, 0);
        assert!(server.join().unwrap().iter().all(|r| r.starts_with("GET ")));
        assert!(
            store
                .issue_draft_v2(key())
                .await
                .unwrap()
                .draft
                .published
                .is_none()
        );
        store.close().await.unwrap();
    }
}
#[tokio::test]
async fn metadata_ambiguous201_keeps_uuid_and_unknown_without_second_post() {
    let (_dir, store, a) = setup().await;
    let r = request(&save(&store, IssueMetadataSelection::default()).await);
    store.submit_issue_v2(r.clone()).await.unwrap();
    let (p, server) = server_headers(vec![
        (200, Some(target()), String::new()),
        (201, Some(json!({"id":9})), String::new()),
    ]);
    let dispatch = claim(&store, &a, &p, &r.command_id).await;
    let report = p.dispatch(&token(), dispatch.clone()).await;
    assert!(matches!(report.outcome, DeliveryOutcome::Unknown));
    complete(&store, &a, &p, &dispatch, &report).await;
    let s = store.issue_draft_v2(key()).await.unwrap();
    assert_eq!(s.draft.reason, Some(IssueDraftReason::PendingSubmission));
    assert!(s.metadata_outcome.is_none());
    assert!(store.submit_issue_v2(r).await.unwrap().duplicate);
    assert_eq!(server.join().unwrap().len(), 2);
    store.close().await.unwrap();
}
#[tokio::test]
async fn metadata_budget_and_consent_fail_before_outbox_and_deselection_blocks_both_versions() {
    let (_dir, store, a) = setup().await;
    let s = save(&store, selected()).await;
    let mut r = request(&s);
    r.accept_metadata_best_effort = false;
    assert_eq!(
        store.submit_issue_v2(r).await.unwrap_err().code,
        ErrorCode::InvalidInput
    );
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM commands")
        .fetch_one(&store.inner.readers)
        .await
        .unwrap();
    assert_eq!(count, 0);
    store.select_repository("a", "repo", false).await.unwrap();
    assert!(
        store
            .issue_draft_v2(key())
            .await
            .unwrap()
            .draft
            .context
            .is_none()
    );
    assert!(store.submit_issue_v2(request(&s)).await.is_err());
    let mut tx = store.inner.readers.begin().await.unwrap();
    assert!(
        super::issue_creation::capture_in(&mut tx, &a, "repo")
            .await
            .is_err()
    );
    tx.commit().await.unwrap();
    store.close().await.unwrap();
}

#[tokio::test]
async fn maximum_selection_runs_exactly_54_reads_before_one_recorded_post() {
    let (_dir, store, a) = setup().await;
    let mut selected = selected();
    selected.labels = (1..=32)
        .map(|id| IssueMetadataLabel {
            provider_id: id.to_string(),
            name: format!("label-{id}"),
            color: None,
        })
        .collect();
    selected.assignees = (100..110)
        .map(|id| IssueMetadataAssignee {
            provider_id: id.to_string(),
            login: format!("user-{id}"),
        })
        .collect();
    let saved = save(&store, selected).await;
    let selected = saved.metadata.clone();
    let r = request(&saved);
    store.submit_issue_v2(r.clone()).await.unwrap();
    let mut responses = vec![reads().remove(0)];
    for label in &selected.labels {
        responses.push((
            200,
            Some(json!({"id":label.provider_id.parse::<u64>().unwrap(),"name":label.name})),
            String::new(),
        ));
    }
    for assignee in &selected.assignees {
        responses.push((
            200,
            Some(json!({"id":assignee.provider_id.parse::<u64>().unwrap(),"login":assignee.login})),
            String::new(),
        ));
        responses.push((204, Some(json!(null)), String::new()));
    }
    responses.push(reads().remove(4));
    responses.push((201, Some(created("body")), String::new()));
    assert_eq!(responses.len(), 55);
    let (p, server) = server_headers(responses);
    let dispatch = claim(&store, &a, &p, &r.command_id).await;
    assert_eq!(
        store
            .delivery_command("a", &r.command_id)
            .await
            .unwrap()
            .attempt_count,
        1
    );
    let report = p.dispatch(&token(), dispatch.clone()).await;
    assert!(matches!(report.outcome, DeliveryOutcome::Confirmed(_)));
    complete(&store, &a, &p, &dispatch, &report).await;
    let calls = server.join().unwrap();
    assert_eq!(calls.iter().filter(|v| v.starts_with("GET ")).count(), 54);
    assert_eq!(calls.iter().filter(|v| v.starts_with("POST ")).count(), 1);
    store.close().await.unwrap();
}
#[tokio::test]
async fn escaped_combined_receipt_budget_refuses_before_admission_but_preserves_authored_text() {
    let (_dir, store, a) = setup().await;
    let old = store.issue_draft_v2(key()).await.unwrap();
    let body = "\u{1}".repeat(16_384);
    let saved = store
        .save_issue_draft_v2(SaveIssueDraftV2Request {
            account_id: "a".into(),
            draft_id: key().draft_id,
            repository_id: "repo".into(),
            authorization_epoch: a.authorization_epoch,
            authorization_view: old.draft.authorization_view,
            expected_generation: old.draft.generation,
            title: "New issue".into(),
            body: body.clone(),
            metadata: selected(),
        })
        .await
        .unwrap();
    let error = store.submit_issue_v2(request(&saved)).await.unwrap_err();
    assert_eq!(error.code, ErrorCode::InvalidInput);
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM commands")
        .fetch_one(&store.inner.readers)
        .await
        .unwrap();
    assert_eq!(count, 0);
    assert_eq!(store.issue_draft_v2(key()).await.unwrap().draft.body, body);
    store.close().await.unwrap();
}
