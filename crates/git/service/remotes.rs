//! Read-only effective remotes. This service never invokes a remote transport.
use crate::{
    context::RepoContext,
    models::remotes::*,
    parsers::remotes::{config_names, urls},
    runner::GitCommandTransaction,
};
use sha2::{Digest, Sha256};
use std::{path::Path, sync::Arc, time::Duration};

pub struct RemotesService {
    ctx: Arc<RepoContext>,
}

/// Native-only filesystem coordinates, never an IPC response or link authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeWorktreePaths {
    pub worktree: std::path::PathBuf,
    pub git_dir: std::path::PathBuf,
    pub common_dir: std::path::PathBuf,
}
const MAX_OUTPUT: usize = 1_048_576;

impl RemotesService {
    pub fn new(ctx: Arc<RepoContext>) -> Self {
        Self { ctx }
    }

    pub async fn worktree_paths(&self) -> Result<NativeWorktreePaths, RemoteObservationError> {
        tokio::time::timeout(Duration::from_secs(5), async {
            let mut tx = self
                .ctx
                .runner
                .transaction()
                .await
                .map_err(|_| RemoteObservationError::Unavailable)?;
            self.worktree_paths_in(&mut tx).await
        })
        .await
        .map_err(|_| RemoteObservationError::Timeout)?
    }

    pub(crate) async fn worktree_paths_in(
        &self,
        tx: &mut GitCommandTransaction,
    ) -> Result<NativeWorktreePaths, RemoteObservationError> {
        let mut remaining = 16384usize;
        let mut paths = Vec::new();
        for args in [
            &["rev-parse", "--show-toplevel"][..],
            &["rev-parse", "--absolute-git-dir"][..],
            &["rev-parse", "--path-format=absolute", "--git-common-dir"][..],
        ] {
            let bytes = read(tx, args, &mut remaining, &[]).await?;
            let value = std::str::from_utf8(&bytes)
                .map_err(|_| RemoteObservationError::InvalidConfiguration)?
                .strip_suffix('\n')
                .ok_or(RemoteObservationError::InvalidConfiguration)?;
            if value.chars().any(char::is_control) {
                return Err(RemoteObservationError::InvalidConfiguration);
            }
            let path = std::path::PathBuf::from(value)
                .canonicalize()
                .map_err(|_| RemoteObservationError::Unavailable)?;
            paths.push(path);
        }
        let mut paths = paths.into_iter();
        Ok(NativeWorktreePaths {
            worktree: paths.next().ok_or(RemoteObservationError::Unavailable)?,
            git_dir: paths.next().ok_or(RemoteObservationError::Unavailable)?,
            common_dir: paths.next().ok_or(RemoteObservationError::Unavailable)?,
        })
    }

    pub async fn snapshot(&self) -> Result<RemoteSnapshot, RemoteObservationError> {
        tokio::time::timeout(Duration::from_secs(5), self.observe())
            .await
            .map_err(|_| RemoteObservationError::Timeout)?
    }

    async fn observe(&self) -> Result<RemoteSnapshot, RemoteObservationError> {
        use RemoteObservationError as Error;
        let mut tx = self
            .ctx
            .runner
            .transaction()
            .await
            .map_err(|_| Error::Unavailable)?;
        self.snapshot_in(&mut tx).await
    }

