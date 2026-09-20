use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use minisign_verify::{PublicKey, Signature};
use reqwest::blocking::Client;
use semver::Version;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, File},
    io::{Read, Write},
    path::{Path, PathBuf},
    str::FromStr,
    time::Duration,
};
use time::{OffsetDateTime, format_description::well_known::Rfc3339};
use url::Url;

pub const PRODUCTION_IDENTIFIER: &str = "com.ruru.gitru";

const REQUIRED_TARGETS: [&str; 6] = [
    "linux-x86_64-appimage",
    "linux-x86_64-deb",
    "linux-x86_64-rpm",
    "darwin-aarch64-app",
    "darwin-x86_64-app",
    "windows-x86_64-nsis",
];

const OPTIONAL_ALIASES: [(&str, &str); 4] = [
    ("linux-x86_64", "linux-x86_64-appimage"),
    ("darwin-aarch64", "darwin-aarch64-app"),
    ("darwin-x86_64", "darwin-x86_64-app"),
    ("windows-x86_64", "windows-x86_64-nsis"),
];

pub type Result<T> = std::result::Result<T, String>;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Channel {
    Stable,
    Beta,
}

impl Channel {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Stable => "stable",
            Self::Beta => "beta",
        }
    }

    pub fn accepts(self, version: &Version) -> bool {
        if !version.build.is_empty() {
            return false;
        }

        match self {
            Self::Stable => version.pre.is_empty(),
            Self::Beta => {
                let prerelease = version.pre.as_str();
                let Some(sequence) = prerelease.strip_prefix("beta.") else {
                    return false;
                };
                !sequence.is_empty()
                    && sequence.bytes().all(|byte| byte.is_ascii_digit())
                    && (sequence == "0" || !sequence.starts_with('0'))
            }
        }
    }
}

impl FromStr for Channel {
    type Err = String;

