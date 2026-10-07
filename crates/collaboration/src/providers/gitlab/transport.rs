//! GitLab.com-only token transport. Response URLs never expand its authority.
use super::super::{ProviderError, ProviderErrorKind};
use crate::credentials::SecretToken;
use reqwest::{Client, StatusCode, Url, header};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const MAX_BODY: usize = 4 * 1024 * 1024;
const MAX_HEADER: usize = 8192;
const MAX_REDIRECTS: usize = 3;
pub(super) const PROJECT_QUERY: &str =
    "membership=true&pagination=keyset&order_by=id&sort=asc&per_page=50";
pub(super) const MERGE_REQUEST_QUERY: &str =
    "state=all&scope=all&order_by=created_at&sort=asc&per_page=50";
pub(super) const ISSUE_QUERY: &str =
    "state=all&scope=all&issue_type=issue&pagination=keyset&order_by=id&sort=asc&per_page=50";
const PULL_COMMIT_PAGE_SIZE: u64 = 100;
const CHECK_PAGE_SIZE: u64 = 100;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ItemRoute {
    MergeRequests,
    Issues,
}
impl ItemRoute {
    fn segment(self) -> &'static str {
        match self {
            Self::MergeRequests => "merge_requests",
            Self::Issues => "issues",
        }
    }
    fn query(self) -> &'static str {
        match self {
            Self::MergeRequests => MERGE_REQUEST_QUERY,
            Self::Issues => ISSUE_QUERY,
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum FeedPosition {
    Offset(u64),
    Keyset {
        cursor: Option<String>,
        after: Option<u64>,
    },
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum Operation {
    User,
    Projects,
    Feed { project: u64, route: ItemRoute },
    Detail,
    PullCommits { project: u64, iid: u64, page: u64 },
    PullFiles { project: u64, iid: u64, page: u64 },
    SelectedPullFile { project: u64, iid: u64, page: u64 },
    CommitStatuses { project: u64, page: u64 },
}

pub(super) struct GitlabHttp {
    client: Client,
    base: Url,
}

pub(super) struct Page {
    pub body: Vec<u8>,
    pub next: Option<String>,
    pub cooldown: Option<u64>,
    pub total: Option<u64>,
}

impl GitlabHttp {
    pub(super) fn new() -> Result<Self, ProviderError> {
        Self::for_base(Url::parse("https://gitlab.com/api/v4/").expect("fixed GitLab API"))
    }
    fn for_base(base: Url) -> Result<Self, ProviderError> {
        let client = Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .connect_timeout(Duration::from_secs(5))
            .timeout(Duration::from_secs(20))
            .pool_idle_timeout(Duration::from_secs(60))
            .https_only(base.scheme() == "https")
            .build()
            .map_err(|_| ProviderError::new(ProviderErrorKind::Unavailable))?;
        Ok(Self { client, base })
    }
    #[cfg(test)]
    pub(super) fn fixture(base: Url) -> Self {
        Self::for_base(base).expect("isolated fixture transport")
    }
    pub(super) fn user(&self) -> Url {
        self.base.join("user").expect("constant route")
    }
    pub(super) fn projects(&self) -> Url {
        self.base
            .join(&format!("projects?{PROJECT_QUERY}"))
            .expect("constant route")
    }
    pub(super) fn continuation(&self, raw: &str) -> Result<Url, ProviderError> {
        let url = self.check_raw(raw)?;
        if url.path() != self.projects().path() || project_after(&url)?.is_none() {
            return Err(invalid());
        }
        Ok(url)
    }
    pub(super) fn resource_feed(
        &self,
        project: u64,
        route: ItemRoute,
    ) -> Result<Url, ProviderError> {
        if project == 0 {
            return Err(invalid());
        }
        let mut url = self
            .base
            .join(&format!("projects/{project}/{}", route.segment()))
            .map_err(|_| invalid())?;
        url.set_query(Some(route.query()));
        if route == ItemRoute::MergeRequests {
            url.query_pairs_mut().append_pair("page", "1");
        }
        Ok(url)
    }
    pub(super) fn resource_detail(
        &self,
        project: u64,
        iid: u64,
        route: ItemRoute,
    ) -> Result<Url, ProviderError> {
        if project == 0 || iid == 0 {
            return Err(invalid());
        }
        self.base
            .join(&format!("projects/{project}/{}/{iid}", route.segment()))
            .map_err(|_| invalid())
    }
    pub(super) fn pull_commits(&self, project: u64, iid: u64) -> Result<Url, ProviderError> {
        if project == 0 || iid == 0 {
            return Err(invalid());
        }
        let mut url = self
            .base
            .join(&format!("projects/{project}/merge_requests/{iid}/commits"))
            .map_err(|_| invalid())?;
        url.query_pairs_mut()
            .append_pair("per_page", &PULL_COMMIT_PAGE_SIZE.to_string())
            .append_pair("page", "1");
        Ok(url)
    }
    pub(super) fn commit_statuses(&self, project: u64, head: &str) -> Result<Url, ProviderError> {
        if project == 0 || !crate::is_canonical_commit_oid(head) {
            return Err(invalid());
        }
        let mut url = self
            .base
            .join(&format!(
                "projects/{project}/repository/commits/{head}/statuses"
            ))
            .map_err(|_| invalid())?;
        url.query_pairs_mut()
            .append_pair("all", "false")
            .append_pair("order_by", "id")
            .append_pair("sort", "asc")
            .append_pair("per_page", &CHECK_PAGE_SIZE.to_string())
            .append_pair("page", "1");
        Ok(url)
    }
    pub(super) fn pull_commit_continuation(
        &self,
        raw: &str,
        project: u64,
        iid: u64,
        expected_page: u64,
    ) -> Result<Url, ProviderError> {
        let url = self.check_resource_raw(raw)?;
        if self.operation(&url)?
            != (Operation::PullCommits {
                project,
                iid,
                page: expected_page,
            })
        {
            return Err(invalid());
        }
        Ok(url)
    }
    pub(super) fn pull_files(&self, project: u64, iid: u64) -> Result<Url, ProviderError> {
        if project == 0 || iid == 0 {
            return Err(invalid());
        }
        let mut url = self
            .base
            .join(&format!("projects/{project}/merge_requests/{iid}/diffs"))
            .map_err(|_| invalid())?;
        url.query_pairs_mut()
            .append_pair("per_page", &PULL_COMMIT_PAGE_SIZE.to_string())
            .append_pair("page", "1");
        Ok(url)
    }
    pub(super) fn selected_pull_file(
        &self,
        project: u64,
        iid: u64,
        ordinal: u32,
    ) -> Result<Url, ProviderError> {
        if ordinal >= crate::MAX_PULL_FILES {
            return Err(invalid());
        }
        let mut url = self.pull_files(project, iid)?;
        url.set_query(Some(&format!("per_page=1&page={}", ordinal + 1)));
        Ok(url)
    }
    pub(super) fn pull_file_continuation(
        &self,
        raw: &str,
        project: u64,
        iid: u64,
        expected_page: u64,
    ) -> Result<Url, ProviderError> {
        let url = self.check_resource_raw(raw)?;
        if self.operation(&url)?
            != (Operation::PullFiles {
                project,
                iid,
                page: expected_page,
            })
        {
            return Err(invalid());
        }
        Ok(url)
    }
    pub(super) fn commit_status_continuation(
        &self,
        raw: &str,
        project: u64,
        head: &str,
        expected_page: u64,
    ) -> Result<Url, ProviderError> {
        let url = self.check_resource_raw(raw)?;
        if !crate::is_canonical_commit_oid(head)
            || self.operation(&url)?
                != (Operation::CommitStatuses {
                    project,
                    page: expected_page,
                })
            || url.path() != self.commit_statuses(project, head)?.path()
        {
            return Err(invalid());
        }
        Ok(url)
    }
    pub(super) fn resource_continuation(
        &self,
        raw: &str,
        project: u64,
        route: ItemRoute,
    ) -> Result<Url, ProviderError> {
        let url = self.check_resource_raw(raw)?;
        if url.path() != self.resource_feed(project, route)?.path() {
            return Err(invalid());
        }
        match feed_position(&url, route)? {
            FeedPosition::Offset(page) if page > 1 => {}
            FeedPosition::Keyset { cursor, after } if cursor.is_some() || after.is_some() => {}
            _ => return Err(invalid()),
        }
        Ok(url)
    }
    // Resource cursors can contain an encoded opaque query value. Numeric
    // routes remain literal; decoded query keys/values have a finite allowlist.
    fn check_resource_raw(&self, raw: &str) -> Result<Url, ProviderError> {
        if raw.len() > 2048
            || raw.trim() != raw
            || raw.chars().any(|c| c.is_control() || c.is_whitespace())
            || raw.contains(['\\', '#'])
            || raw.split('?').next().is_none_or(|path| {
                path.contains('%') || path.split('/').any(|part| matches!(part, "." | ".."))
            })
        {
            return Err(invalid());
        }
        let url = Url::parse(raw).map_err(|_| invalid())?;
        if url.origin() != self.base.origin()
            || !url.username().is_empty()
            || url.password().is_some()
            || url.fragment().is_some()
        {
            return Err(invalid());
        }
        Ok(url)
    }
    fn operation(&self, url: &Url) -> Result<Operation, ProviderError> {
        if *url == self.user() {
            return Ok(Operation::User);
        }
        if url.path() == self.projects().path() {
            project_after(url)?;
            return Ok(Operation::Projects);
        }
        let prefix = format!("{}projects/", self.base.path());
        let parts: Vec<_> = url
            .path()
            .strip_prefix(&prefix)
            .ok_or_else(invalid)?
            .split('/')
            .collect();
        let project = parts
            .first()
            .and_then(|v| positive_id(v))
            .ok_or_else(invalid)?;
        if parts.len() == 4
            && parts.get(1) == Some(&"merge_requests")
            && parts.get(3) == Some(&"commits")
        {
            let iid = parts
                .get(2)
                .and_then(|value| positive_id(value))
                .ok_or_else(invalid)?;
            return Ok(Operation::PullCommits {
                project,
                iid,
                page: pull_commit_page(url, project, iid)?,
            });
        }
        if parts.len() == 4
            && parts.get(1) == Some(&"merge_requests")
            && parts.get(3) == Some(&"diffs")
        {
            let iid = parts
                .get(2)
                .and_then(|value| positive_id(value))
                .ok_or_else(invalid)?;
            if url
                .query_pairs()
                .any(|(key, value)| key == "per_page" && value == "1")
            {
                let page = pull_file_page(url, project, iid, "1")?;
                if page > u64::from(crate::MAX_PULL_FILES) {
                    return Err(invalid());
                }
                return Ok(Operation::SelectedPullFile { project, iid, page });
            }
            return Ok(Operation::PullFiles {
                project,
                iid,
                page: pull_commit_page(url, project, iid)?,
            });
        }
        if parts.len() == 5
            && parts.get(1) == Some(&"repository")
            && parts.get(2) == Some(&"commits")
            && parts.get(4) == Some(&"statuses")
            && parts
                .get(3)
                .is_some_and(|head| crate::is_canonical_commit_oid(head))
        {
            return Ok(Operation::CommitStatuses {
                project,
                page: commit_status_page(url, project, parts[3])?,
            });
        }
        let route = match parts.get(1) {
            Some(&"merge_requests") => ItemRoute::MergeRequests,
            Some(&"issues") => ItemRoute::Issues,
            _ => return Err(invalid()),
        };
        match parts.len() {
            2 => {
                feed_position(url, route)?;
                Ok(Operation::Feed { project, route })
            }
            3 if parts.get(2).and_then(|v| positive_id(v)).is_some() && url.query().is_none() => {
                Ok(Operation::Detail)
            }
            _ => Err(invalid()),
        }
    }
    fn check_raw(&self, raw: &str) -> Result<Url, ProviderError> {
        if raw.len() > 2048
            || raw.trim() != raw
            || raw.chars().any(|c| c.is_control() || c.is_whitespace())
            || raw.contains(['%', '\\', '#'])
            || raw.split(['/', '?']).any(|p| matches!(p, "." | ".."))
        {
            return Err(invalid());
        }
        let url = Url::parse(raw).map_err(|_| invalid())?;
        if url.origin() != self.base.origin()
            || !url.username().is_empty()
            || url.password().is_some()
            || url.fragment().is_some()
        {
            return Err(invalid());
        }
        Ok(url)
    }
    fn check_operation(&self, raw: &str, original: &Url) -> Result<Url, ProviderError> {
        let url = if matches!(
            self.operation(original)?,
            Operation::User | Operation::Projects
        ) {
            self.check_raw(raw)?
        } else {
            self.check_resource_raw(raw)?
        };
        if url.path() != original.path() || url.query() != original.query() {
            return Err(invalid());
        }
        Ok(url)
    }
    pub(super) async fn get(
        &self,
        original: Url,
        token: &SecretToken,
    ) -> Result<Page, ProviderError> {
        let operation = self.operation(&original)?;
        let mut url = self.check_operation(original.as_str(), &original)?;
        let mut observed_cooldown = None;
        // One whole operation deadline also bounds redirect chains and chunking.
        tokio::time::timeout(Duration::from_secs(20), async {
            for redirect in 0..=MAX_REDIRECTS {
                let mut secret = header::HeaderValue::from_str(token.expose())
                    .map_err(|_| ProviderError::new(ProviderErrorKind::Authentication))?;
                secret.set_sensitive(true);
                let mut response = self
                    .client
                    .get(url.clone())
                    .header("PRIVATE-TOKEN", secret)
                    .header(header::ACCEPT, "application/json")
                    .header(header::USER_AGENT, "Gitru-Desktop")
                    .send()
                    .await
                    .map_err(|e| {
                        with_quota(
                            ProviderError::new(if e.is_connect() || e.is_timeout() {
                                ProviderErrorKind::Offline
                            } else {
                                ProviderErrorKind::Unavailable
                            }),
                            observed_cooldown,
                        )
                    })?;
                let status = response.status();
                let headers = response.headers();
                let total = if matches!(operation, Operation::CommitStatuses { .. }) {
                    optional_number(headers, "x-total")?
                } else {
                    None
                };
                let retry = retry_after(headers);
                let reset =
                    number(headers, "ratelimit-reset").map(|n| n.saturating_sub(now()).max(1));
                let cooldown = (number(headers, "ratelimit-remaining") == Some(0))
                    .then(|| reset.unwrap_or(60));
                observed_cooldown = max_wait(observed_cooldown, cooldown);
                if status == StatusCode::UNAUTHORIZED {
                    return Err(with_quota(
                        ProviderError::new(ProviderErrorKind::Authentication),
                        observed_cooldown,
                    ));
                }
                if status == StatusCode::FORBIDDEN {
                    return Err(with_quota(
                        ProviderError::new(ProviderErrorKind::Permission),
                        observed_cooldown,
                    ));
                }
                if status == StatusCode::TOO_MANY_REQUESTS {
                    let wait = max_wait(retry, max_wait(reset, observed_cooldown)).unwrap_or(60);
                    return Err(ProviderError {
                        kind: ProviderErrorKind::RateLimited,
                        retry_after_seconds: Some(wait),
                        account_cooldown_seconds: Some(wait),
                    });
                }
                if status.is_redirection() {
                    if status == StatusCode::NOT_MODIFIED {
                        return Err(with_quota(invalid(), observed_cooldown));
                    }
                    if let Some(wait) = retry {
                        // RFC 9110 makes this a minimum delay before following
                        // a 3xx. Return it to the scheduler; never sleep while
                        // holding the HTTP lane or follow the redirect early.
                        return Err(ProviderError {
                            kind: ProviderErrorKind::Unavailable,
                            retry_after_seconds: Some(wait),
                            account_cooldown_seconds: observed_cooldown,
                        });
                    }
                    if redirect == MAX_REDIRECTS {
                        return Err(with_quota(invalid(), observed_cooldown));
                    }
                    // A depleted successful/redirect response is not permission
                    // to send another request before its reported reset.
                    if let Some(wait) = observed_cooldown {
                        return Err(ProviderError {
                            kind: ProviderErrorKind::RateLimited,
                            retry_after_seconds: Some(wait),
                            account_cooldown_seconds: Some(wait),
                        });
                    }
                    let location = text(headers, "location")?.ok_or_else(invalid)?;
                    let raw = if location.starts_with('/') && !location.starts_with("//") {
                        format!("{}{}", self.base.origin().ascii_serialization(), location)
                    } else {
                        location
                    };
                    url = self
                        .check_operation(&raw, &original)
                        .map_err(|e| with_quota(e, observed_cooldown))?;
                    continue;
                }
                if status == StatusCode::NOT_FOUND || status == StatusCode::GONE {
                    return Err(with_quota(
                        ProviderError::new(ProviderErrorKind::NotFound),
                        observed_cooldown,
                    ));
                }
                if status.is_server_error()
                    || matches!(status, StatusCode::REQUEST_TIMEOUT | StatusCode::CONFLICT)
                {
                    return Err(ProviderError {
                        kind: ProviderErrorKind::Unavailable,
                        retry_after_seconds: retry,
                        account_cooldown_seconds: observed_cooldown,
                    });
                }
                if status != StatusCode::OK {
                    return Err(with_quota(invalid(), observed_cooldown));
                }
                let link = text(headers, "link").map_err(|e| with_quota(e, observed_cooldown))?;
                let next = match link {
                    Some(link) => next_link(&link).map_err(|e| with_quota(e, observed_cooldown))?,
                    None => None,
                };
                if let Some(next) = &next {
                    let validated = match operation {
                        Operation::Projects => self.continuation(next),
                        Operation::Feed { project, route } => {
                            self.resource_continuation(next, project, route)
                        }
                        Operation::PullCommits { project, iid, page } => self
                            .pull_commit_continuation(
                                next,
                                project,
                                iid,
                                page.checked_add(1).ok_or_else(invalid)?,
                            ),
                        Operation::SelectedPullFile { project, iid, page } => {
                            let url = self.check_resource_raw(next)?;
                            if url.path() != original.path()
                                || pull_file_page(&url, project, iid, "1")? != page + 1
                            {
                                return Err(with_quota(invalid(), observed_cooldown));
                            }
                            Ok(url)
                        }
                        Operation::PullFiles { project, iid, page } => self.pull_file_continuation(
                            next,
                            project,
                            iid,
                            page.checked_add(1).ok_or_else(invalid)?,
                        ),
                        Operation::CommitStatuses { project, page } => {
                            let head = original
                                .path()
                                .split('/')
                                .rev()
                                .nth(1)
                                .ok_or_else(invalid)?;
                            self.commit_status_continuation(
                                next,
                                project,
                                head,
                                page.checked_add(1).ok_or_else(invalid)?,
                            )
                        }
                        _ => Err(invalid()),
                    }
                    .map_err(|e| with_quota(e, observed_cooldown))?;
                    if let Operation::Feed { route, .. } = operation
                        && (validated == original
                            || match (
                                feed_position(&original, route)?,
                                feed_position(&validated, route)?,
                            ) {
                                (FeedPosition::Offset(current), FeedPosition::Offset(next)) => {
                                    current.checked_add(1) != Some(next)
                                }
                                (FeedPosition::Keyset { .. }, FeedPosition::Keyset { .. }) => false,
                                _ => true,
                            })
                    {
                        return Err(with_quota(invalid(), observed_cooldown));
                    }
                }
                if response
                    .content_length()
                    .is_some_and(|n| n > MAX_BODY as u64)
                {
                    return Err(with_quota(invalid(), observed_cooldown));
                }
                let mut body = Vec::new();
                while let Some(chunk) = response.chunk().await.map_err(|_| {
                    with_quota(
                        ProviderError::new(ProviderErrorKind::Unavailable),
                        observed_cooldown,
                    )
                })? {
                    if chunk.len() > MAX_BODY.saturating_sub(body.len()) {
                        return Err(with_quota(invalid(), observed_cooldown));
                    }
                    body.extend_from_slice(&chunk);
                }
                return Ok(Page {
                    body,
                    next,
                    cooldown: observed_cooldown,
                    total,
                });
            }
            unreachable!("bounded redirect loop")
        })
        .await
        .map_err(|_| {
            with_quota(
                ProviderError::new(ProviderErrorKind::Offline),
                observed_cooldown,
            )
        })?
    }
}

fn commit_status_page(url: &Url, project: u64, head: &str) -> Result<u64, ProviderError> {
    let mut pairs = std::collections::HashMap::new();
    for (key, value) in url.query_pairs() {
        if pairs.insert(key.to_string(), value.to_string()).is_some() {
            return Err(invalid());
        }
    }
    for (key, expected) in [
        ("all", "false"),
        ("order_by", "id"),
        ("sort", "asc"),
        ("per_page", "100"),
    ] {
        if pairs.remove(key).as_deref() != Some(expected) {
            return Err(invalid());
        }
    }
    let page = pairs
        .remove("page")
        .as_deref()
        .and_then(positive_id)
        .ok_or_else(invalid)?;
    if pairs
        .remove("id")
        .is_some_and(|value| value != project.to_string())
        || pairs.remove("sha").is_some_and(|value| value != head)
        || !pairs.is_empty()
    {
        return Err(invalid());
    }
    Ok(page)
}

fn pull_commit_page(url: &Url, project: u64, iid: u64) -> Result<u64, ProviderError> {
    pull_file_page(url, project, iid, "100")
}
fn pull_file_page(url: &Url, project: u64, iid: u64, per_page: &str) -> Result<u64, ProviderError> {
    let mut pairs = std::collections::HashMap::new();
    for (key, value) in url.query_pairs() {
        if pairs.insert(key.to_string(), value.to_string()).is_some() {
            return Err(invalid());
        }
    }
    if pairs.remove("per_page").as_deref() != Some(per_page) {
        return Err(invalid());
    }
    let page = pairs
        .remove("page")
        .as_deref()
        .and_then(positive_id)
        .ok_or_else(invalid)?;
    if pairs
        .remove("id")
        .is_some_and(|value| value != project.to_string())
        || pairs
            .remove("merge_request_iid")
            .is_some_and(|value| value != iid.to_string())
        || !pairs.is_empty()
    {
        return Err(invalid());
    }
    Ok(page)
}

pub(super) fn positive_id(raw: &str) -> Option<u64> {
    raw.parse::<u64>()
        .ok()
        .filter(|id| *id > 0 && id.to_string() == raw)
}
pub(super) fn feed_position(url: &Url, route: ItemRoute) -> Result<FeedPosition, ProviderError> {
    let mut pairs = std::collections::HashMap::new();
    for (key, value) in url.query_pairs() {
        if pairs.insert(key.to_string(), value.to_string()).is_some() {
            return Err(invalid());
        }
    }
    for (key, value) in Url::parse(&format!("https://fixture.invalid/?{}", route.query()))
        .map_err(|_| invalid())?
        .query_pairs()
    {
        if pairs.remove(key.as_ref()).as_deref() != Some(value.as_ref()) {
            return Err(invalid());
        }
    }
    let position = match route {
        ItemRoute::MergeRequests => FeedPosition::Offset(
            pairs
                .remove("page")
                .as_deref()
                .and_then(positive_id)
                .ok_or_else(invalid)?,
        ),
        ItemRoute::Issues => {
            let cursor = pairs.remove("cursor");
            if cursor.as_ref().is_some_and(|v| {
                v.is_empty()
                    || v.len() > 1024
                    || !v.bytes().all(|b| {
                        b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'+' | b'/' | b'=')
                    })
            }) {
                return Err(invalid());
            }
            let after = pairs
                .remove("id_after")
                .map(|v| positive_id(&v).ok_or_else(invalid))
                .transpose()?;
            if cursor.is_some() && after.is_some() {
                return Err(invalid());
            }
            FeedPosition::Keyset { cursor, after }
        }
    };
    if !pairs.is_empty() {
        return Err(invalid());
    }
    Ok(position)
}
pub(super) fn project_after(url: &Url) -> Result<Option<u64>, ProviderError> {
    let mut pairs = std::collections::HashMap::new();
    for (key, value) in url.query_pairs() {
        if pairs.insert(key.to_string(), value.to_string()).is_some() {
            return Err(invalid());
        }
    }
    for (key, value) in [
        ("membership", "true"),
        ("pagination", "keyset"),
        ("order_by", "id"),
        ("sort", "asc"),
        ("per_page", "50"),
    ] {
        if pairs.remove(key).as_deref() != Some(value) {
            return Err(invalid());
        }
    }
    let after = pairs
        .remove("id_after")
        .map(|raw| positive_id(&raw).ok_or_else(invalid))
        .transpose()?;
    if !pairs.is_empty() {
        return Err(invalid());
    }
    Ok(after)
}
fn next_link(raw: &str) -> Result<Option<String>, ProviderError> {
    let mut next = None;
    for part in raw.split(',') {
        let (target, parameters) = part.trim().split_once('>').ok_or_else(invalid)?;
        let target = target.strip_prefix('<').ok_or_else(invalid)?;
        if !parameters.starts_with(';') {
            return Err(invalid());
        }
        let mut relation = None;
        for parameter in parameters.split(';').skip(1) {
            let (key, value) = parameter.trim().split_once('=').ok_or_else(invalid)?;
            if key == "rel" {
                let value = if let Some(quoted) = value.strip_prefix('"') {
                    quoted.strip_suffix('"').ok_or_else(invalid)?
                } else {
                    value
                };
                if value.contains('"') || value.is_empty() || relation.replace(value).is_some() {
                    return Err(invalid());
                }
            }
        }
        let relation = relation.ok_or_else(invalid)?;
        if relation.split_ascii_whitespace().any(|rel| rel == "next")
            && next.replace(target.to_string()).is_some()
        {
            return Err(invalid());
        }
    }
    Ok(next)
}
fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
fn number(headers: &header::HeaderMap, key: &str) -> Option<u64> {
    unsigned(headers.get(key)?.to_str().ok()?)
}
fn optional_number(headers: &header::HeaderMap, key: &str) -> Result<Option<u64>, ProviderError> {
    text(headers, key)?
        .map(|value| unsigned(&value).ok_or_else(invalid))
        .transpose()
}
fn unsigned(raw: &str) -> Option<u64> {
    if raw.is_empty() || raw.len() > MAX_HEADER || !raw.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    // Overflow is an unrepresentable future budget, not an absent header
    // that permits an earlier fallback retry. The runtime stores its maximum
    // representable durable timestamp while preserving this native evidence.
    Some(raw.parse().unwrap_or(u64::MAX))
}
fn text(headers: &header::HeaderMap, key: &str) -> Result<Option<String>, ProviderError> {
    if headers.get_all(key).iter().count() > 1 {
        return Err(invalid());
    }
    headers
        .get(key)
        .map(|value| {
            value
                .to_str()
                .ok()
                .filter(|s| s.len() <= MAX_HEADER)
                .map(str::to_string)
                .ok_or_else(invalid)
        })
        .transpose()
}
fn retry_after(headers: &header::HeaderMap) -> Option<u64> {
    let raw = headers.get("retry-after")?.to_str().ok()?;
    unsigned(raw).map(|n| n.max(1)).or_else(|| {
        chrono::DateTime::parse_from_rfc2822(raw)
            .ok()
            .map(|t| (t.timestamp().max(0) as u64).saturating_sub(now()).max(1))
    })
}
pub(super) fn with_quota(mut error: ProviderError, wait: Option<u64>) -> ProviderError {
    error.account_cooldown_seconds = max_wait(error.account_cooldown_seconds, wait);
    error
}
pub(super) fn max_wait(a: Option<u64>, b: Option<u64>) -> Option<u64> {
    a.into_iter().chain(b).max()
}
pub(super) fn invalid() -> ProviderError {
    ProviderError::new(ProviderErrorKind::InvalidResponse)
}
