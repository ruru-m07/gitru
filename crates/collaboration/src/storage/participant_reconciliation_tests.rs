//! Saved evidence has a bounded family, even though the enum now has twelve tags.
use super::*;
use crate::runtime::detail_tests::fixtures;

fn fields(participant: bool) -> Vec<DetailField> {
    if participant {
        vec![
            DetailField::ParticipantLogin,
            DetailField::ParticipantDisplayName,
            DetailField::ParticipantRole,
            DetailField::ParticipantApproved,
            DetailField::ParticipantState,
            DetailField::ParticipantParticipatedAt,
        ]
    } else {
        vec![
            DetailField::Body,
            DetailField::Author,
            DetailField::Title,
            DetailField::State,
            DetailField::UpdatedAt,
            DetailField::HeadOid,
        ]
    }
}
fn encoded(clocks: Vec<DetailField>) -> serde_json::Value {
    let mut json = serde_json::to_value(StoredEntry::from_entry(fixtures::entry("saved"))).unwrap();
    json["field_clocks"] = serde_json::Value::Array(
        clocks
            .into_iter()
            .map(|field| {
                serde_json::json!({ "field": field, "source": "saved-source", "adapter_version": 1,
            "provider_updated_at": "2026-10-03T00:00:00Z", "scope_head": null })
            })
            .collect(),
    );
    json
}

#[test]
fn six_saved_clocks_remain_supported_for_each_disjoint_field_family() {
    for (participant, facet) in [
        (false, DetailFacet::Comments),
        (true, DetailFacet::Participants),
    ] {
        let mut saved: StoredEntry = serde_json::from_value(encoded(fields(participant))).unwrap();
        let content = saved.entry.clone();
        saved.initialize_legacy(facet);
        assert_eq!(saved.field_clocks.len(), 6);
        assert_eq!(saved.entry, content);
    }
}

#[test]
fn oversized_mixed_or_future_clock_evidence_cannot_grant_ordering_authority() {
    let mut oversized = fields(false);
    oversized.extend(fields(true));
    let mut mixed = fields(false);
    mixed[0] = DetailField::ParticipantApproved;
    for clocks in [oversized, mixed] {
        let mut saved: StoredEntry = serde_json::from_value(encoded(clocks)).unwrap();
        let content = saved.entry.clone();
        saved.initialize_legacy(DetailFacet::Comments);
        assert!(saved.field_clocks.is_empty());
        assert_eq!(saved.entry, content);
    }
    let mut future = encoded(fields(false));
    future["field_clocks"][0]["field"] = serde_json::Value::String("future_authority".into());
    let mut saved: StoredEntry = serde_json::from_value(future).unwrap();
    saved.initialize_legacy(DetailFacet::Comments);
    assert!(saved.field_clocks.is_empty());
}
