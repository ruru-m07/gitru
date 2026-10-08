use super::*;
use crate::delivery::*;
use crate::providers::github::text_edits::GithubTextEditPolicy;
use crate::runtime::detail_tests::fixtures;
use crate::text_edits::native::*;
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
fn response(title: &str, body: Option<&str>) -> serde_json::Value {
    json!({"id":9007199254740997_u64,"number":67,"url":"https://api.github.com/repos/owner/project/pulls/67","html_url":"https://github.com/owner/project/pull/67","title":title,"body":body,"state":"open","merged":false,"updated_at":"2026-10-07T23:00:00Z","head":{"ref":"feature","sha":HEAD,"repo":null},"base":{"ref":"main","sha":"b".repeat(40),"repo":{"id":1,"full_name":"owner/project","html_url":"https://github.com/owner/project"}}})
}
fn server(
    responses: Vec<Option<serde_json::Value>>,
) -> (GithubTextEditPolicy, std::thread::JoinHandle<Vec<String>>) {
    server_headers(
        responses
            .into_iter()
            .map(|v| (200, v, String::new()))
            .collect(),
    )
}
fn server_headers(
    responses: Vec<(u16, Option<serde_json::Value>, String)>,
) -> (GithubTextEditPolicy, std::thread::JoinHandle<Vec<String>>) {
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
    (GithubTextEditPolicy::for_test_base(url), handle)
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
    observed(&store, &account, "Base title", Some("Base body")).await;
    (dir, store, account)
}
async fn observed(store: &Store, account: &RemoteAccount, title: &str, body: Option<&str>) {
    observed_head(store, account, title, body, HEAD).await;
}
async fn observed_head(
    store: &Store,
    account: &RemoteAccount,
    title: &str,
    body: Option<&str>,
    head: &str,
) {
    let mut commit = fixtures::commit(store, account, DetailFacet::Body).await;
    commit.body = fixtures::known(body);
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
            title: Some(title.into()),
            state: Some("open".into()),
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
async fn request(store: &Store, title: Option<&str>, body: Option<&str>) -> TextEditRequest {
    let snapshot = store.text_edit_snapshot("a", "pull").await.unwrap();
    assert_eq!(snapshot.reason, None);
    TextEditRequest {
        context: snapshot.context.unwrap(),
        command_id: Uuid::new_v4().to_string(),
        accept_best_effort: true,
        title: title.map(str::to_string),
        body: body.map(str::to_string),
    }
}
async fn prepare(
    store: &Store,
    account: &RemoteAccount,
    policy: &GithubTextEditPolicy,
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
#[tokio::test]
async fn text_edit_offline_admission_preserves_bytes_and_exact_retry_after_reopen() {
    let (dir, store, _) = setup().await;
    let request = request(&store, Some("My 雪 title"), Some("")).await;
    let receipt = store.submit_text_edit(request.clone()).await.unwrap();
    assert!(!receipt.duplicate);
    let item = store.item("a", "pull").await.unwrap();
    assert_eq!(item.item.unwrap().title, "My 雪 title");
    assert!(item.pending_intent.is_some());
    assert_eq!(
        store.text_edit_snapshot("a", "pull").await.unwrap().reason,
        Some(TextEditReason::PendingIntent)
    );
    let payload = store
        .delivery_command("a", &receipt.command_id)
        .await
        .unwrap()
        .payload;
    store.close().await.unwrap();
    let store = Store::open(dir.path().join("text.db")).await.unwrap();
    let retry = store.submit_text_edit(request.clone()).await.unwrap();
    assert!(retry.duplicate);
    assert_eq!(retry.admitted_revision, receipt.admitted_revision);
    assert_eq!(
        store
            .delivery_command("a", &receipt.command_id)
            .await
            .unwrap()
            .payload,
        payload
    );
    let mut bad = request;
    bad.title = Some("different".into());
    assert!(store.submit_text_edit(bad).await.is_err());
    store.close().await.unwrap();
}
#[tokio::test]
async fn text_edit_requires_ack_known_base_and_current_snapshot() {
    let (_dir, store, account) = setup().await;
    let mut r = request(&store, Some("mine"), None).await;
    r.accept_best_effort = false;
    assert!(store.submit_text_edit(r.clone()).await.is_err());
    r.accept_best_effort = true;
    observed(&store, &account, "Changed title", Some("Base body")).await;
    assert_eq!(
        store.submit_text_edit(r).await.unwrap_err().code,
        ErrorCode::StaleView
    );
    let mut r = request(&store, Some("mine"), None).await;
    r.body = Some("x".repeat(MAX_BODY + 1));
    assert!(store.submit_text_edit(r).await.is_err());
    let writer = store.inner.writer.acquire().await.unwrap();
    drop(writer);
    store.close().await.unwrap();
}
#[tokio::test]
async fn text_edit_preflight_conflict_saves_remote_and_never_attempts_patch() {
    let (_dir, store, account) = setup().await;
    let r = request(&store, Some("mine"), None).await;
    store.submit_text_edit(r.clone()).await.unwrap();
    let (policy, server) = server(vec![Some(response(
        "their title",
        Some("their independent body"),
    ))]);
    let (req, prep) = prepare(&store, &account, &policy, &r.command_id).await;
    let claim = store
        .claim_delivery(&req.command, &account, &policy, &prep.bytes, &time())
        .await
        .unwrap();
    assert!(claim.request.is_none());
    let command = store.delivery_command("a", &r.command_id).await.unwrap();
    assert_eq!(command.state, DeliveryState::Conflict);
    assert_eq!(command.attempt_count, 0);
    assert_eq!(server.join().unwrap().len(), 1);
    assert_eq!(
        store.item("a", "pull").await.unwrap().item.unwrap().title,
        "mine"
    );
    let detail = store
        .command_recovery_detail("a", &r.command_id, Some(&policy))
        .await
        .unwrap();
    assert!(
        detail
            .fields
            .iter()
            .any(|f| f.field == CommandReviewField::Title
                && f.remote.value.as_deref() == Some("their title"))
    );
    store.close().await.unwrap();
}
#[tokio::test]
async fn text_edit_converged_preflight_finalizes_without_mutation_attempt() {
    let (_dir, store, account) = setup().await;
    let r = request(&store, Some("mine"), Some("")).await;
    store.submit_text_edit(r.clone()).await.unwrap();
    let (policy, server) = server(vec![Some(response("mine", None))]);
    let (req, prep) = prepare(&store, &account, &policy, &r.command_id).await;
    let claim = store
        .claim_delivery(&req.command, &account, &policy, &prep.bytes, &time())
        .await
        .unwrap();
    assert!(claim.request.is_none());
    let command = store.delivery_command("a", &r.command_id).await.unwrap();
    assert_eq!(command.state, DeliveryState::Confirmed);
    assert_eq!(command.attempt_count, 0);
    let item = store.item("a", "pull").await.unwrap();
    assert!(item.pending_intent.is_none());
    assert_eq!(item.item.unwrap().body, None);
    assert_eq!(server.join().unwrap().len(), 1);
    store.close().await.unwrap();
}
#[tokio::test]
async fn text_edit_attempt_precedes_patch_and_untouched_remote_body_is_preserved() {
    let (_dir, store, account) = setup().await;
    let r = request(&store, Some("mine"), None).await;
    store.submit_text_edit(r.clone()).await.unwrap();
    let (policy, server) = server(vec![
        Some(response("Base title", Some("their independent body"))),
        Some(response("mine", Some("their independent body"))),
    ]);
    let (req, prep) = prepare(&store, &account, &policy, &r.command_id).await;
    let claim = store
        .claim_delivery(&req.command, &account, &policy, &prep.bytes, &time())
        .await
        .unwrap();
    let dispatch = claim.request.unwrap();
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
    store
        .complete_delivery(
            &dispatch.command,
            &policy,
            super::delivery::DeliveryCompletion {
                account: &account,
                attempt: Some(dispatch.attempt),
                report: &report,
                now: &now(),
                next: &now(),
            },
        )
        .await
        .unwrap();
    let item = store.item("a", "pull").await.unwrap();
    assert!(item.pending_intent.is_none());
    assert_eq!(
        item.item.unwrap().body.as_deref(),
        Some("their independent body")
    );
    let requests = server.join().unwrap();
    assert!(requests[0].starts_with("GET "));
    assert!(requests[1].starts_with("PATCH "));
    assert!(requests[1].ends_with("{\"title\":\"mine\"}"));
    store.close().await.unwrap();
}
#[tokio::test]
async fn text_edit_lost_response_reopens_unknown_and_reconciles_without_second_patch() {
    let (dir, store, account) = setup().await;
    let r = request(&store, Some("mine"), None).await;
    store.submit_text_edit(r.clone()).await.unwrap();
    let (policy, server) = server(vec![
        Some(response("Base title", Some("Base body"))),
        None,
        Some(response("mine", Some("Base body"))),
    ]);
    let (req, prep) = prepare(&store, &account, &policy, &r.command_id).await;
    let dispatch = store
        .claim_delivery(&req.command, &account, &policy, &prep.bytes, &time())
        .await
        .unwrap()
        .request
        .unwrap();
    let report = policy.dispatch(&token(), dispatch.clone()).await;
    assert!(matches!(report.outcome, DeliveryOutcome::Unknown));
    // Deliberately lose the local completion as well: cold start owns recovery.
    store.close().await.unwrap();
    let store = Store::open(dir.path().join("text.db")).await.unwrap();
    let sending = store.delivery_command("a", &r.command_id).await.unwrap();
    store.recover_delivery(&sending, &now()).await.unwrap();
    let command = store.delivery_command("a", &r.command_id).await.unwrap();
    let (req, _) = store
        .claim_reconciliation(&command, &account, &policy, &time())
        .await
        .unwrap();
    let req = req.unwrap();
    let report = policy.reconcile(&token(), req.clone()).await.unwrap();
    store
        .complete_delivery(
            &req.command,
            &policy,
            super::delivery::DeliveryCompletion {
                account: &account,
                attempt: None,
                report: &report,
                now: &now(),
                next: &now(),
            },
        )
        .await
        .unwrap();
    assert_eq!(
        store
            .delivery_command("a", &r.command_id)
            .await
            .unwrap()
            .state,
        DeliveryState::Confirmed
    );
    let requests = server.join().unwrap();
    assert_eq!(
        requests.iter().filter(|r| r.starts_with("PATCH ")).count(),
        1
    );
    assert_eq!(requests.len(), 3);
    store.close().await.unwrap();
}
#[tokio::test]
async fn text_edit_native_drift_after_preflight_rejects_claim_before_attempt() {
    let (_dir, store, account) = setup().await;
    let r = request(&store, Some("mine"), None).await;
    store.submit_text_edit(r.clone()).await.unwrap();
    let (policy, server) = server(vec![Some(response("Base title", Some("Base body")))]);
    let (req, prep) = prepare(&store, &account, &policy, &r.command_id).await;
    observed(&store, &account, "another cached title", Some("Base body")).await;
    assert_eq!(
        store
            .claim_delivery(&req.command, &account, &policy, &prep.bytes, &time())
            .await
            .err()
            .unwrap()
            .code,
        ErrorCode::StaleView
    );
    assert_eq!(
        store
            .delivery_command("a", &r.command_id)
            .await
            .unwrap()
            .attempt_count,
        0
    );
    server.join().unwrap();
    store.close().await.unwrap();
}

#[tokio::test]
async fn text_edit_unknown_nonconvergence_preserves_intent_without_safe_retry() {
    let (_dir, store, account) = setup().await;
    let r = request(&store, Some("mine"), None).await;
    store.submit_text_edit(r.clone()).await.unwrap();
    let (policy, server) = server(vec![
        Some(response("Base title", Some("Base body"))),
        None,
        Some(response("their later title", Some("Base body"))),
    ]);
    let (req, prep) = prepare(&store, &account, &policy, &r.command_id).await;
    let dispatch = store
        .claim_delivery(&req.command, &account, &policy, &prep.bytes, &time())
        .await
        .unwrap()
        .request
        .unwrap();
    let report = policy.dispatch(&token(), dispatch.clone()).await;
    store
        .complete_delivery(
            &dispatch.command,
            &policy,
            super::delivery::DeliveryCompletion {
                account: &account,
                attempt: Some(1),
                report: &report,
                now: &now(),
                next: &now(),
            },
        )
        .await
        .unwrap();
    let command = store.delivery_command("a", &r.command_id).await.unwrap();
    let (req, _) = store
        .claim_reconciliation(&command, &account, &policy, &time())
        .await
        .unwrap();
    let req = req.unwrap();
    let report = policy.reconcile(&token(), req.clone()).await.unwrap();
    assert!(matches!(report.outcome, DeliveryOutcome::Unknown));
    store
        .complete_delivery(
            &req.command,
            &policy,
            super::delivery::DeliveryCompletion {
                account: &account,
                attempt: None,
                report: &report,
                now: &now(),
                next: &now(),
            },
        )
        .await
        .unwrap();
    let command = store.delivery_command("a", &r.command_id).await.unwrap();
    assert_eq!(command.state, DeliveryState::Unknown);
    assert_eq!(command.attempt_count, 1);
    assert!(
        store
            .item("a", "pull")
            .await
            .unwrap()
            .pending_intent
            .is_some()
    );
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
async fn text_edit_replacement_only_authored_fields_and_preserves_independent_body() {
    let (_dir, store, account) = setup().await;
    let r = request(&store, Some("mine"), None).await;
    store.submit_text_edit(r.clone()).await.unwrap();
    observed(&store, &account, "their title", Some("their body")).await;
    let policy = GithubTextEditPolicy::new().unwrap();
    let detail = store
        .command_recovery_detail("a", &r.command_id, Some(&policy))
        .await
        .unwrap();
    assert_eq!(detail.fields.iter().filter(|f| f.editable).count(), 1);
    let replacement = CommandRecoveryReplaceRequest {
        context: detail.context,
        action_id: Uuid::new_v4().to_string(),
        new_command_id: Uuid::new_v4().to_string(),
        fields: vec![CommandFieldResolution {
            field: CommandReviewField::Title,
            choice: CommandResolutionChoice::Edited,
            value: Some("resolved".into()),
        }],
    };
    let receipt = store
        .command_recovery_replace(replacement, Some(&policy))
        .await
        .unwrap();
    let cmd = store
        .delivery_command("a", receipt.replacement_id.as_deref().unwrap())
        .await
        .unwrap();
    let payload = decode_payload(&cmd).unwrap();
    assert_eq!(payload.request.body, None);
    assert_eq!(payload.base.body.as_deref(), Some("their body"));
    assert_eq!(payload.request.title.as_deref(), Some("resolved"));
    store.close().await.unwrap();
}
#[tokio::test]
async fn text_edit_backup_restore_quarantine_never_dispatches_after_reauthorization() {
    use crate::recovery::{RecoverySession, RestoreChoice};
    let (dir, store, account) = setup().await;
    let r = request(&store, Some("mine"), None).await;
    store.submit_text_edit(r.clone()).await.unwrap();
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
    let mut active = store.account("a").await.unwrap();
    active.state = AccountState::Active;
    active.authorization_epoch =
        (active.authorization_epoch.parse::<u64>().unwrap() + 1).to_string();
    let active = store.upsert_account(active).await.unwrap();
    assert_ne!(active.authorization_epoch, account.authorization_epoch);
    let command = store.delivery_command("a", &r.command_id).await.unwrap();
    assert_eq!(command.payload, bytes);
    assert!(command.reconcile_only());
    assert_eq!(command.attempt_count, 0);
    let policy = GithubTextEditPolicy::new().unwrap();
    assert_eq!(
        store
            .claim_preparation(&command, &active, &policy, &time())
            .await
            .unwrap_err()
            .code,
        ErrorCode::StaleView
    );
    assert!(store.submit_text_edit(r).await.is_err());
    store.close().await.unwrap();
}
struct OversizedContext;
#[async_trait::async_trait]
impl CommandDeliveryPolicy for OversizedContext {
    fn operation_kind(&self) -> &'static str {
        OPERATION
    }
    fn payload_version(&self) -> u32 {
        1
    }
    async fn prepare_context_in(
        &self,
        _: &mut Transaction<'_, Sqlite>,
        _: &DeliveryCommand,
        _: &RemoteAccount,
    ) -> Result<Vec<u8>> {
        Ok(vec![1; MAX_EVIDENCE_BYTES + 1])
    }
    async fn validate_claim(
        &self,
        _: &mut Transaction<'_, Sqlite>,
        _: &DeliveryCommand,
        _: &RemoteAccount,
        _: &[u8],
    ) -> Result<ClaimDecision> {
        unreachable!()
    }
    fn validate_evidence(
        &self,
        _: &DeliveryCommand,
        _: EvidencePurpose,
        _: &OperationEvidence,
    ) -> bool {
        false
    }
    async fn dispatch(
        &self,
        _: &crate::credentials::SecretToken,
        _: DispatchRequest,
    ) -> DeliveryReport {
        unreachable!()
    }
}
#[tokio::test]
async fn text_edit_oversized_native_context_rolls_back_claim_generation_budget_and_revision() {
    let (_dir, store, account) = setup().await;
    let r = request(&store, Some("mine"), None).await;
    store.submit_text_edit(r.clone()).await.unwrap();
    let before = store.delivery_command("a", &r.command_id).await.unwrap();
    let revision = store.revision().await.unwrap();
    assert_eq!(
        store
            .claim_preparation(&before, &account, &OversizedContext, &time())
            .await
            .unwrap_err()
            .code,
        ErrorCode::InvalidInput
    );
    let after = store.delivery_command("a", &r.command_id).await.unwrap();
    assert_eq!(after.generation, before.generation);
    assert_eq!(after.reconciliation_count, before.reconciliation_count);
    assert_eq!(store.revision().await.unwrap(), revision);
    store.close().await.unwrap();
}

#[tokio::test]
async fn text_edit_issue_uses_issue_endpoint_and_explicit_empty_string_body() {
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
            state: Some("open".into()),
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
    let snapshot = store.text_edit_snapshot("a", "issue").await.unwrap();
    let r = TextEditRequest {
        context: snapshot.context.unwrap(),
        command_id: Uuid::new_v4().to_string(),
        accept_best_effort: true,
        title: None,
        body: Some("".into()),
    };
    store.submit_text_edit(r.clone()).await.unwrap();
    let json = |body: Option<&str>| json!({"id":55,"number":2,"url":"https://api.github.com/repos/owner/project/issues/2","repository_url":"https://api.github.com/repos/owner/project","title":"Issue title","body":body,"state":"open","updated_at":"2026-10-07T23:00:00Z"});
    let (policy, server) = server(vec![
        Some(json(Some("saved authoritative body"))),
        Some(json(None)),
    ]);
    let (req, prep) = prepare(&store, &account, &policy, &r.command_id).await;
    let dispatch = store
        .claim_delivery(&req.command, &account, &policy, &prep.bytes, &time())
        .await
        .unwrap()
        .request
        .unwrap();
    let report = policy.dispatch(&token(), dispatch.clone()).await;
    store
        .complete_delivery(
            &dispatch.command,
            &policy,
            super::delivery::DeliveryCompletion {
                account: &account,
                attempt: Some(1),
                report: &report,
                now: &now(),
                next: &now(),
            },
        )
        .await
        .unwrap();
    let requests = server.join().unwrap();
    assert!(requests[1].starts_with("PATCH /repositories/1/issues/2 "));
    assert!(requests[1].ends_with("{\"body\":\"\"}"));
    assert_eq!(
        store
            .delivery_command("a", &r.command_id)
            .await
            .unwrap()
            .state,
        DeliveryState::Confirmed
    );
    assert_eq!(
        store.item("a", "issue").await.unwrap().item.unwrap().body,
        None
    );
    store.close().await.unwrap();
}

#[tokio::test]
async fn text_edit_wrong_native_identity_never_authorizes_a_patch() {
    let (_dir, store, account) = setup().await;
    let r = request(&store, Some("mine"), None).await;
    store.submit_text_edit(r.clone()).await.unwrap();
    let mut wrong = response("Base title", Some("Base body"));
    wrong["id"] = json!(2);
    let (policy, server) = server(vec![Some(wrong)]);
    let command = store.delivery_command("a", &r.command_id).await.unwrap();
    let (request, _) = store
        .claim_preparation(&command, &account, &policy, &time())
        .await
        .unwrap();
    let error = policy
        .prepare(&token(), &request.unwrap())
        .await
        .err()
        .unwrap();
    assert_eq!(
        error.kind,
        crate::providers::ProviderErrorKind::InvalidResponse
    );
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
#[tokio::test]
async fn text_edit_remote_head_change_is_a_preflight_conflict_even_if_title_converged() {
    let (_dir, store, account) = setup().await;
    let r = request(&store, Some("mine"), None).await;
    store.submit_text_edit(r.clone()).await.unwrap();
    let mut changed = response("mine", Some("Base body"));
    changed["head"]["sha"] = json!("c".repeat(40));
    let (policy, server) = server(vec![Some(changed)]);
    let (req, prep) = prepare(&store, &account, &policy, &r.command_id).await;
    let claim = store
        .claim_delivery(&req.command, &account, &policy, &prep.bytes, &time())
        .await
        .unwrap();
    assert!(claim.request.is_none());
    let command = store.delivery_command("a", &r.command_id).await.unwrap();
    assert_eq!(command.state, DeliveryState::Conflict);
    assert_eq!(command.attempt_count, 0);
    server.join().unwrap();
    store.close().await.unwrap();
}

#[tokio::test]
async fn text_edit_post_attempt_convergence_uses_current_head_without_rewriting_original_guard() {
    let (_dir, store, account) = setup().await;
    let r = request(&store, Some("mine"), None).await;
    store.submit_text_edit(r.clone()).await.unwrap();
    let new_head = "c".repeat(40);
    let mut converged = response("mine", Some("Base body"));
    converged["head"]["sha"] = json!(new_head);
    let (policy, server) = server(vec![
        Some(response("Base title", Some("Base body"))),
        None,
        Some(converged),
    ]);
    let (req, prep) = prepare(&store, &account, &policy, &r.command_id).await;
    let dispatch = store
        .claim_delivery(&req.command, &account, &policy, &prep.bytes, &time())
        .await
        .unwrap()
        .request
        .unwrap();
    let report = policy.dispatch(&token(), dispatch.clone()).await;
    store
        .complete_delivery(
            &dispatch.command,
            &policy,
            super::delivery::DeliveryCompletion {
                account: &account,
                attempt: Some(1),
                report: &report,
                now: &now(),
                next: &now(),
            },
        )
        .await
        .unwrap();
    let original = store
        .delivery_command("a", &r.command_id)
        .await
        .unwrap()
        .payload;
    let mut item = store.item("a", "pull").await.unwrap().item.unwrap();
    item.title = "Base title".into();
    item.head_oid = Some(new_head.clone());
    item.updated_at = "2026-10-07T22:30:00Z".into();
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
    observed_head(&store, &account, "Base title", Some("Base body"), &new_head).await;
    let command = store.delivery_command("a", &r.command_id).await.unwrap();
    let (req, _) = store
        .claim_reconciliation(&command, &account, &policy, &time())
        .await
        .unwrap();
    let req = req.unwrap();
    let frame: NativeFrame = decode_bounded(&req.native_context).unwrap();
    assert_eq!(frame.base.head.as_deref(), Some(new_head.as_str()));
    let report = policy.reconcile(&token(), req.clone()).await.unwrap();
    assert!(matches!(report.outcome, DeliveryOutcome::Confirmed(_)));
    store
        .complete_delivery(
            &req.command,
            &policy,
            super::delivery::DeliveryCompletion {
                account: &account,
                attempt: None,
                report: &report,
                now: &now(),
                next: &now(),
            },
        )
        .await
        .unwrap();
    let command = store.delivery_command("a", &r.command_id).await.unwrap();
    assert_eq!(command.state, DeliveryState::Confirmed);
    assert_eq!(command.payload, original);
    assert_eq!(
        decode_payload(&command).unwrap().base.head.as_deref(),
        Some(HEAD)
    );
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
async fn text_edit_auth_failure_is_separate_from_unknown_mutation_outcome() {
    let (_dir, store, account) = setup().await;
    let r = request(&store, Some("mine"), None).await;
    store.submit_text_edit(r.clone()).await.unwrap();
    let (policy, server) = server_headers(vec![
        (
            200,
            Some(response("Base title", Some("Base body"))),
            String::new(),
        ),
        (
            401,
            Some(json!({"message":"Bad credentials"})),
            String::new(),
        ),
    ]);
    let (req, prep) = prepare(&store, &account, &policy, &r.command_id).await;
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
async fn text_edit_successful_preflight_preserves_depleted_account_quota() {
    let (_dir, store, account) = setup().await;
    let r = request(&store, Some("mine"), None).await;
    store.submit_text_edit(r.clone()).await.unwrap();
    let headers = format!(
        "x-ratelimit-remaining: 0\r\nx-ratelimit-reset: {}\r\n",
        chrono::Utc::now().timestamp() + 120
    );
    let (policy, server) = server_headers(vec![(
        200,
        Some(response("Base title", Some("Base body"))),
        headers,
    )]);
    let (_, prep) = prepare(&store, &account, &policy, &r.command_id).await;
    assert!(
        prep.account_cooldown_seconds
            .is_some_and(|v| (100..=121).contains(&v))
    );
    assert_eq!(
        store
            .delivery_command("a", &r.command_id)
            .await
            .unwrap()
            .attempt_count,
        0
    );
    server.join().unwrap();
    store.close().await.unwrap();
}

#[tokio::test]
async fn text_edit_snapshot_rejects_oversized_and_nul_identity_before_lookup() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path().join("bounded.db")).await.unwrap();
    for invalid in ["x".repeat(1025), "nul\0key".into()] {
        for (account, subject) in [(invalid.as_str(), "pull"), ("a", invalid.as_str())] {
            assert_eq!(
                store
                    .text_edit_snapshot(account, subject)
                    .await
                    .unwrap_err()
                    .code,
                ErrorCode::InvalidInput
            );
        }
    }
    store.close().await.unwrap();
}
