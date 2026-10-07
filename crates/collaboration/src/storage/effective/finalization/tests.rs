use super::*;
use crate::commands::{
    CanonicalFields, CommandDraft, CommandPayloadCodec, CommandTarget, CommandTargetKind,
    seal_command,
};
use crate::credentials::SecretToken;
use crate::delivery::*;
use crate::runtime::detail_tests::fixtures;
use crate::storage::command_admission::{CommandAdmissionPolicy, CommandProtection};
use crate::storage::delivery::DeliveryCompletion;
use crate::{
    DetailSubjectBinding, MetadataObservedField, MetadataSource, ResourceMetadataObservation,
    ResourceMetadataValues,
};

struct Payload(ItemIntentPatch);
impl CommandPayloadCodec for Payload {
    const OPERATION_KIND: &'static str = "fixture.canonical";
    const PAYLOAD_VERSION: u32 = 1;
    fn encode_payload(&self, fields: &mut CanonicalFields) -> Result<()> {
        fields.string(1, &encode(&self.0)?)
    }
}
struct Admission(ItemIntentPatch);
#[async_trait::async_trait]
impl CommandAdmissionPolicy for Admission {
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
enum Materializer {
    Noop,
    Body(DetailCommit),
    TamperedBody(DetailCommit),
    TamperedSearch(DetailCommit),
    Notification(CanonicalNotificationObservation),
}
#[async_trait::async_trait]
impl CommandDeliveryPolicy for Materializer {
    fn operation_kind(&self) -> &'static str {
        Payload::OPERATION_KIND
    }
    fn payload_version(&self) -> u32 {
        1
    }
    async fn validate_claim(
        &self,
        _: &mut Transaction<'_, Sqlite>,
        _: &DeliveryCommand,
        _: &RemoteAccount,
        _: &[u8],
    ) -> Result<ClaimDecision> {
        Ok(ClaimDecision::Ready(vec![]))
    }
    fn validate_evidence(
        &self,
        command: &DeliveryCommand,
        _: EvidencePurpose,
        proof: &OperationEvidence,
    ) -> bool {
        proof.kind == "fixture.receipt" && proof.version == 1 && proof.payload == command.hash
    }
    async fn finalize_in(
        &self,
        context: &mut DeliveryFinalization<'_, '_>,
        _: &DeliveryCommand,
        _: EvidencePurpose,
        _: &OperationEvidence,
    ) -> Result<()> {
        match self {
            Self::Noop => Ok(()),
            Self::Body(page) => context.observe_body(page.clone()).await,
            Self::TamperedBody(page) => {
                context.observe_body(page.clone()).await?;
                sqlx::query("UPDATE items SET json=json_set(json,'$.title','after witness') WHERE account_id='a' AND id='pull'")
                    .execute(&mut **context.transaction()).await.map_err(storage_error)?;
                Ok(())
            }
            Self::TamperedSearch(page) => {
                context.observe_body(page.clone()).await?;
                sqlx::query("DELETE FROM items_fts WHERE account_id='a' AND id='pull'")
                    .execute(&mut **context.transaction())
                    .await
                    .map_err(storage_error)?;
                Ok(())
            }
            Self::Notification(observation) => {
                context
                    .observe_notification(CanonicalNotificationObservation {
                        expected: observation.expected.clone(),
                        authorization_view: observation.authorization_view.clone(),
                        instance_id: observation.instance_id.clone(),
                        source: observation.source.clone(),
                        values: observation.values.clone(),
                    })
                    .await
            }
        }
    }
    async fn dispatch(&self, _: &SecretToken, _: DispatchRequest) -> DeliveryReport {
        panic!("synthetic storage proof; no network")
    }
}
async fn setup() -> (tempfile::TempDir, Store, RemoteAccount) {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path().join("canonical.db")).await.unwrap();
    let account = fixtures::seed(&store, "a").await;
    (dir, store, account)
}
async fn admit(
    store: &Store,
    account: &RemoteAccount,
    target: &str,
    kind: CommandTargetKind,
    patch: ItemIntentPatch,
) -> crate::CommandSubmission {
    let submission = seal_command(CommandDraft {
        command_id: Uuid::new_v4().to_string(),
        account_id: account.id.clone(),
        authorization_epoch: account.authorization_epoch.clone(),
        target: CommandTarget::new(kind, target, Some("repo".into())).unwrap(),
        payload: Payload(patch.clone()),
        guards: vec![],
        dependencies: vec![],
    })
    .unwrap();
    store
        .admit_command(&submission, &Admission(patch))
        .await
        .unwrap();
    submission
}
fn title(value: &str) -> ItemIntentPatch {
    ItemIntentPatch {
        title: Some(value.into()),
        ..Default::default()
    }
}
fn all_fields() -> ItemIntentPatch {
    ItemIntentPatch {
        body: Some(BodyIntent {
            text: Some("authored body".into()),
        }),
        state: Some("closed".into()),
        ..title("authored title")
    }
}
fn time() -> DeliveryTime {
    let now = chrono::Utc::now().to_rfc3339();
    DeliveryTime {
        now: now.clone(),
        command_now: now,
    }
}
async fn claim(
    store: &Store,
    account: &RemoteAccount,
    submission: &crate::CommandSubmission,
    policy: &Materializer,
) -> DispatchRequest {
    let command = store
        .delivery_command(&account.id, submission.command_id())
        .await
        .unwrap();
    store
        .claim_delivery(&command, account, policy, &[], &time())
        .await
        .unwrap()
        .request
        .unwrap()
}
async fn complete(
    store: &Store,
    request: &DispatchRequest,
    policy: &Materializer,
    purpose: EvidencePurpose,
) -> Result<String> {
    let proof = OperationEvidence {
        kind: "fixture.receipt".into(),
        version: 1,
        payload: request.command.hash.to_vec(),
    };
    let report = DeliveryReport {
        outcome: match purpose {
            EvidencePurpose::Confirmed => DeliveryOutcome::Confirmed(proof),
            EvidencePurpose::Accepted => DeliveryOutcome::Accepted(proof),
            EvidencePurpose::Rejected => DeliveryOutcome::Rejected(proof),
            _ => panic!("unsupported fixture"),
        },
        ..DeliveryReport::unknown()
    };
    let now = chrono::Utc::now().to_rfc3339();
    store
        .complete_delivery(
            &request.command,
            policy,
            DeliveryCompletion {
                account: &request.account,
                attempt: (request.command.state == DeliveryState::Sending)
                    .then_some(request.attempt),
                report: &report,
                now: &now,
                next: &now,
            },
        )
        .await
}
async fn page(store: &Store, account: &RemoteAccount) -> DetailCommit {
    let mut page = fixtures::commit(store, account, DetailFacet::Body).await;
    page.body = fixtures::known(Some("canonical body after normalization"));
    page.source.provider_updated_at = Some("2026-10-04T00:00:00Z".into());
    page.source.observed_at = "2026-10-04T00:00:01Z".into();
    page.subject_binding = Some(DetailSubjectBinding {
        repository_id: "repo".into(),
        repository_provider_id: "1".into(),
        provider_id: "9007199254740997".into(),
        number: Some("67".into()),
        kind: RemoteItemKind::PullRequest,
        head_oid: None,
    });
    page.metadata = Some(ResourceMetadataObservation {
        kind: RemoteItemKind::PullRequest,
        values: ResourceMetadataValues {
            title: Some("canonical normalized title".into()),
            state: Some("closed".into()),
            ..Default::default()
        },
        fields: vec![
            MetadataObservedField {
                field: MetadataField::Title,
                state: DetailValueState::Known,
            },
            MetadataObservedField {
                field: MetadataField::State,
                state: DetailValueState::Known,
            },
        ],
        source: MetadataSource {
            source: page.source.source.clone(),
            adapter_version: 1,
            provider_updated_at: page.source.provider_updated_at.clone(),
            observed_at: page.source.observed_at.clone(),
        },
    });
    page
}
fn query(search: Option<&str>, state: Option<&str>) -> ItemQuery {
    ItemQuery {
        account_id: "a".into(),
        kind: RemoteItemKind::PullRequest,
        repository_id: Some("repo".into()),
        search: search.map(Into::into),
        state: state.map(Into::into),
        cursor: None,
        limit: 10,
    }
}

