mod common;

use base64::Engine as _;
use common::{TestRepo, run_async};
use git::{
    core::RepoServices,
    models::pull_checkout::{
        PullCheckoutAction, PullCheckoutBlocker, PullCheckoutError, PullCheckoutInspection,
        PullCheckoutReceipt, PullCheckoutTarget,
    },
    runner::GitCommandRunner,
};
use std::{
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    path::{Component, Path, PathBuf},
    process::Command,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    time::Duration,
};

const OPERATION_ID: &str = "12345678-1234-1234-1234-123456789abc";
const LEGACY_TEMPORARY_REF: &str = "refs/gitru/pull-checkout/12345678-1234-1234-1234-123456789abc";
const TEST_CERT_DER: &str = "MIICvjCCAaagAwIBAgIJAOxzgfy4nORqMA0GCSqGSIb3DQEBCwUAMBQxEjAQBgNVBAMMCTEyNy4wLjAuMTAeFw0yNjEwMDcxNTAwNDZaFw0zNjEwMDQxNTAwNDZaMBQxEjAQBgNVBAMMCTEyNy4wLjAuMTCCASIwDQYJKoZIhvcNAQEBBQADggEPADCCAQoCggEBALLWYinJswS8FV/pe+HL6zriDPP3YD1mzHCxFISJTQuHFEUIWn0F4m+6OvcaNTFOz0CDF049To4fGa88G06qUlCxIcu5fwbgbGdrW4uTkWZ6O09DKFi6eCgfQbDV8j0B6gt4a690vAL1+dtOdYfPgOv6l0Coy5oxqayoRI/M4v0thgFCEJyhERSXasf/lzSx2XO1HiuNcTM7MLdHGSzpsa66VZS/am/bsbKBAXMaWj3UsPn+E5lKcaSOa2chgXakJXQmL6Fi+fm3hO9sbcRV/wnATFISP04OBIerT4hHwyMzjyWUx2B8K5yzxeloTDbqE5Pi75df1Bl3tSojxm6DxBECAwEAAaMTMBEwDwYDVR0RBAgwBocEfwAAATANBgkqhkiG9w0BAQsFAAOCAQEACTOlS/HTrmHKiXO/IIXgvNdhIE6uPBVoF8dU9+qhyz4WxMu1z0Sfqum1pkJDBDNRbt8L3EAQhgzM/2N4YUy+6CpMLAWANo4ij0PciojS9/ghvzaANUbNcILpjToz70PxKAgQfyWMLgMGbgLgSonai0q+QI3lJvApvW5FMG6D2e4mZCTKDf9XDEOqe/ace2As3dJkn82+Q90kfOCHlz8X0rBGzZq5HFCt2qCEMdYm0HY0Ktw6rsH5yrbs+N9TJ3u7fo1vSEwVV/GThtH9JR2gGyO5wCzUBjVEyosgtF90+E+De2FZulqysMozbH+LUtW8LMeMfokwK4EHR9FBjfndqA==";
const TEST_KEY_DER: &str = "MIIEvwIBADANBgkqhkiG9w0BAQEFAASCBKkwggSlAgEAAoIBAQCy1mIpybMEvBVf6Xvhy+s64gzz92A9ZsxwsRSEiU0LhxRFCFp9BeJvujr3GjUxTs9AgxdOPU6OHxmvPBtOqlJQsSHLuX8G4Gxna1uLk5FmejtPQyhYungoH0Gw1fI9AeoLeGuvdLwC9fnbTnWHz4Dr+pdAqMuaMamsqESPzOL9LYYBQhCcoREUl2rH/5c0sdlztR4rjXEzOzC3Rxks6bGuulWUv2pv27GygQFzGlo91LD5/hOZSnGkjmtnIYF2pCV0Ji+hYvn5t4TvbG3EVf8JwExSEj9ODgSHq0+IR8MjM48llMdgfCucs8XpaEw26hOT4u+XX9QZd7UqI8Zug8QRAgMBAAECggEBAIAoe/5ASebptlOeaaWdUbxHxEqNC03VPkq/y9lS34CUU6VI4DfaILQ6fAkaoeXs+T7c8rWh34qfpPNcGqGcExM6bOKm0u4lo+nVGKyEmt0aWShrEx3Ku1LdW2ETYN3xYjzIFjuNZzKj/WL47ebegCAb24p9rDKaxmIxz7hRdpVBd93oxZa47jbF0TkNrEZaiyrGkr6DlAk+25z9/gF05qntI7M81hTZawp3nXZ5DBfqXcbH3pMq1MSmczcVJxbMjk735TqHHckEjAGuW+Y59cCihR4rh/6o4SFZ1WVRRk9T57D0C+U1pp4acn3LGQhE6u2CPy+x4tzaYYxg7bGRikECgYEA5giFQkC+eWBO/tG6QsRq8dWdGaNhAjKoKTZA1KSti11KkvGnoNS9AmbTlSTgzu5I9GX8jqxR9PVuZtZr9Yr6CZdqY2i0zmDgUNN7/OAbI30QzmnzbsgkJZa/e3H/v4EJ9LMi8w1omTxacOAWbJiHtcOCfOVn+/QaFVJOev8xhHkCgYEAxwZpB2vepBytgK0OKk8FsA+BiGNQI6Ub/nkE8XKVqcWtaWHsxH1B3YDUqgwMaGzlcXhHgEEK6U0HsmEvvkEPSYmjsskM1toGl+AIPePX+p8oMnqzmbxCZ64ov62cLt3PaF7mNjGF/IckdgHkfIfjh3YzvHkWg2h/TZik5WGI5lkCgYEA3KFLfvIuPqha3Bk4NxXBJVanKZIEV2FS3MRGhi20rji6cBoLlzy0VHtfcGtAm/j8TD0NcaJhsTs9urDqN0Ym79AkoFgrIs7UF3HgN/iSzwUDe5cvfw/Da7Ic0j/S9lDDxcmTOd+gdWjnrd+gYmQhtfphS32UsJm98rlQwLPHQLECgYAlfuysrELu0kRR2Mixad/dcp5pzqQbgxDKGYy33Gmb6ZUpJHzR6/NLwujN/KUdy15SyWFXJWnj2FJZ5ftzsZgqt5ayqTQVClBxrpB+8H0RR4jwMbPCg/hSxjoBGrkxDzLzK+XdUek3UVKqNOMSHxvbuoY2vO1j5n0NZnOyj3SWSQKBgQCYyTFeDE9SsuyLoVAG8v4bnUdcTZpsRc4SiRGkirc2BPDeU3Fsq2Nh0Z+LALsHklHypzhs8G9RVl3Gq70n1Lf8qEqiM3V1gzAzncJpwMcFkZLd5WUofe13pyWK7Qirw7A6CK+mDXDwFrPHBQ3i31nW/72ImJ+291xzTob6YLxiCg==";

