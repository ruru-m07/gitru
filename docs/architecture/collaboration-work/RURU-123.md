# RURU-123 — Cached pull-request reviews and review threads

Status: accepted pre-code contract, 8 October 2026. Read the shared engine and
backlog first. This bounded read-only slice starts from signed RURU-126
`e9cbad6496b0ff5870c928949f32bd7c6c98ec7c` plus the signed local merge of
RURU-118 exact head `e75bf77b920c9f1198d4e7d825cf829298209f5b` at
`3ca3348a5c74836765fc8d6de6f9351be0c4ecc2`. Live Linear RURU-123 was Backlog
with no attachment or duplicate; RURU-77 and RURU-122 are implemented review
stacks in ancestry. No merge is authorized.

## Scope and sources

Implement a provider-independent local model and the first read-only GitHub.com
adapter for review submissions and inline review-comment threads. Rust owns
provider HTTP, exact pull context, validation, SQLite, scheduling and generated
IPC. TypeScript reads only saved local snapshots through the generated SDK and
renders one bounded review panel. Accounts remain independent of Gitru cloud.
There are no review writes, ambient credentials, renderer HTTP, webhooks,
enterprise hosts, branch-protection conclusions or inferred merge permission.

Official source review:

- [GitHub pull-request reviews](https://docs.github.com/en/rest/pulls/reviews?apiVersion=2026-03-10)
  are chronological, page at up to 100 rows, expose review state and optional
  body, and require Pull requests read permission for private resources.
- [GitHub review comments](https://docs.github.com/en/rest/pulls/comments?apiVersion=2026-03-10)
  expose stable comment/review/reply IDs plus current/original commit and line
  anchors. Replies are grouped by their root `in_reply_to_id`.
- [GitHub GraphQL review threads](https://docs.github.com/en/graphql/reference/objects#pullrequestreviewthread)
  expose provider-native `isOutdated`/`isResolved`. The REST v1 slice does not;
  it must preserve those facts as unknown rather than infer them.
- [GitHub REST pagination](https://docs.github.com/en/rest/using-the-rest-api/using-pagination-in-the-rest-api)
  uses Link relations; missing or malformed continuation evidence cannot prove a
  complete enumeration.
- The official 2026-03-10 OpenAPI snapshot (SHA-256
  `4d10d960b047176d1be58543dbdfb8ece46d77efcc2e57411770747259b1345f`)
  declares review `commit_id` nullable and review-comment `pull_request_review_id`
  nullable while requiring comment identity, body, author, path, commit anchors
  and timestamps.

## Common model and authority

Add independent `ReviewSummaries` and `ReviewThreads` detail facets. Both map to
the existing `reviews` capability but retain separate scope, cursor, freshness
and error state. The older `Reviews` key remains a compatibility-only placeholder
so historical untyped rows cannot be misread as the new model. Schema 0018
preserves every historical detail and retention row while admitting the two new
keys. Define provider-independent typed
payloads:

- `ReviewContext`: exact Body base/head OIDs, base/source repository provider
  IDs, and Body metadata facet revision.
- `ReviewV1`: reviewer identity, normalized decision, bounded raw provider state,
  nullable reviewed commit OID and submitted time. A decision applies to the
  reviewed commit only when that OID is known; a fresh observation must never
  manufacture a current-head approval.
- `ReviewThreadV1`: stable thread/root/comment/reply/review IDs, author identity,
  created time, exact context, typed file/line anchor with current/original commit
  IDs, path, subject/side/range, and optional provider-reported outdated/resolved
  facts. Optional means unknown or unsupported, never false.

Every accepted row carries the captured `ReviewContext`. The internal traversal
source also persists that context so a complete-empty observation is still
bound to an exact Body generation. Entries use the context head in generic
`head_oid`; a review's actual commit and a thread's anchor commits stay in the
native payload. `DetailHeadScope::CurrentHead` therefore describes collection
coverage, not the meaning of every historical review.

A review collection is current-head authoritative only when its saved source
context equals the current exact Body context, the authorized view/epoch and
subject binding still match, the traversal began at page one and finished with
valid complete coverage, and saved evidence is fresh/ready. Historical reviewed
commits and anchors remain visible but explicitly historical. Partial, capped,
unknown, syncing, stale, denied and complete-empty states never become approval
or merge authority. There is no aggregate “approved” result in this slice.

Body base/head/source-repository facts must be saved Known from one exact Body
revision before review HTTP begins. Hydration persists Body and requested facet
intent; dispatch waits locally when context is missing, then revalidates context
immediately before HTTP and again in the write transaction. Head/base/repository,
selection, account actor/epoch, access, facet representation and authorization
view changes fence a held response. A changed context starts a new page-one
traversal without erasing previously cached historical entries before complete
replacement.

## GitHub adapter and bounds

Use immutable `/repositories/{repository_id}/pulls/{number}/reviews?per_page=50`
and `/repositories/{repository_id}/pulls/{number}/comments?per_page=50` routes
through the existing no-redirect private collection transport and API version.
Validate positive repository, pull, review and comment IDs plus returned pull URL
identity. Cursors are versioned, <=4 KiB and bind account/actor/epoch,
repository/subject identities, exact ReviewContext, strategy, facet, page count
and validated next URL. Only canonical next page = current + 1 is admitted.
At most 20 remote pages and 100 accepted rows per response; 4 MiB/20-second
transport bounds remain. Bodies are Known up to 65,536 bytes, otherwise
Oversized without prefix. Identity/path/login/provider-state strings and
RFC3339 timestamps are bounded and control-free. Canonical 40/64-hex commit OIDs
only; nullable review commit is preserved as unknown.

Review IDs sort as zero-padded numeric local IDs. Thread-comment local IDs sort
by zero-padded root then comment ID so cached pages keep a thread together when
possible. A root uses its own ID; a reply uses the validated root ID. Duplicate
IDs, self replies, malformed parents, invalid anchors, invalid URLs or arrays
above 50 reject the page atomically while positive quota evidence survives.
REST cannot prove provider outdated/resolved, so both remain `None`; the UI may
label an anchor historical only from exact context/anchor OID comparison.

Only a valid page-one singleton without a next link yields FullEnumeration.
Multipage and resumed terminal pages remain `Uncertain`; unseen historical rows
are retained and coverage stays partial. This avoids claiming a mutable paged
REST collection was an atomic snapshot. Explicit empty page one may reconcile
absence. Permission/auth/offline/quota failures preserve cached rows and expose
existing safe sync evidence.

## Local UI and tests

Replace the generic Reviews placeholder with a collapsed `CachedReviewsPanel`.
Opening it starts exactly one visible demand for each supported local facet and
reads page one of both caches; closing releases both. The panel provides explicit
Sync/Recheck, separate review-summary and thread pagination (50 rows, 100 saved
cursor positions), revision/view/epoch/context fences, and local restart after a
stale cursor. It renders raw body as text, current versus historical/unknown
commit and anchors, partial/stale/offline/permission/rate-limit evidence, and
provider-reported resolved/outdated only when present. It dispatches no mutation.

Native tests cover two independently paged routes, ordering/group identity,
nullable IDs/OIDs, decision normalization with unknown state, known/empty/
oversized bodies, anchor variants, malformed/hostile pagination, duplicate rows,
20-page cold/yield cap, exact-context pre-dispatch and held-response fences,
complete-empty/cache reopen with zero HTTP/vault, and honest partial/permission/
offline/quota states. Storage tests cover native validation, context persistence,
head/base/source/revision drift, historical row retention and current-generation
replacement. Wire tests cover all tagged variants/nulls/unknown provider state.
UI tests cover closed zero work, two independent local pagers, scope/revision
reset, safe rendering, historical labels, collapse and absence of review writes.

Run focused provider/runtime/storage/client/desktop tests, `make typegen`, strict
Clippy/format, then `make verify`. Packaged desktop, live private provider/PAT,
other platforms and exact PR-head remote CI are separate evidence and must not be
claimed unless actually run.
