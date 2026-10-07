use crate::{
    context::RepoContext,
    models::{
        operation::RepoOperationKind,
        pull_checkout::{
            PullCheckoutAction, PullCheckoutBlocker, PullCheckoutError, PullCheckoutInspection,
            PullCheckoutReceipt, PullCheckoutTarget,
        },
        remotes::{RemoteEndpoint, RemoteTransport},
    },
    parsers::remotes::urls,
    runner::{GitCommandTransaction, SensitiveRemoteProtocol},
    service::{operation::OperationService, remotes::RemotesService},
};
use sha2::{Digest, Sha256};
use std::{sync::Arc, time::Duration};

const LOCAL_TIMEOUT: Duration = Duration::from_secs(30);
const FETCH_TIMEOUT: Duration = Duration::from_secs(120);
const MAX_LOCAL_OUTPUT: usize = 16 * 1024;

pub struct PullCheckoutService {
    ctx: Arc<RepoContext>,
}

impl PullCheckoutService {
    pub fn new(ctx: Arc<RepoContext>) -> Self {
        Self { ctx }
    }

    pub async fn inspect(
        &self,
        target: &PullCheckoutTarget,
    ) -> Result<PullCheckoutInspection, PullCheckoutError> {
        validate_target(target)?;
        let mut transaction = self
            .ctx
            .runner
            .transaction()
            .await
            .map_err(|_| PullCheckoutError::InspectionFailed)?;
        self.inspect_in(&mut transaction, target).await
    }

    /// Acquire the repository transaction and prove that the exact plan is
    /// still current. The returned guard performs no mutation until `fetch`
    /// or `finish` is called, so callers can revalidate non-Git authority while
    /// every Gitru command for this repository remains queued behind it.
    pub async fn prepare_execution(
        &self,
        target: &PullCheckoutTarget,
        expected: &PullCheckoutInspection,
        operation_id: &str,
    ) -> Result<PreparedPullCheckout, PullCheckoutError> {
        validate_target(target)?;
        if !valid_operation_id(operation_id) {
            return Err(PullCheckoutError::InvalidTarget);
        }
        let mut transaction = self
            .ctx
            .runner
            .transaction()
            .await
            .map_err(|_| PullCheckoutError::InspectionFailed)?;
        let worktree_paths = RemotesService::new(self.ctx.clone())
            .worktree_paths_in(&mut transaction)
            .await
            .map_err(|_| PullCheckoutError::InspectionFailed)?;
        let actual = self.inspect_in(&mut transaction, target).await?;
        if &actual != expected {
            return Err(PullCheckoutError::StalePlan);
        }
        let Some(action) = actual.action else {
            return Err(PullCheckoutError::Blocked);
        };
        let pinned_remote = if action == PullCheckoutAction::FetchAndCreateBranch {
            Some(
                pin_remote(
                    &mut transaction,
                    target,
                    operation_id,
                    actual
                        .remote_identity
                        .as_deref()
                        .ok_or(PullCheckoutError::RemoteChanged)?,
                )
                .await?,
            )
        } else {
            None
        };
        Ok(PreparedPullCheckout {
            ctx: self.ctx.clone(),
            transaction,
            target: target.clone(),
            baseline: actual,
            action,
            pinned_remote,
            fetched: false,
            worktree_paths,
        })
    }

