# RURU-134 — metadata-bearing issue creation

Chosen implementation contract, 8 October 2026, before source edits. This extends
GitHub.com issue creation from qualified PR201 `dfb660ab`, which is stacked on
PR199. Live Linear RURU-134 remains In Progress; its metadata criterion is open.
No PR is merged. This managed worktree lives on /Volumes/Lexar.

## Scope and staged gates

Support repository labels, up to ten assignees and one open milestone as authored
issue-draft metadata. Creation remains one durable POST. A valid strong 201 proves
creation even when optional metadata differs, is missing, or is malformed; field
outcomes are independent historical observations, never automatic PATCH/POST work.
Other providers, issue types/projects/custom fields, and creating catalog objects
remain explicitly unsupported by this slice.

The first source checkpoint contains native DTOs, a separate v2 payload/evidence
codec, bounded read-only repository catalogs and one-read point revalidation
steps. It exposes no incomplete send path. No migration25, recovery policy change,
or durable v2 registration is permitted before the exact schema24 R132 checkpoint
passes its full gate and is imported. The later checkpoint adds draft/cache storage,
scheduling and v2 delivery/recovery, then generated IPC and the complete UI.

Existing github.create_issue payload1 and github.issue_created proof1 stay frozen;
no added nullable fields, changed hash, rewritten historical bytes or decoder
restriction. V2 uses exact private strict carriers. Old v1 save/send must refuse a
nonempty v2 draft rather than discard metadata. All versions share the same draft,
active submission, resolution and provider-created identity uniqueness.

## Authored and provider data

Metadata selections contain immutable provider IDs and exact saved display names:
label ID/name/optional color; assignee ID/login; milestone ID/number/title. Sort
sets deterministically by native ID; reject duplicate IDs/aliases, malformed IDs,
control characters and oversized values. Never normalize authored spelling into
a different intention. Saving is atomic with title/body under one generation;
unchanged content keeps the generation. Offline/disconnected recovery retains
these authored selections; catalog options and permission/send authority redact.

Bounds: 32 labels (name1024 bytes), 10 assignees (login256 bytes), one milestone
(title1024 bytes), existing title/body bounds. Every command/request/preparation/
proof independently fits64KiB after escaping; saved drafts may exceed send budget
and remain editable. Selection identity may be preserved while provider display
metadata is omitted from proof. No-selection means omit that optional POST key.
Explicit best-effort metadata consent accompanies existing background consent.

## Catalog and preflight contract

Catalogs are independent Labels/Assignees/Milestones scopes, bound to exact
account/actor/epoch/view/repository identity. Native page cursors carry those
bindings and page<=20; at most100 rows/page, 2,000 rows/run, 50 local options/page.
Only demand/refresh fetches. Page-number traversal is not a snapshot: coverage
remains partial, terminal cap drops continuation but retains known-more, and any
ordinary refresh can start page1. No absence pruning or deleted-selection inference.
Mutable provider URLs are never renderer authority; route construction stays native.

Use existing bounded same-origin transport with strict same-operation pagination.
Numeric repository paths are retained as a documented compatibility boundary:
o live PAT or mutation is used to qualify them, and no mutable-name POST fallback
is introduced. Label names/logins are encoded as literal path segments; dot-only
segments refuse instead of URL normalization. Catalogs retain archived/closed/
unknown availability without upgrading it to send permission.

Point revalidation is a finite sequence of individual native reads, not a loop
hidden inside an adapter. Each result returns cooldown/auth evidence so the runtime
can persist it and recheck current account budget/epoch before the next read:
repository ID/full-name/has_issues/archive/permissions.push, label ID/name/archive,
user numeric ID/login plus exact repository assignability204, milestone ID/number/
open state. A changed alias requires reselection/new generation. A valid GET does
not prove token write permission; metadata permission is best-effort preflight,
never server CAS. Refusal, malformed body or continuation preserves observed quota.

## Receipt separation

