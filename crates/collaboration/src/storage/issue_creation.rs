//! Authored issue drafts, repository-target admission and immutable resolution.
use super::*;
use crate::{
    commands::*,
    delivery::DeliveryCommand,
    issue_creation::{native as n, *},
};
use command_admission::{CommandAdmissionPolicy, CommandProtection};
pub(crate) mod publication;
pub(crate) struct Admission;
#[async_trait::async_trait]
impl CommandAdmissionPolicy for Admission {
    const OPERATION_KIND: &'static str = n::OPERATION;
    const PAYLOAD_VERSION: u32 = 1;
    async fn validate(
        &self,
        tx: &mut Transaction<'_, Sqlite>,
        account: &RemoteAccount,
        s: &CommandSubmission,
    ) -> Result<Vec<CommandProtection>> {
        let p = n::decode_submission(s)?;
        let frame = capture_in(tx, account, s.target().id()).await?;
        if context(account, &frame)? != p.request.context
            || frame.repository.provider_id != p.repository_native
        {
            return Err(stale());
        }
        let old = draft_in(
            tx,
            &account.id,
            &p.request.draft_id,
            &p.request.context.repository_id,
        )
        .await?;
        if old.0 != p.title || old.1 != p.body || old.2.to_string() != p.request.draft_generation {
            return Err(stale());
        }
        Ok(vec![CommandProtection::Entity(frame.repository.id)])
    }
}
pub(crate) fn seal(p: n::Payload) -> Result<CommandSubmission> {
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
pub(crate) fn context(a: &RemoteAccount, f: &n::Frame) -> Result<IssueDraftContext> {
    Ok(IssueDraftContext {
        account_id: a.id.clone(),
        repository_id: f.repository.id.clone(),
        authorization_epoch: a.authorization_epoch.clone(),
        authorization_view: f.authorization_view.clone(),
        review_token: format!(
            "{:x}",
            Sha256::new()
                .chain_update(b"gitru.issue-create-review.v1")
                .chain_update(n::encode(&(
                    a.id.as_str(),
                    a.actor_id.as_str(),
                    a.authorization_epoch.as_str(),
                    f
                ))?)
                .finalize()
        ),
    })
}
pub(crate) async fn capture_in(
    tx: &mut Transaction<'_, Sqlite>,
    a: &RemoteAccount,
    repo: &str,
) -> Result<n::Frame> {
    if a.provider != ProviderKind::Github
        || a.host != "github.com"
        || a.state != AccountState::Active
    {
        return Err(stale());
    }
    epoch_in(tx, &a.id, &a.authorization_epoch).await?;
    if !identities::accessible(tx, &a.id, repo, ResourceKind::Repository).await? {
        return Err(stale());
    }
    let json: String =
        sqlx::query_scalar("SELECT json FROM repositories WHERE account_id=? AND id=?")
            .bind(&a.id)
            .bind(repo)
            .fetch_optional(&mut **tx)
            .await
            .map_err(storage_error)?
            .ok_or_else(stale)?;
    let mut repository: RemoteRepository = decode(&json)?;
    if !repository.selected {
        return Err(stale());
    }
    repository.description = None;
    repository.default_branch = None;
    let (_, authorization_view) = metadata(tx).await?;
    Ok(n::Frame {
        repository,
        authorization_view,
    })
}
pub(crate) async fn validate_frame_in(
    tx: &mut Transaction<'_, Sqlite>,
    a: &RemoteAccount,
    f: &n::Frame,
) -> Result<()> {
    if capture_in(tx, a, &f.repository.id).await? != *f {
        return Err(stale());
    }
    Ok(())
}
async fn draft_in(
    tx: &mut Transaction<'_, Sqlite>,
    account: &str,
    draft: &str,
    repo: &str,
) -> Result<(String, String, i64)> {
    let row:Option<(String,String,String,i64)>=sqlx::query_as("SELECT repository_id,title,body,generation FROM issue_drafts WHERE account_id=? AND draft_id=?").bind(account).bind(draft).fetch_optional(&mut **tx).await.map_err(storage_error)?;
    match row {
        Some((r, t, b, g)) if r == repo => Ok((t, b, g)),
        Some(_) => Err(stale()),
        None => Ok((String::new(), String::new(), 0)),
    }
}
async fn submission_in(
    tx: &mut Transaction<'_, Sqlite>,
    account: &str,
    draft: &str,
    generation: i64,
) -> Result<Option<IssueSubmissionStatus>> {
    let row=sqlx::query("SELECT c.command_id,s.draft_generation,c.state,(SELECT count(*) FROM delivery_attempts a WHERE a.account_id=c.account_id AND a.command_id=c.command_id) AS attempt_count,d.attention,EXISTS(SELECT 1 FROM command_recovery_quarantine q WHERE q.account_id=c.account_id AND q.command_id=c.command_id) AS quarantined FROM issue_submissions s JOIN commands c USING(account_id,command_id) JOIN command_delivery d USING(account_id,command_id) WHERE s.account_id=? AND s.draft_id=? AND (s.draft_generation=? OR c.state NOT IN ('cancelled','rejected')) ORDER BY (s.draft_generation=?) DESC,s.draft_generation DESC LIMIT 1").bind(account).bind(draft).bind(generation).bind(generation).fetch_optional(&mut **tx).await.map_err(storage_error)?;
    Ok(row.map(|r| IssueSubmissionStatus {
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
    key: &IssueDraftKey,
) -> Result<IssueDraftSnapshot> {
    let (title, body, g) = draft_in(tx, &a.id, &key.draft_id, &key.repository_id).await?;
    let submission = submission_in(tx, &a.id, &key.draft_id, g).await?;
    let (revision, authorization_view) = metadata(tx).await?;
    let mut ctx = None;
    let reason = if a.provider != ProviderKind::Github || a.host != "github.com" {
        Some(IssueDraftReason::UnsupportedProvider)
    } else if a.state != AccountState::Active {
        Some(IssueDraftReason::AccountUnavailable)
    } else if let Some(s) = &submission {
        Some(
            if s.state == "confirmed" || s.draft_generation == g.to_string() {
                IssueDraftReason::AlreadySubmitted
            } else {
                IssueDraftReason::PendingSubmission
            },
        )
    } else if title.is_empty() {
        Some(IssueDraftReason::EmptyTitle)
    } else {
        match capture_in(tx, a, &key.repository_id).await {
            Ok(f) => {
                ctx = Some(context(a, &f)?);
                None
            }
            Err(e)
                if matches!(
                    e.code,
                    ErrorCode::NotFound | ErrorCode::StaleView | ErrorCode::PermissionDenied
                ) =>
            {
                Some(IssueDraftReason::MissingRepository)
            }
            Err(e) => return Err(e),
        }
    };
    let row=sqlx::query("SELECT r.entity_id,r.provider_id,r.number,r.url,r.command_id FROM issue_resolutions r JOIN commands c USING(account_id,command_id) WHERE r.account_id=? AND r.draft_id=? AND c.state='confirmed'").bind(&a.id).bind(&key.draft_id).fetch_optional(&mut **tx).await.map_err(storage_error)?;
    let mut published = None;
    if a.state == AccountState::Active
        && let Some(r) = row
    {
        let id: String = r.get("entity_id");
        if identities::accessible(tx, &a.id, &id, ResourceKind::Issue).await?
            && details::subject_in(tx, &a.id, &id).await.is_ok()
        {
            published = Some(CreatedIssueIdentity {
                subject_id: id,
                provider_id: r.get("provider_id"),
                number: r.get("number"),
                url: r.get("url"),
                command_id: r.get("command_id"),
            });
        }
    }
    Ok(IssueDraftSnapshot {
        account_id: a.id.clone(),
        draft_id: key.draft_id.clone(),
        repository_id: key.repository_id.clone(),
        title,
        body,
        generation: g.to_string(),
        context: ctx,
        availability: if reason.is_none() {
            IssueDraftAvailability::Available
        } else {
            IssueDraftAvailability::Unavailable
        },
        reason,
        submission,
        published,
        revision,
        authorization_view,
    })
}
pub(crate) fn validate_key(k: &IssueDraftKey) -> Result<()> {
    n::identifier(&k.account_id)?;
    n::validate_uuid(&k.draft_id)?;
    n::identifier(&k.repository_id)
}
pub(crate) fn validate_save(r: &SaveIssueDraftRequest) -> Result<()> {
    validate_key(&IssueDraftKey {
        account_id: r.account_id.clone(),
        draft_id: r.draft_id.clone(),
        repository_id: r.repository_id.clone(),
    })?;
    n::revision(&r.authorization_epoch, true)?;
    n::revision(&r.authorization_view, false)?;
    n::revision(&r.expected_generation, false)?;
    n::validate_title(&r.title, false)?;
    n::validate_body(&r.body)
}
impl Store {
    pub async fn issue_draft(&self, key: IssueDraftKey) -> Result<IssueDraftSnapshot> {
        validate_key(&key)?;
        let mut tx = self.inner.readers.begin().await.map_err(storage_error)?;
        let a = account_in(&mut tx, &key.account_id, false).await?;
        let s = snapshot_in(&mut tx, &a, &key).await?;
        tx.commit().await.map_err(storage_error)?;
        Ok(s)
    }
    pub(crate) async fn save_issue_draft(
        &self,
        r: SaveIssueDraftRequest,
    ) -> Result<IssueDraftSnapshot> {
        validate_save(&r)?;
        let key = IssueDraftKey {
            account_id: r.account_id.clone(),
            draft_id: r.draft_id.clone(),
            repository_id: r.repository_id.clone(),
        };
        let mut writer = self.inner.writer.acquire().await?;
        let mut tx = writer.begin().await.map_err(storage_error)?;
        let a = account_in(&mut tx, &r.account_id, false).await?;
        let (_, view) = metadata(&mut tx).await?;
        if a.authorization_epoch != r.authorization_epoch || view != r.authorization_view {
            return Err(stale());
        }
        let old = draft_in(&mut tx, &r.account_id, &r.draft_id, &r.repository_id).await?;
        let expected = n::revision(&r.expected_generation, false)?;
        if old.2 != expected {
            return Err(stale());
        }
        if old.0 != r.title || old.1 != r.body || expected == 0 {
            let next = expected
                .checked_add(1)
                .ok_or_else(CollaborationError::storage)?;
            sqlx::query("INSERT INTO issue_drafts VALUES(?,?,?,?,?,?) ON CONFLICT(account_id,draft_id) DO UPDATE SET title=excluded.title,body=excluded.body,generation=excluded.generation").bind(&r.account_id).bind(&r.draft_id).bind(&r.repository_id).bind(&r.title).bind(&r.body).bind(next).execute(&mut *tx).await.map_err(storage_error)?;
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
    pub(crate) async fn submit_issue(
        &self,
        r: SubmitIssueRequest,
    ) -> Result<IssueSubmissionReceipt> {
        n::validate_send(&r)?;
        let mut writer = self.inner.writer.acquire().await?;
        let mut tx = writer.begin().await.map_err(storage_error)?;
        epoch_in(
            &mut tx,
            &r.context.account_id,
            &r.context.authorization_epoch,
        )
        .await?;
        let old: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM commands WHERE account_id=? AND command_id=?)",
        )
        .bind(&r.context.account_id)
        .bind(&r.command_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(storage_error)?;
        let s = if old {
            let c = delivery::load_in(&mut tx, &r.context.account_id, &r.command_id).await?;
            if c.operation_kind != n::OPERATION
                || c.payload_version != 1
                || c.target_kind != "repository"
            {
                return Err(n::invalid());
            }
            let p = n::decode(&c)?;
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
            if snapshot.context.as_ref() != Some(&r.context)
                || snapshot.generation != r.draft_generation
                || snapshot.availability != IssueDraftAvailability::Available
            {
                return Err(stale());
            }
            let f = capture_in(&mut tx, &a, &r.context.repository_id).await?;
            record_change(
                &mut tx,
                &a.id,
                positive_revision(&a.authorization_epoch)?,
                &format!("issue_draft:{}", r.draft_id),
                false,
            )
            .await?;
            seal(n::Payload {
                request: r.clone(),
                title: snapshot.title,
                body: snapshot.body,
                repository_native: f.repository.provider_id,
            })?
        };
        let receipt = command_admission::admit_in(&mut tx, &s, &Admission)
            .await
            .map_err(super::text_edits::admission_error)?;
        if !receipt.duplicate {
            let p = n::decode_submission(&s)?;
            sqlx::query("INSERT INTO issue_submissions VALUES(?,?,?,?,?,?)")
                .bind(&r.context.account_id)
                .bind(&r.draft_id)
                .bind(n::revision(&r.draft_generation, true)?)
                .bind(&r.command_id)
                .bind(s.submission_hash().as_slice())
                .bind(n::content_hash(&p.title, &p.body))
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
    pub async fn issue_drafts(&self, q: IssueDraftQuery) -> Result<IssueDraftPage> {
        n::identifier(&q.account_id)?;
        if q.limit == 0 || q.limit > 100 {
            return Err(n::invalid());
        }
        let mut tx = self.inner.readers.begin().await.map_err(storage_error)?;
        account_in(&mut tx, &q.account_id, false).await?;
        let (revision, view) = metadata(&mut tx).await?;
        let after = if let Some(cursor) = q.cursor {
            if cursor.len() > 4096 {
                return Err(n::invalid());
            }
            let c: DraftCursor = decode(&cursor)?;
            if c.account != q.account_id || c.view != view || c.revision != revision {
                return Err(stale());
            }
            n::validate_uuid(&c.after)?;
            c.after
        } else {
            String::new()
        };
        let rows=sqlx::query("SELECT draft_id,repository_id,title,substr(body,1,256) AS preview,generation FROM issue_drafts WHERE account_id=? AND draft_id>? ORDER BY draft_id LIMIT ?").bind(&q.account_id).bind(after).bind(i64::from(q.limit)+1).fetch_all(&mut *tx).await.map_err(storage_error)?;
        let more = rows.len() > q.limit as usize;
        let mut drafts = vec![];
        for r in rows.into_iter().take(q.limit as usize) {
            let id: String = r.get("draft_id");
            let g: i64 = r.get("generation");
            drafts.push(IssueDraftSummary {
                submission: submission_in(&mut tx, &q.account_id, &id, g).await?,
                draft_id: id,
                repository_id: r.get("repository_id"),
                title: r.get("title"),
                preview: r.get("preview"),
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
        Ok(IssueDraftPage {
            account_id: q.account_id,
            drafts,
            next_cursor,
            revision,
            authorization_view: view,
        })
    }
}
#[derive(Serialize, Deserialize)]
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
    let f = capture_in(tx, a, &c.target_id).await?;
    if f.repository.provider_id != p.repository_native {
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
    let f = capture_in(tx, &a, &c.target_id).await?;
    if f != e.preparation.frame
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
    sqlx::query("INSERT INTO issue_resolutions VALUES(?,?,?,?,?,?,?)")
        .bind(&a.id)
        .bind(&p.request.draft_id)
        .bind(&c.command_id)
        .bind(entity)
        .bind(&e.receipt.item.provider_id)
        .bind(e.receipt.item.number.as_deref().ok_or_else(n::invalid)?)
        .bind(e.receipt.item.web_url.as_deref().ok_or_else(n::invalid)?)
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
