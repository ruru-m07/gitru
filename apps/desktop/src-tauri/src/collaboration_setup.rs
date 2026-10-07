use crate::commands::collaboration::CollaborationState;
#[cfg(not(feature = "collaboration-harness"))]
use collaboration::credentials::{CredentialError, CredentialVault, SecretToken};
use collaboration::{CollaborationError, CollaborationRuntime, Store};
use std::sync::Arc;
#[cfg(not(feature = "collaboration-harness"))]
use std::sync::Mutex;
#[cfg(not(feature = "collaboration-harness"))]
use tauri::App;
use tauri::{Emitter, Manager};

#[cfg(not(feature = "e2e"))]
struct NativeVault {
    service: String,
    gate: Mutex<()>,
}

#[cfg(not(feature = "e2e"))]
impl NativeVault {
    fn entry(&self, reference: &str) -> Result<keyring::Entry, CredentialError> {
        keyring::Entry::new(&self.service, reference).map_err(|_| CredentialError::Unavailable)
    }
}

#[cfg(not(feature = "e2e"))]
impl CredentialVault for NativeVault {
    fn store(&self, reference: &str, token: &SecretToken) -> Result<(), CredentialError> {
        let _gate = self.gate.lock().map_err(|_| CredentialError::Unavailable)?;
        self.entry(reference)?
            .set_password(token.expose())
            .map_err(|_| CredentialError::Unavailable)
    }
    fn load(&self, reference: &str) -> Result<Option<SecretToken>, CredentialError> {
        let _gate = self.gate.lock().map_err(|_| CredentialError::Unavailable)?;
        match self.entry(reference)?.get_password() {
            Ok(value) => SecretToken::new(value).map(Some),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(_) => Err(CredentialError::Unavailable),
        }
    }
    fn delete(&self, reference: &str) -> Result<(), CredentialError> {
        let _gate = self.gate.lock().map_err(|_| CredentialError::Unavailable)?;
        match self.entry(reference)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(_) => Err(CredentialError::Unavailable),
        }
    }
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
            registry.register(Arc::new(
                collaboration::providers::gitlab::GitlabProvider::new()?,
            ))?;
            registry.register(Arc::new(
                collaboration::providers::bitbucket_cloud::BitbucketCloudProvider::new()?,
            ))?;
            #[cfg(not(feature = "e2e"))]
            let vault = Arc::new(NativeVault {
                service: format!("{}.collaboration", handle.config().identifier),
                gate: Mutex::new(()),
            });
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
