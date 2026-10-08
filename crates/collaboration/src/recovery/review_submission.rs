//! Revalidate authored review drafts and exact accepted/submitted proof linkage.
use super::*;
use crate::{
    ProviderKind, ReviewDiffSide, delivery::DeliveryState, review_submission::native as n,
};

async fn refuse(db: &mut SqliteConnection, sql: &'static str) -> Result<()> {
    if sqlx::query(sql)
        .fetch_optional(db)
        .await
        .map_err(|_| invalid_backup())?
        .is_some()
    {
        return Err(invalid_backup());
    }
    Ok(())
}

fn command(row: &SqliteRow) -> Result<crate::delivery::DeliveryCommand> {
    let hash = column::<Vec<u8>>(row, "submission_hash")?;
    Ok(crate::delivery::DeliveryCommand {
        account_id: column(row, "account_id")?,
        command_id: column(row, "command_id")?,
        authorization_epoch: column::<i64>(row, "authorization_epoch")?.to_string(),
        operation_kind: column(row, "operation_kind")?,
        payload_version: u32::try_from(column::<i64>(row, "payload_version")?)
            .map_err(|_| invalid_backup())?,
        target_kind: column(row, "target_kind")?,
        target_id: column(row, "target_id")?,
        repository_id: column(row, "repository_id")?,
        canonical_envelope: Vec::new(),
        payload: column(row, "payload_bytes")?,
        guards: Vec::new(),
        hash: hash.try_into().map_err(|_| invalid_backup())?,
        enqueue_order: 0,
        admitted_at: "1970-01-01T00:00:00Z".into(),
        state: DeliveryState::parse(&column::<String>(row, "state")?)
            .map_err(|_| invalid_backup())?,
        generation: 1,
        next_action_at: None,
        reconciliation_count: 0,
        attention: None,
        attempt_count: column(row, "attempt_count")?,
        quarantine_generation: 0,
        evidence: vec![],
    })
}

fn payload(command: &crate::delivery::DeliveryCommand) -> Result<n::Payload> {
    n::decode_parts(
        &command.payload,
        &command.account_id,
        &command.command_id,
        &command.target_id,
        &command.authorization_epoch,
    )
    .map_err(|_| invalid_backup())
}

fn valid_anchor(anchor: &crate::GithubReviewLineAnchor, context: &crate::ReviewContext) -> bool {
    anchor.context.validate().is_ok()
        && anchor.context.base_oid == context.base_oid
        && anchor.context.head_oid == context.head_oid
        && anchor.context.base_repository_provider_id == context.base_repository_provider_id
        && anchor.context.source_repository_provider_id == context.source_repository_provider_id
        && anchor.context.body_metadata_facet_revision == context.metadata_facet_revision
        && anchor
            .file_facet_revision
            .parse::<i64>()
            .is_ok_and(|value| value > 0)
        && !anchor.file_key.is_empty()
        && anchor.file_key.len() <= 256
        && anchor
            .file_key
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b':'))
        && crate::is_valid_pull_file_path(&anchor.path)
        && anchor.line > 0
        && matches!(anchor.side, ReviewDiffSide::Left | ReviewDiffSide::Right)
        && match (anchor.start_line, anchor.start_side) {
            (None, None) => true,
            (Some(start), Some(side)) => start > 0 && start < anchor.line && side == anchor.side,
            _ => false,
        }
}

