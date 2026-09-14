use serde::Serialize;
use std::sync::{
    atomic::{AtomicBool, AtomicU64, Ordering},
    Arc,
};
use tauri::Emitter;
use tauri_plugin_updater::UpdaterExt;
use url::Url;

const DEFAULT_UPDATER_BASE_URL: &str = "https://release.gitru.app";
const PRODUCTION_IDENTIFIER: &str = "com.ruru.gitru";

#[derive(Serialize)]
pub struct UpdateCheckResponse {
    pub available: bool,
    pub channel: String,
    pub current_version: String,
    pub version: Option<String>,
    pub notes: Option<String>,
    pub pub_date: Option<String>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdaterDownloadProgressEvent {
    pub phase: String,
    pub content_length: Option<u64>,
    pub chunk_length: Option<usize>,
    pub downloaded: Option<u64>,
    pub percent: Option<f64>,
    pub channel: String,
    pub version: Option<String>,
}

fn normalize_channel(channel: &str) -> Result<&str, String> {
    match channel.trim().to_ascii_lowercase().as_str() {
        "stable" => Ok("stable"),
        "beta" => Ok("beta"),
        _ => Err("Invalid update channel. Expected 'stable' or 'beta'".to_string()),
    }
}

fn endpoint_for_channel(channel: &str) -> Result<String, String> {
    let channel = normalize_channel(channel)?;
    let base = std::env::var("UPDATER_BASE_URL")
        .unwrap_or_else(|_| DEFAULT_UPDATER_BASE_URL.to_string())
        .trim_end_matches('/')
        .to_string();
    Ok(format!("{base}/{channel}/latest.json"))
}

fn ensure_production_identity(identifier: &str) -> Result<(), String> {
    if cfg!(feature = "qualification") {
        Err("Updates are disabled in qualification builds".to_string())
    } else if identifier == PRODUCTION_IDENTIFIER {
        Ok(())
    } else {
        Err("Updates are disabled outside the production application identity".to_string())
    }
}

#[tauri::command]
pub async fn check_for_update_by_channel(
    app: tauri::AppHandle,
    channel: String,
) -> Result<UpdateCheckResponse, String> {
    ensure_production_identity(&app.config().identifier)?;
    let channel = normalize_channel(&channel)?.to_string();
    let endpoint = endpoint_for_channel(&channel)?;
    let endpoint =
        Url::parse(&endpoint).map_err(|e| format!("Invalid updater endpoint URL: {e}"))?;

    let updater = app
        .updater_builder()
        .endpoints(vec![endpoint])
        .map_err(|e| format!("Failed to configure updater endpoint: {e}"))?
        .build()
        .map_err(|e| format!("Failed to build updater: {e}"))?;

    let update = updater
        .check()
        .await
        .map_err(|e| format!("Failed to check for updates: {e}"))?;

    if let Some(update) = update {
        return Ok(UpdateCheckResponse {
            available: true,
            channel,
            current_version: update.current_version,
            version: Some(update.version),
            notes: update.body,
            pub_date: update.date.map(|d| d.to_string()),
        });
    }

    Ok(UpdateCheckResponse {
        available: false,
        channel,
        current_version: app.package_info().version.to_string(),
        version: None,
        notes: None,
        pub_date: None,
    })
}

#[tauri::command]
pub async fn download_and_install_update_by_channel(
    app: tauri::AppHandle,
    channel: String,
) -> Result<String, String> {
    ensure_production_identity(&app.config().identifier)?;
    let channel = normalize_channel(&channel)?.to_string();
    let endpoint = endpoint_for_channel(&channel)?;
    let endpoint =
        Url::parse(&endpoint).map_err(|e| format!("Invalid updater endpoint URL: {e}"))?;

    let updater = app
        .updater_builder()
        .endpoints(vec![endpoint])
        .map_err(|e| format!("Failed to configure updater endpoint: {e}"))?
        .build()
        .map_err(|e| format!("Failed to build updater: {e}"))?;

    let update = updater
        .check()
        .await
        .map_err(|e| format!("Failed to check for updates: {e}"))?;

    let Some(update) = update else {
        return Ok("No update available".to_string());
    };

    let next_version = update.version.clone();
    let version = Some(next_version.clone());
    let downloaded = Arc::new(AtomicU64::new(0));
    let content_length = Arc::new(AtomicU64::new(0));
    let started = Arc::new(AtomicBool::new(false));

    let app_handle = app.clone();
    let event_channel = channel.clone();
    let event_version = version.clone();
    let progress_downloaded = downloaded.clone();
    let progress_content_length = content_length.clone();
    let progress_started = started.clone();

    update
        .download_and_install(
            move |chunk_length, total_length| {
                let total_length_value = total_length.unwrap_or(0);
                progress_content_length.store(total_length_value, Ordering::Relaxed);
                if !progress_started.swap(true, Ordering::Relaxed) {
                    let _ = app_handle.emit(
                        "updater://download-progress",
                        UpdaterDownloadProgressEvent {
                            phase: "Started".to_string(),
                            content_length: total_length,
                            chunk_length: None,
                            downloaded: Some(0),
                            percent: Some(0.0),
                            channel: event_channel.clone(),
                            version: event_version.clone(),
                        },
                    );
                }

                let downloaded_now = progress_downloaded
                    .fetch_add(chunk_length as u64, Ordering::Relaxed)
                    .saturating_add(chunk_length as u64);
                let percent = if total_length_value == 0 {
                    0.0
                } else {
                    (downloaded_now as f64 / total_length_value as f64 * 100.0).clamp(0.0, 100.0)
                };
                let _ = app_handle.emit(
                    "updater://download-progress",
                    UpdaterDownloadProgressEvent {
                        phase: "Progress".to_string(),
                        content_length: total_length,
                        chunk_length: Some(chunk_length),
                        downloaded: Some(downloaded_now),
                        percent: Some(percent),
                        channel: event_channel.clone(),
                        version: event_version.clone(),
                    },
                );
            },
            {
                let app_handle = app.clone();
                let event_channel = channel.clone();
                let event_version = version.clone();
                let finish_downloaded = downloaded.clone();
                let finish_content_length = content_length.clone();
                move || {
                    let _ = app_handle.emit(
                        "updater://download-progress",
                        UpdaterDownloadProgressEvent {
                            phase: "Finished".to_string(),
                            content_length: Some(finish_content_length.load(Ordering::Relaxed)),
                            chunk_length: None,
                            downloaded: Some(finish_downloaded.load(Ordering::Relaxed)),
                            percent: Some(100.0),
                            channel: event_channel.clone(),
                            version: event_version.clone(),
                        },
                    );
                }
            },
        )
        .await
        .map_err(|e| format!("Failed to download/install update: {e}"))?;

    Ok(format!(
        "Installed update {next_version} from {channel} channel"
    ))
}

#[cfg(test)]
mod tests {
    use super::{ensure_production_identity, PRODUCTION_IDENTIFIER};

