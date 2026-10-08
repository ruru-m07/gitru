//! Exact GitHub.com transport and response codecs for final pull-request reviews.
//!
//! Durable admission and storage are wired after schema 23 lands. This module
//! owns the provider boundary: numeric routes, one final POST, strict response
//! identity, and read-only exact-ID confirmation.
#![allow(
    dead_code,
    reason = "schema-24 delivery policy consumes this frozen provider codec"
)]

use super::*;
use crate::{
    GithubReviewLineAnchor, ReviewDiffSide, ReviewSubmissionEvent, is_canonical_commit_oid,
};
use reqwest::{Method, StatusCode, Url};
use serde::Serialize;
use serde_json::{Map, Value};
use std::{collections::HashSet, sync::Arc};

const MAX_BODY_BYTES: usize = 16 * 1024;
const MAX_COMMENTS: usize = 25;
const MAX_AUTHORED_BYTES: usize = 128 * 1024;
const MAX_WIRE_BYTES: usize = 65_536;
const MAX_URL_BYTES: usize = 2_048;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GithubReviewComment {
    pub comment_id: String,
    pub body: String,
    pub anchor: GithubReviewLineAnchor,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GithubReviewSubmission {
    pub repository_provider_id: String,
    pub repository_full_name: String,
    pub subject_provider_id: String,
    pub number: String,
    pub actor_id: String,
    pub context: ReviewContext,
    pub event: ReviewSubmissionEvent,
    pub body: String,
    pub comments: Vec<GithubReviewComment>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FreshPullReviewTarget {
    pub repository_provider_id: String,
    pub subject_provider_id: String,
    pub number: String,
    pub base_oid: String,
    pub head_oid: String,
    pub base_repository_provider_id: String,
    pub source_repository_provider_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AcceptedReview {
    pub provider_id: String,
    pub url: String,
    pub provider_state: String,
    pub body: String,
    pub reviewed_commit_oid: String,
    pub submitted_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AcceptedReviewComment {
    pub provider_id: String,
    pub path: String,
    pub body: String,
    pub start_line: Option<u32>,
    pub line: u32,
    pub start_side: Option<ReviewDiffSide>,
    pub side: ReviewDiffSide,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ReviewCreateResponse {
    pub accepted: AcceptedReview,
    pub cooldown_seconds: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ReviewCreateOutcome {
    Accepted(ReviewCreateResponse),
    Rejected {
        status: u16,
        error: ProviderError,
        cooldown_seconds: Option<u64>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ReviewReadback {
    Confirmed {
        review: AcceptedReview,
        comments: Vec<AcceptedReviewComment>,
        cooldown_seconds: Option<u64>,
    },
    Deferred {
        cooldown_seconds: u64,
    },
}

pub(crate) struct GithubReviewTransport {
    http: GithubHttp,
}

impl GithubReviewTransport {
    pub(crate) fn new() -> Result<Self, CollaborationError> {
        Ok(Self {
            http: GithubHttp::new()?,
        })
    }

    #[cfg(test)]
    pub(crate) fn for_test_base(base: Url) -> Self {
        Self {
            http: GithubHttp::for_test_base(base).expect("review fixture transport"),
        }
    }

    pub(crate) async fn preflight(
        &self,
        token: &SecretToken,
        operation: &GithubReviewSubmission,
    ) -> Result<FreshPullReviewTarget, ProviderError> {
        operation.validate()?;
        let response = self
            .http
            .get_native_point_no_redirect(self.http.endpoint(&operation.pull_route()?)?, token)
            .await?;
        ensure_point(&response)?;
        if let Some(wait) = response.cooldown_seconds {
            return Err(rate_limited(wait));
        }
        parse_fresh_pull(operation, &response.body)
    }

    /// The caller must durably persist its final attempt before invoking this
    /// method. Any error after invocation is outcome-unknown and never permits
    /// another automatic POST.
    pub(crate) async fn create(
        &self,
        token: &SecretToken,
        operation: &GithubReviewSubmission,
    ) -> Result<ReviewCreateOutcome, ProviderError> {
        operation.validate()?;
        let response = self
            .http
            .mutate_native(
                token,
                Method::POST,
                self.http.endpoint(&operation.reviews_route()?)?,
                operation.wire_body()?,
            )
            .await?;
        if response.status != StatusCode::OK || response.provider_error.is_some() {
            let error = response
                .provider_error
                .unwrap_or_else(|| invalid().with_cooldown(response.cooldown_seconds));
            if known_rejection(response.status) {
                return Ok(ReviewCreateOutcome::Rejected {
                    status: response.status.as_u16(),
                    error,
                    cooldown_seconds: response.cooldown_seconds,
                });
            }
            return Err(error);
        }
        let accepted = parse_review(operation, &response.body)
            .map_err(|error| error.with_cooldown(response.cooldown_seconds))?;
        Ok(ReviewCreateOutcome::Accepted(ReviewCreateResponse {
            accepted,
            cooldown_seconds: response.cooldown_seconds,
        }))
    }

    /// Exact-ID readback only. A cooldown observed on the review response stops
    /// the inline-comment request and leaves accepted evidence unconfirmed.
    pub(crate) async fn readback(
        &self,
        token: &SecretToken,
        operation: &GithubReviewSubmission,
        review_id: &str,
    ) -> Result<ReviewReadback, ProviderError> {
        operation.validate()?;
        positive(review_id)?;
        let review = self
            .http
            .get_native_point_no_redirect(
                self.http.endpoint(&operation.review_route(review_id)?)?,
                token,
            )
            .await?;
        ensure_point(&review)?;
        let accepted = parse_review(operation, &review.body)
            .map_err(|error| error.with_cooldown(review.cooldown_seconds))?;
        if accepted.provider_id != review_id {
            return Err(invalid().with_cooldown(review.cooldown_seconds));
        }
        if operation.comments.is_empty() {
            return Ok(ReviewReadback::Confirmed {
                review: accepted,
                comments: vec![],
                cooldown_seconds: review.cooldown_seconds,
            });
        }
        if let Some(wait) = review.cooldown_seconds {
            return Ok(ReviewReadback::Deferred {
                cooldown_seconds: wait,
            });
        }

        let path = operation.review_comments_path(review_id)?;
        let comments = self
            .http
            .get_collection_with_page_size(
                self.http
                    .endpoint(&format!("{}?per_page=100", path.trim_start_matches('/')))?,
                token,
                &path,
                1,
                100,
            )
            .await?;
        if comments.not_modified || comments.next_url.is_some() {
            return Err(invalid().with_cooldown(comments.cooldown_seconds));
        }
        let parsed = parse_review_comments(operation, review_id, &comments.body)
            .map_err(|error| error.with_cooldown(comments.cooldown_seconds))?;
        Ok(ReviewReadback::Confirmed {
            review: accepted,
            comments: parsed,
            cooldown_seconds: comments.cooldown_seconds,
        })
    }
}

impl GithubReviewSubmission {
    pub(crate) fn validate(&self) -> Result<(), ProviderError> {
        positive(&self.repository_provider_id)?;
        positive(&self.subject_provider_id)?;
        positive(&self.number)?;
        positive(&self.actor_id)?;
        if !valid_repository_path(&self.repository_full_name)
            || !self.context.is_valid()
            || self.context.base_repository_provider_id != self.repository_provider_id
            || self.body.len() > MAX_BODY_BYTES
            || self.body.chars().any(|character| character == '\0')
            || self.comments.len() > MAX_COMMENTS
            || matches!(
                self.event,
                ReviewSubmissionEvent::Comment | ReviewSubmissionEvent::RequestChanges
            ) && self.body.is_empty()
        {
            return Err(invalid());
        }
        let mut bytes = self.body.len();
        let mut comment_ids = HashSet::new();
        let mut anchors = HashSet::new();
        for comment in &self.comments {
            let parsed = uuid::Uuid::parse_str(&comment.comment_id).map_err(|_| invalid())?;
            if parsed.hyphenated().to_string() != comment.comment_id
                || !comment_ids.insert(&comment.comment_id)
                || comment.body.is_empty()
                || comment.body.len() > MAX_BODY_BYTES
                || comment.body.chars().any(|character| character == '\0')
                || !anchor_valid(&comment.anchor, &self.context)
                || !anchors.insert(anchor_key(&comment.anchor))
            {
                return Err(invalid());
            }
            bytes = bytes.checked_add(comment.body.len()).ok_or_else(invalid)?;
        }
        if bytes > MAX_AUTHORED_BYTES {
            return Err(invalid());
        }
        if self.serialize_wire_body()?.len() > MAX_WIRE_BYTES {
            return Err(invalid());
        }
        Ok(())
    }

    fn pull_route(&self) -> Result<String, ProviderError> {
        self.validate_route_identity()?;
        Ok(format!(
            "repositories/{}/pulls/{}",
            self.repository_provider_id, self.number
        ))
    }

    fn reviews_route(&self) -> Result<String, ProviderError> {
        Ok(format!("{}/reviews", self.pull_route()?))
    }

    fn review_route(&self, review_id: &str) -> Result<String, ProviderError> {
        positive(review_id)?;
        Ok(format!("{}/reviews/{review_id}", self.pull_route()?))
    }

    fn review_comments_path(&self, review_id: &str) -> Result<String, ProviderError> {
        positive(review_id)?;
        Ok(format!(
            "/{}/reviews/{review_id}/comments",
            self.pull_route()?
        ))
    }

    fn validate_route_identity(&self) -> Result<(), ProviderError> {
        positive(&self.repository_provider_id)?;
        positive(&self.number)?;
        Ok(())
    }

    fn wire_body(&self) -> Result<Vec<u8>, ProviderError> {
        self.validate()?;
        self.serialize_wire_body()
    }

    fn serialize_wire_body(&self) -> Result<Vec<u8>, ProviderError> {
        #[derive(Serialize)]
        struct WireComment<'a> {
            path: &'a str,
            body: &'a str,
            line: u32,
            side: &'static str,
            #[serde(skip_serializing_if = "Option::is_none")]
            start_line: Option<u32>,
            #[serde(skip_serializing_if = "Option::is_none")]
            start_side: Option<&'static str>,
        }
        #[derive(Serialize)]
        struct WireReview<'a> {
            commit_id: &'a str,
            body: &'a str,
            event: &'static str,
            comments: Vec<WireComment<'a>>,
        }
        let comments = self
            .comments
            .iter()
            .map(|comment| WireComment {
                path: &comment.anchor.path,
                body: &comment.body,
                line: comment.anchor.line,
                side: side_wire(comment.anchor.side),
                start_line: comment.anchor.start_line,
                start_side: comment.anchor.start_side.map(side_wire),
            })
            .collect();
        serde_json::to_vec(&WireReview {
            commit_id: &self.context.head_oid,
            body: &self.body,
            event: event_wire(self.event),
            comments,
        })
        .map_err(|_| invalid())
    }
}

fn anchor_valid(anchor: &GithubReviewLineAnchor, context: &ReviewContext) -> bool {
    anchor.context.validate().is_ok()
        && anchor.context.base_oid == context.base_oid
        && anchor.context.head_oid == context.head_oid
        && anchor.context.base_repository_provider_id == context.base_repository_provider_id
        && anchor.context.source_repository_provider_id == context.source_repository_provider_id
        && anchor.context.body_metadata_facet_revision == context.metadata_facet_revision
        && positive(&anchor.file_facet_revision).is_ok()
        && !anchor.file_key.is_empty()
        && anchor.file_key.len() <= 256
        && anchor
            .file_key
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b':'))
        && crate::is_valid_pull_file_path(&anchor.path)
        && anchor.line > 0
        && matches!(anchor.side, ReviewDiffSide::Left | ReviewDiffSide::Right)
        && match (anchor.start_line, anchor.start_side) {
            (None, None) => true,
            (Some(start), Some(side)) => start > 0 && start < anchor.line && side == anchor.side,
            _ => false,
        }
}

fn anchor_key(
    anchor: &GithubReviewLineAnchor,
) -> (&str, Option<u32>, u32, Option<&'static str>, &'static str) {
    (
        &anchor.path,
        anchor.start_line,
        anchor.line,
        anchor.start_side.map(side_wire),
        side_wire(anchor.side),
    )
}

fn positive(raw: &str) -> Result<u64, ProviderError> {
    let value = raw.parse::<u64>().map_err(|_| invalid())?;
    if value == 0 || value.to_string() != raw || value > i64::MAX as u64 {
        return Err(invalid());
    }
    Ok(value)
}

fn invalid() -> ProviderError {
    ProviderError::new(ProviderErrorKind::InvalidResponse)
}

fn known_rejection(status: StatusCode) -> bool {
    matches!(
        status,
        StatusCode::UNAUTHORIZED
            | StatusCode::FORBIDDEN
            | StatusCode::NOT_FOUND
            | StatusCode::CONFLICT
            | StatusCode::UNPROCESSABLE_ENTITY
            | StatusCode::TOO_MANY_REQUESTS
    )
}

fn rate_limited(wait: u64) -> ProviderError {
    ProviderError {
        kind: ProviderErrorKind::RateLimited,
        retry_after_seconds: Some(wait.max(1)),
        account_cooldown_seconds: Some(wait.max(1)),
    }
}

fn ensure_point(response: &super::super::transport::HttpPage) -> Result<(), ProviderError> {
    if response.not_modified || response.next_url.is_some() {
        Err(invalid().with_cooldown(response.cooldown_seconds))
    } else {
        Ok(())
    }
}

fn event_wire(event: ReviewSubmissionEvent) -> &'static str {
    match event {
        ReviewSubmissionEvent::Comment => "COMMENT",
        ReviewSubmissionEvent::Approve => "APPROVE",
        ReviewSubmissionEvent::RequestChanges => "REQUEST_CHANGES",
    }
}

fn expected_state(event: ReviewSubmissionEvent) -> &'static str {
    match event {
        ReviewSubmissionEvent::Comment => "COMMENTED",
        ReviewSubmissionEvent::Approve => "APPROVED",
        ReviewSubmissionEvent::RequestChanges => "CHANGES_REQUESTED",
    }
}

fn side_wire(side: ReviewDiffSide) -> &'static str {
    match side {
        ReviewDiffSide::Left => "LEFT",
        ReviewDiffSide::Right => "RIGHT",
        ReviewDiffSide::Unknown => "UNKNOWN",
    }
}

fn side(raw: &str) -> Result<ReviewDiffSide, ProviderError> {
    match raw {
        "LEFT" => Ok(ReviewDiffSide::Left),
        "RIGHT" => Ok(ReviewDiffSide::Right),
        _ => Err(invalid()),
    }
}

fn object(body: &[u8]) -> Result<Map<String, Value>, ProviderError> {
    serde_json::from_slice(body).map_err(|_| invalid())
}

fn exact_api_url(raw: &str, paths: &[String]) -> Result<(), ProviderError> {
    if raw.is_empty()
        || raw.len() > MAX_URL_BYTES
        || raw
            .chars()
            .any(|character| character.is_control() || character.is_whitespace())
        || raw.contains(['%', '\\', '#'])
    {
        return Err(invalid());
    }
    let url = Url::parse(raw).map_err(|_| invalid())?;
    if url.as_str() != raw
        || url.scheme() != "https"
        || url.host_str() != Some("api.github.com")
        || url.port_or_known_default() != Some(443)
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || !paths.iter().any(|path| path == url.path())
    {
        return Err(invalid());
    }
    Ok(())
}

fn exact_web_review_url(
    raw: &str,
    operation: &GithubReviewSubmission,
    review_id: u64,
) -> Result<(), ProviderError> {
    if raw.is_empty()
        || raw.len() > MAX_URL_BYTES
        || raw
            .chars()
            .any(|character| character.is_control() || character.is_whitespace())
    {
        return Err(invalid());
    }
    let url = Url::parse(raw).map_err(|_| invalid())?;
    if url.as_str() != raw
        || url.scheme() != "https"
        || url.host_str() != Some("github.com")
        || url.port_or_known_default() != Some(443)
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.path()
            != format!(
                "/{}/pull/{}",
                operation.repository_full_name, operation.number
            )
        || url.fragment() != Some(format!("pullrequestreview-{review_id}").as_str())
    {
        return Err(invalid());
    }
    Ok(())
}

fn canonical_timestamp(raw: &str) -> Result<String, ProviderError> {
    if raw.is_empty() || raw.len() > 128 || raw.chars().any(char::is_control) {
        return Err(invalid());
    }
    let parsed = chrono::DateTime::parse_from_rfc3339(raw).map_err(|_| invalid())?;
    Ok(parsed
        .with_timezone(&chrono::Utc)
        .to_rfc3339_opts(chrono::SecondsFormat::Nanos, true))
}

fn parse_fresh_pull(
    operation: &GithubReviewSubmission,
    body: &[u8],
) -> Result<FreshPullReviewTarget, ProviderError> {
    let row = object(body)?;
    let native = |value: Option<&Value>| {
        value
            .and_then(Value::as_u64)
            .filter(|value| *value > 0 && *value <= i64::MAX as u64)
            .map(|value| value.to_string())
            .ok_or_else(invalid)
    };
    if native(row.get("id"))? != operation.subject_provider_id
        || native(row.get("number"))? != operation.number
        || row.get("state").and_then(Value::as_str) != Some("open")
        || row.get("merged").and_then(Value::as_bool) != Some(false)
    {
        return Err(invalid());
    }
    exact_api_url(
        row.get("url").and_then(Value::as_str).ok_or_else(invalid)?,
        &operation.pull_api_paths(),
    )?;
    let base_oid = nested_string(&row, &["base", "sha"])?;
    let head_oid = nested_string(&row, &["head", "sha"])?;
    let base_repository_provider_id = nested_positive(&row, &["base", "repo", "id"])?;
    let source_repository_provider_id = nested_positive(&row, &["head", "repo", "id"])?;
    if !is_canonical_commit_oid(base_oid)
        || !is_canonical_commit_oid(head_oid)
        || base_oid != operation.context.base_oid
        || head_oid != operation.context.head_oid
        || base_repository_provider_id != operation.context.base_repository_provider_id
        || source_repository_provider_id != operation.context.source_repository_provider_id
        || base_repository_provider_id != operation.repository_provider_id
    {
        return Err(invalid());
    }
    Ok(FreshPullReviewTarget {
        repository_provider_id: operation.repository_provider_id.clone(),
        subject_provider_id: operation.subject_provider_id.clone(),
        number: operation.number.clone(),
        base_oid: base_oid.into(),
        head_oid: head_oid.into(),
        base_repository_provider_id,
        source_repository_provider_id,
    })
}

fn parse_review(
    operation: &GithubReviewSubmission,
    body: &[u8],
) -> Result<AcceptedReview, ProviderError> {
    let row = object(body)?;
    let review_id = row
        .get("id")
        .and_then(Value::as_u64)
        .filter(|value| *value > 0 && *value <= i64::MAX as u64)
        .ok_or_else(invalid)?;
    if nested(&row, &["user", "id"])
        .and_then(Value::as_u64)
        .map(|value| value.to_string())
        .as_deref()
        != Some(operation.actor_id.as_str())
        || row.get("body").and_then(Value::as_str) != Some(operation.body.as_str())
        || row.get("state").and_then(Value::as_str) != Some(expected_state(operation.event))
        || row.get("commit_id").and_then(Value::as_str) != Some(operation.context.head_oid.as_str())
    {
        return Err(invalid());
    }
    exact_api_url(
        row.get("pull_request_url")
            .and_then(Value::as_str)
            .ok_or_else(invalid)?,
        &operation.pull_api_paths(),
    )?;
    let url = row
        .get("html_url")
        .and_then(Value::as_str)
        .ok_or_else(invalid)?;
    exact_web_review_url(url, operation, review_id)?;
    let submitted_at = canonical_timestamp(
        row.get("submitted_at")
            .and_then(Value::as_str)
            .ok_or_else(invalid)?,
    )?;
    Ok(AcceptedReview {
        provider_id: review_id.to_string(),
        url: url.into(),
        provider_state: expected_state(operation.event).into(),
        body: operation.body.clone(),
        reviewed_commit_oid: operation.context.head_oid.clone(),
        submitted_at,
    })
}

fn parse_review_comments(
    operation: &GithubReviewSubmission,
    review_id: &str,
    body: &[u8],
) -> Result<Vec<AcceptedReviewComment>, ProviderError> {
    let rows: Vec<Value> = serde_json::from_slice(body).map_err(|_| invalid())?;
    if rows.len() != operation.comments.len() || rows.len() > MAX_COMMENTS {
        return Err(invalid());
    }
    let review_id_number = positive(review_id)?;
    let mut matched = HashSet::new();
    let mut provider_ids = HashSet::new();
    let mut accepted = vec![None; rows.len()];
    for row in rows {
        let row = row.as_object().ok_or_else(invalid)?;
        let provider_id = row
            .get("id")
            .and_then(Value::as_u64)
            .filter(|value| *value > 0 && *value <= i64::MAX as u64)
            .ok_or_else(invalid)?;
        if !provider_ids.insert(provider_id)
            || row.get("pull_request_review_id").and_then(Value::as_u64) != Some(review_id_number)
            || nested(row, &["user", "id"])
                .and_then(Value::as_u64)
                .map(|value| value.to_string())
                .as_deref()
                != Some(operation.actor_id.as_str())
            || row.get("commit_id").and_then(Value::as_str)
                != Some(operation.context.head_oid.as_str())
            || row.get("original_commit_id").and_then(Value::as_str)
                != Some(operation.context.head_oid.as_str())
            || row
                .get("in_reply_to_id")
                .is_some_and(|value| !value.is_null())
        {
            return Err(invalid());
        }
        exact_api_url(
            row.get("pull_request_url")
                .and_then(Value::as_str)
                .ok_or_else(invalid)?,
            &operation.pull_api_paths(),
        )?;
        let path = row
            .get("path")
            .and_then(Value::as_str)
            .ok_or_else(invalid)?;
        let body = row
            .get("body")
            .and_then(Value::as_str)
            .ok_or_else(invalid)?;
        let line = u32_value(row.get("line"))?;
        let side = side(
            row.get("side")
                .and_then(Value::as_str)
                .ok_or_else(invalid)?,
        )?;
        let start_line = optional_u32(row.get("start_line"))?;
        let start_side = optional_side(row.get("start_side"))?;
        let requested = operation
            .comments
            .iter()
            .enumerate()
            .find(|(index, comment)| {
                !matched.contains(index)
                    && comment.body == body
                    && comment.anchor.path == path
                    && comment.anchor.line == line
                    && comment.anchor.side == side
                    && comment.anchor.start_line == start_line
                    && comment.anchor.start_side == start_side
            })
            .map(|(index, _)| index)
            .ok_or_else(invalid)?;
        matched.insert(requested);
        accepted[requested] = Some(AcceptedReviewComment {
            provider_id: provider_id.to_string(),
            path: path.into(),
            body: body.into(),
            start_line,
            line,
            start_side,
            side,
        });
    }
    if matched.len() != operation.comments.len() {
        return Err(invalid());
    }
    accepted
        .into_iter()
        .collect::<Option<Vec<_>>>()
        .ok_or_else(invalid)
}

impl GithubReviewSubmission {
    fn pull_api_paths(&self) -> Vec<String> {
        vec![
            format!("/repos/{}/pulls/{}", self.repository_full_name, self.number),
            format!(
                "/repositories/{}/pulls/{}",
                self.repository_provider_id, self.number
            ),
        ]
    }
}

fn nested<'a>(row: &'a Map<String, Value>, path: &[&str]) -> Option<&'a Value> {
    let mut value = row.get(*path.first()?)?;
    for key in &path[1..] {
        value = value.get(*key)?;
    }
    Some(value)
}

