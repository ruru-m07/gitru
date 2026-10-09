# RURU-53 remaining-work handoff

## Current continuation — 8 October 2026, 08:42 UTC

This section supersedes the historical handoff below. A live Linear refresh
returns all 49 direct children: **42 In Review, 3 In Progress, 1 Todo and
3 conditional Backlog**. The three In Progress issues are R102, R107 and R134;
R75 remains Todo. R108 additionally has R139 and R140 In Progress, and R141 in
Backlog. No issue or implementation PR was marked merged or shipped.

The next actionable work is:

- **R134:** integrated label, assignee and milestone drafts/catalogs/delivery are
  locally qualified. The frontend suite passes 932 tests/one existing skip;
  schema25 recovery and the historical 1–24 migration matrix pass. The final
  collaboration library run passes 909/4 ignored after repairing a terminal
  catalog deadline loop and a prerequisite guarded-merge read-only recheck.
  Publish on the review dependency composite combining PR203 reviews and PR201
  creation budgets. Preserve each field's independent metadata outcome; a
  confirmed issue does not imply every requested field was applied.
- **R102:** account fairness is published in PR191. Draft PR205 at signed
  `4bb86b98` repairs truncated quota diagnostics and qualifies cold backward/
  forward clock and replacement behavior with synthetic clocks. The full native
  run passes 1,025/5 ignored and strict Clippy passes; exact-head CI is running.
  Current
  GitHub routes all use REST core, so keep the conservative shared provider
  floor rather than invent unsupported family classifications.
- **R107:** PR192 supplies synthetic cache/vault-failure qualification. PR204 at
  `e576fef2` now passes actual native adapter seed/cold-reopen/cleanup on Linux,
  macOS and Windows hosted runners; Linux additionally passes an unavailable
  private D-Bus control. The remaining aggregate CI is pending. Interactive
  locked/user-denied prompts and R75 live PAT/CLI remain separate evidence.
- **R139/R140:** the verified SQLCipher build, create-only macOS key vault and
  owned keyed connection factory are published in PR193/194/200/202. PR200's
  exact `ad9a46c1` has all 15 checks green. PR193 `ac14f582` carries a diagnosed
  Clippy cold-build allowance; PR202 `8d8073f5` fixes a fixture's missing native
  worker-release acknowledgement and inherits that allowance. Their new-head
  CI is pending. Production Store remains plaintext. Next: current-schema keyed
  Store ownership/identity, remaining platform key stores, then recoverable
  conversion and activation. Do not bypass the WAL gate or key-before-SQL rule.
- **R141:** portable encrypted backup/key wrapping and artifact exposure remain
  dependent on qualified keyed Store/recovery. The design must not assume the
  original device vault travels with an export.

R120 PR creation, R132 review submission, R133 guarded merge, R129 labels and
R122 GitLab/Bitbucket activity now have reviewable implementation PRs. The older
missing-feature descriptions below are historical, not the next work queue.
Provider parity, live write compatibility and interactive platform gates are
not inferred from fixtures or open PRs. R75 still needs a dedicated test account;
personal credentials remain outside unattended validation. R109/R113/R135 stay
explicitly deferred. All active checkouts and heavy build targets are on Lexar.

The dependency composite is a review base, not a merge into dev. Local checks,
remote results at exact source heads, and live provider/platform checks remain
separate. Continue actual failures before duplicate PRs. See the current issue
worknotes for command-level evidence and the architecture progress appendices.

## Historical handoff — 8 October 2026, 01:55 UTC

Read live Linear children, individual blocker relations and recent progress comments; checked open GitHub PRs and current implementation worknotes. The following original remaining-work record and exact-head PR ledger are retained as historical evidence. They do not describe the current implementation queue.

## What the status means

