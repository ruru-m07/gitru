//! Durable authored pull-request review drafts and exact submission receipts.
//!
//! Provider-derived anchors are optional in local snapshots so authored text can
//! survive account reset without retaining provider path or commit authority.

use crate::{PullFileContext, ReviewContext, ReviewDiffSide};
use serde::{Deserialize, Serialize};

pub(crate) mod anchors;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewSubmissionEvent {
    Comment,
    Approve,
    RequestChanges,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewSubmissionAvailability {
    Available,
    Unavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewSubmissionReason {
    UnsupportedProvider,
    AccountUnavailable,
    MissingTarget,
    StaleContext,
    MissingProviderDiff,
    InvalidAnchor,
    EmptyRequiredBody,
    PendingSubmission,
    AlreadySubmitted,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReviewDraftKey {
    pub account_id: String,
    pub subject_id: String,
}

/// Renderer-selected coordinates. Native storage resolves `file_key` through the
/// exact current provider artifact and derives the trusted provider path.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReviewDraftAnchorSelection {
    pub file_facet_revision: String,
    pub context: PullFileContext,
    pub file_key: String,
    pub start_line: Option<u32>,
    pub line: u32,
    pub start_side: Option<ReviewDiffSide>,
    pub side: ReviewDiffSide,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReviewDraftCommentInput {
    pub comment_id: String,
    pub body: String,
    pub anchor: ReviewDraftAnchorSelection,
}

/// Native-resolved GitHub line anchor. This exists in snapshots only while the
/// current authorization can read the matching provider-derived authority row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GithubReviewLineAnchor {
    pub file_facet_revision: String,
    pub context: PullFileContext,
    pub file_key: String,
    pub path: String,
    pub start_line: Option<u32>,
    pub line: u32,
    pub start_side: Option<ReviewDiffSide>,
    pub side: ReviewDiffSide,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "provider", content = "anchor", rename_all = "snake_case")]
pub enum ReviewDraftAnchor {
    Github(GithubReviewLineAnchor),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReviewDraftComment {
    pub comment_id: String,
    pub body: String,
    pub anchor: Option<ReviewDraftAnchor>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReviewSubmissionContext {
    pub account_id: String,
    pub subject_id: String,
    pub authorization_epoch: String,
    pub authorization_view: String,
    pub review_context: ReviewContext,
    pub review_token: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReviewSubmissionStatus {
    pub command_id: String,
    pub draft_generation: String,
    pub state: String,
    pub attempt_count: u32,
    pub quarantined: bool,
    pub attention: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReviewDraftSnapshot {
    pub key: ReviewDraftKey,
    pub event: ReviewSubmissionEvent,
    pub body: String,
    pub comments: Vec<ReviewDraftComment>,
    /// Zero denotes a draft that has not been saved yet.
    pub generation: String,
    pub context: Option<ReviewSubmissionContext>,
    pub availability: ReviewSubmissionAvailability,
    pub reason: Option<ReviewSubmissionReason>,
    pub submission: Option<ReviewSubmissionStatus>,
    pub revision: String,
    pub authorization_view: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SaveReviewDraftRequest {
    pub key: ReviewDraftKey,
    pub authorization_epoch: String,
    pub authorization_view: String,
    pub expected_generation: String,
    pub event: ReviewSubmissionEvent,
    pub body: String,
    pub comments: Vec<ReviewDraftCommentInput>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SubmitReviewRequest {
    pub context: ReviewSubmissionContext,
    pub draft_generation: String,
    pub command_id: String,
    pub accept_background_delivery: bool,
    pub accept_best_effort_race: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReviewSubmissionReceipt {
    pub account_id: String,
    pub command_id: String,
    pub admitted_revision: String,
    pub duplicate: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReviewDraftQuery {
    pub account_id: String,
    pub cursor: Option<String>,
    pub limit: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReviewDraftSummary {
    pub subject_id: String,
    pub event: ReviewSubmissionEvent,
    pub preview: String,
    pub inline_comment_count: u32,
    pub generation: String,
    pub submission: Option<ReviewSubmissionStatus>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReviewDraftPage {
    pub account_id: String,
    pub drafts: Vec<ReviewDraftSummary>,
    pub next_cursor: Option<String>,
    pub revision: String,
    pub authorization_view: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SubmittedReviewQuery {
    pub account_id: String,
    pub subject_id: String,
    pub cursor: Option<String>,
    pub limit: u32,
}

/// Validated receipt history. This is separate from whole Reviews-facet
/// coverage and is returned only under current provider authorization.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SubmittedReviewReceipt {
    pub command_id: String,
    pub draft_generation: String,
    pub provider_id: String,
    pub url: String,
    pub event: ReviewSubmissionEvent,
    pub body: String,
    pub provider_state: String,
    pub reviewed_commit_oid: String,
    pub submitted_at: String,
    pub observed_at: String,
    pub inline_comment_count: u32,
    pub confirmed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SubmittedReviewPage {
    pub account_id: String,
    pub subject_id: String,
    pub reviews: Vec<SubmittedReviewReceipt>,
    pub next_cursor: Option<String>,
    pub revision: String,
    pub authorization_view: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const BASE: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const HEAD: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

    fn context() -> PullFileContext {
        PullFileContext {
            base_oid: BASE.into(),
            head_oid: HEAD.into(),
            merge_base_oid: None,
            base_repository_provider_id: "7".into(),
            source_repository_provider_id: "8".into(),
            body_metadata_facet_revision: "9".into(),
        }
    }

    #[test]
    fn wire_tags_keep_final_review_events_and_native_anchor_explicit() {
        assert_eq!(
            serde_json::to_value(ReviewSubmissionEvent::RequestChanges).unwrap(),
            json!("request_changes")
        );
        let anchor = ReviewDraftAnchor::Github(GithubReviewLineAnchor {
            file_facet_revision: "10".into(),
            context: context(),
            file_key: "provider:1".into(),
            path: "src/lib.rs".into(),
            start_line: None,
            line: 12,
            start_side: None,
            side: ReviewDiffSide::Right,
        });
        let wire = serde_json::to_value(anchor).unwrap();
        assert_eq!(wire["provider"], "github");
        assert_eq!(wire["anchor"]["path"], "src/lib.rs");
        assert_eq!(wire["anchor"]["side"], "right");
    }

    #[test]
    fn authored_snapshot_can_survive_without_provider_authority() {
        let snapshot = ReviewDraftSnapshot {
            key: ReviewDraftKey {
                account_id: "account".into(),
                subject_id: "pull".into(),
            },
            event: ReviewSubmissionEvent::Comment,
            body: "saved locally".into(),
            comments: vec![ReviewDraftComment {
                comment_id: "00000000-0000-4000-8000-000000000001".into(),
                body: "inline text".into(),
                anchor: None,
            }],
            generation: "3".into(),
            context: None,
            availability: ReviewSubmissionAvailability::Unavailable,
            reason: Some(ReviewSubmissionReason::AccountUnavailable),
            submission: None,
            revision: "17".into(),
            authorization_view: "22".into(),
        };
        let wire = serde_json::to_value(snapshot).unwrap();
        assert!(wire["context"].is_null());
        assert!(wire["comments"][0]["anchor"].is_null());
        assert_eq!(wire["comments"][0]["body"], "inline text");
    }

    #[test]
    fn renderer_input_rejects_unreviewed_provider_fields() {
        let value = json!({
            "comment_id": "00000000-0000-4000-8000-000000000001",
            "body": "text",
            "anchor": {
                "file_facet_revision": "10",
                "context": context(),
                "file_key": "provider:1",
                "start_line": null,
                "line": 12,
                "start_side": null,
                "side": "right",
                "path": "renderer/forged.rs"
            }
        });
        assert!(serde_json::from_value::<ReviewDraftCommentInput>(value).is_err());
    }
}
