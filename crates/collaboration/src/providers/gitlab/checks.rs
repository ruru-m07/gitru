//! Exact-head GitLab commit statuses.
use super::*;
use serde::{Deserialize, Serialize};

pub(super) const SOURCE: &str = "gitlab/current-head-statuses/v4";
const STRATEGY: &str = "latest_status_per_name_offset_v1";
const PAGE_SIZE: u64 = 100;
const MAX_ROWS: u64 = 1_000;
const MAX_PAGES: u64 = 10;
const MAX_CURSOR_BYTES: usize = 4096;
const FIELDS: [DetailField; 2] = [DetailField::Check, DetailField::HeadOid];

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Cursor {
    version: u8,
    strategy: String,
    account: String,
    actor: String,
    epoch: String,
    repository: String,
    project: u64,
    subject: String,
    subject_native: String,
    head: String,
    source_project: String,
    metadata_revision: String,
    pages: u64,
    seen: u64,
    target: u64,
    total: Option<u64>,
    last_id: u64,
    complete_possible: bool,
    url: String,
}

#[derive(Deserialize)]
struct Status {
    id: u64,
    sha: String,
    status: String,
    name: String,
    description: Option<String>,
    author: Option<GitlabStatusAuthor>,
    created_at: String,
    started_at: Option<String>,
    finished_at: Option<String>,
    allow_failure: Option<bool>,
}

#[derive(Deserialize)]
struct GitlabStatusAuthor {
    username: String,
}

fn bounded_text(value: String, maximum: usize, nonempty: bool) -> Result<String, ProviderError> {
    if value.len() > maximum || nonempty && value.is_empty() || value.chars().any(char::is_control)
    {
        Err(invalid())
    } else {
        Ok(value)
    }
}

fn time(value: Option<String>) -> Result<Option<String>, ProviderError> {
    value
        .map(|value| {
            if value.len() > 128 || chrono::DateTime::parse_from_rfc3339(&value).is_err() {
                Err(invalid())
            } else {
                Ok(value)
            }
        })
        .transpose()
}

fn description(value: Option<String>) -> DetailValue {
    match value {
        None => DetailValue {
            state: DetailValueState::Omitted,
            text: None,
        },
        Some(value) if value.len() <= 65_536 => DetailValue {
            state: DetailValueState::Known,
            text: Some(value),
        },
        Some(_) => DetailValue {
            state: DetailValueState::Oversized,
            text: None,
        },
    }
}

fn normalized_state(value: String) -> Result<String, ProviderError> {
    let value = bounded_text(value, 256, true)?;
    Ok(match value.as_str() {
        "success" => "success".into(),
        "failed" | "canceled" => "failure".into(),
        "pending" | "running" => "pending".into(),
        _ => value,
    })
}

fn identity(request: &CheckRequest) -> Result<u64, ProviderError> {
    let detail = &request.detail;
    if detail.account.provider != ProviderKind::Gitlab || detail.account.host != "gitlab.com" {
        return Err(ProviderError::new(ProviderErrorKind::Unsupported));
    }
    if detail.account.state != AccountState::Active {
        return Err(ProviderError::new(ProviderErrorKind::Authentication));
    }
    let project = resource_details::repository_identity(&detail.account, &detail.repository)?;
    positive_id(&detail.account.actor_id).ok_or_else(invalid)?;
    positive_id(&detail.account.authorization_epoch).ok_or_else(invalid)?;
    positive_id(&detail.subject.provider_id).ok_or_else(invalid)?;
    positive_id(&request.context.source_repository_provider_id).ok_or_else(invalid)?;
    if detail.facet != DetailFacet::Checks
        || detail.subject.kind != RemoteItemKind::PullRequest
        || detail.subject.account_id != detail.account.id
        || detail.subject.repository_id.as_ref() != Some(&detail.repository.id)
        || detail.subject.head_oid.as_deref() != Some(&request.context.head_oid)
        || !crate::is_canonical_commit_oid(&request.context.head_oid)
        || !request.context.is_valid()
    {
        return Err(invalid());
    }
    Ok(project)
}

impl Cursor {
    fn open(
        request: &CheckRequest,
        project: u64,
        http: &GitlabHttp,
    ) -> Result<Self, ProviderError> {
        let Some(raw) = request.detail.cursor.as_ref() else {
            return Ok(Self {
                version: 1,
                strategy: STRATEGY.into(),
                account: request.detail.account.id.clone(),
                actor: request.detail.account.actor_id.clone(),
                epoch: request.detail.account.authorization_epoch.clone(),
                repository: request.detail.repository.id.clone(),
                project,
                subject: request.detail.subject.id.clone(),
                subject_native: request.detail.subject.provider_id.clone(),
                head: request.context.head_oid.clone(),
                source_project: request.context.source_repository_provider_id.clone(),
                metadata_revision: request.context.metadata_facet_revision.clone(),
                pages: 0,
                seen: 0,
                target: 0,
                total: None,
                last_id: 0,
                complete_possible: false,
                url: http
                    .commit_statuses(project, &request.context.head_oid)?
                    .to_string(),
            });
        };
        if raw.len() > MAX_CURSOR_BYTES {
            return Err(invalid());
        }
        let cursor: Self = serde_json::from_str(raw).map_err(|_| invalid())?;
        if cursor.version != 1
            || cursor.strategy != STRATEGY
            || cursor.account != request.detail.account.id
            || cursor.actor != request.detail.account.actor_id
            || cursor.epoch != request.detail.account.authorization_epoch
            || cursor.repository != request.detail.repository.id
            || cursor.project != project
            || cursor.subject != request.detail.subject.id
            || cursor.subject_native != request.detail.subject.provider_id
            || cursor.head != request.context.head_oid
            || cursor.source_project != request.context.source_repository_provider_id
            || cursor.metadata_revision != request.context.metadata_facet_revision
            || cursor.pages == 0
            || cursor.pages >= MAX_PAGES
            || cursor.seen == 0
            || cursor.seen > cursor.target
            || cursor.target > MAX_ROWS
            || cursor.complete_possible != cursor.total.is_some_and(|total| total <= MAX_ROWS)
        {
            return Err(invalid());
        }
        http.commit_status_continuation(&cursor.url, project, &cursor.head, cursor.pages + 1)?;
        Ok(cursor)
    }

