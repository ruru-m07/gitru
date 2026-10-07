//! Transactional admission of authored intent. No delivery or credential work.
#![cfg_attr(
    not(test),
    allow(dead_code, reason = "typed operations land in their owning issues")
)]

use super::*;
use crate::DetailFacet;
use crate::commands::{COMMAND_ENVELOPE_VERSION, CommandSubmission, encode_guards};

const MAX_PROTECTIONS: usize = 128;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum CommandAdmissionError {
    Local(CollaborationError),
    IdempotencyConflict,
    MissingPredecessor,
}

impl From<CollaborationError> for CommandAdmissionError {
    fn from(value: CollaborationError) -> Self {
        Self::Local(value)
    }
}

type AdmissionResult<T> = std::result::Result<T, CommandAdmissionError>;

/// This is an immutable *admission* receipt. Delivery state is a separate read
/// model in RURU-115, and cannot rewrite the original admitted revision/order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CommandReceipt {
    pub account_id: String,
    pub command_id: String,
    pub submission_hash: [u8; 32],
    pub enqueue_order: u64,
    pub admitted_revision: String,
    pub admitted_at: String,
    pub duplicate: bool,
}

/// Account scope is taken from the sealed command, never a reference argument.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum CommandProtection {
    Entity(String),
    Facet {
        subject_id: String,
        facet: DetailFacet,
    },
    Blob(String),
}

impl CommandProtection {
    fn columns(&self) -> (&'static str, &str, &str) {
        match self {
            Self::Entity(id) => ("entity", id, ""),
            Self::Facet { subject_id, facet } => ("facet", subject_id, facet.name()),
            Self::Blob(id) => ("blob", id, ""),
        }
    }
}

/// Implemented by reviewed native operation modules. The hook performs only
/// local checks against the same writer snapshot as admission. Kind/version
/// agreement fails closed before the hook can run. No generic renderer API
/// constructs this policy or its submission.
#[async_trait::async_trait]
pub(crate) trait CommandAdmissionPolicy: Send + Sync {
    const OPERATION_KIND: &'static str;
    const PAYLOAD_VERSION: u32;

    /// Deterministic native effect decoded from immutable submitted intent.
    fn effect(
        &self,
        _submission: &CommandSubmission,
    ) -> Result<Option<crate::effective::ItemIntentPatch>> {
        Ok(None)
    }

    async fn validate(
        &self,
        tx: &mut Transaction<'_, Sqlite>,
        account: &RemoteAccount,
        submission: &CommandSubmission,
    ) -> Result<Vec<CommandProtection>>;
}

impl Store {
    pub(crate) async fn admit_command<P: CommandAdmissionPolicy>(
        &self,
        submission: &CommandSubmission,
        policy: &P,
    ) -> AdmissionResult<CommandReceipt> {
        let mut writer = self.inner.writer.acquire().await?;
        let mut tx = writer.begin().await.map_err(storage_error)?;
        let receipt = admit_in(&mut tx, submission, policy).await?;
        #[cfg(test)]
        tests::crash_checkpoint("before_commit");
        tx.commit().await.map_err(storage_error)?;
        #[cfg(test)]
        tests::crash_checkpoint("after_commit");
        Ok(receipt)
    }

    /// Authored receipts remain available for recovery after disconnection.
    pub(crate) async fn command_receipt(
        &self,
        account_id: &str,
        command_id: &str,
    ) -> Result<Option<CommandReceipt>> {
        validate_identifier(account_id)?;
        validate_identifier(command_id)?;
        let mut tx = self.inner.readers.begin().await.map_err(storage_error)?;
        account_in(&mut tx, account_id, false).await?;
        let row = sqlx::query("SELECT account_id,command_id,submission_hash,enqueue_order,admitted_revision,admitted_at FROM commands WHERE account_id=? AND command_id=?")
            .bind(account_id)
            .bind(command_id)
            .fetch_optional(&mut *tx)
            .await
            .map_err(storage_error)?;
        let receipt = row
            .as_ref()
            .map(|row| receipt_from_row(row, false))
            .transpose()?;
        tx.commit().await.map_err(storage_error)?;
        Ok(receipt)
    }
}

