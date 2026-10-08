use super::*;
use crate::CommandRecoveryReplaceRequest;
use crate::credentials::SecretToken;
use crate::delivery::*;
use crate::providers::{
    inbox_actions::{InboxHttp, InboxPolicy},
    transport::GithubHttp,
};
use crate::storage::delivery::DeliveryCompletion;
use chrono::Utc;
use std::io::{Read, Write};
const FIRST: &str = "123e4567-e89b-12d3-a456-426614174000";
const SECOND: &str = "123e4567-e89b-12d3-a456-426614174001";
pub(crate) async fn setup(
    provider: ProviderKind,
) -> (tempfile::TempDir, Store, RemoteAccount, RemoteItem) {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path().join("inbox.db")).await.unwrap();
    let account = store
        .upsert_account(RemoteAccount {
            id: "a".into(),
            provider,
            host: if provider == ProviderKind::Github {
                "github.com"
            } else {
                "gitlab.com"
            }
            .into(),
            actor_id: "1".into(),
            login: "actor".into(),
            display_name: None,
            authorization_epoch: "1".into(),
            state: AccountState::Active,
            notifications_supported: true,
        })
        .await
        .unwrap();
    store
        .stage_credential(&account.id, "synthetic-reference")
        .await
        .unwrap();
    let account = store
        .commit_account_credential(
            RemoteAccount {
                authorization_epoch: "2".into(),
                ..account
            },
            "synthetic-reference",
        )
        .await
        .unwrap();
    let repo = RemoteRepository {
        id: "repo".into(),
        account_id: account.id.clone(),
        provider_id: "42".into(),
        full_name: "owner/repo".into(),
        name: "repo".into(),
        web_url: format!("https://{}/owner/repo", account.host),
        description: None,
        default_branch: None,
        selected: false,
    };
    let item = RemoteItem {
        id: "notification".into(),
        account_id: account.id.clone(),
        repository_id: Some(repo.id.clone()),
        provider_id: "101".into(),
        kind: RemoteItemKind::Notification,
        number: None,
        title: "Activity".into(),
        body: None,
        body_omitted: false,
        author: None,
        web_url: None,
        state: if provider == ProviderKind::Github {
            "PullRequest"
        } else {
            "pending"
        }
        .into(),
        updated_at: "2026-10-07T00:00:00Z".into(),
        head_oid: None,
        is_draft: None,
        reason: Some("mention".into()),
        unread: (provider == ProviderKind::Github).then_some(true),
        native_inbox: Some(if provider == ProviderKind::Github {
            NativeInboxState::Notification { unread: true }
        } else {
            NativeInboxState::Todo {
                completion: TodoCompletion::Pending,
                action: "mentioned".into(),
                target_type: "MergeRequest".into(),
            }
        }),
    };
    let run = store
        .begin_sync(&account.id, &account.authorization_epoch, "notifications")
        .await
        .unwrap();
    store
        .apply_page(PageCommit {
            account_id: account.id.clone(),
            authorization_epoch: account.authorization_epoch.clone(),
            scope: "notifications".into(),
            run_id: run,
            repositories: vec![repo],
            items: vec![item.clone()],
            endpoint_aliases: vec![],
            next_cursor: None,
            etag: None,
            last_modified: None,
            not_modified: false,
            complete: true,
            observed_at: item.updated_at.clone(),
        })
        .await
        .unwrap();
    (dir, store, account, item)
}
async fn request(store: &Store, action: ProviderInboxAction) -> QueueProviderInboxActionRequest {
    let s = store
        .provider_inbox_actions(
            ProviderInboxActionsQuery {
                account_id: "a".into(),
                subject_id: "notification".into(),
            },
            true,
        )
        .await
        .unwrap();
    QueueProviderInboxActionRequest {
        account_id: s.account_id,
        authorization_epoch: s.authorization_epoch,
        authorization_view: s.authorization_view,
        subject_id: s.subject_id,
        expected_activity_version: s.activity_version,
        command_id: FIRST.into(),
        action,
        activity_policy: ProviderInboxActivityPolicy::BestEffortCurrentItem,
    }
}
async fn update(store: &Store, account: &RemoteAccount, item: RemoteItem) {
    let run = store
        .begin_sync(&account.id, &account.authorization_epoch, "notifications")
        .await
        .unwrap();
    store
        .apply_page(PageCommit {
            account_id: account.id.clone(),
            authorization_epoch: account.authorization_epoch.clone(),
            scope: "notifications".into(),
            run_id: run,
            repositories: vec![],
            items: vec![item.clone()],
            endpoint_aliases: vec![],
            next_cursor: None,
            etag: None,
            last_modified: None,
            not_modified: false,
            complete: true,
            observed_at: item.updated_at,
        })
        .await
        .unwrap();
}
#[tokio::test]
async fn native_semantics_and_uuid_receipt_survive_offline_reopen() {
    for (provider, action) in [
        (ProviderKind::Github, ProviderInboxAction::MarkRead),
        (ProviderKind::Gitlab, ProviderInboxAction::MarkDone),
    ] {
        let (dir, store, account, mut item) = setup(provider).await;
        let r = request(&store, action).await;
        let receipt = store.queue_provider_inbox_action(r.clone()).await.unwrap();
        assert!(!receipt.duplicate);
        let effective = store.item("a", "notification").await.unwrap();
        assert_eq!(effective.pending_intent.unwrap().commands.len(), 1);
        let projected = effective.item.unwrap();
        if provider == ProviderKind::Github {
            assert_eq!(projected.state, "PullRequest");
            assert_eq!(projected.unread, Some(false));
            assert_eq!(
                projected.native_inbox,
                Some(NativeInboxState::Notification { unread: false })
            );
        } else {
            assert_eq!(projected.state, "done");
            assert_eq!(projected.unread, None);
            assert!(matches!(
                projected.native_inbox,
                Some(NativeInboxState::Todo {
                    completion: TodoCompletion::Done,
                    ..
                })
            ));
        }
        item.updated_at = "2026-10-07T01:00:00Z".into();
        update(&store, &account, item).await;
        let duplicate = store.queue_provider_inbox_action(r.clone()).await.unwrap();
        assert!(duplicate.duplicate);
        assert_eq!(duplicate.revision, receipt.revision);
        store.close().await.unwrap();
        let reopened = Store::open(dir.path().join("inbox.db")).await.unwrap();
        assert!(
            reopened
                .queue_provider_inbox_action(r)
                .await
                .unwrap()
                .duplicate
        );
        reopened.close().await.unwrap();
    }
}
#[tokio::test]
async fn stale_activity_foreign_view_and_unsupported_action_never_admit() {
    let (_dir, store, account, mut item) = setup(ProviderKind::Github).await;
    let r = request(&store, ProviderInboxAction::MarkRead).await;
    for mut bad in [r.clone(), r.clone(), r.clone()] {
        if bad.command_id == FIRST {
            bad.authorization_view = "foreign".into();
        }
        assert!(store.queue_provider_inbox_action(bad).await.is_err());
    }
    let mut done = r.clone();
    done.action = ProviderInboxAction::MarkDone;
    assert_eq!(
        store
            .queue_provider_inbox_action(done)
            .await
            .unwrap_err()
            .code,
        ErrorCode::Unsupported
    );
    item.updated_at = "2026-10-07T02:00:00Z".into();
    update(&store, &account, item).await;
    assert_eq!(
        store.queue_provider_inbox_action(r).await.unwrap_err().code,
        ErrorCode::StaleView
    );
    let rows: i64 = sqlx::query_scalar("SELECT count(*) FROM commands")
        .fetch_one(&store.inner.readers)
        .await
        .unwrap();
    assert_eq!(rows, 0);
    store.close().await.unwrap();
}
#[tokio::test]
async fn one_pending_action_and_exact_conflicting_uuid_are_bounded() {
    let (_dir, store, _, _) = setup(ProviderKind::Github).await;
    let r = request(&store, ProviderInboxAction::MarkRead).await;
    store.queue_provider_inbox_action(r.clone()).await.unwrap();
    let mut second = request(&store, ProviderInboxAction::MarkRead).await;
    second.command_id = SECOND.into();
    assert_eq!(
        store
            .queue_provider_inbox_action(second)
            .await
            .unwrap_err()
            .code,
        ErrorCode::Unsupported
    );
    let mut changed = r;
    changed.expected_activity_version = "0".repeat(64);
    assert!(store.queue_provider_inbox_action(changed).await.is_err());
    let s = store
        .provider_inbox_actions(
            ProviderInboxActionsQuery {
                account_id: "a".into(),
                subject_id: "notification".into(),
            },
            true,
        )
        .await
        .unwrap();
    assert_eq!(
        s.actions[0].reason,
        Some(ProviderInboxActionReason::PendingCommand)
    );
    store.close().await.unwrap();
}
fn policy(status: &str, body: &str) -> (InboxPolicy, std::thread::JoinHandle<String>) {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let base = reqwest::Url::parse(&format!("http://{}/", listener.local_addr().unwrap())).unwrap();
    let response = format!(
        "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    let task = std::thread::spawn(move || {
        let (mut s, _) = listener.accept().unwrap();
        s.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
        s.set_write_timeout(Some(Duration::from_secs(2))).unwrap();
        let mut b = [0; 8192];
        let n = s.read(&mut b).unwrap();
        s.write_all(response.as_bytes()).unwrap();
        String::from_utf8(b[..n].to_vec()).unwrap()
    });
    (
        InboxPolicy {
            instance: ProviderInstance::public(ProviderKind::Github),
            http: InboxHttp::Github(GithubHttp::for_test_base(base).unwrap()),
        },
        task,
    )
}
fn time() -> DeliveryTime {
    let now = Utc::now().to_rfc3339();
    DeliveryTime {
        now: now.clone(),
        command_now: now,
    }
}
#[tokio::test]
async fn exact_patch_confirmation_retires_overlay_and_preserves_subject_type() {
    let (_dir, store, account, _) = setup(ProviderKind::Github).await;
    let r = request(&store, ProviderInboxAction::MarkRead).await;
    store.queue_provider_inbox_action(r).await.unwrap();
    let (policy, task) = policy("205 Reset Content", "");
    let command = store.delivery_command("a", FIRST).await.unwrap();
    let preparation = prepare_bytes(
        &store,
        &account,
        &command,
        Observation {
            updated_at: "2026-10-07T00:00:00Z".into(),
            applied: false,
        },
    )
    .await;
    let request = store
        .claim_delivery(&command, &account, &policy, &preparation, &time())
        .await
        .unwrap()
        .request
        .unwrap();
    let report = policy
        .dispatch(
            &SecretToken::new("synthetic_only".into()).unwrap(),
            request.clone(),
        )
        .await;
    assert!(matches!(report.outcome, DeliveryOutcome::Confirmed(_)));
    let now = Utc::now().to_rfc3339();
    store
        .complete_delivery(
            &request.command,
            &policy,
            DeliveryCompletion {
                account: &account,
                attempt: Some(request.attempt),
                report: &report,
                now: &now,
                next: &now,
            },
        )
        .await
        .unwrap();
    let snapshot = store.item("a", "notification").await.unwrap();
    assert!(snapshot.pending_intent.is_none());
    let item = snapshot.item.unwrap();
    assert_eq!(item.unread, Some(false));
    assert_eq!(item.state, "PullRequest");
    assert_eq!(
        item.native_inbox,
        Some(NativeInboxState::Notification { unread: false })
    );
    assert!(
        task.join()
            .unwrap()
            .starts_with("PATCH /notifications/threads/101 ")
    );
    store.close().await.unwrap();
}
#[tokio::test]
async fn malformed_success_is_unknown_and_held_new_activity_cannot_finalize() {
    for (status, body) in [("200 OK", "{}"), ("205 Reset Content", "")] {
        let (_dir, store, account, mut item) = setup(ProviderKind::Github).await;
        let r = request(&store, ProviderInboxAction::MarkRead).await;
        store.queue_provider_inbox_action(r).await.unwrap();
        let (policy, task) = policy(status, body);
        let command = store.delivery_command("a", FIRST).await.unwrap();
        let preparation = prepare_bytes(
            &store,
            &account,
            &command,
            Observation {
                updated_at: item.updated_at.clone(),
                applied: false,
            },
        )
        .await;
        let request = store
            .claim_delivery(&command, &account, &policy, &preparation, &time())
            .await
            .unwrap()
            .request
            .unwrap();
        let report = policy
            .dispatch(
                &SecretToken::new("synthetic_only".into()).unwrap(),
                request.clone(),
            )
            .await;
        if status.starts_with("200") {
            assert!(matches!(report.outcome, DeliveryOutcome::Unknown));
        } else {
            item.updated_at = "2026-10-07T03:00:00Z".into();
            update(&store, &account, item).await;
            let now = Utc::now().to_rfc3339();
            assert!(
                store
                    .complete_delivery(
                        &request.command,
                        &policy,
                        DeliveryCompletion {
                            account: &account,
                            attempt: Some(request.attempt),
                            report: &report,
                            now: &now,
                            next: &now
                        }
                    )
                    .await
                    .is_err()
            );
        }
        task.join().unwrap();
        store.close().await.unwrap();
    }
}
#[test]
fn native_payload_rejects_changed_shape() {
    let p = Payload {
        instance: ProviderInstance::public(ProviderKind::Github).id,
        actor: "1".into(),
        native_id: "101".into(),
        project: "42".into(),
        updated_at: "2026-10-07T00:00:00Z".into(),
        activity: "a".repeat(64),
        view: "1".into(),
        action: ProviderInboxAction::MarkRead,
        subject_type: "PullRequest".into(),
        native_action: String::new(),
    };
    let r = QueueProviderInboxActionRequest {
        account_id: "a".into(),
        authorization_epoch: "1".into(),
        authorization_view: "1".into(),
        subject_id: "notification".into(),
        expected_activity_version: "a".repeat(64),
        command_id: FIRST.into(),
        action: ProviderInboxAction::MarkRead,
        activity_policy: ProviderInboxActivityPolicy::BestEffortCurrentItem,
    };
    let sealed = seal(&r, p.clone(), Some("repo".into())).unwrap();
    assert_eq!(Payload::parse(sealed.payload_bytes()).unwrap(), p);
    let mut bytes = sealed.payload_bytes().to_vec();
    bytes.push(0);
    assert!(Payload::parse(&bytes).is_err());
    assert!(Payload::parse(&vec![0; 8193]).is_err());
}

