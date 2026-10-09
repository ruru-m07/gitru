//! Bounded, feature-only control receipts. None of these inputs carry authority
//! to choose a path, token, provider endpoint, subject or response program.
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HarnessActorSlot {
    Primary,
    Alternate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HarnessFixture {
    Primary,
    RepositoryOnly,
    Performance,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HarnessPhase {
    One,
    Two,
    Offline,
    Denied,
    RateLimited,
    NotModified,
    VaultUnavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HarnessCoreAction {
    PreparePrimary,
    PrepareRepositoryOnly,
    PreparePerformance,
    PhaseOne,
    PhaseTwo,
    PhaseOffline,
    PhaseDenied,
    PhaseRateLimited,
    PhaseNotModified,
    PhaseVaultUnavailable,
    ArmProviderGate,
    ReleaseProviderGate,
    AdvanceRefresh,
    AdvanceLeaseExpiry,
    AdvanceCooldown,
    FillCatchup,
    FillRetention,
    CancelGates,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HarnessCoreRequest {
    pub run_nonce: String,
    pub expected_generation: String,
    pub action: HarnessCoreAction,
    pub gate_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HarnessActorManifest {
    pub slot: HarnessActorSlot,
    pub account_id: String,
    pub authorization_epoch: String,
    pub instance_id: String,
    pub repository_id: String,
    pub subject_id: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HarnessCallState {
    Held,
    Completed,
    Cancelled,
    TimedOut,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HarnessProviderCall {
    pub call_id: String,
    pub scenario_generation: String,
    pub slot: HarnessActorSlot,
    pub authorization_epoch: String,
    pub facet: String,
    pub head_oid: Option<String>,
    pub phase: HarnessPhase,
    pub state: HarnessCallState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HarnessGateState {
    Armed,
    Held,
    Released,
    Cancelled,
    TimedOut,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HarnessProviderGate {
    pub gate_id: String,
    pub scenario_generation: String,
    pub state: HarnessGateState,
    pub call_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HarnessCoreStatus {
    pub run_nonce: String,
    pub session_id: String,
    pub scenario_generation: String,
    pub prepared: bool,
    pub fixture: HarnessFixture,
    pub phase: HarnessPhase,
    pub revision: String,
    pub actors: Vec<HarnessActorManifest>,
    pub calls: Vec<HarnessProviderCall>,
    pub gates: Vec<HarnessProviderGate>,
    pub provider_call_count: String,
    pub vault_load_count: String,
    pub vault_store_count: String,
    pub vault_delete_count: String,
    pub vault_unavailable_count: String,
    pub durable_detail_requests: u32,
    pub demand_lease_count: u32,
    pub clock_elapsed_seconds: u32,
    pub committed_phase: Option<HarnessPhase>,
    pub committed_facet_revision: Option<String>,
    pub performance: Option<HarnessPerformanceFixture>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HarnessPerformanceFixture {
    pub dataset_version: u32,
    pub item_count: u32,
    pub account_count: u32,
    pub repositories_per_account: u32,
    pub items_per_account: u32,
    pub search_sample_count: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HarnessCoreReceipt {
    pub status: HarnessCoreStatus,
    pub gate_id: Option<String>,
}
