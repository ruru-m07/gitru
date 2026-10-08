# RURU-102 — account fairness across priority classes

Design checkpoint: 8 October 2026. Base: combined integration PR #190,
`f0b8801dec569af24a1b1f67744600f85f4d90ac`. This bounded continuation repairs
account starvation in the existing native scheduler. Provider API-family quota
attribution and broader lifecycle/platform qualification remain separate work.

## Problem and contract

The scheduler reserves three interactive turns for every background turn and,
within interaction, two detail turns for every index turn. Its account cursor is
currently shared by both interactive classes. If detail requests continuously
belong to account `z` while index requests belong to `a` and `b`, every detail
turn resets the shared cursor to `z`; each index turn picks `a` again. A queued
index request for `b` can wait forever. The inverse arrangement can starve detail
accounts beyond the two selected before an index turn resets their cursor.

Maintain one bounded round-robin cursor for each actual arbitration class:
interactive detail/discovery/artifact, interactive index, and reconciliation.
Within a continuously eligible class, every queued account receives a turn
before that class revisits an account; existing per-account FIFO scope rotation
is preserved. Keep the current 3:1 interactive/background and 2:1 detail/index
weights, queue limits, lease expiry, manual deferral and durable cooldown gates.
Blocked accounts must not consume a slot or poison rotation when they resume.

No new HTTP path, provider quota, persistence schema, public IPC or UI contract is
introduced. The two interactive cursors replace the shared cursor; storage stays
constant in the number of connected accounts. Runtime recreation starts a fresh
arbitration round while preserving existing durable read intent and budgets.

## Verification plan

First exercise the real `Scheduler::pick` with continuously replenished,
disjoint account populations; demonstrate both starvation directions on unchanged
production. Then verify bounded account coverage in each class, per-account
scope FIFO, the existing priority ratios, and cooldown/resume behavior. Run the
full native demand and independent-clock controls, collaboration suite and strict
Clippy/format checks. Keep local verification, remote CI and live provider/OS
lifecycle evidence separate. No personal credentials or live mutations are used.

## Progress

The two starvation controls both failed against unchanged production after the
signed design checkpoint `fba7765`; account `b` received no index turns, while
only the first two of four detail accounts received service in the inverse case.
The repair gives detail and index arbitration separate account cursors. A third
control verifies no early dispatch during a cooldown and fair peer rotation at
its exact expiry. Independent source review found no blocker.

Local qualification on the repaired source:

- `cargo test -p collaboration --all-features`: **1,019 passed, five subprocess
  helper ignores**, including 21 demand/scheduler controls and 16 independent
  clock/lifecycle controls.
- `cargo clippy -p collaboration --all-targets --all-features -- -D warnings`:
  passed.
- `cargo fmt --all -- --check` and `git diff --check`: passed.
- Logs: `/tmp/gitru-r102-fairness-red.log`,
  `/tmp/gitru-r102-fairness-native.log`, `/tmp/gitru-r102-fairness-clippy.log`.

No public types or command signatures changed, so generated IPC is unchanged.
Frontend/packaged tests were not rerun for this native arbitration-only delta.
Remote CI and live provider/OS suspend evidence remain separate. RURU-102 stays
In Progress for API-family budget attribution and remaining lifecycle policy.

## Follow-up: obsolete provider responses must back off

The RURU-107 packaged run exposed a second scheduler issue: after storing newer
Body data, a comparable older provider timestamp is correctly rejected but
reported as `StaleView`. That code is reserved for retired local authority and
bypasses retry scheduling; the still-live due demand can immediately fetch the
same obsolete response again. The retained synthetic run made 1,383 calls before
its 90-second scenario deadline.

Keep every local account/view/run fence and saved-value ordering rule intact.
Classify only an older comparable provider observation, after those local fences
pass, as a provider read failure. The existing bounded exponential retry delay
then applies without a new public error type or policy. A real-runtime fixture
control must reproduce the busy loop before the change, preserve newer canonical
data/account/draft, refuse early retry even after renewing demand, and recover
after the normal delay with fresh provider data. Direct storage ordering controls
must continue proving rejection and atomic rollback; retired authority controls
must still return `StaleView` and avoid publishing error into a replacement view.

The same distinction applies to independently clocked resource metadata when
Body was omitted: a metadata timestamp can be newer than the retained Body
ordering barrier. A second real-runtime control reproduced immediate retry on
that separate guard while keeping Body authority current. Only the provider
ordering branch changes; native subject-binding failures remain `StaleView`.
