use super::*;
use crate::{CollaborationError, RemoteItem, RemoteRepository, commands::*};
use serde::{Deserialize, Serialize};
pub(crate) const OPERATION: &str = "github.edit_text";
pub(crate) const MAX_BODY: usize = 16_384;
type Result<T> = std::result::Result<T, CollaborationError>;
pub(crate) fn invalid() -> CollaborationError {
    CollaborationError::invalid("Invalid bounded text edit")
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct TextBase {
    pub title: String,
    pub body: Option<String>,
    pub state: String,
    pub updated_at: String,
    pub head: Option<String>,
    pub source: String,
    pub repository_native_id: String,
    pub subject_native_id: String,
    pub number: String,
}
#[derive(Debug, Clone)]
pub(crate) struct Payload {
    pub request: TextEditRequest,
    pub base: TextBase,
}
impl CommandPayloadCodec for Payload {
    const OPERATION_KIND: &'static str = OPERATION;
    const PAYLOAD_VERSION: u32 = 1;
    fn encode_payload(&self, f: &mut CanonicalFields) -> Result<()> {
        validate_request(&self.request)?;
        f.bool(1, self.request.accept_best_effort)?;
        f.optional_string(2, self.request.title.as_deref())?;
        f.optional_string(3, self.request.body.as_deref())?;
        f.string(4, &self.base.title)?;
        f.optional_string(5, self.base.body.as_deref())?;
        f.string(6, &self.base.state)?;
        f.optional_string(7, self.base.head.as_deref())?;
        f.string(8, &self.base.source)?;
        f.string(9, &self.base.repository_native_id)?;
        f.string(10, &self.base.subject_native_id)?;
        f.string(11, &self.base.number)?;
        f.string(12, &self.request.context.authorization_view)?;
        f.string(13, &self.request.context.review_token)?;
        f.string(14, &self.base.updated_at)
    }
}
pub(crate) fn validate_request(r: &TextEditRequest) -> Result<()> {
    if !r.accept_best_effort
        || r.title.is_none() && r.body.is_none()
        || r.title.as_ref().is_some_and(|t| {
            t.trim().is_empty()
                || t.trim() != t
                || t.chars().any(char::is_control)
                || t.len() > 1024
                || t.chars().count() > 256
                || t.contains('\0')
        })
        || r.body
            .as_ref()
            .is_some_and(|t| t.len() > MAX_BODY || t.contains('\0'))
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
pub(crate) fn decode_payload(command: &crate::delivery::DeliveryCommand) -> Result<Payload> {
    decode_bytes(
        &command.payload,
        &command.account_id,
        &command.command_id,
        &command.target_id,
        &command.authorization_epoch,
    )
}
pub(crate) fn decode_submission(command: &CommandSubmission) -> Result<Payload> {
    decode_bytes(
        command.payload_bytes(),
        command.account_id(),
        command.command_id(),
        command.target().id(),
        command.authorization_epoch(),
    )
}
fn decode_bytes(
    bytes: &[u8],
    account: &str,
    command: &str,
    subject: &str,
    epoch: &str,
) -> Result<Payload> {
    if bytes.len() > 65_536 {
        return Err(invalid());
    }
    let mut rest = bytes;
    let mut values = Vec::with_capacity(14);
    for tag in 1u16..=14 {
        if rest.len() < 6 || u16::from_be_bytes(rest[..2].try_into().map_err(|_| invalid())?) != tag
        {
            return Err(invalid());
        }
        let len = u32::from_be_bytes(rest[2..6].try_into().map_err(|_| invalid())?) as usize;
        if len > rest.len() - 6 {
            return Err(invalid());
        }
        values.push(&rest[6..6 + len]);
        rest = &rest[6 + len..];
    }
    if !rest.is_empty() || values[0] != [1] {
        return Err(invalid());
    }
    let text = |index: usize| String::from_utf8(values[index].to_vec()).map_err(|_| invalid());
    let optional = |index: usize| -> Result<Option<String>> {
        match values[index] {
            [0] => Ok(None),
            [1, tail @ ..] => Ok(Some(
                String::from_utf8(tail.to_vec()).map_err(|_| invalid())?,
            )),
            _ => Err(invalid()),
        }
    };
    let payload = Payload {
        request: TextEditRequest {
            context: TextEditContext {
                account_id: account.into(),
                subject_id: subject.into(),
                authorization_epoch: epoch.into(),
                authorization_view: text(11)?,
                review_token: text(12)?,
            },
            command_id: command.into(),
            accept_best_effort: true,
            title: optional(1)?,
            body: optional(2)?,
        },
        base: TextBase {
            title: text(3)?,
            body: optional(4)?,
            state: text(5)?,
            updated_at: text(13)?,
            head: optional(6)?,
            source: text(7)?,
            repository_native_id: text(8)?,
            subject_native_id: text(9)?,
            number: text(10)?,
        },
    };
    validate_request(&payload.request)?;
    Ok(payload)
}
/// Request context carries cached canonical facts, never credential references.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct NativeFrame {
    pub repository: RemoteRepository,
    pub subject: RemoteItem,
    pub base: TextBase,
    pub authorization_view: String,
    pub body_revision: String,
    pub run_id: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct Observation {
    pub title: String,
    pub body: Option<String>,
    pub state: String,
    pub head: Option<crate::DetailBranch>,
    pub provider_updated_at: String,
    pub observed_at: String,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum Origin {
    Preflight,
    MutationResponse,
    Reconciliation,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct Evidence {
    pub frame: NativeFrame,
    pub observation: Observation,
    pub account_id: String,
    pub actor_id: String,
    pub authorization_epoch: String,
    pub command_hash: String,
    pub origin: Origin,
}
pub(crate) fn encode_bounded<T: Serialize>(value: &T) -> Result<Vec<u8>> {
    let bytes = serde_json::to_vec(value).map_err(|_| invalid())?;
    if bytes.len() > crate::delivery::MAX_EVIDENCE_BYTES {
        return Err(invalid());
    }
    Ok(bytes)
}
pub(crate) fn decode_bounded<T: serde::de::DeserializeOwned>(bytes: &[u8]) -> Result<T> {
    if bytes.len() > crate::delivery::MAX_EVIDENCE_BYTES {
        return Err(invalid());
    }
    serde_json::from_slice(bytes).map_err(|_| invalid())
}
pub(crate) fn body_equal(a: Option<&str>, b: Option<&str>) -> bool {
    a.unwrap_or("") == b.unwrap_or("")
}
pub(crate) fn desired_matches(payload: &Payload, o: &Observation) -> bool {
    payload.request.title.as_ref().is_none_or(|t| t == &o.title)
        && payload
            .request
            .body
            .as_deref()
            .is_none_or(|b| body_equal(Some(b), o.body.as_deref()))
}
pub(crate) fn guards_match(payload: &Payload, o: &Observation) -> bool {
    payload.base.state == o.state
        && payload.base.head.as_deref() == o.head.as_ref().map(|h| h.oid.as_str())
}
pub(crate) fn overlap(payload: &Payload, o: &Observation) -> bool {
    !guards_match(payload, o)
        || payload
            .request
            .title
            .as_ref()
            .is_some_and(|t| o.title != payload.base.title && &o.title != t)
        || payload.request.body.as_deref().is_some_and(|t| {
            !body_equal(o.body.as_deref(), payload.base.body.as_deref())
                && !body_equal(o.body.as_deref(), Some(t))
        })
}
