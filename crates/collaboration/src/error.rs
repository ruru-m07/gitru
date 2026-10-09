use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    NotReady,
    InvalidInput,
    NotFound,
    AuthRequired,
    CredentialStoreUnavailable,
    PermissionDenied,
    RateLimited,
    Network,
    Provider,
    Storage,
    Unsupported,
    StaleView,
    Busy,
    LocalStateChanged,
}

/// Safe for IPC. Never construct its message from an HTTP body, URL or token.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CollaborationError {
    pub code: ErrorCode,
    pub message: String,
    pub retry_after_seconds: Option<u32>,
}

impl CollaborationError {
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            retry_after_seconds: None,
        }
    }

    pub fn invalid(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::InvalidInput, message)
    }

    pub fn storage() -> Self {
        Self::new(
            ErrorCode::Storage,
            "Local collaboration storage is unavailable",
        )
    }
}

impl std::fmt::Display for CollaborationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for CollaborationError {}
