use super::*;
use crate::commands::{
    CanonicalFields, CommandDraft, CommandPayloadCodec, CommandTarget, CommandTargetKind,
    seal_command,
};
use crate::runtime::detail_tests::fixtures;
use crate::storage::command_admission::{CommandAdmissionPolicy, CommandProtection};

const SCOPE: &str = "repo:repo:pull_request";
const OBSERVED: &str = "2026-10-08T01:00:00Z";
struct Payload;
impl CommandPayloadCodec for Payload {
    const OPERATION_KIND: &'static str = "github.create_pull_request";
    const PAYLOAD_VERSION: u32 = 1;
    fn encode_payload(&self, fields: &mut CanonicalFields) -> Result<()> {
        fields.string(1, "publication-fixture")
    }
}
struct Policy;
#[async_trait::async_trait]
impl CommandAdmissionPolicy for Policy {
    const OPERATION_KIND: &'static str = "github.create_pull_request";
    const PAYLOAD_VERSION: u32 = 1;
    async fn validate(
        &self,
        _: &mut Transaction<'_, Sqlite>,
        _: &RemoteAccount,
        _: &crate::CommandSubmission,
    ) -> Result<Vec<CommandProtection>> {
        Ok(vec![])
    }
}
struct Fixture {
    _dir: tempfile::TempDir,
    store: Store,
    account: RemoteAccount,
    repo: RemoteRepository,
    command: crate::CommandSubmission,
    view: String,
}
async fn remove_seed_pull(store: &Store) {
    let mut writer = store.inner.writer.acquire().await.unwrap();
    sqlx::query("DELETE FROM items WHERE id='pull'")
        .execute(&mut *writer)
        .await
        .unwrap();
    sqlx::query("DELETE FROM items_fts WHERE id='pull'")
        .execute(&mut *writer)
        .await
        .unwrap();
    sqlx::query("DELETE FROM sync_scopes WHERE scope='repo:repo:pull_request'")
        .execute(&mut *writer)
        .await
        .unwrap();
}
async fn setup() -> Fixture {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path().join("publication.db"))
        .await
        .unwrap();
    let account = fixtures::seed(&store, "a").await;
    remove_seed_pull(&store).await;
    let repo = store.repository("a", "repo").await.unwrap();
    let command = seal_command(CommandDraft {
        command_id: Uuid::new_v4().to_string(),
        account_id: account.id.clone(),
        authorization_epoch: account.authorization_epoch.clone(),
        target: CommandTarget::new(CommandTargetKind::Repository, repo.id.clone(), None).unwrap(),
        payload: Payload,
        guards: vec![],
        dependencies: vec![],
    })
    .unwrap();
    store.admit_command(&command, &Policy).await.unwrap();
    let mut tx = store.inner.readers.begin().await.unwrap();
    let view = metadata(&mut tx).await.unwrap().1;
    tx.commit().await.unwrap();
    Fixture {
        _dir: dir,
        store,
        account,
        repo,
        command,
        view,
    }
}
fn item() -> RemoteItem {
    RemoteItem {
        id: "github:pull:9007199254740999".into(),
        account_id: "a".into(),
        repository_id: Some("repo".into()),
        provider_id: "9007199254740999".into(),
        kind: RemoteItemKind::PullRequest,
        number: Some("68".into()),
        title: "Created pull title".into(),
        body: Some("Created pull body".into()),
        body_omitted: false,
        author: Some("a".into()),
        web_url: Some("https://github.com/owner/project/pull/68".into()),
        state: "open".into(),
        updated_at: "2026-10-08T00:00:00Z".into(),
        head_oid: Some("a".repeat(40)),
        is_draft: Some(false),
        reason: None,
        unread: None,
        native_inbox: None,
    }
}
fn publication<'a>(f: &'a Fixture, item: &'a RemoteItem) -> Publication<'a> {
    Publication {
        command_id: f.command.command_id(),
        authorization_view: &f.view,
        repository: &f.repo,
        item,
        metadata: None,
        observed_at: OBSERVED,
    }
}
async fn publish(f: &Fixture, item: &RemoteItem) {
    let mut writer = f.store.inner.writer.acquire().await.unwrap();
    let mut tx = writer.begin().await.unwrap();
    assert_eq!(
        publish_in(&mut tx, &f.account, publication(f, item))
            .await
            .unwrap(),
        item.id
    );
    tx.commit().await.unwrap();
}
fn query() -> ItemQuery {
    ItemQuery {
        account_id: "a".into(),
        kind: RemoteItemKind::PullRequest,
        repository_id: Some("repo".into()),
        state: None,
        search: None,
        cursor: None,
        limit: 10,
    }
}
fn page(f: &Fixture, run: String, items: Vec<RemoteItem>, complete: bool) -> PageCommit {
    PageCommit {
        account_id: f.account.id.clone(),
        authorization_epoch: f.account.authorization_epoch.clone(),
        scope: SCOPE.into(),
        run_id: run,
        repositories: vec![],
        items,
        endpoint_aliases: vec![],
        next_cursor: if complete {
            None
        } else {
            Some("page-2".into())
        },
        etag: if complete {
            Some("old-feed-validator".into())
        } else {
            None
        },
        last_modified: None,
        not_modified: false,
        complete,
        observed_at: OBSERVED.into(),
    }
}
async fn enumerate(f: &Fixture, items: Vec<RemoteItem>) {
    let run = f
        .store
        .begin_sync("a", &f.account.authorization_epoch, SCOPE)
        .await
        .unwrap();
    let revision = f
        .store
        .scope_state("a", SCOPE)
        .await
        .unwrap()
        .unwrap()
        .data_revision;
    f.store
        .apply_fetched_page(page(f, run, items, true), vec![], revision)
        .await
        .unwrap();
}
async fn markers(f: &Fixture) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM pull_creation_visibility WHERE account_id='a'")
        .fetch_one(&f.store.inner.readers)
        .await
        .unwrap()
}

