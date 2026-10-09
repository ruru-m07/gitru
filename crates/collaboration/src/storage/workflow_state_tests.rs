use super::*;
use crate::delivery::*;
use crate::providers::github::workflow_state::GithubWorkflowStatePolicy;
use crate::runtime::detail_tests::fixtures;
use crate::workflow_state::native::*;
use crate::*;
use serde_json::json;
use std::io::{Read, Write};
const HEAD: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
fn now() -> String {
    chrono::Utc::now().to_rfc3339()
}
fn time() -> DeliveryTime {
    DeliveryTime {
        now: now(),
        command_now: now(),
    }
}
fn response(state: &str) -> serde_json::Value {
    json!({"id":9007199254740997_u64,"number":67,"url":"https://api.github.com/repos/owner/project/pulls/67","html_url":"https://github.com/owner/project/pull/67","title":"Remote title","body":"Remote body","state":state,"merged":false,"updated_at":"2026-10-07T23:00:00Z","head":{"ref":"feature","sha":HEAD,"repo":null},"base":{"ref":"main","sha":"b".repeat(40),"repo":{"id":1,"full_name":"owner/project","html_url":"https://github.com/owner/project"}}})
}
fn server(
    responses: Vec<Option<serde_json::Value>>,
) -> (
    GithubWorkflowStatePolicy,
    std::thread::JoinHandle<Vec<String>>,
) {
    server_headers(
        responses
            .into_iter()
            .map(|v| (200, v, String::new()))
            .collect(),
    )
}
fn server_headers(
    responses: Vec<(u16, Option<serde_json::Value>, String)>,
) -> (
    GithubWorkflowStatePolicy,
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
    (GithubWorkflowStatePolicy::for_test_base(url), handle)
}
async fn setup() -> (tempfile::TempDir, Store, RemoteAccount) {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path().join("text.db")).await.unwrap();
    let account = fixtures::seed(&store, "a").await;
    let mut item = store.item("a", "pull").await.unwrap().item.unwrap();
    item.head_oid = Some(HEAD.into());
    item.title = "Base title".into();
    let run = store
        .begin_sync("a", "1", "repo:repo:pull_request")
        .await
        .unwrap();
    store
        .apply_page(PageCommit {
            account_id: "a".into(),
            authorization_epoch: "1".into(),
            scope: "repo:repo:pull_request".into(),
            run_id: run,
            repositories: vec![],
            items: vec![item],
            endpoint_aliases: vec![],
            next_cursor: None,
            etag: None,
            last_modified: None,
            not_modified: false,
            complete: true,
            observed_at: now(),
        })
        .await
        .unwrap();
    observed(&store, &account, "open", HEAD).await;
    (dir, store, account)
}
async fn observed(store: &Store, account: &RemoteAccount, state: &str, head: &str) {
    observed_body(
        store,
        account,
        state,
        head,
        fixtures::known(Some("Base body")),
    )
    .await;
}
async fn observed_body(
    store: &Store,
    account: &RemoteAccount,
    state: &str,
    head: &str,
    body: DetailValue,
) {
    let mut commit = fixtures::commit(store, account, DetailFacet::Body).await;
    commit.body = body;
    commit.source.source = "github/pull-detail/2026-03-10".into();
    commit.source.provider_updated_at = Some("2026-10-07T22:00:00Z".into());
    commit.source.observed_at = now();
    commit.subject_binding = Some(DetailSubjectBinding {
        repository_id: "repo".into(),
        repository_provider_id: "1".into(),
        provider_id: "9007199254740997".into(),
        number: Some("67".into()),
        kind: RemoteItemKind::PullRequest,
        head_oid: Some(head.into()),
    });
    commit.metadata = Some(ResourceMetadataObservation {
        kind: RemoteItemKind::PullRequest,
        values: ResourceMetadataValues {
            title: Some("Base title".into()),
            state: Some(state.into()),
            head: Some(DetailBranch {
                name: "feature".into(),
                oid: head.into(),
                repository: None,
            }),
            updated_at: commit.source.provider_updated_at.clone(),
            ..Default::default()
        },
        fields: [
            MetadataField::Title,
            MetadataField::State,
            MetadataField::Head,
            MetadataField::UpdatedAt,
        ]
        .into_iter()
        .map(|field| MetadataObservedField {
            field,
            state: DetailValueState::Known,
        })
        .collect(),
        source: MetadataSource {
            source: commit.source.source.clone(),
            adapter_version: 1,
            provider_updated_at: commit.source.provider_updated_at.clone(),
            observed_at: commit.source.observed_at.clone(),
        },
    });
    store.apply_detail(commit).await.unwrap();
}
async fn request(store: &Store, state: WorkflowState) -> WorkflowStateRequest {
    let snapshot = store.workflow_state_snapshot("a", "pull").await.unwrap();
    assert_eq!(snapshot.reason, None);
    WorkflowStateRequest {
        context: snapshot.context.unwrap(),
        command_id: Uuid::new_v4().to_string(),
        accept_best_effort: true,
        desired_state: state,
    }
}
async fn prepare(
    store: &Store,
    account: &RemoteAccount,
    policy: &GithubWorkflowStatePolicy,
    id: &str,
) -> (ReconcileRequest, DeliveryPreparation) {
    let command = store.delivery_command("a", id).await.unwrap();
    let (request, _) = store
        .claim_preparation(&command, account, policy, &time())
        .await
        .unwrap();
    let request = request.unwrap();
    let preparation = policy.prepare(&token(), &request).await.unwrap();
    (request, preparation)
}
fn token() -> crate::credentials::SecretToken {
    crate::credentials::SecretToken::new("synthetic_text_test".into()).unwrap()
}

