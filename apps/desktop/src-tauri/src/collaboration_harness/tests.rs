use super::*;

fn request(action: HarnessAction) -> HarnessControlRequest {
    HarnessControlRequest {
        run_nonce: uuid::Uuid::new_v4().to_string(),
        expected_generation: "1".into(),
        action,
        core_action: None,
        gate_id: None,
    }
}
#[test]
fn wrapper_rejects_irrelevant_core_and_gate_parameters() {
    let mut value = request(HarnessAction::Core);
    assert!(validate_request(&value).is_err());
    value.core_action = Some(HarnessCoreAction::CancelGates);
    assert!(validate_request(&value).is_ok());
    value.gate_id = Some(uuid::Uuid::new_v4().to_string());
    assert!(validate_request(&value).is_err());
    value.core_action = Some(HarnessCoreAction::ReleaseProviderGate);
    assert!(validate_request(&value).is_ok());
    value.action = HarnessAction::ResumeHints;
    assert!(validate_request(&value).is_err());
    value.core_action = None;
    assert!(validate_request(&value).is_err());
    value.action = HarnessAction::ReleaseLocalRead;
    assert!(validate_request(&value).is_ok());
    value.gate_id = Some("../../token".into());
    assert!(validate_request(&value).is_err());
}

struct RootFixture(PathBuf);
impl RootFixture {
    fn new(nonce: &str) -> Self {
        let root =
            std::env::temp_dir().join(format!("gitru-ruru103-app-test-{}", uuid::Uuid::new_v4()));
        let mut builder = fs::DirBuilder::new();
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            builder.mode(0o700);
        }
        builder.create(&root).unwrap();
        let root = fs::canonicalize(root).unwrap();
        fs::write(
            root.join("run.json"),
            serde_json::to_vec(
                &serde_json::json!({"version":1,"application_id":APPLICATION_ID,"run_nonce":nonce}),
            )
            .unwrap(),
        )
        .unwrap();
        Self(root)
    }
}
impl Drop for RootFixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn launch_root_fails_closed_on_identifier_marker_and_run_mismatch() {
    let nonce = uuid::Uuid::new_v4().to_string();
    let fixture = RootFixture::new(&nonce);
    assert!(LaunchRoot::open("com.ruru.gitru", fixture.0.clone(), nonce.clone()).is_err());
    assert!(LaunchRoot::open(
        APPLICATION_ID,
        fixture.0.clone(),
        uuid::Uuid::new_v4().to_string()
    )
    .is_err());
    let launch = LaunchRoot::open(APPLICATION_ID, fixture.0.clone(), nonce).unwrap();
    fs::write(fixture.0.join("run.json"), b"{}").unwrap();
    assert!(launch.check().is_err());
}
#[test]
fn durable_checkpoint_retains_previous_native_session_without_accepting_wrong_run() {
    let nonce = uuid::Uuid::new_v4().to_string();
    let fixture = RootFixture::new(&nonce);
    let launch = LaunchRoot::open(APPLICATION_ID, fixture.0.clone(), nonce.clone()).unwrap();
    assert!(launch.retained_checkpoint().unwrap().is_none());
    let mut checkpoint = HarnessCheckpoint {
        run_nonce: nonce,
        session_id: uuid::Uuid::new_v4().to_string(),
        scenario_generation: "2".into(),
        kind: HarnessCheckpointKind::CommittedBeforeHint,
        gate_id: None,
        committed_phase: Some(HarnessPhase::Two),
        committed_facet_revision: Some("9".into()),
        process_id: 12345,
    };
    launch.checkpoint(&checkpoint).unwrap();
    let retained = launch.retained_checkpoint().unwrap().unwrap();
    assert_eq!(retained.session_id, checkpoint.session_id);
    assert_eq!(retained.committed_facet_revision.as_deref(), Some("9"));
    checkpoint.run_nonce = uuid::Uuid::new_v4().to_string();
    launch.checkpoint(&checkpoint).unwrap();
    assert!(launch.retained_checkpoint().is_err());
}
#[cfg(unix)]
#[test]
fn launch_root_and_checkpoint_refuse_symlinks_and_public_roots() {
    use std::os::unix::fs::{symlink, PermissionsExt};
    let nonce = uuid::Uuid::new_v4().to_string();
    let fixture = RootFixture::new(&nonce);
    let launch = LaunchRoot::open(APPLICATION_ID, fixture.0.clone(), nonce.clone()).unwrap();
    symlink(
        fixture.0.join("run.json"),
        fixture.0.join("crash-checkpoint.json"),
    )
    .unwrap();
    assert!(launch.retained_checkpoint().is_err());
    fs::remove_file(fixture.0.join("crash-checkpoint.json")).unwrap();
    fs::set_permissions(&fixture.0, fs::Permissions::from_mode(0o755)).unwrap();
    assert!(launch.check().is_err());
    assert!(LaunchRoot::open(APPLICATION_ID, fixture.0.clone(), nonce).is_err());
}