#[tokio::test]
async fn no_op_cannot_retire_effect_or_publish_false_confirmation() {
    let (_dir, store, account) = setup().await;
    let command = admit(
        &store,
        &account,
        "pull",
        CommandTargetKind::PullRequest,
        title("pending"),
    )
    .await;
    let request = claim(&store, &account, &command, &Materializer::Noop).await;
    let revision = store.revision().await.unwrap();
    assert!(
        complete(
            &store,
            &request,
            &Materializer::Noop,
            EvidencePurpose::Confirmed
        )
        .await
        .is_err()
    );
    assert_eq!(store.revision().await.unwrap(), revision);
    let command = store
        .delivery_command("a", command.command_id())
        .await
        .unwrap();
    assert_eq!(command.state, DeliveryState::Sending);
    assert!(command.evidence.is_empty());
    assert_eq!(
        store.item("a", "pull").await.unwrap().item.unwrap().title,
        "pending"
    );
    assert_eq!(
        store.detail_subject("a", "pull").await.unwrap().title,
        "Summary"
    );
    store.close().await.unwrap();
}

#[tokio::test]
async fn canonical_normalization_and_successor_replay_are_atomic_across_all_queries() {
    let (_dir, store, account) = setup().await;
    let first = admit(
        &store,
        &account,
        "pull",
        CommandTargetKind::PullRequest,
        all_fields(),
    )
    .await;
    let second = admit(
        &store,
        &account,
        "pull",
        CommandTargetKind::PullRequest,
        title("successor title"),
    )
    .await;
    let policy = Materializer::Body(page(&store, &account).await);
    let request = claim(&store, &account, &first, &policy).await;
    complete(&store, &request, &policy, EvidencePurpose::Confirmed)
        .await
        .unwrap();
    let raw = store.detail_subject("a", "pull").await.unwrap();
    assert_eq!(raw.title, "canonical normalized title");
    assert_eq!(
        raw.body.as_deref(),
        Some("canonical body after normalization")
    );
    assert_eq!(raw.state, "closed");
    let snapshot = store.item("a", "pull").await.unwrap();
    assert_eq!(snapshot.item.unwrap().title, "successor title");
    assert_eq!(
        snapshot.pending_intent.unwrap().commands[0].command_id,
        second.command_id()
    );
    let detail = store
        .detail(fixtures::query("a", DetailFacet::Body))
        .await
        .unwrap();
    assert_eq!(detail.body.text, raw.body);
    assert_eq!(
        detail.metadata.unwrap().values.title.as_deref(),
        Some("successor title")
    );
    assert_eq!(
        store
            .query_items(query(Some("canonical body"), Some("closed")))
            .await
            .unwrap()
            .total_count,
        1
    );
    assert_eq!(
        store
            .query_items(query(Some("authored body"), None))
            .await
            .unwrap()
            .total_count,
        0
    );
    assert_eq!(
        store
            .delivery_command("a", first.command_id())
            .await
            .unwrap()
            .state,
        DeliveryState::Confirmed
    );
    // Reject only the successor through the real delivery transition seam.
    let next = claim(&store, &account, &second, &Materializer::Noop).await;
    complete(
        &store,
        &next,
        &Materializer::Noop,
        EvidencePurpose::Rejected,
    )
    .await
    .unwrap();
    let snapshot = store.item("a", "pull").await.unwrap();
    assert_eq!(snapshot.item.unwrap().title, raw.title);
    assert!(snapshot.pending_intent.is_none());
    assert_eq!(
        store
            .query_items(query(Some("canonical normalized title"), None))
            .await
            .unwrap()
            .total_count,
        1
    );
    store.close().await.unwrap();
}

