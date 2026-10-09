use super::*;
use crate::delivery::*;
use crate::providers::github::pull_creation::GithubPullCreationPolicy;
use crate::runtime::detail_tests::fixtures;
use crate::*;
use serde_json::json;
use std::{
    io::{Read, Write},
    sync::{Arc, Mutex as StdMutex},
    time::{Duration, Instant},
};
const HEAD: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const BASE: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
fn owner() -> PullCreationOwner {
    PullCreationOwner::new("synthetic-owner".into(), Arc::new(|| Ok(()))).unwrap()
}
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
    crate::credentials::SecretToken::new("synthetic_creation".into()).unwrap()
}
fn key() -> PullDraftKey {
    PullDraftKey {
        account_id: "a".into(),
        draft_id: "11111111-1111-4111-8111-111111111111".into(),
        repository_id: "repo".into(),
    }
}
fn repository() -> serde_json::Value {
    json!({"id":1,"full_name":"owner/project","html_url":"https://github.com/owner/project","archived":false,"disabled":false,"permissions":{"push":true}})
}
fn branch_json(name: &str, tip: &str) -> serde_json::Value {
    json!({"name":name,"commit":{"sha":tip}})
}
fn reads(source: &str, base: &str) -> Vec<Reply> {
    vec![
        ok(repository()),
        ok(branch_json("feature/slash", source)),
        ok(branch_json("main", base)),
    ]
}
fn created(head: &str, base: &str) -> serde_json::Value {
    json!({"id":9007199254740999_u64,"number":68,"url":"https://api.github.com/repos/owner/project/pulls/68","html_url":"https://github.com/owner/project/pull/68","title":"New pull","body":"private draft body","state":"open","merged":false,"merged_at":null,"draft":true,"user":{"id":7,"login":"author"},"created_at":"2026-10-08T00:00:00Z","updated_at":"2026-10-08T00:00:00Z","head":{"ref":"feature/slash","sha":head,"repo":{"id":1,"full_name":"owner/project","html_url":"https://github.com/owner/project"}},"base":{"ref":"main","sha":base,"repo":{"id":1,"full_name":"owner/project","html_url":"https://github.com/owner/project"}}})
}
type Reply = (u16, Option<serde_json::Value>, String);
type ResponseHook = Arc<dyn Fn(usize, &Clock) + Send + Sync>;
#[derive(Clone)]
struct Clock(Arc<StdMutex<Instant>>);
impl Clock {
    fn advance(&self, seconds: u64) {
        let mut n = self.0.lock().unwrap();
        *n += Duration::from_secs(seconds);
    }
}
fn server(
    replies: Vec<Reply>,
) -> (
    Arc<GithubPullCreationPolicy>,
    Clock,
    std::thread::JoinHandle<Vec<String>>,
) {
    server_hook(replies, Arc::new(|_, _| {}))
}
fn server_hook(
    replies: Vec<Reply>,
    hook: ResponseHook,
) -> (
    Arc<GithubPullCreationPolicy>,
    Clock,
    std::thread::JoinHandle<Vec<String>>,
) {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let url = reqwest::Url::parse(&format!("http://{}/", listener.local_addr().unwrap())).unwrap();
    let clock = Clock(Arc::new(StdMutex::new(Instant::now())));
    let n = clock.0.clone();
    let policy = Arc::new(GithubPullCreationPolicy::for_test(
        url,
        Arc::new(move || *n.lock().unwrap()),
    ));
    let held_clock = clock.clone();
    let handle = std::thread::spawn(move || {
        let mut requests = vec![];
        for (status, response, headers) in replies {
            let deadline = Instant::now() + Duration::from_secs(15);
            let mut stream = loop {
                match listener.accept() {
                    Ok((s, _)) => break s,
                    Err(e)
                        if e.kind() == std::io::ErrorKind::WouldBlock
                            && Instant::now() < deadline =>
                    {
                        std::thread::sleep(Duration::from_millis(2))
                    }
                    Err(e) => panic!("bounded fixture accept: {e}"),
                }
            };
            stream.set_nonblocking(false).unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(5)))
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
            hook(requests.len(), &held_clock);
            if let Some(response) = response {
                let body = response.to_string();
                write!(stream,"HTTP/1.1 {} Fixture\r\nContent-Length: {}\r\nContent-Type: application/json\r\nConnection: close\r\n{}\r\n{}",status,body.len(),headers,body).unwrap();
            }
        }
        requests
    });
    (policy, clock, handle)
}
fn ok(v: serde_json::Value) -> Reply {
    (200, Some(v), String::new())
}
async fn setup() -> (
    tempfile::TempDir,
    Store,
    RemoteAccount,
    PullCreationLocalObservation,
) {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path().join("pull.db")).await.unwrap();
    let mut account = fixtures::account("a");
    account.actor_id = "7".into();
    let account = store.upsert_account(account).await.unwrap();
    fixtures::project(&store, &account).await;
    let query = LocalLinkQuery {
        local_repository_id: "local".into(),
        registration_proof: Some("native-registration".into()),
        remote_digest: Some("native-digest".into()),
        endpoints: vec![LocalRemoteEndpoint {
            remote_name: "origin".into(),
            direction: LinkDirection::Fetch,
            ordinal: 0,
            transport: LinkTransport::Https,
            host: "github.com".into(),
            port: 443,
            path: "owner/project.git".into(),
        }],
    };
    let snapshot = store.local_link_snapshot(query.clone()).await.unwrap();
    let candidate = snapshot.resolutions[0].candidates[0].id.clone();
    let receipt = store
        .confirm_local_link(ConfirmLocalLink {
            query: query.clone(),
            candidate_id: candidate,
            expected_authorization_view: snapshot.authorization_view,
            expected_bindings_generation: snapshot.bindings_generation,
            replace: None,
        })
        .await
        .unwrap();
    let local = PullCreationLocalObservation {
        query,
        link: LocalLinkVersion {
            id: receipt.link.id,
            generation: receipt.link.generation,
        },
        source_branch: "feature/slash".into(),
        source_oid: HEAD.into(),
    };
    let old = store.pull_draft(key()).await.unwrap();
    store
        .save_pull_draft(SavePullDraftRequest {
            key: key(),
            authorization_epoch: account.authorization_epoch.clone(),
            authorization_view: old.authorization_view,
            expected_generation: old.generation,
            values: PullDraftValues {
                title: "New pull".into(),
                body: "private draft body".into(),
                source_branch: local.source_branch.clone(),
                base_branch: "main".into(),
                local_repository_id: local.query.local_repository_id.clone(),
                link_id: local.link.id.clone(),
                link_generation: local.link.generation.clone(),
                is_draft: true,
            },
        })
        .await
        .unwrap();
    (dir, store, account, local)
}
async fn preview(
    store: &Store,
    a: &RemoteAccount,
    p: &GithubPullCreationPolicy,
    local: &PullCreationLocalObservation,
) -> (PullCreationPreview, n::Frame) {
    let snapshot = store.pull_draft(key()).await.unwrap();
    let request = PreviewPullCreationRequest {
        key: key(),
        draft_generation: snapshot.generation,
        authorization_epoch: a.authorization_epoch.clone(),
        authorization_view: snapshot.authorization_view,
    };
    let (_, frame) = store
        .pull_creation_frame(&request, &n::LocalProof::from_observation(local).unwrap())
        .await
        .unwrap();
    (
        p.preview(&token(), a, &key(), &frame, &owner())
            .await
            .unwrap()
            .0,
        frame,
    )
}
fn request(preview: PullCreationPreview) -> SubmitPullRequest {
    SubmitPullRequest {
        context: preview.context.unwrap(),
        command_id: Uuid::new_v4().to_string(),
        policy: PullCreationPolicy::BestEffortCurrentBranches,
        confirm_current_branches: true,
    }
}
async fn admit(
    store: &Store,
    a: &RemoteAccount,
    p: &GithubPullCreationPolicy,
    r: &SubmitPullRequest,
) {
    let frame = p.arm(a, r, &owner()).unwrap().unwrap();
    store
        .submit_pull(
            r.clone(),
            Some(&frame),
            || Ok(()),
            || {
                if p.live(a, r) {
                    Ok(())
                } else {
                    Err(n::invalid())
                }
            },
        )
        .await
        .unwrap();
}
async fn prepare(
    store: &Store,
    a: &RemoteAccount,
    p: &GithubPullCreationPolicy,
    r: &SubmitPullRequest,
) -> (ReconcileRequest, DeliveryPreparation) {
    let c = store.delivery_command("a", &r.command_id).await.unwrap();
    let (request, _) = store.claim_preparation(&c, a, p, &time()).await.unwrap();
    let request = request.unwrap();
    let prepared = p.prepare(&token(), &request).await.unwrap();
    (request, prepared)
}
async fn dispatch(
    store: &Store,
    a: &RemoteAccount,
    p: &GithubPullCreationPolicy,
    r: &SubmitPullRequest,
) -> (DispatchRequest, DeliveryReport) {
    let (request, prepared) = prepare(store, a, p, r).await;
    let claim = store
        .claim_delivery(&request.command, a, p, &prepared.bytes, &time())
        .await
        .unwrap();
    let request = claim.request.unwrap();
    let report = p.dispatch(&token(), request.clone()).await;
    (request, report)
}
async fn complete(
    store: &Store,
    a: &RemoteAccount,
    p: &GithubPullCreationPolicy,
    r: &DispatchRequest,
    report: &DeliveryReport,
) {
    store
        .complete_delivery(
            &r.command,
            p,
            super::super::delivery::DeliveryCompletion {
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
async fn drafts_are_local_cas_and_private_recoverable_after_disconnect() {
    let (dir, store, a, _) = setup().await;
    let s = store.pull_draft(key()).await.unwrap();
    assert!(s.can_preview);
    let mut values = s.values.clone();
    values.body = "offline changes".into();
    store.disconnect("a").await.unwrap();
    let account = store.account("a").await.unwrap();
    let offline = store.pull_draft(key()).await.unwrap();
    assert!(!offline.can_preview);
    let changed = store
        .save_pull_draft(SavePullDraftRequest {
            key: key(),
            authorization_epoch: account.authorization_epoch,
            authorization_view: offline.authorization_view,
            expected_generation: s.generation.clone(),
            values,
        })
        .await
        .unwrap();
    assert_ne!(changed.generation, s.generation);
    assert!(
        store
            .save_pull_draft(SavePullDraftRequest {
                key: key(),
                authorization_epoch: a.authorization_epoch,
                authorization_view: s.authorization_view,
                expected_generation: s.generation,
                values: s.values
            })
            .await
            .is_err()
    );
    store.close().await.unwrap();
    let store = Store::open(dir.path().join("pull.db")).await.unwrap();
    assert_eq!(
        store.pull_draft(key()).await.unwrap().values.body,
        "offline changes"
    );
    assert_eq!(
        store
            .pull_drafts(PullDraftQuery {
                account_id: "a".into(),
                cursor: None,
                limit: 1
            })
            .await
            .unwrap()
            .drafts
            .len(),
        1
    );
    store.close().await.unwrap();
}
#[tokio::test]
async fn creation_saves_causal_identity_even_if_branch_tips_drift_and_cold_duplicate_never_reposts()
{
    let (dir, store, a, local) = setup().await;
    let changed = "c".repeat(40);
    let mut replies = reads(HEAD, BASE);
    replies.extend(reads(HEAD, BASE));
    replies.push((201, Some(created(&changed, BASE)), String::new()));
    let (p, _, server) = server(replies);
    let r = request(preview(&store, &a, &p, &local).await.0);
    admit(&store, &a, &p, &r).await;
    let (attempt, report) = dispatch(&store, &a, &p, &r).await;
    assert!(matches!(report.outcome, DeliveryOutcome::Confirmed(_)));
    complete(&store, &a, &p, &attempt, &report).await;
    let snapshot = store.pull_draft(key()).await.unwrap();
    let published = snapshot.published.unwrap();
    assert!(published.branches_changed);
    assert_eq!(published.inspected_source_oid, HEAD);
    assert_eq!(published.observed_source_oid, changed);
    assert_eq!(published.subject_id, "github:pull:9007199254740999");
    let body = store
        .detail(DetailQuery {
            account_id: "a".into(),
            subject_id: published.subject_id.clone(),
            facet: DetailFacet::Body,
            cursor: None,
            limit: 1,
        })
        .await
        .unwrap();
    assert_eq!(body.body.text.as_deref(), Some("private draft body"));
    let calls = server.join().unwrap();
    assert_eq!(calls.len(), 7);
    assert!(calls[1].starts_with("GET /repositories/1/branches/feature%2Fslash "));
    assert!(calls[6].starts_with("POST /repositories/1/pulls "));
    let sent: serde_json::Value =
        serde_json::from_str(calls[6].split("\r\n\r\n").nth(1).unwrap()).unwrap();
    assert_eq!(
        sent,
        json!({"title":"New pull","body":"private draft body","head":"feature/slash","base":"main","draft":true,"maintainer_can_modify":false})
    );
    store.close().await.unwrap();
    let store = Store::open(dir.path().join("pull.db")).await.unwrap();
    assert!(
        store
            .submit_pull(r, None, || Ok(()), || Err(n::invalid()))
            .await
            .unwrap()
            .duplicate
    );
    assert_eq!(
        store
            .pull_draft(key())
            .await
            .unwrap()
            .published
            .unwrap()
            .subject_id,
        published.subject_id
    );
    store.close().await.unwrap();
}
#[tokio::test]
async fn unpublished_local_source_permission_loss_and_identical_tips_cannot_issue_grants() {
    for mode in 0..3 {
        let (_dir, store, a, local) = setup().await;
        let mut replies = reads(
            if mode == 0 { BASE } else { HEAD },
            if mode == 2 { HEAD } else { BASE },
        );
        if mode == 1 {
            replies[0].1.as_mut().unwrap()["permissions"]["push"] = json!(false);
        }
        let (p, _, server) = server(replies);
        let (preview, _) = preview(&store, &a, &p, &local).await;
        assert!(preview.context.is_none());
        assert_eq!(
            preview.reason,
            Some(
                [
                    PullCreationReason::UnpublishedSource,
                    PullCreationReason::PermissionUnavailable,
                    PullCreationReason::SameBranch
                ][mode]
            )
        );
        assert_eq!(server.join().unwrap().len(), 3);
        store.close().await.unwrap();
    }
}
#[tokio::test]
async fn grant_expiry_during_preflight_is_durable_conflict_with_zero_post() {
    let (_dir, store, a, local) = setup().await;
    let mut replies = reads(HEAD, BASE);
    replies.extend(reads(HEAD, BASE));
    let (p, _, server) = server_hook(
        replies,
        Arc::new(|count, clock| {
            if count == 6 {
                clock.advance(61)
            }
        }),
    );
    let r = request(preview(&store, &a, &p, &local).await.0);
    admit(&store, &a, &p, &r).await;
    let (read, prepared) = prepare(&store, &a, &p, &r).await;
    let claim = store
        .claim_delivery(&read.command, &a, p.as_ref(), &prepared.bytes, &time())
        .await
        .unwrap();
    assert!(claim.request.is_none());
    let c = store.delivery_command("a", &r.command_id).await.unwrap();
    assert_eq!(c.state, DeliveryState::Conflict);
    assert_eq!(c.attempt_count, 0);
    assert_eq!(server.join().unwrap().len(), 6);
    store.close().await.unwrap();
}
#[tokio::test]
async fn cold_restart_does_not_restore_grant_or_dispatch_authority() {
    let (dir, store, a, local) = setup().await;
    let (p, _, server_handle) = server(reads(HEAD, BASE));
    let r = request(preview(&store, &a, &p, &local).await.0);
    admit(&store, &a, &p, &r).await;
    assert_eq!(server_handle.join().unwrap().len(), 3);
    store.close().await.unwrap();
    drop(p);
    let store = Store::open(dir.path().join("pull.db")).await.unwrap();
    let (cold, _, no_http) = server(vec![]);
    let (read, prepared) = prepare(&store, &a, &cold, &r).await;
    let claim = store
        .claim_delivery(&read.command, &a, cold.as_ref(), &prepared.bytes, &time())
        .await
        .unwrap();
    assert!(claim.request.is_none());
    let c = store.delivery_command("a", &r.command_id).await.unwrap();
    assert_eq!(c.state, DeliveryState::Conflict);
    assert_eq!(c.attempt_count, 0);
    assert!(no_http.join().unwrap().is_empty());
    store.close().await.unwrap();
}
#[tokio::test]
async fn lost_malformed_and_accepted_creation_stay_unknown_without_second_post() {
    for (status, response) in [
        (201, None),
        (201, Some(json!({"id":1}))),
        (202, Some(created(HEAD, BASE))),
    ] {
        let (_dir, store, a, local) = setup().await;
        let mut replies = reads(HEAD, BASE);
        replies.extend(reads(HEAD, BASE));
        replies.push((status, response, String::new()));
        let (p, _, server) = server(replies);
        let r = request(preview(&store, &a, &p, &local).await.0);
        admit(&store, &a, &p, &r).await;
        let (attempt, report) = dispatch(&store, &a, &p, &r).await;
        assert!(matches!(report.outcome, DeliveryOutcome::Unknown));
        complete(&store, &a, &p, &attempt, &report).await;
        let c = store.delivery_command("a", &r.command_id).await.unwrap();
        assert_eq!(c.state, DeliveryState::Unknown);
        assert_eq!(c.attempt_count, 1);
        let (read, _) = store
            .claim_reconciliation(&c, &a, p.as_ref(), &time())
            .await
            .unwrap();
        if let Some(read) = read {
            assert!(matches!(
                p.reconcile(&token(), read).await.unwrap().outcome,
                DeliveryOutcome::Unknown
            ));
        }
        let snapshot = store.pull_draft(key()).await.unwrap();
        assert!(snapshot.published.is_none());
        assert_eq!(snapshot.reason, Some(PullCreationReason::PendingSubmission));
        assert_eq!(snapshot.submission.unwrap().state, "outcome_unknown");
        assert_eq!(server.join().unwrap().len(), 7);
        store.close().await.unwrap();
    }
}
#[tokio::test]
async fn foreign_receipt_actor_branch_repository_and_extra_codec_fields_are_refused() {
    let (_dir, store, a, local) = setup().await;
    let (p, _, server) = server(reads(HEAD, BASE));
    let (view, frame) = preview(&store, &a, &p, &local).await;
    let r = request(view);
    admit(&store, &a, &p, &r).await;
    let c = store.delivery_command("a", &r.command_id).await.unwrap();
    let payload = n::decode(&c).unwrap();
    let prep = n::Preparation {
        frame: frame.clone(),
        actor: a.actor_id.clone(),
        epoch: a.authorization_epoch.clone(),
        command_hash: n::command_hash(&c),
        source_oid: HEAD.into(),
        base_oid: BASE.into(),
        observed_at: now(),
    };
    // The original exact literal receipt must validate; each unrelated mutation must fail.
    let original = crate::providers::github::pull_creation::receipt::parse(
        &a,
        &payload,
        &frame,
        &serde_json::to_vec(&created(HEAD, BASE)).unwrap(),
    )
    .unwrap();
    let evidence = n::ReceiptEvidence {
        preparation: prep,
        receipt: original,
    };
    assert!(
        n::preparation_matches(&evidence.preparation, &payload),
        "preparation failed"
    );
    assert!(
        crate::storage::validate_resource_metadata(&evidence.receipt.metadata.observation())
            .is_ok(),
        "metadata failed: {:?}",
        evidence.receipt.metadata
    );
    assert!(
        n::receipt_matches(&evidence, &payload),
        "synthetic evidence {:?}",
        evidence
    );

    for mutation in 0..6 {
        let mut bad = evidence.clone();
        match mutation {
            0 => {
                bad.receipt
                    .metadata
                    .values
                    .author
                    .as_mut()
                    .unwrap()
                    .provider_id = "8".into()
            }
            1 => bad.receipt.metadata.values.head.as_mut().unwrap().name = "other".into(),
            2 => {
                bad.receipt
                    .metadata
                    .values
                    .base
                    .as_mut()
                    .unwrap()
                    .repository
                    .as_mut()
                    .unwrap()
                    .provider_id = "2".into()
            }
            3 => bad.receipt.item.id = "github:pull:1".into(),
            4 => bad.preparation.source_oid = BASE.into(),
            _ => bad.receipt.item.is_draft = Some(false),
        }
        assert!(!n::receipt_matches(&bad, &payload));
    }
    let mut json = serde_json::to_value(&evidence).unwrap();
    json["receipt"]["item"]["native_inbox"] = serde_json::Value::Null;
    assert!(n::decode_json::<n::ReceiptEvidence>(&serde_json::to_vec(&json).unwrap()).is_err());
    assert_eq!(server.join().unwrap().len(), 3);
    store.close().await.unwrap();
}

#[tokio::test]
async fn expiry_after_attempt_claim_records_no_post_and_a_truthful_conflict() {
    let (_dir, store, a, local) = setup().await;
    let mut replies = reads(HEAD, BASE);
    replies.extend(reads(HEAD, BASE));
    let (p, clock, http) = server(replies);
    let r = request(preview(&store, &a, &p, &local).await.0);
    admit(&store, &a, &p, &r).await;
    let (read, prepared) = prepare(&store, &a, &p, &r).await;
    let attempt = store
        .claim_delivery(&read.command, &a, p.as_ref(), &prepared.bytes, &time())
        .await
        .unwrap()
        .request
        .unwrap();
    clock.advance(61);
    let report = p.dispatch(&token(), attempt.clone()).await;
    assert!(matches!(report.outcome, DeliveryOutcome::Conflict(_)));
    complete(&store, &a, &p, &attempt, &report).await;
    let c = store.delivery_command("a", &r.command_id).await.unwrap();
    assert_eq!(c.state, DeliveryState::Conflict);
    assert_eq!(c.attempt_count, 1);
    assert_eq!(http.join().unwrap().len(), 6);
    store.close().await.unwrap();
}
#[tokio::test]
async fn recreated_or_foreign_window_cannot_arm_or_dispatch_an_existing_grant() {
    use std::sync::atomic::{AtomicBool, Ordering};
    let (_dir, store, a, local) = setup().await;
    let (p, _, http) = server(reads(HEAD, BASE));
    let snapshot = store.pull_draft(key()).await.unwrap();
    let q = PreviewPullCreationRequest {
        key: key(),
        draft_generation: snapshot.generation,
        authorization_epoch: a.authorization_epoch.clone(),
        authorization_view: snapshot.authorization_view,
    };
    let (_, frame) = store
        .pull_creation_frame(&q, &n::LocalProof::from_observation(&local).unwrap())
        .await
        .unwrap();
    let valid = Arc::new(AtomicBool::new(true));
    let v = valid.clone();
    let captured = PullCreationOwner::new(
        "original-window-generation".into(),
        Arc::new(move || {
            if v.load(Ordering::SeqCst) {
                Ok(())
            } else {
                Err(CollaborationError::new(
                    ErrorCode::StaleView,
                    "retired fixture owner",
                ))
            }
        }),
    )
    .unwrap();
    let r = request(
        p.preview(&token(), &a, &key(), &frame, &captured)
            .await
            .unwrap()
            .0,
    );
    assert!(p.arm(&a, &r, &owner()).is_err());
    let f = p.arm(&a, &r, &captured).unwrap().unwrap();
    store
        .submit_pull(
            r.clone(),
            Some(&f),
            || captured.validate(),
            || {
                if p.live(&a, &r) {
                    Ok(())
                } else {
                    Err(n::invalid())
                }
            },
        )
        .await
        .unwrap();
    valid.store(false, Ordering::SeqCst);
    let (read, prepared) = prepare(&store, &a, &p, &r).await;
    assert!(
        store
            .claim_delivery(&read.command, &a, p.as_ref(), &prepared.bytes, &time())
            .await
            .unwrap()
            .request
            .is_none()
    );
    assert_eq!(
        store
            .delivery_command("a", &r.command_id)
            .await
            .unwrap()
            .attempt_count,
        0
    );
    assert_eq!(http.join().unwrap().len(), 3);
    store.close().await.unwrap();
}
#[tokio::test]
async fn changed_remote_tip_or_permission_before_claim_requires_new_consent() {
    for permission in [false, true] {
        let (_dir, store, a, local) = setup().await;
        let mut replies = reads(HEAD, BASE);
        let mut fresh = reads(if permission { HEAD } else { BASE }, BASE);
        if permission {
            fresh[0].1.as_mut().unwrap()["permissions"]["push"] = json!(false);
        }
        replies.extend(fresh);
        let (p, _, http) = server(replies);
        let r = request(preview(&store, &a, &p, &local).await.0);
        admit(&store, &a, &p, &r).await;
        let (read, prepared) = prepare(&store, &a, &p, &r).await;
        assert!(
            store
                .claim_delivery(&read.command, &a, p.as_ref(), &prepared.bytes, &time())
                .await
                .unwrap()
                .request
                .is_none()
        );
        assert_eq!(
            store
                .delivery_command("a", &r.command_id)
                .await
                .unwrap()
                .state,
            DeliveryState::Conflict
        );
        assert_eq!(http.join().unwrap().len(), 6);
        store.close().await.unwrap();
    }
}
#[tokio::test]
async fn preview_quota_stops_following_branch_reads_and_mutation_auth_quota_survives_invalid_receipt()
 {
    let (_dir, store, a, local) = setup().await;
    let (p, _, http) = server(vec![(
        200,
        Some(repository()),
        "Retry-After: 120\r\n".into(),
    )]);
    let snapshot = store.pull_draft(key()).await.unwrap();
    let q = PreviewPullCreationRequest {
        key: key(),
        draft_generation: snapshot.generation,
        authorization_epoch: a.authorization_epoch.clone(),
        authorization_view: snapshot.authorization_view,
    };
    let (_, frame) = store
        .pull_creation_frame(&q, &n::LocalProof::from_observation(&local).unwrap())
        .await
        .unwrap();
    let err = p
        .preview(&token(), &a, &key(), &frame, &owner())
        .await
        .unwrap_err();
    assert_eq!(err.account_cooldown_seconds, Some(120));
    assert_eq!(http.join().unwrap().len(), 1);
    store.close().await.unwrap();
    let (_dir, store, a, local) = setup().await;
    let mut replies = reads(HEAD, BASE);
    replies.extend(reads(HEAD, BASE));
    replies.push((
        401,
        Some(json!({"bad":"receipt"})),
        "Retry-After: 120\r\n".into(),
    ));
    let (p, _, http) = server(replies);
    let r = request(preview(&store, &a, &p, &local).await.0);
    admit(&store, &a, &p, &r).await;
    let (_, report) = dispatch(&store, &a, &p, &r).await;
    assert!(matches!(report.outcome, DeliveryOutcome::Unknown));
    assert_eq!(report.account_cooldown_seconds, Some(120));
    assert_eq!(
        report.provider_error.unwrap().kind,
        crate::providers::ProviderErrorKind::Authentication
    );
    assert_eq!(http.join().unwrap().len(), 7);
    store.close().await.unwrap();
}
#[tokio::test]
async fn bounded_invalid_admission_and_retired_owner_leave_no_command() {
    let (_dir, store, a, local) = setup().await;
    let (p, _, http) = server(reads(HEAD, BASE));
    let r = request(preview(&store, &a, &p, &local).await.0);
    let f = p.arm(&a, &r, &owner()).unwrap().unwrap();
    for mode in 0..5 {
        let mut invalid = r.clone();
        match mode {
            0 => invalid.command_id = "x".repeat(5000),
            1 => invalid.context.authorization_view = "-1".into(),
            2 => invalid.context.key.account_id = "x".repeat(1025),
            3 => invalid.context.source_oid = "A".repeat(40),
            _ => invalid.confirm_current_branches = false,
        };
        assert!(
            store
                .submit_pull(invalid, Some(&f), || Ok(()), || Ok(()))
                .await
                .is_err()
        );
    }
    assert!(
        store
            .submit_pull(
                r.clone(),
                Some(&f),
                || Err(CollaborationError::new(ErrorCode::StaleView, "retired")),
                || Ok(())
            )
            .await
            .is_err()
    );
    let count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM commands WHERE operation_kind='github.create_pull_request'",
    )
    .fetch_one(&store.inner.readers)
    .await
    .unwrap();
    assert_eq!(count, 0);
    assert_eq!(http.join().unwrap().len(), 3);
    store.close().await.unwrap();
}

struct HeldPreviewVault {
    entered: tokio::sync::Notify,
    release: StdMutex<std::sync::mpsc::Receiver<()>>,
}
impl crate::credentials::CredentialVault for HeldPreviewVault {
    fn store(
        &self,
        _: &str,
        _: &crate::credentials::SecretToken,
    ) -> std::result::Result<(), crate::credentials::CredentialError> {
        Ok(())
    }
    fn delete(&self, _: &str) -> std::result::Result<(), crate::credentials::CredentialError> {
        Ok(())
    }
    fn load(
        &self,
        _: &str,
    ) -> std::result::Result<
        Option<crate::credentials::SecretToken>,
        crate::credentials::CredentialError,
    > {
        self.entered.notify_one();
        self.release
            .lock()
            .unwrap()
            .recv_timeout(Duration::from_secs(10))
            .expect("bounded preview vault release");
        Ok(Some(token()))
    }
}
#[tokio::test]
async fn held_vault_preview_rechecks_account_quota_before_first_provider_read() {
    let (_dir, store, a, local) = setup().await;
    store
        .stage_credential("a", "creation-fixture")
        .await
        .unwrap();
    let a = store
        .commit_account_credential(
            RemoteAccount {
                authorization_epoch: "2".into(),
                ..a
            },
            "creation-fixture",
        )
        .await
        .unwrap();
    fixtures::project(&store, &a).await;
    let snapshot = store.pull_draft(key()).await.unwrap();
    let q = PreviewPullCreationRequest {
        key: key(),
        draft_generation: snapshot.generation,
        authorization_epoch: a.authorization_epoch.clone(),
        authorization_view: snapshot.authorization_view,
    };
    let (p, _, http) = server(vec![]);
    let mut registry = crate::providers::ProviderRegistry::default();
    registry
        .register(Arc::new(
            crate::providers::github::GithubProvider::new().unwrap(),
        ))
        .unwrap();
    registry.install_pull_creation(p).unwrap();
    let (send, recv) = std::sync::mpsc::channel();
    let vault = Arc::new(HeldPreviewVault {
        entered: tokio::sync::Notify::new(),
        release: StdMutex::new(recv),
    });
    let store = Arc::new(store);
    let runtime = Arc::new(crate::runtime::CollaborationRuntime::with_registry(
        store.clone(),
        vault.clone(),
        registry,
    ));
    let pending = tokio::spawn({
        let runtime = runtime.clone();
        async move { runtime.preview_pull_creation(q, local, owner()).await }
    });
    tokio::time::timeout(Duration::from_secs(10), vault.entered.notified())
        .await
        .unwrap();
    store
        .merge_provider_budget(
            "a",
            &a.authorization_epoch,
            (chrono::Utc::now() + chrono::Duration::seconds(120)).to_rfc3339(),
            None,
        )
        .await
        .unwrap();
    send.send(()).unwrap();
    assert_eq!(
        pending.await.unwrap().unwrap_err().code,
        ErrorCode::RateLimited
    );
    assert!(http.join().unwrap().is_empty());
    runtime.shutdown().await.unwrap();
}
struct ExpiringVault(Clock);
impl crate::credentials::CredentialVault for ExpiringVault {
    fn store(
        &self,
        _: &str,
        _: &crate::credentials::SecretToken,
    ) -> std::result::Result<(), crate::credentials::CredentialError> {
        Ok(())
    }
    fn delete(&self, _: &str) -> std::result::Result<(), crate::credentials::CredentialError> {
        Ok(())
    }
    fn load(
        &self,
        _: &str,
    ) -> std::result::Result<
        Option<crate::credentials::SecretToken>,
        crate::credentials::CredentialError,
    > {
        self.0.advance(61);
        Ok(Some(token()))
    }
}
#[tokio::test]
async fn delivery_vault_wait_cannot_revive_expired_online_consent() {
    let (_dir, store, a, local) = setup().await;
    store
        .stage_credential("a", "creation-fixture")
        .await
        .unwrap();
    let a = store
        .commit_account_credential(
            RemoteAccount {
                authorization_epoch: "2".into(),
                ..a
            },
            "creation-fixture",
        )
        .await
        .unwrap();
    fixtures::project(&store, &a).await;
    let (p, clock, http) = server(reads(HEAD, BASE));
    let r = request(preview(&store, &a, &p, &local).await.0);
    admit(&store, &a, &p, &r).await;
    let mut registry = crate::providers::ProviderRegistry::default();
    registry
        .register(Arc::new(
            crate::providers::github::GithubProvider::new().unwrap(),
        ))
        .unwrap();
    registry.install_pull_creation(p).unwrap();
    let store = Arc::new(store);
    let runtime = crate::runtime::CollaborationRuntime::with_registry(
        store.clone(),
        Arc::new(ExpiringVault(clock)),
        registry,
    );
    assert!(runtime.run_delivery_next().await.unwrap());
    let c = store.delivery_command("a", &r.command_id).await.unwrap();
    assert_eq!(c.state, DeliveryState::Conflict);
    assert_eq!(c.attempt_count, 0);
    assert_eq!(http.join().unwrap().len(), 3);
    runtime.shutdown().await.unwrap();
}
#[tokio::test]
async fn owner_retired_at_commit_rolls_back_admission_and_exact_receipt_retry_survives_local_drift()
{
    use std::sync::atomic::{AtomicUsize, Ordering};
    let (_dir, store, a, local) = setup().await;
    let (p, _, http) = server(reads(HEAD, BASE));
    let r = request(preview(&store, &a, &p, &local).await.0);
    let frame = p.arm(&a, &r, &owner()).unwrap().unwrap();
    let calls = AtomicUsize::new(0);
    assert!(
        store
            .submit_pull(
                r.clone(),
                Some(&frame),
                || if calls.fetch_add(1, Ordering::SeqCst) == 0 {
                    Ok(())
                } else {
                    Err(CollaborationError::new(
                        ErrorCode::StaleView,
                        "retired fixture",
                    ))
                },
                || Ok(())
            )
            .await
            .is_err()
    );
    assert!(store.delivery_command("a", &r.command_id).await.is_err());
    store
        .submit_pull(r.clone(), Some(&frame), || Ok(()), || Ok(()))
        .await
        .unwrap();
    let mut registry = crate::providers::ProviderRegistry::default();
    registry
        .register(Arc::new(
            crate::providers::github::GithubProvider::new().unwrap(),
        ))
        .unwrap();
    registry.install_pull_creation(p).unwrap();
    let runtime = crate::runtime::CollaborationRuntime::with_registry(
        Arc::new(store),
        Arc::new(ExpiringVault(Clock(Arc::new(
            StdMutex::new(Instant::now()),
        )))),
        registry,
    );
    assert!(
        runtime
            .submit_pull(r, None, owner())
            .await
            .unwrap()
            .duplicate
    );
    assert_eq!(http.join().unwrap().len(), 3);
    runtime.shutdown().await.unwrap();
}
#[tokio::test]
async fn shutdown_releases_process_local_owner_callbacks_after_draining() {
    let (_dir, store, a, local) = setup().await;
    let (p, _, http) = server(reads(HEAD, BASE));
    let snapshot = store.pull_draft(key()).await.unwrap();
    let q = PreviewPullCreationRequest {
        key: key(),
        draft_generation: snapshot.generation,
        authorization_epoch: a.authorization_epoch.clone(),
        authorization_view: snapshot.authorization_view,
    };
    let (_, frame) = store
        .pull_creation_frame(&q, &n::LocalProof::from_observation(&local).unwrap())
        .await
        .unwrap();
    let lifetime = Arc::new(());
    let weak = Arc::downgrade(&lifetime);
    let captured = PullCreationOwner::new(
        "release-fixture".into(),
        Arc::new(move || {
            let _ = &lifetime;
            Ok(())
        }),
    )
    .unwrap();
    p.preview(&token(), &a, &key(), &frame, &captured)
        .await
        .unwrap();
    drop(captured);
    assert!(weak.upgrade().is_some());
    let mut registry = crate::providers::ProviderRegistry::default();
    registry
        .register(Arc::new(
            crate::providers::github::GithubProvider::new().unwrap(),
        ))
        .unwrap();
    registry.install_pull_creation(p).unwrap();
    let runtime = crate::runtime::CollaborationRuntime::with_registry(
        Arc::new(store),
        Arc::new(ExpiringVault(Clock(Arc::new(
            StdMutex::new(Instant::now()),
        )))),
        registry,
    );
    runtime.shutdown().await.unwrap();
    assert!(weak.upgrade().is_none());
    assert_eq!(http.join().unwrap().len(), 3);
}
#[tokio::test]
async fn receipt_only_probe_of_new_consent_returns_not_ready_without_arming_or_admitting() {
    let (_dir, store, a, local) = setup().await;
    let (p, _, http) = server(reads(HEAD, BASE));
    let r = request(preview(&store, &a, &p, &local).await.0);
    let mut registry = crate::providers::ProviderRegistry::default();
    registry
        .register(Arc::new(
            crate::providers::github::GithubProvider::new().unwrap(),
        ))
        .unwrap();
    registry.install_pull_creation(p.clone()).unwrap();
    let store = Arc::new(store);
    let runtime = crate::runtime::CollaborationRuntime::with_registry(
        store.clone(),
        Arc::new(ExpiringVault(Clock(Arc::new(
            StdMutex::new(Instant::now()),
        )))),
        registry,
    );
    assert_eq!(
        runtime
            .submit_pull(r.clone(), None, owner())
            .await
            .unwrap_err()
            .code,
        ErrorCode::NotReady
    );
    assert!(!p.live(&a, &r));
    assert!(store.delivery_command("a", &r.command_id).await.is_err());
    assert!(
        !runtime
            .submit_pull(r.clone(), Some(local), owner())
            .await
            .unwrap()
            .duplicate
    );
    assert!(p.live(&a, &r));
    assert_eq!(http.join().unwrap().len(), 3);
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn escaped_draft_budget_refuses_preview_before_provider_io_and_preserves_authorship() {
    let (_dir, store, a, local) = setup().await;
    let old = store.pull_draft(key()).await.unwrap();
    let mut values = old.values;
    values.body = "\\".repeat(16 * 1024);
    let saved = store
        .save_pull_draft(SavePullDraftRequest {
            key: key(),
            authorization_epoch: a.authorization_epoch.clone(),
            authorization_view: old.authorization_view,
            expected_generation: old.generation,
            values,
        })
        .await
        .unwrap();
    let q = PreviewPullCreationRequest {
        key: key(),
        draft_generation: saved.generation,
        authorization_epoch: a.authorization_epoch.clone(),
        authorization_view: saved.authorization_view,
    };
    let (p, _, http) = server(vec![]);
    let mut registry = crate::providers::ProviderRegistry::default();
    registry
        .register(Arc::new(
            crate::providers::github::GithubProvider::new().unwrap(),
        ))
        .unwrap();
    registry.install_pull_creation(p).unwrap();
    let store = Arc::new(store);
    let runtime = crate::runtime::CollaborationRuntime::with_registry(
        store.clone(),
        Arc::new(ExpiringVault(Clock(Arc::new(
            StdMutex::new(Instant::now()),
        )))),
        registry,
    );
    let error = runtime
        .preview_pull_creation(q, local, owner())
        .await
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::InvalidInput);
    assert!(error.message.contains("receipt budget"));
    assert_eq!(
        store.pull_draft(key()).await.unwrap().values.body,
        "\\".repeat(16 * 1024)
    );
    assert!(http.join().unwrap().is_empty());
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM commands")
            .fetch_one(&store.inner.readers)
            .await
            .unwrap(),
        0
    );
    runtime.shutdown().await.unwrap();
}
#[tokio::test]
async fn maximum_plain_body_and_unrequested_large_collections_fit_causal_receipt_budget() {
    let (_dir, store, a, local) = setup().await;
    let old = store.pull_draft(key()).await.unwrap();
    let mut values = old.values;
    values.body = "a".repeat(16 * 1024);
    let saved = store
        .save_pull_draft(SavePullDraftRequest {
            key: key(),
            authorization_epoch: a.authorization_epoch.clone(),
            authorization_view: old.authorization_view,
            expected_generation: old.generation,
            values,
        })
        .await
        .unwrap();
    let mut response = created(HEAD, BASE);
    response["body"] = json!(saved.values.body);
    response["labels"] = json!(
        (0..100)
            .map(|i| format!("{i:03}{}", "L".repeat(1000)))
            .collect::<Vec<_>>()
    );
    let mut replies = reads(HEAD, BASE);
    replies.extend(reads(HEAD, BASE));
    replies.push((201, Some(response), String::new()));
    let (p, _, http) = server(replies);
    let r = request(preview(&store, &a, &p, &local).await.0);
    admit(&store, &a, &p, &r).await;
    assert_eq!(
        store.pull_draft(key()).await.unwrap().reason,
        Some(PullCreationReason::PendingSubmission)
    );
    let (attempt, report) = dispatch(&store, &a, &p, &r).await;
    let DeliveryOutcome::Confirmed(evidence) = &report.outcome else {
        panic!("valid bounded 201 must confirm");
    };
    assert!(evidence.payload.len() < n::MAX_BYTES);
    let proof: n::ReceiptEvidence = n::decode_json(&evidence.payload).unwrap();
    assert_eq!(
        proof.receipt.item.body.as_deref(),
        Some(saved.values.body.as_str())
    );
    assert!(proof.receipt.metadata.values.labels.is_empty());
    for f in [
        MetadataField::Labels,
        MetadataField::Assignees,
        MetadataField::Milestone,
    ] {
        assert!(
            proof
                .receipt
                .metadata
                .fields
                .contains(&(f, DetailValueState::Omitted))
        );
    }
    complete(&store, &a, &p, &attempt, &report).await;
    let snapshot = store.pull_draft(key()).await.unwrap();
    assert_eq!(snapshot.reason, Some(PullCreationReason::AlreadySubmitted));
    assert!(snapshot.published.is_some());
    assert_eq!(http.join().unwrap().len(), 7);
    store.close().await.unwrap();
}
