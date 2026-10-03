# RURU-106 — Consistent backup and explicit recovery

Status: native core implemented and locally validated, 3 October 2026.
Started on RURU-105 `6becdd5`, then rebased onto its signed `d776d663` head,
including the Windows LF migration-fixture fix. This document is the continuation point.

## Boundary and user choice

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
