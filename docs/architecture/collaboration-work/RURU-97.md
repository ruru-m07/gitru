# RURU-97 — Independent detail facets and explicit hydration

This slice builds on reviewed RURU-76. It creates the storage/scheduler seam
consumed by RURU-77/78; production GitHub detail endpoints and detail UI remain
those issues. Summary observations are never promoted into authoritative detail.

## Concrete contract

- Details are account + canonical subject + facet scoped. Initial facets are
  body, comments, reviews, and checks. Each has independent run generation,
  coverage, validation/freshness, source observation, facet revision, authorization
  epoch, and synchronization/error metadata. Timeline and writes remain unsupported.
- Local queries use one SQLite read snapshot and never enqueue work or call a
  provider. An immutable metadata-only evidence accessor also works within an
  existing Rust read transaction for RURU-100 contextual capabilities.
  `detail_evidence_in` returns `saved_empty` only for an authorized, complete
  saved facet. Body emptiness is computed by a SQLite JSON predicate and
  collection emptiness by `EXISTS`; no description text enters contextual reads.
- Body values explicitly distinguish not loaded, known (including authoritative
  null/empty), omitted, and oversized. Omitted fields preserve previously saved
  observations and cannot freshen them. Collection emptiness is authoritative
  only after a complete traversal within the declared facet scope.
- Provider observations carry endpoint/adapter version, observed field masks,
  validation time, and optional comparable facet timestamps. Partial entry
  observations preserve fields not observed by that endpoint. Parent summary
  timestamps do not order child facets. Validators are equality tokens.
  Latest endpoint `source` is separate from saved-value `value_source` authority.
  Omitted/oversized observations retain the saved body's comparable clock;
  whole-scope 304 validates freshness without erasing that ordering boundary.
- Forward migration 0004 adds rebuildable detail observations/entries. Existing
  summary projections, aliases, authored drafts, credentials and historical
  migrations are preserved. Grant cutover clears private detail cache alongside
  other provider content. Access loss suppresses detail reads even when old
  observations are retained. Temporary network/quota failures retain saved reads.
- Explicit account-bound hydration is capability gated and coalesced with the
  existing bounded scheduler and provider cooldowns. Jobs carry identifiers;
  they do not copy large cached bodies into the queue. Detail work rechecks
  account epoch, installation, parent scope and facet run generation at dispatch
  and commit. SQLite commits observations/change metadata before wake-up hints.
  New hydration requests carry the inspected authorization epoch. Native admission
  rejects an old account-bound handle before creating durable demand, and the
  client fences late receipts through the existing authorization lifecycle.
- A traversal commits bounded pages and checkpoints; restart can resume explicit
  pending read intent. Request cancellation cannot relabel an old response under
  a replacement grant. Outdated generations and known older comparable facet
  observations cannot overwrite newer data. No universal parent timestamp delta
  or provider-independent deletion inference is introduced.
  Deselecting a repository invalidates detail run generations and continuations;
  reselecting cannot revive an earlier lease. Cached observations and drafts remain.
- Bodies are bounded at 1 MiB, individual entry bodies at 64 KiB, write pages and
  local result pages at 100 records, collection cache at a fixed per-facet ceiling,
  and each scheduler turn at ten provider pages. Oversize/partial states remain
  visible; large content is not silently truncated or treated as empty.

The per-facet collection ceiling is 5,000 entries. Pending demand is rebuildable
read intent, not remote-write delivery; a full queue can retain that intent for a
later background admission. A partial ten-page traversal persists its checkpoint
and re-enters the existing due/cooldown policy. Unsupported, inaccessible and
bounded-cache failures stop automatic demand; transient errors retain it and use
the existing bounded retry policy. Successful response cooldowns persist at both
provider and facet levels while saved reads remain available.

## File ownership and sequence

1. New detail domain module and provider request/outcome seam; default adapters
   explicitly return unsupported and their current profiles remain truthful.
