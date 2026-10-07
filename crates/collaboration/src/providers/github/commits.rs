//! Pull-request commits bound to immutable repository and exact range context.
use super::*;
use crate::DetailActor;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::HashSet;

pub(super) const SOURCE: &str = "github/pull-commits/2026-03-10";
const STRATEGY: &str = "pull_commits_v1";
const PAGE_SIZE: u32 = 50;
const PROVIDER_LIMIT: u32 = 250;
const MAX_PAGES: u32 = PROVIDER_LIMIT / PAGE_SIZE;
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
    repository_native: String,
    subject: String,
    subject_native: String,
    pull: u64,
    context: PullCommitContext,
    pages: u32,
    seen: u32,
    url: String,
}

fn invalid() -> ProviderError {
    ProviderError::new(ProviderErrorKind::InvalidResponse)
}

fn positive(raw: &str) -> Result<u64, ProviderError> {
    let value = raw.parse::<u64>().map_err(|_| invalid())?;
    if value == 0 || value.to_string() != raw {
        return Err(invalid());
    }
    Ok(value)
}

fn bounded_identity(value: &str, maximum: usize) -> bool {
    !value.is_empty() && value.len() <= maximum && !value.chars().any(char::is_control)
}

fn identity(request: &PullCommitRequest) -> Result<(String, u64), ProviderError> {
    let repository = positive(&request.repository.provider_id)?;
    let pull = positive(request.subject.number.as_deref().ok_or_else(invalid)?)?;
    positive(&request.subject.provider_id)?;
    positive(&request.account.actor_id)?;
    positive(&request.account.authorization_epoch)?;
    positive(&request.context.source_repository_provider_id)?;
    if request.account.provider != ProviderKind::Github
        || request.account.host != "github.com"
        || request.account.state != AccountState::Active
        || !bounded_identity(&request.account.id, 256)
        || request.repository.account_id != request.account.id
        || !bounded_identity(&request.repository.id, 256)
        || !request.repository.selected
        || !valid_repository_path(&request.repository.full_name)
        || request.subject.kind != RemoteItemKind::PullRequest
        || request.subject.account_id != request.account.id
        || request.subject.repository_id.as_ref() != Some(&request.repository.id)
        || request.subject.id != format!("github:pull:{}", request.subject.provider_id)
        || !bounded_identity(&request.context.metadata_facet_revision, 256)
        || !is_canonical_commit_oid(&request.context.base_oid)
        || !is_canonical_commit_oid(&request.context.head_oid)
    {
        return Err(invalid());
    }
    Ok((
        format!("/repositories/{repository}/pulls/{pull}/commits"),
        pull,
    ))
}