fn nested_string<'a>(row: &'a Map<String, Value>, path: &[&str]) -> Result<&'a str, ProviderError> {
    nested(row, path)
        .and_then(Value::as_str)
        .ok_or_else(invalid)
}

fn nested_positive(row: &Map<String, Value>, path: &[&str]) -> Result<String, ProviderError> {
    nested(row, path)
        .and_then(Value::as_u64)
        .filter(|value| *value > 0 && *value <= i64::MAX as u64)
        .map(|value| value.to_string())
        .ok_or_else(invalid)
}

fn u32_value(value: Option<&Value>) -> Result<u32, ProviderError> {
    value
        .and_then(Value::as_u64)
        .and_then(|value| u32::try_from(value).ok())
        .filter(|value| *value > 0)
        .ok_or_else(invalid)
}

fn optional_u32(value: Option<&Value>) -> Result<Option<u32>, ProviderError> {
    match value {
        None | Some(Value::Null) => Ok(None),
        Some(value) => u32_value(Some(value)).map(Some),
    }
}

fn optional_side(value: Option<&Value>) -> Result<Option<ReviewDiffSide>, ProviderError> {
    match value {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(value)) => side(value).map(Some),
        _ => Err(invalid()),
    }
}

/// Durable final-review delivery. Preparation performs the last authenticated
/// head check; dispatch is exactly one POST after the generic worker has
/// persisted its attempt. Accepted commands reconcile by exact native review ID
/// only and can never return to dispatch.
pub(crate) struct GithubReviewSubmissionPolicy {
    transport: GithubReviewTransport,
}