2. Migration 0004 and separate storage/detail module; integrate scope validation,
   parent access guards and account cache cutover in storage.rs.
3. Separate runtime/detail module using the existing queue/admission/cooldowns;
   adapt the queue work discriminant without adding a parallel scheduler.
4. Local detail and explicit hydrate commands, registrations, generated bindings,
   account-bound client/query options and scoped bridge invalidation. Sequence
   shared IPC integration with RURU-100 after this contract freezes.
5. Native fixture/runtime/storage/restart/denial/stale tests plus frozen-v1 schema
   upgrades, generated wire/client tests, formatting, type checks and Clippy.

RURU-100 owns contextual capability models/modules/UI, with no migration. Its
context snapshot consumes this slice's transactional detail evidence accessor;
it does not call hydration or provider HTTP during capability reads.

## Validation boundary

Test independently cached facets, initial missingness, authoritative emptiness,
omitted/oversized values, partial field preservation, keyset pagination, stale
run/epoch/provider observations, account and subject isolation, permission and
selection loss, temporary failures, coalesced hydration, committed-before-hint
ordering, and restart/resume. Production GitHub adapters remain unsupported for
detail until the separately reviewed RURU-77/78 implementations land. Record
local evidence here and distinguish it from remote CI or live provider checks.

## Local evidence

The collaboration suite includes independent public-API access tests authored by
the migration/recovery peer, plus storage and scheduler fixtures. They exercise
frozen v1 to current migration preservation, complete/partial traversals, real
cold reopen, zero-HTTP local queries, coalescing, committed-before-hint ordering,
old-grant zero-admission, delayed denial responses, deselect/reselect fencing,
quota persistence and retained-value ordering through omitted/oversized/304
responses. The independent review found the deselection generation and retained
ordering-source defects; both have production fixes and regression coverage.
Metadata emptiness uses exact string equality, with a NUL-leading nonempty body
regression, because SQLite text length does not represent that edge faithfully.

Final local checks: 114 collaboration tests, three native IPC caller-policy tests,
36 SDK/wire/cache tests, SDK and desktop type checks, SDK lint, Rust workspace
formatting, and all-target Clippy for collaboration and desktop. Generated bindings
come from normal `make typegen` (95 commands). Desktop UI is unchanged by this
slice; 177 desktop tests and desktop/E2E types pass after stacking onto the reviewed
RURU-76 host-lifecycle base. This is local evidence, not remote CI or a live detail
endpoint claim. Backup restore policy remains separately reviewed in RURU-106;
do not raise its accepted schema ceiling merely because migration 0004 exists.

CodeQL follow-up (3 October 2026): PR #147 head `5d98afce` has two new
`rust/cleartext-logging` alerts
([20](https://github.com/ruru-m07/gitru/security/code-scanning/20),
[21](https://github.com/ruru-m07/gitru/security/code-scanning/21)) at the partial
entry-field test's synthetic `entries.remove(0)` extractions. The successful Rust
analysis also carries the inherited RURU-95 account-extraction alert. The new
flows reach these vectors from the account read through `Store::detail`; no
account or saved body is actually logged by this test. CodeQL 2.27.1's
[generated model](https://github.com/github/codeql/blob/6e9f9e38390175c41b99070a423c875f450759ca/rust/ql/lib/ext/generated/modelgenerator/rust.model.yml#L8766)
treats `Vec::remove`'s receiver as a logging sink. Checked iteration with fixed
failure messages now enforces exactly one saved entry before and after the
partial observation, preserving body, author and validation-time assertions.
The rule remains enabled and production behavior is unchanged. Scoped local
verification passes all ten detail storage tests, all-target collaboration Clippy
with `-D warnings`, workspace formatting and diff whitespace checks. Exact-head
remote rescanning remains pending for this follow-up, including the inherited
RURU-95 alert after its repair is propagated.
