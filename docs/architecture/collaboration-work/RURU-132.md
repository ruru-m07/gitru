# RURU-132 — submit pull-request reviews at the inspected head

Status: bounded pre-code contract, 8 October 2026.

This GitHub.com-first slice submits one final pull-request review with an
optional summary and bounded inline comments. It starts from signed guarded-merge
head `435599c7c45cd884ac3a4edb165549b96f5af759`, which includes the locally
qualified RURU-53 integration read stack and durable command protocol. Live
RURU-132 is Backlog and has no existing implementation or pull request. RURU-120
owns schema 23; this slice reserves schema 24 and must merge RURU-120's signed,
qualified migration and recovery checkpoint before registering or testing it.

GitLab, Bitbucket, GitHub Enterprise, provider-side pending review sessions,
replying to existing threads, resolving threads, suggested changes, reviewer
requests, anchor remapping and editing or deleting submitted reviews are outside
this slice. Accounts remain independent of Gitru cloud. Tests use synthetic HTTP,
SQLite and cached provider artifacts; no personal credentials or live provider
write is permitted.

## Provider contract and product boundary

GitHub's create-review endpoint accepts one optional `commit_id`, a final event
(`COMMENT`, `APPROVE` or `REQUEST_CHANGES`), an optional summary body and inline
comments. A `COMMENT` or `REQUEST_CHANGES` review requires a nonempty summary.
Inline comments use provider path plus `line`/`side` and optional
`start_line`/`start_side`; Gitru does not use the closing-down legacy `position`
field. Omitting the event creates a provider-side pending review, so this slice
always sends an explicit final event.

The endpoint documents no idempotency key or expected-head compare-and-swap. Gitru
therefore binds the request to the exact inspected commit and rejects stale local
authority before dispatch, but discloses that a force-push can still race between
the final preflight and the POST. The body includes the exact `commit_id`. The
resulting review's returned `commit_id` remains distinct from whatever head is
observed later; an approval of an older commit never authorizes a newer head.

Use only numeric GitHub.com routes rooted at the validated repository provider ID:

- `GET /repositories/{repository_id}/pulls/{number}` for a fresh identity,
  open/nonmerged state and exact base/head preflight;
- `POST /repositories/{repository_id}/pulls/{number}/reviews` exactly once after
  the durable attempt record;
- `GET /repositories/{repository_id}/pulls/{number}/reviews/{review_id}` and
  `/comments?per_page=100` only when a returned review ID permits exact readback.

Never fall back to mutable owner/name routes or follow redirects. Recheck provider
quota after every response. A cooldown or retry barrier stops the remaining HTTP
chain. Read support for the cached Reviews facet does not imply write support;
submission has its own typed availability. The preflight proves current
authenticated read access and exact target identity; GitHub's POST response is
the authoritative write-permission result.

Primary contracts checked on 8 October 2026:

