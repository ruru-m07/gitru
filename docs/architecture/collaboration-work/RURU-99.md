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

Native save-dialog and interactive multi-account picker inspection is still pending.
The DOM suite tests an account inventory replacement and ordering change. The pinned
Base UI popup does not reliably settle under jsdom; packaged inspection is recorded
separately. The export uses the pinned Tauri dialog plugin's Rust callback API
([official dialog documentation](https://v2.tauri.app/plugin/dialog/)), rather than
relying on a WKWebView Blob download.

These are local results, with no remote CI or cross-platform dialog claim implied.
