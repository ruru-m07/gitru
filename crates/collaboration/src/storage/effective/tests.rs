use super::*;
use crate::commands::{
    CanonicalFields, CommandDraft, CommandPayloadCodec, CommandTarget, CommandTargetKind,
    seal_command,
};
use crate::effective::BodyIntent;
use crate::runtime::detail_tests::fixtures;
use crate::storage::command_admission::{CommandAdmissionPolicy, CommandProtection};
use crate::{DetailFacet, DetailValueState};

struct Payload(ItemIntentPatch);
impl CommandPayloadCodec for Payload {
    const OPERATION_KIND: &'static str = "fixture.scalar_intent";
    const PAYLOAD_VERSION: u32 = 1;
    fn encode_payload(&self, fields: &mut CanonicalFields) -> Result<()> {
        fields.string(1, &encode(&self.0)?)
    }
}
struct Policy(ItemIntentPatch);
#[async_trait::async_trait]
impl CommandAdmissionPolicy for Policy {
    const OPERATION_KIND: &'static str = Payload::OPERATION_KIND;
    const PAYLOAD_VERSION: u32 = 1;
    fn effect(&self, _: &crate::CommandSubmission) -> Result<Option<ItemIntentPatch>> {
        Ok(Some(self.0.clone()))
    }
    async fn validate(
        &self,
        _: &mut Transaction<'_, Sqlite>,
        _: &RemoteAccount,
        _: &crate::CommandSubmission,
    ) -> Result<Vec<CommandProtection>> {
        Ok(vec![])
    }
}
async fn setup() -> (tempfile::TempDir, Store) {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path().join("intent.db")).await.unwrap();
    let account = fixtures::seed(&store, "a").await;
    fixtures::seed(&store, "b").await;
    let body = fixtures::commit(&store, &account, DetailFacet::Body).await;
    store.apply_detail(body).await.unwrap();
    (dir, store)
}
fn query(account: &str) -> ItemQuery {
    ItemQuery {
        account_id: account.into(),
        kind: RemoteItemKind::PullRequest,
        repository_id: Some("repo".into()),
        state: None,
        search: None,
        cursor: None,
        limit: 10,
    }
}
fn seal(
    account: &str,
    target: &str,
    kind: CommandTargetKind,
    patch: ItemIntentPatch,
) -> crate::CommandSubmission {
    seal_command(CommandDraft {
        command_id: Uuid::new_v4().to_string(),
        account_id: account.into(),
        authorization_epoch: "1".into(),
        target: CommandTarget::new(kind, target, Some("repo".into())).unwrap(),
        payload: Payload(patch),
        guards: vec![],
        dependencies: vec![],
    })
    .unwrap()
}
async fn admit(store: &Store, patch: ItemIntentPatch) -> crate::CommandSubmission {
    let command = seal("a", "pull", CommandTargetKind::PullRequest, patch.clone());
    store.admit_command(&command, &Policy(patch)).await.unwrap();
    command
}
async fn state(store: &Store, id: &str, next: &str) {
    let mut writer = store.inner.writer.acquire().await.unwrap();
    let mut tx = writer.begin().await.unwrap();
    sqlx::query("UPDATE commands SET state=? WHERE account_id='a' AND command_id=?")
        .bind(next)
        .bind(id)
        .execute(&mut *tx)
        .await
        .unwrap();
    refresh_target_in(&mut tx, "a", "pull").await.unwrap();
    tx.commit().await.unwrap();
}
fn patch(title: &str) -> ItemIntentPatch {
    ItemIntentPatch {
        title: Some(title.into()),
        ..Default::default()
    }
}

