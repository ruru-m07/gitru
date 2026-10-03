use crate::credentials::SecretToken;
use reqwest::{Client, StatusCode, Url, header};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const MAX_RESPONSE_BYTES: usize = 4 * 1024 * 1024;
const MAX_REDIRECTS: usize = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderErrorKind {
    Authentication,
    Permission,
    NotFound,
    RateLimited,
    Offline,
    Unavailable,
    InvalidResponse,
    Unsupported,
}

/// Provider bodies, secrets and potentially sensitive URLs never enter errors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderError {
    pub kind: ProviderErrorKind,
    pub retry_after_seconds: Option<u64>,
}

impl ProviderError {
    pub fn new(kind: ProviderErrorKind) -> Self {
        Self {
            kind,
            retry_after_seconds: None,
        }
    }
}

impl std::fmt::Display for ProviderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self.kind {
            ProviderErrorKind::Authentication => "The provider credential is no longer valid",
            ProviderErrorKind::Permission => "The provider denied access to this resource",
            ProviderErrorKind::NotFound => "The provider resource is unavailable",
            ProviderErrorKind::RateLimited => "Provider requests are temporarily rate limited",
            ProviderErrorKind::Offline => "Unable to connect to the provider",
            ProviderErrorKind::Unavailable => "The provider is temporarily unavailable",
            ProviderErrorKind::InvalidResponse => {
                "The provider returned an invalid or oversized response"
            }
            ProviderErrorKind::Unsupported => {
                "The provider does not support this operation with this credential"
            }
        })
    }
}

impl std::error::Error for ProviderError {}

#[derive(Debug, Clone, Default)]
pub(crate) struct HttpValidators {
    pub etag: Option<String>,
    pub last_modified: Option<String>,
}

#[derive(Debug)]
pub(crate) struct HttpPage {
    pub body: Vec<u8>,
    pub not_modified: bool,
    pub validators: HttpValidators,
    pub next_url: Option<String>,
    pub poll_interval_seconds: Option<u64>,
    pub cooldown_seconds: Option<u64>,
    pub oauth_scopes: Option<String>,
}

/// Pooled provider transport. Only the explicitly configured API origin can
/// receive credentials, including after redirects and on pagination requests.
pub(crate) struct GithubHttp {
    client: Client,
    base_url: Url,
}

impl GithubHttp {
    pub fn new() -> Result<Self, ProviderError> {
        Self::for_base(Url::parse("https://api.github.com/").expect("constant GitHub URL"))
    }

    fn for_base(base_url: Url) -> Result<Self, ProviderError> {
        let client = Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .connect_timeout(Duration::from_secs(5))
            .timeout(Duration::from_secs(20))
            .pool_idle_timeout(Duration::from_secs(60))
            .https_only(base_url.scheme() == "https")
            .build()
            .map_err(|_| ProviderError::new(ProviderErrorKind::Unavailable))?;
        Ok(Self { client, base_url })
    }

    #[cfg(test)]
    pub(crate) fn for_test_base(base_url: Url) -> Result<Self, ProviderError> {
        Self::for_base(base_url)
    }

    pub fn endpoint(&self, path: &str) -> Result<Url, ProviderError> {
        let url = self
            .base_url
            .join(path)
            .map_err(|_| ProviderError::new(ProviderErrorKind::InvalidResponse))?;
        self.check_url(&url)?;
        Ok(url)
    }

    pub fn check_page_url(&self, next: &str, expected_path: &str) -> Result<Url, ProviderError> {
        self.check_page_url_with_paths(next, &[expected_path.to_string()])
    }

    pub fn check_page_url_with_paths(
        &self,
        next: &str,
        allowed_paths: &[String],
    ) -> Result<Url, ProviderError> {
        let url =
            Url::parse(next).map_err(|_| ProviderError::new(ProviderErrorKind::InvalidResponse))?;
        self.check_url(&url)?;
        if !allowed_paths.iter().any(|path| path == url.path()) {
            return Err(ProviderError::new(ProviderErrorKind::InvalidResponse));
        }
        Ok(url)
    }

    fn check_url(&self, url: &Url) -> Result<(), ProviderError> {
        if url.origin() != self.base_url.origin()
            || !url.username().is_empty()
            || url.password().is_some()
            || url.fragment().is_some()
        {
            return Err(ProviderError::new(ProviderErrorKind::InvalidResponse));
        }
        Ok(())
    }

