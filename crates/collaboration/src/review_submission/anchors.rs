//! Resolve renderer-selected coordinates through exact cached provider artifacts.
#![allow(
    dead_code,
    reason = "schema-24 draft storage consumes this frozen anchor resolver"
)]

use super::*;
use crate::{
    PullFileArtifactSnapshot, PullFileArtifactValidation, PullFileContentState,
    PullFileSourceStrategy,
};
use std::collections::BTreeSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AnchorResolutionFailure {
    StaleContext,
    MissingProviderDiff,
    InvalidAnchor,
}

/// The renderer supplies only an opaque file key and coordinates. This function
/// accepts a provider path only after storage has resolved the exact current
/// membership and provider-validated text artifact for that selection.
pub(crate) fn resolve_github_anchor(
    request: &SaveReviewDraftRequest,
    comment: &ReviewDraftCommentInput,
    snapshot: &PullFileArtifactSnapshot,
) -> Result<GithubReviewLineAnchor, AnchorResolutionFailure> {
    if snapshot.request.account_id != request.key.account_id
        || snapshot.request.authorization_epoch != request.authorization_epoch
        || snapshot.request.subject_id != request.key.subject_id
        || snapshot.request.file_facet_revision != comment.anchor.file_facet_revision
        || snapshot.request.context != comment.anchor.context
        || snapshot.request.file_key != comment.anchor.file_key
        || snapshot.authorization_view != request.authorization_view
        || snapshot.membership.authorization_view != request.authorization_view
        || !snapshot.membership.is_exact_for(&snapshot.request)
        || snapshot.membership.file_facet_revision != comment.anchor.file_facet_revision
        || snapshot.membership.context != comment.anchor.context
        || snapshot.membership.file_key != comment.anchor.file_key
    {
        return Err(AnchorResolutionFailure::StaleContext);
    }

    let artifact = snapshot
        .artifact
        .as_ref()
        .ok_or(AnchorResolutionFailure::MissingProviderDiff)?;
    if artifact.validate().is_err()
        || artifact.account_id != snapshot.membership.account_id
        || artifact.authorization_epoch != snapshot.membership.authorization_epoch
        || artifact.authorization_view != snapshot.membership.authorization_view
        || artifact.subject_id != snapshot.membership.subject_id
        || artifact.generation != snapshot.membership.generation
        || artifact.file_key != snapshot.membership.file_key
        || artifact.identity != snapshot.membership.identity
        || artifact.context != snapshot.membership.context
    {
        return Err(AnchorResolutionFailure::StaleContext);
    }
    if artifact.content_state != PullFileContentState::Text
        || artifact.validation.as_ref().is_none_or(|validation| {
            !matches!(validation, PullFileArtifactValidation::Provider { .. })
        })
        || artifact.source.as_ref().is_none_or(|source| {
            !matches!(
                source.strategy,
                PullFileSourceStrategy::GithubPullFiles | PullFileSourceStrategy::GithubPullDiff
            )
        })
    {
        return Err(AnchorResolutionFailure::MissingProviderDiff);
    }
    let text = artifact
        .unified_text
        .as_deref()
        .ok_or(AnchorResolutionFailure::MissingProviderDiff)?;
    let path = match comment.anchor.side {
        ReviewDiffSide::Left => snapshot.membership.identity.old_path.as_deref(),
        ReviewDiffSide::Right => snapshot.membership.identity.new_path.as_deref(),
        ReviewDiffSide::Unknown => None,
    }
    .ok_or(AnchorResolutionFailure::InvalidAnchor)?;
    if !valid_range(
        text,
        comment.anchor.start_line,
        comment.anchor.line,
        comment.anchor.start_side,
        comment.anchor.side,
    ) {
        return Err(AnchorResolutionFailure::InvalidAnchor);
    }
    Ok(GithubReviewLineAnchor {
        file_facet_revision: comment.anchor.file_facet_revision.clone(),
        context: comment.anchor.context.clone(),
        file_key: comment.anchor.file_key.clone(),
        path: path.into(),
        start_line: comment.anchor.start_line,
        line: comment.anchor.line,
        start_side: comment.anchor.start_side,
        side: comment.anchor.side,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct HunkHeader {
    old_start: u32,
    old_count: u32,
    new_start: u32,
    new_count: u32,
}

#[derive(Default)]
struct HunkLines {
    left: BTreeSet<u32>,
    right: BTreeSet<u32>,
}

fn valid_range(
    patch: &str,
    start_line: Option<u32>,
    line: u32,
    start_side: Option<ReviewDiffSide>,
    side: ReviewDiffSide,
) -> bool {
    if line == 0 || !matches!(side, ReviewDiffSide::Left | ReviewDiffSide::Right) {
        return false;
    }
    let start = match (start_line, start_side) {
        (None, None) => line,
        (Some(start), Some(start_side)) if start > 0 && start < line && start_side == side => start,
        _ => return false,
    };
    let mut lines = patch.lines().peekable();
    let mut matches = 0u32;
    while let Some(line_text) = lines.next() {
        let Some(header) = parse_hunk_header(line_text) else {
            continue;
        };
        let mut old = header.old_start;
        let mut new = header.new_start;
        let mut consumed_old = 0u32;
        let mut consumed_new = 0u32;
        let mut hunk = HunkLines::default();
        while let Some(next) = lines.peek().copied() {
            if next.starts_with("@@ ") {
                break;
            }
            lines.next();
            let Some(prefix) = next.as_bytes().first().copied() else {
                return false;
            };
            match prefix {
                b' ' => {
                    if consumed_old >= header.old_count || consumed_new >= header.new_count {
                        return false;
                    }
                    hunk.left.insert(old);
                    hunk.right.insert(new);
                    let (Some(next_old), Some(next_new)) = (old.checked_add(1), new.checked_add(1))
                    else {
                        return false;
                    };
                    old = next_old;
                    new = next_new;
                    consumed_old += 1;
                    consumed_new += 1;
                }
                b'-' => {
                    if consumed_old >= header.old_count {
                        return false;
                    }
                    hunk.left.insert(old);
                    let Some(next_old) = old.checked_add(1) else {
                        return false;
                    };
                    old = next_old;
                    consumed_old += 1;
                }
                b'+' => {
                    if consumed_new >= header.new_count {
                        return false;
                    }
                    hunk.right.insert(new);
                    let Some(next_new) = new.checked_add(1) else {
                        return false;
                    };
                    new = next_new;
                    consumed_new += 1;
                }
                b'\\' if next == "\\ No newline at end of file" => {}
                _ => return false,
            }
        }
        if consumed_old != header.old_count || consumed_new != header.new_count {
            return false;
        }
        let selected = if side == ReviewDiffSide::Left {
            &hunk.left
        } else {
            &hunk.right
        };
        let width = u64::from(line) - u64::from(start) + 1;
        if selected.range(start..=line).count() as u64 == width {
            matches += 1;
        }
    }
    matches == 1
}

fn parse_hunk_header(line: &str) -> Option<HunkHeader> {
    let rest = line.strip_prefix("@@ -")?;
    let (old, rest) = rest.split_once(" +")?;
    let (new, suffix) = rest.split_once(" @@")?;
    if !suffix.is_empty() && !suffix.starts_with(' ') {
        return None;
    }
    let (old_start, old_count) = parse_range(old)?;
    let (new_start, new_count) = parse_range(new)?;
    Some(HunkHeader {
        old_start,
        old_count,
        new_start,
        new_count,
    })
}

fn parse_range(raw: &str) -> Option<(u32, u32)> {
    let (start, count) = raw
        .split_once(',')
        .map_or((raw, "1"), |(start, count)| (start, count));
    if start.is_empty()
        || count.is_empty()
        || !start.bytes().all(|byte| byte.is_ascii_digit())
        || !count.bytes().all(|byte| byte.is_ascii_digit())
    {
        return None;
    }
    let start = start.parse::<u32>().ok()?;
    let count = count.parse::<u32>().ok()?;
    if count > 0 && start == 0 {
        return None;
    }
    Some((start, count))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        DetailFreshness, PullFileArtifact, PullFileBlobReferences, PullFileFlag, PullFileIdentity,
        PullFileMembershipReceipt, PullFileSource,
    };

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

    fn request() -> SaveReviewDraftRequest {
        SaveReviewDraftRequest {
            key: ReviewDraftKey {
                account_id: "account".into(),
                subject_id: "pull".into(),
            },
            authorization_epoch: "2".into(),
            authorization_view: "3".into(),
            expected_generation: "0".into(),
            event: ReviewSubmissionEvent::Approve,
            body: String::new(),
            comments: vec![],
        }
    }

    fn selection(side: ReviewDiffSide, line: u32) -> ReviewDraftCommentInput {
        ReviewDraftCommentInput {
            comment_id: "00000000-0000-4000-8000-000000000001".into(),
            body: "review body".into(),
            anchor: ReviewDraftAnchorSelection {
                file_facet_revision: "10".into(),
                context: context(),
                file_key: "provider:1".into(),
                start_line: None,
                line,
                start_side: None,
                side,
            },
        }
    }

    fn snapshot(patch: &str) -> PullFileArtifactSnapshot {
        let request = crate::PullFileDiffRequest {
            account_id: "account".into(),
            authorization_epoch: "2".into(),
            subject_id: "pull".into(),
            file_facet_revision: "10".into(),
            context: context(),
            file_key: "provider:1".into(),
        };
        let membership = PullFileMembershipReceipt {
            account_id: "account".into(),
            authorization_epoch: "2".into(),
            authorization_view: "3".into(),
            subject_id: "pull".into(),
            generation: "00000000-0000-4000-8000-000000000002".into(),
            file_facet_revision: "10".into(),
            context: context(),
            file_key: "provider:1".into(),
            identity: PullFileIdentity {
                old_path: Some("old/src.rs".into()),
                new_path: Some("new/src.rs".into()),
            },
        };
        PullFileArtifactSnapshot {
            request,
            membership: membership.clone(),
            artifact: Some(PullFileArtifact {
                account_id: membership.account_id,
                authorization_epoch: membership.authorization_epoch,
                authorization_view: membership.authorization_view,
                subject_id: membership.subject_id,
                generation: membership.generation,
                file_key: membership.file_key,
                identity: membership.identity,
                context: membership.context,
                source: Some(PullFileSource {
                    strategy: PullFileSourceStrategy::GithubPullFiles,
                    adapter_version: 1,
                }),
                validation: Some(PullFileArtifactValidation::Provider {
                    provider_validated_at: "2026-10-08T00:00:00Z".into(),
                }),
                content_state: PullFileContentState::Text,
                unified_text: Some(patch.into()),
                blob_references: PullFileBlobReferences::default(),
                old_blob_oid: None,
                new_blob_oid: None,
                content_type: None,
                binary_hint: PullFileFlag::Unknown,
                image_hint: PullFileFlag::Unknown,
                last_access_revision: "11".into(),
                logical_bytes: patch.len().to_string(),
                on_disk_bytes: patch.len().to_string(),
            }),
            revision: "12".into(),
            authorization_view: "3".into(),
            freshness: DetailFreshness::Fresh,
        }
    }

    #[test]
    fn exact_provider_hunk_resolves_each_side_to_its_native_path() {
        let patch = "@@ -10,3 +20,4 @@ fn example() {\n old\n-removed\n+added\n+added two\n same\n";
        let left = resolve_github_anchor(
            &request(),
            &selection(ReviewDiffSide::Left, 11),
            &snapshot(patch),
        )
        .unwrap();
        assert_eq!(left.path, "old/src.rs");
        let right = resolve_github_anchor(
            &request(),
            &selection(ReviewDiffSide::Right, 21),
            &snapshot(patch),
        )
        .unwrap();
        assert_eq!(right.path, "new/src.rs");
    }

    #[test]
    fn range_must_exist_on_one_side_in_one_complete_hunk() {
        let patch = "diff --git a/old/src.rs b/new/src.rs\n--- a/old/src.rs\n+++ b/new/src.rs\n@@ -10,2 +10,3 @@\n same\n+added\n same two\n@@ -30,1 +31,1 @@\n tail\n";
        let mut range = selection(ReviewDiffSide::Right, 12);
        range.anchor.start_line = Some(10);
        range.anchor.start_side = Some(ReviewDiffSide::Right);
        assert!(resolve_github_anchor(&request(), &range, &snapshot(patch)).is_ok());
        range.anchor.line = 31;
        assert_eq!(
            resolve_github_anchor(&request(), &range, &snapshot(patch)),
            Err(AnchorResolutionFailure::InvalidAnchor)
        );

        let malformed = "@@ -10,2 +10,2 @@\n one\n";
        assert_eq!(
            resolve_github_anchor(
                &request(),
                &selection(ReviewDiffSide::Right, 10),
                &snapshot(malformed),
            ),
            Err(AnchorResolutionFailure::InvalidAnchor)
        );
    }

    #[test]
    fn local_stale_missing_and_non_text_artifacts_never_authorize_provider_anchor() {
        let patch = "@@ -1,1 +1,1 @@\n line\n";
        let mut local = snapshot(patch);
        local
            .artifact
            .as_mut()
            .unwrap()
            .source
            .as_mut()
            .unwrap()
            .strategy = PullFileSourceStrategy::LocalExactRange;
        local.artifact.as_mut().unwrap().validation =
            Some(PullFileArtifactValidation::LocalExactRange {
                local_validated_at: "2026-10-08T00:00:00Z".into(),
                resolved_merge_base_oid: BASE.into(),
            });
        assert_eq!(
            resolve_github_anchor(&request(), &selection(ReviewDiffSide::Right, 1), &local,),
            Err(AnchorResolutionFailure::MissingProviderDiff)
        );

        let mut stale = snapshot(patch);
        stale.membership.authorization_view = "4".into();
        assert_eq!(
            resolve_github_anchor(&request(), &selection(ReviewDiffSide::Right, 1), &stale,),
            Err(AnchorResolutionFailure::StaleContext)
        );

        let mut missing = snapshot(patch);
        missing.artifact = None;
        assert_eq!(
            resolve_github_anchor(&request(), &selection(ReviewDiffSide::Right, 1), &missing,),
            Err(AnchorResolutionFailure::MissingProviderDiff)
        );

        let mut binary = snapshot(patch);
        let artifact = binary.artifact.as_mut().unwrap();
        artifact.content_state = PullFileContentState::Omitted;
        artifact.unified_text = None;
        artifact.logical_bytes = "0".into();
        artifact.on_disk_bytes = "0".into();
        assert_eq!(
            resolve_github_anchor(&request(), &selection(ReviewDiffSide::Right, 1), &binary,),
            Err(AnchorResolutionFailure::MissingProviderDiff)
        );
    }
}