    async fn inspect_in(
        &self,
        transaction: &mut GitCommandTransaction,
        target: &PullCheckoutTarget,
    ) -> Result<PullCheckoutInspection, PullCheckoutError> {
        let remotes = RemotesService::new(self.ctx.clone())
            .snapshot_in(transaction)
            .await
            .map_err(|error| {
                if error == crate::models::remotes::RemoteObservationError::Changed {
                    PullCheckoutError::RemoteChanged
                } else {
                    PullCheckoutError::InspectionFailed
                }
            })?;
        if remotes.semantic_digest != target.remote_digest {
            return Err(PullCheckoutError::RemoteChanged);
        }
        let source_matches = remotes.remotes.iter().any(|remote| {
            remote.name == target.remote_name
                && remote.fetch_urls.iter().any(|url| {
                    url.ordinal == target.remote_ordinal
                        && url.endpoint.as_ref() == Some(&target.remote_endpoint)
                })
        });
        if !source_matches || target.remote_ordinal != 0 {
            return Err(PullCheckoutError::RemoteChanged);
        }
        if remote_has_custom_transport(transaction, &target.remote_name).await? {
            return Err(PullCheckoutError::RemoteChanged);
        }
        for reference in [
            format!("refs/heads/{}", target.source_branch),
            format!("refs/heads/{}", target.local_branch),
        ] {
            let (status, _) = command(
                transaction,
                &["check-ref-format", &reference],
                LOCAL_TIMEOUT,
            )
            .await?;
            if status != 0 {
                return Err(PullCheckoutError::InvalidTarget);
            }
        }

        let current_branch = symbolic_branch(transaction).await?;
        let current_head_oid = read_oid(transaction, &["rev-parse", "--verify", "HEAD^{commit}"])
            .await?
            .ok_or(PullCheckoutError::InspectionFailed)?;
        let (status, worktree) = command(
            transaction,
            &[
                "--no-optional-locks",
                "status",
                "--porcelain",
                "-z",
                "--untracked-files=all",
            ],
            LOCAL_TIMEOUT,
        )
        .await?;
        if status != 0 {
            return Err(PullCheckoutError::InspectionFailed);
        }
        let dirty = !worktree.is_empty();
        let operation = OperationService::new(self.ctx.clone())
            .get_repo_operation()
            .map_err(|_| PullCheckoutError::InspectionFailed)?;
        let operation_kind = operation.kind.clone();
        let active_operation =
            operation_kind != RepoOperationKind::Clean || !operation.conflict_paths.is_empty();
        let target_ref = format!("refs/heads/{}", target.local_branch);
        let target_branch_oid = read_oid(
            transaction,
            &["show-ref", "--verify", "--hash", &target_ref],
        )
        .await?;
        let object_available = object_exists(transaction, &target.expected_oid).await?;
        let already_checked_out = current_branch.as_deref() == Some(&target.local_branch)
            && current_head_oid == target.expected_oid;

        let (action, blocker) = if already_checked_out {
            (Some(PullCheckoutAction::AlreadyCheckedOut), None)
        } else if active_operation {
            (None, Some(PullCheckoutBlocker::ActiveOperation))
        } else if dirty {
            (None, Some(PullCheckoutBlocker::DirtyWorktree))
        } else if target_branch_oid
            .as_ref()
            .is_some_and(|oid| oid != &target.expected_oid)
        {
            (None, Some(PullCheckoutBlocker::ExistingBranchDiverged))
        } else if target_branch_oid.is_some() {
            (Some(PullCheckoutAction::SwitchExisting), None)
        } else if object_available {
            (Some(PullCheckoutAction::CreateBranch), None)
        } else {
            (Some(PullCheckoutAction::FetchAndCreateBranch), None)
        };
        let remote_identity = if action == Some(PullCheckoutAction::FetchAndCreateBranch) {
            Some(observe_remote_url(transaction, target).await?.identity)
        } else {
            None
        };

        Ok(PullCheckoutInspection {
            current_branch: current_branch.clone(),
            current_head_oid,
            detached: current_branch.is_none(),
            dirty,
            operation: operation_kind,
            target_branch_oid,
            object_available,
            action,
            blocker,
            remote_identity,
        })
    }
}

/// Lock-bound checkout state. Dropping a prepared value before `finish` has no
/// worktree/branch effect. A fetch imports objects without naming a destination
/// ref, so it cannot move a repository ref before `finish` creates the branch.
pub struct PreparedPullCheckout {
    ctx: Arc<RepoContext>,
    transaction: GitCommandTransaction,
    target: PullCheckoutTarget,
    baseline: PullCheckoutInspection,
    action: PullCheckoutAction,
    pinned_remote: Option<PinnedRemote>,
    fetched: bool,
    worktree_paths: crate::service::remotes::NativeWorktreePaths,
}

impl PreparedPullCheckout {
    pub fn worktree_paths(&self) -> &crate::service::remotes::NativeWorktreePaths {
        &self.worktree_paths
    }