fn test_repo() -> TestRepo {
    let repo = TestRepo::new();
    // Tests create synthetic commits; they must not invoke the developer's
    // global signing helper or pinentry.
    repo.git(&["config", "commit.gpgSign", "false"]);
    repo
}

async fn execute(
    services: &RepoServices,
    target: &PullCheckoutTarget,
    plan: &PullCheckoutInspection,
) -> Result<PullCheckoutReceipt, PullCheckoutError> {
    let mut prepared = services
        .pull_checkout()
        .prepare_execution(target, plan, OPERATION_ID)
        .await?;
    if prepared.needs_fetch() {
        prepared.fetch().await?;
    }
    prepared.finish().await
}

async fn target(
    repo: &TestRepo,
    remote_name: &str,
    source_branch: &str,
    expected_oid: &str,
    local_branch: &str,
) -> PullCheckoutTarget {
    let services = RepoServices::new(repo.path_str()).unwrap();
    let snapshot = services.remotes().snapshot().await.unwrap();
    let remote = snapshot
        .remotes
        .iter()
        .find(|remote| remote.name == remote_name)
        .unwrap();
    let url = remote.fetch_urls.first().unwrap();
    PullCheckoutTarget {
        remote_name: remote_name.into(),
        remote_ordinal: url.ordinal,
        remote_endpoint: url.endpoint.clone().unwrap(),
        remote_digest: snapshot.semantic_digest,
        source_branch: source_branch.into(),
        expected_oid: expected_oid.into(),
        local_branch: local_branch.into(),
    }
}

fn local_repo() -> TestRepo {
    let repo = test_repo();
    repo.commit_file("README.md", "base", "base");
    repo.git(&[
        "remote",
        "add",
        "origin",
        "https://github.com/owner/project.git",
    ]);
    repo
}

#[cfg(unix)]
fn marker_script(directory: &Path, name: &str, extra: &str) -> (PathBuf, PathBuf) {
    use std::os::unix::fs::PermissionsExt;

    let script = directory.join(name);
    let marker = directory.join(format!("{name}-called"));
    assert!(!marker.to_string_lossy().contains('\''));
    std::fs::write(
        &script,
        format!("#!/bin/sh\n: > '{}'\n{extra}\n", marker.display()),
    )
    .unwrap();
    let mut permissions = std::fs::metadata(&script).unwrap().permissions();
    permissions.set_mode(0o700);
    std::fs::set_permissions(&script, permissions).unwrap();
    (script, marker)
}

struct DumbHttpsGitServer {
    address: std::net::SocketAddr,
    requests: Arc<AtomicUsize>,
    stop: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl DumbHttpsGitServer {
    fn start(repository: &Path, delay: Option<(PathBuf, PathBuf)>) -> Self {
        let status = Command::new("git")
            .current_dir(repository)
            .arg("update-server-info")
            .status()
            .unwrap();
        assert!(status.success(), "could not prepare dumb HTTP repository");

        let certificate = rustls::pki_types::CertificateDer::from(
            base64::engine::general_purpose::STANDARD
                .decode(TEST_CERT_DER)
                .unwrap(),
        );
        let key =
            rustls::pki_types::PrivateKeyDer::Pkcs8(rustls::pki_types::PrivatePkcs8KeyDer::from(
                base64::engine::general_purpose::STANDARD
                    .decode(TEST_KEY_DER)
                    .unwrap(),
            ));
        let tls = Arc::new(
            rustls::ServerConfig::builder()
                .with_no_client_auth()
                .with_single_cert(vec![certificate], key)
                .unwrap(),
        );
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let address = listener.local_addr().unwrap();
        let stop = Arc::new(AtomicBool::new(false));
        let requests = Arc::new(AtomicUsize::new(0));
        let server_stop = stop.clone();
        let server_requests = requests.clone();
        let repository = repository.to_path_buf();
        let thread = std::thread::spawn(move || {
            let delayed = AtomicBool::new(false);
            while !server_stop.load(Ordering::SeqCst) {
                let stream = match listener.accept() {
                    Ok((stream, _)) => stream,
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(Duration::from_millis(5));
                        continue;
                    }
                    Err(error) => panic!("HTTPS Git fixture accept failed: {error}"),
                };
                stream.set_nonblocking(false).unwrap();
                server_requests.fetch_add(1, Ordering::SeqCst);
                let connection = rustls::ServerConnection::new(tls.clone()).unwrap();
                let mut stream = rustls::StreamOwned::new(connection, stream);
                let _ = serve_dumb_https_request(
                    &mut stream,
                    &repository,
                    delay.as_ref(),
                    &delayed,
                    &server_stop,
                );
            }
        });
        Self {
            address,
            requests,
            stop,
            thread: Some(thread),
        }
    }

    fn url(&self) -> String {
        format!("https://{}/fork.git", self.address)
    }

    fn requests(&self) -> usize {
        self.requests.load(Ordering::SeqCst)
    }
}

impl Drop for DumbHttpsGitServer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(thread) = self.thread.take() {
            thread.join().unwrap();
        }
    }
}

fn serve_dumb_https_request(
    stream: &mut rustls::StreamOwned<rustls::ServerConnection, TcpStream>,
    repository: &Path,
    delay: Option<&(PathBuf, PathBuf)>,
    delayed: &AtomicBool,
    stop: &AtomicBool,
) -> std::io::Result<()> {
    let mut request = Vec::new();
    let mut buffer = [0_u8; 4096];
    while !request.windows(4).any(|window| window == b"\r\n\r\n") {
        let read = stream.read(&mut buffer)?;
        if read == 0 || request.len() + read > 16 * 1024 {
            return Ok(());
        }
        request.extend_from_slice(&buffer[..read]);
    }
    let request = std::str::from_utf8(&request).unwrap_or("");
    let mut fields = request.lines().next().unwrap_or("").split_whitespace();
    let method = fields.next().unwrap_or("");
    let route = fields.next().unwrap_or("").split('?').next().unwrap_or("");
    let Some(relative) = route.strip_prefix("/fork.git/") else {
        return write_https_response(stream, 404, "text/plain", b"", method == "HEAD");
    };
    let path = Path::new(relative);
    if path
        .components()
        .any(|component| !matches!(component, Component::Normal(_)))
    {
        return write_https_response(stream, 404, "text/plain", b"", method == "HEAD");
    }
    if relative == "info/refs"
        && !delayed.swap(true, Ordering::SeqCst)
        && let Some((started, release)) = delay
    {
        std::fs::write(started, "started")?;
        while !release.exists() && !stop.load(Ordering::SeqCst) {
            std::thread::sleep(Duration::from_millis(5));
        }
    }
    if !matches!(method, "GET" | "HEAD") {
        return write_https_response(stream, 405, "text/plain", b"", false);
    }
    match std::fs::read(repository.join(path)) {
        Ok(body) => write_https_response(
            stream,
            200,
            if relative == "info/refs" || relative == "HEAD" {
                "text/plain"
            } else {
                "application/octet-stream"
            },
            &body,
            method == "HEAD",
        ),
        Err(_) => write_https_response(stream, 404, "text/plain", b"", method == "HEAD"),
    }
}

