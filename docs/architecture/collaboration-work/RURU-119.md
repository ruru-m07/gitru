# RURU-119 — Cached pull-request files and diff navigation

Status: pre-code contract frozen on 7 October 2026. The implementation starts
from exact signed RURU-137 head
`0da82cc7e5a3543512be683e68ac074f8d9689cd` in an isolated external-volume
worktree. RURU-124 owns migration 0012 and RURU-114 owns the next command
admission migration, expected 0013. This branch will stack on both frozen heads
before adding its own schema, expected 0014, so no concurrent migration is
renumbered during review.

Live issue: [RURU-119](https://linear.app/catra/issue/RURU-119/add-cached-pr-changed-file-and-diff-navigation),
“Add cached PR changed-file and diff navigation”. No existing RURU-119 branch,
worktree, pull request, or changed-file owner was found in the live audit.

## Outcome

Opening a pull request shows a locally cached, bounded file list immediately.
Opening one file reads a locally cached diff artifact immediately when present,
then creates foreground hydration intent when the exact base/head artifact is
missing or stale. Provider HTTP and optional local Git work remain in Rust. The
renderer receives typed summaries and bounded text/binary states through
generated IPC; it never fetches provider URLs, parses remote diff authority, or
loads an unbounded patch into JavaScript.

The common contract is provider independent. GitHub, GitLab and Bitbucket Cloud
adapters may use different endpoints and limits, but publish the same exact-range
facts and explicit completeness states. A provider or permission that cannot
serve a safe diff declares that capability unsupported or unavailable. It does
not fabricate an empty file or successful complete enumeration.

## Exact range authority

Every file generation is bound to one trusted `PullFileContext` captured from
the current pull Body metadata:

```text
base_oid
head_oid
base_repository_provider_id
source_repository_provider_id
body_metadata_facet_revision
```

The context uses provider-observed Git object IDs as opaque, canonical lowercase
hex values accepted by the existing 40/64-character rules. The source repository
is retained for forked pulls. An adapter echoes the complete context on every
page. A terminal page is published only after a fresh provider parent-range
validation still matches all four remote facts. A Body refresh, head/base change,
source-repository change, authorization-epoch change, or account replacement
retires the old generation from current capability evidence in one transaction.

Older generations remain historical cache candidates until bounded retention
removes them. They never satisfy the current file query or selected-diff request.
Changing a title, labels, comments, checks or another independent facet does not
invalidate an unchanged range.

## File summaries

The common `PullFile` model contains only bounded semantic facts:

* provider file identity when available;
* old path and new path as separate optional bounded UTF-8 values;
* normalized change kind: added, modified, deleted, renamed, copied,
  type-changed, unknown;
* provider-native change kind retained as a bounded string;
* optional additions, deletions and total changes as decimal strings when the
  provider supplies trustworthy values;
* mode/binary/generated indicators as typed known/unknown facts;
* provider order position and exact range context;
* a selected-diff availability hint, never an authority claim that content is
  empty.

Path identity is the tuple `(old_path, new_path)`, with an adapter-owned stable
provider ID as supplemental evidence. Rename and copy entries never collapse to
the destination path alone. Paths reject NUL, control characters, absolute
filesystem interpretation, parent traversal semantics and values above the
bounded byte limit. They are labels for remote repository content, never local
filesystem paths.

File enumeration uses a staging generation and atomic terminal publication like
pull commits. Partial pages are durable restart checkpoints but stay invisible as
an authoritative current list. A terminal provider/local cap publishes `capped`
with the exact reason and `remote_has_more`; an endpoint failure publishes no
partial replacement over the last exact current generation. Empty is
authoritative only after a complete, freshly validated zero-row generation.

Provisional bounds:

* 3,000 files per range, matching the documented GitHub list-files ceiling;
* 30 provider pages and 100 rows per provider page;
* 100 rows and about 1 MiB per local IPC page;
* 4 KiB per old/new path and 128 bytes per provider-native state;
* cursor depth 100 in the renderer, with repeated-cursor rejection;
* deterministic storage ordering by provider position, then normalized path
  tuple and provider ID.

The provider source/version, page cursors, completeness and cap reason are
persisted. GitHub’s documented maximum of 3,000 files is represented as provider
cap evidence, not “complete”. GitLab `overflow`, `collapsed` and `too_large`
signals remain distinct. Bitbucket pagination and diffstat omissions remain
explicit. Unknown future provider fields do not become inferred common facts.

## Selected diff artifacts

A `PullFileDiffRequest` names the account, subject, exact published file-facet
revision, exact range, and stable file key. The store resolves the trusted file
row; renderer-supplied URL, repository selector, arbitrary commit SHA or local
path is never accepted as fetch authority.

One artifact records:

```text
account + subject + range generation + file key
source kind and adapter version
content state
bounded unified text or opaque blob references
old/new blob object IDs when independently known
content type and binary/image hints
provider validation time and local last-access revision
logical and on-disk byte counts
```

Content states are `not_loaded`, `text`, `binary`, `image`, `omitted`,
`oversized`, `unsupported`, and `unavailable`. `omitted` means the provider did
not include content; `oversized` means Gitru deliberately refused content beyond
its bound; neither means an empty diff. A known zero-byte textual change remains
`text` with empty content. Binary/image artifacts carry bounded opaque blob IDs,
not renderer-selected file URLs.

Provider unified diff is treated as untrusted bytes. Rust bounds compressed and
decoded bytes, line count, maximum line length and parser work before storing or
returning it. The normal per-file decoded-text target is 2 MiB, hard maximum
4 MiB, 50,000 lines and 256 KiB per line. Larger results record `oversized`
without retaining their body. Whole-pull fallback streams stop at 16 MiB decoded
and never buffer an unlimited response. These are implementation constants and
can be tuned from measured evidence without changing identity semantics.

The artifact stores provider text as data. The UI renders with the existing safe
diff components and never injects provider HTML. Link/redirect validation uses
the configured provider instance and the existing credential-bearing transport
policy. A pagination or content URL from a response cannot redirect a token to a
different origin.

## Provider strategies

### GitHub

Use `GET /repos/{owner}/{repo}/pulls/{number}/files` with `per_page=100` for
summaries. The endpoint documents a maximum of 3,000 files and an optional
`patch`; a missing patch is omission evidence, commonly for binary or truncated
content, not an empty diff. Selected text can use an already bounded saved patch,
an exact local Git object range, or a bounded streamed pull `.diff` fallback.
Whole-pull fallback must isolate the exact path tuple and reject ambiguous parser
output. Terminal publication revalidates the parent pull base/head and source
repository immediately before activation.

Reference: [GitHub list pull request files](https://docs.github.com/en/rest/pulls/pulls#list-pull-requests-files).

### GitLab

Use the current merge-request diffs API and preserve `overflow`, `collapsed`,
`too_large`, `generated_file`, old/new paths and modes. `access_raw_diffs` is an
explicit higher-cost fallback through Gitaly and remains subject to Gitru’s byte
limits; it is never an automatic unbounded retry. The merge request’s diff refs
must exactly match the captured base/head before publication.

Reference: [GitLab merge request diffs](https://docs.gitlab.com/api/merge_requests/#list-merge-request-diffs).

### Bitbucket Cloud

Use pull-request diffstat for summaries and a bounded exact-spec diff request for
selected content. Do not use the removed `merge=true` behavior: Atlassian removed
that parameter on 4 September 2026. Preserve rename source/destination and binary
or truncation evidence. Revalidate source and destination commit hashes before
terminal publication.

References: [Bitbucket Cloud pull requests](https://developer.atlassian.com/cloud/bitbucket/rest/api-group-pullrequests/)
and [Bitbucket Cloud changelog](https://developer.atlassian.com/cloud/bitbucket/changelog/).

## Local Git path

An explicitly linked local clone is an optional accelerator and offline source,
never implicit provider authority. Rust resolves the saved repository link and
requires both exact base and head objects with `git cat-file -e`. It then invokes
Git with literal pathspec handling, no external diff driver, no textconv, no
replace objects, no hooks and no implicit fetch. The engine records
`local_exact_range` provenance. A missing object returns a clear local-unavailable
state and may leave provider hydration pending; it never fetches automatically.

Local results and provider results share the exact range/file identity but retain
their provenance. A later provider validation can supersede the local artifact;
content disagreements are observable diagnostics and do not rewrite the file
list silently.

## Storage and retention

After RURU-124 and RURU-114 freeze, the expected 0014 migration adds dedicated
normalized tables for file facets, staging/active generations, rows, cursors,
selected artifacts and bounded blob references. Large diff bodies do not use the
generic detail JSON table. Every relation includes `account_id`; composite foreign
keys prevent cross-account rows. Active generation switches and revision/change
records commit atomically through the serialized writer.

RURU-104 retention counts logical bytes and evicts inactive heavy selected diffs
before current file summaries. Current exact-range summaries may be evicted only
with coverage updated and a collaboration revision recorded. Pins and explicit
offline coverage protect chosen artifacts within their configured budget. Drafts,
commands and later review anchors receive normalized protection rows; retention
never scans diff text to find references.

Restart cleanup removes abandoned staging generations in bounded batches, resets
their sync leases and preserves the last active exact generation. Migration fault,
newer-schema refusal and backup/restore tests treat the new rows as rebuildable
cache unless protected by user intent.

## Runtime, query and UI

New provider traits are narrow and typed: one page method for range-bound file
summaries, one selected-file artifact method, and one fresh parent-range
validation. They return provider errors and cooldown evidence through the existing
rate budget path. They do not expose raw HTTP.

Foreground demand has separate scopes for the file collection and selected file.
Concurrent identical demand coalesces; switching files cancels or deprioritizes
abandoned selected hydration without cancelling the shared summary generation.
Account epoch, authorization view, Body facet revision, file facet revision and
selected-file identity are checked again at every commit boundary.

Generated IPC exposes local queries plus hydration intent. TanStack Query keys
include account ID/epoch, subject, exact file facet revision and selected file key.
Revision subscription invalidates only affected keys. The UI opens cached rows
without network waits, virtualizes long lists, keeps one selected artifact in the
active render tree, and supports Up/Down, Home/End and Enter with visible focus.
Loading, partial, capped, offline, permission, omitted, oversized, binary and
stale-range states have distinct text. The panel remains read-only.

## Required evidence

Native contract, storage, runtime and adapter tests cover:

* exact base/head/source binding and fresh terminal revalidation;
* head/base changes mid-page and mid-selected-diff fetch;
* complete empty, partial, provider-capped, local-capped and restart-resumed lists;
* rename/copy identity, Unicode paths, invalid paths and deterministic ordering;
* text, empty, binary, image, omitted, oversized, malformed and decompression
  limits;
* GitHub 3,000-file behavior, GitLab overflow/collapsed/too-large behavior and
  Bitbucket diffstat/diff behavior with finite synthetic transports;
* account/epoch isolation, access loss, rate limits, repeated cursors and
  credential-free offline reads;
* local Git exact-object success and missing-object refusal with zero implicit
  fetch, hook, textconv or external-diff execution;
* staged crash/restart, migration rollback, cache retention and protected anchors.

Client/UI tests cover local-only initial rendering, exact query keys, bounded
pagination, keyboard navigation, focus retention, revision invalidation, safe
text rendering and every content state. `make typegen` is the only source of
generated command changes. Run focused suites, `make verify`, the retained
packaged restart harness if public commands change, `git diff --check`, and
exact-head remote CI. Live private-provider and OS-platform evidence remains
separate from deterministic fixtures.

## Delivery order

1. Freeze this contract and publish the signed docs-only branch state.
2. Wait for RURU-124’s signed source, then RURU-114’s signed migration source;
   rebase on the exact resulting head and take migration 0014.
3. Implement common domain/storage/runtime and generated local query APIs.
4. Implement provider adapters in isolated file ownership, then common UI and
   local Git fallback.
5. Update the shared architecture/backlog with measured evidence, publish one
   reviewable stacked draft PR, attach it to Linear and monitor exact-head CI.

No merge is authorized.
