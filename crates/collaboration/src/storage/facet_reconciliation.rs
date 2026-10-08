//! Versioned native evidence in existing JSON columns; renderer models stay flat.
use super::*;
use crate::detail::*;

const VERSION: u32 = 1;
const DRIFT: &str = "Detail traversal representation changed";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct TraversalEvidence {
    version: u32,
    pub reconciliation: DetailReconciliation,
    pub head_oid: Option<String>,
    #[serde(default)]
    pub check_context: Option<crate::CheckContext>,
    #[serde(default)]
    pub review_context: Option<crate::ReviewContext>,
    pub starts_at_beginning: bool,
}
impl TraversalEvidence {
    fn valid(&self) -> bool {
        self.version == VERSION
            && self.head_oid.as_ref().is_none_or(|oid| {
                !oid.is_empty() && oid.len() <= 256 && !oid.chars().any(char::is_control)
            })
            && (self.reconciliation.head_scope != DetailHeadScope::CurrentHead
                || self.head_oid.is_some())
            && self
                .check_context
                .as_ref()
                .is_none_or(crate::CheckContext::is_valid)
            && self
                .review_context
                .as_ref()
                .is_none_or(crate::ReviewContext::is_valid)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct StoredSource {
    #[serde(flatten)]
    pub source: DetailSource,
    #[serde(default, deserialize_with = "read_traversal")]
    traversal_evidence: Option<TraversalEvidence>,
}
fn read_traversal<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> std::result::Result<Option<TraversalEvidence>, D::Error> {
    let value = Option::<serde_json::Value>::deserialize(deserializer)?;
    Ok(value
        .filter(|v| {
            v.get("version").and_then(serde_json::Value::as_u64) == Some(u64::from(VERSION))
        })
        .and_then(|value| serde_json::from_value(value).ok())
        .filter(TraversalEvidence::valid))
}
impl StoredSource {
    pub fn traversal(&self) -> Option<&TraversalEvidence> {
        self.traversal_evidence.as_ref().filter(|e| e.valid())
    }
}

pub(super) fn drift() -> CollaborationError {
    CollaborationError::new(ErrorCode::StaleView, DRIFT)
}
pub(crate) fn is_drift(error: &CollaborationError) -> bool {
    error.code == ErrorCode::StaleView && error.message == DRIFT
}

pub(super) fn source_for(page: &DetailCommit, old: Option<&StoredSource>) -> Result<StoredSource> {
    // Child enumeration evidence has no meaning for a singleton description.
    let reconciliation = if page.facet == DetailFacet::Body {
        DetailReconciliation {
            enumeration: DetailEnumeration::Uncertain,
            ..page.reconciliation
        }
    } else {
        page.reconciliation
    };
    let head_oid = if page.reconciliation.head_scope == DetailHeadScope::CurrentHead {
        let head = page
            .subject_binding
            .as_ref()
            .and_then(|binding| binding.head_oid.clone())
            .filter(|oid| !oid.is_empty() && oid.len() <= 256 && !oid.chars().any(char::is_control))
            .ok_or_else(invalid_detail)?;
        if page.entries.iter().any(|entry| {
            !entry.field_mask.contains(&DetailField::HeadOid)
                || entry.head_oid.as_ref() != Some(&head)
        }) {
            return Err(invalid_detail());
        }
        Some(head)
    } else {
        None
    };
    let check_context = if page.facet == DetailFacet::Checks {
        let context = page
            .check_context
            .clone()
            .filter(crate::CheckContext::is_valid)
            .ok_or_else(invalid_detail)?;
        if Some(&context.head_oid) != head_oid.as_ref() {
            return Err(invalid_detail());
        }
        Some(context)
    } else {
        if page.check_context.is_some() {
            return Err(invalid_detail());
        }
        None
    };
    let review_context = if matches!(
        page.facet,
        DetailFacet::ReviewSummaries | DetailFacet::ReviewThreads
    ) {
        let context = page
            .review_context
            .clone()
            .filter(crate::ReviewContext::is_valid)
            .ok_or_else(invalid_detail)?;
        if Some(&context.head_oid) != head_oid.as_ref() {
            return Err(invalid_detail());
        }
        Some(context)
    } else {
        if page.review_context.is_some() {
            return Err(invalid_detail());
        }
        None
    };
    let previous = old.and_then(StoredSource::traversal);
    if page.request_cursor.is_some()
        && (old.is_none_or(|old| {
            old.source.source != page.source.source
                || old.source.adapter_version != page.source.adapter_version
                || old.source.field_mask != page.source.field_mask
        }) || previous.is_none_or(|old| {
            // A terminal cap weakens an uncertain Activity traversal without
            // changing its identity or granting absence authority.
            let terminal_cap = page.facet == DetailFacet::Activity
                && page.complete
                && page.next_cursor.is_none()
                && !page.not_modified
                && old.reconciliation.enumeration == DetailEnumeration::Uncertain
                && reconciliation.enumeration == DetailEnumeration::Truncated
                && old.reconciliation.head_scope == reconciliation.head_scope;
            (old.reconciliation != reconciliation && !terminal_cap)
                || old.head_oid != head_oid
                || old.check_context != check_context
                || old.review_context != review_context
        }))
    {
        return Err(drift());
    }
    if page.not_modified
        && previous.is_none_or(|old| {
            old.reconciliation != reconciliation
                || old.head_oid != head_oid
                || old.check_context != check_context
                || old.review_context != review_context
        })
    {
        return Err(invalid_detail());
    }
    Ok(StoredSource {
        source: page.source.clone(),
        traversal_evidence: Some(TraversalEvidence {
            version: VERSION,
            reconciliation,
            head_oid,
            check_context,
            review_context,
            starts_at_beginning: page.request_cursor.is_none()
                || previous.is_some_and(|old| old.starts_at_beginning),
        }),
    })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct FieldClock {
    field: DetailField,
    source: String,
    adapter_version: u32,
    provider_updated_at: Option<String>,
    #[serde(default)]
    scope_head: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct StoredEntry {
    #[serde(flatten)]
    pub entry: DetailEntry,
    #[serde(default)]
    reconciliation_version: Option<u32>,
    #[serde(default, deserialize_with = "read_clocks")]
    field_clocks: Vec<FieldClock>,
}
fn read_clocks<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> std::result::Result<Vec<FieldClock>, D::Error> {
    struct BoundedClocks;
    impl<'de> serde::de::Visitor<'de> for BoundedClocks {
        type Value = Vec<FieldClock>;
        fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter.write_str("at most twelve native field clocks")
        }
        fn visit_seq<A: serde::de::SeqAccess<'de>>(
            self,
            mut sequence: A,
        ) -> std::result::Result<Self::Value, A::Error> {
            let mut clocks = Vec::with_capacity(12);
            let mut invalid = false;
            for _ in 0..12 {
                let Some(value) = sequence.next_element::<serde_json::Value>()? else {
                    return Ok(if invalid { vec![] } else { clocks });
                };
                match serde_json::from_value(value) {
                    Ok(clock) => clocks.push(clock),
                    Err(_) => invalid = true,
                }
            }
            // Ignore oversized future/corrupt evidence without collecting an
            // unbounded array; extra clocks cannot grant ordering authority.
            while sequence.next_element::<serde::de::IgnoredAny>()?.is_some() {
                invalid = true;
            }
            Ok(if invalid { vec![] } else { clocks })
        }
    }
    deserializer.deserialize_seq(BoundedClocks)
}
impl StoredEntry {
    pub fn from_entry(entry: DetailEntry) -> Self {
        Self {
            entry,
            reconciliation_version: Some(VERSION),
            field_clocks: vec![],
        }
    }
    fn valid_clock(clock: &FieldClock) -> bool {
        !clock.source.is_empty()
            && clock.source.len() <= 256
            && clock.adapter_version > 0
            && clock.provider_updated_at.as_ref().is_none_or(|at| {
                at.len() <= 128 && chrono::DateTime::parse_from_rfc3339(at).is_ok()
            })
            && clock.scope_head.as_ref().is_none_or(|head| {
                !head.is_empty() && head.len() <= 256 && !head.chars().any(char::is_control)
            })
    }
    pub fn initialize_legacy(&mut self, facet: DetailFacet) {
        if self.reconciliation_version != Some(VERSION) {
            self.field_clocks.clear();
            if self.reconciliation_version.is_none()
                && let Some(at) = &self.entry.updated_at
                && at.len() <= 128
                && let Some(updated) = self
                    .entry
                    .field_validations
                    .iter()
                    .find(|v| v.field == DetailField::UpdatedAt)
            {
                for validation in &self.entry.field_validations {
                    if validation.source == updated.source
                        && validation.adapter_version == updated.adapter_version
                    {
                        self.field_clocks.push(FieldClock {
                            field: validation.field,
                            source: validation.source.clone(),
                            adapter_version: validation.adapter_version,
                            provider_updated_at: Some(at.clone()),
                            scope_head: None,
                        });
                    }
                }
            }
        }
        if self.field_clocks.len() > facet.field_limit()
            || self
                .field_clocks
                .iter()
                .any(|clock| !Self::valid_clock(clock) || !clock.field.valid_for(facet))
            || self.field_clocks.iter().enumerate().any(|(i, clock)| {
                self.field_clocks[..i]
                    .iter()
                    .any(|old| old.field == clock.field)
            })
        {
            self.field_clocks.clear();
        }
        self.reconciliation_version = Some(VERSION);
    }
    pub fn older(
        &self,
        field: DetailField,
        at: Option<&str>,
        source: &DetailSource,
        head: Option<&str>,
    ) -> bool {
        self.field_clocks.iter().any(|clock| {
            clock.field == field
                && clock.source == source.source
                && clock.adapter_version == source.adapter_version
                && clock.scope_head.as_deref() == head
                && clock
                    .provider_updated_at
                    .as_deref()
                    .zip(at)
                    .is_some_and(|(old, new)| timestamp_older(new, old))
        })
    }
    pub fn observed(
        &mut self,
        field: DetailField,
        at: Option<&str>,
        source: &DetailSource,
        head: Option<&str>,
    ) {
        let old = self
            .field_clocks
            .iter()
            .find(|clock| {
                clock.field == field
                    && clock.source == source.source
                    && clock.adapter_version == source.adapter_version
                    && clock.scope_head.as_deref() == head
            })
            .and_then(|clock| clock.provider_updated_at.clone());
        self.field_clocks.retain(|clock| clock.field != field);
        self.field_clocks.push(FieldClock {
            field,
            source: source.source.clone(),
            adapter_version: source.adapter_version,
            provider_updated_at: at.map(str::to_owned).or(old),
            scope_head: head.map(str::to_owned),
        });
    }
    pub fn forget(&mut self, fields: &[DetailField]) {
        self.field_clocks
            .retain(|clock| !fields.contains(&clock.field));
        self.entry
            .field_validations
            .retain(|validation| !fields.contains(&validation.field));
    }
}

#[cfg(test)]
#[path = "facet_reconciliation_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "participant_reconciliation_tests.rs"]
mod participant_tests;

#[cfg(test)]
#[path = "task_reconciliation_tests.rs"]
mod task_tests;
