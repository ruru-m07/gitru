//! Secrets stay inside the native runtime and are never part of a domain DTO.

use std::fmt;
use zeroize::Zeroize;

/// A credential whose debug representation is redacted and storage is wiped on drop.
/// Deliberately does not implement `Serialize` or `Display`.
pub struct SecretToken(String);

impl SecretToken {
    pub fn new(value: String) -> Result<Self, CredentialError> {
        if value.is_empty() || value.len() > 4096 || !value.bytes().all(|b| b.is_ascii_graphic()) {
            let mut value = value;
            value.zeroize();
            return Err(CredentialError::InvalidToken);
        }
        Ok(Self(value))
    }

    /// Only provider transport and a native credential vault should use this value.
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl Clone for SecretToken {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl fmt::Debug for SecretToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SecretToken([REDACTED])")
    }
}

impl Drop for SecretToken {
    fn drop(&mut self) {
        self.0.zeroize();
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CredentialError {
    InvalidToken,
    Unavailable,
}

impl fmt::Display for CredentialError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::InvalidToken => "Enter a nonempty valid provider token",
            Self::Unavailable => "The operating system credential store is unavailable",
        })
    }
}

impl std::error::Error for CredentialError {}

/// Implemented by the application using an OS credential store. Blocking native
/// calls are made on Tokio's blocking pool by the runtime, never on its scheduler.
/// Deletion is idempotent: an absent reference is success. A failed store may
/// still have written the reference, so the runtime retains cleanup evidence.
pub trait CredentialVault: Send + Sync + 'static {
    fn store(&self, credential_ref: &str, token: &SecretToken) -> Result<(), CredentialError>;
    fn load(&self, credential_ref: &str) -> Result<Option<SecretToken>, CredentialError>;
    fn delete(&self, credential_ref: &str) -> Result<(), CredentialError>;
}

/// Native cleanup metadata; deliberately absent from every IPC DTO.
pub struct CredentialCleanup {
    pub reference: String,
    pub attempts: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn credentials_are_redacted_and_reject_header_injection() {
        let token = SecretToken::new("ghp_secret".to_string()).unwrap();
        assert!(!format!("{token:?}").contains("ghp_secret"));
        for value in ["", " a", "a\r\nAuthorization: Bearer evil", "a\0b"] {
            assert!(SecretToken::new(value.to_string()).is_err());
        }
    }
}