#[tokio::test]
async fn materializer_failure_rolls_back_body_summary_search_evidence_and_state() {
    let (_dir, store, account) = setup().await;
    let command = admit(
        &store,
        &account,
        "pull",
        CommandTargetKind::PullRequest,
        all_fields(),
    )
    .await;
    let policy = Materializer::Body(page(&store, &account).await);
    let request = claim(&store, &account, &command, &policy).await;
    let revision = store.revision().await.unwrap();
    let raw = store.detail_subject("a", "pull").await.unwrap();
    let mut writer = store.inner.writer.acquire().await.unwrap();
    sqlx::query("CREATE TRIGGER fail_canonical_item BEFORE UPDATE ON items BEGIN SELECT RAISE(ABORT,'injected disk fault'); END;").execute(&mut *writer).await.unwrap();
    drop(writer);
    assert!(
        complete(&store, &request, &policy, EvidencePurpose::Confirmed)
            .await
            .is_err()
    );
    assert_eq!(store.revision().await.unwrap(), revision);
    assert_eq!(store.detail_subject("a", "pull").await.unwrap(), raw);
    assert!(
        store
            .detail(fixtures::query("a", DetailFacet::Body))
            .await
            .unwrap()
            .metadata
            .is_none()
    );
    assert_eq!(
        store
            .query_items(query(Some("authored body"), Some("closed")))
            .await
            .unwrap()
            .total_count,
        1
    );
    let command = store
        .delivery_command("a", command.command_id())
        .await
        .unwrap();
    assert_eq!(command.state, DeliveryState::Sending);
    assert!(command.evidence.is_empty());
    store.close().await.unwrap();
}

