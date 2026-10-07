# RURU-106 — Consistent backup and explicit recovery

Status: current-schema recovery and desktop workflow locally qualified; updated
draft PR #145 is ready for review, with new-head remote CI pending, 8 October 2026.
Started on RURU-105 `6becdd5`, then rebased onto its signed `d776d663` head,
including the Windows LF migration-fixture fix. This document is the continuation point.

## Final local qualification — 8 October 2026

Signed integration source `7bd1e37db444539c81e3d4f1805a128f8c9ab00f` is based on
RURU-119 `08a2db3c4d66269ce72bc5a780e9deda78a57f43`. Full `make verify`
passes 727 frontend tests with one platform skip, all lint/type/production-build
checks, Rust formatting, warning-denied workspace Clippy and 1,115 successful
Rust test executions (six standalone helpers ignored by the ordinary runner;
passing parent tests invoke their subprocess paths). The full run used signed
`a18eba4472802863ca3d894432ea0841a2f3eb89` before the final E2E-only cleanup.

The actual release-mode macOS packaged E2E passes both specs and all four
scenarios, including real native backup, paused preview, cancellation without
replacement, explicit restore, increasing revision/authorization view and
resumed isolated CLI policy. Artifacts:
`artifacts/e2e/2026-10-07T21-21-46-449Z-91136`; binary SHA-256
`86bf8c6f584e82f9f6eddd36207de297ae41ecd58b9a66b7cd89be0e167b6dbd`.
A test cleanup fix returns the app to the embedded Git route before the existing
Git smoke spec. Native OS picker interaction is substituted by the fixed,
feature-only native picker helper; the UI, commands, SQLite and recovery protocol
are real. No personal files, account credentials or production database is used.

The separate release-mode retained native harness passes all five app processes,
including crash-before/after-commit and fresh cold-restart evidence. Artifacts:
`artifacts/e2e-harness/2026-10-07T21-27-07-174Z-7496`; binary SHA-256
`46308ae0d0e9f1c232569378c3865f9f391f8cf8d8c8e48daed319db41d15cb5`.
That harness qualifies normal native sync/revision lifetimes, not a live provider
or production vault. Its tested source was signed `3d0597e`; rebasing onto the
RURU-119 Windows fixture repair changed only that inherited Git test and its
work note, verified by tree diff. All production source and recovery tests are
unchanged. The repaired Git integration suite separately passes 13 cases locally.

New-head remote macOS/Linux/Windows CI is a separate gate. Windows power-loss
durability, native OS picker interaction and live provider/PAT/keyring behavior
remain unqualified. No PR has been merged.

## Desktop integration checkpoint — 8 October 2026

The workspace Backups dialog is reachable without a healthy account query.
Native file pickers supply paths; five generated recovery commands expose only
review metadata and opaque caller-bound confirmations. A native gate serializes
backup, preview, cancel and confirm. Preview ownership includes tab incarnation
and expires after ten minutes; abandoned views are retired by a native watchdog.
The blocking mutation rechecks expiry and caller lifetime immediately before
its first write. Native-owned completion resumes storage even if IPC disappears.

Recovery drains the runtime and actual SQLite writer before obtaining the
replacement lease. A failed startup can also enter recovery. Cancel and successful
restore create a fresh runtime with the previous configured provider/vault/CLI
policy. The SDK fences cached reads and restarts revision catch-up generations;
a retired success, rejection or async batch cannot consume the replacement wake.
The trusted main host re-authorizes its visible child using fresh native demand
proof after recovery; suspended and background tabs remain inactive.

Initial combined frontend verification passes 722 tests with one platform skip;
subsequent SDK review passes all 172 SDK cases, and host recovery coverage passes
19 cases. Desktop typechecking and scoped lint pass. `make typegen` produces 135
wrappers and 399 schema exports, including a feature-only ordinary-E2E command
that substitutes a fixed native picker result without accepting renderer paths.
That helper requires the E2E identifier and main caller, and is absent from
production and the retained harness. The packaged scenario exercises the real
Backups UI, preview cancellation, replacement and resumed isolated runtime;
its execution and whole-workspace verification are still pending here. Native
OS picker interaction itself is a separate manual UI check.

## Continuation contract — 8 October 2026

