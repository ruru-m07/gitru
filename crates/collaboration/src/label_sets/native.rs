use super::*;
use crate::{CollaborationError, DetailValue, RemoteItem, RemoteRepository, commands::*};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

pub(crate) const OPERATION: &str = "github.edit_labels";
pub(crate) const MAX_TOUCHED_LABELS: usize = 32;
pub(crate) const MAX_LABELS: usize = 100;
pub(crate) const MAX_BODY: usize = 16_384;
type Result<T> = std::result::Result<T, CollaborationError>;

pub(crate) fn invalid() -> CollaborationError {
    CollaborationError::invalid("Invalid bounded label intent")
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct LabelBase {
    pub labels: Vec<LabelIdentity>,
    pub updated_at: String,
    pub source: String,
    pub repository_native_id: String,
    pub subject_native_id: String,
    pub number: String,
}

#[derive(Debug, Clone)]
pub(crate) struct Payload {
    pub request: LabelSetRequest,
    pub base: LabelBase,
}

impl CommandPayloadCodec for Payload {
    const OPERATION_KIND: &'static str = OPERATION;
    const PAYLOAD_VERSION: u32 = 1;

    fn encode_payload(&self, fields: &mut CanonicalFields) -> Result<()> {
        validate_request(&self.request)?;
        validate_base(&self.base)?;
        fields.bool(1, self.request.accept_best_effort)?;
        fields.string(2, &canonical_labels(&self.request.add_labels)?)?;
        fields.string(3, &canonical_labels(&self.request.remove_labels)?)?;
        fields.string(4, &canonical_labels(&self.base.labels)?)?;
        fields.string(5, &self.base.updated_at)?;
        fields.string(6, &self.base.source)?;
        fields.string(7, &self.base.repository_native_id)?;
        fields.string(8, &self.base.subject_native_id)?;
        fields.string(9, &self.base.number)?;
        fields.string(10, &self.request.context.authorization_view)?;
        fields.string(11, &self.request.context.review_token)
    }
}

fn canonical_labels(labels: &[LabelIdentity]) -> Result<String> {
    serde_json::to_string(labels).map_err(|_| invalid())
}

pub(crate) fn label_valid(label: &LabelIdentity) -> bool {
    label
        .provider_id
        .parse::<u64>()
        .ok()
        .is_some_and(|id| id > 0 && id.to_string() == label.provider_id)
        && !label.name.is_empty()
        && label.name.len() <= 1024
        && label.name.trim() == label.name
        && !matches!(label.name.as_str(), "." | "..")
        && !label.name.chars().any(char::is_control)
        && label.color.as_ref().is_none_or(|color| {
            color.len() == 6
                && color
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
        })
}

pub(crate) fn labels_valid(labels: &[LabelIdentity], maximum: usize) -> bool {
    if labels.len() > maximum || labels.iter().any(|label| !label_valid(label)) {
        return false;
    }
    let mut ids = HashSet::with_capacity(labels.len());
    let mut names = HashSet::with_capacity(labels.len());
    labels
        .iter()
        .all(|label| ids.insert(&label.provider_id) && names.insert(label.name.to_lowercase()))
}

fn labels_sorted(labels: &[LabelIdentity]) -> bool {
    labels.windows(2).all(|pair| {
        pair[0]
            .provider_id
            .parse::<u64>()
            .expect("validated label identity")
            < pair[1]
                .provider_id
                .parse::<u64>()
                .expect("validated label identity")
    })
}

pub(crate) fn sorted(mut labels: Vec<LabelIdentity>) -> Vec<LabelIdentity> {
    labels.sort_by(|left, right| {
        left.provider_id
            .parse::<u64>()
            .unwrap_or(u64::MAX)
            .cmp(&right.provider_id.parse::<u64>().unwrap_or(u64::MAX))
            .then_with(|| left.name.cmp(&right.name))
    });
    labels
}

pub(crate) fn target_labels(payload: &Payload) -> Result<Vec<LabelIdentity>> {
    let remove: HashSet<&str> = payload
        .request
        .remove_labels
        .iter()
        .map(|label| label.provider_id.as_str())
        .collect();
    let mut target: Vec<_> = payload
        .base
        .labels
        .iter()
        .filter(|label| !remove.contains(label.provider_id.as_str()))
        .cloned()
        .collect();
    target.extend(payload.request.add_labels.iter().cloned());
    target = sorted(target);
    if !labels_valid(&target, MAX_LABELS) {
        return Err(invalid());
    }
    Ok(target)
}

pub(crate) fn validate_request(request: &LabelSetRequest) -> Result<()> {
    let identifier =
        |value: &str| !value.is_empty() && value.len() <= 1024 && !value.as_bytes().contains(&0);
    let revision = |value: &str, positive: bool| {
        value.len() <= 19
            && value.parse::<u64>().ok().is_some_and(|number| {
                number <= i64::MAX as u64
                    && (!positive || number > 0)
                    && number.to_string() == value
            })
    };
    if !identifier(&request.context.account_id)
        || !identifier(&request.context.subject_id)
        || !revision(&request.context.authorization_epoch, true)
        || !revision(&request.context.authorization_view, false)
        || uuid::Uuid::parse_str(&request.command_id)
            .ok()
            .is_none_or(|id| id.hyphenated().to_string() != request.command_id)
        || !request.accept_best_effort
        || request.add_labels.len() + request.remove_labels.len() == 0
        || request.add_labels.len() + request.remove_labels.len() > MAX_TOUCHED_LABELS
        || !labels_valid(&request.add_labels, MAX_TOUCHED_LABELS)
        || !labels_valid(&request.remove_labels, MAX_TOUCHED_LABELS)
        || !labels_sorted(&request.add_labels)
        || !labels_sorted(&request.remove_labels)
        || request.context.review_token.len() != 64
        || !request
            .context
            .review_token
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit())
    {
        return Err(invalid());
    }
    let add_ids: HashSet<_> = request
        .add_labels
        .iter()
        .map(|label| label.provider_id.as_str())
        .collect();
    let add_names: HashSet<_> = request
        .add_labels
        .iter()
        .map(|label| label.name.to_lowercase())
        .collect();
    if request.remove_labels.iter().any(|label| {
        add_ids.contains(label.provider_id.as_str())
            || add_names.contains(&label.name.to_lowercase())
    }) {
        return Err(invalid());
    }
    Ok(())
}

