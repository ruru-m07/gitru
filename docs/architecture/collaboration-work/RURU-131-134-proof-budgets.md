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

## Source checkpoint and observed controls

The actual prior-source HTTP diagnostics failed3/3: comment and issue requests
with10,900 allowed control characters fit the64KiB HTTP limit (comment65,411
bytes) but their valid201 receipts became Unknown;100 valid large issue labels
also made an ordinary-body receipt unencodable. Both current-generation queued
snapshot controls failed by reporting AlreadySubmitted. Logs are
`/tmp/gitru-creation-budget-red.log` and `/tmp/gitru-creation-state-red.log`.

The implemented gate counts escaped frame/body bytes and, for issues, both title
copies, with24KiB reserved for bounded mandatory identities/URLs/actor/clocks and
fixed wrappers. It applies to new admission, preparation before HTTP, and the
writer-held final claim. Raw16KiB drafts and immutable v1 codecs remain unchanged.
Old queued intent keeps its exact UUID receipt but oversized intent cannot create
an attempt; preparation uses the existing finite deferred-read policy. An old
already-claimed valid receipt still confirms and passes real restore inspection.
Unknown post-attempt intent keeps its original no-replay semantics.

New issue receipts omit unrequested labels, assignees and milestones, including
rich valid response values, and omit the optional author URL. No known-empty
collection is invented. Confirmed-only snapshot wording is independent from the
actual queued/outcome_unknown state retained in each submission.

The approved narrow UI follow-up clears retry/consent only on definitive native
InvalidInput and offers safe shortening guidance while preserving the editor.
All native validation/admission errors precede the writer commit; after commit
the runtime only publishes/notifies and returns the receipt. Commit failures map
to Storage, and SDK late-authority checks are StaleAuthorizationError. Unknown
IPC failures therefore retain their exact UUID; no raw provider message is shown.
The existing coss controls, labels and local query flow remain unchanged; React
state changes stay in the explicit submission event handler.

Local checkpoint:21 comment cases and28 issue/publication cases pass, plus the
added maximal-frame control. The latter uses1024-byte control-character local
IDs, the longest allowed repository path, max u64 native IDs, escaped title/login
and an exact last-admissible control-body boundary. Its actual201 proof fits64KiB
and one additional escaped character fails admission. Prior valid large proofs
above the new conservative boundary remain byte-identical and restoreable. Strict
all-target/all-feature collaboration Clippy passes on the initial nine budget
controls. The two affected UI suites pass14 cases, including both lost-receipt
controls and definite-refusal editing; desktop/E2E TypeScript and scoped Biome
pass. Complete all-feature native and full frontend gates are still running;
remote CI, packaged UI and live provider checks are not claimed.

Independent native review verified the reserve algebra, unchanged v1 decoding,
three admission/claim gates and explicit Omitted metadata. No schema, public DTO,
command signature or generated binding change is required.

## Final local qualification

Signed source `1a60026c` passes the complete collaboration crate with all features:
**1,085 Rust test executions / five subprocess-helper ignores** across27 result
summaries. This includes the actual runtime/SQLite, ten encoded-budget controls,
confirmed-only state regressions, full recovery/restore/migration matrix, credential
crash fixtures and performance fixture. Final strict all-target/all-feature
collaboration Clippy, workspace Rust formatting and diff checks pass. Logs:
`/tmp/gitru-creation-budget-all-native.log`,
`/tmp/gitru-creation-budget-final-clippy.log`,
`/tmp/gitru-creation-budget-fmt.log`.

The first full frontend invocation completed879 passes/one skip but one existing
cached-issue-details test exceeded its default one-second initial list wait under
concurrent native load. The unchanged affected suite then passed7/7 in isolation.
A complete bounded-concurrency rerun (`bun run test --maxWorkers=4`) passed
**880 tests / one skip across102 files**, with no assertion or production change
for that startup timeout. All workspace lint/types and the uncached desktop build
also pass; fourteen affected composer tests include the two new definite-refusal
controls and the unchanged unknown-IPC identity controls. Logs:
`/tmp/gitru-creation-budget-frontend.log`,
`/tmp/gitru-creation-budget-issue-view-delta.log`,
`/tmp/gitru-creation-budget-frontend-bounded.log`,
`/tmp/gitru-creation-budget-lint.log`,
`/tmp/gitru-creation-budget-workspace-types.log`,
`/tmp/gitru-creation-budget-build.log`.

This is a correctness follow-up stacked on PR199, not a duplicate implementation
of R120 or completion of all R134 metadata-authoring scope. The schema remains23;
public command/type shapes are unchanged. Remote CI starts at publication and
must be assessed at its own exact head. No packaged UI run, authenticated numeric
mutation compatibility or live provider write is claimed. No PR was merged.
