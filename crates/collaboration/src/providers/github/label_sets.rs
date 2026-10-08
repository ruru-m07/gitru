//! Best-effort additive/removal label intent with exact post-write observation.
use super::*;
use crate::delivery::*;
use crate::label_sets::native::*;
use crate::storage::label_sets as store;
use crate::{
    CollaborationError, DetailField, DetailSubjectBinding, MetadataField, MetadataObservedField,
    MetadataSource, ResourceMetadataObservation, ResourceMetadataValues,
};
use sqlx::{Sqlite, Transaction};
use std::sync::Arc;

mod recovery;

pub(crate) struct GithubLabelSetPolicy {
    http: GithubHttp,
}

impl GithubLabelSetPolicy {
    pub(crate) fn new() -> Result<Self, CollaborationError> {
        Ok(Self {
            http: GithubHttp::new()?,
        })
    }

    #[cfg(test)]
    pub(crate) fn for_test_base(base: reqwest::Url) -> Self {
        Self {
            http: GithubHttp::for_test_base(base).expect("fixture transport"),
        }
    }

    fn request(account: &RemoteAccount, frame: &NativeFrame) -> DetailRequest {
        DetailRequest {
            account: account.clone(),
            repository: frame.repository.clone(),
            subject: frame.subject.clone(),
            facet: DetailFacet::Body,
            cursor: None,
            etag: None,
            source: None,
        }
    }

    fn resource_route(frame: &NativeFrame) -> Result<String, ProviderError> {
        validate_frame_route(frame)?;
        let resource = match frame.subject.kind {
            RemoteItemKind::Issue => "issues",
            RemoteItemKind::PullRequest => "pulls",
            _ => return Err(resource_details::invalid()),
        };
        Ok(format!(
            "repositories/{}/{resource}/{}",
            frame.repository.provider_id,
            frame.subject.number.as_deref().unwrap_or("")
        ))
    }

    fn labels_route(frame: &NativeFrame) -> Result<String, ProviderError> {
        validate_frame_route(frame)?;
        Ok(format!(
            "repositories/{}/issues/{}/labels",
            frame.repository.provider_id,
            frame.subject.number.as_deref().unwrap_or("")
        ))
    }

    fn repository_labels_route(frame: &NativeFrame) -> Result<String, ProviderError> {
        validate_frame_route(frame)?;
        Ok(format!(
            "repositories/{}/labels",
            frame.repository.provider_id
        ))
    }

    fn normalize(
        account: &RemoteAccount,
        frame: &NativeFrame,
        bytes: &[u8],
    ) -> Result<Observation, ProviderError> {
        let request = Self::request(account, frame);
        let (body, metadata) = match frame.subject.kind {
            RemoteItemKind::Issue => issue_details::normalize(&request, bytes, &frame.base.source)?,
            RemoteItemKind::PullRequest => {
                pull_details::normalize(&request, bytes, &frame.base.source)?
            }
            _ => return Err(resource_details::invalid()),
        };
        let known = |field| {
            metadata.fields.iter().any(|evidence| {
                evidence.field == field && evidence.state == DetailValueState::Known
            })
        };
        if !known(MetadataField::Labels)
            || !known(MetadataField::UpdatedAt)
            || !known(MetadataField::State)
            || frame.subject.kind == RemoteItemKind::PullRequest && !known(MetadataField::Head)
        {
            return Err(resource_details::invalid());
        }
        let labels = metadata
            .values
            .labels
            .iter()
            .map(|label| {
                Ok(crate::LabelIdentity {
                    provider_id: label
                        .provider_id
                        .clone()
                        .ok_or_else(resource_details::invalid)?,
                    name: label.name.clone(),
                    color: label.color.clone(),
                })
            })
            .collect::<Result<Vec<_>, ProviderError>>()?;
        let labels = sorted(labels);
        if !labels_valid(&labels, MAX_LABELS) {
            return Err(resource_details::invalid());
        }
        let body = if body.text.as_ref().is_some_and(|text| text.len() > MAX_BODY) {
            DetailValue {
                state: DetailValueState::Oversized,
                text: None,
            }
        } else {
            body
        };
        Ok(Observation {
            title: known(MetadataField::Title)
                .then_some(metadata.values.title)
                .flatten(),
            body,
            state: metadata
                .values
                .state
                .ok_or_else(resource_details::invalid)?,
            labels,
            head: metadata.values.head,
            provider_updated_at: metadata
                .source
                .provider_updated_at
                .ok_or_else(resource_details::invalid)?,
            observed_at: metadata.source.observed_at,
        })
    }

