//! Bounded GitHub resource observations; no URL-derived canonical identities.
use super::*;
use crate::resource_metadata::*;
use serde_json::{Map, Value};

pub(super) const PULL_SOURCE: &str = "github/pull-detail/2026-03-10";
pub(super) const ADAPTER_VERSION: u32 = 1;
pub(super) type Normalized = (DetailValue, ResourceMetadataObservation);
pub(super) fn invalid() -> ProviderError {
    ProviderError::new(ProviderErrorKind::InvalidResponse)
}
pub(super) fn object(body: &[u8]) -> Result<Map<String, Value>, ProviderError> {
    serde_json::from_slice(body).map_err(|_| invalid())
}
pub(super) fn id(value: &Value) -> Result<String, ProviderError> {
    value
        .as_u64()
        .filter(|id| *id > 0)
        .map(|id| id.to_string())
        .ok_or_else(invalid)
}

pub(super) fn validate_identity(
    request: &DetailRequest,
    json: &Map<String, Value>,
) -> Result<(), ProviderError> {
    if request
        .repository
        .provider_id
        .parse::<u64>()
        .ok()
        .is_none_or(|id| id == 0)
    {
        return Err(invalid());
    }
    if request.account.provider != ProviderKind::Github
        || request.account.host != "github.com"
        || request.repository.account_id != request.account.id
        || request.subject.account_id != request.account.id
        || request.subject.repository_id.as_ref() != Some(&request.repository.id)
        || request.facet != DetailFacet::Body
        || request.cursor.is_some()
        || !valid_repository_path(&request.repository.full_name)
    {
        return Err(invalid());
    }
    let native = id(json.get("id").ok_or_else(invalid)?)?;
    let number = id(json.get("number").ok_or_else(invalid)?)?;
    if native != request.subject.provider_id || Some(&number) != request.subject.number.as_ref() {
        return Err(invalid());
    }
    let named = format!("/repos/{}", request.repository.full_name);
    let immutable = format!("/repositories/{}", request.repository.provider_id);
    let resource = if request.subject.kind == RemoteItemKind::PullRequest {
        "pulls"
    } else {
        "issues"
    };
    api_identity_url(
        json.get("url")
            .and_then(Value::as_str)
            .ok_or_else(invalid)?,
        &[
            format!("{named}/{resource}/{number}"),
            format!("{immutable}/{resource}/{number}"),
        ],
    )?;
    if let Some(value) = json.get("html_url") {
        let value = value.as_str().ok_or_else(invalid)?;
        web(&Value::String(value.into())).map_err(|_| invalid())?;
        let url = reqwest::Url::parse(value).map_err(|_| invalid())?;
        let resource = if request.subject.kind == RemoteItemKind::PullRequest {
            "pull"
        } else {
            "issues"
        };
        if url.path() != format!("/{}/{resource}/{number}", request.repository.full_name) {
            return Err(invalid());
        }
    }
    if request.subject.kind == RemoteItemKind::Issue {
        let url = json
            .get("repository_url")
            .and_then(Value::as_str)
            .ok_or_else(invalid)?;
        api_identity_url(url, &[named, immutable])?;
        if json.contains_key("pull_request") {
            return Err(invalid());
        }
    } else if request.subject.kind == RemoteItemKind::PullRequest {
        let native = json
            .get("base")
            .and_then(|b| b.get("repo"))
            .and_then(|r| r.get("id"))
            .ok_or_else(invalid)?;
        if id(native)? != request.repository.provider_id {
            return Err(invalid());
        }
    } else {
        return Err(invalid());
    }
    Ok(())
}
fn api_identity_url(value: &str, paths: &[String]) -> Result<(), ProviderError> {
    let url = reqwest::Url::parse(value).map_err(|_| invalid())?;
    if url.scheme() != "https"
        || url.host_str() != Some("api.github.com")
        || url.port_or_known_default() != Some(443)
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || !paths.iter().any(|path| path == url.path())
    {
        return Err(invalid());
    }
    Ok(())
}
pub(super) fn body(json: &Map<String, Value>) -> Result<DetailValue, ProviderError> {
    Ok(match json.get("body") {
        None => DetailValue {
            state: DetailValueState::Omitted,
            text: None,
        },
        Some(Value::Null) => DetailValue {
            state: DetailValueState::Known,
            text: None,
        },
        Some(Value::String(value)) if value.len() <= 1_048_576 => DetailValue {
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

#[derive(Debug)]
pub(super) enum FieldError {
    Invalid,
    Oversized,
}
type FieldResult<T> = Result<T, FieldError>;
pub(super) fn string(value: &Value, max: usize) -> FieldResult<String> {
    let value = value.as_str().ok_or(FieldError::Invalid)?;
    if value.len() > max {
        return Err(FieldError::Oversized);
    }
    Ok(value.into())
}
fn native(value: &Value) -> FieldResult<String> {
    id(value).map_err(|_| FieldError::Invalid)
}
fn web(value: &Value) -> FieldResult<String> {
    let value = string(value, 2048)?;
    let url = reqwest::Url::parse(&value).map_err(|_| FieldError::Invalid)?;
    if url.scheme() != "https"
        || url.host_str() != Some("github.com")
        || url.port_or_known_default() != Some(443)
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(FieldError::Invalid);
    }
    Ok(value)
}
fn optional<T>(
    json: &Map<String, Value>,
    key: &str,
    parse: impl FnOnce(&Value) -> FieldResult<T>,
) -> FieldResult<Option<T>> {
    match json.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(value) => parse(value).map(Some),
    }
}
pub(super) fn actor(value: &Value) -> FieldResult<DetailActor> {
    let value = value.as_object().ok_or(FieldError::Invalid)?;
    Ok(DetailActor {
        provider_id: native(value.get("id").ok_or(FieldError::Invalid)?)?,
        login: string(value.get("login").ok_or(FieldError::Invalid)?, 255)?,
        web_url: optional(value, "html_url", web)?,
    })
}
fn labels(value: &Value) -> FieldResult<Vec<DetailLabel>> {
    let values = value.as_array().ok_or(FieldError::Invalid)?;
    if values.len() > 100 {
        return Err(FieldError::Oversized);
    }
    values
        .iter()
        .map(|value| {
            if value.is_string() {
                return Ok(DetailLabel {
                    provider_id: None,
                    name: string(value, 1024)?,
                    color: None,
                });
            }
            let value = value.as_object().ok_or(FieldError::Invalid)?;
            Ok(DetailLabel {
                provider_id: optional(value, "id", native)?,
                name: string(value.get("name").ok_or(FieldError::Invalid)?, 1024)?,
                color: optional(value, "color", |v| string(v, 32))?,
            })
        })
        .collect()
}
fn assignees(value: &Value) -> FieldResult<Vec<DetailActor>> {
    let values = value.as_array().ok_or(FieldError::Invalid)?;
    if values.len() > 100 {
        return Err(FieldError::Oversized);
    }
    values.iter().map(actor).collect()
}
fn milestone(value: &Value) -> FieldResult<DetailMilestone> {
    let value = value.as_object().ok_or(FieldError::Invalid)?;
    Ok(DetailMilestone {
        provider_id: native(value.get("id").ok_or(FieldError::Invalid)?)?,
        number: optional(value, "number", native)?,
        title: string(value.get("title").ok_or(FieldError::Invalid)?, 16384)?,
        state: optional(value, "state", |v| string(v, 128))?,
        web_url: optional(value, "html_url", web)?,
    })
}
pub(super) fn branch(value: &Value) -> FieldResult<DetailBranch> {
    let value = value.as_object().ok_or(FieldError::Invalid)?;
    let repository = optional(value, "repo", |v| {
        let v = v.as_object().ok_or(FieldError::Invalid)?;
        let full_name = string(v.get("full_name").ok_or(FieldError::Invalid)?, 1024)?;
        if !valid_repository_path(&full_name) {
            return Err(FieldError::Invalid);
        }
        Ok(DetailRepositoryRef {
            provider_id: native(v.get("id").ok_or(FieldError::Invalid)?)?,
            full_name,
            web_url: optional(v, "html_url", web)?,
        })
    })?;
    Ok(DetailBranch {
        name: string(value.get("ref").ok_or(FieldError::Invalid)?, 1024)?,
        oid: string(value.get("sha").ok_or(FieldError::Invalid)?, 128)?,
        repository,
    })
}
fn time(value: &Value) -> FieldResult<String> {
    let value = string(value, 128)?;
    if chrono::DateTime::parse_from_rfc3339(&value).is_err() {
        return Err(FieldError::Invalid);
    }
    Ok(value)
}
pub(super) fn observe<T>(
    json: &Map<String, Value>,
    key: &str,
    field: MetadataField,
    parse: impl FnOnce(&Value) -> FieldResult<T>,
) -> Result<(MetadataObservedField, Option<T>), ProviderError> {
    let (state, value) = match json.get(key) {
        None => (DetailValueState::Omitted, None),
        Some(Value::Null) => (DetailValueState::Known, None),
        Some(value) => match parse(value) {
            Ok(v) => (DetailValueState::Known, Some(v)),
            Err(FieldError::Oversized) => (DetailValueState::Oversized, None),
            Err(FieldError::Invalid) => return Err(invalid()),
        },
    };
    Ok((MetadataObservedField { field, state }, value))
}
pub(super) fn normalize_common(
    json: &Map<String, Value>,
    kind: RemoteItemKind,
    source: &str,
) -> Result<ResourceMetadataObservation, ProviderError> {
    let mut values = ResourceMetadataValues::default();
    let mut fields = vec![];
    macro_rules! scalar {
        ($key:literal,$field:ident,$target:ident,$parser:expr) => {{
            let (state, value) = observe(json, $key, MetadataField::$field, $parser)?;
            fields.push(state);
            values.$target = value;
        }};
    }
    scalar!("title", Title, title, |v| string(v, 16384));
    scalar!("state", State, state, |v| string(v, 128));
    scalar!("state_reason", StateReason, state_reason, |v| string(
        v, 128
    ));
    scalar!("user", Author, author, actor);
    scalar!("html_url", WebUrl, web_url, web);
    scalar!("updated_at", UpdatedAt, updated_at, time);
    scalar!("milestone", Milestone, milestone, milestone);
    for (key, field) in [
        ("labels", MetadataField::Labels),
        ("assignees", MetadataField::Assignees),
    ] {
        if json.get(key) == Some(&Value::Null) {
            return Err(invalid());
        }
        if field == MetadataField::Labels {
            let (state, value) = observe(json, key, field, labels)?;
            fields.push(state);
            values.labels = value.unwrap_or_default();
        } else {
            let (state, value) = observe(json, key, field, assignees)?;
            fields.push(state);
            values.assignees = value.unwrap_or_default();
        }
    }
    let source = MetadataSource {
        source: source.into(),
        adapter_version: ADAPTER_VERSION,
        provider_updated_at: values.updated_at.clone(),
        observed_at: chrono::Utc::now().to_rfc3339(),
    };
    Ok(ResourceMetadataObservation {
        kind,
        values,
        fields,
        source,
    })
}
pub(super) fn bound_metadata(
    observation: &mut ResourceMetadataObservation,
) -> Result<(), ProviderError> {
    while serde_json::to_vec(&observation.values)
        .map_err(|_| invalid())?
        .len()
        > 262144
    {
        let labels = serde_json::to_vec(&observation.values.labels)
            .map_err(|_| invalid())?
            .len();
        let assignees = serde_json::to_vec(&observation.values.assignees)
            .map_err(|_| invalid())?
            .len();
        let field = if labels >= assignees && !observation.values.labels.is_empty() {
            observation.values.labels.clear();
            MetadataField::Labels
        } else if !observation.values.assignees.is_empty() {
            observation.values.assignees.clear();
            MetadataField::Assignees
        } else {
            return Err(invalid());
        };
        if let Some(observed) = observation.fields.iter_mut().find(|f| f.field == field) {
            observed.state = DetailValueState::Oversized;
        }
    }
    Ok(())
}

impl GithubProvider {
    pub(super) async fn request_resource_details(
        &self,
        token: &SecretToken,
        request: DetailRequest,
        source: &str,
        normalize: fn(&DetailRequest, &[u8], &str) -> Result<Normalized, ProviderError>,
    ) -> Result<DetailPage, ProviderError> {
        if request.facet != DetailFacet::Body
            || request.cursor.is_some()
            || request.account.provider != ProviderKind::Github
            || request.account.host != "github.com"
        {
            return Err(ProviderError::new(ProviderErrorKind::Unsupported));
        }
        if request.repository.account_id != request.account.id
            || request.subject.account_id != request.account.id
            || request.subject.repository_id.as_ref() != Some(&request.repository.id)
            || !valid_repository_path(&request.repository.full_name)
        {
            return Err(invalid());
        }
        let number = request
            .subject
            .number
            .as_ref()
            .and_then(|n| n.parse::<u64>().ok())
            .filter(|n| *n > 0)
            .ok_or_else(invalid)?;
        let native = request
            .repository
            .provider_id
            .parse::<u64>()
            .ok()
            .filter(|n| *n > 0)
            .ok_or_else(invalid)?;
        let resource = if request.subject.kind == RemoteItemKind::PullRequest {
            "pulls"
        } else if request.subject.kind == RemoteItemKind::Issue {
            "issues"
        } else {
            return Err(ProviderError::new(ProviderErrorKind::Unsupported));
        };
        let endpoint = self.http.endpoint(&format!(
            "repos/{}/{resource}/{number}",
            request.repository.full_name
        ))?;
        let paths = vec![
            endpoint.path().into(),
            format!("/repositories/{native}/{resource}/{number}"),
        ];
        let comparable = request.source.as_ref().is_some_and(|s| {
            s.source == source
                && s.adapter_version == ADAPTER_VERSION
                && s.field_mask == vec![DetailField::Body]
        });
        let etag = if comparable {
            request.etag.clone()
        } else {
            None
        };
        let response = self
            .http
            .get_with_paths(
                endpoint,
                token,
                &HttpValidators {
                    etag: etag.clone(),
                    last_modified: None,
                },
                &paths,
            )
            .await?;
        if response.next_url.is_some() || response.not_modified && etag.is_none() {
            return Err(invalid());
        }
        let (body, metadata) = if response.not_modified {
            (DetailValue::default(), None)
        } else {
            let (body, mut metadata) = normalize(&request, &response.body, source)?;
            bound_metadata(&mut metadata)?;
            (body, Some(metadata))
        };
        let provider_updated_at = metadata
            .as_ref()
            .and_then(|m| m.source.provider_updated_at.clone());
        Ok(DetailPage {
            reconciliation: Default::default(),
            body,
            metadata,
            entries: vec![],
            source: DetailSource {
                source: source.into(),
                adapter_version: ADAPTER_VERSION,
                field_mask: vec![DetailField::Body],
                provider_updated_at,
                observed_at: chrono::Utc::now().to_rfc3339(),
            },
            next_cursor: None,
            etag: response.validators.etag,
            not_modified: response.not_modified,
            freshness_seconds: 180,
            cooldown_seconds: response.cooldown_seconds,
        })
    }
}
