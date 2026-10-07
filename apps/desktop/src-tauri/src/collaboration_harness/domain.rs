//! Projected, finite app controls. Fixture content stays behind normal IPC.
use collaboration::test_harness::{
    HarnessActorManifest, HarnessCoreAction, HarnessCoreStatus, HarnessPhase,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HarnessAction {
    Core,
    CreateConcurrentChild,
    CloseConcurrentChild,
    ReloadConcurrentChild,
    HoldChildHints,
    DropChildHints,
    DeliverChildHintsReverse,
    ResumeHints,
    ArmItemRead,
    ArmBodyRead,
    ArmDraftRead,
    ReleaseLocalRead,
    CancelLocalReads,
    CheckpointBeforeCommit,
    CheckpointCommittedBeforeHint,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HarnessControlRequest {
    pub run_nonce: String,
    pub expected_generation: String,
    pub action: HarnessAction,
    pub core_action: Option<HarnessCoreAction>,
    pub gate_id: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HarnessStatusRequest {
    pub run_nonce: String,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HarnessViewRole {
    Main,
    ConcurrentChild,
    NormalTab,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HarnessViewManifest {
    pub run_nonce: String,
    pub session_id: String,
    pub scenario_generation: String,
    pub webview_label: String,
    pub role: HarnessViewRole,
    pub actors: Vec<HarnessActorManifest>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HarnessHintMode {
    Normal,
    Hold,
    Drop,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HarnessReadKind {
    Item,
    Body,
    Draft,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HarnessReadState {
    Armed,
    Held,
    Released,
    Cancelled,
    TimedOut,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HarnessQueryKind {
    Accounts,
    Repositories,
    ContextualCapabilities,
    Items,
    Item,
    Detail,
    Draft,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HarnessQueryTiming {
    pub sequence: String,
    pub webview_label: String,
    pub kind: HarnessQueryKind,
    pub elapsed_micros: String,
    pub result_count: u32,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HarnessLocalReadGate {
    pub gate_id: String,
    pub scenario_generation: String,
    pub webview_label: String,
    pub kind: HarnessReadKind,
    pub state: HarnessReadState,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HarnessCheckpointKind {
    BeforeCommit,
    CommittedBeforeHint,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HarnessCheckpoint {
    pub run_nonce: String,
    pub session_id: String,
    pub scenario_generation: String,
    pub kind: HarnessCheckpointKind,
    pub gate_id: Option<String>,
    pub committed_phase: Option<HarnessPhase>,
    pub committed_facet_revision: Option<String>,
    pub process_id: u32,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HarnessStatus {
    pub core: HarnessCoreStatus,
    pub child_label: Option<String>,
    pub hint_mode: HarnessHintMode,
    pub held_hint_revisions: Vec<String>,
    pub local_reads: Vec<HarnessLocalReadGate>,
    pub authorized_hydrate_requests: String,
    pub checkpoint: Option<HarnessCheckpoint>,
    pub process_id: u32,
    pub native_setup_started_epoch_ms: String,
    pub runtime_ready_epoch_ms: String,
    pub runtime_open_micros: String,
    pub performance_queries: Vec<HarnessQueryTiming>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HarnessReceipt {
    pub status: HarnessStatus,
    pub gate_id: Option<String>,
}
