use super::*;
use crate::commands::{CommandDraft, CommandTarget, CommandTargetKind, seal_command};
fn selected() -> IssueMetadataSelection {
    IssueMetadataSelection {
        labels: vec![IssueMetadataLabel {
            provider_id: "7".into(),
            name: "bug/雪 #?%".into(),
            color: Some("aAbB00".into()),
        }],
        assignees: vec![IssueMetadataAssignee {
            provider_id: "8".into(),
            login: "app[bot]".into(),
        }],
        milestone: Some(IssueMetadataMilestone {
            provider_id: "9".into(),
            number: "2".into(),
            title: "Release".into(),
        }),
    }
}
fn payload() -> PayloadV2 {
    PayloadV2 {
        request: SubmitIssueV2Request {
            context: IssueDraftContext {
                account_id: "account".into(),
                repository_id: "repo".into(),
                authorization_epoch: "1".into(),
                authorization_view: "0".into(),
                review_token: "a".repeat(64),
            },
            draft_id: "00000000-0000-4000-8000-000000000001".into(),
            draft_generation: "1".into(),
            command_id: "00000000-0000-4000-8000-000000000002".into(),
            accept_background_delivery: true,
            accept_metadata_best_effort: true,
        },
        title: "Title".into(),
        body: "Body".into(),
        repository_native: "42".into(),
        metadata: SelectionV2::from_public(selected()).unwrap(),
    }
}
fn sealed(p: PayloadV2) -> crate::commands::CommandSubmission {
    seal_command(CommandDraft {
        command_id: p.request.command_id.clone(),
        account_id: "account".into(),
        authorization_epoch: "1".into(),
        target: CommandTarget::new(CommandTargetKind::Repository, "repo", Some("repo".into()))
            .unwrap(),
        payload: p,
        guards: vec![],
        dependencies: vec![],
    })
    .unwrap()
}
#[test]
fn frozen_selection_bytes_reject_noncanonical_unknown_duplicate_and_wrong_shapes() {
    let selection = SelectionV2::from_public(selected()).unwrap();
    let bytes = encode(&selection).unwrap();
    assert_eq!(
        String::from_utf8(bytes.clone()).unwrap(),
        r##"{"labels":[{"id":"7","name":"bug/雪 #?%","color":"aAbB00"}],"assignees":[{"id":"8","login":"app[bot]"}],"milestone":{"id":"9","number":"2","title":"Release"}}"##
    );
    assert_eq!(decode_json::<SelectionV2>(&bytes).unwrap(), selection);
    for wrong in [
        bytes.iter().copied().chain([b' ']).collect::<Vec<_>>(),
        String::from_utf8(bytes.clone())
            .unwrap()
            .replace("\"id\":\"7\"", "\"id\":\"7\",\"extra\":null")
            .into_bytes(),
        String::from_utf8(bytes)
            .unwrap()
            .replace("\"id\":\"7\"", "\"id\":\"7\",\"id\":\"7\"")
            .into_bytes(),
    ] {
        assert!(decode_json::<SelectionV2>(&wrong).is_err());
    }
}
#[test]
fn selection_canonicalization_is_order_independent_but_never_changes_spelling() {
    let mut s = selected();
    s.labels.push(IssueMetadataLabel {
        provider_id: "11".into(),
        name: " BUG ".into(),
        color: None,
    });
    let expected = SelectionV2::from_public(s.clone()).unwrap();
    s.labels.reverse();
    assert_eq!(expected, SelectionV2::from_public(s).unwrap());
    assert_eq!(expected.labels[0].name, " BUG ");
    let mut invalid = expected.clone();
    invalid.labels.reverse();
    assert!(invalid.validate().is_err());
}
#[test]
fn ids_aliases_control_text_and_product_caps_are_bounded() {
    let mut v = selected();
    v.labels.push(v.labels[0].clone());
    assert!(selection(&v).is_err());
    let mut v = selected();
    v.assignees[0].provider_id = "08".into();
    assert!(selection(&v).is_err());
    let mut v = selected();
    v.labels[0].name = "bad\nname".into();
    assert!(selection(&v).is_err());
    let mut v = selected();
    v.milestone.as_mut().unwrap().number = "0".into();
    assert!(selection(&v).is_err());
    let mut v = selected();
    v.assignees = (1..=11)
        .map(|n| IssueMetadataAssignee {
            provider_id: n.to_string(),
            login: format!("user{n}"),
        })
        .collect();
    assert!(selection(&v).is_err());
}
#[test]
fn payload_two_roundtrips_exact_tags_and_rejects_v1_truncation_extra_and_missing_consent() {
    let p = payload();
    let command = sealed(p.clone());
    assert_eq!(command.operation().payload_version(), 2);
    let bytes = command.payload_bytes();
    let decoded = decode_payload(bytes, "account", &p.request.command_id, "repo", "1").unwrap();
    assert_eq!(decoded.metadata, p.metadata);
    for wrong in [
        &bytes[..bytes.len() - 7],
        &bytes.iter().copied().chain([0]).collect::<Vec<_>>(),
    ] {
        assert!(decode_payload(wrong, "account", &p.request.command_id, "repo", "1").is_err());
    }
    let mut p = p;
    p.request.accept_metadata_best_effort = false;
    assert!(p.validate().is_err());
    p.metadata = SelectionV2::default();
    assert!(p.validate().is_ok());
}
#[test]
fn hash_changes_for_authored_metadata_and_preserves_order_equivalence() {
    let v = SelectionV2::from_public(selected()).unwrap();
    let hash = content_hash("t", "b", &v).unwrap();
    let mut changed = v.clone();
    changed.labels[0].name.push('!');
    assert_ne!(hash, content_hash("t", "b", &changed).unwrap());
    assert_ne!(hash, content_hash("tb", "", &v).unwrap());
}
#[test]
fn field_outcome_is_derived_from_selected_identity_and_rejects_fabricated_members() {
    let p = payload();
    let mut o = MetadataObservationV2 {
        labels: SetObservationV2::Known {
            present_ids: vec!["7".into()],
        },
        assignees: SetObservationV2::Known {
            present_ids: vec![],
        },
        milestone: MilestoneObservationV2::Unobserved {
            reason: IssueMetadataUnobservedReason::Malformed,
        },
    };
    let got = o.outcome(&p.request.command_id, &p.metadata).unwrap();
    assert_eq!(
        got.fields.iter().map(|v| v.result).collect::<Vec<_>>(),
        [
            IssueMetadataResult::Applied,
            IssueMetadataResult::Different,
            IssueMetadataResult::Unobserved
        ]
    );
    assert!(got.needs_attention);
    o.labels = SetObservationV2::Known {
        present_ids: vec!["999".into()],
    };
    assert!(o.outcome(&p.request.command_id, &p.metadata).is_err());
    o.labels = SetObservationV2::Known {
        present_ids: vec!["7".into(), "7".into()],
    };
    assert!(o.outcome(&p.request.command_id, &p.metadata).is_err());
}
#[test]
fn wire_omits_unrequested_keys_and_enforces_actual_escaped_body_budget() {
    let mut p = payload();
    let body: serde_json::Value = serde_json::from_slice(&request_bytes(&p).unwrap()).unwrap();
    assert_eq!(body["labels"], serde_json::json!(["bug/雪 #?%"]));
    assert_eq!(body["assignees"], serde_json::json!(["app[bot]"]));
    assert_eq!(body["milestone"], 2);
    p.metadata = SelectionV2::default();
    let body: serde_json::Value = serde_json::from_slice(&request_bytes(&p).unwrap()).unwrap();
    assert!(body.get("labels").is_none());
    assert!(body.get("assignees").is_none());
    assert!(body.get("milestone").is_none());
    p.body = "\u{1}".repeat(16384);
    assert!(request_bytes(&p).is_err());
    p.body = "x".repeat(16384);
    assert!(request_bytes(&p).unwrap().len() < 65536);
}
#[test]
fn combined_payload_budget_refuses_individually_bounded_but_oversized_fields() {
    let mut p = payload();
    p.body = "x".repeat(16384);
    p.metadata.labels = (1..=32)
        .map(|id| LabelV2 {
            id: id.to_string(),
            name: format!("{}{}", "\\".repeat(1018), id),
            color: None,
        })
        .collect();
    p.metadata.labels.sort_by(|a, b| a.id.cmp(&b.id));
    assert!(p.metadata.validate().is_ok());
    assert!(p.validate().is_err());
}