- [GitHub pull-request reviews](https://docs.github.com/en/rest/pulls/reviews?apiVersion=2026-03-10)
- [GitHub review comments](https://docs.github.com/en/rest/pulls/comments?apiVersion=2026-03-10)

The documented API states that creation triggers notifications and can receive
secondary rate limiting. Pull requests write permission is required. A successful
create returns 200; 403 and 422 remain explicit provider results. The absence of a
documented compare-and-swap or idempotency token is an inference from the public
contract, not a claim about GitHub's internal implementation.

## Draft, context and anchor model

Review drafts are independent of private notes and conversation-comment drafts.
One account/subject draft contains:

- `ReviewSubmissionEvent`: `Comment`, `Approve` or `RequestChanges`;
- a bounded summary body;
- zero to 25 ordered inline `ReviewDraftComment` values, each with an immutable
  local UUID, bounded authored body and one native-captured GitHub line anchor;
- a positive CAS generation and an optional current submission status.

The summary and inline comment bodies are authored data and survive disconnect,
cache reset, missing subjects and recovery export. Provider-derived path, range,
repository, commit, URL and review identity never become authored data. They are
stored in a separately purgeable authority record and disappear synchronously
from query caches on account clear or local-view reset. A disconnected recovery
view retains the authored text but cannot submit or navigate to provider content.

`ReviewSubmissionContext` binds account, subject, authorization epoch/view, the
exact `ReviewContext` (base/head/source and target repository IDs plus Body
metadata revision), and a native review token. It is available only for an active
GitHub.com account, selected authorized pull request, open/nonmerged target and
known exact review context. Provider permissions remain unknown until fresh
submission; the fresh preflight proves target access and identity, not write
permission.

The renderer never supplies a trusted path, provider URL, repository ID, commit
OID, diff text or provider proof. It selects an opaque `file_key`, a requested
line/side and optional start range from the currently displayed exact file
artifact. Native save resolves that key through the current
`PullFileMembershipReceipt`, requiring the same account/epoch/view, subject,
`PullFileContext`, file generation and facet revision. Only a provider-origin
GitHub artifact with `PullFileArtifactValidation::Provider`, text content and the
same exact review head can authorize an inline anchor. A local-Git artifact alone
cannot create provider anchor authority.

Rust parses the bounded provider patch and proves that the selected side and every
line in the range occur in one valid hunk. Added and deleted files require their
present side, and a file whose old and new paths differ is not selectable in this
slice. Renamed, copied and unknown change kinds fail closed even when stored path
text happens to match. Mixed-side ranges and missing, omitted, oversized, binary
or local-only artifacts also fail closed. The resolved path and exact modern
GitHub fields are stored in the purgeable authority row; the renderer cannot
change them during submission.

This first slice never silently remaps an anchor after a file or head change.
Changing Body context, file generation, file facet revision, path identity,
provider artifact validation or requested head makes the draft stale. Authored
text remains available; the user may remove the inline comment or select a fresh
anchor explicitly.

## Proposed public DTO and IPC contract

The source types live in `crates/collaboration/src/review_submission.rs` and are
exported through generated commands. Exact spelling can change only before the
first source checkpoint; after that, typegen and wire fixtures freeze it.

- `ReviewDraftKey { account_id, subject_id }`
- `ReviewSubmissionEvent { comment, approve, request_changes }`
- `ReviewDraftAnchorSelection { file_facet_revision, context, file_key,
  start_line, line, start_side, side }`
- `ReviewDraftCommentInput { comment_id, body, anchor }`
- `ReviewSubmissionContext { account_id, subject_id, authorization_epoch,
  authorization_view, review_context, review_token }`
- `ReviewSubmissionAvailability { available, unavailable }`
- `ReviewSubmissionReason { unsupported_provider, account_unavailable,
  missing_target, stale_context, missing_provider_diff, invalid_anchor,
  empty_required_body, pending_submission, already_submitted }`
- `ReviewSubmissionStatus { command_id, draft_generation, state, attempt_count,
  quarantined, attention }`
- `ReviewDraftSnapshot { key, event, body, comments, generation, context,
  availability, reason, submission, revision, authorization_view }`
- `ReviewDraftQuery/Page/Summary` for bounded authored-draft recovery after a
  subject or account is no longer available; summaries contain no provider path,
  commit or URL authority.
- `SaveReviewDraftRequest { key, authorization_epoch, authorization_view,
  expected_generation, event, body, comments }`
- `SubmitReviewRequest { context, draft_generation, command_id,
  accept_background_delivery, accept_best_effort_race }`
- `ReviewSubmissionReceipt { account_id, command_id, admitted_revision,
  duplicate }`
- `SubmittedReviewQuery/Page/Receipt` for bounded validated local history, separate
  from whole Reviews-facet coverage.

IPC commands are local SQLite operations except the scheduler-owned provider
worker:

- `collaboration_review_draft(key) -> ReviewDraftSnapshot`
- `collaboration_review_drafts(query) -> ReviewDraftPage`
- `collaboration_save_review_draft(request) -> ReviewDraftSnapshot`
- `collaboration_submit_review(request) -> ReviewSubmissionReceipt`
- `collaboration_submitted_reviews(query) -> SubmittedReviewPage`

Opening or editing the composer performs no HTTP. Explicit submit durably admits
one command and requires both background-delivery and best-effort-race consent.
The current draft generation cannot be submitted twice under another UUID, and a
pending/accepted/unknown submission blocks a new send even if the user edits the
next draft generation. Retrying after a lost local IPC receipt must use the exact
original command UUID and request; native duplicate admission decides the result.

Bounds: summary and each inline body at most 16 KiB, 25 inline comments, 128 KiB
combined authored UTF-8, UUIDs in canonical hyphenated form, canonical lowercase
SHA-1/SHA-256 OIDs, positive decimal provider IDs and existing path/input bounds.
Control/NUL bytes, duplicate comment UUIDs, duplicate anchors and invalid ranges
are rejected before account or SQLite lookup.

## Durable command and ambiguity protocol

The operation is `github.submit_review`, payload version 1. The immutable payload
binds the exact draft generation/content hash, final event, resolved native
anchors, inspected `ReviewContext`, repository/subject identities, actor,
authorization view and both consent flags. `IntentField::ReviewSubmission` is a
separate operation field; it does not turn cached review reads into write
capability and it does not edit provider-side pending reviews.

Preparation performs a fresh numeric PR GET and verifies the active actor,
repository/native subject identity, open/nonmerged state and exact target/source
repositories plus base and head OIDs. Account/epoch/view, current Body revision,
command payload, draft generation and every retained native anchor are checked
again from local authority under the writer immediately before claim. Quota or
access loss before POST prevents the provider mutation, while the durable local
attempt remains available for conservative recovery.

Persist the final attempt before the single POST. Once the POST may have reached
GitHub, this command never automatically posts again. A lost connection, timeout,
malformed 200, unexpected response or response without a trustworthy review ID is
outcome-unknown. It is never reconciled by matching actor, event, body, time or
nearby review-list entries. Recovery may pause/cancel/export according to the
existing attempted-command rules, but cannot replace or replay it.

A bounded valid 200 must prove a positive review ID, exact pull identity, exact
current actor, event/state, exact summary body, submitted timestamp and the
requested `commit_id`. This yields immutable accepted evidence. A summary-only
review can then confirm from exact review-ID readback. A review with inline
comments confirms only when the exact review-ID comments endpoint returns one
terminal bounded page whose comment IDs, bodies, paths, line/side ranges, review
ID and commit anchors match every requested comment with no extras or duplicates.
A continuation, omitted anchor, mismatch or cooldown preserves accepted evidence
and defers read-only reconciliation; it never repeats the POST.

The append-only delivery ledger records a strict create response and its later
terminal readback as two distinct frozen proof kinds. `github.review_accepted`,
version 1, records the first strong 200 response with an exact native review ID;
`github.review_submitted`, version 1, records exact-ID review and terminal inline
comment readback. Neither codec serializes the evolving generic `RemoteItem`.
An immutable resolution row is inserted only by the accepted proof. A separate
immutable confirmation row links the later proof ordinal to the same account,
command, draft generation, provider review ID and inspected context.
Confirmation is never inferred from the accepted mapping. Separate proof kinds
avoid making a legitimate accepted-to-confirmed transition collide with the
provider-ID uniqueness fence on append-only evidence. Strong evidence links one
provider review ID to exactly one account/command/draft generation.
The submitted-review history exposes confirmed or accepted evidence under current
authorization without claiming whole review-list coverage. Provider cache may
later observe the same review independently; old or historical ReviewV1 rows stay
bound to their actual `reviewed_commit_oid` and never approve the current head by
context alone.

## Schema 24 and recovery

RURU-120 exclusively owns schema 23. RURU-132 must merge its signed qualified
migration/recovery head, retain its exact frozen schema-22 fixture and checksum,
and add schema 24 afterward. Proposed schema-24 ownership:

- `review_drafts`: account/subject primary key, authored event/summary/generation;
- `review_draft_comments`: ordered authored comment UUID/body rows keyed to draft;
- `review_draft_authority`: purgeable exact context and native anchor JSON keyed
  to each draft/comment generation, current account epoch and cache identity;
- `review_submissions`: account/subject/generation primary key, unique
  account/command, immutable content hash and exact command-envelope linkage;
- `review_resolutions`: immutable account/command/provider-review mapping with
  accepted-proof linkage and requested/observed commit identity;
- `review_confirmations`: immutable terminal-readback proof linkage to exactly
  the same resolution and provider review ID;
- separate expression indexes enforcing unique account plus provider review ID
  for accepted and submitted version 1 evidence.

Authored tables have no cascading provider-cache foreign key. Authority rows do.
Update/delete/identity triggers protect admitted generations and immutable
receipt links. Recovery policy advances from 23 to 24, freezes schema-23 SQL and
checksum, inventories every new table/index/trigger, validates hashes, bounds,
anchor/event invariants and converse proof/resolution linkage, and preserves both
backup and target on corruption. Restored commands remain quarantined and cannot
gain new dispatch authority. Previously frozen command/effect/proof codecs do not
change.

## Frontend behavior

The coss-based composer lives with the cached Reviews panel. It mounts lazily on
first open, then preserves unsaved authored text across collapse. Cached review
and file panels remain independently pageable. The event selector explains that
comment/request-changes need a summary. Adding an inline comment starts from an
exact provider diff line; stale or local-only artifacts show a useful reason and
cannot be selected.

The submit control shows the inspected head and requires explicit consent that a
force-push can race after the final check. Queued, accepted, unknown, conflict and
confirmed states use typed native evidence. Accepted never reads as confirmed,
and no optimistic approval badge is applied. Exact UUID retry remains available
for a lost local receipt under the unchanged account/epoch boundary while new
intent stays disabled. Query failure retains typed draft text but removes fresh
submission authority. Account reset synchronously redacts context, anchors,
provider IDs and navigation while retaining authored summary/comment bodies.

`Submitted from Gitru` history is evidence-derived and separate from the provider
Reviews list. It does not imply full list coverage, requested-reviewer state,
branch protection satisfaction or current-head approval.

## Implementation checkpoint — 8 October 2026

The bounded GitHub.com slice is implemented through SQLite, the delivery worker,
generated IPC/SDK and the desktop composer. Native finalization keeps the first
strong review-ID response and the later exact-ID confirmation as separate
append-only proofs. Recovery now verifies the reverse terminal-state links, one
native create attempt, accepted-before-confirmed evidence and delivery order,
and the exact ordered authored comment UUID/body sequence for an unchanged draft
generation. Wall-clock timestamps remain canonical evidence fields but are not
used as a causal clock.

Local focused evidence at this checkpoint:

- 34 review-submission provider, codec, anchor, storage and synthetic-worker
  controls pass. The worker case performs one POST, exact review-ID readback and
  a terminal two-comment readback, then restores a valid schema-24 backup.
- Eleven corrupted backup variants are refused, including missing proof/mapping
  rows, reversed proof/delivery order, a second create attempt and changed,
  reordered or missing authored inline rows.
- Six storage/worker cases cover lost or malformed responses with zero replay,
  durable cooldown across cold reopen, quarantine after restore and trusted
  renamed/copied/unknown file-kind refusal.
- The two-file bounded native HTTP fixture correction from signed checkpoint
  `ad9a46c1d08ab25b0aacba5a02696754e5470a9e` is included because the inherited
  one-read fixture was not safe under fragmented/Windows socket delivery. All 16
  provider-inbox action controls, including its three framing controls, pass.
- Strict collaboration all-target/all-feature Clippy, Rust formatting and diff
  checks pass. The signed frontend checkpoint separately records 908 passing
  tests, one intentional skip, lint, workspace/E2E types and desktop build.

The broad collaboration/workspace regression, packaged desktop behavior, remote
CI and live authenticated provider behavior are not claimed by this checkpoint.

## Qualification gates

- Bounded DTO/wire fixtures, null/unknown shapes, old generated schema ordering,
  invalid renderer IDs before account lookup and local-only query behavior.
- Draft CAS, unchanged saves, independent private/comment drafts, cold reopen,
  disconnect/reset recovery, synchronous cache redaction and held-read fences.
- Exact provider-origin file membership/artifact parsing for added/deleted/modified
  paths, Unicode/reserved paths, left/right ranges, renames, stale generations,
  omitted/binary/oversized/local artifacts and force-pushed heads.
- Numeric route, strict redirect refusal, exact event/body/commit JSON, account and
  permission fences, quota after every response and no POST before durable attempt.
- 200/403/422, malformed responses, lost response, cancellation, partial accepted
  readback, exact comment confirmation, cold restart and proof of zero second POST.
- Concurrent draft/send UUIDs, one-generation uniqueness, current command/recovery
  UI, exact head/body/file revision fences and historical reviewed-commit handling.
- Schema-23-to-24 migration, frozen-23 fixture/checksum, SQLite fault rollback,
  backup corruption, immutable proof/resolution linkage and quarantine.
- Generated IPC through `make typegen`, focused Rust/SDK/UI tests, strict Clippy,
  format/types/lint/build and full `make verify`. Packaged desktop, remote CI and
  live authenticated provider evidence are recorded separately and never inferred.