The original [RURU-53](https://linear.app/catra/issue/RURU-53) has **49 direct children: 37 In Review, 4 In Progress, 7 Backlog, 1 Todo, 0 Done**. The original issue descriptions/status tables contain dated October 2–5 snapshots; newer comments and signed worknotes supersede their implementation claims.

| Live status | Original direct children |
| --- | --- |
| In Review (37) | R76, R77, R78, R79, R95, R96, R97, R98, R99, R100, R101, R103, R104, R105, R106, R108, R110, R111, R112, R114, R115, R116, R117, R118, R119, R121, R123, R124, R125, R126, R127, R128, R130, R131, R136, R137, R138 |
| In Progress (4) | R102 scheduler families/lifecycle; R122 comments/activity/provider expansion; R129 desired-state edits; R134 issue creation |
| Todo (1) | R75 production PAT/explicit CLI/vault verification |
| Backlog (7) | R107 platform qualification; R120 PR creation; R132 review submission; R133 guarded merge; R109 self-hosted instances; R113 Bitbucket Data Center; R135 optional relay assessment |

The collaboration PRs returned by the live open-PR query are **open drafts**, including the foundation and all published descendants through #189. “In Review” means a reviewable bounded implementation, not merged, shipped, all-provider parity or fulfilled live-platform qualification. No PR merge was performed. Append the publication owner's exact final head/check ledger separately; a previously green parent does not qualify its changed descendant.

## Executable implementation still outstanding

1. **R102 — provider API-family scheduling and remaining lifecycle behavior.** The original discovery and >24-hour clock defects are now repaired; do not reopen them as missing implementation. Existing #165's latest Linear evidence records full accepted monotonic waits, cold direct-admission seeding and all 11 reported checks green at `c0a9f2f`. Remaining work is the broader family-aware budget/fair-service model and its deterministic cross-account/foreground/background saturation controls. Incorrect-wall-clock restart behavior needs an explicit tested policy; real OS suspend/resume is separate platform evidence. Prerequisite implementations R76/R98 exist in the stack.

2. **R129 — labels, then additional supported provider edit codecs.** GitHub issue/PR title/body (#181) and separate close/reopen State intent (#185) are implemented. Labels are not: the scalar title/body/state intent does not establish label-set identity, concurrent add/remove semantics, provider permissions, canonical set evidence or effective-query behavior. Next bounded slice: typed GitHub label intent with native label identities, operation-specific comparison/reconciliation and atomic effective/canonical publication; retain explicit best-effort disclosure where the endpoint has no remote CAS. Follow with GitLab/Bitbucket operations only where their capabilities and proof semantics are implemented. Shared prerequisites are R77/R78 and durable R115→R116→R117, already present as unmerged code dependencies.

3. **R134 — creation metadata after the title/body slice.** The local native/UI checkpoint now implements GitHub title/body issue drafts, generation CAS, strict 201 proof, immutable draft→canonical identity, and atomic feed/Body/FTS visibility. The title/body slice is published in PR #187; full workspace qualification at `e7db1639` passes 813 frontend/one skip and 1,340 Rust/seven helper ignores. The separate feature harness passes 21/21. Its latest publication head also incorporates the platform CI timeout repair; current-head remote CI is separate. Labels, assignees, milestones, issue types and custom fields remain outside that slice, so the metadata acceptance criterion is open. Add one supported metadata family at a time, preserve it in drafts and validate exact returned/read-back evidence: a successful create can silently omit optional GitHub metadata, so HTTP 201 alone cannot confirm those fields. Other providers require their own creation policy. Existing prerequisites R78/R99/R115/R116/R117 are implemented in the stack.

4. **R120 — online PR creation is not supplied by issue creation or checkout.** Start with one provider and a durable PR draft; capture explicit local clone/account/repository mapping, source/target refs and fresh native heads, then an operation-specific create receipt→canonical PR transition. No implicit push, branch publication or checkout. Test branch/head drift, fork ownership, permissions, lost response and retained unknown outcomes. Depends on R77/R96/R99/R115/R116/R117. R96 links and R136 checkout are useful existing components, not evidence that PR creation exists.

5. **R132 — review submission is distinct from ordinary comments.** Cache reads of reviews/threads and changed files do not implement approval/request-changes or line review submission. Begin with a bounded review operation carrying inspected head/base and durable text; add line/path/side/provider-position anchors under explicit supported capability. Test force-push refusal/remapping, stale approvals, permissions, accepted/unknown receipts and no duplicate creation. Dependencies: R99/R115/R117/R119/R123. A conversation-comment receipt cannot prove a review was submitted.

6. **R133 — guarded online merge remains unimplemented.** Start only with a provider endpoint whose documented expected-head/version guard can be exercised. Capture fresh permissions and current head, show checks/review coverage without converting a partial cache into authorization, and persist attempt/receipt/reconciliation. A 202/auto-merge acceptance is not a completed merge. Test head change, conflict, permission loss and ambiguous result/restart. Dependencies: R77/R115/R117/R118/R123. Close/reopen does not implement merge; this is product work, not authorization to merge these implementation PRs.

7. **R122 — complete the current provider-read expansion, then explicit missing activity semantics.** GitHub comments (#164) and independent GitHub Activity (#184) are published. Bitbucket Cloud PR top-level conversation comments are published in PR #188: product `d114202c` passes full workspace verification (804 frontend/one skip, 1,330 Rust/seven helper ignores). GitLab issue/MR ordinary notes are published in PR #189: product `ea97e2a3` passes full workspace verification (802 frontend/one skip, 1,327 Rust/seven helper ignores). The initial full run identified an obsolete Comments-unsupported fixture, corrected to retain cold/no-network/draft checks while asserting supported, unloaded read-only comments. Anonymous project and MR-list reads returned 200; the selected Notes read returned 401, so live Notes permissions/payload compatibility remains unqualified. Their scope deliberately excludes system/inline/resolvable or reply/thread semantics that cannot be represented faithfully. They do not implement GitLab/Bitbucket Activity timelines. Keep unsupported facets visible and use separate typed adapters for any further supported activity, rather than flattening system events or review anchors into comments. No need to recreate the existing Comments storage/UI.

These chunks can proceed from the qualified dependency commits in isolated worktrees; open Linear blocker relations do not imply that the prerequisite implementation is absent. Keep ordered migrations and current backup/restore validation in every schema-bearing slice. Continue fixing actual CI failures in existing PRs before duplicating them.

## Encryption follow-ups are real GA implementation work

[R108](https://linear.app/catra/issue/RURU-108) is **In Review** in #186 for an architecture decision and bounded compatibility spike. It does **not** encrypt production storage. The selected GA target is whole-database encryption, including drafts/history/FTS, with a database key independent of provider tokens and Gitru cloud. Current SQLite and sanitized backups remain plaintext; filesystem permissions and the credential vault do not encrypt their contents.

The three new children are **Backlog**, additional to the original 49:

| Dependency order | Remaining deliverable |
| --- | --- |
| [R139](https://linear.app/catra/issue/RURU-139) | Qualify an auditable SQLCipher/native build that passes the existing SQLite WAL safety gate, packaged macOS/Windows/Linux linkage, FTS/migrations/backup and measured performance. The pinned spike reports SQLCipher 4.10.0 / SQLite 3.50.4, which fails that unchanged gate; enabling its feature is not a safe shortcut. |
| [R140](https://linear.app/catra/issue/RURU-140), blocked by R139 | Native random database-key lifecycle; key every connection; locked/missing/wrong-key behavior; recoverable plaintext→encrypted migration, rotation and low-disk/crash/rename controls. Preserve authored intent, restore quarantine and account independence. |
| [R141](https://linear.app/catra/issue/RURU-141), blocked by R139/R140 | Portable encrypted backup/key-wrapping, key-aware inspection/restore, explicit plaintext export and artifact exposure tests across DB/WAL/journals/temp/staging/rollback/diagnostics. A device-local vault key does not make a portable backup. |

R139 can be researched/qualified in parallel with feature work using synthetic data; R140 and R141 must consume its qualified build in order. Do not weaken the existing version gate or claim historical plaintext files have been erased.

## Live-provider, vault and platform evidence still separate

- **R75 (Todo):** packaged manual PAT and explicitly chosen GitHub CLI import with an isolated test account and the real OS vault; same actor, offline restart, replacement, revocation and disconnect. Automated fake credentials and a public anonymous GET do not satisfy this. No personal credentials were inspected for unattended validation.
- **R107 (Backlog):** collaboration-specific Windows/Linux/macOS storage/runtime/migration/offline/vault-failure matrix, building on R95/R105/R103 and coordinated with R92. Existing remote Rust, packaged E2E and retained fake-provider harness results are useful recorded evidence; they do not establish each production vault's live behavior.
- **Current write codecs:** authenticated compatibility of the fixed numeric GitHub mutation aliases, actual provider write permissions/errors and packaged user flows remain unqualified unless the final ledger records a specific exercise. Native fixture validation does not supply that evidence. No mutable-path fallback or blind ambiguous retry should be added to make a test pass.
- **Performance:** R125 has a real packaged 10k-summary macOS baseline (warm React p95 ≤59 ms, SDK/IPC p95 ≤3 ms, useful cold content 142 ms after runtime readiness). It does not establish combined renderer/native memory, the architecture's 100k-summary/500k-child envelope, other device/storage classes, encrypted overhead or live-provider performance. Those are measured extensions, not reasons to erase the completed baseline.

## Explicitly deferred conditional scope

**R109 self-hosted instances**, **R113 Bitbucket Data Center** and **R135 optional webhook relay** remain Backlog by the accepted public-provider rollout. R109 needs a selected host/version/trust matrix; R113 is a distinct server adapter assessment after R109/R112; R135 follows measured polling limits from R102/R125 and is a decision task, not an authorized cloud deployment. None is a prerequisite for normal desktop public-provider accounts or reads. Keep Gitru cloud sign-in optional and normal synchronization local/native.

## Evidence used and final pickup

Live sources: original R53 child listing (49, no next page); R108 child listing (139–141); individual current blockers; latest R102/R122/R129/R134/R125 comments; GitHub open-PR listing through #189. Repository sources: `remote-collaboration-engine.md` sections 1/3/18/23; `remote-collaboration-backlog.md` acceptance criteria (its old status tables are historical); current worknotes `RURU-102-clock-lifecycle.md`, `RURU-108.md`, `RURU-129.md`, `RURU-129-workflow.md`, `RURU-134.md`, `RURU-122-timeline.md`, `RURU-122-gitlab-comments.md`, `RURU-122-bitbucket-comments.md` in their external managed worktrees.

Immediate handoff: address actual PR failures as they arrive; preserve exact source/check evidence; then pick R102 family scheduling, R129 labels, or R139 build qualification as independent bounded lanes. Issue metadata and PR/review/merge delivery follow their existing contracts/dependencies. Refresh the live status counts and add the final CI/PR ledger at the deadline; do not report the whole R53 parent complete merely because many code slices are reviewable.


## Combined integration qualification

The external managed worktree `ruru-53-integration` combines issue creation,
workflow edits, provider inbox actions, Activity and both new comment providers.
The repair is published as [PR #190](https://github.com/ruru-m07/gitru/pull/190),
initial signed head `599c537afa3cf622d91c7fd0ca9c4c73183ff58f`, against
`ruru/ruru-53-integration-dependencies` at `38e43362`. That base combines published
feature branches solely for validation; retarget after those dependencies land.
This is a validation branch, not a merge into dev. Common UI disclosures and test
matrices are combined, and generated IPC is regenerated with make typegen.

Integration review found a real immutable-evidence compatibility defect: the new
optional inbox field changed serialization of RemoteItem values embedded in old
comment/workflow/issue-creation proof bytes. Signed fix `3db6b07a` freezes the operation-v1 item codec, preserving the original
field shape/ordering and rejecting new inbox/unknown fields without rewriting
stored evidence or changing public IPC. Three exact legacy-codec regressions and
13 real restore tests pass, including strict rejection of altered bytes and
preservation of both files when backup injection is refused. Independent review
cleared the repair. Final combined `make verify` passed at `e9e44812`: frontend
838 passed/one skip; native workspace 1,418 passed/seven helper ignores
(collaboration 995, Git 374, Tauri 34, IPC 15); lint/types/desktop build, rustfmt
and strict workspace all-target Clippy passed. A separate strict
`collaboration` all-target, all-features Clippy run also passed on the same
source. This is local qualification of the combined source. Remote CI and live
provider/vault/platform qualification remain separate; later documentation-only
commits do not change tested code.

The Windows platform job budget is 60 minutes after a diagnosed timeout in
job 113090183418: both Rust test steps passed, but post-cache upload reached the old
45-minute limit. Tests and retries were not weakened. New exact-head CI remains
required for the workflow change.

## Remote PR snapshot — 2026-10-08 02:00 UTC

This table records reported contexts at each exact head before the final handoff
documentation commit. Pending or cancelled contexts are not passes; local checks
and live-provider qualification are separate. All listed PRs are open and
unmerged. Refresh before review or merge decisions. The final Linear RURU-53
window-closing comment records any later CI changes and the final publication head.

| PR | Title | Exact head | Checks passed / reported | Other check states | Mergeability |
| --- | --- | --- | --- | --- | --- |
| [#141](https://github.com/ruru-m07/gitru/pull/141) | feat: add local-first collaboration foundation and GitHub PAT accounts | `c21538965fe6411044d0f49797ee74b6f8145748` | 15/15 | none | MERGEABLE |
| [#142](https://github.com/ruru-m07/gitru/pull/142) | fix(collaboration): recover credential cutovers after process crashes (RURU-95) | `4e8c903c37cb096a7a2af9d67332a7763695f78d` | 15/15 | none | MERGEABLE |
| [#143](https://github.com/ruru-m07/gitru/pull/143) | test(collaboration): prove schema upgrades and migration recovery (RURU-105) | `d8717f590373fa128c73c2a4fe33cef3ead08f2e` | 15/15 | none | MERGEABLE |
| [#144](https://github.com/ruru-m07/gitru/pull/144) | feat(collaboration): recover and export saved private drafts (RURU-99) | `2484bbeb5a392f7760d5ff537593128978ad4545` | 14/14 | none | MERGEABLE |
| [#145](https://github.com/ruru-m07/gitru/pull/145) | feat(collaboration): add verified desktop backup and recovery | `332e065a6fc7eae421361c0460c4cf17dc1b5939` | 14/14 | none | MERGEABLE |
| [#146](https://github.com/ruru-m07/gitru/pull/146) | feat(collaboration): add provider registry and canonical identities (RURU-76) | `862d9d748fbc88b243385d4d8190a7c37bcc1b74` | 15/15 | none | MERGEABLE |
| [#147](https://github.com/ruru-m07/gitru/pull/147) | feat(collaboration): cache independent detail facets (RURU-97) | `a23f9f126df09405173035886724c661d871c474` | 15/15 | none | MERGEABLE |
| [#149](https://github.com/ruru-m07/gitru/pull/149) | feat(collaboration): drive remote views from contextual capabilities (RURU-100) | `6799e6d738c2e2abccf7df53eea8be7f703395d8` | 15/15 | none | MERGEABLE |
| [#150](https://github.com/ruru-m07/gitru/pull/150) | feat(collaboration): cached pull request descriptions and metadata | `7e282adced13fdd1da99b937081fa76ff5f26c95` | 15/15 | none | MERGEABLE |
| [#151](https://github.com/ruru-m07/gitru/pull/151) | feat(collaboration): cached GitHub issue details | `863967f8dc55587c1f71061fd2267a1c139bed5e` | 15/15 | none | MERGEABLE |
| [#152](https://github.com/ruru-m07/gitru/pull/152) | feat(collaboration): prioritize visible work with native demand leases | `2d4159380928e2de0bc5551661b0933161287c8f` | 15/15 | none | MERGEABLE |
| [#153](https://github.com/ruru-m07/gitru/pull/153) | feat(collaboration): link local clones to cached provider repositories | `0eb71a5a39db2ae11a7af2a11d0cdd074443142e` | 15/15 | none | MERGEABLE |
| [#154](https://github.com/ruru-m07/gitru/pull/154) | feat(collaboration): resolve inbox subjects from local cache | `e6fe69eba00a598448cc8f29c1164326610ad20b` | 15/15 | none | MERGEABLE |
| [#155](https://github.com/ruru-m07/gitru/pull/155) | feat(collaboration): connect GitLab PAT accounts and cache member projects | `725b9f242c13b153d9da42374a0f9c32e31a8348` | 14/15 | PENDING: 1 | MERGEABLE |
| [#156](https://github.com/ruru-m07/gitru/pull/156) | fix(collaboration): reconcile facets with captured source and head evidence | `63e11970838c3bca1710f16175886f117e95aabb` | 11/11 | none | MERGEABLE |
| [#157](https://github.com/ruru-m07/gitru/pull/157) | test(collaboration): qualify native sync recovery on current engine | `c6eb7e6d678ee13e7a9873219fd1ee17a33bef77` | 9/14 | IN_PROGRESS: 2, QUEUED: 3 | MERGEABLE |
| [#158](https://github.com/ruru-m07/gitru/pull/158) | feat(collaboration): bound cached PR and issue navigation prefetch | `51249e4b0aeb78a04c5837466b1b3d6ce893af7a` | 10/11 | PENDING: 1 | MERGEABLE |
| [#159](https://github.com/ruru-m07/gitru/pull/159) | feat(collaboration): cache GitLab merge requests and issues | `ebb8ae10a2ae90cc3f0fa50176f2f88b64a756dc` | 11/11 | none | MERGEABLE |
| [#160](https://github.com/ruru-m07/gitru/pull/160) | feat(collaboration): add Bitbucket Cloud token accounts and repository sync | `968cd5240f2d96f970a6637bb4a8445e5a815d39` | 11/11 | none | MERGEABLE |
| [#161](https://github.com/ruru-m07/gitru/pull/161) | feat(collaboration): cache Bitbucket PR summaries and raw Body locally | `1766a6e7308375806aa637380ccf7caa2714669b` | 11/11 | none | MERGEABLE |
| [#162](https://github.com/ruru-m07/gitru/pull/162) | feat(collaboration): cache Bitbucket participants through shared local facets | `d2e91957e28de36cc3f7d0ecb2eb6a5d0ea15dcd` | 11/11 | none | MERGEABLE |
| [#163](https://github.com/ruru-m07/gitru/pull/163) | feat(collaboration): cache Bitbucket tasks through shared local facets | `2e4b8d5c57e5dd0bb7dd0cbef5e4f521a870e588` | 11/11 | none | MERGEABLE |
| [#164](https://github.com/ruru-m07/gitru/pull/164) | feat(collaboration): cache GitHub conversation comments locally | `aab107dacf11e67216de602e42954a61e46c4423` | 11/11 | none | MERGEABLE |
| [#165](https://github.com/ruru-m07/gitru/pull/165) | fix(collaboration): retain native cooldowns across clock changes | `c0a9f2f352982f185f93d7c751c052c14d9c7761` | 11/11 | none | MERGEABLE |
| [#166](https://github.com/ruru-m07/gitru/pull/166) | feat(collaboration): bound detail cache retention and WAL maintenance (RURU-104) | `44a94ee69e1d739b55adfdfec666dbacf89e266c` | 14/14 | none | MERGEABLE |
| [#167](https://github.com/ruru-m07/gitru/pull/167) | feat(collaboration): check out saved PR heads locally (RURU-136) | `9548c53cf3c926a47e474d8d9b9511bc41e99d41` | 14/14 | none | MERGEABLE |
| [#168](https://github.com/ruru-m07/gitru/pull/168) | feat(collaboration): cache and navigate pull request commits (RURU-137) | `0da82cc7e5a3543512be683e68ac074f8d9689cd` | 14/14 | none | MERGEABLE |
| [#169](https://github.com/ruru-m07/gitru/pull/169) | feat(collaboration): add local inbox state (RURU-124) | `4d31a40743d3e203f2fc1027f17b4ae292268478` | 14/14 | none | MERGEABLE |
| [#170](https://github.com/ruru-m07/gitru/pull/170) | feat(collaboration): show cached current-head checks (RURU-118) | `e75bf77b920c9f1198d4e7d825cf829298209f5b` | 14/14 | none | MERGEABLE |
| [#171](https://github.com/ruru-m07/gitru/pull/171) | feat(collaboration): admit durable commands and protect pending intent (RURU-114) | `de0e245d5b10bbebb9b66ad78d92febe084a986b` | 14/14 | none | MERGEABLE |
| [#172](https://github.com/ruru-m07/gitru/pull/172) | feat(collaboration): cache pull request files and selected diffs (RURU-119) | `08a2db3c4d66269ce72bc5a780e9deda78a57f43` | 14/14 | none | MERGEABLE |
| [#173](https://github.com/ruru-m07/gitru/pull/173) | test(collaboration): measure native cached navigation (RURU-125) | `8eccce4e7989faa09a3b8b07abbe5727c0492947` | 14/14 | none | MERGEABLE |
| [#174](https://github.com/ruru-m07/gitru/pull/174) | feat(collaboration): sync GitLab todos with explicit inbox semantics (RURU-127) | `cfc5b2345f9ab2ac2ac813e4e05c497a679a8fb7` | 14/14 | none | MERGEABLE |
| [#175](https://github.com/ruru-m07/gitru/pull/175) | feat(collaboration): expose safe local sync diagnostics (RURU-126) | `e9cbad6496b0ff5870c928949f32bd7c6c98ec7c` | 14/14 | none | MERGEABLE |
| [#176](https://github.com/ruru-m07/gitru/pull/176) | feat(collaboration): add durable command delivery and outcome reconciliation | `7ef7e88dc213315e3e32c3e43b89262844185e8b` | 14/14 | none | MERGEABLE |
| [#177](https://github.com/ruru-m07/gitru/pull/177) | feat(collaboration): project durable intent into cached queries | `af1d906fc9ddb1f8e0000eaa56d2bbf5bba0cf18` | 14/14 | none | MERGEABLE |
| [#178](https://github.com/ruru-m07/gitru/pull/178) | feat(collaboration): cache pull request reviews and threads | `25c14c7d5c0828890f49d3109befe67d644e70dc` | 14/14 | none | MERGEABLE |
| [#179](https://github.com/ruru-m07/gitru/pull/179) | feat(collaboration): cache GitLab discussions and approval observations | `403db05d9dd4daacf5cbed200566418cfd8c782e` | 14/14 | none | MERGEABLE |
| [#180](https://github.com/ruru-m07/gitru/pull/180) | feat(collaboration): review and recover saved commands (RURU-117) | `6dc602fa0daa1e574bebb84c695a84cd0b01baa0` | 14/14 | none | MERGEABLE |
| [#181](https://github.com/ruru-m07/gitru/pull/181) | feat(collaboration): queue GitHub title and body edits (RURU-129) | `dd324b1a525470a68cc66816354956307749f576` | 14/14 | none | MERGEABLE |
| [#182](https://github.com/ruru-m07/gitru/pull/182) | feat(collaboration): queue provider inbox read and done actions (RURU-130) | `9db3b4383a433806a3f35d331c7d527404c3541e` | 8/14 | IN_PROGRESS: 3, QUEUED: 3 | MERGEABLE |
| [#183](https://github.com/ruru-m07/gitru/pull/183) | feat(collaboration): submit comments with durable drafts and receipts (RURU-131) | `d3652209679de76f1e3ade30e2d2ea41e08729fd` | 12/14 | IN_PROGRESS: 2 | MERGEABLE |
| [#184](https://github.com/ruru-m07/gitru/pull/184) | feat(collaboration): cache and browse GitHub activity timelines (RURU-122) | `bb0c73c56017b9013d70400d2bbfdd0396dc58d5` | 12/14 | IN_PROGRESS: 1, QUEUED: 1 | MERGEABLE |
| [#185](https://github.com/ruru-m07/gitru/pull/185) | feat(collaboration): queue guarded GitHub close and reopen actions (RURU-129) | `048f53dd65232223a3fb48ed69a2e8d4a734eee8` | 12/14 | IN_PROGRESS: 2 | MERGEABLE |
| [#186](https://github.com/ruru-m07/gitru/pull/186) | docs(collaboration): define encryption policy and test pinned SQLCipher (RURU-108) | `486a936e700fe00b863ac26f6f0b526477574a5d` | 10/14 | IN_PROGRESS: 2, QUEUED: 2 | MERGEABLE |
| [#187](https://github.com/ruru-m07/gitru/pull/187) | feat(collaboration): create GitHub issues from durable drafts (RURU-134) | `98090a78aeb02894e022a0dc3cd3e5dd33b12ce0` | 6/14 | IN_PROGRESS: 6, QUEUED: 2 | MERGEABLE |
| [#188](https://github.com/ruru-m07/gitru/pull/188) | feat(collaboration): cache Bitbucket PR comments (RURU-122) | `907325097099d38f932f1aea87e378b5e39cc45a` | 5/14 | IN_PROGRESS: 7, QUEUED: 2 | MERGEABLE |
| [#189](https://github.com/ruru-m07/gitru/pull/189) | feat(collaboration): cache GitLab conversation notes (RURU-122) | `ae7e572d605e248693ce626efb3add671856d199` | 4/14 | IN_PROGRESS: 7, QUEUED: 3 | MERGEABLE |
| [#190](https://github.com/ruru-m07/gitru/pull/190) | fix(collaboration): preserve legacy receipts across inbox integration | `599c537afa3cf622d91c7fd0ca9c4c73183ff58f` | 5/14 | IN_PROGRESS: 6, QUEUED: 3 | MERGEABLE |
