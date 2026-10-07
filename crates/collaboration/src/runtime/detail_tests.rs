use super::*;
use crate::credentials::CredentialError;
use async_trait::async_trait;
use std::sync::{Mutex as StdMutex, atomic::AtomicUsize};
#[path = "../../tests/detail_support/mod.rs"]
pub(crate) mod fixtures;

#[derive(Default)]
struct Vault {
    tokens: StdMutex<HashMap<String, SecretToken>>,
    loads: AtomicUsize,
}
impl CredentialVault for Vault {
    fn load(&self, reference: &str) -> Result<Option<SecretToken>, CredentialError> {
        self.loads.fetch_add(1, Ordering::SeqCst);
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
    calls: AtomicUsize,
    cursors: StdMutex<Vec<Option<String>>>,
    supported: bool,
    pages: usize,
    entered: Notify,
    hold: Option<Arc<Notify>>,
    error: StdMutex<Option<ProviderError>>,
    cooldown: Option<u64>,
    representations: Vec<String>,
    head_scope: DetailHeadScope,
    empty: bool,
}
impl Provider {
    fn new(pages: usize) -> Self {
        Self {
            calls: AtomicUsize::new(0),
            cursors: StdMutex::new(vec![]),
            supported: true,
            pages,
            entered: Notify::new(),
            hold: None,
            error: StdMutex::new(None),
            cooldown: None,
            representations: vec![],
            head_scope: DetailHeadScope::SubjectHistory,
            empty: false,
        }
    }
}
#[async_trait]
impl CollaborationProvider for Provider {
    fn kind(&self) -> ProviderKind {
        ProviderKind::Github
    }
    fn profile(&self, account: &RemoteAccount) -> ProviderProfile {
        let mut profile = ProviderProfile::read_only(InboxSemantics::None, false);
        if self.supported {
            for cap in &mut profile.facets {
                if matches!(
                    cap.facet,
                    ResourceFacet::PullDetails
                        | ResourceFacet::Comments
                        | ResourceFacet::Reviews
                        | ResourceFacet::Checks
                ) {
                    cap.state = CapabilityState::Supported;
                    cap.reason = None;
                }
            }
        }
        assert_eq!(account.provider, ProviderKind::Github);
        profile
    }
    async fn probe(&self, _: &SecretToken) -> Result<VerifiedAccount, ProviderError> {
        panic!("native fixture grants never probe")
    }
    async fn fetch_page(
        &self,
        _: &SecretToken,
        _: FeedRequest,
    ) -> Result<FetchPage, ProviderError> {
        panic!("a detail read cannot launch summary feed HTTP")
    }
    async fn fetch_detail(
        &self,
        token: &SecretToken,
        request: DetailRequest,
    ) -> Result<DetailPage, ProviderError> {
        assert_eq!(token.expose(), "native_detail_fixture");
        assert_eq!(request.subject.account_id, request.account.id);
        assert_eq!(request.subject.id, "pull");
        let call = self.calls.fetch_add(1, Ordering::SeqCst);
        self.cursors.lock().unwrap().push(request.cursor.clone());
        let error = self.error.lock().unwrap().clone();
        self.entered.notify_one();
        if let Some(hold) = &self.hold {
            hold.notified().await;
        }
        if let Some(error) = error {
            return Err(error);
        }
        let index = request
            .cursor
            .as_ref()
            .map(|s| s.parse::<usize>().unwrap())
            .unwrap_or(1);
        let mut source = fixtures::source(request.facet);
        if let Some(representation) = self
            .representations
            .get(call)
            .or_else(|| self.representations.last())
        {
            source.source = representation.clone();
        }
        let mut child = fixtures::entry(&format!("entry-{index:03}"));
        if self.head_scope == DetailHeadScope::CurrentHead {
            child.head_oid = request.subject.head_oid.clone();
            child.field_mask.push(DetailField::HeadOid);
        }
        Ok(DetailPage {
            reconciliation: DetailReconciliation {
                head_scope: self.head_scope,
                ..DetailReconciliation::full_history()
            },
            metadata: None,
            body: if request.facet == DetailFacet::Body {
                fixtures::known(Some("detail body"))
            } else {
                DetailValue::default()
            },
            entries: if request.facet == DetailFacet::Body || self.empty {
                vec![]
            } else {
                vec![child]
            },
            source,
            next_cursor: (index < self.pages).then(|| (index + 1).to_string()),
            etag: Some(format!("page-{index}")),
            not_modified: false,
            freshness_seconds: 60,
            cooldown_seconds: self.cooldown,
        })
    }

