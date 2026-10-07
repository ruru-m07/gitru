# RURU-117 — Conflict review and superseding intent

Status: native and desktop implementation under qualification, 8 October 2026.
The signed continuation contract preceded implementation; this record now
describes the implemented boundary and explicit remaining validation.

## Baseline and ownership

Managed external worktree `ruru-117-conflict-recovery`, branch
`ruru/ruru-117-conflict-recovery`, is based on the cleaned RURU-123 source
`5a30303`. That review stack includes the explicit RURU-116/RURU-118/RURU-126
integration base; it does not include RURU-127 or RURU-128. The original
implementation ancestry is preserved in `ruru/ruru-117-pre-restack-869e26b`.
Live Linear RURU-117 is In Progress with RURU-115/RURU-116 prerequisites;
these review stacks are not merged releases. Read architecture section 14 and
this document when resuming.

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
local actions never load credentials or call a provider. Export reads only the selected account's preserved authored payload, immutable
hash, safe summary and declared comparison fields. It excludes execution
evidence, raw provider responses, guard internals, credentials and vault references.

## Successors and execution order

An immutable edge links original A to replacement C with both hashes and the
original execution slot. Original submitted dependencies remain immutable.
C takes A's derived execution slot, so existing later B cannot block C merely
because C received a newer admission sequence. Effective replay uses the same
derived ordering, preserving later authored B intent over C.

Superseded A is never treated as confirmed. This slice never substitutes C for
B's immutable dependency on A, even after C confirms. B stays visibly blocked
and requires its own review and a new policy-validated intent; no inferred
dependency rewrite creates dispatch authority. Replacement chains are strictly forward by immutable
admission order and bounded during traversal. A replacement dependency must
have an earlier inherited execution slot, preventing policy-introduced FIFO
cycles. They cannot cross an account,
target or original actor epoch. Tests must cover A→B then replacement C without
FIFO deadlock, preserved A/B envelopes and no unjustified B dispatch.

## Storage and query bounds

Normalized immutable local-action receipts, supersession edges and mutable pause
state justify an ordered migration rather than unindexed mutable JSON aliases.
Migration **0019** follows frozen migration 0018. Migrations 0001–0018 remain
unchanged. Current backup/restore structurally and semantically validates
versioned action requests/receipts, immutable replacement affinity/order, and
pause state. Frozen v18 SQL/checksums extend the historical restore matrix; a
real paused/superseded synthetic store exercises current backup/restore.
Restoring still quarantines every retained command and never resumes delivery.

List queries return at most 50 shallow rows without loading command payloads or
evidence. Detail/export loads one bounded command. Review fields are limited to five, each known scalar to 64 KiB, and the whole
native review to 256 KiB. Immutable action history permits 64 receipts and 1 MiB
of requests per command. Replacement chains have at most 16 edges. Account and
target keyset indexes avoid scanning terminal history for pending pages. Keyset pages bind account/target/filter/view/revision;
cache invalidations use the existing `commands` scope and effective subject
revisions. No user-action event contains private authored text.

## Public native/UI boundary

The common Rust DTOs are the source for generated IPC. The desktop registers
these five native commands, with account-scoped SDK query/action methods:

- `command_recovery_list(query)` — shallow command summaries and cursor,
  revision and authorization view.
- `command_recovery_detail(account_id, command_id)` — summary, generation and
  epoch/view CAS, bounded base/remote/desired fields, review token, action
  availability (`can_retry`, `can_cancel`, `can_pause`, `can_replace`) and reasons.
- `command_recovery_action(request)` — cancel/pause/resume with durable receipt.
- `command_recovery_replace(request)` — reviewed typed field choices, new
  command UUID and immutable receipt; no arbitrary operation payload.
- `command_recovery_export(context)` — explicit local intent
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


## Implementation details and qualification checkpoint

Exact actions recheck account epoch, authorization view, command generation and
native comparison token inside the same writer transaction. The account/view
check also precedes exact action retry. Identical requests return byte-stable
receipts without replaying effects; changed bytes under an old UUID are rejected.

Replacement retires the reviewed original inside the writer transaction before
calling the concrete native admission policy. This reserves exactly its existing
effect slot at the 64-active-effects limit. New admission, validated immutable
supersession/action evidence, final inherited-order replay and the receipt commit
atomically. Any failure rolls back original state, generation, retained-base
protection, projections and revisions. No reader sees the intermediate state.

Local pause clears no provider evidence, attempt/probe budgets or retry deadline.
Delivery candidate scans exclude paused commands and writer claims independently
recheck pause. Native runtime actions share the owned delivery lane: a held
response completes before an action can commit, and its generation change rejects
an older review. Dropping a caller does not interrupt admitted action completion;
runtime shutdown drains it before releasing storage.

Local native qualification: the final storage-action suite passes **17/17**,
including exact-cap replacement/fault rollback, dependency ordering, typed
conflict choices, account/epoch/CAS/concurrent retry, query plans, immutable
schema, pause budgets and current backup/restore. The broader recovery suite
passes **35**, with one subprocess helper intentionally ignored. Integration
recovery **14/14** and migration policy **7/7** pass, including frozen v1–18
restoration and actual interrupt/full-disk fixtures. Strict collaboration
all-target Clippy passes. An earlier full native library run passed **542** with
four helper ignores; final workspace validation follows clean publication
replay. The later resume-boundary regression confirms that eight mutation
attempts do not disable bounded read-only reconciliation or clear the counter.

Logs: `/tmp/gitru-r117-actions-final.log`,
`/tmp/gitru-r117-recovery-final.log`, `/tmp/gitru-r117-migrations-final.log`,
`/tmp/gitru-r117-native-clippy.log`. The coordinator separately reported SDK and
desktop tests/types; it owns generated IPC and final full-workspace validation.
Remote CI and live provider write checks have not run for this slice, and no
production write codec is enabled. Publication must use the coordinator's
cleaned R123 dependency base; the temporary implementation ancestry includes
unrelated inherited branch work.


Desktop and client qualification before the clean dependency replay: 768 tests
passed with one existing skip; all four focused recovery UI tests then passed,
including failed-review refresh. Tests retain real pointer interactions; their
jsdom fixture declares only unsupported modal/fullscreen states false to avoid
the selector engine recursively delegating to itself. Desktop and SDK types pass.
The clean replay regenerates 142 commands and 446 schemas with `make typegen`;
its complete local verification and independent native review are running.

The panel preserves edited choices across dialog closure and native snapshot
changes. A changed review token requires explicit inspection of the latest
values; a failed refresh keeps text visible and blocks new state-changing
actions. Lost local action responses retain their exact request/action/replacement
UUIDs for safe receipt replay. Original intent export uses the native OS picker;
copying the edited resolution is separate. The shared native caller policy is
covered for local main/child views and rejected remote origins.


The cleaned source completed `make verify`: 766 frontend tests passed with one
existing platform skip, 1,229 Rust test executions passed with seven subprocess
helpers ignored, and lint/types/build/format/strict Clippy passed. Independent
native review found a truthful-control issue: paused conflict/attention states
had no resumable turn. Signed `8c3a763` restricts pause to remaining bounded
turns; its expanded recovery suite passes 18/18 with strict Clippy. Native review
otherwise found no remaining CAS, supersession, owned-lane, quarantine or schema
blocker. The canonical held-feed fix from RURU-116 is consumed before publication
and qualified separately; live-provider writes remain outside this foundation.
