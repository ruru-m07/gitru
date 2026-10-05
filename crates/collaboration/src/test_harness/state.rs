use super::{files::OwnedRoot, *};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::{
    collections::VecDeque,
    sync::{Arc, Mutex, MutexGuard},
};
use tokio::sync::Notify;

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PersistentState {
    pub version: u32,
    pub run_nonce: String,
    pub generation: u64,
    pub prepared: bool,
    pub fixture: HarnessFixture,
    pub phase: HarnessPhase,
    pub utc_base: DateTime<Utc>,
    pub elapsed: u32,
}

impl PersistentState {
    pub fn open(root: &OwnedRoot, nonce: &str) -> Result<Self, CollaborationError> {
        if let Some(bytes) = root.read("harness-state.json", 4096)? {
            let state: Self = serde_json::from_slice(&bytes).map_err(|_| invalid())?;
            if state.version != 1
                || state.run_nonce != nonce
                || state.generation == 0
                || state.elapsed > 86_400
                || state.utc_base.timestamp() < 1_700_000_000
                || state.utc_base.timestamp() > 4_102_444_800
            {
                return Err(invalid());
            }
            return Ok(state);
        }
        let state = Self {
            version: 1,
            run_nonce: nonce.into(),
            generation: 1,
            prepared: false,
            fixture: HarnessFixture::Primary,
            phase: HarnessPhase::One,
            utc_base: Utc::now(),
            elapsed: 0,
        };
        root.write("harness-state.json", &state)?;
        Ok(state)
    }
}

pub(super) struct Gate {
    pub receipt: HarnessProviderGate,
    pub notify: Arc<Notify>,
}

pub(super) struct MemoryState {
    pub persistent: PersistentState,
    pub calls: VecDeque<HarnessProviderCall>,
    pub gates: Vec<Gate>,
    pub next_call: u64,
}

pub(super) struct SharedState(pub Mutex<MemoryState>);

impl SharedState {
    pub fn new(persistent: PersistentState) -> Arc<Self> {
        Arc::new(Self(Mutex::new(MemoryState {
            persistent,
            calls: VecDeque::new(),
            gates: vec![],
            next_call: 0,
        })))
    }

    pub fn lock(&self) -> MutexGuard<'_, MemoryState> {
        // A panicked test-controller task cannot turn later safe diagnostics
        // into an unrelated poison panic. Durable inputs are checked separately.
        self.0.lock().unwrap_or_else(|poison| poison.into_inner())
    }

    pub fn cancel_gates(&self) {
        for gate in &mut self.lock().gates {
            if matches!(
                gate.receipt.state,
                HarnessGateState::Armed | HarnessGateState::Held
            ) {
                gate.receipt.state = HarnessGateState::Cancelled;
                gate.notify.notify_waiters();
            }
        }
    }
}

impl Drop for SharedState {
    fn drop(&mut self) {
        self.cancel_gates();
    }
}
