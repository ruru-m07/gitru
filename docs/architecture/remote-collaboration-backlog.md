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
| [RURU-136: Check out pull request branches through the local Git workflow](https://linear.app/catra/issue/RURU-136/check-out-pull-request-branches-through-the-local-git-workflow) | Backlog | [RURU-77](https://linear.app/catra/issue/RURU-77/hydrate-and-render-cached-pull-request-details), [RURU-96](https://linear.app/catra/issue/RURU-96/link-local-git-remotes-to-collaboration-repositories-and-accounts) |
| [RURU-137: Cache and navigate the pull request commit list](https://linear.app/catra/issue/RURU-137/cache-and-navigate-the-pull-request-commit-list) | Backlog | [RURU-77](https://linear.app/catra/issue/RURU-77/hydrate-and-render-cached-pull-request-details) |

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

Planning key: C07. Group: Read experience. Priority: High. State: In Review (local qualification recorded below).
Linear: [RURU-121](https://linear.app/catra/issue/RURU-121/add-bounded-frontend-prefetch-and-cached-navigation).
Prerequisites: [RURU-77](https://linear.app/catra/issue/RURU-77/hydrate-and-render-cached-pull-request-details), [RURU-98](https://linear.app/catra/issue/RURU-98/add-foreground-demand-leases-to-the-native-sync-scheduler), [RURU-78](https://linear.app/catra/issue/RURU-78/hydrate-and-render-cached-issue-details).

Connect active views to native demand leases and prefetch a small working set on hover, keyboard focus and recent navigation.

Acceptance criteria:

- [x] Cached list/detail navigation renders useful content without waiting for provider HTTP; missing data has a clear state.
- [x] Prefetch shares native jobs across webviews and respects a bounded count/byte/concurrency budget.
- [x] Hidden tabs drop urgency; large-list and keyboard tests prove demand/prefetch does not grow without bound.

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

Planning key: C28. Group: Provider rollout. Priority: Medium. State: Backlog.
Linear: [RURU-111](https://linear.app/catra/issue/RURU-111/sync-gitlab-merge-requests-and-issues-through-shared-local-queries).
Prerequisites: [RURU-100](https://linear.app/catra/issue/RURU-100/drive-collaboration-ui-from-typed-resource-capabilities), [RURU-97](https://linear.app/catra/issue/RURU-97/add-independent-detail-scope-storage-and-hydration-contracts), [RURU-110](https://linear.app/catra/issue/RURU-110/connect-gitlab-accounts-and-discover-repositories).

Implement GitLab MR/issue summary and supported detail reads using ordinary collaboration components.

Acceptance criteria:

- [ ] Retain immutable native IDs, scoped IIDs/locators and provider facets while sharing local projections and queries.
- [ ] Support progressive paging, independently cached details, offline restart and explicit capability/access states.
- [ ] Divergent fixtures cover state transitions, transfers/renames, pagination and access loss with no GitLab branches in ordinary UI.

## RURU-127: Support GitLab todos with explicit inbox semantics

Planning key: C29. Group: Provider rollout. Priority: Medium. State: Backlog.
Linear: [RURU-127](https://linear.app/catra/issue/RURU-127/support-gitlab-todos-with-explicit-inbox-semantics).
Prerequisites: [RURU-79](https://linear.app/catra/issue/RURU-79/resolve-inbox-notifications-to-cached-pr-and-issue-subjects), [RURU-111](https://linear.app/catra/issue/RURU-111/sync-gitlab-merge-requests-and-issues-through-shared-local-queries), [RURU-100](https://linear.app/catra/issue/RURU-100/drive-collaboration-ui-from-typed-resource-capabilities).

Add a provider-native todo inbox source without pretending todos are GitHub notification threads.

Acceptance criteria:

- [ ] Persist todo identity, source, subject and completion/read semantics explicitly in the shared model.
- [ ] Reuse inbox UI where semantics match and resolve supported subjects to cached local details.
- [ ] Unsupported completion actions remain unavailable until durable delivery policies are verified; read-only syncing/offline tests pass.

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

Planning key: C33. Group: Offline writes. Priority: High. State: Backlog.
Linear: [RURU-114](https://linear.app/catra/issue/RURU-114/add-durable-command-admission-and-outbox-schema).
Prerequisites: [RURU-104](https://linear.app/catra/issue/RURU-104/implement-bounded-cache-retention-pins-and-wal-maintenance), [RURU-97](https://linear.app/catra/issue/RURU-97/add-independent-detail-scope-storage-and-hydration-contracts), [RURU-76](https://linear.app/catra/issue/RURU-76/introduce-a-provider-registry-canonical-resource-identities-and), [RURU-105](https://linear.app/catra/issue/RURU-105/test-schema-evolution-and-recoverable-migration-failures).

Introduce operation-specific immutable intent, separate from provider base observations, without exposing remote mutation controls yet.

Acceptance criteria:

- [ ] Persist command UUID, canonical submission/hash, account epoch, target, guards and submitted dependencies atomically with its local receipt.
- [ ] Duplicate identical submissions reuse receipts; changed payload with the same UUID is rejected.
- [ ] Forward migrations preserve original intent hashes and drafts; fake-provider/storage tests cover crash/restart and concurrent admission.
- [ ] Integrate retention protection for command targets, predecessor receipts and attempt evidence; eviction tests prove pending/conflicted/unknown commands retain the references needed for recovery.

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

Planning key: C47. Group: Read experience. Priority: Medium. State: Backlog.
Linear: [RURU-136](https://linear.app/catra/issue/RURU-136/check-out-pull-request-branches-through-the-local-git-workflow).
Prerequisites: [RURU-77](https://linear.app/catra/issue/RURU-77/hydrate-and-render-cached-pull-request-details), [RURU-96](https://linear.app/catra/issue/RURU-96/link-local-git-remotes-to-collaboration-repositories-and-accounts).

Preserve the original PR workflow's branch-checkout requirement as a separate local Git slice rather than coupling it to provider merge or outbox delivery.

Acceptance criteria:

- [ ] Resolve the linked local clone and verified PR source/fork/head explicitly; confirm fetch or checkout intent and reuse Rust Git services/generated commands.
- [ ] Respect dirty worktrees, detached heads, existing branches and moved PR heads; inspect fetched/local OIDs before claiming the requested head is checked out.
- [ ] Do not alter remotes or expose credentials implicitly; tests cover fork PRs, stale heads, missing local objects and failed Git operations.

## RURU-137: Cache and navigate the pull request commit list

Planning key: C48. Group: Read experience. Priority: Medium. State: Backlog.
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


### RURU-121 bounded cached navigation qualified locally — 4 October 2026

Pointer dwell, keyboard focus and recent PR/issue navigation now warm the existing
SDK/QueryClient projections and share the native ephemeral demand coordinator.
Bounds are24 identities including retired pending reads, two concurrent local
reads,16MiB estimated resident budget with4MiB active-read reservations,2MiB
per-bundle admission, eight mounted scopes,150ms dwell and120s LRU/TTL. At most
four speculative Body interests are admitted for5s; leaving, hidden activity or
feed-region disposal releases urgency. Estimated accounting does not claim an
exact heap bound. No provider HTTP, durable hydration or second durable cache is
introduced; selected observers, private drafts and ordinary cache semantics stay
with the existing detail path.

Account/epoch/instance/subject/facet identity, real SDK fences, reset and revision
invalidation suppress late private repopulation. Uncancellable native IPC keeps
its count/byte/concurrency reservation until actual settlement. Selected pending
or completed observers and newer cache values win over speculative completion.
Notifications retain explicit subject resolution. Completed recent projections
may remain within the bounded TTL after feed-region disposal; pending work cannot.

Local evidence:429 desktop/SDK tests across48 files, full desktop/SDK Biome291
files, SDK and desktop/E2E types, production frontend build and scoped whitespace
checks pass. Tests use actual QueryClient/SDK observer races,10,000-row focus
sweeps,200 body admissions, native lease lifecycle, offline missing/known-empty,
reset/epoch/visibility fencing and ordinary coss pointer/focus/Enter consumers.
No native package, live provider, exact JS heap or lower native priority claim is
made by this slice. Its review PR is stacked on #156; remote checks qualify its
own published head. No PR is merged. See [R121 work note](./collaboration-work/RURU-121.md).


### RURU-121 exact-head frontend CI repair — 4 October 2026

Initial published #158 head1991748 fails only the200-large-receipt stress case in
frontend job111411954423/run37194003075: Vitest's global5s default expires before
all CPU/allocation admissions finish. It already took4303ms locally; under another
complete local run it takes5689ms. A dedicated15s deadline on that stress test
retains all200 inputs and every exact count/byte/IPC/LRU/scope assertion. Engine
and provider deadlines, admission bounds and every other test remain unchanged.

R103's independently reproduced SDK startup visibility defect is also propagated:
a document can become visible while awaiting native activity subscription before
DOM listeners exist. The coordinator now samples visibility after listener
installation and before the native getter pumps; three held-listener tests cover
hidden-to-visible, stays-hidden and visible-to-hidden, without an invented later
event. Native generation/getter ordering and hidden admission remain intact.

Final full local UI/SDK qualification includes the UI project:433 tests/49 files,
full lint/testing config, SDK/desktop/E2E types and production frontend build pass.
Logs: /tmp/gitru-ruru121-ci-repair-{tests,lint,types,build}.log. These results do not
turn the old failed remote job green; a new signed head restarts its own matrix.
R103's retained native suite is qualified separately. No PR is merged.