    async fn validate_add(
        &self,
        token: &SecretToken,
        frame: &NativeFrame,
        label: &crate::LabelIdentity,
    ) -> Result<(AddValidation, Option<u64>), ProviderError> {
        let url = append_label_name(
            self.http.endpoint(&Self::repository_labels_route(frame)?)?,
            &label.name,
        )?;
        match self.http.get_point(url, token).await {
            Ok(response) => {
                if response.not_modified || response.next_url.is_some() {
                    return Err(with_cooldown(
                        resource_details::invalid(),
                        response.cooldown_seconds,
                    ));
                }
                let observed = label_object(&response.body)
                    .map_err(|error| with_cooldown(error, response.cooldown_seconds))?;
                Ok((
                    AddValidation {
                        label: label.clone(),
                        valid: observed == *label,
                    },
                    response.cooldown_seconds,
                ))
            }
            Err(error) if error.kind == ProviderErrorKind::NotFound => {
                let cooldown = error.account_cooldown_seconds;
                Ok((
                    AddValidation {
                        label: label.clone(),
                        valid: false,
                    },
                    cooldown,
                ))
            }
            Err(error) => Err(error),
        }
    }

    async fn read(
        &self,
        token: &SecretToken,
        request: &ReconcileRequest,
        origin: Origin,
    ) -> Result<(Evidence, Option<u64>), ProviderError> {
        let frame: NativeFrame =
            decode_bounded(&request.native_context).map_err(|_| resource_details::invalid())?;
        let payload = decode_payload(&request.command).map_err(|_| resource_details::invalid())?;
        let route = Self::resource_route(&frame)?;
        let response = self
            .http
            .get_point(self.http.endpoint(&route)?, token)
            .await?;
        if response.not_modified || response.next_url.is_some() {
            let mut error = resource_details::invalid();
            error.account_cooldown_seconds = response.cooldown_seconds;
            return Err(error);
        }
        let observation =
            Self::normalize(&request.account, &frame, &response.body).map_err(|mut error| {
                error.account_cooldown_seconds = response.cooldown_seconds;
                error
            })?;
        if origin == Origin::Preflight
            && response.cooldown_seconds.is_some()
            && !desired_matches(&payload, &observation)
        {
            return Err(rate_limited(response.cooldown_seconds.unwrap_or(60)));
        }
        let fresh = chrono::DateTime::parse_from_rfc3339(&observation.provider_updated_at).ok();
        let base = chrono::DateTime::parse_from_rfc3339(&frame.base.updated_at).ok();
        if fresh.zip(base).is_none_or(|(fresh, base)| fresh < base) {
            let mut error = resource_details::invalid();
            error.account_cooldown_seconds = response.cooldown_seconds;
            return Err(error);
        }
        let mut cooldown = response.cooldown_seconds;
        let mut add_validations = Vec::new();
        if origin == Origin::Preflight {
            for label in &payload.request.add_labels {
                if exact_member(&observation.labels, label)
                    || identity_collision(&observation.labels, label)
                {
                    continue;
                }
                let (validation, observed_cooldown) =
                    self.validate_add(token, &frame, label).await?;
                if let Some(wait) = observed_cooldown.filter(|wait| *wait > 0) {
                    return Err(rate_limited(wait));
                }
                cooldown = cooldown.into_iter().chain(observed_cooldown).max();
                add_validations.push(validation);
            }
        }
        let evidence = Evidence {
            frame,
            observation,
            add_validations,
            account_id: request.account.id.clone(),
            actor_id: request.account.actor_id.clone(),
            authorization_epoch: request.account.authorization_epoch.clone(),
            command_hash: hash(&request.command),
            origin,
        };
        if !valid_add_validations(&payload, &evidence) {
            return Err(with_cooldown(resource_details::invalid(), cooldown));
        }
        encode_bounded(&evidence)
            .map_err(|_| with_cooldown(resource_details::invalid(), cooldown))?;
        Ok((evidence, cooldown))
    }

