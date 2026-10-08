# RURU-134 — local issue metadata authoring

Frontend contract before implementation, 8 October 2026. Read the native
[metadata contract](./RURU-134-metadata.md) together with this note. The existing
title/body issue-creation flow and PR201's definitive-admission error handling
remain the starting point. Root owns the SDK, frontend and this note; the native
lane owns models, storage, provider transport and delivery/recovery.

## Editor and local data

Extend the existing issue composer rather than introducing a second issue editor.
Use the generated version-two draft APIs once their native implementation is
registered. Until then, do not expose a metadata control that silently submits
only title/body. Title, body and metadata share the native CAS generation and one
Save action. Keep unsaved authored values mounted across dialog close/collapse.
An account epoch change retires provider authority without discarding text or
selected display names. A different account/actor/repository/draft owns a different
editor. Conflict refresh preserves the previous editor contents for explicit
recovery instead of overwriting them with another window's changes.

Labels and assignees allow bounded multiple selection; milestone is optional and
single selection. Selections come from typed, repository-bound cached options,
not free-form names or renderer-made provider requests. Render saved selections
even when their catalog is absent, stale, evicted or inaccessible. A saved name
is authored context; it does not establish that an option still exists or can be
applied. Removal remains possible without network access. Archived, closed or
otherwise explicitly unavailable options cannot be newly selected.

The first chooser uses the repository's local catalog with a bounded local
search and explicit next-page control. Empty and not-yet-synced states remain
distinct. Partial/capped coverage and stale data are visible without pretending
to enumerate the entire remote repository. Demand belongs to the mounted visible
chooser/composer and is released on closure, account retirement and unmount.
Opening a query reads SQLite; native demand and explicit Refresh own background
HTTP through the existing scheduler and quota boundaries. No frontend polling or
direct provider requests are added.

## Submission and receipt

Metadata-bearing submission requires its own clear best-effort acknowledgement:
the issue can be created even if some selected metadata is not applied. Bind both
this and background-delivery consent to the saved generation, current authority
and exact selection. Editing then reverting still retires consent. Save/send
through generated commands only. A lost local IPC receipt preserves the original
UUID and captured request; a definitive pre-admission InvalidInput failure lets
the user edit their saved draft. Never retry an uncertain create with a new UUID.

A strong creation receipt continues to show the created issue/link even when
metadata outcomes are Different or Unobserved. Show per-field outcomes as a
historical observation, including any attention needed. Do not infer missing
metadata was deleted, promise a current remote value, convert a valid creation
into failure or offer an automatic corrective PATCH/second POST. Provider receipt
outcomes and navigation redact on account/view retirement while authored
selections remain available in draft recovery.

## Query and verification boundaries

Version-two draft keys must not share incompatible version-one cache shapes.
Catalog keys bind account epoch, repository, kind and bounded query/cursor. The
SDK validates incoming epoch/view/revision fences, deep-copies mutable selection
arrays before native calls and synchronously removes catalog/send authority on
reset. Held responses cannot refill retired keys. Draft-change notifications
invalidate version-two drafts and recovery pages; catalog changes affect only
their repository/family. Pagination refuses stale cursors and bounds retained
pages/options in memory.

Verification covers saved/offline selections, remove and reselection, catalog
coverage/search/pagination, independent families, multiwindow CAS, consent
retirement, exact-receipt retry, metadata partial success and account reset during
held reads/writes. Exercise the real generated Zod wire shape and SDK/query/UI
integration. Run meaningful focused checks plus full lint/types/frontend suite
and production build after native IPC stabilizes. Native migration/proof tests,
remote CI, packaged windows and authenticated live-provider behavior are separate
qualification records. No implementation or test result is claimed by this note.

## Editor preparation checkpoint

The existing title/body editor now owns its session by account, actor and
repository rather than authorization epoch. Unsaved text survives an epoch
replacement; changing actor opens a separate editor. Background consent binds
the exact current saved context and runtime authority, and any title/body edit
retires it even if the user later restores the original text. Exact-receipt retry
also requires its captured runtime, epoch and authorization view.

The ten issue-composer controls pass, including new edit/revert, epoch replacement
and actor isolation cases. Targeted Biome and desktop/E2E TypeScript checks pass.
The initial new-worktree test attempt had no installed Vitest; a frozen copyfile
Bun install completed before validation. This checkpoint still uses version-one
issue APIs: metadata selectors, version-two SDK and native integration remain
pending the native schema24 qualification and version-two IPC registration.