async fn claim(
    store: &Store,
    account: &RemoteAccount,
    policy: &GithubWorkflowStatePolicy,
    id: &str,
) -> super::delivery::DeliveryClaim {
    let (req, prep) = prepare(store, account, policy, id).await;
    store
        .claim_delivery(&req.command, account, policy, &prep.bytes, &time())
        .await
        .unwrap()
}
async fn complete(
    store: &Store,
    account: &RemoteAccount,
    policy: &GithubWorkflowStatePolicy,
    command: &DeliveryCommand,
    attempt: Option<i64>,
    report: &DeliveryReport,
) {
    store
        .complete_delivery(
            command,
            policy,
            super::delivery::DeliveryCompletion {
                account,
                attempt,
                report,
                now: &now(),
                next: &now(),
            },
        )
        .await
        .unwrap();
}
#[tokio::test]
async fn workflow_admission_changes_only_state_and_deduplicates_after_cold_reopen() {
    let (dir, store, _) = setup().await;
    let r = request(&store, WorkflowState::Closed).await;
    let receipt = store.submit_workflow_state(r.clone()).await.unwrap();
    let item = store.item("a", "pull").await.unwrap();
    assert!(item.pending_intent.is_some());
    let item = item.item.unwrap();
    assert_eq!(item.state, "closed");
    assert_eq!(item.title, "Base title");
    let before = store
        .delivery_command("a", &r.command_id)
        .await
        .unwrap()
        .payload;
    assert_eq!(
        store
            .workflow_state_snapshot("a", "pull")
            .await
            .unwrap()
            .reason,
        Some(WorkflowStateReason::PendingIntent)
    );
    store.close().await.unwrap();
    let store = Store::open(dir.path().join("text.db")).await.unwrap();
    let retry = store.submit_workflow_state(r.clone()).await.unwrap();
    assert!(retry.duplicate);
    assert_eq!(retry.admitted_revision, receipt.admitted_revision);
    assert_eq!(
        store
            .delivery_command("a", &r.command_id)
            .await
            .unwrap()
            .payload,
        before
    );
    let mut different = r;
    different.desired_state = WorkflowState::Open;
    assert!(store.submit_workflow_state(different).await.is_err());
    store.close().await.unwrap();
}
#[tokio::test]
async fn workflow_rejects_unconsented_noop_stale_and_unknown_or_merged_bases() {
    let (_dir, store, account) = setup().await;
    let mut r = request(&store, WorkflowState::Closed).await;
    r.accept_best_effort = false;
    assert!(store.submit_workflow_state(r.clone()).await.is_err());
    r.accept_best_effort = true;
    let mut noop = r.clone();
    noop.desired_state = WorkflowState::Open;
    assert!(store.submit_workflow_state(noop).await.is_err());
    for (state, reason) in [
        ("merged", WorkflowStateReason::MergedPullRequest),
        ("future", WorkflowStateReason::UnknownState),
    ] {
        observed(&store, &account, state, HEAD).await;
        assert_eq!(
            store
                .workflow_state_snapshot("a", "pull")
                .await
                .unwrap()
                .reason,
            Some(reason)
        );
        assert!(store.submit_workflow_state(r.clone()).await.is_err());
    }
    store.close().await.unwrap();
}
#[tokio::test]
async fn workflow_converged_preflight_confirms_without_patch_and_preserves_remote_text() {
    let (_dir, store, account) = setup().await;
    let r = request(&store, WorkflowState::Closed).await;
    store.submit_workflow_state(r.clone()).await.unwrap();
    let (policy, server) = server(vec![Some(response("closed"))]);
    let claim = claim(&store, &account, &policy, &r.command_id).await;
    assert!(claim.request.is_none());
    let c = store.delivery_command("a", &r.command_id).await.unwrap();
    assert_eq!(c.state, DeliveryState::Confirmed);
    assert_eq!(c.attempt_count, 0);
    let item = store.item("a", "pull").await.unwrap();
    assert!(item.pending_intent.is_none());
    let item = item.item.unwrap();
    assert_eq!(item.state, "closed");
    assert_eq!(item.title, "Remote title");
    assert_eq!(item.body.as_deref(), Some("Remote body"));
    assert_eq!(server.join().unwrap().len(), 1);
    store.close().await.unwrap();
}
#[tokio::test]
async fn workflow_attempt_precedes_exact_state_only_patch() {
    let (_dir, store, account) = setup().await;
    let r = request(&store, WorkflowState::Closed).await;
    store.submit_workflow_state(r.clone()).await.unwrap();
    let (policy, server) = server(vec![Some(response("open")), Some(response("closed"))]);
    let dispatch = claim(&store, &account, &policy, &r.command_id)
        .await
        .request
        .unwrap();
    assert_eq!(
        store
            .delivery_command("a", &r.command_id)
            .await
            .unwrap()
            .attempt_count,
        1
    );
    let report = policy.dispatch(&token(), dispatch.clone()).await;
    assert!(matches!(report.outcome, DeliveryOutcome::Confirmed(_)));
    complete(
        &store,
        &account,
        &policy,
        &dispatch.command,
        Some(dispatch.attempt),
        &report,
    )
    .await;
    let requests = server.join().unwrap();
    assert!(requests[1].starts_with("PATCH /repositories/1/pulls/67 "));
    assert!(requests[1].ends_with("{\"state\":\"closed\"}"));
    assert_eq!(
        store
            .item("a", "pull")
            .await
            .unwrap()
            .item
            .unwrap()
            .body
            .as_deref(),
        Some("Remote body")
    );
    store.close().await.unwrap();
}
#[tokio::test]
async fn workflow_merged_or_changed_head_preflight_never_dispatches() {
    for merged in [true, false] {
        let (_dir, store, account) = setup().await;
        let r = request(&store, WorkflowState::Closed).await;
        store.submit_workflow_state(r.clone()).await.unwrap();
        let mut remote = response("closed");
        if merged {
            remote["merged"] = true.into();
        } else {
            remote["head"]["sha"] = "c".repeat(40).into();
        }
        let (policy, server) = server(vec![Some(remote)]);
        assert!(
            claim(&store, &account, &policy, &r.command_id)
                .await
                .request
                .is_none()
        );
        let c = store.delivery_command("a", &r.command_id).await.unwrap();
        assert_eq!(c.state, DeliveryState::Conflict);
        assert_eq!(c.attempt_count, 0);
        assert_eq!(server.join().unwrap().len(), 1);
        store.close().await.unwrap();
    }
}
#[tokio::test]
async fn workflow_lost_response_cold_restart_only_reads_and_confirms_convergence() {
    let (dir, store, account) = setup().await;
    let r = request(&store, WorkflowState::Closed).await;
    store.submit_workflow_state(r.clone()).await.unwrap();
    let (policy, server) = server(vec![Some(response("open")), None, Some(response("closed"))]);
    let dispatch = claim(&store, &account, &policy, &r.command_id)
        .await
        .request
        .unwrap();
    let report = policy.dispatch(&token(), dispatch).await;
    assert!(matches!(report.outcome, DeliveryOutcome::Unknown));
    store.close().await.unwrap();
    let store = Store::open(dir.path().join("text.db")).await.unwrap();
    let sending = store.delivery_command("a", &r.command_id).await.unwrap();
    store.recover_delivery(&sending, &now()).await.unwrap();
    let c = store.delivery_command("a", &r.command_id).await.unwrap();
    let (req, _) = store
        .claim_reconciliation(&c, &account, &policy, &time())
        .await
        .unwrap();
    let req = req.unwrap();
    let report = policy.reconcile(&token(), req.clone()).await.unwrap();
    complete(&store, &account, &policy, &req.command, None, &report).await;
    let c = store.delivery_command("a", &r.command_id).await.unwrap();
    assert_eq!(c.state, DeliveryState::Confirmed);
    assert_eq!(c.attempt_count, 1);
    assert_eq!(
        server
            .join()
            .unwrap()
            .iter()
            .filter(|r| r.starts_with("PATCH "))
            .count(),
        1
    );
    store.close().await.unwrap();
}
#[tokio::test]
async fn workflow_held_completion_cannot_cross_same_epoch_body_refresh() {
    let (_dir, store, account) = setup().await;
    let r = request(&store, WorkflowState::Closed).await;
    store.submit_workflow_state(r.clone()).await.unwrap();
    let (policy, server) = server(vec![Some(response("open")), Some(response("closed"))]);
    let dispatch = claim(&store, &account, &policy, &r.command_id)
        .await
        .request
        .unwrap();
    let report = policy.dispatch(&token(), dispatch.clone()).await;
    observed(&store, &account, "open", HEAD).await;
    assert!(
        store
            .complete_delivery(
                &dispatch.command,
                &policy,
                super::delivery::DeliveryCompletion {
                    account: &account,
                    attempt: Some(1),
                    report: &report,
                    now: &now(),
                    next: &now()
                }
            )
            .await
            .is_err()
    );
    assert_eq!(
        store
            .delivery_command("a", &r.command_id)
            .await
            .unwrap()
            .state,
        DeliveryState::Sending
    );
    server.join().unwrap();
    store.close().await.unwrap();
}
#[tokio::test]
async fn workflow_success_quota_and_mutation_auth_remain_independent() {
    let (_dir, store, account) = setup().await;
    let r = request(&store, WorkflowState::Closed).await;
    store.submit_workflow_state(r.clone()).await.unwrap();
    let (policy, server) = server_headers(vec![
        (200, Some(response("open")), "Retry-After: 120\r\n".into()),
        (401, Some(json!({"message":"denied"})), String::new()),
    ]);
    let (req, prep) = prepare(&store, &account, &policy, &r.command_id).await;
    assert_eq!(prep.account_cooldown_seconds, Some(120));
    let dispatch = store
        .claim_delivery(&req.command, &account, &policy, &prep.bytes, &time())
        .await
        .unwrap()
        .request
        .unwrap();
    let report = policy.dispatch(&token(), dispatch).await;
    assert!(matches!(report.outcome, DeliveryOutcome::Unknown));
    assert_eq!(
        report.provider_error.unwrap().kind,
        crate::providers::ProviderErrorKind::Authentication
    );
    assert_eq!(server.join().unwrap().len(), 2);
    store.close().await.unwrap();
}

