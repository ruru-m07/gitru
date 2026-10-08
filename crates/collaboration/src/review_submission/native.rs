//! Frozen version-one command and receipt codecs for final GitHub reviews.
#![allow(
    dead_code,
    reason = "schema-24 admission and delivery consume these frozen codecs"
)]

use super::*;
use crate::{
    CollaborationError,
    commands::{CanonicalFields, CommandPayloadCodec, CommandSubmission},
    delivery::DeliveryCommand,
    providers::github::review_submission::{GithubReviewComment, GithubReviewSubmission},
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub(crate) const OPERATION: &str = "github.submit_review";
const MAX_CODEC_BYTES: usize = 65_536;
type Result<T> = std::result::Result<T, CollaborationError>;

pub(crate) fn invalid() -> CollaborationError {
    CollaborationError::invalid("Invalid bounded review submission")
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Payload {
    pub request: SubmitReviewRequest,
    pub event: ReviewSubmissionEvent,
    pub body: String,
    pub comments: Vec<ResolvedComment>,
    pub content_hash: [u8; 32],
    pub repository_native: String,
    pub repository_full_name: String,
    pub subject_native: String,
    pub number: String,
    pub actor_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ResolvedComment {
    pub comment_id: String,
    pub body: String,
    pub anchor: GithubReviewLineAnchor,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ContentV1 {
    event: String,
    body: String,
    comments: Vec<CommentV1>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CommentV1 {
    comment_id: String,
    body: String,
    anchor: AnchorV1,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct AnchorV1 {
    file_facet_revision: String,
    context: PullFileContextV1,
    file_key: String,
    path: String,
    start_line: Option<u32>,
    line: u32,
    start_side: Option<String>,
    side: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PullFileContextV1 {
    base_oid: String,
    head_oid: String,
    merge_base_oid: Option<String>,
    base_repository_provider_id: String,
    source_repository_provider_id: String,
    body_metadata_facet_revision: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ReviewContextV1 {
    pub(crate) base_oid: String,
    pub(crate) head_oid: String,
    pub(crate) base_repository_provider_id: String,
    pub(crate) source_repository_provider_id: String,
    pub(crate) metadata_facet_revision: String,
}

pub(crate) const ACCEPTED_PROOF: &str = "github.review_accepted";
pub(crate) const SUBMITTED_PROOF: &str = "github.review_submitted";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PreparationV1 {
    pub(crate) account_id: String,
    pub(crate) command_id: String,
    pub(crate) actor_id: String,
    pub(crate) authorization_epoch: String,
    pub(crate) authorization_view: String,
    pub(crate) repository_id: String,
    pub(crate) repository_provider_id: String,
    pub(crate) repository_full_name: String,
    pub(crate) subject_id: String,
    pub(crate) subject_provider_id: String,
    pub(crate) number: String,
    pub(crate) context: ReviewContextV1,
    pub(crate) command_hash: String,
    pub(crate) observed_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct AcceptedReceiptV1 {
    pub(crate) provider_id: String,
    pub(crate) url: String,
    pub(crate) provider_state: String,
    pub(crate) reviewed_commit_oid: String,
    pub(crate) submitted_at: String,
    pub(crate) observed_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct AcceptedEvidenceV1 {
    pub(crate) preparation: PreparationV1,
    pub(crate) receipt: AcceptedReceiptV1,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ConfirmedCommentV1 {
    pub(crate) comment_id: String,
    pub(crate) provider_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SubmittedEvidenceV1 {
    pub(crate) preparation: PreparationV1,
    pub(crate) receipt: AcceptedReceiptV1,
    pub(crate) comments: Vec<ConfirmedCommentV1>,
    pub(crate) confirmed_at: String,
}

impl CommandPayloadCodec for Payload {
    const OPERATION_KIND: &'static str = OPERATION;
    const PAYLOAD_VERSION: u32 = 1;

    fn encode_payload(&self, fields: &mut CanonicalFields) -> Result<()> {
        self.validate()?;
        fields.bytes(1, &canonical_content(self)?)?;
        fields.string(2, &self.request.draft_generation)?;
        fields.bytes(3, &self.content_hash)?;
        fields.string(4, &self.repository_native)?;
        fields.string(5, &self.repository_full_name)?;
        fields.string(6, &self.subject_native)?;
        fields.string(7, &self.number)?;
        fields.string(8, &self.actor_id)?;
        fields.bytes(
            9,
            &encode_json(&ReviewContextV1::from(&self.request.context.review_context))?,
        )?;
        fields.string(10, &self.request.context.authorization_view)?;
        fields.string(11, &self.request.context.review_token)?;
        fields.bool(12, self.request.accept_background_delivery)?;
        fields.bool(13, self.request.accept_best_effort_race)
    }
}

impl Payload {
    pub(crate) fn validate(&self) -> Result<()> {
        validate_submit(&self.request)?;
        identifier(&self.repository_native)?;
        identifier(&self.subject_native)?;
        positive(&self.number)?;
        positive(&self.actor_id)?;
        let operation = self.operation();
        operation.validate().map_err(|_| invalid())?;
        if self.content_hash != content_hash(self)? {
            return Err(invalid());
        }
        Ok(())
    }

    pub(crate) fn operation(&self) -> GithubReviewSubmission {
        GithubReviewSubmission {
            repository_provider_id: self.repository_native.clone(),
            repository_full_name: self.repository_full_name.clone(),
            subject_provider_id: self.subject_native.clone(),
            number: self.number.clone(),
            actor_id: self.actor_id.clone(),
            context: self.request.context.review_context.clone(),
            event: self.event,
            body: self.body.clone(),
            comments: self
                .comments
                .iter()
                .map(|comment| GithubReviewComment {
                    comment_id: comment.comment_id.clone(),
                    body: comment.body.clone(),
                    anchor: comment.anchor.clone(),
                })
                .collect(),
        }
    }
}

pub(crate) fn validate_submit(request: &SubmitReviewRequest) -> Result<()> {
    identifier(&request.context.account_id)?;
    identifier(&request.context.subject_id)?;
    revision(&request.context.authorization_epoch, true)?;
    revision(&request.context.authorization_view, false)?;
    revision(&request.draft_generation, true)?;
    canonical_uuid(&request.command_id)?;
    if !request.context.review_context.is_valid()
        || request.context.review_token.len() != 64
        || !request
            .context
            .review_token
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit())
        || !request.accept_background_delivery
        || !request.accept_best_effort_race
    {
        return Err(invalid());
    }
    Ok(())
}

pub(crate) fn content_hash(payload: &Payload) -> Result<[u8; 32]> {
    Ok(Sha256::digest(canonical_content(payload)?).into())
}

pub(crate) fn command_hash(command: &DeliveryCommand) -> String {
    hex(&command.hash)
}

pub(crate) fn encode_evidence<T: Serialize>(value: &T) -> Result<Vec<u8>> {
    encode_json(value)
}

pub(crate) fn decode_evidence<T: serde::de::DeserializeOwned + Serialize>(
    bytes: &[u8],
) -> Result<T> {
    decode_json(bytes)
}

pub(crate) fn preparation_matches(
    preparation: &PreparationV1,
    payload: &Payload,
    command: &DeliveryCommand,
) -> bool {
    canonical_time(&preparation.observed_at)
        && preparation.account_id == command.account_id
        && preparation.account_id == payload.request.context.account_id
        && preparation.command_id == command.command_id
        && preparation.command_id == payload.request.command_id
        && command.operation_kind == OPERATION
        && command.payload_version == 1
        && command.target_kind == "pull_request"
        && preparation.actor_id == payload.actor_id
        && preparation.authorization_epoch == command.authorization_epoch
        && preparation.authorization_epoch == payload.request.context.authorization_epoch
        && preparation.authorization_view == payload.request.context.authorization_view
        && preparation.repository_id == command.repository_id.as_deref().unwrap_or_default()
        && preparation.repository_provider_id == payload.repository_native
        && preparation.repository_full_name == payload.repository_full_name
        && preparation.subject_id == command.target_id
        && preparation.subject_id == payload.request.context.subject_id
        && preparation.subject_provider_id == payload.subject_native
        && preparation.number == payload.number
        && ReviewContext::from(preparation.context.clone())
            == payload.request.context.review_context
        && preparation.command_hash == command_hash(command)
}

pub(crate) fn accepted_matches(
    evidence: &AcceptedEvidenceV1,
    payload: &Payload,
    command: &DeliveryCommand,
) -> bool {
    preparation_matches(&evidence.preparation, payload, command)
        && receipt_matches(&evidence.receipt, payload)
        && ordered_times(
            &evidence.preparation.observed_at,
            &evidence.receipt.submitted_at,
            &evidence.receipt.observed_at,
        )
}

pub(crate) fn submitted_matches(
    evidence: &SubmittedEvidenceV1,
    payload: &Payload,
    command: &DeliveryCommand,
) -> bool {
    if !preparation_matches(&evidence.preparation, payload, command)
        || !receipt_matches(&evidence.receipt, payload)
        || !canonical_time(&evidence.confirmed_at)
        || !ordered_times(
            &evidence.preparation.observed_at,
            &evidence.receipt.submitted_at,
            &evidence.receipt.observed_at,
        )
        || !ordered_times(
            &evidence.receipt.submitted_at,
            &evidence.receipt.observed_at,
            &evidence.confirmed_at,
        )
        || evidence.comments.len() != payload.comments.len()
    {
        return false;
    }
    let mut provider_ids = std::collections::HashSet::with_capacity(evidence.comments.len());
    evidence
        .comments
        .iter()
        .zip(&payload.comments)
        .all(|(confirmed, requested)| {
            confirmed.comment_id == requested.comment_id
                && positive(&confirmed.provider_id).is_ok()
                && provider_ids.insert(confirmed.provider_id.as_str())
        })
}

fn receipt_matches(receipt: &AcceptedReceiptV1, payload: &Payload) -> bool {
    positive(&receipt.provider_id).is_ok()
        && receipt.provider_state == expected_state(payload.event)
        && receipt.reviewed_commit_oid == payload.request.context.review_context.head_oid
        && canonical_time(&receipt.submitted_at)
        && canonical_time(&receipt.observed_at)
        && exact_review_url(
            &receipt.url,
            &payload.repository_full_name,
            &payload.number,
            &receipt.provider_id,
        )
}

fn expected_state(event: ReviewSubmissionEvent) -> &'static str {
    match event {
        ReviewSubmissionEvent::Comment => "COMMENTED",
        ReviewSubmissionEvent::Approve => "APPROVED",
        ReviewSubmissionEvent::RequestChanges => "CHANGES_REQUESTED",
    }
}

fn exact_review_url(raw: &str, repository: &str, number: &str, review: &str) -> bool {
    let Ok(url) = reqwest::Url::parse(raw) else {
        return false;
    };
    let expected_fragment = format!("pullrequestreview-{review}");
    raw.len() <= 2_048
        && !raw
            .chars()
            .any(|character| character.is_control() || character.is_whitespace())
        && url.as_str() == raw
        && url.scheme() == "https"
        && url.host_str() == Some("github.com")
        && url.port().is_none()
        && url.port_or_known_default() == Some(443)
        && url.username().is_empty()
        && url.password().is_none()
        && url.query().is_none()
        && url.path() == format!("/{repository}/pull/{number}")
        && url.fragment() == Some(expected_fragment.as_str())
}

fn canonical_time(value: &str) -> bool {
    chrono::DateTime::parse_from_rfc3339(value)
        .ok()
        .map(|parsed| {
            parsed
                .with_timezone(&chrono::Utc)
                .to_rfc3339_opts(chrono::SecondsFormat::Nanos, true)
        })
        .as_deref()
        == Some(value)
}

fn ordered_times(first: &str, second: &str, third: &str) -> bool {
    let parsed = [first, second, third]
        .map(chrono::DateTime::parse_from_rfc3339)
        .map(|value| value.ok().map(|value| value.with_timezone(&chrono::Utc)));
    matches!(parsed, [Some(first), Some(second), Some(third)] if first <= second && second <= third)
}

fn hex(bytes: &[u8]) -> String {
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write as _;
        let _ = write!(&mut encoded, "{byte:02x}");
    }
    encoded
}

pub(crate) fn decode(command: &DeliveryCommand) -> Result<Payload> {
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

fn decode_parts(
    bytes: &[u8],
    account_id: &str,
    command_id: &str,
    subject_id: &str,
    authorization_epoch: &str,
) -> Result<Payload> {
    let values = decode_fields(bytes, 13)?;
    if values[2].len() != 32 || values[11] != [1] || values[12] != [1] {
        return Err(invalid());
    }
    let content: ContentV1 = decode_json(values[0])?;
    let review_context: ReviewContextV1 = decode_json(values[8])?;
    let text = |index: usize| {
        std::str::from_utf8(values[index])
            .map(str::to_owned)
            .map_err(|_| invalid())
    };
    let payload = Payload {
        request: SubmitReviewRequest {
            context: ReviewSubmissionContext {
                account_id: account_id.into(),
                subject_id: subject_id.into(),
                authorization_epoch: authorization_epoch.into(),
                authorization_view: text(9)?,
                review_context: review_context.into(),
                review_token: text(10)?,
            },
            draft_generation: text(1)?,
            command_id: command_id.into(),
            accept_background_delivery: true,
            accept_best_effort_race: true,
        },
        event: parse_event(&content.event).ok_or_else(invalid)?,
        body: content.body,
        comments: content
            .comments
            .into_iter()
            .map(ResolvedComment::from)
            .collect(),
        content_hash: values[2].try_into().map_err(|_| invalid())?,
        repository_native: text(3)?,
        repository_full_name: text(4)?,
        subject_native: text(5)?,
        number: text(6)?,
        actor_id: text(7)?,
    };
    payload.validate()?;
    Ok(payload)
}

fn canonical_content(payload: &Payload) -> Result<Vec<u8>> {
    encode_json(&ContentV1 {
        event: event_name(payload.event).into(),
        body: payload.body.clone(),
        comments: payload.comments.iter().map(CommentV1::from).collect(),
    })
}

fn decode_fields(bytes: &[u8], count: u16) -> Result<Vec<&[u8]>> {
    if bytes.len() > crate::commands::MAX_COMMAND_PAYLOAD_BYTES {
        return Err(invalid());
    }
    let mut rest = bytes;
    let mut values = Vec::with_capacity(usize::from(count));
    for tag in 1..=count {
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
    if !rest.is_empty() {
        return Err(invalid());
    }
    Ok(values)
}

fn encode_json<T: Serialize>(value: &T) -> Result<Vec<u8>> {
    let bytes = serde_json::to_vec(value).map_err(|_| invalid())?;
    if bytes.len() > MAX_CODEC_BYTES {
        return Err(invalid());
    }
    Ok(bytes)
}

fn decode_json<T: serde::de::DeserializeOwned + Serialize>(bytes: &[u8]) -> Result<T> {
    if bytes.len() > MAX_CODEC_BYTES {
        return Err(invalid());
    }
    let value = serde_json::from_slice(bytes).map_err(|_| invalid())?;
    if encode_json(&value)? != bytes {
        return Err(invalid());
    }
    Ok(value)
}

fn identifier(value: &str) -> Result<()> {
    if value.is_empty() || value.len() > 1024 || value.contains('\0') {
        Err(invalid())
    } else {
        Ok(())
    }
}

fn positive(value: &str) -> Result<u64> {
    let parsed = value.parse::<u64>().map_err(|_| invalid())?;
    if parsed == 0 || parsed > i64::MAX as u64 || parsed.to_string() != value {
        Err(invalid())
    } else {
        Ok(parsed)
    }
}

fn revision(value: &str, positive_value: bool) -> Result<u64> {
    let parsed = value.parse::<u64>().map_err(|_| invalid())?;
    if value.len() > 19
        || parsed > i64::MAX as u64
        || positive_value && parsed == 0
        || parsed.to_string() != value
    {
        Err(invalid())
    } else {
        Ok(parsed)
    }
}

fn canonical_uuid(value: &str) -> Result<()> {
    if uuid::Uuid::parse_str(value)
        .ok()
        .is_none_or(|parsed| parsed.hyphenated().to_string() != value)
    {
        Err(invalid())
    } else {
        Ok(())
    }
}

fn event_name(event: ReviewSubmissionEvent) -> &'static str {
    match event {
        ReviewSubmissionEvent::Comment => "comment",
        ReviewSubmissionEvent::Approve => "approve",
        ReviewSubmissionEvent::RequestChanges => "request_changes",
    }
}

fn parse_event(value: &str) -> Option<ReviewSubmissionEvent> {
    Some(match value {
        "comment" => ReviewSubmissionEvent::Comment,
        "approve" => ReviewSubmissionEvent::Approve,
        "request_changes" => ReviewSubmissionEvent::RequestChanges,
        _ => return None,
    })
}

fn side_name(side: ReviewDiffSide) -> &'static str {
    match side {
        ReviewDiffSide::Left => "left",
        ReviewDiffSide::Right => "right",
        ReviewDiffSide::Unknown => "unknown",
    }
}

fn parse_side(value: &str) -> ReviewDiffSide {
    match value {
        "left" => ReviewDiffSide::Left,
        "right" => ReviewDiffSide::Right,
        _ => ReviewDiffSide::Unknown,
    }
}

impl From<&ResolvedComment> for CommentV1 {
    fn from(value: &ResolvedComment) -> Self {
        Self {
            comment_id: value.comment_id.clone(),
            body: value.body.clone(),
            anchor: AnchorV1::from(&value.anchor),
        }
    }
}

impl From<CommentV1> for ResolvedComment {
    fn from(value: CommentV1) -> Self {
        Self {
            comment_id: value.comment_id,
            body: value.body,
            anchor: value.anchor.into(),
        }
    }
}

impl From<&GithubReviewLineAnchor> for AnchorV1 {
    fn from(value: &GithubReviewLineAnchor) -> Self {
        Self {
            file_facet_revision: value.file_facet_revision.clone(),
            context: PullFileContextV1::from(&value.context),
            file_key: value.file_key.clone(),
            path: value.path.clone(),
            start_line: value.start_line,
            line: value.line,
            start_side: value.start_side.map(side_name).map(str::to_owned),
            side: side_name(value.side).into(),
        }
    }
}

impl From<AnchorV1> for GithubReviewLineAnchor {
    fn from(value: AnchorV1) -> Self {
        Self {
            file_facet_revision: value.file_facet_revision,
            context: value.context.into(),
            file_key: value.file_key,
            path: value.path,
            start_line: value.start_line,
            line: value.line,
            start_side: value.start_side.as_deref().map(parse_side),
            side: parse_side(&value.side),
        }
    }
}

impl From<&PullFileContext> for PullFileContextV1 {
    fn from(value: &PullFileContext) -> Self {
        Self {
            base_oid: value.base_oid.clone(),
            head_oid: value.head_oid.clone(),
            merge_base_oid: value.merge_base_oid.clone(),
            base_repository_provider_id: value.base_repository_provider_id.clone(),
            source_repository_provider_id: value.source_repository_provider_id.clone(),
            body_metadata_facet_revision: value.body_metadata_facet_revision.clone(),
        }
    }
}

impl From<PullFileContextV1> for PullFileContext {
    fn from(value: PullFileContextV1) -> Self {
        Self {
            base_oid: value.base_oid,
            head_oid: value.head_oid,
            merge_base_oid: value.merge_base_oid,
            base_repository_provider_id: value.base_repository_provider_id,
            source_repository_provider_id: value.source_repository_provider_id,
            body_metadata_facet_revision: value.body_metadata_facet_revision,
        }
    }
}

impl From<&ReviewContext> for ReviewContextV1 {
    fn from(value: &ReviewContext) -> Self {
        Self {
            base_oid: value.base_oid.clone(),
            head_oid: value.head_oid.clone(),
            base_repository_provider_id: value.base_repository_provider_id.clone(),
            source_repository_provider_id: value.source_repository_provider_id.clone(),
            metadata_facet_revision: value.metadata_facet_revision.clone(),
        }
    }
}

impl From<ReviewContextV1> for ReviewContext {
    fn from(value: ReviewContextV1) -> Self {
        Self {
            base_oid: value.base_oid,
            head_oid: value.head_oid,
            base_repository_provider_id: value.base_repository_provider_id,
            source_repository_provider_id: value.source_repository_provider_id,
            metadata_facet_revision: value.metadata_facet_revision,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::{CommandDraft, CommandTarget, CommandTargetKind, seal_command};
    use crate::delivery::DeliveryState;

    const BASE: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const HEAD: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

    fn payload() -> Payload {
        let context = ReviewContext {
            base_oid: BASE.into(),
            head_oid: HEAD.into(),
            base_repository_provider_id: "7".into(),
            source_repository_provider_id: "8".into(),
            metadata_facet_revision: "9".into(),
        };
        let mut payload = Payload {
            request: SubmitReviewRequest {
                context: ReviewSubmissionContext {
                    account_id: "account".into(),
                    subject_id: "pull".into(),
                    authorization_epoch: "2".into(),
                    authorization_view: "3".into(),
                    review_context: context.clone(),
                    review_token: "a".repeat(64),
                },
                draft_generation: "4".into(),
                command_id: "00000000-0000-4000-8000-000000000001".into(),
                accept_background_delivery: true,
                accept_best_effort_race: true,
            },
            event: ReviewSubmissionEvent::RequestChanges,
            body: "Please fix this.".into(),
            comments: vec![ResolvedComment {
                comment_id: "00000000-0000-4000-8000-000000000002".into(),
                body: "Bound this range.".into(),
                anchor: GithubReviewLineAnchor {
                    file_facet_revision: "10".into(),
                    context: PullFileContext {
                        base_oid: BASE.into(),
                        head_oid: HEAD.into(),
                        merge_base_oid: None,
                        base_repository_provider_id: "7".into(),
                        source_repository_provider_id: "8".into(),
                        body_metadata_facet_revision: "9".into(),
                    },
                    file_key: "provider:1".into(),
                    path: "src/lib.rs".into(),
                    start_line: Some(10),
                    line: 12,
                    start_side: Some(ReviewDiffSide::Right),
                    side: ReviewDiffSide::Right,
                },
            }],
            content_hash: [0; 32],
            repository_native: "7".into(),
            repository_full_name: "acme/repo".into(),
            subject_native: "99".into(),
            number: "12".into(),
            actor_id: "5".into(),
        };
        payload.content_hash = content_hash(&payload).unwrap();
        payload
    }

    fn seal(payload: Payload) -> CommandSubmission {
        let request = payload.request.clone();
        seal_command(CommandDraft {
            command_id: request.command_id,
            account_id: request.context.account_id,
            authorization_epoch: request.context.authorization_epoch,
            target: CommandTarget::new(
                CommandTargetKind::PullRequest,
                request.context.subject_id,
                Some("repository".into()),
            )
            .unwrap(),
            payload,
            guards: vec![],
            dependencies: vec![],
        })
        .unwrap()
    }

    fn command(payload: Payload) -> DeliveryCommand {
        let sealed = seal(payload);
        DeliveryCommand {
            account_id: sealed.account_id().into(),
            command_id: sealed.command_id().into(),
            authorization_epoch: sealed.authorization_epoch().into(),
            operation_kind: sealed.operation().kind().into(),
            payload_version: sealed.operation().payload_version(),
            target_kind: sealed.target().kind().storage_name().into(),
            target_id: sealed.target().id().into(),
            repository_id: sealed.target().repository_id().map(str::to_owned),
            canonical_envelope: sealed.canonical_envelope().to_vec(),
            payload: sealed.payload_bytes().to_vec(),
            guards: vec![],
            hash: *sealed.submission_hash(),
            enqueue_order: 1,
            admitted_at: "2026-10-08T01:00:00.000000000Z".into(),
            state: DeliveryState::Queued,
            generation: 1,
            next_action_at: None,
            reconciliation_count: 0,
            attention: None,
            attempt_count: 0,
            quarantine_generation: 0,
            evidence: vec![],
        }
    }

    fn preparation(payload: &Payload, command: &DeliveryCommand) -> PreparationV1 {
        PreparationV1 {
            account_id: payload.request.context.account_id.clone(),
            command_id: payload.request.command_id.clone(),
            actor_id: payload.actor_id.clone(),
            authorization_epoch: payload.request.context.authorization_epoch.clone(),
            authorization_view: payload.request.context.authorization_view.clone(),
            repository_id: command.repository_id.clone().unwrap(),
            repository_provider_id: payload.repository_native.clone(),
            repository_full_name: payload.repository_full_name.clone(),
            subject_id: payload.request.context.subject_id.clone(),
            subject_provider_id: payload.subject_native.clone(),
            number: payload.number.clone(),
            context: ReviewContextV1::from(&payload.request.context.review_context),
            command_hash: command_hash(command),
            observed_at: "2026-10-08T01:01:00.000000000Z".into(),
        }
    }

    fn receipt(payload: &Payload) -> AcceptedReceiptV1 {
        AcceptedReceiptV1 {
            provider_id: "80".into(),
            url: "https://github.com/acme/repo/pull/12#pullrequestreview-80".into(),
            provider_state: expected_state(payload.event).into(),
            reviewed_commit_oid: payload.request.context.review_context.head_oid.clone(),
            submitted_at: "2026-10-08T01:02:00.000000000Z".into(),
            observed_at: "2026-10-08T01:03:00.000000000Z".into(),
        }
    }

    #[test]
    fn version_one_payload_round_trips_exact_content_and_authority() {
        let expected = payload();
        let sealed = seal(expected.clone());
        let decoded = decode_submission(&sealed).unwrap();
        assert_eq!(decoded, expected);
        assert_eq!(decoded.operation().context.head_oid, HEAD);
        assert_eq!(decoded.operation().comments[0].anchor.path, "src/lib.rs");
    }

    #[test]
    fn content_hash_and_consent_are_part_of_exact_retry_bytes() {
        let original = payload();
        let mut changed = original.clone();
        changed.body.push('!');
        changed.content_hash = content_hash(&changed).unwrap();
        assert!(seal(changed).payload_bytes() != seal(original.clone()).payload_bytes());

        let mut stale_hash = original.clone();
        stale_hash.body.push('!');
        assert!(stale_hash.validate().is_err());

        let mut missing_consent = original;
        missing_consent.request.accept_best_effort_race = false;
        assert!(missing_consent.validate().is_err());
    }

    #[test]
    fn frozen_nested_json_rejects_unknown_fields_and_unknown_sides() {
        let expected = payload();
        let mut content: serde_json::Value =
            serde_json::from_slice(&canonical_content(&expected).unwrap()).unwrap();
        content["future"] = serde_json::json!(true);
        assert!(decode_json::<ContentV1>(&serde_json::to_vec(&content).unwrap()).is_err());

        let mut content: ContentV1 =
            serde_json::from_slice(&canonical_content(&expected).unwrap()).unwrap();
        content.comments[0].anchor.side = "unknown".into();
        let bytes = encode_json(&content).unwrap();
        let mut decoded = expected;
        decoded.comments = decode_json::<ContentV1>(&bytes)
            .unwrap()
            .comments
            .into_iter()
            .map(ResolvedComment::from)
            .collect();
        decoded.content_hash = content_hash(&decoded).unwrap();
        assert!(decoded.validate().is_err());
    }

    #[test]
    fn accepted_and_submitted_proofs_bind_the_exact_command_and_ordered_comments() {
        let payload = payload();
        let command = command(payload.clone());
        let accepted = AcceptedEvidenceV1 {
            preparation: preparation(&payload, &command),
            receipt: receipt(&payload),
        };
        assert!(accepted_matches(&accepted, &payload, &command));
        let accepted_bytes = encode_evidence(&accepted).unwrap();
        assert_eq!(
            decode_evidence::<AcceptedEvidenceV1>(&accepted_bytes).unwrap(),
            accepted
        );

        let submitted = SubmittedEvidenceV1 {
            preparation: accepted.preparation.clone(),
            receipt: accepted.receipt.clone(),
            comments: vec![ConfirmedCommentV1 {
                comment_id: payload.comments[0].comment_id.clone(),
                provider_id: "901".into(),
            }],
            confirmed_at: "2026-10-08T01:04:00.000000000Z".into(),
        };
        assert!(submitted_matches(&submitted, &payload, &command));
        assert_ne!(
            encode_evidence(&accepted).unwrap(),
            encode_evidence(&submitted).unwrap()
        );
        assert!(encode_evidence(&submitted).unwrap().len() <= MAX_CODEC_BYTES);
    }

    #[test]
    fn proof_matching_rejects_foreign_commands_receipts_context_and_comment_identity() {
        let payload = payload();
        let command = command(payload.clone());
        let mut accepted = AcceptedEvidenceV1 {
            preparation: preparation(&payload, &command),
            receipt: receipt(&payload),
        };

        accepted.preparation.command_id = "00000000-0000-4000-8000-000000000099".into();
        assert!(!accepted_matches(&accepted, &payload, &command));
        accepted.preparation = preparation(&payload, &command);
        accepted.receipt.provider_id = "81".into();
        accepted.receipt.url = "https://github.com/acme/repo/pull/12#pullrequestreview-80".into();
        assert!(!accepted_matches(&accepted, &payload, &command));
        accepted.receipt = receipt(&payload);
        accepted.preparation.context.head_oid = BASE.into();
        assert!(!accepted_matches(&accepted, &payload, &command));
        accepted.preparation = preparation(&payload, &command);
        accepted.receipt.observed_at = "2026-10-08T01:00:00.000000000Z".into();
        assert!(!accepted_matches(&accepted, &payload, &command));

        let mut submitted = SubmittedEvidenceV1 {
            preparation: preparation(&payload, &command),
            receipt: receipt(&payload),
            comments: vec![ConfirmedCommentV1 {
                comment_id: "00000000-0000-4000-8000-000000000099".into(),
                provider_id: "901".into(),
            }],
            confirmed_at: "2026-10-08T01:04:00.000000000Z".into(),
        };
        assert!(!submitted_matches(&submitted, &payload, &command));
        submitted.comments[0].comment_id = payload.comments[0].comment_id.clone();
        submitted.receipt.url =
            "https://github.com:443/acme/repo/pull/12#pullrequestreview-80".into();
        assert!(!submitted_matches(&submitted, &payload, &command));
    }

    #[test]
    fn evidence_codec_rejects_noncanonical_and_oversized_values() {
        let payload = payload();
        let command = command(payload.clone());
        let accepted = AcceptedEvidenceV1 {
            preparation: preparation(&payload, &command),
            receipt: receipt(&payload),
        };
        let mut value: serde_json::Value =
            serde_json::from_slice(&encode_evidence(&accepted).unwrap()).unwrap();
        value["future"] = serde_json::json!(true);
        assert!(
            decode_evidence::<AcceptedEvidenceV1>(&serde_json::to_vec(&value).unwrap()).is_err()
        );

        let oversized = SubmittedEvidenceV1 {
            preparation: accepted.preparation,
            receipt: accepted.receipt,
            comments: (0..4_000)
                .map(|index| ConfirmedCommentV1 {
                    comment_id: format!("00000000-0000-4000-8000-{index:012}"),
                    provider_id: (index + 1).to_string(),
                })
                .collect(),
            confirmed_at: "2026-10-08T01:04:00.000000000Z".into(),
        };
        assert!(encode_evidence(&oversized).is_err());
    }
}
