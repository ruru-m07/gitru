use super::*;
use crate::delivery::*;
use crate::providers::ProviderErrorKind;
use crate::providers::github::label_sets::GithubLabelSetPolicy;
use crate::runtime::detail_tests::fixtures;
use crate::storage::delivery::DeliveryCompletion;
use crate::*;
use serde_json::json;
use std::io::{Read, Write};

const HEAD: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

fn label(id: &str, name: &str, color: &str) -> DetailLabel {
    DetailLabel {
        provider_id: Some(id.into()),
        name: name.into(),
        color: Some(color.into()),
    }
}

async fn observe(store: &Store, account: &RemoteAccount, subject: &str, labels: Vec<DetailLabel>) {
    let item = store
        .item(&account.id, subject)
        .await
        .unwrap()
        .item
        .unwrap();
    let lease = store
        .begin_detail(
            &account.id,
            &account.authorization_epoch,
            subject,
            DetailFacet::Body,
        )
        .await
        .unwrap();
    let source = match item.kind {
        RemoteItemKind::Issue => "github/issue-detail/2026-03-10",
        RemoteItemKind::PullRequest => "github/pull-detail/2026-03-10",
        _ => unreachable!(),
    };
    let updated = "2026-10-08T00:00:00Z".to_string();
    store
        .apply_detail(DetailCommit {
            reconciliation: DetailReconciliation::full_history(),
            account_id: account.id.clone(),
            authorization_epoch: account.authorization_epoch.clone(),
            authorization_view: lease.authorization_view,
            instance_id: lease.instance_id,
            subject_id: subject.into(),
            facet: DetailFacet::Body,
            run_id: lease.run_id,
            request_cursor: lease.next_cursor,
            body: fixtures::known(Some("saved body")),
            metadata: Some(ResourceMetadataObservation {
                kind: item.kind.clone(),
                values: ResourceMetadataValues {
                    title: Some(item.title.clone()),
                    state: Some(item.state.clone()),
                    updated_at: Some(updated.clone()),
                    labels,
                    head: item.head_oid.as_ref().map(|oid| DetailBranch {
                        name: "feature".into(),
                        oid: oid.clone(),
                        repository: None,
                    }),
                    ..Default::default()
                },
                fields: [
                    MetadataField::Title,
                    MetadataField::State,
                    MetadataField::UpdatedAt,
                    MetadataField::Labels,
                ]
                .into_iter()
                .chain((item.kind == RemoteItemKind::PullRequest).then_some(MetadataField::Head))
                .map(|field| MetadataObservedField {
                    field,
                    state: DetailValueState::Known,
                })
                .collect(),
                source: MetadataSource {
                    source: source.into(),
                    adapter_version: 1,
                    provider_updated_at: Some(updated.clone()),
                    observed_at: updated.clone(),
                },
            }),
            subject_binding: Some(DetailSubjectBinding {
                repository_id: "repo".into(),
                repository_provider_id: "1".into(),
                provider_id: item.provider_id,
                number: item.number,
                kind: item.kind,
                head_oid: item.head_oid,
            }),
            check_context: None,
            review_context: None,
            entries: vec![],
            source: DetailSource {
                source: source.into(),
                adapter_version: 1,
                field_mask: vec![DetailField::Body],
                provider_updated_at: Some(updated.clone()),
                observed_at: updated,
            },
            next_cursor: None,
            etag: None,
            not_modified: false,
            whole_scope: true,
            complete: true,
            freshness_seconds: 180,
        })
        .await
        .unwrap();
}

async fn setup() -> (tempfile::TempDir, Store, RemoteAccount) {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path().join("labels.db")).await.unwrap();
    let account = fixtures::seed(&store, "a").await;
    let mut pull = store.item("a", "pull").await.unwrap().item.unwrap();
    pull.head_oid = Some(HEAD.into());
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
            items: vec![pull.clone()],
            endpoint_aliases: vec![],
            next_cursor: None,
            etag: None,
            last_modified: None,
            not_modified: false,
            complete: true,
            observed_at: "2026-10-08T00:00:00Z".into(),
        })
        .await
        .unwrap();
    let mut issue = pull;
    issue.id = "issue".into();
    issue.provider_id = "9007199254740998".into();
    issue.number = Some("68".into());
    issue.kind = RemoteItemKind::Issue;
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
            observed_at: "2026-10-08T00:00:00Z".into(),
        })
        .await
        .unwrap();
    observe(&store, &account, "pull", vec![label("1", "bug", "aa0000")]).await;
    observe(
        &store,
        &account,
        "issue",
        vec![label("2", "feature", "00aa00")],
    )
    .await;
    (dir, store, account)
}

