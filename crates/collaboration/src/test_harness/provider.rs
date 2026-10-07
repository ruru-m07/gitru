//! Finite observations through the production provider contract. There is no
//! HTTP client, subprocess, keyring or fallback provider in this module.
use super::{state::SharedState, vault::token_slot, *};
use crate::{credentials::SecretToken, providers::*, *};
use async_trait::async_trait;
use std::{sync::Arc, time::Duration};

pub const PRIMARY_ACCOUNT: &str = "ruru103:primary";
pub const ALTERNATE_ACCOUNT: &str = "ruru103:alternate";
pub const REPOSITORY_ID: &str = "github:repository:9007199254741993";
pub const SUBJECT_ID: &str = "github:pull:9007199254742993";
pub const HEAD_OID: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
pub const BASE_OID: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
pub const FIRST_COMMIT_OID: &str = "cccccccccccccccccccccccccccccccccccccccc";
pub const SOURCE_REPOSITORY_PROVIDER_ID: &str = "9007199254741994";
const BODY_SOURCE: &str = "fixture/ruru103/body/v1";
const PULL_COMMITS_SOURCE: &str = "fixture/ruru103/pull-commits/v1";
const MAX_CALLS: usize = 128;
const GATE_TIMEOUT_SECONDS: u64 = 60;

pub fn fixture_body(slot: HarnessActorSlot, phase: HarnessPhase) -> Option<&'static str> {
    match (slot, phase) {
        (HarnessActorSlot::Primary, HarnessPhase::One) => {
            Some("RURU-103 primary body phase one — π 🌱")
        }
        (HarnessActorSlot::Primary, HarnessPhase::Two) => {
            Some("RURU-103 primary body phase two — λ 🌿")
        }
        (HarnessActorSlot::Alternate, HarnessPhase::One) => {
            Some("RURU-103 alternate body phase one — β 🍂")
        }
        (HarnessActorSlot::Alternate, HarnessPhase::Two) => {
            Some("RURU-103 alternate body phase two — γ 🍁")
        }
        _ => None,
    }
}

pub fn account_id(slot: HarnessActorSlot) -> &'static str {
    match slot {
        HarnessActorSlot::Primary => PRIMARY_ACCOUNT,
        HarnessActorSlot::Alternate => ALTERNATE_ACCOUNT,
    }
}

pub(super) fn account(slot: HarnessActorSlot) -> RemoteAccount {
    RemoteAccount {
        id: account_id(slot).into(),
        provider: ProviderKind::Github,
        host: "github.com".into(),
        actor_id: match slot {
            HarnessActorSlot::Primary => "103001",
            HarnessActorSlot::Alternate => "103002",
        }
        .into(),
        login: match slot {
            HarnessActorSlot::Primary => "ruru103-primary",
            HarnessActorSlot::Alternate => "ruru103-alternate",
        }
        .into(),
        display_name: None,
        authorization_epoch: "1".into(),
        state: AccountState::Active,
        notifications_supported: false,
    }
}

pub(super) fn repository(account: &RemoteAccount) -> RemoteRepository {
    RemoteRepository {
        id: REPOSITORY_ID.into(),
        account_id: account.id.clone(),
        provider_id: "9007199254741993".into(),
        full_name: "x-ruru103/project".into(),
        name: "project".into(),
        web_url: "https://github.com/x-ruru103/project".into(),
        description: Some("Native synthetic RURU-103 fixture".into()),
        default_branch: Some("main".into()),
        selected: false,
    }
}

pub(super) fn subject(account: &RemoteAccount, observed_at: &str) -> RemoteItem {
    RemoteItem {
        id: SUBJECT_ID.into(),
        account_id: account.id.clone(),
        repository_id: Some(REPOSITORY_ID.into()),
        provider_id: "9007199254742993".into(),
        kind: RemoteItemKind::PullRequest,
        number: Some("1".into()),
        title: "Native retained fixture pull request".into(),
        body: Some("Cached summary is separate from the endpoint body".into()),
        body_omitted: false,
        author: None,
        web_url: Some("https://github.com/x-ruru103/project/pull/1".into()),
        state: "open".into(),
        updated_at: observed_at.into(),
        head_oid: Some(HEAD_OID.into()),
        is_draft: Some(false),
        reason: None,
        unread: None,
    }
}