#[tokio::test]
async fn committed_intent_is_shared_by_list_detail_filtered_count_and_literal_search() {
    let (_dir, store) = setup().await;
    let raw = store.detail_subject("a", "pull").await.unwrap();
    let intent = ItemIntentPatch {
        title: Some("Queued 雪 phrase".into()),
        body: Some(BodyIntent {
            text: Some("Pending prose".into()),
        }),
        state: Some("closed".into()),
        unread: None,
    };
    let command = admit(&store, intent.clone()).await;
    let snapshot = store.item("a", "pull").await.unwrap();
    assert_eq!(snapshot.item.unwrap().title, "Queued 雪 phrase");
    assert_eq!(
        snapshot.pending_intent.unwrap().commands[0].command_id,
        command.command_id()
    );
    let body = store
        .detail(fixtures::query("a", DetailFacet::Body))
        .await
        .unwrap();
    assert_eq!(body.body.text.as_deref(), Some("Pending prose"));
    assert_eq!(body.body.state, DetailValueState::Known);
    assert_eq!(body.pending_intent.unwrap().commands[0].fields.len(), 3);
    assert_eq!(store.detail_subject("a", "pull").await.unwrap(), raw);
    for (search, state, count) in [
        (None, Some("closed"), 1),
        (Some("Queued 雪"), None, 1),
        (Some("Pending prose"), None, 1),
        (Some("Summary"), None, 0),
        (None, Some("open"), 0),
        (Some("Queued OR Summary"), None, 0),
    ] {
        let mut query = query("a");
        query.search = search.map(Into::into);
        query.state = state.map(Into::into);
        let page = store.query_items(query).await.unwrap();
        assert_eq!(page.total_count, count);
        assert_eq!(page.items.len() as u64, count);
    }
    let other = store.query_items(query("b")).await.unwrap();
    assert_eq!(other.items[0].title, "Summary");
    assert!(other.pending_intents.is_empty());
    // Exact retry cannot re-author an effect through a changed policy.
    let before = store.revision().await.unwrap();
    assert!(
        store
            .admit_command(&command, &Policy(patch("different")))
            .await
            .unwrap()
            .duplicate
    );
    assert_eq!(store.revision().await.unwrap(), before);
}

#[tokio::test]
async fn base_refresh_and_rejected_predecessor_replay_successors_across_restart() {
    let (dir, store) = setup().await;
    let first = admit(
        &store,
        ItemIntentPatch {
            body: Some(BodyIntent {
                text: Some("first body".into()),
            }),
            ..patch("first title")
        },
    )
    .await;
    let second = admit(&store, patch("second title")).await;
    for next in [
        "sending",
        "accepted",
        "outcome_unknown",
        "conflict",
        "retry_wait",
    ] {
        state(&store, first.command_id(), next).await;
        assert_eq!(
            store.item("a", "pull").await.unwrap().item.unwrap().title,
            "second title"
        );
    }
    fixtures::project(&store, &store.account("a").await.unwrap()).await;
    assert_eq!(
        store
            .item("a", "pull")
            .await
            .unwrap()
            .item
            .unwrap()
            .body
            .as_deref(),
        Some("first body")
    );
    state(&store, first.command_id(), "rejected").await;
    let item = store.item("a", "pull").await.unwrap();
    assert_eq!(item.item.unwrap().title, "second title");
    assert_eq!(item.pending_intent.unwrap().commands.len(), 1);
    let detail = store
        .detail(fixtures::query("a", DetailFacet::Body))
        .await
        .unwrap();
    assert_eq!(
        detail.body.text.as_deref(),
        Some("saved authoritative body")
    );
    store.close().await.unwrap();
    drop(store);
    let store = Store::open(dir.path().join("intent.db")).await.unwrap();
    assert_eq!(
        store.query_items(query("a")).await.unwrap().items[0].title,
        "second title"
    );
    state(&store, second.command_id(), "cancelled").await;
    let page = store.query_items(query("a")).await.unwrap();
    assert_eq!(page.items[0].title, "Summary");
    assert!(page.pending_intents.is_empty());
}

#[tokio::test]
async fn cursor_and_projection_change_are_atomic_but_provider_evidence_stays_unchanged() {
    let (_dir, store) = setup().await;
    let evidence = store
        .detail_evidence("a", "pull", DetailFacet::Body)
        .await
        .unwrap();
    // Two selected cached records make a real local pagination cursor.
    let mut writer = store.inner.writer.acquire().await.unwrap();
    sqlx::raw_sql("INSERT INTO items SELECT account_id,'pull-2',repository_id,kind,state,updated_at,json_set(json,'$.id','pull-2') FROM items WHERE account_id='a'; INSERT INTO scope_membership(account_id,scope,entity_id,last_seen_run,missing_count,active) SELECT account_id,scope,'pull-2',last_seen_run,missing_count,active FROM scope_membership WHERE account_id='a' AND entity_id='pull';").execute(&mut *writer).await.unwrap();
    drop(writer);
    let mut q = query("a");
    q.limit = 1;
    let page = store.query_items(q.clone()).await.unwrap();
    assert!(page.next_cursor.is_some());
    let before = store.revision().await.unwrap();
    admit(&store, patch("different")).await;
    q.cursor = page.next_cursor;
    assert_eq!(
        store.query_items(q).await.unwrap_err().code,
        ErrorCode::StaleView
    );
    let changes = store.changes_since(&before).await.unwrap();
    assert!(
        changes
            .changes
            .iter()
            .any(|change| change.scope == "effective:pull")
    );
    assert_eq!(
        store
            .detail_evidence("a", "pull", DetailFacet::Body)
            .await
            .unwrap(),
        evidence
    );
}

