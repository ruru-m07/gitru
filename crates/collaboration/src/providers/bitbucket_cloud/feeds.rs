//! All-state summaries with persistent bounded coverage and opaque continuations.
use super::{resource_details::*, *};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::HashSet;

const MAX_PAGES: usize = 20;
const MAX_CURSOR: usize = 4096;
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Cursor {
    version: u8,
    kind: String,
    account: String,
    epoch: String,
    repository: String,
    url: String,
    pages: usize,
    seen_pages: Vec<String>,
    last_id: Option<u64>,
}
#[derive(Deserialize)]
struct Collection {
    values: Vec<Map<String, Value>>,
    next: Option<String>,
}
impl Cursor {
    fn open(
        request: &FeedRequest,
        repository: &str,
        http: &BitbucketHttp,
        route: &Route,
    ) -> Result<Self, ProviderError> {
        let Some(raw) = &request.cursor else {
            return Ok(Self {
                version: 1,
                kind: "pull_requests".into(),
                account: request.account.id.clone(),
                epoch: request.account.authorization_epoch.clone(),
                repository: repository.into(),
                url: http.endpoint(route)?.to_string(),
                pages: 0,
                seen_pages: vec![],
                last_id: None,
            });
        };
        if raw.len() > MAX_CURSOR {
            return Err(invalid());
        }
        let cursor: Self = serde_json::from_str(raw).map_err(|_| invalid())?;
        if cursor.version != 1
            || cursor.kind != "pull_requests"
            || cursor.account != request.account.id
            || cursor.epoch != request.account.authorization_epoch
            || cursor.repository != repository
            || cursor.pages == 0
            || cursor.pages > MAX_PAGES
            || cursor.seen_pages.len() != cursor.pages
            || cursor.last_id == Some(0)
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
        if hashes.contains(&http.fingerprint(&cursor.url, route)?) || cursor.pages >= MAX_PAGES {
            // Scheduler yields/manual refresh/cold reopen cannot erase this
            // traversal's loop evidence or silently reset its page budget.
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
fn item(
    json: &Map<String, Value>,
    account: &RemoteAccount,
    repository: &RemoteRepository,
    uuid: &str,
) -> Result<RemoteItem, ProviderError> {
    let id = identity(json, uuid)?;
    let (state, reason) = state(json.get("state").ok_or_else(invalid)?)?;
    let (body, body_omitted) = match raw_description(json)? {
        None => (None, true),
        Some(Value::Null) => (None, false),
        Some(Value::String(value)) if value.len() <= 16384 => (Some(value.clone()), false),
        Some(Value::String(_)) => (None, true),
        _ => return Err(invalid()),
    };
    let author = match json.get("author") {
        None | Some(Value::Null) => None,
        Some(value) => Some(actor(value)?.login),
    };
    let web_url = json
        .get("links")
        .map(|value| {
            let links = value.as_object().ok_or_else(invalid)?;
            match links.get("html") {
                None | Some(Value::Null) => Ok(None),
                Some(value) => value
                    .as_object()
                    .ok_or_else(invalid)?
                    .get("href")
                    .map(|value| resource_web(value, id))
                    .transpose(),
            }
        })
        .transpose()?
        .flatten();
    Ok(RemoteItem {
        native_inbox: None,
        id: format!("bitbucket_cloud:pull:{uuid}:{id}"),
        account_id: account.id.clone(),
        repository_id: Some(repository.id.clone()),
        provider_id: format!("{uuid}:{id}"),
        kind: RemoteItemKind::PullRequest,
        number: Some(id.to_string()),
        title: text(
            json.get("title")
                .and_then(Value::as_str)
                .ok_or_else(invalid)?
                .into(),
            16384,
            false,
        )?,
        body,
        body_omitted,
        author,
        web_url,
        state,
        reason,
        updated_at: time(json.get("updated_on").ok_or_else(invalid)?)?,
        head_oid: branch(json, "source")?.map(|head| head.oid),
        is_draft: None,
        unread: None,
    })
}
impl BitbucketCloudProvider {
    pub(super) async fn resource_feed(
        &self,
        token: &SecretToken,
        request: FeedRequest,
    ) -> Result<FetchPage, ProviderError> {
        if request.kind != FeedKind::PullRequests {
            return Err(ProviderError::new(ProviderErrorKind::Unsupported));
        }
        let repository = request.repository.as_ref().ok_or_else(invalid)?;
        let uuid = repository_identity(&request.account, repository)?;
        let route = Route::PullRequests(uuid.clone());
        let mut cursor = Cursor::open(&request, &uuid, &self.http, &route)?;
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
            let mut items = Vec::new();
            let mut ids = HashSet::new();
            for value in collection.values {
                let observed = item(&value, &request.account, repository, &uuid)?;
                let id = observed
                    .number
                    .as_deref()
                    .and_then(positive_id)
                    .ok_or_else(invalid)?;
                if !ids.insert(id) || cursor.last_id.is_some_and(|last| id <= last) {
                    return Err(invalid());
                }
                cursor.last_id = Some(id);
                items.push(observed);
            }
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
            Ok(FetchPage {
                repositories: vec![],
                items,
                endpoint_aliases: vec![],
                notification_subjects: vec![],
                next_cursor,
                etag: None,
                last_modified: None,
                not_modified: false,
                poll_interval_seconds: None,
                cooldown_seconds: response.cooldown,
            })
        })();
        result.map_err(|error| quota(error, response.cooldown))
    }
}