impl GithubReviewSubmissionPolicy {
    fn new() -> Result<Self, CollaborationError> {
        Ok(Self {
            transport: GithubReviewTransport::new()?,
        })
    }

    #[cfg(test)]
    pub(crate) fn for_test_base(base: Url) -> Self {
        Self {
            transport: GithubReviewTransport::for_test_base(base),
        }
    }
}

impl ProviderRegistry {
    pub fn register_github_review_submission(&mut self) -> Result<(), CollaborationError> {
        let policy = Arc::new(GithubReviewSubmissionPolicy::new()?);
        let instance = ProviderInstance::public(ProviderKind::Github);
        self.register_delivery(&instance, policy.clone())?;
        self.register_recovery(&instance, policy)
    }
}

fn local_now() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Nanos, true)
}

fn preparation(
    account: &RemoteAccount,
    command: &crate::delivery::DeliveryCommand,
    frame: &crate::review_submission::native::NativeFrameV1,
) -> crate::review_submission::native::PreparationV1 {
    crate::review_submission::native::PreparationV1 {
        account_id: account.id.clone(),
        command_id: command.command_id.clone(),
        actor_id: account.actor_id.clone(),
        authorization_epoch: account.authorization_epoch.clone(),
        authorization_view: frame.authorization_view.clone(),
        repository_id: frame.repository_id.clone(),
        repository_provider_id: frame.repository_provider_id.clone(),
        repository_full_name: frame.repository_full_name.clone(),
        subject_id: frame.subject_id.clone(),
        subject_provider_id: frame.subject_provider_id.clone(),
        number: frame.number.clone(),
        context: frame.context.clone(),
        command_hash: crate::review_submission::native::command_hash(command),
        observed_at: local_now(),
    }
}

