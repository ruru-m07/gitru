# R140 next bounded slice: opt-in current-schema keyed Store

Pre-code contract approved by the parent implementation task on 2026-10-08.
This authorizes only the bounded opt-in implementation below, not activation.

## Recommendation

Integrate the real Store with an explicit native connection owner, using the exact
qualified PR202 factory and the latest frozen R134 schema25 engine as parents.
Keep `Store::open`, desktop startup, platform-vault selection and default manifests
unchanged. This is an opt-in component qualification, not conversion or encrypted GA.
Do not fork/copy the Store implementation into another test crate.

Reuse one clean attached R140 worktree on a new `ruru/` branch, preserving the
published PR194/200 branches. Root should first construct an explicit dependency
composite of the final R134 source and PR202 (current head 8d8073f5). PR202's
collaboration source is only schema22; testing it alone cannot qualify schema25.
The isolated workspace can depend on that actual collaboration crate and enable
the native seam, while retaining the hash-pinned SQLx0.9/libsqlite3-sys0.37/SQLCipher
build patches. Ordinary production builds must continue using their current driver.

## Concrete source gaps

1. Store currently acquires its own `WriterLease`, constructs raw SQLx writer and
   pool options, and separately opens unkeyed maintenance connections. A prepared
   DatabaseKeySession already owns that same lease: reacquiring it deadlocks.
2. The factory initializes/authenticates actual handles, but its Arc state owns the
   entire session indefinitely. Closed pools/options and stale Store clones retain
   those Arcs. Whole-Store close must retire admission and release the session only
   after every C handle closes; it cannot merely await SQLx close acknowledgement.
   The recent Linux qualification race proved that worker-owned options can outlive
   that acknowledgement. Exceptional close must permanently retain the owner and
   refuse later opens, rather than report successful retirement.
3. There is no encrypted database identity row. Cipher authentication alone does
   not prove that the database matches the sidecar UUID/key generation.
4. `Store::backup_to` calls the existing plaintext export path. Keyed Store must
   explicitly refuse that entry point until an intentional key-aware export API
   exists. Existing RecoverySession constructors also use plaintext options; they
   must reject keyed targets before staging or inspecting through that path.

## Suggested interfaces and ownership

Keep the crypto implementation in the isolated native factory. Add a narrow
native-only connection-owner trait/seam to collaboration (no serde/IPC types):

- path and immutable database identity/profile;
- single-use Store ownership claim;
- open writer, read-only pool and maintenance handles;
- authenticate an existing read-only verifier/recovery handle;
- retire admission, wait for all actual native owners, and acknowledge lease release.

The trait is a trusted native/unsafe contract, not a provider extension or renderer
configuration. All returned SQLx objects must retain their native owner internally.
The Store entry point consumes the owner and uses the same migration, cleanup,
query, writer-gate and shutdown code as ordinary Store. A private ownership enum
keeps ordinary WriterLease behavior unchanged and delegates keyed retirement to
the owner. Open and close remain cancellation-owned native tasks.

Refactor factory ownership first: keep non-secret path/profile and admission state
separate from an optional shared session. Each reserved native handle retains its
own session Arc. Options/closed pools retain only the admission state. Retirement
closes the admission gate; a bounded causal wait observes the final native-owner
release before taking/dropping the session. A failed C close leaves a retained owner
and faulted state, and cannot produce a successful release. New pool replacements,
direct factory opens and duplicate Store claims must fail after retirement.

Do not route backup/current recovery through an unkeyed connection just because it
shares the same path. This slice can expose explicit Unsupported/NotReady errors for
those operations on keyed Stores while documenting their R140/R141 follow-up.

## Identity and migration proposal

Coordinate a new migration26 only after schema25 is frozen (full native qualification remains pending at this checkpoint). Add a normalized
single-row native storage identity table (format, database UUID, key generation,
fixed cipher-profile identifier; no key or credential reference). Plaintext stores
leave it empty. A keyed Store requires exactly the sidecar identity and profile.

Only the original explicit CreateNew reservation may establish the identity row.
Existing encrypted data with a missing/mismatched row is preserved and refused;
never infer identity from its filename or initialize it over existing authored data.
Write identity and initial application-schema completion in one owned transaction
where feasible. If the migrator cannot provide that atomic boundary, classify an
interrupted new file as recoverable-but-not-Ready and preserve it; do not invent an
automatic continuation that could relabel unrelated encrypted data.

Ready publication must happen after authenticated read-only identity/schema/integrity
verification and after all bootstrap writers close. The present borrowed-key
DatabaseKeyVerifier interface is not enough to own a cancelled SQLx worker. Adapt
verification to retain the same session owner throughout the worker, or introduce a
native proof produced only by the owned factory. Do not expose a boolean 'verified'
constructor. Reopen/revalidation must invalidate old Ready authority on failure.

Extend strict schema/recovery policy and freeze schema25 for this new table. The
current plaintext restore path should reject nonempty encrypted identity rather
than import a misleading binding. Key-aware restore and portable-envelope semantics
remain explicit follow-ups, not a schema-version ceiling bump alone.

## Meaningful qualification

- Actual schema25 history migrated to26 in encrypted files; exact migrations,
  SQLite source/WAL gate, FTS, integrity and foreign keys.
