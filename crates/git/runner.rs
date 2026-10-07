use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::{
    Arc, Mutex as StdMutex, OnceLock,
    atomic::{AtomicBool, Ordering},
};
use std::time::{Duration, Instant};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::sync::{Mutex as AsyncMutex, OwnedMutexGuard};
use tokio::time::{sleep, timeout};

#[derive(Clone, Copy)]
pub struct GitRunOptions {
    pub timeout: Duration,
    pub allow_failure_codes: &'static [i32],
    pub local_only: bool,
}

impl GitRunOptions {
    pub fn default_read() -> Self {
        Self {
            timeout: Duration::from_secs(30),
            allow_failure_codes: &[],
            local_only: false,
        }
    }

    /// Prevent an otherwise read-only Git command from consulting replacement
    /// refs or lazily fetching missing objects from a promisor remote.
    pub fn local_only_read() -> Self {
        Self {
            local_only: true,
            ..Self::default_read()
        }
    }

    pub fn allow_exit_codes(mut self, codes: &'static [i32]) -> Self {
        self.allow_failure_codes = codes;
        self
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }
}

#[derive(Clone)]
pub struct GitCommandRunner {
    repo_path: PathBuf,
}

/// A sequence of Git commands executed while holding the runner's per-repository lock.
///
/// Use the transaction methods rather than calling [`GitCommandRunner`] again while this
/// value is alive; the regular runner methods would try to acquire the same lock again.
pub struct GitCommandTransaction {
    repo_path: PathBuf,
    _guard: OwnedMutexGuard<()>,
}

/// A prepared `git update-ref --stdin` transaction holding one exact ref lock.
/// Dropping it closes stdin so Git aborts and releases the lock; callers should
/// use `release` to wait for clean release on ordinary paths.
pub(crate) struct PreparedGitRefLock {
    child: Option<tokio::process::Child>,
    stdin: Option<tokio::process::ChildStdin>,
    _stdout: Option<BufReader<tokio::process::ChildStdout>>,
}

impl PreparedGitRefLock {
    pub(crate) async fn release(mut self) {
        if let Some(mut stdin) = self.stdin.take() {
            let _ = stdin.write_all(b"abort\n").await;
            let _ = stdin.shutdown().await;
        }
        if let Some(child) = self.child.take() {
            finish_ref_lock_child(child).await;
        }
    }
}

async fn finish_ref_lock_child(mut child: tokio::process::Child) {
    if timeout(Duration::from_secs(5), child.wait()).await.is_err() {
        let _ = child.kill().await;
        let _ = child.wait().await;
    }
}

// TODO(ruru-m07): Consider allowing configuration of additional tool directories via environment variable or config file
const DEFAULT_TOOL_DIRS: &[&str] = &[
    "/opt/homebrew/bin",
    "/usr/local/bin",
    "/usr/bin",
    "/bin",
    "/usr/sbin",
    "/sbin",
];
const SENSITIVE_SSH_COMMAND: &str = "ssh -oBatchMode=yes";

#[derive(Clone, Copy)]
pub(crate) enum SensitiveRemoteProtocol {
    Https,
    Ssh,
}

impl SensitiveRemoteProtocol {
    fn as_str(self) -> &'static str {
        match self {
            Self::Https => "https",
            Self::Ssh => "ssh",
        }
    }
}

impl GitCommandRunner {
    pub fn new(repo_path: &str) -> Result<Self, String> {
        let path = Path::new(repo_path);
        if !path.is_dir() {
            return Err(format!("Invalid repository path: {repo_path}"));
        }
        Ok(Self {
            repo_path: path.to_path_buf(),
        })
    }

    pub async fn run_with_options(
        &self,
        args: &[&str],
        options: GitRunOptions,
    ) -> Result<String, String> {
        run_git_command_async(&self.repo_path, args, None, options, &[]).await
    }

    /// Hold the same per-repository lock used by the regular runner methods across
    /// multiple commands that must be observed as one Gitru-internal operation.
    pub async fn transaction(&self) -> Result<GitCommandTransaction, String> {
        let repo_lock = command_lock_for_repo(&self.repo_path)?;
        let guard = repo_lock.lock_owned().await;
        Ok(GitCommandTransaction {
            repo_path: self.repo_path.clone(),
            _guard: guard,
        })
    }

    /// Like [`Self::run_with_options`], but sets extra process environment variables.
    pub async fn run_with_env(
        &self,
        args: &[&str],
        options: GitRunOptions,
        env: &[(&str, &str)],
    ) -> Result<String, String> {
        run_git_command_async(&self.repo_path, args, None, options, env).await
    }

    pub async fn run_with_options_unlocked(
        &self,
        args: &[&str],
        options: GitRunOptions,
    ) -> Result<String, String> {
        run_git_command_async_unlocked(&self.repo_path, args, None, options, &[]).await
    }

    pub async fn run_with_input(
        &self,
        args: &[&str],
        input: &str,
        options: GitRunOptions,
    ) -> Result<String, String> {
        run_git_command_async(&self.repo_path, args, Some(input.as_bytes()), options, &[]).await
    }

    pub async fn run_with_options_bytes(
        &self,
        args: &[&str],
        options: GitRunOptions,
    ) -> Result<Vec<u8>, String> {
        run_git_command_bytes_async(&self.repo_path, args, None, options, &[]).await
    }

    pub async fn run_with_options_bytes_unlocked(
        &self,
        args: &[&str],
        options: GitRunOptions,
    ) -> Result<Vec<u8>, String> {
        run_git_command_bytes_async_unlocked(&self.repo_path, args, None, options, &[]).await
    }

    pub async fn run_streaming<F>(
        &self,
        args: &[&str],
        options: GitRunOptions,
        cancel_flag: Arc<AtomicBool>,
        on_line: F,
    ) -> Result<i32, String>
    where
        F: FnMut(&str) -> bool,
    {
        run_git_command_streaming(&self.repo_path, args, options, cancel_flag, on_line).await
    }
}

