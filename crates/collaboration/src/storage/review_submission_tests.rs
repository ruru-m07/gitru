use super::*;
use crate::{
    DetailValueState, MetadataField, MetadataObservedField, MetadataSource, ReviewDiffSide,
    pull_file_fixture as fixture,
    review_submission::{
        ReviewDraftAnchor, ReviewDraftAnchorSelection, ReviewDraftCommentInput, ReviewDraftKey,
        ReviewDraftQuery, ReviewSubmissionAvailability, ReviewSubmissionEvent,
        ReviewSubmissionReason, SaveReviewDraftRequest, SubmitReviewRequest,
    },
};

async fn publish_current_authority(store: &Store, account: &RemoteAccount) {
    let mut commit =
        fixture::range_observation(store, account, fixture::BASE, fixture::HEAD, "2").await;
    commit.source.source = "github/pull-detail/2026-03-10".into();
    commit.source.provider_updated_at = Some("2026-10-08T00:00:00Z".into());
    commit.source.observed_at = "2099-01-02T00:00:00Z".into();
    let metadata = commit.metadata.as_mut().unwrap();
    metadata.values.state = Some("open".into());
    metadata.values.updated_at = commit.source.provider_updated_at.clone();
    metadata
        .values
        .base
        .as_mut()
        .unwrap()
        .repository
        .as_mut()
        .unwrap()
        .provider_id = "1".into();
    metadata
        .values
        .head
        .as_mut()
        .unwrap()
        .repository
        .as_mut()
        .unwrap()
        .provider_id = "2".into();
    metadata.fields.extend([
        MetadataObservedField {
            field: MetadataField::State,
            state: DetailValueState::Known,
        },
        MetadataObservedField {
            field: MetadataField::UpdatedAt,
            state: DetailValueState::Known,
        },
    ]);
    metadata.source = MetadataSource {
        source: commit.source.source.clone(),
        adapter_version: 1,
        provider_updated_at: commit.source.provider_updated_at.clone(),
        observed_at: commit.source.observed_at.clone(),
    };
    let binding = commit.subject_binding.as_mut().unwrap();
    binding.repository_provider_id = "1".into();
    binding.provider_id = "9007199254740997".into();
    store.apply_detail(commit).await.unwrap();
}

fn key() -> ReviewDraftKey {
    ReviewDraftKey {
        account_id: "a".into(),
        subject_id: "pull".into(),
    }
}

async fn setup() -> (tempfile::TempDir, Store, RemoteAccount) {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path().join("reviews.db")).await.unwrap();
    let mut account = fixture::account("a");
    account.actor_id = "7".into();
    let account = store.upsert_account(account).await.unwrap();
    let repository = RemoteRepository {
        id: "repo".into(),
        account_id: account.id.clone(),
        provider_id: "1".into(),
        full_name: "owner/project".into(),
        name: "project".into(),
        web_url: "https://github.com/owner/project".into(),
        description: None,
        default_branch: Some("main".into()),
        selected: true,
    };
    let pull = RemoteItem {
        native_inbox: None,
        id: "pull".into(),
        account_id: account.id.clone(),
        repository_id: Some(repository.id.clone()),
        provider_id: "9007199254740997".into(),
        kind: RemoteItemKind::PullRequest,
        number: Some("67".into()),
        title: "Review fixture".into(),
        body: None,
        body_omitted: true,
        author: None,
        web_url: Some("https://github.com/owner/project/pull/67".into()),
        state: "open".into(),
        updated_at: "2026-10-08T00:00:00Z".into(),
        head_oid: Some(fixture::HEAD.into()),
        is_draft: Some(false),
        reason: None,
        unread: None,
    };
    for (scope, repositories, items) in [
        ("repositories", vec![repository], vec![]),
        ("repo:repo:pull_request", vec![], vec![pull]),
    ] {
        let run_id = store
            .begin_sync(&account.id, &account.authorization_epoch, scope)
            .await
            .unwrap();
        store
            .apply_page(PageCommit {
                account_id: account.id.clone(),
                authorization_epoch: account.authorization_epoch.clone(),
                scope: scope.into(),
                run_id,
                repositories,
                items,
                endpoint_aliases: vec![],
                next_cursor: None,
                etag: None,
                last_modified: None,
                not_modified: false,
                complete: true,
                observed_at: "2026-10-08T00:00:00Z".into(),
            })
            .await
            .unwrap();
    }
    publish_current_authority(&store, &account).await;
    (dir, store, account)
}