fn validate_base(base: &LabelBase) -> Result<()> {
    if !labels_valid(&base.labels, MAX_LABELS)
        || !labels_sorted(&base.labels)
        || chrono::DateTime::parse_from_rfc3339(&base.updated_at).is_err()
        || !matches!(
            base.source.as_str(),
            "github/issue-detail/2026-03-10" | "github/pull-detail/2026-03-10"
        )
        || !base
            .repository_native_id
            .parse::<u64>()
            .ok()
            .is_some_and(|id| id > 0 && id.to_string() == base.repository_native_id)
        || !base
            .subject_native_id
            .parse::<u64>()
            .ok()
            .is_some_and(|id| id > 0 && id.to_string() == base.subject_native_id)
        || !base
            .number
            .parse::<u64>()
            .ok()
            .is_some_and(|number| number > 0 && number.to_string() == base.number)
    {
        return Err(invalid());
    }
    Ok(())
}

pub(crate) fn validate_payload(payload: &Payload) -> Result<()> {
    validate_request(&payload.request)?;
    validate_base(&payload.base)?;
    if payload.request.add_labels.iter().any(|label| {
        payload.base.labels.iter().any(|base| {
            base.provider_id == label.provider_id
                || base.name.to_lowercase() == label.name.to_lowercase()
        })
    }) || payload
        .request
        .remove_labels
        .iter()
        .any(|label| !payload.base.labels.iter().any(|base| base == label))
    {
        return Err(invalid());
    }
    target_labels(payload)?;
    Ok(())
}

pub(crate) fn decode_payload(command: &crate::delivery::DeliveryCommand) -> Result<Payload> {
    decode(
        &command.payload,
        &command.account_id,
        &command.command_id,
        &command.target_id,
        &command.authorization_epoch,
    )
}

pub(crate) fn decode_submission(command: &CommandSubmission) -> Result<Payload> {
    decode(
        command.payload_bytes(),
        command.account_id(),
        command.command_id(),
        command.target().id(),
        command.authorization_epoch(),
    )
}