    fn proof(evidence: &Evidence) -> Result<OperationEvidence, CollaborationError> {
        Ok(OperationEvidence {
            kind: "github.label_observation".into(),
            version: 1,
            payload: encode_bounded(evidence)?,
        })
    }
}

fn validate_frame_route(frame: &NativeFrame) -> Result<(), ProviderError> {
    if frame
        .repository
        .provider_id
        .parse::<u64>()
        .ok()
        .is_none_or(|id| id == 0 || id.to_string() != frame.repository.provider_id)
        || !valid_repository_path(&frame.repository.full_name)
        || frame.subject.number.as_ref().is_none_or(|number| {
            number
                .parse::<u64>()
                .ok()
                .is_none_or(|value| value == 0 || value.to_string() != *number)
        })
    {
        return Err(resource_details::invalid());
    }
    Ok(())
}

fn label_object(bytes: &[u8]) -> Result<crate::LabelIdentity, ProviderError> {
    let value: serde_json::Value =
        serde_json::from_slice(bytes).map_err(|_| resource_details::invalid())?;
    let object = value.as_object().ok_or_else(resource_details::invalid)?;
    let id = object
        .get("id")
        .and_then(serde_json::Value::as_u64)
        .filter(|id| *id > 0)
        .ok_or_else(resource_details::invalid)?;
    let name = object
        .get("name")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(resource_details::invalid)?
        .to_string();
    let color = object
        .get("color")
        .and_then(serde_json::Value::as_str)
        .map(ToOwned::to_owned);
    let label = crate::LabelIdentity {
        provider_id: id.to_string(),
        name,
        color,
    };
    if !label_valid(&label) {
        return Err(resource_details::invalid());
    }
    Ok(label)
}

fn append_label_name(mut url: reqwest::Url, name: &str) -> Result<reqwest::Url, ProviderError> {
    if matches!(name, "." | "..") {
        return Err(resource_details::invalid());
    }
    url.path_segments_mut()
        .map_err(|_| resource_details::invalid())?
        .push(name);
    Ok(url)
}

fn rate_limited(wait: u64) -> ProviderError {
    ProviderError {
        kind: ProviderErrorKind::RateLimited,
        retry_after_seconds: Some(wait),
        account_cooldown_seconds: Some(wait),
    }
}

fn with_cooldown(mut error: ProviderError, cooldown: Option<u64>) -> ProviderError {
    error.account_cooldown_seconds = error
        .account_cooldown_seconds
        .into_iter()
        .chain(cooldown)
        .max();
    error
}

fn hash(command: &DeliveryCommand) -> String {
    use std::fmt::Write;
    command
        .hash
        .iter()
        .fold(String::with_capacity(64), |mut result, byte| {
            let _ = write!(result, "{byte:02x}");
            result
        })
}

impl ProviderRegistry {
    pub fn register_github_label_sets(&mut self) -> Result<(), CollaborationError> {
        let policy = Arc::new(GithubLabelSetPolicy::new()?);
        let instance = ProviderInstance::public(ProviderKind::Github);
        self.register_delivery(&instance, policy.clone())?;
        self.register_recovery(&instance, policy)
    }
}