Continue the existing [draft PR #145](https://github.com/ruru-m07/gitru/pull/145),
not a second implementation PR. The audit found its managed worktree clean at
signed `ea997d4aa672a014031d54ffb72904d9f92bfa33`, matching the published head
against `ruru/ruru-105-migration-recovery`. All eleven reported checks and status
contexts are successful at that exact head: frontend, formatting/Clippy, Rust and
packaged E2E on Linux/macOS/Windows, Cloudflare, CodeRabbit and Vercel. No
exact-head CodeQL check is reported. That evidence covers the existing v1/v2
core, not the continuation below. Live Linear has RURU-106 In Progress,
RURU-105 In Review and RURU-115 Backlog; RURU-106 blocks delivery in RURU-115.

The sections following this contract describe the existing v1/v2 implementation
and its historical evidence. The work specified here is not implemented by this
documentation checkpoint. First forward-port the existing branch onto the
coordinator's frozen RURU-119 source, which includes RURU-114 migration 0013 and
RURU-119 migration 0014. Then add ordered migration **0015** for recovery metadata.
Record the exact integrated source commit and tested migration ceiling when
that base is established; never modify checksummed migrations 0001–0014.

### Current schema and durable intent

Retain exact ledger/checksum and structural-schema verification before admitting
a selected database. Support recognized historical versions through the reviewed
current schema by migrating a private staged copy; reject future schemas,
unknown schema objects, dirty ledgers and corrupt records without changing the
selected input or current target. Merely increasing `RESTORE_SCHEMA_POLICY` is
insufficient. Validate authored rows with bounded streaming and preserve their
existing bytes and generations.

The explicit preservation policy includes:

- Accounts, provider instances, account-instance bindings, canonical resource
  identities and aliases retain their actor/account partitions.
- Draft bodies and generations, transport mappings and local repository links,
  cache pins, and local inbox disposition/bookmark/snooze intent survive.
  Restored local links do not establish fresh native registration, remote-digest
  or caller authority; their normal native proofs still apply.
- Commands retain their exact canonical envelope, payload and guard bytes,
  submission hash, admission receipt/revision, original authorization epoch,
  dependency ordering, target protections, delivery attempts and evidence.
  Do not reserialize intent, rewrite its epoch to match reauthentication, delete
  terminal evidence, or bypass the immutable-row triggers in migration 0013.

Credential references and cleanup records are removed from both exports and
installed replacements, with a second vacuum proving physical redaction. No
credential payload, vault lookup, imported cleanup dispatch or Gitru cloud
session participates in recovery. Accounts require reauthentication; account
epochs, authorization view and durable revision advance beyond both readable
snapshots with overflow checked. Rebuild provider projections, search indexes,
scope membership, checkpoints, validators, active demand and retention cursors
according to the full current schema, including commit/file generations.

Before clearing provider projections, preserve a scrubbed, verified immutable
snapshot of the incoming data as recovery evidence in the retained bundle.
This snapshot keeps command-protected comparison bases and selected-file
artifacts recoverable without presenting them as fresh observations or granting
access. Its checksum and publication participate in the durable recovery
manifest before any original mutation. The existing original-target bundle
still preserves newer drafts and receipts separately. Neither archive is an
active credential source or an automatic source for dispatch authorization;
normal recovery never silently merges either archive into the active database.

### Restored-command quarantine

Migration 0015 introduces a durable recovery generation and account-scoped
command quarantine tied to immutable command identity and submission hash.
Use separate recovery metadata; the existing command state enum and immutable
intent contract remain intact. Each restore advances recovery generation beyond
both readable snapshots. Every restored command that could be dispatched or
reconciled into dispatch is quarantined, including a command recorded as queued
with zero delivery attempts: the backup cannot prove it was never sent later.
Existing terminal receipts and all independent attempt/evidence rows survive.

Reauthentication, reopening, scheduler retries, dependency completion and an
old snapshot's queued state cannot release quarantine. RURU-115 must check
quarantine in the same transaction that claims a dispatch attempt. Release or
supersession requires operation-specific independent evidence or explicit
manual resolution; a generic retry action cannot resend a possibly successful
create. Structurally valid but unsupported operation/payload versions remain
opaque, preserved and non-dispatchable. Malformed envelope/hash/dependency
records fail verification rather than being normalized into guessed intent.
Safety does not depend on newer receipt archives being available.

The handoff to RURU-115 includes a synthetic backup-queued-command → remote
success → restore-old-backup test proving no second dispatch, including after
reauthentication. Restored quarantine must also survive another backup/restore
and a cold reopen. Any quarantine-resolution API belongs to the delivery and
operation policy work unless explicitly qualified here.

### Owned shutdown and restart

Replace immutable desktop `OnceCell` access with an owned lifecycle that can
represent starting, running, quiescing, recovery and failure, including failure
before a Store is available. Every normal IPC operation acquires a lifecycle
lease for the current runtime generation. Entering recovery rejects new work,
stops scheduler admission, wakes and joins the background worker, retires window
activity observers, and drains existing operation leases and owned credential
cutovers. Credential tasks that may already have mutated the vault must finish
their existing durable protocol; requester cancellation is not permission to
abort them midway.

Wait for reader shutdown and actual SQLx writer close before releasing or
handing over the OS writer lease. `Store::close()` awaiting only readers and
asynchronous writer Drop are insufficient. Stale runtime/Store owners cannot
write through a retired generation or reopen the writer while recovery owns
it. Respect the existing lock order: dispatch paths can acquire lifecycle after
dispatch, so shutdown must not hold lifecycle while waiting for dispatch.

The native recovery session owns the same writer lease throughout preparation,
preview and confirmation. Cancellation before confirmation and safe preparation
failure reopen the unchanged target through a new runtime generation. After
mutation starts, a native owned task completes or leaves a durable interrupted
marker even if its renderer disappears. An interrupted marker continues to
block ordinary bootstrap; recovery UI remains reachable. Successful recovery
restarts storage, subscriptions and demand ownership with fresh generations and
invalidates old frontend snapshots/cursors.

### Native picker and session UI

Expose native file selection for export and restore. Keep selected paths,
staging objects and recovery ownership in Rust; generated IPC carries bounded
preview DTOs and opaque session IDs, not arbitrary renderer-provided filesystem
paths. Bind each session to the current native caller lifetime and revalidate
that proof immediately before starting a confirmed mutation. Support backup,
restore preparation, explicit replacement confirmation, pre-confirmation cancel
and interrupted-recovery inspection/keep-original choice.

The preview shows incoming/current counts, schema/revision/checksum,
reauthentication, command quarantine, preserved newer data and the exact
replacement consequence. Do not automatically reset corrupt storage, merge
drafts, or delete recovery bundles. The flow must be usable from the visible
desktop tabs and when initial storage startup fails; do not require users to
find an inaccessible main window. Use the app's trusted native mediation for
the existing child/main authorization boundary. Generate all wire changes with
`make typegen` and implement the UI with the repository's shared components.

### Validation and ownership

Extend existing synthetic recovery tests rather than use the person's live app
database, provider account or credentials. Required checks include active-WAL
backup and physical reference redaction; frozen historical/current schema
fixtures; byte-for-byte command, dependency and evidence preservation; authored
state preservation; quarantine across restore/reopen/reauthentication; invalid
or future input rejection; protected recovery evidence; stale caller/session
and queued-writer races; drained credential tasks; canceled IPC during owned
confirmation; corrupt targets and retained newer originals; and real process
termination at preserve, marker, install and rollback boundaries. Prove an
actual writer close precedes lease reuse, not just an eventual successful open.

Use focused native tests and strict Clippy first, then generated IPC, complete
`make verify`, packaged UI checks, and exact-head remote platform CI. Report
those evidence classes separately. Existing Windows file flushing and passing
process-crash tests do not prove parent-directory/rename durability under power
loss; qualify that boundary honestly and retain the native Windows durability
work as an explicit release gate until implemented and verified.

The coordinator owns the forward-port/base decision and final integration. Core
recovery, migration 0015 and native fixture work can proceed after that base is
frozen; runtime/storage shutdown, setup/window observer ownership, and bridge/UI
work need explicit file ownership because they overlap active integration.
Update this contract with actual implementation and test evidence as each
checkpoint lands. Do not merge the PR without user authorization.

## Current recovery-core checkpoint — 8 October 2026

The existing branch was forward-ported with signed commits onto frozen RURU-119
`ff1fd0ec49a9265e5080ec16208d5923f4d07c18`; the signed continuation contract is
`9123a95`. The restore ceiling is now **schema 15**. Migrations 0001–0014 remain
byte-identical, with frozen fixtures and independent SHA-384 checksum literals.
Selected files at every recognized version 1–14 migrate only in private staging;
current version 15 also validates against the exact ledger and structural SQL.
Unknown objects, corrupt records and unsupported future schemas are refused.

The native verifier streams authored rows, bounds account and schema-object
materialization, and checks the immutable envelope framing, submission hash,
decomposed identity/payload/guards, dependency ordering, protections and attempt
history. Unknown operation codecs preserve their exact bytes. Recovery retains
commands, receipts, attempts, evidence, drafts and local intent; it does not
rewrite old command epochs or reinterpret opaque payloads.

Migration 0015 adds monotonic recovery generations and immutable account-scoped
quarantine. Every restored potentially dispatchable command, including queued
commands with no recorded attempts, gets quarantine for the new generation.
Repeated restore preserves previous quarantine records. The native
`command_quarantined_in` helper is intended for RURU-115's writer transaction
that claims an attempt. A SQLite `BEFORE INSERT` guard independently refuses new
`delivery_attempts` for quarantined commands, so omitting that helper cannot
claim a restored create. Existing attempt rows remain intact. There is no
generic quarantine-release API.

Before provider projections are cleared, recovery vacuums a credential-scrubbed
incoming snapshot into the retained evidence bundle. Its checksum participates
in manifest format 2 and is rechecked before confirmation. This preserves
command-protected bases and file artifacts without publishing stale provider
authority. The older target is retained separately, including newer receipts.
Credential references and cleanup records are physically removed from the
installed candidate and incoming evidence; no vault operation occurs. Provider
collections, search, validators, demand, file generations and retention cursors
restart cleanly. Revision, authorization view, matching account epochs and
recovery generation advance beyond both readable snapshots with checked
overflow. Quarantine insertion and cache reset share one transaction.

The native preview adds command counts, quarantined-command count, recovery
generation and incoming-evidence retention. Filesystem paths stay inside Rust.
The adjacent lifecycle lane changes `Store::close()` to reject stale writer
handles, await readers and actual SQLx writer shutdown, then release the OS lease
even if clones remain. Final Drop retains the lease through acknowledged writer
cleanup; exceptional cleanup retains it until process exit. Runtime replacement
requires successful shutdown. Desktop picker/session integration has separate
ownership and validation; this core checkpoint does not claim UI completion.

Current local evidence includes 14 original recovery integration cases, all
historical migration versions, exact v14 command-history preservation, real
SQLite interrupt and disk-full migration rollback/retry, zero-attempt quarantine
after reauthentication, repeated restore/cold reopen, protected incoming
artifacts, atomic rollback of fencing, native long remote paths, and malformed
input/evidence refusal. Process-crash cases use synthetic child processes and
retain the original transaction boundaries. The final core rerun passed **11
native recovery cases** (one subprocess entry point is intentionally ignored
outside its parent crash cases), **14 recovery integration cases**, and **4
migration tests**. Strict `cargo clippy -p collaboration --all-targets -- -D
warnings` passed. The adjacent lifecycle lane's complete collaboration crate
rerun passed **692 tests across 25 suites**, with four intentionally ignored
tests. Complete workspace/UI checks and new remote CI remain
separate. No existing remote CI result covers this new source yet. Windows
directory/rename durability under power loss remains the release boundary
described in the continuation contract.

## Historical v1/v2 implementation and evidence

The following sections record the original native-core slice before this
continuation. References to missing runtime ownership or a schema-2 ceiling are
historical and are superseded by the current checkpoint above.

### Boundary and user choice

Rust owns the recovery API. A running `Store` can create a backup. Replacement
requires every runtime/Store owner to shut down and releases the same writer
lease used by normal bootstrap. The native restore session holds that lease
through preview and confirmation; another app instance cannot open storage.
No network or vault operation participates in recovery.

The caller presents a verified preview containing incoming/current draft and
account counts, source revision/schema, backup checksum, and the consequences:
incoming draft bodies replace the active database's drafts; newer current draft
text remains recoverable in the original bundle rather than being automatically
merged. A physically corrupt original has unknown counts and is retained as raw
evidence; readability is not promised. Replacement requires reconnecting
accounts; provider observations and validators are discarded. Confirmation is
bound to the exact preview nonce, staged candidate checksum and current main/WAL
fingerprint. Empty/absent WALs are equivalent and SHM is a derived coordination
index, because merely inspecting SQLite can change those without durable writes.
Cancellation
before confirmation leaves the original untouched. There is no reset API.

The existing desktop runtime is an immutable OnceCell without an owned shutdown
operation. `Store::close()` currently awaits readers only; final writer Drop
closes SQLx asynchronously and can checkpoint after lease release. Integration
must await actual writer quiescence, not infer it from lease release. The preview
CAS safely refuses any such transient durable main/WAL change.
This slice exposes a native two-step API, not a live restore
command that can race that runtime. A desktop picker/dialog integration must
first implement shutdown/startup ownership, use native file selection, and pass
only session IDs across generated IPC. No arbitrary JavaScript path command is
introduced here. This is an explicit product integration boundary.
RURU-106 remains In Progress until that desktop lifecycle/UI acceptance gate is
implemented. Synchronous confirmation must run in a native owned blocking task;
dropping an IPC requester must not cancel the task after mutation begins.

## Backup publication

Use SQLite `VACUUM INTO` through the Store writer connection, under its existing
mutex. This is a transactional snapshot including committed WAL frames, without
copying the live main database. Local reads continue; writer operations queue
for this first bounded implementation. Incremental backup/progress is a later
resource-budget improvement.

Create a private staging directory next to the destination. Check SQLite
integrity, foreign keys, exact supported schema, and every SQLx migration checksum.
Remove all account credential mappings and staged/retired cleanup references in
the staged database. A second `VACUUM INTO` produces the export with deleted
reference bytes removed from free pages. Verify again, sync the file, then
publish with a no-overwrite filesystem operation. An interrupted/failed backup
cannot replace an existing backup or damage the source. Backup data, including
authored drafts and private cached provider content, remains sensitive local
data; it is not encrypted by this issue. Credential payloads never enter SQLite
and no vault reference is exported.

SQLite documents transactional consistency, interruption limitations and fsync
behavior for [VACUUM INTO](https://sqlite.org/lang_vacuum.html). This choice avoids
adding unsafe native-handle ownership around the
[SQLite backup API](https://sqlite.org/backup.html).

## Staging and restore fencing

Copy the selected backup into private same-filesystem staging, verify it, and
run only recognized forward migrations there. Never migrate or repair the
selected backup in place. Refuse dirty, mismatched, missing and future migration
ledgers, unsupported schema objects and malformed account/draft records.

Before creating a restore candidate, remove credential mappings and cleanup
records regardless of source provenance. Require reauthentication for every
restored account, advance matching actor/account authorization epochs beyond
both snapshots, discard provider rows/FTS,
membership, checkpoints, validators and change hints, and advance the revision
and authorization view beyond both snapshots when readable. Preserve
account/actor identity and all authored draft bodies, subjects and generations.
Copied credential references never authorize access, and imported cleanup
records never reach the vault janitor. Local credentials are untouched; previous
installation references survive only in the private original recovery bundle.

## Replacement and interrupted recovery

Before moving any original file, preserve the main database **and** any WAL/SHM
sidecars in a private recovery bundle and verify their checksums. Sync the bundle,
verified candidate and complete manifest. Publish the durable pending marker
only after the entire bundle is verified, so a killed partial copy cannot strand
an invalid marker. Remove old sidecars only after preservation, and atomically install the verified
candidate by rename on the same filesystem. Keep the marker until all file and
directory syncs and archive publication succeed.

`Store::open` refuses a pending recovery marker, including the window in which
the main file is absent, rather than creating an empty database. Interrupted
recovery requires its own inspected, checksum-bound explicit choice to restore
the preserved original files. No automatic rollback loses newly restored work.
The original bundle retains trusted current-target credential/cleanup metadata
as local evidence, but the installed replacement contains none of it. There are
no automatic orphan-reference cleanup guesses. The original bundle is retained
after successful replacement; there is no
automatic deletion policy. Corrupt original files can be preserved as raw
main/WAL/SHM evidence without pretending they are a verified logical backup.

## Schema/outbox evolution gate

The restore policy supports the known v1/v2 schema only. Exact known schema
objects and checksummed migrations form a fail-closed gate. A future migration,
extra table (including an outbox), trigger or unknown command representation
requires an explicit policy update and test. There is no current outbox, so this
issue does not fabricate queued commands. Before admitting a future outbox,
restore must advance a durable recovery generation, quarantine every imported
potentially dispatchable intent, preserve independent delivery evidence and
prove old-backup → remote-success → restore cannot resend. RURU-115 remains
blocked on that policy extension.
The concurrently implemented RURU-76 migration 0003 is deliberately not yet
admitted: its instance/alias associations require their own reviewed restore
policy. Simply raising the supported schema number is insufficient.

## Validation plan

Use task-owned temporary files only; never open, restore or reset the person's
live app database or vault. Cover committed WAL snapshots while Store remains
open, draft/account preservation, source/destination immutability on failure,
credential-reference physical redaction, malicious imported cleanup references,
reauthentication and cache fencing, corruption/truncation, unsupported/dirty/
changed/newer schemas, stale previews, lease contention, interruption at durable
replacement boundaries and recoverable original files. Exercise real SQLite
write interruption/disk limits where meaningful rather than only mocking an
error return. Run focused/full collaboration suites, Clippy and formatting.
Record local results separately from remote CI and product flow integration.

## Implemented API and resource boundaries

- `Store::backup_to(path)` returns checksum/revision/schema/counts after verified
  no-overwrite publication. It never invokes a credential vault.
- `RecoverySession::prepare(target, selected_backup)` asynchronously stages and
  verifies an immutable candidate under the target writer lease. `preview()`
  exposes review data. Consuming `confirm(id, ReplaceCurrentData)` is synchronous
  and returns the original bundle location and installed revision.
- `InterruptedRecovery::inspect(target)` verifies preserved originals and binds
  a new preview to their checksums and the present target. Consuming
  `confirm(id, KeepOriginalData)` restores originals and preserves the current
  candidate/evidence in the bundle too. There is no automatic replay.

Inputs are capped at 512 MiB per file; hashing/copying and draft verification use
bounded buffers/row streaming. At most 100 account metadata records are loaded.
Enough disk is required for private snapshots, scrubbed export, staged
replacement and retained originals. File-allocation failures before marker
publication leave active data untouched; failures afterwards retain verified
originals for explicit recovery. Writer serialization and full integrity/schema
scans are intentionally conservative initial limits, not a measured
large-database latency claim.

Private files/directories use 0600/0700 on Unix. Files are flushed on every
platform; parent-directory fsync is available on Unix. Windows process-crash
behavior can be CI-tested, but directory/rename durability under **power loss**
requires a native Windows durability primitive and separate platform release
qualification. No Windows power-loss guarantee is claimed by this slice.

## Validation evidence

Four focused core cases pass, exercising nine hard child
process terminations (two backup, four replacement and three interrupted
rollback boundaries), real SQLite interruption during backup, and backup after
a real SQLITE_FULL source write rolled back. The ignored child test is invoked
by the passing parents. Fourteen independently authored integration cases pass:
live WAL snapshots and physical reference redaction, lease/cancellation/explicit
choice, monotonic fencing/newer-draft recovery, malicious imported cleanup with
a fake vault and real janitor, WAL-only stale CAS, frozen selected input, invalid
schema/typed ledger/drafts, corruption, newer target refusal, missing-main
bootstrap blocking, historical v1 upgrade, and interrupted nonce/CAS/tampering.
The final full collaboration suite passes **88 tests** (37 unit, 12 migration,
14 recovery integration, 9 scheduler/runtime, 16 storage). Three ignored
subprocess entrypoints are explicitly invoked by passing parent cases. Clippy
for all collaboration targets with warnings denied, workspace formatting and
diff checks pass. After the LF-base rebase, the full 88-test suite passes again;
Git attributes confirm production migration SQL and frozen fixture SQL use LF.
No remote CI, live provider/vault, desktop recovery UI
or Windows power-loss verification is claimed.

Executable-fixture audit follow-up (3 October 2026): PR #147 head `4a1e466`'s
[Linux workspace job](https://github.com/ruru-m07/gitru/actions/runs/37106707761/job/111156589988)
records OS code 26 (`ExecutableFileBusy` / `Text file busy`) at the RURU-95 crash
harness spawn. No RURU-106 failure is observed in that job. The sibling audit
finds the same parent-copy-then-execute pattern in this module's test-only crash
harness; RURU-105 executes its existing test binary without writing it, and
recovery integration copies are SQLite data rather than executables. The Unix
inherited writable-descriptor explanation is supported by the source and
[Rust issue 114554](https://github.com/rust-lang/rust/issues/114554), rather than
a CI descriptor trace. This test-only snapshot now uses fixed `/bin/cp` in a
child with cleared environment, fixture-directory working directory and null
streams, and waits for successful exit before executing the copy. Non-Unix
retains its existing copy. Nine real backup/restore/rollback kills, writer-lease
checks, checkpoint timeouts, parallelism and content assertions are preserved.
Production recovery and SQLite copying are unchanged. Local macOS verification
passes the four focused core tests (nine real process kills), all 14 recovery
integration cases, and the full current collaboration suite: 89 passed and three
intentionally ignored subprocess entry points invoked by their passing parents.
All-target collaboration Clippy with `-D warnings`, workspace formatting and
diff whitespace checks pass. New-head Linux/platform CI remains pending after
the RURU-95 repair is propagated into this branch.
