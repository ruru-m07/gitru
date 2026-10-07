//! Read-only pull-review summaries and anchored review comments.
use super::*;
use crate::NativeDetailPayload;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::HashSet;

const REVIEW_SOURCE: &str = "github/pull-reviews/2026-03-10";
const THREAD_SOURCE: &str = "github/pull-review-comments/2026-03-10";
const MAX_PAGES: u64 = 20;
const MAX_CURSOR_BYTES: usize = 4096;
const REVIEW_FIELDS: [DetailField; 6] = [
    DetailField::Body,
    DetailField::Author,
    DetailField::State,
    DetailField::UpdatedAt,
    DetailField::HeadOid,
    DetailField::Review,
];
const THREAD_FIELDS: [DetailField; 5] = [
    DetailField::Body,
    DetailField::Author,
    DetailField::UpdatedAt,
    DetailField::HeadOid,
    DetailField::ReviewThread,
];

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Cursor {
    version: u8,
    account: String,
    actor: String,
    epoch: String,
    repository: String,
    repository_native: String,
    subject: String,
    subject_native: String,
    number: u64,
    facet: DetailFacet,
    context: ReviewContext,
    pages: u64,
    url: String,
}

fn invalid() -> ProviderError {
    ProviderError::new(ProviderErrorKind::InvalidResponse)
}

fn positive(raw: &str) -> Result<u64, ProviderError> {
    let value = raw.parse::<u64>().map_err(|_| invalid())?;
    if value == 0 || value.to_string() != raw {
        return Err(invalid());
    }
    Ok(value)
}

fn native_id(value: &Value) -> Result<u64, ProviderError> {
    value
        .as_u64()
        .filter(|value| *value > 0)
        .ok_or_else(invalid)
}

fn identity(request: &ReviewRequest) -> Result<(String, u64), ProviderError> {
    let detail = &request.detail;
    let repository = positive(&detail.repository.provider_id)?;
    positive(&detail.subject.provider_id)?;
    let number = positive(detail.subject.number.as_deref().ok_or_else(invalid)?)?;
    positive(&detail.account.actor_id)?;
    positive(&detail.account.authorization_epoch)?;
    if detail.account.provider != ProviderKind::Github
        || detail.account.host != "github.com"
        || detail.account.state != AccountState::Active
        || detail.subject.kind != RemoteItemKind::PullRequest
        || !matches!(
            detail.facet,
            DetailFacet::ReviewSummaries | DetailFacet::ReviewThreads
        )
        || !crate::reviews::bounded_identity(&detail.account.id, 256)
        || detail.repository.account_id != detail.account.id
        || !crate::reviews::bounded_identity(&detail.repository.id, 256)
        || !detail.repository.selected
        || !valid_repository_path(&detail.repository.full_name)
        || detail.subject.account_id != detail.account.id
        || detail.subject.repository_id.as_ref() != Some(&detail.repository.id)
        || !crate::reviews::bounded_identity(&detail.subject.id, 256)
        || !request.context.is_valid()
        || detail.subject.head_oid.as_ref() != Some(&request.context.head_oid)
        || request.context.base_repository_provider_id != detail.repository.provider_id
    {
        return Err(invalid());
    }
    let collection = if detail.facet == DetailFacet::ReviewSummaries {
        "reviews"
    } else {
        "comments"
    };
    Ok((
        format!("/repositories/{repository}/pulls/{number}/{collection}"),
        number,
    ))
}