#[async_trait]
impl CommandDeliveryPolicy for GithubLabelSetPolicy {
    fn operation_kind(&self) -> &'static str {
        OPERATION
    }

    fn payload_version(&self) -> u32 {
        1
    }

    async fn prepare_context_in(
        &self,
        tx: &mut Transaction<'_, Sqlite>,
        command: &DeliveryCommand,
        account: &RemoteAccount,
    ) -> Result<Vec<u8>, CollaborationError> {
        store::prepare_in(tx, command, account).await
    }

    async fn prepare(
        &self,
        token: &SecretToken,
        request: &ReconcileRequest,
    ) -> Result<DeliveryPreparation, ProviderError> {
        let (evidence, cooldown) = self.read(token, request, Origin::Preflight).await?;
        Ok(DeliveryPreparation {
            bytes: encode_bounded(&evidence)
                .map_err(|_| with_cooldown(resource_details::invalid(), cooldown))?,
            account_cooldown_seconds: cooldown,
        })
    }

    async fn validate_claim(
        &self,
        tx: &mut Transaction<'_, Sqlite>,
        command: &DeliveryCommand,
        account: &RemoteAccount,
        preparation: &[u8],
    ) -> Result<ClaimDecision, CollaborationError> {
        let evidence: Evidence = decode_bounded(preparation)?;
        let payload = decode_payload(command)?;
        if evidence.origin != Origin::Preflight
            || !matches_command(&evidence, command)
            || evidence.actor_id != account.actor_id
            || evidence.authorization_epoch != account.authorization_epoch
            || !valid_add_validations(&payload, &evidence)
        {
            return Err(invalid());
        }
        store::validate_frame_in(tx, account, &evidence.frame).await?;
        if identity_conflict(&payload, &evidence) {
            return Ok(ClaimDecision::Conflict(Self::proof(&evidence)?));
        }
        if desired_matches(&payload, &evidence.observation) {
            return Ok(ClaimDecision::Confirmed(Self::proof(&evidence)?));
        }
        Ok(ClaimDecision::Ready(preparation.to_vec()))
    }

    fn validate_evidence(
        &self,
        command: &DeliveryCommand,
        purpose: EvidencePurpose,
        proof: &OperationEvidence,
    ) -> bool {
        if proof.kind != "github.label_observation" || proof.version != 1 || !proof.bounded() {
            return false;
        }
        let Ok(evidence) = decode_bounded::<Evidence>(&proof.payload) else {
            return false;
        };
        let Ok(payload) = decode_payload(command) else {
            return false;
        };
        if !matches_command(&evidence, command) || !valid_add_validations(&payload, &evidence) {
            return false;
        }
        match purpose {
            EvidencePurpose::Confirmed => {
                desired_matches(&payload, &evidence.observation)
                    && match evidence.origin {
                        Origin::Preflight => true,
                        Origin::MutationReadback => command.attempt_count > 0,
                        Origin::Reconciliation => {
                            command.reconcile_only() || command.attempt_count > 0
                        }
                    }
            }
            EvidencePurpose::Conflict => match evidence.origin {
                Origin::Preflight => identity_conflict(&payload, &evidence),
                Origin::MutationReadback => {
                    command.attempt_count > 0 && !desired_matches(&payload, &evidence.observation)
                }
                Origin::Reconciliation => false,
            },
            _ => false,
        }
    }

    async fn finalize_in(
        &self,
        context: &mut crate::storage::effective::finalization::DeliveryFinalization<'_, '_>,
        command: &DeliveryCommand,
        purpose: EvidencePurpose,
        proof: &OperationEvidence,
    ) -> Result<(), CollaborationError> {
        if !matches!(
            purpose,
            EvidencePurpose::Confirmed | EvidencePurpose::Conflict
        ) {
            return Ok(());
        }
        let evidence: Evidence = decode_bounded(&proof.payload)?;
        store::validate_completion_in(context.transaction(), &evidence).await?;
        context
            .observe_body(canonical_page(&evidence, command)?)
            .await
    }

    async fn dispatch(&self, token: &SecretToken, request: DispatchRequest) -> DeliveryReport {
        let Ok(evidence) = decode_bounded::<Evidence>(&request.execution_base) else {
            return DeliveryReport::unknown();
        };
        let Ok(payload) = decode_payload(&request.command) else {
            return DeliveryReport::unknown();
        };
        if evidence.origin != Origin::Preflight
            || !matches_command(&evidence, &request.command)
            || evidence.actor_id != request.account.actor_id
            || evidence.authorization_epoch != request.account.authorization_epoch
            || identity_conflict(&payload, &evidence)
            || !valid_add_validations(&payload, &evidence)
        {
            return DeliveryReport::unknown();
        }
        let additions: Vec<_> = payload
            .request
            .add_labels
            .iter()
            .filter(|label| !exact_member(&evidence.observation.labels, label))
            .collect();
        let removals: Vec<_> = payload
            .request
            .remove_labels
            .iter()
            .filter(|label| exact_member(&evidence.observation.labels, label))
            .collect();
        if additions.is_empty() && removals.is_empty() {
            return DeliveryReport::unknown();
        }
        let Ok(route) = Self::labels_route(&evidence.frame) else {
            return DeliveryReport::unknown();
        };
        let mut cooldown = None;
        if !additions.is_empty() {
            let Ok(url) = self.http.endpoint(&route) else {
                return DeliveryReport::unknown();
            };
            let body = serde_json::json!({
                "labels": additions.iter().map(|label| label.name.as_str()).collect::<Vec<_>>()
            })
            .to_string()
            .into_bytes();
            let response = match self
                .http
                .mutate_native(token, reqwest::Method::POST, url, body)
                .await
            {
                Ok(response) => response,
                Err(error) => {
                    return DeliveryReport {
                        provider_error: Some(error),
                        ..DeliveryReport::unknown()
                    };
                }
            };
            cooldown = cooldown.into_iter().chain(response.cooldown_seconds).max();
            if response.status != reqwest::StatusCode::OK
                || response.provider_error.is_some()
                || response.cooldown_seconds.is_some()
            {
                return DeliveryReport {
                    account_cooldown_seconds: cooldown,
                    provider_error: response.provider_error,
                    ..DeliveryReport::unknown()
                };
            }
        }
        for label in removals {
            let Ok(url) = self.http.endpoint(&route) else {
                return DeliveryReport::unknown();
            };
            let Ok(url) = append_label_name(url, &label.name) else {
                return DeliveryReport::unknown();
            };
            let response = match self
                .http
                .mutate_native(token, reqwest::Method::DELETE, url, vec![])
                .await
            {
                Ok(response) => response,
                Err(error) => {
                    return DeliveryReport {
                        account_cooldown_seconds: cooldown,
                        provider_error: Some(error),
                        ..DeliveryReport::unknown()
                    };
                }
            };
            cooldown = cooldown.into_iter().chain(response.cooldown_seconds).max();
            if response.status != reqwest::StatusCode::OK
                || response.provider_error.is_some()
                || response.cooldown_seconds.is_some()
            {
                return DeliveryReport {
                    account_cooldown_seconds: cooldown,
                    provider_error: response.provider_error,
                    ..DeliveryReport::unknown()
                };
            }
        }
        let Ok(native_context) = encode_bounded(&evidence.frame) else {
            return DeliveryReport {
                account_cooldown_seconds: cooldown,
                ..DeliveryReport::unknown()
            };
        };
        let read_request = ReconcileRequest {
            native_context,
            command: request.command.clone(),
            account: request.account,
            instance_id: request.instance_id,
        };
        let (fresh, observed_cooldown) = match self
            .read(token, &read_request, Origin::MutationReadback)
            .await
        {
            Ok(result) => result,
            Err(error) => {
                return DeliveryReport {
                    account_cooldown_seconds: cooldown
                        .into_iter()
                        .chain(error.account_cooldown_seconds)
                        .max(),
                    provider_error: Some(error),
                    ..DeliveryReport::unknown()
                };
            }
        };
        cooldown = cooldown.into_iter().chain(observed_cooldown).max();
        let Ok(proof) = Self::proof(&fresh) else {
            return DeliveryReport {
                account_cooldown_seconds: cooldown,
                ..DeliveryReport::unknown()
            };
        };
        let outcome =
            if self.validate_evidence(&read_request.command, EvidencePurpose::Confirmed, &proof) {
                DeliveryOutcome::Confirmed(proof)
            } else if self.validate_evidence(
                &read_request.command,
                EvidencePurpose::Conflict,
                &proof,
            ) {
                DeliveryOutcome::Conflict(proof)
            } else {
                DeliveryOutcome::Unknown
            };
        DeliveryReport {
            outcome,
            account_cooldown_seconds: cooldown,
            ..DeliveryReport::unknown()
        }
    }

    async fn reconcile(
        &self,
        token: &SecretToken,
        request: ReconcileRequest,
    ) -> Result<DeliveryReport, ProviderError> {
        let (evidence, cooldown) = self.read(token, &request, Origin::Reconciliation).await?;
        let proof = Self::proof(&evidence)
            .map_err(|_| with_cooldown(resource_details::invalid(), cooldown))?;
        Ok(DeliveryReport {
            outcome: if self.validate_evidence(&request.command, EvidencePurpose::Confirmed, &proof)
            {
                DeliveryOutcome::Confirmed(proof)
            } else {
                DeliveryOutcome::Unknown
            },
            account_cooldown_seconds: cooldown,
            ..DeliveryReport::unknown()
        })
    }
}