impl Cursor {
    fn open(
        request: &PullCommitRequest,
        path: &str,
        pull: u64,
        http: &GithubHttp,
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
                repository_native: request.repository.provider_id.clone(),
                subject: request.subject.id.clone(),
                subject_native: request.subject.provider_id.clone(),
                pull,
                context: request.context.clone(),
                pages: 0,
                seen: 0,
                url: http
                    .endpoint(&format!(
                        "{}?per_page={PAGE_SIZE}",
                        path.trim_start_matches('/')
                    ))?
                    .to_string(),
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
            || cursor.repository_native != request.repository.provider_id
            || cursor.subject != request.subject.id
            || cursor.subject_native != request.subject.provider_id
            || cursor.pull != pull
            || cursor.context != request.context
            || cursor.pages == 0
            || cursor.pages >= MAX_PAGES
            || cursor.seen != cursor.pages.saturating_mul(PAGE_SIZE)
            || cursor.seen != request.start_position
        {
            return Err(invalid());
        }
        let (_, page) = http.check_collection_url(&cursor.url, path)?;
        if page != u64::from(cursor.pages + 1) {
            return Err(invalid());
        }
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

fn git_actor(value: Option<&Value>) -> Result<(PullCommitActor, Option<String>), ProviderError> {
    let value = value.and_then(Value::as_object).ok_or_else(invalid)?;
    let name = required_text(value.get("name"), 1024, false)?;
    if name.chars().any(char::is_control) {
        return Err(invalid());
    }
    Ok((
        PullCommitActor {
            name,
            provider: None,
        },
        optional_time(value.get("date"))?,
    ))
}

fn github_web(raw: &str, oid: &str, account: bool) -> Result<String, ProviderError> {
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
    if url.as_str() != raw
        || url.scheme() != "https"
        || url.host_str() != Some("github.com")
        || url.port().is_some()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(invalid());
    }
    let segments: Vec<_> = url.path().trim_matches('/').split('/').collect();
    let valid = if account {
        segments.len() == 1 && !segments[0].is_empty()
    } else {
        segments.len() == 4
            && segments[2] == "commit"
            && segments[3] == oid
            && valid_repository_path(&format!("{}/{}", segments[0], segments[1]))
    };
    if !valid {
        return Err(invalid());
    }
    Ok(raw.into())
}

fn provider_actor(value: Option<&Value>) -> Result<Option<DetailActor>, ProviderError> {
    let Some(value) = value else {
        return Ok(None);
    };
    if value.is_null() {
        return Ok(None);
    }
    let value = value.as_object().ok_or_else(invalid)?;
    let id = value
        .get("id")
        .and_then(Value::as_u64)
        .filter(|id| *id > 0)
        .ok_or_else(invalid)?;
    let login = required_text(value.get("login"), 255, false)?;
    if login.chars().any(char::is_control) {
        return Err(invalid());
    }
    let web_url = match value.get("html_url") {
        None | Some(Value::Null) => None,
        Some(Value::String(raw)) => Some(github_web(raw, "", true)?),
        _ => return Err(invalid()),
    };
    Ok(Some(DetailActor {
        provider_id: id.to_string(),
        login,
        web_url,
    }))
}

fn observed_message(value: Option<&Value>) -> Result<(PullCommitMessage, String), ProviderError> {
    let Some(value) = value else {
        return Ok((
            PullCommitMessage {
                state: PullCommitMessageState::Omitted,
                text: None,
            },
            String::new(),
        ));
    };
    if value.is_null() {
        return Ok((
            PullCommitMessage {
                state: PullCommitMessageState::Omitted,
                text: None,
            },
            String::new(),
        ));
    }
    let text = value.as_str().ok_or_else(invalid)?;
    if text.contains('\0') {
        return Err(invalid());
    }
    let summary = text.lines().next().unwrap_or_default();
    if summary.len() > MAX_SUMMARY_BYTES {
        return Err(invalid());
    }
    let message = if text.len() <= MAX_PULL_COMMIT_MESSAGE_BYTES {
        PullCommitMessage {
            state: PullCommitMessageState::Known,
            text: Some(text.into()),
        }
    } else {
        PullCommitMessage {
            state: PullCommitMessageState::Oversized,
            text: None,
        }
    };
    Ok((message, summary.into()))
}

fn commit(row: &Map<String, Value>) -> Result<ProviderPullCommit, ProviderError> {
    let oid = required_text(row.get("sha"), 64, false)?;
    if !is_canonical_commit_oid(&oid) {
        return Err(invalid());
    }
    let facts = row
        .get("commit")
        .and_then(Value::as_object)
        .ok_or_else(invalid)?;
    let (message, summary) = observed_message(facts.get("message"))?;
    let (mut author, authored_at) = git_actor(facts.get("author"))?;
    let (mut committer, committed_at) = git_actor(facts.get("committer"))?;
    author.provider = provider_actor(row.get("author"))?;
    committer.provider = provider_actor(row.get("committer"))?;
    let parents = row
        .get("parents")
        .and_then(Value::as_array)
        .ok_or_else(invalid)?;
    if parents.len() > MAX_PULL_COMMIT_PARENTS {
        return Err(invalid());
    }
    let mut parent_oids = Vec::with_capacity(parents.len());
    let mut unique = HashSet::new();
    for parent in parents {
        let parent = parent.as_object().ok_or_else(invalid)?;
        let parent = required_text(parent.get("sha"), 64, false)?;
        if !is_canonical_commit_oid(&parent) || !unique.insert(parent.clone()) {
            return Err(invalid());
        }
        parent_oids.push(parent);
    }
    let web_url = match row.get("html_url") {
        None | Some(Value::Null) => None,
        Some(Value::String(raw)) => Some(github_web(raw, &oid, false)?),
        _ => return Err(invalid()),
    };
    Ok(ProviderPullCommit {
        oid,
        summary,
        message,
        author,
        committer: Some(committer),
        authored_at,
        committed_at,
        parent_oids,
        web_url,
    })
}

impl GithubProvider {
    pub(super) async fn request_pull_commits(
        &self,
        token: &SecretToken,
        request: PullCommitRequest,
    ) -> Result<PullCommitProviderPage, ProviderError> {
        let (path, pull) = identity(&request)?;
        let mut cursor = Cursor::open(&request, &path, pull, &self.http)?;
        let response = self
            .http
            .get_collection(
                reqwest::Url::parse(&cursor.url).map_err(|_| invalid())?,
                token,
                &path,
                u64::from(cursor.pages + 1),
            )
            .await?;
        let start_position = cursor.seen;
        let result = (|| {
            let rows: Vec<Map<String, Value>> =
                serde_json::from_slice(&response.body).map_err(|_| invalid())?;
            if rows.len() > PAGE_SIZE as usize
                || response.next_url.is_some() && rows.len() != PAGE_SIZE as usize
            {
                return Err(invalid());
            }
            let mut identities = HashSet::new();
            let mut commits = Vec::with_capacity(rows.len());
            for row in rows {
                let commit = commit(&row)?;
                if !identities.insert(commit.oid.clone()) {
                    return Err(invalid());
                }
                commits.push(commit);
            }
            cursor.pages += 1;
            cursor.seen = cursor
                .seen
                .checked_add(commits.len() as u32)
                .ok_or_else(invalid)?;
            if cursor.seen > PROVIDER_LIMIT {
                return Err(invalid());
            }
            let provider_capped = cursor.seen == PROVIDER_LIMIT;
            // GitHub documents this endpoint as exposing at most 250 commits.
            // Reaching that boundary cannot prove remote exhaustion even when
            // the response omits a next Link.
            let remote_has_more = provider_capped || response.next_url.is_some();
            let next_cursor = if provider_capped {
                None
            } else {
                response
                    .next_url
                    .map(|next| {
                        cursor.url = next;
                        cursor.encoded()
                    })
                    .transpose()?
            };
            Ok(PullCommitProviderPage {
                context: request.context.clone(),
                commits,
                order: PullCommitProviderOrder::BaseToHead,
                source: PullCommitSource {
                    source: SOURCE.into(),
                    adapter_version: 1,
                },
                start_position,
                next_cursor,
                cap_reason: provider_capped.then_some(PullCommitCapReason::ProviderLimit),
                remote_has_more,
                freshness_seconds: 60,
                cooldown_seconds: response.cooldown_seconds,
            })
        })();
        result.map_err(|mut error: ProviderError| {
            error.account_cooldown_seconds = error
                .account_cooldown_seconds
                .into_iter()
                .chain(response.cooldown_seconds)
                .max();
            error
        })
    }
}
