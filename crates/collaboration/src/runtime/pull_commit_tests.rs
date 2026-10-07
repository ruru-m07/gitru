use super::detail_tests::fixtures;
use super::*;
use crate::credentials::CredentialError;
use async_trait::async_trait;
use std::sync::{Mutex as StdMutex, atomic::AtomicUsize};

const BASE: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const FIRST: &str = "cccccccccccccccccccccccccccccccccccccccc";
const SECOND: &str = "dddddddddddddddddddddddddddddddddddddddd";
const HEAD: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
const CHANGED_HEAD: &str = "eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee";

#[derive(Clone, Copy, PartialEq, Eq)]
enum ValidationFixture {
    Matching,
    Changed,
    Omitted,
}

#[derive(Default)]
struct Vault {
    tokens: StdMutex<HashMap<String, SecretToken>>,
}

impl CredentialVault for Vault {
    fn load(&self, reference: &str) -> Result<Option<SecretToken>, CredentialError> {
        Ok(self.tokens.lock().unwrap().get(reference).cloned())
    }

    fn store(&self, reference: &str, token: &SecretToken) -> Result<(), CredentialError> {
        self.tokens
            .lock()
            .unwrap()
            .insert(reference.into(), token.clone());
        Ok(())
    }

    fn delete(&self, reference: &str) -> Result<(), CredentialError> {
        self.tokens.lock().unwrap().remove(reference);
        Ok(())
    }
}

struct Provider {
    body_calls: AtomicUsize,
    commit_starts: StdMutex<Vec<u32>>,
    second_pages: AtomicUsize,
    drift_once: bool,
    initial_validation: ValidationFixture,
    validation: ValidationFixture,
    provider_cap: bool,
}

impl Provider {
    fn new(drift_once: bool) -> Self {
        Self {
            body_calls: AtomicUsize::new(0),
            commit_starts: StdMutex::new(vec![]),
            second_pages: AtomicUsize::new(0),
            drift_once,
            initial_validation: ValidationFixture::Matching,
            validation: ValidationFixture::Matching,
            provider_cap: false,
        }
    }

    fn terminal_fixture(validation: ValidationFixture, provider_cap: bool) -> Self {
        Self {
            validation,
            provider_cap,
            ..Self::new(false)
        }
    }

    fn body_transition(
        initial_validation: ValidationFixture,
        validation: ValidationFixture,
    ) -> Self {
        Self {
            initial_validation,
            validation,
            ..Self::new(false)
        }
    }
}

fn actor() -> PullCommitActor {
    PullCommitActor {
        name: "Fixture Author".into(),
        provider: None,
    }
}

fn provider_commit(oid: &str) -> ProviderPullCommit {
    ProviderPullCommit {
        oid: oid.into(),
        summary: format!("commit {}", &oid[..8]),
        message: PullCommitMessage {
            state: PullCommitMessageState::Known,
            text: Some(format!("commit {oid}")),
        },
        author: actor(),
        committer: Some(actor()),
        authored_at: Some("2026-10-07T00:00:00Z".into()),
        committed_at: Some("2026-10-07T00:00:01Z".into()),
        parent_oids: vec![BASE.into()],
        web_url: Some(format!("https://github.com/owner/project/commit/{oid}")),
    }
}

#[async_trait]
impl CollaborationProvider for Provider {
    fn kind(&self) -> ProviderKind {
        ProviderKind::Github
    }

    fn profile(&self, _: &RemoteAccount) -> ProviderProfile {
        let mut profile = ProviderProfile::read_only(InboxSemantics::None, false);
        for capability in &mut profile.facets {
            if matches!(
                capability.facet,
                ResourceFacet::PullDetails | ResourceFacet::PullCommits
            ) {
                capability.state = CapabilityState::Supported;
                capability.reason = None;
            }
        }
        profile
    }

    async fn probe(&self, _: &SecretToken) -> Result<VerifiedAccount, ProviderError> {
        panic!("fixture grants are installed natively")
    }