    pub fn needs_fetch(&self) -> bool {
        self.action == PullCheckoutAction::FetchAndCreateBranch
    }

    pub async fn fetch(&mut self) -> Result<(), PullCheckoutError> {
        if !self.needs_fetch() || self.fetched {
            return Ok(());
        }
        let source_ref = format!("refs/heads/{}", self.target.source_branch);
        if remote_has_custom_transport(&mut self.transaction, &self.target.remote_name).await? {
            return Err(PullCheckoutError::RemoteChanged);
        }
        let pinned = self
            .pinned_remote
            .as_ref()
            .ok_or(PullCheckoutError::FetchFailed)?;
        if remote_head(
            &mut self.transaction,
            pinned,
            &source_ref,
            self.target.remote_endpoint.transport,
        )
        .await?
            != self.target.expected_oid
        {
            return Err(PullCheckoutError::HeadMoved);
        }

        // A source-only refspec has no ref destination. `--refmap=` also
        // suppresses every configured remote-tracking mapping, and
        // `--no-write-fetch-head` suppresses FETCH_HEAD. The command can add
        // objects, but there is no repository ref for Git (or a concurrently
        // installed symbolic ref) to dereference and update.
        let status = redacted_status_with_config(
            &mut self.transaction,
            &[
                "-c",
                "protocol.allow=never",
                "-c",
                allowed_protocol_config(self.target.remote_endpoint.transport),
                "fetch",
                "--no-tags",
                "--no-write-fetch-head",
                "--no-auto-gc",
                "--no-recurse-submodules",
                "--no-prune",
                "--no-prune-tags",
                "--upload-pack=git-upload-pack",
                "--refmap=",
                "--",
                &pinned.alias,
                &source_ref,
            ],
            FETCH_TIMEOUT,
            &pinned.config_key,
            &pinned.alias,
            allowed_protocol(self.target.remote_endpoint.transport),
        )
        .await
        .map_err(|_| PullCheckoutError::FetchFailed)?;
        if status != 0 {
            return Err(PullCheckoutError::FetchFailed);
        }

        // Observe the advertised source again after transfer. If it changed in
        // flight, fail before branch/worktree mutation even if the expected
        // commit arrived as an ancestor of a newer tip.
        if remote_head(
            &mut self.transaction,
            pinned,
            &source_ref,
            self.target.remote_endpoint.transport,
        )
        .await?
            != self.target.expected_oid
        {
            return Err(PullCheckoutError::HeadMoved);
        }
        if !object_exists(&mut self.transaction, &self.target.expected_oid).await? {
            return Err(PullCheckoutError::FetchFailed);
        }
        self.fetched = true;
        Ok(())
    }

    pub async fn finish(mut self) -> Result<PullCheckoutReceipt, PullCheckoutError> {
        // This is deliberately the first step after the caller's final native
        // authority gate. Reread every Git mutation precondition after any
        // potentially slow fetch; only exact object availability may change.
        if self.needs_fetch() {
            let expected_identity = self
                .baseline
                .remote_identity
                .as_deref()
                .ok_or(PullCheckoutError::RemoteChanged)?;
            if observe_remote_url(&mut self.transaction, &self.target)
                .await?
                .identity
                != expected_identity
            {
                return Err(PullCheckoutError::RemoteChanged);
            }
        }
        let actual = PullCheckoutService::new(self.ctx.clone())
            .inspect_in(&mut self.transaction, &self.target)
            .await?;
        let mut expected = self.baseline.clone();
        if self.needs_fetch() {
            if !self.fetched {
                return Err(PullCheckoutError::StalePlan);
            }
            expected.object_available = true;
            expected.action = Some(PullCheckoutAction::CreateBranch);
            expected.remote_identity = None;
        }
        if actual != expected {
            return Err(PullCheckoutError::StalePlan);
        }
        let status = match self.action {
            PullCheckoutAction::AlreadyCheckedOut => 0,
            PullCheckoutAction::SwitchExisting => redacted_status(
                &mut self.transaction,
                &["switch", "--", &self.target.local_branch],
                LOCAL_TIMEOUT,
            )
            .await
            .map_err(|_| PullCheckoutError::CheckoutFailed)?,
            PullCheckoutAction::CreateBranch | PullCheckoutAction::FetchAndCreateBranch => {
                redacted_status(
                    &mut self.transaction,
                    &[
                        "switch",
                        "--no-track",
                        "-c",
                        &self.target.local_branch,
                        &self.target.expected_oid,
                    ],
                    LOCAL_TIMEOUT,
                )
                .await
                .map_err(|_| PullCheckoutError::CheckoutFailed)?
            }
        };
        if status != 0 {
            return Err(PullCheckoutError::CheckoutFailed);
        }
        let branch = symbolic_branch(&mut self.transaction)
            .await?
            .ok_or(PullCheckoutError::VerificationFailed)?;
        let oid = read_oid(
            &mut self.transaction,
            &["rev-parse", "--verify", "HEAD^{commit}"],
        )
        .await?
        .ok_or(PullCheckoutError::VerificationFailed)?;
        if branch != self.target.local_branch || oid != self.target.expected_oid {
            return Err(PullCheckoutError::VerificationFailed);
        }
        self.ctx.cache.invalidate_all();
        Ok(PullCheckoutReceipt {
            branch,
            oid,
            fetched: self.fetched,
        })
    }
}

