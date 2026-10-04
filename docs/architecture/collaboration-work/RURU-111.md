# RURU-111: GitLab.com cached MR and issue reads

## Contract saved before implementation, 4 October 2026

Base: published R101/#156 `63e11970838c3bca1710f16175886f117e95aabb`, including
R110/#155 account/repository reads and R97/R100 detail/capability contracts.
Live Linear R111 was Backlog at selection and is now In Progress; prerequisites
are implemented In Review, not Done.
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


## Implemented source and native validation — 4 October 2026

The GitLab.com adapter now declares selected-project PullRequests/Issues and
PullDetails/IssueDetails support alongside repositories. It uses all-state full
MR offset sweeps ordered by creation time and issue ID keyset sweeps. Native-only
versioned cursors bind account, epoch, project and kind, validate exact finite
routes/filter sets and preserve monotonic issue IDs or bounded MR equal-clock
identity boundaries. MR offsets advance one page; replayed boundary identities
and pathological oversized ties fail partial. This is not an atomic remote
snapshot. Existing two-completed-sweep membership reconciliation remains native;
no absence/deletion claim follows incomplete pages.

Global native IDs remain distinct from project IIDs, numeric routes select the
project, and renamed/transferred display URLs do not change identity. Summary
description clocks never establish independently cached Body authority. Singleton
Body/common metadata validates immutable IDs/project/IID and preserves omitted,
known-null, empty and oversized values. Missing async MR diff references remain
Omitted. Body is Uncertain/SubjectHistory with the qualified singleton clock;
endpoint-specific metadata authority and R101 fences stay intact. No ETag guarantee
is assumed: reads are unconditional, unexpected 304 is invalid. Ordinary account
help is updated to match supported reads; inbox/todos, discussion/approval/check
facets, remote mutations and self-hosted GitLab remain unsupported.

Bounds: at most 50 rows/page, 16 KiB summary description, 1 MiB Body, 4 MiB HTTP
response, 256 KiB aggregate typed metadata, 2 KiB URL, 4 KiB persisted cursor and
100 identities in a creation-time tie boundary. Token redirects cannot expand
host/path/filter authority; observed waits and response error classes use the
existing quota/retry policy. No migration or IPC/domain signature changes occur.

Fourteen actual HTTP adapter cases plus four real adapter → Runtime → SQLite
cases qualify paging/identity/states/missingness, cold offline reopen with inert
reads, actual late old-epoch response/quota rejection versus replacement, same
login across actors, concealed 404 facet isolation and cross-project closed-original
plus copied-issue identity. The copy has the same IID/title/body but a distinct
global ID; private draft generations and independently hydrated Body/metadata/facet
evidence remain separate after cold reopen. Initial failures were incorrect test
query/installation seams and a receipt captured before a legitimate draft write;
the corrected assertions retain exact native evidence without production changes.

Full collaboration validation: 329 passed, two explicitly ignored, including all
12 integration suites and three frozen migration tests. Workspace all-target
Clippy with -D warnings, formatting and scoped diff checks pass. Logs:
/tmp/gitru-ruru111-full-tests.log, /tmp/gitru-ruru111-workspace-clippy.log and
/tmp/gitru-ruru111-fmt.log. Ordinary supported GitLab MR/issue component cases pass
alongside historical unsupported-capability controls (14/14 focused tests). The
final complete frontend suite passes 413 tests across 48 files; repository lint,
TypeScript including desktop/E2E/testing, and production desktop frontend build
pass. Shared SavedItemDetail renders Body/common metadata and preserves private
drafts with one ephemeral detail interest and no durable hydration request.
Logs: /tmp/gitru-ruru111-gitlab-ui-tests.log and
/tmp/gitru-ruru111-final-{frontend-tests,lint,types,build}.log. All tokens,
HTTP servers and database actors used for validation are synthetic; no personal
credentials, live GitLab API, production vault or packaged-platform claim is made.


Final independent review found an async diff-reference missingness edge. Base now
requires both the MR SHA and diff_refs.head_sha to be known, nonempty and equal
before accepting start_sha. Missing, null, empty and mismatched heads leave Base
Omitted while independently qualified Body/title/state remain available. Eight
actual HTTP shapes preserve the known matching-head target-start control. After
this correction the full collaboration suite still passes329/two ignored,
focused HTTP14/14, workspace all-target Clippy-Dwarnings and formatting pass.
Logs: /tmp/gitru-ruru111-async-head-{http,full,clippy,fmt-check}.log.