fn identity(id: &str, name: &str, color: &str) -> LabelIdentity {
    LabelIdentity {
        provider_id: id.into(),
        name: name.into(),
        color: Some(color.into()),
    }
}

async fn request(store: &Store) -> LabelSetRequest {
    let snapshot = store.label_set_snapshot("a", "pull").await.unwrap();
    LabelSetRequest {
        context: snapshot.context.unwrap(),
        command_id: Uuid::new_v4().to_string(),
        add_labels: vec![identity("2", "feature", "00aa00")],
        remove_labels: vec![identity("1", "bug", "aa0000")],
        accept_best_effort: true,
    }
}

#[tokio::test]
async fn cached_catalog_admission_and_cold_exact_retry_preserve_typed_effect() {
    let (dir, store, _) = setup().await;
    let snapshot = store.label_set_snapshot("a", "pull").await.unwrap();
    assert_eq!(snapshot.availability, LabelSetAvailability::Available);
    assert!(!snapshot.catalog_complete);
    assert!(!snapshot.catalog_truncated);
    assert_eq!(
        snapshot.canonical_labels,
        vec![identity("1", "bug", "aa0000")]
    );
    assert_eq!(
        snapshot.available_labels,
        vec![
            identity("1", "bug", "aa0000"),
            identity("2", "feature", "00aa00")
        ]
    );
    let request = request(&store).await;
    let receipt = store.submit_label_set(request.clone()).await.unwrap();
    let pending = store.label_set_snapshot("a", "pull").await.unwrap();
    assert_eq!(pending.reason, Some(LabelSetReason::PendingIntent));
    assert_eq!(
        pending.effective_labels,
        vec![identity("2", "feature", "00aa00")]
    );
    assert_eq!(
        pending.pending_intent.unwrap().commands[0].fields,
        vec![IntentField::Labels]
    );
    let payload = store
        .delivery_command("a", &request.command_id)
        .await
        .unwrap()
        .payload;
    store.close().await.unwrap();
    let store = Store::open(dir.path().join("labels.db")).await.unwrap();
    let retry = store.submit_label_set(request.clone()).await.unwrap();
    assert!(retry.duplicate);
    assert_eq!(retry.admitted_revision, receipt.admitted_revision);
    assert_eq!(
        store
            .delivery_command("a", &request.command_id)
            .await
            .unwrap()
            .payload,
        payload
    );
    let mut changed = request;
    changed.remove_labels.clear();
    assert!(store.submit_label_set(changed).await.is_err());
    store.close().await.unwrap();
}

#[tokio::test]
async fn pending_label_snapshot_redacts_authority_after_body_access_denial() {
    let (_dir, store, account) = setup().await;
    let request = request(&store).await;
    store.submit_label_set(request).await.unwrap();
    store
        .set_sync_status(
            "a",
            &account.authorization_epoch,
            "detail:pull:body",
            SyncStatus {
                state: SyncState::Error,
                last_success_at: None,
                next_retry_at: None,
                error: Some(CollaborationError::new(
                    ErrorCode::PermissionDenied,
                    "synthetic Body access denial",
                )),
            },
        )
        .await
        .unwrap();
    let snapshot = store.label_set_snapshot("a", "pull").await.unwrap();
    assert_eq!(snapshot.availability, LabelSetAvailability::Unavailable);
    assert_eq!(snapshot.reason, Some(LabelSetReason::PendingIntent));
    assert!(snapshot.pending_intent.is_some());
    assert!(snapshot.context.is_none());
    assert!(snapshot.canonical_labels.is_empty());
    assert!(snapshot.effective_labels.is_empty());
    assert!(snapshot.available_labels.is_empty());
    store.close().await.unwrap();
}

