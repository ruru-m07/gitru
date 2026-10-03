# RURU-79 — Inbox subjects

Status: implemented and locally qualified, ready for code review. Publication
and exact-head remote CI are tracked on RURU-79 and the attached review PR.
The attached isolated worktree is
`/Users/ruru/.codex/worktrees/collab-ruru-79/gitru`, branch
`ruru/ruru-79-inbox-subjects`. Initial parser planning began from signed RURU-78
`863967f`/PR151; integration now inherits R98/#152 `2d415938` and R96/#153
`0eb71a5` via signed merge `ab958df`. Prerequisites remain implemented unmerged
reviews. The integration/evidence records below distinguish local, native GUI,
packaged macOS and exact-head remote qualification.

## Live requirement and boundaries

Opening a supported provider notification reaches an account-scoped canonical
PR/issue with immediate cached details when saved. Missing/unsupported targets
keep a safe provider destination and useful typed explanation. Target hydration
accepts only validated provider resource coordinates. Permission/actor/grant loss,
offline/restart and private-account isolation are required. Opening performs no
mark-read, remote mutation, implicit repository selection or permission expansion.

## Initial source findings (before implementation)

The GitHub notification mapper retains repository immutable identity and type,
but currently discards subject.url and uses the repository web URL as a safe
fallback. Notification thread ID is not PR/issue ID. The existing resource
resolver/alias/store/detail machinery binds account/instance/repository/native ID,
number/kind and authorization view. A display title/type/URL alone cannot mint a
canonical native identity or relabel drafts. Existing Body hydration requires an
already trusted canonical subject and native ID.

## Contract questions to settle before code

- Store a bounded provider-neutral selector derived only from a strictly checked
notification subject path and trusted notification repository binding; raw remote
URLs must not become unrestricted HTTP authority. Old cached notifications stay
compatible through an absent selector, without editing prior migrations.
- Cache-only resolution can reuse a known canonical subject under current access,
account/instance/native repository/kind/number and unambiguous alias barriers. No
query creates HTTP/durable demand. Unsupported/missing/deleted/access-lost states
must remain distinct and preserve the authorized fallback.
- Decide safe missing-subject admission: reuse bounded selected repository
reconciliation or add a narrowly typed native subject-resolution request. Avoid
invented provider native-ID paths or assigning an unknown object ID from a URL.
A single-resource lookup must verify repository ownership and recheck dispatched
notification/selector/access at commit, respect quota/retry and publish only
trusted canonical identity/details. Identity discovery cannot overwrite list
membership/count truth or other actors' cache/drafts.
- Frontend resolution/subscription/rendering belongs the shared client/components;
manual resolution intent remains separate from local queries. A resolved subject
uses existing PR/issue detail UI and private draft CAS; notification reason/unread
stays on the original notification and no mark-read is sent.
- New DTOs/provider seams require native freeze then normal `make typegen`; no
handwritten generated changes. Coordinate any migration numbering with the
R96 sibling0006 rather than independently reusing it.

## Primary-source research

