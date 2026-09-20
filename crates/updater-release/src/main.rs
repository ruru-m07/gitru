use std::{
    collections::BTreeMap,
    env, fs,
    path::{Path, PathBuf},
    process,
    str::FromStr,
};
use updater_release::{
    Channel, ManifestPolicy, Result, StopPlanState, ValidationReport, current_pointer_high_water,
    package_version, parse_manifest, production_public_key, validate_event_metadata,
    validate_manifest, validate_release_metadata, validate_stop_plan_with_current_bytes,
    verify_and_download, verify_embedded_signature, verify_manifest_sidecar_or_legacy,
};

fn main() {
    if let Err(error) = run() {
        eprintln!("updater-release: {error}");
        process::exit(1);
    }
}

fn run() -> Result<()> {
    let mut arguments = env::args().skip(1);
    let command = arguments.next().ok_or_else(usage)?;
    let flags = parse_flags(arguments.collect())?;

    match command.as_str() {
        "validate-event" => validate_event_command(&flags),
        "validate-release" => validate_release_command(&flags),
        "validate-promotion" => validate_promotion_command(&flags),
        "validate-feed" => validate_feed_command(&flags),
        "validate-stop" => validate_stop_command(&flags),
        "verify-signature" => verify_signature_command(&flags),
        "help" | "--help" | "-h" => {
            println!("{}", usage());
            Ok(())
        }
        _ => Err(format!("unknown command {command:?}\n\n{}", usage())),
    }
}

fn validate_event_command(flags: &BTreeMap<String, String>) -> Result<()> {
    require_only(flags, &["package-json", "tag", "channel"])?;
    let package = package_version(&read(&required_path(flags, "package-json")?)?)?;
    let channel = required_channel(flags)?;
    validate_event_metadata(required(flags, "tag")?, &package, channel)?;
    println!("validated {} release event for {package}", channel.as_str());
    Ok(())
}

fn validate_release_command(flags: &BTreeMap<String, String>) -> Result<()> {
    require_only(
        flags,
        &[
            "manifest",
            "config",
            "package-json",
            "repository",
            "tag",
            "channel",
            "history",
            "revocations",
            "current-manifest",
            "download-directory",
            "report",
        ],
    )?;
    let manifest_path = required_path(flags, "manifest")?;
    let manifest_bytes = read(&manifest_path)?;
    let channel = required_channel(flags)?;
    let repository = required(flags, "repository")?;
    let validated = validate_manifest(
        parse_manifest(&manifest_bytes)?,
        &ManifestPolicy {
            repository,
            channel,
        },
    )?;
    let package_bytes = read(&required_path(flags, "package-json")?)?;
    let mut history = read_history(&required_path(flags, "history")?)?;
    let revocations = read_history(&required_path(flags, "revocations")?)?;
    if let Some(current_path) = flags.get("current-manifest") {
        let current_bytes = read(Path::new(current_path))?;
        if let Some(current_version) = current_pointer_high_water(
            &manifest_bytes,
            &current_bytes,
            &ManifestPolicy {
                repository,
                channel,
            },
        )? {
            history.push(current_version);
        }
    }
    validate_release_metadata(
        &validated,
        required(flags, "tag")?,
        &package_version(&package_bytes)?,
        &history,
        &revocations,
    )?;
    let public_key = read_production_public_key(flags)?;
    let report = verify_and_download(
        &manifest_bytes,
        &validated,
        &public_key,
        &required_path(flags, "download-directory")?,
    )?;
    write_report(flags, &report)
}

fn validate_feed_command(flags: &BTreeMap<String, String>) -> Result<()> {
    require_only(
        flags,
        &[
            "manifest",
            "config",
            "repository",
            "channel",
            "download-directory",
            "report",
        ],
    )?;
    let manifest_path = required_path(flags, "manifest")?;
    let manifest_bytes = read(&manifest_path)?;
    let validated = validate_manifest(
        parse_manifest(&manifest_bytes)?,
        &ManifestPolicy {
            repository: required(flags, "repository")?,
            channel: required_channel(flags)?,
        },
    )?;
    let public_key = read_production_public_key(flags)?;
    let report = verify_and_download(
        &manifest_bytes,
        &validated,
        &public_key,
        &required_path(flags, "download-directory")?,
    )?;
    write_report(flags, &report)
}