fn valid_add_validations(payload: &Payload, evidence: &Evidence) -> bool {
    if evidence.origin != Origin::Preflight {
        return evidence.add_validations.is_empty();
    }
    let expected: Vec<_> = payload
        .request
        .add_labels
        .iter()
        .filter(|label| {
            !exact_member(&evidence.observation.labels, label)
                && !identity_collision(&evidence.observation.labels, label)
        })
        .collect();
    expected.len() == evidence.add_validations.len()
        && expected
            .into_iter()
            .zip(&evidence.add_validations)
            .all(|(expected, validation)| expected == &validation.label)
}

fn matches_command(evidence: &Evidence, command: &DeliveryCommand) -> bool {
    let Ok(payload) = decode_payload(command) else {
        return false;
    };
    (match evidence.frame.subject.kind {
        RemoteItemKind::PullRequest => command.target_kind == "pull_request",
        RemoteItemKind::Issue => command.target_kind == "issue",
        _ => false,
    }) && evidence.account_id == command.account_id
        && evidence.command_hash == hash(command)
        && evidence.frame.subject.id == command.target_id
        && evidence.frame.subject.provider_id == payload.base.subject_native_id
        && evidence.frame.repository.provider_id == payload.base.repository_native_id
        && evidence.frame.subject.number.as_deref() == Some(payload.base.number.as_str())
        && evidence.frame.repository.id == command.repository_id.as_deref().unwrap_or("")
        && evidence.frame.base.source == payload.base.source
        && evidence.frame.subject.account_id == command.account_id
        && evidence.frame.repository.account_id == command.account_id
        && labels_valid(&evidence.observation.labels, MAX_LABELS)
        && evidence.observation.labels == sorted(evidence.observation.labels.clone())
        && evidence
            .observation
            .body
            .text
            .as_ref()
            .is_none_or(|body| body.len() <= MAX_BODY)
        && evidence
            .observation
            .title
            .as_ref()
            .is_none_or(|title| title.len() <= 4096)
        && chrono::DateTime::parse_from_rfc3339(&evidence.observation.provider_updated_at).is_ok()
        && chrono::DateTime::parse_from_rfc3339(&evidence.observation.observed_at).is_ok()
}

