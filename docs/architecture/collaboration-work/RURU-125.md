# RURU-125 — Native cached-navigation performance

Status: implemented and measured locally on macOS; draft
[#173](https://github.com/ruru-m07/gitru/pull/173) is in review, 8 October 2026.
This isolated managed worktree starts at signed RURU-114 head
`de0e245d5b10bbebb9b66ad78d92febe084a986b` (PR #171). RURU-103's real native
webview harness and RURU-121's bounded cached-navigation implementation are
ancestors. RURU-119 is being integrated separately; its measurements must not be
silently attributed to this baseline.

Issue: [RURU-125](https://linear.app/catra/issue/RURU-125/measure-cached-navigation-latency-and-memory-through-native-ipc).

## Measurement boundary

Measure SQLite → normal generated IPC validation → collaboration SDK/TanStack
Query → useful React content in the packaged Tauri application. A storage-only
example, jsdom duration, mocked invocation, or screenshot timestamp cannot stand
in for this boundary. Record SQL/native read and IPC/UI timing separately when
both can be observed without distorting the measured path.

Use the existing separately compiled `collaboration-harness` fixture environment:
synthetic accounts and repository names, isolated SQLite and vault, denied real
provider networking, native caller guards and bounded fixture-only control.
No personal database, PAT, CLI authentication or system credential inspection is
part of this task. No benchmark controls are registered in production builds.

## Repeatable cases

Seed a deterministic 10,000-item cache with bounded bodies, at least two synthetic
accounts and enough selected repositories to exercise account/repository scoping.
Record the exact dataset shape, revision, executable source SHA and build profile.
Use real shared list/detail components. Choose deterministic records and search
terms, avoiding an unbounded renderer-side dataset preload.

Measure warm list, search and detail navigation with repeated samples; label
same-query memory hits separately from local-IPC cache reads. Include fresh-process
cold restart using the saved database, and multiple retained native views. Measure
cold useful content from native/runtime readiness as well as process launch when
the observer can establish both clocks. Check exact useful DOM content to prevent
an empty/loading view from passing as fast navigation. Provider/vault counters
must not rise during cache-only measurements.

Record raw bounded samples and nearest-rank p50/p95/p99, sample count, response
payload bytes, readiness/startup time, database/WAL sizes before and after, and
process memory using a documented platform mechanism. Attribute Rust and webview
memory only when the observed process ownership is proven. Unsupported counters
are explicit missing observations, never zero. Report warm-up, repeat count,
hardware/OS, storage medium and any UI visibility limitations.

## Interpretation and regression gates

Compare with architecture section 18's provisional warm useful-content p95 below
100 ms, local query/IPC below 30 ms and cold cached content below 500 ms after
readiness. The larger 100,000-summary/500,000-child-record memory target remains
unqualified by a 10,000-item run. Do not call an external SSD result a local
internal-SSD result, or infer other supported platforms from macOS.

The first measured baseline determines justified regression bounds. Keep hard
correctness gates for exact content, no provider/vault access, bounded IPC and
valid finite samples. Avoid flaky universal timing limits based on one workstation.
Record reproducible bottlenecks and distinguish them from observer overhead.

## Delivery

Keep fixture seeding, bounded measurement protocol, validation and the runner in
this isolated worktree. Generate any changed IPC with `make typegen`. Run focused
protocol/native tests, normal local checks and the packaged measurement. Preserve
the raw artifact path and summarize actual evidence in this note and the shared
architecture progress record before opening a signed scoped PR. Remote CI,
platform-specific measurements and live-provider evidence stay separate.

## Implemented benchmark

The feature-only `collaboration-harness` now seeds two synthetic accounts, five
selected repositories per account and 5,000 pull summaries per account. It uses
fixed clocks, bounded 100-row seed pages, deterministic detail records and 30
known search needles. Seeding and measurement run against an isolated SQLite
database and fake vault with provider networking denied. Normal builds register
none of the seed or benchmark controls.

The packaged runner measures a real React collaboration workspace in two retained
native views. Each sample waits for exact useful list, search or detail content,
then records the React useful-content boundary, the generated SDK/IPC call and a
same-query TanStack memory read separately. The seed process collects 30 main-view
samples and 10 child-view samples. A distinct packaged process reopens the saved
database and collects 10 samples in each view. Native SQLite projection timings
are recorded from the ordinary commands in a bounded 512-entry harness buffer.

Process RSS comes from `ps` and is accepted only for the exact native PID or
proven descendants. Database, WAL and shared-memory byte sizes are sampled before
and after each phase. The runner records the source commit, release executable
SHA-256, hardware and storage probes. It checks that provider calls and vault
loads do not increase.

## Measured baseline

The corrected retained local report is
`artifacts/collaboration-performance/2026-10-07T21-16-45-421Z-78475/performance-report.json`.
Artifacts are intentionally ignored by Git; the identifying evidence is:

- report trace head: `0a74b3963645809a723925fbe48fb16ff63d58db`
- signed measured-source commit: `07aba1e61cb9a6140ca937c9e5cd2f8e122f46fe`
- build: `release/no-bundle/collaboration-harness`
- executable SHA-256:
  `c62fe45d22c4e9631d5cfdd5fddcae9aeadeb10317a155b2701d090cca1838e8`
- machine: Apple M4, arm64, 10 logical CPUs, 16 GiB, macOS/Darwin `27.0.0`
- fixture location: `/Volumes/Lexar`; the OS storage probe did not establish its
  device location or solid-state property, so the report classifies the medium as
  `unverified`

The report records the checked-out trace head because the corrected harness was
measured before its commit was created. The passing source tree was committed
immediately afterward as `07aba1e` without changing code or assets; the recorded
binary was built from those exact bytes. The trace head and signed measured-source
commit are kept distinct rather than rewriting the raw artifact.

Nearest-rank distributions use integer-millisecond browser observations. Main
seed uses 30 samples; the retained child and both cold-restart views use 10 each.

| Phase/view | React list p50/p95/p99 | React search p50/p95/p99 | React detail p50/p95/p99 | SDK list/search/detail p95 |
| --- | ---: | ---: | ---: | ---: |
| Seed main | 40/59/71 ms | 20/22/29 ms | 30/44/50 ms | 1/2/1 ms |
| Seed child | 30/40/40 ms | 20/22/22 ms | 30/32/32 ms | 2/3/2 ms |
| Restart main | 30/31/31 ms | 20/22/22 ms | 30/32/32 ms | 2/1/1 ms |
| Restart child | 30/34/34 ms | 20/22/22 ms | 30/32/32 ms | 1/2/1 ms |

The 50-row list response is 32,231 bytes, the deterministic search response is
904–905 bytes and the detail response is 681 bytes. Same-query memory reads round
to 0 ms at the runner's integer-millisecond resolution; this establishes only
that they are below that observer's resolution, not that their cost is zero.

The cold restart opened the native collaboration runtime in 217.493 ms. Its main
view automatically mounted the real collaboration workspace and reached exact
useful cached content 142 ms after runtime readiness and 302 ms after document
navigation. Navigation-to-mount was 174 ms and mount-to-useful was 128 ms. The
benchmark request arrived 1,619 ms after useful content, proving the cold result
did not wait for test-driver orchestration.

The retained restart child automatically reached useful content 249 ms after its
own navigation (105 ms to mount and 144 ms from mount to useful). Its
runtime-ready delta is 3,530 ms because both views share a runtime created before
the child; that shared clock is not a child cold-start boundary. Seed-main startup
remains request-mounted because it also performs fixture seeding and orchestration:
164.169 ms native open and 14,554 ms runtime-ready-to-useful. The automatic seed
child reached useful content 300 ms after its own navigation. Process-launch
latency is not claimed because the runner does not establish a shared trustworthy
launch clock.

The native Rust process used 157,581,312 bytes (150.28 MiB) RSS after seed and
147,668,992 bytes (140.83 MiB) after restart. No WebKit process was a proven
native descendant, so WebKit aggregate and per-view RSS are explicitly missing.
The seeded database was 40,521,728 bytes, with a 4,931,672-byte WAL and
32,768-byte shared-memory file (45,486,168 bytes total); all three sizes were
unchanged by the restart measurement.

Ordinary item projection p95 was at most 3.612 ms. Contextual-capability
projection p95 was 24.462 ms in seed main, 41.168 ms in seed child, 9.839 ms in
restart main and 25.770 ms in restart child. Only the seed child exceeds the
provisional 30 ms native projection target; every end-to-end generated SDK sample
stayed at or below 3 ms p95.

## Target assessment and limits

The section 18 warm useful-content target passes: every measured list, search and
detail p95 is at most 59 ms against the provisional 100 ms target. The local
generated SDK/IPC target passes at no more than 3 ms p95 against 30 ms. The cold
cached main landing also passes at 142 ms after runtime readiness against the
provisional 500 ms target.

An earlier report measured 2,010 ms because the performance fixture deliberately
left the workspace unmounted until a WebDriver benchmark request. Raw timestamps
showed the request arrived roughly 1.9 seconds after navigation while the first
list render itself took 46 ms. The correction makes saved performance fixtures
mount the real workspace during bootstrap and preserves the original
runtime-ready-to-useful metric. This is a benchmark observer correction, not a
production performance optimization and not a weakened target.

This 10,000-summary fixture does not qualify the 100,000-summary,
500,000-child-record or combined sub-100-MiB memory target. Combined collaboration
memory is unknown because WebKit RSS could not be attributed. It also supplies no
Windows/Linux performance, live-provider, personal credential, production
keyring, internal-SSD or remote-CI evidence.

## Validation

Normal `make typegen` generated 125 commands. Final full `make verify` passed 688
frontend tests with one platform skip, lint, types, the production desktop build,
Rust formatting, workspace Clippy and every default Rust suite. Focused corrected-
harness validation passed 40 protocol, React and process-observer tests plus
Desktop typecheck, Biome, Rust formatting and feature Clippy. The corrected
release harness build and both packaged processes passed; exact content, two
retained views, generated IPC validation and unchanged provider/vault counters
are hard correctness gates.

The first packaged attempt stopped because the workspace deterministically opened
the alternate account while the assertion expected the primary account. Pinning
the measured account fixed the fixture and demonstrates that wrong-account useful
content cannot be counted as a successful fast render. Remote CI and other
platforms remain separate evidence.
