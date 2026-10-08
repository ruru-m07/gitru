//! Retained, finite native qualification fixtures. This module is compiled only
//! with `test-harness`; default production setup cannot construct its adapter.
mod clock;
mod domain;
mod files;
mod performance;
mod provider;
mod state;
mod vault;

pub(crate) use clock::HarnessClock;
pub use domain::*;
pub use files::APPLICATION_ID;
pub use performance::{
    PERFORMANCE_DATASET_VERSION, PERFORMANCE_ITEMS_PER_ACCOUNT,
    PERFORMANCE_REPOSITORIES_PER_ACCOUNT, PERFORMANCE_SEARCH_SAMPLE_COUNT, PERFORMANCE_TOTAL_ITEMS,
    performance_item, performance_repository,
};
pub use provider::{
    ALTERNATE_ACCOUNT, BASE_OID, BOOKMARKED_NOTIFICATION_ID, DONE_NOTIFICATION_ID,
    FIRST_COMMIT_OID, HEAD_OID, PRIMARY_ACCOUNT, REPOSITORY_ID, SNOOZED_NOTIFICATION_ID,
    SOURCE_REPOSITORY_PROVIDER_ID, SUBJECT_ID, account_id, fixture_body,
};

use crate::{credentials::CredentialVault, *};
use files::OwnedRoot;
use state::{Gate, PersistentState, SharedState};
use std::{
    path::Path,
    sync::{Arc, atomic::Ordering},
};
use tokio::sync::{Mutex, Notify};

fn invalid() -> CollaborationError {
    CollaborationError::invalid("Invalid native collaboration fixture input or root")
}
fn stale() -> CollaborationError {
    CollaborationError::new(ErrorCode::StaleView, "The native fixture scenario changed")
}

/// Native setup passes an already validated launch-owned root and the actual
/// physical-visibility callback. Neither can originate in an IPC request.
pub struct HarnessSession {
    pub runtime: Arc<CollaborationRuntime>,
    pub store: Arc<Store>,
    pub control: HarnessControl,
}

impl HarnessSession {
    pub async fn open(
        root: &Path,
        run_nonce: &str,
        visibility: Arc<dyn Fn(&str) -> bool + Send + Sync>,
    ) -> Result<Self, CollaborationError> {
        let root = OwnedRoot::validate(root, run_nonce)?;
        let database = root.file("collaboration.sqlite")?;
        // Losing the scenario marker must never reset an existing cache/vault.
        if database.exists() && root.read("harness-state.json", 4096)?.is_none() {
            return Err(invalid());
        }
        let persistent = PersistentState::open(&root, run_nonce)?;
        let clock = HarnessClock::new(persistent.utc_base, persistent.elapsed);
        let shared = SharedState::new(persistent.clone());
        let store = Arc::new(Store::open(&database).await?);
        let validated_vault = async {
            let accounts = store.accounts().await?.accounts;
            if !persistent.prepared && !accounts.is_empty() {
                return Err(CollaborationError::new(
                    ErrorCode::NotReady,
                    "An interrupted fixture preparation was preserved",
                ));
            }
            if persistent.prepared
                && (accounts.len() != 2
                    || accounts.iter().any(|account| {
                        [HarnessActorSlot::Primary, HarnessActorSlot::Alternate]
                            .into_iter()
                            .all(|slot| {
                                let fixture = provider::account(slot);
                                fixture.id != account.id
                                    || fixture.actor_id != account.actor_id
                                    || fixture.provider != account.provider
                                    || fixture.host != account.host
                            })
                    }))
            {
                return Err(invalid());
            }
            vault::FixtureVault::new(root.clone(), run_nonce, shared.clone())
        }
        .await;
        let vault = match validated_vault {
            Ok(vault) => Arc::new(vault),
            Err(error) => {
                // A returned startup error must leave the root immediately
                // reopenable. Drop alone only schedules conservative shutdown.
                store.close().await?;
                return Err(error);
            }
        };
        let provider = Arc::new(provider::FixtureProvider {
            shared: shared.clone(),
            clock: clock.clone(),
        });
        let runtime = Arc::new(
            CollaborationRuntime::new(store.clone(), vault.clone(), provider)
                .with_harness_clock(clock.clone())
                .with_demand_visibility_probe(visibility),
        );
        let control = HarnessControl(Arc::new(ControlInner {
            root,
            nonce: run_nonce.into(),
            session_id: uuid::Uuid::new_v4().to_string(),
            runtime: runtime.clone(),
            store: store.clone(),
            shared,
            vault,
            clock,
            operations: Mutex::new(()),
        }));
        Ok(Self {
            runtime,
            store,
            control,
        })
    }
}

