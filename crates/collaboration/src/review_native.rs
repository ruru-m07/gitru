//! Typed provider facts that cannot be safely collapsed into common review anchors.
use crate::ReviewActor;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "provider", content = "value")]
pub enum ReviewThreadNativeV1 {
    #[serde(rename = "gitlab")]
    Gitlab(GitlabDiscussionNoteV1),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GitlabDiscussionNoteV1 {
    pub note_type: Option<String>,
    pub system: bool,
    pub individual_note: bool,
    pub resolvable: Option<bool>,
    pub resolved_at: Option<String>,
    pub resolved_by: Option<ReviewActor>,
    pub position: Option<GitlabReviewPositionV1>,
    pub observed_note_count: u32,
    pub retained_note_count: u32,
}

/// GitLab identifies the native diff version with three distinct SHAs. These
/// are not GitHub current/original commit anchors, and absent fields stay absent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GitlabReviewPositionV1 {
    pub position_type: String,
    pub base_oid: Option<String>,
    pub start_oid: Option<String>,
    pub head_oid: Option<String>,
    pub old_path: Option<String>,
    pub new_path: Option<String>,
    pub old_line: Option<u32>,
    pub new_line: Option<u32>,
    pub line_range: Option<GitlabReviewLineRangeV1>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    /// Finite native JSON decimal, preserved as text to avoid float equality or precision claims.
    pub x: Option<String>,
    pub y: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GitlabReviewLineRangeV1 {
    pub start: GitlabReviewLineV1,
    pub end: GitlabReviewLineV1,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GitlabReviewLineV1 {
    pub line_code: String,
    pub kind: String,
    pub old_line: Option<u32>,
    pub new_line: Option<u32>,
}

impl ReviewThreadNativeV1 {
    pub(crate) fn is_valid(&self) -> bool {
        match self {
            Self::Gitlab(note) => {
                crate::reviews::bounded_optional(&note.note_type, 128)
                    && note.observed_note_count > 0
                    && note.retained_note_count > 0
                    && note.retained_note_count <= 50
                    && note.retained_note_count <= note.observed_note_count
                    && (!note.individual_note || note.observed_note_count == 1)
                    && note.resolved_at.as_deref().is_none_or(timestamp)
                    && note
                        .resolved_by
                        .as_ref()
                        .is_none_or(crate::reviews::actor_valid)
                    && note
                        .position
                        .as_ref()
                        .is_none_or(GitlabReviewPositionV1::is_valid)
            }
        }
    }
}
fn timestamp(value: &str) -> bool {
    value.len() <= 128 && chrono::DateTime::parse_from_rfc3339(value).is_ok()
}
fn path(value: &Option<String>) -> bool {
    value
        .as_deref()
        .is_none_or(|value| crate::reviews::bounded_identity(value, 4096))
}
fn coordinate(value: &str) -> bool {
    value.len() <= 128
        && value.parse::<serde_json::Number>().is_ok_and(|number| {
            number.to_string() == value && number.as_f64().is_some_and(|value| value.is_finite())
        })
}
fn positive_lines(old: Option<u32>, new: Option<u32>) -> bool {
    [old, new].into_iter().flatten().all(|value| value > 0)
}
impl GitlabReviewPositionV1 {
    fn is_valid(&self) -> bool {
        crate::reviews::bounded_identity(&self.position_type, 128)
            && [&self.base_oid, &self.start_oid, &self.head_oid]
                .into_iter()
                .all(|value| value.as_deref().is_none_or(crate::is_canonical_commit_oid))
            && [&self.x, &self.y]
                .into_iter()
                .all(|value| value.as_deref().is_none_or(coordinate))
            && path(&self.old_path)
            && path(&self.new_path)
            && positive_lines(self.old_line, self.new_line)
            && self
                .line_range
                .as_ref()
                .is_none_or(|range| range.start.is_valid() && range.end.is_valid())
    }
}
impl GitlabReviewLineV1 {
    fn is_valid(&self) -> bool {
        crate::reviews::bounded_identity(&self.line_code, 256)
            && crate::reviews::bounded_identity(&self.kind, 128)
            && positive_lines(self.old_line, self.new_line)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn native() -> ReviewThreadNativeV1 {
        ReviewThreadNativeV1::Gitlab(GitlabDiscussionNoteV1 {
            note_type: Some("DiffNote".into()),
            system: false,
            individual_note: false,
            resolvable: Some(true),
            resolved_at: None,
            resolved_by: None,
            observed_note_count: 2,
            retained_note_count: 1,
            position: Some(GitlabReviewPositionV1 {
                position_type: "text".into(),
                base_oid: Some("a".repeat(40)),
                start_oid: Some("b".repeat(40)),
                head_oid: Some("c".repeat(40)),
                old_path: Some("old Δ.rs".into()),
                new_path: Some("new Δ.rs".into()),
                old_line: None,
                new_line: Some(9),
                line_range: None,
                width: None,
                height: None,
                x: None,
                y: None,
            }),
        })
    }
    #[test]
    fn tagged_native_roundtrip_retains_distinct_diff_version_shas() {
        let value = native();
        assert!(value.is_valid());
        let json = serde_json::to_value(&value).unwrap();
        assert_eq!(json["provider"], "gitlab");
        assert_ne!(
            json["value"]["position"]["base_oid"],
            json["value"]["position"]["start_oid"]
        );
        assert_eq!(
            serde_json::from_value::<ReviewThreadNativeV1>(json).unwrap(),
            value
        );
    }
    #[test]
    fn native_cache_validation_rejects_impossible_counts_and_unbounded_positions() {
        for (observed, retained, individual) in
            [(0, 0, false), (1, 2, false), (51, 51, false), (2, 1, true)]
        {
            let ReviewThreadNativeV1::Gitlab(mut value) = native();
            value.observed_note_count = observed;
            value.retained_note_count = retained;
            value.individual_note = individual;
            assert!(!ReviewThreadNativeV1::Gitlab(value).is_valid());
        }
        for bad in ["", "not-a-sha", "a".repeat(65).as_str()] {
            let ReviewThreadNativeV1::Gitlab(mut value) = native();
            value.position.as_mut().unwrap().head_oid = Some(bad.into());
            assert!(!ReviewThreadNativeV1::Gitlab(value).is_valid());
        }
        let ReviewThreadNativeV1::Gitlab(mut value) = native();
        value.position.as_mut().unwrap().new_path = Some("x".repeat(4097));
        assert!(!ReviewThreadNativeV1::Gitlab(value).is_valid());
    }
    #[test]
    fn nullable_native_evidence_is_not_replaced_with_defaults() {
        let ReviewThreadNativeV1::Gitlab(mut value) = native();
        value.position = None;
        value.note_type = None;
        value.resolvable = None;
        assert!(ReviewThreadNativeV1::Gitlab(value).is_valid());
        for (coordinate, valid) in [
            ("12.75", true),
            ("-0.5", true),
            ("NaN", false),
            ("1e9999", false),
            ("01", false),
        ] {
            assert_eq!(super::coordinate(coordinate), valid);
        }
    }
}
