//! Small, ephemeral read chains. They cannot authorize dispatch on their own.
use super::*;
use sha2::{Digest, Sha256};
use std::collections::HashSet;
const MAX_CHAINS: usize = 8;
const MAX_STEPS: u32 = 64;
const MAX_AGE: Duration = Duration::from_secs(120);

pub(crate) struct PendingPreparation {
    hash: [u8; 32],
    generation: i64,
    actor: String,
    epoch: String,
    instance: String,
    pub view: String,
    pub context: Vec<u8>,
    pub continuation: Option<Vec<u8>>,
    started: Instant,
    steps: u32,
    seen: HashSet<[u8; 32]>,
}
impl PendingPreparation {
    pub(super) fn new(request: &ReconcileRequest, view: String, now: Instant) -> Self {
        Self {
            hash: request.command.hash,
            generation: request.command.generation,
            actor: request.account.actor_id.clone(),
            epoch: request.account.authorization_epoch.clone(),
            instance: request.instance_id.clone(),
            view,
            context: request.native_context.clone(),
            continuation: None,
            started: now,
            steps: 0,
            seen: HashSet::new(),
        }
    }
    pub(super) fn current(
        &self,
        command: &DeliveryCommand,
        account: &RemoteAccount,
        instance: &str,
        now: Instant,
    ) -> bool {
        self.hash == command.hash
            && self.generation == command.generation
            && self.actor == account.actor_id
            && self.epoch == account.authorization_epoch
            && self.instance == instance
            && self.live(now)
    }
    pub(super) fn live(&self, now: Instant) -> bool {
        now.checked_duration_since(self.started)
            .is_some_and(|age| age < MAX_AGE)
            && self.steps < MAX_STEPS
    }
    pub(super) fn next(&mut self, bytes: Vec<u8>, now: Instant) -> bool {
        if !self.live(now) || bytes.is_empty() || bytes.len() > MAX_EVIDENCE_BYTES {
            return false;
        }
        self.steps += 1;
        if self.steps >= MAX_STEPS || !self.seen.insert(Sha256::digest(&bytes).into()) {
            return false;
        }
        self.continuation = Some(bytes);
        true
    }
}
#[derive(Default)]
pub(crate) struct PreparationChains {
    entries: HashMap<(String, String), PendingPreparation>,
}
impl PreparationChains {
    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        self.entries.len()
    }
    pub(super) fn take(&mut self, key: &(String, String)) -> Option<PendingPreparation> {
        self.entries.remove(key)
    }
    pub(super) fn put(
        &mut self,
        key: (String, String),
        chain: PendingPreparation,
        now: Instant,
    ) -> bool {
        self.entries.retain(|_, v| v.live(now));
        if self.entries.len() >= MAX_CHAINS {
            return false;
        }
        self.entries.insert(key, chain);
        true
    }
    pub(crate) fn remove(&mut self, account: &str, command: &str) {
        self.entries.remove(&(account.into(), command.into()));
    }
    pub(crate) fn remove_account(&mut self, account: &str) {
        self.entries.retain(|(a, _), _| a != account);
    }
}
