# RURU-78 — Hydrate and render cached issue details

Status: native issue adapter, frontend and isolated macOS native IPC QA are
locally verified. Exact-head remote CI starts after draft PR publication. This slice is
stacked on signed RURU-77 `7e282adced13fdd1da99b937081fa76ff5f26c95`.

The issue detail endpoint feeds the existing account-bound detail engine. Opening
or rereading a saved issue remains a local SQLite query. Explicit hydration uses
the engine's durable demand, native scheduler, account quota, credential owner
and commit-before-event path. There is no second issue cache or TypeScript HTTP
adapter.

## Ownership and integration sequence

RURU-77 owns the common resource metadata observations/snapshots, forward schema
0005, transactional storage, shared GitHub resource helpers, PR adapter, common
runtime changes and generated contracts. This issue owns
`providers/github/issue_details.rs`, its synthetic endpoint fixtures/tests, small
`github.rs` module/profile/Body-Issue dispatch wiring and this note. The shared
DTOs, bounds and helper signatures froze in the signed RURU-77 base before mapper
compilation. The root coordinates stacking, native IPC QA,
frontend ownership, generated bindings, publication and remote CI.

The agreed common direction keeps `DetailFacet::Body` as the unit for a resource
endpoint returning both authoritative description and typed metadata. Body and
metadata commit under one existing actor/instance/canonical subject/kind/run/
epoch/authorization-view fence and one revision. Metadata fields independently
retain saved value, latest observation state, source, comparable timestamp and
validation time. The approved native contract has a typed
`ResourceMetadataObservation` on `DetailPage`, and a saved
`ResourceMetadataSnapshot` on `DetailSnapshot`. `MetadataObservedField` records
each observed field's `DetailValueState`; `MetadataFieldEvidence` retains the
saved and latest observed states independently. `MetadataSource` records the
endpoint, adapter version and provider timestamp; the engine owns observation
and validation times. The issue adapter's common seam is
`normalize(&DetailRequest, &[u8], source) -> (DetailValue,
ResourceMetadataObservation)` with a typed provider error.

Known endpoint metadata supplies the selected issue view. Summary list fields
retain their separately observed feed truth until normal summary reconciliation;
hydrating a detail endpoint does not blindly rewrite the list projection.

## GitHub endpoint and identity boundary

