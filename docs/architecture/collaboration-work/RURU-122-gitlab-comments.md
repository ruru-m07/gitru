# RURU-122 — GitLab.com cached conversation comments

Status: bounded pre-code contract, 8 October 2026.

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
