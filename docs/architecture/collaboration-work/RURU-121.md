# RURU-121: Bounded local prefetch and cached navigation

## Contract saved before implementation, 4 October 2026

Base: published R101/#156 `63e11970838c3bca1710f16175886f117e95aabb`, including
R77/R78 shared cached details and R98 native foreground demand leases. Live R121
is Backlog; prerequisites are implemented In Review, not Done. No existing R121
PR/worktree was found. R103 qualification is separate and not inherited. Read the
engine design/backlog before editing. Desktop accounts remain cloud-independent.

Provide a small bounded working set for pointer hover, keyboard focus and recent
navigation. Local reads warm existing QueryClient/SDK projections; automatic
interest may use existing ephemeral native leases and shared scheduler jobs.
Do not create durable hydration intents, a provider HTTP layer in TypeScript,
per-panel polling loops, or a second durable cache. SavedItemDetail stays the
ordinary shared view. Cached content renders immediately; absent/unsupported/
stale/loading states retain current explicit semantics.

Define explicit count, estimated resident byte and concurrent local-read budgets,
including queued candidates, completed cached entries, active reads and retained
interests. Repeated focus/hover must coalesce by exact account+epoch+instance+
subject/facet identity. Prefer deterministic LRU/TTL with bounded metadata;
large bodies must not bypass byte accounting. Inflight cancellation, hidden-tab
urgency release, account removal/epoch changes and revision reset must prevent
private late-result repopulation. Preserve selected item/editor state and real SDK
fences. Use existing native lease TTL/admission and capability/access checks.
Choose precise practical defaults after inspecting current SDK limits; record
actual limits before implementation, and avoid claiming exact heap measurement.

Own new SDK working-set module/tests, necessary SDK React/demand integration and
ordinary Workspace pointer/focus consumers/tests in this isolated worktree. Root
owns design progress/PR integration; no provider/storage migrations or generated
commands expected. If a shared backend change is necessary, present evidence
before editing. Read coss/react-useeffect skills for matching UI/hooks work.

## Required evidence

Deterministic large-list hover/focus navigation tests prove bounded queue/map/
bytes/concurrency, deduplicated exact identity, cancellation and hidden release.
Actual QueryClient/SDK regressions cover cache-first navigation, offline missing
states, reset/disconnect/account switch fencing, and no durable hydrate request.
Exercise real keyboard/pointer handlers through ordinary components, preserve
accessibility and selected drafts. Test production demand lifecycle behavior;
do not duplicate implementation with tautological mocks. Relevant SDK/UI tests,
repository lint/types/build and an isolated synthetic native flow where useful.
Distinguish estimated cache bytes from actual memory and local evidence from
remote platform/live provider qualification. Never inspect personal credentials.

All Cargo/build/generation commands use `/tmp/gitru-cargo-serial.py`; no direct
parallel Cargo. Signed scoped commits, reviewable PR based on #156, attach it;
do not merge. Root first advances the current R103 failure and native run.