async fn prepare_bytes(
    store: &Store,
    account: &RemoteAccount,
    command: &DeliveryCommand,
    observation: Observation,
) -> Vec<u8> {
    let mut tx = store.inner.readers.begin().await.unwrap();
    let frame = frame_in(&mut tx, account, &command.target_id)
        .await
        .unwrap();
    tx.commit().await.unwrap();
    serde_json::to_vec(&Proof {
        payload: Payload::parse(&command.payload).unwrap(),
        frame,
        observation,
    })
    .unwrap()
}
fn idle_policy() -> InboxPolicy {
    InboxPolicy {
        instance: ProviderInstance::public(ProviderKind::Github),
        http: InboxHttp::Github(
            GithubHttp::for_test_base(reqwest::Url::parse("http://127.0.0.1:9/").unwrap()).unwrap(),
        ),
    }
}
#[tokio::test]
async fn already_read_preflight_confirms_without_any_dispatch_attempt() {
    let (_dir, store, account, item) = setup(ProviderKind::Github).await;
    let r = request(&store, ProviderInboxAction::MarkRead).await;
    store.queue_provider_inbox_action(r).await.unwrap();
    let command = store.delivery_command("a", FIRST).await.unwrap();
    let preparation = prepare_bytes(
        &store,
        &account,
        &command,
        Observation {
            updated_at: item.updated_at,
            applied: true,
        },
    )
    .await;
    let claim = store
        .claim_delivery(&command, &account, &idle_policy(), &preparation, &time())
        .await
        .unwrap();
    assert!(claim.request.is_none());
    let after = store.delivery_command("a", FIRST).await.unwrap();
    assert_eq!(after.state, DeliveryState::Confirmed);
    assert_eq!(after.attempt_count, 0);
    assert_eq!(
        store
            .item("a", "notification")
            .await
            .unwrap()
            .item
            .unwrap()
            .unread,
        Some(false)
    );
    store.close().await.unwrap();
}
#[tokio::test]
async fn preflight_activity_conflicts_then_explicit_replacement_uses_new_native_base() {
    let (_dir, store, account, mut item) = setup(ProviderKind::Github).await;
    let r = request(&store, ProviderInboxAction::MarkRead).await;
    store.queue_provider_inbox_action(r).await.unwrap();
    let command = store.delivery_command("a", FIRST).await.unwrap();
    let preparation = prepare_bytes(
        &store,
        &account,
        &command,
        Observation {
            updated_at: "2026-10-07T02:00:00Z".into(),
            applied: false,
        },
    )
    .await;
    let policy = idle_policy();
    let claim = store
        .claim_delivery(&command, &account, &policy, &preparation, &time())
        .await
        .unwrap();
    assert!(claim.request.is_none());
    assert_eq!(
        store.delivery_command("a", FIRST).await.unwrap().state,
        DeliveryState::Conflict
    );
    item.updated_at = "2026-10-07T02:00:00Z".into();
    update(&store, &account, item).await;
    let detail = store
        .command_recovery_detail("a", FIRST, Some(&policy))
        .await
        .unwrap();
    assert!(detail.can_replace);
    assert!(detail.reason.unwrap().contains("concurrent"));
    let receipt = store
        .command_recovery_replace(
            CommandRecoveryReplaceRequest {
                context: detail.context,
                action_id: uuid::Uuid::new_v4().to_string(),
                new_command_id: SECOND.into(),
                fields: vec![],
            },
            Some(&policy),
        )
        .await
        .unwrap();
    assert_eq!(receipt.replacement_id.as_deref(), Some(SECOND));
    let new = store.delivery_command("a", SECOND).await.unwrap();
    assert_eq!(
        Payload::parse(&new.payload).unwrap().updated_at,
        "2026-10-07T02:00:00.000000000Z"
    );
    assert_eq!(
        store.delivery_command("a", FIRST).await.unwrap().state,
        DeliveryState::Superseded
    );
    store.close().await.unwrap();
}
#[tokio::test]
async fn held_epoch_cannot_claim_or_publish_confirmation() {
    let (_dir, store, account, _) = setup(ProviderKind::Github).await;
    let r = request(&store, ProviderInboxAction::MarkRead).await;
    store.queue_provider_inbox_action(r).await.unwrap();
    let command = store.delivery_command("a", FIRST).await.unwrap();
    let preparation = prepare_bytes(
        &store,
        &account,
        &command,
        Observation {
            updated_at: "2026-10-07T00:00:00Z".into(),
            applied: false,
        },
    )
    .await;
    store
        .upsert_account(RemoteAccount {
            authorization_epoch: "3".into(),
            ..account.clone()
        })
        .await
        .unwrap();
    assert!(
        store
            .claim_delivery(&command, &account, &idle_policy(), &preparation, &time())
            .await
            .is_err()
    );
    assert_eq!(
        store
            .delivery_command("a", FIRST)
            .await
            .unwrap()
            .attempt_count,
        0
    );
    store.close().await.unwrap();
}

