//! Synthetic v2 history. Never invokes a provider, credential vault or real data.
use super::*;
use crate::{
    IssueDraftContext, Store, SubmitIssueV2Request,
    commands::{CommandDraft, CommandSubmission, CommandTarget, CommandTargetKind, seal_command},
    issue_metadata::native as n,
    runtime::detail_tests::fixtures,
    storage::command_admission::{CommandAdmissionPolicy, CommandProtection},
};
const COMMAND: &str = "11111111-1111-4111-8111-111111111111";
const DRAFT: &str = "22222222-2222-4222-8222-222222222222";
const TIME: &str = "2026-10-08T00:00:00.000000000Z";
struct Admission;
#[async_trait::async_trait]
impl CommandAdmissionPolicy for Admission {
    const OPERATION_KIND: &'static str = "github.create_issue";
    const PAYLOAD_VERSION: u32 = 2;
    async fn validate(
        &self,
        _: &mut sqlx::Transaction<'_, Sqlite>,
        _: &RemoteAccount,
        _: &CommandSubmission,
    ) -> Result<Vec<CommandProtection>> {
        Ok(vec![])
    }
}
async fn fixture(path: &Path, state: &str) -> Store {
    let store = Store::open(path).await.unwrap();
    let mut a = fixtures::account("a");
    a.actor_id = "7".into();
    let a = store.upsert_account(a).await.unwrap();
    fixtures::project(&store, &a).await;
    let mut db = connect(path, false).await.unwrap();
    let view: i64 = sqlx::query_scalar("SELECT authorization_view FROM runtime_meta")
        .fetch_one(&mut db)
        .await
        .unwrap();
    let p = n::PayloadV2 {
        request: SubmitIssueV2Request {
            context: IssueDraftContext {
                account_id: "a".into(),
                repository_id: "repo".into(),
                authorization_epoch: "1".into(),
                authorization_view: view.to_string(),
                review_token: "a".repeat(64),
            },
            draft_id: DRAFT.into(),
            draft_generation: "1".into(),
            command_id: COMMAND.into(),
            accept_background_delivery: true,
            accept_metadata_best_effort: true,
        },
        title: "Preserved metadata issue".into(),
        body: "Authored body 雪".into(),
        repository_native: "1".into(),
        metadata: n::SelectionV2 {
            labels: vec![n::LabelV2 {
                id: "81".into(),
                name: "selected".into(),
                color: Some("123abc".into()),
            }],
            assignees: vec![n::AssigneeV2 {
                id: "7".into(),
                login: "author".into(),
            }],
            milestone: Some(n::MilestoneV2 {
                id: "91".into(),
                number: "2".into(),
                title: "selected milestone".into(),
            }),
        },
    };
    sqlx::query("INSERT INTO issue_drafts VALUES('a',?,'repo',?,?,1)")
        .bind(DRAFT)
        .bind(&p.title)
        .bind(&p.body)
        .execute(&mut db)
        .await
        .unwrap();
    sqlx::query("INSERT INTO issue_draft_metadata VALUES('a',?,?)")
        .bind(DRAFT)
        .bind(String::from_utf8(n::encode(&p.metadata).unwrap()).unwrap())
        .execute(&mut db)
        .await
        .unwrap();
    let c = seal_command(CommandDraft {
        command_id: COMMAND.into(),
        account_id: "a".into(),
        authorization_epoch: "1".into(),
        target: CommandTarget::new(CommandTargetKind::Repository, "repo", Some("repo".into()))
            .unwrap(),
        payload: p.clone(),
        guards: vec![],
        dependencies: vec![],
    })
    .unwrap();
    store.admit_command(&c, &Admission).await.unwrap();
    let hash: Vec<u8> = sqlx::query_scalar("SELECT submission_hash FROM commands")
        .fetch_one(&mut db)
        .await
        .unwrap();
    sqlx::query("INSERT INTO issue_submissions VALUES('a',?,1,?,?,?)")
        .bind(DRAFT)
        .bind(COMMAND)
        .bind(&hash)
        .bind(n::content_hash(&p.title, &p.body, &p.metadata).unwrap())
        .execute(&mut db)
        .await
        .unwrap();
    let prep = n::PreparationV2 {
        frame: n::FrameV2 {
            account_id: "a".into(),
            repository_id: "repo".into(),
            repository_native: "1".into(),
            repository_path: "owner/project".into(),
            authorization_view: view.to_string(),
        },
        actor: "7".into(),
        epoch: "1".into(),
        command_hash: hash.iter().map(|b| format!("{b:02x}")).collect(),
        revalidated_metadata: p.metadata.clone(),
        metadata_push_access: Some(true),
    };
    if matches!(state, "confirmed" | "outcome_unknown") {
        sqlx::query("INSERT INTO delivery_attempts VALUES('a',?,1,1,?,?,?)")
            .bind(COMMAND)
            .bind(TIME)
            .bind(state)
            .bind(TIME)
            .execute(&mut db)
            .await
            .unwrap();
        sqlx::query(
            "INSERT INTO delivery_attempt_context VALUES('a',?,1,'github:https://github.com/',?)",
        )
        .bind(COMMAND)
        .bind(n::encode(&prep).unwrap())
        .execute(&mut db)
        .await
        .unwrap();
    }
    if state == "confirmed" {
        let e = n::ReceiptV2 {
            preparation: prep,
            core: n::CreatedCoreV2 {
                provider_id: "9007199254740993".into(),
                number: "42".into(),
                title: p.title.clone(),
                body: Some(p.body.clone()),
                author_id: "7".into(),
                author_login: "author".into(),
                state: "open".into(),
                web_url: "https://github.com/owner/project/issues/42".into(),
                created_at: TIME.into(),
                updated_at: TIME.into(),
            },
            metadata: n::MetadataObservationV2 {
                labels: n::SetObservationV2::Known {
                    present_ids: vec!["81".into()],
                },
                assignees: n::SetObservationV2::Known {
                    present_ids: vec![],
                },
                milestone: n::MilestoneObservationV2::Unobserved {
                    reason: crate::IssueMetadataUnobservedReason::Missing,
                },
            },
        };
        assert!(n::receipt_matches(&e, &p));
        sqlx::query("INSERT INTO command_evidence VALUES('a',?,0,1,'github.issue_created',2,?,?)")
            .bind(COMMAND)
            .bind(n::encode(&e).unwrap())
            .bind(TIME)
            .execute(&mut db)
            .await
            .unwrap();
        sqlx::query("INSERT INTO delivery_resolutions VALUES('a',?,1,0,'confirmed')")
            .bind(COMMAND)
            .execute(&mut db)
            .await
            .unwrap();
        sqlx::query("INSERT INTO issue_resolutions VALUES('a',?,?,'github:issue:9007199254740993','9007199254740993','42','https://github.com/owner/project/issues/42')").bind(DRAFT).bind(COMMAND).execute(&mut db).await.unwrap();
    } else if matches!(state, "conflict" | "cancelled") {
        let e = n::DeclinedV2 {
            frame: prep.frame,
            actor: prep.actor,
            epoch: prep.epoch,
            command_hash: prep.command_hash,
            reason: n::DeclineReasonV2::SelectionChanged,
        };
        sqlx::query("INSERT INTO command_evidence VALUES('a',?,0,NULL,'github.issue_creation_declined',2,?,?)").bind(COMMAND).bind(n::encode(&e).unwrap()).bind(TIME).execute(&mut db).await.unwrap();
        sqlx::query("INSERT INTO delivery_resolutions VALUES('a',?,1,0,'conflict')")
            .bind(COMMAND)
            .execute(&mut db)
            .await
            .unwrap();
    }
    sqlx::query("UPDATE commands SET state=?")
        .bind(state)
        .execute(&mut db)
        .await
        .unwrap();
    db.close().await.unwrap();
    store
}
async fn history(path: &Path) -> Vec<Vec<String>> {
    let mut db = connect(path, true).await.unwrap();
    let mut result = vec![];
    for sql in [
        "SELECT json_array(hex(canonical_envelope),hex(payload_bytes),hex(submission_hash),state) FROM commands",
        "SELECT json_array(draft_id,draft_generation,command_id,hex(content_hash)) FROM issue_submissions",
        "SELECT json_array(kind,version,hex(payload),attempt_number) FROM command_evidence",
        "SELECT json_array(entity_id,provider_id,number,url) FROM issue_resolutions",
        "SELECT json_array(hex(execution_base),attempt_number) FROM delivery_attempt_context",
        "SELECT metadata_json FROM issue_draft_metadata",
    ] {
        result.push(sqlx::query_scalar(sql).fetch_all(&mut db).await.unwrap());
    }
    db.close().await.unwrap();
    result
}
#[tokio::test]
async fn metadata_history_preserves_exact_proofs_selections_and_quarantine_across_restore() {
    for state in [
        "queued",
        "outcome_unknown",
        "confirmed",
        "conflict",
        "cancelled",
    ] {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("target.db");
        let backup = dir.path().join("saved.db");
        let store = fixture(&target, state).await;
        let lease = store
            .begin_issue_metadata("a", "1", "repo", crate::IssueMetadataKind::Labels)
            .await
            .unwrap();
        store
            .apply_issue_metadata(
                &lease,
                crate::providers::IssueMetadataCatalogPage {
                    options: vec![crate::IssueMetadataOption {
                        reference: crate::IssueMetadataReference::Label(
                            crate::IssueMetadataLabel {
                                provider_id: "81".into(),
                                name: "selected".into(),
                                color: Some("123abc".into()),
                            },
                        ),
                        availability: crate::IssueMetadataAvailability::Available,
                        reason: None,
                    }],
                    next_cursor: None,
                    truncated: false,
                    coverage: crate::CoverageState::Partial,
                    cooldown_seconds: None,
                },
                TIME,
            )
            .await
            .unwrap();
        assert_eq!(store.backup_to(&backup).await.unwrap().schema_version, 26);
        store.close().await.unwrap();
        let original = std::fs::read(&backup).unwrap();
        let before = history(&backup).await;
        let session = RecoverySession::prepare(&target, &backup).await.unwrap();
        let confirmation = session.preview().confirmation_id.clone();
        session
            .confirm(&confirmation, RestoreChoice::ReplaceCurrentData)
            .unwrap();
        assert_eq!(std::fs::read(&backup).unwrap(), original);
        assert_eq!(history(&target).await, before);
        let restored = Store::open(&target).await.unwrap();
        let mut cached = connect(&target, true).await.unwrap();
        for sql in [
            "SELECT count(*) FROM repository_metadata_options",
            "SELECT count(*) FROM repository_metadata_catalogs",
        ] {
            assert_eq!(
                sqlx::query_scalar::<_, i64>(sql)
                    .fetch_one(&mut cached)
                    .await
                    .unwrap(),
                0
            );
        }
        let selected: String = sqlx::query_scalar("SELECT metadata_json FROM issue_draft_metadata")
            .fetch_one(&mut cached)
            .await
            .unwrap();
        assert_eq!(
            n::decode_json::<n::SelectionV2>(selected.as_bytes())
                .unwrap()
                .labels[0]
                .name,
            "selected"
        );
        cached.close().await.unwrap();
        let mut a = restored.account("a").await.unwrap();
        assert_eq!(a.state, AccountState::AuthRequired);
        a.authorization_epoch = (a.authorization_epoch.parse::<u64>().unwrap() + 1).to_string();
        a.state = AccountState::Active;
        restored.upsert_account(a).await.unwrap();
        if !matches!(state, "confirmed" | "cancelled") {
            assert!(
                restored
                    .delivery_command("a", COMMAND)
                    .await
                    .unwrap()
                    .reconcile_only()
            );
        }
        let second = dir.path().join("again.db");
        restored.backup_to(&second).await.unwrap();
        restored.close().await.unwrap();
        drop(RecoverySession::prepare(&target, &second).await.unwrap());
    }
}
async fn unlock_triggers(db: &mut SqliteConnection) -> Vec<String> {
    let rows: Vec<(String, String)> =
        sqlx::query_as("SELECT name,sql FROM sqlite_schema WHERE type='trigger' ORDER BY name")
            .fetch_all(&mut *db)
            .await
            .unwrap();
    for (name, _) in &rows {
        sqlx::raw_sql(sqlx::AssertSqlSafe(format!("DROP TRIGGER {name}")))
            .execute(&mut *db)
            .await
            .unwrap();
    }
    rows.into_iter().map(|(_, s)| s).collect()
}
#[tokio::test]
async fn metadata_restore_refuses_corrupted_proof_and_intent_links_preserving_both_files() {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("target.db");
    let backup = dir.path().join("saved.db");
    let store = fixture(&target, "confirmed").await;
    store.backup_to(&backup).await.unwrap();
    store.close().await.unwrap();
    let original = std::fs::read(&target).unwrap();
    for case in [
        "actor",
        "native-repository",
        "metadata-push",
        "metadata-selection",
        "outcome-extra-id",
        "core-body",
        "unknown-field",
        "proof-version",
        "proof-no-attempt",
        "missing-attempt-base",
        "different-attempt-base",
        "wrong-kind",
        "wrong-ordinal",
        "orphan-mapping",
        "mapping-native-id",
        "draft-same-generation",
        "metadata-noncanonical",
        "content-hash",
    ] {
        let bad = dir.path().join(format!("{case}.db"));
        std::fs::copy(&backup, &bad).unwrap();
        let mut db = connect(&bad, false).await.unwrap();
        let triggers = unlock_triggers(&mut db).await;
        match case {
            "proof-version" => {
                sqlx::raw_sql("UPDATE command_evidence SET version=1")
                    .execute(&mut db)
                    .await
                    .unwrap();
            }
            "proof-no-attempt" => {
                sqlx::raw_sql("UPDATE command_evidence SET attempt_number=NULL")
                    .execute(&mut db)
                    .await
                    .unwrap();
            }
            "missing-attempt-base" => {
                sqlx::raw_sql("DELETE FROM delivery_attempt_context")
                    .execute(&mut db)
                    .await
                    .unwrap();
            }
            "different-attempt-base" => {
                let b: Vec<u8> =
                    sqlx::query_scalar("SELECT execution_base FROM delivery_attempt_context")
                        .fetch_one(&mut db)
                        .await
                        .unwrap();
                let mut p: n::PreparationV2 = n::decode_json(&b).unwrap();
                p.frame.repository_path = "owner/other".into();
                sqlx::query("UPDATE delivery_attempt_context SET execution_base=?")
                    .bind(n::encode(&p).unwrap())
                    .execute(&mut db)
                    .await
                    .unwrap();
            }
            "wrong-kind" => {
                sqlx::raw_sql("UPDATE command_evidence SET kind='fixture.unrelated'")
                    .execute(&mut db)
                    .await
                    .unwrap();
            }
            "wrong-ordinal" => {
                sqlx::raw_sql("INSERT INTO command_evidence SELECT account_id,command_id,1,attempt_number,'fixture.unrelated',1,X'00',recorded_at FROM command_evidence; UPDATE delivery_resolutions SET evidence_ordinal=1;").execute(&mut db).await.unwrap();
            }
            "orphan-mapping" => {
                sqlx::raw_sql("DELETE FROM issue_resolutions; DELETE FROM delivery_resolutions; UPDATE commands SET state='outcome_unknown'; UPDATE delivery_attempts SET outcome='outcome_unknown'").execute(&mut db).await.unwrap();
            }
            "mapping-native-id" => {
                sqlx::raw_sql("UPDATE issue_resolutions SET provider_id='999'")
                    .execute(&mut db)
                    .await
                    .unwrap();
            }
            "draft-same-generation" => {
                sqlx::raw_sql(r#"UPDATE issue_draft_metadata SET metadata_json='{"labels":[],"assignees":[],"milestone":null}'"#).execute(&mut db).await.unwrap();
            }
            "metadata-noncanonical" => {
                sqlx::raw_sql("UPDATE issue_draft_metadata SET metadata_json=' {} '")
                    .execute(&mut db)
                    .await
                    .unwrap();
            }
            "content-hash" => {
                sqlx::raw_sql("UPDATE issue_submissions SET content_hash=zeroblob(32)")
                    .execute(&mut db)
                    .await
                    .unwrap();
            }
            _ => {
                let b: Vec<u8> = sqlx::query_scalar("SELECT payload FROM command_evidence")
                    .fetch_one(&mut db)
                    .await
                    .unwrap();
                let mut e: n::ReceiptV2 = n::decode_json(&b).unwrap();
                match case {
                    "actor" => e.preparation.actor = "8".into(),
                    "native-repository" => e.preparation.frame.repository_native = "2".into(),
                    "metadata-push" => e.preparation.metadata_push_access = Some(false),
                    "metadata-selection" => {
                        e.preparation.revalidated_metadata.labels[0].id = "82".into()
                    }
                    "outcome-extra-id" => {
                        e.metadata.labels = n::SetObservationV2::Known {
                            present_ids: vec!["999".into()],
                        }
                    }
                    "core-body" => e.core.body = None,
                    "unknown-field" => {}
                    _ => panic!("fixed case"),
                };
                let bytes = if case == "unknown-field" {
                    let mut b = n::encode(&e).unwrap();
                    b.pop();
                    b.extend_from_slice(b",\"extra\":null}");
                    b
                } else {
                    n::encode(&e).unwrap()
                };
                sqlx::query("UPDATE command_evidence SET payload=?")
                    .bind(bytes)
                    .execute(&mut db)
                    .await
                    .unwrap();
            }
        }
        for trigger in triggers {
            sqlx::raw_sql(sqlx::AssertSqlSafe(trigger))
                .execute(&mut db)
                .await
                .unwrap();
        }
        db.close().await.unwrap();
        let before = std::fs::read(&bad).unwrap();
        assert!(
            RecoverySession::prepare(&target, &bad).await.is_err(),
            "case {case}"
        );
        assert_eq!(std::fs::read(&bad).unwrap(), before, "case {case}");
        assert_eq!(std::fs::read(&target).unwrap(), original, "case {case}");
    }
}

#[tokio::test]
async fn metadata_decline_cannot_become_dispatch_or_an_unrelated_resolution() {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("target.db");
    let backup = dir.path().join("saved.db");
    let store = fixture(&target, "conflict").await;
    store.backup_to(&backup).await.unwrap();
    store.close().await.unwrap();
    let original = std::fs::read(&target).unwrap();
    for case in [
        "unknown-state",
        "accepted-state",
        "wrong-purpose",
        "no-proof",
        "actor",
        "wrong-hash",
        "unknown-reason",
        "attempted-decline",
    ] {
        let bad = dir.path().join(format!("{case}.db"));
        std::fs::copy(&backup, &bad).unwrap();
        let mut db = connect(&bad, false).await.unwrap();
        let triggers = unlock_triggers(&mut db).await;
        match case {
            "unknown-state" => {
                sqlx::raw_sql("UPDATE commands SET state='outcome_unknown'")
                    .execute(&mut db)
                    .await
                    .unwrap();
            }
            "accepted-state" => {
                sqlx::raw_sql("UPDATE commands SET state='accepted'")
                    .execute(&mut db)
                    .await
                    .unwrap();
            }
            "wrong-purpose" => {
                sqlx::raw_sql("UPDATE delivery_resolutions SET purpose='confirmed'")
                    .execute(&mut db)
                    .await
                    .unwrap();
            }
            "no-proof" => {
                sqlx::raw_sql("DELETE FROM delivery_resolutions; DELETE FROM command_evidence")
                    .execute(&mut db)
                    .await
                    .unwrap();
            }
            "attempted-decline" => {
                sqlx::query(
                    "INSERT INTO delivery_attempts VALUES('a',?,1,1,?,'outcome_unknown',?)",
                )
                .bind(COMMAND)
                .bind(TIME)
                .bind(TIME)
                .execute(&mut db)
                .await
                .unwrap();
            }
            _ => {
                let b: Vec<u8> = sqlx::query_scalar("SELECT payload FROM command_evidence")
                    .fetch_one(&mut db)
                    .await
                    .unwrap();
                let mut e: n::DeclinedV2 = n::decode_json(&b).unwrap();
                match case {
                    "actor" => e.actor = "8".into(),
                    "wrong-hash" => e.command_hash = "b".repeat(64),
                    "unknown-reason" => {}
                    _ => panic!("fixed case"),
                };
                let mut b = n::encode(&e).unwrap();
                if case == "unknown-reason" {
                    b = String::from_utf8(b)
                        .unwrap()
                        .replace("selection_changed", "permission_to_retry")
                        .into_bytes();
                }
                sqlx::query("UPDATE command_evidence SET payload=?")
                    .bind(b)
                    .execute(&mut db)
                    .await
                    .unwrap();
            }
        }
        for sql in triggers {
            sqlx::raw_sql(sqlx::AssertSqlSafe(sql))
                .execute(&mut db)
                .await
                .unwrap();
        }
        db.close().await.unwrap();
        let before = std::fs::read(&bad).unwrap();
        assert!(
            RecoverySession::prepare(&target, &bad).await.is_err(),
            "{case}"
        );
        assert_eq!(std::fs::read(&bad).unwrap(), before);
        assert_eq!(std::fs::read(&target).unwrap(), original);
    }
}
