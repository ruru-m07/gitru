# RURU-105 — Schema evolution and migration recovery

Issue: [RURU-105](https://linear.app/catra/issue/RURU-105/test-schema-evolution-and-recoverable-migration-failures).
Architecture: [remote collaboration engine](../remote-collaboration-engine.md),
section 19. Foundation: `baafef75` (PR #141). Integration base: RURU-95 credential
cutover `449db366afab455b751cd691f0deb8aa421105dc`.

## Implementation

The migration suite builds an independent historical v1 database from frozen SQL,
literal JSON/seed data, and the original migration checksum. It does not create
the baseline by calling the current `Store` or serializing today's DTOs. The
fixture preserves four account identities, epochs, three authentication states,
overlapping provider IDs, private FTS rows, denied observations, partial sync
checkpoints, authored drafts and durable revision metadata.

The current production `Store::open` must upgrade and reopen the fixture while
preserving those rows and keeping private reads fenced by account and access.
The fixture checksum rejects changes to the already-applied v1 migration.
New forward migrations automatically join this upgrade gate.

The suite now tests the actual `0002_credential_cutover.sql` migration from
RURU-95. Active and auth-required legacy accounts retain their original vault
references; disconnected references become durable cleanup work. Reopening must
not duplicate or reset that work. The historical v1 SQLx migrator refuses the
upgraded v2 ledger without changing drafts, projections, references or cleanup
metadata. This characterizes the previous migration policy without claiming to
run a published older desktop binary.

Recovery cases verify that future-schema, dirty-ledger and modified-checksum
errors retain all authored/projection data and migration bookkeeping, both before
and after the v2 upgrade. Upgraded failures also preserve committed credential
references and pending cleanup metadata. Failed
bootstrap releases the OS writer lease. Repairing deliberately injected fixture
metadata demonstrates reopen without adding an automatic repair/reset feature to
production.

A deliberate late-table collision runs the actual initial production migration
and verifies transactional rollback of earlier DDL, including FTS shadow tables,
while preserving pre-existing authored text. Additional synthetic pending
migrations run through the pinned SQLx SQLite migrator to inject:

- A SQL error after modifying drafts and inserting a backfill row.
- `SQLITE_INTERRUPT` after an observed backfill insert, with no timing guesses.
- `SQLITE_FULL` from a real SQLite page-allocation limit.
- Hard process termination while a backfill insert is uncommitted and the child
  owns a real `Store` writer lease. The parent verifies `Busy` before termination,
  then reopens and verifies rollback and OS lease release without destructors.

Each case verifies data/schema/ledger rollback and database integrity. The first
three also retry an unapplied corrected fixture migration, reopen it and rerun it
to prove that it commits once. The subprocess is bounded and always killed/reaped
on a failing assertion. Its ignored helper is explicitly invoked by the parent
test; it is not a skipped recovery case.

No runtime, provider, production storage method, migration, or wire contract was
changed by this issue. `make typegen` is therefore not applicable.

## Validation and boundaries

Local validation on 3 October 2026, integrated with RURU-95:

- All 12 migration cases pass. The subprocess helper is marked ignored in the
  normal harness and invoked explicitly by its parent case.
- The full collaboration suite passes 70 tests: 33 unit, 12 migration, 9 runtime
  and 16 storage. Both ignored entry points are helpers invoked by passing crash
  tests, not omitted recovery cases.
- Crate Clippy across all targets with warnings denied, workspace formatting,
  and diff whitespace checks pass.

This provides a real frozen v1-to-v2 production-schema upgrade check, while
distinguishing those schemas from published desktop releases.

Commands (using a shared build cache, with isolated temporary databases):

```sh
cargo test -p collaboration --test migrations
cargo test -p collaboration
cargo clippy -p collaboration --all-targets -- -D warnings
cargo fmt --all -- --check
```

When multiple worktrees share `CARGO_TARGET_DIR`, an outer lock must cover the
whole build-and-test command, and collaboration artifacts must be invalidated
when switching worktrees. Cargo's own build lock does not protect executables
while tests run, and timestamp freshness can reuse another worktree's test binary
when the current worktree's sources have older timestamps. Final local results
were obtained with an outer command lock and
`cargo clean -p collaboration` before full-suite validation; mismatched intermediate
shared-cache runs were discarded.

Evidence must distinguish macOS local execution from remote Windows/Linux runs.
`SQLITE_FULL` tests database allocation exhaustion; they do not simulate every
filesystem failure or a full physical volume. Synthetic migrations are
transaction-recovery fixtures, not supported Gitru schema versions. Backup and
restore belong to RURU-106; platform-native vault/storage verification belongs to
RURU-107. There is currently no large production backfill requiring a progress
UI, and no changed wire contract requiring generated-client compatibility work.
