# RURU-98 — Foreground demand leases and fair page scheduling

Status: implemented and locally qualified for reviewable publication on 3 October 2026. The root-approved contract below was recorded before implementation. Read against
signed RURU-77 `7e282ad`, the live RURU-98 description/relations and architecture
sections 11, 16, 17 and 23 on 2026-10-03. The isolated worktree is
`/Users/ruru/.codex/worktrees/collab-ruru-98/gitru`, branch
`ruru/ruru-98-foreground-demand`. Implementation starts only after this contract is saved in the managed worktree.

Live acceptance: account/scope-bound expiring leases; hidden/closed/disposed/epoch
cleanup; equivalent multi-view interest shares one provider job; interactive work
passes backfill within rate/concurrency limits; deterministic lost-cleanup,
reopen, idle, saturation and offline/reconnect tests without reconciliation
starvation. RURU-97 is the listed prerequisite. RURU-102/103/119/121 are blocked
by this slice. RURU-78 issue support can land independently: a lease follows the
declared native capability, never assumes that PR support enables issues.

## Problems in the current scheduler

`runtime.rs::enqueue_work(manual=true)` bypasses ordinary due time and resets
failure history for a new job. Renewal must not call this path. `run_next` awaits
one `sync_job`/`sync_detail` traversal, currently up to ten HTTP pages; a Notify
wake cannot interleave selected work while that traversal owns the worker.
Partial feeds preserve a committed next cursor but currently receive the same
180/600-second interval as complete validation. Detail pending admission precedes
all feed admission, and 128 ready jobs can crowd out newly selected work.

These are the R98 fairness/due changes, not optional follow-on work. R102 can add
shared-principal/API-family attribution and adaptive concurrency after this
bounded single-request scheduler is correct.

## 1. Native ephemeral lease contract

Use native-issued opaque lease IDs. Core APIs accept a native `DemandOwner`
separately from the wire request; no request can supply its authoritative caller.
An owner includes actual webview label plus native-issued activity/session
generation. Root's Tauri adapter derives the label from the inspected local
`Webview`, checks the existing origin/caller policy and captures the current
native owner generation. Renew/release compare the same owner and the opaque
lease ID. A local child cannot renew/release another child's lease or claim the
main owner's permissions. A delayed old owner session cannot resurrect activity
after hide/close/reopen of the same label.

Proposed additive wire types (final spelling may change together before typegen):

- `DemandTargetKind`: `repositories`, `inbox`, `pull_requests`, `issues`, `detail`.
- `DemandTarget`: kind plus optional `repository_id`, `subject_id`, `facet`.
  A single validator rejects contradictory/missing fields. Repository-list/inbox
  have no IDs; a repository feed can name one selected repository or omit it to
  cover the account's selected repositories; detail requires canonical subject
  and typed `DetailFacet`. No arbitrary scope string, URL, SQL, cursor or user
  cadence is accepted.
- `AcquireDemandRequest`: captured `account_id`, `authorization_epoch`, expected
  owner activity generation, target.
- `DemandLeaseReceipt`: opaque `lease_id`, native TTL/renewal interval and owner
  generation. It is activity metadata, not provider content or a job receipt.
- `RenewDemandRequest`: lease ID plus expected owner generation/account epoch.
  Target is immutable; changing selection releases/acquires.
- `ReleaseDemandRequest`: lease ID. Idempotent for an already removed lease
  belonging to this owner; a live different-owner ID is denied without content.

The native TTL starts at 45 seconds, with a 15-second SDK liveness renewal.
Use an injected monotonic clock in core tests; no `Instant::now() - duration`
backdating. Expiry runs on every admission/dispatch decision and on the single
native timer, even when there are no eligible provider jobs. Renewal does not
create SQLite intent, clear retries, extend another owner's lease, or mark saved
data fresh. Lease creation/expiry alone need no content revision hint.

Bounds: at most 128 native lease handles globally and 16 per native owner; reject
excess new handles with typed Busy while preserving existing handles. Equivalent
targets aggregate by account + verified instance + actor + authorization epoch +
canonical scope. Per-owner SDK dedup further reduces handles, but Rust enforces
all limits independently. Removing one of two equivalent owners removes only
that owner's interest. IDs, deadlines and scheduling metadata are bounded;
leases retain no bodies, tokens, editor buffers or response payloads.

Acquisition and renewal inspect the current active account/epoch, exact provider
instance, selected repository/subject ownership and declared facet. Scope/parent
denial is an authorization barrier. Quota/offline/transient failure may suspend
execution while retaining otherwise authorized interest; it cannot reject the
concept of an interested view merely because HTTP is presently delayed.
Disconnect, grant replacement, scope denial/deselection and owner disposal fence
queued work and all renewals. Existing request/epoch/run/parent/source transaction
guards still decide whether an in-flight page may commit.

