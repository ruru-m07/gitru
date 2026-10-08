//! Authored review drafts, provider-derived anchor authority and immutable receipts.
use super::*;
use crate::{
    DetailFacet, PullFileDiffRequest, ReviewContext, ReviewDiffSide,
    commands::*,
    delivery::DeliveryCommand,
    review_submission::{anchors::*, native as n, *},
    storage::command_admission::{CommandAdmissionPolicy, CommandProtection},
};
use std::collections::HashSet;

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
        let payload = n::decode_submission(submission)?;
        let frame = capture_in(tx, account, submission.target().id()).await?;
        if frame != frame_for(&payload, &frame)? {
            return Err(stale());
        }
        let draft = draft_in(tx, &account.id, submission.target().id()).await?;
        let authority = authority_in(tx, account, &draft, &frame).await?;
        if payload.request.context != authority.context
            || payload.request.draft_generation != draft.generation.to_string()
            || payload.event != draft.event
            || payload.body != draft.body
            || payload.comments != authority.comments
        {
            return Err(stale());
        }
        Ok(vec![
            CommandProtection::Facet {
                subject_id: submission.target().id().into(),
                facet: DetailFacet::Body,
            },
            CommandProtection::Facet {
                subject_id: submission.target().id().into(),
                facet: DetailFacet::Files,
            },
        ])
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Draft {
    event: ReviewSubmissionEvent,
    body: String,
    comments: Vec<ReviewDraftComment>,
    generation: i64,
}

impl Default for Draft {
    fn default() -> Self {
        Self {
            event: ReviewSubmissionEvent::Comment,
            body: String::new(),
            comments: vec![],
            generation: 0,
        }
    }
}