struct ControlInner {
    root: OwnedRoot,
    nonce: String,
    session_id: String,
    runtime: Arc<CollaborationRuntime>,
    store: Arc<Store>,
    shared: Arc<SharedState>,
    vault: Arc<vault::FixtureVault>,
    clock: Arc<HarnessClock>,
    operations: Mutex<()>,
}

impl Drop for ControlInner {
    fn drop(&mut self) {
        self.shared.cancel_gates();
    }
}

#[derive(Clone)]
pub struct HarnessControl(Arc<ControlInner>);

impl HarnessControl {
    pub async fn status(&self, run_nonce: &str) -> Result<HarnessCoreStatus, CollaborationError> {
        if run_nonce != self.0.nonce {
            return Err(invalid());
        }
        self.0.root.check()?;
        self.snapshot().await
    }

    pub async fn execute(
        &self,
        request: HarnessCoreRequest,
    ) -> Result<HarnessCoreReceipt, CollaborationError> {
        self.execute_checked(request, || Ok(())).await
    }

    /// Native caller checks can be repeated after the serialized control wait.
    /// This cannot replace the app's exact caller/origin/incarnation policy.
    pub async fn execute_checked<F>(
        &self,
        request: HarnessCoreRequest,
        guard: F,
    ) -> Result<HarnessCoreReceipt, CollaborationError>
    where
        F: Fn() -> Result<(), CollaborationError> + Send + Sync,
    {
        if request.run_nonce != self.0.nonce {
            return Err(invalid());
        }
        let _operation = self.0.operations.lock().await;
        guard()?;
        self.0.root.check()?;
        {
            let state = self.0.shared.lock();
            if request.expected_generation != state.persistent.generation.to_string() {
                return Err(stale());
            }
            if request.action != HarnessCoreAction::ReleaseProviderGate && request.gate_id.is_some()
            {
                return Err(invalid());
            }
            if !state.persistent.prepared
                && !matches!(
                    request.action,
                    HarnessCoreAction::PreparePrimary
                        | HarnessCoreAction::PrepareRepositoryOnly
                        | HarnessCoreAction::PreparePerformance
                        | HarnessCoreAction::CancelGates
                )
            {
                return Err(CollaborationError::new(
                    ErrorCode::NotReady,
                    "Prepare the finite native fixture first",
                ));
            }
        }
        let mut issued_gate = None;
        match request.action {
            HarnessCoreAction::PreparePrimary
            | HarnessCoreAction::PrepareRepositoryOnly
            | HarnessCoreAction::PreparePerformance => {
                if self.0.shared.lock().persistent.prepared {
                    return Err(stale());
                }
                let fixture = match request.action {
                    HarnessCoreAction::PreparePrimary => HarnessFixture::Primary,
                    HarnessCoreAction::PrepareRepositoryOnly => HarnessFixture::RepositoryOnly,
                    HarnessCoreAction::PreparePerformance => HarnessFixture::Performance,
                    _ => unreachable!(),
                };
                self.prepare(fixture, &guard).await?;
            }
            HarnessCoreAction::PhaseOne
            | HarnessCoreAction::PhaseTwo
            | HarnessCoreAction::PhaseOffline
            | HarnessCoreAction::PhaseDenied
            | HarnessCoreAction::PhaseRateLimited
            | HarnessCoreAction::PhaseNotModified
            | HarnessCoreAction::PhaseVaultUnavailable => {
                let phase = match request.action {
                    HarnessCoreAction::PhaseOne => HarnessPhase::One,
                    HarnessCoreAction::PhaseTwo => HarnessPhase::Two,
                    HarnessCoreAction::PhaseOffline => HarnessPhase::Offline,
                    HarnessCoreAction::PhaseDenied => HarnessPhase::Denied,
                    HarnessCoreAction::PhaseRateLimited => HarnessPhase::RateLimited,
                    HarnessCoreAction::PhaseVaultUnavailable => HarnessPhase::VaultUnavailable,
                    _ => HarnessPhase::NotModified,
                };
                let mut next = self.0.shared.lock().persistent.clone();
                next.phase = phase;
                next.generation = next.generation.checked_add(1).ok_or_else(invalid)?;
                guard()?;
                self.0.root.write("harness-state.json", &next)?;
                self.0.shared.cancel_gates();
                self.0.shared.lock().persistent = next;
                self.0.runtime.harness_wake();
            }
            HarnessCoreAction::ArmProviderGate => {
                let mut state = self.0.shared.lock();
                state.gates.retain(|gate| {
                    matches!(
                        gate.receipt.state,
                        HarnessGateState::Armed | HarnessGateState::Held
                    )
                });
                if state.gates.len() >= 2 {
                    return Err(CollaborationError::new(
                        ErrorCode::Busy,
                        "The finite native fixture gates are full",
                    ));
                }
                let id = uuid::Uuid::new_v4().to_string();
                let generation = state.persistent.generation.to_string();
                state.gates.push(Gate {
                    receipt: HarnessProviderGate {
                        gate_id: id.clone(),
                        scenario_generation: generation,
                        state: HarnessGateState::Armed,
                        call_id: None,
                    },
                    notify: Arc::new(Notify::new()),
                });
                issued_gate = Some(id);
            }
            HarnessCoreAction::ReleaseProviderGate => {
                let id = request.gate_id.as_deref().ok_or_else(invalid)?;
                let mut state = self.0.shared.lock();
                let generation = state.persistent.generation.to_string();
                let gate = state
                    .gates
                    .iter_mut()
                    .find(|gate| {
                        gate.receipt.gate_id == id
                            && gate.receipt.scenario_generation == generation
                            && matches!(
                                gate.receipt.state,
                                HarnessGateState::Armed | HarnessGateState::Held
                            )
                    })
                    .ok_or_else(stale)?;
                gate.receipt.state = HarnessGateState::Released;
                gate.notify.notify_waiters();
            }
            HarnessCoreAction::CancelGates => self.0.shared.cancel_gates(),
            HarnessCoreAction::AdvanceRefresh
            | HarnessCoreAction::AdvanceLeaseExpiry
            | HarnessCoreAction::AdvanceCooldown => {
                let seconds = match request.action {
                    HarnessCoreAction::AdvanceRefresh => 6,
                    HarnessCoreAction::AdvanceLeaseExpiry => 46,
                    _ => 61,
                };
                let mut next = self.0.shared.lock().persistent.clone();
                next.elapsed = next
                    .elapsed
                    .checked_add(seconds)
                    .filter(|value| *value <= 86_400)
                    .ok_or_else(invalid)?;
                guard()?;
                self.0.root.write("harness-state.json", &next)?;
                self.0.clock.set_elapsed(next.elapsed);
                self.0.shared.lock().persistent = next;
                self.0.runtime.harness_wake();
            }
            HarnessCoreAction::FillCatchup | HarnessCoreAction::FillRetention => {
                self.fill(
                    if request.action == HarnessCoreAction::FillCatchup {
                        300
                    } else {
                        4100
                    },
                    &guard,
                )
                .await?;
            }
        }
        Ok(HarnessCoreReceipt {
            status: self.snapshot().await?,
            gate_id: issued_gate,
        })
    }