No activity lease survives process restart. SQLite checkpoints, cached data,
strict cooldowns and explicitly requested durable detail reads do survive.
Automatic foreground interest must not call `Store::request_detail`; closing a
view cannot leave a new durable automatic request behind. Manual Sync/hydrate
retains the existing explicit durable read contract.

## 2. Visibility, close and lost cleanup are part of this slice

Do not rely on `document.visibilityState` for embedded child webviews: native
show/hide is controlled by `webview-tab-host.tsx`, and Tauri 2.11.1 exposes no
`Webview::is_visible` or child-destroyed variant in its WebviewEvent API. Root
owns a native demand-owner activity adapter coordinated with the existing
serialized host visibility work:

- Main host authoritatively activates the successfully shown child owner and
  revokes the previous owner's activity before hiding it. Activity/session
  generations make delayed old acquire/renew calls ineffective.
- Suspension for the account dialog deactivates all child demand owners before
  native hide completion; restoration activates only the selected successfully
  shown child. Cold/background prewarm starts inactive.
- Native tab close/host disposal revokes that owner's activity and leases before
  the actual close, including cleanupAllWebviews and failed-creation paths.
  A reopened same-label child receives a new generation.
- Standalone main-content routes have a main owner. Switching to an embedded
  child or an obscuring host state removes the main content's urgency. Window
  hide/minimize and destruction deactivate its owners; resume reactivates only
  the current selected visible owner. Native window visibility/minimization is
  checked at owner admission as an additional gate.
- The SDK listens to its owner activity/DOM pagehide/visibility events and stops
  renewal immediately, with best-effort release. These events are hints; they
  never authorize another owner's mutation. Native expiry covers lost unload,
  failed release, crashed JS and suspended heartbeat delivery within 45 seconds.

Expose owner activity through generated local commands/events with fixed bounded
payloads. The existing origin/main-only authorization applies to host commands
that target other views; a child only manipulates its own leases. Host integration
needs lifecycle tests for delayed show/hide, modal suspension, close/recreate and
late IPC. This is required acceptance, not deferred to R103's broader webview
recovery qualification.

## 3. One HTTP page is the scheduling quantum

Keep production HTTP concurrency at one. Fairness is measured at committed page
boundaries, so an already dispatched request may finish within the existing
transport deadline; foreground work cannot interrupt a network server mid-page.
Extract feed/detail single-page execution with `Complete`, `Continue`, `Deferred`
or `Failed` outcome. Acquire credentials only for admitted dispatch, fetch outside
SQLite/lifecycle locks, commit the checkpoint and revision before requeue/hints.

A continuation preserves traversal membership: partial feed scopes retain the
existing `scope.run_id`; detail continuation retains its current traversal
generation and authorization view inside the same queued job. Do not begin a
new membership run for each page. Recheck account/instance/capability/repository,
strict scope/provider barriers and stored checkpoint before every dispatch.
Validator use is only comparable page-one/complete-scope validation as today.
Account/grant/deselection/head invalidation still rejects an old page at commit.

The existing ten-page activation ceiling becomes a cumulative page count across
interleaved quanta, not ten uninterrupted requests. A yielded continuation is
eligible immediately for another scheduler turn unless a strict provider poll,
cooldown or retry barrier applies. At the ten-page activation ceiling, retain the
durable checkpoint and defer that traversal's next activation by a bounded
10-second continuation gap, distinct from completed-scope cadence. Foreground
leases and background reconciliation can then resume it fairly. Partial detail
automatic continuation requires live interest or explicit durable demand; an
expired automatic lease cannot authorize pages after the current quantum.
Feed reconciliation may resume selected scopes at background priority when a
foreground owner disappears. No continuation is silently treated as complete.

## 4. Arbitration, queue pressure and reconciliation fairness

Use explicit admission reasons `Manual`, `Foreground`, `Reconcile`, plus page
continuation state. A semantic job key remains one per account epoch/canonical
scope; equivalent owners promote that job instead of duplicating it. Queued
priority is recomputed from live leases; expiry removes urgency immediately.
Manual explicit demand is retained separately from foreground interest.

Weighted arbitration provides up to three interactive page turns followed by
one eligible reconciliation page turn. If one class is empty or quota-blocked,
the other uses the slot. Round-robin eligible accounts within each class, then
rotate their scopes, so a large account/long traversal cannot monopolize the
lane. Manual and visible detail/index jobs share interactive rotation, with
manual/new selected detail favored within a bounded turn rather than unbounded
push_front. A continuously ready background scope receives service within a
finite page-turn bound; a test asserts the exact bound. Do not claim a wall-clock
SLA under exhausted quota or a stalled bounded request.

