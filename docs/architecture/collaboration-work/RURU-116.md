# RURU-116 — Effective projections of durable intent

Status: design/integration in progress, 8 October 2026. No remote write UI or
provider mutation is enabled by this checkpoint.

## Base and ownership

Root owns the isolated external-volume `ruru-116-effective-intent` worktree.
The initial base is signed RURU-106 `a18eba4472802863ca3d894432ea0841a2f3eb89`,
which contains RURU-114 command admission and RURU-97 independent details.
RURU-115 is implementing migration 0016 and delivery in another worktree. This
slice will start schema work from its first buildable schema/recovery checkpoint,
use migration 0017, and publish on the completed RURU-115 branch. Never modify
already published migrations or qualify source using another branch's tests.

## Consistency contract

Provider observations remain authoritative input and never contain optimistic
values. Reviewed native operation codecs may produce bounded typed scalar effects
(title, body, state and notification unread) for their exact command target;
no arbitrary renderer JSON patch, path, provider identity, head OID or permission
change is accepted. Operation-specific validation chooses supported fields.
Store immutable, versioned effects tied to account, command and submission hash in
the same transaction as admission. The command's stable enqueue order determines
replay. Historical unsupported commands remain preserved without guessed effects.

Recompute a materialized effective item and bounded full-text projection whenever
its observed base or active effects change. List, detail, filtered count and
literal search query that same committed effective state. Identity, visibility,
coverage, authorization, comparison ranges and hydration continue to consult
provider evidence, never optimistic fields. Scope and body-detail revisions
advance together with the relevant change hint so old cursors and held SDK reads
cannot cross an effective change. Provider refresh replays pending intent over
the newly accepted base; it cannot overwrite the submitted effect.

Rejected/cancelled/superseded commands lose only their own effect. Remaining
successors replay in stable order over the current base. Accepted and ambiguous
commands retain visible pending intent; a provider acknowledgement is not proof
of completion. Confirmation requires the operation policy to commit its validated
canonical observation in the same writer transaction before retiring the effect.
RURU-115 supplies this finalization seam; this slice must not hide a flashback to
stale provider data with a renderer-only overlay.

Restored quarantine and old authorization epochs retain authored effects as
historical evidence but do not display them as newly authorized active intent.
Account, subject and repository visibility restrictions apply before disclosure.
Derived effective tables can be rebuilt from observations and immutable intent;
restart must reconstruct identical results. Backup/recovery policy explicitly
preserves and validates effect identity/bytes and clears or rebuilds derived
rows, with migration fault and historical upgrade coverage.

## Bounds and verification

Bound effects per command/target, encoded bytes and admitted active chains before
writing; do not scan every authored command during list navigation. Normalize
lookup/index keys, cap stored search body and page sizes, and keep SQLite work in
short transactions. Native command effects require no TypeScript optimistic store.
Any new query IPC is generated with `make typegen`.

Tests will exercise durable admission/retry, refresh during pending intent,
rejection with a successor, list/detail/count/search agreement, account and epoch
isolation, pagination fencing, cold restart, quarantine/restore and transactional
failure. SDK tests will prove change hints invalidate every affected consumer
across independently held clients. No production token, live provider mutation,
remote CI or platform qualification is implied by deterministic local fixtures.

## Integration checkpoint (not final qualification)

The first implementation adds immutable bounded effects, sparse effective item
rows/search, transactional list/count/detail/inbox query metadata, and native
change invalidation. Seven focused native tests pass: committed/query agreement,
refresh/rejection/restart replay, cursor fencing without provider-evidence edits,
transaction rollback, active-chain bounds, epoch isolation and restore quarantine.
IPC was regenerated from Rust (135 commands). Confirmation materialization and
full-stack qualification remain in progress; this checkpoint does not enable a
provider write operation. The unpublished 0017 active-target index includes the
authorization epoch so preserved older commands cannot lengthen current replay.
