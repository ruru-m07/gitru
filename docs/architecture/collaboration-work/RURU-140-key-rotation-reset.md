# RURU-140 bounded slice: database-key rotation, loss, reset and reinstall

This slice starts from packaged keyed startup at `930fb02`. It adds native-only
lifecycle transactions for an already encrypted collaboration database. It does
not add the RURU-141 portable backup envelope or the RURU-139 performance runner.

## Rotation contract

Rotation reserves generation `N + 1` under the same immutable database ID. The
vault entry uses the existing database-key namespace and create-only semantics.
The current generation is loaded first and remains present throughout rotation.
If preparation is cancelled after reservation, the next attempt reuses that exact
next-generation entry; it never overwrites it.

A native boundary must create, authenticate, integrity-check, checkpoint, and
close a candidate encrypted with the reserved key. Only the resulting unsafe
`VerifiedRotatedDatabase` proof can enter the filesystem transaction. The
candidate must carry ready metadata for the exact database ID and next generation.
An explicit preview confirmation writes a durable marker, moves the current
ciphertext and metadata into a recovery bundle, then publishes the candidate and
its metadata together. Wrong identity, uncheckpointed sidecars, stale bytes,
missing/wrong keys, or interrupted markers fail closed without plaintext fallback.
The old key and recovery bundle are retained.

## Key loss and account independence

A missing, locked, malformed, or wrong database key continues to preserve the
existing database, metadata, sidecars, drafts, and recovery evidence. Startup
cannot regenerate a key when any database or key-lifecycle evidence exists.
Database keys remain scoped to the local database identity, not a provider
account. Provider account disconnect therefore has no database-key delete path;
the `DatabaseKeyVault` contract still deliberately exposes no deletion operation.

## Explicit reset and reinstall

Reset requires a preview-specific confirmation. It hashes the closed database and
metadata, publishes a durable recovery marker, and moves both into a uniquely
named quarantine directory. It never deletes the associated vault key. A crash
while moving files makes ordinary keyed and plaintext startup refuse the path.
`InterruptedDatabaseKeyLifecycle::rollback` restores the exact old generation and
retains any partially published next generation as evidence.

After a completed reset, `ExistingOnly` startup still refuses creation. A caller
must separately authorize `AllowNew`, which creates a new database ID and key;
there is no automatic reinstall regeneration over existing or quarantined data.
Quarantine deletion and old vault-key retirement require a future explicit
retention policy and are outside this slice.

## Validation record

Focused lifecycle tests cover next-generation reservation and resume, old-key and
recovery-bundle retention, wrong/uncheckpointed candidates, explicit reset,
missing-key fail-closed behavior inherited from the bootstrap suite, and an
interrupted reset with exact rollback. Full native feature tests, strict Clippy,
rustfmt and diff checks are recorded on the PR after they complete. Remote CI and
live platform-vault behavior remain separate evidence.
