//! GitLab approvals and discussions through shared saved review observations.
use super::*;
use crate::{
    GitlabDiscussionNoteV1, GitlabReviewLineRangeV1, GitlabReviewLineV1, GitlabReviewPositionV1,
    NativeDetailPayload, ReviewActor, ReviewContext, ReviewDecision, ReviewRequest,
    ReviewThreadNativeV1, ReviewThreadV1, ReviewV1,
};
use serde::{Deserialize, Serialize};
mod wire;

const MAX_PAGES: u32 = 20;
const REVIEW_SOURCE: &str = "gitlab/approval-observations/v4";
const THREAD_SOURCE: &str = "gitlab/discussion-notes/v4";
const REVIEW_FIELDS: [DetailField; 6] = [
    DetailField::Body,
    DetailField::Author,
    DetailField::State,
    DetailField::UpdatedAt,
    DetailField::HeadOid,
    DetailField::Review,
];
const THREAD_FIELDS: [DetailField; 5] = [
    DetailField::Body,
    DetailField::Author,
    DetailField::UpdatedAt,
    DetailField::HeadOid,
    DetailField::ReviewThread,
];

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Cursor {
    version: u8,
    account: String,
    actor: String,
    epoch: String,
    repository: String,
    project: u64,
    subject: String,
    subject_native: u64,
    iid: u64,
    context: ReviewContext,
    pages: u32,
    url: String,
}
fn identity(request: &ReviewRequest) -> Result<(u64, u64, u64), ProviderError> {
    let detail = &request.detail;
    if detail.account.provider != ProviderKind::Gitlab || detail.account.host != "gitlab.com" {
        return Err(ProviderError::new(ProviderErrorKind::Unsupported));
    }
    if detail.account.state != AccountState::Active {
        return Err(ProviderError::new(ProviderErrorKind::Authentication));
    }
    let project = resource_details::repository_identity(&detail.account, &detail.repository)?;
    let iid = detail
        .subject
        .number
        .as_deref()
        .and_then(positive_id)
        .ok_or_else(invalid)?;
    let subject = positive_id(&detail.subject.provider_id).ok_or_else(invalid)?;
    if !matches!(
        detail.facet,
        DetailFacet::ReviewSummaries | DetailFacet::ReviewThreads
    ) || detail.subject.kind != RemoteItemKind::PullRequest
        || detail.subject.account_id != detail.account.id
        || detail.subject.repository_id.as_ref() != Some(&detail.repository.id)
        || detail.subject.head_oid.as_ref() != Some(&request.context.head_oid)
        || !request.context.is_valid()
        || request.context.base_repository_provider_id != detail.repository.provider_id
        || positive_id(&detail.account.authorization_epoch).is_none()
        || positive_id(&detail.account.actor_id).is_none()
        || positive_id(&request.context.source_repository_provider_id).is_none()
        || detail.etag.is_some()
    {
        return Err(invalid());
    }
    Ok((project, subject, iid))
}
impl Cursor {
    fn open(
        request: &ReviewRequest,
        project: u64,
        subject_native: u64,
        iid: u64,
        http: &GitlabHttp,
    ) -> Result<Self, ProviderError> {
        let detail = &request.detail;
        let Some(raw) = detail.cursor.as_ref() else {
            return Ok(Self {
                version: 1,
                account: detail.account.id.clone(),
                actor: detail.account.actor_id.clone(),
                epoch: detail.account.authorization_epoch.clone(),
                repository: detail.repository.id.clone(),
                project,
                subject: detail.subject.id.clone(),
                subject_native,
                iid,
                context: request.context.clone(),
                pages: 0,
                url: http.discussions(project, iid)?.to_string(),
            });
        };
        if raw.len() > 4096 {
            return Err(invalid());
        }
        let cursor: Self = serde_json::from_str(raw).map_err(|_| invalid())?;
        if cursor.version != 1
            || cursor.account != detail.account.id
            || cursor.actor != detail.account.actor_id
            || cursor.epoch != detail.account.authorization_epoch
            || cursor.repository != detail.repository.id
            || cursor.project != project
            || cursor.subject != detail.subject.id
            || cursor.subject_native != subject_native
            || cursor.iid != iid
            || cursor.context != request.context
            || cursor.pages == 0
            || cursor.pages >= MAX_PAGES
        {
            return Err(invalid());
        }
        http.discussion_continuation(&cursor.url, project, iid, u64::from(cursor.pages) + 1)?;
        Ok(cursor)
    }
    fn encoded(&self) -> Result<String, ProviderError> {
        let raw = serde_json::to_string(self).map_err(|_| invalid())?;
        if raw.len() > 4096 {
            Err(invalid())
        } else {
            Ok(raw)
        }
    }
}
fn actor(value: crate::DetailActor) -> ReviewActor {
    ReviewActor {
        provider_id: value.provider_id,
        login: Some(value.login),
        display_name: None,
    }
}
fn line(value: wire::LinePosition) -> GitlabReviewLineV1 {
    GitlabReviewLineV1 {
        line_code: value.line_code,
        kind: value.kind,
        old_line: value.old_line,
        new_line: value.new_line,
    }
}
fn position(value: wire::Position) -> GitlabReviewPositionV1 {
    GitlabReviewPositionV1 {
        position_type: value.kind,
        base_oid: value.base_sha,
        start_oid: value.start_sha,
        head_oid: value.head_sha,
        old_path: value.old_path,
        new_path: value.new_path,
        old_line: value.old_line,
        new_line: value.new_line,
        line_range: value.range.map(|value| GitlabReviewLineRangeV1 {
            start: line(value.start),
            end: line(value.end),
        }),
        width: value.width,
        height: value.height,
        x: value.x,
        y: value.y,
    }
}
fn approval(value: wire::Approver, context: &ReviewContext) -> DetailEntry {
    let id = positive_id(&value.actor.provider_id).expect("validated native approver");
    DetailEntry {
        id: format!("gitlab-approval:{id:020}"),
        provider_id: format!("approval:{id}"),
        author: Some(value.actor.login.clone()),
        title: None,
        state: Some("approved".into()),
        body: DetailValue {
            state: DetailValueState::Omitted,
            text: None,
        },
        observed_body_state: DetailValueState::Omitted,
        updated_at: value.approved_at.clone(),
        head_oid: Some(context.head_oid.clone()),
        native: Some(NativeDetailPayload::ReviewV1(ReviewV1 {
            context: context.clone(),
            reviewer: Some(actor(value.actor)),
            decision: ReviewDecision::Approved,
            provider_state: "approved".into(),
            reviewed_commit_oid: None,
            submitted_at: value.approved_at,
        })),
        field_mask: REVIEW_FIELDS.into(),
        field_validations: vec![],
    }
}
fn discussion(value: wire::Discussion, context: &ReviewContext) -> Vec<DetailEntry> {
    let retained = value.notes.len() as u32;
    value
        .notes
        .into_iter()
        .map(|note| {
            let id = positive_id(&note.id).expect("validated native note");
            DetailEntry {
                id: format!("gitlab-discussion:{}:{id:020}", value.id),
                provider_id: note.id.clone(),
                author: note.author.as_ref().map(|a| a.login.clone()),
                title: None,
                state: None,
                observed_body_state: note.body.state,
                body: note.body,
                updated_at: Some(note.updated_at.clone()),
                head_oid: Some(context.head_oid.clone()),
                native: Some(NativeDetailPayload::ReviewThreadV1(ReviewThreadV1 {
                    context: context.clone(),
                    thread_id: value.id.clone(),
                    root_comment_id: None,
                    comment_id: note.id,
                    parent_comment_id: None,
                    review_id: None,
                    author: note.author.map(actor),
                    created_at: note.created_at,
                    updated_at: note.updated_at,
                    anchor: None,
                    provider_outdated: None,
                    provider_resolved: note.resolved,
                    native: Some(Box::new(ReviewThreadNativeV1::Gitlab(
                        GitlabDiscussionNoteV1 {
                            note_type: note.kind,
                            system: note.system,
                            individual_note: value.individual_note,
                            resolvable: note.resolvable,
                            resolved_at: note.resolved_at,
                            resolved_by: note.resolved_by.map(actor),
                            position: note.position.map(position),
                            observed_note_count: value.note_count,
                            retained_note_count: retained,
                        },
                    ))),
                })),
                field_mask: THREAD_FIELDS.into(),
                field_validations: vec![],
            }
        })
        .collect()
}
fn page(
    request: &ReviewRequest,
    entries: Vec<DetailEntry>,
    full: bool,
    next_cursor: Option<String>,
    cooldown: Option<u64>,
) -> DetailPage {
    let reviews = request.detail.facet == DetailFacet::ReviewSummaries;
    DetailPage {
        reconciliation: DetailReconciliation {
            enumeration: if full {
                DetailEnumeration::FullEnumeration
            } else {
                DetailEnumeration::Uncertain
            },
            head_scope: DetailHeadScope::CurrentHead,
        },
        body: DetailValue::default(),
        metadata: None,
        entries,
        source: DetailSource {
            source: if reviews {
                REVIEW_SOURCE
            } else {
                THREAD_SOURCE
            }
            .into(),
            adapter_version: 1,
            field_mask: if reviews {
                REVIEW_FIELDS.to_vec()
            } else {
                THREAD_FIELDS.to_vec()
            },
            provider_updated_at: None,
            observed_at: chrono::Utc::now().to_rfc3339(),
        },
        next_cursor,
        etag: None,
        not_modified: false,
        freshness_seconds: 120,
        cooldown_seconds: cooldown,
    }
}
impl GitlabProvider {
    pub(super) async fn request_reviews(
        &self,
        token: &SecretToken,
        request: ReviewRequest,
    ) -> Result<DetailPage, ProviderError> {
        let (project, subject, iid) = identity(&request)?;
        if request.detail.facet == DetailFacet::ReviewSummaries {
            if request.detail.cursor.is_some() {
                return Err(invalid());
            }
            let response = self
                .http
                .get(self.http.approvals(project, iid)?, token)
                .await?;
            let facts = wire::approvals(&response.body, project, subject, iid)
                .map_err(|e| with_quota(e, response.cooldown))?;
            let entries = facts
                .approvers
                .into_iter()
                .map(|value| approval(value, &request.context))
                .collect();
            return Ok(page(
                &request,
                entries,
                !facts.truncated,
                None,
                response.cooldown,
            ));
        }
        let mut cursor = Cursor::open(&request, project, subject, iid, &self.http)?;
        let response = self
            .http
            .get(
                reqwest::Url::parse(&cursor.url).map_err(|_| invalid())?,
                token,
            )
            .await?;
        let result = (|| {
            let rows = wire::discussions(&response.body, project, subject, iid)?;
            let truncated = rows
                .iter()
                .any(|row| row.note_count as usize > row.notes.len());
            cursor.pages += 1;
            let full = cursor.pages == 1 && response.next.is_none() && !truncated;
            let next =
                if let Some(next) = response.next.as_ref().filter(|_| cursor.pages < MAX_PAGES) {
                    cursor.url = next.clone();
                    Some(cursor.encoded()?)
                } else {
                    None
                };
            let entries = rows
                .into_iter()
                .flat_map(|value| discussion(value, &request.context))
                .collect();
            Ok(page(&request, entries, full, next, response.cooldown))
        })();
        result.map_err(|e| with_quota(e, response.cooldown))
    }
}

#[cfg(test)]
mod tests;