impl Cursor {
    fn open(
        request: &ReviewRequest,
        path: &str,
        number: u64,
        http: &GithubHttp,
    ) -> Result<Self, ProviderError> {
        let detail = &request.detail;
        let Some(raw) = &detail.cursor else {
            return Ok(Self {
                version: 1,
                account: detail.account.id.clone(),
                actor: detail.account.actor_id.clone(),
                epoch: detail.account.authorization_epoch.clone(),
                repository: detail.repository.id.clone(),
                repository_native: detail.repository.provider_id.clone(),
                subject: detail.subject.id.clone(),
                subject_native: detail.subject.provider_id.clone(),
                number,
                facet: detail.facet,
                context: request.context.clone(),
                pages: 0,
                url: http
                    .endpoint(&format!("{}?per_page=50", path.trim_start_matches('/')))?
                    .to_string(),
            });
        };
        if raw.len() > MAX_CURSOR_BYTES {
            return Err(invalid());
        }
        let cursor: Self = serde_json::from_str(raw).map_err(|_| invalid())?;
        if cursor.version != 1
            || cursor.account != detail.account.id
            || cursor.actor != detail.account.actor_id
            || cursor.epoch != detail.account.authorization_epoch
            || cursor.repository != detail.repository.id
            || cursor.repository_native != detail.repository.provider_id
            || cursor.subject != detail.subject.id
            || cursor.subject_native != detail.subject.provider_id
            || cursor.number != number
            || cursor.facet != detail.facet
            || cursor.context != request.context
            || cursor.pages == 0
            || cursor.pages >= MAX_PAGES
        {
            return Err(invalid());
        }
        let (_, next) = http.check_collection_url(&cursor.url, path)?;
        if next != cursor.pages + 1 {
            return Err(invalid());
        }
        Ok(cursor)
    }

    fn encoded(&self) -> Result<String, ProviderError> {
        let raw = serde_json::to_string(self).map_err(|_| invalid())?;
        if raw.len() > MAX_CURSOR_BYTES {
            return Err(invalid());
        }
        Ok(raw)
    }
}

fn api_url(raw: &str, paths: &[String]) -> Result<(), ProviderError> {
    if raw.len() > 2048
        || raw.chars().any(|c| c.is_control() || c.is_whitespace())
        || raw.contains(['%', '\\', '#'])
        || raw.split('/').any(|part| matches!(part, "." | ".."))
    {
        return Err(invalid());
    }
    let url = reqwest::Url::parse(raw).map_err(|_| invalid())?;
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

fn pull_url(raw: &str, request: &ReviewRequest, number: u64) -> Result<(), ProviderError> {
    api_url(
        raw,
        &[
            format!(
                "/repos/{}/pulls/{number}",
                request.detail.repository.full_name
            ),
            format!(
                "/repositories/{}/pulls/{number}",
                request.detail.repository.provider_id
            ),
        ],
    )
}

fn actor(value: &Value) -> Result<Option<ReviewActor>, ProviderError> {
    match value {
        Value::Null => Ok(None),
        Value::Object(user) => {
            let provider_id = native_id(user.get("id").ok_or_else(invalid)?)?.to_string();
            let login = user
                .get("login")
                .and_then(Value::as_str)
                .ok_or_else(invalid)?;
            if !crate::reviews::bounded_identity(login, 255) {
                return Err(invalid());
            }
            Ok(Some(ReviewActor {
                provider_id,
                login: Some(login.into()),
                display_name: None,
            }))
        }
        _ => Err(invalid()),
    }
}

fn body(row: &Map<String, Value>) -> Result<DetailValue, ProviderError> {
    match row.get("body").ok_or_else(invalid)? {
        Value::String(value) if value.len() <= 65_536 => Ok(DetailValue {
            state: DetailValueState::Known,
            text: Some(value.clone()),
        }),
        Value::String(_) => Ok(DetailValue {
            state: DetailValueState::Oversized,
            text: None,
        }),
        _ => Err(invalid()),
    }
}

fn timestamp(value: &str) -> Result<String, ProviderError> {
    if value.len() > 128 || chrono::DateTime::parse_from_rfc3339(value).is_err() {
        Err(invalid())
    } else {
        Ok(value.into())
    }
}

fn optional_timestamp(value: Option<&Value>) -> Result<Option<String>, ProviderError> {
    match value {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(value)) => timestamp(value).map(Some),
        _ => Err(invalid()),
    }
}

fn optional_oid(value: Option<&Value>) -> Result<Option<String>, ProviderError> {
    match value {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(value)) if crate::is_canonical_commit_oid(value) => {
            Ok(Some(value.into()))
        }
        _ => Err(invalid()),
    }
}

