//! Public Store controls: synthetic observations, no HTTP, vault or renderer authority.
use collaboration::*;
mod detail_support;
use detail_support::*;

const FIRST: &str = "2099-01-01T00:00:00Z";
const SECOND: &str = "2099-01-01T00:01:00Z";
const THIRD: &str = "2099-01-01T00:02:00Z";
const ACTOR: &str = "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa";

fn fields() -> Vec<DetailField> {
    vec![
        DetailField::ParticipantLogin,
        DetailField::ParticipantDisplayName,
        DetailField::ParticipantRole,
        DetailField::ParticipantApproved,
        DetailField::ParticipantState,
        DetailField::ParticipantParticipatedAt,
    ]
}

fn participant() -> DetailEntry {
    DetailEntry {
        id: format!("participant:repo:67:{ACTOR}"),
        provider_id: format!("repo:67:{ACTOR}"),
        author: None,
        title: None,
        state: None,
        body: DetailValue::default(),
        observed_body_state: DetailValueState::NotLoaded,
        updated_at: None,
        head_oid: None,
        native: Some(NativeDetailPayload::ParticipantV1(ParticipantV1 {
            user: ParticipantUser {
                provider_id: ACTOR.into(),
                login: Some("same-name".into()),
                display_name: Some("Saved display".into()),
            },
            role: Some("REVIEWER".into()),
            approved: Some(true),
            state: Some("approved".into()),
            participated_at: Some("2026-10-03T12:00:00Z".into()),
        })),
        field_mask: fields(),
        field_validations: vec![],
    }
}

fn native(entry: &DetailEntry) -> &ParticipantV1 {
    let Some(NativeDetailPayload::ParticipantV1(value)) = &entry.native else {
        panic!("typed participant payload");
    };
    value
}
fn native_mut(entry: &mut DetailEntry) -> &mut ParticipantV1 {
    let Some(NativeDetailPayload::ParticipantV1(value)) = &mut entry.native else {
        panic!("typed participant payload");
    };
    value
}
async fn fixture() -> (tempfile::TempDir, Store, RemoteAccount, LocalDraft) {
    let directory = tempfile::tempdir().unwrap();
    let store = Store::open(directory.path().join("participants.sqlite"))
        .await
        .unwrap();
    let actor = seed(&store, "a").await;
    let draft = store
        .save_draft(LocalDraft {
            account_id: "a".into(),
            subject_id: "pull".into(),
            body: "Only the user can change this draft".into(),
            generation: "0".into(),
        })
        .await
        .unwrap();
    (directory, store, actor, draft)
}
async fn binding(store: &Store) -> DetailSubjectBinding {
    let item = store.detail_subject("a", "pull").await.unwrap();
    let repo = store.repository("a", "repo").await.unwrap();
    DetailSubjectBinding {
        repository_id: repo.id,
        repository_provider_id: repo.provider_id,
        provider_id: item.provider_id,
        number: item.number,
        kind: item.kind,
        head_oid: item.head_oid,
    }
}
async fn page(
    store: &Store,
    actor: &RemoteAccount,
    entries: Vec<DetailEntry>,
    at: &str,
) -> DetailCommit {
    let mut page = commit(store, actor, DetailFacet::Participants).await;
    page.subject_binding = Some(binding(store).await);
    page.source = DetailSource {
        source: "synthetic.participants.v1".into(),
        adapter_version: 1,
        field_mask: fields(),
        provider_updated_at: None,
        observed_at: at.into(),
    };
    page.etag = None;
    page.entries = entries;
    page
}
async fn read(store: &Store) -> DetailSnapshot {
    store
        .detail(query("a", DetailFacet::Participants))
        .await
        .unwrap()
}
fn validation(entry: &DetailEntry, field: DetailField) -> &DetailFieldValidation {
    entry
        .field_validations
        .iter()
        .find(|value| value.field == field)
        .unwrap()
}

#[test]
fn legacy_entries_and_adjacent_native_false_null_serialize_without_losing_shape() {
    let mut legacy = serde_json::to_value(entry("legacy")).unwrap();
    legacy.as_object_mut().unwrap().remove("native");
    let decoded: DetailEntry = serde_json::from_value(legacy).unwrap();
    assert_eq!(decoded.native, None);
    assert_eq!(
        serde_json::to_value(decoded).unwrap()["native"],
        serde_json::Value::Null
    );
    let mut record = participant();
    native_mut(&mut record).approved = Some(false);
    native_mut(&mut record).state = None;
    let json = serde_json::to_value(&record).unwrap();
    assert_eq!(json["native"]["kind"], "participant.v1");
    assert_eq!(json["native"]["value"]["approved"], false);
    assert_eq!(json["native"]["value"]["state"], serde_json::Value::Null);
    assert_eq!(serde_json::from_value::<DetailEntry>(json).unwrap(), record);
    assert!(
        serde_json::from_value::<NativeDetailPayload>(serde_json::json!({
            "kind": "participant.v2", "value": {}
        }))
        .is_err()
    );
}