pub(super) async fn verify(db: &mut SqliteConnection) -> Result<()> {
    {
        let mut rows = sqlx::query("SELECT * FROM review_drafts").fetch(&mut *db);
        while let Some(row) = rows.try_next().await.map_err(|_| invalid_backup())? {
            n::identifier(&column::<String>(&row, "account_id")?).map_err(|_| invalid_backup())?;
            n::identifier(&column::<String>(&row, "subject_id")?).map_err(|_| invalid_backup())?;
            if n::parse_event(&column::<String>(&row, "event")?).is_none()
                || column::<String>(&row, "body")?.len() > 16 * 1024
                || column::<String>(&row, "body")?.contains('\0')
                || column::<i64>(&row, "generation")? <= 0
            {
                return Err(invalid_backup());
            }
        }
    }
    {
        let mut rows = sqlx::query(
            "SELECT c.*,d.generation AS draft_generation FROM review_draft_comments c JOIN review_drafts d USING(account_id,subject_id) ORDER BY c.account_id,c.subject_id,c.ordinal",
        )
        .fetch(&mut *db);
        let mut previous: Option<(String, String, i64)> = None;
        while let Some(row) = rows.try_next().await.map_err(|_| invalid_backup())? {
            let account: String = column(&row, "account_id")?;
            let subject: String = column(&row, "subject_id")?;
            let ordinal: i64 = column(&row, "ordinal")?;
            let expected = previous
                .as_ref()
                .filter(|(old_account, old_subject, _)| {
                    old_account == &account && old_subject == &subject
                })
                .map_or(0, |(_, _, old)| old + 1);
            n::canonical_uuid(&column::<String>(&row, "comment_id")?)
                .map_err(|_| invalid_backup())?;
            let body: String = column(&row, "body")?;
            if ordinal != expected
                || body.is_empty()
                || body.len() > 16 * 1024
                || body.contains('\0')
                || column::<i64>(&row, "generation")? != column::<i64>(&row, "draft_generation")?
            {
                return Err(invalid_backup());
            }
            previous = Some((account, subject, ordinal));
        }
    }
    {
        let mut rows = sqlx::query(
            "SELECT a.*,d.generation,(SELECT count(*) FROM review_draft_comments c WHERE c.account_id=a.account_id AND c.subject_id=a.subject_id) AS comment_count FROM review_draft_authority a JOIN review_drafts d USING(account_id,subject_id)",
        )
        .fetch(&mut *db);
        while let Some(row) = rows.try_next().await.map_err(|_| invalid_backup())? {
            let context_json: String = column(&row, "context_json")?;
            let anchors_json: String = column(&row, "anchors_json")?;
            let context: crate::ReviewContext =
                serde_json::from_str(&context_json).map_err(|_| invalid_backup())?;
            let anchors: Vec<crate::GithubReviewLineAnchor> =
                serde_json::from_str(&anchors_json).map_err(|_| invalid_backup())?;
            if serde_json::to_string(&context).map_err(|_| invalid_backup())? != context_json
                || serde_json::to_string(&anchors).map_err(|_| invalid_backup())? != anchors_json
                || !context.is_valid()
                || column::<i64>(&row, "draft_generation")? != column::<i64>(&row, "generation")?
                || column::<String>(&row, "authorization_epoch")?
                    .parse::<i64>()
                    .ok()
                    .is_none_or(|value| value <= 0)
                || column::<String>(&row, "authorization_view")?
                    .parse::<i64>()
                    .ok()
                    .is_none_or(|value| value < 0)
                || anchors.len()
                    != usize::try_from(column::<i64>(&row, "comment_count")?)
                        .map_err(|_| invalid_backup())?
                || anchors.len() > 25
                || anchors.iter().any(|anchor| !valid_anchor(anchor, &context))
            {
                return Err(invalid_backup());
            }
        }
    }

    refuse(db, "SELECT 1 FROM review_submissions s JOIN commands c USING(account_id,command_id) JOIN review_drafts d USING(account_id,subject_id) WHERE c.operation_kind<>'github.submit_review' OR c.payload_version<>1 OR c.target_kind<>'pull_request' OR c.target_id<>s.subject_id OR c.repository_id IS NULL OR d.generation<s.draft_generation LIMIT 1").await?;
    refuse(db, "SELECT 1 FROM commands c LEFT JOIN review_submissions s USING(account_id,command_id) WHERE c.operation_kind='github.submit_review' AND c.payload_version=1 AND s.command_id IS NULL LIMIT 1").await?;
    {
        let mut rows = sqlx::query(
            "SELECT c.*,d.event,d.body,d.generation AS current_generation,s.subject_id,s.draft_generation,s.content_hash,a.json AS account_json,cd.attempt_count FROM review_submissions s JOIN commands c USING(account_id,command_id) JOIN command_delivery cd USING(account_id,command_id) JOIN review_drafts d USING(account_id,subject_id) JOIN accounts a ON a.id=c.account_id",
        )
        .fetch(&mut *db);
        while let Some(row) = rows.try_next().await.map_err(|_| invalid_backup())? {
            let command = command(&row)?;
            let payload = payload(&command)?;
            let account: RemoteAccount =
                serde_json::from_str(&column::<String>(&row, "account_json")?)
                    .map_err(|_| invalid_backup())?;
            if account.provider != ProviderKind::Github
                || account.host != "github.com"
                || payload.actor_id != account.actor_id
                || payload.request.draft_generation
                    != column::<i64>(&row, "draft_generation")?.to_string()
                || payload.content_hash.as_slice()
                    != column::<Vec<u8>>(&row, "content_hash")?.as_slice()
                || column::<i64>(&row, "current_generation")?
                    == column::<i64>(&row, "draft_generation")?
                    && (payload.event
                        != n::parse_event(&column::<String>(&row, "event")?)
                            .ok_or_else(invalid_backup)?
                        || payload.body != column::<String>(&row, "body")?)
            {
                return Err(invalid_backup());
            }
        }
    }
    refuse(db, "SELECT 1 FROM delivery_attempts a JOIN commands c USING(account_id,command_id) LEFT JOIN delivery_attempt_context x USING(account_id,command_id,attempt_number) WHERE c.operation_kind='github.submit_review' AND c.payload_version=1 AND (x.command_id IS NULL OR a.authorization_epoch<>c.authorization_epoch) LIMIT 1").await?;
    {
        let mut rows = sqlx::query(
            "SELECT c.*,cd.attempt_count,x.execution_base FROM delivery_attempt_context x JOIN commands c USING(account_id,command_id) JOIN command_delivery cd USING(account_id,command_id) WHERE c.operation_kind='github.submit_review' AND c.payload_version=1",
        )
        .fetch(&mut *db);
        while let Some(row) = rows.try_next().await.map_err(|_| invalid_backup())? {
            let command = command(&row)?;
            let payload = payload(&command)?;
            let preparation: n::PreparationV1 =
                n::decode_evidence(&column::<Vec<u8>>(&row, "execution_base")?)
                    .map_err(|_| invalid_backup())?;
            if !n::preparation_matches(&preparation, &payload, &command) {
                return Err(invalid_backup());
            }
        }
    }

    refuse(db, "SELECT 1 FROM delivery_resolutions r JOIN commands c USING(account_id,command_id) JOIN command_evidence e ON e.account_id=r.account_id AND e.command_id=r.command_id AND e.ordinal=r.evidence_ordinal WHERE c.operation_kind='github.submit_review' AND c.payload_version=1 AND (r.purpose='accepted' AND (e.kind<>'github.review_accepted' OR e.version<>1) OR r.purpose='confirmed' AND (e.kind<>'github.review_submitted' OR e.version<>1) OR r.purpose='rejected' AND (e.kind<>'github.review_rejected' OR e.version<>1) OR r.purpose NOT IN ('accepted','confirmed','rejected')) LIMIT 1").await?;
    refuse(db, "SELECT 1 FROM command_evidence e JOIN commands c USING(account_id,command_id) LEFT JOIN delivery_resolutions r ON r.account_id=e.account_id AND r.command_id=e.command_id AND r.evidence_ordinal=e.ordinal WHERE e.kind IN ('github.review_accepted','github.review_submitted','github.review_rejected') AND e.version=1 AND (c.operation_kind<>'github.submit_review' OR c.payload_version<>1 OR r.command_id IS NULL) LIMIT 1").await?;

    {
        let mut rows = sqlx::query(
            "SELECT c.*,cd.attempt_count,e.kind,e.payload AS evidence_payload,e.ordinal,e.attempt_number,x.execution_base FROM command_evidence e JOIN commands c USING(account_id,command_id) JOIN command_delivery cd USING(account_id,command_id) LEFT JOIN delivery_attempt_context x ON x.account_id=e.account_id AND x.command_id=e.command_id AND x.attempt_number=e.attempt_number WHERE e.kind IN ('github.review_accepted','github.review_submitted','github.review_rejected') AND e.version=1",
        )
        .fetch(&mut *db);
        while let Some(row) = rows.try_next().await.map_err(|_| invalid_backup())? {
            let command = command(&row)?;
            let payload = payload(&command)?;
            let bytes: Vec<u8> = column(&row, "evidence_payload")?;
            let preparation = match column::<String>(&row, "kind")?.as_str() {
                n::ACCEPTED_PROOF => {
                    let proof: n::AcceptedEvidenceV1 =
                        n::decode_evidence(&bytes).map_err(|_| invalid_backup())?;
                    if !n::accepted_matches(&proof, &payload, &command) {
                        return Err(invalid_backup());
                    }
                    proof.preparation
                }
                n::SUBMITTED_PROOF => {
                    let proof: n::SubmittedEvidenceV1 =
                        n::decode_evidence(&bytes).map_err(|_| invalid_backup())?;
                    if !n::submitted_matches(&proof, &payload, &command) {
                        return Err(invalid_backup());
                    }
                    proof.preparation
                }
                n::REJECTED_PROOF => {
                    let proof: n::RejectedEvidenceV1 =
                        n::decode_evidence(&bytes).map_err(|_| invalid_backup())?;
                    if !n::rejected_matches(&proof, &payload, &command) {
                        return Err(invalid_backup());
                    }
                    proof.preparation
                }
                _ => return Err(invalid_backup()),
            };
            if column::<Option<i64>>(&row, "attempt_number")?.is_none()
                || n::encode_evidence(&preparation).map_err(|_| invalid_backup())?
                    != column::<Vec<u8>>(&row, "execution_base")?
            {
                return Err(invalid_backup());
            }
        }
    }

    refuse(db, "SELECT 1 FROM review_resolutions r JOIN commands c USING(account_id,command_id) LEFT JOIN delivery_resolutions d ON d.account_id=r.account_id AND d.command_id=r.command_id AND d.purpose='accepted' AND d.evidence_ordinal=r.accepted_ordinal LEFT JOIN command_evidence e ON e.account_id=r.account_id AND e.command_id=r.command_id AND e.ordinal=r.accepted_ordinal AND e.kind='github.review_accepted' AND e.version=1 WHERE c.state NOT IN ('accepted','confirmed') OR d.command_id IS NULL OR e.command_id IS NULL LIMIT 1").await?;
    refuse(db, "SELECT 1 FROM command_evidence e JOIN commands c USING(account_id,command_id) LEFT JOIN review_resolutions r ON r.account_id=e.account_id AND r.command_id=e.command_id AND r.accepted_ordinal=e.ordinal WHERE e.kind='github.review_accepted' AND e.version=1 AND r.command_id IS NULL LIMIT 1").await?;
    refuse(db, "SELECT 1 FROM review_confirmations f JOIN commands c USING(account_id,command_id) LEFT JOIN delivery_resolutions d ON d.account_id=f.account_id AND d.command_id=f.command_id AND d.purpose='confirmed' AND d.evidence_ordinal=f.confirmed_ordinal LEFT JOIN command_evidence e ON e.account_id=f.account_id AND e.command_id=f.command_id AND e.ordinal=f.confirmed_ordinal AND e.kind='github.review_submitted' AND e.version=1 WHERE c.state<>'confirmed' OR d.command_id IS NULL OR e.command_id IS NULL LIMIT 1").await?;
    refuse(db, "SELECT 1 FROM command_evidence e JOIN commands c USING(account_id,command_id) LEFT JOIN review_confirmations f ON f.account_id=e.account_id AND f.command_id=e.command_id AND f.confirmed_ordinal=e.ordinal WHERE e.kind='github.review_submitted' AND e.version=1 AND f.command_id IS NULL LIMIT 1").await?;

    let mut rows = sqlx::query(
        "SELECT c.*,cd.attempt_count,r.*,ae.payload AS accepted_payload,f.confirmed_ordinal,se.payload AS submitted_payload,f.confirmed_at,f.inline_comment_count FROM review_resolutions r JOIN commands c USING(account_id,command_id) JOIN command_delivery cd USING(account_id,command_id) JOIN command_evidence ae ON ae.account_id=r.account_id AND ae.command_id=r.command_id AND ae.ordinal=r.accepted_ordinal LEFT JOIN review_confirmations f USING(account_id,command_id,provider_id) LEFT JOIN command_evidence se ON se.account_id=f.account_id AND se.command_id=f.command_id AND se.ordinal=f.confirmed_ordinal",
    )
    .fetch(&mut *db);
    while let Some(row) = rows.try_next().await.map_err(|_| invalid_backup())? {
        let command = command(&row)?;
        let payload = payload(&command)?;
        let accepted: n::AcceptedEvidenceV1 =
            n::decode_evidence(&column::<Vec<u8>>(&row, "accepted_payload")?)
                .map_err(|_| invalid_backup())?;
        if !n::accepted_matches(&accepted, &payload, &command)
            || column::<String>(&row, "subject_id")? != command.target_id
            || column::<i64>(&row, "draft_generation")?.to_string()
                != payload.request.draft_generation
            || column::<String>(&row, "provider_id")? != accepted.receipt.provider_id
            || column::<String>(&row, "url")? != accepted.receipt.url
            || column::<String>(&row, "event")? != n::event_name(payload.event)
            || column::<String>(&row, "provider_state")? != accepted.receipt.provider_state
            || column::<String>(&row, "reviewed_commit_oid")?
                != accepted.receipt.reviewed_commit_oid
            || column::<String>(&row, "submitted_at")? != accepted.receipt.submitted_at
            || column::<String>(&row, "observed_at")? != accepted.receipt.observed_at
        {
            return Err(invalid_backup());
        }
        if let Some(bytes) = column::<Option<Vec<u8>>>(&row, "submitted_payload")? {
            let submitted: n::SubmittedEvidenceV1 =
                n::decode_evidence(&bytes).map_err(|_| invalid_backup())?;
            if !n::submitted_matches(&submitted, &payload, &command)
                || submitted.preparation != accepted.preparation
                || submitted.receipt != accepted.receipt
                || column::<String>(&row, "confirmed_at")? != submitted.confirmed_at
                || usize::try_from(column::<i64>(&row, "inline_comment_count")?)
                    .map_err(|_| invalid_backup())?
                    != submitted.comments.len()
            {
                return Err(invalid_backup());
            }
        } else if column::<Option<i64>>(&row, "confirmed_ordinal")?.is_some() {
            return Err(invalid_backup());
        }
    }
    Ok(())
}
