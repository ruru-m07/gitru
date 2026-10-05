# RURU-136 — Pull request checkout through local Git

Status: contract frozen before implementation, 5 October 2026.
Baseline: signed RURU-96 `0eb71a5a39db2ae11a7af2a11d0cdd074443142e`,
which contains RURU-77. The implementation branch is
`ruru/ruru-136-pr-checkout` in the external managed worktree
`/Volumes/Lexar/.codex/wt/ruru-136-pr-checkout/gitru`.

Live issue: [RURU-136](https://linear.app/catra/issue/RURU-136/check-out-pull-request-branches-through-the-local-git-workflow),
“Check out pull request branches through the local Git workflow”, Backlog at
contract time. Its prerequisites are RURU-77 and RURU-96. Both are present in
this exact baseline; integration and exact-head remote CI remain delivery gates.

## Outcome

From a saved pull request detail, a user can choose an already linked local
clone, inspect a native checkout plan, and explicitly confirm the required Git
operation. Success means Gitru reread the current symbolic branch and `HEAD`
OID after Git completed and proved that both equal the requested local branch
and cached provider head OID.

This slice does not clone repositories, add or rewrite remotes, obtain provider
credentials, reset or force-update an existing branch, stash user changes, or
claim that stale cached PR metadata is current on the provider. Provider data is
the saved observation the user inspected; the confirmation surface shows its
validation state and exact abbreviated OID.

## Native plan and confirmation boundary

Use two generated Tauri commands. Planning accepts only bounded identifiers for
the current account/authorization epoch, PR subject, and authored local link.
Native code resolves the durable RepoManager registration, re-observes the
worktree and effective remotes, checks the current authorized link, and reads the
PR summary plus Body metadata from SQLite. It never accepts a renderer supplied
path, remote URL, remote name, source ref, source repository, or target OID.

The plan requires authoritative saved Head metadata with a repository identity,
branch ref and commit OID. It verifies that the PR is bound to the linked base
repository and maps a current fetch endpoint to the saved head repository through
the same exact instance/transport/path rules as local links. This supports a fork
only when that fork is represented by an existing effective fetch remote. A
deleted or unknown fork remains unavailable rather than falling back to origin.

A successful plan returns a short lived, single use opaque token and a safe DTO:
local clone display name, current branch or detached state, dirty/operation state,
source remote name, source repository/ref, expected OID, target local branch,
whether an exact local object and branch already exist, and whether confirmation
will perform a fetch. No raw URL, filesystem path, credential, command argv, or
provider payload crosses IPC.

The native token is bound to the calling webview incarnation and owner generation,
account actor/epoch/instance, link ID/generation, registration proof, semantic
remote digest, collaboration authorization view/revision, subject and metadata
evidence, expected source repository/ref/OID, current symbolic branch and `HEAD`,
dirty/operation state, target branch state, and expiry. Replanning retires the
older token for that caller/subject/clone. Confirmation consumes the token before
Git mutation and reruns every relevant read. Any changed link, remote, saved head,
authorization, worktree, branch, or active operation returns a stale/blocked
result without mutation.

## Git operation rules

`crates/git` owns checkout inspection and execution. It uses the existing
per-repository command transaction and RepoServices boundary. Branch/ref/OID
inputs receive native Git validation before use.

* A dirty worktree or active merge/rebase/cherry-pick/revert/bisect blocks a
  mutating plan. Gitru does not choose a stash or conflict strategy here.
* Detached HEAD is shown explicitly and may move only after a clean confirmed
  plan. A symbolic branch/current OID change between plan and execution is stale.
* An existing target branch at the expected OID is switched normally. An existing
  target branch at another OID is never reset, deleted, renamed or force-updated;
  planning asks for a different valid local branch name.
* If the commit is missing, explicit confirmation fetches only
  `refs/heads/<saved head ref>` from the verified existing remote into a unique
  `refs/gitru/pull-checkout/<token>` ref with no tags and without writing
  `FETCH_HEAD`. The fetched ref must resolve to the expected commit OID. A moved
  provider head fails visibly and the temporary ref is removed.
* If the commit already exists, execution performs no fetch. Checkout creates the
  new local branch at the exact OID or switches the exact existing branch.
* After the switch, Gitru verifies `symbolic-ref --short HEAD` and `rev-parse
  HEAD`. Only an exact branch and OID match produces a success receipt. Temporary
  refs are best-effort cleaned on every path and are never presented as branches.
* Secret-sensitive fetch output and inherited Git trace destinations are
  discarded. The UI receives bounded typed failures, never stderr, URLs, helper
  output, tokens or provider credentials.

The repository watcher may publish normal Git change events after the operation;
this command does not create a second repository context or manipulate the JSON
repository registry. A successful receipt carries the durable local repository ID
so the frontend can navigate through the existing Git tab/session path.

## Frontend behavior

The saved PR detail adds a provider independent checkout action only when the
resource kind is a pull request and known Head metadata is available. The dialog
first lists currently authorized linked clones. Selecting one asks native code
for a plan; it then shows the clone, source repository/ref, exact target OID,
current branch/detached state, target local branch, and whether confirmation will
fetch. Fetch and checkout are never triggered by selection, mounting, navigation,
or background demand.

Dirty worktrees, active operations, divergent existing branches, missing head
repository metadata, stale links, missing source remotes and moved heads produce
actionable states. Users can choose another clone or enter a different local
branch and replan. Confirmation is disabled for blocked plans. On exact verified
success the app navigates to the existing local Git repository registration.

## Ownership and bounds

This worktree owns a focused pull-checkout model/service and tests in `crates/git`,
narrow endpoint-to-saved-repository matching in collaboration local links, a thin
Tauri plan/execute module and registration, generated command output, SDK wrappers,
the PR detail dialog and focused tests, and this record. It does not touch RURU-104
retention files, command outbox/delivery, provider HTTP adapters, account vaults,
clone flows, remote management, or root architecture/backlog files. Generated
`packages/commands` output is produced only by `make typegen` and never hand edited.

Bounds: at most 64 live plans, a two minute plan lifetime, 255 bytes per Git ref
or branch component, normal 40/64-hex Git object IDs, existing local-link remote
bounds, and one repository command transaction per confirmation. Tests use only
temporary local repositories and synthetic saved collaboration data.

## Verification plan

Focused Rust tests cover same-repository and fork remotes, missing local objects,
exact and divergent existing branches, stale/moved head OIDs, dirty and detached
worktrees, active operations, missing source remote, fetch failure, branch switch
failure, temp-ref cleanup, and post-command branch/OID verification. Tauri/storage
tests cover account epoch, link generation/remote digest, metadata revision and
caller/token expiry fences. SDK/UI tests cover plan-only selection, explicit
fetch wording and confirmation, blocked/stale states, alternate branch replanning,
verified navigation, and redacted failures.

Run focused tests while iterating, then `make typegen`, scoped TypeScript checks,
Rust format/Clippy/tests, and `make verify` when the slice is stable. Packaged E2E,
exact-head remote CI, live provider behavior, and credentials remain separately
reported delivery gates.