    #[test]
    fn updater_accepts_only_the_production_identity() {
        if cfg!(feature = "qualification") {
            assert!(ensure_production_identity(PRODUCTION_IDENTIFIER).is_err());
        } else {
            assert!(ensure_production_identity(PRODUCTION_IDENTIFIER).is_ok());
        }
        assert!(ensure_production_identity("com.ruru.gitru.cef-qualification").is_err());
        assert!(ensure_production_identity("com.ruru.gitru.e2e").is_err());
    }

    #[cfg(feature = "qualification")]
    #[test]
    fn qualification_updater_config_is_valid_and_inert() {
        let root: serde_json::Value = serde_json::from_str(include_str!("../../tauri.conf.json"))
            .expect("base Tauri configuration must be valid JSON");
        let raw_config = root
            .pointer("/plugins/updater")
            .cloned()
            .expect("the updater alpha requires a non-null configuration object");
        let config: tauri_plugin_updater::Config = serde_json::from_value(raw_config)
            .expect("qualification updater configuration must deserialize");

        assert!(config.endpoints.is_empty());
        assert!(config.pubkey.is_empty());
        assert!(!config.dangerous_insecure_transport_protocol);
        assert!(!config.dangerous_accept_invalid_certs);
        assert!(!config.dangerous_accept_invalid_hostnames);
        assert!(config.windows.is_none());
    }
}
