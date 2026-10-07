//! Public Store qualification uses typed synthetic pages, no provider or vault.
use collaboration::*;
mod detail_support;
use detail_support::*;

const FIRST: &str = "2099-01-01T00:00:00Z";
const SECOND: &str = "2099-01-01T00:01:00Z";
const THIRD: &str = "2099-01-01T00:02:00Z";
const FOURTH: &str = "2099-01-01T00:03:00Z";

fn fields() -> Vec<DetailField> {
    vec![
        DetailField::TaskContent,
        DetailField::TaskCreatorLogin,
        DetailField::TaskCreatorDisplayName,
        DetailField::TaskState,
        DetailField::TaskCreatedAt,
        DetailField::TaskUpdatedAt,
        DetailField::TaskPending,
        DetailField::TaskResolvedAt,
        DetailField::TaskResolver,
        DetailField::TaskResolverLogin,
        DetailField::TaskResolverDisplayName,
        DetailField::TaskCommentId,
    ]
}
fn actor(id: &str) -> TaskActor {
    TaskActor {
        provider_id: id.into(),
        kind: "FUTURE_ACCOUNT_TYPE".into(),
        login: Some(format!("{id}-login")),
        display_name: Some(format!("{id}-display")),
    }
}
fn task(id: &str, at: &str) -> DetailEntry {
    DetailEntry {
        id: format!("task:repo:67:{id}"),
        provider_id: format!("repo:67:{id}"),
        author: None,
        title: None,
        state: None,
        body: DetailValue::default(),
        observed_body_state: DetailValueState::NotLoaded,
        updated_at: None,
        head_oid: None,
        native: Some(NativeDetailPayload::TaskV1(TaskV1 {
            content: known(Some("Saved task content")),
            observed_content_state: DetailValueState::Known,
            creator: actor("creator:opaque"),
            state: Some("FUTURE_TASK_STATE".into()),
            created_at: Some("2020-01-01T00:00:00Z".into()),
            updated_at: Some(at.into()),
            pending: Some(true),
            resolved_at: Some("2030-01-01T00:00:00Z".into()),
            resolved_by: Some(actor("resolver-a")),
            comment_id: Some("7".into()),
        })),
        field_mask: fields(),
        field_validations: vec![],
    }
}
fn native(entry: &DetailEntry) -> &TaskV1 {
    let Some(NativeDetailPayload::TaskV1(task)) = &entry.native else {
        panic!("typed task");
    };
    task
}
fn native_mut(entry: &mut DetailEntry) -> &mut TaskV1 {
    let Some(NativeDetailPayload::TaskV1(task)) = &mut entry.native else {
        panic!("typed task");
    };
    task
}
async fn fixture() -> (tempfile::TempDir, Store, RemoteAccount, LocalDraft) {
    let directory = tempfile::tempdir().unwrap();
    let store = Store::open(directory.path().join("tasks.sqlite"))
        .await
        .unwrap();
    let account = seed(&store, "a").await;
    let draft = store
        .save_draft(LocalDraft {
            account_id: "a".into(),
            subject_id: "pull".into(),
            body: "Task sync cannot change this private draft".into(),
            generation: "0".into(),
        })
        .await
        .unwrap();
    (directory, store, account, draft)
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
    account: &RemoteAccount,
    entries: Vec<DetailEntry>,
    at: &str,
) -> DetailCommit {
    let mut page = commit(store, account, DetailFacet::Tasks).await;
    page.subject_binding = Some(binding(store).await);
    page.source = DetailSource {
        source: "synthetic.tasks.v1".into(),
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
    store.detail(query("a", DetailFacet::Tasks)).await.unwrap()
}
fn validation(entry: &DetailEntry, field: DetailField) -> Option<&DetailFieldValidation> {
    entry
        .field_validations
        .iter()
        .find(|validation| validation.field == field)
}

#[test]
fn task_native_serde_keeps_adjacent_tag_false_null_and_opaque_actor_context() {
    let mut entry = task("1", FIRST);
    native_mut(&mut entry).pending = Some(false);
    native_mut(&mut entry).resolved_by = None;
    native_mut(&mut entry).resolved_at = None;
    let json = serde_json::to_value(&entry).unwrap();
    assert_eq!(json["native"]["kind"], "task.v1");
    assert_eq!(json["native"]["value"]["pending"], false);
    assert_eq!(
        json["native"]["value"]["resolved_by"],
        serde_json::Value::Null
    );
    assert_eq!(
        json["native"]["value"]["creator"]["provider_id"],
        "creator:opaque"
    );
    assert_eq!(serde_json::from_value::<DetailEntry>(json).unwrap(), entry);
    assert!(
        serde_json::from_value::<NativeDetailPayload>(serde_json::json!({
            "kind": "task.v2", "value": {}
        }))
        .is_err()
    );
}

#[tokio::test]
async fn twelve_task_fields_retain_own_clocks_and_content_omission_through_cold_reopen() {
    let (directory, store, account, draft) = fixture().await;
    store
        .apply_detail(page(&store, &account, vec![task("1", SECOND)], FIRST).await)
        .await
        .unwrap();
    let initial = read(&store).await;
    assert_eq!(initial.entries[0].field_validations.len(), 12);
    assert_eq!(
        initial
            .evidence
            .value_source
            .as_ref()
            .unwrap()
            .provider_updated_at,
        None
    );
    let mut old = task("1", FIRST);
    native_mut(&mut old).content = known(Some("Older own task content"));
    native_mut(&mut old).state = Some("older-state".into());
    store
        .apply_detail(page(&store, &account, vec![old], SECOND).await)
        .await
        .unwrap();
    let old = read(&store).await;
    assert_eq!(native(&old.entries[0]), native(&initial.entries[0]));
    assert_eq!(
        old.entries[0].field_validations,
        initial.entries[0].field_validations
    );

    let mut omitted = task("1", THIRD);
    omitted.field_mask.retain(|field| {
        !matches!(
            field,
            DetailField::TaskCreatorLogin
                | DetailField::TaskCreatorDisplayName
                | DetailField::TaskResolver
                | DetailField::TaskResolverLogin
                | DetailField::TaskResolverDisplayName
        )
    });
    let value = native_mut(&mut omitted);
    value.content = DetailValue {
        state: DetailValueState::Omitted,
        text: None,
    };
    value.observed_content_state = DetailValueState::Omitted;
    value.creator.login = None;
    value.creator.display_name = None;
    value.resolved_by = None;
    value.pending = Some(false);
    value.resolved_at = None;
    value.comment_id = None;
    value.state = Some("new-state".into());
    store
        .apply_detail(page(&store, &account, vec![omitted], THIRD).await)
        .await
        .unwrap();
    let saved = read(&store).await;
    let row = &saved.entries[0];
    assert_eq!(native(row).content, native(&initial.entries[0]).content);
    assert_eq!(
        native(row).observed_content_state,
        DetailValueState::Omitted
    );
    assert_eq!(native(row).creator, native(&initial.entries[0]).creator);
    assert_eq!(
        native(row).resolved_by,
        native(&initial.entries[0]).resolved_by
    );
    assert_eq!(native(row).pending, Some(false));
    assert_eq!(native(row).resolved_at, None);
    assert_eq!(native(row).comment_id, None);
    assert_eq!(
        validation(row, DetailField::TaskContent),
        validation(&initial.entries[0], DetailField::TaskContent)
    );
    assert_eq!(
        validation(row, DetailField::TaskCreatorLogin),
        validation(&initial.entries[0], DetailField::TaskCreatorLogin)
    );
    assert_eq!(
        validation(row, DetailField::TaskPending)
            .unwrap()
            .validated_at,
        THIRD
    );
    assert_eq!(store.draft("a", "pull").await.unwrap(), Some(draft.clone()));
    store.close().await;
    drop(store);
    let reopened = Store::open(directory.path().join("tasks.sqlite"))
        .await
        .unwrap();
    assert_eq!(read(&reopened).await, saved);
    let mut middle = task("1", "2099-01-01T00:01:30Z");
    native_mut(&mut middle).content = known(Some("Newer than retained content clock"));
    native_mut(&mut middle).state = Some("too-old-for-state".into());
    reopened
        .apply_detail(page(&reopened, &account, vec![middle], FOURTH).await)
        .await
        .unwrap();
    let merged = read(&reopened).await;
    assert_eq!(
        native(&merged.entries[0]).content.text.as_deref(),
        Some("Newer than retained content clock")
    );
    assert_eq!(
        native(&merged.entries[0]).state.as_deref(),
        Some("new-state")
    );
    assert_eq!(
        native(&merged.entries[0]).updated_at.as_deref(),
        Some(THIRD)
    );
    assert_eq!(
        validation(&merged.entries[0], DetailField::TaskContent)
            .unwrap()
            .validated_at,
        FOURTH
    );
    assert_eq!(
        validation(&merged.entries[0], DetailField::TaskState)
            .unwrap()
            .validated_at,
        THIRD
    );
    assert_eq!(reopened.draft("a", "pull").await.unwrap(), Some(draft));
}

#[tokio::test]
async fn omitted_and_oversized_task_content_never_fabricate_known_validation_or_erase_saved_text() {
    let (_directory, store, account, draft) = fixture().await;
    let mut missing = task("1", FIRST);
    native_mut(&mut missing).content = DetailValue {
        state: DetailValueState::Omitted,
        text: None,
    };
    native_mut(&mut missing).observed_content_state = DetailValueState::Omitted;
    store
        .apply_detail(page(&store, &account, vec![missing], FIRST).await)
        .await
        .unwrap();
    let empty = read(&store).await;
    assert_eq!(
        native(&empty.entries[0]).content.state,
        DetailValueState::Omitted
    );
    assert_eq!(
        validation(&empty.entries[0], DetailField::TaskContent),
        None
    );
    let mut oversize = task("2", FIRST);
    native_mut(&mut oversize).content = known(Some(&"x".repeat(65_537)));
    store
        .apply_detail(page(&store, &account, vec![task("1", SECOND), oversize], SECOND).await)
        .await
        .unwrap();
    let saved = read(&store).await;
    assert_eq!(
        native(&saved.entries[1]).content.state,
        DetailValueState::Oversized
    );
    assert_eq!(native(&saved.entries[1]).content.text, None);
    assert_eq!(
        validation(&saved.entries[1], DetailField::TaskContent),
        None
    );
    let mut newer_oversize = task("1", THIRD);
    native_mut(&mut newer_oversize).content = known(Some(&"x".repeat(65_537)));
    store
        .apply_detail(page(&store, &account, vec![newer_oversize], THIRD).await)
        .await
        .unwrap();
    let retained = read(&store).await;
    assert_eq!(
        native(&retained.entries[0]).content,
        native(&saved.entries[0]).content
    );
    assert_eq!(
        native(&retained.entries[0]).observed_content_state,
        DetailValueState::Oversized
    );
    assert_eq!(
        validation(&retained.entries[0], DetailField::TaskContent),
        validation(&saved.entries[0], DetailField::TaskContent)
    );
    let mut known_empty = task("1", FOURTH);
    native_mut(&mut known_empty).content = known(Some(""));
    store
        .apply_detail(page(&store, &account, vec![known_empty], FOURTH).await)
        .await
        .unwrap();
    assert_eq!(
        native(&read(&store).await.entries[0])
            .content
            .text
            .as_deref(),
        Some("")
    );
    assert_eq!(store.draft("a", "pull").await.unwrap(), Some(draft));
}

#[tokio::test]
async fn resolver_identity_precedes_mask_order_and_rejected_person_cannot_restore_cleared_names() {
    let (_directory, store, account, draft) = fixture().await;
    store
        .apply_detail(page(&store, &account, vec![task("1", FIRST)], FIRST).await)
        .await
        .unwrap();
    let mut changed = task("1", SECOND);
    changed.field_mask.retain(|field| {
        !matches!(
            field,
            DetailField::TaskResolverLogin | DetailField::TaskResolverDisplayName
        )
    });
    let mut resolver = actor("resolver-b");
    resolver.login = None;
    resolver.display_name = None;
    native_mut(&mut changed).resolved_by = Some(resolver.clone());
    store
        .apply_detail(page(&store, &account, vec![changed], SECOND).await)
        .await
        .unwrap();
    let changed = read(&store).await;
    assert_eq!(native(&changed.entries[0]).resolved_by, Some(resolver));
    assert_eq!(
        validation(&changed.entries[0], DetailField::TaskResolverLogin),
        None
    );
    assert_eq!(
        validation(&changed.entries[0], DetailField::TaskResolverDisplayName),
        None
    );
    let mut old = task("1", FIRST);
    // A former person's presentations arrive before its stale identity tag.
    old.field_mask.reverse();
    native_mut(&mut old).resolved_by.as_mut().unwrap().login = Some("wrong-person-late".into());
    store
        .apply_detail(page(&store, &account, vec![old], THIRD).await)
        .await
        .unwrap();
    let held = read(&store).await;
    assert_eq!(
        native(&held.entries[0]).resolved_by,
        native(&changed.entries[0]).resolved_by
    );
    assert_eq!(
        validation(&held.entries[0], DetailField::TaskResolverLogin),
        None
    );
    assert!(!held.entries[0].field_mask.iter().any(|field| matches!(
        field,
        DetailField::TaskResolver
            | DetailField::TaskResolverLogin
            | DetailField::TaskResolverDisplayName
    )));
    let mut observed = task("1", THIRD);
    observed.field_mask.reverse();
    native_mut(&mut observed).resolved_by = Some(actor("resolver-b"));
    store
        .apply_detail(page(&store, &account, vec![observed], FOURTH).await)
        .await
        .unwrap();
    let current = read(&store).await;
    assert_eq!(
        native(&current.entries[0])
            .resolved_by
            .as_ref()
            .unwrap()
            .login
            .as_deref(),
        Some("resolver-b-login")
    );
    let mut cleared = task("1", FOURTH);
    native_mut(&mut cleared).resolved_by = None;
    store
        .apply_detail(page(&store, &account, vec![cleared], FOURTH).await)
        .await
        .unwrap();
    let cleared = read(&store).await;
    assert_eq!(native(&cleared.entries[0]).resolved_by, None);
    for field in [
        DetailField::TaskResolver,
        DetailField::TaskResolverLogin,
        DetailField::TaskResolverDisplayName,
    ] {
        assert_eq!(
            validation(&cleared.entries[0], field).unwrap().validated_at,
            FOURTH
        );
    }
    assert_eq!(store.draft("a", "pull").await.unwrap(), Some(draft));
}

#[tokio::test]
async fn invalid_task_values_families_clocks_and_page_authority_roll_back_atomically() {
    let (_directory, store, account, draft) = fixture().await;
    store
        .apply_detail(page(&store, &account, vec![task("1", FIRST)], FIRST).await)
        .await
        .unwrap();
    for invalid in 0..30 {
        let mut observation = page(&store, &account, vec![task("1", SECOND)], SECOND).await;
        match invalid {
            0 => observation.entries[0].native = None,
            1 => observation.entries[0].field_mask[0] = DetailField::Body,
            2 => observation.entries[0].field_mask[0] = DetailField::ParticipantApproved,
            3 => observation.source.field_mask[0] = DetailField::ParticipantApproved,
            4 => observation.entries[0].author = Some("generic overload".into()),
            5 => native_mut(&mut observation.entries[0]).state = None,
            6 => native_mut(&mut observation.entries[0]).created_at = None,
            7 => native_mut(&mut observation.entries[0]).updated_at = None,
            8 => native_mut(&mut observation.entries[0]).updated_at = Some("not-a-date".into()),
            9 => native_mut(&mut observation.entries[0]).pending = None,
            10 => native_mut(&mut observation.entries[0]).comment_id = Some("007".into()),
            11 => native_mut(&mut observation.entries[0]).comment_id = Some("0".into()),
            12 => native_mut(&mut observation.entries[0]).creator.kind = "different-context".into(),
            13 => {
                native_mut(&mut observation.entries[0]).creator.provider_id =
                    "different-person".into()
            }
            14 => native_mut(&mut observation.entries[0]).creator.kind = "bad\nkind".into(),
            15 => native_mut(&mut observation.entries[0]).creator.login = Some("x".repeat(256)),
            16 => native_mut(&mut observation.entries[0]).content = known(None),
            17 => {
                native_mut(&mut observation.entries[0]).observed_content_state =
                    DetailValueState::Omitted
            }
            18 => observation.source.provider_updated_at = Some(SECOND.into()),
            19 => observation.subject_binding = None,
            20 => observation.entries.push(observation.entries[0].clone()),
            21 => observation.reconciliation.head_scope = DetailHeadScope::CurrentHead,
            22 => {
                observation.entries[0].field_validations = vec![DetailFieldValidation {
                    field: DetailField::TaskState,
                    validated_at: SECOND.into(),
                    source: "renderer".into(),
                    adapter_version: 1,
                }]
            }
            23 => observation.entries[0]
                .field_mask
                .push(DetailField::TaskState),
            24 => observation.source.field_mask[0] = DetailField::TaskState,
            25 => observation.entries = vec![task("1", FIRST); 51],
            26 => observation.etag = Some("invalid-task-validator".into()),
            27 => observation.reconciliation.enumeration = DetailEnumeration::Incremental,
            28 => observation.entries[0]
                .field_mask
                .retain(|field| *field != DetailField::TaskUpdatedAt),
            29 => {
                let value = native_mut(&mut observation.entries[0]);
                value.content = DetailValue {
                    state: DetailValueState::Omitted,
                    text: Some("x".repeat(65_537)),
                };
                value.observed_content_state = DetailValueState::Omitted;
            }
            _ => unreachable!(),
        }
        let before = read(&store).await;
        let revision = store.revision().await.unwrap();
        assert_eq!(
            store.apply_detail(observation).await.unwrap_err().code,
            ErrorCode::InvalidInput,
            "invalid case {invalid}"
        );
        assert_eq!(read(&store).await, before, "invalid case {invalid}");
        assert_eq!(store.revision().await.unwrap(), revision);
        assert_eq!(store.draft("a", "pull").await.unwrap(), Some(draft.clone()));
    }
    for facet in [
        DetailFacet::Comments,
        DetailFacet::Reviews,
        DetailFacet::Participants,
    ] {
        let mut wrong = commit(&store, &account, facet).await;
        wrong.entries = vec![task("1", FIRST)];
        assert_eq!(
            store.apply_detail(wrong).await.unwrap_err().code,
            ErrorCode::InvalidInput
        );
    }
}

#[tokio::test]
async fn mutable_task_paging_stays_partial_and_single_page_absence_and_local_keysets_are_distinct()
{
    let (directory, store, account, draft) = fixture().await;
    store
        .apply_detail(page(&store, &account, vec![task("historic", FIRST)], FIRST).await)
        .await
        .unwrap();
    let mut first = page(
        &store,
        &account,
        (1..=50).map(|id| task(&id.to_string(), SECOND)).collect(),
        SECOND,
    )
    .await;
    first.reconciliation.enumeration = DetailEnumeration::Uncertain;
    first.next_cursor = Some("next-task-page".into());
    first.complete = false;
    first.whole_scope = false;
    store.apply_detail(first).await.unwrap();
    assert_eq!(read(&store).await.entries.len(), 51);
    let saved = read(&store).await;
    store.close().await;
    drop(store);
    let reopened = Store::open(directory.path().join("tasks.sqlite"))
        .await
        .unwrap();
    assert_eq!(read(&reopened).await, saved);
    let mut last = page(&reopened, &account, vec![task("51", THIRD)], THIRD).await;
    assert_eq!(last.request_cursor.as_deref(), Some("next-task-page"));
    last.reconciliation.enumeration = DetailEnumeration::Uncertain;
    last.whole_scope = false;
    let mut false_full = last.clone();
    false_full.reconciliation = DetailReconciliation::full_history();
    let before = read(&reopened).await;
    assert_eq!(
        reopened.apply_detail(false_full).await.unwrap_err().code,
        ErrorCode::InvalidInput
    );
    assert_eq!(read(&reopened).await, before);
    reopened.apply_detail(last).await.unwrap();
    let saved = read(&reopened).await;
    assert_eq!(saved.entries.len(), 52);
    assert_eq!(saved.evidence.coverage.state, CoverageState::Partial);
    assert_eq!(saved.evidence.saved_empty, None);
    assert!(
        saved
            .entries
            .iter()
            .any(|entry| entry.id.ends_with(":historic"))
    );
    let mut local_query = query("a", DetailFacet::Tasks);
    local_query.limit = 50;
    let local_first = reopened.detail(local_query.clone()).await.unwrap();
    assert_eq!(local_first.entries.len(), 50);
    local_query.cursor = local_first.next_cursor;
    assert!(local_query.cursor.is_some());
    let local_second = reopened.detail(local_query).await.unwrap();
    assert_eq!(local_second.entries.len(), 2);
    assert!(local_second.next_cursor.is_none());
    assert!(
        local_second
            .entries
            .iter()
            .all(|entry| !local_first.entries.iter().any(|first| first.id == entry.id))
    );
    reopened
        .apply_detail(page(&reopened, &account, vec![], FOURTH).await)
        .await
        .unwrap();
    let empty = read(&reopened).await;
    assert!(empty.entries.is_empty());
    assert_eq!(empty.evidence.coverage.state, CoverageState::Complete);
    assert_eq!(empty.evidence.saved_empty, Some(true));
    assert_eq!(reopened.draft("a", "pull").await.unwrap(), Some(draft));
}

#[tokio::test]
async fn task_deselection_and_permission_denial_preserve_body_and_private_drafts() {
    let (_directory, store, account, draft) = fixture().await;
    store
        .apply_detail(commit(&store, &account, DetailFacet::Body).await)
        .await
        .unwrap();
    store
        .apply_detail(page(&store, &account, vec![task("1", FIRST)], FIRST).await)
        .await
        .unwrap();
    let saved = read(&store).await;
    let held = page(&store, &account, vec![], SECOND).await;
    let body = store.detail(query("a", DetailFacet::Body)).await.unwrap();
    store.select_repository("a", "repo", false).await.unwrap();
    assert_eq!(
        read(&store).await.evidence.availability,
        DetailAvailability::Unavailable
    );
    store.select_repository("a", "repo", true).await.unwrap();
    assert_eq!(read(&store).await.entries, saved.entries);
    assert_eq!(
        store.apply_detail(held).await.unwrap_err().code,
        ErrorCode::StaleView
    );
    store
        .set_sync_status(
            "a",
            "1",
            &DetailFacet::Tasks.scope("pull"),
            SyncStatus {
                state: SyncState::Error,
                error: Some(CollaborationError::new(
                    ErrorCode::PermissionDenied,
                    "Tasks only denial",
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
    let current_body = store.detail(query("a", DetailFacet::Body)).await.unwrap();
    assert_eq!(current_body.body, body.body);
    assert_eq!(current_body.metadata, body.metadata);
    assert_eq!(store.draft("a", "pull").await.unwrap(), Some(draft));
    assert_eq!(DetailFacet::Tasks.capability(&RemoteItemKind::Issue), None);
    assert_eq!(
        DetailFacet::Tasks.capability(&RemoteItemKind::Notification),
        None
    );
}

#[tokio::test]
async fn saved_task_history_survives_head_change_but_held_original_binding_cannot_apply() {
    let (_directory, store, account, draft) = fixture().await;
    store
        .apply_detail(page(&store, &account, vec![task("1", FIRST)], FIRST).await)
        .await
        .unwrap();
    let saved = read(&store).await;
    let held = page(&store, &account, vec![], SECOND).await;
    let mut changed = store.detail_subject("a", "pull").await.unwrap();
    changed.head_oid = Some("new-known-head".into());
    changed.updated_at = "2026-10-04T00:00:00Z".into();
    let run = store
        .begin_sync("a", "1", "repo:repo:pull_request")
        .await
        .unwrap();
    store
        .apply_page(PageCommit {
            account_id: "a".into(),
            authorization_epoch: "1".into(),
            scope: "repo:repo:pull_request".into(),
            run_id: run,
            repositories: vec![],
            items: vec![changed],
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
    let current = read(&store).await;
    assert_eq!(current.entries, saved.entries);
    assert_eq!(
        store.apply_detail(held).await.unwrap_err().code,
        ErrorCode::StaleView
    );
    assert_eq!(read(&store).await, current);
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
fn tasks_profile(_: &RemoteAccount, _: &ProviderInstance) -> providers::ProviderProfile {
    let mut profile = providers::ProviderProfile::read_only(InboxSemantics::None, false);
    for facet in &mut profile.facets {
        if facet.facet == ResourceFacet::Tasks {
            facet.state = CapabilityState::Supported;
            facet.reason = None;
        }
    }
    profile
}

#[tokio::test]
async fn contextual_tasks_are_read_only_and_unimplemented_provider_profiles_stay_explicit() {
    let (_directory, store, account, draft) = fixture().await;
    store
        .apply_detail(page(&store, &account, vec![task("1", FIRST)], FIRST).await)
        .await
        .unwrap();
    let supported = store
        .contextual_capabilities(capability_request(), tasks_profile)
        .await
        .unwrap();
    let tasks = supported
        .facets
        .iter()
        .find(|facet| facet.facet == ResourceFacet::Tasks)
        .unwrap();
    assert_eq!(tasks.saved_read.state, CapabilityState::Supported);
    assert_eq!(tasks.observation, CapabilityObservation::Complete);
    assert_eq!(tasks.remote_write.state, CapabilityState::Unsupported);
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
            .find(|facet| facet.facet == ResourceFacet::Tasks)
            .unwrap()
            .saved_read
            .state,
        CapabilityState::Unsupported
    );
    store
        .set_sync_status(
            "a",
            "1",
            &DetailFacet::Tasks.scope("pull"),
            SyncStatus {
                state: SyncState::Error,
                error: Some(CollaborationError::new(
                    ErrorCode::PermissionDenied,
                    "Tasks only denial",
                )),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    let denied = store
        .contextual_capabilities(capability_request(), tasks_profile)
        .await
        .unwrap();
    let tasks = denied
        .facets
        .iter()
        .find(|facet| facet.facet == ResourceFacet::Tasks)
        .unwrap();
    assert_eq!(tasks.saved_read.state, CapabilityState::Unavailable);
    assert_eq!(store.draft("a", "pull").await.unwrap(), Some(draft));
}