#[tokio::test]
async fn held_authorization_head_run_and_omitted_field_cannot_certify_canonical_base() {
    for fault in [
        "account",
        "view",
        "head",
        "run",
        "omitted",
        "missing_time",
        "old_time",
        "source",
    ] {
        let (_dir, store, account) = setup().await;
        let mut page = page(&store, &account).await;
        // Persist a known old body at the same observed time. An omitted later
        // response must not be mistaken for fresh canonical body confirmation.
        if fault == "omitted" {
            store.apply_detail(page.clone()).await.unwrap();
            page = self::page(&store, &account).await;
            page.body = DetailValue {
                state: DetailValueState::Omitted,
                text: None,
            };
        }
        let command = admit(
            &store,
            &account,
            "pull",
            CommandTargetKind::PullRequest,
            all_fields(),
        )
        .await;
        match fault {
            "account" => page.account_id = "different".into(),
            "view" => page.authorization_view = "999999".into(),
            "head" => page.subject_binding.as_mut().unwrap().head_oid = Some("moved".into()),
            "run" => page.run_id = "obsolete".into(),
            "missing_time" => page.source.provider_updated_at = None,
            "old_time" => page.source.provider_updated_at = Some("2026-10-02T00:00:00Z".into()),
            "source" => {
                page.metadata.as_mut().unwrap().source.source = "another representation".into()
            }
            _ => {}
        }
        let policy = Materializer::Body(page);
        let request = claim(&store, &account, &command, &policy).await;
        assert!(
            complete(&store, &request, &policy, EvidencePurpose::Confirmed)
                .await
                .is_err(),
            "{fault}"
        );
        let command = store
            .delivery_command("a", command.command_id())
            .await
            .unwrap();
        assert_eq!(command.state, DeliveryState::Sending, "{fault}");
        assert!(command.evidence.is_empty(), "{fault}");
        assert!(
            store
                .item("a", "pull")
                .await
                .unwrap()
                .pending_intent
                .is_some()
        );
        store.close().await.unwrap();
    }
}

#[tokio::test]
async fn raw_sql_after_witness_cannot_retire_the_effect() {
    let (_dir, store, account) = setup().await;
    let command = admit(
        &store,
        &account,
        "pull",
        CommandTargetKind::PullRequest,
        title("pending"),
    )
    .await;
    let policy = Materializer::TamperedBody(page(&store, &account).await);
    let request = claim(&store, &account, &command, &policy).await;
    assert!(
        complete(&store, &request, &policy, EvidencePurpose::Confirmed)
            .await
            .is_err()
    );
    assert_eq!(
        store.detail_subject("a", "pull").await.unwrap().title,
        "Summary"
    );
    assert_eq!(
        store.item("a", "pull").await.unwrap().item.unwrap().title,
        "pending"
    );
    store.close().await.unwrap();
}

#[tokio::test]
async fn accepted_needs_no_canonical_witness_and_preserves_pending_fields() {
    let (_dir, store, account) = setup().await;
    let command = admit(
        &store,
        &account,
        "pull",
        CommandTargetKind::PullRequest,
        title("pending"),
    )
    .await;
    let request = claim(&store, &account, &command, &Materializer::Noop).await;
    complete(
        &store,
        &request,
        &Materializer::Noop,
        EvidencePurpose::Accepted,
    )
    .await
    .unwrap();
    assert_eq!(
        store
            .delivery_command("a", command.command_id())
            .await
            .unwrap()
            .state,
        DeliveryState::Accepted
    );
    assert_eq!(
        store.item("a", "pull").await.unwrap().item.unwrap().title,
        "pending"
    );
    assert_eq!(
        store.detail_subject("a", "pull").await.unwrap().title,
        "Summary"
    );
    store.close().await.unwrap();
}