    async fn fetch_page(
        &self,
        _: &SecretToken,
        _: FeedRequest,
    ) -> Result<FetchPage, ProviderError> {
        panic!("commit hydration cannot launch a summary feed")
    }

    async fn fetch_detail(
        &self,
        token: &SecretToken,
        request: DetailRequest,
    ) -> Result<DetailPage, ProviderError> {
        assert_eq!(token.expose(), "pull_commit_fixture");
        assert_eq!(request.facet, DetailFacet::Body);
        assert!(request.cursor.is_none());
        let body_call = self.body_calls.fetch_add(1, Ordering::SeqCst);
        if body_call > 0 && !self.commit_starts.lock().unwrap().is_empty() {
            assert!(request.etag.is_none());
            assert!(request.source.is_none());
        }
        let source = DetailSource {
            source: "github/pull/v1".into(),
            adapter_version: 1,
            field_mask: vec![DetailField::Body],
            provider_updated_at: None,
            observed_at: "2026-10-07T00:00:00Z".into(),
        };
        let validation = if body_call == 0 {
            self.initial_validation
        } else {
            self.validation
        };
        let (values, fields) = match validation {
            ValidationFixture::Matching | ValidationFixture::Changed => {
                let changed = validation == ValidationFixture::Changed;
                (
                    ResourceMetadataValues {
                        base: Some(DetailBranch {
                            name: "main".into(),
                            oid: BASE.into(),
                            repository: Some(DetailRepositoryRef {
                                provider_id: "1".into(),
                                full_name: "owner/project".into(),
                                web_url: None,
                            }),
                        }),
                        head: Some(DetailBranch {
                            name: "feature".into(),
                            oid: if changed { CHANGED_HEAD } else { HEAD }.into(),
                            repository: Some(DetailRepositoryRef {
                                provider_id: if changed { "fork-3" } else { "fork-2" }.into(),
                                full_name: "fork/project".into(),
                                web_url: None,
                            }),
                        }),
                        ..Default::default()
                    },
                    vec![
                        MetadataObservedField {
                            field: MetadataField::Base,
                            state: DetailValueState::Known,
                        },
                        MetadataObservedField {
                            field: MetadataField::Head,
                            state: DetailValueState::Known,
                        },
                    ],
                )
            }
            ValidationFixture::Omitted => (
                ResourceMetadataValues::default(),
                vec![
                    MetadataObservedField {
                        field: MetadataField::Base,
                        state: DetailValueState::Omitted,
                    },
                    MetadataObservedField {
                        field: MetadataField::Head,
                        state: DetailValueState::Omitted,
                    },
                ],
            ),
        };
        Ok(DetailPage {
            reconciliation: DetailReconciliation::full_history(),
            body: DetailValue {
                state: DetailValueState::Known,
                text: Some("body".into()),
            },
            metadata: Some(ResourceMetadataObservation {
                kind: RemoteItemKind::PullRequest,
                values,
                fields,
                source: MetadataSource {
                    source: source.source.clone(),
                    adapter_version: source.adapter_version,
                    provider_updated_at: None,
                    observed_at: source.observed_at.clone(),
                },
            }),
            entries: vec![],
            source,
            next_cursor: None,
            etag: None,
            not_modified: false,
            freshness_seconds: 60,
            cooldown_seconds: None,
        })
    }

