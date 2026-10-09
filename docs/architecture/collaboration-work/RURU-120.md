# RURU-120 — online GitHub pull request creation

Pre-code contract, 8 October 2026. This isolated managed Lexar worktree starts
from published guarded-merge PR195, signed
`435599c7c45cd884ac3a4edb165549b96f5af759`. Live Linear R120 remains Backlog at
planning; R77/R96/R99/R115/R116/R117 have implemented In Review prerequisites in
this ancestry. Fresh worktree/PR inventory found no existing R120 implementation.
No real provider mutation, local push, checkout or PR merge is authorized by the
implementation task. Root reserves migration23 for this slice; R132 reserves24
and must consume the qualified23 checkpoint before registering24.

## Bounded product behavior

Create a GitHub.com PR between two distinct existing branches in the same
explicitly selected remote repository. The user chooses the connected account,
selected repository, authored local-clone link, local source branch, remote base
branch, title/body and ready-versus-draft status. Unsupported providers, forks,
existing-issue conversion, arbitrary head repositories, reviewers/labels and
implicit publishing stay unavailable. Reject detached/unborn/missing source
references, unmapped or changed local clones, absent/unpublished remote branches
and identical source/base tips before admission. Preserve the local authored
text and choices when permissions or provider cache disappear.

Local list/detail/draft reads stay in SQLite. The frontend uses generated Rust
commands and the existing local SDK query/revision bridge. Online preview and
creation are explicit actions, independent of Gitru cloud sign-in. The UI must
explain that uncommitted work and unpublished local commits are not included;
Gitru does not push, fetch, switch branches or mutate the working tree here.

GitHub's documented create endpoint accepts branch names, not an expected-head
SHA or idempotency key. The consent policy is therefore explicitly
`BestEffortCurrentBranches`: preview records the inspected source/base tips, but
the branches can advance before the server creates the PR. A valid authenticated
201 with exact repository/actor/ref identity still proves creation when either
returned head differs. Preserve and expose the created PR identity plus inspected
versus observed head drift; do not discard that receipt or retry the POST. This
is different from R133's server-side expected-SHA merge guard.

## Native authority and online grant

Resolve the selected local repository through RepoManager, caller incarnation
proof and a fresh native local-link observation. Recheck registration, remote
configuration digest, current LocalLinkVersion, actor/account/epoch/view,
repository native identity and selected/access state. The renderer provides only
bounded identifiers and choices; it supplies no path, remote URL, token, provider
body or trusted local observation.

A bounded uncached `crates/git` inspection resolves the selected full local ref
through the existing Git runner with check-ref-format, literal bounded arguments,
finite output and a timeout. Cached list_branches/branch_info data may populate a
chooser but cannot authorize creation. Native preview authenticates fresh remote
repository identity/access and both branch tips; the local source tip must equal
the observed remote source tip. The same-repository first slice conservatively
requires current repository push permission; this is not a claim that the token
has the endpoint's Pull requests write permission. Provider authorization remains
final at POST. Fresh GETs use the shared bounded point-read redirect policy: only the configured origin and exact original route can survive validation, with no query, fragment or encoded redirect tricks. Refused redirects do not trigger mutable-path fallback. The single mutation never follows redirects.

Preview binds draft generation/content, local registration/link/config/ref
observation, remote repository/ref/tip observations and current account authority
in a bounded process-local 60-second grant. Confirmation re-observes the local
mapping/source tip before arming one stable command UUID. Native delivery rechecks
fresh provider identity/branches/permission immediately before its final
writer-held admission/claim and checks the still-live grant again before POST.
Recheck budget after held vault/other native observations. Expiry or cold restart
never reconstructs send authority from durable payload bytes. Pending before-send
work requires explicit recovery/new preview and cannot silently send on reconnect.

Local and remote observations are temporal evidence, not locks against external
Git or remote branch changes. The final contract must state each revalidation
point; never claim an atomic local/remote snapshot. Fresh source-tip drift before
send blocks it; drift in an otherwise valid201 is surfaced alongside the retained
created identity. In-flight cancellation is owned natively, and stale caller,
account/view/link/draft generations cannot publish new admission authority.

## Durable command and receipt protocol

Use the existing command/attempt ledger, repository target ordering, native
preparation hook, captured quota/auth handling, restore quarantine and recovery
UI. Introduce operation `github.create_pull_request`, payload version1 and proof
`github.pull_created`, version1. Do not alter previously frozen v1 item or effect
codecs, add a second outbox or project a fake pending PR into provider feeds.
A draft UUID is local temporary identity; only an authenticated strict201 receipt
creates the canonical PR mapping. Stable UUID resubmission after a lost IPC
receipt returns the exact durable local admission receipt under current matching
authority; it is not another POST or proof of provider completion.

