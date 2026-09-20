use serde::Serialize;
use std::sync::{
    atomic::{AtomicBool, AtomicU64, Ordering},
    Arc,
};
use tauri::Emitter;
use tauri_plugin_updater::UpdaterExt;
use url::Url;

const PRODUCTION_UPDATER_BASE_URL: &str = "https://release.gitru.app";
const PRODUCTION_IDENTIFIER: &str = "com.ruru.gitru";
const RELEASE_HOST: &str = "github.com";
const RELEASE_OWNER: &str = "ruru-m07";
const RELEASE_REPOSITORY: &str = "gitru";

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
    Ok(format!(
        "{PRODUCTION_UPDATER_BASE_URL}/{channel}/latest.json"
    ))
}

fn validate_numeric_identifier(identifier: &str, label: &str) -> Result<(), String> {
    if identifier.is_empty() || !identifier.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(format!("Update {label} must contain only ASCII digits"));
    }
    if identifier.len() > 1 && identifier.starts_with('0') {
        return Err(format!("Update {label} must not contain a leading zero"));
    }
    Ok(())
}

fn validate_core_version(version: &str) -> Result<(), String> {
    let components = version.split('.').collect::<Vec<_>>();
    if components.len() != 3 {
        return Err("Update version must contain exactly major.minor.patch".to_string());
    }

    for (component, label) in components.iter().zip(["major", "minor", "patch"]) {
        validate_numeric_identifier(component, label)?;
    }
    Ok(())
}

fn validate_version_for_channel(channel: &str, version: &str) -> Result<(), String> {
    let channel = normalize_channel(channel)?;
    if version.contains('+') {
        return Err("Update versions must not contain build metadata".to_string());
    }

    match channel {
        "stable" => {
            if version.contains('-') {
                return Err("Stable update versions must not contain a prerelease".to_string());
            }
            validate_core_version(version)
        }
        "beta" => {
            let (core, beta_number) = version.split_once("-beta.").ok_or_else(|| {
                "Beta update versions must use the major.minor.patch-beta.N form".to_string()
            })?;
            if beta_number.contains('.') || beta_number.contains('-') {
                return Err("Beta update versions must use one numeric beta identifier".to_string());
            }
            validate_core_version(core)?;
            validate_numeric_identifier(beta_number, "beta number")
        }
        _ => unreachable!("normalize_channel accepts only stable or beta"),
    }
}

fn runtime_manifest_target(target: &str) -> Result<String, String> {
    if target.contains('-') {
        return Ok(target.to_string());
    }

    let architecture = match std::env::consts::ARCH {
        "x86_64" => "x86_64",
        "aarch64" => "aarch64",
        architecture => {
            return Err(format!(
                "Updates are not published for runtime architecture '{architecture}'"
            ));
        }
    };
    let installer = match tauri::utils::platform::bundle_type() {
        Some(tauri::utils::config::BundleType::AppImage) => "appimage",
        Some(tauri::utils::config::BundleType::Deb) => "deb",
        Some(tauri::utils::config::BundleType::Rpm) => "rpm",
        Some(tauri::utils::config::BundleType::App) => "app",
        Some(tauri::utils::config::BundleType::Nsis) => "nsis",
        Some(bundle_type) => {
            return Err(format!(
                "Updates are not published for runtime bundle type '{bundle_type}'"
            ));
        }
        None => return Err("Could not determine the runtime bundle type".to_string()),
    };

    Ok(format!("{target}-{architecture}-{installer}"))
}

fn expected_asset_suffix(target: &str) -> Result<&'static str, String> {
    match target {
        "linux-x86_64" | "linux-x86_64-appimage" => Ok(".AppImage"),
        "linux-x86_64-deb" => Ok(".deb"),
        "linux-x86_64-rpm" => Ok(".rpm"),
        "darwin-aarch64" | "darwin-x86_64" | "darwin-aarch64-app" | "darwin-x86_64-app" => {
            Ok(".app.tar.gz")
        }
        "windows-x86_64" | "windows-x86_64-nsis" => Ok("_x64-setup.exe"),
        _ => Err(format!(
            "Updater target '{target}' is not published by Gitru"
        )),
    }
}

