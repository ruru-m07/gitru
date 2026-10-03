# RURU-100 — Contextual collaboration capabilities

Status: In Review in draft [PR #149](https://github.com/ruru-m07/gitru/pull/149),
3 October 2026. Local implementation and native fixture QA are verified;
current-head remote CI/security remains pending. Started on reviewed RURU-76
`f5ae068` / PR #146, integrated
signed RURU-97 `64125441` before shared wiring, then signed-rebased onto
`4a1e46698e188ed56d3788cfa243bdefb12bcecd` after its parent fixture/security
corrections. This document is the continuation point for the complete
issue; an account-only inbox change does not satisfy its acceptance criteria.

## Contract and ownership

Native capabilities have three explicit targets: account, canonical repository,
and canonical resource. The account-bound request includes the inspected
authorization epoch. Rust validates target shape, account/instance/resource
ownership and canonical kind. A local SQLite snapshot binds account state,
instance, scope access, resource visibility, revision and authorization view.
Provider declarations are combined with this local evidence without HTTP or
vault access. Capabilities describe observed availability; command dispatch
continues to recheck identity, authorization and provider support.

RURU-100 owns a separate contextual capability module/types and storage evidence
reader, native capability tests, the shared frontend policy/boundary components,
workspace/sidebar consumers and divergent sanitized fixtures. It adds no schema
migration, provider detail endpoints, outbox or write delivery.

RURU-97 owns migration 0004, independent detail DTOs/storage/coverage, provider
facet hydration, scheduler admission and detail client bindings. Agree a local
read-only evidence accessor with that owner before integrating detail coverage.
Do not copy detail schema or make assumptions about its representation. Shared
command registration, crate re-exports and generated binding integration happen
after its contract freeze, with one coordinated `make typegen` run. Never edit
`packages/commands` by hand. Record integration commits and evidence below.

## Saved reads, synchronization and remote writes

Each facet distinguishes reading saved data, requesting synchronization, and
changing remote data. An adapter's implemented read operation does not grant a
write operation. Current remote writes remain explicitly unsupported until
durable delivery is implemented; a PAT or successful login grants nothing by
itself. Read-only presentation means saved reads are available while remote
writes are unavailable, with the native reason retained.

Network errors and rate limits do not hide still-authorized saved content or
disable local queries. They affect remote sync availability and status copy.
One reader transaction captures both facet and account-wide `provider:rest`
barriers, using one eligibility time for all facets. The later future deadline
blocks synchronization; an expired or absent transient barrier permits an
explicit native-checked retry without erasing saved/error evidence. One
bridge-owned SDK deadline coordinator observes contextual cache events and
cancels/invalidates expired local projections. It uses one capped timer,
weak query bookkeeping, consumes unchanged deadlines during delayed reads,
checks cache identity after cancellation, and stops on bridge teardown. It does
not poll provider APIs or install per-panel timers.
Actual account/scope denial prevents private provider reads. Unsupported adapter
operations dispatch no list hydration, refresh or action request. A permission
denial can expose a separate explicit recheck intent, subject to the existing
native coalescing, page, quota and cooldown limits; no automatic loop retries
denied scopes. The recheck is not a remote write or permission grant.

Adapter support, authorization and data observation are independent. Preserve
supported/unsupported/unavailable with bounded reason codes; unknown policy is
unavailable, never inferred support. Facet content distinguishes not yet loaded,
partial, complete, authoritatively empty, omitted, and oversized separately.
Reviews/checks retain explicit unsupported behavior until their readers exist.
The UI does not fabricate content or successful action controls.

## Frontend behavior

Account/epoch/canonical-target query keys and final RURU-76 cancellation before
invalidation fence delayed metadata reads. Queries are native local reads with
no per-component HTTP/polling. Authorization changes and same-epoch scope
changes refresh capabilities; delayed responses cannot replace a newer view.

The ordinary workspace is shared across divergent providers. Typed inbox
semantics choose native notification unread/all filters versus to-do
pending/done/all filters and badge meaning. Replace the legacy
`notifications_supported` gates in both workspace and sidebar. Repository
discovery/selection/refresh and resource reviews/checks/future operation
availability use contextual evidence rather than provider-name branches.

Shared boundaries distinguish unsupported, denied access, content not saved on
this device, and read-only states. Authorized saved content remains visible
during temporary remote failure. Private draft editing/recovery is outside
provider gates. Do not change GitHub's explicitly supported PAT/CLI connection
flow or promise live GitLab/Bitbucket authentication from fixture behavior.

Safe account/installation metadata remains continuous through local metadata
reloads so the same actor/subject editor can stay mounted. Provider query keys,
capability policies and private provider content are reset and fenced normally.
Grant epoch changes reset provider context while the same actor's private text
retains its inspected generation. A reread showing another editor's newer draft
offers explicit conflict/reload handling; it cannot silently advance the dirty
editor's expected generation. Actor or subject changes discard the prior editor
buffer. This adds no autosave or global unsaved editor registry. The already
published RURU-99 authored-cache/editor extraction will require a narrow merge
reconciliation: retain its saved-draft recovery/copy/export/CAS behavior and apply
these provider-independent mount boundaries to the shared SavedDraftEditor.

## Verification and delivery gates

Use only task-owned synthetic databases/accounts/vaults. Native tests must prove
target isolation, expected epoch fences, transactionally consistent local
evidence, zero HTTP during capability reads, unknown/unsupported write policy,
saved reads during network/rate failure, denial and explicit bounded recheck.
UI/client tests must exercise the same feature components with GitHub native
notifications, GitLab to-dos, and unsupported inbox/issues fixtures. Unsupported
requests issue zero calls. Cover account/epoch/target switching, initial pending
metadata races, permission updates, offline reads and authored-text retention.

Run relevant native/client/UI tests, lint/types/build and fixture-based native
QA when available. Every shared-target Cargo/typegen/E2E command uses
`/tmp/gitru-cargo-serial.py` with the root target. Reserve port 4445 with root
before packaged QA. Local validation, remote CI and live integration are
separate gates. Sign only scoped commits; root owns publication and Linear.

## Implementation/evidence

The signed RURU-97 handoff is integrated. Normal `make typegen` generates 96
commands; source-derived corrections include the separate contextual DTO module.
The provider trait default now declares no inferred capabilities or inbox model.
GitHub and synthetic adapters declare their profiles explicitly. The atomic
context reader uses RURU-97's metadata-only accessor and `saved_empty`; it never
copies provider descriptions into capability responses.

Local evidence uses only synthetic accounts, stores and vaults:

- All 127 collaboration tests pass on the integrated RURU-97 base under the
  shared-target serial wrapper;
  two subprocess entry points remain intentionally ignored by the normal runner.
  All 12 new contextual cases cover ownership/epoch/kind/instance fences,
  concurrent cutover snapshot coherence, inherited discovery and summary denial,
  inactive canonical identities, unknown profiles and zero HTTP/vault/durable
  hydration admission, detail empty/omitted/oversized evidence, successful-page
  cooldown without an error, quota-only idle detail barriers and expired/absent
  explicit retry eligibility. The full final run includes the single captured
  eligibility time and inherited exact CLI-expiry regression; all-target
  collaboration Clippy passes.
- All 44 SDK tests pass, including generated contextual wire/nullability/string
  identities, initial pending same-epoch invalidation, epoch reset and target
  isolation, and five shared deadline cases using real QueryObservers.
- All 193 desktop tests pass. Twelve ordinary-workspace regressions exercise
  notifications/to-dos without legacy provider flags, unsupported zero calls,
  explicit denied-scope recheck, offline/cooldown reads, idle saved feed with
  account-wide cooldown, authoritative null/empty descriptions and empty
  reviews/checks, delayed account/draft authorization reset with preserved
  inspected CAS and dirty text, and B-only admission after actor switching with
  a delayed A policy response. Four shared policy/boundary tests pass.
- SDK/desktop types and scoped Biome pass; production frontend and isolated
  native debug/E2E builds pass. All three native collaboration caller-policy
  tests include the new command. All-target collaboration and desktop-native
  Clippy with warnings denied and Rust formatting pass.

The cross-actor jsdom picker test narrowly overrides exact unsupported
`:modal`, `:fullscreen`, and `:popover-open` matches, delegating every other
selector. Independent CPU profiling found NWSAPI recursion through Floating UI
top-layer detection; it was not a React/provider render loop. The test keeps real
user-event picker interaction. Native WKWebView QA qualifies browser behavior.
Dynamic inbox Select instances are keyed by their semantics so Base UI does not
retain an obsolete filter collection across the initial policy observation.

Both independent native/deadline and frontend/client reviews accepted the final
source. No migration, live provider endpoints, remote write delivery or production
GitLab/Bitbucket authentication is added. Detail collection rendering is a bounded
first-50 preview; it advertises remaining saved entries rather than pretending to
show the complete collection. Save remains explicit, and navigation can discard
unsaved text; durable composer retention belongs to the separate composer work.

Native QA uses `/tmp/gitru-ruru100-qa/Gitru RURU-100 QA.app`, containing the frozen
debug/E2E executable SHA256
`eff3e868574040e8b3fb60f227fc55806c55d7d1954a2f191ff62532862d8c91`.
Its identifier is `com.ruru.gitru.ruru100.qa`; the exact Store seed source is
`/tmp/gitru-ruru100-qa/seed.rs`. The task-owned database contains only two
synthetic active actors with the same repository/subject IDs, two summaries and
two authored drafts. It has zero credential references/cleanup rows and a 2099
`provider:rest` barrier for each actor. The E2E feature uses an in-memory vault and
disabled GitHub CLI. The reset environment is unset; normal product data and
keychain credentials are not used.

Root-operated CUA on that frozen executable passed the actual WKWebView flow:
A's saved summary appeared immediately; read-only badges, disabled Refresh and
2099 paused-until/saved-data copy matched the account-wide barrier. Full
description/Comments/Reviews/Checks showed explicit unsupported states and
disabled Sync; the independently seeded description remained hidden under the
current GitHub declaration. Merge stayed disabled. Native picker Down/Return
switched A to B with no A summary or unsaved buffer, and displayed B's saved
summary/private draft. B's edited Unicode draft saved locally; Up/Return back
to A restored A's original saved draft. Unsaved A did not autosave or cross
accounts. Accounts/PAT/CLI dialog worked in that same window. Actual native Quit
was used; the known QA PID exited and port 4445 was released. Read-only SQLite
after quitting verified A generation 1 original, B generation 2 saved Unicode,
both actors still active, zero credential/cleanup rows and unchanged 2099
barriers. This qualifies synthetic cached/common-policy behavior, not live
GitHub authentication or provider APIs.

Root owns publication, current exact-head CI/security review and Linear status.
This local evidence does not claim remote matrix completion or live-account QA.
