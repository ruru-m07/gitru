//! The native caller supplies lease ownership; content views cannot activate peers.
use super::collaboration::{CollaborationState, Operation, authorize, caller_allowed};
use collaboration::{
    AcquireDemandRequest, CollaborationError, DemandLeaseReceipt, DemandOwnerActivity,
    DemandRenewalReceipt, ErrorCode, ReleaseDemandRequest, RenewDemandRequest,
};
use serde::Serialize;
use tauri::{AppHandle, Emitter, EventTarget, Manager, State, Webview};

const ACTIVITY_EVENT: &str = "collaboration:owner-activity";

#[derive(Clone, Serialize)]
struct ActivityHint {
    owner_label: String,
    activity: DemandOwnerActivity,
}

fn window_available(view: &Webview) -> bool {
    view.window().is_visible().unwrap_or(false) && !view.window().is_minimized().unwrap_or(true)
}

fn emit_activity(view: &Webview, activity: &DemandOwnerActivity) {
    let _ = view.app_handle().emit_to(
        EventTarget::webview(view.label()),
        ACTIVITY_EVENT,
        ActivityHint {
            owner_label: view.label().into(),
            activity: activity.clone(),
        },
    );
}

fn validate_target(
    label: &str,
    address: Option<&url::Url>,
    allow_absent: bool,
) -> Result<(), CollaborationError> {
    let child = label.strip_prefix("tab-webview:").is_some_and(|suffix| {
        !suffix.is_empty()
            && suffix.bytes().all(|byte| {
                byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b':' | b'/')
            })
    });
    if label.len() > 128 || (label != "main" && !child) {
        return Err(CollaborationError::invalid("Invalid demand owner"));
    }
    if let Some(address) = address {
        if !caller_allowed(label, address, Operation::DemandActivity) {
            return Err(CollaborationError::new(
                ErrorCode::PermissionDenied,
                "The content view is unavailable",
            ));
        }
    } else if !allow_absent || !child {
        return Err(CollaborationError::new(
            ErrorCode::NotFound,
            "The content view is closed",
        ));
    }
    Ok(())
}

fn target_view(
    app: &AppHandle,
    label: &str,
    allow_absent: bool,
) -> Result<Option<Webview>, CollaborationError> {
    let target = app.get_webview(label);
    let address = target
        .as_ref()
        .map(|view| view.url())
        .transpose()
        .map_err(|_| {
            CollaborationError::new(
                ErrorCode::PermissionDenied,
                "The content view is unavailable",
            )
        })?;
    validate_target(label, address.as_ref(), allow_absent)?;
    Ok(target)
}

pub(crate) fn owner_window_available(app: &AppHandle, label: &str) -> bool {
    target_view(app, label, false)
        .ok()
        .flatten()
        .is_some_and(|view| window_available(&view))
}

async fn own_activity(
    view: &Webview,
    state: &CollaborationState,
) -> Result<DemandOwnerActivity, CollaborationError> {
    let runtime = state.get().await?;
    let mut hosts = state.demand_hosts.lock().await;
    let activity = runtime.demand_owner_activity(view.label()).await?;
    // Standalone content in main has no tab host. Normal child activity is
    // activated only by the main host after native show succeeds.
    let standalone = view.label() == "main"
        && view.url().is_ok_and(|url| {
            url.path().starts_with("/app/")
                && url
                    .query_pairs()
                    .any(|(key, value)| key == "embedded" && value == "1")
        });
    let desired = *hosts.entry(view.label().into()).or_insert(standalone) && window_available(view);
    if desired == activity.active {
        return Ok(activity);
    }
    let changed = runtime
        .set_demand_owner_activity(view.label(), &activity.generation, desired)
        .await?;
    emit_activity(view, &changed);
    Ok(changed)
}

#[tauri::command]
pub async fn collaboration_demand_activity(
    view: Webview,
    state: State<'_, CollaborationState>,
) -> Result<DemandOwnerActivity, CollaborationError> {
    authorize(&view, Operation::DemandActivity)?;
    own_activity(&view, &state).await
}

#[tauri::command]
pub async fn collaboration_acquire_demand(
    view: Webview,
    state: State<'_, CollaborationState>,
    request: AcquireDemandRequest,
) -> Result<DemandLeaseReceipt, CollaborationError> {
    authorize(&view, Operation::AcquireDemand)?;
    own_activity(&view, &state).await?;
    state
        .get()
        .await?
        .acquire_demand(view.label(), request)
        .await
}

#[tauri::command]
pub async fn collaboration_renew_demand(
    view: Webview,
    state: State<'_, CollaborationState>,
    request: RenewDemandRequest,
) -> Result<DemandRenewalReceipt, CollaborationError> {
    authorize(&view, Operation::RenewDemand)?;
    own_activity(&view, &state).await?;
    state.get().await?.renew_demand(view.label(), request).await
}

#[tauri::command]
pub async fn collaboration_release_demand(
    view: Webview,
    state: State<'_, CollaborationState>,
    request: ReleaseDemandRequest,
) -> Result<(), CollaborationError> {
    authorize(&view, Operation::ReleaseDemand)?;
    state
        .get()
        .await?
        .release_demand(view.label(), request)
        .await
}

