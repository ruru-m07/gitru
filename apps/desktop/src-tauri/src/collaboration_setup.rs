use crate::commands::collaboration::CollaborationState;
#[cfg(not(feature = "e2e"))]
use crate::native_vault::NativeVault;
#[cfg(all(feature = "e2e", not(feature = "collaboration-harness")))]
use collaboration::credentials::{CredentialError, CredentialVault, SecretToken};
#[cfg(feature = "native-keyed-storage")]
use collaboration::database_keys::{DatabaseCreation, DatabaseKeySession, DatabaseKeyVault};
#[cfg(all(feature = "native-keyed-storage", not(feature = "e2e")))]
use collaboration::database_keys::{DatabaseKey, DatabaseKeyError, DatabaseKeyIdentity};
use collaboration::{CollaborationError, CollaborationRuntime, Store};
use std::sync::Arc;
#[cfg(all(feature = "e2e", not(feature = "collaboration-harness")))]
use std::sync::Mutex;
#[cfg(not(feature = "collaboration-harness"))]
use tauri::App;
use tauri::{Emitter, Manager};
#[cfg(all(feature = "native-keyed-storage", not(feature = "e2e")))]
use zeroize::Zeroizing;

#[cfg(feature = "native-keyed-storage")]
fn keyed_error() -> CollaborationError {
    CollaborationError::new(
        collaboration::ErrorCode::NotReady,
        "Native keyed collaboration storage could not open; files were preserved",
    )
}

#[cfg(all(feature = "native-keyed-storage", not(feature = "e2e")))]
struct NativeDatabaseVault {
    gate: Mutex<()>,
}

#[cfg(all(feature = "native-keyed-storage", not(feature = "e2e")))]
impl NativeDatabaseVault {
    fn entry(&self, identity: &DatabaseKeyIdentity) -> Result<keyring::Entry, DatabaseKeyError> {
        keyring::Entry::new(
            collaboration::database_keys::DATABASE_KEY_SERVICE,
            &identity.vault_reference(),
        )
        .map_err(|_| DatabaseKeyError::VaultUnavailable)
    }
}

#[cfg(all(feature = "native-keyed-storage", not(feature = "e2e")))]
impl DatabaseKeyVault for NativeDatabaseVault {
    fn load(
        &self,
        identity: &DatabaseKeyIdentity,
    ) -> Result<Option<DatabaseKey>, DatabaseKeyError> {
        let _gate = self
            .gate
            .lock()
            .map_err(|_| DatabaseKeyError::VaultUnavailable)?;
        let encoded = match self.entry(identity)?.get_password() {
            Ok(value) => Zeroizing::new(value),
            Err(keyring::Error::NoEntry) => return Ok(None),
            Err(_) => return Err(DatabaseKeyError::VaultUnavailable),
        };
        if encoded.len() != 64 || !encoded.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(DatabaseKeyError::VaultInvalidKey);
        }
        let mut bytes = [0_u8; 32];
        for (index, output) in bytes.iter_mut().enumerate() {
            *output = u8::from_str_radix(&encoded[index * 2..index * 2 + 2], 16)
                .map_err(|_| DatabaseKeyError::VaultInvalidKey)?;
        }
        Ok(Some(DatabaseKey::from_bytes(bytes)))
    }

    fn store_new(
        &self,
        _identity: &DatabaseKeyIdentity,
        _key: &DatabaseKey,
    ) -> Result<(), DatabaseKeyError> {
        // Application startup never creates keys. Activation must publish a
        // separately verified database/key pair before this adapter can load it.
        Err(DatabaseKeyError::CreationNotAuthorized)
    }
}

#[cfg(feature = "native-keyed-storage")]
pub(crate) async fn open_keyed_store(
    path: std::path::PathBuf,
    vault: Arc<dyn DatabaseKeyVault>,
    creation: DatabaseCreation,
) -> Result<Store, CollaborationError> {
    let session = tokio::task::spawn_blocking(move || {
        DatabaseKeySession::prepare(&path, vault.as_ref(), creation)
    })
    .await
    .map_err(|_| keyed_error())?
    .map_err(|error| {
        log::error!("Native keyed collaboration key preparation failed: {error}");
        keyed_error()
    })?;
    let factory = Arc::new(gitru_keyed_connections::KeyedConnectionFactory::new(
        session,
    ));
    Store::open_keyed(factory).await.map_err(|error| {
        log::error!("Native keyed collaboration store open failed: {error}");
        error
    })
}

// Packaged automation never opens or alters a person's native keychain.
#[cfg(all(feature = "e2e", not(feature = "collaboration-harness")))]
#[derive(Default)]
struct TestVault(Mutex<std::collections::HashMap<String, SecretToken>>);
#[cfg(all(feature = "e2e", not(feature = "collaboration-harness")))]
impl CredentialVault for TestVault {
    fn store(&self, reference: &str, token: &SecretToken) -> Result<(), CredentialError> {
        self.0
            .lock()
            .map_err(|_| CredentialError::Unavailable)?
            .insert(reference.into(), token.clone());
        Ok(())
    }
    fn load(&self, reference: &str) -> Result<Option<SecretToken>, CredentialError> {
        Ok(self
            .0
            .lock()
            .map_err(|_| CredentialError::Unavailable)?
            .get(reference)
            .cloned())
    }
    fn delete(&self, reference: &str) -> Result<(), CredentialError> {
        self.0
            .lock()
            .map_err(|_| CredentialError::Unavailable)?
            .remove(reference);
        Ok(())
    }
}

