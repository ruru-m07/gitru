# RURU-132 — review submission frontend

Pre-code contract, 8 October 2026. This supplements `RURU-132.md` and the
remote-collaboration architecture. Root owns `packages/collaboration-client/**`,
desktop collaboration components/tests, and this note. The native owner retains
Rust, schema 24, recovery, registration and generated IPC. No hand edits to
`packages/commands` are permitted. Shared-index commits are coordinated explicitly.

The SDK exposes local draft, recovery index, save, submit and submitted-history
queries through generated commands. Account, subject, epoch and authorization-view
fences apply to returned data and request contexts. Review draft keys are authored
local data; reset synchronously removes context and provider-derived anchors while
retaining summary/comment text. Submitted history is provider data and is cleared
on account retirement. Preview/receipt authority uses the R120 ephemeral SDK
authority generation; query errors or retirement remove submission authority.

The review composer opens from the Reviews section and mounts on first use.
Closing or collapsing it preserves unsaved text. Changes to provider head/context
must not remount the authored editor; instead they retire its consent and require
an explicit save against the current snapshot. CAS conflicts preserve local text
and offer an explicit latest-draft load. Approval can have an empty summary;
comment/request-changes require one. Native validation remains authoritative.

Inline comments originate in a currently displayed provider-validated file diff.
The renderer selects an opaque file key, exact file context/facet revision and
line/side range; it never supplies a trusted path or provider proof. Native save
resolves each selection. Local-only, binary, missing or stale artifacts cannot
offer a submit-capable anchor. Retired anchors retain authored text, require
explicit fresh selection or removal, and are never silently remapped.

Submission requires both background-delivery consent and acknowledgement that
a force-push can race the final check. One UUID is retained for a lost local
receipt; retry must use the exact original request under unchanged authority.
Queued, accepted, outcome-unknown and confirmed are distinct. There is no
optimistic approval badge or inference that an old reviewed commit approves a
new head. Accepted/confirmed receipt history is separate from cached Reviews
coverage and is bounded/pageable. A global local recovery index includes review
drafts whose account or subject is unavailable.

Qualification covers generated wire/account/key fences; held-read retirement and
authored preservation; local-only opening/saving; close/collapse preservation;
CAS conflicts; explicit consents and exact retry; changed head/runtime retirement;
provider-only anchor selection and stale anchor preservation; accepted versus
confirmed history; and disconnected paginated recovery. Run focused SDK/UI tests,
lint/types/build and the complete frontend suite after generated IPC stabilizes.
Native, packaged, live-provider and remote CI evidence remains separately recorded.

No frontend implementation or validation is claimed by this initial contract.

## Frontend implementation and qualification — 8 October 2026

The generated five-command SDK, lazy review composer, provider-diff selection,
separate submitted-receipt history and paginated authored recovery are implemented.
The composer preserves unsaved text across close/collapse and account epoch changes;
account/actor/subject identity still isolates editor ownership. Runtime, epoch or
account-state retirement removes inline authority and consent. Exact local-receipt
retry retains its UUID only under the captured authority; definitive InvalidInput
admission unlocks editing and asks for corrected lines or shorter text. Uncertain
IPC failures retain the original request. Opening/saving performs local reads/writes.

Independent review caught two boundaries before publication: a parent epoch key
remounted the editor, and initial hydration of an already errored cache could
restore retired paths. Both are corrected and exercised through the real parent
and an initially errored Query cache. SDK account/runtime reset retains authored
summary/comment bodies while synchronously clearing context, anchors and provider
receipt history. Body/file change notifications retire submission context before
refetch. Stale, local-only and binary diff artifacts offer no inline authoring
control. The renderer sends only file keys/context/line selections; native storage
resolves trusted paths. Submitted history distinguishes accepted from confirmed and
never represents whole provider coverage or current-head approval.

Local qualification on this frontend checkpoint:

- **904 tests passed, one existing skip, 104 files** in the complete frontend suite
  with four workers. The first run overlapped lint/types/build and native work:
  902 passed and two pre-existing UI tests hit timing limits. Those 44 tests passed
  unchanged in isolation; the complete bounded rerun passed without relaxed assertions.
- Focused **105 tests** passed: 62 SDK client, six generated review wire, 14 review
  authoring UI, and 23 file-panel controls (including four new provider-anchor cases).
- Full lint, workspace TypeScript, desktop/E2E types and an uncached desktop
  production build pass. The initial type check found the new test's plain error
  fixture did not satisfy Query's Error type; it now uses a typed synthetic Error.
- Generator review corrected a tagged ReviewDraftAnchor initially emitted as an
  enum and missing enum aliases through generator source plus normal `make typegen`.
  The combined graph has **170 commands**, with no manual generated-file edits.
- The query-error transition test waits for Query notification, and the runtime
  reset test drains its restart bridge before teardown; both preserve their
  immediate authority-refusal assertions.

Native worker/recovery, packaged application, authenticated live-provider and
remote CI evidence is separate and is not inferred from these frontend gates.