struct Authority {
    context: ReviewSubmissionContext,
    comments: Vec<n::ResolvedComment>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct DraftCursor {
    account: String,
    view: String,
    revision: String,
    after: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct HistoryCursor {
    account: String,
    subject: String,
    view: String,
    revision: String,
    before: i64,
}

pub(crate) fn validate_key(key: &ReviewDraftKey) -> Result<()> {
    n::identifier(&key.account_id)?;
    n::identifier(&key.subject_id)
}

pub(crate) fn validate_save(request: &SaveReviewDraftRequest) -> Result<()> {
    validate_key(&request.key)?;
    n::revision(&request.authorization_epoch, true)?;
    n::revision(&request.authorization_view, false)?;
    n::revision(&request.expected_generation, false)?;
    if request.body.len() > 16 * 1024 || request.body.contains('\0') || request.comments.len() > 25
    {
        return Err(n::invalid());
    }
    let mut authored = request.body.len();
    let mut ids = HashSet::with_capacity(request.comments.len());
    let mut selections = HashSet::with_capacity(request.comments.len());
    for comment in &request.comments {
        n::canonical_uuid(&comment.comment_id)?;
        if !ids.insert(comment.comment_id.as_str())
            || comment.body.is_empty()
            || comment.body.len() > 16 * 1024
            || comment.body.contains('\0')
            || comment.anchor.line == 0
            || matches!(comment.anchor.side, ReviewDiffSide::Unknown)
            || !selections.insert(encode(&comment.anchor)?)
        {
            return Err(n::invalid());
        }
        authored = authored
            .checked_add(comment.body.len())
            .ok_or_else(n::invalid)?;
    }
    if authored > 128 * 1024 {
        return Err(n::invalid());
    }
    Ok(())
}

async fn draft_in(tx: &mut Transaction<'_, Sqlite>, account: &str, subject: &str) -> Result<Draft> {
    let row = sqlx::query(
        "SELECT event,body,generation FROM review_drafts WHERE account_id=? AND subject_id=?",
    )
    .bind(account)
    .bind(subject)
    .fetch_optional(&mut **tx)
    .await
    .map_err(storage_error)?;
    let Some(row) = row else {
        return Ok(Draft::default());
    };
    let generation: i64 = row.get("generation");
    let authored = sqlx::query(
        "SELECT comment_id,body FROM review_draft_comments WHERE account_id=? AND subject_id=? AND generation=? ORDER BY ordinal",
    )
    .bind(account)
    .bind(subject)
    .bind(generation)
    .fetch_all(&mut **tx)
    .await
    .map_err(storage_error)?;
    let anchors: Option<(i64, String)> = sqlx::query_as(
        "SELECT draft_generation,anchors_json FROM review_draft_authority WHERE account_id=? AND subject_id=?",
    )
    .bind(account)
    .bind(subject)
    .fetch_optional(&mut **tx)
    .await
    .map_err(storage_error)?;
    let anchors = anchors
        .filter(|(saved, _)| *saved == generation)
        .map(|(_, value)| decode::<Vec<GithubReviewLineAnchor>>(&value))
        .transpose()?
        .unwrap_or_default();
    let comments = authored
        .into_iter()
        .enumerate()
        .map(|(index, row)| ReviewDraftComment {
            comment_id: row.get("comment_id"),
            body: row.get("body"),
            anchor: anchors.get(index).cloned().map(ReviewDraftAnchor::Github),
        })
        .collect();
    Ok(Draft {
        event: n::parse_event(row.get("event")).ok_or_else(n::invalid)?,
        body: row.get("body"),
        comments,
        generation,
    })
}

pub(crate) async fn capture_in(
    tx: &mut Transaction<'_, Sqlite>,
    account: &RemoteAccount,
    subject: &str,
) -> Result<n::NativeFrameV1> {
    let current = super::workflow_state::capture_in(tx, account, subject).await?;
    if current.subject.kind != RemoteItemKind::PullRequest || current.base.state != "open" {
        return Err(stale());
    }
    let context = super::pull_commits::review_context_in(tx, &account.id, subject, true).await?;
    if current.base.head.as_deref() != Some(context.head_oid.as_str())
        || current.repository.provider_id != context.base_repository_provider_id
    {
        return Err(stale());
    }
    Ok(n::NativeFrameV1 {
        repository_id: current.repository.id,
        repository_provider_id: current.repository.provider_id,
        repository_full_name: current.repository.full_name,
        subject_id: current.subject.id,
        subject_provider_id: current.subject.provider_id,
        number: current.subject.number.ok_or_else(stale)?,
        authorization_view: current.authorization_view,
        context: n::ReviewContextV1::from(&context),
    })
}

fn frame_for(payload: &n::Payload, current: &n::NativeFrameV1) -> Result<n::NativeFrameV1> {
    if current.repository_provider_id != payload.repository_native
        || current.repository_full_name != payload.repository_full_name
        || current.subject_provider_id != payload.subject_native
        || current.number != payload.number
        || current.subject_id != payload.request.context.subject_id
        || current.review_context() != payload.request.context.review_context
        || current.authorization_view != payload.request.context.authorization_view
    {
        return Err(stale());
    }
    Ok(current.clone())
}

fn context(
    account: &RemoteAccount,
    frame: &n::NativeFrameV1,
    generation: i64,
    comments: &[n::ResolvedComment],
) -> Result<ReviewSubmissionContext> {
    let bytes = n::encode_evidence(&(
        account.id.as_str(),
        account.actor_id.as_str(),
        account.authorization_epoch.as_str(),
        frame,
        generation,
        comments,
    ))?;
    Ok(ReviewSubmissionContext {
        account_id: account.id.clone(),
        subject_id: frame.subject_id.clone(),
        authorization_epoch: account.authorization_epoch.clone(),
        authorization_view: frame.authorization_view.clone(),
        review_context: frame.review_context(),
        review_token: format!("{:x}", Sha256::digest(bytes)),
    })
}

async fn resolve_comments_in(
    tx: &mut Transaction<'_, Sqlite>,
    request: &SaveReviewDraftRequest,
) -> Result<Vec<n::ResolvedComment>> {
    let mut resolved = Vec::with_capacity(request.comments.len());
    for comment in &request.comments {
        let snapshot = super::pull_files::artifact_in(
            tx,
            PullFileDiffRequest {
                account_id: request.key.account_id.clone(),
                authorization_epoch: request.authorization_epoch.clone(),
                subject_id: request.key.subject_id.clone(),
                file_facet_revision: comment.anchor.file_facet_revision.clone(),
                context: comment.anchor.context.clone(),
                file_key: comment.anchor.file_key.clone(),
            },
        )
        .await?;
        let anchor = resolve_github_anchor(request, comment, &snapshot).map_err(|failure| {
            CollaborationError::new(
                ErrorCode::StaleView,
                match failure {
                    AnchorResolutionFailure::StaleContext => {
                        "The selected diff is no longer current"
                    }
                    AnchorResolutionFailure::MissingProviderDiff => {
                        "The provider diff is not saved on this device"
                    }
                    AnchorResolutionFailure::InvalidAnchor => {
                        "The selected review line is not in the saved provider diff"
                    }
                },
            )
        })?;
        resolved.push(n::ResolvedComment {
            comment_id: comment.comment_id.clone(),
            body: comment.body.clone(),
            anchor,
        });
    }
    Ok(resolved)
}

async fn authority_in(
    tx: &mut Transaction<'_, Sqlite>,
    account: &RemoteAccount,
    draft: &Draft,
    frame: &n::NativeFrameV1,
) -> Result<Authority> {
    let row = sqlx::query("SELECT draft_generation,authorization_epoch,authorization_view,context_json,anchors_json FROM review_draft_authority WHERE account_id=? AND subject_id=?")
        .bind(&account.id).bind(&frame.subject_id).fetch_optional(&mut **tx).await.map_err(storage_error)?.ok_or_else(stale)?;
    let saved_context: ReviewContext = decode(row.get("context_json"))?;
    let anchors: Vec<GithubReviewLineAnchor> = decode(row.get("anchors_json"))?;
    if row.get::<i64, _>("draft_generation") != draft.generation
        || row.get::<String, _>("authorization_epoch") != account.authorization_epoch
        || row.get::<String, _>("authorization_view") != frame.authorization_view
        || saved_context != frame.review_context()
        || anchors.len() != draft.comments.len()
    {
        return Err(stale());
    }
    let mut resolved = Vec::with_capacity(anchors.len());
    for (comment, anchor) in draft.comments.iter().zip(anchors) {
        let request = SaveReviewDraftRequest {
            key: ReviewDraftKey {
                account_id: account.id.clone(),
                subject_id: frame.subject_id.clone(),
            },
            authorization_epoch: account.authorization_epoch.clone(),
            authorization_view: frame.authorization_view.clone(),
            expected_generation: draft.generation.to_string(),
            event: draft.event,
            body: draft.body.clone(),
            comments: vec![],
        };
        let input = ReviewDraftCommentInput {
            comment_id: comment.comment_id.clone(),
            body: comment.body.clone(),
            anchor: ReviewDraftAnchorSelection {
                file_facet_revision: anchor.file_facet_revision.clone(),
                context: anchor.context.clone(),
                file_key: anchor.file_key.clone(),
                start_line: anchor.start_line,
                line: anchor.line,
                start_side: anchor.start_side,
                side: anchor.side,
            },
        };
        let snapshot = super::pull_files::artifact_in(
            tx,
            PullFileDiffRequest {
                account_id: account.id.clone(),
                authorization_epoch: account.authorization_epoch.clone(),
                subject_id: frame.subject_id.clone(),
                file_facet_revision: anchor.file_facet_revision.clone(),
                context: anchor.context.clone(),
                file_key: anchor.file_key.clone(),
            },
        )
        .await?;
        if resolve_github_anchor(&request, &input, &snapshot).map_err(|_| stale())? != anchor {
            return Err(stale());
        }
        resolved.push(n::ResolvedComment {
            comment_id: comment.comment_id.clone(),
            body: comment.body.clone(),
            anchor,
        });
    }
    Ok(Authority {
        context: context(account, frame, draft.generation, &resolved)?,
        comments: resolved,
    })
}

async fn submission_in(
    tx: &mut Transaction<'_, Sqlite>,
    account: &str,
    subject: &str,
    generation: i64,
) -> Result<Option<ReviewSubmissionStatus>> {
    let row=sqlx::query("SELECT c.command_id,s.draft_generation,c.state,(SELECT count(*) FROM delivery_attempts a WHERE a.account_id=c.account_id AND a.command_id=c.command_id) AS attempt_count,d.attention,EXISTS(SELECT 1 FROM command_recovery_quarantine q WHERE q.account_id=c.account_id AND q.command_id=c.command_id) AS quarantined FROM review_submissions s JOIN commands c USING(account_id,command_id) JOIN command_delivery d USING(account_id,command_id) WHERE s.account_id=? AND s.subject_id=? AND (s.draft_generation=? OR c.state IN ('queued','sending','retry_wait','accepted','outcome_unknown','conflict')) ORDER BY (s.draft_generation=?) DESC,s.draft_generation DESC LIMIT 1")
        .bind(account).bind(subject).bind(generation).bind(generation).fetch_optional(&mut **tx).await.map_err(storage_error)?;
    Ok(row.map(|row| ReviewSubmissionStatus {
        command_id: row.get("command_id"),
        draft_generation: row.get::<i64, _>("draft_generation").to_string(),
        state: row.get("state"),
        attempt_count: u32::try_from(row.get::<i64, _>("attempt_count")).unwrap_or(u32::MAX),
        quarantined: row.get("quarantined"),
        attention: row.get("attention"),
    }))
}

async fn snapshot_in(
    tx: &mut Transaction<'_, Sqlite>,
    account: &RemoteAccount,
    key: &ReviewDraftKey,
) -> Result<ReviewDraftSnapshot> {
    let draft = draft_in(tx, &account.id, &key.subject_id).await?;
    let submission = submission_in(tx, &account.id, &key.subject_id, draft.generation).await?;
    let (revision, authorization_view) = metadata(tx).await?;
    let mut snapshot = ReviewDraftSnapshot {
        key: key.clone(),
        event: draft.event,
        body: draft.body.clone(),
        comments: draft.comments.clone(),
        generation: draft.generation.to_string(),
        context: None,
        availability: ReviewSubmissionAvailability::Unavailable,
        reason: None,
        submission,
        revision,
        authorization_view,
    };
    snapshot.reason = if account.provider != ProviderKind::Github || account.host != "github.com" {
        Some(ReviewSubmissionReason::UnsupportedProvider)
    } else if account.state != AccountState::Active {
        Some(ReviewSubmissionReason::AccountUnavailable)
    } else if let Some(status) = &snapshot.submission {
        Some(if status.state == "confirmed" {
            ReviewSubmissionReason::AlreadySubmitted
        } else {
            ReviewSubmissionReason::PendingSubmission
        })
    } else if matches!(
        draft.event,
        ReviewSubmissionEvent::Comment | ReviewSubmissionEvent::RequestChanges
    ) && draft.body.is_empty()
    {
        Some(ReviewSubmissionReason::EmptyRequiredBody)
    } else {
        match capture_in(tx, account, &key.subject_id).await {
            Ok(frame) => match authority_in(tx, account, &draft, &frame).await {
                Ok(authority) => {
                    snapshot.context = Some(authority.context);
                    snapshot.comments = draft
                        .comments
                        .iter()
                        .zip(authority.comments)
                        .map(|(authored, resolved)| ReviewDraftComment {
                            comment_id: authored.comment_id.clone(),
                            body: authored.body.clone(),
                            anchor: Some(ReviewDraftAnchor::Github(resolved.anchor)),
                        })
                        .collect();
                    snapshot.availability = ReviewSubmissionAvailability::Available;
                    None
                }
                Err(error) if matches!(error.code, ErrorCode::NotFound) => {
                    Some(ReviewSubmissionReason::MissingProviderDiff)
                }
                Err(error)
                    if matches!(
                        error.code,
                        ErrorCode::StaleView | ErrorCode::PermissionDenied
                    ) =>
                {
                    Some(if draft.comments.is_empty() {
                        ReviewSubmissionReason::StaleContext
                    } else {
                        ReviewSubmissionReason::MissingProviderDiff
                    })
                }
                Err(error) => return Err(error),
            },
            Err(error)
                if matches!(
                    error.code,
                    ErrorCode::NotFound | ErrorCode::StaleView | ErrorCode::PermissionDenied
                ) =>
            {
                Some(ReviewSubmissionReason::MissingTarget)
            }
            Err(error) => return Err(error),
        }
    };
    Ok(snapshot)
}

pub(crate) fn seal(payload: n::Payload, repository_id: &str) -> Result<CommandSubmission> {
    seal_command(CommandDraft {
        command_id: payload.request.command_id.clone(),
        account_id: payload.request.context.account_id.clone(),
        authorization_epoch: payload.request.context.authorization_epoch.clone(),
        target: CommandTarget::new(
            CommandTargetKind::PullRequest,
            payload.request.context.subject_id.clone(),
            Some(repository_id.into()),
        )?,
        payload,
        guards: vec![],
        dependencies: vec![],
    })
}

impl Store {
    pub async fn review_draft(&self, key: ReviewDraftKey) -> Result<ReviewDraftSnapshot> {
        validate_key(&key)?;
        let mut tx = self.inner.readers.begin().await.map_err(storage_error)?;
        let account = account_in(&mut tx, &key.account_id, false).await?;
        let snapshot = snapshot_in(&mut tx, &account, &key).await?;
        tx.commit().await.map_err(storage_error)?;
        Ok(snapshot)
    }

    pub(crate) async fn save_review_draft(
        &self,
        request: SaveReviewDraftRequest,
    ) -> Result<ReviewDraftSnapshot> {
        validate_save(&request)?;
        let mut writer = self.inner.writer.acquire().await?;
        let mut tx = writer.begin().await.map_err(storage_error)?;
        let account = account_in(&mut tx, &request.key.account_id, true).await?;
        let (_, view) = metadata(&mut tx).await?;
        if account.authorization_epoch != request.authorization_epoch
            || view != request.authorization_view
        {
            return Err(stale());
        }
        let old = draft_in(&mut tx, &account.id, &request.key.subject_id).await?;
        if old.generation != n::revision(&request.expected_generation, false)? {
            return Err(stale());
        }
        let frame = capture_in(&mut tx, &account, &request.key.subject_id).await?;
        let resolved = resolve_comments_in(&mut tx, &request).await?;
        if resolved.iter().any(|comment| {
            comment.anchor.context.base_oid != frame.context.base_oid
                || comment.anchor.context.head_oid != frame.context.head_oid
                || comment.anchor.context.base_repository_provider_id
                    != frame.context.base_repository_provider_id
                || comment.anchor.context.source_repository_provider_id
                    != frame.context.source_repository_provider_id
                || comment.anchor.context.body_metadata_facet_revision
                    != frame.context.metadata_facet_revision
        }) {
            return Err(stale());
        }
        let next_comments: Vec<ReviewDraftComment> = resolved
            .iter()
            .map(|comment| ReviewDraftComment {
                comment_id: comment.comment_id.clone(),
                body: comment.body.clone(),
                anchor: Some(ReviewDraftAnchor::Github(comment.anchor.clone())),
            })
            .collect();
        let changed = old.generation == 0
            || old.event != request.event
            || old.body != request.body
            || old.comments != next_comments;
        if changed {
            let generation = old
                .generation
                .checked_add(1)
                .ok_or_else(CollaborationError::storage)?;
            sqlx::query("INSERT INTO review_drafts VALUES(?,?,?,?,?) ON CONFLICT(account_id,subject_id) DO UPDATE SET event=excluded.event,body=excluded.body,generation=excluded.generation")
                .bind(&account.id).bind(&request.key.subject_id).bind(n::event_name(request.event)).bind(&request.body).bind(generation).execute(&mut *tx).await.map_err(storage_error)?;
            sqlx::query("DELETE FROM review_draft_comments WHERE account_id=? AND subject_id=?")
                .bind(&account.id)
                .bind(&request.key.subject_id)
                .execute(&mut *tx)
                .await
                .map_err(storage_error)?;
            for (ordinal, comment) in resolved.iter().enumerate() {
                sqlx::query("INSERT INTO review_draft_comments VALUES(?,?,?,?,?,?)")
                    .bind(&account.id)
                    .bind(&request.key.subject_id)
                    .bind(&comment.comment_id)
                    .bind(i64::try_from(ordinal).map_err(|_| n::invalid())?)
                    .bind(&comment.body)
                    .bind(generation)
                    .execute(&mut *tx)
                    .await
                    .map_err(storage_error)?;
            }
            let anchors: Vec<_> = resolved
                .iter()
                .map(|comment| comment.anchor.clone())
                .collect();
            sqlx::query("INSERT INTO review_draft_authority VALUES(?,?,?,?,?,?,?) ON CONFLICT(account_id,subject_id) DO UPDATE SET draft_generation=excluded.draft_generation,authorization_epoch=excluded.authorization_epoch,authorization_view=excluded.authorization_view,context_json=excluded.context_json,anchors_json=excluded.anchors_json")
                .bind(&account.id).bind(&request.key.subject_id).bind(generation).bind(&account.authorization_epoch).bind(&view).bind(encode(&frame.review_context())?).bind(encode(&anchors)?).execute(&mut *tx).await.map_err(storage_error)?;
            record_change(
                &mut tx,
                &account.id,
                positive_revision(&account.authorization_epoch)?,
                &format!("review_draft:{}", request.key.subject_id),
                false,
            )
            .await?;
        }
        let snapshot = snapshot_in(&mut tx, &account, &request.key).await?;
        tx.commit().await.map_err(storage_error)?;
        Ok(snapshot)
    }

    pub(crate) async fn submit_review(
        &self,
        request: SubmitReviewRequest,
    ) -> Result<ReviewSubmissionReceipt> {
        n::validate_submit(&request)?;
        let mut writer = self.inner.writer.acquire().await?;
        let mut tx = writer.begin().await.map_err(storage_error)?;
        epoch_in(
            &mut tx,
            &request.context.account_id,
            &request.context.authorization_epoch,
        )
        .await?;
        let duplicate: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM commands WHERE account_id=? AND command_id=?)",
        )
        .bind(&request.context.account_id)
        .bind(&request.command_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(storage_error)?;
        let submission = if duplicate {
            let command =
                delivery::load_in(&mut tx, &request.context.account_id, &request.command_id)
                    .await?;
            if command.operation_kind != n::OPERATION
                || command.payload_version != 1
                || n::decode(&command)?.request != request
            {
                return Err(n::invalid());
            }
            let repository = command.repository_id.clone().ok_or_else(n::invalid)?;
            seal(n::decode(&command)?, &repository)?
        } else {
            let account = account_in(&mut tx, &request.context.account_id, true).await?;
            let key = ReviewDraftKey {
                account_id: account.id.clone(),
                subject_id: request.context.subject_id.clone(),
            };
            let draft = draft_in(&mut tx, &account.id, &key.subject_id).await?;
            if submission_in(&mut tx, &account.id, &key.subject_id, draft.generation)
                .await?
                .is_some()
            {
                return Err(stale());
            }
            let frame = capture_in(&mut tx, &account, &key.subject_id).await?;
            let authority = authority_in(&mut tx, &account, &draft, &frame).await?;
            if authority.context != request.context
                || draft.generation.to_string() != request.draft_generation
            {
                return Err(stale());
            }
            let mut payload = n::Payload {
                request: request.clone(),
                event: draft.event,
                body: draft.body,
                comments: authority.comments,
                content_hash: [0; 32],
                repository_native: frame.repository_provider_id.clone(),
                repository_full_name: frame.repository_full_name.clone(),
                subject_native: frame.subject_provider_id.clone(),
                number: frame.number.clone(),
                actor_id: account.actor_id.clone(),
            };
            payload.content_hash = n::content_hash(&payload)?;
            seal(payload, &frame.repository_id)?
        };
        let receipt = command_admission::admit_in(&mut tx, &submission, &Admission)
            .await
            .map_err(super::workflow_state::admission_error)?;
        if !receipt.duplicate {
            let payload = n::decode_submission(&submission)?;
            sqlx::query("INSERT INTO review_submissions VALUES(?,?,?,?,?,?)")
                .bind(&request.context.account_id)
                .bind(&request.context.subject_id)
                .bind(n::revision(&request.draft_generation, true)?)
                .bind(&request.command_id)
                .bind(submission.submission_hash().as_slice())
                .bind(payload.content_hash.as_slice())
                .execute(&mut *tx)
                .await
                .map_err(storage_error)?;
            record_change(
                &mut tx,
                &request.context.account_id,
                positive_revision(&request.context.authorization_epoch)?,
                &format!("review_draft:{}", request.context.subject_id),
                false,
            )
            .await?;
        }
        tx.commit().await.map_err(storage_error)?;
        Ok(ReviewSubmissionReceipt {
            account_id: receipt.account_id,
            command_id: receipt.command_id,
            admitted_revision: receipt.admitted_revision,
            duplicate: receipt.duplicate,
        })
    }

    pub async fn review_drafts(&self, query: ReviewDraftQuery) -> Result<ReviewDraftPage> {
        n::identifier(&query.account_id)?;
        if query.limit == 0 || query.limit > 100 {
            return Err(n::invalid());
        }
        let mut tx = self.inner.readers.begin().await.map_err(storage_error)?;
        account_in(&mut tx, &query.account_id, false).await?;
        let (revision, authorization_view) = metadata(&mut tx).await?;
        let after = if let Some(cursor) = query.cursor {
            if cursor.len() > 4_096 {
                return Err(n::invalid());
            }
            let cursor: DraftCursor = decode(&cursor)?;
            if cursor.account != query.account_id
                || cursor.view != authorization_view
                || cursor.revision != revision
            {
                return Err(stale());
            }
            n::identifier(&cursor.after)?;
            cursor.after
        } else {
            String::new()
        };
        let rows = sqlx::query(
            "SELECT d.subject_id,d.event,substr(d.body,1,256) AS preview,d.generation,\
             (SELECT count(*) FROM review_draft_comments c WHERE c.account_id=d.account_id AND c.subject_id=d.subject_id) AS inline_count \
             FROM review_drafts d WHERE d.account_id=? AND d.subject_id>? ORDER BY d.subject_id LIMIT ?",
        )
        .bind(&query.account_id)
        .bind(after)
        .bind(i64::from(query.limit) + 1)
        .fetch_all(&mut *tx)
        .await
        .map_err(storage_error)?;
        let more = rows.len() > query.limit as usize;
        let mut drafts = Vec::with_capacity(rows.len().min(query.limit as usize));
        for row in rows.into_iter().take(query.limit as usize) {
            let subject_id: String = row.get("subject_id");
            let generation: i64 = row.get("generation");
            drafts.push(ReviewDraftSummary {
                submission: submission_in(&mut tx, &query.account_id, &subject_id, generation)
                    .await?,
                subject_id,
                event: n::parse_event(row.get("event")).ok_or_else(n::invalid)?,
                preview: row.get("preview"),
                inline_comment_count: u32::try_from(row.get::<i64, _>("inline_count"))
                    .map_err(|_| n::invalid())?,
                generation: generation.to_string(),
            });
        }
        let next_cursor = if more {
            Some(encode(&DraftCursor {
                account: query.account_id.clone(),
                view: authorization_view.clone(),
                revision: revision.clone(),
                after: drafts.last().ok_or_else(n::invalid)?.subject_id.clone(),
            })?)
        } else {
            None
        };
        tx.commit().await.map_err(storage_error)?;
        Ok(ReviewDraftPage {
            account_id: query.account_id,
            drafts,
            next_cursor,
            revision,
            authorization_view,
        })
    }

    pub async fn submitted_reviews(
        &self,
        query: SubmittedReviewQuery,
    ) -> Result<SubmittedReviewPage> {
        n::identifier(&query.account_id)?;
        n::identifier(&query.subject_id)?;
        if query.limit == 0 || query.limit > 100 {
            return Err(n::invalid());
        }
        let mut tx = self.inner.readers.begin().await.map_err(storage_error)?;
        let account = account_in(&mut tx, &query.account_id, true).await?;
        if account.provider != ProviderKind::Github || account.host != "github.com" {
            return Err(stale());
        }
        let current = details::subject_in(&mut tx, &account.id, &query.subject_id).await?;
        if current.kind != RemoteItemKind::PullRequest {
            return Err(stale());
        }
        let repository_id = current.repository_id.as_deref().ok_or_else(stale)?;
        if !identities::accessible(
            &mut tx,
            &account.id,
            repository_id,
            ResourceKind::Repository,
        )
        .await?
        {
            return Err(stale());
        }
        let repository_json: String =
            sqlx::query_scalar("SELECT json FROM repositories WHERE account_id=? AND id=?")
                .bind(&account.id)
                .bind(repository_id)
                .fetch_one(&mut *tx)
                .await
                .map_err(storage_error)?;
        let repository: RemoteRepository = decode(&repository_json)?;
        let (revision, authorization_view) = metadata(&mut tx).await?;
        let before = if let Some(cursor) = query.cursor {
            if cursor.len() > 4_096 {
                return Err(n::invalid());
            }
            let cursor: HistoryCursor = decode(&cursor)?;
            if cursor.account != query.account_id
                || cursor.subject != query.subject_id
                || cursor.view != authorization_view
                || cursor.revision != revision
            {
                return Err(stale());
            }
            cursor.before
        } else {
            i64::MAX
        };
        let rows = sqlx::query(
            "SELECT r.draft_generation,r.command_id,r.provider_id,r.accepted_ordinal,\
             f.confirmed_ordinal \
             FROM review_resolutions r \
             LEFT JOIN review_confirmations f USING(account_id,command_id,provider_id) \
             WHERE r.account_id=? AND r.subject_id=? AND r.draft_generation<? \
             ORDER BY r.draft_generation DESC LIMIT ?",
        )
        .bind(&query.account_id)
        .bind(&query.subject_id)
        .bind(before)
        .bind(i64::from(query.limit) + 1)
        .fetch_all(&mut *tx)
        .await
        .map_err(storage_error)?;
        let more = rows.len() > query.limit as usize;
        let mut reviews = Vec::with_capacity(rows.len().min(query.limit as usize));
        let mut last = 0;
        for row in rows.into_iter().take(query.limit as usize) {
            let command_id: String = row.get("command_id");
            let provider_id: String = row.get("provider_id");
            let command = delivery::load_in(&mut tx, &query.account_id, &command_id).await?;
            let payload = n::decode(&command)?;
            let accepted_ordinal: i64 = row.get("accepted_ordinal");
            let accepted: Vec<u8> = sqlx::query_scalar(
                "SELECT payload FROM command_evidence WHERE account_id=? AND command_id=? AND ordinal=? AND kind=? AND version=1",
            )
            .bind(&query.account_id)
            .bind(&command_id)
            .bind(accepted_ordinal)
            .bind(n::ACCEPTED_PROOF)
            .fetch_one(&mut *tx)
            .await
            .map_err(storage_error)?;
            let accepted: n::AcceptedEvidenceV1 = n::decode_evidence(&accepted)?;
            if !n::accepted_matches(&accepted, &payload, &command)
                || accepted.receipt.provider_id != provider_id
                || accepted.preparation.actor_id != account.actor_id
                || accepted.preparation.subject_provider_id != current.provider_id
                || accepted.preparation.repository_provider_id != repository.provider_id
            {
                return Err(stale());
            }
            let confirmed = if let Some(ordinal) = row.get::<Option<i64>, _>("confirmed_ordinal") {
                let submitted: Vec<u8> = sqlx::query_scalar(
                    "SELECT payload FROM command_evidence WHERE account_id=? AND command_id=? AND ordinal=? AND kind=? AND version=1",
                )
                .bind(&query.account_id)
                .bind(&command_id)
                .bind(ordinal)
                .bind(n::SUBMITTED_PROOF)
                .fetch_one(&mut *tx)
                .await
                .map_err(storage_error)?;
                let submitted: n::SubmittedEvidenceV1 = n::decode_evidence(&submitted)?;
                if !n::submitted_matches(&submitted, &payload, &command)
                    || submitted.receipt != accepted.receipt
                    || submitted.preparation != accepted.preparation
                {
                    return Err(stale());
                }
                true
            } else {
                false
            };
            let generation: i64 = row.get("draft_generation");
            last = generation;
            reviews.push(SubmittedReviewReceipt {
                command_id,
                draft_generation: generation.to_string(),
                provider_id,
                url: accepted.receipt.url,
                event: payload.event,
                body: payload.body,
                provider_state: accepted.receipt.provider_state,
                reviewed_commit_oid: accepted.receipt.reviewed_commit_oid,
                submitted_at: accepted.receipt.submitted_at,
                observed_at: accepted.receipt.observed_at,
                inline_comment_count: u32::try_from(payload.comments.len())
                    .map_err(|_| n::invalid())?,
                confirmed,
            });
        }
        let next_cursor = if more {
            Some(encode(&HistoryCursor {
                account: query.account_id.clone(),
                subject: query.subject_id.clone(),
                view: authorization_view.clone(),
                revision: revision.clone(),
                before: last,
            })?)
        } else {
            None
        };
        tx.commit().await.map_err(storage_error)?;
        Ok(SubmittedReviewPage {
            account_id: query.account_id,
            subject_id: query.subject_id,
            reviews,
            next_cursor,
            revision,
            authorization_view,
        })
    }
}

pub(crate) async fn prepare_in(
    tx: &mut Transaction<'_, Sqlite>,
    command: &DeliveryCommand,
    account: &RemoteAccount,
) -> Result<Vec<u8>> {
    if command.reconcile_only() {
        epoch_in(tx, &account.id, &account.authorization_epoch).await?;
        details::subject_in(tx, &account.id, &command.target_id).await?;
        return n::encode_evidence(&(
            account.id.as_str(),
            account.authorization_epoch.as_str(),
            metadata(tx).await?.1,
        ));
    }
    let payload = n::decode(command)?;
    let frame = capture_in(tx, account, &command.target_id).await?;
    frame_for(&payload, &frame)?;
    let draft = draft_in(tx, &account.id, &command.target_id).await?;
    let authority = authority_in(tx, account, &draft, &frame).await?;
    if authority.context != payload.request.context || authority.comments != payload.comments {
        return Err(stale());
    }
    n::encode_evidence(&frame)
}

pub(crate) async fn validate_frame_in(
    tx: &mut Transaction<'_, Sqlite>,
    command: &DeliveryCommand,
    account: &RemoteAccount,
    frame: &n::NativeFrameV1,
) -> Result<()> {
    let payload = n::decode(command)?;
    let current = capture_in(tx, account, &command.target_id).await?;
    if current != *frame || frame_for(&payload, &current)? != *frame {
        return Err(stale());
    }
    let draft = draft_in(tx, &account.id, &command.target_id).await?;
    let authority = authority_in(tx, account, &draft, &current).await?;
    if authority.context != payload.request.context || authority.comments != payload.comments {
        return Err(stale());
    }
    Ok(())
}

pub(crate) async fn finalize_in(
    tx: &mut Transaction<'_, Sqlite>,
    command: &DeliveryCommand,
    purpose: crate::delivery::EvidencePurpose,
    proof: &crate::delivery::OperationEvidence,
) -> Result<()> {
    let payload = n::decode(command)?;
    let ordinal: i64 = sqlx::query_scalar("SELECT ordinal FROM command_evidence WHERE account_id=? AND command_id=? AND kind=? AND version=? AND payload=? ORDER BY ordinal DESC LIMIT 1")
        .bind(&command.account_id).bind(&command.command_id).bind(&proof.kind).bind(i64::from(proof.version)).bind(&proof.payload).fetch_one(&mut **tx).await.map_err(storage_error)?;
    match purpose {
        crate::delivery::EvidencePurpose::Accepted => {
            let evidence: n::AcceptedEvidenceV1 = n::decode_evidence(&proof.payload)?;
            if !n::accepted_matches(&evidence, &payload, command) {
                return Err(n::invalid());
            }
            sqlx::query("INSERT INTO review_resolutions VALUES(?,?,?,?,?,?,?,?,?,?,?,?)")
                .bind(&command.account_id)
                .bind(&command.target_id)
                .bind(n::revision(&payload.request.draft_generation, true)?)
                .bind(&command.command_id)
                .bind(&evidence.receipt.provider_id)
                .bind(&evidence.receipt.url)
                .bind(n::event_name(payload.event))
                .bind(&evidence.receipt.provider_state)
                .bind(&evidence.receipt.reviewed_commit_oid)
                .bind(&evidence.receipt.submitted_at)
                .bind(&evidence.receipt.observed_at)
                .bind(ordinal)
                .execute(&mut **tx)
                .await
                .map_err(storage_error)?;
        }
        crate::delivery::EvidencePurpose::Confirmed => {
            let evidence: n::SubmittedEvidenceV1 = n::decode_evidence(&proof.payload)?;
            if !n::submitted_matches(&evidence, &payload, command) {
                return Err(n::invalid());
            }
            let accepted_payload: Vec<u8> = sqlx::query_scalar(
                "SELECT e.payload FROM review_resolutions r JOIN command_evidence e ON e.account_id=r.account_id AND e.command_id=r.command_id AND e.ordinal=r.accepted_ordinal WHERE r.account_id=? AND r.command_id=? AND r.provider_id=? AND e.kind=? AND e.version=1",
            )
            .bind(&command.account_id)
            .bind(&command.command_id)
            .bind(&evidence.receipt.provider_id)
            .bind(n::ACCEPTED_PROOF)
            .fetch_one(&mut **tx)
            .await
            .map_err(storage_error)?;
            let accepted: n::AcceptedEvidenceV1 = n::decode_evidence(&accepted_payload)?;
            if !n::accepted_matches(&accepted, &payload, command)
                || accepted.preparation != evidence.preparation
                || accepted.receipt != evidence.receipt
            {
                return Err(n::invalid());
            }
            sqlx::query("INSERT INTO review_confirmations VALUES(?,?,?,?,?,?)")
                .bind(&command.account_id)
                .bind(&command.command_id)
                .bind(&evidence.receipt.provider_id)
                .bind(ordinal)
                .bind(&evidence.confirmed_at)
                .bind(i64::try_from(evidence.comments.len()).map_err(|_| n::invalid())?)
                .execute(&mut **tx)
                .await
                .map_err(storage_error)?;
        }
        _ => return Err(n::invalid()),
    }
    record_change(
        tx,
        &command.account_id,
        positive_revision(&command.authorization_epoch)?,
        &format!("submitted_reviews:{}", command.target_id),
        false,
    )
    .await?;
    record_change(
        tx,
        &command.account_id,
        positive_revision(&command.authorization_epoch)?,
        &format!("review_draft:{}", command.target_id),
        false,
    )
    .await?;
    Ok(())
}
