//! Frozen github.create_issue payload2 carriers; v1 remains in issue_creation.
#![allow(
    dead_code,
    reason = "v2 delivery is gated on the qualified schema24 prerequisite"
)]
use super::*;
use crate::{
    CollaborationError,
    commands::{CanonicalFields, CommandPayloadCodec},
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashSet;

pub(crate) const MAX_BYTES: usize = 65_536;
pub(crate) const MAX_LABELS: usize = 32;
pub(crate) const MAX_ASSIGNEES: usize = 10;
type Result<T> = std::result::Result<T, CollaborationError>;
pub(crate) fn invalid() -> CollaborationError {
    CollaborationError::invalid("Invalid bounded issue metadata")
}
pub(crate) fn positive(s: &str) -> Result<()> {
    if s.parse::<u64>()
        .ok()
        .is_none_or(|n| n == 0 || n.to_string() != s)
    {
        Err(invalid())
    } else {
        Ok(())
    }
}
pub(crate) fn display(s: &str, max: usize) -> Result<()> {
    if s.is_empty() || s.len() > max || s.chars().any(char::is_control) {
        Err(invalid())
    } else {
        Ok(())
    }
}
pub(crate) fn label(v: &IssueMetadataLabel) -> Result<()> {
    positive(&v.provider_id)?;
    display(&v.name, 1024)?;
    if v.color
        .as_ref()
        .is_some_and(|c| c.len() != 6 || !c.bytes().all(|b| b.is_ascii_hexdigit()))
    {
        return Err(invalid());
    }
    Ok(())
}
pub(crate) fn assignee(v: &IssueMetadataAssignee) -> Result<()> {
    positive(&v.provider_id)?;
    display(&v.login, 256)
}
pub(crate) fn milestone(v: &IssueMetadataMilestone) -> Result<()> {
    positive(&v.provider_id)?;
    positive(&v.number)?;
    display(&v.title, 1024)
}
pub(crate) fn selection(v: &IssueMetadataSelection) -> Result<()> {
    if v.labels.len() > MAX_LABELS || v.assignees.len() > MAX_ASSIGNEES {
        return Err(invalid());
    }
    let mut ids = HashSet::new();
    let mut aliases = HashSet::new();
    for v in &v.labels {
        label(v)?;
        if !ids.insert(&v.provider_id) || !aliases.insert(&v.name) {
            return Err(invalid());
        }
    }
    ids.clear();
    aliases.clear();
    for v in &v.assignees {
        assignee(v)?;
        if !ids.insert(&v.provider_id) || !aliases.insert(&v.login) {
            return Err(invalid());
        }
    }
    if let Some(v) = &v.milestone {
        milestone(v)?;
    }
    Ok(())
}
pub(crate) fn canonicalize(mut v: IssueMetadataSelection) -> Result<IssueMetadataSelection> {
    selection(&v)?;
    v.labels.sort_by(|a, b| a.provider_id.cmp(&b.provider_id));
    v.assignees
        .sort_by(|a, b| a.provider_id.cmp(&b.provider_id));
    Ok(v)
}
// Explicit v2 wire types: adding fields to public DTOs cannot change saved bytes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct LabelV2 {
    pub id: String,
    pub name: String,
    pub color: Option<String>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct AssigneeV2 {
    pub id: String,
    pub login: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct MilestoneV2 {
    pub id: String,
    pub number: String,
    pub title: String,
}
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SelectionV2 {
    pub labels: Vec<LabelV2>,
    pub assignees: Vec<AssigneeV2>,
    pub milestone: Option<MilestoneV2>,
}
impl SelectionV2 {
    pub(crate) fn from_public(v: IssueMetadataSelection) -> Result<Self> {
        let v = canonicalize(v)?;
        Ok(Self {
            labels: v
                .labels
                .into_iter()
                .map(|v| LabelV2 {
                    id: v.provider_id,
                    name: v.name,
                    color: v.color,
                })
                .collect(),
            assignees: v
                .assignees
                .into_iter()
                .map(|v| AssigneeV2 {
                    id: v.provider_id,
                    login: v.login,
                })
                .collect(),
            milestone: v.milestone.map(|v| MilestoneV2 {
                id: v.provider_id,
                number: v.number,
                title: v.title,
            }),
        })
    }
    pub(crate) fn public(&self) -> IssueMetadataSelection {
        IssueMetadataSelection {
            labels: self
                .labels
                .iter()
                .map(|v| IssueMetadataLabel {
                    provider_id: v.id.clone(),
                    name: v.name.clone(),
                    color: v.color.clone(),
                })
                .collect(),
            assignees: self
                .assignees
                .iter()
                .map(|v| IssueMetadataAssignee {
                    provider_id: v.id.clone(),
                    login: v.login.clone(),
                })
                .collect(),
            milestone: self.milestone.as_ref().map(|v| IssueMetadataMilestone {
                provider_id: v.id.clone(),
                number: v.number.clone(),
                title: v.title.clone(),
            }),
        }
    }
    pub(crate) fn validate(&self) -> Result<()> {
        if Self::from_public(self.public())? != *self {
            return Err(invalid());
        }
        Ok(())
    }
    pub(crate) fn is_empty(&self) -> bool {
        self.labels.is_empty() && self.assignees.is_empty() && self.milestone.is_none()
    }
}
pub(crate) fn encode<T: Serialize>(v: &T) -> Result<Vec<u8>> {
    let b = serde_json::to_vec(v).map_err(|_| invalid())?;
    if b.len() > MAX_BYTES {
        Err(invalid())
    } else {
        Ok(b)
    }
}
pub(crate) fn decode_json<T: serde::de::DeserializeOwned + Serialize>(b: &[u8]) -> Result<T> {
    if b.len() > MAX_BYTES {
        return Err(invalid());
    }
    let v = serde_json::from_slice(b).map_err(|_| invalid())?;
    if encode(&v)? != b {
        return Err(invalid());
    }
    Ok(v)
}
#[derive(Debug, Clone)]
pub(crate) struct PayloadV2 {
    pub request: SubmitIssueV2Request,
    pub title: String,
    pub body: String,
    pub repository_native: String,
    pub metadata: SelectionV2,
}
impl PayloadV2 {
    pub(crate) fn validate(&self) -> Result<()> {
        let r = &self.request;
        crate::issue_creation::native::validate_send(&crate::SubmitIssueRequest {
            context: r.context.clone(),
            draft_id: r.draft_id.clone(),
            draft_generation: r.draft_generation.clone(),
            command_id: r.command_id.clone(),
            accept_background_delivery: r.accept_background_delivery,
        })?;
        crate::issue_creation::native::validate_title(&self.title, true)?;
        crate::issue_creation::native::validate_body(&self.body)?;
        positive(&self.repository_native)?;
        self.metadata.validate()?;
        if !self.metadata.is_empty() && !r.accept_metadata_best_effort {
            return Err(invalid());
        }
        // Ten tagged fields, two one-byte consents; the generic envelope limit
        // is broader than this operation's frozen payload limit.
        let size = 10 * 6
            + 2
            + self.title.len()
            + self.body.len()
            + r.draft_id.len()
            + r.draft_generation.len()
            + self.repository_native.len()
            + r.context.authorization_view.len()
            + r.context.review_token.len()
            + encode(&self.metadata)?.len();
        if size > MAX_BYTES {
            return Err(invalid());
        }
        Ok(())
    }
}
impl CommandPayloadCodec for PayloadV2 {
    const OPERATION_KIND: &'static str = "github.create_issue";
    const PAYLOAD_VERSION: u32 = 2;
    fn encode_payload(&self, f: &mut CanonicalFields) -> Result<()> {
        self.validate()?;
        f.string(1, &self.title)?;
        f.string(2, &self.body)?;
        f.string(3, &self.request.draft_id)?;
        f.string(4, &self.request.draft_generation)?;
        f.string(5, &self.repository_native)?;
        f.string(6, &self.request.context.authorization_view)?;
        f.string(7, &self.request.context.review_token)?;
        f.bool(8, true)?;
        f.bytes(9, &encode(&self.metadata)?)?;
        f.bool(10, self.request.accept_metadata_best_effort)
    }
}
#[derive(Serialize)]
struct WireBodyV2<'a> {
    title: &'a str,
    body: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    labels: Option<Vec<&'a str>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    assignees: Option<Vec<&'a str>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    milestone: Option<u64>,
}
pub(crate) fn request_bytes(p: &PayloadV2) -> Result<Vec<u8>> {
    p.validate()?;
    encode(&WireBodyV2 {
        title: &p.title,
        body: &p.body,
        labels: (!p.metadata.labels.is_empty())
            .then(|| p.metadata.labels.iter().map(|v| v.name.as_str()).collect()),
        assignees: (!p.metadata.assignees.is_empty()).then(|| {
            p.metadata
                .assignees
                .iter()
                .map(|v| v.login.as_str())
                .collect()
        }),
        milestone: p
            .metadata
            .milestone
            .as_ref()
            .map(|v| v.number.parse())
            .transpose()
            .map_err(|_| invalid())?,
    })
}
pub(crate) fn decode_payload(
    bytes: &[u8],
    account: &str,
    command: &str,
    repository: &str,
    epoch: &str,
) -> Result<PayloadV2> {
    if bytes.len() > MAX_BYTES {
        return Err(invalid());
    }
    let mut rest = bytes;
    let mut fields = Vec::with_capacity(10);
    for tag in 1u16..=10 {
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
    if !rest.is_empty() || fields[7] != [1] || !matches!(fields[9], [0] | [1]) {
        return Err(invalid());
    }
    let t = |i: usize| String::from_utf8(fields[i].to_vec()).map_err(|_| invalid());
    let p = PayloadV2 {
        request: SubmitIssueV2Request {
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
            accept_metadata_best_effort: fields[9] == [1],
        },
        title: t(0)?,
        body: t(1)?,
        repository_native: t(4)?,
        metadata: decode_json(fields[8])?,
    };
    p.validate()?;
    Ok(p)
}
pub(crate) fn content_hash(title: &str, body: &str, metadata: &SelectionV2) -> Result<Vec<u8>> {
    metadata.validate()?;
    let mut h = Sha256::new();
    h.update(b"gitru.issue-draft.v2\0");
    for v in [
        title.as_bytes(),
        body.as_bytes(),
        encode(metadata)?.as_slice(),
    ] {
        h.update((v.len() as u64).to_be_bytes());
        h.update(v);
    }
    Ok(h.finalize().to_vec())
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum SetObservationV2 {
    Known {
        present_ids: Vec<String>,
    },
    Unobserved {
        reason: IssueMetadataUnobservedReason,
    },
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum MilestoneObservationV2 {
    Known {
        provider_id: Option<String>,
    },
    Unobserved {
        reason: IssueMetadataUnobservedReason,
    },
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct MetadataObservationV2 {
    pub labels: SetObservationV2,
    pub assignees: SetObservationV2,
    pub milestone: MilestoneObservationV2,
}
impl MetadataObservationV2 {
    pub(crate) fn outcome(
        &self,
        command: &str,
        selection: &SelectionV2,
    ) -> Result<IssueMetadataOutcome> {
        selection.validate()?;
        crate::issue_creation::native::validate_uuid(command)?;
        let set =
            |field, ids: Vec<&str>, obs: &SetObservationV2| -> Result<IssueMetadataFieldOutcome> {
                let (result, reason) = match obs {
                    SetObservationV2::Known { present_ids } => {
                        if present_ids.windows(2).any(|v| v[0] >= v[1])
                            || present_ids.iter().any(|id| !ids.contains(&id.as_str()))
                        {
                            return Err(invalid());
                        }
                        (
                            if ids.is_empty() {
                                IssueMetadataResult::NotRequested
                            } else if present_ids.len() == ids.len() {
                                IssueMetadataResult::Applied
                            } else {
                                IssueMetadataResult::Different
                            },
                            None,
                        )
                    }
                    SetObservationV2::Unobserved { reason } => (
                        if ids.is_empty() {
                            IssueMetadataResult::NotRequested
                        } else {
                            IssueMetadataResult::Unobserved
                        },
                        (!ids.is_empty()).then_some(*reason),
                    ),
                };
                Ok(IssueMetadataFieldOutcome {
                    field,
                    result,
                    reason,
                })
            };
        let mut fields = vec![
            set(
                IssueMetadataField::Labels,
                selection.labels.iter().map(|v| v.id.as_str()).collect(),
                &self.labels,
            )?,
            set(
                IssueMetadataField::Assignees,
                selection.assignees.iter().map(|v| v.id.as_str()).collect(),
                &self.assignees,
            )?,
        ];
        let (result, reason) = match &self.milestone {
            MilestoneObservationV2::Known { provider_id } => {
                if let Some(id) = provider_id {
                    positive(id)?;
                }
                (
                    match &selection.milestone {
                        None => IssueMetadataResult::NotRequested,
                        Some(v) if provider_id.as_deref() == Some(&v.id) => {
                            IssueMetadataResult::Applied
                        }
                        Some(_) => IssueMetadataResult::Different,
                    },
                    None,
                )
            }
            MilestoneObservationV2::Unobserved { reason } => (
                if selection.milestone.is_none() {
                    IssueMetadataResult::NotRequested
                } else {
                    IssueMetadataResult::Unobserved
                },
                selection.milestone.as_ref().map(|_| *reason),
            ),
        };
        fields.push(IssueMetadataFieldOutcome {
            field: IssueMetadataField::Milestone,
            result,
            reason,
        });
        Ok(IssueMetadataOutcome {
            command_id: command.into(),
            needs_attention: fields.iter().any(|v| {
                matches!(
                    v.result,
                    IssueMetadataResult::Different | IssueMetadataResult::Unobserved
                )
            }),
            fields,
        })
    }
}

#[cfg(test)]
mod tests;

// Frozen receipt2 carriers deliberately exclude extensible RemoteItem/Repository.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct FrameV2 {
    pub account_id: String,
    pub repository_id: String,
    pub repository_native: String,
    pub repository_path: String,
    pub authorization_view: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PreparationV2 {
    pub frame: FrameV2,
    pub actor: String,
    pub epoch: String,
    pub command_hash: String,
    /// Native attestation after every required identity/assignability step matched.
    pub revalidated_metadata: SelectionV2,
    pub metadata_push_access: Option<bool>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CreatedCoreV2 {
    pub provider_id: String,
    pub number: String,
    pub title: String,
    pub body: Option<String>,
    pub author_id: String,
    pub author_login: String,
    pub state: String,
    pub web_url: String,
    pub created_at: String,
    pub updated_at: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ReceiptV2 {
    pub preparation: PreparationV2,
    pub core: CreatedCoreV2,
    pub metadata: MetadataObservationV2,
}
pub(crate) fn repository_path(s: &str) -> bool {
    let p: Vec<_> = s.split('/').collect();
    p.len() == 2
        && p.iter().all(|p| {
            !p.is_empty()
                && p.len() <= 255
                && !matches!(*p, "." | "..")
                && p.bytes()
                    .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
        })
}
pub(crate) fn preparation_matches(p: &PreparationV2, intent: &PayloadV2) -> bool {
    intent.validate().is_ok()
        && p.frame.account_id == intent.request.context.account_id
        && p.frame.repository_id == intent.request.context.repository_id
        && p.frame.repository_native == intent.repository_native
        && p.frame.authorization_view == intent.request.context.authorization_view
        && p.epoch == intent.request.context.authorization_epoch
        && repository_path(&p.frame.repository_path)
        && positive(&p.actor).is_ok()
        && p.command_hash.len() == 64
        && p.command_hash
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        && p.revalidated_metadata == intent.metadata
        && (intent.metadata.is_empty() || p.metadata_push_access == Some(true))
}
pub(crate) fn receipt_matches(r: &ReceiptV2, p: &PayloadV2) -> bool {
    if !preparation_matches(&r.preparation, p)
        || positive(&r.core.provider_id).is_err()
        || positive(&r.core.number).is_err()
        || r.core.title != p.title
        || r.core.body.as_deref().unwrap_or("") != p.body
        || r.core.author_id != r.preparation.actor
        || display(&r.core.author_login, 256).is_err()
        || !matches!(r.core.state.as_str(), "open" | "closed")
        || r.core.web_url
            != format!(
                "https://github.com/{}/issues/{}",
                r.preparation.frame.repository_path, r.core.number
            )
        || r.metadata
            .outcome(&p.request.command_id, &p.metadata)
            .is_err()
    {
        return false;
    }
    let (Ok(created), Ok(updated)) = (
        chrono::DateTime::parse_from_rfc3339(&r.core.created_at),
        chrono::DateTime::parse_from_rfc3339(&r.core.updated_at),
    ) else {
        return false;
    };
    updated >= created
        && [(&r.core.created_at, created), (&r.core.updated_at, updated)]
            .iter()
            .all(|(text, time)| {
                **text
                    == time
                        .with_timezone(&chrono::Utc)
                        .to_rfc3339_opts(chrono::SecondsFormat::Nanos, true)
            })
}
/// A conservative *encoded* core reservation, independent of optional provider
/// arrays/text. Already-saved v1 proofs are never decoded through this policy.
pub(crate) fn validate_receipt_budget(p: &PayloadV2, preparation: &PreparationV2) -> Result<()> {
    if !preparation_matches(preparation, p) {
        return Err(invalid());
    }
    request_bytes(p)?;
    let max = u64::MAX.to_string();
    let core = CreatedCoreV2 {
        provider_id: max.clone(),
        number: max.clone(),
        title: p.title.clone(),
        body: Some(p.body.clone()),
        author_id: preparation.actor.clone(),
        author_login: "\\".repeat(256),
        state: "closed".into(),
        web_url: format!(
            "https://github.com/{}/issues/{max}",
            preparation.frame.repository_path
        ),
        created_at: "9999-12-31T23:59:59.999999999Z".into(),
        updated_at: "9999-12-31T23:59:59.999999999Z".into(),
    };
    // The actual selected membership evidence is <2KiB at32+10 IDs. This4KiB
    // reserve also dominates every fixed unobserved reason/enum combination.
    if encode(preparation)?.len() + encode(&core)?.len() + 4096 > MAX_BYTES {
        return Err(CollaborationError::invalid(
            "Saved draft exceeds the encoded metadata creation receipt budget; shorten its text or selections",
        ));
    }
    Ok(())
}
