# RURU-134 — Durable GitHub issue creation

Status: full local workspace qualification passed; draft publication, 8 October 2026.

This first slice creates GitHub.com issues with title/body in one explicitly
selected cached repository. Labels, assignees, milestones, issue types and custom
fields remain unsupported in this slice; the broader metadata criterion stays
open. The issue remains In Progress until that scope is handled. Keep the existing
account-independent desktop architecture and native HTTP/vault/database ownership.
No live provider mutation or personal credential inspection is permitted during
unattended qualification. The managed external worktree starts atop Activity
schema21 and consumes final qualified parents before publication.

## Draft identity and admission

Use separate durable issue draft UUIDs, repository/account bindings and generation
CAS. Preserve original authored title/body independently of current authentication,
cache retention, restart and provider confirmation. Draft identity is temporary
local intent, never a fabricated remote issue ID. Distinct new drafts require an
explicit user action. For one draft, an unresolved admitted creation blocks edited
generations and different command UUIDs; a lost local receipt retries the identical
UUID/payload. A repository-target command is supported by the existing outbox and
serializes its mutation lane without pretending an existing item effect exists.

Migration 0022 adds draft/submission and immutable canonical identity linkage with
unique receipt constraints. Include frozen0021 migration bytes and strict restore
validation/quarantine. Admission, immutable linkage and revision hints commit
atomically. Opening/saving the composer is entirely local. Background submission
requires explicit consent and cannot silently change the selected repository.

## Provider operation and ambiguity

Official reference checked 8 October 2026:
https://docs.github.com/en/rest/issues/issues?apiVersion=2026-03-10#create-an-issue
GitHub documents Issues-write token permission, a required title, an optional body,
a201 creation response and secondary rate limits; no request idempotency key is
documented. Some optional metadata can be silently omitted by the provider, so
this slice never treats HTTP success as proof that unsupported metadata applied.

Use validated numeric repository identity for fixed GET/POST routes, with no
mutable name fallback, redirects or internal automatic mutation retry. Live
authenticated numeric POST alias compatibility remains unqualified. Capture current
account/actor/epoch/view, selected repository/native identity and current access;
preflight uses bounded native HTTP and final writer-owned claim checks. Persist
an attempt before POST. Independently retain shared quota and authorization facts.

Confirm only a strict bounded 201 receipt bound to the command hash, original
actor, repository, native issue ID/number, valid canonical URLs/timestamps, issue
kind (reject pull-request payloads), and the authored title/body evidence. Original
intent stays distinct from canonical observed content. A canonical provider ID
cannot prove two creations. Timeout, malformed201, response loss, crash after
dispatch or missing strong evidence stays outcome-unknown with no automatic POST
replay and no text/time search heuristic. Restore never authorizes resend. Recovery
preserves/export drafts and status without manufacturing non-delivery proof.

## Canonical publication and local feeds

Do not copy the comment receipt path alone: issue creation must materialize a
canonical entity. In the same finalization transaction, revalidate current access
and account/actor/epoch/view/repository, resolve through the existing provider
identity mapping (including a feed-discovered entity that arrived first), insert
canonical summary/Body/search data and link the immutable temporary draft identity.
Only then may command confirmation become visible. Authored draft bytes survive.

Define receipt-derived feed visibility independently from pagination membership.
A created issue must appear in local filtered lists/counts/search without asserting
full provider enumeration, pretending membership in a current partial run, or
rewinding a sync cursor. Invalidate affected scope revisions/validators so an older
in-flight/full or partial feed cannot overwrite or withdraw newer receipt evidence.
Retire receipt-derived visibility only after genuine newer enumeration or explicit
current access withdrawal; never pin inaccessible provider content merely because
its authored draft remains recoverable. Keep normal detail/provider visibility
fences and bounded caches intact. The native owner must freeze the exact finalizer
and visibility transition with the independent reviewer before coding that part.

## UI and qualification

Provide an explicitly scoped New issue flow, local save, exact retry, clear queued
versus confirmed status, canonical navigation after validated publication and a
bounded saved-draft recovery view that survives disconnect/missing repositories.
Account/repository transitions cannot carry another target's late receipts or text.
Unsupported providers disclose the limit and never dispatch.

Finite tests must cover generation CAS/rollback, duplicate UUID/generation,
cache-observed-first identity convergence, receipt reuse, full/partial held-feed
races, atomic entity/FTS/visibility/linkage/confirmation rollback, edited draft after
dispatch, authorization/view/repository drift, unknown after restart/restore,
permission/quota observations and disconnected draft recovery. Meaningful UI/SDK
cases cover save-before-send, exact receipt retry, changed context, canonical
navigation and retained drafts. Generate IPC with make typegen. Run full local
make verify and separate its evidence from remote CI and live provider/vault/GUI
qualification. Use signed scoped commits; open a reviewable draft PR, never merge.

## Frozen native publication transition

