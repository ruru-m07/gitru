use super::*;
use crate::commands::CommandSubmission;
use crate::comment_send::native as n;
use crate::storage::command_admission::{CommandAdmissionPolicy, CommandProtection};
use crate::storage::comment_send as operation;

// Native fixture for immutable v1 commands admitted by the previous source.
// Only the newly added encoded-budget gate is absent; authored bytes and real
// SQLite command/submission guards are retained.
struct LegacyAdmission;
#[async_trait::async_trait]
impl CommandAdmissionPolicy for LegacyAdmission {
    const OPERATION_KIND: &'static str = "github.create_comment";
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
async fn comment_send_encoded_budget_refuses_unsafe_send_and_retains_raw_draft() {
    for length in [10_900, 16_384] {
        let (_dir, store, _) = setup().await;
        let body = "\u{1}".repeat(length);
        let saved = save(&store, &body).await;
        let r = request(&saved);
        let error = store.send_comment(r.clone()).await.unwrap_err();
        assert_eq!(error.code, ErrorCode::InvalidInput);
        assert_eq!(store.comment_draft("a", "pull").await.unwrap(), saved);
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
async fn comment_send_encoded_budget_maximum_plain_and_bounded_escaped_body_confirm() {
    for body in ["x".repeat(16_384), "\u{1}".repeat(6_000)] {
        let (_dir, store, a) = setup().await;
        let r = request(&save(&store, &body).await);
        store.send_comment(r.clone()).await.unwrap();
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
        let saved = store.comment_draft("a", "pull").await.unwrap();
        assert_eq!(saved.body, body);
        assert_eq!(saved.reason, Some(CommentSendReason::AlreadySubmitted));
        assert!(store.send_comment(r).await.unwrap().duplicate);
        store.close().await.unwrap();
    }
}
#[tokio::test]
async fn comment_send_pending_generation_never_claims_created() {
    let (_dir, store, _) = setup().await;
    let r = request(&save(&store, "body").await);
    store.send_comment(r).await.unwrap();
    assert_eq!(
        store.comment_draft("a", "pull").await.unwrap().reason,
        Some(CommentSendReason::PendingSubmission)
    );
    store.close().await.unwrap();
}

async fn legacy_admit(
    store: &Store,
    a: &RemoteAccount,
    r: &SendCommentRequest,
    body: &str,
) -> n::Preparation {
    let mut writer = store.inner.writer.acquire().await.unwrap();
    let mut tx = writer.begin().await.unwrap();
    let frame = operation::capture_in(&mut tx, a, "pull").await.unwrap();
    let payload = n::Payload {
        request: r.clone(),
        body: body.into(),
        repository_native: frame.repository.provider_id.clone(),
        subject_native: frame.subject.provider_id.clone(),
        number: frame.subject.number.clone().unwrap(),
    };
    let sealed = operation::seal(payload, &frame).unwrap();
    command_admission::admit_in(&mut tx, &sealed, &LegacyAdmission)
        .await
        .unwrap();
    sqlx::query("INSERT INTO comment_submissions VALUES(?,?,?,?,?,?)")
        .bind("a")
        .bind("pull")
        .bind(r.draft_generation.parse::<i64>().unwrap())
        .bind(&r.command_id)
        .bind(sealed.submission_hash().as_slice())
        .bind(n::body_hash(body))
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
async fn comment_send_encoded_budget_legacy_queue_refuses_before_http_and_attempt_but_exact_retry_survives()
 {
    let (dir, store, a) = setup().await;
    let body = "\u{1}".repeat(10_900);
    let r = request(&save(&store, &body).await);
    let prep = legacy_admit(&store, &a, &r, &body).await;
    let original = store.delivery_command("a", &r.command_id).await.unwrap();
    assert!(store.send_comment(r.clone()).await.unwrap().duplicate);
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
    assert_eq!(store.comment_draft("a", "pull").await.unwrap().body, body);
    store.close().await.unwrap();
    let store = Store::open(dir.path().join("comments.db")).await.unwrap();
    assert!(store.send_comment(r.clone()).await.unwrap().duplicate);
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
async fn comment_send_encoded_budget_preserves_previous_valid_strong_proof_and_restore() {
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
    assert!(store.send_comment(r).await.unwrap().duplicate);
    assert_eq!(server.join().unwrap().len(), 1);
    let backup = dir.path().join("legacy-proof.db");
    store.backup_to(&backup).await.unwrap();
    store.close().await.unwrap();
    let preview = RecoverySession::prepare(dir.path().join("comments.db"), &backup)
        .await
        .unwrap();
    drop(preview);
}