fn write_https_response(
    stream: &mut rustls::StreamOwned<rustls::ServerConnection, TcpStream>,
    status: u16,
    content_type: &str,
    body: &[u8],
    head_only: bool,
) -> std::io::Result<()> {
    let reason = if status == 200 { "OK" } else { "Not Found" };
    write!(
        stream,
        "HTTP/1.1 {status} {reason}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    )?;
    if !head_only {
        stream.write_all(body)?;
    }
    stream.flush()
}

#[test]
fn creates_and_verifies_a_branch_from_an_existing_exact_object() {
    run_async(async {
        let repo = local_repo();
        let oid = repo.head_commit();
        let services = RepoServices::new(repo.path_str()).unwrap();
        let target = target(&repo, "origin", "feature", &oid, "pr/1").await;
        let before = repo.git(&["config", "--get-regexp", "^remote\\."]);

        let plan = services.pull_checkout().inspect(&target).await.unwrap();
        assert_eq!(plan.action, Some(PullCheckoutAction::CreateBranch));
        let receipt = execute(&services, &target, &plan).await.unwrap();

        assert_eq!(receipt.branch, "pr/1");
        assert_eq!(receipt.oid, oid);
        assert!(!receipt.fetched);
        assert!(!receipt.git_reported_failure);
        assert_eq!(repo.current_branch(), "pr/1");
        assert_eq!(repo.git(&["config", "--get-regexp", "^remote\\."]), before);
    });
}

#[test]
fn annotated_tag_oid_is_not_treated_as_the_exact_commit_object() {
    run_async(async {
        let repo = local_repo();
        let commit_oid = repo.head_commit();
        repo.git(&[
            "-c",
            "tag.gpgSign=false",
            "tag",
            "-a",
            "provider-tag",
            "-m",
            "provider tag",
        ]);
        let tag_oid = repo.git(&["rev-parse", "refs/tags/provider-tag"]);
        assert_ne!(tag_oid, commit_oid);
        assert_eq!(repo.git(&["cat-file", "-t", &tag_oid]), "tag");

        let services = RepoServices::new(repo.path_str()).unwrap();
        let target = target(&repo, "origin", "feature", &tag_oid, "pr/tag").await;
        let plan = services.pull_checkout().inspect(&target).await.unwrap();

        assert!(!plan.object_available);
        assert_eq!(plan.action, Some(PullCheckoutAction::FetchAndCreateBranch));
        assert_eq!(repo.current_branch(), "main");
        assert!(!repo.list_branches().contains(&"pr/tag".to_string()));
    });
}

#[test]
fn switches_only_an_existing_branch_at_the_exact_oid() {
    run_async(async {
        let repo = local_repo();
        let oid = repo.head_commit();
        repo.git(&["branch", "pr/2", &oid]);
        let services = RepoServices::new(repo.path_str()).unwrap();
        let target = target(&repo, "origin", "feature", &oid, "pr/2").await;

        let plan = services.pull_checkout().inspect(&target).await.unwrap();
        assert_eq!(plan.action, Some(PullCheckoutAction::SwitchExisting));
        execute(&services, &target, &plan).await.unwrap();
        assert_eq!(repo.current_branch(), "pr/2");
        assert_eq!(repo.head_commit(), oid);
    });
}

#[test]
fn symbolic_existing_branch_is_rejected_before_switching_head() {
    run_async(async {
        let repo = local_repo();
        let expected = repo.head_commit();
        repo.git(&["branch", "real-target", &expected]);
        repo.git(&[
            "symbolic-ref",
            "refs/heads/pr/symbolic",
            "refs/heads/real-target",
        ]);
        let services = RepoServices::new(repo.path_str()).unwrap();
        let target = target(&repo, "origin", "feature", &expected, "pr/symbolic").await;
        let plan = services.pull_checkout().inspect(&target).await.unwrap();
        assert_eq!(plan.action, Some(PullCheckoutAction::SwitchExisting));

        assert_eq!(
            execute(&services, &target, &plan).await.unwrap_err(),
            PullCheckoutError::StalePlan
        );
        assert_eq!(repo.current_branch(), "main");
        assert_eq!(repo.head_commit(), expected);
        assert_eq!(
            repo.git(&["symbolic-ref", "refs/heads/pr/symbolic"]),
            "refs/heads/real-target"
        );
    });
}

#[test]
fn existing_branch_lock_ignores_unbounded_repository_lock_timeouts() {
    run_async(async {
        let repo = local_repo();
        let expected = repo.head_commit();
        repo.git(&["branch", "pr/held-lock", &expected]);
        let services = RepoServices::new(repo.path_str()).unwrap();
        let target = target(&repo, "origin", "feature", &expected, "pr/held-lock").await;
        let plan = services.pull_checkout().inspect(&target).await.unwrap();
        assert_eq!(plan.action, Some(PullCheckoutAction::SwitchExisting));

        for key in [
            "core.filesRefLockTimeout",
            "core.packedRefsTimeout",
            "reftable.lockTimeout",
        ] {
            repo.git(&["config", key, "-1"]);
        }
        let lock_path = repo.path().join(".git/refs/heads/pr/held-lock.lock");
        std::fs::write(&lock_path, "held by another process").unwrap();

        // Full execution repeats the repository inspection before touching the
        // ref lock. Its many process starts are not a lock-wait measurement on
        // a loaded runner. The runner's focused prepare_ref_lock regression
        // separately requires a real lock conflict, never its timeout fallback.
        let result = execute(&services, &target, &plan).await;
        assert_eq!(result.unwrap_err(), PullCheckoutError::StalePlan);
        assert_eq!(repo.current_branch(), "main");
        assert_eq!(repo.head_commit(), expected);
        assert_eq!(
            std::fs::read_to_string(lock_path).unwrap(),
            "held by another process"
        );
    });
}

#[test]
fn switches_an_existing_branch_in_a_sha256_repository_when_supported() {
    run_async(async {
        let dir = tempfile::tempdir().unwrap();
        let initialized = Command::new("git")
            .current_dir(dir.path())
            .args(["init", "-b", "main", "--object-format=sha256"])
            .output()
            .unwrap();
        if !initialized.status.success() {
            return;
        }
        let repo = TestRepo { dir };
        repo.git(&["config", "user.email", "test@example.com"]);
        repo.git(&["config", "user.name", "Test User"]);
        repo.git(&["config", "commit.gpgSign", "false"]);
        repo.commit_file("README.md", "base", "base");
        repo.git(&[
            "remote",
            "add",
            "origin",
            "https://github.com/owner/project.git",
        ]);
        let oid = repo.head_commit();
        assert_eq!(oid.len(), 64);
        repo.git(&["branch", "pr/sha256", &oid]);

        let services = RepoServices::new(repo.path_str()).unwrap();
        let target = target(&repo, "origin", "feature", &oid, "pr/sha256").await;
        let plan = services.pull_checkout().inspect(&target).await.unwrap();
        assert_eq!(plan.action, Some(PullCheckoutAction::SwitchExisting));

        let receipt = execute(&services, &target, &plan).await.unwrap();
        assert_eq!(receipt.branch, "pr/sha256");
        assert_eq!(receipt.oid, oid);
        assert!(!receipt.git_reported_failure);
        assert_eq!(repo.current_branch(), "pr/sha256");
    });
}

#[test]
fn divergent_existing_branch_and_dirty_worktree_are_never_mutated() {
    run_async(async {
        let repo = local_repo();
        let expected = repo.head_commit();
        repo.commit_file("next.txt", "next", "next");
        let divergent = repo.head_commit();
        repo.git(&["branch", "pr/3", &divergent]);
        let services = RepoServices::new(repo.path_str()).unwrap();
        let divergent_target = target(&repo, "origin", "feature", &expected, "pr/3").await;
        let plan = services
            .pull_checkout()
            .inspect(&divergent_target)
            .await
            .unwrap();
        assert_eq!(plan.action, None);
        assert_eq!(
            plan.blocker,
            Some(PullCheckoutBlocker::ExistingBranchDiverged)
        );
        assert_eq!(repo.current_branch(), "main");
        assert_eq!(repo.git(&["rev-parse", "pr/3"]), divergent);

        let clean_target = target(&repo, "origin", "feature", &expected, "pr/4").await;
        repo.create_file("dirty.txt", "unsaved");
        let dirty = services
            .pull_checkout()
            .inspect(&clean_target)
            .await
            .unwrap();
        assert_eq!(dirty.action, None);
        assert_eq!(dirty.blocker, Some(PullCheckoutBlocker::DirtyWorktree));
    });
}

#[test]
fn detached_head_is_explicit_and_a_changed_worktree_invalidates_the_plan() {
    run_async(async {
        let repo = local_repo();
        let expected = repo.head_commit();
        repo.git(&["switch", "--detach", &expected]);
        let services = RepoServices::new(repo.path_str()).unwrap();
        let target = target(&repo, "origin", "feature", &expected, "pr/5").await;
        let plan = services.pull_checkout().inspect(&target).await.unwrap();
        assert!(plan.detached);
        assert_eq!(plan.action, Some(PullCheckoutAction::CreateBranch));

        repo.git(&["switch", "main"]);
        assert_eq!(
            execute(&services, &target, &plan).await.unwrap_err(),
            PullCheckoutError::StalePlan
        );
        assert!(!repo.list_branches().contains(&"pr/5".to_string()));
    });
}

#[test]
fn checkout_failure_is_fixed_and_preserves_the_current_branch() {
    run_async(async {
        let repo = local_repo();
        let oid = repo.head_commit();
        repo.git(&["branch", "pr/locked", &oid]);
        let linked = tempfile::tempdir().unwrap();
        let linked_path = linked.path().join("worktree");
        repo.git(&[
            "worktree",
            "add",
            linked_path.to_str().unwrap(),
            "pr/locked",
        ]);
        let services = RepoServices::new(repo.path_str()).unwrap();
        let target = target(&repo, "origin", "feature", &oid, "pr/locked").await;
        let plan = services.pull_checkout().inspect(&target).await.unwrap();
        assert_eq!(plan.action, Some(PullCheckoutAction::SwitchExisting));

        let error = execute(&services, &target, &plan).await.unwrap_err();
        assert_eq!(error, PullCheckoutError::CheckoutFailed);
        assert_eq!(repo.current_branch(), "main");
    });
}

#[cfg(unix)]
#[test]
fn existing_branch_lock_blocks_hook_race_and_reports_verified_hook_failure() {
    use std::os::unix::fs::PermissionsExt;

    run_async(async {
        let repo = local_repo();
        let expected = repo.head_commit();
        repo.git(&["branch", "pr/hook-race", &expected]);
        repo.commit_file("later.txt", "later", "later main commit");
        let raced_oid = repo.head_commit();
        assert_ne!(expected, raced_oid);

        let hook = repo.path().join(".git/hooks/post-checkout");
        std::fs::write(
            &hook,
            format!("#!/bin/sh\ngit update-ref refs/heads/pr/hook-race {raced_oid} {expected}\n"),
        )
        .unwrap();
        let mut permissions = std::fs::metadata(&hook).unwrap().permissions();
        permissions.set_mode(0o700);
        std::fs::set_permissions(&hook, permissions).unwrap();

        let services = RepoServices::new(repo.path_str()).unwrap();
        let target = target(&repo, "origin", "feature", &expected, "pr/hook-race").await;
        let plan = services.pull_checkout().inspect(&target).await.unwrap();
        assert_eq!(plan.action, Some(PullCheckoutAction::SwitchExisting));

        let receipt = execute(&services, &target, &plan).await.unwrap();
        assert!(receipt.git_reported_failure);
        assert_eq!(receipt.branch, "pr/hook-race");
        assert_eq!(receipt.oid, expected);
        assert_eq!(repo.current_branch(), "pr/hook-race");
        assert_eq!(repo.head_commit(), expected);
        assert_eq!(
            repo.git(&["rev-parse", "refs/heads/pr/hook-race"]),
            expected
        );
    });
}

#[cfg(unix)]
#[test]
fn internal_ref_lock_does_not_run_reference_transaction_hooks() {
    use std::os::unix::fs::PermissionsExt;

    run_async(async {
        let repo = local_repo();
        let expected = repo.head_commit();
        repo.git(&["branch", "pr/ref-hook", &expected]);
        repo.commit_file("later.txt", "later", "later main commit");
        let raced_oid = repo.head_commit();
        let marker = repo.path().join("reference-transaction-aborted");
        assert!(!marker.to_string_lossy().contains('\''));

        let hook = repo.path().join(".git/hooks/reference-transaction");
        std::fs::write(
            &hook,
            format!(
                "#!/bin/sh\npayload=$(cat)\ncase \"$1:$payload\" in\n  aborted:*refs/heads/pr/ref-hook*)\n    : > '{}'\n    git update-ref refs/heads/pr/ref-hook {raced_oid} {expected}\n    ;;\nesac\n",
                marker.display()
            ),
        )
        .unwrap();
        let mut permissions = std::fs::metadata(&hook).unwrap().permissions();
        permissions.set_mode(0o700);
        std::fs::set_permissions(&hook, permissions).unwrap();

        let services = RepoServices::new(repo.path_str()).unwrap();
        let target = target(&repo, "origin", "feature", &expected, "pr/ref-hook").await;
        let plan = services.pull_checkout().inspect(&target).await.unwrap();
        let receipt = execute(&services, &target, &plan).await.unwrap();

        assert!(!receipt.git_reported_failure);
        assert_eq!(receipt.branch, "pr/ref-hook");
        assert_eq!(receipt.oid, expected);
        assert!(!marker.exists());
        assert_eq!(repo.git(&["rev-parse", "refs/heads/pr/ref-hook"]), expected);
    });
}

#[test]
fn active_merge_blocks_checkout_before_dirty_state() {
    run_async(async {
        let repo = local_repo();
        let expected = repo.head_commit();
        repo.create_branch("other");
        repo.create_file("README.md", "other");
        repo.add("README.md");
        repo.commit("other");
        repo.switch_branch("main");
        repo.create_file("README.md", "main");
        repo.add("README.md");
        repo.commit("main");
        let status = Command::new("git")
            .current_dir(repo.path())
            .args(["merge", "other"])
            .status()
            .unwrap();
        assert!(!status.success());

        let services = RepoServices::new(repo.path_str()).unwrap();
        let target = target(&repo, "origin", "feature", &expected, "pr/6").await;
        let plan = services.pull_checkout().inspect(&target).await.unwrap();
        assert_eq!(plan.action, None);
        assert_eq!(plan.blocker, Some(PullCheckoutBlocker::ActiveOperation));
        repo.git(&["merge", "--abort"]);
    });
}

#[test]
fn unmerged_index_without_an_operation_marker_is_still_blocked_as_active() {
    run_async(async {
        let repo = local_repo();
        repo.create_file("README.md", "stashed change");
        repo.git(&["stash", "push", "-m", "conflicting stash"]);
        repo.create_file("README.md", "committed change");
        repo.add("README.md");
        repo.commit("conflicting commit");
        let expected = repo.head_commit();
        let status = Command::new("git")
            .current_dir(repo.path())
            .args(["stash", "apply"])
            .status()
            .unwrap();
        assert!(!status.success());
        assert!(!repo.path().join(".git/MERGE_HEAD").exists());
        assert!(
            repo.git(&["status", "--porcelain"])
                .starts_with("UU README.md")
        );

        let services = RepoServices::new(repo.path_str()).unwrap();
        let target = target(&repo, "origin", "feature", &expected, "pr/unmerged").await;
        let plan = services.pull_checkout().inspect(&target).await.unwrap();

        assert_eq!(plan.action, None);
        assert_eq!(plan.blocker, Some(PullCheckoutBlocker::ActiveOperation));
        assert_eq!(repo.current_branch(), "main");
    });
}

#[test]
fn refspec_and_option_like_inputs_are_rejected_without_creating_refs() {
    run_async(async {
        let repo = local_repo();
        let oid = repo.head_commit();
        let services = RepoServices::new(repo.path_str()).unwrap();

        let mut injection =
            target(&repo, "origin", "feature:refs/heads/injected", &oid, "pr/7").await;
        assert_eq!(
            services
                .pull_checkout()
                .inspect(&injection)
                .await
                .unwrap_err(),
            PullCheckoutError::InvalidTarget
        );
        assert!(!repo.list_branches().contains(&"injected".to_string()));

        injection.source_branch = "feature".into();
        injection.local_branch = "--discard-changes".into();
        assert_eq!(
            services
                .pull_checkout()
                .inspect(&injection)
                .await
                .unwrap_err(),
            PullCheckoutError::InvalidTarget
        );
        injection.local_branch = "pr/7".into();
        injection.remote_name = "--upload-pack=credential-helper".into();
        assert_eq!(
            services
                .pull_checkout()
                .inspect(&injection)
                .await
                .unwrap_err(),
            PullCheckoutError::InvalidTarget
        );

        injection.remote_name = "origin".into();
        injection.local_branch = format!("pr/{}", "a".repeat(256));
        assert_eq!(
            services
                .pull_checkout()
                .inspect(&injection)
                .await
                .unwrap_err(),
            PullCheckoutError::InvalidTarget
        );
        injection.local_branch = "pr/7".into();
        injection.source_branch = format!("feature/{}", "a".repeat(256));
        assert_eq!(
            services
                .pull_checkout()
                .inspect(&injection)
                .await
                .unwrap_err(),
            PullCheckoutError::InvalidTarget
        );
    });
}

#[test]
fn authority_revoked_while_waiting_for_the_repository_lock_causes_no_mutation() {
    run_async(async {
        let repo = local_repo();
        let oid = repo.head_commit();
        let services = Arc::new(RepoServices::new(repo.path_str()).unwrap());
        let target = target(&repo, "origin", "feature", &oid, "pr/queued").await;
        let plan = services.pull_checkout().inspect(&target).await.unwrap();
        let runner = GitCommandRunner::new(repo.path_str()).unwrap();
        let held = runner.transaction().await.unwrap();
        let authorized = Arc::new(AtomicBool::new(true));
        let mut queued = tokio::spawn({
            let services = services.clone();
            let target = target.clone();
            let plan = plan.clone();
            async move {
                services
                    .pull_checkout()
                    .prepare_execution(&target, &plan, OPERATION_ID)
                    .await
            }
        });

        assert!(
            tokio::time::timeout(Duration::from_millis(50), &mut queued)
                .await
                .is_err(),
            "prepare should wait behind the existing repository transaction"
        );
        authorized.store(false, Ordering::SeqCst);
        drop(held);
        let prepared = queued.await.unwrap().unwrap();

        // This is the same lock-bound admission shape used by the Tauri
        // command: authority is checked only after prepare owns the guard.
        if !authorized.load(Ordering::SeqCst) {
            drop(prepared);
        } else {
            panic!("authority should have been revoked while queued");
        }
        assert_eq!(repo.current_branch(), "main");
        assert_eq!(repo.head_commit(), oid);
        assert!(!repo.list_branches().contains(&"pr/queued".to_string()));
        assert!(
            repo.git(&["for-each-ref", "--format=%(refname)", "refs/gitru/"])
                .is_empty()
        );
    });
}

#[test]
fn uppercase_expected_oid_is_rejected_before_branch_or_worktree_mutation() {
    run_async(async {
        let repo = local_repo();
        let oid = repo.head_commit();
        let services = RepoServices::new(repo.path_str()).unwrap();
        let mut target = target(&repo, "origin", "feature", &oid, "pr/uppercase").await;
        target.expected_oid = oid.to_ascii_uppercase();

        assert_eq!(
            services.pull_checkout().inspect(&target).await.unwrap_err(),
            PullCheckoutError::InvalidTarget
        );
        assert_eq!(repo.current_branch(), "main");
        assert_eq!(repo.head_commit(), oid);
        assert!(!repo.list_branches().contains(&"pr/uppercase".to_string()));
    });
}

#[test]
fn repository_replace_refs_cannot_change_the_checked_out_tree() {
    run_async(async {
        let repo = test_repo();
        repo.commit_file("README.md", "expected tree", "expected");
        let expected = repo.head_commit();
        repo.commit_file("README.md", "replacement tree", "replacement");
        let replacement = repo.head_commit();
        repo.git(&["replace", &expected, &replacement]);
        repo.git(&[
            "remote",
            "add",
            "origin",
            "https://github.com/owner/project.git",
        ]);
        let services = RepoServices::new(repo.path_str()).unwrap();
        let target = target(&repo, "origin", "feature", &expected, "pr/no-replace").await;

        let plan = services.pull_checkout().inspect(&target).await.unwrap();
        assert_eq!(plan.action, Some(PullCheckoutAction::CreateBranch));
        let receipt = execute(&services, &target, &plan).await.unwrap();

        assert_eq!(receipt.oid, expected);
        assert_eq!(repo.current_branch(), "pr/no-replace");
        assert_eq!(
            std::fs::read_to_string(repo.path().join("README.md")).unwrap(),
            "expected tree"
        );
        assert_eq!(repo.git(&["replace", "-l"]), expected);
    });
}

#[test]
#[cfg(unix)]
fn inline_https_credentials_are_rejected_before_transport_or_helpers() {
    run_async(async {
        let source = test_repo();
        source.commit_file("README.md", "source", "source");
        let bare = source.setup_remote();
        let server = DumbHttpsGitServer::start(bare.path(), None);
        let repo = test_repo();
        repo.commit_file("README.md", "base", "base");
        let helpers = tempfile::tempdir().unwrap();
        let (credential_helper, helper_marker) = marker_script(
            helpers.path(),
            "credential-helper",
            "printf 'username=unexpected\\npassword=unexpected\\n'",
        );
        let credential_config = format!("!{}", credential_helper.display());
        repo.git(&["config", "credential.helper", &credential_config]);
        let remote = format!(
            "https://synthetic-user:synthetic-secret@{}/fork.git?token=synthetic-query#synthetic-fragment",
            server.address
        );
        repo.git(&["remote", "add", "fork", &remote]);
        let services = RepoServices::new(repo.path_str()).unwrap();
        let expected = "a".repeat(40);
        let target = target(&repo, "fork", "feature", &expected, "pr/8").await;
        let error = services.pull_checkout().inspect(&target).await.unwrap_err();
        assert_eq!(error, PullCheckoutError::UnsupportedCredentials);
        let rendered = error.to_string();
        assert!(!rendered.contains("synthetic"));
        assert_eq!(server.requests(), 0);
        assert!(!helper_marker.exists());
        assert!(
            repo.git(&["for-each-ref", "--format=%(refname)", "refs/gitru/"])
                .is_empty()
        );
        assert!(!repo.list_branches().contains(&"pr/8".to_string()));
    });
}

#[test]
fn selected_remote_with_custom_transport_is_rejected_before_network() {
    run_async(async {
        let source = test_repo();
        source.commit_file("README.md", "source", "source");
        let bare = source.setup_remote();
        let server = DumbHttpsGitServer::start(bare.path(), None);
        let repo = test_repo();
        repo.commit_file("README.md", "local", "local");
        repo.git(&["config", "http.sslVerify", "false"]);
        repo.git(&["remote", "add", "fork", &server.url()]);
        let expected = "a".repeat(40);
        let target = target(&repo, "fork", "feature", &expected, "pr/custom-vcs").await;
        repo.git(&["config", "remote.fork.vcs", "ssh"]);

        let services = RepoServices::new(repo.path_str()).unwrap();
        assert_eq!(
            services.pull_checkout().inspect(&target).await.unwrap_err(),
            PullCheckoutError::RemoteChanged
        );
        assert_eq!(server.requests(), 0);
        assert!(!repo.list_branches().contains(&"pr/custom-vcs".to_string()));
    });
}

#[cfg(unix)]
#[test]
fn changed_ssh_username_invalidates_the_native_plan_without_transport() {
    run_async(async {
        let repo = test_repo();
        repo.commit_file("README.md", "local", "local");
        repo.git(&[
            "remote",
            "add",
            "fork",
            "ssh://alice@127.0.0.1:9/owner/fork.git",
        ]);
        let services = RepoServices::new(repo.path_str()).unwrap();
        let expected = "a".repeat(40);
        let target = target(&repo, "fork", "feature", &expected, "pr/ssh-user").await;
        let plan = services.pull_checkout().inspect(&target).await.unwrap();
        assert_eq!(plan.action, Some(PullCheckoutAction::FetchAndCreateBranch));

        let fixture = tempfile::tempdir().unwrap();
        let (ssh_command, ssh_marker) = marker_script(fixture.path(), "ssh-command", "exit 97");
        repo.git(&[
            "remote",
            "set-url",
            "fork",
            "ssh://bob@127.0.0.1:9/owner/fork.git",
        ]);
        repo.git(&["config", "core.sshCommand", ssh_command.to_str().unwrap()]);

        let error = services
            .pull_checkout()
            .prepare_execution(&target, &plan, OPERATION_ID)
            .await
            .err()
            .unwrap();
        assert_eq!(error, PullCheckoutError::StalePlan);
        assert!(!ssh_marker.exists());
        assert_eq!(repo.current_branch(), "main");
        assert!(!repo.list_branches().contains(&"pr/ssh-user".to_string()));
    });
}

#[test]
fn missing_fork_object_is_fetched_exactly_without_updating_fetch_refs() {
    run_async(async {
        let source = test_repo();
        source.commit_file("README.md", "base", "base");
        let bare = source.setup_remote();
        source.create_branch("feature");
        source.commit_file("feature.txt", "fork", "fork head");
        let expected = source.head_commit();
        source.git(&["push", "origin", "feature"]);

        let repo = test_repo();
        repo.commit_file("README.md", "local", "local base");
        let server = DumbHttpsGitServer::start(bare.path(), None);
        repo.git(&["config", "http.sslVerify", "false"]);
        repo.git(&["remote", "add", "fork", &server.url()]);

        let services = RepoServices::new(repo.path_str()).unwrap();
        let target = target(&repo, "fork", "feature", &expected, "pr/9").await;
        let plan = services.pull_checkout().inspect(&target).await.unwrap();
        assert!(!plan.object_available);
        assert_eq!(plan.action, Some(PullCheckoutAction::FetchAndCreateBranch));
        let refs_before = repo.git(&["for-each-ref", "--format=%(refname) %(objectname)"]);
        let mut prepared = services
            .pull_checkout()
            .prepare_execution(&target, &plan, OPERATION_ID)
            .await
            .unwrap();
        prepared.fetch().await.unwrap();
        assert_eq!(
            repo.git(&["for-each-ref", "--format=%(refname) %(objectname)"]),
            refs_before
        );
        assert!(!repo.path().join(".git/FETCH_HEAD").exists());
        let receipt = prepared.finish().await.unwrap();
        assert!(receipt.fetched);
        assert_eq!(receipt.oid, expected);
        assert_eq!(repo.current_branch(), "pr/9");
        assert!(
            repo.git(&["for-each-ref", "--format=%(refname)", "refs/gitru/"])
                .is_empty()
        );
    });
}

#[cfg(unix)]
#[test]
fn raced_remote_vcs_cannot_select_ssh_or_invoke_repository_ssh_command() {
    run_async(async {
        let source = test_repo();
        source.commit_file("README.md", "base", "base");
        let bare = source.setup_remote();
        source.create_branch("feature");
        source.commit_file("feature.txt", "fork", "fork head");
        let expected = source.head_commit();
        source.git(&["push", "origin", "feature"]);

        let repo = test_repo();
        repo.commit_file("README.md", "local", "local base");
        let fixture = tempfile::tempdir().unwrap();
        let started = fixture.path().join("started");
        let release = fixture.path().join("release");
        let (ssh_command, ssh_marker) = marker_script(fixture.path(), "ssh-command", "exit 97");
        let server =
            DumbHttpsGitServer::start(bare.path(), Some((started.clone(), release.clone())));
        repo.git(&["config", "http.sslVerify", "false"]);
        repo.git(&["remote", "add", "fork", &server.url()]);

        let services = RepoServices::new(repo.path_str()).unwrap();
        let target = target(&repo, "fork", "feature", &expected, "pr/vcs-race").await;
        let plan = services.pull_checkout().inspect(&target).await.unwrap();
        let mut prepared = services
            .pull_checkout()
            .prepare_execution(&target, &plan, OPERATION_ID)
            .await
            .unwrap();
        let repo_path = repo.path().to_path_buf();
        let attacker = std::thread::spawn(move || {
            let deadline = std::time::Instant::now() + Duration::from_secs(5);
            while !started.exists() && std::time::Instant::now() < deadline {
                std::thread::sleep(Duration::from_millis(10));
            }
            assert!(started.exists(), "remote observation did not start");
            for (key, value) in [
                ("remote.fork.vcs", "ssh"),
                ("core.sshCommand", ssh_command.to_str().unwrap()),
            ] {
                let status = Command::new("git")
                    .current_dir(&repo_path)
                    .args(["config", key, value])
                    .status()
                    .unwrap();
                assert!(status.success(), "attacker could not set {key}");
            }
            std::fs::write(release, "continue").unwrap();
        });

        prepared.fetch().await.unwrap();
        attacker.join().unwrap();
        assert!(!ssh_marker.exists());
        repo.git(&["config", "--unset", "remote.fork.vcs"]);
        repo.git(&["config", "--unset", "core.sshCommand"]);
        let receipt = prepared.finish().await.unwrap();
        assert_eq!(receipt.oid, expected);
        assert_eq!(repo.current_branch(), "pr/vcs-race");
    });
}

#[cfg(unix)]
#[test]
fn raced_ext_rewrite_cannot_redirect_the_pinned_https_source() {
    run_async(async {
        let source = test_repo();
        source.commit_file("README.md", "base", "base");
        let bare = source.setup_remote();
        source.create_branch("feature");
        source.commit_file("feature.txt", "fork", "fork head");
        let expected = source.head_commit();
        source.git(&["push", "origin", "feature"]);

        let repo = test_repo();
        repo.commit_file("README.md", "local", "local base");
        let fixture = tempfile::tempdir().unwrap();
        let started = fixture.path().join("started");
        let release = fixture.path().join("release");
        let (remote_helper, helper_marker) =
            marker_script(fixture.path(), "remote-helper", "exit 97");
        let server =
            DumbHttpsGitServer::start(bare.path(), Some((started.clone(), release.clone())));
        let server_url = server.url();
        repo.git(&["config", "http.sslVerify", "false"]);
        repo.git(&["remote", "add", "fork", &server_url]);

        let services = RepoServices::new(repo.path_str()).unwrap();
        let target = target(&repo, "fork", "feature", &expected, "pr/rewrite-race").await;
        let plan = services.pull_checkout().inspect(&target).await.unwrap();
        let mut prepared = services
            .pull_checkout()
            .prepare_execution(&target, &plan, OPERATION_ID)
            .await
            .unwrap();
        let repo_path = repo.path().to_path_buf();
        let rewrite_key = format!("url.ext::{}.insteadOf", remote_helper.display());
        let attacker_key = rewrite_key.clone();
        let attacker = std::thread::spawn(move || {
            let deadline = std::time::Instant::now() + Duration::from_secs(5);
            while !started.exists() && std::time::Instant::now() < deadline {
                std::thread::sleep(Duration::from_millis(10));
            }
            assert!(started.exists(), "remote observation did not start");
            for value in [&server_url, "gitru-pin::"] {
                let status = Command::new("git")
                    .current_dir(&repo_path)
                    .args(["config", "--add", &attacker_key, value])
                    .status()
                    .unwrap();
                assert!(status.success(), "attacker could not install URL rewrite");
            }
            std::fs::write(release, "continue").unwrap();
        });

        prepared.fetch().await.unwrap();
        attacker.join().unwrap();
        assert!(!helper_marker.exists());
        repo.git(&["config", "--unset-all", &rewrite_key]);
        let receipt = prepared.finish().await.unwrap();
        assert_eq!(receipt.oid, expected);
        assert_eq!(repo.current_branch(), "pr/rewrite-race");
    });
}

#[test]
fn symbolic_ref_installed_during_fetch_cannot_move_its_victim() {
    run_async(async {
        let source = test_repo();
        source.commit_file("README.md", "base", "base");
        let bare = source.setup_remote();
        source.create_branch("feature");
        source.commit_file("feature.txt", "fork", "fork head");
        let expected = source.head_commit();
        source.git(&["push", "origin", "feature"]);

        let repo = test_repo();
        repo.commit_file("README.md", "local", "local base");
        repo.git(&["branch", "victim"]);
        let victim_before = repo.git(&["rev-parse", "refs/heads/victim"]);
        let helper = tempfile::tempdir().unwrap();
        let started = helper.path().join("started");
        let release = helper.path().join("release");
        let server =
            DumbHttpsGitServer::start(bare.path(), Some((started.clone(), release.clone())));
        repo.git(&["config", "http.sslVerify", "false"]);
        repo.git(&["remote", "add", "fork", &server.url()]);

        let services = RepoServices::new(repo.path_str()).unwrap();
        let target = target(&repo, "fork", "feature", &expected, "pr/symref-race").await;
        let plan = services.pull_checkout().inspect(&target).await.unwrap();
        let mut prepared = services
            .pull_checkout()
            .prepare_execution(&target, &plan, OPERATION_ID)
            .await
            .unwrap();
        let repo_path = repo.path().to_path_buf();
        let started_for_thread = started.clone();
        let release_for_thread = release.clone();
        let attacker = std::thread::spawn(move || {
            let deadline = std::time::Instant::now() + Duration::from_secs(5);
            while !started_for_thread.exists() && std::time::Instant::now() < deadline {
                std::thread::sleep(Duration::from_millis(10));
            }
            assert!(
                started_for_thread.exists(),
                "fetch helper did not start before the deadline"
            );
            let status = Command::new("git")
                .current_dir(repo_path)
                .args(["symbolic-ref", LEGACY_TEMPORARY_REF, "refs/heads/victim"])
                .status()
                .unwrap();
            assert!(status.success(), "attacker could not install symbolic ref");
            std::fs::write(release_for_thread, "continue").unwrap();
        });

        prepared.fetch().await.unwrap();
        attacker.join().unwrap();
        assert_eq!(
            repo.git(&["symbolic-ref", LEGACY_TEMPORARY_REF]),
            "refs/heads/victim"
        );
        assert_eq!(repo.git(&["rev-parse", "refs/heads/victim"]), victim_before);

        let receipt = prepared.finish().await.unwrap();
        assert_eq!(receipt.oid, expected);
        assert_eq!(repo.current_branch(), "pr/symref-race");
        assert_eq!(repo.git(&["rev-parse", "refs/heads/victim"]), victim_before);
    });
}

#[test]
fn moved_remote_head_is_rejected_without_creating_the_local_branch() {
    run_async(async {
        let source = test_repo();
        source.commit_file("README.md", "old", "old");
        let expected = source.head_commit();
        let bare = source.setup_remote();
        source.create_branch("feature");
        source.commit_file("feature.txt", "new", "moved");
        source.git(&["push", "origin", "feature"]);

        let repo = test_repo();
        repo.commit_file("README.md", "local", "local");
        let server = DumbHttpsGitServer::start(bare.path(), None);
        repo.git(&["config", "http.sslVerify", "false"]);
        repo.git(&["remote", "add", "fork", &server.url()]);

        let services = RepoServices::new(repo.path_str()).unwrap();
        let target = target(&repo, "fork", "feature", &expected, "pr/10").await;
        let plan = services.pull_checkout().inspect(&target).await.unwrap();
        let mut prepared = services
            .pull_checkout()
            .prepare_execution(&target, &plan, OPERATION_ID)
            .await
            .unwrap();
        let error = prepared.fetch().await.unwrap_err();
        assert_eq!(error, PullCheckoutError::HeadMoved);
        assert!(!repo.list_branches().contains(&"pr/10".to_string()));
        assert!(
            repo.git(&["for-each-ref", "--format=%(refname)", "refs/gitru/"])
                .is_empty()
        );
    });
}

#[test]
fn worktree_dirtied_during_a_delayed_fetch_is_not_switched() {
    run_async(async {
        let source = test_repo();
        source.commit_file("README.md", "base", "base");
        let bare = source.setup_remote();
        source.create_branch("feature");
        source.commit_file("feature.txt", "fork", "fork head");
        let expected = source.head_commit();
        source.git(&["push", "origin", "feature"]);

        let repo = test_repo();
        repo.commit_file("README.md", "local", "local base");
        let helper = tempfile::tempdir().unwrap();
        let started = helper.path().join("started");
        let release = helper.path().join("release");
        let server =
            DumbHttpsGitServer::start(bare.path(), Some((started.clone(), release.clone())));
        repo.git(&["config", "http.sslVerify", "false"]);
        repo.git(&["remote", "add", "fork", &server.url()]);

        let services = RepoServices::new(repo.path_str()).unwrap();
        let target = target(&repo, "fork", "feature", &expected, "pr/delayed").await;
        let plan = services.pull_checkout().inspect(&target).await.unwrap();
        let mut prepared = services
            .pull_checkout()
            .prepare_execution(&target, &plan, OPERATION_ID)
            .await
            .unwrap();
        let repo_path = repo.path().to_path_buf();
        let started_for_thread = started.clone();
        let release_for_thread = release.clone();
        let editor = std::thread::spawn(move || {
            let deadline = std::time::Instant::now() + Duration::from_secs(5);
            while !started_for_thread.exists() && std::time::Instant::now() < deadline {
                std::thread::sleep(Duration::from_millis(10));
            }
            let saw_fetch = started_for_thread.exists();
            std::fs::write(repo_path.join("dirty-during-fetch.txt"), "unsaved").unwrap();
            std::fs::write(release_for_thread, "continue").unwrap();
            assert!(saw_fetch, "fetch helper did not start before the deadline");
        });

        prepared.fetch().await.unwrap();
        editor.join().unwrap();
        assert_eq!(
            prepared.finish().await.unwrap_err(),
            PullCheckoutError::StalePlan
        );
        assert_eq!(repo.current_branch(), "main");
        assert!(!repo.list_branches().contains(&"pr/delayed".to_string()));
        assert!(
            repo.git(&["for-each-ref", "--format=%(refname)", "refs/gitru/"])
                .is_empty()
        );
    });
}