Bound total ready scopes at 128 and ordinary reconciliation occupancy at 96,
reserving 32 interactive slots. Promotion of an existing scope allocates no new
job. At capacity, evict/defer only rebuildable ordinary background admissions;
retain their committed checkpoint and rotating admission cursor. Never evict
explicit manual intent, live foreground leases or active HTTP. If more than 32
distinct interactive scopes exist, a bounded rotating lease-admission cursor
gives them turns rather than always taking the same prefix. Lease success is
independent of immediate queue admission: saturated interest stays represented
in the bounded lease registry and receives a later turn.

Durable detail admission and feed admission must share this arbitration. A fixed
`pending_details LIMIT 128 ORDER BY account_id,...` prefix must not repeatedly
fill every turn and starve feed reconciliation. Use bounded cursor rotation for
pending explicit detail discovery (a narrow storage read change if necessary),
then arbitrate it with feeds and lease targets. Account-wide list leases expand
only selected repository scopes through a rotating iterator; they never clone
or enqueue every accessible repository into memory.

## 5. Due, quota, retries and continuous foreground renewal

Lease renewal is liveness, never a refresh request. The single native timer/
Notify planner inspects aggregate interest and admits only due work. Separate
ordinary completed cadence, committed continuation readiness and strict barriers
instead of overloading one due map. Persisted absolute retry/provider deadlines
remain authoritative across restart/long waits; monotonic bounded segments are
only local scheduling helpers. Every page rechecks persisted barriers, and all
owners/accounts continue to share the existing upstream account REST cooldown.
Preserve maximum concurrent quota deadlines; a shorter later observation cannot
shorten an already known longer barrier.

Eligibility requires all of: current authorized target, runnable owner interest
or background/explicit intent, no equivalent in-flight job, strict retry/poll/
provider deadline elapsed, and appropriate soft cadence or continuation ready.
Foreground may shorten background cadence using a native policy measured from
last committed validation, but repeated renew/open events do not slide/reset or
bypass that foreground deadline. Defaults: visible inbox at least 60 seconds,
selected PR/issue indices 60 seconds, visible repository discovery 120 seconds;
background defaults remain inbox60/index180/discovery600. Body uses its committed
native freshness deadline (currently180 seconds for GitHub) and supported future
collection facets a conservative60-second native cadence. Provider minima and
strict barriers always raise these intervals.

Cold/dirty required coverage is immediately eligible only if strict barriers are
clear. An old-schema fresh body lacking the required resource metadata gets one
coverage opportunity per aggregate selected session/binding, preserving R77's
bounded first-selection behavior; completion/omission/oversize records a native
next due time so renewals do not loop on incomplete optional fields. Known-null
and known-empty detail snapshots with fresh metadata do not request HTTP merely
because they are empty. A changed canonical head/run binding dirties the scope
and can rearm coverage after strict barriers, without erasing saved data.

200/304 success updates cadence from engine receipt/provider TTL and strict poll
headers. Omitted/oversized results use a bounded next attempt time, not immediate
renewal loops. Offline/network failures preserve saved reads and interest, and
retain exponential retry count, jitter and next_retry_at across renewals. An
online/resume hint wakes the planner; it cannot reset quota or retry deadlines.
When the deadline expires, the still-visible lease makes one coalesced attempt.
Permission/auth failures fence content/interest under existing lifecycle policy;
recovery uses an explicit recheck/new authorized lease, not an automatic denial
loop. Expired cooldowns cannot remain stuck merely because there is no new UI
content event: the native planner is timer-driven.

## 6. Bounded SDK and UI integration

Root owns this integration. Add one per-client/webview lease coordinator with
reference-counted equivalent targets and at most128 entries. Active view hooks
register demand outside TanStack queryFn; query functions remain SQLite-only,
networkMode always and interval/focus/reconnect fetching disabled. Replace R77's
automatic durable one-shot selection hydration with the new foreground lease;
keep explicit Sync/hydrate as the existing user-action path. Visible list hooks
retain the typed index/inbox target, selected details additionally retain Body.

One15-second liveness heartbeat batches renewals for that owner (bounded16),
rather than a provider poll per component/tab. StrictMode cleanup/remount gets a
one-task reference-counted grace; released/epoch-reset/account-switched handles
are inactive and late acquisitions are immediately released, never adopted by
the replacement selection. Capture account/actor/epoch/target immutably. No
visited-resource history, bodies, draft data or unbounded timers are retained.
Visibility/owner activity/pagehide/bridge disposal stops heartbeat and releases
handles; a restored owner obtains its new generation and reacquires exactly the
currently mounted targets. Local offline SQLite queries still run normally.

R121 remains bounded hover/recent-navigation prefetch. This slice covers actual
visible list/detail subscriptions and continuous native freshness renewal;
prefetch is unnecessary to satisfy R98.

## 7. Meaningful deterministic tests

