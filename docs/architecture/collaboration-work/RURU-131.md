# RURU-131 — Durable GitHub conversation comment submission

Status: approved bounded implementation contract, 8 October 2026. This document
precedes implementation and does not claim completed code or qualification.

## Scope and ownership

Managed external worktree `ruru-131-comment-send`, branch
`ruru/ruru-131-comment-send`, starts on the RURU-129 title/body review stack and
will consume its final qualified parent. GitHub.com PR/issue conversation
comments are the only initial creation operation. Review comments, GitLab,
Bitbucket, issue/PR creation and merge remain separate work. Accounts use native
PAT/manual or explicit CLI import and remain independent of Gitru cloud.

The native lane owns all collaboration Rust, DTOs, migration/recovery, finite
provider transport fixtures, and the operation policy. The frontend lane owns
Tauri registration/caller policy, generated IPC via `make typegen`, SDK, React
UI and its tests. The coordinator reviews boundaries, records qualification and
publishes a scoped draft PR. Never inspect personal credentials or send live
provider comments during unattended validation.

## Separate authored draft and admission

A dedicated `comment_drafts` table and explicit Comment composer are separate
from the existing Private note `drafts` table. Private notes are never copied,
prefilled or submitted implicitly. Local comment drafts survive disconnect,
missing subjects, restart and restore; read/save uses a known local account,
while provider-send authority additionally requires current active credentials
and accessible native target context. Save uses generation CAS, bounds text at
16 KiB, and does not increment generation for unchanged bytes.

Migration 0020 adds immutable submission linkage keyed by account, subject and
saved draft generation, with command ID and body hash. Admission checks the
saved generation/content, native account/epoch/view/target token and explicit
queue-when-connected policy in one writer transaction. The provider payload and
route are built only in native code. The same command UUID/request returns the
saved receipt after a lost IPC response; changed bytes under that UUID fail.
A fresh UUID cannot submit an already-linked draft generation. Outstanding
queued/accepted/unknown creation for the same target blocks another send even
if the user edits the draft; editing must not bypass ambiguous-create recovery.
Original draft bytes remain after admission, success, failure and unknown result.

## Provider evidence and unknown outcomes

Use the established bounded GitHub native transport and authenticated exact
repository/issue identity preflight. One durable dispatch attempt must commit
before POST. GitHub's documented conversation-comment endpoint applies to both
issues and PRs. A validated 201 response with exact parent identity, canonical
comment ID, expected authored body and author, and valid timestamps supplies
strong creation evidence. Mere HTTP success without valid identity/body evidence
does not confirm creation.

Lost responses, timeouts, malformed receipts, crashes after dispatch and restore
remain outcome-unknown unless strong operation evidence proves completion or
non-delivery. Matching text, actor or time in a list is not unique causality.
There is no automatic POST replay, hidden marker, invented idempotency guarantee,
or generic replacement path for an ambiguous create. Recovery offers saved
intent/status/export and honest next-step text. No user acknowledgement is
silently converted into proof that the remote comment was never created.

Official API reference, checked 8 October 2026:
https://docs.github.com/en/rest/issues/comments#create-an-issue-comment
The endpoint documents a required body, a 201 creation response and write-level
Issues or Pull requests token permission. It can trigger secondary rate limits.
No request idempotency key is documented; this contract therefore assumes none.

## Local created receipts and frontend API

A bounded, keyset local query follows immutable submission linkage to validated
confirmed operation evidence and returns canonical comment IDs/URLs. It is
separate from normal cached comment enumeration, labeled as submission history
from Gitru; it does not claim complete or current provider conversation coverage.
A stale/full detail feed cannot erase creation proof. Auth/view/visibility fences
still govern provider-derived receipt details; authored draft recovery remains
independent. A future canonical cache seed must preserve these same boundaries.

Native DTOs expose a local draft snapshot (body, generation, send context,
availability/reason, pending submission, revision/view), save-CAS request,
send request (context, saved generation, command UUID, explicit offline policy),
local admission receipt and bounded submitted-comment history. Freeze DTOs with
the frontend lane before implementation. Names may follow established package
conventions without changing the contract.

The distinct composer reads local state, saves before Send, discloses queued
rather than completed delivery, and preserves entered text across stale context
and transport failures. Exact local retry retains command UUID/context/generation.
Account or target transitions cannot carry text or late receipts to another
account. Native availability drives controls; unsupported providers retain
private drafts and an honest unsupported action. No network request starts merely
because the composer is opened.

## Required qualification