#[tokio::test]
async fn native_fields_merge_independently_and_cold_reopen_preserves_exact_saved_authority() {
    let (directory, store, actor, draft) = fixture().await;
    store
        .apply_detail(page(&store, &actor, vec![participant()], FIRST).await)
        .await
        .unwrap();
    let initial = read(&store).await;
    let initial_entry = initial.entries[0].clone();

    let mut partial = participant();
    partial.field_mask = vec![
        DetailField::ParticipantApproved,
        DetailField::ParticipantState,
    ];
    native_mut(&mut partial).approved = Some(false);
    native_mut(&mut partial).state = None;
    native_mut(&mut partial).user.login = None;
    native_mut(&mut partial).user.display_name = None;
    native_mut(&mut partial).role = None;
    native_mut(&mut partial).participated_at = None;
    store
        .apply_detail(page(&store, &actor, vec![partial], SECOND).await)
        .await
        .unwrap();
    let saved = read(&store).await;
    let record = &saved.entries[0];
    assert_eq!(native(record).approved, Some(false));
    assert_eq!(native(record).state, None);
    assert_eq!(native(record).user.login, native(&initial_entry).user.login);
    assert_eq!(native(record).role, native(&initial_entry).role);
    for field in [
        DetailField::ParticipantLogin,
        DetailField::ParticipantDisplayName,
        DetailField::ParticipantRole,
        DetailField::ParticipantParticipatedAt,
    ] {
        assert_eq!(validation(record, field), validation(&initial_entry, field));
    }
    assert_eq!(
        validation(record, DetailField::ParticipantApproved).validated_at,
        SECOND
    );
    assert_eq!(
        validation(record, DetailField::ParticipantState).validated_at,
        SECOND
    );
    assert_eq!(saved.evidence.coverage.state, CoverageState::Complete);
    assert_eq!(
        saved
            .evidence
            .value_source
            .as_ref()
            .unwrap()
            .provider_updated_at,
        None
    );

    let mut cleared = participant();
    native_mut(&mut cleared).user.login = None;
    native_mut(&mut cleared).user.display_name = None;
    native_mut(&mut cleared).role = Some("FUTURE_PROVIDER_ROLE".into());
    native_mut(&mut cleared).state = Some("future-native-state".into());
    native_mut(&mut cleared).approved = Some(false);
    // An older action timestamp is a newly observed value, never an update clock.
    native_mut(&mut cleared).participated_at = Some("2020-01-01T00:00:00Z".into());
    store
        .apply_detail(page(&store, &actor, vec![cleared], THIRD).await)
        .await
        .unwrap();
    let saved = read(&store).await;
    assert_eq!(native(&saved.entries[0]).user.login, None);
    assert_eq!(
        native(&saved.entries[0]).role.as_deref(),
        Some("FUTURE_PROVIDER_ROLE")
    );
    assert_eq!(
        native(&saved.entries[0]).participated_at.as_deref(),
        Some("2020-01-01T00:00:00Z")
    );
    assert!(
        saved.entries[0]
            .field_validations
            .iter()
            .all(|field| field.validated_at == THIRD)
    );
    assert_eq!(store.draft("a", "pull").await.unwrap(), Some(draft.clone()));
    store.close().await;
    drop(store);
    let reopened = Store::open(directory.path().join("participants.sqlite"))
        .await
        .unwrap();
    assert_eq!(read(&reopened).await, saved);
    assert_eq!(
        reopened.draft("a", "pull").await.unwrap(),
        Some(draft.clone())
    );
    let empty = page(&reopened, &actor, vec![], THIRD).await;
    reopened.apply_detail(empty).await.unwrap();
    let empty = read(&reopened).await;
    assert!(empty.entries.is_empty());
    assert_eq!(empty.evidence.saved_empty, Some(true));
    assert_eq!(reopened.draft("a", "pull").await.unwrap(), Some(draft));
}

