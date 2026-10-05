# RURU-104 bounded detail retention and WAL maintenance contract

Status: signed pre-implementation contract, 5 October 2026. No implementation
or qualification is claimed by this document.

## Frozen input and delivery shape

This work starts from signed RURU-99 exact head
`2484bbeb5a392f7760d5ff537593128978ad4545`. Exact-head run 37295924539
passes all 14 reported checks: frontend, format/Clippy, Rust, ordinary packaged
E2E and the retained collaboration harness on Linux, macOS and Windows, plus
Cloudflare, Vercel and CodeRabbit. No CodeQL check is reported. RURU-97 exact
head `a23f9f126df09405173035886724c661d871c474` is already in that ancestry.

The review branch is `ruru/ruru-104-cache-retention`, stacked on
`ruru/ruru-99-draft-recovery`. It stays draft and unmerged. The first reviewable
slice is native-only:

- bounded accounting and eviction for rebuildable detail facets;
- durable, account-scoped pins for canonical pull-request and issue identities;
- truthful Missing coverage and revision/run fencing after eviction; and
- observable WAL/database growth plus nonblocking passive checkpoints.

This slice deliberately excludes summary/repository/identity eviction, automatic
maintenance cadence, physical compaction, frontend pin controls, Tauri commands,
outbox integration, and deletion of downloaded assets. Those require their own
measured contracts after this native core exists. Normal `make typegen` must
produce no public command change.

## Storage contract

Forward migration `0010_cache_retention.sql` adds only empty metadata tables. It
must not perform an unbounded historical backfill while opening a user's database.

`cache_pins` stores `(account_id, instance_id, entity_id, kind, pinned_revision)`.
Its key is the account/instance/canonical entity identity; its foreign key includes
the existing three-column identity primary key, while constant-time insert/update
triggers verify that the stored kind matches that exact parent and accepts only
pull requests and issues. This avoids building an unbounded identity index while
opening an existing database. A pin is authored, durable local state. It survives
disconnect, cache rebuilding and process restart, and one account's pin cannot
protect or reveal another account's same-named item.

`cache_retention_entries` stores one accounting row per
`(account_id, subject_id, facet)`, with logical JSON/blob bytes and the last
accepted observation revision. It references `detail_observations` with cascading
deletion. Every newly accepted detail commit updates its accounting row inside the
same writer transaction after entry reconciliation, so a crash cannot publish
content without its accounting or vice versa. The hook belongs inside shared
`details::apply_detail_in`, including notification-subject discovery paths that do
not call the public `Store::apply_detail` wrapper. Summary-driven head invalidation
can rewrite cached Body metadata, so that transaction also refreshes the Body
accounting row after its metadata mutation.

`cache_retention_state` stores the bounded historical-index keyset cursor, a
numeric-revision-plus-primary-key eviction scan cursor, aggregate indexed logical
bytes/facet count, and an `index_complete` flag. Insert/update/delete triggers on
the accounting ledger maintain those aggregates, including cascading cache resets,
so maintenance never needs an unbounded whole-ledger `SUM`. Maintenance indexes at
most 128 legacy facets per call and computes byte lengths in SQL without decoding
provider JSON.
Eviction is disabled until a prior call has established complete accounting; a
call that finishes the historical index returns without evicting. Empty/new
databases may complete indexing immediately, still preserving that phase boundary.
Before completion, usage is explicitly incomplete/unknown rather than reporting
the indexed prefix as the whole cache.

The byte metric is intentionally logical rather than a claim about exact SQLite
page ownership: it sums persisted observation, source, value-source, entry and
resource-metadata JSON/blob bytes. The maintenance report separately records main
database pages, free pages and physical database/WAL byte counts.

## Pin and maintenance API

The storage API is provider-independent and remains below Tauri:

```rust
Store::set_cache_pin(account_id, subject_id, pinned) -> Result<String>
Store::cache_usage() -> Result<CacheUsage>
Store::run_cache_maintenance(policy) -> Result<CacheMaintenanceReport>
Store::wal_status() -> Result<WalStatus>
Store::checkpoint_wal_passive() -> Result<WalCheckpointResult>
```

Pin changes validate the account's canonical instance and PR/issue identity,
work without an active provider credential, and publish a targeted local `pins`
revision only when state changes. Policy input is clamped internally: historical
indexing is at most 128 facets and eviction at most 32 facets per call. Zero or
oversized caller values cannot disable the hard caps or cause unbounded work.

Maintenance uses `try_lock` on the single local writer. If interactive storage
work owns it, maintenance reports `skipped_busy` and returns rather than queuing
behind the write. Candidate selection and protection are rechecked inside the
writer transaction. Each call examines at most 128 raw ledger rows before testing
protection, using a durable keyset cursor ordered by numeric accepted revision and
the account/subject/facet key. Reaching the end wraps the next call to the oldest
row, so protected prefixes cannot cause an unbounded eligibility scan and newly
unpinned rows are eventually reconsidered.

