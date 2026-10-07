//! Merge-request commits from GitLab's bounded singleton collection.
use super::*;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::HashSet;

pub(super) const SOURCE: &str = "gitlab/merge-request-commits/v4";
const STRATEGY: &str = "offset_pages_v1";
const PAGE_SIZE: u32 = 100;
const MAX_CURSOR_BYTES: usize = 4096;
const MAX_SUMMARY_BYTES: usize = 4 * 1024;

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
    iid: u64,
    context: PullCommitContext,
    pages: u32,
    seen: u32,
    url: String,
}

fn bounded_identity(value: &str, maximum: usize) -> bool {
    !value.is_empty() && value.len() <= maximum && !value.chars().any(char::is_control)
}

fn positive(raw: &str) -> Result<u64, ProviderError> {
    positive_id(raw).ok_or_else(invalid)
}

fn identity(request: &PullCommitRequest) -> Result<(u64, u64), ProviderError> {
    if request.account.provider != ProviderKind::Gitlab || request.account.host != "gitlab.com" {
        return Err(ProviderError::new(ProviderErrorKind::Unsupported));
    }
    if request.account.state != AccountState::Active {
        return Err(ProviderError::new(ProviderErrorKind::Authentication));
    }
    let project = resource_details::repository_identity(&request.account, &request.repository)?;
    let native = positive(&request.subject.provider_id)?;
    let iid = positive(request.subject.number.as_deref().ok_or_else(invalid)?)?;
    positive(&request.account.actor_id)?;
    positive(&request.account.authorization_epoch)?;
    positive(&request.context.source_repository_provider_id)?;
    if !bounded_identity(&request.account.id, 256)
        || request.subject.kind != RemoteItemKind::PullRequest
        || request.subject.account_id != request.account.id
        || request.subject.repository_id.as_ref() != Some(&request.repository.id)
        || request.subject.id != format!("gitlab:pull:{native}")
        || !bounded_identity(&request.context.metadata_facet_revision, 256)
        || !is_canonical_commit_oid(&request.context.base_oid)
        || !is_canonical_commit_oid(&request.context.head_oid)
    {
        return Err(invalid());
    }
    Ok((project, iid))
}

