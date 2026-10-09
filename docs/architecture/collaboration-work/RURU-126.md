# RURU-126 — Safe local sync diagnostics

Status: implemented and locally qualified, 8 October 2026.

Issue: [RURU-126](https://linear.app/catra/issue/RURU-126/expose-safe-local-sync-diagnostics-and-actionable-retry-states).

This isolated managed worktree started at signed RURU-125 measured head
`a9a671bae185bb0de8e2845100a6f2a33b14baae` and now inherits its signed,
test-only ordinary-suite isolation fix
`8eccce4e7989faa09a3b8b07abbe5727c0492947`. It inherits RURU-102's bounded fair
scheduler/lifecycle slice and schema 13 from RURU-114. Schema 14/15 command
operations are outside this source and must not be assumed or recreated here.

## User outcome

The trusted main window exposes a local-only Sync health panel. It explains how
much cached coverage exists, whether work is queued or cooling down, the class of
the current recovery state, SQLite/WAL usage and bounded aggregate native sync
latency. Reading the panel or exporting a report never loads a credential, calls
a provider, admits a sync job or clears a cooldown.

Recovery guidance is category-specific:

- authentication asks the user to reconnect the affected account;
- permission failures ask for a credential with the required provider access;
- rate limits show the saved wait and provide no early retry;
- offline state waits for connectivity and permits only explicit retry after the
  barrier is eligible;
- transient provider unavailability permits a deliberate retry after its saved
  barrier; and
- permanent/unsupported/local-storage failures provide explanation without an
  automatic retry loop.

The existing explicit refresh API remains the only network admission path. The
diagnostics panel may offer it for an exact active account only when the native
snapshot says an explicit retry is currently eligible. It never schedules retry
from rendering, polling, export, subscription or error observation.

## Native model and authority

Rust owns measurement, classification, SQLite reads and export. The public
snapshot contains a generated timestamp, bounded per-account health records and
global aggregates:

- coverage counts from saved `sync_scopes.coverage_json`;
- ready/deferred queue counts, oldest queue age and cooldown remaining time from
  the single native scheduler;
- saved retry category and next eligible time from `SyncStatus`, without its raw
  message;
- existing retention accounting plus database and WAL byte counts;
- a fixed-bucket, bounded in-memory histogram of actual native sync-job elapsed
  time, including count, total, maximum and p50/p95/p99 bucket upper bounds.

Queue timestamps are monotonic process-local observations. Saved scope state is
durable authority across restart; diagnostics must identify unavailable process
observations rather than fabricate continuity. Latency buckets are updated only
from real elapsed work in the normal runtime. Test fixtures may drive actual fake
provider operations but production code contains no seeded or fixed metrics.

The read command is main-window-only and local-only. It returns account IDs only
for authorized contextual UI joins; provider host, login, actor, repository or
resource identity and remote text are absent from diagnostics rows. Counts and
times are bounded and saturating. No database migration is planned.

## Privacy-preserving export

Export is a separate main-window-only native operation. Rust derives a report
from the snapshot, removes all account keys and emits only aggregate counts,
categories, durations, version/platform facts and storage sizes. A native save
dialog writes the chosen UTF-8 JSON file atomically with private permissions.

The export type has no fields capable of carrying tokens, account usernames,
actor/account/repository/resource IDs, provider hosts/URLs, remote titles/bodies,
raw provider responses or `CollaborationError.message`. Tests seed conspicuous
canary values in every forbidden class and reject their appearance in serialized
bytes. The UI preview remains contextual and is never reused as the export body.

## Generated IPC, client and UI

New Rust command signatures are generated through `make typegen`; generated
files are never edited manually. `@gitru/collaboration-client` exposes a
local-query transport and TanStack query with no reconnect/provider refetch
semantics. Revision changes invalidate it, while a bounded interval updates
process-local queue ages and cooldown countdowns.

The existing suspended main account/settings dialog hosts the panel so credential
and retry controls never appear in child webviews. Ordinary labels describe
coverage, waiting, storage and recent sync speed without exposing storage schema,
scope keys or provider IDs. Keyboard and screen-reader status are covered; the UI
does not claim missing latency, WAL support or cooldown evidence is zero.

## Bounds and failure behavior

- At most 128 ready and 128 deferred scheduler entries already exist; diagnostics
  aggregate rather than return jobs or scope keys.
- Histogram storage is a constant number of atomic/small locked counters, never a
  request log. Durations and counts saturate.
- Snapshot acquisition uses short native locks and existing bounded retention
  queries. A busy/unavailable storage observation is explicit and does not block
  the rest of the panel.
- Diagnostic failures return fixed safe messages. Provider error messages, URLs
  and HTTP bodies cannot reach snapshot/export fields.
- Export cancellation writes nothing. Partial writes use a private temporary file
  and atomic replacement; a failure preserves an existing destination.

## Acceptance evidence

Native tests must cover all six recovery categories, exact retry eligibility,
blocked cooldown behavior, queue-age bounds, real elapsed histogram updates,
cold runtime missing observations, cache/WAL usage and serialization privacy
canaries. IPC tests cover main/child/foreign authorization and export cancel/
atomic-write behavior. Client/UI tests prove cache-only reads, no render-driven
refresh, explicit eligible retry, inaccessible retry during barriers, contextual
labels, export success/failure and keyboard access.

Run focused Rust and frontend suites, `make typegen`, `make verify`, and the
packaged collaboration harness if the runtime/IPC surface changes materially.
Record actual local checks separately from remote CI, live providers, production
vaults and other platforms. Publish a signed scoped draft PR stacked on RURU-125;
do not merge.

## Implemented slice — 8 October 2026

The native runtime now owns a bounded diagnostics snapshot. SQLite supplies
saved coverage and recovery evidence, the scheduler supplies ready/deferred
counts and monotonic queue/cooldown ages, the existing retention accounting
supplies cache usage, and an eight-bucket in-memory histogram records elapsed
time only around real sync attempts. A cold runtime intentionally retains saved
recovery evidence while process-local queue and latency observations restart.
Snapshot reads do not load the vault, admit work or invoke a provider.

The main-window-only Tauri surface exposes local snapshot and native export
commands through generated IPC. The export model is aggregate-only and cannot
represent account, user, provider, repository, resource, URL or remote-text
identity. Its native save flow uses the existing private atomic writer. Canary
tests reject every forbidden class in serialized bytes, and cancellation writes
nothing.

The collaboration client adds a local TanStack query whose bounded interval only
re-reads native state. The trusted account dialog now shows cache coverage,
ready/waiting work, cooldowns, actual latency samples and database/WAL size.
Recovery copy distinguishes authentication, permission, rate-limit, offline,
provider-unavailable and permanent states. A retry button is rendered only when
the native snapshot marks the exact active account eligible, and clicking it
invokes the existing explicit refresh path once. Rendering, polling and export
never start provider work.

Local qualification passed normal 127-command `make typegen` and the final
serialized `make verify`: 693 frontend/SDK/UI tests passed with one platform
fixture skipped, followed by lint, type checks, production build, Rust format,
workspace warning-denied Clippy and every default Rust workspace suite. The
collaboration library portion passed 355 tests with two standalone subprocess
helpers ignored. Focused collaboration-client and desktop suites passed 159 and
533 tests respectively, with the same single platform skip in the desktop run;
the feature-enabled native harness passed 43 tests.

No database migration, provider request, credential/keyring access, retry-loop
timer, Gitru cloud dependency or production telemetry was added. This run did
not exercise a live provider, personal credential, packaged desktop binary,
other operating system or remote CI; those remain separate qualification
boundaries.
