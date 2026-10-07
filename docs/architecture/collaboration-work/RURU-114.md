# RURU-114 — Durable command admission and outbox schema

Status: pre-code contract frozen on 7 October 2026. Implementation waits only for the concurrently active RURU-124 migration to freeze so this branch can stack on it and take the next schema version without renumbering either review.

Baseline: exact signed RURU-137 evidence head `0da82cc7e5a3543512be683e68ac074f8d9689cd` in an isolated managed worktree on the external volume. The declared RURU-76, RURU-97, RURU-105 and RURU-104 prerequisites are implemented in this ancestry and remain In Review; none is claimed merged. No RURU-114 branch, worktree or pull request existed at the live audit.

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
  state                      queued for this slice
  duplicate                  true only for an identical retry
```

Admission accepts a sealed Rust value produced by an operation codec. The store does not accept a renderer-supplied provider name, host, raw URL, arbitrary SQL state, delivery state or already-computed hash. The account determines provider installation and actor. A future generated IPC command must parse an operation-specific DTO before it reaches this generic store.

Initial code may include one feature/test-only fixture operation so storage can be proven without prematurely exposing title, comment, merge or inbox writes. Production operation variants land with their owning issues. Unknown operation or payload versions fail closed.

## Canonical immutable submission

The engine encodes one versioned binary envelope with explicit field tags and length prefixes. It contains operation kind/version, account-bound target, authorization epoch, guards, dependency IDs and canonical payload bytes. UUID and admission timestamps are keys/receipt facts rather than mutable payload. Strings are UTF-8 after existing identifier/text validation; integers use fixed unsigned decimal or fixed-width binary forms; no floating point or unordered map reaches the encoder.

Operation codecs use typed structs and deterministic field order. If a future operation needs map-like data, the codec sorts unique keys before encoding and rejects duplicates. Dependencies preserve submitted order, reject duplicates and must name already admitted commands under the same account. The exact envelope bytes and their domain-separated SHA-256 are persisted. Equality checks compare both bytes and all decomposed indexed fields, never the digest alone.

Payload migrations may add a derived execution representation in a separate table. They never rewrite the original envelope, payload bytes, guards, dependencies or submission hash. Conflict resolution with changed intent creates a new UUID and an explicit supersession/dependency edge.

## Schema

RURU-124 owns migration 0012 on the concurrent stack. RURU-114 will rebase onto its frozen source before schema work and use the next migration, expected `0013_command_admission.sql`; it must not publish a competing 0012.

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

1. validates bounds and canonical UUID;
2. reads the account/provider identity and requires the exact active authorization epoch supplied when the operation was composed;
3. invokes the operation codec's local admission policy, including target kind, current cached identity/base requirements and supported offline admission;
4. resolves every dependency under the same account, captures its immutable hash and rejects duplicates, missing predecessors, forward/self edges and a dependency count above the bound;
5. constructs canonical envelope bytes and computes the domain-separated hash internally;
6. on an existing `(account_id, command_id)`, compares the complete stored envelope and decomposed facts; exact equality returns its original receipt with `duplicate=true`, otherwise returns a typed idempotency conflict;
7. otherwise allocates account-local enqueue order and one collaboration revision, inserts command, dependencies and normalized retention protections, emits a bounded `commands` change record, and commits;
8. returns the receipt only after the durable commit.

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

Delivery order is deliberate: freeze this contract, wait for RURU-124's migration/source commit, rebase this branch on that exact signed head, then implement and publish a stacked draft. No merge is authorized.
