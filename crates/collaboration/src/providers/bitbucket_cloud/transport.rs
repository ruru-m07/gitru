//! Fixed API origin, sensitive Bearer headers, and bounded opaque continuations.
use super::super::{ProviderError, ProviderErrorKind};
use crate::credentials::SecretToken;
use reqwest::{Client, StatusCode, Url, header};
use std::{collections::BTreeMap, time::Duration};

const MAX_BODY: usize = 4 * 1024 * 1024;
pub(super) const MAX_URL: usize = 512;

#[derive(Clone)]
pub(super) enum Route {
    User,
    Workspaces,
    Repositories(String),
    PullRequests(String),
    PullRequest(String, u64),
    Tasks(String, u64),
    Commits(String, u64),
    PullFiles {
        repository: String,
        base: String,
        head: String,
    },
}

pub(super) struct BitbucketHttp {
    client: Client,
    base: Url,
}

pub(super) struct Page {
    pub body: Vec<u8>,
    pub cooldown: Option<u64>,
}

impl BitbucketHttp {
    pub(super) fn new() -> Result<Self, ProviderError> {
        Self::for_base(Url::parse("https://api.bitbucket.org/2.0/").expect("fixed API URL"))
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
        assert!(base.host_str() == Some("127.0.0.1") && base.path() == "/2.0/");
        Self::for_base(base).expect("isolated fixture HTTP")
    }

    pub(super) fn endpoint(&self, route: &Route) -> Result<Url, ProviderError> {
        let relative = match route {
            Route::User => "user".into(),
            Route::Workspaces => "user/workspaces?pagelen=10".into(),
            Route::Repositories(workspace) => {
                if super::canonical_uuid(workspace)? != *workspace {
                    return Err(invalid());
                }
                format!("repositories/%7B{workspace}%7D?role=member&pagelen=50")
            }
            Route::PullFiles {
                repository,
                base,
                head,
            } => {
                if super::canonical_uuid(repository)? != *repository
                    || !crate::is_canonical_pull_file_oid(base)
                    || !crate::is_canonical_pull_file_oid(head)
                {
                    return Err(invalid());
                }
                // Bitbucket spec order is source..destination (opposite git diff).
                // topic=true selects the PR merge-base comparison explicitly.
                format!(
                    "repositories/%7B%7D/%7B{repository}%7D/diffstat/{head}..{base}?topic=true&renames=true&pagelen=100"
                )
            }
            Route::PullRequests(repository)
            | Route::PullRequest(repository, _)
            | Route::Tasks(repository, _)
            | Route::Commits(repository, _) => {
                if super::canonical_uuid(repository)? != *repository {
                    return Err(invalid());
                }
                let path = format!("repositories/%7B%7D/%7B{repository}%7D/pullrequests");
                match route {
                    Route::PullRequest(_, id) if *id > 0 => format!("{path}/{id}"),
                    Route::Tasks(_, id) if *id > 0 && *id <= i64::MAX as u64 => {
                        format!("{path}/{id}/tasks?pagelen=50")
                    }
                    Route::Commits(_, id) if *id > 0 && *id <= i64::MAX as u64 => {
                        format!("{path}/{id}/commits?pagelen=50")
                    }
                    Route::PullRequests(_) => format!(
                        "{path}?state=OPEN&state=MERGED&state=DECLINED&state=SUPERSEDED&pagelen=50&sort=id"
                    ),
                    _ => return Err(invalid()),
                }
            }
        };
        self.base.join(&relative).map_err(|_| invalid())
    }

    pub(super) fn continuation(&self, raw: &str, route: &Route) -> Result<Url, ProviderError> {
        let url = self.validate(raw, route)?;
        if matches!(route, Route::User | Route::PullRequest(..))
            || !url
                .query_pairs()
                .any(|(key, _)| matches!(key.as_ref(), "page" | "cursor" | "after" | "before"))
        {
            return Err(invalid());
        }
        Ok(url)
    }

