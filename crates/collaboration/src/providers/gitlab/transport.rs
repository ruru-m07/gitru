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

pub(super) struct GitlabHttp {
    client: Client,
    base: Url,
}

pub(super) struct Page {
    pub body: Vec<u8>,
    pub next: Option<String>,
    pub cooldown: Option<u64>,
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
        let url = self.check_raw(raw)?;
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
        let projects = original.path() == self.projects().path();
        if projects {
            project_after(&original)?;
        } else if original != self.user() {
            return Err(invalid());
        }
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
                if status.is_server_error() {
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
                    if !projects {
                        return Err(with_quota(invalid(), observed_cooldown));
                    }
                    self.continuation(next)
                        .map_err(|e| with_quota(e, observed_cooldown))?;
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

pub(super) fn positive_id(raw: &str) -> Option<u64> {
    raw.parse::<u64>()
        .ok()
        .filter(|id| *id > 0 && id.to_string() == raw)
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
