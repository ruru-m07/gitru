//! Top-level published conversation comments only; no inline/thread reconstruction.
use super::*;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::HashSet;
const MAX_PAGES: usize = 20;
const MAX_CURSOR: usize = 4096;
pub(super) const SOURCE: &str = "bitbucket.comments.v1";
const FIELDS: [DetailField; 4] = [
    DetailField::Body,
    DetailField::Author,
    DetailField::UpdatedAt,
    DetailField::State,
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
                strategy: "uncertain_subject_history".into(),
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
            || cursor.strategy != "uncertain_subject_history"
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
fn comment(
    row: &Map<String, Value>,
    repository: &str,
    pull: u64,
) -> Result<Option<DetailEntry>, ProviderError> {
    // Unknown kinds, inline positions, replies, and pending/system rows are not
    // ordinary published conversation. Skipping them cannot authorize absence.
    if row.get("type").and_then(Value::as_str) != Some("pullrequest_comment")
        || ["inline", "parent"]
            .iter()
            .any(|key| row.get(*key).is_some_and(|v| !v.is_null()))
        || ["system", "pending"].iter().any(|key| {
            row.get(*key)
                .is_some_and(|v| !v.is_null() && v != &Value::Bool(false))
        })
    {
        return Ok(None);
    }
    let id = native_id(row.get("id").ok_or_else(invalid)?)?;
    if let Some(parent) = row.get("pullrequest") {
        let parent = parent.as_object().ok_or_else(invalid)?;
        if native_id(parent.get("id").ok_or_else(invalid)?)? != pull {
            return Err(invalid());
        }
    }
    let updated_at = resource_details::time(row.get("updated_on").ok_or_else(invalid)?)?;
    if let Some(created) = row.get("created_on") {
        let created = resource_details::time(created)?;
        if chrono::DateTime::parse_from_rfc3339(&created).map_err(|_| invalid())?
            > chrono::DateTime::parse_from_rfc3339(&updated_at).map_err(|_| invalid())?
        {
            return Err(invalid());
        }
    }
    let deleted = row
        .get("deleted")
        .and_then(Value::as_bool)
        .ok_or_else(invalid)?;
    let mut mask = vec![
        DetailField::Body,
        DetailField::UpdatedAt,
        DetailField::State,
    ];
    let author = if deleted {
        // An explicit current tombstone clears private retained text and actor.
        mask.push(DetailField::Author);
        None
    } else {
        match row.get("user") {
            None => None,
            Some(Value::Null) => {
                mask.push(DetailField::Author);
                None
            }
            Some(Value::Object(user)) => {
                canonical_uuid(
                    user.get("uuid")
                        .and_then(Value::as_str)
                        .ok_or_else(invalid)?,
                )?;
                match user.get("nickname") {
                    None => None,
                    Some(Value::Null) => {
                        mask.push(DetailField::Author);
                        None
                    }
                    Some(Value::String(name)) => {
                        mask.push(DetailField::Author);
                        Some(text(name.clone(), 255, false)?)
                    }
                    _ => return Err(invalid()),
                }
            }
            _ => return Err(invalid()),
        }
    };
    let body = if deleted {
        DetailValue {
            state: DetailValueState::Known,
            text: None,
        }
    } else {
        match row
            .get("content")
            .and_then(Value::as_object)
            .and_then(|c| c.get("raw"))
        {
            None => DetailValue {
                state: DetailValueState::Omitted,
                text: None,
            },
            Some(Value::String(raw)) if raw.len() <= 65_536 && !raw.contains('\0') => DetailValue {
                state: DetailValueState::Known,
                text: Some(raw.clone()),
            },
            Some(Value::String(raw)) if raw.len() > 65_536 => DetailValue {
                state: DetailValueState::Oversized,
                text: None,
            },
            _ => return Err(invalid()),
        }
    };
    Ok(Some(DetailEntry {
        id: format!("bitbucket-comment:{id:020}"),
        provider_id: format!("{repository}:{pull}:{id}"),
        author,
        title: None,
        state: Some(if deleted { "deleted" } else { "present" }.into()),
        observed_body_state: body.state,
        body,
        updated_at: Some(updated_at),
        head_oid: None,
        native: None,
        field_mask: mask,
        field_validations: vec![],
    }))
}
impl BitbucketCloudProvider {
    pub(super) async fn comments(
        &self,
        token: &SecretToken,
        request: DetailRequest,
    ) -> Result<DetailPage, ProviderError> {
        if request.facet != DetailFacet::Comments
            || request.subject.kind != RemoteItemKind::PullRequest
        {
            return Err(ProviderError::new(ProviderErrorKind::Unsupported));
        }
        if request.etag.is_some() {
            return Err(invalid());
        }
        let (repository, pull) = subject_identity(&request)?;
        let route = Route::Comments(repository.clone(), pull);
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
            let mut full = collection
                .size
                .is_none_or(|size| size == collection.values.len() as u64);
            for row in collection.values {
                // Duplicates are malformed even if one representation is skipped.
                let id = native_id(row.get("id").ok_or_else(invalid)?)?;
                if !identities.insert(id) {
                    return Err(invalid());
                }
                if let Some(entry) = comment(&row, &repository, pull)? {
                    full &= entry.body.state == DetailValueState::Known
                        && entry.field_mask.contains(&DetailField::Author);
                    entries.push(entry);
                } else {
                    full = false;
                }
            }
            let reconciliation = if full && cursor.pages == 0 && collection.next.is_none() {
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
                    cursor.url = continuation(&self.http, &next, &route, cursor.pages)?.to_string();
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
        result.map_err(|e| quota(e, response.cooldown))
    }
}