    async fn prepare<F>(&self, fixture: HarnessFixture, guard: &F) -> Result<(), CollaborationError>
    where
        F: Fn() -> Result<(), CollaborationError> + Send + Sync,
    {
        if !self.0.store.accounts().await?.accounts.is_empty() {
            return Err(stale());
        }
        let at = self.0.clock.utc_time().to_rfc3339();
        for slot in [HarnessActorSlot::Primary, HarnessActorSlot::Alternate] {
            guard()?;
            let account = provider::account(slot);
            let reference = self.0.vault.reference(slot);
            self.0
                .store
                .stage_credential(&account.id, reference)
                .await?;
            self.0
                .vault
                .store(reference, &vault::token(slot))
                .map_err(|_| {
                    CollaborationError::new(
                        ErrorCode::CredentialStoreUnavailable,
                        "The native synthetic fixture vault is unavailable",
                    )
                })?;
            guard()?;
            let account = self
                .0
                .store
                .commit_account_credential(account, reference)
                .await?;
            if fixture == HarnessFixture::Performance {
                self.prepare_performance_account(slot, &account, &guard)
                    .await?;
                continue;
            }
            let scope = "repositories";
            let run = self
                .0
                .store
                .begin_sync(&account.id, &account.authorization_epoch, scope)
                .await?;
            self.0
                .store
                .apply_page(PageCommit {
                    account_id: account.id.clone(),
                    authorization_epoch: account.authorization_epoch.clone(),
                    scope: scope.into(),
                    run_id: run,
                    repositories: vec![provider::repository(&account)],
                    items: vec![],
                    endpoint_aliases: vec![],
                    next_cursor: None,
                    etag: None,
                    last_modified: None,
                    not_modified: false,
                    complete: true,
                    observed_at: at.clone(),
                })
                .await?;
            self.0
                .store
                .select_repository(&account.id, REPOSITORY_ID, true)
                .await?;
            if slot == HarnessActorSlot::Primary {
                let scope = "notifications";
                let run = self
                    .0
                    .store
                    .begin_sync(&account.id, &account.authorization_epoch, scope)
                    .await?;
                self.0
                    .store
                    .apply_page(PageCommit {
                        account_id: account.id.clone(),
                        authorization_epoch: account.authorization_epoch.clone(),
                        scope: scope.into(),
                        run_id: run,
                        repositories: vec![],
                        items: provider::notifications(&account, &at),
                        endpoint_aliases: vec![],
                        next_cursor: None,
                        etag: None,
                        last_modified: None,
                        not_modified: false,
                        complete: true,
                        observed_at: at.clone(),
                    })
                    .await?;
            }
            for (scope, items) in [
                (
                    format!("repo:{REPOSITORY_ID}:pull_request"),
                    vec![provider::subject(&account, &at)],
                ),
                (format!("repo:{REPOSITORY_ID}:issue"), vec![]),
            ] {
                guard()?;
                let run = self
                    .0
                    .store
                    .begin_sync(&account.id, &account.authorization_epoch, &scope)
                    .await?;
                self.0
                    .store
                    .apply_page(PageCommit {
                        account_id: account.id.clone(),
                        authorization_epoch: account.authorization_epoch.clone(),
                        scope,
                        run_id: run,
                        repositories: vec![],
                        items,
                        endpoint_aliases: vec![],
                        next_cursor: None,
                        etag: None,
                        last_modified: None,
                        not_modified: false,
                        complete: true,
                        observed_at: at.clone(),
                    })
                    .await?;
            }
            guard()?;
            self.0
                .store
                .save_draft(LocalDraft {
                    account_id: account.id.clone(),
                    subject_id: SUBJECT_ID.into(),
                    body: if slot == HarnessActorSlot::Primary {
                        "RURU-103 primary private draft — α 🔒"
                    } else {
                        "RURU-103 alternate private draft — β 🔐"
                    }
                    .into(),
                    generation: "0".into(),
                })
                .await?;
        }
        let mut next = self.0.shared.lock().persistent.clone();
        next.prepared = true;
        next.fixture = fixture;
        next.generation = next.generation.checked_add(1).ok_or_else(invalid)?;
        guard()?;
        self.0.root.write("harness-state.json", &next)?;
        self.0.shared.lock().persistent = next;
        self.0.runtime.harness_publish_current().await
    }

