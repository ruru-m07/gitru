# RURU-77 — Cached pull request description and metadata

This slice builds on reviewed RURU-97/100 and supplies the shared resource-detail
contract consumed by sibling RURU-78. Rust owns provider HTTP, admission, field
authority and atomic SQLite publication. The frontend renders local snapshots and
submits explicit bounded hydration intent. Root coordinates IPC generation and
frontend integration; this worktree owns the native contract and GitHub PR adapter.

## Approved contract before implementation

- Keep the existing Body facet as one resource endpoint unit. A single GET pull
  response supplies description and typed metadata; both commit in the same
  account/instance/canonical-subject/epoch/authorization-view/run transaction and
  revision. No second metadata job or HTTP request is introduced.
- Add forward migration 0005 for rebuildable metadata, linked to the Body
  observation. Existing migrations, identities, aliases, authored drafts and
  credential journals are immutable. Metadata disappears with provider-cache
  grant cutover and is never copied into the authored draft partition.
- Return typed metadata values and per-field evidence. Common fields are title,
  state/reason, author, web URL, updated time, labels, assignees and milestone.
  PR fields include draft, merged time, head/base refs and OIDs. Actor, repository,
  label and milestone native IDs are strings; mutable names are presentation.
- Known null and known empty arrays/text differ from not loaded, omitted,
  oversized and unsupported. An omitted/oversized observation preserves a known
  saved value, its source clock and its validation time. No truncation becomes
  an authoritative empty result. Each metadata field has its own saved source,
  latest observation state, validation/freshness and comparable ordering clock.
- A matching whole-resource 304 validates only fields actually observed-known
  in the preceding representation. Known labels at T2 followed by labels omitted
  in a T3 body response remain validated at T2 through a T3 304. Equality tokens
  are never ordered; unknown/unmatched validators cannot manufacture content.
- Verify request and response native subject ID, resource kind, number and
  repository binding. GET issue representations marked as PRs are rejected by
  the Issue adapter. Native/named repository URLs are accepted only when they
  match the cached trusted repository. Transfers/path reuse require reconciliation,
  never a guessed identity or draft relabeling. PR base repository native ID is
  checked; a missing fork repository may retain the observed head ref/OID.
- Capture parent binding at dispatch and recheck at commit. A newer accepted
  list head change invalidates the Body run/validator and marks saved details
  stale without erasing them. Older summaries cannot overwrite detail authority.
  Detail reads project authorized saved metadata over summaries with truthful
  freshness; this does not imply fresh list membership/counts.
- Production GitHub advertises only PullDetails until the separate RURU-78 issue
  adapter lands. Comments/reviews/checks/diffs/remote writes remain unsupported.
  Local detail/capability queries never call HTTP or create demand.

## Bounds and ownership

Bodies retain the existing 1 MiB bound. The saved metadata snapshot has a 256 KiB ceiling,
100 labels/assignees, 16 KiB titles, 255-byte logins, 128-byte state/OID fields,
1024-byte refs and 2048-byte URLs. Optional oversized fields are explicit and
retain prior authority. Snapshot admission reserves 8 KiB for later validation
and observation changes, including thirteen fields' bounded 128-byte RFC3339
timestamps; a final persisted-size guard also applies to 304 updates. A growing
field is marked oversized as a whole, preserving its previous saved value and
ordering clock. The separately bounded source diagnostic is not body content.
Unknown additive states remain bounded strings.

Native lead owns resource metadata domain/storage helpers, migration 0005,
detail/runtime integration, GitHub shared request/mapping helpers and PR module,
PR/storage/runtime fixtures and this note. RURU-78 owns
`providers/github/issue_details.rs`, issue fixtures/tests and its note, with no
shared wiring until the shared contract freezes. Root owns generated bindings,
frontend coordination and publication. No live credentials/provider or personal
vault is used for verification.

## Primary API research

