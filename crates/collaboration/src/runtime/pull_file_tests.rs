//! Native synthetic provider/vault qualification for file hydration.
use super::detail_tests::fixtures;
use super::*;
use crate::credentials::CredentialError;
use async_trait::async_trait;
use std::sync::{Mutex as StdMutex, atomic::AtomicUsize};

const BASE: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const HEAD: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

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
    body_calls: AtomicUsize,
    starts: StdMutex<Vec<u32>>,
    validation_calls: AtomicUsize,
    pages: u32,
    page_cooldown: Option<(u32, u64)>,
    validation_cooldown: Option<u64>,
    validation_error: StdMutex<Option<ProviderError>>,
    hold: Option<Arc<Notify>>,
    entered: Notify,
}
impl Provider {
    fn new(pages: u32) -> Self {
        Self {
            body_calls: AtomicUsize::new(0),
            starts: StdMutex::new(vec![]),
            validation_calls: AtomicUsize::new(0),
            pages,
            page_cooldown: None,
            validation_cooldown: None,
            validation_error: StdMutex::new(None),
            hold: None,
            entered: Notify::new(),
        }
    }
}
fn file(position: u32) -> ProviderPullFile {
    ProviderPullFile {
        identity: PullFileIdentity {
            old_path: Some(format!("file{position}.txt")),
            new_path: Some(format!("file{position}.txt")),
        },
        provider_file_id: None,
        change_kind: PullFileChangeKind::Modified,
        provider_change_kind: "modified".into(),
        additions: PullFileCount::Known("1".into()),
        deletions: PullFileCount::Known("1".into()),
        total_changes: PullFileCount::Known("2".into()),
        old_mode: None,
        new_mode: None,
        mode_changed: PullFileFlag::Unknown,
        binary: PullFileFlag::Unknown,
        generated: PullFileFlag::Unknown,
        provider_collapsed: PullFileFlag::Unknown,
        provider_too_large: PullFileFlag::Unknown,
        diff_hint: PullFileDiffHint::Omitted,
    }
}
#[async_trait]
impl CollaborationProvider for Provider {
    fn kind(&self) -> ProviderKind {
        ProviderKind::Github
    }
    fn profile(&self, _: &RemoteAccount) -> ProviderProfile {
        let mut profile = ProviderProfile::read_only(InboxSemantics::None, false);
        for facet in &mut profile.facets {
            if matches!(
                facet.facet,
                ResourceFacet::PullDetails | ResourceFacet::PullFiles
            ) {
                facet.state = CapabilityState::Supported;
                facet.reason = None;
            }
        }
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
        panic!("file hydration cannot launch feed reads")
    }
    async fn fetch_detail(
        &self,
        token: &SecretToken,
        request: DetailRequest,
    ) -> Result<DetailPage, ProviderError> {
        assert_eq!(token.expose(), "synthetic_file_runtime");
        assert_eq!(request.facet, DetailFacet::Body);
        self.body_calls.fetch_add(1, Ordering::SeqCst);
        let source = fixtures::source(DetailFacet::Body);
        let branch = |name: &str, oid: &str, repository: &str| DetailBranch {
            name: name.into(),
            oid: oid.into(),
            repository: Some(DetailRepositoryRef {
                provider_id: repository.into(),
                full_name: format!("owner/{repository}"),
                web_url: None,
            }),
        };
        Ok(DetailPage {
            reconciliation: DetailReconciliation::full_history(),
            body: fixtures::known(Some("Body")),
            metadata: Some(ResourceMetadataObservation {
                kind: RemoteItemKind::PullRequest,
                values: ResourceMetadataValues {
                    base: Some(branch("main", BASE, "1")),
                    head: Some(branch("feature", HEAD, "2")),
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
                    source: source.source.clone(),
                    adapter_version: 1,
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
    async fn fetch_pull_files(
        &self,
        token: &SecretToken,
        request: PullFileCollectionRequest,
    ) -> Result<PullFileProviderPage, ProviderError> {
        assert_eq!(token.expose(), "synthetic_file_runtime");
        request.validate().unwrap();
        assert_eq!(request.binding.context.base_oid, BASE);
        assert_eq!(request.binding.context.head_oid, HEAD);
        self.starts.lock().unwrap().push(request.start_position);
        self.entered.notify_one();
        if let Some(hold) = &self.hold {
            hold.notified().await;
        }
        Ok(PullFileProviderPage {
            context: request.binding.context.clone(),
            files: vec![file(request.start_position)],
            source: request.source.clone(),
            start_position: request.start_position,
            next_cursor: (request.start_position + 1 < self.pages)
                .then(|| format!("page-{}", request.start_position + 1)),
            cap: None,
            freshness_seconds: 60,
            cooldown_seconds: self
                .page_cooldown
                .filter(|(position, _)| *position == request.start_position)
                .map(|(_, wait)| wait),
        })
    }
    async fn validate_pull_file_range(
        &self,
        token: &SecretToken,
        request: PullFileCollectionRequest,
    ) -> Result<PullFileRangeValidationResult, ProviderError> {
        assert_eq!(token.expose(), "synthetic_file_runtime");
        self.validation_calls.fetch_add(1, Ordering::SeqCst);
        if let Some(error) = self.validation_error.lock().unwrap().clone() {
            return Err(error);
        }
        let context = request.binding.context;
        Ok(PullFileRangeValidationResult {
            validation: PullFileRangeValidation {
                base_oid: context.base_oid,
                head_oid: context.head_oid,
                merge_base_oid: context.merge_base_oid,
                base_repository_provider_id: context.base_repository_provider_id,
                source_repository_provider_id: context.source_repository_provider_id,
            },
            expected_file_count: Some(self.pages),
            collection_cap: None,
            cooldown_seconds: self.validation_cooldown,
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
    let reference = "fixture:pull-files:v1";
    store
        .stage_credential(&account.id, reference)
        .await
        .unwrap();
    vault
        .store(
            reference,
            &SecretToken::new("synthetic_file_runtime".into()).unwrap(),
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
    let mut item = store.item(&account.id, "pull").await.unwrap().item.unwrap();
    item.head_oid = Some(HEAD.into());
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
    (
        Arc::new(CollaborationRuntime::new(store, vault.clone(), provider)),
        vault,
        account,
    )
}
fn query(account: &RemoteAccount) -> PullFileQuery {
    PullFileQuery {
        account_id: account.id.clone(),
        subject_id: "pull".into(),
        cursor: None,
        limit: 100,
    }
}
async fn hydrate(runtime: &CollaborationRuntime, account: &RemoteAccount) -> RefreshReceipt {
    runtime
        .hydrate_detail(HydrateDetailRequest {
            account_id: account.id.clone(),
            authorization_epoch: account.authorization_epoch.clone(),
            subject_id: "pull".into(),
            facet: DetailFacet::Files,
        })
        .await
        .unwrap()
}
async fn complete(runtime: &CollaborationRuntime, account: &RemoteAccount) {
    for _ in 0..12 {
        if runtime
            .store
            .pull_files(query(account))
            .await
            .unwrap()
            .completeness
            .state
            == PullFileCompletenessState::Complete
        {
            return;
        }
        assert!(
            runtime.run_next().await,
            "body={:?}; files={:?}",
            runtime.store.scope_state("a", "detail:pull:body").await,
            runtime.store.scope_state("a", "detail:pull:files").await
        );
    }
    panic!("files did not reach terminal publication")
}
async fn first_file(runtime: &CollaborationRuntime, provider: &Provider) {
    for _ in 0..8 {
        assert!(
            runtime.run_next().await,
            "body={:?}; files={:?}",
            runtime.store.scope_state("a", "detail:pull:body").await,
            runtime.store.scope_state("a", "detail:pull:files").await
        );
        if !provider.starts.lock().unwrap().is_empty() {
            return;
        }
    }
    panic!("no file request")
}

#[tokio::test]
async fn missing_context_hydrates_body_then_stages_until_fresh_terminal_validation() {
    let dir = tempfile::tempdir().unwrap();
    let provider = Arc::new(Provider::new(3));
    let (runtime, vault, account) =
        fixture(&dir.path().join("files.sqlite"), provider.clone()).await;
    hydrate(&runtime, &account).await;
    first_file(&runtime, &provider).await;
    let partial = runtime.store.pull_files(query(&account)).await.unwrap();
    assert!(partial.files.is_empty());
    assert_ne!(
        partial.completeness.state,
        PullFileCompletenessState::Complete
    );
    complete(&runtime, &account).await;
    let snapshot = runtime.store.pull_files(query(&account)).await.unwrap();
    assert_eq!(snapshot.files.len(), 3);
    assert_eq!(*provider.starts.lock().unwrap(), vec![0, 1, 2]);
    assert_eq!(provider.body_calls.load(Ordering::SeqCst), 1);
    assert_eq!(provider.validation_calls.load(Ordering::SeqCst), 1);
    let loads = vault.loads.load(Ordering::SeqCst);
    runtime.store.pull_files(query(&account)).await.unwrap();
    assert_eq!(vault.loads.load(Ordering::SeqCst), loads);
}
#[tokio::test]
async fn repeated_explicit_intent_coalesces_into_one_collection_job() {
    let dir = tempfile::tempdir().unwrap();
    let provider = Arc::new(Provider::new(2));
    let (runtime, _, account) = fixture(&dir.path().join("files.sqlite"), provider.clone()).await;
    let one = hydrate(&runtime, &account).await;
    let two = hydrate(&runtime, &account).await;
    assert_eq!(one.job_id, two.job_id);
    complete(&runtime, &account).await;
    assert_eq!(*provider.starts.lock().unwrap(), vec![0, 1]);
    assert_eq!(provider.validation_calls.load(Ordering::SeqCst), 1);
}
#[tokio::test]
async fn terminal_quota_prevents_parent_request_and_preserves_checkpoint() {
    let dir = tempfile::tempdir().unwrap();
    let provider = Arc::new(Provider {
        page_cooldown: Some((1, 600)),
        ..Provider::new(2)
    });
    let (runtime, _, account) = fixture(&dir.path().join("files.sqlite"), provider.clone()).await;
    hydrate(&runtime, &account).await;
    for _ in 0..8 {
        runtime.run_next().await;
        if provider.starts.lock().unwrap().len() == 2 {
            break;
        }
    }
    assert_eq!(*provider.starts.lock().unwrap(), vec![0, 1]);
    assert_eq!(provider.validation_calls.load(Ordering::SeqCst), 0);
    let lease = runtime
        .store
        .resume_pull_files(&account.id, &account.authorization_epoch, "pull")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(lease.accepted_row_count, 1);
    assert_eq!(lease.provider_page_count, 1);
    assert!(
        runtime
            .store
            .pull_files(query(&account))
            .await
            .unwrap()
            .files
            .is_empty()
    );
    for _ in 0..3 {
        runtime.run_next().await;
    }
    assert_eq!(provider.starts.lock().unwrap().len(), 2);
}
#[tokio::test]
async fn nonterminal_quota_checkpoint_survives_cold_store_reopen() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("files.sqlite");
    let provider = Arc::new(Provider {
        page_cooldown: Some((0, 600)),
        ..Provider::new(2)
    });
    let (runtime, _, account) = fixture(&path, provider.clone()).await;
    hydrate(&runtime, &account).await;
    first_file(&runtime, &provider).await;
    let lease = runtime
        .store
        .resume_pull_files(&account.id, &account.authorization_epoch, "pull")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(lease.accepted_row_count, 1);
    drop(runtime);
    let store = Store::open(&path).await.unwrap();
    let resumed = store
        .resume_pull_files(&account.id, &account.authorization_epoch, "pull")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(resumed, lease);
    assert_eq!(provider.validation_calls.load(Ordering::SeqCst), 0);
}
#[tokio::test]
async fn parent_validation_error_keeps_old_published_files_and_requests_body() {
    let dir = tempfile::tempdir().unwrap();
    let provider = Arc::new(Provider::new(1));
    let (runtime, _, account) = fixture(&dir.path().join("files.sqlite"), provider.clone()).await;
    hydrate(&runtime, &account).await;
    complete(&runtime, &account).await;
    let before = runtime.store.pull_files(query(&account)).await.unwrap();
    *provider.validation_error.lock().unwrap() =
        Some(ProviderError::new(ProviderErrorKind::InvalidResponse));
    hydrate(&runtime, &account).await;
    for _ in 0..8 {
        runtime.run_next().await;
        if provider.validation_calls.load(Ordering::SeqCst) > 1 {
            break;
        }
    }
    let after = runtime.store.pull_files(query(&account)).await.unwrap();
    assert_eq!(after.files, before.files);
    assert_eq!(after.facet_revision, before.facet_revision);
    assert!(
        runtime
            .store
            .pending_detail_batch(None)
            .await
            .unwrap()
            .iter()
            .any(|d| d.facet == DetailFacet::Body)
    );
}
#[tokio::test]
async fn account_disconnect_during_file_read_blocks_parent_validation_and_publication() {
    let dir = tempfile::tempdir().unwrap();
    let hold = Arc::new(Notify::new());
    let provider = Arc::new(Provider {
        hold: Some(hold.clone()),
        ..Provider::new(1)
    });
    let (runtime, _, account) = fixture(&dir.path().join("files.sqlite"), provider.clone()).await;
    hydrate(&runtime, &account).await;
    // First queued read establishes Body; the collection then blocks in its
    // synthetic transport while the lifecycle changes independently.
    for _ in 0..4 {
        if runtime
            .store
            .pull_files(query(&account))
            .await
            .unwrap()
            .context
            .is_some()
        {
            break;
        }
        assert!(
            runtime.run_next().await,
            "body={:?}; files={:?}",
            runtime.store.scope_state("a", "detail:pull:body").await,
            runtime.store.scope_state("a", "detail:pull:files").await
        );
    }
    let cloned = runtime.clone();
    let task = tokio::spawn(async move { cloned.run_next().await });
    provider.entered.notified().await;
    runtime.disconnect(&account.id).await.unwrap();
    hold.notify_one();
    assert!(task.await.unwrap());
    assert_eq!(provider.validation_calls.load(Ordering::SeqCst), 0);
}
