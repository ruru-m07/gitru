use super::*;
use crate::{CollaborationError, DetailValue, RemoteItem, RemoteRepository, commands::*};
pub(crate) const MAX_BODY: usize = 16_384;
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct WorkflowBase {
    pub state: String,
    pub updated_at: String,
    pub head: Option<String>,
    pub source: String,
    pub repository_native_id: String,
    pub subject_native_id: String,
    pub number: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct NativeFrame {
    pub repository: RemoteRepository,
    #[serde(with = "crate::stored_item_v1")]
    pub subject: RemoteItem,
    pub base: WorkflowBase,
    pub authorization_view: String,
    pub body_revision: String,
    pub run_id: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Observation {
    pub title: Option<String>,
    pub body: DetailValue,
    pub state: String,
    pub head: Option<crate::DetailBranch>,
    pub provider_updated_at: String,
    pub observed_at: String,
}
use serde::{Deserialize, Serialize};
pub(crate) const OPERATION: &str = "github.workflow_state";
type Result<T> = std::result::Result<T, CollaborationError>;
pub(crate) fn invalid() -> CollaborationError {
    CollaborationError::invalid("Invalid bounded workflow intent")
}
#[derive(Debug, Clone)]
pub(crate) struct Payload {
    pub request: WorkflowStateRequest,
    pub base: WorkflowBase,
}
impl CommandPayloadCodec for Payload {
    const OPERATION_KIND: &'static str = OPERATION;
    const PAYLOAD_VERSION: u32 = 1;
    fn encode_payload(&self, f: &mut CanonicalFields) -> Result<()> {
        validate_request(&self.request)?;
        f.string(1, self.request.desired_state.as_str())?;
        f.string(
            2,
            &serde_json::to_string(&self.base).map_err(|_| invalid())?,
        )?;
        f.string(3, &self.request.context.authorization_view)?;
        f.string(4, &self.request.context.review_token)?;
        f.bool(5, self.request.accept_best_effort)
    }
}
pub(crate) fn validate_request(r: &WorkflowStateRequest) -> Result<()> {
    let identifier = |v: &str| !v.is_empty() && v.len() <= 1024 && !v.contains('\0');
    let revision = |v: &str, positive: bool| {
        v.len() <= 19
            && v.parse::<u64>()
                .ok()
                .is_some_and(|n| n <= i64::MAX as u64 && (!positive || n > 0) && n.to_string() == v)
    };
    if !identifier(&r.context.account_id)
        || !identifier(&r.context.subject_id)
        || !revision(&r.context.authorization_epoch, true)
        || !revision(&r.context.authorization_view, false)
        || !r.accept_best_effort
        || r.context.review_token.len() != 64
        || !r
            .context
            .review_token
            .bytes()
            .all(|b| b.is_ascii_hexdigit())
        || uuid::Uuid::parse_str(&r.command_id)
            .ok()
            .is_none_or(|u| u.hyphenated().to_string() != r.command_id)
    {
        return Err(invalid());
    }
    Ok(())
}

pub(crate) fn decode_payload(c: &crate::delivery::DeliveryCommand) -> Result<Payload> {
    decode(
        &c.payload,
        &c.account_id,
        &c.command_id,
        &c.target_id,
        &c.authorization_epoch,
    )
}
pub(crate) fn decode_submission(c: &CommandSubmission) -> Result<Payload> {
    decode(
        c.payload_bytes(),
        c.account_id(),
        c.command_id(),
        c.target().id(),
        c.authorization_epoch(),
    )
}
fn decode(bytes: &[u8], account: &str, id: &str, subject: &str, epoch: &str) -> Result<Payload> {
    if bytes.len() > 65_536 {
        return Err(invalid());
    }
    let mut rest = bytes;
    let mut values = Vec::with_capacity(5);
    for tag in 1u16..=5 {
        if rest.len() < 6 || u16::from_be_bytes(rest[..2].try_into().map_err(|_| invalid())?) != tag
        {
            return Err(invalid());
        }
        let n = u32::from_be_bytes(rest[2..6].try_into().map_err(|_| invalid())?) as usize;
        if n > rest.len() - 6 {
            return Err(invalid());
        }
        values.push(&rest[6..6 + n]);
        rest = &rest[6 + n..];
    }
    if !rest.is_empty() || values[4] != [1] {
        return Err(invalid());
    }
    let s = |i: usize| std::str::from_utf8(values[i]).map_err(|_| invalid());
    let base: WorkflowBase = serde_json::from_slice(values[1]).map_err(|_| invalid())?;
    // Exact canonical representation disallows additional or duplicate JSON fields.
    if serde_json::to_vec(&base).map_err(|_| invalid())? != values[1] {
        return Err(invalid());
    }
    let p = Payload {
        request: WorkflowStateRequest {
            context: WorkflowStateContext {
                account_id: account.into(),
                subject_id: subject.into(),
                authorization_epoch: epoch.into(),
                authorization_view: s(2)?.into(),
                review_token: s(3)?.into(),
            },
            command_id: id.into(),
            desired_state: WorkflowState::parse(s(0)?).ok_or_else(invalid)?,
            accept_best_effort: true,
        },
        base,
    };
    validate_request(&p.request)?;
    Ok(p)
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Evidence {
    pub frame: NativeFrame,
    pub observation: Observation,
    pub account_id: String,
    pub actor_id: String,
    pub authorization_epoch: String,
    pub command_hash: String,
    pub origin: Origin,
}
pub(crate) fn desired_matches(p: &Payload, o: &Observation) -> bool {
    p.request.desired_state.as_str() == o.state
}
pub(crate) fn guards_match(p: &Payload, o: &Observation) -> bool {
    WorkflowState::parse(&o.state).is_some()
        && p.base.head.as_deref() == o.head.as_ref().map(|h| h.oid.as_str())
        && (p.base.state == o.state || desired_matches(p, o))
}
pub(crate) fn overlap(p: &Payload, o: &Observation) -> bool {
    !guards_match(p, o)
}

pub(crate) fn decode_bounded<T: serde::de::DeserializeOwned + serde::Serialize>(
    bytes: &[u8],
) -> Result<T> {
    if bytes.len() > crate::delivery::MAX_EVIDENCE_BYTES {
        return Err(invalid());
    }
    let value: T = serde_json::from_slice(bytes).map_err(|_| invalid())?;
    if encode_bounded(&value)? != bytes {
        return Err(invalid());
    }
    Ok(value)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum Origin {
    Preflight,
    MutationResponse,
    Reconciliation,
}
pub(crate) fn encode_bounded<T: Serialize>(value: &T) -> Result<Vec<u8>> {
    let bytes = serde_json::to_vec(value).map_err(|_| invalid())?;
    if bytes.len() > crate::delivery::MAX_EVIDENCE_BYTES {
        return Err(invalid());
    }
    Ok(bytes)
}