[GitHub notification docs](https://docs.github.com/en/rest/activity/notifications)
show subject type, API URL and repository context separately from thread ID and
latest-comment URL. The [documented issue endpoint](https://docs.github.com/en/rest/issues/issues#get-an-issue)
uses owner/repo/number and distinguishes PR representations. These facts justify
strict selector parsing and native identity verification; they do not establish
an undocumented immutable-ID subject lookup or arbitrary URL-following policy.

## Verification plan

Synthetic complete/missing/unsupported/malformed/wrong-origin/path/query/userinfo
selectors, same number across two accounts/repositories, issue-side PR markers,
path reuse/alias ambiguity, notification/epoch updates during resolution, denied
scope, quota/offline, late response and cold SQLite restart. Actual local query
zero-HTTP proof and resolved cached UI/fallback/private actor-draft routing tests.
Native/provider production qualification remains separate from sanitized fixtures
and exact-head remote CI. No personal credentials/vault/config may be inspected.

<!-- BEGIN RURU-79 native planning and research -->
## Native planning and research — proposal for approval

Planning-only source inspection on 3 October 2026. Root selected the narrow
current-inbox provenance direction below; the complete implementation contract
and ownership still need final review. No production code, schema, command,
generated binding, master architecture or Linear state was changed by this lane.

### Live dependency and overlap evidence

Live Linear RURU-79 is Backlog, with no attached PR or comments. Its blockers
RURU-76, RURU-77 and RURU-78 are In Review in #146, #150 and #151. It blocks
RURU-124 local disposition, RURU-127 GitLab todos and RURU-130 read/done delivery;
none of those remote-write or provider implementations is part of this slice.
The local base is exactly `863967f8dc55587c1f71061fd2267a1c139bed5e`, the current
#151 head. These are reviewed prerequisites, not merged/Done assumptions.

#151 already contains the shared Body/metadata contract and the issue adapter.
Open #152 RURU-98 is a sibling of #151, based on #150, and replaces scheduler
and automatic selected-detail code. RURU-96 is another active sibling with new
migration 0006. The RURU-79 worktree currently contains only this uncommitted note.
RURU-98 overlaps `runtime.rs`, `runtime/details.rs`, `storage.rs`,
`storage/details.rs`, client hooks/client/index, workspace and native command
registration/generation. RURU-96 overlaps `storage.rs`, client/hooks/workspace,
native command policy/registration and bindings. Use separate subject modules;
coordinate common wiring after their source freeze. Do not copy the old automatic
durable selected-detail hook from this base over RURU-98's leased consumer.
If the final branch includes RURU-96, this slice uses a new migration 0007;
root must settle ancestry/numbering before any SQL is authored. Never amend or
independently reuse 0006, or install 0007 and later try to apply 0006 underneath it.

### What the current source actually supports

- `providers/github.rs` keeps the notification thread ID and repository ID, but
  `GithubSubject` discards its API URL. `RemoteItem.provider_id` for a notification
  is the thread ID, not a PR/issue ID; `state` is the provider subject type.
- `storage/identities.rs::resolve_resource` is cache-only, retains contradictory
  aliases and checks access. Its repository-number lookup uses mutable path
  aliases. A notification needs an additional immutable-repository lookup.
- `DetailRequest` and GitHub detail normalizers require an already trusted native
  subject ID. They cannot bootstrap an unknown ID by assigning its number or URL.
- Item/detail visibility requires a selected parent. Contextual capabilities
  additionally inspect feed membership. An inbox-only repository is commonly
  unselected; adding an item row alone would still produce `NotObserved`.
- Retained notification rows can pass `Store.item` after feed retirement. They
  cannot serve as current provenance: the new gate must explicitly join active
  notification membership and its non-denied inbox scope.
- `apply_page` always updates feed membership, validation and absence evidence.
  A single-resource response must have its own writer transaction; a synthetic
  one-item page, even marked partial, would corrupt those independent facts.
- Existing transport maps 404 and 410 to the same `NotFound` category. This slice
  must say unavailable/not found or inaccessible, rather than claim deletion.

### Primary-source findings and limits

The [notification documentation](https://docs.github.com/en/rest/activity/notifications?apiVersion=2026-03-10)
separates a thread ID, subject type/API URL and repository context. The documented
example itself has different repository and subject paths: it is useful as a
negative fixture, not proof that such a mismatch is safe. The API URL and the
latest-comment URL are not interchangeable. Opening here issues no read/done
request and does not copy the web application's mark-read behavior.

The [GET issue contract](https://docs.github.com/en/rest/issues/issues?apiVersion=2026-03-10#get-an-issue)
documents owner/repository/number coordinates, transfer redirects, and that an
issue-side PR representation carries an issue ID. The [GET pull contract](https://docs.github.com/en/rest/pulls/pulls?apiVersion=2026-03-10#get-a-pull-request)
returns its PR ID and base repository. Those responses, rather than the path's
number, must supply immutable identities. Transfers to a different parent are
outside this bounded operation's trusted coordinates.

The official [pinned OpenAPI description](https://github.com/github/rest-api-description/blob/main/descriptions/api.github.com/api.github.com.2026-03-10.json)
was inspected as JSON. A pull response requires `base.repo`; an issue requires
`repository_url`, while the embedded `repository` object is optional. The native
repository API path appears in an issue URL example, but there is no documented
GET operation at `/repositories/{repository_id}` or its issue-number child in
that description. Do not synthesize those requests. A named parent URL plus a
separate repository GET cannot close a path-reuse race between the responses.
Accept direct issue discovery only when the same response supplies a matching
embedded repository ID or an exact validated native parent URL. Otherwise retain
an unresolved result with `identity_unverified`; this is a deliberate boundary,
not an assertion that all live issue responses have strong parent evidence.

### Options and recommendation

| Option | Benefits | Practical limits |
| --- | --- | --- |
| Existing selected-repository reconciliation only | Reuses the current adapter and queue; minimal native change | Requires an explicit selection for inbox-only repos; bounded traversal may not reach a closed/off-page target quickly; cannot promise point discovery |
| Narrow notification point discovery with current provenance — recommended | Immediate saved navigation; bounded GET-only work for a missing target; no repository selection or false list coverage | Requires a new provenance/storage gate and strict parent evidence; unverifiable issue responses remain unresolved |
| GraphQL atomic repository/resource lookup | Can return immutable repository and child IDs in one response | Adds a new transport, budget/error/permission contract and mapping; defer rather than introduce them incidentally in RURU-79 |

The recommended slice delivers local navigation and safe bounded discovery;
it does not promise discovery where the REST response cannot prove its parent.
An existing authorized selected feed can still discover an unresolved resource
later. That background observation wakes the same local resolver, without
changing the notification's read state.

### Selector, local query and result contract

Introduce a separate `notification_subjects` domain/storage module rather than
adding arbitrary remote URLs to frontend request DTOs. Names below are proposed
freeze names, not generated API claims:

- `NotificationSubjectSelector`: finite PR/issue kind, provider-native repository
  ID, positive scoped display number, bounded validated repository path and
  adapter-owned representation namespace. The number is not a native resource ID.
  Unknown provider subject types and missing/invalid paths have typed reasons.
- `NotificationSubjectObservation`: native-only notification ID + normalized
  selector/missingness, carried with its notifications page. Store atomically
  only with an accepted page/account/epoch/run and the same notification row.
  An unsupported/malformed subject is safe per-item fallback, not an arbitrary
  HTTP request or a whole-inbox failure. Never retain raw credential/query URLs.
- `NotificationSubjectQuery`: account ID, inspected authorization epoch and
  notification ID. `Store.notification_subject` reads one SQLite snapshot;
  it never writes an intent, loads a credential or calls an adapter.
- `NotificationSubjectSnapshot`: revision, authorization view, account epoch,
  typed resolution state/reason, opaque selector generation, optional canonical
  resource reference, authorized safe web fallback, and discovery sync status.
  Distinguish resolved, not cached, unsupported/no selector, ambiguous,
  unavailable and unverified identity. Denied/inactive cases expose no private
  title, selector, repository/resource IDs or fallback URL.
- `DiscoverNotificationSubjectRequest`: account ID, inspected epoch,
  notification ID and inspected opaque selector generation only. It accepts no
  URL, repository path, number, kind or provider ID from TypeScript.

GitHub selector parsing is provider-owned: only `Issue`/`PullRequest` with the
corresponding exact issues/pulls resource path, configured API origin, no
userinfo/query/fragment, no raw escapes/dot segments/control characters, and
positive exact integer number. Named paths must match the repository in the
same notification observation; native paths must match its exact repository ID.
`latest_comment_url`, comment/release/security/check paths and unrelated parent
paths never become navigation authority. Unknown types remain supported inbox
rows with typed unsupported-subject fallback. Build the public GitHub web
fallback from validated coordinates, or use a validated same-instance repository
destination; never open a raw API URL as if it were a provider page.

Local resolution binds account + stored instance + immutable repository ID +
kind + number. Count **all** matching canonical/representation claims, including
currently hidden claims, before selecting an accessible candidate. Do not pick
the first visible identity, cross accounts/instances, or select an old mutable
path alias after path reuse. Issue-side PR markers may attach an endpoint alias
only to a verified PR; a true issue is never relabeled by number alone. Existing
canonical IDs and draft CAS generations remain unchanged.

### Current-provenance eligibility, not a persisted permission grant

Add one shared metadata-only transaction accessor for subject eligibility and
reuse it in item reads, detail reads/evidence, native admission/commit and
contextual capabilities. Ordinary selected-repository eligibility stays intact.
The narrow alternative is an exact canonical subject matching a **current**
selector under the same account/instance/repository/kind/number, current epoch,
active inbox membership and a non-denied inbox. Any explicit parent discovery,
resource/feed or detail-facet denial wins. Hidden contradictory claims stay
ambiguous and cannot grant access. Provider support remains separately typed.

This alternative grants access only to that known notified subject; it never
sets `repositories.selected`, manufactures feed `scope_membership`, changes
repo/list/search/count visibility, or enables a whole-repository refresh. A
point-created canonical row can support detail without pretending it was seen
in a list. Contextual resource evidence must recognize this provenance rather
than fabricate complete list coverage. The Body observation retains the existing
R97/R77 authority, freshness and missingness semantics.

Eligibility is derived on every read/dispatch/commit. A retained selector or
canonical row alone is insufficient. Notification retirement, selector change,
account replacement, inbox denial, parent denial or proof contradiction withdraw
subject-only eligibility. Invalidate affected detail/discovery runs, stop their
pending intent and advance authorization-view/reset evidence when an effective
read grant is withdrawn, in the same writer transaction. Old responses cannot
restore it. Ordinary selected eligibility can independently remain valid.
Saved body/metadata and authored drafts survive eligibility withdrawal; reads
hide provider content. Offline cannot detect a remote revocation it has not yet
observed, but local known denials and retired memberships still apply on restart.

Private text routes to the actual canonical subject when resolved. Never rename,
merge or move an existing notification-thread draft into a subject draft. Those
older drafts remain durable/recoverable. Same-actor refresh preserves the editor's
inspected CAS generation; actor/subject changes clear its transient buffer using
the existing editor lifecycle. Provider resets cannot replace private text.

### Explicit discovery and atomic publication

Add a default-unsupported provider seam such as
`discover_notification_subject(token, trusted_request)` returning a verified
canonical summary plus one Body/typed-metadata observation, or a typed unresolved
reason. Its request is assembled from native notification/repository evidence;
it is not a general locator fetch. Adapter support must be explicitly declared
and gated independently for PR and issue subjects. Live GitLab/Bitbucket inboxes
and their mapping remain outside this slice.

For GitHub PRs, one GET returns the PR ID and exact `base.repo.id` parent proof.
Verify number, kind, API/web routes and parent before constructing the normal
`DetailRequest`; then reuse the existing normalizer on that same response. For
issues, require inline repository ID or exact native parent URL proof before
using its native issue ID. Named-only responses produce `identity_unverified`.
If an issue endpoint explicitly returns a PR representation, never create an
Issue row from its ID: either a verified pull GET establishes the canonical PR
and retains the issue endpoint alias, or return a typed representation mismatch.
No number-only type conversion or guessed Base64/node/native IDs is allowed.

Discovery is explicit, coalesced by account/epoch/notification/selector generation,
and uses the one native worker and existing persisted `provider:rest` budget.
Persist bounded rebuildable read intent before queue admission; never create an
outbox/remote-write command. Proposed bounds: 16 pending discovery intents per
account, 64 total, at most three provider attempts per explicit generation,
each at most two resource GETs; existing three-redirect/4 MiB transport limits
apply to each GET. Transient/offline/quota retries use persisted strict deadlines
and bounded backoff. Admission/coalescing/visibility never clears a quota, denial
or retry barrier. After the attempt budget, display an error/manual retry instead
of an automatic loop. Unsupported, unverified, not-found or denied results do
not autonomously repeat. A new explicit retry may start a new bounded generation.
Restart re-admits only still-current pending intent, preserving attempts/deadline;
close does not revoke an already accepted explicit read, and no new view interest
is inferred from it. No row-driven or hidden per-panel timer is introduced.

New commands use the existing trusted local collaboration caller policy in main
and managed child views. Check the inspected account epoch, selector generation
and current provenance before persistence, after writer acquisition and before
commit; retire replaced/native foreign callers before accepting late IPC. Root
owns Tauri command policy/registration. Credential import/account management
remain main-only. The runtime alone captures a native vault reference/token after
capability and current provenance checks; neither SQLite nor IPC contains secrets.

Dispatch captures account/epoch/instance, authorization view, notification selector
generation/current membership, repository immutable ID and expected canonical
claims. The writer rechecks all of them and any current summary head/source
ordering before committing. Do not re-read a changed subject mid-request and
pretend the receipt was dispatched for that new subject. Uncached discovery
sends no conditional validator; an unsolicited 304 cannot establish identity,
empty body or metadata. Known subject refresh uses the existing detail seam and
its comparable-source/last-observed-known mask rules.

The point transaction inserts/retains canonical identity and aliases, bounded
summary, Body + metadata/source/validator and current provenance together, then
publishes revision hints. Reuse transaction-local detail merge helpers; do not
fork their per-field saved-value/order barriers or silently freshen omitted data.
Do not overwrite newer list head/state metadata with an older point response.
Do not call `apply_page`, `seen`, reset feed denial/coverage, advance list cursors,
reconcile absence, or assert fresh list counts. Errors preserve bodies/drafts;
404/410 are unavailable evidence, not deletion or permission expansion.

### SDK and UI contract

Expose account-bound local `notificationSubject` query/options and explicit
`discoverNotificationSubject` action. Capture account/actor/epoch/notification
and selector-generation primitives when constructing a handle/request; external
mutation of the account object cannot retarget a delayed action. Query keys bind
account epoch and notification ID. The existing bridge handles notifications,
subject-discovery, parent and detail changes: cancel affected pending reads
**before** invalidating/refetching, including initial no-data reads. Withdrawal
removes provider projections without touching private draft keys/forms.

Opening a notification first reads the local resolver. A resolved result mounts
the shared PR/issue detail/header/private-editor path for its canonical ID while
keeping reason/unread display on the original notification. Saved content is
shown without waiting for HTTP. Missing/unverified/unsupported states show the
typed explanation, authorized safe provider action, and explicit load/retry only
when allowed. No mark-read write, implicit selection, whole-repo fallback or
provider-name branching in UI. A late resolver/point receipt after account,
epoch, notification or subject change cannot mount the prior subject.

When R98 is integrated, existing selected-known-subject Body freshness uses its
visible native lease; do not reinstate the superseded automatic durable hook.
Missing identity discovery remains an explicit finite action. This slice does
not add automatic row prefetch, continuous discovery leases or conversation,
review, check, diff, snooze/read/done behaviors.

### Staged ownership and meaningful validation

After root approves this complete note, native owner may author new
`src/notification_subjects.rs`, `storage/notification_subjects.rs`,
`runtime/notification_subjects.rs` and a GitHub subject parser/discovery module,
new focused tests/fixtures and the approved forward migration. Common edits are
limited to page observation wiring, lifecycle cache cleanup, shared subject
eligibility/detail transaction helper, provider profile/seam and job dispatch.
R98 scheduler ancestry must be fixed before editing its common dispatch paths.

Root owns Tauri command/caller registration and normal `make typegen` once native
DTOs freeze. SDK/UI owner then adds native transport + account query/action +
bridge hooks + notification navigation and real QueryObserver/UI regressions;
do not hand-edit generated bindings. Independent native review owns a new test
file only after contract freeze. Main architecture/backlog/Linear/PR/publication
remain root-owned. Keep this note's later implementation evidence distinct from
the planning proposal.

Required acceptance tests use sanitized fixtures and synthetic adapters:

- Strict subject parser positives and wrong parent/origin/port/userinfo/query,
  fragment/escape/dot/comment/unknown/null/malformed cases, exact IDs above 2^53;
  the inconsistent official notification example is a negative case.
- Local resolver zero HTTP/vault/durable admission; same number across accounts,
  native repositories, kinds and instances; hidden all-claim ambiguity, path
  rename/reuse and issue-side-first PR alias conservation.
- Unselected current-inbox subject reads item/Body/context without selection or
  list membership; parent/feed/detail/inbox denial wins. Retired retained inbox
  row is not provenance. Withdrawal hides saved provider content while preserving
  draft text/CAS and saved bodies; an independently selected subject still uses
  ordinary eligibility. Restart/offline exercise the same gates.
- Verified PR and strongly parent-proven issue GET bootstrap atomically; named
  issue parent only remains unresolved; PR marker does not mint an Issue; wrong
  response ID/number/parent/path/head/source and unsolicited 304 are rejected.
- Feed membership/coverage/cursor/selection/count bytes remain invariant after
  point lookup; body null/empty/omitted/oversized and retained metadata clocks keep
  the existing source authority; transaction interruption leaves no half binding.
- Notification selector update/retirement, authorization-view change, epoch
  replacement and proof contradiction during delayed HTTP reject stale commits;
  old caller/old account handle admission creates zero intent/provider calls.
- Deterministic coalescing, queue/global/account bounds, restart attempt/deadline,
  offline retry budget, strict future quota zero dispatch and explicit retry tests.
- Real QueryObserver initial-pending cancellation, actor/epoch/notification switch,
  safe unsupported fallback, immediate cached shared detail/private draft routing
  and no mark-read or implicit selection calls. Platform/remote CI and any live
  provider qualification are reported separately from these local fixtures.

<!-- END RURU-79 native planning and research -->

## Root approval and implementation sequencing

Root reviewed the complete native proposal on 3 October 2026 and approves the
narrow current-inbox provenance contract and bounded explicit discovery. Named
issue parent URLs cannot establish immutable parent identity; unverifiable issue
responses keep the typed safe fallback. Notification retirement/denial and all
identity claims participate in current access, rather than granting access from
a retained row. No implicit selection, feed-membership mutation or mark-read
operation is accepted.

Stage one starts only provider-neutral selector/domain definitions, strict
GitHub normalization and synthetic parser tests in isolated source modules.
No SQL, persistence, scheduler, existing access gate, IPC or UI changes begin
until root records the final RURU-96/RURU-98 ancestry and migration number. This
allows useful independent work without implementing against superseded shared
seams. Subsequent native storage/runtime remains the assigned native owner's
lane; root owns generated/caller/integration/publication and the frontend owner
waits for native DTO freeze. The issue is In Progress, with the prerequisites
implemented in unmerged review PRs; this is not completion or live provider QA.


<!-- BEGIN RURU-79 stage-one implementation -->
## Native implementation — stage one

Implemented the approved selector and strict parser stage on the unchanged
RURU-78 base `863967f`. Subsequent access/provenance/persistence/runtime work is
still gated on root's final RURU-96/RURU-98 ancestry and migration decision.

`src/notification_subjects.rs` defines finite PR/issue kinds, the bounded selector
shape, a closed adapter representation enum, tagged selector/fallback results
and a native-only notification observation. Repository IDs and display numbers
remain decimal strings, including values beyond JavaScript's integer precision.
The selector has no native subject ID or raw URL; the notification thread ID
remains separate. Future adapters add typed representation variants rather than
arbitrary strings that could become HTTP authority. No IPC result or command
shape is frozen or generated by this stage.

`providers/github/notification_subjects.rs::normalize` accepts only a trusted
native HTTPS API base, the repository captured in the same notification and the
raw subject JSON. It recognizes exact `Issue`/`PullRequest` types and matching
issues/pulls routes. Named parents must match that observation's repository path;
native parents must match its immutable repository ID. API origin, configured
port and segment-bounded API prefix must match. The parser rejects raw escapes,
dot/empty segments, backslashes, whitespace/control characters, userinfo, query
and fragments before URL parsing can normalize them away. Positive canonical
u64 decimal IDs/numbers retain their exact strings; no leading-zero/sign/float
coercion is accepted. An explicitly configured default HTTPS port is equivalent
to its implicit default origin.

Native-route recognition records a parser fact only. It does not establish that
an immutable-ID GET endpoint is documented, prove a returned issue's parent, or
allow following the supplied URL. Later discovery must construct its accepted
named endpoint and verify authoritative response identity as approved above.
Missing, unknown or invalid inputs produce one finite fallback reason and retain
no raw URL, secret, title or comment path. `latest_comment_url` is ignored. Existing
notification page mapping and repository-web fallback remain unchanged here.

Executed local evidence, using the shared outer Cargo lock across build and test
execution on this worktree:

- **11 focused synthetic parser cases pass**:
  `/tmp/gitru-r79-stage1-parser-tests.log`.
- **165 collaboration cases pass**, plus two ignored subprocess entrypoints
  exercised by the existing parent crash tests:
  `/tmp/gitru-r79-stage1-native-tests.log`.
- Collaboration all-target Clippy with `-D warnings` passes:
  `/tmp/gitru-r79-stage1-clippy.log`.
- Workspace formatting and `git diff --check` pass:
  `/tmp/gitru-r79-stage1-fmt-check.log`.

The fixture and cases cover named/native issue and PR routes, maximum u64 and
IDs above 2^53, exact configured origin/port/base path, path normalization attacks,
wrong parents, rename/path reuse, unsupported/missing/null/malformed values,
kind/endpoint mismatches, ignored comment URL, byte bounds and secret-free
serialized fallback. The official documentation's inconsistent parent/subject
example is a negative fixture. Configured enterprise-looking parser coordinates
are synthetic protocol tests, not enterprise adapter or live provider support.
The independent native read-only review accepted the final domain/parser and
test boundaries with no material semantic mismatch; it performed no duplicate
builds or source edits.

Only the two new source modules, their focused test/fixture and minimal module
exports are implemented. No SQL, existing migration, page observation wiring,
cache query, permission gate, runtime/scheduler, command, generated binding or UI
behavior changed. No personal credential, CLI, vault, app database or provider
HTTP was accessed. Root owns the later stack integration and remote CI; this
stage does not complete RURU-79 or qualify live notifications.
<!-- END RURU-79 stage-one implementation -->


## Root integration checkpoint and stage-two approval — 3 October 2026

Stage one was signed as `e20c088` on R78, then replayed with only the module
export union onto signed R96 `c435af8ba3666b6f558d2938d6d15c2d7996e123`/draft
PR153. Current signed stage-one head is `364a9e3794b2bb41ee5d41b4e9f6ebf6fedc701f`.
R96 actually contains migration0006 and inherits signed R98 `2d415938`/PR152,
which inherits R78 `863967f`/PR151. Root verifies that ancestry and clean tree;
these remain open unmerged reviews, not Done prerequisites.

Root approves the native owner to implement the complete accepted stage-two
storage/provenance/provider-discovery/scheduler contract using **forward0007**.
Do not edit migrations0001–0006, broaden list membership/selection, weaken any
native lifetime/authorization/source gate, or reinstate automatic durable detail
selection. Preserve the R96 authored-link and R98 physical/activity lease seams.
Native owner owns collaboration domain/provider/storage/runtime/new tests and
this note's distinctly marked native evidence only; root owns Tauri caller,
registration, generation, master documents, Linear, commits and publication.
Frontend owner owns SDK/UI only after native DTO freeze. The independent reviewer
may add one separate test file after the contract freezes, with no overlapping
source ownership.

Final consumer review requires the native snapshot to carry discovery admission,
support/access and retry status rather than the renderer inferring permission
from selector presence. One shared current-inbox provenance accessor must govern
item/detail/context and demand admission. Notification withdrawal must invalidate
pending detail/resolver reads even if saved body bytes do not change. The SDK
cancels initial pending reads before invalidation, uses immutable account/epoch
bindings, and reuses its existing deadline coordinator. Same-actor withdrawal
hides provider data while preserving the prior verified canonical draft CAS;
actor/notification/canonical identity changes choose a distinct editor. Retained
thread drafts remain separate. Full acceptance and safe unverifiable-issue
fallback remain as approved above; no live provider or personal credential QA.

<!-- BEGIN RURU-79 frontend and SDK integration -->
## Frontend and SDK integration

The accepted renderer snapshot and explicit discovery request remain native
authority. The SDK adds account/actor/epoch/notification-bound local keys and
immutable request bindings, query options and typed transport methods. The
existing bridge cancels pending resolver/detail reads before notification,
subject-discovery or parent invalidation. Author drafts keep their separate CAS
and lifecycle; this work preserves the R96 local-link and R98 demand APIs.

Notification selection mounts a local resolver wrapper inside the inbox. Current
authorized notification reason/unread stay separate from canonical PR/issue
metadata. A resolved subject reuses the existing saved resource pane and Body
visibility lease; missing identity discovery is exclusively an explicit button
action. Native admission accepts a finite intent even while dispatch is paused,
without clearing quotas. No automatic discovery timer, provider-name branch,
mark-read, repository selection or false list membership is introduced.

The shared pane keeps only its prior verified subject binding and private draft
mounted during same-actor resolver/authorization refresh; provider header/body
are hidden while authority is unavailable. Actor, notification or canonical
identity switches replace the editor. Existing notification-thread drafts stay
separate and are never renamed or copied into canonical drafts. R99's sibling
recovery/editor reconciliation remains a separate root integration gate.

The pure SDK notification suite passes **9 tests**, with **41 combined** existing
client/local-link/resolver tests at `/tmp/gitru-ruru79-sdk-client-regressions.log`.
They include real QueryObserver initial-pending
resolver/detail cancellation, immutable actor/epoch/selector requests and stale
epoch rejection before revision or authorization-view metadata is accepted.
Authored cache survival and delayed withdrawal receipts are independently covered.
The native peer found and the SDK corrected the `provider:rest` resolver dependency;
both pending and cached quota-status observers reread while preserving explicit
admission and issuing no discovery or content requests. The unchanged shared
workspace/cached PR/cached issue regression suites pass **37 tests** at
`/tmp/gitru-ruru79-shared-pane-regressions.log`.
Owned Biome checks pass; the frozen Bun copyfile install changed no dependency
declaration or lockfile.

The renderer regression suite passes **16 cases** for immediate saved null/empty
and Unicode content, unselected current-inbox views, explicit paused discovery,
unsupported/ambiguous/unverified fallback, bounded terminal manual retry, late
receipts, actor/notification/selector switches, actual bridge withdrawal, denied
Body metadata, separate thread drafts and private inspected-generation retention.
Four generated wire cases pass against root's normal **112-command generation**,
covering native string identities, nulls, all resolution states and authority-free
explicit requests. The initial UI run caught only a test expectation for the
existing provider button label; changing the expectation to “Open on provider”
left production behavior unchanged. No generated file is edited here.

Final local frontend checks on the generated source all pass:

- **106 SDK tests**, 12 files: `/tmp/gitru-ruru79-sdk-tests.log`.
- **283 desktop tests**, 33 files: `/tmp/gitru-ruru79-desktop-tests.log`.
- **16 new notification UI tests**: `/tmp/gitru-ruru79-notification-ui.log`.
- **13 new resolver/wire tests**: `/tmp/gitru-ruru79-notification-sdk.log`.
- SDK types: `/tmp/gitru-ruru79-sdk-types.log`; desktop and E2E types:
  `/tmp/gitru-ruru79-desktop-types.log`.
- Owned SDK/UI Biome: `/tmp/gitru-ruru79-sdk-ui-lint.log`; production frontend
  build: `/tmp/gitru-ruru79-ui-build.log` (existing Vite large-chunk warnings).

The independent native peer accepted the consumer fences and corrected quota
dependency read-only. These are mocked local/native-command and jsdom assertions,
not live-provider or packaged-window qualification. Native provenance, caller
policy, Rust checks and native GUI QA remain root/native ownership. R99's
save/copy/export recovery editor is an unmerged sibling and is not claimed here.
<!-- END RURU-79 frontend and SDK integration -->

<!-- BEGIN RURU-79 native stage two -->
## Native implementation — stage two

Forward migration `0007_notification_subjects.sql` stores accepted inbox
observations and finite explicit discovery intents. Migrations 0001–0006 and
their frozen fixtures remain unchanged. An older notification representation
cannot replace a newer selector; the shared page transaction accepts the item
and selector together. Active repository-less legacy notifications remain valid
inbox rows and resolve to `Unsupported`/`MissingSelector` without discovery
authority. Denied or inactive notifications hide coordinates, fallback and the
selector generation.

The transaction-local current-inbox predicate gates canonical item reads,
Body/detail queries, contextual capabilities, visibility-demand admission,
point dispatch and publication. It requires current inbox membership, immutable
account/instance/parent coordinates and the relevant access gates. Hidden
canonical competitors participate in resolution. One immutable Native alias
that identifies different definite canonical targets also prevents a new inbox
grant, including a collision introduced by the point response's own endpoint
aliases. Multiple distinct representation aliases for one canonical target
remain valid; mutable URL/path aliases do not create immutable conflicts.
Supporting indices bound the alias checks to the candidate's representations.

This preserves the inherited ordinary selected-cache policy: repository
discovery absence withdraws an unselected notification grant, while an
independently selected canonical subject keeps its existing cached eligibility.
Actual discovery denial hides both. Withdrawal uses that same ordinary predicate
before fencing detail runs and the authorization view. Retained provider rows,
bodies and private draft IDs/CAS generations remain intact. Point reads never
change selection, list membership, seen markers, coverage, cursors, counts or
full-text indexing; thread drafts are never renamed into canonical drafts.

Discovery is an explicit GET-only intent. The cache query loads neither a vault
credential nor a provider response. Admission coalesces the same current intent,
enforces 16 pending intents per account and 64 globally, and reserves at most
three attempts durably across restart. Reservations include abandoned attempts
before HTTP, so interruption cannot authorize a fourth request. Snapshot
admission describes support and current authority; queue capacity is enforced
at action time with `Busy`. An explicit new intent can be admitted while paused
but cannot clear account/scope cooldowns or a known permission denial. Terminal
transient exhaustion requires another explicit user action; there is no
continuous discovery lease, automatic lookup loop or mark-read operation.

The GitHub adapter dispatches only the accepted named PR/issue GET route. Exact
origin/path, raw redirect spelling and a four-request redirect bound are
enforced without conditional or pagination requests. Immutable parent proof
comes from the same response: PR base repository identity, or a strongly proven
issue parent. A named-only issue parent remains `IdentityUnverified`; an
issue-side PR marker remains a typed representation mismatch without a second
GET. Body and typed metadata use the existing shared normalization and detail
transaction helper. Identity, summary, aliases, Body and metadata publish
atomically under account, instance, epoch, authorization-view, selector,
intent/run and canonical-source fences. Existing canonical IDs, retained clocks,
observed masks and metadata budgets remain authoritative.

The worker rechecks current authority and the maximum persisted provider/scope
deadline after awaited credential access and before dispatch. Point errors use
the captured lease, so a late 403/404 or transient failure cannot modify a
replacement selector. Successful response quota evidence survives unresolved,
malformed and oversized responses. There is one deliberate narrow exception to
selector fencing: a response consumed under the same authorization epoch may
still extend account-wide quota metadata after its selector becomes stale. All
subject, denial, error and intent effects remain rejected; a replacement
authorization epoch rejects the old quota observation too. The finite runtime
tests exercise this distinction explicitly.

Final native validation uses synthetic accounts/providers/vaults and isolated
temporary databases:

- **249 collaboration tests pass**, with two ignored subprocess entrypoints;
  `/tmp/gitru-ruru79-native-final.log`. This includes all **12 frozen-v1
  migration/recovery cases** against actual migration 0007, the inherited
  credential crash checkpoints and unchanged storage disappearance tests.
- **50 stage-two cases** cover 16 real transport/adapter fixtures, 11 deterministic
  worker scenarios and 23 independent public storage/access cases. Final peer
  logs are `/tmp/gitru-ruru79-discovery-adapter-final.log` and
  `/tmp/gitru-ruru79-independent-access-23.log`.
- Independent access tests demonstrate real writer-wait caller retirement,
  atomic rollback, same-result representation collision, selected versus
  unselected withdrawal, draft continuity and no feed mutation. The older
  selector regression failed before its source fix. The immutable-alias case
  also has a preserved executed red result at
  `/tmp/gitru-ruru79-representation-red.log`; its primary `Ambiguous` assertion
  passes unchanged after the shared guard. The generic alias diagnostic now
  accepts its existing access-filtered `Unavailable` result as well.
- Full workspace Clippy with warnings denied passes at
  `/tmp/gitru-ruru79-workspace-clippy.log`; independent owned native Clippy passes
  at `/tmp/gitru-ruru79-independent-final-clippy.log`. Formatting is applied with
  `cargo fmt --all`; the final check passes at
  `/tmp/gitru-ruru79-native-format-final.log`. `git diff --check` also passes.

These are local native results. Parent-owned Tauri caller/generated bindings,
SDK/UI, fresh packaged-window QA and exact-head remote CI remain distinct
integration evidence. No personal account, live provider API or vault was
qualified. R106's restore schema ceiling and the unmerged R99 editor-recovery
sibling remain unchanged.
<!-- END RURU-79 native stage two -->


<!-- root final integration and native QA evidence: 2026-10-03 -->
### Root final integration and isolated native qualification

Final native source is formatting-frozen with249 collaboration tests (two
pre-existing ignored subprocess helpers),12 frozen-v1 cases,15 complete desktop
native tests (11 collaboration caller/lifetime plus four updater), full workspace
all-target Clippy and formatting passing. SDK106/12files, desktop287/33files,
focused notificationUI20 and resolver/generated-wire13 pass. SDK/desktop/E2E
types, owned lint and production frontend builds pass. Root source pipeline lint
passes. Normal112-command `make typegen` regenerates IPC after final formatting;
a named-schema comparison against inherited `ab958df` confirms every existing
schema retains exact normalized semantics, with eight new command/domain schemas.
No generated file was hand-edited.

The reviewed authority corrections include an older observation replacing a
newer selector (reproduced red→green), same immutable Native representation key
claiming two distinct canonical targets (including a hidden competitor), and
same-point alias insertion causing atomic rollback. Several distinct issue IDs
naming one canonical PR remain resolved; mutable renamed paths do not contradict
immutable coordinates. Existing ordinary selected-cache reads survive discovery
absence, as required by the unchanged inherited storage regression; actual
parent/feed denial and retirement of the narrow unselected inbox grant remain
separate. The approved same-epoch consumed-quota-only exception has a worker
regression; old epochs and stale identity/denial/intent receipts cannot publish.

Root used real native WKWebView UI in dedicated task-owned apps, compiled with
the e2e feature (vault/CLI disabled) and production frontend. No personal Git
configuration, credentials, keychain or provider account was inspected. Initial
QA exercised both synthetic actors, cached PR/issue descriptions, UTF-8 metadata,
canonical draft CAS and separate thread draft text. An actual UX gap—no visible
receipt after accepting a paused read—was corrected in the notification view,
with four semantic UI cases, localized retry dates and existing late-receipt
actor/thread/selector suppression intact.

Because unpublished0007 gained two lookup indexes before publication, final QA
uses a fresh `com.ruru.gitru.ruru79.qa.final` installation; the earlier synthetic
installation/evidence is preserved. It does not rewrite an old migration checksum.
Frozen final executable SHA256:
`58340e32ecfbfb0fa476d946565777d6d47450fa584a307a14b263ae02d87c07`.
Task LaunchServices Git config/XDG paths are empty fixtures. Final real UI checks
cached PR67 and Issue68 from unselected parents, known authoritative Unicode Body
and metadata, original reason/unread, actor-B isolation, safe unsupported fallback,
localized strict2099 quota and visible acknowledgement of an explicit missing-PR
request. A cold Quit/relaunch restores cached inbox and generation2 canonical draft.

Final post-Quit read-only database evidence is at
`/tmp/gitru-ruru79-qa/final-post-quit-evidence.json`: six distinct authored drafts
(one saved canonical draft generation2, all other generations1), eight unread
notifications, both parent selections0, one durable explicit discovery intent
requested1/attempts0/runNULL, strict2099 provider barriers, zero credentials,
cleanup and automatic durable detail intents, migration versions1..7. No remote
write or implicit repository selection occurred. Adapter proof/HTTP worker tests
are separate from synthetic GUI checks; this does not qualify live PAT/provider,
OS vault, Windows power loss or exact-head remote security/CI.

Logs: `/tmp/gitru-ruru79-{native-final,caller-tests,workspace-clippy,typegen-final,
sdk-types-final,desktop-types-final,bindings-lint}.log`, agent-owned SDK/UI/native
logs above, `/tmp/gitru-ruru79-qa-build-final.log`, and task-only prepare/evidence.
Fresh packaged macOS E2E passes all three cases across two specs, including
real UI→Tauri→Rust→Git repository import/branch/stage/commit flows and both
collaboration storage/host-dialog flows. No timeout, fixture order or inherited
assertion was relaxed. Log `/tmp/gitru-ruru79-packaged-e2e.log`; artifacts are in
the task worktree's ignored `artifacts/e2e/` directory. Signed publication and exact-head remote CI are tracked on RURU-79 and the
attached PR; local success alone does not qualify remote CI or live accounts.