#[tokio::test]
async fn canonical_notification_updates_unread_counts_without_authored_guessing() {
    let (_dir, store, account) = setup().await;
    let mut notification = store.detail_subject("a", "pull").await.unwrap();
    notification.id = "notification".into();
    notification.kind = RemoteItemKind::Notification;
    notification.provider_id = "thread1".into();
    notification.number = None;
    notification.unread = Some(true);
    notification.state = "pending".into();
    notification.body = None;
    let run = store.begin_sync("a", "1", "notifications").await.unwrap();
    store
        .apply_page(PageCommit {
            account_id: "a".into(),
            authorization_epoch: "1".into(),
            scope: "notifications".into(),
            run_id: run,
            repositories: vec![],
            items: vec![notification.clone()],
            endpoint_aliases: vec![],
            next_cursor: None,
            etag: None,
            last_modified: None,
            not_modified: false,
            complete: true,
            observed_at: notification.updated_at.clone(),
        })
        .await
        .unwrap();
    let command = admit(
        &store,
        &account,
        "notification",
        CommandTargetKind::Notification,
        ItemIntentPatch {
            unread: Some(false),
            ..Default::default()
        },
    )
    .await;
    let policy = Materializer::Notification(CanonicalNotificationObservation {
        expected: notification.clone(),
        authorization_view: store
            .item("a", "notification")
            .await
            .unwrap()
            .authorization_view,
        instance_id: store.provider_instance("a").await.unwrap().id,
        source: MetadataSource {
            source: "fixture/notification".into(),
            adapter_version: 1,
            provider_updated_at: Some("2026-10-03T05:30:00+05:30".into()),
            observed_at: notification.updated_at.clone(),
        },
        values: ItemIntentPatch {
            unread: Some(false),
            ..Default::default()
        },
    });
    let request = claim(&store, &account, &command, &policy).await;
    complete(&store, &request, &policy, EvidencePurpose::Confirmed)
        .await
        .unwrap();
    let snapshot = store.item("a", "notification").await.unwrap();
    let item = snapshot.item.unwrap();
    assert_eq!(item.unread, Some(false));
    assert_eq!(item.updated_at, "2026-10-03T00:00:00.000000000Z");
    assert!(snapshot.pending_intent.is_none());
    for (state, count) in [("unread", 0), ("read", 1)] {
        let page = store
            .query_items(ItemQuery {
                kind: RemoteItemKind::Notification,
                repository_id: None,
                state: Some(state.into()),
                ..query(None, None)
            })
            .await
            .unwrap();
        assert_eq!(page.total_count, count);
        let page = store
            .inbox(InboxQuery {
                account_id: "a".into(),
                remote_state: Some(state.into()),
                local_state: LocalInboxFilter::All,
                search: None,
                cursor: None,
                limit: 10,
            })
            .await
            .unwrap();
        assert_eq!(page.total_count, count);
    }
    store.close().await.unwrap();
}

async fn feed(store: &Store, account: &RemoteAccount, item: RemoteItem) {
    let run_id = store
        .begin_sync(
            &account.id,
            &account.authorization_epoch,
            "repo:repo:pull_request",
        )
        .await
        .unwrap();
    store
        .apply_page(PageCommit {
            account_id: account.id.clone(),
            authorization_epoch: account.authorization_epoch.clone(),
            scope: "repo:repo:pull_request".into(),
            run_id,
            repositories: vec![],
            items: vec![item],
            endpoint_aliases: vec![],
            next_cursor: None,
            etag: None,
            last_modified: None,
            not_modified: false,
            complete: true,
            observed_at: "2026-10-05T00:00:00Z".into(),
        })
        .await
        .unwrap();
}

