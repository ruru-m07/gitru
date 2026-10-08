# RURU-122 — Bounded GitHub activity timeline

Status: implementation contract, 8 October 2026, before source changes.

## Scope

Add an independent local Activity facet for GitHub.com issue and PR timeline
observations. Existing Comments, Body, Reviews and authored drafts retain their
own coverage and lifecycle. The review stack starts on RURU-128 `5915f690`, which
already contains the read foundations; no user PR is merged. Provider expansion,
webhooks, writes and timeline inference of present permissions/state are outside
this slice. The native lane owns models, provider reads, SQLite/recovery and tests;
the frontend lane owns generated IPC, SDK and the opened-only safe-text panel.

GitHub documents the timeline endpoint for both issues and PRs with page/per_page
pagination and optional event exclusion. Reference checked 8 October 2026:
https://docs.github.com/en/rest/issues/timeline?apiVersion=2026-03-10
The existing immutable repository-ID read route pattern is preferred, without
redirects or a mutable-name fallback. Public anonymous endpoint probes, finite
fixtures and private authenticated compatibility are distinct evidence.

## Authority and bounds

Use a distinct typed facet/capability and bounded native event representation;
do not store unbounded raw JSON or reuse Comments authority. Retain event kind,
provider identity, known actor/time and safe bounded presentation data. Supported
heterogeneous events need stable source-specific identities and deterministic
ordering; commit events cannot be forced into a numeric event-ID assumption.
Unknown event kinds are tolerated and rendered as unsupported activity when
stable identity is available. Missing identity or skipped/unrepresentable rows
must downgrade coverage, never produce authoritative complete-empty history.
Provider event text is inert text; provider-derived links require existing URL
validation. Historical activity is not present-day workflow or write authority.

Use 50-row provider pages, the existing 20-page durable traversal budget and a
bounded local keyset pager. Strict cursor/source/account/actor/epoch/subject/native
repository fences, same-origin next links and duplicate/loop protection apply.
A complete initial singleton page can replace its facet only when all observed
rows are represented. Multipage/partial/capped history remains uncertain. Do not
infer deleted events from incomplete page absence. Successful-response quota and
auth observations must survive discarded publication after head/access drift.

Cache reads are offline and local. Opening only this disclosure mounts its query
and visible demand; closing releases demand and inactive pages. Independent
pagination must not disturb dirty comment or Private note text. Account/view
changes cannot keep prior account content visible. Other providers show explicit
unsupported capability rather than a fabricated empty timeline.

## Qualification

Before publication run finite adapter tests for heterogeneous and unknown events,
stable ordering, edits/deletions under complete versus incomplete coverage,
malformed/hostile links, bounds, quota and permission failures. Native SQLite
controls prove independent facet coverage, cold restart/local reads without
provider or vault work, held-response revocation and pagination fences. Cover
migration/recovery if persisted accepted enum values change. Generate IPC via
make typegen; no handwritten generated edits. UI checks cover collapsed zero-work,
page transitions, safe text, partial/empty distinction, access loss and retained
authored editors. Record focused checks and full make verify separately from
remote CI, live provider/vault checks and native-window qualification.

## Frozen integration and DTO decision

The feature now builds on integration-only parent `dd6f23f3`, combining the read
stack `5915f690` with durable comment stack `130b9f75` (schema 0020). It introduces
migration 0021 and freezes schema-0020 SQL for recovery; it does not create an
alternate migration 0019. No existing PR has been merged.

Use `DetailFacet::Activity`, `ResourceFacet::Activity`, `DetailField::Activity`
and `NativeDetailPayload::ActivityV1(ActivityEvent)`. The event has bounded
provider `kind`, `supported`, optional canonical `occurred_at` and optional inert
`description`. Existing entry author/body hold the actor and bounded text. No new
IPC function is needed. Provider response objects and arbitrary URLs are not
stored or exposed as payloads.

Stable numeric identities are scoped by event kind; committed activity uses its
canonical commit SHA. Unsupported kinds may retain a bounded stable node ID.
Local keys sort known immutable event-created timestamps, then kind/native ID;
unknown times follow in a deterministic group. Comment updated-at stays an edit
clock and does not change identity. Commit author/committer timestamps are not
misrepresented as the time the commit was added to the PR. Missing identities
and unrepresentable rows downgrade completeness, including an otherwise empty
singleton page. Official heterogeneous event reference checked during design:
https://docs.github.com/en/rest/using-the-rest-api/issue-event-types
