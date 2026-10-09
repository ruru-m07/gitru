//! Clock decoding stays bounded, and only Tasks owns the twelve-field family.
use super::*;
use crate::runtime::detail_tests::fixtures;

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
fn encoded(fields: Vec<DetailField>) -> serde_json::Value {
    let mut json = serde_json::to_value(StoredEntry::from_entry(fixtures::entry("saved"))).unwrap();
    json["field_clocks"] = serde_json::Value::Array(
        fields
            .into_iter()
            .map(|field| {
                serde_json::json!({ "field": field, "source": "task-source", "adapter_version": 1,
            "provider_updated_at": "2026-10-03T00:00:00Z", "scope_head": null })
            })
            .collect(),
    );
    json
}

#[test]
fn twelve_saved_task_clocks_survive_decode_without_widening_other_facets() {
    let json = encoded(fields());
    let mut saved: StoredEntry = serde_json::from_value(json.clone()).unwrap();
    let content = saved.entry.clone();
    saved.initialize_legacy(DetailFacet::Tasks);
    assert_eq!(saved.field_clocks.len(), 12);
    assert_eq!(saved.entry, content);
    for facet in [
        DetailFacet::Body,
        DetailFacet::Comments,
        DetailFacet::Reviews,
        DetailFacet::Checks,
        DetailFacet::Participants,
    ] {
        let mut wrong: StoredEntry = serde_json::from_value(json.clone()).unwrap();
        wrong.initialize_legacy(facet);
        assert!(wrong.field_clocks.is_empty());
        assert_eq!(wrong.entry, content);
    }
}

#[test]
fn mixed_duplicate_corrupt_future_and_thirteenth_clocks_never_grant_task_authority() {
    let mut mixed = fields();
    mixed[0] = DetailField::Body;
    let mut participant = fields();
    participant[0] = DetailField::ParticipantApproved;
    let mut duplicate = fields();
    duplicate[0] = DetailField::TaskState;
    let mut oversized = fields();
    oversized.push(DetailField::TaskContent);
    let mut cases = vec![
        encoded(mixed),
        encoded(participant),
        encoded(duplicate),
        encoded(oversized),
    ];
    let mut future = encoded(fields());
    future["field_clocks"][0]["field"] = serde_json::Value::String("future_task_authority".into());
    cases.push(future);
    let mut invalid_time = encoded(fields());
    invalid_time["field_clocks"][0]["provider_updated_at"] =
        serde_json::Value::String("yesterday".into());
    cases.push(invalid_time);
    let mut future_version = encoded(fields());
    future_version["reconciliation_version"] = serde_json::json!(2);
    cases.push(future_version);
    for json in cases {
        let mut saved: StoredEntry = serde_json::from_value(json).unwrap();
        let content = saved.entry.clone();
        saved.initialize_legacy(DetailFacet::Tasks);
        assert!(saved.field_clocks.is_empty());
        assert_eq!(saved.entry, content);
    }
}

#[test]
fn forgetting_resolver_presentations_removes_both_clocks_and_validations_only() {
    let mut saved: StoredEntry = serde_json::from_value(encoded(fields())).unwrap();
    saved.initialize_legacy(DetailFacet::Tasks);
    saved.entry.field_validations = fields()
        .into_iter()
        .map(|field| DetailFieldValidation {
            field,
            validated_at: "2026-10-03T00:00:00Z".into(),
            source: "task-source".into(),
            adapter_version: 1,
        })
        .collect();
    saved.forget(&[
        DetailField::TaskResolverLogin,
        DetailField::TaskResolverDisplayName,
    ]);
    assert_eq!(saved.field_clocks.len(), 10);
    assert_eq!(saved.entry.field_validations.len(), 10);
    for field in [
        DetailField::TaskResolverLogin,
        DetailField::TaskResolverDisplayName,
    ] {
        assert!(!saved.field_clocks.iter().any(|clock| clock.field == field));
        assert!(
            !saved
                .entry
                .field_validations
                .iter()
                .any(|validation| validation.field == field)
        );
    }
    assert!(
        saved
            .field_clocks
            .iter()
            .any(|clock| clock.field == DetailField::TaskResolver)
    );
    assert!(
        saved
            .entry
            .field_validations
            .iter()
            .any(|validation| validation.field == DetailField::TaskResolver)
    );
}
