//! Optional 201 metadata never decides whether required issue identity was created.
use crate::{
    IssueMetadataUnobservedReason as Reason,
    issue_metadata::native::{
        MetadataObservationV2, MilestoneObservationV2, SelectionV2, SetObservationV2,
    },
};
use serde_json::Value;
use std::collections::BTreeSet;

fn set(v: Option<&Value>, selected: Vec<&str>) -> SetObservationV2 {
    let unknown = |reason| SetObservationV2::Unobserved { reason };
    if selected.is_empty() {
        return unknown(Reason::Missing);
    }
    let Some(v) = v else {
        return unknown(Reason::Missing);
    };
    let Some(rows) = v.as_array() else {
        return unknown(Reason::Malformed);
    };
    if rows.len() > 1000 || serde_json::to_vec(v).map_or(true, |v| v.len() > 32 * 1024) {
        return unknown(Reason::Oversized);
    }
    let mut seen = BTreeSet::new();
    let mut present = BTreeSet::new();
    for row in rows {
        let Some(id) = row.get("id").and_then(Value::as_u64).filter(|id| *id > 0) else {
            return unknown(Reason::IdentityUnavailable);
        };
        let id = id.to_string();
        if !seen.insert(id.clone()) {
            return unknown(Reason::Malformed);
        }
        if selected.contains(&id.as_str()) {
            present.insert(id);
        }
    }
    SetObservationV2::Known {
        present_ids: present.into_iter().collect(),
    }
}
pub(crate) fn observe(v: &Value, selected: &SelectionV2) -> MetadataObservationV2 {
    let milestone = if selected.milestone.is_none() {
        MilestoneObservationV2::Unobserved {
            reason: Reason::Missing,
        }
    } else {
        match v.get("milestone") {
            None => MilestoneObservationV2::Unobserved {
                reason: Reason::Missing,
            },
            Some(Value::Null) => MilestoneObservationV2::Known { provider_id: None },
            Some(v) if serde_json::to_vec(v).map_or(true, |v| v.len() > 32 * 1024) => {
                MilestoneObservationV2::Unobserved {
                    reason: Reason::Oversized,
                }
            }
            Some(v) => match v.get("id").and_then(Value::as_u64).filter(|n| *n > 0) {
                Some(id) => MilestoneObservationV2::Known {
                    provider_id: Some(id.to_string()),
                },
                None => MilestoneObservationV2::Unobserved {
                    reason: Reason::IdentityUnavailable,
                },
            },
        }
    };
    MetadataObservationV2 {
        labels: set(
            v.get("labels"),
            selected.labels.iter().map(|v| v.id.as_str()).collect(),
        ),
        assignees: set(
            v.get("assignees"),
            selected.assignees.iter().map(|v| v.id.as_str()).collect(),
        ),
        milestone,
    }
}