The [GitHub GET pull documentation](https://docs.github.com/en/rest/pulls/pulls#get-a-pull-request)
returns body, labels, assignees, milestone and branch information in one response.
The [GET issue documentation](https://docs.github.com/en/rest/issues/issues#get-an-issue)
distinguishes PR representations with `pull_request`; their issue ID is not the
pull object ID. Use the pinned 2026-03-10 API version and the existing restricted
transport/error/quota policy. [Conditional-request guidance](https://docs.github.com/en/rest/using-the-rest-api/best-practices-for-using-the-rest-api#use-conditional-requests)
binds validators to a cached representation. Nullable fork repository handling is
defensive resilience, not a claimed documented GitHub guarantee.

## Planned verification

Sanitized HTTP fixtures cover complete/null/empty/omitted/oversized metadata,
literal ETags and matched 304, wrong native identity/number/repository/kind,
unknown states, fork absence, provider errors and zero dispatch for IssueDetails.
Public storage/runtime tests cover atomic body+metadata, per-field ordering and
304 omission barriers, head-change late responses, offline/cold restart, denied
scopes and account/epoch isolation. Frozen v1-to-current upgrades must preserve
drafts/aliases/credentials. Local checks and exact-head remote CI remain distinct.

## Implementation evidence

The additive 0005 table stores only rebuildable typed resource metadata and is
linked to the existing Body observation. Description and metadata publish in
the existing fenced SQLite transaction before the revision hint. Production
GitHub performs one restricted GET pull endpoint request and advertises only
PullDetails. The shared request/mapping seam is ready for RURU-78's independently
owned issue adapter, which remains unwired in this slice.

The request binds account, canonical subject/native ID, kind/number, repository
native ID and the dispatched head. Responses validate the API URL and resource
web path against that binding; named and native repository API paths are
accepted without trusting a different origin. Present `html_url: null` fails
closed for GitHub's required URL; the common nullable WebUrl model remains
available to future providers. Unknown additive provider states are retained.
`merged: true` or a known merge timestamp records authoritative merged state
even when the raw state key is absent. Malformed present merge flags fail.

Every saved field keeps its own ordering clock. Omission, oversize, timestamp-less
observations and matched 304 cannot erase a comparable known clock. A new 200
that omits the entire optional metadata object records all fields latest-omitted,
so its later 304 cannot freshen the preceding representation's metadata. 304
refreshes only saved-known fields also observed-known by that representation.
Accepted summary head changes invalidate the validator/run even for old-schema
body-only caches. Newer saved description/title authority remains fresh when an
older head hint cannot validate those fields; selected details retain independent
field freshness rather than promising current list membership/counts.

Ten public storage regressions cover known-null description with rich metadata,
account partitioning, denied access/private draft retention, cold restart,
atomic parent-binding rejection, field/body clock barriers, omitted/entirely
missing metadata through 304, retained-field aggregate bounds and head-run
invalidation without creating local-read HTTP demand. The tight budget fixture
constructs a valid 262,048-byte candidate that the former 64-byte reserve would
admit, proves legitimate validation precision growth would cross 256 KiB, and
asserts the current whole-field oversized admission before successful 304.
This is a regression against the former policy, not a claim of an executed old
binary reproducer. Six private PR mapper/HTTP cases cover exact IDs above 2^53,
Markdown and nullable fork repositories, null/empty/omitted/oversized fields,
future states, identity/URL rejection, literal weak ETags, permission/rate errors
and zero HTTP dispatch for unsupported issue/comment facets.

The final frozen native suite passes 143 tests with two intentionally ignored
subprocess entry points (those entry points are invoked by their real crash
tests). This includes all twelve frozen-v1-to-current migration cases against
0005, the inherited credential crash/recovery checks and the new mapper/storage
cases. `cargo fmt --all -- --check` and `git diff --check` pass. Final all-target
native Clippy passes with warnings denied through the same serialized build
wrapper, with raw output at `/tmp/gitru-ruru77-native-clippy.log`. Full native output is captured
by validation session 80020. Normal `make typegen` completed through root's
serialized generator; no generated commands were edited manually. No live
provider or credential was used. Packaged native QA and exact-head remote CI
remain root-owned delivery checks.

## Frontend/SDK plan (frontend owner)

The shared PR/issue view will consume the Body snapshot's typed metadata without
provider-name branching. Local query reads remain cache-only. Metadata renders
independently of null or empty descriptions; authorized known detail fields
project the selected title/state/author/URL/draft/head over the saved summary.
Known null suppresses summary fallback. Labels, assignees, milestone and PR
head/base refs and OIDs retain explicit known-empty/not-loaded/latest
omitted/oversized distinctions, per-field validation and TTL freshness. A denied
or inactive Body facet suppresses its metadata as well as its description.

Selecting an eligible missing/stale Body will register a separate bounded
hydration intent. An SDK selection lease coalesces overlapping mounts and React
StrictMode replay for the same account/actor/authorization epoch/canonical
subject. Each lease admits at most one automatic attempt per observed summary
head binding; ordinary facet revisions, omissions and 304 events cannot rearm
it. Known native errors, running work and access denial do not cause automatic
retries. Explicit capability-guarded Sync/recheck remains available. Selection
retirement/reset releases bookkeeping; query reads never create hydration
demand. Freshness/continuous foreground renewal beyond this bounded selection
belongs to RURU-98.

The private editor stays outside provider detail gates and preserves its
inspected draft generation and dirty buffer for the same actor/subject. Actor or
subject switches discard that view's buffer. The separately published RURU-99
saved-draft recovery/editor extraction remains a later narrow merge gate.

Frontend/SDK owns shared metadata/header presentation, selection lease/query
hooks, consumer wiring and sanitized wire/UI/lifecycle tests. Native lead owns
all Rust/shared DTOs/migration and the PR adapter; RURU-78 owns its isolated issue
adapter; root owns generated commands/typegen and publication. Planned tests
cover immediate cached render plus separate intent, null/empty/omitted/oversized
metadata, head change, 304/no retry loop, StrictMode coalescing, access loss and
actor switches with delayed old snapshots, and private draft/CAS continuity.

### Frontend/SDK evidence

The generated metadata contract is consumed through the normal commands SDK;
root owns generation and corrected the Rust-qualified field name at its source.
SDK wire fixtures parse the actual generated schemas, covering all metadata
field/state enums, nested nulls, known-empty arrays, optional fork repository,
unknown bounded provider state strings and IDs beyond JavaScript's exact integer
range. Cache-only reads and separately epoch-captured hydration remain distinct.

Local frontend evidence on the shared RURU-77 source:

- All 51 SDK tests pass: 25 client, seven wire, four selection-lease, plus the
  existing authorization/revision/deadline suites. The client-owned coordinator
  retains at most 128 active/grace selections; departures retire after one task,
  covering StrictMode replay without a visited-resource history. Account resets
  and bridge teardown deactivate handles and clear timers. Mutated caller
  objects cannot change the captured actor/epoch/subject request; late receipts
  are fenced. Failures/304/omission revisions cannot rearm an automatic binding.
- All 208 desktop tests pass. Ten cached-detail workspace cases use the actual
  singleton bridge and StrictMode: cached render while a separate intent is
  pending; fresh old-schema body with missing metadata; partial/unknown omitted
  body; no persistent oversized retry; authoritative null/empty issue body with
  metadata; 304/omission no loop; accepted head change; access-reset header/body
  suppression with private text/CAS preserved; delayed actor-A body after a
  same-subject switch to B. Five metadata cases check known-null projection,
  state/draft truthfulness, independent omitted/oversized validation, issue/PR
  distinctions, and 100 duplicate/name-only 1024-byte labels without key warnings.
- SDK/desktop types, scoped Biome and production frontend build pass. Raw logs
  are `/tmp/gitru-ruru77-{sdk-tests,desktop-tests,sdk-types,desktop-types,sdk-lint,desktop-lint,production-build}.log`.
  After root's closed-PR badge/state-reason review correction, the changed
  metadata/detail cases are rerun separately in
  `/tmp/gitru-ruru77-post-review-ui.log`; broad suites are not repeated solely
  for unchanged views.

Metadata warnings remain visible, while detailed header validation uses a coss
disclosure. Every displayed validation is a timestamp rather than an ongoing
freshness promise; no per-field polling/timer is added. Long labels wrap within
the detail width. Known false draft status says ready for review only for an
open PR; closed/merged PRs are explicitly not drafts. State reason is shown for
issues or a provider's actually known PR observation.

The frontend owner prepared `/tmp/gitru-ruru77-qa/seed.rs`; root subsequently
compiled, seeded and launched it for the native qualification below. It requires a new task-owned
`com.ruru.gitru.ruru77.qa` database, uses public Store APIs and matching Body
subject bindings, and creates two synthetic actors with the same canonical
repository/PR/issue IDs, four authored drafts and known metadata. A has a known
null PR body and 100 labels (including an unbroken 1024-byte name and duplicate
IDs/names); B has Unicode full descriptions. Both actors have 2099
`provider:rest` barriers and zero credential references. Root owns build/run/CUA
qualification after the native target is released. No live credentials are used.

Selected detail authority does not rewrite cached list membership/counts; lists
wait for their normal bounded reconciliation. Continuous foreground renewal is
RURU-98. This source preserves explicit-save private editor/CAS behavior; durable
unsaved navigation retention and the RURU-99 sibling extraction remain separate
merge gates. Native provider/core evidence and exact-head remote CI are recorded
by their owners; this frontend evidence does not claim them.


## Root native qualification and delivery

Root built the isolated debug/E2E app with identifier
`com.ruru.gitru.ruru77.qa` and exercised its actual macOS WKWebView. The final
app executable SHA-256 is
`acef940a6b6f5f81097ef100dbca63087f9fb86d99ff4f346466fd7fd6ed0ed3`.
Its task-owned database has two synthetic actors, four metadata records and four
private drafts, zero credential references and zero cleanup journal entries.
The E2E build uses the in-memory test vault, disables GitHub CLI credential
import and seeds future provider quota barriers. No personal credential, live
provider account, production database or production keychain was inspected.

Cached PR selection renders endpoint title/author, open or merged state,
head/base OIDs, milestone and labels immediately. Actor A's known-null body
retains rich metadata and the explicit no-description state. One hundred labels,
including duplicate identities and a 1024-byte unbroken label, exposed a real
fixed-height Badge wrapping overlap. Root corrected only these label badges to
use automatic height, reran all fifteen changed UI cases plus types/lint,
rebuilt the app and visually confirmed wrapping without overlap. The final
build log is `/tmp/gitru-ruru77-qa-native-build-label-fix.log`.

Native account switching removes actor A's provider content and unsaved buffer;
actor B displays its Unicode body, merged metadata and separate draft. B's
explicit Save persists the new text at generation 2. Returning to A restores
A's saved generation-1 text, without autosaving its discarded buffer. Read-only
SQLite inspection after native Quit confirms exactly those generations/texts,
unchanged issue drafts, four metadata rows and zero credential references or
cleanup entries. The task's WebDriver port is released after Quit.

Issue selection in this PR keeps GitHub IssueDetails unsupported: the saved
full body/metadata is suppressed, separately authorized summary and private
issue draft remain visible, and Sync is disabled. Future quota barriers disable
PR sync controls while cached reads remain usable. The child tab's Accounts
button opens the account dialog in the same native window, showing manual PAT
and explicit CLI-import controls; no token or account mutation was submitted.

Three native caller-policy tests pass with the E2E feature enabled; raw output
is `/tmp/gitru-ruru77-caller-tests.log`. Debug/E2E packaging passes with updater
artifact signing disabled only in the temporary QA configuration. Product
release signing configuration is unchanged. Source review, 143 native tests,
51 SDK tests, 208 desktop tests, generated IPC and local checks are complete.
The signed review branch is ready for draft PR publication; exact-head remote
CI is pending publication. Live GitHub and production vault qualification,
RURU-98 continuous foreground leases, RURU-99 authored editor integration and
RURU-106 newer-schema restore qualification remain separate gates.