Core clock and jitter seams permit deterministic deadlines without wall sleeps.
Use fake adapters/vaults and controlled first-page barriers with actual public
Store commits where fences matter. Planned regressions:

1. Two caller owners/same target share one job; one release preserves the other;
   final release removes foreground continuation; no new SQLite detail demand.
2. Foreign lease IDs, old epoch, wrong instance/repository/subject and replaced
   owner generation create zero provider/vault/queue mutations. Scope denial and
   deselection fence delayed commits while drafts remain untouched.
3. Lost cleanup expires exactly at45seconds; renew before expiry extends only its
   own lease; late renewal after expiry fails; close/reopen same label cannot use
   the old token/generation. Idle/fresh views renew with zero HTTP until due.
4. A fake paginated backfill pauses after committed page1; a selected detail
   arrives and dispatches before page2, rather than waiting for pages2..10.
   Existing membership run/cursor survives interleaving and cold reopen.
5. Constant interactive demand cannot starve eligible reconciliation: exact3:1
   page-turn bound plus account/scope rotation; later detail/large-account scopes
   receive turns. Quota-blocked scopes do not occupy the one HTTP lane.
6. Ready saturation96background+interactive reservation, >32 interactive targets,
   bounded registry Busy, durable-detail discovery beyond an ordered128prefix and
   account-wide selected-feed expansion prove finite rotation and bounded memory.
7. Offline response sets retry; repeated renewals/online hints issue zero calls
   before the exact deadline and do not reset failures. At deadline one job runs;
   release while offline prevents automatic retry; explicit intent survives.
8. Future provider cooldown/scope Retry-After/poll minimum constrains all lease
   calls/pages and persists after runtime restart. Longer barrier cannot be
   shortened by renew/manual or a later smaller hint. Expired quota wakes without
   requiring another content event.
9. Native view-host tests cover prewarm inactive, delayed show/hide, modal
   suspension, failed close/release, disposed host and old same-label IPC.
   SDK fake-time tests cover one batched heartbeat, real StrictMode, late acquire,
   account reset/actor/target switch, hidden restored owner and bridge teardown.
10. Public cache tests prove cold reads/reopen/304, partial/omitted/oversized/null
    details stay truthful; freshness changes drive native due work, not local
    query HTTP; authored buffers/CAS stay outside activity lifecycle.

Native focused tests, complete collaboration suite, all-target Clippy/fmt and
normal make typegen/SDK/types/scoped frontend tests precede signing. Root owns
isolated packaged native visibility/close QA and exact-head remote CI. No personal
account/CLI/vault or live provider is used. Do not mark fairness/visibility/offline
acceptance complete solely from an isolated in-memory lease registry test.

## 8. Ownership and implementation modules

Native lead (this agent):

- New `crates/collaboration/src/demand.rs`: additive wire DTOs/typed target shape.
- New `crates/collaboration/src/runtime/demand.rs`: owner/lease aggregation,
  admission/expiry/clock seams; no Tauri dependency.
- New `crates/collaboration/src/runtime/scheduler.rs` (or equivalent narrowly
  extracted module): priorities, queues, fairness and page outcomes. Runtime
  wiring stays in runtime.rs; feed/detail page extraction in runtime/details.rs
  and a new runtime/feeds.rs to keep the main module reviewable.
- Narrow storage accessor for initial detail due/metadata evidence in an existing
  read transaction; optional bounded rotating pending-details accessor if needed.
  No migration, authored draft, alias or credential lifecycle changes expected.
- New runtime/demand_tests.rs, runtime/scheduler_tests.rs and public integration
  tests/foreground_demand.rs; minimal existing fixture adaptations. New
  docs/architecture/collaboration-work/RURU-98.md saved before implementation
  once root approves this contract and copies it into the worktree.

Root/assigned UI/caller owner:

- apps/desktop/src-tauri/src/commands/collaboration.rs and command registration;
  native owner-activity/window lifecycle adapter in a separate new module.
- Normal make typegen + scripts/collaboration-bindings.ts changes if required.
- SDK new demand coordinator/hooks/transport methods and lifecycle tests;
  minimal visible list/detail consumer wiring.
- webview-tab-host.tsx visibility/close/disposal integration and native caller/
  host lifecycle tests. Shared generation happens only after DTO freeze.
- Master architecture/backlog continuation, Linear, final signed integration,
  PR/push/merge/QA. No overlapping generated-command edits from native lead.

Keep production GitHub/issue mapper/storage-metadata contract from77/78 unchanged.
R78 can rebase independently; shared scheduler behavior is provider-independent.
No new external package/server/service, remote writes, OAuth flow, mutation outbox
or future-provider live implementation is included. If implementation exposes a
necessary migration, pause shared-schema edits for root review rather than
silently changing the contract.


## Root approval clarifications

