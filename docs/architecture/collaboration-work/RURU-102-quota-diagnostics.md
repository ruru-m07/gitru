# RURU-102 — durable quota diagnostics and cold-clock qualification

Design checkpoint: 8 October 2026. This bounded continuation starts from the
published RURU-102 fairness/ordering head
`079bc2f6554363f764323aad91ee31122d39a02f` (PR #191). RURU-102 remains In
Progress: this slice does not claim full provider-family isolation, real OS
suspend behavior, or a zero-request guarantee after an unknowable forward wall
clock reset.

## Problem and contract

The scheduler intentionally caps a single wake at 24 hours, then rechecks the
durable account deadline. `CollaborationRuntime::diagnostics` currently reuses
that capped wake calculation. After a cold reopen with a saved 48-hour
`provider:rest` wait, the runtime still blocks provider work for the full saved
duration, but diagnostics reports about 24 hours. The local support view must
report the full parseable wall-clock remainder without changing scheduler wake,
admission, retry, permission, or provider-I/O behavior.

All implemented GitHub calls currently share the non-search REST core budget;
GitLab and Bitbucket observations are layered or ambiguous. Keep the conservative
account-wide `provider:rest` floor for current, secondary, unknown, and legacy
observations. Do not add speculative request-family state or a migration.

Across a process restart, only the absolute UTC deadline survives. A backward
wall jump safely lengthens the wait. A forward jump beyond the deadline is
indistinguishable from real elapsed time, so the runtime may make one bounded
request. If that request observes a current rate limit, persist and install the
new floor before another account request; repeated scheduler wakes must neither
spin nor reload credentials, while an eligible peer remains serviceable. A
stronger zero-request promise after an invalid forward clock needs durable
boot/elapsed provenance and is outside this slice.

## Ownership and verification

Owned production scope is the local diagnostics deadline conversion in
`crates/collaboration/src/runtime.rs` (and a private clock helper only if needed).
Owned tests stay in native runtime clock/diagnostics fixtures. Documentation may
record the executed evidence. No schema, public Rust DTO, Tauri command,
generated TypeScript, SDK, UI, dependency, provider route, credential-vault, or
operating-system clock/sleep change is planned.

Before production repair, add a cold 48-hour diagnostics control and demonstrate
that unchanged code truncates the reported remainder. The repaired control must
prove the full remaining duration, explicit retry remains unavailable, and the
read performs zero vault loads and zero provider requests. Add deterministic
qualification for:

1. a backward-wall cold reopen, with no own-account vault/provider I/O, peer
   progress, and authored cache/draft CAS preserved;
2. a forward-wall-past-deadline cold reopen, allowing at most one request that
   returns a fresh synthetic 429, then proving the new durable/live floor stops
   repeated I/O/spin and a peer still progresses; and
3. shutdown/replacement/background restart with a still-valid saved deadline,
   proving a wake seeds the account floor before vault/provider I/O.

Use independent synthetic UTC/monotonic clocks, fake vaults and providers only;
do not change the machine clock, sleep the OS, or use credentials. Preserve the
existing long-clock, reconnect, account-fairness and provider-ordering controls.
Run focused RED/GREEN tests, the all-feature collaboration suite, strict native
Clippy, rustfmt and diff checks. Public IPC generation is not expected to change;
run the generator only if a public signature changes. Record local evidence,
ancestor CI and new exact-head remote CI separately. Publish as a scoped draft
stacked on PR #191; do not merge it or mark RURU-102 complete.

## Executed result — 8 October 2026

The pre-repair cold 48-hour control failed because diagnostics exposed the
24-hour scheduler wake cap. Production now converts the persisted absolute UTC
deadline directly for diagnostics while leaving scheduler admission and wake
calculation unchanged. The repaired control reports the full remaining wait,
keeps explicit retry unavailable, and performs no credential or provider I/O.
Deterministic backward-wall, forward-wall-with-fresh-429, replacement, shutdown,
background restart, peer-service, cache and draft-CAS controls also pass.

Focused clock/lifecycle and diagnostics validation passes 20 tests. The complete
native all-feature run passes **1,025 tests / 5 ignored / 0 failed** across 27
suites, including documentation tests. Workspace all-target/all-feature Clippy
with warnings denied and rustfmt pass. No public IPC, schema, migration,
dependency, SDK, UI, provider route or vault implementation changed, so typegen
was not run. These are local synthetic-clock/provider results; ancestor PR #191
CI and this branch's future exact-head CI remain separate, and no live provider,
vault, operating-system suspend or multi-family isolation claim is made.
