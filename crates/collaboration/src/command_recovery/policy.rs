//! Compiled operation codecs own comparison and replacement authority.
#![cfg_attr(not(test), allow(dead_code))]
use super::*;
use crate::storage::command_admission::CommandReceipt;
use crate::{CollaborationError, RemoteAccount, delivery::DeliveryCommand};
use sqlx::{Sqlite, Transaction};

pub(crate) const MAX_REVIEW_BYTES: usize = 262_144;
pub(crate) const MAX_ACTIONS: i64 = 64;
pub(crate) const MAX_REPLACEMENT_DEPTH: usize = 16;

#[derive(Clone, Debug, Serialize)]
pub(crate) struct NativeRecoveryReview {
    pub fields: Vec<CommandFieldReview>,
    pub can_replace: bool,
    pub reason: Option<String>,
    /// Exact additional native context (e.g. body revision/head/access binding),
    /// hashed into the CAS token but never sent to the renderer or exported.
    pub fence: Vec<u8>,
}
impl NativeRecoveryReview {
    pub fn unsupported() -> Self {
        Self {
            fields: vec![],
            can_replace: false,
            reason: Some("This command version has no installed recovery policy".into()),
            fence: vec![],
        }
    }
    pub fn validate(&self) -> Result<(), CollaborationError> {
        let encoded = serde_json::to_vec(self).map_err(|_| invalid())?;
        if self.fields.len() > 5
            || encoded.len() > MAX_REVIEW_BYTES
            || self.fence.len() > 65_536
            || self
                .reason
                .as_ref()
                .is_some_and(|r| r.len() > 1024 || r.contains('\0'))
        {
            return Err(invalid());
        }
        for (index, field) in self.fields.iter().enumerate() {
            if self.fields[..index]
                .iter()
                .any(|old| old.field == field.field)
                || field.field == CommandReviewField::Head && field.editable
            {
                return Err(invalid());
            }
            for value in [&field.base, &field.remote, &field.desired] {
                if !value.known && value.value.is_some()
                    || value
                        .value
                        .as_ref()
                        .is_some_and(|v| v.len() > 65_536 || v.contains('\0'))
                {
                    return Err(invalid());
                }
            }
            if field.comparison
                != compare_field(field.field, &field.base, &field.remote, &field.desired)
            {
                return Err(invalid());
            }
        }
        Ok(())
    }
}

/// Whole-field three-way comparison. Text is never merged by guessing line
/// ownership; workflow/head changes are review gates even when desired matches.
pub(crate) fn compare_field(
    field: CommandReviewField,
    base: &CommandFieldValue,
    remote: &CommandFieldValue,
    desired: &CommandFieldValue,
) -> CommandFieldComparison {
    if !base.known || !remote.known || !desired.known {
        return CommandFieldComparison::Unknown;
    }
    if matches!(field, CommandReviewField::Head | CommandReviewField::State) && remote != base {
        return CommandFieldComparison::GuardChanged;
    }
    if remote == desired {
        return CommandFieldComparison::Converged;
    }
    if desired == base {
        return CommandFieldComparison::Unchanged;
    }
    if remote == base {
        return CommandFieldComparison::Independent;
    }
    CommandFieldComparison::Conflict
}

#[async_trait::async_trait]
pub(crate) trait CommandRecoveryPolicy: Send + Sync {
    fn instance_id(&self) -> &str;
    fn operation_kind(&self) -> &'static str;
    fn payload_version(&self) -> u32;
    /// Local raw-provider observation only; no HTTP or vault use under SQLite.
    async fn review_in(
        &self,
        tx: &mut Transaction<'_, Sqlite>,
        command: &DeliveryCommand,
        account: &RemoteAccount,
    ) -> Result<NativeRecoveryReview, CollaborationError>;
    /// Use command_admission::admit_in with the concrete operation admission
    /// policy. The engine has tentatively retired the original in this same
    /// transaction to reserve its active-effect slot; the passed command/review
    /// describe the validated pre-retirement state. Any failure restores it.
    /// No commit here: the engine checks the new receipt and persists
    /// supersession/order/action evidence in this same transaction.
    async fn replace_in(
        &self,
        tx: &mut Transaction<'_, Sqlite>,
        command: &DeliveryCommand,
        account: &RemoteAccount,
        request: &CommandRecoveryReplaceRequest,
        review: &NativeRecoveryReview,
    ) -> Result<CommandReceipt, CollaborationError>;
}
fn invalid() -> CollaborationError {
    CollaborationError::invalid("Invalid or unbounded native command review")
}

#[cfg(test)]
mod tests {
    use super::*;
    fn text(value: &str) -> CommandFieldValue {
        CommandFieldValue {
            known: true,
            value: Some(value.into()),
        }
    }
    #[test]
    fn independent_fields_merge_but_overlapping_text_and_changed_guards_require_review() {
        assert_eq!(
            compare_field(
                CommandReviewField::Title,
                &text("old"),
                &text("old"),
                &text("mine")
            ),
            CommandFieldComparison::Independent
        );
        assert_eq!(
            compare_field(
                CommandReviewField::Body,
                &text("old"),
                &text("theirs"),
                &text("old")
            ),
            CommandFieldComparison::Unchanged
        );
        assert_eq!(
            compare_field(
                CommandReviewField::Body,
                &text("old"),
                &text("theirs"),
                &text("mine")
            ),
            CommandFieldComparison::Conflict
        );
        assert_eq!(
            compare_field(
                CommandReviewField::Title,
                &text("old"),
                &text("mine"),
                &text("mine")
            ),
            CommandFieldComparison::Converged
        );
        for field in [CommandReviewField::State, CommandReviewField::Head] {
            assert_eq!(
                compare_field(field, &text("old"), &text("changed"), &text("changed")),
                CommandFieldComparison::GuardChanged
            );
        }
    }
    #[test]
    fn unavailable_is_not_a_known_empty_body() {
        let unknown = CommandFieldValue {
            known: false,
            value: None,
        };
        let empty = CommandFieldValue {
            known: true,
            value: None,
        };
        assert_eq!(
            compare_field(CommandReviewField::Body, &unknown, &unknown, &empty),
            CommandFieldComparison::Unknown
        );
        assert_eq!(
            compare_field(CommandReviewField::Body, &empty, &empty, &text("new")),
            CommandFieldComparison::Independent
        );
    }
}