#[tokio::test]
async fn invalid_facet_payload_masks_identity_and_native_values_roll_back_all_saved_content() {
    let (_directory, store, actor, draft) = fixture().await;
    store
        .apply_detail(page(&store, &actor, vec![participant()], FIRST).await)
        .await
        .unwrap();
    for invalid in 0..19 {
        let mut observation = page(&store, &actor, vec![participant()], SECOND).await;
        match invalid {
            0 => observation.entries[0].native = None,
            1 => observation.entries[0].field_mask = vec![DetailField::Author],
            2 => observation.source.field_mask[0] = DetailField::Author,
            3 => observation.entries[0].author = Some("overloaded actor".into()),
            4 => native_mut(&mut observation.entries[0]).approved = None,
            5 => native_mut(&mut observation.entries[0]).role = None,
            6 => native_mut(&mut observation.entries[0]).state = Some("bad\nstate".into()),
            7 => native_mut(&mut observation.entries[0]).participated_at = Some("yesterday".into()),
            8 => native_mut(&mut observation.entries[0]).user.login = Some("x".repeat(256)),
            9 => {
                native_mut(&mut observation.entries[0]).user.provider_id = "different-actor".into()
            }
            10 => observation.entries[0].provider_id = "changed-provider-entry".into(),
            11 => observation.source.provider_updated_at = Some("2026-10-03T00:00:00Z".into()),
            12 => observation.subject_binding = None,
            13 => observation.entries.push(observation.entries[0].clone()),
            14 => observation.reconciliation.head_scope = DetailHeadScope::CurrentHead,
            15 => {
                observation.entries[0].field_validations = vec![DetailFieldValidation {
                    field: DetailField::ParticipantApproved,
                    source: "renderer".into(),
                    adapter_version: 1,
                    validated_at: SECOND.into(),
                }]
            }
            16 => observation.entries[0]
                .field_mask
                .push(DetailField::ParticipantApproved),
            17 => observation.source.field_mask[0] = DetailField::ParticipantApproved,
            18 => observation.entries = vec![participant(); 101],
            _ => unreachable!(),
        }
        let before = read(&store).await;
        let revision = store.revision().await.unwrap();
        assert_eq!(
            store.apply_detail(observation).await.unwrap_err().code,
            ErrorCode::InvalidInput,
            "invalid native observation {invalid}"
        );
        assert_eq!(
            read(&store).await,
            before,
            "invalid native observation {invalid}"
        );
        assert_eq!(store.revision().await.unwrap(), revision);
        assert_eq!(store.draft("a", "pull").await.unwrap(), Some(draft.clone()));
    }
    for facet in [
        DetailFacet::Comments,
        DetailFacet::Reviews,
        DetailFacet::Checks,
    ] {
        let mut generic = commit(&store, &actor, facet).await;
        generic.entries = vec![participant()];
        assert_eq!(
            store.apply_detail(generic).await.unwrap_err().code,
            ErrorCode::InvalidInput
        );
        let mut generic = commit(&store, &actor, facet).await;
        generic.entries = vec![entry("generic")];
        generic.entries[0].field_mask = vec![DetailField::ParticipantApproved];
        assert_eq!(
            store.apply_detail(generic).await.unwrap_err().code,
            ErrorCode::InvalidInput
        );
    }
}

async fn head(store: &Store, actor: &RemoteAccount, head: &str) {
    let mut item = store.detail_subject("a", "pull").await.unwrap();
    item.head_oid = Some(head.into());
    item.updated_at = "2026-10-04T00:00:00Z".into();
    let run = store
        .begin_sync("a", &actor.authorization_epoch, "repo:repo:pull_request")
        .await
        .unwrap();
    store
        .apply_page(PageCommit {
            account_id: "a".into(),
            authorization_epoch: actor.authorization_epoch.clone(),
            scope: "repo:repo:pull_request".into(),
            run_id: run,
            repositories: vec![],
            items: vec![item],
            endpoint_aliases: vec![],
            next_cursor: None,
            etag: None,
            last_modified: None,
            not_modified: false,
            complete: true,
            observed_at: SECOND.into(),
        })
        .await
        .unwrap();
}

#[tokio::test]
async fn saved_subject_history_survives_new_head_but_held_old_binding_cannot_apply() {
    let (_directory, store, actor, draft) = fixture().await;
    head(&store, &actor, "old-head").await;
    store
        .apply_detail(page(&store, &actor, vec![participant()], FIRST).await)
        .await
        .unwrap();
    let saved = read(&store).await;
    let old = page(&store, &actor, vec![], SECOND).await;
    let held = read(&store).await;
    head(&store, &actor, "new-head").await;
    let current = read(&store).await;
    assert_eq!(current.entries, saved.entries);
    assert_eq!(current.evidence, held.evidence);
    assert_eq!(
        store.apply_detail(old).await.unwrap_err().code,
        ErrorCode::StaleView
    );
    assert_eq!(read(&store).await, current);
    assert_eq!(store.draft("a", "pull").await.unwrap(), Some(draft));
}

