# RURU-112 — Bitbucket Cloud task observations

Status: implemented and locally qualified fourth bounded R112 slice; publication
and exact-head remote CI remain separate. Read the shared
engine/backlog and R112 account, PR and participant notes first. The isolated
managed branch `ruru/ruru-112-bitbucket-tasks` starts from attached draft #162,
exact `d2e91957e28de36cc3f7d0ecb2eb6a5d0ea15dcd`. No merge is authorized.
Live R112 remains In Progress, with R76/R100/R111 implementations in the reviewed
unmerged ancestry; no duplicate Task issue or PR was found. #162's own reported
CI is progressing, separate from #157's all14 and #161's all11 successful checks.
This contract and any amendments must be signed before their source changes.

## Scope and evidence

Read-only PR Tasks through existing native HTTP, Runtime, SQLite, saved queries,
revision bridge and visible demand. Add typed task facts and the ordinary common
Tasks panel. Accounts stay independent of Gitru cloud. No task writes, remote
completion controls, comment hydration, approval interpretation, readiness or
strict head-guarded merge. Other providers/issues explicitly do not support Tasks.
Preserve Participants, summary/Body behavior, existing discovery fingerprints,
authored drafts/CAS, account/epoch/scope fences and the separate R106 restore
ceiling. R103's separate SDK own-Webview revision listener must survive eventual
stack integration; this branch does not inherit or qualify that listener change.

Fresh primary-source review (4 October 2026):