impl GitCommandTransaction {
    /// Atomically verify and hold a direct ref at an exact object ID while
    /// subsequent commands run through this Gitru transaction. The child
    /// remains at update-ref's prepared phase until the returned guard is
    /// released, so ordinary external Git processes cannot move the ref.
    pub(crate) async fn prepare_ref_lock(
        &mut self,
        reference: &str,
        expected_oid: &str,
        duration: Duration,
    ) -> Result<PreparedGitRefLock, String> {
        if !reference.starts_with("refs/heads/")
            || reference.chars().any(char::is_whitespace)
            || !matches!(expected_oid.len(), 40 | 64)
            || !expected_oid.bytes().all(|byte| byte.is_ascii_hexdigit())
        {
            return Err("Invalid Git ref lock request".into());
        }
        let binary = git_binary_path()?;
        let path = git_path_env()?;
        let mut command = tokio::process::Command::new(binary);
        command
            .current_dir(&self.repo_path)
            .args([
                "-c",
                "core.askPass=",
                "-c",
                "core.hooksPath=",
                "-c",
                "core.filesRefLockTimeout=0",
                "-c",
                "core.packedRefsTimeout=0",
                "-c",
                "reftable.lockTimeout=0",
                "update-ref",
                "--stdin",
            ])
            .env("PATH", path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        configure_sensitive_command(&mut command, std::env::vars_os().map(|(key, _)| key));
        // Do not kill on drop: closing the pipe lets Git abort its prepared
        // transaction and remove the lock even if the caller is cancelled.
        let mut child = command.spawn().map_err(|_| "Git ref lock unavailable")?;
        let mut stdin = child.stdin.take().ok_or("Git ref lock unavailable")?;
        let stdout = child.stdout.take().ok_or("Git ref lock unavailable")?;
        let mut stdout = BufReader::new(stdout);
        let protocol = async {
            stdin
                .write_all(
                    format!("start\noption no-deref\nverify {reference} {expected_oid}\nprepare\n")
                        .as_bytes(),
                )
                .await
                .map_err(|_| "Git ref lock unavailable")?;
            stdin
                .flush()
                .await
                .map_err(|_| "Git ref lock unavailable")?;
            let mut line = String::new();
            if stdout
                .read_line(&mut line)
                .await
                .map_err(|_| "Git ref lock unavailable")?
                == 0
                || line.trim_end_matches(['\r', '\n']) != "start: ok"
            {
                return Err("Git ref changed");
            }
            line.clear();
            if stdout
                .read_line(&mut line)
                .await
                .map_err(|_| "Git ref lock unavailable")?
                == 0
                || line.trim_end_matches(['\r', '\n']) != "prepare: ok"
            {
                return Err("Git ref changed");
            }
            Ok(())
        };
        match timeout(duration, protocol).await {
            Ok(Ok(())) => {}
            Ok(Err(error)) => {
                drop(stdin);
                finish_ref_lock_child(child).await;
                return Err(error.into());
            }
            Err(_) => {
                drop(stdin);
                finish_ref_lock_child(child).await;
                return Err("Git ref lock timed out".into());
            }
        }
        if let Some(status) = child.try_wait().map_err(|_| "Git ref lock unavailable")? {
            drop(stdin);
            return Err(if status.success() {
                "Git ref lock ended"
            } else {
                "Git ref changed"
            }
            .into());
        }
        Ok(PreparedGitRefLock {
            child: Some(child),
            stdin: Some(stdin),
            _stdout: Some(stdout),
        })
    }

    /// Bounded local metadata read. Raw diagnostics are discarded before return.
    /// This method intentionally does not share the ordinary stderr fallback.
    pub(crate) async fn sensitive_read(
        &mut self,
        args: &[&str],
        maximum: usize,
    ) -> Result<(Vec<u8>, i32), crate::models::remotes::RemoteObservationError> {
        self.sensitive_read_inner(args, maximum, None, false).await
    }

    /// Bounded local-only metadata/content read. In addition to the sensitive
    /// command policy, this forbids every Git transport and disables pagers,
    /// system attributes and lazy object hydration. Callers must still pass
    /// command-specific defenses such as `--no-ext-diff` and `--no-textconv`.
    pub(crate) async fn sensitive_local_read(
        &mut self,
        args: &[&str],
        maximum: usize,
    ) -> Result<(Vec<u8>, i32), crate::models::remotes::RemoteObservationError> {
        self.sensitive_read_inner(args, maximum, None, true).await
    }

    /// Run a sensitive command with one private command-scope Git config
    /// value. The value travels through `GIT_CONFIG_VALUE_0`, so it never
    /// appears in argv or diagnostics.
    pub(crate) async fn sensitive_read_with_config(
        &mut self,
        args: &[&str],
        maximum: usize,
        key: &str,
        value: &str,
        protocol: SensitiveRemoteProtocol,
    ) -> Result<(Vec<u8>, i32), crate::models::remotes::RemoteObservationError> {
        self.sensitive_read_inner(args, maximum, Some((key, value, protocol)), false)
            .await
    }

    async fn sensitive_read_inner(
        &mut self,
        args: &[&str],
        maximum: usize,
        private_config: Option<(&str, &str, SensitiveRemoteProtocol)>,
        local_only: bool,
    ) -> Result<(Vec<u8>, i32), crate::models::remotes::RemoteObservationError> {
        use crate::models::remotes::RemoteObservationError as Error;
        let binary = git_binary_path().map_err(|_| Error::Unavailable)?;
        let path = git_path_env().map_err(|_| Error::Unavailable)?;
        let mut command = tokio::process::Command::new(binary);
        command
            .current_dir(&self.repo_path)
            // Keep configured noninteractive credential helpers available, but
            // suppress both environment and repository-configured askpass UIs.
            .args(["-c", "core.askPass="])
            .args(args)
            .env("PATH", path)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        configure_sensitive_command(&mut command, std::env::vars_os().map(|(key, _)| key));
        if local_only {
            configure_local_only_read(&mut command);
        }
        if let Some((key, value, protocol)) = private_config {
            configure_private_transport(&mut command, key, value, protocol);
        }
        let mut child = command
            .kill_on_drop(true)
            .spawn()
            .map_err(|_| Error::Unavailable)?;
        let stdout = child.stdout.take().ok_or(Error::Unavailable)?;
        let mut bytes = Vec::new();
        let read = async {
            stdout
                .take(maximum as u64 + 1)
                .read_to_end(&mut bytes)
                .await
                .map_err(|_| Error::Unavailable)?;
            if bytes.len() > maximum {
                return Err(Error::LimitExceeded);
            }
            Ok(())
        };
        let wait = async { child.wait().await.map_err(|_| Error::Unavailable) };
        let (_, status) = tokio::try_join!(read, wait)?;
        Ok((bytes, status.code().ok_or(Error::Unavailable)?))
    }

    pub async fn run_with_options(
        &mut self,
        args: &[&str],
        options: GitRunOptions,
    ) -> Result<String, String> {
        run_git_command_async_unlocked(&self.repo_path, args, None, options, &[]).await
    }

    pub async fn run_with_input(
        &mut self,
        args: &[&str],
        input: &str,
        options: GitRunOptions,
    ) -> Result<String, String> {
        run_git_command_async_unlocked(&self.repo_path, args, Some(input.as_bytes()), options, &[])
            .await
    }

    pub async fn run_with_env(
        &mut self,
        args: &[&str],
        options: GitRunOptions,
        env: &[(&str, &str)],
    ) -> Result<String, String> {
        run_git_command_async_unlocked(&self.repo_path, args, None, options, env).await
    }
}

fn configure_sensitive_command(
    command: &mut tokio::process::Command,
    inherited_keys: impl IntoIterator<Item = OsString>,
) {
    // An explicit empty override wins over inherited variables and
    // repository/system `core.askPass` configuration. Removing the variables
    // is insufficient because Git would then fall back to configured helpers.
    command.env("GIT_ASKPASS", "");
    command.env("SSH_ASKPASS", "");
    for key in inherited_keys {
        if key.to_str().is_some_and(|key| {
            let upper = key.to_ascii_uppercase();
            upper.starts_with("GIT_TRACE")
                || upper == "GIT_CURL_VERBOSE"
                || upper == "SSLKEYLOGFILE"
                || matches!(
                    upper.as_str(),
                    "GIT_SSH" | "GIT_SSH_COMMAND" | "GIT_SSH_VARIANT"
                )
                || upper == "GIT_CONFIG_COUNT"
                || upper.starts_with("GIT_CONFIG_KEY_")
                || upper.starts_with("GIT_CONFIG_VALUE_")
                || upper == "GIT_CONFIG_PARAMETERS"
                || upper == "GIT_EXEC_PATH"
                || upper == "GIT_ALLOW_PROTOCOL"
                || upper == "GIT_PROTOCOL_FROM_USER"
                || upper == "GIT_EXTERNAL_DIFF"
                || upper == "GIT_DIFF_OPTS"
                || upper == "GIT_PAGER"
                || upper == "GIT_ATTR_NOSYSTEM"
                || upper == "GIT_CONFIG_NOSYSTEM"
                || upper == "GIT_LITERAL_PATHSPECS"
                || matches!(
                    upper.as_str(),
                    "GIT_DIR"
                        | "GIT_WORK_TREE"
                        | "GIT_COMMON_DIR"
                        | "GIT_INDEX_FILE"
                        | "GIT_OBJECT_DIRECTORY"
                        | "GIT_ALTERNATE_OBJECT_DIRECTORIES"
                        | "GIT_NAMESPACE"
                        | "GIT_REPLACE_REF_BASE"
                        | "GIT_NO_REPLACE_OBJECTS"
                        | "GIT_NO_LAZY_FETCH"
                        | "GIT_GRAFT_FILE"
                        | "GIT_SHALLOW_FILE"
                        | "GIT_QUARANTINE_PATH"
                )
        }) {
            command.env_remove(key);
        }
    }
    // Credential helpers may still satisfy a fetch non-interactively, but Git,
    // Git Credential Manager, and SSH must never open a prompt for this action.
    command.env("GIT_TERMINAL_PROMPT", "0");
    command.env("GCM_INTERACTIVE", "never");
    command.env("SSH_ASKPASS_REQUIRE", "never");
    command.env("GIT_NO_REPLACE_OBJECTS", "1");
    command.env("GIT_NO_LAZY_FETCH", "1");
    // Use only the OpenSSH client resolved through the runner's curated PATH.
    // This fixed command bypasses inherited and repository-configured shell
    // commands while retaining normal OpenSSH config, agent and key discovery.
    command.env("GIT_SSH_COMMAND", SENSITIVE_SSH_COMMAND);
    command.env("GIT_SSH_VARIANT", "ssh");
}

fn configure_local_only_read(command: &mut tokio::process::Command) {
    command.env("GIT_ALLOW_PROTOCOL", "");
    command.env("GIT_PROTOCOL_FROM_USER", "0");
    command.env("GIT_NO_LAZY_FETCH", "1");
    command.env("GIT_NO_REPLACE_OBJECTS", "1");
    command.env("GIT_ATTR_NOSYSTEM", "1");
    command.env("GIT_CONFIG_NOSYSTEM", "1");
    command.env("GIT_LITERAL_PATHSPECS", "1");
    command.env(
        "GIT_GRAFT_FILE",
        if cfg!(windows) { "NUL" } else { "/dev/null" },
    );
    command.env("GIT_PAGER", "cat");
    command.env("PAGER", "cat");
    command.env("LC_ALL", "C");
}

fn configure_private_transport(
    command: &mut tokio::process::Command,
    key: &str,
    value: &str,
    protocol: SensitiveRemoteProtocol,
) {
    command.env("GIT_CONFIG_COUNT", "1");
    command.env("GIT_CONFIG_KEY_0", key);
    command.env("GIT_CONFIG_VALUE_0", value);
    command.env("GIT_ALLOW_PROTOCOL", protocol.as_str());
    command.env("GIT_PROTOCOL_FROM_USER", "0");
}

pub(crate) fn git_binary_path() -> Result<PathBuf, String> {
    resolve_program_path("git")
}

pub(crate) fn git_path_env() -> Result<OsString, String> {
    let search_dirs = preferred_tool_dirs();
    join_paths(&search_dirs)
}

async fn run_git_command_async(
    repo_path: &Path,
    args: &[&str],
    input: Option<&[u8]>,
    options: GitRunOptions,
    env: &[(&str, &str)],
) -> Result<String, String> {
    let repo_lock = command_lock_for_repo(repo_path)?;
    let _guard = repo_lock.lock().await;

    let mut attempt: u32 = 0;
    const MAX_INDEX_LOCK_RETRIES: u32 = 6;

    loop {
        attempt += 1;
        match run_git_command_once_output(repo_path, args, input, options, env).await {
            Ok(output) => return finalize_output(output, options.allow_failure_codes),
            Err(err) if is_index_lock_error(&err) && attempt < MAX_INDEX_LOCK_RETRIES => {
                let backoff_ms = 50 * attempt as u64;
                sleep(Duration::from_millis(backoff_ms)).await;
            }
            Err(err) => return Err(err),
        }
    }
}

async fn run_git_command_async_unlocked(
    repo_path: &Path,
    args: &[&str],
    input: Option<&[u8]>,
    options: GitRunOptions,
    env: &[(&str, &str)],
) -> Result<String, String> {
    let mut attempt: u32 = 0;
    const MAX_INDEX_LOCK_RETRIES: u32 = 6;

    loop {
        attempt += 1;
        match run_git_command_once_output(repo_path, args, input, options, env).await {
            Ok(output) => return finalize_output(output, options.allow_failure_codes),
            Err(err) if is_index_lock_error(&err) && attempt < MAX_INDEX_LOCK_RETRIES => {
                let backoff_ms = 50 * attempt as u64;
                sleep(Duration::from_millis(backoff_ms)).await;
            }
            Err(err) => return Err(err),
        }
    }
}

async fn run_git_command_bytes_async(
    repo_path: &Path,
    args: &[&str],
    input: Option<&[u8]>,
    options: GitRunOptions,
    env: &[(&str, &str)],
) -> Result<Vec<u8>, String> {
    let repo_lock = command_lock_for_repo(repo_path)?;
    let _guard = repo_lock.lock().await;

    let mut attempt: u32 = 0;
    const MAX_INDEX_LOCK_RETRIES: u32 = 6;

    loop {
        attempt += 1;
        match run_git_command_once_output(repo_path, args, input, options, env).await {
            Ok(output) => return finalize_output_bytes(output, options.allow_failure_codes),
            Err(err) if is_index_lock_error(&err) && attempt < MAX_INDEX_LOCK_RETRIES => {
                let backoff_ms = 50 * attempt as u64;
                sleep(Duration::from_millis(backoff_ms)).await;
            }
            Err(err) => return Err(err),
        }
    }
}

async fn run_git_command_bytes_async_unlocked(
    repo_path: &Path,
    args: &[&str],
    input: Option<&[u8]>,
    options: GitRunOptions,
    env: &[(&str, &str)],
) -> Result<Vec<u8>, String> {
    let mut attempt: u32 = 0;
    const MAX_INDEX_LOCK_RETRIES: u32 = 6;

    loop {
        attempt += 1;
        match run_git_command_once_output(repo_path, args, input, options, env).await {
            Ok(output) => return finalize_output_bytes(output, options.allow_failure_codes),
            Err(err) if is_index_lock_error(&err) && attempt < MAX_INDEX_LOCK_RETRIES => {
                let backoff_ms = 50 * attempt as u64;
                sleep(Duration::from_millis(backoff_ms)).await;
            }
            Err(err) => return Err(err),
        }
    }
}

async fn run_git_command_once_output(
    repo_path: &Path,
    args: &[&str],
    input: Option<&[u8]>,
    options: GitRunOptions,
    env: &[(&str, &str)],
) -> Result<std::process::Output, String> {
    let git_binary = git_binary_path()?;
    let git_path_env = git_path_env()?;
    let mut command = tokio::process::Command::new(git_binary);
    command.current_dir(repo_path);
    command.env("PATH", git_path_env);
    for (key, value) in env {
        command.env(key, value);
    }
    if options.local_only {
        command.env("GIT_NO_LAZY_FETCH", "1");
        command.env("GIT_NO_REPLACE_OBJECTS", "1");
    }
    command.args(args);
    command.stdin(if input.is_some() {
        Stdio::piped()
    } else {
        Stdio::null()
    });
    command.stdout(Stdio::piped());
    command.stderr(Stdio::piped());

    let mut child = command
        .kill_on_drop(true)
        .spawn()
        .map_err(|e| e.to_string())?;

    if let Some(payload) = input
        && let Some(mut stdin) = child.stdin.take()
    {
        stdin.write_all(payload).await.map_err(|e| e.to_string())?;
        drop(stdin);
    }

    match timeout(options.timeout, child.wait_with_output()).await {
        Ok(Ok(output)) => Ok(output),
        Ok(Err(e)) => Err(e.to_string()),
        Err(_) => {
            // Timeout - tokio's timeout drops the future, which kills the process
            Err("Git command timed out".to_string())
        }
    }
}

fn command_lock_for_repo(repo_path: &Path) -> Result<std::sync::Arc<AsyncMutex<()>>, String> {
    static REPO_LOCKS: OnceLock<
        StdMutex<std::collections::HashMap<String, std::sync::Arc<AsyncMutex<()>>>>,
    > = OnceLock::new();

    let locks = REPO_LOCKS.get_or_init(|| StdMutex::new(std::collections::HashMap::new()));
    let mut guard = locks
        .lock()
        .map_err(|_| "Failed to lock command map".to_string())?;

    let key = repo_path.to_string_lossy().to_string();
    Ok(guard
        .entry(key)
        .or_insert_with(|| std::sync::Arc::new(AsyncMutex::new(())))
        .clone())
}

fn preferred_tool_dirs() -> Vec<PathBuf> {
    preferred_tool_dirs_with_path_dirs(path_dirs_from_env())
}

fn preferred_tool_dirs_with_path_dirs(
    path_dirs: impl IntoIterator<Item = PathBuf>,
) -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    let mut seen = std::collections::HashSet::new();

