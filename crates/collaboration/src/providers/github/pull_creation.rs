//! Explicit online consent permits one branch-name POST. Only its causal 201 proves creation.
use super::*;
use crate::pull_creation::native::{decode, encode};
use crate::{
    delivery::*,
    pull_creation::{native::*, *},
    storage::pull_creation as store,
};
use chrono::Utc;
use serde_json::Value;
use sqlx::{Sqlite, Transaction};
use std::sync::Arc;
mod grants;
pub(crate) mod receipt;

pub(crate) struct GithubPullCreationPolicy {
    http: GithubHttp,
    grants: grants::Grants,
}
struct Fresh {
    source_oid: String,
    base_oid: String,
    can_push: bool,
    observed_at: String,
}
impl GithubPullCreationPolicy {
    pub(crate) fn clear_grants(&self) {
        self.grants.clear();
    }
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
    pub(crate) fn arm(
        &self,
        a: &RemoteAccount,
        r: &SubmitPullRequest,
        owner: &PullCreationOwner,
    ) -> Result<Option<Frame>, CollaborationError> {
        self.grants.arm(a, r, owner)
    }
    pub(crate) fn live(&self, a: &RemoteAccount, r: &SubmitPullRequest) -> bool {
        self.grants.live(a, r)
    }
    fn route(f: &Frame) -> Result<String, ProviderError> {
        if !native_id(&f.repository.provider_id) || !repository_path(&f.repository.full_name) {
            return Err(bad(None));
        }
        Ok(format!("repositories/{}/pulls", f.repository.provider_id))
    }
    async fn read(
        &self,
        token: &SecretToken,
        f: &Frame,
    ) -> Result<(Fresh, Option<u64>), ProviderError> {
        Self::route(f)?;
        let response = self
            .http
            .get_point(
                self.http
                    .endpoint(&format!("repositories/{}", f.repository.provider_id))?,
                token,
            )
            .await?;
        let mut cooldown = response.cooldown_seconds;
        stop_for_cooldown(cooldown)?;
        let repo: Value = serde_json::from_slice(&response.body).map_err(|_| bad(cooldown))?;
        if response.not_modified
            || response.next_url.is_some()
            || repo
                .get("id")
                .and_then(Value::as_u64)
                .map(|v| v.to_string())
                .as_deref()
                != Some(&f.repository.provider_id)
            || repo.get("full_name").and_then(Value::as_str) != Some(&f.repository.full_name)
            || repo.get("html_url").and_then(Value::as_str) != Some(&f.repository.web_url)
            || repo.get("archived").and_then(Value::as_bool) != Some(false)
            || repo.get("disabled").and_then(Value::as_bool) != Some(false)
        {
            return Err(bad(cooldown));
        }
        let can_push = repo.pointer("/permissions/push").and_then(Value::as_bool) == Some(true);
        // No head authority is fabricated from repository metadata. Each exact branch is read independently.
        let mut tips = Vec::with_capacity(2);
        for name in [&f.values.source_branch, &f.values.base_branch] {
            if !branch(name) {
                return Err(bad(cooldown));
            }
            let mut url = self.http.endpoint(&format!(
                "repositories/{}/branches/",
                f.repository.provider_id
            ))?;
            url.path_segments_mut()
                .map_err(|_| bad(cooldown))?
                .pop_if_empty()
                .push(name);
            let response = self.http.get_point(url, token).await.map_err(|mut e| {
                e.account_cooldown_seconds = e.account_cooldown_seconds.max(cooldown);
                e
            })?;
            cooldown = cooldown.max(response.cooldown_seconds);
            stop_for_cooldown(cooldown)?;
            let json: Value = serde_json::from_slice(&response.body).map_err(|_| bad(cooldown))?;
            let tip = json
                .pointer("/commit/sha")
                .and_then(Value::as_str)
                .filter(|s| oid(s))
                .ok_or_else(|| bad(cooldown))?;
            if response.not_modified
                || response.next_url.is_some()
                || json.get("name").and_then(Value::as_str) != Some(name)
            {
                return Err(bad(cooldown));
            }
            tips.push(tip.to_owned());
        }
        Ok((
            Fresh {
                source_oid: tips.remove(0),
                base_oid: tips.remove(0),
                can_push,
                observed_at: Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Nanos, true),
            },
            cooldown,
        ))
    }
    pub(crate) async fn preview(
        &self,
        token: &SecretToken,
        a: &RemoteAccount,
        key: &PullDraftKey,
        f: &Frame,
        owner: &PullCreationOwner,
    ) -> Result<(PullCreationPreview, Option<u64>), ProviderError> {
        validate_receipt_budget(f).map_err(|_| bad(None))?;
        let (fresh, cooldown) = self.read(token, f).await?;
        let reason = if !fresh.can_push {
            Some(PullCreationReason::PermissionUnavailable)
        } else if fresh.source_oid != f.local.source_oid {
            Some(PullCreationReason::UnpublishedSource)
        } else if fresh.source_oid == fresh.base_oid {
            Some(PullCreationReason::SameBranch)
        } else {
            None
        };
        let context = if reason.is_none() {
            Some(
                self.grants
                    .issue(
                        a,
                        f,
                        key,
                        fresh.source_oid.clone(),
                        fresh.base_oid.clone(),
                        owner,
                    )
                    .map_err(|_| bad(cooldown))?,
            )
        } else {
            None
        };
        Ok((
            PullCreationPreview {
                context,
                reason,
                values: f.values.clone(),
                local_source_oid: f.local.source_oid.clone(),
                observed_source_oid: Some(fresh.source_oid),
                observed_base_oid: Some(fresh.base_oid),
                can_push: Some(fresh.can_push),
                observed_at: fresh.observed_at,
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
}
fn bad(cooldown: Option<u64>) -> ProviderError {
    let mut e = resource_details::invalid();
    e.account_cooldown_seconds = cooldown;
    e
}
fn stop_for_cooldown(c: Option<u64>) -> Result<(), ProviderError> {
    if let Some(seconds) = c {
        let mut e = ProviderError::new(ProviderErrorKind::RateLimited);
        e.account_cooldown_seconds = c;
        e.retry_after_seconds = Some(seconds);
        Err(e)
    } else {
        Ok(())
    }
}
fn matches_command(p: &Preparation, c: &DeliveryCommand) -> bool {
    decode(c).is_ok_and(|payload| {
        preparation_matches(p, &payload)
            && p.command_hash == command_hash(c)
            && c.target_kind == "repository"
            && c.target_id == p.frame.repository.id
            && c.repository_id.as_deref() == Some(&p.frame.repository.id)
    })
}
fn declined(
    preparation: Preparation,
    reason: PullCreationReason,
) -> Result<OperationEvidence, CollaborationError> {
    Ok(OperationEvidence {
        kind: "github.pull_creation_declined".into(),
        version: 1,
        payload: encode(&DeclinedEvidence {
            preparation,
            reason,
        })?,
    })
}
impl ProviderRegistry {
    pub fn register_github_pull_creation(&mut self) -> Result<(), CollaborationError> {
        self.install_pull_creation(Arc::new(GithubPullCreationPolicy::new()?))
    }
    pub(crate) fn install_pull_creation(
        &mut self,
        p: Arc<GithubPullCreationPolicy>,
    ) -> Result<(), CollaborationError> {
        let i = ProviderInstance::public(ProviderKind::Github);
        self.register_delivery(&i, p.clone())?;
        self.register_recovery(&i, p.clone())?;
        self.pull_creation = Some(p);
        Ok(())
    }
}
#[async_trait]
impl CommandDeliveryPolicy for GithubPullCreationPolicy {
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
    async fn prepare(
        &self,
        token: &SecretToken,
        r: &ReconcileRequest,
    ) -> Result<DeliveryPreparation, ProviderError> {
        let p = decode(&r.command).map_err(|_| bad(None))?;
        let f: Frame = decode_json(&r.native_context).map_err(|_| bad(None))?;
        let (refusal, cooldown, observed_at) = if !self.live(&r.account, &p.request) {
            (
                Some(PullCreationReason::GrantExpired),
                None,
                Utc::now().to_rfc3339(),
            )
        } else {
            let (fresh, cooldown) = self.read(token, &f).await?;
            let reason = if !fresh.can_push {
                Some(PullCreationReason::PermissionUnavailable)
            } else if fresh.source_oid != p.request.context.source_oid
                || fresh.base_oid != p.request.context.base_oid
            {
                Some(PullCreationReason::LocalHeadChanged)
            } else {
                None
            };
            (reason, cooldown, fresh.observed_at)
        };
        let prepared = Prepared {
            preparation: Preparation {
                frame: f,
                actor: r.account.actor_id.clone(),
                epoch: r.account.authorization_epoch.clone(),
                command_hash: command_hash(&r.command),
                source_oid: p.request.context.source_oid,
                base_oid: p.request.context.base_oid,
                observed_at,
            },
            refusal,
        };
        Ok(DeliveryPreparation {
            bytes: encode(&prepared).map_err(|_| bad(cooldown))?,
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
        let mut prepared: Prepared = decode_json(bytes)?;
        let p = decode(c)?;
        if !matches_command(&prepared.preparation, c)
            || prepared.preparation.actor != a.actor_id
            || prepared.preparation.epoch != a.authorization_epoch
        {
            return Err(invalid());
        }
        store::validate_frame_in(tx, a, &prepared.preparation.frame, &p.request.context.key)
            .await?;
        if !self.live(a, &p.request) {
            prepared.refusal = Some(PullCreationReason::GrantExpired);
        }
        match prepared.refusal {
            Some(reason) => Ok(ClaimDecision::Conflict(declined(
                prepared.preparation,
                reason,
            )?)),
            None => Ok(ClaimDecision::Ready(encode(&prepared.preparation)?)),
        }
    }
    fn validate_evidence(
        &self,
        c: &DeliveryCommand,
        purpose: EvidencePurpose,
        proof: &OperationEvidence,
    ) -> bool {
        if proof.version != 1 || !proof.bounded() {
            return false;
        }
        let Ok(p) = decode(c) else {
            return false;
        };
        if purpose == EvidencePurpose::Conflict && proof.kind == "github.pull_creation_declined" {
            return decode_json::<DeclinedEvidence>(&proof.payload)
                .is_ok_and(|e| matches_command(&e.preparation, c) && declined_matches(&e, &p));
        }
        purpose == EvidencePurpose::Confirmed
            && proof.kind == PROOF
            && c.attempt_count > 0
            && decode_json::<ReceiptEvidence>(&proof.payload)
                .is_ok_and(|e| matches_command(&e.preparation, c) && receipt_matches(&e, &p))
    }
    async fn finalize_in(
        &self,
        context: &mut crate::storage::effective::finalization::DeliveryFinalization<'_, '_>,
        c: &DeliveryCommand,
        purpose: EvidencePurpose,
        proof: &OperationEvidence,
    ) -> Result<(), CollaborationError> {
        if purpose == EvidencePurpose::Conflict {
            return Ok(());
        }
        if purpose != EvidencePurpose::Confirmed {
            return Err(invalid());
        }
        let e: ReceiptEvidence = decode_json(&proof.payload)?;
        store::finalize_in(context.transaction(), c, &e).await
    }
    async fn dispatch(&self, token: &SecretToken, r: DispatchRequest) -> DeliveryReport {
        let Ok(prep) = decode_json::<Preparation>(&r.execution_base) else {
            return DeliveryReport::unknown();
        };
        let Ok(p) = decode(&r.command) else {
            return DeliveryReport::unknown();
        };
        if !matches_command(&prep, &r.command)
            || prep.actor != r.account.actor_id
            || prep.epoch != r.account.authorization_epoch
        {
            return DeliveryReport::unknown();
        }
        if !self.grants.consume(&r.account, &p.request) {
            return DeliveryReport {
                outcome: declined(prep, PullCreationReason::GrantExpired)
                    .map(DeliveryOutcome::Conflict)
                    .unwrap_or(DeliveryOutcome::Unknown),
                ..DeliveryReport::unknown()
            };
        }
        let Ok(url) = Self::route(&prep.frame).and_then(|r| self.http.endpoint(&r)) else {
            return DeliveryReport::unknown();
        };
        let bytes=serde_json::to_vec(&serde_json::json!({"title":p.values.title,"body":p.values.body,"head":p.values.source_branch,"base":p.values.base_branch,"draft":p.values.is_draft,"maintainer_can_modify":false})).unwrap_or_default();
        let response = match self
            .http
            .mutate_native(token, reqwest::Method::POST, url, bytes)
            .await
        {
            Ok(v) => v,
            Err(e) => {
                return DeliveryReport {
                    retry_after_seconds: e.retry_after_seconds,
                    account_cooldown_seconds: e.account_cooldown_seconds,
                    provider_error: Some(e),
                    ..DeliveryReport::unknown()
                };
            }
        };
        let mut report = DeliveryReport {
            provider_error: response.provider_error,
            account_cooldown_seconds: response.cooldown_seconds,
            ..DeliveryReport::unknown()
        };
        if response.status == reqwest::StatusCode::CREATED
            && let Ok(receipt) = receipt::parse(&r.account, &p, &prep.frame, &response.body)
        {
            let e = ReceiptEvidence {
                preparation: prep,
                receipt,
            };
            if let Ok(bytes) = encode(&e) {
                let proof = OperationEvidence {
                    kind: PROOF.into(),
                    version: 1,
                    payload: bytes,
                };
                if self.validate_evidence(&r.command, EvidencePurpose::Confirmed, &proof) {
                    report.outcome = DeliveryOutcome::Confirmed(proof);
                }
            }
        }
        report
    }
    async fn reconcile(
        &self,
        _: &SecretToken,
        _: ReconcileRequest,
    ) -> Result<DeliveryReport, ProviderError> {
        Ok(DeliveryReport::unknown())
    }
}
#[async_trait]
impl crate::command_recovery::policy::CommandRecoveryPolicy for GithubPullCreationPolicy {
    fn instance_id(&self) -> &str {
        "github:https://github.com/"
    }
    fn operation_kind(&self) -> &'static str {
        OPERATION
    }
    fn payload_version(&self) -> u32 {
        1
    }
    async fn review_in(
        &self,
        _: &mut Transaction<'_, Sqlite>,
        c: &DeliveryCommand,
        _: &RemoteAccount,
    ) -> Result<crate::command_recovery::policy::NativeRecoveryReview, CollaborationError> {
        use crate::command_recovery::*;
        let p = decode(c)?;
        Ok(policy::NativeRecoveryReview{fields:[(CommandReviewField::Title,p.values.title),(CommandReviewField::Body,p.values.body)].into_iter().map(|(field,text)|CommandFieldReview{field,base:CommandFieldValue{known:true,value:None},remote:CommandFieldValue{known:false,value:None},desired:CommandFieldValue{known:true,value:Some(text)},comparison:CommandFieldComparison::Unknown,editable:false}).collect(),can_replace:false,reason:Some("Creation requires a new online preview. A lost creation response cannot be proved by matching branches, title or time; Gitru never automatically posts it again.".into()),fence:vec![]})
    }
    async fn replace_in(
        &self,
        _: &mut Transaction<'_, Sqlite>,
        _: &DeliveryCommand,
        _: &RemoteAccount,
        _: &crate::CommandRecoveryReplaceRequest,
        _: &crate::command_recovery::policy::NativeRecoveryReview,
    ) -> Result<crate::storage::command_admission::CommandReceipt, CollaborationError> {
        Err(invalid())
    }
}
