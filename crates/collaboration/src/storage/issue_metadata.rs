//! Authored v2 selections and causal issue publication. Catalogs are advisory.
use super::issue_creation as old;
use super::*;
use crate::issue_creation::*;
use crate::{
    commands::*,
    delivery::DeliveryCommand,
    issue_metadata::{native as n, *},
};
use command_admission::{CommandAdmissionPolicy, CommandProtection};

pub(crate) async fn selection_in(
    tx: &mut Transaction<'_, Sqlite>,
    account: &str,
    draft: &str,
) -> Result<n::SelectionV2> {
    let raw: Option<String> = sqlx::query_scalar(
        "SELECT metadata_json FROM issue_draft_metadata WHERE account_id=? AND draft_id=?",
    )
    .bind(account)
    .bind(draft)
    .fetch_optional(&mut **tx)
    .await
    .map_err(storage_error)?;
    let selected = raw
        .map(|s| n::decode_json::<n::SelectionV2>(s.as_bytes()))
        .transpose()?
        .unwrap_or_default();
    selected.validate()?;
    Ok(selected)
}
pub(crate) async fn require_empty_in(
    tx: &mut Transaction<'_, Sqlite>,
    account: &str,
    draft: &str,
) -> Result<()> {
    if !selection_in(tx, account, draft).await?.is_empty() {
        return Err(CollaborationError::invalid(
            "Use the metadata draft editor to preserve this draft's selections",
        ));
    }
    Ok(())
}
pub(crate) fn validate_save(r: &SaveIssueDraftV2Request) -> Result<()> {
    old::validate_save(&SaveIssueDraftRequest {
        account_id: r.account_id.clone(),
        draft_id: r.draft_id.clone(),
        repository_id: r.repository_id.clone(),
        authorization_epoch: r.authorization_epoch.clone(),
        authorization_view: r.authorization_view.clone(),
        expected_generation: r.expected_generation.clone(),
        title: r.title.clone(),
        body: r.body.clone(),
    })?;
    n::SelectionV2::from_public(r.metadata.clone())
        .and_then(|v| n::encode(&v))
        .map(|_| ())
}
pub(crate) fn validate_send(r: &SubmitIssueV2Request) -> Result<()> {
    crate::issue_creation::native::validate_send(&SubmitIssueRequest {
        context: r.context.clone(),
        draft_id: r.draft_id.clone(),
        draft_generation: r.draft_generation.clone(),
        command_id: r.command_id.clone(),
        accept_background_delivery: r.accept_background_delivery,
    })
}
pub(crate) fn decode_command(c: &DeliveryCommand) -> Result<n::PayloadV2> {
    if c.operation_kind != "github.create_issue"
        || c.payload_version != 2
        || c.target_kind != "repository"
        || c.repository_id.as_deref() != Some(&c.target_id)
    {
        return Err(n::invalid());
    }
    n::decode_payload(
        &c.payload,
        &c.account_id,
        &c.command_id,
        &c.target_id,
        &c.authorization_epoch,
    )
}
fn decode_submission(s: &CommandSubmission) -> Result<n::PayloadV2> {
    if s.operation().kind() != "github.create_issue" || s.operation().payload_version() != 2 {
        return Err(n::invalid());
    }
    n::decode_payload(
        s.payload_bytes(),
        s.account_id(),
        s.command_id(),
        s.target().id(),
        s.authorization_epoch(),
    )
}
pub(crate) async fn frame_in(
    tx: &mut Transaction<'_, Sqlite>,
    a: &RemoteAccount,
    repo: &str,
) -> Result<n::FrameV2> {
    let f = old::capture_in(tx, a, repo).await?;
    Ok(n::FrameV2 {
        account_id: a.id.clone(),
        repository_id: repo.into(),
        repository_native: f.repository.provider_id,
        repository_path: f.repository.full_name,
        authorization_view: f.authorization_view,
    })
}
fn context(
    a: &RemoteAccount,
    f: &n::FrameV2,
    title: &str,
    body: &str,
    generation: &str,
    selected: &n::SelectionV2,
) -> Result<IssueDraftContext> {
    let mut h = Sha256::new();
    h.update(b"gitru.issue-create-review.v2\0");
    for b in [
        a.actor_id.as_bytes(),
        a.authorization_epoch.as_bytes(),
        n::encode(f)?.as_slice(),
        generation.as_bytes(),
        n::content_hash(title, body, selected)?.as_slice(),
    ] {
        h.update((b.len() as u64).to_be_bytes());
        h.update(b);
    }
    Ok(IssueDraftContext {
        account_id: a.id.clone(),
        repository_id: f.repository_id.clone(),
        authorization_epoch: a.authorization_epoch.clone(),
        authorization_view: f.authorization_view.clone(),
        review_token: format!("{:x}", h.finalize()),
    })
}
async fn snapshot_in(
    tx: &mut Transaction<'_, Sqlite>,
    a: &RemoteAccount,
    key: &IssueDraftKey,
) -> Result<IssueDraftV2Snapshot> {
    let mut draft = old::snapshot_in(tx, a, key).await?;
    let selected = selection_in(tx, &a.id, &key.draft_id).await?;
    if draft.context.is_some() {
        let f = frame_in(tx, a, &key.repository_id).await?;
        draft.context = Some(context(
            a,
            &f,
            &draft.title,
            &draft.body,
            &draft.generation,
            &selected,
        )?);
    }
    let metadata_outcome = if let Some(published) = &draft.published {
        let proof:Option<Vec<u8>>=sqlx::query_scalar("SELECT e.payload FROM delivery_resolutions r JOIN command_evidence e ON e.account_id=r.account_id AND e.command_id=r.command_id AND e.ordinal=r.evidence_ordinal WHERE r.account_id=? AND r.command_id=? AND r.purpose='confirmed' AND e.kind='github.issue_created' AND e.version=2").bind(&a.id).bind(&published.command_id).fetch_optional(&mut **tx).await.map_err(storage_error)?;
        proof
            .map(|b| {
                let e: n::ReceiptV2 = n::decode_json(&b)?;
                e.metadata
                    .outcome(&published.command_id, &e.preparation.revalidated_metadata)
            })
            .transpose()?
    } else {
        None
    };
    Ok(IssueDraftV2Snapshot {
        draft,
        metadata: selected.public(),
        metadata_outcome,
    })
}
fn seal(p: n::PayloadV2) -> Result<CommandSubmission> {
    seal_command(CommandDraft {
        command_id: p.request.command_id.clone(),
        account_id: p.request.context.account_id.clone(),
        authorization_epoch: p.request.context.authorization_epoch.clone(),
        target: CommandTarget::new(
            CommandTargetKind::Repository,
            p.request.context.repository_id.clone(),
            Some(p.request.context.repository_id.clone()),
        )?,
        payload: p,
        guards: vec![],
        dependencies: vec![],
    })
}
pub(crate) fn expected_preparation(
    a: &RemoteAccount,
    f: n::FrameV2,
    p: &n::PayloadV2,
    hash: String,
) -> n::PreparationV2 {
    n::PreparationV2 {
        frame: f,
        actor: a.actor_id.clone(),
        epoch: a.authorization_epoch.clone(),
        command_hash: hash,
        revalidated_metadata: p.metadata.clone(),
        metadata_push_access: (!p.metadata.is_empty()).then_some(true),
    }
}
struct Admission;
#[async_trait::async_trait]
impl CommandAdmissionPolicy for Admission {
    const OPERATION_KIND: &'static str = "github.create_issue";
    const PAYLOAD_VERSION: u32 = 2;
    async fn validate(
        &self,
        tx: &mut Transaction<'_, Sqlite>,
        a: &RemoteAccount,
        s: &CommandSubmission,
    ) -> Result<Vec<CommandProtection>> {
        let p = decode_submission(s)?;
        let f = frame_in(tx, a, s.target().id()).await?;
        let old = old::draft_in(tx, &a.id, &p.request.draft_id, &f.repository_id).await?;
        let selected = selection_in(tx, &a.id, &p.request.draft_id).await?;
        if old.0 != p.title
            || old.1 != p.body
            || old.2.to_string() != p.request.draft_generation
            || selected != p.metadata
            || f.repository_native != p.repository_native
            || context(
                a,
                &f,
                &p.title,
                &p.body,
                &p.request.draft_generation,
                &selected,
            )? != p.request.context
        {
            return Err(stale());
        }
        n::validate_receipt_budget(&p, &expected_preparation(a, f.clone(), &p, "0".repeat(64)))?;
        Ok(vec![CommandProtection::Entity(f.repository_id)])
    }
}
impl Store {
    pub async fn issue_draft_v2(&self, key: IssueDraftKey) -> Result<IssueDraftV2Snapshot> {
        old::validate_key(&key)?;
        let mut tx = self.inner.readers.begin().await.map_err(storage_error)?;
        let a = account_in(&mut tx, &key.account_id, false).await?;
        let s = snapshot_in(&mut tx, &a, &key).await?;
        tx.commit().await.map_err(storage_error)?;
        Ok(s)
    }
    pub(crate) async fn save_issue_draft_v2(
        &self,
        r: SaveIssueDraftV2Request,
    ) -> Result<IssueDraftV2Snapshot> {
        validate_save(&r)?;
        let selected = n::SelectionV2::from_public(r.metadata.clone())?;
        let encoded = String::from_utf8(n::encode(&selected)?).map_err(|_| n::invalid())?;
        let key = IssueDraftKey {
            account_id: r.account_id.clone(),
            draft_id: r.draft_id.clone(),
            repository_id: r.repository_id.clone(),
        };
        let mut writer = self.inner.writer.acquire().await?;
        let mut tx = writer.begin().await.map_err(storage_error)?;
        let a = account_in(&mut tx, &r.account_id, false).await?;
        if a.authorization_epoch != r.authorization_epoch
            || metadata(&mut tx).await?.1 != r.authorization_view
        {
            return Err(stale());
        }
        let old = old::draft_in(&mut tx, &a.id, &r.draft_id, &r.repository_id).await?;
        let previous = selection_in(&mut tx, &a.id, &r.draft_id).await?;
        let expected = crate::issue_creation::native::revision(&r.expected_generation, false)?;
        if old.2 != expected {
            return Err(stale());
        }
        if old.0 != r.title || old.1 != r.body || selected != previous || expected == 0 {
            let next = expected
                .checked_add(1)
                .ok_or_else(CollaborationError::storage)?;
            sqlx::query("INSERT INTO issue_drafts VALUES(?,?,?,?,?,?) ON CONFLICT(account_id,draft_id) DO UPDATE SET title=excluded.title,body=excluded.body,generation=excluded.generation").bind(&a.id).bind(&r.draft_id).bind(&r.repository_id).bind(&r.title).bind(&r.body).bind(next).execute(&mut *tx).await.map_err(storage_error)?;
            sqlx::query("INSERT INTO issue_draft_metadata VALUES(?,?,?) ON CONFLICT(account_id,draft_id) DO UPDATE SET metadata_json=excluded.metadata_json").bind(&a.id).bind(&r.draft_id).bind(encoded).execute(&mut *tx).await.map_err(storage_error)?;
            record_change(
                &mut tx,
                &a.id,
                positive_revision(&a.authorization_epoch)?,
                &format!("issue_draft:{}", r.draft_id),
                false,
            )
            .await?;
        }
        let s = snapshot_in(&mut tx, &a, &key).await?;
        tx.commit().await.map_err(storage_error)?;
        Ok(s)
    }
    pub(crate) async fn submit_issue_v2(
        &self,
        r: SubmitIssueV2Request,
    ) -> Result<IssueSubmissionReceipt> {
        validate_send(&r)?;
        let mut writer = self.inner.writer.acquire().await?;
        let mut tx = writer.begin().await.map_err(storage_error)?;
        epoch_in(
            &mut tx,
            &r.context.account_id,
            &r.context.authorization_epoch,
        )
        .await?;
        let duplicate: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM commands WHERE account_id=? AND command_id=?)",
        )
        .bind(&r.context.account_id)
        .bind(&r.command_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(storage_error)?;
        let s = if duplicate {
            let c = delivery::load_in(&mut tx, &r.context.account_id, &r.command_id).await?;
            let p = decode_command(&c)?;
            if p.request != r {
                return Err(n::invalid());
            }
            seal(p)?
        } else {
            let a = account_in(&mut tx, &r.context.account_id, true).await?;
            let snapshot = snapshot_in(
                &mut tx,
                &a,
                &IssueDraftKey {
                    account_id: a.id.clone(),
                    draft_id: r.draft_id.clone(),
                    repository_id: r.context.repository_id.clone(),
                },
            )
            .await?;
            if snapshot.draft.context.as_ref() != Some(&r.context)
                || snapshot.draft.generation != r.draft_generation
                || snapshot.draft.availability != IssueDraftAvailability::Available
            {
                return Err(stale());
            }
            let f = frame_in(&mut tx, &a, &r.context.repository_id).await?;
            record_change(
                &mut tx,
                &a.id,
                positive_revision(&a.authorization_epoch)?,
                &format!("issue_draft:{}", r.draft_id),
                false,
            )
            .await?;
            seal(n::PayloadV2 {
                request: r.clone(),
                title: snapshot.draft.title,
                body: snapshot.draft.body,
                repository_native: f.repository_native,
                metadata: n::SelectionV2::from_public(snapshot.metadata)?,
            })?
        };
        let receipt = command_admission::admit_in(&mut tx, &s, &Admission)
            .await
            .map_err(super::text_edits::admission_error)?;
        if !receipt.duplicate {
            let p = decode_submission(&s)?;
            sqlx::query("INSERT INTO issue_submissions VALUES(?,?,?,?,?,?)")
                .bind(&r.context.account_id)
                .bind(&r.draft_id)
                .bind(crate::issue_creation::native::revision(
                    &r.draft_generation,
                    true,
                )?)
                .bind(&r.command_id)
                .bind(s.submission_hash().as_slice())
                .bind(n::content_hash(&p.title, &p.body, &p.metadata)?)
                .execute(&mut *tx)
                .await
                .map_err(storage_error)?;
        }
        tx.commit().await.map_err(storage_error)?;
        Ok(IssueSubmissionReceipt {
            account_id: receipt.account_id,
            command_id: receipt.command_id,
            admitted_revision: receipt.admitted_revision,
            duplicate: receipt.duplicate,
        })
    }
    pub async fn issue_drafts_v2(&self, q: IssueDraftQuery) -> Result<IssueDraftV2Page> {
        // Read each authored selection under the same revision/view as the legacy page;
        // a concurrent edit retires this bounded page instead of mixing generations.
        let page = self.issue_drafts(q).await?;
        let mut tx = self.inner.readers.begin().await.map_err(storage_error)?;
        let (revision, view) = metadata(&mut tx).await?;
        if revision != page.revision || view != page.authorization_view {
            return Err(stale());
        }
        let mut drafts = Vec::with_capacity(page.drafts.len());
        for draft in page.drafts {
            let metadata = selection_in(&mut tx, &page.account_id, &draft.draft_id)
                .await?
                .public();
            drafts.push(IssueDraftV2Summary { draft, metadata });
        }
        tx.commit().await.map_err(storage_error)?;
        Ok(IssueDraftV2Page {
            account_id: page.account_id,
            drafts,
            next_cursor: page.next_cursor,
            revision,
            authorization_view: view,
        })
    }
}
pub(crate) async fn prepare_in(
    tx: &mut Transaction<'_, Sqlite>,
    c: &DeliveryCommand,
    a: &RemoteAccount,
) -> Result<Vec<u8>> {
    let p = decode_command(c)?;
    let f = frame_in(tx, a, &c.target_id).await?;
    if f.repository_native != p.repository_native {
        return Err(stale());
    }
    n::encode(&f)
}
pub(crate) async fn finalize_in(
    tx: &mut Transaction<'_, Sqlite>,
    c: &DeliveryCommand,
    e: &n::ReceiptV2,
) -> Result<()> {
    let a = account_in(tx, &c.account_id, true).await?;
    let f = frame_in(tx, &a, &c.target_id).await?;
    let p = decode_command(c)?;
    if f != e.preparation.frame
        || a.actor_id != e.preparation.actor
        || a.authorization_epoch != e.preparation.epoch
        || !n::receipt_matches(e, &p)
    {
        return Err(stale());
    }
    let repository = old::capture_in(tx, &a, &c.target_id).await?.repository;
    let item = RemoteItem {
        id: format!("github:issue:{}", e.core.provider_id),
        account_id: a.id.clone(),
        repository_id: Some(f.repository_id.clone()),
        provider_id: e.core.provider_id.clone(),
        kind: RemoteItemKind::Issue,
        number: Some(e.core.number.clone()),
        title: e.core.title.clone(),
        body: e.core.body.clone(),
        body_omitted: false,
        author: Some(e.core.author_login.clone()),
        web_url: Some(e.core.web_url.clone()),
        state: e.core.state.clone(),
        updated_at: e.core.updated_at.clone(),
        head_oid: None,
        is_draft: None,
        reason: None,
        unread: None,
        native_inbox: None,
    };
    let observed_at = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Nanos, true);
    let entity = old::publication::publish_in(
        tx,
        &a,
        old::publication::Publication {
            command_id: &c.command_id,
            authorization_view: &f.authorization_view,
            repository: &repository,
            item: &item,
            metadata: Some(core_metadata(e, &observed_at)),
            observed_at: &observed_at,
        },
    )
    .await?;
    sqlx::query("INSERT INTO issue_resolutions VALUES(?,?,?,?,?,?,?)")
        .bind(&a.id)
        .bind(&p.request.draft_id)
        .bind(&c.command_id)
        .bind(entity)
        .bind(&e.core.provider_id)
        .bind(&e.core.number)
        .bind(&e.core.web_url)
        .execute(&mut **tx)
        .await
        .map_err(storage_error)?;
    record_change(
        tx,
        &a.id,
        positive_revision(&a.authorization_epoch)?,
        &format!("issue_draft:{}", p.request.draft_id),
        false,
    )
    .await?;
    Ok(())
}

fn core_metadata(e: &n::ReceiptV2, observed_at: &str) -> crate::ResourceMetadataObservation {
    use crate::*;
    ResourceMetadataObservation {
        kind: RemoteItemKind::Issue,
        values: ResourceMetadataValues {
            title: Some(e.core.title.clone()),
            state: Some(e.core.state.clone()),
            author: Some(DetailActor {
                provider_id: e.core.author_id.clone(),
                login: e.core.author_login.clone(),
                web_url: None,
            }),
            web_url: Some(e.core.web_url.clone()),
            updated_at: Some(e.core.updated_at.clone()),
            ..Default::default()
        },
        fields: MetadataField::COMMON
            .into_iter()
            .map(|field| MetadataObservedField {
                field,
                state: if matches!(
                    field,
                    MetadataField::Labels
                        | MetadataField::Assignees
                        | MetadataField::Milestone
                        | MetadataField::StateReason
                ) {
                    DetailValueState::Omitted
                } else {
                    DetailValueState::Known
                },
            })
            .collect(),
        source: MetadataSource {
            source: "github/issue-detail/2026-03-10".into(),
            adapter_version: 1,
            provider_updated_at: Some(e.core.updated_at.clone()),
            observed_at: observed_at.into(),
        },
    }
}