A call evicts at most 32 parent facets and at most 5,000 associated child-entry
rows in total. The existing per-facet 5,000-entry ceiling makes one maximum facet a
known unit of work; additional candidates are deferred when the remaining row
budget is insufficient. Oldest accounted facets are considered first and the call
stops at the byte target or either hard cap. Protected/ineligible bytes can leave
the cache above target and the report states that explicitly.

## Eviction safety and observable state

A detail facet is eligible only when all of these remain true under the writer:

- historical accounting was complete before this call;
- no durable pin exists for its exact account/instance/canonical identity;
- its exact detail demand is not requested;
- its exact sync scope is not `syncing`;
- its account, canonical PR/issue identity, summary item and sync scope exist; and
- it still has the accounting row selected by the bounded candidate query.

Eviction deletes only the `detail_observations` row. Entries, Body resource
metadata and accounting cascade from it. The same transaction rotates the exact
scope's `run_id`, clears continuation and HTTP validators plus completed-run
proof, writes Missing coverage, records a targeted change and assigns its revision
to `data_revision`. It preserves `access_denied` and the existing sync/error
metadata. A pre-eviction lease therefore cannot repopulate obsolete content, while
the UI sees uncached Missing state and can request hydration again.

The first slice never deletes or rewrites:

- drafts or their bodies/generations;
- summary items, repositories, canonical identities, aliases or pending aliases;
- accounts, credentials, credential-cutover/cleanup evidence;
- local repository links or notification discovery intent; or
- detail demand, including requested work.

These exclusions preserve minimum navigation identity and all current durable user
intent. When an outbox or other pending-effect table lands, its references must be
added to the protection tests before any summary/identity eviction is enabled.

## WAL contract

Observation uses `PRAGMA main.wal_checkpoint(NOOP)` only when the opened SQLite
version supports that mode (SQLite 3.51 or newer). The pinned bundled build is
3.51.3, but the existing WAL-reset gate also accepts older patched 3.44/3.50
builds. On those builds observation reports `supported=false`; it must not run a
PASSIVE checkpoint and disguise mutation as observation. Maintenance uses only
`PRAGMA main.wal_checkpoint(PASSIVE)` after its bounded transaction commits.
PASSIVE progress is recorded even when a long reader prevents all frames from
being checkpointed; later maintenance can continue. A busy writer causes an
immediate skip. This slice must not invoke FULL, RESTART or TRUNCATE checkpoints,
`VACUUM`, or manual WAL/SHM sidecar deletion.

NOOP and PASSIVE run outside SQL transactions. After committing retention and
dropping the application writer guard, checkpointing uses a dedicated connection
with zero busy timeout and a separate nonblocking maintenance guard. It therefore
does not hold the application's writer mutex while doing checkpoint I/O. PASSIVE
still has no frame/time limit; the report does not mislabel it as latency-bounded,
and negative frame counts mean unavailable rather than zero.

This boundary avoids conflicting semantics with divergent RURU-106 backup/restore
PR #145, whose verified snapshots and restore replacement own `VACUUM INTO` and
sidecar handling. Before those branches integrate, RURU-106 must learn schema 10
and prove pins/accounting survive backup and restore. Divergent RURU-121 PR #158
owns ephemeral navigation demand/prefetch; its working set is not a durable offline
pin. Frontend pin controls can integrate there later without changing this storage
meaning.

## Required evidence

Focused real-SQLite tests must prove:

- one call never indexes or evicts beyond its hard caps;
- protected candidate prefixes advance only the bounded scan cursor, eventually
  wrap, and never trigger a full eligibility or aggregate scan;
- eviction cannot begin while historical accounting is incomplete;
- accepted detail writes, complete-enumeration pruning, 304 validation and
  summary-driven Body metadata invalidation update accounting transactionally;
- pins, requested demand and syncing scopes are protected and account-isolated;
- drafts, identities, aliases, links and credential evidence remain byte-for-byte
  unchanged across maintenance;
- eviction produces Missing coverage, a targeted revision and a new run ID;
- stale pre-eviction commits fail and a fresh lease can rehydrate normally;
- an injected SQLite abort rolls back content, coverage, accounting and revisions;
- interrupted/reopened indexing remains correct when newly accepted observations
  sort before or after the saved historical cursor;
- a held read transaction remains usable while PASSIVE checkpoint returns bounded
  partial progress, followed by further progress after the reader closes; and
- migration from the frozen historical fixtures preserves all existing data and
  creates empty, valid retention metadata.

A separate ignored or explicit benchmark may build the architecture's large
synthetic dataset and record database/WAL growth, logical bytes, batch counts and
maintenance latency. Normal CI keeps its fixture bounded and deterministic. Local
checks, remote CI, benchmark output and any future live-provider observation must
remain separate claims. No personal credential, provider account or Gitru cloud
account is required for this work.

After focused tests, run Rustfmt, warnings-denied collaboration Clippy, the full
collaboration suite, migration suites, normal `make typegen` with a clean generated
diff, serialized `make verify`, and the repository's packaged E2E gates. Record
actual counts and limitations here before publishing a reviewable draft PR.
