//! Bounded GitLab wire facts. This parser grants no current-head review authority.
use super::super::{invalid, resource_details};
use crate::{DetailActor, DetailValue, DetailValueState, providers::ProviderError};
use serde_json::{Map, Value};
use std::collections::HashSet;

const MAX_BODY_BYTES: usize = 65_536;
pub(super) const MAX_NOTES_PER_THREAD: usize = 50;
pub(super) const MAX_THREADS_PER_PAGE: usize = 50;

#[derive(Debug)]
pub(super) struct Approvals {
    pub approvers: Vec<Approver>,
    pub truncated: bool,
}
#[derive(Debug)]
pub(super) struct Approver {
    pub actor: DetailActor,
    pub approved_at: Option<String>,
}
#[derive(Debug)]
pub(super) struct Discussion {
    pub id: String,
    pub individual_note: bool,
    pub notes: Vec<Note>,
    pub note_count: u32,
}
#[derive(Debug)]
pub(super) struct Note {
    pub id: String,
    pub kind: Option<String>,
    pub body: DetailValue,
    pub author: Option<DetailActor>,
    pub system: bool,
    pub created_at: String,
    pub updated_at: String,
    pub resolvable: Option<bool>,
    pub resolved: Option<bool>,
    pub resolved_at: Option<String>,
    pub resolved_by: Option<DetailActor>,
    pub position: Option<Position>,
}
#[derive(Debug)]
pub(super) struct Position {
    pub kind: String,
    pub base_sha: Option<String>,
    pub start_sha: Option<String>,
    pub head_sha: Option<String>,
    pub old_path: Option<String>,
    pub new_path: Option<String>,
    pub old_line: Option<u32>,
    pub new_line: Option<u32>,
    pub range: Option<LineRange>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub x: Option<String>,
    pub y: Option<String>,
}
#[derive(Debug)]
pub(super) struct LineRange {
    pub start: LinePosition,
    pub end: LinePosition,
}
#[derive(Debug)]
pub(super) struct LinePosition {
    pub line_code: String,
    pub kind: String,
    pub old_line: Option<u32>,
    pub new_line: Option<u32>,
}

fn object(value: &Value) -> Result<&Map<String, Value>, ProviderError> {
    value.as_object().ok_or_else(invalid)
}
fn required<'a>(value: &'a Map<String, Value>, key: &str) -> Result<&'a Value, ProviderError> {
    value.get(key).ok_or_else(invalid)
}
fn text(value: &Value, max: usize) -> Result<String, ProviderError> {
    value
        .as_str()
        .filter(|value| {
            !value.is_empty() && value.len() <= max && !value.chars().any(char::is_control)
        })
        .map(str::to_owned)
        .ok_or_else(invalid)
}
fn optional<T>(
    value: &Map<String, Value>,
    key: &str,
    parse: impl FnOnce(&Value) -> Result<T, ProviderError>,
) -> Result<Option<T>, ProviderError> {
    match value.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(value) => parse(value).map(Some),
    }
}
fn boolean(value: &Value) -> Result<bool, ProviderError> {
    value.as_bool().ok_or_else(invalid)
}
fn count(value: &Value) -> Result<u32, ProviderError> {
    value
        .as_u64()
        .and_then(|value| u32::try_from(value).ok())
        .ok_or_else(invalid)
}
fn line(value: &Value) -> Result<u32, ProviderError> {
    count(value).and_then(|value| if value > 0 { Ok(value) } else { Err(invalid()) })
}
fn oid(value: &Value) -> Result<String, ProviderError> {
    let value = text(value, 64)?;
    if crate::is_canonical_commit_oid(&value) {
        Ok(value)
    } else {
        Err(invalid())
    }
}
fn actor(value: &Value) -> Result<DetailActor, ProviderError> {
    let value = object(value)?;
    // Avatar/web URLs are not needed to render an actor and never become request authority.
    Ok(DetailActor {
        provider_id: resource_details::id(required(value, "id")?)?.to_string(),
        login: text(required(value, "username")?, 255)?,
        web_url: None,
    })
}
fn body(value: &Value) -> Result<DetailValue, ProviderError> {
    let value = value.as_str().ok_or_else(invalid)?;
    Ok(if value.len() > MAX_BODY_BYTES {
        DetailValue {
            state: DetailValueState::Oversized,
            text: None,
        }
    } else {
        DetailValue {
            state: DetailValueState::Known,
            text: Some(value.into()),
        }
    })
}
fn coordinate(value: &Value) -> Result<String, ProviderError> {
    let Value::Number(number) = value else {
        return Err(invalid());
    };
    let text = number.to_string();
    if text.len() > 128 || number.as_f64().is_none_or(|value| !value.is_finite()) {
        return Err(invalid());
    }
    Ok(text)
}
fn position(value: &Value) -> Result<Position, ProviderError> {
    let value = object(value)?;
    Ok(Position {
        kind: text(required(value, "position_type")?, 128)?,
        base_sha: optional(value, "base_sha", oid)?,
        start_sha: optional(value, "start_sha", oid)?,
        head_sha: optional(value, "head_sha", oid)?,
        old_path: optional(value, "old_path", |v| text(v, 4096))?,
        new_path: optional(value, "new_path", |v| text(v, 4096))?,
        old_line: optional(value, "old_line", line)?,
        new_line: optional(value, "new_line", line)?,
        width: optional(value, "width", count)?,
        height: optional(value, "height", count)?,
        x: optional(value, "x", coordinate)?,
        y: optional(value, "y", coordinate)?,
        range: optional(value, "line_range", |v| {
            let v = object(v)?;
            Ok(LineRange {
                start: line_position(required(v, "start")?)?,
                end: line_position(required(v, "end")?)?,
            })
        })?,
    })
}
fn line_position(value: &Value) -> Result<LinePosition, ProviderError> {
    let value = object(value)?;
    Ok(LinePosition {
        line_code: text(required(value, "line_code")?, 256)?,
        kind: text(required(value, "type")?, 128)?,
        old_line: optional(value, "old_line", line)?,
        new_line: optional(value, "new_line", line)?,
    })
}