    async fn prepare_performance_account<F>(
        &self,
        slot: HarnessActorSlot,
        account: &RemoteAccount,
        guard: &F,
    ) -> Result<(), CollaborationError>
    where
        F: Fn() -> Result<(), CollaborationError> + Send + Sync,
    {
        let observed_at = "2026-01-01T00:00:00Z";
        let repositories = (0..PERFORMANCE_REPOSITORIES_PER_ACCOUNT)
            .map(|index| performance_repository(slot, account, index))
            .collect::<Vec<_>>();
        let run = self
            .0
            .store
            .begin_sync(&account.id, &account.authorization_epoch, "repositories")
            .await?;
        self.0
            .store
            .apply_page(PageCommit {
                account_id: account.id.clone(),
                authorization_epoch: account.authorization_epoch.clone(),
                scope: "repositories".into(),
                run_id: run,
                repositories: repositories.clone(),
                items: vec![],
                endpoint_aliases: vec![],
                next_cursor: None,
                etag: None,
                last_modified: None,
                not_modified: false,
                complete: true,
                observed_at: observed_at.into(),
            })
            .await?;
        let items_per_repository =
            PERFORMANCE_ITEMS_PER_ACCOUNT / PERFORMANCE_REPOSITORIES_PER_ACCOUNT as usize;
        for (repository_index, repository) in repositories.iter().enumerate() {
            guard()?;
            let scope = format!("repo:{}:pull_request", repository.id);
            let run = self
                .0
                .store
                .begin_sync(&account.id, &account.authorization_epoch, &scope)
                .await?;
            for page_index in 0..(items_per_repository / 100) {
                guard()?;
                let start = page_index * 100;
                let items = (start..start + 100)
                    .map(|item_index| {
                        performance_item(slot, account, repository, repository_index, item_index)
                    })
                    .collect();
                let last = start + 100 == items_per_repository;
                self.0
                    .store
                    .apply_page(PageCommit {
                        account_id: account.id.clone(),
                        authorization_epoch: account.authorization_epoch.clone(),
                        scope: scope.clone(),
                        run_id: run.clone(),
                        repositories: vec![],
                        items,
                        endpoint_aliases: vec![],
                        next_cursor: (!last).then(|| format!("page-{}", page_index + 1)),
                        etag: None,
                        last_modified: None,
                        not_modified: false,
                        complete: last,
                        observed_at: observed_at.into(),
                    })
                    .await?;
            }
        }
        Ok(())
    }

