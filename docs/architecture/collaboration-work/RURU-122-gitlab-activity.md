# RURU-122 — bounded GitLab activity history

Design checkpoint, 8 October 2026. Isolated branch starts from `0a4c149`, the
published integration/scheduler/startup prerequisite branch. Live review found no
existing GitLab activity PR or overlapping writer. GitHub activity #184, Bitbucket
comments #188 and GitLab notes #189 each pass all 14 reported checks; their source
is integrated in this parent. They remain unmerged.

## Provider facts and product contract

The current primary [Notes API](https://docs.gitlab.com/api/notes/#resource-events)
returns system notes but excludes separately tracked resource events. GitLab's
[state events](https://docs.gitlab.com/api/resource_state_events/) and
[label events](https://docs.gitlab.com/api/resource_label_events/) provide numeric
project + issue/MR IID routes and native resource IDs in each event. Other event
families (milestone, weight, iteration and future types) remain outside this
bounded slice. No complete audit-history claim or current-workflow authority may
be derived from these rows.

Enable the existing Activity facet for GitLab.com issues and merge requests.
Read system notes, state events and label events into the existing typed
`ActivityEvent`/`DetailEntry` projection. Render ordinary provider text as safe
text; never execute markup, follow provider-supplied API URLs, infer capabilities
from historical events or change current labels/state/merge/check evidence.
Conversation comments and anchored discussions keep their independent facets.
The shared UI must explicitly describe the available GitLab history sources and
partial coverage; unsupported kinds remain visible as unsupported observations.
No new schema or IPC data shape is expected.

## Native read and cursor contract

- Use immutable numeric project identity plus validated issue/MR IID and native
  subject identity. Pin provider/host, active account, immutable actor, epoch,
  repository, subject kind/ID and route in the durable cursor. Reject foreign
  row resource identity atomically; missing/malformed rows never become absence
  authority.
- Three independently paginated collections share one bounded Activity cursor.
  Rotate among unfinished collections after each 50-row page so a large notes
  history cannot starve state/label history. Retain all three continuation states
  across the runtime's existing ten-page yield and cold restart. At most 20 pages
  total per traversal; reject repeated pages, malformed cursors and excess work.
  A terminal page from one family does not terminate the other families.
- Existing GitLab transport limits, finite same-operation redirects, credential
  redaction, exact origin/route/query/page validation, response byte/header limits
  and captured-epoch rate observations remain mandatory. The new resource-event
  routes receive the same validation. HTTP 304 cannot validate the composite
  history; no parent validator is sent or accepted. One page means one HTTP read;
  a depleted response returns its cooldown before another collection is read.
- System-note membership requires `system: true`; never duplicate ordinary
  comments/discussion replies into Activity. Use a family-qualified stable entry
  key and provider identity so equal numeric IDs in notes/state/labels cannot
  collide. Per-row timestamps, author and fields retain ordinary independent
  source validation. Missing/oversized text remains typed missingness.
- The source is always uncertain history. Even if all three first pages are
  empty or terminal, omitted event families and non-snapshot pagination prohibit
  complete-history/absence authority or destructive pruning. Rejected or missing
  rows preserve saved observations; a late result cannot cross account/view/run
  fences or restore denied provider content.

## Verification before publication

Exercise actual loopback HTTP/SQLite with no personal credentials: issue and MR
routes; three sources with colliding IDs; normal comments excluded; unknown event
kinds; malformed/cross-subject rows; author/text missingness; strict next links,
redirects and 304 rejection; invalid cursors; global page cap; source round-robin
and durable continuation through yield/cold reopen; captured cooldown on both
successful malformed responses and rejected publication; same-epoch denial and
old-epoch responses; offline cached reads and unchanged authored drafts.

Use the existing Activity UI, local query/subscription and bounded pagination.
Add focused UI evidence for provider coverage wording and safe system-note text
if those components change. Run native focused/full checks, strict Clippy/fmt,
frontend tests/types/lint and generated IPC verification when touching signatures.
Keep local/remote CI/native-window/live-provider evidence separate. Document exact
results; never merge a PR or claim production account validation from fixtures.
