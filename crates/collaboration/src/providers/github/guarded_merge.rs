//! A single guarded synchronous PUT. Accepted/unknown outcomes only perform GETs.
use super::*;
use crate::guarded_merge::native::{decode, encode};
use crate::storage::guarded_merge as store;
use crate::workflow_state::native::{NativeFrame, Observation};
use crate::{
    delivery::*,
    guarded_merge::{native::*, *},
};
use serde_json::Value;
use sqlx::{Sqlite, Transaction};
use std::sync::Arc;
mod grants;
mod recovery;

pub(crate) struct GithubGuardedMergePolicy {
    http: GithubHttp,
    grants: grants::Grants,
}
impl GithubGuardedMergePolicy {
    fn new() -> Result<Self, CollaborationError> {
        Ok(Self {
            http: GithubHttp::new()?,
            grants: Default::default(),
        })
    }
    #[cfg(test)]
    pub(crate) fn for_test(
        base: reqwest::Url,
        now: Arc<dyn Fn() -> std::time::Instant + Send + Sync>,
    ) -> Self {
        Self {
            http: GithubHttp::for_test_base(base).unwrap(),
            grants: grants::Grants::new(now),
        }
    }
    fn route(f: &NativeFrame) -> Result<String, ProviderError> {
        if f.subject.kind != RemoteItemKind::PullRequest
            || !native_id(&f.repository.provider_id)
            || f.subject.number.as_deref().is_none_or(|n| !native_id(n))
        {
            return Err(resource_details::invalid());
        }
        Ok(format!(
            "repositories/{}/pulls/{}",
            f.repository.provider_id,
            f.subject.number.as_deref().unwrap_or_default()
        ))
    }
    pub(crate) fn arm(
        &self,
        a: &RemoteAccount,
        r: &GuardedMergeRequest,
    ) -> Result<Option<NativeFrame>, CollaborationError> {
        self.grants.arm(a, r)
    }
    pub(crate) fn live(&self, a: &RemoteAccount, r: &GuardedMergeRequest) -> bool {
        self.grants.live(a, r)
    }
    pub(crate) async fn preview(
        &self,
        token: &SecretToken,
        a: &RemoteAccount,
        f: &NativeFrame,
    ) -> Result<(GuardedMergePreview, Option<u64>), ProviderError> {
        let (fresh, cooldown) = self.read(token, a, f, true).await?;
        if let Some(seconds) = cooldown {
            return Err(ProviderError {
                kind: ProviderErrorKind::RateLimited,
                retry_after_seconds: Some(seconds),
                account_cooldown_seconds: Some(seconds),
            });
        }
        let reason = unavailable(f.base.head.as_deref().unwrap_or_default(), &fresh, None);
        let context = if reason.is_none() {
            Some(
                self.grants
                    .issue(a, f, fresh.methods.clone())
                    .map_err(|_| observed_invalid(cooldown))?,
            )
        } else {
            None
        };
        Ok((
            GuardedMergePreview {
                context,
                expected_head: fresh.observation.head.as_ref().map(|h| h.oid.clone()),
                methods: fresh.methods,
                can_push: fresh.can_push,
                mergeable: fresh.mergeable,
                provider_mergeability: fresh.mergeability,
                reason,
                observed_at: Some(fresh.observation.observed_at),
                expires_in_seconds: if reason.is_none() {
                    GRANT_SECONDS as u32
                } else {
                    0
                },
                authorization_view: f.authorization_view.clone(),
            },
            cooldown,
        ))
    }
    async fn read(
        &self,
        token: &SecretToken,
        a: &RemoteAccount,
        f: &NativeFrame,
        permissions: bool,
    ) -> Result<(Fresh, Option<u64>), ProviderError> {
        let mut cooldown = None;
        let repository = if permissions {
            let response = self
                .http
                .get_point(
                    self.http
                        .endpoint(&format!("repositories/{}", f.repository.provider_id))?,
                    token,
                )
                .await?;
            cooldown = response.cooldown_seconds;
            if let Some(seconds) = cooldown {
                let mut e = ProviderError::new(ProviderErrorKind::RateLimited);
                e.account_cooldown_seconds = Some(seconds);
                e.retry_after_seconds = Some(seconds);
                return Err(e);
            }
            if response.not_modified || response.next_url.is_some() {
                return Err(resource_details::invalid());
            }
            let json: Value =
                serde_json::from_slice(&response.body).map_err(|_| resource_details::invalid())?;
            if json
                .get("id")
                .and_then(Value::as_u64)
                .map(|n| n.to_string())
                .as_deref()
                != Some(&f.repository.provider_id)
                || json.get("archived").and_then(Value::as_bool) != Some(false)
                || json.get("disabled").and_then(Value::as_bool) != Some(false)
            {
                return Err(resource_details::invalid());
            }
            Some(json)
        } else {
            None
        };
        let response = self
            .http
            .get_point(self.http.endpoint(&Self::route(f)?)?, token)
            .await
            .map_err(|mut e| {
                e.account_cooldown_seconds = e.account_cooldown_seconds.max(cooldown);
                e
            })?;
        cooldown = cooldown.max(response.cooldown_seconds);
        let parsed = (|| {
            if response.not_modified || response.next_url.is_some() {
                return Err(resource_details::invalid());
            }
            normalize(a, f, &response.body, repository.as_ref())
        })()
        .map_err(|mut e: ProviderError| {
            e.account_cooldown_seconds = cooldown;
            e
        })?;
        Ok((parsed, cooldown))
    }
    fn evidence(
        c: &DeliveryCommand,
        a: &RemoteAccount,
        f: NativeFrame,
        origin: Origin,
        fresh: Option<Fresh>,
        result: ResultKind,
        http_status: Option<u16>,
    ) -> Evidence {
        Evidence {
            frame: f,
            account_id: a.id.clone(),
            actor_id: a.actor_id.clone(),
            authorization_epoch: a.authorization_epoch.clone(),
            command_hash: command_hash(c),
            origin,
            fresh,
            result,
            http_status,
        }
    }
    fn proof(e: &Evidence) -> Result<OperationEvidence, CollaborationError> {
        Ok(OperationEvidence {
            kind: PROOF.into(),
            version: 1,
            payload: encode(e)?,
        })
    }
}
impl ProviderRegistry {
    pub fn register_github_guarded_merge(&mut self) -> Result<(), CollaborationError> {
        self.install_guarded_merge(Arc::new(GithubGuardedMergePolicy::new()?))
    }
    pub(crate) fn install_guarded_merge(
        &mut self,
        p: Arc<GithubGuardedMergePolicy>,
    ) -> Result<(), CollaborationError> {
        let instance = ProviderInstance::public(ProviderKind::Github);
        self.register_delivery(&instance, p.clone())?;
        self.register_recovery(&instance, p.clone())?;
        self.guarded_merge = Some(p);
        Ok(())
    }
}

