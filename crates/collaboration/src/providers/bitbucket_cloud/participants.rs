//! Independent singleton participant observations; no Body or parent clock authority.
use super::*;
use crate::{NativeDetailPayload, ParticipantUser, ParticipantV1};
use serde_json::{Map, Value};
use std::collections::HashSet;

pub(super) const SOURCE: &str = "bitbucket.participants.v1";
const FIELDS: [DetailField; 6] = [
    DetailField::ParticipantLogin,
    DetailField::ParticipantDisplayName,
    DetailField::ParticipantRole,
    DetailField::ParticipantApproved,
    DetailField::ParticipantState,
    DetailField::ParticipantParticipatedAt,
];

// Native UUIDs alone bind this facet. Optional repository presentation is not
// participant authority and must never affect this route or its admission.
fn identity(json: &Map<String, Value>, repository: &str) -> Result<u64, ProviderError> {
    if json.get("type").and_then(Value::as_str) != Some("pullrequest") {
        return Err(invalid());
    }
    let id = resource_details::native_id(json.get("id").ok_or_else(invalid)?)?;
    let destination = json
        .get("destination")
        .and_then(Value::as_object)
        .ok_or_else(invalid)?;
    let observed = destination
        .get("repository")
        .and_then(Value::as_object)
        .ok_or_else(invalid)?;
    if observed.get("type").and_then(Value::as_str) != Some("repository")
        || canonical_uuid(
            observed
                .get("uuid")
                .and_then(Value::as_str)
                .ok_or_else(invalid)?,
        )? != repository
    {
        return Err(invalid());
    }
    Ok(id)
}

fn observed_text(
    object: &Map<String, Value>,
    key: &str,
    field: DetailField,
    maximum: usize,
    nullable: bool,
    empty: bool,
    mask: &mut Vec<DetailField>,
) -> Result<Option<String>, ProviderError> {
    match object.get(key) {
        None => Ok(None),
        Some(Value::Null) if nullable => {
            mask.push(field);
            Ok(None)
        }
        Some(Value::String(value)) => {
            let value = text(value.clone(), maximum, empty)?;
            mask.push(field);
            Ok(Some(value))
        }
        _ => Err(invalid()),
    }
}

fn participant(value: &Value, repository: &str, id: u64) -> Result<DetailEntry, ProviderError> {
    let value = value.as_object().ok_or_else(invalid)?;
    if value.get("type").and_then(Value::as_str) != Some("participant") {
        return Err(invalid());
    }
    let user = value
        .get("user")
        .and_then(Value::as_object)
        .ok_or_else(invalid)?;
    if user.get("type").and_then(Value::as_str) != Some("user") {
        return Err(invalid());
    }
    let actor = canonical_uuid(
        user.get("uuid")
            .and_then(Value::as_str)
            .ok_or_else(invalid)?,
    )?;
    let mut mask = Vec::with_capacity(6);
    let login = observed_text(
        user,
        "nickname",
        DetailField::ParticipantLogin,
        255,
        true,
        true,
        &mut mask,
    )?;
    let display_name = observed_text(
        user,
        "display_name",
        DetailField::ParticipantDisplayName,
        1024,
        true,
        true,
        &mut mask,
    )?;
    let role = observed_text(
        value,
        "role",
        DetailField::ParticipantRole,
        128,
        false,
        false,
        &mut mask,
    )?;
    let approved = match value.get("approved") {
        None => None,
        Some(Value::Bool(value)) => {
            mask.push(DetailField::ParticipantApproved);
            Some(*value)
        }
        _ => return Err(invalid()),
    };
    let state = observed_text(
        value,
        "state",
        DetailField::ParticipantState,
        128,
        true,
        false,
        &mut mask,
    )?;
    let participated_at = observed_text(
        value,
        "participated_on",
        DetailField::ParticipantParticipatedAt,
        128,
        true,
        false,
        &mut mask,
    )?;
    if participated_at
        .as_ref()
        .is_some_and(|value| chrono::DateTime::parse_from_rfc3339(value).is_err())
    {
        return Err(invalid());
    }
    Ok(DetailEntry {
        id: format!("bitbucket_cloud:participant:{repository}:{id}:{actor}"),
        provider_id: format!("{repository}:{id}:{actor}"),
        native: Some(NativeDetailPayload::ParticipantV1(ParticipantV1 {
            user: ParticipantUser {
                provider_id: actor,
                login,
                display_name,
            },
            role,
            approved,
            state,
            participated_at,
        })),
        author: None,
        title: None,
        state: None,
        body: DetailValue::default(),
        observed_body_state: DetailValueState::NotLoaded,
        updated_at: None,
        head_oid: None,
        field_mask: mask,
        field_validations: vec![],
    })
}

impl BitbucketCloudProvider {
    pub(super) async fn participants(
        &self,
        token: &SecretToken,
        request: DetailRequest,
    ) -> Result<DetailPage, ProviderError> {
        if request.facet != DetailFacet::Participants
            || request.subject.kind != RemoteItemKind::PullRequest
        {
            return Err(ProviderError::new(ProviderErrorKind::Unsupported));
        }
        if request.etag.is_some() {
            return Err(invalid());
        }
        let (repository, id) = resource_details::subject_identity(&request)?;
        let route = Route::PullRequest(repository.clone(), id);
        let response = self
            .http
            .get(self.http.endpoint(&route)?, &route, token)
            .await?;
        let result = (|| {
            let json: Value = serde_json::from_slice(&response.body).map_err(|_| invalid())?;
            let object = json.as_object().ok_or_else(invalid)?;
            if identity(object, &repository)? != id {
                return Err(invalid());
            }
            let values = object
                .get("participants")
                .and_then(Value::as_array)
                .filter(|values| values.len() <= 100)
                .ok_or_else(invalid)?;
            let mut identities = HashSet::with_capacity(values.len());
            let mut entries = Vec::with_capacity(values.len());
            for value in values {
                let entry = participant(value, &repository, id)?;
                if !identities.insert(entry.provider_id.clone()) {
                    return Err(invalid());
                }
                entries.push(entry);
            }
            Ok(DetailPage {
                reconciliation: DetailReconciliation::full_history(),
                body: DetailValue::default(),
                metadata: None,
                entries,
                source: DetailSource {
                    source: SOURCE.into(),
                    adapter_version: 1,
                    field_mask: FIELDS.into(),
                    provider_updated_at: None,
                    observed_at: chrono::Utc::now().to_rfc3339(),
                },
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