struct PinnedRemote {
    alias: String,
    config_key: String,
}

async fn pin_remote(
    transaction: &mut GitCommandTransaction,
    target: &PullCheckoutTarget,
    operation_id: &str,
    expected_identity: &str,
) -> Result<PinnedRemote, PullCheckoutError> {
    let observed = observe_remote_url(transaction, target).await?;
    if observed.identity != expected_identity {
        return Err(PullCheckoutError::RemoteChanged);
    }
    let alias = format!("gitru-pin::{operation_id}");
    Ok(PinnedRemote {
        alias: alias.clone(),
        config_key: format!("url.{}.insteadOf", observed.canonical_url),
    })
}

struct ObservedRemoteUrl {
    canonical_url: String,
    identity: String,
}

async fn observe_remote_url(
    transaction: &mut GitCommandTransaction,
    target: &PullCheckoutTarget,
) -> Result<ObservedRemoteUrl, PullCheckoutError> {
    if remote_has_custom_transport(transaction, &target.remote_name).await? {
        return Err(PullCheckoutError::RemoteChanged);
    }
    let (status, bytes) = command(
        transaction,
        &["remote", "get-url", "--all", "--", &target.remote_name],
        LOCAL_TIMEOUT,
    )
    .await?;
    if status != 0 {
        return Err(PullCheckoutError::RemoteChanged);
    }
    let observed = urls(&bytes).map_err(|_| PullCheckoutError::InspectionFailed)?;
    if observed.first().is_none_or(|url| {
        url.ordinal != target.remote_ordinal
            || url.endpoint.as_ref() != Some(&target.remote_endpoint)
    }) {
        return Err(PullCheckoutError::RemoteChanged);
    }
    let raw_url = std::str::from_utf8(
        bytes
            .split(|byte| *byte == b'\n')
            .next()
            .ok_or(PullCheckoutError::InspectionFailed)?,
    )
    .map_err(|_| PullCheckoutError::InspectionFailed)?;
    if raw_url.is_empty() || raw_url.chars().any(char::is_control) {
        return Err(PullCheckoutError::InspectionFailed);
    }
    let canonical_url = canonical_remote_url(raw_url, &target.remote_endpoint)?;
    let digest = Sha256::digest(canonical_url.as_bytes());
    Ok(ObservedRemoteUrl {
        canonical_url,
        identity: digest.iter().map(|byte| format!("{byte:02x}")).collect(),
    })
}

