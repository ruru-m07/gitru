//! Account-scoped local recovery. No credential/provider access in this module.
use super::*;
use crate::command_recovery::policy::*;
use crate::command_recovery::*;
use crate::delivery::{DeliveryCommand, DeliveryState, MAX_ATTEMPTS, MAX_RECONCILIATIONS};

#[derive(Serialize, Deserialize)]
struct RecoveryCursor {
    account: String,
    target: Option<String>,
    terminal: bool,
    view: String,
    revision: String,
    before: i64,
}

impl Store {
    pub(crate) async fn command_recovery_list(
        &self,
        query: CommandRecoveryQuery,
    ) -> Result<CommandRecoverySnapshot> {
        validate_identifier(&query.account_id)?;
        if let Some(target) = &query.target_id {
            validate_identifier(target)?;
        }
        if query.limit == 0
            || query.limit > 50
            || query.cursor.as_ref().is_some_and(|c| c.len() > 4096)
        {
            return Err(invalid());
        }
        let mut tx = self
            .inner
            .readers
            .begin()
            .await
            .map_err(|_| CollaborationError::storage())?;
        account_in(&mut tx, &query.account_id, false).await?;
        let (revision, view) = metadata(&mut tx).await?;
        let before = if let Some(cursor) = &query.cursor {
            let c: RecoveryCursor = decode(cursor)?;
            if c.account != query.account_id
                || c.target != query.target_id
                || c.terminal != query.include_terminal
                || c.view != view
                || c.revision != revision
                || c.before <= 0
            {
                return Err(stale());
            }
            c.before
        } else {
            i64::MAX
        };
        let mut sql = sqlx::QueryBuilder::<Sqlite>::new(
            "SELECT c.account_id,c.command_id,c.target_id,c.target_kind,c.operation_kind,c.payload_version,c.state,c.admitted_at,c.enqueue_order,d.attention,(SELECT count(*) FROM delivery_attempts a WHERE a.account_id=c.account_id AND a.command_id=c.command_id) attempt_count,EXISTS(SELECT 1 FROM command_recovery_quarantine q WHERE q.account_id=c.account_id AND q.command_id=c.command_id) quarantined,coalesce(u.paused,0) paused,s.replacement_id FROM commands c JOIN command_delivery d USING(account_id,command_id) LEFT JOIN command_user_controls u USING(account_id,command_id) LEFT JOIN command_supersessions s ON s.account_id=c.account_id AND s.original_id=c.command_id WHERE c.account_id=",
        );
        sql.push_bind(&query.account_id)
            .push(" AND c.enqueue_order<")
            .push_bind(before);
        if let Some(target) = &query.target_id {
            sql.push(" AND c.target_id=").push_bind(target);
        }
        if !query.include_terminal {
            sql.push(" AND c.state IN ('queued','sending','retry_wait','accepted','outcome_unknown','conflict')");
        }
        sql.push(" ORDER BY c.enqueue_order DESC LIMIT ")
            .push_bind(i64::from(query.limit) + 1);
        let rows = sql
            .build()
            .fetch_all(&mut *tx)
            .await
            .map_err(|_| CollaborationError::storage())?;
        let more = rows.len() > query.limit as usize;
        let mut commands = vec![];
        let mut last = before;
        for row in rows.into_iter().take(query.limit as usize) {
            last = row.get("enqueue_order");
            let mut summary = summary_from_row(&row)?;
            summary.blocked_reason =
                blocked_reason_in(&mut tx, &summary.account_id, &summary.command_id).await?;
            commands.push(summary);
        }
        let next_cursor = if more {
            Some(encode(&RecoveryCursor {
                account: query.account_id,
                target: query.target_id,
                terminal: query.include_terminal,
                view: view.clone(),
                revision: revision.clone(),
                before: last,
            })?)
        } else {
            None
        };
        tx.commit()
            .await
            .map_err(|_| CollaborationError::storage())?;
        Ok(CommandRecoverySnapshot {
            commands,
            next_cursor,
            revision,
            authorization_view: view,
        })
    }

