//! Parsing occurs before URL normalization can discard unsafe path spellings.
use crate::models::remotes::*;

pub fn sanitize_remote_url(raw: &str) -> SafeRemoteUrl {
    let unsupported = |redacted| SafeRemoteUrl {
        ordinal: 0,
        sanitized_url: None,
        endpoint: None,
        redacted,
    };
    if raw.is_empty()
        || raw.len() > 4096
        || raw.trim() != raw
        || raw.chars().any(char::is_control)
        || raw.contains('\\')
    {
        return unsupported(true);
    }
    let parsed = if raw.starts_with("https://") || raw.starts_with("ssh://") {
        // Inspect the unnormalized path, so /a/../b cannot become a valid /b.
        let authority_end = raw.find("://").map(|i| i + 3).unwrap_or(0);
        let rest = &raw[authority_end..];
        let path = rest
            .find(['/', '?', '#'])
            .filter(|i| rest.as_bytes()[*i] == b'/')
            .map(|i| &rest[i..])
            .unwrap_or("");
        let path = path.split(['?', '#']).next().unwrap_or("");
        if !valid_path(path.strip_prefix('/').unwrap_or(path)) {
            return unsupported(true);
        }
        let Ok(url) = url::Url::parse(raw) else {
            return unsupported(true);
        };
        let Some(host) = url.host_str() else {
            return unsupported(true);
        };
        let transport = if url.scheme() == "https" {
            RemoteTransport::Https
        } else {
            RemoteTransport::Ssh
        };
        let port = url
            .port()
            .unwrap_or(if transport == RemoteTransport::Https {
                443
            } else {
                22
            });
        if port == 0 {
            return unsupported(true);
        }
        let redacted = !url.username().is_empty()
            || url.password().is_some()
            || url.query().is_some()
            || url.fragment().is_some();
        (
            transport,
            host.to_owned(),
            port,
            path.strip_prefix('/').unwrap_or(path).to_owned(),
            redacted,
        )
    } else {
        // SCP is recognized only before any slash, as Git itself specifies.
        if raw.contains("::") {
            return unsupported(true);
        }
        let Some((authority, path)) = raw.split_once(':') else {
            return unsupported(false);
        };
        if authority.contains('/')
            || authority.is_empty()
            || !valid_path(path.trim_start_matches('/'))
            || path.starts_with('/')
        {
            return unsupported(true);
        }
        let (host, redacted) = match authority.rsplit_once('@') {
            Some((_, host)) => (host, true),
            None => (authority, false),
        };
        if host.contains([':', '%', '?', '#']) || host.is_empty() {
            return unsupported(true);
        }
        let Ok(url) = url::Url::parse(&format!("ssh://{host}/")) else {
            return unsupported(true);
        };
        let Some(host) = url.host_str() else {
            return unsupported(true);
        };
        (
            RemoteTransport::Scp,
            host.to_owned(),
            22,
            path.to_owned(),
            redacted,
        )
    };
    let (transport, host, port, path, redacted) = parsed;
    let scheme = if transport == RemoteTransport::Https {
        "https"
    } else {
        "ssh"
    };
    let default = if transport == RemoteTransport::Https {
        443
    } else {
        22
    };
    let port_text = if port == default {
        String::new()
    } else {
        format!(":{port}")
    };
    SafeRemoteUrl {
        ordinal: 0,
        sanitized_url: Some(format!("{scheme}://{host}{port_text}/{path}")),
        endpoint: Some(RemoteEndpoint {
            transport,
            host,
            port,
            path,
        }),
        redacted,
    }
}

fn valid_path(path: &str) -> bool {
    !path.is_empty()
        && path.len() <= 2048
        && !path.contains(['%', '?', '#', '\\'])
        && !path.chars().any(char::is_control)
        && path
            .split('/')
            .all(|part| !part.is_empty() && part != "." && part != ".." && !part.starts_with('~'))
}

pub(crate) fn config_names(bytes: &[u8]) -> Result<Vec<String>, RemoteObservationError> {
    use RemoteObservationError::{InvalidConfiguration, LimitExceeded};
    if !bytes.is_empty() && !bytes.ends_with(&[0]) {
        return Err(InvalidConfiguration);
    }
    let mut names = std::collections::BTreeSet::new();
    for record in bytes.split(|b| *b == 0).filter(|r| !r.is_empty()) {
        let record = std::str::from_utf8(record).map_err(|_| InvalidConfiguration)?;
        let (key, value) = record.split_once('\n').unwrap_or((record, ""));
        let relevant = key.starts_with("remote.") || key.starts_with("url.");
        if relevant && (key.chars().any(char::is_control) || value.chars().any(char::is_control)) {
            return Err(InvalidConfiguration);
        }
        if let Some(rest) = key.strip_prefix("remote.") {
            let Some((name, _)) = rest.rsplit_once('.') else {
                return Err(InvalidConfiguration);
            };
            if name.is_empty() || name.len() > 255 || name.starts_with('-') {
                return Err(InvalidConfiguration);
            }
            names.insert(name.to_owned());
        }
    }
    if names.len() > 128 {
        return Err(LimitExceeded);
    }
    Ok(names.into_iter().collect())
}