    for dir in DEFAULT_TOOL_DIRS.iter().map(PathBuf::from).chain(path_dirs) {
        if seen.insert(dir.clone()) {
            dirs.push(dir);
        }
    }

    dirs
}

fn path_dirs_from_env() -> Vec<PathBuf> {
    std::env::var_os("PATH")
        .map(|value| std::env::split_paths(&value).collect())
        .unwrap_or_default()
}

fn resolve_program_path(program: &str) -> Result<PathBuf, String> {
    resolve_program_path_from_dirs(program, &preferred_tool_dirs())
        .ok_or_else(|| format!("Unable to locate `{program}` executable"))
}

fn resolve_program_path_from_dirs(program: &str, dirs: &[PathBuf]) -> Option<PathBuf> {
    for dir in dirs {
        for candidate in executable_candidates(dir, program) {
            if is_executable_file(&candidate) {
                return Some(candidate);
            }
        }
    }

    None
}

fn executable_candidates(dir: &Path, program: &str) -> Vec<PathBuf> {
    #[cfg(target_os = "windows")]
    let candidate_names = vec![
        program.to_string(),
        format!("{program}.exe"),
        format!("{program}.cmd"),
        format!("{program}.bat"),
    ];
    #[cfg(not(target_os = "windows"))]
    let candidate_names = [program.to_string()];

    candidate_names.iter().map(|name| dir.join(name)).collect()
}