fn canonical_remote_url(
    raw_url: &str,
    endpoint: &RemoteEndpoint,
) -> Result<String, PullCheckoutError> {
    let unsupported = || PullCheckoutError::UnsupportedCredentials;
    let username = match endpoint.transport {
        RemoteTransport::Https => {
            let parsed = url::Url::parse(raw_url).map_err(|_| PullCheckoutError::RemoteChanged)?;
            if parsed.scheme() != "https" {
                return Err(PullCheckoutError::RemoteChanged);
            }
            if !parsed.username().is_empty()
                || parsed.password().is_some()
                || parsed.query().is_some()
                || parsed.fragment().is_some()
            {
                return Err(unsupported());
            }
            String::new()
        }
        RemoteTransport::Ssh => {
            let parsed = url::Url::parse(raw_url).map_err(|_| PullCheckoutError::RemoteChanged)?;
            if parsed.scheme() != "ssh" {
                return Err(PullCheckoutError::RemoteChanged);
            }
            if parsed.password().is_some()
                || parsed.query().is_some()
                || parsed.fragment().is_some()
                || !valid_remote_username(parsed.username())
            {
                return Err(unsupported());
            }
            parsed.username().to_owned()
        }
        RemoteTransport::Scp => {
            let (authority, _) = raw_url
                .split_once(':')
                .ok_or(PullCheckoutError::RemoteChanged)?;
            let username = authority
                .rsplit_once('@')
                .map(|(username, _)| username)
                .unwrap_or("");
            if !valid_remote_username(username) {
                return Err(unsupported());
            }
            username.to_owned()
        }
    };
    let scheme = if endpoint.transport == RemoteTransport::Https {
        "https"
    } else {
        "ssh"
    };
    let default_port = if endpoint.transport == RemoteTransport::Https {
        443
    } else {
        22
    };
    let host = if endpoint.host.starts_with('[') && endpoint.host.ends_with(']') {
        endpoint.host.clone()
    } else if endpoint.host.contains(':') {
        format!("[{}]", endpoint.host)
    } else {
        endpoint.host.clone()
    };
    let user = if username.is_empty() {
        String::new()
    } else {
        format!("{username}@")
    };
    let port = if endpoint.port == default_port {
        String::new()
    } else {
        format!(":{}", endpoint.port)
    };
    let path = if endpoint.transport == RemoteTransport::Scp {
        format!("/~/{}", endpoint.path)
    } else {
        format!("/{}", endpoint.path)
    };
    Ok(format!("{scheme}://{user}{host}{port}{path}"))
}

fn valid_remote_username(username: &str) -> bool {
    username.len() <= 64
        && !username.starts_with('-')
        && username
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
}

async fn remote_head(
    transaction: &mut GitCommandTransaction,
    pinned: &PinnedRemote,
    source_ref: &str,
    transport: RemoteTransport,
) -> Result<String, PullCheckoutError> {
    let (status, bytes) = command_with_config(
        transaction,
        &[
            "-c",
            "protocol.allow=never",
            "-c",
            allowed_protocol_config(transport),
            "ls-remote",
            "--quiet",
            "--exit-code",
            "--refs",
            "--upload-pack=git-upload-pack",
            "--",
            &pinned.alias,
            source_ref,
        ],
        FETCH_TIMEOUT,
        &pinned.config_key,
        &pinned.alias,
        allowed_protocol(transport),
    )
    .await
    .map_err(|_| PullCheckoutError::FetchFailed)?;
    if status == 2 {
        return Err(PullCheckoutError::HeadMoved);
    }
    if status != 0 {
        return Err(PullCheckoutError::FetchFailed);
    }
    let value = std::str::from_utf8(&bytes).map_err(|_| PullCheckoutError::FetchFailed)?;
    let line = value
        .strip_suffix('\n')
        .ok_or(PullCheckoutError::FetchFailed)?;
    if line.contains('\n') {
        return Err(PullCheckoutError::FetchFailed);
    }
    let (oid, advertised_ref) = line
        .split_once('\t')
        .ok_or(PullCheckoutError::FetchFailed)?;
    if advertised_ref != source_ref || !valid_oid(oid) {
        return Err(PullCheckoutError::FetchFailed);
    }
    Ok(oid.to_owned())
}

fn allowed_protocol_config(transport: RemoteTransport) -> &'static str {
    match transport {
        RemoteTransport::Https => "protocol.https.allow=always",
        RemoteTransport::Ssh | RemoteTransport::Scp => "protocol.ssh.allow=always",
    }
}

