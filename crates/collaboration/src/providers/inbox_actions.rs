//! Endpoint-specific notification delivery; no generic provider mutation gateway.
use super::{
    transport::{
        GithubHttp,
        mutations::{MutationHttpResponse, mutate, mutation_client},
    },
    *,
};
use crate::{
    credentials::SecretToken, delivery::*, provider_inbox_actions::native::*,
    storage::provider_inbox_actions::current_claim, *,
};
use reqwest::{Client, Method, StatusCode, Url, header};
use serde_json::Value;
use sqlx::{Sqlite, Transaction};
use std::{
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

pub(crate) enum InboxHttp {
    Github(GithubHttp),
    Gitlab { client: Client, base: Url },
}
pub(crate) struct InboxPolicy {
    pub(crate) instance: ProviderInstance,
    pub(crate) http: InboxHttp,
}
impl InboxPolicy {
    fn new(provider: ProviderKind) -> Result<Self, ProviderError> {
        let http = match provider {
            ProviderKind::Github => InboxHttp::Github(GithubHttp::new()?),
            ProviderKind::Gitlab => {
                let base = Url::parse("https://gitlab.com/api/v4/").unwrap();
                InboxHttp::Gitlab {
                    client: mutation_client(&base)?,
                    base,
                }
            }
            _ => return Err(ProviderError::new(ProviderErrorKind::Unsupported)),
        };
        Ok(Self {
            instance: ProviderInstance::public(provider),
            http,
        })
    }
    fn valid(&self, p: &Payload, account: &RemoteAccount) -> bool {
        p.validate().is_ok()
            && self.instance.id == p.instance
            && account.actor_id == p.actor
            && account.provider == self.instance.provider
            && ProviderInstance::for_account(account).is_ok_and(|i| i == self.instance)
            && account.notifications_supported
    }
    async fn observe(
        &self,
        token: &SecretToken,
        p: &Payload,
        done: bool,
    ) -> Result<(Observation, Option<u64>), ProviderError> {
        match &self.http {
            InboxHttp::Github(http) => {
                let page = http
                    .get_point(
                        http.endpoint(&format!("notifications/threads/{}", p.native_id))?,
                        token,
                    )
                    .await?;
                let observed = parse_github(&page.body, p).map_err(|mut e| {
                    e.account_cooldown_seconds = page.cooldown_seconds;
                    e
                })?;
                Ok((observed, page.cooldown_seconds))
            }
            InboxHttp::Gitlab { client, base } => {
                // One finite provider round. No undocumented GET /todos/:id and
                // no list absence can establish completed state or permission.
                let mut url = base.join("todos").map_err(|_| bad())?;
                url.query_pairs_mut()
                    .append_pair("project_id", &p.project)
                    .append_pair("state", if done { "done" } else { "pending" })
                    .append_pair("per_page", "100")
                    .append_pair("page", "1");
                let (bytes, cooldown) = gitlab_read(client, url, token).await?;
                let observed = (|| {
                    let values: Vec<Value> = serde_json::from_slice(&bytes).map_err(|_| bad())?;
                    if values.len() > 100 {
                        return Err(bad());
                    }
                    let mut matches = values.iter().filter(|v| {
                        v.get("id")
                            .and_then(Value::as_u64)
                            .map(|id| id.to_string())
                            .as_deref()
                            == Some(p.native_id.as_str())
                    });
                    let value = matches
                        .next()
                        .ok_or_else(|| ProviderError::new(ProviderErrorKind::NotFound))?;
                    if matches.next().is_some() {
                        return Err(bad());
                    }
                    parse_gitlab(value, p)
                })()
                .map_err(|mut e: ProviderError| {
                    e.account_cooldown_seconds = cooldown;
                    e
                })?;
                Ok((observed, cooldown))
            }
        }
    }
    async fn mutate(
        &self,
        token: &SecretToken,
        p: &Payload,
    ) -> Result<MutationHttpResponse, ProviderError> {
        match &self.http {
            InboxHttp::Github(http) => {
                http.mutate_native(
                    token,
                    Method::PATCH,
                    http.endpoint(&format!("notifications/threads/{}", p.native_id))?,
                    vec![],
                )
                .await
            }
            InboxHttp::Gitlab { client, base } => {
                mutate(
                    client,
                    base,
                    token,
                    Method::POST,
                    base.join(&format!("todos/{}/mark_as_done", p.native_id))
                        .map_err(|_| bad())?,
                    vec![],
                    false,
                )
                .await
            }
        }
    }
}
impl ProviderRegistry {
    /// Install only the compiled public GitHub/GitLab inbox operation policies.
    /// Adapters must already be registered for those exact installations.
    pub fn register_provider_inbox_actions(&mut self) -> Result<(), CollaborationError> {
        for provider in [ProviderKind::Github, ProviderKind::Gitlab] {
            let policy = Arc::new(InboxPolicy::new(provider).map_err(|_| {
                CollaborationError::new(
                    ErrorCode::Provider,
                    "Unable to initialize provider inbox operations",
                )
            })?);
            self.register_delivery(&policy.instance, policy.clone())?;
            self.register_recovery(&policy.instance, policy.clone())?;
        }
        Ok(())
    }
}
fn bad() -> ProviderError {
    ProviderError::new(ProviderErrorKind::InvalidResponse)
}
fn time(v: &Value) -> Result<String, ProviderError> {
    let s = v.as_str().ok_or_else(bad)?;
    chrono::DateTime::parse_from_rfc3339(s).map_err(|_| bad())?;
    if s.len() > 128 {
        return Err(bad());
    }
    Ok(s.into())
}
fn parse_github(bytes: &[u8], p: &Payload) -> Result<Observation, ProviderError> {
    let v: Value = serde_json::from_slice(bytes).map_err(|_| bad())?;
    if v.get("id").and_then(Value::as_str) != Some(&p.native_id)
        || v.pointer("/repository/id")
            .and_then(Value::as_u64)
            .map(|id| id.to_string())
            .as_deref()
            != Some(&p.project)
        || v.pointer("/subject/type").and_then(Value::as_str) != Some(&p.subject_type)
    {
        return Err(bad());
    }
    Ok(Observation {
        updated_at: time(v.get("updated_at").ok_or_else(bad)?)?,
        applied: !v.get("unread").and_then(Value::as_bool).ok_or_else(bad)?,
    })
}
fn parse_gitlab(v: &Value, p: &Payload) -> Result<Observation, ProviderError> {
    if v.get("id")
        .and_then(Value::as_u64)
        .map(|id| id.to_string())
        .as_deref()
        != Some(&p.native_id)
        || v.pointer("/project/id")
            .and_then(Value::as_u64)
            .map(|id| id.to_string())
            .as_deref()
            != Some(&p.project)
        || v.get("target_type").and_then(Value::as_str) != Some(&p.subject_type)
        || v.get("action_name").and_then(Value::as_str) != Some(&p.native_action)
    {
        return Err(bad());
    }
    let applied = match v.get("state").and_then(Value::as_str) {
        Some("pending") => false,
        Some("done") => true,
        _ => return Err(bad()),
    };
    Ok(Observation {
        updated_at: time(v.get("updated_at").ok_or_else(bad)?)?,
        applied,
    })
}
async fn gitlab_read(
    client: &Client,
    url: Url,
    token: &SecretToken,
) -> Result<(Vec<u8>, Option<u64>), ProviderError> {
    let mut auth = header::HeaderValue::from_str(token.expose()).map_err(|_| bad())?;
    auth.set_sensitive(true);
    let mut observed = None;
    tokio::time::timeout(Duration::from_secs(20), async {
        let mut r = client
            .get(url)
            .header("PRIVATE-TOKEN", auth)
            .header(header::ACCEPT, "application/json")
            .header(header::USER_AGENT, "Gitru-Desktop")
            .send()
            .await
            .map_err(|_| ProviderError::new(ProviderErrorKind::Offline))?;
        let number = |name: &str| {
            r.headers()
                .get(name)
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.parse::<u64>().ok())
        };
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let cooldown = (number("ratelimit-remaining") == Some(0))
            .then(|| {
                number("ratelimit-reset")
                    .map(|t| t.saturating_sub(now).max(1))
                    .unwrap_or(60)
            })
            .into_iter()
            .chain(number("retry-after").map(|n| n.max(1)))
            .max()
            .or_else(|| (r.status() == StatusCode::TOO_MANY_REQUESTS).then_some(60));
        observed = cooldown;
        let err = |kind| ProviderError {
            kind,
            retry_after_seconds: cooldown,
            account_cooldown_seconds: cooldown,
        };
        if r.status() != StatusCode::OK {
            return Err(err(match r.status() {
                StatusCode::UNAUTHORIZED => ProviderErrorKind::Authentication,
                StatusCode::FORBIDDEN if cooldown.is_some() => ProviderErrorKind::RateLimited,
                StatusCode::FORBIDDEN => ProviderErrorKind::Permission,
                StatusCode::TOO_MANY_REQUESTS => ProviderErrorKind::RateLimited,
                StatusCode::NOT_FOUND => ProviderErrorKind::NotFound,
                _ => ProviderErrorKind::Unavailable,
            }));
        }
        if r.content_length().is_some_and(|n| n > 4 * 1024 * 1024)
            || r.headers()
                .get(header::CONTENT_TYPE)
                .and_then(|v| v.to_str().ok())
                .is_none_or(|v| v.split(';').next() != Some("application/json"))
        {
            return Err(err(ProviderErrorKind::InvalidResponse));
        }
        let mut bytes = Vec::new();
        while let Some(chunk) = r
            .chunk()
            .await
            .map_err(|_| err(ProviderErrorKind::Unavailable))?
        {
            if chunk.len() > (4 * 1024 * 1024usize).saturating_sub(bytes.len()) {
                return Err(err(ProviderErrorKind::InvalidResponse));
            }
            bytes.extend_from_slice(&chunk);
        }
        Ok((bytes, cooldown))
    })
    .await
    .map_err(|_| ProviderError {
        kind: ProviderErrorKind::Offline,
        retry_after_seconds: None,
        account_cooldown_seconds: observed,
    })?
}
fn encode<T: serde::Serialize>(v: &T) -> Result<Vec<u8>, CollaborationError> {
    let bytes = serde_json::to_vec(v).map_err(|_| invalid())?;
    if bytes.len() > 65_536 {
        return Err(invalid());
    }
    Ok(bytes)
}
fn decode<T: serde::de::DeserializeOwned>(bytes: &[u8]) -> Result<T, CollaborationError> {
    if bytes.len() > 65_536 {
        return Err(invalid());
    }
    serde_json::from_slice(bytes).map_err(|_| invalid())
}
fn same_time(a: &str, b: &str) -> bool {
    chrono::DateTime::parse_from_rfc3339(a)
        .ok()
        .zip(chrono::DateTime::parse_from_rfc3339(b).ok())
        .is_some_and(|(a, b)| a == b)
}
fn proof(kind: &str, body: &Proof) -> Result<OperationEvidence, CollaborationError> {
    Ok(OperationEvidence {
        kind: kind.into(),
        version: 1,
        payload: encode(body)?,
    })
}
const OBSERVED: &str = "provider.inbox.observed";
const CHANGED: &str = "provider.inbox.changed";