    async fn fetch_pull_commits(
        &self,
        token: &SecretToken,
        request: PullCommitRequest,
    ) -> Result<PullCommitProviderPage, ProviderError> {
        assert_eq!(token.expose(), "pull_commit_fixture");
        assert_eq!(request.context.base_oid, BASE);
        assert_eq!(request.context.head_oid, HEAD);
        assert_eq!(request.context.source_repository_provider_id, "fork-2");
        self.commit_starts
            .lock()
            .unwrap()
            .push(request.start_position);
        let (commits, next_cursor, start_position) = if request.start_position == 0 {
            assert!(request.cursor.is_none());
            (vec![provider_commit(FIRST)], Some("page-2".into()), 0)
        } else {
            assert_eq!(request.start_position, 1);
            assert_eq!(request.cursor.as_deref(), Some("page-2"));
            let page = self.second_pages.fetch_add(1, Ordering::SeqCst);
            (
                vec![provider_commit(if self.provider_cap {
                    SECOND
                } else {
                    HEAD
                })],
                None,
                if self.drift_once && page == 0 { 0 } else { 1 },
            )
        };
        let cap_reason = (self.provider_cap && request.start_position != 0)
            .then_some(PullCommitCapReason::ProviderLimit);
        Ok(PullCommitProviderPage {
            context: request.context,
            commits,
            order: PullCommitProviderOrder::BaseToHead,
            source: PullCommitSource {
                source: "github/pull-commits/v1".into(),
                adapter_version: 1,
            },
            start_position,
            remote_has_more: next_cursor.is_some() || cap_reason.is_some(),
            next_cursor,
            cap_reason,
            freshness_seconds: 60,
            cooldown_seconds: None,
        })
    }
}

async fn set_summary_head(store: &Store, account: &RemoteAccount, head_oid: &str) {
    let mut item = store.item(&account.id, "pull").await.unwrap().item.unwrap();
    item.head_oid = Some(head_oid.into());
    item.updated_at = "2026-10-07T00:00:00Z".into();
    let scope = "repo:repo:pull_request";
    let run_id = store
        .begin_sync(&account.id, &account.authorization_epoch, scope)
        .await
        .unwrap();
    store
        .apply_page(PageCommit {
            account_id: account.id.clone(),
            authorization_epoch: account.authorization_epoch.clone(),
            scope: scope.into(),
            run_id,
            repositories: vec![],
            items: vec![item],
            endpoint_aliases: vec![],
            next_cursor: None,
            etag: None,
            last_modified: None,
            not_modified: false,
            complete: true,
            observed_at: "2026-10-07T00:00:00Z".into(),
        })
        .await
        .unwrap();
}

async fn fixture(
    path: &std::path::Path,
    provider: Arc<Provider>,
) -> (Arc<CollaborationRuntime>, RemoteAccount) {
    fixture_with_head(path, provider, HEAD).await
}

async fn fixture_with_head(
    path: &std::path::Path,
    provider: Arc<Provider>,
    head_oid: &str,
) -> (Arc<CollaborationRuntime>, RemoteAccount) {
    let store = Arc::new(Store::open(path).await.unwrap());
    let account = fixtures::seed(&store, "a").await;
    let vault = Arc::new(Vault::default());
    let reference = "fixture:pull-commits:v1";
    store
        .stage_credential(&account.id, reference)
        .await
        .unwrap();
    vault
        .store(
            reference,
            &SecretToken::new("pull_commit_fixture".into()).unwrap(),
        )
        .unwrap();
    let account = store
        .commit_account_credential(
            RemoteAccount {
                authorization_epoch: "2".into(),
                ..account
            },
            reference,
        )
        .await
        .unwrap();
    fixtures::project(&store, &account).await;
    set_summary_head(&store, &account, head_oid).await;
    (
        Arc::new(CollaborationRuntime::new(store, vault, provider)),
        account,
    )
}

async fn acquire_visible_commit_demand(
    runtime: &CollaborationRuntime,
    account: &RemoteAccount,
    owner: &str,
) -> DemandLeaseReceipt {
    let activity = runtime.demand_owner_activity(owner).await.unwrap();
    let activity = runtime
        .set_demand_owner_activity(owner, &activity.generation, true)
        .await
        .unwrap();
    runtime
        .acquire_demand(
            owner,
            AcquireDemandRequest {
                account_id: account.id.clone(),
                authorization_epoch: account.authorization_epoch.clone(),
                owner_generation: activity.generation,
                target: DemandTarget {
                    kind: DemandTargetKind::Detail,
                    repository_id: None,
                    subject_id: Some("pull".into()),
                    facet: Some(DetailFacet::Commits),
                },
            },
        )
        .await
        .unwrap()
}