#[tokio::test]
async fn additions_require_an_exact_unambiguous_cached_catalog_identity() {
    let (_dir, store, _) = setup().await;
    let mut renamed = request(&store).await;
    renamed.add_labels[0].name = "renamed".into();
    assert_eq!(
        store.submit_label_set(renamed).await.unwrap_err().code,
        ErrorCode::StaleView
    );
    let mut reassigned = request(&store).await;
    reassigned.add_labels[0].provider_id = "3".into();
    assert_eq!(
        store.submit_label_set(reassigned).await.unwrap_err().code,
        ErrorCode::StaleView
    );
    store.close().await.unwrap();
}

#[test]
fn legacy_v1_effect_bytes_remain_exact_when_labels_are_absent() {
    let old = br#"{"title":"old","body":null,"state":null,"unread":null}"#;
    let effect: crate::effective::ItemIntentPatch = serde_json::from_slice(old).unwrap();
    assert!(effect.labels.is_none());
    assert_eq!(serde_json::to_vec(&effect).unwrap(), old);
}

enum Reply {
    Json(u16, serde_json::Value),
    JsonCooldown(u16, serde_json::Value, u64),
    Close,
}

fn server(replies: Vec<Reply>) -> (GithubLabelSetPolicy, std::thread::JoinHandle<Vec<String>>) {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let url = reqwest::Url::parse(&format!("http://{}/", listener.local_addr().unwrap())).unwrap();
    let handle = std::thread::spawn(move || {
        let mut requests = vec![];
        for reply in replies {
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
            let mut stream = loop {
                match listener.accept() {
                    Ok((stream, _)) => break stream,
                    Err(error)
                        if error.kind() == std::io::ErrorKind::WouldBlock
                            && std::time::Instant::now() < deadline =>
                    {
                        std::thread::sleep(std::time::Duration::from_millis(5));
                    }
                    Err(error) => panic!("finite server accept: {error}"),
                }
            };
            stream.set_nonblocking(false).unwrap();
            stream
                .set_read_timeout(Some(std::time::Duration::from_secs(5)))
                .unwrap();
            let mut request = vec![];
            let mut chunk = [0; 4096];
            loop {
                let count = stream.read(&mut chunk).unwrap();
                if count == 0 {
                    break;
                }
                request.extend_from_slice(&chunk[..count]);
                if let Some(end) = request.windows(4).position(|window| window == b"\r\n\r\n") {
                    let headers = String::from_utf8_lossy(&request[..end]);
                    let length = headers
                        .lines()
                        .find_map(|line| {
                            line.to_ascii_lowercase()
                                .strip_prefix("content-length:")
                                .and_then(|value| value.trim().parse::<usize>().ok())
                        })
                        .unwrap_or(0);
                    if request.len() >= end + 4 + length {
                        break;
                    }
                }
                assert!(request.len() < 100_000);
            }
            requests.push(String::from_utf8(request).unwrap());
            match reply {
                Reply::Json(status, value) => {
                    let body = value.to_string();
                    write!(
                        stream,
                        "HTTP/1.1 {status} Fixture\r\nContent-Length: {}\r\nContent-Type: application/json\r\nConnection: close\r\n\r\n{body}",
                        body.len()
                    )
                    .unwrap();
                }
                Reply::JsonCooldown(status, value, seconds) => {
                    let body = value.to_string();
                    write!(
                        stream,
                        "HTTP/1.1 {status} Fixture\r\nContent-Length: {}\r\nContent-Type: application/json\r\nRetry-After: {seconds}\r\nConnection: close\r\n\r\n{body}",
                        body.len()
                    )
                    .unwrap();
                }
                Reply::Close => {}
            }
        }
        requests
    });
    (GithubLabelSetPolicy::for_test_base(url), handle)
}

fn pull_response(labels: Vec<serde_json::Value>) -> serde_json::Value {
    json!({
        "id": 9007199254740997_u64,
        "number": 67,
        "url": "https://api.github.com/repos/owner/project/pulls/67",
        "html_url": "https://github.com/owner/project/pull/67",
        "title": "Remote title",
        "body": "Remote body",
        "state": "open",
        "merged": false,
        "updated_at": "2026-10-08T01:00:00Z",
        "draft": false,
        "labels": labels,
        "head": {"ref":"feature","sha":HEAD,"repo":null},
        "base": {"ref":"main","sha":"b".repeat(40),"repo":{"id":1,"full_name":"owner/project","html_url":"https://github.com/owner/project"}}
    })
}

