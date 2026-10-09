//! Operation-validated scalar intent; provider identity and authorization are immutable.
use crate::{CollaborationError, RemoteItem, RemoteItemKind};
use serde::{Deserialize, Serialize};

type Result<T> = std::result::Result<T, CollaborationError>;
pub(crate) const MAX_ACTIVE_EFFECTS: i64 = 64;
pub(crate) const EFFECT_VERSION: i64 = 1;

/// Explicit set-to-null differs from an absent patch field.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct BodyIntent {
    pub text: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ItemIntentPatch {
    pub title: Option<String>,
    pub body: Option<BodyIntent>,
    pub state: Option<String>,
    pub unread: Option<bool>,
}

impl ItemIntentPatch {
    pub(crate) fn validate(&self, kind: &RemoteItemKind) -> Result<()> {
        let valid = self.title.as_ref().is_none_or(|value| {
            !value.trim().is_empty() && value.len() <= 4096 && !value.contains('\0')
        }) && self
            .body
            .as_ref()
            .and_then(|value| value.text.as_ref())
            .is_none_or(|value| value.len() <= 65_536 && !value.contains('\0'))
            && self.state.as_ref().is_none_or(|value| {
                !value.is_empty()
                    && value.len() <= 64
                    && value
                        .bytes()
                        .all(|byte| byte.is_ascii_lowercase() || byte == b'_')
            })
            && (self.unread.is_none() || *kind == RemoteItemKind::Notification)
            && (*kind != RemoteItemKind::Notification || self.body.is_none())
            && !self.fields().is_empty();
        if !valid {
            return Err(CollaborationError::invalid(
                "Invalid bounded command effect",
            ));
        }
        Ok(())
    }
    pub(crate) fn fields(&self) -> Vec<IntentField> {
        let mut result = Vec::new();
        if self.title.is_some() {
            result.push(IntentField::Title);
        }
        if self.body.is_some() {
            result.push(IntentField::Body);
        }
        if self.state.is_some() {
            result.push(IntentField::State);
        }
        if self.unread.is_some() {
            result.push(IntentField::Unread);
        }
        result
    }
    pub(crate) fn apply(&self, item: &mut RemoteItem) {
        if let Some(value) = &self.title {
            item.title = value.clone();
        }
        if let Some(value) = &self.body {
            item.body = value.text.clone();
            item.body_omitted = false;
        }
        if let Some(value) = &self.state {
            item.state = value.clone();
            if let Some(crate::NativeInboxState::Todo { completion, .. }) = &mut item.native_inbox {
                if value == "done" {
                    *completion = crate::TodoCompletion::Done;
                } else if value == "pending" {
                    *completion = crate::TodoCompletion::Pending;
                }
            }
        }
        if let Some(value) = self.unread {
            item.unread = Some(value);
            if let Some(crate::NativeInboxState::Notification { unread }) = &mut item.native_inbox {
                *unread = value;
            }
        }
    }
    pub(crate) fn overlay(&mut self, next: Self) {
        if next.title.is_some() {
            self.title = next.title;
        }
        if next.body.is_some() {
            self.body = next.body;
        }
        if next.state.is_some() {
            self.state = next.state;
        }
        if next.unread.is_some() {
            self.unread = next.unread;
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IntentField {
    Title,
    Body,
    State,
    Unread,
    ReviewSubmission,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PendingCommandIntent {
    pub command_id: String,
    pub state: String,
    pub fields: Vec<IntentField>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PendingItemIntent {
    pub subject_id: String,
    pub commands: Vec<PendingCommandIntent>,
}
