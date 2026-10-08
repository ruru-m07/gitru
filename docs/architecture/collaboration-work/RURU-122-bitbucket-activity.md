# RURU-122 — bounded Bitbucket Cloud activity observations

Pre-code contract, 8 October 2026. Base: GitLab Activity PR #197 signed
`93d1a35e`, including its qualified terminal-truncation writer and RURU-102
backoff/fairness corrections. Root owns this independent managed Lexar worktree.
RURU-120/RURU-132 retain schema 23/24 ownership elsewhere; this slice needs no
schema, IPC or public DTO change.

## Primary provider contract

The public [Bitbucket Cloud PR activity API](https://developer.atlassian.com/cloud/bitbucket/rest/api-group-pullrequests/#api-repositories-workspace-repo-slug-pullrequests-pull-request-id-activity-get)
returns paginated comments, updates, approvals and change requests. Tasks and
attachments belong to an internal API and are not requested. The documented
update and approval examples contain timestamps/actors but no stable event ID.
An observation key must not be described as a provider event identity or used to
infer current PR/review state. No live provider credentials are needed or used.

## Identity and presentation contract

- Use the existing immutable repository UUID route with the empty-workspace UUID
  form, numeric PR ID, fixed `pagelen=50`, exact-origin/path/query validation and
  the existing no-redirect transport. Never follow payload hyperlinks. Only an
  active Bitbucket Cloud account, selected repository and exact native PR may
  dispatch. Provider issue/inbox semantics remain unsupported.
- Require one known event family per row. Validate the top-level `pull_request`
  and any nested `pullrequest` identity against the captured PR; validate any
  supplied destination repository UUID against the captured repository. Unknown
  families are skipped and remain partial; malformed known identities fail the
  whole page without changing saved content. No synthetic default IDs/dates.
- Comments retain the provider comment ID, independent own timestamps and
  explicit deleted tombstone. Render raw text only, with a 4 KiB body limit and
  typed omitted/oversized states. A tombstone clears saved body and actor rather
  than leaving an old private comment visible. Inline/reply text is historical
  Activity only; no review-thread, anchor or resolution authority is inferred.
- Updates, approvals and change requests are content-addressed observations.
  Version-one keys hash a fixed typed tuple of subject, family, normalized event
  time, native actor UUID and observed semantic payload (state/title/body and
  source/destination refs where provided). Exclude nickname/display-name/avatar,
  links and the mutable top-level PR summary. Reordered JSON and cosmetic actor
  changes cannot create new identities. Changed semantic records produce separate
  observations; byte-equivalent normalized observations may coalesce. This is
  explicitly not a complete count or stable-identity event stream.
- Author display is only the safe bounded nickname/display name. Store neither
  email nor avatar links. UTC nanosecond timestamps drive existing indexed local
  chronological pages. No source entry may set current workflow/head/review state.
- Always retain Uncertain SubjectHistory coverage. Partial distinguishes zero
  saved supported observations from complete empty history. No absence pruning,
  historical-to-current state propagation or 304/parent-validator authority.

## Bounded traversal and recovery

- One native detail page is one provider request. A cursor binds version, source,
  account/actor/epoch, repository UUID, subject/PR and exact next URL. Preserve
  independent local keyset pagination. Reject foreign/nonprogressing/repeated
  continuations and bound all cursor hashes/bytes. No ordering filter is invented.
- At most 20 pages of 50 provider rows per traversal, including skipped unknown
  records. Resume the committed cursor across the runtime's ten-page yield and
  cold restart. Validate a returned continuation even at the cap.
- The twentieth page with known continuation uses the existing native Truncated
  terminal evidence: Partial/known-more, no usable continuation, no immediate 21st
  request. A later ordinary/manual refresh starts a new bounded window and keeps
  previous observations and authored drafts.
- Keep successful and malformed/error response Retry-After observations through
  storage rejection and cold restart. Same-epoch denial and retired-epoch results
  cannot expose provider content or mutate local drafts. Clock/retry/account
  policies and all existing dispatch fences remain unchanged.

## Verification and delivery

Finite synthetic HTTP controls cover routes and no redirects/304, every family,
comment edits/tombstones, unknown/omitted/oversized content, normalized observation
identity, cosmetic variation, malformed/foreign payloads, duplicate observations,
strict next URLs/cursor binding, the twenty-page cap and preserved quota.
Real native runtime/SQLite controls cover ten-page yield/cold resume, terminal cap
refresh/no pruning, local ordering/paging, cached offline reads, unchanged draft,
authorization loss and error/cooldown before any resumed vault access.

The common UI adds short Bitbucket-specific partial-history coverage text and safe
labels; use existing coss components, local subscriptions and disclosure demand.
Run focused/full native tests, strict Clippy/fmt, frontend lint/types/tests/build
and normal typegen semantic verification as appropriate. Record source review,
local checks, exact-head remote CI and live/native-window evidence separately.
Publish a signed scoped draft stacked on PR #197; do not merge any PR.
