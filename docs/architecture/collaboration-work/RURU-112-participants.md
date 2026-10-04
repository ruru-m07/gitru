# RURU-112 — Bitbucket Cloud participant facet

Status: **bounded pre-code contract accepted by root**, 4 October 2026. The
participant implementation and forward migration are authorized within this
contract. Root signs it before source changes start; qualification remains pending.

This supplements `remote-collaboration-engine.md`, `remote-collaboration-backlog.md`,
`RURU-112.md` and `RURU-112-pulls.md`. Those documents retain the account/repository,
PR-feed and singleton Body contracts. This slice adds only a read-only native
participant facet. Tasks need their own subsequent contract; RURU-112 stays
In Progress after participant delivery until that remaining acceptance is met.

## Base and live audit

Managed worktree: `/Volumes/Lexar/.codex/wt/collab-ruru-112-participants/gitru`.
Branch: `ruru/ruru-112-bitbucket-participants`. Exact starting head:
`1766a6e7308375806aa637380ccf7caa2714669b`, the published, unmerged draft
[PR #161](https://github.com/ruru-m07/gitru/pull/161), stacked on #160. Preserve
both existing PRs. This document is the only initial edit in the new worktree;
fresh dependencies were installed with the frozen lockfile/copyfile backend.

Live audit on 4 October, approximately 13:41 UTC:

- [RURU-112](https://linear.app/catra/issue/RURU-112/add-bitbucket-cloud-account-and-pull-request-reads)
  remains In Progress and attaches #160/#161. Its RURU-76, RURU-100 and RURU-111
  prerequisites remain In Review; their implementations are in this reviewed
  stack. No merge is authorized.
- The parent RURU-53 participant search returns RURU-112 alone, with no duplicate
  participant issue; RURU-112 has no duplicate relation. No participant PR is
  attached. Re-audit before publication rather than assuming this snapshot stays
  current.
- Exact #161 head `1766a6e` reports nine successful checks/statuses. Windows Rust
  and Windows ordinary Desktop E2E are still In Progress. Linux/macOS Rust and
  ordinary E2E, frontend, Clippy/format, Cloudflare, CodeRabbit and Vercel pass.
  No exact-head CodeQL check is reported. Pending Windows checks are not passes.
- RURU-121/#158 remains In Review. Its working-set coordinator/navigation changes
  are a separate owner/branch. R103's retained fixture pipeline and R106's restore
  policy remain separate. Do not modify their source or silently expand their
  qualification claims.

R103 separately published `4054a6c`, with its own-webview revision listener change
in `packages/collaboration-client/src/index.ts`. This participant branch starts
at #161 and does not inherit that repair. Participant SDK edits in that overlapping
file are limited to exported types; leave revision listeners unchanged. A later
stack/integration audit must preserve both additions. Do not borrow R103's fresh
five-session macOS retained qualification for this branch or its new migration.

This audit distinguishes prerequisite implementation from merge/completion, and
ordinary packaged CI from live Bitbucket/token/keyring or retained-harness proof.

## Official sources and routing inference

Rechecked official [REST introduction](https://developer.atlassian.com/cloud/bitbucket/rest/intro/),
[PR API](https://developer.atlassian.com/cloud/bitbucket/rest/api-group-pullrequests/),
[OpenAPI](https://dac-static.atlassian.com/cloud/bitbucket/swagger.v3.json?_v=2.300.196),
[token permissions](https://support.atlassian.com/bitbucket-cloud/docs/api-token-permissions/)
and [changelog](https://developer.atlassian.com/cloud/bitbucket/changelog/).

The singleton PR contains embedded participants/reviewers; the collection omits
those expensive fields. Cloud has no separate participant GET in this reference.
The participant schema supplies `user`, `role`, `approved`, `state` and
`participated_on`. The latter describes an action, which may be an approval or
last comment. It is not an update clock for every participant property. The
schema supplies no reviewed commit OID. Preserve that uncertainty.

Use the existing fixed singleton route:

```text
GET https://api.bitbucket.org/2.0/repositories/%7B%7D/%7B<repo-uuid>%7D/pullrequests/<pr-id>
```

Atlassian documents immutable repository UUID addressing with literal empty
braces in the workspace segment. Extending that repository route to the PR child
remains a documented-hierarchy inference, not a live-provider qualification.
Use the existing adapter route builder and identity guard; mutable full names,
embedded self links and renderer strings never supply HTTP authority. Reject a
cursor/query/validator on this singleton request. Do not invent `/participants`
or fetch default reviewers, activity, comments or tasks.

Keep token-only sensitive Bearer authentication from #160/#161. The 18 August
2026 changelog's API-token Bearer support is newer than the introduction's older
Basic-auth description. This slice uses the existing optional
`read:pullrequest:bitbucket` grant; base account/repository probes and their three
minimum scopes stay unchanged. Desktop accounts remain independent of Gitru cloud.

## Scope and capability API

Add `DetailFacet::Participants` (`participants`) and
`ResourceFacet::Participants`. Admit it only for selected, observed Bitbucket
Cloud PR subjects. The Bitbucket profile advertises it; other providers and
inapplicable kinds remain explicitly unsupported. No task enum/profile admission
is needed in this slice. Existing Body, feed and account grants retain their
independent policies. A participant-scope 403 gates this facet; it must not revoke
another actor, repository discovery, unrelated Body or private authored drafts.

Use the existing command/query/hydration/demand envelope:

```ts
const query = {
  account_id: accountId,
  subject_id: "bitbucket_cloud:pull:<repo-uuid>:67",
  facet: "participants" as const,
};
// Existing SDK local detail query options / DetailQuery accept this new facet.
// An explicit existing hydrateDetail request creates native read intent.
```

No remote TypeScript client, renderer-authored provider observation, new secret
IPC, new scheduler, polling loop or unopened-facet hydration is introduced. Ordinary
local reads/subscriptions remain provider/vault-free. Existing demand ownership,
background scheduling, retries, account cooldown and privacy fences apply.

## Additive typed native payload

Preferred representation: add one nullable `native` member to `DetailEntry` and
define these concrete Rust wire types in a small participant model module. The
payload is provider-independent; source identifiers and native enum strings
preserve provider semantics:

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value")]
pub enum NativeDetailPayload {
    #[serde(rename = "participant.v1")]
    ParticipantV1(ParticipantV1),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParticipantUser {
    pub provider_id: String,
    pub login: Option<String>,
    pub display_name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParticipantV1 {
    pub user: ParticipantUser,
    pub role: Option<String>,
    pub approved: Option<bool>,
    pub state: Option<String>,
    pub participated_at: Option<String>,
}

// Add to the existing DetailEntry; existing generic entry fields remain.
#[serde(default)]
pub native: Option<NativeDetailPayload>,
```

The adjacent-tag pattern is explicit and versioned. It has no arbitrary response
dump, untyped map, token, URL authority or renderer-selectable extension name.
Legacy stored entries without `native` decode as None. Serialize None as JSON
null, rather than skipping the field: normal generated nullable schemas must
validate ordinary legacy-provider responses too. Existing provider/test entry
constructors need only mechanical `native: None` defaults.

For participant entries, `native` must be this v1 payload. The six generic value
members (`author`, `title`, `state`, Body, `updated_at`, `head_oid`) stay at their
empty/default values. Do not overload title/state, represent the record as a
review event, put participants in assignees, or synthesize reviewed-commit data.
Other facets cannot carry this participant payload or participant field tags.

For Bitbucket, `ParticipantUser.provider_id` is the required immutable actor UUID,
normalized lowercase without braces and never nil. `user.login` observes native
nickname, and `participated_at` observes native participated_on. No name/date
fallback grants authority. Native participant identity is independent of
nicknames/roles:

```text
DetailEntry.id = bitbucket_cloud:participant:<repo-uuid>:<pr-id>:<actor-uuid>
DetailEntry.provider_id = <repo-uuid>:<pr-id>:<actor-uuid>
```

Rows already partition by account/subject/facet. Validate identity lengths against
existing identifier bounds. The same actor in another PR/repository or under
another connected account cannot alias this entry. A changed role does not create
a new actor entry; changing the UUID behind a saved ID is an error.

### Individual masks and clocks

Extend `DetailField` with these six participant-only tags:

```text
participant_login
participant_display_name
participant_role
participant_approved
participant_state
participant_participated_at
```

The page source declares the fixed six-field set. Each entry's mask contains only
fields actually observed with valid values. Store owns validations and source
clocks, as for existing fields; adapters never submit validation timestamps.
Participant entries contain no generic field tags, and generic entries contain
no participant tags. Thus a valid entry still has at most six masks/validations/
private clocks; the enum grows, but the existing six-clock memory bound can stay.
Validate the legal mask/payload combination per facet, rather than accepting an
arbitrary mix of the twelve enum values. Legacy generic evidence remains valid.

| Observation | Payload/mask and saved authority |
|---|---|
| Field absent | No tag; initialize unknown if never saved, otherwise retain the saved value and its old field validation/clock. |
| `approved: false` | Some(false) plus approved tag; Known false, never omitted/falsy fallback. |
| `approved: true` | Some(true) plus approved tag; native flag only. |
| `approved: null` or a nonboolean | Reject the page; this is not an authoritative false. |
| `state: null` | None plus state tag; an explicitly Known absence, distinct from never observed. |
| Bounded native state/role string | Preserve its exact value and tag, including unknown future enum strings; do not invent normalized approval meaning. |
| Present null nickname/display/action date | None in common login/display_name/participated_at plus its tag clears that optional value authoritatively. |
| Role null, malformed nonnull text/date or invalid nested actor | Reject the page rather than grant authority. |

Field validation presence disambiguates a saved known-null from never observed.
The most recent entry mask disambiguates a newly observed field from an omitted
field whose older saved value remains. The renderer must use both; it cannot
infer approval unknown from `!approved` or claim retained flags were just validated
because collection membership is fresh.

Bounds: nickname <=255 bytes; display name <=1024; role/state nonempty <=128;
action timestamp <=128 and RFC3339 when nonnull. Reject controls in presentation
and enum values. Keep optional presentation separate from actor identity; the UI
can fall back to UUID for display without storing a fabricated nickname. Unknown
enum text is escaped text with an unknown-state fallback, never executable HTML.

## Adapter and collection authority

Dispatch the Participants facet through the existing singleton HTTP transport
into a separate mapper. Reuse compound PR identity validation, not the whole Body
mapper: bad/omitted descriptions or unsupported unrelated metadata must not supply
participant authority or block an otherwise valid participant observation.
Validate account/epoch, selected repository, subject compound ID/number before
HTTP; validate returned PR type/local ID and destination repository UUID before
mapping. Parse the array only from the singleton's `participants` field. Do not
union `reviewers` into it, infer approval from that array, or infer participants
from collection/author/comment data.

Require a JSON array with <=100 entries, each a valid participant/account object
with canonical actor UUID. Reject duplicate actor UUIDs, malformed entries or
an oversized array before applying any record. Preserve bounded unknown role/state
strings. Optional field omissions retain prior authority rather than fabricate
defaults. Keep the existing finite HTTP envelope (4MiB body, no redirects,
fixed origin, bounded URL and request deadline); no additional provider calls.

Absent, null or nonarray `participants` is a safe InvalidResponse with **no page
apply**. In particular, current generic detail storage treats every successful
non-Body page as Known and validates its collection; returning `entries: []` for
absence would falsely clear records and refresh coverage. Valid explicit `[]` is
Known empty; a fully validated nonempty array is a full singleton enumeration.
Use `DetailReconciliation::full_history()`, no next cursor/ETag/304, default Body,
no common metadata observation, and existing initial finite freshness (180 seconds).
This permits absence reconciliation only from a real complete valid array. It
can remove absent provider entries in this facet, never authored drafts.

Source: `bitbucket.participants.v1`, adapter_version 1. Runtime
sets observed/validation time. Both facet `provider_updated_at` and generic entry
`updated_at` remain None. Do not borrow PR `updated_on` or put `participated_on`
into update-clock ordering. Existing captured lease/run/source/view/epoch and
subject-binding guards must apply unchanged, including head changes while HTTP
is held. SubjectHistory does not claim that approval belongs to the current head;
UI labels native approval as provider state last observed at a time.

Successful mapping and safe failures retain positive observed quota through the
existing captured-actor/epoch budget path. A same-actor invalid response may
legitimately update durable quota/global revision; it must not replace provider
rows, coverage validation, metadata, grants or drafts. An obsolete-epoch success
or error must update neither data nor quota. No new terminal retry or rate-limit
policy is introduced. Unknown providers and unsupported facets never fall back
to a real provider request.

## Schema 0008 and recovery

Add `0008_participant_facets.sql`. Do not edit migrations 0001-0007 or their
checksums. Their observations and demand CHECK constraints admit only four names.
The new constraints admit those same four plus `participants`; tasks remain a
later schema/contract decision. Participant payload/evidence uses bounded JSON
in the existing detail-entry storage, with explicit typed validation.

Migration strategy under the pinned SQLx transaction and enabled foreign keys:

1. Create temporary-name replacement `detail_observations`, `detail_entries`,
   `detail_resource_metadata` and `detail_demand` tables. Preserve all columns,
   keys/defaults/checks and account references. New child foreign keys reference
   the **new temporary-name parent**, not the parent that will be dropped.
2. Copy observations first, then entries/Body metadata and demand with explicit
   column lists, verbatim.
   Preserve old JSON bytes, run IDs, authorization epochs, source clocks,
   validations, requested flags and intent identity. Do not reinterpret records
   or emit sync/change-log events during migration.
3. Drop old dependent metadata/entry tables and old demand before dropping their
   old parent. Rename the new parent and then its child tables to the
   original public names, relying on the pinned SQLite rename semantics to
   update those new FK references. Recreate any relevant indexes/triggers.
4. Check FK integrity and exact copied values/counts. Commit only the successful
   whole migration. Never disable FKs globally or drop the parent while retained
   old children could cascade away. A failure rolls back the table rebuild and
   ledger entry; reopening the original schema remains possible.

The exclusive Store owner and writer lease still protect bootstrap. Account,
credential metadata/cleanup, repositories, identities, feeds, drafts/generations,
runtime revision/view/log floor and change log stay byte/value equivalent. All
four old facets and their retryable detail intents survive. New nullable payload
defaults do not rewrite old entry JSON merely because migration opened the DB.

Require a **frozen v7 SQL/seed/ledger fixture**, independent of today's Rust DTO
serialization, containing all four old detail facets, entries, Body metadata,
pending demands, two actor grants and private drafts. Qualify exact upgrade and
FK behavior, a fault during the actual rebuild boundary/transaction, ledger and
data rollback, and immediate cold reopen with ordinary saved reads. Retain
existing v1 migration, writer-exclusivity and crash/rollback controls.

This slice makes **no R106 backup/restore ceiling claim**. Its known v1/v2 restore
allowlist/manifest policy remains unchanged, and no R106 files are edited. Schema
0008 supported Store bootstrap is separate from archive restore compatibility;
the latter requires its own later explicit contract and qualification.

## IPC, frontend and ownership

The existing DetailQuery/HydrateDetailRequest/DetailSnapshot/DemandTarget command
signatures remain the API; reachable enums/payload schemas change. Root runs
normal `make typegen`, never hand-edits `packages/commands`. The pinned typegen
0.4 plus `scripts/collaboration-bindings.ts` already needs source-derived Serde
corrections. Ensure the new tagged payload and Option/null fields match actual
Rust serialization exactly, through installed generated schemas/SDK tests. If
the native model is in a new file, add it to that source-derived correction input.
Any correction belongs in the generator source, with no handwritten remote SDK.

The ordinary detail panel adds Participants, selected through typed capability
policy. One typed payload renderer narrows to `participant.v1`, independent of
the account provider. Future adapters may produce only compatible facts they
actually observe; native role/state strings retain their source's meaning. There
is no semantic reason to require a Bitbucket-only renderer branch here. Render
actor presentation, role, approved true/false/unknown, explicit/unknown native
state and action time separately. Show saved/stale/denied/partial/error evidence
and older retained field validation truthfully. Do not offer approval/task/write
controls or infer merge readiness.

### Shown-panel query and native demand boundary

Audit the actual mount path before editing: SavedItemDetail retains Body demand,
and ResourceCapabilityPanels currently mounts its listed facet query panels.
Appending Participants to that always-mounted list would not prove unopened-facet
isolation. Add a collapsed participant section/header; only an explicitly opened,
supported participant panel mounts its inner local query and visible native detail
interest. The header can use existing contextual capability evidence without
reading participant entries or acquiring their lease.

Use the existing `useVisibleDemand`/native activity mechanism with the exact
Participants target only while the panel is shown and policy permits demand.
Native physical visibility/activity remains authoritative; never pass synthetic
visible=true or retain demand on a closed panel. Releasing/collapsing/unmounting
must dispose this interest. Opening a supported panel permits normal native due
work; it is not a renderer hydrateDetail call on mount. The existing explicit
Sync/Recheck action remains a distinct user command. Parent selection, Body,
hover/keyboard prefetch or merely displaying the collapsed header must not enqueue
participant hydration or retain participant demand.

The shown panel uses the common local query with limit100, covering the bounded
singleton without provider pagination. Display and gate it with the normal saved
read/evidence/capability policy. GitHub/GitLab/not-yet-admitted adapters and issues
cannot acquire Participants demand or cause a provider request; unsupported policy
is displayed or the inapplicable section is omitted. Grant/actor/subject changes
must retire old queries/leases under existing SDK fences; reset only the panel's
own open state where needed, never remount the independent private draft editor.
Unsupported or inapplicable Participants must not cause an implicit local facet
query either. Preserve the existing Body lookup/query and other panel behavior;
this new panel's capability gates do not broaden their access or mount policies.

The existing local query key includes account/subject/facet. Automatic private
prefetch, navigator, DemandCoordinator and R121 working-set bounds stay unchanged.
Add common SDK payload exports and ordinary renderer tests after the generated
DTO contract freezes. Test closed header, real shown interest, collapse cleanup,
hidden/denied/unsupported controls and switching accounts with a held local read;
differentiate local query calls, lease calls, explicit hydration and actual HTTP.

Owned file sets for assignment after signature review:

| Lane | Owned source |
|---|---|
| Core contract/storage | `crates/collaboration/src/detail.rs`, new `src/participants.rs`, `src/lib.rs`, `src/domain.rs`, `src/providers/registry.rs`, `src/storage.rs`, `src/storage/details.rs`, `src/storage/facet_reconciliation.rs`, `src/storage/contextual_capabilities.rs`, `src/storage/notification_subjects.rs`, new migration 0008, storage/capability/reconciliation tests and frozen v7 migration fixtures/tests. |
| Native provider | `src/providers/bitbucket_cloud.rs` dispatch/profile, new `src/providers/bitbucket_cloud/participants.rs` and participant actual-HTTP tests; limited shared singleton-identity helper extraction in `resource_details.rs` if needed. No discovery/feed fingerprint changes. |
| Independent Runtime qualification | New `src/runtime/bitbucket_participants_tests.rs`; root alone owns test-only registration in runtime.rs. Existing provider fixtures may get narrowly required empty participant fields only with coordinated ownership; do not suppress legitimate work. |
| Frontend | `apps/desktop/src/features/collaboration/resource-capability-panels.tsx`, new native participant renderer and its ordinary Workspace/bridge tests; `packages/collaboration-client/src/index.ts` payload exports and wire tests. No working-set/coordinator or R103 changes. |
| Integration/root | This contract review/signature, necessary mechanical native=None defaults in old provider/test constructors, source-derived typegen correction and generated outputs, serial checks, architecture/Linear evidence later, signed commits and one attached reviewable PR. |

The crate source paths in the table are relative to `crates/collaboration`; the
new model/module must be exported once. No overlapping edits: provider profile/
dispatch belongs to the native provider owner, registry to the core owner, and
all generated integration to root. Coordinate source freeze before any Cargo or
typegen. Do not change native caller policies or add commands unless an actual
contract gap is reviewed first.

## Meaningful acceptance and evidence boundaries

1. Actual local HTTP -> production adapter -> Runtime -> SQLite, with finite
   clock/fake vault: PR67 in two repositories and under two actors; identical
   nicknames/distinct UUIDs; repository/user rename with stable UUID entry IDs.
2. Real singleton identity/route verification; wrong PR/destination/actor, hostile
   URLs/cursors and malformed arrays cannot apply. Valid [] is Known empty;
   absent/null/nonarray/101 entries/duplicate actor causes no provider-data apply
   and cannot refresh old coverage or clear entries. Parent Body fields remain
   independent, including omitted/bad raw description alongside valid participants.
3. Approved true -> false, explicit state null, unknown native role/state,
   optional nickname/display/date omissions retaining old field validations,
   later known clearing, and malformed boolean/date controls. Actor identity is
   immutable. Changed action/parent timestamps never create ordering authority.
4. Held old-epoch 200 and 429/quota-bearing failures reject both data and budget;
   captured-head/source/access changes reject late apply. Test same-actor safe
   invalid-response positive quota without changed rows/grants/drafts, and
   participant 403 isolation from another facet/actor/repository.
5. Frozen v7 upgrade/rollback/FK data preservation and immediate cold reopen.
   Saved participant and ordinary legacy-provider reads make zero HTTP/vault
   calls; private draft generations/text remain exact. Preserve existing migration,
   exclusivity, account, quota and legacy detail evidence assertions.
6. Actual generated wire/SDK schemas accept native known false/null, distinguish
   omission through masks/validations, accept generic native:null and reject
   unsupported payload/mask combinations. Ordinary UI renders accurate native
   semantics and denied/offline/unknown controls without hidden provider requests.
   An unopened participant panel has zero participant hydration/demand/provider
   calls; supported shown panels use real native visibility and release leases on
   collapse. Unsupported providers/issues make zero participant provider calls.
7. Root runs scoped/focused native checks, full required checks, formatting/Clippy,
   normal typegen, meaningful frontend tests/lint/types/build. New-head remote
   Linux/macOS/Windows and security/status checks are separately recorded. No
   merge without user authorization; no personal credential/keyring/live-provider
   inspection for unattended tests.

Live route/token/provider behavior and new platform migration/native UI evidence
remain unqualified until actually exercised. Synthetic HTTP, generated IPC mocks,
ordinary packaged E2E and retained fixtures are different evidence classes.
The bounded 100-participant limit is intentional; large-singleton recovery is a
future policy gap, not proof of total completeness after truncation.

## Review decisions before implementation

Preferred additive payload reuses scoped entry storage and existing six-field
merge evidence. Dedicated typed participant tables/commands would isolate domain
records further but duplicate paging/privacy/lease envelopes and enlarge this
slice; they are not the proposed default.

The remaining integration decision is how the pinned generator emits the adjacent
tagged union. Root must qualify the source-derived generated schema correction if
necessary; it must not flatten the payload into strings merely for generator
convenience. The optional-field representation intentionally uses values plus
existing masks/validations, avoiding a second independent evidence API. Six legal
participant field tags keep the clock bound unchanged despite the wider enum.

If implementation reveals that those existing clock/mask or migration mechanisms
cannot preserve these invariants, stop dependent source changes and record the
specific gap here for root review. Do not silently expand into task pagination,
generic arbitrary native JSON, scheduler redesign, R106 restore support or remote
writes. All progress/evidence entries are pending until actual work is qualified.


## Root review clarification before source changes

SubjectHistory describes the observed participant set and never proves approval of
the current commit. Independently, every participant page supplies the existing
subject binding captured from the original request, including its known head tuple.
Validate returned singleton PR/destination identity separately. A head or other
subject-binding change while HTTP is held rejects late apply through existing
Store validation. Already saved subject-history participant observations are not
erased solely by a new head, and UI must not label them current-head approval.
No parent/action clock becomes a participant update clock.

Six participant tags are mutually exclusive with all six generic entry tags, so
each valid record's existing bounded six-clock visitor remains sufficient. Core
validation must enforce that legal facet/payload/mask combination explicitly; enum
growth alone must not permit mixed generic/native authority. Centralize the facet
list used for scope invalidation where practical, retaining all existing policies.
Production SDK own-Webview listener from separate R1034054a6c is not inherited
here; future stack integration must preserve that correction alongside SDK type
exports. No retained/native/global-target qualification is borrowed for this slice.


## Generated wire family guard decision — 4 October, before correction

Independent frontend review found that the first null-payload wire control used
a participant-only mask with native:null. That is an impossible Store-produced
participant shape; structural Serde/Zod types alone do not reject the family mix.
Acceptance6 requires a real generic null payload and disjoint payload/mask proof.

Root accepts a bounded source-derived generated DetailEntry schema refinement:
read the existing Rust DetailField::is_participant declaration, emit its six
wire names, and require masks and field validations to belong to that family
exactly when the typed participant payload is present. Generic native:null
entries keep generic fields. No manually authored SDK schema or new renderer
authority is introduced. The Store remains the semantic admission authority for
identity, values, clocks, facet/source/binding, bounds and atomic publication;
the wire guard additionally rejects the two impossible payload/family mixes.

Replace the null control with an ordinary generic entry, add actual red controls
for native:null plus participant fields and a native participant plus generic
fields (including validations), then qualify normal make typegen and installed
wire schemas. The current one-variant payload guard is intentionally bounded; a
future task payload/field family must extend this source-derived check explicitly
and qualify both existing families rather than silently treating every native
payload as a participant. Rust source and generated command signatures remain
unchanged. No platform or live provider claim follows from these wire controls.