fn validate_promotion_command(flags: &BTreeMap<String, String>) -> Result<()> {
    require_only(
        flags,
        &[
            "manifest",
            "manifest-signature",
            "config",
            "package-json",
            "repository",
            "tag",
            "channel",
            "history",
            "revocations",
            "current-manifest",
        ],
    )?;
    let manifest_bytes = read(&required_path(flags, "manifest")?)?;
    let channel = required_channel(flags)?;
    let repository = required(flags, "repository")?;
    let policy = ManifestPolicy {
        repository,
        channel,
    };
    let validated = validate_manifest(parse_manifest(&manifest_bytes)?, &policy)?;
    let mut history = read_history(&required_path(flags, "history")?)?;
    if let Some(current_path) = flags.get("current-manifest") {
        let current_bytes = read(Path::new(current_path))?;
        if let Some(current_version) =
            current_pointer_high_water(&manifest_bytes, &current_bytes, &policy)?
        {
            history.push(current_version);
        }
    }
    validate_release_metadata(
        &validated,
        required(flags, "tag")?,
        &package_version(&read(&required_path(flags, "package-json")?)?)?,
        &history,
        &read_history(&required_path(flags, "revocations")?)?,
    )?;
    let signature = read_string(&required_path(flags, "manifest-signature")?)?;
    verify_embedded_signature(
        &manifest_bytes,
        &signature,
        &read_production_public_key(flags)?,
    )
    .map_err(|error| format!("candidate manifest sidecar failed verification: {error}"))?;
    println!(
        "validated {} {} promotion state and manifest sidecar",
        channel.as_str(),
        validated.version
    );
    Ok(())
}

fn validate_stop_command(flags: &BTreeMap<String, String>) -> Result<()> {
    require_only(
        flags,
        &[
            "current-manifest",
            "fallback-manifest",
            "fallback-signature",
            "allow-legacy-manifest-without-sidecar",
            "confirmation",
            "stop-state",
            "config",
            "repository",
            "channel",
            "expected-current-version",
            "expected-fallback-version",
            "download-directory",
            "report",
        ],
    )?;
    let channel = required_channel(flags)?;
    let repository = required(flags, "repository")?;
    let policy = ManifestPolicy {
        repository,
        channel,
    };
    let current_bytes = read(&required_path(flags, "current-manifest")?)?;
    let fallback_bytes = read(&required_path(flags, "fallback-manifest")?)?;
    let fallback = validate_manifest(parse_manifest(&fallback_bytes)?, &policy)?;

    let public_key = read_production_public_key(flags)?;
    let fallback_signature = flags
        .get("fallback-signature")
        .map(|path| read_string(Path::new(path)))
        .transpose()?;
    let allow_legacy = match flags.get("allow-legacy-manifest-without-sidecar") {
        Some(value) if value == "true" => true,
        Some(value) if value == "false" => false,
        Some(value) => {
            return Err(format!(
                "--allow-legacy-manifest-without-sidecar must be true or false, got {value:?}"
            ));
        }
        None => false,
    };
    if fallback_signature.is_some() && allow_legacy {
        return Err(
            "do not allow a legacy manifest when a fallback signature sidecar is available"
                .to_string(),
        );
    }
    let sidecar_verified = verify_manifest_sidecar_or_legacy(
        &fallback_bytes,
        fallback_signature.as_deref(),
        &public_key,
        allow_legacy,
    )?;
    let stop_state = validate_stop_plan_with_current_bytes(
        &current_bytes,
        &fallback,
        &policy,
        required(flags, "expected-current-version")?,
        required(flags, "expected-fallback-version")?,
        required(flags, "confirmation")?,
    )?;
    write_stop_state(&required_path(flags, "stop-state")?, &stop_state)?;
    let report = verify_and_download(
        &fallback_bytes,
        &fallback,
        &public_key,
        &required_path(flags, "download-directory")?,
    )?;
    if !sidecar_verified {
        println!(
            "legacy fallback has no manifest sidecar; all embedded artifact signatures were verified"
        );
    }
    write_report(flags, &report)
}

fn write_stop_state(path: &Path, state: &StopPlanState) -> Result<()> {
    let json = serde_json::to_string_pretty(state)
        .map_err(|error| format!("failed to serialize stop state: {error}"))?;
    fs::write(path, format!("{json}\n"))
        .map_err(|error| format!("failed to write {}: {error}", path.display()))?;
    if let Some(warning) = &state.warning {
        println!("WARNING: {warning}");
    }
    Ok(())
}

