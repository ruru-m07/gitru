//! Bounded native task observations. Parent PR fields never order task facts.
use super::*;
use crate::{NativeDetailPayload, TaskActor, TaskV1};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::HashSet;

const MAX_PAGES: usize = 20;
const MAX_CURSOR: usize = 4096;
pub(super) const SOURCE: &str = "bitbucket.tasks.v1";
const FIELDS: [DetailField; 12] = [
    DetailField::TaskContent,
    DetailField::TaskCreatorLogin,
    DetailField::TaskCreatorDisplayName,
    DetailField::TaskState,
    DetailField::TaskCreatedAt,
    DetailField::TaskUpdatedAt,
    DetailField::TaskPending,
    DetailField::TaskResolvedAt,
    DetailField::TaskResolver,
    DetailField::TaskResolverLogin,
    DetailField::TaskResolverDisplayName,
    DetailField::TaskCommentId,
];

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Cursor {
    version: u8,
    strategy: String,
    account: String,
    epoch: String,
    subject: String,
    repository: String,
    pull: u64,
    url: String,
    pages: usize,
    seen_pages: Vec<String>,
}
#[derive(Deserialize)]
struct Collection {
    values: Vec<Map<String, Value>>,
    next: Option<String>,
}
impl Cursor {
    fn open(
        request: &DetailRequest,
        repository: &str,
        pull: u64,
        http: &BitbucketHttp,
        route: &Route,
    ) -> Result<Self, ProviderError> {
        let Some(raw) = &request.cursor else {
            return Ok(Self {
                version: 1,
                strategy: "uncertain_subject_history".into(),
                account: request.account.id.clone(),
                epoch: request.account.authorization_epoch.clone(),
                subject: request.subject.id.clone(),
                repository: repository.into(),
                pull,
                url: http.endpoint(route)?.to_string(),
                pages: 0,
                seen_pages: vec![],
            });
        };
        if raw.len() > MAX_CURSOR {
            return Err(invalid());
        }
        let cursor: Self = serde_json::from_str(raw).map_err(|_| invalid())?;
        if cursor.version != 1
            || cursor.strategy != "uncertain_subject_history"
            || cursor.account != request.account.id
            || cursor.epoch != request.account.authorization_epoch
            || cursor.subject != request.subject.id
            || cursor.repository != repository
            || cursor.pull != pull
            || cursor.pages == 0
            || cursor.pages > MAX_PAGES
            || cursor.seen_pages.len() != cursor.pages
        {
            return Err(invalid());
        }
        let mut hashes = HashSet::new();
        for hash in &cursor.seen_pages {
            if hash.len() != 64
                || !hash
                    .bytes()
                    .all(|b| b.is_ascii_digit() || matches!(b, b'a'..=b'f'))
                || !hashes.insert(hash.clone())
            {
                return Err(invalid());
            }
        }
        http.continuation(&cursor.url, route)?;
        if cursor.pages >= MAX_PAGES || hashes.contains(&http.fingerprint(&cursor.url, route)?) {
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
        let raw = serde_json::to_string(self).map_err(|_| invalid())?;
        if raw.len() > MAX_CURSOR {
            return Err(invalid());
        }
        Ok(raw)
    }
}
fn native_id(value: &Value) -> Result<u64, ProviderError> {
    value
        .as_u64()
        .filter(|id| *id > 0 && *id <= i64::MAX as u64)
        .ok_or_else(invalid)
}
fn subject_identity(request: &DetailRequest) -> Result<(String, u64), ProviderError> {
    let repository = resource_details::repository_identity(&request.account, &request.repository)?;
    let pull = request
        .subject
        .number
        .as_deref()
        .and_then(resource_details::positive_id)
        .filter(|id| *id <= i64::MAX as u64)
        .ok_or_else(invalid)?;
    if request.subject.kind != RemoteItemKind::PullRequest
        || request.subject.account_id != request.account.id
        || request.subject.repository_id.as_ref() != Some(&request.repository.id)
        || request.subject.provider_id != format!("{repository}:{pull}")
        || request.subject.id != format!("bitbucket_cloud:pull:{repository}:{pull}")
    {
        return Err(invalid());
    }
    Ok((repository, pull))
}
fn observed_text(
    object: &Map<String, Value>,
    key: &str,
    field: DetailField,
    maximum: usize,
    mask: &mut Vec<DetailField>,
) -> Result<Option<String>, ProviderError> {
    match object.get(key) {
        None => Ok(None),
        Some(Value::Null) => {
            mask.push(field);
            Ok(None)
        }
        Some(Value::String(value)) => {
            let value = text(value.clone(), maximum, true)?;
            mask.push(field);
            Ok(Some(value))
        }
        _ => Err(invalid()),
    }
}
fn actor(
    value: &Value,
    login: DetailField,
    display: DetailField,
    mask: &mut Vec<DetailField>,
) -> Result<TaskActor, ProviderError> {
    let object = value.as_object().ok_or_else(invalid)?;
    Ok(TaskActor {
        provider_id: canonical_uuid(
            object
                .get("uuid")
                .and_then(Value::as_str)
                .ok_or_else(invalid)?,
        )?,
        kind: text(
            object
                .get("type")
                .and_then(Value::as_str)
                .ok_or_else(invalid)?
                .into(),
            128,
            false,
        )?,
        login: observed_text(object, "nickname", login, 255, mask)?,
        display_name: observed_text(object, "display_name", display, 1024, mask)?,
    })
}
fn required_time(object: &Map<String, Value>, key: &str) -> Result<String, ProviderError> {
    let raw = text(
        object
            .get(key)
            .and_then(Value::as_str)
            .ok_or_else(invalid)?
            .into(),
        128,
        false,
    )?;
    chrono::DateTime::parse_from_rfc3339(&raw).map_err(|_| invalid())?;
    Ok(raw)
}
fn task(
    object: &Map<String, Value>,
    repository: &str,
    pull: u64,
) -> Result<DetailEntry, ProviderError> {
    let id = native_id(object.get("id").ok_or_else(invalid)?)?;
    let mut mask = vec![
        DetailField::TaskContent,
        DetailField::TaskState,
        DetailField::TaskCreatedAt,
        DetailField::TaskUpdatedAt,
    ];
    let content = object
        .get("content")
        .and_then(Value::as_object)
        .ok_or_else(invalid)?;
    let content = match content.get("raw") {
        None => DetailValue {
            state: DetailValueState::Omitted,
            text: None,
        },
        Some(Value::String(raw)) if raw.len() <= 65_536 => DetailValue {
            state: DetailValueState::Known,
            text: Some(raw.clone()),
        },
        Some(Value::String(_)) => DetailValue {
            state: DetailValueState::Oversized,
            text: None,
        },
        _ => return Err(invalid()),
    };
    let creator = actor(
        object.get("creator").ok_or_else(invalid)?,
        DetailField::TaskCreatorLogin,
        DetailField::TaskCreatorDisplayName,
        &mut mask,
    )?;
    let state = text(
        object
            .get("state")
            .and_then(Value::as_str)
            .ok_or_else(invalid)?
            .into(),
        128,
        false,
    )?;
    let created_at = required_time(object, "created_on")?;
    let updated_at = required_time(object, "updated_on")?;
    let pending = match object.get("pending") {
        None => None,
        Some(Value::Bool(value)) => {
            mask.push(DetailField::TaskPending);
            Some(*value)
        }
        _ => return Err(invalid()),
    };
    let resolved_at = observed_text(
        object,
        "resolved_on",
        DetailField::TaskResolvedAt,
        128,
        &mut mask,
    )?;
    if resolved_at
        .as_ref()
        .is_some_and(|raw| chrono::DateTime::parse_from_rfc3339(raw).is_err())
    {
        return Err(invalid());
    }
    let resolved_by = match object.get("resolved_by") {
        None => None,
        Some(Value::Null) => {
            mask.extend([
                DetailField::TaskResolver,
                DetailField::TaskResolverLogin,
                DetailField::TaskResolverDisplayName,
            ]);
            None
        }
        Some(value) => {
            mask.push(DetailField::TaskResolver);
            Some(actor(
                value,
                DetailField::TaskResolverLogin,
                DetailField::TaskResolverDisplayName,
                &mut mask,
            )?)
        }
    };
    let comment_id = match object.get("comment") {
        None => None,
        Some(Value::Null) => {
            mask.push(DetailField::TaskCommentId);
            None
        }
        Some(value) => {
            let value = value.as_object().ok_or_else(invalid)?;
            let id = native_id(value.get("id").ok_or_else(invalid)?)?;
            mask.push(DetailField::TaskCommentId);
            Some(id.to_string())
        }
    };
    Ok(DetailEntry {
        id: format!("bitbucket_cloud:task:{repository}:{pull}:{id}"),
        provider_id: format!("{repository}:{pull}:{id}"),
        native: Some(NativeDetailPayload::TaskV1(TaskV1 {
            observed_content_state: content.state,
            content,
            creator,
            state: Some(state),
            created_at: Some(created_at),
            updated_at: Some(updated_at),
            pending,
            resolved_at,
            resolved_by,
            comment_id,
        })),
        author: None,
        title: None,
        state: None,
        body: DetailValue::default(),
        observed_body_state: DetailValueState::NotLoaded,
        updated_at: None,
        head_oid: None,
        field_mask: mask,
        field_validations: vec![],
    })
}
impl BitbucketCloudProvider {
    pub(super) async fn tasks(
        &self,
        token: &SecretToken,
        request: DetailRequest,
    ) -> Result<DetailPage, ProviderError> {
        if request.facet != DetailFacet::Tasks
            || request.subject.kind != RemoteItemKind::PullRequest
        {
            return Err(ProviderError::new(ProviderErrorKind::Unsupported));
        }
        if request.etag.is_some() {
            return Err(invalid());
        }
        let (repository, pull) = subject_identity(&request)?;
        let route = Route::Tasks(repository.clone(), pull);
        let mut cursor = Cursor::open(&request, &repository, pull, &self.http, &route)?;
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
            if collection.values.len() > 50 {
                return Err(invalid());
            }
            let mut identities = HashSet::new();
            let mut entries = Vec::with_capacity(collection.values.len());
            for row in collection.values {
                let entry = task(&row, &repository, pull)?;
                if !identities.insert(entry.provider_id.clone()) {
                    return Err(invalid());
                }
                entries.push(entry);
            }
            let reconciliation = if cursor.pages == 0 && collection.next.is_none() {
                DetailReconciliation::full_history()
            } else {
                DetailReconciliation::default()
            };
            cursor
                .seen_pages
                .push(self.http.fingerprint(&cursor.url, &route)?);
            cursor.pages += 1;
            let next_cursor = collection
                .next
                .map(|next| {
                    cursor.url = self.http.continuation(&next, &route)?.to_string();
                    cursor.encoded(&self.http, &route)
                })
                .transpose()?;
            Ok(DetailPage {
                reconciliation,
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
                freshness_seconds: 180,
                cooldown_seconds: response.cooldown,
            })
        })();
        result.map_err(|error| quota(error, response.cooldown))
    }
}
