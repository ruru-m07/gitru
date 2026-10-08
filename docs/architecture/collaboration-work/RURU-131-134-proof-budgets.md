# RURU-131 / RURU-134 — Bounded creation proof follow-up

Pre-code contract, 8 October 2026. This isolated managed Lexar worktree starts on
published R120 PR199 at `6ecd5d94`. It repairs the existing comment/issue creation
slices without changing schema23, public DTOs, operation versions or immutable
stored evidence. Broader issue metadata authoring remains outside this scope.

## Observed risk and bounded repair

Raw body limits do not bound JSON encoding: allowed control characters expand to
six bytes. Both older codecs admit raw16KiB, while native requests and strong
receipt evidence have64KiB caps. A request can fit and receive a valid201 yet
fail to retain its causal proof. Optional issue metadata can independently make
that proof too large. Reproduce these paths through finite synthetic HTTP and
the actual native claim/dispatch/finalization path before production changes.

Keep saving the same raw title/body limits. Before new admission, bound the actual
encoded native frame and authored fields plus a documented conservative reserve
for mandatory receipt fields. Recheck before a durable dispatch attempt so old
oversized queued intent cannot reach POST. Exact already-durable UUID receipt
retry remains ahead of this new admission restriction; decoding/restoring old
valid v1 payloads/proofs must retain the existing contract. No immutable bytes are
rewritten and no new retry, non-delivery proof or replacement policy is invented.

New issue receipts retain mandatory canonical identity, title/body, actor, state,
URLs and times. Unrequested labels/assignees/milestone are explicitly Omitted;
empty collections must not claim known-empty authority. Preserve rate/auth
observations and all authority/identity/revision/uniqueness fences. Unknown
creation never performs another POST, including after restart/restore.

Only a confirmed submission may say AlreadySubmitted. Queued/uncertain/failed
intent retains its actual status and saved draft without implying remote creation.

## Ownership and qualification

This lane owns only its isolated tree: the comment/issue native codecs, policies,
storage admission/snapshot checks, focused fixtures and this worknote. No edits
in R132 or parent publication trees; no live credentials or provider writes.

Required controls: real RED encoded-size failures; before-admission zero outbox
and attempt for unsafe encoded drafts; raw draft retained; bounded legacy queued
refusal without a POST/attempt; maximal ordinary body success; rich unrequested
metadata omission; strict existing proof/restore and tamper controls; exact UUID
receipt retry; unknown state labeling and no second POST. Run focused suites,
recovery gates and strict Clippy, then wider native checks as justified. Record
local and remote evidence separately. All scoped commits are signed; no merges.
