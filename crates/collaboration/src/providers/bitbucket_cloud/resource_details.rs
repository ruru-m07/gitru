//! Singleton field authority, independent of list summaries and child facets.
use super::*;
use crate::resource_metadata::*;
use serde_json::{Map, Value};

pub(super) const SOURCE: &str = "bitbucket_cloud/pull-request-detail/v2";
const ADAPTER_VERSION: u32 = 1;
const MAX_BODY: usize = 1_048_576;

pub(super) fn positive_id(raw: &str) -> Option<u64> {
    raw.parse::<u64>()
        .ok()
        .filter(|id| *id > 0 && id.to_string() == raw)
}
pub(super) fn native_id(value: &Value) -> Result<u64, ProviderError> {
    value.as_u64().filter(|id| *id > 0).ok_or_else(invalid)
}
pub(super) fn account(account: &RemoteAccount) -> Result<(), ProviderError> {
    if account.provider != ProviderKind::BitbucketCloud || account.host != "bitbucket.org" {
        return Err(ProviderError::new(ProviderErrorKind::Unsupported));
    }
    if account.state != AccountState::Active {
        return Err(ProviderError::new(ProviderErrorKind::Authentication));
    }
    if text(account.id.clone(), 128, false)? != account.id
        || canonical_uuid(&account.actor_id)? != account.actor_id
        || account
            .authorization_epoch
            .parse::<i64>()
            .ok()
            .filter(|id| *id > 0)
            .is_none_or(|id| id.to_string() != account.authorization_epoch)
    {
        return Err(invalid());
    }
    Ok(())
}
pub(super) fn repository_identity(
    account: &RemoteAccount,
    repository: &RemoteRepository,
) -> Result<String, ProviderError> {
    self::account(account)?;
    let uuid = canonical_uuid(&repository.provider_id)?;
    if uuid != repository.provider_id
        || repository.account_id != account.id
        || !repository.selected
        || repository.id != format!("bitbucket_cloud:repository:{uuid}")
    {
        return Err(invalid());
    }
    Ok(uuid)
}
pub(super) fn identity(json: &Map<String, Value>, repository: &str) -> Result<u64, ProviderError> {
    if json.get("type").and_then(Value::as_str) != Some("pullrequest") {
        return Err(invalid());
    }
    let id = native_id(json.get("id").ok_or_else(invalid)?)?;
    let destination = json
        .get("destination")
        .and_then(Value::as_object)
        .ok_or_else(invalid)?;
    let observed = condensed_repository(destination.get("repository").ok_or_else(invalid)?)?;
    if observed.0 != repository {
        return Err(invalid());
    }
    Ok(id)
}
pub(super) fn state(value: &Value) -> Result<(String, Option<String>), ProviderError> {
    match value.as_str() {
        Some("OPEN") => Ok(("open".into(), None)),
        Some("MERGED") => Ok(("merged".into(), None)),
        Some("DECLINED") => Ok(("closed".into(), Some("declined".into()))),
        Some("SUPERSEDED") => Ok(("closed".into(), Some("superseded".into()))),
        _ => Err(invalid()),
    }
}
pub(super) fn time(value: &Value) -> Result<String, ProviderError> {
    let raw = value
        .as_str()
        .filter(|value| value.len() <= 128)
        .ok_or_else(invalid)?;
    chrono::DateTime::parse_from_rfc3339(raw).map_err(|_| invalid())?;
    Ok(raw.into())
}
fn object<'a>(
    json: &'a Map<String, Value>,
    key: &str,
) -> Result<Option<&'a Map<String, Value>>, ProviderError> {
    match json.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Object(value)) => Ok(Some(value)),
        _ => Err(invalid()),
    }
}
pub(super) fn raw_description(json: &Map<String, Value>) -> Result<Option<&Value>, ProviderError> {
    let raw = match object(json, "rendered")? {
        Some(rendered) => object(rendered, "description")?.and_then(|value| value.get("raw")),
        None => None,
    };
    if let Some(raw) = raw {
        if !matches!(raw, Value::String(_) | Value::Null)
            || json.get("description").is_some_and(|top| top != raw)
        {
            return Err(invalid());
        }
        if raw.as_str().is_some_and(|value| value.contains('\0')) {
            return Err(invalid());
        }
    }
    Ok(raw)
}
fn description(json: &Map<String, Value>) -> Result<DetailValue, ProviderError> {
    Ok(match raw_description(json)? {
        None => DetailValue {
            state: DetailValueState::Omitted,
            text: None,
        },
        Some(Value::Null) => DetailValue {
            state: DetailValueState::Known,
            text: None,
        },
        Some(Value::String(value)) if value.len() <= MAX_BODY => DetailValue {
            state: DetailValueState::Known,
            text: Some(value.clone()),
        },
        Some(Value::String(_)) => DetailValue {
            state: DetailValueState::Oversized,
            text: None,
        },
        _ => return Err(invalid()),
    })
}
fn safe_web(raw: &str) -> Result<reqwest::Url, ProviderError> {
    if raw.len() > 2048
        || raw.trim() != raw
        || raw.chars().any(|c| c.is_control() || c.is_whitespace())
        || raw.contains(['\\', '#'])
        || raw.split('?').next().is_none_or(|path| {
            path.split('/').any(|v| matches!(v, "." | ".."))
                || path.to_ascii_lowercase().contains("%2e")
        })
    {
        return Err(invalid());
    }
    let url = reqwest::Url::parse(raw).map_err(|_| invalid())?;
    if url.scheme() != "https"
        || url.host_str() != Some("bitbucket.org")
        || url.port().is_some()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(invalid());
    }
    Ok(url)
}
fn path(raw: &str) -> bool {
    let values: Vec<_> = raw.split('/').collect();
    values.len() == 2 && values.iter().all(|value| discovery::segment(value))
}
fn html(json: &Map<String, Value>) -> Result<Option<&Value>, ProviderError> {
    let Some(links) = object(json, "links")? else {
        return Ok(None);
    };
    Ok(object(links, "html")?.and_then(|html| html.get("href")))
}
pub(super) fn resource_web(value: &Value, id: u64) -> Result<String, ProviderError> {
    let raw = value.as_str().ok_or_else(invalid)?;
    let url = safe_web(raw)?;
    let suffix = format!("/pull-requests/{id}");
    let full_name = url
        .path()
        .trim_end_matches('/')
        .strip_prefix('/')
        .and_then(|value| value.strip_suffix(&suffix))
        .ok_or_else(invalid)?;
    if !path(full_name) {
        return Err(invalid());
    }
    Ok(raw.into())
}
pub(super) fn actor(value: &Value) -> Result<DetailActor, ProviderError> {
    let value = value.as_object().ok_or_else(invalid)?;
    if value
        .get("type")
        .is_some_and(|kind| kind.as_str() != Some("user"))
    {
        return Err(invalid());
    }
    let uuid = canonical_uuid(
        value
            .get("uuid")
            .and_then(Value::as_str)
            .ok_or_else(invalid)?,
    )?;
    let login = match value.get("nickname") {
        None | Some(Value::Null) => uuid.clone(),
        Some(Value::String(value)) => text(value.clone(), 255, false)?,
        _ => return Err(invalid()),
    };
    let web_url = html(value)?
        .map(|value| {
            let raw = value.as_str().ok_or_else(invalid)?;
            let url = safe_web(raw)?;
            if !discovery::segment(url.path().trim_matches('/')) {
                return Err(invalid());
            }
            Ok(raw.into())
        })
        .transpose()?;
    Ok(DetailActor {
        provider_id: uuid,
        login,
        web_url,
    })
}
fn condensed_repository(
    value: &Value,
) -> Result<(String, Option<DetailRepositoryRef>), ProviderError> {
    let value = value.as_object().ok_or_else(invalid)?;
    if value
        .get("type")
        .is_some_and(|kind| kind.as_str() != Some("repository"))
    {
        return Err(invalid());
    }
    let provider_id = canonical_uuid(
        value
            .get("uuid")
            .and_then(Value::as_str)
            .ok_or_else(invalid)?,
    )?;
    let full_name = match value.get("full_name") {
        None | Some(Value::Null) => None,
        Some(Value::String(value)) if path(value) => Some(value.clone()),
        _ => return Err(invalid()),
    };
    let web_url = html(value)?
        .map(|value| {
            let raw = value.as_str().ok_or_else(invalid)?;
            let url = safe_web(raw)?;
            let name = url.path().trim_matches('/');
            if !path(name) || full_name.as_ref().is_some_and(|expected| expected != name) {
                return Err(invalid());
            }
            Ok(raw.into())
        })
        .transpose()?;
    Ok((
        provider_id.clone(),
        full_name.map(|full_name| DetailRepositoryRef {
            provider_id,
            full_name,
            web_url,
        }),
    ))
}
fn oid(value: &Value) -> Result<Option<String>, ProviderError> {
    match value {
        Value::Null => Ok(None),
        Value::String(value)
            if value.len() >= 7
                && value.len() <= 64
                && value.bytes().all(|b| b.is_ascii_hexdigit()) =>
        {
            Ok(matches!(value.len(), 40 | 64).then(|| value.to_ascii_lowercase()))
        }
        _ => Err(invalid()),
    }
}
pub(super) fn branch(
    json: &Map<String, Value>,
    key: &str,
) -> Result<Option<DetailBranch>, ProviderError> {
    let Some(endpoint) = object(json, key)? else {
        return Ok(None);
    };
    let name = object(endpoint, "branch")?
        .and_then(|value| value.get("name"))
        .map(|value| text(value.as_str().ok_or_else(invalid)?.into(), 1024, false))
        .transpose()?;
    let oid = object(endpoint, "commit")?
        .and_then(|value| value.get("hash"))
        .map(oid)
        .transpose()?
        .flatten();
    let repository = endpoint
        .get("repository")
        .filter(|value| !value.is_null())
        .map(condensed_repository)
        .transpose()?
        .and_then(|(_, presentation)| presentation);
    Ok(name.zip(oid).map(|(name, oid)| DetailBranch {
        name,
        oid,
        repository,
    }))
}
fn observe<T>(
    json: &Map<String, Value>,
    key: &str,
    field: MetadataField,
    parse: impl FnOnce(&Value) -> Result<T, ProviderError>,
) -> Result<(MetadataObservedField, Option<T>), ProviderError> {
    let (state, value) = match json.get(key) {
        None => (DetailValueState::Omitted, None),
        Some(Value::Null) => (DetailValueState::Known, None),
        Some(value) => (DetailValueState::Known, Some(parse(value)?)),
    };
    Ok((MetadataObservedField { field, state }, value))
}
fn normalize(
    request: &DetailRequest,
    json: &Map<String, Value>,
    repository: &str,
    id: u64,
) -> Result<(DetailValue, ResourceMetadataObservation), ProviderError> {
    if identity(json, repository)? != id {
        return Err(invalid());
    }
    let body = description(json)?;
    let mut fields = vec![];
    let mut values = ResourceMetadataValues::default();
    macro_rules! scalar {
        ($key:literal, $field:ident, $target:ident, $parse:expr) => {{
            let (field, value) = observe(json, $key, MetadataField::$field, $parse)?;
            fields.push(field);
            values.$target = value;
        }};
    }
    if let Some(Value::String(title)) = json.get("title")
        && title.len() > 16384
        && !title.chars().any(char::is_control)
    {
        fields.push(MetadataObservedField {
            field: MetadataField::Title,
            state: DetailValueState::Oversized,
        });
    } else {
        scalar!("title", Title, title, |v| text(
            v.as_str().ok_or_else(invalid)?.into(),
            16384,
            false
        ));
    }
    scalar!("author", Author, author, actor);
    scalar!("updated_on", UpdatedAt, updated_at, time);
    let (state_field, observed_state) = observe(json, "state", MetadataField::State, state)?;
    let reason_field = MetadataObservedField {
        field: MetadataField::StateReason,
        state: state_field.state,
    };
    if let Some((state, reason)) = observed_state {
        values.state = Some(state);
        values.state_reason = reason;
    }
    fields.extend([state_field, reason_field]);
    let (web_field, web_url) = match html(json)? {
        None => (DetailValueState::Omitted, None),
        Some(value) => (DetailValueState::Known, Some(resource_web(value, id)?)),
    };
    values.web_url = web_url;
    fields.push(MetadataObservedField {
        field: MetadataField::WebUrl,
        state: web_field,
    });
    values.head = branch(json, "source")?;
    let base = branch(json, "destination")?;
    values.base = values.head.as_ref().and(base);
    for (field, known) in [
        (MetadataField::Head, values.head.is_some()),
        (MetadataField::Base, values.base.is_some()),
    ] {
        fields.push(MetadataObservedField {
            field,
            state: if known {
                DetailValueState::Known
            } else {
                DetailValueState::Omitted
            },
        });
    }
    // Participant/task/draft/merge fields have no common authority in this slice.
    for field in [
        MetadataField::Labels,
        MetadataField::Assignees,
        MetadataField::Milestone,
        MetadataField::IsDraft,
        MetadataField::MergedAt,
    ] {
        fields.push(MetadataObservedField {
            field,
            state: DetailValueState::Omitted,
        });
    }
    Ok((
        body,
        ResourceMetadataObservation {
            kind: request.subject.kind.clone(),
            source: MetadataSource {
                source: SOURCE.into(),
                adapter_version: ADAPTER_VERSION,
                provider_updated_at: values.updated_at.clone(),
                observed_at: chrono::Utc::now().to_rfc3339(),
            },
            values,
            fields,
        },
    ))
}
impl BitbucketCloudProvider {
    pub(super) async fn resource_details(
        &self,
        token: &SecretToken,
        request: DetailRequest,
    ) -> Result<DetailPage, ProviderError> {
        if request.facet != DetailFacet::Body || request.subject.kind != RemoteItemKind::PullRequest
        {
            return Err(ProviderError::new(ProviderErrorKind::Unsupported));
        }
        let (repository, id) = subject_identity(&request)?;
        let route = Route::PullRequest(repository.clone(), id);
        let response = self
            .http
            .get(self.http.endpoint(&route)?, &route, token)
            .await?;
        let result = (|| {
            let json = serde_json::from_slice(&response.body).map_err(|_| invalid())?;
            let (body, metadata) = normalize(&request, &json, &repository, id)?;
            Ok(DetailPage {
                reconciliation: DetailReconciliation::default(),
                body,
                source: DetailSource {
                    source: SOURCE.into(),
                    adapter_version: ADAPTER_VERSION,
                    field_mask: vec![DetailField::Body],
                    provider_updated_at: metadata.source.provider_updated_at.clone(),
                    observed_at: metadata.source.observed_at.clone(),
                },
                metadata: Some(metadata),
                entries: vec![],
                next_cursor: None,
                etag: None,
                not_modified: false,
                freshness_seconds: 180,
                cooldown_seconds: response.cooldown,
            })
        })();
        result.map_err(|error| quota(error, response.cooldown))
    }
}

/// Shared singleton request identity; each facet still owns its own fields.
pub(super) fn subject_identity(request: &DetailRequest) -> Result<(String, u64), ProviderError> {
    let repository = repository_identity(&request.account, &request.repository)?;
    let id = request
        .subject
        .number
        .as_deref()
        .and_then(positive_id)
        .ok_or_else(invalid)?;
    if request.cursor.is_some()
        || request.subject.kind != RemoteItemKind::PullRequest
        || request.subject.account_id != request.account.id
        || request.subject.repository_id.as_ref() != Some(&request.repository.id)
        || request.subject.provider_id != format!("{repository}:{id}")
        || request.subject.id != format!("bitbucket_cloud:pull:{repository}:{id}")
    {
        return Err(invalid());
    }
    Ok((repository, id))
}