pub(super) struct FixtureProvider {
    pub shared: Arc<SharedState>,
    pub clock: Arc<HarnessClock>,
}

struct HeldCall {
    shared: Arc<SharedState>,
    call_id: String,
    gate_id: String,
    finished: bool,
}
impl Drop for HeldCall {
    fn drop(&mut self) {
        if self.finished {
            return;
        }
        let mut state = self.shared.lock();
        if let Some(call) = state
            .calls
            .iter_mut()
            .find(|call| call.call_id == self.call_id)
        {
            call.state = HarnessCallState::Cancelled;
        }
        if let Some(gate) = state
            .gates
            .iter_mut()
            .find(|gate| gate.receipt.gate_id == self.gate_id)
            && gate.receipt.state == HarnessGateState::Held
        {
            gate.receipt.state = HarnessGateState::Cancelled;
            gate.notify.notify_waiters();
        }
    }
}

impl FixtureProvider {
    fn slot(
        token: &SecretToken,
        account: &RemoteAccount,
    ) -> Result<HarnessActorSlot, ProviderError> {
        let slot =
            token_slot(token).map_err(|_| ProviderError::new(ProviderErrorKind::Authentication))?;
        let expected = self::account(slot);
        if account.id != expected.id
            || account.actor_id != expected.actor_id
            || account.provider != expected.provider
            || account.host != expected.host
            || account.state != AccountState::Active
        {
            return Err(ProviderError::new(ProviderErrorKind::Authentication));
        }
        Ok(slot)
    }

    async fn call(
        &self,
        slot: HarnessActorSlot,
        account: &RemoteAccount,
        facet: &str,
        hold: bool,
    ) -> Result<HarnessPhase, ProviderError> {
        let (phase, call_id, gate) = {
            let mut state = self.shared.lock();
            if !state.persistent.prepared {
                return Err(ProviderError::new(ProviderErrorKind::Unsupported));
            }
            state.next_call = state
                .next_call
                .checked_add(1)
                .ok_or_else(|| ProviderError::new(ProviderErrorKind::Unavailable))?;
            let call_id = state.next_call.to_string();
            let generation = state.persistent.generation.to_string();
            let phase = state.persistent.phase;
            let gate = if hold {
                state
                    .gates
                    .iter_mut()
                    .find(|gate| {
                        gate.receipt.scenario_generation == generation
                            && gate.receipt.state == HarnessGateState::Armed
                    })
                    .map(|gate| {
                        gate.receipt.state = HarnessGateState::Held;
                        gate.receipt.call_id = Some(call_id.clone());
                        (gate.receipt.gate_id.clone(), gate.notify.clone())
                    })
            } else {
                None
            };
            if state.calls.len() == MAX_CALLS {
                state.calls.pop_front();
            }
            state.calls.push_back(HarnessProviderCall {
                call_id: call_id.clone(),
                scenario_generation: generation,
                slot,
                authorization_epoch: account.authorization_epoch.clone(),
                facet: facet.into(),
                head_oid: (facet == "body").then(|| HEAD_OID.into()),
                phase,
                state: if gate.is_some() {
                    HarnessCallState::Held
                } else {
                    HarnessCallState::Completed
                },
            });
            (phase, call_id, gate)
        };
        if let Some((gate_id, notify)) = gate {
            let mut guard = HeldCall {
                shared: self.shared.clone(),
                call_id: call_id.clone(),
                gate_id: gate_id.clone(),
                finished: false,
            };
            let wait = async {
                loop {
                    let notified = notify.notified();
                    tokio::pin!(notified);
                    notified.as_mut().enable();
                    let status = self
                        .shared
                        .lock()
                        .gates
                        .iter()
                        .find(|gate| gate.receipt.gate_id == gate_id)
                        .map(|gate| gate.receipt.state);
                    match status {
                        Some(HarnessGateState::Released) => return HarnessCallState::Completed,
                        Some(HarnessGateState::Cancelled) | None => {
                            return HarnessCallState::Cancelled;
                        }
                        Some(HarnessGateState::TimedOut) => return HarnessCallState::TimedOut,
                        _ => notified.await,
                    }
                }
            };
            let result = tokio::time::timeout(Duration::from_secs(GATE_TIMEOUT_SECONDS), wait)
                .await
                .unwrap_or(HarnessCallState::TimedOut);
            let mut state = self.shared.lock();
            if let Some(call) = state.calls.iter_mut().find(|call| call.call_id == call_id) {
                call.state = result;
            }
            if result == HarnessCallState::TimedOut
                && let Some(gate) = state
                    .gates
                    .iter_mut()
                    .find(|gate| gate.receipt.gate_id == gate_id)
            {
                gate.receipt.state = HarnessGateState::TimedOut;
            }
            guard.finished = true;
            if result != HarnessCallState::Completed {
                return Err(ProviderError::new(ProviderErrorKind::Unavailable));
            }
        }
        Ok(phase)
    }

