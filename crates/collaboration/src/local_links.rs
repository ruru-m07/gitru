//! Local authored repository associations; these types grant no remote permission.
use crate::RemoteRepository;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LinkTransport {
    Https,
    Ssh,
    Scp,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LinkDirection {
    Fetch,
    Push,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RepositoryPathLayout {
    OwnerRepository,
    Subgroups,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalRemoteEndpoint {
    pub remote_name: String,
    pub direction: LinkDirection,
    pub ordinal: u32,
    pub transport: LinkTransport,
    pub host: String,
    pub port: u16,
    pub path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalTransportBinding {
    pub id: String,
    pub instance_id: String,
    pub transport: LinkTransport,
    pub host: String,
    pub port: u16,
    pub path_prefix: String,
    pub layout: RepositoryPathLayout,
    pub generation: String,
}

/// Constructed by the native caller from a trusted RepoManager registration and
/// a new safe Git observation, never accepted verbatim from JavaScript.
#[derive(Debug, Clone)]
pub struct LocalLinkQuery {
    pub local_repository_id: String,
    pub registration_proof: Option<String>,
    pub remote_digest: Option<String>,
    pub endpoints: Vec<LocalRemoteEndpoint>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LocalLinkState {
    Linked,
    Unresolved,
    Ambiguous,
    UnconfiguredInstance,
    UnsupportedTransport,
    RemoteChanged,
    LocalRepositoryMissing,
    Unavailable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalLinkCandidate {
    pub id: String,
    pub endpoint: LocalRemoteEndpoint,
    pub account_id: String,
    pub actor_id: String,
    pub authorization_epoch: String,
    pub instance_id: String,
    pub repository: RemoteRepository,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalEndpointResolution {
    pub endpoint: LocalRemoteEndpoint,
    pub state: LocalLinkState,
    pub candidates: Vec<LocalLinkCandidate>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalRepositoryLink {
    pub id: String,
    pub local_repository_id: String,
    pub endpoint: LocalRemoteEndpoint,
    pub account_id: String,
    pub actor_id: String,
    pub instance_id: String,
    pub repository_provider_id: String,
    pub repository_id: String,
    pub generation: String,
    pub state: LocalLinkState,
    /// Present only under current authorized repository visibility.
    pub repository: Option<RemoteRepository>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalLinkSnapshot {
    pub links: Vec<LocalRepositoryLink>,
    pub resolutions: Vec<LocalEndpointResolution>,
    pub bindings: Vec<LocalTransportBinding>,
    pub bindings_generation: String,
    pub revision: String,
    pub authorization_view: String,
}

#[derive(Debug, Clone)]
pub struct ConfirmLocalLink {
    pub query: LocalLinkQuery,
    pub candidate_id: String,
    pub expected_authorization_view: String,
    pub expected_bindings_generation: String,
    /// None creates a link; Some replaces exactly this existing generation.
    pub replace: Option<LocalLinkVersion>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalLinkVersion {
    pub id: String,
    pub generation: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalLinkWriteReceipt {
    pub link: LocalRepositoryLink,
    pub revision: String,
    pub authorization_view: String,
}

#[derive(Debug, Clone)]
pub struct SaveTransportBinding {
    pub instance_id: String,
    pub transport: LinkTransport,
    pub host: String,
    pub port: u16,
    pub path_prefix: String,
    pub layout: RepositoryPathLayout,
    pub expected_bindings_generation: String,
    pub replace: Option<LocalLinkVersion>,
}