fn allowed_protocol(transport: RemoteTransport) -> SensitiveRemoteProtocol {
    match transport {
        RemoteTransport::Https => SensitiveRemoteProtocol::Https,
        RemoteTransport::Ssh | RemoteTransport::Scp => SensitiveRemoteProtocol::Ssh,
    }
}

async fn remote_has_custom_transport(
    transaction: &mut GitCommandTransaction,
    remote: &str,
) -> Result<bool, PullCheckoutError> {
    let key = format!("remote.{remote}.vcs");
    let (status, _) = command(transaction, &["config", "--get-all", &key], LOCAL_TIMEOUT).await?;
    match status {
        0 => Ok(true),
        1 => Ok(false),
        _ => Err(PullCheckoutError::InspectionFailed),
    }
}

async fn symbolic_branch(
    transaction: &mut GitCommandTransaction,
) -> Result<Option<String>, PullCheckoutError> {
    let (status, bytes) = command(
        transaction,
        &["symbolic-ref", "-q", "--short", "HEAD"],
        LOCAL_TIMEOUT,
    )
    .await?;
    if status == 1 {
        return Ok(None);
    }
    if status != 0 {
        return Err(PullCheckoutError::InspectionFailed);
    }
    let branch = one_line(&bytes).ok_or(PullCheckoutError::InspectionFailed)?;
    Ok(Some(branch))
}

async fn read_oid(
    transaction: &mut GitCommandTransaction,
    args: &[&str],
) -> Result<Option<String>, PullCheckoutError> {
    let (status, bytes) = command(transaction, args, LOCAL_TIMEOUT).await?;
    if status == 1 || status == 128 {
        return Ok(None);
    }
    if status != 0 {
        return Err(PullCheckoutError::InspectionFailed);
    }
    let oid = one_line(&bytes).ok_or(PullCheckoutError::InspectionFailed)?;
    if !valid_oid(&oid) {
        return Err(PullCheckoutError::InspectionFailed);
    }
    Ok(Some(oid))
}

async fn object_exists(
    transaction: &mut GitCommandTransaction,
    oid: &str,
) -> Result<bool, PullCheckoutError> {
    let object = format!("{oid}^{{commit}}");
    let (status, _) = command(transaction, &["cat-file", "-e", &object], LOCAL_TIMEOUT).await?;
    match status {
        0 => Ok(true),
        1 | 128 => Ok(false),
        _ => Err(PullCheckoutError::InspectionFailed),
    }
}

async fn command(
    transaction: &mut GitCommandTransaction,
    args: &[&str],
    duration: Duration,
) -> Result<(i32, Vec<u8>), PullCheckoutError> {
    tokio::time::timeout(duration, transaction.sensitive_read(args, MAX_LOCAL_OUTPUT))
        .await
        .map_err(|_| PullCheckoutError::InspectionFailed)?
        .map(|(bytes, status)| (status, bytes))
        .map_err(|_| PullCheckoutError::InspectionFailed)
}

async fn redacted_status(
    transaction: &mut GitCommandTransaction,
    args: &[&str],
    duration: Duration,
) -> Result<i32, PullCheckoutError> {
    command(transaction, args, duration)
        .await
        .map(|(status, _)| status)
}

async fn command_with_config(
    transaction: &mut GitCommandTransaction,
    args: &[&str],
    duration: Duration,
    key: &str,
    value: &str,
    protocol: SensitiveRemoteProtocol,
) -> Result<(i32, Vec<u8>), PullCheckoutError> {
    tokio::time::timeout(
        duration,
        transaction.sensitive_read_with_config(args, MAX_LOCAL_OUTPUT, key, value, protocol),
    )
    .await
    .map_err(|_| PullCheckoutError::InspectionFailed)?
    .map(|(bytes, status)| (status, bytes))
    .map_err(|_| PullCheckoutError::InspectionFailed)
}

async fn redacted_status_with_config(
    transaction: &mut GitCommandTransaction,
    args: &[&str],
    duration: Duration,
    key: &str,
    value: &str,
    protocol: SensitiveRemoteProtocol,
) -> Result<i32, PullCheckoutError> {
    command_with_config(transaction, args, duration, key, value, protocol)
        .await
        .map(|(status, _)| status)
}

