//! Uncached, local-only source-ref evidence for an explicit online PR preview.
use crate::{
    context::RepoContext,
    runner::GitCommandTransaction,
    service::remotes::{NativeWorktreePaths, RemotesService},
};
use std::{sync::Arc, time::Duration};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PullCreationInspection {
    pub source_branch: String,
    pub source_oid: String,
    pub current_branch: String,
    pub current_head_oid: String,
    pub paths: NativeWorktreePaths,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PullCreationInspectionError {
    InvalidBranch,
    Detached,
    Unborn,
    MissingSource,
    InvalidObject,
    Changed,
    TimedOut,
    Unavailable,
}
use PullCreationInspectionError as Error;

pub struct PullCreationService {
    ctx: Arc<RepoContext>,
}
impl PullCreationService {
    pub fn new(ctx: Arc<RepoContext>) -> Self {
        Self { ctx }
    }
    /// No status/index refresh, fetch, checkout, replacement objects or lazy hydration.
    /// The deadline includes waiting for the repository's native runner lease.
    pub async fn inspect(&self, branch: &str) -> Result<PullCreationInspection, Error> {
        if branch.is_empty()
            || branch.len() > 1024
            || branch.starts_with('-')
            || branch.starts_with("refs/")
            || branch.chars().any(char::is_control)
        {
            return Err(Error::InvalidBranch);
        }
        tokio::time::timeout(Duration::from_secs(10), self.inspect_in(branch))
            .await
            .map_err(|_| Error::TimedOut)?
    }
    async fn inspect_in(&self, branch: &str) -> Result<PullCreationInspection, Error> {
        let mut tx = self
            .ctx
            .runner
            .transaction()
            .await
            .map_err(|_| Error::Unavailable)?;
        let reference = format!("refs/heads/{branch}");
        let (_, status) = read(&mut tx, &["check-ref-format", &reference], 16).await?;
        if status != 0 {
            return Err(Error::InvalidBranch);
        }
        let remotes = RemotesService::new(self.ctx.clone());
        let paths = remotes
            .worktree_paths_in(&mut tx)
            .await
            .map_err(|_| Error::Unavailable)?;
        let (current_branch, current_head_oid) = head(&mut tx).await?;
        let source_oid = source(&mut tx, &reference).await?;
        let after = remotes
            .worktree_paths_in(&mut tx)
            .await
            .map_err(|_| Error::Unavailable)?;
        if paths != after
            || head(&mut tx).await? != (current_branch.clone(), current_head_oid.clone())
            || source(&mut tx, &reference).await? != source_oid
        {
            return Err(Error::Changed);
        }
        Ok(PullCreationInspection {
            source_branch: branch.into(),
            source_oid,
            current_branch,
            current_head_oid,
            paths,
        })
    }
}
async fn read(
    tx: &mut GitCommandTransaction,
    args: &[&str],
    limit: usize,
) -> Result<(Vec<u8>, i32), Error> {
    tx.sensitive_local_read(args, limit)
        .await
        .map_err(|_| Error::Unavailable)
}
async fn head(tx: &mut GitCommandTransaction) -> Result<(String, String), Error> {
    let (bytes, status) = read(tx, &["symbolic-ref", "--quiet", "HEAD"], 2048).await?;
    if status != 0 {
        return Err(Error::Detached);
    }
    let reference = std::str::from_utf8(&bytes)
        .map_err(|_| Error::InvalidObject)?
        .strip_suffix('\n')
        .ok_or(Error::InvalidObject)?;
    let branch = reference
        .strip_prefix("refs/heads/")
        .filter(|v| !v.is_empty() && !v.chars().any(char::is_control))
        .ok_or(Error::InvalidObject)?;
    let oid = source(tx, reference).await.map_err(|e| {
        if e == Error::MissingSource {
            Error::Unborn
        } else {
            e
        }
    })?;
    Ok((branch.into(), oid))
}
async fn source(tx: &mut GitCommandTransaction, reference: &str) -> Result<String, Error> {
    let (bytes, status) = read(tx, &["show-ref", "--verify", "--hash", reference], 128).await?;
    if status != 0 {
        return Err(Error::MissingSource);
    }
    let oid = std::str::from_utf8(&bytes)
        .map_err(|_| Error::InvalidObject)?
        .strip_suffix('\n')
        .ok_or(Error::InvalidObject)?;
    if oid.len() != 40
        || !oid
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err(Error::InvalidObject);
    }
    let (kind, status) = read(tx, &["cat-file", "-t", oid], 32).await?;
    if status != 0 || kind != b"commit\n" {
        return Err(Error::InvalidObject);
    }
    Ok(oid.into())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test(start_paused = true)]
    async fn deadline_includes_a_blocked_native_runner_lease() {
        let dir = tempfile::tempdir().unwrap();
        assert!(
            std::process::Command::new("git")
                .args(["init", "-b", "main"])
                .arg(dir.path())
                .output()
                .unwrap()
                .status
                .success()
        );
        let ctx = Arc::new(RepoContext::new(dir.path().to_str().unwrap()).unwrap());
        let _lease = ctx.runner.transaction().await.unwrap();
        let service = PullCreationService::new(ctx.clone());
        let started = tokio::time::Instant::now();
        assert_eq!(service.inspect("main").await.unwrap_err(), Error::TimedOut);
        assert_eq!(started.elapsed(), Duration::from_secs(10));
    }
}