#[tokio::test]
async fn workflow_pending_disconnected_snapshot_keeps_a_valid_local_shape_after_reopen() {
    let (dir, store, _) = setup().await;
    let r = request(&store, WorkflowState::Closed).await;
    store.submit_workflow_state(r).await.unwrap();
    store.disconnect("a").await.unwrap();
    let snapshot = store.workflow_state_snapshot("a", "pull").await.unwrap();
    // Epoch fencing may retire the active overlay, but must never return a reason that contradicts pending metadata.
    assert_eq!(
        snapshot.reason == Some(WorkflowStateReason::PendingIntent),
        snapshot.pending_intent.is_some()
    );
    assert!(snapshot.context.is_none());
    store.close().await.unwrap();
    let store = Store::open(dir.path().join("text.db")).await.unwrap();
    let cold = store.workflow_state_snapshot("a", "pull").await.unwrap();
    assert_eq!(cold, snapshot);
    store.close().await.unwrap();
}
#[tokio::test]
async fn workflow_missing_merged_evidence_or_wrong_identity_never_authorizes_patch() {
    for case in 0..3 {
        let (_dir, store, account) = setup().await;
        let r = request(&store, WorkflowState::Closed).await;
        store.submit_workflow_state(r.clone()).await.unwrap();
        let mut remote = response("open");
        match case {
            0 => {
                remote.as_object_mut().unwrap().remove("merged");
            }
            1 => remote["id"] = json!(7),
            _ => remote["base"]["repo"]["id"] = json!(99),
        };
        let (policy, server) = server(vec![Some(remote)]);
        let c = store.delivery_command("a", &r.command_id).await.unwrap();
        let (req, _) = store
            .claim_preparation(&c, &account, &policy, &time())
            .await
            .unwrap();
        assert!(policy.prepare(&token(), &req.unwrap()).await.is_err());
        assert_eq!(
            store
                .delivery_command("a", &r.command_id)
                .await
                .unwrap()
                .attempt_count,
            0
        );
        assert_eq!(server.join().unwrap().len(), 1);
        store.close().await.unwrap();
    }
}
#[tokio::test]
async fn workflow_restore_quarantine_preserves_intent_and_never_gains_dispatch_permission() {
    use crate::recovery::{RecoverySession, RestoreChoice};
    let (dir, store, _) = setup().await;
    let r = request(&store, WorkflowState::Closed).await;
    store.submit_workflow_state(r.clone()).await.unwrap();
    let bytes = store
        .delivery_command("a", &r.command_id)
        .await
        .unwrap()
        .payload;
    let backup = dir.path().join("backup.db");
    store.backup_to(&backup).await.unwrap();
    store.close().await.unwrap();
    let session = RecoverySession::prepare(dir.path().join("text.db"), &backup)
        .await
        .unwrap();
    let id = session.preview().confirmation_id.clone();
    session
        .confirm(&id, RestoreChoice::ReplaceCurrentData)
        .unwrap();
    let store = Store::open(dir.path().join("text.db")).await.unwrap();
    let mut account = store.account("a").await.unwrap();
    account.state = AccountState::Active;
    account.authorization_epoch =
        (account.authorization_epoch.parse::<u64>().unwrap() + 1).to_string();
    let account = store.upsert_account(account).await.unwrap();
    let c = store.delivery_command("a", &r.command_id).await.unwrap();
    assert_eq!(c.payload, bytes);
    assert!(c.reconcile_only());
    assert_eq!(c.attempt_count, 0);
    let policy = GithubWorkflowStatePolicy::new().unwrap();
    assert!(
        store
            .claim_preparation(&c, &account, &policy, &time())
            .await
            .is_err()
    );
    assert!(store.submit_workflow_state(r).await.is_err());
    store.close().await.unwrap();
}
#[tokio::test]
async fn workflow_recovery_only_edits_state_and_cannot_create_merge_or_noop() {
    let (_dir, store, account) = setup().await;
    let r = request(&store, WorkflowState::Closed).await;
    store.submit_workflow_state(r.clone()).await.unwrap();
    let policy = GithubWorkflowStatePolicy::new().unwrap();
    let detail = store
        .command_recovery_detail("a", &r.command_id, Some(&policy))
        .await
        .unwrap();
    assert_eq!(
        detail
            .fields
            .iter()
            .filter(|f| f.editable)
            .map(|f| f.field)
            .collect::<Vec<_>>(),
        vec![CommandReviewField::State]
    );
    let replacement = CommandRecoveryReplaceRequest {
        context: detail.context,
        action_id: Uuid::new_v4().to_string(),
        new_command_id: Uuid::new_v4().to_string(),
        fields: vec![CommandFieldResolution {
            field: CommandReviewField::State,
            choice: CommandResolutionChoice::Edited,
            value: Some("merged".into()),
        }],
    };
    assert!(
        store
            .command_recovery_replace(replacement, Some(&policy))
            .await
            .is_err()
    );
    assert_eq!(
        store
            .delivery_command("a", &r.command_id)
            .await
            .unwrap()
            .state,
        DeliveryState::Queued
    );
    observed(&store, &account, "merged", HEAD).await;
    let detail = store
        .command_recovery_detail("a", &r.command_id, Some(&policy))
        .await
        .unwrap();
    assert!(!detail.can_replace);
    store.close().await.unwrap();
}
#[tokio::test]
async fn workflow_snapshot_and_submit_bound_renderer_identifiers_before_lookup() {
    let (_dir, store, _) = setup().await;
    let r = request(&store, WorkflowState::Closed).await;
    for value in ["x".repeat(1025), "nul\0key".into()] {
        for (a, s) in [(value.as_str(), "pull"), ("a", value.as_str())] {
            assert_eq!(
                store.workflow_state_snapshot(a, s).await.unwrap_err().code,
                ErrorCode::InvalidInput
            );
        }
        let mut bad = r.clone();
        bad.context.account_id = value;
        assert_eq!(
            store.submit_workflow_state(bad).await.unwrap_err().code,
            ErrorCode::InvalidInput
        );
    }
    let mut bad = r;
    bad.context.authorization_epoch = "01".into();
    assert!(validate_request(&bad).is_err());
    store.close().await.unwrap();
}