#[tokio::test]
async fn creation_is_visible_in_lists_count_search_body_without_enumeration_authority() {
    let f = setup().await;
    let created = item();
    publish(&f, &created).await;
    let result = f.store.query_items(query()).await.unwrap();
    assert_eq!(result.total_count, 1);
    assert_eq!(result.items[0].id, created.id);
    assert_eq!(result.coverage.state, CoverageState::Missing);
    let mut q = query();
    q.search = Some("Created pull body".into());
    assert_eq!(f.store.query_items(q).await.unwrap().total_count, 1);
    let body = f
        .store
        .detail(crate::DetailQuery {
            account_id: "a".into(),
            subject_id: created.id.clone(),
            facet: DetailFacet::Body,
            cursor: None,
            limit: 10,
        })
        .await
        .unwrap();
    assert_eq!(body.body.text, created.body);
    assert_eq!(body.body.state, DetailValueState::Known);
    assert_eq!(markers(&f).await, 1);
    let last_seen: String = sqlx::query_scalar("SELECT last_seen_run FROM scope_membership WHERE account_id='a' AND scope=? AND entity_id=?")
        .bind(SCOPE).bind(&created.id).fetch_one(&f.store.inner.readers).await.unwrap();
    assert!(last_seen.starts_with("receipt:"));
    f.store.close().await.unwrap();
}

#[tokio::test]
async fn held_terminal_feed_is_rejected_and_old_resumed_traversal_cannot_retire_creation() {
    let f = setup().await;
    let run = f
        .store
        .begin_sync("a", &f.account.authorization_epoch, SCOPE)
        .await
        .unwrap();
    let first_revision = f
        .store
        .scope_state("a", SCOPE)
        .await
        .unwrap()
        .unwrap()
        .data_revision;
    f.store
        .apply_fetched_page(page(&f, run.clone(), vec![], false), vec![], first_revision)
        .await
        .unwrap();
    let captured = f.store.scope_state("a", SCOPE).await.unwrap().unwrap();
    let held = page(&f, run.clone(), vec![], true);
    publish(&f, &item()).await;
    assert_eq!(
        f.store
            .apply_fetched_page(held.clone(), vec![], captured.data_revision)
            .await
            .unwrap_err()
            .code,
        ErrorCode::StaleView
    );
    let after = f.store.scope_state("a", SCOPE).await.unwrap().unwrap();
    assert_eq!(after.run_id, run);
    assert_eq!(after.next_cursor, Some("page-2".into()));
    assert!(after.etag.is_none());
    assert!(after.data_revision > captured.data_revision);
    f.store
        .apply_fetched_page(held, vec![], after.data_revision)
        .await
        .unwrap();
    enumerate(&f, vec![]).await;
    enumerate(&f, vec![]).await;
    assert_eq!(f.store.query_items(query()).await.unwrap().total_count, 1);
    assert_eq!(markers(&f).await, 1);
    f.store.close().await.unwrap();
}

