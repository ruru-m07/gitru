//! Native operation contracts. No renderer or production provider mutation codec
//! can bypass the durable claim/result protocol by supplying a generic payload.
#![cfg_attr(
    not(test),
    allow(dead_code, reason = "operation codecs land in downstream issues")
)]

use crate::{
    CollaborationError, RemoteAccount, credentials::SecretToken, providers::ProviderError,
};
use async_trait::async_trait;
use sqlx::{Sqlite, Transaction};

pub(crate) const MAX_ATTEMPTS: i64 = 8;
pub(crate) const MAX_RECONCILIATIONS: i64 = 8;
pub(crate) const MAX_EVIDENCE_BYTES: usize = 65_536;
pub(crate) const CALL_TIMEOUT_SECONDS: u64 = 30;

/// Wall time records provider-budget deadlines; command deadlines use one
/// process UTC anchor advanced only by monotonic elapsed time.
pub(crate) struct DeliveryTime {
    pub now: String,
    pub command_now: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DeliveryState {
    Queued,
    Sending,
    RetryWait,
    Accepted,
    Confirmed,
    Unknown,
    Conflict,
    Rejected,
    Cancelled,
    Superseded,
}
impl DeliveryState {
    pub fn name(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Sending => "sending",
            Self::RetryWait => "retry_wait",
            Self::Accepted => "accepted",
            Self::Confirmed => "confirmed",
            Self::Unknown => "outcome_unknown",
            Self::Conflict => "conflict",
            Self::Rejected => "rejected",
            Self::Cancelled => "cancelled",
            Self::Superseded => "superseded",
        }
    }
    pub fn parse(value: &str) -> Result<Self, CollaborationError> {
        Ok(match value {
            "queued" => Self::Queued,
            "sending" => Self::Sending,
            "retry_wait" => Self::RetryWait,
            "accepted" => Self::Accepted,
            "confirmed" => Self::Confirmed,
            "outcome_unknown" => Self::Unknown,
            "conflict" => Self::Conflict,
            "rejected" => Self::Rejected,
            "cancelled" => Self::Cancelled,
            "superseded" => Self::Superseded,
            _ => return Err(CollaborationError::storage()),
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct OperationEvidence {
    pub kind: String,
    pub version: u32,
    pub payload: Vec<u8>,
}
impl OperationEvidence {
    pub fn bounded(&self) -> bool {
        !self.kind.is_empty()
            && self.kind.len() <= 128
            && self.version > 0
            && self.kind.as_bytes()[0].is_ascii_lowercase()
            && self.kind.bytes().all(|b| {
                b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'.' | b'_' | b'-')
            })
            && self.payload.len() <= MAX_EVIDENCE_BYTES
    }
    pub fn native(kind: &str) -> Self {
        Self {
            kind: kind.into(),
            version: 1,
            payload: vec![],
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct RecordedEvidence {
    pub ordinal: i64,
    pub attempt: Option<i64>,
    pub evidence: OperationEvidence,
}

#[derive(Clone, Debug)]
pub(crate) struct DeliveryCommand {
    pub account_id: String,
    pub command_id: String,
    pub authorization_epoch: String,
    pub operation_kind: String,
    pub payload_version: u32,
    pub target_kind: String,
    pub target_id: String,
    pub repository_id: Option<String>,
    pub canonical_envelope: Vec<u8>,
    pub payload: Vec<u8>,
    pub guards: Vec<u8>,
    pub hash: [u8; 32],
    pub enqueue_order: i64,
    pub admitted_at: String,
    pub state: DeliveryState,
    pub generation: i64,
    pub next_action_at: Option<String>,
    pub reconciliation_count: i64,
    pub attention: Option<String>,
    pub attempt_count: i64,
    pub quarantine_generation: i64,
    pub evidence: Vec<RecordedEvidence>,
}
impl DeliveryCommand {
    pub fn reconcile_only(&self) -> bool {
        self.quarantine_generation > 0
            || matches!(self.state, DeliveryState::Accepted | DeliveryState::Unknown)
    }
}

#[derive(Clone, Debug)]
pub(crate) struct DispatchRequest {
    pub command: DeliveryCommand,
    pub account: RemoteAccount,
    pub instance_id: String,
    pub attempt: i64,
    pub execution_base: Vec<u8>,
}

#[derive(Clone, Debug)]
pub(crate) struct ReconcileRequest {
    /// Bounded native snapshot captured in the claim transaction; never renderer input.
    pub native_context: Vec<u8>,
    pub command: DeliveryCommand,
    /// Current actor authorization; never replacement dispatch authority.
    pub account: RemoteAccount,
    pub instance_id: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum EvidencePurpose {
    Accepted,
    Confirmed,
    Rejected,
    Conflict,
    SafeRetry,
}

#[derive(Clone, Debug)]
pub(crate) enum DeliveryOutcome {
    Confirmed(OperationEvidence),
    Accepted(OperationEvidence),
    Rejected(OperationEvidence),
    Conflict(OperationEvidence),
    /// Independent proof of non-delivery or a documented operation idempotency
    /// mechanism; never inferred generically from an HTTP/transport error.
    SafeRetry(OperationEvidence),
    Unknown,
}
impl DeliveryOutcome {
    pub fn proof(&self) -> Option<(EvidencePurpose, &OperationEvidence)> {
        Some(match self {
            Self::Confirmed(e) => (EvidencePurpose::Confirmed, e),
            Self::Accepted(e) => (EvidencePurpose::Accepted, e),
            Self::Rejected(e) => (EvidencePurpose::Rejected, e),
            Self::Conflict(e) => (EvidencePurpose::Conflict, e),
            Self::SafeRetry(e) => (EvidencePurpose::SafeRetry, e),
            Self::Unknown => return None,
        })
    }
}

#[derive(Clone, Debug)]
pub(crate) struct DeliveryReport {
    pub outcome: DeliveryOutcome,
    pub retry_after_seconds: Option<u64>,
    pub account_cooldown_seconds: Option<u64>,
    /// Credential/quota observations are separate from proof of the operation
    /// outcome: for example, a 401 must stop account dispatch without implying
    /// that an ambiguous remote create was safely rejected.
    pub provider_error: Option<ProviderError>,
}
impl DeliveryReport {
    pub fn unknown() -> Self {
        Self {
            outcome: DeliveryOutcome::Unknown,
            retry_after_seconds: None,
            account_cooldown_seconds: None,
            provider_error: None,
        }
    }
}

pub(crate) struct DeliveryPreparation {
    pub bytes: Vec<u8>,
    /// Successful prerequisite reads can exhaust the same quota as mutations.
    pub account_cooldown_seconds: Option<u64>,
}

/// One bounded read per owned delivery turn. Continue never records an attempt.
pub(crate) enum PreparationStep {
    Continue(DeliveryPreparation),
    Complete(DeliveryPreparation),
}
pub(crate) enum ClaimDecision {
    Ready(Vec<u8>),
    Conflict(OperationEvidence),
    /// A fresh authenticated canonical observation already proves convergence.
    Confirmed(OperationEvidence),
}

/// Compiled native policies are explicitly registered per exact installation.
/// HTTP callbacks must not spawn unowned mutation tasks or retry internally.
/// Evidence validation must verify endpoint-specific provenance/receipts; body
/// equality, an arbitrary 2xx, or a partial-list absence is insufficient.
#[async_trait]
pub(crate) trait CommandDeliveryPolicy: Send + Sync {
    fn operation_kind(&self) -> &'static str;
    fn payload_version(&self) -> u32;
    async fn prepare_context_in(
        &self,
        _tx: &mut Transaction<'_, Sqlite>,
        _command: &DeliveryCommand,
        _account: &RemoteAccount,
    ) -> Result<Vec<u8>, CollaborationError> {
        Ok(vec![])
    }
    async fn prepare(
        &self,
        _token: &SecretToken,
        _request: &ReconcileRequest,
    ) -> Result<DeliveryPreparation, ProviderError> {
        Ok(DeliveryPreparation {
            bytes: vec![],
            account_cooldown_seconds: None,
        })
    }
    async fn prepare_step(
        &self,
        token: &SecretToken,
        request: &ReconcileRequest,
        continuation: Option<&[u8]>,
    ) -> Result<PreparationStep, ProviderError> {
        if continuation.is_some() {
            return Err(ProviderError::new(
                crate::providers::ProviderErrorKind::InvalidResponse,
            ));
        }
        self.prepare(token, request)
            .await
            .map(PreparationStep::Complete)
    }
    async fn validate_claim(
        &self,
        tx: &mut Transaction<'_, Sqlite>,
        command: &DeliveryCommand,
        account: &RemoteAccount,
        preparation: &[u8],
    ) -> Result<ClaimDecision, CollaborationError>;
    fn validate_evidence(
        &self,
        command: &DeliveryCommand,
        purpose: EvidencePurpose,
        evidence: &OperationEvidence,
    ) -> bool;
    /// Commit canonical provider state before a confirmed optimistic effect can
    /// retire. Only the scoped materializer can issue canonical field coverage;
    /// the default no-op cannot confirm a command with authored effects.
    async fn finalize_in(
        &self,
        _context: &mut crate::storage::effective::finalization::DeliveryFinalization<'_, '_>,
        _command: &DeliveryCommand,
        _purpose: EvidencePurpose,
        _evidence: &OperationEvidence,
    ) -> Result<(), CollaborationError> {
        Ok(())
    }
    async fn dispatch(&self, token: &SecretToken, request: DispatchRequest) -> DeliveryReport;
    /// Read-only reconciliation. Unknown preserves intent and schedules bounded
    /// later observation; there is no implicit resubmission.
    async fn reconcile(
        &self,
        _token: &SecretToken,
        _request: ReconcileRequest,
    ) -> Result<DeliveryReport, ProviderError> {
        Ok(DeliveryReport::unknown())
    }
}