#[tokio::test]
async fn workflow_state_is_independent_of_queued_title_intent() {
    let (_dir, store, _) = setup().await;
    let context = store
        .text_edit_snapshot("a", "pull")
        .await
        .unwrap()
        .context
        .unwrap();
    let text = store
        .submit_text_edit(TextEditRequest {
            context,
            command_id: Uuid::new_v4().to_string(),
            accept_best_effort: true,
            title: Some("My title".into()),
            body: None,
        })
        .await
        .unwrap();
    let snapshot = store.workflow_state_snapshot("a", "pull").await.unwrap();
    assert_eq!(snapshot.availability, WorkflowStateAvailability::Available);
    assert!(snapshot.pending_intent.is_none());
    let state = store
        .submit_workflow_state(request(&store, WorkflowState::Closed).await)
        .await
        .unwrap();
    let item = store.item("a", "pull").await.unwrap();
    assert_eq!(item.pending_intent.unwrap().commands.len(), 2);
    let raw = item.item.unwrap();
    assert_eq!(raw.title, "My title");
    assert_eq!(raw.state, "closed");
    let pending = store
        .workflow_state_snapshot("a", "pull")
        .await
        .unwrap()
        .pending_intent
        .unwrap();
    assert_eq!(pending.commands.len(), 1);
    assert_eq!(pending.commands[0].command_id, state.command_id);
    assert_ne!(pending.commands[0].command_id, text.command_id);
    store.close().await.unwrap();
}

