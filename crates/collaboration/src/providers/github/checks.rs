//! Exact-head GitHub check runs and commit statuses.
use super::*;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

pub(super) const SOURCE: &str = "github/current-head-checks/2026-03-10";
const STRATEGY: &str = "latest_check_runs_and_current_statuses_v1";
const PAGE_SIZE: u64 = 50;
const MAX_FAMILY_ROWS: u64 = 500;
const MAX_PAGES: u64 = 20;
const MAX_CURSOR_BYTES: usize = 4096;
const FIELDS: [DetailField; 2] = [DetailField::Check, DetailField::HeadOid];
const CHECK_QUERY: [(&str, &str); 1] = [("filter", "latest")];

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Cursor {
    version: u8,
    strategy: String,
    account: String,
    actor: String,
    epoch: String,
    repository: String,
    repository_native: String,
    source_repository_native: String,
    subject: String,
    subject_native: String,
    head: String,
    metadata_revision: String,
    pages: u64,
    status_page: u64,
    status_seen: u64,
    status_total: u64,
    status_target: u64,
    status_next: Option<String>,
    check_page: u64,
    check_seen: u64,
    check_total: u64,
    check_target: u64,
    check_next: Option<String>,
    complete_possible: bool,
}

#[derive(Deserialize)]
struct StatusEnvelope {
    sha: String,
    total_count: u64,
    statuses: Vec<CommitStatus>,
}

#[derive(Deserialize)]
struct CommitStatus {
    id: u64,
    state: String,
    context: String,
    description: Option<String>,
    creator: Option<StatusCreator>,
    updated_at: String,
}

#[derive(Deserialize)]
struct StatusCreator {
    login: String,
}

#[derive(Deserialize)]
struct CheckEnvelope {
    total_count: u64,
    check_runs: Vec<CheckRun>,
}

#[derive(Deserialize)]
struct CheckRun {
    id: u64,
    name: String,
    head_sha: String,
    status: String,
    conclusion: Option<String>,
    started_at: Option<String>,
    completed_at: Option<String>,
    app: Option<CheckApp>,
    output: CheckOutput,
}

#[derive(Deserialize)]
struct CheckApp {
    name: String,
}

#[derive(Default, Deserialize)]
struct CheckOutput {
    title: Option<String>,
    summary: Option<String>,
}

fn invalid() -> ProviderError {
    ProviderError::new(ProviderErrorKind::InvalidResponse)
}

fn with_cooldown(mut error: ProviderError, cooldown: Option<u64>) -> ProviderError {
    error.account_cooldown_seconds = error
        .account_cooldown_seconds
        .into_iter()
        .chain(cooldown)
        .max();
    error
}

fn positive(raw: &str) -> Result<u64, ProviderError> {
    let value = raw.parse::<u64>().map_err(|_| invalid())?;
    if value == 0 || value.to_string() != raw {
        return Err(invalid());
    }
    Ok(value)
}

fn bounded_text(value: String, max: usize, nonempty: bool) -> Result<String, ProviderError> {
    if value.len() > max || nonempty && value.is_empty() || value.chars().any(char::is_control) {
        return Err(invalid());
    }
    Ok(value)
}