- Real Store accounts, local drafts, discovery/query/search/revisions, metadata
  catalogs, durable v1/v2 commands and proofs; cold reopen preserves exact bytes.
- Three read-only pool connections plus replacement, maintenance/WAL checkpoint,
  wrong key/plaintext/profile/identity mismatch refusals, no recreating missing DB.
- Real Store writer drain, blocked readers, startup/migration fault, cancellation
  before/after open and verification, stale Store clones, release acknowledgement
  before new process lease; exceptional close retains owner and faults admission.
- Backup/recovery entry points fail closed before creating plaintext staging.
- Synthetic DB/WAL/temp canaries and trace/error scans with keyed/unkeyed controls;
  bounded artifact evidence, not a claim of physical erasure.
- Default plaintext Store regression unchanged; exact native matrix per platform.

Linux/Windows create-only key-vault adapters, packaged data-protection keychain,
plaintext conversion/activation/rollback/low-disk/rename cases, rotation, portable
backups and production encryption selection remain open. Synthetic vaults suffice
for this component without pretending to qualify those remaining boundaries.

## Current live prerequisites

R140 is In Progress and blocked by R139; R141 is Backlog and blocked by R139/R140.
PR202 exact 8d8073f5 currently has Linux actual keyed handles SUCCESS; Windows is
running and macOS queued. PR193 exact ac14f582 has no current failure but its native
matrix is pending. Those pending results must be recorded separately from the
previous successful source qualification and any new local Store tests.

The dependency composite uses R134 `db04b366198ef17f44bce7a0243626784fe9948f`
and PR202 `8d8073f5da878684e63cdbc91d0c4d93cd6d86fc`. Only appended architecture
progress sections conflicted; both were retained. No generated file was edited.

## Implemented bounded slice — 8 October 2026

The native-only `native-keyed-store` feature now connects the qualified factory
to the real Store without changing `Store::open`, desktop startup or default
features. The factory supplies the writer, bounded read pool and maintenance
handles; every handle retains the same key session and exclusive lease through
actual native destruction. Store close first stops admission, drains SQLx, then
waits for causal owner release. Drop follows the same fail-closed path.

Migration26 adds one immutable `database_storage_identity` row containing only
the database UUID, key generation and fixed cipher profile. Plaintext Stores
leave it empty. Only a fresh keyed reservation inserts it; existing keyed data
must authenticate and match it before migrations or Ready publication. The
retained verifier owns the session through its worker and serializes Ready
publication. Interrupted creation without identity remains preserved and
refused rather than relabeled.

Ordinary Store/recovery entry points reject any key sidecar, including malformed
or pending metadata. Keyed backup and current plaintext recovery return an
explicit key-aware-recovery error before export or staging. This deliberately
leaves portable encrypted export/restore to RURU-141.

The actual SQLCipher 4.19.0 / SQLite 3.53.4 Store control creates schema26,
writes an authored account canary through Store, checkpoints through a keyed
maintenance connection, refuses plaintext backup/recovery and `Store::open`,
cold reopens with the exact identity/key, and reads the authored value. Raw DB,
WAL and journal scans contain no canary or plaintext SQLite header. The complete
factory suite passes 10 tests with one isolated exceptional-close child ignored;
the child separately proves a failed native close retains one owner, refuses
new admission and keeps the OS lease busy. Existing database-key units pass 13
with three explicit platform/subprocess helpers ignored; shutdown passes four;
plaintext recovery and historical migration integrations pass 14 and 12.

The first full run exposed four stale schema25 assertions plus the guarded-merge
prerequisite defect already repaired in final R134. After integrating signed
R134 head `ac87504e`, updating only current-schema expectations to26 and adding a
plaintext-recovery rejection control, the complete all-feature collaboration run
passes **1,204 tests / 8 explicit helpers ignored / 0 failed across 27 suites**.
The new restore-focused delta passes27, and the identity-claim rejection passes
without changing either input. Strict workspace all-target/all-feature Clippy,
rustfmt and diff checks pass. The factory's separate exact native suite remains
10 passed/one isolated helper ignored. No IPC signature changed, so typegen is
not required. These are local macOS synthetic-key results; exact-head remote
matrix evidence starts only after publication.

This slice does not activate encryption, convert plaintext databases, implement
rotation/loss/reset, qualify Windows or Linux production vaults, or provide
portable backups.

## Exact-head Windows repair — 8 October 2026

PR207's first Windows native-factory job exposed a fixture teardown error rather
than a keyed-store failure. The exceptional-close child wrote its complete
retained-owner evidence, returned to Rust's test harness, and then the harness
dropped the deliberately uncloseable SQLx connection. That expected native
`SQLITE_BUSY` Drop panic made the child exit 101, so the parent rejected otherwise
complete evidence.

The isolated child now exits successfully only after its evidence file is
durable. This follows the fixture's existing process-exit ownership contract and
prevents the test harness from running the deliberately invalid Drop path. The
complete native qualification runner passes locally with **10 passed / one
isolated helper ignored / 0 failed**, including the parent watchdog and retained
lease assertion. A replacement exact-head Windows result remains remote CI
evidence and is not inferred from this local macOS run.
