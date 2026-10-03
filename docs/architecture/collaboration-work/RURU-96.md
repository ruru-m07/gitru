# RURU-96: local Git repository links — design and implementation record

Status: root-approved contract saved before implementation, 3 October 2026.
Native and frontend source are implemented and locally qualified, including
actual isolated macOS native QA. Review-stack integration and exact-head remote
CI remain acceptance gates before completion.
Baseline: signed RURU-78 `863967f8dc55587c1f71061fd2267a1c139bed5e`,
draft PR #151, in the separate managed worktree
`/Users/ruru/.codex/worktrees/collab-ruru-96/gitru`, branch
`ruru/ruru-96-local-repository-links`. RURU-98 runs in an isolated sibling lane.

Live issue: [RURU-96](https://linear.app/catra/issue/RURU-96/link-local-git-remotes-to-collaboration-repositories-and-accounts),
“Link local Git remotes to collaboration repositories and accounts”, In Progress,
blocked by RURU-76 and blocking RURU-120/RURU-136. Full acceptance includes an
inspect/change/remove flow, explicit ambiguity choices, restart/rename behavior,
and navigation in both directions. A parser/database-only PR does not complete
this issue.

## 1. Outcome and source-backed fit

A user chooses which effective local Git endpoint and connected account represent
a cached collaboration repository. Gitru records that local intent durably.
Opening either side resolves the existing local repository and current authorized
provider cache; it does not fetch a provider API, modify a Git remote, fetch Git
objects, check out a branch, or infer credential permission from the URL.

Relevant current source:

* `crates/ipc/src/repo_manager.rs`: `RepositoryInfo.id` is persisted in
  `repositories.json`. Preserve this ID across display-name updates and ordinary
  restart. `create_repo_context` in `crates/ipc/src/commands.rs` creates a fresh
  UUID on each open; that context ID is never a link identity.
* `crates/git/service/query.rs::repository_origin` reads only the first origin
  URL. `crates/git/parsers/origin.rs` uses presentation heuristics; it cannot
  establish provider identity. Keep it out of link resolution.
* `crates/collaboration/src/providers/registry.rs`: `ProviderInstance::new`
  defines an exact HTTPS origin, port and base path. Account bindings are checked
  against `ProviderInstance::for_account`; a transport alias cannot change them.
* `crates/collaboration/src/storage/identities.rs`: account/instance-scoped
  immutable identities and durable multi-claim aliases already exist. Hidden
  competing alias claims continue to make a locator ambiguous.
* `apps/desktop/src/features/collaboration/workspace.tsx` currently holds account
  and repository selection in component state, and only uses selected repositories
  as a feed filter. Explicit navigation needs a validated target rather than
  hoping the default account/filter matches.
* `apps/desktop/src/store/session-store-slice.ts` and `types/store.ts` already
  carry a durable `repositoryId` and route path for Git tabs/sessions. Reuse that
  navigation path. Do not create a second local repository/context manager.

The architecture’s local-link paragraph (section 6) and section 23 remain the
repository source of truth. This issue record preserves the accepted contract
saved before implementation and records implemented scope and validation below.

## 2. Git remote observation and credential boundary

Add standalone `crates/git/{models,parsers}/remotes.rs` and
`crates/git/service/remotes.rs`, exposed as `RepoServices::remotes()`. This is
read-only Git functionality, independent of collaboration providers. The native
caller resolves the supplied durable repository ID through RepoManager, validates
the actual worktree root, and invokes the service on that trusted path.

Observe the effective remote configuration, including all fetch URLs and all push
URLs. Preserve order and distinguish the first fetch URL (Git’s actual fetch
target) from additional configured fetch alternatives. Push may target several
URLs; never collapse them into an `origin` assumption. Let Git apply configuration
includes, `insteadOf`, `pushInsteadOf`, explicit push URLs and worktree config;
do not reimplement Git precedence in TypeScript or a second rewrite engine.

Use NUL-framed Git config/name discovery and separate argv for
`git remote get-url --all -- <name>` / `--push --all`. A task-temp probe with the
current local Git confirmed this delimiter and dotted names; supported-platform
fixtures must retain that check. Read the relevant NUL-framed config inputs first, rejecting raw
control/newline bytes in names, URL values and URL-rewrite subsection bases before
interpreting line-framed `get-url` output. No output trimming/lossy UTF-8 conversion
may manufacture a valid URL. Reject non-UTF-8 or malformed framing with a fixed
typed error. Legacy `.git/remotes`/`.git/branches` entries need an explicit bounded
enumeration/validation path or a visible unsupported-configuration result; they
must not be silently omitted from a supposedly complete snapshot. This edge is a
required implementation spike, not a claim that the current runner solves it.

The existing runner captures stderr and can return it verbatim; it also buffers
output without a byte cap. Add a narrowly scoped secret-sensitive, bounded read
mode rather than wrapping raw returned error text after logging. Proposed bounds:
128 remote names, 32 effective URLs per direction per remote, 4 KiB per raw URL,
1 MiB aggregate stdout/config inputs, bounded discarded stderr, and a 5-second
whole observation deadline. Overflow returns an incomplete/unavailable snapshot;
it never publishes a truncated list as complete. An isolated prototype also
confirmed that one remote URL with an embedded newline produces two `get-url`
lines while NUL config retains the actual value, so line framing alone is unsafe.
The read mode disables Git trace
destinations inherited from `GIT_TRACE*`/`GIT_CURL_VERBOSE`, has null stdin, kills
the child on cancellation, and uses fixed error categories with no raw stderr or
argv/URL interpolation. Preserve normal Git configuration semantics. It runs no
SSH, credential helper, remote helper, hook or network command.

Raw URL bytes exist only in bounded native normalization scope and receive no
`Debug`/logging implementation that prints their content. The safe DTO contains
only canonical transport host/port/path, direction, URL ordinal, redaction flag,
and a sanitized display URL. Strip every userinfo component, query and fragment.
For an unsupported/unparseable form, display a fixed placeholder and reason;
never return the original string as a fallback. SCP usernames are transport
credentials, not provider-account identity, and are omitted from persistence/IPC.

Privacy requires adapting the existing origin boundary too: current
`RepositoryOrigin.remote_url` feeds `RepoSitoryStore.origin` and persisted
`RepositoryInfo.origin`. Reuse the sanitizer for new/refresh origin reads and
sanitize old persisted origins before returning them even with
`list_repositories(false)`. Rewrite the known `origin` property on normal safe
store access so old copies do not remain in Gitru’s own current repository JSON;
preserve all unrelated fields and IDs. Failure to parse discards that property’s
unsafe value. Do not change `.git/config`, git credential storage, clone transport
inputs, or claim historical backups/OS logs have been erased. The logger proc macro
currently records function name and duration, not arguments; do not invent an
existing logger leak. Test real DTO/error/JSON bytes rather than token masking
assertions only at the new link boundary.

Primary Git references:
[remote get-url](https://git-scm.com/docs/git-remote),
[URL/push/rewrite configuration](https://git-scm.com/docs/git-config), and
[SSH/SCP transport spelling](https://git-scm.com/docs/git-clone).
The first fetch URL and explicit push-url behavior come from Git, as do longest
prefix rewrites and the rule that explicit push URLs bypass `pushInsteadOf`.

## 3. Exact transport-to-instance mapping

The Git layer returns safe transport coordinates; `collaboration::local_links`
maps them through persisted explicit transport bindings. It does not use substring
host matching, SSH username, repository basename, or provider API DNS guesses.

A binding has `id`, exact `instance_id`, transport `https|ssh|scp`, exact normalized
host, effective port, transport path prefix, repository path layout, and a
configuration generation. HTTPS and SSH ports are separate namespaces. An explicit
default port may equal its scheme’s default; an arbitrary SSH port never aliases
an HTTPS port automatically. IPv6 and DNS normalization use one Rust parser;
credentials, percent-encoded separators, backslashes, control bytes, dot segments,
double separators and normalization across the installation base boundary are
rejected before a URL parser can silently clean them up.

Built-in exact public bindings may cover GitHub.com, GitLab.com and Bitbucket
Cloud’s normal HTTPS/SSH/SCP layouts. Every custom hostname/port/base path and SSH
host alias requires an explicit binding to an existing configured ProviderInstance.
An alias such as `github-work` is user-configured metadata; do not read personal
SSH configuration or run `ssh -G` (configuration may execute `Match exec`). Show
the mapped instance in the confirmation flow. Bindings do not supply tokens,
register an adapter, change account-instance association, or authorize network to
the transport host. A provider without an adapter can still have parser/cached
identity fixtures; this does not qualify its live account integration.

Path layouts are typed and tested: GitHub/Bitbucket Cloud owner-or-workspace plus
repository, GitLab full subgroup chain plus repository, and explicitly configured
Data Center HTTPS `scm/<project>/<repo>` vs SSH `<project>/<repo>` layouts. Remove
one terminal `.git` suffix, preserve case and all subgroup segments, and remove
only a segment-exact configured transport prefix. Do not lowercase a repository
path or guess a mapping from cached case variants; if exact cached aliases cannot
resolve it, report unresolved. Namespaces and `~user`/relative filesystem paths
that are not a configured provider layout remain unsupported. Percent spelling
which cannot be admitted consistently with the current ProviderInstance/alias
model remains unsupported rather than partially decoded into a different path.

Bindings with the same matching specificity and different instances are ambiguous
and cannot resolve by insertion order. Reject contradictory duplicate bindings;
more-specific matching is explicit and uses path-segment boundaries. A binding
edit invalidates affected previews and link eligibility. Changing it requires the
main-window local settings policy, separate from ordinary link selection in a
trusted content tab.

## 4. Link identity, eligibility and ambiguity

Persist user intent as:

`local RepositoryInfo.id + remote name + fetch/push direction + chosen safe URL
coordinates -> account_id + instance_id + immutable repository provider_id +
canonical repository entity_id`.

Remote name/direction/ordinal are useful presentation evidence, not repository
identity. Native IDs remain strings. Multiple accounts on one instance are
separate choices, even when their native repository ID is the same. A fork’s
fetch and push endpoints may resolve to different repositories; show both and let
the user choose. Do not silently prefer `origin`, the first account, a logged-in
SSH user, or an active tab. Multiple local clones of the same remote require a
choice in the reverse-navigation flow.

Resolve every candidate in one bounded SQLite read snapshot using current account,
instance, authorization epoch/view, repository visibility and alias claims.
Unauthenticated/disconnected/denied/inactive membership cannot expose repository
names, web URLs, bodies, or other cached provider fields. Include hidden competing
claims in the ambiguity decision without returning their metadata. An authorized
saved read remains possible when offline/rate-limited; link resolution does not
require synchronization capability.

Expose separate states: `linked`, `unresolved` (cache miss), `ambiguous`,
`unconfigured_instance`, `unsupported_transport`, `remote_changed`,
`local_repository_missing`, and `unavailable` with a safe reason. Retain a durable
link after access loss, grant replacement or local disappearance; local inspection
can return its locally authored opaque IDs/remote evidence and allow removal,
while the provider-derived presentation is absent. Distinguish local proof state
from provider access state instead of overwriting one with the other.

Repository rename keeps the immutable target; stored old aliases may resolve it
when unique, and the current authorized title/path comes from cache. If a remote
path is reused and has multiple native-ID claims, never pick the visible subset
or the previously chosen ID automatically. Mark the link ambiguous/stale and block
automatic navigation. The current resolver retains historical multi-claims; this
plan does not assume a refresh erases them. A currently unique native identity
must be established by a separately reviewed reconciliation contract before that
same ambiguous mutable URL can become an automatically usable proof. Choosing
among different accounts/remotes is supported; choosing which native repository a
currently ambiguous URL actually points to is not proven by local Git commits.
A separately requested repository discovery may resolve a cache miss;
link APIs must neither enqueue that request nor pretend a cached URL proves the
remote server’s current state. Show saved/cached proof semantics honestly.

## 5. Snapshot, CAS and lifecycle contracts

The native mediator owns preview tokens, not JavaScript. A preview binds a
durable repository registration, bounded sanitized effective-remote vector,
transport-binding generation, selected account actor/epoch/view, immutable target,
and current link generation. Return an opaque random token with a short bounded
lifetime (for example two minutes), scoped to the trusted caller’s tab/owner
generation. Confirm accepts token + chosen candidate ID, not arbitrary raw path,
account/URL DTO or caller-supplied remote fingerprint. A fresh preview is required
after restart or caller disposal.

Before confirm and navigation, resolve the durable repository registration again,
validate the worktree and re-enumerate effective remotes without using a stale Git
cache. Verify source config stability around the multi-command read; if relevant
config changes during it, return `RemoteChanged`, without mutation retries. The
Gitru per-repo transaction excludes its own concurrent commands, not external Git
or global/include config writers. Preserve this practical limitation: the observed
configuration is a local read proof, not an atomic lock over external files.

Hash only the canonical sanitized effective vector (names, order, directions,
safe coordinates and redaction presence), never credentials or raw URLs. A
credential rotation with identical transport coordinates need not invalidate the
link. Persist only that safe proof. In the SQLite writer transaction compare link
generation, bindings generation, current account actor/epoch/instance, current
repository visibility and the freshly resolved immutable target. Failed CAS
leaves old intent unchanged. Confirm/change/remove bump a durable generation only
after commit; commit-before-hint remains mandatory.

Bind the local registration to a native worktree identity: canonical worktree/Git
directory plus stable filesystem identity where available. A different repository
replacing the same path must require a new preview, not inherit a previously
confirmed link. For Windows/non-Unix use the platform’s native file identity; do
not invent an ephemeral Rust pointer or path-only identity. A cross-device move or
repair which changes this proof is explicitly re-confirmed. Ordinary display
rename, directory rename retaining identity after a trusted RepoManager relocation,
and restart preserve `RepositoryInfo.id`. If relocation support is absent, add a
native picker/validated registration update as a root-owned integration item;
never quietly allocate a new ID under a feature claiming rename survival.

Local removal uses `link_id + expected_generation`, needs a trusted local caller,
and deliberately does not require current provider access/credential epoch. It
removes only this authored link, never repositories, provider cache or Git config.
Selecting a replacement target goes through a fresh preview/confirm; no blind
upsert. A deleted RepoManager registration leaves an inspectable orphaned link
until explicit removal or deliberate reconciliation; there is no cross-file JSON/
SQLite transaction to pretend otherwise.

## 6. Proposed schema and event contract

Migration `0006_local_repository_links.sql` comes after the unchanged signed 0005.
Own new `src/local_links.rs` + `src/storage/local_links.rs`; no changes to resource
metadata/detail observations or provider fetching. Proposed bounded tables:

* `local_transport_bindings`: instance FK RESTRICT, exact matching coordinates,
  typed layout, positive generation; uniqueness prevents contradictory identical
  match rules. No credential, raw URL, remote helper command or SSH configuration.
* `local_repository_links`: UUID link PK; local durable repository ID; chosen
  remote/direction/safe coordinates and semantic digest; account/instance;
  immutable provider ID and canonical repository ID; registration proof; positive
  generation and authored timestamps. A unique key per local repository/remote/
  direction/chosen endpoint avoids duplicate intent while allowing multiple links.
  Reference the durable account/instance association RESTRICT, not disposable
  repository projections. Credential cutover/cache replacement must not erase it.

Per-link generation is CAS, while the existing runtime global revision is change
notification. Existing `change_log` requires an account FK and positive epoch;
ordinary link changes can use its account’s current epoch even when disconnected
because the content is locally authored. Scope `local_link:<local_repository_id>`
invalidates link queries. Binding edits record one change per affected account in
the same transaction; no foreign fabricated/sentinel account. If truly unbound
instance configuration is desired, extend the event contract explicitly rather
than hacking around this account-bound schema. Initial scope can require an
existing registered instance, which is all linkable accounts today.

Link snapshots include their current global revision/authorization view plus link
generations. SDK query keys include local repository ID and account/instance/epoch
when provider metadata is projected. The single existing native/query bridge
invalidates on account resets, repository identity changes, local-link scopes and
transport-binding edits. Git config watcher events invalidate the safe observation
cache; explicit inspection/confirm/navigation always reads fresh. One coordinator
may refresh visible local proof on app focus; no per-panel interval or provider
HTTP. Global/include config changes which are not watched are caught by mandatory
intent-time reobservation.

Migration fixtures must prove existing drafts, identities, credential journals,
detail metadata, epochs and revisions survive v5→v6. RURU-106 currently refuses
schemas beyond its explicit allow-list; do not relax that gate as a side effect.
The new authored tables need a separately reviewed backup/restore policy before
R106 can accept this schema. Imported transport/link records never authorize
credential use or delete anything from the local vault. Preserve newest authored
link evidence with the original bundle when older backups replace it.

## 7. Root-owned native commands and actual product flow

Suggested generated native operations (names may follow current conventions):

* `inspect_local_repository_links(local_repository_id)` → safe effective remotes,
  current authored links, bounded authorized choices and opaque preview.
* `confirm_local_repository_link(preview_id, candidate_id)` and
  `remove_local_repository_link(link_id, expected_generation)`.
* `local_repository_links_for_resource(account_id, instance_id, repository_id,
  inspected_epoch)` → bounded linked local registrations, respecting current
  provider authorization and immutable target identity.
* `prepare_link_navigation(link_id, expected_generation, direction)` → a safe
  typed native navigation target after fresh local/SQLite validation.
* main-only transport-binding inspect/change/remove commands if settings UI is
  needed. No arbitrary path URL inference command in untrusted webviews.

Authorization reuses existing local origin/main-or-tab-webview policies. A content
tab can manage its local link; global trust binding configuration follows main
window policy. Caller owner-generation checks happen before/after asynchronous
work, so closed/replaced tabs cannot complete a mutation or navigation.

Git → collaboration: add a real “Linked collaboration” action to the repository
chrome/menu. With no confirmed usable choice, show the remote/account chooser,
with unavailable/unsupported/ambiguous explanations and explicit manage actions.
With a chosen link, validate and navigate to a typed account+instance+repository
target. The destination must not silently fall back to another account or all
repositories. Show a repository-scoped saved view even if the repo is unselected;
an optional “Sync this repository” action is explicit, capability-bound and
separate. Do not silently select/sync as a navigation side effect. Remember the
target in validated route/session state, and reauthorize it on reload.

Collaboration → Git: provide “Open local repository” for an authorized repository
view. Ask the user which clone if several registered links exist. Activate/reuse
the existing Git tab/session or create it with `repositoryId` set to the durable
RepoManager ID, then bootstrap through the ordinary trusted context lifecycle.
This navigation never accepts a provider-supplied filesystem path. Missing local
registrations/path errors produce a local recovery/removal flow, not automatic
clone or checkout.

Inspect/change/remove links remain accessible from the Git side when the account
is disconnected or denied. No hidden provider names/text should appear in this
fallback. Use coss components and read local coss/react-useeffect skills when UI
implementation starts. UI state does not duplicate the native identity engine.

## 8. Meaningful validation and delivery ownership

Native lead scope, once root approves: new Git remotes service/models/parsers and
secret-sensitive runner seam; new collaboration link DTO/storage module; 0006;
bounded identity/access reuse. Root owns RepoManager/caller integration, command
registration, generated bindings, SDK/query bridge and navigation UI. Delegate
an independent file of black-box parser/storage tests to this agent after concrete
public contracts freeze. Keep production core/model and test-file ownership
disjoint; reserve the execution wrapper before any shared target build.

Required fixture/native tests:

1. Isolated Git repositories/config HOME: all ordered fetch/push URLs, explicit
   pushurl vs rewrites, longest rewrite, global/include/worktree configuration,
   empty URL resets, remote names with dots and safe argv delimiter behavior.
   Snapshot config/worktree/refs before and after: observation changes none.
   No Git network/credential/helper subprocess is called. Corrupt framing/control
   input, legacy unsupported edge and resource overflow fail closed visibly.
2. HTTPS/SSH/SCP, explicit default/nondefault ports, IPv6, base-path boundaries,
   GitLab subgroup depth, Bitbucket Cloud/DC layouts and explicit aliases.
   Lookalike hosts, prefix confusion, encoded separators/dot segments, unknown
   host, overlap ambiguity and unsupported helpers/filesystem forms reject safely.
3. Put synthetic secrets in both URL userinfo, query/fragment and malformed
   error-producing config. Inspect serialized Git DTO, link DTO, errors, logger
   output and persisted current repository JSON/SQLite bytes. Values must be
   absent; legacy origin refresh-false returns no secret. Do not assert merely
   that the frontend displays asterisks.
4. Real Store public APIs: two accounts/instances and two remotes/fork identities,
   conflicting and hidden alias claims, inactive membership/permission denial,
   grant refresh/actor mismatch, epoch/CAS/link generation races, binding change,
   remote-vector/registration replacement, cancel/disposed caller, and SQLITE_FULL
   failure leave authored intent intact. Offline/rate-limited saved cache resolves
   without provider admission; denied cache hides values while removal succeeds.
5. Cold reopen with the same durable local ID; provider rename preserves immutable
   link and local display rename preserves intent. Reused path marks ambiguous,
   not newest-row-wins. Several clones remain distinct choices. Migration v5→v6,
   newer/dirty/checksum failures and draft/credential/detail preservation use the
   existing frozen migration suite; no changed old SQL/checksum fixtures.

Required caller/SDK/UI tests:

6. Native trust-origin/owner policy rejects arbitrary path, ephemeral context ID,
   foreign preview, stale actor/instance and disposed owner; content link removal
   still works with denied provider access. Settings trust changes remain main-only.
7. Real query observers and bridge hints cover commit-before-hint, reload/catch-up,
   account reset and local config invalidation. Metadata hidden after denial is
   removed from cached query state. No infinite/per-panel polling.
8. Actual user flows choose fetch/push and account explicitly; change/remove CAS;
   no fallback to default account/all-repos; correct unselected saved repository
   state; both navigation directions and multi-clone choice persist through reload.
   Denied/missing/stale/ambiguous views remain usable and removable, without cached
   private metadata. Assert zero refresh/hydrate/Git fetch/clone/checkout calls and
   zero provider HTTP caused by these flows with synthetic adapters.
9. One task-temp packaged native QA: configured synthetic two-account cache and
   two local fixture repositories; choose/link → Git-to-collaboration → reverse
   choose clone → restart → remove while disconnected. Confirm route/account/
   repository identity, UI affordances and unchanged Git state. No personal app DB,
   live token, user CLI or production API request. Full platform/CodeQL CI qualifies
   the exact published head; local checks do not imply remote success.

Implementation order: approve/copy this design → freeze Git safe observation and
local-link DTO seam → native parser/storage/migration plus independent fixtures →
root caller/generated/SDK/navigation → focused/full native and UI checks → actual
native fixture QA → signed scoped commits and root PR publication. RURU-96 stays
In Progress until the real navigation and ambiguity/inspection acceptance gates
pass. Future PR creation/checkout issues consume these links but add their own
command/credential/branch authority; this issue grants none.


## Root scope clarifications

Repository rename acceptance covers provider repository path/name rename and
local registration display-name changes, preserving immutable target and durable
RepoManager ID. Adding a new physical directory relocation UI is separate scope;
missing or replaced paths are explicitly unavailable and require a fresh proof.
Do not silently claim directory relocation support that RepoManager lacks.

Conditional enterprise/Data Center connection/layout spikes remain later work.
This slice may normalize already configured exact instances/base paths with
sanitized fixture evidence; it does not add an enterprise auth flow or qualify
Data Center. Public GitHub/GitLab/Bitbucket Cloud parser layouts and explicit
transport aliases are required. Unknown instances remain unresolved.

Respect existing repository selection and grant policies at navigation. Opening
a link must not select a repository, create sync demand or bypass a saved-read
gate. An unselected repository can show a scoped unavailable/empty saved view
with a separate explicit selection affordance. Root will integrate route targets
and inspect/change/remove/bidirectional navigation after the native seam freezes.

Native owner may implement the narrow existing origin privacy boundary and its
synthetic regression tests; root owns subsequent RepoManager registration/caller
integration. No live personal credential, SSH configuration, production database
or provider is inspected for validation. Schema 0006 must preserve authored
intent and inherited schema/migration checks without widening RURU-106 restore.


## Frontend and SDK validation

<!-- Frontend/SDK owner: foundation_frontend_review. Native owner edits other sections. -->

Plan accepted for implementation: expose the seven generated local-link commands
through the common SDK; inspection is a local Git/SQLite observation, clone lookup
is an account/actor/epoch/instance/repository-scoped saved read, and every link
mutation/navigation remains an explicit user intent. Native opaque preview and
link generations are the authority. Neither queries nor navigation call refresh,
hydration, clone, checkout, Git transport or a provider API.

SDK inspection keys include the durable RepoManager repository ID and local view
version. Any account reset clears global inspections before reread, since a
snapshot can contain several accounts. Account-bound clone keys capture actor,
epoch, instance and canonical repository ID. The existing durable bridge cancels
before invalidating local-link/binding or repository changes; the existing Git
watcher conservatively invalidates local observations because its context ID is
not a durable repository ID. No per-panel timer is added.

Git chrome opens an inspect/change/remove panel using the currently registered
RepositoryInfo.id, never the ephemeral native Git context ID or raw filesystem
path. Each safe endpoint/account choice is explicit. Unresolved, ambiguous,
unknown installation, unsupported transport, changed remote and missing local
registration remain visible; denied provider metadata stays absent while authored
links can be removed. A failed/expired preview requires explicit reinspection.

Git-to-Pulls/Issues stores the native-validated account, epoch, instance and
repository target plus local link/version in route state. Reload revalidates the
link and exact target. Wrong account/epoch/instance, missing repository or access
loss yields a scoped unavailable state with no fallback to another account or
all repositories. Unselected repositories keep this exact scoped target and
offer the existing explicit selectRepository flow separately.

Collaboration-to-Git opens an explicit clone picker, including for a single clone,
then native-validates the chosen link before updating the existing tab/session
repository ID and navigating through the normal Git context bootstrap. No
provider-supplied filesystem path enters navigation.

Advanced transport binding forms live only inside the existing main AccountDialog
host, behind a collapsed Repository transports section. This reuses its proven
child suspension/focus lifecycle. A content tab may request that existing host
with the current payload-free settings event; it cannot pass a mutation or
authoritative instance/account choice to main. The form itself explicitly
chooses a registered local repository and connected account/instance, observes
current binding generations and confirms typed transport/layout changes.

The frontend and SDK slice is implemented. Seven generated commands are exposed
through typed client methods and local query options. Global inspections are
fenced across account changes; reverse clone keys include actor, epoch, instance
and repository. The bridge cancels affected reads before invalidation for actual
`local_link:<durable-id>` and `local_transport_bindings` scopes. Local Git watcher
hints conservatively invalidate visible observations, while unrelated body and
private draft changes do not trigger new Git inspections. Native preview
supersession bounds repeated inspection; the renderer also retires each used or
failed opaque preview until reinspection.

Exact link targets persist only typed IDs in Pulls/Issues URLs. They use TanStack's
search serializer so opaque decimal epoch/generation strings survive reload
without JSON-number coercion or loss above JavaScript's integer precision limit.
Native navigation is checked before the account workspace mounts, then current
account epoch, installation and saved repository are checked independently. A
missing or inaccessible target cannot become an all-repository feed. Local
selection uses the existing authorized saved repository flow and remains
available during a provider quota cooldown; it is never automatic. A unique
native-authorized account choice remains usable when another account's hidden
historical identities conflict, while ambiguous accounts provide no candidates.

Executed initial frontend/SDK evidence before the packaged QA corrections below:

- Frozen Bun installation used `--frozen-lockfile --backend=copyfile` without
  dependency changes: `/tmp/gitru-ruru96-bun-install.log`.
- SDK: **61 tests across 8 files** passed, including 8 local-link behavior cases
  and 2 generated-wire/schema cases: `/tmp/gitru-ruru96-sdk-tests.log`.
- Desktop: **237 tests across 30 files** passed, including **22 local-link UI
  cases**: `/tmp/gitru-ruru96-desktop-tests.log`; focused results are in
  `/tmp/gitru-ruru96-ui-tests.log`.
- SDK typecheck and package Biome passed:
  `/tmp/gitru-ruru96-sdk-types.log`, `/tmp/gitru-ruru96-sdk-lint.log`.
- Desktop and E2E TypeScript checks plus scoped Biome passed:
  `/tmp/gitru-ruru96-desktop-types.log`, `/tmp/gitru-ruru96-ui-lint.log`.
- Desktop production frontend build passed:
  `/tmp/gitru-ruru96-ui-build.log`. Existing large editor bundle warnings remain;
  this is frontend build evidence, not packaged/native runtime qualification.

Tests use the real QueryObserver, generated command schemas, coss controls, Router
URL parsing and existing tab/session/repository store. They cover delayed global
and clone reads across a binding committed by another webview, account reset and
actor switch races, safe legacy/null observations, retired preview CAS, explicit
clone selection, both navigation directions, unselected repository isolation,
main-only mapping create/edit/remove, and payload-free child settings requests.
Operational native mocks fail closed. Refresh/hydration/selection calls remain
zero unless the test explicitly clicks the corresponding authorized action. The
jsdom fixture disables only its unsupported top-layer CSS selectors and stubs
scrolling; it does not replace production Select or Dialog behavior.

Task-owned native QA inputs exist under `/tmp/gitru-ruru96-qa/inputs.json` with two
real synthetic Git worktrees, isolated Git HOME/global configuration, matching
GitHub paths and separate SCP/SSH aliases. Native QA must seed public Store data
with synthetic actors, saved PR/issue data, quota barriers through 2099 and zero
credential references, then register these clones through normal RepoManager
commands/UI so native supplies durable UUIDs and filesystem proof. No QA seed is
committed and no personal Git configuration, provider credential or HTTP was
used to prepare the fixtures. Root owns native packaging and actual window QA.

The frontend/client read-only peer review accepted the corrected binding scope,
mixed-account ambiguity semantics and current authority fences. Actual native QA
and exact-head remote CI remain separate gates. The sibling R98 workspace/SDK
lease integration and R99 saved-draft recovery merge reconciliation remain root
integration work; this branch does not claim those sibling features are included.

Root's actual WKWebView QA identified three presentation regressions after the
initial freeze: the expanded settings dialog could exceed its available viewport
space and collapse the registered-repository Select; same-name clone choices did
not distinguish registrations; and the Origin display parser did not recognize
the native sanitizer's credentialless SCP/SSH output. The local corrections keep
the dialog header outside one bounded scroll body, use collision-aware anchored
placement only for the four transport settings Selects, and label native-returned
clone choices with the matching already-known AppStore registration path. An
unavailable registration shows its opaque durable ID instead. Same-name choices
inside the registered-repository Select also include their known local paths.
No provider-supplied path or presentation label establishes navigation authority.

The existing presentation-only Origin parser now accepts credentialless SCP and
SSH URLs as well as existing public HTTPS origins. It reconstructs the external
repository link without userinfo, query, fragment or SSH transport port; unknown
aliases remain unknown providers. Native Git observation, credential sanitization,
transport resolution and link confirmation are unchanged.

Post-QA local checks: **43 focused tests across 3 files** passed (**10 account
dialog host**, **23 local-link UI**, **10 origin presentation**), including real
coss Dialog/Select selection of the exact durable registration with duplicate
names, accessible path descriptions and the exact chosen reverse link ID, plus
synthetic credential/query exclusion:
`/tmp/gitru-ruru96-layout-tests.log`. Desktop/E2E types, scoped seven-file Biome
and the production frontend build passed in
`/tmp/gitru-ruru96-layout-types.log`, `/tmp/gitru-ruru96-layout-lint.log` and
`/tmp/gitru-ruru96-layout-build.log`. The final full desktop suite passed **249
tests across 31 files** in `/tmp/gitru-ruru96-desktop-tests-postqa.log`; the
**237-case** result above records the earlier pre-QA source. SDK code was
unchanged by these presentation fixes, so its **61-case** evidence remains
applicable. jsdom cannot qualify actual viewport/portal geometry; root's rebuilt
WKWebView QA remains the gate for these visual corrections.

<!-- End Frontend/SDK owner section. -->

## Native implementation and validation

The native implementation has a forward-only `0006` migration for authored links
and transport bindings, separate from disposable provider projections. Public
Store calls resolve one SQLite snapshot and fence account, actor, exact instance,
immutable repository identity, authorization view, binding generation and link
generation. Missing local registration, changed filesystem/configuration proof,
denied access and historical path reuse do not silently rebind a link. Removing
local intent remains possible when provider access is unavailable. Replacing an
account records change scopes for both affected accounts.

Effective remotes are read by bounded Git subprocesses with null stdin/stderr and
trace destinations disabled. Fetch/push URL order, includes, explicit push URLs,
longest rewrite prefixes, `pushInsteadOf`, dotted names and worktree config are
tested in an exited child with isolated synthetic HOME/global configuration.
Malformed configuration, unsupported legacy config and byte overflow return
fixed safe errors. A real blocked FIFO include proves the whole five-second
native deadline; cancellation releases the transaction for a subsequent read.
No personal Git, SSH, provider credential or vault data is used. Existing origin
return/storage boundaries share the sanitizer; the old JSON migration preserves
unrelated unknown fields and durable IDs. Raw credentials never enter the link
DTO or digest.

Empty inherited URL semantics follow the actual Git executable. The fixture
asserts either the newer exact reset output or the older exact append output,
including its empty unsafe-to-resolve entry; it does not guess from a version
string or manufacture an endpoint. Git's newer reset and older append semantics
are supported by the primary [current Git config documentation](https://git-scm.com/docs/git-config)
and upstream [Git 2.43 remote source](https://github.com/git/git/blob/v2.43.0/remote.c)
and [Git 2.54 remote source](https://github.com/git/git/blob/v2.54.0/remote.c).

Native registration proof includes the durable RepoManager UUID, canonical
worktree/Git/common-directory paths and stable filesystem identifiers. Unix uses
`dev/ino`; Windows uses an opened directory handle with
[GetFileInformationByHandle](https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-getfileinformationbyhandle)
volume/file-index evidence. Missing filesystem evidence fails closed.
Display-name changes do not change this proof; path replacement or another
registration does. These paths/IDs are native only and are not renderer-supplied
authority or wire responses. Physical relocation is not an implemented UI flow.

Seven native commands expose inspect/confirm/remove, explicit transport binding
save/remove, reverse clone inspection and validated navigation. Inspect creates
a random opaque preview with a two-minute monotonic lifetime and at most 64
pending previews. Reinspection by the same caller for the same durable repository
retires its previous preview. The registry retains candidate IDs and old link
generations rather than provider metadata. Confirm reobserves the registered
worktree and effective remotes; renderer input contains only the opaque preview,
candidate ID and optional replacement link ID.

Native Webview presence, exact local origin, repository-context owner generations
and an independent monotonic Webview incarnation bind the preview to the actual
caller. The ready/navigation hooks cover views without a Git context and revoke
old authority before foreign navigation. A retained native resource-table
allocation identity also rejects same-label/same-URL replacement before its ready
callback arrives. Ordinary successful host disposal already bumps the context
owner generation; the independent fence covers failed/omitted disposal and native
replacement outside that managed path. No renderer URL or label establishes this
authority.

All four authored mutations check current native ownership after acquiring the
SQLite writer and immediately before commit. The synchronous callback reads only
native presence/generation state; it performs no await, provider request or
UI-thread URL dispatch under the writer. A retired caller rolls back even if it
retired after the INSERT. Plain Store methods remain available to core callers.
Revision-only hints are emitted after commit. Binding writes retain main-window
policy; the exact-origin predicate rejects userinfo and unexpected ports.

Reverse clone probes have four concurrent slots and a whole-request deadline.
Unreadable registrations remain unavailable; current provider access is checked
again after probing. Navigation returns a typed target for the existing session
machinery. It never accepts a renderer filesystem path, selects a repository,
creates a clone, edits Git configuration or dispatches a provider API. An
unselected target reports `selected:false`; normal saved item/read policies still
apply. Explicit clone/link choice and both navigation directions are implemented
by the root-owned SDK/UI and require the separate packaged qualification below.

Executed local native evidence:

- Full collaboration suite: **172 passing cases**, plus two ignored subprocess
  entrypoints exercised by their parent crash tests, including all 12 frozen
  historical migration cases against actual `0006`:
  `/tmp/gitru-r96-collaboration-integrated-final.log`.
- Fourteen public link cases cover accounts/clones, restart, rename and path reuse,
  alias ports/prefixes/subgroups, IDs beyond JavaScript integer precision,
  historical hidden ambiguity, unselected data, denial/cutover, preserved drafts,
  removal and CAS invalidation.
- Four focused native Store failure cases cover actual SQLite interruption after
  an authored INSERT; caller retirement while blocked on the writer; retirement
  before commit; and all three removal/binding mutations at both boundaries.
  Each preserves authored state/generation/revision on failure. The final
  repository-only test fixture avoids including unrelated detail test modules:
  `/tmp/gitru-r96-storage-failure-frozen.log`.
- Full Git/IPC suite: **142 Git unit cases**, **179 Git integration cases** and
  **15 IPC cases**, plus the isolated remote child entrypoint:
  `/tmp/gitru-r96-git-ipc-tests-final.log`. Expanded real-Git semantics and deadline
  fixtures are separately rerun in `/tmp/gitru-r96-git-remotes-frozen.log`.
- Nine focused native caller cases pass, including same-label/no-context native
  replacement, pre-ready identity change, foreign navigation, bounded preview
  supersession, wrong caller and exact origin policy:
  `/tmp/gitru-r96-native-caller-tests-frozen2.log`.
- Final all-target Clippy for `collaboration`, `git`, `ipc` and `gitru`, with
  `-D warnings`, and workspace formatting checks passed and are recorded in
  `/tmp/gitru-r96-clippy-frozen.log` and `/tmp/gitru-r96-format-frozen.log`.
  Every shared-target Cargo command holds the outer execution lock across both
  compilation and execution; another worktree cannot replace a running fixture.

The independent native peer review accepted the final caller/store/parser source
and meaningful fixture boundaries. Windows filesystem proof and native plugin
hook behavior have source and synthetic-unit coverage here; this macOS local
suite does not claim Windows execution or packaged platform qualification.
Actual native QA and exact-head remote CI remain separate acceptance gates.
Synthetic GitLab/Bitbucket cache/parser cases do not qualify live provider
adapters. R106 retains its explicit supported-schema ceiling and does not import
`0006` implicitly.

Task-only `/tmp/gitru-ruru96-qa/seed.rs` uses public Store APIs for two synthetic
actors sharing an immutable repository. Account A is selected and B unselected;
PR #67 and issue #68 and authored drafts are cached; provider admission is blocked
until 2099. It creates no credential reference or authored link and accepts only
a new dedicated QA installation database. It has not been compiled/run by the
native implementation owner. Root registers the two synthetic Git worktrees
through normal RepoManager APIs/UI to produce real durable UUIDs and filesystem
proof, then owns packaged window/navigation/database verification. No manual
repository registration JSON or personal app database is part of this fixture.

## Root native QA and initial qualification

Root built the actual macOS desktop shell with the native `e2e` feature (test-only
vault and disabled CLI discovery) and the production frontend, which retains the
real native directory picker. Dedicated identifier `com.ruru.gitru.ruru96.qa`
and task-only Store seed keep personal app data, tokens and vaults outside this
qualification. The copied QA app has LaunchServices `LSEnvironment` pointing Git
global/system configuration to the task-owned empty file, with system Git config
disabled; no personal Git or SSH configuration was inspected. This environment
is confined to the task artifact, not a product setting.

Final pre-restack QA binary SHA256:
`58d4202c728aa11f0a33c16d4b7e973521be891cf813a24343fedec45eb0844f`.
Build log: `/tmp/gitru-ruru96-qa-native-build-final.log`; task seed/inputs and
read-only post-Quit evidence: `/tmp/gitru-ruru96-qa/`. The app was quit and its
local automation listener is absent. No personal credential or live provider
account was used.

Actual computer-use qualification exercised:

- Two real synthetic Git clones registered through the normal app import flow,
  giving durable RepoManager UUIDs and native filesystem proof; no manually
  written registration JSON. Effective HTTPS, SCP and explicit SSH push routes
  present account choices, including unresolved custom-alias mapping states.
- Explicit account-A link creation; cached PR/issue navigation and reverse
  opening of a registered clone; exact opaque IDs greater than JavaScript's
  integer precision remain quoted strings in route state.
- Link replacement to account B advanced its generation. The initially unselected
  B repository stayed unselected on navigation and showed a scoped explanation;
  only the explicitly clicked selection action exposed B's saved issue while the
  provider quota remained paused until 2099.
- Child settings requests suspend the child and mount main-window transport
  controls. All four transport Select popups display in the actual WKWebView.
  Creating, editing, removing and recreating the task SSH host/port/prefix mapping
  makes the push endpoint eligible without changing `.git/config`.
- Authored-link removal leaves saved content/drafts intact; separate account-A
  fetch and account-B push associations can coexist. Both link/mapping eligibility
  and the saved route survive a cold app restart.
- Two clones with identical display names expose distinct already-known local
  paths in the reverse chooser. Clicking clone A opens its existing Git session
  and persists A's actual durable selection. Sanitized SCP/SSH origins retain
  their ordinary display link, with no username/query/fragment returned.

Three real UI findings were repaired and the final source rebuilt: clipped
settings/dropdown placement, indistinguishable clone names, and sanitized SSH
origin presentation. The final 249 desktop tests, 43 focused cases, types, scoped
Biome and production build pass after those changes; independent review accepts
the delta. Native/storage/SDK contracts did not change during these UI repairs.

Read-only post-Quit SQLite/RepoManager evidence shows three authored links over
two genuine registrations, one explicit binding, immutable repository strings
`9007199254740997`/`9007199254740993`, four unchanged authored drafts at generation
1, zero credential references/cleanup entries, and both strict provider cooldowns
through 2099. B's selection is explicitly authored during this QA, not a claim
that the initial seed was selected. Both task worktrees remain clean and their
configured remotes retained. This is synthetic macOS qualification; Windows,
CodeQL, live provider/vault and the R106 schema-0006 restore policy remain their
own gates.

The next integration step restacks the signed R98 scheduler onto R78, then this
local-link range onto R98 before R79 persistence uses schema0007. Initial QA above
is scoped to the isolated R78-based implementation, not a claim of a combined
lease/link binary. The integration must preserve native lifetime fences, generated
DTO union, visible demand leases and exact-target navigation and requalify them.

### Combined scheduler/link qualification — 3 October 2026

The signed R96 range is now restacked onto R98 `2d415938`, which is based on
R78 `863967f`: cached PR/issue details → foreground scheduler → authored local
links. This supersedes the pending integration step above; its earlier isolated
QA and test counts remain historical evidence. Native source review confirms
the original link/proof/checked-write/migration0006 code and the parent's demand
runtime/storage/lifetime code are unchanged. Shared main/child command policies,
native registrations and the generated source pipeline contain both contracts.
The frontend retains ephemeral Body leases, including through link/binding
invalidation; account reset releases demand and fences delayed local navigation.

Combined local checks pass **188 collaboration tests** (two existing helpers
ignored), including all 12 frozen migration cases; **11 native caller cases**;
**93 SDK tests** in ten files; and **264 desktop tests** in 31 files. Workspace
all-target Clippy with warnings denied, Rust formatting, SDK/desktop/E2E types,
SDK/scoped UI Biome and the production frontend build pass. Normal `make typegen`
generates **110 commands** from the union of frozen native signatures. The
source pipeline trims a generated events-file trailing blank line; generated
files are never manually resolved or edited. Logs are
`/tmp/gitru-ruru96-combined-{native-tests,caller-tests,clippy,format}.log`,
`/tmp/gitru-ruru96-integrated-{sdk-tests,desktop-tests,sdk-types,desktop-types,sdk-lint,ui-lint,ui-build}.log`
and `/tmp/gitru-ruru96-restack-typegen.log`.

A fresh combined native app is built with production frontend and the isolated
E2E native vault/disabled CLI, confined to the same task-only Git config and
dedicated application ID used above. Build log:
`/tmp/gitru-ruru96-combined-qa-build.log`; implementation head `0c0a950`;
ad-hoc signed artifact binary SHA256
`ff71100f6677ab617ecec7e063a9e68742521b6c669c7588e0a37542101e4e58`.
The earlier app is preserved separately. Actual macOS computer use verifies:

- Cold-started clone A retains its authored link and opens the exact cached
  actor-A issue scope with opaque IDs preserved in route strings.
- The reverse chooser shows both identically named clones with their distinct
  already-known paths. Choosing B opens B's existing Git context.
- B retains both actor-A fetch and actor-B mapped SSH push links. Opening the
  push association reads actor B's saved issue/draft under its strict cooldown.
- Main-host settings suspend the child and show the saved SSH port/prefix
  mapping. Its registered-clone popup is visible and paths distinguish A/B.
  Closing settings restores the same actor-B detail and authored draft.

After Quit, read-only `/tmp/gitru-ruru96-qa/combined-post-quit-evidence.json`
confirms all **three links**, **one mapping** and **four generation-1 drafts** are
unchanged, both provider cooldowns remain strict through 2099, and credential,
cleanup and automatic durable detail-intent counts are **zero**. Native
RepoManager persists B as the actual clone selected through reverse navigation.
This is combined synthetic macOS GUI qualification, not live GitHub/vault or
Windows/power-loss/security qualification. R106's accepted restore ceiling is
unchanged; schema0006 recovery policy remains its own gate. Remote CI for the
forthcoming R96 publication is pending, and no PR is merged.
