use super::*;
use crate::providers::github::comment_send::GithubCommentPolicy;
use crate::runtime::detail_tests::fixtures;
use crate::{DetailFacet, DetailQuery};
use crate::{comment_send::*, delivery::*};
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
    json!({"id":9007199254740997_u64,"number":67,"url":"https://api.github.com/repos/owner/project/pulls/67","html_url":"https://github.com/owner/project/pull/67","title":"Title","body":null,"state":"open","merged":false,"updated_at":"2026-10-07T23:00:00Z","head":{"ref":"feature","sha":"a".repeat(40),"repo":null},"base":{"ref":"main","sha":"b".repeat(40),"repo":{"id":1,"full_name":"owner/project","html_url":"https://github.com/owner/project"}}})
}
fn created(body: &str) -> serde_json::Value {
    json!({"id":9007199254740995_u64,"url":"https://api.github.com/repos/owner/project/issues/comments/9007199254740995","html_url":"https://github.com/owner/project/pull/67#issuecomment-9007199254740995","issue_url":"https://api.github.com/repos/owner/project/issues/67","body":body,"user":{"id":7,"login":"author"},"created_at":"2026-10-08T00:00:00Z","updated_at":"2026-10-08T00:00:00Z"})
}
fn server_headers(
    responses: Vec<(u16, Option<serde_json::Value>, String)>,
) -> (GithubCommentPolicy, std::thread::JoinHandle<Vec<String>>) {
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
    (GithubCommentPolicy::for_test_base(url), handle)
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
async fn save(store: &Store, body: &str) -> CommentDraftSnapshot {
    let old = store.comment_draft("a", "pull").await.unwrap();
    let a = store.account("a").await.unwrap();
    store
        .save_comment_draft(SaveCommentDraftRequest {
            account_id: "a".into(),
            subject_id: "pull".into(),
            authorization_epoch: a.authorization_epoch,
            authorization_view: old.authorization_view,
            expected_generation: old.generation,
            body: body.into(),
        })
        .await
        .unwrap()
}
fn request(s: &CommentDraftSnapshot) -> SendCommentRequest {
    SendCommentRequest {
        context: s.context.clone().unwrap(),
        draft_generation: s.generation.clone(),
        command_id: Uuid::new_v4().to_string(),
        accept_background_delivery: true,
    }
}
async fn claim_dispatch(
    store: &Store,
    a: &RemoteAccount,
    p: &GithubCommentPolicy,
    id: &str,
) -> DispatchRequest {
    let c = store.delivery_command("a", id).await.unwrap();
    let (r, _) = store.claim_preparation(&c, a, p, &time()).await.unwrap();
    let r = r.unwrap();
    let prepared = p.prepare(&token(), &r).await.unwrap();
    store
        .claim_delivery(&r.command, a, p, &prepared.bytes, &time())
        .await
        .unwrap()
        .request
        .unwrap()
}
async fn complete(
    store: &Store,
    a: &RemoteAccount,
    p: &GithubCommentPolicy,
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
fn query() -> CreatedCommentQuery {
    CreatedCommentQuery {
        account_id: "a".into(),
        subject_id: "pull".into(),
        cursor: None,
        limit: 20,
    }
}
#[tokio::test]
async fn comment_send_private_notes_never_prefill_and_unchanged_save_keeps_generation() {
    let (dir, store, _) = setup().await;
    store
        .save_draft(LocalDraft {
            account_id: "a".into(),
            subject_id: "pull".into(),
            body: "PRIVATE note".into(),
            generation: "0".into(),
        })
        .await
        .unwrap();
    let empty = store.comment_draft("a", "pull").await.unwrap();
    assert_eq!(empty.body, "");
    assert_eq!(empty.generation, "0");
    let first = save(&store, "public comment").await;
    let same = save(&store, "public comment").await;
    assert_eq!(first.generation, same.generation);
    assert_eq!(first.revision, same.revision);
    let mut stale = SaveCommentDraftRequest {
        account_id: "a".into(),
        subject_id: "pull".into(),
        authorization_epoch: "1".into(),
        authorization_view: same.authorization_view,
        expected_generation: "0".into(),
        body: "lost edit".into(),
    };
    assert_eq!(
        store
            .save_comment_draft(stale.clone())
            .await
            .unwrap_err()
            .code,
        ErrorCode::StaleView
    );
    stale.expected_generation = first.generation;
    stale.body = "x".repeat(16385);
    assert_eq!(
        store.save_comment_draft(stale).await.unwrap_err().code,
        ErrorCode::InvalidInput
    );
    store.close().await.unwrap();
    let store = Store::open(dir.path().join("comments.db")).await.unwrap();
    assert_eq!(
        store.comment_draft("a", "pull").await.unwrap().body,
        "public comment"
    );
    assert_eq!(
        store.draft("a", "pull").await.unwrap().unwrap().body,
        "PRIVATE note"
    );
    store.close().await.unwrap();
}
#[tokio::test]
async fn comment_send_saved_generation_is_exactly_once_across_new_uuid_and_edit() {
    let (dir, store, _) = setup().await;
    let s = save(&store, "comment").await;
    let r = request(&s);
    let receipt = store.send_comment(r.clone()).await.unwrap();
    assert!(!receipt.duplicate);
    assert!(store.send_comment(r.clone()).await.unwrap().duplicate);
    let mut another = r.clone();
    another.command_id = Uuid::new_v4().to_string();
    assert_eq!(
        store.send_comment(another).await.unwrap_err().code,
        ErrorCode::StaleView
    );
    let edited = save(&store, "another comment").await;
    assert_eq!(edited.reason, Some(CommentSendReason::PendingSubmission));
    assert_eq!(edited.submission.as_ref().unwrap().command_id, r.command_id);
    assert!(store.send_comment(r.clone()).await.unwrap().duplicate);
    store.close().await.unwrap();
    let store = Store::open(dir.path().join("comments.db")).await.unwrap();
    assert!(store.send_comment(r).await.unwrap().duplicate);
    assert_eq!(
        store.comment_draft("a", "pull").await.unwrap().body,
        "another comment"
    );
    store.close().await.unwrap();
}
#[tokio::test]
async fn comment_send_offline_authorship_survives_disconnect_and_missing_subject() {
    let (_dir, store, _) = setup().await;
    store.disconnect("a").await.unwrap();
    let a = store.account("a").await.unwrap();
    let s = store.comment_draft("a", "missing").await.unwrap();
    let saved = store
        .save_comment_draft(SaveCommentDraftRequest {
            account_id: "a".into(),
            subject_id: "missing".into(),
            authorization_epoch: a.authorization_epoch,
            authorization_view: s.authorization_view,
            expected_generation: "0".into(),
            body: "recover me".into(),
        })
        .await
        .unwrap();
    assert_eq!(saved.body, "recover me");
    assert_eq!(saved.reason, Some(CommentSendReason::AccountUnavailable));
    assert!(saved.context.is_none());
    store.close().await.unwrap();
}
#[tokio::test]
async fn comment_send_strong_201_commits_canonical_id_without_whole_comments_coverage() {
    let (_dir, store, a) = setup().await;
    let r = request(&save(&store, "public body").await);
    store.send_comment(r.clone()).await.unwrap();
    let (p, server) = server_headers(vec![
        (200, Some(target()), String::new()),
        (201, Some(created("public body")), String::new()),
    ]);
    let dispatch = claim_dispatch(&store, &a, &p, &r.command_id).await;
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
    let history = store.created_comments(query()).await.unwrap();
    assert_eq!(history.comments.len(), 1);
    assert_eq!(history.comments[0].provider_id, "9007199254740995");
    assert_eq!(history.comments[0].body, "public body");
    assert_eq!(
        store.comment_draft("a", "pull").await.unwrap().body,
        "public body"
    );
    let detail = store
        .detail(DetailQuery {
            account_id: "a".into(),
            subject_id: "pull".into(),
            facet: DetailFacet::Comments,
            cursor: None,
            limit: 10,
        })
        .await
        .unwrap();
    assert_ne!(detail.evidence.coverage.state, CoverageState::Complete);
    let mut empty = fixtures::commit(&store, &a, DetailFacet::Comments).await;
    empty.entries.clear();
    store.apply_detail(empty).await.unwrap();
    assert_eq!(
        store.created_comments(query()).await.unwrap().comments[0].provider_id,
        "9007199254740995"
    );
    let requests = server.join().unwrap();
    assert_eq!(requests.len(), 2);
    assert!(requests[1].starts_with("POST /repositories/1/issues/67/comments "));
    assert!(requests[1].contains("public body"));
    store.close().await.unwrap();
}
#[tokio::test]
async fn comment_send_lost_post_and_restart_never_match_text_or_post_again() {
    let (dir, store, a) = setup().await;
    let r = request(&save(&store, "same text").await);
    store.send_comment(r.clone()).await.unwrap();
    let (p, server) = server_headers(vec![
        (200, Some(target()), String::new()),
        (200, None, String::new()),
    ]);
    let dispatch = claim_dispatch(&store, &a, &p, &r.command_id).await;
    assert!(matches!(
        p.dispatch(&token(), dispatch).await.outcome,
        DeliveryOutcome::Unknown
    ));
    store.close().await.unwrap();
    let store = Store::open(dir.path().join("comments.db")).await.unwrap();
    let old = store.delivery_command("a", &r.command_id).await.unwrap();
    store.recover_delivery(&old, &now()).await.unwrap();
    let c = store.delivery_command("a", &r.command_id).await.unwrap();
    let (req, _) = store
        .claim_reconciliation(&c, &a, &p, &time())
        .await
        .unwrap();
    let req = req.unwrap();
    let report = p.reconcile(&token(), req.clone()).await.unwrap();
    assert!(matches!(report.outcome, DeliveryOutcome::Unknown));
    store
        .complete_delivery(
            &req.command,
            &p,
            super::delivery::DeliveryCompletion {
                account: &a,
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
        DeliveryState::Unknown
    );
    assert!(
        store
            .created_comments(query())
            .await
            .unwrap()
            .comments
            .is_empty()
    );
    assert_eq!(
        save(&store, "changed text cannot bypass").await.reason,
        Some(CommentSendReason::PendingSubmission)
    );
    assert_eq!(server.join().unwrap().len(), 2);
    store.close().await.unwrap();
}
#[tokio::test]
async fn comment_send_untrusted_201_fields_or_success_status_never_confirm() {
    for case in 0..6 {
        let (_dir, store, a) = setup().await;
        let r = request(&save(&store, "body").await);
        store.send_comment(r.clone()).await.unwrap();
        let mut created = created("body");
        let mut status = 201;
        match case {
            0 => created["id"] = json!(0),
            1 => {
                created["issue_url"] = json!("https://api.github.com/repos/other/project/issues/67")
            }
            2 => created["body"] = json!("different"),
            3 => created["user"]["id"] = json!(8),
            4 => created["html_url"] = json!("https://evil.invalid/"),
            _ => status = 200,
        };
        let (p, server) = server_headers(vec![
            (200, Some(target()), String::new()),
            (status, Some(created), String::new()),
        ]);
        let dispatch = claim_dispatch(&store, &a, &p, &r.command_id).await;
        let report = p.dispatch(&token(), dispatch.clone()).await;
        assert!(
            matches!(report.outcome, DeliveryOutcome::Unknown),
            "case {case}"
        );
        complete(&store, &a, &p, &dispatch, &report).await;
        assert!(
            store
                .created_comments(query())
                .await
                .unwrap()
                .comments
                .is_empty()
        );
        server.join().unwrap();
        store.close().await.unwrap();
    }
}
#[tokio::test]
async fn comment_send_stale_view_and_native_target_change_cannot_create_attempt() {
    let (_dir, store, a) = setup().await;
    let s = save(&store, "body").await;
    let mut bad = request(&s);
    bad.accept_background_delivery = false;
    assert!(store.send_comment(bad).await.is_err());
    let r = request(&s);
    store.send_comment(r.clone()).await.unwrap();
    let (p, server) = server_headers(vec![(200, Some(target()), String::new())]);
    let c = store.delivery_command("a", &r.command_id).await.unwrap();
    let (req, _) = store.claim_preparation(&c, &a, &p, &time()).await.unwrap();
    let req = req.unwrap();
    let prep = p.prepare(&token(), &req).await.unwrap();
    store.disconnect("a").await.unwrap();
    assert!(
        store
            .claim_delivery(&req.command, &a, &p, &prep.bytes, &time())
            .await
            .is_err()
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
async fn comment_send_atomic_link_failure_preserves_draft_and_rolls_back_command_and_revision() {
    let (_dir, store, _) = setup().await;
    let r = request(&save(&store, "keep my bytes").await);
    let before = store.revision().await.unwrap();
    {
        let mut writer = store.inner.writer.acquire().await.unwrap();
        sqlx::query("CREATE TRIGGER fail_comment_link BEFORE INSERT ON comment_submissions BEGIN SELECT RAISE(ABORT,'fixture link fault'); END").execute(&mut *writer).await.unwrap();
    }
    assert!(store.send_comment(r.clone()).await.is_err());
    assert!(store.delivery_command("a", &r.command_id).await.is_err());
    assert_eq!(store.revision().await.unwrap(), before);
    assert_eq!(
        store.comment_draft("a", "pull").await.unwrap().body,
        "keep my bytes"
    );
    {
        let mut writer = store.inner.writer.acquire().await.unwrap();
        sqlx::query("DROP TRIGGER fail_comment_link")
            .execute(&mut *writer)
            .await
            .unwrap();
    }
    store.send_comment(r).await.unwrap();
    store.close().await.unwrap();
}
#[tokio::test]
async fn comment_send_concurrent_distinct_ids_admit_only_one_saved_generation() {
    let (_dir, store, _) = setup().await;
    let s = save(&store, "once").await;
    let a = request(&s);
    let b = request(&s);
    let (a, b) = tokio::join!(store.send_comment(a), store.send_comment(b));
    assert_eq!(u8::from(a.is_ok()) + u8::from(b.is_ok()), 1);
    let mut writer = store.inner.writer.acquire().await.unwrap();
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM comment_submissions")
        .fetch_one(&mut *writer)
        .await
        .unwrap();
    assert_eq!(count, 1);
    drop(writer);
    store.close().await.unwrap();
}
#[tokio::test]
async fn comment_send_drafts_recover_locally_with_bounded_pages_after_disconnect() {
    let (_dir, store, _) = setup().await;
    save(&store, "draft one").await;
    store.disconnect("a").await.unwrap();
    let a = store.account("a").await.unwrap();
    for subject in ["missing-a", "missing-b"] {
        let s = store.comment_draft("a", subject).await.unwrap();
        store
            .save_comment_draft(SaveCommentDraftRequest {
                account_id: "a".into(),
                subject_id: subject.into(),
                authorization_epoch: a.authorization_epoch.clone(),
                authorization_view: s.authorization_view,
                expected_generation: "0".into(),
                body: "x".repeat(500),
            })
            .await
            .unwrap();
    }
    let first = store
        .comment_drafts(CommentDraftQuery {
            account_id: "a".into(),
            cursor: None,
            limit: 1,
        })
        .await
        .unwrap();
    assert_eq!(first.drafts[0].subject_id, "missing-a");
    assert_eq!(first.drafts[0].preview.len(), 256);
    let next = store
        .comment_drafts(CommentDraftQuery {
            account_id: "a".into(),
            cursor: first.next_cursor,
            limit: 1,
        })
        .await
        .unwrap();
    assert_eq!(next.drafts[0].subject_id, "missing-b");
    assert!(next.next_cursor.is_some());
    assert!(
        store
            .comment_drafts(CommentDraftQuery {
                account_id: "a".into(),
                cursor: None,
                limit: 101
            })
            .await
            .is_err()
    );
    assert!(
        store
            .comment_draft("a", "missing-a")
            .await
            .unwrap()
            .context
            .is_none()
    );
    store.close().await.unwrap();
}
#[tokio::test]
async fn comment_send_restore_preserves_authored_link_and_quarantines_zero_attempts() {
    use crate::recovery::{RecoverySession, RestoreChoice};
    let (dir, store, a) = setup().await;
    let r = request(&save(&store, "restored draft").await);
    store.send_comment(r.clone()).await.unwrap();
    let payload = store
        .delivery_command("a", &r.command_id)
        .await
        .unwrap()
        .payload;
    let backup = dir.path().join("backup.db");
    let summary = store.backup_to(&backup).await.unwrap();
    assert_eq!(summary.schema_version, 21);
    assert_eq!(summary.drafts, 1);
    store.close().await.unwrap();
    let session = RecoverySession::prepare(dir.path().join("comments.db"), &backup)
        .await
        .unwrap();
    let id = session.preview().confirmation_id.clone();
    session
        .confirm(&id, RestoreChoice::ReplaceCurrentData)
        .unwrap();
    let store = Store::open(dir.path().join("comments.db")).await.unwrap();
    let mut active = store.account("a").await.unwrap();
    active.state = AccountState::Active;
    active.authorization_epoch =
        (active.authorization_epoch.parse::<u64>().unwrap() + 1).to_string();
    let active = store.upsert_account(active).await.unwrap();
    assert_ne!(active.authorization_epoch, a.authorization_epoch);
    let c = store.delivery_command("a", &r.command_id).await.unwrap();
    assert_eq!(c.payload, payload);
    assert!(c.reconcile_only());
    assert_eq!(c.attempt_count, 0);
    let (p, _server) = server_headers(vec![]);
    assert!(
        store
            .claim_preparation(&c, &active, &p, &time())
            .await
            .is_err()
    );
    let draft = store.comment_draft("a", "pull").await.unwrap();
    assert_eq!(draft.body, "restored draft");
    assert_eq!(draft.reason, Some(CommentSendReason::AlreadySubmitted));
    assert!(store.send_comment(r).await.is_err());
    store.close().await.unwrap();
}

#[tokio::test]
async fn comment_send_held_receipt_after_same_epoch_view_reset_cannot_publish() {
    let (_dir, store, a) = setup().await;
    let r = request(&save(&store, "body").await);
    store.send_comment(r.clone()).await.unwrap();
    let (p, server) = server_headers(vec![
        (200, Some(target()), String::new()),
        (201, Some(created("body")), String::new()),
    ]);
    let dispatch = claim_dispatch(&store, &a, &p, &r.command_id).await;
    let report = p.dispatch(&token(), dispatch.clone()).await;
    assert!(matches!(report.outcome, DeliveryOutcome::Confirmed(_)));
    {
        let mut writer = store.inner.writer.acquire().await.unwrap();
        sqlx::query(
            "UPDATE runtime_meta SET authorization_view=authorization_view+1 WHERE singleton=1",
        )
        .execute(&mut *writer)
        .await
        .unwrap();
    }
    assert!(
        store
            .complete_delivery(
                &dispatch.command,
                &p,
                super::delivery::DeliveryCompletion {
                    account: &a,
                    attempt: Some(dispatch.attempt),
                    report: &report,
                    now: &now(),
                    next: &now()
                }
            )
            .await
            .is_err()
    );
    assert!(
        store
            .created_comments(query())
            .await
            .unwrap()
            .comments
            .is_empty()
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
async fn comment_send_proof_rejects_unknown_nested_fields_and_duplicate_comment_identity() {
    let (_dir, store, a) = setup().await;
    let r = request(&save(&store, "first").await);
    store.send_comment(r.clone()).await.unwrap();
    let (p, server) = server_headers(vec![
        (200, Some(target()), String::new()),
        (201, Some(created("first")), String::new()),
        (200, Some(target()), String::new()),
        (201, Some(created("second")), String::new()),
    ]);
    let dispatch = claim_dispatch(&store, &a, &p, &r.command_id).await;
    let report = p.dispatch(&token(), dispatch.clone()).await;
    let DeliveryOutcome::Confirmed(proof) = &report.outcome else {
        panic!("valid201")
    };
    let mut value: serde_json::Value = serde_json::from_slice(&proof.payload).unwrap();
    value["preparation"]["frame"]["subject"]["unexpected_secret"] = json!("unknown");
    let bad = OperationEvidence {
        payload: serde_json::to_vec(&value).unwrap(),
        ..proof.clone()
    };
    assert!(!p.validate_evidence(&dispatch.command, EvidencePurpose::Confirmed, &bad));
    complete(&store, &a, &p, &dispatch, &report).await;
    let r2 = request(&save(&store, "second").await);
    store.send_comment(r2.clone()).await.unwrap();
    let dispatch2 = claim_dispatch(&store, &a, &p, &r2.command_id).await;
    let report2 = p.dispatch(&token(), dispatch2.clone()).await;
    assert!(
        store
            .complete_delivery(
                &dispatch2.command,
                &p,
                super::delivery::DeliveryCompletion {
                    account: &a,
                    attempt: Some(dispatch2.attempt),
                    report: &report2,
                    now: &now(),
                    next: &now()
                }
            )
            .await
            .is_err()
    );
    assert_eq!(
        store
            .created_comments(query())
            .await
            .unwrap()
            .comments
            .len(),
        1
    );
    server.join().unwrap();
    store.close().await.unwrap();
}

#[tokio::test]
async fn comment_send_immutable_route_rejects_recycled_repository_or_moved_subject_before_post() {
    for repo_changed in [false, true] {
        let (_dir, store, a) = setup().await;
        let r = request(&save(&store, "private intended destination").await);
        store.send_comment(r.clone()).await.unwrap();
        let mut wrong = target();
        if repo_changed {
            wrong["base"]["repo"]["id"] = json!(2)
        } else {
            wrong["id"] = json!(9007199254740996_u64)
        }
        let (p, server) = server_headers(vec![(200, Some(wrong), String::new())]);
        let c = store.delivery_command("a", &r.command_id).await.unwrap();
        let (req, _) = store.claim_preparation(&c, &a, &p, &time()).await.unwrap();
        assert!(p.prepare(&token(), &req.unwrap()).await.is_err());
        assert_eq!(
            store
                .delivery_command("a", &r.command_id)
                .await
                .unwrap()
                .attempt_count,
            0
        );
        let calls = server.join().unwrap();
        assert_eq!(calls.len(), 1);
        assert!(calls[0].starts_with("GET /repositories/1/pulls/67 "));
        store.close().await.unwrap();
    }
}
#[tokio::test]
async fn comment_send_numeric_post_never_falls_back_to_named_redirect() {
    let (_dir, store, a) = setup().await;
    let r = request(&save(&store, "body").await);
    store.send_comment(r.clone()).await.unwrap();
    let (p, server) = server_headers(vec![
        (200, Some(target()), String::new()),
        (
            301,
            Some(json!({})),
            "Location: https://api.github.com/repos/recycled/name/issues/67/comments\r\n".into(),
        ),
    ]);
    let dispatch = claim_dispatch(&store, &a, &p, &r.command_id).await;
    let report = p.dispatch(&token(), dispatch.clone()).await;
    assert!(matches!(report.outcome, DeliveryOutcome::Unknown));
    complete(&store, &a, &p, &dispatch, &report).await;
    let calls = server.join().unwrap();
    assert_eq!(calls.len(), 2);
    assert!(calls[1].starts_with("POST /repositories/1/issues/67/comments "));
    store.close().await.unwrap();
}

#[tokio::test]
async fn comment_send_restore_refuses_missing_link_or_mismatched_proof_without_mutating_files() {
    use crate::comment_send::native as n;
    use crate::recovery::RecoverySession;
    let (dir, store, account) = setup().await;
    let request = request(&save(&store, "retained receipt").await);
    store.send_comment(request.clone()).await.unwrap();
    let (policy, server) = server_headers(vec![
        (200, Some(target()), String::new()),
        (201, Some(created("retained receipt")), String::new()),
    ]);
    let dispatch = claim_dispatch(&store, &account, &policy, &request.command_id).await;
    let report = policy.dispatch(&token(), dispatch.clone()).await;
    complete(&store, &account, &policy, &dispatch, &report).await;
    server.join().unwrap();
    {
        let mut writer = store.inner.writer.acquire().await.unwrap();
        for sql in [
            "DELETE FROM comment_submissions",
            "UPDATE comment_submissions SET body_hash=zeroblob(32)",
            "DELETE FROM comment_drafts",
            "UPDATE comment_drafts SET subject_id='different'",
        ] {
            assert!(
                sqlx::query(sql).execute(&mut *writer).await.is_err(),
                "{sql}"
            );
        }
    }
    let backup = dir.path().join("backup.db");
    store.backup_to(&backup).await.unwrap();
    store.close().await.unwrap();
    let target_path = dir.path().join("comments.db");
    let original = std::fs::read(&target_path).unwrap();
    // A legitimate completed receipt remains inspectable for a future restore.
    let good = RecoverySession::prepare(&target_path, &backup)
        .await
        .unwrap();
    drop(good);
    for case in ["missing-link", "body-hash", "proof-hash", "proof-actor"] {
        let bad = dir.path().join(format!("{case}.db"));
        std::fs::copy(&backup, &bad).unwrap();
        let mut db = SqliteConnection::connect_with(&SqliteConnectOptions::new().filename(&bad))
            .await
            .unwrap();
        let trigger_name = match case {
            "missing-link" => "comment_submission_retained",
            "body-hash" => "comment_submission_immutable",
            _ => "evidence_immutable",
        };
        let trigger: String = sqlx::query_scalar("SELECT sql FROM sqlite_schema WHERE name=?")
            .bind(trigger_name)
            .fetch_one(&mut db)
            .await
            .unwrap();
        sqlx::raw_sql(sqlx::AssertSqlSafe(format!("DROP TRIGGER {trigger_name}")))
            .execute(&mut db)
            .await
            .unwrap();
        match case {
            "missing-link" => {
                sqlx::query("DELETE FROM comment_submissions")
                    .execute(&mut db)
                    .await
                    .unwrap();
            }
            "body-hash" => {
                sqlx::query("UPDATE comment_submissions SET body_hash=zeroblob(32)")
                    .execute(&mut db)
                    .await
                    .unwrap();
            }
            _ => {
                let bytes: Vec<u8> = sqlx::query_scalar(
                    "SELECT payload FROM command_evidence WHERE kind='github.comment_created'",
                )
                .fetch_one(&mut db)
                .await
                .unwrap();
                let mut proof: n::ReceiptEvidence = n::decode_json(&bytes).unwrap();
                if case == "proof-hash" {
                    proof.preparation.command_hash = "0".repeat(64);
                } else {
                    proof.preparation.actor = "different-actor".into();
                }
                sqlx::query(
                    "UPDATE command_evidence SET payload=? WHERE kind='github.comment_created'",
                )
                .bind(n::encode(&proof).unwrap())
                .execute(&mut db)
                .await
                .unwrap();
            }
        }
        sqlx::raw_sql(sqlx::AssertSqlSafe(trigger))
            .execute(&mut db)
            .await
            .unwrap();
        db.close().await.unwrap();
        let selected = std::fs::read(&bad).unwrap();
        assert!(
            RecoverySession::prepare(&target_path, &bad).await.is_err(),
            "{case}"
        );
        assert_eq!(std::fs::read(&bad).unwrap(), selected);
        assert_eq!(std::fs::read(&target_path).unwrap(), original);
    }
}