The immutable payload binds account/actor/installation, repository native ID,
draft ID/generation/content hash, explicit ref names and draft status, inspected
tips, local link/observation identity and consent policy. A final attempt is
persisted before the one native POST. Require a strict bounded201 payload with
positive native PR ID/number, exact repository and actor, exact source/base refs,
validated timestamps/URLs/title/body/draft status and actual source/base tips.
Record actual canonical state without inferring it from a local desired value.
The proof binds original command/preparation plus the returned identity; unique
receipt linkage prevents one remote PR from resolving two independent drafts.

Timeout, lost response, malformed201, unexpected202 or ambiguous rejection never
becomes an automatically replayable POST. Preserve authored text and durable
unknown/accepted state. No list/search/body-text heuristic can establish that an
unknown PR was created by this command. Exact known receipt identity may support
read-only reconciliation, but a valid creation receipt remains linked even if
later access is lost or branches advance. Recovery is inspect/cancel/pause under
existing zero-attempt and ambiguity rules; replacement is unavailable until its
operation-specific semantics are separately implemented.

## Schema23 and publication contract

Migration23 owns these names (no competing definitions in R132):

- `pull_drafts`: account/draft UUID primary key; immutable repository target;
  bounded title/body, local repository/link IDs and link generation, source/base
  branches, draft flag and positive draft generation. Authored rows are not
  foreign keyed to evictable repository/local-link cache. Identity and retention
  triggers mirror the established issue-draft invariants.
- `pull_submissions`: account/draft/generation primary key, unique account/command,
  32-byte immutable submission/content hashes, authored-draft and exact immutable
  command-envelope FKs. Admission trigger requires repository target,
  `github.create_pull_request`/version1 and the current draft generation. History
  index is `pull_submissions_history`; update/delete triggers retain proof.
- `pull_resolutions`: immutable account/draft primary key and unique account/command
  and account/provider PR ID; binds canonical entity/native ID/number/URL and
  observed source/base OIDs to the exact submission. It has no provider-cache FK,
  so cache purge cannot erase authored creation identity. Original inspected tips
  remain in immutable command bytes; the snapshot derives head-drift evidence.
- `pull_created_receipt_id`: unique command_evidence expression index on
  account plus `$.receipt.item.provider_id`, restricted to `github.pull_created`
  version1; exact receipt verification precedes admission.
- `pull_creation_visibility`: purgeable account/entity primary key, unique
  account/command, captured epoch, FK to cached items with cascading deletion and
  retained command FK. It records receipt-origin visibility, never feed coverage.

Canonical publication uses the same identity/visibility/Body/FTS/effective-feed
machinery as issue creation, specialized for PR identity/head metadata. Under the
writer transaction, recheck active actor/epoch/view and selected authorized
repository; verify the immutable command/proof; insert or reuse the exact native
PR identity without overwriting a newer cached observation. Bind the authored
resolution in that same transaction. Bump the feed data-revision fence and clear
conditional validators so older held pages/304s cannot overwrite receipt data.

If provider membership has not already observed the item, seed provisional
membership and its marker. Old or resumed full enumeration cannot remove a
receipt-created PR it never saw. The first authenticated feed observation of the
same identity consumes the marker; only a later authoritative traversal may
prove absence. Reauthentication/disconnect/access purge removes provider cache
and marker without erasing authored drafts/submissions/resolutions. Canonical
queries retain all normal selection/access/epoch gates; no broad query exception.

Recovery policy advances to23; exact frozen schema22 SQL/checksum joins the
historical migration matrix. Backup verification inventories every new table,
trigger/index, immutable envelope/submission/resolution linkage, hash, bounded
ref/head field and receipt-origin marker's confirmed command/epoch/PR identity.
Invalid backups preserve both selected backup and current database. Restored
commands remain quarantined, and process grants never survive restore/restart.
Send the signed, qualified migration/recovery checkpoint to R132 before it adds24.

## Qualification plan

Use synthetic provider HTTP, SQLite and temporary Git repositories only. Prove
local registration/remote/link/ref changes, detached/missing/unpublished branches,
source/base identity and head drift, wrong repo/actor/fork, permission revocation,
held-vault/preflight/grant expiry, exact wire body, bounded input/response, quota
and auth preservation, persisted-before-dispatch attempts and no duplicate POST
following ambiguity or cold restart. Test a valid201 with advanced source/base
retains the actual canonical identity and explicit drift evidence.