Native owner-activity observation must have a local getter and bounded event
for the calling webview so acquisition cannot depend on a frontend-invented
generation. The main host's existing serialized native show/hide lifecycle will
coordinate revocation before hide and activation after successful show. Delayed
commands compare expected generations; ordinary child commands cannot activate
other owners. Window hide/minimize gates are checked natively.

One-page continuation retains the existing partial feed run ID/checkpoint and
final traversal membership semantics. Reopening or renewing interest never
resets completed-scope cadence, provider maxima or retry count. Automatic detail
interest stays ephemeral; manual explicit hydration remains durable. Save this
contract before edits; root owns caller/generated bindings and publication.

## Implemented native core and verification

`demand.rs` contains the additive wire types. `runtime/demand.rs` owns caller
generations, ephemeral handles, atomic renewal and local authorization. Owners
start inactive; activity transitions/disposal/reopen allocate increasing decimal
generations. Native bounds are 128 owners, 128 live handles and 16 handles per
owner; handles expire exactly at 45 seconds and advertise renewal at 15 seconds.
Admission and renewal never read the vault, contact a provider, persist detail
intent, clear retry history or change content freshness. A visibility probe is
optional for core fixtures and supplied by the desktop in production; probes run
outside the scheduler lock and deactivate only the generation they inspected.
Physical hiding or a missing native view is checked again before each page.

The desktop caller policy binds operations to the inspected local webview and
exact configured production origins, with the explicit development port 1420.
Host lifecycle commands remain main-only; content views can manage only their
own opaque leases. Resetting an absent owner is limited to valid managed child
labels and does not restore desired host activity. A cold same-label replacement
resets that absent incarnation before constructing the native view, including
after failed demand disposal. Present targets must still have an inspected local
URL. Native policy tests cover rejected production ports/userinfo and absent,
foreign, malformed and present target labels.

`runtime/scheduler.rs`, `feeds.rs` and `details.rs` arbitrate one committed HTTP
page per turn. Partial feeds preserve the original membership run and cursor;
detail continuations retain their run/authorization receipt. Each activation
stops after ten pages and yields at least a ten-second ordinary continuation
gap. Strict provider, polling and retry barriers remain maxima above ordinary
cadence. A private injected clock and deterministic jitter seam support tests;
production uses monotonic time, UTC receipt times and bounded random jitter.
One dispatch mutex prevents concurrent wakes from opening a second HTTP lane.

Ready capacity reserves 96 reconciliation jobs and 32 interactive jobs, with a
3:1 interactive/background arbitration limit and rotating account/scope order.
Within interactive work, two detail turns yield to one index turn when both are
eligible. Account-wide interest does not promote every queued backfill: actual
admission claims a bounded foreground reservation. A selected detail can demote
a rebuildable promoted feed, or release a rebuildable background ready slot at
total saturation, while its saved run/cursor and lease remain available. Manual
receipts retain at most 128 deferred jobs outside ready reservations when quota
or capacity blocks them. Continuations recheck capacity after HTTP, so admission
during a request cannot exceed the ready bound. Automatic blocked work remains
represented by its lease or durable explicit intent rather than occupying ready
capacity. Neither release nor native expiry deletes provider data or drafts.

The storage additions are migration-free, metadata-only admission reads:
authorized detail freshness/coverage, batches of at most sixteen repository
keys for global indexes, and rotating batches of sixteen durable explicit detail
intents. Fresh or inaccessible prefixes do not hide subjects beyond the former
fixed 128-row prefix. The existing public pending-detail read remains compatible.
Old credential, alias, detail metadata and authored-draft schemas are unchanged.
Existing runtime detail/crash fixtures were adapted to the new one-page method;
the integration bootstrap wait now observes the actual page-eleven checkpoint,
because every committed partial page can be idle between turns.

Sixteen clock-driven native cases cover exact expiry/lost cleanup, atomic batch
failure, foreign owners, same-label reopen, native-hidden dispatch, equivalent
views, idle renewal, reconnect/deselection/denial before vault access, offline
backoff and quota maxima, continuous due work, partial interruption with one HTTP
lane, 3:1/account/scope fairness, lease/ready saturation, global repository
rotation, explicit intent beyond a fresh 128-subject prefix, ten-page yielding,
and close/offline/restart with saved body and draft CAS intact. The explicit
promotion regression queues 96 actual partial repository scopes, fills 32
foreground reservations with an account-wide view, opens a body, and verifies
the body is the next HTTP request while all 96 run/cursor checkpoints survive.

Native validation uses the outer serialized Cargo wrapper and this worktree:

