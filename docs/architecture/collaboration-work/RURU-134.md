# RURU-134 — Durable GitHub issue creation

Status: bounded pre-code contract, 8 October 2026.

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

Migration0022 adds draft/submission and immutable canonical identity linkage with
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

Confirm only a strict bounded201 receipt bound to the command hash, original
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