    pub(crate) async fn command_recovery_detail(
        &self,
        account: &str,
        id: &str,
        policy: Option<&dyn CommandRecoveryPolicy>,
    ) -> Result<CommandRecoveryDetail> {
        validate_identifier(account)?;
        validate_uuid(id)?;
        let mut tx = self
            .inner
            .readers
            .begin()
            .await
            .map_err(|_| CollaborationError::storage())?;
        let (detail, _, _) = detail_in(&mut tx, account, id, policy).await?;
        tx.commit()
            .await
            .map_err(|_| CollaborationError::storage())?;
        Ok(detail)
    }

    pub(crate) async fn command_recovery_action(
        &self,
        request: CommandRecoveryActionRequest,
        policy: Option<&dyn CommandRecoveryPolicy>,
    ) -> Result<CommandRecoveryReceipt> {
        validate_request(&request.context, &request.action_id)?;
        let request_json = encode(&request)?;
        let mut writer = self.inner.writer.acquire().await?;
        let mut tx = writer
            .begin()
            .await
            .map_err(|_| CollaborationError::storage())?;
        validate_account_view_in(&mut tx, &request.context).await?;
        if let Some(receipt) = duplicate_in(
            &mut tx,
            &request.context.account_id,
            &request.action_id,
            &request_json,
        )
        .await?
        {
            tx.commit()
                .await
                .map_err(|_| CollaborationError::storage())?;
            return Ok(receipt);
        }
        let (detail, command, _) = checked_in(&mut tx, &request.context, policy).await?;
        let allowed = match request.action {
            CommandRecoveryAction::Cancel => detail.can_cancel,
            CommandRecoveryAction::Pause => detail.can_pause,
            CommandRecoveryAction::Resume => detail.can_retry,
        };
        if !allowed {
            return Err(CollaborationError::new(
                ErrorCode::Unsupported,
                "This recovery action is unavailable for the current command",
            ));
        }
        let paused = match request.action {
            CommandRecoveryAction::Cancel | CommandRecoveryAction::Resume => false,
            CommandRecoveryAction::Pause => true,
        };
        sqlx::query("INSERT INTO command_user_controls(account_id,command_id,paused) VALUES(?,?,?) ON CONFLICT(account_id,command_id) DO UPDATE SET paused=excluded.paused")
            .bind(&command.account_id).bind(&command.command_id).bind(paused).execute(&mut *tx).await.map_err(|_| CollaborationError::storage())?;
        let state = if request.action == CommandRecoveryAction::Cancel {
            DeliveryState::Cancelled
        } else {
            command.state
        };
        // A pause/resume keeps quota, attempt/probe counters and deadlines.
        let revision = super::delivery::transition_in(
            &mut tx,
            &command,
            state,
            command.next_action_at.as_deref(),
            command.attention.as_deref(),
        )
        .await?;
        let receipt = CommandRecoveryReceipt {
            account_id: command.account_id.clone(),
            action_id: request.action_id.clone(),
            command_id: command.command_id.clone(),
            replacement_id: None,
            state: state.name().into(),
            paused,
            remote_may_have_happened: command.attempt_count > 0
                || command.quarantine_generation > 0,
            revision,
        };
        record_action_in(
            &mut tx,
            &command,
            &request.action_id,
            &request_json,
            &receipt,
        )
        .await?;
        tx.commit()
            .await
            .map_err(|_| CollaborationError::storage())?;
        Ok(receipt)
    }