async fn drive_commits_to_terminal(
    runtime: &CollaborationRuntime,
    account: &RemoteAccount,
) -> PullCommitSnapshot {
    for _ in 0..12 {
        let snapshot = runtime
            .store
            .pull_commits(PullCommitQuery {
                account_id: account.id.clone(),
                subject_id: "pull".into(),
                cursor: None,
                limit: 100,
            })
            .await
            .unwrap();
        if snapshot.completeness.is_complete() {
            return snapshot;
        }
        assert!(runtime.run_next().await);
    }
    panic!("pull commit demand did not reach terminal publication");
}

async fn hydrate_to_terminal(runtime: &CollaborationRuntime, account: &RemoteAccount) {
    runtime
        .hydrate_detail(HydrateDetailRequest {
            account_id: account.id.clone(),
            authorization_epoch: account.authorization_epoch.clone(),
            subject_id: "pull".into(),
            facet: DetailFacet::Commits,
        })
        .await
        .unwrap();
    for _ in 0..8 {
        let snapshot = runtime
            .store
            .pull_commits(PullCommitQuery {
                account_id: account.id.clone(),
                subject_id: "pull".into(),
                cursor: None,
                limit: 100,
            })
            .await
            .unwrap();
        if snapshot.completeness.is_complete() {
            return;
        }
        assert!(runtime.run_next().await);
    }
    panic!("pull commit fixture did not reach terminal publication");
}

async fn drive_to_first_terminal_validation(
    runtime: &CollaborationRuntime,
    account: &RemoteAccount,
    provider: &Provider,
) {
    runtime
        .hydrate_detail(HydrateDetailRequest {
            account_id: account.id.clone(),
            authorization_epoch: account.authorization_epoch.clone(),
            subject_id: "pull".into(),
            facet: DetailFacet::Commits,
        })
        .await
        .unwrap();
    for _ in 0..8 {
        assert!(runtime.run_next().await);
        if provider.body_calls.load(Ordering::SeqCst) >= 2 {
            return;
        }
    }
    panic!("pull commit fixture did not reach terminal parent validation");
}

#[tokio::test]
async fn visible_commit_demand_hydrates_missing_body_before_retrying_commits() {
    let directory = tempfile::tempdir().unwrap();
    let provider = Arc::new(Provider::new(false));
    let (runtime, account) =
        fixture(&directory.path().join("cache.sqlite"), provider.clone()).await;
    let _demand = acquire_visible_commit_demand(&runtime, &account, "pull-view").await;

    assert!(runtime.run_next().await);
    assert_eq!(provider.body_calls.load(Ordering::SeqCst), 0);
    assert!(provider.commit_starts.lock().unwrap().is_empty());
    for _ in 0..4 {
        assert!(runtime.run_next().await);
        if provider.body_calls.load(Ordering::SeqCst) == 1 {
            break;
        }
        assert!(provider.commit_starts.lock().unwrap().is_empty());
    }
    assert_eq!(provider.body_calls.load(Ordering::SeqCst), 1);
    assert!(provider.commit_starts.lock().unwrap().is_empty());

    let snapshot = drive_commits_to_terminal(&runtime, &account).await;
    assert_eq!(&*provider.commit_starts.lock().unwrap(), &[0, 1]);
    assert_eq!(snapshot.context.as_ref().unwrap().head_oid, HEAD);
    assert_eq!(
        snapshot
            .commits
            .iter()
            .map(|commit| commit.oid.as_str())
            .collect::<Vec<_>>(),
        vec![FIRST, HEAD]
    );
}

