# RURU-129 — GitHub title/body desired-state edits

Status: first bounded implementation locally qualified for review, 8 October 2026.
Publication base: RURU-117 #180, including its qualified restart fixture correction. This is the first selected-operation slice of RURU-129; state changes,
labels, GitLab, and Bitbucket edits remain unavailable and keep the issue partial.

## Operation boundary

GitHub.com issues and pull requests accept durable title/body intent from a local
snapshot. Admission has no HTTP dependency. It binds the account, current actor
and authorization epoch/view, immutable native repository and subject identities,
known canonical Body metadata, workflow state and PR head to a versioned native
codec. The renderer supplies only selected text fields, command UUID, and an
opaque native review context. It cannot supply routes, provider payloads, guards,
execution context, evidence, or capability overrides.

Only changed fields are sent. Body edits preserve known null separately from an
omitted field; unknown/omitted/oversized bases cannot authorize editing. Initial
limits are 16 KiB UTF-8 body and 1 KiB/256 characters title. Complete native
preparation and evidence are bounded to the existing 64 KiB command-delivery cap.
No text is silently truncated. An unavailable snapshot or failed submission leaves
entered UI text intact. Cached read access does not prove token write scope;
provider authorization remains explicitly unverified until the mutation response.
Accounts continue to work independently of Gitru cloud.

## Provider semantics and safety

The documented issue and pull update endpoints accept title/body PATCH fields but
provide no documented conditional compare token for these edits. A GET followed
by PATCH is therefore **best effort**: a provider edit can race in the interval.
The product must disclose this limitation. It must not claim optimistic UI or
preflight comparison provides remote compare-and-swap.

Each provider turn obtains fresh native context inside the writer transaction,
then performs a bounded authenticated GET outside it. The response must match the
installation, repository ID, typed subject ID/number, workflow/source, and PR head.
Changed authored fields compare base/remote/desired; independent untouched remote
fields remain untouched. Overlapping edits and unknown guards become explicit
conflict, without dispatch. The writer revalidates the context before committing
an attempt and its execution base. A dispatch requires a durable attempt first.

The shared transport is owned by R130: fixed native route, same API origin,
redirects/retries disabled, bounded request/response, pinned API version and finite
timeout. R129 owns the title/body route, payload, identity and outcome codecs.
Quota/auth observations are independent of operation proof and retain the R115
failure semantics.

Confirmed means the desired fields were verified on the exact canonical resource;
it does not claim causal proof that this command produced them. A successful PATCH
must return validated canonical resource evidence. After transport ambiguity, a
fresh exact-resource GET may prove convergence. A different field value does not
prove non-delivery, and never causes automatic PATCH replay. Unknown outcomes retain
intent for R117 review. Restored quarantine can only reconcile; it never dispatches.

Canonical observations commit through R116's scoped Body finalizer, atomically
with command resolution and optimistic-effect retirement. Its timestamp, identity,
source and active authorization checks remain authoritative. Raw provider facts
and effective pending intent remain separate. Recovery replacement uses R117's
immutable supersession mechanism and native policy choices; no generic payload or
retry bypass is added.

## Native seams and ownership

R129 adds a default-empty `CommandDeliveryPolicy::prepare_context_in` hook. Both
preparation and reconciliation capture <=64 KiB of fresh native context inside the
claim transaction and carry it on the native-only request. A provider policy does
not retain a Store, so backup/restore runtime replacement cannot strand it on a
closed database. `validate_claim` rechecks current authoritative facts after HTTP.
Existing synthetic policies keep their empty context. No migration is required.

Native modules, codec/admission/storage/runtime, provider operation and tests are
owned by the R129 worktree. Root owns later desktop IPC, generated bindings, SDK
and UI integration after DTO coordination. R130 owns `providers/transport.rs` and
`providers/transport/mutations.rs`; consume its signed checkpoint rather than
creating another transport. No production credentials or live mutation validation.

## Qualification gates

- Offline admission, exact retries, stale snapshot/epoch rejection and immediate
  effective list/detail/search projection with preserved authored bytes.
