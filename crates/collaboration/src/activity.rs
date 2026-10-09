//! Historical activity is presentation evidence, never current workflow authority.
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ActivityEvent {
    pub kind: String,
    pub supported: bool,
    pub occurred_at: Option<String>,
    pub description: Option<String>,
}
impl ActivityEvent {
    pub(crate) fn valid(&self) -> bool {
        !self.kind.is_empty()
            && self.kind.len() <= 64
            && self
                .kind
                .bytes()
                .all(|b| b.is_ascii_lowercase() || matches!(b, b'_' | b'-'))
            && self
                .occurred_at
                .as_ref()
                .is_none_or(|s| s.len() <= 64 && chrono::DateTime::parse_from_rfc3339(s).is_ok())
            && self
                .description
                .as_ref()
                .is_none_or(|s| s.len() <= 1024 && !s.contains('\0'))
    }
}
