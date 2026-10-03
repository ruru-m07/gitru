//! Optional, explicit import of credentials already managed by GitHub CLI.
//! Background discovery returns only metadata. No shell, login, switch, logout,
//! environment token, raw diagnostic, or credential-bearing DTO is involved.

use crate::{credentials::SecretToken, *};
use async_trait::async_trait;
use serde::Deserialize;
use std::{
    collections::{HashMap, HashSet},
    ffi::OsString,
    path::PathBuf,
    process::Stdio,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::{io::AsyncReadExt, process::Command, sync::Mutex};
use zeroize::Zeroizing;

const COMMAND_TIMEOUT: Duration = Duration::from_secs(8);
const CANDIDATE_TTL: Duration = Duration::from_secs(300);
const MAX_ACCOUNTS: usize = 100;
const MAX_METADATA_BYTES: usize = 64 * 1024;
const MAX_DIAGNOSTIC_BYTES: usize = 16 * 1024;
const METADATA_PROJECTION: &str = "[.hosts[\"github.com\"][]? | {state,active,host,login}]";

/// Native-only runner seam. Tests supply fixtures without touching real CLI
/// configuration or keychains. Output buffers are wiped on every exit path.
#[async_trait]
pub trait GithubCliRunner: Send + Sync + 'static {
    async fn run(&self, command: GithubCliCommand) -> Result<GithubCliOutput, GithubCliFailure>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GithubCliCommand {
    TokenHelp,
    Discover,
    Token { login: String },
}

pub struct GithubCliOutput {
    pub success: bool,
    pub stdout: Zeroizing<Vec<u8>>,
    pub stderr: Zeroizing<Vec<u8>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GithubCliFailure {
    NotInstalled,
    Unsupported,
    Unavailable,
}

#[derive(Clone)]
struct Candidate {
    login: String,
    availability: GithubCliAccountAvailability,
    discovered_at: Instant,
}

pub struct GithubCli {
    runner: Arc<dyn GithubCliRunner>,
    gate: Mutex<()>,
    candidates: Mutex<HashMap<String, Candidate>>,
}

impl GithubCli {
    pub fn native() -> Self {
        Self::with_runner(Arc::new(NativeRunner::discover()))
    }

    /// The default runtime and packaged E2E never discover a person's account.
    /// Production Tauri setup opts into `native()` explicitly.
    pub fn disabled() -> Self {
        Self::with_runner(Arc::new(DisabledRunner))
    }

    pub fn with_runner(runner: Arc<dyn GithubCliRunner>) -> Self {
        Self {
            runner,
            gate: Mutex::new(()),
            candidates: Mutex::new(HashMap::new()),
        }
    }

    pub async fn discover(&self) -> GithubCliDiscovery {
        let _gate = self.gate.lock().await;
        // Refresh invalidates previously displayed selections, including on
        // failure. Import always has to select a current, bounded candidate.
        self.candidates.lock().await.clear();
        let metadata = match self.metadata().await {
            Ok(metadata) => metadata,
            Err(failure) => {
                return GithubCliDiscovery {
                    status: match failure {
                        GithubCliFailure::NotInstalled => GithubCliStatus::NotInstalled,
                        GithubCliFailure::Unsupported => GithubCliStatus::Unsupported,
                        GithubCliFailure::Unavailable => GithubCliStatus::Unavailable,
                    },
                    accounts: Vec::new(),
                };
            }
        };
        let mut candidates = self.candidates.lock().await;
        let mut accounts = Vec::with_capacity(metadata.len());
        for entry in metadata {
            let id = uuid::Uuid::new_v4().to_string();
            let availability = entry.availability();
            candidates.insert(
                id.clone(),
                Candidate {
                    login: entry.login.clone(),
                    availability: availability.clone(),
                    discovered_at: Instant::now(),
                },
            );
            accounts.push(GithubCliAccount {
                id,
                login: entry.login,
                host: "github.com".into(),
                active: entry.active,
                availability,
            });
        }
        GithubCliDiscovery {
            status: GithubCliStatus::Available,
            accounts,
        }
    }

    /// Only an explicitly selected, fresh account can trigger credential output.
    /// The caller must verify the returned credential's login with `/user`
    /// before native-vault or account-database side effects.
    pub async fn import(
        &self,
        candidate_id: &str,
    ) -> Result<(String, SecretToken), CollaborationError> {
        let _gate = self.gate.lock().await;
        let candidate = self
            .candidates
            .lock()
            .await
            .get(candidate_id)
            .filter(|candidate| candidate.discovered_at.elapsed() < CANDIDATE_TTL)
            .cloned()
            .ok_or_else(stale_candidate)?;
        if candidate.availability != GithubCliAccountAvailability::Ready {
            return Err(cli_auth_required());
        }
        // Do not trust the old status: gh can change accounts or log out while
        // the dialog remains open. No implicit active-account token fallback.
        let current = self.metadata().await.map_err(import_failure)?;
        let selected = current
            .iter()
            .find(|entry| entry.login.eq_ignore_ascii_case(&candidate.login))
            .ok_or_else(stale_candidate)?;
        if selected.availability() != GithubCliAccountAvailability::Ready {
            return Err(cli_auth_required());
        }
        let output = self
            .runner
            .run(GithubCliCommand::Token {
                login: candidate.login.clone(),
            })
            .await
            .map_err(import_failure)?;
        if !output.success {
            return Err(if unsupported_diagnostic(&output.stderr) {
                import_failure(GithubCliFailure::Unsupported)
            } else {
                cli_auth_required()
            });
        }
        let token = parse_token(&output.stdout)?;
        Ok((candidate.login, token))
    }

    async fn metadata(&self) -> Result<Vec<MetadataEntry>, GithubCliFailure> {
        // Old CLI versions must support both JSON and explicit account tokens.
        // Never compensate with locale-sensitive status text or active tokens.
        let help = self.runner.run(GithubCliCommand::TokenHelp).await?;
        if !help.success || !contains(&help.stdout, b"--user") {
            return Err(GithubCliFailure::Unsupported);
        }
        let output = self.runner.run(GithubCliCommand::Discover).await?;
        if !output.success {
            return Err(if unsupported_diagnostic(&output.stderr) {
                GithubCliFailure::Unsupported
            } else {
                GithubCliFailure::Unavailable
            });
        }
        parse_metadata(&output.stdout)
    }
}

#[derive(Deserialize)]
struct MetadataEntry {
    state: String,
    active: bool,
    host: String,
    login: String,
}

impl MetadataEntry {
    fn availability(&self) -> GithubCliAccountAvailability {
        match self.state.as_str() {
            "success" => GithubCliAccountAvailability::Ready,
            "error" => GithubCliAccountAvailability::AuthRequired,
            _ => GithubCliAccountAvailability::Unavailable,
        }
    }
}

fn parse_metadata(bytes: &[u8]) -> Result<Vec<MetadataEntry>, GithubCliFailure> {
    if bytes.len() > MAX_METADATA_BYTES {
        return Err(GithubCliFailure::Unavailable);
    }
    let entries: Vec<MetadataEntry> =
        serde_json::from_slice(bytes).map_err(|_| GithubCliFailure::Unavailable)?;
    if entries.len() > MAX_ACCOUNTS {
        return Err(GithubCliFailure::Unavailable);
    }
    let mut seen = HashSet::new();
    let mut entries: Vec<_> = entries
        .into_iter()
        .filter(|entry| {
            entry.host == "github.com"
                && valid_login(&entry.login)
                && seen.insert(entry.login.to_ascii_lowercase())
        })
        .collect();
    entries.sort_by(|left, right| {
        right.active.cmp(&left.active).then_with(|| {
            left.login
                .to_ascii_lowercase()
                .cmp(&right.login.to_ascii_lowercase())
        })
    });
    Ok(entries)
}

fn valid_login(login: &str) -> bool {
    !login.is_empty()
        && login.len() <= 39
        && login
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
        && login.as_bytes()[0].is_ascii_alphanumeric()
        && login.as_bytes()[login.len() - 1].is_ascii_alphanumeric()
        && !["ghp_", "gho_", "github_pat_"]
            .iter()
            .any(|prefix| login.starts_with(prefix))
}

fn parse_token(bytes: &[u8]) -> Result<SecretToken, CollaborationError> {
    // gh emits exactly one token followed by a newline. Do not accept arbitrary
    // trimming: leading whitespace, extra lines and embedded spaces are errors.
    let bytes = bytes
        .strip_suffix(b"\r\n")
        .or_else(|| bytes.strip_suffix(b"\n"))
        .unwrap_or(bytes);
    let text = std::str::from_utf8(bytes).map_err(|_| cli_auth_required())?;
    SecretToken::new(text.to_string()).map_err(|_| cli_auth_required())
}

fn contains(bytes: &[u8], needle: &[u8]) -> bool {
    bytes.windows(needle.len()).any(|window| window == needle)
}

fn unsupported_diagnostic(bytes: &[u8]) -> bool {
    contains(bytes, b"unknown flag") || contains(bytes, b"unknown command")
}

fn stale_candidate() -> CollaborationError {
    CollaborationError::new(
        ErrorCode::StaleView,
        "Check GitHub CLI accounts again, then choose the account to connect",
    )
}

fn cli_auth_required() -> CollaborationError {
    CollaborationError::new(
        ErrorCode::AuthRequired,
        "This GitHub CLI account needs attention; sign in with gh or enter a personal access token",
    )
}

fn import_failure(failure: GithubCliFailure) -> CollaborationError {
    let (code, message) = match failure {
        GithubCliFailure::NotInstalled => (
            ErrorCode::Unsupported,
            "Install GitHub CLI in a supported location or enter a personal access token",
        ),
        GithubCliFailure::Unsupported => (
            ErrorCode::Unsupported,
            "Update GitHub CLI to support account discovery or enter a personal access token",
        ),
        GithubCliFailure::Unavailable => (
            ErrorCode::Provider,
            "GitHub CLI did not respond; try again or enter a personal access token",
        ),
    };
    CollaborationError::new(code, message)
}

struct DisabledRunner;

#[async_trait]
impl GithubCliRunner for DisabledRunner {
    async fn run(&self, _: GithubCliCommand) -> Result<GithubCliOutput, GithubCliFailure> {
        Err(GithubCliFailure::NotInstalled)
    }
}

struct NativeRunner {
    candidates: Vec<PathBuf>,
    environment: Vec<(OsString, OsString)>,
    cwd: PathBuf,
    timeout: Duration,
}

impl NativeRunner {
    fn discover() -> Self {
        let environment = std::env::vars_os().collect::<Vec<_>>();
        Self {
            candidates: executable_candidates(&environment),
            environment: allowed_environment(environment),
            // Account queries must never resolve repository-local executables,
            // configuration, hooks or dotenv files from the current checkout.
            cwd: neutral_cwd(),
            timeout: COMMAND_TIMEOUT,
        }
    }
}

#[async_trait]
impl GithubCliRunner for NativeRunner {
    async fn run(&self, request: GithubCliCommand) -> Result<GithubCliOutput, GithubCliFailure> {
        // Resolve each invocation from trusted installation locations. Caching
        // a missing result would prevent Check again from seeing a new install;
        // caching a canonical Homebrew Cellar path would break after upgrades.
        let executable =
            resolve_executable(&self.candidates).ok_or(GithubCliFailure::NotInstalled)?;
        let mut command = Command::new(executable);
        command
            .args(command_arguments(&request))
            .env_clear()
            .envs(self.environment.iter().cloned())
            .current_dir(&self.cwd)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        #[cfg(windows)]
        command.creation_flags(0x08000000); // CREATE_NO_WINDOW
        let mut child = command.spawn().map_err(|_| GithubCliFailure::Unavailable)?;
        let stdout = child.stdout.take().ok_or(GithubCliFailure::Unavailable)?;
        let stderr = child.stderr.take().ok_or(GithubCliFailure::Unavailable)?;
        let stdout_limit = if matches!(request, GithubCliCommand::Token { .. }) {
            4098
        } else {
            MAX_METADATA_BYTES
        };
        let result = tokio::time::timeout(self.timeout, async {
            let (stdout, stderr, status) = tokio::try_join!(
                read_bounded(stdout, stdout_limit),
                read_bounded(stderr, MAX_DIAGNOSTIC_BYTES),
                async {
                    child
                        .wait()
                        .await
                        .map_err(|_| GithubCliFailure::Unavailable)
                },
            )?;
            Ok::<_, GithubCliFailure>(GithubCliOutput {
                success: status.success(),
                stdout,
                stderr,
            })
        })
        .await;
        match result {
            Ok(Ok(output)) => Ok(output),
            _ => {
                // Do not wait indefinitely for a stuck credential helper or
                // flooded pipe. Child drop also arranges process cleanup.
                let _ = child.start_kill();
                let _ = tokio::time::timeout(Duration::from_secs(1), child.wait()).await;
                Err(GithubCliFailure::Unavailable)
            }
        }
    }
}

fn resolve_executable(candidates: &[PathBuf]) -> Option<PathBuf> {
    candidates.iter().find_map(|path| {
        let resolved = std::fs::canonicalize(path).ok()?;
        let metadata = std::fs::metadata(&resolved).ok()?;
        if !metadata.is_file() {
            return None;
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if metadata.permissions().mode() & 0o111 == 0 {
                return None;
            }
        }
        Some(resolved)
    })
}

fn command_arguments(request: &GithubCliCommand) -> Vec<&str> {
    match request {
        GithubCliCommand::TokenHelp => vec!["auth", "token", "--help"],
        GithubCliCommand::Discover => vec![
            "auth",
            "status",
            "--hostname",
            "github.com",
            "--json",
            "hosts",
            "--jq",
            METADATA_PROJECTION,
        ],
        GithubCliCommand::Token { login } => {
            vec!["auth", "token", "--hostname", "github.com", "--user", login]
        }
    }
}

async fn read_bounded(
    mut reader: impl tokio::io::AsyncRead + Unpin,
    limit: usize,
) -> Result<Zeroizing<Vec<u8>>, GithubCliFailure> {
    // Allocate the bound once: reallocation could otherwise leave secret bytes
    // behind in a freed earlier allocation before Zeroizing wipes the final one.
    let mut output = Zeroizing::new(Vec::with_capacity(limit));
    let mut chunk = Zeroizing::new([0u8; 4096]);
    loop {
        let size = reader
            .read(&mut *chunk)
            .await
            .map_err(|_| GithubCliFailure::Unavailable)?;
        if size == 0 {
            return Ok(output);
        }
        if output.len() + size > limit {
            return Err(GithubCliFailure::Unavailable);
        }
        output.extend_from_slice(&chunk[..size]);
    }
}

fn allowed_environment(environment: Vec<(OsString, OsString)>) -> Vec<(OsString, OsString)> {
    const ALLOWED: &[&str] = &[
        "HOME",
        "USERPROFILE",
        "APPDATA",
        "LOCALAPPDATA",
        "XDG_CONFIG_HOME",
        "GH_CONFIG_DIR",
        "XDG_RUNTIME_DIR",
        "DBUS_SESSION_BUS_ADDRESS",
        "DISPLAY",
        "WAYLAND_DISPLAY",
        "SYSTEMROOT",
        "WINDIR",
        "TEMP",
        "TMP",
        "HTTPS_PROXY",
        "HTTP_PROXY",
        "NO_PROXY",
        "SSL_CERT_FILE",
        "SSL_CERT_DIR",
    ];
    let mut environment: Vec<_> = environment
        .into_iter()
        .filter(|(name, _)| {
            name.to_str()
                .is_some_and(|name| ALLOWED.contains(&name.to_ascii_uppercase().as_str()))
        })
        .collect();
    environment.extend([
        ("GH_PROMPT_DISABLED".into(), "1".into()),
        ("GH_NO_UPDATE_NOTIFIER".into(), "1".into()),
        ("GH_NO_EXTENSION_UPDATE_NOTIFIER".into(), "1".into()),
        ("GH_PAGER".into(), "cat".into()),
        ("GH_BROWSER".into(), "false".into()),
        ("GH_TELEMETRY".into(), "false".into()),
        ("NO_COLOR".into(), "1".into()),
    ]);
    environment
}

#[cfg(any(windows, all(unix, not(target_os = "macos"))))]
fn environment_value<'a>(
    environment: &'a [(OsString, OsString)],
    name: &str,
) -> Option<&'a std::ffi::OsStr> {
    environment.iter().find_map(|(key, value)| {
        key.to_str()
            .is_some_and(|key| key.eq_ignore_ascii_case(name))
            .then_some(value.as_os_str())
    })
}

