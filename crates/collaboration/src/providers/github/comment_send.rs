//! One attempt per durable creation. Only its validated 201 receipt confirms.
use super::*;
use crate::{
    comment_send::{native as n, *},
    delivery::*,
    storage::comment_send as store,
};
use sqlx::{Sqlite, Transaction};
use std::sync::Arc;
pub(crate) struct GithubCommentPolicy {
    http: GithubHttp,
}
impl GithubCommentPolicy {
    fn new() -> Result<Self, CollaborationError> {
        Ok(Self {
            http: GithubHttp::new()?,
        })
    }
    #[cfg(test)]
    pub(crate) fn for_test_base(url: reqwest::Url) -> Self {
        Self {
            http: GithubHttp::for_test_base(url).unwrap(),
        }
    }
    fn route(f: &n::Frame) -> Result<String, ProviderError> {
        if f.repository
            .provider_id
            .parse::<u64>()
            .ok()
            .is_none_or(|id| id == 0 || id.to_string() != f.repository.provider_id)
            || !valid_repository_path(&f.repository.full_name)
            || f.subject.number.as_deref().is_none_or(|n| {
                n.parse::<u64>()
                    .ok()
                    .is_none_or(|v| v == 0 || v.to_string() != n)
            })
        {
            return Err(resource_details::invalid());
        }
        Ok(format!(
            "repositories/{}/issues/{}/comments",
            f.repository.provider_id,
            f.subject.number.as_deref().unwrap_or_default()
        ))
    }
    fn proof(e: &n::ReceiptEvidence) -> Result<OperationEvidence, CollaborationError> {
        Ok(OperationEvidence {
            kind: "github.comment_created".into(),
            version: 1,
            payload: n::encode(e)?,
        })
    }
    fn parse_created(
        account: &RemoteAccount,
        p: &n::Payload,
        frame: &n::Frame,
        bytes: &[u8],
    ) -> Result<CreatedCommentReceipt, ProviderError> {
        let v: serde_json::Value =
            serde_json::from_slice(bytes).map_err(|_| resource_details::invalid())?;
        let bad = resource_details::invalid;
        let id = v
            .get("id")
            .and_then(|v| v.as_u64())
            .filter(|v| *v > 0)
            .ok_or_else(bad)?
            .to_string();
        let text = |name: &str| v.get(name).and_then(|v| v.as_str()).ok_or_else(bad);
        let parent = format!(
            "https://api.github.com/repos/{}/issues/{}",
            frame.repository.full_name, p.number
        );
        let api = format!(
            "https://api.github.com/repos/{}/issues/comments/{id}",
            frame.repository.full_name
        );
        let web = text("html_url")?;
        let issues = format!(
            "https://github.com/{}/issues/{}#issuecomment-{id}",
            frame.repository.full_name, p.number
        );
        let pull = format!(
            "https://github.com/{}/pull/{}#issuecomment-{id}",
            frame.repository.full_name, p.number
        );
        if text("issue_url")? != parent
            || text("url")? != api
            || web != issues && web != pull
            || text("body")? != p.body
            || v.pointer("/user/id")
                .and_then(|v| v.as_u64())
                .map(|v| v.to_string())
                .as_deref()
                != Some(account.actor_id.as_str())
        {
            return Err(bad());
        }
        let author = v
            .pointer("/user/login")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty() && s.len() <= 256 && !s.chars().any(char::is_control))
            .ok_or_else(bad)?;
        let created =
            chrono::DateTime::parse_from_rfc3339(text("created_at")?).map_err(|_| bad())?;
        let updated =
            chrono::DateTime::parse_from_rfc3339(text("updated_at")?).map_err(|_| bad())?;
        if updated < created {
            return Err(bad());
        }
        Ok(CreatedCommentReceipt {
            command_id: p.request.command_id.clone(),
            draft_generation: p.request.draft_generation.clone(),
            provider_id: id,
            url: web.into(),
            body: p.body.clone(),
            author: author.into(),
            created_at: created
                .with_timezone(&chrono::Utc)
                .to_rfc3339_opts(chrono::SecondsFormat::Nanos, true),
            observed_at: chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Nanos, true),
        })
    }
}
impl ProviderRegistry {
    pub fn register_github_comments(&mut self) -> Result<(), CollaborationError> {
        let p = Arc::new(GithubCommentPolicy::new()?);
        let instance = ProviderInstance::public(ProviderKind::Github);
        self.register_delivery(&instance, p.clone())?;
        self.register_recovery(&instance, p)
    }
}
#[async_trait]
impl CommandDeliveryPolicy for GithubCommentPolicy {
    fn operation_kind(&self) -> &'static str {
        n::OPERATION
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
        let f: n::Frame =
            n::decode_json(&r.native_context).map_err(|_| resource_details::invalid())?;
        let payload = n::decode(&r.command).map_err(|_| resource_details::invalid())?;
        n::validate_dispatch_budget(&f, &payload.body).map_err(|_| resource_details::invalid())?;
        let resource = if f.subject.kind == RemoteItemKind::PullRequest {
            "pulls"
        } else {
            "issues"
        };
        Self::route(&f)?;
        let route = format!(
            "repositories/{}/{resource}/{}",
            f.repository.provider_id,
            f.subject.number.as_deref().unwrap_or_default()
        );
        let response = self
            .http
            .get_point(self.http.endpoint(&route)?, token)
            .await?;
        let request = DetailRequest {
            account: r.account.clone(),
            repository: f.repository.clone(),
            subject: f.subject.clone(),
            facet: DetailFacet::Body,
            cursor: None,
            etag: None,
            source: None,
        };
        let normalized = if f.subject.kind == RemoteItemKind::PullRequest {
            pull_details::normalize(&request, &response.body, "github/pull-detail/2026-03-10")
        } else {
            issue_details::normalize(&request, &response.body, "github/issue-detail/2026-03-10")
        };
        normalized.map_err(|mut e| {
            e.account_cooldown_seconds = response.cooldown_seconds;
            e
        })?;
        let preparation = n::Preparation {
            frame: f,
            actor: r.account.actor_id.clone(),
            epoch: r.account.authorization_epoch.clone(),
            command_hash: n::command_hash(&r.command),
        };
        Ok(DeliveryPreparation {
            bytes: n::encode(&preparation).map_err(|_| {
                let mut error = resource_details::invalid();
                error.account_cooldown_seconds = response.cooldown_seconds;
                error
            })?,
            account_cooldown_seconds: response.cooldown_seconds,
        })
    }
    async fn validate_claim(
        &self,
        tx: &mut Transaction<'_, Sqlite>,
        c: &DeliveryCommand,
        a: &RemoteAccount,
        bytes: &[u8],
    ) -> Result<ClaimDecision, CollaborationError> {
        let p: n::Preparation = n::decode_json(bytes)?;
        if !matches_command(&p, c) || p.actor != a.actor_id || p.epoch != a.authorization_epoch {
            return Err(n::invalid());
        }
        store::validate_frame_in(tx, a, &p.frame).await?;
        let payload = n::decode(c)?;
        n::validate_dispatch_budget(&p.frame, &payload.body)?;
        Ok(ClaimDecision::Ready(bytes.to_vec()))
    }
    fn validate_evidence(
        &self,
        c: &DeliveryCommand,
        purpose: EvidencePurpose,
        proof: &OperationEvidence,
    ) -> bool {
        if purpose != EvidencePurpose::Confirmed
            || proof.kind != "github.comment_created"
            || proof.version != 1
            || !proof.bounded()
            || c.attempt_count == 0
        {
            return false;
        }
        let Ok(e) = n::decode_json::<n::ReceiptEvidence>(&proof.payload) else {
            return false;
        };
        let Ok(payload) = n::decode(c) else {
            return false;
        };
        matches_command(&e.preparation, c) && n::receipt_matches(&e, &payload)
    }
    async fn finalize_in(
        &self,
        context: &mut crate::storage::effective::finalization::DeliveryFinalization<'_, '_>,
        c: &DeliveryCommand,
        purpose: EvidencePurpose,
        proof: &OperationEvidence,
    ) -> Result<(), CollaborationError> {
        if purpose != EvidencePurpose::Confirmed {
            return Err(n::invalid());
        }
        let e: n::ReceiptEvidence = n::decode_json(&proof.payload)?;
        store::finalize_in(context.transaction(), c, &e).await
    }
    async fn dispatch(&self, token: &SecretToken, r: DispatchRequest) -> DeliveryReport {
        let Ok(prep) = n::decode_json::<n::Preparation>(&r.execution_base) else {
            return DeliveryReport::unknown();
        };
        let Ok(p) = n::decode(&r.command) else {
            return DeliveryReport::unknown();
        };
        if !matches_command(&prep, &r.command)
            || prep.actor != r.account.actor_id
            || prep.epoch != r.account.authorization_epoch
        {
            return DeliveryReport::unknown();
        }
        let Ok(route) = Self::route(&prep.frame) else {
            return DeliveryReport::unknown();
        };
        let Ok(url) = self.http.endpoint(&route) else {
            return DeliveryReport::unknown();
        };
        let bytes = serde_json::to_vec(&serde_json::json!({"body":p.body})).unwrap_or_default();
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
            && let Ok(receipt) = Self::parse_created(&r.account, &p, &prep.frame, &response.body)
        {
            let e = n::ReceiptEvidence {
                preparation: prep,
                receipt,
            };
            if let Ok(proof) = Self::proof(&e)
                && self.validate_evidence(&r.command, EvidencePurpose::Confirmed, &proof)
            {
                report.outcome = DeliveryOutcome::Confirmed(proof)
            }
        }
        report
    }
    // No GET/list heuristic can prove a lost creation's causality. The generic
    // worker caps these read-only turns and retains unknown intent for review.
    async fn reconcile(
        &self,
        _: &SecretToken,
        _: ReconcileRequest,
    ) -> Result<DeliveryReport, ProviderError> {
        Ok(DeliveryReport::unknown())
    }
}
fn matches_command(p: &n::Preparation, c: &DeliveryCommand) -> bool {
    let Ok(payload) = n::decode(c) else {
        return false;
    };
    p.command_hash == n::command_hash(c)
        && p.frame.subject.account_id == c.account_id
        && p.frame.repository.account_id == c.account_id
        && p.frame.subject.id == c.target_id
        && p.frame.repository.id == c.repository_id.as_deref().unwrap_or_default()
        && p.frame.repository.provider_id == payload.repository_native
        && p.frame.subject.provider_id == payload.subject_native
        && p.frame.subject.number.as_deref() == Some(payload.number.as_str())
        && ((p.frame.subject.kind == RemoteItemKind::PullRequest
            && c.target_kind == "pull_request")
            || (p.frame.subject.kind == RemoteItemKind::Issue && c.target_kind == "issue"))
}
#[async_trait]
impl crate::command_recovery::policy::CommandRecoveryPolicy for GithubCommentPolicy {
    fn instance_id(&self) -> &str {
        "github:https://github.com/"
    }
    fn operation_kind(&self) -> &'static str {
        n::OPERATION
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
        let p = n::decode(c)?;
        Ok(policy::NativeRecoveryReview{fields:vec![CommandFieldReview{field:CommandReviewField::Body,base:CommandFieldValue{known:true,value:None},remote:CommandFieldValue{known:false,value:None},desired:CommandFieldValue{known:true,value:Some(p.body)},comparison:CommandFieldComparison::Unknown,editable:false}],can_replace:false,reason:Some("A lost comment response cannot be proved by matching text or time. The saved draft is retained; this creation will never be automatically posted again.".into()),fence:vec![]})
    }
    async fn replace_in(
        &self,
        _: &mut Transaction<'_, Sqlite>,
        _: &DeliveryCommand,
        _: &RemoteAccount,
        _: &crate::CommandRecoveryReplaceRequest,
        _: &crate::command_recovery::policy::NativeRecoveryReview,
    ) -> Result<crate::storage::command_admission::CommandReceipt, CollaborationError> {
        Err(n::invalid())
    }
}
