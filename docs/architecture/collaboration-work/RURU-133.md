# RURU-133 — guarded online GitHub merge

Pre-code contract, 8 October 2026. Base: integration PR190, signed
`f0b8801dec569af24a1b1f67744600f85f4d90ac`. The isolated managed worktree lives
on Lexar. Live Linear R133 is Backlog; its five prerequisites R77/R115/R117/R118/
R123 are In Review and implemented as ancestors of this base. Inventory found no
existing R133 PR or worktree. This work does not authorize merging a real PR.

## Bounded product contract

Implement GitHub.com direct synchronous merge only, with an explicit merge,
squash or rebase method and the exact inspected head SHA. Other providers and
GitHub stack/merge-queue/auto-merge paths stay unavailable. GitHub's currently
documented asynchronous API can include open downstack PRs; do not call it in
this slice. The server-side sha guard protects the inspected head. It does not
freeze repository rules, permissions or the target branch, and UI must not claim
otherwise. No bypass-rules option, arbitrary URL, provider request body or implicit
branch deletion/push is accepted from the renderer.

Native online preview reads authenticated current repository identity,
permissions and supported merge methods plus exact PR identity/head/state and
mergeability. Refuse missing/unknown evidence, drafts, closed/merged PRs,
non-mergeable or blocked/unknown provider states. Existing checks and review
panels display head-bound cached coverage; partial/stale/missing data never
becomes authorization. Provider rules remain authoritative at merge time. The UI
explains this distinction and shows the inspected full head and explicit method.

A bounded process-local grant binds account, actor, epoch, authorization view,
repository, subject, local Body revision, head and the online observation. It
expires after 60 monotonic seconds and is capped at 32 live entries. Confirmation
binds one stable command UUID and method; duplicate IPC submission returns the
same durable local receipt. New intent requires the unexpired grant and active
authorization. Expiry during a held vault/preflight/claim prevents mutation.
Restart never reconstructs send authority from persisted payload bytes. Fresh
permission/head checks precede final writer-held claim, which rechecks local
context and grant. Dispatch checks the grant again after the durable attempt.

Use the existing immutable command/attempt/evidence/recovery protocol, with a
new operation-specific payload/proof codec. No shared v1 effect/proof mutation,
second outbox or optimistic merged state. A command blocked before send remains
visible for explicit recovery; it cannot silently reactivate on reconnect. Merge
recovery supports inspection/cancel/pause under existing limits, not offline
replacement or replay. New merge consent must come through a new online preview.

Only a structurally valid 200 response with merged=true and a bounded merge OID
can prove direct completion; its evidence binds the exact request guard/method
and native subject. Unexpected 202 remains accepted, never confirmed. Lost,
malformed, transport or uncertain responses never authorize a second PUT. Bounded
read-only reconciliation requires authenticated repository/PR/head identity and
actual merged state plus merge-result evidence. Matching desired state proves
observed convergence, not which actor caused it. Head drift/definite conflict and
permission denial are explicit outcomes. No absence or expired asynchronous
receipt proves non-delivery. Preserve captured quota/auth observations even if
canonical publication fails its fence.

Canonical merged state publishes only under current account/actor/epoch/view and
Body context guards, using existing canonical materialization/feed revision
fences. Checks/reviews remain independently scoped; no stale cache is elevated.
Fixed numeric repository routing avoids mutable-name redirection, with no named
fallback. Its live authenticated alias compatibility remains a separate gate;
synthetic HTTP tests do not claim production-provider verification.

## Primary references checked 8 October 2026

- https://docs.github.com/en/rest/pulls/pulls?apiVersion=2026-03-10#merge-a-pull-request
- https://docs.github.com/en/rest/pulls/pulls?apiVersion=2026-03-10#get-a-pull-request
- https://docs.github.com/en/rest/pulls/pulls?apiVersion=2026-03-10#merge-a-pull-request-asynchronously
- https://docs.github.com/en/rest/pulls/pulls?apiVersion=2026-03-10#get-the-result-of-an-asynchronous-merge

## Qualification and ownership

This lane owns only its isolated R133 tree. Use its own native target, generated
IPC via make typegen, coss UI and established query/reset/receipt patterns. Tests
must prove exact SHA/method wire bytes; fresh identity/head/permission drift;
method/mergeability uncertainty; expiry before and during held preflight/vault;
dedup after a lost local receipt; persisted-before-dispatch attempt; strict merge
receipt; 202/malformed/lost-response read-only reconciliation; no second PUT;
cold restart and immutable restore quarantine; quota/auth failure isolation;
canonical held-feed fencing; unsupported providers; UI context/consent/late reply
isolation and preserved sibling drafts. Run focused native/SDK/UI checks, strict
Clippy and applicable full workspace gates. Record local results separately from
new-head remote CI and live provider/platform boundaries. Open and attach a
reviewable draft PR explicitly dependent on PR190. Never merge it automatically.

Implementation and qualification are pending.
