//! Native scheduling controls; no provider credentials or external network.
use super::*;
use crate::{credentials::CredentialError, issue_metadata::*};
use std::sync::atomic::{AtomicU64, AtomicUsize};
#[derive(Default)]
struct Vault {
    loads: AtomicUsize,
}
impl CredentialVault for Vault {
    fn load(&self, _: &str) -> Result<Option<SecretToken>, CredentialError> {
        self.loads.fetch_add(1, Ordering::SeqCst);
        Ok(Some(
            SecretToken::new("synthetic_metadata_runtime".into()).unwrap(),
        ))
    }
    fn store(&self, _: &str, _: &SecretToken) -> Result<(), CredentialError> {
        Ok(())
    }
    fn delete(&self, _: &str) -> Result<(), CredentialError> {
        Ok(())
    }
}
#[derive(Default)]
struct Provider {
    calls: AtomicUsize,
    held: AtomicBool,
    entered: Notify,
    release: Notify,
    cooldown: AtomicU64,
    cursors: std::sync::Mutex<Vec<Option<String>>>,
}
#[async_trait::async_trait]
impl CollaborationProvider for Provider {
    fn kind(&self) -> ProviderKind {
        ProviderKind::Github
    }
    async fn probe(&self, _: &SecretToken) -> Result<VerifiedAccount, ProviderError> {
        panic!("no probe")
    }
    async fn fetch_page(
        &self,
        _: &SecretToken,
        _: FeedRequest,
    ) -> Result<FetchPage, ProviderError> {
        panic!("no feed")
    }
    async fn fetch_issue_metadata_catalog(
        &self,
        _: &SecretToken,
        r: IssueMetadataCatalogRequest,
    ) -> Result<IssueMetadataCatalogPage, ProviderError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.cursors.lock().unwrap().push(r.cursor.clone());
        self.entered.notify_one();
        if self.held.load(Ordering::SeqCst) {
            self.release.notified().await;
        }
        let terminal = r.cursor.is_some();
        Ok(IssueMetadataCatalogPage {
            options: vec![IssueMetadataOption {
                reference: IssueMetadataReference::Label(IssueMetadataLabel {
                    provider_id: if terminal { "2" } else { "1" }.into(),
                    name: if terminal { "second" } else { "first" }.into(),
                    color: None,
                }),
                availability: IssueMetadataAvailability::Unknown,
                reason: Some(IssueMetadataReason::Unobserved),
            }],
            next_cursor: (!terminal).then_some("native-page-two".into()),
            truncated: false,
            coverage: CoverageState::Partial,
            cooldown_seconds: Some(self.cooldown.load(Ordering::SeqCst)),
        })
    }
}
async fn setup(
    dir: &std::path::Path,
    provider: Arc<Provider>,
) -> (Arc<CollaborationRuntime>, Arc<Vault>, RemoteAccount) {
    let store = Arc::new(Store::open(dir.join("catalog.db")).await.unwrap());
    let a = store
        .upsert_account(RemoteAccount {
            actor_id: "7".into(),
            ..detail_tests::fixtures::account("a")
        })
        .await
        .unwrap();
    store
        .stage_credential("a", "metadata-fixture")
        .await
        .unwrap();
    let a = store
        .commit_account_credential(
            RemoteAccount {
                authorization_epoch: "2".into(),
                ..a
            },
            "metadata-fixture",
        )
        .await
        .unwrap();
    detail_tests::fixtures::project(&store, &a).await;
    let vault = Arc::new(Vault::default());
    (
        Arc::new(CollaborationRuntime::new(store, vault.clone(), provider)),
        vault,
        a,
    )
}
fn query() -> IssueMetadataQuery {
    IssueMetadataQuery {
        account_id: "a".into(),
        repository_id: "repo".into(),
        kind: IssueMetadataKind::Labels,
        search: String::new(),
        cursor: None,
        limit: 10,
    }
}
async fn demand(runtime: &CollaborationRuntime, a: &RemoteAccount) -> DemandLeaseReceipt {
    let current = runtime.demand_owner_activity("owner").await.unwrap();
    let owner = runtime
        .set_demand_owner_activity("owner", &current.generation, true)
        .await
        .unwrap();
    runtime
        .acquire_demand(
            "owner",
            AcquireDemandRequest {
                account_id: a.id.clone(),
                authorization_epoch: a.authorization_epoch.clone(),
                owner_generation: owner.generation,
                target: DemandTarget {
                    kind: DemandTargetKind::RepositoryLabels,
                    repository_id: Some("repo".into()),
                    subject_id: None,
                    facet: None,
                },
            },
        )
        .await
        .unwrap()
}
#[tokio::test]
async fn catalog_reads_are_local_and_leased_pages_stop_after_hidden_owner() {
    let dir = tempfile::tempdir().unwrap();
    let provider = Arc::new(Provider::default());
    let (runtime, vault, a) = setup(dir.path(), provider.clone()).await;
    assert!(
        runtime
            .issue_metadata_options(query())
            .await
            .unwrap()
            .options
            .is_empty()
    );
    assert_eq!(vault.loads.load(Ordering::SeqCst), 0);
    let lease = demand(&runtime, &a).await;
    assert!(runtime.run_next().await);
    assert_eq!(provider.calls.load(Ordering::SeqCst), 1);
    assert_eq!(
        runtime
            .issue_metadata_options(query())
            .await
            .unwrap()
            .options
            .len(),
        1
    );
    runtime
        .set_demand_owner_activity("owner", &lease.owner_generation, false)
        .await
        .unwrap();
    assert!(!runtime.run_next().await);
    assert_eq!(provider.calls.load(Ordering::SeqCst), 1);
    runtime.shutdown().await.unwrap();
    let store = Store::open(dir.path().join("catalog.db")).await.unwrap();
    assert_eq!(
        store
            .issue_metadata_options(query())
            .await
            .unwrap()
            .options
            .len(),
        1
    );
    assert_eq!(
        store
            .resume_issue_metadata(
                "a",
                &a.authorization_epoch,
                "repo",
                IssueMetadataKind::Labels
            )
            .await
            .unwrap()
            .unwrap()
            .request
            .cursor
            .as_deref(),
        Some("native-page-two")
    );
    store.close().await.unwrap();
}
#[tokio::test]
async fn catalog_success_quota_blocks_continuation_before_vault_and_survives_restart() {
    let dir = tempfile::tempdir().unwrap();
    let provider = Arc::new(Provider::default());
    provider.cooldown.store(3600, Ordering::SeqCst);
    let (runtime, vault, a) = setup(dir.path(), provider.clone()).await;
    runtime
        .refresh_issue_metadata(RefreshIssueMetadataRequest {
            account_id: "a".into(),
            authorization_epoch: a.authorization_epoch.clone(),
            repository_id: "repo".into(),
            kind: IssueMetadataKind::Labels,
        })
        .await
        .unwrap();
    assert!(runtime.run_next().await);
    assert_eq!(provider.calls.load(Ordering::SeqCst), 1);
    assert_eq!(
        runtime
            .issue_metadata_options(query())
            .await
            .unwrap()
            .sync
            .state,
        SyncState::RateLimited
    );
    let loads = vault.loads.load(Ordering::SeqCst);
    let _ = runtime.run_next().await;
    assert_eq!(vault.loads.load(Ordering::SeqCst), loads);
    assert_eq!(provider.calls.load(Ordering::SeqCst), 1);
    assert!(
        runtime
            .store
            .scope_state("a", "provider:rest")
            .await
            .unwrap()
            .unwrap()
            .sync
            .next_retry_at
            .is_some()
    );
    runtime.shutdown().await.unwrap();
    let store = Store::open(dir.path().join("catalog.db")).await.unwrap();
    assert!(
        store
            .scope_state("a", "provider:rest")
            .await
            .unwrap()
            .unwrap()
            .sync
            .next_retry_at
            .is_some()
    );
    assert_eq!(
        store
            .issue_metadata_options(query())
            .await
            .unwrap()
            .options
            .len(),
        1
    );
    store.close().await.unwrap();
}
#[tokio::test]
async fn held_catalog_cannot_resurrect_same_epoch_deselected_repository() {
    let dir = tempfile::tempdir().unwrap();
    let provider = Arc::new(Provider::default());
    provider.held.store(true, Ordering::SeqCst);
    let (runtime, _, a) = setup(dir.path(), provider.clone()).await;
    runtime
        .refresh_issue_metadata(RefreshIssueMetadataRequest {
            account_id: "a".into(),
            authorization_epoch: a.authorization_epoch,
            repository_id: "repo".into(),
            kind: IssueMetadataKind::Labels,
        })
        .await
        .unwrap();
    let task = tokio::spawn({
        let r = runtime.clone();
        async move { r.run_next().await }
    });
    tokio::time::timeout(Duration::from_secs(5), provider.entered.notified())
        .await
        .unwrap();
    runtime
        .store
        .select_repository("a", "repo", false)
        .await
        .unwrap();
    runtime
        .store
        .select_repository("a", "repo", true)
        .await
        .unwrap();
    provider.release.notify_one();
    assert!(task.await.unwrap());
    assert!(
        runtime
            .issue_metadata_options(query())
            .await
            .unwrap()
            .options
            .is_empty()
    );
    assert_eq!(provider.calls.load(Ordering::SeqCst), 1);
    runtime.shutdown().await.unwrap();
}