#[tokio::test]
async fn workflow_issue_reopen_uses_numeric_issue_route_only() {
    let (_dir, store, account) = setup().await;
    let mut issue = store.item("a", "pull").await.unwrap().item.unwrap();
    issue.id = "issue".into();
    issue.kind = RemoteItemKind::Issue;
    issue.provider_id = "55".into();
    issue.number = Some("2".into());
    issue.head_oid = None;
    let run = store.begin_sync("a", "1", "repo:repo:issue").await.unwrap();
    store
        .apply_page(PageCommit {
            account_id: "a".into(),
            authorization_epoch: "1".into(),
            scope: "repo:repo:issue".into(),
            run_id: run,
            repositories: vec![],
            items: vec![issue],
            endpoint_aliases: vec![],
            next_cursor: None,
            etag: None,
            last_modified: None,
            not_modified: false,
            complete: true,
            observed_at: now(),
        })
        .await
        .unwrap();
    let lease = store
        .begin_detail("a", "1", "issue", DetailFacet::Body)
        .await
        .unwrap();
    let mut commit = fixtures::from_lease(&account, DetailFacet::Body, lease);
    commit.subject_id = "issue".into();
    commit.source.source = "github/issue-detail/2026-03-10".into();
    commit.source.provider_updated_at = Some("2026-10-07T22:00:00Z".into());
    commit.metadata = Some(ResourceMetadataObservation {
        kind: RemoteItemKind::Issue,
        values: ResourceMetadataValues {
            title: Some("Issue title".into()),
            state: Some("closed".into()),
            updated_at: commit.source.provider_updated_at.clone(),
            ..Default::default()
        },
        fields: [
            MetadataField::Title,
            MetadataField::State,
            MetadataField::UpdatedAt,
        ]
        .into_iter()
        .map(|field| MetadataObservedField {
            field,
            state: DetailValueState::Known,
        })
        .collect(),
        source: MetadataSource {
            source: commit.source.source.clone(),
            adapter_version: 1,
            provider_updated_at: commit.source.provider_updated_at.clone(),
            observed_at: commit.source.observed_at.clone(),
        },
    });
    commit.subject_binding = Some(DetailSubjectBinding {
        repository_id: "repo".into(),
        repository_provider_id: "1".into(),
        provider_id: "55".into(),
        number: Some("2".into()),
        kind: RemoteItemKind::Issue,
        head_oid: None,
    });
    store.apply_detail(commit).await.unwrap();

    let context = store
        .workflow_state_snapshot("a", "issue")
        .await
        .unwrap()
        .context
        .unwrap();
    let r = WorkflowStateRequest {
        context,
        command_id: Uuid::new_v4().to_string(),
        desired_state: WorkflowState::Open,
        accept_best_effort: true,
    };
    store.submit_workflow_state(r.clone()).await.unwrap();
    let response = |state: &str| json!({"id":55,"number":2,"url":"https://api.github.com/repos/owner/project/issues/2","repository_url":"https://api.github.com/repos/owner/project","title":"Issue title","body":"Untouched issue text","state":state,"updated_at":"2026-10-07T23:00:00Z"});
    let (policy, server) = server(vec![Some(response("closed")), Some(response("open"))]);
    let dispatch = claim(&store, &account, &policy, &r.command_id)
        .await
        .request
        .unwrap();
    let report = policy.dispatch(&token(), dispatch.clone()).await;
    complete(
        &store,
        &account,
        &policy,
        &dispatch.command,
        Some(1),
        &report,
    )
    .await;
    let requests = server.join().unwrap();
    assert!(requests[1].starts_with("PATCH /repositories/1/issues/2 "));
    assert!(requests[1].ends_with("{\"state\":\"open\"}"));
    let item = store.item("a", "issue").await.unwrap().item.unwrap();
    assert_eq!(item.state, "open");
    assert_eq!(item.body.as_deref(), Some("Untouched issue text"));
    store.close().await.unwrap();
}
#[tokio::test]
async fn workflow_pending_filtered_counts_and_confirmed_feed_fence_agree() {
    let (_dir, store, account) = setup().await;
    let mut held_item = store.item("a", "pull").await.unwrap().item.unwrap();
    held_item.updated_at = "2026-10-07T23:00:00Z".into();
    let r = request(&store, WorkflowState::Closed).await;
    store.submit_workflow_state(r.clone()).await.unwrap();
    for (state, count) in [("open", 0), ("closed", 1)] {
        let page = store
            .query_items(ItemQuery {
                account_id: "a".into(),
                kind: RemoteItemKind::PullRequest,
                repository_id: Some("repo".into()),
                state: Some(state.into()),
                search: None,
                cursor: None,
                limit: 20,
            })
            .await
            .unwrap();
        assert_eq!(page.total_count, count);
        assert_eq!(page.items.len() as u64, count);
    }
    let checkpoint = store
        .scope_state("a", "repo:repo:pull_request")
        .await
        .unwrap()
        .unwrap();
    let (policy, server) = server(vec![Some(response("closed"))]);
    assert!(
        claim(&store, &account, &policy, &r.command_id)
            .await
            .request
            .is_none()
    );
    let held = PageCommit {
        account_id: "a".into(),
        authorization_epoch: "1".into(),
        scope: "repo:repo:pull_request".into(),
        run_id: checkpoint.run_id,
        repositories: vec![],
        items: vec![held_item],
        endpoint_aliases: vec![],
        next_cursor: None,
        etag: None,
        last_modified: None,
        not_modified: false,
        complete: true,
        observed_at: now(),
    };
    assert_eq!(
        store
            .apply_fetched_page(held, vec![], checkpoint.data_revision)
            .await
            .unwrap_err()
            .code,
        ErrorCode::StaleView
    );
    assert_eq!(
        store.item("a", "pull").await.unwrap().item.unwrap().state,
        "closed"
    );
    server.join().unwrap();
    store.close().await.unwrap();
}
#[tokio::test]
async fn workflow_admission_failure_rolls_back_command_effect_receipt_and_revision() {
    let (_dir, store, _) = setup().await;
    let r = request(&store, WorkflowState::Closed).await;
    let revision = store.revision().await.unwrap();
    {
        let mut writer = store.inner.writer.acquire().await.unwrap();
        sqlx::query("CREATE TEMP TRIGGER workflow_fault BEFORE INSERT ON command_effects BEGIN SELECT RAISE(ABORT,'fixture rollback'); END").execute(&mut *writer).await.unwrap();
    }
    assert!(store.submit_workflow_state(r.clone()).await.is_err());
    assert_eq!(store.revision().await.unwrap(), revision);
    assert!(store.delivery_command("a", &r.command_id).await.is_err());
    assert!(
        store
            .item("a", "pull")
            .await
            .unwrap()
            .pending_intent
            .is_none()
    );
    {
        let mut writer = store.inner.writer.acquire().await.unwrap();
        sqlx::query("DROP TRIGGER workflow_fault")
            .execute(&mut *writer)
            .await
            .unwrap();
    }
    assert!(!store.submit_workflow_state(r).await.unwrap().duplicate);
    store.close().await.unwrap();
}