    pub async fn get(
        &self,
        url: Url,
        token: &SecretToken,
        validators: &HttpValidators,
    ) -> Result<HttpPage, ProviderError> {
        let allowed_paths = vec![url.path().to_string()];
        self.get_with_paths(url, token, validators, &allowed_paths)
            .await
    }

    pub async fn get_with_paths(
        &self,
        mut url: Url,
        token: &SecretToken,
        validators: &HttpValidators,
        allowed_paths: &[String],
    ) -> Result<HttpPage, ProviderError> {
        for redirect in 0..=MAX_REDIRECTS {
            self.check_page_url_with_paths(url.as_str(), allowed_paths)?;
            let mut authorization =
                header::HeaderValue::from_str(&format!("Bearer {}", token.expose()))
                    .map_err(|_| ProviderError::new(ProviderErrorKind::Authentication))?;
            authorization.set_sensitive(true);
            let mut request = self
                .client
                .get(url.clone())
                .header(header::AUTHORIZATION, authorization)
                .header(header::ACCEPT, "application/vnd.github+json")
                .header(header::USER_AGENT, "Gitru-Desktop")
                .header("X-GitHub-Api-Version", "2026-03-10");
            if let Some(etag) = &validators.etag {
                request = request.header(header::IF_NONE_MATCH, etag);
            } else if let Some(last_modified) = &validators.last_modified {
                request = request.header(header::IF_MODIFIED_SINCE, last_modified);
            }
            let mut response = request.send().await.map_err(|error| {
                ProviderError::new(if error.is_connect() || error.is_timeout() {
                    ProviderErrorKind::Offline
                } else {
                    ProviderErrorKind::Unavailable
                })
            })?;
            let status = response.status();
            if status.is_redirection() && status != StatusCode::NOT_MODIFIED {
                if redirect == MAX_REDIRECTS {
                    return Err(ProviderError::new(ProviderErrorKind::InvalidResponse));
                }
                let location = header_text(response.headers(), header::LOCATION.as_str())
                    .ok_or_else(|| ProviderError::new(ProviderErrorKind::InvalidResponse))?;
                url = url
                    .join(&location)
                    .map_err(|_| ProviderError::new(ProviderErrorKind::InvalidResponse))?;
                self.check_page_url_with_paths(url.as_str(), allowed_paths)?;
                continue;
            }
            let headers = response.headers();
            let retry_after = header_number(headers, "retry-after");
            let remaining = header_number(headers, "x-ratelimit-remaining");
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            let reset_wait = header_number(headers, "x-ratelimit-reset")
                .map(|reset| reset.saturating_sub(now).max(1));
            let cooldown = if remaining == Some(0) {
                Some(reset_wait.unwrap_or(60))
            } else {
                None
            };
            let response_validators = HttpValidators {
                etag: header_text(headers, "etag"),
                last_modified: header_text(headers, "last-modified"),
            };
            let poll_interval_seconds = header_number(headers, "x-poll-interval").map(|n| n.max(1));
            let oauth_scopes = header_text(headers, "x-oauth-scopes");
            let links = header_text(headers, "link");
            let next_url = links.as_deref().and_then(next_link);
            if let Some(next) = &next_url {
                self.check_page_url_with_paths(next, allowed_paths)?;
            }
            if status == StatusCode::UNAUTHORIZED {
                return Err(ProviderError::new(ProviderErrorKind::Authentication));
            }
            if status == StatusCode::TOO_MANY_REQUESTS
                || (status == StatusCode::FORBIDDEN
                    && (retry_after.is_some() || remaining == Some(0)))
            {
                return Err(ProviderError {
                    kind: ProviderErrorKind::RateLimited,
                    retry_after_seconds: Some(match (retry_after, cooldown) {
                        (Some(retry), Some(reset)) => retry.max(reset).max(1),
                        (Some(wait), None) | (None, Some(wait)) => wait.max(1),
                        (None, None) => 60,
                    }),
                });
            }
            if status == StatusCode::FORBIDDEN {
                let mut body = Vec::new();
                while let Some(chunk) = response
                    .chunk()
                    .await
                    .map_err(|_| ProviderError::new(ProviderErrorKind::Unavailable))?
                {
                    if chunk.len() > MAX_RESPONSE_BYTES.saturating_sub(body.len()) {
                        return Err(ProviderError::new(ProviderErrorKind::InvalidResponse));
                    }
                    body.extend_from_slice(&chunk);
                }
                let rate_limited = serde_json::from_slice::<serde_json::Value>(&body)
                    .ok()
                    .and_then(|value| {
                        value
                            .get("message")
                            .and_then(|value| value.as_str())
                            .map(|message| message.to_ascii_lowercase().contains("rate limit"))
                    })
                    .unwrap_or(false);
                if rate_limited {
                    return Err(ProviderError {
                        kind: ProviderErrorKind::RateLimited,
                        retry_after_seconds: Some(60),
                    });
                }
                return Err(ProviderError::new(ProviderErrorKind::Permission));
            }
            if status == StatusCode::NOT_FOUND || status == StatusCode::GONE {
                return Err(ProviderError::new(ProviderErrorKind::NotFound));
            }
            if status.is_server_error() {
                return Err(ProviderError::new(ProviderErrorKind::Unavailable));
            }
            let not_modified = status == StatusCode::NOT_MODIFIED;
            if !status.is_success() && !not_modified {
                return Err(ProviderError::new(ProviderErrorKind::InvalidResponse));
            }
            let mut body = Vec::new();
            if !not_modified {
                if response
                    .content_length()
                    .is_some_and(|length| length > MAX_RESPONSE_BYTES as u64)
                {
                    return Err(ProviderError::new(ProviderErrorKind::InvalidResponse));
                }
                while let Some(chunk) = response
                    .chunk()
                    .await
                    .map_err(|_| ProviderError::new(ProviderErrorKind::Unavailable))?
                {
                    if chunk.len() > MAX_RESPONSE_BYTES.saturating_sub(body.len()) {
                        return Err(ProviderError::new(ProviderErrorKind::InvalidResponse));
                    }
                    body.extend_from_slice(&chunk);
                }
            }
            return Ok(HttpPage {
                body,
                not_modified,
                validators: response_validators,
                next_url,
                poll_interval_seconds,
                cooldown_seconds: cooldown,
                oauth_scopes,
            });
        }
        unreachable!("bounded redirect loop returns")
    }
}

