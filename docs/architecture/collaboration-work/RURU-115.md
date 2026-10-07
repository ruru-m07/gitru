# RURU-115 — Durable delivery and ambiguous-outcome recovery

Status: bounded implementation contract, 7 October 2026 21:10 UTC. No delivery
implementation or remote write is claimed at this checkpoint.

## Baseline and ownership

This managed external-volume worktree starts at signed RURU-106
`e3e32bcd3bbed5e00f676e54c91fa22eb5174a1b`, including final RURU-119
`218c61024cf5c9c557f4642a8a429742e10bc571`, RURU-114 admission/schema 0013,
RURU-95 credential cutover and RURU-106 recovery/schema 0015. The branch is
`ruru/ruru-115-durable-delivery`. Live Linear RURU-115 is Backlog with RURU-95,
RURU-106 and RURU-114 prerequisites; no duplicate delivery PR exists. Those
prerequisites remain review stacks, not merged releases. RURU-106 final combined
and packaged qualification belongs to its coordinator.

This slice owns native delivery policy/state, migration 0016, storage transitions,
provider-registry integration, bounded runtime scheduling and synthetic tests.
It leaves real provider mutation codecs, renderer submission controls, optimistic
projection and conflict-resolution UI to their dependent issues. Production
GitHub/GitLab/Bitbucket adapters register no mutation policy here. It never reads
personal credentials or changes Gitru-cloud-independent account behavior.

## Native operation-policy seam

Register reviewed, crate-private policies by exact provider instance, operation
kind and payload version in the existing ProviderRegistry. Unknown versions or
instances have no dispatch implementation. Policies receive the stored immutable
command bytes and typed account context, never renderer-authored URLs or a raw
HTTP escape hatch. A policy is responsible for:

- interpreting its codec and validating required local guards and separately
  captured execution-base evidence under the same writer snapshot as claim;
- any operation-specific, read-only preparation or reconciliation requests;
- one dispatch call and an explicit result classification, including whether
  evidence proves completion, asynchronous acceptance, rejection, conflict,
  non-delivery or an unresolved outcome;
- validating the provenance and bounds of evidence accepted for its operation.

The worker owns account/instance matching, credential selection, shared quota,
durable state, scheduling, retry limits, publication and lifecycle ownership.
Policy validation inside a transaction performs no HTTP/vault access. Remote
preparation is followed by fresh local claim validation. A callback cannot
rewrite the canonical submitted envelope, payload, guards, dependency hashes,
admission receipt or original authorization epoch.

## Durable transition protocol

Migration 0016 adds command scheduling metadata and per-attempt execution context;
it does not alter immutable migrations or overload cached provider projections.
All records are account-scoped and foreign-keyed to existing command/attempt
identity. Execution context and recorded evidence are bounded separately from
immutable submitted intent. Transitions produce a `commands` change revision in
the same transaction as their durable result.

For queued or proven-safe retry intent:

1. Select a bounded candidate with a registered policy. Unknown operations stay
   preserved and undispatched. Recheck exact active account epoch, instance,
   applicable quota, age/attempt limits, dependency state and target ordering.
2. Every predecessor must be confirmed with a proven result. Accepted or unknown
   predecessors block. Rejected/cancelled/superseded predecessors require conflict
   handling; they do not authorize a guessed execution base. A still-pending
   earlier command for the same account/target also blocks later delivery.
3. Obtain the native credential and any policy preparation outside SQLite.
   Validate the final execution base and fences under the writer transaction.
4. Insert the next ordered `delivery_attempts` row with outcome `started`, persist
   execution context and move the command to `sending`, committing before calling
   the provider. A claim failure performs no remote mutation.
5. Invoke the adapter once with a finite timeout, under native-owned completion.
   Local credential cutover and dispatch admission share the existing lifecycle
   gate; cancellation cannot silently abandon result recording after a claim.
6. Commit operation evidence, attempt outcome, command state and next action
   atomically. A storage failure after dispatch leaves durable `sending`, which
   becomes `outcome_unknown` on the next recovery pass. Never infer failure from
   the absence of a saved response.

The state model remains queued, sending, retry_wait, accepted, confirmed,
outcome_unknown, conflict, rejected, cancelled and superseded. HTTP 202 or an
operation receipt alone is accepted, never confirmed. Timeout, task/panic loss,
transport error or process death after claim produces an unresolved outcome.
Successful reconciliation requires operation-specific independent evidence.
Matching text, wall time, a generic 2xx, or absent data on a partial list is not
proof of completion or non-delivery.

Only a policy's explicit proven-safe result can schedule another dispatch under
the original immutable intent. Generic network/status retries cannot do so.
Non-idempotent unknown outcomes stay unknown until positive outcome evidence,
strong non-delivery proof, or downstream manual resolution. A changed intent
always needs a new command UUID; there is no generic retry/reset-state API.

