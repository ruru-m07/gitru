# RURU-128 — GitLab discussions and approval observations

Status: pre-code work contract, 8 October 2026. No feature implementation or qualification is claimed yet.

[Linear RURU-128](https://linear.app/catra/issue/RURU-128/add-gitlab-discussion-and-approval-detail-facets) adds read-only GitLab.com collaboration facets through the existing native provider, SQLite detail projections, scheduler and cache-only UI. Initial isolated managed worktree starts at signed R127 `cfc5b2345f9ab2ac2ac813e4e05c497a679a8fb7` on `/Volumes/Lexar`. R123 shared review/thread models are being implemented in a separate worktree; R118 checks is a sibling, not an ancestor of this starting tree. Their exact signed implementations must be integrated before shared adapter/UI work and final qualification. No parallel review model or competing storage is authorized.

## Source contract and scope

Official sources inspected on 8 October 2026:

- [GitLab discussions API](https://docs.gitlab.com/api/discussions/): merge-request discussions have a stable discussion ID and nested notes; individual notes, discussion notes, diff notes and system notes are distinct. Notes carry independent optional resolution evidence and provider-native position data.
- [GitLab merge request approvals API](https://docs.gitlab.com/api/merge_request_approvals/): `/approvals` returns current approvers and aggregate provider facts; it does not supply a per-approval commit SHA. Rule details from `/approval_state` require Premium/Ultimate and are a distinct representation.
- [GitLab merge request pipelines](https://docs.gitlab.com/api/merge_requests/#list-merge-request-pipelines): an MR pipeline may run a detached head or a merged-result SHA. Its success cannot be silently substituted for a current source-head check.
- [GitLab REST pagination](https://docs.gitlab.com/api/rest/#pagination): offset traversal is mutable, bounded, and resumable; an incomplete traversal cannot establish absence.

The intended slice maps GitLab MR discussion and approval observations to the frozen R123 provider-independent review/thread contracts. General and diff discussions retain native note type, system flag, resolution and position evidence; no inferred resolved, outdated or ready-to-merge state. Approval observation context is separate from an actual approval commit anchor: an absent provider commit anchor remains unknown even when the containing MR was freshly read. Aggregate approval counts/booleans are provider facts, never merge authorization. Unsupported plan-dependent rule details stay explicit; no required-policy completeness is implied.

The existing R118 GitLab exact-head commit-status adapter and common checks UI are reused and qualified alongside this slice. Separate MR merged-result pipeline or required-policy inference is outside this contract; native statuses remain bound to the exact captured SHA and unknown/partial/empty observations cannot imply readiness.

## Native bounds and authority

Rust owns HTTP, tokens, route construction, normalization, scheduling, SQLite and invalidation. Requests use native numeric project ID, MR IID, immutable subject ID, account/instance/authorization epoch and the saved Body context. Renderer URLs are never provider request authority. Existing same-origin route validation, redirect policy, 4 MiB response bound, operation timeout, quota persistence and retry behavior remain mandatory.

Discussions use bounded offset pages, bounded nested note count/body/path/position fields, and account/epoch/context-bound cursors. Local collection caps are visible partial coverage. Unknown future note kinds remain visible bounded native evidence. A malformed note or wrong project/MR identity cannot silently authorize another resource. Provider response ordering, totals, individual-note flags and absent resolution fields do not establish stronger semantics than their documented meaning.

Approval observations preserve immutable resource identity and are fenced by current saved Body range and source generation. A provider approval has no invented commit SHA; actual anchoring remains unknown. Where additional current-parent validation is required, each provider round consumes the native account budget and a successful exhausted response prevents the next round. Authentication, permission, plan/version absence, rate limiting, offline and malformed responses remain typed; they cannot become empty successful observations.

Shared local readers and subscriptions return immediately from SQLite and retain explicit fresh/stale/partial/unavailable evidence across restart. New facets must use the same current authorization, membership, context and source-generation fences at admission and publication. No token, personal keyring, production database, provider mutation or Gitru cloud dependency is used for qualification.

## Implementation sequence and ownership

1. Sign this contract, record live blockers and agree R123 model seam.
2. Add bounded GitLab wire normalization and secure native routes in this isolated worktree; no shared model changes until R123 freezes.
3. Integrate exact signed R123/R118 prerequisites, then map GitLab data through the shared runtime, cache and panels. Preserve the R127 todo grants and native identities.
4. Add finite provider HTTP, storage/runtime restart/context, and shared UI tests with divergent native fields and missing/unknown evidence.
5. Run normal `make typegen`, focused checks and full `make verify`; publish a signed reviewable draft, attach it and update Linear plus architecture/backlog with exact evidence. Remote CI and live provider/platform execution remain separate gates.

All feature edits belong only to `/Volumes/Lexar/.codex/wt/ruru-128-gitlab-details/gitru`. R123 owns its shared-model source in its own worktree; this branch consumes signed checkpoints. R115/R116 command delivery/optimistic state, recovery, new provider writes, enterprise hosts, and optional relay work remain outside this slice. No merge to a shared destination branch is authorized.