    fn phase_error(phase: HarnessPhase) -> Option<ProviderError> {
        match phase {
            HarnessPhase::Offline => Some(ProviderError::new(ProviderErrorKind::Offline)),
            HarnessPhase::Denied => Some(ProviderError::new(ProviderErrorKind::Permission)),
            HarnessPhase::RateLimited => Some(ProviderError {
                kind: ProviderErrorKind::RateLimited,
                retry_after_seconds: Some(60),
                account_cooldown_seconds: Some(60),
            }),
            _ => None,
        }
    }
}

#[async_trait]
impl CollaborationProvider for FixtureProvider {
    fn kind(&self) -> ProviderKind {
        ProviderKind::Github
    }
    fn profile(&self, _: &RemoteAccount) -> ProviderProfile {
        let fixture = self.shared.lock().persistent.fixture;
        ProviderProfile {
            facets: crate::providers::FACETS
                .into_iter()
                .map(|facet| {
                    let supported = facet == ResourceFacet::Repositories
                        || fixture == HarnessFixture::Primary
                            && matches!(
                                facet,
                                ResourceFacet::PullRequests
                                    | ResourceFacet::Issues
                                    | ResourceFacet::PullDetails
                                    | ResourceFacet::PullCommits
                            );
                    FacetCapability {
                        facet,
                        state: if supported {
                            CapabilityState::Supported
                        } else {
                            CapabilityState::Unsupported
                        },
                        reason: (!supported).then_some(CapabilityReason::NotImplemented),
                    }
                })
                .collect(),
            inbox_semantics: InboxSemantics::None,
        }
    }
    async fn probe(&self, _: &SecretToken) -> Result<VerifiedAccount, ProviderError> {
        Err(ProviderError::new(ProviderErrorKind::Unsupported))
    }
    async fn fetch_page(
        &self,
        token: &SecretToken,
        request: FeedRequest,
    ) -> Result<FetchPage, ProviderError> {
        let slot = Self::slot(token, &request.account)?;
        if request.cursor.is_some()
            || self
                .profile(&request.account)
                .facet(request.kind.facet())
                .state
                != CapabilityState::Supported
        {
            return Err(ProviderError::new(ProviderErrorKind::Unsupported));
        }
        if matches!(request.kind, FeedKind::PullRequests | FeedKind::Issues)
            && request.repository.as_ref().is_none_or(|repo| {
                repo.id != REPOSITORY_ID
                    || repo.account_id != request.account.id
                    || repo.provider_id != "9007199254741993"
            })
        {
            return Err(ProviderError::new(ProviderErrorKind::InvalidResponse));
        }
        let facet = match request.kind {
            FeedKind::Repositories => "repositories",
            FeedKind::PullRequests => "pull_requests",
            FeedKind::Issues => "issues",
            FeedKind::Notifications => "inbox",
        };
        let phase = self.call(slot, &request.account, facet, false).await?;
        if let Some(error) = Self::phase_error(phase) {
            return Err(error);
        }
        Ok(FetchPage {
            repositories: if request.kind == FeedKind::Repositories {
                vec![repository(&request.account)]
            } else {
                vec![]
            },
            items: if request.kind == FeedKind::PullRequests {
                vec![subject(
                    &request.account,
                    &self.clock.utc_time().to_rfc3339(),
                )]
            } else {
                vec![]
            },
            endpoint_aliases: vec![],
            notification_subjects: vec![],
            next_cursor: None,
            etag: None,
            last_modified: None,
            not_modified: false,
            poll_interval_seconds: None,
            cooldown_seconds: None,
        })
    }
    async fn fetch_detail(
        &self,
        token: &SecretToken,
        request: DetailRequest,
    ) -> Result<DetailPage, ProviderError> {
        let slot = Self::slot(token, &request.account)?;
        if request.facet != DetailFacet::Body
            || self
                .profile(&request.account)
                .facet(ResourceFacet::PullDetails)
                .state
                != CapabilityState::Supported
            || request.cursor.is_some()
            || request.subject.id != SUBJECT_ID
            || request.subject.provider_id != "9007199254742993"
            || request.subject.account_id != request.account.id
            || request.subject.repository_id.as_deref() != Some(REPOSITORY_ID)
            || request.subject.kind != RemoteItemKind::PullRequest
            || request.subject.number.as_deref() != Some("1")
            || request.repository.id != REPOSITORY_ID
            || request.repository.provider_id != "9007199254741993"
            || request.repository.account_id != request.account.id
        {
            return Err(ProviderError::new(ProviderErrorKind::InvalidResponse));
        }
        let phase = self.call(slot, &request.account, "body", true).await?;
        if let Some(error) = Self::phase_error(phase) {
            return Err(error);
        }
        let not_modified = phase == HarnessPhase::NotModified;
        let (source, etag) = if not_modified {
            let source = request
                .source
                .filter(|source| {
                    source.source == BODY_SOURCE
                        && source.adapter_version == 1
                        && source.field_mask == vec![DetailField::Body]
                })
                .ok_or_else(|| ProviderError::new(ProviderErrorKind::InvalidResponse))?;
            let etag = request
                .etag
                .filter(|etag| matches!(etag.as_str(), "\"ruru103-one\"" | "\"ruru103-two\""))
                .ok_or_else(|| ProviderError::new(ProviderErrorKind::InvalidResponse))?;
            (source, etag)
        } else {
            let offset = if phase == HarnessPhase::Two { 2 } else { 1 };
            let base = self.shared.lock().persistent.utc_base;
            (
                DetailSource {
                    source: BODY_SOURCE.into(),
                    adapter_version: 1,
                    field_mask: vec![DetailField::Body],
                    provider_updated_at: Some(
                        (base + chrono::Duration::seconds(offset)).to_rfc3339(),
                    ),
                    observed_at: self.clock.utc_time().to_rfc3339(),
                },
                if phase == HarnessPhase::Two {
                    "\"ruru103-two\""
                } else {
                    "\"ruru103-one\""
                }
                .into(),
            )
        };
        let metadata = (!not_modified).then(|| ResourceMetadataObservation {
            kind: RemoteItemKind::PullRequest,
            values: ResourceMetadataValues {
                title: Some(
                    if phase == HarnessPhase::Two {
                        "Native endpoint title phase two"
                    } else {
                        "Native endpoint title phase one"
                    }
                    .into(),
                ),
                state: Some("open".into()),
                updated_at: source.provider_updated_at.clone(),
                labels: vec![DetailLabel {
                    provider_id: Some("9007199254743993".into()),
                    name: "synthetic".into(),
                    color: Some("55aa77".into()),
                }],
                is_draft: Some(false),
                head: Some(DetailBranch {
                    name: "fixture".into(),
                    oid: HEAD_OID.into(),
                    repository: Some(DetailRepositoryRef {
                        provider_id: SOURCE_REPOSITORY_PROVIDER_ID.into(),
                        full_name: "x-ruru103/project-fork".into(),
                        web_url: Some("https://github.com/x-ruru103/project-fork".into()),
                    }),
                }),
                base: Some(DetailBranch {
                    name: "main".into(),
                    oid: BASE_OID.into(),
                    repository: Some(DetailRepositoryRef {
                        provider_id: "9007199254741993".into(),
                        full_name: "x-ruru103/project".into(),
                        web_url: Some("https://github.com/x-ruru103/project".into()),
                    }),
                }),
                ..ResourceMetadataValues::default()
            },
            fields: MetadataField::COMMON
                .into_iter()
                .chain(MetadataField::PULL)
                .map(|field| MetadataObservedField {
                    field,
                    state: DetailValueState::Known,
                })
                .collect(),
            source: MetadataSource {
                source: source.source.clone(),
                adapter_version: source.adapter_version,
                provider_updated_at: source.provider_updated_at.clone(),
                observed_at: source.observed_at.clone(),
            },
        });
        Ok(DetailPage {
            // This fixture observes the single Body representation. It does
            // not enumerate a child collection or bind Checks to a PR head.
            reconciliation: DetailReconciliation::default(),
            body: if not_modified {
                DetailValue::default()
            } else {
                DetailValue {
                    state: DetailValueState::Known,
                    text: fixture_body(slot, phase).map(str::to_owned),
                }
            },
            metadata,
            entries: vec![],
            source,
            next_cursor: None,
            etag: Some(etag),
            not_modified,
            freshness_seconds: 5,
            cooldown_seconds: None,
        })
    }