fn header_text(headers: &header::HeaderMap, name: &str) -> Option<String> {
    headers
        .get(name)
        .and_then(|v| v.to_str().ok())
        .filter(|v| v.len() <= 8192)
        .map(str::to_string)
}

fn header_number(headers: &header::HeaderMap, name: &str) -> Option<u64> {
    headers.get(name)?.to_str().ok()?.parse().ok()
}

fn next_link(value: &str) -> Option<String> {
    value.split(',').find_map(|link| {
        let (url, parameters) = link.trim().split_once('>')?;
        if !parameters
            .split(';')
            .any(|parameter| parameter.trim() == "rel=\"next\"")
        {
            return None;
        }
        Some(url.strip_prefix('<')?.to_string())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};

    fn server(response: String) -> (GithubHttp, std::thread::JoinHandle<String>) {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}/", listener.local_addr().unwrap());
        let handle = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            let mut bytes = Vec::new();
            loop {
                let mut buffer = [0u8; 1024];
                let length = stream.read(&mut buffer).unwrap();
                bytes.extend_from_slice(&buffer[..length]);
                if length == 0 || bytes.windows(4).any(|bytes| bytes == b"\r\n\r\n") {
                    break;
                }
            }
            stream.write_all(response.as_bytes()).unwrap();
            String::from_utf8(bytes).unwrap()
        });
        (
            GithubHttp::for_base(Url::parse(&base).unwrap()).unwrap(),
            handle,
        )
    }

    #[test]
    fn pagination_and_redirects_cannot_cross_credential_origin() {
        let http = GithubHttp::new().unwrap();
        for url in [
            "https://evil.example/repos/a/b/pulls",
            "http://api.github.com/repos/a/b/pulls",
            "https://secret@api.github.com/repos/a/b/pulls",
            "https://api.github.com/repos/a/b/pulls#fragment",
            "https://api.github.com/user",
        ] {
            assert!(http.check_page_url(url, "/repos/a/b/pulls").is_err());
        }
        assert!(
            http.check_page_url(
                "https://api.github.com/repos/a/b/pulls?page=2",
                "/repos/a/b/pulls"
            )
            .is_ok()
        );
    }

    #[test]
    fn reads_only_the_declared_next_link() {
        assert_eq!(
            next_link(
                "<https://api.github.com/user/repos?page=2>; rel=\"next\", <https://api.github.com/user/repos?page=9>; rel=\"last\""
            ),
            Some("https://api.github.com/user/repos?page=2".to_string())
        );
        assert_eq!(
            next_link("<https://api.github.com/user/repos?page=1>; rel=\"prev\""),
            None
        );
    }

    #[tokio::test]
    async fn conditional_requests_send_the_exact_validator_and_read_304_metadata() {
        let (http, request) = server("HTTP/1.1 304 Not Modified\r\nETag: \"same\"\r\nX-Poll-Interval: 90\r\nConnection: close\r\n\r\n".into());
        let token = SecretToken::new("secret_token".into()).unwrap();
        let page = http
            .get(
                http.endpoint("notifications").unwrap(),
                &token,
                &HttpValidators {
                    etag: Some("\"same\"".into()),
                    last_modified: None,
                },
            )
            .await
            .unwrap();
        assert!(page.not_modified);
        assert!(page.body.is_empty());
        assert_eq!(page.poll_interval_seconds, Some(90));
        let request = request.join().unwrap().to_ascii_lowercase();
        assert!(request.contains("if-none-match: \"same\""));
        assert!(request.contains("authorization: bearer secret_token"));
        assert!(request.contains("x-github-api-version: 2026-03-10"));
    }

    #[tokio::test]
    async fn does_not_follow_redirects_that_would_send_credentials_to_another_host() {
        let (http, request) = server("HTTP/1.1 302 Found\r\nLocation: https://evil.example/steal\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".into());
        let error = http
            .get(
                http.endpoint("user").unwrap(),
                &SecretToken::new("secret_token".into()).unwrap(),
                &HttpValidators::default(),
            )
            .await
            .unwrap_err();
        assert_eq!(error.kind, ProviderErrorKind::InvalidResponse);
        assert!(!error.to_string().contains("secret_token"));
        request.join().unwrap();
    }

    #[tokio::test]
    async fn rejects_oversized_bodies_before_allocating_them() {
        let (http, request) = server(format!(
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            MAX_RESPONSE_BYTES + 1
        ));
        let error = http
            .get(
                http.endpoint("user").unwrap(),
                &SecretToken::new("secret_token".into()).unwrap(),
                &HttpValidators::default(),
            )
            .await
            .unwrap_err();
        assert_eq!(error.kind, ProviderErrorKind::InvalidResponse);
        request.join().unwrap();
    }

    #[tokio::test]
    async fn secondary_rate_limits_without_retry_header_pause_requests() {
        let body = r#"{"message":"You have exceeded a secondary rate limit. Please wait a few minutes before you try again."}"#;
        let (http, request) = server(format!(
            "HTTP/1.1 403 Forbidden\r\nContent-Length: {}\r\nX-RateLimit-Remaining: 4000\r\nConnection: close\r\n\r\n{body}",
            body.len()
        ));
        let error = http
            .get(
                http.endpoint("user").unwrap(),
                &SecretToken::new("secret_token".into()).unwrap(),
                &HttpValidators::default(),
            )
            .await
            .unwrap_err();
        assert_eq!(error.kind, ProviderErrorKind::RateLimited);
        assert_eq!(error.retry_after_seconds, Some(60));
        assert!(!error.to_string().contains("exceeded"));
        request.join().unwrap();
    }

    #[tokio::test]
    async fn long_provider_cooldowns_are_preserved() {
        let (http, request) = server("HTTP/1.1 429 Too Many Requests\r\nRetry-After: 172800\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".into());
        let error = http
            .get(
                http.endpoint("user").unwrap(),
                &SecretToken::new("secret_token".into()).unwrap(),
                &HttpValidators::default(),
            )
            .await
            .unwrap_err();
        assert_eq!(error.retry_after_seconds, Some(172800));
        request.join().unwrap();
    }
}
