//! Native actionable to-dos. Pending and done are distinct provider observations.
use super::{
    resource_details::{actor_login, time},
    *,
};
use crate::notification_subjects::*;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::{HashMap, HashSet};

const DONE_PAGES_BETWEEN_PENDING_SWEEPS: u8 = 5;
// Bound native pagination counters independently of untrusted JSON integers.
const MAX_TODO_PAGE: u64 = u32::MAX as u64;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Cursor {
    version: u8,
    account: String,
    epoch: String,
    done: bool,
    page: u64,
    /// Saved done-history position while the higher-priority pending feed runs.
    #[serde(default)]
    resume_done_page: Option<u64>,
    #[serde(default)]
    done_pages_since_pending: u8,
}

impl GitlabProvider {
    pub(super) async fn todos(
        &self,
        token: &SecretToken,
        request: FeedRequest,
    ) -> Result<FetchPage, ProviderError> {
        if request.repository.is_some() {
            return Err(invalid());
        }
        let position = match request.cursor {
            None => Cursor {
                version: 1,
                account: request.account.id.clone(),
                epoch: request.account.authorization_epoch.clone(),
                done: false,
                page: 1,
                resume_done_page: None,
                done_pages_since_pending: 0,
            },
            Some(raw) => {
                if raw.len() > 1024 {
                    return Err(invalid());
                }
                let cursor: Cursor = serde_json::from_str(&raw).map_err(|_| invalid())?;
                if cursor.version != 1
                    || cursor.account != request.account.id
                    || cursor.epoch != request.account.authorization_epoch
                    || cursor.page == 0
                    || cursor.page > MAX_TODO_PAGE
                    || cursor.done_pages_since_pending >= DONE_PAGES_BETWEEN_PENDING_SWEEPS
                    || (cursor.done && cursor.resume_done_page.is_some())
                    || (!cursor.done && cursor.done_pages_since_pending != 0)
                    || (cursor.done
                        && (cursor.page - 1) % u64::from(DONE_PAGES_BETWEEN_PENDING_SWEEPS)
                            != u64::from(cursor.done_pages_since_pending))
                    || cursor.resume_done_page.is_some_and(|page| {
                        page > MAX_TODO_PAGE
                            || page <= u64::from(DONE_PAGES_BETWEEN_PENDING_SWEEPS)
                            || (page - 1) % u64::from(DONE_PAGES_BETWEEN_PENDING_SWEEPS) != 0
                    })
                {
                    return Err(invalid());
                }
                cursor
            }
        };
        // Only the closed native phase/page route reaches the transport. No raw
        // response URL is persisted or accepted as renderer authority.
        let response = self
            .http
            .get(self.http.todos(position.done, position.page)?, token)
            .await?;
        let mapped = (|| {
            let values: Vec<Map<String, Value>> =
                serde_json::from_slice(&response.body).map_err(|_| invalid())?;
            if values.len() > 50 || (values.is_empty() && response.next.is_some()) {
                return Err(invalid());
            }
            let mut repositories = HashMap::new();
            let mut ids = HashSet::new();
            let mut items = Vec::new();
            let mut subjects = Vec::new();
            for value in values {
                let (item, repository, mapping) = todo(&value, &request.account, position.done)?;
                if !ids.insert(item.provider_id.clone()) {
                    return Err(invalid());
                }
                if let Some(repository) = repository
                    && let Some(previous) =
                        repositories.insert(repository.id.clone(), repository.clone())
                    && previous != repository
                {
                    return Err(invalid());
                }
                subjects.push(NotificationSubjectObservation {
                    notification_id: item.id.clone(),
                    mapping,
                });
                items.push(item);
            }
            let next = if response.next.is_some() {
                let next_page = position
                    .page
                    .checked_add(1)
                    .filter(|page| *page <= MAX_TODO_PAGE)
                    .ok_or_else(invalid)?;
                let done_pages = if position.done {
                    position.done_pages_since_pending + 1
                } else {
                    0
                };
                if done_pages == DONE_PAGES_BETWEEN_PENDING_SWEEPS {
                    // Completed history cannot monopolize foreground freshness.
                    // Repeat the entire pending feed, without assuming where new
                    // items sort, then resume the exact saved done-history page.
                    Some(Cursor {
                        done: false,
                        page: 1,
                        resume_done_page: Some(next_page),
                        done_pages_since_pending: 0,
                        ..position
                    })
                } else {
                    Some(Cursor {
                        page: next_page,
                        done_pages_since_pending: done_pages,
                        ..position
                    })
                }
            } else if !position.done {
                Some(Cursor {
                    done: true,
                    page: position.resume_done_page.unwrap_or(1),
                    resume_done_page: None,
                    done_pages_since_pending: 0,
                    ..position
                })
            } else {
                None
            };
            Ok(FetchPage {
                repositories: repositories.into_values().collect(),
                items,
                endpoint_aliases: vec![],
                notification_subjects: subjects,
                next_cursor: next
                    .map(|v| {
                        let raw = serde_json::to_string(&v).map_err(|_| invalid())?;
                        if raw.len() > 1024 {
                            return Err(invalid());
                        }
                        Ok(raw)
                    })
                    .transpose()?,
                etag: None,
                last_modified: None,
                not_modified: false,
                poll_interval_seconds: None,
                cooldown_seconds: response.cooldown,
            })
        })();
        mapped.map_err(|e| with_quota(e, response.cooldown))
    }
}