    async fn fetch_checks(
        &self,
        token: &SecretToken,
        request: CheckRequest,
    ) -> Result<DetailPage, ProviderError> {
        let context = request.context;
        let mut page = self.fetch_detail(token, request.detail).await?;
        page.reconciliation = DetailReconciliation {
            enumeration: DetailEnumeration::FullEnumeration,
            head_scope: DetailHeadScope::CurrentHead,
        };
        page.source.field_mask = vec![DetailField::Check, DetailField::HeadOid];
        for entry in &mut page.entries {
            let name = entry.id.clone();
            entry.author = None;
            entry.title = None;
            entry.state = None;
            entry.body = DetailValue::default();
            entry.observed_body_state = DetailValueState::NotLoaded;
            entry.updated_at = None;
            entry.head_oid = Some(context.head_oid.clone());
            entry.native = Some(NativeDetailPayload::CheckV1(CheckV1 {
                kind: CheckKind::CheckRun,
                name,
                state: CheckStateV1::CheckRun {
                    status: "completed".into(),
                    conclusion: Some("success".into()),
                },
                description: DetailValue::default(),
                producer: Some("fixture".into()),
                started_at: None,
                completed_at: None,
                updated_at: None,
                allow_failure: None,
            }));
            entry.field_mask = vec![DetailField::Check, DetailField::HeadOid];
            entry.field_validations.clear();
        }
        Ok(page)
    }
}
async fn fixture(
    path: &std::path::Path,
    provider: Arc<Provider>,
) -> (Arc<CollaborationRuntime>, Arc<Vault>, RemoteAccount) {
    let store = Arc::new(Store::open(path).await.unwrap());
    let account = fixtures::seed(&store, "a").await;
    let vault = Arc::new(Vault::default());
    let reference = "fixture:credential:version1";
    store
        .stage_credential(&account.id, reference)
        .await
        .unwrap();
    vault
        .store(
            reference,
            &SecretToken::new("native_detail_fixture".into()).unwrap(),
        )
        .unwrap();
    let account = RemoteAccount {
        authorization_epoch: "2".into(),
        ..account
    };
    let account = store
        .commit_account_credential(account, reference)
        .await
        .unwrap();
    fixtures::project(&store, &account).await;
    (
        Arc::new(CollaborationRuntime::new(store, vault.clone(), provider)),
        vault,
        account,
    )
}

const HEAD_A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const HEAD_B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
const BASE: &str = "cccccccccccccccccccccccccccccccccccccccc";

async fn save_exact_pull_context(
    runtime: &CollaborationRuntime,
    actor: &RemoteAccount,
    head: &str,
) {
    save_exact_pull_context_source(runtime, actor, head, "1").await;
}

async fn save_exact_pull_context_source(
    runtime: &CollaborationRuntime,
    actor: &RemoteAccount,
    head: &str,
    source_repository_provider_id: &str,
) {
    let subject = runtime.store.detail_subject("a", "pull").await.unwrap();
    let lease = runtime
        .store
        .begin_detail("a", &actor.authorization_epoch, "pull", DetailFacet::Body)
        .await
        .unwrap();
    let mut page = fixtures::from_lease(actor, DetailFacet::Body, lease);
    page.subject_binding = Some(DetailSubjectBinding {
        repository_id: "repo".into(),
        repository_provider_id: "1".into(),
        provider_id: subject.provider_id,
        number: subject.number,
        kind: subject.kind,
        head_oid: subject.head_oid,
    });
    page.metadata = Some(ResourceMetadataObservation {
        kind: RemoteItemKind::PullRequest,
        values: ResourceMetadataValues {
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
                oid: head.into(),
                repository: Some(DetailRepositoryRef {
                    provider_id: source_repository_provider_id.into(),
                    full_name: "owner/project".into(),
                    web_url: None,
                }),
            }),
            ..Default::default()
        },
        fields: vec![
            MetadataObservedField {
                field: MetadataField::Base,
                state: DetailValueState::Known,
            },
            MetadataObservedField {
                field: MetadataField::Head,
                state: DetailValueState::Known,
            },
        ],
        source: MetadataSource {
            source: page.source.source.clone(),
            adapter_version: page.source.adapter_version,
            provider_updated_at: page.source.provider_updated_at.clone(),
            observed_at: page.source.observed_at.clone(),
        },
    });
    runtime.store.apply_detail(page).await.unwrap();
}

async fn save_current_pull_head(runtime: &CollaborationRuntime, actor: &RemoteAccount, head: &str) {
    let mut subject = runtime.store.detail_subject("a", "pull").await.unwrap();
    subject.head_oid = Some(head.into());
    let scope = "repo:repo:pull_request";
    let run_id = runtime
        .store
        .begin_sync("a", &actor.authorization_epoch, scope)
        .await
        .unwrap();
    runtime
        .store
        .apply_page(PageCommit {
            account_id: "a".into(),
            authorization_epoch: actor.authorization_epoch.clone(),
            scope: scope.into(),
            run_id,
            repositories: vec![],
            items: vec![subject],
            endpoint_aliases: vec![],
            next_cursor: None,
            etag: None,
            last_modified: None,
            not_modified: false,
            complete: true,
            observed_at: "2026-10-03T00:00:00Z".into(),
        })
        .await
        .unwrap();
    save_exact_pull_context(runtime, actor, head).await;
}
fn demand(account: &RemoteAccount, facet: DetailFacet) -> HydrateDetailRequest {
    HydrateDetailRequest {
        account_id: account.id.clone(),
        authorization_epoch: account.authorization_epoch.clone(),
        subject_id: "pull".into(),
        facet,
    }
}

#[tokio::test]
async fn local_queries_are_inert_and_explicit_hydration_coalesces_and_commits_before_hints() {
    let dir = tempfile::tempdir().unwrap();
    let provider = Arc::new(Provider::new(1));
    let (runtime, _, actor) = fixture(&dir.path().join("cache.sqlite"), provider.clone()).await;
    let mut changes = runtime.subscribe();
    let missing = runtime
        .store
        .detail(fixtures::query("a", DetailFacet::Body))
        .await
        .unwrap();
    assert_eq!(missing.evidence.availability, DetailAvailability::Missing);
    assert_eq!(provider.calls.load(Ordering::SeqCst), 0);
    let first = runtime
        .hydrate_detail(demand(&actor, DetailFacet::Body))
        .await
        .unwrap();
    let second = runtime
        .hydrate_detail(demand(&actor, DetailFacet::Body))
        .await
        .unwrap();
    assert_eq!(first.job_id, second.job_id);
    assert_eq!(runtime.scheduler.lock().await.queue.len(), 1);
    assert_eq!(provider.calls.load(Ordering::SeqCst), 0);
    assert!(runtime.run_next().await);
    assert_eq!(provider.calls.load(Ordering::SeqCst), 1);
    let saved = runtime
        .store
        .detail(fixtures::query("a", DetailFacet::Body))
        .await
        .unwrap();
    assert_eq!(saved.body.text.as_deref(), Some("detail body"));
    assert_eq!(saved.evidence.coverage.state, CoverageState::Complete);
    let mut last = None;
    while let Ok(hint) = changes.try_recv() {
        last = Some(hint.revision);
    }
    assert_eq!(last, Some(saved.revision));
    assert!(runtime.store.pending_details().await.unwrap().is_empty());
}

