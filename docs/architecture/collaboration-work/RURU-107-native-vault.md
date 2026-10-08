# RURU-107 — production credential-vault qualification

Pre-code contract, 8 October 2026. This continuation starts from PR192's signed
`52263c4517a619b48055808a5dbbf6f02416717b`, whose fourteen remote checks pass.
Its packaged tests deliberately use synthetic vaults. This lane qualifies the
actual production credential adapter with synthetic values in ephemeral CI OS
stores. It does not require a provider account and does not complete RURU-75.

## Source and ownership

Extract the unchanged `NativeVault` implementation from desktop
`collaboration_setup.rs` into a private native module; keep the production
service name and runtime construction unchanged. The opt-in native tests must
exercise this same implementation and the pinned keyring3.6.3 feature selection.
Do not substitute a mock, introduce a second credential backend or upgrade the
dependency. Normal packaged automation must continue using its existing fake
vault and never open a person's native credential store.

## Scope and isolation

The actual platform tests run only on ephemeral GitHub-hosted CI runners, with an
explicit opt-in gate that fails when missing. No actual native-vault test will
run against this user's desktop. Every invocation allocates a random test service
and random credential references and stores newly generated synthetic values.
Read and delete only those exact owned references; never enumerate a keychain,
read Gitru's production service, import CLI authentication or contact a provider.
Child processes inherit a bounded manifest of owned fixture identifiers. Do not
log secrets, credential payloads or unfiltered platform errors. Cleanup removes
only the fixture's entries, including after a failed assertion where possible.

Use the production macOS and Windows backend in each runner's ephemeral session.
Linux needs a fresh D-Bus session and an isolated Secret Service data directory,
with its test daemon and lifecycle owned by the job. Keep the user's HOME and
system login-keychain/default settings unchanged. Do not unlock or lock a user's
existing vault, change a default keychain or weaken an authentication prompt.
Runner service-start failure is a failed or explicitly unqualified gate, never a
successful mock fallback.

## Evidence

Exercise absent entry, store/load, replacement, fresh-process reopen, distinct
services/references, delete and idempotent missing deletion through `NativeVault`.
Verify surviving owned entries when another is deleted. Assert redacted error
and secret formatting without printing values. Bound subprocess and whole-job
time; propagate child failures and retain sanitized receipts with exact source,
OS and backend information. A successful in-process round trip is insufficient
for the cold-reopen claim.

The existing synthetic runtime/harness controls remain the evidence for offline
cache survival, vault-unavailable backoff and provider suppression. Add safe real
platform failure controls only where the isolated fixture can create them; for
example a fresh child with an intentionally unavailable private D-Bus endpoint.
Do not label missing-entry, invalid input or injected errors as a real locked
keychain test. Interactive lock/prompt policy and live PAT/CLI/revocation flows
remain separately recorded qualification gaps.

Before publication, run compile/lint and synthetic adapter controls locally;
publish the opt-in OS matrix and record each actual remote result against its
head. No local native-vault or cross-platform success is claimed by this plan.
No schema or IPC changes are expected. Preserve PR192 and its existing evidence.

## References checked

- [Pinned keyring feature manifest](https://docs.rs/crate/keyring/3.6.3/source/Cargo.toml)
  and the checked-out keyring3.6.3 source define the production backend selection.
- [GNOME Keyring](https://github.com/GNOME/gnome-keyring/blob/main/README)
  documents the session daemon and D-Bus discovery used by the isolated Linux job.

## Implementation checkpoint

Signed source checkpoint `6f03583cea638637b9855382c596cfd610891cd5`
extracts the existing production adapter into the private desktop
`native_vault` module. Its keyring entry construction, serialized store/load/
delete operations, missing-entry behavior and sanitized unavailable mapping are
unchanged. Runtime construction still derives the production service as
`<application identifier>.collaboration`. The `e2e` and retained collaboration
harness configurations continue to exclude this module and use the existing
in-memory test vault.

The new opt-in workflow runs one ignored supervisor test on Linux, macOS and
Windows. The supervisor requires the explicit qualification marker plus the
GitHub-hosted runner and matching runner-OS evidence. Each run creates a random
UUID-backed service/reference namespace, clears the child environment, forwards
only a finite platform allowlist and passes no provider credential variables.
Seed, cold-reopen and cleanup phases run in separate bounded processes. They
cover absence, store/load, replacement, service/reference isolation, exact
delete and idempotent missing delete without printing the synthetic values.

Linux additionally owns a fresh `dbus-run-session`, private `XDG_DATA_HOME`,
private `XDG_RUNTIME_DIR`, explicit GNOME Keyring daemon PID and exact cleanup.
It checks service readiness without activating an unrelated daemon and verifies
that an intentionally missing private D-Bus socket maps to unavailable instead
of falling back. The workflow keeps `HOME` unchanged. macOS and Windows use the
pinned production-native backend in the ephemeral hosted-runner session. Each
child phase has a 45-second bound and the matrix job has a 60-minute outer
bound. The workflow also carries the exact signed Windows platform-test timeout
allowance from `eb12ab23536cbd835db2fabce910b99cafeaa719`; it removes no test
commands.

## Local evidence and remaining qualification

The source checkpoint was validated locally without executing a native
credential-store operation:

- `cargo test -p gitru --lib`: 36 passed, with both actual native-vault tests
  visibly ignored;
- `cargo test -p gitru --features collaboration-harness --lib`: 49 passed,
  proving the packaged retained harness still compiles and uses its synthetic
  vault;
- both ordinary and `collaboration-harness` `gitru` all-target Clippy passed
  with warnings denied;
- Rust format/diff checks, workflow YAML parsing and the nested Linux Bash
  syntax check passed.

These are compile, isolation-gate and synthetic-vault results only. They do not
qualify this Mac's keychain or any remote platform. Publication still requires
the exact PR head's three new native-vault jobs to pass and be recorded by OS
and backend. Interactive locked-vault prompts, user-denied access, PAT/CLI
import, live provider authentication and revocation remain outside this slice.
