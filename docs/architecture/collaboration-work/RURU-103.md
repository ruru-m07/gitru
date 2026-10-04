# RURU-103 — retained native multi-webview synchronization qualification

Status: locally qualified on macOS on 4 October 2026. The complete retained
pipeline and separate normal packaged control pass. Final repository/feature
checks pass; remote Linux/Windows CI and live platform/provider/vault checks
remain separate. Reviewable [draft PR #157](https://github.com/ruru-m07/gitru/pull/157) is open
and attached, stacked on #156. Linear is In Review; nothing is merged.

Attached worktree `/Users/ruru/.codex/worktrees/collab-ruru-103/gitru`, branch
`ruru/ruru-103-native-sync-harness`, now inherits published R101/#156
`63e11970838c3bca1710f16175886f117e95aabb` and R110/#155
`725b9f242c13b153d9da42374a0f9c32e31a8348`. These prerequisites remain In Review.
The original pre-code contract was saved on 3 October 2026 at R110719fb63;
selection-time observations below are historical. The final evidence record at
this document's end is authoritative. Primary dev and unrelated work are intact.

Root accepts the bounded test-only architecture and acceptance cases below.
This is a retained native qualification lane, with default production builds
compiling/registering neither fixture controller nor fake provider. Do not weaken
production caller, demand, hint, account or lease policies to make tests pass.
Persist finite synthetic fixtures only inside a marker-validated run-owned root;
no keyring/CLI/cloud/provider-network fallback. New generated controller inputs
must be finite actions, run/scenario IDs and native-issued gate IDs, never an
arbitrary URL/path/token/script/SQL program. Hard-crash runner kills only its exact
launch-owned executable/PID after a native-issued checkpoint, then observes exit
before reopen. Native fake observations and clock advances remain explicitly
synthetic evidence; live vault/power loss/provider qualification is separate.

Ownership: native core owner creates only new feature-gated harness files and
crates/collaboration Cargo/lib feature plumbing; root owns shared runtime seams,
Tauri setup/commands/config/typegen/runner/CI/docs. Frontend owner creates only
new e2e harness bootstrap/probe/protocol/spec files, coordinating DTO freeze
before consuming normally generated wrappers. No generated hand edits. All Cargo
and native app/typegen/build/seed processes serialize through the recorded wrapper.
Normal packaged smoke/handoff and a new retained lane remain separate checks.
Read master architecture/backlog first; update this contract before material
architecture changes.

## Observed prerequisite and base

Live Linear RURU-103 is Backlog, blocked by RURU-98, with no duplicate relation
or attachment. RURU-98 is In Review with the implemented native/SDK lease seam;
its latest documented review head is `2d415938`, not a merged prerequisite. Root
has independently audited open PRs/artifacts and found no R103 duplicate. This
proposal does not change Linear or claim those review gates are Done.

Inspection base: R110 worktree `/Users/ruru/.codex/worktrees/collab-ruru-110/gitru`,
HEAD `719fb6386f4d28031dd0239e93cb1ac9620a0d05`, containing signed R110 `c24ef58`
plus the qualified parent synthetic HTTP repair. This inherits R98, R96 and R79.
R110 has the atomic, current-epoch quota-only merge, saturated persisted waits and
before-vault/before-fetch feed budget fences. These must remain authoritative.

R101 is still independently qualifying. Its owner reports renderer DTOs unchanged
but new native `DetailReconciliation` on `DetailPage`, `DetailCommit`, and optional
receipt on `DetailLease`: enumeration FullEnumeration/Incremental/Uncertain and
head scope SubjectHistory/CurrentHead. Full fake child traversal must explicitly
declare `full_history()`; Body defaults Uncertain. R103 should start on root's
final R110 ancestry, then inherit the final signed/restacked R101 before native
fixture detail construction freezes. Prefer final R101-on-R110 as the R103 PR
base if ready. Do not restore pre-R101 detail literals or implement another head
reconciliation policy. The TS/runner scaffold can be scoped independently once
root creates the managed worktree, but native source integration is sequential.

## Existing seams and actual gaps

* `apps/desktop/e2e/{run.ts,wdio.conf.ts}` builds/launches an isolated `e2e` app,
  uses WDIO embedded driver and an exact temp Git configuration, captures logs
  and screenshots, and uploads artifacts on all three desktop CI platforms.
* `apps/desktop/src/bootstrap/e2e-collaboration.ts` is imported only in Vite e2e
  mode. It offers five fixed DOM/navigation actions and bounded metadata receipts.
  The current packaged `collaboration.e2e.ts` retains one main script because
  the embedded driver cannot subsequently address main once it contains native
  children. It proves the child Accounts handoff and same-instance restoration,
  not shared fake-provider sync.
* `collaboration_setup.rs` currently uses an in-memory TestVault and disabled
  GitHub CLI under e2e, but still constructs real GitHub/GitLab providers and the
  production background worker. Current automated collaboration tests use an
  empty database. R103 needs a retained native fake adapter and a durable fake
  vault to qualify an active actor and process restart safely.
* Each real document owns the production CollaborationClient, QueryClient,
  RevisionBridge, AuthorizationFence and DemandCoordinator. Generated local
  queries never call the provider. Runtime owns one scheduler and all SQLite
  writes. `useVisibleDemand` and 45-second native/15-second SDK leases are already
  implemented; automatic selection must not create `detail_demands` requests.
* `changes_since` has 256-row pages, a 4096-revision retained log, authorization
  view/epoch reset evidence and floor-based ResetRequired. RevisionBridge registers
  before catch-up, drains has_more, awaits reset application and ignores event
  revision ordering. A later real event/focus/online wake is needed if all earlier
  hints were dropped; no claim of recovery without any future wake is made.
* Production `webview-tab-host.tsx` turns main demand off before showing its
  selected child. Concurrent main+child qualification must not fake two active
  owners in that layout or weaken visibility policy.

## Test-only architecture

Use an additive app Cargo feature `collaboration-harness` implying `e2e` and a
core `test-harness` feature. Default release/dev builds compile neither module,
register no fixture command, and import no fixture JS. Add a dedicated e2e config
with identifier `com.ruru.gitru.e2e.collaboration` and Vite mode/define selecting
the compiled fixture bootstrap. Keep the ordinary smoke/Accounts lane unchanged.
The identifier, feature, run nonce and isolated-root checks are all mandatory;
an environment variable alone never enables fixture authority in production.

### Core retained fixture

New core feature-gated files, e.g. `test_harness/{mod,provider,vault,scenarios}.rs`
and a narrowly scoped `runtime/harness.rs` child for access to the injected clock
and lifecycle checkpoints. The public feature-only constructor returns one
actual CollaborationRuntime plus bounded HarnessControl. Seed through public
Store/credential/runtime APIs, never SQL-authored membership or renderer proofs.

The fake adapter implements the actual CollaborationProvider contract. It has no
HTTP client, subprocess, shell, arbitrary URL or fallback adapter. Its own profile
explicitly declares fixture read facets and Unsupported writes; this never
changes production GitHub/GitLab capabilities. Two static synthetic actors share
canonical-looking repository/subject paths but have separate account/epoch and
private text. The primary fixture has a selected repository, fresh feed scopes,
known cached summary, private draft generation 1 and missing/stale Body, avoiding
unrelated startup list jobs while a body acquisition is measured. Optionally a
repositories-only divergent fixture exercises typed unsupported admission.

Scenario state is an enum, not an arbitrary response program. Fixed phases return
bounded Body metadata/text, complete/304 observations, permission denial or typed
quota/offline evidence. A gate holds one admitted response until explicitly
released. Calls get monotonic fixture IDs and fixed actor/epoch/facet/head labels;
keep at most 128 diagnostic entries and expose no token/body in native metrics.
Gates/receipts use run and scenario generations so old commands cannot change a
new phase. Maximum two retained gates; terminal/timeout cancellation releases
them. The runtime's real worker, demand eligibility, commit fences, budget helper
and SQLite are used, rather than a second scheduler.

Use a feature-only injected monotonic/UTC fixture clock to test lease expiry
without waiting 45 seconds. Only bounded predefined advances are accepted;
advancing wakes the existing native worker. It does not mark data fresh, erase
cooldowns or turn query reads into provider work. Fake clock behavior is recorded
as synthetic qualification, not real OS sleep/resume evidence.

The durable fake vault stores only task-owned synthetic credentials/reference
records beneath the fixture root, with portable hashed names, flush/atomic file
replacement and restrictive permissions where supported. It never calls keyring.
Prefer fixed token IDs/allowlist and synthetic tokens produced inside native code;
no fixture command accepts a PAT. Restart reuses this vault so a committed Active
account does not spuriously become auth_required merely because the process died.

### Native harness/controller

New app modules `collaboration_harness.rs` and
`commands/collaboration_harness.rs`, registered only under the new feature. A
small projected command set, finalized before normal typegen:

* `collaboration_harness_control(request)` — root/main-only fixed action enum,
  run nonce, expected scenario generation and native-issued gate ID where needed.
  Actions prepare one named fixture, create/close one fixed secondary surface,
  arm/release a preset provider/read gate, set a hint-delivery mode, advance one
  preset clock interval, fill the real retention gap or select a fixed crash
  checkpoint. No JS, SQL, URL, path, token, free-form scope/body or PID input.
* `collaboration_harness_status({run_nonce})` — main-only bounded fixture metrics,
  current durable revision, committed phase, gate states and the process/run
  identity. It is not a bypass for reading actor content.
* If native lifecycle creation cannot fit the first command cleanly, keep a
  separate projected create/close command with the same fixed action authority.

Recheck the actual caller Webview, exact local origin, main label, compiled
identifier, scenario nonce and generation in every handler. Child calls to the
controller, production credential handoff/host mutations or peer lease renewals
must fail without side effects. Existing production demand/snapshot/draft/refresh
commands remain the only renderer path for actual collaboration operations.
Run normal `make typegen`; fixture TS consumes generated wrappers and source
schemas. Verify default invoke_handler has no fixture command registration.

For delayed IPC, a feature-only bounded hook captures a real authorized native
item/detail/draft snapshot and holds its return for one fixed fixture target and
known view incarnation. It holds no SQLite transaction or lifecycle lock. Release
after disconnect/reload tests the real SDK fence, not a fabricated snapshot.
Only needed projection hooks are added to thin wrappers; production compiled
branches remain unchanged. No generic dynamic invocation/evaluation hook exists.

### Real surface arrangement

For concurrent demand, keep main's real embedded content physically visible and
create one feature-only secondary native Window containing an actual child
`tab-webview:ruru103:<native-issued-id>`, fixed app route and bounded geometry.
Native creation uses async/main-thread Tauri APIs; do not use a synchronous
command on Windows. The real main executor uses generated Inspect/Set owner
commands only after child show succeeds. Both physical windows must be visible
and unminimized. No frontend-authored generation or forged owner is accepted.

Separately exercise normal single-window tab-host creation/switch/modal hide,
dispose and same-label replacement, using its existing serialized implementation.
Close the secondary fixture surface before entering the ordinary tab-host mode:
the host's prefix cleanup must not accidentally adopt/close unrelated fixture
surfaces. All fixture windows are tracked by exact native-issued labels and
closed in finally; never close arbitrary webviews.

### TS driver and observations

New e2e-only bootstrap/fixture probe and fixed action driver. It imports generated
commands and the real singleton SDK/react hooks with the document's production
QueryClient. Mount the normal collaboration workspace/detail/private editor where
UI behavior matters; a tiny fixed probe can report the real local query state.
Do not substitute a mock transport or a test-only React Query cache. Instrument
actual generated catch-up calls if cursor/page/reset receipts are needed, without
altering their results or exposing private SDK state.

The driver dispatches a finite validated action schema to a native-registered
fixture label and request nonce. Receipts include document instance nonce,
scenario/generation, current account/epoch/canonical subject, safe query revision/
facet revision, known fixture value hash, loading/error state, draft generation
and counts. Do not return arbitrary DOM text, passwords, provider errors or query
cache serialization. Only fixed synthetic editor text can be entered/copied.

Keep one compiled main scenario executor alive for child operations; WDIO calls
only that fixed action entry point. No generated wrapper is recreated with raw
invoke names inside browser.execute; no native API accepts evaluated strings.
Main SPA navigation can retain the executor, but a full main unload cannot:
use a new external runner phase/session after crash. Child reload/close/recreate
can be driven through the fixed native/compiled bootstrap actions. This honors
the embedded-driver limitation rather than claiming direct child WDIO control.

## Acceptance cases and decisive evidence

1. **Equivalent real main+child interest.** Hold missing Body dispatch, open the
   same authorized resource on both visible surfaces through common UI/hooks.
   Verify two actual native owner-bound handles but one admitted fake provider
   call for the same account/epoch/target. Release; both real local query caches
   render identical canonical Body/metadata and facet revision at quiescence.
   Hook reads themselves add zero provider calls and durable automatic requests
   remain zero. Removing one owner leaves the other interested; removing both
   stops the later stale-Body foreground job, while ordinary backfill is judged
   independently.
2. **Dropped and reordered wake hints.** Feature-only relay drops/holds hints
   for exactly one registered view, retaining only bounded revision strings.
   Commit several real fixture phases. Deliver the final/earlier hints out of
   order; verify durable catch-up reaches the final committed projection without
   moving backwards or republishing the old phase. Also drop all current hints
   then use an actual focus/online or explicit public SDK wake once. Do not add
   production polling to make this pass.
3. **Catch-up paging, irrelevant rows and retention overflow.** Pause one view's
   hint delivery while real writes for another fixture actor produce more than
   256 revisions, then drain actual pages and verify cursor advancement despite
   irrelevant scopes. Separately use a fixed bounded fill action for more than
   the real 4096 retention window, without lowering the production constant or
   editing log_floor. Save only one reused synthetic draft subject, keeping data
   bounded. Assert real ResetRequired, fresh authorized snapshots and suppression
   of a delayed old snapshot after reset. Private dirty text retains its original
   inspected generation, and a conflicting saved generation fails CAS safely.
4. **Reload/close/lost cleanup.** Reload the real child document; assert a new
   document nonce, fresh bridge registration/catch-up and saved data after reload.
   Exercise ordinary disposal/recreate owner generations and stale late lease
   receipts. With intentional lost JS cleanup, advance the fixture clock beyond
   native TTL and verify expired interest cannot dispatch or renew; new visible
   interest is independently acquired. No historical handle/map grows per visit.
5. **Disconnect with provider and local read in flight.** Gate an old-epoch fake
   provider response and one actual native local return. Disconnect through the
   main's generated production command, catch up on both views, release the old
   responses. Body/header/summary stay absent; obsolete epoch/run pages cannot
   commit or recreate provider cache. Private drafts remain recoverable and exact
   actor partitions hold. A different actor with the same repository/subject text
   never sees the first actor's text. Local authored writes are explicitly legal
   under their own account/CAS rules; this case does not invent a remote outbox.
6. **Actual app hard-crash/restart.** A dedicated outer runner receives a
   native-generated run-bound checkpoint and kills only its owned fixture app
   process with no Rust unwind. Case A holds a provider result before commit;
   case B commits the page, writes a durable checkpoint and withholds its hint
   before the process is killed. Relaunch the same isolated DB/vault without
   reset, under a new run/session incarnation. Verify no persisted ephemeral
   leases/automatic demand, saved drafts/ref/epoch and strict cooldown survive,
   uncommitted response is absent, committed phase is available via local read
   and revisions, and fresh demand alone can admit fresh work. Existing core
   transaction-boundary crash tests remain distinct evidence; if a new true
   mid-transaction seam is necessary, add only a fixed feature checkpoint after
   R101 source freeze. Do not claim power-loss/vault platform qualification.
7. **Authority controls.** Same generated controller command from child is
   denied; foreign lease ownership, stale scenario/read gate IDs and wrong run
   nonce fail closed. Wrong compiled identifier/root marker aborts setup. Default
   production build contains no fixture command registration/bootstrap. Normal
   child cannot connect/disconnect accounts or mutate host state; native consent
   handoff/permissions are not relaxed for this harness.

## Isolation, cleanup and runner

Use a dedicated mkdtemp run directory containing collaboration SQLite+WAL,
durable fake vault, exact Git config, checkpoint markers, logs and run.json.
Validate canonical root/owned marker before native fixture setup; no symlink
traversal or caller-supplied file path. Compile identifier and feature are fixed.
Disable native keyring, GitHub CLI, cloud sign-in, updater requests and personal
Git configuration. No production provider adapter exists in this lane. Persist
only finite synthetic values. Restart retains its own DB/vault; reset/cleanup
only visits known run-owned files. Ordinary repository/preferences/window-state
files use the fixed harness-ID OS namespace, which is distinct from personal
`com.ruru.gitru` but shared across harness runs. The runner never resets or deletes
unknown files there, and does not override HOME. This lane does not claim all
application persistence is beneath the run root or a freshly cleared OS profile.

The outer runner owns process launch identity and crash checkpoints; it must
prove the target is the exact run-owned child executable before kill, and wait
for exit/lease release before restart. Expected kill is a scenario result, not
a swallowed WDIO pass/failure. New sessions get new artifact subdirectories and
failure status is retained even if cleanup also fails. Always close fixture
windows/provider gates, stop child service and snapshot safe metrics on failure.

## Ownership split and order

* **Native harness owner:** core feature/provider/vault/scenario clock and gates;
  isolated app setup/event relay; native controller and origin/ownership checks;
  retained process checkpoint hooks; focused Rust contract tests. Coordinate
  exact touching of runtime.rs, providers/mod.rs, storage/details.rs and scheduler
  with final R101. No migration or production budget rewrite is planned.
* **Frontend/SDK-driver owner:** e2e-only bootstrap/probe, fixed action protocol,
  real UI/query/demand observations and new packaged scenarios; focused protocol/
  lifecycle tests. Production SDK files change only for an actual reproduced bug,
  with an independent regression and peer review. Preserve R96 links/R79 subject
  provenance/current private CAS and R98 demand; no old durable selection hook.
* **Root integration owner:** feature/config/build script/lib registration,
  source-derived generation/typegen, external crash runner coordination, CI lanes,
  docs/Linear/PR publication and final native UI/authority review. Assign each
  shared file before edits. No agent hand-edits generated packages/commands.

Sequence: approve/source-truth contract → managed worktree on chosen exact signed
base → native DTO/control freeze → one normal typegen → native and TS lanes in
parallel → combined deterministic cases → dedicated packaged harness all three
OS → root review and signed publication. Existing smoke/handoff lane stays passing.

## Validation and honest completion gates

Run focused native harness/caller/feature-gate tests and all collaboration tests,
workspace fmt/Clippy with warnings denied for default and harness feature sets.
Run generated schema/wire tests, SDK bridge/demand tests, fixed probe/driver tests,
desktop types including e2e, scoped Biome and production+e2e frontend builds.
Run normal `make typegen` and inspect semantic diff; `make verify` and the normal
packaged E2E remain separate from the new retained native lane.

Add the dedicated harness build/run to the existing Linux/Xvfb, macOS and Windows
matrix with bounded per-scenario deadlines and always-uploaded artifacts. It
must exercise actual WebKitGTK/WKWebView/WebView2 surfaces, real generated IPC,
the native worker and SQLite; do not replace a failing platform with jsdom.
Main/child construction is async on Windows, and process restart/port4445 cleanup
is explicit. Local shared Cargo commands use the full-process serial wrapper;
packaged QA reserves port4445 and never overlaps another task's app. Exact-head
remote CI/security gates remain required before merge; local Mac success is not
three-platform qualification. Performance thresholds, native OS vault/live
provider testing, broad sleep/resume and future delivery/outbox are separate
R125/R107/R102 gates, not silently completed here.

## Primary references checked

* Pinned local Tauri2.11.1 source exposes Window::add_child under desktop/unstable;
  the app already enables unstable. Current official docs describe real child
  webviews and caution against synchronous Windows command construction:
  [Tauri Window/add_child](https://docs.rs/tauri/latest/tauri/window/struct.Window.html#method.add_child).
* The current WDIO docs say an asynchronous executor cannot span a document
  unload; retain main only for child lifecycle and use a new runner/session for
  process restart. Existing pinned driver behavior must be checked rather than
  upgraded as part of this slice:
  [WDIO executeAsync](https://webdriver.io/docs/api/browser/executeAsync/).
* [Live R103](https://linear.app/catra/issue/RURU-103/prove-sync-and-revision-recovery-across-real-native-webviews)
  and [live R98](https://linear.app/catra/issue/RURU-98/add-foreground-demand-leases-to-the-native-sync-scheduler)
  were read without mutation. Repository architecture/backlog and R98 worknote
  supplied the implementation boundaries above.

## Native wrapper and own-view manifest contract — 4 October2026

Core control/status DTOs are frozen in test_harness/domain.rs. Root accepts a
separate read-only generated own-view manifest getter: feature/run/identifier
and exact local caller proof yield only run/session/scenario, actual caller label,
finite Main/ConcurrentChild/NormalTab role and native actor descriptors. It grants
no controller metrics or mutation authority. Control/status remain main-only.
The core receives the real Tauri visibility probe at first shared-Arc construction;
background starts only after prepared seeding or reopening prepared state.

Root wrapper control is finite Core/CreateConcurrentChild/CloseConcurrentChild/
ReloadConcurrentChild/HoldChildHints/DropChildHints/DeliverChildHintsReverse/
ResumeHints/ArmItemRead/ArmBodyRead/ArmDraftRead/ReleaseLocalRead/CancelLocalReads/
CheckpointBeforeCommit/CheckpointCommittedBeforeHint. Request contains run nonce,
expected scenario generation, optional core action and native-issued gate ID;
irrelevant parameters fail closed. Status contains core receipt, optional actual
child label, finite hint mode, bounded held hints/local gates and checkpoint.
Child manifest access never enables control, credential mutation or peer activity.

Mount the existing SavedItemDetail/private editor using the document's existing
QueryClient and singleton SDK. Observational changesSince instrumentation calls
the original generated transport and returns the identical receipt, with bounded
counters restored on teardown. It never fabricates cursors/snapshots/owners.
Real current pending intent count is not a historical automatic-request metric;
the native thin hydrate ingress must count actual authorized requests for that
claim. Fake clock advances govern real runtime scheduling/lease expiry, while
read-side wall-clock freshness remains real and is not relabeled as advanced.

Committed-before-hint checkpoint requires held targeted hints plus actual public
Store detail evidence matching a finite fixture phase/facet revision. BeforeCommit
uses a genuinely held provider receipt, and the runner kills only its own exact
process. No shared production publish hook or arbitrary observation callback is
needed. Default release/dev compiles/registers none of these fixture commands.

Ownership refinement: delegated native app owner owns new Tauri harness/controller
and narrow feature gates/setup/read-return hooks, including native app Cargo/lib
registration. Core owner owns only feature-gated core files; frontend owner owns
new harness probe/protocol/spec files. Root owns generator/activation/config/
external runner/CI/docs and final signed publication. Existing source DTOs,
production operations and migration bytes remain unchanged.

### Native core checkpoint and final-ancestor integration — 4 October2026

The feature-only retained core constructs one actual Store/runtime, fixed provider,
synthetic durable vault and injected scheduling/lease clock. Fourteen original
core cases and five independent review regressions pass after resumption: real
visibility loss prevents vault/dispatch, phase change cancels held old observation,
cold reopen retains committed cache/ref/epoch/private CAS with no ephemeral
interest, and interrupted preparation/lost marker refuse to reset existing data.
Current pending-detail count remains distinct from historical authorized hydrate
ingress. These are cold-close/synthetic proof, not actual hard-kill/power loss.

Root checkpoints only frozen core source plus its feature/mod plumbing before
integrating published R101/#15663e1197 (R110/#155725b9f2 inherited). Native app and
frontend source lanes remain independently dirty/owned, with no PR claim until
combined feature/production gates and real retained native scenarios pass. Adapt
native-only receipt fixtures to final R101 without changing its authority policy.
Actual logs `/tmp/gitru-ruru103-core-review-baseline.log` and
`/tmp/gitru-ruru103-core-review-independent.log` qualify19 core cases; no personal
credentials/config/vault or provider network were used.

### Packaged crash coordination and source generation — 4 October 2026

Root accepts a local WDIO service adapter around pinned tauri-service 1.4.0.
Its launcher owns the actual spawned ChildProcess handles and their exact binary/
run environment. A successful fixed checkpoint case writes a bounded driver ack;
the afterTest hook waits for observed forced exit before driver session deletion.
The launcher checks marker, nonce, native session/scenario/kind/PID and exact
owned binary/handle, sends SIGKILL to that handle only, and records actual exit.
No process enumeration, global PID kill, guessed path or swallowed failure is
allowed. A new driver/app session reopens the same task-owned database and vault.
Guard the pinned private-map shape and fail closed if the dependency changes.
Independent helper tests qualify ownership refusal and force-exit mechanics;
only the retained packaged lane qualifies the real native checkpoint/restart.
The core reviewer now owns only new process helper/tests/local service files;
root retains configuration/outer runner/activation/CI/docs ownership.

Normal typegen now includes native/core fixture DTO sources, correct Serde enums
and nullability, and three additive commands. The scanner sees both mutually
exclusive typed event relays; the source correction collapses only exact duplicate
listener definitions and continues rejecting different identifier collisions.
No generated files are edited by hand. Main activation follows the existing
bridge/root installation and imports fixtures only in the dedicated Vite mode.

Vercel reconnection confirms parent R110/#155 exact-head deployment is READY with
a completed build; its GitHub Vercel status remains stale pending. All14 other
reported checks pass. R101/#156 is published and its own CI is still running.
Neither the status mismatch nor local evidence authorizes a merge.

R101/#156 exact head63e1197 subsequently passed all11 reported checks in CI run
37185030333, including Rust and packaged E2E on Linux/macOS/Windows. No exact-head
CodeQL run is reported, so none is assumed. All attached collaboration PR heads
were rechecked: no failed reported checks; only R110's stale Vercel status remains
pending despite its separately verified READY deployment.

Independent TypeScript AST review compares all285 previous named Zod initializers
against63e1197: zero removed/changed,26 additive fixture schemas. Generated116
commands remain normal source-generated output. Native core19, native app26
feature/15 default and frontend24 focused tests pass locally; default/feature
Clippy and source formatting pass in their lanes. Actual packaged retained
webviews, captured IPC returns and forced-process restart remain open gates.

### Finite real-child authority extension — 4 October 2026

Root accepts one additional compiled action carrying only the real main-issued
lease UUID, literal owner main, native owner generation and primary actor/epoch.
The child matches actor/epoch against its own native manifest, then uses its real
native caller/current owner receipt to attempt foreign renewal/release, synthetic
actor disconnect and fixed main owner inspect/activity/dispose. Each must return
typed PermissionDenied; main's same lease remains renewable and is released in
finally. Capture only finite outcomes and unchanged actor/epoch/revision/vault/
call counters, never persist the opaque lease or arbitrary scope. No personal CLI
token or credential inspection occurs. Production caller and lease guards remain
unchanged; this exercises the actual generated IPC, rather than claiming existing
core tests alone qualify every real-webview authority case.


### RURU-103 integrated local checkpoint and current remote evidence — 4 October 2026

R101/#156 head63e1197 passes all11 reported checks in run37185030333, including
Rust and packaged E2E on Linux, macOS and Windows. No exact-head CodeQL run is
reported. After Vercel reconnection, R110/#155 head725b9f2 deployment
`dpl_6UXbMPbqvvj1Fd6RXKGtCFFMiBdU` is READY and its build completed3 October
13:41:13Z; GitHub still reports its stale Vercel status Pending. The other14
reported checks pass. No failed attached collaboration checks or duplicate PRs
were found, and no merge is authorized.

R103 inherits both exact published heads. Full `make verify` passes locally:
477 frontend/SDK/UI tests across53 files, all repo lint/types, production desktop
build, workspace Rust tests, formatting and default all-target Clippy. This
includes28 focused frontend protocol/probe/observer cases,36 owned-process helper
cases and2 environment-isolation cases. Additional feature lanes pass19 actual
core cases and26 native app cases, with15 default native caller/updater cases;
core/app feature Clippy also passes. Normal `make typegen` produces116 commands;
independent AST comparison preserves all285 previous named Zod schemas and adds
26 fixture schemas. Production built JS contains none of the fixture bootstrap
markers. Frozen migrations and provider authority policy remain unchanged.

The finite synthetic native controller, real query/SDK probe, pinned WDIO service
adapter, owned-handle crash coordinator and separate three-platform CI lane are
checkpointed before retained execution. These local tests do not yet qualify
actual packaged shared webviews, captured IPC returns, hard-kill/restart or the
normal packaged E2E lane on this head. Collaboration DB/vault/checkpoints live
under each private run root; ordinary app/window persistence uses only the fixed
harness-ID OS namespace and is never reset. A dependency startup failure before
its child handle is registered uses the dependency's cleanup; it is not claimed
as independently proven forced-exit ownership. See the
accepted contract above.


### First retained launch and runner failure corrections — 4 October 2026

Signed checkpointdc97ef9 builds the actual macOS harness release. First native
main launch creates its real secondary view, but WDIO cuts executeAsync off at
10.002s because its HTTP timeout was10s while script/scenario bounds were190s/
160s. Later null receipts are fallout; no scenario/crash success is claimed.
Artifacts remain under `artifacts/e2e-harness/2026-10-04T08-20-28-598Z-19412`.
Root aligns the transport deadline to200s and separates outer `runner.log` from
the dependency-owned `wdio.log`, avoiding diagnostic truncation.

Installed WDIO9.31.7 also swallows ordinary service-hook errors and afterTest
rejections. The pinned launcher now promotes lifecycle failures to the actual
SevereServiceError with finite public stage/code and preserved causes. Actual
installed dispatcher regressions qualify fatal prepare/worker/complete behavior
and an ordinary swallowed control; process helper cases now total39. A finite
qualification-error receipt checked at user onComplete and the outer runner
prevents swallowed crash-evidence failures from producing green qualification.
Outer crash proof independently matches complete driver ack, phase, exact binary,
PID/session/scenario/kind and observed SIGKILL. Two actual installed-hook/proof
regressions pass. Production engine policy stays unchanged. Retained packaged
rerun and normal packaged E2E remain open gates.


### Bounded second retained receipts and isolation refinement — 4 October 2026

Second native launch31996a6 produces complete bounded receipts: actual reload/
recreation, normal public tab lifecycle and peer authority pass on macOS. Cold
concurrent demand and hint catchup fail before their first observations; delayed
local read during disconnect fails before the captured-return gate. These are
open harness qualification failures, not engine or restart success. Evidence is
retained at `artifacts/e2e-harness/2026-10-04T08-32-49-542Z-20754`. A source-backed
probe schema incorrectly names cold DetailValueState unknown instead of actual
not_loaded; the frontend owner is correcting this without changing production
DTOs/fences. Preserve NotReady/old-binding safety around phase changes.

The pinned CLI imports dotenv/config. Root forces it to a fresh task-owned empty
driver.env, preventing cwd .env reload after the inherited-environment whitelist.
An actual installed dotenv/config subprocess regression with entirely synthetic
files qualifies the empty override against an unconfigured sentinel control;
owned-process cases now total40. This change accesses no personal config. Normal
packaged E2E is being checked separately while the probe correction proceeds.


### Cold-state and React-binding regressions; normal package control — 4 October 2026

The cold-cache observation test independently reproduces the prior ZodError and
now uses generated DetailValueStateSchema, including real not_loaded. A probe
regression proves a fresh native phase with an uncommitted React binding returns
NotReady and performs zero local query calls, then reaches one real query-path
call after commit while preserving the editor. The executor retries only that
pre-IPC NotReady for its explicit gated reads; it never counts NotReady/timeout
as captured stale/cancelled success or changes native/provider authority. All30
focused frontend cases and desktop/E2E types/lint pass locally.

Separate normal packaged macOS E2E passes all3 cases across2 specs on this source:
local collaboration snapshots, real Inbox child-to-host account dialog with exact
tab restoration, and UI/Tauri/Rust/Git repository smoke flows. Actual log is
`/tmp/gitru-ruru103-normal-e2e.log`. This is distinct from retained webview/crash
qualification, whose complete corrected run and Linux/Windows CI remain pending.


### Native captured-read lock regression and bound action settlement — 4 October 2026

Third retained macOS runfede493 passes cold concurrent demand plus reload, normal
tab lifecycle and peer authority. Reverse/dropped hints and monotonic Body/dirty
editor observations also pass before a main edit receives the correct pre-IPC
NotReady after a phase update. Fixed finite edit/save/read actions now settle only
that NotReady within existing bounds; accepted mutations run once, all other
errors remain failures.32 frontend cases/types/lint pass. Precise CAS substages
will expose actual editor/save/conflict evidence in the next retained run.

Disconnect's held local Body path then exceeds190s with no result receipt. Two
source reviewers identify a self-deadlock in the new harness matcher: cloned
webviews lock the same actual Tauri ResourceTable twice before entering the Held
state/timer. A bounded real-thread regression reproduces the old timeout/exit101.
Sequential lexical pointer snapshots release both guards before current caller
revalidation, preserving exact label/resource identity/current proof. Four real
ResourceTable mutex regressions qualify same/different allocation, label mismatch,
guard drop before proof and stale proof rejection.30 native feature cases and
feature Clippy/fmt pass; no production guard or IPC signature changes.

A separate speculative core-status AuthRequired concern is rejected after source
and existing regression review: inactive Store::detail already returns empty
Unavailable evidence, and old-epoch held-response tests successfully release
through post-disconnect status and assert committed Body evidence None. No blanket
catch or core policy change is made. Actual retained proof remains incomplete;
third-run artifacts are `artifacts/e2e-harness/2026-10-04T08-41-14-989Z-23734`.


### Explicit SDK completion after query cancellation — 4 October 2026

The fourth retained macOS run at 36a5307 passes five main scenarios, including
actual withheld-read disconnect. The hint scenario qualifies reversed/dropped
hints, dirty draft preservation, an actual private-draft CAS conflict, 300 writes
with a 256-row catch-up page and 4,100 writes producing ResetRequired. Its final
obsolete-read assertion fails. Artifacts are
`artifacts/e2e-harness/2026-10-04T08-56-17-023Z-25115`; crash phases did not run.

Installed TanStack Query source and a real QueryClient regression establish that
cancelling a cached refetch with revert enabled can resolve the prior cached value.
That query completion cannot identify the held native read's final outcome.
Explicit probe reads now call the existing account-scoped SDK directly, retaining
its actual authorization fence and generated local transport. Ordinary UI hooks,
QueryClient cache behavior and independent UI snapshot observations are unchanged.
A second regression invalidates the actual singleton SDK fence during held
transport and observes the real stale authorization outcome. Bounded result
receipts now retain retention-reset and disconnect read outcomes before assertions;
only stale_view or cancelled qualify. All 35 focused frontend cases, both desktop
and E2E TypeScript checks, scoped lint and diff checks pass. Complete retained
packaged main/crash/restart and Linux/Windows qualification remain pending.


### Crash driver teardown boundary — 4 October 2026

Signed 6e87965 now passes all six retained main scenarios in the actual macOS
release binary. The retention-reset and disconnect receipts both observe real
SDK stale authorization. The first before-commit fixture issues its real native
checkpoint and the launch-owned exact process exits on SIGKILL with matching
proof. WDIO still exits 1 because killing during afterTest removes its embedded
server before normal DELETE session teardown. This run is incomplete, not a
qualified crash/restart pass; artifacts are
`artifacts/e2e-harness/2026-10-04T09-07-22-613Z-26124`.

Installed WDIO 9.31.7 and embedded Rust driver 1.4.0 source confirm DELETE removes
only a driver session map entry, leaving native views, collaboration work and
SQLite alive. The launcher now kills the exact post-health-check captured app
only at successful worker-end, before delegated native cleanup. afterTest writes
only its strict passed acknowledgment. No connection error or nonzero exit is
relabelled success. Completion requires matching crash proof and always delegates
cleanup. A monotonic 45-second window from worker start conservatively precedes
the before-commit provider gate's 60-second expiry; fresh acknowledgment is also
bounded to 15 seconds with one millisecond filesystem rounding tolerance. Failed,
missing, late, foreign or spontaneously exited workers cannot qualify a crash.
Focused tests use real owned Node children, installed dispatcher behavior and
failed/late teardown controls. Actual native crash/restart rerun remains pending.


### R103 retained local qualification completed — 4 October 2026

Final signed runner source c0e06bb, with the release application built from
6e87965, passes the entire retained macOS pipeline: six real native webview cases,
before-commit forced crash, fresh restart, committed-before-hint forced crash and
fresh restart. Both crashes carry exact launch-owned SIGKILL/observed-exit proof;
both restarts use different native PIDs/session UUIDs on the same respective
retained fixture. Before-commit reopen sees no saved Body before fresh interest;
after-commit reopen sees the exact committed Body/facet revision before interest.
Both actors retain their private drafts, with no phantom ephemeral/durable demand
or provider calls before renewed interest. Retention reset and disconnect both
carry actual SDK stale_view receipts. The default app's three packaged macOS E2E
control cases also pass separately.

Safe artifacts: `artifacts/e2e-harness/2026-10-04T09-17-51-218Z-27055`.
Application SHA256: cbc8cfa2260d8ced4910d206041ed6dfe053d9ac226bc2c3dd1fe341ffdb1537.
All five driver stages exit 0; passing owned fixture roots are cleaned. Ordinary
preferences remain in the fixed harness-ID OS namespace and are never reset.
No personal credentials, native keyring, GitHub CLI login, provider network or
Gitru cloud sign-in are used. This qualifies process crash, not power loss or
production provider/vault behavior.

Final make verify passes: 497 frontend/SDK/UI cases across 54 files, repository
lint/types/production desktop build, default workspace formatting/Clippy and
663 Rust cases (three explicitly ignored). Additional feature validation passes
330 collaboration cases (two ignored, including all 19 retained core cases),
30 native app cases and workspace all-target feature Clippy. Normal typegen has
116 commands; independent AST comparison preserves all 285 prior Zod schemas
with 26 additive fixture schemas. Default built assets contain neither the
retained executor global nor installer; generated finite schema literals can
remain shared. Linux/Windows retained packaged jobs are newly wired and still
require remote CI. No merge is authorized or performed.

R111 GitLab resource reads and R121 bounded navigation prefetch now progress in
separate attached worktrees based on published R101/#156, with signed pre-code
contracts and disjoint provider/runtime-test versus SDK/UI ownership. Their local
checks and publication are still pending; no R103 fixture code is required by them.


R103 delivery: [draft PR #157](https://github.com/ruru-m07/gitru/pull/157) is open
and attached, stacked on #156. Linear RURU-103 is In Review. The qualified source
and signed publication are retained; the initial remote matrix is running,
including the three new retained packaged jobs. No merge was performed.


### RURU-103 remote Windows and subsequent local investigation — 4 October 2026

Published5efea64 remote run37192175309 passes the new retained Linux/macOS jobs
and ordinary Rust/packaged E2E on all three platforms. Retained Windows
job111406556841 fails launcher preparation before any scenario after an actual
spawn; its safe artifact does not expose a native exception. Source audit proves
Bun/libuv canonical paths omit the Windows extended prefix that Rust retains.
The runner/helper now resolve actual filesystem identities then use consistent
namespaced Windows spelling for native roots/binaries/evidence; POSIX spelling,
symlink/type/dev/ino/environment/actual-child ownership checks remain strict.
Focused path/process/proof57 tests pass/one Windows-only case skipped locally,
E2E types and scoped lint pass. This is not a local Windows execution claim.

The first runner-only unchanged cbc8cfa macOS rerun fails concurrent-demand and
disconnect before any new provider dispatch. The SDK separately reproduces a
hidden-to-visible document transition while awaiting native event subscription;
no DOM listener exists yet and the prior visibility sample stays false. Sampling
again after listener installation fixes that source defect;113 SDK tests include
three actual held-listener visibility controls. The new bb3b7d4 macOS executable
passes concurrent-demand, catchup and reload but still fails normal-host startup,
authority counter stability and disconnect provider capture. Current receipts
cannot prove visibility as the previous run's cause or attribute legitimate
background writes to denied peer calls. Full post-fix retained qualification is
still open; historical c0e06bb qualification is not presented as new-head evidence.

Failure diagnostics now preserve finite first-error classification, separate
cleanup failure and bounded pre-cleanup actual document/native owner observations.
Normal lifecycle substages distinguish route/inventory/handshake failures. Source
review also identifies two fixture setup races: phase changes wake eligible live
jobs before the next gate is armed, and known saved Body is not evidence that a
manual foreground lease cannot refresh it. Counter assertions remain exact;
actual committed-facet warmup and zero-lease phase/gate ordering are being qualified.
No production native gate is weakened; no personal credentials/provider/vault used.
Logs /tmp/gitru-ruru103-{windows-job,path-tests,path-retained-native,
 demand-startup-tests,startup-retained-native,failure-diagnostics-tests}.log;
safe failed artifacts09-59-15-158Z-37949 and10-11-07-212Z-72795 remain task-owned.
R111/#159 and R121/#158 are separately published review slices on #156. No merge.


Source fixes frozen before the next native build: finite activation consumes
actual setter/inspector generations (false-to-true legitimately issues a newer
native generation); hidden and older-replay controls remain enforced. Final
frontend harness checks49/4 files and desktop/E2E types/lint pass. The production
SDK visibility correction passes113 SDK cases. The inherited writer-lease fix
from R111 is integrated verbatim; its331-case native qualification, deterministic
pre-fix duplicate-descriptor Busy and preserved clone/final-owner assertions are
recorded in R111. R103's feature/native package must qualify this combined source
separately. No public DTO/signature change or hand-generated IPC occurs.


### RURU-103 repaired-source local qualification — 4 October 2026

Signed combined source `9554cc3f4b4481ae03be40d8e58068d115c2ab2b` passes the
entire rebuilt macOS retained pipeline, not merely the historical executable.
All six main cases and both forced-crash/fresh-restart pairs pass; every one of
five driver stages exits zero. Before-commit SIGKILL owns PID20219 and restart
PID20281; committed-before-hint SIGKILL owns PID20335 and restart PID20392.
Each restart has its own native session UUID and retains its matching earlier
checkpoint. Before-commit data is missing until fresh actual demand; committed
Body/facet20 survives the after-commit restart before interest. Both actors keep
private draft generation1; startup remains inert until real interest.

Artifact run: `artifacts/e2e-harness/2026-10-04T10-39-36-631Z-19854`.
Rebuilt native SHA256:
`30db1b4ad5ecf07873e6552ebb43854ec398c4610c84324b3eb7b531dfb76031`.
Retention reset and disconnect observe actual SDK `stale_view`; all eight peer
operations observe permission denial with exact unchanged account, revision,
owner, provider-call and vault-access evidence, plus successful main renewal and
release. Normal public tab creation/navigation/disposal passes. Actual committed
facet warmup and zero-lease phase/gate ordering remove the identified fixture
setup races without relaxing native authority or counter assertions. Finite
pre-cleanup diagnostics retain first failure and separate cleanup outcomes;
activity inspection may register/refresh its requesting native owner and is not
claimed to be mutation-free. Earlier failed artifacts remain historical evidence.

Fresh full checks on this source pass: 522 frontend/SDK/UI tests across56 files,
one Windows-only case skipped on macOS; repository lint/types; production frontend
build; default workspace665 Rust tests/three ignored; feature collaboration332
cases/two ignored; native feature30 cases; default and feature all-target workspace
Clippy; formatting and diff checks. Default assets contain neither fixture executor
global nor installer across403 JavaScript files. The earlier normal packaged macOS
3-case control remains separately recorded, not rerun/new-source qualification.
Logs: /tmp/gitru-ruru103-final-repair-{tests,lint,types,build,default-tests,
default-clippy,native-tests,native-app-tests,native-clippy,native-fmt}.log and
/tmp/gitru-ruru103-diagnostics-retained-native.log. No new Rust command signature
or DTO changed in this repair; existing normal typegen evidence still applies.

This qualifies the combined source locally on macOS with synthetic provider/vault
fixtures. The Windows canonical-spelling repair still needs its actual remote
retained job; previous published5efea64 Linux/macOS retained successes do not
qualify the new source. Remote matrix restarts after publication. Live provider,
production keyring, other platforms and power-loss behavior remain separate.
No personal credentials or cloud account are inspected; no merge is performed.


### Windows native-path identity follow-up — 4 October 2026

Exact published #157 cce498b passes retained Linux/macOS and ordinary Windows
E2E, but retained Windows job111420764848 fails native startup before any scenario
with `Invalid native collaboration harness input`. Its owned artifact preserves
`C:\\Users\\RUNNER~1\\AppData\\Local\\Temp` in the namespaced launch root.
The job actually runs Bun1.3.0+b0a6feca; any earlier1.3.7 source assumption does
not describe this job. Bun1.3.0's default Windows realpath walker preserves
non-symlink DOS aliases; its native resolver uses uv_fs_realpath. See the exact
[JS source](https://raw.githubusercontent.com/oven-sh/bun/bun-v1.3.0/src/js/node/fs.ts)
and [native source](https://raw.githubusercontent.com/oven-sh/bun/bun-v1.3.0/src/bun.js/node/node_fs.zig).

Signed source b5154effa039c7dab3a2b0accc7d9ca114a54786 resolves actual filesystem
identity with realpathSync.native before restoring Rust's Windows namespace.
Root/type/symlink/dev/ino/environment/actual-child guards remain strict. An actual
resolver spy proves native selection; the owned Windows child regression exercises
the real short alias when supplied by the host, proves matching filesystem identity
and rejects alias spelling without signaling the child. It fabricates no DOS name
or filesystem response. Full frontend523 tests pass/one Windows-only skip on macOS,
plus full lint/types and diff checks. Logs:
/tmp/gitru-ruru103-native-realpath-{focused-tests,final-tests,final-lint,final-types}.log.
Fresh retained local qualification and exact-head remote Windows execution remain
separate gates; this source audit is not a passing Windows scenario claim.

### Real surface preparation contract — 4 October 2026

Fresh b5154ef retained run11-45-28-268Z-41161 uses the identical previously passing
native binary SHA30db1b4a. Four main cases pass; normal first-surface observation and
disconnect provider capture time out before crash phases. Both disconnect documents
are actually DOM-hidden with native owners active, zero leases and an untouched
armed provider gate. SDK hidden-document suspension is correct. The normal host's
initial measurement waits requestAnimationFrame; hidden-page throttling fits its
failure, but current diagnostics do not prove the exact host branch or OS trigger.

Before additional fixture edits, adopt this bounded preparation contract: the
private native fixture focuses its exact created child window after show, and
restores/focuses its exact main window after closing that child. Use real Tauri
window methods; no generic labels, new external controller, fabricated DOM state,
production permission change or bypass of SDK/native hidden-owner admission.
The executor observes actual document visibility through the existing finite
probe before visibility-dependent mounts and normal-host startup. Preserve the
existing deadlines, native generations, counter assertions and hidden controls.
Pinned Tao0.35.2 set_focus activates the macOS application; show alone does not
establish actual document visibility. Exact native/DOM receipts, fresh compiled
execution and all crash stages must qualify the combined repair. The earlier
passing artifact and source-only path inference are not new-head qualification.


### Real-surface and native-path repair qualification — 4 October 2026

Signed source `dbed691b7ca9880db9f6695edc393fe7defcfee7` combines the actual
Windows native resolver with private fixture window show/unminimize/focus and
bounded actual DOM-visibility preconditions. Every visibility-dependent mount and
ordinary host startup observes the real document first; disconnect observes two
actual admitted SDK leases before advancing its gate. No fabricated visibility,
production authority change, extended deadline or polling delay is introduced.
Four new executor regressions preserve the hidden/native-active distinction,
normal-host admission and actual lease ordering. Native fixture focus uses only
its exact guarded child/main windows; projection locks are released before restore.

The first freshly compiled combined run12-02-10-744Z-43546 passes authority but
fails other cases while both actual documents remain hidden, with zero SDK leases.
Cleanup succeeds; no crash stage runs. The user then confirms the desktop is
available. An unchanged-source, unchanged-binary rerun passes all six main cases
and both exact-owned SIGKILL/fresh-restart pairs, with all five driver stages zero:
`artifacts/e2e-harness/2026-10-04T12-11-00-118Z-44210`.
Native SHA256:
`d226d6c88f93e03c5abb43b1aae7de92aee81239debafbdcf88e473729f606f3`.
Before-commit owns PID44487/session23ab3d87-a91e-48f7-9fa4-d67dedffac15,
restart PID44557/sessionf29b1244-891d-4364-a173-cb4b4624b1ee; checkpoint has no
committed facet and fresh demand produces facet21. Committed-before-hint owns
PID44610/sessionb939302c-2e21-4167-9782-4151e8c71087, restart
PID44664/sessiondce54d86-0866-4bf7-9828-43c79818f958; facet20 survives with zero
provider/vault reads before interest. Both retain draft generation1. Retention and
disconnect reject actual obsolete reads; all eight peer denials preserve exact
account, revision, owner, provider and vault counters, with main lease renewal and
release successful. Ordinary tab creation, navigation, modal pause, disposal and
recreation pass. Process crashes do not qualify power loss.

The earlier hidden runs remain failure evidence. Their exact OS trigger was not
recorded and is not inferred from the successful rerun. CUA could not bind the
unbundled fixture executable; no GUI mutation or personal app was performed.
Both ordinary and fixture lanes intentionally use unbundled binaries with pinned
Tao Regular activation policy, so no packaging change is inferred from that tool
binding limitation. SDK hidden-document suspension and native admission remain
correctly enforced.

Fresh checks on this repair: frontend527 passed/one Windows-only skip across56
files; executor12/12; full repository lint/types; native feature app30/30; feature
workspace all-target Clippy with warnings denied; formatting/diff; production
frontend build. All403 default JavaScript assets exclude fixture executor/global
and installer. The earlier9554cc3 full core/default workspace tests remain
separate evidence for unchanged core, not freshly rerun counts for this repair.
No Rust signature/public DTO changed; existing normal typegen evidence applies.
Logs: /tmp/gitru-ruru103-surface-final-{tests,lint,types}.log,
/tmp/gitru-ruru103-visible-{native-app-tests,native-clippy,native-fmt,
default-build,retained-native}.log and
/tmp/gitru-ruru103-available-desktop-retained.log.

This is current-source local macOS qualification with synthetic provider/vault.
The Windows short-alias native-path fix still requires new-head retained remote
execution; old Linux/macOS successes and ordinary Windows E2E are distinct.
Live provider, production keyring, platform and power-loss claims remain outside
this evidence. No personal credentials inspected; no merge performed.


### Exact-head Windows retention-return lifetime seam — 4 October 2026

Published #1579ef7148 passes13/14 reported checks/statuses, including retained
Linux/macOS and ordinary Windows E2E. RetainedWindows job111434742147,
run37201765360, artifact2026-10-04T12-40-04-421Z-6136 now successfully launches
under actualBun1.3.0+b0a6feca5; all6 main scenarios execute,5 pass. The earlier
native short-alias setup rejection is gone on this head, but full Windows
retained qualification is still failed. NativeSHA256
`eef8f0e922525c7aae7b852cdce4d24a6c9e27122a4d88e1b8b364695c017252`.

Only hints-and-catchup fails at release of the real held local Body snapshot after
retention reset: generation6 gate2bedf9ba-a945-474c-aafd-8a3b552d4097 is TimedOut,
and release_local_read rejects its terminal state with stale_view. Both documents
are actually visible with active native owners and2SDK leases; dirty text/CAS and
real ResetRequired are already observed at revision4431. Cleanup succeeds. No
crash stages execute after main failure. Artifact has no individual control times,
so exact FillRetention duration is not claimed. Source proves the held native15s
return gate/child10s protocol currently span4100 sequential real Store::save_draft
transactions plus reset/catchup. The lifetime seam is established by source order
and terminal gate receipt, not inferred desktop availability.

Before another fixture edit, accept bounded preparation: capture the child's real
reset/cursor baseline while hints are dropped, perform the same4100 real writes
BEFORE arming or starting its held Body read, then verify that actual child reset
count/cursor have not advanced and remain behind the pruned native revision. Arm
and capture the real SDK read under its unchanged pre-reset fence, observe Held,
then wake the actual bridge, require ResetRequired and all dirty/CAS/cache controls,
release within the existing bounds, and require actual stale_view/cancelled. The
native query data revision may already equal the filled revision; the tested old
property is the SDK authorization generation before ResetRequired, not older data.
No fabrication, pre-accepted stale outcome, timeout increase, retry, reduced write
count, retention threshold, permission or production SDK change. If the child
already caught up during preparation, fail rather than claim a pre-reset fence.
Meaningful held-preparation/early-catchup controls and fresh combined retained
execution must qualify this next fixture change; current historical passes do not.


The frozen fixture also waits for the actual cached Body snapshot revision to reach
the filled native revision after wake, while retaining the unchanged body hash,
dirty editor, CAS conflict and reset assertions. Accepted wake already awaits the
real bridge drain/apply/reset; this adds an explicit fresh-cache receipt before
releasing the obsolete SDK-generation read. It does not assert older native data.
Native peer review confirms no production SDK/storage/native change is required.


## Retained target delivery failure and bounded SDK correction

Fresh signed5376f3b binary SHA256
54de3a062efcef8b5611e19684392e6070ee883bf4c63bf6f4e500501623f873
run2026-10-04T13-16-49-739Z-55780 fails1/6 main scenarios at the new pre-arm
retention assertion; no crash stages run. Task-owned PID55783/session
d61ef798-bb15-4b2c-851e-6d06399f941d receipts prove child reads22→23, cursor331→4431
and resets0→1 during FillRetention, with Drop hints, both documents actually
visible and two demand leases. No local gate was armed and cleanup succeeded.
This is not evidence of polling or a hidden desktop.

Pinned JS API2.11.1 event.js defaults a listener without target to EventTarget::Any.
Pinned Rust tauri2.11.1 event/listener.rs match_any_or_filter accepts Any even
when an emitter targets another webview, and iterates all registered webviews.
The ordinary SDK uses this catch-all for collaboration revision hints, so native
fixture main-only hints also wake the supposedly withheld child. Own-webview
listener semantics are documented in the official
[Tauri Webview API](https://tauri.app/reference/javascript/api/namespacewebview/)
and [event API](https://tauri.app/reference/javascript/api/namespaceevent/).

Before production edits, accept the bounded transport correction: bind ONLY the
ordinary SDK collaboration-change listener to the actual current Webview target
through the documented API. Global app.emit broadcasts must still wake that
listener; events targeted to another view must not. Do not alter RevisionBridge,
authorization/view fences, event payloads, catchup receipts, native filtering,
retention4100 writes or existing deadlines. Local Git hints and demand activity
retain their existing behavior. Add an actual Tauri event-API boundary regression
for both main/child labels and unchanged unsubscribe; preserve all existing
bridge, field/draft and native fixture assertions. No DTO/signature/schema change
or typegen is needed. Frontend owner additionally owns SDK index.ts and a narrow
transport regression file; root serializes a fresh compiled retained run.


The catch-all also bypassed earlier Hold/Drop hint isolation. Earlier passing
receipts prove native hint collection and later convergence, but do not establish
that the child cache was unchanged while withheld. Before the final fixture edits,
accept two finite observed controls: capture actual child Body/facet/cache-cursor
baseline immediately before Hold; require it unchanged after all three real
refreshes and before reverse delivery. Likewise require the pre-Drop Body/facet
receipt unchanged after the real304 refresh and before explicit public wake.
Retain dirty editor/CAS checks and every existing limit. Root authorizes frontend
owner to add these assertions to the existing two executor files and ordering
controls; no fake cursor, visibility, native event bus or production bridge change.
