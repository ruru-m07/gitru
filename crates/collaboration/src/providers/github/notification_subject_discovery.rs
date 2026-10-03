//! Bounded point discovery verifies immutable identity before constructing details.
//! Selectors never supply HTTP authority or a native child identity.
use super::{resource_details::*, *};
use crate::{
    NotificationSubjectKind as Kind, NotificationSubjectReason as Reason,
    NotificationSubjectRepresentation as Representation,
};
use serde_json::{Map, Value};

impl GithubProvider {
    pub(super) async fn request_notification_subject_discovery(
        &self,
        token: &SecretToken,
        request: TrustedNotificationSubjectRequest,
    ) -> Result<NotificationSubjectDiscovery, ProviderError> {
        validate_request(self, &request)?;
        let resource = match request.selector.kind {
            Kind::PullRequest => "pulls",
            Kind::Issue => "issues",
        };
        // Only the documented named operation is requested. A native URL in a
        // response can be identity evidence without becoming a GET destination.
        let endpoint = self.http.endpoint(&format!(
            "repos/{}/{resource}/{}",
            request.repository.full_name, request.selector.number
        ))?;
        let response = self.http.get_point(endpoint, token).await?;
        let normalized = if response.not_modified || response.next_url.is_some() {
            Err(invalid())
        } else {
            normalize_response(
                request,
                &response.body,
                response.validators.etag,
                response.cooldown_seconds,
            )
        };
        Ok(
            normalized.unwrap_or_else(|error| NotificationSubjectDiscovery::Failed {
                error,
                cooldown_seconds: response.cooldown_seconds,
            }),
        )
    }
}

fn canonical_positive(value: &str) -> bool {
    value
        .parse::<u64>()
        .is_ok_and(|id| id > 0 && id.to_string() == value)
}
fn bounded_identity(value: &str) -> bool {
    !value.is_empty() && value.len() <= 256 && !value.chars().any(char::is_control)
}
fn validate_request(
    provider: &GithubProvider,
    request: &TrustedNotificationSubjectRequest,
) -> Result<(), ProviderError> {
    if request.account.provider != ProviderKind::Github
        || request.account.host != "github.com"
        || request.instance_id != provider.instance().id
        || !request.account.notifications_supported
    {
        return Err(ProviderError::new(ProviderErrorKind::Unsupported));
    }
    if request.account.state != AccountState::Active {
        return Err(ProviderError::new(ProviderErrorKind::Authentication));
    }
    if request.repository.account_id != request.account.id
        || request.selector.repository_provider_id != request.repository.provider_id
        || request.selector.repository_path != request.repository.full_name
        || !valid_repository_path(&request.repository.full_name)
        || !canonical_positive(&request.repository.provider_id)
        || !canonical_positive(&request.selector.number)
        || !canonical_positive(&request.account.authorization_epoch)
        || !canonical_positive(&request.authorization_view)
        || !bounded_identity(&request.account.id)
        || !bounded_identity(&request.repository.id)
        || !bounded_identity(&request.notification_id)
        || !bounded_identity(&request.selector_generation)
        || !matches!(
            (request.selector.kind, request.selector.representation),
            (Kind::PullRequest, Representation::GithubPullRequest)
                | (Kind::Issue, Representation::GithubIssue)
        )
    {
        return Err(invalid());
    }
    Ok(())
}

/// Preserve raw spelling until all normalization attacks have been excluded.
fn exact_url_path(value: &str, host: &str, paths: &[String]) -> Result<String, ProviderError> {
    if value.len() > 2048
        || value.chars().any(|c| c.is_control() || c.is_whitespace())
        || value.contains(['%', '\\', '?', '#'])
    {
        return Err(invalid());
    }
    let (scheme, rest) = value.split_once("://").ok_or_else(invalid)?;
    let slash = rest.find('/').ok_or_else(invalid)?;
    let raw_path = &rest[slash..];
    if !scheme.eq_ignore_ascii_case("https")
        || rest[..slash].contains('@')
        || raw_path
            .strip_prefix('/')
            .ok_or_else(invalid)?
            .split('/')
            .any(|segment| matches!(segment, "" | "." | ".."))
    {
        return Err(invalid());
    }
    let url = reqwest::Url::parse(value).map_err(|_| invalid())?;
    if url.scheme() != "https"
        || url.host_str() != Some(host)
        || url.port_or_known_default() != Some(443)
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || url.path() != raw_path
        || !paths.iter().any(|path| path == raw_path)
    {
        return Err(invalid());
    }
    Ok(raw_path.into())
}
fn text<'a>(json: &'a Map<String, Value>, field: &str) -> Result<&'a str, ProviderError> {
    json.get(field).and_then(Value::as_str).ok_or_else(invalid)
}