| Check | Result | Log |
| --- | --- | --- |
| Focused native demand cases | 16 passed | Full native log includes all cases |
| Full collaboration suite | 159 passed; 2 existing ignored subprocess helpers | `/tmp/gitru-ruru98-native-tests.log` |
| Frozen v1-to-current migrations | 12 passed in the full suite; no new migration | Same full native log |
| Native caller and target policy | 6 passed | `/tmp/gitru-ruru98-native-caller-tests.log` |
| Workspace all-target Clippy | Passed with warnings denied | `/tmp/gitru-ruru98-native-clippy.log` |
| Workspace Rust formatting and diff whitespace | Passed | `cargo fmt --all --check`; `git diff --check` |

Native GUI/host, generated bindings, SDK and publication evidence are owned by
their respective authors below or in the final root handoff; these core tests do
not use personal accounts, live provider HTTP or an operating-system keychain.

## Frontend and SDK integration

<!-- SDK owner section: foundation_frontend_review. Root owns UI/native host evidence. -->

The approved SDK plan is recorded in `/tmp/gitru-ruru98-sdk-plan.md`. Native
`Demand*` DTOs are source-generated normally; no generated file is hand-edited.
The SDK exports `useVisibleDemand({ account, target, enabled }): unknown | null`
and account-bound `retainDemand(target)` handles. This SDK/frontend owner also
owns shared workspace interest eligibility and its collaboration regressions. Root owns native
caller/activity and webview visibility integration.

One client/webview coordinator shares equivalent mounted targets with reference
counts and one next-task StrictMode retirement grace. It captures account,
actor, provider/installation, epoch and the fixed typed target before IPC.
It retains at most 128 current/pending/grace records and limits acquired plus
in-flight acquisitions to 16; excess interest reports bounded Busy. There is no
visited-resource history, provider body, credential or private-draft payload.
Installing the query bridge alone starts no activity IPC: activity observation
starts lazily at the first visible retain.

The coordinator subscribes to native activity before reading its initial
snapshot. Positive decimal generations are compared with BigInt so a delayed
getter/event cannot undo a newer owner transition. The transport listens for
`collaboration:owner-activity` and filters payloads to the actual current native
webview label; it never accepts a caller label from a component. It has no
activation/host mutation API. DOM visibility/pagehide can suspend leases; DOM
resume first observes native activity, which remains the authorization source.

One 15-second timer sends an atomic native renewal batch of at most 16 IDs. It
never overlaps renewal batches, performs content reads, calls refresh/hydrate,
changes provider due times or invalidates query caches. Native owns the 45-second
TTL and all HTTP/retry/freshness policy. An SDK receipt that arrives after its
conservative request-start TTL cannot be adopted. A pending old acquisition
retains its raw opaque ID for best-effort release after account/target/owner
replacement, final release or bridge disposal. Failed cleanup is ultimately
bounded by native expiry, rather than a frontend retry loop.

Account/global resets deactivate and release handles before provider cache
removal. The React hook observes the existing client lifecycle version, so a
same-epoch authorization-view reset re-retains only the current enabled target.
Private draft/editor/CAS behavior remains outside activity lifetime.
Busy and permission failures are not retried by heartbeat, readiness hints or
visibility changes. Current stale leases can rearm on a new authoritative owner
generation. A batch invalidated by a known local account reset does not misclassify another still
current account as denied; remaining receipts retain their original bounded TTL.

Expiry after a JavaScript stall gets one shared authoritative activity read,
then one current account/actor/epoch/target/generation reacquisition opportunity.
A failed or inactive observation stays paused until later native activity or an
actual resume/selection; it never starts a provider retry loop. A DOM-only hide
can interrupt that expiry read without changing native generation. A real
resume captures new current-binding tokens and reads native activity before
clearing only a stale-view failure; newer resume events during a pending read
coalesce one follow-up observation. Old actor/session bindings cannot rearm.

A successful local revision catch-up is also a readiness proof. Startup
`not_ready` failures receive at most one readiness-triggered activity repair per
retained binding; permanent Busy/permission failures remain terminal. Root emits
the existing durable wake after publishing the native runtime, so this repair
does not require reopening a route or adding a readiness polling interval.
Repeated readiness events and repeated NotReady results do not spin.

Root switched common consumers to this API. The obsolete RURU-77 automatic
one-shot durable selection hook, coordinator and client API were then removed,
with a usage scan confirming no remaining callers. Their six obsolete tests were
replaced by lifecycle coverage. Cache-only query options and explicit manual
`hydrateDetail` remain, including the existing epoch-injection and delayed
manual receipt fencing tests. Authorized transient/quota interest is distinct
from immediate synchronization eligibility; root's policy preserves that
interest while native dispatch remains subject to persisted barriers. The
workspace retains canonical body interest independently of null/empty, omitted,
oversized and freshness evidence; native eligibility decides whether work is
due. Accepted head/list/304/metadata revisions reuse the same ephemeral lease.
Manual Sync remains a separate explicit durable intent. Body access loss hides
provider header/text and releases interest while preserving private authored
text and its inspected CAS generation; actor switches isolate both. Back-to-list
releases the selected Body lease while repository/feed interest remains.

