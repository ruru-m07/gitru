//! Synthetic fixtures exercise the actual Store/runtime/provider boundaries.
use super::*;
use crate::credentials::{CredentialError, CredentialVault, SecretToken};
use std::{fs, path::PathBuf, time::Duration};

struct Run {
    directory: tempfile::TempDir,
    root: PathBuf,
    nonce: String,
}
impl Run {
    fn new() -> Self {
        let directory = tempfile::tempdir().unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700)).unwrap();
        }
        let root = fs::canonicalize(directory.path()).unwrap();
        let nonce = uuid::Uuid::new_v4().to_string();
        fs::write(root.join("run.json"), serde_json::to_vec(&serde_json::json!({ "version": 1, "application_id": APPLICATION_ID, "run_nonce": nonce })).unwrap()).unwrap();
        Self {
            directory,
            root,
            nonce,
        }
    }
    async fn open(&self) -> HarnessSession {
        HarnessSession::open(
            &self.root,
            &self.nonce,
            Arc::new(|owner| owner.starts_with("test-")),
        )
        .await
        .unwrap()
    }
}

async fn action(session: &HarnessSession, action: HarnessCoreAction) -> HarnessCoreReceipt {
    let status = session
        .control
        .status(&session.control.0.nonce)
        .await
        .unwrap();
    session
        .control
        .execute(HarnessCoreRequest {
            run_nonce: status.run_nonce,
            expected_generation: status.scenario_generation,
            action,
            gate_id: None,
        })
        .await
        .unwrap()
}
async fn prepared(run: &Run) -> HarnessSession {
    let session = run.open().await;
    action(&session, HarnessCoreAction::PreparePrimary).await;
    session
}
async fn interest(
    session: &HarnessSession,
    owner: &str,
    slot: HarnessActorSlot,
) -> (DemandOwnerActivity, DemandLeaseReceipt) {
    let activity = session.runtime.demand_owner_activity(owner).await.unwrap();
    let activity = session
        .runtime
        .set_demand_owner_activity(owner, &activity.generation, true)
        .await
        .unwrap();
    let account = session.store.account(account_id(slot)).await.unwrap();
    let lease = session
        .runtime
        .acquire_demand(
            owner,
            AcquireDemandRequest {
                account_id: account.id,
                authorization_epoch: account.authorization_epoch,
                owner_generation: activity.generation.clone(),
                target: DemandTarget {
                    kind: DemandTargetKind::Detail,
                    repository_id: None,
                    subject_id: Some(SUBJECT_ID.into()),
                    facet: Some(DetailFacet::Body),
                },
            },
        )
        .await
        .unwrap();
    (activity, lease)
}
async fn detail(session: &HarnessSession, slot: HarnessActorSlot) -> DetailSnapshot {
    session
        .store
        .detail(DetailQuery {
            account_id: account_id(slot).into(),
            subject_id: SUBJECT_ID.into(),
            facet: DetailFacet::Body,
            cursor: None,
            limit: 1,
        })
        .await
        .unwrap()
}
async fn pull_commits(session: &HarnessSession) -> PullCommitSnapshot {
    session
        .store
        .pull_commits(PullCommitQuery {
            account_id: PRIMARY_ACCOUNT.into(),
            subject_id: SUBJECT_ID.into(),
            cursor: None,
            limit: 50,
        })
        .await
        .unwrap()
}
fn assert_fixture_pull_commits(snapshot: &PullCommitSnapshot) {
    assert_eq!(snapshot.subject_id, SUBJECT_ID);
    let context = snapshot
        .context
        .as_ref()
        .expect("published commit fixture has exact context");
    assert_eq!(context.base_oid, BASE_OID);
    assert_eq!(context.head_oid, HEAD_OID);
    assert_eq!(
        context.source_repository_provider_id,
        SOURCE_REPOSITORY_PROVIDER_ID
    );
    assert!(context.metadata_facet_revision.parse::<u64>().unwrap() > 0);
    assert!(snapshot.facet_revision.is_some());
    assert_eq!(
        snapshot
            .commits
            .iter()
            .map(|commit| (commit.position, commit.oid.as_str()))
            .collect::<Vec<_>>(),
        vec![(0, FIRST_COMMIT_OID), (1, HEAD_OID)]
    );
    assert_eq!(snapshot.completeness, PullCommitCompleteness::complete());
    assert_eq!(snapshot.coverage.state, CoverageState::Complete);
    assert!(!snapshot.coverage.remote_has_more);
    assert!(snapshot.next_cursor.is_none());
}
async fn held(session: &HarnessSession, gate: &str) {
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if session
                .control
                .status(&session.control.0.nonce)
                .await
                .unwrap()
                .gates
                .iter()
                .any(|entry| entry.gate_id == gate && entry.state == HarnessGateState::Held)
            {
                return;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("actual admitted fixture gate reached");
}
async fn release(session: &HarnessSession, gate: &str) {
    let status = session
        .control
        .status(&session.control.0.nonce)
        .await
        .unwrap();
    session
        .control
        .execute(HarnessCoreRequest {
            run_nonce: status.run_nonce,
            expected_generation: status.scenario_generation,
            action: HarnessCoreAction::ReleaseProviderGate,
            gate_id: Some(gate.into()),
        })
        .await
        .unwrap();
}

#[tokio::test]
async fn prepared_fixture_has_real_selected_cache_and_separate_actor_draft_cas() {
    let run = Run::new();
    let session = prepared(&run).await;
    let status = session.control.status(&run.nonce).await.unwrap();
    assert_eq!(status.actors.len(), 2);
    assert_eq!(status.provider_call_count, "0");
    assert_eq!(status.vault_store_count, "2");
    assert_eq!(status.durable_detail_requests, 0);
    let inbox = session
        .store
        .inbox(InboxQuery {
            account_id: PRIMARY_ACCOUNT.into(),
            remote_state: None,
            local_state: LocalInboxFilter::All,
            search: None,
            cursor: None,
            limit: 100,
        })
        .await
        .unwrap();
    assert_eq!(inbox.entries.len(), 3);
    assert_eq!(
        inbox
            .entries
            .iter()
            .map(|entry| entry.item.id.as_str())
            .collect::<std::collections::BTreeSet<_>>(),
        [
            DONE_NOTIFICATION_ID,
            SNOOZED_NOTIFICATION_ID,
            BOOKMARKED_NOTIFICATION_ID,
        ]
        .into_iter()
        .collect()
    );
    assert!(inbox.entries.iter().all(|entry| {
        entry.local.generation == "0"
            && entry.local.effective_disposition == LocalInboxEffectiveDisposition::Inbox
            && !entry.local.bookmarked
    }));
    assert_eq!(status.provider_call_count, "0");
    assert_eq!(status.vault_load_count, "0");
    for slot in [HarnessActorSlot::Primary, HarnessActorSlot::Alternate] {
        assert!(
            session
                .store
                .repository(account_id(slot), REPOSITORY_ID)
                .await
                .unwrap()
                .selected
        );
        assert_eq!(
            detail(&session, slot).await.body.state,
            DetailValueState::NotLoaded
        );
        assert_eq!(
            session
                .store
                .draft(account_id(slot), SUBJECT_ID)
                .await
                .unwrap()
                .unwrap()
                .generation,
            "1"
        );
    }
    assert_ne!(
        session
            .store
            .draft(PRIMARY_ACCOUNT, SUBJECT_ID)
            .await
            .unwrap()
            .unwrap()
            .body,
        session
            .store
            .draft(ALTERNATE_ACCOUNT, SUBJECT_ID)
            .await
            .unwrap()
            .unwrap()
            .body
    );
    assert_eq!(status.committed_phase, None);
}

#[tokio::test]
async fn equivalent_real_leases_coalesce_and_queries_never_dispatch() {
    let run = Run::new();
    let session = prepared(&run).await;
    let gate = action(&session, HarnessCoreAction::ArmProviderGate)
        .await
        .gate_id
        .unwrap();
    let (_, first) = interest(&session, "test-main", HarnessActorSlot::Primary).await;
    let (_, second) = interest(&session, "test-child", HarnessActorSlot::Primary).await;
    let runtime = session.runtime.clone();
    let worker = tokio::spawn(async move { runtime.harness_run_next().await });
    held(&session, &gate).await;
    let status = session.control.status(&run.nonce).await.unwrap();
    assert_eq!(status.demand_lease_count, 2);
    assert_eq!(status.provider_call_count, "1");
    for _ in 0..5 {
        assert_eq!(
            detail(&session, HarnessActorSlot::Primary).await.body.state,
            DetailValueState::NotLoaded
        );
    }
    assert!(!session.runtime.harness_run_next().await);
    release(&session, &gate).await;
    assert!(worker.await.unwrap());
    let snapshot = detail(&session, HarnessActorSlot::Primary).await;
    assert_eq!(
        snapshot.body.text.as_deref(),
        fixture_body(HarnessActorSlot::Primary, HarnessPhase::One)
    );
    assert_eq!(
        snapshot.metadata.unwrap().values.title.as_deref(),
        Some("Native endpoint title phase one")
    );
    assert_eq!(
        session
            .control
            .status(&run.nonce)
            .await
            .unwrap()
            .committed_facet_revision,
        snapshot.evidence.facet_revision
    );
    session
        .runtime
        .release_demand(
            "test-main",
            ReleaseDemandRequest {
                lease_id: first.lease_id,
            },
        )
        .await
        .unwrap();
    action(&session, HarnessCoreAction::PhaseTwo).await;
    action(&session, HarnessCoreAction::AdvanceRefresh).await;
    assert!(session.runtime.harness_run_next().await);
    assert_eq!(
        detail(&session, HarnessActorSlot::Primary)
            .await
            .body
            .text
            .as_deref(),
        fixture_body(HarnessActorSlot::Primary, HarnessPhase::Two)
    );
    session
        .runtime
        .release_demand(
            "test-child",
            ReleaseDemandRequest {
                lease_id: second.lease_id,
            },
        )
        .await
        .unwrap();
    action(&session, HarnessCoreAction::AdvanceRefresh).await;
    assert!(!session.runtime.harness_run_next().await);
    let status = session.control.status(&run.nonce).await.unwrap();
    assert_eq!(status.provider_call_count, "2");
    assert_eq!(status.durable_detail_requests, 0);
}

#[tokio::test]
async fn identical_canonical_subjects_never_share_actor_body_or_private_generations() {
    let run = Run::new();
    let session = prepared(&run).await;
    for (owner, slot) in [
        ("test-primary", HarnessActorSlot::Primary),
        ("test-alternate", HarnessActorSlot::Alternate),
    ] {
        interest(&session, owner, slot).await;
        assert!(session.runtime.harness_run_next().await);
        assert_eq!(
            detail(&session, slot).await.body.text.as_deref(),
            fixture_body(slot, HarnessPhase::One)
        );
    }
    let primary = session
        .store
        .draft(PRIMARY_ACCOUNT, SUBJECT_ID)
        .await
        .unwrap()
        .unwrap();
    let alternate = session
        .store
        .draft(ALTERNATE_ACCOUNT, SUBJECT_ID)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(primary.generation, alternate.generation);
    session
        .store
        .save_draft(LocalDraft {
            body: "RURU-103 primary edited private draft".into(),
            ..primary.clone()
        })
        .await
        .unwrap();
    assert_eq!(
        session.store.save_draft(primary).await.unwrap_err().code,
        ErrorCode::StaleView
    );
    assert_eq!(
        session
            .store
            .draft(ALTERNATE_ACCOUNT, SUBJECT_ID)
            .await
            .unwrap(),
        Some(alternate)
    );
    let status = session.control.status(&run.nonce).await.unwrap();
    assert_eq!(status.provider_call_count, "2");
    assert_eq!(
        status
            .calls
            .iter()
            .filter(|call| call.slot == HarnessActorSlot::Primary)
            .count(),
        1
    );
    assert_eq!(
        status
            .calls
            .iter()
            .filter(|call| call.slot == HarnessActorSlot::Alternate)
            .count(),
        1
    );
    let encoded = serde_json::to_string(&status).unwrap();
    for private in [
        vault::PRIMARY_TOKEN,
        vault::ALTERNATE_TOKEN,
        "RURU-103 primary edited private draft",
        fixture_body(HarnessActorSlot::Primary, HarnessPhase::One).unwrap(),
    ] {
        assert!(
            !encoded.contains(private),
            "native metrics must exclude private payloads"
        );
    }
}

#[tokio::test]
async fn diagnostic_history_is_bounded_after_actual_repeated_native_refreshes() {
    let run = Run::new();
    let session = prepared(&run).await;
    let (activity, lease) = interest(&session, "test-main", HarnessActorSlot::Primary).await;
    for index in 0..130 {
        if index > 0 {
            action(&session, HarnessCoreAction::AdvanceRefresh).await;
        }
        session
            .runtime
            .renew_demand(
                "test-main",
                RenewDemandRequest {
                    owner_generation: activity.generation.clone(),
                    leases: vec![DemandLeaseRenewal {
                        lease_id: lease.lease_id.clone(),
                        account_id: PRIMARY_ACCOUNT.into(),
                        authorization_epoch: "1".into(),
                    }],
                },
            )
            .await
            .unwrap();
        assert!(session.runtime.harness_run_next().await);
    }
    let status = session.control.status(&run.nonce).await.unwrap();
    assert_eq!(status.provider_call_count, "130");
    assert_eq!(status.calls.len(), 128);
    assert_eq!(status.calls.first().unwrap().call_id, "3");
    assert_eq!(status.calls.last().unwrap().call_id, "130");
    assert_eq!(status.demand_lease_count, 1);
    assert_eq!(status.durable_detail_requests, 0);
}

#[tokio::test]
async fn old_epoch_held_result_cannot_restore_provider_content_and_private_draft_survives() {
    let run = Run::new();
    let session = prepared(&run).await;
    let draft = session
        .store
        .draft(PRIMARY_ACCOUNT, SUBJECT_ID)
        .await
        .unwrap()
        .unwrap();
    let gate = action(&session, HarnessCoreAction::ArmProviderGate)
        .await
        .gate_id
        .unwrap();
    interest(&session, "test-main", HarnessActorSlot::Primary).await;
    let runtime = session.runtime.clone();
    let worker = tokio::spawn(async move { runtime.harness_run_next().await });
    held(&session, &gate).await;
    session.runtime.disconnect(PRIMARY_ACCOUNT).await.unwrap();
    release(&session, &gate).await;
    assert!(worker.await.unwrap());
    assert_eq!(
        detail(&session, HarnessActorSlot::Primary)
            .await
            .evidence
            .availability,
        DetailAvailability::Unavailable
    );
    assert_eq!(
        session
            .store
            .draft(PRIMARY_ACCOUNT, SUBJECT_ID)
            .await
            .unwrap(),
        Some(draft)
    );
    assert_eq!(
        session
            .control
            .status(&run.nonce)
            .await
            .unwrap()
            .committed_phase,
        None
    );
}

#[tokio::test]
async fn lease_expiry_uses_actual_native_generation_and_stops_automatic_dispatch() {
    let run = Run::new();
    let session = prepared(&run).await;
    let (activity, lease) = interest(&session, "test-main", HarnessActorSlot::Primary).await;
    action(&session, HarnessCoreAction::AdvanceLeaseExpiry).await;
    assert_eq!(
        session
            .control
            .status(&run.nonce)
            .await
            .unwrap()
            .demand_lease_count,
        0
    );
    assert!(!session.runtime.harness_run_next().await);
    assert_eq!(
        session
            .runtime
            .renew_demand(
                "test-main",
                RenewDemandRequest {
                    owner_generation: activity.generation,
                    leases: vec![DemandLeaseRenewal {
                        lease_id: lease.lease_id,
                        account_id: PRIMARY_ACCOUNT.into(),
                        authorization_epoch: "1".into()
                    }]
                }
            )
            .await
            .unwrap_err()
            .code,
        ErrorCode::StaleView
    );
    interest(&session, "test-main", HarnessActorSlot::Primary).await;
    assert!(session.runtime.harness_run_next().await);
    assert_eq!(
        session
            .control
            .status(&run.nonce)
            .await
            .unwrap()
            .provider_call_count,
        "1"
    );
}

#[tokio::test]
async fn actual_conditional_response_preserves_values_and_advances_facet_revision() {
    let run = Run::new();
    let session = prepared(&run).await;
    interest(&session, "test-main", HarnessActorSlot::Primary).await;
    assert!(session.runtime.harness_run_next().await);
    let first = detail(&session, HarnessActorSlot::Primary).await;
    action(&session, HarnessCoreAction::PhaseNotModified).await;
    action(&session, HarnessCoreAction::AdvanceRefresh).await;
    assert!(session.runtime.harness_run_next().await);
    let second = detail(&session, HarnessActorSlot::Primary).await;
    assert_eq!(second.body, first.body);
    assert_eq!(
        second.metadata.as_ref().unwrap().values,
        first.metadata.unwrap().values
    );
    assert_ne!(
        second.evidence.facet_revision,
        first.evidence.facet_revision
    );
}

#[tokio::test]
async fn denied_and_offline_phases_use_real_access_and_preserve_authored_text() {
    for phase in [
        HarnessCoreAction::PhaseDenied,
        HarnessCoreAction::PhaseOffline,
    ] {
        let run = Run::new();
        let session = prepared(&run).await;
        interest(&session, "test-main", HarnessActorSlot::Primary).await;
        assert!(session.runtime.harness_run_next().await);
        let first = detail(&session, HarnessActorSlot::Primary).await;
        let draft = session
            .store
            .draft(PRIMARY_ACCOUNT, SUBJECT_ID)
            .await
            .unwrap();
        action(&session, phase).await;
        action(&session, HarnessCoreAction::AdvanceRefresh).await;
        assert!(session.runtime.harness_run_next().await);
        let snapshot = detail(&session, HarnessActorSlot::Primary).await;
        if phase == HarnessCoreAction::PhaseDenied {
            assert_eq!(
                snapshot.evidence.availability,
                DetailAvailability::Unavailable
            );
            assert_eq!(snapshot.body.text, None);
        } else {
            assert_eq!(snapshot.body, first.body);
            assert_eq!(snapshot.evidence.sync.state, SyncState::Offline);
        }
        assert_eq!(
            session
                .store
                .draft(PRIMARY_ACCOUNT, SUBJECT_ID)
                .await
                .unwrap(),
            draft
        );
    }
}

#[tokio::test]
async fn real_cas_writes_qualify_page_boundary_and_retention_reset() {
    let run = Run::new();
    let session = prepared(&run).await;
    let baseline = session.store.revision().await.unwrap();
    let original = session
        .store
        .draft(PRIMARY_ACCOUNT, SUBJECT_ID)
        .await
        .unwrap();
    action(&session, HarnessCoreAction::FillCatchup).await;
    let first = session.store.changes_since(&baseline).await.unwrap();
    assert!(!first.reset_required);
    assert_eq!(first.changes.len(), 256);
    assert!(first.has_more);
    assert!(
        first
            .changes
            .iter()
            .all(|change| change.account_id == ALTERNATE_ACCOUNT)
    );
    let second = session.store.changes_since(&first.revision).await.unwrap();
    assert_eq!(second.changes.len(), 44);
    assert!(!second.has_more);
    let floor = second.revision;
    action(&session, HarnessCoreAction::FillRetention).await;
    assert!(
        session
            .store
            .changes_since(&floor)
            .await
            .unwrap()
            .reset_required
    );
    assert_eq!(
        session
            .store
            .draft(ALTERNATE_ACCOUNT, SUBJECT_ID)
            .await
            .unwrap()
            .unwrap()
            .generation,
        "4401"
    );
    assert_eq!(
        session
            .store
            .draft(PRIMARY_ACCOUNT, SUBJECT_ID)
            .await
            .unwrap(),
        original
    );
    assert_eq!(
        session
            .control
            .status(&run.nonce)
            .await
            .unwrap()
            .provider_call_count,
        "0"
    );
}

#[tokio::test]
async fn wrong_nonce_generation_gate_and_retired_native_guard_have_no_mutations() {
    let run = Run::new();
    let session = prepared(&run).await;
    let initial = session.control.status(&run.nonce).await.unwrap();
    for (nonce, generation, gate) in [
        ("wrong", initial.scenario_generation.as_str(), None),
        (run.nonce.as_str(), "1", None),
        (
            run.nonce.as_str(),
            initial.scenario_generation.as_str(),
            Some("unknown"),
        ),
    ] {
        let request = HarnessCoreRequest {
            run_nonce: nonce.into(),
            expected_generation: generation.into(),
            action: if gate.is_some() {
                HarnessCoreAction::ReleaseProviderGate
            } else {
                HarnessCoreAction::FillCatchup
            },
            gate_id: gate.map(str::to_owned),
        };
        assert!(session.control.execute(request).await.is_err());
    }
    let request = HarnessCoreRequest {
        run_nonce: run.nonce.clone(),
        expected_generation: initial.scenario_generation.clone(),
        action: HarnessCoreAction::FillCatchup,
        gate_id: None,
    };
    assert_eq!(
        session
            .control
            .execute_checked(request, || Err(CollaborationError::new(
                ErrorCode::PermissionDenied,
                "Retired fixture caller"
            )))
            .await
            .unwrap_err()
            .code,
        ErrorCode::PermissionDenied
    );
    let final_status = session.control.status(&run.nonce).await.unwrap();
    assert_eq!(final_status.revision, initial.revision);
    assert_eq!(
        final_status.scenario_generation,
        initial.scenario_generation
    );
    let first = action(&session, HarnessCoreAction::ArmProviderGate)
        .await
        .gate_id
        .unwrap();
    let second = action(&session, HarnessCoreAction::ArmProviderGate)
        .await
        .gate_id
        .unwrap();
    assert_ne!(first, second);
    assert_eq!(
        session
            .control
            .execute(HarnessCoreRequest {
                run_nonce: run.nonce.clone(),
                expected_generation: final_status.scenario_generation,
                action: HarnessCoreAction::ArmProviderGate,
                gate_id: None
            })
            .await
            .unwrap_err()
            .code,
        ErrorCode::Busy
    );
    action(&session, HarnessCoreAction::PhaseTwo).await;
    assert_eq!(
        session
            .control
            .status(&run.nonce)
            .await
            .unwrap()
            .gates
            .iter()
            .filter(|gate| gate.state == HarnessGateState::Cancelled)
            .count(),
        2
    );
    assert!(
        session
            .control
            .execute(HarnessCoreRequest {
                run_nonce: run.nonce.clone(),
                expected_generation: "3".into(),
                action: HarnessCoreAction::ReleaseProviderGate,
                gate_id: Some(first)
            })
            .await
            .is_err()
    );
}

#[tokio::test]
async fn cancelling_native_wait_retires_the_gate_without_holding_runtime_dispatch() {
    let run = Run::new();
    let session = prepared(&run).await;
    let gate = action(&session, HarnessCoreAction::ArmProviderGate)
        .await
        .gate_id
        .unwrap();
    interest(&session, "test-main", HarnessActorSlot::Primary).await;
    let runtime = session.runtime.clone();
    let worker = tokio::spawn(async move { runtime.harness_run_next().await });
    held(&session, &gate).await;
    worker.abort();
    assert!(worker.await.unwrap_err().is_cancelled());
    let status = session.control.status(&run.nonce).await.unwrap();
    assert!(
        status
            .gates
            .iter()
            .any(|gate| gate.state == HarnessGateState::Cancelled)
    );
    assert_eq!(
        status.calls.last().unwrap().state,
        HarnessCallState::Cancelled
    );
    assert_eq!(status.committed_phase, None);
}

#[tokio::test]
async fn durable_vault_and_real_quota_survive_cold_session_without_ephemeral_interest() {
    let run = Run::new();
    let session = prepared(&run).await;
    let original_session = session.control.status(&run.nonce).await.unwrap().session_id;
    let draft = session
        .store
        .draft(PRIMARY_ACCOUNT, SUBJECT_ID)
        .await
        .unwrap();
    action(&session, HarnessCoreAction::PhaseRateLimited).await;
    interest(&session, "test-main", HarnessActorSlot::Primary).await;
    assert!(session.runtime.harness_run_next().await);
    let barrier = session
        .store
        .scope_state(PRIMARY_ACCOUNT, "provider:rest")
        .await
        .unwrap()
        .unwrap()
        .sync
        .next_retry_at;
    assert!(barrier.is_some());
    session.store.close().await;
    drop(session);
    let session = run.open().await;
    session.runtime.recover_credentials().await.unwrap();
    assert_eq!(
        session.store.account(PRIMARY_ACCOUNT).await.unwrap().state,
        AccountState::Active
    );
    let status = session.control.status(&run.nonce).await.unwrap();
    assert_ne!(status.session_id, original_session);
    assert_eq!(status.demand_lease_count, 0);
    assert_eq!(status.durable_detail_requests, 0);
    assert_eq!(
        session
            .store
            .draft(PRIMARY_ACCOUNT, SUBJECT_ID)
            .await
            .unwrap(),
        draft
    );
    assert_eq!(
        session
            .store
            .scope_state(PRIMARY_ACCOUNT, "provider:rest")
            .await
            .unwrap()
            .unwrap()
            .sync
            .next_retry_at,
        barrier
    );
    interest(&session, "test-new-document", HarnessActorSlot::Primary).await;
    assert!(!session.runtime.harness_run_next().await);
    assert_eq!(
        session
            .control
            .status(&run.nonce)
            .await
            .unwrap()
            .provider_call_count,
        "0"
    );
    action(&session, HarnessCoreAction::PhaseOne).await;
    action(&session, HarnessCoreAction::AdvanceCooldown).await;
    interest(&session, "test-new-document", HarnessActorSlot::Primary).await;
    assert!(session.runtime.harness_run_next().await);
    assert_eq!(
        session
            .control
            .status(&run.nonce)
            .await
            .unwrap()
            .committed_phase,
        Some(HarnessPhase::One)
    );
}

#[tokio::test]
async fn repository_only_profile_never_admits_body_and_vault_rejects_other_tokens_refs() {
    let run = Run::new();
    let session = run.open().await;
    action(&session, HarnessCoreAction::PrepareRepositoryOnly).await;
    assert_eq!(
        session
            .runtime
            .capabilities(PRIMARY_ACCOUNT)
            .await
            .unwrap()
            .facets
            .iter()
            .find(|facet| facet.facet == ResourceFacet::PullDetails)
            .unwrap()
            .state,
        CapabilityState::Unsupported
    );
    assert_eq!(
        session
            .runtime
            .hydrate_detail(HydrateDetailRequest {
                account_id: PRIMARY_ACCOUNT.into(),
                authorization_epoch: "1".into(),
                subject_id: SUBJECT_ID.into(),
                facet: DetailFacet::Body
            })
            .await
            .unwrap_err()
            .code,
        ErrorCode::Unsupported
    );
    assert_eq!(
        session
            .control
            .0
            .vault
            .load("foreign-reference")
            .unwrap_err(),
        CredentialError::InvalidToken
    );
    assert_eq!(
        session.control.0.vault.store(
            session.control.0.vault.reference(HarnessActorSlot::Primary),
            &SecretToken::new("unknown-synthetic-token".into()).unwrap()
        ),
        Err(CredentialError::InvalidToken)
    );
    assert_eq!(
        session.control.0.vault.store(
            session.control.0.vault.reference(HarnessActorSlot::Primary),
            &vault::token(HarnessActorSlot::Alternate)
        ),
        Err(CredentialError::InvalidToken)
    );
    assert_eq!(
        session
            .control
            .status(&run.nonce)
            .await
            .unwrap()
            .provider_call_count,
        "0"
    );
}

#[tokio::test]
async fn invalid_marker_and_known_symlinks_fail_before_creating_the_cache() {
    let run = Run::new();
    fs::write(run.root.join("run.json"), b"{}").unwrap();
    assert!(
        HarnessSession::open(&run.root, &run.nonce, Arc::new(|_| true))
            .await
            .is_err()
    );
    assert!(!run.root.join("collaboration.sqlite").exists());
    assert!(!run.root.join("harness-state.json").exists());
    let run = Run::new();
    assert!(
        HarnessSession::open(&run.root, "not-a-run-uuid", Arc::new(|_| true))
            .await
            .is_err()
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::symlink;
        let outside = tempfile::tempdir().unwrap();
        symlink(outside.path(), run.root.join("vault")).unwrap();
        assert!(
            HarnessSession::open(&run.root, &run.nonce, Arc::new(|_| true))
                .await
                .is_err()
        );
        assert!(!run.root.join("collaboration.sqlite").exists());
        assert!(fs::read_dir(outside.path()).unwrap().next().is_none());
    }
    // Keep the fixture directory owned for the entire assertions.
    assert!(run.directory.path().exists());
}

#[tokio::test]
async fn independent_native_visibility_loss_retires_interest_before_vault_or_provider_dispatch() {
    use std::sync::atomic::{AtomicBool, Ordering};

    let run = Run::new();
    let visible = Arc::new(AtomicBool::new(true));
    let probe = visible.clone();
    let session = HarnessSession::open(
        &run.root,
        &run.nonce,
        Arc::new(move |_| probe.load(Ordering::SeqCst)),
    )
    .await
    .unwrap();
    action(&session, HarnessCoreAction::PreparePrimary).await;
    let (activity, lease) = interest(&session, "test-main", HarnessActorSlot::Primary).await;
    let before = session.control.status(&run.nonce).await.unwrap();
    visible.store(false, Ordering::SeqCst);

    assert!(!session.runtime.harness_run_next().await);
    let after = session.control.status(&run.nonce).await.unwrap();
    assert_eq!(after.provider_call_count, before.provider_call_count);
    assert_eq!(after.vault_load_count, before.vault_load_count);
    assert_eq!(after.demand_lease_count, 0);
    assert_eq!(after.durable_detail_requests, 0);
    assert_eq!(
        session
            .runtime
            .renew_demand(
                "test-main",
                RenewDemandRequest {
                    owner_generation: activity.generation,
                    leases: vec![DemandLeaseRenewal {
                        lease_id: lease.lease_id,
                        account_id: PRIMARY_ACCOUNT.into(),
                        authorization_epoch: "1".into(),
                    }],
                },
            )
            .await
            .unwrap_err()
            .code,
        ErrorCode::StaleView
    );
    assert_eq!(
        detail(&session, HarnessActorSlot::Primary).await.body.state,
        DetailValueState::NotLoaded
    );
}

#[tokio::test]
async fn independent_phase_change_cancels_held_observation_without_committing_the_previous_phase() {
    let run = Run::new();
    let session = prepared(&run).await;
    let draft = session
        .store
        .draft(PRIMARY_ACCOUNT, SUBJECT_ID)
        .await
        .unwrap();
    let gate = action(&session, HarnessCoreAction::ArmProviderGate)
        .await
        .gate_id
        .unwrap();
    interest(&session, "test-main", HarnessActorSlot::Primary).await;
    let runtime = session.runtime.clone();
    let worker = tokio::spawn(async move { runtime.harness_run_next().await });
    held(&session, &gate).await;
    action(&session, HarnessCoreAction::PhaseTwo).await;
    assert!(
        tokio::time::timeout(Duration::from_secs(5), worker)
            .await
            .expect("phase transition releases the old retained provider wait")
            .unwrap()
    );
    let status = session.control.status(&run.nonce).await.unwrap();
    assert_eq!(status.committed_phase, None);
    assert_eq!(status.calls.last().unwrap().phase, HarnessPhase::One);
    assert_eq!(
        status.calls.last().unwrap().state,
        HarnessCallState::Cancelled
    );
    assert_eq!(
        detail(&session, HarnessActorSlot::Primary).await.body.state,
        DetailValueState::NotLoaded
    );
    assert_eq!(
        session
            .store
            .draft(PRIMARY_ACCOUNT, SUBJECT_ID)
            .await
            .unwrap(),
        draft
    );
    assert_eq!(status.durable_detail_requests, 0);
}

#[tokio::test]
async fn independent_cold_reopen_retains_committed_local_phase_without_automatic_interest() {
    let run = Run::new();
    let session = prepared(&run).await;
    interest(&session, "test-main", HarnessActorSlot::Primary).await;
    assert!(session.runtime.harness_run_next().await);
    let before = detail(&session, HarnessActorSlot::Primary).await;
    let draft = session
        .store
        .draft(PRIMARY_ACCOUNT, SUBJECT_ID)
        .await
        .unwrap();
    let account = session.store.account(PRIMARY_ACCOUNT).await.unwrap();
    let reference = session
        .store
        .credential_reference(PRIMARY_ACCOUNT)
        .await
        .unwrap();
    let session_id = session.control.status(&run.nonce).await.unwrap().session_id;
    session.store.close().await;
    drop(session);

    let session = run.open().await;
    session.runtime.recover_credentials().await.unwrap();
    let after = detail(&session, HarnessActorSlot::Primary).await;
    assert_eq!(after.body, before.body);
    assert_eq!(after.metadata, before.metadata);
    assert_eq!(
        after.evidence.facet_revision,
        before.evidence.facet_revision
    );
    assert_eq!(
        session.store.account(PRIMARY_ACCOUNT).await.unwrap(),
        account
    );
    assert_eq!(
        session
            .store
            .credential_reference(PRIMARY_ACCOUNT)
            .await
            .unwrap(),
        reference
    );
    assert_eq!(
        session
            .store
            .draft(PRIMARY_ACCOUNT, SUBJECT_ID)
            .await
            .unwrap(),
        draft
    );
    assert!(!session.runtime.harness_run_next().await);
    let status = session.control.status(&run.nonce).await.unwrap();
    assert_ne!(status.session_id, session_id);
    assert_eq!(status.provider_call_count, "0");
    assert_eq!(status.demand_lease_count, 0);
    assert_eq!(status.durable_detail_requests, 0);
    assert_eq!(status.committed_phase, Some(HarnessPhase::One));
    assert_eq!(
        status.committed_facet_revision,
        before.evidence.facet_revision
    );
}

#[tokio::test]
async fn independent_cold_reopen_reads_exact_pull_commits_without_provider_or_vault_access() {
    let run = Run::new();
    let session = prepared(&run).await;
    session
        .runtime
        .hydrate_detail(HydrateDetailRequest {
            account_id: PRIMARY_ACCOUNT.into(),
            authorization_epoch: "1".into(),
            subject_id: SUBJECT_ID.into(),
            facet: DetailFacet::Body,
        })
        .await
        .unwrap();
    assert!(session.runtime.harness_run_next().await);
    session
        .runtime
        .hydrate_detail(HydrateDetailRequest {
            account_id: PRIMARY_ACCOUNT.into(),
            authorization_epoch: "1".into(),
            subject_id: SUBJECT_ID.into(),
            facet: DetailFacet::Commits,
        })
        .await
        .unwrap();
    assert!(session.runtime.harness_run_next().await);

    let before = pull_commits(&session).await;
    assert_fixture_pull_commits(&before);
    let before_status = session.control.status(&run.nonce).await.unwrap();
    assert_eq!(before_status.provider_call_count, "3");
    assert_eq!(before_status.vault_load_count, "2");
    assert_eq!(before_status.durable_detail_requests, 0);
    let original_session = before_status.session_id;
    session.store.close().await;
    drop(session);

    let session = run.open().await;
    let reopened = session.control.status(&run.nonce).await.unwrap();
    assert_ne!(reopened.session_id, original_session);
    assert_eq!(reopened.provider_call_count, "0");
    assert_eq!(reopened.vault_load_count, "0");
    assert_eq!(reopened.demand_lease_count, 0);
    assert_eq!(reopened.durable_detail_requests, 0);

    let after = pull_commits(&session).await;
    assert_fixture_pull_commits(&after);
    assert_eq!(after.context, before.context);
    assert_eq!(after.commits, before.commits);
    assert_eq!(after.facet_revision, before.facet_revision);
    assert_eq!(after.authorization_view, before.authorization_view);
    assert_eq!(after.revision, before.revision);
    let queried = session.control.status(&run.nonce).await.unwrap();
    assert_eq!(queried.provider_call_count, "0");
    assert_eq!(queried.vault_load_count, "0");
    assert!(!session.runtime.harness_run_next().await);
}

#[tokio::test]
async fn independent_interrupted_preparation_preserves_committed_account_and_refuses_reset() {
    use std::sync::atomic::{AtomicU32, Ordering};

    let run = Run::new();
    let session = run.open().await;
    let guards = AtomicU32::new(0);
    let status = session.control.status(&run.nonce).await.unwrap();
    let error = session
        .control
        .execute_checked(
            HarnessCoreRequest {
                run_nonce: run.nonce.clone(),
                expected_generation: status.scenario_generation,
                action: HarnessCoreAction::PreparePrimary,
                gate_id: None,
            },
            || {
                if guards.fetch_add(1, Ordering::SeqCst) >= 3 {
                    Err(CollaborationError::new(
                        ErrorCode::PermissionDenied,
                        "Retired fixture caller",
                    ))
                } else {
                    Ok(())
                }
            },
        )
        .await
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::PermissionDenied);
    let account = session.store.account(PRIMARY_ACCOUNT).await.unwrap();
    let reference = session
        .store
        .credential_reference(PRIMARY_ACCOUNT)
        .await
        .unwrap();
    assert!(reference.is_some());
    assert!(!session.control.status(&run.nonce).await.unwrap().prepared);
    let revision = session.store.revision().await.unwrap();
    session.store.close().await;
    drop(session);

    let reopened = HarnessSession::open(&run.root, &run.nonce, Arc::new(|_| true)).await;
    assert_eq!(
        reopened
            .err()
            .expect("partial preparation must not be reset")
            .code,
        ErrorCode::NotReady
    );
    let store = Store::open(run.root.join("collaboration.sqlite"))
        .await
        .unwrap();
    assert_eq!(store.account(PRIMARY_ACCOUNT).await.unwrap(), account);
    assert_eq!(
        store.credential_reference(PRIMARY_ACCOUNT).await.unwrap(),
        reference
    );
    assert_eq!(store.revision().await.unwrap(), revision);
    store.close().await;
}

#[tokio::test]
async fn independent_lost_marker_refuses_reopen_without_resetting_authored_cache() {
    let run = Run::new();
    let session = prepared(&run).await;
    let draft = session
        .store
        .draft(PRIMARY_ACCOUNT, SUBJECT_ID)
        .await
        .unwrap();
    let revision = session.store.revision().await.unwrap();
    session.store.close().await;
    drop(session);
    fs::remove_file(run.root.join("harness-state.json")).unwrap();

    assert!(
        HarnessSession::open(&run.root, &run.nonce, Arc::new(|_| true))
            .await
            .is_err()
    );
    assert!(!run.root.join("harness-state.json").exists());
    let store = Store::open(run.root.join("collaboration.sqlite"))
        .await
        .unwrap();
    assert_eq!(
        store.draft(PRIMARY_ACCOUNT, SUBJECT_ID).await.unwrap(),
        draft
    );
    assert_eq!(store.revision().await.unwrap(), revision);
    store.close().await;
}
