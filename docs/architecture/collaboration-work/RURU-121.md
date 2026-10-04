# RURU-121: Bounded local prefetch and cached navigation

## Contract saved before implementation, 4 October 2026

Base: published R101/#156 `63e11970838c3bca1710f16175886f117e95aabb`, including
R77/R78 shared cached details and R98 native foreground demand leases. Live R121
is Backlog; prerequisites are implemented In Review, not Done. No existing R121
PR/worktree was found. R103 qualification is separate and not inherited. Read the
engine design/backlog before editing. Desktop accounts remain cloud-independent.

Provide a small bounded working set for pointer hover, keyboard focus and recent
navigation. Local reads warm existing QueryClient/SDK projections; automatic
interest may use existing ephemeral native leases and shared scheduler jobs.
Do not create durable hydration intents, a provider HTTP layer in TypeScript,
per-panel polling loops, or a second durable cache. SavedItemDetail stays the
ordinary shared view. Cached content renders immediately; absent/unsupported/
stale/loading states retain current explicit semantics.

Define explicit count, estimated resident byte and concurrent local-read budgets,
including queued candidates, completed cached entries, active reads and retained
interests. Repeated focus/hover must coalesce by exact account+epoch+instance+
subject/facet identity. Prefer deterministic LRU/TTL with bounded metadata;
large bodies must not bypass byte accounting. Inflight cancellation, hidden-tab
urgency release, account removal/epoch changes and revision reset must prevent
private late-result repopulation. Preserve selected item/editor state and real SDK
fences. Use existing native lease TTL/admission and capability/access checks.
Choose precise practical defaults after inspecting current SDK limits; record
actual limits before implementation, and avoid claiming exact heap measurement.

Own new SDK working-set module/tests, necessary SDK React/demand integration and
ordinary Workspace pointer/focus consumers/tests in this isolated worktree. Root
owns design progress/PR integration; no provider/storage migrations or generated
commands expected. If a shared backend change is necessary, present evidence
before editing. Read coss/react-useeffect skills for matching UI/hooks work.

## Required evidence

Deterministic large-list hover/focus navigation tests prove bounded queue/map/
bytes/concurrency, deduplicated exact identity, cancellation and hidden release.
Actual QueryClient/SDK regressions cover cache-first navigation, offline missing
states, reset/disconnect/account switch fencing, and no durable hydrate request.
Exercise real keyboard/pointer handlers through ordinary components, preserve
accessibility and selected drafts. Test production demand lifecycle behavior;
do not duplicate implementation with tautological mocks. Relevant SDK/UI tests,
repository lint/types/build and an isolated synthetic native flow where useful.
Distinguish estimated cache bytes from actual memory and local evidence from
remote platform/live provider qualification. Never inspect personal credentials.

All Cargo/build/generation commands use `/tmp/gitru-cargo-serial.py`; no direct
parallel Cargo. Signed scoped commits, reviewable PR based on #156, attach it;
do not merge. Root first advances the current R103 failure and native run.

## Practical bounds selected from the current source, before code

The existing per-webview DemandCoordinator admits at most 16 live handles and 128
entries, and native leases expire after 45s with a 15s heartbeat. Saved detail reads
use an existing QueryClient projection with infinite freshness until revisions
invalidate it and five-minute ordinary GC. Native saved bodies are capped at 1 MiB;
provider responses are separately capped at 4 MiB. Prefetch must therefore have its
own explicit admission bounds without changing those ordinary cache semantics.

Use at most 24 speculative resource identities per webview, counting queued,
reading, completed, and retired-but-still-pending records together. Allow 2 local
SDK reads concurrently across those identities; a cancelled native IPC retains
its slot/reservation until its promise actually settles. Reserve 4 MiB per active
read and 1 KiB per descriptor against a 16 MiB estimated-resident budget. Cache an
individual speculative projection bundle only when its measured estimate fits
within 2 MiB; oversized receipts are not inserted into the speculative cache.
The estimator counts UTF-16 string storage, primitive values, array/object/key
overheads and descriptor metadata; it is deliberately not an exact heap measure.
Decoded IPC receipts can have temporary allocation overhead before this check.

