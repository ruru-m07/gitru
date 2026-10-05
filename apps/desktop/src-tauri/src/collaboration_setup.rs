use crate::commands::collaboration::CollaborationState;
use collaboration::{
    credentials::{CredentialError, CredentialVault, SecretToken},
    CollaborationError, CollaborationRuntime, Store,
};
use std::sync::{Arc, Mutex};
use tauri::{App, Emitter, Manager};

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
#[cfg(feature = "e2e")]
#[derive(Default)]
struct TestVault(Mutex<std::collections::HashMap<String, SecretToken>>);
#[cfg(feature = "e2e")]
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

pub fn setup(app: &App) {
    let handle = app.handle().clone();
    // App identifier separates release/development/E2E credentials and data.
    #[cfg(not(feature = "e2e"))]
    let service = format!("{}.collaboration", app.config().identifier);
    tauri::async_runtime::spawn(async move {
        let result = async {
            let dir = handle
                .path()
                .app_data_dir()
                .map_err(|_| CollaborationError::storage())?;
            std::fs::create_dir_all(&dir).map_err(|_| CollaborationError::storage())?;
            let store = Arc::new(Store::open(dir.join("collaboration.sqlite3")).await?);
            let provider = collaboration::providers::github::GithubProvider::new()?;
            let mut registry = collaboration::providers::ProviderRegistry::default();
            registry.register(Arc::new(provider))?;
            registry.register(Arc::new(
                collaboration::providers::gitlab::GitlabProvider::new()?,
            ))?;
            registry.register(Arc::new(
                collaboration::providers::bitbucket_cloud::BitbucketCloudProvider::new()?,
            ))?;
            #[cfg(not(feature = "e2e"))]
            let vault = Arc::new(NativeVault {
                service,
                gate: Mutex::new(()),
            });
            #[cfg(feature = "e2e")]
            let vault = Arc::new(TestVault::default());
            #[cfg(not(feature = "e2e"))]
            let github_cli = collaboration::github_cli::GithubCli::native();
            #[cfg(feature = "e2e")]
            let github_cli = collaboration::github_cli::GithubCli::disabled();
            let visibility_handle = handle.clone();
            Ok::<_, CollaborationError>(Arc::new(
                CollaborationRuntime::with_registry(store, vault, registry)
                    .with_github_cli(github_cli)
                    .with_demand_visibility_probe(Arc::new(move |owner| {
                        crate::commands::collaboration_demand::owner_window_available(
                            &visibility_handle,
                            owner,
                        )
                    })),
            ))
        }
        .await;
        if let Ok(runtime) = &result {
            let mut changes = runtime.subscribe();
            let event_handle = handle.clone();
            tauri::async_runtime::spawn(async move {
                loop {
                    match changes.recv().await {
                        Ok(hint) => {
                            // A wake hint contains only a revision. Views obtain scoped
                            // content through authorized local snapshot commands.
                            let _ = event_handle.emit("gitru:collaboration-change", hint);
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                        Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                    }
                }
            });
            crate::commands::collaboration_demand::observe_window_activity(
                handle.clone(),
                runtime.clone(),
            );
            runtime.clone().start_background();
        } else {
            log::error!("Collaboration storage initialization failed");
        }
        let ready_runtime = result.as_ref().ok().cloned();
        let _ = handle.state::<CollaborationState>().runtime.set(result);
        if let Some(runtime) = ready_runtime {
            // An early local reader can outlast the bounded startup wait. Publish
            // readiness after the state is installed, even with no provider work.
            if let Ok(revision) = runtime.store().revision().await {
                let _ = handle.emit(
                    "gitru:collaboration-change",
                    collaboration::ChangeHint { revision },
                );
            }
        }
    });
}
