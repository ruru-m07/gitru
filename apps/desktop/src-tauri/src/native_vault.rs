use collaboration::credentials::{CredentialError, CredentialVault, SecretToken};
use std::sync::Mutex;

/// The production collaboration credential adapter.
///
/// Access stays serialized because the platform stores do not promise reliable
/// ordering for concurrent operations on the same credential.
pub(crate) struct NativeVault {
    service: String,
    gate: Mutex<()>,
}

impl NativeVault {
    pub(crate) fn new(service: String) -> Self {
        Self {
            service,
            gate: Mutex::new(()),
        }
    }

    fn entry(&self, reference: &str) -> Result<keyring::Entry, CredentialError> {
        keyring::Entry::new(&self.service, reference).map_err(|_| CredentialError::Unavailable)
    }
}

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

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        ffi::OsString,
        process::{Command, Stdio},
        time::{Duration, Instant},
    };
    use uuid::Uuid;

    const OPT_IN: &str = "github-hosted-v1";
    const CHILD_TEST: &str = "native_vault::tests::native_vault_platform_child";
    const CHILD_TIMEOUT: Duration = Duration::from_secs(45);
    const FORWARDED_PLATFORM_ENV: &[&str] = &[
        "APPDATA",
        "DBUS_SESSION_BUS_ADDRESS",
        "DYLD_FALLBACK_LIBRARY_PATH",
        "DYLD_LIBRARY_PATH",
        "GNOME_KEYRING_CONTROL",
        "HOME",
        "LD_LIBRARY_PATH",
        "LOCALAPPDATA",
        "PATH",
        "PROGRAMDATA",
        "SystemRoot",
        "TEMP",
        "TMP",
        "TMPDIR",
        "USERPROFILE",
        "WINDIR",
        "XDG_DATA_HOME",
        "XDG_RUNTIME_DIR",
    ];

    struct Fixture {
        run_id: String,
        service_one: String,
        service_two: String,
        reference_one: String,
        reference_two: String,
    }

    impl Fixture {
        fn new() -> Self {
            let run_id = Uuid::new_v4().to_string();
            Self {
                service_one: format!("build.gitru.native-vault.qual.{run_id}.one"),
                service_two: format!("build.gitru.native-vault.qual.{run_id}.two"),
                reference_one: format!("credential:{run_id}:one"),
                reference_two: format!("credential:{run_id}:two"),
                run_id,
            }
        }

        fn from_environment() -> Self {
            let fixture = Self {
                run_id: required("GITRU_NATIVE_VAULT_RUN_ID"),
                service_one: required("GITRU_NATIVE_VAULT_SERVICE_ONE"),
                service_two: required("GITRU_NATIVE_VAULT_SERVICE_TWO"),
                reference_one: required("GITRU_NATIVE_VAULT_REFERENCE_ONE"),
                reference_two: required("GITRU_NATIVE_VAULT_REFERENCE_TWO"),
            };
            let parsed = Uuid::parse_str(&fixture.run_id).expect("fixture run ID is a UUID");
            assert_eq!(parsed.to_string(), fixture.run_id, "run ID is canonical");
            assert_eq!(
                fixture.service_one,
                format!("build.gitru.native-vault.qual.{}.one", fixture.run_id),
                "first service belongs to this fixture"
            );
            assert_eq!(
                fixture.service_two,
                format!("build.gitru.native-vault.qual.{}.two", fixture.run_id),
                "second service belongs to this fixture"
            );
            assert_eq!(
                fixture.reference_one,
                format!("credential:{}:one", fixture.run_id),
                "first reference belongs to this fixture"
            );
            assert_eq!(
                fixture.reference_two,
                format!("credential:{}:two", fixture.run_id),
                "second reference belongs to this fixture"
            );
            fixture
        }

        fn add_to(&self, command: &mut Command) {
            command
                .env("GITRU_NATIVE_VAULT_RUN_ID", &self.run_id)
                .env("GITRU_NATIVE_VAULT_SERVICE_ONE", &self.service_one)
                .env("GITRU_NATIVE_VAULT_SERVICE_TWO", &self.service_two)
                .env("GITRU_NATIVE_VAULT_REFERENCE_ONE", &self.reference_one)
                .env("GITRU_NATIVE_VAULT_REFERENCE_TWO", &self.reference_two);
        }

        fn token(&self, suffix: &str) -> SecretToken {
            SecretToken::new(format!(
                "gitru_ci_{}_{}",
                self.run_id.replace('-', ""),
                suffix
            ))
            .expect("fixture token is valid")
        }
    }

    fn required(name: &str) -> String {
        std::env::var(name).unwrap_or_else(|_| panic!("missing isolated fixture field {name}"))
    }

    fn compiled_runner_os() -> &'static str {
        if cfg!(target_os = "linux") {
            "Linux"
        } else if cfg!(target_os = "macos") {
            "macOS"
        } else if cfg!(target_os = "windows") {
            "Windows"
        } else {
            "unsupported"
        }
    }

    fn validate_gate(
        opt_in: Option<&str>,
        github_actions: Option<&str>,
        runner_environment: Option<&str>,
        runner_os: Option<&str>,
    ) -> Result<(), &'static str> {
        if opt_in != Some(OPT_IN) {
            return Err("native vault qualification is not explicitly enabled");
        }
        if github_actions != Some("true") || runner_environment != Some("github-hosted") {
            return Err("native vault qualification requires a GitHub-hosted runner");
        }
        if runner_os != Some(compiled_runner_os()) {
            return Err("native vault qualification runner does not match the compiled target");
        }
        Ok(())
    }

    fn qualification_gate() {
        validate_gate(
            std::env::var("GITRU_NATIVE_VAULT_QUALIFICATION")
                .ok()
                .as_deref(),
            std::env::var("GITHUB_ACTIONS").ok().as_deref(),
            std::env::var("RUNNER_ENVIRONMENT").ok().as_deref(),
            std::env::var("RUNNER_OS").ok().as_deref(),
        )
        .unwrap_or_else(|message| panic!("{message}"));
        let temporary = std::env::var_os("RUNNER_TEMP").expect("runner temp is available");
        assert!(
            std::path::Path::new(&temporary).is_dir(),
            "runner temp is an existing directory"
        );
    }

    fn forward_platform_environment(command: &mut Command) {
        for name in FORWARDED_PLATFORM_ENV {
            if let Some(value) = std::env::var_os(name) {
                command.env(name, value);
            }
        }
    }

    fn run_phase(fixture: &Fixture, phase: &str) -> Result<(), &'static str> {
        let mut command =
            Command::new(std::env::current_exe().map_err(|_| "test executable missing")?);
        command
            .args(["--exact", CHILD_TEST, "--ignored", "--nocapture"])
            .env_clear()
            .env("GITRU_NATIVE_VAULT_QUALIFICATION", OPT_IN)
            .env("GITHUB_ACTIONS", "true")
            .env("RUNNER_ENVIRONMENT", "github-hosted")
            .env("RUNNER_OS", compiled_runner_os())
            .env(
                "RUNNER_TEMP",
                std::env::var_os("RUNNER_TEMP").ok_or("runner temp missing")?,
            )
            .env("GITRU_NATIVE_VAULT_PHASE", phase)
            .stdin(Stdio::null())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit());
        forward_platform_environment(&mut command);
        if phase == "unavailable" && cfg!(target_os = "linux") {
            let missing_socket = std::path::PathBuf::from(
                std::env::var_os("RUNNER_TEMP").ok_or("runner temp missing")?,
            )
            .join(format!(
                "gitru-native-vault-missing-{}.sock",
                fixture.run_id
            ));
            let _ = std::fs::remove_file(&missing_socket);
            command.env(
                "DBUS_SESSION_BUS_ADDRESS",
                format!("unix:path={}", missing_socket.display()),
            );
        }
        fixture.add_to(&mut command);
        let mut child = command
            .spawn()
            .map_err(|_| "qualification child did not start")?;
        let deadline = Instant::now() + CHILD_TIMEOUT;
        loop {
            if let Some(status) = child
                .try_wait()
                .map_err(|_| "qualification child status was unavailable")?
            {
                return status
                    .success()
                    .then_some(())
                    .ok_or("qualification child reported failure");
            }
            if Instant::now() >= deadline {
                let _ = child.kill();
                let _ = child.wait();
                return Err("qualification child exceeded its 45-second bound");
            }
            std::thread::sleep(Duration::from_millis(25));
        }
    }

    fn expect_token(vault: &NativeVault, reference: &str, expected: &SecretToken, label: &str) {
        let loaded = vault
            .load(reference)
            .expect("native credential load succeeds");
        assert!(
            loaded
                .as_ref()
                .is_some_and(|value| value.expose() == expected.expose()),
            "{label} has the expected synthetic value"
        );
        assert_eq!(
            format!("{expected:?}"),
            "SecretToken([REDACTED])",
            "synthetic values stay redacted in diagnostics"
        );
    }

    #[test]
    fn native_vault_gate_requires_explicit_github_hosted_matrix() {
        assert!(validate_gate(None, Some("true"), Some("github-hosted"), None).is_err());
        assert!(validate_gate(
            Some(OPT_IN),
            Some("true"),
            Some("self-hosted"),
            Some(compiled_runner_os())
        )
        .is_err());
        assert!(validate_gate(
            Some(OPT_IN),
            Some("true"),
            Some("github-hosted"),
            Some("different-os")
        )
        .is_err());
        assert!(validate_gate(
            Some(OPT_IN),
            Some("true"),
            Some("github-hosted"),
            Some(compiled_runner_os())
        )
        .is_ok());
        for forbidden in ["GH_TOKEN", "GITHUB_TOKEN", "GITRU_GITHUB_PAT"] {
            assert!(!FORWARDED_PLATFORM_ENV.contains(&forbidden));
        }
    }

    #[test]
    #[ignore = "runs only in the explicit GitHub-hosted native-vault matrix"]
    fn native_vault_platform_round_trip() {
        qualification_gate();
        let fixture = Fixture::new();
        let first = run_phase(&fixture, "seed");
        let reopened = if first.is_ok() {
            run_phase(&fixture, "reopen")
        } else {
            Err("reopen was skipped after seed failure")
        };
        let cleanup = run_phase(&fixture, "cleanup");
        let unavailable = if cfg!(target_os = "linux") {
            run_phase(&fixture, "unavailable")
        } else {
            Ok(())
        };
        assert!(cleanup.is_ok(), "owned native credentials are cleaned up");
        assert!(first.is_ok(), "native seed phase succeeds");
        assert!(reopened.is_ok(), "fresh-process native reopen succeeds");
        assert!(
            unavailable.is_ok(),
            "isolated unavailable Secret Service fails closed"
        );
    }

    #[test]
    #[ignore = "entered only by the bounded native-vault qualification parent"]
    fn native_vault_platform_child() {
        qualification_gate();
        let fixture = Fixture::from_environment();
        let first = NativeVault::new(fixture.service_one.clone());
        let second = NativeVault::new(fixture.service_two.clone());
        match required("GITRU_NATIVE_VAULT_PHASE").as_str() {
            "seed" => {
                first
                    .delete(&fixture.reference_one)
                    .expect("owned first reference can start absent");
                first
                    .delete(&fixture.reference_two)
                    .expect("owned second reference can start absent");
                second
                    .delete(&fixture.reference_one)
                    .expect("owned second service can start absent");
                assert!(
                    first.load(&fixture.reference_one).unwrap().is_none(),
                    "absent credential is distinct from unavailable storage"
                );
                let original = fixture.token("original");
                first
                    .store(&fixture.reference_one, &original)
                    .expect("native credential store succeeds");
                expect_token(
                    &first,
                    &fixture.reference_one,
                    &original,
                    "stored credential",
                );
                let replacement = fixture.token("replacement");
                first
                    .store(&fixture.reference_one, &replacement)
                    .expect("native credential replacement succeeds");
                expect_token(
                    &first,
                    &fixture.reference_one,
                    &replacement,
                    "replacement credential",
                );
                let sibling = fixture.token("sibling");
                first
                    .store(&fixture.reference_two, &sibling)
                    .expect("second native reference stores independently");
                let other_service = fixture.token("other_service");
                second
                    .store(&fixture.reference_one, &other_service)
                    .expect("same reference in another service stores independently");
            }
            "reopen" => {
                let replacement = fixture.token("replacement");
                let sibling = fixture.token("sibling");
                let other_service = fixture.token("other_service");
                expect_token(
                    &first,
                    &fixture.reference_one,
                    &replacement,
                    "cold reopened replacement",
                );
                expect_token(
                    &first,
                    &fixture.reference_two,
                    &sibling,
                    "cold reopened sibling",
                );
                expect_token(
                    &second,
                    &fixture.reference_one,
                    &other_service,
                    "cold reopened separate service",
                );
                let child_replacement = fixture.token("child_replacement");
                first
                    .store(&fixture.reference_one, &child_replacement)
                    .expect("fresh process can replace its owned credential");
                expect_token(
                    &first,
                    &fixture.reference_one,
                    &child_replacement,
                    "fresh-process replacement",
                );
                first
                    .delete(&fixture.reference_one)
                    .expect("fresh process deletes its owned credential");
                first
                    .delete(&fixture.reference_one)
                    .expect("missing credential deletion is idempotent");
                assert!(first.load(&fixture.reference_one).unwrap().is_none());
                expect_token(
                    &first,
                    &fixture.reference_two,
                    &sibling,
                    "sibling survives exact deletion",
                );
                expect_token(
                    &second,
                    &fixture.reference_one,
                    &other_service,
                    "separate service survives exact deletion",
                );
            }
            "cleanup" => {
                for result in [
                    first.delete(&fixture.reference_one),
                    first.delete(&fixture.reference_two),
                    second.delete(&fixture.reference_one),
                ] {
                    result.expect("owned fixture credential cleanup succeeds");
                }
                assert!(first.load(&fixture.reference_one).unwrap().is_none());
                assert!(first.load(&fixture.reference_two).unwrap().is_none());
                assert!(second.load(&fixture.reference_one).unwrap().is_none());
                first
                    .delete(&fixture.reference_one)
                    .expect("cleanup confirms idempotent missing deletion");
            }
            "unavailable" if cfg!(target_os = "linux") => {
                assert!(matches!(
                    first.load(&fixture.reference_one),
                    Err(CredentialError::Unavailable)
                ));
            }
            _ => panic!("unknown bounded native-vault phase"),
        }
    }

    #[test]
    fn forwarded_environment_names_are_bounded() {
        let expected = FORWARDED_PLATFORM_ENV
            .iter()
            .map(|value| OsString::from(*value))
            .collect::<Vec<_>>();
        assert_eq!(expected.len(), FORWARDED_PLATFORM_ENV.len());
        assert!(expected.contains(&OsString::from("DBUS_SESSION_BUS_ADDRESS")));
        assert!(!expected.contains(&OsString::from("GITHUB_TOKEN")));
    }
}