#[tokio::test]
async fn unsupported_capabilities_never_admit_or_dispatch_detail_http() {
    let dir = tempfile::tempdir().unwrap();
    let mut adapter = Provider::new(1);
    adapter.supported = false;
    let provider = Arc::new(adapter);
    let (runtime, _, actor) = fixture(&dir.path().join("cache.sqlite"), provider.clone()).await;
    assert_eq!(
        runtime
            .hydrate_detail(demand(&actor, DetailFacet::Body))
            .await
            .unwrap_err()
            .code,
        ErrorCode::Unsupported
    );
    assert_eq!(provider.calls.load(Ordering::SeqCst), 0);
    assert!(runtime.store.pending_details().await.unwrap().is_empty());
    assert!(runtime.scheduler.lock().await.queue.is_empty());
}

#[tokio::test]
async fn an_old_grant_cannot_create_detail_intent_under_the_replacement_authorization() {
    let dir = tempfile::tempdir().unwrap();
    let provider = Arc::new(Provider::new(1));
    let (runtime, _, actor) = fixture(&dir.path().join("cache.sqlite"), provider.clone()).await;
    let mut request = demand(&actor, DetailFacet::Body);
    request.authorization_epoch = "1".into();
    assert_eq!(
        runtime.hydrate_detail(request).await.unwrap_err().code,
        ErrorCode::StaleView
    );
    assert!(runtime.store.pending_details().await.unwrap().is_empty());
    assert!(runtime.scheduler.lock().await.queue.is_empty());
    assert_eq!(provider.calls.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn bounded_partial_traversal_resumes_from_committed_checkpoint_after_restart() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("cache.sqlite");
    let provider = Arc::new(Provider::new(12));
    let (runtime, vault, actor) = fixture(&path, provider.clone()).await;
    runtime
        .hydrate_detail(demand(&actor, DetailFacet::Comments))
        .await
        .unwrap();
    for _ in 0..10 {
        assert!(runtime.run_next().await);
    }
    assert_eq!(provider.calls.load(Ordering::SeqCst), 10);
    let partial = runtime
        .store
        .detail(fixtures::query("a", DetailFacet::Comments))
        .await
        .unwrap();
    assert_eq!(partial.entries.len(), 10);
    assert_eq!(partial.evidence.coverage.state, CoverageState::Partial);
    runtime.store.close().await;
    drop(runtime);
    let store = Arc::new(Store::open(path).await.unwrap());
    let runtime = CollaborationRuntime::new(store, vault, provider.clone());
    runtime.enqueue_pending_details().await.unwrap();
    for _ in 0..2 {
        assert!(runtime.run_next().await);
    }
    let complete = runtime
        .store
        .detail(fixtures::query("a", DetailFacet::Comments))
        .await
        .unwrap();
    assert_eq!(complete.entries.len(), 12);
    assert_eq!(complete.evidence.coverage.state, CoverageState::Complete);
    assert_eq!(provider.cursors.lock().unwrap()[10].as_deref(), Some("11"));
    assert_eq!(
        runtime
            .store
            .account("a")
            .await
            .unwrap()
            .authorization_epoch,
        actor.authorization_epoch
    );
}

#[tokio::test]
async fn permission_loss_during_detail_http_fences_the_response_and_retains_authored_drafts() {
    let dir = tempfile::tempdir().unwrap();
    let hold = Arc::new(Notify::new());
    let mut adapter = Provider::new(1);
    adapter.hold = Some(hold.clone());
    let provider = Arc::new(adapter);
    let (runtime, _, actor) = fixture(&dir.path().join("cache.sqlite"), provider.clone()).await;
    runtime
        .store
        .save_draft(LocalDraft {
            account_id: actor.id.clone(),
            subject_id: "pull".into(),
            body: "authored draft".into(),
            generation: "0".into(),
        })
        .await
        .unwrap();
    runtime
        .hydrate_detail(demand(&actor, DetailFacet::Body))
        .await
        .unwrap();
    let running = {
        let runtime = runtime.clone();
        tokio::spawn(async move { runtime.run_next().await })
    };
    provider.entered.notified().await;
    runtime
        .store
        .set_sync_status(
            "a",
            &actor.authorization_epoch,
            "repo:repo:pull_request",
            SyncStatus {
                state: SyncState::Error,
                last_success_at: None,
                next_retry_at: None,
                error: Some(CollaborationError::new(
                    ErrorCode::PermissionDenied,
                    "Denied",
                )),
            },
        )
        .await
        .unwrap();
    hold.notify_one();
    assert!(running.await.unwrap());
    let result = runtime
        .store
        .detail(fixtures::query("a", DetailFacet::Body))
        .await
        .unwrap();
    assert_eq!(
        result.evidence.availability,
        DetailAvailability::Unavailable
    );
    assert!(result.body.text.is_none());
    assert_eq!(
        runtime
            .store
            .draft("a", "pull")
            .await
            .unwrap()
            .unwrap()
            .body,
        "authored draft"
    );
}

#[tokio::test]
async fn detail_permission_denial_hides_only_the_denied_facet_and_transient_failure_keeps_saved_reads()
 {
    let dir = tempfile::tempdir().unwrap();
    let provider = Arc::new(Provider::new(1));
    let (runtime, _, actor) = fixture(&dir.path().join("cache.sqlite"), provider.clone()).await;
    for facet in [DetailFacet::Body, DetailFacet::Comments] {
        runtime.hydrate_detail(demand(&actor, facet)).await.unwrap();
        assert!(runtime.run_next().await);
    }
    *provider.error.lock().unwrap() = Some(ProviderError::new(ProviderErrorKind::Permission));
    runtime
        .hydrate_detail(demand(&actor, DetailFacet::Comments))
        .await
        .unwrap();
    assert!(runtime.run_next().await);
    assert_eq!(
        runtime
            .store
            .detail(fixtures::query("a", DetailFacet::Comments))
            .await
            .unwrap()
            .evidence
            .availability,
        DetailAvailability::Unavailable
    );
    assert_eq!(
        runtime
            .store
            .detail(fixtures::query("a", DetailFacet::Body))
            .await
            .unwrap()
            .body
            .text
            .as_deref(),
        Some("detail body")
    );
    *provider.error.lock().unwrap() = Some(ProviderError::new(ProviderErrorKind::Offline));
    runtime
        .hydrate_detail(demand(&actor, DetailFacet::Body))
        .await
        .unwrap();
    assert!(runtime.run_next().await);
    let saved = runtime
        .store
        .detail(fixtures::query("a", DetailFacet::Body))
        .await
        .unwrap();
    assert_eq!(saved.body.text.as_deref(), Some("detail body"));
    assert_eq!(saved.evidence.sync.state, SyncState::Offline);
    assert_eq!(saved.evidence.availability, DetailAvailability::Ready);
}

#[tokio::test]
async fn provider_cooldown_preserves_saved_detail_and_blocks_other_facets_after_restart() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("cache.sqlite");
    let mut adapter = Provider::new(1);
    adapter.cooldown = Some(3600);
    let provider = Arc::new(adapter);
    let (runtime, vault, actor) = fixture(&path, provider.clone()).await;
    runtime
        .hydrate_detail(demand(&actor, DetailFacet::Body))
        .await
        .unwrap();
    assert!(runtime.run_next().await);
    let saved = runtime
        .store
        .detail(fixtures::query("a", DetailFacet::Body))
        .await
        .unwrap();
    assert_eq!(saved.body.text.as_deref(), Some("detail body"));
    assert_eq!(saved.evidence.sync.state, SyncState::RateLimited);
    runtime.store.close().await;
    drop(runtime);
    let runtime = CollaborationRuntime::new(
        Arc::new(Store::open(path).await.unwrap()),
        vault,
        provider.clone(),
    );
    runtime
        .hydrate_detail(demand(&actor, DetailFacet::Comments))
        .await
        .unwrap();
    assert!(!runtime.run_next().await);
    assert_eq!(provider.calls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn representation_drift_restarts_once_from_beginning_and_commits_only_stable_traversal() {
    let dir = tempfile::tempdir().unwrap();
    let mut adapter = Provider::new(2);
    adapter.representations = vec!["initial".into(), "replacement".into()];
    let provider = Arc::new(adapter);
    let (runtime, _, actor) = fixture(&dir.path().join("cache.sqlite"), provider.clone()).await;
    let draft = runtime
        .store
        .save_draft(LocalDraft {
            account_id: actor.id.clone(),
            subject_id: "pull".into(),
            body: "Private draft survives rejected pages".into(),
            generation: "0".into(),
        })
        .await
        .unwrap();
    runtime
        .hydrate_detail(demand(&actor, DetailFacet::Comments))
        .await
        .unwrap();
    assert!(runtime.run_next().await);
    let first = runtime
        .store
        .detail(fixtures::query("a", DetailFacet::Comments))
        .await
        .unwrap();
    assert_eq!(first.entries.len(), 1);
    assert!(runtime.run_next().await); // Reject changed continuation, separately fence restart.
    let reset = runtime
        .store
        .detail(fixtures::query("a", DetailFacet::Comments))
        .await
        .unwrap();
    assert_eq!(reset.entries, first.entries);
    assert_eq!(reset.evidence.source, first.evidence.source);
    assert_eq!(reset.evidence.coverage.state, CoverageState::Partial);
    assert!(runtime.run_next().await);
    assert!(runtime.run_next().await);
    assert_eq!(provider.calls.load(Ordering::SeqCst), 4);
    assert_eq!(
        *provider.cursors.lock().unwrap(),
        vec![None, Some("2".into()), None, Some("2".into())]
    );
    let complete = runtime
        .store
        .detail(fixtures::query("a", DetailFacet::Comments))
        .await
        .unwrap();
    assert_eq!(complete.entries.len(), 2);
    assert_eq!(complete.evidence.coverage.state, CoverageState::Complete);
    assert_eq!(complete.evidence.source.unwrap().source, "replacement");
    assert!(runtime.store.pending_details().await.unwrap().is_empty());
    assert_eq!(runtime.store.draft("a", "pull").await.unwrap(), Some(draft));
    assert!(!runtime.run_next().await);
}

#[tokio::test]
async fn repeatedly_drifting_adapter_stops_after_one_restart_and_cold_restart_has_no_intent() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("cache.sqlite");
    let mut adapter = Provider::new(2);
    adapter.representations = vec![
        "first".into(),
        "second".into(),
        "third".into(),
        "fourth".into(),
    ];
    let provider = Arc::new(adapter);
    let (runtime, vault, actor) = fixture(&path, provider.clone()).await;
    runtime
        .hydrate_detail(demand(&actor, DetailFacet::Comments))
        .await
        .unwrap();
    for _ in 0..4 {
        assert!(runtime.run_next().await);
    }
    assert_eq!(provider.calls.load(Ordering::SeqCst), 4);
    assert_eq!(
        *provider.cursors.lock().unwrap(),
        vec![None, Some("2".into()), None, Some("2".into())]
    );
    assert!(runtime.store.pending_details().await.unwrap().is_empty());
    let saved = runtime
        .store
        .detail(fixtures::query("a", DetailFacet::Comments))
        .await
        .unwrap();
    assert_eq!(saved.entries.len(), 1);
    assert_eq!(saved.evidence.source.unwrap().source, "third");
    assert_eq!(saved.evidence.sync.state, SyncState::Error);
    assert!(saved.evidence.sync.next_retry_at.is_some());
    for _ in 0..8 {
        assert!(!runtime.run_next().await);
    }
    assert_eq!(provider.calls.load(Ordering::SeqCst), 4);
    let runtime_owner = Arc::downgrade(&runtime);
    let store_owner = Arc::downgrade(&runtime.store);
    runtime.store.close().await;
    drop(runtime);
    assert!(runtime_owner.upgrade().is_none());
    assert!(store_owner.upgrade().is_none());
    let runtime = CollaborationRuntime::new(
        Arc::new(Store::open(path).await.unwrap()),
        vault,
        provider.clone(),
    );
    runtime.enqueue_pending_details().await.unwrap();
    assert!(!runtime.run_next().await);
    assert_eq!(provider.calls.load(Ordering::SeqCst), 4);
    assert_eq!(
        runtime
            .store
            .detail(fixtures::query("a", DetailFacet::Comments))
            .await
            .unwrap()
            .entries
            .len(),
        1
    );
}

#[tokio::test]
async fn known_current_head_continuation_is_vetoed_before_http_after_metadata_head_conflict() {
    let dir = tempfile::tempdir().unwrap();
    let mut adapter = Provider::new(2);
    adapter.head_scope = DetailHeadScope::CurrentHead;
    let provider = Arc::new(adapter);
    let (runtime, _, actor) = fixture(&dir.path().join("cache.sqlite"), provider.clone()).await;
    let mut subject = runtime.store.detail_subject("a", "pull").await.unwrap();
    subject.head_oid = Some(HEAD_A.into());
    let scope = "repo:repo:pull_request";
    let run_id = runtime
        .store
        .begin_sync("a", &actor.authorization_epoch, scope)
        .await
        .unwrap();
    runtime
        .store
        .apply_page(PageCommit {
            account_id: "a".into(),
            authorization_epoch: actor.authorization_epoch.clone(),
            scope: scope.into(),
            run_id,
            repositories: vec![],
            items: vec![subject.clone()],
            endpoint_aliases: vec![],
            next_cursor: None,
            etag: None,
            last_modified: None,
            not_modified: false,
            complete: true,
            observed_at: "2026-10-03T00:00:00Z".into(),
        })
        .await
        .unwrap();
    save_exact_pull_context(&runtime, &actor, HEAD_A).await;
    runtime
        .hydrate_detail(demand(&actor, DetailFacet::Checks))
        .await
        .unwrap();
    assert!(runtime.run_next().await);
    assert_eq!(provider.calls.load(Ordering::SeqCst), 1);
    let job = runtime
        .scheduler
        .lock()
        .await
        .queue
        .front()
        .cloned()
        .unwrap();
    assert_eq!(
        job.detail_lease
            .as_ref()
            .unwrap()
            .reconciliation
            .unwrap()
            .head_scope,
        DetailHeadScope::CurrentHead
    );

    let lease = runtime
        .store
        .begin_detail("a", &actor.authorization_epoch, "pull", DetailFacet::Body)
        .await
        .unwrap();
    let mut metadata = fixtures::from_lease(&actor, DetailFacet::Body, lease);
    metadata.subject_binding = Some(DetailSubjectBinding {
        repository_id: "repo".into(),
        repository_provider_id: "1".into(),
        provider_id: subject.provider_id,
        number: subject.number,
        kind: subject.kind,
        head_oid: subject.head_oid,
    });
    metadata.metadata = Some(ResourceMetadataObservation {
        kind: RemoteItemKind::PullRequest,
        values: ResourceMetadataValues {
            head: Some(DetailBranch {
                name: "feature".into(),
                oid: HEAD_B.into(),
                repository: None,
            }),
            ..Default::default()
        },
        fields: vec![MetadataObservedField {
            field: MetadataField::Head,
            state: DetailValueState::Known,
        }],
        source: MetadataSource {
            source: metadata.source.source.clone(),
            adapter_version: metadata.source.adapter_version,
            provider_updated_at: metadata.source.provider_updated_at.clone(),
            observed_at: metadata.source.observed_at.clone(),
        },
    });
    runtime.store.apply_detail(metadata).await.unwrap();
    assert!(runtime.run_next().await);
    assert_eq!(
        provider.calls.load(Ordering::SeqCst),
        1,
        "the classified old-head continuation never reaches the provider"
    );
    let saved = runtime
        .store
        .detail(fixtures::query("a", DetailFacet::Checks))
        .await
        .unwrap();
    assert_eq!(saved.entries.len(), 1);
    assert_eq!(saved.evidence.freshness, DetailFreshness::Stale);
    assert_eq!(
        runtime
            .store
            .detail_subject("a", "pull")
            .await
            .unwrap()
            .head_oid
            .as_deref(),
        Some(HEAD_A)
    );
}

#[tokio::test]
async fn same_head_context_refresh_vetoes_queued_checks_before_provider_http() {
    for source_repository_provider_id in ["1", "2"] {
        let dir = tempfile::tempdir().unwrap();
        let mut adapter = Provider::new(2);
        adapter.head_scope = DetailHeadScope::CurrentHead;
        let provider = Arc::new(adapter);
        let (runtime, _, actor) = fixture(&dir.path().join("cache.sqlite"), provider.clone()).await;
        let mut subject = runtime.store.detail_subject("a", "pull").await.unwrap();
        subject.head_oid = Some(HEAD_A.into());
        let scope = "repo:repo:pull_request";
        let run_id = runtime
            .store
            .begin_sync("a", &actor.authorization_epoch, scope)
            .await
            .unwrap();
        runtime
            .store
            .apply_page(PageCommit {
                account_id: "a".into(),
                authorization_epoch: actor.authorization_epoch.clone(),
                scope: scope.into(),
                run_id,
                repositories: vec![],
                items: vec![subject],
                endpoint_aliases: vec![],
                next_cursor: None,
                etag: None,
                last_modified: None,
                not_modified: false,
                complete: true,
                observed_at: "2026-10-03T00:00:00Z".into(),
            })
            .await
            .unwrap();
        save_exact_pull_context(&runtime, &actor, HEAD_A).await;
        runtime
            .hydrate_detail(demand(&actor, DetailFacet::Checks))
            .await
            .unwrap();
        assert!(runtime.run_next().await);
        assert_eq!(provider.calls.load(Ordering::SeqCst), 1);
        assert_eq!(
            runtime
                .scheduler
                .lock()
                .await
                .queue
                .front()
                .unwrap()
                .detail_lease
                .as_ref()
                .unwrap()
                .next_cursor
                .as_deref(),
            Some("2")
        );

        // The OID remains identical. A new Body facet revision, and in the
        // second case a different source repository, retires the old context.
        save_exact_pull_context_source(&runtime, &actor, HEAD_A, source_repository_provider_id)
            .await;
        assert!(runtime.run_next().await);
        assert_eq!(
            provider.calls.load(Ordering::SeqCst),
            1,
            "source repository {source_repository_provider_id}: stale continuation reached provider"
        );
        assert_eq!(*provider.cursors.lock().unwrap(), vec![None]);
        let saved = runtime
            .store
            .detail(fixtures::query("a", DetailFacet::Checks))
            .await
            .unwrap();
        assert_eq!(saved.entries.len(), 1);
        assert_eq!(saved.evidence.freshness, DetailFreshness::Stale);
        assert!(!saved.check_aggregate(Some(HEAD_A)).unwrap().authoritative);

        runtime
            .hydrate_detail(demand(&actor, DetailFacet::Checks))
            .await
            .unwrap();
        assert!(runtime.run_next().await, "fresh replacement first page");
        assert!(runtime.run_next().await, "fresh replacement completion");
        assert_eq!(provider.calls.load(Ordering::SeqCst), 3);
        assert_eq!(
            *provider.cursors.lock().unwrap(),
            vec![None, None, Some("2".into())]
        );
        let replaced = runtime
            .store
            .detail(fixtures::query("a", DetailFacet::Checks))
            .await
            .unwrap();
        assert_eq!(
            replaced.check_aggregate(Some(HEAD_A)),
            Some(CheckAggregate {
                state: CheckAggregateState::Passed,
                authoritative: true,
                total: 2,
            }),
            "source repository {source_repository_provider_id}: replacement traversal publishes"
        );
    }
}

#[tokio::test]
async fn retired_current_head_cursor_is_not_dispatched_under_a_new_summary_head_without_metadata() {
    let dir = tempfile::tempdir().unwrap();
    let mut adapter = Provider::new(2);
    adapter.head_scope = DetailHeadScope::CurrentHead;
    let provider = Arc::new(adapter);
    let (runtime, _, actor) = fixture(&dir.path().join("cache.sqlite"), provider.clone()).await;
    for (head, at) in [
        (HEAD_A, "2026-10-03T01:00:00Z"),
        (HEAD_B, "2026-10-03T02:00:00Z"),
    ] {
        let mut subject = runtime.store.detail_subject("a", "pull").await.unwrap();
        subject.head_oid = Some(head.into());
        subject.updated_at = at.into();
        let scope = "repo:repo:pull_request";
        let run_id = runtime
            .store
            .begin_sync("a", &actor.authorization_epoch, scope)
            .await
            .unwrap();
        runtime
            .store
            .apply_page(PageCommit {
                account_id: "a".into(),
                authorization_epoch: actor.authorization_epoch.clone(),
                scope: scope.into(),
                run_id,
                repositories: vec![],
                items: vec![subject],
                endpoint_aliases: vec![],
                next_cursor: None,
                etag: None,
                last_modified: None,
                not_modified: false,
                complete: true,
                observed_at: at.into(),
            })
            .await
            .unwrap();
        if head == HEAD_A {
            save_exact_pull_context(&runtime, &actor, HEAD_A).await;
            runtime
                .hydrate_detail(demand(&actor, DetailFacet::Checks))
                .await
                .unwrap();
            assert!(runtime.run_next().await);
            assert_eq!(provider.calls.load(Ordering::SeqCst), 1);
            assert_eq!(
                runtime
                    .scheduler
                    .lock()
                    .await
                    .queue
                    .front()
                    .unwrap()
                    .detail_lease
                    .as_ref()
                    .unwrap()
                    .next_cursor
                    .as_deref(),
                Some("2")
            );
        }
    }
    assert!(
        runtime
            .store
            .detail(fixtures::query("a", DetailFacet::Body))
            .await
            .unwrap()
            .metadata
            .is_some()
    );
    assert!(runtime.run_next().await);
    assert_eq!(
        provider.calls.load(Ordering::SeqCst),
        1,
        "head-a cursor must be fenced before dispatch against head-b"
    );
    assert_eq!(*provider.cursors.lock().unwrap(), vec![None]);
    let saved = runtime
        .store
        .detail(fixtures::query("a", DetailFacet::Checks))
        .await
        .unwrap();
    assert_eq!(saved.entries.len(), 1);
    assert_eq!(
        saved.entries.first().unwrap().head_oid.as_deref(),
        Some(HEAD_A)
    );
    assert_eq!(saved.evidence.freshness, DetailFreshness::Stale);
}

#[tokio::test]
async fn exact_head_checks_coalesce_and_publish_complete_empty_evidence() {
    let dir = tempfile::tempdir().unwrap();
    let mut adapter = Provider::new(1);
    adapter.head_scope = DetailHeadScope::CurrentHead;
    adapter.empty = true;
    let provider = Arc::new(adapter);
    let (runtime, _, actor) = fixture(&dir.path().join("cache.sqlite"), provider.clone()).await;
    save_current_pull_head(&runtime, &actor, HEAD_A).await;

    let first = runtime
        .hydrate_detail(demand(&actor, DetailFacet::Checks))
        .await
        .unwrap();
    let second = runtime
        .hydrate_detail(demand(&actor, DetailFacet::Checks))
        .await
        .unwrap();
    assert_eq!(first.job_id, second.job_id);
    assert_eq!(runtime.scheduler.lock().await.queue.len(), 1);
    assert_eq!(provider.calls.load(Ordering::SeqCst), 0);
    assert!(runtime.run_next().await);
    assert_eq!(provider.calls.load(Ordering::SeqCst), 1);

    let saved = runtime
        .store
        .detail(fixtures::query("a", DetailFacet::Checks))
        .await
        .unwrap();
    assert!(saved.entries.is_empty());
    assert_eq!(saved.evidence.availability, DetailAvailability::Ready);
    assert_eq!(saved.evidence.coverage.state, CoverageState::Complete);
    assert_eq!(
        saved.check_aggregate(Some(HEAD_A)),
        Some(CheckAggregate {
            state: CheckAggregateState::Empty,
            authoritative: false,
            total: 0,
        })
    );
}

#[tokio::test]
async fn exact_head_checks_keep_saved_rows_offline_and_hide_permission_denial() {
    let dir = tempfile::tempdir().unwrap();
    let mut adapter = Provider::new(1);
    adapter.head_scope = DetailHeadScope::CurrentHead;
    let provider = Arc::new(adapter);
    let (runtime, _, actor) = fixture(&dir.path().join("offline.sqlite"), provider.clone()).await;
    save_current_pull_head(&runtime, &actor, HEAD_A).await;
    runtime
        .hydrate_detail(demand(&actor, DetailFacet::Checks))
        .await
        .unwrap();
    assert!(runtime.run_next().await);
    *provider.error.lock().unwrap() = Some(ProviderError::new(ProviderErrorKind::Offline));
    runtime
        .hydrate_detail(demand(&actor, DetailFacet::Checks))
        .await
        .unwrap();
    assert!(runtime.run_next().await);
    let offline = runtime
        .store
        .detail(fixtures::query("a", DetailFacet::Checks))
        .await
        .unwrap();
    assert_eq!(offline.entries.len(), 1);
    assert_eq!(offline.evidence.availability, DetailAvailability::Ready);
    assert_eq!(offline.evidence.sync.state, SyncState::Offline);
    assert_eq!(
        offline.check_aggregate(Some(HEAD_A)),
        Some(CheckAggregate {
            state: CheckAggregateState::Passed,
            authoritative: true,
            total: 1,
        })
    );

    let mut denied_adapter = Provider::new(1);
    denied_adapter.head_scope = DetailHeadScope::CurrentHead;
    *denied_adapter.error.lock().unwrap() = Some(ProviderError::new(ProviderErrorKind::Permission));
    let denied_provider = Arc::new(denied_adapter);
    let (denied_runtime, _, denied_actor) = fixture(
        &dir.path().join("permission.sqlite"),
        denied_provider.clone(),
    )
    .await;
    save_current_pull_head(&denied_runtime, &denied_actor, HEAD_A).await;
    denied_runtime
        .hydrate_detail(demand(&denied_actor, DetailFacet::Checks))
        .await
        .unwrap();
    assert!(denied_runtime.run_next().await);
    let denied = denied_runtime
        .store
        .detail(fixtures::query("a", DetailFacet::Checks))
        .await
        .unwrap();
    assert!(denied.entries.is_empty());
    assert_eq!(
        denied.evidence.availability,
        DetailAvailability::Unavailable
    );
    assert_eq!(
        denied.check_aggregate(Some(HEAD_A)),
        Some(CheckAggregate {
            state: CheckAggregateState::Unavailable,
            authoritative: false,
            total: 0,
        })
    );
}

#[tokio::test]
async fn exact_head_checks_keep_complete_evidence_during_provider_cooldown() {
    let dir = tempfile::tempdir().unwrap();
    let mut adapter = Provider::new(1);
    adapter.head_scope = DetailHeadScope::CurrentHead;
    adapter.cooldown = Some(3600);
    let provider = Arc::new(adapter);
    let (runtime, _, actor) = fixture(&dir.path().join("cache.sqlite"), provider).await;
    save_current_pull_head(&runtime, &actor, HEAD_A).await;
    runtime
        .hydrate_detail(demand(&actor, DetailFacet::Checks))
        .await
        .unwrap();
    assert!(runtime.run_next().await);
    let saved = runtime
        .store
        .detail(fixtures::query("a", DetailFacet::Checks))
        .await
        .unwrap();
    assert_eq!(saved.entries.len(), 1);
    assert_eq!(saved.evidence.coverage.state, CoverageState::Complete);
    assert_eq!(saved.evidence.sync.state, SyncState::RateLimited);
    assert!(saved.evidence.sync.next_retry_at.is_some());
    assert_eq!(
        saved.check_aggregate(Some(HEAD_A)),
        Some(CheckAggregate {
            state: CheckAggregateState::Passed,
            authoritative: true,
            total: 1,
        })
    );
}

#[tokio::test]
async fn exact_head_checks_reopen_from_sqlite_without_provider_or_vault_access() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("cache.sqlite");
    let provider = Arc::new(Provider::new(1));
    let (runtime, vault, actor) = fixture(&path, provider.clone()).await;
    let mut subject = runtime.store.detail_subject("a", "pull").await.unwrap();
    subject.head_oid = Some(HEAD_A.into());
    let scope = "repo:repo:pull_request";
    let run_id = runtime
        .store
        .begin_sync("a", &actor.authorization_epoch, scope)
        .await
        .unwrap();
    runtime
        .store
        .apply_page(PageCommit {
            account_id: "a".into(),
            authorization_epoch: actor.authorization_epoch.clone(),
            scope: scope.into(),
            run_id,
            repositories: vec![],
            items: vec![subject],
            endpoint_aliases: vec![],
            next_cursor: None,
            etag: None,
            last_modified: None,
            not_modified: false,
            complete: true,
            observed_at: "2026-10-03T00:00:00Z".into(),
        })
        .await
        .unwrap();
    save_exact_pull_context(&runtime, &actor, HEAD_A).await;
    runtime
        .hydrate_detail(demand(&actor, DetailFacet::Checks))
        .await
        .unwrap();
    assert!(runtime.run_next().await);
    assert_eq!(provider.calls.load(Ordering::SeqCst), 1);
    let saved = runtime
        .store
        .detail(fixtures::query("a", DetailFacet::Checks))
        .await
        .unwrap();
    assert_eq!(
        saved.check_aggregate(Some(HEAD_A)),
        Some(CheckAggregate {
            state: CheckAggregateState::Passed,
            authoritative: true,
            total: 1,
        })
    );
    let with_state = |id: &str, state: CheckStateV1| {
        let mut entry = saved.entries[0].clone();
        entry.id = id.into();
        entry.provider_id = id.into();
        let Some(NativeDetailPayload::CheckV1(check)) = &mut entry.native else {
            panic!("saved fixture check")
        };
        check.name = id.into();
        check.state = state;
        entry
    };
    let failed = with_state(
        "failed",
        CheckStateV1::CheckRun {
            status: "completed".into(),
            conclusion: Some("failure".into()),
        },
    );
    let pending = with_state(
        "pending",
        CheckStateV1::CheckRun {
            status: "queued".into(),
            conclusion: None,
        },
    );
    let unknown = with_state(
        "unknown",
        CheckStateV1::CommitStatus {
            state: "future".into(),
        },
    );
    for entries in [
        vec![failed.clone(), pending.clone()],
        vec![pending, failed.clone()],
        vec![failed.clone(), unknown.clone()],
        vec![unknown, failed],
    ] {
        let mut mixed = saved.clone();
        mixed.entries = entries;
        assert_eq!(
            mixed.check_aggregate(Some(HEAD_A)),
            Some(CheckAggregate {
                state: CheckAggregateState::Failed,
                authoritative: false,
                total: 2,
            })
        );
    }
    let mut partial = saved.clone();
    partial.evidence.coverage.state = CoverageState::Partial;
    assert_eq!(
        partial.check_aggregate(Some(HEAD_A)).unwrap().state,
        CheckAggregateState::Partial
    );
    assert!(!partial.check_aggregate(Some(HEAD_A)).unwrap().authoritative);
    let mut paged = saved.clone();
    paged.next_cursor = Some("after-first-local-page".into());
    assert_eq!(
        paged.check_aggregate(Some(HEAD_A)),
        Some(CheckAggregate {
            state: CheckAggregateState::Partial,
            authoritative: false,
            total: 1,
        })
    );
    let mut empty = saved.clone();
    empty.entries.clear();
    assert_eq!(
        empty.check_aggregate(Some(HEAD_A)),
        Some(CheckAggregate {
            state: CheckAggregateState::Empty,
            authoritative: false,
            total: 0,
        })
    );
    let mut stale = saved.clone();
    stale.evidence.freshness = DetailFreshness::Stale;
    assert_eq!(
        stale.check_aggregate(Some(HEAD_A)).unwrap().state,
        CheckAggregateState::Stale
    );
    assert!(!stale.check_aggregate(Some(HEAD_A)).unwrap().authoritative);
    let mut syncing = saved.clone();
    syncing.evidence.sync.state = SyncState::Syncing;
    assert_eq!(
        syncing.check_aggregate(Some(HEAD_A)).unwrap().state,
        CheckAggregateState::Syncing
    );
    assert!(!syncing.check_aggregate(Some(HEAD_A)).unwrap().authoritative);
    for current_head in [None, Some(HEAD_B)] {
        assert_eq!(
            saved.check_aggregate(current_head),
            Some(CheckAggregate {
                state: CheckAggregateState::Stale,
                authoritative: false,
                total: 1,
            })
        );
    }
    let loads = vault.loads.load(Ordering::SeqCst);
    runtime.store.close().await;
    drop(runtime);

    let reopened = Arc::new(Store::open(&path).await.unwrap());
    let unopened_provider = Arc::new(Provider::new(1));
    let restarted = CollaborationRuntime::new(reopened, vault.clone(), unopened_provider.clone());
    let cached = restarted
        .store
        .detail(fixtures::query("a", DetailFacet::Checks))
        .await
        .unwrap();
    assert_eq!(cached.entries, saved.entries);
    assert_eq!(
        cached.check_aggregate(Some(HEAD_A)),
        saved.check_aggregate(Some(HEAD_A))
    );
    assert_eq!(unopened_provider.calls.load(Ordering::SeqCst), 0);
    assert_eq!(vault.loads.load(Ordering::SeqCst), loads);
}
