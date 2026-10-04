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
        };
        self.base.join(&relative).map_err(|_| invalid())
    }

    pub(super) fn continuation(&self, raw: &str, route: &Route) -> Result<Url, ProviderError> {
        let url = self.validate(raw, route)?;
        if matches!(route, Route::User)
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
        let same_path = url.path() == expected.path()
            || match route {
                Route::Repositories(workspace) => {
                    url.path() == format!("{}repositories/{{{workspace}}}", self.base.path())
                        || url.path()
                            == format!("{}repositories/%7b{workspace}%7d", self.base.path())
                }
                _ => false,
            };
        if url.origin() != self.base.origin()
            || !url.username().is_empty()
            || url.password().is_some()
            || url.fragment().is_some()
            || !same_path
        {
            return Err(invalid());
        }
        let mut pairs = BTreeMap::new();
        for (key, value) in url.query_pairs() {
            if key.len() > 32
                || value.is_empty()
                || value.len() > 256
                || value.chars().any(char::is_control)
                || pairs.insert(key.to_string(), value.to_string()).is_some()
            {
                return Err(invalid());
            }
        }
        for (key, value) in expected.query_pairs() {
            if pairs.remove(key.as_ref()).as_deref() != Some(value.as_ref()) {
                return Err(invalid());
            }
        }
        if pairs
            .keys()
            .any(|key| !matches!(key.as_str(), "page" | "cursor" | "after" | "before"))
            || (matches!(route, Route::User) && !pairs.is_empty())
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
        let pairs: BTreeMap<_, _> = url.query_pairs().collect();
        let encoded =
            serde_json::to_vec(&(self.endpoint(route)?.path(), pairs)).map_err(|_| invalid())?;
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