fn observed_invalid(cooldown: Option<u64>) -> ProviderError {
    let mut error = resource_details::invalid();
    error.account_cooldown_seconds = cooldown;
    error
}

fn normalize(
    a: &RemoteAccount,
    f: &NativeFrame,
    bytes: &[u8],
    repo: Option<&Value>,
) -> Result<Fresh, ProviderError> {
    let json: Value = serde_json::from_slice(bytes).map_err(|_| resource_details::invalid())?;
    let merged = json
        .get("merged")
        .and_then(Value::as_bool)
        .ok_or_else(resource_details::invalid)?;
    let request = DetailRequest {
        account: a.clone(),
        repository: f.repository.clone(),
        subject: f.subject.clone(),
        facet: DetailFacet::Body,
        cursor: None,
        etag: None,
        source: None,
    };
    let (mut body, metadata) = pull_details::normalize(&request, bytes, &f.base.source)?;
    let known = |field| {
        metadata
            .fields
            .iter()
            .any(|f| f.field == field && f.state == DetailValueState::Known)
    };
    if !known(crate::MetadataField::State)
        || !known(crate::MetadataField::Head)
        || !known(crate::MetadataField::UpdatedAt)
    {
        return Err(resource_details::invalid());
    }
    if body.text.as_ref().is_some_and(|s| s.len() > 16_384) {
        body = DetailValue {
            state: DetailValueState::Oversized,
            text: None,
        };
    }
    let updated = metadata
        .source
        .provider_updated_at
        .ok_or_else(resource_details::invalid)?;
    if chrono::DateTime::parse_from_rfc3339(&updated)
        .ok()
        .zip(chrono::DateTime::parse_from_rfc3339(&f.base.updated_at).ok())
        .is_none_or(|(new, old)| new < old)
    {
        return Err(resource_details::invalid());
    }
    let merge_oid = json
        .get("merge_commit_sha")
        .and_then(Value::as_str)
        .map(str::to_owned);
    let merged_at = json
        .get("merged_at")
        .and_then(Value::as_str)
        .map(str::to_owned);
    if merged
        && (json.get("state").and_then(Value::as_str) != Some("closed")
            || merge_oid.as_deref().is_none_or(|s| !valid_oid(s))
            || merged_at
                .as_deref()
                .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
                .zip(chrono::DateTime::parse_from_rfc3339(&updated).ok())
                .is_none_or(|(merged, updated)| merged > updated))
    {
        return Err(resource_details::invalid());
    }
    let mut methods = vec![];
    if let Some(repo) = repo {
        for (key, method) in [
            ("allow_merge_commit", MergeMethod::Merge),
            ("allow_squash_merge", MergeMethod::Squash),
            ("allow_rebase_merge", MergeMethod::Rebase),
        ] {
            if repo.get(key).and_then(Value::as_bool) == Some(true) {
                methods.push(method);
            }
        }
    }
    let observation = Observation {
        title: metadata.values.title,
        body,
        state: metadata
            .values
            .state
            .ok_or_else(resource_details::invalid)?,
        head: metadata.values.head,
        provider_updated_at: updated,
        observed_at: metadata.source.observed_at,
    };
    if observation.head.as_ref().is_none_or(|h| !valid_oid(&h.oid)) {
        return Err(resource_details::invalid());
    }
    Ok(Fresh {
        observation,
        merged,
        merge_oid,
        merged_at,
        can_push: repo
            .and_then(|r| r.pointer("/permissions/push"))
            .and_then(Value::as_bool),
        methods,
        draft: json.get("draft").and_then(Value::as_bool),
        mergeable: json.get("mergeable").and_then(Value::as_bool),
        mergeability: json
            .get("mergeable_state")
            .and_then(Value::as_str)
            .filter(|s| s.len() <= 64)
            .map(str::to_owned),
        auto_merge: json.get("auto_merge").is_none_or(|v| !v.is_null()),
    })
}
fn unavailable(
    head: &str,
    f: &Fresh,
    method: Option<MergeMethod>,
) -> Option<MergeUnavailableReason> {
    if f.observation.head.as_ref().is_none_or(|h| h.oid != head) {
        Some(MergeUnavailableReason::HeadChanged)
    } else if f.merged || f.observation.state != "open" {
        Some(MergeUnavailableReason::NotOpen)
    } else if f.draft != Some(false) {
        Some(MergeUnavailableReason::Draft)
    } else if f.can_push != Some(true) {
        Some(MergeUnavailableReason::PermissionUnavailable)
    } else if f.auto_merge {
        Some(MergeUnavailableReason::AutomaticMergeUnsupported)
    } else if f.mergeable != Some(true) || f.mergeability.as_deref() != Some("clean") {
        Some(MergeUnavailableReason::MergeabilityUnavailable)
    } else if f.methods.is_empty() || method.is_some_and(|m| !f.methods.contains(&m)) {
        Some(MergeUnavailableReason::MethodsUnavailable)
    } else {
        None
    }
}

