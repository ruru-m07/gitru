//! Explicit best-effort desired-state policy; never generically retries PATCH.
use super::*;
use crate::delivery::*;
use crate::storage::text_edits as store;
use crate::text_edits::native::*;
use crate::{
    CollaborationError, DetailField, DetailSubjectBinding, MetadataField, MetadataObservedField,
    MetadataSource, ResourceMetadataObservation, ResourceMetadataValues,
};
use sqlx::{Sqlite, Transaction};
use std::sync::Arc;
mod recovery;

pub(crate) struct GithubTextEditPolicy {
    pub(super) http: GithubHttp,
}
impl GithubTextEditPolicy {
    pub(crate) fn new() -> Result<Self, CollaborationError> {
        Ok(Self {
            http: GithubHttp::new()?,
        })
    }
    #[cfg(test)]
    pub(crate) fn for_test_base(base: reqwest::Url) -> Self {
        Self {
            http: GithubHttp::for_test_base(base).unwrap(),
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
    fn route(frame: &NativeFrame) -> Result<String, ProviderError> {
        if !valid_repository_path(&frame.repository.full_name)
            || frame
                .subject
                .number
                .as_ref()
                .is_none_or(|n| n.parse::<u64>().ok().is_none_or(|n| n == 0))
        {
            return Err(resource_details::invalid());
        }
        let resource = match frame.subject.kind {
            RemoteItemKind::Issue => "issues",
            RemoteItemKind::PullRequest => "pulls",
            _ => return Err(resource_details::invalid()),
        };
        Ok(format!(
            "repos/{}/{resource}/{}",
            frame.repository.full_name,
            frame.subject.number.as_deref().unwrap_or("")
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
            metadata
                .fields
                .iter()
                .any(|f| f.field == field && f.state == DetailValueState::Known)
        };
        if body.state != DetailValueState::Known
            || body.text.as_ref().is_some_and(|s| s.len() > MAX_BODY)
            || !known(MetadataField::Title)
            || !known(MetadataField::State)
            || !known(MetadataField::UpdatedAt)
            || frame.subject.kind == RemoteItemKind::PullRequest && !known(MetadataField::Head)
        {
            return Err(resource_details::invalid());
        }
        let title = metadata
            .values
            .title
            .ok_or_else(resource_details::invalid)?;
        if title.len() > 1024 || title.chars().count() > 256 {
            return Err(resource_details::invalid());
        }
        Ok(Observation {
            title,
            body: body.text,
            state: metadata
                .values
                .state
                .ok_or_else(resource_details::invalid)?,
            head: metadata.values.head,
            provider_updated_at: metadata
                .source
                .provider_updated_at
                .ok_or_else(resource_details::invalid)?,
            observed_at: metadata.source.observed_at,
        })
    }
    async fn read(
        &self,
        token: &SecretToken,
        request: &ReconcileRequest,
        origin: Origin,
    ) -> Result<(Evidence, Option<u64>), ProviderError> {
        let frame: NativeFrame =
            decode_bounded(&request.native_context).map_err(|_| resource_details::invalid())?;
        let route = Self::route(&frame)?;
        let response = self
            .http
            .get_point(self.http.endpoint(&route)?, token)
            .await?;
        if response.not_modified || response.next_url.is_some() {
            return Err(resource_details::invalid());
        }
        let observation =
            Self::normalize(&request.account, &frame, &response.body).map_err(|mut error| {
                error.account_cooldown_seconds = response.cooldown_seconds;
                error
            })?;
        let fresh = chrono::DateTime::parse_from_rfc3339(&observation.provider_updated_at).ok();
        let base = chrono::DateTime::parse_from_rfc3339(&frame.base.updated_at).ok();
        if fresh.zip(base).is_none_or(|(fresh, base)| fresh < base) {
            let mut error = resource_details::invalid();
            error.account_cooldown_seconds = response.cooldown_seconds;
            return Err(error);
        }
        let evidence = Evidence {
            frame,
            observation,
            account_id: request.account.id.clone(),
            actor_id: request.account.actor_id.clone(),
            authorization_epoch: request.account.authorization_epoch.clone(),
            command_hash: hash(&request.command),
            origin,
        };
        encode_bounded(&evidence).map_err(|_| {
            let mut error = resource_details::invalid();
            error.account_cooldown_seconds = response.cooldown_seconds;
            error
        })?;
        Ok((evidence, response.cooldown_seconds))
    }
    fn proof(evidence: &Evidence) -> Result<OperationEvidence, CollaborationError> {
        Ok(OperationEvidence {
            kind: "github.text_observation".into(),
            version: 1,
            payload: encode_bounded(evidence)?,
        })
    }
}
fn hash(command: &DeliveryCommand) -> String {
    use std::fmt::Write;
    command
        .hash
        .iter()
        .fold(String::with_capacity(64), |mut s, b| {
            let _ = write!(s, "{b:02x}");
            s
        })
}
impl ProviderRegistry {
    /// Explicit native installation registration. No renderer can register codecs.
    pub fn register_github_text_edits(&mut self) -> Result<(), CollaborationError> {
        let policy = Arc::new(GithubTextEditPolicy::new()?);
        let instance = ProviderInstance::public(ProviderKind::Github);
        self.register_delivery(&instance, policy.clone())?;
        self.register_recovery(&instance, policy)
    }
}
#[async_trait]
impl CommandDeliveryPolicy for GithubTextEditPolicy {
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
            bytes: encode_bounded(&evidence).map_err(|_| resource_details::invalid())?,
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
        {
            return Err(invalid());
        }
        store::validate_frame_in(tx, account, &evidence.frame).await?;
        if overlap(&payload, &evidence.observation) {
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
        if proof.kind != "github.text_observation" || proof.version != 1 || !proof.bounded() {
            return false;
        }
        let Ok(evidence) = decode_bounded::<Evidence>(&proof.payload) else {
            return false;
        };
        let Ok(payload) = decode_payload(command) else {
            return false;
        };
        if !matches_command(&evidence, command) {
            return false;
        }
        match purpose {
            EvidencePurpose::Confirmed => {
                desired_matches(&payload, &evidence.observation)
                    && match evidence.origin {
                        Origin::Preflight => guards_match(&payload, &evidence.observation),
                        Origin::MutationResponse => command.attempt_count > 0,
                        Origin::Reconciliation => {
                            command.reconcile_only() || command.attempt_count > 0
                        }
                    }
            }
            EvidencePurpose::Conflict => {
                evidence.origin == Origin::Preflight && overlap(&payload, &evidence.observation)
            }
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
        // Complete provider facts update even on conflict, so R117 presents the
        // fresh conflicting text without replacing the immutable authored base.
        let page = canonical_page(&evidence, command)?;
        context.observe_body(page).await
    }
    async fn dispatch(&self, token: &SecretToken, request: DispatchRequest) -> DeliveryReport {
        let Ok(mut evidence) = decode_bounded::<Evidence>(&request.execution_base) else {
            return DeliveryReport::unknown();
        };
        let Ok(payload) = decode_payload(&request.command) else {
            return DeliveryReport::unknown();
        };
        if evidence.origin != Origin::Preflight
            || !matches_command(&evidence, &request.command)
            || evidence.authorization_epoch != request.account.authorization_epoch
            || evidence.actor_id != request.account.actor_id
        {
            return DeliveryReport::unknown();
        }
        let Ok(route) = Self::route(&evidence.frame) else {
            return DeliveryReport::unknown();
        };
        let Ok(url) = self.http.endpoint(&route) else {
            return DeliveryReport::unknown();
        };
        let mut patch = serde_json::Map::new();
        if let Some(title) = payload.request.title {
            patch.insert("title".into(), serde_json::Value::String(title));
        }
        if let Some(body) = payload.request.body {
            patch.insert("body".into(), serde_json::Value::String(body));
        }
        let Ok(bytes) = serde_json::to_vec(&patch) else {
            return DeliveryReport::unknown();
        };
        let response = match self
            .http
            .mutate_native(token, reqwest::Method::PATCH, url, bytes)
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
        let mut report = DeliveryReport {
            account_cooldown_seconds: response.cooldown_seconds,
            provider_error: response.provider_error,
            ..DeliveryReport::unknown()
        };
        // Only a bounded canonical 200 response to this exact native PATCH may
        // supply convergence evidence. Status alone cannot confirm or reject.
        if response.status == reqwest::StatusCode::OK
            && let Ok(observation) =
                Self::normalize(&request.account, &evidence.frame, &response.body)
        {
            evidence.observation = observation;
            evidence.origin = Origin::MutationResponse;
            if let Ok(proof) = Self::proof(&evidence)
                && self.validate_evidence(&request.command, EvidencePurpose::Confirmed, &proof)
            {
                report.outcome = DeliveryOutcome::Confirmed(proof);
            }
        }
        report
    }
    async fn reconcile(
        &self,
        token: &SecretToken,
        request: ReconcileRequest,
    ) -> Result<DeliveryReport, ProviderError> {
        let (evidence, cooldown) = self.read(token, &request, Origin::Reconciliation).await?;
        let proof = Self::proof(&evidence).map_err(|_| resource_details::invalid())?;
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
fn matches_command(e: &Evidence, c: &DeliveryCommand) -> bool {
    let Ok(p) = decode_payload(c) else {
        return false;
    };
    (match e.frame.subject.kind {
        RemoteItemKind::PullRequest => c.target_kind == "pull_request",
        RemoteItemKind::Issue => c.target_kind == "issue",
        _ => false,
    }) && e.account_id == c.account_id
        && e.command_hash == hash(c)
        && e.frame.subject.id == c.target_id
        && e.frame.subject.provider_id == p.base.subject_native_id
        && e.frame.repository.provider_id == p.base.repository_native_id
        && e.frame.subject.number.as_deref() == Some(p.base.number.as_str())
        && e.frame.repository.id == c.repository_id.as_deref().unwrap_or("")
        && e.frame.base.source == p.base.source
        && e.frame.subject.account_id == c.account_id
        && e.frame.repository.account_id == c.account_id
        && e.observation
            .body
            .as_ref()
            .is_none_or(|b| b.len() <= MAX_BODY)
        && e.observation.title.len() <= 1024
        && chrono::DateTime::parse_from_rfc3339(&e.observation.provider_updated_at).is_ok()
        && chrono::DateTime::parse_from_rfc3339(&e.observation.observed_at).is_ok()
}
fn canonical_page(
    e: &Evidence,
    command: &DeliveryCommand,
) -> Result<DetailCommit, CollaborationError> {
    if !matches_command(e, command) {
        return Err(invalid());
    }
    let o = &e.observation;
    let f = &e.frame;
    let source = MetadataSource {
        source: f.base.source.clone(),
        adapter_version: 1,
        provider_updated_at: Some(o.provider_updated_at.clone()),
        observed_at: o.observed_at.clone(),
    };
    let mut fields = vec![
        MetadataField::Title,
        MetadataField::State,
        MetadataField::UpdatedAt,
    ];
    if f.subject.kind == RemoteItemKind::PullRequest {
        fields.push(MetadataField::Head);
    }
    Ok(DetailCommit {
        account_id: e.account_id.clone(),
        authorization_epoch: e.authorization_epoch.clone(),
        authorization_view: f.authorization_view.clone(),
        instance_id: "github:https://github.com/".into(),
        subject_id: command.target_id.clone(),
        facet: DetailFacet::Body,
        run_id: f.run_id.clone(),
        request_cursor: None,
        body: DetailValue {
            state: DetailValueState::Known,
            text: o.body.clone(),
        },
        metadata: Some(ResourceMetadataObservation {
            kind: f.subject.kind.clone(),
            values: ResourceMetadataValues {
                title: Some(o.title.clone()),
                state: Some(o.state.clone()),
                updated_at: Some(o.provider_updated_at.clone()),
                head: o.head.clone(),
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
            repository_id: f.repository.id.clone(),
            repository_provider_id: f.repository.provider_id.clone(),
            provider_id: f.subject.provider_id.clone(),
            number: f.subject.number.clone(),
            kind: f.subject.kind.clone(),
            head_oid: f.subject.head_oid.clone(),
        }),
        check_context: None,
        review_context: None,
        entries: vec![],
        source: DetailSource {
            source: f.base.source.clone(),
            adapter_version: 1,
            field_mask: vec![DetailField::Body],
            provider_updated_at: Some(o.provider_updated_at.clone()),
            observed_at: o.observed_at.clone(),
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
