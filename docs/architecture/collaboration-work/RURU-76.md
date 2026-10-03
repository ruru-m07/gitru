# RURU-76 — Provider dispatch, capabilities, and local identities

This slice builds on the collaboration foundation and RURU-95 credential
cutover. The main architecture document remains the product source of truth.
RURU-99 draft recovery, RURU-100 capability consumers, RURU-109 instance setup,
and RURU-110 live GitLab authentication remain separate slices.

## Concrete contract

- A provider registry binds an adapter to an explicit provider instance. Instance
  identity includes provider family and canonical HTTPS base URL, including port
  and installation path. Runtime work resolves the account's stored instance;
  there is no default adapter fallback for unknown hosts or providers.
- Existing accounts retain their immutable account IDs and receive a durable
  instance association through forward migration 0003. No historical migration,
  credential reference, epoch, draft subject ID, or draft generation is rewritten.
  Existing projection IDs remain opaque canonical IDs for compatibility.
- Resource identity uses provider-native stable IDs, separate from mutable paths,
  display numbers/IIDs, and URLs. Identity metadata and aliases are account and
  instance scoped. Repository rename/transfer adds aliases while keeping canonical
  identity and durable draft references unchanged.
- GitHub issue-side pull-request observations are explicitly marked as pull
  representations. They retain their issue endpoint identity and immutable
  repository ID plus number without creating a true issue row. If the canonical
  pull observation has not arrived, resolution is explicitly unresolved. A later
  pull observation binds those pending aliases transactionally to the existing
  canonical pull ID. True issues cannot merge with pulls by number.
- Locator reuse never overwrites a different immutable identity. Multiple
  authoritative resources claiming a historical path/URL/number return an
  explicit ambiguous result. Local resolution never starts provider HTTP and
  never reveals cached provider bodies. Account authorization and denied scopes
  gate resolved metadata; preserved aliases do not resurrect inaccessible data.
- Capabilities are typed resource facets with supported, unsupported, and
  unavailable states plus bounded reason codes. Supported adapter functionality
  remains distinct from currently missing credentials/scopes and unavailable
  adapters. Inbox semantics declare native notifications versus to-dos; to-dos
  never invent GitHub unread behavior. Existing notification flag remains a
  compatibility field until RURU-100 consumes the capability API.
- Registry capabilities are checked when scheduling and again at dispatch.
  Existing quota, epoch, credential journal, retry, and stale-page checks remain
  engine-owned. Adapters continue to expose typed pages, not raw HTTP.

## Planned files and sequence

1. Domain and registry: instance normalization/association, typed capability and
   resolver DTOs; preserve the existing single-adapter constructor as a narrow
   compatibility wrapper while production setup uses the registry constructor.
2. Forward migration 0003: provider instances/account association, canonical
   identity and locator/native alias tables, and pending endpoint observations.
   Backfill existing repositories/items using their existing canonical IDs.
3. Page commit: atomically record current aliases and bind pending endpoint
   observations alongside projections/change log. Contradictions become
   ambiguity; aliases survive cache cutover without becoming provider content.
4. Native local resolver/capability commands and account-bound TypeScript methods;
   regenerate commands with `make typegen`. No capability-driven UI changes.
5. Divergent GitHub and test-only GitLab fixtures use the same registry/page/error
   contract. Cover global IDs versus project IIDs, nested namespaces, installation
   base paths/ports, pagination, permission/rate/transient errors, and inbox
   semantics. No live GitLab support claim.

## Verification gates

Test issue-first/pull-first convergence, repository rename/transfer/path reuse,
true issue separation, same IDs across actors/providers/hosts/base paths,
restart and migration preservation, denied/disconnected access, and registry
dispatch without cross-instance credential routing. Run collaboration tests,
Clippy, formatting, generated wire/client tests, and relevant desktop type checks.
Coordinate frozen-v1 migration validation with RURU-105. Keep local checks
separate from remote CI and live provider integration gates.

## Sources checked

GitHub documents that issues endpoints expose pull requests with issue IDs rather
than pull IDs: [GitHub issues API](https://docs.github.com/en/rest/issues/issues).
GitLab separates resource IDs from project-local IIDs and exposes a distinct
to-do list: [merge requests API](https://docs.gitlab.com/api/merge_requests/),
[to-do API](https://docs.gitlab.com/api/todos/). Fixture normalization exercises
these differences; production GitLab connection remains RURU-110.

## Implementation evidence

The implementation now includes the registry and production setup switch,
forward migration 0003, transactional identity/alias observations, native local
capability/resolver commands, account-bound client methods/query options, and
generated wire schemas. Instance IDs are opaque strings derived from the
canonical provider/base-URL tuple; callers must not parse them. Ports and base
paths remain part of the trust boundary. Invalid credentials, URL query/fragment,
encoded/dot paths, and insecure installation URLs are rejected.

The canonical `provider_id` is a stable adapter-owned native identity key within
its resource kind and instance. Providers with repository-local native IDs must
compose that key with the immutable repository ID in their adapter. Display
numbers/IIDs and repository paths are separate aliases. Existing canonical
projection IDs remain unchanged; native identity contradictions fail the entire
page transaction. Path reuse makes both repository and child locators ambiguous,
including historical resource URLs beneath that repository URL.

Storage no longer gates the common inbox on the legacy GitHub notification flag.
Registry capabilities gate network admission and dispatch; local storage enforces
the account, epoch, selected scope, and access rules. Native read/unread filters
remain available, and to-do pending/done filters match remote state without
creating unread flags. Unsupported and missing-scope inbox requests never dispatch
an adapter call. Revalidating a denied scope remains an explicit refresh policy;
an observed capability snapshot is not authorization for provider writes.

Local resolution returns resolved, unresolved, ambiguous, or unavailable metadata.
Unknown aliases remain unresolved until a separate refresh/hydration slice brings
their authoritative identity onto the device. The resolver does not queue network
work. Endpoint aliases and canonical metadata survive cache replacement while
inactive/denied accounts cannot use them to recover provider content.

After stacking RURU-105 commit `d3889072`, all 83 collaboration tests pass: 37 unit,
6 identity, 12 migration, 3 dispatch, 9 runtime, and 16 storage tests. Two process
worker entry points are ignored in ordinary discovery and invoked by their
parent crash tests. The migration suite upgrades the frozen first-version
database through the actual migration 0003 and checks interruption, checksum,
dirty/newer-version refusal, and preserved drafts/epochs/credential metadata.
Native checks cover both representation arrival orders, rename/transfer/restart/
path reuse, actor/provider/host/port/base-path isolation, denied/disconnected
visibility, transaction rollback, divergent fixtures, bound credential dispatch,
and unsupported/unavailable inbox semantics.

All 25 client tests and 3 desktop command-caller tests pass on the stacked base.
The 171 desktop frontend tests passed before the base update; that update changed
native migration/security checks and SQL line-ending attributes. Desktop/client
type checks, core and desktop Clippy, formatting, and regenerated 93-command
bindings pass. No remote CI or live GitLab support is asserted.

The final stack uses RURU-105 commit `d776d663`; its changes from the tested base
are architecture/backlog documentation only, with identical code, migrations,
fixtures, and SQL line-ending policy.

The release `read_benchmark` example measures 200 local reads against 10,000
cached records on this development Mac. Identity resolution measured p50 59µs /
p95 78µs; a 50-row indexed list measured 221µs / 261µs, and an FTS phrase query
measured 4,364µs / 4,635µs. These are synthetic warm local-query measurements;
they do not include IPC/rendering or provider/network latency.