fn verify_signature_command(flags: &BTreeMap<String, String>) -> Result<()> {
    require_only(flags, &["file", "signature", "config"])?;
    let bytes = read(&required_path(flags, "file")?)?;
    let signature = read_string(&required_path(flags, "signature")?)?;
    let public_key = read_production_public_key(flags)?;
    verify_embedded_signature(&bytes, &signature, &public_key)?;
    println!("verified {}", required_path(flags, "file")?.display());
    Ok(())
}

fn parse_flags(arguments: Vec<String>) -> Result<BTreeMap<String, String>> {
    let mut flags = BTreeMap::new();
    let mut index = 0;
    while index < arguments.len() {
        let raw = &arguments[index];
        let name = raw
            .strip_prefix("--")
            .filter(|name| !name.is_empty())
            .ok_or_else(|| format!("expected --flag, got {raw:?}"))?;
        let value = arguments
            .get(index + 1)
            .filter(|value| !value.starts_with("--"))
            .ok_or_else(|| format!("missing value for --{name}"))?;
        if flags.insert(name.to_string(), value.clone()).is_some() {
            return Err(format!("duplicate --{name}"));
        }
        index += 2;
    }
    Ok(flags)
}

fn require_only(flags: &BTreeMap<String, String>, allowed: &[&str]) -> Result<()> {
    for flag in flags.keys() {
        if !allowed.contains(&flag.as_str()) {
            return Err(format!("unexpected flag --{flag}"));
        }
    }
    Ok(())
}

fn required<'a>(flags: &'a BTreeMap<String, String>, name: &str) -> Result<&'a str> {
    flags
        .get(name)
        .map(String::as_str)
        .ok_or_else(|| format!("missing required --{name}"))
}

fn required_path(flags: &BTreeMap<String, String>, name: &str) -> Result<PathBuf> {
    required(flags, name).map(PathBuf::from)
}

fn required_channel(flags: &BTreeMap<String, String>) -> Result<Channel> {
    Channel::from_str(required(flags, "channel")?)
}

fn read(path: &Path) -> Result<Vec<u8>> {
    fs::read(path).map_err(|error| format!("failed to read {}: {error}", path.display()))
}

fn read_string(path: &Path) -> Result<String> {
    String::from_utf8(read(path)?).map_err(|_| format!("{} is not UTF-8", path.display()))
}

fn read_history(path: &Path) -> Result<Vec<String>> {
    Ok(read_string(path)?
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_string)
        .collect())
}

fn read_production_public_key(flags: &BTreeMap<String, String>) -> Result<String> {
    production_public_key(&read(&required_path(flags, "config")?)?)
}

fn write_report(flags: &BTreeMap<String, String>, report: &ValidationReport) -> Result<()> {
    let json = serde_json::to_string_pretty(report)
        .map_err(|error| format!("failed to serialize validation report: {error}"))?;
    fs::write(required_path(flags, "report")?, format!("{json}\n"))
        .map_err(|error| format!("failed to write validation report: {error}"))?;
    println!(
        "validated {} {} manifest and {} unique signed artifacts",
        report.channel.as_str(),
        report.version,
        report.artifacts.len()
    );
    Ok(())
}

fn usage() -> String {
    "Usage:\n  updater-release validate-event --package-json PATH --tag vVERSION --channel stable|beta\n  updater-release validate-release --manifest PATH --config PATH --package-json PATH --repository OWNER/NAME --tag vVERSION --channel stable|beta --history PATH --revocations PATH [--current-manifest PATH] --download-directory PATH --report PATH\n  updater-release validate-promotion --manifest PATH --manifest-signature PATH --config PATH --package-json PATH --repository OWNER/NAME --tag vVERSION --channel stable|beta --history PATH --revocations PATH [--current-manifest PATH]\n  updater-release validate-feed --manifest PATH --config PATH --repository OWNER/NAME --channel stable|beta --download-directory PATH --report PATH\n  updater-release validate-stop --current-manifest PATH --fallback-manifest PATH (--fallback-signature PATH | --allow-legacy-manifest-without-sidecar true) --config PATH --repository OWNER/NAME --channel stable|beta --expected-current-version VERSION --expected-fallback-version VERSION --confirmation STOP_ROLLOUT|STOP_CORRUPT_ROLLOUT --stop-state PATH --download-directory PATH --report PATH\n  updater-release verify-signature --file PATH --signature PATH --config PATH".to_string()
}
