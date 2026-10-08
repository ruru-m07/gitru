use super::*;
use crate::{CollaborationError, RemoteItem, RemoteRepository, commands::*};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
pub(crate) const OPERATION: &str = "github.create_comment";
pub(crate) const MAX_BODY: usize = 16384;
type Result<T> = std::result::Result<T, CollaborationError>;
pub(crate) fn invalid() -> CollaborationError {
    CollaborationError::invalid("Invalid bounded comment submission")
}
pub(crate) fn identifier(s: &str) -> Result<()> {
    if s.is_empty() || s.len() > 1024 || s.contains('\0') {
        Err(invalid())
    } else {
        Ok(())
    }
}
pub(crate) fn revision(s: &str, positive: bool) -> Result<i64> {
    let n = s.parse::<i64>().map_err(|_| invalid())?;
    if s.len() > 19 || n < 0 || positive && n == 0 || n.to_string() != s {
        return Err(invalid());
    }
    Ok(n)
}
pub(crate) fn validate_send(r: &SendCommentRequest) -> Result<()> {
    identifier(&r.context.account_id)?;
    identifier(&r.context.subject_id)?;
    revision(&r.context.authorization_epoch, true)?;
    revision(&r.context.authorization_view, false)?;
    revision(&r.draft_generation, true)?;
    if !r.accept_background_delivery
        || uuid::Uuid::parse_str(&r.command_id)
            .ok()
            .is_none_or(|v| v.hyphenated().to_string() != r.command_id)
        || r.context.review_token.len() != 64
        || !r
            .context
            .review_token
            .bytes()
            .all(|b| b.is_ascii_hexdigit())
    {
        return Err(invalid());
    }
    Ok(())
}
pub(crate) fn validate_body(s: &str) -> Result<()> {
    if s.len() > MAX_BODY || s.contains('\0') {
        Err(invalid())
    } else {
        Ok(())
    }
}
#[derive(Debug, Clone)]
pub(crate) struct Payload {
    pub request: SendCommentRequest,
    pub body: String,
    pub repository_native: String,
    pub subject_native: String,
    pub number: String,
}
impl CommandPayloadCodec for Payload {
    const OPERATION_KIND: &'static str = OPERATION;
    const PAYLOAD_VERSION: u32 = 1;
    fn encode_payload(&self, f: &mut CanonicalFields) -> Result<()> {
        validate_send(&self.request)?;
        validate_body(&self.body)?;
        if self.body.trim().is_empty() {
            return Err(invalid());
        }
        f.string(1, &self.body)?;
        f.string(2, &self.request.draft_generation)?;
        f.string(3, &self.repository_native)?;
        f.string(4, &self.subject_native)?;
        f.string(5, &self.number)?;
        f.string(6, &self.request.context.authorization_view)?;
        f.string(7, &self.request.context.review_token)?;
        f.bool(8, true)
    }
}
pub(crate) fn decode(command: &crate::delivery::DeliveryCommand) -> Result<Payload> {
    decode_parts(
        &command.payload,
        &command.account_id,
        &command.command_id,
        &command.target_id,
        &command.authorization_epoch,
    )
}
pub(crate) fn decode_submission(command: &CommandSubmission) -> Result<Payload> {
    decode_parts(
        command.payload_bytes(),
        command.account_id(),
        command.command_id(),
        command.target().id(),
        command.authorization_epoch(),
    )
}
pub(crate) fn decode_parts(
    bytes: &[u8],
    account: &str,
    command: &str,
    subject: &str,
    epoch: &str,
) -> Result<Payload> {
    if bytes.len() > 65536 {
        return Err(invalid());
    }
    let mut rest = bytes;
    let mut fields = Vec::with_capacity(8);
    for tag in 1u16..=8 {
        if rest.len() < 6 || u16::from_be_bytes(rest[..2].try_into().map_err(|_| invalid())?) != tag
        {
            return Err(invalid());
        }
        let len = u32::from_be_bytes(rest[2..6].try_into().map_err(|_| invalid())?) as usize;
        if len > rest.len() - 6 {
            return Err(invalid());
        }
        fields.push(&rest[6..6 + len]);
        rest = &rest[6 + len..];
    }
    if !rest.is_empty() || fields[7] != [1] {
        return Err(invalid());
    }
    let text = |i: usize| String::from_utf8(fields[i].to_vec()).map_err(|_| invalid());
    let p = Payload {
        request: SendCommentRequest {
            context: CommentSendContext {
                account_id: account.into(),
                subject_id: subject.into(),
                authorization_epoch: epoch.into(),
                authorization_view: text(5)?,
                review_token: text(6)?,
            },
            draft_generation: text(1)?,
            command_id: command.into(),
            accept_background_delivery: true,
        },
        body: text(0)?,
        repository_native: text(2)?,
        subject_native: text(3)?,
        number: text(4)?,
    };
    validate_send(&p.request)?;
    validate_body(&p.body)?;
    if p.body.trim().is_empty() {
        return Err(invalid());
    }
    Ok(p)
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Frame {
    pub repository: RemoteRepository,
    #[serde(with = "crate::stored_item_v1")]
    pub subject: RemoteItem,
    pub authorization_view: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Preparation {
    pub frame: Frame,
    pub actor: String,
    pub epoch: String,
    pub command_hash: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ReceiptEvidence {
    pub preparation: Preparation,
    pub receipt: CreatedCommentReceipt,
}
pub(crate) fn encode<T: Serialize>(v: &T) -> Result<Vec<u8>> {
    let b = serde_json::to_vec(v).map_err(|_| invalid())?;
    if b.len() > 65536 {
        return Err(invalid());
    }
    Ok(b)
}
// New dispatch admission only: leave saved raw drafts and immutable v1 decoding
// compatible. The actual frame/body are counted after JSON escaping. The 24KiB
// reserve covers fixed wrappers, bounded actor/IDs, canonical URL and normalized
// clocks. No arbitrary provider metadata enters a conversation comment receipt.
pub(crate) fn validate_dispatch_budget(frame: &Frame, body: &str) -> Result<()> {
    let bytes = serde_json::to_vec(frame).map_err(|_| invalid())?.len()
        + serde_json::to_vec(body).map_err(|_| invalid())?.len()
        + 24 * 1024;
    if bytes > 65_536 {
        return Err(CollaborationError::invalid(
            "Saved draft exceeds the encoded creation receipt budget; shorten its body before sending",
        ));
    }
    Ok(())
}
pub(crate) fn decode_json<T: serde::de::DeserializeOwned + Serialize>(bytes: &[u8]) -> Result<T> {
    if bytes.len() > 65536 {
        return Err(invalid());
    }
    let value: T = serde_json::from_slice(bytes).map_err(|_| invalid())?;
    if serde_json::to_vec(&value).map_err(|_| invalid())? != bytes {
        return Err(invalid());
    }
    Ok(value)
}
pub(crate) fn body_hash(s: &str) -> Vec<u8> {
    Sha256::digest(s.as_bytes()).to_vec()
}
pub(crate) fn command_hash(c: &crate::delivery::DeliveryCommand) -> String {
    c.hash.iter().map(|b| format!("{b:02x}")).collect()
}

pub(crate) fn receipt_matches(e: &ReceiptEvidence, p: &Payload) -> bool {
    let r = &e.receipt;
    let f = &e.preparation.frame;
    r.command_id == p.request.command_id
        && r.draft_generation == p.request.draft_generation
        && r.body == p.body
        && f.subject.account_id == p.request.context.account_id
        && f.repository.account_id == p.request.context.account_id
        && f.subject.id == p.request.context.subject_id
        && f.subject.provider_id == p.subject_native
        && f.repository.provider_id == p.repository_native
        && f.subject.number.as_deref() == Some(p.number.as_str())
        && e.preparation.epoch == p.request.context.authorization_epoch
        && r.provider_id
            .parse::<u64>()
            .is_ok_and(|id| id > 0 && id.to_string() == r.provider_id)
        && !r.author.is_empty()
        && r.author.len() <= 256
        && !r.author.chars().any(char::is_control)
        && (r.url
            == format!(
                "https://github.com/{}/issues/{}#issuecomment-{}",
                f.repository.full_name, p.number, r.provider_id
            )
            || r.url
                == format!(
                    "https://github.com/{}/pull/{}#issuecomment-{}",
                    f.repository.full_name, p.number, r.provider_id
                ))
        && chrono::DateTime::parse_from_rfc3339(&r.created_at).is_ok()
        && chrono::DateTime::parse_from_rfc3339(&r.observed_at).is_ok()
}
