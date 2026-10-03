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
