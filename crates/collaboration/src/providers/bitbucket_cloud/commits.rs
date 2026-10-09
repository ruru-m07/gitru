//! Pull-request commit pages with opaque continuation and exact range binding.
use super::*;
use crate::DetailActor;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::HashSet;

pub(super) const SOURCE: &str = "bitbucket_cloud/pull-request-commits/v2";
const STRATEGY: &str = "pull_commits_v1";
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
    subject: String,
    subject_native: String,
    repository: String,
    source_repository: String,
    pull: u64,
    context: PullCommitContext,
    url: String,
    pages: u32,
    seen: u32,
    seen_pages: Vec<String>,
}

#[derive(Deserialize)]
struct Collection {
    values: Vec<Map<String, Value>>,
    next: Option<String>,
}

fn bounded_identity(value: &str, maximum: usize) -> bool {
    !value.is_empty() && value.len() <= maximum && !value.chars().any(char::is_control)
}

fn identity(request: &PullCommitRequest) -> Result<(String, String, u64), ProviderError> {
    let repository = resource_details::repository_identity(&request.account, &request.repository)?;
    let source_repository = canonical_uuid(&request.context.source_repository_provider_id)?;
    let pull = request
        .subject
        .number
        .as_deref()
        .and_then(resource_details::positive_id)
        .filter(|id| *id <= i64::MAX as u64)
        .ok_or_else(invalid)?;
    if source_repository != request.context.source_repository_provider_id
        || request.subject.kind != RemoteItemKind::PullRequest
        || request.subject.account_id != request.account.id
        || request.subject.repository_id.as_ref() != Some(&request.repository.id)
        || request.subject.provider_id != format!("{repository}:{pull}")
        || request.subject.id != format!("bitbucket_cloud:pull:{repository}:{pull}")
        || !bounded_identity(&request.context.metadata_facet_revision, 256)
        || !is_canonical_commit_oid(&request.context.base_oid)
        || !is_canonical_commit_oid(&request.context.head_oid)
    {
        return Err(invalid());
    }
    Ok((repository, source_repository, pull))
}

