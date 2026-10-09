use super::*;
use crate::commands::CommandSubmission;
use crate::issue_creation::native as n;
use crate::storage::command_admission::{CommandAdmissionPolicy, CommandProtection};
use crate::storage::issue_creation as operation;

// Native fixture for immutable v1 commands admitted by the previous source.
// Only the newly added encoded-budget gate is absent; authored bytes and real
// SQLite command/submission guards are retained.
struct LegacyAdmission;
#[async_trait::async_trait]
impl CommandAdmissionPolicy for LegacyAdmission {
    const OPERATION_KIND: &'static str = "github.create_issue";
    const PAYLOAD_VERSION: u32 = 1;
    async fn validate(
        &self,
        _: &mut Transaction<'_, Sqlite>,
        _: &RemoteAccount,
        _: &CommandSubmission,
    ) -> Result<Vec<CommandProtection>> {
        Ok(vec![])
    }
}
#[tokio::test]
async fn issue_creation_encoded_budget_refuses_unsafe_send_and_retains_raw_draft() {
    for length in [10_900, 16_384] {
        let (_dir, store, _) = setup().await;
        let body = "\u{1}".repeat(length);
        let saved = save(&store, &body).await;
        let r = request(&saved);
        let error = store.submit_issue(r.clone()).await.unwrap_err();
        assert_eq!(error.code, ErrorCode::InvalidInput);
        assert_eq!(store.issue_draft(key()).await.unwrap(), saved);
        for sql in [
            "SELECT count(*) FROM commands",
            "SELECT count(*) FROM delivery_attempts",
            "SELECT count(*) FROM command_effects",
        ] {
            let count: i64 = sqlx::query_scalar(sql)
                .fetch_one(&store.inner.readers)
                .await
                .unwrap();
            assert_eq!(count, 0);
        }
        store.close().await.unwrap();
    }
}
#[tokio::test]
async fn issue_creation_encoded_budget_maximum_plain_and_bounded_escaped_body_confirm() {
    for body in ["x".repeat(16_384), "\u{1}".repeat(6_000)] {
        let (_dir, store, a) = setup().await;
        let r = request(&save(&store, &body).await);
        store.submit_issue(r.clone()).await.unwrap();
        let (p, server) = server_headers(vec![
            (200, Some(target()), String::new()),
            (201, Some(created(&body)), "Retry-After: 120\r\n".into()),
        ]);
        let d = claim_dispatch(&store, &a, &p, &r.command_id).await;
        let report = p.dispatch(&token(), d.clone()).await;
        assert_eq!(server.join().unwrap().len(), 2);
        assert_eq!(report.account_cooldown_seconds, Some(120));
        let DeliveryOutcome::Confirmed(proof) = &report.outcome else {
            panic!("admitted valid201 did not retain its proof")
        };
        assert!(proof.payload.len() <= 65_536);
        complete(&store, &a, &p, &d, &report).await;
        let saved = store.issue_draft(key()).await.unwrap();
        assert_eq!(saved.body, body);
        assert_eq!(saved.reason, Some(IssueDraftReason::AlreadySubmitted));
        assert!(store.submit_issue(r).await.unwrap().duplicate);
        store.close().await.unwrap();
    }
}
#[tokio::test]
async fn issue_creation_encoded_budget_unrequested_metadata_cannot_lose_proof() {
    let (_dir, store, a) = setup().await;
    let r = request(&save(&store, "ordinary body").await);
    store.submit_issue(r.clone()).await.unwrap();
    let mut remote = created("ordinary body");
    remote["assignees"] = json!(
        (1..=100)
            .map(|n| json!({"id":n,"login":"a".repeat(255)}))
            .collect::<Vec<_>>()
    );
    remote["milestone"] = json!({"id":123,"title":"m".repeat(16_384)});
    remote["user"]["html_url"] = json!("https://github.com/author");
    remote["labels"] = json!(
        (1..=100)
            .map(|n| json!({"id":n,"name":"x".repeat(1024),"color":"abcdef"}))
            .collect::<Vec<_>>()
    );
    let (p, server) = server_headers(vec![
        (200, Some(target()), String::new()),
        (201, Some(remote), String::new()),
    ]);
    let d = claim_dispatch(&store, &a, &p, &r.command_id).await;
    let report = p.dispatch(&token(), d.clone()).await;
    assert_eq!(server.join().unwrap().len(), 2);
    assert!(
        matches!(report.outcome, DeliveryOutcome::Confirmed(_)),
        "unrequested metadata made causal201 proof unencodable"
    );
    let DeliveryOutcome::Confirmed(proof) = &report.outcome else {
        unreachable!()
    };
    let evidence: crate::issue_creation::native::ReceiptEvidence =
        crate::issue_creation::native::decode_json(&proof.payload).unwrap();
    assert!(evidence.receipt.metadata.values.labels.is_empty());
    assert!(evidence.receipt.metadata.values.assignees.is_empty());
    assert!(evidence.receipt.metadata.values.milestone.is_none());
    assert!(
        evidence
            .receipt
            .metadata
            .values
            .author
            .as_ref()
            .unwrap()
            .web_url
            .is_none()
    );
    for field in [
        crate::MetadataField::Labels,
        crate::MetadataField::Assignees,
        crate::MetadataField::Milestone,
    ] {
        assert!(
            evidence
                .receipt
                .metadata
                .fields
                .contains(&(field, crate::DetailValueState::Omitted))
        );
    }
    complete(&store, &a, &p, &d, &report).await;
    store.close().await.unwrap();
}
#[tokio::test]
async fn issue_creation_pending_generation_never_claims_created() {
    let (_dir, store, _) = setup().await;
    let r = request(&save(&store, "body").await);
    store.submit_issue(r).await.unwrap();
    assert_eq!(
        store.issue_draft(key()).await.unwrap().reason,
        Some(IssueDraftReason::PendingSubmission)
    );
    store.close().await.unwrap();
}

