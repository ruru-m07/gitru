# RURU-53 staging bundle

Updated: 2026-10-09

Branch `ruru/ruru-53-stage` is a temporary integration branch based on `dev`
`ddaecfad99195f9ce57e46cd3b1bbc0bb02c666d`. It bundles the open local-first
collaboration stack for combined review and CI without merging the feature into
`dev`.

The ancestry-minimal input set contains the 16 independent open PR heads at the
time of assembly: PRs #157, #158, #182, #184, #185, #187, #188, #189, #196,
#198, #204, #205, #208, #209, #213, and #215. Those heads contain every other
open collaboration PR head from #141 through #215 (there is no #148). PR #213
is included separately because PR #215 contained only its older `942dcd06`
revision.

PR #215 supplied the newest integrated product tree. PR #213 was then merged to
apply its later packaged keyed-harness corrections. RURU-102 and RURU-107 were
reconciled with the newer integrated tree, retaining both bounded portable HTTP
fixture handling and the independent vault/obsolete-observation qualification.
The remaining divergent leaf heads were recorded with signed ancestry-only
merges because their product changes were already present in the integrated
tree; replaying their older snapshots would overwrite newer code. Unrelated
open updater, release, dependency, documentation, and Tauri migration PRs were
excluded.

Local validation on the assembled tree:

- `make typegen`: 176 generated commands; generated files committed.
- `make verify`: 934 frontend tests passed with 1 intentional skip; lint, type
  checks, desktop production build, Rust formatting and strict workspace Clippy
  passed; the Rust workspace test suite passed with only intentional fixture and
  platform helper ignores.
- All 16 maximal input commits are ancestors of the staging head.

Remote exact-head CI, packaged platforms, live provider authentication, and
production vault checks remain separate evidence. Linear currently has 44 of 49
direct children In Review, RURU-75 and RURU-107 In Progress, and RURU-109,
RURU-113, and RURU-135 intentionally deferred in Backlog. This staging bundle
therefore does not claim that every ticket is resolved and does not authorize a
merge to `dev`.