#[cfg(target_os = "windows")]
fn is_executable_file(path: &Path) -> bool {
    path.is_file()
}

#[cfg(not(target_os = "windows"))]
fn is_executable_file(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;

    let Ok(metadata) = path.metadata() else {
        return false;
    };

    if !metadata.is_file() {
        return false;
    }

    metadata.permissions().mode() & 0o111 != 0
}

fn join_paths(paths: &[PathBuf]) -> Result<OsString, String> {
    std::env::join_paths(paths).map_err(|e| e.to_string())
}

fn is_index_lock_error(err: &str) -> bool {
    err.contains("index.lock") && err.contains("File exists")
}

pub fn validate_relative_path(path: &str) -> Result<(), String> {
    let path = Path::new(path);
    if path.is_absolute() {
        return Err("Absolute paths are not allowed".to_string());
    }
    for component in path.components() {
        match component {
            std::path::Component::ParentDir => {
                return Err("Path traversal is not allowed".to_string());
            }
            std::path::Component::Normal(name) if name == ".git" => {
                return Err("Access to .git is not allowed".to_string());
            }
            _ => {}
        }
    }
    Ok(())
}

pub fn throttle_command(repo_key: &str, min_interval: Duration) -> Result<(), String> {
    static THROTTLE: OnceLock<StdMutex<std::collections::HashMap<String, Instant>>> =
        OnceLock::new();
    let throttle = THROTTLE.get_or_init(|| StdMutex::new(std::collections::HashMap::new()));

    let mut map = throttle
        .lock()
        .map_err(|_| "Failed to lock command throttle".to_string())?;

    let now = Instant::now();

    map.retain(|_, last| now.duration_since(*last) < Duration::from_secs(3600));

    if let Some(last) = map.get(repo_key)
        && now.duration_since(*last) < min_interval
    {
        return Err("Command throttled to protect performance".to_string());
    }

    map.insert(repo_key.to_string(), now);
    Ok(())
}

fn finalize_output(
    output: std::process::Output,
    allow_failure_codes: &[i32],
) -> Result<String, String> {
    let is_allowed = output
        .status
        .code()
        .map(|code| allow_failure_codes.contains(&code))
        .unwrap_or(false);

    if output.status.success() || is_allowed {
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();

        if stdout.is_empty() {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            if !stderr.is_empty() {
                Ok(stderr)
            } else {
                Ok(String::new())
            }
        } else {
            Ok(stdout)
        }
    } else {
        let stderr_lossy = String::from_utf8_lossy(&output.stderr);
        let stderr = stderr_lossy.trim();
        if !stderr.is_empty() {
            Err(stderr.to_string())
        } else {
            Err(format!(
                "Command failed with exit code: {:?}",
                output.status.code()
            ))
        }
    }
}

