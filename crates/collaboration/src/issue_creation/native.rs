use super::*;
use crate::{CollaborationError, RemoteItem, RemoteRepository, commands::*};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
pub(crate) const OPERATION: &str = "github.create_issue";
pub(crate) const MAX_BODY: usize = 16384;
type Result<T> = std::result::Result<T, CollaborationError>;
pub(crate) fn invalid() -> CollaborationError {
    CollaborationError::invalid("Invalid bounded issue submission")
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
pub(crate) fn validate_send(r: &SubmitIssueRequest) -> Result<()> {
    identifier(&r.context.account_id)?;
    identifier(&r.context.repository_id)?;
    validate_uuid(&r.draft_id)?;
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
    pub request: SubmitIssueRequest,
    pub title: String,
    pub body: String,
    pub repository_native: String,
}
impl CommandPayloadCodec for Payload {
    const OPERATION_KIND: &'static str = OPERATION;
    const PAYLOAD_VERSION: u32 = 1;
    fn encode_payload(&self, f: &mut CanonicalFields) -> Result<()> {
        validate_send(&self.request)?;
        validate_title(&self.title, true)?;
        validate_body(&self.body)?;
        f.string(1, &self.title)?;
        f.string(2, &self.body)?;
        f.string(3, &self.request.draft_id)?;
        f.string(4, &self.request.draft_generation)?;
        f.string(5, &self.repository_native)?;
        f.string(6, &self.request.context.authorization_view)?;
        f.string(7, &self.request.context.review_token)?;
        f.bool(8, true)
    }
}
pub(crate) fn decode(c: &crate::delivery::DeliveryCommand) -> Result<Payload> {
    decode_parts(
        &c.payload,
        &c.account_id,
        &c.command_id,
        &c.target_id,
        &c.authorization_epoch,
    )
}
pub(crate) fn decode_submission(c: &CommandSubmission) -> Result<Payload> {
    decode_parts(
        c.payload_bytes(),
        c.account_id(),
        c.command_id(),
        c.target().id(),
        c.authorization_epoch(),
    )
}
pub(crate) fn decode_parts(
    bytes: &[u8],
    account: &str,
    command: &str,
    repository: &str,
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
        let n = u32::from_be_bytes(rest[2..6].try_into().map_err(|_| invalid())?) as usize;
        if n > rest.len() - 6 {
            return Err(invalid());
        }
        fields.push(&rest[6..6 + n]);
        rest = &rest[6 + n..];
    }
    if !rest.is_empty() || fields[7] != [1] {
        return Err(invalid());
    }
    let t = |i: usize| String::from_utf8(fields[i].to_vec()).map_err(|_| invalid());
    let p = Payload {
        request: SubmitIssueRequest {
            context: IssueDraftContext {
                account_id: account.into(),
                repository_id: repository.into(),
                authorization_epoch: epoch.into(),
                authorization_view: t(5)?,
                review_token: t(6)?,
            },
            draft_id: t(2)?,
            draft_generation: t(3)?,
            command_id: command.into(),
            accept_background_delivery: true,
        },
        title: t(0)?,
        body: t(1)?,
        repository_native: t(4)?,
    };
    validate_send(&p.request)?;
    validate_title(&p.title, true)?;
    validate_body(&p.body)?;
    Ok(p)
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Frame {
    pub repository: RemoteRepository,
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
    pub receipt: CreatedReceipt,
}
pub(crate) fn encode<T: Serialize>(v: &T) -> Result<Vec<u8>> {
    let b = serde_json::to_vec(v).map_err(|_| invalid())?;
    if b.len() > 65536 {
        return Err(invalid());
    }
    Ok(b)
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
pub(crate) fn command_hash(c: &crate::delivery::DeliveryCommand) -> String {
    c.hash.iter().map(|b| format!("{b:02x}")).collect()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CreatedReceipt {
    pub item: RemoteItem,
    pub metadata: ReceiptMetadata,
    pub created_at: String,
}
pub(crate) fn validate_uuid(s: &str) -> Result<()> {
    if uuid::Uuid::parse_str(s)
        .ok()
        .is_none_or(|u| u.hyphenated().to_string() != s)
    {
        Err(invalid())
    } else {
        Ok(())
    }
}
pub(crate) fn validate_title(s: &str, required: bool) -> Result<()> {
    if s.len() > 1024
        || s.chars().count() > 256
        || s.chars().any(char::is_control)
        || s.trim() != s
        || required && s.is_empty()
    {
        Err(invalid())
    } else {
        Ok(())
    }
}
pub(crate) fn content_hash(title: &str, body: &str) -> Vec<u8> {
    let mut h = Sha256::new();
    h.update((title.len() as u64).to_be_bytes());
    h.update(title);
    h.update(body);
    h.finalize().to_vec()
}
pub(crate) fn receipt_matches(e: &ReceiptEvidence, p: &Payload) -> bool {
    let r = &e.receipt;
    let i = &r.item;
    let f = &e.preparation.frame;
    let clocks = chrono::DateTime::parse_from_rfc3339(&r.created_at)
        .ok()
        .zip(chrono::DateTime::parse_from_rfc3339(&i.updated_at).ok());
    crate::storage::validate_resource_metadata(&r.metadata.observation()).is_ok()
        && r.metadata.fields.len() == crate::MetadataField::COMMON.len()
        && [
            crate::MetadataField::Title,
            crate::MetadataField::State,
            crate::MetadataField::Author,
            crate::MetadataField::WebUrl,
            crate::MetadataField::UpdatedAt,
        ]
        .iter()
        .all(|field| {
            r.metadata
                .fields
                .contains(&(*field, crate::DetailValueState::Known))
        })
        && clocks.is_some_and(|(created, updated)| updated >= created)
        && r.metadata.values.author.as_ref().is_some_and(|author| {
            author.provider_id == e.preparation.actor
                && Some(author.login.as_str()) == i.author.as_deref()
        })
        && r.metadata.values.updated_at.as_deref() == Some(i.updated_at.as_str())
        && r.metadata.values.web_url == i.web_url
        && i.account_id == p.request.context.account_id
        && i.repository_id.as_deref() == Some(f.repository.id.as_str())
        && f.repository.id == p.request.context.repository_id
        && f.repository.account_id == i.account_id
        && f.repository.provider_id == p.repository_native
        && e.preparation.epoch == p.request.context.authorization_epoch
        && i.kind == crate::RemoteItemKind::Issue
        && i.title == p.title
        && i.body.as_deref().unwrap_or("") == p.body
        && !i.body_omitted
        && i.head_oid.is_none()
        && i.is_draft.is_none()
        && i.unread.is_none()
        && i.reason.is_none()
        && i.provider_id
            .parse::<u64>()
            .is_ok_and(|n| n > 0 && n.to_string() == i.provider_id)
        && i.number
            .as_ref()
            .is_some_and(|s| s.parse::<u64>().is_ok_and(|n| n > 0 && n.to_string() == *s))
        && i.id == format!("github:issue:{}", i.provider_id)
        && i.web_url.as_deref()
            == Some(
                format!(
                    "https://github.com/{}/issues/{}",
                    f.repository.full_name,
                    i.number.as_deref().unwrap_or("")
                )
                .as_str(),
            )
        && i.author
            .as_ref()
            .is_some_and(|s| !s.is_empty() && s.len() <= 256 && !s.chars().any(char::is_control))
        && chrono::DateTime::parse_from_rfc3339(&r.created_at).is_ok()
        && chrono::DateTime::parse_from_rfc3339(&i.updated_at).is_ok()
        && matches!(i.state.as_str(), "open" | "closed")
        && r.metadata.kind == crate::RemoteItemKind::Issue
        && r.metadata.values.title.as_deref() == Some(i.title.as_str())
        && r.metadata.values.state.as_deref() == Some(i.state.as_str())
        && r.metadata.source.source == "github/issue-detail/2026-03-10"
        && r.metadata.source.adapter_version == 1
        && r.metadata.source.provider_updated_at.as_deref() == Some(i.updated_at.as_str())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ReceiptMetadata {
    pub kind: crate::RemoteItemKind,
    pub values: crate::ResourceMetadataValues,
    pub source: crate::MetadataSource,
    pub fields: Vec<(crate::MetadataField, crate::DetailValueState)>,
}
impl ReceiptMetadata {
    pub(crate) fn from_observation(m: crate::ResourceMetadataObservation) -> Self {
        Self {
            kind: m.kind,
            values: m.values,
            source: m.source,
            fields: m.fields.into_iter().map(|f| (f.field, f.state)).collect(),
        }
    }
    pub(crate) fn observation(&self) -> crate::ResourceMetadataObservation {
        crate::ResourceMetadataObservation {
            kind: self.kind.clone(),
            values: self.values.clone(),
            source: self.source.clone(),
            fields: self
                .fields
                .iter()
                .map(|(field, state)| crate::MetadataObservedField {
                    field: *field,
                    state: *state,
                })
                .collect(),
        }
    }
}