    fn validate(&self, raw: &str, route: &Route) -> Result<Url, ProviderError> {
        if raw.len() > MAX_URL
            || raw.trim() != raw
            || raw.chars().any(|c| c.is_control() || c.is_whitespace())
            || raw.contains(['\\', '#'])
            || raw.split('?').next().is_none_or(|path| {
                path.split('/').any(|segment| matches!(segment, "." | ".."))
                    || path.to_ascii_lowercase().contains("%2e")
            })
        {
            return Err(invalid());
        }
        let url = Url::parse(raw).map_err(|_| invalid())?;
        if url.as_str().len() > MAX_URL {
            return Err(invalid());
        }
        let expected = self.endpoint(route)?;
        // Only equivalent brace escaping is an alias; route identities and
        // every other path byte remain fixed to native authority.
        let braces = |path: &str| {
            path.replace("%7B", "{")
                .replace("%7b", "{")
                .replace("%7D", "}")
                .replace("%7d", "}")
        };
        let same_path = braces(url.path()) == braces(expected.path());
        if url.origin() != self.base.origin()
            || !url.username().is_empty()
            || url.password().is_some()
            || url.fragment().is_some()
            || !same_path
        {
            return Err(invalid());
        }
        let mut pairs = BTreeMap::new();
        let mut states = Vec::new();
        for (key, value) in url.query_pairs() {
            if key.len() > 32
                || value.is_empty()
                || value.len() > 256
                || value.chars().any(char::is_control)
            {
                return Err(invalid());
            }
            if key == "state" && matches!(route, Route::PullRequests(_)) {
                states.push(value.to_string());
            } else if pairs.insert(key.to_string(), value.to_string()).is_some() {
                return Err(invalid());
            }
        }
        if matches!(route, Route::PullRequests(_)) {
            states.sort();
            if states != ["DECLINED", "MERGED", "OPEN", "SUPERSEDED"] {
                return Err(invalid());
            }
        }
        for (key, value) in expected.query_pairs() {
            if key != "state" && pairs.remove(key.as_ref()).as_deref() != Some(value.as_ref()) {
                return Err(invalid());
            }
        }
        if pairs
            .keys()
            .any(|key| !matches!(key.as_str(), "page" | "cursor" | "after" | "before"))
            || (matches!(route, Route::User | Route::PullRequest(..)) && !pairs.is_empty())
        {
            return Err(invalid());
        }
        Ok(url)
    }

    pub(super) fn fingerprint(&self, raw: &str, route: &Route) -> Result<String, ProviderError> {
        use sha2::{Digest, Sha256};
        let url = self.validate(raw, route)?;
        // Query ordering and percent-encoding aliases cannot evade the same
        // continuation fingerprint. Values remain opaque; no page is guessed.
        let endpoint = self.endpoint(route)?;
        let encoded = if matches!(route, Route::PullRequests(_)) {
            // New PR cursors retain every repeated state in a sorted multiset.
            let mut pairs: Vec<_> = url.query_pairs().collect();
            pairs.sort();
            serde_json::to_vec(&(endpoint.path(), pairs))
        } else {
            // Preserve the exact v1 discovery fingerprint bytes so persisted
            // workspace/repository histories still reject their old targets.
            let pairs: BTreeMap<_, _> = url.query_pairs().collect();
            serde_json::to_vec(&(endpoint.path(), pairs))
        }
        .map_err(|_| invalid())?;
        Ok(format!("{:x}", Sha256::digest(encoded)))
    }