- [Tasks endpoint](https://developer.atlassian.com/cloud/bitbucket/rest/api-group-pullrequests/#api-repositories-workspace-repo-slug-pullrequests-pull-request-id-tasks-get)
  documents PR tasks, optional q/sort/pagelen and read:pullrequest:bitbucket.
  Use pagelen=50, no filter and the documented default ordering; do not borrow the
  PR feed's sort=id rule. Tasks and comment resolution have separate semantics.
- [Current OpenAPI](https://dac-static.atlassian.com/cloud/bitbucket/swagger.v3.json?_v=2.300.196)
  requires content, creator, state, created_on and updated_on. Raw content is an
  optional string, not declared nullable. Task id/type are not mandatory schema
  identity evidence. Account requires a string type, with user/team/app_user
  subclasses and an optional UUID; preserve bounded future account types.
- [REST introduction](https://developer.atlassian.com/cloud/bitbucket/rest/intro/)
  describes UUID addressing and opaque next links. Extending the existing UUID
  repository route to the child Tasks route is a documentation inference until
  live-provider qualification, not evidence from a personal token.

Task IDs and canonical nonnil account UUIDs are stronger Gitru admission rules,
needed for stable saved identity. Explicit optional resolver/date/comment null
means known absence by this contract; OpenAPI does not independently establish
all such nullable shapes. Synthetic HTTP tests qualify our policy separately
from provider behavior. No unattended personal credentials/keyring inspection.

## Typed local contract and field authority

Add DetailFacet::Tasks / ResourceFacet::Tasks, PR-only, and adjacent-tag
NativeDetailPayload::TaskV1 serialized kind `task.v1`. Reuse scoped DetailEntry
storage; never overload generic Body/title/review/check fields. Proposed Rust DTOs:

```rust
pub struct TaskActor {
    pub provider_id: String,
    pub kind: String,
    pub login: Option<String>,
    pub display_name: Option<String>,
}
pub struct TaskV1 {
    pub content: DetailValue,
    pub observed_content_state: DetailValueState,
    pub creator: TaskActor,
    pub state: Option<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
    pub pending: Option<bool>,
    pub resolved_at: Option<String>,
    pub resolved_by: Option<TaskActor>,
    pub comment_id: Option<String>,
}
```

Optional local state/time fields allow the engine's blank/retained projection;
the adapter requires their actual nonnull valid API observations. Empty task
content is Known; missing raw is Omitted; raw:null/nonstring is invalid. Ignore
markup/HTML rather than using them as content authority. Required content object
must exist; text above65,536 bytes becomes Oversized with no retained incoming
text. Keep saved Known content/its validation and ordering clock on omission or
oversize, with a separate latest observed_content_state. A new unknown row may
save Omitted/Oversized evidence without inventing a Known-content validation.

Exactly twelve mutually exclusive Task field tags:
task_content, task_creator_login, task_creator_display_name, task_state,
task_created_at, task_updated_at, task_pending, task_resolved_at, task_resolver,
task_resolver_login, task_resolver_display_name, task_comment_id.
Masks/source/validations/clocks must match payload and facet. Tasks alone permit
twelve fields; ordinary and Participants families retain six. The bounded clock
decoder may retain at most twelve before facet validation; non-Task evidence
above six or mixed-family/duplicate/corrupt clocks cannot grant ordering authority.
Do not broaden legacy six-field masks or silently accept arbitrary native JSON.

Creator UUID/account type form immutable task identity context; reject conflicting
creator identity for one compound task ID. Login/display name are bounded optional
presentation (255/1024 bytes), observed null is known absence, omitted retains.
Actor type/state are nonempty <=128 bytes/control-free, including future strings;
map Account.type, not app_user's independent kind subfield. State is not a common
review result. Required created_on/updated_on and optional resolved_on must parse
RFC3339, <=128 bytes. Pending is true/false or unobserved, never null/nonboolean.
Comment association is only a canonical positive int64 string scoped by this PR;
reject malformed nonnull identity, ignore unrelated comment content and links.

Per-entry ordering uses only the observed task's own updated_on when its required
task_updated_at observation is present. Collection source provider_updated_at is
None. Parent PR dates, created/resolved action dates, aggregate maxima, or opaque
cursors cannot order task fields. Preserve historical per-field source/version/
timestamp proof through cold reopen; older entry fields cannot replace newer ones.

Resolver identity/type is one TaskResolver field, presentations two separate
fields. Absence retains; explicit resolver:null establishes absent resolver and
absent presentations. A newly accepted resolver identity invalidates prior
person's omitted presentations and their validations/clocks to Unknown. Do not
retain the old person's nickname or fabricate a Known-null presentation for a
new person. Presentations apply only if incoming resolver identity equals the
accepted saved resolver identity; an older rejected identity cannot rename the
new resolver. Exclude rejected/mismatched-context presentation tags from the
published current mask. Qualify both resolver changes and late older responses.
Resolve identity before presentation regardless of incoming mask order; clearing
old clocks must not make held old-person presentation admissible. Incoming API
requiredness and clock validation remain distinct from blank saved Option values.
The common native actor identifier is bounded/opaque; only the Bitbucket adapter
requires UUIDs, so future provider actors need not pretend to have UUID identities.

## Transport, identity, pagination and completeness

Task identity: `bitbucket_cloud:task:<repository UUID>:<PR id>:<task id>` and
compound provider_id. Account remains the storage partition. Require positive
int64 task/PR identifiers; reject duplicates within each page. No synthetic id
from order/content or provider task.type requirement. Bind every request/page
to captured account/epoch/immutable repository+PR/subject and original head tuple;
no parent object in the task response supplies identity/clock authority.

Use the existing fixed public UUID PR hierarchy with /tasks?pagelen=50. Reuse
sensitive Bearer handling, no redirects, fixed host,4MiB response/20-second deadline.
Only validated exact-origin/exact-child-path continuations may be followed, with
the existing permitted opaque page/cursor/after/before parameters and fixed
pagelen, no duplicate/unexpected query credentials/filter/sort. Reject off-origin,
wrong PR/repository, replay/loops, malformed envelopes, absent/null/nonarray values
or >50 rows; no invalid page reaches Store. Empty valid values is meaningful.
No ETag/304 authority until independently documented and qualified.

Cursor carries version/strategy, account/epoch/subject/repository/PR, accepted-page
count and at most20 SHA-256 continuation fingerprints, <=4KiB total and existing
bounded URL length. Hard limit20 accepted pages persists across native ten-page
yield, manual/cold resume; stop Partial with continuation retained at the cap.
Do not reset the budget via another process/job. A later explicit rescan policy
for capped collections is a documented remaining coverage-policy gap.

Mutable multi-page traversal is Uncertain from first page to terminal page;
propagate that exact reconciliation in cursor/lease. Never upgrade a terminal
page to FullEnumeration. Existing unseen rows survive this partial traversal even
when next is absent. Only valid first-page values with no next, requested from
the beginning, can declare FullEnumeration/SubjectHistory and known completeness
within this saved observation; explicit[] can then clear provider task rows.
This is no global provider snapshot guarantee. Single-page FullEnumeration must
be refused for a continued/cold-resumed traversal. Parent head changes reject
held apply, but saved historical task facts never establish current-head approval.
Source `bitbucket.tasks.v1`, adapter version1, all twelve Task tags, freshness180s.

## Storage, migrations and generated IPC

Forward0009 widens only detail observation/demand facet checks to Tasks through
the existing four-table FK-enabled transactional rebuild. Never change old0001–0008
SQL or ledger/checksums. Freeze independent v8 SQL/seed/checksums with generic and
participant payload/private evidence, Body metadata, demands/cursors, grants,
two actors, revisions and authored drafts. Compare exact historical rows/ledger
over actual pinned SQLx/bundled SQLite; inject failure after actual old-parent
drop and prove rollback/FK enforcement/immediate cold reopen/draft CAS. Keep the
known R106 archive v1/v2 policy untouched. Scope invalidation must include Tasks.
Keep the existing 'Tasks rejected' historical schema control scoped to real v8;
latest v9 must admit Tasks and reject an unknown future facet. Do not weaken the
original participant upgrade/rollback assertions to accommodate schema growth.

Root extends the normal source-derived tagged dependency/nullable generator to
tasks.rs and the explicit task.v1 family. Derive field families from Rust helper
declarations, enforce disjoint generic/participant/task masks and validations,
and preserve all original participant guard controls, commands and public Git
Branch fields. No hand-written generated DTO or arbitrary schema escape hatch.
Run normal make typegen after the shared model/capability freeze.

## Frontend and implementation ownership

Ordinary PR Tasks disclosure starts collapsed. Only an opened, supported,
saved-readable inner panel mounts the local query and ordinary native visible
demand. Collapse releases interest immediately, independent of animation. Hidden/
inactive webviews follow existing native authority. Temporary offline/quota
conditions retain authorized saved reads; access loss/actor cutover suppresses old
content. No implicit hydrateDetail on mount; Sync/Recheck uses existing explicit
commands. No provider-name branch, readiness checkbox or remote resolution UI.

Browse local saved rows with bounded50-row keyset pages and local previous/next
controls, at most100 retained cursor positions, separate from provider sync.
Reset only this panel's open/page/error state on account+actor+epoch+subject key;
never reset the independent authored editor. Show native content/state/pending/
actors/action dates/comment association with accurate Unknown/None/false/retained
validation times, latest omission/oversize, missing/partial/known-empty/error/
cooldown evidence. Render plain escaped text, bounded DOM and reduced motion.

After signed review: native-core owner owns models/detail/capability/store/private
clock/migration/tests; provider owner owns Bitbucket routing/cursors/mapper and
actual HTTP + Runtime tests; frontend owner owns production panel/SDK type exports
and real Workspace/SDK/bridge/wire tests. Root owns generator/docs/integration and
serialized validation/publishing. No overlapping source ownership. All Cargo,
typegen and packaged native processes use the existing serialized cargo lane.

## Acceptance gates and explicit limits

Qualify actual HTTP empty/50/invalid/duplicate account/task IDs, UUID route rename,
all native field states, own clocks, mixed families, resolver context and stable
multi-page Uncertain; twenty-page persisted budget and hostile/repeated next URLs.
Actual Runtime -> SQLite cases must cover missing/capped/cold/older/held head,
selection/deselection and old-epoch200/429, current positive quota, facet-specific
403 and two actors with the same presentation. Invalid pages cannot replace
rows/coverage/grants; no task sync overwrites Body/Participants/drafts/CAS.

Store tests must prove twelve-field known/false/null versus omission/oversize,
per-field clock retention, resolution identity fences, single-page absence versus
multi-page uncertainty, invalid clocks/family bounds, scope invalidation and cold
saved reads with zero HTTP/vault calls. Frozen-v8 migration qualifies the actual
transaction boundary, all old tables/ledger, FK constraints and authored intent.
Installed generated schema + ordinary production UI/SDK tests prove all three
payload families, unsupported/closed zero reads/demand/hydration, local cursor
browse beyond50 rows, privacy fence/draft retention and collapse interest release.

Root runs focused native, required workspace tests/Clippy/fmt, normal typegen,
meaningful frontend tests/lint/types/production build, independent reviews and a
scoped signed attached draft PR on #162. Local results remain separate from new
exact-head remote CI/platform/live provider/vault qualification. No live token,
unreported CodeQL, all-platform UI, power-loss or navigation benchmark is inferred.
R112 completion status requires its actual acceptance coverage; this worknote
cannot treat a planned feature or ancestor CI as implemented qualification.

## Independent pre-code review

Provider/schema and native/storage peers reviewed this contract read-only. Both
confirm the bounded route/model/FK approach. Accepted clarifications above make
three-family6/6/12 authority explicit, isolate own-task clocks from collection
clocks, process accepted resolver identity before presentations, preserve stable
Uncertain across continuation, and retain historical v8 negative admission.
No source changes or validation were performed during that review.


## Ancestor remote qualification — 4 October

Participant #162 exactd2e91957e28de36cc3f7d0ecb2eb6a5d0ea15dcd now passes all11
reported checks/statuses. CI37210699030 includes Rust and ordinary packaged E2E
on Linux/macOS/Windows, frontend/Clippy/fmt; Windows E2E111461119358 completed
15:20:34UTC. Cloudflare/CodeRabbit/Vercel also pass. No exact-head CodeQL is
reported. This does not qualify new Tasks code/0009/native UI or live tokens.


## Implementation and local qualification — 5 October 2026

Typed task.v1/TaskActor and twelve disjoint Task fields now flow through the
existing native provider, Runtime, SQLite, generated SDK and ordinary PR panel.
Per-task updated_on clocks, immutable creator context and resolver-first merging
preserve known false/null versus omission and reject older/mismatched evidence.
The fixed UUID Tasks child route has bounded exact-origin continuation validation;
mutable multipage traversal remains Uncertain, and the durable twenty-page budget
survives the scheduler's ten-page yield, manual resume and two cold reopens.
Capped collections retain Partial coverage and authorized history without extra
HTTP. Single initial-page enumeration can reconcile explicit empty results.

Forward0009 rebuilds the existing four facet tables with foreign keys enabled.
Frozen independent v8 SQL/seed/checksums qualify exact historical rows across all
25 tables and the eight applied ledger records, actual post-parent-drop rollback,
immediate cold reopen, FK admission/enforcement and authored draft CAS. Published
0001–0008 bytes and the R106 v1/v2 archive ceiling remain unchanged. Historical
participant negative admission runs against the actual version8 migration set;
its original upgrade, rollback and draft controls remain intact.

The common collapsed Tasks disclosure reads and registers visible demand only
when opened and supported. Native activity remains authoritative. It uses bounded
50-row local keyset pages, a100-position cursor cap, safe text, validated actor
presentations, a120-Unicode-scalar content heading and explicit Sync/Recheck.
Authorization/actor/epoch/subject cuts suppress obsolete content without resetting
the independent private editor. No provider-name branch, automatic hydration or
remote resolution/write controls were added.

Actual local gates:

- Focused native task library20 (HTTP8, actual Runtime/SQLite9, private clocks3)
  and Store/migration20 (Task Store9, frozen-v8 migration6, existing participant
  migration5) pass. The final full default Rust workspace passes794 top-level
  tests/three ignored, including443 collaboration tests/two ignored. All-target
  workspace Clippy with-Dwarnings and final formatting/diff checks pass.
- The original cap fixture incorrectly equated resumed request-generation IDs.
  Store intentionally creates a fresh lease while carrying durable traversal
  membership. The corrected test asserts fresh IDs plus exact10→20 page/history
  continuity, cursor/coverage/rows/drafts and zero extra capped HTTP. Production
  request-generation fencing was unchanged. Clippy also caught a fixture guard
  held across await; lexical scoping fixed it without changing assertions.
- Normal make typegen generates114 commands. An independent TypeScript AST
  inventory preserves all289 prior schemas and241 aliases, adds only TaskActor
  and TaskV1 (291/243), and permits only the five expected facet/field/payload/entry
  schemas to change. All114 command functions, the one event function and public
  Branch name/display_name/is_remote/is_detached remain unchanged.
- Full frontend473 tests/54files, lint, types and the production desktop build
  pass after final UI changes and normal generation. The make verify run completed
  those frontend/build gates, then stopped on the fixture-only Clippy finding;
  final Clippy and the full Rust workspace/fmt were run successfully after that
  correction. No aggregate successful make verify execution is invented.

Independent read-only native/migration and frontend/generator reviews found no
reachable blocker. Reviews
inspect source and fixture fidelity; actual local commands above provide execution
evidence. Temporary validation logs are diagnostic conveniences, not durable
artifacts; commands and test sources are reproducible evidence.

This branch is ready for a scoped signed draft stacked on #162. Its own remote
CI/platform matrix is pending separately from ancestor #162's all11 successful
checks. Live Bitbucket UUID child routing/null policies, production credentials/
vaults, actual Tasks native-window UI, navigation benchmarks and unreported CodeQL
are not qualified. The capped-collection rescan policy, writes, enterprise/Data
Center, R106 coordinated shutdown/current-schema archives and the separate R103
own-Webview listener integration remain outside this slice. No merge is authorized.
