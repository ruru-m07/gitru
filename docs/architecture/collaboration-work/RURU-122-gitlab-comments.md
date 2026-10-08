# RURU-122 — GitLab.com cached conversation comments

Status: native adapter qualified and ready for full workspace validation, 8 October 2026.

Expand the existing common Comments facet to GitLab.com issue and merge request reads. The base is
qualified GitHub Activity PR #184 at 5a311296. Reuse the existing local paginated
Comments UI, storage, own-row freshness clocks and demand/revision machinery.
No new schema or public DTO is planned. Preserve provider-specific Discussions,
Reviews, Activity and author-private drafts. Only enable capabilities actually
implemented by this adapter; unsupported resource/provider combinations stay
explicit. No mutations or personal credentials are needed for qualification.

Official endpoint reference checked 8 October 2026:
https://docs.gitlab.com/api/notes/
Use the provider's existing immutable native repository/project addressing,
validated resource number/native identity and strict transport/pagination helpers.
Do not follow hostile next URLs, redirects, altered selectors, page skips, cycles,
or unbounded provider responses. Request bounded pages and reuse the durable
20-page continuation policy rather than fetching all comments at once. Preserve
quota/auth observations before local publication fences and never invent 304
support for these collection semantics.

Map stable provider comment IDs to existing deterministic local entry identity.
Keep authored body/author/created/updated facts bounded, with each comment's own
freshness clock; a parent issue/PR timestamp cannot prove child-content freshness.
Do not persist raw JSON, email addresses or arbitrary provider links. Timestamps
that are missing/invalid do not become fabricated authority. Edits and explicit
deleted/tombstone representations must not silently resurrect stale text. Treat
missing fields, skipped/unrepresentable rows and incomplete/capped/multipage
history as partial; absence removal only has authority for a fully represented
complete singleton response, matching the existing Comments contract.

Make the rendered scope honest when an endpoint mixes ordinary comments, inline
review notes, system events or deleted entries. Preserve supported generic text
without claiming thread/anchor fidelity. If a row must be skipped because its
meaning cannot be represented, retain uncertain coverage and never delete unseen
rows from that traversal. Do not recast system activity as authored discussion.

Native qualification must exercise finite synthetic HTTP, immutable route/identity,
strict pagination, per-row clock edits/regressions/deletions, partial-vs-empty,
provider errors/quota, restart/continuation and authorization loss/held-response
fences. Extend actual common UI capability tests for this provider without
forking the UI. Run make typegen; if public inventory is unchanged, avoid committing
unrelated generated ordering churn. Use signed scoped commits and full local make
verify, with remote CI and live provider/vault/platform boundaries stated separately.
No merge. Root owns publication and progress docs; provider owner owns native
implementation/tests plus this work note. Review before qualification.

## Implemented adapter semantics

The public GitLab.com provider now advertises Comments and serves both native
issue and merge-request note collections through the existing facet. Requests use
`projects/{numeric_project}/{issues|merge_requests}/{iid}/notes` with fixed50-row
pages ordered by updated_at ascending. Cursor bytes bind account, actor, epoch,
project, subject native/local identity, kind and IID. Transport permits only the
exact next page and collection; selectors, ordering, hosts and routes cannot
change. A20-page traversal retains an inert next21 cursor, truthfully recording
that more remote data exists without allowing another request through that cursor.

Only ordinary, non-system unanchored notes map to the common Comments view.
System activity, diff/discussion types, resolvable/position-bearing notes,
deleted representations and malformed/unrepresentable rows keep coverage partial.
Conflicting explicit parent identities and duplicate represented note IDs reject
the page. Missing/oversized body observations preserve their explicit state and
cannot authorize absence removal. A fully represented singleton, including a
valid empty array, is the only full-history authority; multipage history remains
uncertain. This slice makes no deletion inference from a skipped system or unknown
row, and does not claim thread/anchor fidelity.

Stable IDs do not depend on dates. Each saved note uses its own normalized update
clock; valid created_at is checked for ordering but is not fabricated as a separate
common DTO field. Parent timestamps/ETags are never used to validate comments.
Only bounded body and author login are stored; provider email/raw JSON/links are
not persisted. The common SQLite pipeline retains newer text against an older
row, preserves prior text with explicit omitted evidence, and fences late results
by account authorization and native subject context.

A finite HTTP regression exposed inherited GitLab Retry-After handling: a response
without primary quota exhaustion previously discarded its account cooldown. The
transport now preserves the larger of explicit Retry-After and primary cooldown,
including successful responses and malformed/error observations. Existing GitLab
provider controls remain part of the qualification run.

Native evidence:76 GitLab provider tests passed, including9 new controls with
finite HTTP, hostile next links,20-page cursor bound, unrepresentable row matrix,
identity/duplicate/body bounds, status/quota preservation, HTTP-to-SQLite own-clock
edits/regressions/partial-vs-empty history, cold continuation and late epoch refusal.
Strict collaboration all-target Clippy, workspace Rust formatting and diff checks
passed. Full workspace verification, remote CI and real GitLab/vault/platform compatibility
remain separate publication gates. No live provider or personal credential was used.

Final compatibility review preserves opaque local projection IDs, as required by
the shared identity store and existing GitHub Comments contract. A new actual
HTTP-to-SQLite control proves that opaque issue/repository IDs work while altered
native subject/project bindings cannot publish. After integrating Activity
`bb0c73c5`, all10 Comments controls pass. No provider-specific local-ID string codec
was introduced; immutable provider identity and writer-time binding are authority.

The first full workspace run found one stale inherited assertion: the cold GitLab
resource-read fixture still expected Comments to be unsupported. Its failure was
`Supported` versus `Unsupported` at line566, not a timeout or a runtime regression.
The fixture now proves cold Comments reads/sync are supported but remain NotLoaded
and read-only, while Merge remains unsupported. Its existing Body/draft/reopen
and unchanged provider/vault counters remain intact. All four actual GitLab
HTTP-to-SQLite resource-read controls pass after this test-only correction. The
initial full native library result was629 passed/1 failed/4 ignored; it is not
reported as a passing full run. Final full workspace qualification is separate.
Strict collaboration all-target Clippy, Rust formatting and diff checks also
pass for this narrow correction.


## Full workspace qualification

Signed product `ea97e2a3f6686a0a058c89e2a6e37a474bde2030` passes complete local
`make verify`: 802 frontend passes/one platform skip and 1,327 Rust test executions
with seven helper ignores, lint, TypeScript checks, desktop build, formatting and
strict workspace Clippy. The earlier full-run failure and narrow capability fixture
repair are retained above; no assertion was weakened to bypass unsupported data.

A separate anonymous public compatibility probe received200 for project278964 and
its MR list, then401 for the selected MR Notes endpoint. It therefore does not
qualify live note payloads or authenticated permissions. No token was inspected,
no provider mutation was performed and no personal comment content was retained.

The platform test budget also consumes R130's diagnosed45→60-minute CI repair:
Windows job113090183418 passed both Rust steps, then cache upload hit the old
job limit. Tests/assertions/retries stay unchanged; current-head remote CI is
separate from the local results.