async fn save_summary(
    store: &Store,
    account: &RemoteAccount,
    body: &str,
    expected_generation: &str,
) -> crate::review_submission::ReviewDraftSnapshot {
    let current = store.review_draft(key()).await.unwrap();
    store
        .save_review_draft(SaveReviewDraftRequest {
            key: key(),
            authorization_epoch: account.authorization_epoch.clone(),
            authorization_view: current.authorization_view,
            expected_generation: expected_generation.into(),
            event: ReviewSubmissionEvent::Comment,
            body: body.into(),
            comments: vec![],
        })
        .await
        .unwrap()
}

fn submit_request(snapshot: &crate::review_submission::ReviewDraftSnapshot) -> SubmitReviewRequest {
    SubmitReviewRequest {
        context: snapshot.context.clone().unwrap(),
        draft_generation: snapshot.generation.clone(),
        command_id: Uuid::new_v4().to_string(),
        accept_background_delivery: true,
        accept_best_effort_race: true,
    }
}

#[tokio::test]
async fn review_draft_cas_exact_retry_and_authored_recovery_are_durable() {
    let (dir, store, account) = setup().await;
    let empty = store.review_draft(key()).await.unwrap();
    assert_eq!(empty.generation, "0");
    assert_eq!(
        empty.reason,
        Some(ReviewSubmissionReason::EmptyRequiredBody)
    );

    let first = save_summary(&store, &account, "Summary review", "0").await;
    assert_eq!(first.availability, ReviewSubmissionAvailability::Available);
    assert!(first.context.is_some());
    let same = save_summary(&store, &account, "Summary review", &first.generation).await;
    assert_eq!(same, first);

    let request = submit_request(&first);
    assert!(
        !store
            .submit_review(request.clone())
            .await
            .unwrap()
            .duplicate
    );
    let admitted = store.review_draft(key()).await.unwrap();
    assert_eq!(
        admitted.reason,
        Some(ReviewSubmissionReason::PendingSubmission)
    );
    assert_eq!(admitted.submission.as_ref().unwrap().state, "queued");
    {
        let mut writer = store.inner.writer.acquire().await.unwrap();
        let mut tx = writer.begin().await.unwrap();
        sqlx::query(
            "UPDATE commands SET state='outcome_unknown' WHERE account_id=? AND command_id=?",
        )
        .bind("a")
        .bind(&request.command_id)
        .execute(&mut *tx)
        .await
        .unwrap();
        tx.commit().await.unwrap();
    }
    let unknown = store.review_draft(key()).await.unwrap();
    assert_eq!(
        unknown.reason,
        Some(ReviewSubmissionReason::PendingSubmission)
    );
    assert_eq!(
        unknown.submission.as_ref().unwrap().state,
        "outcome_unknown"
    );
    assert!(
        store
            .submit_review(request.clone())
            .await
            .unwrap()
            .duplicate
    );
    let mut different_uuid = request.clone();
    different_uuid.command_id = Uuid::new_v4().to_string();
    assert_eq!(
        store.submit_review(different_uuid).await.unwrap_err().code,
        ErrorCode::StaleView
    );

    let edited = save_summary(&store, &account, "Edited while queued", &first.generation).await;
    assert_eq!(
        edited.reason,
        Some(ReviewSubmissionReason::PendingSubmission)
    );
    assert!(edited.context.is_none());
    assert_eq!(
        edited.submission.as_ref().unwrap().command_id,
        request.command_id
    );
    assert!(
        store
            .submit_review(request.clone())
            .await
            .unwrap()
            .duplicate
    );

    let page = store
        .review_drafts(ReviewDraftQuery {
            account_id: "a".into(),
            cursor: None,
            limit: 1,
        })
        .await
        .unwrap();
    assert_eq!(page.drafts.len(), 1);
    assert_eq!(page.drafts[0].subject_id, "pull");
    assert_eq!(page.drafts[0].preview, "Edited while queued");

    store.disconnect("a").await.unwrap();
    let offline = store.review_draft(key()).await.unwrap();
    assert_eq!(offline.body, "Edited while queued");
    assert!(offline.context.is_none());
    assert_eq!(
        offline.reason,
        Some(ReviewSubmissionReason::AccountUnavailable)
    );
    store.close().await.unwrap();

    let store = Store::open(dir.path().join("reviews.db")).await.unwrap();
    let reopened = store.review_draft(key()).await.unwrap();
    assert_eq!(reopened.body, "Edited while queued");
    assert!(reopened.context.is_none());
    assert!(reopened.submission.is_some());
    store.close().await.unwrap();
}