#[tokio::test]
async fn real_observation_retires_provisional_protection_then_normal_absence_applies() {
    let f = setup().await;
    publish(&f, &item()).await;
    enumerate(&f, vec![item()]).await;
    assert_eq!(markers(&f).await, 0);
    enumerate(&f, vec![]).await;
    assert_eq!(f.store.query_items(query()).await.unwrap().total_count, 1);
    enumerate(&f, vec![]).await;
    assert_eq!(f.store.query_items(query()).await.unwrap().total_count, 0);
    assert!(f.store.item("a", &item().id).await.is_ok());
    f.store.close().await.unwrap();
}

#[tokio::test]
async fn prior_feed_identity_and_newer_or_equal_conflicting_content_are_preserved() {
    for timestamp in ["2026-10-08T00:00:00Z", "2026-10-08T00:30:00Z"] {
        let f = setup().await;
        let mut observed = item();
        observed.title = "Already current".into();
        observed.body = Some("Already changed".into());
        observed.updated_at = timestamp.into();
        enumerate(&f, vec![observed.clone()]).await;
        let before: String = sqlx::query_scalar("SELECT last_seen_run FROM scope_membership WHERE account_id='a' AND scope=? AND entity_id=?")
            .bind(SCOPE).bind(&observed.id).fetch_one(&f.store.inner.readers).await.unwrap();
        publish(&f, &item()).await;
        let result = f.store.query_items(query()).await.unwrap();
        assert_eq!(result.total_count, 1);
        assert_eq!(result.items[0].title, observed.title);
        assert_eq!(result.items[0].body, observed.body);
        assert_eq!(markers(&f).await, 0);
        let after: String = sqlx::query_scalar("SELECT last_seen_run FROM scope_membership WHERE account_id='a' AND scope=? AND entity_id=?")
            .bind(SCOPE).bind(&observed.id).fetch_one(&f.store.inner.readers).await.unwrap();
        assert_eq!(before, after);
        let count: i64 = sqlx::query_scalar("SELECT count(*) FROM resource_identities WHERE account_id='a' AND kind='pull_request' AND provider_id=?")
            .bind(&observed.provider_id).fetch_one(&f.store.inner.readers).await.unwrap();
        assert_eq!(count, 1);
        f.store.close().await.unwrap();
    }
}

#[tokio::test]
async fn delayed_receipt_does_not_resurrect_retired_real_membership() {
    let f = setup().await;
    enumerate(&f, vec![item()]).await;
    enumerate(&f, vec![]).await;
    enumerate(&f, vec![]).await;
    publish(&f, &item()).await;
    assert_eq!(f.store.query_items(query()).await.unwrap().total_count, 0);
    assert_eq!(markers(&f).await, 0);
    f.store.close().await.unwrap();
}

#[tokio::test]
async fn matching_feed_first_receipt_seeds_body_without_provisional_membership() {
    let f = setup().await;
    let created = item();
    enumerate(&f, vec![created.clone()]).await;
    publish(&f, &created).await;
    assert_eq!(markers(&f).await, 0);
    let body = f
        .store
        .detail(crate::DetailQuery {
            account_id: "a".into(),
            subject_id: created.id,
            facet: DetailFacet::Body,
            cursor: None,
            limit: 10,
        })
        .await
        .unwrap();
    assert_eq!(body.body.text, created.body);
    assert_eq!(body.body.state, DetailValueState::Known);
    f.store.close().await.unwrap();
}