fn validate_download_url(version: &str, target: &str, download_url: &Url) -> Result<(), String> {
    if download_url.scheme() != "https"
        || download_url.host_str() != Some(RELEASE_HOST)
        || !download_url.username().is_empty()
        || download_url.password().is_some()
        || download_url.port().is_some()
        || download_url.query().is_some()
        || download_url.fragment().is_some()
    {
        return Err("Update download URL must use the canonical GitHub release origin".to_string());
    }
    if download_url.as_str().contains('%') {
        return Err("Update download URL must not contain percent-encoding".to_string());
    }

    let segments = download_url
        .path_segments()
        .ok_or_else(|| "Update download URL must contain a release asset path".to_string())?
        .collect::<Vec<_>>();
    let expected_tag = format!("v{version}");
    if segments.len() != 6
        || segments[0] != RELEASE_OWNER
        || segments[1] != RELEASE_REPOSITORY
        || segments[2] != "releases"
        || segments[3] != "download"
        || segments[4] != expected_tag
        || segments[5].is_empty()
    {
        return Err(format!(
            "Update download URL must point to the v{version} Gitru release"
        ));
    }

    let target = runtime_manifest_target(target)?;
    let expected_suffix = expected_asset_suffix(&target)?;
    if !segments[5].ends_with(expected_suffix) {
        return Err(format!(
            "Update asset for target '{target}' must end with '{expected_suffix}'"
        ));
    }
    Ok(())
}

fn validate_raw_manifest_candidate(
    version: &str,
    download_url: &Url,
    raw_manifest: &serde_json::Value,
) -> Result<(), String> {
    let raw_version = raw_manifest
        .get("version")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| "Update manifest must contain a string version".to_string())?;
    if raw_version != version {
        return Err("Update manifest version must use its exact canonical form".to_string());
    }

    let mut matching_raw_urls = Vec::new();
    if let Some(url) = raw_manifest.get("url").and_then(serde_json::Value::as_str) {
        matching_raw_urls.push(url);
    }
    if let Some(platforms) = raw_manifest
        .get("platforms")
        .and_then(serde_json::Value::as_object)
    {
        matching_raw_urls.extend(
            platforms
                .values()
                .filter_map(|platform| platform.get("url").and_then(serde_json::Value::as_str)),
        );
    }

    let mut found_candidate = false;
    for raw_url in matching_raw_urls {
        let parsed = Url::parse(raw_url)
            .map_err(|error| format!("Invalid update URL in raw manifest: {error}"))?;
        if parsed == *download_url {
            found_candidate = true;
            if raw_url != download_url.as_str() {
                return Err(
                    "Update download URL must not rely on URL parser normalization".to_string(),
                );
            }
        }
    }
    if !found_candidate {
        return Err("Selected update URL was not present in the raw manifest".to_string());
    }
    Ok(())
}