fn review(
    row: &Map<String, Value>,
    request: &ReviewRequest,
    number: u64,
) -> Result<DetailEntry, ProviderError> {
    let id = native_id(row.get("id").ok_or_else(invalid)?)?;
    pull_url(
        row.get("pull_request_url")
            .and_then(Value::as_str)
            .ok_or_else(invalid)?,
        request,
        number,
    )?;
    let reviewer = actor(row.get("user").ok_or_else(invalid)?)?;
    let state = row
        .get("state")
        .and_then(Value::as_str)
        .ok_or_else(invalid)?;
    if !crate::reviews::bounded_identity(state, 256) {
        return Err(invalid());
    }
    let decision = match state {
        "APPROVED" => ReviewDecision::Approved,
        "CHANGES_REQUESTED" => ReviewDecision::ChangesRequested,
        "COMMENTED" => ReviewDecision::Commented,
        "DISMISSED" => ReviewDecision::Dismissed,
        "PENDING" => ReviewDecision::Pending,
        _ => ReviewDecision::Unknown,
    };
    let submitted_at = optional_timestamp(row.get("submitted_at"))?;
    let reviewed_commit_oid = optional_oid(row.get("commit_id"))?;
    let body = body(row)?;
    Ok(DetailEntry {
        id: format!("github-review:{id:020}"),
        provider_id: id.to_string(),
        author: reviewer.as_ref().and_then(|actor| actor.login.clone()),
        title: None,
        state: Some(state.into()),
        observed_body_state: body.state,
        body,
        updated_at: submitted_at.clone(),
        head_oid: Some(request.context.head_oid.clone()),
        native: Some(NativeDetailPayload::ReviewV1(ReviewV1 {
            context: request.context.clone(),
            reviewer,
            decision,
            provider_state: state.into(),
            reviewed_commit_oid,
            submitted_at,
        })),
        field_mask: REVIEW_FIELDS.into(),
        field_validations: vec![],
    })
}

fn optional_positive(row: &Map<String, Value>, key: &str) -> Result<Option<u32>, ProviderError> {
    match row.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(value) => {
            let value = value
                .as_u64()
                .filter(|value| *value > 0 && *value <= u64::from(u32::MAX))
                .ok_or_else(invalid)?;
            Ok(Some(value as u32))
        }
    }
}

fn side(value: Option<&Value>) -> Result<Option<ReviewDiffSide>, ProviderError> {
    match value {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(value)) => Ok(Some(match value.as_str() {
            "LEFT" => ReviewDiffSide::Left,
            "RIGHT" => ReviewDiffSide::Right,
            _ if crate::reviews::bounded_identity(value, 64) => ReviewDiffSide::Unknown,
            _ => return Err(invalid()),
        })),
        _ => Err(invalid()),
    }
}

fn thread_comment(
    row: &Map<String, Value>,
    request: &ReviewRequest,
    number: u64,
) -> Result<DetailEntry, ProviderError> {
    let id = native_id(row.get("id").ok_or_else(invalid)?)?;
    pull_url(
        row.get("pull_request_url")
            .and_then(Value::as_str)
            .ok_or_else(invalid)?,
        request,
        number,
    )?;
    api_url(
        row.get("url").and_then(Value::as_str).ok_or_else(invalid)?,
        &[
            format!(
                "/repos/{}/pulls/comments/{id}",
                request.detail.repository.full_name
            ),
            format!(
                "/repositories/{}/pulls/comments/{id}",
                request.detail.repository.provider_id
            ),
        ],
    )?;
    let parent = match row.get("in_reply_to_id") {
        None | Some(Value::Null) => None,
        Some(value) => Some(native_id(value)?),
    };
    if parent == Some(id) {
        return Err(invalid());
    }
    let root = parent.unwrap_or(id);
    let review_id = match row.get("pull_request_review_id").ok_or_else(invalid)? {
        Value::Null => None,
        value => Some(native_id(value)?.to_string()),
    };
    let author = actor(row.get("user").ok_or_else(invalid)?)?;
    let created_at = timestamp(
        row.get("created_at")
            .and_then(Value::as_str)
            .ok_or_else(invalid)?,
    )?;
    let updated_at = timestamp(
        row.get("updated_at")
            .and_then(Value::as_str)
            .ok_or_else(invalid)?,
    )?;
    let path = row
        .get("path")
        .and_then(Value::as_str)
        .ok_or_else(invalid)?;
    if !crate::reviews::bounded_identity(path, 4096) {
        return Err(invalid());
    }
    let commit_oid = optional_oid(row.get("commit_id"))?.ok_or_else(invalid)?;
    let original_commit_oid = optional_oid(row.get("original_commit_id"))?.ok_or_else(invalid)?;
    let subject = match row.get("subject_type") {
        None | Some(Value::Null) => ReviewAnchorSubject::Unknown,
        Some(Value::String(value)) => match value.as_str() {
            "line" => ReviewAnchorSubject::Line,
            "file" => ReviewAnchorSubject::File,
            _ if crate::reviews::bounded_identity(value, 64) => ReviewAnchorSubject::Unknown,
            _ => return Err(invalid()),
        },
        _ => return Err(invalid()),
    };
    let body = body(row)?;
    Ok(DetailEntry {
        id: format!("github-review-thread:{root:020}:{id:020}"),
        provider_id: id.to_string(),
        author: author.as_ref().and_then(|actor| actor.login.clone()),
        title: None,
        state: None,
        observed_body_state: body.state,
        body,
        updated_at: Some(updated_at.clone()),
        head_oid: Some(request.context.head_oid.clone()),
        native: Some(NativeDetailPayload::ReviewThreadV1(ReviewThreadV1 {
            context: request.context.clone(),
            thread_id: root.to_string(),
            root_comment_id: Some(root.to_string()),
            comment_id: id.to_string(),
            parent_comment_id: parent.map(|value| value.to_string()),
            review_id,
            author,
            created_at,
            updated_at,
            anchor: Some(ReviewAnchor {
                path: path.into(),
                commit_oid,
                original_commit_oid,
                subject,
                start_line: optional_positive(row, "start_line")?,
                line: optional_positive(row, "line")?,
                start_side: side(row.get("start_side"))?,
                side: side(row.get("side"))?,
            }),
            provider_outdated: None,
            provider_resolved: None,
            native: None,
        })),
        field_mask: THREAD_FIELDS.into(),
        field_validations: vec![],
    })
}