Parse required201 identity/repository/actor/title/body/clocks separately from
optional collections. Optional observation is bounded: retain only selected-ID
membership evidence (or exact milestone ID), not unbounded provider text. Full
well-formed lists establish selected membership; malformed/oversized/missing lists
are Unobserved, never empty. Extra server defaults do not defeat success. Derive
NotRequested/Applied/Different/Unobserved per field; Different is not a claim about
why it changed. Core ambiguity stays Unknown and cannot replay. A valid core201
must remain confirmable with Unobserved optional data and a reserved proof budget.
Publication reuses the existing canonical issue identity/visibility/held-feed rules.

## Proposed public interface and ownership

Native `issue_metadata.rs` owns IssueMetadataSelection, typed option/availability,
IssueMetadataQuery/Page, IssueMetadataOutcome, SaveIssueDraftV2Request,
SubmitIssueV2Request, IssueDraftV2Snapshot/Page. New APIs stay separate from legacy
DTOs: issue_draft_v2/save_issue_draft_v2/issue_drafts_v2/submit_issue_v2 plus local
issue_metadata_options and typed demand/refresh. Exact shapes freeze in source
before root's frontend work; generated IPC is the only TS contract.

R119 owns native new modules, provider trait/default/dispatch registration,
transport extension only as needed, native tests and this note. Root owns frontend
SDK/UI and final generated IPC after native commands freeze. No other lane edits
this tree. Schema25/recovery ownership starts only after R132's qualified24 import.

## Verification plan

Synthetic finite HTTP controls cover all three catalogs, strict next/cap reset,
foreign account/epoch/cursor/route, archived/closed/malformed options, literal path
characters, recycled aliases, exact assignee204 vs arbitrary200, repository access,
quota on success/error/invalid response, no follow-on read after quota, and malformed
optional metadata independent of required identity. Frozen v2 codec controls prove
canonical bytes/duplicates/tamper/budgets; frozen v1 fixtures remain unchanged.
Later storage gates cover atomic generations, legacy refusal, cross-version
ambiguity, restore corruption/byte preservation, auth reset and held responses.
Local checks, current-head CI and live provider/platform qualification are recorded
separately. No credentials or provider mutations are needed for these controls.

## Primary references checked 8 October 2026

