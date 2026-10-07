# Collaboration engine — Linear backlog

Parent: [RURU-53: Build a provider-independent, local-first remote collaboration engine](https://linear.app/catra/issue/RURU-53/build-a-provider-independent-local-first-remote-collaboration-engine).
Project: Gitru. Team: ruru. Milestone: Hosted Collaboration.
Published and verified: 2026-10-03.
49 direct children: 44 new, 5 existing issues updated. All 49 issue bodies and 106 direct blocker relationships were read back; the graph is acyclic. Six immediate independent tasks are Todo. Other unblocked product-policy work remains Backlog.

Local C01–C49 keys are planning keys, not Linear identifiers. Linear status and dependency links are authoritative and may change; this document records the verified publication snapshot. Retained valid dependencies from the original five children are included below.

## Goal

Continuation started on 2026-10-03: signed foundation commit
`baafef75e82743b756b412bd5d7bc443636c76c8` is published in draft
[PR #141](https://github.com/ruru-m07/gitru/pull/141). RURU-138 is In Review;
RURU-95 is In Review in [PR #142](https://github.com/ruru-m07/gitru/pull/142),
RURU-105 in [PR #143](https://github.com/ruru-m07/gitru/pull/143), and RURU-99
in [PR #144](https://github.com/ruru-m07/gitru/pull/144). These are scoped,
signed stacks: #142 and #144 target #141's branch; #143 targets #142's branch.
`make verify` and packaged macOS E2E (2 specs / 3 cases) passed locally.
Credential recovery passed 58 collaboration tests including 25 process-kill
checkpoints; the integrated migration suite passed 70 tests including 12
migration cases. Draft recovery passed 208 frontend, 49 collaboration and 11
desktop tests, plus native fixture QA of actor switching/copy/save/export/cancel.
Scope and evidence live in each branch's `docs/architecture/collaboration-work/`
note. RURU-76 is In Review in
[PR #146](https://github.com/ruru-m07/gitru/pull/146): 83 native, 32 client,
171 desktop frontend and 3 command-caller tests pass, including real pending
query invalidation regressions. RURU-106's native core is published in
[PR #145](https://github.com/ruru-m07/gitru/pull/145), with 88 native tests and
14 independent recovery cases. Both target #143. RURU-106 stays In Progress:
desktop writer shutdown/picker/dialog integration, schema-0003 policy and
Windows power-loss qualification remain open. Its core deliberately refuses
schemas beyond the known v1/v2 policy rather than guessing about durable intent.

The next parallel batch started from the reviewed #146 commit: RURU-97 owns
independent detail storage/hydration; RURU-100 owns contextual capabilities and
their shared UI consumers. RURU-97 is now In Review in
[PR #147](https://github.com/ruru-m07/gitru/pull/147), stacked on #146. It passes
114 native, 3 command-caller, 36 client and 177 desktop frontend tests, plus
lint/types/Clippy; independent access and source-authority reviews are recorded
in its work note. Production GitHub detail endpoints remain RURU-77/78.
RURU-100 is In Review in [PR #149](https://github.com/ruru-m07/gitru/pull/149),
stacked on #147, with its issue-specific design note written before major edits.
Shared IPC generation followed the detail contract freeze. Contextual native
policies and UI cover account/repository/resource targets, saved reads, sync and
write availability. Temporary network/quota errors retain authorized saved
reads, while access loss still suppresses provider content.
Remote CI is running; the original publication index below remains a snapshot.
Windows Rust migration tests pass after the explicit SQL LF policy. Foundation
CodeQL passes on `7c5364d`; a later native-host lifetime correction passes all
200 frontend tests and fresh packaged macOS E2E (2 specs / 3 cases), with remote
Linux/Windows qualification still required. Neither test order nor assertions
were weakened to work around the observed post-cleanup native view registration.
The exact `e1ad8c569` foundation head subsequently passed Linux/macOS packaged
E2E and all CodeQL analyses; Windows checks remained pending. A provider-registry
Linux Rust run exposed a newly-written CLI executable fixture race. Test-only
publication now uses an exited child writer, with nine focused macOS cases
passing and all original assertions retained. The original OS error was not
captured; fresh Linux CI qualifies this correction. Production CLI behavior and
test ordering/parallelism are unchanged.
Windows CI also exposed an expired-candidate fixture that backdated an Instant
before runner boot. An equivalent strict expiry deadline preserves the five-minute
account selection lifetime and avoids subtraction; its exact-boundary regression
and all 48 foundation collaboration tests pass locally. The exact `c215389`
foundation head subsequently passed Rust tests on Linux/macOS/Windows, all
CodeQL analyses and Linux/macOS packaged E2E; Windows packaged E2E remained
pending when recorded. Narrow synthetic `Vec::remove` fixture repairs in #142
and #147 preserve exact-one assertions without suppression; their current
signed stacks are published and remote rescanning is pending.
The hourly chat continuation checks live Linear/PR state before picking work.
PR publication does not authorize merging. Exact-head dynamic CodeQL runs have
now been observed on stacked children #142, #143, #146 and #147. Every merge
requires completed relevant analyses and zero relevant alerts on its own exact
proposed head. This records observed run availability without assuming why it
changed or treating an ancestor's scan as a child's qualification.

RURU-100's complete contextual slice is published in draft #149 atop the signed
RURU-97 contract; source/evidence are in
[RURU-100's work note](./collaboration-work/RURU-100.md). Atomic native policies
control account/repository/resource saved reads, sync and explicitly unsupported
writes. Shared workspace/sidebar consumers distinguish provider inbox semantics,
permissions and data missingness while retaining authored text/CAS through a
same-actor authorization refresh. A shared local deadline coordinator repairs
cooldown eligibility. All 127 collaboration, 44 SDK and 193 desktop tests and
both independent reviews and native synthetic fixture QA pass locally.
Current exact-head remote security and platform checks remain separate gates; the
publication index below remains historical. The separately published RURU-99
authored-editor/recovery extraction needs narrow merge reconciliation.

Eight scoped PRs are published: #141 foundation, #142 credential cutover, #143
migration recovery, #144 saved drafts, #145 backup/recovery native core, #146
provider registry, #147 independent details and #149 contextual capabilities.
RURU-106 remains In Progress with its desktop/schema/power-loss gates open.
The next read experience chunks are RURU-77 pull request details and RURU-78
issue details; both consume the reviewed registry/detail/capability contracts.
The hourly continuation rechecks live blockers, worktrees, overlap and CI before
starting them. No PR has been merged.

Subsequent #147 Linux CI captured OS code 26 (`Text file busy`) while launching
the inherited credential test snapshot. Audit found only two parent-written
executable snapshots, in RURU-95/RURU-106; both use waited Unix child writers now,
preserving every hard-kill checkpoint and assertion without retries or changed
parallelism. Signed repairs passed native/Clippy/review and are propagated through
the descendant stacks. Current RURU-100 again passes all 127 native tests. New
exact-head Linux/platform/security runs remain pending; see the work notes for
the observed errno versus source-inferred descriptor mechanism.

Make GitHub, GitLab and Bitbucket collaboration feel like native local data: cached navigation never waits for provider HTTP, while one Rust runtime keeps durable SQLite projections current across all tabs.

The next bounded batch has started: RURU-77 and RURU-78 are In Progress in
isolated managed worktrees based on #149's signed `6799e6d` head. The approved
[RURU-77 contract](./collaboration-work/RURU-77.md) uses one Body endpoint and
atomic description/typed-metadata publication, with migration 0005 and independent
field authority. The issue mapper consumes that shared contract; generated IPC
and common UI integrate after it freezes. No new slice is yet validated or
published. Live CI on 3 October subsequently confirms all eight published heads
pass every reported check, including Rust and packaged E2E on all three platforms.
#141/#142/#143/#146/#147 report 15/15 with all CodeQL analyses; #144/#145/#149
report 11/11 with no current exact-head CodeQL run observed. Those three retain
their separate security qualification gate. No credentials were inspected and
no PR was merged.

Provider accounts work independently of Gitru cloud sign-in. GitHub connects with a manual PAT or explicitly selected existing GitHub CLI credential; Gitru does not initiate an OAuth/device flow.

## Current implementation — 2 October 2026

The initial GitHub read slice exists locally on branch `ruru/remote-collaboration`: SQLite/WAL/FTS, secure account lifecycle, PAT/CLI connection, repository discovery/selection, PR/issue/inbox summaries, local queries/search, background paging, durable drafts, revision catch-up and the single-window Accounts dialog.

Historical local evidence: 194 frontend tests, 47 collaboration Rust tests, 3 native caller-policy tests, lint/types/Clippy/formatting, production frontend build and two packaged macOS E2E specs (3 cases). Native computer-use QA verified Inbox → Accounts → close using an isolated E2E vault. The 3 October continuation above supersedes publication and CI status. Live production credentials/vaults remain separate gates; no release completion is claimed.

## Source of truth

Read `docs/architecture/remote-collaboration-engine.md` in the repository before implementing. Its section 23 records actual implementation limits; sections 6–20 define contracts and sections 21–22 define rollout and gates. The document is committed in foundation PR #141; each follow-on branch adds its own work note before major edits.

Rust owns provider networking, credentials, durable storage, sync, commands and delivery. TypeScript owns typed local queries/subscriptions and UI. Generate changed IPC with `make typegen`; never edit `packages/commands` manually.

## Delivery rules

- Child issues are concrete remaining slices. Implemented foundations are context, not new duplicate tickets.
- Follow actual Linear blocker relations. Work that is independent can run in parallel; aim for one or two reviewable PRs and split further if a measured scope grows.
- Provider differences use capabilities and native facets; unsupported operations are explicit.
- Add focused contracts, recovery and UI checks for changed behavior. Keep local verification, remote CI and production/provider validation distinct.
- Build durable admission, delivery evidence, effective optimistic projections and conflict recovery before exposing remote writes.
- Never silently retry an ambiguous non-idempotent create or promise an unsupported guarded merge.
- Enterprise/Data Center/webhook-relay assessments are later conditional scope, not prerequisites for the public-provider milestone.

## Completion gates

- A user can connect accounts, link a local clone, browse cached lists/details/search/inbox offline after restart, and see truthful partial/stale/access states.
- One bounded native scheduler coalesces work across tabs, prioritizes demand fairly, honors provider quotas and survives reconnect/crash.
- Supported edits preserve durable user intent and reconcile optimistic state without losing text, duplicating ambiguous delivery or using an obsolete authorization/head.
- GitLab and Bitbucket Cloud exercise the common contracts with explicit unsupported capabilities; self-hosted support is claimed only for tested versions.
- Migration/backup/restore, resource budgets, account isolation and supported-platform vault/runtime gates have recorded evidence.

## Pickup order

Start with [RURU-76](https://linear.app/catra/issue/RURU-76/introduce-a-provider-registry-canonical-resource-identities-and) (provider registry/identities) and [RURU-95](https://linear.app/catra/issue/RURU-95/make-credential-replacement-recover-safely-after-a-process-crash) (crash-safe credential cutover). [RURU-75](https://linear.app/catra/issue/RURU-75/verify-production-github-patcli-authentication-and-offline-restart) (production PAT/CLI verification), [RURU-99](https://linear.app/catra/issue/RURU-99/recover-private-drafts-after-disconnect-or-missing-subjects) (draft recovery), [RURU-105](https://linear.app/catra/issue/RURU-105/test-schema-evolution-and-recoverable-migration-failures) (migration fixtures) and [RURU-138](https://linear.app/catra/issue/RURU-138/review-and-publish-the-implemented-collaboration-foundation) (review/publish current foundation) are independent parallel lanes, all marked Todo. [RURU-108](https://linear.app/catra/issue/RURU-108/decide-cached-private-data-encryption-policy-before-ga) is an unblocked product-policy decision kept in Backlog for the GA gate.

Then implement [RURU-97](https://linear.app/catra/issue/RURU-97/add-independent-detail-scope-storage-and-hydration-contracts) (detail scopes), [RURU-77](https://linear.app/catra/issue/RURU-77/hydrate-and-render-cached-pull-request-details) / [RURU-78](https://linear.app/catra/issue/RURU-78/hydrate-and-render-cached-issue-details) (cached PR/issue details), [RURU-98](https://linear.app/catra/issue/RURU-98/add-foreground-demand-leases-to-the-native-sync-scheduler) (foreground leases) and [RURU-121](https://linear.app/catra/issue/RURU-121/add-bounded-frontend-prefetch-and-cached-navigation) (bounded prefetch). Broader facets, local mapping and reliability checks follow; remote actions stay blocked on durable delivery and conflict recovery. Check live blockers and changed-file overlap before picking parallel implementation work.

## Published issue index

### Foundation, identities and accounts

| Task | State | Blocked by |
| --- | --- | --- |
| [RURU-138: Review and publish the implemented collaboration foundation](https://linear.app/catra/issue/RURU-138/review-and-publish-the-implemented-collaboration-foundation) | Todo | — |
| [RURU-76: Introduce a provider registry, canonical resource identities and capabilities](https://linear.app/catra/issue/RURU-76/introduce-a-provider-registry-canonical-resource-identities-and) | Todo | — |
| [RURU-75: Verify production GitHub PAT/CLI authentication and offline restart](https://linear.app/catra/issue/RURU-75/verify-production-github-patcli-authentication-and-offline-restart) | Todo | — |
| [RURU-95: Make credential replacement recover safely after a process crash](https://linear.app/catra/issue/RURU-95/make-credential-replacement-recover-safely-after-a-process-crash) | Todo | — |
| [RURU-99: Recover private drafts after disconnect or missing subjects](https://linear.app/catra/issue/RURU-99/recover-private-drafts-after-disconnect-or-missing-subjects) | Todo | — |
| [RURU-100: Drive collaboration UI from typed resource capabilities](https://linear.app/catra/issue/RURU-100/drive-collaboration-ui-from-typed-resource-capabilities) | Backlog | [RURU-76](https://linear.app/catra/issue/RURU-76/introduce-a-provider-registry-canonical-resource-identities-and) |
| [RURU-105: Test schema evolution and recoverable migration failures](https://linear.app/catra/issue/RURU-105/test-schema-evolution-and-recoverable-migration-failures) | Todo | — |
| [RURU-108: Decide cached private-data encryption policy before GA](https://linear.app/catra/issue/RURU-108/decide-cached-private-data-encryption-policy-before-ga) | Backlog | — |

### Cached collaboration experience

| Task | State | Blocked by |
| --- | --- | --- |
| [RURU-96: Link local Git remotes to collaboration repositories and accounts](https://linear.app/catra/issue/RURU-96/link-local-git-remotes-to-collaboration-repositories-and-accounts) | Backlog | [RURU-76](https://linear.app/catra/issue/RURU-76/introduce-a-provider-registry-canonical-resource-identities-and) |
| [RURU-97: Add independent detail-scope storage and hydration contracts](https://linear.app/catra/issue/RURU-97/add-independent-detail-scope-storage-and-hydration-contracts) | Backlog | [RURU-76](https://linear.app/catra/issue/RURU-76/introduce-a-provider-registry-canonical-resource-identities-and) |
| [RURU-77: Hydrate and render cached pull request details](https://linear.app/catra/issue/RURU-77/hydrate-and-render-cached-pull-request-details) | In Review | [RURU-97](https://linear.app/catra/issue/RURU-97/add-independent-detail-scope-storage-and-hydration-contracts), [RURU-76](https://linear.app/catra/issue/RURU-76/introduce-a-provider-registry-canonical-resource-identities-and) |
| [RURU-78: Hydrate and render cached issue details](https://linear.app/catra/issue/RURU-78/hydrate-and-render-cached-issue-details) | In Progress | [RURU-97](https://linear.app/catra/issue/RURU-97/add-independent-detail-scope-storage-and-hydration-contracts), [RURU-76](https://linear.app/catra/issue/RURU-76/introduce-a-provider-registry-canonical-resource-identities-and) |
| [RURU-121: Add bounded frontend prefetch and cached navigation](https://linear.app/catra/issue/RURU-121/add-bounded-frontend-prefetch-and-cached-navigation) | Backlog | [RURU-77](https://linear.app/catra/issue/RURU-77/hydrate-and-render-cached-pull-request-details), [RURU-98](https://linear.app/catra/issue/RURU-98/add-foreground-demand-leases-to-the-native-sync-scheduler), [RURU-78](https://linear.app/catra/issue/RURU-78/hydrate-and-render-cached-issue-details) |
| [RURU-79: Resolve inbox notifications to cached PR and issue subjects](https://linear.app/catra/issue/RURU-79/resolve-inbox-notifications-to-cached-pr-and-issue-subjects) | Backlog | [RURU-77](https://linear.app/catra/issue/RURU-77/hydrate-and-render-cached-pull-request-details), [RURU-78](https://linear.app/catra/issue/RURU-78/hydrate-and-render-cached-issue-details), [RURU-76](https://linear.app/catra/issue/RURU-76/introduce-a-provider-registry-canonical-resource-identities-and) |
| [RURU-122: Cache and display conversation comments and activity timelines](https://linear.app/catra/issue/RURU-122/cache-and-display-conversation-comments-and-activity-timelines) | Backlog | [RURU-77](https://linear.app/catra/issue/RURU-77/hydrate-and-render-cached-pull-request-details), [RURU-78](https://linear.app/catra/issue/RURU-78/hydrate-and-render-cached-issue-details) |
| [RURU-123: Cache PR review summaries and review threads with head context](https://linear.app/catra/issue/RURU-123/cache-pr-review-summaries-and-review-threads-with-head-context) | Backlog | [RURU-122](https://linear.app/catra/issue/RURU-122/cache-and-display-conversation-comments-and-activity-timelines), [RURU-77](https://linear.app/catra/issue/RURU-77/hydrate-and-render-cached-pull-request-details) |
| [RURU-118: Show cached checks and commit statuses for the current PR head](https://linear.app/catra/issue/RURU-118/show-cached-checks-and-commit-statuses-for-the-current-pr-head) | Backlog | [RURU-77](https://linear.app/catra/issue/RURU-77/hydrate-and-render-cached-pull-request-details) |
| [RURU-119: Add cached PR changed-file and diff navigation](https://linear.app/catra/issue/RURU-119/add-cached-pr-changed-file-and-diff-navigation) | Backlog | [RURU-77](https://linear.app/catra/issue/RURU-77/hydrate-and-render-cached-pull-request-details), [RURU-98](https://linear.app/catra/issue/RURU-98/add-foreground-demand-leases-to-the-native-sync-scheduler) |
| [RURU-124: Add local inbox snooze, bookmark and disposition state](https://linear.app/catra/issue/RURU-124/add-local-inbox-snooze-bookmark-and-disposition-state) | Backlog | [RURU-79](https://linear.app/catra/issue/RURU-79/resolve-inbox-notifications-to-cached-pr-and-issue-subjects) |
| [RURU-136: Check out pull request branches through the local Git workflow](https://linear.app/catra/issue/RURU-136/check-out-pull-request-branches-through-the-local-git-workflow) | In Progress | [RURU-77](https://linear.app/catra/issue/RURU-77/hydrate-and-render-cached-pull-request-details), [RURU-96](https://linear.app/catra/issue/RURU-96/link-local-git-remotes-to-collaboration-repositories-and-accounts) |
| [RURU-137: Cache and navigate the pull request commit list](https://linear.app/catra/issue/RURU-137/cache-and-navigate-the-pull-request-commit-list) | In Review | [RURU-77](https://linear.app/catra/issue/RURU-77/hydrate-and-render-cached-pull-request-details) |

### Sync, storage and performance

| Task | State | Blocked by |
| --- | --- | --- |
| [RURU-98: Add foreground demand leases to the native sync scheduler](https://linear.app/catra/issue/RURU-98/add-foreground-demand-leases-to-the-native-sync-scheduler) | Backlog | [RURU-97](https://linear.app/catra/issue/RURU-97/add-independent-detail-scope-storage-and-hydration-contracts) |
| [RURU-101: Validate incremental reconciliation for independent resource facets](https://linear.app/catra/issue/RURU-101/validate-incremental-reconciliation-for-independent-resource-facets) | Backlog | [RURU-97](https://linear.app/catra/issue/RURU-97/add-independent-detail-scope-storage-and-hydration-contracts) |
| [RURU-102: Add fair rate budgets and scheduler lifecycle recovery](https://linear.app/catra/issue/RURU-102/add-fair-rate-budgets-and-scheduler-lifecycle-recovery) | Backlog | [RURU-98](https://linear.app/catra/issue/RURU-98/add-foreground-demand-leases-to-the-native-sync-scheduler), [RURU-76](https://linear.app/catra/issue/RURU-76/introduce-a-provider-registry-canonical-resource-identities-and) |
| [RURU-103: Prove sync and revision recovery across real native webviews](https://linear.app/catra/issue/RURU-103/prove-sync-and-revision-recovery-across-real-native-webviews) | Backlog | [RURU-98](https://linear.app/catra/issue/RURU-98/add-foreground-demand-leases-to-the-native-sync-scheduler) |
| [RURU-104: Implement bounded cache retention, pins and WAL maintenance](https://linear.app/catra/issue/RURU-104/implement-bounded-cache-retention-pins-and-wal-maintenance) | Backlog | [RURU-97](https://linear.app/catra/issue/RURU-97/add-independent-detail-scope-storage-and-hydration-contracts), [RURU-99](https://linear.app/catra/issue/RURU-99/recover-private-drafts-after-disconnect-or-missing-subjects) |
| [RURU-106: Add consistent collaboration backup and restore recovery](https://linear.app/catra/issue/RURU-106/add-consistent-collaboration-backup-and-restore-recovery) | Backlog | [RURU-105](https://linear.app/catra/issue/RURU-105/test-schema-evolution-and-recoverable-migration-failures) |
| [RURU-125: Measure cached navigation latency and memory through native IPC](https://linear.app/catra/issue/RURU-125/measure-cached-navigation-latency-and-memory-through-native-ipc) | Backlog | [RURU-121](https://linear.app/catra/issue/RURU-121/add-bounded-frontend-prefetch-and-cached-navigation), [RURU-103](https://linear.app/catra/issue/RURU-103/prove-sync-and-revision-recovery-across-real-native-webviews) |
| [RURU-126: Expose safe local sync diagnostics and actionable retry states](https://linear.app/catra/issue/RURU-126/expose-safe-local-sync-diagnostics-and-actionable-retry-states) | Backlog | [RURU-102](https://linear.app/catra/issue/RURU-102/add-fair-rate-budgets-and-scheduler-lifecycle-recovery), [RURU-125](https://linear.app/catra/issue/RURU-125/measure-cached-navigation-latency-and-memory-through-native-ipc) |
| [RURU-107: Verify collaboration vaults and storage on supported desktop platforms](https://linear.app/catra/issue/RURU-107/verify-collaboration-vaults-and-storage-on-supported-desktop-platforms) | Backlog | [RURU-105](https://linear.app/catra/issue/RURU-105/test-schema-evolution-and-recoverable-migration-failures), [RURU-103](https://linear.app/catra/issue/RURU-103/prove-sync-and-revision-recovery-across-real-native-webviews), [RURU-95](https://linear.app/catra/issue/RURU-95/make-credential-replacement-recover-safely-after-a-process-crash) |

### GitLab and Bitbucket Cloud

| Task | State | Blocked by |
| --- | --- | --- |
| [RURU-110: Connect GitLab accounts and discover repositories](https://linear.app/catra/issue/RURU-110/connect-gitlab-accounts-and-discover-repositories) | Backlog | [RURU-76](https://linear.app/catra/issue/RURU-76/introduce-a-provider-registry-canonical-resource-identities-and), [RURU-95](https://linear.app/catra/issue/RURU-95/make-credential-replacement-recover-safely-after-a-process-crash) |
| [RURU-111: Sync GitLab merge requests and issues through shared local queries](https://linear.app/catra/issue/RURU-111/sync-gitlab-merge-requests-and-issues-through-shared-local-queries) | Backlog | [RURU-100](https://linear.app/catra/issue/RURU-100/drive-collaboration-ui-from-typed-resource-capabilities), [RURU-97](https://linear.app/catra/issue/RURU-97/add-independent-detail-scope-storage-and-hydration-contracts), [RURU-110](https://linear.app/catra/issue/RURU-110/connect-gitlab-accounts-and-discover-repositories) |
| [RURU-127: Support GitLab todos with explicit inbox semantics](https://linear.app/catra/issue/RURU-127/support-gitlab-todos-with-explicit-inbox-semantics) | Backlog | [RURU-79](https://linear.app/catra/issue/RURU-79/resolve-inbox-notifications-to-cached-pr-and-issue-subjects), [RURU-111](https://linear.app/catra/issue/RURU-111/sync-gitlab-merge-requests-and-issues-through-shared-local-queries), [RURU-100](https://linear.app/catra/issue/RURU-100/drive-collaboration-ui-from-typed-resource-capabilities) |
| [RURU-128: Add GitLab discussion and approval detail facets](https://linear.app/catra/issue/RURU-128/add-gitlab-discussion-and-approval-detail-facets) | Backlog | [RURU-122](https://linear.app/catra/issue/RURU-122/cache-and-display-conversation-comments-and-activity-timelines), [RURU-118](https://linear.app/catra/issue/RURU-118/show-cached-checks-and-commit-statuses-for-the-current-pr-head), [RURU-123](https://linear.app/catra/issue/RURU-123/cache-pr-review-summaries-and-review-threads-with-head-context), [RURU-111](https://linear.app/catra/issue/RURU-111/sync-gitlab-merge-requests-and-issues-through-shared-local-queries) |
| [RURU-112: Add Bitbucket Cloud account and pull request reads](https://linear.app/catra/issue/RURU-112/add-bitbucket-cloud-account-and-pull-request-reads) | Backlog | [RURU-111](https://linear.app/catra/issue/RURU-111/sync-gitlab-merge-requests-and-issues-through-shared-local-queries), [RURU-76](https://linear.app/catra/issue/RURU-76/introduce-a-provider-registry-canonical-resource-identities-and), [RURU-100](https://linear.app/catra/issue/RURU-100/drive-collaboration-ui-from-typed-resource-capabilities) |

### Durable writes and remote actions

| Task | State | Blocked by |
| --- | --- | --- |
| [RURU-114: Add durable command admission and outbox schema](https://linear.app/catra/issue/RURU-114/add-durable-command-admission-and-outbox-schema) | Backlog | [RURU-104](https://linear.app/catra/issue/RURU-104/implement-bounded-cache-retention-pins-and-wal-maintenance), [RURU-97](https://linear.app/catra/issue/RURU-97/add-independent-detail-scope-storage-and-hydration-contracts), [RURU-76](https://linear.app/catra/issue/RURU-76/introduce-a-provider-registry-canonical-resource-identities-and), [RURU-105](https://linear.app/catra/issue/RURU-105/test-schema-evolution-and-recoverable-migration-failures) |
| [RURU-115: Implement outbox delivery and ambiguous-outcome recovery](https://linear.app/catra/issue/RURU-115/implement-outbox-delivery-and-ambiguous-outcome-recovery) | Backlog | [RURU-95](https://linear.app/catra/issue/RURU-95/make-credential-replacement-recover-safely-after-a-process-crash), [RURU-106](https://linear.app/catra/issue/RURU-106/add-consistent-collaboration-backup-and-restore-recovery), [RURU-114](https://linear.app/catra/issue/RURU-114/add-durable-command-admission-and-outbox-schema) |
| [RURU-116: Project optimistic intent into local lists, details, counts and search](https://linear.app/catra/issue/RURU-116/project-optimistic-intent-into-local-lists-details-counts-and-search) | Backlog | [RURU-114](https://linear.app/catra/issue/RURU-114/add-durable-command-admission-and-outbox-schema), [RURU-97](https://linear.app/catra/issue/RURU-97/add-independent-detail-scope-storage-and-hydration-contracts) |
| [RURU-117: Add conflict resolution and superseding-command recovery UI](https://linear.app/catra/issue/RURU-117/add-conflict-resolution-and-superseding-command-recovery-ui) | Backlog | [RURU-116](https://linear.app/catra/issue/RURU-116/project-optimistic-intent-into-local-lists-details-counts-and-search), [RURU-115](https://linear.app/catra/issue/RURU-115/implement-outbox-delivery-and-ambiguous-outcome-recovery) |
| [RURU-129: Deliver queued issue and PR desired-state edits](https://linear.app/catra/issue/RURU-129/deliver-queued-issue-and-pr-desired-state-edits) | Backlog | [RURU-78](https://linear.app/catra/issue/RURU-78/hydrate-and-render-cached-issue-details), [RURU-115](https://linear.app/catra/issue/RURU-115/implement-outbox-delivery-and-ambiguous-outcome-recovery), [RURU-117](https://linear.app/catra/issue/RURU-117/add-conflict-resolution-and-superseding-command-recovery-ui), [RURU-116](https://linear.app/catra/issue/RURU-116/project-optimistic-intent-into-local-lists-details-counts-and-search), [RURU-77](https://linear.app/catra/issue/RURU-77/hydrate-and-render-cached-pull-request-details) |
| [RURU-130: Deliver provider inbox read/done actions with explicit activity policy](https://linear.app/catra/issue/RURU-130/deliver-provider-inbox-readdone-actions-with-explicit-activity-policy) | Backlog | [RURU-79](https://linear.app/catra/issue/RURU-79/resolve-inbox-notifications-to-cached-pr-and-issue-subjects), [RURU-117](https://linear.app/catra/issue/RURU-117/add-conflict-resolution-and-superseding-command-recovery-ui), [RURU-116](https://linear.app/catra/issue/RURU-116/project-optimistic-intent-into-local-lists-details-counts-and-search), [RURU-115](https://linear.app/catra/issue/RURU-115/implement-outbox-delivery-and-ambiguous-outcome-recovery), [RURU-124](https://linear.app/catra/issue/RURU-124/add-local-inbox-snooze-bookmark-and-disposition-state) |
| [RURU-131: Submit comments with durable drafts and ambiguous-create handling](https://linear.app/catra/issue/RURU-131/submit-comments-with-durable-drafts-and-ambiguous-create-handling) | Backlog | [RURU-115](https://linear.app/catra/issue/RURU-115/implement-outbox-delivery-and-ambiguous-outcome-recovery), [RURU-99](https://linear.app/catra/issue/RURU-99/recover-private-drafts-after-disconnect-or-missing-subjects), [RURU-122](https://linear.app/catra/issue/RURU-122/cache-and-display-conversation-comments-and-activity-timelines), [RURU-117](https://linear.app/catra/issue/RURU-117/add-conflict-resolution-and-superseding-command-recovery-ui) |
| [RURU-132: Submit PR reviews bound to the inspected head and anchors](https://linear.app/catra/issue/RURU-132/submit-pr-reviews-bound-to-the-inspected-head-and-anchors) | Backlog | [RURU-115](https://linear.app/catra/issue/RURU-115/implement-outbox-delivery-and-ambiguous-outcome-recovery), [RURU-123](https://linear.app/catra/issue/RURU-123/cache-pr-review-summaries-and-review-threads-with-head-context), [RURU-117](https://linear.app/catra/issue/RURU-117/add-conflict-resolution-and-superseding-command-recovery-ui), [RURU-119](https://linear.app/catra/issue/RURU-119/add-cached-pr-changed-file-and-diff-navigation), [RURU-99](https://linear.app/catra/issue/RURU-99/recover-private-drafts-after-disconnect-or-missing-subjects) |
| [RURU-133: Add guarded online merge with asynchronous receipt reconciliation](https://linear.app/catra/issue/RURU-133/add-guarded-online-merge-with-asynchronous-receipt-reconciliation) | Backlog | [RURU-115](https://linear.app/catra/issue/RURU-115/implement-outbox-delivery-and-ambiguous-outcome-recovery), [RURU-77](https://linear.app/catra/issue/RURU-77/hydrate-and-render-cached-pull-request-details), [RURU-118](https://linear.app/catra/issue/RURU-118/show-cached-checks-and-commit-statuses-for-the-current-pr-head), [RURU-123](https://linear.app/catra/issue/RURU-123/cache-pr-review-summaries-and-review-threads-with-head-context), [RURU-117](https://linear.app/catra/issue/RURU-117/add-conflict-resolution-and-superseding-command-recovery-ui) |
| [RURU-134: Create issues through durable drafts and reconciled delivery](https://linear.app/catra/issue/RURU-134/create-issues-through-durable-drafts-and-reconciled-delivery) | Backlog | [RURU-78](https://linear.app/catra/issue/RURU-78/hydrate-and-render-cached-issue-details), [RURU-117](https://linear.app/catra/issue/RURU-117/add-conflict-resolution-and-superseding-command-recovery-ui), [RURU-116](https://linear.app/catra/issue/RURU-116/project-optimistic-intent-into-local-lists-details-counts-and-search), [RURU-99](https://linear.app/catra/issue/RURU-99/recover-private-drafts-after-disconnect-or-missing-subjects), [RURU-115](https://linear.app/catra/issue/RURU-115/implement-outbox-delivery-and-ambiguous-outcome-recovery) |
| [RURU-120: Create pull requests with online branch and head validation](https://linear.app/catra/issue/RURU-120/create-pull-requests-with-online-branch-and-head-validation) | Backlog | [RURU-77](https://linear.app/catra/issue/RURU-77/hydrate-and-render-cached-pull-request-details), [RURU-96](https://linear.app/catra/issue/RURU-96/link-local-git-remotes-to-collaboration-repositories-and-accounts), [RURU-115](https://linear.app/catra/issue/RURU-115/implement-outbox-delivery-and-ambiguous-outcome-recovery), [RURU-99](https://linear.app/catra/issue/RURU-99/recover-private-drafts-after-disconnect-or-missing-subjects), [RURU-117](https://linear.app/catra/issue/RURU-117/add-conflict-resolution-and-superseding-command-recovery-ui), [RURU-116](https://linear.app/catra/issue/RURU-116/project-optimistic-intent-into-local-lists-details-counts-and-search) |

### Conditional later expansion

| Task | State | Blocked by |
| --- | --- | --- |
| [RURU-109: Configure and validate self-hosted provider instances](https://linear.app/catra/issue/RURU-109/configure-and-validate-self-hosted-provider-instances) | Backlog | [RURU-95](https://linear.app/catra/issue/RURU-95/make-credential-replacement-recover-safely-after-a-process-crash), [RURU-76](https://linear.app/catra/issue/RURU-76/introduce-a-provider-registry-canonical-resource-identities-and) |
| [RURU-113: Assess Bitbucket Data Center support against selected server versions](https://linear.app/catra/issue/RURU-113/assess-bitbucket-data-center-support-against-selected-server-versions) | Backlog | [RURU-109](https://linear.app/catra/issue/RURU-109/configure-and-validate-self-hosted-provider-instances), [RURU-112](https://linear.app/catra/issue/RURU-112/add-bitbucket-cloud-account-and-pull-request-reads) |
| [RURU-135: Assess an optional webhook relay after measuring polling limits](https://linear.app/catra/issue/RURU-135/assess-an-optional-webhook-relay-after-measuring-polling-limits) | Backlog | [RURU-125](https://linear.app/catra/issue/RURU-125/measure-cached-navigation-latency-and-memory-through-native-ipc), [RURU-102](https://linear.app/catra/issue/RURU-102/add-fair-rate-budgets-and-scheduler-lifecycle-recovery) |

## RURU-75: Verify production GitHub PAT/CLI authentication and offline restart

Planning key: C01. Group: Authentication. Priority: High. State: Todo.
Linear: [RURU-75](https://linear.app/catra/issue/RURU-75/verify-production-github-patcli-authentication-and-offline-restart).
Prerequisites: Existing implementation; no new child blocker.

Validate the implemented connection flow with an isolated test account in a packaged build; this is verification of production vault behavior, not another OAuth implementation.

Acceptance criteria:

- [ ] Manual PAT and explicitly selected CLI import verify the same actor and store credentials only in the production OS vault.
- [ ] Sync, restart offline, credential replacement, revocation and disconnect behave predictably; Gitru never changes CLI authentication.
- [ ] Record reproducible results without tokens, private payloads or real credentials in fixtures; record other-platform gaps explicitly.

## RURU-95: Make credential replacement recover safely after a process crash

Planning key: C02. Group: Authentication. Priority: High. State: Todo.
Linear: [RURU-95](https://linear.app/catra/issue/RURU-95/make-credential-replacement-recover-safely-after-a-process-crash).
Prerequisites: Existing implementation; no new child blocker.

Close the cross-store crash window between writing a replacement credential and committing its SQLite authorization epoch; the current compensation only runs if the process survives.

Acceptance criteria:

- [ ] Use a recoverable cutover journal or versioned credential references so an old authorization partition cannot use an uncommitted replacement token.
- [ ] Fault-inject crashes before and after each vault/SQLite boundary; recovery either completes a verified cutover or requires authentication.
- [ ] Preserve drafts, reject obsolete in-flight responses, and safely clean orphaned credentials without losing a working account.

## RURU-76: Introduce a provider registry, canonical resource identities and capabilities

Planning key: C03. Group: Provider contracts. Priority: High. State: Todo.
Linear: [RURU-76](https://linear.app/catra/issue/RURU-76/introduce-a-provider-registry-canonical-resource-identities-and).
Prerequisites: Existing implementation; no new child blocker.

Extend the existing single-provider seam into account/instance-dispatched adapters and typed resource/facet capabilities. Exercise a divergent GitLab fixture before expanding GitHub-specific details. Persist account-scoped locator/endpoint aliases and expose typed local resource resolution so callers can open resources without parsing provider APIs.

Acceptance criteria:

- [ ] Runtime dispatch selects an adapter by provider instance/account; normal feature code never branches on provider names or uses raw provider HTTP.
- [ ] Common identities distinguish immutable IDs from mutable repository paths and numbers; capability states describe unsupported, unavailable and supported operations.
- [ ] GitHub and GitLab fixtures cover nested namespaces, IDs versus IIDs, divergent inbox semantics, pagination and errors through the same contract.
- [ ] Persist and resolve locator/endpoint aliases to one canonical resource: GitHub issue/pull representations of a PR and repository rename/transfer must converge without crossing account or provider-instance boundaries.

## RURU-96: Link local Git remotes to collaboration repositories and accounts

Planning key: C04. Group: Read experience. Priority: High. State: Backlog.
Linear: [RURU-96](https://linear.app/catra/issue/RURU-96/link-local-git-remotes-to-collaboration-repositories-and-accounts).
Prerequisites: [RURU-76](https://linear.app/catra/issue/RURU-76/introduce-a-provider-registry-canonical-resource-identities-and).

Add durable links between local clones and remote repository identities without changing Git remotes or checking out branches implicitly.

Acceptance criteria:

- [ ] Resolve SSH/HTTPS remotes to provider instances and cached immutable repository identities through Rust Git services.
- [ ] Multiple remotes/accounts present an explicit choice; users can inspect, change or remove a link.
- [ ] Links survive restart and repository rename; local Git and collaboration views navigate in both directions without provider HTTP on the cached path.

## RURU-97: Add independent detail-scope storage and hydration contracts

Planning key: C05. Group: Read experience. Priority: High. State: Backlog.
Linear: [RURU-97](https://linear.app/catra/issue/RURU-97/add-independent-detail-scope-storage-and-hydration-contracts).
Prerequisites: [RURU-76](https://linear.app/catra/issue/RURU-76/introduce-a-provider-registry-canonical-resource-identities-and).

Separate resource detail coverage and synchronization from summary-feed coverage; establish the engine seam consumed by the PR and issue detail tasks.

Acceptance criteria:

- [ ] Persist bounded detail facets with independent freshness, completeness, authorization and revision metadata.
- [ ] Local detail queries never trigger HTTP; explicit hydration intent coalesces jobs and commits observations before notifying views.
- [ ] Distinguish uncached, omitted/oversized and authoritatively empty content; test restart, offline reads, stale responses and access loss.

## RURU-98: Add foreground demand leases to the native sync scheduler

Planning key: C06. Group: Sync scheduling. Priority: High. State: Backlog.
Linear: [RURU-98](https://linear.app/catra/issue/RURU-98/add-foreground-demand-leases-to-the-native-sync-scheduler).
Prerequisites: [RURU-97](https://linear.app/catra/issue/RURU-97/add-independent-detail-scope-storage-and-hydration-contracts).

Track visible list/detail demand across webviews so interactive work has priority without creating per-tab polling engines.

Acceptance criteria:

- [ ] Leases are account/scope-bound, expire safely, and release on hidden/closed/disposed views and account epoch changes.
- [ ] Equivalent demand across tabs produces one provider job; foreground work bypasses backfill within rate and concurrency budgets.
- [ ] Deterministic tests cover lost cleanup, reopen, idle views, queue saturation and offline/reconnect without starving reconciliation.

## RURU-121: Add bounded frontend prefetch and cached navigation

Planning key: C07. Group: Read experience. Priority: High. State: Backlog.
Linear: [RURU-121](https://linear.app/catra/issue/RURU-121/add-bounded-frontend-prefetch-and-cached-navigation).
Prerequisites: [RURU-77](https://linear.app/catra/issue/RURU-77/hydrate-and-render-cached-pull-request-details), [RURU-98](https://linear.app/catra/issue/RURU-98/add-foreground-demand-leases-to-the-native-sync-scheduler), [RURU-78](https://linear.app/catra/issue/RURU-78/hydrate-and-render-cached-issue-details).

Connect active views to native demand leases and prefetch a small working set on hover, keyboard focus and recent navigation.

Acceptance criteria:

- [ ] Cached list/detail navigation renders useful content without waiting for provider HTTP; missing data has a clear state.
- [ ] Prefetch shares native jobs across webviews and respects a bounded count/byte/concurrency budget.
- [ ] Hidden tabs drop urgency; large-list and keyboard tests prove demand/prefetch does not grow without bound.

## RURU-79: Resolve inbox notifications to cached PR and issue subjects

Planning key: C08. Group: Read experience. Priority: High. State: Backlog.
Linear: [RURU-79](https://linear.app/catra/issue/RURU-79/resolve-inbox-notifications-to-cached-pr-and-issue-subjects).
Prerequisites: [RURU-77](https://linear.app/catra/issue/RURU-77/hydrate-and-render-cached-pull-request-details), [RURU-78](https://linear.app/catra/issue/RURU-78/hydrate-and-render-cached-issue-details), [RURU-76](https://linear.app/catra/issue/RURU-76/introduce-a-provider-registry-canonical-resource-identities-and).

Extend the implemented notification-summary inbox so opening a supported item reaches its actual local collaboration subject.

Acceptance criteria:

- [ ] Resolve provider notification subjects to canonical account-scoped PR/issue identities and open cached details immediately.
- [ ] Hydration accepts only validated provider resource paths; missing or unsupported subjects retain a safe provider link and useful explanation.
- [ ] Tests cover permission changes, private-account isolation, restart and offline navigation; this task performs no mark-read write.

## RURU-122: Cache and display conversation comments and activity timelines

Planning key: C09. Group: Read experience. Priority: Medium. State: Backlog.
Linear: [RURU-122](https://linear.app/catra/issue/RURU-122/cache-and-display-conversation-comments-and-activity-timelines).
Prerequisites: [RURU-77](https://linear.app/catra/issue/RURU-77/hydrate-and-render-cached-pull-request-details), [RURU-78](https://linear.app/catra/issue/RURU-78/hydrate-and-render-cached-issue-details).

Add read-only, independently paginated PR/issue conversation comments and a bounded activity timeline.

Acceptance criteria:

- [ ] Persist deterministic ordering, pagination and independent facet coverage; partial history is distinct from an empty conversation.
- [ ] Render cached provider text safely offline and tolerate unsupported activity types.
- [ ] Contract/UI tests cover edits, deleted comments, incomplete pages, restart and authorization loss.

## RURU-123: Cache PR review summaries and review threads with head context

Planning key: C10. Group: Read experience. Priority: Medium. State: Backlog.
Linear: [RURU-123](https://linear.app/catra/issue/RURU-123/cache-pr-review-summaries-and-review-threads-with-head-context).
Prerequisites: [RURU-122](https://linear.app/catra/issue/RURU-122/cache-and-display-conversation-comments-and-activity-timelines), [RURU-77](https://linear.app/catra/issue/RURU-77/hydrate-and-render-cached-pull-request-details).

Add read-only review decisions and review threads without implying old reviews apply to a newly pushed head.

Acceptance criteria:

- [ ] Persist reviewer decisions and independently paged review threads with head/base/anchor metadata.
- [ ] Show stale or outdated anchors and distinguish historical review state from current-head coverage.
- [ ] Offline, insufficient-permission and partial-coverage states remain honest; no review mutation is dispatched.

## RURU-118: Show cached checks and commit statuses for the current PR head

Planning key: C11. Group: Read experience. Priority: Medium. State: Backlog.
Linear: [RURU-118](https://linear.app/catra/issue/RURU-118/show-cached-checks-and-commit-statuses-for-the-current-pr-head).
Prerequisites: [RURU-77](https://linear.app/catra/issue/RURU-77/hydrate-and-render-cached-pull-request-details).

Introduce a head-scoped checks/status facet that can later support guarded review and merge decisions.

Acceptance criteria:

- [ ] Checks and statuses bind to exact head OIDs; prior-head results remain clearly historical.
- [ ] Partial coverage, missing permission or a changed head cannot produce an authoritative all-checks-passed state.
- [ ] Coalesced background hydration and local-only UI reads pass fake-provider, restart and stale-head tests.

## RURU-119: Add cached PR changed-file and diff navigation

Planning key: C12. Group: Read experience. Priority: Medium. State: Backlog.
Linear: [RURU-119](https://linear.app/catra/issue/RURU-119/add-cached-pr-changed-file-and-diff-navigation).
Prerequisites: [RURU-77](https://linear.app/catra/issue/RURU-77/hydrate-and-render-cached-pull-request-details), [RURU-98](https://linear.app/catra/issue/RURU-98/add-foreground-demand-leases-to-the-native-sync-scheduler).

Load PR file summaries and selected diffs on demand, reusing the existing desktop diff viewers.

Acceptance criteria:

- [ ] File/diff observations bind to base and head; changing the head invalidates only affected coverage.
- [ ] Diff/blob caching is bounded and handles binary, omitted and oversized content with clear offline states.
- [ ] Navigation supports keyboard access and safe provider content rendering; local Git fetching/parsing stays in Rust.

## RURU-124: Add local inbox snooze, bookmark and disposition state

Planning key: C13. Group: Read experience. Priority: Medium. State: Backlog.
Linear: [RURU-124](https://linear.app/catra/issue/RURU-124/add-local-inbox-snooze-bookmark-and-disposition-state).
Prerequisites: [RURU-79](https://linear.app/catra/issue/RURU-79/resolve-inbox-notifications-to-cached-pr-and-issue-subjects).

Provide useful inbox organization owned by Gitru, explicitly separate from the provider's read/done state.

Acceptance criteria:

- [ ] Local disposition persists across restart and updates effective lists/counts/search coherently.
- [ ] New provider activity does not disappear behind stale local read intent; remote versus local status is clear.
- [ ] Unsupported provider inbox capabilities remain explicit; no remote write is sent in this task.

## RURU-99: Recover private drafts after disconnect or missing subjects

Planning key: C14. Group: Read experience. Priority: High. State: Todo.
Linear: [RURU-99](https://linear.app/catra/issue/RURU-99/recover-private-drafts-after-disconnect-or-missing-subjects).
Prerequisites: Existing implementation; no new child blocker.

Add recovery navigation for the durable drafts that already survive disconnect, missing targets and cache rebuilding.

Acceptance criteria:

- [ ] List and reopen/copy/export user-authored drafts after restart and disconnect without requiring valid provider credentials.
- [ ] Recovery never resurrects inaccessible provider bodies or mixes account-owned drafts.
- [ ] Generation conflicts preserve the user's text; recovering a draft never sends it automatically.

## RURU-100: Drive collaboration UI from typed resource capabilities

Planning key: C15. Group: Provider contracts. Priority: Medium. State: Backlog.
Linear: [RURU-100](https://linear.app/catra/issue/RURU-100/drive-collaboration-ui-from-typed-resource-capabilities).
Prerequisites: [RURU-76](https://linear.app/catra/issue/RURU-76/introduce-a-provider-registry-canonical-resource-identities-and).

Replace future feature/provider conditionals with shared capability-driven presentation and query/action availability.

Acceptance criteria:

- [ ] Account, repository and resource capabilities control inbox, reviews, checks and future actions.
- [ ] Unsupported, permission-denied, not-yet-loaded and read-only states have distinct user-visible behavior.
- [ ] Divergent provider fixtures share ordinary feature components and cannot dispatch an unsupported operation.

## RURU-101: Validate incremental reconciliation for independent resource facets

Planning key: C16. Group: Sync reliability. Priority: High. State: Backlog.
Linear: [RURU-101](https://linear.app/catra/issue/RURU-101/validate-incremental-reconciliation-for-independent-resource-facets).
Prerequisites: [RURU-97](https://linear.app/catra/issue/RURU-97/add-independent-detail-scope-storage-and-hydration-contracts).

Extend the existing conservative feed reconciliation to independently refreshed details, children and head-bound facets; do not rewrite the already implemented paging/absence policy.

Acceptance criteria:

- [ ] Each facet defines validators, overlap/watermark rules and qualified absence evidence rather than treating timestamps as a complete event log.
- [ ] Edits, closes, merges, renames, deletions and access denial reconcile without partial traversal erasing useful cache or drafts.
- [ ] Fake-provider tests cover 304 responses, overlapping pages, pagination drift and obsolete authorization/head responses.

## RURU-102: Add fair rate budgets and scheduler lifecycle recovery

Planning key: C17. Group: Sync scheduling. Priority: High. State: Backlog.
Linear: [RURU-102](https://linear.app/catra/issue/RURU-102/add-fair-rate-budgets-and-scheduler-lifecycle-recovery).
Prerequisites: [RURU-98](https://linear.app/catra/issue/RURU-98/add-foreground-demand-leases-to-the-native-sync-scheduler), [RURU-76](https://linear.app/catra/issue/RURU-76/introduce-a-provider-registry-canonical-resource-identities-and).

Extend the current persisted GitHub REST quota/backoff into fair scheduling across accounts, provider API families and foreground/background work.

Acceptance criteria:

- [ ] Foreground work remains responsive while background accounts/scopes receive bounded service under saturated queues.
- [ ] Respect observed provider cooldowns and documented quota families, with bounded retry jitter and no spinning on permanent/auth failures.
- [ ] Deterministic sleep/resume, clock-change, reconnect and long-cooldown tests demonstrate persisted recovery.

## RURU-103: Prove sync and revision recovery across real native webviews

Planning key: C18. Group: Sync reliability. Priority: High. State: Backlog.
Linear: [RURU-103](https://linear.app/catra/issue/RURU-103/prove-sync-and-revision-recovery-across-real-native-webviews).
Prerequisites: [RURU-98](https://linear.app/catra/issue/RURU-98/add-foreground-demand-leases-to-the-native-sync-scheduler).

Build a retained multi-client fake-provider harness and focused native lifecycle coverage beyond the current fixed Accounts handoff test.

Acceptance criteria:

- [ ] Equivalent host/child demand produces one provider job and consistent committed local snapshots.
- [ ] Dropped/reordered hints, catch-up overflow and reload recover through durable revisions without stale private cache repopulation.
- [ ] Disconnect and process-crash tests fence delayed reads/writes and preserve drafts; avoid exposing arbitrary evaluation in production.

## RURU-104: Implement bounded cache retention, pins and WAL maintenance

Planning key: C19. Group: Storage and operations. Priority: Medium. State: Backlog.
Linear: [RURU-104](https://linear.app/catra/issue/RURU-104/implement-bounded-cache-retention-pins-and-wal-maintenance).
Prerequisites: [RURU-97](https://linear.app/catra/issue/RURU-97/add-independent-detail-scope-storage-and-hydration-contracts), [RURU-99](https://linear.app/catra/issue/RURU-99/recover-private-drafts-after-disconnect-or-missing-subjects).

Introduce explicit coverage and rebuildable-content budgets without evicting user intent.

Acceptance criteria:

- [ ] Eviction runs in bounded batches and protects drafts, pins, minimum identities and any durable pending-intent/evidence references.
- [ ] Eviction updates coverage/revisions so the UI distinguishes uncached from empty data and can request hydration again.
- [ ] Measure database/WAL growth under large datasets and verify safe checkpoints without blocking active reads; extend protection tests when outbox lands.

## RURU-105: Test schema evolution and recoverable migration failures

Planning key: C20. Group: Storage and operations. Priority: High. State: Todo.
Linear: [RURU-105](https://linear.app/catra/issue/RURU-105/test-schema-evolution-and-recoverable-migration-failures).
Prerequisites: Existing implementation; no new child blocker.

Extend current forward migrations with realistic upgrade fixtures and interruption/recovery verification.

Acceptance criteria:

- [ ] Old-schema fixtures migrate without losing drafts, identity, authorization partitions or durable revisions.
- [ ] Unknown/newer schemas and failed upgrades never silently reset user data; large upgrades expose bounded progress where needed.
- [ ] Regenerate changed wire contracts through make typegen and test compatible/incompatible clients and restart boundaries.

## RURU-106: Add consistent collaboration backup and restore recovery

Planning key: C21. Group: Storage and operations. Priority: Medium. State: Backlog.
Linear: [RURU-106](https://linear.app/catra/issue/RURU-106/add-consistent-collaboration-backup-and-restore-recovery).
Prerequisites: [RURU-105](https://linear.app/catra/issue/RURU-105/test-schema-evolution-and-recoverable-migration-failures).

Back up and restore durable local collaboration state consistently while SQLite WAL and the writer runtime are active.

Acceptance criteria:

- [ ] Backups are transactionally consistent and verified; never copy only the live main database while omitting its WAL.
- [ ] Recovery preserves drafts and refuses destructive reset without explicit user choice; credentials are not exported in backups.
- [ ] When outbox exists, restored dispatchable commands are quarantined until reconciled; cover interrupted backup/restore and database corruption.

## RURU-125: Measure cached navigation latency and memory through native IPC

Planning key: C22. Group: Performance. Priority: High. State: Backlog.
Linear: [RURU-125](https://linear.app/catra/issue/RURU-125/measure-cached-navigation-latency-and-memory-through-native-ipc).
Prerequisites: [RURU-121](https://linear.app/catra/issue/RURU-121/add-bounded-frontend-prefetch-and-cached-navigation), [RURU-103](https://linear.app/catra/issue/RURU-103/prove-sync-and-revision-recovery-across-real-native-webviews).

Measure the actual SQLite → generated IPC → React useful-content path rather than relying on the existing storage-only benchmark.

Acceptance criteria:

- [ ] Define repeatable hardware/datasets including 10k cached items, cold restart and multiple native views.
- [ ] Record p50/p95/p99, payload sizes, startup time, Rust/webview memory and disk/WAL growth against architecture section 18.
- [ ] Document bottlenecks and add justified performance regression checks; separate warm storage latency from end-to-end UI latency.

## RURU-126: Expose safe local sync diagnostics and actionable retry states

Planning key: C23. Group: Storage and operations. Priority: Medium. State: Backlog.
Linear: [RURU-126](https://linear.app/catra/issue/RURU-126/expose-safe-local-sync-diagnostics-and-actionable-retry-states).
Prerequisites: [RURU-102](https://linear.app/catra/issue/RURU-102/add-fair-rate-budgets-and-scheduler-lifecycle-recovery), [RURU-125](https://linear.app/catra/issue/RURU-125/measure-cached-navigation-latency-and-memory-through-native-ipc).

Add bounded, user-readable sync health and a privacy-preserving export for troubleshooting.

Acceptance criteria:

- [ ] Show coverage/queue age, cooldowns, retry category, storage/WAL usage and aggregate latency using local state.
- [ ] Auth, permission, rate, offline, unavailable and permanent errors offer appropriate recovery without retry loops.
- [ ] Export tests reject tokens, usernames, repository identifiers, provider URLs and remote text; authorized contextual UI remains separate.

## RURU-107: Verify collaboration vaults and storage on supported desktop platforms

Planning key: C24. Group: Release gates. Priority: High. State: Backlog.
Linear: [RURU-107](https://linear.app/catra/issue/RURU-107/verify-collaboration-vaults-and-storage-on-supported-desktop-platforms).
Prerequisites: [RURU-105](https://linear.app/catra/issue/RURU-105/test-schema-evolution-and-recoverable-migration-failures), [RURU-103](https://linear.app/catra/issue/RURU-103/prove-sync-and-revision-recovery-across-real-native-webviews), [RURU-95](https://linear.app/catra/issue/RURU-95/make-credential-replacement-recover-safely-after-a-process-crash).

Extend packaged platform coverage specifically for collaboration, coordinating with the existing cross-platform smoke issue.

Acceptance criteria:

- [ ] Windows/Linux/macOS lanes verify bundled SQLite/FTS, exclusive runtime ownership, migration, offline startup and native vault failure behavior.
- [ ] Use isolated fake credentials in automated tests; record live production-vault verification separately per supported OS.
- [ ] Record actual platform results and remaining gates without equating a macOS fixture pass to cross-platform support.

## RURU-108: Decide cached private-data encryption policy before GA

Planning key: C25. Group: Storage and operations. Priority: Medium. State: Backlog.
Linear: [RURU-108](https://linear.app/catra/issue/RURU-108/decide-cached-private-data-encryption-policy-before-ga).
Prerequisites: Existing implementation; no new child blocker.

Resolve the architecture's explicit at-rest encryption product/security gate; evaluate a tested SQLite encryption option if required.

Acceptance criteria:

- [ ] Document threat model, platform/key lifecycle, disk/WAL/backup exposure and the chosen product policy.
- [ ] If encryption is required, spike compatibility with the pinned SQLx/SQLite build, migrations, backup and recovery and create bounded follow-up implementation tasks.
- [ ] Do not imply the current credential vault encrypts SQLite content; distinguish permission hardening from encryption.

## RURU-109: Configure and validate self-hosted provider instances

Planning key: C26. Group: Later provider expansion. Priority: Low. State: Backlog.
Linear: [RURU-109](https://linear.app/catra/issue/RURU-109/configure-and-validate-self-hosted-provider-instances).
Prerequisites: [RURU-95](https://linear.app/catra/issue/RURU-95/make-credential-replacement-recover-safely-after-a-process-crash), [RURU-76](https://linear.app/catra/issue/RURU-76/introduce-a-provider-registry-canonical-resource-identities-and).

Conditional later scope: instance-aware accounts and private-host trust controls for a user-selected enterprise provider/version.

Acceptance criteria:

- [ ] Validate canonical endpoints, host-specific vault namespaces, trust policy and credential-safe redirects; keep provider networking in Rust.
- [ ] Keep accounts and immutable identities isolated between public and private instances, even when logins/path names match.
- [ ] Test the selected supported versions and document unsupported configurations; no broad private-network access or hosted secret broker is assumed.

## RURU-110: Connect GitLab accounts and discover repositories

Planning key: C27. Group: Provider rollout. Priority: Medium. State: Backlog.
Linear: [RURU-110](https://linear.app/catra/issue/RURU-110/connect-gitlab-accounts-and-discover-repositories).
Prerequisites: [RURU-76](https://linear.app/catra/issue/RURU-76/introduce-a-provider-registry-canonical-resource-identities-and), [RURU-95](https://linear.app/catra/issue/RURU-95/make-credential-replacement-recover-safely-after-a-process-crash).

Add GitLab.com authentication and repository discovery through the common native lifecycle; enterprise instances follow their separate trust task.

Acceptance criteria:

- [ ] Verify provider actor/credential capabilities and use the existing native vault, epoch, disconnect and crash-recovery contracts.
- [ ] Discover/select resumable paginated repositories including nested subgroup paths without assuming GitHub owner/name semantics.
- [ ] Contract tests cover multiple accounts, replacement, rate/auth errors, rename and revoked scope; use current official auth requirements during implementation.

## RURU-111: Sync GitLab merge requests and issues through shared local queries

Planning key: C28. Group: Provider rollout. Priority: Medium. State: In Review (local qualification recorded below).
Linear: [RURU-111](https://linear.app/catra/issue/RURU-111/sync-gitlab-merge-requests-and-issues-through-shared-local-queries).
Prerequisites: [RURU-100](https://linear.app/catra/issue/RURU-100/drive-collaboration-ui-from-typed-resource-capabilities), [RURU-97](https://linear.app/catra/issue/RURU-97/add-independent-detail-scope-storage-and-hydration-contracts), [RURU-110](https://linear.app/catra/issue/RURU-110/connect-gitlab-accounts-and-discover-repositories).

Implement GitLab MR/issue summary and supported detail reads using ordinary collaboration components.

Acceptance criteria:

- [x] Retain immutable native IDs, scoped IIDs/locators and provider facets while sharing local projections and queries.
- [x] Support progressive paging, independently cached details, offline restart and explicit capability/access states.
- [x] Divergent fixtures cover state transitions, transfers/renames, pagination and access loss with no GitLab branches in ordinary UI.

## RURU-127: Support GitLab todos with explicit inbox semantics

Planning key: C29. Group: Provider rollout. Priority: Medium. State: In Review.
Linear: [RURU-127](https://linear.app/catra/issue/RURU-127/support-gitlab-todos-with-explicit-inbox-semantics).
Prerequisites: [RURU-79](https://linear.app/catra/issue/RURU-79/resolve-inbox-notifications-to-cached-pr-and-issue-subjects), [RURU-111](https://linear.app/catra/issue/RURU-111/sync-gitlab-merge-requests-and-issues-through-shared-local-queries), [RURU-100](https://linear.app/catra/issue/RURU-100/drive-collaboration-ui-from-typed-resource-capabilities).

Add a provider-native todo inbox source without pretending todos are GitHub notification threads.

Acceptance criteria:

- [x] Persist todo identity, source, subject and completion/read semantics explicitly in the shared model.
- [x] Reuse inbox UI where semantics match and resolve supported subjects to cached local details.
- [x] Unsupported completion actions remain unavailable until durable delivery policies are verified; read-only syncing/offline tests pass.

Implementation is published in draft [PR #174](https://github.com/ruru-m07/gitru/pull/174), stacked on #172. The native read-only adapter interleaves pending sweeps with completed history and persists the exact cursor across restart. Typed provider completion remains separate from Gitru-local inbox state, and immutable target evidence controls cached MR/issue routing. Final local `make verify` passed: 716 frontend tests / 1 platform skip; 1,091 top-level Rust tests plus 2 child helper runs / 5 helper ignores, plus lint/types/build/format/Clippy. Generated IPC is 129 commands / 388 schemas. Remote CI remains separate and pending. No live provider, personal credential, production database/vault or packaged GUI was used; no merge occurred. See the [RURU-127 work note](./collaboration-work/RURU-127.md) for exact source and bounds. The original publication table above remains a historical snapshot.

## RURU-128: Add GitLab discussion and approval detail facets

Planning key: C30. Group: Provider rollout. Priority: Medium. State: Backlog.
Linear: [RURU-128](https://linear.app/catra/issue/RURU-128/add-gitlab-discussion-and-approval-detail-facets).
Prerequisites: [RURU-122](https://linear.app/catra/issue/RURU-122/cache-and-display-conversation-comments-and-activity-timelines), [RURU-118](https://linear.app/catra/issue/RURU-118/show-cached-checks-and-commit-statuses-for-the-current-pr-head), [RURU-123](https://linear.app/catra/issue/RURU-123/cache-pr-review-summaries-and-review-threads-with-head-context), [RURU-111](https://linear.app/catra/issue/RURU-111/sync-gitlab-merge-requests-and-issues-through-shared-local-queries).

Validate richer shared detail contracts against GitLab discussion, approval and pipeline/check differences.

Acceptance criteria:

- [ ] Normalize supported discussion/review/approval facets while preserving provider-native positions and plan/version limitations.
- [ ] Bind approvals and pipeline/check observations to relevant heads; unavailable/partial data cannot imply readiness to merge.
- [ ] Run shared component/adapter tests with divergent fixtures and offline snapshots; remain read-only until mutation policies exist.

## RURU-112: Add Bitbucket Cloud account and pull request reads

Planning key: C31. Group: Provider rollout. Priority: Medium. State: Backlog.
Linear: [RURU-112](https://linear.app/catra/issue/RURU-112/add-bitbucket-cloud-account-and-pull-request-reads).
Prerequisites: [RURU-111](https://linear.app/catra/issue/RURU-111/sync-gitlab-merge-requests-and-issues-through-shared-local-queries), [RURU-76](https://linear.app/catra/issue/RURU-76/introduce-a-provider-registry-canonical-resource-identities-and), [RURU-100](https://linear.app/catra/issue/RURU-100/drive-collaboration-ui-from-typed-resource-capabilities).

Implement a bounded read-only Bitbucket Cloud vertical slice after the common contract is tested against two providers.

Acceptance criteria:

- [ ] Verify current API-token authentication, actor identity and repositories through the common account/vault lifecycle.
- [ ] Sync PR states, participants/tasks and supported details using opaque continuation links and shared local queries.
- [ ] Unsupported native issues/inbox and strict head-guarded merge remain capability states; test pagination, access isolation and offline restart.

## RURU-113: Assess Bitbucket Data Center support against selected server versions

Planning key: C32. Group: Later provider expansion. Priority: Low. State: Backlog.
Linear: [RURU-113](https://linear.app/catra/issue/RURU-113/assess-bitbucket-data-center-support-against-selected-server-versions).
Prerequisites: [RURU-109](https://linear.app/catra/issue/RURU-109/configure-and-validate-self-hosted-provider-instances), [RURU-112](https://linear.app/catra/issue/RURU-112/add-bitbucket-cloud-account-and-pull-request-reads).

Conditional feasibility task; Data Center must be a distinct adapter from Bitbucket Cloud, not an assumed compatible API.

Acceptance criteria:

- [ ] Select and record a concrete supported server/version matrix with auth, pagination, PR version/head and capability evidence.
- [ ] Use fixtures/spikes to test common-model fit, including external issue tracking and unsupported inbox semantics.
- [ ] Produce bounded implementation follow-ups and a support decision; do not claim shipping Data Center support from this assessment.

## RURU-114: Add durable command admission and outbox schema

Planning key: C33. Group: Offline writes. Priority: High. State: In Progress (native admission locally qualified; publication/remote CI pending).
Linear: [RURU-114](https://linear.app/catra/issue/RURU-114/add-durable-command-admission-and-outbox-schema).
Prerequisites: [RURU-104](https://linear.app/catra/issue/RURU-104/implement-bounded-cache-retention-pins-and-wal-maintenance), [RURU-97](https://linear.app/catra/issue/RURU-97/add-independent-detail-scope-storage-and-hydration-contracts), [RURU-76](https://linear.app/catra/issue/RURU-76/introduce-a-provider-registry-canonical-resource-identities-and), [RURU-105](https://linear.app/catra/issue/RURU-105/test-schema-evolution-and-recoverable-migration-failures).

Introduce operation-specific immutable intent, separate from provider base observations, without exposing remote mutation controls yet.

Acceptance criteria:

- [x] Persist command UUID, canonical submission/hash, account epoch, target, guards and submitted dependencies atomically with its local receipt.
- [x] Duplicate identical submissions reuse receipts; changed payload with the same UUID is rejected.
- [x] Forward migrations preserve original intent hashes and drafts; fake-provider/storage tests cover crash/restart and concurrent admission.
- [x] Integrate retention protection for command targets, predecessor receipts and attempt evidence; eviction tests prove pending/conflicted/unknown commands retain the references needed for recovery.

Implementation record: [RURU-114](./collaboration-work/RURU-114.md), signed source
`bc01d7d0044d0325437d28dbb1a37598eb0e4837`, stacked on RURU-124 migration 0012.
Focused 13 native and 5 migration tests, independent-review corrections and full
`make verify` pass. Publication and remote CI remain pending. No public operation/IPC or dispatch exists yet.
Normalized blob references persist; actual attachment-byte recovery requires the
future blob store. Remote CI is not inferred from these local fixtures.

## RURU-115: Implement outbox delivery and ambiguous-outcome recovery

Planning key: C34. Group: Offline writes. Priority: High. State: Backlog.
Linear: [RURU-115](https://linear.app/catra/issue/RURU-115/implement-outbox-delivery-and-ambiguous-outcome-recovery).
Prerequisites: [RURU-95](https://linear.app/catra/issue/RURU-95/make-credential-replacement-recover-safely-after-a-process-crash), [RURU-106](https://linear.app/catra/issue/RURU-106/add-consistent-collaboration-backup-and-restore-recovery), [RURU-114](https://linear.app/catra/issue/RURU-114/add-durable-command-admission-and-outbox-schema).

Build a native delivery worker with durable attempt evidence and operation-specific retry/reconciliation policy.

Acceptance criteria:

- [ ] Persist attempt start before dispatch and distinguish queued, sending, accepted, confirmed, rejected, conflict and outcome-unknown.
- [ ] Crash at each boundary preserves evidence; accepted/202 acknowledgements are not treated as completion and ambiguous non-idempotent creates never blindly retry.
- [ ] Account replacement/revocation fences delivery; dependent commands wait for proven predecessor results and quota/offline recovery is bounded.
- [ ] Exercise backup → successful remote dispatch → restore: quarantine restored potentially sent commands and allow recovery only through operation-specific strong evidence, never automatic duplicate delivery.

## RURU-116: Project optimistic intent into local lists, details, counts and search

Planning key: C35. Group: Offline writes. Priority: High. State: Backlog.
Linear: [RURU-116](https://linear.app/catra/issue/RURU-116/project-optimistic-intent-into-local-lists-details-counts-and-search).
Prerequisites: [RURU-114](https://linear.app/catra/issue/RURU-114/add-durable-command-admission-and-outbox-schema), [RURU-97](https://linear.app/catra/issue/RURU-97/add-independent-detail-scope-storage-and-hydration-contracts).

Layer pending intent over provider observations as a derived effective projection consumed by existing local queries.

Acceptance criteria:

- [ ] Pending effects update detail/list/count/search views together after durable admission, including across webviews.
- [ ] Refresh cannot overwrite submitted intent; rejection removes only that command's effect and replays successors over current base.
- [ ] Relevant effective changes invalidate paging/subscriptions coherently; restart reconstructs optimistic state from SQLite.

## RURU-117: Add conflict resolution and superseding-command recovery UI

Planning key: C36. Group: Offline writes. Priority: High. State: Backlog.
Linear: [RURU-117](https://linear.app/catra/issue/RURU-117/add-conflict-resolution-and-superseding-command-recovery-ui).
Prerequisites: [RURU-116](https://linear.app/catra/issue/RURU-116/project-optimistic-intent-into-local-lists-details-counts-and-search), [RURU-115](https://linear.app/catra/issue/RURU-115/implement-outbox-delivery-and-ambiguous-outcome-recovery).

Implement operation-specific base/remote/desired comparison and user-preserving recovery instead of last-write-wins.

Acceptance criteria:

- [ ] Independent field changes merge when safe; overlapping text/workflow/head changes remain visible conflicts.
- [ ] Changed resolution creates a new superseding command UUID while preserving original intent, attempts and dependency evidence.
- [ ] UI preserves text and offers retry/review/export/cancel options honestly; cancellation after dispatch never claims to undo a remote action.

## RURU-129: Deliver queued issue and PR desired-state edits

Planning key: C37. Group: Remote actions. Priority: Medium. State: Backlog.
Linear: [RURU-129](https://linear.app/catra/issue/RURU-129/deliver-queued-issue-and-pr-desired-state-edits).
Prerequisites: [RURU-78](https://linear.app/catra/issue/RURU-78/hydrate-and-render-cached-issue-details), [RURU-115](https://linear.app/catra/issue/RURU-115/implement-outbox-delivery-and-ambiguous-outcome-recovery), [RURU-117](https://linear.app/catra/issue/RURU-117/add-conflict-resolution-and-superseding-command-recovery-ui), [RURU-116](https://linear.app/catra/issue/RURU-116/project-optimistic-intent-into-local-lists-details-counts-and-search), [RURU-77](https://linear.app/catra/issue/RURU-77/hydrate-and-render-cached-pull-request-details).

First remote edit slice: selected title/body/label and close/reopen intent using per-operation capability and conflict policies.

Acceptance criteria:

- [ ] Admit supported offline intent durably and apply effective local updates immediately without hiding pending state.
- [ ] Revalidate affected fields, workflow and permissions; disclose guarded versus best-effort delivery rather than claiming compare-and-swap from a preflight GET.
- [ ] Failure, restart and remote concurrent-edit tests preserve user text and reconcile without duplicate dispatch.

## RURU-130: Deliver provider inbox read/done actions with explicit activity policy

Planning key: C38. Group: Remote actions. Priority: Medium. State: Backlog.
Linear: [RURU-130](https://linear.app/catra/issue/RURU-130/deliver-provider-inbox-readdone-actions-with-explicit-activity-policy).
Prerequisites: [RURU-79](https://linear.app/catra/issue/RURU-79/resolve-inbox-notifications-to-cached-pr-and-issue-subjects), [RURU-117](https://linear.app/catra/issue/RURU-117/add-conflict-resolution-and-superseding-command-recovery-ui), [RURU-116](https://linear.app/catra/issue/RURU-116/project-optimistic-intent-into-local-lists-details-counts-and-search), [RURU-115](https://linear.app/catra/issue/RURU-115/implement-outbox-delivery-and-ambiguous-outcome-recovery), [RURU-124](https://linear.app/catra/issue/RURU-124/add-local-inbox-snooze-bookmark-and-disposition-state).

Add supported provider mark-read/done delivery separately from Gitru-local disposition.

Acceptance criteria:

- [ ] Model source-specific read/done semantics and use documented server fences where available.
- [ ] New activity causes skip/conflict or explicit local disposition; unfenced per-item delivery is best effort and cannot promise it consumes no newer activity.
- [ ] Offline/retry/restart tests maintain truthful counts and preserve provider capability differences; unsupported actions never dispatch.

## RURU-131: Submit comments with durable drafts and ambiguous-create handling

Planning key: C39. Group: Remote actions. Priority: Medium. State: Backlog.
Linear: [RURU-131](https://linear.app/catra/issue/RURU-131/submit-comments-with-durable-drafts-and-ambiguous-create-handling).
Prerequisites: [RURU-115](https://linear.app/catra/issue/RURU-115/implement-outbox-delivery-and-ambiguous-outcome-recovery), [RURU-99](https://linear.app/catra/issue/RURU-99/recover-private-drafts-after-disconnect-or-missing-subjects), [RURU-122](https://linear.app/catra/issue/RURU-122/cache-and-display-conversation-comments-and-activity-timelines), [RURU-117](https://linear.app/catra/issue/RURU-117/add-conflict-resolution-and-superseding-command-recovery-ui).

Expose one non-idempotent creation flow only after the delivery state machine can preserve unknown outcomes.

Acceptance criteria:

- [ ] Save drafts before send and use explicit offline-send policy where supported; normal provider comments retain canonical remote IDs.
- [ ] A lost response or crash leaves outcome-unknown unless strong endpoint evidence proves completion/non-delivery; matching text/time is not proof.
- [ ] UI preserves drafts and requires explicit reconciliation before repeating an ambiguous create; test duplicates, revocation and restart.

## RURU-132: Submit PR reviews bound to the inspected head and anchors

Planning key: C40. Group: Remote actions. Priority: Medium. State: Backlog.
Linear: [RURU-132](https://linear.app/catra/issue/RURU-132/submit-pr-reviews-bound-to-the-inspected-head-and-anchors).
Prerequisites: [RURU-115](https://linear.app/catra/issue/RURU-115/implement-outbox-delivery-and-ambiguous-outcome-recovery), [RURU-123](https://linear.app/catra/issue/RURU-123/cache-pr-review-summaries-and-review-threads-with-head-context), [RURU-117](https://linear.app/catra/issue/RURU-117/add-conflict-resolution-and-superseding-command-recovery-ui), [RURU-119](https://linear.app/catra/issue/RURU-119/add-cached-pr-changed-file-and-diff-navigation), [RURU-99](https://linear.app/catra/issue/RURU-99/recover-private-drafts-after-disconnect-or-missing-subjects).

Add online review submission and approval/request-changes with durable drafts and provider-specific anchor metadata.

Acceptance criteria:

- [ ] Submitted intent retains inspected head/base/path/line/side and provider position fields.
- [ ] A force-push or stale anchor blocks submission or presents a verified explicit remapping choice; old approvals cannot authorize a new head.
- [ ] Use current permissions and durable evidence; accepted/unknown outcomes preserve drafts and never silently duplicate submission.

## RURU-133: Add guarded online merge with asynchronous receipt reconciliation

Planning key: C41. Group: Remote actions. Priority: Medium. State: Backlog.
Linear: [RURU-133](https://linear.app/catra/issue/RURU-133/add-guarded-online-merge-with-asynchronous-receipt-reconciliation).
Prerequisites: [RURU-115](https://linear.app/catra/issue/RURU-115/implement-outbox-delivery-and-ambiguous-outcome-recovery), [RURU-77](https://linear.app/catra/issue/RURU-77/hydrate-and-render-cached-pull-request-details), [RURU-118](https://linear.app/catra/issue/RURU-118/show-cached-checks-and-commit-statuses-for-the-current-pr-head), [RURU-123](https://linear.app/catra/issue/RURU-123/cache-pr-review-summaries-and-review-threads-with-head-context), [RURU-117](https://linear.app/catra/issue/RURU-117/add-conflict-resolution-and-superseding-command-recovery-ui).

Expose merge only for providers with a verified server-side guard protecting the head the user inspected.

Acceptance criteria:

- [ ] Send and test the expected head/version guard; unsupported strict-guard providers keep merge unavailable.
- [ ] Surface current permissions, checks/approval coverage and provider merge methods without treating cached partial data as authorization.
- [ ] Auto-merge/202 responses remain accepted until confirmed; test head change, conflict, revocation, lost response and restart.

## RURU-134: Create issues through durable drafts and reconciled delivery

Planning key: C42. Group: Later remote actions. Priority: Medium. State: Backlog.
Linear: [RURU-134](https://linear.app/catra/issue/RURU-134/create-issues-through-durable-drafts-and-reconciled-delivery).
Prerequisites: [RURU-78](https://linear.app/catra/issue/RURU-78/hydrate-and-render-cached-issue-details), [RURU-117](https://linear.app/catra/issue/RURU-117/add-conflict-resolution-and-superseding-command-recovery-ui), [RURU-116](https://linear.app/catra/issue/RURU-116/project-optimistic-intent-into-local-lists-details-counts-and-search), [RURU-99](https://linear.app/catra/issue/RURU-99/recover-private-drafts-after-disconnect-or-missing-subjects), [RURU-115](https://linear.app/catra/issue/RURU-115/implement-outbox-delivery-and-ambiguous-outcome-recovery).

Add provider-capability-aware issue creation after non-idempotent recovery is proven.

Acceptance criteria:

- [ ] Preserve title/body/metadata drafts independently of authentication and restart.
- [ ] Use documented idempotency/strong receipt evidence where available; ambiguous creation requires explicit recovery before resend.
- [ ] Creation resolves its temporary local identity to a canonical provider entity and updates effective feeds without erasing drafts.

## RURU-120: Create pull requests with online branch and head validation

Planning key: C43. Group: Later remote actions. Priority: Medium. State: Backlog.
Linear: [RURU-120](https://linear.app/catra/issue/RURU-120/create-pull-requests-with-online-branch-and-head-validation).
Prerequisites: [RURU-77](https://linear.app/catra/issue/RURU-77/hydrate-and-render-cached-pull-request-details), [RURU-96](https://linear.app/catra/issue/RURU-96/link-local-git-remotes-to-collaboration-repositories-and-accounts), [RURU-115](https://linear.app/catra/issue/RURU-115/implement-outbox-delivery-and-ambiguous-outcome-recovery), [RURU-99](https://linear.app/catra/issue/RURU-99/recover-private-drafts-after-disconnect-or-missing-subjects), [RURU-117](https://linear.app/catra/issue/RURU-117/add-conflict-resolution-and-superseding-command-recovery-ui), [RURU-116](https://linear.app/catra/issue/RURU-116/project-optimistic-intent-into-local-lists-details-counts-and-search).

Add durable PR drafts and online creation without silently publishing or checking out Git branches.

Acceptance criteria:

- [ ] Map repository/account explicitly and validate source/target branches, heads and permissions online before provider creation.
- [ ] Keep local Git operations in generated Rust commands; publishing/checkouts require their explicit existing user flows.
- [ ] Non-idempotent creation ambiguity preserves the draft and receipt; successful creation resolves canonical identity and cached navigation.

## RURU-135: Assess an optional webhook relay after measuring polling limits

Planning key: C44. Group: Later infrastructure. Priority: Low. State: Backlog.
Linear: [RURU-135](https://linear.app/catra/issue/RURU-135/assess-an-optional-webhook-relay-after-measuring-polling-limits).
Prerequisites: [RURU-125](https://linear.app/catra/issue/RURU-125/measure-cached-navigation-latency-and-memory-through-native-ipc), [RURU-102](https://linear.app/catra/issue/RURU-102/add-fair-rate-budgets-and-scheduler-lifecycle-recovery).

Conditional research/decision task; build a relay only if measured budgets and product needs justify it.

Acceptance criteria:

- [ ] Document achievable polling freshness/cost and compare a small optional relay with desktop constraints.
- [ ] Specify signed, replay-protected, deduplicated event hints, privacy boundaries and polling recovery after gaps/outages.
- [ ] Keep provider accounts and all normal reads independent of Gitru cloud sign-in; no cloud deployment or provider webhook registration is part of this task.

## RURU-77: Hydrate and render cached pull request details

Planning key: C45. Group: Read experience. Priority: High. State: Backlog.
Linear: [RURU-77](https://linear.app/catra/issue/RURU-77/hydrate-and-render-cached-pull-request-details).
Prerequisites: [RURU-97](https://linear.app/catra/issue/RURU-97/add-independent-detail-scope-storage-and-hydration-contracts), [RURU-76](https://linear.app/catra/issue/RURU-76/introduce-a-provider-registry-canonical-resource-identities-and).

Narrow the existing broad PR workflow issue to the next read-only PR detail slice; comments/reviews/checks/diffs and writes have separate sibling tasks.

Acceptance criteria:

- [ ] Fetch and persist PR description, state, head/base, author and supported metadata independently of list traversal.
- [ ] Open cached details immediately and request missing/stale facets as background intent; partial, offline, oversized and permission states are clear.
- [ ] No provider SDK/HTTP runs in React query functions; adapter, IPC and UI tests cover restart, head changes and revoked scope.

## RURU-78: Hydrate and render cached issue details

Planning key: C46. Group: Read experience. Priority: High. State: Backlog.
Linear: [RURU-78](https://linear.app/catra/issue/RURU-78/hydrate-and-render-cached-issue-details).
Prerequisites: [RURU-97](https://linear.app/catra/issue/RURU-97/add-independent-detail-scope-storage-and-hydration-contracts), [RURU-76](https://linear.app/catra/issue/RURU-76/introduce-a-provider-registry-canonical-resource-identities-and).

Narrow the existing broad issue workflow issue to independent read-only details; comments and create/edit/close delivery remain sibling tasks.

Acceptance criteria:

- [ ] Fetch and persist issue body, state, author, labels, assignees, milestones and supported metadata independently of summary feeds.
- [ ] Cached details render immediately with clear partial/unavailable/oversized/offline coverage rather than waiting for provider HTTP.
- [ ] Shared local query/components work with divergent provider fixtures; pagination, restart and access-loss tests pass.

## RURU-136: Check out pull request branches through the local Git workflow

Planning key: C47. Group: Read experience. Priority: Medium. State: In Progress.
Linear: [RURU-136](https://linear.app/catra/issue/RURU-136/check-out-pull-request-branches-through-the-local-git-workflow).
Prerequisites: [RURU-77](https://linear.app/catra/issue/RURU-77/hydrate-and-render-cached-pull-request-details), [RURU-96](https://linear.app/catra/issue/RURU-96/link-local-git-remotes-to-collaboration-repositories-and-accounts).

Preserve the original PR workflow's branch-checkout requirement as a separate local Git slice rather than coupling it to provider merge or outbox delivery.

Acceptance criteria:

- [ ] Resolve the linked local clone and verified PR source/fork/head explicitly; confirm fetch or checkout intent and reuse Rust Git services/generated commands.
- [ ] Respect dirty worktrees, detached heads, existing branches and moved PR heads; inspect fetched/local OIDs before claiming the requested head is checked out.
- [ ] Do not alter remotes or expose credentials implicitly; tests cover fork PRs, stale heads, missing local objects and failed Git operations.

## RURU-137: Cache and navigate the pull request commit list

Planning key: C48. Group: Read experience. Priority: Medium. State: In Review.
Linear: [RURU-137](https://linear.app/catra/issue/RURU-137/cache-and-navigate-the-pull-request-commit-list).
Prerequisites: [RURU-77](https://linear.app/catra/issue/RURU-77/hydrate-and-render-cached-pull-request-details).

Preserve the original PR detail commit-list requirement as an independently paginated read facet.

Acceptance criteria:

- [ ] Cache provider PR commits in deterministic order with completeness and exact head/base context; changed heads supersede stale membership.
- [ ] Render commit metadata locally/offline and navigate to existing local Git commit/diff views when objects are available.
- [ ] Bound large histories and represent unavailable objects honestly; restart, paging drift and authorization tests do not fabricate Git history.

## RURU-138: Review and publish the implemented collaboration foundation

Planning key: C49. Group: Release gates. Priority: High. State: Todo.
Linear: [RURU-138](https://linear.app/catra/issue/RURU-138/review-and-publish-the-implemented-collaboration-foundation).
Prerequisites: Existing implementation; no new child blocker.

Turn the existing local implementation and architecture into a reviewable foundation PR. This publication lane can run alongside independent feature work; it does not reimplement shipped-local foundations.

Acceptance criteria:

- [ ] Review and commit the architecture, generated IPC contracts and scoped implementation on ruru/remote-collaboration while preserving unrelated work.
- [ ] Open a PR with accurate scope and local validation evidence; record and resolve actual remote CI results, including platform checks, before claiming the foundation is merge-ready.
- [ ] Update section 23 and this backlog with the reviewed commit/PR and remaining production/provider/platform gates; do not mark future features complete from fixture-only coverage.


### RURU-77 qualification checkpoint

RURU-77 is ready for draft PR publication with cached PR body/metadata, atomic
forward migration 0005 and bounded selection hydration. Local native/SDK/desktop
suites and isolated native UI/account/draft QA pass; [the work note](./collaboration-work/RURU-77.md)
records exact evidence and remaining gates. RURU-78's isolated issue mapper is
ready to integrate after this shared contract is signed. Remote CI and live
provider/production vault qualification are distinct from these local results.


### RURU-78 publication checkpoint

RURU-77 is In Review in [PR #150](https://github.com/ruru-m07/gitru/pull/150),
with remote CI running. RURU-78 is locally complete with the shared cached issue
view, supported GitHub issue detail endpoint and scoped adapter/UI tests;
[its work note](./collaboration-work/RURU-78.md) records 154 native/215 desktop
passes and credential-free native QA. It is ready for a signed draft PR on #150.
RURU-98 has started in a separate managed worktree with its lease/fairness
contract recorded before edits. Dependencies are implemented review stacks;
no issue is claimed merged or globally qualified by local checks alone.


### RURU-98 implementation start

RURU-77 is published as draft [PR #150](https://github.com/ruru-m07/gitru/pull/150)
and In Review, with local evidence recorded; its remote matrix is running.
RURU-78 is locally verified and undergoing isolated native QA. RURU-98 starts
in a separate managed worktree from the signed shared contract, using the
[approved lease/fairness plan](./collaboration-work/RURU-98.md). It keeps native
leases ephemeral, visibility caller-bound, and cadence/retry/quota authoritative;
provider HTTP remains single-owner. RURU-97's prerequisite is implemented in an
open review stack, not marked merged or Done. No duplicate PR or merge is created.


### Exact-head read-detail CI checkpoint — 3 October 2026

Signed PR [#150](https://github.com/ruru-m07/gitru/pull/150) (`7e282ad`) and
[#151](https://github.com/ruru-m07/gitru/pull/151) (`863967f`) each pass all 11
reported checks, including three-platform Rust and packaged desktop E2E. No
exact-head CodeQL check is reported; security and production provider/vault
qualification remain separate. Both remain open and unmerged. RURU-98 lease/page
fairness integration and RURU-96 native local-link integration are in progress
in isolated worktrees with saved contracts.


### RURU-98 local qualification checkpoint — 3 October 2026

Foreground demand now uses caller/session/account/scope-bound ephemeral leases,
native activity/visibility/expiry fences and one bounded SDK heartbeat. Automatic
interest creates no durable detail intent; manual Sync remains explicit. The
single native worker yields at committed pages, preserves traversal checkpoints,
rotates accounts/scopes and reserves selected-detail capacity while guaranteeing
reconciliation turns. Native cadence and persisted provider/retry/quota deadlines
control eligibility; renewals cannot reset those barriers. Same-label failed
close-disposal and slow-start readiness are fenced and tested.

Local159 native/82 SDK/223 desktop tests,6 caller cases, types/lint/Clippy/fmt,
normal103-command generation, frontend build and actual isolated two-native-tab
macOS QA pass. Post-Quit SQLite confirms zero automatic durable detail demand,
zero credentials, preserved metadata/drafts and strict cooldowns. Detailed bounds,
review findings/fixes and evidence are in [RURU-98's work note](./collaboration-work/RURU-98.md).
Signed draft publication/remote CI is the next step; live provider/vault and
R102/R103/R121 gates remain distinct. RURU-96 native links and SDK/UI navigation
continue in an isolated sibling worktree; no partial link UI is claimed complete.
No PR has been merged.


### RURU-98 integration restack checkpoint — 3 October 2026

RURU-98/PR #152 is restacked locally from signed `9046322` onto signed
RURU-78/PR #151 `863967f`. GitHub issue details and foreground Body leases now
qualify together: 170 collaboration, 6 caller, 82 SDK and 230 desktop cases pass,
including all 12 frozen migration cases. Types/lint/Clippy/formatting pass; schema
and generated wire contracts are unchanged. One inherited issue test is adapted
to verify a single bound Body lease and zero automatic durable hydration through
omission/revalidation, with all saved-value assertions retained.

Both historical master-document chronicles are preserved. The [RURU-98 work
note](./collaboration-work/RURU-98.md) distinguishes these combined local results
from all 11 reported green checks on the old published head, previous native
GUI QA and pending publication/new exact-head CI. RURU-96/RURU-79 are untouched;
no issue is marked merged or Done by the restack.

### RURU-96 implementation start

RURU-78 is published in draft [PR #151](https://github.com/ruru-m07/gitru/pull/151)
at signed `863967f`, stacked on #150. RURU-96 starts from that frozen cache/identity
contract in a separate managed worktree while RURU-98 changes scheduling.
[The approved local-link plan](./collaboration-work/RURU-96.md) requires sanitized
Rust Git observations, exact instances, durable account/local/remote identities,
access/CAS proof and real chooser/navigation/removal flows. Validation and
publication are pending; RURU-76 prerequisite is an implemented review stack.


### RURU-96 local qualification and stack integration

RURU-96 implements bounded safe Git observations, explicit account/endpoint
choices, durable authored links and transport mapping CAS, actual settings and
navigation in both directions. [Its work note](./collaboration-work/RURU-96.md)
records 172 native, nine caller, 61 SDK and 249 desktop passing tests plus real
isolated macOS clone-import/link/change/remove/mapping/restart QA. UI findings
were fixed and rebuilt; four authored drafts and strict quota barriers remain
intact with zero credential records. This synthetic qualification is separate
from remote/platform/security/live-account and schema0006 recovery gates.

The review stack is being aligned to R78 → R98 → R96 before R79 storage starts.
RURU-98 PR152 old signed head9046322 passed all11reported checks including all
three native/E2E platforms, with no exact-head CodeQL reported. Its signed local
restack onto R78 includes 170 native/82SDK/230desktop passing checks; new-head
remote CI awaits publication. No PR is merged, and generated IPC is regenerated
only after the combined native command/source contract freezes.


### RURU-96 combined review publication checkpoint — 3 October 2026

The review stack now follows R78/#151 → R98/#152 (`2d415938`) → R96. Authored
local clone/account/endpoint links, explicit transport mapping settings and cached
navigation are qualified together with the native foreground scheduler. The
[RURU-96 work note](./collaboration-work/RURU-96.md) records 188 collaboration,
11 caller, 93 SDK and 264 desktop tests, types/lint/workspace Clippy/formatting,
normal110-command generation and a fresh combined native macOS GUI check. Cold
saved routes, two duplicate-name clones, mapped SSH push actor scope and main-host
settings suspension/resume all pass. Post-Quit preserves three authored links,
one mapping, four drafts and strict cooldowns with zero credentials or automatic
durable detail intent. R106's recovery ceiling remains unchanged.

R96's signed draft publication targets #152; its own exact-head CI is pending.
At this checkpoint #152's restacked head passes reported frontend, native
formatting and all three Rust platforms plus Linux/macOS packaged E2E; Windows
packaged E2E is still running. No exact-head CodeQL is reported. No PR has been
merged. R79's reviewed parser stage remains isolated; SQL0007 may begin only once
this actual0006 ancestor is established, with strict notified-subject provenance
and explicit finite identity discovery as recorded in its approved work note.


### RURU-79 notified-subject integration — 3 October 2026

RURU-79 now implements the recorded contract atop R96/#153's actual signed
repair `0eb71a5` (inherited by signed merge `ab958df`). Forward0007 adds only
rebuildable selector/discovery state and indexed immutable-alias lookups;
previous migration bytes and R106's conservative recovery ceiling are unchanged.
A cache-only account/instance/immutable-parent/kind/number resolver opens PR/issue
Body and metadata immediately. Only current, non-denied inbox provenance grants
that exact subject independently of local repository selection. All hidden
canonical claims and contradictory immutable Native representation targets take
part in ambiguity. Mutable path/web aliases after a rename and several distinct
issue-side aliases naming one PR do not create false conflicts.

Notification membership/denial withdrawal fences pending detail/discovery and
provider projections atomically, preserving private drafts and their generations.
Existing independently selected cached detail remains readable after discovery
absence; actual discovery denial still wins. The old absence-versus-denial
regression remains unchanged. Shared item/detail/context/demand predicates and
point commit recheck the same grant rather than creating a second access policy.

Explicit identity discovery has durable finite intent (16/account,64/process,
three attempts per generation), one HTTP worker and strict persisted REST/retry
barriers. It accepts no renderer URL or native ID. PR responses prove immutable
parent via their own base repository; named-only issue responses remain typed
IdentityUnverified. Unsolicited304 cannot bootstrap identity/content. Verified
identity, aliases, bounded summary and shared Body/metadata publish in one point
transaction without feed membership, cursor, selection or mark-read writes.
Same-epoch stale responses may preserve consumed account quota only; stale
subject/error/denial/intent effects and old epochs remain fenced.

Normal112-command generation adds eight named schemas and changes no existing
schema semantics. Local evidence passes249 collaboration tests (two existing
ignored subprocess helpers), all12 frozen-v1 migration cases,11 collaboration
caller/lifetime tests (15 complete desktop-native package tests),106 SDK and287
desktop tests. Workspace all-target Clippy and native formatting pass. Fresh isolated macOS
QA and cold restart pass with six separate authored drafts, both parents
unselected, eight unchanged unread notifications, zero credentials and one
strictly paused explicit intent (attempts0). Fresh packaged macOS E2E passes all three cases; review publication and
exact-head remote CI are tracked on the linked RURU-79 and attached PR. Independent adversarial review
reproduced and repaired older-selector rollback, immutable representation
ambiguity, and a missing provider:rest subscription dependency. The shared
frontend retains canonical private draft CAS separately from historical thread
drafts and acknowledges explicit durable read receipts. Read experience never
silently selects a repository or marks a notification read.

R96 repaired head `0eb71a5` now passes all11 reported checks, including
Linux/macOS/Windows packaged E2E and all three Rust platforms; the final Windows
E2E check completed2026-10-03T12:11:26Z. R98 head `2d415938` passes all11 reported checks. Neither reports
exact-head CodeQL. R79 is locally qualified for review; live provider/vault and platform/security
qualifications remain distinct from its exact-head remote CI. No PR has
been merged. See [RURU-79's work note](./collaboration-work/RURU-79.md) for current
native proof, frontend and isolated QA evidence.


### RURU-101 qualification lane — 3 October 2026

Live dependency/PR/worktree overlap audit selects independent facet reconciliation
qualification at R79/#154 `1a51a75`, in its own attached worktree.
[R101's pre-code contract](./collaboration-work/RURU-101.md) requires a source-backed
validator/overlap/absence/head policy and independent observable regressions before
any storage correction; provider facets that are Unsupported remain so. R110's
GitLab account/repository lane uses a separate checkout. Parent #154 currently has
a real macOS Rust socket-fixture failure, which root repairs before publishing
next PRs. Local checks, exact-head CI and live provider/vault gates stay distinct.
No PR has been merged.


R101 source audit establishes that `whole_scope` means validator authority, not
full-enumeration absence. Six independent cases produce four actual gaps:
collection clock loss after304, a nullable child timestamp erasing retained-field
clock evidence, continuation representation drift qualifying absence, and current-
head checks remaining fresh after an accepted new head. Full multipage absence
and historical Reviews are green controls. The approved work note records narrow
schema-free native receipt/provenance/field-clock corrections before source edits,
with conservative legacy JSON and unchanged public DTOs/provider support.

### Next bounded lanes after RURU-79 review publication — 3 October 2026

[R79/#154](https://github.com/ruru-m07/gitru/pull/154) is published at signed
`1a51a75a0f122c1e41288a9b6b6ca09421a38025`, stacked on repaired R96/#153.
Its exact-head remote matrix is running; local249 native/106SDK/287desktop,
frozen migration12, caller11, isolated native cold restart and packaged macOS
E2E3 pass. Parent R96 and R98 each pass all11 reported checks; CodeQL/live
provider/platform recovery gates remain distinct. No PR is merged.

Live Linear, PR, worktree and file-overlap audit selects R110 GitLab.com accounts/
repositories and an independent R101 facet reconciliation qualification lane.
Both begin in attached isolated worktrees at the actual R79 head, preserving its
shared scheduler/access/cache contracts; implementation blockers are reviewed
ancestors, not silently marked Done. [R110's contract](./collaboration-work/RURU-110.md)
records direct PAT operation probes, fixed installation trust, the existing owned
vault cutover and an explicitly repository-only profile. OAuth/PKCE, enterprise,
Data Center and relay spikes stay separate. R101 must preserve conservative
qualified absence and add independent facet/validator/head/epoch evidence, not
rewrite existing paging or invent unsupported provider operations.


### RURU-110 local implementation qualification — 3 October 2026

GitLab.com manual PAT actor/member-project probes and a repository-only native
profile now use the shared owned vault journal/cutover, immutable actor/installation
identity, SQLite selection and resumable keyset feed. GitLab MR/issues/inbox/write
facets remain explicitly Unsupported; accounts remain independent of cloud.
A fixed main-only command is normally generated113, with every existing284 named
Zod schema semantically unchanged. SDK/UI add a transient manual form and accurate
provider/capability guidance. No migration changed (frozen0001..0007), and R106
restore's v1/v2 acceptance ceiling remains separate.

Independent tests/reviews drive fixes for long valid quota truncation,503/redirect
Retry-After loss, unparseable extreme deadlines, proven-actor partial-probe quota
loss and a picked worker bypassing a newly stored barrier. Quota merge is atomic
under writer/current-epoch guards, including known inactive actors without grant
reactivation; successful probe quota commits with credential promotion. Existing
GitHub CLI/login/cancel/crash/cleanup behavior and authored cache/draft boundaries
remain tested. Local qualification:285native+2existingignored,12frozenmigration,
37focusedGitLab (six independent actual local HTTP/lifecycle cases),110SDK,299
desktop (12 new GitLab cases), types/scopedBiome/frontend build, all-target
collaboration Clippy and format pass. Synthetic fixtures do not qualify a live
PAT, production vault or other platforms. Root still integrates the repaired
R79 ancestor and qualifies caller/native GUI/package before PR publication.
No PR is merged. See [R110 work note](./collaboration-work/RURU-110.md).

### Exact-head remote CI repair — 3 October 2026

Published R79/#154 `1a51a75` passes frontend/lint/types/build, Clippy, Linux
Rust and packaged Linux/macOS E2E in run37122670576; Windows is still running.
The macOS Rust job111201717043 fails two synthetic HTTP fixture cases at
notification_subject_discovery/tests.rs345 with Darwin WouldBlock35. The
nonblocking listener's flag is inherited by accepted sockets on that platform;
read timeout does not turn them blocking. The test helper now explicitly makes
the accepted socket blocking before the existing bounded read timeout. No
production HTTP behavior, deadline, assertion or CI ordering is changed.

Local correction: all16 focused notification-discovery adapter cases and all120
collaboration library cases pass (one existing subprocess-entrypoint ignored);
workspace formatting passes. Previously qualified GUI/package evidence remains
on its recorded executable; fresh remote checks qualify the repaired PR head.
Logs: `/tmp/gitru-ruru79-macos-ci-failure.log`,
`/tmp/gitru-ruru79-accepted-socket-tests.log`,
`/tmp/gitru-ruru79-accepted-socket-lib.log`,
`/tmp/gitru-ruru79-accepted-socket-fmt.log`. No PR is merged.

### RURU-110 integrated native qualification and review — 3 October 2026

Signed native source719fb63 inherits the R79 socket repair; signed picker04af1e8
adds provider-aware labels after actual same-login native QA. All285 native cases
(two existing ignored helpers), all12 frozen-v1 migration cases,15 native caller/
updater cases,110 SDK and final300 desktop cases pass. Normal113-command typegen
preserves all284 existing named schema semantics. Actual desktop/E2E types,
scoped Biome, production frontend build, workspace all-target Clippy-Dwarnings
and format pass. Fresh macOS packaged E2E passes all3 cases/two specs after the
picker correction, including real UI→Tauri→Rust→Git and child Accounts handoff.

Dedicated e2e-feature app `com.ruru.gitru.ruru110.qa` disables native vault/CLI and
uses task-only empty Git configuration. Final rebuilt executable SHA256
`7e9f5e6f6995584ef8135e53fe18b1b734cc257bb0bdcef24291820a6fb8ee76` shows
main-only manual GitLab PAT form through actual child handoff and dialog scrolling,
distinct GitHub/GitLab labels, cached nested repositories and typed unsupported PR
feed. Cold process reopen preserves GitLab-A selections2, GitLab-B0 and same-login
GitHub0. Post-Quit readonly SQLite proof preserves six native repository identities,
three private drafts generation1, zero credential refs/cleanup/automatic detail
intent, and all three strict2099 provider barriers. No token was entered/submitted;
no personal credentials/config/vault or live provider was inspected. Evidence:
`/tmp/gitru-ruru110-qa/final-post-quit-evidence.json`, final source logs and
`/tmp/gitru-ruru110-picker-packaged-e2e.log`.

Parent R79/#154 repaired e6fe69e now passes all11 reported exact-head checks,
including Rust and packaged E2E on Linux/macOS/Windows; final Windows cleanup
completed2026-10-03T13:24:08Z. No exact-head CodeQL is reported. R110's own remote
matrix starts on publication and is separate from local Mac/synthetic evidence.
Live PAT/production vault, other platforms and power-loss gates remain distinct.
No PR is merged. R101 facet qualification and R103 retained native-webview harness
continue in separate attached worktrees; final shared source integration is
sequential. See [R110 contract](./collaboration-work/RURU-110.md).

### RURU-101 integrated facet reconciliation qualification — 4 October2026

Signed source1cb9bea integrates R110/#155725b9f2 and repaired R79/e6fe69e.
Full combined native qualification completed3 October:311 cases pass, including
165 library and146 integration cases, with two existing ignored subprocess
helpers; all12 frozen-v1 migration cases and15 native caller/updater cases pass.
Workspace all-target Clippy-Dwarnings and formatting pass. The one-off initial
cold-reopen OS Busy does not recur in the combined run; weak ownership and fatal
immediate reopen assertions remain. Its original cause is not claimed resolved.

Normal make typegen regenerates113 commands; all285 existing named Zod schemas
preserve normalized initializer semantics and public DTOs stay unchanged. After
interruption, resumed4 October SDK110, actual SDK/desktop/E2E types, source binding
lint and the285-schema comparison pass. Regenerated files are generator output,
including ordering/cache metadata changes, not handwritten IPC. Native receipt,
versioned stored provenance and finite field clocks remain private to the engine;
frozen0001..0007 and R106's restore ceiling are unchanged.

Independent source-backed regressions drive retained clocks through304/null
observations, qualified full collection absence, continuation representation
restart, current-head stale/metadata conflict veto, pre-HTTP captured lease/subject
fences and exact old-job terminal cleanup. Historical Reviews and ordinary feed
paging/absence/selected-cache/private CAS remain qualified controls. Full trusted
enumeration is distinct from terminal delta/uncertain paging and whole_scope
continues to mean validator coverage. No production Comments/Reviews/Checks
adapter is enabled; future endpoint/head policies and live provider tests remain
separate rollout tasks. No UI behavior or Tauri authority is expanded.

A review PR is stacked on published R110/#155; this new head's remote CI starts
at publication. ParentR110 Rust and packaged E2E on all three platforms plus
CodeQL pass, while its Vercel API deployment status remains Pending and the
connector currently requires reauthentication. Local evidence does not replace
new-head CI, live provider/vault or power-loss qualification. No PR is merged.
R103's retained native-webview harness inherits this final source sequentially.
See [R101 work note](./collaboration-work/RURU-101.md).


### RURU-103 integrated local checkpoint and current remote evidence — 4 October 2026

R101/#156 head63e1197 passes all11 reported checks in run37185030333, including
Rust and packaged E2E on Linux, macOS and Windows. No exact-head CodeQL run is
reported. After Vercel reconnection, R110/#155 head725b9f2 deployment
`dpl_6UXbMPbqvvj1Fd6RXKGtCFFMiBdU` is READY and its build completed3 October
13:41:13Z; GitHub still reports its stale Vercel status Pending. The other14
reported checks pass. No failed attached collaboration checks or duplicate PRs
were found, and no merge is authorized.

R103 inherits both exact published heads. Full `make verify` passes locally:
477 frontend/SDK/UI tests across53 files, all repo lint/types, production desktop
build, workspace Rust tests, formatting and default all-target Clippy. This
includes28 focused frontend protocol/probe/observer cases,36 owned-process helper
cases and2 environment-isolation cases. Additional feature lanes pass19 actual
core cases and26 native app cases, with15 default native caller/updater cases;
core/app feature Clippy also passes. Normal `make typegen` produces116 commands;
independent AST comparison preserves all285 previous named Zod schemas and adds
26 fixture schemas. Production built JS contains none of the fixture bootstrap
markers. Frozen migrations and provider authority policy remain unchanged.

The finite synthetic native controller, real query/SDK probe, pinned WDIO service
adapter, owned-handle crash coordinator and separate three-platform CI lane are
checkpointed before retained execution. These local tests do not yet qualify
actual packaged shared webviews, captured IPC returns, hard-kill/restart or the
normal packaged E2E lane on this head. Collaboration DB/vault/checkpoints live
under each private run root; ordinary app/window persistence uses only the fixed
harness-ID OS namespace and is never reset. A dependency startup failure before
its child handle is registered uses the dependency's cleanup; it is not claimed
as independently proven forced-exit ownership. See the
[R103 work note](./collaboration-work/RURU-103.md) for the accepted contract.


### First retained launch and runner failure corrections — 4 October 2026

Signed checkpointdc97ef9 builds the actual macOS harness release. First native
main launch creates its real secondary view, but WDIO cuts executeAsync off at
10.002s because its HTTP timeout was10s while script/scenario bounds were190s/
160s. Later null receipts are fallout; no scenario/crash success is claimed.
Artifacts remain under `artifacts/e2e-harness/2026-10-04T08-20-28-598Z-19412`.
Root aligns the transport deadline to200s and separates outer `runner.log` from
the dependency-owned `wdio.log`, avoiding diagnostic truncation.

Installed WDIO9.31.7 also swallows ordinary service-hook errors and afterTest
rejections. The pinned launcher now promotes lifecycle failures to the actual
SevereServiceError with finite public stage/code and preserved causes. Actual
installed dispatcher regressions qualify fatal prepare/worker/complete behavior
and an ordinary swallowed control; process helper cases now total39. A finite
qualification-error receipt checked at user onComplete and the outer runner
prevents swallowed crash-evidence failures from producing green qualification.
Outer crash proof independently matches complete driver ack, phase, exact binary,
PID/session/scenario/kind and observed SIGKILL. Two actual installed-hook/proof
regressions pass. Production engine policy stays unchanged. Retained packaged
rerun and normal packaged E2E remain open gates.


### Bounded second retained receipts and isolation refinement — 4 October 2026

Second native launch31996a6 produces complete bounded receipts: actual reload/
recreation, normal public tab lifecycle and peer authority pass on macOS. Cold
concurrent demand and hint catchup fail before their first observations; delayed
local read during disconnect fails before the captured-return gate. These are
open harness qualification failures, not engine or restart success. Evidence is
retained at `artifacts/e2e-harness/2026-10-04T08-32-49-542Z-20754`. A source-backed
probe schema incorrectly names cold DetailValueState unknown instead of actual
not_loaded; the frontend owner is correcting this without changing production
DTOs/fences. Preserve NotReady/old-binding safety around phase changes.

The pinned CLI imports dotenv/config. Root forces it to a fresh task-owned empty
driver.env, preventing cwd .env reload after the inherited-environment whitelist.
An actual installed dotenv/config subprocess regression with entirely synthetic
files qualifies the empty override against an unconfigured sentinel control;
owned-process cases now total40. This change accesses no personal config. Normal
packaged E2E is being checked separately while the probe correction proceeds.


### Cold-state and React-binding regressions; normal package control — 4 October 2026

The cold-cache observation test independently reproduces the prior ZodError and
now uses generated DetailValueStateSchema, including real not_loaded. A probe
regression proves a fresh native phase with an uncommitted React binding returns
NotReady and performs zero local query calls, then reaches one real query-path
call after commit while preserving the editor. The executor retries only that
pre-IPC NotReady for its explicit gated reads; it never counts NotReady/timeout
as captured stale/cancelled success or changes native/provider authority. All30
focused frontend cases and desktop/E2E types/lint pass locally.

Separate normal packaged macOS E2E passes all3 cases across2 specs on this source:
local collaboration snapshots, real Inbox child-to-host account dialog with exact
tab restoration, and UI/Tauri/Rust/Git repository smoke flows. Actual log is
`/tmp/gitru-ruru103-normal-e2e.log`. This is distinct from retained webview/crash
qualification, whose complete corrected run and Linux/Windows CI remain pending.


### Native captured-read lock regression and bound action settlement — 4 October 2026

Third retained macOS runfede493 passes cold concurrent demand plus reload, normal
tab lifecycle and peer authority. Reverse/dropped hints and monotonic Body/dirty
editor observations also pass before a main edit receives the correct pre-IPC
NotReady after a phase update. Fixed finite edit/save/read actions now settle only
that NotReady within existing bounds; accepted mutations run once, all other
errors remain failures.32 frontend cases/types/lint pass. Precise CAS substages
will expose actual editor/save/conflict evidence in the next retained run.

Disconnect's held local Body path then exceeds190s with no result receipt. Two
source reviewers identify a self-deadlock in the new harness matcher: cloned
webviews lock the same actual Tauri ResourceTable twice before entering the Held
state/timer. A bounded real-thread regression reproduces the old timeout/exit101.
Sequential lexical pointer snapshots release both guards before current caller
revalidation, preserving exact label/resource identity/current proof. Four real
ResourceTable mutex regressions qualify same/different allocation, label mismatch,
guard drop before proof and stale proof rejection.30 native feature cases and
feature Clippy/fmt pass; no production guard or IPC signature changes.

A separate speculative core-status AuthRequired concern is rejected after source
and existing regression review: inactive Store::detail already returns empty
Unavailable evidence, and old-epoch held-response tests successfully release
through post-disconnect status and assert committed Body evidence None. No blanket
catch or core policy change is made. Actual retained proof remains incomplete;
third-run artifacts are `artifacts/e2e-harness/2026-10-04T08-41-14-989Z-23734`.


### Explicit SDK completion after query cancellation — 4 October 2026

The fourth retained macOS run at 36a5307 passes five main scenarios, including
actual withheld-read disconnect. The hint scenario qualifies reversed/dropped
hints, dirty draft preservation, an actual private-draft CAS conflict, 300 writes
with a 256-row catch-up page and 4,100 writes producing ResetRequired. Its final
obsolete-read assertion fails. Artifacts are
`artifacts/e2e-harness/2026-10-04T08-56-17-023Z-25115`; crash phases did not run.

Installed TanStack Query source and a real QueryClient regression establish that
cancelling a cached refetch with revert enabled can resolve the prior cached value.
That query completion cannot identify the held native read's final outcome.
Explicit probe reads now call the existing account-scoped SDK directly, retaining
its actual authorization fence and generated local transport. Ordinary UI hooks,
QueryClient cache behavior and independent UI snapshot observations are unchanged.
A second regression invalidates the actual singleton SDK fence during held
transport and observes the real stale authorization outcome. Bounded result
receipts now retain retention-reset and disconnect read outcomes before assertions;
only stale_view or cancelled qualify. All 35 focused frontend cases, both desktop
and E2E TypeScript checks, scoped lint and diff checks pass. Complete retained
packaged main/crash/restart and Linux/Windows qualification remain pending.


### Crash driver teardown boundary — 4 October 2026

Signed 6e87965 now passes all six retained main scenarios in the actual macOS
release binary. The retention-reset and disconnect receipts both observe real
SDK stale authorization. The first before-commit fixture issues its real native
checkpoint and the launch-owned exact process exits on SIGKILL with matching
proof. WDIO still exits 1 because killing during afterTest removes its embedded
server before normal DELETE session teardown. This run is incomplete, not a
qualified crash/restart pass; artifacts are
`artifacts/e2e-harness/2026-10-04T09-07-22-613Z-26124`.

Installed WDIO 9.31.7 and embedded Rust driver 1.4.0 source confirm DELETE removes
only a driver session map entry, leaving native views, collaboration work and
SQLite alive. The launcher now kills the exact post-health-check captured app
only at successful worker-end, before delegated native cleanup. afterTest writes
only its strict passed acknowledgment. No connection error or nonzero exit is
relabelled success. Completion requires matching crash proof and always delegates
cleanup. A monotonic 45-second window from worker start conservatively precedes
the before-commit provider gate's 60-second expiry; fresh acknowledgment is also
bounded to 15 seconds with one millisecond filesystem rounding tolerance. Failed,
missing, late, foreign or spontaneously exited workers cannot qualify a crash.
Focused tests use real owned Node children, installed dispatcher behavior and
failed/late teardown controls. Actual native crash/restart rerun remains pending.


### R103 retained local qualification completed — 4 October 2026

Final signed runner source c0e06bb, with the release application built from
6e87965, passes the entire retained macOS pipeline: six real native webview cases,
before-commit forced crash, fresh restart, committed-before-hint forced crash and
fresh restart. Both crashes carry exact launch-owned SIGKILL/observed-exit proof;
both restarts use different native PIDs/session UUIDs on the same respective
retained fixture. Before-commit reopen sees no saved Body before fresh interest;
after-commit reopen sees the exact committed Body/facet revision before interest.
Both actors retain their private drafts, with no phantom ephemeral/durable demand
or provider calls before renewed interest. Retention reset and disconnect both
carry actual SDK stale_view receipts. The default app's three packaged macOS E2E
control cases also pass separately.

Safe artifacts: `artifacts/e2e-harness/2026-10-04T09-17-51-218Z-27055`.
Application SHA256: cbc8cfa2260d8ced4910d206041ed6dfe053d9ac226bc2c3dd1fe341ffdb1537.
All five driver stages exit 0; passing owned fixture roots are cleaned. Ordinary
preferences remain in the fixed harness-ID OS namespace and are never reset.
No personal credentials, native keyring, GitHub CLI login, provider network or
Gitru cloud sign-in are used. This qualifies process crash, not power loss or
production provider/vault behavior.

Final make verify passes: 497 frontend/SDK/UI cases across 54 files, repository
lint/types/production desktop build, default workspace formatting/Clippy and
663 Rust cases (three explicitly ignored). Additional feature validation passes
330 collaboration cases (two ignored, including all 19 retained core cases),
30 native app cases and workspace all-target feature Clippy. Normal typegen has
116 commands; independent AST comparison preserves all 285 prior Zod schemas
with 26 additive fixture schemas. Default built assets contain neither the
retained executor global nor installer; generated finite schema literals can
remain shared. Linux/Windows retained packaged jobs are newly wired and still
require remote CI. No merge is authorized or performed.

R111 GitLab resource reads and R121 bounded navigation prefetch now progress in
separate attached worktrees based on published R101/#156, with signed pre-code
contracts and disjoint provider/runtime-test versus SDK/UI ownership. Their local
checks and publication are still pending; no R103 fixture code is required by them.


R103 delivery: [draft PR #157](https://github.com/ruru-m07/gitru/pull/157) is open
and attached, stacked on #156. Linear RURU-103 is In Review. The qualified source
and signed publication are retained; the initial remote matrix is running,
including the three new retained packaged jobs. No merge was performed.


### RURU-103 remote Windows and subsequent local investigation — 4 October 2026

Published5efea64 remote run37192175309 passes the new retained Linux/macOS jobs
and ordinary Rust/packaged E2E on all three platforms. Retained Windows
job111406556841 fails launcher preparation before any scenario after an actual
spawn; its safe artifact does not expose a native exception. Source audit proves
Bun/libuv canonical paths omit the Windows extended prefix that Rust retains.
The runner/helper now resolve actual filesystem identities then use consistent
namespaced Windows spelling for native roots/binaries/evidence; POSIX spelling,
symlink/type/dev/ino/environment/actual-child ownership checks remain strict.
Focused path/process/proof57 tests pass/one Windows-only case skipped locally,
E2E types and scoped lint pass. This is not a local Windows execution claim.

The first runner-only unchanged cbc8cfa macOS rerun fails concurrent-demand and
disconnect before any new provider dispatch. The SDK separately reproduces a
hidden-to-visible document transition while awaiting native event subscription;
no DOM listener exists yet and the prior visibility sample stays false. Sampling
again after listener installation fixes that source defect;113 SDK tests include
three actual held-listener visibility controls. The new bb3b7d4 macOS executable
passes concurrent-demand, catchup and reload but still fails normal-host startup,
authority counter stability and disconnect provider capture. Current receipts
cannot prove visibility as the previous run's cause or attribute legitimate
background writes to denied peer calls. Full post-fix retained qualification is
still open; historical c0e06bb qualification is not presented as new-head evidence.

Failure diagnostics now preserve finite first-error classification, separate
cleanup failure and bounded pre-cleanup actual document/native owner observations.
Normal lifecycle substages distinguish route/inventory/handshake failures. Source
review also identifies two fixture setup races: phase changes wake eligible live
jobs before the next gate is armed, and known saved Body is not evidence that a
manual foreground lease cannot refresh it. Counter assertions remain exact;
actual committed-facet warmup and zero-lease phase/gate ordering are being qualified.
No production native gate is weakened; no personal credentials/provider/vault used.
Logs /tmp/gitru-ruru103-{windows-job,path-tests,path-retained-native,
 demand-startup-tests,startup-retained-native,failure-diagnostics-tests}.log;
safe failed artifacts09-59-15-158Z-37949 and10-11-07-212Z-72795 remain task-owned.
R111/#159 and R121/#158 are separately published review slices on #156. No merge.


Source fixes frozen before the next native build: finite activation consumes
actual setter/inspector generations (false-to-true legitimately issues a newer
native generation); hidden and older-replay controls remain enforced. Final
frontend harness checks49/4 files and desktop/E2E types/lint pass. The production
SDK visibility correction passes113 SDK cases. The inherited writer-lease fix
from R111 is integrated verbatim; its331-case native qualification, deterministic
pre-fix duplicate-descriptor Busy and preserved clone/final-owner assertions are
recorded in R111. R103's feature/native package must qualify this combined source
separately. No public DTO/signature change or hand-generated IPC occurs.


### RURU-103 repaired-source local qualification — 4 October 2026

Signed combined source `9554cc3f4b4481ae03be40d8e58068d115c2ab2b` passes the
entire rebuilt macOS retained pipeline, not merely the historical executable.
All six main cases and both forced-crash/fresh-restart pairs pass; every one of
five driver stages exits zero. Before-commit SIGKILL owns PID20219 and restart
PID20281; committed-before-hint SIGKILL owns PID20335 and restart PID20392.
Each restart has its own native session UUID and retains its matching earlier
checkpoint. Before-commit data is missing until fresh actual demand; committed
Body/facet20 survives the after-commit restart before interest. Both actors keep
private draft generation1; startup remains inert until real interest.

Artifact run: `artifacts/e2e-harness/2026-10-04T10-39-36-631Z-19854`.
Rebuilt native SHA256:
`30db1b4ad5ecf07873e6552ebb43854ec398c4610c84324b3eb7b531dfb76031`.
Retention reset and disconnect observe actual SDK `stale_view`; all eight peer
operations observe permission denial with exact unchanged account, revision,
owner, provider-call and vault-access evidence, plus successful main renewal and
release. Normal public tab creation/navigation/disposal passes. Actual committed
facet warmup and zero-lease phase/gate ordering remove the identified fixture
setup races without relaxing native authority or counter assertions. Finite
pre-cleanup diagnostics retain first failure and separate cleanup outcomes;
activity inspection may register/refresh its requesting native owner and is not
claimed to be mutation-free. Earlier failed artifacts remain historical evidence.

Fresh full checks on this source pass: 522 frontend/SDK/UI tests across56 files,
one Windows-only case skipped on macOS; repository lint/types; production frontend
build; default workspace665 Rust tests/three ignored; feature collaboration332
cases/two ignored; native feature30 cases; default and feature all-target workspace
Clippy; formatting and diff checks. Default assets contain neither fixture executor
global nor installer across403 JavaScript files. The earlier normal packaged macOS
3-case control remains separately recorded, not rerun/new-source qualification.
Logs: /tmp/gitru-ruru103-final-repair-{tests,lint,types,build,default-tests,
default-clippy,native-tests,native-app-tests,native-clippy,native-fmt}.log and
/tmp/gitru-ruru103-diagnostics-retained-native.log. No new Rust command signature
or DTO changed in this repair; existing normal typegen evidence still applies.

This qualifies the combined source locally on macOS with synthetic provider/vault
fixtures. The Windows canonical-spelling repair still needs its actual remote
retained job; previous published5efea64 Linux/macOS retained successes do not
qualify the new source. Remote matrix restarts after publication. Live provider,
production keyring, other platforms and power-loss behavior remain separate.
No personal credentials or cloud account are inspected; no merge is performed.


### Windows native-path identity follow-up — 4 October 2026

Exact published #157 cce498b passes retained Linux/macOS and ordinary Windows
E2E, but retained Windows job111420764848 fails native startup before any scenario
with `Invalid native collaboration harness input`. Its owned artifact preserves
`C:\\Users\\RUNNER~1\\AppData\\Local\\Temp` in the namespaced launch root.
The job actually runs Bun1.3.0+b0a6feca; any earlier1.3.7 source assumption does
not describe this job. Bun1.3.0's default Windows realpath walker preserves
non-symlink DOS aliases; its native resolver uses uv_fs_realpath. See the exact
[JS source](https://raw.githubusercontent.com/oven-sh/bun/bun-v1.3.0/src/js/node/fs.ts)
and [native source](https://raw.githubusercontent.com/oven-sh/bun/bun-v1.3.0/src/bun.js/node/node_fs.zig).

Signed source b5154effa039c7dab3a2b0accc7d9ca114a54786 resolves actual filesystem
identity with realpathSync.native before restoring Rust's Windows namespace.
Root/type/symlink/dev/ino/environment/actual-child guards remain strict. An actual
resolver spy proves native selection; the owned Windows child regression exercises
the real short alias when supplied by the host, proves matching filesystem identity
and rejects alias spelling without signaling the child. It fabricates no DOS name
or filesystem response. Full frontend523 tests pass/one Windows-only skip on macOS,
plus full lint/types and diff checks. Logs:
/tmp/gitru-ruru103-native-realpath-{focused-tests,final-tests,final-lint,final-types}.log.
Fresh retained local qualification and exact-head remote Windows execution remain
separate gates; this source audit is not a passing Windows scenario claim.


### Real-surface and native-path repair qualification — 4 October 2026

Signed source `dbed691b7ca9880db9f6695edc393fe7defcfee7` combines the actual
Windows native resolver with private fixture window show/unminimize/focus and
bounded actual DOM-visibility preconditions. Every visibility-dependent mount and
ordinary host startup observes the real document first; disconnect observes two
actual admitted SDK leases before advancing its gate. No fabricated visibility,
production authority change, extended deadline or polling delay is introduced.
Four new executor regressions preserve the hidden/native-active distinction,
normal-host admission and actual lease ordering. Native fixture focus uses only
its exact guarded child/main windows; projection locks are released before restore.

The first freshly compiled combined run12-02-10-744Z-43546 passes authority but
fails other cases while both actual documents remain hidden, with zero SDK leases.
Cleanup succeeds; no crash stage runs. The user then confirms the desktop is
available. An unchanged-source, unchanged-binary rerun passes all six main cases
and both exact-owned SIGKILL/fresh-restart pairs, with all five driver stages zero:
`artifacts/e2e-harness/2026-10-04T12-11-00-118Z-44210`.
Native SHA256:
`d226d6c88f93e03c5abb43b1aae7de92aee81239debafbdcf88e473729f606f3`.
Before-commit owns PID44487/session23ab3d87-a91e-48f7-9fa4-d67dedffac15,
restart PID44557/sessionf29b1244-891d-4364-a173-cb4b4624b1ee; checkpoint has no
committed facet and fresh demand produces facet21. Committed-before-hint owns
PID44610/sessionb939302c-2e21-4167-9782-4151e8c71087, restart
PID44664/sessiondce54d86-0866-4bf7-9828-43c79818f958; facet20 survives with zero
provider/vault reads before interest. Both retain draft generation1. Retention and
disconnect reject actual obsolete reads; all eight peer denials preserve exact
account, revision, owner, provider and vault counters, with main lease renewal and
release successful. Ordinary tab creation, navigation, modal pause, disposal and
recreation pass. Process crashes do not qualify power loss.

The earlier hidden runs remain failure evidence. Their exact OS trigger was not
recorded and is not inferred from the successful rerun. CUA could not bind the
unbundled fixture executable; no GUI mutation or personal app was performed.
Both ordinary and fixture lanes intentionally use unbundled binaries with pinned
Tao Regular activation policy, so no packaging change is inferred from that tool
binding limitation. SDK hidden-document suspension and native admission remain
correctly enforced.

Fresh checks on this repair: frontend527 passed/one Windows-only skip across56
files; executor12/12; full repository lint/types; native feature app30/30; feature
workspace all-target Clippy with warnings denied; formatting/diff; production
frontend build. All403 default JavaScript assets exclude fixture executor/global
and installer. The earlier9554cc3 full core/default workspace tests remain
separate evidence for unchanged core, not freshly rerun counts for this repair.
No Rust signature/public DTO changed; existing normal typegen evidence applies.
Logs: /tmp/gitru-ruru103-surface-final-{tests,lint,types}.log,
/tmp/gitru-ruru103-visible-{native-app-tests,native-clippy,native-fmt,
default-build,retained-native}.log and
/tmp/gitru-ruru103-available-desktop-retained.log.

This is current-source local macOS qualification with synthetic provider/vault.
The Windows short-alias native-path fix still requires new-head retained remote
execution; old Linux/macOS successes and ordinary Windows E2E are distinct.
Live provider, production keyring, platform and power-loss claims remain outside
this evidence. No personal credentials inspected; no merge performed.


### Exact-head Windows retention-return lifetime seam — 4 October 2026

Published #1579ef7148 passes13/14 reported checks/statuses, including retained
Linux/macOS and ordinary Windows E2E. RetainedWindows job111434742147,
run37201765360, artifact2026-10-04T12-40-04-421Z-6136 now successfully launches
under actualBun1.3.0+b0a6feca5; all6 main scenarios execute,5 pass. The earlier
native short-alias setup rejection is gone on this head, but full Windows
retained qualification is still failed. NativeSHA256
`eef8f0e922525c7aae7b852cdce4d24a6c9e27122a4d88e1b8b364695c017252`.

Only hints-and-catchup fails at release of the real held local Body snapshot after
retention reset: generation6 gate2bedf9ba-a945-474c-aafd-8a3b552d4097 is TimedOut,
and release_local_read rejects its terminal state with stale_view. Both documents
are actually visible with active native owners and2SDK leases; dirty text/CAS and
real ResetRequired are already observed at revision4431. Cleanup succeeds. No
crash stages execute after main failure. Artifact has no individual control times,
so exact FillRetention duration is not claimed. Source proves the held native15s
return gate/child10s protocol currently span4100 sequential real Store::save_draft
transactions plus reset/catchup. The lifetime seam is established by source order
and terminal gate receipt, not inferred desktop availability.

Before another fixture edit, accept bounded preparation: capture the child's real
reset/cursor baseline while hints are dropped, perform the same4100 real writes
BEFORE arming or starting its held Body read, then verify that actual child reset
count/cursor have not advanced and remain behind the pruned native revision. Arm
and capture the real SDK read under its unchanged pre-reset fence, observe Held,
then wake the actual bridge, require ResetRequired and all dirty/CAS/cache controls,
release within the existing bounds, and require actual stale_view/cancelled. The
native query data revision may already equal the filled revision; the tested old
property is the SDK authorization generation before ResetRequired, not older data.
No fabrication, pre-accepted stale outcome, timeout increase, retry, reduced write
count, retention threshold, permission or production SDK change. If the child
already caught up during preparation, fail rather than claim a pre-reset fence.
Meaningful held-preparation/early-catchup controls and fresh combined retained
execution must qualify this next fixture change; current historical passes do not.


R1035376f3b fresh retained Mac run rejects pre-arm catchup correctly: child
cursor331→4431/reset0→1 despite nativeDrop filtering, both actual documentsvisible.
Pinned Tauri catch-all EventTarget::Any receives main-targeted events too. The
[signed bounded contract](./collaboration-work/RURU-103.md) permits an ordinary
SDK own-Webview collaboration listener, preserving global broadcasts and all
real catchup/reset/fence/deadline controls. No repair is remotely qualified yet.


### RURU-103 revision target and retention repair qualified — 4 October 2026

Signed201005db corrects ordinary SDK revision listener to actualWebview scope
while preserving intentional global broadcasts. Wire regression is RED→GREEN;
535 frontend tests/one platformskip, lint/types and production build pass. Fresh
serialized retained Mac binary13070937 passes all6 main scenarios and both owned
SIGKILL/fresh-session restart pairs (all5 stagesexit0). Actual childcache22/28
stays unchanged until reverse/publicwake; real retention331→4431 ResetRequired
fences a genuinely held return as stale_view while retaining dirty/CAS state.
Full evidence/previous failure boundaries are in the [R103 work note](./collaboration-work/RURU-103.md).
Publishing to existing #157; new exact-head remote CI remains pending separately
from these local results. Previous9ef7148 retainedWindows failure is not labeled
a new-head Windows pass. No merge or live credentials.

### RURU-111 GitLab MR/issue reads qualified locally — 4 October 2026

GitLab.com selected projects now synchronize all-state MR and issue summaries and
independent singleton Body/common metadata through the existing native scheduler,
SQLite, provider traits and ordinary shared detail UI. Immutable global IDs remain
separate from project IIDs; numeric routes and bounded account/epoch/project/kind
cursors preserve identity across renamed display paths and copied issue moves.
Unconditional MR creation-order offset and issue ID keyset traversals fail partial
on malformed/replayed/cross-scope paging; neither is claimed an atomic snapshot.
List clocks do not validate Body. Omitted/null/empty/oversized descriptions and
missing asynchronous MR diff references retain explicit states. GitLab inbox,
discussions, approval/check facets, remote writes and self-hosted instances remain
unsupported. No migration, public DTO or generated IPC signature changes occur.

Local evidence: full collaboration329 passed/two ignored, workspace all-target
Clippy-Dwarnings and format, complete frontend413 tests/48 files, repository lint,
types including desktop/E2E/testing and production frontend build pass. Shared
GitLab MR/issue UI tests preserve private drafts and use ephemeral detail demand
without durable hydration. Actual synthetic HTTP-to-Runtime-to-SQLite regressions
cover inert cold reopen, copied identities, concealed404 and late old-epoch cache/
quota veto. No personal credentials, live provider or production vault was used.
The review PR is stacked on #156; its remote platform checks qualify its own head.
No PR is merged. See [R111 work note](./collaboration-work/RURU-111.md).

Vercel reauthentication is resolved: #155's exact725b9f2 deployment
`dpl_6UXbMPbqvvj1Fd6RXKGtCFFMiBdU` is READY with build completed3 October13:41:13Z,
although its GitHub check still reports Pending. No check override or redeploy is
claimed. #156's reported11 checks pass; no exact-head CodeQL is reported there.


Final independent review found an async diff-reference missingness edge. Base now
requires both the MR SHA and diff_refs.head_sha to be known, nonempty and equal
before accepting start_sha. Missing, null, empty and mismatched heads leave Base
Omitted while independently qualified Body/title/state remain available. Eight
actual HTTP shapes preserve the known matching-head target-start control. After
this correction the full collaboration suite still passes329/two ignored,
focused HTTP14/14, workspace all-target Clippy-Dwarnings and formatting pass.
Logs: /tmp/gitru-ruru111-async-head-{http,full,clippy,fmt-check}.log.


### RURU-111 inherited Linux immediate-reopen repair — 4 October 2026

Published5eab038 run37194446146 passes frontend/Clippy and macOS/Windows Rust;
Linux job111413272739 fails the unchanged demand cold-reopen test at1228 with
Busy immediately after close/drop. The same earlier one-off R101 boundary is now
an actual repeated failure; it is not declared fixed by isolated local success.

A deterministic actual-Store regression keeps a duplicated Unix file descriptor:
reader close and a surviving Store clone remain Busy; after weak Inner proves all
owners gone, the old raw File still blocks immediate reopen. A raw OS control
confirms flock remains attached to the shared open file description across dup/
fork until explicitly unlocked or all duplicates close. The exact process that
may have inherited a descriptor in remote CI is not instrumented or claimed.

A private WriterLease now explicitly unlocks only on final Inner destruction,
with its field after writer/readers so their handles drop first. The stable lock
inode, clone/close exclusivity, crash OS release and bootstrap failure/cancellation
ownership stay intact. No early close unlock, sleeps, retry masking, schema/public
API or original demand-test assertion changes occur. New tests also prove closing
the old duplicate cannot release the reopened owner's independent lock.

Focused raw/Store controls2/2, unchanged demand restart1/1 and existing ownership
case1/1 pass. Full collaboration331/two ignored, workspace all-target
Clippy-Dwarnings and formatting pass. Pre-fix red and final green logs:
/tmp/gitru-ruru111-writer-lease-{baseline,red,green,demand,ownership,full,clippy,
 fmt-check}.log. A new signed #159 head must qualify its own remote platforms;
old failing CI is retained as history. The same private fix is integrated into
R103 before its next retained native build. R106 coordinated shutdown/current-
schema backup qualification remains open. No PR is merged.


### RURU-112 bounded Bitbucket Cloud account slice started — 4 October 2026

Live Linear/PR/worktree/dependency audit selected R112 after R111/#159 repaired
`ebb8ae1` passes its exact Linux Rust job; its Windows packaged check remains
separate. Reviewed R76/R100 prerequisites are unmerged. An isolated attached
worktree on that head now owns the first repository-only account slice, with
native adapter/runtime/test and SDK/UI work in parallel under disjoint paths.
Signed pre-code contracts ee0edda/e9bd990/f1d1a14 live in
[the R112 work note](./collaboration-work/RURU-112.md). R112 remains In Progress:
PR summaries/Body and explicit native participant/task facets require later chunks.

Current official Bitbucket sources confirm token-only Bearer API tokens and the
removed native issue/app-password endpoints. The proposed fixed public connection
uses existing native vault/actor/epoch cutover, no Gitru cloud or ambient credential
lookup; only repository capability is supported in this slice. Workspace-to-member-
repository opaque continuation state is account/epoch bound and bounded. Shared
per-job page limits merely yield, so this private cursor persists continuation
fingerprints and a20 accepted-page/20workspace limit across resumption/reopen.
Exhaustion remains Partial without inferred absence; capped large accounts need a
later explicit restart/coverage policy. Response-detected loops reject the page;
bounded retries may refetch its current URL but never follow the repeated target.
Username-only clone metadata is validated then discarded, not executed/persisted.

At the frozen additive main-only command signature, normal make typegen passes114
commands. Independent AST comparison preserves all285 prior schemas and adds only
Bitbucket connect params. Adapter/UI implementation and qualification are underway;
this record does not claim live provider, platform, completed R112 or publication.
R103's repaired source is separately published in #157 atcce498b with fresh macOS
retained proof and exact-head remote CI running; R121/#158 and R111/#159 stay review
stacks. No merge is authorized or performed.

### RURU-112 first account/repository slice qualified — 4 October 2026

Signed source4dad563 implements manual token-only Bitbucket Cloud accounts and
UUID member-repository discovery in the existing native vault/SQLite/scheduler,
generated SDK command and ordinary account dialog/repository picker. Repositories
alone are supported; PR/Body and explicit participants/tasks remain required and
R112 stays In Progress. Durable20 accepted-page/20workspace/fingerprint caps retain
Partial coverage/historical authorized rows across cold/manual resume; a capped
large account still needs a later explicit restart/coverage policy.

Actual HTTP17 plus Runtime/SQLite7 focused cases pass24; full default workspace706
top-level tests/three ignored passes, including355 collaboration and native caller
policy. Clippy/fmt and fresh24 after the equivalent style cleanup pass. Frontend431
tests/50files, lint/types and production build pass. Normal typegen114 preserves
all285 prior schemas,237 aliases,113 commands/events and adds only token connect.
Known-actor invalid-presentation quota regression is recorded red/green without
credential staging/cache replacement. Full evidence and platform/live limitations
are in [R112's work note](./collaboration-work/RURU-112.md). Publication and exact-
head remote qualification remain separate. No live credentials or merge.


### RURU-112 second bounded slice contract — 4 October 2026

First account/repository draft #160 exact968cd524 passes all11 reported checks,
including all-platform Rust/ordinary packaged E2E. R112 stays In Progress: PR and
explicit participant/task facets remain. The next isolated stack accepts the
[PR-summary/Body contract](./collaboration-work/RURU-112-pulls.md) before code.
Compound repoUUID/local PR identity, exact all-state opaque pagination, durable20
page/history bound, singleton rendered.description.raw authority, full-OID refs,
and typed unsupported differences use the existing Runtime/SQLite/SDK. The stable
UUID repository child route is explicitly a documentation inference until live
qualification; no provider credentials are read. No migration/scheduler/SDK
production change is planned. R103/#157 latest9ef7148 repair is separately pushed
with local all5 retained stages passing; its new remote matrix runs independently.
No merge. First #160 remains unchanged while the new reviewable slice is prepared.


R112 PR-read integration first49-case native run:45 passed,4 failed. Two actual
HTTP singleton failures expose lost account cooldown during detail error conversion;
[the signed work contract](./collaboration-work/RURU-112-pulls.md) now permits a
bounded existing-barrier correction in runtime/details.rs. Original red quota
assertions stay intact; old-epoch data/quota fencing remains required. Two fixture
assumptions were corrected to actual cold-admission/historical-coverage semantics.
Fresh post-fix qualification is pending. #1579ef7148 separately passes13/14 remote
checks; retainedWindows now starts and passes5/6 main scenarios but fails actual
retention-reset stale_view. Owned artifacts are under investigation; not qualified.


### RURU-112 PR reads qualified locally — 4 October 2026

Signed source6892db97 implements Bitbucket all-state PR summaries and singleton
raw Body/common metadata through existing local projections. The independent
detail-error cooldown regression is repaired through captured account/epoch
barriers, with both original red assertions green.49/49 focused native cases and
731 full default workspace tests/three ignored pass; all-target Clippy/fmt pass.
Frontend438/51files, lint/types and production build pass. Full evidence and
coverage/routing/platform limits are in the [PR-read work note](./collaboration-work/RURU-112-pulls.md).
A separate draft stack on #160 is ready for publication; remote exact-head CI is
pending separately. R112 remains In Progress for explicit participant/task facets.
#160968cd524 and #159ebb8ae1 each pass all11 reported checks. #1579ef7148 remains
13/14 with retained Windows failing a bounded test-lifetime seam; its isolated
fixture repair and fresh native qualification are underway. No merge.


### RURU-112 participant-only contract accepted — 4 October 2026

The [participant contract](./collaboration-work/RURU-112-participants.md) is accepted
before source changes in an isolated managed stack on #1611766a6e. Common typed
ParticipantV1 preserves native role/approval/state/action-date facts, independent
field masks/clocks and immutable identity; no review/readiness overloading. A
forward0008 rebuild preserves all four dependent detail tables with FKON and
frozen-v7 upgrade/rollback/cold evidence. The panel acquires ordinary local query
and native interest only when opened and supported. Tasks are a later bounded
chunk; R112 stays In Progress. R1034054a6c is separately published after fresh
all5 retained Mac stages pass; both new exact-head remote matrices remain pending.
No source/IPC/migration qualification or live credentials/merge is claimed yet.


Participant native/storage qualification passes28 focused and759 full default
workspace top-level tests/3 ignored, with all-target Clippy/fmt. Independent wire
review found an impossible native:null/participant-field test. The accepted
[wire guard correction](./collaboration-work/RURU-112-participants.md) derives the
family from Rust, rejects payload/mask/validation mixes and uses a real generic
null control; installed generated wire qualification will rerun before delivery.
Separately #1611766a6e passes all11 reported remote checks and #1574054a6c all14,
including retained Linux/macOS/Windows. No exact-head CodeQL pass is inferred.
Participant publication/new-head platform CI remain pending; no merge.


The participant wire-family correction now passes all13 installed wire tests
from the original four red controls;453 full frontend tests pass. Final types/
build exposed a pinned-generator short-name collision between private Bitbucket
discovery Branch and public Git Branch. The accepted work note limits repair to
a private DTO rename, fresh normal generation/native checks and full types/build;
no generated hand edits or dropped public Git properties. Publication waits.


### RURU-112 typed participant slice qualified locally — 4 October 2026

Signed source63d9c67 implements the common ParticipantV1 facet, Bitbucket singleton
mapper, per-field saved evidence/identity fences, FK-preserving0008/frozen-v7
recovery and opened-panel-only ordinary SDK/native interest.28 focused native
cases and759 full default workspace top-level tests/3 ignored pass; full run
preceded an equivalent private DTO rename with fresh64 Bitbucket cases afterward.
Final all-target Clippy/fmt, normal typegen114,453 frontend/52files, lint/types/
production build pass. Source-derived tag/dependency/null and field-family guards
qualify original red wire controls; private BitbucketDefaultBranch avoids a real
pinned-generator public Git Branch collision without changing wire fields.
Full qualification, reviews and limits are in the
[participant work note](./collaboration-work/RURU-112-participants.md). A separate
draft stack on #161 is ready; its new-head remote CI remains independent/pending.
R112 stays In Progress for Tasks; R106 restore ceiling and R121/R103 owners remain
separate. #1611766a6e passes all11 reported remote checks; #1574054a6c all14
including retained Linux/macOS/Windows. No unreported exact-head CodeQL/live-token/
platform vault qualification or merge is inferred.


### RURU-112 Tasks contract accepted before code — 4 October 2026

The [Tasks contract](./collaboration-work/RURU-112-tasks.md) defines the fourth
bounded slice on attached #162 exactd2e9195. Independent provider/native reviews
qualify the proposed approach before source: typed task.v1/TaskActor, twelve
disjoint Task fields with generic/Participants still six, required own-task clocks,
resolver identity before presentation, bounded opaque continuation and stable
multi-page Uncertain. Only an initial valid page without next can declare complete
within the observation; caps preserve Partial. FK-enabled0009/frozen-v8 recovery
must preserve old participant/generic JSON, eight ledger rows and authored intent.
Opened supported panel only, bounded local pagination and ordinary native demand;
no writes/readiness/provider branches or R106 restore-policy change. R112 remains
In Progress. Models/Store/migration, provider/HTTP/Runtime and frontend have clear
file owners; root owns generated IPC/docs/serialized validation. No implementation
qualification, new Task PR, live credentials or merge is claimed.


### RURU-112 Tasks locally qualified — 5 October 2026

The fourth bounded slice implements typed task.v1/TaskActor/twelve disjoint Task
fields through existing native HTTP/Runtime/SQLite, source-generated SDK and the
common collapsed/opened-only Tasks panel. Own-task clocks and resolver context
preserve historical content, false/null/omission authority; stable multipage
Uncertain and durable20-page history survive ten-page yield/cold/manual resume.
Foreign-key-enabled0009 plus independent frozen-v8 controls preserve historical
rows/ledger and rollback/draft CAS; original0001–0008 and R106 archive policy stay
unchanged. Local50-row/100-cursor browsing and privacy cuts preserve private drafts.

Focused40 native controls and full default Rust workspace794 top-level/3ignored
(including collaboration443/2ignored), Clippy/fmt/diff pass. Frontend473/54files,
full lint/types/prodbuild pass. Normal typegen114 preserves289 schemas/241aliases
plus only2 new Task types,114 command functions/1event/publicGit Branch fields.
Actual cap-fixture generation and Clippy guard findings were corrected, without
weakening production fences or original controls. See
[the Tasks work note](./collaboration-work/RURU-112-tasks.md) for commands and
qualification limits. Signed stacked publication/its exact-head remote CI remain
separate; live provider/vault/new native UI/CodeQL is not inferred. No merge.


### RURU-112 publication and RURU-122 first contract — 5 October 2026

Attached draft [Tasks#163](https://github.com/ruru-m07/gitru/pull/163) exact signed
2e4b8d5c57e5dd0bb7dd0cbef5e4f521a870e588 is stacked on#162; R112 is In Review
with all four bounded read-only slices published. Its new remote matrix is running,
separate from local794Rust/473frontend and ancestor checks. No merge.

Live R122/77/78/PR/worktree/file-overlap audit finds no duplicate conversation PR
and implemented reviewed prerequisites in ancestry. New isolated managed
ruru/ruru-122-conversation-comments owns the first GitHub PR/issue Comments slice.
[Signed pre-code contract](./collaboration-work/RURU-122-comments.md) uses existing
generic entries/DTO/schema with own-comment clocks, immutable numeric repository
addressing, strict no-redirect/no304/all-Link boundary, operational50-row pages,
durable20-page progression, singletonFull versus stable multipageUncertain, and
opened-only revision-fenced local paging/singular native demand. Independent
provider/schema and UI reviews completed before source. Anonymous public routing
probe is distinct from private provider/token/vault/platform proof. Root serializes
validation/generated IPC/publishing; provider, Runtime and UI have disjoint files.
R122 stays In Progress for this slice and later activity/provider coverage; no
comment implementation, new remote CI or production qualification is claimed yet.


### Tasks exact-head remote matrix qualified — 5 October 2026

Attached Tasks draft#163 exact signed2e4b8d5c57e5dd0bb7dd0cbef5e4f521a870e588
passes all11 reported checks. [CI37268340457](https://github.com/ruru-m07/gitru/actions/runs/37268340457)
passes frontend/lint/types/build, formatting/Clippy and Rust plus packaged E2E
on Linux/macOS/Windows; final Windows E2E completed06:02:57UTC. Cloudflare,
CodeRabbit and Vercel also pass. No exact-head CodeQL is reported. Local794Rust/
473frontend remains separate from live private-provider/vault/new-panel native
window or latency proof. R112 stays In Review and draft remains unmerged.


### RURU-122 first Comments slice locally qualified — 5 October 2026

Read-only GitHub PR/issue conversations now use native immutable collection
addressing, own-comment clocks and the existing generic Comments storage/schema.
Strict all-Link/noRedirect/no304 admission, bounded50/20page continuation and
stable multipageUncertain prevent invalid absence; cold/yield/explicit-retry caps
persist. Common opened-only raw-text/local50row/100cursor/gc0 paging, revision and
privacy fences preserve private Body draft/CAS and singular demand. Root also
fixes a demonstrated held200 account-quota loss before Store/reconciliation
validation: actual RED→GREEN proves same-epoch120s durable barrier/cold sibling
zeroHTTP/vault, with obsolete-epoch200/429 controls intact.

Final serialized make verify exits0:815 Rust passed/3ignored (collaboration464/
2ignored),499 frontend/55files, lint/types/prodbuild and all-target Clippy/fmt.
21 focused native and45 new+legacy Workspace controls pass. Normal typegen114
preserves291schemas/243aliases/114functions/1event/Branch with zero public changes;
generator-only timestamp/order churn restored, no hand edits. Independent reviews,
real fixture/old-control corrections and qualification limits are in the
[Comments work note](./collaboration-work/RURU-122-comments.md). Signed publication/
new-head remote CI remain separate next gates; no private-provider/vault/new UI
native/latency proof, activity timeline/provider expansion or merge is inferred.
R122 remains In Progress. R104 destructive retention awaits separate R99 recovery
integration; bounded R102 independent-clock lifecycle audit is the next candidate.

### RURU-103 exact-head remote and RURU-99 current-stack local qualification — 5 October 2026

Draft [RURU-103 #157](https://github.com/ruru-m07/gitru/pull/157) exact signed
`ba45cbda093b4b178a0937441c1718691ce92e60` passes all 14 reported checks in
[run 37281997503](https://github.com/ruru-m07/gitru/actions/runs/37281997503),
including Rust, ordinary packaged E2E and the collaboration harness on Linux,
macOS and Windows. No CodeQL check is reported; RURU-103 remains In Review and
unmerged.

RURU-99 #144 is integrated locally on that head. Typegen emits 119 commands, 322
schemas, 262 aliases and one event with no RURU-103 declaration removed or changed.
Independent final review found and bounded one product defect: ordinary item and
notification drafts still used the old save-only editor. Signed repair routes all
surfaces through shared Copy/Export behavior and adds a generation-bound ordinary-
detail regression.

Fresh post-repair gates pass 645 frontend/one platform skip, 831 Rust/three
ignored, lint/types/build/format/Clippy, 495 feature collaboration/two ignored and
34 feature-native tests. Ordinary packaged E2E passes three tests in
`2026-10-05T09-23-07-938Z-94115`; the complete five-stage retained run passes in
`2026-10-05T09-25-00-937Z-96300` with binary SHA-256
`b7278812f63e7173d374ae30cb77880728da0912d61929e40424bfe489414465`.
Current native export automation uses synthetic drafts/owned temporary paths; old
actual-dialog QA remains ancestor evidence. The integrated implementation was
published without rewriting history as signed exact head
`b39b55f501e2207cea8e39de77fc267eb465ede0`, and #144 now targets RURU-103.
[Run 37291579988](https://github.com/ruru-m07/gitru/actions/runs/37291579988)
passes all 11 Actions jobs across Linux, macOS and Windows plus Cloudflare, Vercel
and CodeRabbit, for 14 reported green checks; no CodeQL check is reported. This
evidence-only record needs its own final exact-head matrix before RURU-104 starts;
RURU-99 remains draft/unmerged and no merge is authorized.


### Comments publication and RURU-102 clock contract — 5 October 2026

Attached draft[Comments#164](https://github.com/ruru-m07/gitru/pull/164) exact signed
aab107dacf11e67216de602e42954a61e46c4423 is stacked on#163. Final local make verify
passes815Rust/3ignored and499frontend plus lint/types/build/Clippy/fmt; its new CI
is running separately with no failures. R122 stays In Progress for later activity/
provider criteria. No merge or live private-provider/vault/new native UI proof.

Fresh liveR102/R104/prerequisite/PR/worktree/ancestry audit selects a bounded R102
independent-clock lifecycle slice on that exact frozen head. R98/R76 are inherited;
R104's R99 recovery/export is a separate fork and absent, so destructive eviction
is deferred. The[pre-code clock contract](./collaboration-work/RURU-102-clock-lifecycle.md)
requires RED before source-risk repair, accepted-epoch live monotonic plus full
persisted budgets, actual held dispatch boundaries, peer/reconnect/cold/long-wait
controls and zero public contract changes. Real OS suspend/invalid-clock restart
and separateR103 own-Webview integration are not claimed. Root serializes gates;
production Runtime and new fixture have distinct file owners. No R102 source or
qualification exists yet; review/sign before implementation.


R102 pre-code peer review accepts serialized Store-epoch/live-map publication and
both native pre-vault/final pre-HTTP checks; private per-dispatch local refusal
classification preserves original quota instead of inventing observations after
a clock jump. One existing account map slot, narrow three Job initialization
seams, real rejected-probe lifecycle and explicit post-vault boundary controls;
no public DTO/schema or fairness-policy changes. Signed contract precedes fixture
RED/production repair, and no clock defect/qualification is claimed before execution.


### Comments exact-head remote matrix qualified — 5 October 2026

Attached draft [Comments #164](https://github.com/ruru-m07/gitru/pull/164) at signed
`aab107dacf11e67216de602e42954a61e46c4423` passes all 11 reported checks.
[CI run 37272166940](https://github.com/ruru-m07/gitru/actions/runs/37272166940)
passes frontend tests/lint/types/build, formatting/Clippy, Rust tests and packaged
E2E on Linux/macOS/Windows; final Windows E2E completed 06:54:17 UTC. Cloudflare,
CodeRabbit and Vercel also pass. No exact-head CodeQL is reported. Local 815 Rust/
499 frontend remains separate from live private-provider/PAT/keyring, new Comments
native-window behavior or latency. R122 stays In Progress for activity/provider
criteria; draft remains unmerged.


### RURU-102 bounded clock lifecycle locally qualified — 5 October 2026

Actual independent-clock Runtime RED reproduced five failures out of ten before
production repair. Accepted-epoch quota commit/live max-install now serialize
with feed/detail dispatch checks; successful probes capture before vault cutover
awaits, and local refusal preserves the original provider observation. Strengthened
receipt-time capture plus both held feed/Body boundaries pass all ten controls.
One final serialized make verify exits0:825 Rust/3ignored (collaboration474/2),
499 frontend/55files, full lint/types/build/Clippy/fmt; desktop build is a valid
Turbo cache hit. Credential crashes/rollback, fairness, long persisted budgets
and Comments/privacy controls pass unchanged. Normal typegen114 plus independent
AST inventory preserves291schemas/243aliases/114functions/1event/publicBranch
with zero public changes; generator-only churn restored, no hand edits.

See the [clock lifecycle work note](./collaboration-work/RURU-102-clock-lifecycle.md)
for actual RED/GREEN commands, source boundaries and limits. R102 remains In
Progress: notification discovery, broader family scheduling and clock-jump immunity
above24h are separate. No new-head remote CI, OS suspend/live provider/platform
vault/new native UI or merge qualification is inferred. R99 recovery and R103
own-Webview/native integration remain separate forks to integrate explicitly.


### Current-stack RURU-103 integration starts — 5 October 2026

Attached draft [R102#165](https://github.com/ruru-m07/gitru/pull/165) now publishes
exact signed36455f9fa72b46d7dd1c58e8d3368e08b598fc34 on Comments#164; its new CI
is running separately from local825Rust/499frontend and ancestor164's11/11.
R102 remains In Progress for broader criteria.

Existing attached [R103#157](https://github.com/ruru-m07/gitru/pull/157) is draft
and unmerged at4054a6c081aef47aeace489e01cc8c2cdcae1128, with all14 reported
checks successful including retained and ordinary packaged E2E on three platforms.
Live Linear remains In Review. Its own-Webview event listener and retained native
lane were a separate fork; the signed
[integration contract](./collaboration-work/RURU-103-stack-integration.md)
now advances that existing PR onto exact165 through a signed local integration
commit preserving published history. Both implementation/evidence histories above
are retained. Textual merge/old14checks do not qualify the combined tree. Normal
generation, independent AST/source review, default/feature/full/native retained
checks and a fresh exact-head remote matrix are required before qualification.
No credentials inspected or PR merged; destructive R104 still awaits R99 recovery.

### R102 remote fixture timing correction — 5 October 2026

Published165 exact36455f9 remote frontend job111653744289 fails one of499 tests
on an immediate Body visibility assertion after only observing Comments close
during same-actor epoch cutover. Actual lint/types/build independently pass. A
controlled replacement Body local-read gate proves old Body remains fenced while
waiting, dirty draft survives, and new receipt restores Body; old Comments rejection/
eviction and demand/no-hydration checks remain. Test-only source correction passes
focused1control, full499frontend/55files, lint/types. No production/native/generation/
bundle source changed; earlier825Rust and build gates remain separately attributed.
Fresh signed head/remote matrix is next; failed36455f9 is not labeled green.


### R102 remote qualification and R103 current-stack local qualification — 5 October 2026

R102 draft [#165](https://github.com/ruru-m07/gitru/pull/165) exact signed
`d08fa56c2bf0399b7f1a1fe5fd3ecbd03f9e1fc4` passes all 11 reported checks in
[CI run 37278725328](https://github.com/ruru-m07/gitru/actions/runs/37278725328),
including frontend, Rust and packaged E2E on Linux/macOS/Windows. No exact-head
CodeQL check is reported. Keep R102 In Progress and unmerged because notification
discovery, broader family scheduling and clock-jump work remain separate criteria.

R103 draft [#157](https://github.com/ruru-m07/gitru/pull/157) is locally integrated
on that head at signed `8c8068b5e2c7022477c80a31f0667903b9ba1912`.
Normal typegen emits 117 commands, 317 schemas, 257 aliases and one event, preserving
the R102 public contract. The combined tree passes default `make verify` with
825 Rust/3 ignored and 630 frontend/one Windows-only skip, all lint/types/build/
format/Clippy gates, feature tests 493 collaboration/2 ignored plus 30 native app
tests and feature Clippy, ordinary packaged E2E, and every stage of a fresh retained
crash/restart run. The exact evidence and source-generator ordering repair are in
the [stack integration note](./collaboration-work/RURU-103-stack-integration.md).

Next: sign the result documentation, ordinary-push the existing R103 branch,
change its base to R102, update its review text and run a new exact-head remote
matrix. The original R103 ancestor's 14/14 checks do not qualify this head. Keep
R103 In Review and unmerged; no live-provider, personal credential, keyring,
other-platform or CodeQL result is inferred. Integrate R99 recovery/export before
starting destructive R104 retention.

### RURU-136 pull checkout published for review — 7 October 2026

Draft [#167](https://github.com/ruru-m07/gitru/pull/167) publishes the provider-
independent local checkout flow at signed source/evidence head
`39dc8d7a4bd69efff999479f6a46cb29a05cac37`, stacked on exact signed RURU-104
head `44a94ee69e1d739b55adfdfec666dbacf89e266c`. The native boundary resolves a
linked clone, binds the cached provider OID and credential-free endpoint, fetches
without rewriting remotes when needed, serializes an existing branch with a
prepared Git ref transaction, and verifies branch/OID before reporting success.

Final local `make verify` passes 661 frontend/SDK/UI tests with one platform skip,
891 Rust tests with four ignored, lint, types, production build, format and
workspace Clippy. Packaged macOS E2E passes both specs and all three scenarios;
the focused checkout integration passes 26/26. Exact-head remote CI, Windows,
live-provider and real credential-manager behavior remain pending and separate.
RURU-136 stays In Progress and draft/unmerged.

### RURU-137 cached pull commits published for review — 7 October 2026

Attached draft [#168](https://github.com/ruru-m07/gitru/pull/168) publishes signed
source `8b45143cf54bfc5bea728c6155ed36496c25b5b2`, stacked on exact RURU-136 head
`9548c53cf3c926a47e474d8d9b9511bc41e99d41`; that base's 14 reported checks
all pass. The implementation supplies exact-context, atomically published commit
generations for GitHub, GitLab and Bitbucket Cloud, local-only PR rendering and
exact-object navigation through linked target or source clones without implicit
fetch.

Final `make verify`, 123-command generation, feature-native checks and the real
packaged five-process crash/restart harness pass locally. The independent restart
reads the exact complete two-row snapshot before new interest with zero provider
and vault access. [The work note](./collaboration-work/RURU-137.md) records the
scope and evidence. RURU-137 is In Review; PR #168 remains draft/unmerged and its
exact-head remote matrix plus live private-provider/PAT/keyring and other-platform
behavior remain separate gates.


### RURU-124 local inbox state locally qualified — 8 October 2026

RURU-124 is In Progress from exact signed RURU-137 final head
`0da82cc7e5a3543512be683e68ac074f8d9689cd`; its sole declared prerequisite
RURU-79 remains open in draft #154 and is an ancestor of this source. The signed
[pre-code and implementation record](./collaboration-work/RURU-124.md) defines
and implements provider-independent local inbox disposition, bookmark and
bounded UTC snooze state in SQLite. Provider unread/read or pending/done fields
remain immutable and no adapter, vault, Gitru cloud or provider-write path is
used. Explicit mutation kinds keep bookmarks independent; newer accepted
provider activity re-surfaces done or snoozed rows even when remotely read.

Local qualification passes normal 125-command type generation, every default
collaboration Rust suite, warning-denied collaboration/native Clippy, 27 native
caller-policy tests, 156 collaboration-client tests and 237 desktop
collaboration tests plus their type/lint gates, Rust formatting and diff checks.
Focused controls cover cold restart, account/access/CAS isolation, all filters,
literal search and stable cursors, exact snooze expiry, cache clear and stale UI
recovery. Local actions also fence the exact provider activity the renderer saw;
external activity/account changes clear stale detail and paged cursors.

The signed release-mode five-process harness passes from source head
`cc832aa7672729a7762adc9b0059d77d409cf3d8`: crash-after-commit persists exact
done, snoozed and bookmarked generation-1 projections without increasing
provider/vault counters, and the fresh restart reads byte-equivalent SQLite
evidence with provider and vault access both `0 -> 0`. Remote CI, other platforms
and live provider/PAT/keyring behavior remain separate gates. No PR is merged.

### Publication and next slice checkpoint — 8 October 2026

RURU-124 is now In Review in draft [#169](https://github.com/ruru-m07/gitru/pull/169).
Its exact final head `4d31a40743d3e203f2fc1027f17b4ae292268478` passes all
14 reported remote checks. RURU-118 is In Review in draft
[#170](https://github.com/ruru-m07/gitru/pull/170); exact head
`e75bf77b920c9f1198d4e7d825cf829298209f5b` also passes all 14 reported checks,
including the three-platform Rust, packaged E2E and retained harness matrix.
RURU-114 is In Review in draft [#171](https://github.com/ruru-m07/gitru/pull/171)
at `de0e245d5b10bbebb9b66ad78d92febe084a986b`. Its final local `make verify`
passes; remote macOS/Linux checks pass while Windows is still pending at
7 October 20:10 UTC. Draft-skip bot checks do not constitute code review.

RURU-119 is the active bounded slice, stacked on RURU-114. Its signed contract,
provider summary adapters, migration 0014 storage/retention, collection scheduler,
and native local-Git selected-diff accelerator are implemented. Selected provider
hydration and the frontend are being integrated; final combined validation and
PR publication remain outstanding. The architecture progress section records
the focused evidence and limits. All worktrees remain on `/Volumes/Lexar` and
the no-merge and no-personal-credential constraints remain in force.

### Local files qualification and recovery continuation — 8 October 2026

RURU-119's signed source `df54c444771d5c6b671080eafc6c4b3daf3c44ab` passes
full local verification (712 frontend tests/one platform skip; 1,076 Rust tests/
five helper ignores) and the real five-process macOS retained harness. The
work note records exact evidence and deferred live-provider/platform boundaries.
Publish this bounded read-only slice on RURU-114; do not open a duplicate.

RURU-114 #171 now has all 14 reported remote checks passing. Existing RURU-106
#145 is being continued on the current schema with recovery metadata migration
0015, immutable command quarantine and native runtime shutdown/restart; it stays
In Progress until integrated recovery UI and qualification pass. RURU-125's
isolated native measurement lane is In Progress. RURU-115 remains dependent on
RURU-106; restored commands must never bypass quarantine merely after reconnect.
No merge or personal credential validation is authorized by these checkpoints.