#[tokio::test]
async fn workflow_state_does_not_require_or_seal_provider_description() {
    for omitted in [true, false] {
        let (_dir, store, account) = setup().await;
        observed_body(
            &store,
            &account,
            "open",
            HEAD,
            DetailValue {
                state: if omitted {
                    DetailValueState::Omitted
                } else {
                    DetailValueState::Oversized
                },
                text: None,
            },
        )
        .await;
        let r = request(&store, WorkflowState::Closed).await;
        store.submit_workflow_state(r.clone()).await.unwrap();
        let sealed = store
            .delivery_command("a", &r.command_id)
            .await
            .unwrap()
            .payload;
        assert!(!String::from_utf8_lossy(&sealed).contains("Base body"));
        assert!(!String::from_utf8_lossy(&sealed).contains("Base title"));
        assert!(sealed.len() < 1024);
        let remote = |state: &str| {
            let mut v = response(state);
            v.as_object_mut().unwrap().remove("title");
            if omitted {
                v.as_object_mut().unwrap().remove("body");
            } else {
                v["body"] = "x".repeat(32_768).into();
            }
            v
        };
        let (policy, server) = server(vec![Some(remote("open")), Some(remote("closed"))]);
        let dispatch = claim(&store, &account, &policy, &r.command_id)
            .await
            .request
            .unwrap();
        let report = policy.dispatch(&token(), dispatch.clone()).await;
        assert!(matches!(report.outcome, DeliveryOutcome::Confirmed(_)));
        complete(
            &store,
            &account,
            &policy,
            &dispatch.command,
            Some(1),
            &report,
        )
        .await;
        let item = store.item("a", "pull").await.unwrap();
        assert!(item.pending_intent.is_none());
        let item = item.item.unwrap();
        assert_eq!(item.state, "closed");
        assert_eq!(item.title, "Base title");
        assert_eq!(server.join().unwrap().len(), 2);
        store.close().await.unwrap();
    }
}