fn receipt(
    review: AcceptedReview,
    observed_at: String,
) -> crate::review_submission::native::AcceptedReceiptV1 {
    crate::review_submission::native::AcceptedReceiptV1 {
        provider_id: review.provider_id,
        url: review.url,
        provider_state: review.provider_state,
        reviewed_commit_oid: review.reviewed_commit_oid,
        submitted_at: review.submitted_at,
        observed_at,
    }
}

fn proof<T: Serialize>(
    kind: &str,
    value: &T,
) -> Result<crate::delivery::OperationEvidence, CollaborationError> {
    Ok(crate::delivery::OperationEvidence {
        kind: kind.into(),
        version: 1,
        payload: crate::review_submission::native::encode_evidence(value)?,
    })
}

#[async_trait]
impl crate::delivery::CommandDeliveryPolicy for GithubReviewSubmissionPolicy {
    fn operation_kind(&self) -> &'static str {
        crate::review_submission::native::OPERATION
    }

    fn payload_version(&self) -> u32 {
        1
    }

    async fn prepare_context_in(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
        command: &crate::delivery::DeliveryCommand,
        account: &RemoteAccount,
    ) -> Result<Vec<u8>, CollaborationError> {
        crate::storage::review_submission::prepare_in(tx, command, account).await
    }

    async fn prepare(
        &self,
        token: &SecretToken,
        request: &crate::delivery::ReconcileRequest,
    ) -> Result<crate::delivery::DeliveryPreparation, ProviderError> {
        let payload =
            crate::review_submission::native::decode(&request.command).map_err(|_| invalid())?;
        let frame: crate::review_submission::native::NativeFrameV1 =
            crate::review_submission::native::decode_evidence(&request.native_context)
                .map_err(|_| invalid())?;
        if request.command.reconcile_only() {
            return Err(invalid());
        }
        self.transport
            .preflight(token, &payload.operation())
            .await?;
        Ok(crate::delivery::DeliveryPreparation {
            bytes: crate::review_submission::native::encode_evidence(&preparation(
                &request.account,
                &request.command,
                &frame,
            ))
            .map_err(|_| invalid())?,
            account_cooldown_seconds: None,
        })
    }

    async fn validate_claim(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
        command: &crate::delivery::DeliveryCommand,
        account: &RemoteAccount,
        bytes: &[u8],
    ) -> Result<crate::delivery::ClaimDecision, CollaborationError> {
        let prepared: crate::review_submission::native::PreparationV1 =
            crate::review_submission::native::decode_evidence(bytes)?;
        let payload = crate::review_submission::native::decode(command)?;
        if !crate::review_submission::native::preparation_matches(&prepared, &payload, command)
            || prepared.actor_id != account.actor_id
            || prepared.authorization_epoch != account.authorization_epoch
        {
            return Err(crate::review_submission::native::invalid());
        }
        let frame = crate::review_submission::native::NativeFrameV1 {
            repository_id: prepared.repository_id.clone(),
            repository_provider_id: prepared.repository_provider_id.clone(),
            repository_full_name: prepared.repository_full_name.clone(),
            subject_id: prepared.subject_id.clone(),
            subject_provider_id: prepared.subject_provider_id.clone(),
            number: prepared.number.clone(),
            authorization_view: prepared.authorization_view.clone(),
            context: prepared.context.clone(),
        };
        crate::storage::review_submission::validate_frame_in(tx, command, account, &frame).await?;
        Ok(crate::delivery::ClaimDecision::Ready(bytes.to_vec()))
    }

    fn validate_evidence(
        &self,
        command: &crate::delivery::DeliveryCommand,
        purpose: crate::delivery::EvidencePurpose,
        evidence: &crate::delivery::OperationEvidence,
    ) -> bool {
        use crate::{delivery::EvidencePurpose, review_submission::native as n};
        if evidence.version != 1 || !evidence.bounded() || command.attempt_count == 0 {
            return false;
        }
        let Ok(payload) = n::decode(command) else {
            return false;
        };
        match purpose {
            EvidencePurpose::Accepted if evidence.kind == n::ACCEPTED_PROOF => {
                n::decode_evidence::<n::AcceptedEvidenceV1>(&evidence.payload)
                    .is_ok_and(|value| n::accepted_matches(&value, &payload, command))
            }
            EvidencePurpose::Confirmed if evidence.kind == n::SUBMITTED_PROOF => {
                let Ok(value) = n::decode_evidence::<n::SubmittedEvidenceV1>(&evidence.payload)
                else {
                    return false;
                };
                n::submitted_matches(&value, &payload, command)
                    && command.evidence.iter().any(|recorded| {
                        recorded.evidence.kind == n::ACCEPTED_PROOF
                            && recorded.evidence.version == 1
                            && n::decode_evidence::<n::AcceptedEvidenceV1>(
                                &recorded.evidence.payload,
                            )
                            .is_ok_and(|accepted| {
                                n::accepted_matches(&accepted, &payload, command)
                                    && accepted.preparation == value.preparation
                                    && accepted.receipt == value.receipt
                            })
                    })
            }
            EvidencePurpose::Rejected if evidence.kind == n::REJECTED_PROOF => {
                n::decode_evidence::<n::RejectedEvidenceV1>(&evidence.payload)
                    .is_ok_and(|value| n::rejected_matches(&value, &payload, command))
            }
            _ => false,
        }
    }

    async fn finalize_in(
        &self,
        context: &mut crate::storage::effective::finalization::DeliveryFinalization<'_, '_>,
        command: &crate::delivery::DeliveryCommand,
        purpose: crate::delivery::EvidencePurpose,
        evidence: &crate::delivery::OperationEvidence,
    ) -> Result<(), CollaborationError> {
        if matches!(
            purpose,
            crate::delivery::EvidencePurpose::Accepted
                | crate::delivery::EvidencePurpose::Confirmed
        ) {
            crate::storage::review_submission::finalize_in(
                context.transaction(),
                command,
                purpose,
                evidence,
            )
            .await
        } else if purpose == crate::delivery::EvidencePurpose::Rejected {
            Ok(())
        } else {
            Err(crate::review_submission::native::invalid())
        }
    }

    async fn dispatch(
        &self,
        token: &SecretToken,
        request: crate::delivery::DispatchRequest,
    ) -> crate::delivery::DeliveryReport {
        use crate::{delivery::*, review_submission::native as n};
        let Ok(prepared) = n::decode_evidence::<n::PreparationV1>(&request.execution_base) else {
            return DeliveryReport::unknown();
        };
        let Ok(payload) = n::decode(&request.command) else {
            return DeliveryReport::unknown();
        };
        if !n::preparation_matches(&prepared, &payload, &request.command)
            || prepared.actor_id != request.account.actor_id
            || prepared.authorization_epoch != request.account.authorization_epoch
        {
            return DeliveryReport::unknown();
        }
        match self.transport.create(token, &payload.operation()).await {
            Ok(ReviewCreateOutcome::Accepted(created)) => {
                let accepted = n::AcceptedEvidenceV1 {
                    preparation: prepared,
                    receipt: receipt(created.accepted, local_now()),
                };
                let outcome = proof(n::ACCEPTED_PROOF, &accepted)
                    .ok()
                    .filter(|value| {
                        self.validate_evidence(&request.command, EvidencePurpose::Accepted, value)
                    })
                    .map(DeliveryOutcome::Accepted)
                    .unwrap_or(DeliveryOutcome::Unknown);
                DeliveryReport {
                    outcome,
                    retry_after_seconds: None,
                    account_cooldown_seconds: created.cooldown_seconds,
                    provider_error: None,
                }
            }
            Ok(ReviewCreateOutcome::Rejected {
                status,
                error,
                cooldown_seconds,
            }) => {
                let rejected = n::RejectedEvidenceV1 {
                    preparation: prepared,
                    status,
                    observed_at: local_now(),
                };
                let outcome = proof(n::REJECTED_PROOF, &rejected)
                    .ok()
                    .filter(|value| {
                        self.validate_evidence(&request.command, EvidencePurpose::Rejected, value)
                    })
                    .map(DeliveryOutcome::Rejected)
                    .unwrap_or(DeliveryOutcome::Unknown);
                DeliveryReport {
                    outcome,
                    retry_after_seconds: error.retry_after_seconds,
                    account_cooldown_seconds: cooldown_seconds,
                    provider_error: Some(error),
                }
            }
            Err(error) => DeliveryReport {
                retry_after_seconds: error.retry_after_seconds,
                account_cooldown_seconds: error.account_cooldown_seconds,
                provider_error: Some(error),
                ..DeliveryReport::unknown()
            },
        }
    }

    async fn reconcile(
        &self,
        token: &SecretToken,
        request: crate::delivery::ReconcileRequest,
    ) -> Result<crate::delivery::DeliveryReport, ProviderError> {
        use crate::{delivery::*, review_submission::native as n};
        let payload = n::decode(&request.command).map_err(|_| invalid())?;
        let accepted = request
            .command
            .evidence
            .iter()
            .filter(|recorded| {
                recorded.evidence.kind == n::ACCEPTED_PROOF && recorded.evidence.version == 1
            })
            .find_map(|recorded| {
                n::decode_evidence::<n::AcceptedEvidenceV1>(&recorded.evidence.payload)
                    .ok()
                    .filter(|value| n::accepted_matches(value, &payload, &request.command))
            })
            .ok_or_else(invalid)?;
        if accepted.preparation.actor_id != request.account.actor_id {
            return Err(invalid());
        }
        match self
            .transport
            .readback(token, &payload.operation(), &accepted.receipt.provider_id)
            .await?
        {
            ReviewReadback::Deferred { cooldown_seconds } => Ok(DeliveryReport {
                retry_after_seconds: Some(cooldown_seconds),
                account_cooldown_seconds: Some(cooldown_seconds),
                ..DeliveryReport::unknown()
            }),
            ReviewReadback::Confirmed {
                review,
                comments,
                cooldown_seconds,
            } => {
                let observed = receipt(review, accepted.receipt.observed_at.clone());
                if observed != accepted.receipt || comments.len() != payload.comments.len() {
                    return Err(invalid().with_cooldown(cooldown_seconds));
                }
                let evidence = n::SubmittedEvidenceV1 {
                    preparation: accepted.preparation,
                    receipt: accepted.receipt,
                    comments: payload
                        .comments
                        .iter()
                        .zip(comments)
                        .map(|(requested, observed)| n::ConfirmedCommentV1 {
                            comment_id: requested.comment_id.clone(),
                            provider_id: observed.provider_id,
                        })
                        .collect(),
                    confirmed_at: local_now(),
                };
                let proof = proof(n::SUBMITTED_PROOF, &evidence)
                    .map_err(|_| invalid().with_cooldown(cooldown_seconds))?;
                if !self.validate_evidence(&request.command, EvidencePurpose::Confirmed, &proof) {
                    return Err(invalid().with_cooldown(cooldown_seconds));
                }
                Ok(DeliveryReport {
                    outcome: DeliveryOutcome::Confirmed(proof),
                    retry_after_seconds: None,
                    account_cooldown_seconds: cooldown_seconds,
                    provider_error: None,
                })
            }
        }
    }
}

