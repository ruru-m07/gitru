# RURU-124 — Local inbox state

Status: signed pre-code contract on 7 October 2026. Read the shared engine and
backlog documents first.

Baseline: exact signed RURU-137 evidence head
`0da82cc7e5a3543512be683e68ac074f8d9689cd` in an isolated managed worktree on
the external volume. RURU-79 is the sole declared prerequisite. Its draft
[PR #154](https://github.com/ruru-m07/gitru/pull/154) is open at exact signed
`e6fe69eba00a598448cc8f29c1164326610ad20b`, is an ancestor of this baseline,
and all 15 reported checks pass, including the three CodeQL analyses. RURU-124
has no prior branch, worktree, pull request or comment and is Backlog before this
contract. No ancestor is merged.

Live issue: [RURU-124](https://linear.app/catra/issue/RURU-124/add-local-inbox-snooze-bookmark-and-disposition-state),
“Add local inbox snooze, bookmark and disposition state”.

## Outcome

Gitru owns a small local intent ledger for cached inbox rows. A user can keep an
item in the local inbox, mark it locally done, snooze it until a bounded UTC
deadline and bookmark it. This state persists across restart and never changes
the provider's unread/read or pending/done fields. This slice sends no provider
request, loads no credential and depends on no Gitru cloud account.

The provider observation remains the source for notification content, remote
state and activity time. Local intent is a projection layered over that saved
observation. List rows and detail presentation show the provider state and the
local state as different facts.

## Public model

The new provider-neutral local types are intentionally inbox-specific:

```text
LocalInboxDisposition
  inbox | done

LocalInboxEffectiveDisposition
  inbox | snoozed | done

LocalInboxState
  disposition
  effective_disposition
  bookmarked
  snoozed_until             canonical UTC or null
  activity_updated_at       provider activity captured when intent was authored
  superseded_by_activity    true when newer provider activity overrides done/snooze
  generation                positive local CAS generation, or "0" before first write

InboxEntry
  item                      unchanged RemoteItem/provider observation
  local                     LocalInboxState

InboxQuery
  account_id
  remote_state              unread/read or pending/done, provider semantics only
  local_state               inbox/snoozed/done/bookmarked/all
  search, cursor, limit

InboxPage
  entries
  revision, authorization_view, next_cursor
  coverage, sync
  evaluated_at              native UTC used for the effective projection
  next_local_change_at      earliest future snooze transition relevant to this query

SetLocalInboxStateRequest
  account_id, authorization_epoch, notification_id
  mutation                   disposition | bookmark
  disposition/snoozed_until  present only for disposition mutation
  bookmarked                 present only for bookmark mutation
  expected_generation
```

`RemoteItem.state`, `RemoteItem.unread` and `RemoteItem.updated_at` are never
rewritten by a local action. The existing generic item query remains compatible
for pull requests, issues and older notification consumers; the Inbox UI and
badge move to the typed inbox query so local state is not smuggled into provider
JSON.

## Storage and authority

Forward migration `0012_local_inbox_state.sql` adds one table keyed by
`(account_id, notification_id)` with:

* `disposition` constrained to `inbox` or `done`;
* `bookmarked` constrained to a boolean;
* nullable canonical `snoozed_until`;
* non-null canonical `activity_updated_at` captured from `items.updated_at` in
  the same native write transaction;
* positive `generation` for compare-and-swap updates.

The row has an account foreign key but deliberately has no item foreign key.
Like a draft, authored local intent survives provider cache retirement and can
apply when the same stable provider notification identity returns. Account and
notification identity remain a composite boundary, so overlapping provider IDs
cannot cross accounts. No token, response body or mutable provider URL is saved.

The native update command validates the active account and exact authorization
epoch, requires a currently accessible notification row, compares the expected
generation and captures the current saved provider activity time internally. It
rejects PR/issue IDs, retired or denied notification membership, malformed or
non-future snooze deadlines and incompatible `done + snoozed_until` input. A
successful write increments the generation, records a `local_inbox` change and
returns the newly derived local snapshot. A stale CAS or epoch does not mutate
the row.

Bookmarks are independent. Changing disposition or snooze never clears a
bookmark, and toggling a bookmark never changes disposition, its captured
activity basis or provider state. The mutation kind is explicit so bookmarking
a row that new provider activity has re-surfaced cannot accidentally reapply its
superseded done/snooze intent. The renderer submits the expected generation;
concurrent windows receive `stale_view`, reload and preserve the winning action.

## Effective state and new activity

SQLite owns one effective-state predicate used by local list filters, literal
FTS search, pagination and sidebar counts:

1. No local row means `inbox`, not bookmarked.
2. If `items.updated_at > activity_updated_at`, the old local done/snooze intent
   is superseded and the effective disposition is `inbox`.
3. Otherwise, `done` is effective done.
4. Otherwise, a future `snoozed_until` is effective snoozed.
5. Otherwise, the item is effective inbox.

Provider timestamps are already normalized to canonical UTC nanosecond strings
before persistence. The local captured activity time comes only from that row.
The comparison does not infer activity from title, unread or reason. Equal or
older provider observations cannot revive an item. A newer accepted observation
re-surfaces it even when it is remotely read, because remote status is an
independent filter and presentation fact. `superseded_by_activity` remains true
until the user authors another local action against the new activity revision;
the stored bookmark continues to be true throughout.

`bookmarked` selects every bookmarked accessible row regardless of its effective
disposition. Remote-state and search filters may still narrow that view when the
user explicitly applies them. The normal Inbox filter selects only effective
inbox rows; Snoozed and Done select their exact effective states. `all` preserves
all accessible saved provider rows.

## Time and pagination

Native UTC is the authority for validating deadlines and evaluating a first
page. Snoozes are bounded to at most 30 days from that native observation. A
local cursor binds the query fingerprint, account authorization view, local
projection revision and `evaluated_at`. Later pages reuse the same evaluation
time, so a deadline cannot reorder rows halfway through one pagination snapshot.
Any local write, account authorization change or provider publication makes an
old cursor stale.

`next_local_change_at` is the earliest unexpired snooze deadline that can change
the current filtered result. The frontend uses it to return to page one and
re-read SQLite. It also rechecks on normal window focus/visibility behavior.
Desktop timer delivery can be delayed while the process is suspended or the OS
throttles a hidden webview; the UI must not claim an exact alarm guarantee.
After resume/focus, the next native read evaluates current wall time and fixes
the projection. This task does not claim immunity to an incorrect system clock,
timezone database behavior, or a deadline firing while Gitru is not running.
Stored timestamps are UTC and a restart never depends on a prior JavaScript
timer.

## Query, subscription and UI

The native command surface adds only cache-local `collaboration_inbox` and
`collaboration_set_local_inbox_state`. Both go through generated
`@gitru/commands` bindings. The client exposes account-fenced read/write methods,
query keys and hooks. `local_inbox` changes invalidate inbox pages, inbox badge
queries and the exact selected row; provider `notifications` changes continue to
invalidate them and can re-surface a row through the common SQLite predicate.

The Inbox toolbar keeps the provider-state selector and adds a local selector:
Inbox, Snoozed, Done, Bookmarked and All local. Native-notification provider
choices remain Unread/Read/All; todo choices remain Pending/Done/All. Labels say
“Provider unread/read” or “Provider pending/done”. Each row shows remote state
and local badges separately. Local controls provide Move to inbox, Done,
bookmark toggle and bounded snooze presets. They optimistically update only after
the native CAS returns; while pending they are disabled. A stale mutation reloads
instead of overwriting another window.

The sidebar badge uses the same typed inbox query with effective `inbox` plus the
provider's current badge state. Its existing `99+` and incomplete-page suffix
remain. Search executes in SQLite before the effective local predicate and uses
the same cursor/evaluation authority as an unsearched list. No independent
renderer count or hidden array filter may disagree with the native page.

Unsupported/missing inbox saved-read capability keeps the existing explicit
CapabilityBoundary. Local controls are not rendered when the provider account
has no readable inbox projection. Remote-write capability is neither requested
nor advertised by this task.

## Bounds and errors

* Inbox page limits remain 1–100 and search text remains at most 256 bytes.
* Local cursors remain at most 4 KiB and bind account, authorization and query.
* Notification IDs and UTC strings use existing identifier/text validation and
  canonical RFC 3339 parsing.
* Snooze is strictly future at commit time and at most 30 days.
* Provider/auth/access errors remain typed; local invalid input and stale CAS do
  not expose database or provider diagnostics.
* A denied or inactive notification is unreadable even when local intent exists.
  Reauthorization must expose a current provider row before local state can be
  read or changed.

## Required evidence

Native storage tests must cover:

* first write, generation CAS, concurrent stale writer and restart persistence;
* account isolation and overlapping notification IDs;
* PR/issue, retired membership, denied scope and obsolete epoch rejection;
* inbox/done/snoozed/expired/bookmarked/all plus every provider remote filter;
* literal FTS search, stable local pagination and stale cursor after local write;
* newer/equal/older provider activity, including remote-read newer activity;
* bookmark preservation while activity supersedes done/snooze;
* exact deadline, invalid UTC, past and over-30-day rejection;
* migration upgrade, rollback/fault behavior and old migration immutability;
* no provider/vault call for queries or mutations.

Client/UI tests must cover generated wire parsing, query invalidation,
authorization fencing, provider versus local labels, controls, stale CAS recovery,
timer/root-page reset, sidebar count consistency, keyboard operation and an
unsupported inbox boundary. A packaged restart scenario must persist local done,
snooze and bookmark state in one process and read the same effective projection
in a fresh process without provider or vault access.

## Ownership and non-goals

This worktree owns the new 0012 migration, local inbox domain/store module,
narrow Tauri commands and registration, generated bindings, collaboration client
methods/hooks, Inbox UI/badge integration, focused tests and this work note. It
does not edit the RURU-137 worktree.

Out of scope: provider mark-read/mark-done APIs, outbox/replay, webhook delivery,
cross-device/cloud sync, notification subject discovery changes, provider
adapter changes, credential/keyring changes, global notification preferences,
arbitrary custom snooze dates, destructive cache retention policy or PR/issue
local state. RURU-130 owns remote read/done actions and their activity policy.
No PR merge is authorized.

## Implemented source and local evidence — 8 October 2026

The bounded local-only slice is implemented on the frozen baseline above. The
new 0012 migration and SQLite projection keep provider status immutable while
adding account-partitioned local disposition, bookmark, snooze deadline and CAS
generation. Explicit bookmark versus disposition mutations preserve independent
intent. A newer accepted provider `updated_at` re-surfaces locally done or
snoozed rows, including remotely read rows; equal and older observations do not.
The same native query applies provider/local filters, literal FTS search,
pagination, badge pages and the next relevant UTC transition.

Generated IPC now exposes the two cache-local commands through
`@gitru/commands`; the collaboration client fences reads and writes by account
epoch and invalidates inbox projections for both provider and local revisions.
The desktop Inbox presents provider and local selectors/badges separately and
offers keyboard-accessible bookmark, one-hour snooze, local Done and Move to
inbox controls. A stale CAS reloads SQLite. Deadline handling returns paged
views to their first page, checks at most once per minute while active and
rechecks after focus; it still makes no exact alarm or correct-system-clock
claim.

Local source qualification at this worktree passes:

* `make typegen` with 125 generated commands;
* `cargo test -p collaboration`, including six focused local-inbox integration
  cases, the exact-deadline unit control and the frozen-v1 migration upgrade;
* `cargo clippy -p collaboration -p gitru --all-targets -- -D warnings`;
* `cargo test -p gitru` with 27 passing native caller-policy tests;
* collaboration-client typecheck/lint and 156 tests across 19 files;
* desktop typecheck/lint and 234 collaboration feature tests across 18 files;
* `cargo fmt --all --check` and `git diff --check`.

The focused native cases also prove restart persistence for done, bookmark and
snooze; overlapping account IDs; stale generation and epoch fencing;
non-notification, retired and denied access rejection; every local and provider
filter; search/cursor invalidation; past, invalid and over-30-day deadline
rejection; and authored-state survival across provider-cache clearing. The
queries and writes call no adapter, vault or Gitru cloud service.

Packaged multi-process restart qualification, fresh remote CI, other-platform
execution and live-provider/PAT/keyring behavior remain separate gates. No
personal credential or provider endpoint was used, no provider write was added,
and no PR is published or merged by this source checkpoint.
