# RURU-103 — retained native multi-webview sync qualification proposal

Status: root-approved retained harness implementation contract, saved before
major code on 3 October2026. Attached worktree
`/Users/ruru/.codex/worktrees/collab-ruru-103/gitru`, branch
`ruru/ruru-103-native-sync-harness`, starts at signed R110719fb63. R110 picker
qualification and R101 captured-dispatch correction are still in progress in
separate managed worktrees. Inherit their final signed source before combined
qualification/publication; do not claim either unmerged blocker Done. Live Linear
R103 is Backlog at selection, with no attached/open duplicate. Primary dev and
unrelated work remain untouched.

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

Use a dedicated mkdtemp run directory containing app config/data, SQLite+WAL,
durable fake vault, exact Git config, checkpoint markers, logs and run.json.
Validate canonical root/owned marker before native fixture setup; no symlink
traversal or caller-supplied file path. Compile identifier and feature are fixed.
Disable native keyring, GitHub CLI, cloud sign-in, updater requests and personal
Git configuration. No production provider adapter exists in this lane. Persist
only finite synthetic values. Restart retains its own DB/vault; reset/cleanup
only visits known run-owned files. Never reuse/reset ordinary user AppData.

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