Exercise admission CAS and lost IPC receipt, immutable unique receipt linkage,
feed-before-receipt and held/resumed-feed races, Body/head/FTS visibility,
authorization/cache purge, recoverable private drafts and migration/restore
faults. UI/SDK controls cover explicit choices/consent, local-save versus remote
submit states, foreign account/view and late callbacks, exact UUID retry,
unknown outcomes and canonical navigation. Use coss components, meaningful local
checks, make typegen, strict Clippy/format/types/lint and full workspace validation.
Keep exact source, local, remote-CI, packaged/native and live-provider evidence
separate. No personal credential or production provider mutation is needed.

## Primary references checked 8 October 2026

- [GitHub create PR](https://docs.github.com/en/rest/pulls/pulls?apiVersion=2026-03-10#create-a-pull-request): branch-name body, Pull requests write permission and201/403/422.
- [GitHub get branch](https://docs.github.com/en/rest/branches/branches?apiVersion=2026-03-10#get-a-branch): authenticated current branch tip;200/301/404 and Contents read permission.
- Existing R134 issue-creation/R131 comment-send immutable receipt and recovery
  contracts, R96 local links, R136 native local checkout authority and R133
  bounded online grants. These are implementation patterns, not evidence that
  PR creation already exists.

## Native contract refinement before qualification

The native caller carries a nonserializable `PullCreationOwner` containing its
exact lifecycle identity and synchronous validation callback. Process-local grants
retain that owner, reject another window's grant use, and validate its original
lifetime before preflight, writer claim, and the final one-shot dispatch. A known
exact durable UUID receipt may be recovered under a current authorized caller
without a new local Git or network preview.

A refusal before POST is explicit `github.pull_creation_declined` v1 evidence with
an exact typed preparation and a bounded reason (expired grant, changed inspected
branch range, or unavailable permission). Refusal after an attempt was recorded
keeps that attempt and its exact preparation; it does not claim HTTP happened.
Unknown attempted creations retain typed `Preparation` bytes and never reconstruct
a grant during restore. The confirmed receipt codec now explicitly validates all
fourteen metadata field observations, with GitHub's unavailable merge base marked
`Omitted` and no merge-base OID. The canonical created identity is `github:pull:<id>`.

These are contract checkpoints, not a claim that end-to-end creation or schema23
qualification is complete. Recovery and native runtime/provider controls are being
qualified independently; no live account or provider mutation has been used.

## Schema23 recovery qualification and native caller boundary

Native source checkpoint `b69807a1a500b09562fd9660070adf442af62627`
qualified 26 synthetic creation/publication controls and strict collaboration
all-target Clippy. The recovery lane adds a frozen byte-identical schema22
migration/checksum, streaming typed draft/submission/attempt/proof verification,
and exact bidirectional confirmed-resolution linkage. Both pre-attempt and
post-claim no-HTTP decline evidence retain their exact conflict ordinal and typed
preparation. Canonical receipt proof, actor, metadata and actual head observations
remain immutable; no process-local grant can be reconstructed from them.

The recovery matrix covers queued, attempted-unknown and confirmed histories,
repeated backup/restore, exact authored/envelope/proof bytes, and current account
reauthorization. Pending restored commands remain quarantined and reject new
attempt inserts; already-confirmed history remains terminal rather than gaining a
spurious pending quarantine. Nineteen strong-proof corruption variants and eight
pre/post-attempt decline corruption variants reject the backup without modifying
selected or current files. Schema23 late-DDL failure rolls back the full migration,
preserves schema22 authored rows and retries cleanly.

The following native Git/IPC slice is scoped to fresh local observations at
preview and confirmation. It resolves a literal full local branch with bounded
local-only Git commands and a ten-second deadline including runner queue wait,
checks HEAD/source/path observations twice, and never pushes, fetches, checks out
or refreshes the index. The retained native owner rechecks window incarnation,
context ownership and configured registration/filesystem identity under the
writer and before dispatch. It rediscovers current `.git` coordinates so replacing
that pointer cannot reuse old directory evidence. These are temporal drift checks;
external Git/config changes after observation are not locked atomically with the
remote POST. A durable exact-UUID receipt can still be recovered before attempting
new Git or provider observations.

Qualification here uses temporary repositories, synthetic HTTP and SQLite only.
IPC/UI integration, full workspace checks, remote CI, packaged platform behavior
and live GitHub endpoint compatibility remain separate gates.

Completed recovery qualification on this checkpoint: `cargo test -p collaboration
--lib recovery::` — 35 passed, one intentional subprocess-entry ignore;
`cargo test -p collaboration --test recovery --test recovery_migrations` — 14 and
10 passed; strict collaboration all-target Clippy passed. Earlier reruns identified
and corrected only two test expectations: confirmed commands are terminal rather
than quarantined, and the historical-v1 restore now applies 23 migrations. The
unchanged frozen-v22 checksum is
`48c80b4dd9bd4594e02cb1e4108f84e1325cea2b6df0ab9926b3094903974813c86851f400352d8fc4c97b05c0b032c6`.
This qualifies schema23/recovery for R132 to stack migration24; it does not claim
completion of the still-in-progress native Git/IPC and frontend integration.

The native Git/IPC source is now implemented and locally qualified: six temporary
Git controls pass (dirty/index preservation, uncached moved ref, detached/unborn/
missing/invalid ref, Unicode packed refs and linked worktrees, replacement objects,
no partial-clone blob hydration, unsupported SHA256 identity); one paused-time
control proves the ten-second bound includes a blocked runner lease. Two desktop
unit controls prove the retained owner rejects same-path directory replacement
and a changed `.git` pointer even while old directories remain. Strict Git and
desktop all-target Clippy passed. The five native command wrappers compile and
accept only typed draft/request IDs; paths and trusted source observations never
come from IPC. These controls do not exercise a real OS webview lifecycle or live
GitHub mutation, and they do not replace the pending complete workspace/platform
qualification.


## Independent native review refinements

A receipt-only IPC retry now returns `NotReady` for new intent before arming any
process grant. Native IPC then obtains the fresh Git observation and retries the
same explicit submission; a previously committed exact UUID still returns its
durable receipt without Git, vault or HTTP. Snapshot `AlreadySubmitted` is reserved
for confirmed history. Queued and uncertain submissions retain their actual state
and use `PendingSubmission`, without claiming that a provider PR exists.

Raw authored limits alone do not bound JSON evidence. Before preview/network or
admission, the native frame now reserves its actual encoded size, a second encoded
body/title, and 24 KiB for the bounded preparation and required receipt identity,
refs, actor, URLs and clocks within the existing 64 KiB proof limit. Oversized
escaped text remains saved locally but needs shortening before preview. The causal
creation receipt intentionally marks unrequested labels, assignees and milestone
omitted rather than copying arbitrarily large concurrent automation metadata or
claiming those collections are empty. Required created identity and observed branch
range remain intact. A maximum 16 KiB ordinary body is supported; the separate
encoded limit can reject a heavily escaped body of the same raw length.

The final focused native review run passes 32 controls: 29 creation/publication
controls and three typed restore matrices. The maximum plain-body/large-collection,
escaped-body refusal, receipt-only probe and uncertain snapshot assertions pass.
The first run of the new snapshot assertion expected `unknown`; it was corrected
to the existing serialized command state `outcome_unknown` without a production
state change. Initial complete collaboration testing passed 1,046 cases with five
helper ignores and found one historical restore test still expecting 22 migrations;
the recovery checkpoint corrected it to 23 and qualified the affected suite. The
final workspace test/Clippy gates are recorded separately after integration.

## Final integrated local qualification — 8 October 2026

Qualified product source is `83e14bcbbea551d4a3ab71c5390d91d36a4b55c2`.
The frontend checkpoint `dfb2338e` passes the complete frontend suite (877 passes,
one platform skip), lint, workspace type checks and an uncached production build.
Its final eleven composer controls, desktop/E2E types and build also pass after
the canonical uncertain-state and encoded-size guidance refinements. Generated
IPC has 165 commands and 551 validated schemas; `make typegen` completed normally
after marking the native-only observation/owner as outside the wire graph.

The native workspace run qualified all 1,050 collaboration test executions with
five intentional subprocess-entry ignores. It then caught a temporary Git
fixture inheriting the machine's signing setting: a fixture commit's external
signer failed to allocate memory. The repair sets `commit.gpgsign=false` only in
temporary `TestRepo` constructors (ordinary, SHA256 and blobless), preserving all
real repository/global signing configuration. The complete remaining workspace
was rerun: 434 passing test executions and two helper ignores, including all six
new temporary-Git integration controls, desktop, IPC and logger. Total qualified
Rust executions across the complete run and scoped repair are 1,484, with seven
helper ignores. This is not described as one uninterrupted green `make verify`
invocation. Final strict workspace all-target Clippy, Rust formatting and diff
checks pass on the repaired source.

Local evidence logs: `/tmp/gitru-r120-native-review-final.log`,
`/tmp/gitru-r120-workspace-tests.log`, `/tmp/gitru-r120-workspace-remainder.log`,
`/tmp/gitru-r120-final-clippy.log` and `/tmp/gitru-r120-final-fmt.log`. The first
whole-crate run's historical migration-count assertion and the later Git fixture
failure are retained in this record; neither required a production rollback.

This slice is ready for a draft PR stacked on the explicit R133 dependency
`ruru/ruru-133-guarded-merge`. That parent PR195 at `435599c7` was independently
observed with 14/14 successful remote checks. The new creation PR's own remote
CI remains a separate publication gate. No live provider mutation, personal
credential read, real-webview lifecycle, or packaged creation flow was used to
claim these local results. No PR has been merged.
