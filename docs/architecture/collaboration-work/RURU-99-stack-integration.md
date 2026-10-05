# RURU-99 current-stack integration contract

Status: signed pre-integration contract, 5 October 2026. No combined-tree
qualification is claimed by this document.

## Frozen inputs and delivery shape

The existing draft PR #144 publishes signed RURU-99 head
`eced62bf6af0144bcd3449ad2357ebd94f742c2b` on
`ruru/remote-collaboration`. Its 11 reported checks pass, including Rust and
ordinary packaged E2E on Linux, macOS and Windows. That matrix qualifies only the
published ancestor. Preserve its two signed feature/evidence commits and existing
PR; do not rewrite history, open a duplicate PR or merge anything.

The current collaboration review stack ends at signed RURU-103 head
`ba45cbda093b4b178a0937441c1718691ce92e60`, whose base is signed RURU-102 head
`d08fa56c2bf0399b7f1a1fe5fd3ecbd03f9e1fc4`. RURU-103 has fresh exact-head local
qualification, while its new remote matrix is still running. Integrate that exact
head into this existing RURU-99 review branch with a signed merge commit only
after this contract. Change PR #144's base to the RURU-103 branch only after a
clean combined tree and fresh local gates. Use ordinary fast-forward push.

RURU-99 remains In Review. RURU-104 stays Backlog until this recovery/export code
is actually present and qualified on the current stack. No merge authorization
exists.

## Semantic boundaries

Preserve RURU-99's account-scoped, bounded draft listing; disconnected and missing-
subject recovery; generation-safe native export; current-text copy; explicit save
before export; cancel behavior; atomic private temporary file; and rule that
recovery never sends. Preserve RURU-103/RURU-102 provider, Tasks, Comments, clock,
privacy, WriterLease, own-Webview event, visibility, retained harness and generated-
schema ordering behavior. Account identity must remain part of every draft key and
query. Provider bodies and other actors' drafts must remain inaccessible.

Read-only merge-tree finds content conflicts in native Cargo/command registration,
desktop workspace composition, collaboration-client exports/hooks/tests and all
generated command files. Resolve native registration and UI/client files as a
semantic union. Do not restore obsolete command arrays, legacy global revision
listeners, old workspace panels or pre-facet/pre-Tasks assumptions. Restore the
current-stack generated files as a temporary baseline if needed, then run normal
`make typegen`; never hand-edit generated output. Preserve the complete current
117-command/317-schema/257-alias/one-event contract and add only the RURU-99 draft
list/export IPC definitions supported by combined Rust sources. Investigate any
other removal or initializer/signature change.

No migration, provider call, credential flow, automatic send, retention policy or
RURU-104 eviction is authorized in this integration. Existing RURU-99 storage uses
the current database and authored draft rows; it must not introduce a new account
source of truth. If integration reveals an actual defect, record the failing
control and a bounded repair contract before editing beyond conflict resolution.

## Qualification and evidence

Serialize native generation and builds. Required local gates are normal typegen
plus independent AST inventory, focused storage/account/generation/export tests,
full default `make verify`, and ordinary packaged E2E. Run the feature-gated RURU-103
core/native tests and retained five-stage pipeline again if any conflict or repair
touches harness registration, SDK revision handling, storage/runtime behavior or
generated fixture commands. Native export QA must use only synthetic saved drafts
and a task-owned temporary destination; do not inspect personal accounts, provider
items, credentials, vaults or arbitrary files.

Record old RURU-99 remote CI, current-stack local/remote evidence and new combined-
head validation separately. After signed result documentation and ordinary push,
update existing PR #144 and Linear RURU-99 with exact head/base/results, attach no
duplicate artifact, and inspect actual new CI failures before more work. Keep the
PR draft and unmerged. Only a qualified published RURU-99 current-stack head may
unblock the first bounded RURU-104 retention slice.
