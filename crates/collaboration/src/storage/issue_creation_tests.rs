use super::*;
use crate::providers::github::issue_creation::GithubIssueCreationPolicy;
use crate::runtime::detail_tests::fixtures;

use crate::{delivery::*, issue_creation::*};
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
    GithubIssueCreationPolicy,
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
    (GithubIssueCreationPolicy::for_test_base(url), handle)
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
async fn save(store: &Store, body: &str) -> IssueDraftSnapshot {
    let k = key();
    let old = store.issue_draft(k.clone()).await.unwrap();
    let a = store.account("a").await.unwrap();
    store
        .save_issue_draft(SaveIssueDraftRequest {
            account_id: k.account_id,
            draft_id: k.draft_id,
            repository_id: k.repository_id,
            authorization_epoch: a.authorization_epoch,
            authorization_view: old.authorization_view,
            expected_generation: old.generation,
            title: "New issue".into(),
            body: body.into(),
        })
        .await
        .unwrap()
}
fn request(s: &IssueDraftSnapshot) -> SubmitIssueRequest {
    SubmitIssueRequest {
        context: s.context.clone().unwrap(),
        draft_id: s.draft_id.clone(),
        draft_generation: s.generation.clone(),
        command_id: Uuid::new_v4().to_string(),
        accept_background_delivery: true,
    }
}
async fn claim_dispatch(
    store: &Store,
    a: &RemoteAccount,
    p: &GithubIssueCreationPolicy,
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
    p: &GithubIssueCreationPolicy,
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
async fn issue_creation_drafts_are_local_cas_stable_bound_and_recoverable() {
    let (dir, store, _) = setup().await;
    let blank = store.issue_draft(key()).await.unwrap();
    assert_eq!(blank.generation, "0");
    assert_eq!(blank.body, "");
    let first = save(&store, "my body").await;
    let same = save(&store, "my body").await;
    assert_eq!(first, same);
    let mut wrong = key();
    wrong.repository_id = "another".into();
    assert!(store.issue_draft(wrong).await.is_err());
    store.disconnect("a").await.unwrap();
    let local = save(&store, "offline edits").await;
    assert_eq!(local.reason, Some(IssueDraftReason::AccountUnavailable));
    assert!(local.context.is_none());
    store.close().await.unwrap();
    let store = Store::open(dir.path().join("comments.db")).await.unwrap();
    let local = store.issue_draft(key()).await.unwrap();
    assert_eq!(local.body, "offline edits");
    let page = store
        .issue_drafts(IssueDraftQuery {
            account_id: "a".into(),
            cursor: None,
            limit: 1,
        })
        .await
        .unwrap();
    assert_eq!(page.drafts[0].draft_id, key().draft_id);
    assert_eq!(page.drafts[0].preview, "offline edits");
    store.close().await.unwrap();
}
#[tokio::test]
async fn issue_creation_generation_and_uuid_cannot_bypass_pending_admission() {
    let (dir, store, _) = setup().await;
    let r = request(&save(&store, "body").await);
    let receipt = store.submit_issue(r.clone()).await.unwrap();
    assert!(!receipt.duplicate);
    let bytes = store
        .delivery_command("a", &r.command_id)
        .await
        .unwrap()
        .payload;
    let mut another = r.clone();
    another.command_id = Uuid::new_v4().to_string();
    assert!(store.submit_issue(another).await.is_err());
    let edited = save(&store, "edited body").await;
    assert_eq!(edited.reason, Some(IssueDraftReason::PendingSubmission));
    assert!(store.submit_issue(r.clone()).await.unwrap().duplicate);
    store.close().await.unwrap();
    let store = Store::open(dir.path().join("comments.db")).await.unwrap();
    assert!(store.submit_issue(r.clone()).await.unwrap().duplicate);
    assert_eq!(
        store
            .delivery_command("a", &r.command_id)
            .await
            .unwrap()
            .payload,
        bytes
    );
    store.close().await.unwrap();
}
#[tokio::test]
async fn issue_creation_strong_201_atomically_publishes_canonical_identity_and_retains_draft() {
    for empty in [false, true] {
        let (_dir, store, a) = setup().await;
        let body = if empty { "" } else { "body" };
        let r = request(&save(&store, body).await);
        store.submit_issue(r.clone()).await.unwrap();
        let mut remote = created(body);
        if empty {
            remote["body"] = serde_json::Value::Null;
        }
        let (p, server) = server_headers(vec![
            (200, Some(target()), String::new()),
            (201, Some(remote), String::new()),
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
        let draft = store.issue_draft(key()).await.unwrap();
        let published = draft.published.unwrap();
        assert_eq!(published.subject_id, "github:issue:9007199254740995");
        assert_eq!(draft.body, body);
        assert_eq!(draft.reason, Some(IssueDraftReason::AlreadySubmitted));
        let item = store
            .item("a", &published.subject_id)
            .await
            .unwrap()
            .item
            .unwrap();
        assert_eq!(item.title, "New issue");
        assert_eq!(item.body.as_deref().unwrap_or(""), body);
        let page = store
            .query_items(ItemQuery {
                account_id: "a".into(),
                kind: RemoteItemKind::Issue,
                repository_id: Some("repo".into()),
                state: Some("open".into()),
                search: Some("New issue".into()),
                cursor: None,
                limit: 20,
            })
            .await
            .unwrap();
        assert_eq!(page.total_count, 1);
        assert_ne!(page.coverage.state, CoverageState::Complete);
        let requests = server.join().unwrap();
        assert!(requests[0].starts_with("GET /repositories/1 "));
        assert!(requests[1].starts_with("POST /repositories/1/issues "));
        let wire: serde_json::Value =
            serde_json::from_str(requests[1].split("\r\n\r\n").nth(1).unwrap()).unwrap();
        assert_eq!(wire, json!({"title":"New issue","body":body}));
        store.close().await.unwrap();
    }
}
#[tokio::test]
async fn issue_creation_lost_response_and_restart_never_posts_or_matches_again() {
    let (dir, store, a) = setup().await;
    let r = request(&save(&store, "body").await);
    store.submit_issue(r.clone()).await.unwrap();
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
    let c = store.delivery_command("a", &r.command_id).await.unwrap();
    store.recover_delivery(&c, &now()).await.unwrap();
    let c = store.delivery_command("a", &r.command_id).await.unwrap();
    let (req, _) = store
        .claim_reconciliation(&c, &a, &p, &time())
        .await
        .unwrap();
    assert!(matches!(
        p.reconcile(&token(), req.unwrap()).await.unwrap().outcome,
        DeliveryOutcome::Unknown
    ));
    assert!(store.issue_draft(key()).await.unwrap().published.is_none());
    assert!(save(&store, "new edited body").await.context.is_none());
    assert_eq!(server.join().unwrap().len(), 2);
    store.close().await.unwrap();
}
#[tokio::test]
async fn issue_creation_receipts_reject_actor_pr_body_identity_and_non201() {
    for case in 0..6 {
        let (_dir, store, a) = setup().await;
        let r = request(&save(&store, "body").await);
        store.submit_issue(r.clone()).await.unwrap();
        let mut remote = created("body");
        match case {
            0 => remote["pull_request"] = json!({"url":"pull"}),
            1 => remote["user"]["id"] = 99.into(),
            2 => remote["body"] = "different".into(),
            3 => remote["repository_url"] = "https://api.github.com/repos/other/project".into(),
            4 => remote["id"] = 0.into(),
            _ => {}
        }
        let (p, server) = server_headers(vec![
            (200, Some(target()), String::new()),
            (
                if case == 5 { 200 } else { 201 },
                Some(remote),
                String::new(),
            ),
        ]);
        let d = claim_dispatch(&store, &a, &p, &r.command_id).await;
        let report = p.dispatch(&token(), d.clone()).await;
        assert!(matches!(report.outcome, DeliveryOutcome::Unknown));
        complete(&store, &a, &p, &d, &report).await;
        assert!(store.issue_draft(key()).await.unwrap().published.is_none());
        server.join().unwrap();
        store.close().await.unwrap();
    }
}
#[tokio::test]
async fn issue_creation_preflight_checks_repository_identity_and_feature_availability_without_push_gate()
 {
    for case in 0..3 {
        let (_dir, store, a) = setup().await;
        let r = request(&save(&store, "body").await);
        store.submit_issue(r.clone()).await.unwrap();
        let mut remote = target();
        match case {
            0 => remote["id"] = 99.into(),
            1 => remote["archived"] = true.into(),
            _ => remote["has_issues"] = false.into(),
        };
        let (p, server) = server_headers(vec![(200, Some(remote), "Retry-After: 120\r\n".into())]);
        let c = store.delivery_command("a", &r.command_id).await.unwrap();
        let (req, _) = store.claim_preparation(&c, &a, &p, &time()).await.unwrap();
        let e = p.prepare(&token(), &req.unwrap()).await.err().unwrap();
        assert_eq!(e.account_cooldown_seconds, Some(120));
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
}
#[tokio::test]
async fn issue_creation_authorization_reset_rejects_held_completion() {
    let (_dir, store, a) = setup().await;
    let r = request(&save(&store, "body").await);
    store.submit_issue(r.clone()).await.unwrap();
    let (p, server) = server_headers(vec![
        (200, Some(target()), String::new()),
        (201, Some(created("body")), String::new()),
    ]);
    let d = claim_dispatch(&store, &a, &p, &r.command_id).await;
    let report = p.dispatch(&token(), d.clone()).await;
    store.disconnect("a").await.unwrap();
    assert!(
        store
            .complete_delivery(
                &d.command,
                &p,
                super::delivery::DeliveryCompletion {
                    account: &a,
                    attempt: Some(1),
                    report: &report,
                    now: &now(),
                    next: &now()
                }
            )
            .await
            .is_err()
    );
    assert!(store.issue_draft(key()).await.unwrap().published.is_none());
    server.join().unwrap();
    store.close().await.unwrap();
}
#[tokio::test]
async fn issue_creation_finalization_failure_rolls_back_entity_fts_linkage_and_confirmation() {
    let (_dir, store, a) = setup().await;
    let r = request(&save(&store, "body").await);
    store.submit_issue(r.clone()).await.unwrap();
    let (p, server) = server_headers(vec![
        (200, Some(target()), String::new()),
        (201, Some(created("body")), String::new()),
    ]);
    let d = claim_dispatch(&store, &a, &p, &r.command_id).await;
    let report = p.dispatch(&token(), d.clone()).await;
    {
        let mut w = store.inner.writer.acquire().await.unwrap();
        sqlx::query("CREATE TEMP TRIGGER issue_final_fault BEFORE INSERT ON issue_resolutions BEGIN SELECT RAISE(ABORT,'fixture rollback'); END").execute(&mut *w).await.unwrap();
    }
    assert!(
        store
            .complete_delivery(
                &d.command,
                &p,
                super::delivery::DeliveryCompletion {
                    account: &a,
                    attempt: Some(1),
                    report: &report,
                    now: &now(),
                    next: &now()
                }
            )
            .await
            .is_err()
    );
    assert!(store.issue_draft(key()).await.unwrap().published.is_none());
    assert_eq!(
        store
            .delivery_command("a", &r.command_id)
            .await
            .unwrap()
            .state,
        DeliveryState::Sending
    );
    let count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM items WHERE id='github:issue:9007199254740995'")
            .fetch_one(&store.inner.readers)
            .await
            .unwrap();
    assert_eq!(count, 0);
    server.join().unwrap();
    store.close().await.unwrap();
}
#[tokio::test]
async fn issue_creation_backup_restore_retains_drafts_and_quarantines_every_dispatch() {
    use crate::recovery::{RecoverySession, RestoreChoice};
    let (dir, store, _) = setup().await;
    let r = request(&save(&store, "authored body").await);
    store.submit_issue(r.clone()).await.unwrap();
    let bytes = store
        .delivery_command("a", &r.command_id)
        .await
        .unwrap()
        .payload;
    let backup = dir.path().join("backup.db");
    let summary = store.backup_to(&backup).await.unwrap();
    assert_eq!(summary.schema_version, 24);
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
    let mut a = store.account("a").await.unwrap();
    a.state = AccountState::Active;
    a.authorization_epoch = (a.authorization_epoch.parse::<u64>().unwrap() + 1).to_string();
    let a = store.upsert_account(a).await.unwrap();
    let c = store.delivery_command("a", &r.command_id).await.unwrap();
    assert_eq!(c.payload, bytes);
    assert!(c.reconcile_only());
    assert_eq!(
        store.issue_draft(key()).await.unwrap().body,
        "authored body"
    );
    let (p, server) = server_headers(vec![]);
    assert!(store.claim_preparation(&c, &a, &p, &time()).await.is_err());
    server.join().unwrap();
    store.close().await.unwrap();
}

#[tokio::test]
async fn issue_creation_restore_requires_atomic_mapping_and_exact_canonical_proof() {
    use crate::issue_creation::native as n;
    use crate::recovery::{RecoverySession, RestoreChoice};
    let (dir, store, account) = setup().await;
    let request = request(&save(&store, "retained receipt").await);
    store.submit_issue(request.clone()).await.unwrap();
    let (policy, server) = server_headers(vec![
        (200, Some(target()), String::new()),
        (201, Some(created("retained receipt")), String::new()),
    ]);
    let dispatch = claim_dispatch(&store, &account, &policy, &request.command_id).await;
    let report = policy.dispatch(&token(), dispatch.clone()).await;
    complete(&store, &account, &policy, &dispatch, &report).await;
    server.join().unwrap();
    let backup = dir.path().join("confirmed.db");
    store.backup_to(&backup).await.unwrap();
    store.close().await.unwrap();
    let target_path = dir.path().join("comments.db");
    let original = std::fs::read(&target_path).unwrap();
    drop(
        RecoverySession::prepare(&target_path, &backup)
            .await
            .unwrap(),
    );
    for case in [
        "wrong-resolution-kind",
        "wrong-resolution-ordinal",
        "orphan-evidence",
        "native-inbox-field",
        "actor",
        "login",
        "clock-order",
        "updated-clock",
        "url",
        "duplicate-field",
        "missing-field",
        "unknown-field-state",
        "oversized-label",
    ] {
        let bad = dir.path().join(format!("{case}.db"));
        std::fs::copy(&backup, &bad).unwrap();
        let mut db = SqliteConnection::connect_with(&SqliteConnectOptions::new().filename(&bad))
            .await
            .unwrap();
        let names: &[&str] = if case == "orphan-evidence" {
            &["issue_resolution_retained", "delivery_resolution_retained"]
        } else if case == "wrong-resolution-ordinal" {
            &["delivery_resolution_immutable"]
        } else {
            &["evidence_immutable"]
        };
        let mut triggers = vec![];
        for name in names {
            let sql: String = sqlx::query_scalar("SELECT sql FROM sqlite_schema WHERE name=?")
                .bind(name)
                .fetch_one(&mut db)
                .await
                .unwrap();
            sqlx::raw_sql(sqlx::AssertSqlSafe(format!("DROP TRIGGER {name}")))
                .execute(&mut db)
                .await
                .unwrap();
            triggers.push(sql);
        }
        if case == "orphan-evidence" {
            sqlx::raw_sql("DELETE FROM issue_creation_visibility; DELETE FROM issue_resolutions; DELETE FROM delivery_resolutions; UPDATE commands SET state='outcome_unknown'; UPDATE delivery_attempts SET outcome='outcome_unknown';").execute(&mut db).await.unwrap();
        } else if case == "wrong-resolution-ordinal" {
            sqlx::raw_sql("INSERT INTO command_evidence SELECT account_id,command_id,ordinal+1,attempt_number,'fixture.unrelated',1,X'00',recorded_at FROM command_evidence WHERE kind='github.issue_created'; UPDATE delivery_resolutions SET evidence_ordinal=evidence_ordinal+1;").execute(&mut db).await.unwrap();
        } else if case == "wrong-resolution-kind" {
            sqlx::query("UPDATE command_evidence SET kind='fixture.unrelated' WHERE kind='github.issue_created'").execute(&mut db).await.unwrap();
        } else if case == "native-inbox-field" {
            let bytes: Vec<u8> = sqlx::query_scalar(
                "SELECT payload FROM command_evidence WHERE kind='github.issue_created'",
            )
            .fetch_one(&mut db)
            .await
            .unwrap();
            let old = String::from_utf8(bytes).unwrap();
            assert!(!old.contains("native_inbox"));
            let bad = old.replace("\"unread\":null", "\"unread\":null,\"native_inbox\":null");
            assert_ne!(bad, old);
            sqlx::query("UPDATE command_evidence SET payload=? WHERE kind='github.issue_created'")
                .bind(bad.into_bytes())
                .execute(&mut db)
                .await
                .unwrap();
        } else {
            let bytes: Vec<u8> = sqlx::query_scalar(
                "SELECT payload FROM command_evidence WHERE kind='github.issue_created'",
            )
            .fetch_one(&mut db)
            .await
            .unwrap();
            let mut proof: n::ReceiptEvidence = n::decode_json(&bytes).unwrap();
            match case {
                "actor" => {
                    proof
                        .receipt
                        .metadata
                        .values
                        .author
                        .as_mut()
                        .unwrap()
                        .provider_id = "different-actor".into()
                }
                "login" => {
                    proof.receipt.metadata.values.author.as_mut().unwrap().login =
                        "different-login".into()
                }
                "clock-order" => proof.receipt.created_at = "2026-10-09T00:00:00Z".into(),
                "updated-clock" => {
                    proof.receipt.metadata.values.updated_at = Some("2026-10-09T00:00:00Z".into())
                }
                "url" => {
                    proof.receipt.metadata.values.web_url =
                        Some("https://github.com/other/repo/issues/91".into())
                }
                "duplicate-field" => {
                    proof.receipt.metadata.fields[1] = proof.receipt.metadata.fields[0]
                }
                "missing-field" => {
                    proof.receipt.metadata.fields.pop();
                }
                "unknown-field-state" => {
                    proof
                        .receipt
                        .metadata
                        .fields
                        .iter_mut()
                        .find(|(f, _)| *f == crate::MetadataField::Author)
                        .unwrap()
                        .1 = crate::DetailValueState::Omitted;
                }
                "oversized-label" => {
                    proof
                        .receipt
                        .metadata
                        .values
                        .labels
                        .push(crate::DetailLabel {
                            provider_id: None,
                            name: "x".repeat(1025),
                            color: None,
                        })
                }
                _ => unreachable!(),
            }
            sqlx::query("UPDATE command_evidence SET payload=? WHERE kind='github.issue_created'")
                .bind(n::encode(&proof).unwrap())
                .execute(&mut db)
                .await
                .unwrap();
        }
        for sql in triggers {
            sqlx::raw_sql(sqlx::AssertSqlSafe(sql))
                .execute(&mut db)
                .await
                .unwrap();
        }
        db.close().await.unwrap();
        let selected = std::fs::read(&bad).unwrap();
        assert!(
            RecoverySession::prepare(&target_path, &bad).await.is_err(),
            "{case}"
        );
        assert_eq!(std::fs::read(&bad).unwrap(), selected, "{case}");
        assert_eq!(std::fs::read(&target_path).unwrap(), original, "{case}");
    }
    let session = RecoverySession::prepare(&target_path, &backup)
        .await
        .unwrap();
    let id = session.preview().confirmation_id.clone();
    session
        .confirm(&id, RestoreChoice::ReplaceCurrentData)
        .unwrap();
    let restored = Store::open(&target_path).await.unwrap();
    let snapshot = restored.issue_draft(key()).await.unwrap();
    assert_eq!(snapshot.body, "retained receipt");
    assert_eq!(snapshot.submission.unwrap().state, "confirmed");
    assert!(
        snapshot.published.is_none(),
        "restored provider cache requires reauthorization"
    );
    {
        let mut writer = restored.inner.writer.acquire().await.unwrap();
        assert_eq!(
            sqlx::query_scalar::<_, String>("SELECT entity_id FROM issue_resolutions")
                .fetch_one(&mut *writer)
                .await
                .unwrap(),
            "github:issue:9007199254740995"
        );
    }
    restored.close().await.unwrap();
}

#[tokio::test]
async fn issue_creation_duplicate_remote_receipt_cannot_confirm_a_second_draft() {
    let (_dir, store, account) = setup().await;
    let first = request(&save(&store, "same authored text").await);
    store.submit_issue(first.clone()).await.unwrap();
    let (policy, server) = server_headers(vec![
        (200, Some(target()), String::new()),
        (201, Some(created("same authored text")), String::new()),
        (200, Some(target()), String::new()),
        (201, Some(created("same authored text")), String::new()),
    ]);
    let dispatched = claim_dispatch(&store, &account, &policy, &first.command_id).await;
    let report = policy.dispatch(&token(), dispatched.clone()).await;
    complete(&store, &account, &policy, &dispatched, &report).await;
    let new_key = IssueDraftKey {
        draft_id: Uuid::new_v4().to_string(),
        ..key()
    };
    let blank = store.issue_draft(new_key.clone()).await.unwrap();
    let second = store
        .save_issue_draft(SaveIssueDraftRequest {
            account_id: new_key.account_id,
            draft_id: new_key.draft_id,
            repository_id: new_key.repository_id,
            authorization_epoch: account.authorization_epoch.clone(),
            authorization_view: blank.authorization_view,
            expected_generation: blank.generation,
            title: "New issue".into(),
            body: "same authored text".into(),
        })
        .await
        .unwrap();
    let second = request(&second);
    store.submit_issue(second.clone()).await.unwrap();
    let dispatched = claim_dispatch(&store, &account, &policy, &second.command_id).await;
    let report = policy.dispatch(&token(), dispatched.clone()).await;
    assert!(
        store
            .complete_delivery(
                &dispatched.command,
                &policy,
                super::delivery::DeliveryCompletion {
                    account: &account,
                    attempt: Some(dispatched.attempt),
                    report: &report,
                    now: &now(),
                    next: &now(),
                }
            )
            .await
            .is_err()
    );
    assert_eq!(
        store
            .delivery_command("a", &second.command_id)
            .await
            .unwrap()
            .state,
        DeliveryState::Sending
    );
    {
        let mut writer = store.inner.writer.acquire().await.unwrap();
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT count(*) FROM issue_resolutions")
                .fetch_one(&mut *writer)
                .await
                .unwrap(),
            1
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>(
                "SELECT count(*) FROM command_evidence WHERE kind='github.issue_created'"
            )
            .fetch_one(&mut *writer)
            .await
            .unwrap(),
            1
        );
    }
    assert_eq!(server.join().unwrap().len(), 4);
    store.close().await.unwrap();
}

#[tokio::test]
async fn issue_creation_submission_link_failure_rolls_back_command_and_generation() {
    let (_dir, store, _) = setup().await;
    let saved = save(&store, "atomic draft").await;
    let r = request(&saved);
    let revision = store.revision().await.unwrap();
    {
        let mut writer = store.inner.writer.acquire().await.unwrap();
        sqlx::raw_sql("CREATE TRIGGER fixture_issue_submission_failure BEFORE INSERT ON issue_submissions BEGIN SELECT RAISE(ABORT,'fixture failure'); END;").execute(&mut *writer).await.unwrap();
    }
    assert!(store.submit_issue(r.clone()).await.is_err());
    assert_eq!(store.revision().await.unwrap(), revision);
    assert!(store.delivery_command("a", &r.command_id).await.is_err());
    let after = store.issue_draft(key()).await.unwrap();
    assert_eq!(after.generation, saved.generation);
    assert_eq!(after.body, saved.body);
    assert!(after.submission.is_none());
    {
        let mut writer = store.inner.writer.acquire().await.unwrap();
        sqlx::raw_sql("DROP TRIGGER fixture_issue_submission_failure")
            .execute(&mut *writer)
            .await
            .unwrap();
    }
    assert!(!store.submit_issue(r.clone()).await.unwrap().duplicate);
    assert!(store.submit_issue(r).await.unwrap().duplicate);
    store.close().await.unwrap();
}