#[tauri::command]
pub async fn collaboration_inspect_demand_owner(
    view: Webview,
    app: AppHandle,
    state: State<'_, CollaborationState>,
    owner_label: String,
) -> Result<DemandOwnerActivity, CollaborationError> {
    authorize(&view, Operation::InspectDemandOwner)?;
    match target_view(&app, &owner_label, true)? {
        Some(target) => own_activity(&target, &state).await,
        // A cold replacement must fence the old incarnation before its native
        // label becomes visible again. This read does not restore desired state.
        None => state.get().await?.demand_owner_activity(&owner_label).await,
    }
}

#[tauri::command]
pub async fn collaboration_set_demand_owner_activity(
    view: Webview,
    app: AppHandle,
    state: State<'_, CollaborationState>,
    owner_label: String,
    expected_generation: String,
    active: bool,
) -> Result<DemandOwnerActivity, CollaborationError> {
    authorize(&view, Operation::SetDemandOwner)?;
    let target = target_view(&app, &owner_label, false)?.ok_or_else(|| {
        CollaborationError::new(ErrorCode::NotFound, "The content view is closed")
    })?;
    let mut hosts = state.demand_hosts.lock().await;
    let activity = state
        .get()
        .await?
        .set_demand_owner_activity(
            &owner_label,
            &expected_generation,
            active && window_available(&target),
        )
        .await?;
    hosts.insert(owner_label, active);
    emit_activity(&target, &activity);
    Ok(activity)
}

#[tauri::command]
pub async fn collaboration_dispose_demand_owner(
    view: Webview,
    app: AppHandle,
    state: State<'_, CollaborationState>,
    owner_label: String,
    expected_generation: String,
) -> Result<(), CollaborationError> {
    authorize(&view, Operation::DisposeDemandOwner)?;
    let target = target_view(&app, &owner_label, true)?;
    let mut hosts = state.demand_hosts.lock().await;
    let runtime = state.get().await?;
    if let Some(target) = target {
        let inactive = runtime
            .set_demand_owner_activity(&owner_label, &expected_generation, false)
            .await?;
        emit_activity(&target, &inactive);
        runtime
            .dispose_demand_owner(&owner_label, &inactive.generation)
            .await?;
    } else {
        // The observer may already have removed this absent incarnation. The
        // core's idempotent dispose does not invent a replacement generation.
        runtime
            .dispose_demand_owner(&owner_label, &expected_generation)
            .await?;
    }
    hosts.remove(&owner_label);
    Ok(())
}

/// A single local window observer restores the host's desired owner after
/// minimize/hide. Dispatch uses the same physical gate independently of this
/// event delivery cadence; this timer never creates provider or SQLite demand.
pub(crate) fn observe_window_activity(
    app: AppHandle,
    runtime: std::sync::Arc<collaboration::CollaborationRuntime>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut timer = tokio::time::interval(std::time::Duration::from_secs(1));
        timer.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            timer.tick().await;
            let state = app.state::<CollaborationState>();
            let mut hosts = state.demand_hosts.lock().await;
            let labels: Vec<_> = hosts.keys().cloned().collect();
            for label in labels {
                let Ok(activity) = runtime.demand_owner_activity(&label).await else {
                    continue;
                };
                let Ok(Some(target)) = target_view(&app, &label, false) else {
                    let _ = runtime
                        .dispose_demand_owner(&label, &activity.generation)
                        .await;
                    hosts.remove(&label);
                    continue;
                };
                let desired =
                    hosts.get(&label).copied().unwrap_or(false) && window_available(&target);
                if desired != activity.active {
                    if let Ok(changed) = runtime
                        .set_demand_owner_activity(&label, &activity.generation, desired)
                        .await
                    {
                        emit_activity(&target, &changed);
                    }
                }
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn absent_reset_is_limited_to_valid_managed_children() {
        assert!(validate_target("tab-webview:abc-123", None, true).is_ok());
        assert_eq!(
            validate_target("tab-webview:abc-123", None, false)
                .unwrap_err()
                .code,
            ErrorCode::NotFound
        );
        assert_eq!(
            validate_target("main", None, true).unwrap_err().code,
            ErrorCode::NotFound
        );
        for label in [
            "foreign",
            "tab-webview:",
            "tab-webview: ",
            "tab-webview:a\n",
            "tab-webview:é",
            "tab-webview:a?other",
        ] {
            assert_eq!(
                validate_target(label, None, true).unwrap_err().code,
                ErrorCode::InvalidInput
            );
        }
        assert_eq!(
            validate_target(&format!("tab-webview:{}", "a".repeat(128)), None, true)
                .unwrap_err()
                .code,
            ErrorCode::InvalidInput
        );
    }

    #[test]
    fn present_targets_must_be_local_even_when_absent_reset_is_permitted() {
        let native = url::Url::parse("tauri://localhost/app/inbox?embedded=1").unwrap();
        assert!(validate_target("tab-webview:1", Some(&native), true).is_ok());
        assert!(validate_target("tab-webview:1", Some(&native), false).is_ok());
        for address in [
            "https://github.com/app/inbox",
            "https://tauri.localhost:4445/app/inbox",
            "tauri://actor@localhost/app/inbox",
        ] {
            let remote = url::Url::parse(address).unwrap();
            assert_eq!(
                validate_target("tab-webview:1", Some(&remote), true)
                    .unwrap_err()
                    .code,
                ErrorCode::PermissionDenied
            );
            assert_eq!(
                validate_target("tab-webview:1", Some(&remote), false)
                    .unwrap_err()
                    .code,
                ErrorCode::PermissionDenied
            );
        }
    }
}
