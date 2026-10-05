# RURU-102 — Independent-clock quota lifecycle, first bounded slice

Status: pre-code contract,5October2026. Read shared engine/backlog first.
Isolated managed ruru/ruru-102-clock-lifecycle starts attached Comments#164 exact
signed aab107dacf11e67216de602e42954a61e46c4423. Live R102 is Backlog with no
attachment/duplicate; R98/R76 are In Review and their actual heads are ancestors.
Worktree audit finds only primary unrelated untracked architecture work; preserve
it. Current collaboration PRs have no failed checks; #164's new CI is running,
#163 passes11/11. Older#155/#158 stale Vercel contexts have separate verified READY
evidence. No failed CI is being bypassed or inferred green. No merge.

## Scope and authority

Qualify independently changing monotonic and UTC clocks through existing native
Runtime/scheduler/SQLite and synthetic vault. Preserve existing3:1 interactive/
background and2:1 detail/index fairness, account rotation, queue reservations,
retry jitter, one-worker dispatch and durable read intent. This bounded slice
addresses clock-change/recovery; broader provider-family criteria remain open.
Comments preserves positive captured-account/epoch cooldown immediately after
successful detail fetch before any validation/admission/reconciliation. Preserve
that boundary and its RED→GREEN held200/cold-sibling controls; rejected content
can consume quota without proving data/access. Store rejects old epochs before
publication. No personal credentials or live provider/window tests are authorized
for unattended validation.

Current fixtures derive UTC and Instant from one counter. Shared dispatch budget
checks consult persisted UTC, while scheduler admission holds live monotonic
barriers and shared persistence does not centrally seed them. These are unqualified
source risks. Demonstrate RED through real Runtime boundaries before describing or
repairing a clock defect; do not duplicate existing fairness/48h tests.

## Clock and budget rules

A forward wall jump must not shorten/remove/bypass an already observed unexpired
short live monotonic budget; a later shorter observation cannot reduce its lower
bound. Capture the monotonic lower bound with the durable proposal and install it
only after Store accepts captured account/epoch. Merge stricter live deadline.
Failed/old-epoch writes must not install a scheduler barrier. Shared dispatch
checks respect both live lower bound and full persisted deadline before vault and
again before HTTP after intervening work, without provider-name branches.