    pub(crate) async fn command_recovery_replace(
        &self,
        request: CommandRecoveryReplaceRequest,
        policy: Option<&dyn CommandRecoveryPolicy>,
    ) -> Result<CommandRecoveryReceipt> {
        validate_request(&request.context, &request.action_id)?;
        validate_uuid(&request.new_command_id)?;
        if request.new_command_id == request.context.command_id || request.fields.len() > 5 {
            return Err(invalid());
        }
        let request_json = encode(&request)?;
        if request_json.len() > MAX_REVIEW_BYTES {
            return Err(invalid());
        }
        let mut writer = self.inner.writer.acquire().await?;
        let mut tx = writer
            .begin()
            .await
            .map_err(|_| CollaborationError::storage())?;
        validate_account_view_in(&mut tx, &request.context).await?;
        if let Some(receipt) = duplicate_in(
            &mut tx,
            &request.context.account_id,
            &request.action_id,
            &request_json,
        )
        .await?
        {
            tx.commit()
                .await
                .map_err(|_| CollaborationError::storage())?;
            return Ok(receipt);
        }
        let (detail, command, review) = checked_in(&mut tx, &request.context, policy).await?;
        if !detail.can_replace {
            return Err(CollaborationError::new(
                ErrorCode::Unsupported,
                "This command requires fresh delivery evidence before replacement",
            ));
        }
        validate_choices(&request.fields, &review.fields)?;
        let policy = policy.ok_or_else(invalid)?;
        let account = account_in(&mut tx, &command.account_id, true).await?;
        let depth = depth_in(&mut tx, &command.account_id, &command.command_id).await?;
        if depth >= MAX_REPLACEMENT_DEPTH {
            return Err(CollaborationError::new(
                ErrorCode::Busy,
                "Replacement history limit reached; export this intent for review",
            ));
        }
        let occupied: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM commands WHERE account_id=? AND command_id=?)",
        )
        .bind(&command.account_id)
        .bind(&request.new_command_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(|_| CollaborationError::storage())?;
        if occupied {
            return Err(invalid());
        }
        // Reserve exactly the original's active-effect slot, within this same
        // transaction. No reader can see retirement without the replacement;
        // any admission/policy/edge/receipt failure restores all original state.
        super::delivery::transition_in(&mut tx, &command, DeliveryState::Superseded, None, None)
            .await?;
        let admitted = policy
            .replace_in(&mut tx, &command, &account, &request, &review)
            .await?;
        let replacement =
            super::delivery::load_in(&mut tx, &command.account_id, &request.new_command_id).await?;
        if admitted.duplicate
            || admitted.command_id != request.new_command_id
            || admitted.account_id != command.account_id
            || admitted.submission_hash != replacement.hash
            || replacement.authorization_epoch != command.authorization_epoch
            || replacement.state != DeliveryState::Queued
            || replacement.quarantine_generation > 0
            || replacement.attempt_count != 0
            || replacement.target_kind != command.target_kind
            || replacement.target_id != command.target_id
            || replacement.repository_id != command.repository_id
            || replacement.operation_kind != command.operation_kind
            || replacement.payload_version != command.payload_version
            || replacement.enqueue_order <= command.enqueue_order
        {
            return Err(invalid());
        }
        let execution_order = execution_order_in(&mut tx, &command).await?;
        let invalid_dependency:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM command_dependencies d JOIN commands p ON p.account_id=d.account_id AND p.command_id=d.predecessor_id LEFT JOIN command_supersessions s ON s.account_id=p.account_id AND s.replacement_id=p.command_id WHERE d.account_id=? AND d.command_id=? AND coalesce(s.execution_order,p.enqueue_order)>=?)").bind(&command.account_id).bind(&replacement.command_id).bind(execution_order).fetch_one(&mut *tx).await.map_err(|_| CollaborationError::storage())?;
        if invalid_dependency {
            return Err(CollaborationError::invalid(
                "Replacement dependency would reverse execution order",
            ));
        }
        sqlx::query("INSERT INTO command_supersessions(account_id,original_id,original_hash,replacement_id,replacement_hash,action_id,execution_order,depth) VALUES(?,?,?,?,?,?,?,?)")
            .bind(&command.account_id).bind(&command.command_id).bind(command.hash.as_slice()).bind(&replacement.command_id).bind(replacement.hash.as_slice()).bind(&request.action_id).bind(execution_order).bind((depth+1) as i64).execute(&mut *tx).await.map_err(|_| CollaborationError::storage())?;
        sqlx::query(
            "UPDATE command_user_controls SET paused=0 WHERE account_id=? AND command_id=?",
        )
        .bind(&command.account_id)
        .bind(&command.command_id)
        .execute(&mut *tx)
        .await
        .map_err(|_| CollaborationError::storage())?;
        // Admission replay used its new enqueue order; only after recording the
        // immutable edge can replay place it into the original execution slot.
        super::effective::refresh_target_in(&mut tx, &command.account_id, &command.target_id)
            .await?;
        let revision = metadata(&mut tx).await?.0;
        let receipt = CommandRecoveryReceipt {
            account_id: command.account_id.clone(),
            action_id: request.action_id.clone(),
            command_id: command.command_id.clone(),
            replacement_id: Some(replacement.command_id),
            state: "superseded".into(),
            paused: false,
            remote_may_have_happened: command.attempt_count > 0,
            revision,
        };
        record_action_in(
            &mut tx,
            &command,
            &request.action_id,
            &request_json,
            &receipt,
        )
        .await?;
        tx.commit()
            .await
            .map_err(|_| CollaborationError::storage())?;
        Ok(receipt)
    }

    pub(crate) async fn command_recovery_export(
        &self,
        context: CommandRecoveryContext,
        policy: Option<&dyn CommandRecoveryPolicy>,
    ) -> Result<CommandRecoveryExport> {
        validate_context(&context)?;
        let mut tx = self
            .inner
            .readers
            .begin()
            .await
            .map_err(|_| CollaborationError::storage())?;
        let (detail, command, _) = checked_in(&mut tx, &context, policy).await?;
        // Only authored command bytes and safe review fields. No vault refs,
        // raw provider response/evidence bodies, headers or general DB dump.
        let text=serde_json::to_string_pretty(&serde_json::json!({"format":"gitru.authored-command.v1","command":detail.command,"fields":detail.fields,"submission_hash":hex(&command.hash),"authored_payload_hex":hex(&command.payload),"original_authorization_epoch":command.authorization_epoch})).map_err(|_| CollaborationError::storage())?;
        tx.commit()
            .await
            .map_err(|_| CollaborationError::storage())?;
        Ok(CommandRecoveryExport {
            suggested_name: format!("gitru-command-{}.json", command.command_id),
            text,
        })
    }
}

