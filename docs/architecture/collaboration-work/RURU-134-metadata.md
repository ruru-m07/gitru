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