#[async_trait::async_trait]
impl CommandDeliveryPolicy for InboxPolicy {
    fn operation_kind(&self) -> &'static str {
        KIND
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
        encode(
            &crate::storage::provider_inbox_actions::frame_in(tx, account, &command.target_id)
                .await?,
        )
    }
    async fn prepare(
        &self,
        token: &SecretToken,
        request: &ReconcileRequest,
    ) -> Result<DeliveryPreparation, ProviderError> {
        let p = Payload::parse(&request.command.payload).map_err(|_| bad())?;
        if !self.valid(&p, &request.account) || request.instance_id != self.instance.id {
            return Err(bad());
        }
        let frame: Frame = decode(&request.native_context).map_err(|_| bad())?;
        let (observation, cooldown) = self.observe(token, &p, false).await?;
        Ok(DeliveryPreparation {
            bytes: encode(&Proof {
                payload: p,
                frame,
                observation,
            })
            .map_err(|_| ProviderError {
                kind: ProviderErrorKind::InvalidResponse,
                retry_after_seconds: None,
                account_cooldown_seconds: cooldown,
            })?,
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
        let p = Payload::parse(&command.payload)?;
        if !self.valid(&p, account) {
            return Err(invalid());
        }
        let captured: Proof = decode(preparation)?;
        let observation = captured.observation.clone();
        if captured.payload != p {
            return Err(invalid());
        }
        let frame = current_claim(tx, account, command, &p).await;
        let changed = || {
            Ok(ClaimDecision::Conflict(OperationEvidence {
                kind: CHANGED.into(),
                version: 1,
                payload: encode(&p)?,
            }))
        };
        if !same_time(&observation.updated_at, &p.updated_at) {
            return changed();
        }
        match frame {
            Ok(frame) => {
                if encode(&frame)? != encode(&captured.frame)? {
                    return changed();
                }
                let proof_body = Proof {
                    payload: p,
                    frame,
                    observation,
                };
                if proof_body.observation.applied {
                    Ok(ClaimDecision::Confirmed(proof(OBSERVED, &proof_body)?))
                } else {
                    Ok(ClaimDecision::Ready(encode(&proof_body)?))
                }
            }
            Err(e) if matches!(e.code, ErrorCode::StaleView | ErrorCode::NotFound) => changed(),
            Err(e) => Err(e),
        }
    }
    fn validate_evidence(
        &self,
        command: &DeliveryCommand,
        purpose: EvidencePurpose,
        e: &OperationEvidence,
    ) -> bool {
        let Ok(p) = Payload::parse(&command.payload) else {
            return false;
        };
        if p.instance != self.instance.id || e.version != 1 {
            return false;
        }
        if purpose == EvidencePurpose::Conflict && e.kind == CHANGED {
            return decode::<Payload>(&e.payload).is_ok_and(|saved| saved == p);
        }
        if purpose != EvidencePurpose::Confirmed || e.kind != OBSERVED {
            return false;
        }
        let Ok(proof) = decode::<Proof>(&e.payload) else {
            return false;
        };
        proof.payload == p
            && proof.frame.instance == p.instance
            && !proof.frame.view.is_empty()
            && proof.frame.item.id == command.target_id
            && proof.frame.item.account_id == command.account_id
            && proof.frame.item.provider_id == p.native_id
            && proof.frame.item.kind == RemoteItemKind::Notification
            && proof.frame.item.repository_id == command.repository_id
            && (activity(&proof.frame.item).is_ok_and(|v| v == p.activity)
                || observed_convergence(&p, &proof.frame, &proof.observation))
            && proof.observation.applied
            && chrono::DateTime::parse_from_rfc3339(&proof.observation.updated_at)
                .ok()
                .zip(chrono::DateTime::parse_from_rfc3339(&p.updated_at).ok())
                .is_some_and(|(a, b)| a >= b)
    }
    async fn finalize_in(
        &self,
        context: &mut crate::storage::effective::finalization::DeliveryFinalization<'_, '_>,
        command: &DeliveryCommand,
        purpose: EvidencePurpose,
        e: &OperationEvidence,
    ) -> Result<(), CollaborationError> {
        if purpose != EvidencePurpose::Confirmed {
            return Ok(());
        }
        if !self.validate_evidence(command, purpose, e) {
            return Err(invalid());
        }
        let p: Proof = decode(&e.payload)?;
        context
            .observe_notification(
                crate::storage::effective::finalization::CanonicalNotificationObservation {
                    expected: p.frame.item,
                    authorization_view: p.frame.view,
                    instance_id: p.frame.instance,
                    source: MetadataSource {
                        source: OBSERVED.into(),
                        adapter_version: 1,
                        provider_updated_at: Some(p.observation.updated_at),
                        observed_at: chrono::Utc::now().to_rfc3339(),
                    },
                    values: p.payload.patch(),
                },
            )
            .await
    }
    async fn dispatch(&self, token: &SecretToken, request: DispatchRequest) -> DeliveryReport {
        let mut report = DeliveryReport::unknown();
        let Ok(mut captured) = decode::<Proof>(&request.execution_base) else {
            return report;
        };
        if !self.valid(&captured.payload, &request.account)
            || Payload::parse(&request.command.payload).ok().as_ref() != Some(&captured.payload)
            || request.instance_id != self.instance.id
        {
            return report;
        }
        // A same-activity point read already observing the desired state avoids
        // a redundant provider write while preserving native canonical proof.
        if captured.observation.applied {
            if let Ok(e) = proof(OBSERVED, &captured) {
                report.outcome = DeliveryOutcome::Confirmed(e);
            }
            return report;
        }
        let response = match self.mutate(token, &captured.payload).await {
            Ok(r) => r,
            Err(e) => {
                report.account_cooldown_seconds = e.account_cooldown_seconds;
                report.retry_after_seconds = e.retry_after_seconds;
                report.provider_error = Some(e);
                return report;
            }
        };
        report.account_cooldown_seconds = response.cooldown_seconds;
        report.retry_after_seconds = response
            .provider_error
            .as_ref()
            .and_then(|e| e.retry_after_seconds);
        report.provider_error = response.provider_error;
        let observed = match (&self.http, response.status) {
            (InboxHttp::Github(_), StatusCode::RESET_CONTENT) if response.body.is_empty() => {
                Some(Observation {
                    updated_at: captured.payload.updated_at.clone(),
                    applied: true,
                })
            }
            (InboxHttp::Gitlab { .. }, StatusCode::OK) => {
                serde_json::from_slice::<Value>(&response.body)
                    .ok()
                    .and_then(|v| parse_gitlab(&v, &captured.payload).ok())
                    .filter(|o| o.applied)
            }
            _ => None,
        };
        if let Some(observation) = observed {
            captured.observation = observation;
            if let Ok(e) = proof(OBSERVED, &captured)
                && self.validate_evidence(&request.command, EvidencePurpose::Confirmed, &e)
            {
                report.outcome = DeliveryOutcome::Confirmed(e);
            }
        }
        report
    }
    async fn reconcile(
        &self,
        token: &SecretToken,
        request: ReconcileRequest,
    ) -> Result<DeliveryReport, ProviderError> {
        let p = Payload::parse(&request.command.payload).map_err(|_| bad())?;
        if !self.valid(&p, &request.account) || request.instance_id != self.instance.id {
            return Err(bad());
        }
        let frame: Frame = decode(&request.native_context).map_err(|_| bad())?;
        let (observation, cooldown) = self.observe(token, &p, true).await?;
        let mut report = DeliveryReport::unknown();
        report.account_cooldown_seconds = cooldown;
        if !observation.applied {
            return Ok(report);
        }
        // Reconciliation observes the current authorized raw frame. A newer
        // local activity invalidates original intent; absence never confirms.
        if activity(&frame.item).map_err(|_| bad())? != p.activity
            && !observed_convergence(&p, &frame, &observation)
        {
            return Ok(report);
        }
        let captured = Proof {
            payload: p,
            frame,
            observation,
        };
        if let Ok(e) = proof(OBSERVED, &captured)
            && self.validate_evidence(&request.command, EvidencePurpose::Confirmed, &e)
        {
            report.outcome = DeliveryOutcome::Confirmed(e);
        }
        Ok(report)
    }
}

#[async_trait::async_trait]
impl crate::command_recovery::policy::CommandRecoveryPolicy for InboxPolicy {
    fn instance_id(&self) -> &str {
        &self.instance.id
    }
    fn operation_kind(&self) -> &'static str {
        KIND
    }
    fn payload_version(&self) -> u32 {
        1
    }
    async fn review_in(
        &self,
        tx: &mut Transaction<'_, Sqlite>,
        command: &DeliveryCommand,
        account: &RemoteAccount,
    ) -> Result<crate::command_recovery::policy::NativeRecoveryReview, CollaborationError> {
        use crate::command_recovery::policy::{NativeRecoveryReview, compare_field};
        let payload = Payload::parse(&command.payload)?;
        let frame =
            crate::storage::provider_inbox_actions::frame_in(tx, account, &command.target_id)
                .await?;
        let field = match payload.action {
            ProviderInboxAction::MarkRead => CommandReviewField::Unread,
            ProviderInboxAction::MarkDone => CommandReviewField::State,
        };
        let value = |s: &str| CommandFieldValue {
            known: true,
            value: Some(s.into()),
        };
        let base = value(if payload.action == ProviderInboxAction::MarkRead {
            "true"
        } else {
            "pending"
        });
        let desired = value(if payload.action == ProviderInboxAction::MarkRead {
            "false"
        } else {
            "done"
        });
        let remote = match (&frame.item.native_inbox, payload.action) {
            (Some(NativeInboxState::Notification { unread }), ProviderInboxAction::MarkRead) => {
                value(if *unread { "true" } else { "false" })
            }
            (Some(NativeInboxState::Todo { completion, .. }), ProviderInboxAction::MarkDone) => {
                value(if *completion == TodoCompletion::Done {
                    "done"
                } else {
                    "pending"
                })
            }
            _ => CommandFieldValue {
                known: false,
                value: None,
            },
        };
        let can_replace =
            self.valid(&payload, account) && remote == base && frame.instance == payload.instance;
        let comparison = compare_field(field, &base, &remote, &desired);
        Ok(NativeRecoveryReview{fields:vec![CommandFieldReview{field,base,remote,desired,comparison,editable:false}],can_replace,reason:Some(if can_replace{"Replacement applies to the currently cached notification. Best effort may also affect concurrent provider activity; no server activity guard exists."}else{"The current provider item does not support replacing this action."}.into()),fence:encode(&frame)?})
    }
    async fn replace_in(
        &self,
        tx: &mut Transaction<'_, Sqlite>,
        command: &DeliveryCommand,
        account: &RemoteAccount,
        request: &CommandRecoveryReplaceRequest,
        review: &crate::command_recovery::policy::NativeRecoveryReview,
    ) -> Result<crate::storage::command_admission::CommandReceipt, CollaborationError> {
        let payload = Payload::parse(&command.payload)?;
        let frame =
            crate::storage::provider_inbox_actions::frame_in(tx, account, &command.target_id)
                .await?;
        if !review.can_replace || encode(&frame)? != review.fence || !request.fields.is_empty() {
            return Err(invalid());
        }
        let p =
            crate::storage::provider_inbox_actions::payload_in(tx, account, &frame, payload.action)
                .await?;
        let request = QueueProviderInboxActionRequest {
            account_id: account.id.clone(),
            authorization_epoch: account.authorization_epoch.clone(),
            authorization_view: frame.view.clone(),
            subject_id: frame.item.id.clone(),
            expected_activity_version: activity(&frame.item)?,
            command_id: request.new_command_id.clone(),
            action: payload.action,
            activity_policy: ProviderInboxActivityPolicy::BestEffortCurrentItem,
        };
        let sealed = seal(&request, p, frame.item.repository_id)?;
        crate::storage::command_admission::admit_in(
            tx,
            &sealed,
            &crate::storage::provider_inbox_actions::Admission,
        )
        .await
        .map_err(|e| match e {
            crate::storage::command_admission::CommandAdmissionError::Local(e) => e,
            _ => invalid(),
        })
    }
}

fn observed_convergence(p: &Payload, frame: &Frame, observation: &Observation) -> bool {
    if !observation.applied
        || !same_time(&frame.item.updated_at, &observation.updated_at)
        || frame.item.provider_id != p.native_id
    {
        return false;
    }
    match (&frame.item.native_inbox, p.action) {
        (Some(NativeInboxState::Notification { unread: false }), ProviderInboxAction::MarkRead) => {
            frame.item.unread == Some(false) && frame.item.state == p.subject_type
        }
        (
            Some(NativeInboxState::Todo {
                completion: TodoCompletion::Done,
                action,
                target_type,
            }),
            ProviderInboxAction::MarkDone,
        ) => {
            frame.item.state == "done"
                && frame.item.unread.is_none()
                && action == &p.native_action
                && target_type == &p.subject_type
        }
        _ => false,
    }
}
#[cfg(test)]
mod tests;