First correction/independent-jump qualification is bounded to captured cooldowns
<=24hours. Existing longer-deadline behavior remains: bounded wake is a recheck,
never permission to ignore a later full durable deadline. Preserve48h/extreme
deadline tests; no automatic shortening on backward time changes. Cold recovery
uses valid persisted wall time. Incorrect clock across process restart and portable
suspend elapsed-time guarantees are excluded. Fresh official [SystemTime](https://doc.rust-lang.org/std/time/struct.SystemTime.html)
documents nonmonotonic wall time; [Instant](https://doc.rust-lang.org/std/time/struct.Instant.html)
explicitly leaves suspend accounting platform/version dependent. Synthetic clock
controls cannot qualify real OS suspend behavior.

## Ownership and fixtures

Runtime owner owns runtime.rs (narrow registration/shared persistence/budget
helpers), runtime/clock.rs only if calculation needed, and narrowly necessary
feed/detail dispatch callsites. Root owns docs/serialized Cargo/typegen/commits/
publishing. Fixture owner owns new runtime/clock_lifecycle_tests.rs. No shared
storage/model/migration/SDK/UI/dependency/generated-file changes planned. Agree
expansion before editing other production paths.

Fixture supplies independent monotonic advancement/signed UTC movement/jitter,
synthetic adapter receipt/counter/gates and vault counters. Use bounded handshakes,
release guards and timeouts, no timing sleeps. Admit real work through Runtime
methods and actual scheduler pick; no handcrafted replacement Job or direct
scheduler barrier population. Hold lifecycle or the real blocking-vault boundary
and poll the actual worker to prove pending. Obtain a genuine synthetic adapter
cooldown and use the common captured-account persistence path. Distinguish such
native boundary controls from a full provider transport test. Release/finish
workers before cold SQLite reopen. Source risks require executed RED before repair.

Required RED→GREEN/unchanged controls:

1. Forward UTC during short live cooldown blocks early sibling HTTP/vault and
   retains original durable deadline; later shorter observation cannot shorten it.
2. Already picked real feed/detail work waiting on lifecycle is blocked by newly
   accepted budget even after a wall jump. After a vault gate, newly accepted budget
   blocks HTTP; distinguish completed vault load from forbidden additional loads.
3. Backward UTC causes no provider spinning/evidence shortening or peer starvation.
   Eligible second account gets bounded service; auth/permanent failures do not spin.
4. Same-actor reconnect retains quota; obsolete epochs affect neither replacement
   live/durable budgets nor truth or another actor.
5. Cold reopen with valid wall time retains full deadline, cache, draft generation/
   CAS and zero early HTTP/vault; long48h/extreme controls remain intact.
6. Comments held-rejection/other-facet isolation and existing fairness controls
   remain unchanged. No broad sleep/resume/platform claim beyond executed gates.

## Compatibility and delivery

No public command/DTO/schema promise. Root runs normal make typegen and complete
independent AST comparison against exact#164:114commands,291schemas/243aliases,
1event, publicBranch name/display_name/is_remote/is_detached; zero public changes
expected. Only timestamp/order churn may be restored, never hand edit generated.
Root serializes focused RED/GREEN/native gates, full make verify, independent
review and signed attached draft on#164. Record actual commands/counts/errors;
local validation, inherited CI and new exact-head CI remain distinct. Live provider/
vault/OS/native UI and unreported CodeQL are not inferred. R102 becomes In Progress
and remains so for broader criteria; no merge.

R104 destructive eviction awaits R99 recovery/export: its separately published
#144 head is absent from this base even though ordinary draft CAS is present.
R103#157 own-Webview listener correction/retained native integration is also a
separate outstanding lane; no implicit integration or completion by this slice.


## Accepted pre-code peer clarifications

Three read-only native/fixture/architecture reviews confirm bounded feasibility;
no clock implementation/test result exists yet. Final dispatch checks and quota
commit/install share scheduler serialization: capture monotonic lower bound and
UTC proposal before awaiting Store, then hold the scheduler mutex across Store's
captured-epoch acceptance and max-install in the existing account-keyed map. Drop
before publish/vault/network. Store releases its own writer and calls no Runtime;
current lock order has no reverse writer-to-scheduler cycle. Callers may already
hold lifecycle; do not acquire lifecycle/reenter scheduler inside the helper.
Guarantee accepted quota visible at the dispatch check's linearization point,
not atomicity across the later HTTP future. A quota write obsolete at Store
admission installs nothing; quota accepted before same-actor epoch replacement
remains valid actor consumption and survives reconnect. Reuse one map slot per
account, no per-epoch/jump/receipt history. Reconnect cardinality controls inspect
without manually creating or prematurely evicting budgets.

Place checks immediately before load_token after reference/native reads, and
immediately before provider fetch after adapter selection and detail dispatch
validation. Existing feed checks require appropriate placement/live qualification;
detail currently lacks these rechecks. Local admission refusal must not fabricate
a new provider observation at shifted UTC. Accepted private seam: Job gains
local_budget_refusal bool, false in its three literals and reset after every pick.
Shared budget check takes mutable Job, tags only immediately before returning its
existing local RateLimited error. In record_error guard only the provider:rest
persist call with !local_budget_refusal; retain scope status/retry/monotonic due
behavior and genuine provider-rate persistence. No string matching or public
ErrorCode/DTO/schema expansion. Runtime owner additionally owns narrow
runtime/scheduler.rs, runtime/demand_tests.rs and runtime_credential_tests.rs
initialization-only seams; existing assertions/fairness policy unchanged.

Lifecycle race uses a verified-actor rejected synthetic probe under actual connect
lock, which persists positive cooldown without replacing epoch while a real
picked feed/detail waits. Post-vault case uses a genuine synthetic adapter receipt
and normal captured-epoch persistence while spawn_blocking vault is held: it is
Runtime boundary qualification, not proof that a second public probe can run
while lifecycle is held. Handshakes/actual counters, unchanged provider:rest
deadline and existing obsolete-reply/privacy/fairness controls remain mandatory.
Root signs before source; allow only test registration/fixture work to establish
RED before production repair. Public baseline and scope limits remain unchanged.


### Successful-probe observation clarification before RED/repair

A successful connection separately captures quota_deadline after the probe and
before staged/vault work, committing it atomically with credentials/account through
commit_account_credential_with_quota. Normal refresh can seed its live barrier;
a forward UTC jump during actual blocking vault.store may make that reconstruction
miss the unexpired monotonic lower bound. This remains an unproven source risk.
Fixture owner adds first-grant and same-actor replacement controls gated at that
real vault boundary, with original durable deadline, stable account-map cardinality
and no premature reads/vault loads. If executed RED proves it, Runtime owner may
capture the Instant at the same post-probe receipt point, then scheduler-serialize
Store credential+quota commit and max-install for returned accepted account.id.
Install nothing on Err and release before existing failure cleanup, after_cutover
checkpoint, scheduler reset/publish/refresh/retirement cleanup. No network/vault
under scheduler; Store transaction/journal/crash checkpoints and credential cleanup
remain unchanged. Existing rollback/after-cutover crash tests must pass. A shared
helper-only repair cannot qualify this separate successful observation path.