#[tokio::test]
async fn newer_independent_body_observation_survives_delayed_creation() {
    let f = setup().await;
    let created = item();
    enumerate(&f, vec![created.clone()]).await;
    let lease = f
        .store
        .begin_detail(
            "a",
            &f.account.authorization_epoch,
            &created.id,
            DetailFacet::Body,
        )
        .await
        .unwrap();
    let mut body = fixtures::from_lease(&f.account, DetailFacet::Body, lease);
    body.subject_id = created.id.clone();
    body.subject_binding = Some(crate::DetailSubjectBinding {
        kind: created.kind.clone(),
        number: created.number.clone(),
        repository_id: "repo".into(),
        provider_id: created.provider_id.clone(),
        repository_provider_id: "1".into(),
        head_oid: created.head_oid.clone(),
    });
    body.body = DetailValue {
        state: DetailValueState::Known,
        text: Some("Newest separately fetched body".into()),
    };
    body.source.provider_updated_at = Some("2026-10-08T00:45:00Z".into());
    f.store.apply_detail(body).await.unwrap();
    let mut delayed = created.clone();
    delayed.updated_at = "2026-10-08T00:30:00Z".into();
    delayed.title = "Historical receipt title".into();
    publish(&f, &delayed).await;
    let body = f
        .store
        .detail(crate::DetailQuery {
            account_id: "a".into(),
            subject_id: created.id,
            facet: DetailFacet::Body,
            cursor: None,
            limit: 10,
        })
        .await
        .unwrap();
    assert_eq!(
        body.body.text.as_deref(),
        Some("Newest separately fetched body")
    );
    assert_eq!(
        f.store.query_items(query()).await.unwrap().items[0].title,
        created.title
    );
    f.store.close().await.unwrap();
}

#[tokio::test]
async fn outer_failure_rolls_back_identity_body_search_membership_and_revision() {
    let f = setup().await;
    let before = f.store.query_items(query()).await.unwrap().revision;
    {
        let mut writer = f.store.inner.writer.acquire().await.unwrap();
        let mut tx = writer.begin().await.unwrap();
        publish_in(&mut tx, &f.account, publication(&f, &item()))
            .await
            .unwrap();
        tx.rollback().await.unwrap();
    }
    assert_eq!(f.store.query_items(query()).await.unwrap().revision, before);
    assert_eq!(f.store.query_items(query()).await.unwrap().total_count, 0);
    assert_eq!(markers(&f).await, 0);
    let artifacts: i64 = sqlx::query_scalar("SELECT (SELECT count(*) FROM resource_identities WHERE account_id='a' AND kind='pull_request' AND provider_id='9007199254740999')+(SELECT count(*) FROM items_fts WHERE account_id='a' AND id=?)+(SELECT count(*) FROM detail_observations WHERE account_id='a' AND subject_id=?)")
        .bind(&item().id).bind(&item().id).fetch_one(&f.store.inner.readers).await.unwrap();
    assert_eq!(artifacts, 0);
    f.store.close().await.unwrap();
}

#[tokio::test]
async fn stale_view_and_current_access_denial_refuse_publication() {
    let f = setup().await;
    let mut writer = f.store.inner.writer.acquire().await.unwrap();
    let mut tx = writer.begin().await.unwrap();
    let created = item();
    let mut request = publication(&f, &created);
    request.authorization_view = "9999";
    assert_eq!(
        publish_in(&mut tx, &f.account, request)
            .await
            .unwrap_err()
            .code,
        ErrorCode::StaleView
    );
    tx.rollback().await.unwrap();
    drop(writer);
    let run = f
        .store
        .begin_sync("a", &f.account.authorization_epoch, SCOPE)
        .await
        .unwrap();
    assert!(!run.is_empty());
    {
        let mut writer = f.store.inner.writer.acquire().await.unwrap();
        sqlx::query("UPDATE sync_scopes SET access_denied=1 WHERE account_id='a' AND scope=?")
            .bind(SCOPE)
            .execute(&mut *writer)
            .await
            .unwrap();
        let mut tx = writer.begin().await.unwrap();
        assert_eq!(
            publish_in(&mut tx, &f.account, publication(&f, &created))
                .await
                .unwrap_err()
                .code,
            ErrorCode::StaleView
        );
        tx.rollback().await.unwrap();
    }
    assert_eq!(markers(&f).await, 0);
    f.store.close().await.unwrap();
}

#[tokio::test]
async fn disconnect_purges_provider_marker_and_reconnect_does_not_recreate_it() {
    let f = setup().await;
    publish(&f, &item()).await;
    f.store.disconnect("a").await.unwrap();
    assert_eq!(markers(&f).await, 0);
    let mut reconnect = f.store.account("a").await.unwrap();
    reconnect.state = AccountState::Active;
    reconnect.authorization_epoch =
        (reconnect.authorization_epoch.parse::<u64>().unwrap() + 1).to_string();
    let account = f.store.upsert_account(reconnect).await.unwrap();
    fixtures::project(&f.store, &account).await;
    remove_seed_pull(&f.store).await;
    assert_eq!(f.store.query_items(query()).await.unwrap().total_count, 0);
    assert_eq!(markers(&f).await, 0);
    f.store.close().await.unwrap();
}