fn validate_update_candidate(
    channel: &str,
    version: &str,
    target: &str,
    download_url: &Url,
    raw_manifest: &serde_json::Value,
) -> Result<(), String> {
    validate_version_for_channel(channel, version)?;
    validate_download_url(version, target, download_url)?;
    validate_raw_manifest_candidate(version, download_url, raw_manifest)
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
        validate_update_candidate(
            &channel,
            &update.version,
            &update.target,
            &update.download_url,
            &update.raw_json,
        )?;
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
    validate_update_candidate(
        &channel,
        &update.version,
        &update.target,
        &update.download_url,
        &update.raw_json,
    )?;

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
    use super::{
        endpoint_for_channel, ensure_production_identity, validate_download_url,
        validate_raw_manifest_candidate, validate_version_for_channel, PRODUCTION_IDENTIFIER,
    };
    use serde_json::json;
    use url::Url;

    fn release_url(version: &str, asset: &str) -> Url {
        Url::parse(&format!(
            "https://github.com/ruru-m07/gitru/releases/download/v{version}/{asset}"
        ))
        .expect("test release URL must parse")
    }

    #[test]
    fn updater_endpoints_are_pinned_by_channel() {
        assert_eq!(
            endpoint_for_channel("stable").unwrap(),
            "https://release.gitru.app/stable/latest.json"
        );
        assert_eq!(
            endpoint_for_channel(" BETA ").unwrap(),
            "https://release.gitru.app/beta/latest.json"
        );
        assert!(endpoint_for_channel("../beta").is_err());
        assert!(endpoint_for_channel("nightly").is_err());
    }

    #[test]
    fn updater_versions_are_isolated_by_channel() {
        assert!(validate_version_for_channel("stable", "1.2.3").is_ok());
        assert!(validate_version_for_channel("beta", "1.2.3-beta.0").is_ok());

        for version in [
            "v1.2.3",
            "01.2.3",
            "1.02.3",
            "1.2.03",
            "1.2",
            "1.2.3-beta.1",
            "1.2.3+build.1",
        ] {
            assert!(
                validate_version_for_channel("stable", version).is_err(),
                "stable unexpectedly accepted {version}"
            );
        }
        for version in [
            "1.2.3",
            "1.2.3-beta",
            "1.2.3-beta.01",
            "1.2.3-beta.1.2",
            "1.2.3-alpha.1",
            "1.2.3-beta.1+build.1",
        ] {
            assert!(
                validate_version_for_channel("beta", version).is_err(),
                "beta unexpectedly accepted {version}"
            );
        }
    }

    #[test]
    fn updater_download_urls_match_release_version_target_and_asset() {
        let cases = [
            ("linux-x86_64-appimage", "Gitru_1.2.3-beta.4_amd64.AppImage"),
            ("linux-x86_64", "Gitru_1.2.3-beta.4_amd64.AppImage"),
            ("linux-x86_64-deb", "Gitru_1.2.3-beta.4_amd64.deb"),
            ("linux-x86_64-rpm", "Gitru-1.2.3-beta.4-1.x86_64.rpm"),
            ("darwin-aarch64-app", "Gitru_universal.app.tar.gz"),
            ("darwin-x86_64", "Gitru_universal.app.tar.gz"),
            ("windows-x86_64-nsis", "Gitru_1.2.3-beta.4_x64-setup.exe"),
            ("windows-x86_64", "Gitru_1.2.3-beta.4_x64-setup.exe"),
        ];

        for (target, asset) in cases {
            assert!(
                validate_download_url("1.2.3-beta.4", target, &release_url("1.2.3-beta.4", asset))
                    .is_ok(),
                "valid target {target} was rejected"
            );
        }
    }

    #[test]
    fn updater_download_urls_reject_untrusted_locations_and_shapes() {
        let invalid_urls = [
            "http://github.com/ruru-m07/gitru/releases/download/v1.2.3/Gitru_1.2.3_amd64.AppImage",
            "https://github.example/ruru-m07/gitru/releases/download/v1.2.3/Gitru_1.2.3_amd64.AppImage",
            "https://user@github.com/ruru-m07/gitru/releases/download/v1.2.3/Gitru_1.2.3_amd64.AppImage",
            "https://github.com:444/ruru-m07/gitru/releases/download/v1.2.3/Gitru_1.2.3_amd64.AppImage",
            "https://github.com/ruru-m07/other/releases/download/v1.2.3/Gitru_1.2.3_amd64.AppImage",
            "https://github.com/ruru-m07/gitru/releases/download/v1.2.4/Gitru_1.2.3_amd64.AppImage",
            "https://github.com/ruru-m07/gitru/releases/download/v1.2.3/Gitru_1.2.3_amd64.AppImage?download=1",
            "https://github.com/ruru-m07/gitru/releases/download/v1.2.3/Gitru_1.2.3_amd64.AppImage#fragment",
            "https://github.com/ruru-m07/gitru/releases/download/v1.2.3/%2e%2e%2fasset.AppImage",
        ];
        for raw_url in invalid_urls {
            let url = Url::parse(raw_url).expect("invalid policy URL should still parse");
            assert!(
                validate_download_url("1.2.3", "linux-x86_64-appimage", &url).is_err(),
                "untrusted URL unexpectedly accepted: {raw_url}"
            );
        }

        assert!(validate_download_url(
            "1.2.3",
            "linux-x86_64-appimage",
            &release_url("1.2.3", "Gitru_1.2.3_amd64.deb")
        )
        .is_err());
    }

    #[test]
    fn updater_rejects_raw_manifest_url_normalization() {
        let candidate = release_url("1.2.3", "Gitru_1.2.3_amd64.AppImage");
        let canonical = json!({
            "version": "1.2.3",
            "platforms": {
                "linux-x86_64-appimage": {
                    "url": candidate.as_str(),
                    "signature": "fixture"
                }
            }
        });
        assert!(validate_raw_manifest_candidate("1.2.3", &candidate, &canonical).is_ok());

        let explicit_default_port = json!({
            "version": "1.2.3",
            "platforms": {
                "linux-x86_64-appimage": {
                    "url": "https://github.com:443/ruru-m07/gitru/releases/download/v1.2.3/Gitru_1.2.3_amd64.AppImage",
                    "signature": "fixture"
                }
            }
        });
        assert!(
            validate_raw_manifest_candidate("1.2.3", &candidate, &explicit_default_port).is_err()
        );

        let traversal_path = json!({
            "version": "1.2.3",
            "platforms": {
                "linux-x86_64-appimage": {
                    "url": "https://github.com/ruru-m07/gitru/releases/download/ignored/../v1.2.3/Gitru_1.2.3_amd64.AppImage",
                    "signature": "fixture"
                }
            }
        });
        assert!(validate_raw_manifest_candidate("1.2.3", &candidate, &traversal_path).is_err());

        let noncanonical_version = json!({
            "version": "v1.2.3",
            "platforms": {
                "linux-x86_64-appimage": {
                    "url": candidate.as_str(),
                    "signature": "fixture"
                }
            }
        });
        assert!(
            validate_raw_manifest_candidate("1.2.3", &candidate, &noncanonical_version).is_err()
        );
    }

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
