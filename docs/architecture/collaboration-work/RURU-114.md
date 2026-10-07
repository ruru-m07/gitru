# RURU-114 — Durable command admission and outbox schema

Status: native admission locally qualified on 8 October 2026 at signed source `bc01d7d0044d0325437d28dbb1a37598eb0e4837`. Independent review corrections and full local verification pass. No RURU-114 PR is open yet; publication and remote CI remain the next gates.

Baseline: exact signed RURU-124 evidence head `4d31a40743d3e203f2fc1027f17b4ae292268478` (draft PR #169), stacked on RURU-137 in an isolated managed worktree on the external volume. The declared RURU-76, RURU-97, RURU-105 and RURU-104 prerequisites are implemented in this ancestry and remain In Review; none is claimed merged. The native canonical-model commits were rebased with signatures onto this exact dependency; no competing migration 0012 was introduced.

Live issue: [RURU-114](https://linear.app/catra/issue/RURU-114/add-durable-command-admission-and-outbox-schema), “Add durable command admission and outbox schema”.

## Outcome

The native engine can atomically admit an already validated, operation-specific remote command as immutable user intent and return a durable local receipt. A lost IPC response is safe to retry: the same account and UUID with byte-identical canonical submission returns the original receipt, while any changed target, operation, payload, guard or dependency is rejected without mutation.

This slice establishes storage and Rust APIs only. It exposes no renderer mutation control, performs no provider request, loads no credential and applies no optimistic domain projection. RURU-115 owns dispatch, attempts and ambiguous-outcome reconciliation; RURU-116 owns effective optimistic views; operation tasks own their typed public commands and capability rules.

## Native contract

```text
CommandSubmission
  command_id                 canonical lowercase UUID
  account_id
  authorization_epoch
  operation                  closed native kind plus payload schema version
  target                     canonical kind/id plus optional repository id
  payload                    operation codec's bounded canonical value
  guards                     ordered typed concurrency observations
  dependencies               ordered unique predecessor command IDs

CommandReceipt
  account_id, command_id
  submission_hash            lowercase SHA-256 of the domain-separated envelope
  enqueue_order              durable account-local order
  admitted_revision          committed collaboration revision
  admitted_at                immutable UTC admission timestamp
  duplicate                  true only for an identical retry
```

Admission accepts a sealed Rust value produced by an operation codec. The store does not accept a renderer-supplied provider name, host, raw URL, arbitrary SQL state, delivery state or already-computed hash. The account determines provider installation and actor. A future generated IPC command must parse an operation-specific DTO before it reaches this generic store.

Only test fixture operation codecs currently construct submissions. Production operation variants land with their owning issues. The native policy must match both the sealed operation kind and payload version before any transaction can admit it; unknown versions fail closed.

## Canonical immutable submission

The engine encodes one versioned binary envelope with explicit field tags and length prefixes. It contains operation kind/version, account-bound target, authorization epoch, guards, dependency IDs and canonical payload bytes. UUID and admission timestamps are keys/receipt facts rather than mutable payload. Strings are UTF-8 after existing identifier/text validation; integers use fixed unsigned decimal or fixed-width binary forms; no floating point or unordered map reaches the encoder.

Operation codecs use typed structs and deterministic field order. If a future operation needs map-like data, the codec sorts unique keys before encoding and rejects duplicates. Dependencies preserve submitted order, reject duplicates and must name already admitted commands under the same account. The exact envelope bytes and their domain-separated SHA-256 are persisted. Equality checks compare both bytes and all decomposed indexed fields, never the digest alone.

Payload migrations may add a derived execution representation in a separate table. They never rewrite the original envelope, payload bytes, guards, dependencies or submission hash. Conflict resolution with changed intent creates a new UUID and an explicit supersession/dependency edge.

## Schema

RURU-124 migration 0012 is frozen in the baseline. RURU-114 adds `0013_command_admission.sql`. The admission receipt records immutable commit facts; current delivery state is stored separately and will gain its own query/transition API in RURU-115. Admission always inserts `queued`.

The forward migration adds:

* `commands`, keyed by `(account_id, command_id)`, containing authorization epoch, operation kind/version, target kind/id/repository, exact canonical envelope, payload bytes, submission hash, enqueue order, admitted revision/time and constrained delivery state;
* `command_dependencies`, keyed by command plus ordinal, with a same-account composite foreign key to the predecessor and the predecessor's immutable submission hash captured at admission;
* `delivery_attempts`, keyed by command plus monotonically increasing attempt number, ready for durable dispatch-start/outcome evidence but empty in ordinary RURU-114 admission;
* `command_evidence`, bounded typed immutable receipt/attempt evidence rows for later reconciliation;
* `command_target_protections`, normalized account/entity/facet/blob references used by bounded retention without decoding payload JSON.

The initial constrained states match the architecture state machine: `queued`, `sending`, `retry_wait`, `accepted`, `confirmed`, `outcome_unknown`, `conflict`, `rejected`, `cancelled`, and `superseded`. RURU-114 creates only `queued`. Later transitions require their own compare-and-swap APIs and evidence; generic SQL callers cannot select an arbitrary state.

All relations include `account_id`; cross-account dependencies and target protection are impossible by foreign key and validation. Account disconnect/auth loss keeps immutable user intent and receipt evidence while blocking later dispatch. Command rows are user-authored durable state rather than rebuildable provider cache.

## Admission transaction

One serialized writer transaction:

1. accepts only a bounded sealed native submission and matches its operation kind/version to the native policy;
2. requires the exact active account authorization epoch supplied when the operation was composed;
3. on an existing `(account_id, command_id)`, compares the stored envelope, payload, guards, hash, ordered dependencies and every decomposed immutable fact; identical retries return the original receipt with `duplicate=true`, otherwise a typed idempotency conflict;
4. for new intent, invokes the operation's local policy against the same writer snapshot, including target/base identity and supported offline admission;
5. resolves every ordered dependency under the same account and captures its immutable hash; missing/cross-account/forward edges fail before admission, while construction rejects self/duplicate edges and more than 64 dependencies;
6. allocates account-local enqueue order and one collaboration revision, inserts the exact previously sealed envelope plus dependencies and normalized retention protections, and emits a bounded `commands` change record;
7. commits before returning the receipt.

Retry ordering is deliberate: an already committed identical intent does not rerun mutable base policy, so a lost response remains recoverable after cache changes. Active epoch validation still applies; disconnected/replaced accounts can read their authored receipt through the native recovery query but cannot resubmit old intent. The generic layer recomputes neither renderer JSON nor a caller-supplied digest: reviewed crate-private codecs seal and hash the complete envelope before storage receives it.

No provider/vault/scheduler work occurs in admission. A SQLite error or process crash before commit leaves no partial command, dependency, protection, revision or receipt. A crash after commit can lose the caller response but an identical retry returns the same receipt and revision. Concurrent identical submissions converge on one row; concurrent changed submissions produce one winner and one typed conflict.

## Bounds

* canonical envelope and payload: at most 256 KiB each;
* operation kind: 128 bytes; payload version: positive bounded integer;
* target/repository/command identifiers: existing bounded canonical ID rules;
* at most 32 guards, 64 dependencies and 128 normalized target protections;
* evidence payload: at most 64 KiB per row and 128 rows per command;
* UUIDs must parse and round-trip to canonical lowercase hyphenated form;
* hashes are fixed 32-byte values internally and lowercase hex only at API/document boundaries.

Oversized or malformed input fails before a write. Stored diagnostics never contain credential material or unbounded provider response bodies.

## Retention and recovery

RURU-104 retention gains indexed `NOT EXISTS` protection checks for command references. Rebuildable targets, predecessor receipts and attempt/evidence references needed by `queued`, `sending`, `retry_wait`, `accepted`, `outcome_unknown` or `conflict` commands cannot be evicted or garbage-collected. Rejected/cancelled/confirmed/superseded history can become eligible only under a future explicit receipt-history policy; RURU-114 does not delete it.

Protection lookup uses normalized rows and bounded indexed joins. It never scans/decompresses payloads during an eviction pass. Pending command rows, envelopes, dependency edges, receipts, evidence and intent-referenced blobs are excluded from ordinary cache byte budgets and LRU deletion. If a protected target is already absent, the command remains durable and later delivery must report missing base/conflict rather than fabricate an observation.

Backup/restore and migration recovery treat the new schema as user intent. Upgrade is one forward transaction; a checksum mismatch, interruption, `SQLITE_FULL`, `SQLITE_INTERRUPT` or newer schema preserves the prior database. Frozen pre-command fixtures upgrade with accounts, drafts, revisions and provider cache byte-for-byte unchanged except declared schema metadata. Reopen verifies original envelope/hash without reserialization.

## Required evidence

Storage and migration tests cover:

* first admission and exact receipt/revision atomicity;
* identical retry before and after restart returning one receipt;
* same UUID with changed payload, target, operation, guard, dependency order, epoch or account partition;
* two concurrent identical writers and two conflicting writers;
* missing/self/duplicate/cross-account/forward dependencies and predecessor-hash capture;
* inactive/replaced epoch, missing base and operation-policy rejection;
* envelope/hash golden vectors, Unicode, boundary sizes and collision defense through byte comparison;
* crash before commit, hard crash after commit before response and cold retry;
* frozen-schema upgrade, migration rollback/faults, newer-schema refusal and draft preservation;
* retention under small budgets proving command target, predecessor, evidence and referenced-blob protection without unbounded scans;
* disconnect/restart persistence with zero provider and vault calls.

Default `make verify`, focused fault tests, formatting/Clippy and `git diff --check` are required. `make typegen` is required only if an actual public Tauri signature lands; generated files are never edited by hand. Packaged restart coverage is required if the retained harness or public command surface changes. Exact-head remote CI and other-platform execution remain separate evidence.

## Ownership and delivery order

This worktree owns the RURU-114 contract, next command migration after RURU-124, command domain/canonicalization/store modules, retention protection integration and focused native tests. It does not touch provider adapters, scheduler delivery, optimistic UI, generated IPC without a public API, credentials, Gitru cloud, or the RURU-118/RURU-124 worktrees.

Delivery order is deliberate: this branch is rebased on the exact signed RURU-124 head, owns migration 0013, and will publish a stacked draft only after source review and local checks. No merge is authorized.

## Implementation evidence — 8 October 2026

* Native `storage::command_admission` has a crate-private operation policy, immutable receipt lookup, and exact retry/dependency admission. Only test codecs construct operations in this slice; no public IPC signature changed, so type generation is not required.
* SQL protects immutable command facts/dependencies/evidence and policy-produced protection identities, constrains account partitions, caps evidence at 128 rows/64 KiB per row, and orders attempt identities. Dispatch transitions are deferred to RURU-115.
* Retention uses a partial index of required normalized references, maintained by transactional command/dependency triggers so terminal receipt history does not become an eviction scan. It preserves targets of six pending/recovery states, including terminal predecessors still needed by pending successors. Commands, receipts, attempts and evidence are never ordinary cache victims. Blob references are durable normalized records; there is no blob-store eviction implementation in this slice, so actual attachment recovery remains future work.
* `cargo test -p collaboration --lib command_admission`: **13 passed**, one subprocess helper ignored as a standalone test; the parent executes that helper for two real hard-kill/cold-reopen boundaries. Focused tests also cover concurrent admission, Unicode, changed guards/operation, hash-plus-byte collision defense, transaction rollback, dependency/account isolation, protection bounds, recovery after disconnect and an `EXPLAIN QUERY PLAN` assertion for the actual retention query.
* `cargo test -p collaboration --test command_migrations`: **5 passed**. Frozen v12 source/seed bytes retain original account/draft/cache/local-inbox/ledger rows through upgrade. Real `SQLITE_FULL` and `SQLITE_INTERRUPT`, partial-schema failure, checksum mismatch and newer-schema refusal preserve prior bytes; corrected retries succeed.
* `make verify` passes on final signed source `bc01d7d0044d0325437d28dbb1a37598eb0e4837`: 683 frontend tests (one platform fixture skipped), all lint/type/build gates, workspace formatting and warning-denied Clippy, all default Rust workspace suites, including 349 collaboration library tests (two standalone subprocess helpers ignored) and all integration suites. The new worktree dependencies were installed with `bun install --frozen-lockfile --backend=copyfile`; no lockfile changed. Focused checks and `git diff --check` also pass. Full local log: `/tmp/gitru-ruru-114-verify-final.log`.
* No remote CI, other-platform execution, real provider request, credential-store access, packaged UI or dispatch/attachment recovery is claimed by these local fixtures.

Independent source review found that normalized protection identities also need
immutable update/delete guards because exact retries intentionally skip local
policy. Signed follow-up `bc01d7d0044d0325437d28dbb1a37598eb0e4837` adds those
guards, validates derived required-flag updates against authoritative command and
dependency state, and proves both rejection and valid terminal-state propagation.
The corrected focused admission suite remains 13/13 passing.