    fn encoded(&self) -> Result<String, ProviderError> {
        let value = serde_json::to_string(self).map_err(|_| invalid())?;
        if value.len() > MAX_CURSOR_BYTES {
            return Err(invalid());
        }
        Ok(value)
    }

    fn reconciliation(&self) -> DetailReconciliation {
        DetailReconciliation {
            enumeration: if self.complete_possible {
                DetailEnumeration::FullEnumeration
            } else {
                DetailEnumeration::Uncertain
            },
            head_scope: DetailHeadScope::CurrentHead,
        }
    }
}

fn normalize(
    body: &[u8],
    head: &str,
    last_id: &mut u64,
) -> Result<Vec<DetailEntry>, ProviderError> {
    let rows: Vec<Status> = serde_json::from_slice(body).map_err(|_| invalid())?;
    if rows.len() > PAGE_SIZE as usize {
        return Err(invalid());
    }
    let mut entries = Vec::with_capacity(rows.len());
    for row in rows {
        if row.id <= *last_id || row.sha != head {
            return Err(invalid());
        }
        *last_id = row.id;
        entries.push(DetailEntry {
            id: format!("gitlab-commit-status:{:020}", row.id),
            provider_id: format!("commit-status:{}", row.id),
            author: None,
            title: None,
            state: None,
            body: DetailValue::default(),
            observed_body_state: DetailValueState::NotLoaded,
            updated_at: None,
            head_oid: Some(head.into()),
            native: Some(crate::NativeDetailPayload::CheckV1(crate::CheckV1 {
                kind: crate::CheckKind::CommitStatus,
                name: bounded_text(row.name, 16_384, true)?,
                state: crate::CheckStateV1::CommitStatus {
                    state: normalized_state(row.status)?,
                },
                description: description(row.description),
                producer: row
                    .author
                    .map(|author| bounded_text(author.username, 1_024, false))
                    .transpose()?,
                started_at: time(row.started_at)?,
                completed_at: time(row.finished_at)?,
                updated_at: time(Some(row.created_at))?,
                allow_failure: row.allow_failure,
            })),
            field_mask: FIELDS.into(),
            field_validations: vec![],
        });
    }
    Ok(entries)
}

impl GitlabProvider {
    pub(super) async fn request_checks(
        &self,
        token: &SecretToken,
        request: CheckRequest,
    ) -> Result<DetailPage, ProviderError> {
        let project = identity(&request)?;
        if request.detail.etag.is_some() {
            return Err(invalid());
        }
        let mut cursor = Cursor::open(&request, project, &self.http)?;
        let response = self
            .http
            .get(
                reqwest::Url::parse(&cursor.url).map_err(|_| invalid())?,
                token,
            )
            .await?;
        let result = (|| {
            if cursor.pages > 0 && response.total != cursor.total {
                return Err(invalid());
            }
            let entries = normalize(
                &response.body,
                &request.context.head_oid,
                &mut cursor.last_id,
            )?;
            cursor.pages += 1;
            cursor.seen = cursor
                .seen
                .checked_add(entries.len() as u64)
                .ok_or_else(invalid)?;
            if cursor.pages == 1 {
                cursor.total = response.total;
                cursor.target = response.total.unwrap_or(MAX_ROWS).min(MAX_ROWS);
                cursor.complete_possible = response.total.is_some_and(|total| total <= MAX_ROWS);
            }
            if cursor.seen > cursor.target
                || response.next.is_some() && entries.len() != PAGE_SIZE as usize
                || cursor
                    .total
                    .is_some_and(|total| response.next.is_none() && cursor.seen != total)
                || cursor
                    .total
                    .is_some_and(|total| response.next.is_some() && cursor.seen >= total)
            {
                return Err(invalid());
            }
            let next_cursor = if response.next.is_some() && cursor.seen < cursor.target {
                cursor.url = response.next.clone().ok_or_else(invalid)?;
                Some(cursor.encoded()?)
            } else {
                None
            };
            Ok(DetailPage {
                reconciliation: cursor.reconciliation(),
                body: DetailValue::default(),
                metadata: None,
                entries,
                source: DetailSource {
                    source: SOURCE.into(),
                    adapter_version: 1,
                    field_mask: FIELDS.into(),
                    provider_updated_at: None,
                    observed_at: chrono::Utc::now().to_rfc3339(),
                },
                next_cursor,
                etag: None,
                not_modified: false,
                freshness_seconds: 60,
                cooldown_seconds: response.cooldown,
            })
        })();
        result.map_err(|error| with_quota(error, response.cooldown))
    }
}