pub(crate) fn urls(bytes: &[u8]) -> Result<Vec<SafeRemoteUrl>, RemoteObservationError> {
    if bytes.is_empty() {
        return Ok(vec![]);
    }
    if !bytes.ends_with(b"\n") {
        return Err(RemoteObservationError::InvalidConfiguration);
    }
    let raw =
        std::str::from_utf8(bytes).map_err(|_| RemoteObservationError::InvalidConfiguration)?;
    let mut urls = Vec::new();
    for (ordinal, raw) in raw[..raw.len() - 1].split('\n').enumerate() {
        if ordinal >= 32 || raw.len() > 4096 {
            return Err(RemoteObservationError::LimitExceeded);
        }
        let mut safe = sanitize_remote_url(raw);
        safe.ordinal = ordinal as u32;
        urls.push(safe);
    }
    Ok(urls)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn credentials_are_absent_from_every_serialized_supported_transport() {
        for raw in [
            "https://synthetic-user:synthetic-token@github.com/Owner/Repo.git?auth=synthetic-query#synthetic-fragment",
            "ssh://synthetic-user@github.com:2222/Owner/Repo.git",
            "synthetic-user@github.com:Owner/Repo.git",
        ] {
            let safe = sanitize_remote_url(raw);
            assert!(safe.endpoint.is_some());
            assert!(safe.redacted);
            assert!(!serde_json::to_string(&safe).unwrap().contains("synthetic-"));
            assert_eq!(safe.endpoint.as_ref().unwrap().path, "Owner/Repo.git");
        }
    }
    #[test]
    fn unsafe_path_spellings_are_rejected_before_url_normalization() {
        for raw in [
            "https://github.com/a/../owner/repo",
            "https://github.com//owner/repo",
            "https://github.com/owner%2frepo",
            "https://github.com/owner/repo%0aother",
            "https://github.com?redirect=/owner/repo",
            "ssh://host/~someone/repo",
            "https://host/a\\b/repo",
            "helper::synthetic-token",
            "./foo:bar",
            "https://host/owner/repo\nhttps://other/owner/repo",
        ] {
            let safe = sanitize_remote_url(raw);
            assert!(
                safe.endpoint.is_none(),
                "Unsupported path must not become an endpoint"
            );
            assert!(safe.sanitized_url.is_none());
        }
    }
    #[test]
    fn ports_ipv6_subgroups_and_case_remain_exact_transport_coordinates() {
        let safe = sanitize_remote_url("ssh://git@[2001:db8::1]:2222/base/Group/Sub/Repo.git");
        let endpoint = safe.endpoint.unwrap();
        assert_eq!(endpoint.port, 2222);
        assert_eq!(endpoint.host, "[2001:db8::1]");
        assert_eq!(endpoint.path, "base/Group/Sub/Repo.git");
        assert_eq!(
            sanitize_remote_url("https://GITHUB.COM:443/Owner/Repo.git")
                .endpoint
                .unwrap()
                .host,
            "github.com"
        );
    }
    #[test]
    fn nul_config_frames_preserve_dotted_names_and_reject_embedded_url_newlines() {
        assert_eq!(
            config_names(
                b"remote.fork.with.dot.url\nhttps://host/a/b\0remote.origin.fetch\nrefs/heads/*\0"
            )
            .unwrap(),
            vec!["fork.with.dot", "origin"]
        );
        assert_eq!(
            config_names(b"remote.origin.url\nhttps://host/a/b\nhttps://evil/a/b\0"),
            Err(RemoteObservationError::InvalidConfiguration)
        );
        assert_eq!(
            config_names(b"url.https://host/\ninstead.insteadOf\nshort:\0"),
            Err(RemoteObservationError::InvalidConfiguration)
        );
        assert_eq!(
            config_names(b"remote.origin.url\nhttps://host/a/b"),
            Err(RemoteObservationError::InvalidConfiguration)
        );
    }
}
