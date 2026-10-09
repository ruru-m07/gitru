//! Exact-head Bitbucket Cloud commit build statuses.
use super::*;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashSet;

pub(super) const SOURCE: &str = "bitbucket_cloud/current-head-statuses/v2";
const STRATEGY: &str = "commit_build_statuses_v1";
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
    target_repository: String,
    source_repository: String,
    subject: String,
    subject_native: String,
    head: String,
    metadata_revision: String,
    pages: u64,
    seen: u64,
    target: u64,
    total: Option<u64>,
    complete_possible: bool,
    url: String,
    seen_pages: Vec<String>,
}

#[derive(Deserialize)]
struct Collection {
    values: Vec<Status>,
    next: Option<String>,
    size: Option<u64>,
    pagelen: Option<u64>,
    page: Option<u64>,
}

#[derive(Deserialize)]
struct Status {
    #[serde(rename = "type")]
    kind: String,
    key: String,
    name: String,
    state: String,
    description: Option<String>,
    created_on: String,
    updated_on: String,
    links: StatusLinks,
}

#[derive(Deserialize)]
struct StatusLinks {
    commit: Link,
}

#[derive(Deserialize)]
struct Link {
    href: String,
}

fn bounded_text(value: String, maximum: usize, nonempty: bool) -> Result<String, ProviderError> {
    if value.len() > maximum || nonempty && value.is_empty() || value.chars().any(char::is_control)
    {
        Err(invalid())
    } else {
        Ok(value)
    }
}