Also cap mounted scope descriptors at 8; scope ownership and queue metadata share
the resource budget. Per-resource query keys do not register permanent QueryClient
defaults. Native pending acquisition cleanup remains inside the existing bounded
DemandCoordinator, including its 16-handle admission ceiling.

Use 150 ms pointer/focus dwell to avoid scanning every crossed row, deterministic
LRU admission/eviction, and 120 s cache retention from last navigation interest.
At most 4 speculative native Body interests may be retained, with a 5 s lifetime
after admission/recent navigation; leaving the row releases urgency immediately.
These are existing ephemeral leases sharing native jobs, not new durable jobs or
a claimed lower native priority. Selected detail observers remain authoritative:
prefetch eviction never removes their active queries, private drafts, list pages,
or unrelated caches. The bounded working set accounts only for its speculative
resource projections, rather than claiming to cap the entire application heap.

Warm resource-specific contextual capabilities before item/Body reads. Require
the exact current account, epoch and instance and supported saved-read policies;
only eligible Body synchronization adds speculative native interest. Notifications
keep their current explicit subject-resolution flow. Do not prefetch authored
drafts, comments, diffs or other facets in this chunk. Pointer/focus and recent
selection reuse exact existing resource keys and selected SavedItemDetail views.
Hidden/inactive host, scope disposal, account removal/epoch change, and revision
reset cancel speculative reads and release their urgency; late completions cannot
insert old private data. Ordinary SDK fences and capability checks still apply.

## Implemented behavior and local evidence, 4 October 2026

`CollaborationClient.navigationScope` and `useNavigationPrefetch` share the real
SDK, authorization fence, revision bridge, QueryClient keys and DemandCoordinator.
No additional native command, provider transport or durable hydration admission
was added. The DemandCoordinator supplies one authoritative availability stream;
unknown ownership waits, while document/native deactivation retires pending work
and releases interest. An aborted uncancellable native read retains its descriptor,
concurrency slot and byte reservation until the underlying SDK promise settles.

Ordinary PR/issue rows send pointer enter/leave, native focus/blur and selection
intent. Search/filter/page changes dispose the previous feed region. Disposal
cancels incomplete reads and releases urgency immediately; already-completed
account-bound projections remain within the same LRU/120s budget for recent
navigation. Authentication cutover/reset revokes old scopes and removes their
speculative projections. Notifications retain their explicit subject flow.

A selected observer owns its projection even while disabled awaiting contextual
policy; speculative work yields rather than issuing a competing Body IPC. At
completion, the working set re-reads the current query and cannot overwrite an
active/fetching selected query or a newer unobserved result. Exact query cache
invalidation also retires interest, including contextual cooldown deadlines.
Inactive speculative eviction leaves active selected observers and private drafts
alone. No permanent per-resource QueryClient defaults accumulate.

Local validation for the final source:

- Full desktop and collaboration-client Vitest projects: 48 files, 429 tests pass.
  This includes 15 new SDK regressions and four new ordinary component cases.
  The SDK suite exercises a 10,000-row pointer sweep, 200 resident-body admissions,
  scope/count/byte/IPC bounds, deduplication, LRU/TTL, real native lease lifecycle,
  held-read/selected-observer races, offline missing/known-empty evidence, actual
  revision invalidation, and disconnect/reset/epoch fencing. Component cases use
  real coss buttons, native focus plus user Enter, actual saved QueryClient data,
  document hide and saved-search region disposal while preserving private text.
- Full desktop/SDK Biome: 291 files pass without fixes.
- SDK TypeScript and desktop plus E2E TypeScript pass.
- Production frontend build passes (5128 modules). Existing font resolution,
  mixed static/dynamic import and large-chunk warnings remain visible in its log.
- Scoped `git diff --check` passes; generated commands and native source unchanged.

Logs: `/tmp/gitru-ruru121-full-frontend-tests.log`,
`/tmp/gitru-ruru121-full-frontend-lint.log`, `/tmp/gitru-ruru121-sdk-types.log`,
`/tmp/gitru-ruru121-desktop-types.log`, `/tmp/gitru-ruru121-frontend-build.log`.
These are local UI/SDK results. No native package run, real provider/account check,
remote CI, exact JS heap measurement or lower native scheduler priority is claimed
by this issue's local evidence. Root owns signing, PR publication and remote checks.