async fn detail_in(
    tx: &mut Transaction<'_, Sqlite>,
    account_id: &str,
    id: &str,
    policy: Option<&dyn CommandRecoveryPolicy>,
) -> Result<(CommandRecoveryDetail, DeliveryCommand, NativeRecoveryReview)> {
    let command = super::delivery::load_in(tx, account_id, id).await?;
    let account = account_in(tx, account_id, false).await?;
    if let Some(policy) = policy
        && (policy.instance_id() != ProviderInstance::for_account(&account)?.id
            || policy.operation_kind() != command.operation_kind
            || policy.payload_version() != command.payload_version)
    {
        return Err(invalid());
    }
    let review = match policy {
        Some(policy) => policy.review_in(tx, &command, &account).await?,
        None => NativeRecoveryReview::unsupported(),
    };
    review.validate()?;
    let paused = paused_in(tx, account_id, id).await?;
    let replacement_id: Option<String> = sqlx::query_scalar(
        "SELECT replacement_id FROM command_supersessions WHERE account_id=? AND original_id=?",
    )
    .bind(account_id)
    .bind(id)
    .fetch_optional(&mut **tx)
    .await
    .map_err(|_| CollaborationError::storage())?;
    let (revision, view) = metadata(tx).await?;
    let token = review_token(&command, &account, &view, paused, &review)?;
    let pending = matches!(
        command.state,
        DeliveryState::Queued
            | DeliveryState::Sending
            | DeliveryState::RetryWait
            | DeliveryState::Accepted
            | DeliveryState::Unknown
            | DeliveryState::Conflict
    );
    let remote_possible = command.attempt_count > 0 || command.quarantine_generation > 0;
    let can_cancel = pending && !remote_possible;
    let can_pause = pending && remote_possible && !paused && policy.is_some();
    let can_retry = matches!(
        command.state,
        DeliveryState::Queued
            | DeliveryState::Sending
            | DeliveryState::RetryWait
            | DeliveryState::Accepted
            | DeliveryState::Unknown
    ) && paused
        && policy.is_some()
        && account.state == AccountState::Active
        && (command.authorization_epoch == account.authorization_epoch || command.reconcile_only())
        && command.attention.is_none()
        && (command.reconcile_only()
            || command.state == DeliveryState::Sending
            || command.attempt_count < MAX_ATTEMPTS)
        && command.reconciliation_count < MAX_RECONCILIATIONS;
    let can_replace = review.can_replace
        && policy.is_some()
        && !command.reconcile_only()
        && command.quarantine_generation == 0
        && account.state == AccountState::Active
        && command.authorization_epoch == account.authorization_epoch
        && matches!(
            command.state,
            DeliveryState::Queued
                | DeliveryState::RetryWait
                | DeliveryState::Conflict
                | DeliveryState::Rejected
        );
    let summary = CommandRecoverySummary {
        account_id: account_id.into(),
        command_id: id.into(),
        target_id: command.target_id.clone(),
        target_kind: command.target_kind.clone(),
        operation_kind: command.operation_kind.clone(),
        payload_version: command.payload_version,
        state: command.state.name().into(),
        admitted_at: command.admitted_at.clone(),
        attempt_count: command.attempt_count as u32,
        paused,
        quarantined: command.quarantine_generation > 0,
        attention: command.attention.clone(),
        replacement_id,
        blocked_reason: blocked_reason_in(tx, account_id, id).await?,
    };
    let context = CommandRecoveryContext {
        account_id: account_id.into(),
        command_id: id.into(),
        expected_generation: command.generation.to_string(),
        expected_epoch: account.authorization_epoch.clone(),
        authorization_view: view,
        review_token: token,
    };
    let detail = CommandRecoveryDetail {
        command: summary,
        context,
        fields: review.fields.clone(),
        can_retry,
        can_cancel,
        can_pause,
        can_replace,
        reason: review.reason.clone(),
        revision,
    };
    Ok((detail, command, review))
}
async fn checked_in(
    tx: &mut Transaction<'_, Sqlite>,
    context: &CommandRecoveryContext,
    policy: Option<&dyn CommandRecoveryPolicy>,
) -> Result<(CommandRecoveryDetail, DeliveryCommand, NativeRecoveryReview)> {
    validate_account_view_in(tx, context).await?;
    let value = detail_in(tx, &context.account_id, &context.command_id, policy).await?;
    if value.0.context != *context {
        return Err(stale());
    }
    Ok(value)
}
async fn validate_account_view_in(
    tx: &mut Transaction<'_, Sqlite>,
    context: &CommandRecoveryContext,
) -> Result<()> {
    let account = account_in(tx, &context.account_id, false).await?;
    if account.authorization_epoch != context.expected_epoch
        || metadata(tx).await?.1 != context.authorization_view
    {
        return Err(stale());
    }
    Ok(())
}
pub(super) async fn paused_in(
    tx: &mut Transaction<'_, Sqlite>,
    account: &str,
    id: &str,
) -> Result<bool> {
    Ok(sqlx::query_scalar::<_, bool>(
        "SELECT paused FROM command_user_controls WHERE account_id=? AND command_id=?",
    )
    .bind(account)
    .bind(id)
    .fetch_optional(&mut **tx)
    .await
    .map_err(|_| CollaborationError::storage())?
    .unwrap_or(false))
}
pub(super) async fn execution_order_in(
    tx: &mut Transaction<'_, Sqlite>,
    command: &DeliveryCommand,
) -> Result<i64> {
    Ok(sqlx::query_scalar::<_, i64>(
        "SELECT execution_order FROM command_supersessions WHERE account_id=? AND replacement_id=?",
    )
    .bind(&command.account_id)
    .bind(&command.command_id)
    .fetch_optional(&mut **tx)
    .await
    .map_err(|_| CollaborationError::storage())?
    .unwrap_or(command.enqueue_order))
}
async fn depth_in(tx: &mut Transaction<'_, Sqlite>, account: &str, id: &str) -> Result<usize> {
    Ok(sqlx::query_scalar::<_, i64>(
        "SELECT depth FROM command_supersessions WHERE account_id=? AND replacement_id=?",
    )
    .bind(account)
    .bind(id)
    .fetch_optional(&mut **tx)
    .await
    .map_err(|_| CollaborationError::storage())?
    .unwrap_or(0) as usize)
}
async fn blocked_reason_in(
    tx: &mut Transaction<'_, Sqlite>,
    account: &str,
    id: &str,
) -> Result<Option<String>> {
    let blocked:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM command_dependencies d JOIN commands p ON p.account_id=d.account_id AND p.command_id=d.predecessor_id WHERE d.account_id=? AND d.command_id=? AND p.state IN ('superseded','cancelled','rejected','conflict'))").bind(account).bind(id).fetch_one(&mut **tx).await.map_err(|_| CollaborationError::storage())?;
    Ok(blocked.then(|| "A predecessor changed; this saved intent requires its own review".into()))
}
fn review_token(
    command: &DeliveryCommand,
    account: &RemoteAccount,
    view: &str,
    paused: bool,
    review: &NativeRecoveryReview,
) -> Result<String> {
    let bytes = serde_json::to_vec(&(
        "gitru.command-review.v1",
        &command.hash,
        command.generation,
        &account.id,
        &account.authorization_epoch,
        &account.state,
        view,
        paused,
        command.quarantine_generation,
        review,
    ))
    .map_err(|_| CollaborationError::storage())?;
    Ok(hex(&Sha256::digest(bytes)))
}
fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write;
    let mut result = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        write!(&mut result, "{byte:02x}").expect("String write");
    }
    result
}
fn validate_context(context: &CommandRecoveryContext) -> Result<()> {
    validate_identifier(&context.account_id)?;
    validate_uuid(&context.command_id)?;
    positive_revision(&context.expected_generation)?;
    positive_revision(&context.expected_epoch)?;
    positive_revision(&context.authorization_view)?;
    if context.review_token.len() != 64
        || !context
            .review_token
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err(invalid());
    }
    Ok(())
}
fn validate_request(context: &CommandRecoveryContext, id: &str) -> Result<()> {
    validate_context(context)?;
    validate_uuid(id)
}
fn validate_uuid(id: &str) -> Result<()> {
    if Uuid::parse_str(id)
        .map(|u| u.hyphenated().to_string())
        .ok()
        .as_deref()
        != Some(id)
    {
        Err(invalid())
    } else {
        Ok(())
    }
}
fn validate_choices(
    choices: &[CommandFieldResolution],
    fields: &[CommandFieldReview],
) -> Result<()> {
    for (index, choice) in choices.iter().enumerate() {
        let Some(field) = fields.iter().find(|f| f.field == choice.field) else {
            return Err(invalid());
        };
        if !field.editable
            || choices[..index].iter().any(|c| c.field == choice.field)
            || choice.choice != CommandResolutionChoice::Edited && choice.value.is_some()
            || choice
                .value
                .as_ref()
                .is_some_and(|s| s.len() > 65_536 || s.contains('\0'))
            || choice.choice == CommandResolutionChoice::KeepDesired && !field.desired.known
            || choice.choice == CommandResolutionChoice::UseRemote && !field.remote.known
        {
            return Err(invalid());
        }
    }
    if fields.iter().any(|f| {
        f.editable
            && matches!(
                f.comparison,
                CommandFieldComparison::Conflict
                    | CommandFieldComparison::Unknown
                    | CommandFieldComparison::GuardChanged
            )
            && !choices.iter().any(|c| c.field == f.field)
    }) {
        return Err(invalid());
    }
    Ok(())
}
async fn duplicate_in(
    tx: &mut Transaction<'_, Sqlite>,
    account: &str,
    id: &str,
    request: &str,
) -> Result<Option<CommandRecoveryReceipt>> {
    let row:Option<(String,String)>=sqlx::query_as("SELECT request_json,receipt_json FROM command_recovery_actions WHERE account_id=? AND action_id=?").bind(account).bind(id).fetch_optional(&mut **tx).await.map_err(|_| CollaborationError::storage())?;
    match row {
        Some((saved, receipt)) if saved == request => Ok(Some(decode(&receipt)?)),
        Some(_) => Err(CollaborationError::invalid(
            "Recovery action ID was already used for different intent",
        )),
        None => Ok(None),
    }
}
async fn record_action_in(
    tx: &mut Transaction<'_, Sqlite>,
    command: &DeliveryCommand,
    id: &str,
    request: &str,
    receipt: &CommandRecoveryReceipt,
) -> Result<()> {
    let (count,bytes):(i64,i64)=sqlx::query_as("SELECT count(*),coalesce(sum(octet_length(request_json)),0) FROM command_recovery_actions WHERE account_id=? AND command_id=?").bind(&command.account_id).bind(&command.command_id).fetch_one(&mut **tx).await.map_err(|_| CollaborationError::storage())?;
    if count >= MAX_ACTIONS || bytes + request.len() as i64 > 1_048_576 {
        return Err(CollaborationError::new(
            ErrorCode::Busy,
            "Recovery history limit reached; export this intent",
        ));
    }
    sqlx::query("INSERT INTO command_recovery_actions(account_id,command_id,action_id,ordinal,submission_hash,request_json,receipt_json) VALUES(?,?,?,?,?,?,?)").bind(&command.account_id).bind(&command.command_id).bind(id).bind(count).bind(command.hash.as_slice()).bind(request).bind(encode(receipt)?).execute(&mut **tx).await.map_err(|_| CollaborationError::storage())?;
    Ok(())
}
fn summary_from_row(row: &sqlx::sqlite::SqliteRow) -> Result<CommandRecoverySummary> {
    Ok(CommandRecoverySummary {
        account_id: row
            .try_get("account_id")
            .map_err(|_| CollaborationError::storage())?,
        command_id: row
            .try_get("command_id")
            .map_err(|_| CollaborationError::storage())?,
        target_id: row
            .try_get("target_id")
            .map_err(|_| CollaborationError::storage())?,
        target_kind: row
            .try_get("target_kind")
            .map_err(|_| CollaborationError::storage())?,
        operation_kind: row
            .try_get("operation_kind")
            .map_err(|_| CollaborationError::storage())?,
        payload_version: u32::try_from(
            row.try_get::<i64, _>("payload_version")
                .map_err(|_| CollaborationError::storage())?,
        )
        .map_err(|_| CollaborationError::storage())?,
        state: row
            .try_get("state")
            .map_err(|_| CollaborationError::storage())?,
        admitted_at: row
            .try_get("admitted_at")
            .map_err(|_| CollaborationError::storage())?,
        attempt_count: row
            .try_get::<i64, _>("attempt_count")
            .map_err(|_| CollaborationError::storage())? as u32,
        paused: row
            .try_get("paused")
            .map_err(|_| CollaborationError::storage())?,
        quarantined: row
            .try_get("quarantined")
            .map_err(|_| CollaborationError::storage())?,
        attention: row
            .try_get("attention")
            .map_err(|_| CollaborationError::storage())?,
        replacement_id: row
            .try_get("replacement_id")
            .map_err(|_| CollaborationError::storage())?,
        blocked_reason: None,
    })
}
fn invalid() -> CollaborationError {
    CollaborationError::invalid("Invalid command recovery request")
}
fn stale() -> CollaborationError {
    CollaborationError::new(
        ErrorCode::StaleView,
        "The command review changed; reload before resolving it",
    )
}

#[cfg(test)]
mod tests;