fn time(value: String) -> Result<String, ProviderError> {
    if value.len() > 128 || chrono::DateTime::parse_from_rfc3339(&value).is_err() {
        Err(invalid())
    } else {
        Ok(value)
    }
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

fn identity(request: &CheckRequest) -> Result<(String, String), ProviderError> {
    let detail = &request.detail;
    let target = resource_details::repository_identity(&detail.account, &detail.repository)?;
    let source = canonical_uuid(&request.context.source_repository_provider_id)?;
    let pull = detail
        .subject
        .number
        .as_deref()
        .and_then(resource_details::positive_id)
        .ok_or_else(invalid)?;
    if source != request.context.source_repository_provider_id
        || detail.facet != DetailFacet::Checks
        || detail.subject.kind != RemoteItemKind::PullRequest
        || detail.subject.account_id != detail.account.id
        || detail.subject.repository_id.as_ref() != Some(&detail.repository.id)
        || detail.subject.provider_id != format!("{target}:{pull}")
        || detail.subject.id != format!("bitbucket_cloud:pull:{target}:{pull}")
        || detail.subject.head_oid.as_deref() != Some(&request.context.head_oid)
        || !request.context.is_valid()
    {
        return Err(invalid());
    }
    Ok((target, source))
}

impl Cursor {
    fn open(
        request: &CheckRequest,
        target_repository: &str,
        source_repository: &str,
        http: &BitbucketHttp,
        route: &Route,
    ) -> Result<Self, ProviderError> {
        let Some(raw) = request.detail.cursor.as_ref() else {
            return Ok(Self {
                version: 1,
                strategy: STRATEGY.into(),
                account: request.detail.account.id.clone(),
                actor: request.detail.account.actor_id.clone(),
                epoch: request.detail.account.authorization_epoch.clone(),
                repository: request.detail.repository.id.clone(),
                target_repository: target_repository.into(),
                source_repository: source_repository.into(),
                subject: request.detail.subject.id.clone(),
                subject_native: request.detail.subject.provider_id.clone(),
                head: request.context.head_oid.clone(),
                metadata_revision: request.context.metadata_facet_revision.clone(),
                pages: 0,
                seen: 0,
                target: 0,
                total: None,
                complete_possible: false,
                url: http.endpoint(route)?.to_string(),
                seen_pages: vec![],
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
            || cursor.target_repository != target_repository
            || cursor.source_repository != source_repository
            || cursor.subject != request.detail.subject.id
            || cursor.subject_native != request.detail.subject.provider_id
            || cursor.head != request.context.head_oid
            || cursor.metadata_revision != request.context.metadata_facet_revision
            || cursor.pages == 0
            || cursor.pages >= MAX_PAGES
            || cursor.seen == 0
            || cursor.seen > cursor.target
            || cursor.target > MAX_ROWS
            || cursor.target
                != cursor
                    .total
                    .map(|total| total.min(MAX_ROWS))
                    .unwrap_or(MAX_ROWS)
            || cursor.complete_possible != cursor.total.is_some_and(|total| total <= MAX_ROWS)
            || cursor.seen_pages.len() != cursor.pages as usize
        {
            return Err(invalid());
        }
        let mut hashes = HashSet::new();
        if cursor.seen_pages.iter().any(|hash| {
            hash.len() != 64
                || !hash
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
                || !hashes.insert(hash)
        }) {
            return Err(invalid());
        }
        http.continuation(&cursor.url, route)?;
        if hashes.contains(&http.fingerprint(&cursor.url, route)?) {
            return Err(invalid());
        }
        Ok(cursor)
    }

    fn encoded(&self, http: &BitbucketHttp, route: &Route) -> Result<String, ProviderError> {
        if self
            .seen_pages
            .contains(&http.fingerprint(&self.url, route)?)
        {
            return Err(invalid());
        }
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

fn normalized_state(raw: String) -> Result<String, ProviderError> {
    let raw = bounded_text(raw, 256, true)?;
    Ok(match raw.as_str() {
        "SUCCESSFUL" => "success".into(),
        "FAILED" => "failure".into(),
        "INPROGRESS" => "pending".into(),
        "STOPPED" => "error".into(),
        _ => raw,
    })
}

fn entry(
    status: Status,
    source_repository: &str,
    head: &str,
    http: &BitbucketHttp,
) -> Result<DetailEntry, ProviderError> {
    if status.kind != "build" {
        return Err(invalid());
    }
    http.commit_link(&status.links.commit.href, source_repository, head)?;
    let key = bounded_text(status.key, 255, true)?;
    let id = format!("{:x}", Sha256::digest(key.as_bytes()));
    let created_at = time(status.created_on)?;
    let updated_at = time(status.updated_on)?;
    let state = normalized_state(status.state)?;
    let completed_at =
        matches!(state.as_str(), "success" | "failure" | "error").then_some(updated_at.clone());
    Ok(DetailEntry {
        id: format!("bitbucket-cloud-commit-status:{id}"),
        provider_id: format!("commit-status:{key}"),
        author: None,
        title: None,
        state: None,
        body: DetailValue::default(),
        observed_body_state: DetailValueState::NotLoaded,
        updated_at: None,
        head_oid: Some(head.into()),
        native: Some(crate::NativeDetailPayload::CheckV1(crate::CheckV1 {
            kind: crate::CheckKind::CommitStatus,
            name: bounded_text(status.name, 16_384, true)?,
            state: crate::CheckStateV1::CommitStatus { state },
            description: description(status.description),
            producer: None,
            started_at: Some(created_at),
            completed_at,
            updated_at: Some(updated_at),
            allow_failure: None,
        })),
        field_mask: FIELDS.into(),
        field_validations: vec![],
    })
}

impl BitbucketCloudProvider {
    pub(super) async fn request_checks(
        &self,
        token: &SecretToken,
        request: CheckRequest,
    ) -> Result<DetailPage, ProviderError> {
        let (target_repository, source_repository) = identity(&request)?;
        if request.detail.etag.is_some() {
            return Err(invalid());
        }
        let route = Route::Statuses(source_repository.clone(), request.context.head_oid.clone());
        let mut cursor = Cursor::open(
            &request,
            &target_repository,
            &source_repository,
            &self.http,
            &route,
        )?;
        let response = self
            .http
            .get(
                reqwest::Url::parse(&cursor.url).map_err(|_| invalid())?,
                &route,
                token,
            )
            .await?;
        let result = (|| {
            let collection: Collection =
                serde_json::from_slice(&response.body).map_err(|_| invalid())?;
            if collection.values.len() > PAGE_SIZE as usize
                || collection.pagelen.is_some_and(|value| value != PAGE_SIZE)
                || collection
                    .page
                    .is_some_and(|value| value != cursor.pages + 1)
                || cursor.pages > 0 && collection.size != cursor.total
            {
                return Err(invalid());
            }
            if cursor.pages == 0 {
                cursor.total = collection.size;
                cursor.target = collection
                    .size
                    .map(|total| total.min(MAX_ROWS))
                    .unwrap_or(MAX_ROWS);
                cursor.complete_possible = collection.size.is_some_and(|total| total <= MAX_ROWS);
            }
            let mut ids = HashSet::new();
            let mut provider_ids = HashSet::new();
            let entries = collection
                .values
                .into_iter()
                .map(|status| {
                    entry(
                        status,
                        &source_repository,
                        &request.context.head_oid,
                        &self.http,
                    )
                })
                .collect::<Result<Vec<_>, _>>()?;
            if entries.iter().any(|value| {
                !ids.insert(value.id.as_str()) || !provider_ids.insert(value.provider_id.as_str())
            }) {
                return Err(invalid());
            }
            cursor.seen = cursor
                .seen
                .checked_add(entries.len() as u64)
                .ok_or_else(invalid)?;
            cursor.pages += 1;
            cursor
                .seen_pages
                .push(self.http.fingerprint(&cursor.url, &route)?);
            let has_next = collection.next.is_some();
            if cursor.seen > cursor.target
                || has_next && entries.len() != PAGE_SIZE as usize
                || cursor.complete_possible && (has_next != (cursor.seen < cursor.target))
                || !cursor.complete_possible && cursor.seen < cursor.target && !has_next
                || cursor.total.is_some_and(|total| total > MAX_ROWS)
                    && cursor.seen == cursor.target
                    && !has_next
            {
                return Err(invalid());
            }
            let next_cursor = if has_next && cursor.seen < cursor.target {
                cursor.url = self
                    .http
                    .continuation(collection.next.as_deref().ok_or_else(invalid)?, &route)?
                    .to_string();
                Some(cursor.encoded(&self.http, &route)?)
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
        result.map_err(|error| quota(error, response.cooldown))
    }
}