/// Shared exact admission protocol for a native recovery transaction. The caller
/// owns commit; a supersession edge and the new receipt therefore cannot split.
pub(crate) async fn admit_in<P: CommandAdmissionPolicy>(
    tx: &mut Transaction<'_, Sqlite>,
    submission: &CommandSubmission,
    policy: &P,
) -> AdmissionResult<CommandReceipt> {
    if submission.operation().kind() != P::OPERATION_KIND
        || submission.operation().payload_version() != P::PAYLOAD_VERSION
    {
        return Err(CollaborationError::new(
            ErrorCode::Unsupported,
            "Unsupported collaboration command version",
        )
        .into());
    }
    let guards = encode_guards(submission.guards())?;
    epoch_in(
        tx,
        submission.account_id(),
        submission.authorization_epoch(),
    )
    .await?;
    if let Some(row) = sqlx::query("SELECT * FROM commands WHERE account_id=? AND command_id=?")
        .bind(submission.account_id())
        .bind(submission.command_id())
        .fetch_optional(&mut **tx)
        .await
        .map_err(storage_error)?
    {
        let dependencies: Vec<String> = sqlx::query_scalar("SELECT predecessor_id FROM command_dependencies WHERE account_id=? AND command_id=? ORDER BY ordinal")
            .bind(submission.account_id()).bind(submission.command_id())
            .fetch_all(&mut **tx).await.map_err(storage_error)?;
        if !exact_stored_submission(&row, submission, &guards, &dependencies) {
            return Err(CommandAdmissionError::IdempotencyConflict);
        }
        // Do not rerun mutable base policy for an already committed intent.
        // A lost response remains recoverable after the cache changes.
        let receipt = receipt_from_row(&row, true)?;
        return Ok(receipt);
    }
    let account = account_in(tx, submission.account_id(), true).await?;
    let protections = policy.validate(tx, &account, submission).await?;
    if protections.len() > MAX_PROTECTIONS {
        return Err(CollaborationError::invalid("Too many command protections").into());
    }
    let mut protections = protections;
    protections.push(CommandProtection::Entity(submission.target().id().into()));
    if let Some(repository) = submission.target().repository_id() {
        protections.push(CommandProtection::Entity(repository.into()));
    }
    protections.sort_by(|left, right| left.columns().cmp(&right.columns()));
    protections.dedup();
    if protections.len() > MAX_PROTECTIONS {
        return Err(CollaborationError::invalid("Too many command protections").into());
    }
    for protection in &protections {
        let (_, id, _) = protection.columns();
        validate_identifier(id)?;
    }
    let mut dependencies = Vec::with_capacity(submission.dependencies().len());
    for id in submission.dependencies() {
        let hash: Option<Vec<u8>> = sqlx::query_scalar(
            "SELECT submission_hash FROM commands WHERE account_id=? AND command_id=?",
        )
        .bind(submission.account_id())
        .bind(id)
        .fetch_optional(&mut **tx)
        .await
        .map_err(storage_error)?;
        dependencies.push((id, hash.ok_or(CommandAdmissionError::MissingPredecessor)?));
    }
    let last_order: i64 = sqlx::query_scalar(
        "SELECT coalesce(max(enqueue_order),0) FROM commands WHERE account_id=?",
    )
    .bind(submission.account_id())
    .fetch_one(&mut **tx)
    .await
    .map_err(storage_error)?;
    let order = last_order
        .checked_add(1)
        .ok_or_else(CollaborationError::storage)?;
    let revision = record_change(
        tx,
        submission.account_id(),
        positive_revision(submission.authorization_epoch())?,
        "commands",
        false,
    )
    .await?;
    let admitted_at = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
    sqlx::query("INSERT INTO commands(account_id,command_id,authorization_epoch,envelope_version,operation_kind,payload_version,target_kind,target_id,repository_id,canonical_envelope,payload_bytes,guard_bytes,submission_hash,enqueue_order,admitted_revision,admitted_at,state) VALUES(?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,'queued')")
        .bind(submission.account_id()).bind(submission.command_id())
        .bind(positive_revision(submission.authorization_epoch())?).bind(i64::from(COMMAND_ENVELOPE_VERSION))
        .bind(submission.operation().kind()).bind(i64::from(submission.operation().payload_version()))
        .bind(submission.target().kind().storage_name()).bind(submission.target().id()).bind(submission.target().repository_id())
        .bind(submission.canonical_envelope()).bind(submission.payload_bytes()).bind(&guards).bind(submission.submission_hash().as_slice())
        .bind(order).bind(positive_revision(&revision)?).bind(&admitted_at)
        .execute(&mut **tx).await.map_err(storage_error)?;
    for (ordinal, (id, hash)) in dependencies.into_iter().enumerate() {
        sqlx::query("INSERT INTO command_dependencies(account_id,command_id,ordinal,predecessor_id,predecessor_hash) VALUES(?,?,?,?,?)")
            .bind(submission.account_id()).bind(submission.command_id()).bind(ordinal as i64).bind(id).bind(hash)
            .execute(&mut **tx).await.map_err(storage_error)?;
    }
    for protection in protections {
        let (kind, id, facet) = protection.columns();
        sqlx::query("INSERT INTO command_target_protections(account_id,command_id,reference_kind,reference_id,facet) VALUES(?,?,?,?,?)")
            .bind(submission.account_id()).bind(submission.command_id()).bind(kind).bind(id).bind(facet)
            .execute(&mut **tx).await.map_err(storage_error)?;
    }
    super::effective::admit_in(tx, submission, policy.effect(submission)?).await?;
    Ok(CommandReceipt {
        account_id: submission.account_id().into(),
        command_id: submission.command_id().into(),
        submission_hash: *submission.submission_hash(),
        enqueue_order: order as u64,
        admitted_revision: revision,
        admitted_at,
        duplicate: false,
    })
}

