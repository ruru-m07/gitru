# RURU-118 — Cached current-head checks and commit statuses

Status: source implementation complete and locally qualified on 8 October 2026 at signed source `f2c6f2e769c70e83531f047e8ef718508489bb52`. Review publication, exact-head remote CI, packaged execution, and live-provider sampling remain separate gates.

Baseline: exact signed RURU-137 evidence head `0da82cc7e5a3543512be683e68ac074f8d9689cd`, stacked through the signed RURU-77 pull-detail contract. The pre-code contract is signed at `613409bc99705b88574f45e7a660c28c3e4842ee`. RURU-118 is In Progress and has no review PR yet. RURU-77 is implemented in this ancestry; no merge is inferred.

Live issue: [RURU-118](https://linear.app/catra/issue/RURU-118/show-cached-checks-and-commit-statuses-for-the-current-pr-head), “Show cached checks and commit statuses for the current PR head”.

## Implemented outcome

Pull request details can render saved check runs and commit statuses immediately from SQLite, including after restart and while the provider and credential vault remain unopened. The common `CheckV1` model keeps check-run lifecycle/conclusion separate from commit-status state and carries a bounded name, description, producer, timestamps, and provider-native `allow_failure` evidence. An exact canonical head OID is stored on every row.

The shared aggregate is presentation evidence only. It reports an authoritative pass or failure only when the saved facet is ready, fresh, idle, complete, has no unread local continuation, has at least one row, and every row belongs to the caller's current head. Empty, pending, unknown, capped, partial, syncing, missing, unavailable, stale, and foreign-head sets remain non-authoritative. `allow_failure` is retained for display and never silently converts a failed status into a universal merge-safe success.

This slice is read-only. It does not rerun jobs, create statuses, inspect required-check or branch-protection policy, merge, inspect ambient credentials, add TypeScript provider HTTP, or make Gitru cloud sign-in a prerequisite.

## Native data and authority path

The implementation reuses `DetailFacet::Checks`, the existing independent detail observations/entries, sync scopes, foreground demand, authorization fencing, and retention accounting. There is no schema migration or second checks store. `NativeDetailPayload::CheckV1` and `DetailField::Check` extend the versioned native detail representation, while generated IPC exposes the same cache-only detail query.

Checks dispatch requires an authorized Body-derived `CheckContext`:

```text
head_oid
source_repository_provider_id
metadata_facet_revision
```

Missing or conflicting Body range metadata records Body demand before Checks can dispatch. The runtime validates account epoch, authorization view, provider instance, selected target repository, subject binding, exact head, source repository, Body facet revision, traversal generation, and cursor both before provider HTTP and before publication. A Body refresh, including the same head with a new Body revision or source repository, retires queued validators/cursors and rejects an old in-flight page. Prior-context rows remain retained but stale and non-authoritative.

## Provider implementations

All network work stays in Rust behind the account-selected provider adapter.

* GitHub reads the exact fork/source repository ID and immutable head SHA. It combines `GET /repositories/{source}/commits/{sha}/status?per_page=50` with `GET /repositories/{source}/commits/{sha}/check-runs?filter=latest&per_page=50`. Status and check-run totals are tracked independently and must remain stable through continuation. Each family is capped at 500 rows; either cap makes the combined reconciliation uncertain. Pagination pins the route, page order, page size, and `filter=latest`. Quota reported by the first family prevents the second request.
* GitLab reads `GET /projects/{target}/repository/commits/{sha}/statuses?all=false&order_by=id&sort=asc&per_page=100&page=…`, validates every returned SHA, monotonic native ID, total, and exact next route, and caps at 1,000 rows/10 pages. `failed` and `canceled` normalize to failure; `pending` and `running` normalize to pending; unknown bounded states remain unknown. `allow_failure` remains explicit provider evidence.
* Bitbucket Cloud reads the exact captured source repository UUID and head through `/repositories/{source}/commit/{sha}/statuses?pagelen=100`. Opaque continuations remain fixed-origin and exact-route, page fingerprints reject loops, returned commit links prove the captured head OID, and traversal caps at 1,000 rows/10 pages. The captured repository UUID and continuation route bind the source repository separately.

Adapters advertise Checks only with their implementation present. Authentication, permission, quota, transient, malformed, foreign-head, duplicate, contradictory-total, hostile-continuation, oversized-field, unknown-state, empty, and capped outcomes stay typed and cannot become false empty or false green evidence. Provider responses retain the existing 4 MiB transport ceiling; cursors remain versioned, context-bound, and capped at 4 KiB.

## Renderer behavior

`CachedChecksPanel` uses generated local IPC through `@gitru/collaboration-client`. The query identity includes the saved Body head and facet revision. Body or repository change events synchronously clear affected cached check and commit projections before active refetch, so a previous green generation cannot flash as current. Local reads do not hydrate; visible demand and the explicit **Sync checks** action enqueue native intent.

The panel gives semantic text and icon/color-independent states for missing, unavailable/denied, loading, syncing, stale, partial, empty, pending, failed, passed, and unknown observations. It labels non-authoritative evidence explicitly, reads every saved local page for the aggregate, displays only the bounded first page, and says that it does not determine required checks or merge eligibility. Authorization/facet-bound local cursors fail closed if the generation changes during that read.

## Local evidence

The following source-local checks pass on the implementation worktree:

* normal `make typegen`: 123 generated commands;
* final serialized `make verify`: 681 frontend/SDK/UI tests passed with one platform skip, plus lint, desktop/E2E types, the production desktop build, Rust formatting, warning-denied workspace Clippy, and every default Rust suite;
* the collaboration library reports 353 passed tests with one ignored process helper, plus all integration suites, including 19 facet reconciliation and 13 contextual capability tests;
* `bun --cwd packages/collaboration-client test`: 156/156 tests;
* focused desktop checks panel/contextual workspace tests: 17/17 tests;
* GitHub's focused check adapter suite: 11/11, including invalid initial `CheckContext` rejection before HTTP;
* two independent source/test audits reported no remaining blocker after the local paging/type boundary and GitHub context validation repairs;
* formatting and diff whitespace checks.

The source commit is signed and signature-verified locally. Packaged desktop/restart execution, remote CI, review publication, live GitHub/GitLab/Bitbucket accounts, private PAT scopes, production keyrings, Gitru cloud services, other-platform behavior, and provider-hosted required-check policy have not been exercised by this local fixture evidence.

## Remaining gates

1. Freeze and signature-verify the scoped evidence commit.
2. Publish a reviewable draft stacked on the exact RURU-137 base, attach it, and record exact-head remote CI separately.
3. Keep live private-provider/PAT/keyring and packaged multi-platform validation as explicit follow-up evidence; do not infer them from fixtures.

## Ownership

Branch `ruru/ruru-118-cached-checks` in the external managed worktree owns this document; check models and aggregation; provider adapters/fixtures; narrow shared detail validation; Body/check-context fences; generated IPC/client integration; the dedicated local-only checks panel/tests; and architecture/backlog progress records.

It does not own the RURU-137 or RURU-124 worktrees, credential flows, remote writes, guarded merge, required-check policy, enterprise/Data Center support, or the RURU-119 diff slice.