fn string(value: &Map<String, Value>, key: &str, max: usize) -> Result<String, ProviderError> {
    bounded(
        value
            .get(key)
            .and_then(Value::as_str)
            .ok_or_else(invalid)?
            .into(),
        max,
    )
}
fn id(value: &Map<String, Value>, key: &str) -> Result<u64, ProviderError> {
    value
        .get(key)
        .and_then(Value::as_u64)
        .filter(|v| *v > 0)
        .ok_or_else(invalid)
}
fn repository(
    value: &Value,
    account: &RemoteAccount,
) -> Result<Option<RemoteRepository>, ProviderError> {
    if value.is_null() {
        return Ok(None);
    }
    let value = value.as_object().ok_or_else(invalid)?;
    let full_name = string(value, "path_with_namespace", 1024)?;
    Project {
        id: id(value, "id")?,
        path: string(value, "path", 255)?,
        name: string(value, "name", 1024)?,
        web_url: format!("https://gitlab.com/{full_name}"),
        path_with_namespace: full_name,
        description: None,
        default_branch: None,
    }
    .remote(&account.id)
    .map(Some)
}
// This is presentation-only. Even a valid web URL is never an API route.
fn safe_web(raw: Option<&Value>) -> Option<String> {
    let raw = raw?.as_str()?;
    if raw.len() > 2048
        || raw.chars().any(|c| c.is_control() || c.is_whitespace())
        || raw.contains('\\')
    {
        return None;
    }
    let url = reqwest::Url::parse(raw).ok()?;
    (url.scheme() == "https"
        && url.host_str() == Some("gitlab.com")
        && url.port().is_none()
        && url.username().is_empty()
        && url.password().is_none())
    .then(|| url.to_string())
}
fn selector(
    value: &Map<String, Value>,
    repository: Option<&RemoteRepository>,
    target_type: &str,
) -> NotificationSubjectMapping {
    use NotificationSubjectFallbackReason as F;
    let fallback = NotificationSubjectMapping::Fallback;
    let (kind, representation) = match target_type {
        "MergeRequest" => (
            NotificationSubjectKind::PullRequest,
            NotificationSubjectRepresentation::GitlabMergeRequest,
        ),
        "Issue" => (
            NotificationSubjectKind::Issue,
            NotificationSubjectRepresentation::GitlabIssue,
        ),
        _ => return fallback(F::UnsupportedSubjectType),
    };
    let Some(repository) = repository else {
        return fallback(F::InvalidRepository);
    };
    let Some(target) = value.get("target").and_then(Value::as_object) else {
        return fallback(F::MissingSubjectType);
    };
    let (Ok(native), Ok(iid), Ok(project)) = (
        id(target, "id"),
        id(target, "iid"),
        id(target, "project_id"),
    ) else {
        return fallback(F::InvalidSubjectType);
    };
    if project.to_string() != repository.provider_id
        || (kind == NotificationSubjectKind::PullRequest
            && target
                .get("target_project_id")
                .is_some_and(|v| v.as_u64() != Some(project)))
    {
        return fallback(F::RepositoryMismatch);
    }
    if kind == NotificationSubjectKind::Issue
        && target
            .get("issue_type")
            .is_some_and(|v| v.as_str() != Some("issue"))
    {
        return fallback(F::UnsupportedSubjectType);
    }
    NotificationSubjectMapping::Selector(NotificationSubjectSelector {
        kind,
        repository_provider_id: repository.provider_id.clone(),
        number: iid.to_string(),
        repository_path: repository.full_name.clone(),
        representation,
        subject_provider_id: Some(native.to_string()),
    })
}
fn todo(
    value: &Map<String, Value>,
    account: &RemoteAccount,
    done: bool,
) -> Result<
    (
        RemoteItem,
        Option<RemoteRepository>,
        NotificationSubjectMapping,
    ),
    ProviderError,
> {
    let native = id(value, "id")?;
    let state = string(value, "state", 32)?;
    if state != if done { "done" } else { "pending" } {
        return Err(invalid());
    }
    let completion = if done {
        TodoCompletion::Done
    } else {
        TodoCompletion::Pending
    };
    let action = string(value, "action_name", 128)?;
    let target_type = string(value, "target_type", 128)?;
    let repository = repository(value.get("project").unwrap_or(&Value::Null), account)?;
    let mapping = selector(value, repository.as_ref(), &target_type);
    let body = string(value, "body", 1024 * 1024)?;
    let title = value
        .get("target")
        .and_then(|v| v.get("title"))
        .and_then(Value::as_str)
        .filter(|v| !v.is_empty())
        .unwrap_or(&body);
    // Provider descriptions can be larger than a list title. Preserve a bounded
    // UTF-8 preview without retaining the target's full description in the cache.
    let title = title
        .chars()
        .scan(0usize, |n, c| {
            *n += c.len_utf8();
            (*n <= 16384).then_some(c)
        })
        .collect();
    let body_omitted = body.len() > 16384;
    let item = RemoteItem {
        id: format!("gitlab:todo:{native}"),
        account_id: account.id.clone(),
        repository_id: repository.as_ref().map(|r| r.id.clone()),
        provider_id: native.to_string(),
        kind: RemoteItemKind::Notification,
        number: None,
        title,
        body: (!body_omitted).then_some(body),
        body_omitted,
        author: value
            .get("author")
            .filter(|v| !v.is_null())
            .map(actor_login)
            .transpose()?,
        web_url: safe_web(value.get("target_url")),
        state,
        updated_at: time(value.get("updated_at").ok_or_else(invalid)?)?,
        head_oid: None,
        is_draft: None,
        reason: Some(action.clone()),
        unread: None,
        native_inbox: Some(NativeInboxState::Todo {
            completion,
            action,
            target_type,
        }),
    };
    Ok((item, repository, mapping))
}

#[cfg(test)]
mod tests;