    fn from_str(value: &str) -> Result<Self> {
        match value {
            "stable" => Ok(Self::Stable),
            "beta" => Ok(Self::Beta),
            _ => Err(format!(
                "invalid updater channel {value:?}; expected stable or beta"
            )),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PlatformEntry {
    pub url: String,
    pub signature: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub version: String,
    #[serde(default)]
    pub notes: Option<String>,
    pub pub_date: String,
    pub platforms: BTreeMap<String, PlatformEntry>,
}

#[derive(Clone, Debug)]
pub struct ManifestPolicy<'a> {
    pub repository: &'a str,
    pub channel: Channel,
}

#[derive(Clone, Debug)]
pub struct ValidatedArtifact {
    pub url: Url,
    pub signature: String,
    pub asset_name: String,
    pub targets: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct ValidatedManifest {
    pub manifest: Manifest,
    pub version: Version,
    pub channel: Channel,
    pub artifacts: Vec<ValidatedArtifact>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ArtifactReport {
    pub url: String,
    pub targets: Vec<String>,
    pub bytes: u64,
    pub sha256: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct ValidationReport {
    pub version: String,
    pub channel: Channel,
    pub manifest_sha256: String,
    pub artifacts: Vec<ArtifactReport>,
}

#[derive(Clone, Debug, Serialize)]
pub struct StopPlanState {
    pub current_manifest_valid: bool,
    pub current_manifest_sha256: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub warning: Option<String>,
}

pub fn parse_manifest(bytes: &[u8]) -> Result<Manifest> {
    serde_json::from_slice(bytes).map_err(|error| format!("invalid updater manifest JSON: {error}"))
}

pub fn validate_manifest(
    manifest: Manifest,
    policy: &ManifestPolicy<'_>,
) -> Result<ValidatedManifest> {
    validate_repository(policy.repository)?;
    let version = Version::parse(&manifest.version)
        .map_err(|error| format!("manifest version is not strict SemVer: {error}"))?;
    if version.to_string() != manifest.version {
        return Err(format!(
            "manifest version must use canonical SemVer without a leading v: {}",
            manifest.version
        ));
    }
    if !policy.channel.accepts(&version) {
        return Err(format!(
            "manifest version {} does not belong to the {} channel",
            version,
            policy.channel.as_str()
        ));
    }

    OffsetDateTime::parse(&manifest.pub_date, &Rfc3339)
        .map_err(|error| format!("manifest pub_date is not RFC 3339: {error}"))?;

    let allowed_targets: BTreeSet<&str> = REQUIRED_TARGETS
        .iter()
        .copied()
        .chain(OPTIONAL_ALIASES.iter().map(|(alias, _)| *alias))
        .collect();

    for required in REQUIRED_TARGETS {
        if !manifest.platforms.contains_key(required) {
            return Err(format!("manifest is missing required target {required}"));
        }
    }

    for target in manifest.platforms.keys() {
        if !allowed_targets.contains(target.as_str()) {
            return Err(format!("manifest contains unsupported target {target}"));
        }
    }

    for (alias, canonical) in OPTIONAL_ALIASES {
        if let Some(alias_entry) = manifest.platforms.get(alias) {
            let canonical_entry = manifest
                .platforms
                .get(canonical)
                .expect("required canonical target checked above");
            if alias_entry != canonical_entry {
                return Err(format!(
                    "target alias {alias} must exactly match {canonical}"
                ));
            }
        }
    }

    let mut unique = BTreeMap::<String, (String, String, Vec<String>)>::new();
    for (target, entry) in &manifest.platforms {
        decode_signature(&entry.signature)
            .map_err(|error| format!("invalid signature for target {target}: {error}"))?;
        let (url, asset_name) = validate_artifact_url(policy.repository, &version, &entry.url)
            .map_err(|error| format!("invalid URL for target {target}: {error}"))?;
        validate_asset_type(target, &asset_name)?;

        match unique.get_mut(url.as_str()) {
            Some((existing_signature, _, targets)) => {
                if existing_signature != &entry.signature {
                    return Err(format!(
                        "artifact {} is referenced with different signatures",
                        url
                    ));
                }
                targets.push(target.clone());
            }
            None => {
                unique.insert(
                    url.to_string(),
                    (entry.signature.clone(), asset_name, vec![target.clone()]),
                );
            }
        }
    }

    let artifacts = unique
        .into_iter()
        .map(|(url, (signature, asset_name, targets))| {
            Ok(ValidatedArtifact {
                url: Url::parse(&url).map_err(|error| error.to_string())?,
                signature,
                asset_name,
                targets,
            })
        })
        .collect::<Result<Vec<_>>>()?;

    Ok(ValidatedManifest {
        manifest,
        version,
        channel: policy.channel,
        artifacts,
    })
}

pub fn validate_release_metadata(
    candidate: &ValidatedManifest,
    tag: &str,
    package_version: &str,
    historical_versions: &[String],
    revoked_versions: &[String],
) -> Result<()> {
    let expected_tag = format!("v{}", candidate.version);
    if tag != expected_tag {
        return Err(format!(
            "release tag {tag:?} must exactly equal {expected_tag:?}"
        ));
    }
    if package_version != candidate.version.to_string() {
        return Err(format!(
            "desktop package version {package_version:?} does not match manifest version {}",
            candidate.version
        ));
    }

    validate_high_water(&candidate.version, candidate.channel, historical_versions)?;
    validate_revocations(&candidate.version, candidate.channel, revoked_versions)
}

pub fn current_pointer_high_water(
    candidate_bytes: &[u8],
    current_bytes: &[u8],
    policy: &ManifestPolicy<'_>,
) -> Result<Option<String>> {
    if candidate_bytes == current_bytes {
        return Ok(None);
    }

    let current = validate_manifest(parse_manifest(current_bytes)?, policy)
        .map_err(|error| format!("current channel pointer is invalid: {error}"))?;
    Ok(Some(current.version.to_string()))
}

pub fn validate_event_metadata(tag: &str, package_version: &str, channel: Channel) -> Result<()> {
    let version = Version::parse(package_version)
        .map_err(|error| format!("desktop package version is not strict SemVer: {error}"))?;
    if version.to_string() != package_version {
        return Err("desktop package version is not canonical SemVer".to_string());
    }
    if !channel.accepts(&version) {
        return Err(format!(
            "desktop package version {version} does not belong to the {} channel",
            channel.as_str()
        ));
    }
    let expected_tag = format!("v{version}");
    if tag != expected_tag {
        return Err(format!(
            "release tag {tag:?} must exactly equal {expected_tag:?}"
        ));
    }
    Ok(())
}

pub fn validate_high_water(
    candidate: &Version,
    channel: Channel,
    historical_versions: &[String],
) -> Result<()> {
    for raw in historical_versions {
        let historical = Version::parse(raw)
            .map_err(|error| format!("invalid historical version {raw:?}: {error}"))?;
        if !channel.accepts(&historical) {
            return Err(format!(
                "historical version {historical} does not belong to the {} channel",
                channel.as_str()
            ));
        }
        if candidate <= &historical {
            return Err(format!(
                "candidate {candidate} must be newer than historical high-water version {historical}"
            ));
        }
    }
    Ok(())
}

pub fn validate_revocations(
    candidate: &Version,
    channel: Channel,
    revoked_versions: &[String],
) -> Result<()> {
    for raw in revoked_versions {
        let revoked = Version::parse(raw)
            .map_err(|error| format!("invalid revoked version {raw:?}: {error}"))?;
        if revoked.to_string() != *raw {
            return Err(format!("revoked version {raw:?} must use canonical SemVer"));
        }
        if !channel.accepts(&revoked) {
            return Err(format!(
                "revoked version {revoked} does not belong to the {} channel",
                channel.as_str()
            ));
        }
        if candidate == &revoked {
            return Err(format!(
                "candidate {candidate} is permanently blocked by its rollout revocation marker"
            ));
        }
    }
    Ok(())
}

pub fn verify_manifest_sidecar_or_legacy(
    manifest_bytes: &[u8],
    signature: Option<&str>,
    public_key: &str,
    allow_legacy_without_sidecar: bool,
) -> Result<bool> {
    match signature {
        Some(signature) => {
            verify_embedded_signature(manifest_bytes, signature, public_key)
                .map_err(|error| format!("fallback manifest sidecar failed verification: {error}"))?;
            Ok(true)
        }
        None if allow_legacy_without_sidecar => Ok(false),
        None => Err(
            "fallback manifest has no signature sidecar; explicitly allow a legacy manifest only after verifying every embedded artifact signature"
                .to_string(),
        ),
    }
}

pub fn validate_stop_plan(
    current: &ValidatedManifest,
    fallback: &ValidatedManifest,
    expected_current: &str,
    expected_fallback: &str,
) -> Result<()> {
    if current.channel != fallback.channel {
        return Err("current and fallback manifests belong to different channels".to_string());
    }
    if current.version.to_string() != expected_current {
        return Err(format!(
            "current feed is {}, not the expected compare-and-swap version {expected_current}",
            current.version
        ));
    }
    if fallback.version.to_string() != expected_fallback {
        return Err(format!(
            "fallback manifest is {}, not requested version {expected_fallback}",
            fallback.version
        ));
    }
    if fallback.version >= current.version {
        return Err(format!(
            "stop fallback {} must be older than current feed {}",
            fallback.version, current.version
        ));
    }
    Ok(())
}

pub fn validate_stop_plan_with_current_bytes(
    current_bytes: &[u8],
    fallback: &ValidatedManifest,
    policy: &ManifestPolicy<'_>,
    expected_current: &str,
    expected_fallback: &str,
    confirmation: &str,
) -> Result<StopPlanState> {
    let asserted_current = Version::parse(expected_current)
        .map_err(|error| format!("expected current version is not strict SemVer: {error}"))?;
    if asserted_current.to_string() != expected_current {
        return Err("expected current version must use canonical SemVer".to_string());
    }
    if !policy.channel.accepts(&asserted_current) {
        return Err(format!(
            "expected current version {asserted_current} does not belong to the {} channel",
            policy.channel.as_str()
        ));
    }
    if fallback.channel != policy.channel {
        return Err("fallback manifest belongs to the wrong channel".to_string());
    }
    if fallback.version.to_string() != expected_fallback {
        return Err(format!(
            "fallback manifest is {}, not requested version {expected_fallback}",
            fallback.version
        ));
    }
    if fallback.version >= asserted_current {
        return Err(format!(
            "stop fallback {} must be older than asserted current feed {}",
            fallback.version, asserted_current
        ));
    }

    let current_manifest_sha256 = digest_hex(Sha256::digest(current_bytes).as_slice());
    match parse_manifest(current_bytes).and_then(|manifest| validate_manifest(manifest, policy)) {
        Ok(current) => {
            if confirmation != "STOP_ROLLOUT" {
                return Err(
                    "a valid current pointer requires confirmation STOP_ROLLOUT".to_string()
                );
            }
            if current.version != asserted_current {
                return Err(format!(
                    "current feed is {}, not the expected compare-and-swap version {expected_current}",
                    current.version
                ));
            }
            Ok(StopPlanState {
                current_manifest_valid: true,
                current_manifest_sha256,
                warning: None,
            })
        }
        Err(error) => {
            if let Ok(value) = serde_json::from_slice::<serde_json::Value>(current_bytes)
                && let Some(raw) = value.get("version").and_then(serde_json::Value::as_str)
                && let Ok(declared) = Version::parse(raw)
                && declared.to_string() == raw
                && declared != asserted_current
            {
                return Err(format!(
                    "corrupt current pointer declares version {declared}, not asserted version {asserted_current}"
                ));
            }
            if confirmation != "STOP_CORRUPT_ROLLOUT" {
                return Err(format!(
                    "current pointer failed strict validation ({error}); rerun with confirmation STOP_CORRUPT_ROLLOUT to replace these exact ETag-guarded bytes"
                ));
            }
            Ok(StopPlanState {
                current_manifest_valid: false,
                current_manifest_sha256,
                warning: Some(format!(
                    "current pointer failed strict validation and was replaced under explicit corrupt-rollout confirmation: {error}"
                )),
            })
        }
    }
}

pub fn production_public_key(config_bytes: &[u8]) -> Result<String> {
    let value: serde_json::Value = serde_json::from_slice(config_bytes)
        .map_err(|error| format!("invalid Tauri configuration JSON: {error}"))?;
    let identifier = value
        .get("identifier")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| "Tauri configuration is missing identifier".to_string())?;
    if identifier != PRODUCTION_IDENTIFIER {
        return Err(format!(
            "refusing updater publication for non-production identifier {identifier:?}"
        ));
    }
    if value
        .pointer("/bundle/createUpdaterArtifacts")
        .and_then(serde_json::Value::as_bool)
        != Some(true)
    {
        return Err(
            "production configuration must enable bundle.createUpdaterArtifacts".to_string(),
        );
    }
    let public_key = value
        .pointer("/plugins/updater/pubkey")
        .and_then(serde_json::Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "production updater public key is empty".to_string())?;
    decode_public_key(public_key)?;
    Ok(public_key.to_string())
}

pub fn package_version(package_bytes: &[u8]) -> Result<String> {
    let value: serde_json::Value = serde_json::from_slice(package_bytes)
        .map_err(|error| format!("invalid package JSON: {error}"))?;
    let raw = value
        .get("version")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| "package JSON is missing a string version".to_string())?;
    let parsed = Version::parse(raw)
        .map_err(|error| format!("package version is not strict SemVer: {error}"))?;
    if parsed.to_string() != raw {
        return Err("package version is not canonical SemVer".to_string());
    }
    Ok(raw.to_string())
}

pub fn verify_embedded_signature(
    bytes: &[u8],
    signature_base64: &str,
    public_key_base64: &str,
) -> Result<()> {
    let public_key = decode_public_key(public_key_base64)?;
    let signature = decode_signature(signature_base64)?;
    public_key
        .verify(bytes, &signature, true)
        .map_err(|error| format!("Minisign verification failed: {error}"))
}

pub fn verify_and_download(
    manifest_bytes: &[u8],
    validated: &ValidatedManifest,
    public_key_base64: &str,
    output_directory: &Path,
) -> Result<ValidationReport> {
    fs::create_dir_all(output_directory).map_err(|error| {
        format!(
            "failed to create artifact directory {}: {error}",
            output_directory.display()
        )
    })?;
    let public_key = decode_public_key(public_key_base64)?;
    let _ = rustls::crypto::ring::default_provider().install_default();
    let client = Client::builder()
        .https_only(true)
        .connect_timeout(Duration::from_secs(30))
        .timeout(Duration::from_secs(20 * 60))
        .user_agent("gitru-updater-release-validator")
        .build()
        .map_err(|error| format!("failed to create HTTPS client: {error}"))?;

    let mut reports = Vec::with_capacity(validated.artifacts.len());
    for artifact in &validated.artifacts {
        let signature = decode_signature(&artifact.signature)?;
        let mut verifier = public_key
            .verify_stream(&signature)
            .map_err(|error| format!("unsupported signature for {}: {error}", artifact.url))?;
        let mut response = client
            .get(artifact.url.clone())
            .send()
            .and_then(reqwest::blocking::Response::error_for_status)
            .map_err(|error| format!("failed to download {}: {error}", artifact.url))?;
        if response.url().scheme() != "https" {
            return Err(format!("artifact redirect left HTTPS: {}", response.url()));
        }

        let output_path = safe_output_path(output_directory, &artifact.asset_name)?;
        let partial_path = output_path.with_extension(format!(
            "{}.partial",
            output_path
                .extension()
                .and_then(|value| value.to_str())
                .unwrap_or("download")
        ));
        let mut file = File::create(&partial_path)
            .map_err(|error| format!("failed to create {}: {error}", partial_path.display()))?;
        let mut sha256 = Sha256::new();
        let mut bytes_written = 0_u64;
        let mut buffer = [0_u8; 64 * 1024];
        loop {
            let count = response
                .read(&mut buffer)
                .map_err(|error| format!("failed while downloading {}: {error}", artifact.url))?;
            if count == 0 {
                break;
            }
            file.write_all(&buffer[..count]).map_err(|error| {
                format!("failed while writing {}: {error}", partial_path.display())
            })?;
            sha256.update(&buffer[..count]);
            verifier.update(&buffer[..count]);
            bytes_written = bytes_written.saturating_add(count as u64);
        }
        file.sync_all()
            .map_err(|error| format!("failed to sync {}: {error}", partial_path.display()))?;
        drop(file);
        verifier.finalize().map_err(|error| {
            let _ = fs::remove_file(&partial_path);
            format!(
                "signature verification failed for {}: {error}",
                artifact.url
            )
        })?;
        fs::rename(&partial_path, &output_path).map_err(|error| {
            format!(
                "failed to finalize downloaded artifact {}: {error}",
                output_path.display()
            )
        })?;

        reports.push(ArtifactReport {
            url: artifact.url.to_string(),
            targets: artifact.targets.clone(),
            bytes: bytes_written,
            sha256: digest_hex(sha256.finalize().as_slice()),
        });
    }

    Ok(ValidationReport {
        version: validated.version.to_string(),
        channel: validated.channel,
        manifest_sha256: digest_hex(Sha256::digest(manifest_bytes).as_slice()),
        artifacts: reports,
    })
}

fn validate_repository(repository: &str) -> Result<()> {
    let mut parts = repository.split('/');
    let Some(owner) = parts.next() else {
        return Err("repository must be owner/name".to_string());
    };
    let Some(name) = parts.next() else {
        return Err("repository must be owner/name".to_string());
    };
    if parts.next().is_some()
        || !valid_slug(owner)
        || !valid_slug(name)
        || owner.is_empty()
        || name.is_empty()
    {
        return Err("repository must be a safe owner/name slug".to_string());
    }
    Ok(())
}

fn valid_slug(value: &str) -> bool {
    !value.is_empty()
        && value != "."
        && value != ".."
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
}

fn validate_artifact_url(repository: &str, version: &Version, raw: &str) -> Result<(Url, String)> {
    if !raw.starts_with("https://github.com/") {
        return Err("artifact URL must use the exact https://github.com origin".to_string());
    }
    if raw.contains('%') || raw.contains('\\') {
        return Err("artifact URL must not contain encoded or backslash path segments".to_string());
    }
    let url = Url::parse(raw).map_err(|error| format!("URL parse failed: {error}"))?;
    if url.scheme() != "https"
        || url.host_str() != Some("github.com")
        || url.port().is_some()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err("artifact URL contains a forbidden origin or component".to_string());
    }
    let segments = url
        .path_segments()
        .ok_or_else(|| "artifact URL has no path".to_string())?
        .collect::<Vec<_>>();
    let repository_parts = repository.split('/').collect::<Vec<_>>();
    if segments.len() != 6
        || segments[0] != repository_parts[0]
        || segments[1] != repository_parts[1]
        || segments[2] != "releases"
        || segments[3] != "download"
        || segments[4] != format!("v{version}")
    {
        return Err(format!(
            "artifact URL must point to {repository} release tag v{version}"
        ));
    }
    let asset_name = segments[5].to_string();
    if asset_name.is_empty() || !valid_slug(&asset_name) {
        return Err("artifact asset name contains unsafe characters".to_string());
    }
    let expected =
        format!("https://github.com/{repository}/releases/download/v{version}/{asset_name}");
    if raw != expected {
        return Err("artifact URL is not in canonical form".to_string());
    }
    Ok((url, asset_name))
}

fn validate_asset_type(target: &str, asset_name: &str) -> Result<()> {
    let valid = match target {
        "linux-x86_64" | "linux-x86_64-appimage" => asset_name.ends_with(".AppImage"),
        "linux-x86_64-deb" => asset_name.ends_with(".deb"),
        "linux-x86_64-rpm" => asset_name.ends_with(".rpm"),
        "darwin-aarch64" | "darwin-x86_64" | "darwin-aarch64-app" | "darwin-x86_64-app" => {
            asset_name.ends_with(".app.tar.gz")
        }
        "windows-x86_64" | "windows-x86_64-nsis" => asset_name.ends_with("_x64-setup.exe"),
        _ => false,
    };
    if !valid {
        return Err(format!(
            "artifact {asset_name:?} has the wrong type for target {target}"
        ));
    }
    Ok(())
}

fn decode_public_key(value: &str) -> Result<PublicKey> {
    let decoded = decode_canonical_base64(value, "public key")?;
    let text = std::str::from_utf8(&decoded)
        .map_err(|_| "decoded updater public key is not UTF-8".to_string())?;
    PublicKey::decode(text).map_err(|error| format!("invalid updater public key: {error}"))
}

fn decode_signature(value: &str) -> Result<Signature> {
    let decoded = decode_canonical_base64(value, "signature")?;
    let text = std::str::from_utf8(&decoded)
        .map_err(|_| "decoded updater signature is not UTF-8".to_string())?;
    Signature::decode(text).map_err(|error| format!("invalid updater signature: {error}"))
}

fn decode_canonical_base64(value: &str, label: &str) -> Result<Vec<u8>> {
    if value.is_empty() || value.trim() != value {
        return Err(format!("{label} must be non-empty canonical base64"));
    }
    let decoded = BASE64
        .decode(value)
        .map_err(|error| format!("{label} is not base64: {error}"))?;
    if BASE64.encode(&decoded) != value {
        return Err(format!("{label} is not canonical base64"));
    }
    Ok(decoded)
}

fn safe_output_path(directory: &Path, asset_name: &str) -> Result<PathBuf> {
    if !valid_slug(asset_name) {
        return Err("unsafe artifact output filename".to_string());
    }
    Ok(directory.join(asset_name))
}

fn digest_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const REPOSITORY: &str = "ruru-m07/gitru";
    const PUBLIC_KEY: &str = include_str!("../tests/fixtures/test-only.pub");
    const WRONG_PUBLIC_KEY: &str = include_str!("../tests/fixtures/wrong-test-only.pub");
    const SIGNATURE: &str = include_str!("../tests/fixtures/artifact.bin.sig");
    const OTHER_SIGNATURE: &str = include_str!("../tests/fixtures/other.bin.sig");
    const ARTIFACT: &[u8] = include_bytes!("../tests/fixtures/artifact.bin");

    fn entry(asset: &str) -> PlatformEntry {
        PlatformEntry {
            url: format!("https://github.com/{REPOSITORY}/releases/download/v1.2.3/{asset}"),
            signature: SIGNATURE.trim().to_string(),
        }
    }

    fn manifest(version: &str) -> Manifest {
        let mut platforms = BTreeMap::new();
        platforms.insert(
            "linux-x86_64-appimage".to_string(),
            entry("Gitru_1.2.3_amd64.AppImage"),
        );
        platforms.insert(
            "linux-x86_64-deb".to_string(),
            entry("Gitru_1.2.3_amd64.deb"),
        );
        platforms.insert(
            "linux-x86_64-rpm".to_string(),
            entry("Gitru-1.2.3-1.x86_64.rpm"),
        );
        platforms.insert(
            "darwin-aarch64-app".to_string(),
            entry("Gitru_universal.app.tar.gz"),
        );
        platforms.insert(
            "darwin-x86_64-app".to_string(),
            entry("Gitru_universal.app.tar.gz"),
        );
        platforms.insert(
            "windows-x86_64-nsis".to_string(),
            entry("Gitru_1.2.3_x64-setup.exe"),
        );
        Manifest {
            version: version.to_string(),
            notes: Some(String::new()),
            pub_date: "2026-09-20T12:00:00Z".to_string(),
            platforms,
        }
    }

    fn stable_policy() -> ManifestPolicy<'static> {
        ManifestPolicy {
            repository: REPOSITORY,
            channel: Channel::Stable,
        }
    }