async fn legacy_admit(
    store: &Store,
    a: &RemoteAccount,
    r: &SubmitIssueRequest,
    body: &str,
) -> n::Preparation {
    let mut writer = store.inner.writer.acquire().await.unwrap();
    let mut tx = writer.begin().await.unwrap();
    let frame = operation::capture_in(&mut tx, a, "repo").await.unwrap();
    let payload = n::Payload {
        request: r.clone(),
        title: "New issue".into(),
        body: body.into(),
        repository_native: frame.repository.provider_id.clone(),
    };
    let sealed = operation::seal(payload).unwrap();
    command_admission::admit_in(&mut tx, &sealed, &LegacyAdmission)
        .await
        .unwrap();
    sqlx::query("INSERT INTO issue_submissions VALUES(?,?,?,?,?,?)")
        .bind("a")
        .bind(&r.draft_id)
        .bind(r.draft_generation.parse::<i64>().unwrap())
        .bind(&r.command_id)
        .bind(sealed.submission_hash().as_slice())
        .bind(n::content_hash("New issue", body))
        .execute(&mut *tx)
        .await
        .unwrap();
    tx.commit().await.unwrap();
    drop(writer);
    let c = store.delivery_command("a", &r.command_id).await.unwrap();
    n::Preparation {
        frame,
        actor: a.actor_id.clone(),
        epoch: a.authorization_epoch.clone(),
        command_hash: n::command_hash(&c),
    }
}
#[tokio::test]
async fn issue_creation_encoded_budget_legacy_queue_refuses_before_http_and_attempt_but_exact_retry_survives()
 {
    let (dir, store, a) = setup().await;
    let body = "\u{1}".repeat(10_900);
    let r = request(&save(&store, &body).await);
    let prep = legacy_admit(&store, &a, &r, &body).await;
    let original = store.delivery_command("a", &r.command_id).await.unwrap();
    assert!(store.submit_issue(r.clone()).await.unwrap().duplicate);
    let (p, server) = server_headers(vec![]);
    let (req, _) = store
        .claim_preparation(&original, &a, &p, &time())
        .await
        .unwrap();
    let req = req.unwrap();
    assert_eq!(
        p.prepare(&token(), &req).await.err().unwrap().kind,
        crate::providers::ProviderErrorKind::InvalidResponse
    );
    // An old preparation held across the upgrade also cannot claim an attempt.
    let error = store
        .claim_delivery(&req.command, &a, &p, &n::encode(&prep).unwrap(), &time())
        .await
        .err()
        .unwrap();
    assert_eq!(error.code, ErrorCode::InvalidInput);
    let saved = store.delivery_command("a", &r.command_id).await.unwrap();
    assert_eq!(saved.payload, original.payload);
    assert_eq!(saved.attempt_count, 0);
    assert_eq!(saved.reconciliation_count, 1);
    assert_eq!(server.join().unwrap().len(), 0);
    assert_eq!(store.issue_draft(key()).await.unwrap().body, body);
    store.close().await.unwrap();
    let store = Store::open(dir.path().join("comments.db")).await.unwrap();
    assert!(store.submit_issue(r.clone()).await.unwrap().duplicate);
    assert_eq!(
        store
            .delivery_command("a", &r.command_id)
            .await
            .unwrap()
            .payload,
        original.payload
    );
    store.close().await.unwrap();
}

