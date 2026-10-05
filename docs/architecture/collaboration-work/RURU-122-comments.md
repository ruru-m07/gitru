# RURU-122 — Cached conversation comments, first bounded slice

Status: first Comments slice implemented and locally qualified, 5 October2026.
Read the shared engine/backlog first; activity/provider expansion is still open.
Isolated managed `ruru/ruru-122-conversation-comments` starts from attached
Tasks draft#163 exact2e4b8d5c57e5dd0bb7dd0cbef5e4f521a870e588. At the pre-code audit, live R122 was
Backlog with no attachment/duplicate; R77/R78 prerequisites were In Review and
implemented in ancestry. Current collaboration PRs have no reported failed
checks; #163's exact remote matrix is running, distinct from local qualification.
Primary untracked architecture work and unrelated PRs remain untouched. This
contract was reviewed and signed as5856378 before source changes. R122 is now
In Progress; source publication and its new-head remote CI remain separate.

## Scope and current primary evidence

Read-only GitHub.com conversation comments for PRs and issues through existing
Comments facet, native HTTP/Runtime/SQLite, generated local query/subscription SDK
and a common collapsed panel. Accounts remain independent of Gitru cloud; no
ambient credentials, remote TypeScript HTTP, comment writes, inline review threads,
activity timeline, enterprise host or schema migration. R122 stays In Progress
until its later activity/provider criteria are implemented; no merge is authorized.

Fresh official review:

- [Versioned comment endpoint](https://docs.github.com/en/rest/issues/comments?apiVersion=2026-03-10#list-issue-comments)
  covers PR and issue conversation comments in ascending numeric-ID order; inline
  review comments are separate. Page size is bounded100; either Issues or Pull
  requests read permission can authorize fine-grained tokens.
- [Official current OpenAPI](https://raw.githubusercontent.com/github/rest-api-description/main/descriptions/api.github.com/api.github.com.2026-03-10.json)
  requires id, issue_url, updated_at and nullable user. Body is an optional string,
  not declared nullable: missing is Omitted, empty is Known, explicit null invalid.
- [Pagination](https://docs.github.com/en/rest/using-the-rest-api/using-pagination-in-the-rest-api)
  shows immutable repository-ID child links. Versioned OpenAPI declares named
  routes; extending that immutable prefix to comments is an inference, supported
  by a finite anonymous/no-Authorization public request on5October returning200
  for `/repositories/274190073/issues/6344/comments?per_page=50`. This confirms
  public routing only; private/PAT/null-policy qualification remains separate.
- [Conditional/caching guidance](https://docs.github.com/en/rest/using-the-rest-api/best-practices-for-using-the-rest-api)
  warns page contents shift on insert/delete. v1 uses no ETag/304 or since filter;
  stable local saved reads and bounded native demand work independently of network.
  Conditional and incremental optimization must preserve collection authority.

## Identity, representation and bounds

Use the pooled native GitHub transport/API2026-03-10 and application/vnd.github+json
(default raw body). Request only fixed immutable
`/repositories/{repository.provider_id}/issues/{subject.number}/comments?per_page=50`.
PRs use the issues hierarchy too. Validate positive numeric native repository and
subject IDs/number, account/provider/host and exact repository/subject bindings.
An immutable request avoids attributing a reused owner/name to an old repository.
A returned issue_url must be HTTPS api.github.com without query/fragment/userinfo,
matching captured named `/repos/{full_name}/issues/{number}` or immutable parent.
A named parent changed by rename is refused until repository metadata catches up;
it cannot authorize a different repository. Do not derive native identity from URLs.

Every comment requires positive u64 id, valid updated_at RFC3339 <=128 bytes and
required nullable user. Nonnull user requires positive id and nonempty bounded
<=255-byte/control-free login; save only presentation in generic Author. Other
actor/created_at/link/association/minimization fields are ignored by this slice,
not overloaded into title/state or interpreted as deleted status. Requiredness of
ignored fields does not grant local authority. Body strings <=65,536 bytes are
Known, larger strings become Oversized with no incoming text. Missing body is
Omitted; null/nonstring invalid. Reject invalid rows/duplicate IDs/arrays above50
or malformed envelopes atomically. Reuse4MiB/20-second/sensitive Bearer transport.

Local id `github-comment:{id:020}` preserves numeric ordering in existing
lexicographic SQLite keysets; provider_id is the canonical numeric string. ID is
adapter-owned within the account/subject/facet partition. Edits cannot reorder
conversation history by updated_at. No fabricated timestamp from parent/max-row
or action date. Generic title/state/head_oid/native remain absent; field mask is
Body/Author/UpdatedAt, source `github/comments/2026-03-10`, adapter1, freshness180s,
collection provider_updated_at=None. Engine receipt time determines freshness.
Always include Body observation even omitted/oversized; Store's existing
observed_body_state records latest state while retaining known text/old per-field
validation/clock. Known nullable user establishes absent Author. Incoming per-row
updated_at orders only that comment's fields; older replies cannot erase edits.

## HTTP collection boundary, continuation and completeness

Add an opt-in private collection request mode to existing GithubHttp. It refuses
redirects (including query-changing redirects), sends no validators and rejects304.
Standard feeds, Body and strict point-discovery semantics remain unchanged.
Parse all bounded Link header values strictly: malformed/truncated/ambiguous next
relations cannot become an absent-next FullEnumeration. Reject duplicate next
relations and invalid header syntax with positive observed quota preserved. Validate
first/prev/last relations against current page as well: last beyond current with
no next is contradictory evidence, never a singleton complete observation. Before
following next, require exact configured origin/immutable child path/no userinfo,
fragment, normalization tricks or unexpected/duplicate query keys. Only fixed
per_page=50 and canonical positive page are accepted. Next page must be current+1;
raw returned URLs remain opaque request targets after validation, never rebuilt.
No named-route fallback, arbitrary since/sort/filter, renderer-authored URL or
unvalidated redirect can change collection identity/representation.

A <=4KiB versioned private cursor binds account/actor/epoch, repository local/native
identity, subject local/native/kind/number, strategy and accepted page count/next
URL. Initial page1; accepted count determines the next canonical page, preventing
cycles/regression/skip even if query order changes. This strict numeric progression
is the durable cycle proof; no separate URL fingerprint history is necessary for
this fixed route/query contract. It does not generalize to opaque other providers. Limit20 accepted pages persists
across ten-page native yield/manual/cold resume. At cap, reject before HTTP while
retaining Partial/cursor/history; explicit rescan policy is a later coverage gap.
All post-response validation errors preserve account cooldown; stale-epoch quota
is fenced by existing Runtime/Store authority, never used as a grant.

FullEnumeration/SubjectHistory only when requested from the beginning with no
cursor and a valid initial page has no next. Explicit[] may then reconcile removed
cached comments. Every multipage page, including terminal/cold-resumed, stays
Uncertain/SubjectHistory and retains unseen historical rows. Mutable paging never
establishes a provider snapshot/deleted tombstone; 404/410/403 never become empty
successful conversations. Source/reconciliation remains stable during continuation.
Existing captured dispatch/head/account/epoch/selection leases still fence apply;
comments are historical subject facts and grant no current-head approval.

## Ordinary common UI

Replace the generic comments status panel with a common ConversationCommentsPanel
for PR/issue contexts; capability policy decides support. No provider-name switch.
Start collapsed: no local query/demand/hydration until opened and saved-readable.
Collapse releases demand immediately, including reduced motion. Native physical
activity/visibility remains authoritative; explicit Sync/Recheck only. Offline or
quota permits authorized cached reads, access/actor/epoch/subject cuts hide obsolete
content and reset only this panel. Private editor/draft CAS remains independent.

Read50-row local keyset pages with Previous/Next, at most100 cursor positions and
replay protection. Keep local cursor distinct from remote provider continuation.
When a facet revision changes, discard the local chain and restart at page1 before
publishing later-page rows; no cross-revision concatenation. Use a keyed paging
session/unmount so old demand/query callbacks cannot control the new session.
Surface missing/partial/known-empty/stale/sync errors/cooldown accurately. Render
raw text safely as React text, empty content, latest omitted/oversized badge with
retained known text and old validation, nullable/unknown author, timestamp labelled
Updated only. No author identity, creation-time, minimization or deletion claims
unsupported by the common model. Bound DOM/memory and preserve authored draft.

## Ownership and acceptance gates

After signed peer review: provider owner owns github.rs/comments.rs/transport.rs,
profile assertions and actual HTTP collection tests; Runtime owner owns new
runtime/github_comments_tests.rs plus registration; frontend owner owns new panel,
resource-capability-panels.tsx integration and real Workspace/SDK/bridge tests.
Root owns docs, serialized Cargo/typegen integration and publishing. No shared
DTO/storage/migration/generated-file/SDK runtime edits are planned. Agree any
required seam change before expanding ownership. All Cargo/native/typegen builds
use the existing serialized lane; synthetic fixtures only.

Actual HTTP and Runtime/SQLite tests must cover PR+issue identity/numeric ordering,
50rows/invalid/null/duplicate/oversized bodies, own-clock edits/older responses,
nullable user, initial empty removal versus multipage retention, malformed/duplicate
Link/redirect/query/hostile next and persistent20-page cold/yield cap. Prove positive
malformed-response quota durability, old-epoch200/429, held head/selection/access
loss/two actors, Body/Participants/Tasks/private drafts untouched, cold saved reads
with zero HTTP/vault. Common UI tests cover closed/unsupported zero work, local50+
paging/revision reset, safe rendering/retained states/current permissions, held
page/actor fences, collapse and editor/CAS retention. Existing independent facet
and participant/task wire controls remain unchanged.

Root runs focused checks, normal make typegen compatibility inventory, full local
frontend/native gates, independent review and attached signed draft on#163. Local
checks remain separate from exact new-head remote CI, anonymous public routing,
live private provider/vault/platform/native-window evidence and navigation latency.
No new schema/types/API promise or aggregate validation is inferred from ancestry.


## Accepted pre-code peer review

Native/provider/schema review confirms the optional nonnullable Body and required
nullable user shapes (official snapshot SHA2561429e93b5cbfa7547aa197e9553a6dacbd8d4aaf28012446ed730b152bcdf284).
Immutable-only no-redirect addressing protects explicit empty observations from
namespace reuse; v1 no304 and strict all-header/contradictory-Link admission protect
completeness. Operational page size50 matches the finite anonymous public probe
and bounds ordinary page working sets, below the official endpoint maximum100.
Strict canonical numeric next=current+1 plus persisted count20 replaces redundant
fingerprint history for this one route; accepted pages cannot cycle or skip.
Body's existing observed_body_state supports truthful latest omission/oversize with
retained content/validation. No shared public DTO or schema changes are required.
No source changes/tests/private credentials were involved in review.


Frontend peer review confirms existing SDK/policy seams are sufficient. The opened
scope owns a first-page50-row observer and exactly one visible demand, outside a
revision-keyed pager. Key by account/actor/epoch/subject/authorized view and Comments
facet revision, not unrelated global revisions. Suppress later-page rendering
while the first-page observer refreshes/errors/is unavailable; remount from page1
after an accepted current result. Every later snapshot must match captured facet,
subject, epoch, authorized view and facet revision. A stale_view cursor offers a
bounded local first-page reread or Restart saved conversation action, without
provider hydration. Inactive paging queries use gcTime0; retain at most the active
first-page/current-page snapshots and100 cursor positions instead of100 body pages.
Private Body/editor mounting remains independent. The held-page2/changed-revision
case must prove old rows hidden, page1 restored, singular demand and dirty editor
retention. No source edits or execution evidence were supplied by that review.


## Integration contract extension before repair — 5 October 2026

Independent review found a reachable existing Runtime boundary gap: a valid held
200 with positive account cooldown can fail Store admission after a same-epoch
head, repository-selection or facet-access cut. The generic apply_detail error
returned before persist_rate_limit and StaleView record_error intentionally does
nothing. Provider correctly preserves quota, but sibling work could dispatch
without that observed barrier. This violates the already accepted post-response
quota rule; no comment data or access should be granted by a rejected response.
Root additionally owns the narrow runtime/details.rs quota-boundary repair.
Runtime fixture owner first adds actual held200 durable provider:rest plus cold
sibling Body/noHTTP/no-vault assertions to establish a red regression. Independent
review refines placement: persist positive captured-account/epoch cooldown once
immediately after fetch Ok, before any conditional/page/Store/reconciliation
validation. Remove redundant later drift/success persistence; keep success live
scheduler cooldown and facet SyncStatus behavior. This also covers fallible
secondary reconciliation after a rejected apply. Existing Store epoch fences
reject obsolete responses before quota or truth can affect replacement/other
actors. Quota revision changes neither authorized view nor facet/run/cursor and
cannot grant access. Root also owns narrow updates to two existing UI controls
whose eager Comments/read-button assumptions contradict the accepted disclosure:
prove zero Comments read while closed and explicit fourth read on opening; retain
unsupported Bitbucket zero-work and original Reviews/Checks/merge/draft checks.
No schema/DTO/scheduler-policy/credential behavior expansion. Run the red control,
then repaired focused tests and complete local gates; source freezes and reviews
remain separate. Current focused21 native controls passed after synthetic typed
seed source-family/binding repair, with production guards unchanged. Normal
make typegen114 and independent AST inventory preserve291schemas/243aliases,
114functions/1event/public Branch with zero public changes; only generator
order/timestamp churn was discarded by restoring generated paths, no hand edits.
Frontend26 focused real Workspace/SDK/bridge tests, scoped Biome and desktop types
pass; full integration gates and publication remain pending.


## Local implementation qualification — 5 October 2026

Implemented the bounded GitHub.com PR/issue Comments mapper/strict collection
transport and existing generic saved rows, plus common disclosure, safe raw text,
50-row local keysets/100 cursor positions, facet revision/privacy fences and
singular opened-scope native demand. No public DTO/schema/migration/SDK runtime
change; other providers retain their capability policy. Collection page20 commits
its inert next21 before rejecting future dispatch; ten-page yield, explicit retry
and two cold reopen controls preserve the cap and Partial/history. Multipage
terminal pages retain unseen rows; only a valid initial singleton may reconcile
absence. Parent/max-row clocks do not order individual comment fields.

Executed evidence:

- `cargo test -p collaboration --lib comments`: all21 focused tests pass (11
  actual finite HTTP matrices,1 profile control,9 real Runtime/SQLite cases).
  Cold saved reads perform zero HTTP/vault; PR/issue/two actors/native IDs,
  rename refusal/recovery, own clocks, nullable user/omitted/oversized bodies,
  source identity/lease/access barriers, empty/full versus uncertain, local51
  keysets, durable20-page cap, positive invalid-response quota are exercised.
- Added held200 +Retry-After120 Store-rejection regression. Executed RED failed
  exactly at missing provider:rest before the repair. Early captured-epoch quota
  persistence once after fetch Ok turns it GREEN for head/selection/facet-denial,
  exact120s durable deadline and cold sibling Body blocked before HTTP/vault.
  Existing obsolete-epoch200/429, other-actor, saved facet/draft controls pass.
- Synthetic typed Participant/Task independent-control fixtures initially failed
  Store admission; corrected their full source-field families and captured subject
  binding while retaining sparse entry masks and all original assertions. No
  production validation was weakened. Old unsupported-GitHub detail test removes
  only its obsolete Comments member; Reviews/Checks expectations remain intact.
- `make typegen` normally generates114 commands. Independent AST inventory
  against exactTasks2e4b8d5 preserves291schemas/243aliases,114 command functions,
 1event and public Git Branch fields with zero changed/added public contracts.
  Generator-only ordering/timestamp churn was restored, not hand edited.
- All26 new real Workspace/SDK/bridge cases pass. They prove closed/unsupported
  zero work,51 local rows/gc0/100 cursors, safe/null/retained states, snapshot
  scope/facet/epoch/revision guards, held page/head refresh/error/actor/authview
  cuts, physical activity, explicit Sync/Recheck/restart, singular demand and
  independent dirty-editor/draft CAS. Real regression corrections require explicit
  restart generation for a synchronous unchanged reread and a valid current
  first-page receipt before admitting demand during an epoch cut.
- Full frontend exposed3 old eager-Comments/always-present unsupported-button
  assumptions. Narrow root updates retain original Reviews/Checks/merge/draft
  controls and add closed zero-read/open fourth-read/known-empty and unsupported
  zero-work assertions. All45 combined new+legacy controls pass; incorrect
  testing-library role option typing/formatting was corrected.
- Final serialized **`make verify` exits0**: all499 frontend tests/55files,
  repository lint/types/production desktop build, Rust fmt/all-target Clippy with
  -Dwarnings, and full default workspace **815 passed/3ignored** (collaboration
 464passed/2ignored). One successful filtered Git child-control result is excluded
  from the top-level815 count. No failed Rust result. Diff check passes.

Native/provider and UI/Runtime independent reviews found no remaining reachable
blocker after the quota fix; reviews are read-only, executed validation is root's
separate evidence. Root used the serialized native lane, frozen copyfile Bun
installation and scoped generated compatibility inspection. Reproducible tests
are committed source; temporary qualification logs are not repository artifacts.

Limits: no live private GitHub/PAT/keyring, GitLab/Bitbucket comments, inline review
threads, activity timeline, writes, enterprise/DC/relay or new Comments native
window/latency benchmark qualification. Anonymous public immutable routing is
separate limited evidence. Existing R103 own-Webview/native harness branch remains
separate and must be integrated with its qualification preserved. R122 stays In
Progress. Attached draft publication/new-head remote CI are the next gates; no
merge or full R122 completion is claimed.