#[test]
fn held_hint_overflow_keeps_bounded_latest_receipts_in_reversible_order() {
    let mut hints = VecDeque::new();
    for revision in 1..=MAX_HINTS + 40 {
        retain_hint(&mut hints, revision.to_string());
    }
    assert_eq!(hints.len(), MAX_HINTS);
    assert_eq!(hints.front().unwrap(), "41");
    let reversed: Vec<_> = hints.drain(..).rev().collect();
    assert_eq!(reversed.first().unwrap(), &(MAX_HINTS + 40).to_string());
    assert_eq!(reversed.last().unwrap(), "41");
    assert!(hints.is_empty());
}
#[test]
fn finite_wrapper_actions_never_accept_an_unrelated_gate_or_core_program() {
    for action in [
        HarnessAction::CreateConcurrentChild,
        HarnessAction::CloseConcurrentChild,
        HarnessAction::ReloadConcurrentChild,
        HarnessAction::HoldChildHints,
        HarnessAction::DropChildHints,
        HarnessAction::DeliverChildHintsReverse,
        HarnessAction::ResumeHints,
        HarnessAction::ArmItemRead,
        HarnessAction::ArmBodyRead,
        HarnessAction::ArmDraftRead,
        HarnessAction::CancelLocalReads,
        HarnessAction::CheckpointCommittedBeforeHint,
    ] {
        let mut value = request(action);
        assert!(validate_request(&value).is_ok());
        value.core_action = Some(HarnessCoreAction::FillRetention);
        assert!(validate_request(&value).is_err());
        value.core_action = None;
        value.gate_id = Some(uuid::Uuid::new_v4().to_string());
        assert!(validate_request(&value).is_err());
    }
    for action in [
        HarnessAction::ReleaseLocalRead,
        HarnessAction::CheckpointBeforeCommit,
    ] {
        let mut value = request(action);
        assert!(validate_request(&value).is_err());
        value.gate_id = Some(uuid::Uuid::new_v4().to_string());
        assert!(validate_request(&value).is_ok());
    }
}

#[test]
fn controller_deserialization_rejects_unmodeled_inputs() {
    let value = serde_json::json!({"run_nonce":uuid::Uuid::new_v4().to_string(),"expected_generation":"1","action":"resume_hints","core_action":null,"gate_id":null,"path":"/tmp/unowned"});
    assert!(serde_json::from_value::<HarnessControlRequest>(value).is_err());
    assert!(serde_json::from_value::<HarnessStatusRequest>(
        serde_json::json!({"run_nonce":uuid::Uuid::new_v4().to_string(),"token":"unmodeled"})
    )
    .is_err());
}

#[tokio::test]
async fn aborting_a_held_local_return_releases_the_bounded_slot_without_view_cleanup() {
    let state = Arc::new(Mutex::new(HarnessLocalReadGate {
        gate_id: uuid::Uuid::new_v4().to_string(),
        scenario_generation: "1".into(),
        webview_label: "tab-webview:ruru103:current".into(),
        kind: HarnessReadKind::Body,
        state: HarnessReadState::Held,
    }));
    let (ready_tx, ready_rx) = tokio::sync::oneshot::channel();
    let task_state = state.clone();
    let held = tokio::spawn(async move {
        let _terminal = HeldLocalRead {
            cancel: Some(|| {
                cancel_held_return(&mut task_state.lock().unwrap());
            }),
        };
        ready_tx.send(()).unwrap();
        std::future::pending::<()>().await;
    });
    ready_rx.await.unwrap();
    // Abort the admitted wait while the owner label/incarnation is unchanged.
    held.abort();
    assert!(held.await.unwrap_err().is_cancelled());
    assert_eq!(state.lock().unwrap().state, HarnessReadState::Cancelled);
    assert!(!matches!(
        state.lock().unwrap().state,
        HarnessReadState::Armed | HarnessReadState::Held
    ));
    let mut released = state.lock().unwrap();
    released.state = HarnessReadState::Released;
    assert!(!cancel_held_return(&mut released));
    assert_eq!(released.state, HarnessReadState::Released);
}

#[test]
fn corrupt_checkpoint_and_marker_are_read_with_a_hard_byte_bound() {
    let nonce = uuid::Uuid::new_v4().to_string();
    let fixture = RootFixture::new(&nonce);
    let launch = LaunchRoot::open(APPLICATION_ID, fixture.0.clone(), nonce).unwrap();
    fs::write(fixture.0.join("crash-checkpoint.json"), vec![b' '; 8192]).unwrap();
    assert!(launch.retained_checkpoint().is_err());
    assert!(launch.read_bounded("crash-checkpoint.json").is_err());
    fs::write(fixture.0.join("run.json"), vec![b' '; 8192]).unwrap();
    assert!(launch.check().is_err());
}
