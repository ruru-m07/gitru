# RURU-134 catalog storage and schema25 recovery

This is a qualification companion to `RURU-134-metadata.md`. The native catalog
is advisory provider cache; durable draft selections and delivery evidence use
the separate authored tables and frozen version-two codec. Neither cached labels
nor a completed traversal confers mutation permission or proves remote absence.

## Catalog invariants

Each family lease binds account actor, authorization epoch/view, native repository
identity/path, family, unique generation, cursor/page count and catalog revision.
Every request/publication rechecks this under native storage authority. A failure,
cold Syncing retirement, permission denial or eviction changes the catalog
revision and rejects held pages. Deselecting a repository purges its catalog;
reselecting it cannot revive an older lease.

Local keyset queries bind normalized search, account/repository/family, epoch,
view and catalog revision. They survive unrelated authored/global revisions.
Options are bounded to 100 per provider page, 20 pages per traversal, 2000 per
family, 6000 per account and 48 family headers per account. Retention advances
invalidation hints and marks advisory data stale; it preserves terminal traversal
state. Empty, partial and capped refreshes never infer deletion. A terminal cap
has no continuation and the next refresh begins a fresh generation. Five-minute
freshness is only advisory and cannot override access denial or retained stale
rows. Authored drafts are independent of catalog retention.

Eleven finite SQLite controls cover tuple ordering/search/family binding,
unrelated revision stability, cold continuation and lease retirement, held-writer
view changes, denial visibility, old epochs, repository path drift,
deselect/reselect, malformed/duplicate pages, empty/capped non-pruning, each
retention limit, unique generations after eviction, rate-limit checkpoint recovery
and private draft preservation. They pass within R119's integrated metadata
selector in `/tmp/gitru-r134-metadata-integrated.log`.

## Restore acceptance

Schema 25 accepts both existing payload 1 and new payload 2 issue creation, dispatching
validation by the stored version. Existing version-one codec bytes are unchanged.
The new authored child stores canonical bounded IDs and selected display values;
its identity is immutable and it is retained through provider-cache resets.
Equal-generation submissions must match title/body/selections and their content
hash. Later edited generations may coexist with immutable older submissions.

Every possible version-two POST requires the original epoch and exact canonical
PreparationV2 in its durable attempt context. This codec permits at most one
attempt. Confirmation requires the exact referenced `github.issue_created/v2`
ordinal, matching actor/command hash/repository/selection, canonical core receipt,
and the authored issue mapping in both directions. Unknown optional metadata is
retained as explicit unobserved evidence; it is never converted to a matched
selection. Zero-attempt `github.issue_creation_declined/v2` evidence requires its
own conflict resolution and can only remain Conflict or become Cancelled. This
codec does not produce Accepted, Rejected or Superseded states.

Restore preserves these bytes and selected names, quarantines nonterminal intent,
requires reauthorization and purges catalog authority. It cannot reconstruct a
send grant or bypass reconciliation. The frozen schema24 SQL/checksum joins the
historical 1–24 matrix. A failure injected after schema 25 DDL must roll back the
old trigger/index/table structure and authored rows before a clean retry.

## Qualification status

The full recovery-related unit selector passes 38 tests with one subprocess
helper ignored, including legacy v1/PR controls and the new five-state restore
and 26-corruption matrices. Both integration suites pass: 14 recovery tests and
12 migration tests, including every recognized historical schema 1–24 and
schema 25 failure rollback. Logs are `/tmp/gitru-r134-recovery-all.log` and
`/tmp/gitru-r134-recovery-integrations.log`.

The final valid-state restore control also passes 1/1 after adding catalog purge
and authored-name retention assertions (`/tmp/gitru-r134-metadata-purge-restore.log`).
Strict collaboration all-target/all-feature Clippy passes on the same combined
source in R119's native qualification; the redundant second Clippy run was
stopped after that result, rather than counted as another pass. The native/catalog checkpoint is `c271bbf2`; full
workspace checks remain with the parent task. No live provider, credential
vault, packaged application or remote CI result is implied here.