fn native_label(id: u64, name: &str, color: &str) -> serde_json::Value {
    json!({"id":id,"name":name,"color":color})
}

fn token() -> crate::credentials::SecretToken {
    crate::credentials::SecretToken::new("synthetic_label_test".into()).unwrap()
}

fn time() -> DeliveryTime {
    let now = chrono::Utc::now().to_rfc3339();
    DeliveryTime {
        now: now.clone(),
        command_now: now,
    }
}

async fn ready_dispatch(
    store: &Store,
    account: &RemoteAccount,
    policy: &GithubLabelSetPolicy,
    command_id: &str,
) -> DispatchRequest {
    let command = store.delivery_command("a", command_id).await.unwrap();
    let (request, _) = store
        .claim_preparation(&command, account, policy, &time())
        .await
        .unwrap();
    let request = request.unwrap();
    let preparation = policy.prepare(&token(), &request).await.unwrap();
    store
        .claim_delivery(
            &request.command,
            account,
            policy,
            &preparation.bytes,
            &time(),
        )
        .await
        .unwrap()
        .request
        .unwrap()
}

async fn complete(
    store: &Store,
    account: &RemoteAccount,
    policy: &GithubLabelSetPolicy,
    dispatch: &DispatchRequest,
    report: &DeliveryReport,
) {
    let now = chrono::Utc::now().to_rfc3339();
    store
        .complete_delivery(
            &dispatch.command,
            policy,
            DeliveryCompletion {
                account,
                attempt: Some(dispatch.attempt),
                report,
                now: &now,
                next: &now,
            },
        )
        .await
        .unwrap();
}

#[tokio::test]
async fn exact_readback_confirms_delta_preserves_provider_fields_and_retires_effect() {
    let (_dir, store, account) = setup().await;
    let request = request(&store).await;
    store.submit_label_set(request.clone()).await.unwrap();
    let final_labels = vec![native_label(2, "feature", "00aa00")];
    let (policy, server) = server(vec![
        Reply::Json(200, pull_response(vec![native_label(1, "bug", "aa0000")])),
        Reply::Json(200, native_label(2, "feature", "00aa00")),
        Reply::Json(200, json!([])),
        Reply::Json(200, json!([])),
        Reply::Json(200, pull_response(final_labels)),
    ]);
    let dispatch = ready_dispatch(&store, &account, &policy, &request.command_id).await;
    let report = policy.dispatch(&token(), dispatch.clone()).await;
    assert!(matches!(report.outcome, DeliveryOutcome::Confirmed(_)));
    complete(&store, &account, &policy, &dispatch, &report).await;
    let snapshot = store.label_set_snapshot("a", "pull").await.unwrap();
    assert_eq!(snapshot.availability, LabelSetAvailability::Available);
    assert!(snapshot.pending_intent.is_none());
    assert_eq!(
        snapshot.canonical_labels,
        vec![identity("2", "feature", "00aa00")]
    );
    assert_eq!(snapshot.effective_labels, snapshot.canonical_labels);
    let item = store.item("a", "pull").await.unwrap().item.unwrap();
    assert_eq!(item.title, "Remote title");
    assert_eq!(item.body.as_deref(), Some("Remote body"));
    let requests = server.join().unwrap();
    assert_eq!(requests.len(), 5);
    assert!(requests[1].starts_with("GET /repositories/1/labels/feature "));
    assert!(requests[2].ends_with("{\"labels\":[\"feature\"]}"));
    assert!(requests[3].starts_with("DELETE /repositories/1/issues/67/labels/bug "));
    store.close().await.unwrap();
}

