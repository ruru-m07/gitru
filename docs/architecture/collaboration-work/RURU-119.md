# RURU-119 — Cached pull-request files and diff navigation

Status: implementation in integrated validation on 8 October 2026. The pre-code
contract was frozen on 7 October. The branch now builds on exact signed RURU-114
head `de0e245d5b10bbebb9b66ad78d92febe084a986b`, after RURU-124 migration 0012
and RURU-114 migration 0013. RURU-119 owns migration 0014 in its isolated
external-volume worktree.

Live issue: [RURU-119](https://linear.app/catra/issue/RURU-119/add-cached-pr-changed-file-and-diff-navigation),
“Add cached PR changed-file and diff navigation”. One owned branch and worktree
hold the integrated slice; no duplicate RURU-119 PR is opened.

## Outcome

Opening a pull request shows a locally cached, bounded file list immediately.
Opening one file reads a locally cached diff artifact immediately when present,
and offers an explicit foreground load when the exact-range artifact is missing.
Visible file-list demand synchronizes summaries through the existing scheduler. Provider HTTP and optional local Git work remain in Rust. The
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
merge_base_oid (optional provider observation)
base_repository_provider_id
source_repository_provider_id
body_metadata_facet_revision
```

`base_oid` is the provider-observed target tip, not the merge base. Every current
closed source strategy has `MergeBaseToHead` semantics. An absent merge-base OID
means the provider did not expose it; it never means to compare the two tips
directly. A newly observed merge base changes context and starts a new generation.

The context uses provider-observed Git object IDs as opaque, canonical lowercase
hex values accepted by the existing 40/64-character rules. The source repository
is retained for forked pulls. An adapter echoes the complete context on every
page. A terminal page is published only after a fresh provider parent-range
validation still matches every comparison fact. A changed range, newly observed merge base, source-repository change,
authorization-epoch change, or account replacement retires the old generation
from current capability evidence in one transaction. Equivalent Body refreshes,
including title changes and conditional validation, preserve the file generation.
`body_metadata_facet_revision` records when that exact range last became current;
it is stable across equivalent Body observations. Feed head invalidation also
retires current and staging authority, preventing an A-to-B-to-A head sequence
from resurrecting an old generation before a fresh Body observation.

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
not renderer-selected file URLs. This slice defines and validates blob references
but does not register or fetch blob bytes. A local binary detection therefore
records `omitted` with a known binary hint, not an available image or binary body.

Provider unified diff is treated as untrusted bytes. Rust bounds compressed and
decoded bytes, line count, maximum line length and parser work before storing or
returning it. The hard selected-text maximum is 4 MiB, 50,000 lines and 256 KiB per line.
Larger results record `oversized` without retaining their body. The providers'
JSON transports retain their existing decoded-response bounds; Bitbucket's
selected diff uses incremental bounded UTF-8 collection. No whole-pull raw-diff
fallback is implemented. These constants can be tuned from measured evidence
without changing identity semantics.

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
content, not an empty diff. An explicit selected read requests one provider row
with `per_page=1` and its saved ordinal. It verifies the returned old/new identity
and then freshly validates parent base/head/source before storing the bounded
patch. Provider hunk text remains provider text; the UI supplies trusted path
labels from membership without inventing full-file content. Terminal list
publication uses the same separate fresh parent validation. A linked local Git
object range provides an optional independent source.

Reference: [GitHub list pull request files](https://docs.github.com/en/rest/pulls/pulls#list-pull-requests-files).

### GitLab

Use the current merge-request diffs API and preserve `overflow`, `collapsed`,
`too_large`, `generated_file`, old/new paths and modes. Selected reads request
one saved ordinal with `per_page=1`, verify both path sides, and separately
validate the parent. Collapsed/too-large content remains explicit; raw Gitaly
diff fallback is deferred. The merge request's `diff_refs.start_sha`, `base_sha` and `head_sha` must match
the captured target tip, merge base and head independently before publication.
Ordinary divergent branches are valid. Asynchronously missing diff refs do not
become empty or permanently unsupported results. The current `/diffs` response
has no top-level overflow flag; fresh `changes_count` evidence qualifies terminal
completeness, with `1000+` retained as a provider limit and unknown counts unable
to prove a complete enumeration.

Reference: [GitLab merge request diffs](https://docs.gitlab.com/api/merge_requests/#list-merge-request-diffs).

### Bitbucket Cloud

Use a fixed-origin, exact-spec diffstat with explicit `topic=true` for summaries
and a bounded exact-spec diff request with the same semantics for
selected content. The path-filtered response must prove exactly one selected
file: independent old/new headers or rename/copy sides disambiguate path names
that contain ` b/`; a combined unquoted header alone cannot prove a renamed
identity. Quoted/octal paths are decoded and checked, and a hunk before identity
proof is rejected. Do not use the removed `merge=true` behavior: Atlassian removed
that parameter on 4 September 2026. Preserve rename source/destination and binary
or truncation evidence. Revalidate source and destination commit hashes before
terminal publication.

References: [Bitbucket Cloud pull requests](https://developer.atlassian.com/cloud/bitbucket/rest/api-group-pullrequests/)
and [Bitbucket Cloud changelog](https://developer.atlassian.com/cloud/bitbucket/changelog/).

## Local Git path

An explicitly linked local clone is an optional accelerator and offline source,
never implicit provider authority. Rust resolves the saved repository link and
requires both exact base and head objects with `git cat-file -t`, resolves
`git merge-base --all`, and requires exactly one canonical result. If the
provider supplied a merge-base OID it must equal that result. The local artifact
records the resolved merge base as additional evidence and compares it to the
head; it never substitutes a direct tip-to-tip diff. It then invokes
Git with literal pathspec handling, no external diff driver, no textconv, no
replace objects, no hooks and no implicit fetch. Selected merge-base/head tree
entries are checked before diff/rename work: only existing blobs within 8 MiB
per side and 12 MiB combined enter the bounded 20-second diff operation.
The engine records
`local_exact_range` provenance for this merge-base comparison. Multiple best
merge bases are unavailable locally. A missing object returns a clear local-unavailable
state and may leave provider hydration pending; it never fetches automatically.

Local results and provider results share the exact range/file identity but retain
their provenance. A later provider validation can supersede the local artifact;
both sources remain bound to the published file membership. Local publication
acquires the serialized writer before rechecking caller lifetime and the exact
saved clone link, registration proof, account and repository affinity inside the
artifact transaction. It rechecks caller lifetime again before commit. Provider
artifact admission rejects local provenance, so this guard cannot be bypassed.

## Storage and retention

Migration 0014 adds dedicated
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

Restart cleanup examines staging rows in pages of 128, preserves valid durable
checkpoints, and removes staging authority whose account/range/access is stale.
It resets abandoned syncing state while preserving the last valid active range.
Retention reclaims at most one superseded generation per maintenance turn and
uses normalized indexed protection rows for authored anchors. Frozen-v13
migration tests cover rollback, disk-full/interrupted failure, checksum and
newer-schema refusal. Full user-driven backup/restore remains RURU-106.

## Runtime, query and UI

New provider traits are narrow and typed: one page method for range-bound file
summaries, one selected-file artifact method, and one fresh parent-range
validation. They return provider errors and cooldown evidence through the existing
rate budget path. They do not expose raw HTTP.

Visible foreground demand synchronizes the file collection. An explicit selected
load schedules a separate bounded job keyed by generation and file identity;
identical jobs coalesce. Successful content fetch and fresh parent validation
consume separate provider budget turns. A cooldown prevents publication without
fresh parent validation; the next attempt re-reads content. An explicit job has
at most five deferred transport retries. Switching selection clears rendered
artifact state but does not cancel the native bounded job. Account epoch,
authorization view, stable Body range revision, file facet revision and selected
identity are checked again at commit boundaries.

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

## Integrated local qualification — 8 October 2026

The signed implementation is `df54c444771d5c6b671080eafc6c4b3daf3c44ab`
(on RURU-114 `de0e245d5b10bbebb9b66ad78d92febe084a986b`). Normal
`make typegen` emits 129 commands and the generated schema inventory is 386.
Full `make verify` exits successfully: 712 frontend/SDK/UI tests pass with one
platform-only skip, all lint/types/build/format/strict workspace Clippy gates
pass, and 1,076 default Rust tests pass with five intentionally ignored
standalone subprocess entrypoints. The collaboration slice contributes 656
passing tests and three helper ignores; selected provider tests include 17 cases,
with 11 file-runtime cases, 17 storage-file cases, four native writer races and
five frozen-v13 migration cases. The Git accelerator passes 13 integration cases.

The real macOS release-mode retained harness also passes all five processes:
main scenarios, crash-before/restart-before and crash-after/restart-after. Run
`2026-10-07T20-42-09-968Z-80050` records binary SHA-256
`4a27f0d3d339362da655c35bca1396e71534d883d7a398ad3fceb9b4849578c6`.
This qualifies the existing real-webview lifecycle, generated IPC and durable
restart integration against this source. Selected-file provider transport,
publication and rendering are covered by the focused finite Rust/SDK/UI suites;
the retained harness does not claim a live provider diff or OS file-picker test.
No personal credential or production vault was read. Remote CI and other-platform
execution for this final head remain separate gates. No PR is merged.

### Windows fixture repair — 8 October 2026

The exact-head Windows Rust job for `218c610` failed in
`treats_special_unicode_paths_as_one_literal_identity` before reaching the
production service: Windows rejected creation of `:(glob)[x]*雪?.txt` with OS
error 123. The fixture now writes blobs and NUL-delimited trees with Git plumbing,
then creates the two commits directly. Every platform exercises the same literal
path and excludes the unrelated changed file without requiring that path to be
representable in the host filesystem. Production code is unchanged. All 13
`pull_file_service` cases pass on macOS after the change; new-head Windows CI
remains the platform gate. The previous Windows failure is not recorded as a
passing platform result.
