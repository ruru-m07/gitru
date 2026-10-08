//! Operation-v1 authored intent and receipt bytes. Never deserialize send consent from storage.
use super::*;
use crate::{CollaborationError, RemoteItem, RemoteItemKind, RemoteRepository, commands::*};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
pub(crate) const OPERATION: &str = "github.create_pull_request";
pub(crate) const PROOF: &str = "github.pull_created";
pub(crate) const MAX_BYTES: usize = 65536;
pub(crate) const GRANT_SECONDS: u64 = 60;
pub(crate) const MAX_GRANTS: usize = 32;
type Result<T> = std::result::Result<T, CollaborationError>;
pub(crate) fn invalid() -> CollaborationError {
    CollaborationError::invalid("Invalid bounded pull request creation")
}
pub(crate) fn identifier(s: &str) -> bool {
    !s.is_empty() && s.len() <= 1024 && !s.chars().any(char::is_control)
}
pub(crate) fn native_id(s: &str) -> bool {
    s.len() <= 20 && s.parse::<u64>().is_ok_and(|n| n > 0 && n.to_string() == s)
}
pub(crate) fn repository_path(s: &str) -> bool {
    let parts: Vec<_> = s.split('/').collect();
    s.len() <= 512
        && parts.len() == 2
        && parts.iter().all(|part| {
            !part.is_empty()
                && !matches!(*part, "." | "..")
                && part
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
        })
}
pub(crate) fn oid(s: &str) -> bool {
    s.len() == 40
        && s.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
pub(crate) fn uuid(s: &str) -> bool {
    uuid::Uuid::parse_str(s).is_ok_and(|u| u.hyphenated().to_string() == s)
}
pub(crate) fn revision(s: &str, positive: bool) -> Result<i64> {
    let n = s.parse::<i64>().map_err(|_| invalid())?;
    if s.len() > 19 || n < 0 || positive && n == 0 || n.to_string() != s {
        return Err(invalid());
    }
    Ok(n)
}
/// This bounds authored syntax; the native Git inspection also runs check-ref-format.
pub(crate) fn branch(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 1024
        && !s.starts_with('-')
        && !s.starts_with('/')
        && !s.ends_with('/')
        && !s.ends_with('.')
        && !s.contains("..")
        && !s.contains("@{")
        && !s.chars().any(|c| c.is_control() || " ~^:?*[\\".contains(c))
        && s.split('/')
            .all(|part| !part.is_empty() && !part.starts_with('.') && !part.ends_with(".lock"))
}
pub(crate) fn validate_key(k: &PullDraftKey) -> Result<()> {
    if !identifier(&k.account_id) || !identifier(&k.repository_id) || !uuid(&k.draft_id) {
        return Err(invalid());
    }
    Ok(())
}
pub(crate) fn validate_values(v: &PullDraftValues, complete: bool) -> Result<()> {
    crate::issue_creation::native::validate_title(&v.title, complete)?;
    crate::issue_creation::native::validate_body(&v.body)?;
    for s in [&v.source_branch, &v.base_branch] {
        if !(s.is_empty() && !complete || branch(s)) {
            return Err(invalid());
        }
    }
    for s in [&v.local_repository_id, &v.link_id, &v.link_generation] {
        if !(s.is_empty() && !complete || identifier(s)) {
            return Err(invalid());
        }
    }
    if complete && v.source_branch == v.base_branch {
        return Err(invalid());
    }
    Ok(())
}
pub(crate) fn validate_save(r: &SavePullDraftRequest) -> Result<()> {
    validate_key(&r.key)?;
    revision(&r.authorization_epoch, true)?;
    revision(&r.authorization_view, false)?;
    revision(&r.expected_generation, false)?;
    validate_values(&r.values, false)
}
pub(crate) fn validate_preview(r: &PreviewPullCreationRequest) -> Result<()> {
    validate_key(&r.key)?;
    revision(&r.authorization_epoch, true)?;
    revision(&r.authorization_view, false)?;
    revision(&r.draft_generation, true)?;
    Ok(())
}
pub(crate) fn validate_send(r: &SubmitPullRequest) -> Result<()> {
    validate_key(&r.context.key)?;
    revision(&r.context.authorization_epoch, true)?;
    revision(&r.context.authorization_view, false)?;
    revision(&r.context.draft_generation, true)?;
    if !uuid(&r.command_id)
        || !uuid(&r.context.grant_id)
        || !oid(&r.context.source_oid)
        || !oid(&r.context.base_oid)
        || r.context.source_oid == r.context.base_oid
        || !r.confirm_current_branches
    {
        return Err(invalid());
    }
    Ok(())
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct LocalProof {
    pub local_repository_id: String,
    pub link_id: String,
    pub link_generation: String,
    pub registration_proof: String,
    pub remote_digest: String,
    pub source_branch: String,
    pub source_oid: String,
}
impl LocalProof {
    pub(crate) fn from_observation(o: &PullCreationLocalObservation) -> Result<Self> {
        let v = Self {
            local_repository_id: o.query.local_repository_id.clone(),
            link_id: o.link.id.clone(),
            link_generation: o.link.generation.clone(),
            registration_proof: o.query.registration_proof.clone().ok_or_else(invalid)?,
            remote_digest: o.query.remote_digest.clone().ok_or_else(invalid)?,
            source_branch: o.source_branch.clone(),
            source_oid: o.source_oid.clone(),
        };
        v.validate()?;
        Ok(v)
    }
    pub(crate) fn validate(&self) -> Result<()> {
        if [
            &self.local_repository_id,
            &self.link_id,
            &self.link_generation,
            &self.registration_proof,
            &self.remote_digest,
        ]
        .iter()
        .any(|s| !identifier(s))
            || !branch(&self.source_branch)
            || !oid(&self.source_oid)
        {
            return Err(invalid());
        }
        Ok(())
    }
    pub(crate) fn matches_values(&self, v: &PullDraftValues) -> bool {
        self.local_repository_id == v.local_repository_id
            && self.link_id == v.link_id
            && self.link_generation == v.link_generation
            && self.source_branch == v.source_branch
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Payload {
    pub request: SubmitPullRequest,
    pub actor_id: String,
    pub repository_native: String,
    pub values: PullDraftValues,
    pub local: LocalProof,
}
impl Payload {
    fn validate(&self) -> Result<()> {
        validate_send(&self.request)?;
        validate_values(&self.values, true)?;
        self.local.validate()?;
        if !native_id(&self.actor_id)
            || !native_id(&self.repository_native)
            || !self.local.matches_values(&self.values)
            || self.local.source_oid != self.request.context.source_oid
        {
            return Err(invalid());
        }
        Ok(())
    }
}
impl CommandPayloadCodec for Payload {
    const OPERATION_KIND: &'static str = OPERATION;
    const PAYLOAD_VERSION: u32 = 1;
    fn encode_payload(&self, fields: &mut CanonicalFields) -> Result<()> {
        self.validate()?;
        fields.bytes(1, &encode(self)?)
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
    if bytes.len() < 6
        || bytes.len() > MAX_BYTES
        || u16::from_be_bytes(bytes[..2].try_into().map_err(|_| invalid())?) != 1
        || u32::from_be_bytes(bytes[2..6].try_into().map_err(|_| invalid())?) as usize
            != bytes.len() - 6
    {
        return Err(invalid());
    }
    let p: Payload = decode_json(&bytes[6..])?;
    p.validate()?;
    if p.request.context.key.account_id != account
        || p.request.command_id != command
        || p.request.context.key.repository_id != repository
        || p.request.context.authorization_epoch != epoch
    {
        return Err(invalid());
    }
    Ok(p)
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Frame {
    pub repository: RemoteRepository,
    pub authorization_view: String,
    pub draft_generation: String,
    pub values: PullDraftValues,
    pub local: LocalProof,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Preparation {
    pub frame: Frame,
    pub actor: String,
    pub epoch: String,
    pub command_hash: String,
    pub source_oid: String,
    pub base_oid: String,
    pub observed_at: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ReceiptEvidence {
    pub preparation: Preparation,
    pub receipt: CreatedReceipt,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CreatedReceipt {
    #[serde(with = "crate::stored_item_v1")]
    pub item: RemoteItem,
    pub metadata: ReceiptMetadata,
    pub created_at: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ReceiptMetadata {
    pub kind: RemoteItemKind,
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
pub(crate) fn encode<T: Serialize>(v: &T) -> Result<Vec<u8>> {
    let b = serde_json::to_vec(v).map_err(|_| invalid())?;
    if b.len() > MAX_BYTES {
        return Err(invalid());
    }
    Ok(b)
}
pub(crate) fn decode_json<T: serde::de::DeserializeOwned + Serialize>(bytes: &[u8]) -> Result<T> {
    if bytes.len() > MAX_BYTES {
        return Err(invalid());
    }
    let v: T = serde_json::from_slice(bytes).map_err(|_| invalid())?;
    if encode(&v)? != bytes {
        return Err(invalid());
    }
    Ok(v)
}
pub(crate) fn content_hash(v: &PullDraftValues) -> Result<Vec<u8>> {
    validate_values(v, false)?;
    Ok(Sha256::digest(encode(v)?).to_vec())
}
pub(crate) fn command_hash(c: &crate::delivery::DeliveryCommand) -> String {
    crate::guarded_merge::native::command_hash(c)
}
pub(crate) fn receipt_matches(e: &ReceiptEvidence, p: &Payload) -> bool {
    use crate::{DetailValueState::Known, MetadataField as F};
    let prep = &e.preparation;
    let f = &prep.frame;
    let r = &e.receipt;
    let i = &r.item;
    let m = &r.metadata;
    let clocks = chrono::DateTime::parse_from_rfc3339(&r.created_at)
        .ok()
        .zip(chrono::DateTime::parse_from_rfc3339(&i.updated_at).ok());
    let branch_matches = |b: Option<&crate::DetailBranch>, name: &str| {
        b.is_some_and(|b| {
            branch(&b.name)
                && b.name == name
                && oid(&b.oid)
                && b.repository.as_ref().is_some_and(|repo| {
                    repo.provider_id == p.repository_native
                        && repo.full_name == f.repository.full_name
                        && repo.web_url.as_deref()
                            == Some(
                                format!("https://github.com/{}", f.repository.full_name).as_str(),
                            )
                })
        })
    };
    p.validate().is_ok()
        && f.local.validate().is_ok()
        && prep.actor == p.actor_id
        && prep.epoch == p.request.context.authorization_epoch
        && prep.source_oid == p.request.context.source_oid
        && prep.base_oid == p.request.context.base_oid
        && chrono::DateTime::parse_from_rfc3339(&prep.observed_at).is_ok()
        && f.authorization_view == p.request.context.authorization_view
        && f.draft_generation == p.request.context.draft_generation
        && f.values == p.values
        && f.local == p.local
        && f.repository.id == p.request.context.key.repository_id
        && f.repository.provider_id == p.repository_native
        && f.repository.account_id == p.request.context.key.account_id
        && f.repository.selected
        && repository_path(&f.repository.full_name)
        && f.repository.web_url == format!("https://github.com/{}", f.repository.full_name)
        && crate::storage::validate_resource_metadata(&m.observation()).is_ok()
        && m.fields.len() == F::COMMON.len() + F::PULL.len()
        && [
            F::Title,
            F::State,
            F::Author,
            F::WebUrl,
            F::UpdatedAt,
            F::IsDraft,
            F::Head,
            F::Base,
            F::MergedAt,
        ]
        .iter()
        .all(|field| m.fields.contains(&(*field, Known)))
        && clocks.is_some_and(|(created, updated)| updated >= created)
        && i.kind == RemoteItemKind::PullRequest
        && i.native_inbox.is_none()
        && i.reason.is_none()
        && i.unread.is_none()
        && i.account_id == p.request.context.key.account_id
        && i.repository_id.as_deref() == Some(f.repository.id.as_str())
        && native_id(&i.provider_id)
        && i.number.as_deref().is_some_and(native_id)
        && i.id == format!("github:pr:{}", i.provider_id)
        && i.title == p.values.title
        && i.body.as_deref().unwrap_or("") == p.values.body
        && !i.body_omitted
        && i.is_draft == Some(p.values.is_draft)
        && m.values.is_draft == i.is_draft
        && matches!(i.state.as_str(), "open" | "closed" | "merged")
        && (i.state == "merged") == m.values.merged_at.is_some()
        && i.web_url.as_deref()
            == Some(
                format!(
                    "https://github.com/{}/pull/{}",
                    f.repository.full_name,
                    i.number.as_deref().unwrap_or("")
                )
                .as_str(),
            )
        && i.author
            .as_ref()
            .is_some_and(|s| !s.is_empty() && s.len() <= 256 && !s.chars().any(char::is_control))
        && m.kind == RemoteItemKind::PullRequest
        && m.values.title.as_deref() == Some(i.title.as_str())
        && m.values.state.as_deref() == Some(i.state.as_str())
        && m.values.web_url == i.web_url
        && m.values.updated_at.as_deref() == Some(i.updated_at.as_str())
        && m.values.author.as_ref().is_some_and(|a| {
            a.provider_id == p.actor_id && Some(a.login.as_str()) == i.author.as_deref()
        })
        && branch_matches(m.values.head.as_ref(), &p.values.source_branch)
        && branch_matches(m.values.base.as_ref(), &p.values.base_branch)
        && m.values.head.as_ref().map(|h| h.oid.as_str()) == i.head_oid.as_deref()
        && m.source.source == "github/pull-detail/2026-03-10"
        && m.source.adapter_version == 1
        && m.source.provider_updated_at.as_deref() == Some(i.updated_at.as_str())
}