    #[test]
    fn validates_complete_stable_manifest_and_deduplicates_assets() {
        let validated = validate_manifest(manifest("1.2.3"), &stable_policy()).unwrap();
        assert_eq!(validated.artifacts.len(), 5);
        assert_eq!(validated.version, Version::new(1, 2, 3));
    }

    #[test]
    fn validates_beta_channel_and_rejects_cross_channel_versions() {
        let mut beta = manifest("1.2.3-beta.4");
        for entry in beta.platforms.values_mut() {
            entry.url = entry.url.replace("v1.2.3/", "v1.2.3-beta.4/");
        }
        let policy = ManifestPolicy {
            repository: REPOSITORY,
            channel: Channel::Beta,
        };
        assert!(validate_manifest(beta.clone(), &policy).is_ok());
        assert!(validate_manifest(beta, &stable_policy()).is_err());
        assert!(validate_manifest(manifest("1.2.3"), &policy).is_err());
    }

    #[test]
    fn rejects_build_metadata_and_non_beta_prereleases() {
        assert!(validate_manifest(manifest("1.2.3+rebuilt"), &stable_policy()).is_err());
        let policy = ManifestPolicy {
            repository: REPOSITORY,
            channel: Channel::Beta,
        };
        assert!(
            !policy
                .channel
                .accepts(&Version::parse("1.2.3-rc.1").unwrap())
        );
        assert!(Version::parse("1.2.3-beta.01").is_err());
    }

