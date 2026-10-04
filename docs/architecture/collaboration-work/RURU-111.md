# RURU-111: GitLab.com cached MR and issue reads

## Contract saved before implementation, 4 October 2026

Base: published R101/#156 `63e11970838c3bca1710f16175886f117e95aabb`, including
R110/#155 account/repository reads and R97/R100 detail/capability contracts.
Live Linear R111 is Backlog; prerequisites are implemented In Review, not Done.
No existing R111 PR/worktree was found. R103 qualification is a separate branch;
this slice does not rely on unqualified harness changes. Read the engine design
and backlog before editing. Accounts remain independent of Gitru cloud.

Implement GitLab.com selected-project MR/Issue summaries and independently
cached singleton Body/common metadata through existing provider traits, Rust
scheduler/storage, generated IPC, SDK and ordinary collaboration UI. Preserve
immutable global native IDs separately from project IIDs and numeric project
routes. Scope-validate bounded provider pagination links. Start unconditional
full traversals with issue keyset ID ordering and MR offset stable creation
ordering; preserve existing qualified membership reconciliation and restart
semantics. Treat incomplete traversals as incomplete, never full absence.

Singletons validate id/iid/project identity and distinguish omitted/null/empty/
oversized descriptions. Keep missing MR branch OIDs optional; async diff_refs
must never produce invented values. Body uses endpoint-specific authority and a
qualified singleton representation clock; list clocks cannot validate Body.
Without a verified endpoint ETag guarantee, do not add conditional requests.
Issue moves create distinct copied identities; private drafts never follow a
locator alias. Concealed 404/access loss and epoch fencing preserve cached rows
and private drafts. Token handling uses current manual PAT and observed rate
headers/backoff, with no personal credential inspection or live provider calls.

Own adapter `gitlab.rs`, `gitlab/transport.rs`, new feeds/resource_details modules
and corresponding tests/fixtures. Prefer no migration, IPC signature or ordinary
UI provider branch. Escalate necessary shared changes with concrete evidence;
root owns architecture progress, generated IPC and PR integration. Discussion,
approval/check/todo facets, mutations, enterprise and relay remain later issues.

## Required evidence

Divergent fixtures: large IDs; repeated IIDs across projects; rename/transfer;
copied issue moves; MR states/drafts; missing branch/description fields; malformed,
repeated and cross-scope paging; partial restart; 401/403/404/429/503; obsolete
responses after epoch/access/head changes; two-account isolation; cold SQLite
reopen and ordinary capability/query rendering. Use actual adapter/runtime paths,
not mock tests mirroring normalization. Run focused core tests, workspace Clippy/
format and relevant SDK/UI checks. Regenerate signatures with make typegen only
when changed. Record local validation separately from remote CI and live checks.

All Cargo/build/generation commands use the shared serialized validation wrapper
`/tmp/gitru-cargo-serial.py`; no direct parallel Cargo. Create signed scoped commits
and a reviewable PR based on #156; attach it. Do not merge.

## Primary research

- https://docs.gitlab.com/api/rest/ (global ID vs IID; issue keyset support)
- https://docs.gitlab.com/api/merge_requests/ (MR lists/singletons/diff references)
- https://docs.gitlab.com/api/issues/ (project issue lists/singletons)
- https://docs.gitlab.com/user/project/issues/managing_issues/ (close-and-copy moves)
- https://docs.gitlab.com/security/tokens/access_token_scopes/ (read_api)
- https://docs.gitlab.com/api/rest/authentication/ (PAT header/auth responses)
- https://docs.gitlab.com/api/rest/troubleshooting/ (concealed denial)
- https://docs.gitlab.com/user/gitlab_com/rate_limits/ (observed budgets)