#[tokio::test]
async fn projection_failure_rolls_back_command_effect_revision_and_search() {
    let (_dir, store) = setup().await;
    let before = store.revision().await.unwrap();
    let mut writer = store.inner.writer.acquire().await.unwrap();
    sqlx::query("CREATE TRIGGER fail_effect BEFORE INSERT ON effective_item_overrides BEGIN SELECT RAISE(ABORT,'injected effect failure'); END;").execute(&mut *writer).await.unwrap();
    drop(writer);
    let command = seal(
        "a",
        "pull",
        CommandTargetKind::PullRequest,
        patch("failure"),
    );
    assert!(
        store
            .admit_command(&command, &Policy(patch("failure")))
            .await
            .is_err()
    );
    assert_eq!(store.revision().await.unwrap(), before);
    for table in [
        "commands",
        "command_effects",
        "effective_item_overrides",
        "effective_items_fts",
        "effective_item_revisions",
    ] {
        let count: i64 =
            sqlx::query_scalar(sqlx::AssertSqlSafe(format!("SELECT count(*) FROM {table}")))
                .fetch_one(&store.inner.readers)
                .await
                .unwrap();
        assert_eq!(count, 0, "{table}");
    }
}

#[tokio::test]
async fn active_chain_is_bounded_and_rejection_frees_capacity_without_deleting_history() {
    let (_dir, store) = setup().await;
    let mut ids = vec![];
    for index in 0..64 {
        ids.push(admit(&store, patch(&format!("title{index}"))).await);
    }
    let request = seal(
        "a",
        "pull",
        CommandTargetKind::PullRequest,
        patch("overflow"),
    );
    assert!(
        store
            .admit_command(&request, &Policy(patch("overflow")))
            .await
            .is_err()
    );
    state(&store, ids[0].command_id(), "rejected").await;
    store
        .admit_command(&request, &Policy(patch("overflow")))
        .await
        .unwrap();
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM command_effects")
        .fetch_one(&store.inner.readers)
        .await
        .unwrap();
    assert_eq!(count, 65);
    assert_eq!(
        store
            .item("a", "pull")
            .await
            .unwrap()
            .pending_intent
            .unwrap()
            .commands
            .len(),
        64
    );
}

#[tokio::test]
async fn old_epoch_effect_cannot_mask_new_epoch_observations_or_search() {
    let (_dir, store) = setup().await;
    admit(&store, patch("old secret")).await;
    let mut account = store.account("a").await.unwrap();
    account.authorization_epoch = "2".into();
    store.upsert_account(account.clone()).await.unwrap();
    fixtures::project(&store, &account).await;
    let mut q = query("a");
    q.search = Some("Summary".into());
    let page = store.query_items(q).await.unwrap();
    assert_eq!(page.total_count, 1);
    assert!(page.pending_intents.is_empty());
    let mut q = query("a");
    q.search = Some("old secret".into());
    assert_eq!(store.query_items(q).await.unwrap().total_count, 0);
}

#[tokio::test]
async fn restore_preserves_effect_bytes_but_quarantine_never_reactivates_projection() {
    let (dir, store) = setup().await;
    let command = admit(&store, patch("author-owned intent")).await;
    let backup = dir.path().join("backup.db");
    store.backup_to(&backup).await.unwrap();
    let bytes: String = sqlx::query_scalar("SELECT patch_json FROM command_effects")
        .fetch_one(&store.inner.readers)
        .await
        .unwrap();
    store.close().await.unwrap();
    drop(store);
    let target = dir.path().join("intent.db");
    let session = crate::recovery::RecoverySession::prepare(&target, &backup)
        .await
        .unwrap();
    let confirmation = session.preview().confirmation_id.clone();
    session
        .confirm(
            &confirmation,
            crate::recovery::RestoreChoice::ReplaceCurrentData,
        )
        .unwrap();
    let store = Store::open(&target).await.unwrap();
    let after: String = sqlx::query_scalar("SELECT patch_json FROM command_effects")
        .fetch_one(&store.inner.readers)
        .await
        .unwrap();
    assert_eq!(after, bytes);
    let mut account = store.account("a").await.unwrap();
    account.authorization_epoch =
        (account.authorization_epoch.parse::<u64>().unwrap() + 1).to_string();
    account.state = AccountState::Active;
    store.upsert_account(account.clone()).await.unwrap();
    fixtures::project(&store, &account).await;
    let item = store.item("a", "pull").await.unwrap();
    assert_eq!(item.item.unwrap().title, "Summary");
    assert!(item.pending_intent.is_none());
    assert!(
        store
            .command_receipt("a", command.command_id())
            .await
            .unwrap()
            .is_some()
    );
}