fn finalize_output_bytes(
    output: std::process::Output,
    allow_failure_codes: &[i32],
) -> Result<Vec<u8>, String> {
    let is_allowed = output
        .status
        .code()
        .map(|code| allow_failure_codes.contains(&code))
        .unwrap_or(false);

    if output.status.success() {
        if output.stdout.is_empty() && !output.stderr.is_empty() {
            Ok(output.stderr)
        } else {
            Ok(output.stdout)
        }
    } else if is_allowed {
        Ok(output.stdout)
    } else {
        let stderr_lossy = String::from_utf8_lossy(&output.stderr);
        let stderr = stderr_lossy.trim();
        if !stderr.is_empty() {
            Err(stderr.to_string())
        } else {
            Err(format!(
                "Command failed with exit code: {:?}",
                output.status.code()
            ))
        }
    }
}

async fn run_git_command_streaming<F>(
    repo_path: &Path,
    args: &[&str],
    options: GitRunOptions,
    cancel_flag: Arc<AtomicBool>,
    mut on_line: F,
) -> Result<i32, String>
where
    F: FnMut(&str) -> bool,
{
    let git_binary = git_binary_path()?;
    let git_path_env = git_path_env()?;
    let mut command = tokio::process::Command::new(git_binary);
    command.current_dir(repo_path);
    command.env("PATH", git_path_env);
    command.args(args);
    command.stdin(Stdio::null());
    command.stdout(Stdio::piped());
    command.stderr(Stdio::piped());

    let mut child = command
        .kill_on_drop(true)
        .spawn()
        .map_err(|e| e.to_string())?;

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "Failed to capture git stdout".to_string())?;

    let mut reader = BufReader::new(stdout).lines();
    let start = Instant::now();

    loop {
        if cancel_flag.load(Ordering::Relaxed) {
            let _ = child.kill().await;
            return Ok(-1);
        }

        if start.elapsed() > options.timeout {
            let _ = child.kill().await;
            return Err("Git command timed out".to_string());
        }

        let next_line = match timeout(Duration::from_millis(250), reader.next_line()).await {
            Ok(Ok(Some(line))) => line,
            Ok(Ok(None)) => break,
            Ok(Err(error)) => return Err(error.to_string()),
            Err(_) => continue,
        };

        if !on_line(&next_line) {
            let _ = child.kill().await;
            return Ok(0);
        }
    }

    let status = child.wait().await.map_err(|e| e.to_string())?;
    Ok(status.code().unwrap_or(1))
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── validate_relative_path tests ─────────────────────────────────

    #[test]
    fn validate_simple_filename() {
        assert!(validate_relative_path("file.txt").is_ok());
    }

    #[test]
    fn validate_nested_path() {
        assert!(validate_relative_path("src/main.rs").is_ok());
        assert!(validate_relative_path("deeply/nested/path/to/file.rs").is_ok());
    }

    #[test]
    fn validate_path_with_dots_in_name() {
        assert!(validate_relative_path("file.test.rs").is_ok());
        assert!(validate_relative_path(".gitignore").is_ok());
        assert!(validate_relative_path("src/.hidden").is_ok());
    }

    #[test]
    fn reject_git_directory_paths() {
        assert!(validate_relative_path(".git").is_err());
        assert!(validate_relative_path(".git/config").is_err());
        assert!(validate_relative_path("foo/.git/hooks").is_err());
    }

    #[test]
    #[cfg(unix)]
    fn reject_absolute_path_unix() {
        let result = validate_relative_path("/etc/passwd");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Absolute"));
    }

    #[test]
    fn reject_absolute_path_windows() {
        // Windows-style paths should be rejected when running on Windows
        // On non-Windows systems, this test is a no-op as the path looks relative
        #[cfg(windows)]
        {
            let result = validate_relative_path("C:\\Windows\\System32");
            assert!(result.is_err());
        }
        #[cfg(not(windows))]
        {
            // On Unix, Windows paths look like relative paths with special characters
            // which may or may not be rejected depending on implementation
            let _ = validate_relative_path("C:\\Windows\\System32");
        }
    }

    #[test]
    fn reject_parent_directory_traversal() {
        let result = validate_relative_path("../secret");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("traversal"));
    }

    #[test]
    fn reject_nested_parent_traversal() {
        let result = validate_relative_path("src/../../outside");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("traversal"));
    }

    #[test]
    fn reject_multiple_parent_dirs() {
        let result = validate_relative_path("../../../etc/passwd");
        assert!(result.is_err());
    }

    #[test]
    fn allow_current_dir_component() {
        // ./ is allowed (current directory)
        assert!(validate_relative_path("./file.txt").is_ok());
        assert!(validate_relative_path("src/./file.txt").is_ok());
    }

    #[test]
    fn validate_path_with_spaces() {
        assert!(validate_relative_path("path with spaces/file.txt").is_ok());
    }

    #[test]
    fn validate_path_with_unicode() {
        assert!(validate_relative_path("日本語/ファイル.rs").is_ok());
    }

    // ── is_index_lock_error tests ────────────────────────────────────

    #[test]
    fn detect_index_lock_error() {
        let err = "fatal: Unable to create '/path/.git/index.lock': File exists.";
        assert!(is_index_lock_error(err));
    }

    #[test]
    fn detect_index_lock_error_variant() {
        let err = "Another process is locking index.lock File exists";
        assert!(is_index_lock_error(err));
    }

    #[test]
    fn not_index_lock_error() {
        let err = "fatal: not a git repository";
        assert!(!is_index_lock_error(err));
    }

    #[test]
    fn not_index_lock_partial_match() {
        // Must have both parts
        let err = "index.lock";
        assert!(!is_index_lock_error(err));

        let err = "File exists";
        assert!(!is_index_lock_error(err));
    }

    // ── GitRunOptions tests ──────────────────────────────────────────

    #[test]
    fn default_read_options() {
        let opts = GitRunOptions::default_read();
        assert_eq!(opts.timeout, Duration::from_secs(30));
        assert!(opts.allow_failure_codes.is_empty());
        assert!(!opts.local_only);
    }

    #[test]
    fn local_only_read_disables_object_hydration() {
        let opts = GitRunOptions::local_only_read();
        assert_eq!(opts.timeout, Duration::from_secs(30));
        assert!(opts.allow_failure_codes.is_empty());
        assert!(opts.local_only);
    }

    #[test]
    fn with_timeout() {
        let opts = GitRunOptions::default_read().with_timeout(Duration::from_secs(60));
        assert_eq!(opts.timeout, Duration::from_secs(60));
    }

    #[test]
    fn allow_exit_codes() {
        let opts = GitRunOptions::default_read().allow_exit_codes(&[1, 2]);
        assert_eq!(opts.allow_failure_codes, &[1, 2]);
    }

    #[test]
    fn sensitive_commands_disable_prompts_and_remove_trace_destinations() {
        let mut command = tokio::process::Command::new("git");
        command
            .env("GIT_TRACE", "/tmp/leak")
            .env("git_trace2_event", "/tmp/leak-2")
            .env("GIT_CURL_VERBOSE", "1")
            .env("SSLKEYLOGFILE", "/tmp/leaking-tls-secrets")
            .env("GIT_ASKPASS", "/tmp/leaking-git-askpass")
            .env("SSH_ASKPASS", "/tmp/leaking-ssh-askpass")
            .env("GIT_SSH", "/tmp/leaking-ssh")
            .env("GIT_SSH_COMMAND", "/tmp/leaking-ssh --interactive")
            .env("GIT_SSH_VARIANT", "plink")
            .env("GIT_CONFIG_COUNT", "1")
            .env("GIT_CONFIG_KEY_0", "url.evil.insteadOf")
            .env("GIT_CONFIG_VALUE_0", "https://example.invalid/")
            .env("GIT_CONFIG_KEY_42", "protocol.ext.allow")
            .env("GIT_CONFIG_VALUE_42", "always")
            .env("GIT_CONFIG_PARAMETERS", "'protocol.ext.allow=always'")
            .env("GIT_EXEC_PATH", "/tmp/leaking-git-exec")
            .env("GIT_ALLOW_PROTOCOL", "ext")
            .env("GIT_PROTOCOL_FROM_USER", "1")
            .env("GIT_DIR", "/tmp/leaking-git-dir")
            .env("GIT_WORK_TREE", "/tmp/leaking-worktree")
            .env("GIT_COMMON_DIR", "/tmp/leaking-common-dir")
            .env("GIT_INDEX_FILE", "/tmp/leaking-index")
            .env("GIT_OBJECT_DIRECTORY", "/tmp/leaking-objects")
            .env(
                "GIT_ALTERNATE_OBJECT_DIRECTORIES",
                "/tmp/leaking-alternates",
            )
            .env("GIT_NAMESPACE", "leaking-namespace")
            .env("GIT_REPLACE_REF_BASE", "refs/leaking/")
            .env("GIT_NO_REPLACE_OBJECTS", "0")
            .env("GIT_NO_LAZY_FETCH", "0")
            .env("GIT_GRAFT_FILE", "/tmp/leaking-grafts")
            .env("GIT_SHALLOW_FILE", "/tmp/leaking-shallow")
            .env("GIT_QUARANTINE_PATH", "/tmp/leaking-quarantine")
            .env("HOME", "/tmp/preserved-home")
            .env("SSH_AUTH_SOCK", "/tmp/preserved-agent")
            .env("UNRELATED", "kept");
        configure_sensitive_command(
            &mut command,
            [
                OsString::from("GIT_TRACE"),
                OsString::from("git_trace2_event"),
                OsString::from("GIT_CURL_VERBOSE"),
                OsString::from("SSLKEYLOGFILE"),
                OsString::from("GIT_SSH"),
                OsString::from("GIT_SSH_COMMAND"),
                OsString::from("GIT_SSH_VARIANT"),
                OsString::from("GIT_CONFIG_COUNT"),
                OsString::from("GIT_CONFIG_KEY_0"),
                OsString::from("GIT_CONFIG_VALUE_0"),
                OsString::from("GIT_CONFIG_KEY_42"),
                OsString::from("GIT_CONFIG_VALUE_42"),
                OsString::from("GIT_CONFIG_PARAMETERS"),
                OsString::from("GIT_EXEC_PATH"),
                OsString::from("GIT_ALLOW_PROTOCOL"),
                OsString::from("GIT_PROTOCOL_FROM_USER"),
                OsString::from("GIT_DIR"),
                OsString::from("GIT_WORK_TREE"),
                OsString::from("GIT_COMMON_DIR"),
                OsString::from("GIT_INDEX_FILE"),
                OsString::from("GIT_OBJECT_DIRECTORY"),
                OsString::from("GIT_ALTERNATE_OBJECT_DIRECTORIES"),
                OsString::from("GIT_NAMESPACE"),
                OsString::from("GIT_REPLACE_REF_BASE"),
                OsString::from("GIT_NO_REPLACE_OBJECTS"),
                OsString::from("GIT_NO_LAZY_FETCH"),
                OsString::from("GIT_GRAFT_FILE"),
                OsString::from("GIT_SHALLOW_FILE"),
                OsString::from("GIT_QUARANTINE_PATH"),
                OsString::from("HOME"),
                OsString::from("SSH_AUTH_SOCK"),
                OsString::from("UNRELATED"),
            ],
        );

        let settings = command
            .as_std()
            .get_envs()
            .map(|(key, value)| {
                (
                    key.to_string_lossy().into_owned(),
                    value.map(|value| value.to_string_lossy().into_owned()),
                )
            })
            .collect::<std::collections::HashMap<_, _>>();
        assert_eq!(settings.get("GIT_TRACE"), Some(&None));
        assert_eq!(settings.get("git_trace2_event"), Some(&None));
        assert_eq!(settings.get("GIT_CURL_VERBOSE"), Some(&None));
        assert_eq!(settings.get("SSLKEYLOGFILE"), Some(&None));
        assert_eq!(settings.get("GIT_ASKPASS"), Some(&Some(String::new())));
        assert_eq!(settings.get("SSH_ASKPASS"), Some(&Some(String::new())));
        assert_eq!(settings.get("GIT_SSH"), Some(&None));
        assert_eq!(
            settings.get("GIT_SSH_COMMAND"),
            Some(&Some(SENSITIVE_SSH_COMMAND.into()))
        );
        assert_eq!(settings.get("GIT_SSH_VARIANT"), Some(&Some("ssh".into())));
        assert_eq!(settings.get("GIT_CONFIG_COUNT"), Some(&None));
        assert_eq!(settings.get("GIT_CONFIG_KEY_0"), Some(&None));
        assert_eq!(settings.get("GIT_CONFIG_VALUE_0"), Some(&None));
        assert_eq!(settings.get("GIT_CONFIG_KEY_42"), Some(&None));
        assert_eq!(settings.get("GIT_CONFIG_VALUE_42"), Some(&None));
        for key in [
            "GIT_CONFIG_PARAMETERS",
            "GIT_EXEC_PATH",
            "GIT_ALLOW_PROTOCOL",
            "GIT_PROTOCOL_FROM_USER",
            "GIT_DIR",
            "GIT_WORK_TREE",
            "GIT_COMMON_DIR",
            "GIT_INDEX_FILE",
            "GIT_OBJECT_DIRECTORY",
            "GIT_ALTERNATE_OBJECT_DIRECTORIES",
            "GIT_NAMESPACE",
            "GIT_REPLACE_REF_BASE",
            "GIT_GRAFT_FILE",
            "GIT_SHALLOW_FILE",
            "GIT_QUARANTINE_PATH",
        ] {
            assert_eq!(settings.get(key), Some(&None), "{key} was not scrubbed");
        }
        assert_eq!(settings.get("UNRELATED"), Some(&Some("kept".into())));
        assert_eq!(
            settings.get("GIT_NO_REPLACE_OBJECTS"),
            Some(&Some("1".into()))
        );
        assert_eq!(settings.get("GIT_NO_LAZY_FETCH"), Some(&Some("1".into())));
        assert_eq!(
            settings.get("HOME"),
            Some(&Some("/tmp/preserved-home".into()))
        );
        assert_eq!(
            settings.get("SSH_AUTH_SOCK"),
            Some(&Some("/tmp/preserved-agent".into()))
        );
        assert_eq!(settings.get("GIT_TERMINAL_PROMPT"), Some(&Some("0".into())));
        assert_eq!(settings.get("GCM_INTERACTIVE"), Some(&Some("never".into())));
        assert_eq!(
            settings.get("SSH_ASKPASS_REQUIRE"),
            Some(&Some("never".into()))
        );

        let key = "url.https://example.invalid/owner/repository.git.insteadOf";
        let alias = "gitru-pin::12345678-1234-1234-1234-123456789abc";
        configure_private_transport(&mut command, key, alias, SensitiveRemoteProtocol::Https);
        let settings = command
            .as_std()
            .get_envs()
            .map(|(key, value)| {
                (
                    key.to_string_lossy().into_owned(),
                    value.map(|value| value.to_string_lossy().into_owned()),
                )
            })
            .collect::<std::collections::HashMap<_, _>>();
        assert_eq!(settings.get("GIT_CONFIG_COUNT"), Some(&Some("1".into())));
        assert_eq!(settings.get("GIT_CONFIG_KEY_0"), Some(&Some(key.into())));
        assert_eq!(
            settings.get("GIT_CONFIG_VALUE_0"),
            Some(&Some(alias.into()))
        );
        assert_eq!(
            settings.get("GIT_ALLOW_PROTOCOL"),
            Some(&Some("https".into()))
        );
        assert_eq!(
            settings.get("GIT_PROTOCOL_FROM_USER"),
            Some(&Some("0".into()))
        );
    }

    #[cfg(unix)]
    #[test]
    fn sensitive_http_401_never_invokes_inherited_or_configured_askpass() {
        use std::{os::unix::fs::PermissionsExt, process::Stdio};

        let directory = tempfile::tempdir().unwrap();
        let home = directory.path().join("home");
        std::fs::create_dir(&home).unwrap();
        let inherited_marker = directory.path().join("inherited-askpass-called");
        let inherited_helper = directory.path().join("inherited-askpass");
        std::fs::write(
            &inherited_helper,
            format!("#!/bin/sh\n: > '{}'\nexit 1\n", inherited_marker.display()),
        )
        .unwrap();
        let mut permissions = std::fs::metadata(&inherited_helper).unwrap().permissions();
        permissions.set_mode(0o700);
        std::fs::set_permissions(&inherited_helper, permissions).unwrap();

        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "runner::tests::sensitive_http_401_never_invokes_askpass_child",
                "--ignored",
                "--nocapture",
            ])
            .env_clear()
            .env("HOME", &home)
            .env("USERPROFILE", &home)
            .env("PATH", std::env::var_os("PATH").unwrap_or_default())
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CONFIG_GLOBAL", home.join("gitconfig"))
            .env("GIT_ASKPASS", &inherited_helper)
            .env("GITRU_R136_ASKPASS_ROOT", directory.path())
            .stdin(Stdio::null())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .status()
            .unwrap();

        assert!(status.success(), "isolated askpass fixture failed");
        assert!(!inherited_marker.exists());
        assert!(directory.path().join("credential-helper-called").exists());
    }

    #[cfg(unix)]
    #[tokio::test]
    #[ignore = "entered by isolated parent fixture"]
    async fn sensitive_http_401_never_invokes_askpass_child() {
        use std::{
            io::{Read, Write},
            net::TcpListener,
            os::unix::fs::PermissionsExt,
            sync::{
                Arc,
                atomic::{AtomicBool, AtomicUsize, Ordering},
            },
        };

        let root =
            PathBuf::from(std::env::var_os("GITRU_R136_ASKPASS_ROOT").expect("fixture root"));
        let configured_marker = root.join("configured-askpass-called");
        let configured_helper = root.join("configured-askpass");
        std::fs::write(
            &configured_helper,
            format!("#!/bin/sh\n: > '{}'\nexit 1\n", configured_marker.display()),
        )
        .unwrap();
        let mut permissions = std::fs::metadata(&configured_helper).unwrap().permissions();
        permissions.set_mode(0o700);
        std::fs::set_permissions(&configured_helper, permissions).unwrap();
        let credential_marker = root.join("credential-helper-called");
        let credential_helper = root.join("credential-helper");
        std::fs::write(
            &credential_helper,
            format!(
                "#!/bin/sh\n: > '{}'\nif [ \"$1\" = get ]; then\n  printf 'username=helper-user\\npassword=helper-password\\n'\nfi\n",
                credential_marker.display()
            ),
        )
        .unwrap();
        let mut permissions = std::fs::metadata(&credential_helper).unwrap().permissions();
        permissions.set_mode(0o700);
        std::fs::set_permissions(&credential_helper, permissions).unwrap();

        let repo = root.join("repo");
        std::fs::create_dir(&repo).unwrap();
        let git = git_binary_path().unwrap();
        let status = std::process::Command::new(&git)
            .current_dir(&repo)
            .args(["init", "-b", "main"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .unwrap();
        assert!(status.success());
        let helper_config = format!("!{}", credential_helper.display());
        let status = std::process::Command::new(&git)
            .current_dir(&repo)
            .args(["config", "credential.helper", &helper_config])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .unwrap();
        assert!(status.success());
        let status = std::process::Command::new(&git)
            .current_dir(&repo)
            .args([
                "config",
                "core.askPass",
                configured_helper.to_str().unwrap(),
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .unwrap();
        assert!(status.success());

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let address = listener.local_addr().unwrap();
        let stop = Arc::new(AtomicBool::new(false));
        let requests = Arc::new(AtomicUsize::new(0));
        let server = std::thread::spawn({
            let stop = stop.clone();
            let requests = requests.clone();
            move || {
                let deadline = Instant::now() + Duration::from_secs(10);
                while !stop.load(Ordering::SeqCst) && Instant::now() < deadline {
                    match listener.accept() {
                        Ok((mut stream, _)) => {
                            requests.fetch_add(1, Ordering::SeqCst);
                            let mut request = [0_u8; 4096];
                            let _ = stream.read(&mut request);
                            stream
                                .write_all(
                                    b"HTTP/1.1 401 Unauthorized\r\nWWW-Authenticate: Basic realm=\"gitru\"\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                                )
                                .unwrap();
                        }
                        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                            std::thread::sleep(Duration::from_millis(10));
                        }
                        Err(error) => panic!("authentication server failed: {error}"),
                    }
                }
            }
        });

        let runner = GitCommandRunner::new(repo.to_str().unwrap()).unwrap();
        let mut transaction = runner.transaction().await.unwrap();
        let url = format!("http://synthetic-user@{address}/owner/repository.git");
        let (_, status) = transaction
            .sensitive_read(&["ls-remote", "--", &url], 1024)
            .await
            .unwrap();
        assert_ne!(status, 0);
        stop.store(true, Ordering::SeqCst);
        server.join().unwrap();

        assert!(requests.load(Ordering::SeqCst) > 0);
        assert!(credential_marker.exists());
        assert!(!configured_marker.exists());
        assert!(!root.join("inherited-askpass-called").exists());
    }

    // ── GitCommandRunner tests ───────────────────────────────────────

    #[tokio::test]
    async fn prepared_ref_lock_rejects_contention_without_using_timeout_fallback() {
        let directory = tempfile::tempdir().unwrap();
        let repository = git2::Repository::init(directory.path()).unwrap();
        let reference = "refs/heads/pr/held-lock";
        let signature = git2::Signature::now("Fixture", "fixture@example.invalid").unwrap();
        let tree_id = repository.treebuilder(None).unwrap().write().unwrap();
        let tree = repository.find_tree(tree_id).unwrap();
        let expected = repository
            .commit(
                Some(reference),
                &signature,
                &signature,
                "fixture",
                &tree,
                &[],
            )
            .unwrap();
        let mut config = repository.config().unwrap();
        for key in [
            "core.filesRefLockTimeout",
            "core.packedRefsTimeout",
            "reftable.lockTimeout",
        ] {
            config.set_i64(key, -1).unwrap();
            assert_eq!(config.get_i64(key).unwrap(), -1);
        }
        let foreign_lock = repository.path().join("refs/heads/pr/held-lock.lock");
        std::fs::write(&foreign_lock, "held by another process").unwrap();
        let runner = GitCommandRunner::new(directory.path().to_str().unwrap()).unwrap();
        let mut transaction = runner.transaction().await.unwrap();

        // Exercise the native lock protocol directly, without unrelated
        // checkout inspections. Keep the production operation budget for slow
        // process startup; an inherited -1 wait must hit the distinct timeout
        // fallback and fail this assertion, not count as successful rejection.
        let error = transaction
            .prepare_ref_lock(reference, &expected.to_string(), Duration::from_secs(30))
            .await
            .err()
            .expect("a foreign ref lock must prevent preparation");
        assert_eq!(error, "Git ref changed");
        assert_eq!(
            std::fs::read_to_string(&foreign_lock).unwrap(),
            "held by another process"
        );
        assert_eq!(
            repository.find_reference(reference).unwrap().target(),
            Some(expected)
        );

        // Prove the same ref, OID and protocol are valid once only our fixture
        // lock is removed; a generic spawn/protocol failure cannot pass above.
        std::fs::remove_file(&foreign_lock).unwrap();
        let held = transaction
            .prepare_ref_lock(reference, &expected.to_string(), Duration::from_secs(30))
            .await
            .expect("uncontended exact ref should prepare");
        assert!(foreign_lock.exists());
        held.release().await;
        assert!(!foreign_lock.exists());
        assert_eq!(
            repository.find_reference(reference).unwrap().target(),
            Some(expected)
        );
    }

    #[test]
    fn runner_rejects_invalid_path() {
        let result = GitCommandRunner::new("/nonexistent/path/to/repo");
        assert!(result.is_err());
        let err = result.err().unwrap();
        assert!(err.contains("Invalid repository path"));
    }

    #[test]
    fn runner_accepts_valid_temp_dir() {
        let temp_dir = tempfile::tempdir().expect("failed to create temp dir");

        // Initialize git repo
        std::process::Command::new("git")
            .current_dir(temp_dir.path())
            .args(["init"])
            .output()
            .expect("failed to init git");

        let result = GitCommandRunner::new(temp_dir.path().to_str().unwrap());
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn transaction_holds_repo_lock_across_commands() {
        let temp_dir = tempfile::tempdir().expect("failed to create temp dir");
        let output = std::process::Command::new("git")
            .current_dir(temp_dir.path())
            .args(["init"])
            .output()
            .expect("failed to init git");
        assert!(output.status.success(), "git init failed");

        let runner = GitCommandRunner::new(temp_dir.path().to_str().unwrap())
            .expect("failed to create runner");
        let mut transaction = runner.transaction().await.expect("transaction lock");

        let object_id = transaction
            .run_with_input(
                &["hash-object", "--stdin"],
                "transaction input",
                GitRunOptions::default_read(),
            )
            .await
            .expect("transaction command should not reacquire its own lock");
        assert!(!object_id.is_empty());

        let waiting_runner = runner.clone();
        let (started_tx, started_rx) = tokio::sync::oneshot::channel();
        let mut waiting = tokio::spawn(async move {
            started_tx.send(()).expect("start receiver dropped");
            waiting_runner
                .run_with_options(
                    &["rev-parse", "--is-inside-work-tree"],
                    GitRunOptions::default_read(),
                )
                .await
        });
        started_rx.await.expect("waiting command did not start");

        assert!(
            tokio::time::timeout(Duration::from_millis(100), &mut waiting)
                .await
                .is_err(),
            "regular runner command should wait for the transaction lock"
        );

        drop(transaction);
        let output = tokio::time::timeout(Duration::from_secs(2), waiting)
            .await
            .expect("waiting command did not resume")
            .expect("waiting task failed")
            .expect("waiting Git command failed");
        assert_eq!(output, "true");
    }

    #[test]
    #[cfg(unix)]
    fn resolve_program_path_prefers_earlier_dirs() {
        use std::fs;
        use std::os::unix::fs::PermissionsExt;

        let temp_dir = tempfile::tempdir().expect("failed to create temp dir");
        let first_dir = temp_dir.path().join("first");
        let second_dir = temp_dir.path().join("second");
        fs::create_dir_all(&first_dir).expect("failed to create first dir");
        fs::create_dir_all(&second_dir).expect("failed to create second dir");

        let first_git = first_dir.join("git");
        let second_git = second_dir.join("git");
        fs::write(&first_git, "#!/bin/sh\nexit 0\n").expect("failed to write first git");
        fs::write(&second_git, "#!/bin/sh\nexit 0\n").expect("failed to write second git");

        let mut first_perms = fs::metadata(&first_git)
            .expect("first metadata")
            .permissions();
        first_perms.set_mode(0o755);
        fs::set_permissions(&first_git, first_perms).expect("first permissions");

        let mut second_perms = fs::metadata(&second_git)
            .expect("second metadata")
            .permissions();
        second_perms.set_mode(0o755);
        fs::set_permissions(&second_git, second_perms).expect("second permissions");

        let resolved =
            resolve_program_path_from_dirs("git", &[first_dir.clone(), second_dir.clone()])
                .expect("expected git to resolve");

        assert_eq!(resolved, first_git);
    }

    #[test]
    fn preferred_tool_dirs_include_current_path_dirs_last() {
        let temp_dir = tempfile::tempdir().expect("failed to create temp dir");
        let custom_dir = temp_dir.path().join("custom");
        std::fs::create_dir_all(&custom_dir).expect("failed to create custom dir");

        let dirs = preferred_tool_dirs_with_path_dirs(vec![custom_dir.clone()]);

        assert!(dirs.iter().any(|dir| dir == &custom_dir));
        assert!(!dirs.is_empty());
        assert!(
            dirs.iter()
                .position(|dir| dir == &custom_dir)
                .expect("custom dir missing")
                >= DEFAULT_TOOL_DIRS.len()
        );
    }
}