pub(crate) fn database_path(
    handle: &tauri::AppHandle,
) -> Result<std::path::PathBuf, CollaborationError> {
    let state = handle.state::<CollaborationState>();
    if let Some(path) = state.configured_database_path() {
        return Ok(path.to_path_buf());
    }
    #[cfg(feature = "collaboration-harness")]
    return Err(CollaborationError::new(
        collaboration::ErrorCode::NotReady,
        "Retained collaboration fixture storage is not configured",
    ));
    #[cfg(not(feature = "collaboration-harness"))]
    {
        let path = handle
            .path()
            .app_data_dir()
            .map_err(|_| CollaborationError::storage())?
            .join("collaboration.sqlite3");
        state.configure_database_path(path.clone())?;
        Ok(path)
    }
}
pub(crate) async fn build_runtime(
    handle: &tauri::AppHandle,
    previous: Option<&CollaborationRuntime>,
) -> Result<Arc<CollaborationRuntime>, CollaborationError> {
    #[cfg(feature = "collaboration-harness")]
    if previous.is_none() {
        return Err(CollaborationError::new(
            collaboration::ErrorCode::NotReady,
            "Retained fixture recovery needs its original provider configuration",
        ));
    }
    let path = database_path(handle)?;
    let dir = path.parent().ok_or_else(CollaborationError::storage)?;
    std::fs::create_dir_all(dir).map_err(|_| CollaborationError::storage())?;
    #[cfg(all(feature = "native-keyed-storage", not(feature = "e2e")))]
    let keyed = collaboration::database_keys::requires_keyed_open(&path);
    #[cfg(all(feature = "native-keyed-storage", not(feature = "e2e")))]
    let store = if keyed {
        let vault = Arc::new(NativeDatabaseVault {
            gate: Mutex::new(()),
        });
        Arc::new(open_keyed_store(path.clone(), vault, DatabaseCreation::ExistingOnly).await?)
    } else {
        Arc::new(Store::open(path).await?)
    };
    #[cfg(any(
        not(feature = "native-keyed-storage"),
        all(feature = "native-keyed-storage", feature = "e2e")
    ))]
    let store = Arc::new(Store::open(path).await?);
    if let Some(previous) = previous {
        let result = previous.replacement(store.clone()).map(Arc::new);
        if result.is_err() {
            store.close().await?;
        }
        return result;
    }
    #[cfg(feature = "collaboration-harness")]
    return Err(CollaborationError::new(
        collaboration::ErrorCode::NotReady,
        "Retained fixture recovery needs its original provider configuration",
    ));
    #[cfg(not(feature = "collaboration-harness"))]
    {
        let result = (|| {
            let provider = collaboration::providers::github::GithubProvider::new()?;
            let mut registry = collaboration::providers::ProviderRegistry::default();
            registry.register(Arc::new(provider))?;
            registry.register_github_text_edits()?;
            registry.register_github_workflow_state()?;
            registry.register_github_guarded_merge()?;
            registry.register_github_comments()?;
            registry.register_github_issue_creation()?;
            registry.register_github_issue_metadata_creation()?;
            registry.register_github_review_submission()?;
            registry.register_github_pull_creation()?;
            registry.register(Arc::new(
                collaboration::providers::gitlab::GitlabProvider::new()?,
            ))?;
            registry.register(Arc::new(
                collaboration::providers::bitbucket_cloud::BitbucketCloudProvider::new()?,
            ))?;
            registry.register_provider_inbox_actions()?;
            #[cfg(not(feature = "e2e"))]
            let vault = Arc::new(NativeVault::new(format!(
                "{}.collaboration",
                handle.config().identifier
            )));
            #[cfg(feature = "e2e")]
            let vault = Arc::new(TestVault::default());
            #[cfg(not(feature = "e2e"))]
            let github_cli = collaboration::github_cli::GithubCli::native();
            #[cfg(feature = "e2e")]
            let github_cli = collaboration::github_cli::GithubCli::disabled();
            let visibility_handle = handle.clone();
            Ok(Arc::new(
                CollaborationRuntime::with_registry(store.clone(), vault, registry)
                    .with_github_cli(github_cli)
                    .with_demand_visibility_probe(Arc::new(move |owner| {
                        crate::commands::collaboration_demand::owner_window_available(
                            &visibility_handle,
                            owner,
                        )
                    })),
            ))
        })();
        if result.is_err() {
            store.close().await?;
        }
        result
    }
}

pub(crate) fn start_services(handle: &tauri::AppHandle, runtime: Arc<CollaborationRuntime>) {
    let mut changes = runtime.subscribe();
    let event_handle = handle.clone();
    let relay = tokio::spawn(async move {
        loop {
            match changes.recv().await {
                Ok(hint) => {
                    let _ = event_handle.emit("gitru:collaboration-change", hint);
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
            }
        }
    });
    let state = handle.state::<CollaborationState>();
    state.own_service(relay);
    state.own_service(
        crate::commands::collaboration_demand::observe_window_activity(
            handle.clone(),
            runtime.clone(),
        ),
    );
    runtime.start_background();
}
#[cfg(not(feature = "collaboration-harness"))]
pub fn setup(app: &App) {
    let handle = app.handle().clone();
    tauri::async_runtime::spawn(async move {
        let result = build_runtime(&handle, None).await;
        if let Ok(runtime) = &result {
            start_services(&handle, runtime.clone());
        } else {
            log::error!("Collaboration storage initialization failed");
        }
        let ready = result.as_ref().ok().cloned();
        let _ = handle.state::<CollaborationState>().runtime.set(result);
        if let Some(runtime) = ready {
            if let Ok(revision) = runtime.store().revision().await {
                let _ = handle.emit(
                    "gitru:collaboration-change",
                    collaboration::ChangeHint { revision },
                );
            }
        }
    });
}
