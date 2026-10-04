# Remote collaboration engine architecture

Status: architecture accepted as the implementation direction. The initial
read-only foundation is in progress; section 23 records its verified scope and
remaining gates. Proposed later-phase contracts are not shipping APIs.

Research date: **2 October 2026**. Repository inspection: HEAD
`ddaecfad99195f9ce57e46cd3b1bbc0bb02c666d` (`dev`), implementation branch
`ruru/remote-collaboration`. Revalidate provider behavior and dependency versions
during implementation, particularly on self-hosted installations.

## 1. Purpose and recommended foundation

Gitru should make remote collaboration feel like native local data: opening an
already synchronized pull request, changing views, searching cached issues, and
reading the inbox should not wait for provider HTTP requests. Connecting several
accounts should produce one coherent experience without mixing their permissions
or hiding important provider differences.

**Recommendation:** build a Tauri-independent Rust collaboration service with
SQLite as the durable local read model, a persistent command outbox, a bounded
reconciliation scheduler, and typed local queries/subscriptions. Use the existing
React and TanStack Query stack as the presentation and bounded projection cache.
Provider adapters translate domain operations into provider-specific APIs.

The providers remain authoritative for shared PRs, issues, reviews, and server
notification state. Gitru owns drafts, local preferences, pending intent, cached
observations, and its own local change log. This is a local-first experience over
external authoritative systems; it cannot guarantee offline completion of every
remote operation or a globally consistent snapshot of a provider.

**Confirmed product requirement:** connected provider accounts work without
signing into a Gitru cloud account. Direct desktop authentication, synchronization,
and offline reads are the baseline. A hosted authentication broker or webhook
relay may be an optional feature; it is not on the normal read path.

**Proposed rollout assumption:** GitHub.com first, GitLab second, Bitbucket Cloud
third. Design instance identities for enterprise hosts from the start; claim
support only for tested server versions. The exact enterprise launch order is
still a product decision.

This document is the source of truth for future implementation. Change decisions
here before changing the architecture in code, record why, and update the
implementation record in section 23. API examples are proposed contracts, not
exports that currently exist.

Reading guide: sections 1–5 and 22 explain the decisions; sections 6–17 define
contracts and correctness; sections 18–20 cover operations and verification;
sections 21 and 23 are the implementation roadmap and continuation record.

## 2. What the current repository actually provides

| Integration point | Inspected behavior | Design consequence |
| --- | --- | --- |
| `apps/desktop/src-tauri/Cargo.toml`, `src/lib.rs` | Tauri **2**, Tokio, managed Git state, command registration | Add one process-level collaboration runtime beside Git state |
| `crates/git/lib.rs`, `context.rs`; `crates/ipc/src/commands.rs` | Git services/cache live under context IDs; opening a tab can create a new context | Account sync must outlive tabs and local clones |
| `crates/git/cache.rs` | In-memory TTL cache with single-flight/stale-on-error behavior | Keep it for Git; do not use it as the durable remote replica |
| `apps/desktop/src/state/core/state-manager.ts` | TanStack Query: five-minute stale time, thirty-minute GC; repository watcher and native focus refresh Git queries | Remote queries use the durable revision bridge; Rust owns provider refresh |
| `src/components/webview-tab-host.tsx`, `src/bootstrap/app-root.tsx` | Child webviews run separate JS runtimes and QueryClients | A TS singleton cannot be the application-wide scheduler |
| `src/state/domains/repository-state.ts`, `src/hooks/use-repository.ts` | Domain/query and component-hook separation already exists | Add a collaboration client/domain rather than expanding RepositoryState |
| `src/store/*`, `crates/ipc/src/repo_manager.rs` | JSON plugin store persists UI state/repository registrations | Keep preferences there; put domain rows, outbox, cursors, and jobs in SQLite |
| `scripts/typegen.sh`, `packages/commands/` | Rust commands generate Zod-validated TS wrappers | Generate new DTOs/wrappers with `make typegen`; never hand-edit this package |
| `src/components/sidebar/index.tsx` | Hard-coded account avatars and inbox badge `"5"` | Replace only after real account and inbox queries exist |
| `src/routes/app/pulls/index.tsx` | “Cooking pulls” placeholder | Introduce actual feature UI in `src/features/pulls/` |
| `src/routes/app/inbox/index.tsx`, `issues/index.tsx` | Development prototypes; production redirects to Git | These are not existing collaboration implementations |
| `src/routes/auth/onboarding/index.tsx` | Fabricated user/session | Build a real desktop provider-account lifecycle |
| `apps/api/src/auth.ts`, `db/auth-schema.ts`, `index.ts` | Better Auth/GitHub social login and waitlist; PostgreSQL account-token columns | Gitru identity is separate from provider authorization; do not reuse social tokens as desktop collaboration sessions |
| `docs/security/threat-model.md`, Tauri capabilities/CSP | OS credential store required; deny-default webview networking; narrow child permissions | Rust owns HTTP and secrets; new commands validate caller/account scope |

These findings describe the implementation base. Repository watchers are present
on `dev`; existing clone/pickaxe/updater progress events are
ephemeral and do not provide a durable domain subscription protocol.

## 3. Goals, boundaries, and correctness rules

Goals are fast local reads, useful offline behavior, provider-independent feature
code, explicit account isolation, bounded resource consumption, and recoverable
background work. Support repositories that are not cloned locally.

Initial non-goals are mirroring every accessible repository, downloading all
historical code and comments, multi-device synchronization of Gitru drafts,
arbitrary third-party executable adapters, or rebuilding provider permission and
workflow systems.

The following rules are implementation requirements:

1. A local query never waits for provider HTTP. An uncached record returns a
   meaningful missing/partial state; hydration is a separate operation.
2. A “queued locally” receipt means intent and its optimistic projection have
   committed durably. It does not mean the provider accepted the operation.
3. One Rust runtime owns synchronization and database writes across webviews.
4. Every query, job, validator, asset, and mutation has explicit account and
   provider-instance scope. No implicit “currently logged-in account” fallback.
5. Apply data, page progress, outbox changes, and local change metadata atomically
   when they represent one database transition.
6. Never perform HTTP, credential prompts, or expensive content parsing inside a
   database transaction.
7. Partial results, failed pagination, and capped searches do not prove absence.
   Unknown permissions and unknown fields do not become empty/false values.
8. Provider timestamps, opaque ETags, and pagination cursors are not assumed to be
   monotonic revision numbers or durable change-log positions.
9. Transport events are recoverable hints. SQLite state and its persisted revision
   are authoritative for local subscriptions.
10. A retry must preserve command identity. An ambiguous remote write is not
    automatically treated as failed or retried.
11. Pending operations are reapplied over confirmed base state in deterministic
    order; a refresh must not erase user intent.
12. Expired/revoked credentials stop writes. Jobs from an old authorization epoch
    cannot commit into the current account view.
13. Cache eviction, migration, and reset never silently discard drafts, outbox
    records, conflict evidence, or unknown delivery outcomes.
14. Unsupported capabilities fail explicitly. Provider-specific extensions cannot
    bypass scheduling, credential isolation, or mutation recovery.

## 4. Research and technology decisions

These are architectural judgments derived from the linked primary sources, not
claims that one technology is universally superior.

| Candidate | Current capabilities and fit | Decision |
| --- | --- | --- |
| SQLite + SQLx | Indexed local queries, transactions, FTS5, worker-backed async SQLite access, migrations | Preferred storage; explicit writer owner and small read pool |
| SQLite + rusqlite | Strong SQLite-specific access, bundled engine, backup facilities | Viable alternative with a dedicated blocking DB actor; select only if the spike shows SQLx friction |
| TanStack Query | Already integrated; bounded view caching and React lifecycle | Keep as frontend query cache; query functions read local Rust data |
| TanStack DB | Live queries/normalized collections; v0.6 adds persistence/offline support including Tauri SQLite | Evaluate later as a bounded Rust-fed projection layer; do not create a competing database writer/outbox |
| Electric | Syncs Postgres read paths through Shapes; writes remain application responsibilities | Revisit if Gitru owns a hosted mirror; does not remove provider ingestion |
| PowerSync | Syncs managed client SQLite against a backend source database with application upload logic | Same hosted-mirror tradeoff; not necessary for standalone desktop |
| Zero | Query-oriented sync for application-controlled backend data | Future hosted product option; it does not supply provider delta or mutation semantics |
| RxDB | Custom replication handlers and local reactive queries; checkpoint/conflict protocol | More flexible, but provider APIs do not directly satisfy its ordering/deletion/conflict protocol; no initial JS replica |
| Automerge/CRDTs | Merge concurrently edited state among participating replicas | Potentially useful for Gitru-owned notes/drafts; provider transition APIs do not consume CRDT changes |
| IndexedDB/WASM SQLite in each webview | Browser-oriented local persistence | Adds independent stores and lifecycle coordination to an already native app; omit initially |
| Embedded server database or Node sidecar | Can centralize persistence/provider SDKs | Extra distribution/process cost without an initial need |
| Hosted PostgreSQL mirror + sync service | Can amortize ingestion and push to devices | Adds sensitive data custody, tenants, provider auth, operational cost, and an availability dependency; defer |