// Reproduce a dispatch claimed by the prior source without adding a production
// bypass. The new policy still validates and finalizes the resulting v1 proof.
struct LegacyClaim;
#[async_trait::async_trait]
impl CommandDeliveryPolicy for LegacyClaim {
    fn operation_kind(&self) -> &'static str {
        n::OPERATION
    }
    fn payload_version(&self) -> u32 {
        1
    }
    async fn validate_claim(
        &self,
        tx: &mut Transaction<'_, Sqlite>,
        c: &DeliveryCommand,
        a: &RemoteAccount,
        bytes: &[u8],
    ) -> Result<ClaimDecision> {
        let p: n::Preparation = n::decode_json(bytes)?;
        assert_eq!(p.command_hash, n::command_hash(c));
        assert_eq!(p.actor, a.actor_id);
        operation::validate_frame_in(tx, a, &p.frame).await?;
        Ok(ClaimDecision::Ready(bytes.to_vec()))
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
        unreachable!("fixture only claims")
    }
}
#[tokio::test]
async fn issue_creation_encoded_budget_preserves_previous_valid_strong_proof_and_restore() {
    use crate::recovery::RecoverySession;
    let (dir, store, a) = setup().await;
    // This old v1 payload has a valid <64KiB proof, but lies above the new
    // conservative admission boundary. Never tighten the immutable v1 decoder.
    let body = "\u{1}".repeat(7_000);
    let r = request(&save(&store, &body).await);
    let prep = legacy_admit(&store, &a, &r, &body).await;
    let c = store.delivery_command("a", &r.command_id).await.unwrap();
    let (p, server) = server_headers(vec![(201, Some(created(&body)), String::new())]);
    let (req, _) = store.claim_preparation(&c, &a, &p, &time()).await.unwrap();
    assert!(p.prepare(&token(), &req.clone().unwrap()).await.is_err());
    let d = store
        .claim_delivery(
            &req.unwrap().command,
            &a,
            &LegacyClaim,
            &n::encode(&prep).unwrap(),
            &time(),
        )
        .await
        .unwrap()
        .request
        .unwrap();
    let report = p.dispatch(&token(), d.clone()).await;
    let DeliveryOutcome::Confirmed(proof) = &report.outcome else {
        panic!("legacy valid201 proof must remain accepted")
    };
    assert!(proof.payload.len() <= 65_536);
    let evidence: n::ReceiptEvidence = n::decode_json(&proof.payload).unwrap();
    assert_eq!(n::encode(&evidence).unwrap(), proof.payload);
    complete(&store, &a, &p, &d, &report).await;
    assert!(store.submit_issue(r).await.unwrap().duplicate);
    assert_eq!(server.join().unwrap().len(), 1);
    let backup = dir.path().join("legacy-proof.db");
    store.backup_to(&backup).await.unwrap();
    store.close().await.unwrap();
    let preview = RecoverySession::prepare(dir.path().join("comments.db"), &backup)
        .await
        .unwrap();
    drop(preview);
}

