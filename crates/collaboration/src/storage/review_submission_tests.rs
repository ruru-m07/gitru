use super::*;
use crate::{
    DetailField, DetailValueState, MetadataField, MetadataObservedField, MetadataSource,
    ProviderInstance, PullFileChangeKind, ReviewDiffSide,
    credentials::{CredentialError, CredentialVault, SecretToken},
    delivery::DeliveryState,
    providers::{
        ProviderRegistry,
        github::{GithubProvider, review_submission::GithubReviewSubmissionPolicy},
    },
    pull_file_fixture as fixture,
    review_submission::{
        ReviewDraftAnchor, ReviewDraftAnchorSelection, ReviewDraftCommentInput, ReviewDraftKey,
        ReviewDraftQuery, ReviewSubmissionAvailability, ReviewSubmissionEvent,
        ReviewSubmissionReason, SaveReviewDraftRequest, SubmitReviewRequest,
    },
    runtime::CollaborationRuntime,
};
use serde_json::{Value, json};
use sqlx::Connection;
use std::{
    io::{Read, Write},
    net::TcpListener,
    sync::Arc,
    time::Duration,
};

#[derive(Default)]
struct SyntheticVault;

impl CredentialVault for SyntheticVault {
    fn store(&self, _: &str, _: &SecretToken) -> std::result::Result<(), CredentialError> {
        Ok(())
    }

    fn load(&self, _: &str) -> std::result::Result<Option<SecretToken>, CredentialError> {
        Ok(Some(
            SecretToken::new("synthetic_review_token".into()).unwrap(),
        ))
    }

    fn delete(&self, _: &str) -> std::result::Result<(), CredentialError> {
        Ok(())
    }
}

async fn publish_current_authority(store: &Store, account: &RemoteAccount) {
    let mut commit =
        fixture::range_observation(store, account, fixture::BASE, fixture::HEAD, "2").await;
    commit.source.source = "github/pull-detail/2026-03-10".into();
    commit.source.provider_updated_at = Some("2026-10-08T00:00:00Z".into());
    commit.source.observed_at = "2099-01-02T00:00:00Z".into();
    commit
        .source
        .field_mask
        .extend([DetailField::State, DetailField::UpdatedAt]);
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
    populate(&store, &account).await;
    (dir, store, account)
}

