use serde::Serialize;
use tauri_plugin_updater::UpdaterExt;
use url::Url;

const DEFAULT_UPDATER_BASE_URL: &str = "https://updates.gitru.app";

#[derive(Serialize)]
pub struct UpdateCheckResponse {
    pub available: bool,
    pub channel: String,
    pub current_version: String,
    pub version: Option<String>,
    pub notes: Option<String>,
    pub pub_date: Option<String>,
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

#[tauri::command]
pub async fn check_for_update_by_channel(
    app: tauri::AppHandle,
    channel: String,
) -> Result<UpdateCheckResponse, String> {
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
pub async fn install_update_by_channel(
    app: tauri::AppHandle,
    channel: String,
) -> Result<String, String> {
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
    update
        .download_and_install(|_, _| {}, || {})
        .await
        .map_err(|e| format!("Failed to download/install update: {e}"))?;

    Ok(format!(
        "Installed update {next_version} from {channel} channel"
    ))
}