#[tokio::test]
async fn issue_creation_encoded_budget_worst_frame_still_retains_confirmed_proof() {
    let (_dir, store, mut account) = setup().await;
    let r = request(&save(&store, "body").await);
    let mut prep = legacy_admit(&store, &account, &r, "body").await;
    let mut command = store.delivery_command("a", &r.command_id).await.unwrap();
    let mut payload = n::decode(&command).unwrap();
    // All lengths are permitted by the existing native v1 contracts. These
    // local identifiers expand sixfold; repository path/author/title remain
    // legal literal strings. Compute the final admitted escaped-body boundary.
    account.id = "\u{1}".repeat(1024);
    account.actor_id = u64::MAX.to_string();
    let repository_id = "\u{2}".repeat(1024);
    prep.frame.repository.id = repository_id.clone();
    prep.frame.repository.account_id = account.id.clone();
    prep.frame.repository.provider_id = u64::MAX.to_string();
    prep.frame.repository.full_name = format!("{}/{}", "o".repeat(255), "r".repeat(255));
    prep.frame.repository.name = "r".repeat(255);
    prep.frame.repository.web_url =
        format!("https://github.com/{}", prep.frame.repository.full_name);
    payload.request.context.account_id = account.id.clone();
    payload.request.context.repository_id = repository_id.clone();
    payload.repository_native = u64::MAX.to_string();
    payload.title = "\"".repeat(256);
    let counted = serde_json::to_vec(&prep.frame).unwrap().len()
        + 2 * serde_json::to_vec(&payload.title).unwrap().len()
        + 24 * 1024;
    let length = (65_536 - counted - 2) / 6;
    payload.body = "\u{1}".repeat(length);
    n::validate_dispatch_budget(&prep.frame, &payload.title, &payload.body).unwrap();
    assert!(
        n::validate_dispatch_budget(&prep.frame, &payload.title, &"\u{1}".repeat(length + 1))
            .is_err()
    );
    let sealed = operation::seal(payload.clone()).unwrap();
    command.account_id = account.id.clone();
    command.target_id = repository_id.clone();
    command.repository_id = Some(repository_id);
    command.payload = sealed.payload_bytes().to_vec();
    command.hash = *sealed.submission_hash();
    command.attempt_count = 1;
    prep.actor = account.actor_id.clone();
    prep.command_hash = n::command_hash(&command);
    let mut remote = created(&payload.body);
    remote["id"] = json!(u64::MAX);
    remote["number"] = json!(u64::MAX);
    remote["user"] = json!({"id":u64::MAX,"login":"\"".repeat(255)});
    remote["title"] = json!(payload.title);
    remote["state_reason"] = json!("\u{3}".repeat(128));
    let path = &prep.frame.repository.full_name;
    remote["url"] = json!(format!(
        "https://api.github.com/repos/{path}/issues/{}",
        u64::MAX
    ));
    remote["html_url"] = json!(format!("https://github.com/{path}/issues/{}", u64::MAX));
    remote["repository_url"] = json!(format!("https://api.github.com/repos/{path}"));
    let (policy, server) = server_headers(vec![(201, Some(remote), String::new())]);
    let request = DispatchRequest {
        command,
        account,
        instance_id: "github:https://github.com/".into(),
        attempt: 1,
        execution_base: n::encode(&prep).unwrap(),
    };
    let report = policy.dispatch(&token(), request.clone()).await;
    let DeliveryOutcome::Confirmed(proof) = report.outcome else {
        panic!("worst admitted frame exceeded the actual proof budget")
    };
    assert!(proof.payload.len() <= 65_536);
    assert!(policy.validate_evidence(&request.command, EvidencePurpose::Confirmed, &proof));
    assert_eq!(server.join().unwrap().len(), 1);
    store.close().await.unwrap();
}
