# RURU-125 — Native cached-navigation performance

Status: pre-code measurement contract, 8 October 2026. This isolated managed
worktree starts at signed RURU-114 head
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