## Restored commands and immutable quarantine

The schema-0015 quarantine relation and SQLite attempt guard remain immutable.
No worker action removes quarantine or admits a new attempt for a quarantined
command. A restored queued command with zero attempts is equally quarantined:
its backup cannot know that a remote create happened later.

Read-only reconciliation may use a newly authenticated credential only for the
same stored actor/account/instance and with a freshly captured current epoch.
Its commit revalidates that epoch and the command/hash/recovery-generation
snapshot. Strong positive or final-rejection evidence may settle the command
while retaining every quarantine row and original attempt/receipt. Even strong
non-delivery evidence does not release restored intent for automatic dispatch
in this slice; an explicit operation-specific superseding command belongs to
RURU-117 and its dependent write flow. Unsupported codecs remain opaque.

## Scheduling, quota and shutdown bounds

Use the existing process runtime and provider dispatch ownership. A background
turn performs bounded delivery work interleaved with existing read work; no
second unbounded worker or frontend scheduler is introduced. One dispatch lane
provides a conservative initial concurrency bound. Candidate scans, in-memory
deadlines, evidence payloads, attempts and reconciliation probes have explicit
limits. No retry wait holds an active HTTP slot or writer lock.

Reuse provider account-wide cooldowns and native clock helpers. Persist future
action deadlines and bounded failure counts; retain monotonic barriers while
the process is alive and validate/clamp restart wall-clock waits. Offline/rate
conditions before dispatch leave durable intent queued; ambiguous failures
after dispatch require the policy-specific recovery path. A noisy or blocked
account must not monopolize all candidate capacity.

RURU-106 shutdown stops admission and waits for already-owned operations.
Delivery integrates that ownership so a detached caller or preview transition
cannot close SQLite beneath a pending result transaction. Provider calls and
reconciliation are finite; exact account/epoch and policy identity are checked
before credential use and dispatch. Late evidence never grants a replacement
account the old command's write authority.

## Recovery and migration compatibility

Raise the explicit recovery schema policy only with corresponding typed
validation and preservation of the new durable scheduling/context records.
Historical v1–v15 backups migrate in private staging; source files remain
unchanged. Attempt execution evidence remains in both the installed ledger and
protected incoming recovery snapshot. Restore adds quarantine and resets only
ephemeral scheduling authority; it never erases attempts or makes old intent
newly dispatchable. Freeze schema-0015 bytes and add migration interruption,
disk-full and rollback/retry controls for 0016.

## Required evidence and delivery

Use deterministic in-memory credentials and a synthetic operation adapter with
an independent durable remote-effect ledger. Cover successful dispatch, async
acceptance and later confirmation/rejection, conflict, proven-safe retry,
ambiguous non-idempotent creation, unsupported policy, dependency/target order,
cross-account isolation, epoch change before claim/after preparation, quota and
offline bounds, delayed response, requester cancellation and owned shutdown.

Crash synthetic child processes before claim commit, after claim before HTTP,
after remote side effect before result commit, and after result commit. A cold
restart must preserve evidence and never duplicate uncertain creates. Exercise
backup queued intent → successful independent remote create → restore → same
actor reauthentication → strong reconciliation, retaining quarantine and proving
the remote create count stays one. Test actual SQLite faults and frozen migration
bytes. Keep cache retention protection effective for every pending/conflict/
unknown command and required predecessor.

Run focused storage/runtime/recovery/migration checks, strict Clippy and full
`make verify` before publication. Generate IPC only if a public signature truly
changes; no IPC is planned. Update this note, the architecture progress record
and Linear with actual evidence. Open and attach one draft PR stacked on the
existing RURU-106 branch; do not merge. Local synthetic tests, remote CI and
live provider/platform qualification remain separate evidence.

## Schema checkpoint — 8 October implementation

Migration 0016 is frozen before worker integration. `command_delivery` provides
one monotonic generation and bounded probe/deadline metadata per admitted
command. `delivery_attempt_context` retains immutable bounded execution bases;
`delivery_resolutions` retains immutable purpose-to-evidence references. Neither
relation can rewrite the original admission ledger. Unsupported operation bytes
remain opaque. Recovery policy explicitly accepts schema 16, validates coverage,
instance affinity, resolution generations and timestamps, preserves context and
resolution bytes, advances generations, and clears only old scheduling authority.

Local evidence: five migration tests passed, covering frozen v1–v15 inputs,
source-preserving restores, rollback, and actual SQLite interrupt/full-disk
failures from v14 and v15. Eight current recovery cases passed, including new
execution-context/resolution preservation with reset deadlines and retained
quarantine, plus the existing queued-before-backup ambiguity and immutable
attempt guard. Worker code remains in progress; no provider dispatch or remote CI
qualification is claimed by this schema checkpoint.
