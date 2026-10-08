//! Exact-status, redirect-free read for metadata identity/assignability evidence.
use super::*;
impl GithubHttp {
    pub(crate) async fn get_metadata_point(
        &self,
        url: Url,
        token: &SecretToken,
        no_content: bool,
    ) -> Result<HttpPage, ProviderError> {
        self.check_url(&url)?;
        if url.query().is_some() || url.as_str().len() > 8192 {
            return Err(ProviderError::new(ProviderErrorKind::InvalidResponse));
        }
        let mut auth = header::HeaderValue::from_str(&format!("Bearer {}", token.expose()))
            .map_err(|_| ProviderError::new(ProviderErrorKind::Authentication))?;
        auth.set_sensitive(true);
        let mut observed = None;
        tokio::time::timeout(Duration::from_secs(20), async {
            let mut response = self
                .mutation_client
                .get(url)
                .header(header::AUTHORIZATION, auth)
                .header(header::ACCEPT, "application/vnd.github+json")
                .header(header::USER_AGENT, "Gitru-Desktop")
                .header("X-GitHub-Api-Version", "2026-03-10")
                .send()
                .await
                .map_err(|e| {
                    ProviderError::new(if e.is_connect() || e.is_timeout() {
                        ProviderErrorKind::Offline
                    } else {
                        ProviderErrorKind::Unavailable
                    })
                })?;
            let status = response.status();
            let headers = response.headers();
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            let reset =
                header_number(headers, "x-ratelimit-reset").map(|n| n.saturating_sub(now).max(1));
            let cooldown = (header_number(headers, "x-ratelimit-remaining") == Some(0))
                .then(|| reset.unwrap_or(60))
                .into_iter()
                .chain(header_number(headers, "retry-after").map(|n| n.max(1)))
                .max();
            observed = cooldown;
            let fail = |kind| ProviderError::new(kind).with_cooldown(cooldown);
            if status == StatusCode::UNAUTHORIZED {
                return Err(fail(ProviderErrorKind::Authentication));
            }
            if status == StatusCode::TOO_MANY_REQUESTS
                || status == StatusCode::FORBIDDEN && cooldown.is_some()
            {
                let wait = cooldown.unwrap_or(60);
                return Err(ProviderError {
                    kind: ProviderErrorKind::RateLimited,
                    retry_after_seconds: Some(wait),
                    account_cooldown_seconds: Some(wait),
                });
            }
            if matches!(status, StatusCode::NOT_FOUND | StatusCode::GONE) {
                return Err(fail(ProviderErrorKind::NotFound));
            }
            if status.is_server_error() || status == StatusCode::REQUEST_TIMEOUT {
                return Err(fail(ProviderErrorKind::Unavailable));
            }
            if status.is_redirection()
                || headers.contains_key(header::LOCATION)
                || headers.contains_key(header::LINK)
            {
                return Err(fail(ProviderErrorKind::InvalidResponse));
            }
            let expected = if no_content {
                StatusCode::NO_CONTENT
            } else {
                StatusCode::OK
            };
            if status != expected && status != StatusCode::FORBIDDEN {
                return Err(fail(ProviderErrorKind::InvalidResponse));
            }
            if status == expected && no_content {
                if response.content_length().is_some_and(|n| n > 0) {
                    return Err(fail(ProviderErrorKind::InvalidResponse));
                }
                return Ok(HttpPage {
                    body: vec![],
                    not_modified: false,
                    validators: HttpValidators::default(),
                    next_url: None,
                    poll_interval_seconds: None,
                    cooldown_seconds: cooldown,
                    oauth_scopes: None,
                });
            }
            if response
                .content_length()
                .is_some_and(|n| n > MAX_RESPONSE_BYTES as u64)
            {
                return Err(fail(ProviderErrorKind::InvalidResponse));
            }
            let mut body = vec![];
            while let Some(chunk) = response
                .chunk()
                .await
                .map_err(|_| fail(ProviderErrorKind::Unavailable))?
            {
                if chunk.len() > MAX_RESPONSE_BYTES.saturating_sub(body.len()) {
                    return Err(fail(ProviderErrorKind::InvalidResponse));
                }
                body.extend_from_slice(&chunk);
            }
            if status == StatusCode::FORBIDDEN {
                let limited = serde_json::from_slice::<serde_json::Value>(&body)
                    .ok()
                    .and_then(|v| {
                        v.get("message")
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_ascii_lowercase().contains("rate limit"))
                    })
                    .unwrap_or(false);
                return Err(if limited {
                    ProviderError {
                        kind: ProviderErrorKind::RateLimited,
                        retry_after_seconds: Some(60),
                        account_cooldown_seconds: Some(60),
                    }
                } else {
                    fail(ProviderErrorKind::Permission)
                });
            }
            Ok(HttpPage {
                body,
                not_modified: false,
                validators: HttpValidators::default(),
                next_url: None,
                poll_interval_seconds: None,
                cooldown_seconds: cooldown,
                oauth_scopes: None,
            })
        })
        .await
        .map_err(|_| ProviderError::new(ProviderErrorKind::Offline).with_cooldown(observed))?
    }
}
