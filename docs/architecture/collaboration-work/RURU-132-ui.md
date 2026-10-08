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