Use the existing deterministic canonical identity `github:issue:{native_id}` and
`identities::item_in` to converge with a feed-discovered entity. Insert active
scope membership only with an explicit receipt-origin cache marker; never set
last_seen_run to an ongoing enumeration run. A marker keyed by account, epoch,
canonical issue and creation command protects this provisional membership from
absence retirement until an accepted real feed page observes that exact issue.
Then the ordinary seen path removes the marker and normal enumeration absence
handling resumes. Existing list/count/search predicates remain authoritative.

Creation increments affected data_revision and clears validators to reject held
responses. The explicit receipt marker also handles an old traversal resuming
after creation, which data_revision alone cannot distinguish. Preserve its cursor
and run. A missing scope starts Missing/Partial, never Complete. Existing selected
repository/access-denial/epoch gates apply; auth/cache purge removes provider cache
markers and cannot resurrect visibility from retained authored receipt history.

A validated 201 may be historical by finalization: retain a newer existing summary
and Body, or already-published equal-timestamp conflicting data, while preserving
creation receipt bytes and the immutable draft mapping separately. In one writer
transaction establish identity, bounded summary/Body/FTS/effective projection,
provisional membership/protection, immutable linkage and revision invalidation
before confirmation. A failure rolls back all of them. Required race controls
include held terminal empty feeds, a resumed old multipage traversal,
feed-before-receipt convergence, newer cache before receipt and auth purge/reconnect.

## Implemented native checkpoint

Migration 0022 separates retained issue drafts and immutable submission/resolution
ledgers from purgeable receipt-origin visibility. The generated API exposes local
lookup/save, bounded draft recovery and explicit background submission. Generation
CAS, unchanged-save stability, original-epoch exact UUID retry, same-draft pending
blocking and atomic admission prevent a lost receipt or edit from authorizing a
second creation. Canonical publication uses the reviewed transition above, and
retains the authored draft after both confirmed and unknown outcomes.

The GitHub policy uses fixed numeric GET/POST routes, rejects archived repositories
and disabled issues, and does not require repository push permission. A valid
Issues-write credential is still adjudicated by the provider. A strong 201 is
bound to the saved command, actor, repository and authored fields; null provider
body matches only an explicitly empty authored body. Unknown outcomes have no
HTTP reconciliation heuristic and never automatically resend. Persisted proof
validation repeats mandatory normalized metadata, actor/login, URL and timestamp
checks instead of trusting that a live parser once ran.

Restore recognizes frozen schemas 1–21 and current22. It validates every authored
submission's canonical bytes/content hash and every confirmed resolution's exact
operation-evidence ordinal. Conversely, strong issue-created evidence requires
its immutable mapping and confirmed state. Marker validation joins that mapping;
restored commands remain quarantined. Corrupt-backup checks preserve both selected
backup bytes and the untouched target database.

Local validation remains separate from remote CI, packaged GUI exercise and live
provider write qualification. Numeric GitHub mutation aliases remain a documented
live-provider compatibility boundary; this task never inspected personal tokens
or dispatched a live provider mutation. Labels, assignees, milestones and other
issue metadata creation remain outside this first slice.

Focused native qualification on this checkpoint: 22 issue-creation/publication
controls passed, including 12 corrupt-backup variations, duplicate canonical
receipt rejection and transaction rollback; 14 recovery integration tests, 9 frozen
migration/fault controls (schemas 1–21 plus failed 22 retry), and 8 current-schema
recovery controls passed. Strict collaboration all-target Clippy, workspace Rust formatting
and diff checks passed. Independent native reviews rechecked admission,
publication and exact proof/mapping validation after the fixes. Full workspace
verification and remote CI remain publication-owner gates.

The frontend checkpoint generated 153 commands. Its latest full SDK run passed 211
tests; desktop passed 599 with one platform skip, with both TypeScript checks and
scoped formatting checks passing. Auth/reset synchronously redacts provider-derived
navigation/context while retaining authored text, and fresh submissions require a
new native context. These are local checks, not packaged/live-provider evidence.


## Full integrated local qualification

Signed product integration `e7db1639f2904ce694d3db4af428d9e220e8052b` includes
Activity and the retained comment/issue draft authorization fixes. `make verify`
passes 813 frontend tests with one platform skip and 1,340 Rust test executions
with seven standalone helper ignores, plus lint, TypeScript checks, desktop
production build, formatting and strict workspace Clippy. Generated IPC contains
153 commands and 496 schema exports. The retained exact-UUID retry stays local
and unavailable during an account reset or failed authoritative refetch.

The native creation/publication, restore-integrity and UI boundaries received
independent reviews. Account clear/reset removes provider context and canonical
links immediately while preserving authored title/body and submission status;
held native reads cannot repopulate the cleared authority. Creation receipts
converge with independently discovered identities and cannot retire genuine feed
membership or overwrite newer Body evidence.

This is a GitHub.com title/body creation slice on Activity PR #184. Metadata creation
and authenticated numeric POST compatibility remain open. Remote CI starts with
publication; local tests do not qualify real provider writes, production vaults
or packaged execution. No PR is merged.


The separate feature-enabled native harness passes all 21 cases on the same
product source, with strict all-target collaboration Clippy under `test-harness`.
These synthetic native results do not add a packaged GUI or live-vault claim.