#[tokio::test]
async fn partial_success_and_lost_response_remain_unknown_and_never_continue_writes() {
    for fail_after_add_response in [false, true] {
        let (_dir, store, account) = setup().await;
        let request = request(&store).await;
        store.submit_label_set(request.clone()).await.unwrap();
        let mut replies = vec![
            Reply::Json(200, pull_response(vec![native_label(1, "bug", "aa0000")])),
            Reply::Json(200, native_label(2, "feature", "00aa00")),
        ];
        if fail_after_add_response {
            replies.push(Reply::Json(200, json!([])));
            replies.push(Reply::Close);
        } else {
            replies.push(Reply::Close);
        }
        let expected = replies.len();
        let (policy, server) = server(replies);
        let dispatch = ready_dispatch(&store, &account, &policy, &request.command_id).await;
        let report = policy.dispatch(&token(), dispatch.clone()).await;
        assert!(matches!(report.outcome, DeliveryOutcome::Unknown));
        assert!(report.provider_error.is_some());
        complete(&store, &account, &policy, &dispatch, &report).await;
        let command = store
            .delivery_command("a", &request.command_id)
            .await
            .unwrap();
        assert_eq!(command.state, DeliveryState::Unknown);
        assert!(command.reconcile_only());
        let requests = server.join().unwrap();
        assert_eq!(requests.len(), expected);
        assert!(requests[0].starts_with("GET /repositories/1/pulls/67 "));
        assert!(requests[1].starts_with("GET /repositories/1/labels/feature "));
        assert!(requests[2].starts_with("POST /repositories/1/issues/67/labels "));
        if fail_after_add_response {
            assert!(requests[3].starts_with("DELETE /repositories/1/issues/67/labels/bug "));
        }
        store.close().await.unwrap();
    }
}

#[tokio::test]
async fn cooldown_barriers_stop_preflight_and_post_write_request_chains() {
    for root_barrier in [true, false] {
        let (_dir, store, account) = setup().await;
        let request = request(&store).await;
        store.submit_label_set(request.clone()).await.unwrap();
        let replies = if root_barrier {
            vec![Reply::JsonCooldown(
                200,
                pull_response(vec![native_label(1, "bug", "aa0000")]),
                120,
            )]
        } else {
            vec![
                Reply::Json(200, pull_response(vec![native_label(1, "bug", "aa0000")])),
                Reply::JsonCooldown(200, native_label(2, "feature", "00aa00"), 120),
            ]
        };
        let expected = replies.len();
        let (policy, server) = server(replies);
        let command = store
            .delivery_command("a", &request.command_id)
            .await
            .unwrap();
        let (preflight, _) = store
            .claim_preparation(&command, &account, &policy, &time())
            .await
            .unwrap();
        let error = match policy.prepare(&token(), &preflight.unwrap()).await {
            Err(error) => error,
            Ok(_) => panic!("cooldown barrier must stop preparation"),
        };
        assert_eq!(error.kind, ProviderErrorKind::RateLimited);
        assert_eq!(error.account_cooldown_seconds, Some(120));
        let requests = server.join().unwrap();
        assert_eq!(requests.len(), expected);
        assert!(requests[0].starts_with("GET /repositories/1/pulls/67 "));
        if !root_barrier {
            assert!(requests[1].starts_with("GET /repositories/1/labels/feature "));
        }
        store.close().await.unwrap();
    }

    {
        let (_dir, store, account) = setup().await;
        let request = request(&store).await;
        store.submit_label_set(request.clone()).await.unwrap();
        let (policy, server) = server(vec![
            Reply::Json(200, pull_response(vec![native_label(1, "bug", "aa0000")])),
            Reply::JsonCooldown(200, json!({"malformed": true}), 120),
        ]);
        let command = store
            .delivery_command("a", &request.command_id)
            .await
            .unwrap();
        let (preflight, _) = store
            .claim_preparation(&command, &account, &policy, &time())
            .await
            .unwrap();
        let error = match policy.prepare(&token(), &preflight.unwrap()).await {
            Err(error) => error,
            Ok(_) => panic!("malformed point evidence must stop preparation"),
        };
        assert_eq!(error.kind, ProviderErrorKind::InvalidResponse);
        assert_eq!(error.account_cooldown_seconds, Some(120));
        let requests = server.join().unwrap();
        assert_eq!(requests.len(), 2);
        assert!(requests[1].starts_with("GET /repositories/1/labels/feature "));
        store.close().await.unwrap();
    }

    for stop_after_delete in [false, true] {
        let (_dir, store, account) = setup().await;
        let request = request(&store).await;
        store.submit_label_set(request.clone()).await.unwrap();
        let mut replies = vec![
            Reply::Json(200, pull_response(vec![native_label(1, "bug", "aa0000")])),
            Reply::Json(200, native_label(2, "feature", "00aa00")),
        ];
        if stop_after_delete {
            replies.push(Reply::Json(200, json!([])));
            replies.push(Reply::JsonCooldown(200, json!([]), 75));
        } else {
            replies.push(Reply::JsonCooldown(200, json!([]), 75));
        }
        let expected = replies.len();
        let (policy, server) = server(replies);
        let dispatch = ready_dispatch(&store, &account, &policy, &request.command_id).await;
        let report = policy.dispatch(&token(), dispatch).await;
        assert!(matches!(report.outcome, DeliveryOutcome::Unknown));
        assert_eq!(report.account_cooldown_seconds, Some(75));
        let requests = server.join().unwrap();
        assert_eq!(requests.len(), expected);
        assert!(requests[2].starts_with("POST /repositories/1/issues/67/labels "));
        if stop_after_delete {
            assert!(requests[3].starts_with("DELETE /repositories/1/issues/67/labels/bug "));
        }
        store.close().await.unwrap();
    }
}

