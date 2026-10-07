# RURU-125 — Native cached-navigation performance

Status: implemented and measured locally on macOS, 8 October 2026. This isolated
managed worktree starts at signed RURU-114 head
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

The retained local report is
`artifacts/collaboration-performance/2026-10-07T20-59-32-918Z-23699/performance-report.json`.
Artifacts are intentionally ignored by Git; the identifying evidence is:

- source: `e136d9cf933e986018b00f2ad561fbba9b9b11a1`
- build: `release/no-bundle/collaboration-harness`
- executable SHA-256:
  `af31149f21d80f22904a78239826d7a32970df7c4131e80b8dd23c9f8d5c5d6b`
- machine: Apple M4, arm64, 10 logical CPUs, 16 GiB, macOS/Darwin `27.0.0`
- fixture location: `/Volumes/Lexar`; the OS storage probe did not establish its
  device location or solid-state property, so the report classifies the medium as
  `unverified`

Nearest-rank distributions use integer-millisecond browser observations. Main
seed uses 30 samples; the retained child and both cold-restart views use 10 each.

| Phase/view | React list p50/p95/p99 | React search p50/p95/p99 | React detail p50/p95/p99 | SDK list/search/detail p95 |
| --- | ---: | ---: | ---: | ---: |
| Seed main | 30/42/60 ms | 20/22/23 ms | 30/32/32 ms | 2/2/2 ms |
| Seed child | 31/51/51 ms | 20/22/22 ms | 30/30/30 ms | 3/2/2 ms |
| Restart main | 40/51/51 ms | 20/32/32 ms | 30/32/32 ms | 5/7/4 ms |
| Restart child | 38/50/50 ms | 20/22/22 ms | 30/32/32 ms | 2/2/2 ms |

The 50-row list response is 32,231 bytes, the deterministic search response is
904–905 bytes and the detail response is 681 bytes. Same-query memory reads round
to 0 ms at the runner's integer-millisecond resolution; this establishes only
that they are below that observer's resolution, not that their cost is zero.

The fresh process opened the native collaboration runtime in 7.876 ms. Its main
view reached exact useful cached content 2,010 ms after runtime readiness and
1,958 ms after document navigation. The retained child reached it 13,794 ms after
runtime readiness and 10,169 ms after its navigation. Seed startup includes the
10,000-row seed and is recorded separately: 229.321 ms native open, 23,193 ms
main readiness-to-useful and 37,853 ms child readiness-to-useful. Process-launch
latency is not claimed because the runner does not establish a shared trustworthy
launch clock.

The native Rust process used 157,319,168 bytes (150.03 MiB) RSS after seed and
152,551,424 bytes (145.48 MiB) after restart. No WebKit process was a proven
descendant, so WebKit aggregate and per-view RSS are explicitly missing. The
seeded database was 40,521,728 bytes, with a 4,931,672-byte WAL and 32,768-byte
shared-memory file (45,486,168 bytes total); all three sizes were unchanged by the
restart measurement.

Ordinary item projections stayed below 9.546 ms p95. Contextual-capability
projection p95 reached 43.943 ms in the seed child and 36.868 ms in the restart
main view, with a 54.946 ms seed-main p99. The end-to-end SDK samples stayed below
7 ms p95, but repeated contextual-capability projection is the clearest measured
native optimization candidate.

## Target assessment and limits

The section 18 warm useful-content target passes: every measured list, search and
detail p95 is at most 51 ms against the provisional 100 ms target. The local
generated SDK/IPC target passes at no more than 7 ms p95 against 30 ms. The cold cached
landing target does not pass: the fresh main view took 2,010 ms after runtime
readiness against the provisional 500 ms target. This is a measured baseline and
optimization input, not a universal timing gate from one workstation.

This 10,000-summary fixture does not qualify the 100,000-summary,
500,000-child-record or combined sub-100-MiB memory target. Combined collaboration
memory is unknown because WebKit RSS could not be attributed. It also supplies no
Windows/Linux performance, live-provider, personal credential, production
keyring, internal-SSD or remote-CI evidence.

## Validation

Normal `make typegen` generated 125 commands. Final full `make verify` passed 688
frontend tests with one platform skip, lint, types, the production desktop build,
Rust formatting, workspace Clippy and every default Rust suite. The final release
harness build and both packaged processes passed; exact content, two retained
views, generated IPC validation and unchanged provider/vault counters are hard
correctness gates.

The first packaged attempt stopped because the workspace deterministically opened
the alternate account while the assertion expected the primary account. Pinning
the measured account fixed the fixture and demonstrates that wrong-account useful
content cannot be counted as a successful fast render. Remote CI and other
platforms remain separate evidence.