pub(super) fn approvals(
    bytes: &[u8],
    project: u64,
    subject: u64,
    iid: u64,
) -> Result<Approvals, ProviderError> {
    let value: Value = serde_json::from_slice(bytes).map_err(|_| invalid())?;
    let value = object(&value)?;
    if resource_details::id(required(value, "id")?)? != subject
        || resource_details::id(required(value, "iid")?)? != iid
        || resource_details::id(required(value, "project_id")?)? != project
    {
        return Err(invalid());
    }
    let rows = required(value, "approved_by")?
        .as_array()
        .ok_or_else(invalid)?;
    let mut seen = HashSet::new();
    let approvers = rows
        .iter()
        .take(50)
        .map(|row| {
            let row = object(row)?;
            let actor = actor(required(row, "user")?)?;
            if !seen.insert(actor.provider_id.clone()) {
                return Err(invalid());
            }
            Ok(Approver {
                actor,
                approved_at: optional(row, "approved_at", resource_details::time)?,
            })
        })
        .collect::<Result<_, _>>()?;
    // Aggregate approval policy is a different, plan-dependent representation.
    // Validate known scalar shapes but never infer it from the approver list.
    optional(value, "approved", boolean)?;
    optional(value, "approvals_required", count)?;
    optional(value, "approvals_left", count)?;
    Ok(Approvals {
        approvers,
        truncated: rows.len() > 50,
    })
}

