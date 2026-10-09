//! Test helper utilities for git crate integration tests.
//!
//! Provides a `TestRepo` struct that creates a temporary git repository
//! with utilities for common git operations.

#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::process::Command;
use tempfile::TempDir;

/// A temporary git repository for testing purposes.
/// Automatically cleans up on drop.
pub struct TestRepo {
    /// The temporary directory containing the repo.
    pub dir: TempDir,
}

impl TestRepo {
    /// Create a new temporary git repository.
    pub fn new() -> Self {
        let dir = TempDir::new().expect("failed to create temp dir");

        // Initialize git repo with 'main' as initial branch
        let output = Command::new("git")
            .current_dir(dir.path())
            .args(["init", "-b", "main"])
            .output()
            .expect("failed to run git init");
        assert!(output.status.success(), "git init failed");

        // Configure user for commits
        let _ = Command::new("git")
            .current_dir(dir.path())
            .args(["config", "user.email", "test@example.com"])
            .output();

        let _ = Command::new("git")
            .current_dir(dir.path())
            .args(["config", "user.name", "Test User"])
            .output();

        let repository = Self { dir };
        // Synthetic fixture commits must not invoke the developer's signer.
        // This is repository-local and never changes global or product settings.
        repository.git(&["config", "--local", "commit.gpgsign", "false"]);
        repository
    }

    /// Create a SHA-256 repository when the installed Git supports the object
    /// format. Callers may skip only on an explicit init failure.
    pub fn new_sha256() -> Option<Self> {
        let dir = TempDir::new().expect("failed to create temp dir");
        let output = Command::new("git")
            .current_dir(dir.path())
            .args(["init", "--object-format=sha256", "-b", "main"])
            .output()
            .expect("failed to run SHA-256 git init");
        if !output.status.success() {
            return None;
        }
        for (key, value) in [
            ("user.email", "test@example.com"),
            ("user.name", "Test User"),
            ("commit.gpgsign", "false"),
        ] {
            let output = Command::new("git")
                .current_dir(dir.path())
                .args(["config", key, value])
                .output()
                .expect("failed to configure SHA-256 test repository");
            assert!(output.status.success(), "git config failed");
        }
        Some(Self { dir })
    }

    /// Clone a test repository as a blobless partial clone while keeping the
    /// source available as a functioning promisor remote. Tests can therefore
    /// prove that a read failed locally instead of silently hydrating objects.
    pub fn blobless_clone(source: &Self) -> Self {
        source.git(&["config", "uploadpack.allowFilter", "true"]);
        let dir = TempDir::new().expect("failed to create partial clone dir");
        let source_url = format!("file://{}", source.path().display());
        let output = Command::new("git")
            .args([
                "clone",
                "--quiet",
                "--filter=blob:none",
                "--no-checkout",
                &source_url,
            ])
            .arg(dir.path())
            .output()
            .expect("failed to run partial git clone");
        assert!(
            output.status.success(),
            "partial git clone failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let repository = Self { dir };
        repository.git(&["config", "--local", "commit.gpgsign", "false"]);
        repository
    }

    /// Check the object database without allowing a promisor remote fetch.
    pub fn has_local_object(&self, oid: &str) -> bool {
        Command::new("git")
            .current_dir(self.dir.path())
            .env("GIT_NO_LAZY_FETCH", "1")
            .args(["cat-file", "-e", oid])
            .output()
            .expect("failed to inspect local object")
            .status
            .success()
    }

    /// Get the path to the repository.
    pub fn path(&self) -> &Path {
        self.dir.path()
    }

    /// Get the path as a string.
    pub fn path_str(&self) -> &str {
        self.dir.path().to_str().expect("path is valid utf-8")
    }

    /// Create a file with the given content.
    pub fn create_file(&self, name: &str, content: &str) -> PathBuf {
        let file_path = self.dir.path().join(name);
        if let Some(parent) = file_path.parent() {
            std::fs::create_dir_all(parent).expect("failed to create parent dirs");
        }
        std::fs::write(&file_path, content).expect("failed to write file");
        file_path
    }

    /// Run a git command and return stdout.
    pub fn git(&self, args: &[&str]) -> String {
        let output = Command::new("git")
            .current_dir(self.dir.path())
            .args(args)
            .output()
            .expect("failed to run git command");

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            panic!("git {} failed: {}", args.join(" "), stderr);
        }

        String::from_utf8_lossy(&output.stdout).trim().to_string()
    }

