# RURU-95 — Recoverable credential cutover

Status: implemented and locally verified; review and remote CI remain pending.
This work follows the credential isolation,
authorization epoch and draft preservation contracts in the engine architecture.

SQLite owns the committed vault reference for each account. Replacements use a
fresh reference; token payloads never enter the database, IPC or this journal.
Before writing to the vault, persist a staged cleanup record. Keep the previous
committed reference usable until one SQLite transaction promotes the new
reference, advances the account epoch, clears its provider cache, and retires the
previous reference. After that transaction, cleanup can only remove staged or
retired references, never the committed reference.

A process crash before promotion leaves the previous authorization and reference
intact. Recovery abandons the uncommitted replacement and deletes its staged
reference. A crash after promotion keeps the new epoch/reference and retries
deletion of the retired reference. Cleanup failures retain durable work with
bounded backoff and bounded batches. Local disconnect retires the committed
reference in the same transaction that invalidates authorization. Drafts survive
every path. Legacy version-one vault references are migrated from account IDs.

The native runtime retains ownership of a started cutover even if its requesting
future is cancelled, preventing a delayed blocking vault write from creating a
secret after recovery has removed its journal entry. The provider is probed
before staging; a different verified actor gets a separate account partition.

Migration `0002_credential_cutover.sql` creates `account_credentials` and
`credential_cleanup`. Active/auth-required v1 accounts retain their account-ID
references; disconnected legacy references are journaled for deletion. New
references use UUIDs. Staging admission is capped at 128 pending records,
background cleanup takes eight records per pass, and retry delays grow from 30
seconds to a 15-minute ceiling. Explicit disconnect prioritizes its own retired
reference. Neither journal table stores secrets or enters IPC DTOs.

Local verification (3 October 2026):

- `cargo test -p collaboration --tests`: **58 passed**, one ignored subprocess
  entry point. The three process-crash tests actually launch and terminate **25
  child processes** at before/during/after staging, vault writing, atomic cutover,
  disconnect, secret deletion and journal completion. Restart checks the committed
  epoch/reference, provider token selection, orphan removal and preserved drafts.
- Failure tests cover partially successful vault writes, locked retirement and
  disconnect, SQLite cutover abort, cancellation during a held blocking vault
  write, per-account ownership, admission/batch/retry bounds and explicit
  disconnect priority. Existing late-response, rate-budget and CLI actor tests
  pass with committed reference lookup.
- Crate Clippy with `-D warnings`, workspace formatting and diff whitespace checks
  pass. No command signatures or generated wire DTOs changed.

Crash checkpoints and their environment variables compile only in the library's
unit-test build. Fixtures use an isolated durable fake vault and snapshots of the
test executable. In-process failure tests retain their storage owner; process
restart is tested by actual child termination, without retry sleeps or relaxed
writer-lease checks. The fixture asserts that a completed owned cutover released
its Store before tearing down the original owner.

Frozen-v1 fixture upgrade/ledger coverage belongs to the parallel RURU-105 suite.
Production native-vault/live-PAT and Linux/Windows execution remain separate
validation gates. No remote CI or platform success is implied by this local run.

CodeQL follow-up (3 October 2026): PR #142 head `20625e17` has one
`rust/cleartext-logging` alert ([19](https://github.com/ruru-m07/gitru/security/code-scanning/19))
at the crash child's synthetic `accounts.remove(0)` extraction. The completed
Rust analysis has no infrastructure error or warning. CodeQL 2.27.1's
[generated model](https://github.com/github/codeql/blob/6e9f9e38390175c41b99070a423c875f450759ca/rust/ql/lib/ext/generated/modelgenerator/rust.model.yml#L8766)
classifies `Vec::remove`'s receiver as a logging sink; this test has no account
logging and uses only its temporary database and fake vault. Checked iteration
with fixed failure messages now requires exactly one fixture account before
disconnect. The repair preserves every crash boundary without suppressing the
rule or changing production behavior. Scoped local verification passes all nine
credential crash/failure tests (one intentionally ignored subprocess entry point),
all-target collaboration Clippy with `-D warnings`, workspace formatting and diff
whitespace checks. Exact-head remote rescanning remains pending for this follow-up.

Linux snapshot follow-up (3 October 2026): PR #147 head `4a1e466`'s
[Linux workspace test job](https://github.com/ruru-m07/gitru/actions/runs/37106707761/job/111156589988)
fails while spawning the copied credential crash harness, before a recovery
assertion or checkpoint. The original error is OS code 26,
`ExecutableFileBusy` / `Text file busy`; library results are 44 passed, one failed
and one ignored subprocess entry point. The CLI and detail runtime cases pass.
The parent copies an executable immediately before spawning it while other tests
fork. This matches the inherited writable-descriptor mechanism described in
[Rust issue 114554](https://github.com/rust-lang/rust/issues/114554); that mechanism
is a source-supported diagnosis, not a captured CI file-descriptor trace.
Unix snapshots now use fixed `/bin/cp` in a child with cleared environment,
fixture-directory working directory and null standard streams. The parent waits
for successful child exit before executing the snapshot and never opens its
destination for writing. Non-Unix keeps the existing copy. All real hard-kill
checkpoints, timeouts, parallelism and recovery assertions remain intact; there
are no spawn retries. The parallel library suite passes all 34 tests, and the
focused credential suite passes all nine tests, each retaining the intentionally
ignored child entry point used by the passing parents. All-target collaboration
Clippy with `-D warnings`, workspace formatting and diff whitespace checks pass.
These are local macOS results; new-head Linux/platform CI remains pending.