    pub(super) async fn get(
        &self,
        url: Url,
        route: &Route,
        token: &SecretToken,
    ) -> Result<Page, ProviderError> {
        let url = self.validate(url.as_str(), route)?;
        let mut authorization =
            header::HeaderValue::from_str(&format!("Bearer {}", token.expose()))
                .map_err(|_| ProviderError::new(ProviderErrorKind::Authentication))?;
        authorization.set_sensitive(true);
        let mut observed_wait = None;
        // A whole-operation timeout includes chunk reads; no retries or
        // redirects run inside the transport/scheduler lane.
        tokio::time::timeout(Duration::from_secs(20), async {
            let mut response = self
                .client
                .get(url)
                .header(header::AUTHORIZATION, authorization)
                .header(header::ACCEPT, "application/json")
                .header(header::USER_AGENT, "Gitru-Desktop")
                .send()
                .await
                .map_err(|error| {
                    ProviderError::new(if error.is_connect() || error.is_timeout() {
                        ProviderErrorKind::Offline
                    } else {
                        ProviderErrorKind::Unavailable
                    })
                })?;
            let status = response.status();
            let retry = retry_after(response.headers());
            observed_wait = retry;
            if status == StatusCode::TOO_MANY_REQUESTS {
                return Err(rate_limited(retry.unwrap_or(60)));
            }
            let failure = match status {
                StatusCode::UNAUTHORIZED => Some(ProviderErrorKind::Authentication),
                StatusCode::FORBIDDEN => Some(ProviderErrorKind::Permission),
                StatusCode::NOT_FOUND | StatusCode::GONE => Some(ProviderErrorKind::NotFound),
                _ if status.is_server_error() || status == StatusCode::REQUEST_TIMEOUT => {
                    Some(ProviderErrorKind::Unavailable)
                }
                _ if status != StatusCode::OK => Some(ProviderErrorKind::InvalidResponse),
                _ => None,
            };
            if let Some(kind) = failure {
                return Err(ProviderError {
                    kind,
                    retry_after_seconds: retry,
                    account_cooldown_seconds: retry,
                });
            }
            if response
                .content_length()
                .is_some_and(|length| length > MAX_BODY as u64)
            {
                return Err(quota(invalid(), retry));
            }
            let mut body = Vec::new();
            while let Some(chunk) = response
                .chunk()
                .await
                .map_err(|_| quota(ProviderError::new(ProviderErrorKind::Unavailable), retry))?
            {
                if chunk.len() > MAX_BODY.saturating_sub(body.len()) {
                    return Err(quota(invalid(), retry));
                }
                body.extend_from_slice(&chunk);
            }
            // Capacity/near-limit are advisory, not remaining/reset counters.
            Ok(Page {
                body,
                cooldown: retry,
            })
        })
        .await
        .map_err(|_| {
            quota(
                ProviderError::new(ProviderErrorKind::Offline),
                observed_wait,
            )
        })?
    }
}

fn retry_after(headers: &header::HeaderMap) -> Option<u64> {
    let raw = headers.get("retry-after")?.to_str().ok()?;
    if !raw.is_empty() && raw.len() <= 8192 && raw.bytes().all(|b| b.is_ascii_digit()) {
        return Some(raw.parse::<u64>().unwrap_or(u64::MAX).max(1));
    }
    chrono::DateTime::parse_from_rfc2822(raw).ok().map(|date| {
        date.timestamp()
            .saturating_sub(chrono::Utc::now().timestamp())
            .max(1) as u64
    })
}

pub(super) fn rate_limited(wait: u64) -> ProviderError {
    ProviderError {
        kind: ProviderErrorKind::RateLimited,
        retry_after_seconds: Some(wait),
        account_cooldown_seconds: Some(wait),
    }
}

pub(super) fn quota(mut error: ProviderError, wait: Option<u64>) -> ProviderError {
    error.account_cooldown_seconds = error.account_cooldown_seconds.into_iter().chain(wait).max();
    error
}

pub(super) fn invalid() -> ProviderError {
    ProviderError::new(ProviderErrorKind::InvalidResponse)
}