    async fn fetch_pull_commits(
        &self,
        token: &SecretToken,
        request: PullCommitRequest,
    ) -> Result<PullCommitProviderPage, ProviderError> {
        let slot = Self::slot(token, &request.account)?;
        if self
            .profile(&request.account)
            .facet(ResourceFacet::PullCommits)
            .state
            != CapabilityState::Supported
            || request.repository.id != REPOSITORY_ID
            || request.repository.provider_id != "9007199254741993"
            || request.repository.account_id != request.account.id
            || request.subject.id != SUBJECT_ID
            || request.subject.provider_id != "9007199254742993"
            || request.subject.account_id != request.account.id
            || request.subject.repository_id.as_deref() != Some(REPOSITORY_ID)
            || request.subject.kind != RemoteItemKind::PullRequest
            || request.subject.number.as_deref() != Some("1")
            || request.subject.head_oid.as_deref() != Some(HEAD_OID)
            || request.context.base_oid != BASE_OID
            || request.context.head_oid != HEAD_OID
            || request.context.source_repository_provider_id != SOURCE_REPOSITORY_PROVIDER_ID
            || request.context.metadata_facet_revision.is_empty()
            || request.cursor.is_some()
            || request.start_position != 0
        {
            return Err(ProviderError::new(ProviderErrorKind::InvalidResponse));
        }
        let phase = self
            .call(slot, &request.account, "pull_commits", false)
            .await?;
        if let Some(error) = Self::phase_error(phase) {
            return Err(error);
        }
        let actor = PullCommitActor {
            name: "Ruru 103".into(),
            provider: Some(DetailActor {
                provider_id: "103001".into(),
                login: "ruru103-primary".into(),
                web_url: Some("https://github.com/ruru103-primary".into()),
            }),
        };
        Ok(PullCommitProviderPage {
            context: request.context,
            commits: vec![
                ProviderPullCommit {
                    oid: FIRST_COMMIT_OID.into(),
                    summary: "Retain pull commits locally".into(),
                    message: PullCommitMessage {
                        state: PullCommitMessageState::Known,
                        text: Some(
                            "Retain pull commits locally\n\nSynthetic restart fixture.".into(),
                        ),
                    },
                    author: actor.clone(),
                    committer: Some(actor.clone()),
                    authored_at: Some("2026-10-01T00:01:00Z".into()),
                    committed_at: Some("2026-10-01T00:01:00Z".into()),
                    parent_oids: vec![BASE_OID.into()],
                    web_url: Some(format!(
                        "https://github.com/x-ruru103/project-fork/commit/{FIRST_COMMIT_OID}"
                    )),
                },
                ProviderPullCommit {
                    oid: HEAD_OID.into(),
                    summary: "Complete retained pull range".into(),
                    message: PullCommitMessage {
                        state: PullCommitMessageState::Known,
                        text: Some("Complete retained pull range".into()),
                    },
                    author: actor.clone(),
                    committer: Some(actor),
                    authored_at: Some("2026-10-01T00:02:00Z".into()),
                    committed_at: Some("2026-10-01T00:02:00Z".into()),
                    parent_oids: vec![FIRST_COMMIT_OID.into()],
                    web_url: Some(format!(
                        "https://github.com/x-ruru103/project-fork/commit/{HEAD_OID}"
                    )),
                },
            ],
            order: PullCommitProviderOrder::BaseToHead,
            source: PullCommitSource {
                source: PULL_COMMITS_SOURCE.into(),
                adapter_version: 1,
            },
            start_position: 0,
            next_cursor: None,
            cap_reason: None,
            remote_has_more: false,
            freshness_seconds: 5,
            cooldown_seconds: None,
        })
    }
}