The test-only demand boundary explicitly registers generated acquire/activity/
renew/release command mocks. It does not make unregistered content commands
succeed. Actual transport recipient filtering is tested separately against the
real generated SDK boundary.

SDK/frontend validation (final owned source after expiry/startup/resume repair):

| Check | Result | Log |
| --- | --- | --- |
| Full SDK suite | 82 tests passed across 8 files | `/tmp/gitru-ruru98-sdk-tests.log` |
| SDK TypeScript | Passed | `/tmp/gitru-ruru98-sdk-types.log` |
| SDK Biome | Passed | `/tmp/gitru-ruru98-sdk-lint.log` |
| Focused collaboration UI | 54 tests passed across 4 files | `/tmp/gitru-ruru98-desktop-focused.log` |
| Full desktop suite | 222 tests passed across 28 files | `/tmp/gitru-ruru98-desktop-tests.log` |
| Desktop and E2E TypeScript | Passed | `/tmp/gitru-ruru98-desktop-types.log` |
| Scoped desktop Biome | Passed | `/tmp/gitru-ruru98-desktop-lint.log` |
| Legacy automatic selection usage scan | Zero matches in SDK/desktop source | `/tmp/gitru-ruru98-legacy-selection-usage.log` |
| Frozen Bun copyfile installation | Passed; dependency files unchanged | `/tmp/gitru-ruru98-bun-install.log` |

New coverage comprises 23 deterministic coordinator cases, 11 real React hook
cases and 3 actual generated-wire cases. They prove equivalent/StrictMode
coalescing, final-reference release, actor/epoch/target input capture, late
acquire cleanup, old getter/event suppression above 2^53, inactive/reopened
owners, bounded records/atomic batches, expired pending renewal, non-overlap,
account-reset isolation and listener/timer disposal. React cases exercise real
StrictMode effects, actual revision-bridge authorization reset, DOM hide/native
resume, DOM-only same-generation repair, bounded NotReady recovery through
actual bridge catch-up, actor switching and disabled consumers with zero
admission. SDK test timers perform only lease liveness IPC, never provider HTTP.
A real local query remains immediately readable while acquire is pending and keeps its read
count unchanged through heartbeat ticks. Wire cases use generated Zod schemas
and generated commands for all target variants, nullable IDs/facets, exact epoch/
generation strings, native activity, atomic renew, release and recipient-label
filtering. No personal credential, live provider HTTP or native GUI is used in
these SDK checks; native core/host fairness and actual WKWebView QA are recorded
separately by their owners.

<!-- End SDK owner section. -->


<!-- ROOT_HOST_EVIDENCE_START -->
## Root host integration evidence — 3 October 2026

Eighteen desktop host cases pass, including the existing delayed native show,
hide, create, modal, remount and StrictMode coverage. New assertions verify that
only a successfully shown selected child gets urgency; prewarmed/main/hidden
owners stay inactive; modal transitions revoke before hide; close disposes before
physical close; and a replacement same-label view gets a fresh generation even
when the previous revocation IPC failed. A missing child incarnation is disposed
with the main-only generation-fenced command before constructing the replacement.
Cleanup still physically closes the surface when revocation fails. The native
window observer/probe provides the separate minimize/hide/destruction gate.

Generated IPC was produced only by normal `make typegen` (103 commands), without
manual generated-file edits. Host Biome passes. Raw log:
`/tmp/gitru-ruru98-host-tests.log`. Final caller (6 cases), SDK (82 cases), desktop (223 cases), types/Biome and
all-target workspace Clippy pass. The host additionally repairs a genuine
NotReady startup once after an authoritative post-publication revision wake,
without tab revisits, polling or permission/busy retries. Native/SDK/host peer
review accepted these lifetime and repair fences. Isolated E2E/debug macOS
packaging passes; actual native UI QA and publication remain in progress.

Inherited signed PR #150 `7e282ad` and sibling issue PR #151 `863967f` each now
pass all 11 reported remote checks, including Rust and packaged E2E on all three
platforms. Neither reports an exact-head CodeQL check; that gate and production
provider/vault qualification remain separate. Neither was merged.
<!-- ROOT_HOST_EVIDENCE_END -->


## Final native UI and publication checkpoint — 3 October 2026

The isolated macOS debug/E2E build uses identifier `com.ruru.gitru.ruru98.qa`,
`/tmp/gitru-ruru98-qa/Gitru RURU-98 QA.app` and frozen executable SHA256
`7c82853223483966483ac835453fbe7350e4d40add5ef91edff43ca7072469ab`.
It uses the test-only memory vault and disabled CLI discovery. Its seed contains
only two synthetic actors, four cached subjects/metadata/drafts and strict
provider cooldowns until2099; no credential or production app data was read.