- Fresh HTTP preflight: independent change preserved, overlap conflict, native
  identity/source/head drift prevents dispatch, arbitrary route rejected.
- Durable attempt-before-write and no automatic repeat after timeout/reset/restart.
- Canonical response finalization, GET convergence, absent desired state remains
  unknown, old provider observation cannot replace newer canonical base.
- Auth/quota response handling, bounded context/response/evidence and restore
  quarantine, owned shutdown and account cutover.
- Operation-specific R117 review/replacement; unsupported codecs/providers fail
  explicitly. Current migration/recovery tests remain valid without schema bump.
- Meaningful focused tests, strict Clippy, generated IPC via `make typegen` if
  commands are integrated, then `make verify`. Report remote CI and any real
  provider/platform checks separately; no live write claim from fixture tests.

## Sources and inference

[GitHub issue update](https://docs.github.com/en/rest/issues/issues#update-an-issue)
and [pull request update](https://docs.github.com/en/rest/pulls/pulls#update-a-pull-request)
document selected fields and token permissions. The lack of a documented edit CAS
contract is the reason for the explicit best-effort boundary; this is an inference
from the documented endpoint contract, not a claim that GitHub cannot implement
other internal concurrency mechanisms. Researched 7 October 2026. Existing adapter
version is `2026-03-10`.

## Native implementation checkpoint — 8 October 2026

The selected native operation is implemented as `github.edit_text`, payload version
1. The immutable codec writes fixed typed scalar fields in tag order, including
explicit best-effort acknowledgement, affected text, saved title/body/state/head,
native identities, source/time, and review context. Unchanged supplied fields are
rejected; the editor must omit them. A known null description remains a raw
provider fact; sending an empty string explicitly clears it. Only this empty-body
comparison treats null and empty as semantically equal. No provider bytes or
routes are accepted from the renderer.

`text_edit_snapshot` is local-only and declines unknown/oversized bases or existing
pending intent. `submit_text_edit` atomically admits the command and effective
projection, protects Body retention, and returns an immutable admission receipt.
Exact retries compare the original request and sealed bytes even after the cache
changes or process reopens; current epoch authorization still precedes retry.
The runtime owns accepted completion through shutdown and signals the existing
revision/subscription path.

The shared native-context seam is signed as `ed8f66d`; the transport is the reviewed
R130 checkpoint `21b2ae5` (cherry-picked here as `9549dcb`). Both preparation and
reconciliation capture current native context inside the writer transaction;
64 KiB overflow rolls back generation, budget and revision together. Fresh GET
observations cannot predate the captured provider timestamp. A converged
preflight can commit canonical state and confirmation without recording a mutation
attempt. An overlapping edit, workflow or head change instead commits conflict
and fresh canonical evidence while retaining authored intent.

After an actual attempt or restored quarantine, read-only convergence verifies the
exact resource and every authored field using the **current** captured lease.
Unrelated newer head/workflow facts do not prohibit observing the desired text.
The original authored guards remain byte-identical. A different desired field or
resource never proves non-delivery. No error, arbitrary HTTP status, differing
read, or authentication failure produces a generic safe-retry receipt. Successful
PATCH and read convergence both go through the scoped canonical finalizer before
retiring effective intent. Confirmation describes observed convergence, not proof
of command causality.

Operation-specific R117 review exposes only authored text fields as editable.
Replacement rebuilds an acknowledged native operation from a fresh saved base;
untouched fields remain omitted. Existing actor/epoch/quarantine, supersession,
history, attempts and attention limits remain authoritative. No schema change is
introduced; current backup/restore preserves immutable payloads and quarantines
pending commands even after account reauthorization.

Local qualification: **17/17 focused native tests passed** in
`/tmp/gitru-r129-native-text-tests4.log`, including finite loopback HTTP, cold reopen,
transaction bounds, conflict/convergence, current-head reconciliation, canonical
empty-body behavior, auth/quota, replacement, and actual synthetic backup/restore.
Strict all-target collaboration Clippy passed in `/tmp/gitru-r129-native-clippy2.log`.
Broader regression is being finalized; desktop IPC/editor work
is concurrent and has its own evidence. No live provider mutation, credential
inspection, remote CI, or packaged platform claim follows from these fixtures.


## Desktop integration and publication qualification

The native-only snapshot and admission operations are registered behind the same
trusted desktop caller policy, generated with `make typegen` (144 commands,
454 schemas), and exposed through account-bound SDK reads with revision hints.
The cached detail editor sends only changed fields, requires explicit best-effort
consent, preserves text on stale context, and reuses the exact command UUID after
a lost local receipt. Its disclosure covers concurrent provider edits in both
directions. Local receipt copy does not claim provider confirmation.

Full local `make verify` at signed integrated source `17db925` passed **774 frontend
tests**, one platform skip, **1,254 Rust test executions**, seven helper ignores,
lint/types/desktop build, formatting and strict workspace Clippy. Final source
`f01b0392` adds separately qualified native entry bounds, common provider quota/auth
transport preservation, the inherited graceful restart fixture correction, and
consent copy. Final delta: **20 text-operation tests**, **14 transport tests**,
**21 feature-enabled native harness tests**, strict all-target collaboration Clippy
with test-harness, full formatting, and **4 UI tests** pass. Independent native and
frontend reviews found no remaining blocker within this bounded scope. Integration
`5e8098d1` only joins the already-identical RURU-117 fixture correction ancestry.

Logs: `/tmp/gitru-r129-verify.log`, `/tmp/gitru-r129-final-delta.log`,
`/tmp/gitru-r129-final-clippy.log`. New PR remote CI remains separate. No production
provider requests, personal credentials, platform vault or packaged-UI validation
were used. The issue stays In Progress for state/labels and other-provider edits;
this title/body slice is reviewable. No merge is authorized.

## Numeric repository route follow-up — 8 October 2026

The GitHub text-edit preflight and PATCH now address the repository by its validated
positive numeric provider ID (`/repositories/{id}/issues|pulls/{number}`), so a
namespace transfer or reuse cannot redirect a queued write through the saved
owner/name. The stored full name remains a validated response identity and URL fact;
there is no fallback from the numeric request route to the mutable namespace route.
Existing GitHub read adapters already exercise the corresponding numeric nested
aliases, and the focused synthetic text-edit suite passes **20/20** with the numeric
GET/PATCH route. No authenticated live GitHub write was performed, so compatibility
of the numeric mutation alias in a live provider environment remains unqualified.

## Shared clock and Windows fixture qualification — 8 October 2026

The reviewed clock follow-up retains full provider cooldowns, checks discovery
before vault/HTTP admission, and seeds durable waits into the monotonic budget
on the first cold direct delivery check. A deterministic regression first failed
when a UTC jump erased a still-active 47-hour wait; the corrected direct check
keeps the remaining bound and leaves the peer account eligible.

Production checkpoint `36e6f153` passes 21 clock controls and the complete native
library suite (567 passed, four subprocess helpers ignored), strict all-target
collaboration Clippy and formatting. The numeric route delta separately passes
20 text-edit operation cases. Final test-only checkpoint `a6dd0572` incorporates
the Windows credential crash startup-ready handshake from RURU-128: 14 parent
crash cases pass, with one subprocess helper ignored, and strict Clippy/fmt pass.
The setup phase has its own bounded allowance; the existing credential-boundary
watchdog and authorization assertions are unchanged. These are qualified deltas
to the earlier full workspace run, not a claim that it ran on the new head.

The PR remains the title/body slice. State close/reopen is being developed in a
separate bounded follow-up; labels and other provider writes remain outstanding.
Remote CI restarts on the new published head and live numeric mutation, vault and
packaged-window qualification remain separate. No merge was performed.

## Windows disk-backed sync fixture follow-up

Test-only source `fb7418c5` incorporates the qualified RURU-123 cursor-resume
fixture repair. The capped-bootstrap test subscribes before startup and awaits
its precise committed cursor/run/coverage milestones with a bounded 30-second
watchdog; it retains the original request-count, continuation and completeness
assertions. All nine runtime-sync tests and strict all-target collaboration
Clippy/fmt pass. This is a focused delta to the recorded full workspace baseline;
fresh remote Windows CI remains pending.