fn one_line(bytes: &[u8]) -> Option<String> {
    let value = std::str::from_utf8(bytes).ok()?.strip_suffix('\n')?;
    if value.is_empty() || value.chars().any(char::is_control) {
        return None;
    }
    Some(value.to_owned())
}

fn validate_target(target: &PullCheckoutTarget) -> Result<(), PullCheckoutError> {
    if !plain(&target.remote_name, 255)
        || target.remote_name.starts_with('-')
        || target.remote_ordinal != 0
        || !valid_branch_path(&target.source_branch)
        || target.source_branch.starts_with('-')
        || !valid_branch_path(&target.local_branch)
        || target.local_branch.starts_with('-')
        || !valid_oid(&target.expected_oid)
        || target.remote_digest.len() != 64
        || !target
            .remote_digest
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit())
    {
        return Err(PullCheckoutError::InvalidTarget);
    }
    Ok(())
}

fn valid_branch_path(value: &str) -> bool {
    plain(value, 1024)
        && value
            .split('/')
            .all(|component| !component.is_empty() && component.len() <= 255)
}

fn plain(value: &str, maximum: usize) -> bool {
    !value.is_empty()
        && value.len() <= maximum
        && value.trim() == value
        && !value.chars().any(char::is_control)
}

fn valid_oid(value: &str) -> bool {
    matches!(value.len(), 40 | 64)
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
}

fn valid_operation_id(value: &str) -> bool {
    value.len() == 36
        && value.bytes().enumerate().all(|(index, byte)| {
            if matches!(index, 8 | 13 | 18 | 23) {
                byte == b'-'
            } else {
                byte.is_ascii_hexdigit()
            }
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn endpoint(transport: RemoteTransport, host: &str, port: u16) -> RemoteEndpoint {
        RemoteEndpoint {
            transport,
            host: host.into(),
            port,
            path: "Owner/Repo.git".into(),
        }
    }

    #[test]
    fn canonical_urls_are_credential_free_and_preserve_safe_ssh_usernames() {
        assert_eq!(
            canonical_remote_url(
                "https://example.com:443/Owner/Repo.git",
                &endpoint(RemoteTransport::Https, "example.com", 443),
            ),
            Ok("https://example.com/Owner/Repo.git".into())
        );
        assert_eq!(
            canonical_remote_url(
                "ssh://git@[2001:db8::1]:2222/Owner/Repo.git",
                &endpoint(RemoteTransport::Ssh, "[2001:db8::1]", 2222),
            ),
            Ok("ssh://git@[2001:db8::1]:2222/Owner/Repo.git".into())
        );
        assert_eq!(
            canonical_remote_url(
                "git@example.com:Owner/Repo.git",
                &endpoint(RemoteTransport::Scp, "example.com", 22),
            ),
            Ok("ssh://git@example.com/~/Owner/Repo.git".into())
        );
    }

    #[test]
    fn canonical_urls_reject_inline_secrets_and_unsafe_ssh_usernames() {
        let https = endpoint(RemoteTransport::Https, "example.com", 443);
        for raw in [
            "https://user@example.com/Owner/Repo.git",
            "https://user:secret@example.com/Owner/Repo.git",
            "https://example.com/Owner/Repo.git?token=secret",
            "https://example.com/Owner/Repo.git#secret",
        ] {
            assert_eq!(
                canonical_remote_url(raw, &https),
                Err(PullCheckoutError::UnsupportedCredentials)
            );
        }
        let ssh = endpoint(RemoteTransport::Ssh, "example.com", 22);
        for raw in [
            "ssh://git:secret@example.com/Owner/Repo.git",
            "ssh://bad%20user@example.com/Owner/Repo.git",
            "ssh://git@example.com/Owner/Repo.git?token=secret",
            "ssh://git@example.com/Owner/Repo.git#secret",
        ] {
            assert_eq!(
                canonical_remote_url(raw, &ssh),
                Err(PullCheckoutError::UnsupportedCredentials)
            );
        }
    }
}
