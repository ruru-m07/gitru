use super::*;
use crate::commands::{
    CanonicalFields, CommandDraft, CommandPayloadCodec, CommandSubmission, CommandTarget,
    CommandTargetKind, seal_command,
};
use crate::delivery::DeliveryTime;
use crate::effective::ItemIntentPatch;
use crate::runtime::detail_tests::fixtures;
use crate::storage::command_admission::{
    self, CommandAdmissionPolicy, CommandProtection, CommandReceipt,
};

#[derive(Clone, Serialize, Deserialize)]
struct Payload {
    base: String,
    desired: String,
}
impl CommandPayloadCodec for Payload {
    const OPERATION_KIND: &'static str = "fixture.recovery_title";
    const PAYLOAD_VERSION: u32 = 1;
    fn encode_payload(&self, fields: &mut CanonicalFields) -> Result<()> {
        fields.string(1, &encode(self)?)
    }
}
fn payload(bytes: &[u8]) -> Payload {
    serde_json::from_slice(&bytes[6..]).unwrap()
}
struct Admission;
#[async_trait::async_trait]
impl CommandAdmissionPolicy for Admission {
    const OPERATION_KIND: &'static str = Payload::OPERATION_KIND;
    const PAYLOAD_VERSION: u32 = 1;
    fn effect(&self, command: &CommandSubmission) -> Result<Option<ItemIntentPatch>> {
        Ok(Some(ItemIntentPatch {
            title: Some(payload(command.payload_bytes()).desired),
            ..Default::default()
        }))
    }
    async fn validate(
        &self,
        _: &mut Transaction<'_, Sqlite>,
        _: &RemoteAccount,
        _: &CommandSubmission,
    ) -> Result<Vec<CommandProtection>> {
        Ok(vec![])
    }
}
struct Delivery;
#[async_trait::async_trait]
impl crate::delivery::CommandDeliveryPolicy for Delivery {
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
    ) -> Result<crate::delivery::ClaimDecision> {
        unreachable!()
    }
    fn validate_evidence(
        &self,
        _: &DeliveryCommand,
        _: crate::delivery::EvidencePurpose,
        _: &crate::delivery::OperationEvidence,
    ) -> bool {
        false
    }
    async fn dispatch(
        &self,
        _: &crate::credentials::SecretToken,
        _: crate::delivery::DispatchRequest,
    ) -> crate::delivery::DeliveryReport {
        unreachable!()
    }
}
#[derive(Default)]
struct Policy {
    reject_after_admission: bool,
    wrong_target: bool,
    dependency: Option<String>,
}
#[async_trait::async_trait]
impl CommandRecoveryPolicy for Policy {
    fn instance_id(&self) -> &str {
        "github:https://github.com/"
    }
    fn operation_kind(&self) -> &'static str {
        Payload::OPERATION_KIND
    }
    fn payload_version(&self) -> u32 {
        1
    }
    async fn review_in(
        &self,
        tx: &mut Transaction<'_, Sqlite>,
        command: &DeliveryCommand,
        _: &RemoteAccount,
    ) -> Result<NativeRecoveryReview> {
        let original = payload(&command.payload);
        let json: Option<String> =
            sqlx::query_scalar("SELECT json FROM items WHERE account_id=? AND id=?")
                .bind(&command.account_id)
                .bind(&command.target_id)
                .fetch_optional(&mut **tx)
                .await
                .map_err(storage_error)?;
        let remote: Option<RemoteItem> = json.map(|j| decode(&j)).transpose()?;
        let base = value(&original.base);
        let desired = value(&original.desired);
        let remote = remote
            .map(|r| value(&r.title))
            .unwrap_or(CommandFieldValue {
                known: false,
                value: None,
            });
        let comparison = compare_field(CommandReviewField::Title, &base, &remote, &desired);
        Ok(NativeRecoveryReview {
            fields: vec![CommandFieldReview {
                field: CommandReviewField::Title,
                base,
                remote,
                desired,
                comparison,
                editable: true,
            }],
            can_replace: true,
            reason: None,
            fence: vec![],
        })
    }
    async fn replace_in(
        &self,
        tx: &mut Transaction<'_, Sqlite>,
        command: &DeliveryCommand,
        account: &RemoteAccount,
        request: &CommandRecoveryReplaceRequest,
        review: &NativeRecoveryReview,
    ) -> Result<CommandReceipt> {
        let field = &review.fields[0];
        let desired = match request.fields.first() {
            Some(choice) => match choice.choice {
                CommandResolutionChoice::Edited => choice.value.clone(),
                CommandResolutionChoice::KeepDesired => field.desired.value.clone(),
                CommandResolutionChoice::UseRemote => field.remote.value.clone(),
            },
            None if field.comparison == CommandFieldComparison::Unchanged => {
                field.remote.value.clone()
            }
            None => field.desired.value.clone(),
        }
        .ok_or_else(invalid)?;
        let command = seal_command(CommandDraft {
            command_id: request.new_command_id.clone(),
            account_id: account.id.clone(),
            authorization_epoch: account.authorization_epoch.clone(),
            target: CommandTarget::new(
                CommandTargetKind::PullRequest,
                if self.wrong_target {
                    "other"
                } else {
                    &command.target_id
                },
                Some("repo".into()),
            )?,
            payload: Payload {
                base: field.remote.value.clone().unwrap(),
                desired,
            },
            guards: vec![],
            dependencies: self.dependency.iter().cloned().collect(),
        })?;
        let receipt = command_admission::admit_in(tx, &command, &Admission)
            .await
            .map_err(|_| invalid())?;
        if self.reject_after_admission {
            return Err(invalid());
        }
        Ok(receipt)
    }
}
fn value(text: &str) -> CommandFieldValue {
    CommandFieldValue {
        known: true,
        value: Some(text.into()),
    }
}
fn uuid() -> String {
    Uuid::new_v4().to_string()
}
async fn setup() -> (tempfile::TempDir, Store) {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path().join("recovery.db")).await.unwrap();
    fixtures::seed(&store, "a").await;
    fixtures::seed(&store, "b").await;
    (dir, store)
}
async fn admit(
    store: &Store,
    account: &str,
    desired: &str,
    dependencies: Vec<String>,
) -> CommandSubmission {
    let command = seal_command(CommandDraft {
        command_id: uuid(),
        account_id: account.into(),
        authorization_epoch: "1".into(),
        target: CommandTarget::new(CommandTargetKind::PullRequest, "pull", Some("repo".into()))
            .unwrap(),
        payload: Payload {
            base: "Summary".into(),
            desired: desired.into(),
        },
        guards: vec![],
        dependencies,
    })
    .unwrap();
    store.admit_command(&command, &Admission).await.unwrap();
    command
}
async fn detail(store: &Store, command: &CommandSubmission) -> CommandRecoveryDetail {
    store
        .command_recovery_detail(
            command.account_id(),
            command.command_id(),
            Some(&Policy::default()),
        )
        .await
        .unwrap()
}
fn replacement(context: CommandRecoveryContext) -> CommandRecoveryReplaceRequest {
    CommandRecoveryReplaceRequest {
        context,
        action_id: uuid(),
        new_command_id: uuid(),
        fields: vec![],
    }
}
async fn count(store: &Store, table: &str) -> i64 {
    sqlx::query_scalar(sqlx::AssertSqlSafe(format!("SELECT count(*) FROM {table}")))
        .fetch_one(&store.inner.readers)
        .await
        .unwrap()
}
async fn remote_title(store: &Store, title: &str) {
    let mut writer = store.inner.writer.acquire().await.unwrap();
    sqlx::query(
        "UPDATE items SET json=json_set(json,'$.title',?) WHERE account_id='a' AND id='pull'",
    )
    .bind(title)
    .execute(&mut *writer)
    .await
    .unwrap();
}
async fn attempted(store: &Store, command: &CommandSubmission) {
    let mut writer = store.inner.writer.acquire().await.unwrap();
    let mut tx = writer.begin().await.unwrap();
    sqlx::query("INSERT INTO delivery_attempts(account_id,command_id,attempt_number,authorization_epoch,started_at,outcome,completed_at) VALUES('a',?,1,1,?,'outcome_unknown',?)").bind(command.command_id()).bind(now()).bind(now()).execute(&mut *tx).await.unwrap();
    sqlx::query(
        "INSERT INTO delivery_attempt_context VALUES('a',?,1,'github:https://github.com/',X'')",
    )
    .bind(command.command_id())
    .execute(&mut *tx)
    .await
    .unwrap();
    let loaded = super::super::delivery::load_in(&mut tx, "a", command.command_id())
        .await
        .unwrap();
    super::super::delivery::transition_in(
        &mut tx,
        &loaded,
        DeliveryState::Unknown,
        Some("2099-01-01T00:00:00Z"),
        None,
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();
}
fn now() -> String {
    chrono::Utc::now().to_rfc3339()
}

#[tokio::test]
async fn cancel_is_atomic_idempotent_account_scoped_and_survives_reopen() {
    let (dir, store) = setup().await;
    let command = admit(&store, "a", "Authored 雪 text", vec![]).await;
    let reviewed = detail(&store, &command).await;
    assert!(reviewed.can_cancel && reviewed.can_replace);
    let request = CommandRecoveryActionRequest {
        context: reviewed.context,
        action_id: uuid(),
        action: CommandRecoveryAction::Cancel,
    };
    let receipt = store
        .command_recovery_action(request.clone(), None)
        .await
        .unwrap_err();
    assert_eq!(receipt.code, ErrorCode::StaleView); // Installed policy is part of the review fence.
    let receipt = store
        .command_recovery_action(request.clone(), Some(&Policy::default()))
        .await
        .unwrap();
    assert!(!receipt.remote_may_have_happened);
    assert_eq!(receipt.state, "cancelled");
    assert_eq!(
        store.item("a", "pull").await.unwrap().item.unwrap().title,
        "Summary"
    );
    assert_eq!(count(&store, "delivery_attempts").await, 0);
    assert_eq!(
        store
            .command_recovery_action(request.clone(), Some(&Policy::default()))
            .await
            .unwrap(),
        receipt
    );
    let mut reused = request.clone();
    reused.action = CommandRecoveryAction::Pause;
    assert!(
        store
            .command_recovery_action(reused, Some(&Policy::default()))
            .await
            .is_err()
    );
    let mut forged = request.clone();
    forged.context.account_id = "b".into();
    forged.action_id = uuid();
    assert!(
        store
            .command_recovery_action(forged, Some(&Policy::default()))
            .await
            .is_err()
    );
    store.close().await.unwrap();
    let reopened = Store::open(dir.path().join("recovery.db")).await.unwrap();
    assert_eq!(
        reopened
            .command_recovery_action(request, Some(&Policy::default()))
            .await
            .unwrap(),
        receipt
    );
    assert_eq!(
        reopened
            .command_receipt("a", command.command_id())
            .await
            .unwrap()
            .unwrap()
            .submission_hash,
        *command.submission_hash()
    );
}

#[tokio::test]
async fn overlapping_text_requires_choice_and_remote_refresh_invalidates_review() {
    let (_dir, store) = setup().await;
    let command = admit(&store, "a", "mine", vec![]).await;
    let old = detail(&store, &command).await;
    remote_title(&store, "theirs").await;
    assert_eq!(
        store
            .command_recovery_replace(replacement(old.context), Some(&Policy::default()))
            .await
            .unwrap_err()
            .code,
        ErrorCode::StaleView
    );
    let current = detail(&store, &command).await;
    assert_eq!(
        current.fields[0].comparison,
        CommandFieldComparison::Conflict
    );
    let mut request = replacement(current.context);
    assert!(
        store
            .command_recovery_replace(request.clone(), Some(&Policy::default()))
            .await
            .is_err()
    );
    request.fields.push(CommandFieldResolution {
        field: CommandReviewField::Title,
        choice: CommandResolutionChoice::Edited,
        value: Some("Resolved 雪 prose".into()),
    });
    let receipt = store
        .command_recovery_replace(request.clone(), Some(&Policy::default()))
        .await
        .unwrap();
    assert_eq!(
        store.item("a", "pull").await.unwrap().item.unwrap().title,
        "Resolved 雪 prose"
    );
    assert_eq!(
        store
            .command_recovery_replace(request, Some(&Policy::default()))
            .await
            .unwrap(),
        receipt
    );
    assert_eq!(
        store
            .delivery_command("a", command.command_id())
            .await
            .unwrap()
            .payload,
        command.payload_bytes()
    );
    assert_eq!(count(&store, "command_supersessions").await, 1);
    assert_eq!(count(&store, "delivery_attempts").await, 0);
}

#[tokio::test]
async fn supersession_preserves_successor_dependency_without_fifo_deadlock() {
    let (_dir, store) = setup().await;
    let first = admit(&store, "a", "A", vec![]).await;
    let successor = admit(&store, "a", "B", vec![first.command_id().into()]).await;
    let request = replacement(detail(&store, &first).await.context);
    let receipt = store
        .command_recovery_replace(request.clone(), Some(&Policy::default()))
        .await
        .unwrap();
    assert_eq!(
        store.item("a", "pull").await.unwrap().item.unwrap().title,
        "B"
    );
    let replacement = store
        .delivery_command("a", receipt.replacement_id.as_ref().unwrap())
        .await
        .unwrap();
    let next = store
        .delivery_command("a", successor.command_id())
        .await
        .unwrap();
    let account = store.account("a").await.unwrap();
    let clock = DeliveryTime {
        now: now(),
        command_now: now(),
    };
    assert!(
        store
            .claim_preparation(&next, &account, &Delivery, &clock)
            .await
            .unwrap()
            .0
            .is_none()
    );
    assert!(
        store
            .claim_preparation(&replacement, &account, &Delivery, &clock)
            .await
            .unwrap()
            .0
            .is_some()
    );
    let blocked = detail(&store, &successor).await;
    assert!(
        blocked
            .command
            .blocked_reason
            .unwrap()
            .contains("predecessor")
    );
    let original_dependency: String = sqlx::query_scalar(
        "SELECT predecessor_id FROM command_dependencies WHERE account_id='a' AND command_id=?",
    )
    .bind(successor.command_id())
    .fetch_one(&store.inner.readers)
    .await
    .unwrap();
    assert_eq!(original_dependency, first.command_id());
    assert_eq!(count(&store, "delivery_resolutions").await, 0);
}

#[tokio::test]
async fn failed_policy_or_receipt_rolls_back_new_intent_effects_and_supersession() {
    let (_dir, store) = setup().await;
    let command = admit(&store, "a", "mine", vec![]).await;
    let request = replacement(detail(&store, &command).await.context);
    let before = store.revision().await.unwrap();
    for policy in [
        Policy {
            reject_after_admission: true,
            ..Default::default()
        },
        Policy {
            wrong_target: true,
            ..Default::default()
        },
    ] {
        assert!(
            store
                .command_recovery_replace(request.clone(), Some(&policy))
                .await
                .is_err()
        );
        assert_eq!(count(&store, "commands").await, 1);
        assert_eq!(count(&store, "command_effects").await, 1);
        assert_eq!(count(&store, "command_supersessions").await, 0);
        assert_eq!(store.revision().await.unwrap(), before);
    }
    {
        let mut writer = store.inner.writer.acquire().await.unwrap();
        sqlx::raw_sql("CREATE TRIGGER fixture_receipt_failure BEFORE INSERT ON command_recovery_actions BEGIN SELECT RAISE(ABORT,'fixture receipt failure'); END;").execute(&mut *writer).await.unwrap();
    }
    assert!(
        store
            .command_recovery_replace(request, Some(&Policy::default()))
            .await
            .is_err()
    );
    assert_eq!(count(&store, "commands").await, 1);
    assert_eq!(count(&store, "command_supersessions").await, 0);
    assert_eq!(store.revision().await.unwrap(), before);
}

#[tokio::test]
async fn attempted_intent_can_pause_but_never_cancel_or_lose_budget_on_resume() {
    let (_dir, store) = setup().await;
    let command = admit(&store, "a", "mine", vec![]).await;
    attempted(&store, &command).await;
    let reviewed = detail(&store, &command).await;
    assert!(!reviewed.can_cancel && !reviewed.can_replace && reviewed.can_pause);
    let cancel = CommandRecoveryActionRequest {
        context: reviewed.context.clone(),
        action_id: uuid(),
        action: CommandRecoveryAction::Cancel,
    };
    assert_eq!(
        store
            .command_recovery_action(cancel, Some(&Policy::default()))
            .await
            .unwrap_err()
            .code,
        ErrorCode::Unsupported
    );
    let pause = CommandRecoveryActionRequest {
        context: reviewed.context,
        action_id: uuid(),
        action: CommandRecoveryAction::Pause,
    };
    let receipt = store
        .command_recovery_action(pause, Some(&Policy::default()))
        .await
        .unwrap();
    assert!(receipt.remote_may_have_happened && receipt.paused);
    assert!(
        store
            .delivery_candidates("a", None, 32)
            .await
            .unwrap()
            .is_empty()
    );
    let reviewed = detail(&store, &command).await;
    assert!(reviewed.can_retry);
    let resume = CommandRecoveryActionRequest {
        context: reviewed.context,
        action_id: uuid(),
        action: CommandRecoveryAction::Resume,
    };
    store
        .command_recovery_action(resume, Some(&Policy::default()))
        .await
        .unwrap();
    let saved = store
        .delivery_command("a", command.command_id())
        .await
        .unwrap();
    assert_eq!(saved.state, DeliveryState::Unknown);
    assert_eq!(saved.attempt_count, 1);
    assert_eq!(
        saved.next_action_at.as_deref(),
        Some("2099-01-01T00:00:00Z")
    );
    assert_eq!(
        store
            .delivery_candidates("a", None, 32)
            .await
            .unwrap()
            .len(),
        1
    );
}

#[tokio::test]
async fn conflict_and_attention_halts_never_advertise_or_record_a_noop_pause() {
    let (_dir, store) = setup().await;
    let conflict = admit(&store, "a", "conflict", vec![]).await;
    attempted(&store, &conflict).await;
    {
        let mut writer = store.inner.writer.acquire().await.unwrap();
        let mut tx = writer.begin().await.unwrap();
        let current = super::super::delivery::load_in(&mut tx, "a", conflict.command_id())
            .await
            .unwrap();
        super::super::delivery::transition_in(
            &mut tx,
            &current,
            DeliveryState::Conflict,
            None,
            None,
        )
        .await
        .unwrap();
        tx.commit().await.unwrap();
    }
    let conflict_review = detail(&store, &conflict).await;
    assert!(!conflict_review.can_pause);
    let conflict_pause = CommandRecoveryActionRequest {
        context: conflict_review.context,
        action_id: uuid(),
        action: CommandRecoveryAction::Pause,
    };
    assert_eq!(
        store
            .command_recovery_action(conflict_pause, Some(&Policy::default()))
            .await
            .unwrap_err()
            .code,
        ErrorCode::Unsupported
    );

    let attention = admit(&store, "a", "attention", vec![]).await;
    attempted(&store, &attention).await;
    {
        let mut writer = store.inner.writer.acquire().await.unwrap();
        let mut tx = writer.begin().await.unwrap();
        let current = super::super::delivery::load_in(&mut tx, "a", attention.command_id())
            .await
            .unwrap();
        super::super::delivery::transition_in(
            &mut tx,
            &current,
            current.state,
            current.next_action_at.as_deref(),
            Some("reconciliation_limit"),
        )
        .await
        .unwrap();
        tx.commit().await.unwrap();
    }
    let attention_review = detail(&store, &attention).await;
    assert!(!attention_review.can_pause);
    let attention_pause = CommandRecoveryActionRequest {
        context: attention_review.context,
        action_id: uuid(),
        action: CommandRecoveryAction::Pause,
    };
    assert_eq!(
        store
            .command_recovery_action(attention_pause, Some(&Policy::default()))
            .await
            .unwrap_err()
            .code,
        ErrorCode::Unsupported
    );
    assert_eq!(count(&store, "command_recovery_actions").await, 0);
}

#[tokio::test]
async fn unknown_codec_exports_only_authored_bytes_and_refuses_resume_or_replacement() {
    let (_dir, store) = setup().await;
    let command = admit(&store, "a", "mine", vec![]).await;
    attempted(&store, &command).await;
    let reviewed = store
        .command_recovery_detail("a", command.command_id(), None)
        .await
        .unwrap();
    assert!(
        !reviewed.can_retry && !reviewed.can_cancel && !reviewed.can_pause && !reviewed.can_replace
    );
    let exported = store
        .command_recovery_export(reviewed.context, None)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_str(&exported.text).unwrap();
    assert_eq!(json["authored_payload_hex"], hex(command.payload_bytes()));
    for secret in [
        "credential_ref",
        "synthetic_secret",
        "command_evidence",
        "headers",
        "canonical_envelope",
    ] {
        assert!(!exported.text.contains(secret));
    }
}

#[tokio::test]
async fn list_is_bounded_account_scoped_and_cursor_fenced_by_mutations() {
    let (_dir, store) = setup().await;
    let first = admit(&store, "a", "one", vec![]).await;
    admit(&store, "a", "two", vec![]).await;
    admit(&store, "b", "private", vec![]).await;
    let query = CommandRecoveryQuery {
        account_id: "a".into(),
        target_id: Some("pull".into()),
        include_terminal: true,
        cursor: None,
        limit: 1,
    };
    let page = store.command_recovery_list(query.clone()).await.unwrap();
    assert_eq!(page.commands.len(), 1);
    let mut next = query.clone();
    next.cursor = page.next_cursor;
    assert!(next.cursor.is_some());
    assert_eq!(
        store
            .command_recovery_list(next.clone())
            .await
            .unwrap()
            .commands[0]
            .command_id,
        first.command_id()
    );
    let action = CommandRecoveryActionRequest {
        context: detail(&store, &first).await.context,
        action_id: uuid(),
        action: CommandRecoveryAction::Cancel,
    };
    store
        .command_recovery_action(action, Some(&Policy::default()))
        .await
        .unwrap();
    assert_eq!(
        store.command_recovery_list(next).await.unwrap_err().code,
        ErrorCode::StaleView
    );
    let mut invalid = query;
    invalid.limit = 51;
    assert!(store.command_recovery_list(invalid).await.is_err());
}

#[tokio::test]
async fn backup_restore_preserves_receipts_edges_and_pauses_without_authorizing_dispatch() {
    use crate::recovery::{RecoverySession, RestoreChoice};
    let (dir, store) = setup().await;
    let first = admit(&store, "a", "one", vec![]).await;
    let request = replacement(detail(&store, &first).await.context);
    let receipt = store
        .command_recovery_replace(request, Some(&Policy::default()))
        .await
        .unwrap();
    let other = admit(&store, "a", "two", vec![]).await;
    attempted(&store, &other).await;
    let pause = CommandRecoveryActionRequest {
        context: detail(&store, &other).await.context,
        action_id: uuid(),
        action: CommandRecoveryAction::Pause,
    };
    store
        .command_recovery_action(pause, Some(&Policy::default()))
        .await
        .unwrap();
    let backup = dir.path().join("backup.db");
    store.backup_to(&backup).await.unwrap();
    store.close().await.unwrap();
    let session = RecoverySession::prepare(&dir.path().join("recovery.db"), &backup)
        .await
        .unwrap();
    let confirmation = session.preview().confirmation_id.clone();
    session
        .confirm(&confirmation, RestoreChoice::ReplaceCurrentData)
        .unwrap();
    let reopened = Store::open(dir.path().join("recovery.db")).await.unwrap();
    assert_eq!(count(&reopened, "command_supersessions").await, 1);
    assert_eq!(count(&reopened, "command_recovery_actions").await, 2);
    let replaced = reopened
        .command_recovery_detail(
            "a",
            receipt.replacement_id.as_ref().unwrap(),
            Some(&Policy::default()),
        )
        .await
        .unwrap();
    assert!(replaced.command.quarantined);
    assert!(!replaced.can_cancel && !replaced.can_replace && !replaced.can_retry);
    let paused = reopened
        .command_recovery_detail("a", other.command_id(), Some(&Policy::default()))
        .await
        .unwrap();
    assert!(paused.command.paused && paused.command.quarantined);
    let raw = sqlx::query("SELECT receipt_json FROM command_recovery_actions WHERE action_id=?")
        .bind(&receipt.action_id)
        .fetch_one(&reopened.inner.readers)
        .await
        .unwrap();
    assert_eq!(
        decode::<CommandRecoveryReceipt>(&raw.get::<String, _>(0)).unwrap(),
        receipt
    );
}

#[tokio::test]
async fn independent_remote_body_survives_title_replacement_and_envelope_is_immutable() {
    let (_dir, store) = setup().await;
    let command = admit(&store, "a", "my title", vec![]).await;
    {
        let mut writer = store.inner.writer.acquire().await.unwrap();
        sqlx::query("UPDATE items SET json=json_set(json,'$.body','remote body edit') WHERE account_id='a' AND id='pull'").execute(&mut *writer).await.unwrap();
    }
    let reviewed = detail(&store, &command).await;
    assert_eq!(
        reviewed.fields[0].comparison,
        CommandFieldComparison::Independent
    );
    store
        .command_recovery_replace(replacement(reviewed.context), Some(&Policy::default()))
        .await
        .unwrap();
    let result = store.item("a", "pull").await.unwrap().item.unwrap();
    assert_eq!(result.title, "my title");
    assert_eq!(result.body.as_deref(), Some("remote body edit"));
    assert_eq!(
        store
            .delivery_command("a", command.command_id())
            .await
            .unwrap()
            .canonical_envelope,
        command.canonical_envelope()
    );
}

#[tokio::test]
async fn concurrent_exact_actions_commit_once_and_epoch_change_rejects_saved_context() {
    let (_dir, store) = setup().await;
    let command = admit(&store, "a", "mine", vec![]).await;
    let context = detail(&store, &command).await.context;
    let request = CommandRecoveryActionRequest {
        context,
        action_id: uuid(),
        action: CommandRecoveryAction::Cancel,
    };
    let policy = Policy::default();
    let (left, right) = tokio::join!(
        store.command_recovery_action(request.clone(), Some(&policy)),
        store.command_recovery_action(request.clone(), Some(&policy))
    );
    assert_eq!(left.unwrap(), right.unwrap());
    assert_eq!(count(&store, "command_recovery_actions").await, 1);
    let next = admit(&store, "a", "second", vec![]).await;
    let saved = detail(&store, &next).await.context;
    let mut account = store.account("a").await.unwrap();
    account.authorization_epoch = "2".into();
    store.upsert_account(account).await.unwrap();
    for request in [
        request,
        CommandRecoveryActionRequest {
            context: saved,
            action_id: uuid(),
            action: CommandRecoveryAction::Cancel,
        },
    ] {
        assert_eq!(
            store
                .command_recovery_action(request, Some(&policy))
                .await
                .unwrap_err()
                .code,
            ErrorCode::StaleView
        );
    }
}

#[tokio::test]
async fn schema_protects_immutable_resolution_identity_and_deferred_receipt_atomicity() {
    let (_dir, store) = setup().await;
    let command = admit(&store, "a", "mine", vec![]).await;
    store
        .command_recovery_replace(
            replacement(detail(&store, &command).await.context),
            Some(&Policy::default()),
        )
        .await
        .unwrap();
    let mut writer = store.inner.writer.acquire().await.unwrap();
    for statement in [
        "UPDATE command_supersessions SET execution_order=99",
        "DELETE FROM command_supersessions",
        "UPDATE command_recovery_actions SET request_json='{}'",
        "DELETE FROM command_recovery_actions",
    ] {
        assert!(sqlx::query(statement).execute(&mut *writer).await.is_err());
    }
    let checked: Option<String> = sqlx::query_scalar("PRAGMA foreign_key_check")
        .fetch_optional(&mut *writer)
        .await
        .unwrap();
    assert!(checked.is_none());
}

#[tokio::test]
async fn pause_limits_and_unknown_context_remain_fail_closed() {
    let (_dir, store) = setup().await;
    let command = admit(&store, "a", "mine", vec![]).await;
    attempted(&store, &command).await;
    let policy = Policy::default();
    for index in 0..64 {
        let context = detail(&store, &command).await.context;
        store
            .command_recovery_action(
                CommandRecoveryActionRequest {
                    context,
                    action_id: uuid(),
                    action: if index % 2 == 0 {
                        CommandRecoveryAction::Pause
                    } else {
                        CommandRecoveryAction::Resume
                    },
                },
                Some(&policy),
            )
            .await
            .unwrap();
    }
    let before = store.revision().await.unwrap();
    let reviewed = detail(&store, &command).await;
    assert_eq!(
        store
            .command_recovery_action(
                CommandRecoveryActionRequest {
                    context: reviewed.context,
                    action_id: uuid(),
                    action: CommandRecoveryAction::Pause
                },
                Some(&policy)
            )
            .await
            .unwrap_err()
            .code,
        ErrorCode::Busy
    );
    assert_eq!(store.revision().await.unwrap(), before);
    assert!(!detail(&store, &command).await.command.paused);
    assert_eq!(count(&store, "command_recovery_actions").await, 64);
    assert_eq!(
        store
            .delivery_command("a", command.command_id())
            .await
            .unwrap()
            .attempt_count,
        1
    );
}

#[tokio::test]
async fn recovery_history_queries_use_account_target_keyset_indexes() {
    let (_dir, store) = setup().await;
    for (query, index) in [
        (
            "EXPLAIN QUERY PLAN SELECT command_id FROM commands WHERE account_id='a' AND enqueue_order<100 AND state IN ('queued','sending','retry_wait','accepted','outcome_unknown','conflict') ORDER BY enqueue_order DESC LIMIT 51",
            "command_recovery_pending",
        ),
        (
            "EXPLAIN QUERY PLAN SELECT command_id FROM commands WHERE account_id='a' AND target_id='pull' AND enqueue_order<100 ORDER BY enqueue_order DESC LIMIT 51",
            "command_recovery_target_history",
        ),
        (
            "EXPLAIN QUERY PLAN SELECT command_id FROM commands WHERE account_id='a' AND target_id='pull' AND enqueue_order<100 AND state IN ('queued','sending','retry_wait','accepted','outcome_unknown','conflict') ORDER BY enqueue_order DESC LIMIT 51",
            "command_recovery_target_pending",
        ),
    ] {
        let rows = sqlx::query(query)
            .fetch_all(&store.inner.readers)
            .await
            .unwrap();
        let plan = rows
            .iter()
            .map(|r| r.get::<String, _>("detail"))
            .collect::<Vec<_>>()
            .join("\n");
        assert!(plan.contains(index), "{plan}");
        assert!(!plan.contains("TEMP B-TREE"), "{plan}");
    }
}

#[tokio::test]
async fn exact_effect_cap_replaces_attempted_conflict_and_failure_restores_original_slot() {
    let (_dir, store) = setup().await;
    let oldest = admit(&store, "a", "oldest", vec![]).await;
    attempted(&store, &oldest).await;
    {
        let mut writer = store.inner.writer.acquire().await.unwrap();
        let mut tx = writer.begin().await.unwrap();
        let current = super::super::delivery::load_in(&mut tx, "a", oldest.command_id())
            .await
            .unwrap();
        super::super::delivery::transition_in(
            &mut tx,
            &current,
            DeliveryState::Conflict,
            None,
            None,
        )
        .await
        .unwrap();
        tx.commit().await.unwrap();
    }
    for index in 1..64 {
        admit(
            &store,
            "a",
            &format!("successor-{index}"),
            vec![oldest.command_id().into()],
        )
        .await;
    }
    let reviewed = detail(&store, &oldest).await;
    assert!(!reviewed.can_cancel && reviewed.can_replace);
    let request = replacement(reviewed.context);
    let before = store.item("a", "pull").await.unwrap();
    let command_before = store
        .delivery_command("a", oldest.command_id())
        .await
        .unwrap();
    let revision_before = store.revision().await.unwrap();
    let fault = Policy {
        reject_after_admission: true,
        ..Default::default()
    };
    assert!(
        store
            .command_recovery_replace(request.clone(), Some(&fault))
            .await
            .is_err()
    );
    assert_eq!(store.revision().await.unwrap(), revision_before);
    assert_eq!(store.item("a", "pull").await.unwrap(), before);
    let preserved = store
        .delivery_command("a", oldest.command_id())
        .await
        .unwrap();
    assert_eq!(preserved.state, DeliveryState::Conflict);
    assert_eq!(preserved.generation, command_before.generation);
    assert_eq!(preserved.hash, command_before.hash);
    assert_eq!(count(&store, "commands").await, 64);
    assert_eq!(count(&store, "command_supersessions").await, 0);
    let receipt = store
        .command_recovery_replace(request, Some(&Policy::default()))
        .await
        .unwrap();
    let after = store.item("a", "pull").await.unwrap();
    assert_eq!(after.item.unwrap().title, "successor-63");
    assert_eq!(after.pending_intent.unwrap().commands.len(), 64);
    assert_eq!(count(&store, "commands").await, 65);
    assert_eq!(count(&store, "delivery_attempts").await, 1);
    let replacement = store
        .delivery_command("a", receipt.replacement_id.as_ref().unwrap())
        .await
        .unwrap();
    let clock = DeliveryTime {
        now: now(),
        command_now: now(),
    };
    assert!(
        store
            .claim_preparation(
                &replacement,
                &store.account("a").await.unwrap(),
                &Delivery,
                &clock
            )
            .await
            .unwrap()
            .0
            .is_some()
    );
}

#[tokio::test]
async fn replacement_cannot_depend_on_a_later_execution_slot_but_successor_can_review_new_predecessor()
 {
    let (_dir, store) = setup().await;
    let first = admit(&store, "a", "A", vec![]).await;
    let second = admit(&store, "a", "B", vec![first.command_id().into()]).await;
    let bad = Policy {
        dependency: Some(second.command_id().into()),
        ..Default::default()
    };
    assert!(
        store
            .command_recovery_replace(
                replacement(detail(&store, &first).await.context),
                Some(&bad)
            )
            .await
            .is_err()
    );
    assert_eq!(
        store
            .delivery_command("a", first.command_id())
            .await
            .unwrap()
            .state,
        DeliveryState::Queued
    );
    assert_eq!(count(&store, "commands").await, 2);
    let first_receipt = store
        .command_recovery_replace(
            replacement(detail(&store, &first).await.context),
            Some(&Policy::default()),
        )
        .await
        .unwrap();
    let predecessor = first_receipt.replacement_id.unwrap();
    let good = Policy {
        dependency: Some(predecessor.clone()),
        ..Default::default()
    };
    let second_receipt = store
        .command_recovery_replace(
            replacement(detail(&store, &second).await.context),
            Some(&good),
        )
        .await
        .unwrap();
    let dependency: String = sqlx::query_scalar(
        "SELECT predecessor_id FROM command_dependencies WHERE account_id='a' AND command_id=?",
    )
    .bind(second_receipt.replacement_id.unwrap())
    .fetch_one(&store.inner.readers)
    .await
    .unwrap();
    assert_eq!(dependency, predecessor);
    assert_eq!(
        store
            .delivery_command("a", second.command_id())
            .await
            .unwrap()
            .canonical_envelope,
        second.canonical_envelope()
    );
}

#[tokio::test]
async fn restore_rejects_semantically_forged_action_with_an_exact_known_schema() {
    use crate::recovery::RecoverySession;
    let (dir, store) = setup().await;
    let command = admit(&store, "a", "retained", vec![]).await;
    let action = CommandRecoveryActionRequest {
        context: detail(&store, &command).await.context,
        action_id: uuid(),
        action: CommandRecoveryAction::Cancel,
    };
    store
        .command_recovery_action(action, Some(&Policy::default()))
        .await
        .unwrap();
    let backup = dir.path().join("forged.db");
    store.backup_to(&backup).await.unwrap();
    store.close().await.unwrap();
    let original = std::fs::read(dir.path().join("recovery.db")).unwrap();
    let mut db = sqlx::SqliteConnection::connect_with(
        &sqlx::sqlite::SqliteConnectOptions::new()
            .filename(&backup)
            .foreign_keys(true),
    )
    .await
    .unwrap();
    let trigger: String = sqlx::query_scalar(
        "SELECT sql FROM sqlite_schema WHERE name='command_recovery_action_immutable'",
    )
    .fetch_one(&mut db)
    .await
    .unwrap();
    sqlx::raw_sql("DROP TRIGGER command_recovery_action_immutable; UPDATE command_recovery_actions SET receipt_json=json_set(receipt_json,'$.state','confirmed');").execute(&mut db).await.unwrap();
    sqlx::raw_sql(sqlx::AssertSqlSafe(trigger))
        .execute(&mut db)
        .await
        .unwrap();
    db.close().await.unwrap();
    let incoming = std::fs::read(&backup).unwrap();
    assert!(
        RecoverySession::prepare(&dir.path().join("recovery.db"), &backup)
            .await
            .is_err()
    );
    assert_eq!(std::fs::read(&backup).unwrap(), incoming);
    assert_eq!(
        std::fs::read(dir.path().join("recovery.db")).unwrap(),
        original
    );
}

#[tokio::test]
async fn exhausted_mutation_budget_does_not_prevent_resuming_read_only_reconciliation() {
    let (_dir, store) = setup().await;
    let command = admit(&store, "a", "mine", vec![]).await;
    attempted(&store, &command).await;
    {
        let mut writer = store.inner.writer.acquire().await.unwrap();
        for number in 2..=8 {
            sqlx::query("INSERT INTO delivery_attempts VALUES('a',?,?,1,?,'outcome_unknown',?)")
                .bind(command.command_id())
                .bind(number)
                .bind(now())
                .bind(now())
                .execute(&mut *writer)
                .await
                .unwrap();
            sqlx::query("INSERT INTO delivery_attempt_context VALUES('a',?,?,'github:https://github.com/',X'')").bind(command.command_id()).bind(number).execute(&mut *writer).await.unwrap();
        }
    }
    let pause = CommandRecoveryActionRequest {
        context: detail(&store, &command).await.context,
        action_id: uuid(),
        action: CommandRecoveryAction::Pause,
    };
    assert!(detail(&store, &command).await.can_pause);
    store
        .command_recovery_action(pause, Some(&Policy::default()))
        .await
        .unwrap();
    let reviewed = detail(&store, &command).await;
    assert!(reviewed.can_retry && !reviewed.can_replace);
    store
        .command_recovery_action(
            CommandRecoveryActionRequest {
                context: reviewed.context,
                action_id: uuid(),
                action: CommandRecoveryAction::Resume,
            },
            Some(&Policy::default()),
        )
        .await
        .unwrap();
    let resumed = store
        .delivery_command("a", command.command_id())
        .await
        .unwrap();
    assert_eq!(resumed.attempt_count, 8);
    assert!(resumed.reconcile_only());
}