impl Cursor {
    fn open(
        request: &PullCommitRequest,
        repository: &str,
        source_repository: &str,
        pull: u64,
        http: &BitbucketHttp,
        route: &Route,
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
                subject: request.subject.id.clone(),
                subject_native: request.subject.provider_id.clone(),
                repository: repository.into(),
                source_repository: source_repository.into(),
                pull,
                context: request.context.clone(),
                url: http.endpoint(route)?.to_string(),
                pages: 0,
                seen: 0,
                seen_pages: vec![],
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
            || cursor.subject != request.subject.id
            || cursor.subject_native != request.subject.provider_id
            || cursor.repository != repository
            || cursor.source_repository != source_repository
            || cursor.pull != pull
            || cursor.context != request.context
            || cursor.pages == 0
            || cursor.pages >= MAX_PULL_COMMIT_PAGES
            || cursor.seen == 0
            || cursor.seen >= MAX_PULL_COMMITS
            || cursor.seen != request.start_position
            || cursor.seen_pages.len() != cursor.pages as usize
        {
            return Err(invalid());
        }
        let mut hashes = HashSet::new();
        for hash in &cursor.seen_pages {
            if hash.len() != 64
                || !hash
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
                || !hashes.insert(hash.clone())
            {
                return Err(invalid());
            }
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

fn provider_actor(value: Option<&Value>) -> Result<Option<DetailActor>, ProviderError> {
    match value {
        None | Some(Value::Null) => Ok(None),
        Some(value) => resource_details::actor(value).map(Some),
    }
}

fn raw_name(raw: &str) -> Result<String, ProviderError> {
    if raw.len() > 2048 || raw.contains(['\n', '\r', '\0']) {
        return Err(invalid());
    }
    let name = raw
        .strip_suffix('>')
        .and_then(|value| value.rsplit_once(" <").map(|(name, _)| name))
        .unwrap_or(raw);
    text(name.into(), 1024, false)
}

fn actor(value: Option<&Value>) -> Result<PullCommitActor, ProviderError> {
    let value = value.and_then(Value::as_object).ok_or_else(invalid)?;
    if value
        .get("type")
        .is_some_and(|kind| kind.as_str() != Some("author"))
    {
        return Err(invalid());
    }
    Ok(PullCommitActor {
        name: raw_name(
            value
                .get("raw")
                .and_then(Value::as_str)
                .ok_or_else(invalid)?,
        )?,
        provider: provider_actor(value.get("user"))?,
    })
}

fn observed_message(value: Option<&Value>) -> Result<(PullCommitMessage, String), ProviderError> {
    match value {
        None | Some(Value::Null) => Ok((
            PullCommitMessage {
                state: PullCommitMessageState::Omitted,
                text: None,
            },
            String::new(),
        )),
        Some(Value::String(value)) if !value.contains('\0') => {
            let summary = value.lines().next().unwrap_or_default();
            if summary.len() > MAX_SUMMARY_BYTES {
                return Err(invalid());
            }
            let message = if value.len() <= MAX_PULL_COMMIT_MESSAGE_BYTES {
                PullCommitMessage {
                    state: PullCommitMessageState::Known,
                    text: Some(value.clone()),
                }
            } else {
                PullCommitMessage {
                    state: PullCommitMessageState::Oversized,
                    text: None,
                }
            };
            Ok((message, summary.into()))
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
    let suffix = format!("/commits/{oid}");
    let repository = url
        .path()
        .strip_suffix(&suffix)
        .and_then(|path| path.strip_prefix('/'))
        .ok_or_else(invalid)?;
    if url.as_str() != raw
        || url.scheme() != "https"
        || url.host_str() != Some("bitbucket.org")
        || url.port().is_some()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || repository.split('/').count() != 2
        || !repository.split('/').all(discovery::segment)
    {
        return Err(invalid());
    }
    Ok(raw.into())
}

fn nested_web(value: Option<&Value>, oid: &str) -> Result<Option<String>, ProviderError> {
    let Some(value) = value else {
        return Ok(None);
    };
    let links = value.as_object().ok_or_else(invalid)?;
    let Some(html) = links.get("html") else {
        return Ok(None);
    };
    let html = html.as_object().ok_or_else(invalid)?;
    match html.get("href") {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(raw)) => commit_web(raw, oid).map(Some),
        _ => Err(invalid()),
    }
}

fn repository_evidence(
    value: Option<&Value>,
    repository: &str,
    source_repository: &str,
) -> Result<(), ProviderError> {
    let Some(value) = value else {
        return Ok(());
    };
    if value.is_null() {
        return Ok(());
    }
    let value = value.as_object().ok_or_else(invalid)?;
    let observed = canonical_uuid(
        value
            .get("uuid")
            .and_then(Value::as_str)
            .ok_or_else(invalid)?,
    )?;
    if observed != repository && observed != source_repository {
        return Err(invalid());
    }
    Ok(())
}

fn commit(
    row: &Map<String, Value>,
    repository: &str,
    source_repository: &str,
) -> Result<ProviderPullCommit, ProviderError> {
    if row
        .get("type")
        .is_some_and(|value| value.as_str() != Some("commit"))
    {
        return Err(invalid());
    }
    repository_evidence(row.get("repository"), repository, source_repository)?;
    let oid = required_text(row.get("hash"), 64, false)?;
    if !is_canonical_commit_oid(&oid) {
        return Err(invalid());
    }
    let (message, summary) = observed_message(row.get("message"))?;
    let parent_values = row
        .get("parents")
        .and_then(Value::as_array)
        .ok_or_else(invalid)?;
    if parent_values.len() > MAX_PULL_COMMIT_PARENTS {
        return Err(invalid());
    }
    let mut parent_oids = Vec::with_capacity(parent_values.len());
    let mut parents = HashSet::new();
    for parent in parent_values {
        let parent = parent.as_object().ok_or_else(invalid)?;
        let parent = required_text(parent.get("hash"), 64, false)?;
        if !is_canonical_commit_oid(&parent) || !parents.insert(parent.clone()) {
            return Err(invalid());
        }
        parent_oids.push(parent);
    }
    Ok(ProviderPullCommit {
        oid: oid.clone(),
        summary,
        message,
        author: actor(row.get("author"))?,
        // Bitbucket Cloud's Commit representation does not expose the Git
        // committer. Keep that fact unknown instead of copying the author.
        committer: None,
        authored_at: None,
        committed_at: optional_time(row.get("date"))?,
        parent_oids,
        web_url: nested_web(row.get("links"), &oid)?,
    })
}

impl BitbucketCloudProvider {
    pub(super) async fn pull_commits(
        &self,
        token: &SecretToken,
        request: PullCommitRequest,
    ) -> Result<PullCommitProviderPage, ProviderError> {
        let (repository, source_repository, pull) = identity(&request)?;
        let route = Route::Commits(repository.clone(), pull);
        let mut cursor = Cursor::open(
            &request,
            &repository,
            &source_repository,
            pull,
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
        let start_position = cursor.seen;
        let result = (|| {
            let collection: Collection =
                serde_json::from_slice(&response.body).map_err(|_| invalid())?;
            if collection.values.len() > 50
                || collection.next.is_some() && collection.values.is_empty()
            {
                return Err(invalid());
            }
            let mut identities = HashSet::new();
            let mut commits = Vec::with_capacity(collection.values.len());
            for row in collection.values {
                let commit = commit(&row, &repository, &source_repository)?;
                if !identities.insert(commit.oid.clone()) {
                    return Err(invalid());
                }
                commits.push(commit);
            }
            // Bitbucket Cloud returns the source head first and then walks
            // backwards. A merged pull can expose target-side commits before
            // that head; those rows are not members of the captured source
            // range, so reject the response instead of silently skipping them.
            if start_position == 0
                && commits
                    .first()
                    .is_some_and(|commit| commit.oid != request.context.head_oid)
            {
                return Err(invalid());
            }
            let remaining = MAX_PULL_COMMITS.saturating_sub(start_position) as usize;
            let truncated = commits.len() > remaining;
            commits.truncate(remaining);
            cursor.seen = cursor
                .seen
                .checked_add(commits.len() as u32)
                .ok_or_else(invalid)?;
            cursor
                .seen_pages
                .push(self.http.fingerprint(&cursor.url, &route)?);
            cursor.pages += 1;
            let hit_page_cap = cursor.pages >= MAX_PULL_COMMIT_PAGES && collection.next.is_some();
            let hit_row_cap =
                cursor.seen >= MAX_PULL_COMMITS && (collection.next.is_some() || truncated);
            let capped = truncated || hit_page_cap || hit_row_cap;
            let remote_has_more = collection.next.is_some() || truncated;
            let next_cursor = if capped {
                None
            } else {
                collection
                    .next
                    .map(|next| {
                        cursor.url = self.http.continuation(&next, &route)?.to_string();
                        cursor.encoded(&self.http, &route)
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
                cap_reason: capped.then_some(PullCommitCapReason::LocalLimit),
                remote_has_more,
                freshness_seconds: 60,
                cooldown_seconds: response.cooldown,
            })
        })();
        result.map_err(|error| quota(error, response.cooldown))
    }
}
