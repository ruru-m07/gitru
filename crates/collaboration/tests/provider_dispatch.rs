use async_trait::async_trait;
use collaboration::{credentials::*, providers::*, *};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

#[derive(Default)]
struct Vault(Mutex<HashMap<String, SecretToken>>);
impl CredentialVault for Vault {
    fn store(&self, reference: &str, token: &SecretToken) -> Result<(), CredentialError> {
        self.0
            .lock()
            .unwrap()
            .insert(reference.into(), token.clone());
        Ok(())
    }
    fn load(&self, reference: &str) -> Result<Option<SecretToken>, CredentialError> {
        Ok(self.0.lock().unwrap().get(reference).cloned())
    }
    fn delete(&self, reference: &str) -> Result<(), CredentialError> {
        self.0.lock().unwrap().remove(reference);
        Ok(())
    }
}
struct Adapter {
    instance: ProviderInstance,
    expected_token: &'static str,
    calls: Mutex<Vec<(String, FeedKind)>>,
    inbox: InboxSemantics,
}
#[async_trait]
impl CollaborationProvider for Adapter {
    fn kind(&self) -> ProviderKind {
        self.instance.provider
    }
    fn instance(&self) -> ProviderInstance {
        self.instance.clone()
    }
    fn profile(&self, account: &RemoteAccount) -> ProviderProfile {
        ProviderProfile::read_only(
            self.inbox,
            self.inbox == InboxSemantics::Todos || account.notifications_supported,
        )
    }
    async fn probe(&self, _: &SecretToken) -> Result<VerifiedAccount, ProviderError> {
        panic!("Fixture GL accounts are injected; live GL connection is not implemented")
    }
    async fn fetch_page(
        &self,
        token: &SecretToken,
        request: FeedRequest,
    ) -> Result<FetchPage, ProviderError> {
        assert_eq!(token.expose(), self.expected_token);
        assert_eq!(
            ProviderInstance::for_account(&request.account).unwrap(),
            self.instance
        );
        self.calls
            .lock()
            .unwrap()
            .push((request.account.id.clone(), request.kind));
        let repositories = if request.kind == FeedKind::Repositories {
            vec![RemoteRepository {
                id: "opaque-repo".into(),
                account_id: request.account.id.clone(),
                provider_id: "9007199254740993".into(),
                full_name: "group/subgroup/project".into(),
                name: "project".into(),
                web_url: format!("{}group/subgroup/project", self.instance.base_url),
                description: None,
                default_branch: None,
                selected: false,
            }]
        } else {
            vec![]
        };
        Ok(FetchPage {
            repositories,
            items: vec![],
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
}
fn account(id: &str, provider: ProviderKind, host: &str) -> RemoteAccount {
    RemoteAccount {
        id: id.into(),
        provider,
        host: host.into(),
        actor_id: "same-actor".into(),
        login: "fixture".into(),
        display_name: None,
        authorization_epoch: "1".into(),
        state: AccountState::Active,
        notifications_supported: false,
    }
}
async fn seed(store: &Store, vault: &Vault, account: RemoteAccount, token: &str) {
    let reference = format!("credential:{}", account.id);
    store
        .stage_credential(&account.id, &reference)
        .await
        .unwrap();
    vault
        .store(&reference, &SecretToken::new(token.into()).unwrap())
        .unwrap();
    store
        .commit_account_credential(account, &reference)
        .await
        .unwrap();
}
async fn until(mut check: impl AsyncFnMut() -> bool) {
    tokio::time::timeout(std::time::Duration::from_secs(3), async {
        while !check().await {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
}

#[tokio::test]
async fn runtime_routes_accounts_by_provider_host_port_and_base_path_without_secret_fallback() {
    let dir = tempfile::tempdir().unwrap();
    let store = Arc::new(Store::open(dir.path().join("db")).await.unwrap());
    let vault = Arc::new(Vault::default());
    let mut registry = ProviderRegistry::default();
    let mut adapters = vec![];
    for (id, provider, host, token, inbox) in [
        (
            "a",
            ProviderKind::Github,
            "github.com",
            "token-a",
            InboxSemantics::NativeNotifications,
        ),
        (
            "b",
            ProviderKind::Gitlab,
            "git.example:8443/one",
            "token-b",
            InboxSemantics::Todos,
        ),
        (
            "c",
            ProviderKind::Gitlab,
            "git.example:8443/two",
            "token-c",
            InboxSemantics::Todos,
        ),
    ] {
        let a = account(id, provider, host);
        let adapter = Arc::new(Adapter {
            instance: ProviderInstance::for_account(&a).unwrap(),
            expected_token: token,
            calls: Mutex::new(vec![]),
            inbox,
        });
        registry.register(adapter.clone()).unwrap();
        adapters.push(adapter);
        seed(&store, &vault, a, token).await;
    }
    let runtime = Arc::new(CollaborationRuntime::with_registry(
        store.clone(),
        vault,
        registry,
    ));
    assert_eq!(
        runtime.capabilities("b").await.unwrap().inbox_semantics,
        InboxSemantics::Todos
    );
    assert!(
        adapters.iter().all(|a| a.calls.lock().unwrap().is_empty()),
        "Local capabilities never probe/fetch provider HTTP"
    );
    runtime.clone().start_background();
    until(async || {
        adapters.iter().all(|a| {
            a.calls
                .lock()
                .unwrap()
                .iter()
                .any(|(_, kind)| *kind == FeedKind::Repositories)
        })
    })
    .await;
    until(async || {
        adapters[1..].iter().all(|a| {
            a.calls
                .lock()
                .unwrap()
                .iter()
                .any(|(_, kind)| *kind == FeedKind::Notifications)
        })
    })
    .await;
    for (index, id) in ["a", "b", "c"].into_iter().enumerate() {
        assert!(
            adapters[index]
                .calls
                .lock()
                .unwrap()
                .iter()
                .all(|(actor, _)| actor == id)
        );
    }
    assert!(
        !adapters[0]
            .calls
            .lock()
            .unwrap()
            .iter()
            .any(|(_, kind)| *kind == FeedKind::Notifications)
    );
    for id in ["a", "b", "c"] {
        assert_eq!(
            store.repositories(id).await.unwrap().repositories[0].provider_id,
            "9007199254740993"
        );
    }
    let resolution = store
        .resolve_resource(
            "b",
            ResourceLocator {
                instance_id: store.provider_instance("c").await.unwrap().id,
                kind: ResourceKind::Repository,
                locator_kind: LocatorKind::Canonical,
                value: "opaque-repo".into(),
                repository_path: None,
            },
        )
        .await;
    assert_eq!(resolution.unwrap_err().code, ErrorCode::InvalidInput);
}

#[tokio::test]
async fn unavailable_adapters_auth_and_permissions_are_distinct_from_unsupported_facets() {
    let dir = tempfile::tempdir().unwrap();
    let store = Arc::new(Store::open(dir.path().join("db")).await.unwrap());
    let vault = Arc::new(Vault::default());
    let a = account("a", ProviderKind::Github, "github.com");
    seed(&store, &vault, a.clone(), "token-a").await;
    store
        .upsert_account(account(
            "unregistered",
            ProviderKind::Gitlab,
            "another.example",
        ))
        .await
        .unwrap();
    let mut registry = ProviderRegistry::default();
    let adapter = Arc::new(Adapter {
        instance: ProviderInstance::for_account(&a).unwrap(),
        expected_token: "token-a",
        calls: Mutex::new(vec![]),
        inbox: InboxSemantics::NativeNotifications,
    });
    registry.register(adapter.clone()).unwrap();
    assert!(registry.register(adapter.clone()).is_err());
    let runtime = CollaborationRuntime::with_registry(store.clone(), vault, registry);
    let profile = runtime.capabilities("a").await.unwrap();
    let facet = |profile: &CapabilitySnapshot, kind| {
        profile
            .facets
            .iter()
            .find(|f| f.facet == kind)
            .unwrap()
            .clone()
    };
    assert_eq!(
        facet(&profile, ResourceFacet::Inbox).reason,
        Some(CapabilityReason::MissingScope)
    );
    assert_eq!(
        facet(&profile, ResourceFacet::Merge).state,
        CapabilityState::Unsupported
    );
    assert_eq!(
        runtime
            .refresh(RefreshRequest {
                account_id: "a".into(),
                repository_id: None,
                kind: Some(RemoteItemKind::Notification)
            })
            .await
            .unwrap_err()
            .code,
        ErrorCode::PermissionDenied
    );
    let missing = runtime.capabilities("unregistered").await.unwrap();
    assert!(
        missing
            .facets
            .iter()
            .all(|f| f.state == CapabilityState::Unavailable
                && f.reason == Some(CapabilityReason::AdapterUnavailable))
    );
    assert_eq!(
        runtime
            .refresh(RefreshRequest {
                account_id: "unregistered".into(),
                repository_id: None,
                kind: None
            })
            .await
            .unwrap_err()
            .code,
        ErrorCode::Unsupported
    );
    store
        .set_sync_status(
            "a",
            "1",
            "repositories",
            SyncStatus {
                state: SyncState::Error,
                error: Some(CollaborationError::new(
                    ErrorCode::PermissionDenied,
                    "denied",
                )),
                ..SyncStatus::default()
            },
        )
        .await
        .unwrap();
    assert_eq!(
        facet(
            &runtime.capabilities("a").await.unwrap(),
            ResourceFacet::Repositories
        )
        .reason,
        Some(CapabilityReason::PermissionDenied)
    );
    store
        .set_sync_status(
            "a",
            "1",
            "repositories",
            SyncStatus {
                state: SyncState::Offline,
                error: Some(CollaborationError::new(ErrorCode::Network, "offline")),
                ..SyncStatus::default()
            },
        )
        .await
        .unwrap();
    assert_eq!(
        facet(
            &runtime.capabilities("a").await.unwrap(),
            ResourceFacet::Repositories
        )
        .reason,
        Some(CapabilityReason::TemporarilyUnavailable)
    );
    store.disconnect("a").await.unwrap();
    let disconnected = runtime.capabilities("a").await.unwrap();
    assert_eq!(
        facet(&disconnected, ResourceFacet::Repositories).reason,
        Some(CapabilityReason::AuthenticationRequired)
    );
    assert_eq!(
        facet(&disconnected, ResourceFacet::Merge).state,
        CapabilityState::Unsupported
    );
    assert!(adapter.calls.lock().unwrap().is_empty());
}

#[tokio::test]
async fn unsupported_inbox_refresh_never_dispatches_adapter_work() {
    let dir = tempfile::tempdir().unwrap();
    let store = Arc::new(Store::open(dir.path().join("db")).await.unwrap());
    let vault = Arc::new(Vault::default());
    let a = account("a", ProviderKind::BitbucketCloud, "bitbucket.org");
    seed(&store, &vault, a.clone(), "fixture").await;
    let adapter = Arc::new(Adapter {
        instance: ProviderInstance::for_account(&a).unwrap(),
        expected_token: "fixture",
        calls: Mutex::new(vec![]),
        inbox: InboxSemantics::None,
    });
    let mut registry = ProviderRegistry::default();
    registry.register(adapter.clone()).unwrap();
    let runtime = CollaborationRuntime::with_registry(store, vault, registry);
    assert_eq!(
        runtime
            .capabilities("a")
            .await
            .unwrap()
            .facets
            .into_iter()
            .find(|f| f.facet == ResourceFacet::Inbox)
            .unwrap()
            .state,
        CapabilityState::Unsupported
    );
    assert_eq!(
        runtime
            .refresh(RefreshRequest {
                account_id: "a".into(),
                repository_id: None,
                kind: Some(RemoteItemKind::Notification)
            })
            .await
            .unwrap_err()
            .code,
        ErrorCode::Unsupported
    );
    assert!(adapter.calls.lock().unwrap().is_empty());
}