    pub(crate) async fn snapshot_in(
        &self,
        tx: &mut GitCommandTransaction,
    ) -> Result<RemoteSnapshot, RemoteObservationError> {
        use RemoteObservationError as Error;
        let mut remaining = MAX_OUTPUT;
        for directory in ["remotes", "branches"] {
            let bytes = read(
                tx,
                &["rev-parse", "--git-path", directory],
                &mut remaining,
                &[],
            )
            .await?;
            let path = std::str::from_utf8(&bytes).map_err(|_| Error::InvalidConfiguration)?;
            let path = path.strip_suffix('\n').ok_or(Error::InvalidConfiguration)?;
            if path.chars().any(char::is_control) {
                return Err(Error::InvalidConfiguration);
            }
            let path = Path::new(path);
            let path = if path.is_absolute() {
                path.to_path_buf()
            } else {
                Path::new(&self.ctx.repo_path).join(path)
            };
            match std::fs::read_dir(path) {
                Ok(mut entries) => {
                    if entries.next().is_some() {
                        return Err(Error::UnsupportedLegacyConfiguration);
                    }
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(_) => return Err(Error::Unavailable),
            }
        }
        let before = read(tx, &["config", "--null", "--list"], &mut remaining, &[]).await?;
        let names = config_names(&before)?;
        let mut remotes = Vec::new();
        for name in names {
            let fetch = read(
                tx,
                &["remote", "get-url", "--all", "--", &name],
                &mut remaining,
                &[2],
            )
            .await?;
            let push = read(
                tx,
                &["remote", "get-url", "--push", "--all", "--", &name],
                &mut remaining,
                &[2],
            )
            .await?;
            remotes.push(SafeGitRemote {
                name,
                fetch_urls: urls(&fetch)?,
                push_urls: urls(&push)?,
            });
        }
        let after = read(tx, &["config", "--null", "--list"], &mut remaining, &[]).await?;
        if before != after {
            return Err(Error::Changed);
        }
        config_names(&after)?;
        let serialized = serde_json::to_vec(&remotes).map_err(|_| Error::Unavailable)?;
        let digest = Sha256::digest(serialized);
        let semantic_digest = digest.iter().map(|b| format!("{b:02x}")).collect();
        Ok(RemoteSnapshot {
            remotes,
            semantic_digest,
        })
    }
}

async fn read(
    tx: &mut GitCommandTransaction,
    args: &[&str],
    remaining: &mut usize,
    allowed: &[i32],
) -> Result<Vec<u8>, RemoteObservationError> {
    let (bytes, status) = tx.sensitive_read(args, *remaining).await?;
    *remaining = remaining.saturating_sub(bytes.len());
    if status != 0 && !allowed.contains(&status) {
        return Err(RemoteObservationError::Unavailable);
    }
    if status != 0 {
        return Ok(vec![]);
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::{Command, Stdio};

    #[test]
    fn effective_remote_fixture_runs_in_an_isolated_child_environment() {
        let directory = tempfile::tempdir().unwrap();
        let home = directory.path().join("home");
        std::fs::create_dir(&home).unwrap();
        let executable = std::env::current_exe().unwrap();
        let mut child = Command::new(executable);
        child
            .current_dir(directory.path())
            .args([
                "--exact",
                "service::remotes::tests::isolated_effective_remote_fixture",
                "--ignored",
                "--nocapture",
            ])
            .env_clear()
            .env("HOME", &home)
            .env("USERPROFILE", &home)
            .env("PATH", crate::runner::git_path_env().unwrap())
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CONFIG_GLOBAL", home.join("config"))
            .env("GITRU_R96_FIXTURE_ROOT", directory.path())
            .stdin(Stdio::null())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit());
        #[cfg(windows)]
        for name in ["SystemRoot", "WINDIR", "TEMP", "TMP"] {
            if let Some(value) = std::env::var_os(name) {
                child.env(name, value);
            }
        }
        let status = child.status().unwrap();
        assert!(
            status.success(),
            "Isolated effective remotes fixture failed"
        );
    }

    #[tokio::test]
    #[ignore = "entered by isolated parent fixture"]
    async fn isolated_effective_remote_fixture() {
        let root = std::path::PathBuf::from(
            std::env::var_os("GITRU_R96_FIXTURE_ROOT").expect("fixture root"),
        );
        let repo = root.join("repo");
        std::fs::create_dir(&repo).unwrap();
        let binary = crate::runner::git_binary_path().unwrap();
        let git = |args: &[&str]| {
            let status = Command::new(&binary)
                .current_dir(&repo)
                .args(args)
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .unwrap();
            assert!(
                status.success(),
                "Synthetic Git fixture configuration failed"
            );
        };
        git(&["init", "-q"]);
        git(&[
            "config",
            "--global",
            "url.ssh://git@github.com/.pushInsteadOf",
            "https://github.com/",
        ]);
        git(&[
            "config",
            "remote.implicit-push.url",
            "https://github.com/owner/implicit.git",
        ]);
        git(&[
            "config",
            "--global",
            "url.https://github.com/.insteadOf",
            "short:",
        ]);
        git(&[
            "config",
            "--global",
            "url.https://gitlab.com/team/.insteadOf",
            "short:long/",
        ]);
        git(&["config", "remote.origin.url", "short:owner/project.git"]);
        git(&[
            "config",
            "--add",
            "remote.origin.url",
            "https://synthetic-user:synthetic-token@github.com/owner/alternate.git?token=synthetic-query",
        ]);
        git(&[
            "config",
            "remote.origin.pushurl",
            "ssh://git@github.com:2222/owner/push.git",
        ]);
        git(&[
            "config",
            "--add",
            "remote.origin.pushurl",
            "git@github.com:owner/second-push.git",
        ]);
        git(&[
            "config",
            "--add",
            "remote.origin.pushurl",
            "https://github.com/owner/explicit.git",
        ]);
        git(&[
            "config",
            "remote.fork.with.dot.url",
            "short:long/sub/project.git",
        ]);
        let include = root.join("included-config");
        std::fs::write(
            &include,
            "[remote \"included\"]\nurl = https://bitbucket.org/team/project.git\n",
        )
        .unwrap();
        git(&["config", "include.path", include.to_str().unwrap()]);
        let config_path = repo.join(".git/config");
        let before = std::fs::read(&config_path).unwrap();
        let service =
            RemotesService::new(Arc::new(RepoContext::new(repo.to_str().unwrap()).unwrap()));
        let snapshot = service.snapshot().await.unwrap();
        assert!(
            std::fs::read(&config_path).unwrap() == before,
            "Read-only remote inspection modified configuration"
        );
        let origin = snapshot
            .remotes
            .iter()
            .find(|r| r.name == "origin")
            .unwrap();
        assert_eq!(origin.fetch_urls.len(), 2);
        assert_eq!(origin.push_urls.len(), 3);
        assert_eq!(
            origin.push_urls[2].endpoint.as_ref().unwrap().transport,
            RemoteTransport::Https,
            "Explicit push URL bypasses pushInsteadOf"
        );
        let implicit = snapshot
            .remotes
            .iter()
            .find(|r| r.name == "implicit-push")
            .unwrap();
        assert_eq!(
            implicit.fetch_urls[0].endpoint.as_ref().unwrap().transport,
            RemoteTransport::Https
        );
        assert_eq!(
            implicit.push_urls[0].endpoint.as_ref().unwrap().transport,
            RemoteTransport::Ssh
        );
        assert_eq!(
            origin.fetch_urls[0].endpoint.as_ref().unwrap().host,
            "github.com"
        );
        assert_eq!(origin.push_urls[0].endpoint.as_ref().unwrap().port, 2222);
        let fork = snapshot
            .remotes
            .iter()
            .find(|r| r.name == "fork.with.dot")
            .unwrap();
        assert_eq!(
            fork.fetch_urls[0].endpoint.as_ref().unwrap().path,
            "team/sub/project.git"
        );
        assert!(snapshot.remotes.iter().any(|r| r.name == "included"));
        assert!(
            !serde_json::to_string(&snapshot)
                .unwrap()
                .contains("synthetic-")
        );
        git(&[
            "config",
            "--replace-all",
            "remote.origin.url",
            "https://rotated-user:rotated-token@github.com/owner/project.git?token=rotated-query",
        ]);
        let rotated = service.snapshot().await.unwrap();
        assert!(
            !serde_json::to_string(&rotated)
                .unwrap()
                .contains("rotated-")
        );
        git(&[
            "config",
            "remote.injected.url",
            "https://github.com/one/repo\nhttps://github.com/two/repo",
        ]);
        assert_eq!(
            service.snapshot().await.unwrap_err(),
            RemoteObservationError::InvalidConfiguration
        );
        git(&["config", "--remove-section", "remote.injected"]);
        git(&["config", "extensions.worktreeConfig", "true"]);
        git(&[
            "config",
            "--worktree",
            "remote.worktree-specific.url",
            "https://github.com/owner/worktree.git",
        ]);
        assert!(
            service
                .snapshot()
                .await
                .unwrap()
                .remotes
                .iter()
                .any(|r| r.name == "worktree-specific")
        );
        git(&[
            "config",
            "--global",
            "remote.reset.url",
            "https://github.com/owner/global.git",
        ]);
        git(&["config", "remote.reset.url", ""]);
        git(&[
            "config",
            "--add",
            "remote.reset.url",
            "https://github.com/owner/local.git",
        ]);
        let reset = service.snapshot().await.unwrap();
        let reset = reset.remotes.iter().find(|r| r.name == "reset").unwrap();
        // Git 2.43 appends empty URL values; newer Git clears the list. Honor
        // the actual executable's effective URLs without imposing new semantics.
        let effective = Command::new(&binary)
            .current_dir(&repo)
            .args(["remote", "get-url", "--all", "--", "reset"])
            .stdin(Stdio::null())
            .stderr(Stdio::null())
            .output()
            .unwrap();
        assert!(effective.status.success());
        if effective.stdout == b"https://github.com/owner/local.git\n" {
            assert_eq!(reset.fetch_urls.len(), 1);
            assert_eq!(
                reset.fetch_urls[0].endpoint.as_ref().unwrap().path,
                "owner/local.git"
            );
        } else {
            assert!(effective.stdout==b"https://github.com/owner/global.git\n\nhttps://github.com/owner/local.git\n","Unexpected effective empty-URL semantics");
            assert_eq!(reset.fetch_urls.len(), 3);
            assert_eq!(
                reset.fetch_urls[0].endpoint.as_ref().unwrap().path,
                "owner/global.git"
            );
            assert!(
                reset.fetch_urls[1].sanitized_url.is_none()
                    && reset.fetch_urls[1].endpoint.is_none()
            );
            assert_eq!(
                reset.fetch_urls[2].endpoint.as_ref().unwrap().path,
                "owner/local.git"
            );
        }
        std::fs::create_dir(repo.join(".git/remotes")).unwrap();
        std::fs::write(
            repo.join(".git/remotes/legacy"),
            "URL: https://github.com/owner/project.git\n",
        )
        .unwrap();
        assert_eq!(
            service.snapshot().await.unwrap_err(),
            RemoteObservationError::UnsupportedLegacyConfiguration
        );
        std::fs::remove_dir_all(repo.join(".git/remotes")).unwrap();
        // Real Git output exceeds the cumulative cap. No truncated snapshot or
        // raw diagnostic reaches the safe caller.
        std::fs::write(
            &include,
            format!(
                "[remote \"large\"]\nurl = https://synthetic-secret@github.com/{}\n",
                "x".repeat(MAX_OUTPUT)
            ),
        )
        .unwrap();
        assert_eq!(
            service.snapshot().await.unwrap_err(),
            RemoteObservationError::LimitExceeded
        );
        std::fs::write(
            &include,
            "[remote \"broken\"\nurl = synthetic-stderr-secret\n",
        )
        .unwrap();
        let error = service.snapshot().await.unwrap_err();
        assert_eq!(error, RemoteObservationError::Unavailable);
        assert_eq!(serde_json::to_string(&error).unwrap(), "\"unavailable\"");
        std::fs::write(
            &include,
            "[remote \"included\"]\nurl = https://bitbucket.org/team/project.git\n",
        )
        .unwrap();
        #[cfg(unix)]
        {
            let fifo = root.join("blocked-config");
            git(&["config", "include.path", fifo.to_str().unwrap()]);
            let mkfifo = if Path::new("/usr/bin/mkfifo").is_file() {
                "/usr/bin/mkfifo"
            } else {
                "/bin/mkfifo"
            };
            assert!(
                Command::new(mkfifo)
                    .arg(&fifo)
                    .env_clear()
                    .current_dir(&root)
                    .stdin(Stdio::null())
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .status()
                    .unwrap()
                    .success()
            );
            assert_eq!(
                service.snapshot().await.unwrap_err(),
                RemoteObservationError::Timeout
            );
            std::fs::remove_file(&fifo).unwrap();
            std::fs::write(
                &fifo,
                "[remote \"after-timeout\"]\nurl = https://github.com/owner/after.git\n",
            )
            .unwrap();
            assert!(
                service
                    .snapshot()
                    .await
                    .unwrap()
                    .remotes
                    .iter()
                    .any(|r| r.name == "after-timeout"),
                "Deadline cancellation releases the native Git transaction"
            );
            std::fs::remove_file(&fifo).unwrap();
            assert!(
                Command::new(mkfifo)
                    .arg(&fifo)
                    .env_clear()
                    .current_dir(&root)
                    .stdin(Stdio::null())
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .status()
                    .unwrap()
                    .success()
            );
            assert!(
                tokio::time::timeout(Duration::from_millis(50), service.snapshot())
                    .await
                    .is_err()
            );
            std::fs::remove_file(&fifo).unwrap();
            std::fs::write(
                &fifo,
                "[remote \"after-cancellation\"]\nurl = https://github.com/owner/cancelled.git\n",
            )
            .unwrap();
            assert!(
                service
                    .snapshot()
                    .await
                    .unwrap()
                    .remotes
                    .iter()
                    .any(|r| r.name == "after-cancellation"),
                "Caller cancellation releases the native Git transaction"
            );
        }
    }
}