fn receipt_from_row(row: &sqlx::sqlite::SqliteRow, duplicate: bool) -> Result<CommandReceipt> {
    let hash: Vec<u8> = row.try_get("submission_hash").map_err(storage_error)?;
    let order: i64 = row.try_get("enqueue_order").map_err(storage_error)?;
    let revision: i64 = row.try_get("admitted_revision").map_err(storage_error)?;
    if order <= 0 || revision <= 0 {
        return Err(CollaborationError::storage());
    }
    Ok(CommandReceipt {
        account_id: row.try_get("account_id").map_err(storage_error)?,
        command_id: row.try_get("command_id").map_err(storage_error)?,
        submission_hash: hash.try_into().map_err(|_| CollaborationError::storage())?,
        enqueue_order: order as u64,
        admitted_revision: revision.to_string(),
        admitted_at: row.try_get("admitted_at").map_err(storage_error)?,
        duplicate,
    })
}

fn exact_stored_submission(
    row: &sqlx::sqlite::SqliteRow,
    submission: &CommandSubmission,
    guards: &[u8],
    dependencies: &[String],
) -> bool {
    row.get::<String, _>("account_id") == submission.account_id()
        && row.get::<String, _>("command_id") == submission.command_id()
        && row.get::<i64, _>("authorization_epoch").to_string() == submission.authorization_epoch()
        && row.get::<i64, _>("envelope_version") == i64::from(COMMAND_ENVELOPE_VERSION)
        && row.get::<String, _>("operation_kind") == submission.operation().kind()
        && row.get::<i64, _>("payload_version")
            == i64::from(submission.operation().payload_version())
        && row.get::<String, _>("target_kind") == submission.target().kind().storage_name()
        && row.get::<String, _>("target_id") == submission.target().id()
        && row.get::<Option<String>, _>("repository_id").as_deref()
            == submission.target().repository_id()
        && row.get::<Vec<u8>, _>("canonical_envelope") == submission.canonical_envelope()
        && row.get::<Vec<u8>, _>("payload_bytes") == submission.payload_bytes()
        && row.get::<Vec<u8>, _>("guard_bytes") == guards
        && row.get::<Vec<u8>, _>("submission_hash") == submission.submission_hash()
        && dependencies == submission.dependencies()
}

#[cfg(test)]
mod tests;