Actual WKWebView computer-use QA exercised the main host plus two native child
tabs: opening the saved PR detail under quota, child Accounts→main-only PAT/CLI
dialog→close/restored detail, switching to the prewarmed Inbox and back, and
native minimize/Window-menu restoration with cached detail retained. No token
was entered and no connection was attempted. This proves actual native view,
command, modal and saved-render wiring; opaque lease state and HTTP fairness are
qualified by the deterministic native/SDK tests, not inferred from a screenshot.

After native Quit, read-only SQLite comparison proves automatic `detail_demand`
remains empty as before launch, all four metadata snapshots and generation1 drafts
remain intact, both strict2099 REST barriers remain, and credential references and
cleanup counts are zero. The QA process is gone and test port4445 is free.
Logs/artifacts: `/tmp/gitru-ruru98-qa-native-build.log`,
`/tmp/gitru-ruru98-qa/prepare.log`, `detail-demand-before.json` and
`post-quit-evidence.txt` in that QA folder.

Final local validation:159 collaboration tests (16 demand,12 frozen migration;
2 existing ignored subprocess entrypoints),6 native caller/target cases,82 SDK
cases,223 desktop cases (18 host lifecycle), SDK/desktop/E2E types, scoped Biome,
workspace all-target Clippy/formatting, frontend production build and isolated
native debug/E2E packaging pass. Normal `make typegen` generated103 commands.
A source generation correction normalizes event function separators and removes
the generator's unused Event import while preserving the wire event name; it
also fixes the newly discovered sibling local-link event output. Generated files
remain generated. Peer review accepted native scheduler, caller, SDK and host
fences. No schema migration was introduced by this slice.

RURU-98 is prepared as a scoped signed review PR stacked on signed RURU-77
`7e282ad`/PR150. RURU-78/PR151 is a sibling with independent issue adapter support;
this branch does not claim production issue support before that integration.
Exact-head remote CI starts only after publication and remains distinct from
these local results. Production provider/vault qualification, shared-principal
budgets R102, broader webview recovery R103 and prefetch R121 remain separate.
No merge is authorized or performed.


## RURU-78 integration restack — 3 October 2026

The two RURU-98 commits originally published at `904632260f44a5dce80f398ff7afcb996dca9204`
are replayed with signatures onto signed RURU-78/PR #151
`863967f8dc55587c1f71061fd2267a1c139bed5e`. This inherits the issue-detail
adapter, capability and endpoint tests while preserving RURU-98's scheduler,
native caller, SDK and host implementation. The two master-document conflicts
are resolved by retaining both RURU-78 and RURU-98 chronicles. No runtime,
schema, command signature or generated wire change is needed for the restack;
`packages/commands` remains byte-identical to the previously published RURU-98
head. RURU-96 and RURU-79 remain separate worktrees.

The first combined desktop suite passed 229 cases and exposed one obsolete
RURU-78 test assertion expecting automatic durable `collaboration_hydrate_detail`
admission. The approved test-only adaptation uses the existing native demand
mock and captures exactly one account/epoch/canonical-subject Body lease through
both omission and later validation revisions, with zero automatic durable
hydration. Its saved reason, Unicode text, omission and staleness assertions are
retained. This is the only implementation-range integration adjustment; the
event generator correction remains a separate unchanged commit.

Final combined local results are 170 collaboration cases plus the two existing
ignored subprocess entrypoints, all 12 frozen migration fixtures, 6 native
caller/target cases, 82 SDK cases, 7 focused issue-view cases and all 230 desktop
cases. Workspace all-target Clippy with warnings denied, Rust formatting,
SDK/desktop/E2E TypeScript, SDK/desktop Biome and diff checks pass. Every shared
Cargo invocation holds the outer execution lock through both compilation and
test execution. Raw logs are `/tmp/gitru-r98-restack-collaboration-tests.log`,
`caller-tests.log`, `clippy.log`, `format.log`, `sdk-tests.log`, `issue-tests.log`,
`desktop-tests.log`, `sdk-types.log`, `desktop-types.log`, `sdk-lint.log` and
`desktop-lint.log`, each with the same `/tmp/gitru-r98-restack-` prefix. The
initial failed desktop run is retained separately as
`/tmp/gitru-r98-restack-desktop-initial-failure.log`.

Live preflight verified all 11 reported checks green on the old PR #152 head
`9046322`; those results qualify that old head only. No exact-head CodeQL result
was reported there. The restacked head is unpublished and needs its own remote
matrix after parent review/publication. Prior isolated native macOS QA above
remains evidence for its original build, not a new combined-build run. No new
packaged GUI, personal credential, live provider HTTP, push or merge is claimed
by this restack.
