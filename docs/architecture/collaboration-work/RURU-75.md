# RURU-75: production GitHub authentication qualification

This record qualifies GitHub manual PAT and explicitly selected GitHub CLI
imports against the production adapters. It must never contain a token, private
provider payload, credential reference, keychain password, or copied CLI
configuration.

## Why the ordinary packaged E2E build cannot qualify this

The `e2e` Cargo feature intentionally constructs an in-memory `TestVault` and a
disabled GitHub CLI adapter. That protects developer credentials during
unattended automation, but it cannot prove native keychain persistence or real
CLI behavior.

`bun --cwd apps/desktop run auth:qualification:build` builds the normal product
code without the `e2e` feature while merging
`tauri.auth-qualification.conf.json`. The resulting application has its own
identifier, `com.ruru.gitru.auth-qualification`, so its SQLite database, window
state, and native credential service are isolated from the installed Gitru
profile. The credential service is
`com.ruru.gitru.auth-qualification.collaboration`. The updater is disabled and
the build is not bundled.

This target is interactive by design. Automation must not inject a PAT, read a
CLI token, approve a keychain prompt, revoke a grant, or operate a real provider
account.

## Preconditions

- Use a dedicated GitHub test actor and a repository containing no private
  production data.
- Create two short-lived credentials for that same actor, labelled A and B.
  Keep them outside the repository, terminal history, screenshots, and notes.
- For inbox qualification, use classic PATs with the documented notification
  access. Fine-grained PATs can qualify repository, pull-request, and issue
  reads but cannot establish the inbox claim.
- Sign the test actor into GitHub CLI only if exercising CLI import. Do this
  before opening Gitru; Gitru must not run `gh auth login`, switch the active
  account, or modify CLI configuration.
- Record the Gitru commit, OS version, GitHub CLI version, build command and
  safe actor login. Do not record scopes beyond pass/fail for the expected
  feature.

Safe CLI state evidence can be captured before and after with the same command
Gitru uses for discovery:

```bash
gh auth status --hostname github.com --json active,host,login,state
```

Do not run `gh auth token`, add token-bearing debug logging, or use shell
tracing during this qualification.

## Qualification sequence

1. Build with `bun --cwd apps/desktop run auth:qualification:build`, then launch
   the generated release binary from this worktree's Cargo target directory.
2. Open **Connected accounts** in the main window. Confirm no Gitru cloud login
   is required and only the dedicated qualification profile is initially
   visible.
3. Enter PAT A manually. Confirm the connected login is the expected actor,
   choose the test repository, complete a sync, and open saved repository, pull
   request, issue, and inbox data covered by that credential.
4. Quit the process completely, disable networking, and launch the same binary.
   Confirm the account and previously saved views render without provider
   requests. Record which facets are saved and which correctly report offline
   or unavailable.
5. Restore networking and enter PAT B without disconnecting. Confirm it resolves
   to the same actor, the account remains singular, syncing succeeds, and stale
   work from PAT A cannot publish after replacement.
6. Revoke PAT B on GitHub. Trigger a foreground refresh and confirm Gitru moves
   the account to reconnect/auth-required behavior without erasing saved data or
   drafts.
7. Click **Check again**, select the dedicated actor's GitHub CLI row, then
   explicitly click **Use account**. Confirm the same actor reconnects and sync
   succeeds. Compare the safe CLI-state command output from before and after;
   active/login/state must be unchanged.
8. Disconnect in Gitru. Confirm saved non-secret data follows the documented
   disconnected retention behavior, remote operations are fenced, and a second
   launch remains disconnected. Confirm GitHub CLI still reports the same safe
   state.
9. Inspect only keychain metadata (never the password) to confirm the
   qualification service no longer has a live item after successful cleanup.
   On macOS, Keychain Access can filter by the exact service name above. Do not
   export the item or use `security ... -w`.

If revocation propagation is delayed, record the elapsed time and retry a
bounded foreground refresh. Do not mark the revocation step passed based only
on the upstream settings page.

## Evidence table

Fill this table only after running the sequence. Use `pass`, `fail`, or
`blocked`, plus a short secret-free observation.

| Check | Result | Safe observation |
| --- | --- | --- |
| Isolated production-path build | pass | macOS release binary built without `e2e`; output `target/release/gitru` |
| Manual PAT A verifies expected actor | pending | Not run in this record |
| Initial sync and cached views | pending | Not run in this record |
| Cold offline restart | pending | Not run in this record |
| Same-actor PAT B replacement | pending | Not run in this record |
| Upstream revocation becomes auth-required | pending | Not run in this record |
| Explicit CLI import verifies same actor | pending | Not run in this record |
| CLI active/login/state unchanged | pending | Not run in this record |
| Local disconnect fences work and survives restart | pending | Not run in this record |
| Native vault item cleanup | pending | Not run in this record |

## Platform coverage

Record macOS, Windows, and Linux separately because the `keyring` backend and
desktop session behavior differ. A pass on one OS does not qualify another.
Locked-vault and user-denied prompts are interactive platform checks and remain
blocked until exercised on each supported OS.

## Existing deterministic coverage

Unit, integration, and packaged harness tests already cover bounded CLI output,
explicit candidate selection, stale candidate rejection, secret redaction,
same-actor authorization-epoch replacement, hard-crash recovery around vault
and SQLite boundaries, offline cached reads, disconnect fencing, retryable
cleanup, and preservation of private drafts. Those tests support this live
qualification but do not replace it.