- [Create issue](https://docs.github.com/en/rest/issues/issues#create-an-issue):
  optional labels/assignees/milestone can be silently dropped without push access.
- [Labels](https://docs.github.com/en/rest/issues/labels#get-a-label): name point
  lookup returns ID; current API also reports archived_at.
- [Assignees](https://docs.github.com/en/rest/issues/assignees#check-if-a-user-can-be-assigned):
  repository assignability has exact204/404 semantics.
- [User by numeric ID](https://docs.github.com/en/rest/users/users#get-a-user-using-their-id):
  `/user/{account_id}` distinguishes durable identity from mutable login.
- [Milestones](https://docs.github.com/en/rest/issues/milestones#get-a-milestone):
  native ID and repository number differ; listing supports state=open/closed/all.

## First native checkpoint — 8 October, schema gate still held

The native-only DTO/codec/read slice is implemented. Public v2 DTOs are in
`crates/collaboration/src/issue_metadata.rs`; nested draft snapshots/pages retain
legacy display/context shapes without changing their serialization. The frozen
v2 payload uses explicit private selection carriers and tagged fields; its
receipt uses explicit FrameV2/PreparationV2/CreatedCoreV2 carriers rather than
serializing extensible public repository/item models. It is not registered for
admission or dispatch and has no callable desktop commands yet.

GitHub catalog/point methods are on the provider read trait with Unsupported
by default. Catalogs explicitly return Partial, including terminal empty pages;
continuations bind native catalog_generation, account/actor/epoch/view/repository/
path/family, without unrelated global revisions. Page20+next becomes terminal
truncated and a new run restarts page1. No absence pruning is implemented or
claimed. Point reads perform exactly one HTTP operation: strict200 JSON or exact
204 assignability, no redirect/continuation, retained cooldown on success/error.
The configured header and checked docs both use2026-03-10. Missing archived_at is
Unknown, not false; it does not itself forbid selecting/revalidating a matched
label. Explicit archived/closed observations remain unavailable.

Required201 core parsing now survives missing/malformed/oversized/different
optional collections. Selected native-ID membership produces independent
historical outcomes; unrequested defaults do not consume the causal proof.
Preparation/request/core reservations count actual escaped text plus a bounded
4KiB observation/wrapper reserve. Strict v1 operation/proof source is untouched.

Local qualification:26 focused native model/HTTP/receipt controls pass, including
all three catalogs, immutable identity/alias checks, exact204, foreign/cyclic
continuations, cap reset, hostile routes, quota preservation, canonical/tampered
v2 bytes and worst escaped frame/proof boundary. Strict all-target/all-feature
collaboration Clippy passes. These are synthetic reads/receipt codecs; they do
not qualify live numeric aliases, real credentials or enabled v2 delivery.

Before runtime integration, a shared preparation limitation was identified:
existing delivery accounts one read for policy.prepare, while metadata can need
54 reads. A hidden loop cannot provide the agreed live quota/account boundary.
The proposed native-only stepwise preparation protocol must check account and
budget and persist every successful/error observation before the next HTTP;
continuations are bounded and process-local, never restored permission authority.
Its exact integration is under review; no shared delivery code changed here.
Schema25 and recovery still wait for qualified schema24. Root's separate UI
preparation checkpoint55ecab1b changes authority/editor behavior only; it does not
expose a metadata send path.
Existing v1 issue creation/publication/budget/restore regressions also pass29/29;
workspace Rust formatting and diff checks pass. No full-suite or remote-CI result
is attributed to this unpublished checkpoint.

## Approved stepwise delivery preparation contract — before shared edits

The native preparation protocol adds Continue versus Complete while default
policies keep their existing single Complete behavior. One delivery turn performs
at most one preparation read. Continue persists observed quota/auth state before
saving bounded process-local continuation and returning, releasing dispatch and
lifecycle locks. It never reaches the attempt writer or POST. Later turns preserve
account/foreground scheduling opportunities and load the current command/account.

Each continuation binds command hash/generation, actor/epoch/installation, native
context and authorization_view. A writer-held read rechecks current authorization,
command state/controls/dependencies and fresh policy context without incrementing
its durable preparation/reconciliation budget again. Only Complete reaches the
existing final writer-held claim. A paused/cancelled/replaced command, context or
view change, credential cutover, stop, quota wait or monotonic expiry drops the
chain. Restart loses every continuation; it restores no permission authority.

Memory is bounded to eight chains, each with <=64KiB native context and <=64KiB
continuation, <=64 read steps, and <=120 seconds monotonic lifetime. Capacity or
invalid/oversized/cyclic continuation refuses/defer safely; it never evicts into
POST. A step returning byte-identical continuation is invalid. The final read's
quota is persisted before considering claim, and expiry/stop/current budget are
checked after a held vault/read too. New runtime controls must exercise real
SQLite/owned delivery turns: interleaved other-account/foreground work, held quota,
epoch/view/context/command cancellation, expiry, restart, no attempt before final
completion and unchanged single-step policy behavior. No schema change is needed
for this generic native process-local mechanism.

### Stepwise preparation qualification

The native delivery lane now carries process-local preparation state across
separate owned turns. A continuation does not consume another durable preparation
budget or record an attempt. The existing background loop offers foreground reads
between turns, and account rotation remains intact. Eight chains, 64 steps,
64KiB context/continuation values and a 120-second monotonic lifetime bound this
state. Repeated continuation bytes, expiry, errors, quota waits, account reset,
command controls and shutdown discard partial authority. Cold startup redoes the
first read; saved command/proof bytes are unchanged.

Independent review found that a preclaim read alone left a writer-wait gap. The
final native claim now receives exact authorization view/context and a native
lifecycle/deadline callback. It rechecks them under the writer before and after
operation validation, and checks the live callback again before committing an
attempt. The held-writer regression proves same-epoch view/context changes and
expiry refuse the claim; its unchanged-authority control can claim normally.

Local qualification: all 40 delivery tests pass with one subprocess helper
ignored, including 12 new runtime controls. They cover a 54-read chain using one
durable preparation budget, foreground/peer-account interleaving, first/middle/
final-step quota on success and error, authentication, cold reopening,
cancellation, cache capacity, account reset, cyclic/oversized/excessive chains,
held-vault quota/expiry, held-read shutdown and writer-held final authority.
Initial new fixtures incorrectly expected the preparation counter to survive the
existing successful-attempt reset, requested Pause for a never-attempted command,
omitted a synthetic provider capability and read quota through an intentionally
auth-blocked reader. Those fixture expectations were corrected; production
recovery semantics and visibility gates were retained. Test source also corrected
an unavailable quota accessor and an explicit clock-trait import during compile.
These are synthetic native/runtime checks, not live provider, OS suspend or
remote CI evidence. The metadata operation remains unregistered until its
qualified schema24 prerequisite and complete schema25 storage/delivery slice.

### Frozen IPC and catalog ownership contract

The six planned native commands are `collaboration_issue_draft_v2(IssueDraftKey)`,
`collaboration_issue_drafts_v2(IssueDraftQuery)`,
`collaboration_save_issue_draft_v2(SaveIssueDraftV2Request)`,
`collaboration_submit_issue_v2(SubmitIssueV2Request)`,
`collaboration_issue_metadata_options(IssueMetadataQuery)` and
`collaboration_refresh_issue_metadata(RefreshIssueMetadataRequest)`. They return,
respectively, `IssueDraftV2Snapshot`, `IssueDraftV2Page`, `IssueDraftV2Snapshot`,
`IssueSubmissionReceipt`, `IssueMetadataPage` and `RefreshReceipt`. The refresh
request binds account, authorization epoch, repository and catalog kind. No
callable stub is registered while storage/delivery remains incomplete. Existing
leased demand gains RepositoryLabels, RepositoryAssignees and
RepositoryMilestones, each requiring a repository and forbidding subject/facet.

Catalog runs use a writer-issued monotonically increasing revision as their run
identity, preventing ABA after eviction. Local pagination binds catalog revision,
run generation, actor/epoch/view, repository and normalized search, while the
ordinary global revision remains only the change-feed envelope. Repeated mutable
traversals stay Partial; cache eviction cannot prove provider deletion. Selected
references are authored data in the draft and survive all catalog eviction.

Final stepwise checkpoint gates also passed strict collaboration Clippy with all
targets/features, workspace Rust formatting and diff whitespace validation.
The production claim entry always carries native authority; the prior unguarded
entry remains only for unchanged synthetic fixture call sites. Both attempt and
preflight-proof commits apply the final live check.

### Schema25 and catalog implementation ownership

Qualified R132 schema24 was merged as signed87aa445d after its owner reported
834 library cases,11 migration cases,434 remaining-workspace cases and strict
workspace Clippy passing. This is prerequisite evidence; it is not a test claim
for the combined metadata branch. Its frozen24 SQL fixture has SHA384
`a0621e2184e541a638c5326667675e8de556eee5126366a87afddb2be3f85e488f4bf8c7c8705fe7f0ca5e8dd6c1f2ec`.

The additive25 migration uses `issue_draft_metadata` rather than altering the
legacy draft row shape. Native saves update its metadata and the parent CAS
generation in the same transaction. Identity/retention triggers protect the
child table. The existing submission-binding trigger accepts only explicit
payload1 with empty metadata or payload2; the created-receipt uniqueness index
covers the frozen native-ID paths in both proof versions. Existing authored
submission/resolution identity constraints are retained.

Catalog tables are `repository_metadata_catalogs` and
`repository_metadata_options`, with a per-family browse index and per-account
retention indexes. The catalog module owns only these new cache tables and its
finite tests. It retains at most2,000 options per family across runs,6,000 per
account and48 family headers per account. Capacity removal is cache eviction,
never remote absence evidence. Eviction emits changes for affected exact scopes.
The native migration/registration/draft/delivery/recovery lane remains separate
from this catalog module, while root owns SDK/UI/generated bindings.

Catalog APIs are begin/resume/request/apply/fail and the local options query.
A `CatalogLease` carries the native provider request plus accepted page count;
`CatalogApplyReceipt` returns the committed revision and optional next lease.
Cold-open cleanup retires Syncing while preserving valid resumable checkpoints.
Synthetic SQLite3.54 applied all25 DDL files with an empty foreign-key check;
Rust migration, restoration, quota and runtime integration checks remain pending.

### Native integration checkpoint

The six commands now have functional native implementations. Authored metadata
uses the parent draft's CAS generation and a frozen selection carrier; v1 saves
and new v1 sends reject nonempty v2 selections instead of dropping them. Current
context hashes title/body/metadata, generation, account actor/epoch and repository
identity. An exact admitted UUID returns its existing durable receipt. Editing a
draft cannot bypass a pending or uncertain submission.

The version2 delivery policy performs repository permission plus every selected
label, numeric assignee identity and assignability, and milestone point read as
separate native turns. At the product maxima this is54 reads, bounded by the
shared64-step/120-second preparation contract. Label archive status omitted by
the pinned API remains Unknown; an explicit archived label, closed milestone,
changed identity/name or missing metadata permission prevents a POST. The exact
zero-attempt `github.issue_creation_declined` version2 proof records that refusal.
A decline does not prove a remote outcome. Recovery cannot replace the operation;
a never-attempted command may be cancelled through existing controls.

The final claim rechecks native account/repository/context and encoded receipt
budget under the writer. Only the authenticated strict201 core confirms creation.
Optional returned fields yield Applied, Different or Unobserved independently;
there is no second POST or automatic PATCH. The receipt retains selected-ID
membership rather than claiming a complete label/assignee set. Canonical cache
publication therefore marks those optional collections Omitted and retains normal
provider refresh authority. Created identity and the historical metadata outcome
bind the same immutable confirmed command. Both are hidden when provider access
retires; authored selections remain local.

Catalogs use the existing fair read scheduler, one page per turn and at most20
pages per traversal. Reads are local; visible leased demand and explicit refresh
admit network work. Cold continuation resumes exact native generation/cursor.
Budget is checked before and after vault access; observed quota is persisted
before another page. Captured binding includes actor/provider/host plus immutable
repository ID and path. Each lease also carries catalog revision, so a later
same-epoch denial, eviction or cold cleanup retires an older held page. Terminal
local eviction preserves traversal completion but marks freshness stale. Repository
deselection removes only catalog caches and prevents ABA resurrection. The shared
issue-creation capture now checks the canonical selected column instead of stale
repository JSON.

Current local evidence:47 metadata controls pass across the focused runs
(26 frozen codec/HTTP controls,11 SQLite catalog controls, seven
authored/delivery/publication controls and three
actual runtime catalog controls). These cover independent global-revision paging,
bounded retention, denial/view/epoch/binding fences, cold continuation, offline
CAS, v1 refusal, uncertain201, optional-field differences, no work for local reads,
hidden-owner termination, persisted quota and held-response deselection. The
maximum-selection control observes all54 point reads followed by exactly one
POST; the escaped-body control refuses encoded-budget overflow before any outbox
admission while retaining authored text. The initial maximum-selection fixture
served numeric-ID order instead of the frozen payload's lexicographic order;
the adapter correctly refused it, and correcting only that fixture produced the
seven-case authored/delivery pass. The final three runtime controls also pass;
the successful depleted page persists RateLimited rather than leaving the catalog
indefinitely Syncing while continuation waits. Strict collaboration Clippy passes
with all targets/features; its initial two collapsible-if diagnostics were fixed
without changing behavior. The complete workspace gate remains pending at this
checkpoint. One
intermediate compile saw the recovery module declaration before its concurrently
owned source file existed; it was an integration checkpoint, not a runtime fault.
