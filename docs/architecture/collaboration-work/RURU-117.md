# RURU-117 — Conflict review and superseding intent

Status: continuation contract, 7 October 2026. Implementation and validation are
pending; this document fixes the native/UI boundary before code changes.

## Baseline and ownership

Managed external worktree `ruru-117-conflict-recovery`, branch
`ruru/ruru-117-conflict-recovery`, starts from signed RURU-116
`42c270c674b4ce11daf60f263f33ea7bce9b0886` and signed merge `c539b2cd` of
RURU-115 repair source `3e5f7f07`. Live Linear RURU-117 is Backlog, with
RURU-115 and RURU-116 prerequisites. Their review stacks are not merged releases.
Read architecture section 14 and this document when resuming.

The native lane owns common review DTOs, operation-specific recovery policies,
atomic storage actions, delivery/effective dependency integration, migrations and
backup preservation, runtime ownership and focused native tests. The coordinator
owns desktop commands, generated IPC, SDK/UI and product flow checks. No real
provider mutation codec is registered by this issue. Existing GitHub PAT/manual
or explicit CLI import and cloud-independent accounts remain unchanged.

## Recovery policy and comparisons

Only a compiled native recovery policy registered for the exact provider
installation, operation kind and payload version can decode an operation's
authored base and desired values, inspect the current raw provider observation,
or seal a replacement submission. A generic renderer payload, URL or boolean
cannot authorize provider dispatch or optimistic effects. Unknown codecs retain
their immutable intent and an exportable local recovery bundle; replacement is
unavailable with an explicit reason.

Common review fields are title, body, workflow state, unread state and inspected
head, each with explicit known/unknown values and a comparison classification.
The policy chooses which fields are applicable and editable. A bounded scalar
comparison helper permits independent fields when remote still equals base and
recognizes observed convergence when remote equals desired. Overlapping text,
unknown bases and changed workflow/head guards require visible review. Equality
alone is not evidence that a particular remote create succeeded. A later codec
must explicitly distinguish server-guarded writes from best-effort preflight.

Every review token is derived from the native account/view, command generation,
immutable hash and exact policy-produced current comparison frame. Replacement
recomputes this frame inside the writer transaction; stale body/head/access,
account epoch, view or delivery changes reject the action without losing text.
The UI keeps its edited resolution independently of refetches and can export it.

## Durable local actions

All actions carry a fresh action UUID and exact account, current account epoch,
authorization view and command-generation compare-and-swap. The native layer
persists an immutable, bounded action request and receipt in the same writer
transaction as state/projection changes. Exact retries return the existing
receipt; reuse of an action UUID with different bytes fails. Receipt values
include revision, resulting local state, pause status, replacement UUID if any,
and whether a remote action may already have happened.

- **Cancel:** only before any durable dispatch attempt. Keep the original
  envelope, guards, receipt, dependencies and evidence; retire its effect and
  re-evaluate successors. Never infer a remote undo.
- **Pause:** after an attempt, retain its sending/accepted/unknown/result state
  and all evidence, and stop future delivery/reconciliation turns. Serialize
  the action behind the owned delivery lane, allowing an in-flight result to
  finish durably before the pause receipt. No dispatched HTTP cancellation is
  represented as non-delivery.
- **Resume/check again:** clear only the local pause. Preserve account quota,
  age/attempt/probe limits, original authorization epoch and immutable recovery
  quarantine. Unknown/accepted commands can only reconcile; no blind retry.
- **Replace:** a native policy seals a new command UUID against freshly reviewed
  canonical state and explicit choices. Admission, immutable supersession edge,
  old command state and effective replay commit atomically. The old canonical
  envelope/hash, attempts and submitted dependencies never change. Unknown
  delivery cannot be replaced to bypass proof of non-delivery; restored
  quarantine cannot gain new dispatch authority through this action.

Native completion belongs to the runtime even if a caller disappears. These
local actions never load credentials or call a provider. Export likewise reads
only the selected account's preserved local intent and evidence, never a vault.

## Successors and execution order

An immutable edge links original A to replacement C with both hashes and the
original execution slot. Original submitted dependencies remain immutable.
C takes A's derived execution slot, so existing later B cannot block C merely
because C received a newer admission sequence. Effective replay uses the same
derived ordering, preserving later authored B intent over C.

Superseded A is never treated as confirmed. B's dependency may resolve through
C only after C has a valid canonical confirmation and B's native operation
policy validates that replacement provenance and current execution base. The
default is denial. Otherwise B stays visibly blocked and requires its own
review/replacement. Replacement chains are strictly forward by immutable
admission order and bounded during traversal; they cannot cross an account,
target or original actor epoch. Tests must cover A→B then replacement C without
FIFO deadlock, preserved A/B envelopes and no unjustified B dispatch.

## Storage and query bounds

Normalized immutable local-action receipts, supersession edges and mutable pause
state justify an ordered migration rather than unindexed mutable JSON aliases.
Migration **0019** is reserved, but must not be added until the coordinator's
frozen migration 0018 is merged. Do not rewrite migrations 0001–0018. Current
backup/restore must structurally validate and preserve these authored records;
add frozen-v18 migration fixtures and a real paused/superseded restore case.
Restoring still quarantines every retained command and never resumes delivery.

List queries return at most 50 shallow rows without loading command payloads or
evidence. Detail/export loads one bounded command. Fields and action history
have explicit limits. Keyset pages bind account/target/filter/view/revision;
cache invalidations use the existing `commands` scope and effective subject
revisions. No user-action event contains private authored text.

## Public native/UI boundary

The common Rust DTOs are the source for generated IPC; the coordinator will wire
the exact exported methods after their definitions compile:

- `command_recovery_list(query)` — shallow command summaries and cursor,
  revision and authorization view.
- `command_recovery_detail(account_id, command_id)` — summary, generation and
  epoch/view CAS, bounded base/remote/desired fields, review token, action
  availability (`can_retry`, `can_cancel`, `can_replace`) and reasons.
- `command_recovery_action(request)` — cancel/pause/resume with durable receipt.
- `command_recovery_replace(request)` — reviewed typed field choices, new
  command UUID and immutable receipt; no arbitrary operation payload.
- `command_recovery_export(account_id, command_id)` — explicit local intent
  bundle for the user's selected export destination.

The UI distinguishes cancel-before-send from pause-after-attempt, shows why
retry/replacement is unavailable, preserves edited text during revision updates,
and never labels accepted or unknown delivery as completed. Controls remain
honest when there are no registered production mutation codecs yet.

## Validation plan

Use task-owned temporary stores and synthetic native policies only. Cover
independent-field merge and overlapping text/workflow/head conflicts; stale
review/auth/view/generation rejection; atomic fault rollback and exact action
retry; cancel versus pause with a held dispatch result; immutable supersession
and successor ordering/proof; account isolation, unknown codec export, bounded
queries and schema/recovery preservation. Native tests must prove no extra
dispatch after pause, unknown result or restored quarantine. Run focused/full
collaboration tests, strict Clippy, generated IPC and meaningful UI tests.
Record local checks separately from remote CI and live-provider/platform checks.