#[async_trait]
impl crate::command_recovery::policy::CommandRecoveryPolicy for GithubReviewSubmissionPolicy {
    fn instance_id(&self) -> &str {
        "github:https://github.com/"
    }

    fn operation_kind(&self) -> &'static str {
        crate::review_submission::native::OPERATION
    }

    fn payload_version(&self) -> u32 {
        1
    }

    async fn review_in(
        &self,
        _: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
        command: &crate::delivery::DeliveryCommand,
        _: &RemoteAccount,
    ) -> Result<crate::command_recovery::policy::NativeRecoveryReview, CollaborationError> {
        use crate::command_recovery::*;
        let payload = crate::review_submission::native::decode(command)?;
        Ok(policy::NativeRecoveryReview {
            fields: vec![CommandFieldReview {
                field: CommandReviewField::Body,
                base: CommandFieldValue {
                    known: true,
                    value: None,
                },
                remote: CommandFieldValue {
                    known: false,
                    value: None,
                },
                desired: CommandFieldValue {
                    known: true,
                    value: Some(payload.body),
                },
                comparison: CommandFieldComparison::Unknown,
                editable: false,
            }],
            can_replace: false,
            reason: Some(
                "A lost review response cannot be proved from body, actor or time. The saved summary and inline comments remain local; Gitru never posts this command again automatically."
                    .into(),
            ),
            fence: vec![],
        })
    }

    async fn replace_in(
        &self,
        _: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
        _: &crate::delivery::DeliveryCommand,
        _: &RemoteAccount,
        _: &crate::CommandRecoveryReplaceRequest,
        _: &crate::command_recovery::policy::NativeRecoveryReview,
    ) -> Result<crate::storage::command_admission::CommandReceipt, CollaborationError> {
        Err(crate::review_submission::native::invalid())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{PullFileContext, credentials::SecretToken};
    use serde_json::json;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::time::Duration;

    const BASE: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const HEAD: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
    const MERGE_BASE: &str = "cccccccccccccccccccccccccccccccccccccccc";

    fn operation() -> GithubReviewSubmission {
        GithubReviewSubmission {
            repository_provider_id: "7".into(),
            repository_full_name: "acme/repo".into(),
            subject_provider_id: "99".into(),
            number: "12".into(),
            actor_id: "5".into(),
            context: ReviewContext {
                base_oid: BASE.into(),
                head_oid: HEAD.into(),
                base_repository_provider_id: "7".into(),
                source_repository_provider_id: "8".into(),
                metadata_facet_revision: "41".into(),
            },
            event: ReviewSubmissionEvent::RequestChanges,
            body: "Please update the implementation.".into(),
            comments: vec![GithubReviewComment {
                comment_id: "00000000-0000-4000-8000-000000000001".into(),
                body: "This range needs a bound.".into(),
                anchor: GithubReviewLineAnchor {
                    file_facet_revision: "42".into(),
                    context: PullFileContext {
                        base_oid: BASE.into(),
                        head_oid: HEAD.into(),
                        merge_base_oid: Some(MERGE_BASE.into()),
                        base_repository_provider_id: "7".into(),
                        source_repository_provider_id: "8".into(),
                        body_metadata_facet_revision: "41".into(),
                    },
                    file_key: "provider:0001".into(),
                    path: "src/lib.rs".into(),
                    start_line: Some(10),
                    line: 12,
                    start_side: Some(ReviewDiffSide::Right),
                    side: ReviewDiffSide::Right,
                },
            }],
        }
    }

    fn pull_json() -> Value {
        json!({
            "id": 99,
            "number": 12,
            "url": "https://api.github.com/repositories/7/pulls/12",
            "state": "open",
            "merged": false,
            "base": {"sha": BASE, "repo": {"id": 7}},
            "head": {"sha": HEAD, "repo": {"id": 8}}
        })
    }

    fn review_json() -> Value {
        json!({
            "id": 80,
            "user": {"id": 5, "login": "reviewer"},
            "body": "Please update the implementation.",
            "state": "CHANGES_REQUESTED",
            "html_url": "https://github.com/acme/repo/pull/12#pullrequestreview-80",
            "pull_request_url": "https://api.github.com/repositories/7/pulls/12",
            "submitted_at": "2026-10-08T01:02:03Z",
            "commit_id": HEAD
        })
    }

    fn comment_json() -> Value {
        json!({
            "id": 901,
            "pull_request_review_id": 80,
            "user": {"id": 5, "login": "reviewer"},
            "body": "This range needs a bound.",
            "path": "src/lib.rs",
            "line": 12,
            "side": "RIGHT",
            "start_line": 10,
            "start_side": "RIGHT",
            "commit_id": HEAD,
            "original_commit_id": HEAD,
            "pull_request_url": "https://api.github.com/repositories/7/pulls/12"
        })
    }

    fn response(status: &str, body: &Value, extra_headers: &str) -> String {
        let body = serde_json::to_string(body).unwrap();
        format!(
            "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n{extra_headers}\r\n{body}",
            body.len()
        )
    }

    fn server(responses: Vec<String>) -> (Url, std::thread::JoinHandle<Vec<String>>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let base = Url::parse(&format!("http://{}/", listener.local_addr().unwrap())).unwrap();
        let task = std::thread::spawn(move || {
            let mut requests = Vec::with_capacity(responses.len());
            for response in responses {
                let (mut stream, _) = listener.accept().unwrap();
                stream.set_nonblocking(false).unwrap();
                stream
                    .set_read_timeout(Some(Duration::from_secs(2)))
                    .unwrap();
                let mut request = Vec::new();
                loop {
                    let mut bytes = [0u8; 1024];
                    let read = stream.read(&mut bytes).unwrap();
                    request.extend_from_slice(&bytes[..read]);
                    if read == 0 || request.windows(4).any(|bytes| bytes == b"\r\n\r\n") {
                        break;
                    }
                }
                stream
                    .set_write_timeout(Some(Duration::from_secs(2)))
                    .unwrap();
                stream.write_all(response.as_bytes()).unwrap();
                requests.push(String::from_utf8(request).unwrap());
            }
            requests
        });
        (base, task)
    }

    fn token() -> SecretToken {
        SecretToken::new("synthetic_review_token".into()).unwrap()
    }

    #[test]
    fn final_review_wire_uses_exact_head_and_modern_line_fields() {
        let operation = operation();
        operation.validate().unwrap();
        let wire: Value = serde_json::from_slice(&operation.wire_body().unwrap()).unwrap();
        assert_eq!(wire["commit_id"], HEAD);
        assert_eq!(wire["event"], "REQUEST_CHANGES");
        assert_eq!(wire["comments"][0]["path"], "src/lib.rs");
        assert_eq!(wire["comments"][0]["line"], 12);
        assert_eq!(wire["comments"][0]["start_line"], 10);
        assert_eq!(wire["comments"][0]["side"], "RIGHT");
        assert!(wire["comments"][0].get("position").is_none());
    }

    #[test]
    fn exact_encoded_wire_budget_includes_json_escaping_and_all_comments() {
        let mut near = operation();
        near.body = "x".into();
        let template = near.comments[0].clone();
        near.comments = (0..3)
            .map(|index| {
                let mut comment = template.clone();
                comment.comment_id = format!("00000000-0000-4000-8000-{:012}", index + 1);
                comment.body = "x".repeat(16_000);
                comment.anchor.path = format!("src/{index}.rs");
                comment.anchor.line += index;
                comment.anchor.start_line = comment.anchor.start_line.map(|line| line + index);
                comment
            })
            .collect();
        near.validate().unwrap();
        assert!(near.wire_body().unwrap().len() <= MAX_WIRE_BYTES);

        for comment in &mut near.comments {
            comment.body = "\n".repeat(12_000);
        }
        assert!(
            near.comments
                .iter()
                .map(|comment| comment.body.len())
                .sum::<usize>()
                < MAX_WIRE_BYTES
        );
        assert_eq!(
            near.validate().unwrap_err().kind,
            ProviderErrorKind::InvalidResponse
        );
    }

    #[test]
    fn renderer_anchor_and_authored_bounds_fail_closed() {
        let mut request = operation();
        request.comments[0].anchor.side = ReviewDiffSide::Unknown;
        assert_eq!(
            request.validate().unwrap_err().kind,
            ProviderErrorKind::InvalidResponse
        );

        let mut request = operation();
        request.comments.push(request.comments[0].clone());
        request.comments[1].comment_id = "00000000-0000-4000-8000-000000000002".into();
        assert_eq!(
            request.validate().unwrap_err().kind,
            ProviderErrorKind::InvalidResponse
        );

        let mut request = operation();
        request.context.head_oid = BASE.into();
        assert_eq!(
            request.validate().unwrap_err().kind,
            ProviderErrorKind::InvalidResponse
        );

        let mut request = operation();
        request.body.clear();
        assert_eq!(
            request.validate().unwrap_err().kind,
            ProviderErrorKind::InvalidResponse
        );
    }

    #[test]
    fn fresh_pull_review_and_inline_receipts_require_exact_provider_identity() {
        let operation = operation();
        let fresh =
            parse_fresh_pull(&operation, &serde_json::to_vec(&pull_json()).unwrap()).unwrap();
        assert_eq!(fresh.head_oid, HEAD);
        let review =
            parse_review(&operation, &serde_json::to_vec(&review_json()).unwrap()).unwrap();
        assert_eq!(review.provider_id, "80");
        assert_eq!(review.reviewed_commit_oid, HEAD);
        let comments = parse_review_comments(
            &operation,
            "80",
            &serde_json::to_vec(&json!([comment_json()])).unwrap(),
        )
        .unwrap();
        assert_eq!(comments.len(), 1);
        assert_eq!(comments[0].provider_id, "901");

        let mut wrong = review_json();
        wrong["commit_id"] = Value::String(BASE.into());
        assert!(parse_review(&operation, &serde_json::to_vec(&wrong).unwrap()).is_err());

        let mut extra = vec![comment_json(), comment_json()];
        extra[1]["id"] = json!(902);
        assert!(
            parse_review_comments(&operation, "80", &serde_json::to_vec(&extra).unwrap()).is_err()
        );
    }

    #[test]
    fn inline_receipts_are_returned_in_authored_comment_order() {
        let mut operation = operation();
        let mut second = operation.comments[0].clone();
        second.comment_id = "00000000-0000-4000-8000-000000000002".into();
        second.body = "Second authored comment.".into();
        second.anchor.path = "src/second.rs".into();
        second.anchor.line = 22;
        second.anchor.start_line = Some(20);
        operation.comments.push(second);

        let mut first_response = comment_json();
        let mut second_response = comment_json();
        second_response["id"] = json!(902);
        second_response["body"] = json!("Second authored comment.");
        second_response["path"] = json!("src/second.rs");
        second_response["line"] = json!(22);
        second_response["start_line"] = json!(20);
        let parsed = parse_review_comments(
            &operation,
            "80",
            &serde_json::to_vec(&json!([second_response, first_response.take()])).unwrap(),
        )
        .unwrap();
        assert_eq!(parsed[0].provider_id, "901");
        assert_eq!(parsed[0].path, "src/lib.rs");
        assert_eq!(parsed[1].provider_id, "902");
        assert_eq!(parsed[1].path, "src/second.rs");
    }

    #[tokio::test]
    async fn exact_numeric_preflight_and_single_create_preserve_wire_identity() {
        let responses = vec![
            response("200 OK", &pull_json(), ""),
            response("200 OK", &review_json(), ""),
        ];
        let (base, server) = server(responses);
        let transport = GithubReviewTransport::for_test_base(base);
        let operation = operation();
        let fresh = transport.preflight(&token(), &operation).await.unwrap();
        assert_eq!(fresh.head_oid, HEAD);
        let created = transport.create(&token(), &operation).await.unwrap();
        assert!(matches!(
            created,
            ReviewCreateOutcome::Accepted(ReviewCreateResponse {
                accepted: AcceptedReview { ref provider_id, .. },
                ..
            }) if provider_id == "80"
        ));
        let requests = server.join().unwrap();
        assert!(requests[0].starts_with("GET /repositories/7/pulls/12 HTTP/1.1"));
        assert!(requests[1].starts_with("POST /repositories/7/pulls/12/reviews HTTP/1.1"));
        let request_body = requests[1].split("\r\n\r\n").nth(1).unwrap();
        let wire: Value = serde_json::from_str(request_body).unwrap();
        assert_eq!(wire["commit_id"], HEAD);
        assert_eq!(wire["comments"][0]["path"], "src/lib.rs");
    }

    #[tokio::test]
    async fn review_cooldown_stops_inline_readback_before_a_second_request() {
        let responses = vec![response("200 OK", &review_json(), "Retry-After: 73\r\n")];
        let (base, server) = server(responses);
        let transport = GithubReviewTransport::for_test_base(base);
        let result = transport
            .readback(&token(), &operation(), "80")
            .await
            .unwrap();
        assert_eq!(
            result,
            ReviewReadback::Deferred {
                cooldown_seconds: 73
            }
        );
        let requests = server.join().unwrap();
        assert_eq!(requests.len(), 1);
        assert!(requests[0].starts_with("GET /repositories/7/pulls/12/reviews/80 HTTP/1.1"));
    }

    #[tokio::test]
    async fn validation_denial_is_a_known_rejection_without_receipt_authority() {
        let (base, server) = server(vec![response(
            "422 Unprocessable Entity",
            &json!({"message": "Validation Failed"}),
            "Retry-After: 31\r\n",
        )]);
        let transport = GithubReviewTransport::for_test_base(base);
        let outcome = transport.create(&token(), &operation()).await.unwrap();
        assert!(matches!(
            outcome,
            ReviewCreateOutcome::Rejected {
                status: 422,
                error: ProviderError {
                    kind: ProviderErrorKind::InvalidResponse,
                    ..
                },
                cooldown_seconds: Some(31),
            }
        ));
        let requests = server.join().unwrap();
        assert_eq!(requests.len(), 1);
        assert!(requests[0].starts_with("POST /repositories/7/pulls/12/reviews HTTP/1.1"));
    }

    #[tokio::test]
    async fn server_failure_after_post_is_outcome_unknown() {
        let (base, server) = server(vec![response(
            "500 Internal Server Error",
            &json!({"message": "unavailable"}),
            "Retry-After: 19\r\n",
        )]);
        let transport = GithubReviewTransport::for_test_base(base);
        let error = transport.create(&token(), &operation()).await.unwrap_err();
        assert_eq!(error.kind, ProviderErrorKind::Unavailable);
        assert_eq!(error.account_cooldown_seconds, Some(19));
        let requests = server.join().unwrap();
        assert_eq!(requests.len(), 1);
    }

    #[tokio::test]
    async fn unexpected_success_and_malformed_success_are_outcome_unknown() {
        for (status, body, wait) in [
            ("202 Accepted", review_json(), 17),
            ("200 OK", json!({}), 19),
        ] {
            let (base, server) = server(vec![response(
                status,
                &body,
                &format!("Retry-After: {wait}\r\n"),
            )]);
            let transport = GithubReviewTransport::for_test_base(base);
            let error = transport.create(&token(), &operation()).await.unwrap_err();
            assert_eq!(error.kind, ProviderErrorKind::InvalidResponse);
            assert_eq!(error.account_cooldown_seconds, Some(wait));
            let requests = server.join().unwrap();
            assert_eq!(requests.len(), 1);
            assert!(requests[0].starts_with("POST /repositories/7/pulls/12/reviews HTTP/1.1"));
        }
    }

    #[tokio::test]
    async fn redirect_after_post_is_outcome_unknown_without_following_or_replaying() {
        let (base, server) = server(vec![response(
            "302 Found",
            &json!({}),
            "Location: /repositories/7/pulls/12/reviews/80\r\nRetry-After: 23\r\n",
        )]);
        let transport = GithubReviewTransport::for_test_base(base);
        let error = transport.create(&token(), &operation()).await.unwrap_err();
        assert_eq!(error.kind, ProviderErrorKind::InvalidResponse);
        assert_eq!(error.account_cooldown_seconds, Some(23));
        let requests = server.join().unwrap();
        assert_eq!(requests.len(), 1);
        assert!(requests[0].starts_with("POST /repositories/7/pulls/12/reviews HTTP/1.1"));
    }

    #[tokio::test]
    async fn exact_review_and_terminal_comment_page_confirm_without_list_search() {
        let responses = vec![
            response("200 OK", &review_json(), ""),
            response("200 OK", &json!([comment_json()]), ""),
        ];
        let (base, server) = server(responses);
        let transport = GithubReviewTransport::for_test_base(base);
        let result = transport
            .readback(&token(), &operation(), "80")
            .await
            .unwrap();
        assert!(matches!(
            result,
            ReviewReadback::Confirmed { ref comments, .. } if comments.len() == 1
        ));
        let requests = server.join().unwrap();
        assert_eq!(requests.len(), 2);
        assert!(requests[0].starts_with("GET /repositories/7/pulls/12/reviews/80 HTTP/1.1"));
        assert!(
            requests[1].starts_with(
                "GET /repositories/7/pulls/12/reviews/80/comments?per_page=100 HTTP/1.1"
            )
        );
        assert!(
            requests
                .iter()
                .all(|request| !request.contains("/reviews?"))
        );
    }
}