#[tokio::test]
async fn visible_commit_demand_rehydrates_a_conflicting_summary_head_before_retrying() {
    let directory = tempfile::tempdir().unwrap();
    let provider = Arc::new(Provider::body_transition(
        ValidationFixture::Changed,
        ValidationFixture::Matching,
    ));
    let (runtime, account) = fixture_with_head(
        &directory.path().join("cache.sqlite"),
        provider.clone(),
        CHANGED_HEAD,
    )
    .await;
    runtime
        .hydrate_detail(HydrateDetailRequest {
            account_id: account.id.clone(),
            authorization_epoch: account.authorization_epoch.clone(),
            subject_id: "pull".into(),
            facet: DetailFacet::Body,
        })
        .await
        .unwrap();
    assert!(runtime.run_next().await);
    assert_eq!(provider.body_calls.load(Ordering::SeqCst), 1);
    set_summary_head(&runtime.store, &account, HEAD).await;
    let _demand = acquire_visible_commit_demand(&runtime, &account, "changed-pull-view").await;

    for _ in 0..5 {
        assert!(runtime.run_next().await);
        if provider.body_calls.load(Ordering::SeqCst) == 2 {
            break;
        }
        assert!(provider.commit_starts.lock().unwrap().is_empty());
    }
    assert_eq!(provider.body_calls.load(Ordering::SeqCst), 2);
    assert!(provider.commit_starts.lock().unwrap().is_empty());

    let snapshot = drive_commits_to_terminal(&runtime, &account).await;
    let context = snapshot.context.as_ref().unwrap();
    assert_eq!(context.head_oid, HEAD);
    assert_eq!(context.source_repository_provider_id, "fork-2");
    assert_eq!(&*provider.commit_starts.lock().unwrap(), &[0, 1]);
    assert_eq!(
        snapshot
            .commits
            .iter()
            .map(|commit| commit.oid.as_str())
            .collect::<Vec<_>>(),
        vec![FIRST, HEAD]
    );
}

#[tokio::test]
async fn manual_commit_intent_survives_busy_body_admission_and_retries_from_durable_demand() {
    let directory = tempfile::tempdir().unwrap();
    let provider = Arc::new(Provider::new(false));
    let (runtime, account) =
        fixture(&directory.path().join("cache.sqlite"), provider.clone()).await;
    {
        let mut scheduler = runtime.scheduler.lock().await;
        for index in 0..32 {
            let scope = format!("detail:queue-{index}:body");
            scheduler.queue.push_back(Job {
                key: format!("{}:{}:{scope}", account.id, account.authorization_epoch),
                account: account.clone(),
                repository: None,
                kind: JobKind::Detail {
                    subject_id: format!("queue-{index}"),
                    facet: DetailFacet::Body,
                },
                scope,
                reason: scheduler::Admission::Explicit,
                pages: 0,
                detail_lease: None,
                detail_restarted: false,
                pull_commit_lease: None,
                pull_commit_restarted: false,
                local_budget_refusal: false,
                enqueued_at: runtime.now(),
            });
        }
    }

    let error = runtime
        .hydrate_detail(HydrateDetailRequest {
            account_id: account.id.clone(),
            authorization_epoch: account.authorization_epoch.clone(),
            subject_id: "pull".into(),
            facet: DetailFacet::Commits,
        })
        .await
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::Busy);
    assert_eq!(provider.body_calls.load(Ordering::SeqCst), 0);
    assert!(provider.commit_starts.lock().unwrap().is_empty());
    let pending = runtime.store.pending_details().await.unwrap();
    assert!(
        pending
            .iter()
            .any(|demand| demand.facet == DetailFacet::Body)
    );
    assert!(
        pending
            .iter()
            .any(|demand| demand.facet == DetailFacet::Commits)
    );

    runtime.scheduler.lock().await.queue.clear();
    runtime.enqueue_pending_details().await.unwrap();
    let snapshot = drive_commits_to_terminal(&runtime, &account).await;
    assert_eq!(provider.body_calls.load(Ordering::SeqCst), 2);
    assert_eq!(&*provider.commit_starts.lock().unwrap(), &[0, 1]);
    assert_eq!(snapshot.context.as_ref().unwrap().head_oid, HEAD);
    assert_eq!(
        snapshot
            .commits
            .iter()
            .map(|commit| commit.oid.as_str())
            .collect::<Vec<_>>(),
        vec![FIRST, HEAD]
    );
}