    #[test]
    fn rejects_missing_unknown_and_mismatched_alias_targets() {
        let mut missing = manifest("1.2.3");
        missing.platforms.remove("linux-x86_64-deb");
        assert!(validate_manifest(missing, &stable_policy()).is_err());

        let mut unknown = manifest("1.2.3");
        unknown
            .platforms
            .insert("plan9-amd64".to_string(), entry("Gitru.zip"));
        assert!(validate_manifest(unknown, &stable_policy()).is_err());

        let mut alias = manifest("1.2.3");
        alias.platforms.insert(
            "linux-x86_64".to_string(),
            entry("Gitru_other_amd64.AppImage"),
        );
        assert!(validate_manifest(alias, &stable_policy()).is_err());
    }

    #[test]
    fn rejects_untrusted_or_noncanonical_urls() {
        for bad_url in [
            "http://github.com/ruru-m07/gitru/releases/download/v1.2.3/Gitru_1.2.3_amd64.deb",
            "https://evil.example/ruru-m07/gitru/releases/download/v1.2.3/Gitru_1.2.3_amd64.deb",
            "https://user@github.com/ruru-m07/gitru/releases/download/v1.2.3/Gitru_1.2.3_amd64.deb",
            "https://github.com:444/ruru-m07/gitru/releases/download/v1.2.3/Gitru_1.2.3_amd64.deb",
            "https://github.com/ruru-m07/gitru/releases/download/v9.9.9/Gitru_1.2.3_amd64.deb",
            "https://github.com/ruru-m07/other/releases/download/v1.2.3/Gitru_1.2.3_amd64.deb",
            "https://github.com/ruru-m07/gitru/releases/download/v1.2.3/Gitru_1.2.3_amd64.deb?x=1",
            "https://github.com/ruru-m07/gitru/releases/download/v1.2.3/Gitru_1.2.3_amd64.deb#x",
            "https://github.com/ruru-m07/gitru/releases/download/v1.2.3/%2e%2e",
        ] {
            let mut candidate = manifest("1.2.3");
            candidate.platforms.get_mut("linux-x86_64-deb").unwrap().url = bad_url.to_string();
            assert!(
                validate_manifest(candidate, &stable_policy()).is_err(),
                "accepted {bad_url}"
            );
        }
    }

