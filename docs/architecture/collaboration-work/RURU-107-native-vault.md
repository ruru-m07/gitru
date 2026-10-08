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