fn normalize_response(
    request: TrustedNotificationSubjectRequest,
    bytes: &[u8],
    etag: Option<String>,
    cooldown_seconds: Option<u64>,
) -> Result<NotificationSubjectDiscovery, ProviderError> {
    let json = object(bytes)?;
    let native_id = id(json.get("id").ok_or_else(invalid)?)?;
    let number = id(json.get("number").ok_or_else(invalid)?)?;
    if number != request.selector.number {
        return Err(invalid());
    }
    let named = format!("/repos/{}", request.repository.full_name);
    let immutable = format!("/repositories/{}", request.repository.provider_id);
    let (resource, web_resource) = match request.selector.kind {
        Kind::PullRequest => ("pulls", "pull"),
        Kind::Issue => ("issues", "issues"),
    };
    exact_url_path(
        text(&json, "url")?,
        "api.github.com",
        &[
            format!("{named}/{resource}/{number}"),
            format!("{immutable}/{resource}/{number}"),
        ],
    )?;
    if let Some(value) = json.get("html_url") {
        exact_url_path(
            value.as_str().ok_or_else(invalid)?,
            "github.com",
            &[format!(
                "/{}/{web_resource}/{number}",
                request.repository.full_name
            )],
        )?;
    }
    if request.selector.kind == Kind::Issue {
        // Even a null/malformed marker is an issue-side PR representation.
        if json.contains_key("pull_request") {
            return Ok(NotificationSubjectDiscovery::Unresolved {
                reason: Reason::RepresentationMismatch,
                cooldown_seconds,
            });
        }
        let parent_path = exact_url_path(
            text(&json, "repository_url")?,
            "api.github.com",
            &[named, immutable.clone()],
        )?;
        let inline = match json.get("repository") {
            None => false,
            Some(value) => {
                let parent = value.get("id").ok_or_else(invalid)?;
                if id(parent)? != request.repository.provider_id {
                    return Err(invalid());
                }
                true
            }
        };
        if !inline && parent_path != immutable {
            // A separate repository GET would leave a path-reuse TOCTOU gap.
            return Ok(NotificationSubjectDiscovery::Unresolved {
                reason: Reason::IdentityUnverified,
                cooldown_seconds,
            });
        }
    } else if id(json
        .get("base")
        .and_then(|value| value.get("repo"))
        .and_then(|value| value.get("id"))
        .ok_or_else(invalid)?)?
        != request.repository.provider_id
    {
        return Err(invalid());
    }
    let kind = request.selector.kind.item_kind();
    let source = if kind == RemoteItemKind::PullRequest {
        PULL_SOURCE
    } else {
        issue_details::ISSUE_SOURCE
    };
    // A temporary DTO carries only the now-verified native coordinates. The
    // canonical storage identity may already have a different opaque local ID.
    let mut subject = RemoteItem {
        id: format!(
            "github:{}:{native_id}",
            if kind == RemoteItemKind::PullRequest {
                "pull"
            } else {
                "issue"
            }
        ),
        account_id: request.account.id.clone(),
        repository_id: Some(request.repository.id.clone()),
        provider_id: native_id,
        kind: kind.clone(),
        number: Some(number),
        title: String::new(),
        body: None,
        body_omitted: true,
        author: None,
        web_url: None,
        state: String::new(),
        updated_at: String::new(),
        head_oid: None,
        is_draft: None,
        reason: None,
        unread: None,
    };
    let detail_request = DetailRequest {
        account: request.account,
        repository: request.repository,
        subject: subject.clone(),
        facet: DetailFacet::Body,
        cursor: None,
        etag: None,
        source: None,
    };
    let (body, metadata) = if kind == RemoteItemKind::PullRequest {
        pull_details::normalize(&detail_request, bytes, source)?
    } else {
        issue_details::normalize(&detail_request, bytes, source)?
    };
    subject.title = metadata.values.title.clone().ok_or_else(invalid)?;
    subject.state = metadata.values.state.clone().ok_or_else(invalid)?;
    subject.updated_at = metadata.values.updated_at.clone().ok_or_else(invalid)?;
    subject.author = metadata
        .values
        .author
        .as_ref()
        .map(|actor| actor.login.clone());
    subject.web_url = metadata.values.web_url.clone();
    subject.head_oid = metadata.values.head.as_ref().map(|head| head.oid.clone());
    subject.is_draft = metadata.values.is_draft;
    if body.state == DetailValueState::Known
        && body.text.as_ref().is_none_or(|text| text.len() <= 65536)
    {
        subject.body = body.text.clone();
        subject.body_omitted = false;
    }
    Ok(NotificationSubjectDiscovery::Verified {
        subject: Box::new(subject),
        detail: Box::new(DetailPage {
            body,
            source: DetailSource {
                source: source.into(),
                adapter_version: ADAPTER_VERSION,
                field_mask: vec![DetailField::Body],
                provider_updated_at: metadata.source.provider_updated_at.clone(),
                observed_at: metadata.source.observed_at.clone(),
            },
            metadata: Some(metadata),
            entries: vec![],
            next_cursor: None,
            etag,
            not_modified: false,
            freshness_seconds: 180,
            cooldown_seconds,
        }),
        endpoint_aliases: vec![],
    })
}

#[cfg(test)]
mod tests;