    #[test]
    fn rejects_target_extension_and_signature_conflicts() {
        let mut extension = manifest("1.2.3");
        extension.platforms.get_mut("linux-x86_64-deb").unwrap().url =
            "https://github.com/ruru-m07/gitru/releases/download/v1.2.3/Gitru.exe".to_string();
        assert!(validate_manifest(extension, &stable_policy()).is_err());

        let mut wrong_windows_architecture = manifest("1.2.3");
        wrong_windows_architecture
            .platforms
            .get_mut("windows-x86_64-nsis")
            .unwrap()
            .url =
            "https://github.com/ruru-m07/gitru/releases/download/v1.2.3/Gitru_arm64-setup.exe"
                .to_string();
        assert!(validate_manifest(wrong_windows_architecture, &stable_policy()).is_err());

        let mut conflict = manifest("1.2.3");
        conflict.platforms.insert(
            "darwin-aarch64".to_string(),
            PlatformEntry {
                url: conflict.platforms["darwin-aarch64-app"].url.clone(),
                signature: OTHER_SIGNATURE.trim().to_string(),
            },
        );
        assert!(validate_manifest(conflict, &stable_policy()).is_err());
    }

    #[test]
    fn release_metadata_enforces_exact_tag_package_and_high_water() {
        let validated = validate_manifest(manifest("1.2.3"), &stable_policy()).unwrap();
        assert!(
            validate_release_metadata(&validated, "v1.2.3", "1.2.3", &["1.2.2".to_string()], &[])
                .is_ok()
        );
        assert!(validate_release_metadata(&validated, "1.2.3", "1.2.3", &[], &[]).is_err());
        assert!(validate_release_metadata(&validated, "v1.2.3", "1.2.2", &[], &[]).is_err());
        assert!(
            validate_release_metadata(&validated, "v1.2.3", "1.2.3", &["1.2.3".to_string()], &[])
                .is_err()
        );
        assert!(
            validate_release_metadata(&validated, "v1.2.3", "1.2.3", &["1.2.4".to_string()], &[])
                .is_err()
        );
    }