async fn populate(store: &Store, account: &RemoteAccount) {
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
    publish_current_authority(store, account).await;
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

fn pull_response() -> Value {
    json!({
        "id": 9_007_199_254_740_997_u64,
        "number": 67,
        "url": "https://api.github.com/repositories/1/pulls/67",
        "state": "open",
        "merged": false,
        "base": {"sha": fixture::BASE, "repo": {"id": 1}},
        "head": {"sha": fixture::HEAD, "repo": {"id": 2}}
    })
}

fn review_response(body: &str) -> Value {
    json!({
        "id": 80,
        "user": {"id": 7, "login": "reviewer"},
        "body": body,
        "state": "COMMENTED",
        "html_url": "https://github.com/owner/project/pull/67#pullrequestreview-80",
        "pull_request_url": "https://api.github.com/repositories/1/pulls/67",
        "submitted_at": "2026-10-08T01:02:03Z",
        "commit_id": fixture::HEAD
    })
}

fn inline_response(provider_id: u64, body: &str, line: u32) -> Value {
    json!({
        "id": provider_id,
        "pull_request_review_id": 80,
        "user": {"id": 7, "login": "reviewer"},
        "body": body,
        "path": "src/lib.rs",
        "line": line,
        "side": "RIGHT",
        "start_line": null,
        "start_side": null,
        "commit_id": fixture::HEAD,
        "original_commit_id": fixture::HEAD,
        "pull_request_url": "https://api.github.com/repositories/1/pulls/67"
    })
}

type HttpResponse = (u16, Option<Value>, String);

fn http_server(
    responses: Vec<HttpResponse>,
) -> (reqwest::Url, std::thread::JoinHandle<Vec<String>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let base = reqwest::Url::parse(&format!("http://{}/", listener.local_addr().unwrap())).unwrap();
    let handle = std::thread::spawn(move || {
        let mut requests = Vec::with_capacity(responses.len());
        for (status, response, headers) in responses {
            let deadline = std::time::Instant::now() + Duration::from_secs(15);
            let mut stream = loop {
                match listener.accept() {
                    Ok((stream, _)) => break stream,
                    Err(error)
                        if error.kind() == std::io::ErrorKind::WouldBlock
                            && std::time::Instant::now() < deadline =>
                    {
                        std::thread::sleep(Duration::from_millis(5));
                    }
                    Err(error) => panic!("finite review server accept: {error}"),
                }
            };
            stream.set_nonblocking(false).unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut bytes = Vec::new();
            let mut chunk = [0; 4096];
            loop {
                let read = stream.read(&mut chunk).unwrap();
                if read == 0 {
                    break;
                }
                bytes.extend_from_slice(&chunk[..read]);
                if let Some(end) = bytes.windows(4).position(|window| window == b"\r\n\r\n") {
                    let headers = String::from_utf8_lossy(&bytes[..end]);
                    let length = headers
                        .lines()
                        .find_map(|line| {
                            line.to_ascii_lowercase()
                                .strip_prefix("content-length:")
                                .and_then(|value| value.trim().parse::<usize>().ok())
                        })
                        .unwrap_or(0);
                    if bytes.len() >= end + 4 + length {
                        break;
                    }
                }
                assert!(bytes.len() < 100_000, "bounded synthetic request");
            }
            requests.push(String::from_utf8(bytes).unwrap());
            if let Some(response) = response {
                let body = response.to_string();
                write!(
                    stream,
                    "HTTP/1.1 {status} Fixture\r\nContent-Length: {}\r\nContent-Type: application/json\r\nConnection: close\r\n{headers}\r\n{body}",
                    body.len()
                )
                .unwrap();
            }
        }
        requests
    });
    (base, handle)
}

async fn runtime_at(
    path: &std::path::Path,
    base: reqwest::Url,
    initialize: bool,
) -> (Arc<CollaborationRuntime>, RemoteAccount) {
    let store = Arc::new(Store::open(path).await.unwrap());
    let account = if initialize {
        let mut account = fixture::account("a");
        account.actor_id = "7".into();
        let account = store.upsert_account(account).await.unwrap();
        let reference = "synthetic-review-reference";
        store
            .stage_credential(&account.id, reference)
            .await
            .unwrap();
        let account = store
            .commit_account_credential(
                RemoteAccount {
                    authorization_epoch: (account.authorization_epoch.parse::<u64>().unwrap() + 1)
                        .to_string(),
                    state: AccountState::Active,
                    ..account
                },
                reference,
            )
            .await
            .unwrap();
        populate(&store, &account).await;
        account
    } else {
        store.account("a").await.unwrap()
    };
    let mut registry = ProviderRegistry::default();
    registry
        .register(Arc::new(GithubProvider::for_test_base(base.clone())))
        .unwrap();
    let policy = Arc::new(GithubReviewSubmissionPolicy::for_test_base(base));
    let instance = ProviderInstance::public(ProviderKind::Github);
    registry
        .register_delivery(&instance, policy.clone())
        .unwrap();
    registry.register_recovery(&instance, policy).unwrap();
    (
        Arc::new(CollaborationRuntime::with_registry(
            store,
            Arc::new(SyntheticVault),
            registry,
        )),
        account,
    )
}

async fn force_delivery_due(store: &Store, command_id: &str) {
    let mut writer = store.inner.writer.acquire().await.unwrap();
    sqlx::query(
        "UPDATE command_delivery SET generation=generation+1, \
         next_action_at='2000-01-01T00:00:00.000000000Z' \
         WHERE account_id='a' AND command_id=?",
    )
    .bind(command_id)
    .execute(&mut *writer)
    .await
    .unwrap();
}

async fn corrupted_copy(
    source: &std::path::Path,
    target: &std::path::Path,
    statement: &'static str,
) {
    std::fs::copy(source, target).unwrap();
    let mut connection = sqlx::SqliteConnection::connect_with(
        &sqlx::sqlite::SqliteConnectOptions::new()
            .filename(target)
            .foreign_keys(false),
    )
    .await
    .unwrap();
    sqlx::raw_sql(statement)
        .execute(&mut connection)
        .await
        .unwrap();
    connection.close().await.unwrap();
}

async fn save_inline_review(
    store: &Store,
    account: &RemoteAccount,
) -> crate::review_submission::ReviewDraftSnapshot {
    let files = fixture::publish(store, account, &["src/lib.rs"]).await;
    let request = fixture::request(&files, account);
    let membership = store
        .verify_pull_file_membership(request.clone())
        .await
        .unwrap();
    store
        .apply_pull_file_artifact(
            request.clone(),
            membership.clone(),
            fixture::artifact(&membership, "@@ -1,2 +1,2 @@\n-old\n+new\n context\n"),
        )
        .await
        .unwrap();
    let before = store.review_draft(key()).await.unwrap();
    store
        .save_review_draft(SaveReviewDraftRequest {
            key: key(),
            authorization_epoch: account.authorization_epoch.clone(),
            authorization_view: before.authorization_view,
            expected_generation: before.generation,
            event: ReviewSubmissionEvent::Comment,
            body: "Summary".into(),
            comments: [
                (
                    "00000000-0000-4000-8000-000000000001",
                    "First inline comment",
                    1,
                ),
                (
                    "00000000-0000-4000-8000-000000000002",
                    "Second inline comment",
                    2,
                ),
            ]
            .into_iter()
            .map(|(comment_id, body, line)| ReviewDraftCommentInput {
                comment_id: comment_id.into(),
                body: body.into(),
                anchor: ReviewDraftAnchorSelection {
                    file_facet_revision: request.file_facet_revision.clone(),
                    context: request.context.clone(),
                    file_key: request.file_key.clone(),
                    start_line: None,
                    line,
                    start_side: None,
                    side: ReviewDiffSide::Right,
                },
            })
            .collect(),
        })
        .await
        .unwrap()
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
async fn synthetic_worker_accepts_then_confirms_exact_review_and_inline_receipts() {
    use crate::recovery::{RecoverySession, RestoreChoice};

    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("worker.db");
    let (base, server) = http_server(vec![
        (200, Some(pull_response()), String::new()),
        (200, Some(review_response("Summary")), String::new()),
        (200, Some(review_response("Summary")), String::new()),
        (
            200,
            Some(json!([
                inline_response(901, "First inline comment", 1),
                inline_response(902, "Second inline comment", 2)
            ])),
            String::new(),
        ),
    ]);
    let (runtime, account) = runtime_at(&path, base, true).await;
    let saved = save_inline_review(runtime.store(), &account).await;
    let request = submit_request(&saved);
    assert!(
        !runtime
            .submit_review(request.clone())
            .await
            .unwrap()
            .duplicate
    );

    assert!(runtime.run_delivery_next().await.unwrap());
    let accepted = runtime
        .store()
        .delivery_command("a", &request.command_id)
        .await
        .unwrap();
    assert_eq!(accepted.state, DeliveryState::Accepted);
    assert_eq!(accepted.attempt_count, 1);
    assert_eq!(accepted.evidence.len(), 1);
    assert_eq!(accepted.evidence[0].evidence.kind, "github.review_accepted");
    let accepted_page = runtime
        .submitted_reviews(crate::review_submission::SubmittedReviewQuery {
            account_id: "a".into(),
            subject_id: "pull".into(),
            cursor: None,
            limit: 10,
        })
        .await
        .unwrap();
    assert_eq!(accepted_page.reviews.len(), 1);
    assert!(!accepted_page.reviews[0].confirmed);
    assert_eq!(accepted_page.reviews[0].inline_comment_count, 2);

    force_delivery_due(runtime.store(), &request.command_id).await;
    assert!(runtime.run_delivery_next().await.unwrap());
    let confirmed = runtime
        .store()
        .delivery_command("a", &request.command_id)
        .await
        .unwrap();
    assert_eq!(confirmed.state, DeliveryState::Confirmed);
    assert_eq!(
        confirmed.attempt_count, 1,
        "exact-ID readback is not a POST"
    );
    assert_eq!(confirmed.evidence.len(), 2);
    assert_eq!(
        confirmed.evidence[1].evidence.kind,
        "github.review_submitted"
    );
    let confirmed_page = runtime
        .submitted_reviews(crate::review_submission::SubmittedReviewQuery {
            account_id: "a".into(),
            subject_id: "pull".into(),
            cursor: None,
            limit: 10,
        })
        .await
        .unwrap();
    assert!(confirmed_page.reviews[0].confirmed);
    assert_eq!(confirmed_page.reviews[0].provider_id, "80");
    assert_eq!(confirmed_page.reviews[0].reviewed_commit_oid, fixture::HEAD);

    let requests = server.join().unwrap();
    assert_eq!(requests.len(), 4);
    assert!(requests[0].starts_with("GET /repositories/1/pulls/67 HTTP/1.1"));
    assert!(requests[1].starts_with("POST /repositories/1/pulls/67/reviews HTTP/1.1"));
    assert!(requests[2].starts_with("GET /repositories/1/pulls/67/reviews/80 HTTP/1.1"));
    assert!(
        requests[3]
            .starts_with("GET /repositories/1/pulls/67/reviews/80/comments?per_page=100 HTTP/1.1")
    );
    assert_eq!(
        requests
            .iter()
            .filter(|request| request.starts_with("POST "))
            .count(),
        1
    );
    {
        let mut writer = runtime.store().inner.writer.acquire().await.unwrap();
        assert!(
            sqlx::query("UPDATE review_resolutions SET provider_id='81' WHERE account_id='a'")
                .execute(&mut *writer)
                .await
                .is_err(),
            "accepted review mappings are immutable"
        );
        assert!(
            sqlx::query("DELETE FROM review_confirmations WHERE account_id='a'")
                .execute(&mut *writer)
                .await
                .is_err(),
            "terminal confirmation rows are retained"
        );
    }
    let backup = dir.path().join("confirmed-backup.db");
    runtime.shutdown().await.unwrap();
    let current = Store::open(&path).await.unwrap();
    assert_eq!(current.backup_to(&backup).await.unwrap().schema_version, 26);
    current.close().await.unwrap();

    for (name, corruption) in [
        (
            "missing-confirmation",
            "DROP TRIGGER review_confirmation_retained; DELETE FROM review_confirmations;",
        ),
        (
            "missing-accepted",
            "DROP TRIGGER review_confirmation_retained; \
             DROP TRIGGER review_resolution_retained; \
             DELETE FROM review_confirmations; DELETE FROM review_resolutions;",
        ),
        (
            "missing-accepted-delivery-resolution",
            "DROP TRIGGER delivery_resolution_retained; \
             DELETE FROM delivery_resolutions WHERE purpose='accepted';",
        ),
        (
            "missing-all-review-proofs-and-mappings",
            "DROP TRIGGER review_confirmation_retained; \
             DROP TRIGGER review_resolution_retained; \
             DROP TRIGGER delivery_resolution_retained; \
             DROP TRIGGER evidence_retained; \
             DELETE FROM review_confirmations; DELETE FROM review_resolutions; \
             DELETE FROM delivery_resolutions; DELETE FROM command_evidence;",
        ),
        (
            "reversed-resolution-generation",
            "DROP TRIGGER delivery_resolution_retained; \
             DELETE FROM delivery_resolutions WHERE purpose='confirmed'; \
             INSERT INTO delivery_resolutions \
             SELECT a.account_id,a.command_id,a.delivery_generation-1,f.confirmed_ordinal,'confirmed' \
             FROM delivery_resolutions a JOIN review_confirmations f USING(account_id,command_id) \
             WHERE a.purpose='accepted';",
        ),
        (
            "reversed-evidence-ordinal",
            "DROP TRIGGER evidence_immutable; \
             DROP TRIGGER review_resolution_immutable; \
             DROP TRIGGER delivery_resolution_immutable; \
             UPDATE command_evidence SET ordinal=2 WHERE kind='github.review_accepted'; \
             UPDATE review_resolutions SET accepted_ordinal=2; \
             UPDATE delivery_resolutions SET evidence_ordinal=2 WHERE purpose='accepted';",
        ),
        (
            "second-native-attempt",
            "INSERT INTO delivery_attempts \
             SELECT account_id,command_id,2,authorization_epoch,started_at,outcome,completed_at \
             FROM delivery_attempts WHERE attempt_number=1; \
             INSERT INTO delivery_attempt_context \
             SELECT account_id,command_id,2,instance_id,execution_base \
             FROM delivery_attempt_context WHERE attempt_number=1;",
        ),
        (
            "changed-inline-body",
            "UPDATE review_draft_comments SET body='Changed after sealing' WHERE ordinal=0;",
        ),
        (
            "changed-inline-uuid",
            "DROP TRIGGER review_draft_comment_identity; \
             UPDATE review_draft_comments \
             SET comment_id='00000000-0000-4000-8000-000000000099' WHERE ordinal=0;",
        ),
        (
            "reordered-inline-comments",
            "UPDATE review_draft_comments SET ordinal=24 WHERE ordinal=0; \
             UPDATE review_draft_comments SET ordinal=0 WHERE ordinal=1; \
             UPDATE review_draft_comments SET ordinal=1 WHERE ordinal=24;",
        ),
        (
            "missing-inline-comment",
            "DELETE FROM review_draft_comments WHERE ordinal=1;",
        ),
    ] {
        let candidate = dir.path().join(format!("{name}.db"));
        corrupted_copy(&backup, &candidate, corruption).await;
        assert!(
            RecoverySession::prepare(&path, &candidate).await.is_err(),
            "recovery accepted {name} corruption"
        );
    }

    let session = RecoverySession::prepare(&path, &backup).await.unwrap();
    let confirmation_id = session.preview().confirmation_id.clone();
    session
        .confirm(&confirmation_id, RestoreChoice::ReplaceCurrentData)
        .unwrap();
    let restored = Store::open(&path).await.unwrap();
    let restored_draft = restored.review_draft(key()).await.unwrap();
    assert_eq!(restored_draft.body, "Summary");
    assert_eq!(restored_draft.comments[0].body, "First inline comment");
    assert_eq!(restored_draft.comments[1].body, "Second inline comment");
    assert!(restored_draft.context.is_none());
    assert_eq!(
        restored_draft.reason,
        Some(ReviewSubmissionReason::AccountUnavailable)
    );
    let accepted_rows: i64 =
        sqlx::query_scalar("SELECT count(*) FROM review_resolutions WHERE account_id='a'")
            .fetch_one(&restored.inner.readers)
            .await
            .unwrap();
    let confirmed_rows: i64 =
        sqlx::query_scalar("SELECT count(*) FROM review_confirmations WHERE account_id='a'")
            .fetch_one(&restored.inner.readers)
            .await
            .unwrap();
    assert_eq!((accepted_rows, confirmed_rows), (1, 1));
    restored.close().await.unwrap();
}

#[tokio::test]
async fn lost_or_malformed_create_response_stays_unknown_and_never_posts_again() {
    for (case, response) in [
        ("lost", None),
        ("malformed", Some(json!({"unexpected": true}))),
    ] {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(format!("{case}.db"));
        let (base, server) = http_server(vec![
            (200, Some(pull_response()), String::new()),
            (200, response, String::new()),
        ]);
        let (runtime, account) = runtime_at(&path, base, true).await;
        let saved = save_summary(runtime.store(), &account, "Summary", "0").await;
        let request = submit_request(&saved);
        runtime.submit_review(request.clone()).await.unwrap();

        assert!(runtime.run_delivery_next().await.unwrap());
        let unknown = runtime
            .store()
            .delivery_command("a", &request.command_id)
            .await
            .unwrap();
        assert_eq!(unknown.state, DeliveryState::Unknown);
        assert_eq!(unknown.attempt_count, 1);
        assert!(unknown.evidence.is_empty());
        let requests = server.join().unwrap();
        assert_eq!(requests.len(), 2);
        assert_eq!(
            requests
                .iter()
                .filter(|request| request.starts_with("POST "))
                .count(),
            1
        );

        // A due recovery turn may review the immutable intent, but without a
        // strong native review ID there is no safe remote query and no second
        // POST. The durable dispatch attempt remains exactly one.
        force_delivery_due(runtime.store(), &request.command_id).await;
        assert!(runtime.run_delivery_next().await.unwrap());
        let recovered = runtime
            .store()
            .delivery_command("a", &request.command_id)
            .await
            .unwrap();
        assert_eq!(recovered.state, DeliveryState::Unknown);
        assert_eq!(recovered.attempt_count, 1);
        assert!(recovered.evidence.is_empty());
        runtime.shutdown().await.unwrap();
    }
}

#[tokio::test]
async fn accepted_quota_survives_cold_restart_and_restore_quarantines_without_losing_authorship() {
    use crate::recovery::{RecoverySession, RestoreChoice};

    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("quota.db");
    let backup = dir.path().join("quota-backup.db");
    let (base, server) = http_server(vec![
        (200, Some(pull_response()), String::new()),
        (
            200,
            Some(review_response("Summary")),
            "Retry-After: 120\r\n".into(),
        ),
    ]);
    let (runtime, account) = runtime_at(&path, base.clone(), true).await;
    let saved = save_summary(runtime.store(), &account, "Summary", "0").await;
    let request = submit_request(&saved);
    runtime.submit_review(request.clone()).await.unwrap();
    assert!(runtime.run_delivery_next().await.unwrap());
    let accepted = runtime
        .store()
        .delivery_command("a", &request.command_id)
        .await
        .unwrap();
    assert_eq!(accepted.state, DeliveryState::Accepted);
    assert_eq!(accepted.attempt_count, 1);
    assert!(
        runtime
            .store()
            .scope_state("a", "provider:rest")
            .await
            .unwrap()
            .and_then(|state| state.sync.next_retry_at)
            .is_some()
    );
    let requests = server.join().unwrap();
    assert_eq!(requests.len(), 2);

    force_delivery_due(runtime.store(), &request.command_id).await;
    assert!(
        !runtime.run_delivery_next().await.unwrap(),
        "the accepted review must remain behind the observed account cooldown"
    );
    runtime.shutdown().await.unwrap();

    let (runtime, _) = runtime_at(&path, base, false).await;
    force_delivery_due(runtime.store(), &request.command_id).await;
    assert!(
        !runtime.run_delivery_next().await.unwrap(),
        "a cold runtime must reinstall the durable provider cooldown before HTTP"
    );
    let cold = runtime
        .store()
        .delivery_command("a", &request.command_id)
        .await
        .unwrap();
    assert_eq!(cold.state, DeliveryState::Accepted);
    assert_eq!(cold.attempt_count, 1);
    let summary = runtime.store().backup_to(&backup).await.unwrap();
    assert_eq!(summary.schema_version, 26);
    runtime.shutdown().await.unwrap();

    let session = RecoverySession::prepare(&path, &backup).await.unwrap();
    assert!(session.preview().quarantined_commands >= 1);
    let confirmation_id = session.preview().confirmation_id.clone();
    session
        .confirm(&confirmation_id, RestoreChoice::ReplaceCurrentData)
        .unwrap();
    let restored = Store::open(&path).await.unwrap();
    let draft = restored.review_draft(key()).await.unwrap();
    assert_eq!(draft.body, "Summary");
    assert!(draft.context.is_none());
    assert_eq!(
        draft.reason,
        Some(ReviewSubmissionReason::AccountUnavailable)
    );
    let status = draft.submission.unwrap();
    assert_eq!(status.command_id, request.command_id);
    assert!(status.quarantined);
    let command = restored
        .delivery_command("a", &status.command_id)
        .await
        .unwrap();
    assert!(command.reconcile_only());
    assert_eq!(command.attempt_count, 1);
    restored.close().await.unwrap();
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

#[tokio::test]
async fn renamed_copied_and_unknown_files_never_authorize_inline_reviews() {
    for change_kind in [
        PullFileChangeKind::Renamed,
        PullFileChangeKind::Copied,
        PullFileChangeKind::Unknown,
    ] {
        let (_dir, store, account) = setup().await;
        let lease = store
            .begin_pull_files(
                &account.id,
                &account.authorization_epoch,
                "pull",
                fixture::source(),
            )
            .await
            .unwrap();
        let mut commit = fixture::commit(&store, &lease, &["src/lib.rs"], None).await;
        commit.page.files[0].change_kind = change_kind;
        commit.page.files[0].provider_change_kind = format!("{change_kind:?}").to_lowercase();
        store.apply_pull_files(commit).await.unwrap();
        let files = fixture::snapshot(&store).await;
        let request = fixture::request(&files, &account);
        let membership = store
            .verify_pull_file_membership(request.clone())
            .await
            .unwrap();
        store
            .apply_pull_file_artifact(
                request.clone(),
                membership.clone(),
                fixture::artifact(&membership, "@@ -1 +1 @@\n-old\n+new\n"),
            )
            .await
            .unwrap();
        let before = store.review_draft(key()).await.unwrap();
        let error = store
            .save_review_draft(SaveReviewDraftRequest {
                key: key(),
                authorization_epoch: account.authorization_epoch.clone(),
                authorization_view: before.authorization_view,
                expected_generation: before.generation,
                event: ReviewSubmissionEvent::Comment,
                body: "Summary".into(),
                comments: vec![ReviewDraftCommentInput {
                    comment_id: Uuid::new_v4().to_string(),
                    body: "Inline comment".into(),
                    anchor: ReviewDraftAnchorSelection {
                        file_facet_revision: request.file_facet_revision,
                        context: request.context,
                        file_key: request.file_key,
                        start_line: None,
                        line: 1,
                        start_side: None,
                        side: ReviewDiffSide::Right,
                    },
                }],
            })
            .await
            .unwrap_err();
        assert_eq!(error.code, ErrorCode::StaleView, "{change_kind:?}");
        assert_eq!(store.review_draft(key()).await.unwrap().generation, "0");
        store.close().await.unwrap();
    }

    // Admission revalidates the trusted file kind even if cache bytes change
    // after an authored draft was saved; the sealed review is never admitted.
    let (_dir, store, account) = setup().await;
    let saved = save_inline_review(&store, &account).await;
    {
        let mut writer = store.inner.writer.acquire().await.unwrap();
        let stored: String = sqlx::query_scalar(
            "SELECT summary_json FROM pull_file_rows WHERE account_id='a' AND subject_id='pull'",
        )
        .fetch_one(&mut *writer)
        .await
        .unwrap();
        let mut summary: Value = serde_json::from_str(&stored).unwrap();
        summary["change_kind"] = json!("copied");
        sqlx::query(
            "UPDATE pull_file_rows SET summary_json=? WHERE account_id='a' AND subject_id='pull'",
        )
        .bind(summary.to_string())
        .execute(&mut *writer)
        .await
        .unwrap();
    }
    assert_eq!(
        store
            .submit_review(submit_request(&saved))
            .await
            .unwrap_err()
            .code,
        ErrorCode::StaleView
    );
    let commands: i64 = sqlx::query_scalar("SELECT count(*) FROM commands WHERE account_id='a'")
        .fetch_one(&store.inner.readers)
        .await
        .unwrap();
    assert_eq!(commands, 0);
    store.close().await.unwrap();
}
