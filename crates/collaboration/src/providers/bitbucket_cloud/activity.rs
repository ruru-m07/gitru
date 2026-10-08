//! Bounded historical observations; never current workflow or review authority.
use super::*;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
const MAX_PAGES: usize = 20;
const MAX_CURSOR: usize = 4096;
pub(super) const SOURCE: &str = "bitbucket.activity.observations.v1";
const FIELDS: [DetailField; 4] = [
    DetailField::Body,
    DetailField::Author,
    DetailField::UpdatedAt,
    DetailField::Activity,
];
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Cursor {
    version: u8,
    strategy: String,
    account: String,
    epoch: String,
    actor: String,
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
    page: Option<u64>,
    pagelen: Option<u64>,
    size: Option<u64>,
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
                strategy: SOURCE.into(),
                account: request.account.id.clone(),
                epoch: request.account.authorization_epoch.clone(),
                actor: request.account.actor_id.clone(),
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
            || cursor.strategy != SOURCE
            || cursor.account != request.account.id
            || cursor.epoch != request.account.authorization_epoch
            || cursor.actor != request.account.actor_id
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
        continuation(http, &cursor.url, route, cursor.pages)?;
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

fn continuation(
    http: &BitbucketHttp,
    raw: &str,
    route: &Route,
    completed: usize,
) -> Result<reqwest::Url, ProviderError> {
    let url = http.continuation(raw, route)?;
    let paging: Vec<_> = url
        .query_pairs()
        .filter(|(k, _)| matches!(k.as_ref(), "page" | "cursor" | "after" | "before"))
        .collect();
    if paging.len() != 1 {
        return Err(invalid());
    }
    if paging[0].0 == "page" && paging[0].1 != (completed + 1).to_string() {
        return Err(invalid());
    }
    Ok(url)
}
fn timestamp(value: &Value) -> Result<String, ProviderError> {
    let raw = value
        .as_str()
        .filter(|s| s.len() <= 128)
        .ok_or_else(invalid)?;
    chrono::DateTime::parse_from_rfc3339(raw)
        .map(|date| {
            date.with_timezone(&chrono::Utc)
                .to_rfc3339_opts(chrono::SecondsFormat::Nanos, true)
        })
        .map_err(|_| invalid())
}
fn actor(value: &Value) -> Result<(String, Option<String>), ProviderError> {
    let value = value.as_object().ok_or_else(invalid)?;
    let id = canonical_uuid(
        value
            .get("uuid")
            .and_then(Value::as_str)
            .ok_or_else(invalid)?,
    )?;
    let name = value.get("nickname").or_else(|| value.get("display_name"));
    let name = match name {
        None | Some(Value::Null) => None,
        Some(Value::String(name)) => Some(text(name.clone(), 255, false)?),
        _ => return Err(invalid()),
    };
    Ok((id, name))
}
fn optional_text(value: Option<&Value>, max: usize) -> Result<Option<String>, ProviderError> {
    match value {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(s)) if s.len() <= max && !s.contains('\0') => Ok(Some(s.clone())),
        _ => Err(invalid()),
    }
}
fn body(value: Option<&Value>) -> Result<DetailValue, ProviderError> {
    Ok(match value {
        None => DetailValue {
            state: DetailValueState::Omitted,
            text: None,
        },
        Some(Value::Null) => DetailValue {
            state: DetailValueState::Known,
            text: None,
        },
        Some(Value::String(s)) if s.contains('\0') => return Err(invalid()),
        Some(Value::String(s)) if s.len() > 4096 => DetailValue {
            state: DetailValueState::Oversized,
            text: None,
        },
        Some(Value::String(s)) => DetailValue {
            state: DetailValueState::Known,
            text: Some(s.clone()),
        },
        _ => return Err(invalid()),
    })
}
fn pull_identity(value: &Value, repository: &str, pull: u64) -> Result<(), ProviderError> {
    let value = value.as_object().ok_or_else(invalid)?;
    if native_id(value.get("id").ok_or_else(invalid)?)? != pull {
        return Err(invalid());
    }
    if let Some(destination) = value.get("destination").filter(|v| !v.is_null()) {
        let destination = destination.as_object().ok_or_else(invalid)?;
        if let Some(target) = destination.get("repository").filter(|v| !v.is_null()) {
            let target = target.as_object().ok_or_else(invalid)?;
            if let Some(uuid) = target.get("uuid") {
                if canonical_uuid(uuid.as_str().ok_or_else(invalid)?)? != repository {
                    return Err(invalid());
                }
            }
        }
    }
    Ok(())
}
// Normalize only semantic ref fields, never hyperlinks or display metadata.
fn reference(value: Option<&Value>, target: Option<&str>) -> Result<Value, ProviderError> {
    let Some(value) = value.filter(|v| !v.is_null()) else {
        return Ok(Value::Null);
    };
    let value = value.as_object().ok_or_else(invalid)?;
    let repository = match value.get("repository") {
        None | Some(Value::Null) => None,
        Some(r) => Some(canonical_uuid(
            r.get("uuid").and_then(Value::as_str).ok_or_else(invalid)?,
        )?),
    };
    if target.is_some() && repository.as_deref().is_some_and(|id| Some(id) != target) {
        return Err(invalid());
    }
    let branch = value
        .get("branch")
        .map(|v| {
            let v = v.as_object().ok_or_else(invalid)?;
            optional_text(v.get("name"), 1024)
        })
        .transpose()?
        .flatten();
    let commit = value
        .get("commit")
        .map(|v| {
            let v = v.as_object().ok_or_else(invalid)?;
            let hash = optional_text(v.get("hash"), 64)?;
            if hash.as_ref().is_some_and(|s| {
                s.len() < 7
                    || !s
                        .bytes()
                        .all(|b| b.is_ascii_digit() || matches!(b, b'a'..=b'f'))
            }) {
                return Err(invalid());
            }
            Ok(hash)
        })
        .transpose()?
        .flatten();
    Ok(serde_json::json!([repository, branch, commit]))
}
fn activity(
    row: &Map<String, Value>,
    account: &str,
    repository: &str,
    pull: u64,
) -> Result<Option<DetailEntry>, ProviderError> {
    let families: Vec<_> = ["comment", "update", "approval", "changes_requested"]
        .into_iter()
        .filter(|key| row.contains_key(*key))
        .collect();
    if families.is_empty() {
        return Ok(None);
    }
    if families.len() != 1 {
        return Err(invalid());
    }
    pull_identity(
        row.get("pull_request").ok_or_else(invalid)?,
        repository,
        pull,
    )?;
    let family = families[0];
    let value = row[family].as_object().ok_or_else(invalid)?;
    if let Some(parent) = value.get("pullrequest") {
        pull_identity(parent, repository, pull)?;
    }
    let (identity, kind, occurred_at, updated_at, author, body, description) =
        if family == "comment" {
            if value.get("type").and_then(Value::as_str) != Some("pullrequest_comment") {
                return Ok(None);
            }
            let id = native_id(value.get("id").ok_or_else(invalid)?)?;
            let created = timestamp(value.get("created_on").ok_or_else(invalid)?)?;
            let updated = timestamp(value.get("updated_on").ok_or_else(invalid)?)?;
            if created > updated {
                return Err(invalid());
            }
            let deleted = value
                .get("deleted")
                .and_then(Value::as_bool)
                .ok_or_else(invalid)?;
            let author = if deleted {
                None
            } else {
                value
                    .get("user")
                    .filter(|v| !v.is_null())
                    .map(actor)
                    .transpose()?
                    .and_then(|a| a.1)
            };
            let body = if deleted {
                DetailValue {
                    state: DetailValueState::Known,
                    text: None,
                }
            } else {
                body(value.get("content").and_then(|v| v.get("raw")))?
            };
            (
                format!("comment:{id:020}"),
                "commented".to_owned(),
                created,
                updated,
                author,
                body,
                deleted.then(|| "Comment removed".into()),
            )
        } else {
            let date = timestamp(value.get("date").ok_or_else(invalid)?)?;
            let (actor_id, author) = actor(
                value
                    .get(if family == "update" { "author" } else { "user" })
                    .ok_or_else(invalid)?,
            )?;
            let title = optional_text(value.get("title"), 1024)?;
            let state = optional_text(value.get("state"), 64)?;
            let description = value.get("description");
            let body = if family == "update" {
                body(description)?
            } else {
                DetailValue {
                    state: DetailValueState::Known,
                    text: None,
                }
            };
            // Hash full bounded-response text rather than its truncated display, so
            // different oversized records are not silently conflated.
            let description_hash = match description {
                None | Some(Value::Null) => None,
                Some(Value::String(s)) if !s.contains('\0') => {
                    Some(format!("{:x}", Sha256::digest(s.as_bytes())))
                }
                _ => return Err(invalid()),
            };
            let reason = optional_text(value.get("reason"), 4096)?;
            let source = reference(value.get("source"), None)?;
            let destination = reference(value.get("destination"), Some(repository))?;
            let semantics = serde_json::to_vec(&(
                "bitbucket.org/activity/observation/v1",
                account,
                repository,
                pull,
                family,
                &date,
                actor_id,
                &state,
                &title,
                description_hash,
                reason,
                source,
                destination,
            ))
            .map_err(|_| invalid())?;
            let identity = format!("observation:{:x}", Sha256::digest(semantics));
            let kind = match family {
                "approval" => "approved",
                "changes_requested" => "changes_requested",
                _ => "updated",
            };
            let description = if family == "update" {
                match (state, title) {
                    (Some(state), Some(title)) => {
                        Some(format!("{state} · {title}").chars().take(256).collect())
                    }
                    (state, title) => state.or(title),
                }
            } else {
                None
            };
            (
                identity,
                kind.to_owned(),
                date.clone(),
                date,
                author,
                body,
                description,
            )
        };
    let event = crate::ActivityEvent {
        kind,
        supported: true,
        occurred_at: Some(occurred_at),
        description,
    };
    if !event.valid() {
        return Err(invalid());
    }
    Ok(Some(DetailEntry {
        id: format!("bitbucket-activity:{identity}"),
        provider_id: format!("{repository}:{pull}:{identity}"),
        author,
        title: None,
        state: None,
        observed_body_state: body.state,
        body,
        updated_at: Some(updated_at),
        head_oid: None,
        native: Some(crate::NativeDetailPayload::ActivityV1(event)),
        field_mask: FIELDS.into(),
        field_validations: vec![],
    }))
}
impl BitbucketCloudProvider {
    pub(super) async fn activity(
        &self,
        token: &SecretToken,
        request: DetailRequest,
    ) -> Result<DetailPage, ProviderError> {
        if request.facet != DetailFacet::Activity
            || request.subject.kind != RemoteItemKind::PullRequest
        {
            return Err(ProviderError::new(ProviderErrorKind::Unsupported));
        }
        if request.etag.is_some() {
            return Err(invalid());
        }
        let (repository, pull) = subject_identity(&request)?;
        let route = Route::Activity(repository.clone(), pull);
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
            if collection.values.len() > 50
                || collection
                    .page
                    .is_some_and(|page| page != cursor.pages as u64 + 1)
                || collection
                    .pagelen
                    .is_some_and(|size| size == 0 || size > 50)
                || collection
                    .size
                    .is_some_and(|size| size < collection.values.len() as u64)
            {
                return Err(invalid());
            }
            let mut identities = HashSet::new();
            let mut entries = Vec::with_capacity(collection.values.len());
            for row in collection.values {
                if let Some(entry) = activity(&row, &request.account.id, &repository, pull)? {
                    // Public records have no unique event ID. Identical normalized
                    // observations may repeat within a shifting provider page.
                    if identities.insert(entry.provider_id.clone()) {
                        entries.push(entry);
                    } else if entry.id.contains(":comment:") {
                        return Err(invalid());
                    }
                }
            }
            let reconciliation = if collection.next.is_some() && cursor.pages + 1 >= MAX_PAGES {
                DetailReconciliation {
                    enumeration: DetailEnumeration::Truncated,
                    head_scope: DetailHeadScope::SubjectHistory,
                }
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
                    cursor.url = continuation(&self.http, &next, &route, cursor.pages)?.to_string();
                    if cursor
                        .seen_pages
                        .contains(&self.http.fingerprint(&cursor.url, &route)?)
                    {
                        return Err(invalid());
                    }
                    if cursor.pages < MAX_PAGES {
                        cursor.encoded(&self.http, &route).map(Some)
                    } else {
                        Ok(None)
                    }
                })
                .transpose()?
                .flatten();
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
        result.map_err(|e| quota(e, response.cooldown))
    }
}

#[cfg(test)]
mod tests;