fn decode(bytes: &[u8], account: &str, id: &str, subject: &str, epoch: &str) -> Result<Payload> {
    if bytes.len() > crate::delivery::MAX_EVIDENCE_BYTES {
        return Err(invalid());
    }
    let mut rest = bytes;
    let mut values = Vec::with_capacity(11);
    for tag in 1u16..=11 {
        if rest.len() < 6 || u16::from_be_bytes(rest[..2].try_into().map_err(|_| invalid())?) != tag
        {
            return Err(invalid());
        }
        let length = u32::from_be_bytes(rest[2..6].try_into().map_err(|_| invalid())?) as usize;
        if length > rest.len() - 6 {
            return Err(invalid());
        }
        values.push(&rest[6..6 + length]);
        rest = &rest[6 + length..];
    }
    if !rest.is_empty() || values[0] != [1] {
        return Err(invalid());
    }
    let text = |index: usize| std::str::from_utf8(values[index]).map_err(|_| invalid());
    let labels = |index: usize| -> Result<Vec<LabelIdentity>> {
        let parsed: Vec<LabelIdentity> =
            serde_json::from_slice(values[index]).map_err(|_| invalid())?;
        if serde_json::to_vec(&parsed).map_err(|_| invalid())? != values[index] {
            return Err(invalid());
        }
        Ok(parsed)
    };
    let payload = Payload {
        request: LabelSetRequest {
            context: LabelSetContext {
                account_id: account.into(),
                subject_id: subject.into(),
                authorization_epoch: epoch.into(),
                authorization_view: text(9)?.into(),
                review_token: text(10)?.into(),
            },
            command_id: id.into(),
            add_labels: labels(1)?,
            remove_labels: labels(2)?,
            accept_best_effort: true,
        },
        base: LabelBase {
            labels: labels(3)?,
            updated_at: text(4)?.into(),
            source: text(5)?.into(),
            repository_native_id: text(6)?.into(),
            subject_native_id: text(7)?.into(),
            number: text(8)?.into(),
        },
    };
    validate_payload(&payload)?;
    Ok(payload)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct NativeFrame {
    pub repository: RemoteRepository,
    #[serde(with = "crate::stored_item_v1")]
    pub subject: RemoteItem,
    pub base: LabelBase,
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
    pub labels: Vec<LabelIdentity>,
    pub head: Option<crate::DetailBranch>,
    pub provider_updated_at: String,
    pub observed_at: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum Origin {
    Preflight,
    MutationReadback,
    Reconciliation,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct AddValidation {
    pub label: LabelIdentity,
    pub valid: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Evidence {
    pub frame: NativeFrame,
    pub observation: Observation,
    pub add_validations: Vec<AddValidation>,
    pub account_id: String,
    pub actor_id: String,
    pub authorization_epoch: String,
    pub command_hash: String,
    pub origin: Origin,
}

pub(crate) fn exact_member(labels: &[LabelIdentity], target: &LabelIdentity) -> bool {
    labels.iter().any(|label| label == target)
}

pub(crate) fn identity_collision(labels: &[LabelIdentity], target: &LabelIdentity) -> bool {
    labels.iter().any(|label| {
        (label.provider_id == target.provider_id
            || label.name.to_lowercase() == target.name.to_lowercase())
            && label != target
    })
}

pub(crate) fn desired_matches(payload: &Payload, observation: &Observation) -> bool {
    payload
        .request
        .add_labels
        .iter()
        .all(|label| exact_member(&observation.labels, label))
        && payload.request.remove_labels.iter().all(|label| {
            !exact_member(&observation.labels, label)
                && !identity_collision(&observation.labels, label)
        })
}

pub(crate) fn identity_conflict(payload: &Payload, evidence: &Evidence) -> bool {
    payload
        .request
        .add_labels
        .iter()
        .chain(payload.request.remove_labels.iter())
        .any(|label| identity_collision(&evidence.observation.labels, label))
        || evidence.add_validations.iter().any(|validation| {
            !validation.valid && !exact_member(&evidence.observation.labels, &validation.label)
        })
}

pub(crate) fn encode_bounded<T: Serialize>(value: &T) -> Result<Vec<u8>> {
    let bytes = serde_json::to_vec(value).map_err(|_| invalid())?;
    if bytes.len() > crate::delivery::MAX_EVIDENCE_BYTES {
        return Err(invalid());
    }
    Ok(bytes)
}

pub(crate) fn decode_bounded<T: serde::de::DeserializeOwned + Serialize>(
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

#[cfg(test)]
mod tests {
    use super::*;

    fn label(id: &str, name: &str) -> LabelIdentity {
        LabelIdentity {
            provider_id: id.into(),
            name: name.into(),
            color: Some("aabbcc".into()),
        }
    }

    #[test]
    fn typed_delta_is_bounded_disjoint_and_uses_canonical_identities() {
        let request = LabelSetRequest {
            context: LabelSetContext {
                account_id: "a".into(),
                subject_id: "issue".into(),
                authorization_epoch: "1".into(),
                authorization_view: "0".into(),
                review_token: "a".repeat(64),
            },
            command_id: "abcdefab-cdef-4abc-9def-abcdefabcdef".into(),
            add_labels: vec![label("2", "feature")],
            remove_labels: vec![label("1", "bug")],
            accept_best_effort: true,
        };
        validate_request(&request).unwrap();
        for mutate in 0..5 {
            let mut invalid_request = request.clone();
            match mutate {
                0 => invalid_request.add_labels[0].provider_id = "02".into(),
                1 => invalid_request.add_labels[0].name = " feature".into(),
                2 => invalid_request.add_labels[0].color = Some("AABBCC".into()),
                3 => invalid_request.remove_labels = invalid_request.add_labels.clone(),
                _ => invalid_request.accept_best_effort = false,
            }
            assert!(validate_request(&invalid_request).is_err(), "case {mutate}");
        }
    }

    #[test]
    fn label_names_are_conservatively_case_insensitive() {
        assert!(!labels_valid(
            &[label("1", "Bug"), label("2", "bug")],
            MAX_LABELS
        ));
        assert!(identity_collision(&[label("1", "Bug")], &label("2", "bug")));
        let mut request = LabelSetRequest {
            context: LabelSetContext {
                account_id: "a".into(),
                subject_id: "issue".into(),
                authorization_epoch: "1".into(),
                authorization_view: "0".into(),
                review_token: "a".repeat(64),
            },
            command_id: "abcdefab-cdef-4abc-9def-abcdefabcdef".into(),
            add_labels: vec![label("2", "Bug")],
            remove_labels: vec![label("1", "bug")],
            accept_best_effort: true,
        };
        assert!(validate_request(&request).is_err());
        request.remove_labels.clear();
        assert!(validate_request(&request).is_ok());
    }
}
