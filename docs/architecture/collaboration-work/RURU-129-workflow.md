# RURU-129 — GitHub close/reopen workflow intent

Status: bounded implementation contract, 8 October 2026, before source changes.

This follow-up extends the title/body slice with GitHub.com issue and PR close or
reopen actions. Labels and other provider mutations remain separate. The managed
external worktree starts on the schema-20 comment/recovery stack and consumes
its final qualified parent before publication. No merge action is implemented or
performed. The native lane owns Rust models/admission/provider/recovery/tests;
the frontend lane owns generated IPC, SDK and selected-resource controls. Root
reviews, records evidence and publishes a scoped draft PR.

Use a separate typed operation and native local snapshot/context. Renderer input
contains only desired open/closed state, exact account/epoch/view/review token,
stable command UUID and explicit best-effort consent. Never accept renderer URLs
or arbitrary operation bytes. Existing immutable command/effect/delivery/recovery
storage should suffice; do not invent a second outbox or mutate admitted payloads.
Only the State effect is authored. Title, body, labels, local disposition and
private/comment drafts stay independent. Effective local lists, detail state,
counts and search must show pending workflow intent coherently.

Admission and current writer-owned preflight capture current native repository,
resource, workflow and PR head facts. Merged/unknown workflow or missing access
refuses an unsafe reopen. Use fixed numeric repository routes with no mutable
name fallback, disabled redirect/internal retry and bounded response size/time.
The final pre-dispatch claim rechecks the current context. Preflight is not remote
CAS: explicitly disclose that provider changes arriving afterward can race the
request. The ordinary issue/PR update endpoint supports state=open/closed;
no merge endpoint, base-branch change or draft conversion belongs in this slice.

Official endpoint references checked 8 October 2026:
- https://docs.github.com/en/rest/issues/issues#update-an-issue
- https://docs.github.com/en/rest/pulls/pulls#update-a-pull-request
Numeric mutation alias compatibility remains a separate live-provider gate,
matching the title/body adapter. No personal credential inspection or live
provider mutation is permitted for unattended validation.

A committed dispatch attempt precedes PATCH. Validate canonical native identity
and exact desired workflow state before confirmation; apply canonical observation
and retire the State effect atomically. Independently retain quota/auth facts even
when canonical publication loses its fence. An authenticated observation already
showing desired state can prove convergence, not causality. Lost responses and
uncertain delivery use bounded read-only reconciliation; never blindly resend.
Preserve original intent across restart, access loss and restore quarantine.
Recovery may expose safe explicit superseding intent only under the existing
writer/policy guards; it cannot bypass an accepted/unknown command.

UI controls read local native availability, explain queued versus confirmed
state, require best-effort acknowledgement, and preserve exact request identity
when a local receipt is lost. Account/resource changes cannot receive another
account's late receipt. Existing draft editors remain mounted and untouched.

Qualification includes native admission rollback/dedup, current permission/view/
workflow/head drift, merged PR refusal, already-desired convergence, exact PATCH
state without text changes, unknown/no-second-PATCH recovery, cooldown and auth,
cold restart/restore, coherent effective filtered lists/counts and held-feed
canonical fencing. Add meaningful SDK/UI tests for consent, exact retry, disabled
unsupported actions, changed context and retained sibling text. Generate IPC
with make typegen. Record full local verification separately from remote CI and
live provider/platform validation. This workflow slice does not complete label
editing or change the prohibition on merging user PRs.

## Implemented checkpoint — 8 October 2026

The separate `github.workflow_state` codec now admits only typed open/closed
intent with explicit best-effort consent. Its immutable base contains workflow,
PR head, native identity/source and provider timestamp, excluding description and
title. Omitted/oversized descriptions and missing title evidence do not prevent a
valid State action. Provider observations preserve those distinctions, and the
canonical transaction only replaces independently proven fields. A pending text
edit does not masquerade as or block a pending State change; generic ordered
command delivery and effective replay preserve both fields.

Native admission seals generation/view context and receipt atomically; exact UUID
retries reuse the original bytes after restart. Fresh numeric-route preflight
requires state and PR head facts plus explicit merged evidence, refuses changed
heads and merged workflows, and recognizes desired-state convergence without a
PATCH. A committed attempt precedes the exact state-only PATCH. Uncertain results
remain reconciliation-only through restart and restore quarantine. Completion
rechecks the account/actor/epoch/view and captured Body revision/run before atomic
canonical publication, State effect retirement and feed revision invalidation.
Recovery exposes only State editing under the shared supersession guards.

Local qualification: 18 focused native workflow cases passed on the final source,
strict collaboration all-target Clippy and workspace rustfmt checks passed. Cases
cover issue reopen and PR close, state-only wire body, independent text intent,
merged/unknown/missing-merge facts, identity and head drift, description omission/
oversize, held completion and equal-time feed races, filtered counts, transaction
rollback, auth/quota facts, exact restart retry, unknown no-second-PATCH and restore
quarantine. Independent review is clear after fixing pending reason precedence,
State-only pending filtering and description-independent authority. Frontend
owner qualified SDK206 and desktop589/one platform skip, types and generated151
commands; its exact-receipt retry follow-up also passed five UI cases.

Full workspace and remote CI are publication-owner gates. The numeric mutation
alias and live authenticated provider behavior remain unqualified; no personal
credential or provider write was used. This slice adds no merge action, issue
state-reason selection, labels, or another provider's mutation codec.
