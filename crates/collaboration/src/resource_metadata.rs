//! Typed resource fields share one endpoint, but retain independent authority.
use crate::{RemoteItemKind, detail::DetailValueState};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MetadataField {
    Title,
    State,
    StateReason,
    Author,
    WebUrl,
    UpdatedAt,
    Labels,
    Assignees,
    Milestone,
    IsDraft,
    Head,
    Base,
    MergedAt,
}

impl MetadataField {
    pub const COMMON: [Self; 9] = [
        Self::Title,
        Self::State,
        Self::StateReason,
        Self::Author,
        Self::WebUrl,
        Self::UpdatedAt,
        Self::Labels,
        Self::Assignees,
        Self::Milestone,
    ];
    pub const PULL: [Self; 4] = [Self::IsDraft, Self::Head, Self::Base, Self::MergedAt];
    pub fn supports(self, kind: &RemoteItemKind) -> bool {
        Self::COMMON.contains(&self)
            || *kind == RemoteItemKind::PullRequest && Self::PULL.contains(&self)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DetailActor {
    pub provider_id: String,
    pub login: String,
    pub web_url: Option<String>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DetailLabel {
    /// Name-only labels have no observed immutable ID; never invent one.
    pub provider_id: Option<String>,
    pub name: String,
    pub color: Option<String>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DetailMilestone {
    pub provider_id: String,
    pub number: Option<String>,
    pub title: String,
    pub state: Option<String>,
    pub web_url: Option<String>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DetailRepositoryRef {
    pub provider_id: String,
    pub full_name: String,
    pub web_url: Option<String>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DetailBranch {
    pub name: String,
    pub oid: String,
    /// A deleted/unavailable fork need not erase its observed ref and OID.
    pub repository: Option<DetailRepositoryRef>,
}
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResourceMetadataValues {
    pub title: Option<String>,
    pub state: Option<String>,
    pub state_reason: Option<String>,
    pub author: Option<DetailActor>,
    pub web_url: Option<String>,
    pub updated_at: Option<String>,
    pub labels: Vec<DetailLabel>,
    pub assignees: Vec<DetailActor>,
    pub milestone: Option<DetailMilestone>,
    pub is_draft: Option<bool>,
    pub head: Option<DetailBranch>,
    pub base: Option<DetailBranch>,
    pub merged_at: Option<String>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MetadataSource {
    pub source: String,
    pub adapter_version: u32,
    pub provider_updated_at: Option<String>,
    pub observed_at: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MetadataFieldEvidence {
    pub field: MetadataField,
    pub saved_state: DetailValueState,
    pub observed_state: DetailValueState,
    pub validated_at: Option<String>,
    pub stale_at: Option<String>,
    pub source: Option<MetadataSource>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResourceMetadataSnapshot {
    pub kind: RemoteItemKind,
    pub values: ResourceMetadataValues,
    pub fields: Vec<MetadataFieldEvidence>,
}

/// Adapter-only observations. Engine owns validation times and saved authority.
#[derive(Debug, Clone)]
pub struct MetadataObservedField {
    pub field: MetadataField,
    pub state: DetailValueState,
}
#[derive(Debug, Clone)]
pub struct ResourceMetadataObservation {
    pub kind: RemoteItemKind,
    pub values: ResourceMetadataValues,
    pub fields: Vec<MetadataObservedField>,
    pub source: MetadataSource,
}
/// Captured at dispatch, never accepted from IPC.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DetailSubjectBinding {
    pub repository_id: String,
    pub repository_provider_id: String,
    pub provider_id: String,
    pub number: Option<String>,
    pub kind: RemoteItemKind,
    pub head_oid: Option<String>,
}