fn executable_candidates(environment: &[(OsString, OsString)]) -> Vec<PathBuf> {
    #[cfg(target_os = "macos")]
    let candidates = [
        "/opt/homebrew/bin/gh",
        "/usr/local/bin/gh",
        "/opt/local/bin/gh",
        "/usr/bin/gh",
    ]
    .into_iter()
    .map(PathBuf::from)
    .collect::<Vec<_>>();
    #[cfg(all(unix, not(target_os = "macos")))]
    let mut candidates = [
        "/usr/bin/gh",
        "/usr/local/bin/gh",
        "/snap/bin/gh",
        "/home/linuxbrew/.linuxbrew/bin/gh",
    ]
    .into_iter()
    .map(PathBuf::from)
    .collect::<Vec<_>>();
    #[cfg(all(unix, not(target_os = "macos")))]
    if let Some(home) = environment_value(environment, "HOME")
        .map(PathBuf::from)
        .filter(|home| home.is_absolute())
    {
        candidates.push(home.join(".local/bin/gh"));
        candidates.push(home.join(".linuxbrew/bin/gh"));
    }
    #[cfg(windows)]
    let mut candidates = Vec::new();
    #[cfg(windows)]
    for (name, suffixes) in [
        ("ProgramFiles", vec!["GitHub CLI/gh.exe"]),
        ("ProgramFiles(x86)", vec!["GitHub CLI/gh.exe"]),
        (
            "LOCALAPPDATA",
            vec![
                "Microsoft/WinGet/Links/gh.exe",
                "Programs/GitHub CLI/gh.exe",
            ],
        ),
        (
            "USERPROFILE",
            vec![
                "scoop/apps/gh/current/bin/gh.exe",
                "scoop/apps/gh/current/gh.exe",
            ],
        ),
    ] {
        if let Some(directory) = environment_value(environment, name)
            .map(PathBuf::from)
            .filter(|path| path.is_absolute())
        {
            candidates.extend(suffixes.into_iter().map(|suffix| directory.join(suffix)));
        }
    }
    #[cfg(target_os = "macos")]
    let _ = environment;
    candidates
}

