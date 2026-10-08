# RURU-122 — Bitbucket Cloud cached conversation comments

Status: bounded pre-code contract, 8 October 2026.

Expand the existing common Comments facet to Bitbucket Cloud pull request reads. The base is
qualified GitHub Activity PR #184 at 5a311296. Reuse the existing local paginated
Comments UI, storage, own-row freshness clocks and demand/revision machinery.
No new schema or public DTO is planned. Preserve provider-specific Discussions,
Reviews, Activity and author-private drafts. Only enable capabilities actually
implemented by this adapter; unsupported resource/provider combinations stay
explicit. No mutations or personal credentials are needed for qualification.

Official endpoint reference checked 8 October 2026:
https://developer.atlassian.com/cloud/bitbucket/rest/api-group-pullrequests/#api-repositories-workspace-repo-slug-pullrequests-pull-request-id-comments-get
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

## Implemented native slice — 8 October 2026

The Bitbucket Cloud adapter now reads the common Comments facet for selected,
canonically identified pull requests via the immutable repository UUID route.
Pages retain at most 50 rows; the cursor binds account, immutable actor,
authorization epoch, repository and pull identity, and the completed page count.
Each next URL preserves the fixed route and page size, contains exactly one
continuation selector, advances numeric pages by one, and cannot repeat a saved
normalized fingerprint. Opaque forward selectors remain bounded. The existing
20-page ceiling survives cursor serialization. No ETag/304 support is claimed.

This slice represents published top-level conversation text only. Inline notes,
replies, pending/system representations and unknown comment kinds are skipped
with uncertain coverage. Their anchors, thread relationships and resolution are
not flattened into ordinary discussion. Missing author/body fields, oversized
text, contradictory total counts and every multipage traversal remain partial.
Only a fully represented first-page terminal enumeration can reconcile absence;
a truly empty complete response can remove prior saved rows. The published
provider schema also explicitly documents `parent`, `inline`, `deleted`,
`created_on`, `updated_on` and `pending`:
https://dac-static.atlassian.com/cloud/bitbucket/swagger.v3.json?_v=2.300.196

Every retained row requires its own valid `updated_on`; the parent PR timestamp
never orders comment edits. `created_on`, when supplied, must be valid and no
later than the update clock. Explicit deleted rows carry validated
`state="deleted"`, Known/null body and a cleared author. Older observations cannot
resurrect deleted text. Provider markup, raw response JSON, emails and arbitrary
links are discarded. There is no provider mutation or comment-send capability.

Account-level Comments support is qualified by resource kind: a provider that
explicitly marks Issues unsupported cannot advertise issue Comments through the
contextual capability API. Missing or temporarily unavailable primary-facet
support does not become a semantic unsupported claim. The adapter itself also
rejects issue requests before HTTP.

Native qualification uses finite synthetic HTTP, synthetic in-memory credentials
and temporary SQLite databases. All 114 Bitbucket provider/runtime tests passed,
including nine new adapter controls and three real HTTP-to-SQLite controls for
own-clock edits, tombstone regression, skipped-row retention, full-empty absence,
closed-store cached reads, exact serialized continuation resume, scoped denial
that preserves authored drafts, and held old-epoch success/quota responses that
cannot mutate a replacement account. No live Bitbucket token/account, OS vault,
OS suspend or remote CI result is inferred from these local tests. Common UI,
contextual capability qualification and final integrated verification are recorded
by their owners after completion.

The provider-neutral contextual rule passed all 14 native contextual-capability
controls, including a real cached Bitbucket PR/issue comparison and missing versus
unavailable primary declarations. The final comment runtime delta passed 3/3;
strict collaboration Clippy over all targets, Rust formatting and diff whitespace
checks passed. These are local results; final stack integration, generated IPC
verification, full workspace verification and remote CI remain root-owned gates.


## Integrated workspace qualification

Signed product source `d114202c50d381bddb2bf1701213de4dc8292bc4` passes complete
local `make verify`: 804 frontend passes/one platform skip and 1,330 Rust test
executions/seven helper ignores, lint, TypeScript checks, desktop build, formatting
and strict workspace Clippy. The reviewed native adapter and common UI are
stacked on Activity PR184. There is no schema/public IPC change. No authenticated
provider, production vault or packaged GUI qualification is claimed.

The platform-test job budget also consumes the diagnosed R130 CI repair:45 to60
minutes. Its Windows job113090183418 passed workspace and retained tests before
cache upload hit the old job boundary. No test/assertion/retry behavior changes;
new-head remote CI is separate.
