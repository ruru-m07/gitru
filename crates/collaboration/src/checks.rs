//! Provider-independent current-head check and commit-status observations.
use crate::{
    CoverageState, DetailAvailability, DetailFacet, DetailFreshness, DetailSnapshot, DetailValue,
    NativeDetailPayload, SyncState,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CheckKind {
    CheckRun,
    CommitStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckContext {
    pub head_oid: String,
    pub source_repository_provider_id: String,
    pub metadata_facet_revision: String,
}

impl CheckContext {
    pub(crate) fn is_valid(&self) -> bool {
        crate::is_canonical_commit_oid(&self.head_oid)
            && !self.source_repository_provider_id.is_empty()
            && self.source_repository_provider_id.len() <= 512
            && !self
                .source_repository_provider_id
                .chars()
                .any(char::is_control)
            && self
                .metadata_facet_revision
                .parse::<u64>()
                .is_ok_and(|value| value > 0 && value.to_string() == self.metadata_facet_revision)
    }
}

/// Keep check-run status/conclusion separate from legacy commit-status state.
/// Provider strings remain available for future values; consumers must treat an
/// unknown value as non-authoritative rather than guessing a passing outcome.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CheckStateV1 {
    CheckRun {
        status: String,
        conclusion: Option<String>,
    },
    CommitStatus {
        state: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckV1 {
    pub kind: CheckKind,
    pub name: String,
    pub state: CheckStateV1,
    pub description: DetailValue,
    pub producer: Option<String>,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
    pub updated_at: Option<String>,
    /// Provider-native policy fact. It does not make a failed row universally
    /// merge-safe and is never folded into the common passed aggregate.
    pub allow_failure: Option<bool>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CheckAggregateState {
    Missing,
    Unavailable,
    Syncing,
    Stale,
    Partial,
    Empty,
    Pending,
    Failed,
    Passed,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckAggregate {
    pub state: CheckAggregateState,
    /// True only for an exact-current-head, complete and fresh saved set.
    pub authoritative: bool,
    pub total: u32,
}

fn row_state(value: &CheckV1) -> CheckAggregateState {
    match &value.state {
        CheckStateV1::CheckRun { status, conclusion } => {
            if status != "completed" {
                return if matches!(status.as_str(), "queued" | "in_progress" | "pending") {
                    CheckAggregateState::Pending
                } else {
                    CheckAggregateState::Unknown
                };
            }
            match conclusion.as_deref() {
                Some("success" | "neutral" | "skipped") => CheckAggregateState::Passed,
                Some("failure" | "cancelled" | "timed_out" | "action_required" | "stale") => {
                    CheckAggregateState::Failed
                }
                None => CheckAggregateState::Pending,
                Some(_) => CheckAggregateState::Unknown,
            }
        }
        CheckStateV1::CommitStatus { state } => match state.as_str() {
            "success" => CheckAggregateState::Passed,
            "failure" | "error" => CheckAggregateState::Failed,
            "pending" | "expected" => CheckAggregateState::Pending,
            _ => CheckAggregateState::Unknown,
        },
    }
}

impl DetailSnapshot {
    /// Returns a provider-independent presentation summary. It never grants
    /// merge or branch-protection authority.
    pub fn check_aggregate(&self, current_head: Option<&str>) -> Option<CheckAggregate> {
        if self.evidence.facet != DetailFacet::Checks {
            return None;
        }
        let total = u32::try_from(self.entries.len()).unwrap_or(u32::MAX);
        let non_authoritative = |state| CheckAggregate {
            state,
            authoritative: false,
            total,
        };
        if self.evidence.availability == DetailAvailability::Unavailable {
            return Some(non_authoritative(CheckAggregateState::Unavailable));
        }
        if self.evidence.availability == DetailAvailability::Missing {
            return Some(non_authoritative(CheckAggregateState::Missing));
        }
        if self.evidence.sync.state == SyncState::Syncing {
            return Some(non_authoritative(CheckAggregateState::Syncing));
        }
        let Some(current_head) = current_head.filter(|head| crate::is_canonical_commit_oid(head))
        else {
            return Some(non_authoritative(CheckAggregateState::Stale));
        };
        if self.evidence.freshness != DetailFreshness::Fresh
            || self
                .entries
                .iter()
                .any(|entry| entry.head_oid.as_deref() != Some(current_head))
        {
            return Some(non_authoritative(CheckAggregateState::Stale));
        }
        if self.evidence.coverage.state != CoverageState::Complete
            || self.evidence.availability != DetailAvailability::Ready
            || self.next_cursor.is_some()
        {
            return Some(non_authoritative(CheckAggregateState::Partial));
        }
        if self.entries.is_empty() {
            return Some(non_authoritative(CheckAggregateState::Empty));
        }
        let mut failed = false;
        let mut pending = false;
        let mut unknown = false;
        for entry in &self.entries {
            let Some(NativeDetailPayload::CheckV1(value)) = &entry.native else {
                unknown = true;
                continue;
            };
            match row_state(value) {
                CheckAggregateState::Failed => failed = true,
                CheckAggregateState::Pending => pending = true,
                CheckAggregateState::Unknown => unknown = true,
                _ => {}
            }
        }
        let aggregate = if failed {
            CheckAggregateState::Failed
        } else if pending {
            CheckAggregateState::Pending
        } else if unknown {
            CheckAggregateState::Unknown
        } else {
            CheckAggregateState::Passed
        };
        Some(CheckAggregate {
            state: aggregate,
            authoritative: !pending
                && !unknown
                && matches!(
                    aggregate,
                    CheckAggregateState::Passed | CheckAggregateState::Failed
                ),
            total,
        })
    }
}