pub(super) struct SelectedDiff {
    pub text: Result<String, crate::pull_files::diff::PullFileTextError>,
    pub cooldown: Option<u64>,
}
impl BitbucketHttp {
    /// A single native-selected path, exact object range, and fixed origin.
    /// No redirects, response links, or renderer URLs participate in routing.
    pub(super) async fn selected_diff(
        &self,
        repository: &str,
        base: &str,
        head: &str,
        path: &str,
        token: &SecretToken,
    ) -> Result<SelectedDiff, ProviderError> {
        use crate::pull_files::diff::{PullFileTextCollector, PullFileTextError};
        if super::canonical_uuid(repository)? != repository
            || !crate::is_canonical_pull_file_oid(base)
            || !crate::is_canonical_pull_file_oid(head)
            || path.is_empty()
            || path.len() > crate::MAX_PULL_FILE_PATH_BYTES
            || path.chars().any(char::is_control)
        {
            return Err(invalid());
        }
        let mut url = self
            .base
            .join(&format!(
                "repositories/%7B%7D/%7B{repository}%7D/diff/{head}..{base}"
            ))
            .map_err(|_| invalid())?;
        url.query_pairs_mut()
            .append_pair("topic", "true")
            .append_pair("renames", "true")
            .append_pair("context", "3")
            .append_pair("binary", "false")
            .append_pair("path", path);
        let mut authorization =
            header::HeaderValue::from_str(&format!("Bearer {}", token.expose()))
                .map_err(|_| ProviderError::new(ProviderErrorKind::Authentication))?;
        authorization.set_sensitive(true);
        let mut cooldown = None;
        tokio::time::timeout(Duration::from_secs(20), async {
            let mut response = self
                .client
                .get(url)
                .header(header::AUTHORIZATION, authorization)
                .header(header::ACCEPT, "text/plain")
                .header(header::USER_AGENT, "Gitru-Desktop")
                .send()
                .await
                .map_err(|error| {
                    ProviderError::new(if error.is_connect() || error.is_timeout() {
                        ProviderErrorKind::Offline
                    } else {
                        ProviderErrorKind::Unavailable
                    })
                })?;
            cooldown = retry_after(response.headers());
            let status = response.status();
            if status == StatusCode::TOO_MANY_REQUESTS {
                return Err(rate_limited(cooldown.unwrap_or(60)));
            }
            let kind = match status {
                StatusCode::OK => None,
                StatusCode::UNAUTHORIZED => Some(ProviderErrorKind::Authentication),
                StatusCode::FORBIDDEN => Some(ProviderErrorKind::Permission),
                StatusCode::NOT_FOUND | StatusCode::GONE => Some(ProviderErrorKind::NotFound),
                _ if status.is_server_error() || status == StatusCode::REQUEST_TIMEOUT => {
                    Some(ProviderErrorKind::Unavailable)
                }
                _ => Some(ProviderErrorKind::InvalidResponse),
            };
            if let Some(kind) = kind {
                return Err(quota(ProviderError::new(kind), cooldown));
            }
            let content_type = response
                .headers()
                .get(header::CONTENT_TYPE)
                .and_then(|value| value.to_str().ok())
                .and_then(|value| value.split(';').next())
                .map(str::trim);
            if !matches!(
                content_type,
                Some("text/plain" | "text/x-diff" | "text/x-patch")
            ) {
                return Err(quota(invalid(), cooldown));
            }
            if response
                .content_length()
                .is_some_and(|length| length > crate::MAX_PULL_FILE_TEXT_BYTES as u64)
            {
                return Ok(SelectedDiff {
                    text: Err(PullFileTextError::ByteLimit),
                    cooldown,
                });
            }
            let mut collector = PullFileTextCollector::new();
            while let Some(chunk) = response
                .chunk()
                .await
                .map_err(|_| quota(ProviderError::new(ProviderErrorKind::Unavailable), cooldown))?
            {
                if let Err(error) = collector.push(&chunk) {
                    // Dropping response aborts remaining transport bytes; never
                    // cache a prefix or continue draining a whole PR response.
                    return Ok(SelectedDiff {
                        text: Err(error),
                        cooldown,
                    });
                }
            }
            Ok(SelectedDiff {
                text: collector.finish(),
                cooldown,
            })
        })
        .await
        .map_err(|_| quota(ProviderError::new(ProviderErrorKind::Offline), cooldown))?
    }
}
