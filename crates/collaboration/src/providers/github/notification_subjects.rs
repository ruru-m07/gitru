//! Strict notification subject coordinates. This module performs no HTTP or storage.
use super::valid_repository_path;
use crate::{
    NotificationSubjectFallbackReason as Reason, NotificationSubjectKind as Kind,
    NotificationSubjectMapping as Mapping, NotificationSubjectRepresentation as Representation,
    NotificationSubjectSelector, RemoteRepository,
};
use serde_json::Value;
use url::Url;

const MAX_SUBJECT_URL_BYTES: usize = 2048;

/// `api_base` and `repository` are native adapter configuration and the parent
/// captured in the same notification, not inputs from a renderer action.
/// A native route is a parser fact, not evidence that a GET endpoint exists.
pub fn normalize(api_base: &Url, repository: &RemoteRepository, subject: &Value) -> Mapping {
    let kind = match subject.get("type") {
        None | Some(Value::Null) => return fallback(Reason::MissingSubjectType),
        Some(Value::String(kind)) if kind == "PullRequest" => Kind::PullRequest,
        Some(Value::String(kind)) if kind == "Issue" => Kind::Issue,
        Some(Value::String(_)) => return fallback(Reason::UnsupportedSubjectType),
        Some(_) => return fallback(Reason::InvalidSubjectType),
    };
    let raw = match subject.get("url") {
        None | Some(Value::Null) => return fallback(Reason::MissingSubjectUrl),
        Some(Value::String(value)) => value,
        Some(_) => return fallback(Reason::InvalidSubjectUrl),
    };
    if !valid_api_base(api_base) {
        return fallback(Reason::InvalidApiConfiguration);
    }
    if !valid_repository_path(&repository.full_name) || !positive_decimal(&repository.provider_id) {
        return fallback(Reason::InvalidRepository);
    }
    // Reject spells that URL parsers remove or normalize before comparing paths.
    // Nothing from this raw string is retained in the result or diagnostics.
    let Some(raw_path) = strict_raw_path(raw) else {
        return fallback(Reason::InvalidSubjectUrl);
    };
    let Ok(url) = Url::parse(raw) else {
        return fallback(Reason::InvalidSubjectUrl);
    };
    if url.origin() != api_base.origin()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || url.path() != raw_path
    {
        return fallback(Reason::InvalidSubjectUrl);
    }
    let Some(relative) = raw_path.strip_prefix(api_base.path()) else {
        return fallback(Reason::InvalidSubjectUrl);
    };
    let coordinates: Vec<_> = relative.split('/').collect();
    let (parent_matches, resource, number) = match coordinates.as_slice() {
        ["repos", owner, name, resource, number] => (
            format!("{owner}/{name}") == repository.full_name,
            *resource,
            *number,
        ),
        ["repositories", native, resource, number] => (
            *native == repository.provider_id && positive_decimal(native),
            *resource,
            *number,
        ),
        _ => return fallback(Reason::InvalidSubjectUrl),
    };
    if !parent_matches {
        return fallback(Reason::RepositoryMismatch);
    }
    let representation = match (kind, resource) {
        (Kind::Issue, "issues") => Representation::GithubIssue,
        (Kind::PullRequest, "pulls") => Representation::GithubPullRequest,
        (_, "issues" | "pulls") => return fallback(Reason::RepresentationMismatch),
        _ => return fallback(Reason::InvalidSubjectUrl),
    };
    if !positive_decimal(number) {
        return fallback(Reason::InvalidSubjectUrl);
    }
    Mapping::Selector(NotificationSubjectSelector {
        kind,
        repository_provider_id: repository.provider_id.clone(),
        number: number.into(),
        repository_path: repository.full_name.clone(),
        representation,
    })
}

fn fallback(reason: Reason) -> Mapping {
    Mapping::Fallback(reason)
}

fn positive_decimal(value: &str) -> bool {
    value
        .parse::<u64>()
        .is_ok_and(|number| number > 0 && number.to_string() == value)
}

fn valid_api_base(base: &Url) -> bool {
    base.scheme() == "https"
        && base.host_str().is_some()
        && base.username().is_empty()
        && base.password().is_none()
        && base.query().is_none()
        && base.fragment().is_none()
        && base.path().ends_with('/')
        && !base.path().contains(['%', '\\'])
        && base
            .path()
            .strip_prefix('/')
            .and_then(|path| path.strip_suffix('/'))
            .is_none_or(|path| {
                path.split('/')
                    .all(|segment| !matches!(segment, "" | "." | ".."))
            })
}

fn strict_raw_path(raw: &str) -> Option<&str> {
    if raw.len() > MAX_SUBJECT_URL_BYTES
        || raw.chars().any(|c| c.is_control() || c.is_whitespace())
        || raw.contains(['%', '\\', '?', '#'])
    {
        return None;
    }
    let (scheme, rest) = raw.split_once("://")?;
    if !scheme.eq_ignore_ascii_case("https") {
        return None;
    }
    let slash = rest.find('/')?;
    if slash == 0 || rest[..slash].contains('@') {
        return None;
    }
    let path = &rest[slash..];
    if path
        .strip_prefix('/')?
        .split('/')
        .any(|segment| matches!(segment, "" | "." | ".."))
    {
        return None;
    }
    Some(path)
}

#[cfg(test)]
mod tests;
