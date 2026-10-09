use super::*;
use crate::workflow_state::native::{NativeFrame, Observation};
use crate::{CollaborationError, commands::*, delivery::*};

pub(crate) const OPERATION: &str = "github.guarded_merge";
pub(crate) const PROOF: &str = "github.guarded_merge_observation";
pub(crate) const GRANT_SECONDS: u64 = 60;
pub(crate) const MAX_GRANTS: usize = 32;
type Result<T> = std::result::Result<T, CollaborationError>;
pub(crate) fn invalid() -> CollaborationError {
    CollaborationError::invalid("Invalid bounded guarded merge request")
}
pub(crate) fn valid_oid(s: &str) -> bool {
    s.len() == 40
        && s.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
pub(crate) fn identifier(s: &str) -> bool {
    !s.is_empty() && s.len() <= 1024 && !s.chars().any(char::is_control)
}
pub(crate) fn canonical_uuid(s: &str) -> bool {
    uuid::Uuid::parse_str(s)
        .ok()
        .is_some_and(|u| u.hyphenated().to_string() == s)
}
pub(crate) fn revision(s: &str, positive: bool) -> bool {
    s.len() <= 19
        && s.parse::<u64>()
            .ok()
            .is_some_and(|n| n <= i64::MAX as u64 && (!positive || n > 0) && n.to_string() == s)
}
pub(crate) fn validate_query(q: &GuardedMergeQuery) -> Result<()> {
    if !identifier(&q.account_id) || !identifier(&q.subject_id) {
        return Err(invalid());
    }
    Ok(())
}
pub(crate) fn validate_request(r: &GuardedMergeRequest) -> Result<()> {
    validate_query(&GuardedMergeQuery {
        account_id: r.context.account_id.clone(),
        subject_id: r.context.subject_id.clone(),
    })?;
    if !canonical_uuid(&r.command_id)
        || !canonical_uuid(&r.context.grant_id)
        || !revision(&r.context.authorization_epoch, true)
        || !revision(&r.context.authorization_view, false)
        || !valid_oid(&r.context.expected_head)
        || !r.confirm_inspected_head
    {
        return Err(invalid());
    }
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Payload {
    pub request: GuardedMergeRequest,
    pub actor_id: String,
    pub repository_native_id: String,
    pub subject_native_id: String,
    pub number: String,
}
impl CommandPayloadCodec for Payload {
    const OPERATION_KIND: &'static str = OPERATION;
    const PAYLOAD_VERSION: u32 = 1;
    fn encode_payload(&self, fields: &mut CanonicalFields) -> Result<()> {
        validate_request(&self.request)?;
        fields.string(1, &String::from_utf8(encode(self)?).map_err(|_| invalid())?)
    }
}
pub(crate) fn decode_payload(c: &DeliveryCommand) -> Result<Payload> {
    decode_payload_bytes(
        &c.payload,
        &c.account_id,
        &c.command_id,
        &c.target_id,
        &c.authorization_epoch,
    )
}
pub(crate) fn decode_submission(c: &CommandSubmission) -> Result<Payload> {
    decode_payload_bytes(
        c.payload_bytes(),
        c.account_id(),
        c.command_id(),
        c.target().id(),
        c.authorization_epoch(),
    )
}
fn decode_payload_bytes(
    bytes: &[u8],
    account: &str,
    command: &str,
    subject: &str,
    epoch: &str,
) -> Result<Payload> {
    if bytes.len() < 6
        || bytes.len() > MAX_EVIDENCE_BYTES
        || u16::from_be_bytes(bytes[..2].try_into().map_err(|_| invalid())?) != 1
        || u32::from_be_bytes(bytes[2..6].try_into().map_err(|_| invalid())?) as usize
            != bytes.len() - 6
    {
        return Err(invalid());
    }
    let p: Payload = decode(&bytes[6..])?;
    validate_request(&p.request)?;
    if p.request.context.account_id != account
        || p.request.command_id != command
        || p.request.context.subject_id != subject
        || p.request.context.authorization_epoch != epoch
        || !identifier(&p.actor_id)
        || !native_id(&p.repository_native_id)
        || !native_id(&p.subject_native_id)
        || !native_id(&p.number)
    {
        return Err(invalid());
    }
    Ok(p)
}
pub(crate) fn native_id(s: &str) -> bool {
    s.parse::<u64>()
        .ok()
        .is_some_and(|n| n > 0 && n.to_string() == s)
}
pub(crate) fn encode<T: Serialize>(v: &T) -> Result<Vec<u8>> {
    let bytes = serde_json::to_vec(v).map_err(|_| invalid())?;
    if bytes.len() > MAX_EVIDENCE_BYTES {
        return Err(invalid());
    }
    Ok(bytes)
}
pub(crate) fn decode<T: serde::de::DeserializeOwned + Serialize>(bytes: &[u8]) -> Result<T> {
    if bytes.len() > MAX_EVIDENCE_BYTES {
        return Err(invalid());
    }
    let v: T = serde_json::from_slice(bytes).map_err(|_| invalid())?;
    if encode(&v)? != bytes {
        return Err(invalid());
    }
    Ok(v)
}
pub(crate) fn command_hash(c: &DeliveryCommand) -> String {
    use std::fmt::Write;
    c.hash.iter().fold(String::with_capacity(64), |mut s, b| {
        let _ = write!(&mut s, "{b:02x}");
        s
    })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Fresh {
    pub observation: Observation,
    pub merged: bool,
    pub merge_oid: Option<String>,
    pub merged_at: Option<String>,
    pub can_push: Option<bool>,
    pub methods: Vec<MergeMethod>,
    pub draft: Option<bool>,
    pub mergeable: Option<bool>,
    pub mergeability: Option<String>,
    pub auto_merge: bool,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Origin {
    Preflight,
    MutationResponse,
    Reconciliation,
    LocalGate,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum ResultKind {
    Ready,
    Merged { merge_oid: String },
    Accepted,
    Declined { reason: MergeUnavailableReason },
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Evidence {
    pub frame: NativeFrame,
    pub account_id: String,
    pub actor_id: String,
    pub authorization_epoch: String,
    pub command_hash: String,
    pub origin: Origin,
    pub fresh: Option<Fresh>,
    pub result: ResultKind,
    pub http_status: Option<u16>,
}