#[tokio::test]
async fn commit_hydration_establishes_exact_body_context_before_publishing_pages() {
    let directory = tempfile::tempdir().unwrap();
    let provider = Arc::new(Provider::new(false));
    let (runtime, account) =
        fixture(&directory.path().join("cache.sqlite"), provider.clone()).await;
    hydrate_to_terminal(&runtime, &account).await;
    assert_eq!(provider.body_calls.load(Ordering::SeqCst), 2);
    assert_eq!(&*provider.commit_starts.lock().unwrap(), &[0, 1]);
    let snapshot = runtime
        .store
        .pull_commits(PullCommitQuery {
            account_id: account.id,
            subject_id: "pull".into(),
            cursor: None,
            limit: 100,
        })
        .await
        .unwrap();
    assert_eq!(
        snapshot
            .commits
            .iter()
            .map(|commit| commit.oid.as_str())
            .collect::<Vec<_>>(),
        vec![FIRST, HEAD]
    );
    assert!(runtime.store.pending_details().await.unwrap().is_empty());
}

#[tokio::test]
async fn representation_drift_restarts_once_and_only_the_stable_generation_is_visible() {
    let directory = tempfile::tempdir().unwrap();
    let provider = Arc::new(Provider::new(true));
    let (runtime, account) =
        fixture(&directory.path().join("cache.sqlite"), provider.clone()).await;
    hydrate_to_terminal(&runtime, &account).await;
    assert_eq!(&*provider.commit_starts.lock().unwrap(), &[0, 1, 0, 1]);
    let snapshot = runtime
        .store
        .pull_commits(PullCommitQuery {
            account_id: account.id,
            subject_id: "pull".into(),
            cursor: None,
            limit: 100,
        })
        .await
        .unwrap();
    assert_eq!(snapshot.completeness, PullCommitCompleteness::complete());
    assert_eq!(snapshot.commits.len(), 2);
}

#[tokio::test]
async fn changed_parent_range_blocks_a_provider_capped_terminal_generation() {
    let directory = tempfile::tempdir().unwrap();
    let provider = Arc::new(Provider::terminal_fixture(ValidationFixture::Changed, true));
    let (runtime, account) =
        fixture(&directory.path().join("cache.sqlite"), provider.clone()).await;
    drive_to_first_terminal_validation(&runtime, &account, &provider).await;

    assert_eq!(provider.body_calls.load(Ordering::SeqCst), 2);
    assert_eq!(&*provider.commit_starts.lock().unwrap(), &[0, 1]);
    let snapshot = runtime
        .store
        .pull_commits(PullCommitQuery {
            account_id: account.id.clone(),
            subject_id: "pull".into(),
            cursor: None,
            limit: 100,
        })
        .await
        .unwrap();
    assert!(snapshot.context.is_none());
    assert!(snapshot.commits.is_empty());
    assert_eq!(snapshot.completeness, PullCommitCompleteness::missing());
    assert!(
        runtime
            .store
            .pending_details()
            .await
            .unwrap()
            .iter()
            .any(|demand| demand.account_id == account.id
                && demand.subject_id == "pull"
                && demand.facet == DetailFacet::Body)
    );
}

#[tokio::test]
async fn omitted_parent_range_blocks_terminal_publication_and_requests_body_again() {
    let directory = tempfile::tempdir().unwrap();
    let provider = Arc::new(Provider::terminal_fixture(
        ValidationFixture::Omitted,
        false,
    ));
    let (runtime, account) =
        fixture(&directory.path().join("cache.sqlite"), provider.clone()).await;
    drive_to_first_terminal_validation(&runtime, &account, &provider).await;

    let snapshot = runtime
        .store
        .pull_commits(PullCommitQuery {
            account_id: account.id.clone(),
            subject_id: "pull".into(),
            cursor: None,
            limit: 100,
        })
        .await
        .unwrap();
    assert!(snapshot.context.is_none());
    assert!(snapshot.commits.is_empty());
    assert_eq!(
        runtime
            .store
            .begin_pull_commits(&account.id, &account.authorization_epoch, "pull")
            .await
            .unwrap_err()
            .code,
        ErrorCode::NotFound
    );
    assert!(
        runtime
            .store
            .pending_details()
            .await
            .unwrap()
            .iter()
            .any(|demand| demand.account_id == account.id
                && demand.subject_id == "pull"
                && demand.facet == DetailFacet::Body)
    );
}