fn canonical_page(
    evidence: &Evidence,
    command: &DeliveryCommand,
) -> Result<DetailCommit, CollaborationError> {
    if !matches_command(evidence, command) {
        return Err(invalid());
    }
    let observation = &evidence.observation;
    let frame = &evidence.frame;
    let source = MetadataSource {
        source: frame.base.source.clone(),
        adapter_version: 1,
        provider_updated_at: Some(observation.provider_updated_at.clone()),
        observed_at: observation.observed_at.clone(),
    };
    let mut fields = vec![
        MetadataField::State,
        MetadataField::UpdatedAt,
        MetadataField::Labels,
    ];
    if observation.title.is_some() {
        fields.push(MetadataField::Title);
    }
    if frame.subject.kind == RemoteItemKind::PullRequest {
        fields.push(MetadataField::Head);
    }
    Ok(DetailCommit {
        account_id: evidence.account_id.clone(),
        authorization_epoch: evidence.authorization_epoch.clone(),
        authorization_view: frame.authorization_view.clone(),
        instance_id: "github:https://github.com/".into(),
        subject_id: command.target_id.clone(),
        facet: DetailFacet::Body,
        run_id: frame.run_id.clone(),
        request_cursor: None,
        body: observation.body.clone(),
        metadata: Some(ResourceMetadataObservation {
            kind: frame.subject.kind.clone(),
            values: ResourceMetadataValues {
                title: observation.title.clone(),
                state: Some(observation.state.clone()),
                updated_at: Some(observation.provider_updated_at.clone()),
                labels: observation
                    .labels
                    .iter()
                    .map(|label| crate::DetailLabel {
                        provider_id: Some(label.provider_id.clone()),
                        name: label.name.clone(),
                        color: label.color.clone(),
                    })
                    .collect(),
                head: observation.head.clone(),
                ..Default::default()
            },
            fields: fields
                .into_iter()
                .map(|field| MetadataObservedField {
                    field,
                    state: DetailValueState::Known,
                })
                .collect(),
            source,
        }),
        subject_binding: Some(DetailSubjectBinding {
            repository_id: frame.repository.id.clone(),
            repository_provider_id: frame.repository.provider_id.clone(),
            provider_id: frame.subject.provider_id.clone(),
            number: frame.subject.number.clone(),
            kind: frame.subject.kind.clone(),
            head_oid: frame.subject.head_oid.clone(),
        }),
        check_context: None,
        review_context: None,
        entries: vec![],
        source: DetailSource {
            source: frame.base.source.clone(),
            adapter_version: 1,
            field_mask: vec![DetailField::Body],
            provider_updated_at: Some(observation.provider_updated_at.clone()),
            observed_at: observation.observed_at.clone(),
        },
        next_cursor: None,
        etag: None,
        not_modified: false,
        whole_scope: true,
        complete: true,
        freshness_seconds: 180,
        reconciliation: Default::default(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn literal_label_names_stay_below_the_numeric_labels_route() {
        for base in [
            "https://api.github.com/repositories/1/labels",
            "https://api.github.com/repositories/1/issues/2/labels",
        ] {
            let base = reqwest::Url::parse(base).unwrap();
            for name in [".", ".."] {
                assert!(append_label_name(base.clone(), name).is_err(), "{name}");
            }
            for (name, encoded) in [
                ("a/b", "a%2Fb"),
                ("a%b", "a%25b"),
                ("a#b", "a%23b"),
                ("a?b", "a%3Fb"),
                ("雪", "%E9%9B%AA"),
            ] {
                let url = append_label_name(base.clone(), name).unwrap();
                assert_eq!(url.as_str(), format!("{base}/{encoded}"), "{name}");
            }
        }
    }
}
