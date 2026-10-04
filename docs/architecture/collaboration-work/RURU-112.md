# RURU-112 — Bitbucket Cloud account and repository slice

Status: first bounded implementation slice accepted on 4 October 2026, before
major code changes. This note supplements `remote-collaboration-engine.md`.
RURU-112 stays In Progress until subsequent pull request and native facet slices
meet its remaining acceptance criteria. This slice does not complete RURU-112.

Base: reviewed, unmerged R111/#159 at
`ebb8ae10a2ae90cc3f0fa50176f2f88b64a756dc`. Its repaired exact-head Linux Rust job
111417591547 passes; Windows ordinary packaged E2E is still running when selected.
R76/#146 and R100/#149 prerequisites are reviewed stacks. No merge is authorized.
No duplicate issue branch or PR existed in the live audit. The isolated managed
worktree is `/Volumes/Lexar/.codex/wt/collab-ruru-112/gitru`; branch
`ruru/ruru-112-bitbucket-accounts`. R103/#157 and R121/#158 remain separate sibling
stacks; their fixture/prefetch code is not required here.

## Current provider evidence

Rechecked official sources on 4 October 2026. The
[Bitbucket Cloud API changelog](https://developer.atlassian.com/cloud/bitbucket/changelog/)
records API-token Bearer authentication on 18 August, native issue tracker API
removal on 20 August, and app-password final removal on 28 July 2026. Prefer these
dated changes over older REST introduction/support examples. Manual scoped API
tokens therefore require only a token, with no Atlassian email or cloud broker.
An API token is distinct from repository/project/workspace access tokens.

[Token usage](https://support.atlassian.com/bitbucket-cloud/docs/using-api-tokens/)
and [token permissions](https://support.atlassian.com/bitbucket-cloud/docs/api-token-permissions/)
define separately granted `read:user:bitbucket`, `read:workspace:bitbucket`, and
`read:repository:bitbucket`. The later PR slice requires its own PR read scope;
none of these scopes implies another. Read
[users](https://developer.atlassian.com/cloud/bitbucket/rest/api-group-users/),
[workspaces](https://developer.atlassian.com/cloud/bitbucket/rest/api-group-workspaces/),
[repositories](https://developer.atlassian.com/cloud/bitbucket/rest/api-group-repositories/),
[pagination](https://developer.atlassian.com/cloud/bitbucket/rest/intro/), and
[rate limits](https://support.atlassian.com/bitbucket-cloud/docs/api-request-limits/)
alongside those changes. Tests use synthetic actual HTTP fixtures; public
documentation is not a claim of live account/platform qualification.

## Scope and boundaries

Connect public Bitbucket Cloud accounts, verify immutable actor UUIDs, discover
member repositories, persist them through the existing single native runtime and
SQLite feed machinery, and read saved repositories offline through shared local
queries. Account management and disconnect use the existing account lifecycle.
Desktop provider accounts remain independent of Gitru cloud sign-in.

The adapter profile supports only Repositories in this slice. Pulls, details,
comments, reviews, checks and remote writes remain NotImplemented. Issues and
Inbox remain Unsupported/ProviderSemantics; `InboxSemantics::None` and
`notifications_supported: false` apply. Do not call the broader `read_only`
profile helper, which would advertise unsupported features. No enterprise host,
Data Center, OAuth, CLI credential import, app password, ambient credential read,
or native issue/inbox endpoint is introduced.

Later R112 chunks are: (1) all-state PR summaries and independently authoritative
Body/common metadata, (2) explicit versioned participant/task facets. Do not map
participants onto issue assignees or tasks onto comments merely to reuse a field.
Repository-local PR numbers will need repository UUID plus PR number identity;
an account-global bare PR number would collide. No mutation/merge readiness is
implied by any read capability.

## Native account and transport contract

`collaboration_connect_bitbucket_cloud(token: String) -> RemoteAccount` is a
main-webview-only generated command. The SDK exposes `connectBitbucketCloud(token)`
and the ordinary account dialog accepts one password input. Rust constructs the
fixed public ProviderInstance and owns sensitive Bearer HTTP and the existing
zeroized SecretToken, recoverable vault cutover, account/actor isolation and
authorization epochs. Do not add email, username, arbitrary URL, requested scope,
cloud session or token-kind arguments; response identity proves the actor.

Probe `/2.0/user` and validate `type == user`, a canonical UUID, and active account
status when supplied. UUID is identity; nickname/display name is bounded mutable
presentation. A different actor cannot replace the prior actor's credential.
Then read one `/2.0/user/workspaces` page; if a workspace exists, read its first
actual member-repository page. All required probes succeed before staging any
credential. Empty workspace membership is a valid account and does not prove
repository access. Quota-bearing failures after identity proof preserve the
proven actor through the existing ProbeFailure/cooldown path.

Production HTTP is pooled, fixed to `https://api.bitbucket.org/2.0/`, uses a
sensitive Authorization header, rejects credential-bearing/arbitrary host links,
does not follow redirects, and enforces existing finite request/body limits.
Validate provider-returned web/clone URLs and immutable workspace/repository UUIDs
before applying any row. Standard username-only HTTPS/SSH clone URL userinfo is
allowed only as bounded input during fixed origin/path validation, then discarded;
password-bearing clone URLs are rejected and no clone URL is executed, fetched or
persisted. Web/API/continuation URLs permit no userinfo. This preserves documented
username/static-token-user clone forms without retaining credentials. See the
[clone guide](https://support.atlassian.com/bitbucket-cloud/docs/clone-a-git-repository/)
and token usage guide above. Repository name/full-name/path and workspace slug may
change without changing identity. Unexpected 304 is invalid; no ETag or Last-
Modified consistency guarantee is assumed. Rolling-hour capacity/near-limit
headers are not remaining-request counts. Actual 429/Retry-After and documented
retry signals feed the account cooldown; do not hardcode a universal rate quota.

## Bounded repository discovery

The provider-owned account-global Repositories feed first fetches
`/user/workspaces`, then `/repositories/{trusted-workspace-UUID}?role=member` for
each returned workspace. One adapter `fetch_page` performs exactly one HTTP page,
including empty workspace pages. At most ten pending workspace UUIDs are carried
in a cursor; repository pages are capped at fifty rows. Workspace list pages are
requested with `pagelen=10`. Extra provider rows fail partial/invalid rather than
creating an unbounded queue. Complete means every accepted workspace/repository
continuation is exhausted; an empty intermediate page is not global absence.

Continuation URLs are opaque provider tokens. Validate fixed origin, expected
endpoint path, query size and preserved membership/size filters; never guess or
increment page values. The versioned private serialized cursor binds account,
authorization epoch, feed kind and stage, with bounded pending workspace UUIDs,
outer continuation and current repository continuation. Cap total encoded state
at 4096 bytes; reject oversize input/output without establishing coverage. Prefer
small limits for individual URLs so the aggregate remains bounded. Reject foreign
account/epoch/stage/path, malformed UUIDs, duplicates and self-loops. The shared
ten-page job budget only yields and resumes its durable cursor; it
does not detect repeated continuations or cap a full traversal. Therefore the
private cursor persists up to twenty accepted enumeration page receipts and full
SHA-256 continuation fingerprints, with at most twenty seen workspace UUIDs and
ten pending UUIDs. Individual URLs are limited to512 bytes; the serialized4096
byte limit is still enforced. Reject known repeated current targets and cap
exhaustion before HTTP across job resumption, manual refresh and cold reopen.
A loop proposed by an HTTP response is detected before applying that page; the
rejected page does not commit, so later bounded backoff may retry its current URL
but must never follow the repeated target or erase accepted history. Failed HTTP
attempts do not commit a page counter; existing bounded backoff remains
authoritative. It does not promise a lifetime retry-count limit.
The twentieth accepted page with remaining continuation commits Partial plus the
exhausted cursor, never Complete. Resumption cannot erase history or reset its
budget. This first slice deliberately cannot finish/rescan a capped large account
until a later explicit traversal restart/coverage policy exists; saved authorized
rows remain available and no absence is established. Capped/failed traversal
retains prior authorized cached rows and does not infer deletion or full coverage.

Existing feed transactions atomically apply rows, cursor and durable revision.
The scheduler owns admission, retries, bounded concurrency, cache invalidation
and cooldowns. Old authorization epochs cannot commit late data or quota state
into a replacement account. No network request runs inside a SQLite transaction.
No schema migration, second database/cache/scheduler, or TS provider HTTP layer
is expected. If an implementation discovers a genuine contract gap, update this
note before broadening the architecture.

## Ownership and verification

- Native owner: new `providers/bitbucket_cloud*`, provider registration, runtime
  connect entry point, native command/caller registration, actual HTTP/runtime
  tests. Root coordinates generated IPC only after signatures freeze.
- Frontend owner: manual Bitbucket form/account-manager and focused UI test;
  SDK client/index and transport fixtures with exact account scope. Existing
  repository picker/shared capability UI remains the consumer. No demand,
  prefetch, fixture, private draft or repository Git layer edits.
- Root: this contract, shared progress docs, serial typegen/Cargo coordination,
  independent review, signed scoped commits, PR publication/attachment and Linear.

Meaningful tests: actual HTTP Bearer/identity/type/status/probe/no-staging failure;
two actors with identical nicknames; token replacement/different actor rejection;
multi-workspace and multi-page opaque continuations, empty intermediate pages,
rename/transfer UUID continuity, malicious links, cross-epoch/cross-account
cursors, loops/oversize/capped coverage; actual Runtime-to-SQLite offline cold
reopen with zero HTTP/vault access; auth/access/rate/late-response fences with
cached repository isolation and private drafts preserved. UI covers token-only
submission, no automatic CLI/ambient lookup, errors/clearing, main-only controls,
successful account refresh and truthful unsupported-feature copy. Typegen must
produce the additive command and no unrelated schema removals. Run focused and
full native/client/UI checks, repository lint/types/build, default Clippy/fmt and
appropriate platform CI. Keep local, remote CI and live provider/vault evidence
separate. Never inspect personal credentials for unattended validation.

## Progress

4 October: live issue/dependency/PR/worktree/file-overlap audit complete; official
provider contract refreshed; managed worktree created; this pre-code contract
records the bounded first slice. Implementation and validation remain pending. Normal make typegen at the frozen
additive command signature passes114 commands; independent TypeScript AST
comparison preserves all285 prior Zod schema initializers, adding only the
Bitbucket connect-params schema. No generated file is hand-edited. Cursor audit
found the shared per-job budget is not a traversal/loop guard; the private durable
limits and response-loop retry boundary above were fixed before qualification.