#[tokio::test]
async fn renamed_or_reassigned_name_conflicts_before_any_write() {
    for remote in [
        native_label(2, "Feature", "00aa00"),
        native_label(3, "feature", "00aa00"),
    ] {
        let (_dir, store, account) = setup().await;
        let request = request(&store).await;
        store.submit_label_set(request.clone()).await.unwrap();
        let (policy, server) = server(vec![Reply::Json(
            200,
            pull_response(vec![native_label(1, "bug", "aa0000"), remote]),
        )]);
        let command = store
            .delivery_command("a", &request.command_id)
            .await
            .unwrap();
        let (preflight, _) = store
            .claim_preparation(&command, &account, &policy, &time())
            .await
            .unwrap();
        let preflight = preflight.unwrap();
        let preparation = policy.prepare(&token(), &preflight).await.unwrap();
        let claim = store
            .claim_delivery(
                &preflight.command,
                &account,
                &policy,
                &preparation.bytes,
                &time(),
            )
            .await
            .unwrap();
        assert!(claim.request.is_none());
        assert_eq!(
            store
                .delivery_command("a", &request.command_id)
                .await
                .unwrap()
                .state,
            DeliveryState::Conflict
        );
        assert_eq!(server.join().unwrap().len(), 1);
        store.close().await.unwrap();
    }
}

#[tokio::test]
async fn label_intent_backup_restore_preserves_bytes_and_never_reauthorizes_dispatch() {
    use crate::recovery::{RecoverySession, RestoreChoice};

    let (dir, store, account) = setup().await;
    let request = request(&store).await;
    store.submit_label_set(request.clone()).await.unwrap();
    let payload = store
        .delivery_command("a", &request.command_id)
        .await
        .unwrap()
        .payload;
    let backup = dir.path().join("backup.db");
    store.backup_to(&backup).await.unwrap();
    store.close().await.unwrap();

    let restored_path = dir.path().join("labels.db");
    let session = RecoverySession::prepare(&restored_path, &backup)
        .await
        .unwrap();
    let confirmation = session.preview().confirmation_id.clone();
    session
        .confirm(&confirmation, RestoreChoice::ReplaceCurrentData)
        .unwrap();
    let store = Store::open(&restored_path).await.unwrap();
    let mut active = store.account("a").await.unwrap();
    active.state = AccountState::Active;
    active.authorization_epoch =
        (active.authorization_epoch.parse::<u64>().unwrap() + 1).to_string();
    let active = store.upsert_account(active).await.unwrap();
    assert_ne!(active.authorization_epoch, account.authorization_epoch);
    let command = store
        .delivery_command("a", &request.command_id)
        .await
        .unwrap();
    assert_eq!(command.payload, payload);
    assert!(command.reconcile_only());
    assert_eq!(command.attempt_count, 0);
    let policy = GithubLabelSetPolicy::new().unwrap();
    assert_eq!(
        store
            .claim_preparation(&command, &active, &policy, &time())
            .await
            .unwrap_err()
            .code,
        ErrorCode::StaleView
    );
    assert!(store.submit_label_set(request).await.is_err());
    store.close().await.unwrap();
}