Cover draft isolation from Private notes, generation CAS/unchanged-save behavior,
exact local retry and different-UUID deduplication, atomic admission rollback,
account/epoch/view/target fences, revoked or missing targets, same-target pending
creation blocking, strict 201 receipt parsing, accepted/unknown/no-second-POST
across crash/reopen/restore, quota and authentication observations, and preserved
draft bytes. Restore frozen schemas through 0019 and validate new immutable
linkage. UI/SDK tests cover offline save/admission, changed context with text
retention, failed local receipt retry, account retirement and original subject.
Run full local `make verify`; keep its evidence separate from remote CI and live
provider/platform/vault qualification. Signed scoped commits; no merge.

## Implemented native slice and qualification checkpoint

The native implementation now supplies separate CAS comment drafts and a bounded
local saved-comment-draft recovery list, transactional admission plus immutable
per-generation submission linkage, and the registered GitHub creation policy.
Unchanged saves keep the generation; both different-UUID duplicate submissions
and edited drafts behind an unresolved creation remain blocked. Authored text is
recoverable with a disconnected account or missing subject; provider-derived
submission receipts require current active access to that exact target.

Both preflight and POST use the immutable numeric repository route, matching the
existing GitHub reader's route family. No redirect or mutable named-route fallback
is allowed. The numeric mutation alias has not been exercised against a live
provider, so live compatibility remains unqualified; the documented named
conversation-comment endpoint supplies the operation/201 semantics, not proof
of numeric-alias deployment support. All HTTP qualification uses finite local
synthetic servers and synthetic credentials.

A creation confirms only a strict, canonical, bounded receipt tied to the command
hash, original actor/epoch, exact native target and authenticated 201 response.
The final writer transaction also checks the current authorization view. A
canonical GitHub comment ID cannot back two creation receipts. Submitted history
comes from immutable command evidence and does not claim or change whole-comment
coverage. Unknown creation only reconciles to unknown without another HTTP call;
there is no text/time matching or automatic retry of POST. Rate-limit and
credential observations flow through the shared delivery worker independently
of the operation result.

Migration 0020 extends recovery policy with authored draft/submission validation,
canonical receipt linkage and preservation of original command bytes. All
restored pending commands remain quarantined even with zero attempts and later
reauthentication. Frozen migration 0019 bytes/checksum extend the supported
historical matrix through versions 1–19.

Local qualification before parent integration: 16 finite native comment tests,
14 recovery tests and 7 historical migration tests pass; strict collaboration
all-target Clippy passes. The native integrity cases also check direct
immutable-row enforcement and reject missing linkage or tampered receipt/hash
without changing either selected backup or current database. Full Rust formatting
also passes. The prerequisite clock/transport fixes, full workspace verification, remote CI
and packaged/live-provider qualification are recorded separately by the
coordinator at publication.

## Integrated publication evidence — 8 October 2026

Full local `make verify` at signed source `1895c542` passes **787 frontend tests**
(one platform skip), **1,279 Rust test executions** (seven subprocess-helper
ignores), lint, typechecks, desktop build, full formatting and strict workspace
Clippy. This run includes the corrected historical fixture: its simulated v14
rewind removes schema20-only draft/submission objects before testing restoration;
production schema-integrity checks remain unchanged.

Final parent integration `5eab2109` consumes PR181's qualified Windows crash
startup handshake and disk-backed cursor-resume fixture corrections. On that
source, nine runtime-sync cases, 14 credential crash parent cases (one helper
ignored), 21 feature-harness cases and strict feature-enabled all-target
collaboration Clippy/fmt pass. These are separate qualified deltas to the full
workspace baseline, not an assertion that the full command reran on this head.

Generated IPC contains 149 commands/473 executable schemas. SDK/UI coverage
includes account/context fences, local retry identity, saved draft recovery and
retained composer text. Native and frontend independent review found no remaining
blocker in this bounded slice. Logs: `/tmp/gitru-r131-final-verify.log`,
`/tmp/gitru-r131-final-sync.log`, `/tmp/gitru-r131-final-credential-qualified.log`,
`/tmp/gitru-r131-final-harness.log`, `/tmp/gitru-r131-final-clippy.log`.

The review base is PR181 (`ruru/ruru-129-title-body-edits`). New-head remote CI
starts at publication. Authenticated live numeric mutation support, OS vaults
and packaged-window qualification are unclaimed. No merge was performed.


### Retained draft authorization follow-up — 8 October 2026

Signed `413c909` synchronously removes provider context from retained comment
drafts on account clear or runtime reset while preserving authored body, generation
and immutable submission status. Held pre-reset reads cannot restore that context.
Signed `2b37e07` also disables the retained exact-UUID receipt retry while the
snapshot is AccountUnavailable or its refetch fails; the same request can reappear
after an authoritative local reload. An open composer preserves unsaved text.

The SDK passes203 tests; desktop passes585 with one platform skip, both TypeScript
checks pass and scoped Biome/diff checks pass. The later retry gate passes all six
composer cases and desktop types. These are frontend deltas to the full workspace
baseline above; new-head remote CI remains separate. No live credentials or
provider writes were used.
