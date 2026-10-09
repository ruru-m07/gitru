use super::*;
use crate::delivery::*;
use crate::guarded_merge::native::{decode, encode};
use crate::guarded_merge::{native::*, *};
use crate::providers::github::guarded_merge::GithubGuardedMergePolicy;
use crate::runtime::detail_tests::fixtures;
use crate::*;
use serde_json::json;
use std::{
    io::{Read, Write},
    sync::{Arc, Mutex as StdMutex},
    time::{Duration, Instant},
};
const HEAD: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const MERGE: &str = "cccccccccccccccccccccccccccccccccccccccc";
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
    crate::credentials::SecretToken::new("synthetic_merge_fixture".into()).unwrap()
}
fn repo() -> serde_json::Value {
    json!({"id":1,"archived":false,"disabled":false,"permissions":{"push":true},"allow_merge_commit":true,"allow_squash_merge":true,"allow_rebase_merge":false})
}
fn pull(merged: bool) -> serde_json::Value {
    json!({"id":9007199254740997_u64,"number":67,"url":"https://api.github.com/repos/owner/project/pulls/67","html_url":"https://github.com/owner/project/pull/67","title":"Remote title","body":"Remote body","state":if merged{"closed"}else{"open"},"merged":merged,"merged_at":if merged{Some("2026-10-08T01:00:00Z")}else{None},"merge_commit_sha":if merged{Some(MERGE)}else{None},"draft":false,"mergeable":true,"mergeable_state":"clean","auto_merge":null,"updated_at":"2026-10-08T02:00:00Z","head":{"ref":"feature","sha":HEAD,"repo":null},"base":{"ref":"main","sha":"b".repeat(40),"repo":{"id":1,"full_name":"owner/project","html_url":"https://github.com/owner/project"}}})
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
    Arc<GithubGuardedMergePolicy>,
    Clock,
    std::thread::JoinHandle<Vec<String>>,
) {
    server_hook(replies, Arc::new(|_, _| {}))
}
fn server_hook(
    replies: Vec<Reply>,
    hook: ResponseHook,
) -> (
    Arc<GithubGuardedMergePolicy>,
    Clock,
    std::thread::JoinHandle<Vec<String>>,
) {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let url = reqwest::Url::parse(&format!("http://{}/", listener.local_addr().unwrap())).unwrap();
    let clock = Clock(Arc::new(StdMutex::new(Instant::now())));
    let n = clock.0.clone();
    let policy = Arc::new(GithubGuardedMergePolicy::for_test(
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

async fn preview(
    store: &Store,
    a: &RemoteAccount,
    p: &GithubGuardedMergePolicy,
) -> GuardedMergeRequest {
    let (_, frame) = store
        .guarded_merge_frame(&GuardedMergeQuery {
            account_id: a.id.clone(),
            subject_id: "pull".into(),
        })
        .await
        .unwrap();
    let (view, _) = p.preview(&token(), a, &frame).await.unwrap();
    assert_eq!(view.reason, None);
    GuardedMergeRequest {
        context: view.context.unwrap(),
        command_id: Uuid::new_v4().to_string(),
        method: MergeMethod::Squash,
        confirm_inspected_head: true,
    }
}
async fn admit(
    store: &Store,
    a: &RemoteAccount,
    p: &GithubGuardedMergePolicy,
    r: GuardedMergeRequest,
) -> GuardedMergeReceipt {
    let frame = p.arm(a, &r).unwrap();
    store
        .submit_guarded_merge(r.clone(), frame.as_ref(), || {
            if p.live(a, &r) {
                Ok(())
            } else {
                Err(invalid())
            }
        })
        .await
        .unwrap()
}
async fn preparation(
    store: &Store,
    a: &RemoteAccount,
    p: &GithubGuardedMergePolicy,
    id: &str,
) -> (ReconcileRequest, DeliveryPreparation) {
    let c = store.delivery_command(&a.id, id).await.unwrap();
    let (r, _) = store.claim_preparation(&c, a, p, &time()).await.unwrap();
    let r = r.unwrap();
    let prep = p.prepare(&token(), &r).await.unwrap();
    (r, prep)
}
async fn claim(
    store: &Store,
    a: &RemoteAccount,
    p: &GithubGuardedMergePolicy,
    id: &str,
) -> super::delivery::DeliveryClaim {
    let (r, prep) = preparation(store, a, p, id).await;
    store
        .claim_delivery(&r.command, a, p, &prep.bytes, &time())
        .await
        .unwrap()
}
async fn complete(
    store: &Store,
    a: &RemoteAccount,
    p: &GithubGuardedMergePolicy,
    c: &DeliveryCommand,
    attempt: Option<i64>,
    report: &DeliveryReport,
) {
    store
        .complete_delivery(
            c,
            p,
            super::delivery::DeliveryCompletion {
                account: a,
                attempt,
                report,
                now: &now(),
                next: &now(),
            },
        )
        .await
        .unwrap();
}
async fn reconcile(
    store: &Store,
    a: &RemoteAccount,
    p: &GithubGuardedMergePolicy,
    id: &str,
) -> DeliveryReport {
    let c = store.delivery_command(&a.id, id).await.unwrap();
    let (r, _) = store.claim_reconciliation(&c, a, p, &time()).await.unwrap();
    let r = r.unwrap();
    let report = p.reconcile(&token(), r.clone()).await.unwrap();
    complete(store, a, p, &r.command, None, &report).await;
    report
}
#[tokio::test]
async fn guarded_put_is_durable_exact_and_confirmed_only_after_canonical_merged_read() {
    let (_dir, store, a) = setup().await;
    let checkpoint = store
        .scope_state("a", "repo:repo:pull_request")
        .await
        .unwrap()
        .unwrap();
    let mut held_item = store.item("a", "pull").await.unwrap().item.unwrap();
    held_item.updated_at = "2026-10-08T02:00:00Z".into();
    let (p, _clock, http) = server(vec![
        ok(repo()),
        ok(pull(false)),
        ok(repo()),
        ok(pull(false)),
        ok(json!({"merged":true,"sha":MERGE,"message":"merged"})),
        ok(pull(true)),
    ]);
    let r = preview(&store, &a, &p).await;
    let receipt = admit(&store, &a, &p, r.clone()).await;
    assert!(!receipt.duplicate);
    let duplicate = admit(&store, &a, &p, r.clone()).await;
    assert!(duplicate.duplicate);
    let call = claim(&store, &a, &p, &r.command_id).await.request.unwrap();
    assert_eq!(
        store
            .delivery_command("a", &r.command_id)
            .await
            .unwrap()
            .attempt_count,
        1
    );
    let report = p.dispatch(&token(), call.clone()).await;
    assert!(matches!(report.outcome, DeliveryOutcome::Accepted(_)));
    complete(&store, &a, &p, &call.command, Some(call.attempt), &report).await;
    assert_eq!(
        store.item("a", "pull").await.unwrap().item.unwrap().state,
        "open"
    );
    assert!(matches!(
        reconcile(&store, &a, &p, &r.command_id).await.outcome,
        DeliveryOutcome::Confirmed(_)
    ));
    assert_eq!(
        store.item("a", "pull").await.unwrap().item.unwrap().state,
        "merged"
    );
    assert_eq!(
        store
            .delivery_command("a", &r.command_id)
            .await
            .unwrap()
            .state,
        DeliveryState::Confirmed
    );
    // Equal-timestamp page/304 responses started before canonical publication
    // cannot replace merged state or hide it through a completed old traversal.
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
    for response in [
        held.clone(),
        PageCommit {
            items: vec![],
            not_modified: true,
            ..held
        },
    ] {
        assert_eq!(
            store
                .apply_fetched_page(response, vec![], checkpoint.data_revision)
                .await
                .unwrap_err()
                .code,
            ErrorCode::StaleView
        );
    }
    assert_eq!(
        store.item("a", "pull").await.unwrap().item.unwrap().state,
        "merged"
    );
    let requests = http.join().unwrap();
    let put: Vec<_> = requests.iter().filter(|r| r.starts_with("PUT ")).collect();
    assert_eq!(put.len(), 1);
    assert!(put[0].starts_with("PUT /repositories/1/pulls/67/merge HTTP/1.1"));
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(put[0].split("\r\n\r\n").nth(1).unwrap())
            .unwrap(),
        json!({"sha":HEAD,"merge_method":"squash"})
    );
    store.close().await.unwrap();
}
#[tokio::test]
async fn expiry_after_admission_is_a_zero_attempt_conflict_and_does_not_read_or_merge() {
    let (_dir, store, a) = setup().await;
    let (p, clock, http) = server(vec![ok(repo()), ok(pull(false))]);
    let r = preview(&store, &a, &p).await;
    admit(&store, &a, &p, r.clone()).await;
    clock.advance(60);
    assert!(claim(&store, &a, &p, &r.command_id).await.request.is_none());
    let c = store.delivery_command("a", &r.command_id).await.unwrap();
    assert_eq!(c.state, DeliveryState::Conflict);
    assert_eq!(c.attempt_count, 0);
    assert_eq!(http.join().unwrap().len(), 2);
    store.close().await.unwrap();
}

#[tokio::test]
async fn online_preview_refuses_unknown_permission_method_and_head_or_workflow_drift() {
    for (field, value, expected) in [
        ("draft", json!(true), MergeUnavailableReason::Draft),
        ("draft", json!(null), MergeUnavailableReason::Draft),
        (
            "mergeable",
            json!(null),
            MergeUnavailableReason::MergeabilityUnavailable,
        ),
        (
            "mergeable_state",
            json!("blocked"),
            MergeUnavailableReason::MergeabilityUnavailable,
        ),
        (
            "auto_merge",
            json!({"enabled_by":{"id":1}}),
            MergeUnavailableReason::AutomaticMergeUnsupported,
        ),
        ("state", json!("closed"), MergeUnavailableReason::NotOpen),
        (
            "head",
            json!({"ref":"feature","sha":"d".repeat(40),"repo":null}),
            MergeUnavailableReason::HeadChanged,
        ),
    ] {
        let (_dir, store, a) = setup().await;
        let mut response = pull(false);
        response[field] = value;
        let (p, _, http) = server(vec![ok(repo()), ok(response)]);
        let (_, frame) = store
            .guarded_merge_frame(&GuardedMergeQuery {
                account_id: a.id.clone(),
                subject_id: "pull".into(),
            })
            .await
            .unwrap();
        let (preview, _) = p.preview(&token(), &a, &frame).await.unwrap();
        assert_eq!(preview.reason, Some(expected));
        assert!(preview.context.is_none());
        assert_eq!(http.join().unwrap().len(), 2);
        store.close().await.unwrap();
    }
    for permission in [json!(false), json!(null)] {
        let (_dir, store, a) = setup().await;
        let mut repository = repo();
        repository["permissions"]["push"] = permission;
        let (p, _, http) = server(vec![ok(repository), ok(pull(false))]);
        let (_, frame) = store
            .guarded_merge_frame(&GuardedMergeQuery {
                account_id: a.id.clone(),
                subject_id: "pull".into(),
            })
            .await
            .unwrap();
        let (preview, _) = p.preview(&token(), &a, &frame).await.unwrap();
        assert_eq!(
            preview.reason,
            Some(MergeUnavailableReason::PermissionUnavailable)
        );
        assert!(preview.context.is_none());
        http.join().unwrap();
        store.close().await.unwrap();
    }
}

#[tokio::test]
async fn expiry_after_preflight_or_after_durable_claim_never_sends_put() {
    for after_claim in [false, true] {
        let (_dir, store, a) = setup().await;
        let (p, clock, http) = server(vec![
            ok(repo()),
            ok(pull(false)),
            ok(repo()),
            ok(pull(false)),
        ]);
        let r = preview(&store, &a, &p).await;
        admit(&store, &a, &p, r.clone()).await;
        let (req, prep) = preparation(&store, &a, &p, &r.command_id).await;
        if !after_claim {
            clock.advance(60);
        }
        let claimed = store
            .claim_delivery(&req.command, &a, p.as_ref(), &prep.bytes, &time())
            .await
            .unwrap();
        if after_claim {
            let call = claimed.request.unwrap();
            clock.advance(60);
            let report = p.dispatch(&token(), call.clone()).await;
            assert!(matches!(report.outcome, DeliveryOutcome::Conflict(_)));
            complete(&store, &a, &p, &call.command, Some(call.attempt), &report).await;
        } else {
            assert!(claimed.request.is_none());
        }
        let c = store.delivery_command("a", &r.command_id).await.unwrap();
        assert_eq!(c.state, DeliveryState::Conflict);
        assert_eq!(c.attempt_count, if after_claim { 1 } else { 0 });
        assert!(http.join().unwrap().iter().all(|r| r.starts_with("GET ")));
        store.close().await.unwrap();
    }
}

#[tokio::test]
async fn writer_rechecks_expired_online_authority_and_rolls_back_new_admission() {
    let (_dir, store, a) = setup().await;
    let (p, clock, http) = server(vec![ok(repo()), ok(pull(false))]);
    let r = preview(&store, &a, &p).await;
    let frame = p.arm(&a, &r).unwrap();
    clock.advance(60);
    let e = store
        .submit_guarded_merge(r.clone(), frame.as_ref(), || {
            if p.live(&a, &r) {
                Ok(())
            } else {
                Err(invalid())
            }
        })
        .await
        .unwrap_err();
    assert_eq!(e.code, ErrorCode::InvalidInput);
    assert!(
        store
            .command_receipt("a", &r.command_id)
            .await
            .unwrap()
            .is_none()
    );
    http.join().unwrap();
    store.close().await.unwrap();
}

#[tokio::test]
async fn fresh_preflight_head_permission_or_method_change_conflicts_without_an_attempt() {
    for mode in ["head", "permission", "method", "draft"] {
        let (_dir, store, a) = setup().await;
        let mut repository = repo();
        let mut changed = pull(false);
        match mode {
            "head" => changed["head"]["sha"] = json!("d".repeat(40)),
            "permission" => repository["permissions"]["push"] = json!(false),
            "method" => repository["allow_squash_merge"] = json!(false),
            _ => changed["draft"] = json!(true),
        }
        let (p, _, http) = server(vec![
            ok(repo()),
            ok(pull(false)),
            ok(repository),
            ok(changed),
        ]);
        let r = preview(&store, &a, &p).await;
        admit(&store, &a, &p, r.clone()).await;
        assert!(claim(&store, &a, &p, &r.command_id).await.request.is_none());
        let c = store.delivery_command("a", &r.command_id).await.unwrap();
        assert_eq!(c.state, DeliveryState::Conflict);
        assert_eq!(c.attempt_count, 0);
        assert!(http.join().unwrap().iter().all(|r| r.starts_with("GET ")));
        store.close().await.unwrap();
    }
}

#[tokio::test]
async fn accepted_lost_and_malformed_results_reconcile_without_second_put() {
    for response in [
        (
            202,
            Some(json!({"status":"pending"})),
            DeliveryState::Accepted,
        ),
        (
            200,
            Some(json!({"merged":false,"sha":MERGE,"message":"pending"})),
            DeliveryState::Unknown,
        ),
        (
            200,
            Some(json!({"merged":true,"sha":MERGE,"message":"ok","extra":true})),
            DeliveryState::Unknown,
        ),
        (200, None, DeliveryState::Unknown),
    ] {
        let (_dir, store, a) = setup().await;
        let (p, _, http) = server(vec![
            ok(repo()),
            ok(pull(false)),
            ok(repo()),
            ok(pull(false)),
            (response.0, response.1, String::new()),
            ok(pull(false)),
            ok(pull(true)),
        ]);
        let r = preview(&store, &a, &p).await;
        admit(&store, &a, &p, r.clone()).await;
        let call = claim(&store, &a, &p, &r.command_id).await.request.unwrap();
        let report = p.dispatch(&token(), call.clone()).await;
        complete(&store, &a, &p, &call.command, Some(call.attempt), &report).await;
        assert_eq!(
            store
                .delivery_command("a", &r.command_id)
                .await
                .unwrap()
                .state,
            response.2
        );
        assert!(matches!(
            reconcile(&store, &a, &p, &r.command_id).await.outcome,
            DeliveryOutcome::Unknown
        ));
        assert!(matches!(
            reconcile(&store, &a, &p, &r.command_id).await.outcome,
            DeliveryOutcome::Confirmed(_)
        ));
        let requests = http.join().unwrap();
        assert_eq!(requests.iter().filter(|r| r.starts_with("PUT ")).count(), 1);
        store.close().await.unwrap();
    }
}

#[tokio::test]
async fn cold_restart_keeps_exact_receipt_but_cannot_reconstruct_online_dispatch_authority() {
    let (dir, store, a) = setup().await;
    let (p, _, http) = server(vec![ok(repo()), ok(pull(false))]);
    let r = preview(&store, &a, &p).await;
    admit(&store, &a, &p, r.clone()).await;
    store.close().await.unwrap();
    drop(store);
    drop(p);
    let store = Store::open(dir.path().join("text.db")).await.unwrap();
    let (cold, _, no_http) = server(vec![]);
    let duplicate = store
        .submit_guarded_merge(r.clone(), None, || Err(invalid()))
        .await
        .unwrap();
    assert!(duplicate.duplicate);
    assert!(
        claim(&store, &a, &cold, &r.command_id)
            .await
            .request
            .is_none()
    );
    let c = store.delivery_command("a", &r.command_id).await.unwrap();
    assert_eq!(c.state, DeliveryState::Conflict);
    assert_eq!(c.attempt_count, 0);
    assert!(no_http.join().unwrap().is_empty());
    assert_eq!(http.join().unwrap().len(), 2);
    store.close().await.unwrap();
}

#[tokio::test]
async fn inspected_context_and_canonical_request_bounds_cannot_admit_foreign_or_forged_intent() {
    let (_dir, store, a) = setup().await;
    let (p, _, http) = server(vec![ok(repo()), ok(pull(false))]);
    let r = preview(&store, &a, &p).await;
    for field in 0..5 {
        let mut bad = r.clone();
        match field {
            0 => bad.command_id = "x".repeat(5000),
            1 => bad.context.grant_id = "x".repeat(5000),
            2 => bad.context.account_id = "x".repeat(1025),
            3 => bad.context.authorization_epoch = "0001".into(),
            _ => bad.context.authorization_view = "-1".into(),
        }
        assert!(
            store
                .submit_guarded_merge(bad, None, || Ok(()))
                .await
                .is_err()
        );
    }
    let mut bad = r.clone();
    bad.context.expected_head = "d".repeat(40);
    assert!(p.arm(&a, &bad).is_err());
    let mut bad = r.clone();
    bad.method = MergeMethod::Rebase;
    assert!(p.arm(&a, &bad).is_err());
    let mut bad = r.clone();
    bad.confirm_inspected_head = false;
    assert!(
        store
            .submit_guarded_merge(bad, None, || Ok(()))
            .await
            .is_err()
    );
    assert!(
        store
            .command_receipt("a", &r.command_id)
            .await
            .unwrap()
            .is_none()
    );
    http.join().unwrap();
    store.close().await.unwrap();
}

#[tokio::test]
async fn consent_expiring_inside_held_preflight_response_is_refused_before_attempt() {
    let (_dir, store, a) = setup().await;
    let (p, _, http) = server_hook(
        vec![ok(repo()), ok(pull(false)), ok(repo()), ok(pull(false))],
        Arc::new(|index, clock| {
            if index == 4 {
                clock.advance(60);
            }
        }),
    );
    let r = preview(&store, &a, &p).await;
    admit(&store, &a, &p, r.clone()).await;
    assert!(claim(&store, &a, &p, &r.command_id).await.request.is_none());
    let c = store.delivery_command("a", &r.command_id).await.unwrap();
    assert_eq!(c.state, DeliveryState::Conflict);
    assert_eq!(c.attempt_count, 0);
    assert_eq!(http.join().unwrap().len(), 4);
    store.close().await.unwrap();
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
        // A held native vault completes after the preview's monotonic deadline.
        self.0.advance(60);
        Ok(Some(token()))
    }
}
#[tokio::test]
async fn consent_expiring_inside_runtime_vault_load_cannot_start_provider_preflight() {
    let (_dir, store, a) = setup().await;
    store
        .stage_credential("a", "synthetic-merge-reference")
        .await
        .unwrap();
    let a = store
        .commit_account_credential(
            RemoteAccount {
                authorization_epoch: "2".into(),
                ..a
            },
            "synthetic-merge-reference",
        )
        .await
        .unwrap();
    fixtures::project(&store, &a).await;
    reseed_head(&store, &a).await;
    observed(&store, &a, "open", HEAD).await;
    let (p, clock, http) = server(vec![ok(repo()), ok(pull(false))]);
    let r = preview(&store, &a, &p).await;
    admit(&store, &a, &p, r.clone()).await;
    let mut registry = crate::providers::ProviderRegistry::default();
    registry
        .register(Arc::new(
            crate::providers::github::GithubProvider::new().unwrap(),
        ))
        .unwrap();
    registry.install_guarded_merge(p).unwrap();
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
    assert_eq!(http.join().unwrap().len(), 2);
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn provider_denials_keep_status_and_quota_without_becoming_replayable() {
    for (status, expected) in [
        (403, DeliveryState::Rejected),
        (405, DeliveryState::Rejected),
        (409, DeliveryState::Conflict),
        (422, DeliveryState::Rejected),
    ] {
        let (_dir, store, a) = setup().await;
        let (p, _, http) = server(vec![
            ok(repo()),
            ok(pull(false)),
            ok(repo()),
            ok(pull(false)),
            (
                status,
                Some(json!({"message":"denied"})),
                "Retry-After: 120\r\n".into(),
            ),
        ]);
        let r = preview(&store, &a, &p).await;
        admit(&store, &a, &p, r.clone()).await;
        let call = claim(&store, &a, &p, &r.command_id).await.request.unwrap();
        let report = p.dispatch(&token(), call.clone()).await;
        assert_eq!(report.account_cooldown_seconds, Some(120));
        complete(&store, &a, &p, &call.command, Some(call.attempt), &report).await;
        let c = store.delivery_command("a", &r.command_id).await.unwrap();
        assert_eq!(c.state, expected);
        assert_eq!(c.attempt_count, 1);
        assert_eq!(
            store.item("a", "pull").await.unwrap().item.unwrap().state,
            "open"
        );
        assert_eq!(
            http.join()
                .unwrap()
                .iter()
                .filter(|r| r.starts_with("PUT "))
                .count(),
            1
        );
        store.close().await.unwrap();
    }
}

#[tokio::test]
async fn wrong_repository_pull_head_or_merge_receipt_cannot_confirm_cached_state() {
    for field in ["repository", "pull", "head", "merge"] {
        let (_dir, store, a) = setup().await;
        let mut wrong = pull(true);
        match field {
            "repository" => wrong["base"]["repo"]["id"] = json!(2),
            "pull" => wrong["id"] = json!(68),
            "head" => wrong["head"]["sha"] = json!("d".repeat(40)),
            _ => wrong["merge_commit_sha"] = json!("d".repeat(40)),
        }
        let (p, _, http) = server(vec![
            ok(repo()),
            ok(pull(false)),
            ok(repo()),
            ok(pull(false)),
            ok(json!({"merged":true,"sha":MERGE,"message":"merged"})),
            ok(wrong),
        ]);
        let r = preview(&store, &a, &p).await;
        admit(&store, &a, &p, r.clone()).await;
        let call = claim(&store, &a, &p, &r.command_id).await.request.unwrap();
        let report = p.dispatch(&token(), call.clone()).await;
        complete(&store, &a, &p, &call.command, Some(call.attempt), &report).await;
        let c = store.delivery_command("a", &r.command_id).await.unwrap();
        let (request, _) = store
            .claim_reconciliation(&c, &a, p.as_ref(), &time())
            .await
            .unwrap();
        let result = p.reconcile(&token(), request.unwrap()).await;
        assert!(
            !matches!(
                result,
                Ok(DeliveryReport {
                    outcome: DeliveryOutcome::Confirmed(_),
                    ..
                })
            ),
            "{field}"
        );
        assert_eq!(
            store.item("a", "pull").await.unwrap().item.unwrap().state,
            "open"
        );
        http.join().unwrap();
        store.close().await.unwrap();
    }
}

#[tokio::test]
async fn accepted_attempt_reconciles_under_fresh_reconnected_authority_without_new_consent_or_put()
{
    let (_dir, store, a) = setup().await;
    let (p, _, http) = server(vec![
        ok(repo()),
        ok(pull(false)),
        ok(repo()),
        ok(pull(false)),
        ok(json!({"merged":true,"sha":MERGE,"message":"merged"})),
        ok(pull(true)),
    ]);
    let r = preview(&store, &a, &p).await;
    admit(&store, &a, &p, r.clone()).await;
    let call = claim(&store, &a, &p, &r.command_id).await.request.unwrap();
    let report = p.dispatch(&token(), call.clone()).await;
    complete(&store, &a, &p, &call.command, Some(call.attempt), &report).await;
    store
        .stage_credential("a", "reconnected-merge-fixture")
        .await
        .unwrap();
    let a = store
        .commit_account_credential(
            RemoteAccount {
                authorization_epoch: "2".into(),
                ..a
            },
            "reconnected-merge-fixture",
        )
        .await
        .unwrap();
    fixtures::project(&store, &a).await;
    reseed_head(&store, &a).await;
    observed(&store, &a, "open", HEAD).await;
    assert!(matches!(
        reconcile(&store, &a, &p, &r.command_id).await.outcome,
        DeliveryOutcome::Confirmed(_)
    ));
    assert_eq!(
        store.item("a", "pull").await.unwrap().item.unwrap().state,
        "merged"
    );
    assert_eq!(
        http.join()
            .unwrap()
            .iter()
            .filter(|r| r.starts_with("PUT "))
            .count(),
        1
    );
    store.close().await.unwrap();
}

async fn reseed_head(store: &Store, account: &RemoteAccount) {
    let mut item = store.item(&account.id, "pull").await.unwrap().item.unwrap();
    item.head_oid = Some(HEAD.into());
    let run_id = store
        .begin_sync(
            &account.id,
            &account.authorization_epoch,
            "repo:repo:pull_request",
        )
        .await
        .unwrap();
    store
        .apply_page(PageCommit {
            account_id: account.id.clone(),
            authorization_epoch: account.authorization_epoch.clone(),
            scope: "repo:repo:pull_request".into(),
            run_id,
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
}

#[tokio::test]
async fn restored_pending_merge_is_immutable_quarantined_and_cannot_gain_online_authority() {
    use crate::recovery::{RecoverySession, RestoreChoice};
    let (dir, store, a) = setup().await;
    let (p, _, http) = server(vec![ok(repo()), ok(pull(false))]);
    let r = preview(&store, &a, &p).await;
    admit(&store, &a, &p, r.clone()).await;
    let original = store
        .delivery_command("a", &r.command_id)
        .await
        .unwrap()
        .payload;
    let backup = dir.path().join("backup.db");
    store.backup_to(&backup).await.unwrap();
    store.close().await.unwrap();
    let recovery = RecoverySession::prepare(dir.path().join("text.db"), &backup)
        .await
        .unwrap();
    let id = recovery.preview().confirmation_id.clone();
    recovery
        .confirm(&id, RestoreChoice::ReplaceCurrentData)
        .unwrap();
    let store = Store::open(dir.path().join("text.db")).await.unwrap();
    let mut account = store.account("a").await.unwrap();
    account.state = AccountState::Active;
    account.authorization_epoch =
        (account.authorization_epoch.parse::<u64>().unwrap() + 1).to_string();
    let account = store.upsert_account(account).await.unwrap();
    let c = store.delivery_command("a", &r.command_id).await.unwrap();
    assert_eq!(c.payload, original);
    assert!(c.reconcile_only());
    assert_eq!(c.attempt_count, 0);
    assert!(
        store
            .claim_preparation(&c, &account, p.as_ref(), &time())
            .await
            .is_err()
    );
    assert!(
        store
            .submit_guarded_merge(r, None, || Ok(()))
            .await
            .is_err()
    );
    assert_eq!(http.join().unwrap().len(), 2);
    store.close().await.unwrap();
}

#[tokio::test]
async fn lost_put_cold_restart_is_only_read_reconciled_and_never_replayed() {
    let (dir, store, a) = setup().await;
    let (p, _, http) = server(vec![
        ok(repo()),
        ok(pull(false)),
        ok(repo()),
        ok(pull(false)),
        (200, None, String::new()),
    ]);
    let r = preview(&store, &a, &p).await;
    admit(&store, &a, &p, r.clone()).await;
    let call = claim(&store, &a, &p, &r.command_id).await.request.unwrap();
    let report = p.dispatch(&token(), call.clone()).await;
    complete(&store, &a, &p, &call.command, Some(call.attempt), &report).await;
    assert_eq!(
        store
            .delivery_command("a", &r.command_id)
            .await
            .unwrap()
            .state,
        DeliveryState::Unknown
    );
    store.close().await.unwrap();
    drop(store);
    drop(p);
    assert_eq!(
        http.join()
            .unwrap()
            .iter()
            .filter(|r| r.starts_with("PUT "))
            .count(),
        1
    );
    let store = Store::open(dir.path().join("text.db")).await.unwrap();
    let (cold, _, http) = server(vec![ok(pull(false)), ok(pull(true))]);
    assert!(matches!(
        reconcile(&store, &a, &cold, &r.command_id).await.outcome,
        DeliveryOutcome::Unknown
    ));
    assert!(matches!(
        reconcile(&store, &a, &cold, &r.command_id).await.outcome,
        DeliveryOutcome::Confirmed(_)
    ));
    assert!(
        http.join()
            .unwrap()
            .iter()
            .all(|r| r.starts_with("GET /repositories/1/pulls/67 "))
    );
    store.close().await.unwrap();
}

#[tokio::test]
async fn accepted_receipt_is_strict_and_cannot_be_promoted_or_cross_bound() {
    let (_dir, store, a) = setup().await;
    let (p, _, http) = server(vec![
        ok(repo()),
        ok(pull(false)),
        ok(repo()),
        ok(pull(false)),
        ok(json!({"merged":true,"sha":MERGE,"message":"merged"})),
    ]);
    let r = preview(&store, &a, &p).await;
    admit(&store, &a, &p, r.clone()).await;
    let call = claim(&store, &a, &p, &r.command_id).await.request.unwrap();
    let report = p.dispatch(&token(), call.clone()).await;
    let DeliveryOutcome::Accepted(proof) = report.outcome else {
        panic!("strict accepted receipt")
    };
    assert!(p.validate_evidence(&call.command, EvidencePurpose::Accepted, &proof));
    for purpose in [EvidencePurpose::Confirmed, EvidencePurpose::SafeRetry] {
        assert!(!p.validate_evidence(&call.command, purpose, &proof));
    }
    for field in [
        "actor_id",
        "authorization_epoch",
        "command_hash",
        "extra",
        "native_inbox",
    ] {
        let mut e: serde_json::Value = serde_json::from_slice(&proof.payload).unwrap();
        match field {
            "actor_id" => e[field] = json!(""),
            "authorization_epoch" => e[field] = json!("2"),
            "native_inbox" => e["frame"]["subject"][field] = json!(null),
            _ => e[field] = json!("tampered"),
        }
        // Canonical field order is part of the proof; arbitrary JSON never
        // replaces a native proof even if its apparent values look plausible.
        let bad = OperationEvidence {
            payload: serde_json::to_vec(&e).unwrap(),
            ..proof.clone()
        };
        assert!(
            !p.validate_evidence(&call.command, EvidencePurpose::Accepted, &bad),
            "{field}"
        );
    }
    let mut e: Evidence = decode(&proof.payload).unwrap();
    e.authorization_epoch = "2".into();
    let bad = OperationEvidence {
        payload: encode(&e).unwrap(),
        ..proof.clone()
    };
    assert!(!p.validate_evidence(&call.command, EvidencePurpose::Accepted, &bad));
    http.join().unwrap();
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
async fn preview_rechecks_concurrent_successful_probe_quota_after_held_vault_before_http() {
    let (_dir, store, a) = setup().await;
    store
        .stage_credential("a", "preview-held-vault")
        .await
        .unwrap();
    let a = store
        .commit_account_credential(
            RemoteAccount {
                authorization_epoch: "2".into(),
                ..a
            },
            "preview-held-vault",
        )
        .await
        .unwrap();
    fixtures::project(&store, &a).await;
    reseed_head(&store, &a).await;
    observed(&store, &a, "open", HEAD).await;
    let (p, _, http) = server(vec![]);
    let mut registry = crate::providers::ProviderRegistry::default();
    registry
        .register(Arc::new(
            crate::providers::github::GithubProvider::new().unwrap(),
        ))
        .unwrap();
    registry.install_guarded_merge(p).unwrap();
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
        async move {
            runtime
                .preview_guarded_merge(GuardedMergeQuery {
                    account_id: "a".into(),
                    subject_id: "pull".into(),
                })
                .await
        }
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
    let error = pending.await.unwrap().unwrap_err();
    assert_eq!(error.code, ErrorCode::RateLimited);
    assert!(http.join().unwrap().is_empty());
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn successful_preview_quota_never_issues_a_grant_and_retains_account_deadline() {
    let (_dir, store, a) = setup().await;
    let (p, _, http) = server(vec![
        ok(repo()),
        (200, Some(pull(false)), "Retry-After: 120\r\n".into()),
    ]);
    let (_, frame) = store
        .guarded_merge_frame(&GuardedMergeQuery {
            account_id: "a".into(),
            subject_id: "pull".into(),
        })
        .await
        .unwrap();
    let error = p.preview(&token(), &a, &frame).await.unwrap_err();
    assert_eq!(error.kind, crate::providers::ProviderErrorKind::RateLimited);
    assert_eq!(error.account_cooldown_seconds, Some(120));
    assert_eq!(http.join().unwrap().len(), 2);
    store.close().await.unwrap();
}
