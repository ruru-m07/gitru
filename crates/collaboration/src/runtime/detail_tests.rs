use super::*;
use crate::credentials::CredentialError;
use async_trait::async_trait;
use std::sync::{Mutex as StdMutex, atomic::AtomicUsize};
#[path = "../../tests/detail_support/mod.rs"]
mod fixtures;

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
    calls: AtomicUsize,
    cursors: StdMutex<Vec<Option<String>>>,
    supported: bool,
    pages: usize,
    entered: Notify,
    hold: Option<Arc<Notify>>,
    error: StdMutex<Option<ProviderError>>,
    cooldown: Option<u64>,
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
        self.calls.fetch_add(1, Ordering::SeqCst);
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
        Ok(DetailPage {
            body: if request.facet == DetailFacet::Body {
                fixtures::known(Some("detail body"))
            } else {
                DetailValue::default()
            },
            entries: if request.facet == DetailFacet::Body {
                vec![]
            } else {
                vec![fixtures::entry(&format!("entry-{index:03}"))]
            },
            source: fixtures::source(request.facet),
            next_cursor: (index < self.pages).then(|| (index + 1).to_string()),
            etag: Some(format!("page-{index}")),
            not_modified: false,
            freshness_seconds: 60,
            cooldown_seconds: self.cooldown,
        })
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
    assert!(runtime.run_next().await);
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
    assert!(runtime.run_next().await);
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