struct CatalogClock {
    start: Instant,
    wall: DateTime<Utc>,
    seconds: AtomicU64,
}
impl clock::Clock for CatalogClock {
    fn now(&self) -> Instant {
        self.start + Duration::from_secs(self.seconds.load(Ordering::SeqCst))
    }
    fn utc(&self) -> DateTime<Utc> {
        // Keep cache observations current while advancing only scheduler time.
        self.wall
    }
    fn jitter(&self) -> u64 {
        0
    }
}
#[tokio::test]
async fn terminal_catalog_refresh_replaces_expired_due_after_each_traversal() {
    let dir = tempfile::tempdir().unwrap();
    let provider = Arc::new(Provider::default());
    let (mut runtime, vault, account) = setup(dir.path(), provider.clone()).await;
    let clock = Arc::new(CatalogClock {
        start: Instant::now(),
        wall: Utc::now(),
        seconds: AtomicU64::new(0),
    });
    Arc::get_mut(&mut runtime).unwrap().clock = clock.clone();
    demand(&runtime, &account).await;
    assert!(runtime.run_next().await);
    assert!(runtime.run_next().await);
    assert_eq!(provider.calls.load(Ordering::SeqCst), 2);
    assert!(!runtime.run_next().await);

    // An explicit second traversal starts after the old completion deadline.
    clock.seconds.store(301, Ordering::SeqCst);
    demand(&runtime, &account).await;
    runtime
        .refresh_issue_metadata(RefreshIssueMetadataRequest {
            account_id: account.id.clone(),
            authorization_epoch: account.authorization_epoch.clone(),
            repository_id: "repo".into(),
            kind: IssueMetadataKind::Labels,
        })
        .await
        .unwrap();
    assert!(runtime.run_next().await);
    assert!(runtime.run_next().await);
    assert_eq!(provider.calls.load(Ordering::SeqCst), 4);
    let loads = vault.loads.load(Ordering::SeqCst);
    assert!(
        !runtime.run_next().await,
        "a completed fresh traversal must replace the expired deadline"
    );
    assert_eq!(provider.calls.load(Ordering::SeqCst), 4);
    assert_eq!(vault.loads.load(Ordering::SeqCst), loads);

    clock.seconds.store(600, Ordering::SeqCst);
    demand(&runtime, &account).await;
    assert!(!runtime.run_next().await);
    assert_eq!(provider.calls.load(Ordering::SeqCst), 4);
    clock.seconds.store(601, Ordering::SeqCst);
    demand(&runtime, &account).await;
    assert!(runtime.run_next().await);
    assert!(runtime.run_next().await);
    assert_eq!(provider.calls.load(Ordering::SeqCst), 6);
    assert!(!runtime.run_next().await);
    runtime.shutdown().await.unwrap();
}
