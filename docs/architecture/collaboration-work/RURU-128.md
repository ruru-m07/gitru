# RURU-128 — GitLab discussions and approval observations

Status: clean scoped source replay complete, 8 October 2026; final-tree qualification and draft publication are in progress. The original pre-code contract was signed as `fb1b5e1`.

[Linear RURU-128](https://linear.app/catra/issue/RURU-128/add-gitlab-discussion-and-approval-detail-facets) adds read-only GitLab.com collaboration facets through the existing native provider, SQLite detail projections, scheduler and cache-only UI. The isolated managed worktree on `/Volumes/Lexar` was initially based on R127, but final scoped history replays only the R128 commits atop clean signed R123 `5a30303`. Its explicit integration base combines the existing R126, R118 and finalized R116 prerequisites through schema 0018. R127 to-do inbox changes are not inherited or required. R128 introduces no migration or parallel review storage.

## Source contract and scope

Official sources inspected on 8 October 2026:

- [GitLab discussions API](https://docs.gitlab.com/api/discussions/): merge-request discussions have a stable discussion ID and nested notes; individual notes, discussion notes, diff notes and system notes are distinct. Notes carry independent optional resolution evidence and provider-native position data.
- [GitLab merge request approvals API](https://docs.gitlab.com/api/merge_request_approvals/): `/approvals` returns current approvers and aggregate provider facts; it does not supply a per-approval commit SHA. Rule details from `/approval_state` require Premium/Ultimate and are a distinct representation.
- [GitLab merge request pipelines](https://docs.gitlab.com/api/merge_requests/#list-merge-request-pipelines): an MR pipeline may run a detached head or a merged-result SHA. Its success cannot be silently substituted for a current source-head check.
- [GitLab REST pagination](https://docs.gitlab.com/api/rest/#pagination): offset traversal is mutable, bounded, and resumable; an incomplete traversal cannot establish absence.

This slice maps GitLab MR discussion and approval observations to the frozen R123 provider-independent review/thread contracts. General and diff discussions retain native note type, system flag, resolution and position evidence; no inferred resolved, outdated or ready-to-merge state. Approval observation context is separate from an actual approval commit anchor: an absent provider commit anchor remains unknown even when the containing MR was freshly read. The adapter validates the known aggregate approval scalar shapes but does not project them as policy: only individual approver observations are retained. Neither these rows nor an empty approver list establish merge authorization. Unsupported plan-dependent rule details stay explicit; no required-policy completeness is implied.

The existing R118 GitLab exact-head commit-status adapter and common checks UI are reused and qualified alongside this slice. Separate MR merged-result pipeline or required-policy inference is outside this contract; native statuses remain bound to the exact captured SHA and unknown/partial/empty observations cannot imply readiness.

## Native bounds and authority

Rust owns HTTP, tokens, route construction, normalization, scheduling, SQLite and invalidation. Requests use native numeric project ID, MR IID, immutable subject ID, account/instance/authorization epoch and the saved Body context. Renderer URLs are never provider request authority. Existing same-origin route validation, redirect policy, 4 MiB response bound, operation timeout, quota persistence and retry behavior remain mandatory.

Discussions use at most 20 offset pages of 50 source discussions, with at most 50 retained notes per page and per discussion. Approvals retain at most 50 approvers. The common review writer remains bounded to 50 entries per source page. Bodies above 65,536 UTF-8 bytes become explicit oversized omissions; paths, identifiers, dates and native positions are validated and bounded. Cursors are at most 4,096 bytes and bind the account, immutable actor, authorization epoch, repository, subject, exact saved review context and monotonically increasing page. Repeated, mismatched or impossible cursors fail before HTTP. Local collection caps and any nested-note omission are visible partial coverage. Only a complete, untruncated single source page grants full enumeration; resumed/multipage traversal remains partial and cannot remove rows by absence. Individual/root/reply identity is never inferred from array order. Native GitLab base/start/head SHAs, resolution evidence, line ranges and finite image coordinates remain separate from GitHub-style common anchors. Unknown future note kinds remain visible bounded native evidence. A malformed note or wrong project/MR identity cannot silently authorize another resource. Provider response ordering, totals, individual-note flags and absent resolution fields do not establish stronger semantics than their documented meaning.

Approval observations preserve immutable resource identity and are fenced by current saved Body range and source generation. A provider approval has no invented commit SHA; actual anchoring remains unknown. Where additional current-parent validation is required, each provider round consumes the native account budget and a successful exhausted response prevents the next round. Authentication, permission, plan/version absence, rate limiting, offline and malformed responses remain typed; they cannot become empty successful observations.

Shared local readers and subscriptions return immediately from SQLite and retain explicit fresh/stale/partial/unavailable evidence across restart. New facets must use the same current authorization, membership, context and source-generation fences at admission and publication. No token, personal keyring, production database, provider mutation or Gitru cloud dependency is used for qualification.

## Implementation sequence and ownership

1. Sign this contract, record live blockers and agree R123 model seam.
2. Add bounded GitLab wire normalization and secure native routes in this isolated worktree; no shared model changes until R123 freezes.
3. Integrate exact signed R123/R118 prerequisites, then map GitLab data through the shared runtime, cache and panels. Preserve exact selected-repository membership and native identities.
4. Add finite provider HTTP, storage/runtime restart/context, and shared UI tests with divergent native fields and missing/unknown evidence.
5. Run normal `make typegen`, focused checks and full `make verify`; publish a signed reviewable draft, attach it and update Linear plus architecture/backlog with exact evidence. Remote CI and live provider/platform execution remain separate gates.

All feature edits belong only to `/Volumes/Lexar/.codex/wt/ruru-128-gitlab-details/gitru`. R123 owns its shared-model source in its own worktree; this branch consumes signed checkpoints. R115/R116 command delivery/optimistic state, recovery, new provider writes, enterprise hosts, and optional relay work remain outside this slice. No merge to a shared destination branch is authorized.

## Qualification checkpoint

- Native review-focused checks pass, including finite actual HTTP parsing/routes, quota preservation, caps, missing approval commit anchors, typed native DTO validation, and exact Body → review fetch → SQLite → fully closed offline reopen.
- Held GitLab responses cannot cross a summary head change or a credential epoch cutover. Same-epoch rejected data still records consumed quota; an old epoch cannot mutate the new account budget.
- A second native runtime fixture publishes 50 retained notes from a 51-note discussion as partial. Invalid native counts, OIDs and dates reject atomically without replacing the saved notes.
- `make typegen` succeeds with 137 commands; the generated schema inventory is 431 and includes the boxed provider-tagged native position graph. SDK tests pass 187/187; the independent GitLab note display tests pass 3/3.
- GitLab regression tests pass 106/106 before the final two added finite collection cases; the final focused adapter suite passes 17/17, native runtime cases pass 3/3, and integrated common/GitLab UI checks pass 6/6. The preliminary full crate run found inherited R123 schema-18 recovery and legacy capability/lifecycle fixture failures; signed canonical prerequisite fixes are now integrated.
- Full `make verify` passes at signed source `440f6fe`: 766 frontend tests / one platform skip; 1,251 Rust reported passes / seven standalone helper ignores; lint, types, desktop build, formatting and strict workspace Clippy. This exact checkpoint is preserved on `ruru/ruru-128-pre-restack-440f6fe`; its log is `/tmp/gitru-r128-pre-restack-verify.log`.
- Clean replay is complete atop signed R123 `5a30303`, with no R127 or old R128 ancestry. The prior implementation is preserved on `ruru/ruru-128-pre-restack`. Only the selected-repository read slice is inherited; the synthetic fixture no longer depends on the unrelated native inbox extension. Bindings are regenerated (137 commands / 431 schemas). Final-tree `make verify` is required before draft publication; no PR has been opened yet. These are local synthetic checks; remote CI, live GitLab accounts, provider mutation, personal credentials and GUI/platform execution remain unclaimed.