#[tokio::test]
async fn confirmed_canonical_timestamp_blocks_older_feed_but_allows_equal_and_newer_observations() {
    let (_dir, store, account) = setup().await;
    let command = admit(
        &store,
        &account,
        "pull",
        CommandTargetKind::PullRequest,
        title("pending"),
    )
    .await;
    let policy = Materializer::Body(page(&store, &account).await);
    let request = claim(&store, &account, &command, &policy).await;
    complete(&store, &request, &policy, EvidencePurpose::Confirmed)
        .await
        .unwrap();
    let canonical = store.detail_subject("a", "pull").await.unwrap();
    assert_eq!(canonical.updated_at, "2026-10-04T00:00:00.000000000Z");
    let mut stale = canonical.clone();
    stale.title = "older feed value".into();
    stale.updated_at = "2026-10-03T12:00:00Z".into();
    feed(&store, &account, stale).await;
    assert_eq!(
        store.item("a", "pull").await.unwrap().item.unwrap().title,
        canonical.title
    );
    for timestamp in ["2026-10-04T05:30:00+05:30", "2026-10-05T00:00:00Z"] {
        let mut next = canonical.clone();
        next.title = timestamp.into();
        next.updated_at = timestamp.into();
        feed(&store, &account, next).await;
        assert_eq!(
            store.item("a", "pull").await.unwrap().item.unwrap().title,
            timestamp
        );
    }
    store.close().await.unwrap();
}

struct NoEffectAdmission;
#[async_trait::async_trait]
impl CommandAdmissionPolicy for NoEffectAdmission {
    const OPERATION_KIND: &'static str = Payload::OPERATION_KIND;
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

#[tokio::test]
async fn canonical_no_effect_command_advances_list_projection_and_change_hint() {
    let (_dir, store, account) = setup().await;
    let mut second = store.detail_subject("a", "pull").await.unwrap();
    second.id = "another".into();
    second.provider_id = "another-native".into();
    second.number = Some("68".into());
    feed(&store, &account, second).await;
    // Feed fixtures visit one page per call; re-observe the original so both
    // remain active before obtaining a real pagination cursor.
    fixtures::project(&store, &account).await;
    let submission = seal_command(CommandDraft {
        command_id: Uuid::new_v4().to_string(),
        account_id: "a".into(),
        authorization_epoch: "1".into(),
        target: CommandTarget::new(CommandTargetKind::PullRequest, "pull", Some("repo".into()))
            .unwrap(),
        payload: Payload(title("not an authored effect")),
        guards: vec![],
        dependencies: vec![],
    })
    .unwrap();
    store
        .admit_command(&submission, &NoEffectAdmission)
        .await
        .unwrap();
    let mut q = query(None, None);
    q.limit = 1;
    let cursor = store
        .query_items(q.clone())
        .await
        .unwrap()
        .next_cursor
        .unwrap();
    let policy = Materializer::Body(page(&store, &account).await);
    let request = claim(&store, &account, &submission, &policy).await;
    let before = store.revision().await.unwrap();
    complete(&store, &request, &policy, EvidencePurpose::Confirmed)
        .await
        .unwrap();
    let changes = store.changes_since(&before).await.unwrap();
    assert!(
        changes
            .changes
            .iter()
            .any(|change| change.scope == "repo:repo:pull_request")
    );
    q.cursor = Some(cursor);
    assert_eq!(
        store.query_items(q).await.unwrap_err().code,
        ErrorCode::StaleView
    );
    assert_eq!(
        store
            .query_items(query(Some("canonical normalized title"), None))
            .await
            .unwrap()
            .total_count,
        1
    );
    assert!(
        store
            .item("a", "pull")
            .await
            .unwrap()
            .pending_intent
            .is_none()
    );
    store.close().await.unwrap();
}

#[tokio::test]
async fn search_tamper_after_witness_cannot_publish_inconsistent_confirmation() {
    let (_dir, store, account) = setup().await;
    let command = admit(
        &store,
        &account,
        "pull",
        CommandTargetKind::PullRequest,
        title("pending"),
    )
    .await;
    let policy = Materializer::TamperedSearch(page(&store, &account).await);
    let request = claim(&store, &account, &command, &policy).await;
    assert!(
        complete(&store, &request, &policy, EvidencePurpose::Confirmed)
            .await
            .is_err()
    );
    assert_eq!(
        store.detail_subject("a", "pull").await.unwrap().title,
        "Summary"
    );
    assert_eq!(
        store
            .query_items(query(Some("pending"), None))
            .await
            .unwrap()
            .total_count,
        1
    );
    store.close().await.unwrap();
}
