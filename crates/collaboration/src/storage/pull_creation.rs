//! Authored drafts and native-only online authority over the existing durable ledger.
use super::*;
use crate::{
    commands::*,
    delivery::DeliveryCommand,
    pull_creation::{native as n, *},
};
use command_admission::{CommandAdmissionPolicy, CommandProtection};
pub(crate) mod publication;

fn read_values(row: &sqlx::sqlite::SqliteRow) -> PullDraftValues {
    PullDraftValues {
        title: row.get("title"),
        body: row.get("body"),
        source_branch: row.get("source_branch"),
        base_branch: row.get("base_branch"),
        local_repository_id: row.get("local_repository_id"),
        link_id: row.get("link_id"),
        link_generation: row.get("link_generation"),
        is_draft: row.get("is_draft"),
    }
}
async fn draft_in(
    tx: &mut Transaction<'_, Sqlite>,
    key: &PullDraftKey,
) -> Result<(PullDraftValues, i64)> {
    let row = sqlx::query("SELECT * FROM pull_drafts WHERE account_id=? AND draft_id=?")
        .bind(&key.account_id)
        .bind(&key.draft_id)
        .fetch_optional(&mut **tx)
        .await
        .map_err(storage_error)?;
    match row {
        Some(r) if r.get::<String, _>("repository_id") == key.repository_id => {
            Ok((read_values(&r), r.get("generation")))
        }
        Some(_) => Err(stale()),
        None => Ok((PullDraftValues::default(), 0)),
    }
}
async fn submission_in(
    tx: &mut Transaction<'_, Sqlite>,
    account: &str,
    draft: &str,
    generation: i64,
) -> Result<Option<PullSubmissionStatus>> {
    let row=sqlx::query("SELECT c.command_id,s.draft_generation,c.state,(SELECT count(*) FROM delivery_attempts a WHERE a.account_id=c.account_id AND a.command_id=c.command_id) AS attempt_count,d.attention,EXISTS(SELECT 1 FROM command_recovery_quarantine q WHERE q.account_id=c.account_id AND q.command_id=c.command_id) AS quarantined FROM pull_submissions s JOIN commands c USING(account_id,command_id) JOIN command_delivery d USING(account_id,command_id) WHERE s.account_id=? AND s.draft_id=? AND (s.draft_generation=? OR c.state NOT IN ('cancelled','rejected')) ORDER BY (s.draft_generation=?) DESC,s.draft_generation DESC LIMIT 1").bind(account).bind(draft).bind(generation).bind(generation).fetch_optional(&mut **tx).await.map_err(storage_error)?;
    Ok(row.map(|r| PullSubmissionStatus {
        command_id: r.get("command_id"),
        draft_generation: r.get::<i64, _>("draft_generation").to_string(),
        state: r.get("state"),
        attempt_count: r.get::<i64, _>("attempt_count") as u32,
        quarantined: r.get("quarantined"),
        attention: r.get("attention"),
    }))
}
async fn snapshot_in(
    tx: &mut Transaction<'_, Sqlite>,
    a: &RemoteAccount,
    k: &PullDraftKey,
) -> Result<PullDraftSnapshot> {
    let (values, generation) = draft_in(tx, k).await?;
    let submission = submission_in(tx, &a.id, &k.draft_id, generation).await?;
    let (revision, authorization_view) = metadata(tx).await?;
    let reason = if a.provider != ProviderKind::Github || a.host != "github.com" {
        Some(PullCreationReason::UnsupportedProvider)
    } else if a.state != AccountState::Active {
        Some(PullCreationReason::AccountUnavailable)
    } else if let Some(s) = &submission {
        Some(
            if s.state == "confirmed" || s.draft_generation == generation.to_string() {
                PullCreationReason::AlreadySubmitted
            } else {
                PullCreationReason::PendingSubmission
            },
        )
    } else if n::validate_values(&values, true).is_err() {
        Some(PullCreationReason::IncompleteDraft)
    } else {
        match issue_creation::capture_in(tx, a, &k.repository_id).await {
            Ok(_) => None,
            Err(e)
                if matches!(
                    e.code,
                    ErrorCode::NotFound | ErrorCode::StaleView | ErrorCode::PermissionDenied
                ) =>
            {
                Some(PullCreationReason::MissingRepository)
            }
            Err(e) => return Err(e),
        }
    };
    let row=sqlx::query("SELECT r.* FROM pull_resolutions r JOIN commands c USING(account_id,command_id) WHERE r.account_id=? AND r.draft_id=? AND c.state='confirmed'").bind(&a.id).bind(&k.draft_id).fetch_optional(&mut **tx).await.map_err(storage_error)?;
    let mut published = None;
    if a.state == AccountState::Active
        && let Some(r) = row
    {
        let id: String = r.get("entity_id");
        if identities::accessible(tx, &a.id, &id, ResourceKind::PullRequest).await?
            && details::subject_in(tx, &a.id, &id).await.is_ok()
        {
            let command: String = r.get("command_id");
            let p = n::decode(&delivery::load_in(tx, &a.id, &command).await?)?;
            let observed_source_oid: String = r.get("observed_source_oid");
            let observed_base_oid: String = r.get("observed_base_oid");
            published = Some(CreatedPullIdentity {
                subject_id: id,
                provider_id: r.get("provider_id"),
                number: r.get("number"),
                url: r.get("url"),
                command_id: command,
                branches_changed: observed_source_oid != p.request.context.source_oid
                    || observed_base_oid != p.request.context.base_oid,
                inspected_source_oid: p.request.context.source_oid,
                inspected_base_oid: p.request.context.base_oid,
                observed_source_oid,
                observed_base_oid,
            });
        }
    }
    Ok(PullDraftSnapshot {
        key: k.clone(),
        values,
        generation: generation.to_string(),
        can_preview: reason.is_none(),
        reason,
        submission,
        published,
        revision,
        authorization_view,
    })
}
pub(crate) async fn validate_local_in(
    tx: &mut Transaction<'_, Sqlite>,
    a: &RemoteAccount,
    repo: &RemoteRepository,
    proof: &n::LocalProof,
) -> Result<()> {
    proof.validate()?;
    let instance = identities::instance_in(tx, a).await?;
    let valid:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM local_repository_links WHERE id=? AND CAST(generation AS TEXT)=? AND local_repository_id=? AND account_id=? AND actor_id=? AND instance_id=? AND repository_id=? AND repository_provider_id=? AND registration_proof=? AND remote_digest=?)")
        .bind(&proof.link_id).bind(&proof.link_generation).bind(&proof.local_repository_id).bind(&a.id).bind(&a.actor_id).bind(&instance.id).bind(&repo.id).bind(&repo.provider_id).bind(&proof.registration_proof).bind(&proof.remote_digest).fetch_one(&mut **tx).await.map_err(storage_error)?;
    if !valid {
        return Err(stale());
    }
    Ok(())
}
pub(crate) async fn capture_in(
    tx: &mut Transaction<'_, Sqlite>,
    a: &RemoteAccount,
    k: &PullDraftKey,
    local: &n::LocalProof,
) -> Result<n::Frame> {
    n::validate_key(k)?;
    let base = issue_creation::capture_in(tx, a, &k.repository_id).await?;
    let (values, generation) = draft_in(tx, k).await?;
    n::validate_values(&values, true)?;
    if generation == 0 || !local.matches_values(&values) {
        return Err(stale());
    }
    validate_local_in(tx, a, &base.repository, local).await?;
    Ok(n::Frame {
        repository: base.repository,
        authorization_view: base.authorization_view,
        draft_generation: generation.to_string(),
        values,
        local: local.clone(),
    })
}
pub(crate) async fn validate_frame_in(
    tx: &mut Transaction<'_, Sqlite>,
    a: &RemoteAccount,
    f: &n::Frame,
    k: &PullDraftKey,
) -> Result<()> {
    if capture_in(tx, a, k, &f.local).await? != *f {
        return Err(stale());
    }
    Ok(())
}
pub(crate) struct Admission;
#[async_trait::async_trait]
impl CommandAdmissionPolicy for Admission {
    const OPERATION_KIND: &'static str = n::OPERATION;
    const PAYLOAD_VERSION: u32 = 1;
    async fn validate(
        &self,
        tx: &mut Transaction<'_, Sqlite>,
        a: &RemoteAccount,
        s: &CommandSubmission,
    ) -> Result<Vec<CommandProtection>> {
        let p = n::decode_submission(s)?;
        let c = &p.request.context;
        let f = capture_in(tx, a, &c.key, &p.local).await?;
        if a.actor_id != p.actor_id
            || f.repository.provider_id != p.repository_native
            || f.authorization_view != c.authorization_view
            || f.draft_generation != c.draft_generation
            || f.values != p.values
            || submission_in(
                tx,
                &a.id,
                &c.key.draft_id,
                n::revision(&c.draft_generation, true)?,
            )
            .await?
            .is_some()
        {
            return Err(stale());
        }
        Ok(vec![CommandProtection::Entity(f.repository.id)])
    }
}
pub(crate) fn seal(p: n::Payload) -> Result<CommandSubmission> {
    seal_command(CommandDraft {
        command_id: p.request.command_id.clone(),
        account_id: p.request.context.key.account_id.clone(),
        authorization_epoch: p.request.context.authorization_epoch.clone(),
        target: CommandTarget::new(
            CommandTargetKind::Repository,
            p.request.context.key.repository_id.clone(),
            Some(p.request.context.key.repository_id.clone()),
        )?,
        payload: p,
        guards: vec![],
        dependencies: vec![],
    })
}
impl Store {
    pub async fn pull_draft(&self, key: PullDraftKey) -> Result<PullDraftSnapshot> {
        n::validate_key(&key)?;
        let mut tx = self.inner.readers.begin().await.map_err(storage_error)?;
        let a = account_in(&mut tx, &key.account_id, false).await?;
        let s = snapshot_in(&mut tx, &a, &key).await?;
        tx.commit().await.map_err(storage_error)?;
        Ok(s)
    }
    pub(crate) async fn save_pull_draft(
        &self,
        r: SavePullDraftRequest,
    ) -> Result<PullDraftSnapshot> {
        n::validate_save(&r)?;
        let mut writer = self.inner.writer.acquire().await?;
        let mut tx = writer.begin().await.map_err(storage_error)?;
        let a = account_in(&mut tx, &r.key.account_id, false).await?;
        if a.authorization_epoch != r.authorization_epoch
            || metadata(&mut tx).await?.1 != r.authorization_view
        {
            return Err(stale());
        }
        let (old, generation) = draft_in(&mut tx, &r.key).await?;
        if generation != n::revision(&r.expected_generation, false)? {
            return Err(stale());
        }
        if generation == 0 || old != r.values {
            let next = generation
                .checked_add(1)
                .ok_or_else(CollaborationError::storage)?;
            let v = &r.values;
            sqlx::query("INSERT INTO pull_drafts VALUES(?,?,?,?,?,?,?,?,?,?,?,?) ON CONFLICT(account_id,draft_id) DO UPDATE SET title=excluded.title,body=excluded.body,source_branch=excluded.source_branch,base_branch=excluded.base_branch,local_repository_id=excluded.local_repository_id,link_id=excluded.link_id,link_generation=excluded.link_generation,is_draft=excluded.is_draft,generation=excluded.generation")
                .bind(&a.id).bind(&r.key.draft_id).bind(&r.key.repository_id).bind(&v.title).bind(&v.body).bind(&v.source_branch).bind(&v.base_branch).bind(&v.local_repository_id).bind(&v.link_id).bind(&v.link_generation).bind(v.is_draft).bind(next).execute(&mut *tx).await.map_err(storage_error)?;
            record_change(
                &mut tx,
                &a.id,
                positive_revision(&a.authorization_epoch)?,
                &format!("pull_draft:{}", r.key.draft_id),
                false,
            )
            .await?;
        }
        let s = snapshot_in(&mut tx, &a, &r.key).await?;
        tx.commit().await.map_err(storage_error)?;
        Ok(s)
    }
    pub(crate) async fn pull_creation_frame(
        &self,
        r: &PreviewPullCreationRequest,
        local: &n::LocalProof,
    ) -> Result<(RemoteAccount, n::Frame)> {
        n::validate_preview(r)?;
        let mut tx = self.inner.readers.begin().await.map_err(storage_error)?;
        let a = account_in(&mut tx, &r.key.account_id, true).await?;
        let snapshot = snapshot_in(&mut tx, &a, &r.key).await?;
        if !snapshot.can_preview
            || snapshot.generation != r.draft_generation
            || a.authorization_epoch != r.authorization_epoch
            || snapshot.authorization_view != r.authorization_view
        {
            return Err(stale());
        }
        let f = capture_in(&mut tx, &a, &r.key, local).await?;
        tx.commit().await.map_err(storage_error)?;
        Ok((a, f))
    }
    pub(crate) async fn validate_pull_creation_frame<F>(
        &self,
        a: &RemoteAccount,
        f: &n::Frame,
        k: &PullDraftKey,
        validate_owner: F,
    ) -> Result<()>
    where
        F: Fn() -> Result<()> + Send,
    {
        let mut writer = self.inner.writer.acquire().await?;
        let mut tx = writer.begin().await.map_err(storage_error)?;
        validate_owner()?;
        validate_frame_in(&mut tx, a, f, k).await?;
        validate_owner()?;
        tx.commit().await.map_err(storage_error)
    }
    pub(crate) async fn submit_pull<F, O>(
        &self,
        r: SubmitPullRequest,
        grant: Option<&n::Frame>,
        validate_owner: O,
        validate_online: F,
    ) -> Result<PullSubmissionReceipt>
    where
        F: Fn() -> Result<()> + Send,
        O: Fn() -> Result<()> + Send,
    {
        n::validate_send(&r)?;
        let mut writer = self.inner.writer.acquire().await?;
        validate_owner()?;
        let mut tx = writer.begin().await.map_err(storage_error)?;
        let c = &r.context;
        epoch_in(&mut tx, &c.key.account_id, &c.authorization_epoch).await?;
        let a = account_in(&mut tx, &c.key.account_id, true).await?;
        if metadata(&mut tx).await?.1 != c.authorization_view {
            return Err(stale());
        }
        let duplicate: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM commands WHERE account_id=? AND command_id=?)",
        )
        .bind(&a.id)
        .bind(&r.command_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(storage_error)?;
        let s = if duplicate {
            let old = delivery::load_in(&mut tx, &a.id, &r.command_id).await?;
            if old.operation_kind != n::OPERATION
                || old.payload_version != 1
                || old.target_kind != "repository"
            {
                return Err(n::invalid());
            }
            let p = n::decode(&old)?;
            if p.request != r || p.actor_id != a.actor_id {
                return Err(n::invalid());
            }
            seal(p)?
        } else {
            let f = grant.ok_or_else(|| {
                CollaborationError::new(
                    ErrorCode::NotReady,
                    "Preview this pull request online again",
                )
            })?;
            validate_frame_in(&mut tx, &a, f, &c.key).await?;
            if f.authorization_view != c.authorization_view
                || f.draft_generation != c.draft_generation
                || f.local.source_oid != c.source_oid
            {
                return Err(stale());
            }
            validate_online()?;
            seal(n::Payload {
                request: r.clone(),
                actor_id: a.actor_id.clone(),
                repository_native: f.repository.provider_id.clone(),
                values: f.values.clone(),
                local: f.local.clone(),
            })?
        };
        let receipt = command_admission::admit_in(&mut tx, &s, &Admission)
            .await
            .map_err(super::text_edits::admission_error)?;
        if !receipt.duplicate {
            let p = n::decode_submission(&s)?;
            sqlx::query("INSERT INTO pull_submissions VALUES(?,?,?,?,?,?)")
                .bind(&a.id)
                .bind(&c.key.draft_id)
                .bind(n::revision(&c.draft_generation, true)?)
                .bind(&r.command_id)
                .bind(s.submission_hash().as_slice())
                .bind(n::content_hash(&p.values)?)
                .execute(&mut *tx)
                .await
                .map_err(storage_error)?;
            validate_online()?;
        }
        validate_owner()?;
        tx.commit().await.map_err(storage_error)?;
        Ok(PullSubmissionReceipt {
            account_id: receipt.account_id,
            command_id: receipt.command_id,
            admitted_revision: receipt.admitted_revision,
            duplicate: receipt.duplicate,
        })
    }
    pub async fn pull_drafts(&self, q: PullDraftQuery) -> Result<PullDraftPage> {
        if !n::identifier(&q.account_id)
            || q.limit == 0
            || q.limit > 100
            || q.cursor.as_ref().is_some_and(|c| c.len() > 4096)
        {
            return Err(n::invalid());
        }
        let mut tx = self.inner.readers.begin().await.map_err(storage_error)?;
        account_in(&mut tx, &q.account_id, false).await?;
        let (revision, view) = metadata(&mut tx).await?;
        let after = if let Some(raw) = q.cursor {
            let c: DraftCursor = decode(&raw)?;
            if c.account != q.account_id
                || c.view != view
                || c.revision != revision
                || !n::uuid(&c.after)
            {
                return Err(stale());
            }
            c.after
        } else {
            String::new()
        };
        let rows=sqlx::query("SELECT draft_id,repository_id,title,substr(body,1,256) AS preview,source_branch,base_branch,generation FROM pull_drafts WHERE account_id=? AND draft_id>? ORDER BY draft_id LIMIT ?").bind(&q.account_id).bind(after).bind(i64::from(q.limit)+1).fetch_all(&mut *tx).await.map_err(storage_error)?;
        let more = rows.len() > q.limit as usize;
        let mut drafts = vec![];
        for r in rows.into_iter().take(q.limit as usize) {
            let id: String = r.get("draft_id");
            let g: i64 = r.get("generation");
            drafts.push(PullDraftSummary {
                submission: submission_in(&mut tx, &q.account_id, &id, g).await?,
                draft_id: id,
                repository_id: r.get("repository_id"),
                title: r.get("title"),
                preview: r.get("preview"),
                source_branch: r.get("source_branch"),
                base_branch: r.get("base_branch"),
                generation: g.to_string(),
            });
        }
        let next_cursor = if more {
            Some(encode(&DraftCursor {
                account: q.account_id.clone(),
                view: view.clone(),
                revision: revision.clone(),
                after: drafts.last().ok_or_else(n::invalid)?.draft_id.clone(),
            })?)
        } else {
            None
        };
        tx.commit().await.map_err(storage_error)?;
        Ok(PullDraftPage {
            account_id: q.account_id,
            drafts,
            next_cursor,
            revision,
            authorization_view: view,
        })
    }
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct DraftCursor {
    account: String,
    view: String,
    revision: String,
    after: String,
}
pub(crate) async fn prepare_in(
    tx: &mut Transaction<'_, Sqlite>,
    c: &DeliveryCommand,
    a: &RemoteAccount,
) -> Result<Vec<u8>> {
    let p = n::decode(c)?;
    let f = capture_in(tx, a, &p.request.context.key, &p.local).await?;
    if a.actor_id != p.actor_id
        || f.repository.provider_id != p.repository_native
        || f.values != p.values
        || f.draft_generation != p.request.context.draft_generation
        || f.authorization_view != p.request.context.authorization_view
    {
        return Err(stale());
    }
    n::encode(&f)
}
pub(crate) async fn finalize_in(
    tx: &mut Transaction<'_, Sqlite>,
    c: &DeliveryCommand,
    e: &n::ReceiptEvidence,
) -> Result<()> {
    let a = account_in(tx, &c.account_id, true).await?;
    let f = &e.preparation.frame;
    let current = issue_creation::capture_in(tx, &a, &c.target_id).await?;
    if current.repository != f.repository
        || current.authorization_view != f.authorization_view
        || a.actor_id != e.preparation.actor
        || a.authorization_epoch != e.preparation.epoch
    {
        return Err(stale());
    }
    let p = n::decode(c)?;
    let entity = publication::publish_in(
        tx,
        &a,
        publication::Publication {
            command_id: &c.command_id,
            authorization_view: &f.authorization_view,
            repository: &f.repository,
            item: &e.receipt.item,
            metadata: Some(e.receipt.metadata.observation()),
            observed_at: &e.receipt.metadata.source.observed_at,
        },
    )
    .await?;
    sqlx::query("INSERT INTO pull_resolutions VALUES(?,?,?,?,?,?,?,?,?)")
        .bind(&a.id)
        .bind(&p.request.context.key.draft_id)
        .bind(&c.command_id)
        .bind(entity)
        .bind(&e.receipt.item.provider_id)
        .bind(e.receipt.item.number.as_deref().ok_or_else(n::invalid)?)
        .bind(e.receipt.item.web_url.as_deref().ok_or_else(n::invalid)?)
        .bind(
            &e.receipt
                .metadata
                .values
                .head
                .as_ref()
                .ok_or_else(n::invalid)?
                .oid,
        )
        .bind(
            &e.receipt
                .metadata
                .values
                .base
                .as_ref()
                .ok_or_else(n::invalid)?
                .oid,
        )
        .execute(&mut **tx)
        .await
        .map_err(storage_error)?;
    record_change(
        tx,
        &a.id,
        positive_revision(&a.authorization_epoch)?,
        &format!("pull_draft:{}", p.request.context.key.draft_id),
        false,
    )
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests;