#[async_trait]
impl CommandDeliveryPolicy for GithubGuardedMergePolicy {
    fn operation_kind(&self) -> &'static str {
        OPERATION
    }
    fn payload_version(&self) -> u32 {
        1
    }
    async fn prepare_context_in(
        &self,
        tx: &mut Transaction<'_, Sqlite>,
        c: &DeliveryCommand,
        a: &RemoteAccount,
    ) -> Result<Vec<u8>, CollaborationError> {
        store::prepare_in(tx, c, a).await
    }
    async fn preparation_context_matches_in(
        &self,
        tx: &mut Transaction<'_, Sqlite>,
        c: &DeliveryCommand,
        a: &RemoteAccount,
        expected: &[u8],
    ) -> Result<bool, CollaborationError> {
        store::preparation_matches_in(tx, c, a, expected).await
    }
    async fn prepare(
        &self,
        token: &SecretToken,
        r: &ReconcileRequest,
    ) -> Result<DeliveryPreparation, ProviderError> {
        let p = decode_payload(&r.command).map_err(|_| resource_details::invalid())?;
        let f: NativeFrame = decode(&r.native_context).map_err(|_| resource_details::invalid())?;
        let (origin, fresh, result, cooldown) = if !self.live(&r.account, &p.request) {
            (
                Origin::LocalGate,
                None,
                ResultKind::Declined {
                    reason: MergeUnavailableReason::ConsentExpired,
                },
                None,
            )
        } else {
            let (fresh, cooldown) = self.read(token, &r.account, &f, true).await?;
            let result = if fresh.merged
                && fresh
                    .observation
                    .head
                    .as_ref()
                    .is_some_and(|h| h.oid == p.request.context.expected_head)
            {
                ResultKind::Merged {
                    merge_oid: fresh
                        .merge_oid
                        .clone()
                        .ok_or_else(|| observed_invalid(cooldown))?,
                }
            } else if let Some(reason) = unavailable(
                &p.request.context.expected_head,
                &fresh,
                Some(p.request.method),
            ) {
                ResultKind::Declined { reason }
            } else {
                ResultKind::Ready
            };
            (Origin::Preflight, Some(fresh), result, cooldown)
        };
        let e = Self::evidence(&r.command, &r.account, f, origin, fresh, result, None);
        Ok(DeliveryPreparation {
            bytes: encode(&e).map_err(|_| observed_invalid(cooldown))?,
            account_cooldown_seconds: cooldown,
        })
    }
    async fn validate_claim(
        &self,
        tx: &mut Transaction<'_, Sqlite>,
        c: &DeliveryCommand,
        a: &RemoteAccount,
        bytes: &[u8],
    ) -> Result<ClaimDecision, CollaborationError> {
        let mut e: Evidence = decode(bytes)?;
        let p = decode_payload(c)?;
        if !matches_evidence(&e, c)
            || !matches!(e.origin, Origin::Preflight | Origin::LocalGate)
            || e.actor_id != a.actor_id
            || e.authorization_epoch != a.authorization_epoch
        {
            return Err(invalid());
        }
        crate::storage::workflow_state::validate_frame_in(tx, a, &e.frame).await?;
        if !self.live(a, &p.request) {
            e.origin = Origin::LocalGate;
            e.result = ResultKind::Declined {
                reason: MergeUnavailableReason::ConsentExpired,
            };
            e.fresh = None;
        }
        match &e.result {
            ResultKind::Ready => {
                if e.fresh.as_ref().is_none_or(|f| {
                    unavailable(&p.request.context.expected_head, f, Some(p.request.method))
                        .is_some()
                }) {
                    return Err(invalid());
                }
                Ok(ClaimDecision::Ready(encode(&e)?))
            }
            ResultKind::Merged { .. } => Ok(ClaimDecision::Confirmed(Self::proof(&e)?)),
            ResultKind::Declined { .. } => Ok(ClaimDecision::Conflict(Self::proof(&e)?)),
            ResultKind::Accepted => Err(invalid()),
        }
    }
    fn validate_evidence(
        &self,
        c: &DeliveryCommand,
        purpose: EvidencePurpose,
        proof: &OperationEvidence,
    ) -> bool {
        if proof.kind != PROOF || proof.version != 1 || !proof.bounded() {
            return false;
        }
        let Ok(e) = decode::<Evidence>(&proof.payload) else {
            return false;
        };
        let Ok(p) = decode_payload(c) else {
            return false;
        };
        if !matches_evidence(&e, c) {
            return false;
        }
        match purpose {
            EvidencePurpose::Confirmed=>matches!(&e.result,ResultKind::Merged{merge_oid} if valid_oid(merge_oid))
                && matches!(e.origin,Origin::Preflight|Origin::Reconciliation)
                && (e.origin==Origin::Preflight || c.reconcile_only() || c.attempt_count>0)
                && e.fresh.as_ref().is_some_and(|f|f.merged && f.observation.state=="merged"
                    && f.observation.head.as_ref().is_some_and(|h|h.oid==p.request.context.expected_head)
                    && matches!(&e.result,ResultKind::Merged{merge_oid} if f.merge_oid.as_ref()==Some(merge_oid))
                    && f.merged_at.as_deref().is_some_and(|t|chrono::DateTime::parse_from_rfc3339(t).is_ok())),
            EvidencePurpose::Accepted=>e.origin==Origin::MutationResponse && c.attempt_count>0
                && (matches!(&e.result,ResultKind::Merged{merge_oid} if valid_oid(merge_oid)) && e.http_status==Some(200)
                    || matches!(e.result,ResultKind::Accepted) && e.http_status==Some(202)),
            EvidencePurpose::Conflict=>match e.result {
                ResultKind::Declined{reason:MergeUnavailableReason::ConsentExpired}=>e.origin==Origin::LocalGate,
                ResultKind::Declined{reason:MergeUnavailableReason::ProviderConflict}=>e.origin==Origin::MutationResponse && c.attempt_count>0 && e.http_status==Some(409),
                ResultKind::Declined{reason}=>e.origin==Origin::Preflight && e.fresh.as_ref().is_some_and(|f|unavailable(&p.request.context.expected_head,f,Some(p.request.method))==Some(reason)),
                _=>false,
            },
            EvidencePurpose::Rejected=>matches!(e.result,ResultKind::Declined{reason:MergeUnavailableReason::ProviderRejected})
                && e.origin==Origin::MutationResponse && c.attempt_count>0 && matches!(e.http_status,Some(403|404|405|422)),
            EvidencePurpose::SafeRetry=>false,
        }
    }
    async fn finalize_in(
        &self,
        ctx: &mut crate::storage::effective::finalization::DeliveryFinalization<'_, '_>,
        c: &DeliveryCommand,
        purpose: EvidencePurpose,
        proof: &OperationEvidence,
    ) -> Result<(), CollaborationError> {
        if !matches!(purpose, EvidencePurpose::Confirmed) {
            return Ok(());
        }
        let e: Evidence = decode(&proof.payload)?;
        let workflow = crate::workflow_state::native::Evidence {
            frame: e.frame.clone(),
            observation: e.fresh.as_ref().ok_or_else(invalid)?.observation.clone(),
            account_id: e.account_id.clone(),
            actor_id: e.actor_id.clone(),
            authorization_epoch: e.authorization_epoch.clone(),
            command_hash: e.command_hash.clone(),
            origin: crate::workflow_state::native::Origin::Reconciliation,
        };
        crate::storage::workflow_state::validate_completion_in(ctx.transaction(), &workflow)
            .await?;
        ctx.observe_body(canonical_page(&e, c)?).await
    }
    async fn dispatch(&self, token: &SecretToken, r: DispatchRequest) -> DeliveryReport {
        let Ok(mut e) = decode::<Evidence>(&r.execution_base) else {
            return DeliveryReport::unknown();
        };
        let Ok(p) = decode_payload(&r.command) else {
            return DeliveryReport::unknown();
        };
        if !matches_evidence(&e, &r.command)
            || e.origin != Origin::Preflight
            || !matches!(e.result, ResultKind::Ready)
            || e.actor_id != r.account.actor_id
            || e.authorization_epoch != r.account.authorization_epoch
            || e.fresh.as_ref().is_none_or(|f| {
                unavailable(&p.request.context.expected_head, f, Some(p.request.method)).is_some()
            })
        {
            return DeliveryReport::unknown();
        }
        if !self.grants.consume(&r.account, &p.request) {
            e.origin = Origin::LocalGate;
            e.result = ResultKind::Declined {
                reason: MergeUnavailableReason::ConsentExpired,
            };
            e.fresh = None;
            return DeliveryReport {
                outcome: Self::proof(&e)
                    .map(DeliveryOutcome::Conflict)
                    .unwrap_or(DeliveryOutcome::Unknown),
                ..DeliveryReport::unknown()
            };
        }
        let Ok(route) = Self::route(&e.frame) else {
            return DeliveryReport::unknown();
        };
        let Ok(url) = self.http.endpoint(&format!("{route}/merge")) else {
            return DeliveryReport::unknown();
        };
        let body=serde_json::json!({"sha":p.request.context.expected_head,"merge_method":p.request.method.wire()}).to_string().into_bytes();
        let response = match self
            .http
            .mutate_native(token, reqwest::Method::PUT, url, body)
            .await
        {
            Ok(r) => r,
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
        e.origin = Origin::MutationResponse;
        e.http_status = Some(response.status.as_u16());
        let outcome = match response.status.as_u16() {
            200 => {
                #[derive(serde::Deserialize)]
                #[serde(deny_unknown_fields)]
                struct Receipt {
                    sha: String,
                    merged: bool,
                    message: String,
                }
                let Ok(json) = serde_json::from_slice::<Receipt>(&response.body) else {
                    return report;
                };
                if !valid_oid(&json.sha)
                    || !json.merged
                    || json.message.len() > 1024
                    || json.message.contains('\0')
                {
                    return report;
                }
                let oid = json.sha;
                e.result = ResultKind::Merged { merge_oid: oid };
                // Retain the strong receipt first; a later GET supplies canonical
                // Body timestamps instead of inventing them from this response.
                DeliveryOutcome::Accepted
            }
            202 => {
                e.result = ResultKind::Accepted;
                DeliveryOutcome::Accepted
            }
            409 => {
                e.result = ResultKind::Declined {
                    reason: MergeUnavailableReason::ProviderConflict,
                };
                DeliveryOutcome::Conflict
            }
            403 | 404 | 405 | 422 => {
                e.result = ResultKind::Declined {
                    reason: MergeUnavailableReason::ProviderRejected,
                };
                DeliveryOutcome::Rejected
            }
            _ => return report,
        };
        if let Ok(proof) = Self::proof(&e) {
            report.outcome = outcome(proof);
        }
        report
    }
    async fn reconcile(
        &self,
        token: &SecretToken,
        r: ReconcileRequest,
    ) -> Result<DeliveryReport, ProviderError> {
        let f: NativeFrame = decode(&r.native_context).map_err(|_| resource_details::invalid())?;
        let p = decode_payload(&r.command).map_err(|_| resource_details::invalid())?;
        let (fresh, cooldown) = self.read(token, &r.account, &f, false).await?;
        let mut report = DeliveryReport {
            account_cooldown_seconds: cooldown,
            ..DeliveryReport::unknown()
        };
        if !fresh.merged
            || fresh
                .observation
                .head
                .as_ref()
                .is_none_or(|h| h.oid != p.request.context.expected_head)
        {
            return Ok(report);
        }
        let oid = fresh
            .merge_oid
            .clone()
            .ok_or_else(|| observed_invalid(cooldown))?;
        // If a direct receipt was saved, require its merge result as well as head.
        for saved in &r.command.evidence {
            if saved.evidence.kind == PROOF
                && let Ok(e) = decode::<Evidence>(&saved.evidence.payload)
                && e.origin == Origin::MutationResponse
                && e.http_status == Some(200)
                && matches!(&e.result,ResultKind::Merged{merge_oid} if merge_oid!=&oid)
            {
                return Ok(report);
            }
        }
        let e = Self::evidence(
            &r.command,
            &r.account,
            f,
            Origin::Reconciliation,
            Some(fresh),
            ResultKind::Merged { merge_oid: oid },
            None,
        );
        let proof = Self::proof(&e).map_err(|_| observed_invalid(cooldown))?;
        if self.validate_evidence(&r.command, EvidencePurpose::Confirmed, &proof) {
            report.outcome = DeliveryOutcome::Confirmed(proof);
        }
        Ok(report)
    }
}
fn matches_evidence(e: &Evidence, c: &DeliveryCommand) -> bool {
    let Ok(p) = decode_payload(c) else {
        return false;
    };
    c.operation_kind == OPERATION
        && c.payload_version == 1
        && c.target_kind == "pull_request"
        && e.account_id == c.account_id
        && identifier(&e.actor_id)
        && e.actor_id == p.actor_id
        && e.command_hash == command_hash(c)
        && store::matches_frame(&p, &e.frame)
        && e.frame.repository.id == c.repository_id.as_deref().unwrap_or_default()
        && revision(&e.authorization_epoch, true)
        && revision(&e.frame.authorization_view, false)
        && (if e.origin == Origin::Reconciliation {
            c.reconcile_only() || c.attempt_count > 0
        } else {
            e.authorization_epoch == c.authorization_epoch
                && e.frame.authorization_view == p.request.context.authorization_view
        })
        && e.fresh.as_ref().is_none_or(|f| {
            f.observation.title.as_ref().is_none_or(|t| t.len() <= 4096)
                && f.observation
                    .body
                    .text
                    .as_ref()
                    .is_none_or(|b| b.len() <= 16_384)
                && chrono::DateTime::parse_from_rfc3339(&f.observation.provider_updated_at).is_ok()
                && chrono::DateTime::parse_from_rfc3339(&f.observation.observed_at).is_ok()
        })
}
fn canonical_page(e: &Evidence, c: &DeliveryCommand) -> Result<DetailCommit, CollaborationError> {
    let fresh = e.fresh.as_ref().ok_or_else(invalid)?;
    let o = &fresh.observation;
    let f = &e.frame;
    let mut fields = vec![
        crate::MetadataField::State,
        crate::MetadataField::UpdatedAt,
        crate::MetadataField::Head,
        crate::MetadataField::MergedAt,
    ];
    if o.title.is_some() {
        fields.push(crate::MetadataField::Title);
    }
    Ok(DetailCommit {
        account_id: e.account_id.clone(),
        authorization_epoch: e.authorization_epoch.clone(),
        authorization_view: f.authorization_view.clone(),
        instance_id: "github:https://github.com/".into(),
        subject_id: c.target_id.clone(),
        facet: DetailFacet::Body,
        run_id: f.run_id.clone(),
        request_cursor: None,
        body: o.body.clone(),
        metadata: Some(crate::ResourceMetadataObservation {
            kind: RemoteItemKind::PullRequest,
            values: crate::ResourceMetadataValues {
                title: o.title.clone(),
                state: Some("merged".into()),
                updated_at: Some(o.provider_updated_at.clone()),
                head: o.head.clone(),
                merged_at: fresh.merged_at.clone(),
                ..Default::default()
            },
            fields: fields
                .into_iter()
                .map(|field| crate::MetadataObservedField {
                    field,
                    state: DetailValueState::Known,
                })
                .collect(),
            source: crate::MetadataSource {
                source: f.base.source.clone(),
                adapter_version: 1,
                provider_updated_at: Some(o.provider_updated_at.clone()),
                observed_at: o.observed_at.clone(),
            },
        }),
        subject_binding: Some(crate::DetailSubjectBinding {
            repository_id: f.repository.id.clone(),
            repository_provider_id: f.repository.provider_id.clone(),
            provider_id: f.subject.provider_id.clone(),
            number: f.subject.number.clone(),
            kind: RemoteItemKind::PullRequest,
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