#[tokio::test]
async fn deselect_reselect_retires_every_old_participant_lease_without_pruning_saved_history() {
    let (_directory, store, actor, draft) = fixture().await;
    store
        .apply_detail(page(&store, &actor, vec![participant()], FIRST).await)
        .await
        .unwrap();
    let saved = read(&store).await;
    let old = page(&store, &actor, vec![], SECOND).await;
    store.select_repository("a", "repo", false).await.unwrap();
    assert_eq!(
        read(&store).await.evidence.availability,
        DetailAvailability::Unavailable
    );
    store.select_repository("a", "repo", true).await.unwrap();
    let selected = read(&store).await;
    assert_eq!(selected.entries, saved.entries);
    assert_eq!(
        store.apply_detail(old).await.unwrap_err().code,
        ErrorCode::StaleView
    );
    assert_eq!(read(&store).await, selected);
    assert_eq!(store.draft("a", "pull").await.unwrap(), Some(draft));
}

fn capability_request() -> ContextCapabilityRequest {
    ContextCapabilityRequest {
        account_id: "a".into(),
        authorization_epoch: "1".into(),
        target: CapabilityTarget {
            kind: CapabilityTargetKind::Resource,
            instance_id: Some(ProviderInstance::public(ProviderKind::Github).id),
            repository_id: None,
            resource_id: Some("pull".into()),
            resource_kind: Some(ResourceKind::PullRequest),
        },
    }
}
fn participant_profile(_: &RemoteAccount, _: &ProviderInstance) -> providers::ProviderProfile {
    let mut profile = providers::ProviderProfile::read_only(InboxSemantics::None, false);
    for capability in &mut profile.facets {
        if capability.facet == ResourceFacet::Participants {
            capability.state = CapabilityState::Supported;
            capability.reason = None;
        }
    }
    profile
}

#[tokio::test]
async fn contextual_participant_denial_is_separate_from_body_and_private_authored_data() {
    let (_directory, store, actor, draft) = fixture().await;
    store
        .apply_detail(commit(&store, &actor, DetailFacet::Body).await)
        .await
        .unwrap();
    store
        .apply_detail(page(&store, &actor, vec![participant()], FIRST).await)
        .await
        .unwrap();
    let before_body = store.detail(query("a", DetailFacet::Body)).await.unwrap();
    let capabilities = store
        .contextual_capabilities(capability_request(), participant_profile)
        .await
        .unwrap();
    let participant_capability = capabilities
        .facets
        .iter()
        .find(|facet| facet.facet == ResourceFacet::Participants)
        .unwrap();
    assert_eq!(
        participant_capability.saved_read.state,
        CapabilityState::Supported
    );
    assert_eq!(
        participant_capability.observation,
        CapabilityObservation::Complete
    );
    assert_eq!(
        participant_capability.remote_write.state,
        CapabilityState::Unsupported
    );

    store
        .set_sync_status(
            "a",
            "1",
            &DetailFacet::Participants.scope("pull"),
            SyncStatus {
                state: SyncState::Error,
                error: Some(CollaborationError::new(
                    ErrorCode::PermissionDenied,
                    "Synthetic participant-only denial",
                )),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    assert_eq!(
        read(&store).await.evidence.availability,
        DetailAvailability::Unavailable
    );
    let after_body = store.detail(query("a", DetailFacet::Body)).await.unwrap();
    assert_eq!(after_body.body, before_body.body);
    assert_eq!(after_body.metadata, before_body.metadata);
    assert_eq!(after_body.evidence, before_body.evidence);
    assert_eq!(store.draft("a", "pull").await.unwrap(), Some(draft));
    let capabilities = store
        .contextual_capabilities(capability_request(), participant_profile)
        .await
        .unwrap();
    let participant_capability = capabilities
        .facets
        .iter()
        .find(|facet| facet.facet == ResourceFacet::Participants)
        .unwrap();
    assert_eq!(
        participant_capability.saved_read.state,
        CapabilityState::Unavailable
    );
    assert_eq!(
        participant_capability.saved_read.reason,
        Some(ContextCapabilityReason::PermissionDenied)
    );
    assert!(participant_capability.can_recheck_access);
    let unsupported = store
        .contextual_capabilities(capability_request(), |_, _| {
            providers::ProviderProfile::read_only(InboxSemantics::None, false)
        })
        .await
        .unwrap();
    assert_eq!(
        unsupported
            .facets
            .iter()
            .find(|facet| facet.facet == ResourceFacet::Participants)
            .unwrap()
            .saved_read
            .state,
        CapabilityState::Unsupported
    );
    assert_eq!(
        DetailFacet::Participants.capability(&RemoteItemKind::Issue),
        None
    );
    assert_eq!(
        DetailFacet::Participants.capability(&RemoteItemKind::Notification),
        None
    );
}