pub(super) fn discussions(
    bytes: &[u8],
    project: u64,
    subject: u64,
    iid: u64,
) -> Result<Vec<Discussion>, ProviderError> {
    let values: Vec<Value> = serde_json::from_slice(bytes).map_err(|_| invalid())?;
    if values.len() > MAX_THREADS_PER_PAGE {
        return Err(invalid());
    }
    let mut threads = HashSet::new();
    let mut note_ids = HashSet::new();
    let mut remaining_notes = 50usize;
    values
        .iter()
        .map(|value| {
            let value = object(value)?;
            let id = text(required(value, "id")?, 256)?;
            if !threads.insert(id.clone()) {
                return Err(invalid());
            }
            let individual_note = boolean(required(value, "individual_note")?)?;
            let notes = required(value, "notes")?.as_array().ok_or_else(invalid)?;
            if notes.is_empty() || individual_note && notes.len() != 1 {
                return Err(invalid());
            }
            let note_count = u32::try_from(notes.len()).map_err(|_| invalid())?;
            let retained = notes.len().min(MAX_NOTES_PER_THREAD).min(remaining_notes);
            remaining_notes -= retained;
            let notes = notes
                .iter()
                .take(retained)
                .map(|value| {
                    let value = object(value)?;
                    if resource_details::id(required(value, "noteable_id")?)? != subject
                        || resource_details::id(required(value, "project_id")?)? != project
                        || required(value, "noteable_type")?.as_str() != Some("MergeRequest")
                        || optional(value, "noteable_iid", resource_details::id)?
                            .is_some_and(|native| native != iid)
                    {
                        return Err(invalid());
                    }
                    let id = resource_details::id(required(value, "id")?)?.to_string();
                    if !note_ids.insert(id.clone()) {
                        return Err(invalid());
                    }
                    Ok(Note {
                        id,
                        kind: optional(value, "type", |v| text(v, 128))?,
                        body: body(required(value, "body")?)?,
                        author: optional(value, "author", actor)?,
                        system: boolean(required(value, "system")?)?,
                        created_at: resource_details::time(required(value, "created_at")?)?,
                        updated_at: resource_details::time(required(value, "updated_at")?)?,
                        resolvable: optional(value, "resolvable", boolean)?,
                        resolved: optional(value, "resolved", boolean)?,
                        resolved_at: optional(value, "resolved_at", resource_details::time)?,
                        resolved_by: optional(value, "resolved_by", actor)?,
                        position: optional(value, "position", position)?,
                    })
                })
                .collect::<Result<_, _>>()?;
            Ok(Discussion {
                id,
                individual_note,
                notes,
                note_count,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    const HEAD: &str = "1111111111111111111111111111111111111111";
    fn approver() -> Value {
        json!({"user":{"id":9007199254740993_u64,"username":"actor"},"approved_at":"2026-10-08T00:00:00Z"})
    }
    fn approvals_json() -> Value {
        json!({"id":99,"iid":7,"project_id":2,"approved":true,"approvals_required":0,"approvals_left":0,"approved_by":[approver()]})
    }
    fn note() -> Value {
        json!({"id":9007199254741993_u64,"type":"DiffNote","noteable_id":99,"noteable_type":"MergeRequest","project_id":2,"noteable_iid":null,"body":"a note\nwith text","author":{"id":42,"username":"actor"},"system":false,"created_at":"2026-10-07T00:00:00Z","updated_at":"2026-10-08T00:00:00Z","resolvable":true,"resolved":false,"position":{"position_type":"text","base_sha":"2222222222222222222222222222222222222222","start_sha":"3333333333333333333333333333333333333333","head_sha":HEAD,"old_path":"old Δ.txt","new_path":"new Δ.txt","old_line":null,"new_line":4,"line_range":{"start":{"line_code":"native_start","type":"new","new_line":3,"old_line":null},"end":{"line_code":"native_end","type":"new","new_line":4,"old_line":null}}}})
    }
    fn discussion() -> Value {
        json!({"id":"native-discussion","individual_note":false,"notes":[note()]})
    }
    fn bytes(value: &Value) -> Vec<u8> {
        serde_json::to_vec(value).unwrap()
    }
    #[test]
    fn approvals_preserve_identity_counts_dates_and_absence_without_inventing_commit() {
        let result = approvals(&bytes(&approvals_json()), 2, 99, 7).unwrap();
        assert_eq!(result.approvers[0].actor.provider_id, "9007199254740993");
        assert_eq!(
            result.approvers[0].approved_at.as_deref(),
            Some("2026-10-08T00:00:00Z")
        );
        let mut value = approvals_json();
        for key in ["approved", "approvals_required", "approvals_left"] {
            value.as_object_mut().unwrap().remove(key);
        }
        value["approved_by"][0]
            .as_object_mut()
            .unwrap()
            .remove("approved_at");
        let result = approvals(&bytes(&value), 2, 99, 7).unwrap();
        assert!(result.approvers[0].approved_at.is_none());
    }
    #[test]
    fn approvals_reject_cross_parent_duplicates_and_bad_types() {
        for key in ["id", "iid", "project_id"] {
            let mut value = approvals_json();
            value[key] = json!(100);
            assert!(approvals(&bytes(&value), 2, 99, 7).is_err());
        }
        let mut value = approvals_json();
        value["approved_by"] = json!([approver(), approver()]);
        assert!(approvals(&bytes(&value), 2, 99, 7).is_err());
        for (key, bad) in [
            ("approved", json!("true")),
            ("approvals_left", json!(-1)),
            ("approvals_required", json!(u64::MAX)),
        ] {
            let mut value = approvals_json();
            value[key] = bad;
            assert!(approvals(&bytes(&value), 2, 99, 7).is_err());
        }
    }
    #[test]
    fn nested_notes_preserve_native_positions_and_independent_resolution() {
        let result = discussions(&bytes(&json!([discussion()])), 2, 99, 7).unwrap();
        let thread = &result[0];
        let note = &thread.notes[0];
        let position = note.position.as_ref().unwrap();
        assert_eq!(thread.id, "native-discussion");
        assert_eq!(thread.note_count, 1);
        assert!(!thread.individual_note);
        assert_eq!(note.id, "9007199254741993");
        assert_eq!(note.resolved, Some(false));
        assert_eq!(note.resolvable, Some(true));
        assert_ne!(position.base_sha, position.start_sha);
        assert_eq!(position.head_sha.as_deref(), Some(HEAD));
        assert_eq!(position.old_path.as_deref(), Some("old Δ.txt"));
        assert_eq!(position.range.as_ref().unwrap().start.new_line, Some(3));
        let mut value = discussion();
        value["notes"][0]
            .as_object_mut()
            .unwrap()
            .remove("resolved");
        value["notes"][0]["position"]["position_type"] = json!("future_image_variant");
        assert!(
            discussions(&bytes(&json!([value])), 2, 99, 7).unwrap()[0].notes[0]
                .resolved
                .is_none()
        );
    }
    #[test]
    fn notes_validate_each_native_identity_and_do_not_accept_empty_threads() {
        for key in ["noteable_id", "project_id", "noteable_iid"] {
            let mut value = discussion();
            value["notes"][0][key] = json!(100);
            assert!(discussions(&bytes(&json!([value])), 2, 99, 7).is_err());
        }
        let mut value = discussion();
        value["notes"] = json!([]);
        assert!(discussions(&bytes(&json!([value])), 2, 99, 7).is_err());
        let mut value = discussion();
        value["notes"] = json!([note(), note()]);
        assert!(discussions(&bytes(&json!([value])), 2, 99, 7).is_err());
        assert!(discussions(&bytes(&json!([discussion(), discussion()])), 2, 99, 7).is_err());
    }
    #[test]
    fn oversized_body_is_explicit_and_nested_note_truncation_is_visible() {
        let mut value = discussion();
        value["notes"][0]["body"] = json!("x".repeat(MAX_BODY_BYTES + 1));
        let result = discussions(&bytes(&json!([value.clone()])), 2, 99, 7).unwrap();
        assert_eq!(result[0].notes[0].body.state, DetailValueState::Oversized);
        assert!(result[0].notes[0].body.text.is_none());
        value["notes"] = (0..51)
            .map(|i| {
                let mut value = note();
                value["id"] = json!(100 + i);
                value
            })
            .collect();
        let result = discussions(&bytes(&json!([value])), 2, 99, 7).unwrap();
        assert_eq!(result[0].notes.len(), 50);
        assert_eq!(result[0].note_count, 51);
    }
    #[test]
    fn malformed_anchors_do_not_become_current_head_evidence() {
        for (key, bad) in [
            ("head_sha", json!("bad")),
            ("new_line", json!(0)),
            ("old_line", json!(-1)),
            ("new_path", json!("x\u{0}y")),
        ] {
            let mut value = discussion();
            value["notes"][0]["position"][key] = bad;
            assert!(discussions(&bytes(&json!([value])), 2, 99, 7).is_err());
        }
    }
    #[test]
    fn decimal_image_coordinates_and_missing_position_are_preserved() {
        let mut value = discussion();
        value["notes"][0]["position"] =
            json!({"position_type":"image","width":1920,"height":1080,"x":12.75,"y":-0.5});
        let result = discussions(&bytes(&json!([value.clone()])), 2, 99, 7).unwrap();
        let position = result[0].notes[0].position.as_ref().unwrap();
        assert_eq!(position.width, Some(1920));
        assert_eq!(position.x.as_deref(), Some("12.75"));
        assert_eq!(position.y.as_deref(), Some("-0.5"));
        assert!(position.head_sha.is_none());
        value["notes"][0]["position"]["x"] = json!("12.75");
        assert!(discussions(&bytes(&json!([value.clone()])), 2, 99, 7).is_err());
        value["notes"][0]["position"] = Value::Null;
        assert!(
            discussions(&bytes(&json!([value])), 2, 99, 7).unwrap()[0].notes[0]
                .position
                .is_none()
        );
    }
    #[test]
    fn page_budget_is_shared_across_threads_with_explicit_omission() {
        let mut first = discussion();
        first["notes"] = (0..50)
            .map(|i| {
                let mut value = note();
                value["id"] = json!(100 + i);
                value
            })
            .collect();
        let mut second = discussion();
        second["id"] = json!("next-thread");
        second["notes"][0]["id"] = json!(999);
        let result = discussions(&bytes(&json!([first, second])), 2, 99, 7).unwrap();
        assert_eq!(result[0].notes.len(), 50);
        assert_eq!(result[1].note_count, 1);
        assert!(result[1].notes.is_empty());
    }
}