#[derive(Default)]
struct SyntheticVault;
impl crate::credentials::CredentialVault for SyntheticVault {
    fn store(
        &self,
        _: &str,
        _: &SecretToken,
    ) -> std::result::Result<(), crate::credentials::CredentialError> {
        Ok(())
    }
    fn load(
        &self,
        _: &str,
    ) -> std::result::Result<Option<SecretToken>, crate::credentials::CredentialError> {
        Ok(Some(SecretToken::new("synthetic_only".into()).unwrap()))
    }
    fn delete(&self, _: &str) -> std::result::Result<(), crate::credentials::CredentialError> {
        Ok(())
    }
}
struct ReadAdapter(ProviderKind);
#[async_trait::async_trait]
impl crate::providers::CollaborationProvider for ReadAdapter {
    fn kind(&self) -> ProviderKind {
        self.0
    }
    async fn probe(
        &self,
        _: &SecretToken,
    ) -> std::result::Result<crate::providers::VerifiedAccount, crate::providers::ProviderError>
    {
        panic!("No provider probe in isolated operation test")
    }
    async fn fetch_page(
        &self,
        _: &SecretToken,
        _: crate::providers::FeedRequest,
    ) -> std::result::Result<crate::providers::FetchPage, crate::providers::ProviderError> {
        panic!("No feed request in isolated operation test")
    }
}
fn multi_policy(
    provider: ProviderKind,
    responses: Vec<(&str, &str, String)>,
) -> (InboxPolicy, std::thread::JoinHandle<Vec<String>>) {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let base = reqwest::Url::parse(&format!("http://{}/", listener.local_addr().unwrap())).unwrap();
    let responses=responses.into_iter().map(|(status,headers,body)|format!("HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n{headers}\r\n{body}",body.len())).collect::<Vec<_>>();
    let task = std::thread::spawn(move || {
        let mut requests = Vec::new();
        for response in responses {
            let start = std::time::Instant::now();
            let (mut s, _) = loop {
                match listener.accept() {
                    Ok(v) => break v,
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        assert!(
                            start.elapsed() < Duration::from_secs(8),
                            "Expected finite provider request"
                        );
                        std::thread::sleep(Duration::from_millis(5));
                    }
                    Err(e) => panic!("Fixture accept: {e}"),
                }
            };
            // Darwin inherits O_NONBLOCK from the listener on accept. The
            // bounded request read must wait for bytes on every platform.
            s.set_nonblocking(false).unwrap();
            s.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
            s.set_write_timeout(Some(Duration::from_secs(2))).unwrap();
            let mut b = [0u8; 8192];
            let n = s.read(&mut b).unwrap();
            let _ = s.write_all(response.as_bytes());
            requests.push(String::from_utf8(b[..n].to_vec()).unwrap());
        }
        requests
    });
    let http = if provider == ProviderKind::Github {
        InboxHttp::Github(GithubHttp::for_test_base(base).unwrap())
    } else {
        InboxHttp::Gitlab {
            client: crate::providers::transport::mutations::mutation_client(&base).unwrap(),
            base,
        }
    };
    (
        InboxPolicy {
            instance: ProviderInstance::public(provider),
            http,
        },
        task,
    )
}
async fn native_runtime(
    store: Arc<Store>,
    account: &RemoteAccount,
    policy: InboxPolicy,
) -> crate::CollaborationRuntime {
    if store
        .credential_reference(&account.id)
        .await
        .unwrap()
        .is_none()
    {
        store
            .stage_credential(&account.id, "synthetic-reference")
            .await
            .unwrap();
        store
            .commit_account_credential(account.clone(), "synthetic-reference")
            .await
            .unwrap();
    }
    let mut registry = crate::providers::ProviderRegistry::default();
    registry
        .register(Arc::new(ReadAdapter(account.provider)))
        .unwrap();
    let policy = Arc::new(policy);
    registry
        .register_delivery(&policy.instance, policy.clone())
        .unwrap();
    registry
        .register_recovery(&policy.instance, policy.clone())
        .unwrap();
    crate::CollaborationRuntime::with_registry(store, Arc::new(SyntheticVault), registry)
}
fn gh_observation(unread: bool) -> String {
    serde_json::json!({"id":"101","repository":{"id":42},"subject":{"type":"PullRequest"},"unread":unread,"updated_at":"2026-10-07T00:00:00Z"}).to_string()
}
fn gl_observation(done: bool) -> serde_json::Value {
    serde_json::json!({"id":101,"project":{"id":42},"target_type":"MergeRequest","action_name":"mentioned","state":if done{"done"}else{"pending"},"updated_at":if done{"2026-10-07T01:00:00Z"}else{"2026-10-07T00:00:00Z"}})
}
#[tokio::test]
async fn real_native_worker_keeps_successful_preflight_quota_across_restart_without_attempt() {
    let (dir, store, account, _) = setup(ProviderKind::Github).await;
    let store = Arc::new(store);
    let (policy, task) = multi_policy(
        ProviderKind::Github,
        vec![("200 OK", "Retry-After: 300\r\n", gh_observation(true))],
    );
    let runtime = native_runtime(store.clone(), &account, policy).await;
    let r = request(&store, ProviderInboxAction::MarkRead).await;
    runtime.queue_provider_inbox_action(r).await.unwrap();
    assert!(runtime.run_delivery_next().await.unwrap());
    let command = store.delivery_command("a", FIRST).await.unwrap();
    assert_eq!(command.attempt_count, 0);
    assert_eq!(command.state, DeliveryState::Queued);
    assert_eq!(task.join().unwrap().len(), 1);
    runtime.shutdown().await.unwrap();
    let store = Arc::new(Store::open(dir.path().join("inbox.db")).await.unwrap());
    let runtime = native_runtime(store.clone(), &account, idle_policy()).await;
    assert!(!runtime.run_delivery_next().await.unwrap());
    assert_eq!(
        store
            .delivery_command("a", FIRST)
            .await
            .unwrap()
            .attempt_count,
        0
    );
    runtime.shutdown().await.unwrap();
}
#[tokio::test]
async fn real_gitlab_worker_confirms_explicit_done_without_inventing_unread() {
    let (_dir, store, account, _) = setup(ProviderKind::Gitlab).await;
    let store = Arc::new(store);
    let (policy, task) = multi_policy(
        ProviderKind::Gitlab,
        vec![
            (
                "200 OK",
                "",
                serde_json::json!([gl_observation(false)]).to_string(),
            ),
            ("200 OK", "", gl_observation(true).to_string()),
        ],
    );
    let runtime = native_runtime(store.clone(), &account, policy).await;
    let r = request(&store, ProviderInboxAction::MarkDone).await;
    runtime.queue_provider_inbox_action(r).await.unwrap();
    assert!(runtime.run_delivery_next().await.unwrap());
    let command = store.delivery_command("a", FIRST).await.unwrap();
    assert_eq!(command.state, DeliveryState::Confirmed);
    assert_eq!(command.attempt_count, 1);
    let snapshot = store.item("a", "notification").await.unwrap();
    assert!(snapshot.pending_intent.is_none());
    let item = snapshot.item.unwrap();
    assert_eq!(item.state, "done");
    assert_eq!(item.unread, None);
    assert!(matches!(
        item.native_inbox,
        Some(NativeInboxState::Todo {
            completion: TodoCompletion::Done,
            ..
        })
    ));
    let requests = task.join().unwrap();
    assert!(requests[0].starts_with("GET /todos?project_id=42&state=pending&per_page=100&page=1 "));
    assert!(requests[1].starts_with("POST /todos/101/mark_as_done "));
    runtime.shutdown().await.unwrap();
}
#[tokio::test]
async fn unknown_write_reconciles_after_cold_restart_with_get_only() {
    let (dir, store, account, _) = setup(ProviderKind::Github).await;
    let store = Arc::new(store);
    let (policy, task) = multi_policy(
        ProviderKind::Github,
        vec![
            ("200 OK", "", gh_observation(true)),
            ("200 OK", "", "{}".into()),
        ],
    );
    let runtime = native_runtime(store.clone(), &account, policy).await;
    let r = request(&store, ProviderInboxAction::MarkRead).await;
    runtime.queue_provider_inbox_action(r).await.unwrap();
    assert!(runtime.run_delivery_next().await.unwrap());
    assert_eq!(
        store.delivery_command("a", FIRST).await.unwrap().state,
        DeliveryState::Unknown
    );
    assert_eq!(task.join().unwrap().len(), 2);
    runtime.shutdown().await.unwrap();
    let store = Arc::new(Store::open(dir.path().join("inbox.db")).await.unwrap());
    let (policy, task) = multi_policy(
        ProviderKind::Github,
        vec![("200 OK", "", gh_observation(false))],
    );
    // Only advance synthetic durable retry eligibility; no production clock or
    // credential state is inspected. New process still has no dispatch authority.
    {
        let mut writer = store.inner.writer.acquire().await.unwrap();
        sqlx::query("UPDATE command_delivery SET generation=generation+1,next_action_at='2000-01-01T00:00:00Z' WHERE account_id='a' AND command_id=?").bind(FIRST).execute(&mut *writer).await.unwrap();
    }
    let runtime = native_runtime(store.clone(), &account, policy).await;
    assert!(runtime.run_delivery_next().await.unwrap());
    let after = store.delivery_command("a", FIRST).await.unwrap();
    assert_eq!(after.state, DeliveryState::Confirmed);
    assert_eq!(after.attempt_count, 1);
    let requests = task.join().unwrap();
    assert_eq!(requests.len(), 1);
    assert!(requests[0].starts_with("GET /notifications/threads/101 "));
    runtime.shutdown().await.unwrap();
}
#[tokio::test]
async fn invalid_bounded_requests_fail_before_any_mutation() {
    let (_dir, store, _, _) = setup(ProviderKind::Github).await;
    let r = request(&store, ProviderInboxAction::MarkRead).await;
    let before = store.revision().await.unwrap();
    let mut cases = Vec::new();
    for field in 0..7 {
        let mut invalid = r.clone();
        match field {
            0 => invalid.account_id = "x".repeat(1025),
            1 => invalid.subject_id = "x".repeat(1025),
            2 => invalid.authorization_epoch = "01".into(),
            3 => invalid.authorization_view = "1".repeat(20),
            4 => invalid.command_id = "not-a-uuid".into(),
            5 => invalid.expected_activity_version = "A".repeat(64),
            _ => invalid.expected_activity_version = "f".repeat(65),
        }
        cases.push(invalid);
    }
    for invalid in cases {
        assert_eq!(
            store
                .queue_provider_inbox_action(invalid)
                .await
                .unwrap_err()
                .code,
            ErrorCode::InvalidInput
        );
    }
    for query in [
        ProviderInboxActionsQuery {
            account_id: "x".repeat(1025),
            subject_id: "notification".into(),
        },
        ProviderInboxActionsQuery {
            account_id: "a".into(),
            subject_id: "\0".into(),
        },
    ] {
        assert_eq!(
            store
                .provider_inbox_actions(query, true)
                .await
                .unwrap_err()
                .code,
            ErrorCode::InvalidInput
        );
    }
    assert_eq!(store.revision().await.unwrap(), before);
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM commands")
        .fetch_one(&store.inner.readers)
        .await
        .unwrap();
    assert_eq!(count, 0);
    let effects: i64 = sqlx::query_scalar("SELECT count(*) FROM command_effects")
        .fetch_one(&store.inner.readers)
        .await
        .unwrap();
    assert_eq!(effects, 0);
    store.close().await.unwrap();
}