#[tokio::test]
async fn inline_review_anchor_is_derived_only_from_exact_provider_artifact() {
    let (_dir, store, account) = setup().await;
    let files = fixture::publish(&store, &account, &["src/lib.rs"]).await;
    let request = fixture::request(&files, &account);
    let membership = store
        .verify_pull_file_membership(request.clone())
        .await
        .unwrap();
    let patch = "@@ -1,2 +1,2 @@\n-old\n+new\n context\n";
    store
        .apply_pull_file_artifact(
            request.clone(),
            membership.clone(),
            fixture::artifact(&membership, patch),
        )
        .await
        .unwrap();

    let before = store.review_draft(key()).await.unwrap();
    let selection = ReviewDraftAnchorSelection {
        file_facet_revision: request.file_facet_revision.clone(),
        context: request.context.clone(),
        file_key: request.file_key.clone(),
        start_line: None,
        line: 1,
        start_side: None,
        side: ReviewDiffSide::Right,
    };
    let saved = store
        .save_review_draft(SaveReviewDraftRequest {
            key: key(),
            authorization_epoch: account.authorization_epoch.clone(),
            authorization_view: before.authorization_view.clone(),
            expected_generation: before.generation,
            event: ReviewSubmissionEvent::Comment,
            body: "Summary".into(),
            comments: vec![ReviewDraftCommentInput {
                comment_id: Uuid::new_v4().to_string(),
                body: "Inline comment".into(),
                anchor: selection.clone(),
            }],
        })
        .await
        .unwrap();
    assert_eq!(saved.availability, ReviewSubmissionAvailability::Available);
    let Some(ReviewDraftAnchor::Github(anchor)) = &saved.comments[0].anchor else {
        panic!("provider authority should resolve a GitHub anchor");
    };
    assert_eq!(anchor.path, "src/lib.rs");
    assert_eq!(anchor.line, 1);
    assert_eq!(anchor.context.head_oid, fixture::HEAD);

    let mut invalid = selection;
    invalid.line = 99;
    assert_eq!(
        store
            .save_review_draft(SaveReviewDraftRequest {
                key: key(),
                authorization_epoch: account.authorization_epoch,
                authorization_view: saved.authorization_view,
                expected_generation: saved.generation.clone(),
                event: ReviewSubmissionEvent::Comment,
                body: "Should not replace saved authority".into(),
                comments: vec![ReviewDraftCommentInput {
                    comment_id: Uuid::new_v4().to_string(),
                    body: "Outside provider patch".into(),
                    anchor: invalid,
                }],
            })
            .await
            .unwrap_err()
            .code,
        ErrorCode::StaleView
    );
    assert_eq!(
        store.review_draft(key()).await.unwrap().generation,
        saved.generation
    );
    store.close().await.unwrap();
}