    #[test]
    fn current_pointer_joins_high_water_unless_bytes_match_candidate() {
        let candidate = serde_json::to_vec(&manifest("1.2.3")).unwrap();
        assert_eq!(
            current_pointer_high_water(&candidate, &candidate, &stable_policy()).unwrap(),
            None
        );

        let mut higher = manifest("1.2.4");
        for entry in higher.platforms.values_mut() {
            entry.url = entry.url.replace("v1.2.3/", "v1.2.4/");
        }
        let higher = serde_json::to_vec(&higher).unwrap();
        assert_eq!(
            current_pointer_high_water(&candidate, &higher, &stable_policy()).unwrap(),
            Some("1.2.4".to_string())
        );
        assert!(
            current_pointer_high_water(&candidate, br#"{"version":"9.9.9"}"#, &stable_policy())
                .is_err()
        );
    }

    #[test]
    fn revoked_release_cannot_be_repromoted_but_forward_recovery_can() {
        let revoked = vec!["1.2.3".to_string()];
        assert!(
            validate_revocations(&Version::parse("1.2.3").unwrap(), Channel::Stable, &revoked)
                .is_err()
        );
        assert!(
            validate_revocations(&Version::parse("1.2.4").unwrap(), Channel::Stable, &revoked)
                .is_ok()
        );
        assert!(
            validate_revocations(
                &Version::parse("1.2.4").unwrap(),
                Channel::Stable,
                &["1.2.3-beta.1".to_string()]
            )
            .is_err()
        );
    }

    #[test]
    fn legacy_manifest_requires_explicit_opt_in_and_bad_sidecars_still_fail() {
        assert!(
            verify_manifest_sidecar_or_legacy(ARTIFACT, None, PUBLIC_KEY.trim(), false).is_err()
        );
        assert!(
            !verify_manifest_sidecar_or_legacy(ARTIFACT, None, PUBLIC_KEY.trim(), true).unwrap()
        );
        assert!(
            verify_manifest_sidecar_or_legacy(
                ARTIFACT,
                Some(SIGNATURE.trim()),
                PUBLIC_KEY.trim(),
                false
            )
            .unwrap()
        );
        assert!(
            verify_manifest_sidecar_or_legacy(
                ARTIFACT,
                Some(OTHER_SIGNATURE.trim()),
                PUBLIC_KEY.trim(),
                true
            )
            .is_err()
        );
    }

    #[test]
    fn release_event_enforces_channel_and_exact_tag() {
        assert!(validate_event_metadata("v1.2.3", "1.2.3", Channel::Stable).is_ok());
        assert!(validate_event_metadata("v1.2.3-beta.4", "1.2.3-beta.4", Channel::Beta).is_ok());
        assert!(validate_event_metadata("v1.2.3", "1.2.3", Channel::Beta).is_err());
        assert!(validate_event_metadata("1.2.3", "1.2.3", Channel::Stable).is_err());
    }

    #[test]
    fn forward_recovery_must_exceed_the_bad_historical_version() {
        let bad_version = vec!["1.2.3".to_string()];
        assert!(
            validate_high_water(
                &Version::parse("1.2.4").unwrap(),
                Channel::Stable,
                &bad_version
            )
            .is_ok()
        );
        assert!(
            validate_high_water(
                &Version::parse("1.2.2").unwrap(),
                Channel::Stable,
                &bad_version
            )
            .is_err()
        );
    }

    #[test]
    fn verifies_valid_bytes_and_rejects_tampering_wrong_keys_and_swapped_signatures() {
        assert!(verify_embedded_signature(ARTIFACT, SIGNATURE.trim(), PUBLIC_KEY.trim()).is_ok());

        let mut tampered = ARTIFACT.to_vec();
        tampered[0] ^= 1;
        assert!(verify_embedded_signature(&tampered, SIGNATURE.trim(), PUBLIC_KEY.trim()).is_err());
        assert!(
            verify_embedded_signature(ARTIFACT, SIGNATURE.trim(), WRONG_PUBLIC_KEY.trim()).is_err()
        );
        assert!(
            verify_embedded_signature(ARTIFACT, OTHER_SIGNATURE.trim(), PUBLIC_KEY.trim()).is_err()
        );
    }

    #[test]
    fn production_config_requires_identity_artifacts_and_a_valid_public_key() {
        let valid = serde_json::json!({
            "identifier": PRODUCTION_IDENTIFIER,
            "bundle": { "createUpdaterArtifacts": true },
            "plugins": { "updater": { "pubkey": PUBLIC_KEY.trim() } }
        });
        assert_eq!(
            production_public_key(valid.to_string().as_bytes()).unwrap(),
            PUBLIC_KEY.trim()
        );

        let mut invalid_identity = valid.clone();
        invalid_identity["identifier"] = serde_json::json!("com.ruru.gitru.qualification");
        assert!(production_public_key(invalid_identity.to_string().as_bytes()).is_err());

        let mut missing_artifacts = valid.clone();
        missing_artifacts["bundle"]["createUpdaterArtifacts"] = serde_json::json!(false);
        assert!(production_public_key(missing_artifacts.to_string().as_bytes()).is_err());

        let mut invalid_key = valid;
        invalid_key["plugins"]["updater"]["pubkey"] = serde_json::json!("not-base64");
        assert!(production_public_key(invalid_key.to_string().as_bytes()).is_err());
    }

    #[test]
    fn stop_plan_requires_exact_current_and_an_older_fallback() {
        let current = validate_manifest(manifest("1.2.3"), &stable_policy()).unwrap();
        let mut older_manifest = manifest("1.2.2");
        for entry in older_manifest.platforms.values_mut() {
            entry.url = entry.url.replace("v1.2.3/", "v1.2.2/");
        }
        let older = validate_manifest(older_manifest, &stable_policy()).unwrap();
        assert!(validate_stop_plan(&current, &older, "1.2.3", "1.2.2").is_ok());
        assert!(validate_stop_plan(&current, &older, "1.2.4", "1.2.2").is_err());
        assert!(validate_stop_plan(&current, &current, "1.2.3", "1.2.3").is_err());
    }

    #[test]
    fn corrupt_current_pointer_requires_distinct_confirmation_and_preserves_evidence() {
        let mut older_manifest = manifest("1.2.2");
        for entry in older_manifest.platforms.values_mut() {
            entry.url = entry.url.replace("v1.2.3/", "v1.2.2/");
        }
        let fallback = validate_manifest(older_manifest, &stable_policy()).unwrap();
        let corrupt = br#"{"version":"1.2.3","unexpected":true}"#;

        assert!(
            validate_stop_plan_with_current_bytes(
                corrupt,
                &fallback,
                &stable_policy(),
                "1.2.3",
                "1.2.2",
                "STOP_ROLLOUT"
            )
            .is_err()
        );
        let state = validate_stop_plan_with_current_bytes(
            corrupt,
            &fallback,
            &stable_policy(),
            "1.2.3",
            "1.2.2",
            "STOP_CORRUPT_ROLLOUT",
        )
        .unwrap();
        assert!(!state.current_manifest_valid);
        assert_eq!(state.current_manifest_sha256.len(), 64);
        assert!(state.warning.is_some());

        let wrong_declared_version = br#"{"version":"1.2.4","unexpected":true}"#;
        assert!(
            validate_stop_plan_with_current_bytes(
                wrong_declared_version,
                &fallback,
                &stable_policy(),
                "1.2.3",
                "1.2.2",
                "STOP_CORRUPT_ROLLOUT"
            )
            .is_err()
        );
        let wrong_channel_version = br#"{"version":"1.2.3-beta.1","unexpected":true}"#;
        assert!(
            validate_stop_plan_with_current_bytes(
                wrong_channel_version,
                &fallback,
                &stable_policy(),
                "1.2.3",
                "1.2.2",
                "STOP_CORRUPT_ROLLOUT"
            )
            .is_err()
        );
    }

    #[test]
    fn valid_current_pointer_rejects_corrupt_rollout_confirmation() {
        let current_manifest = manifest("1.2.3");
        let current_bytes = serde_json::to_vec(&current_manifest).unwrap();
        let current = validate_manifest(current_manifest, &stable_policy()).unwrap();
        let mut older_manifest = manifest("1.2.2");
        for entry in older_manifest.platforms.values_mut() {
            entry.url = entry.url.replace("v1.2.3/", "v1.2.2/");
        }
        let fallback = validate_manifest(older_manifest, &stable_policy()).unwrap();

        assert!(
            validate_stop_plan_with_current_bytes(
                &current_bytes,
                &fallback,
                &stable_policy(),
                "1.2.3",
                "1.2.2",
                "STOP_CORRUPT_ROLLOUT"
            )
            .is_err()
        );
        assert!(validate_stop_plan(&current, &fallback, "1.2.3", "1.2.2").is_ok());
    }
}