Use `GET /repos/{owner}/{repo}/issues/{issue_number}` through the existing
credential-safe GitHub transport and pinned API version `2026-03-10`. The
[official endpoint reference](https://docs.github.com/en/rest/issues/issues?apiVersion=2026-03-10#get-an-issue)
and [pinned OpenAPI description](https://github.com/github/rest-api-description/blob/main/descriptions/api.github.com/api.github.com.2026-03-10.json)
define this response. The description was inspected on 3 October 2026; no live
provider account or token was used.

Validate the request's account, public GitHub installation, selected repository,
canonical issue kind, native issue ID and positive repository-local number.
Response identity must match that request before any field becomes authoritative.
The issue API can also return a pull request, whose issue-side native ID differs
from the pull endpoint identity. A `pull_request` representation must never be
installed as canonical issue details or silently reclassified by number.

Only exact expected named and immutable repository-ID API paths may receive
credentials or satisfy response URL checks. Named and native-ID repository URL
forms are recognized by the shared validator. Wrong repository, transferred
issue locator, kind, number, ID, installation, redirect origin or response path
fails closed and requires normal repository/identity reconciliation. A repository
name does not establish a new immutable repository identity. Web destinations
also require the shared bounded safe URL policy.

## Field authority

The mapper preserves JSON key presence rather than collapsing absent keys into
nullable defaults. The common contract provides typed observations for:

| Endpoint field | Authority and absence |
| --- | --- |
| `body` | Raw Markdown; explicit null/empty is known absence/empty, missing is omitted, content above the existing 1 MiB body bound is oversized. |
| `title`, `state`, `updated_at` | Bounded endpoint values, with a valid comparable resource timestamp. Native state values remain additive. |
| `state_reason` | Bounded nullable reason, including future additive values. |
| `user` | Typed immutable actor ID and mutable login; explicit null is known absence, missing is omitted. |
| `labels` | Empty array is authoritative empty. Object labels retain native ID; name-only labels have no invented ID. Missing is omitted. |
| `assignees` | Typed actors; empty array is authoritative empty, missing is omitted. |
| `milestone` | Typed bounded milestone; explicit null clears it, missing is omitted. |
| `html_url` | Validated destination for the same resource, with a bounded value. |

Metadata field and collection limits come from the frozen shared contract. The
agreed limits are 16 KiB for titles/milestone titles, 128 bytes for state/reason,
255 bytes for actor logins, 1024 bytes for label names, 32 bytes for label colors,
2048 bytes for URLs, 100 labels/assignees and 256 KiB
for aggregate native metadata. Actor, label and milestone IDs are exact native
integers converted to decimal strings, including IDs above JavaScript's safe
integer range. Name-only labels have a nullable native ID. Limits
must be checked before persistence and must not silently truncate a collection
or claim an unobserved field is empty. Invalid required identity/shape is a safe
typed provider error. Oversized optional content remains explicit according to
the common field observation policy. The aggregate bound drops an entire largest
collection and marks that field oversized until the observation fits. It never
truncates a collection or presents that omission as known empty. Raw responses
and credential-bearing diagnostics never enter SQLite or IPC.

Omitted or oversized observations preserve saved field values and their ordering
evidence without freshening them. Summary-list body, author or state is not
promoted to authoritative endpoint metadata. A known field at T2 followed by an
omitted field at T3 must still reject an older comparable value at T1.

## Conditional requests, access and restart

Use the existing whole-resource validator/source lease; there is no issue detail
pagination or continuation cursor. A 304 must correspond to a known matching
endpoint/source/version and stored validator. It can refresh only fields the last
response actually observed as known. In particular, known labels at T2 followed
by a T3 response that omits labels and a 304 for T3 must leave the labels' T2
validation time and comparable clock unchanged. GitHub documents authenticated
[conditional requests](https://docs.github.com/en/rest/using-the-rest-api/best-practices-for-using-the-rest-api#use-conditional-requests).

Existing transport outcomes retain provider backoff and successful-response
cooldowns. Permission/not-found responses suppress private detail reads; they
must not become an authoritative deletion. GitHub can return 404 for inaccessible
private resources, as its [troubleshooting guide](https://docs.github.com/en/rest/using-the-rest-api/troubleshooting-the-rest-api#404-not-found-for-an-existing-resource)
explains. Transient offline/rate/provider failures retain authorized saved
description and metadata. Reauthentication, revocation, selection loss and late
responses retain the engine's existing native fences and authored drafts.

After SQLite restart, saved description, field truth and freshness remain local;
durable explicit demand follows the same bounded scheduler and persisted quota.
Opening the local query never requires HTTP, credential lookup or a live CLI.

## Verification and delivery gates

1. Synthetic HTTP fixture tests inspect exact endpoint, version, credential
   origin and conditional headers; cover native IDs above JavaScript's safe
   integer range, canonical issue/PR distinction and response identity failures.
2. Mapping cases cover known Markdown, null/empty/omitted/oversized description,
   deleted/null author, string/object/empty labels, empty assignees, absent/null
   milestone, additive state/reason, malformed fields and collection bounds.
3. Native engine tests cover metadata/body atomic commit, retained field clocks,
   304 omission behavior, actor/instance/epoch/run isolation, denial versus
   transient saved reads, conditional cache after restart, quota preservation
   and zero-HTTP local reads. Reuse existing public engine boundaries.
4. After RURU-77 stacking and agreed source freeze, run focused endpoint/core
   cases and full collaboration tests, all-target Clippy, formatting and generated
   contract checks through the shared validation reservation.
5. The frontend delivery gate is a cached issue detail surface consuming the
   native snapshot and contextual capabilities, with truthful field/body states
   and an explicit hydration intent. Root coordinates that integration and QA.

Comments, timelines, review/check/diff facets and remote actions remain separate
issues. Divergent provider fixtures demonstrate common semantics; they do not
qualify live GitLab/Bitbucket detail adapters. RURU-106 restore keeps its explicit
accepted-schema policy until its own review covers schema 0005. Local success,
new-head CI, packaged desktop QA and live provider qualification will be recorded
separately; none is currently claimed for this issue.

## Implementation evidence

The issue mapper uses the concrete RURU-77 helper seam:
`object`, `validate_identity`, `body`, `normalize_common` and `bound_metadata`.
It rejects every `pull_request` key and any non-issue request before returning
typed description and metadata. Its endpoint source is
`github/issue-detail/2026-03-10`, adapter version 1. GitHub now advertises
IssueDetails alongside PullDetails and dispatches Body requests by their
canonical kind. Notification body, comments, reviews and checks remain
unsupported and issue no HTTP requests. The inherited PR test's obsolete
unsupported-Issue assertion was replaced with those still-unimplemented kinds
and facets; the issue's own endpoint test verifies supported profile and actual
production dispatch.

Eleven private synthetic tests pass in the owned module, including actual
localhost HTTP requests and a public Store sequence for known labels, omitted
labels, matching 304, cold reopen and rejection of an older response. The older
commit leaves revision and saved truth unchanged after the engine's intentional
dispatch revision. Accepted fixture sockets explicitly use blocking reads with
a bounded timeout; request actions are not retried. The two
checked-in fixtures contain synthetic actors/resources and integer IDs above
2^53.

Local native verification on 3 October 2026, using the execution-wide shared
Cargo lock:

- Focused issue mapper/endpoint/storage sequence: 11 passed.
- Full collaboration suite: 154 passed, two subprocess entry points intentionally
  ignored as standalone cases and exercised through their real crash callers.
- All-target collaboration Clippy with warnings denied, workspace Rust formatting
  and whitespace checks pass.

Logs are `/tmp/gitru-r78-issue-focused-final-2026-10-03.log`,
`/tmp/gitru-r78-collaboration-full-2026-10-03.log`,
`/tmp/gitru-r78-collaboration-clippy-2026-10-03.log` and
`/tmp/gitru-r78-collaboration-format-2026-10-03.log`. Native signatures, DTOs,
migrations and generated bindings are unchanged by this issue. No personal
provider account, token, vault or production API was used. Packaged native IPC
QA, desktop/frontend evidence and exact-head remote CI are tracked separately.


## Frontend validation

<!-- RURU-78 frontend owner: scoped plan and evidence. -->

RURU-78 uses the shared RURU-77 issue/PR view, account-bound SDK, selection
intent coordinator and generated metadata contract from signed base
`7e282adced13fdd1da99b937081fa76ff5f26c95`. This slice adds acceptance
regressions in `cached-issue-details.test.tsx`; it changes no shared component,
SDK, IPC command or generated binding. Native issue normalization and transport
remain owned and verified separately by the issue adapter workstream.

Seven issue-specific integration cases exercise the real query bridge and
shared workspace under React StrictMode with fail-closed mocked native commands:

- Known null and empty descriptions retain issue metadata; known absent state
  reason/milestone and empty label/assignee collections render their explicit
  absence. PR branch, review, check and merge controls are absent.
- Unicode description/labels and additive provider state/reason values remain
  readable during a provider cooldown while synchronization stays disabled and
  explains the barrier. Reads do not admit a hydration intent.
- A retained issue reason and Unicode description survive a latest omitted
  observation and a later validation revision, with visible omitted/stale
  evidence and exactly one selected hydration intent. These are SDK/UI snapshot
  tests; the native suite separately proves actual conditional 304 persistence.
- Permission loss through an authorization-view reset and an unavailable native
  snapshot representing inactive membership both hide issue header/metadata and
  description. The same actor/subject private editor retains its text and saves
  with its inspected generation `3`, rather than taking a later query generation.
- Switching the same canonical subject to another actor/epoch keeps only actor
  B's issue reason, Unicode text and private draft; a delayed actor A snapshot
  cannot repopulate the view or transfer the dirty private editor. The narrow
  jsdom top-layer-selector shim follows the shared picker test; it does not
  replace native WKWebView interaction evidence.

Validation on this worktree:

| Check | Result | Log |
| --- | --- | --- |
| New issue + existing shared cached-resource/metadata tests | 22 passed across 3 files (7 new issue cases) | `/tmp/gitru-ruru78-issue-ui-tests.log` |
| Complete desktop suite, one frozen-source run | 215 passed across 29 files; no harness/order contamination | `/tmp/gitru-ruru78-desktop-tests.log` |
| Desktop and E2E TypeScript checks | Passed | `/tmp/gitru-ruru78-desktop-types.log` |
| Scoped Biome check | Passed | `/tmp/gitru-ruru78-ui-lint.log` |
| Frozen-lockfile dependency installation with copyfile backend | Passed; no dependency files changed | `/tmp/gitru-ruru78-bun-install.log` |

Read-only review of the issue module/profile integration found no material
frontend contract mismatch: IssueDetails is explicitly supported; the issue
normalizer validates canonical issue identity and rejects every present
`pull_request` marker; unsupported notification/detail facets retain their
zero-dispatch boundary. This is source review, not live GitHub qualification.

A task-owned synthetic native QA source is prepared at
`/tmp/gitru-ruru78-qa/seed.rs` with application identifier
`com.ruru.gitru.ruru78.qa`. It refuses an existing database and creates two
synthetic active actors sharing one canonical repository and PR/issue IDs, with
separate cached descriptions, metadata and private drafts. Actor A's issue has
a known null description/reason and empty labels/assignees/milestone; actor B's
closed issue has `not_planned`, Unicode text/label/milestone. Both account-wide
`provider:rest` barriers remain in 2099 and no credential reference is created.
The seed uses public Store APIs with matching body/metadata observation sources
and subject bindings. It is formatted, but the frontend workstream has not
compiled, executed or launched it; root owns native QA and its later evidence.

Continuous foreground renewal, issue list-summary reconciliation, remote writes
and the RURU-99 sibling merge gate retain the boundaries documented in RURU-77.
The issue view adds no duplicate data pipeline or React fetch loop.

<!-- End RURU-78 frontend owner section. -->


## Root native qualification

The isolated debug/E2E bundle `Gitru RURU-78 QA.app`, identifier
`com.ruru.gitru.ruru78.qa`, was built and exercised through its actual macOS
WKWebView. Executable SHA-256:
`2935f48e866c00d17f08944e8821c309168e99c67224664374a1aea38e732e42`.
The task-owned database uses two synthetic actors, four cached subjects/metadata
records and four private drafts. It has zero credential references or cleanup
entries; future `provider:rest` barriers block HTTP until 2099. The E2E vault is
in-memory and CLI credential import disabled. No personal provider credential,
production database or keychain was accessed.

Actual cached issue selection immediately renders A's endpoint title/author,
known-null description and known-empty labels/assignees/milestone/state reason.
Quota disables sync while saved detail stays available. Native screenshot review
confirms the metadata/body layout. Switching after editing A's unsaved draft
removes A's provider data/buffer; B shows independently cached closed state,
`not_planned`, Unicode label/milestone/description and its separate draft. B's
explicit Save persists `Actor B saved issue through metadata view — 日本語` at
generation 2. Returning to A restores its original generation-1 saved text,
without autosaving A's discarded buffer. Comments remain explicitly unsupported.

Read-only SQLite after native Quit confirms exactly those issue bodies and
generations, unchanged PR drafts, four metadata rows, unchanged quota barriers
and zero credential/journal rows. The task's WebDriver listener is released.
Packaging output is `/tmp/gitru-ruru78-qa-native-build.log`; seed/hashed bundle
output is `/tmp/gitru-ruru78-qa/prepare.log`. Updater artifact signing is disabled
only in the temporary QA configuration, preserving release settings.

Root review accepts the thin shared helper integration, the eleven real fixture
HTTP/storage cases and seven issue-specific UI cases. All 154 native and 215
desktop tests, native Clippy/fmt, desktop/E2E types, scoped Biome and debug/E2E
packaging pass. There is no changed IPC signature or domain/schema contract, so
the signed RURU-77 normal `make typegen` output is inherited unchanged rather
than hand-edited or regenerated needlessly. Ready for signed scoped publication
on PR #150's branch. Exact-head remote CI and live provider/production vault
qualification remain distinct gates; no merge is performed.
