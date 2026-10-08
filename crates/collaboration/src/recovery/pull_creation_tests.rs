//! Synthetic immutable PR history; no online grant, vault or provider calls.
use super::*;
use crate::{
    DetailActor, DetailBranch, DetailRepositoryRef, DetailValueState, MetadataField,
    MetadataSource, PullCreationContext, PullCreationPolicy, PullDraftKey, PullDraftValues,
    RemoteItem, RemoteItemKind, ResourceMetadataValues, Store, SubmitPullRequest,
    commands::{CommandDraft, CommandSubmission, CommandTarget, CommandTargetKind, seal_command},
    pull_creation::native as n,
    runtime::detail_tests::fixtures,
    storage::command_admission::{CommandAdmissionPolicy, CommandProtection},
};

const COMMAND: &str = "11111111-1111-4111-8111-111111111111";
const DRAFT: &str = "22222222-2222-4222-8222-222222222222";
const TIME: &str = "2026-10-08T00:00:00Z";

struct SyntheticAdmission;
#[async_trait::async_trait]
impl CommandAdmissionPolicy for SyntheticAdmission {
    const OPERATION_KIND: &'static str = n::OPERATION;
    const PAYLOAD_VERSION: u32 = 1;
    async fn validate(
        &self,
        _: &mut sqlx::Transaction<'_, Sqlite>,
        _: &RemoteAccount,
        _: &CommandSubmission,
    ) -> Result<Vec<CommandProtection>> {
        Ok(vec![])
    }
}
fn key() -> PullDraftKey {
    PullDraftKey {
        account_id: "a".into(),
        draft_id: DRAFT.into(),
        repository_id: "repo".into(),
    }
}
fn payload(view: String) -> n::Payload {
    let values = PullDraftValues {
        title: "Authored PR 雪".into(),
        body: "Private until explicitly sent".into(),
        source_branch: "feature".into(),
        base_branch: "main".into(),
        local_repository_id: "local-repo".into(),
        link_id: "link".into(),
        link_generation: "1".into(),
        is_draft: true,
    };
    n::Payload {
        request: SubmitPullRequest {
            context: PullCreationContext {
                key: key(),
                draft_generation: "1".into(),
                authorization_epoch: "1".into(),
                authorization_view: view,
                grant_id: "33333333-3333-4333-8333-333333333333".into(),
                source_oid: "a".repeat(40),
                base_oid: "b".repeat(40),
            },
            command_id: COMMAND.into(),
            policy: PullCreationPolicy::BestEffortCurrentBranches,
            confirm_current_branches: true,
        },
        actor_id: "7".into(),
        repository_native: "1".into(),
        local: n::LocalProof {
            local_repository_id: values.local_repository_id.clone(),
            link_id: values.link_id.clone(),
            link_generation: values.link_generation.clone(),
            registration_proof: "synthetic-registration".into(),
            remote_digest: "synthetic-remotes".into(),
            source_branch: values.source_branch.clone(),
            source_oid: "a".repeat(40),
        },
        values,
    }
}
fn proof(p: &n::Payload, repository: crate::RemoteRepository, hash: &[u8]) -> n::ReceiptEvidence {
    // Provider branches advanced after preview. A strict creation201 retains
    // these observed OIDs without rewriting the immutable inspected tips.
    let branch = |name: &str, c: &str| DetailBranch {
        name: name.into(),
        oid: c.repeat(40),
        repository: Some(DetailRepositoryRef {
            provider_id: "1".into(),
            full_name: repository.full_name.clone(),
            web_url: Some(repository.web_url.clone()),
        }),
    };
    let url = "https://github.com/owner/project/pull/91".to_owned();
    let item = RemoteItem {
        id: "github:pull:9007199254740995".into(),
        account_id: "a".into(),
        repository_id: Some("repo".into()),
        provider_id: "9007199254740995".into(),
        kind: RemoteItemKind::PullRequest,
        number: Some("91".into()),
        title: p.values.title.clone(),
        body: Some(p.values.body.clone()),
        body_omitted: false,
        author: Some("author".into()),
        web_url: Some(url.clone()),
        state: "open".into(),
        updated_at: TIME.into(),
        head_oid: Some("c".repeat(40)),
        is_draft: Some(true),
        reason: None,
        unread: None,
        native_inbox: None,
    };
    let values = ResourceMetadataValues {
        title: Some(item.title.clone()),
        state: Some("open".into()),
        author: Some(DetailActor {
            provider_id: "7".into(),
            login: "author".into(),
            web_url: None,
        }),
        web_url: Some(url),
        updated_at: Some(TIME.into()),
        is_draft: Some(true),
        head: Some(branch("feature", "c")),
        base: Some(branch("main", "d")),
        ..Default::default()
    };
    let evidence = n::ReceiptEvidence {
        preparation: n::Preparation {
            frame: n::Frame {
                repository,
                authorization_view: p.request.context.authorization_view.clone(),
                draft_generation: "1".into(),
                values: p.values.clone(),
                local: p.local.clone(),
            },
            actor: "7".into(),
            epoch: "1".into(),
            command_hash: hash.iter().map(|b| format!("{b:02x}")).collect(),
            source_oid: "a".repeat(40),
            base_oid: "b".repeat(40),
            observed_at: TIME.into(),
        },
        receipt: n::CreatedReceipt {
            item,
            created_at: TIME.into(),
            metadata: n::ReceiptMetadata {
                kind: RemoteItemKind::PullRequest,
                values,
                source: MetadataSource {
                    source: "github/pull-detail/2026-03-10".into(),
                    adapter_version: 1,
                    provider_updated_at: Some(TIME.into()),
                    observed_at: TIME.into(),
                },
                fields: MetadataField::COMMON
                    .into_iter()
                    .chain(MetadataField::PULL)
                    .map(|f| {
                        (
                            f,
                            if f == MetadataField::MergeBase {
                                DetailValueState::Omitted
                            } else {
                                DetailValueState::Known
                            },
                        )
                    })
                    .collect(),
            },
        },
    };
    assert!(
        n::receipt_matches(&evidence, p),
        "synthetic strong proof must obey the actual v1 codec"
    );
    evidence
}
async fn fixture(path: &Path, confirmed: bool) -> (Store, Vec<u8>) {
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
    let p = payload(view.to_string());
    let v = &p.values;
    sqlx::query("INSERT INTO pull_drafts VALUES('a',?,'repo',?,?,?,?,?,?,?,?,1)")
        .bind(DRAFT)
        .bind(&v.title)
        .bind(&v.body)
        .bind(&v.source_branch)
        .bind(&v.base_branch)
        .bind(&v.local_repository_id)
        .bind(&v.link_id)
        .bind(&v.link_generation)
        .bind(v.is_draft)
        .execute(&mut db)
        .await
        .unwrap();
    let command = seal_command(CommandDraft {
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
    store
        .admit_command(&command, &SyntheticAdmission)
        .await
        .unwrap();
    let hash: Vec<u8> =
        sqlx::query_scalar("SELECT submission_hash FROM commands WHERE command_id=?")
            .bind(COMMAND)
            .fetch_one(&mut db)
            .await
            .unwrap();
    sqlx::query("INSERT INTO pull_submissions VALUES('a',?,1,?,?,?)")
        .bind(DRAFT)
        .bind(COMMAND)
        .bind(&hash)
        .bind(n::content_hash(v).unwrap())
        .execute(&mut db)
        .await
        .unwrap();
    if confirmed {
        let repo: String =
            sqlx::query_scalar("SELECT json FROM repositories WHERE account_id='a' AND id='repo'")
                .fetch_one(&mut db)
                .await
                .unwrap();
        let evidence = proof(&p, serde_json::from_str(&repo).unwrap(), &hash);
        sqlx::query("INSERT INTO delivery_attempts VALUES('a',?,1,1,?,'confirmed',?)")
            .bind(COMMAND)
            .bind(TIME)
            .bind(TIME)
            .execute(&mut db)
            .await
            .unwrap();
        sqlx::query(
            "INSERT INTO delivery_attempt_context VALUES('a',?,1,'github:https://github.com/',?)",
        )
        .bind(COMMAND)
        .bind(n::encode(&evidence.preparation).unwrap())
        .execute(&mut db)
        .await
        .unwrap();
        sqlx::query("INSERT INTO command_evidence VALUES('a',?,0,1,'github.pull_created',1,?,?)")
            .bind(COMMAND)
            .bind(n::encode(&evidence).unwrap())
            .bind(TIME)
            .execute(&mut db)
            .await
            .unwrap();
        sqlx::query("INSERT INTO delivery_resolutions VALUES('a',?,1,0,'confirmed')")
            .bind(COMMAND)
            .execute(&mut db)
            .await
            .unwrap();
        let item = &evidence.receipt.item;
        sqlx::query("INSERT INTO pull_resolutions VALUES('a',?,?,?,?,?,?,?,?)")
            .bind(DRAFT)
            .bind(COMMAND)
            .bind(&item.id)
            .bind(&item.provider_id)
            .bind(item.number.as_ref().unwrap())
            .bind(item.web_url.as_ref().unwrap())
            .bind("c".repeat(40))
            .bind("d".repeat(40))
            .execute(&mut db)
            .await
            .unwrap();
        sqlx::query("INSERT INTO items VALUES('a',?,'repo','pull_request','open',?,?)")
            .bind(&item.id)
            .bind(TIME)
            .bind(serde_json::to_string(item).unwrap())
            .execute(&mut db)
            .await
            .unwrap();
        sqlx::query("INSERT INTO pull_creation_visibility VALUES('a','1',?,?)")
            .bind(&item.id)
            .bind(COMMAND)
            .execute(&mut db)
            .await
            .unwrap();
        sqlx::query("UPDATE commands SET state='confirmed' WHERE command_id=?")
            .bind(COMMAND)
            .execute(&mut db)
            .await
            .unwrap();
    }
    db.close().await.unwrap();
    (store, command.payload_bytes().to_vec())
}
async fn history(path: &Path) -> Vec<Vec<String>> {
    let mut db = connect(path, true).await.unwrap();
    let mut result = vec![];
    for sql in [
        "SELECT json_array(command_id,hex(canonical_envelope),hex(payload_bytes),hex(guard_bytes),hex(submission_hash),state) FROM commands ORDER BY command_id",
        "SELECT json_array(draft_id,draft_generation,command_id,hex(submission_hash),hex(content_hash)) FROM pull_submissions ORDER BY draft_id,draft_generation",
        "SELECT json_array(command_id,ordinal,kind,version,hex(payload)) FROM command_evidence ORDER BY command_id,ordinal",
        "SELECT json_array(draft_id,command_id,entity_id,provider_id,number,url,observed_source_oid,observed_base_oid) FROM pull_resolutions ORDER BY draft_id",
        "SELECT json_array(command_id,attempt_number,instance_id,hex(execution_base)) FROM delivery_attempt_context ORDER BY command_id,attempt_number",
    ] {
        result.push(sqlx::query_scalar(sql).fetch_all(&mut db).await.unwrap());
    }
    db.close().await.unwrap();
    result
}

#[tokio::test]
async fn queued_unknown_and_confirmed_pull_history_restores_without_online_send_authority() {
    for state in ["queued", "outcome_unknown", "confirmed"] {
        let confirmed = state == "confirmed";
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("current.db");
        let backup = dir.path().join("saved.db");
        let (store, bytes) = fixture(&target, confirmed).await;
        if state == "outcome_unknown" {
            let mut db = connect(&target, false).await.unwrap();
            let raw: Vec<u8> = sqlx::query_scalar("SELECT payload_bytes FROM commands")
                .fetch_one(&mut db)
                .await
                .unwrap();
            let p = n::decode_parts(&raw, "a", COMMAND, "repo", "1").unwrap();
            let repo: String = sqlx::query_scalar("SELECT json FROM repositories WHERE id='repo'")
                .fetch_one(&mut db)
                .await
                .unwrap();
            let hash: Vec<u8> = sqlx::query_scalar("SELECT submission_hash FROM commands")
                .fetch_one(&mut db)
                .await
                .unwrap();
            let prep = proof(&p, serde_json::from_str(&repo).unwrap(), &hash).preparation;
            sqlx::query("INSERT INTO delivery_attempts VALUES('a',?,1,1,?,'outcome_unknown',?)")
                .bind(COMMAND)
                .bind(TIME)
                .bind(TIME)
                .execute(&mut db)
                .await
                .unwrap();
            sqlx::query("INSERT INTO delivery_attempt_context VALUES('a',?,1,'github:https://github.com/',?)").bind(COMMAND).bind(n::encode(&prep).unwrap()).execute(&mut db).await.unwrap();
            sqlx::query("UPDATE commands SET state='outcome_unknown'")
                .execute(&mut db)
                .await
                .unwrap();
            db.close().await.unwrap();
        }
        let summary = store.backup_to(&backup).await.unwrap();
        assert_eq!(summary.schema_version, 25);
        assert_eq!(summary.drafts, 1);
        assert_eq!(summary.commands, 1);
        store.close().await.unwrap();
        let original = std::fs::read(&backup).unwrap();
        let before = history(&backup).await;
        let session = RecoverySession::prepare(&target, &backup).await.unwrap();
        assert_eq!(
            session.preview().quarantined_commands,
            u64::from(!confirmed)
        );
        let id = session.preview().confirmation_id.clone();
        session
            .confirm(&id, RestoreChoice::ReplaceCurrentData)
            .unwrap();
        assert_eq!(std::fs::read(&backup).unwrap(), original);
        assert_eq!(history(&target).await, before);
        let restored = Store::open(&target).await.unwrap();
        let mut a = restored.account("a").await.unwrap();
        assert_eq!(a.state, AccountState::AuthRequired);
        a.authorization_epoch = (a.authorization_epoch.parse::<u64>().unwrap() + 1).to_string();
        a.state = AccountState::Active;
        restored.upsert_account(a).await.unwrap();
        let c = restored.delivery_command("a", COMMAND).await.unwrap();
        assert_eq!(c.payload, bytes);
        if confirmed {
            assert_eq!(c.state, crate::delivery::DeliveryState::Confirmed);
        } else {
            assert!(c.reconcile_only());
        }
        let mut db = connect(&target, false).await.unwrap();
        assert_eq!(
            sqlx::query_scalar::<_, String>("SELECT body FROM pull_drafts")
                .fetch_one(&mut db)
                .await
                .unwrap(),
            "Private until explicitly sent"
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT count(*) FROM pull_creation_visibility")
                .fetch_one(&mut db)
                .await
                .unwrap(),
            0
        );
        if !confirmed {
            let denied =
                sqlx::query("INSERT INTO delivery_attempts VALUES('a',?,?,3,?,'started',NULL)")
                    .bind(COMMAND)
                    .bind(if state == "queued" { 1 } else { 2 })
                    .bind(TIME)
                    .execute(&mut db)
                    .await
                    .unwrap_err();
            assert!(
                denied
                    .to_string()
                    .contains("restored command requires reconciliation")
            );
        }
        db.close().await.unwrap();
        let again = dir.path().join("restored.db");
        restored.backup_to(&again).await.unwrap();
        restored.close().await.unwrap();
        drop(RecoverySession::prepare(&target, &again).await.unwrap());
    }
}

async fn temporarily_remove_triggers(db: &mut SqliteConnection, names: &[&str]) -> Vec<String> {
    let mut result = vec![];
    for name in names {
        let sql = sqlx::query_scalar::<_, String>("SELECT sql FROM sqlite_schema WHERE name=?")
            .bind(name)
            .fetch_one(&mut *db)
            .await
            .unwrap();
        sqlx::raw_sql(sqlx::AssertSqlSafe(format!("DROP TRIGGER {name}")))
            .execute(&mut *db)
            .await
            .unwrap();
        result.push(sql);
    }
    result
}
#[tokio::test]
async fn pull_restore_refuses_broken_authorship_proof_and_cache_links_without_changing_inputs() {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("current.db");
    let backup = dir.path().join("saved.db");
    let (store, _) = fixture(&target, true).await;
    store.backup_to(&backup).await.unwrap();
    store.close().await.unwrap();
    let current = std::fs::read(&target).unwrap();
    for case in [
        "wrong-kind",
        "wrong-ordinal",
        "orphan-proof",
        "resolution-head",
        "content-hash",
        "draft-same-generation",
        "draft-invalid-ref",
        "marker-epoch",
        "marker-identity",
        "actor",
        "local-proof",
        "head-ref",
        "native-inbox",
        "clock-order",
        "metadata-state",
        "fabricated-merge-base",
        "attempt-base",
        "missing-attempt-base",
        "proof-attempt-link",
    ] {
        let bad = dir.path().join(format!("{case}.db"));
        std::fs::copy(&backup, &bad).unwrap();
        let mut db = connect(&bad, false).await.unwrap();
        let names: &[&str] = match case {
            "wrong-ordinal" => &["delivery_resolution_immutable"],
            "orphan-proof" => &["pull_resolution_retained", "delivery_resolution_retained"],
            "resolution-head" => &["pull_resolution_immutable"],
            "content-hash" => &["pull_submission_immutable"],
            "attempt-base" => &["delivery_context_immutable"],
            "missing-attempt-base" => &["delivery_context_retained"],
            _ => &["evidence_immutable"],
        };
        let triggers = temporarily_remove_triggers(&mut db, names).await;
        match case {
            "attempt-base" => {
                let bytes: Vec<u8> =
                    sqlx::query_scalar("SELECT execution_base FROM delivery_attempt_context")
                        .fetch_one(&mut db)
                        .await
                        .unwrap();
                let mut prep: n::Preparation = n::decode_json(&bytes).unwrap();
                prep.frame.local.registration_proof = "different".into();
                sqlx::query("UPDATE delivery_attempt_context SET execution_base=?")
                    .bind(n::encode(&prep).unwrap())
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
            "proof-attempt-link" => {
                sqlx::raw_sql("UPDATE command_evidence SET attempt_number=NULL")
                    .execute(&mut db)
                    .await
                    .unwrap();
            }
            "wrong-kind" => {
                sqlx::raw_sql("UPDATE command_evidence SET kind='fixture.unrelated';")
                    .execute(&mut db)
                    .await
                    .unwrap();
            }
            "wrong-ordinal" => {
                sqlx::raw_sql("INSERT INTO command_evidence SELECT account_id,command_id,1,attempt_number,'fixture.unrelated',1,X'00',recorded_at FROM command_evidence; UPDATE delivery_resolutions SET evidence_ordinal=1;").execute(&mut db).await.unwrap();
            }
            "orphan-proof" => {
                sqlx::raw_sql("DELETE FROM pull_creation_visibility; DELETE FROM pull_resolutions; DELETE FROM delivery_resolutions; UPDATE commands SET state='outcome_unknown'; UPDATE delivery_attempts SET outcome='outcome_unknown';").execute(&mut db).await.unwrap();
            }
            "resolution-head" => {
                sqlx::query("UPDATE pull_resolutions SET observed_source_oid=?")
                    .bind("e".repeat(40))
                    .execute(&mut db)
                    .await
                    .unwrap();
            }
            "content-hash" => {
                sqlx::raw_sql("UPDATE pull_submissions SET content_hash=zeroblob(32)")
                    .execute(&mut db)
                    .await
                    .unwrap();
            }
            "draft-same-generation" => {
                sqlx::raw_sql("UPDATE pull_drafts SET source_branch='other'")
                    .execute(&mut db)
                    .await
                    .unwrap();
            }
            "draft-invalid-ref" => {
                sqlx::raw_sql(
                    "UPDATE pull_drafts SET source_branch='invalid..branch',generation=2",
                )
                .execute(&mut db)
                .await
                .unwrap();
            }
            "marker-epoch" => {
                sqlx::raw_sql("UPDATE pull_creation_visibility SET authorization_epoch='2'")
                    .execute(&mut db)
                    .await
                    .unwrap();
            }
            "marker-identity" => {
                sqlx::raw_sql("UPDATE items SET json=json_set(json,'$.provider_id','999') WHERE kind='pull_request'").execute(&mut db).await.unwrap();
            }
            other => {
                let bytes: Vec<u8> = sqlx::query_scalar("SELECT payload FROM command_evidence")
                    .fetch_one(&mut db)
                    .await
                    .unwrap();
                let bytes = if other == "native-inbox" {
                    let s = String::from_utf8(bytes).unwrap();
                    assert!(!s.contains("native_inbox"));
                    s.replace("\"unread\":null", "\"unread\":null,\"native_inbox\":null")
                        .into_bytes()
                } else {
                    let mut e: n::ReceiptEvidence = n::decode_json(&bytes).unwrap();
                    match other {
                        "actor" => {
                            e.receipt
                                .metadata
                                .values
                                .author
                                .as_mut()
                                .unwrap()
                                .provider_id = "8".into()
                        }
                        "local-proof" => {
                            e.preparation.frame.local.remote_digest = "different".into()
                        }
                        "head-ref" => {
                            e.receipt.metadata.values.head.as_mut().unwrap().name = "other".into()
                        }
                        "clock-order" => e.receipt.created_at = "2026-10-09T00:00:00Z".into(),
                        "fabricated-merge-base" => {
                            e.receipt.metadata.values.merge_base_oid = Some("e".repeat(40));
                            e.receipt
                                .metadata
                                .fields
                                .iter_mut()
                                .find(|(f, _)| *f == MetadataField::MergeBase)
                                .unwrap()
                                .1 = DetailValueState::Known;
                        }
                        "metadata-state" => {
                            e.receipt
                                .metadata
                                .fields
                                .iter_mut()
                                .find(|(f, _)| *f == MetadataField::Head)
                                .unwrap()
                                .1 = DetailValueState::Omitted
                        }
                        _ => unreachable!(),
                    }
                    n::encode(&e).unwrap()
                };
                sqlx::query("UPDATE command_evidence SET payload=?")
                    .bind(bytes)
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
        let selected = std::fs::read(&bad).unwrap();
        assert!(
            RecoverySession::prepare(&target, &bad).await.is_err(),
            "{case}"
        );
        assert_eq!(std::fs::read(&bad).unwrap(), selected, "{case}");
        assert_eq!(std::fs::read(&target).unwrap(), current, "{case}");
    }
}

#[tokio::test]
async fn declines_before_or_after_claim_retain_exact_context_and_resolution() {
    for attempted in [false, true] {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("current.db");
        let backup = dir.path().join("saved.db");
        let (store, _) = fixture(&target, false).await;
        let mut db = connect(&target, false).await.unwrap();
        let raw: Vec<u8> = sqlx::query_scalar("SELECT payload_bytes FROM commands")
            .fetch_one(&mut db)
            .await
            .unwrap();
        let p = n::decode_parts(&raw, "a", COMMAND, "repo", "1").unwrap();
        let repo: String = sqlx::query_scalar("SELECT json FROM repositories WHERE id='repo'")
            .fetch_one(&mut db)
            .await
            .unwrap();
        let hash: Vec<u8> = sqlx::query_scalar("SELECT submission_hash FROM commands")
            .fetch_one(&mut db)
            .await
            .unwrap();
        let declined = n::DeclinedEvidence {
            preparation: proof(&p, serde_json::from_str(&repo).unwrap(), &hash).preparation,
            reason: crate::PullCreationReason::GrantExpired,
        };
        assert!(n::declined_matches(&declined, &p));
        if attempted {
            sqlx::query("INSERT INTO delivery_attempts VALUES('a',?,1,1,?,'rejected',?)")
                .bind(COMMAND)
                .bind(TIME)
                .bind(TIME)
                .execute(&mut db)
                .await
                .unwrap();
            sqlx::query("INSERT INTO delivery_attempt_context VALUES('a',?,1,'github:https://github.com/',?)").bind(COMMAND).bind(n::encode(&declined.preparation).unwrap()).execute(&mut db).await.unwrap();
        }
        sqlx::query(
            "INSERT INTO command_evidence VALUES('a',?,0,?,'github.pull_creation_declined',1,?,?)",
        )
        .bind(COMMAND)
        .bind(attempted.then_some(1_i64))
        .bind(n::encode(&declined).unwrap())
        .bind(TIME)
        .execute(&mut db)
        .await
        .unwrap();
        sqlx::query("INSERT INTO delivery_resolutions VALUES('a',?,1,0,'conflict')")
            .bind(COMMAND)
            .execute(&mut db)
            .await
            .unwrap();
        sqlx::query("UPDATE commands SET state='conflict'")
            .execute(&mut db)
            .await
            .unwrap();
        db.close().await.unwrap();
        store.backup_to(&backup).await.unwrap();
        store.close().await.unwrap();
        let before = std::fs::read(&target).unwrap();
        drop(RecoverySession::prepare(&target, &backup).await.unwrap());
        for case in [
            "unsupported-reason",
            "wrong-hash",
            "wrong-purpose",
            "missing-resolution",
        ] {
            let bad = dir.path().join(format!("{case}.db"));
            std::fs::copy(&backup, &bad).unwrap();
            let mut db = connect(&bad, false).await.unwrap();
            let names: &[&str] = match case {
                "missing-resolution" => &["delivery_resolution_retained"],
                "wrong-purpose" => &["delivery_resolution_immutable"],
                _ => &["evidence_immutable"],
            };
            let triggers = temporarily_remove_triggers(&mut db, names).await;
            match case {
                "missing-resolution" => {
                    sqlx::query("DELETE FROM delivery_resolutions")
                        .execute(&mut db)
                        .await
                        .unwrap();
                }
                "wrong-purpose" => {
                    sqlx::query("UPDATE delivery_resolutions SET purpose='accepted'")
                        .execute(&mut db)
                        .await
                        .unwrap();
                }
                _ => {
                    let mut e = declined.clone();
                    if case == "unsupported-reason" {
                        e.reason = crate::PullCreationReason::MissingRepository;
                    } else {
                        e.preparation.command_hash = "0".repeat(64);
                    }
                    sqlx::query("UPDATE command_evidence SET payload=?")
                        .bind(n::encode(&e).unwrap())
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
            let selected = std::fs::read(&bad).unwrap();
            assert!(
                RecoverySession::prepare(&target, &bad).await.is_err(),
                "{case}"
            );
            assert_eq!(std::fs::read(&bad).unwrap(), selected);
            assert_eq!(std::fs::read(&target).unwrap(), before);
        }
        let session = RecoverySession::prepare(&target, &backup).await.unwrap();
        let id = session.preview().confirmation_id.clone();
        session
            .confirm(&id, RestoreChoice::ReplaceCurrentData)
            .unwrap();
        assert_eq!(history(&target).await, history(&backup).await);
        let mut db = connect(&target, true).await.unwrap();
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT count(*) FROM delivery_attempts")
                .fetch_one(&mut db)
                .await
                .unwrap(),
            i64::from(attempted)
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT count(*) FROM command_recovery_quarantine")
                .fetch_one(&mut db)
                .await
                .unwrap(),
            1
        );
        db.close().await.unwrap();
    }
}