    async fn fill<F>(&self, count: usize, guard: &F) -> Result<(), CollaborationError>
    where
        F: Fn() -> Result<(), CollaborationError> + Send + Sync,
    {
        let mut draft = self
            .0
            .store
            .draft(ALTERNATE_ACCOUNT, SUBJECT_ID)
            .await?
            .ok_or_else(stale)?;
        for index in 0..count {
            guard()?;
            draft.body = if index % 2 == 0 {
                "RURU-103 alternate catch-up fixture A"
            } else {
                "RURU-103 alternate catch-up fixture B"
            }
            .into();
            draft = self.0.store.save_draft(draft).await?;
        }
        self.0.runtime.harness_publish_current().await
    }

    async fn snapshot(&self) -> Result<HarnessCoreStatus, CollaborationError> {
        let (state, calls, gates, call_count) = {
            let state = self.0.shared.lock();
            (
                state.persistent.clone(),
                state.calls.iter().cloned().collect(),
                state
                    .gates
                    .iter()
                    .map(|gate| gate.receipt.clone())
                    .collect(),
                state.next_call.to_string(),
            )
        };
        let accounts = self.0.store.accounts().await?;
        let mut actors = vec![];
        for slot in [HarnessActorSlot::Primary, HarnessActorSlot::Alternate] {
            if let Some(account) = accounts
                .accounts
                .iter()
                .find(|account| account.id == account_id(slot))
            {
                actors.push(HarnessActorManifest {
                    slot,
                    account_id: account.id.clone(),
                    authorization_epoch: account.authorization_epoch.clone(),
                    instance_id: self.0.store.provider_instance(&account.id).await?.id,
                    repository_id: REPOSITORY_ID.into(),
                    subject_id: SUBJECT_ID.into(),
                });
            }
        }
        let (committed_phase, committed_facet_revision) =
            if state.prepared && state.fixture == HarnessFixture::Primary {
                let detail = self
                    .0
                    .store
                    .detail(DetailQuery {
                        account_id: PRIMARY_ACCOUNT.into(),
                        subject_id: SUBJECT_ID.into(),
                        facet: DetailFacet::Body,
                        cursor: None,
                        limit: 1,
                    })
                    .await?;
                let phase = [HarnessPhase::One, HarnessPhase::Two]
                    .into_iter()
                    .find(|phase| {
                        detail.body.state == DetailValueState::Known
                            && detail.body.text.as_deref()
                                == fixture_body(HarnessActorSlot::Primary, *phase)
                    });
                (phase, phase.and(detail.evidence.facet_revision))
            } else {
                (None, None)
            };
        Ok(HarnessCoreStatus {
            run_nonce: self.0.nonce.clone(),
            session_id: self.0.session_id.clone(),
            scenario_generation: state.generation.to_string(),
            prepared: state.prepared,
            fixture: state.fixture,
            phase: state.phase,
            revision: self.0.store.revision().await?,
            actors,
            calls,
            gates,
            provider_call_count: call_count,
            vault_load_count: self.0.vault.loads.load(Ordering::SeqCst).to_string(),
            vault_store_count: self.0.vault.stores.load(Ordering::SeqCst).to_string(),
            vault_delete_count: self.0.vault.deletes.load(Ordering::SeqCst).to_string(),
            vault_unavailable_count: self.0.vault.unavailable.load(Ordering::SeqCst).to_string(),
            durable_detail_requests: self.0.store.pending_details().await?.len() as u32,
            demand_lease_count: self.0.runtime.harness_lease_count().await,
            clock_elapsed_seconds: state.elapsed,
            committed_phase,
            committed_facet_revision,
            performance: if state.fixture == HarnessFixture::Performance {
                let item_count = self.0.store.harness_item_count().await?;
                Some(HarnessPerformanceFixture {
                    dataset_version: PERFORMANCE_DATASET_VERSION,
                    item_count,
                    account_count: 2,
                    repositories_per_account: PERFORMANCE_REPOSITORIES_PER_ACCOUNT,
                    items_per_account: PERFORMANCE_ITEMS_PER_ACCOUNT as u32,
                    search_sample_count: PERFORMANCE_SEARCH_SAMPLE_COUNT,
                })
            } else {
                None
            },
        })
    }
}

#[cfg(test)]
mod tests;
