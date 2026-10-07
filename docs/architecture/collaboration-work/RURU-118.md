# RURU-118 — Cached current-head checks and commit statuses

Status: pre-code contract frozen on 7 October 2026. Implementation and validation remain pending.

Baseline: exact signed RURU-137 evidence head `0da82cc7e5a3543512be683e68ac074f8d9689cd`, stacked through the signed RURU-77 pull-detail contract. RURU-118 was Backlog with no PR, worktree, branch, comment, or duplicate implementation at the live pre-code audit. RURU-77 is In Review in draft PR #150 and is implemented in this ancestry; no merge is inferred.

Live issue: [RURU-118](https://linear.app/catra/issue/RURU-118/show-cached-checks-and-commit-statuses-for-the-current-pr-head), “Show cached checks and commit statuses for the current PR head”.

## Outcome

A pull request detail renders its saved check runs and commit statuses immediately from SQLite, including after restart and while offline. Every row and every aggregate is bound to the exact current pull-request head OID captured from authorized Body metadata. A previous-head success remains saved only as historical/stale evidence and can never authorize an “all passed” result for a new head.

This slice is read-only. It does not rerun jobs, create statuses, merge, inspect ambient credentials, add TypeScript provider HTTP, or make Gitru cloud sign-in a prerequisite.

## Existing engine seam

Use the existing `DetailFacet::Checks` and `ResourceFacet::Checks` lifecycle rather than adding a second checks store. The inherited runtime already coalesces durable detail demand, schedules native provider work, persists pages before publishing revisions, and exposes cache-only renderer queries. Its detail reconciliation evidence already records `CurrentHead`; storage requires a trusted `DetailSubjectBinding`, exact head OID on every row, and rejects continuations after summary or Body head drift.

The checks implementation therefore owns provider normalization, capability advertisement, focused exact-head fixtures, and a dedicated cached panel. It may extend the shared detail validation only when required by a typed common field. It does not weaken generic authorization, cursor, coverage, freshness, retry, retention, or subscription rules.

## Common row contract

The v1 common projection uses bounded `DetailEntry` fields with one stable provider-owned row identity:

```text
id             adapter namespace + native immutable row id
provider_id    provider immutable check/status id or stable native key
title          check name or status context
author         optional bounded producer/app/creator presentation
state          normalized lifecycle/outcome, preserving unknown native values safely
body           known / omitted / oversized provider description or summary
updated_at     comparable row timestamp when the endpoint supplies one
head_oid       exact canonical SHA-1 or SHA-256 OID from the captured PR head
```

Check runs and legacy/external commit statuses remain distinct identities even when their display names match. Known normalized states are `queued`, `running`, `pending`, `success`, `failure`, `neutral`, `cancelled`, `timed_out`, `action_required`, `skipped`, and `stale`; an unknown provider state is displayed as unknown and never treated as passing. Provider-specific raw state remains bounded in the saved state string when it cannot be mapped without loss.

The common renderer does not infer required-check policy, branch protection, mergeability, or approval. “All reported checks passed” is only a presentation over a same-head snapshot when coverage is complete, access is available, synchronization is not actively replacing the generation, at least one row exists, and every row is a terminal passing outcome. Partial/uncertain/capped coverage, no rows, permission loss, offline-first missing data, unknown outcomes, a changed head, queued/running work, and stale evidence all produce a non-authoritative explanation instead.

## Provider reads

All network work remains in Rust and uses the account-selected provider adapter and trusted repository/subject binding.

* GitHub combines the latest check runs for the exact head SHA with current commit statuses. Check runs use `GET /repos/{owner}/{repo}/commits/{ref}/check-runs?filter=latest&per_page=100`; the API documents a 1,000-check-suite limit, so that condition is explicit partial coverage. Commit statuses use the exact SHA and bounded pagination. Classic PATs need private-repository `repo`; fine-grained tokens expose separate read permissions for Checks and Commit statuses. A denial cannot become an empty successful set. Official references: [check runs](https://docs.github.com/en/rest/checks/runs?apiVersion=2026-03-10#list-check-runs-for-a-git-reference) and [commit statuses](https://docs.github.com/en/rest/commits/statuses?apiVersion=2026-03-10#list-commit-statuses-for-a-reference).
* GitLab reads `GET /projects/:id/repository/commits/:sha/statuses` against the numeric target project and exact head SHA. It requests latest statuses, follows bounded provider pagination, validates every returned `sha`, and preserves `allow_failure` only as provider-native presentation; an allowed failure is not silently converted into a universal merge-safe success. Official reference: [GitLab commit statuses](https://docs.gitlab.com/api/commits/#list-commit-statuses).
* Bitbucket Cloud reads `GET /repositories/{workspace}/{repo_slug}/commit/{commit}/statuses` against the captured head. Opaque `next` links are accepted only through the existing fixed-origin/exact-route continuation validator, and every returned commit link must resolve to the captured OID. Official reference: [Bitbucket Cloud commit statuses](https://developer.atlassian.com/cloud/bitbucket/rest/api-group-commit-statuses/#api-repositories-workspace-repo-slug-commit-commit-statuses-get).

An adapter advertises Checks only after its implementation and fixtures exist. Unsupported providers stay typed unsupported. Unknown response fields are additive; malformed identity, head, pagination, URL, state shape, or over-bound payload rejects the page atomically.

## Paging, completeness and bounds

Provider cursors are opaque, versioned, account/actor/epoch/repository/subject/head/strategy-bound, and capped at 4 KiB. Initial and continued requests use the exact immutable head OID, never a mutable branch name. A cursor cannot change endpoint family, filters, page order, repository, subject, or head.

Each provider page is at most 100 rows. One traversal is bounded to 20 pages and 1,000 normalized rows. Duplicate row identities, cursor loops/regression/skips, contradictory totals, foreign continuations, or a changed head fail closed. Full coverage is published only after a terminal traversal under a reconciliation strategy that can account for all intended endpoint families. Reaching a documented/provider/local cap publishes partial evidence, never complete emptiness or all-passed authority.

Names are at most 16 KiB through the shared limit, state strings 256 bytes, descriptions 65,536 bytes, timestamps RFC 3339 up to 128 bytes, URLs are not accepted as navigation authority, and provider responses retain the existing 4 MiB transport ceiling.

## Head changes, access and offline behavior

The trusted Body facet supplies the current head. Missing Body context schedules/coalesces Body hydration before Checks and preserves the original demand. A summary-only head cannot establish check authority. Any account epoch, authorization view, repository binding, Body metadata revision, or head change fences an in-flight page and makes the old snapshot stale before a replacement can publish.

Authentication or permission failures update typed capability/sync evidence and keep retained private bytes unreadable under policy. Offline, rate-limited, and transient failures may leave a same-context cached snapshot readable with its saved freshness and sync state. They cannot upgrade partial coverage, clear a known snapshot, or relabel old-head results as current.

## Frontend query and subscription behavior

The panel reads only generated local IPC through `@gitru/collaboration-client`. Visible demand and manual synchronization enqueue native intent; React query functions never call provider APIs. The panel renders missing, loading/syncing, stale, partial, denied, offline, empty, running, failed, passed, and unknown states distinctly. A head or Body revision change clears incompatible query data before refetch so prior-head green rows do not flash as current.

The list is keyboard-readable, uses semantic status text in addition to color/icons, and bounds the first local page. It retains provider-neutral wording; native provider states can appear as secondary text without provider branches in common view logic.

## Required evidence

* fake-provider exact-head complete, empty, partial, permission, offline, rate-limit, restart, coalescing, and stale-head tests;
* old-epoch and changed-Body/head completion rejection before publication;
* local cache reopen before fresh interest with zero provider/vault calls;
* provider fixtures for empty, multipage, malformed, duplicate, foreign-head, unknown-state, oversized, cap, permission, and quota responses;
* contextual capability evidence for missing, syncing, complete-empty/nonempty, partial, offline, and denied;
* frontend local-only reads, aggregate safety, query reset on head context change, and accessible state copy;
* normal `make typegen`, focused suites, `make verify`, and packaged restart coverage when the public/native surface or retained harness changes.

Local validation, exact-head remote CI, live public-provider sampling, private PAT/keyring behavior, and other-platform packaged execution are separate evidence. No merge is authorized.

## Ownership

Branch `ruru/ruru-118-cached-checks` in the external managed worktree owns this contract; the provider check/status normalizers and fixtures; narrow common detail validation if needed; provider capability advertisement; generated IPC/client output only through `make typegen`; a dedicated cached checks panel and tests; and concise architecture/backlog progress records.

It does not touch the active RURU-137 or RURU-124 worktrees, change credential flows, add remote writes, implement guarded merge or required-check policy, or claim enterprise/Data Center support.
