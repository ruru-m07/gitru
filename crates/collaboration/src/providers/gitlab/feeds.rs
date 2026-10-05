//! Bounded full feeds. Incremental time windows cannot establish feed absence.
use super::{
    resource_details::*,
    transport::{FeedPosition, ItemRoute, feed_position},
    *,
};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::HashSet;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Cursor {
    version: u8,
    url: String,
    account: String,
    epoch: String,
    project: u64,
    issue: bool,
    last_id: Option<u64>,
    last_created_at: Option<String>,
    // Offset pages can split equal creation timestamps. Persist that bounded
    // tie group so replayed pages cannot silently establish full coverage.
    creation_boundary: Vec<(u64, u64)>,
}
fn summary_body(json: &Map<String, Value>) -> Result<(Option<String>, bool), ProviderError> {
    match json.get("description") {
        None => Ok((None, true)),
        Some(Value::Null) => Ok((None, false)),
        Some(Value::String(value)) if value.len() <= 16384 => Ok((Some(value.clone()), false)),
        Some(Value::String(_)) => Ok((None, true)),
        _ => Err(invalid()),
    }
}
fn optional_string(
    json: &Map<String, Value>,
    key: &str,
    max: usize,
) -> Result<Option<String>, ProviderError> {
    match json.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(value)) => bounded(value.clone(), max).map(Some),
        _ => Err(invalid()),
    }
}
fn item(
    json: &Map<String, Value>,
    account: &RemoteAccount,
    repository: &RemoteRepository,
    project: u64,
    item_route: ItemRoute,
) -> Result<RemoteItem, ProviderError> {
    let (native, iid) = identity(json, project, item_route)?;
    let kind = if item_route == ItemRoute::MergeRequests {
        RemoteItemKind::PullRequest
    } else {
        RemoteItemKind::Issue
    };
    let title = json
        .get("title")
        .and_then(Value::as_str)
        .ok_or_else(invalid)?;
    let (body, body_omitted) = summary_body(json)?;
    let author = match json.get("author") {
        None | Some(Value::Null) => None,
        Some(value) => Some(actor_login(value)?),
    };
    let web_url = json
        .get("web_url")
        .map(|v| resource_web(v, iid, item_route))
        .transpose()?;
    let (head_oid, is_draft) = if item_route == ItemRoute::MergeRequests {
        let head = json.get("sha").map(oid).transpose()?.flatten();
        let draft = match json.get("draft") {
            None | Some(Value::Null) => None,
            Some(value) => Some(value.as_bool().ok_or_else(invalid)?),
        };
        (head, draft)
    } else {
        (None, None)
    };
    Ok(RemoteItem {
        id: format!(
            "gitlab:{}:{native}",
            if item_route == ItemRoute::MergeRequests {
                "pull"
            } else {
                "issue"
            }
        ),
        account_id: account.id.clone(),
        repository_id: Some(repository.id.clone()),
        provider_id: native.to_string(),
        kind,
        number: Some(iid.to_string()),
        title: bounded(title.into(), 16384)?,
        body,
        body_omitted,
        author,
        web_url,
        state: state(json.get("state").ok_or_else(invalid)?)?,
        updated_at: time(json.get("updated_at").ok_or_else(invalid)?)?,
        head_oid,
        is_draft,
        reason: optional_string(json, "state_reason", 128)?,
        unread: None,
    })
}

