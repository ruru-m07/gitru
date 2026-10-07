//! Native durable claims. No HTTP or vault access is permitted under this writer.
use super::*;
use crate::ProviderInstance;
use crate::delivery::*;

pub(crate) struct DeliveryClaim {
    pub request: Option<DispatchRequest>,
    pub revision: Option<String>,
}
pub(crate) struct DeliveryCompletion<'a> {
    pub account: &'a RemoteAccount,
    pub attempt: Option<i64>,
    pub report: &'a DeliveryReport,
    pub now: &'a str,
    pub next: &'a str,
}
const CANDIDATE_PAGE: i64 = 32;
const MAX_STORED_EVIDENCE: i64 = 1_048_576;

impl Store {
    pub(crate) async fn delivery_accounts(&self, after: Option<&str>) -> Result<Vec<String>> {
        sqlx::query_scalar("SELECT id FROM accounts INDEXED BY command_delivery_active_accounts WHERE state='active' AND id>? ORDER BY id LIMIT 32").bind(after.unwrap_or("")).fetch_all(&self.inner.readers).await.map_err(storage_error)
    }
    pub(crate) async fn delivery_candidates(
        &self,
        account: &str,
        after: Option<&str>,
        limit: usize,
    ) -> Result<Vec<(String, String)>> {
        sqlx::query_as("SELECT account_id,command_id FROM commands INDEXED BY command_delivery_pending WHERE account_id=? AND command_id>? AND state IN ('queued','sending','retry_wait','accepted','outcome_unknown') AND NOT EXISTS(SELECT 1 FROM command_user_controls u WHERE u.account_id=commands.account_id AND u.command_id=commands.command_id AND u.paused=1) ORDER BY command_id LIMIT ?")
            .bind(account).bind(after.unwrap_or("")).bind((limit as i64).clamp(1,CANDIDATE_PAGE))
            .fetch_all(&self.inner.readers).await.map_err(storage_error)
    }
    pub(crate) async fn delivery_command(
        &self,
        account: &str,
        command: &str,
    ) -> Result<DeliveryCommand> {
        let mut tx = self.inner.readers.begin().await.map_err(storage_error)?;
        let result = load_in(&mut tx, account, command).await?;
        tx.commit().await.map_err(storage_error)?;
        Ok(result)
    }
    /// Called only while the runtime dispatch lane is held, so a live request
    /// cannot be mistaken for a crashed attempt in this process.
    pub(crate) async fn recover_delivery(
        &self,
        expected: &DeliveryCommand,
        now: &str,
    ) -> Result<String> {
        let mut writer = self.inner.writer.acquire().await?;
        let mut tx = writer.begin().await.map_err(storage_error)?;
        let command = exact_in(&mut tx, expected).await?;
        if command.state != DeliveryState::Sending {
            return Err(stale());
        }
        sqlx::query("UPDATE delivery_attempts SET outcome='outcome_unknown',completed_at=? WHERE account_id=? AND command_id=? AND outcome='started'")
            .bind(now).bind(&command.account_id).bind(&command.command_id).execute(&mut *tx).await.map_err(storage_error)?;
        let revision = transition_in(&mut tx, &command, DeliveryState::Unknown, None, None).await?;
        tx.commit().await.map_err(storage_error)?;
        Ok(revision)
    }
    pub(crate) async fn defer_delivery(
        &self,
        expected: &DeliveryCommand,
        next: &str,
    ) -> Result<String> {
        let mut writer = self.inner.writer.acquire().await?;
        let mut tx = writer.begin().await.map_err(storage_error)?;
        let command = exact_in(&mut tx, expected).await?;
        let revision = transition_in(&mut tx, &command, command.state, Some(next), None).await?;
        tx.commit().await.map_err(storage_error)?;
        Ok(revision)
    }
    pub(crate) async fn claim_preparation(
        &self,
        expected: &DeliveryCommand,
        account: &RemoteAccount,
        policy: &dyn CommandDeliveryPolicy,
        now: &DeliveryTime,
    ) -> Result<(Option<ReconcileRequest>, Option<String>)> {
        let mut writer = self.inner.writer.acquire().await?;
        let mut tx = writer.begin().await.map_err(storage_error)?;
        let command = exact_in(&mut tx, expected).await?;
        policy_matches(&command, policy)?;
        let instance = authorize_in(&mut tx, &command, account, now).await?;
        if command.authorization_epoch != account.authorization_epoch
            || command.reconcile_only()
            || !matches!(
                command.state,
                DeliveryState::Queued | DeliveryState::RetryWait
            )
        {
            return Err(stale());
        }
        if blocked_in(&mut tx, &command).await? {
            return Ok((None, None));
        }
        let attention = if command.attempt_count >= MAX_ATTEMPTS {
            Some("attempt_limit")
        } else if age_exceeded(&command.admitted_at, &now.command_now)? {
            Some("age_limit")
        } else if command.reconciliation_count >= MAX_RECONCILIATIONS {
            Some("reconciliation_limit")
        } else if evidence_full(&command) {
            Some("evidence_limit")
        } else {
            None
        };
        let revision = transition_in(&mut tx, &command, command.state, None, attention).await?;
        let request = if attention.is_none() {
            sqlx::query("UPDATE command_delivery SET reconciliation_count=reconciliation_count+1,generation=generation+1 WHERE account_id=? AND command_id=?").bind(&command.account_id).bind(&command.command_id).execute(&mut *tx).await.map_err(storage_error)?;
            let native_context = policy
                .prepare_context_in(&mut tx, &command, account)
                .await?;
            if native_context.len() > MAX_EVIDENCE_BYTES {
                return Err(CollaborationError::invalid(
                    "Oversized native delivery context",
                ));
            }
            Some(ReconcileRequest {
                native_context,
                command: load_in(&mut tx, &command.account_id, &command.command_id).await?,
                account: account.clone(),
                instance_id: instance,
            })
        } else {
            None
        };
        tx.commit().await.map_err(storage_error)?;
        Ok((request, Some(revision)))
    }
    pub(crate) async fn claim_delivery(
        &self,
        expected: &DeliveryCommand,
        account: &RemoteAccount,
        policy: &dyn CommandDeliveryPolicy,
        preparation: &[u8],
        now: &DeliveryTime,
    ) -> Result<DeliveryClaim> {
        if preparation.len() > MAX_EVIDENCE_BYTES {
            return Err(CollaborationError::invalid(
                "Oversized delivery preparation",
            ));
        }
        let mut writer = self.inner.writer.acquire().await?;
        let mut tx = writer.begin().await.map_err(storage_error)?;
        let command = exact_in(&mut tx, expected).await?;
        let instance = authorize_in(&mut tx, &command, account, now).await?;
        if command.authorization_epoch != account.authorization_epoch
            || command.reconcile_only()
            || !matches!(
                command.state,
                DeliveryState::Queued | DeliveryState::RetryWait
            )
        {
            return Err(stale());
        }
        policy_matches(&command, policy)?;
        // A retry needs a durable policy-certified safety receipt from the last
        // result, never merely a mutable state value.
        if command.state == DeliveryState::RetryWait
            && !last_resolution_in(&mut tx, &command, "safe_retry").await?
        {
            return Err(stale());
        }
        let attention = if command.attempt_count >= MAX_ATTEMPTS {
            Some("attempt_limit")
        } else if age_exceeded(&command.admitted_at, &now.command_now)? {
            Some("age_limit")
        } else if evidence_full(&command) {
            Some("evidence_limit")
        } else {
            None
        };
        if let Some(attention) = attention {
            let revision =
                transition_in(&mut tx, &command, command.state, None, Some(attention)).await?;
            tx.commit().await.map_err(storage_error)?;
            return Ok(DeliveryClaim {
                request: None,
                revision: Some(revision),
            });
        }
        if blocked_in(&mut tx, &command).await? {
            return Ok(DeliveryClaim {
                request: None,
                revision: None,
            });
        }
        let execution_base = match policy
            .validate_claim(&mut tx, &command, account, preparation)
            .await?
        {
            ClaimDecision::Ready(base) if base.len() <= MAX_EVIDENCE_BYTES => base,
            ClaimDecision::Ready(_) => {
                return Err(CollaborationError::invalid("Oversized execution base"));
            }
            resolution @ (ClaimDecision::Conflict(_) | ClaimDecision::Confirmed(_)) => {
                let (purpose, state, proof) = match resolution {
                    ClaimDecision::Conflict(proof) => {
                        (EvidencePurpose::Conflict, DeliveryState::Conflict, proof)
                    }
                    ClaimDecision::Confirmed(proof) => {
                        (EvidencePurpose::Confirmed, DeliveryState::Confirmed, proof)
                    }
                    ClaimDecision::Ready(_) => unreachable!(),
                };
                validate_proof(policy, &command, purpose, &proof)?;
                record_proof_in(&mut tx, &command, None, purpose, &proof, &now.now).await?;
                let mut finalization = super::effective::finalization::DeliveryFinalization::new(
                    &mut tx, &command, account, purpose,
                )
                .await?;
                policy
                    .finalize_in(&mut finalization, &command, purpose, &proof)
                    .await?;
                finalization.finish().await?;
                let revision = transition_in(&mut tx, &command, state, None, None).await?;
                tx.commit().await.map_err(storage_error)?;
                return Ok(DeliveryClaim {
                    request: None,
                    revision: Some(revision),
                });
            }
        };
        let attempt = command.attempt_count + 1;
        sqlx::query("INSERT INTO delivery_attempts(account_id,command_id,attempt_number,authorization_epoch,started_at,outcome) VALUES(?,?,?,?,?,'started')")
            .bind(&command.account_id).bind(&command.command_id).bind(attempt).bind(positive_revision(&account.authorization_epoch)?).bind(&now.now).execute(&mut *tx).await.map_err(storage_error)?;
        sqlx::query("INSERT INTO delivery_attempt_context VALUES(?,?,?,?,?)")
            .bind(&command.account_id)
            .bind(&command.command_id)
            .bind(attempt)
            .bind(&instance)
            .bind(&execution_base)
            .execute(&mut *tx)
            .await
            .map_err(storage_error)?;
        let revision = transition_in(&mut tx, &command, DeliveryState::Sending, None, None).await?;
        sqlx::query("UPDATE command_delivery SET reconciliation_count=0,generation=generation+1 WHERE account_id=? AND command_id=?").bind(&command.account_id).bind(&command.command_id).execute(&mut *tx).await.map_err(storage_error)?;
        let command = load_in(&mut tx, &command.account_id, &command.command_id).await?;
        checkpoint("before_claim_commit");
        tx.commit().await.map_err(storage_error)?;
        checkpoint("after_claim_commit");
        Ok(DeliveryClaim {
            request: Some(DispatchRequest {
                command,
                account: account.clone(),
                instance_id: instance,
                attempt,
                execution_base,
            }),
            revision: Some(revision),
        })
    }
    /// Persist a bounded read-only probe before provider observation, so process
    /// crashes cannot reset an unbounded reconciliation loop.
    pub(crate) async fn claim_reconciliation(
        &self,
        expected: &DeliveryCommand,
        account: &RemoteAccount,
        policy: &dyn CommandDeliveryPolicy,
        now: &DeliveryTime,
    ) -> Result<(Option<ReconcileRequest>, String)> {
        let mut writer = self.inner.writer.acquire().await?;
        let mut tx = writer.begin().await.map_err(storage_error)?;
        let command = exact_in(&mut tx, expected).await?;
        policy_matches(&command, policy)?;
        let instance = authorize_in(&mut tx, &command, account, now).await?;
        if !command.reconcile_only()
            || !matches!(
                command.state,
                DeliveryState::Queued
                    | DeliveryState::RetryWait
                    | DeliveryState::Accepted
                    | DeliveryState::Unknown
            )
        {
            return Err(stale());
        }
        let attention = if command.reconciliation_count >= MAX_RECONCILIATIONS {
            Some("reconciliation_limit")
        } else if evidence_full(&command) {
            Some("evidence_limit")
        } else {
            None
        };
        let revision = transition_in(&mut tx, &command, command.state, None, attention).await?;
        let request = if attention.is_none() {
            sqlx::query("UPDATE command_delivery SET reconciliation_count=reconciliation_count+1,generation=generation+1 WHERE account_id=? AND command_id=?").bind(&command.account_id).bind(&command.command_id).execute(&mut *tx).await.map_err(storage_error)?;
            let native_context = policy
                .prepare_context_in(&mut tx, &command, account)
                .await?;
            if native_context.len() > MAX_EVIDENCE_BYTES {
                return Err(CollaborationError::invalid(
                    "Oversized native delivery context",
                ));
            }
            Some(ReconcileRequest {
                native_context,
                command: load_in(&mut tx, &command.account_id, &command.command_id).await?,
                account: account.clone(),
                instance_id: instance,
            })
        } else {
            None
        };
        tx.commit().await.map_err(storage_error)?;
        Ok((request, revision))
    }
    pub(crate) async fn complete_delivery(
        &self,
        expected: &DeliveryCommand,
        policy: &dyn CommandDeliveryPolicy,
        completion: DeliveryCompletion<'_>,
    ) -> Result<String> {
        let DeliveryCompletion {
            account,
            attempt,
            report,
            now,
            next,
        } = completion;
        let mut writer = self.inner.writer.acquire().await?;
        let mut tx = writer.begin().await.map_err(storage_error)?;
        let command = exact_in(&mut tx, expected).await?;
        binding_in(&mut tx, &command, account).await?;
        policy_matches(&command, policy)?;
        if let Some(attempt) = attempt {
            if command.state != DeliveryState::Sending
                || command.attempt_count != attempt
                || command.authorization_epoch != account.authorization_epoch
                || command.quarantine_generation > 0
            {
                return Err(stale());
            }
        } else if !command.reconcile_only() {
            return Err(stale());
        }
        let outcome = if let Some((purpose, proof)) = report.outcome.proof() {
            validate_proof(policy, &command, purpose, proof)?;
            record_proof_in(
                &mut tx,
                &command,
                attempt.or((command.attempt_count > 0).then_some(command.attempt_count)),
                purpose,
                proof,
                now,
            )
            .await?;
            let mut finalization = super::effective::finalization::DeliveryFinalization::new(
                &mut tx, &command, account, purpose,
            )
            .await?;
            policy
                .finalize_in(&mut finalization, &command, purpose, proof)
                .await?;
            finalization.finish().await?;
            match purpose {
                EvidencePurpose::Confirmed => DeliveryState::Confirmed,
                EvidencePurpose::Accepted => DeliveryState::Accepted,
                EvidencePurpose::Rejected => DeliveryState::Rejected,
                EvidencePurpose::Conflict => DeliveryState::Conflict,
                EvidencePurpose::SafeRetry if command.quarantine_generation == 0 => {
                    DeliveryState::RetryWait
                }
                EvidencePurpose::SafeRetry => DeliveryState::Unknown,
            }
        } else if command.state == DeliveryState::Accepted {
            DeliveryState::Accepted
        } else {
            DeliveryState::Unknown
        };
        let pending = matches!(
            outcome,
            DeliveryState::Accepted | DeliveryState::Unknown | DeliveryState::RetryWait
        );
        // A reconciliation has no new dispatch attempt. Preserve old attempts,
        // including restored historical receipts, and append independent proof.
        if let Some(attempt) = attempt {
            let attempt_outcome = match outcome {
                DeliveryState::Confirmed => "confirmed",
                DeliveryState::Accepted => "accepted",
                DeliveryState::Rejected | DeliveryState::Conflict | DeliveryState::RetryWait => {
                    "rejected"
                }
                _ => "outcome_unknown",
            };
            let changed=sqlx::query("UPDATE delivery_attempts SET outcome=?,completed_at=? WHERE account_id=? AND command_id=? AND attempt_number=? AND outcome='started'")
                .bind(attempt_outcome).bind(now).bind(&command.account_id).bind(&command.command_id).bind(attempt).execute(&mut *tx).await.map_err(storage_error)?;
            if changed.rows_affected() != 1 {
                return Err(stale());
            }
        }
        let revision =
            transition_in(&mut tx, &command, outcome, pending.then_some(next), None).await?;
        checkpoint("before_result_commit");
        tx.commit().await.map_err(storage_error)?;
        checkpoint("after_result_commit");
        Ok(revision)
    }
}
fn policy_matches(command: &DeliveryCommand, policy: &dyn CommandDeliveryPolicy) -> Result<()> {
    if command.operation_kind != policy.operation_kind()
        || command.payload_version != policy.payload_version()
    {
        return Err(CollaborationError::invalid(
            "Delivery policy does not match intent",
        ));
    }
    Ok(())
}
fn validate_proof(
    policy: &dyn CommandDeliveryPolicy,
    command: &DeliveryCommand,
    purpose: EvidencePurpose,
    proof: &OperationEvidence,
) -> Result<()> {
    if !proof.bounded() || !policy.validate_evidence(command, purpose, proof) {
        return Err(CollaborationError::invalid("Unverified operation evidence"));
    }
    Ok(())
}
fn evidence_full(command: &DeliveryCommand) -> bool {
    command.evidence.len() >= 127
        || command
            .evidence
            .iter()
            .map(|e| e.evidence.payload.len())
            .sum::<usize>()
            > MAX_STORED_EVIDENCE as usize - MAX_EVIDENCE_BYTES
}
fn age_exceeded(admitted: &str, now: &str) -> Result<bool> {
    let admitted = chrono::DateTime::parse_from_rfc3339(admitted)
        .map_err(|_| CollaborationError::storage())?;
    let now =
        chrono::DateTime::parse_from_rfc3339(now).map_err(|_| CollaborationError::storage())?;
    Ok(now.signed_duration_since(admitted) > chrono::Duration::days(30))
}
async fn authorize_in(
    tx: &mut Transaction<'_, Sqlite>,
    command: &DeliveryCommand,
    account: &RemoteAccount,
    now: &DeliveryTime,
) -> Result<String> {
    if command.account_id != account.id
        || command.attention.is_some()
        || super::command_recovery::paused_in(tx, &command.account_id, &command.command_id).await?
    {
        return Err(stale());
    }
    let instance = binding_in(tx, command, account).await?;
    let command_now = chrono::DateTime::parse_from_rfc3339(&now.command_now)
        .map_err(|_| CollaborationError::storage())?;
    let wall_now = chrono::DateTime::parse_from_rfc3339(&now.now)
        .map_err(|_| CollaborationError::storage())?;
    let budget: Option<String> = sqlx::query_scalar(
        "SELECT sync_json FROM sync_scopes WHERE account_id=? AND scope='provider:rest'",
    )
    .bind(&account.id)
    .fetch_optional(&mut **tx)
    .await
    .map_err(storage_error)?;
    let budget = budget
        .map(|s| serde_json::from_str::<SyncStatus>(&s))
        .transpose()
        .map_err(|_| CollaborationError::storage())?
        .and_then(|s| s.next_retry_at);
    for (deadline, now) in command
        .next_action_at
        .iter()
        .map(|d| (d, command_now))
        .chain(budget.iter().map(|d| (d, wall_now)))
    {
        if chrono::DateTime::parse_from_rfc3339(deadline)
            .map_err(|_| CollaborationError::storage())?
            > now
        {
            return Err(CollaborationError::new(
                ErrorCode::RateLimited,
                "Delivery is waiting for its next permitted request",
            ));
        }
    }
    Ok(instance)
}
async fn exact_in(
    tx: &mut Transaction<'_, Sqlite>,
    expected: &DeliveryCommand,
) -> Result<DeliveryCommand> {
    let current = load_in(tx, &expected.account_id, &expected.command_id).await?;
    if current.generation != expected.generation
        || current.state != expected.state
        || current.hash != expected.hash
        || current.quarantine_generation != expected.quarantine_generation
    {
        return Err(stale());
    }
    Ok(current)
}
pub(super) async fn load_in(
    tx: &mut Transaction<'_, Sqlite>,
    account: &str,
    id: &str,
) -> Result<DeliveryCommand> {
    let row=sqlx::query("SELECT c.*,d.generation,d.next_action_at,d.reconciliation_count,d.attention,(SELECT count(*) FROM delivery_attempts a WHERE a.account_id=c.account_id AND a.command_id=c.command_id) AS attempt_count,coalesce((SELECT max(q.recovery_generation) FROM command_recovery_quarantine q WHERE q.account_id=c.account_id AND q.command_id=c.command_id),0) AS quarantine_generation FROM commands c JOIN command_delivery d USING(account_id,command_id) WHERE c.account_id=? AND c.command_id=?")
        .bind(account).bind(id).fetch_optional(&mut **tx).await.map_err(storage_error)?.ok_or_else(stale)?;
    // The schema bounds legacy histories at 128 x 64KiB; new writes have an
    // additional aggregate 1MiB admission bound. Fetch one command at a time.
    let rows=sqlx::query("SELECT ordinal,attempt_number,kind,version,payload FROM command_evidence WHERE account_id=? AND command_id=? ORDER BY ordinal LIMIT 128").bind(account).bind(id).fetch_all(&mut **tx).await.map_err(storage_error)?;
    let evidence = rows
        .into_iter()
        .map(|r| {
            Ok(RecordedEvidence {
                ordinal: r.try_get("ordinal").map_err(storage_error)?,
                attempt: r.try_get("attempt_number").map_err(storage_error)?,
                evidence: OperationEvidence {
                    kind: r.try_get("kind").map_err(storage_error)?,
                    version: u32::try_from(r.try_get::<i64, _>("version").map_err(storage_error)?)
                        .map_err(|_| CollaborationError::storage())?,
                    payload: r.try_get("payload").map_err(storage_error)?,
                },
            })
        })
        .collect::<Result<_>>()?;
    Ok(DeliveryCommand {
        account_id: account.into(),
        command_id: id.into(),
        authorization_epoch: row
            .try_get::<i64, _>("authorization_epoch")
            .map_err(storage_error)?
            .to_string(),
        operation_kind: row.try_get("operation_kind").map_err(storage_error)?,
        payload_version: u32::try_from(
            row.try_get::<i64, _>("payload_version")
                .map_err(storage_error)?,
        )
        .map_err(|_| CollaborationError::storage())?,
        target_kind: row.try_get("target_kind").map_err(storage_error)?,
        target_id: row.try_get("target_id").map_err(storage_error)?,
        repository_id: row.try_get("repository_id").map_err(storage_error)?,
        canonical_envelope: row.try_get("canonical_envelope").map_err(storage_error)?,
        payload: row.try_get("payload_bytes").map_err(storage_error)?,
        guards: row.try_get("guard_bytes").map_err(storage_error)?,
        hash: row
            .try_get::<Vec<u8>, _>("submission_hash")
            .map_err(storage_error)?
            .try_into()
            .map_err(|_| CollaborationError::storage())?,
        enqueue_order: row.try_get("enqueue_order").map_err(storage_error)?,
        admitted_at: row.try_get("admitted_at").map_err(storage_error)?,
        state: DeliveryState::parse(row.try_get("state").map_err(storage_error)?)?,
        generation: row.try_get("generation").map_err(storage_error)?,
        next_action_at: row.try_get("next_action_at").map_err(storage_error)?,
        reconciliation_count: row.try_get("reconciliation_count").map_err(storage_error)?,
        attention: row.try_get("attention").map_err(storage_error)?,
        attempt_count: row.try_get("attempt_count").map_err(storage_error)?,
        quarantine_generation: row
            .try_get("quarantine_generation")
            .map_err(storage_error)?,
        evidence,
    })
}
pub(super) async fn transition_in(
    tx: &mut Transaction<'_, Sqlite>,
    command: &DeliveryCommand,
    state: DeliveryState,
    next: Option<&str>,
    attention: Option<&str>,
) -> Result<String> {
    sqlx::query("UPDATE commands SET state=? WHERE account_id=? AND command_id=?")
        .bind(state.name())
        .bind(&command.account_id)
        .bind(&command.command_id)
        .execute(&mut **tx)
        .await
        .map_err(storage_error)?;
    sqlx::query("UPDATE command_delivery SET generation=generation+1,next_action_at=?,attention=? WHERE account_id=? AND command_id=?").bind(next).bind(attention).bind(&command.account_id).bind(&command.command_id).execute(&mut **tx).await.map_err(storage_error)?;
    super::effective::refresh_target_in(tx, &command.account_id, &command.target_id).await?;
    let account = account_in(tx, &command.account_id, false).await?;
    record_change(
        tx,
        &command.account_id,
        positive_revision(&account.authorization_epoch)?,
        "commands",
        false,
    )
    .await
}
async fn last_resolution_in(
    tx: &mut Transaction<'_, Sqlite>,
    command: &DeliveryCommand,
    purpose: &str,
) -> Result<bool> {
    let latest:Option<String>=sqlx::query_scalar("SELECT purpose FROM delivery_resolutions WHERE account_id=? AND command_id=? ORDER BY delivery_generation DESC LIMIT 1").bind(&command.account_id).bind(&command.command_id).fetch_optional(&mut **tx).await.map_err(storage_error)?;
    Ok(latest.as_deref() == Some(purpose))
}
fn purpose_name(purpose: EvidencePurpose) -> &'static str {
    match purpose {
        EvidencePurpose::Accepted => "accepted",
        EvidencePurpose::Confirmed => "confirmed",
        EvidencePurpose::Rejected => "rejected",
        EvidencePurpose::Conflict => "conflict",
        EvidencePurpose::SafeRetry => "safe_retry",
    }
}
async fn record_proof_in(
    tx: &mut Transaction<'_, Sqlite>,
    command: &DeliveryCommand,
    attempt: Option<i64>,
    purpose: EvidencePurpose,
    proof: &OperationEvidence,
    now: &str,
) -> Result<()> {
    if evidence_full(command) {
        return Err(CollaborationError::invalid(
            "Operation evidence limit reached",
        ));
    }
    let ordinal = command.evidence.last().map_or(0, |e| e.ordinal + 1);
    sqlx::query("INSERT INTO command_evidence VALUES(?,?,?,?,?,?,?,?)")
        .bind(&command.account_id)
        .bind(&command.command_id)
        .bind(ordinal)
        .bind(attempt)
        .bind(&proof.kind)
        .bind(i64::from(proof.version))
        .bind(&proof.payload)
        .bind(now)
        .execute(&mut **tx)
        .await
        .map_err(storage_error)?;
    sqlx::query("INSERT INTO delivery_resolutions VALUES(?,?,?,?,?)")
        .bind(&command.account_id)
        .bind(&command.command_id)
        .bind(command.generation + 1)
        .bind(ordinal)
        .bind(purpose_name(purpose))
        .execute(&mut **tx)
        .await
        .map_err(storage_error)?;
    Ok(())
}
fn checkpoint(_name: &str) {
    #[cfg(test)]
    if std::env::var("GITRU_DELIVERY_CRASH").ok().as_deref() == Some(_name) {
        std::process::exit(91);
    }
}

