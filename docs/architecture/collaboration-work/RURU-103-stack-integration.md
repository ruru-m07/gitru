# RURU-103 — integrate the qualified native-webview lane into the current stack

Status: accepted pre-integration contract, 5 October 2026. Read the current shared
engine/backlog from attached R102 worktree first, then RURU-103.md; historical
qualification remains evidence for its exact original heads only.

## Live selection and bounded outcome

Existing attached draft [PR157](https://github.com/ruru-m07/gitru/pull/157) is OPEN
on signed4054a6c081aef47aeace489e01cc8c2cdcae1128, based on15663e11970838c3bca1710f16175886f117e95aabb, with all14 reported
checks successful including retained native plus ordinary packaged E2E on all
three platforms. Linear R103 remains In Review. Its clean managed worktree is
/Users/ruru/.codex/worktrees/collab-ruru-103/gitru. Its own-Webview SDK listener,
visibility resampling and retained harness are absent from current engine ancestry.

New attached draft [R102 clock slice165](https://github.com/ruru-m07/gitru/pull/165)
is signed36455f9fa72b46d7dd1c58e8d3368e08b598fc34 on Comments164, with final local
825Rust/3ignored and499frontend/55files plus full lint/types/build/Clippy/fmt.
Its exact-head remote CI is running separately. Comments164 passes11 reported
checks, with final Windows packaged E2E06:54:17UTC. No failed engine CI is being
bypassed; unrelated133/140 failures and primary untracked architecture work are
outside this lane. All managed worktrees are clean after R102 publication.

Advance EXISTING157 onto exact165 rather than creating a duplicate. A signed
local integration merge into the R103 branch preserves25 published signed
commits and their historical evidence, avoids rewriting remote history and
allows a normal fast-forward push after fresh qualification. This is integration
of review branches; no pull request is merged into dev or another reviewed branch.
Set157 base to ruru/ruru-102-clock-lifecycle when publishing the qualified update.
Do not infer old14/14 or local retained passes apply to the new head.

## Conflict and compatibility contract

Read-only merge-tree against actual165 finds eight marker-conflict paths: core
lib.rs, engine/backlog and all five generated command files. Auto-merging runtime,
Store, native IPC, SDK and generator is not independent semantic qualification.
Preserve all newer provider/Tasks/Comments/public exports, R102 accepted-epoch
live/durable checks and private Job marker, and existing R103 harness module.
Inherited R111 writer-lease repair is identical; do not duplicate it or restore
stale storage scope_sql_list without Tasks. R103 own-Webview revision listener
and startup visibility resample remain required. Keep ordinary global broadcasts
compatible and fixture/default build boundaries intact.

Root reconciles both documented implementation/evidence histories without deleting
old failed runs, adds current integration evidence, and owns generated/native IPC
plumbing and publishing. Core owner resolves only crates/collaboration/src/lib.rs
after explicit dispatch. Native and SDK/generator reviewers independently inspect
auto-merged semantics without edits/builds. Additional actual-failure edits require
clear file ownership and a recorded bounded contract. No parallel Cargo/build/
typegen or reentrant validation.

Regenerate every command normally via make typegen from combined Rust/generator
sources; never hand-edit or choose stale generated content. If the conflict-marked
cache prevents generation, restore exact165 generated files as a temporary clean
baseline, then regenerate before staging. Independent AST inventory must preserve
all165 exported schemas/aliases/functions and one event/Branch. Read-only expectation
is117commands,317schemas,257aliases,1event: +3fixturecommands/+26schemas/+14aliases.
Actual generation is authoritative; investigate unexplained changes/removals.
No migration/dependency/public shipping schema changes beyond the existing
feature-only harness contract. No new provider capability or fairness policy.

## Required fresh qualification and delivery

Root serializes normal typegen, full default make verify, core test-harness feature
checks and native collaboration-harness tests/Clippy. Run ordinary packaged
control plus the complete fresh retained pipeline with all five owned crash/
restart sessions, marker-validated synthetic root and finite controllers. Only
owned synthetic credentials and exact launch-owned PIDs are permitted; no keyring,
gh-token/CLI import, cloud or live-provider fallback. Port4445 must be available;
old shared-target artifacts are historical, not a fresh binary.

Do not weaken visibility, revision withholding, actual ResetRequired, dirty draft/
CAS, authorization/lease/epoch/head/privacy fences, bounded gate/timeouts or
crash checkpoint requirements. If Mac documents are actually hidden, record that
evidence and continue other independent gates rather than faking visibility.
Harness clocks remain coupled and do not extend165's independent-clock claims.
No portable OS suspend/power-loss/live vault/provider/latency guarantees follow.

After concrete source review and actual gates, signed integration/result commits,
normal push and existing157 base/body/Linear updates record exact head, source
ancestry, local results and fresh remote matrix separately. Keep attached existing
157, inspect actual new CI failures before more work, and never merge PRs.
R99 recovery/export integration still precedes destructive R104 retention.


## Actual generated-import failure and accepted bounded repair

Normal generation117 commands and independent AST comparison preserve all165
schemas/aliases/functions, adding only the existing fixture definitions317/257/117/1.
Full combined make verify then stops in frontend tests:31 files fail at import,
33 pass,308 tests pass/1platform skip. Native payload injection inserts TaskV1Schema
with eager DetailValueSchema/DetailValueStateSchema dependencies before their
initialization. AST equality ignores declaration order; it does not prove module
execution. This is an actual new integration failure, not a dismissed test.

Before repair, root accepts a narrow source-generator correction: use the existing
TypeScript parser to order generated exported schema declarations by their actual
value dependencies after all Rust-derived corrections. Preserve every declaration/
initializer/type/API token and ordinary helper/import statements; type-only order
is irrelevant. Stable order for unrelated schemas; fail clearly on unsupported
cycles/duplicate declarations rather than inventing new lazy wire schemas.
Implement in a small scripts helper with meaningful shuffled native-payload
regression and executable import qualification; never edit generated outputs.
Root owns docs/normal generation/gates; generator owner owns only
scripts/collaboration-bindings.ts and new scoped schema-order helper/test files.
No command/model/schema/native/runtime/fairness change is authorized by this
repair. Root reruns normal make typegen, complete baseline/qualified-fixture AST
comparison, actual generated module import and full make verify. Default/feature/
native pipeline remain required and unqualified until executed.


## Actual combined-tree source/default qualification

Normal generation plus source-order correction produces117commands,317schemas,
257aliases and1event. Independent complete AST comparison preserves every165
291schema/243alias/114command initializer/signature and all existing qualified
fixture additions verbatim; Branch fields unchanged. Actual Bun module import
executes317schemas successfully. Focused sorter tests initially reproduce the
real module TDZ (6pass/1fail), then all7 pass after normal regeneration. The shuffled
fixture executes local const temporal-dead-zone semantics; CommonJS export rewrites
would mask that test, so root removed only synthetic fixture export modifiers
before execution. No generated hand edits or new runtime wire semantics.

Final combined `make verify` exits0 after correction:825Rust/3ignored
(collaboration474/2),630frontend/1Windows-onlyskip/65files, full lint/types, fresh
desktop production build (cache miss,7.43s Vite), Rustfmt/all-targetClippy. Separately
serialized core test-harness feature493passed/2ignored and native collaboration-
harness app30passed. Feature Clippy, ordinary packaged and fresh retained native
pipeline remain pending. These gates qualify the combined byte tree before the
subsequent test-only165followup; not a new remote matrix or native window pass.

Independent source reviews preserve Runtime/Store exactly165 except the feature
module, identical single WriterLease/currentTasks scopes, fixture/default cfg
boundaries, own-Webview listener/visibility resample, both generator corrections
and current frontend selectors. Independent sorter audit finds0parse errors,
0eager schema forward references,0immediate-function invocations and only deferred
detailFieldFamilies helper use in actual output. The sorter qualifies these finite
generated shapes, not arbitrary executable plugin code.

PR165 old36455f9remote frontend failed one immediate Body assertion during epoch
refresh. Its published signed test-only correctiond08fa56 proves held replacement
Body privacy/draft retention then awaits the current receipt; full499frontend/lint/
types pass and fresh CI now has7success/4pending/no fail. Propagate that accepted
correction into this existing review branch before final source/native delivery.
Old failed-head and initial generator import failure remain evidence; neither is
called passing. No credentials or PR merged.

The signed R102 test-only follow-up `d08fa56c2bf0399b7f1a1fe5fd3ecbd03f9e1fc4`
is now integrated without changing native production code. The controlled
same-actor epoch case passes in this combined tree, and full frontend validation
passes 630 tests with one Windows-only skip across 65 files, plus lint and type
checks. This result covers the source tree before its fresh packaged/native
builds; remote checks for both PR heads remain separate.


## Final local qualification on the current stack — 5 October 2026

The signed integration head `8c8068b5e2c7022477c80a31f0667903b9ba1912`
contains the current R102 head and the retained R103 implementation without
rewriting the existing published history. Normal `make typegen` emits 117 commands,
317 schemas, 257 aliases and one event. Independent AST comparison preserves all
114 commands, 291 schemas, 243 aliases, the event and public `Branch` fields from
R102; the only additions are the three feature-gated harness commands and their
26 schemas and 14 aliases. The generated module imports all 317 schemas without a
temporal-dead-zone failure. Generated files were not edited by hand.

The final serialized default `make verify` exits zero with 825 Rust tests passed
and three ignored (collaboration 474/2), 630 frontend tests passed and one
Windows-only skip across 65 files, full lint and type checks, a fresh desktop
production build, Rustfmt and all-target Clippy. The feature lane separately
passes 493 collaboration tests with two ignored, 30 native app tests and feature
Clippy. The focused R102 epoch replacement control and the full 630-test frontend
suite pass after integrating its test-only timing correction.

The ordinary packaged desktop run passes both spec files and all three tests in
`artifacts/e2e/2026-10-05T07-55-13-449Z-52562`. The fresh retained pipeline uses
binary SHA-256 `3b36b25b9a002685cfac0dfe19e636dc0a08761f358939dfa3401277b5d388d0`
and passes every owned stage in
`artifacts/e2e-harness/2026-10-05T07-59-36-355Z-54899`: six main scenarios, the
before-commit crash checkpoint and fresh restart, and the after-commit crash
checkpoint and fresh restart. Every stage exits zero, uses distinct owned sessions,
and completes on an available visible macOS desktop. No personal credential,
provider account, Gitru cloud account or live provider endpoint was inspected.

These results qualify local macOS behavior at the exact integrated source head.
They do not qualify other platforms, live providers, keyrings, CodeQL or remote CI.
The existing draft PR must be rebased as a stack by changing its base to R102,
pushed by ordinary fast-forward, and independently pass a new exact-head remote
matrix. R103 remains In Review and unmerged. R99 recovery/export still precedes
destructive R104 retention work.

## Accepted current-parent conflict repair — 8 October 2026

Live PR157 at `ba45cbda093b4b178a0937441c1718691ce92e60` reports a merge conflict
against its current R102 parent `c0a9f2f352982f185f93d7c751c052c14d9c7761`.
A read-only merge-tree identifies only the two append-only architecture evidence
files as textual conflicts; runtime source merges automatically. Preserve both
histories and integrate the exact current parent with a signed merge, then run
fresh independent-clock and native test-harness feature tests plus strict Clippy.
This repair neither changes public DTOs nor requalifies live providers, native
vaults, OS suspend or packaged webviews. The old 14/14 remote matrix remains
attached to its old exact head. Publish to existing PR157 only after local gates,
record the new source head, and let fresh remote checks qualify the updated head.
