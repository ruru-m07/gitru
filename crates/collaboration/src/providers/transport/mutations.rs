//! Native-only mutation transport. It never interprets success as operation proof.
#![allow(
    dead_code,
    reason = "reviewed operation codecs consume this shared seam"
)]
use super::*;
use reqwest::Method;

pub(crate) struct MutationHttpResponse {
    pub status: StatusCode,
    pub body: Vec<u8>,
    pub cooldown_seconds: Option<u64>,
    pub provider_error: Option<ProviderError>,
}

pub(crate) fn mutation_client(base: &Url) -> Result<Client, ProviderError> {
    Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .retry(reqwest::retry::never())
        .connect_timeout(Duration::from_secs(5))
        .timeout(Duration::from_secs(20))
        .pool_idle_timeout(Duration::from_secs(60))
        .https_only(base.scheme() == "https")
        .build()
        .map_err(|_| ProviderError::new(ProviderErrorKind::Unavailable))
}

impl GithubHttp {
    pub(crate) async fn mutate_native(
        &self,
        token: &SecretToken,
        method: Method,
        url: Url,
        body: Vec<u8>,
    ) -> Result<MutationHttpResponse, ProviderError> {
        self.check_url(&url)?;
        mutate(
            &self.mutation_client,
            &self.base_url,
            token,
            method,
            url,
            body,
            true,
        )
        .await
    }
}

