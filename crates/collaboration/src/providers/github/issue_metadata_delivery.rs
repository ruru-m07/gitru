//! Metadata creation v2: one fresh identity read per native delivery turn.
use super::*;
use crate::{
    delivery::*,
    issue_metadata::{native as n, *},
    storage::issue_metadata as store,
};
use serde::{Deserialize, Serialize};
use sqlx::{Sqlite, Transaction};
use std::sync::Arc;
pub(crate) struct GithubIssueMetadataCreationPolicy {
    provider: GithubProvider,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Progress {
    frame: n::FrameV2,
    actor: String,
    epoch: String,
    hash: String,
    next: usize,
    push: Option<bool>,
}
#[derive(Serialize, Deserialize)]
#[serde(
    tag = "state",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
enum Prepared {
    Ready(n::PreparationV2),
    Declined(n::DeclinedV2),
}
fn invalid() -> ProviderError {
    ProviderError::new(ProviderErrorKind::InvalidResponse)
}
fn observed(mut e: ProviderError, cooldown: Option<u64>) -> ProviderError {
    e.account_cooldown_seconds = e.account_cooldown_seconds.into_iter().chain(cooldown).max();
    e
}
fn hash(c: &DeliveryCommand) -> String {
    c.hash.iter().map(|b| format!("{b:02x}")).collect()
}
fn matches(p: &n::PreparationV2, c: &DeliveryCommand) -> bool {
    store::decode_command(c)
        .is_ok_and(|v| n::preparation_matches(p, &v) && p.command_hash == hash(c))
}
fn points(p: &n::PayloadV2) -> Vec<IssueMetadataPoint> {
    let v = p.metadata.public();
    let mut steps = vec![IssueMetadataPoint::Repository];
    steps.extend(v.labels.into_iter().map(IssueMetadataPoint::Label));
    for a in v.assignees {
        steps.push(IssueMetadataPoint::AssigneeIdentity(a.clone()));
        steps.push(IssueMetadataPoint::AssigneeAssignable(a));
    }
    if let Some(m) = v.milestone {
        steps.push(IssueMetadataPoint::Milestone(m));
    }
    steps
}
impl GithubIssueMetadataCreationPolicy {
    #[cfg(test)]
    pub(crate) fn for_test_base(url: reqwest::Url) -> Self {
        Self {
            provider: GithubProvider::for_test_base(url),
        }
    }
}
impl ProviderRegistry {
    pub fn register_github_issue_metadata_creation(&mut self) -> Result<(), CollaborationError> {
        let p = Arc::new(GithubIssueMetadataCreationPolicy {
            provider: GithubProvider::new()?,
        });
        let instance = ProviderInstance::public(ProviderKind::Github);
        self.register_delivery(&instance, p.clone())?;
        self.register_recovery(&instance, p)
    }
}
#[async_trait]
impl CommandDeliveryPolicy for GithubIssueMetadataCreationPolicy {
    fn operation_kind(&self) -> &'static str {
        "github.create_issue"
    }
    fn payload_version(&self) -> u32 {
        2
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
        _: &SecretToken,
        _: &ReconcileRequest,
    ) -> Result<DeliveryPreparation, ProviderError> {
        Err(invalid())
    }
    async fn prepare_step(
        &self,
        token: &SecretToken,
        r: &ReconcileRequest,
        continuation: Option<&[u8]>,
    ) -> Result<PreparationStep, ProviderError> {
        let p = store::decode_command(&r.command).map_err(|_| invalid())?;
        let frame: n::FrameV2 = n::decode_json(&r.native_context).map_err(|_| invalid())?;
        let mut progress = if let Some(bytes) = continuation {
            n::decode_json::<Progress>(bytes).map_err(|_| invalid())?
        } else {
            Progress {
                frame: frame.clone(),
                actor: r.account.actor_id.clone(),
                epoch: r.account.authorization_epoch.clone(),
                hash: hash(&r.command),
                next: 0,
                push: None,
            }
        };
        if progress.frame != frame
            || progress.actor != r.account.actor_id
            || progress.epoch != r.account.authorization_epoch
            || progress.hash != hash(&r.command)
        {
            return Err(invalid());
        }
        let steps = points(&p);
        let point = steps.get(progress.next).ok_or_else(invalid)?.clone();
        let expected =
            store::expected_preparation(&r.account, frame.clone(), &p, progress.hash.clone());
        n::validate_receipt_budget(&p, &expected).map_err(|_| invalid())?;
        let repository = RemoteRepository {
            id: frame.repository_id.clone(),
            account_id: r.account.id.clone(),
            provider_id: frame.repository_native.clone(),
            full_name: frame.repository_path.clone(),
            description: None,
            default_branch: None,
            web_url: format!("https://github.com/{}", frame.repository_path),
            name: frame
                .repository_path
                .rsplit('/')
                .next()
                .unwrap_or_default()
                .into(),
            selected: true,
        };
        let read = self
            .provider
            .issue_metadata_point(
                token,
                IssueMetadataPointRequest {
                    context: IssueMetadataReadContext {
                        account: r.account.clone(),
                        repository,
                        authorization_view: frame.authorization_view.clone(),
                    },
                    point,
                },
            )
            .await?;
        let declined = match read.value {
            IssueMetadataPointValue::Repository { metadata_access } => {
                progress.push = match metadata_access {
                    IssueMetadataAvailability::Available => Some(true),
                    IssueMetadataAvailability::Unavailable => Some(false),
                    IssueMetadataAvailability::Unknown => None,
                };
                (!p.metadata.is_empty() && progress.push != Some(true))
                    .then_some(n::DeclineReasonV2::MetadataPermission)
            }
            IssueMetadataPointValue::Selection {
                identity_matches,
                availability,
                ..
            } => {
                if !identity_matches {
                    Some(n::DeclineReasonV2::SelectionChanged)
                } else {
                    (availability == IssueMetadataAvailability::Unavailable)
                        .then_some(n::DeclineReasonV2::SelectionUnavailable)
                }
            }
            IssueMetadataPointValue::Assignable => None,
        };
        let encode =
            |v: &Prepared| n::encode(v).map_err(|_| observed(invalid(), read.cooldown_seconds));
        if let Some(reason) = declined {
            return Ok(PreparationStep::Complete(DeliveryPreparation {
                bytes: encode(&Prepared::Declined(n::DeclinedV2 {
                    frame,
                    actor: progress.actor,
                    epoch: progress.epoch,
                    command_hash: progress.hash,
                    reason,
                }))?,
                account_cooldown_seconds: read.cooldown_seconds,
            }));
        }
        progress.next += 1;
        let (bytes, complete) = if progress.next == steps.len() {
            let ready = n::PreparationV2 {
                frame,
                actor: progress.actor,
                epoch: progress.epoch,
                command_hash: progress.hash,
                revalidated_metadata: p.metadata,
                metadata_push_access: progress.push,
            };
            (encode(&Prepared::Ready(ready))?, true)
        } else {
            (
                n::encode(&progress).map_err(|_| observed(invalid(), read.cooldown_seconds))?,
                false,
            )
        };
        let d = DeliveryPreparation {
            bytes,
            account_cooldown_seconds: read.cooldown_seconds,
        };
        Ok(if complete {
            PreparationStep::Complete(d)
        } else {
            PreparationStep::Continue(d)
        })
    }
    async fn validate_claim(
        &self,
        tx: &mut Transaction<'_, Sqlite>,
        c: &DeliveryCommand,
        a: &RemoteAccount,
        bytes: &[u8],
    ) -> Result<ClaimDecision, CollaborationError> {
        let p = store::decode_command(c)?;
        let f = store::frame_in(tx, a, &c.target_id).await?;
        match n::decode_json::<Prepared>(bytes)? {
            Prepared::Ready(e) => {
                if !matches(&e, c)
                    || e.actor != a.actor_id
                    || e.epoch != a.authorization_epoch
                    || e.frame != f
                {
                    return Err(n::invalid());
                }
                n::validate_receipt_budget(&p, &e)?;
                Ok(ClaimDecision::Ready(n::encode(&e)?))
            }
            Prepared::Declined(e) => {
                if !n::declined_matches(&e, &p)
                    || e.command_hash != hash(c)
                    || e.actor != a.actor_id
                    || e.epoch != a.authorization_epoch
                    || e.frame != f
                {
                    return Err(n::invalid());
                }
                Ok(ClaimDecision::Conflict(OperationEvidence {
                    kind: "github.issue_creation_declined".into(),
                    version: 2,
                    payload: n::encode(&e)?,
                }))
            }
        }
    }
    fn validate_evidence(
        &self,
        c: &DeliveryCommand,
        purpose: EvidencePurpose,
        proof: &OperationEvidence,
    ) -> bool {
        let Ok(p) = store::decode_command(c) else {
            return false;
        };
        if proof.version != 2 || !proof.bounded() {
            return false;
        }
        match purpose {
            EvidencePurpose::Confirmed
                if proof.kind == "github.issue_created" && c.attempt_count > 0 =>
            {
                n::decode_json::<n::ReceiptV2>(&proof.payload)
                    .is_ok_and(|e| matches(&e.preparation, c) && n::receipt_matches(&e, &p))
            }
            EvidencePurpose::Conflict
                if proof.kind == "github.issue_creation_declined" && c.attempt_count == 0 =>
            {
                n::decode_json::<n::DeclinedV2>(&proof.payload)
                    .is_ok_and(|e| e.command_hash == hash(c) && n::declined_matches(&e, &p))
            }
            _ => false,
        }
    }
    async fn finalize_in(
        &self,
        context: &mut crate::storage::effective::finalization::DeliveryFinalization<'_, '_>,
        c: &DeliveryCommand,
        purpose: EvidencePurpose,
        proof: &OperationEvidence,
    ) -> Result<(), CollaborationError> {
        if !self.validate_evidence(c, purpose, proof) {
            return Err(n::invalid());
        }
        if purpose == EvidencePurpose::Confirmed {
            store::finalize_in(context.transaction(), c, &n::decode_json(&proof.payload)?).await?;
        }
        Ok(())
    }
    async fn dispatch(&self, token: &SecretToken, r: DispatchRequest) -> DeliveryReport {
        let Ok(p) = store::decode_command(&r.command) else {
            return DeliveryReport::unknown();
        };
        let Ok(prep) = n::decode_json::<n::PreparationV2>(&r.execution_base) else {
            return DeliveryReport::unknown();
        };
        if !matches(&prep, &r.command)
            || prep.actor != r.account.actor_id
            || prep.epoch != r.account.authorization_epoch
        {
            return DeliveryReport::unknown();
        }
        let Ok(body) = n::request_bytes(&p) else {
            return DeliveryReport::unknown();
        };
        let Ok(url) = self.provider.http.endpoint(&format!(
            "repositories/{}/issues",
            prep.frame.repository_native
        )) else {
            return DeliveryReport::unknown();
        };
        let response = match self
            .provider
            .http
            .mutate_native(token, reqwest::Method::POST, url, body)
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
        if let Ok(e) = issue_metadata::receipt::parse_created(
            response.status.as_u16(),
            &p,
            &prep,
            &response.body,
        ) && let Ok(payload) = n::encode(&e)
        {
            let proof = OperationEvidence {
                kind: "github.issue_created".into(),
                version: 2,
                payload,
            };
            if self.validate_evidence(&r.command, EvidencePurpose::Confirmed, &proof) {
                report.outcome = DeliveryOutcome::Confirmed(proof);
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
impl crate::command_recovery::policy::CommandRecoveryPolicy for GithubIssueMetadataCreationPolicy {
    fn instance_id(&self) -> &str {
        "github:https://github.com/"
    }
    fn operation_kind(&self) -> &'static str {
        "github.create_issue"
    }
    fn payload_version(&self) -> u32 {
        2
    }
    async fn review_in(
        &self,
        _: &mut Transaction<'_, Sqlite>,
        c: &DeliveryCommand,
        _: &RemoteAccount,
    ) -> Result<crate::command_recovery::policy::NativeRecoveryReview, CollaborationError> {
        use crate::command_recovery::*;
        let p = store::decode_command(c)?;
        Ok(policy::NativeRecoveryReview{fields:[(CommandReviewField::Title,p.title),(CommandReviewField::Body,p.body)].into_iter().map(|(field,text)|CommandFieldReview{field,base:CommandFieldValue{known:true,value:None},remote:CommandFieldValue{known:false,value:None},desired:CommandFieldValue{known:true,value:Some(text)},comparison:CommandFieldComparison::Unknown,editable:false}).collect(),can_replace:false,reason:Some("A lost creation response cannot prove whether this issue exists. Its authored metadata is retained; Gitru will not post it again or automatically repair optional fields.".into()),fence:vec![]})
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