    /// Stage a file.
    pub fn add(&self, path: &str) {
        self.git(&["add", path]);
    }

    /// Stage all changes.
    pub fn add_all(&self) {
        self.git(&["add", "-A"]);
    }

    /// Create a commit with the given message.
    pub fn commit(&self, message: &str) -> String {
        self.git(&["commit", "-m", message])
    }

    /// Create a file, stage it, and commit.
    pub fn commit_file(&self, name: &str, content: &str, message: &str) -> String {
        self.create_file(name, content);
        self.add(name);
        self.commit(message)
    }

    /// Get the current branch name.
    pub fn current_branch(&self) -> String {
        self.git(&["rev-parse", "--abbrev-ref", "HEAD"])
    }

    /// Create and switch to a new branch.
    pub fn create_branch(&self, name: &str) -> String {
        self.git(&["switch", "-c", name])
    }

    /// Switch to an existing branch.
    pub fn switch_branch(&self, name: &str) -> String {
        self.git(&["switch", name])
    }

    /// Get the HEAD commit hash.
    pub fn head_commit(&self) -> String {
        self.git(&["rev-parse", "HEAD"])
    }

    /// List all local branches.
    pub fn list_branches(&self) -> Vec<String> {
        self.git(&["branch", "--list", "--format=%(refname:short)"])
            .lines()
            .map(|s| s.to_string())
            .collect()
    }

    /// Create a stash (includes untracked files).
    pub fn stash_push(&self, message: Option<&str>) -> String {
        match message {
            Some(msg) => self.git(&["stash", "push", "-u", "-m", msg]),
            None => self.git(&["stash", "push", "-u"]),
        }
    }

    /// List stashes.
    pub fn stash_list(&self) -> String {
        self.git(&["stash", "list"])
    }

    /// Check if there are uncommitted changes.
    pub fn has_changes(&self) -> bool {
        let status = self.git(&["status", "--porcelain"]);
        !status.is_empty()
    }

    /// Check if a file is staged.
    pub fn is_staged(&self, path: &str) -> bool {
        let output = Command::new("git")
            .current_dir(self.dir.path())
            .args(["diff", "--cached", "--name-only"])
            .output()
            .expect("failed to run git diff");

        let staged = String::from_utf8_lossy(&output.stdout);
        staged.lines().any(|l| l == path || l.ends_with(path))
    }

    /// Get the HEAD commit hash.
    pub fn get_head_hash(&self) -> String {
        self.git(&["rev-parse", "HEAD"])
    }

    /// Set up a bare remote repository and add it as origin.
    pub fn setup_remote(&self) -> TempDir {
        let remote_dir = TempDir::new().expect("failed to create remote temp dir");

        // Initialize bare repo
        Command::new("git")
            .current_dir(remote_dir.path())
            .args(["init", "--bare"])
            .output()
            .expect("failed to init bare repo");

        // Add as remote
        let remote_url = remote_dir.path().to_str().unwrap();
        self.git(&["remote", "add", "origin", remote_url]);

        remote_dir
    }

    /// Push to origin.
    pub fn push(&self, branch: &str) -> String {
        self.git(&["push", "-u", "origin", branch])
    }

    /// Fetch from origin.
    pub fn fetch(&self) -> String {
        self.git(&["fetch", "--all"])
    }
}

impl Default for TestRepo {
    fn default() -> Self {
        Self::new()
    }
}

/// Run an async test using tokio's current_thread runtime.
pub fn run_async<F>(f: F)
where
    F: std::future::Future<Output = ()>,
{
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("failed to create runtime")
        .block_on(f);
}
