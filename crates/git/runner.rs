use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::{Mutex as StdMutex, OnceLock};
use std::time::{Duration, Instant};
use tokio::io::AsyncWriteExt;
use tokio::sync::Mutex as AsyncMutex;
use tokio::time::{sleep, timeout};

#[derive(Clone, Copy)]
pub struct GitRunOptions {
    pub timeout: Duration,
    pub allow_failure_codes: &'static [i32],
}

impl GitRunOptions {
    pub fn default_read() -> Self {
        Self {
            timeout: Duration::from_secs(30),
            allow_failure_codes: &[],
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
        run_git_command_async(&self.repo_path, args, None, options).await
    }

    pub async fn run_with_input(
        &self,
        args: &[&str],
        input: &str,
        options: GitRunOptions,
    ) -> Result<String, String> {
        run_git_command_async(&self.repo_path, args, Some(input.as_bytes()), options).await
    }
}

async fn run_git_command_async(
    repo_path: &Path,
    args: &[&str],
    input: Option<&[u8]>,
    options: GitRunOptions,
) -> Result<String, String> {
    let repo_lock = command_lock_for_repo(repo_path)?;
    let _guard = repo_lock.lock().await;

    let mut attempt: u32 = 0;
    const MAX_INDEX_LOCK_RETRIES: u32 = 6;

    loop {
        attempt += 1;
        match run_git_command_once(repo_path, args, input, options).await {
            Ok(output) => return Ok(output),
            Err(err) if is_index_lock_error(&err) && attempt < MAX_INDEX_LOCK_RETRIES => {
                let backoff_ms = 50 * attempt as u64;
                sleep(Duration::from_millis(backoff_ms)).await;
            }
            Err(err) => return Err(err),
        }
    }
}

async fn run_git_command_once(
    repo_path: &Path,
    args: &[&str],
    input: Option<&[u8]>,
    options: GitRunOptions,
) -> Result<String, String> {
    let mut command = tokio::process::Command::new("git");
    command.current_dir(repo_path);
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
        Ok(Ok(output)) => finalize_output(output, options.allow_failure_codes),
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

fn is_index_lock_error(err: &str) -> bool {
    err.contains("index.lock") && err.contains("File exists")
}

pub fn validate_relative_path(path: &str) -> Result<(), String> {
    let path = Path::new(path);
    if path.is_absolute() {
        return Err("Absolute paths are not allowed".to_string());
    }
    for component in path.components() {
        if matches!(component, std::path::Component::ParentDir) {
            return Err("Path traversal is not allowed".to_string());
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

    // ── GitCommandRunner tests ───────────────────────────────────────

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
}