impl Cursor {
    fn open(
        request: &PullCommitRequest,
        project: u64,
        iid: u64,
        http: &GitlabHttp,
    ) -> Result<Self, ProviderError> {
        let Some(raw) = &request.cursor else {
            if request.start_position != 0 {
                return Err(invalid());
            }
            return Ok(Self {
                version: 1,
                strategy: STRATEGY.into(),
                account: request.account.id.clone(),
                actor: request.account.actor_id.clone(),
                epoch: request.account.authorization_epoch.clone(),
                repository: request.repository.id.clone(),
                project,
                subject: request.subject.id.clone(),
                subject_native: request.subject.provider_id.clone(),
                iid,
                context: request.context.clone(),
                pages: 0,
                seen: 0,
                url: http.pull_commits(project, iid)?.to_string(),
            });
        };
        if raw.len() > MAX_CURSOR_BYTES {
            return Err(invalid());
        }
        let cursor: Self = serde_json::from_str(raw).map_err(|_| invalid())?;
        if cursor.version != 1
            || cursor.strategy != STRATEGY
            || cursor.account != request.account.id
            || cursor.actor != request.account.actor_id
            || cursor.epoch != request.account.authorization_epoch
            || cursor.repository != request.repository.id
            || cursor.project != project
            || cursor.subject != request.subject.id
            || cursor.subject_native != request.subject.provider_id
            || cursor.iid != iid
            || cursor.context != request.context
            || cursor.pages == 0
            || cursor.pages >= MAX_PULL_COMMIT_PAGES
            || cursor.seen != cursor.pages.saturating_mul(PAGE_SIZE)
            || cursor.seen != request.start_position
            || cursor.seen >= MAX_PULL_COMMITS
        {
            return Err(invalid());
        }
        http.pull_commit_continuation(&cursor.url, project, iid, u64::from(cursor.pages + 1))?;
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

fn required_text(
    value: Option<&Value>,
    maximum: usize,
    empty: bool,
) -> Result<String, ProviderError> {
    let value = value.and_then(Value::as_str).ok_or_else(invalid)?;
    if value.len() > maximum || !empty && value.is_empty() || value.contains('\0') {
        return Err(invalid());
    }
    Ok(value.into())
}

fn optional_time(value: Option<&Value>) -> Result<Option<String>, ProviderError> {
    match value {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(value))
            if value.len() <= 128 && chrono::DateTime::parse_from_rfc3339(value).is_ok() =>
        {
            Ok(Some(value.clone()))
        }
        _ => Err(invalid()),
    }
}

fn actor(value: Option<&Value>) -> Result<PullCommitActor, ProviderError> {
    let name = required_text(value, 1024, false)?;
    if name.chars().any(char::is_control) {
        return Err(invalid());
    }
    Ok(PullCommitActor {
        name,
        provider: None,
    })
}

fn message(value: Option<&Value>) -> Result<PullCommitMessage, ProviderError> {
    match value {
        None | Some(Value::Null) => Ok(PullCommitMessage {
            state: PullCommitMessageState::Omitted,
            text: None,
        }),
        Some(Value::String(value)) if !value.contains('\0') => {
            if value.len() <= MAX_PULL_COMMIT_MESSAGE_BYTES {
                Ok(PullCommitMessage {
                    state: PullCommitMessageState::Known,
                    text: Some(value.clone()),
                })
            } else {
                Ok(PullCommitMessage {
                    state: PullCommitMessageState::Oversized,
                    text: None,
                })
            }
        }
        _ => Err(invalid()),
    }
}

fn commit_web(raw: &str, oid: &str) -> Result<String, ProviderError> {
    if raw.len() > 2048
        || raw.trim() != raw
        || raw
            .chars()
            .any(|character| character.is_control() || character.is_whitespace())
        || raw.contains(['\\', '#'])
    {
        return Err(invalid());
    }
    let url = reqwest::Url::parse(raw).map_err(|_| invalid())?;
    let suffix = format!("/-/commit/{oid}");
    let repository = url
        .path()
        .strip_suffix(&suffix)
        .and_then(|path| path.strip_prefix('/'))
        .ok_or_else(invalid)?;
    if url.as_str() != raw
        || url.scheme() != "https"
        || url.host_str() != Some("gitlab.com")
        || url.port().is_some()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || !valid_path(repository)
    {
        return Err(invalid());
    }
    Ok(raw.into())
}

fn commit(row: &Map<String, Value>) -> Result<ProviderPullCommit, ProviderError> {
    let oid = required_text(row.get("id"), 64, false)?;
    if !is_canonical_commit_oid(&oid) {
        return Err(invalid());
    }
    let summary = required_text(row.get("title"), MAX_SUMMARY_BYTES, true)?;
    if summary.contains(['\n', '\r', '\0']) {
        return Err(invalid());
    }
    let parent_values = row
        .get("parent_ids")
        .and_then(Value::as_array)
        .ok_or_else(invalid)?;
    if parent_values.len() > MAX_PULL_COMMIT_PARENTS {
        return Err(invalid());
    }
    let mut parent_oids = Vec::with_capacity(parent_values.len());
    let mut parents = HashSet::new();
    for parent in parent_values {
        let parent = required_text(Some(parent), 64, false)?;
        if !is_canonical_commit_oid(&parent) || !parents.insert(parent.clone()) {
            return Err(invalid());
        }
        parent_oids.push(parent);
    }
    let web_url = match row.get("web_url") {
        None | Some(Value::Null) => None,
        Some(Value::String(raw)) => Some(commit_web(raw, &oid)?),
        _ => return Err(invalid()),
    };
    Ok(ProviderPullCommit {
        oid,
        summary,
        message: message(row.get("message"))?,
        author: actor(row.get("author_name"))?,
        committer: Some(actor(row.get("committer_name"))?),
        authored_at: optional_time(row.get("authored_date"))?,
        committed_at: optional_time(row.get("committed_date"))?,
        parent_oids,
        web_url,
    })
}

fn normalize(body: &[u8]) -> Result<Vec<ProviderPullCommit>, ProviderError> {
    let rows: Vec<Map<String, Value>> = serde_json::from_slice(body).map_err(|_| invalid())?;
    if rows.len() > PAGE_SIZE as usize {
        return Err(invalid());
    }
    let mut commits = Vec::with_capacity(rows.len());
    let mut identities = HashSet::new();
    for row in rows {
        let commit = commit(&row)?;
        if !identities.insert(commit.oid.clone()) {
            return Err(invalid());
        }
        commits.push(commit);
    }
    Ok(commits)
}

impl GitlabProvider {
    pub(super) async fn request_pull_commits(
        &self,
        token: &SecretToken,
        request: PullCommitRequest,
    ) -> Result<PullCommitProviderPage, ProviderError> {
        let (project, iid) = identity(&request)?;
        let mut cursor = Cursor::open(&request, project, iid, &self.http)?;
        let response = self
            .http
            .get(
                reqwest::Url::parse(&cursor.url).map_err(|_| invalid())?,
                token,
            )
            .await?;
        let start_position = cursor.seen;
        let result = (|| {
            let commits = normalize(&response.body)?;
            if response.next.is_some() && commits.len() != PAGE_SIZE as usize {
                return Err(invalid());
            }
            cursor.pages += 1;
            cursor.seen = cursor
                .seen
                .checked_add(commits.len() as u32)
                .ok_or_else(invalid)?;
            if cursor.seen > MAX_PULL_COMMITS {
                return Err(invalid());
            }
            let remote_has_more = response.next.is_some();
            let local_capped = remote_has_more
                && (cursor.seen == MAX_PULL_COMMITS || cursor.pages == MAX_PULL_COMMIT_PAGES);
            let next_cursor = if local_capped {
                None
            } else {
                response
                    .next
                    .map(|next| {
                        cursor.url = next;
                        cursor.encoded()
                    })
                    .transpose()?
            };
            Ok(PullCommitProviderPage {
                context: request.context.clone(),
                commits,
                order: PullCommitProviderOrder::HeadToBase,
                source: PullCommitSource {
                    source: SOURCE.into(),
                    adapter_version: 1,
                },
                start_position,
                next_cursor,
                cap_reason: local_capped.then_some(PullCommitCapReason::LocalLimit),
                remote_has_more,
                freshness_seconds: 60,
                cooldown_seconds: response.cooldown,
            })
        })();
        result.map_err(|error| with_quota(error, response.cooldown))
    }
}
