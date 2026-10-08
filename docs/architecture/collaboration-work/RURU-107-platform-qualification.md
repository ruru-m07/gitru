# RURU-107 — isolated platform vault qualification

Design recorded before implementation, 8 October 2026. Parent: RURU-53.

## Scope and prerequisites

This bounded slice adds a retained synthetic vault-unavailable scenario to the
actual native collaboration runtime and packaged desktop harness. It builds on
PR #191 (integrated schema 22 and scheduler fairness) and the bounded fixture
startup repair from PR #157. The dependency branch combines those prerequisites;
the PR delta contains only this qualification work. No production credentials,
keyring entries, provider accounts or remote mutations are used.

## Contract

- A finite, launch-owned native controller selects `vault_unavailable`, persists
  that phase in the isolated harness marker and increments its generation. The
  synthetic vault refuses load/store/delete while unavailable. Existing exact
  reference and token allowlists remain mandatory. Failure counters contain no
  secret data and are session-local; the fault phase survives cold reopen.
- Qualification uses the production runtime's credential error boundary. A
  failed credential load must not dispatch a provider request, remove account
  authorization, destroy a saved Body or discard an authored draft. Local reads
  remain independent of credential availability.
- Explicitly restoring a normal fixture phase permits a fresh due refresh and
  proves recovery without reconnecting/replacing credentials. Tests do not infer
  real macOS/Windows/Linux vault availability from the synthetic implementation.
- The compiled renderer scenario retains exact before/failure/after evidence,
  checks cached content and draft identities, then restores normal operation.
  A strict bounded receipt is checked by the packaged runner on each OS.
- Native tests additionally reopen the real store while the fault is persisted,
  verify exclusive ownership, cached FTS/reads and drafts, then recover. Existing
  migration/rollback, offline cold-start and hard-crash cases remain required.

## Validation and remaining gates

Regenerate changed feature-only IPC with `make typegen`. Run focused native and
protocol tests, strict Clippy, frontend/type checks and the packaged harness when
the local desktop permits. Record remote Windows/Linux/macOS results for the
exact published head separately. Production OS-vault prompt/lock/replacement
verification still requires dedicated test accounts and remains an explicit
release gate; this slice cannot close RURU-107 on synthetic evidence alone.

Implementation and actual evidence will be appended below.