impl GithubProvider {
    pub(super) async fn request_reviews(
        &self,
        token: &SecretToken,
        request: ReviewRequest,
    ) -> Result<DetailPage, ProviderError> {
        let (path, number) = identity(&request)?;
        let mut cursor = Cursor::open(&request, &path, number, &self.http)?;
        let response = self
            .http
            .get_collection(
                reqwest::Url::parse(&cursor.url).map_err(|_| invalid())?,
                token,
                &path,
                cursor.pages + 1,
            )
            .await?;
        let result = (|| {
            let rows: Vec<Map<String, Value>> =
                serde_json::from_slice(&response.body).map_err(|_| invalid())?;
            if rows.len() > 50 {
                return Err(invalid());
            }
            let mut identities = HashSet::new();
            let mut entries = Vec::with_capacity(rows.len());
            for row in rows {
                let entry = if request.detail.facet == DetailFacet::ReviewSummaries {
                    review(&row, &request, number)?
                } else {
                    thread_comment(&row, &request, number)?
                };
                if !identities.insert(entry.provider_id.clone()) {
                    return Err(invalid());
                }
                entries.push(entry);
            }
            let enumeration = if cursor.pages == 0 && response.next_url.is_none() {
                DetailEnumeration::FullEnumeration
            } else {
                DetailEnumeration::Uncertain
            };
            cursor.pages += 1;
            let next_cursor = response
                .next_url
                .as_ref()
                .map(|next| {
                    cursor.url = next.clone();
                    cursor.encoded()
                })
                .transpose()?;
            let fields = if request.detail.facet == DetailFacet::ReviewSummaries {
                REVIEW_FIELDS.to_vec()
            } else {
                THREAD_FIELDS.to_vec()
            };
            Ok(DetailPage {
                reconciliation: DetailReconciliation {
                    enumeration,
                    head_scope: DetailHeadScope::CurrentHead,
                },
                body: DetailValue::default(),
                metadata: None,
                entries,
                source: DetailSource {
                    source: if request.detail.facet == DetailFacet::ReviewSummaries {
                        REVIEW_SOURCE.into()
                    } else {
                        THREAD_SOURCE.into()
                    },
                    adapter_version: 1,
                    field_mask: fields,
                    provider_updated_at: None,
                    observed_at: chrono::Utc::now().to_rfc3339(),
                },
                next_cursor,
                etag: None,
                not_modified: false,
                freshness_seconds: 60,
                cooldown_seconds: response.cooldown_seconds,
            })
        })();
        result.map_err(|mut error: ProviderError| {
            error.account_cooldown_seconds = error
                .account_cooldown_seconds
                .into_iter()
                .chain(response.cooldown_seconds)
                .max();
            error
        })
    }
}
