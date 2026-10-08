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
