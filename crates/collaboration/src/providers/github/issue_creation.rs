//! One attempt per durable creation. Only its validated 201 receipt confirms.
use super::*;
use crate::{delivery::*, issue_creation::native as n, storage::issue_creation as store};
use sqlx::{Sqlite, Transaction};
use std::sync::Arc;
pub(crate) struct GithubIssueCreationPolicy {
    http: GithubHttp,
}
impl GithubIssueCreationPolicy {
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
            .is_none_or(|n| n == 0 || n.to_string() != f.repository.provider_id)
            || !valid_repository_path(&f.repository.full_name)
        {
            return Err(resource_details::invalid());
        }
        Ok(format!("repositories/{}/issues", f.repository.provider_id))
    }
    fn proof(e: &n::ReceiptEvidence) -> Result<OperationEvidence, CollaborationError> {
        Ok(OperationEvidence {
            kind: "github.issue_created".into(),
            version: 1,
            payload: n::encode(e)?,
        })
    }
    fn parse_created(
        account: &RemoteAccount,
        p: &n::Payload,
        frame: &n::Frame,
        bytes: &[u8],
    ) -> Result<n::CreatedReceipt, ProviderError> {
        let v: serde_json::Value =
            serde_json::from_slice(bytes).map_err(|_| resource_details::invalid())?;
        let bad = resource_details::invalid;
        let id = v
            .get("id")
            .and_then(|x| x.as_u64())
            .filter(|n| *n > 0)
            .ok_or_else(bad)?
            .to_string();
        let number = v
            .get("number")
            .and_then(|x| x.as_u64())
            .filter(|n| *n > 0)
            .ok_or_else(bad)?
            .to_string();
        let text = |k: &str| v.get(k).and_then(|x| x.as_str()).ok_or_else(bad);
        let canonical = format!(
            "https://api.github.com/repos/{}/issues/{number}",
            frame.repository.full_name
        );
        let web = format!(
            "https://github.com/{}/issues/{number}",
            frame.repository.full_name
        );
        let body = match v.get("body") {
            Some(serde_json::Value::Null) => None,
            Some(serde_json::Value::String(s)) => Some(s.clone()),
            _ => return Err(bad()),
        };
        if v.get("pull_request").is_some_and(|x| !x.is_null())
            || text("url")? != canonical
            || text("repository_url")?
                != format!(
                    "https://api.github.com/repos/{}",
                    frame.repository.full_name
                )
            || text("html_url")? != web
            || text("title")? != p.title
            || body.as_deref().unwrap_or("") != p.body
            || v.pointer("/user/id")
                .and_then(|x| x.as_u64())
                .map(|n| n.to_string())
                .as_deref()
                != Some(account.actor_id.as_str())
            || !matches!(text("state")?, "open" | "closed")
        {
            return Err(bad());
        }
        let author = v
            .pointer("/user/login")
            .and_then(|x| x.as_str())
            .filter(|s| !s.is_empty() && s.len() <= 256 && !s.chars().any(char::is_control))
            .ok_or_else(bad)?;
        let created =
            chrono::DateTime::parse_from_rfc3339(text("created_at")?).map_err(|_| bad())?;
        let updated =
            chrono::DateTime::parse_from_rfc3339(text("updated_at")?).map_err(|_| bad())?;
        if updated < created {
            return Err(bad());
        }
        let item = RemoteItem {
            id: format!("github:issue:{id}"),
            account_id: account.id.clone(),
            repository_id: Some(frame.repository.id.clone()),
            provider_id: id,
            kind: RemoteItemKind::Issue,
            number: Some(number),
            title: p.title.clone(),
            body,
            body_omitted: false,
            author: Some(author.into()),
            web_url: Some(web),
            state: text("state")?.into(),
            updated_at: updated
                .with_timezone(&chrono::Utc)
                .to_rfc3339_opts(chrono::SecondsFormat::Nanos, true),
            head_oid: None,
            is_draft: None,
            reason: None,
            unread: None,
            native_inbox: None,
        };
        let request = DetailRequest {
            account: account.clone(),
            repository: frame.repository.clone(),
            subject: item.clone(),
            facet: DetailFacet::Body,
            cursor: None,
            etag: None,
            source: None,
        };
        let (_, mut metadata) =
            issue_details::normalize(&request, bytes, "github/issue-detail/2026-03-10")?;
        // These fields were not authored by this operation. Retaining them can
        // consume the causal proof budget after an otherwise valid creation.
        // Omitted keeps normal metadata reads authoritative; it is not known-empty.
        metadata.values.labels.clear();
        metadata.values.assignees.clear();
        metadata.values.milestone = None;
        if let Some(author) = &mut metadata.values.author {
            author.web_url = None;
        }
        for field in &mut metadata.fields {
            if matches!(
                field.field,
                crate::MetadataField::Labels
                    | crate::MetadataField::Assignees
                    | crate::MetadataField::Milestone
            ) {
                field.state = DetailValueState::Omitted;
            }
        }
        metadata.values.updated_at = Some(item.updated_at.clone());
        metadata.source.provider_updated_at = Some(item.updated_at.clone());
        Ok(n::CreatedReceipt {
            item,
            metadata: n::ReceiptMetadata::from_observation(metadata),
            created_at: created
                .with_timezone(&chrono::Utc)
                .to_rfc3339_opts(chrono::SecondsFormat::Nanos, true),
        })
    }
}
impl ProviderRegistry {
    pub fn register_github_issue_creation(&mut self) -> Result<(), CollaborationError> {
        let p = Arc::new(GithubIssueCreationPolicy::new()?);
        let instance = ProviderInstance::public(ProviderKind::Github);
        self.register_delivery(&instance, p.clone())?;
        self.register_recovery(&instance, p)
    }
}
#[async_trait]
impl CommandDeliveryPolicy for GithubIssueCreationPolicy {
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
        n::validate_dispatch_budget(&f, &payload.title, &payload.body)
            .map_err(|_| resource_details::invalid())?;
        Self::route(&f)?;
        let response = self
            .http
            .get_point(
                self.http
                    .endpoint(&format!("repositories/{}", f.repository.provider_id))?,
                token,
            )
            .await?;
        let result = (|| {
            let v: serde_json::Value =
                serde_json::from_slice(&response.body).map_err(|_| resource_details::invalid())?;
            if response.not_modified
                || response.next_url.is_some()
                || v.get("id")
                    .and_then(|v| v.as_u64())
                    .map(|n| n.to_string())
                    .as_deref()
                    != Some(f.repository.provider_id.as_str())
                || v.get("full_name").and_then(|v| v.as_str())
                    != Some(f.repository.full_name.as_str())
                || v.get("has_issues").and_then(|v| v.as_bool()) != Some(true)
                || v.get("archived").and_then(|v| v.as_bool()) != Some(false)
            {
                return Err(resource_details::invalid());
            }
            Ok(())
        })();
        result.map_err(|mut e| {
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
        n::validate_dispatch_budget(&p.frame, &payload.title, &payload.body)?;
        Ok(ClaimDecision::Ready(bytes.to_vec()))
    }
    fn validate_evidence(
        &self,
        c: &DeliveryCommand,
        purpose: EvidencePurpose,
        proof: &OperationEvidence,
    ) -> bool {
        if purpose != EvidencePurpose::Confirmed
            || proof.kind != "github.issue_created"
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
        let bytes = serde_json::to_vec(&serde_json::json!({"title":p.title,"body":p.body}))
            .unwrap_or_default();
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
        && p.frame.repository.account_id == c.account_id
        && p.frame.repository.id == c.target_id
        && p.frame.repository.id == c.repository_id.as_deref().unwrap_or_default()
        && p.frame.repository.provider_id == payload.repository_native
        && c.target_kind == "repository"
}
#[async_trait]
impl crate::command_recovery::policy::CommandRecoveryPolicy for GithubIssueCreationPolicy {
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
        Ok(policy::NativeRecoveryReview{fields:[(CommandReviewField::Title,p.title),(CommandReviewField::Body,p.body)].into_iter().map(|(field,text)|CommandFieldReview{field,base:CommandFieldValue{known:true,value:None},remote:CommandFieldValue{known:false,value:None},desired:CommandFieldValue{known:true,value:Some(text)},comparison:CommandFieldComparison::Unknown,editable:false}).collect(),can_replace:false,reason:Some("A lost issue creation response cannot be proved by matching title, body or time. The saved draft is retained and this creation will never be automatically posted again.".into()),fence:vec![]})
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
