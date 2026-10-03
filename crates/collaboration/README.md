# Collaboration engine

Initial read-only GitHub.com implementation of
[the collaboration architecture](../../docs/architecture/remote-collaboration-engine.md).

`Store` owns the durable SQLite projection, migrations, drafts, and change log.
`CollaborationRuntime` owns account credentials and one coalescing background
queue. `CollaborationProvider` translates feed requests; `GithubProvider` is the
first adapter. UI queries read only SQLite, including when browser networking is
offline. A refresh receipt acknowledges scheduling, not provider completion.

The app initializes this runtime independently of Git repository contexts and
tabs. Its native commands validate local webview callers; only the main webview
can manage credentials/accounts. Local child tabs can select repositories and
read domain data. Every Accounts button opens the single main-host dialog in
the same native window; native tabs are suspended until that dialog closes.
Tokens live in the OS
credential store. E2E builds use an isolated in-memory test vault and disable
personal GitHub CLI discovery.

## Current scope

- Multiple GitHub.com accounts verified through `/user` using personal tokens
  or explicit import of an existing GitHub CLI credential.
- Repository discovery and explicit persistent sync selection.
- All-state PR/issue summaries, cached bodies up to 64 KiB, native notification
  summaries for supported classic or imported OAuth credentials. No provider writes.
- Bounded local search and 1–100-row pages, cached detail, durable private drafts.
- WAL/FULL transactions, one writer lease, three read connections, FTS5,
  authorization/run fences, durable revisions and catch-up.
- Partial page continuation, single-page conditional requests, rate cooldowns,
  offline/retry states, conservative membership reconciliation.

GitHub notifications require a classic `ghp_` or existing OAuth `gho_` credential
and observed `notifications` or `repo` scope. Fine-grained tokens can connect for
repository features; the inbox renders an unsupported capability state.
Permissions come from the credential and provider response, not the connection
method. See [GitHub's endpoint documentation](https://docs.github.com/en/rest)
and [OAuth scopes](https://docs.github.com/en/apps/oauth-apps/building-oauth-apps/scopes-for-oauth-apps).

The main account dialog discovers GitHub CLI account metadata in the background
and offers explicit selection alongside manual PAT entry. Discovery never
requests a token. Import accepts an opaque five-minute candidate ID, rechecks
CLI account status and verifies the returned credential's `/user` identity before
saving anything. Gitru never runs CLI login, switches accounts, or alters CLI
configuration; disconnect removes only Gitru's credential copy. CLI reuse can
import an existing OAuth credential without starting an OAuth flow in Gitru.

The native subprocess uses recognized installation paths resolved on every
invocation, a neutral working directory, bounded output and deadlines. It drops
token overrides and debug variables, disables prompts, and exposes no token to
the frontend. Discovery requires `gh auth status --json` support (GitHub CLI
2.81 or later). Missing/unsupported/custom installations fall back to manual PAT.

The scheduler admits at most 128 scopes, uses one HTTP worker and yields after
10 pages. Background discovery runs every 10 minutes; selected PR/issue feeds
every 3 minutes; inbox polling starts at 60 seconds and respects provider hints.
Rate cooldowns survive process restart and same-actor reauthorization. Remote
requests have a 4 MiB response limit and 5-second connect/20-second total timeout.

Repository renames can continue through the exact immutable native-ID API path.
Requests, redirects and next links are restricted to that path and the selected
repository's current named endpoint.

`Complete` means a traversal finished, not that the provider supplied a globally
consistent snapshot. Unseen membership is hidden after two successful complete
enumerations; partial/failed/304 responses do not count. Pending absence disables
validators until that second enumeration occurs. Cached entity rows and
drafts are retained. Missing data, stale observations, and provider access loss
are different states. Bodies deliberately omitted from cache carry
`body_omitted`, while an authoritative null description clears the cached body.

Pagination binds the query, authorization view and relevant selected feed data
revisions. Unrelated accounts, drafts and status updates do not invalidate the
cursor. Newly denied scopes advance the authorization view and reset private UI
projections; successful authorized observations recover them.

Forward SQLx migrations reject unknown/newer schema versions without resetting
data. SQLite 3.51.3 is bundled and checked at startup along with FTS5 support.
The encrypted-at-rest database, retention policy, backup/restore workflow, and
crash fault injection remain later acceptance gates.

## Development

```sh
make typegen
cargo test -p collaboration
cargo run -p collaboration --example read_benchmark --release
bun run test --project collaboration-client
```

CLI tests use injected runners and isolated temporary executables; they never
read the developer's real `gh` account or credential store. Packaged E2E asserts
that personal CLI discovery is disabled.

After changing Rust command/DTO signatures, always regenerate bindings.
`scripts/collaboration-bindings.ts` applies source-derived Serde/nullability and
injected-Webview corrections that the current type generator misses. Wire tests
verify these contracts. Never hand-edit `packages/commands`.

The benchmark creates synthetic data in a temporary database and measures
indexed/FTS reads. It does not measure UI/IPC latency or app memory usage.

On the local Apple Silicon release build, 10,000 cached rows and a 50-row page
over 200 reads measured p95 243 µs for an indexed list and 4,329 µs for FTS. This
is a storage measurement, not an end-to-end product latency claim. Build-time
debug information is retained to avoid [Rust's macOS macro-library alignment
bug](https://github.com/rust-lang/rust/issues/157750) with the pinned toolchain.

## Next slices

Finish live packaged authentication and platform verification, richer detail
facets/hydration, local repo links, foreground demand, resource budgets and
retention. Then implement the durable outbox/optimistic effective views and
operation-specific ambiguity/conflict handling before remote mutations. Validate
the abstraction with GitLab, then Bitbucket Cloud and tested enterprise adapters.
No webhooks, relay, OAuth broker, or Gitru cloud account is required today.