impl GitlabProvider {
    pub(super) async fn resource_feed(
        &self,
        token: &SecretToken,
        request: FeedRequest,
    ) -> Result<FetchPage, ProviderError> {
        let item_route = match request.kind {
            FeedKind::PullRequests => ItemRoute::MergeRequests,
            FeedKind::Issues => ItemRoute::Issues,
            _ => return Err(ProviderError::new(ProviderErrorKind::Unsupported)),
        };
        let repository = request.repository.as_ref().ok_or_else(invalid)?;
        let project = repository_identity(&request.account, repository)?;
        let issue = item_route == ItemRoute::Issues;
        let saved: Option<Cursor> = request
            .cursor
            .as_ref()
            .map(|raw| {
                if raw.len() > 4096 {
                    return Err(invalid());
                }
                let saved: Cursor = serde_json::from_str(raw).map_err(|_| invalid())?;
                if saved.version != 1
                    || saved.account != request.account.id
                    || saved.epoch != request.account.authorization_epoch
                    || saved.project != project
                    || saved.issue != issue
                    || if issue {
                        saved.last_id.is_none_or(|id| id == 0)
                            || saved.last_created_at.is_some()
                            || !saved.creation_boundary.is_empty()
                    } else {
                        saved.last_id.is_some()
                            || saved
                                .last_created_at
                                .as_ref()
                                .is_none_or(|at| time(&Value::String(at.clone())).is_err())
                            || saved.creation_boundary.is_empty()
                            || saved.creation_boundary.len() > 100
                            || saved
                                .creation_boundary
                                .iter()
                                .any(|(id, iid)| *id == 0 || *iid == 0)
                    }
                {
                    return Err(invalid());
                }
                let ids: HashSet<_> = saved.creation_boundary.iter().map(|(id, _)| id).collect();
                let iids: HashSet<_> = saved.creation_boundary.iter().map(|(_, iid)| iid).collect();
                if ids.len() != saved.creation_boundary.len()
                    || iids.len() != saved.creation_boundary.len()
                {
                    return Err(invalid());
                }
                Ok(saved)
            })
            .transpose()?;
        let endpoint = match &saved {
            Some(saved) => self
                .http
                .resource_continuation(&saved.url, project, item_route)?,
            None => self.http.resource_feed(project, item_route)?,
        };
        if let FeedPosition::Keyset {
            after: Some(after), ..
        } = feed_position(&endpoint, item_route)?
            && Some(after) != saved.as_ref().and_then(|saved| saved.last_id)
        {
            return Err(invalid());
        }
        let response = self.http.get(endpoint, token).await?;
        let mapped = (|| {
            let values: Vec<Map<String, Value>> =
                serde_json::from_slice(&response.body).map_err(|_| invalid())?;
            if values.len() > 50 {
                return Err(invalid());
            }
            let mut last_id = saved.as_ref().and_then(|saved| saved.last_id).unwrap_or(0);
            let mut last_created_at = saved
                .as_ref()
                .and_then(|saved| saved.last_created_at.clone());
            let mut creation_boundary = saved
                .as_ref()
                .map(|saved| saved.creation_boundary.clone())
                .unwrap_or_default();
            let mut native_ids = HashSet::new();
            let mut iids = HashSet::new();
            let mut items = vec![];
            for value in values {
                let observed = item(&value, &request.account, repository, project, item_route)?;
                let native = positive_id(&observed.provider_id).ok_or_else(invalid)?;
                if !native_ids.insert(native) || !iids.insert(observed.number.clone()) {
                    return Err(invalid());
                }
                if issue {
                    if native <= last_id {
                        return Err(invalid());
                    }
                    last_id = native;
                } else {
                    let created = time(value.get("created_at").ok_or_else(invalid)?)?;
                    let created_time =
                        chrono::DateTime::parse_from_rfc3339(&created).map_err(|_| invalid())?;
                    if let Some(old) = &last_created_at {
                        match created_time.cmp(
                            &chrono::DateTime::parse_from_rfc3339(old).map_err(|_| invalid())?,
                        ) {
                            std::cmp::Ordering::Less => return Err(invalid()),
                            std::cmp::Ordering::Greater => creation_boundary.clear(),
                            std::cmp::Ordering::Equal => {}
                        }
                    }
                    let iid = observed
                        .number
                        .as_deref()
                        .and_then(positive_id)
                        .ok_or_else(invalid)?;
                    if creation_boundary
                        .iter()
                        .any(|(seen_id, seen_iid)| *seen_id == native || *seen_iid == iid)
                    {
                        return Err(invalid());
                    }
                    creation_boundary.push((native, iid));
                    // Fail this partial traversal rather than silently discard
                    // identity evidence when a pathological tie group grows.
                    if creation_boundary.len() > 100 {
                        return Err(invalid());
                    }
                    last_created_at = Some(created);
                }
                items.push(observed);
            }
            let next_cursor = response
                .next
                .as_ref()
                .map(|url| {
                    if items.is_empty() {
                        return Err(invalid());
                    }
                    if let FeedPosition::Keyset {
                        after: Some(after), ..
                    } = feed_position(
                        &self.http.resource_continuation(url, project, item_route)?,
                        item_route,
                    )? && after != last_id
                    {
                        return Err(invalid());
                    }
                    let cursor = Cursor {
                        version: 1,
                        url: url.clone(),
                        account: request.account.id.clone(),
                        epoch: request.account.authorization_epoch.clone(),
                        project,
                        issue,
                        last_id: issue.then_some(last_id),
                        last_created_at: if issue { None } else { last_created_at },
                        creation_boundary,
                    };
                    let serialized = serde_json::to_string(&cursor).map_err(|_| invalid())?;
                    if serialized.len() > 4096 {
                        return Err(invalid());
                    }
                    Ok(serialized)
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
        mapped.map_err(|error| with_quota(error, response.cooldown))
    }
}
