# RURU-137 — Cached pull request commit lists

Status: Locally qualified on 7 October 2026; draft publication and exact-head remote CI remain pending.

Baseline: signed RURU-136 draft [PR #167](https://github.com/ruru-m07/gitru/pull/167) exact head `9548c53cf3c926a47e474d8d9b9511bc41e99d41`, which contains the signed RURU-77 pull-detail prerequisite. All 14 checks reported for that exact head pass, including Rust, ordinary packaged E2E and the collaboration harness on Linux, macOS and Windows plus Cloudflare, Vercel and CodeRabbit. This branch is stacked and does not turn that ancestor result into evidence for its own source head.

Live issue: [RURU-137](https://linear.app/catra/issue/RURU-137/cache-and-navigate-the-pull-request-commit-list), “Cache and navigate the pull request commit list”.

## Outcome

A pull request detail can render its saved commits immediately and offline in deterministic base-nearest-to-head order. The list states the exact cached base, head and source repository context that produced it. A changed range supersedes the old membership atomically; pages from different ranges or authorization epochs are never spliced together.

When a saved commit exists as a commit object in an explicitly linked registered clone, the user can open that exact OID in the existing local Git history/diff view. This read-only navigation does not fetch, check out, add or change remotes, infer provider membership from local Git, or accept a renderer-provided path.

## Public model

The generic detail-demand lifecycle gains a `commits` facet and provider profiles gain `pull_commits`, but renderer-facing rows are typed:

```text
PullCommitContext
  base_oid
  head_oid
  source_repository_provider_id
  metadata_facet_revision

PullCommit
  oid                    stable identity
  position               base-nearest zero-based order
  summary
  message                 known / omitted / oversized
  author                  bounded name plus optional provider presentation
  committer               same shape when the provider exposes this Git fact
  authored_at / committed_at
  parent_oids
  web_url                 optional validated provider URL

PullCommitCompleteness
  missing
  syncing
  complete
  capped(provider_limit | local_limit)
  partial

PullCommitSnapshot
  subject_id, context, commits, local next_cursor
  completeness, coverage, sync, freshness
  facet_revision, revision, authorization_view
```

The OID is identity. Position is range membership and ordering, never identity. A complete non-empty generation ends at the captured head OID. SHA-1 and SHA-256 OIDs are accepted only as canonical lowercase 40- or 64-hex strings. Empty is authoritative only after a successful exact-range terminal publication.

## Exact context and publication

Pull commits use a dedicated ordered cache while reusing account authorization, detail demand, scheduler admission, rate budgets, retry classification, revisions, subscriptions, retention and pins.

A traversal captures the current authorized pull-detail binding: account epoch and authorization view; canonical subject and target repository; provider pull identity; exact base/head OIDs; source/fork repository provider identity; and the body metadata facet revision that observed the range. Missing or conflicting known base/head metadata blocks commit synchronization and asks for pull-detail hydration. Summary-only fields cannot establish this context.

Provider pages are written to a bounded unpublished generation. Each page must match the captured context, run ID, expected provider cursor and prior page sequence. Duplicate OIDs or positions, overlapping/reordered pages, non-progressing or foreign cursors, source-strategy changes and changed range metadata are drift. The rejected generation is never readable. One drift may restart from page one; repeated drift ends with a provider error and preserves only an older generation that still matches the current exact context.

The terminal transaction validates authorization and pull context again, verifies positions and head membership, records completeness/cap evidence, then atomically swaps the active generation, facet revision, coverage and change record. A base, head, source repository or metadata-revision change hides the previous generation immediately while replacement sync runs. Local queries read only an active same-context generation.

The local page cursor binds account, authorization view, subject, active generation, facet revision, base/head and the last `(position, oid)`. Any publication or authorization change returns `stale_view`; clients restart rather than append incompatible pages.

## Provider boundary

Providers return typed commit pages through a dedicated adapter method. The engine supplies the exact captured range context and an opaque provider cursor. Each page reports normalized commits, provider order, continuation, cap evidence and rate-limit observations. Terminal publication still revalidates the parent pull detail; provider rows cannot change the captured context.

Initial adapters:

* GitHub uses `GET /repos/{owner}/{repo}/pulls/{number}/commits`, validates immutable repository/pull addressing and follows only checked pagination links. The documented endpoint exposes at most 250 commits, so a terminal result of 250 is published as provider-capped rather than guessed complete. See [GitHub REST pull requests](https://docs.github.com/en/rest/pulls/pulls?apiVersion=latest#list-commits-on-a-pull-request).
* GitLab uses `GET /projects/:id/merge_requests/:iid/commits`, binds the numeric target project and merge-request IID, validates returned commit identities and caps the normalized generation locally. See [GitLab merge request commits](https://docs.gitlab.com/api/merge_requests/#retrieve-merge-request-commits).
* Bitbucket Cloud uses the pull-request commit collection and follows its opaque `next` links only after transport origin and exact-route validation. Its Cloud commit representation exposes `author` and `date` without a distinct committer, so the normalized committer remains unknown rather than copying author data into a fact the provider did not return. See [Bitbucket Cloud REST](https://developer.atlassian.com/cloud/bitbucket/rest/).

A provider advertises `pull_commits` only when its adapter and contract fixtures are installed. Unknown fields remain additive. Unsupported providers return a typed unsupported capability instead of an empty list.

## Bounds

* At most 100 provider entries per page.
* At most 500 commits and 20 provider pages per local generation.
* At most 100 commits and approximately 1 MiB per IPC page.
* Commit message text is saved up to 65,536 bytes; larger messages are represented as `oversized` without partial text.
* At most 64 parent OIDs, 1,024-byte names, 2,048-byte validated web URLs and existing 4 MiB provider response bodies.
* Staging and active rows count toward RURU-104 retention accounting. Abandoned generations are reclaimable and can never become visible after restart.

A provider or local cap produces explicit capped/partial evidence and keeps `remote_has_more` truthful. The engine does not use a repository-wide commit walk as proof of pull-request membership.

## Access, offline and failures

An old account epoch, authorization view, parent metadata revision or link generation cannot publish or navigate. Authentication loss hides provider rows. Access-classified permission/not-found responses make the facet unavailable while policy-controlled retained bytes stay unreadable. Reauthentication must validate data under the new epoch before exposure.

Offline, transient and rate-limited failures may leave an already published same-context snapshot readable with its saved freshness/sync evidence. They cannot turn partial data into complete data, clear a previously complete generation, or expose unpublished rows.

## Local Git navigation

A new generated Tauri command accepts opaque account/subject/commit membership and local-link identities plus the captured authorization epoch. Native code resolves the current active commit row, revalidates the authored link and repository registration, obtains the native path internally, and runs `git cat-file -t <oid>`. Only the exact `commit` type returns a receipt containing the local repository ID and OID.

The renderer uses that receipt to activate `/app/git`, select the History view and set `selectedHistoryCommitHash`. Missing objects, blobs/tags, stale links and authorization changes stay in the commit panel with bounded recovery copy. No provider request or implicit fetch occurs.

## Test gates

Required focused evidence includes:

* canonical base-to-head ordering and stable local pagination;
* duplicate, overlap, reorder, cursor-loop and foreign-continuation rejection;
* missing, partial, complete-empty, complete and capped states;
* base/head/source/metadata-revision changes before, during and after paging;
* crash after an intermediate page, restart cleanup and atomic terminal publication;
* cache reopen, retention accounting, eviction and pinned preservation;
* GitHub 250-cap, GitLab bounded response and Bitbucket opaque continuation fixtures;
* unknown fields, malformed identities/URLs, oversized text and rate limits;
* old-epoch completion, authentication loss, access denial and reauthentication;
* offline same-context render and stale client-cursor rejection;
* linked-clone available/missing/blob/tag/stale-link/cross-account cases in SHA-1 and SHA-256 repositories;
* keyboard-accessible UI states and packaged restart coverage.

## Implemented behavior and local evidence

Migration 0011 and the pull-commit store now publish bounded staging generations atomically against the exact authorized Body base, head, source repository and metadata revision. GitHub, GitLab and Bitbucket Cloud adapters normalize their native commit collections into the common model while preserving provider ordering evidence, truthful caps, validated continuation authority and provider-specific missing fields. Contextual capability evidence is derived from the same active generation and commit scope, so missing, syncing, empty, complete, partial, capped, offline and denied remain distinct.

The runtime durably retains the original commit demand when Body context is missing or changes, admits Body hydration through the shared scheduler, and retries commits only after a valid context exists. Renderer queries remain local-only. The panel resets incompatible repository or Body query state immediately and uses the generated native navigation receipt to open only an exact cached member present as a local commit object in the linked target or source clone. Git reads set `GIT_NO_LAZY_FETCH=1`, disable replacement objects and external diff execution, and do not hydrate promisor objects.

Final serialized `make verify` passes 670 frontend/SDK/UI tests with one platform skip, lint, type checks, the production desktop build, Rust formatting, workspace Clippy and every default Rust suite. Focused provider, runtime, storage, navigation, client and UI regressions cover paging drift, authorization/context changes, caps, retention, SHA-1/SHA-256, missing/blob/tag objects and promisor repositories. Normal `make typegen` succeeds with 123 generated commands.

The feature-gated harness passes 20 native tests, its full collaboration suites and warning-denied Clippy. A real release-mode packaged five-process run passes the ordinary scenarios, both hard-crash checkpoints and both independent restarts. The crash-after phase publishes the exact two-row generation; the new process reads the same account, subject, base, head, source repository, Body metadata revision, order and complete/no-cap evidence before acquiring fresh interest, with provider and vault counters remaining `0 -> 0`. Local artifact: `artifacts/e2e-harness/2026-10-07T17-58-07-291Z-32420`.

An independent final source review found no remaining release blocker. This is local macOS and fixture evidence. Exact-head remote CI, live private-provider/PAT/keyring behavior and other-platform packaged execution remain separate gates.

## Ownership and delivery evidence

This worktree owns the typed commit models/store/runtime/provider modules, the narrow local navigation command, generated IPC/client bindings, the PR detail commit panel and focused tests, this work note, and concise root progress records. It does not implement checks, changed files, reviews, remote writes, merge, provider webhooks, cloning or implicit fetch.

The signed pre-code contract remains the authority for scope. Draft publication must retain this issue's stack base and attach the resulting PR before Linear moves to review. No credential was inspected, no Gitru cloud account was required, and no merge is authorized.