fn timestamp(value: Option<String>) -> Result<Option<String>, ProviderError> {
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

fn description(value: Option<String>) -> Result<DetailValue, ProviderError> {
    match value {
        None => Ok(DetailValue {
            state: DetailValueState::Omitted,
            text: None,
        }),
        Some(value) if value.len() <= 65_536 => Ok(DetailValue {
            state: DetailValueState::Known,
            text: Some(value),
        }),
        Some(_) => Ok(DetailValue {
            state: DetailValueState::Oversized,
            text: None,
        }),
    }
}

fn identity(request: &CheckRequest) -> Result<(String, String), ProviderError> {
    let detail = &request.detail;
    let source = positive(&request.context.source_repository_provider_id)?;
    positive(&detail.repository.provider_id)?;
    positive(&detail.subject.provider_id)?;
    positive(&detail.account.actor_id)?;
    positive(&detail.account.authorization_epoch)?;
    if detail.facet != DetailFacet::Checks
        || detail.account.provider != ProviderKind::Github
        || detail.account.host != "github.com"
        || detail.account.state != AccountState::Active
        || detail.repository.account_id != detail.account.id
        || !detail.repository.selected
        || detail.subject.kind != RemoteItemKind::PullRequest
        || detail.subject.account_id != detail.account.id
        || detail.subject.repository_id.as_ref() != Some(&detail.repository.id)
        || detail.subject.head_oid.as_deref() != Some(&request.context.head_oid)
        || !request.context.is_valid()
    {
        return Err(invalid());
    }
    let prefix = format!(
        "/repositories/{source}/commits/{}",
        request.context.head_oid
    );
    Ok((format!("{prefix}/status"), format!("{prefix}/check-runs")))
}

impl Cursor {
    fn initial(request: &CheckRequest) -> Self {
        Self {
            version: 1,
            strategy: STRATEGY.into(),
            account: request.detail.account.id.clone(),
            actor: request.detail.account.actor_id.clone(),
            epoch: request.detail.account.authorization_epoch.clone(),
            repository: request.detail.repository.id.clone(),
            repository_native: request.detail.repository.provider_id.clone(),
            source_repository_native: request.context.source_repository_provider_id.clone(),
            subject: request.detail.subject.id.clone(),
            subject_native: request.detail.subject.provider_id.clone(),
            head: request.context.head_oid.clone(),
            metadata_revision: request.context.metadata_facet_revision.clone(),
            pages: 0,
            status_page: 0,
            status_seen: 0,
            status_total: 0,
            status_target: 0,
            status_next: None,
            check_page: 0,
            check_seen: 0,
            check_total: 0,
            check_target: 0,
            check_next: None,
            complete_possible: false,
        }
    }

    fn open(request: &CheckRequest) -> Result<Self, ProviderError> {
        let Some(raw) = request.detail.cursor.as_ref() else {
            return Ok(Self::initial(request));
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
            || cursor.repository_native != request.detail.repository.provider_id
            || cursor.source_repository_native != request.context.source_repository_provider_id
            || cursor.subject != request.detail.subject.id
            || cursor.subject_native != request.detail.subject.provider_id
            || cursor.head != request.context.head_oid
            || cursor.metadata_revision != request.context.metadata_facet_revision
            || cursor.pages == 0
            || cursor.pages >= MAX_PAGES
            || cursor.status_seen > cursor.status_target
            || cursor.check_seen > cursor.check_target
            || cursor.status_target != cursor.status_total.min(MAX_FAMILY_ROWS)
            || cursor.check_target != cursor.check_total.min(MAX_FAMILY_ROWS)
            || cursor.status_target > MAX_FAMILY_ROWS
            || cursor.check_target > MAX_FAMILY_ROWS
            || cursor.complete_possible
                != (cursor.status_total <= MAX_FAMILY_ROWS && cursor.check_total <= MAX_FAMILY_ROWS)
            || cursor.status_next.is_none() && cursor.status_seen < cursor.status_target
            || cursor.check_next.is_none() && cursor.check_seen < cursor.check_target
        {
            return Err(invalid());
        }
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

fn validate_count(
    rows: usize,
    total: u64,
    seen: u64,
    next: &Option<String>,
) -> Result<(), ProviderError> {
    if rows > PAGE_SIZE as usize
        || seen > total
        || seen < total && next.is_none()
        || seen == total && next.is_some()
    {
        return Err(invalid());
    }
    Ok(())
}

fn status_entry(value: CommitStatus, head: &str) -> Result<DetailEntry, ProviderError> {
    if value.id == 0 {
        return Err(invalid());
    }
    let updated_at = timestamp(Some(value.updated_at))?;
    Ok(DetailEntry {
        id: format!("github-commit-status:{:020}", value.id),
        provider_id: format!("commit-status:{}", value.id),
        author: None,
        title: None,
        state: None,
        body: DetailValue::default(),
        observed_body_state: DetailValueState::NotLoaded,
        updated_at: None,
        head_oid: Some(head.into()),
        native: Some(crate::NativeDetailPayload::CheckV1(crate::CheckV1 {
            kind: crate::CheckKind::CommitStatus,
            name: bounded_text(value.context, 16_384, true)?,
            state: crate::CheckStateV1::CommitStatus {
                state: bounded_text(value.state, 256, true)?,
            },
            description: description(value.description)?,
            producer: value
                .creator
                .map(|creator| bounded_text(creator.login, 1_024, false))
                .transpose()?,
            started_at: None,
            completed_at: None,
            updated_at,
            allow_failure: None,
        })),
        field_mask: FIELDS.into(),
        field_validations: vec![],
    })
}

fn check_entry(value: CheckRun, head: &str) -> Result<DetailEntry, ProviderError> {
    if value.id == 0 || value.head_sha != head {
        return Err(invalid());
    }
    let detail = value.output.summary.or(value.output.title);
    Ok(DetailEntry {
        id: format!("github-check-run:{:020}", value.id),
        provider_id: format!("check-run:{}", value.id),
        author: None,
        title: None,
        state: None,
        body: DetailValue::default(),
        observed_body_state: DetailValueState::NotLoaded,
        updated_at: None,
        head_oid: Some(head.into()),
        native: Some(crate::NativeDetailPayload::CheckV1(crate::CheckV1 {
            kind: crate::CheckKind::CheckRun,
            name: bounded_text(value.name, 16_384, true)?,
            state: crate::CheckStateV1::CheckRun {
                status: bounded_text(value.status, 256, true)?,
                conclusion: value
                    .conclusion
                    .map(|value| bounded_text(value, 256, true))
                    .transpose()?,
            },
            description: description(detail)?,
            producer: value
                .app
                .map(|app| bounded_text(app.name, 1_024, false))
                .transpose()?,
            started_at: timestamp(value.started_at)?,
            completed_at: timestamp(value.completed_at)?,
            updated_at: None,
            allow_failure: None,
        })),
        field_mask: FIELDS.into(),
        field_validations: vec![],
    })
}

fn unique(entries: &[DetailEntry]) -> Result<(), ProviderError> {
    let mut ids = HashSet::with_capacity(entries.len());
    let mut provider_ids = HashSet::with_capacity(entries.len());
    if entries.iter().any(|entry| {
        !ids.insert(entry.id.as_str()) || !provider_ids.insert(entry.provider_id.as_str())
    }) {
        return Err(invalid());
    }
    Ok(())
}

impl GithubProvider {
    pub(super) async fn request_checks(
        &self,
        token: &SecretToken,
        request: CheckRequest,
    ) -> Result<DetailPage, ProviderError> {
        let (status_path, check_path) = identity(&request)?;
        if request.detail.etag.is_some() {
            return Err(invalid());
        }
        let mut cursor = Cursor::open(&request)?;
        let mut entries = Vec::new();
        let cooldown;

        if cursor.pages == 0 {
            let status_url = self.http.endpoint(&format!(
                "{}?per_page={PAGE_SIZE}",
                status_path.trim_start_matches('/')
            ))?;
            let check_url = self.http.endpoint(&format!(
                "{}?filter=latest&per_page={PAGE_SIZE}",
                check_path.trim_start_matches('/')
            ))?;
            let statuses = self
                .http
                .get_collection_with_page_size(status_url, token, &status_path, 1, PAGE_SIZE)
                .await?;
            if let Some(wait) = statuses.cooldown_seconds.filter(|wait| *wait > 0) {
                return Err(ProviderError {
                    kind: ProviderErrorKind::RateLimited,
                    retry_after_seconds: Some(wait),
                    account_cooldown_seconds: Some(wait),
                });
            }
            let checks = self
                .http
                .get_collection_with_fixed_query(
                    check_url,
                    token,
                    &check_path,
                    1,
                    PAGE_SIZE,
                    &CHECK_QUERY,
                )
                .await?;
            cooldown = statuses
                .cooldown_seconds
                .into_iter()
                .chain(checks.cooldown_seconds)
                .max();
            (|| {
                let statuses_body: StatusEnvelope =
                    serde_json::from_slice(&statuses.body).map_err(|_| invalid())?;
                let checks_body: CheckEnvelope =
                    serde_json::from_slice(&checks.body).map_err(|_| invalid())?;
                if statuses_body.sha != request.context.head_oid {
                    return Err(invalid());
                }
                cursor.status_page = 1;
                cursor.status_seen = statuses_body.statuses.len() as u64;
                cursor.status_total = statuses_body.total_count;
                cursor.status_target = statuses_body.total_count.min(MAX_FAMILY_ROWS);
                cursor.status_next = (cursor.status_seen < cursor.status_target)
                    .then_some(statuses.next_url.clone())
                    .flatten();
                cursor.check_page = 1;
                cursor.check_seen = checks_body.check_runs.len() as u64;
                cursor.check_total = checks_body.total_count;
                cursor.check_target = checks_body.total_count.min(MAX_FAMILY_ROWS);
                cursor.check_next = (cursor.check_seen < cursor.check_target)
                    .then_some(checks.next_url.clone())
                    .flatten();
                cursor.complete_possible = statuses_body.total_count <= MAX_FAMILY_ROWS
                    && checks_body.total_count <= MAX_FAMILY_ROWS;
                validate_count(
                    statuses_body.statuses.len(),
                    statuses_body.total_count,
                    cursor.status_seen,
                    &statuses.next_url,
                )?;
                validate_count(
                    checks_body.check_runs.len(),
                    checks_body.total_count,
                    cursor.check_seen,
                    &checks.next_url,
                )?;
                entries.extend(
                    statuses_body
                        .statuses
                        .into_iter()
                        .map(|value| status_entry(value, &request.context.head_oid))
                        .collect::<Result<Vec<_>, _>>()?,
                );
                entries.extend(
                    checks_body
                        .check_runs
                        .into_iter()
                        .map(|value| check_entry(value, &request.context.head_oid))
                        .collect::<Result<Vec<_>, _>>()?,
                );
                Ok(())
            })()
            .map_err(|error| with_cooldown(error, cooldown))?;
        } else if let Some(next) = cursor.status_next.take() {
            let response = self
                .http
                .get_collection_with_page_size(
                    reqwest::Url::parse(&next).map_err(|_| invalid())?,
                    token,
                    &status_path,
                    cursor.status_page + 1,
                    PAGE_SIZE,
                )
                .await?;
            cooldown = response.cooldown_seconds;
            (|| {
                let body: StatusEnvelope =
                    serde_json::from_slice(&response.body).map_err(|_| invalid())?;
                if body.sha != request.context.head_oid {
                    return Err(invalid());
                }
                if body.total_count != cursor.status_total {
                    return Err(invalid());
                }
                cursor.status_page += 1;
                cursor.status_seen = cursor
                    .status_seen
                    .checked_add(body.statuses.len() as u64)
                    .ok_or_else(invalid)?;
                if cursor.status_seen > cursor.status_target {
                    return Err(invalid());
                }
                if cursor.status_seen < cursor.status_target {
                    cursor.status_next = response.next_url.clone();
                }
                if cursor.complete_possible {
                    validate_count(
                        body.statuses.len(),
                        cursor.status_target,
                        cursor.status_seen,
                        &response.next_url,
                    )?;
                } else if cursor.status_seen < cursor.status_target && response.next_url.is_none() {
                    return Err(invalid());
                }
                entries = body
                    .statuses
                    .into_iter()
                    .map(|value| status_entry(value, &request.context.head_oid))
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(())
            })()
            .map_err(|error| with_cooldown(error, cooldown))?;
        } else if let Some(next) = cursor.check_next.take() {
            let response = self
                .http
                .get_collection_with_fixed_query(
                    reqwest::Url::parse(&next).map_err(|_| invalid())?,
                    token,
                    &check_path,
                    cursor.check_page + 1,
                    PAGE_SIZE,
                    &CHECK_QUERY,
                )
                .await?;
            cooldown = response.cooldown_seconds;
            (|| {
                let body: CheckEnvelope =
                    serde_json::from_slice(&response.body).map_err(|_| invalid())?;
                if body.total_count != cursor.check_total {
                    return Err(invalid());
                }
                cursor.check_page += 1;
                cursor.check_seen = cursor
                    .check_seen
                    .checked_add(body.check_runs.len() as u64)
                    .ok_or_else(invalid)?;
                if cursor.check_seen > cursor.check_target {
                    return Err(invalid());
                }
                if cursor.check_seen < cursor.check_target {
                    cursor.check_next = response.next_url.clone();
                }
                if cursor.complete_possible {
                    validate_count(
                        body.check_runs.len(),
                        cursor.check_target,
                        cursor.check_seen,
                        &response.next_url,
                    )?;
                } else if cursor.check_seen < cursor.check_target && response.next_url.is_none() {
                    return Err(invalid());
                }
                entries = body
                    .check_runs
                    .into_iter()
                    .map(|value| check_entry(value, &request.context.head_oid))
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(())
            })()
            .map_err(|error| with_cooldown(error, cooldown))?;
        } else {
            return Err(invalid());
        }

        (|| {
            cursor.pages += 1;
            if cursor.pages > MAX_PAGES {
                return Err(invalid());
            }
            unique(&entries)?;
            let next_cursor = if cursor.status_next.is_some() || cursor.check_next.is_some() {
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
                cooldown_seconds: cooldown,
            })
        })()
        .map_err(|error| with_cooldown(error, cooldown))
    }
}