fn neutral_cwd() -> PathBuf {
    #[cfg(unix)]
    {
        PathBuf::from("/")
    }
    #[cfg(windows)]
    {
        std::env::var_os("SystemRoot")
            .map(PathBuf::from)
            .filter(|path| path.is_absolute())
            .unwrap_or_else(|| PathBuf::from("C:\\Windows"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;

    struct FixtureRunner {
        results: std::sync::Mutex<VecDeque<Result<GithubCliOutput, GithubCliFailure>>>,
        calls: std::sync::Mutex<Vec<GithubCliCommand>>,
    }

    impl FixtureRunner {
        fn new(results: Vec<Result<GithubCliOutput, GithubCliFailure>>) -> Arc<Self> {
            Arc::new(Self {
                results: std::sync::Mutex::new(results.into()),
                calls: std::sync::Mutex::new(Vec::new()),
            })
        }
    }

    #[async_trait]
    impl GithubCliRunner for FixtureRunner {
        async fn run(
            &self,
            command: GithubCliCommand,
        ) -> Result<GithubCliOutput, GithubCliFailure> {
            self.calls.lock().unwrap().push(command);
            self.results
                .lock()
                .unwrap()
                .pop_front()
                .expect("fixture command")
        }
    }

    fn output(stdout: &str) -> Result<GithubCliOutput, GithubCliFailure> {
        Ok(GithubCliOutput {
            success: true,
            stdout: Zeroizing::new(stdout.as_bytes().to_vec()),
            stderr: Zeroizing::new(Vec::new()),
        })
    }

    fn metadata(login: &str) -> String {
        format!(r#"[{{"state":"success","active":true,"host":"github.com","login":"{login}"}}]"#)
    }

    fn only_fixture_account(discovery: GithubCliDiscovery) -> GithubCliAccount {
        let mut accounts = discovery.accounts.into_iter();
        let account = accounts
            .next()
            .expect("fixture discovery contains an account");
        assert!(
            accounts.next().is_none(),
            "fixture discovery contains exactly one account"
        );
        account
    }

    #[tokio::test]
    async fn discovery_returns_only_bounded_account_metadata_and_never_requests_a_token() {
        let runner = FixtureRunner::new(vec![
            output("--user string"),
            output(
                r#"[
                {"state":"success","active":false,"host":"github.com","login":"another"},
                {"state":"success","active":true,"host":"github.com","login":"Actor","token":"fixture_secret","error":"fixture_secret"},
                {"state":"success","active":false,"host":"github.com","login":"actor"},
                {"state":"success","active":true,"host":"other.example","login":"enterprise"},
                {"state":"error","active":false,"host":"github.com","login":"expired"},
                {"state":"timeout","active":false,"host":"github.com","login":"offline"},
                {"state":"success","active":false,"host":"github.com","login":"--user"},
                {"state":"success","active":false,"host":"github.com","login":"fixture_secret"}
            ]"#,
            ),
        ]);
        let cli = GithubCli::with_runner(runner.clone());
        let result = cli.discover().await;
        assert_eq!(result.status, GithubCliStatus::Available);
        assert_eq!(result.accounts.len(), 4);
        assert_eq!(result.accounts[0].login, "Actor");
        assert!(result.accounts[0].active);
        assert_eq!(
            result.accounts[0].availability,
            GithubCliAccountAvailability::Ready
        );
        assert!(
            result
                .accounts
                .iter()
                .any(|account| account.availability == GithubCliAccountAvailability::AuthRequired)
        );
        assert!(
            result
                .accounts
                .iter()
                .any(|account| account.availability == GithubCliAccountAvailability::Unavailable)
        );
        assert!(
            !serde_json::to_string(&result)
                .unwrap()
                .contains("fixture_secret")
        );
        assert_eq!(
            *runner.calls.lock().unwrap(),
            vec![GithubCliCommand::TokenHelp, GithubCliCommand::Discover]
        );
    }

    #[tokio::test]
    async fn import_uses_explicit_user_and_rechecks_current_metadata() {
        let runner = FixtureRunner::new(vec![
            output("--user string"),
            output(&metadata("Actor")),
            output("--user string"),
            output(&metadata("actor")),
            output("fixture_secret\n"),
        ]);
        let cli = GithubCli::with_runner(runner.clone());
        let candidate = only_fixture_account(cli.discover().await);
        let (login, token) = cli.import(&candidate.id).await.unwrap();
        assert_eq!(login, "Actor");
        assert_eq!(token.expose(), "fixture_secret");
        assert_eq!(
            runner.calls.lock().unwrap().last(),
            Some(&GithubCliCommand::Token {
                login: "Actor".into()
            })
        );
        assert_eq!(
            command_arguments(&GithubCliCommand::Token {
                login: "Actor".into()
            }),
            vec![
                "auth",
                "token",
                "--hostname",
                "github.com",
                "--user",
                "Actor"
            ]
        );
    }

    #[tokio::test]
    async fn unknown_expired_and_removed_candidates_never_request_a_token() {
        let runner = FixtureRunner::new(vec![
            output("--user string"),
            output(&metadata("actor")),
            output("--user string"),
            output("[]"),
        ]);
        let cli = GithubCli::with_runner(runner.clone());
        assert!(
            matches!(cli.import("--user").await, Err(error) if error.code == ErrorCode::StaleView)
        );
        assert!(runner.calls.lock().unwrap().is_empty());
        let candidate = only_fixture_account(cli.discover().await);
        let current = cli
            .candidates
            .lock()
            .await
            .get(&candidate.id)
            .unwrap()
            .clone();
        cli.candidates
            .lock()
            .await
            .get_mut(&candidate.id)
            .unwrap()
            .discovered_at = Instant::now() - CANDIDATE_TTL;
        assert!(
            matches!(cli.import(&candidate.id).await, Err(error) if error.code == ErrorCode::StaleView)
        );
        cli.candidates
            .lock()
            .await
            .insert(candidate.id.clone(), current);
        assert!(
            matches!(cli.import(&candidate.id).await, Err(error) if error.code == ErrorCode::StaleView)
        );
        assert!(
            !runner
                .calls
                .lock()
                .unwrap()
                .iter()
                .any(|call| matches!(call, GithubCliCommand::Token { .. }))
        );
    }

    #[tokio::test]
    async fn missing_old_and_invalid_cli_are_metadata_states_without_raw_diagnostics() {
        let missing = GithubCli::disabled().discover().await;
        assert_eq!(missing.status, GithubCliStatus::NotInstalled);
        let old = GithubCli::with_runner(FixtureRunner::new(vec![output("old token command")]));
        assert_eq!(old.discover().await.status, GithubCliStatus::Unsupported);
        let failed = GithubCli::with_runner(FixtureRunner::new(vec![
            output("--user string"),
            Ok(GithubCliOutput {
                success: false,
                stdout: Zeroizing::new(Vec::new()),
                stderr: Zeroizing::new(b"unknown flag: --json fixture_secret".to_vec()),
            }),
        ]));
        let result = failed.discover().await;
        assert_eq!(result.status, GithubCliStatus::Unsupported);
        assert!(
            !serde_json::to_string(&result)
                .unwrap()
                .contains("fixture_secret")
        );
        assert!(matches!(
            parse_metadata(b"invalid fixture_secret"),
            Err(GithubCliFailure::Unavailable)
        ));
        assert!(matches!(
            parse_metadata(&vec![b' '; MAX_METADATA_BYTES + 1]),
            Err(GithubCliFailure::Unavailable)
        ));
    }

    #[test]
    fn token_output_is_exactly_one_bounded_ascii_token() {
        assert_eq!(
            parse_token(b"fixture_secret\r\n").unwrap().expose(),
            "fixture_secret"
        );
        for invalid in [
            b" fixture_secret\n".as_slice(),
            b"fixture_secret \n",
            b"fixture_secret\n\n",
            b"first\nsecond",
            b"",
            b"non\xc3\xa9ascii",
        ] {
            assert!(
                matches!(parse_token(invalid), Err(error) if error.code == ErrorCode::AuthRequired && !error.message.contains("fixture_secret"))
            );
        }
        assert!(parse_token(&vec![b'a'; 4097]).is_err());
    }

    #[test]
    fn process_environment_excludes_tokens_debug_hooks_and_repository_path() {
        let input = [
            ("GH_TOKEN", "fixture_secret"),
            ("GITHUB_TOKEN", "fixture_secret"),
            ("GH_ENTERPRISE_TOKEN", "fixture_secret"),
            ("GITHUB_ENTERPRISE_TOKEN", "fixture_secret"),
            ("GH_DEBUG", "api"),
            ("DEBUG", "1"),
            ("GH_HOST", "evil.example"),
            ("PATH", ".:/tmp/repository/bin"),
            ("LD_PRELOAD", "evil.so"),
            ("HOME", "/tmp/isolated-fixture"),
            ("GH_CONFIG_DIR", "/tmp/isolated-fixture/config"),
        ]
        .into_iter()
        .map(|(key, value)| (key.into(), value.into()))
        .collect();
        let environment = allowed_environment(input);
        assert!(
            !environment
                .iter()
                .any(|(_, value)| value == "fixture_secret")
        );
        for (key, _) in &environment {
            assert!(
                !["GH_DEBUG", "DEBUG", "GH_HOST", "PATH", "LD_PRELOAD"]
                    .iter()
                    .any(|denied| key == denied)
            );
        }
        assert!(environment.contains(&("GH_PROMPT_DISABLED".into(), "1".into())));
        let arguments = command_arguments(&GithubCliCommand::Discover);
        assert!(
            !arguments
                .iter()
                .any(|argument| argument.contains("show-token"))
        );
        assert!(arguments.contains(&"--jq"));
        assert!(
            executable_candidates(&[])
                .iter()
                .all(|path| path.is_absolute())
        );
        #[cfg(target_os = "macos")]
        assert!(executable_candidates(&[]).contains(&PathBuf::from("/opt/homebrew/bin/gh")));
    }

    #[cfg(unix)]
    fn fixture_process(script: &str, timeout: Duration) -> (tempfile::TempDir, NativeRunner) {
        use std::os::unix::fs::PermissionsExt;
        let directory = tempfile::tempdir().unwrap();
        let executable = directory.path().join("gh-fixture");
        std::fs::write(&executable, format!("#!/bin/sh\n{script}\n")).unwrap();
        std::fs::set_permissions(&executable, std::fs::Permissions::from_mode(0o700)).unwrap();
        let environment = allowed_environment(vec![
            ("GH_TOKEN".into(), "fixture_secret".into()),
            ("HOME".into(), directory.path().as_os_str().to_owned()),
        ]);
        (
            directory,
            NativeRunner {
                candidates: vec![executable],
                environment,
                cwd: neutral_cwd(),
                timeout,
            },
        )
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn native_discovery_observes_new_install_and_upgraded_symlink_without_changing_choice() {
        use std::os::unix::{fs::PermissionsExt, fs::symlink};
        let directory = tempfile::tempdir().unwrap();
        let installation = directory.path().join("gh");
        let runner = Arc::new(NativeRunner {
            candidates: vec![installation.clone()],
            environment: allowed_environment(vec![(
                "HOME".into(),
                directory.path().as_os_str().to_owned(),
            )]),
            cwd: neutral_cwd(),
            timeout: COMMAND_TIMEOUT,
        });
        let cli = GithubCli::with_runner(runner.clone());
        assert_eq!(cli.discover().await.status, GithubCliStatus::NotInstalled);
        let write_version = |name: &str, login: &str, token: &str| {
            let executable = directory.path().join(name);
            let script = format!(
                "#!/bin/sh\nif [ \"$3\" = --help ]; then printf '%s\\n' '--user string'; elif [ \"$2\" = status ]; then printf '%s\\n' '{}'; elif [ \"$2\" = token ]; then printf '%s\\n' '{token}'; else exit 9; fi\n",
                metadata(login)
            );
            std::fs::write(&executable, script).unwrap();
            std::fs::set_permissions(&executable, std::fs::Permissions::from_mode(0o700)).unwrap();
            executable
        };
        let first_version = write_version("first-version-gh", "actor", "first_fixture_secret");
        symlink(&first_version, &installation).unwrap();
        let original = only_fixture_account(cli.discover().await);
        assert_eq!(original.login, "actor");
        let second_version = write_version("second-version-gh", "another", "second_fixture_secret");
        std::fs::remove_file(&installation).unwrap();
        symlink(&second_version, &installation).unwrap();
        std::fs::remove_file(first_version).unwrap();
        assert!(
            matches!(cli.import(&original.id).await, Err(error) if error.code == ErrorCode::StaleView)
        );
        assert_eq!(
            resolve_executable(&runner.candidates),
            Some(second_version.canonicalize().unwrap())
        );
        let current = only_fixture_account(cli.discover().await);
        assert_eq!(current.login, "another");
        let (login, token) = cli.import(&current.id).await.unwrap();
        assert_eq!(login, "another");
        assert_eq!(token.expose(), "second_fixture_secret");
        std::fs::remove_file(installation).unwrap();
        assert_eq!(cli.discover().await.status, GithubCliStatus::NotInstalled);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn native_child_has_no_environment_token_and_cannot_read_stdin() {
        let (_directory, runner) = fixture_process(
            "if read value; then exit 8; fi\nprintf '%s\\n' \"${GH_TOKEN-unset}\"\nprintf '%s\\n' \"${GH_PROMPT_DISABLED-unset}\"\n/bin/pwd",
            COMMAND_TIMEOUT,
        );
        let output = runner.run(GithubCliCommand::Discover).await.unwrap();
        assert!(output.success);
        assert_eq!(&*output.stdout, b"unset\n1\n/\n");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn native_hung_and_flooding_children_are_bounded_and_redacted() {
        let (_directory, runner) = fixture_process("exec /bin/sleep 5", Duration::from_millis(30));
        let started = Instant::now();
        assert!(matches!(
            runner.run(GithubCliCommand::Discover).await,
            Err(GithubCliFailure::Unavailable)
        ));
        assert!(started.elapsed() < Duration::from_secs(2));
        let (_directory, runner) = fixture_process(
            "while :; do printf 'fixture_secret_fixture_secret_fixture_secret\\n'; done",
            COMMAND_TIMEOUT,
        );
        assert!(matches!(
            runner
                .run(GithubCliCommand::Token {
                    login: "actor".into()
                })
                .await,
            Err(GithubCliFailure::Unavailable)
        ));
    }
}
