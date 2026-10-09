//! Singleton GitLab resource authority; summaries never validate these fields.
use super::{transport::ItemRoute, *};
use crate::resource_metadata::*;
use serde_json::{Map, Value};

pub(super) const ADAPTER_VERSION: u32 = 1;
pub(super) const PULL_SOURCE: &str = "gitlab/merge-request-detail/v4";
pub(super) const ISSUE_SOURCE: &str = "gitlab/issue-detail/v4";
const MAX_BODY: usize = 1_048_576;

pub(super) fn id(value: &Value) -> Result<u64, ProviderError> {
    value.as_u64().filter(|id| *id > 0).ok_or_else(invalid)
}
pub(super) fn time(value: &Value) -> Result<String, ProviderError> {
    let value = value
        .as_str()
        .filter(|v| v.len() <= 128)
        .ok_or_else(invalid)?;
    chrono::DateTime::parse_from_rfc3339(value).map_err(|_| invalid())?;
    Ok(value.into())
}
pub(super) fn state(value: &Value) -> Result<String, ProviderError> {
    let value = value
        .as_str()
        .filter(|v| !v.is_empty() && v.len() <= 128)
        .ok_or_else(invalid)?;
    Ok(if value == "opened" { "open" } else { value }.into())
}
pub(super) fn description(json: &Map<String, Value>) -> Result<DetailValue, ProviderError> {
    Ok(match json.get("description") {
        None => DetailValue {
            state: DetailValueState::Omitted,
            text: None,
        },
        Some(Value::Null) => DetailValue {
            state: DetailValueState::Known,
            text: None,
        },
        Some(Value::String(text)) if text.len() <= MAX_BODY => DetailValue {
            state: DetailValueState::Known,
            text: Some(text.clone()),
        },
        Some(Value::String(_)) => DetailValue {
            state: DetailValueState::Oversized,
            text: None,
        },
        _ => return Err(invalid()),
    })
}
fn web_url(value: &str) -> Result<reqwest::Url, ProviderError> {
    if value.len() > 2048 || value.chars().any(char::is_control) {
        return Err(invalid());
    }
    let url = reqwest::Url::parse(value).map_err(|_| invalid())?;
    if url.scheme() != "https"
        || url.host_str() != Some("gitlab.com")
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
pub(super) fn resource_web(
    value: &Value,
    iid: u64,
    route: ItemRoute,
) -> Result<String, ProviderError> {
    let value = value.as_str().ok_or_else(invalid)?;
    let url = web_url(value)?;
    let segment = if route == ItemRoute::MergeRequests {
        "merge_requests"
    } else {
        "issues"
    };
    let modern = format!("/-/{segment}/{iid}");
    let legacy = format!("/{segment}/{iid}");
    let repository = url
        .path()
        .strip_suffix(&modern)
        .or_else(|| url.path().strip_suffix(&legacy))
        .ok_or_else(invalid)?;
    // Numeric project_id is the authority. A renamed/transferred project's
    // display URL can change before the repository discovery projection does.
    if !valid_path(repository.strip_prefix('/').ok_or_else(invalid)?) {
        return Err(invalid());
    }
    Ok(value.into())
}
pub(super) fn route(kind: &RemoteItemKind) -> Result<ItemRoute, ProviderError> {
    match kind {
        RemoteItemKind::PullRequest => Ok(ItemRoute::MergeRequests),
        RemoteItemKind::Issue => Ok(ItemRoute::Issues),
        _ => Err(ProviderError::new(ProviderErrorKind::Unsupported)),
    }
}
// Point-read authorization is checked by the Store against current selected
// scope or exact todo provenance. Only feed enumeration requires selection.
pub(super) fn repository_identity(
    account: &RemoteAccount,
    repository: &RemoteRepository,
) -> Result<u64, ProviderError> {
    if account.id.is_empty()
        || repository.id.is_empty()
        || repository.account_id != account.id
        || !valid_path(&repository.full_name)
    {
        return Err(invalid());
    }
    positive_id(&repository.provider_id).ok_or_else(invalid)
}
pub(super) fn identity(
    json: &Map<String, Value>,
    project: u64,
    route: ItemRoute,
) -> Result<(u64, u64), ProviderError> {
    let native = id(json.get("id").ok_or_else(invalid)?)?;
    let iid = id(json.get("iid").ok_or_else(invalid)?)?;
    if id(json.get("project_id").ok_or_else(invalid)?)? != project {
        return Err(invalid());
    }
    if route == ItemRoute::Issues
        && json
            .get("issue_type")
            .is_some_and(|v| v.as_str() != Some("issue"))
    {
        return Err(invalid());
    }
    if route == ItemRoute::MergeRequests
        && json
            .get("target_project_id")
            .is_some_and(|v| id(v).ok() != Some(project))
    {
        return Err(invalid());
    }
    if let Some(value) = json.get("web_url") {
        resource_web(value, iid, route)?;
    }
    Ok((native, iid))
}

#[derive(Debug)]
enum FieldError {
    Invalid,
    Oversized,
}
type FieldResult<T> = Result<T, FieldError>;
fn string(value: &Value, max: usize) -> FieldResult<String> {
    let value = value.as_str().ok_or(FieldError::Invalid)?;
    if value.len() > max {
        Err(FieldError::Oversized)
    } else {
        Ok(value.into())
    }
}
fn native(value: &Value) -> FieldResult<String> {
    id(value)
        .map(|v| v.to_string())
        .map_err(|_| FieldError::Invalid)
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
fn repository_provider_id(
    json: &Map<String, Value>,
    key: &str,
) -> Result<Option<String>, ProviderError> {
    match json.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(value) => native(value).map(Some).map_err(|_| invalid()),
    }
}
fn web(value: &Value) -> FieldResult<String> {
    let value = string(value, 2048)?;
    web_url(&value).map_err(|_| FieldError::Invalid)?;
    Ok(value)
}
pub(super) fn actor_login(value: &Value) -> Result<String, ProviderError> {
    actor(value).map(|v| v.login).map_err(|_| invalid())
}
fn actor(value: &Value) -> FieldResult<DetailActor> {
    let value = value.as_object().ok_or(FieldError::Invalid)?;
    Ok(DetailActor {
        provider_id: native(value.get("id").ok_or(FieldError::Invalid)?)?,
        login: string(value.get("username").ok_or(FieldError::Invalid)?, 255)?,
        web_url: optional(value, "web_url", web)?,
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
        number: optional(value, "iid", native)?,
        title: string(value.get("title").ok_or(FieldError::Invalid)?, 16384)?,
        state: optional(value, "state", |v| string(v, 128))?,
        web_url: optional(value, "web_url", web)?,
    })
}
fn observe<T>(
    json: &Map<String, Value>,
    key: &str,
    field: MetadataField,
    parse: impl FnOnce(&Value) -> FieldResult<T>,
) -> Result<(MetadataObservedField, Option<T>), ProviderError> {
    let (state, value) = match json.get(key) {
        None => (DetailValueState::Omitted, None),
        Some(Value::Null) => (DetailValueState::Known, None),
        Some(value) => match parse(value) {
            Ok(value) => (DetailValueState::Known, Some(value)),
            Err(FieldError::Oversized) => (DetailValueState::Oversized, None),
            Err(FieldError::Invalid) => return Err(invalid()),
        },
    };
    Ok((MetadataObservedField { field, state }, value))
}
pub(super) fn oid(value: &Value) -> Result<Option<String>, ProviderError> {
    match value {
        Value::Null => Ok(None),
        Value::String(value) if value.is_empty() => Ok(None),
        Value::String(value)
            if matches!(value.len(), 40 | 64) && value.bytes().all(|b| b.is_ascii_hexdigit()) =>
        {
            Ok(Some(value.clone()))
        }
        _ => Err(invalid()),
    }
}
fn branch(
    json: &Map<String, Value>,
    name: &str,
    oid_value: Option<&Value>,
    field: MetadataField,
    repository: Option<DetailRepositoryRef>,
) -> Result<(MetadataObservedField, Option<DetailBranch>), ProviderError> {
    let name = json
        .get(name)
        .map(|v| {
            if v.is_null() {
                Ok(None)
            } else {
                string(v, 1024).map(Some).map_err(|_| invalid())
            }
        })
        .transpose()?
        .flatten();
    let oid = oid_value.map(oid).transpose()?.flatten();
    let value = name
        .zip(oid)
        .filter(|(name, _)| !name.is_empty())
        .map(|(name, oid)| DetailBranch {
            name,
            oid,
            repository,
        });
    // Empty asynchronous SHAs/refs are unknown, not an authoritative deletion.
    Ok((
        MetadataObservedField {
            field,
            state: if value.is_some() {
                DetailValueState::Known
            } else {
                DetailValueState::Omitted
            },
        },
        value,
    ))
}

fn normalize(
    request: &DetailRequest,
    body: &[u8],
    project: u64,
    item_route: ItemRoute,
    source: &str,
) -> Result<(DetailValue, ResourceMetadataObservation), ProviderError> {
    let json: Map<String, Value> = serde_json::from_slice(body).map_err(|_| invalid())?;
    let (native, iid) = identity(&json, project, item_route)?;
    if positive_id(&request.subject.provider_id) != Some(native)
        || request.subject.number.as_deref().and_then(positive_id) != Some(iid)
    {
        return Err(invalid());
    }
    let body = description(&json)?;
    let mut values = ResourceMetadataValues::default();
    let mut fields = vec![];
    macro_rules! scalar {
        ($key:literal, $field:ident, $target:ident, $parser:expr) => {{
            let (observed, value) = observe(&json, $key, MetadataField::$field, $parser)?;
            fields.push(observed);
            values.$target = value;
        }};
    }
    scalar!("title", Title, title, |v| string(v, 16384));
    scalar!("state", State, state, |v| state(v)
        .map_err(|_| FieldError::Invalid));
    scalar!("state_reason", StateReason, state_reason, |v| string(
        v, 128
    ));
    scalar!("author", Author, author, actor);
    scalar!("web_url", WebUrl, web_url, |v| resource_web(
        v, iid, item_route
    )
    .map_err(|_| FieldError::Invalid));
    scalar!("updated_at", UpdatedAt, updated_at, |v| time(v)
        .map_err(|_| FieldError::Invalid));
    scalar!("milestone", Milestone, milestone, milestone);
    for key in ["labels", "assignees"] {
        if json.get(key).is_some_and(Value::is_null) {
            return Err(invalid());
        }
    }
    let (observed, labels) = observe(&json, "labels", MetadataField::Labels, labels)?;
    fields.push(observed);
    values.labels = labels.unwrap_or_default();
    let (observed, assignees) = observe(&json, "assignees", MetadataField::Assignees, assignees)?;
    fields.push(observed);
    values.assignees = assignees.unwrap_or_default();
    if item_route == ItemRoute::MergeRequests {
        scalar!("draft", IsDraft, is_draft, |v| v
            .as_bool()
            .ok_or(FieldError::Invalid));
        scalar!("merged_at", MergedAt, merged_at, |v| time(v)
            .map_err(|_| FieldError::Invalid));
        let target_repository =
            repository_provider_id(&json, "target_project_id")?.map(|provider_id| {
                DetailRepositoryRef {
                    provider_id,
                    full_name: request.repository.full_name.clone(),
                    web_url: None,
                }
            });
        let source_repository =
            repository_provider_id(&json, "source_project_id")?.map(|provider_id| {
                let full_name = if provider_id == request.repository.provider_id {
                    request.repository.full_name.clone()
                } else {
                    provider_id.clone()
                };
                DetailRepositoryRef {
                    provider_id,
                    full_name,
                    web_url: None,
                }
            });
        let (observed, head) = branch(
            &json,
            "source_branch",
            json.get("sha"),
            MetadataField::Head,
            source_repository,
        )?;
        fields.push(observed);
        values.head = head;
        let diff = match json.get("diff_refs") {
            None | Some(Value::Null) => None,
            Some(Value::Object(value)) => Some(value),
            _ => return Err(invalid()),
        };
        // Async diff references validate the target commit only when their
        // actual known head matches this singleton's actual known MR SHA.
        // Missing heads (including None == None) cannot establish that link.
        let returned_head = json.get("sha").map(oid).transpose()?.flatten();
        let diff_head = diff
            .and_then(|diff| diff.get("head_sha"))
            .map(oid)
            .transpose()?
            .flatten();
        let matching_diff = returned_head
            .zip(diff_head)
            .is_some_and(|(returned, diff)| returned == diff);
        let (observed, base) = branch(
            &json,
            "target_branch",
            diff.filter(|_| matching_diff)
                .and_then(|d| d.get("start_sha")),
            MetadataField::Base,
            target_repository,
        )?;
        fields.push(observed);
        values.base = base;
        let merge_base = diff
            .filter(|_| matching_diff)
            .and_then(|refs| refs.get("base_sha"))
            .map(oid)
            .transpose()?
            .flatten();
        fields.push(MetadataObservedField {
            field: MetadataField::MergeBase,
            state: if merge_base.is_some() {
                DetailValueState::Known
            } else {
                DetailValueState::Omitted
            },
        });
        values.merge_base_oid = merge_base;
    }
    let mut metadata = ResourceMetadataObservation {
        kind: request.subject.kind.clone(),
        values,
        fields,
        source: MetadataSource {
            source: source.into(),
            adapter_version: ADAPTER_VERSION,
            // Only this singleton representation's timestamp is comparable.
            // It cannot validate independent children or a list projection.
            provider_updated_at: None,
            observed_at: chrono::Utc::now().to_rfc3339(),
        },
    };
    metadata.source.provider_updated_at = metadata.values.updated_at.clone();
    while serde_json::to_vec(&metadata.values)
        .map_err(|_| invalid())?
        .len()
        > 262144
    {
        let labels = serde_json::to_vec(&metadata.values.labels)
            .map_err(|_| invalid())?
            .len();
        let assignees = serde_json::to_vec(&metadata.values.assignees)
            .map_err(|_| invalid())?
            .len();
        let field = if labels >= assignees && !metadata.values.labels.is_empty() {
            metadata.values.labels.clear();
            MetadataField::Labels
        } else if !metadata.values.assignees.is_empty() {
            metadata.values.assignees.clear();
            MetadataField::Assignees
        } else {
            return Err(invalid());
        };
        metadata
            .fields
            .iter_mut()
            .find(|observed| observed.field == field)
            .ok_or_else(invalid)?
            .state = DetailValueState::Oversized;
    }
    Ok((body, metadata))
}

impl GitlabProvider {
    pub(super) async fn resource_details(
        &self,
        token: &SecretToken,
        request: DetailRequest,
    ) -> Result<DetailPage, ProviderError> {
        if request.account.provider != ProviderKind::Gitlab
            || request.account.host != "gitlab.com"
            || request.facet != DetailFacet::Body
        {
            return Err(ProviderError::new(ProviderErrorKind::Unsupported));
        }
        if request.account.state != AccountState::Active {
            return Err(ProviderError::new(ProviderErrorKind::Authentication));
        }
        if request.cursor.is_some()
            || request.subject.account_id != request.account.id
            || request.subject.repository_id.as_ref() != Some(&request.repository.id)
        {
            return Err(invalid());
        }
        let project = repository_identity(&request.account, &request.repository)?;
        let item_route = route(&request.subject.kind)?;
        let iid = request
            .subject
            .number
            .as_deref()
            .and_then(positive_id)
            .ok_or_else(invalid)?;
        positive_id(&request.subject.provider_id).ok_or_else(invalid)?;
        let response = self
            .http
            .get(self.http.resource_detail(project, iid, item_route)?, token)
            .await?;
        let source = if item_route == ItemRoute::MergeRequests {
            PULL_SOURCE
        } else {
            ISSUE_SOURCE
        };
        let (body, metadata) = normalize(&request, &response.body, project, item_route, source)
            .map_err(|e| with_quota(e, response.cooldown))?;
        Ok(DetailPage {
            reconciliation: DetailReconciliation::default(),
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
            etag: None,
            not_modified: false,
            freshness_seconds: 180,
            cooldown_seconds: response.cooldown,
        })
    }
}
