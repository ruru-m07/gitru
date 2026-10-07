# RURU-136 — Pull request checkout through local Git

Status: In Progress in Linear; contract frozen before implementation and local
implementation complete pending review, 7 October 2026.
Baseline: signed RURU-96 `0eb71a5a39db2ae11a7af2a11d0cdd074443142e`,
which contains RURU-77. The implementation branch is
`ruru/ruru-136-pr-checkout` in the external managed worktree
`/Volumes/Lexar/.codex/wt/ruru-136-pr-checkout/gitru`.

Live issue: [RURU-136](https://linear.app/catra/issue/RURU-136/check-out-pull-request-branches-through-the-local-git-workflow),
“Check out pull request branches through the local Git workflow”. Its
prerequisites are RURU-77 and RURU-96. Both are present in this exact baseline;
review, integration and exact-head remote CI remain delivery gates.

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
result without worktree or branch mutation. Execution acquires the repository
transaction before its first durable/caller revalidation. It checks the same
authority again after a potentially slow fetch and immediately before the final
Git reinspection and switch. Dropping the guard at either admission gate cannot
change the worktree or branch.

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
  `refs/heads/<saved head ref>` from the verified existing endpoint. While the
  repository transaction is held, native code captures the selected effective
  URL, re-sanitizes and compares its ordinal and endpoint, rejects a configured
  `remote.<name>.vcs`, and binds a digest of its credential-free canonical form.
  That native-only digest includes a validated SSH username, so changing only
  the SSH account invalidates the plan even though usernames never cross IPC.
  HTTPS userinfo, passwords, queries and fragments are rejected; SSH/SCP
  passwords, queries, fragments and unsafe usernames are rejected. HTTPS users
  configure a credential helper, while SSH uses normal key or agent discovery.
* Each remote command addresses a random native-only `gitru-pin::<uuid>` alias.
  One scrubbed command-scope config pair maps that exact alias to the captured
  credential-free URL without putting the URL in argv. Git applies this rewrite
  once, so repository `url.*.insteadOf` rules cannot redirect the resulting URL;
  the exact nonce mapping also wins over a broader `gitru-pin::` rewrite. The
  command never addresses the configured remote name, so a concurrently added
  `remote.<name>.vcs` cannot select another helper. `protocol.allow=never`, an
  allow rule for only the expected built-in HTTPS or SSH transport,
  `GIT_ALLOW_PROTOCOL`, `GIT_PROTOCOL_FROM_USER=0`, and the fixed
  `--upload-pack=git-upload-pack` close the remaining helper-selection paths.
* The fetch uses a source-only refspec, an explicitly empty refmap, and disables
  tags, pruning, submodule recursion and `FETCH_HEAD`. Because it has no
  destination ref, it can import objects but cannot dereference or update a
  repository ref even if an external process creates or replaces a symbolic ref
  during transfer. Native code requires the advertised source OID to equal the
  canonical lowercase expected OID before and after transfer, then proves the
  exact commit object is available. A missing or moved provider head fails
  before checkout.
* If the commit already exists, execution performs no fetch. Checkout creates the
  new local branch at the exact OID or switches the exact existing branch.
* After the switch, Gitru verifies `symbolic-ref --short HEAD` and `rev-parse
  HEAD`. Only an exact branch and OID match produces a success receipt.
* Secret-sensitive fetch output and inherited Git trace destinations are
  discarded, and `SSLKEYLOGFILE` is removed. The sensitive runner removes
  inherited command-scope Git config, executable-path overrides, repository and
  object-directory redirects, namespace/replace/graft/shallow redirects and
  protocol overrides before installing its private pair. It forces
  `GIT_NO_REPLACE_OBJECTS=1` and `GIT_NO_LAZY_FETCH=1`, so replacement objects
  cannot substitute another tree and partial-clone reads or checkout cannot
  contact an unpinned promisor remote. `core.askPass` and inherited Git/SSH
  askpass programs are overridden with empty native values, terminal prompts and
  interactive credential-manager flows are disabled, and SSH transport is
  forced through the runner's constant `ssh -oBatchMode=yes` command and OpenSSH
  variant. HOME/global and repository configuration, `SSH_AUTH_SOCK`, standard
  OpenSSH config, known-hosts and default key discovery remain available, as do
  configured noninteractive HTTPS credential helpers. The UI receives bounded
  typed failures, never stderr, URLs, helper output, tokens or provider
  credentials.

Persistent supported-transport URL/configuration changes are caught by the
native URL identity, remote digest and final reinspection. An arbitrary external
change followed by restoration between observations is not claimed as globally
detectable. Network commands remain pinned and protocol-limited during such a
race, and the ref-free fetch guarantee does not depend on detecting it.

The repository watcher may publish normal Git change events after the operation;
this command does not create a second repository context or manipulate the JSON
repository registry. A successful receipt carries the durable local repository ID
so the frontend can navigate through the existing Git tab/session path.

## Frontend behavior

The saved PR detail adds a provider independent checkout action only when the
resource kind is a pull request and known Head metadata is available. The dialog
first lists currently authorized linked clones and their registered worktree
paths so duplicate clone names remain distinguishable. Selecting one asks native
code for a plan; it then shows the clone, source repository/ref, exact target
OID, current branch/detached state, target local branch, and whether confirmation
will fetch. Fetch and checkout are never triggered by selection, mounting,
navigation, or background demand.

Dirty worktrees, active operations, divergent existing branches, missing head
repository metadata, stale links, missing source remotes and moved heads produce
actionable states. Users can choose another clone or enter a different local
branch and replan. Confirmation is disabled for blocked plans. On exact verified
success the app navigates to the existing local Git repository registration. A
verified native receipt remains an explicit success even if repository opening or
navigation then fails; the consumed plan is not offered as a retry.

## Implemented slice

The local branch now contains the focused Git checkout model/service, fork-source
resolver, caller-bound Tauri plan registry and commands, generated IPC bindings,
provider-independent collaboration client methods, and the pull-request checkout
dialog. Planning retires an older token for the same caller/subject/clone before
all accepted replans, including blocked outcomes and safe planning errors after
identity validation. Client planning remains authorization-fenced; execution
trusts a successful native receipt so an account fence invalidation after the
local mutation cannot manufacture an ambiguous failure.

The final mutation path is: consume token; acquire the repository transaction;
reinspect the bound plan and credential-free transport identity; revalidate
caller, account epoch, link generation, metadata binding and registration proof;
optionally perform the pinned ref-free fetch; repeat the full authority
validation; re-read Git dirtiness, active operation, symbolic branch, current
OID, target branch and object availability; switch; then verify the resulting
symbolic branch and exact OID.

## Ownership and bounds

This worktree owns a focused pull-checkout model/service and tests in `crates/git`,
narrow endpoint-to-saved-repository matching in collaboration local links, a thin
Tauri plan/execute module and registration, generated command output, SDK wrappers,
the PR detail dialog and focused tests, and this record. It does not touch RURU-104
retention files, command outbox/delivery, provider HTTP adapters, account vaults,
clone flows, remote management, or root architecture/backlog files. Generated
`packages/commands` output is produced only by `make typegen` and never hand edited.

Bounds: at most 64 live plans, a two minute plan lifetime, 255 bytes per Git ref
or branch component, canonical lowercase 40/64-hex Git object IDs, existing
local-link remote bounds, and one repository command transaction per
confirmation. Tests use only temporary local repositories and synthetic saved
collaboration data.

## Verification evidence

Focused Rust tests cover same-repository and fork remotes, missing local objects,
exact and divergent existing branches, canonical and moved head OIDs, dirty and
detached worktrees, active operations, missing source remote, fetch failure,
branch switch failure, a symbolic-ref replacement during delayed fetch, ref-free
transfer, and post-command branch/OID verification. Tauri/storage tests cover
account epoch, link generation/remote digest, metadata revision and caller/token
expiry fences. SDK/UI tests cover plan-only selection, explicit fetch wording and
confirmation, blocked/stale states, alternate branch replanning, verified
navigation, registered worktree paths, post-checkout navigation failure, and
redacted failures.

Local evidence completed on 7 October 2026:

* `make typegen`: generated 112 commands successfully.
* `cargo test -p git --test pull_checkout`: 19 passed after review fixes,
  including queued-lock authority revocation, a worktree dirtied during delayed
  fetch, ref-free HTTPS fork/moved-head fetches, in-flight symbolic-ref
  replacement, raced `remote.vcs` and `ext::` rewrites with non-invoked marker
  helpers, inline-credential rejection before transport/helpers, SSH username
  rebinding, replacement-ref isolation, canonical OID rejection,
  component-length bounds and exact post-checkout verification.
* `cargo test -p git service::pull_checkout::tests::`: 2 passed; canonical
  credential-free HTTPS, SSH, SCP and IPv6 reconstruction plus inline-secret and
  unsafe-username rejection are covered.
* `cargo test -p git runner::tests::sensitive_http_401_never_invokes_inherited_or_configured_askpass`:
  passed against a local HTTP 401 fixture; neither inherited nor repository
  configured askpass helper ran.
* `cargo test -p git runner::tests::sensitive_`: 2 passed after review fixes
  (plus its isolated child fixture); the effective command environment scrubs
  inherited Git config/path/object/protocol overrides and `SSLKEYLOGFILE`, forces
  no-replace/no-lazy-fetch behavior, `ssh -oBatchMode=yes` and the OpenSSH
  variant, preserves HOME and `SSH_AUTH_SOCK`, invokes a configured noninteractive
  credential helper, and never invokes inherited or configured askpass programs.
* `cargo test -p collaboration checkout_source`: 2 passed.
* `cargo test -p gitru collaboration_pull_checkout`: 6 passed after review
  fixes, including canonical provider OID validation.
* `cargo clippy -p git -p gitru --all-targets -- -D warnings` and
  `cargo fmt --all -- --check`: passed.
* Collaboration client focused Vitest: 35 passed; package TypeScript and scoped
  Biome checks passed. This includes a successful native execution receipt that
  remains observable when the local authorization fence invalidates in flight.
* Desktop checkout/workspace Vitest: 27 passed; desktop and E2E TypeScript plus
  scoped Biome checks passed.
* Before the final native transport hardening, `make verify` passed the complete
  repository frontend test, lint, type-check, desktop build, Rust format,
  workspace Clippy and workspace test suite (45 frontend files and 374 tests),
  and `make test-e2e` passed both packaged macOS spec files and all three
  scenarios. The post-review native changes are covered by the focused Rust,
  Tauri, format and Clippy checks above; the full commands have not been rerun.

Not yet verified: Windows behavior, a live GitHub/GitLab/Bitbucket checkout,
credential-manager behavior against a real provider, exact-head remote CI, or a
built installer. These remain separate delivery evidence and must not be
inferred from the local checks above.
