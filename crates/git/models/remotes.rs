//! Credential-free transport observations. Raw Git configuration is never a DTO.
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RemoteTransport {
    Https,
    Ssh,
    Scp,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemoteEndpoint {
    pub transport: RemoteTransport,
    pub host: String,
    pub port: u16,
    pub path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SafeRemoteUrl {
    pub ordinal: u32,
    pub sanitized_url: Option<String>,
    pub endpoint: Option<RemoteEndpoint>,
    pub redacted: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SafeGitRemote {
    pub name: String,
    pub fetch_urls: Vec<SafeRemoteUrl>,
    pub push_urls: Vec<SafeRemoteUrl>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemoteSnapshot {
    pub remotes: Vec<SafeGitRemote>,
    /// SHA-256 of only the serialized, sanitized semantic observation.
    pub semantic_digest: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RemoteObservationError {
    Unavailable,
    InvalidConfiguration,
    UnsupportedLegacyConfiguration,
    LimitExceeded,
    Changed,
    Timeout,
}

impl std::fmt::Display for RemoteObservationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Unavailable => "Local Git remote configuration is unavailable",
            Self::InvalidConfiguration => "Local Git remote configuration is invalid",
            Self::UnsupportedLegacyConfiguration => {
                "Legacy Git remote configuration requires conversion before linking"
            }
            Self::LimitExceeded => "Local Git remote configuration exceeds the supported limit",
            Self::Changed => "Local Git remote configuration changed during inspection",
            Self::Timeout => "Local Git remote inspection timed out",
        })
    }
}
impl std::error::Error for RemoteObservationError {}