Sources: [SQLite WAL](https://sqlite.org/wal.html),
[SQLx connection options](https://docs.rs/sqlx/latest/sqlx/sqlite/struct.SqliteConnectOptions.html),
[SQLx migrations](https://docs.rs/sqlx/latest/sqlx/migrate/struct.Migrator.html),
[rusqlite](https://github.com/rusqlite/rusqlite),
[TanStack DB overview](https://tanstack.com/db/latest/docs/overview),
[TanStack DB persistence announcement](https://tanstack.com/blog/tanstack-db-0.6-app-ready-with-persistence-and-includes),
[Electric](https://electric.ax/docs/sync/),
[PowerSync](https://docs.powersync.com/architecture/architecture-overview),
[Zero](https://zero.rocicorp.dev/docs/introduction),
[RxDB replication](https://rxdb.info/replication.html),
[Automerge](https://automerge.org/docs/hello/).

Linear's current engineering account describes an ordered application change log,
client checkpoints, permission/subscription filtering, and replay. Gitru can
adopt those invariants for its own local log. It cannot assume GitHub/GitLab/
Bitbucket expose Linear's server log. Linear's specialized hosted read index
solves a much larger server workload and is not a reason to add that dependency
to Gitru. [Linear delta sync engineering, August 2026](https://linear.app/now/rebuilding-delta-sync-read-path).

Choose SQLx + SQLite provisionally; phase 0 must verify bundled SQLite, migrations,
backup/recovery tooling, type generation, and packaged cross-platform behavior.
Pin tested versions in Cargo.lock and the Bun lockfile instead of prescribing
unverified “latest” dependency numbers here.

## 5. System structure and responsibility boundaries

```mermaid
flowchart TB
  UI["React features and hooks"] --> Client["Typed collaboration client"]
  Client --> IPC["Generated Tauri commands and subscription bridge"]
  IPC --> Query["Local query service"]
  IPC --> Command["Command admission and outbox"]
  IPC --> Demand["Subscription demand and refresh intents"]
  Query --> DB[("SQLite: confirmed base, effective views, intent, sync metadata")]
  Command --> Writer["Serialized transaction writer"]
  Writer --> DB
  Writer --> Changes["Durable change revision and log"]
  Changes --> IPC
  Demand --> Scheduler["Bounded sync and command scheduler"]
  Scheduler --> Adapters["Provider adapters"]
  Adapters --> HTTP["Pooled HTTP, rate budgets, authentication"]
  HTTP --> Providers["GitHub / GitLab / Bitbucket / future hosts"]
  Adapters --> Writer
  Vault["OS credential store"] --> HTTP
  Relay["Optional webhook relay"] -. "invalidation hints" .-> Scheduler
```

Keep the dependency direction:

- `crates/collaboration` owns domain models, provider contracts/adapters, storage,
  query planning, scheduler, outbox, normalization, and redacted errors. It knows
  nothing about Tauri windows or React.
- `apps/desktop/src-tauri/src/commands/collaboration.rs` and bootstrap wiring own
  IPC validation, calling-webview policy, channels/events, OS credential-store
  integration, browser OAuth callbacks, and lifecycle adapters.
- `packages/collaboration-client` (`@gitru/collaboration-client`) owns handwritten
  ergonomic handles, query options, subscription coordination, and React hooks
  over generated `@gitru/commands` DTOs. Split React exports from framework-neutral
  client exports.
- `crates/git` continues to own clone/fetch/checkout/worktree/diff operations.
  Collaboration-to-Git actions compose explicit operations; they do not bypass
  existing Git safeguards.
- Zustand/plugin-store keeps selected account, navigation, layouts, and
  preferences. It does not store provider entities, cursors, or tokens.
- `apps/api` is optional infrastructure only. Its existing sign-in service is not
  required for the desktop engine.

Start as one focused crate with modules. Split provider/storage/domain crates when
dependency boundaries, reuse, or compile cost justify it, not before the first
vertical slice.

## 6. Identity, provider instances, and account isolation

Distinguish four concepts:

| Concept | Identity | Example |
| --- | --- | --- |
| Provider implementation | Adapter family/version | github, gitlab, bitbucket-cloud, bitbucket-dc |
| Provider instance | Local UUID + canonical validated base URL, deployment flavor and optional API version | github.com versus github.company.example |
| Connected account | Local UUID + instance + verified remote actor ID | Personal and work accounts on the same host |
| Domain entity | Opaque local EntityId mapped to provider-native stable ID and scope | A PR that survives repo rename/transfer |

Provider instance identity includes host, port, and installation base path.
Credentials reference an account and a credential profile; one actor may need
different profiles for repository access and inbox access. Reauthentication
updates credentials for the verified actor; authenticating a different actor
creates a different account rather than repurposing a partition.

Use stable provider IDs when available. GitHub REST IDs/node IDs, GitLab project
IDs plus resource IDs/IIDs, and Bitbucket repository UUIDs/local PR IDs require
adapter-owned identity mapping. Never use login, email, repository slug, or URL as
the durable primary key. Do not merge users across providers by matching email.

Store aliases for URLs, repository paths, issue/PR locators, and different endpoint
representations. GitHub Issues and Pulls representations can expose different IDs
for the same PR; resolve the repository and number and attach both provider
identities to one PR. Never create a separate issue solely because it arrived
through an issues endpoint.
[GitHub issues behavior](https://docs.github.com/en/rest/issues/issues),
[GitHub pull requests](https://docs.github.com/en/rest/pulls/pulls).

`github:ruru-m07/gitru:67` is useful shorthand for a **locator**, not a primary key.
It omits actor and enterprise host, and the path can change. Prefer a structured
locator or ordinary provider URL at the boundary, then resolve to an EntityId.
For an uncached locator, return an unresolved handle and explicit hydration job;
do not disguise network resolution as synchronous `open()`.

Partition all provider observations by account, even if two accounts see the same
logical PR. Return account-bound entity views. A global identity registry may
link them for deliberate UI deduplication, but must not share bodies, permissions,
validators, or caches between partitions. Unified queries require an explicit
account set and retain the account on each item. Default to distinct actionable
rows if two actors have different notification or review state.

Keep `authorization_epoch` and account lifecycle generation. All jobs and commands
capture these; commit checks reject old generations after disconnect, scope
changes, or credential replacement. Disconnect cancels jobs, blocks commands,
deletes credentials, clears affected in-memory views and subscriptions, and
follows the chosen local-data retention policy. Discovered access revocation
hides the affected cached private scope and pauses its writes until access is
revalidated; revocation cannot be detected while completely offline.

Local repository linking is separate: `local_repository_id + remote_name →
provider_instance + remote_repository_id + preferred_account`. Parse HTTPS/SSH,
ports, GitLab subgroups, multiple remotes, and configured aliases in Rust.
`src/lib/parse-origin.ts` is currently presentation metadata and cannot be the
identity authority. Unknown custom hosts require explicit instance configuration;
a Git remote must not silently authorize sending credentials to an arbitrary host.

## 7. Unified domain models

Use a common semantic core plus explicit availability and typed provider facets.
Normalize immutable identity and relations; keep provider URLs/display paths
mutable. Shared values use opaque string IDs, UTC timestamps, and bounded text.
Keep large numeric provider IDs and local revisions out of JavaScript number
precision hazards by serializing them as strings.

| Model | Common fields and semantics | Important qualifications |
| --- | --- | --- |
| Account | instance, actor, credential profiles, scope observations, auth state | Gitru cloud identity is separate |
| Actor | ID, kind, display name, login, avatar asset reference | Users, bots, teams, deleted/unknown actors; email optional |
| Repository | ID, namespace/path, name, visibility, default branch, archive state | Namespace is not universally “owner”; access is account-specific |
| PullRequest | repo, number/display key, title/body, author, assignees, labels, state, draft state, source/target repo/ref/OID, times | Preserve merged versus closed; mergeability and review state are separately hydrated |
| Issue | repo/project, display key, title/body, author, assignees, labels, milestone, state/category, times | Provider workflow/status facet preserves richer states |
| Comment | subject, author, body, times, edit/delete permissions | Issue comments and diff comments are different kinds |
| Review and ReviewThread | reviewer, decision, commit anchor, thread, resolution, comments | GitLab approvals/discussions are not identical to a GitHub review object |
| Check/BuildStatus | head OID, source app, status/conclusion, URL, times | Unknown/pending/stale; do not imply the common model knows all merge rules |
| Label/Milestone | stable provider identity, name, color, description, scope | Team/project/repo semantics vary |
| InboxItem | account, source, subject reference, reason, activity time, remote/local read disposition | Native notification, actionable todo, and locally derived activity remain distinct |
| TimelineEntry | subject, kind, actor, time, versioned payload | Unsupported events retain typed extension/fallback display |
| Draft | account, subject/temp ID, body, expected anchor, generation | User-owned durable state, never ordinary cache |
| CommandReceipt | command ID, local revision, delivery state, conflict/error | Stable across restart and retried IPC submission |

Use `ValueState<T>` for values where missingness changes behavior:
`known(value)`, `not_loaded`, `unsupported(reason)`, `unavailable(reason)`.
For collection facets, add loaded range, completion, and last validation metadata.
SQL can store tagged value/status columns; the public DTO need not expose storage
representation. A PR summary must not null out a previously loaded body.

A source observation includes facet name, projection/field mask, endpoint/API
version, validator, provider updated time if meaningful, received time,
`validated_at`, schema/adapter version, and fetch generation. Different endpoints
have different authority for different fields. Preserve provider enum values in
typed facets so a new remote value becomes an unknown fallback rather than a
panic or an invented mapping.

Keep provider facets in versioned namespaced tables/DTOs, for example
`gitlab.approval_rules.v1` and `bitbucket.tasks.v1`. Core feature code uses
capabilities and common fields; specialized panels can narrow to a provider facet.
Avoid an untyped `Record<string, any>` becoming the real model.

### Notification semantics

A common inbox is a presentation aggregate, not an assertion that all providers
offer the same notification API. Keep server read/done state separate from
Gitru-local dismissed/snoozed/bookmarked state. Do not promise remote “mark
unread” when the provider only supports mark-read. A local dismissed item
reappears according to a persisted latest-activity fence, not whenever a poll
rewrites it.

GitHub notifications are threads and specify conditional polling and a minimum
poll interval. Current endpoint documentation states classic PAT authentication
and excludes GitHub App/fine-grained token support. The implementation also
recognizes existing OAuth credentials imported from GitHub CLI, but only when
`/user` reports the `notifications` or `repo` scope; GitHub documents those OAuth
scopes separately. Live notification behavior for each credential type remains
an integration-test gate. Unknown token types and unobserved scopes remain
unsupported rather than being inferred from a successful account connection.
[GitHub notifications](https://docs.github.com/en/rest/activity/notifications),
[GitHub OAuth scopes](https://docs.github.com/en/apps/oauth-apps/building-oauth-apps/scopes-for-oauth-apps).

GitLab todos are actionable items, not a full notification mirror. Their adapter
and UI should say “todos” where that distinction matters.
[GitLab todos](https://docs.gitlab.com/api/todos/).

Bitbucket Cloud's native issue-tracker endpoints were removed on 20 August 2026.
Its issue capability is unsupported. Jira or another external tracker is a
separate future connection, even when linked from a Bitbucket repository.
[Bitbucket Cloud changelog](https://developer.atlassian.com/cloud/bitbucket/changelog/#20-august-2026).

No supported general native notification-inbox API was established for Bitbucket
Cloud or Data Center in this research; mark that capability unsupported in the
initial adapters. Any derived activity uses `source_kind: local-activity` and
Gitru-local disposition, distinct from `native-thread` and `native-todo` items.
Data Center native issue CRUD is likewise unsupported; an external issue tracker
requires its own connection. Capability declarations are per item/operation.

## 8. Provider abstraction and capability contract

Use explicit resource operations and synchronization primitives, rather than one
giant CRUD adapter or an arbitrary provider HTTP escape hatch.

```rust
// Illustrative contract; exact async trait design is chosen in phase 0.
trait CollaborationProvider {
    async fn probe(&self, ctx: &RequestContext)
        -> Result<ProviderProfile, ProviderError>;
    async fn resolve(&self, ctx: &RequestContext, locator: Locator)
        -> Result<ResolvedIdentity, ProviderError>;
    async fn fetch_page(&self, ctx: &RequestContext, request: FeedRequest)
        -> Result<FetchPageOutcome, ProviderError>;
    async fn fetch_facet(&self, ctx: &RequestContext, request: FacetRequest)
        -> Result<FacetOutcome, ProviderError>;
    async fn execute(&self, ctx: &RequestContext, command: ProviderCommand)
        -> Result<DeliveryOutcome, ProviderError>;
    async fn reconcile_delivery(&self, ctx: &RequestContext, evidence: DeliveryEvidence)
        -> Result<ReconciliationOutcome, ProviderError>;
}
```

`RequestContext` is supplied by the engine: account, auth epoch, cancellation,
request budget/deadline, trace-safe ID, and HTTP/auth facade. It contains no Tauri
types. Adapters normalize external data into bounded batches and declare their
field authority, rate family, continuation, and completeness semantics. The engine
owns persistence, retries, scheduling, and optimistic projection rules.

`ProviderProfile` exposes tested operations and conditional support:
`supported`, `unsupported`, `requires_scope`, `requires_permission`,
`unknown`. Compute effective capabilities from adapter, installation/version,
credential type/scopes, repository policy, and current actor permission.
Recheck at command dispatch; cached capability checks are not authorization.

Each operation declares:

- input/result DTO and required facets;
- online requirement and offline admission policy;
- server concurrency guard: enforced revision/head, best-effort preflight, or none;
- remote idempotency/effect-verification strategy and ambiguity policy;
- affected entity facets, collection dependencies, and rate family;
- asynchronous completion handling and typed provider extension, if applicable.

For example, `PullRequest.setTitle`, `Issue.close`, `Comment.create`,
`Review.submit`, and `PullRequest.merge` have different delivery rules; treating
them all as `update(entity, patch)` loses essential semantics.

| Provider | Sync and feature constraints | Planned adapter behavior |
| --- | --- | --- |
| GitHub | PR lists have updated sorting but no universal delta cursor; issues feeds include PRs; REST and GraphQL have different budgets | REST baseline; targeted GraphQL batching where measured; alias identities and hydrate children separately |
| GitLab SaaS/self-managed | MR time filters, IIDs scoped to projects, todos, discussions/approvals; versions/plans/settings vary | Instance probes; capability-gated facets; inclusive overlap and reconciliation |
| Bitbucket Cloud | UUID identities, opaque next links, q/sort support, PR tasks/participants/draft fields; no native issue tracker | Independent adapter; all relevant PR states explicit; issue unsupported |
| Bitbucket Data Center | Different routes/auth/models/version fields from Cloud; external issue tracking | Separate adapter/test matrix; native issues/general inbox unsupported initially |
| Future Gitea/Forgejo/Azure/Jira | Different resource concepts and delivery guarantees | Add manifests, normalization, contract tests, and typed extensions without provider branches in core UI |

Sources: [GitHub pull requests](https://docs.github.com/en/rest/pulls/pulls),
[GitLab merge requests](https://docs.gitlab.com/api/merge_requests/),
[Bitbucket Cloud PRs](https://developer.atlassian.com/cloud/bitbucket/rest/api-group-pullrequests/),
[Bitbucket Data Center PRs](https://developer.atlassian.com/server/bitbucket/rest/v900/api-group-pull-requests/).

Provider SDKs can reduce endpoint boilerplate, but must allow pooled transport,
cancellation, header/validator access, redaction, and engine-controlled retries.
Default to explicit reqwest requests with serde models where SDK policy conflicts.
Pin provider API versions where supported; request only required fields, tolerate
unknown additions, and validate size/depth. Extension commands travel through the
same command machinery. Native dynamic plugins are deferred.

## 9. Local storage and schema

Use one database under Tauri's **local application-data directory**, separate from
repositories and shared/network mounts. Partition provider content by account.
A single DB makes atomic account lifecycle changes and a unified inbox practical;
account-scoped repository methods and composite keys enforce isolation.

Proposed schema groups:

| Group | Tables/concepts |
| --- | --- |
| Identity | provider_instances, accounts, credential_refs, entity_registry, provider_identity_aliases, locator_aliases, local_remote_links |
| Confirmed observations | repositories, actors, pull_requests, issues, comments, reviews, review_threads, checks, labels, milestones, relation tables, provider_facets |
| Effective query views | account-scoped effective PR/issue/inbox rows, relationships, search documents, local preferences |
| Coverage | subscriptions/sync_scopes, scope_membership, facet_coverage, page_validators, feed_checkpoints, reconciliation_runs |
| User intent | drafts, commands/outbox, command_dependencies, delivery_attempts, receipts, optimistic_effects, conflicts |
| Runtime/recovery | sync_jobs, rate_budget_state, account epochs, change_revision, change_log, schema_migrations |
| Assets | bounded blob/asset metadata and account-namespaced content-addressed files |

Confirmed base and effective views are distinct. The effective view applies
pending commands and local disposition to base rows so list membership, ordering,
counts, and detail views agree. Start with materialized effective rows updated by
the writer; do not assemble huge overlays in React. Pending temporary creations
participate in lists and get durable temp-to-provider ID mappings on confirmation.

A representative shape:

```sql
-- Conceptual excerpt, not a migration ready to run.
CREATE TABLE pull_requests (
  account_id TEXT NOT NULL,
  entity_id TEXT NOT NULL,
  repository_id TEXT NOT NULL,
  title TEXT NOT NULL,
  state TEXT NOT NULL,
  provider_updated_at TEXT,
  base_revision INTEGER NOT NULL,
  PRIMARY KEY (account_id, entity_id),
  FOREIGN KEY (account_id) REFERENCES accounts(id)
);
CREATE TABLE commands (
  account_id TEXT NOT NULL,
  command_id TEXT NOT NULL,
  target_id TEXT NOT NULL,
  authorization_epoch INTEGER NOT NULL,
  kind TEXT NOT NULL,
  payload_version INTEGER NOT NULL,
  payload_json TEXT NOT NULL,
  submission_hash TEXT NOT NULL,
  delivery_state TEXT NOT NULL,
  enqueue_order INTEGER NOT NULL,
  next_attempt_at TEXT,
  PRIMARY KEY (account_id, command_id),
  FOREIGN KEY (account_id) REFERENCES accounts(id)
);
CREATE INDEX effective_pr_list
  ON effective_pull_requests(account_id, repository_id, state, sort_time DESC, entity_id);
```

Full migrations add composite foreign keys for account-bound relations, constraints,
operation bases/guards, retry evidence, coverage, and validated enum handling.
Typed repositories always accept an AccountScope; no public raw-SQL IPC. Audit
every cross-account aggregate explicitly.

Initial DB configuration: WAL, foreign keys enabled on every connection,
`synchronous=FULL`, a bounded busy timeout, one serialized writer connection and
approximately 2–4 read connections. Keep transactions short; bounded write batches
yield between pages and prioritize interactive commands. WAL still has one writer,
and NORMAL durability can lose recent commits after power failure; drafts/outbox
justify FULL initially.
[SQLite durability settings](https://sqlite.org/pragma.html#pragma_synchronous).

Bundle and verify a tested SQLite release including the WAL-reset fix. Current
official guidance identifies SQLite 3.51.3+ or the documented 3.44.6/3.50.7
backports as fixed. Verify `sqlite_version()` and build features in packaged
platform tests, rather than trusting a system library.
[SQLite WAL](https://sqlite.org/wal.html).

Use indexed keyset pagination with deterministic ID tie-breaks. Do not keep
database read transactions open for the lifetime of a screen. Start FTS5 with
titles and bounded cached bodies, account-filtered before results leave Rust;
maintain index/content consistency transactionally and rebuild when projections
change. Do not index all diffs by default.
[SQLite FTS5](https://sqlite.org/fts5.html).

Large diffs/attachments use account-namespaced blobs: write temp file, atomic rename,
then commit metadata; collect unreferenced files later. Never share a sensitive blob
across account authorization boundaries solely because its content hash matches.
Draft text stays in the durable DB. Files referenced by drafts/outbox are durable
user intent, distinct from rebuildable provider attachments. Flush contents and
platform-supported rename/directory metadata before acknowledging their DB
reference; protect them from eviction/GC until all intent references are released.
SQLite FULL alone cannot make a separately stored attachment durable. Test crash
and power-loss recovery with referenced attachments, not only text commands.

## 10. Local API and developer experience

Prefer an explicit account-bound client and inert handles:

```ts
const remote = collaboration.forAccount(accountId);

// Local construction only; provider/host is bound by the account.
const pr = remote.pullRequests.open({
  repository: { path: "ruru-m07/gitru" },
  number: 67,
});

const snapshot = await pr.read(); // SQLite only; may be missing/partial
const stop = pr.watch((next) => render(next));
const refreshJob = await pr.ensureFresh({
  facets: ["summary", "reviews"],
  reason: "user-refresh",
});

// Durable command admission; delivery is tracked separately.
if (snapshot.data === null || snapshot.entityBaseRevision === null) {
  throw new Error("Load the PR before changing its title");
}
// The UI also gates on capabilities; Rust rechecks authorization on submission.
const receipt = await pr.setTitle({
  commandId: crypto.randomUUID(),
  title: "Improve remote collaboration",
  expectedBaseRevision: snapshot.entityBaseRevision,
});
await remote.commands.waitForLocalRevision(receipt.localRevision);
```

The missing state has no entity revision; this example's mutation requires a
loaded editable snapshot. Commands reject absent bases instead of guessing.
A URL/shorthand parser is convenience syntax:
`remote.pullRequests.open(locator)` can remember a locator but does not claim that
it already resolved an immutable identity. A separate resolve/hydrate job fills it.
`forAccount` removes provider knowledge from normal operations while preserving
the actor responsible for writes. `open()` does not allocate a forever-live
subscription or start hidden HTTP.

Local query result contract:

```ts
type LocalResult<T> = {
  data: T | null;
  availability: "missing" | "partial" | "ready" | "inaccessible";
  coverage: {
    scope: string;
    completeness: "unknown" | "partial" | "complete-within-scope";
    localHasMore: boolean;
    remoteHasMore: boolean | "unknown";
  };
  localRevision: string;
  entityBaseRevision: string | null;
  authorizationViewToken: string;
  validatedAt: string | null;
  syncState: "idle" | "syncing" | "offline" | "rate-limited" | "auth-required" | "error";
  pendingCommandIds: string[];
};
```

“Complete” always means a defined scope/range/observation, not all provider data.
`localRevision` is the database transaction sequence; `entityBaseRevision` is the
confirmed entity/facet base against which a command was composed, and is null for
missing entities and collection-only results. Query/effective-view versions are
separate. Commands following pending edits capture dependency order and the
relevant base/effective fields; they do not mistake optimistic state for provider
confirmation.

An empty complete local range and an uncached range must render differently.
Expose local/remote pagination separately; “load more” requests a remote hydration
job if local coverage is exhausted. Cache-only search labels its coverage; remote
search is an explicit online operation whose results can be imported as partial
observations.

React helpers such as `usePullRequests({ accounts, repositories, filter })`,
`usePullRequest({ accountId, entityId })`, and `useInbox({ accounts })` hide the
subscription bridge. Query keys include account set/epoch, entity/query kind,
facets, canonical filter/sort, and pagination.

For remote local-read queries set `networkMode: "always"` so TanStack's browser
online manager cannot suppress SQLite reads; set `refetchInterval: false`,
`refetchOnWindowFocus: false`, and appropriate stale/GC behavior. “Always” applies
to the **local query function**, not a provider request. Subscription commits
invalidate or update only matching projections. Focus/visibility reports demand
to Rust once per webview; Rust coalesces it across the app.

Use a validated query DSL over supported filters/sorts, not arbitrary SQL or
GraphQL from components. Stable handles identify entities; query results and
receipts describe availability and effects.

## 11. Sync engine and incremental reconciliation

### 11.1 Scheduling unit and ownership

A sync scope is account + resource/facet + normalized filter/range, for example a
repository's recent PR index, one PR's reviews at head H, or an account's native
notification feed. It records desired coverage, strategy/version, checkpoint,
continuation, generation, last successful validation, and backfill state.

Separate intent from execution:

- subscriptions, local repository links, pins, inbox subjects, and user refreshes
  produce demand;
- a deduplicated job planner turns demand into bounded work;
- an account/host-aware scheduler obtains rate/concurrency permission;
- an adapter fetches outside the DB;
- the writer validates generations, applies normalized changes, updates coverage
  and progress, recomputes effective views, and appends change metadata;
- subscribers see the committed result.

Persist resumable jobs and outbox attempts; activity leases and in-flight tasks are
ephemeral. On restart, recover expired leases and incomplete pages. A sending
mutation becomes outcome-unknown until reconciled; a fetching read can be retried.
Use a process lock/single-runtime policy so a second app process cannot dispatch
the same outbox independently. Do not add distributed locking for one desktop.

Background sync runs while the app process is running. Sleep suspends it; resume
triggers reconciliation. Quitting does not leave an OS service running.

### 11.2 Progressive bootstrap and working set

After account verification, serve local account/repository state immediately.
Discover repository metadata in bounded pages, then prioritize user-selected
repositories and account inbox demand. Do not hydrate every repository an account
can access.

Proposed default coverage: open PR/issue summaries plus a recent closed window
(e.g. 90 days), inbox subjects, pinned items, and recently visited detail facets.
That window is a product setting, not an API guarantee. Keep older history as
demand-driven backfill. Some APIs require scanning beyond the desired window;
report and budget that cost rather than claiming cheap delta support.

Hydration tiers:

1. Minimal identity and summary needed for lists and badges.
2. Body and related actors/labels for likely-to-open items.
3. Reviews, threads, checks, timeline, and small diff summaries for active/pinned PRs.
4. Full diffs, attachments, historical comments, and deep backfill on explicit demand.

Selection/hover prefetch submits low-priority deduplicated jobs; it cannot launch
unlimited requests. Cancel irrelevant speculative work when demand disappears.
Accounts and repositories with no active demand retain bounded metadata and a
lower reconciliation cadence.

### 11.3 Incremental strategies are endpoint-specific

Adapters declare `timestamp-window`, `updated-order-scan`,
`conditional-snapshot`, `membership-reconcile`, or a tested event/cursor
strategy. There is no universal `changesSince()` contract.

GitHub PR listing has update ordering; its issue listing has `since` and includes
PRs. Use the latter as a dirty-entity discovery input and hydrate PR facets. Both
open and closed transitions need appropriate state filters. GitLab MRs expose
inclusive update-time bounds, but these are filters over current resources, not
an immutable log. Bitbucket Cloud uses q/sort and opaque next links; Data Center
uses its returned page continuation. Children have independent freshness scopes.
[GitHub issue lists](https://docs.github.com/en/rest/issues/issues),
[GitLab MR filters](https://docs.gitlab.com/api/merge_requests/),
[Bitbucket pagination](https://developer.atlassian.com/cloud/bitbucket/rest/intro/),
[Data Center REST paging](https://developer.atlassian.com/server/bitbucket/rest/v1004/).

For a timestamp strategy:

1. Read the last completed watermark W and choose lower bound W minus an overlap
   interval. Use a server-time upper bound only if the endpoint supports it and
   its semantics are understood; local wall-clock “now” alone is not safe.
2. Persist the run's parameters and page continuation. Fetch pages with stable
   endpoint ordering/tie-breaks where supported; otherwise flag mutable paging.
3. Commit each page's data, seen identities, validator, and continuation together.
   Retrying a committed page is harmless.
4. Advance the completed watermark only after the intended traversal succeeds.
   Timestamp overlap, equal timestamps, deduplication, and secondary identity
   checks are mandatory. Where no safe upper bound exists, retain overlap and
   regular full-scope reconciliation rather than claiming gap-free delta.
5. On expired continuation or changed auth/filter/adapter version, restart that
   traversal safely without clearing existing local data.

A moving offset-paginated list can skip items even with timestamp overlap.
Periodically perform broader reconciliation, rescan mutable boundaries, and
directly revalidate known objects missing from a run. “Complete within scope”
means a completed observed traversal; it is not an atomic provider snapshot.

GitLab todos need membership reconciliation because their API does not establish
an updated-time delta feed. GitHub search has result caps; activity events have
retention and latency limits. Use search/events for discovery/hints, never as proof
that the replica contains every matching item.
[GitLab todos](https://docs.gitlab.com/api/todos/),
[GitHub search](https://docs.github.com/en/rest/search/search),
[GitHub events](https://docs.github.com/en/rest/activity/events).

### 11.4 Absence, deletes, and permission changes

Maintain scope membership with run ID, last-seen observation, and verification
state. A successful traversal can nominate missing membership for verification;
an incomplete/failed/capped traversal cannot. Absence from an open list may mean
closed, transferred, hidden, or missed through mutable paging.

For suspected missing items, request a canonical detail/status or relevant
permission endpoint under a bounded budget. Remove the item from a scope only
when its status/membership is established, or mark membership uncertain. Do not
infer a global entity tombstone from a list miss. A provider 404 can conceal lack
of access; distinguish deleted, inaccessible, and unknown only when evidence
allows. Retain minimal tombstone/identity metadata as appropriate to prevent
reappearance from old responses.

When scope access becomes inaccessible, atomically suppress effective rows,
search documents, counts, subscriptions, and asset access, and pause its outbox.
All query/FTS/replay/asset paths filter current accessible scopes and authorization
epochs in Rust. Protected retention of old bodies, if offered, is a deliberate
policy and must not make them discoverable through search or global deduplication.

### 11.5 Stale response and write races

At fetch start, capture authorization epoch, scope generation, facet/base revision,
and request fingerprint. At commit:

- reject results from old account/scope generations;
- apply only fields the endpoint actually observed;
- reject known older provider revisions/timestamps where that comparison is valid;
- serialize competing refreshes for a facet or refetch when incomparable responses
  would overwrite a newer observation;
- recompute pending effects on the accepted base.

A mutation invalidates the relevant fetch generation. A request started before
the mutation cannot undo its acknowledged result. ETags are compared for equality,
not ordered. Parent `updated_at` cannot order review/check observations.

Providers can also return stale data to a request started after an accepted write.
Keep a bounded acknowledgement/observation barrier for affected fields and perform
targeted reconciliation. Do not instantly clear a 202 overlay or revert an
acknowledged effect on one inconsistent list read. If the acknowledgement barrier expires while observations still disagree and
revisions are incomparable, enter an explicit observation-uncertain state. Retain
acknowledgement evidence and show the acknowledged effect alongside the divergent
latest observation for reconciliation; expiry proves neither failure nor a
competing edit, and cannot trigger retransmission or a silent rollback.
If later canonical evidence
shows a real competing change, surface it; never suppress it forever to preserve
optimism. Adapter tests must define the operation's observation/completion rule.

## 12. Caching and invalidation

There are three distinct layers:

1. SQLite confirmed observations + effective views: durable local data and intent.
2. Rust query/projection caches: small, keyed by query and committed revision.
3. Each webview's TanStack Query cache: only active/recent view DTOs.

A TTL answers “when should we consider refreshing?” It does not establish validity,
permissions, or completeness. Keep `observed_at`, `validated_at`, facet coverage,
provider validators, and pending effects separately. A valid 304 updates validation
time without incrementing entity content revision; it may still change visible
sync/freshness metadata.

Key HTTP validators by account/credential visibility epoch, provider instance,
method, canonical URL/query, representation/Accept headers, API version, and
projection. A 304 for page one does not establish that all later pages or child
facets are unchanged. Invalidate relevant validators after a successful mutation,
auth/scope changes, and adapter representation changes.

The writer produces a change set describing affected account/repository/entity/
facet/collection namespaces plus lifecycle changes. Initial behavior re-queries
affected local list/count projections and patches detail only when safe. Include
old and new dependency keys: relabeling or closing a PR affects list membership,
sort order, counts, search, and its detail. Do not invalidate the entire app after
each item changes.

Serve stale permitted data while reconciliation runs, with visible freshness and
offline/rate/auth status when relevant. Provider failure does not delete good
cached content. A revoked/inaccessible scope takes precedence over stale fallback.
Use no duplicate webview provider cache or frontend persistence replica.

Read-your-writes follows the effective view: command admission and effective
projection commit together and return a local revision. Every local query after
that revision includes the command's effect until conflict/rejection/cancellation
or confirmed completion.

## 13. Query subscriptions and reliable IPC

Use typed generated commands for local reads, query subscriptions, refresh
requests, command submission, receipts, account lifecycle, and demand reporting.
Prefer Tauri Channels for compact ordered batches. They are not durable storage.
If the existing generator cannot represent Channel/structured result DTOs, phase 0
must either extend the generator in its source or use a targeted typed event hint
plus generated snapshot/catch-up commands. Do not hand-edit generated wrappers or
introduce ad-hoc string invokes.

Tauri recommends Channels for ordered throughput; global events are suited to
simpler communication. This motivates the transport choice, not a guarantee of
delivery across webview reloads.
[Tauri frontend messaging](https://v2.tauri.app/develop/calling-frontend/).

### Snapshot and catch-up protocol

Persist an incrementing DB transaction revision and compact change log in the same
transaction as state changes. Entity/facet revisions and query result versions are
separate. Serialize revision values as decimal strings. Do not substitute
`PRAGMA data_version`, whose semantics are connection-local.
[SQLite data_version](https://sqlite.org/pragma.html#pragma_data_version).

Proposed subscription protocol:

1. Register a scoped subscriber/lease with the coordinator and start buffering
   hints before reading its snapshot.
2. In one short SQLite read transaction, read the requested projection and the
   committed revision R. Registration and snapshot coordination ensure all changes
   after R are buffered or replayable.
3. Deliver snapshot R; discard buffered hints already covered by it and replay
   subsequent relevant ranges.
4. Messages carry subscription ID, auth epoch, `fromExclusive`,
   `throughInclusive`, and affected keys or projection deltas. The cursor
   advances through **scanned** revisions even when some changes are irrelevant;
   filtered subscribers do not assume numeric sequence gaps imply lost messages.
5. On gap/overflow/reload, call catch-up after the last acknowledged revision.
   If history was compacted or authorization changed, return `ResetRequired`
   and obtain a new authorized snapshot.
6. Account disconnect/revocation sends a lifecycle reset and forbids replay of old
   content. The query cache is removed, not merely marked stale.

Every local read, snapshot, catch-up response, and buffered batch includes an
authorization-view token. A unified multi-account query uses an epoch vector or
equivalent opaque generation covering all included accounts/scopes. The client
cancels old requests/subscriptions and rejects obsolete view tokens, request
generations, and older projection revisions before writing QueryClient data.
This prevents a delayed IPC completion from refilling a cache cleared by
revocation. Rust remains the authorization authority. Test revocation while a
local read/snapshot is in flight and old messages are buffered.

Deduplicate equivalent subscriptions per webview. Each subscriber acknowledges
progress and holds a renewable activity lease; close/unmount unregisters it,
and expiration cleans up crashed windows. Slow subscribers get coalesced
invalidations and eventually a reset, never an unbounded memory buffer.
Log retention is bounded by time/bytes; it is a catch-up facility, not an
indefinite audit log.

Initially send invalidation batches and requery bounded local projections.
Incremental list deltas can follow once correctness and profiling justify them.
A list cursor contains its query identity and result revision; membership/order
changes require restarting the page window or an explicit reset. Do not splice
pages from different revisions and call the combined result a snapshot.

## 14. Commands, optimistic updates, offline support, and conflicts

### 14.1 Command admission and durable intent

Submission includes account, client-generated command UUID, kind/version,
target, payload, expected base fields/revision or head, and dependencies.
Rust derives/checks the provider instance through the account; it never trusts
independently supplied account/instance pairs.

Validate inputs/capability/scope, capture a minimal base for conflict resolution,
persist the command and optimistic effect, recompute affected effective views, and
return a receipt after commit. Same UUID + same canonical submission returns the
existing receipt; same UUID + a different submission is an error. This makes a lost IPC response
safe to retry locally without creating a second remote command.

UI may show a transient saving state before the receipt; only after the durable
acknowledgement may it say queued. Do not also maintain an independent TanStack
optimistic overlay over the Rust effective state. Client previews must retire at
the receipt revision.

Serialize commands per target/dependency chain, with bounded parallelism for
unrelated targets. Commands retain the original actor/profile; never reroute a
pending write through another connected account.

### 14.2 Delivery state machine

```mermaid
stateDiagram-v2
  [*] --> queued: durable local receipt
  queued --> sending: durable dispatch attempt
  queued --> cancelled: not dispatched
  sending --> accepted: remote asynchronous acknowledgement
  sending --> confirmed: canonical completed effect
  sending --> outcome_unknown: ambiguous delivery
  sending --> retry_wait: proven safe retry
  sending --> conflict: concurrency mismatch
  sending --> rejected: permanent failure
  retry_wait --> sending: budget and deadline permit
  accepted --> confirmed: canonical completion observed
  accepted --> rejected: provider reports failure
  outcome_unknown --> confirmed: positive outcome evidence
  outcome_unknown --> queued: explicit safe retry decision
  conflict --> superseded: resolution creates a new command
```

Auth/rate/offline blocking reasons can attach to queued/retry_wait/accepted;
they do not erase delivery evidence. Persist attempt start before dispatch, then
record response/evidence. A crash between marking sending and receiving a result
is ambiguous even if the request might never have left the device.

For dependent title edits A → B → C, the unsent successor retains its original
user-observed base and predecessor relation, but computes a separate execution
base after B is confirmed. Accepted/outcome-unknown predecessors block dependent
delivery. Rejection, cancellation, or supersession re-evaluates successors and
may produce a conflict; it never blindly dispatches a stale chain. Preserve
immutable submitted intent while recording execution-base changes separately.

Conflict resolution with changed intent creates a new superseding command UUID.
Retain the original conflict receipt/attempts and update the derived execution
dependency graph while preserving original submitted dependencies and their hash.
Hash the complete canonical submission (kind, target, guards, dependencies and
payload); preserve its original hash across payload migrations. Retransmitting a
command UUID cannot change those submitted fields.

A 202 or queue/auto-merge acknowledgement means accepted, not merged/completed.
Keep its operation receipt and reconcile asynchronously. On definitive rejection
remove only that command's effect and replay later commands over current base;
preserve text/draft/conflict evidence.

Exactly-once remote delivery is not a general promise. Use provider idempotency
keys only where documented for the operation. An identical remote body is not
proof that a particular comment attempt created it; user/time/body matching can
be ambiguous. Prefer returned remote IDs or strong endpoint evidence. Without
proof of completion or non-delivery, retain outcome-unknown and require user
resolution before repeating a non-idempotent create.

### 14.3 Offline admission policy

| Operation | Proposed offline behavior | Dispatch/concurrency policy |
| --- | --- | --- |
| Local bookmark/snooze/draft | Fully local | No provider write |
| Mark native item read/done | Queue only where supported fenced semantics permit | Enforced activity fence where available; otherwise online best effort or local disposition |
| Change title/body/labels/assignees | Queue supported desired-state intent | Compare affected base fields, merge independent changes, use server guard if available |
| Close/reopen issue or PR | Optional queue with visible pending state | Revalidate current workflow/state; no implied approval or merge |
| Create comment or issue | Save draft; explicit offline send may queue | Non-idempotent outcome-unknown recovery required |
| Create PR | Durable draft initially | Online branch/head/permission validation; creation can be ambiguous |
| Submit review/approval, resolve diff discussion | Durable draft; send online | Bind to inspected head/anchor and current permissions |
| Merge, delete resource, destructive branch action | Online only | Require a tested server-enforced guard where the safety contract depends on it |

Unsupported operations stay unsupported offline. Users can edit drafts even when
credentials are expired. Cached navigation/search/details work offline within
stored coverage; uncached bodies/diffs show unavailable state and keep hydration
intent. Offline detection is a scheduling hint, not proof a private host is
unreachable; use per-instance failures/circuit state.

### 14.4 Conflict handling and provider guarantees

Use operation-specific three-way comparison: captured base B, current remote R,
desired intent L. If affected remote fields still equal B, apply L. If remote
already equals L, record observed convergence with appropriate delivery evidence.
Merge independent field edits; expose overlapping text/workflow changes rather
than silently taking last-write-wins. Preserve label/assignee add/remove intent
where possible instead of always replacing a stale entire set.

This protects user intent but is **best effort** unless the provider enforces a
conditional write. A preflight GET followed by PATCH still races. The UI and
capability contract must distinguish guarded from best-effort operations.

GitHub/GitLab merge endpoints expose expected head SHA guards. Always send the
head the user inspected. Bitbucket Data Center exposes versioned operations;
test whether the supported version also protects the reviewed head. Bitbucket
Cloud's documented merge endpoint does not establish an expected-head guard.
The strict common `merge({ expectedHead })` capability remains unsupported there
until verified; any online provider-native alternative must disclose its weaker
semantics rather than claiming the same guarantee.
[GitHub merge](https://docs.github.com/en/rest/pulls/pulls#merge-a-pull-request),
[GitLab merge](https://docs.gitlab.com/api/merge_requests/#merge-a-merge-request),
[Data Center versioned PR operations](https://developer.atlassian.com/server/bitbucket/rest/v900/api-group-pull-requests/),
[Bitbucket Cloud merge](https://developer.atlassian.com/cloud/bitbucket/rest/api-group-pullrequests/).

Reviews and inline comments retain head/base/start OIDs, paths, line/side anchors,
and provider-required position metadata. A force-push marks a draft anchor stale;
remap only with an explicit verified mapping and show conflicts. Checks and
approvals bind to the relevant head; old success cannot authorize merging a newer
head.

GitHub bulk mark-read exposes a last-read timestamp fence, while its single-thread
mark-read does not document equivalent compare-and-swap. Revalidate queued read
intent; newer activity means skip/conflict or Gitru-local dismissal. Without an
enforced per-item fence, preflight is best effort and must not promise that no
new activity can be consumed in the intervening race.
[GitHub thread and bulk read operations](https://docs.github.com/en/rest/activity/notifications#mark-a-thread-as-read).

Cancellation after dispatch means stop waiting/attempts, not “undo remote action”.
Undo is a new compensating command, subject to provider permissions and current
state.

## 15. Authentication and multiple accounts

Account connections are independent of Gitru social sign-in. Obtain the remote
actor from the provider after every authorization and after adding a credential
profile. Two profiles belong to one account only if they verify the same actor on
the same instance. Select profiles by operation and supported scopes; do not
silently use a different actor's token to fill an inbox.

| Adapter | Proposed initial authentication | Integration gates |
| --- | --- | --- |
| GitHub.com | Manual PAT; optional import of an existing GitHub CLI account | Confirmed product decision: Gitru does not initiate OAuth/device login. Verify the actor and actual credential capabilities; an existing CLI credential may itself be OAuth |
| GitHub Enterprise | PAT first or tested registered flow | Host/version/policy/device support; separate registration may be required |
| GitLab.com | Direct manual PAT with actor and repository-operation probes | Initial R110 rollout; OAuth/PKCE and self-hosted trust remain later separately qualified strategies |
| Bitbucket Cloud | Current API token initially; optional confidential OAuth broker | No established secretless native OAuth flow in examined docs; never ship consumer secret |
| Bitbucket Data Center | PAT/admin-configured integration initially | OAuth/PKCE/secret requirements and capabilities verified per version |

For providers whose product flow uses OAuth, GitHub's current OAuth documentation
supports PKCE and expiring tokens, but the
web code exchange still documents a client secret; device flow does not require
one. GitLab supports public-client PKCE, with device flow generally available
since 17.9. Bitbucket Cloud documents secret-based exchanges and retired app
passwords; Data Center PKCE examples still use secrets. These are reasons for
provider-specific auth strategies, not a universal OAuth wrapper.
[GitHub OAuth](https://docs.github.com/en/apps/oauth-apps/building-oauth-apps/authorizing-oauth-apps),
[GitLab OAuth](https://docs.gitlab.com/api/oauth2/),
[Bitbucket Cloud OAuth](https://developer.atlassian.com/cloud/bitbucket/rest/intro/),
[App-password retirement](https://support.atlassian.com/bitbucket-cloud/docs/revoke-an-app-password/),
[Data Center OAuth](https://confluence.atlassian.com/bitbucketserver/bitbucket-oauth-2-0-provider-api-1108483661.html).

Use the system browser, state/nonce validation, PKCE where supported, exact
registered callback validation, short-lived connect-session IDs, cancellation,
and restricted loopback/deep-link handling. OAuth callback support is a dedicated
native path; it must not loosen the general HTTPS external opener. Device flows
obey polling interval/slow_down and never log codes or tokens.
[Native OAuth guidance](https://www.rfc-editor.org/rfc/rfc8252/).

Secrets live in macOS Keychain, Windows Credential Manager, or Linux Secret
Service through a tested Rust credential abstraction; SQLite stores references
and nonsecret capability observations only.
[Keyring platform interfaces](https://docs.rs/keyring/latest/keyring/v1/index.html).

Stored credentials are never returned to webviews. A manually entered PAT/API
token passes once from the trusted main account form to Rust; never persist it
in Zustand/localStorage/query cache, browser URLs, telemetry, or error strings.
Clear the form promptly and do not claim JavaScript memory can be securely wiped.
Child webviews can request authorized domain commands, not credential export or
account-connect management.

Refresh is single-flight per credential profile. Store each rotated token pair as
one versioned secret envelope, with explicit recovery tests for vault failure.
The provider refresh and local vault write are not a distributed transaction:
a crash after rotation can require reauthentication. Preserve pending intent,
pause delivery, and report auth-required rather than discarding the outbox.
Credential replacement advances the relevant epoch and revalidates access.
For queued, undispatched intent, revalidate the same actor, profile, permissions,
and command preconditions before binding a fresh dispatch epoch. Preserve command
UUID, immutable payload, ordering, and attempt history. Sending/accepted/unknown
work is reconciled rather than reset to queued. A late response from an old epoch
may record minimal delivery evidence against its existing attempt, but cannot
repopulate provider content or authorize new work under the current account view.

Local disconnect is separate from upstream token/grant revocation. Adapters
declare whether revocation is supported and its scope/cross-device effects. The
UI must not claim that removing local credentials revoked provider access.

If the OS vault is unavailable/locked, fail the credential operation clearly.
No silent plaintext fallback. Offer reconnect/session-only mode only with an
explicitly designed nonpersistent policy. Validate exact host and credential
audience; cross-host redirect must not carry Authorization.

Use least-privilege scope requests and incremental consent. An installation grant,
SSO restriction, token expiry, and account sign-in are separate states. Show the
actor responsible for every mutation, particularly when personal/work accounts
overlap.

### 15.1 Confirmed GitHub PAT and CLI import policy

The user selected PAT authentication for GitHub collaboration on 2026-10-02.
Manual token entry remains available. The connection dialog may discover
existing `github.com` accounts through GitHub CLI in the background and offer
an explicit account choice; detection does not import credentials or start sync.
Gitru does not run `gh auth login`, switch the CLI's active account, change its
configuration, or initiate its own OAuth/device flow. CLI browser login can have
created an OAuth credential previously; the UI describes that source accurately
and does not promise that every PAT grants more permissions than OAuth.
[GitHub CLI login modes](https://cli.github.com/manual/gh_auth_login).

Native discovery uses `gh auth status --hostname github.com --json hosts` with
a fixed metadata-only `--jq` projection, without `--show-token`. JSON status
support requires GitHub CLI 2.81 or later; unsupported/missing/unavailable CLI
states leave manual PAT entry usable. JSON authentication failures do not imply
nonzero process exit, so parse each account's authentication state.
[GitHub CLI status](https://cli.github.com/manual/gh_auth_status),
[JSON status introduction](https://github.com/cli/cli/releases/tag/v2.81.0).

Discovery returns only bounded login/host/active/availability metadata and opaque
native candidate IDs with a five-minute expiry. Selection revalidates metadata,
runs `gh auth token --hostname github.com --user <selected-login>` natively,
verifies the provider `/user` identity, then enters the same secure vault and
authorization-epoch lifecycle as manual token entry. No token crosses back into
JS, a query cache, SQLite, analytics, URLs or error strings. Disconnect removes
Gitru's copy and does not log the account out of GitHub CLI.
[Explicit CLI token selection](https://cli.github.com/manual/gh_auth_token).

The runner uses recognized absolute install paths, neutral working directory,
argument arrays, null stdin, disabled prompting/debug/paging/update notices,
bounded output and deadlines, and wiped native output buffers. Stored-account
selection removes `GH_TOKEN`/`GITHUB_TOKEN` and enterprise token environment
overrides so a different environment credential cannot replace the chosen user.
Custom CLI installations outside the recognized locations currently fall back
to manual PAT. Recognized executable locations are resolved on every invocation
so retrying discovery can detect installation, removal, or an upgraded symlink.
E2E builds and the default standalone runtime disable personal
CLI discovery; explicit fixture runners test the behavior without touching a
developer's accounts or keychain.
[GitHub CLI environment precedence](https://cli.github.com/manual/gh_help_environment).

Gitru has one native window with a main host and embedded tab webviews. Every
Accounts control sends the fixed, payload-free `gitru:open-account-settings`
UI hint to the explicit main webview; there is no separate window for users to
find. A single host dialog mounts the form only after pending native child
creation/visibility work settles and every native tab is hidden. Tab creation
is deferred during suspension; closing restores the currently selected tab
without destroying its session. Events are not authenticated caller identities
and carry no credentials or connection/disconnection instructions. All credential
commands still validate the local main caller. Repository sync selection uses
the ordinary local-domain caller policy so the picker works inside content tabs.

## 16. Rate limits, errors, retries, and backpressure

Centralize budgets by provider instance + verified principal/installation +
resource family, with credential-profile attribution. Several tokens for the same
user may share an upstream budget; multiple accounts/credentials do not multiply
a known shared allowance.

Observe response headers and endpoint policy; reserve budget for user actions and
auth recovery. Use conservative initial concurrency (e.g. 2–4 requests per host
and one mutation lane per principal), adaptive reduction, fairness between
accounts/repos, and starvation bounds for backfill. These are tuning defaults,
not documented provider limits.

GitHub has primary and secondary constraints and supports conditional requests.
GitLab.com/self-managed limits vary; some endpoints do not advertise useful
remaining-budget headers. Bitbucket Cloud uses rolling/auth-dependent budgets;
Data Center can be admin-configured. Do not hard-code one universal hourly quota.
[GitHub rate limits](https://docs.github.com/en/rest/using-the-rest-api/rate-limits-for-the-rest-api),
[GitLab.com limits](https://docs.gitlab.com/user/gitlab_com/rate_limits/),
[GitLab self-managed limits](https://docs.gitlab.com/rate_limits/api/),
[Bitbucket Cloud limits](https://support.atlassian.com/bitbucket-cloud/docs/api-request-limits/),
[Data Center rate limiting](https://confluence.atlassian.com/bitbucketserver094/improving-instance-stability-with-rate-limiting-1489803023.html).

Priorities: user-requested writes/refresh and visible details; inbox/active
repository indices; pins/recent views; background reconciliation; speculative
prefetch/backfill. Aging prevents low-priority scopes from never being checked.
Coalesce duplicate jobs, bound queues/page sizes/response bytes, and drop redundant
refresh intent rather than allowing a backlog of identical polls.

| Error class | Engine response |
| --- | --- |
| Offline/DNS/TLS/timeout on read | Keep cached data; bounded exponential retry with full jitter |
| 401/expired credentials | One refresh attempt where supported, then auth-required |
| 403 | Classify auth, scope, SSO, policy, or rate restriction using adapter evidence; no blind retry loop |
| 404/410 | Resolve identity/access/deletion semantics; do not blanket-delete cache |
| 409/412 or head/version mismatch | Fetch fresh base; conflict workflow; no unconditional retry |
| 422/validation | Permanent actionable rejection unless adapter proves transient |
| 429/secondary throttle | Respect Retry-After/reset/poll minimum; persist cooldown and reduce concurrency |
| 5xx on read | Bounded jittered retry; circuit breaker per host/resource family |
| Network/5xx after mutation dispatch | Operation-specific ambiguous-delivery handling; HTTP status alone is insufficient |
| Provider schema/normalization error | Retain good cache; stop poisoned scope, emit redacted adapter diagnostic |
| SQLite busy/disk full/corruption | Bounded local retry only where safe; refuse a durable receipt if commit failed |

Retry waits never occupy an active worker. Persist retry deadlines/attempt counts,
cap retries and command age, use monotonic time for in-process deadlines and
validated wall time for restart recovery, and cap clock-skew-derived waits.
Writes never inherit a generic HTTP retry policy.

Public errors carry a stable code, operation/account scope, retryability,
retry-after/deadline, safe message, and conflict/unknown-outcome reference. Raw
provider bodies and URLs containing secrets/content do not reach UI or logs.
Errors on cached reads can live beside useful data; mutations retain a receipt
and preserved user text.

## 17. Polling, webhooks, and optional cloud infrastructure

Adaptive polling is the standalone baseline. Proposed intervals are starting
budgets to measure, always constrained by provider headers, cooldowns, and
available quota:

| Demand | Starting cadence |
| --- | --- |
| Native inbox | Approximately 60 seconds, never below provider minimum |
| Visible PR checks/reviews | Approximately 15–60 seconds while useful |
| Active repository summary index | Approximately 1–3 minutes |
| Selected inactive repositories | Approximately 10–30 minutes |
| Wider retained history | Hours/daily, incremental within budget |
| Resume/reconnect/manual refresh | Coalesced prioritized reconciliation |

Increase intervals for repeated 304/no-change results, idle/background windows,
battery constraints, or near-limit signals. Preserve a bounded reconciliation SLA
for selected scopes where budget allows; expose last validation and rate-limit
delay rather than guaranteeing freshness under exhausted quota. If every view is
hidden, active-demand cadence drops; explicit pins can retain configured coverage.

Provider events/webhooks are hints to revalidate affected scopes. They can be
duplicated, reordered, incomplete, missed, or unavailable to a desktop without
public ingress. GitHub does not automatically redeliver failed webhook
deliveries, which reinforces the need for polling/reconciliation.
[GitHub failed deliveries](https://docs.github.com/en/webhooks/using-webhooks/handling-failed-webhook-deliveries),
[GitLab webhooks](https://docs.gitlab.com/user/project/integrations/webhooks/),
[Bitbucket webhooks](https://developer.atlassian.com/cloud/bitbucket/rest/api-group-webhooks/).

If demand justifies an optional relay:

1. Add a separate service/module to `apps/api` with provider signature/secret
   validation, replay prevention, delivery-ID deduplication, bounded payloads,
   durable ingest acknowledgement, and tenant/grant isolation.
2. Route minimal entity/scope hints to enrolled devices over authenticated
   reconnectable SSE/WebSocket with a relay cursor and bounded retained history.
3. Do not broadcast repository bodies or credentials; grant revocation cancels
   routing. Relay enrollment must never authorize broader provider access.
4. After reconnect/gap/relay outage, the desktop reconciles from APIs.
5. Keep separate broker auth if a provider requires a confidential client secret.
   Provider token exchange does not require turning normal provider reads into
   a Gitru cloud proxy.

A hosted mirror is a later architecture extension, not an invisible prerequisite.
It needs a separate threat model, cost/ingestion design, retention controls, and
multi-device semantics. Electric/PowerSync/Zero become relevant only after that
source-of-truth boundary is deliberately introduced.

## 18. Performance, memory, storage, and security

Measure end-to-end local read → IPC decode → React commit, not just SQL duration.
Initial targets below are hypotheses and release gates to benchmark on defined
reference hardware, not measured results:

- Warm cached detail/list navigation: p95 under 100 ms to useful content; local
  query/IPC portion ideally under 30 ms.
- Cold cached landing: useful list under 500 ms after runtime/storage readiness;
  first app paint remains independent of provider network.
- Durable optimistic admission: p95 under 50 ms on reference local SSD.
- Synthetic dataset: 5 accounts, 100 selected repositories, 100,000 PR/issue
  summaries and 500,000 cached child records without full JS hydration.
- Bounded collaboration memory overhead: target under 100 MB across Rust and the
  active webview working set on that dataset; report per-additional-webview cost
  separately. Tune explicit caches, never rely only on entity count.
- Provisional limits: 50–100 rows per IPC list page, about 1 MiB per batch with
  large content delivered separately, and a configurable disk budget (initially
  around 1 GiB for rebuildable collaboration content).

Use EXPLAIN/query-plan assertions for common filters, keyset indices, batched actor
lookups, virtualized lists, structural sharing, memoized selectors, and coalesced
notifications. Avoid N+1 hydration from each row, loading all repo data into MobX/
Zustand, and returning diffs through summary commands. Cancel abandoned read jobs,
bound compressed/decompressed response sizes and JSON depth, and pool HTTP
connections.

Evict heavy assets/diffs first, then old unpinned child facets and unused summary
ranges. Pins/explicit offline coverage protect chosen content within configured
limits. Drafts, outbox, pending effects, ambiguity evidence, intent-referenced files, and minimum identity
metadata are never ordinary LRU entries. Eviction updates coverage and increments
query change metadata so “ready” does not lie about missing content.

Track WAL growth and long readers; use passive checkpoints on maintenance cadence
and bounded batches. Compaction/FTS rebuilds should not block interaction for a
large unbounded transaction. Copying a live main DB file without its WAL is not
a backup; use a consistent SQLite backup mechanism.
[SQLite backup API](https://sqlite.org/backup.html).

Remote titles, markdown, avatar links, attachments, pagination URLs, and provider
errors are untrusted. Sanitize rendering; no arbitrary HTML/script. Validate
pagination and redirect destinations against the configured adapter/instance
allowlist; do not follow provider-supplied URLs with credentials to new origins.
Self-hosted instances can legitimately use private addresses, so trust is an
explicit instance configuration, not a blanket ban on private IPs.

Cache avatars/assets through Rust with content-type/size limits and opaque asset
IDs. Asset serving rechecks account/scope access. Do not wildcard CSP to accept
every self-hosted provider domain; revise asset capability/CSP narrowly and verify
packaged builds as required by `docs/security/threat-model.md`.

App-data permissions and OS disk protection are the initial storage baseline;
SQLite plaintext is not encryption. Decide enterprise at-rest requirements before
GA. If SQLCipher is required, it affects bundled SQLite, cross-platform packaging,
key recovery, backup and migrations and must be tested in phase 0/its own ADR.
Do not claim keychain-stored tokens also encrypt cached private discussions.

Telemetry, if explicitly enabled under existing policy, records only redacted
aggregate latency/job/error categories. No repo identifiers, account names, text,
remotes, provider URLs, tokens, or raw errors. Keep a local opt-in diagnostic view
with bounded nonsensitive queue/rate/coverage information for support.

## 19. Database migrations and schema evolution

Version storage schema, public DTOs, provider facet schemas, normalized projections,
sync strategies, and command payloads separately. A provider adding a field does
not necessarily require a migration; changing identity or command interpretation
does.

Use embedded forward migrations with checksums and recorded compatibility bounds.
During bootstrap, open/migrate storage before remote commands/subscriptions become
ready; render native/React shell and local Git independently. Acquire the runtime
lock, stop sync dispatch, take a consistent backup for destructive changes, then
migrate transactionally where SQLite permits. Chunk large backfills with durable
progress and explicit readiness/coverage; do not hold UI hostage to a giant rewrite.

Migrate pending commands/drafts with versioned converters and preserve actor,
client UUID, delivery evidence, dependency ordering, and ambiguity state.
Unknown command versions are quarantined for recovery/user action, not replayed
with guessed semantics. Changed normalization may rebuild disposable effective
views/FTS/validators but cannot erase user intent.

An older app refuses unsupported newer schema with a recoverable message; do not
attempt automatic down-migration of dispatched commands. Test rollback via the
previous compatible binary and backup recovery, not just migration SQL syntax.
An older backup cannot include receipts created after it. On restore, advance a
recovery generation and quarantine all restored potentially dispatchable commands;
none auto-send because the snapshot says queued. Reconcile against independent
provider/attempt evidence or require manual resolution, especially for creates.
Test backup-with-queued-command → remote-success → restore-old-backup and prove
that no duplicate is sent. Retain recoverable newer receipts when available, but
do not make safety depend on their survival.

Separate recovery paths: rebuild provider cache/projections; repair/export durable
user intent; reconnect credentials. Never “delete database and resync” as a generic
response to migration/corruption. Test disk-full/interruptions and retain a safe
backup before replacement.
[SQLx migrator](https://docs.rs/sqlx/latest/sqlx/migrate/struct.Migrator.html),
[SQLite ALTER TABLE](https://sqlite.org/lang_altertable.html).

## 20. Testing and observability

Correctness tests are part of each phase, not deferred to the final provider.

| Layer | Required cases |
| --- | --- |
| Domain/identity | PR/issue aliasing, repo rename/transfer, nested namespace, custom base path, 64-bit IDs, deleted actors, unknown enums/facets |
| Provider contracts | Pagination/304/permissions/unknown fields, all relevant states, token types, API versions, field-mask authority, malformed URLs, response limits |
| Sync simulation | Equal timestamps, mutable page movement, overlapping windows, partial traversal, expired cursor, missing/deleted/inaccessible resources, independent child changes |
| Storage | Real temp SQLite/WAL, atomic page checkpoint, account-scoped foreign keys/FTS, no content leak across scopes, durability/reset/migration/backup |
| Outbox | Crash before/after each commit and dispatch boundary, lost response, duplicate IPC UUID, changed payload, 202, ambiguous create, refresh crash, command dependencies |
| Conflicts | Concurrent field edits, label sets, new head, stale inline anchor, remote rejected write, optimistic list/count consistency |
| Scheduler | Shared budgets, secondary throttle, Retry-After/poll minimum, clock skew, fairness, cancellation, queue saturation, circuit breaker |
| Subscription | Snapshot/register race, dropped/out-of-order hints, irrelevant sequence ranges, overflow/reset, log compaction, webview reload, epoch revocation |
| Frontend | Ready/missing/partial/offline/auth states, cached offline reads, pending/conflict receipts, account switch, real badges, keyboard/accessibility |
| Packaged runtime | Vault behavior, SQLite version/features, CSP/assets, sleep/resume, offline restart, no required Gitru cloud login |

Use deterministic fake provider servers, fake time/network, bounded replayable
fixtures, and property/state-machine tests for randomized response/command order.
Store sanitized fixtures without tokens/private user content; maintain schema
contracts when official APIs change. Real sandbox-account tests are an opt-in
integration lane with isolated credentials, quotas, and cleanup, not mandatory
public-network dependencies in normal CI.

Add a second provider's divergent fixtures before declaring the common contract
stable. Contract tests check semantics, not identical HTTP shapes.

Extend the current Vitest config to include a new client package. Use the existing
fail-closed Tauri command mocks. Rust tests operate on temp DBs and fake HTTP/vault
adapters. Extend packaged E2E isolation/reset to collaboration DB/assets and fake
credential namespace under `com.ruru.gitru.e2e`; never reset real user credentials.

The embedded driver cannot directly target child webviews. A fixed-action,
E2E-only event probe exercises the actual native-child Accounts handoff from one
retained main executor; this is a focused lifecycle regression, not general
multi-client synchronization coverage. Add a dedicated runtime integration
harness for multiple clients/subscribers and retain native lifecycle tests. Do
not claim broader multi-webview coverage from embedded-main-window E2E alone.

Track local read p50/p95/p99, commit/IPC payload latency, job age, scope validation
age, rate cooldowns, outbox pending/unknown counts, WAL/disk/cache sizes, and
subscription resets in nonsensitive local diagnostics. Performance regression
fixtures require fixed hardware/config and dataset definitions.

For implementation changes, run relevant focused suites followed by
`make typegen` when signatures change and `make verify`; packaged security/runtime
changes also require the separate E2E target. Local checks and remote CI remain
distinct. The current implementation evidence is recorded in section 23.

## 21. Phased implementation plan and acceptance gates

Implement reviewable vertical slices; do not build all providers or an elaborate
generic engine before testing real user flows. Phases are ordered by dependency,
not calendar estimates.

### Phase 0 — Feasibility and contract spikes

Deliver: minimal throwaway/prototype storage-runtime seam, fake provider server,
generated structured DTO/error/channel experiment, vault/auth experiments, and
a benchmark harness. Choose tested SQLx/SQLite/keyring versions, query DTO shape,
and Channel fallback only after packaged verification.

Verify:

- Tauri 2 typegen handles tagged unions, structured errors, string revisions, and
  subscription transport without manual generated edits.
- SQLite bundles the tested WAL fix/FTS features on Linux/macOS/Windows.
- A committed intent and snapshot revision survive crash/restart.
- Vault failures and account isolation are reproducible without real credentials.
- Direct GitHub PAT/explicit CLI import and GitLab PAT operation probes are viable;
  notification token and Bitbucket auth restrictions are explicit.

Gate: record spike results and final dependency/auth choices here. Prototype code
must not become an unreviewed shipping foundation.

### Phase 1 — Storage, identity, and local query skeleton

Deliver `crates/collaboration`, bootstrap runtime state, forward migrations,
accounts/instances/entities/aliases, base/effective query model, change log,
local-query commands, and `@gitru/collaboration-client`. Use fake data/provider.

Gate: account/scope-filtered local reads, deterministic pagination, race-free
snapshot/catch-up, reload/reset recovery, cold cached view, and offline local
navigation with no HTTP from query functions. Runtime lifetime is independent of
RepoContext/tab disposal.

### Phase 2 — GitHub read-only vertical slice

Deliver secure account connection, repository discovery/selection, PR and issue
summary feeds, body hydration, explicit local repo links, background scheduler,
conditional requests, rate budgets, and real PR/issue pages.

Gate: provider network never blocks cached navigation; bootstrap is progressive;
close/merge/rename and child changes reconcile; partial lists are labeled; app
restarts offline with useful cached views. Multiple webviews do not duplicate
provider jobs. Credentials never enter persisted frontend state.

### Phase 3 — Native inbox and richer PR details

Deliver GitHub notification credential profile where supported, capability-aware
inbox, reviews/comments/checks/timeline, active-detail prefetch, and cached avatars.
Replace dummy account buttons/badge. Introduce local disposition with clear
remote read semantics. Provider interaction in this phase is read-only; remote
mark-read/done delivery lands with the durable outbox in phase 4.

Gate: token-type restrictions handled honestly; badges come from effective local
data with coverage; local disposition has no remote write; stale-head
checks/reviews cannot masquerade as current. Unsupported inbox sources render a
capability state rather than fabricated native notifications.

### Phase 4 — Durable mutations and offline behavior

Deliver desired-state edits, operation-specific outbox, drafts, conflict UI,
delivery reconciliation, optimistic list/count/search changes, and guarded online
merge/review actions. Add non-idempotent creates only with explicit ambiguity UX.

Gate: crash tests at every delivery boundary; duplicate local submission cannot
duplicate dispatch; ambiguous writes never silently retry; rejected commands
preserve text; offline edits survive restart; refresh cannot overwrite pending
intent; remote mark-read/done observes activity-fence limitations; merge requires
a tested head guard.

### Phase 5 — GitLab abstraction validation

Start GitLab.com with PAT actor verification and repositories (R110), then add
MR/issues/todos and discussions/approvals facets; optional public-client
auth against selected supported versions. Add enterprise instance configuration,
registration, and private-host trust controls according to product priority.

Gate: ordinary common feature code requires no GitHub branches; todos semantics
remain explicit; subgroups/IIDs and multi-account hosts work; unsupported
plan/version features degrade through capabilities. Revise common contracts based
on actual differences before expanding further.

A small GitLab fake/read-only adapter should be exercised earlier during phases
0–2 to reveal contract bias; phase 5 is full supported-product integration.

### Phase 6 — Bitbucket and provider expansion

Add Bitbucket Cloud PRs, API-token auth, tasks/participants/draft facets and
locally derived activity where useful. Native issues/inbox remain unsupported.
Implement Data Center as a separate adapter only for explicitly tested versions.
Introduce container/work-item abstractions before Jira/group-scoped issue sources.

Gate: Cloud/DC tests are separate, opaque pagination honored, no removed issue
API dependency, merge concurrency limitations visible, account isolation intact,
and each supported feature has a declared capability/delivery policy.

### Phase 7 — Scale, reliability, and optional push

Deliver measured resource budgets, retention/eviction, large migrations, diagnostic
tools, accessibility polish, optional TanStack DB projection experiment, and
optional webhook relay if polling budgets/user demand warrant it.

Gate: targets in section 18 measured with tail latencies; memory/disk bounded;
backfill does not starve interaction; relay outages/gaps recover through polling;
no required Gitru login introduced. Security and provider contract tests pass
across supported platforms/versions.

Cloud mirror/multi-device draft sync receives a separate architecture proposal;
it must not silently change this desktop-first foundation.

### Planned repository additions

```text
crates/collaboration/
  src/
    domain/          # IDs, resources, availability, capabilities
    providers/       # contracts, github, gitlab, bitbucket_cloud, bitbucket_dc
    storage/         # writer, queries, migrations, effective projections
    sync/            # scopes, planner, scheduler, reconciliation
    commands/        # admission, outbox, delivery, conflicts
    subscriptions/   # revision, snapshots, catch-up
    errors.rs
  migrations/
  tests/             # fake-provider, crash, migration, property tests
apps/desktop/src-tauri/src/commands/collaboration.rs
packages/collaboration-client/src/  # client, query options, React hooks
apps/desktop/src/features/{pulls,issues,inbox,accounts}/
apps/desktop/src/bootstrap/collaboration-bridge.ts
```

Update Cargo workspace membership, Bun exports/dependencies, explicit Vitest
projects, bootstrap lifecycle, command registration/generation, and narrow security
capabilities as each phase lands. Keep route files thin per `docs/conventions.md`.
Use existing `@gitru/ui`/coss and relevant local UI/Effects skills during UI work.

## 22. Decisions to review and major failure modes

The recommended decisions are Rust runtime + SQLite, direct desktop auth,
account/scope-partitioned observations, explicit provider capabilities, local
query subscriptions, operation-specific durable commands, adaptive polling, and
progressive coverage. The following remain deliberate review points:

| Decision | Recommended position | Revisit trigger |
| --- | --- | --- |
| Enterprise rollout | Instance-aware from day one; test/ship by demand | User requires a specific enterprise host/version at launch |
| SQLx versus rusqlite | SQLx initially | Backup/build/threading/typegen spike exposes material cost |
| TanStack DB | Optional bounded projection later | Measured Query invalidation/list recompute cost justifies change |
| At-rest encryption | Explicit product/security decision before GA | Enterprise/private-data requirement mandates SQLCipher |
| Offline coverage | Selected repos + pins + recent window | Measured budgets/user needs require full history packs |
| GitHub inbox auth | Supported separate credential profile | Tested official token support changes |
| Webhook relay | Optional later | Polling rate/cadence cannot satisfy selected-scope UX |
| Hosted mirror | Defer | Multi-device/shared ingestion benefits exceed operational and privacy cost |
| Provider-native unsafe merge semantics | Strict guarded common API; separate capability | A tested endpoint establishes server-enforced head concurrency |

Top failure modes and planned defenses:

- “Fast” UI backed only by JS cache: disappears on reload → durable SQLite and
  local-only query functions.
- Timestamp feed treated as a complete log: missed edits → overlap, coverage,
  independent facets, and reconciliation.
- Partial paging treated as deletion: data vanishes → qualified absence plus
  direct verification.
- Multiple tabs each poll: rate exhaustion → one Rust scheduler and demand leases.
- Retry creates duplicate comments: timeout is ambiguous → durable attempt
  evidence and outcome-unknown UX.
- A refresh erases pending edits: double authority → base/effective separation.
- Account revoked but FTS/avatar still reveals content: incomplete isolation →
  centralized epoch/access gates across every read surface.
- Provider differences leak as switches everywhere: weak contract → capability
  metadata and typed facets with second-provider tests.
- Memory expands with repository size: whole replica hydration → bounded pages,
  projection caches, tiered content, explicit retention.
- Migration/reset loses pending work: treating intent as cache → independent
  durable intent semantics and recovery fixtures.

These constraints are more valuable than an elegant `PR.open()` spelling. The
ergonomic client should emerge from the tested lifecycle and correctness rules.

## 23. Decision and implementation record

| Date | Entry | State |
| --- | --- | --- |
| 2026-10-02 | Researched official provider/storage/sync docs and inspected current repository | Completed design research |
| 2026-10-02 | User confirmed provider accounts work independently of Gitru cloud sign-in | Confirmed requirement |
| 2026-10-02 | User authorized implementation using parallel agents | Accepted direction; staged implementation |
| 2026-10-02 | Independent repository, provider, and sync-correctness review; recovery and concurrency corrections incorporated | Reviewed design; implementation verification recorded below |
| 2026-10-02 | Isolated worktree from `dev`, branch `ruru/remote-collaboration` | Implementation in progress |
| 2026-10-02 | Added Rust storage/runtime/provider seam, generated IPC and local TypeScript client | Initial foundation; verification below |
| 2026-10-02 | Added GitHub.com PAT repository/PR/issue/native-inbox reads, selection, background paging and private drafts | Read-only slice; no live account or cross-platform verification claimed |
| 2026-10-02 | Replaced production collaboration placeholders and fabricated sidebar accounts/badge | Actual local query UI; fixture-only browser verification |
| 2026-10-02 | Workspace checks, 34 latest collaboration Rust tests, 166 frontend tests, release benchmark and two packaged macOS E2E specs passed | Verified initial read slice; live credentials and other platforms remain gates |
| 2026-10-02 | User selected manual PAT and optional existing GitHub CLI account import instead of a Gitru OAuth flow | Implemented native discovery/import, account picker, token creation links and regression coverage; validation below |
| 2026-10-02 | PAT/CLI follow-up checks: 178 frontend tests, 47 collaboration Rust tests, workspace Clippy/formatting, production frontend build and two packaged macOS E2E specs passed | Verified fixture/native boundaries; live credential import and other platforms remain gates |
| 2026-10-02 | User exposed unreachable Accounts form in content tabs; added single host dialog, tab-to-host UI request and native visibility suspension; corrected child repository-selection policy | Regression fixes; validation below |
| 2026-10-02 | Single-window Accounts follow-up: 194 frontend tests, 3 native caller-policy tests, workspace Clippy/formatting, lint/types, production frontend build, actual native computer-use inspection and both packaged macOS E2E specs passed | Verified host/child dialog handoff and tab restoration; live credentials and other platforms remain gates |

### Initial implementation contract and deliberate limits

The first slice uses a typed `RemoteItem` read projection and account-bound
`collaboration.forAccount(account)` client. Its local `items`, `item`, and
`repositories` calls never start provider HTTP. `refresh` is a separate queued
intent; drafts await the SQLite commit before the UI acknowledges saving.
Provider immutable native IDs identify PR/issue rows; repository/number are mutable
locators. The future full canonical graph, alias resolver, common mutation API,
base/effective outbox overlay and review/check/comment facets are not implemented.

Implementation locations are `crates/collaboration/src/{domain,storage,runtime}`,
`src/providers/`, `apps/desktop/src-tauri/src/{collaboration_setup,commands/collaboration}`,
`packages/collaboration-client`, and `apps/desktop/src/features/collaboration`.
Bootstrap uses `src/bootstrap/query-bridge.ts`. The engine is managed once per app
process, with an exclusive writer lease preventing a second process from opening
the same collaboration database and running another scheduler.

Chosen versions: SQLx **0.9.0**, `libsqlite3-sys` **0.37.0** (SQLite **3.51.3**),
keyring **3.6.3**, Rust **1.94.1**. OS vault access is serialized and uses the
blocking pool. Linux uses Secret Service, macOS Keychain and Windows Credential
Manager. [Keyring 3.6.3's platform feature documentation](https://docs.rs/keyring/3.6.3/keyring/)
supports these explicit native backends; there is no plaintext fallback.
SQLx migrations use `run_direct` so the initialization future is Send when Tauri
spawns it. Unix files are private; at-rest encryption remains a product gate.

The local release build exposed [Rust's macOS LINKEDIT alignment bug](https://github.com/rust-lang/rust/issues/157750)
while loading SQLx's proc-macro library. The workspace release build override
retains debug information for build-time dependencies; runtime release
optimization is unchanged. The release storage benchmark passes with this
setting. Revisit the workaround when upgrading the pinned compiler.

The current type generator misses injected `Webview`, Serde enum names and
nullable `Option` fields. `make typegen` therefore applies a fail-closed,
source-derived correction script, with generated-wire contract tests. Generated
files remain generated from Rust; the long-term gate is replacing this seam with
a generator that handles the complete Serde contract directly.

Native snapshots expose a decimal revision and persisted global authorization
view. Catch-up is bounded to 256 records and advertises `has_more`; 4,096 records
are retained before a reset is required. A listener is installed before catch-up;
clients serialize drains, repair event gaps, cancel/remove scoped caches on
lifecycle changes, and reject delayed reads from obsolete authorization views.
Events carry only revision hints. Draft commits also publish hints. Projection
pagination cursors bind the query, authorization view and SHA-256 digest of the
selected visible feed data revisions. Relevant observations or selections
require restarting the page sequence; unrelated accounts, drafts and sync-status
updates do not. The global revision remains the change-log catch-up position.

Feed continuation reuses a persisted run/checkpoint across the 10-page budget.
Only a complete single-page feed retains an HTTP validator; page-one 304 cannot
prove unchanged later pages. Mutable-feed membership requires **two** successful
complete enumerations before hiding unseen membership. Partial, failed and 304
responses do not increment absence; canonical cached rows/drafts survive. Pending
absence disables conditional validators so a second full traversal actually
occurs. Repository picker/list membership follows the same authorized visibility
rules, including notification-only references. Absence retains canonical detail;
actual discovery denial hides discovered repository data. A newly denied scope
advances the global authorization view and emits a reset, preventing delayed
private snapshots from repopulating UI caches. Access-denied scopes stay hidden
until a successful authorized observation.
`body_omitted` distinguishes unavailable oversized content from authoritative
null and prevents either accidental body erasure or resurrecting a cleared body.

The scheduler currently has one HTTP worker and a 128-scope admission limit,
rotates background candidates, honors persisted per-account REST quotas, and
uses bounded backoff/jitter. Known account quotas survive same-actor reconnect.
Provider token capture and authorization failures coordinate with the lifecycle
lock; old responses cannot revoke or populate a newly connected partition.
GitHub rename redirects and continuation links accept only the selected
repository's named resource path and exact immutable native-ID resource path;
other repository IDs, resources, hosts and credential-bearing URLs are rejected.
Periodic discovery is 10 minutes, selected PR/issues 3 minutes and inbox at least
60 seconds (provider hints can increase it). These are initial policies, not the
foreground-demand/weighted scheduling system described in the final design.

Current authentication is direct GitHub.com PAT or explicit import of a saved
GitHub CLI account, as specified in section 15.1. Opening the main account dialog
discovers account metadata in the background; it does not request a token.
Selecting an account retrieves its credential natively, verifies `/user` matches
the selected login, then uses the same OS-vault/account lifecycle as PAT entry.
The UI links to fine-grained PR/issue read permissions and explains classic inbox
scopes. Token creation and provider links use the existing native HTTPS opener
command, preserving the main webview's restricted plugin permissions.
No Gitru device/OAuth flow, enterprise host, GitLab, Bitbucket or cloud relay is
implemented. Inbox capability requires a classic `ghp_` or existing OAuth `gho_`
credential with observed `notifications`/`repo` scope. Fine-grained or unknown
credential types can connect for repository features but do not enable the
inbox. Notification subject hydration is pending; browser links currently lead
to the repository.
There are no remote writes, outbox dispatch, merge, mark-read, comments/reviews,
optimistic remote edits, or offline remote delivery yet. Private drafts are
durable, preserve text on generation conflict, and remain after disconnection;
an independent disconnected-account draft recovery UI is still needed.

Phases 0–3 are partially implemented; none is declared complete solely from local
tests. Remaining early gates include live packaged vault/PAT verification, all
supported OS builds, actual IPC/UI latency and memory measurements, independent
detail hydration, local repo mapping, foreground demand, retention/eviction,
backup/reset recovery and process-crash fault injection. Implement outbox delivery
before exposing remote mutation controls. Continue from the current code and this
record rather than assuming the broader proposed contracts are already present.

### Local verification evidence (2026-10-02)

The following records the initial read slice before the PAT/CLI follow-up below.

- Frontend: **166 tests** passed, including 18 client tests and 10 collaboration
  UI tests; root lint/type checks and the production desktop frontend build
  passed. Client lint is included in Turbo. `make typegen` generated 89 commands;
  wire tests cover Serde enum names, nullable fields and injected native inputs.
- Browser fixture QA: actual collaboration components at 1280 px and 390 px,
  including feeds, search, details, private draft saving and account settings.
  The narrow account dialog's scroll width equals its 390 px viewport width.
  Fixtures remain under tests and never seed production storage.
- Offline release storage benchmark: 10,000 cached issues, 50-row pages, 200
  reads each; indexed list p50 **223 µs**, p95 **243 µs**; FTS p50 **4,086 µs**,
  p95 **4,329 µs** on this Apple Silicon machine. Synthetic temporary data,
  warm local reads; excludes IPC/rendering and app memory consumption.
- Native: Rust workspace tests passed on this macOS host; the latest focused
  collaboration run passed **34 tests** (14 provider/credential unit tests,
  6 runtime tests, 14 storage tests). Strict workspace Clippy, formatting and
  E2E-feature compilation passed. Native caller authorization has a separate
  regression test in the desktop shell.
- Packaged macOS: optimized E2E app built and **2 specs passed**, covering real
  collaboration storage/IPC snapshots and the existing UI/Tauri/Rust/Git
  workflow. Artifacts are in
  `artifacts/e2e/2026-10-02T08-11-38-635Z-16564/`. The collaboration spec validates
  generated wire schemas against a fresh isolated database. E2E credentials use
  the in-memory test vault; the production OS vault was not exercised.

### PAT/CLI follow-up verification (2026-10-02)

- Frontend: **178 tests** passed, including **22** client tests and **18**
  collaboration UI tests. Root lint/type checks passed. `make typegen` generated
  **91** commands; generated-wire tests include CLI metadata enum names and
  opaque candidate input. Token creation and provider-link regression tests
  use the actual native HTTPS command name rather than an unavailable opener
  plugin permission.
- Native: **47 collaboration tests** passed (**24** unit, **9** runtime,
  **14** storage); strict workspace Clippy and formatting passed. Isolated
  temporary executables cover installation/replacement, environment filtering,
  stdin closure, output limits, timeout and process cleanup. Injected runners
  cover explicit user selection and candidate expiry; the `/user` identity
  mismatch test proves neither vault writes nor account creation occur.
- Browser fixture QA: actual account dialog at **1280 px** and **390 px**,
  multiple CLI accounts, connection with the required PAT field empty, discovery
  retry, status feedback, keyboard access to token creation, and Escape closing.
  The narrow dialog has **390 px** client and scroll widths; vertical scrolling
  exposes the PAT controls. Temporary preview entrypoints were removed.
- Final build: production desktop frontend build passed. The optimized packaged
  macOS E2E app passed **both specs**, including the real native CLI discovery
  command returning the disabled fixture state and the existing UI/Tauri/Rust/Git
  workflow. Artifacts are in
  `artifacts/e2e/2026-10-02T10-10-01-848Z-39343/`. The build continues to emit the
  existing frontend chunk/font warnings; no live accounts or OS vault entries
  were accessed by E2E.

### Single-window Accounts regression verification (2026-10-02)

- Frontend: **194 tests** passed, including **26** account/workspace tests and
  **8** native-tab visibility tests. The latter cover real creation versus the
  normal warm-up timeout, HMR survivors, delayed shows, draining failed hides,
  cold tabs during a modal, and restoring the currently selected tab. Root
  lint/type checks and the production frontend build passed; production output
  contains no E2E child-probe event names.
- Native: **3** caller-policy tests passed. Main-only credential discovery,
  connection and disconnection remain restricted; local child views may select
  repositories and read authorized domain data. Strict workspace Clippy and
  formatting passed. No command signatures changed in this regression fix.
- Computer-use QA: a bundled macOS build with a separate QA identifier, the
  in-memory E2E vault and disabled personal CLI discovery ran the real host/tab
  layout. Clicking Accounts in the actual Inbox child displayed the PAT form
  in the same window; closing the dialog restored Inbox. The QA check neither
  entered credentials nor connected an account. The unbundled dev executable
  could not be attached through the computer-use app selector.
- Packaged macOS: **both E2E specs passed** (**3 test cases**), including the
  actual Inbox child-to-host Accounts request, credential-form confinement,
  unchanged child instance/route after closing, and cleanup back to the
  embedded test route. The existing UI/Tauri/Rust/Git workflow also passed.
  Artifacts are in `artifacts/e2e/2026-10-02T11-30-35-130Z-86393/`.

No live PAT/CLI credential import, native production vault or Linux/Windows
validation is implied by these fixtures, native tests or the benchmark. No
personal CLI credentials were inspected and no remote CI run was created.

### Foundation publication and continuation (2026-10-03)

Current continuation adds [PR #147](https://github.com/ruru-m07/gitru/pull/147),
RURU-97, on the reviewed provider-registry branch, and
[PR #149](https://github.com/ruru-m07/gitru/pull/149), RURU-100, stacked on #147.
Both issues are In Review. Independent detail facets
now have cache-only queries, explicit epoch-bound coalesced hydration, bounded
paging/restart intent and atomic source/coverage/access/revision metadata.
Known null/empty, missing, omitted, oversized and partial observations remain
distinct. Retained body authority survives omitted/oversized and timestamp-free
304 responses; deselect/reselect cannot revive an old facet lease. A metadata-only
accessor shares the caller's SQLite snapshot for contextual capability reads.
Local validation passes 114 collaboration, 3 command-caller, 36 client and 177
desktop frontend tests, types/lint/Clippy/formatting, and the 9 CLI cases after
the ancestor fixture correction. Independent storage and runtime/client reviews
are accepted. Production GitHub detail endpoints/UI remain RURU-77/78, the next
read experience chunks. RURU-100's atomic account/repository/resource policies,
shared local deadline coordinator and ordinary workspace/sidebar consumers
pass 127 native, 44 SDK, 193 desktop and 3 caller-policy tests, scoped types/lint,
build/Clippy/formatting, both independent reviews and native synthetic fixture QA.
Same-actor grant refresh retains private text and inspected CAS; actor/subject
switching resets the buffer. RURU-99's separate editor/recovery extraction needs
narrow merge reconciliation. Migration 0004 adds rebuildable
details; RURU-106's restore policy still refuses unreviewed schemas 0003/0004.
These are published review stacks, not merged or release-qualified features.

Eight scoped draft PRs are published: #141 foundation, #142 credential cutover,
#143 migration recovery, #144 saved draft recovery, #145 backup/recovery native
core, #146 registry, #147 independent details and #149 contextual capabilities.
RURU-106 remains In Progress. The exact `c215389` foundation head passes frontend,
Clippy/formatting, Linux/macOS/Windows Rust, all CodeQL analyses and Linux/macOS
packaged E2E; Windows packaged E2E remained pending when recorded. Test-only
checked iteration repairs newly reported synthetic `Vec::remove` logging-model
alerts in #142/#147 without suppression, while retaining exact-one fixture and
original content assertions. These signed descendant heads have restarted CI;
completed ancestor checks do not qualify a child's current head. The hourly
continuation checks live issue dependencies, overlap and CI before proceeding.

A later #147 Linux run captured OS code 26 (`Text file busy`) at the inherited
credential crash-test snapshot spawn. Fixture audit/review corrected the only
two parent-written executable snapshots (RURU-95 and RURU-106) with waited Unix
child writers; no retries, test serialization or production change is introduced.
The observed error and source-inferred descriptor mechanism are distinguished
in the work notes. The signed repairs are propagated through the review stacks;
the current RURU-100 native suite again passes 127 tests. Fresh exact-head
Linux/platform and security checks remain pending.

The foundation received independent native and frontend review with no new
blocking findings. `make verify` passed on macOS on 3 October, including the
194 frontend tests, workspace lint/type checks, production frontend build,
Rust formatting/Clippy and all Rust workspace tests. The packaged macOS rerun
passed both specs and all three cases. The signed foundation commit
`baafef75e82743b756b412bd5d7bc443636c76c8` is published in draft
[PR #141](https://github.com/ruru-m07/gitru/pull/141) against `dev`; remote CI
and live provider/vault gates remain pending. RURU-138 is In Review;
follow-on issues use isolated branches from this foundation until its review
completes. RURU-95 (credential cutover),
RURU-99 (private draft recovery) and RURU-105 (migration fixtures) are the
first parallel batch, subject to live blockers and explicit file ownership.

That batch is now In Review in separate signed stacks:

- [PR #142](https://github.com/ruru-m07/gitru/pull/142), RURU-95: versioned native
  vault references and a durable staged/retired cutover journal. Local validation
  passed 58 collaboration tests, including 25 actual process-kill checkpoints.
- [PR #143](https://github.com/ruru-m07/gitru/pull/143), RURU-105, stacked on #142:
  frozen v1→v2 upgrade/downgrade-refusal and migration-failure recovery. The
  integrated local suite passed 70 tests, including 12 migration cases.
- [PR #144](https://github.com/ruru-m07/gitru/pull/144), RURU-99: bounded private
  draft recovery without active provider access and generation-bound native
  export. Local validation passed 208 frontend, 49 collaboration and 11 desktop
  tests. Native bundled fixture QA verified two disconnected actors with the same
  missing subject, exact Unicode export, save-first editing, copy, cancel/reopen,
  and private file permissions. Recovery covers explicitly saved drafts.

Each branch records its design and evidence in
`docs/architecture/collaboration-work/RURU-<number>.md`. Remote CI remains a
separate gate. Foundation CI exposed an initial child-probe readiness race and
four CLI fixture `Vec::remove` logging-model alerts; scoped test fixes passed
local packaged E2E/CLI checks and the exact-head remote matrix was restarted.
No production credential logging or rule suppression was introduced.
Exact-head dynamic CodeQL runs have now been observed on stacked children
#142, #143, #146 and #147. Each merge still requires every relevant analysis to
complete and zero relevant alerts on the exact proposed head. Do not infer a
policy change from the observed run availability or reuse an ancestor's scan.

CI follow-up: production migrations and frozen SQL fixtures now explicitly use
LF in Git attributes, preserving SQLx checksums and literal text on Windows.
The migration stack's exact `d776d663` head passes Windows Rust tests. The
foundation's `7c5364d` CodeQL analyses and aggregate pass with zero open PR alerts.
Its Linux collaboration assertions passed, but the subsequent desktop-smoke
session failed to find the host; artifacts recorded a native view registering
after cleanup. A delayed dialog suspension release is a source-backed causal
inference. The host now invalidates its owner and geometry immediately on
unmount, fences asynchronous lookup/creation/show, and waits for old native close
before remount adoption. Six lifecycle regressions pass; four failure cases
fail against the previous source. All 200 frontend tests, desktop/E2E types,
scoped lint and fresh packaged macOS E2E (both specs/all three cases) pass.
The E2E order and assertions remain unchanged. The new remote matrix remains
required; this local repair is not a Linux/Windows packaged pass claim.

Subsequent exact-head CI on `e1ad8c569` passed Linux and macOS packaged E2E
and all three CodeQL analyses. Windows checks were still pending when recorded.
The provider-registry stack's Linux Rust job `111149561690` exposed an
intermittent newly-installed CLI fixture failure before its first account
assertion. Its original status/OS error was not captured, so the exact cause is
unconfirmed. The fixture wrote its executable in the multithreaded test parent,
matching the concurrent-fork writable-descriptor race documented in
[Rust issue 114554](https://github.com/rust-lang/rust/issues/114554).
Unix fixture publication now writes synthetic scripts in an isolated child,
closes input and waits for writer exit before chmod/symlink publication. The
parent never owns the executable's writable descriptor. All existing discovery,
upgrade, selected-account, timeout and output-bound assertions remain; safe
status assertions improve future failure evidence. Nine focused CLI tests, all
47 foundation collaboration tests and all-target Clippy pass locally on macOS.
No production runner behavior, retry, test ordering or global
test serialization changed. Fresh Linux CI remains the qualification gate;
this source-backed correction is not a locally reproduced Linux failure claim.

Windows Rust jobs `111152076847` and `111152155586` then recorded a distinct
expiry-fixture panic: subtracting five minutes from `Instant::now()` underflowed
on a freshly booted runner. CLI candidates now store an equivalent monotonic
expiry deadline and are accepted only strictly before it. The existing expired
selection test sets the deadline to now; a deterministic boundary test covers
fresh, before, exactly at and after expiry without backdating an instant. Account
selection and the five-minute lifetime are unchanged. Independent review accepts
the equivalence. All 48 foundation collaboration tests pass locally; actual
Windows CI remains required for this platform-specific correction.

Those lanes are published in separate draft PRs based on #143:

- [PR #146](https://github.com/ruru-m07/gitru/pull/146), RURU-76: explicit
  installation registry, canonical identities, persisted aliases and local
  capability/resource resolution. Local checks pass 83 native, 32 client,
  171 desktop frontend and 3 command-caller tests. Independent review found an
  initial pending-query invalidation race; the shared bridge now cancels affected
  provider reads before invalidation, with seven real QueryObserver regressions.
  Authored draft writes retain their separate generation rules. A synthetic
  10,000-row warm identity lookup measured 78µs p95, excluding IPC/rendering.
- [PR #145](https://github.com/ruru-m07/gitru/pull/145), RURU-106 native core:
  verified WAL-consistent snapshots, physical credential-reference redaction,
  inspected nonce/checksum/CAS replacement and preserved original bundles.
  Local checks pass 88 native tests, including 14 independently authored recovery
  cases and nine real hard process terminations. Newer current drafts remain
  recoverable in the original bundle when incoming drafts replace active data.
  The issue stays In Progress: actual writer shutdown, native picker/dialog UI,
  reviewed schema-0003 recovery policy and Windows power-loss qualification remain
  open. Core policy deliberately refuses v3/unknown/outbox schemas. The parallel
  stacks do not yet form a release-qualified combined backup feature.

This batch adds RURU-97 independent detail storage/hydration in draft #147 and
RURU-100 contextual capability consumers from the reviewed #146 contract.
RURU-97 is In Review; RURU-100 remains In Progress. Both record their contracts
before major edits. Detail coverage
is separate from summary coverage; local reads never initiate provider HTTP.
Contextual capabilities distinguish authorized saved reads, remote sync and
remote writes across account/repository/resource targets. Temporary network or
quota errors must not hide still-authorized cached content; access denial does.
Private draft recovery stays independent. Shared IPC generation is sequenced
after the detail contract freezes. Publication authorizes review; no PR is
merged without user authorization.

RURU-100's complete contextual slice is locally implemented atop the signed
RURU-97 contract. Its separate native reader captures account/repository/resource
ownership, authorization, visibility, detail evidence and both facet/account
quota barriers in one SQLite snapshot. Saved reads, synchronization and remote
writes remain distinct; every remote write is explicitly unsupported. The
ordinary workspace and sidebar use typed policy/inbox semantics, with shared
unsupported/denied/missing/read-only boundaries and private drafts outside
provider gates. One bridge-owned local deadline timer repairs eligibility after
cooldown expiry. Local evidence passes 127 collaboration, 44 SDK and 193 desktop
tests, including actual authorization-reset/CAS and pending-query races; both
independent reviews accepted the source. Native fixture QA and final publication
gates are recorded in [RURU-100's work note](./collaboration-work/RURU-100.md).
RURU-99's separately published authored recovery/editor extraction still needs a
narrow merge reconciliation preserving recovery/copy/export and its own CAS
rules. Remote CI/security and live-account qualification remain separate gates.

### Resource description and metadata contract (2026-10-03)

RURU-77 and RURU-78 now implement cached PR and issue details from #149's signed
`6799e6d` head in isolated managed worktrees. Their approved common contract is
recorded before implementation in [RURU-77's work note](./collaboration-work/RURU-77.md).
The existing Body facet owns one resource endpoint; description and typed
metadata publish atomically under the same native dispatch and authorization
fences. Migration 0005 is additive rebuildable metadata storage. Each field keeps
its saved source clock, validation time and latest observation; absent/null/empty/
oversized values remain distinct. Matching conditional responses validate only
fields known in their preceding representation. Selected detail headers can
project authorized endpoint values without implying fresh list membership.

The PR adapter and common native contract are one lane; the issue mapper and
synthetic fixtures are another. A single frontend owner consumes the frozen
generated contract, avoiding competing common UI or schema changes. Validation,
publication and live provider qualification are pending for these slices. Live
CI is now green for every reported check on all eight published heads, including
Rust and packaged E2E on Linux/macOS/Windows. Five report completed CodeQL
analyses (#141/#142/#143/#146/#147); no current exact-head run is observed for
#144/#145/#149, which retain that separate security gate. No PR has been merged.

### Linear implementation backlog (2026-10-03)

The remaining work is organized under
[RURU-53](https://linear.app/catra/issue/RURU-53/build-a-provider-independent-local-first-remote-collaboration-engine)
as **49 direct sub-issues**: 44 new issues and five existing children updated
to concrete remaining slices. See
[remote-collaboration-backlog.md](./remote-collaboration-backlog.md) for the
published issue index, acceptance criteria and verified blocker relationships.
The publication snapshot has 106 direct dependency edges and no cycles; six
immediate independent lanes are Todo. Recheck live Linear state before picking
work, especially when multiple contributors share files.

The backlog follows the accepted independent desktop account and PAT/CLI
decisions. It includes persisted locator aliases, independently cached details,
foreground demand, retention and restore gates before remote delivery, and
separate GitLab/Bitbucket rollout. Enterprise/Data Center/webhook assessments
remain conditional later scope. RURU-138 tracks review and publication of the
existing local foundation; creating this backlog does not imply implementation,
production authentication verification, a published PR or completed remote CI.

For future work, read this document and relevant current source before editing.
Record accepted architecture changes, phase completion evidence, unresolved
integration gates, and tested provider/server/dependency versions here. Do not
mark a phase complete from a demo or local-only result while its acceptance gates
remain unresolved.


### RURU-77 local qualification

The shared cached resource metadata contract and GitHub pull detail endpoint
are locally complete in the signed review lane. [RURU-77's work note](./collaboration-work/RURU-77.md)
records 143 native, 51 SDK, 208 desktop and three caller-policy passing tests,
forward migration 0005, generated IPC and accepted independent reviews. Actual
isolated macOS WKWebView QA verifies immediate cached metadata, known-null body,
large wrapped labels, actor isolation and explicit-save draft persistence. A
label-height overlap found during native QA was fixed and the app rebuilt.
No personal credential or live provider was used. Exact-head remote CI starts
after publication; RURU-78 will stack its issue adapter on this frozen contract.


### RURU-78 local qualification and shared detail delivery

RURU-77 is published in draft [PR #150](https://github.com/ruru-m07/gitru/pull/150)
at signed `7e282ad`, with exact-head remote CI running. The companion
[RURU-78 work note](./collaboration-work/RURU-78.md) records the integrated GitHub
issue endpoint and capability, eleven new mapper/HTTP/store cases, seven new UI
cases and actual isolated native WKWebView issue/account/draft QA. Local totals
are 154 native and 215 desktop passing tests; formatting/Clippy/types/lint and
packaging pass. Shared schema 0005/IPC/view code is inherited unchanged. Issue
endpoint identity rejects PR representations and retains field authority/304
masks. Signed draft publication and remote CI remain separate from live provider
qualification and merging. RURU-98's approved foreground lease contract follows.


### RURU-98 implementation contract

[RURU-98](./collaboration-work/RURU-98.md) is the next bounded scheduler slice,
starting from signed cached PR details #150. Native-issued, caller-owned
45-second activity leases coalesce visible list/detail demand; a 15-second SDK
heartbeat renews liveness without provider polling or durable automatic intent.
Native host visibility/close/window gates fence owners. One committed HTTP page
is the scheduling quantum, with preserved traversal checkpoints, weighted
interactive/reconciliation and account rotation, bounded queue reservations and
strict persisted quota/retry/poll barriers. Manual Sync remains explicit durable
intent. The approved work note defines deterministic lifecycle, saturation,
offline and fairness acceptance; implementation and qualification are pending.


### Exact-head read-detail CI checkpoint — 3 October 2026

Signed PR [#150](https://github.com/ruru-m07/gitru/pull/150) (`7e282ad`) and
[#151](https://github.com/ruru-m07/gitru/pull/151) (`863967f`) each pass all 11
reported checks, including three-platform Rust and packaged desktop E2E. No
exact-head CodeQL check is reported; security and production provider/vault
qualification remain separate. Both remain open and unmerged. RURU-98 lease/page
fairness integration and RURU-96 native local-link integration are in progress
in isolated worktrees with saved contracts.


### RURU-98 local qualification checkpoint — 3 October 2026

Foreground demand now uses caller/session/account/scope-bound ephemeral leases,
native activity/visibility/expiry fences and one bounded SDK heartbeat. Automatic
interest creates no durable detail intent; manual Sync remains explicit. The
single native worker yields at committed pages, preserves traversal checkpoints,
rotates accounts/scopes and reserves selected-detail capacity while guaranteeing
reconciliation turns. Native cadence and persisted provider/retry/quota deadlines
control eligibility; renewals cannot reset those barriers. Same-label failed
close-disposal and slow-start readiness are fenced and tested.

Local159 native/82 SDK/223 desktop tests,6 caller cases, types/lint/Clippy/fmt,
normal103-command generation, frontend build and actual isolated two-native-tab
macOS QA pass. Post-Quit SQLite confirms zero automatic durable detail demand,
zero credentials, preserved metadata/drafts and strict cooldowns. Detailed bounds,
review findings/fixes and evidence are in [RURU-98's work note](./collaboration-work/RURU-98.md).
Signed draft publication/remote CI is the next step; live provider/vault and
R102/R103/R121 gates remain distinct. RURU-96 native links and SDK/UI navigation
continue in an isolated sibling worktree; no partial link UI is claimed complete.
No PR has been merged.


### RURU-98 restack onto cached issue details — 3 October 2026

The two signed RURU-98 commits from published PR #152 head `9046322` are replayed
onto signed RURU-78/PR #151 `863967f`, inheriting GitHub issue details. Both
chronicles remain intact. The only code-range adjustment is a test-only issue
omission case that now proves one account/epoch/subject Body lease across two
revisions and zero automatic durable hydration, preserving its cached text and
staleness assertions. Runtime, schema and generated command contracts are
unchanged.

Combined local validation passes 170 collaboration, 6 caller, 82 SDK and 230
desktop cases, including 12 frozen migration and 7 focused issue-view cases;
types/Biome/workspace Clippy/formatting pass. Full evidence and old-head versus
new-head qualification boundaries are in [RURU-98's work note](./collaboration-work/RURU-98.md).
All 11 reported remote checks passed on the old `9046322` head only. Parent
review/publication and the new exact-head matrix remain pending; prior native
macOS QA is not represented as a rerun of this combined branch. No merge occurs.

### RURU-96 implementation contract

[RURU-96's work note](./collaboration-work/RURU-96.md) records the next independent
local-clone link slice before implementation. Native Rust enumerates bounded,
credential-safe effective Git remotes and matches only exact configured provider
instances/transport aliases. User-confirmed links preserve durable local and
immutable remote IDs with CAS/proof/access fences; migration 0006 retains authored
intent independently of provider cache. Actual inspect/change/remove, ambiguity
choices and navigation both ways are required before completion. No Git remote,
branch, provider demand or credential is changed by cached navigation. Conditional
enterprise and physical directory relocation remain separate scope.


### RURU-96 local qualification and stack integration

RURU-96 implements bounded safe Git observations, explicit account/endpoint
choices, durable authored links and transport mapping CAS, actual settings and
navigation in both directions. [Its work note](./collaboration-work/RURU-96.md)
records 172 native, nine caller, 61 SDK and 249 desktop passing tests plus real
isolated macOS clone-import/link/change/remove/mapping/restart QA. UI findings
were fixed and rebuilt; four authored drafts and strict quota barriers remain
intact with zero credential records. This synthetic qualification is separate
from remote/platform/security/live-account and schema0006 recovery gates.

The review stack is being aligned to R78 → R98 → R96 before R79 storage starts.
RURU-98 PR152 old signed head9046322 passed all11reported checks including all
three native/E2E platforms, with no exact-head CodeQL reported. Its signed local
restack onto R78 includes 170 native/82SDK/230desktop passing checks; new-head
remote CI awaits publication. No PR is merged, and generated IPC is regenerated
only after the combined native command/source contract freezes.


### RURU-96 combined review publication checkpoint — 3 October 2026

The review stack now follows R78/#151 → R98/#152 (`2d415938`) → R96. Authored
local clone/account/endpoint links, explicit transport mapping settings and cached
navigation are qualified together with the native foreground scheduler. The
[RURU-96 work note](./collaboration-work/RURU-96.md) records 188 collaboration,
11 caller, 93 SDK and 264 desktop tests, types/lint/workspace Clippy/formatting,
normal110-command generation and a fresh combined native macOS GUI check. Cold
saved routes, two duplicate-name clones, mapped SSH push actor scope and main-host
settings suspension/resume all pass. Post-Quit preserves three authored links,
one mapping, four drafts and strict cooldowns with zero credentials or automatic
durable detail intent. R106's recovery ceiling remains unchanged.

R96's signed draft publication targets #152; its own exact-head CI is pending.
At this checkpoint #152's restacked head passes reported frontend, native
formatting and all three Rust platforms plus Linux/macOS packaged E2E; Windows
packaged E2E is still running. No exact-head CodeQL is reported. No PR has been
merged. R79's reviewed parser stage remains isolated; SQL0007 may begin only once
this actual0006 ancestor is established, with strict notified-subject provenance
and explicit finite identity discovery as recorded in its approved work note.


### RURU-79 notified-subject integration — 3 October 2026

RURU-79 now implements the recorded contract atop R96/#153's actual signed
repair `0eb71a5` (inherited by signed merge `ab958df`). Forward0007 adds only
rebuildable selector/discovery state and indexed immutable-alias lookups;
previous migration bytes and R106's conservative recovery ceiling are unchanged.
A cache-only account/instance/immutable-parent/kind/number resolver opens PR/issue
Body and metadata immediately. Only current, non-denied inbox provenance grants
that exact subject independently of local repository selection. All hidden
canonical claims and contradictory immutable Native representation targets take
part in ambiguity. Mutable path/web aliases after a rename and several distinct
issue-side aliases naming one PR do not create false conflicts.

Notification membership/denial withdrawal fences pending detail/discovery and
provider projections atomically, preserving private drafts and their generations.
Existing independently selected cached detail remains readable after discovery
absence; actual discovery denial still wins. The old absence-versus-denial
regression remains unchanged. Shared item/detail/context/demand predicates and
point commit recheck the same grant rather than creating a second access policy.

Explicit identity discovery has durable finite intent (16/account,64/process,
three attempts per generation), one HTTP worker and strict persisted REST/retry
barriers. It accepts no renderer URL or native ID. PR responses prove immutable
parent via their own base repository; named-only issue responses remain typed
IdentityUnverified. Unsolicited304 cannot bootstrap identity/content. Verified
identity, aliases, bounded summary and shared Body/metadata publish in one point
transaction without feed membership, cursor, selection or mark-read writes.
Same-epoch stale responses may preserve consumed account quota only; stale
subject/error/denial/intent effects and old epochs remain fenced.

Normal112-command generation adds eight named schemas and changes no existing
schema semantics. Local evidence passes249 collaboration tests (two existing
ignored subprocess helpers), all12 frozen-v1 migration cases,11 collaboration
caller/lifetime tests (15 complete desktop-native package tests),106 SDK and287
desktop tests. Workspace all-target Clippy and native formatting pass. Fresh isolated macOS
QA and cold restart pass with six separate authored drafts, both parents
unselected, eight unchanged unread notifications, zero credentials and one
strictly paused explicit intent (attempts0). Fresh packaged macOS E2E passes all three cases; review publication and
exact-head remote CI are tracked on the linked RURU-79 and attached PR. Independent adversarial review
reproduced and repaired older-selector rollback, immutable representation
ambiguity, and a missing provider:rest subscription dependency. The shared
frontend retains canonical private draft CAS separately from historical thread
drafts and acknowledges explicit durable read receipts. Read experience never
silently selects a repository or marks a notification read.

R96 repaired head `0eb71a5` now passes all11 reported checks, including
Linux/macOS/Windows packaged E2E and all three Rust platforms; the final Windows
E2E check completed2026-10-03T12:11:26Z. R98 head `2d415938` passes all11 reported checks. Neither reports
exact-head CodeQL. R79 is locally qualified for review; live provider/vault and platform/security
qualifications remain distinct from its exact-head remote CI. No PR has
been merged. See [RURU-79's work note](./collaboration-work/RURU-79.md) for current
native proof, frontend and isolated QA evidence.


### RURU-101 qualification lane — 3 October 2026

Live dependency/PR/worktree overlap audit selects independent facet reconciliation
qualification at R79/#154 `1a51a75`, in its own attached worktree.
[R101's pre-code contract](./collaboration-work/RURU-101.md) requires a source-backed
validator/overlap/absence/head policy and independent observable regressions before
any storage correction; provider facets that are Unsupported remain so. R110's
GitLab account/repository lane uses a separate checkout. Parent #154 currently has
a real macOS Rust socket-fixture failure, which root repairs before publishing
next PRs. Local checks, exact-head CI and live provider/vault gates stay distinct.
No PR has been merged.


R101 source audit establishes that `whole_scope` means validator authority, not
full-enumeration absence. Six independent cases produce four actual gaps:
collection clock loss after304, a nullable child timestamp erasing retained-field
clock evidence, continuation representation drift qualifying absence, and current-
head checks remaining fresh after an accepted new head. Full multipage absence
and historical Reviews are green controls. The approved work note records narrow
schema-free native receipt/provenance/field-clock corrections before source edits,
with conservative legacy JSON and unchanged public DTOs/provider support.

### Next bounded lanes after RURU-79 review publication — 3 October 2026

[R79/#154](https://github.com/ruru-m07/gitru/pull/154) is published at signed
`1a51a75a0f122c1e41288a9b6b6ca09421a38025`, stacked on repaired R96/#153.
Its exact-head remote matrix is running; local249 native/106SDK/287desktop,
frozen migration12, caller11, isolated native cold restart and packaged macOS
E2E3 pass. Parent R96 and R98 each pass all11 reported checks; CodeQL/live
provider/platform recovery gates remain distinct. No PR is merged.

Live Linear, PR, worktree and file-overlap audit selects R110 GitLab.com accounts/
repositories and an independent R101 facet reconciliation qualification lane.
Both begin in attached isolated worktrees at the actual R79 head, preserving its
shared scheduler/access/cache contracts; implementation blockers are reviewed
ancestors, not silently marked Done. [R110's contract](./collaboration-work/RURU-110.md)
records direct PAT operation probes, fixed installation trust, the existing owned
vault cutover and an explicitly repository-only profile. OAuth/PKCE, enterprise,
Data Center and relay spikes stay separate. R101 must preserve conservative
qualified absence and add independent facet/validator/head/epoch evidence, not
rewrite existing paging or invent unsupported provider operations.


### RURU-110 local implementation qualification — 3 October 2026

GitLab.com manual PAT actor/member-project probes and a repository-only native
profile now use the shared owned vault journal/cutover, immutable actor/installation
identity, SQLite selection and resumable keyset feed. GitLab MR/issues/inbox/write
facets remain explicitly Unsupported; accounts remain independent of cloud.
A fixed main-only command is normally generated113, with every existing284 named
Zod schema semantically unchanged. SDK/UI add a transient manual form and accurate
provider/capability guidance. No migration changed (frozen0001..0007), and R106
restore's v1/v2 acceptance ceiling remains separate.

Independent tests/reviews drive fixes for long valid quota truncation,503/redirect
Retry-After loss, unparseable extreme deadlines, proven-actor partial-probe quota
loss and a picked worker bypassing a newly stored barrier. Quota merge is atomic
under writer/current-epoch guards, including known inactive actors without grant
reactivation; successful probe quota commits with credential promotion. Existing
GitHub CLI/login/cancel/crash/cleanup behavior and authored cache/draft boundaries
remain tested. Local qualification:285native+2existingignored,12frozenmigration,
37focusedGitLab (six independent actual local HTTP/lifecycle cases),110SDK,299
desktop (12 new GitLab cases), types/scopedBiome/frontend build, all-target
collaboration Clippy and format pass. Synthetic fixtures do not qualify a live
PAT, production vault or other platforms. Root still integrates the repaired
R79 ancestor and qualifies caller/native GUI/package before PR publication.
No PR is merged. See [R110 work note](./collaboration-work/RURU-110.md).

### Exact-head remote CI repair — 3 October 2026

Published R79/#154 `1a51a75` passes frontend/lint/types/build, Clippy, Linux
Rust and packaged Linux/macOS E2E in run37122670576; Windows is still running.
The macOS Rust job111201717043 fails two synthetic HTTP fixture cases at
notification_subject_discovery/tests.rs345 with Darwin WouldBlock35. The
nonblocking listener's flag is inherited by accepted sockets on that platform;
read timeout does not turn them blocking. The test helper now explicitly makes
the accepted socket blocking before the existing bounded read timeout. No
production HTTP behavior, deadline, assertion or CI ordering is changed.

Local correction: all16 focused notification-discovery adapter cases and all120
collaboration library cases pass (one existing subprocess-entrypoint ignored);
workspace formatting passes. Previously qualified GUI/package evidence remains
on its recorded executable; fresh remote checks qualify the repaired PR head.
Logs: `/tmp/gitru-ruru79-macos-ci-failure.log`,
`/tmp/gitru-ruru79-accepted-socket-tests.log`,
`/tmp/gitru-ruru79-accepted-socket-lib.log`,
`/tmp/gitru-ruru79-accepted-socket-fmt.log`. No PR is merged.

### RURU-110 integrated native qualification and review — 3 October 2026

Signed native source719fb63 inherits the R79 socket repair; signed picker04af1e8
adds provider-aware labels after actual same-login native QA. All285 native cases
(two existing ignored helpers), all12 frozen-v1 migration cases,15 native caller/
updater cases,110 SDK and final300 desktop cases pass. Normal113-command typegen
preserves all284 existing named schema semantics. Actual desktop/E2E types,
scoped Biome, production frontend build, workspace all-target Clippy-Dwarnings
and format pass. Fresh macOS packaged E2E passes all3 cases/two specs after the
picker correction, including real UI→Tauri→Rust→Git and child Accounts handoff.

Dedicated e2e-feature app `com.ruru.gitru.ruru110.qa` disables native vault/CLI and
uses task-only empty Git configuration. Final rebuilt executable SHA256
`7e9f5e6f6995584ef8135e53fe18b1b734cc257bb0bdcef24291820a6fb8ee76` shows
main-only manual GitLab PAT form through actual child handoff and dialog scrolling,
distinct GitHub/GitLab labels, cached nested repositories and typed unsupported PR
feed. Cold process reopen preserves GitLab-A selections2, GitLab-B0 and same-login
GitHub0. Post-Quit readonly SQLite proof preserves six native repository identities,
three private drafts generation1, zero credential refs/cleanup/automatic detail
intent, and all three strict2099 provider barriers. No token was entered/submitted;
no personal credentials/config/vault or live provider was inspected. Evidence:
`/tmp/gitru-ruru110-qa/final-post-quit-evidence.json`, final source logs and
`/tmp/gitru-ruru110-picker-packaged-e2e.log`.

Parent R79/#154 repaired e6fe69e now passes all11 reported exact-head checks,
including Rust and packaged E2E on Linux/macOS/Windows; final Windows cleanup
completed2026-10-03T13:24:08Z. No exact-head CodeQL is reported. R110's own remote
matrix starts on publication and is separate from local Mac/synthetic evidence.
Live PAT/production vault, other platforms and power-loss gates remain distinct.
No PR is merged. R101 facet qualification and R103 retained native-webview harness
continue in separate attached worktrees; final shared source integration is
sequential. See [R110 contract](./collaboration-work/RURU-110.md).

### RURU-101 integrated facet reconciliation qualification — 4 October2026

Signed source1cb9bea integrates R110/#155725b9f2 and repaired R79/e6fe69e.
Full combined native qualification completed3 October:311 cases pass, including
165 library and146 integration cases, with two existing ignored subprocess
helpers; all12 frozen-v1 migration cases and15 native caller/updater cases pass.
Workspace all-target Clippy-Dwarnings and formatting pass. The one-off initial
cold-reopen OS Busy does not recur in the combined run; weak ownership and fatal
immediate reopen assertions remain. Its original cause is not claimed resolved.

Normal make typegen regenerates113 commands; all285 existing named Zod schemas
preserve normalized initializer semantics and public DTOs stay unchanged. After
interruption, resumed4 October SDK110, actual SDK/desktop/E2E types, source binding
lint and the285-schema comparison pass. Regenerated files are generator output,
including ordering/cache metadata changes, not handwritten IPC. Native receipt,
versioned stored provenance and finite field clocks remain private to the engine;
frozen0001..0007 and R106's restore ceiling are unchanged.

Independent source-backed regressions drive retained clocks through304/null
observations, qualified full collection absence, continuation representation
restart, current-head stale/metadata conflict veto, pre-HTTP captured lease/subject
fences and exact old-job terminal cleanup. Historical Reviews and ordinary feed
paging/absence/selected-cache/private CAS remain qualified controls. Full trusted
enumeration is distinct from terminal delta/uncertain paging and whole_scope
continues to mean validator coverage. No production Comments/Reviews/Checks
adapter is enabled; future endpoint/head policies and live provider tests remain
separate rollout tasks. No UI behavior or Tauri authority is expanded.

A review PR is stacked on published R110/#155; this new head's remote CI starts
at publication. ParentR110 Rust and packaged E2E on all three platforms plus
CodeQL pass, while its Vercel API deployment status remains Pending and the
connector currently requires reauthentication. Local evidence does not replace
new-head CI, live provider/vault or power-loss qualification. No PR is merged.
R103's retained native-webview harness inherits this final source sequentially.
See [R101 work note](./collaboration-work/RURU-101.md).


### RURU-111 GitLab MR/issue reads qualified locally — 4 October 2026

GitLab.com selected projects now synchronize all-state MR and issue summaries and
independent singleton Body/common metadata through the existing native scheduler,
SQLite, provider traits and ordinary shared detail UI. Immutable global IDs remain
separate from project IIDs; numeric routes and bounded account/epoch/project/kind
cursors preserve identity across renamed display paths and copied issue moves.
Unconditional MR creation-order offset and issue ID keyset traversals fail partial
on malformed/replayed/cross-scope paging; neither is claimed an atomic snapshot.
List clocks do not validate Body. Omitted/null/empty/oversized descriptions and
missing asynchronous MR diff references retain explicit states. GitLab inbox,
discussions, approval/check facets, remote writes and self-hosted instances remain
unsupported. No migration, public DTO or generated IPC signature changes occur.

Local evidence: full collaboration329 passed/two ignored, workspace all-target
Clippy-Dwarnings and format, complete frontend413 tests/48 files, repository lint,
types including desktop/E2E/testing and production frontend build pass. Shared
GitLab MR/issue UI tests preserve private drafts and use ephemeral detail demand
without durable hydration. Actual synthetic HTTP-to-Runtime-to-SQLite regressions
cover inert cold reopen, copied identities, concealed404 and late old-epoch cache/
quota veto. No personal credentials, live provider or production vault was used.
The review PR is stacked on #156; its remote platform checks qualify its own head.
No PR is merged. See [R111 work note](./collaboration-work/RURU-111.md).

Vercel reauthentication is resolved: #155's exact725b9f2 deployment
`dpl_6UXbMPbqvvj1Fd6RXKGtCFFMiBdU` is READY with build completed3 October13:41:13Z,
although its GitHub check still reports Pending. No check override or redeploy is
claimed. #156's reported11 checks pass; no exact-head CodeQL is reported there.


Final independent review found an async diff-reference missingness edge. Base now
requires both the MR SHA and diff_refs.head_sha to be known, nonempty and equal
before accepting start_sha. Missing, null, empty and mismatched heads leave Base
Omitted while independently qualified Body/title/state remain available. Eight
actual HTTP shapes preserve the known matching-head target-start control. After
this correction the full collaboration suite still passes329/two ignored,
focused HTTP14/14, workspace all-target Clippy-Dwarnings and formatting pass.
Logs: /tmp/gitru-ruru111-async-head-{http,full,clippy,fmt-check}.log.


### RURU-111 inherited Linux immediate-reopen repair — 4 October 2026

Published5eab038 run37194446146 passes frontend/Clippy and macOS/Windows Rust;
Linux job111413272739 fails the unchanged demand cold-reopen test at1228 with
Busy immediately after close/drop. The same earlier one-off R101 boundary is now
an actual repeated failure; it is not declared fixed by isolated local success.

A deterministic actual-Store regression keeps a duplicated Unix file descriptor:
reader close and a surviving Store clone remain Busy; after weak Inner proves all
owners gone, the old raw File still blocks immediate reopen. A raw OS control
confirms flock remains attached to the shared open file description across dup/
fork until explicitly unlocked or all duplicates close. The exact process that
may have inherited a descriptor in remote CI is not instrumented or claimed.

A private WriterLease now explicitly unlocks only on final Inner destruction,
with its field after writer/readers so their handles drop first. The stable lock
inode, clone/close exclusivity, crash OS release and bootstrap failure/cancellation
ownership stay intact. No early close unlock, sleeps, retry masking, schema/public
API or original demand-test assertion changes occur. New tests also prove closing
the old duplicate cannot release the reopened owner's independent lock.

Focused raw/Store controls2/2, unchanged demand restart1/1 and existing ownership
case1/1 pass. Full collaboration331/two ignored, workspace all-target
Clippy-Dwarnings and formatting pass. Pre-fix red and final green logs:
/tmp/gitru-ruru111-writer-lease-{baseline,red,green,demand,ownership,full,clippy,
 fmt-check}.log. A new signed #159 head must qualify its own remote platforms;
old failing CI is retained as history. The same private fix is integrated into
R103 before its next retained native build. R106 coordinated shutdown/current-
schema backup qualification remains open. No PR is merged.


### RURU-112 bounded Bitbucket Cloud account slice started — 4 October 2026

Live Linear/PR/worktree/dependency audit selected R112 after R111/#159 repaired
`ebb8ae1` passes its exact Linux Rust job; its Windows packaged check remains
separate. Reviewed R76/R100 prerequisites are unmerged. An isolated attached
worktree on that head now owns the first repository-only account slice, with
native adapter/runtime/test and SDK/UI work in parallel under disjoint paths.
Signed pre-code contracts ee0edda/e9bd990/f1d1a14 live in
[the R112 work note](./collaboration-work/RURU-112.md). R112 remains In Progress:
PR summaries/Body and explicit native participant/task facets require later chunks.

Current official Bitbucket sources confirm token-only Bearer API tokens and the
removed native issue/app-password endpoints. The proposed fixed public connection
uses existing native vault/actor/epoch cutover, no Gitru cloud or ambient credential
lookup; only repository capability is supported in this slice. Workspace-to-member-
repository opaque continuation state is account/epoch bound and bounded. Shared
per-job page limits merely yield, so this private cursor persists continuation
fingerprints and a20 accepted-page/20workspace limit across resumption/reopen.
Exhaustion remains Partial without inferred absence; capped large accounts need a
later explicit restart/coverage policy. Response-detected loops reject the page;
bounded retries may refetch its current URL but never follow the repeated target.
Username-only clone metadata is validated then discarded, not executed/persisted.

At the frozen additive main-only command signature, normal make typegen passes114
commands. Independent AST comparison preserves all285 prior schemas and adds only
Bitbucket connect params. Adapter/UI implementation and qualification are underway;
this record does not claim live provider, platform, completed R112 or publication.
R103's repaired source is separately published in #157 atcce498b with fresh macOS
retained proof and exact-head remote CI running; R121/#158 and R111/#159 stay review
stacks. No merge is authorized or performed.

### RURU-112 first account/repository slice qualified — 4 October 2026

Signed source4dad563 implements manual token-only Bitbucket Cloud accounts and
UUID member-repository discovery in the existing native vault/SQLite/scheduler,
generated SDK command and ordinary account dialog/repository picker. Repositories
alone are supported; PR/Body and explicit participants/tasks remain required and
R112 stays In Progress. Durable20 accepted-page/20workspace/fingerprint caps retain
Partial coverage/historical authorized rows across cold/manual resume; a capped
large account still needs a later explicit restart/coverage policy.

Actual HTTP17 plus Runtime/SQLite7 focused cases pass24; full default workspace706
top-level tests/three ignored passes, including355 collaboration and native caller
policy. Clippy/fmt and fresh24 after the equivalent style cleanup pass. Frontend431
tests/50files, lint/types and production build pass. Normal typegen114 preserves
all285 prior schemas,237 aliases,113 commands/events and adds only token connect.
Known-actor invalid-presentation quota regression is recorded red/green without
credential staging/cache replacement. Full evidence and platform/live limitations
are in [R112's work note](./collaboration-work/RURU-112.md). Publication and exact-
head remote qualification remain separate. No live credentials or merge.


### RURU-112 second bounded slice contract — 4 October 2026

First account/repository draft #160 exact968cd524 passes all11 reported checks,
including all-platform Rust/ordinary packaged E2E. R112 stays In Progress: PR and
explicit participant/task facets remain. The next isolated stack accepts the
[PR-summary/Body contract](./collaboration-work/RURU-112-pulls.md) before code.
Compound repoUUID/local PR identity, exact all-state opaque pagination, durable20
page/history bound, singleton rendered.description.raw authority, full-OID refs,
and typed unsupported differences use the existing Runtime/SQLite/SDK. The stable
UUID repository child route is explicitly a documentation inference until live
qualification; no provider credentials are read. No migration/scheduler/SDK
production change is planned. R103/#157 latest9ef7148 repair is separately pushed
with local all5 retained stages passing; its new remote matrix runs independently.
No merge. First #160 remains unchanged while the new reviewable slice is prepared.


R112 PR-read integration first49-case native run:45 passed,4 failed. Two actual
HTTP singleton failures expose lost account cooldown during detail error conversion;
[the signed work contract](./collaboration-work/RURU-112-pulls.md) now permits a
bounded existing-barrier correction in runtime/details.rs. Original red quota
assertions stay intact; old-epoch data/quota fencing remains required. Two fixture
assumptions were corrected to actual cold-admission/historical-coverage semantics.
Fresh post-fix qualification is pending. #1579ef7148 separately passes13/14 remote
checks; retainedWindows now starts and passes5/6 main scenarios but fails actual
retention-reset stale_view. Owned artifacts are under investigation; not qualified.


### RURU-112 PR reads qualified locally — 4 October 2026

Signed source6892db97 implements Bitbucket all-state PR summaries and singleton
raw Body/common metadata through existing local projections. The independent
detail-error cooldown regression is repaired through captured account/epoch
barriers, with both original red assertions green.49/49 focused native cases and
731 full default workspace tests/three ignored pass; all-target Clippy/fmt pass.
Frontend438/51files, lint/types and production build pass. Full evidence and
coverage/routing/platform limits are in the [PR-read work note](./collaboration-work/RURU-112-pulls.md).
A separate draft stack on #160 is ready for publication; remote exact-head CI is
pending separately. R112 remains In Progress for explicit participant/task facets.
#160968cd524 and #159ebb8ae1 each pass all11 reported checks. #1579ef7148 remains
13/14 with retained Windows failing a bounded test-lifetime seam; its isolated
fixture repair and fresh native qualification are underway. No merge.


### RURU-112 participant-only contract accepted — 4 October 2026

The [participant contract](./collaboration-work/RURU-112-participants.md) is accepted
before source changes in an isolated managed stack on #1611766a6e. Common typed
ParticipantV1 preserves native role/approval/state/action-date facts, independent
field masks/clocks and immutable identity; no review/readiness overloading. A
forward0008 rebuild preserves all four dependent detail tables with FKON and
frozen-v7 upgrade/rollback/cold evidence. The panel acquires ordinary local query
and native interest only when opened and supported. Tasks are a later bounded
chunk; R112 stays In Progress. R1034054a6c is separately published after fresh
all5 retained Mac stages pass; both new exact-head remote matrices remain pending.
No source/IPC/migration qualification or live credentials/merge is claimed yet.
