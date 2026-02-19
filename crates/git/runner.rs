use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};
use tokio::io::AsyncWriteExt;
use tokio::time::timeout;

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
            return Err(format!("Invalid repository path: {}", repo_path));
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

    if let Some(payload) = input {
        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(payload).await.map_err(|e| e.to_string())?;
            drop(stdin);
        }
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
    static THROTTLE: OnceLock<Mutex<std::collections::HashMap<String, Instant>>> = OnceLock::new();
    let throttle = THROTTLE.get_or_init(|| Mutex::new(std::collections::HashMap::new()));

    let mut map = throttle
        .lock()
        .map_err(|_| "Failed to lock command throttle".to_string())?;

    let now = Instant::now();

    map.retain(|_, last| now.duration_since(*last) < Duration::from_secs(3600));

    if let Some(last) = map.get(repo_key) {
        if now.duration_since(*last) < min_interval {
            return Err("Command throttled to protect performance".to_string());
        }
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