/// Caller constructs fixed native route; body is a reviewed operation codec.
/// There is no generic command or renderer-facing transport API.
pub(crate) async fn mutate(
    client: &Client,
    base: &Url,
    token: &SecretToken,
    method: Method,
    url: Url,
    body: Vec<u8>,
    github: bool,
) -> Result<MutationHttpResponse, ProviderError> {
    let invalid = || ProviderError::new(ProviderErrorKind::InvalidResponse);
    if url.origin() != base.origin()
        || !url.path().starts_with(base.path())
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || url.as_str().len() > 2048
        || !matches!(method, Method::PATCH | Method::POST | Method::DELETE)
        || body.len() > 65_536
    {
        return Err(invalid());
    }
    let (name, value) = if github {
        (header::AUTHORIZATION, format!("Bearer {}", token.expose()))
    } else {
        (
            header::HeaderName::from_static("private-token"),
            token.expose().to_string(),
        )
    };
    let mut auth = header::HeaderValue::from_str(&value)
        .map_err(|_| ProviderError::new(ProviderErrorKind::Authentication))?;
    auth.set_sensitive(true);
    let mut observed = None;
    let mut observed_error = None;
    tokio::time::timeout(Duration::from_secs(20), async {
        let mut request = client
            .request(method, url)
            .header(name, auth)
            .header(header::USER_AGENT, "Gitru-Desktop")
            .header(
                header::ACCEPT,
                if github {
                    "application/vnd.github+json"
                } else {
                    "application/json"
                },
            )
            .header(header::CONTENT_TYPE, "application/json")
            .body(body);
        if github {
            request = request.header("X-GitHub-Api-Version", "2026-03-10");
        }
        let mut response = request.send().await.map_err(|error| {
            ProviderError::new(if error.is_connect() || error.is_timeout() {
                ProviderErrorKind::Offline
            } else {
                ProviderErrorKind::Unavailable
            })
        })?;
        let status = response.status();
        let remaining = header_number(
            response.headers(),
            if github {
                "x-ratelimit-remaining"
            } else {
                "ratelimit-remaining"
            },
        );
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let reset = header_number(
            response.headers(),
            if github {
                "x-ratelimit-reset"
            } else {
                "ratelimit-reset"
            },
        )
        .map(|v| v.saturating_sub(now).max(1));
        let retry = header_number(response.headers(), "retry-after").map(|v| v.max(1));
        let cooldown = (remaining == Some(0))
            .then(|| reset.unwrap_or(60))
            .into_iter()
            .chain(retry)
            .max();
        observed = cooldown;
        let mut provider_error = if status == StatusCode::UNAUTHORIZED {
            Some(ProviderError::new(ProviderErrorKind::Authentication))
        } else if status == StatusCode::TOO_MANY_REQUESTS
            || (status == StatusCode::FORBIDDEN && cooldown.is_some())
        {
            let wait = cooldown.unwrap_or(60);
            Some(ProviderError {
                kind: ProviderErrorKind::RateLimited,
                retry_after_seconds: Some(wait),
                account_cooldown_seconds: Some(wait),
            })
        } else if status == StatusCode::FORBIDDEN {
            Some(ProviderError::new(ProviderErrorKind::Permission))
        } else if matches!(status, StatusCode::NOT_FOUND | StatusCode::GONE) {
            Some(ProviderError::new(ProviderErrorKind::NotFound))
        } else if status.is_server_error() || status == StatusCode::REQUEST_TIMEOUT {
            Some(ProviderError::new(ProviderErrorKind::Unavailable))
        } else if !status.is_success() {
            Some(invalid())
        } else {
            None
        };
        if let Some(error) = provider_error.as_mut() {
            error.account_cooldown_seconds = error
                .account_cooldown_seconds
                .into_iter()
                .chain(cooldown)
                .max();
        }
        observed_error = provider_error.clone();
        let fail = || {
            let mut error = provider_error.clone().unwrap_or_else(invalid);
            error.account_cooldown_seconds = error
                .account_cooldown_seconds
                .into_iter()
                .chain(cooldown)
                .max();
            error
        };
        // Never follow redirects. Even same-origin redirects lack operation proof.
        if status.is_redirection() && status != StatusCode::NOT_MODIFIED {
            return Err(fail());
        }
        if response
            .content_length()
            .is_some_and(|n| n > MAX_RESPONSE_BYTES as u64)
        {
            return Err(fail());
        }
        let json = response
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|h| h.to_str().ok())
            .is_some_and(|s| {
                s.split(';').next().is_some_and(|s| {
                    s.trim() == "application/json" || s.trim() == "application/vnd.github+json"
                })
            });
        let mut body = Vec::new();
        while let Some(chunk) = response.chunk().await.map_err(|_| fail())? {
            if chunk.len() > MAX_RESPONSE_BYTES.saturating_sub(body.len()) {
                return Err(fail());
            }
            body.extend_from_slice(&chunk);
        }
        if !body.is_empty() && !json {
            return Err(fail());
        }
        if status == StatusCode::FORBIDDEN
            && provider_error
                .as_ref()
                .is_some_and(|e| e.kind == ProviderErrorKind::Permission)
        {
            let limited = serde_json::from_slice::<serde_json::Value>(&body)
                .ok()
                .and_then(|v| {
                    v.get("message")
                        .and_then(|m| m.as_str())
                        .map(|m| m.to_ascii_lowercase().contains("rate limit"))
                })
                .unwrap_or(false);
            if limited {
                provider_error = Some(ProviderError {
                    kind: ProviderErrorKind::RateLimited,
                    retry_after_seconds: Some(60),
                    account_cooldown_seconds: Some(60),
                });
            }
        }
        Ok(MutationHttpResponse {
            status,
            body,
            cooldown_seconds: cooldown
                .into_iter()
                .chain(
                    provider_error
                        .as_ref()
                        .and_then(|e| e.account_cooldown_seconds),
                )
                .max(),
            provider_error,
        })
    })
    .await
    .map_err(|_| {
        let mut error =
            observed_error.unwrap_or_else(|| ProviderError::new(ProviderErrorKind::Offline));
        error.account_cooldown_seconds = error
            .account_cooldown_seconds
            .into_iter()
            .chain(observed)
            .max();
        error
    })?
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    fn server(
        status: &str,
        headers: &str,
        body: &str,
    ) -> (GithubHttp, std::thread::JoinHandle<String>) {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let base = Url::parse(&format!("http://{}/", listener.local_addr().unwrap())).unwrap();
        let response = format!(
            "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n{headers}\r\n{body}",
            body.len()
        );
        let task = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            let mut bytes = [0u8; 8192];
            let n = stream.read(&mut bytes).unwrap();
            stream
                .set_write_timeout(Some(Duration::from_secs(1)))
                .unwrap();
            let _ = stream.write_all(response.as_bytes());
            String::from_utf8(bytes[..n].to_vec()).unwrap()
        });
        (GithubHttp::for_test_base(base).unwrap(), task)
    }
    fn token() -> SecretToken {
        SecretToken::new("synthetic-only".into()).unwrap()
    }
    #[tokio::test]
    async fn exact_patch_exposes_status_without_claiming_confirmation() {
        let (http, task) = server("205 Reset Content", "X-RateLimit-Remaining: 0\r\n", "");
        let response = http
            .mutate_native(
                &token(),
                Method::PATCH,
                http.endpoint("notifications/threads/123").unwrap(),
                vec![],
            )
            .await
            .unwrap();
        assert_eq!(response.status, StatusCode::RESET_CONTENT);
        assert!(response.body.is_empty());
        assert_eq!(response.cooldown_seconds, Some(60));
        assert!(response.provider_error.is_none());
        let request = task.join().unwrap();
        assert!(request.starts_with("PATCH /notifications/threads/123 HTTP/1.1"));
        assert!(request.contains("x-github-api-version: 2026-03-10"));
    }
    #[tokio::test]
    async fn denial_keeps_separate_authentication_and_quota() {
        let (http, task) = server(
            "401 Unauthorized",
            "Content-Type: application/json\r\nRetry-After: 72\r\n",
            "{\"message\":\"denied\"}",
        );
        let response = http
            .mutate_native(
                &token(),
                Method::PATCH,
                http.endpoint("repos/a/b/issues/1").unwrap(),
                b"{}".to_vec(),
            )
            .await
            .unwrap();
        assert_eq!(response.status, StatusCode::UNAUTHORIZED);
        assert_eq!(response.cooldown_seconds, Some(72));
        assert_eq!(
            response.provider_error.unwrap().kind,
            ProviderErrorKind::Authentication
        );
        task.join().unwrap();
    }
    #[tokio::test]
    async fn redirect_is_not_followed_and_malformed_success_has_no_receipt() {
        for (status, headers, body) in [
            ("302 Found", "Location: /elsewhere\r\n", ""),
            (
                "200 OK",
                "Content-Type: text/html\r\n",
                "<html>login</html>",
            ),
        ] {
            let (http, task) = server(status, headers, body);
            let result = http
                .mutate_native(
                    &token(),
                    Method::PATCH,
                    http.endpoint("notifications/threads/1").unwrap(),
                    vec![],
                )
                .await;
            assert!(matches!(
                result,
                Err(ProviderError {
                    kind: ProviderErrorKind::InvalidResponse,
                    ..
                })
            ));
            task.join().unwrap();
        }
    }
    #[tokio::test]
    async fn routes_and_request_bounds_fail_before_network() {
        let http = GithubHttp::for_test_base(Url::parse("http://127.0.0.1:9/").unwrap()).unwrap();
        for (method, url, body) in [
            (Method::GET, "http://127.0.0.1:9/x", vec![]),
            (Method::PATCH, "http://127.0.0.1:10/x", vec![]),
            (Method::POST, "http://127.0.0.1:9/x?q=1", vec![]),
            (Method::PATCH, "http://127.0.0.1:9/x", vec![0; 65_537]),
        ] {
            assert!(matches!(
                http.mutate_native(&token(), method, Url::parse(url).unwrap(), body)
                    .await,
                Err(ProviderError {
                    kind: ProviderErrorKind::InvalidResponse,
                    ..
                })
            ));
        }
    }
    #[tokio::test]
    async fn unbounded_body_is_rejected_with_observed_quota() {
        let (http, task) = server(
            "200 OK",
            "Content-Type: application/json\r\nRetry-After: 42\r\n",
            &"x".repeat(MAX_RESPONSE_BYTES + 1),
        );
        let error = http
            .mutate_native(
                &token(),
                Method::PATCH,
                http.endpoint("notifications/threads/1").unwrap(),
                vec![],
            )
            .await
            .err()
            .unwrap();
        assert_eq!(error.account_cooldown_seconds, Some(42));
        let _ = task.join();
    }
    #[tokio::test]
    async fn malformed_denials_keep_auth_and_default_rate_observations() {
        for (status, kind, quota) in [
            ("401 Unauthorized", ProviderErrorKind::Authentication, None),
            (
                "429 Too Many Requests",
                ProviderErrorKind::RateLimited,
                Some(60),
            ),
        ] {
            let (http, task) = server(status, "Content-Type: text/html\r\n", "provider denial");
            let error = http
                .mutate_native(
                    &token(),
                    Method::PATCH,
                    http.endpoint("notifications/threads/1").unwrap(),
                    vec![],
                )
                .await
                .err()
                .unwrap();
            assert_eq!(error.kind, kind);
            assert_eq!(error.account_cooldown_seconds, quota);
            task.join().unwrap();
        }
    }
}
