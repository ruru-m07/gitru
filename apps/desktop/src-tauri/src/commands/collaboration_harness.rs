//! Feature-only fixture authority. Child content can discover only its own manifest.
use super::collaboration_local_links::CallerProof;
use crate::collaboration_harness::{
    self, HarnessControlRequest, HarnessReadKind, HarnessReceipt, HarnessState, HarnessStatus,
    HarnessStatusRequest, HarnessViewManifest, NativeHarness,
};
use collaboration::{test_harness::APPLICATION_ID, CollaborationError};
use tauri::{AppHandle, Manager, State, Webview};

struct HarnessCaller {
    proof: CallerProof,
    view: Webview,
    app: AppHandle,
    main_only: bool,
}
impl HarnessCaller {
    fn capture(view: Webview, main_only: bool) -> Result<Self, CollaborationError> {
        let app = view.app_handle().clone();
        if !allowed(
            view.label(),
            &view.url().map_err(|_| collaboration_harness::denied())?,
            main_only,
            &app.config().identifier,
        ) {
            return Err(collaboration_harness::denied());
        }
        let caller = Self {
            proof: CallerProof::capture(&view, &app)?,
            view,
            app,
            main_only,
        };
        caller.validate()?;
        Ok(caller)
    }
    fn validate(&self) -> Result<(), CollaborationError> {
        if !allowed(
            self.view.label(),
            &self
                .view
                .url()
                .map_err(|_| collaboration_harness::denied())?,
            self.main_only,
            &self.app.config().identifier,
        ) {
            return Err(collaboration_harness::denied());
        }
        self.proof.validate(&self.view, &self.app)
    }
    fn validate_runtime(&self, harness: &NativeHarness) -> Result<(), CollaborationError> {
        self.validate()?;
        harness.validate_runtime(&self.app)
    }
}
fn allowed(label: &str, address: &url::Url, main_only: bool, identifier: &str) -> bool {
    identifier == APPLICATION_ID
        && collaboration_harness::exact_local_origin(address)
        && (label == "main"
            || (!main_only && label.starts_with("tab-webview:") && label.len() <= 128))
}

#[tauri::command]
pub async fn collaboration_harness_control(
    request: HarnessControlRequest,
    view: Webview,
    state: State<'_, HarnessState>,
) -> Result<HarnessReceipt, CollaborationError> {
    let caller = HarnessCaller::capture(view, true)?;
    let harness = state.get().await?;
    caller.validate_runtime(harness)?;
    harness
        .execute(&caller.app, request, || caller.validate_runtime(harness))
        .await
}
#[tauri::command]
pub async fn collaboration_harness_status(
    request: HarnessStatusRequest,
    view: Webview,
    state: State<'_, HarnessState>,
) -> Result<HarnessStatus, CollaborationError> {
    let caller = HarnessCaller::capture(view, true)?;
    let harness = state.get().await?;
    caller.validate_runtime(harness)?;
    harness
        .status(request, || caller.validate_runtime(harness))
        .await
}
#[tauri::command]
pub async fn collaboration_harness_view_manifest(
    view: Webview,
    state: State<'_, HarnessState>,
) -> Result<HarnessViewManifest, CollaborationError> {
    let caller = HarnessCaller::capture(view, false)?;
    let harness = state.get().await?;
    caller.validate_runtime(harness)?;
    harness
        .manifest(&caller.app, &caller.view, || {
            caller.validate_runtime(harness)
        })
        .await
}

/// Holds only a captured authorized return, after SQLite has released its reader.
/// The unchanged SDK still owns authorization/reset suppression of that old value.
pub(crate) struct CapturedReadOwner {
    caller: HarnessCaller,
}
impl CapturedReadOwner {
    pub(crate) fn capture(view: &Webview) -> Result<Self, CollaborationError> {
        Ok(Self {
            caller: HarnessCaller::capture(view.clone(), false)?,
        })
    }
    pub(crate) fn current(&self) -> bool {
        self.caller.validate().is_ok()
    }
    pub(crate) fn matches(&self, view: &Webview) -> bool {
        let expected = self.caller.view.resources_table();
        let observed = view.resources_table();
        self.caller.view.label() == view.label()
            && std::ptr::eq(&*expected, &*observed)
            && self.current()
    }
}

pub(super) struct LocalReturnProof {
    caller: HarnessCaller,
}
impl LocalReturnProof {
    pub(super) fn capture(view: &Webview) -> Result<Self, CollaborationError> {
        Ok(Self {
            caller: HarnessCaller::capture(view.clone(), false)?,
        })
    }
    pub(super) async fn hold(
        &self,
        kind: HarnessReadKind,
        account: &str,
        subject: &str,
    ) -> Result<(), CollaborationError> {
        let state = self.caller.app.state::<HarnessState>();
        let harness = state.get().await?;
        self.caller.validate_runtime(harness)?;
        harness
            .hold_read(&self.caller.view, kind, account, subject, || {
                self.caller.validate_runtime(harness)
            })
            .await
    }
    pub(super) async fn record_hydrate(&self) -> Result<(), CollaborationError> {
        let state = self.caller.app.state::<HarnessState>();
        let harness = state.get().await?;
        self.caller.validate_runtime(harness)?;
        harness.record_hydrate()
    }
}

pub(crate) async fn dispose_harness_child_owner(
    app: &AppHandle,
    runtime: &collaboration::CollaborationRuntime,
    label: &str,
) -> Result<(), CollaborationError> {
    let state = app.state::<super::collaboration::CollaborationState>();
    let mut hosts = state.demand_hosts.lock().await;
    let activity = runtime.demand_owner_activity(label).await?;
    runtime
        .dispose_demand_owner(label, &activity.generation)
        .await?;
    hosts.remove(label);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn controller_is_main_only_on_exact_compiled_native_origin() {
        for address in [
            "tauri://localhost/app/pulls",
            "https://tauri.localhost/app/pulls",
            "http://tauri.localhost/app/pulls",
        ] {
            let address = url::Url::parse(address).unwrap();
            assert!(allowed("main", &address, true, APPLICATION_ID));
            assert!(!allowed(
                "tab-webview:ruru103:child",
                &address,
                true,
                APPLICATION_ID
            ));
            assert!(allowed(
                "tab-webview:ruru103:child",
                &address,
                false,
                APPLICATION_ID
            ));
            assert!(!allowed("other", &address, false, APPLICATION_ID));
            assert!(!allowed("main", &address, true, "com.ruru.gitru"));
        }
    }
    #[test]
    fn debug_and_remote_origins_do_not_inherit_fixture_authority() {
        for address in [
            "http://localhost:1420/app/pulls",
            "http://127.0.0.1:1420/app/pulls",
            "tauri://localhost:4445/app/pulls",
            "https://tauri.localhost:443/app/pulls",
            "https://github.com",
            "file:///tmp/local.html",
            "tauri://actor@localhost/app/pulls",
            "https://tauri.localhost.evil.com/app/pulls",
        ] {
            let address = url::Url::parse(address).unwrap();
            // An explicit default HTTPS port is normalized by URL parsing. Its
            // resulting native authority is identical to the portless address.
            if address.host_str() == Some("tauri.localhost")
                && address.port().is_none()
                && address.username().is_empty()
            {
                continue;
            }
            for main in [false, true] {
                assert!(!allowed("main", &address, main, APPLICATION_ID));
                assert!(!allowed("tab-webview:1", &address, main, APPLICATION_ID));
            }
        }
    }
}
