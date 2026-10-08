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

## Implemented slice

The feature-only vault now shares the persisted native fixture phase and counts
synthetic unavailable refusals separately from successful loads/stores/deletes.
Every existing root/reference/token check remains in force. Two native controls
exercise all three refused operations, untouched synthetic credentials, provider
suppression, saved Body/draft/account identity, the real writer lease, cached FTS,
cold reopen with a retained fault, and recovery after the production 180-second
credential-error barrier. The controller advances its existing finite clock;
production retry timing is unchanged.

The compiled `vault-unavailable` renderer scenario is now in every packaged
collaboration matrix's main phase. It requires the actual typed credential error,
unchanged provider count during refusal, retained Body/draft hashes and account
identity, and a newer committed facet after recovery, even when Body is unchanged. Its strict evidence
schema rejects invented production-vault scope, missing refusal, continued HTTP,
lost drafts and failed recovery. Existing hard-crash/restart scenarios remain.

Local evidence so far: 23 native harness tests, 627 desktop tests (one pre-existing
skip), desktop/E2E TypeScript and desktop Biome pass. `make typegen` regenerated
the feature-only enum/counter contracts. Full collaboration tests, strict Clippy,
and the packaged macOS build/run are in progress; remote exact-head CI has not
run for this slice yet. A first cold-reopen test correctly found that a single
61-second clock advance did not clear the 180-second retry barrier; the fixture
now advances three existing 61-second steps and the native control passes.

This is isolated synthetic qualification. Real production vault prompts, lock,
replacement and failure behavior on each supported OS still require separately
recorded dedicated-account tests. No production keyring or personal credential
was inspected, no provider mutation was attempted, and no PR was merged.

## Packaged sequence correction

The first packaged macOS run passed six main-phase scenarios and failed the new
vault scenario during warm-up, before any vault refusal. Earlier scenarios had
already committed phase two; requesting phase one supplied an older provider
timestamp which the production store correctly refused. The obsolete response
remained due and exposed a repeated fixture refresh, not a vault failure.

The revised scenario warms phase two and recovers to phase two. Recovery must
prove a newer committed facet revision, cleared credential error and a new
provider call; it must not require the remote text to change when the credential
store becomes available. Cache/draft hashes stay equal throughout. A native cold
reopen control will repeat this same-content sequence as well as the original
phase-one to phase-two case. Production ordering/retry policies remain unchanged.

The correction at `70ad1a40` passes three fault/cold-reopen native controls,
41 protocol/executor controls and desktop/E2E type checking. Independent review
confirms a stale cached view cannot satisfy recovery: the renderer must match
exactly the freshly committed facet, with error cleared and resumed provider
calls. All 24 native harness controls and strict Clippy pass. The packaged macOS
qualification at the same source passes all five phases: main (seven scenarios),
before-commit hard crash, restart-before, after-commit hard crash and restart-after.
The actual refusal preserves facet `4442`, account/epoch and Body/draft hashes,
records one vault failure with provider calls fixed at nine, then recovers to
facet `4451` with the error cleared and provider calls resumed. This includes the
real native writer/revision/runtime and compiled renderer, using synthetic data.

Retained evidence: `artifacts/e2e-harness/2026-10-08T04-22-36-523Z-38227`,
binary SHA-256 `68e05e5c45ee1c6cb7c3c5a3f06be905853efa0e74eb1b0c242f840a2816adac`.
Logs: `/tmp/gitru-r107-packaged-corrected.log`,
`/tmp/gitru-r107-corrected-harness-native.log`,
`/tmp/gitru-r107-corrected-clippy.log`. Before the sequence correction, the full
native suite passed 1,021 tests/five helper ignores and 627 desktop tests/one
existing skip; after correction the focused 24 native and 41 protocol/executor
controls, both type checks and formatting pass. The production build excluded
all retained-harness markers. Fresh remote platform CI is still pending; its
results must be recorded against the published head separately.
