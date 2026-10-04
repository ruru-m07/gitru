# RURU-112 — Bitbucket Cloud PR summaries and Body slice

Status: bounded contract accepted on 4 October 2026 before major code changes.
This supplements the engine architecture and RURU-112 account/repository work note.
RURU-112 remains In Progress until explicit participant/task facets are delivered.

Base: unmerged draft #160 exact `968cd5240f2d96f970a6637bb4a8445e5a815d39`,
all11 reported checks successful including ordinary packaged E2E/Rust on all three
platforms. No exact-head CodeQL check reported. R111/R76/R100 remain In Review
blockers; their implementations exist in this reviewed stack. No merge authorized.
New managed worktree `/Volumes/Lexar/.codex/wt/collab-ruru-112-pulls/gitru`, branch
`ruru/ruru-112-bitbucket-pull-reads`. Keep #160 unchanged and reviewable. Live audit
finds no duplicate PR for this remaining slice; R103/R121/R106 worktrees are clean.

## Sources and explicit inference

Rechecked official [PR API](https://developer.atlassian.com/cloud/bitbucket/rest/api-group-pullrequests/),
[OpenAPI](https://dac-static.atlassian.com/cloud/bitbucket/swagger.v3.json?_v=2.300.196),
[URI/UUID/pagination introduction](https://developer.atlassian.com/cloud/bitbucket/rest/intro/),
and [token permissions](https://support.atlassian.com/bitbucket-cloud/docs/api-token-permissions/).
PR ids are repository-local; the default list is OPEN only, so all states must be
explicit. PR read is a separate `read:pullrequest:bitbucket` grant, not implied by
repository read. The account/repository token probe remains unchanged and does
not require the PR grant or claim to verify it.

Atlassian documents UUID repository addressing with `%7B%7D` (literal empty braces)
in the workspace segment and the immutable repository UUID in the next segment.
Extending that documented repository URI to the documented pullrequests child is
an inference from the resource hierarchy, not a live-provider qualification.
Use that stable fixed route to avoid treating mutable workspace/repo display names
as identity. Do not use a double slash or guess a hidden workspace UUID. Actual
HTTP fixtures must prove exact encoding and destination repository binding. Live
provider/platform/keyring checks remain separate, with no personal credential read.

## Domain and capability contract

Implement selected-repository all-state PR summaries and singleton Body/common
metadata through existing Rust Runtime/SQLite and ordinary local SDK/Workspace.
Advertise Repositories, PullRequests and PullDetails only. Issues/Inbox remain
ProviderSemantics; comments/reviews/checks/writes remain NotImplemented. No new
DTO/schema migration, remote TypeScript client, scheduler/cache or permission gate.
Participants, tasks, task counts and approval state never map onto assignees,
comments or merge readiness in this slice; explicit later facets remain required.

Both projection id and native provider_id contain canonical repository UUID plus
canonical positive repository-local PR id; number remains the local decimal id.
Account/instance/authorization-epoch partitioning remains native. This prevents
PR67 in different repositories from colliding under existing identity uniqueness.
Reject wrong account, deselected repo, malformed/noncanonical identifiers, wrong
subject compound identity, singleton id/destination UUID mismatch before apply.
Validate condensed embedded repo UUID independently of full discovery fields;
missing/deleted source fork presentation can be omitted without erasing its ref.
Map OPEN to open, MERGED to merged, DECLINED/SUPERSEDED to closed. Retain declined
versus superseded in the summary reason and singleton StateReason when explicitly
observed; other unsupported provider-specific semantics remain unadvertised.

## Transport and bounded feed contract

Use existing fixed-origin sensitive Bearer/no-redirect/finite-body HTTP. Build
`repositories/%7B%7D/%7B<uuid>%7D/pullrequests` plus exact four repeated state
parameters, pagelen50, sort=id ascending. Only PR list routes admit that exact
state multiset; duplicates of other query keys or missing/added states remain
invalid. Continue only bounded opaque same-origin/same-repo/same-route links;
normalize equivalent bounded literal/lowercase/uppercase brace spelling and query
ordering for SHA256 history, never different paths, credentials or filters.

One dispatch fetches one page. Persist a versioned <=4096-byte cursor bound to
account, positive canonical authorization epoch and repository UUID, last positive
PR id, accepted-page count and <=20 full normalized URL fingerprints. Strictly
ascending ids and page-local identity uniqueness prevent replay/reorder coverage.
Accept at most50 values/page and20 total accepted pages across scheduler yields,
manual refresh and cold reopen. Reject self/multi-page loops before following the
old target or applying its proposed current page. Complete only on actual terminal
next absence; caps/loops/failed pages remain Partial and preserve authorized cached
rows/drafts. A capped traversal stays capped until a future explicit restart/coverage
policy, an intentional large-feed limitation; never claim total counts or offsets
prove coverage. This does not establish a provider-atomic snapshot. No ETag/304
semantics are invented. Quota-bearing mapping errors retain actual cooldown.

## Singleton field authority

List descriptions are bounded previews (<=16KiB) and never establish saved Body or
metadata authority. Singleton validates type, local id, destination UUID and saved
compound identity. Only the documented `rendered.description.raw` supplies Body
in this slice. Missing rendered/description/raw is Omitted; explicit raw null is
Known empty; bounded string (<=1MiB) is Known; a larger raw string is Oversized and
retains previous authority. No HTML or summary.raw inference. If a top-level
`description` is also present alongside canonical raw, require matching string/null
observations or reject the conflicting page; it is a consistency check, not fallback
authority. Newlines in raw Markdown remain allowed. Use a versioned named singleton
source; missing independent fields preserve prior saved values/evidence.

Validate common title/state/author/web URL/updated timestamp independently with
existing field evidence. Author uses immutable UUID and bounded nickname or UUID
fallback, with fixed safe web URLs if present. Unsupported labels/assignees/milestone,
draft and merged timestamp are Omitted; updated_on or merge_commit never becomes
merged_at. Head/base refs require nonempty bounded branch plus complete40/64-hex
OID; valid abbreviated hex is unknown/Omitted, malformed values fail safely. Base
can become Known only when this singleton also has an actual full source Head,
never None==None. Source/destination condensed repo presentation is optional,
validated when provided; it never supplies route authority. Existing captured
subject/head/access/source fences apply unchanged, including response races.

## Ownership and meaningful qualification

Native owner: Bitbucket provider dispatch/profile/transport and new feeds/detail
modules plus actual HTTP adapter tests. Independent test owner: new runtime
resource-read tests and test-only registration. Frontend owner: token help copy
and ordinary Workspace/Detail tests. Root: contract/shared docs, serial integration,
qualified signed commits and separate attached draft PR. No simultaneous Cargo;
all native/typegen/build work uses the existing serial wrapper and shared target.

Qualify actual HTTP -> adapter -> Runtime -> SQLite with two repositories sharing
PR67, same number across actors, rename/transfer with stable UUID, all-state reads,
condensed repos, hostile/aliased/loop/cap cursors including cold/job-yield boundaries,
scoped permission loss preserving other accounts/drafts, held old-epoch success
and quota failures, Body omission/empty/oversize/conflict and head/source races.
Cold saved reads must make zero HTTP/vault calls. Frontend uses ordinary singleton
SDK/bridge with generated IPC mocks, no direct fake component data path: selection,
offline metadata/Body, unsupported facets/no hydrate or write, one real Body
interest, compound identity isolation and private drafts. Full relevant frontend,
lint/types/build and native focused/workspace checks qualify the frozen source.
Remote exact-head CI is recorded separately; synthetic qualification is not live
provider/keyring validation. No generated hand edits or personal credential reads.
