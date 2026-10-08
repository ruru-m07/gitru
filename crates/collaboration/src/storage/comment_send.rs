//! Independent authored comment drafts and immutable send admission.
use super::*;
use crate::{
    commands::*,
    comment_send::{native as n, *},
    delivery::DeliveryCommand,
};
use command_admission::{CommandAdmissionPolicy, CommandProtection};
pub(crate) struct Admission;
#[async_trait::async_trait]
impl CommandAdmissionPolicy for Admission {
    const OPERATION_KIND: &'static str = n::OPERATION;
    const PAYLOAD_VERSION: u32 = 1;
    async fn validate(
        &self,
        tx: &mut Transaction<'_, Sqlite>,
        account: &RemoteAccount,
        submission: &CommandSubmission,
    ) -> Result<Vec<CommandProtection>> {
        let p = n::decode_submission(submission)?;
        let frame = capture_in(tx, account, submission.target().id()).await?;
        n::validate_dispatch_budget(&frame, &p.body)?;
        if context(account, &frame)? != p.request.context
            || frame.repository.provider_id != p.repository_native
            || frame.subject.provider_id != p.subject_native
            || frame.subject.number.as_deref() != Some(p.number.as_str())
        {
            return Err(stale());
        }
        let draft: Option<(String, i64)> = sqlx::query_as(
            "SELECT body,generation FROM comment_drafts WHERE account_id=? AND subject_id=?",
        )
        .bind(&account.id)
        .bind(submission.target().id())
        .fetch_optional(&mut **tx)
        .await
        .map_err(storage_error)?;
        if draft
            != Some((
                p.body,
                p.request
                    .draft_generation
                    .parse()
                    .map_err(|_| n::invalid())?,
            ))
        {
            return Err(stale());
        }
        Ok(vec![
            CommandProtection::Entity(submission.target().id().into()),
            CommandProtection::Entity(frame.repository.id),
        ])
    }
}
pub(crate) fn seal(p: n::Payload, frame: &n::Frame) -> Result<CommandSubmission> {
    seal_command(CommandDraft {
        command_id: p.request.command_id.clone(),
        account_id: p.request.context.account_id.clone(),
        authorization_epoch: p.request.context.authorization_epoch.clone(),
        target: CommandTarget::new(
            if frame.subject.kind == RemoteItemKind::PullRequest {
                CommandTargetKind::PullRequest
            } else {
                CommandTargetKind::Issue
            },
            p.request.context.subject_id.clone(),
            Some(frame.repository.id.clone()),
        )?,
        payload: p,
        guards: vec![],
        dependencies: vec![],
    })
}
pub(crate) fn context(account: &RemoteAccount, frame: &n::Frame) -> Result<CommentSendContext> {
    let bytes = n::encode(&(
        account.id.as_str(),
        account.actor_id.as_str(),
        account.authorization_epoch.as_str(),
        frame,
    ))?;
    Ok(CommentSendContext {
        account_id: account.id.clone(),
        subject_id: frame.subject.id.clone(),
        authorization_epoch: account.authorization_epoch.clone(),
        authorization_view: frame.authorization_view.clone(),
        review_token: format!(
            "{:x}",
            Sha256::new()
                .chain_update(b"gitru.comment-send-review.v1")
                .chain_update(bytes)
                .finalize()
        ),
    })
}
pub(crate) async fn capture_in(
    tx: &mut Transaction<'_, Sqlite>,
    account: &RemoteAccount,
    subject: &str,
) -> Result<n::Frame> {
    if account.provider != ProviderKind::Github
        || account.host != "github.com"
        || account.state != AccountState::Active
    {
        return Err(stale());
    }
    epoch_in(tx, &account.id, &account.authorization_epoch).await?;
    let mut item = details::subject_in(tx, &account.id, subject).await?;
    let repo = item.repository_id.as_deref().ok_or_else(stale)?;
    if !identities::accessible(tx, &account.id, repo, crate::ResourceKind::Repository).await? {
        return Err(stale());
    }
    let json: String =
        sqlx::query_scalar("SELECT json FROM repositories WHERE account_id=? AND id=?")
            .bind(&account.id)
            .bind(repo)
            .fetch_one(&mut **tx)
            .await
            .map_err(storage_error)?;
    let mut repository: RemoteRepository = decode(&json)?;
    if !repository.selected {
        return Err(stale());
    }
    // Capture identity/access/range facts, never arbitrary cached description content.
    item.body = None;
    item.body_omitted = false;
    item.title.clear();
    item.author = None;
    item.reason = None;
    repository.description = None;
    repository.default_branch = None;
    let (_, authorization_view) = metadata(tx).await?;
    let frame = n::Frame {
        repository,
        subject: item,
        authorization_view,
    };
    n::encode(&frame)?;
    Ok(frame)
}
pub(crate) async fn validate_frame_in(
    tx: &mut Transaction<'_, Sqlite>,
    account: &RemoteAccount,
    frame: &n::Frame,
) -> Result<()> {
    if capture_in(tx, account, &frame.subject.id).await? != *frame {
        return Err(stale());
    }
    Ok(())
}
async fn draft_in(
    tx: &mut Transaction<'_, Sqlite>,
    account: &str,
    subject: &str,
) -> Result<(String, i64)> {
    Ok(sqlx::query_as(
        "SELECT body,generation FROM comment_drafts WHERE account_id=? AND subject_id=?",
    )
    .bind(account)
    .bind(subject)
    .fetch_optional(&mut **tx)
    .await
    .map_err(storage_error)?
    .unwrap_or_default())
}
async fn submission_in(
    tx: &mut Transaction<'_, Sqlite>,
    account: &str,
    subject: &str,
    generation: i64,
) -> Result<Option<CommentSubmissionStatus>> {
    let row=sqlx::query("SELECT c.command_id,s.draft_generation,c.state,(SELECT count(*) FROM delivery_attempts a WHERE a.account_id=c.account_id AND a.command_id=c.command_id) AS attempt_count,d.attention,EXISTS(SELECT 1 FROM command_recovery_quarantine q WHERE q.account_id=c.account_id AND q.command_id=c.command_id) AS quarantined FROM comment_submissions s JOIN commands c USING(account_id,command_id) JOIN command_delivery d USING(account_id,command_id) WHERE s.account_id=? AND s.subject_id=? AND (s.draft_generation=? OR c.state IN ('queued','sending','retry_wait','accepted','outcome_unknown','conflict')) ORDER BY (s.draft_generation=?) DESC,s.draft_generation DESC LIMIT 1").bind(account).bind(subject).bind(generation).bind(generation).fetch_optional(&mut **tx).await.map_err(storage_error)?;
    Ok(row.map(|r| CommentSubmissionStatus {
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
    account: &RemoteAccount,
    subject: &str,
) -> Result<CommentDraftSnapshot> {
    let (body, generation) = draft_in(tx, &account.id, subject).await?;
    let submission = submission_in(tx, &account.id, subject, generation).await?;
    let (revision, authorization_view) = metadata(tx).await?;
    let mut context_value = None;
    let reason = if account.provider != ProviderKind::Github || account.host != "github.com" {
        Some(CommentSendReason::UnsupportedProvider)
    } else if account.state != AccountState::Active {
        Some(CommentSendReason::AccountUnavailable)
    } else if submission.is_some() {
        Some(
            if submission.as_ref().is_some_and(|s| s.state == "confirmed") {
                CommentSendReason::AlreadySubmitted
            } else {
                CommentSendReason::PendingSubmission
            },
        )
    } else if body.trim().is_empty() {
        Some(CommentSendReason::EmptyDraft)
    } else {
        match capture_in(tx, account, subject).await {
            Ok(frame) => {
                context_value = Some(context(account, &frame)?);
                None
            }
            Err(e)
                if matches!(
                    e.code,
                    ErrorCode::NotFound | ErrorCode::PermissionDenied | ErrorCode::StaleView
                ) =>
            {
                Some(CommentSendReason::MissingTarget)
            }
            Err(e) => return Err(e),
        }
    };
    Ok(CommentDraftSnapshot {
        account_id: account.id.clone(),
        subject_id: subject.into(),
        body,
        generation: generation.to_string(),
        context: context_value,
        availability: if reason.is_none() {
            CommentSendAvailability::Available
        } else {
            CommentSendAvailability::Unavailable
        },
        reason,
        submission,
        revision,
        authorization_view,
    })
}
impl Store {
    pub async fn comment_draft(
        &self,
        account: &str,
        subject: &str,
    ) -> Result<CommentDraftSnapshot> {
        n::identifier(account)?;
        n::identifier(subject)?;
        let mut tx = self.inner.readers.begin().await.map_err(storage_error)?;
        let account = account_in(&mut tx, account, false).await?;
        let s = snapshot_in(&mut tx, &account, subject).await?;
        tx.commit().await.map_err(storage_error)?;
        Ok(s)
    }
    pub(crate) async fn save_comment_draft(
        &self,
        r: SaveCommentDraftRequest,
    ) -> Result<CommentDraftSnapshot> {
        validate_save(&r)?;
        let mut writer = self.inner.writer.acquire().await?;
        let mut tx = writer.begin().await.map_err(storage_error)?;
        let account = account_in(&mut tx, &r.account_id, false).await?;
        let (_, view) = metadata(&mut tx).await?;
        if account.authorization_epoch != r.authorization_epoch || view != r.authorization_view {
            return Err(stale());
        }
        let old = draft_in(&mut tx, &r.account_id, &r.subject_id).await?;
        let expected = n::revision(&r.expected_generation, false)?;
        if old.1 != expected {
            return Err(stale());
        }
        if old.0 != r.body {
            let next = expected
                .checked_add(1)
                .ok_or_else(CollaborationError::storage)?;
            sqlx::query("INSERT INTO comment_drafts VALUES(?,?,?,?) ON CONFLICT(account_id,subject_id) DO UPDATE SET body=excluded.body,generation=excluded.generation").bind(&r.account_id).bind(&r.subject_id).bind(&r.body).bind(next).execute(&mut *tx).await.map_err(storage_error)?;
            record_change(
                &mut tx,
                &account.id,
                positive_revision(&account.authorization_epoch)?,
                &format!("comment_draft:{}", r.subject_id),
                false,
            )
            .await?;
        }
        let s = snapshot_in(&mut tx, &account, &r.subject_id).await?;
        tx.commit().await.map_err(storage_error)?;
        Ok(s)
    }
    pub(crate) async fn send_comment(
        &self,
        r: SendCommentRequest,
    ) -> Result<CommentSubmissionReceipt> {
        n::validate_send(&r)?;
        let mut writer = self.inner.writer.acquire().await?;
        let mut tx = writer.begin().await.map_err(storage_error)?;
        epoch_in(
            &mut tx,
            &r.context.account_id,
            &r.context.authorization_epoch,
        )
        .await?;
        let existing: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM commands WHERE account_id=? AND command_id=?)",
        )
        .bind(&r.context.account_id)
        .bind(&r.command_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(storage_error)?;
        let submission = if existing {
            let c = delivery::load_in(&mut tx, &r.context.account_id, &r.command_id).await?;
            if c.operation_kind != n::OPERATION || c.payload_version != 1 {
                return Err(n::invalid());
            }
            let p = n::decode(&c)?;
            if p.request != r {
                return Err(n::invalid());
            }
            seal_command(CommandDraft {
                command_id: c.command_id.clone(),
                account_id: c.account_id.clone(),
                authorization_epoch: c.authorization_epoch.clone(),
                target: CommandTarget::new(
                    if c.target_kind == "pull_request" {
                        CommandTargetKind::PullRequest
                    } else {
                        CommandTargetKind::Issue
                    },
                    c.target_id.clone(),
                    c.repository_id.clone(),
                )?,
                payload: p,
                guards: vec![],
                dependencies: vec![],
            })?
        } else {
            let account = account_in(&mut tx, &r.context.account_id, true).await?;
            let snapshot = snapshot_in(&mut tx, &account, &r.context.subject_id).await?;
            if snapshot.context.as_ref() != Some(&r.context)
                || snapshot.generation != r.draft_generation
                || snapshot.availability != CommentSendAvailability::Available
            {
                return Err(stale());
            }
            let frame = capture_in(&mut tx, &account, &r.context.subject_id).await?;
            let p = n::Payload {
                request: r.clone(),
                body: snapshot.body,
                repository_native: frame.repository.provider_id.clone(),
                subject_native: frame.subject.provider_id.clone(),
                number: frame.subject.number.clone().ok_or_else(n::invalid)?,
            };
            record_change(
                &mut tx,
                &account.id,
                positive_revision(&account.authorization_epoch)?,
                &format!("comment_draft:{}", r.context.subject_id),
                false,
            )
            .await?;
            seal(p, &frame)?
        };
        let receipt = command_admission::admit_in(&mut tx, &submission, &Admission)
            .await
            .map_err(super::text_edits::admission_error)?;
        if !receipt.duplicate {
            let p = n::decode_submission(&submission)?;
            sqlx::query("INSERT INTO comment_submissions VALUES(?,?,?,?,?,?)")
                .bind(&r.context.account_id)
                .bind(&r.context.subject_id)
                .bind(n::revision(&r.draft_generation, true)?)
                .bind(&r.command_id)
                .bind(submission.submission_hash().as_slice())
                .bind(n::body_hash(&p.body))
                .execute(&mut *tx)
                .await
                .map_err(storage_error)?;
        }
        tx.commit().await.map_err(storage_error)?;
        Ok(CommentSubmissionReceipt {
            account_id: receipt.account_id,
            command_id: receipt.command_id,
            admitted_revision: receipt.admitted_revision,
            duplicate: receipt.duplicate,
        })
    }
    pub async fn created_comments(&self, q: CreatedCommentQuery) -> Result<CreatedCommentPage> {
        n::identifier(&q.account_id)?;
        n::identifier(&q.subject_id)?;
        if q.limit == 0 || q.limit > 100 {
            return Err(n::invalid());
        }
        let mut tx = self.inner.readers.begin().await.map_err(storage_error)?;
        let account = account_in(&mut tx, &q.account_id, true).await?;
        let current = capture_in(&mut tx, &account, &q.subject_id).await?;
        let (revision, view) = metadata(&mut tx).await?;
        let after = if let Some(cursor) = q.cursor {
            if cursor.len() > 4096 {
                return Err(n::invalid());
            }
            let c: HistoryCursor = decode(&cursor)?;
            if c.account != q.account_id
                || c.subject != q.subject_id
                || c.view != view
                || c.revision != revision
            {
                return Err(stale());
            }
            c.before
        } else {
            i64::MAX
        };
        let rows=sqlx::query("SELECT s.draft_generation,e.payload,c.payload_bytes,c.command_id,c.authorization_epoch FROM comment_submissions s JOIN commands c USING(account_id,command_id) JOIN delivery_resolutions r ON r.account_id=c.account_id AND r.command_id=c.command_id AND r.purpose='confirmed' JOIN command_evidence e ON e.account_id=r.account_id AND e.command_id=r.command_id AND e.ordinal=r.evidence_ordinal WHERE s.account_id=? AND s.subject_id=? AND s.draft_generation<? AND c.state='confirmed' AND e.kind='github.comment_created' AND e.version=1 ORDER BY s.draft_generation DESC LIMIT ?").bind(&q.account_id).bind(&q.subject_id).bind(after).bind(i64::from(q.limit)+1).fetch_all(&mut *tx).await.map_err(storage_error)?;
        let more = rows.len() > q.limit as usize;
        let mut comments = vec![];
        let mut last = 0;
        for row in rows.into_iter().take(q.limit as usize) {
            let e: n::ReceiptEvidence = n::decode_json(&row.get::<Vec<u8>, _>("payload"))?;
            let payload = n::decode_parts(
                &row.get::<Vec<u8>, _>("payload_bytes"),
                &q.account_id,
                &row.get::<String, _>("command_id"),
                &q.subject_id,
                &row.get::<i64, _>("authorization_epoch").to_string(),
            )?;
            if !n::receipt_matches(&e, &payload)
                || e.preparation.actor != account.actor_id
                || e.preparation.frame.repository.provider_id != current.repository.provider_id
                || e.preparation.frame.subject.provider_id != current.subject.provider_id
            {
                return Err(stale());
            }
            last = row.get("draft_generation");
            comments.push(e.receipt)
        }
        let cursor = if more {
            Some(encode(&HistoryCursor {
                account: q.account_id.clone(),
                subject: q.subject_id.clone(),
                view: view.clone(),
                revision: revision.clone(),
                before: last,
            })?)
        } else {
            None
        };
        tx.commit().await.map_err(storage_error)?;
        Ok(CreatedCommentPage {
            account_id: q.account_id,
            subject_id: q.subject_id,
            comments,
            next_cursor: cursor,
            revision,
            authorization_view: view,
        })
    }
}
pub(crate) fn validate_save(r: &SaveCommentDraftRequest) -> Result<()> {
    n::identifier(&r.account_id)?;
    n::identifier(&r.subject_id)?;
    n::revision(&r.authorization_epoch, true)?;
    n::revision(&r.authorization_view, false)?;
    n::revision(&r.expected_generation, false)?;
    n::validate_body(&r.body)
}
#[derive(Serialize, Deserialize)]
struct HistoryCursor {
    account: String,
    subject: String,
    view: String,
    revision: String,
    before: i64,
}
pub(crate) async fn prepare_in(
    tx: &mut Transaction<'_, Sqlite>,
    command: &DeliveryCommand,
    account: &RemoteAccount,
) -> Result<Vec<u8>> {
    let p = n::decode(command)?;
    let f = capture_in(tx, account, &command.target_id).await?;
    if f.repository.provider_id != p.repository_native
        || f.subject.provider_id != p.subject_native
        || f.subject.number.as_deref() != Some(p.number.as_str())
    {
        return Err(stale());
    }
    n::encode(&f)
}

pub(crate) async fn finalize_in(
    tx: &mut Transaction<'_, Sqlite>,
    command: &DeliveryCommand,
    e: &n::ReceiptEvidence,
) -> Result<()> {
    let account = account_in(tx, &command.account_id, true).await?;
    let current = capture_in(tx, &account, &command.target_id).await?;
    if current.authorization_view != e.preparation.frame.authorization_view
        || e.preparation.actor != account.actor_id
        || e.preparation.epoch != account.authorization_epoch
        || current.repository.provider_id != e.preparation.frame.repository.provider_id
        || current.subject.provider_id != e.preparation.frame.subject.provider_id
    {
        return Err(stale());
    }
    record_change(
        tx,
        &account.id,
        positive_revision(&account.authorization_epoch)?,
        &format!("created_comments:{}", command.target_id),
        false,
    )
    .await?;
    Ok(())
}

#[derive(Serialize, Deserialize)]
struct DraftCursor {
    account: String,
    view: String,
    revision: String,
    after: String,
}
impl Store {
    pub async fn comment_drafts(&self, q: CommentDraftQuery) -> Result<CommentDraftPage> {
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
            n::identifier(&c.after)?;
            c.after
        } else {
            String::new()
        };
        let rows=sqlx::query("SELECT subject_id,substr(body,1,256) AS preview,generation FROM comment_drafts WHERE account_id=? AND subject_id>? ORDER BY subject_id LIMIT ?").bind(&q.account_id).bind(after).bind(i64::from(q.limit)+1).fetch_all(&mut *tx).await.map_err(storage_error)?;
        let more = rows.len() > q.limit as usize;
        let drafts: Vec<CommentDraftSummary> = rows
            .into_iter()
            .take(q.limit as usize)
            .map(|r| CommentDraftSummary {
                subject_id: r.get("subject_id"),
                preview: r.get("preview"),
                generation: r.get::<i64, _>("generation").to_string(),
            })
            .collect();
        let next_cursor = if more {
            Some(encode(&DraftCursor {
                account: q.account_id.clone(),
                view: view.clone(),
                revision: revision.clone(),
                after: drafts.last().ok_or_else(n::invalid)?.subject_id.clone(),
            })?)
        } else {
            None
        };
        tx.commit().await.map_err(storage_error)?;
        Ok(CommentDraftPage {
            account_id: q.account_id,
            drafts,
            next_cursor,
            revision,
            authorization_view: view,
        })
    }
}