async fn blocked_in(tx: &mut Transaction<'_, Sqlite>, command: &DeliveryCommand) -> Result<bool> {
    let order = super::command_recovery::execution_order_in(tx, command).await?;
    // Original submitted dependencies are never rewritten or satisfied by a
    // supersession. A blocked successor needs its own reviewed new submission.
    let blocked:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM command_dependencies d JOIN commands p ON p.account_id=d.account_id AND p.command_id=d.predecessor_id WHERE d.account_id=? AND d.command_id=? AND (p.state<>'confirmed' OR NOT EXISTS(SELECT 1 FROM delivery_resolutions r WHERE r.account_id=p.account_id AND r.command_id=p.command_id AND r.purpose='confirmed'))) OR EXISTS(SELECT 1 FROM commands p LEFT JOIN command_supersessions s ON s.account_id=p.account_id AND s.replacement_id=p.command_id WHERE p.account_id=? AND p.target_kind=? AND p.target_id=? AND coalesce(s.execution_order,p.enqueue_order)<? AND p.state IN ('queued','sending','retry_wait','accepted','outcome_unknown','conflict'))")
        .bind(&command.account_id).bind(&command.command_id).bind(&command.account_id).bind(&command.target_kind).bind(&command.target_id).bind(order).fetch_one(&mut **tx).await.map_err(storage_error)?;
    Ok(blocked)
}

async fn binding_in(
    tx: &mut Transaction<'_, Sqlite>,
    command: &DeliveryCommand,
    account: &RemoteAccount,
) -> Result<String> {
    if command.account_id != account.id {
        return Err(stale());
    }
    epoch_in(tx, &account.id, &account.authorization_epoch).await?;
    let stored = account_in(tx, &account.id, true).await?;
    let instance = ProviderInstance::for_account(&stored)?.id;
    if stored.actor_id != account.actor_id
        || stored.provider != account.provider
        || stored.host != account.host
    {
        return Err(stale());
    }
    let bound: String =
        sqlx::query_scalar("SELECT instance_id FROM account_instances WHERE account_id=?")
            .bind(&account.id)
            .fetch_one(&mut **tx)
            .await
            .map_err(storage_error)?;
    if bound != instance {
        return Err(stale());
    }
    Ok(instance)
}
