# RURU-99 — Private draft recovery

This slice extends the local collaboration foundation from PR #141. It provides
recovery for saved, user-authored drafts when an account is disconnected or its
provider subject is no longer available. Provider access is never required.

## Contract

- Recovery is reachable from each collaboration workspace before connected-account
  gating. Its account picker includes disconnected and authentication-required
  accounts using locally retained actor metadata.
- An account-bound, bounded keyset query lists subject IDs and draft generations.
  It does not join provider items, expose cached provider descriptions, or return
  every draft body. Individual bodies use the existing local draft read.
- Cursors bind to the account and use an exclusive subject-ID boundary. A local
  draft change or collaboration reset restarts pagination; ordinary provider
  sync does not. An open editor remains mounted through these resets.
- The shared editor keeps generation-based compare-and-swap saves. Concurrent
  changes preserve the user's text, and an explicit reload retains a copy of it.
  Opening or recovering a draft never dispatches provider writes.
- Authored query keys use immutable local account identity and subject, independent
  of provider authorization epochs. Revocation clears provider caches while keeping
  authored data and edited text. Lifecycle/stream resets cancel obsolete reads and
  invalidate retained authored projections, preventing missed draft events from
  making their local cache permanently stale.
- Copy is an explicit user action on the editor's current text. Native export
  captures the inspected saved generation before a system save dialog; the frontend supplies
  neither a path nor arbitrary text. Cancellation writes nothing, and failures
  expose only safe error messages. Later saves while the dialog is open cannot
  replace the captured text. Export requires saved, conflict-free text and uses a
  fixed `gitru-draft.txt` suggestion, a native-selected path, and a private atomic
  temporary file. It does not need broad filesystem grants.
- Recovery retains saved text. Explicit Save is required before leaving an editor;
  unsaved typing is not retained across navigation to another workspace or subject.
  Mounted recovery editors do preserve edits through same-actor authorization and
  stream refreshes. Broader composer autosave is a separate slice.
- Runtime provider/authentication logic and credential migrations remain outside
  this issue. Generated commands are rebuilt with `make typegen`.

## Validation plan

Storage checks cover restart, disconnected and missing subjects, keyset bounds,
cross-account cursor rejection, and concurrent generation conflicts. Client
checks cover query partitioning and draft-only subscriptions. UI checks cover
recovery without credentials, account switching, copy/export/cancel/failure and
conflict text retention. Native checks cover caller policy and the actual
export writer. Run relevant Rust/frontend checks, lint, types and desktop build;
record packaged native dialog inspection separately from automated writer tests.

## Evidence

Local automated checks completed on macOS:

- All 208 frontend/client tests passed, including 9 recovery UI tests and 13 client
  tests. They cover offline disconnected/missing subjects, delayed actor and subject
  reads, account replacement/disconnect fences, authored-cache retention, stream
  reset invalidation, pagination reset without editor loss, save/export/cancel/failure,
  exact native arguments, and conflict/failed-read text retention.
- The collaboration crate passed all 49 tests (24 unit, 9 runtime, 16 storage).
  Desktop native tests passed all 11, including exact UTF-8 export, cancellation,
  changed generations while a dialog is open, private file permissions, symlink
  replacement, failed-destination cleanup, and caller authorization.
- Workspace Clippy with warnings denied and Rust formatting checks passed. Native
  checks ran inside an outer lock that cleans branch packages when switching
  worktrees, preventing stale shared-target binaries from being counted.
- The generated 93-command bindings were rebuilt with `make typegen`. Type checks,
  lint, and the production desktop frontend build passed. Existing font/chunk
  build warnings remain outside this slice.

Native computer-use QA also passed on 2026-10-03 using a release binary with
packaged assets, a separate `com.ruru.gitru.draftqa99` identifier, an in-memory E2E
credential vault, disabled GitHub CLI discovery, and two synthetic disconnected
accounts. There were no tokens or provider item rows.

- Drafts opened with zero active accounts. The missing-subject draft for actor A
  rendered its authored Unicode text. Cancelling the actual system save dialog
  reported cancellation and retained the editor text; reopening the dialog then
  exported actor A's exact saved UTF-8 body, including its final newline.
- Keyboard selection in the native account picker switched to actor B, removed
  actor A's editor, and displayed actor B's different body for the same subject.
- Editing actor B enabled Save and disabled Export with the Save-first hint. Copy
  followed by a temporary edit and paste restored the exact current edited body.
  Saving committed generation 2, enabled Export, and the next native dialog wrote
  the exact new actor B body to the explicitly chosen QA file.
- Read-only inspection confirmed actor A remained generation 1, actor B became
  generation 2, provider item count stayed zero, and both exported files matched
  their respective saved account bodies. Quitting the QA app released its driver
  port. The pinned Base UI popup does not reliably settle under jsdom; this actual
  packaged keyboard interaction supplements the DOM inventory-switch tests.

The export uses the pinned Tauri dialog plugin's Rust callback API
([official dialog documentation](https://v2.tauri.app/plugin/dialog/)), rather than
relying on a WKWebView Blob download.

These are local results, with no remote CI or cross-platform dialog claim implied.

## Current-stack integration qualification — 5 October 2026

The existing review branch now contains signed RURU-103 exact head
`ba45cbda093b4b178a0937441c1718691ce92e60` through merge
`6e8cc8da556f642db1a04ff549e72eda64a09127`, plus
the RURU-99 recovery/export history and bounded repairs. An independent final
review caught that ordinary PR, issue and notification views still used a
duplicate save-only editor. Signed repair
`e0b565ce92c7381e8d4f97db28ff9d4c527f187a` consolidates them on the
shared Copy/Export editor, preserves the notification label and adds a normal-
detail generation-bound export control.

Fresh post-repair local gates pass: 43 affected UI tests; full `make verify` with
645 frontend/one platform skip across 66 files and 831 Rust/three ignored
(collaboration 476/two ignored); complete lint, types, fresh production build,
Rustfmt and all-target Clippy; feature tests 495 collaboration/two ignored plus
34 native app cases and feature Clippy. Normal typegen emits 119 commands, 322
schemas, 262 aliases and one event while an independent AST inventory preserves
the entire RURU-103 public contract and adds only the two draft commands and five
schema/alias families.

Fresh ordinary packaged E2E passes all three cases in
`artifacts/e2e/2026-10-05T09-23-07-938Z-94115`. Fresh retained native E2E passes
all five stages in `artifacts/e2e-harness/2026-10-05T09-25-00-937Z-96300`, using
binary SHA-256 `b7278812f63e7173d374ae30cb77880728da0912d61929e40424bfe489414465`.
Current automated native export controls use only synthetic drafts and owned
temporary paths. The earlier actual system-dialog QA remains ancestor evidence,
not an exact-current-head dialog claim. No personal credential or live provider
was inspected.

RURU-103's exact-head run 37281997503 passes all 14 reported checks. RURU-99's
old 11/11 matrix belongs to ancestor `eced62b`. The integrated implementation was
published and restacked without a force push as signed exact head
`b39b55f501e2207cea8e39de77fc267eb465ede0`. Exact-head run 37291579988 passes
all 11 Actions jobs across Linux, macOS and Windows plus Cloudflare, Vercel and
CodeRabbit, for 14 reported green checks; no CodeQL check is reported. This
evidence-only documentation commit requires a final exact-head matrix before
dependent destructive retention work starts. RURU-99 stays In Review, draft and
unmerged.
