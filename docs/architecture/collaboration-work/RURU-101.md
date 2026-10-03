# RURU-101 — Independent facet reconciliation qualification

Status: root-approved bounded implementation contract, saved before source edits.
Selected after live Linear/blocker/PR/file-overlap audit on 3 October 2026. R97
is an implemented unmerged reviewed ancestor. Attached isolated worktree
`/Users/ruru/.codex/worktrees/collab-ruru-101/gitru`, branch
`ruru/ruru-101-facet-reconciliation`, starts at signed R79/#154
`1a51a75a0f122c1e41288a9b6b6ca09421a38025`. Parent CI currently has an actual
macOS synthetic socket failure; root owns its repair before next publication.
Read the master architecture/backlog and this contract before editing.

## Scope and invariants

Qualify independently refreshed Body, Comments, Reviews and Checks through the
existing provider-neutral native detail contract and fake provider observations.
Record each facet's validator, ordering/overlap, completeness/absence and head
rules with exact source evidence. Do not rewrite common feed paging, the existing
two-complete-traversal feed absence policy, or ordinary selected-cache semantics.
GitHub production Comments/Reviews/Checks stay Unsupported until their separate
provider rollout tasks; this work must not claim live endpoint qualification.

Timestamp strings and opaque validators are evidence, never a universal event
log. An incomplete traversal, transport error, omission, different representation,
old authorization/view/run/cursor/head or pagination drift cannot erase saved
content or authored drafts. Qualified full collection replacement may reconcile
actual deletion; incremental/delta completion needs distinct absence evidence.
Audit existing DetailCommit.complete/whole_scope and stored run semantics before
assuming they already encode that distinction. Document any discovered gap and
reproduce it with an independent observable test before proposing a source fix.

Body null/empty may be authoritative only under its explicit observed field mask;
omitted/oversized observations preserve the previous saved value and value clock.
304 may validate only an already known complete matching source/representation
and validator; it cannot bootstrap data, turn missing fields into known emptiness,
or validate a changed head. Opaque validators do not admit lexical ordering.
Overlapping page entries merge observed fields conservatively without moving a
saved field's clock backwards. Partial traversal retains unseen children. Parent
edit/close/merge/rename/denial must retain local CAS drafts while provider reads
respect the current authorization and independently scoped facet evidence.

## Ownership and initial qualification

Independent native owner first audits existing details/detail_access tests,
storage/details.rs, resource metadata and head semantics, then writes a concrete
policy table and independent red/green evidence in a proposal to root. Start new
`crates/collaboration/tests/facet_reconciliation.rs` or a similarly scoped test
module, with narrow test fixture support. Root approves any justified production
change before broad shared storage edits; source fixes must preserve existing
assertions and ordinary query/paging contracts. No Tauri/SDK/UI/generated or
migration edits are expected. Migrations0001..0007 are frozen; R106 restore's
v1/v2 acceptance ceiling remains unchanged.

Meaningful qualification includes whole-source known 304 vs unsolicited/wrong
validator/omitted data; overlapping page edits and old field clocks; cursor/run
restart and capped/drifting traversal; obsolete authorization/head; collection
absence and saved draft generation; edit/closed/merged/renamed parent and explicit
access denial. Test externally observable invariants rather than mirroring code.
Keep local native evidence distinct from remote Linux/macOS/Windows CI and live
provider/vault qualification. Use synthetic credentials only if necessary.

Root owns architecture/progress docs, signed scoped commits, PR/Linear/artifacts
and final integration. R110 owns a separate managed worktree and will not share
edits. Do not commit/push or inspect personal credentials/config/vault. All native
checks serialize through `CARGO_TARGET_DIR=/Users/ruru/.codex/worktrees/remote-collaboration/gitru/target
python3 /tmp/gitru-cargo-serial.py COMMAND ...`; no direct Cargo concurrently.
Fresh Bun dependencies use frozen lock/copyfile. Open an attached reviewable PR
only after actual parent failures are repaired; no merge is authorized.

Implementation, policy table and exact validation evidence follow this contract.


## Approved source-backed correction — before production edits

Root accepts the bounded proposal below after the initial independent six cases
produce four actual reds and two green controls. Native owner may change only
detail native receipt/lease/provider seams, storage/details.rs and narrow source
helpers, resource_metadata head invalidation, runtime detail integration and the
required synthetic struct literals/tests. Preserve all inherited assertions.
Public renderer DTOs and historical Reviews semantics remain unchanged.

Persisted native JSON metadata must be explicitly versioned, bounded and decode
legacy rows conservatively; unknown metadata versions cannot become validator/
absence/head authority. Reader compatibility is not an application downgrade
promise. Bounded per-field authority has at most the six finite DetailFields and
retains only comparable same-source/version clocks, never invented cross-source
order. Representation drift restart is native, explicit, fenced and bounded once
per dispatched job; rejected observations themselves commit no content/revision.
Root owns any necessary source generator exclusions for new native-only types.
No migration/IPC change is expected and frozen0001..0007 stay unchanged.

# RURU-101 source audit and bounded correction proposal

Source: signed pre-code dbbcc8052c467165d39f05bd4b736afba93bc78c in worktree101.
Only new tests/facet_reconciliation.rs is edited; production remains unchanged.
Initial actual validation: 2 green controls, 4 reds in
/tmp/gitru-ruru101-independent-initial.log.

## Actual existing policy

| Area | Existing rule and source | Qualification / correction |
| --- | --- | --- |
| Body value | storage/details.rs679–717: known null/empty replaces; omitted/oversized preserves saved value, value_source and validation. Body comparable clock uses value_source at644–654. | Preserve. No summary body promotion. Existing metadata per-field known-mask/clock tests remain unchanged. |
| Collection value | latest source_json orders whole facet at644; value_source also exists and preserves comparable timestamp through304. Entry masks merge only observed fields at184–275. | Actual red: ordering ignores preserved value_source after304 without timestamp. Use comparable saved authority, retaining clocks through304/timestamp-less observations. |
| Child value ordering | merge_entry231–241 compares incoming UpdatedAt with previous public updated_at, under UpdatedAt validation source/version. | Actual red: Known-null UpdatedAt can erase retained Body's clock. Separate saved field authority from public nullable value; omitted/null other fields cannot reset Body authority. |
| Facet304 |657–675: requires existing complete facet, whole_scope=true, stable source/version/mask, stored validator, no content/entries, returned validator absent or equal. | Preserve equality semantics, never order opaque ETags. A matching304 cannot bootstrap or validate an omitted Body. |
| Whole-facet validator | detail.rs213 declares whole_scope as validator coverage. Runtime/details.rs255 sets it only on first terminal page. | Do NOT reinterpret as full-enumeration evidence: existing qualified multipage final page is whole_scope=false. New green preserves this exact behavior. |
| Collection absence | storage/details.rs759 deletes unseen children on complete=true, irrespective of whole_scope. Complete currently only means next_cursor absent in runtime234. | Current trusted adapters/fake full scans retain their behavior. Add explicit native enumeration evidence for full, delta and uncertain/capped/drifting runs; exhausted delta cannot establish deletion or full coverage. |
| Pagination/run | begin_detail504–516 rotates request generation and transfers seen membership when resuming committed partial cursor. Apply627 requires run/cursor match. | Preserve. Actual red: continuation changes source/version/mask and still completes/prunes. Persist captured representation/strategy provenance in existing JSON, compare before accepting continuations. |
| Checks head | apply validates optional dispatch binding; metadata/head invalidation only touches Body atresource_metadata313–396. Entries have explicit HeadOid. | Actual red uses head-a check entries and captured head-a binding: accepted head-b leaves saved success Fresh and old continuation live. Invalidate explicitly head-bound checks independently of Body existence. |
| Review history | finite Reviews facet, entry HeadOid retains original review head; no universal current-head contract. Master968–971, backlogR123 distinguish historical review from current-head approval. | New green preserves historical review through close/merge/new head. Do not infer current-head Reviews policy; future adapter must explicitly declare that scope. |
| Comments history | independent subject collection; parent updated_at does not order comments. | Preserve. Parent edit/state/head alone is not a child revision. |
| Access/drafts | all reads/commits account/instance/epoch/authview/run/subject gated; actual denial hides, private CAS drafts retained. Ordinary selected-cache absence is independent of strict new inbox grant. | Preserve inherited tests/SQL. No feed policy or selection change. |

## Proposed schema-free native contract

1. New native-only typed collection evidence on DetailPage and DetailCommit,
   e.g. FullEnumeration / Incremental / Uncertain. Complete remains traversal
   termination; whole_scope remains validator authority. Body observations do
   not use child absence evidence. Current full trusted collection fixtures map
   explicitly to FullEnumeration; production unsupported adapters stay unsupported.
2. Persist native traversal provenance beside public DetailSource in the existing
   source_json using a serde-flatten wrapper. Include endpoint/version/field-mask,
   enumeration mode and optional explicit head binding. Public DetailSource and
   renderer DTOs are unchanged; old JSON decodes under an explicit conservative
   legacy policy. No schema/migration/IPC/SDK edit is needed.
3. Persist native per-field saved comparable clock beside public DetailEntry in
   existing json with a serde-flatten wrapper. Renderer reads retain the exact
   DetailEntry shape. Known-null UpdatedAt remains nullable public truth, while
   retained Body/other field clocks survive. Upgrade legacy known clocks only
   when existing UpdatedAt/value validation has matching source/version evidence;
   do not invent cross-source order. Reject/ignore genuinely older observed fields
   while preserving independently observed fields and inherited incomparable-source
   partial merge behavior.
4. Explicit head specificity distinguishes historical children from current-head
   evidence. Captured native subject_binding with known head supports the new
   checks regression; optionally a native CurrentHead/SubjectHistory declaration
   makes provider policy unambiguous. Future production Checks must declare exact
   current head, historical Reviews remain history unless explicitly scoped. Keep
   old-head entries as historical data; invalidate freshness/validator/run/cursor
   and restart head-scoped traversal without clearing drafts.
5. For changed continuation provenance, fail before changing cached truth or
   absence. A separately fenced native restart resets only rebuildable cursor/run
   evidence, preserving saved entries/authorities/drafts. Runtime can coalesce one
   restart, never loop repeatedly on a malformed adapter. Qualified full stable
   traversal retains the inherited single-complete collection replacement policy.

## Initial actual test evidence

| Test | Actual result before source edits |
| --- | --- |
| collection_304_without_timestamp_preserves_saved_comparable_ordering_boundary | RED: T2→304(no clock)→T1 accepted Ok rev12. First Comments matrix case executed. |
| child_null_timestamp_cannot_erase_retained_body_ordering_evidence | RED: retained 'newer saved body' becomes 'obsolete body'. First Comments matrix case executed. |
| continuation_representation_drift_cannot_finish_or_prune_a_different_traversal | RED: different endpoint exhausted final page accepted Ok rev12. First endpoint variant executed. |
| accepted_head_change_stales_current_head_checks_and_discards_old_continuation | RED: Fresh athead-b despite saved head-a checks. Old-head explicitly bound response already rejects, which is preserved. |
| completed_same_representation_multipage_scan_reconciles_only_its_own_children | GREEN: all Comments/Reviews/Checks cases; stable multi-page complete+whole_scope=false, overlap ordering, qualified absence, independentBody/privateCAS preserved. |
| close_merge_and_historical_review_head_do_not_order_unrelated_child_fields | GREEN: parent state/head does not relabel or invalidate review history; privateCAS unchanged. |


## Authoritative head conflict correction — before source edits

Root's independent source review proposes and the native lane reproduces an
additional actual red: saved current-head checkA stays Fresh when authoritative
Body metadata proves HeadB(T3), while an intermediate accepted feed still carries
summaryA(T2). R77 correctly keeps summary/list and detail-field authority
independent. Root approves a conservative current-head veto: authorized saved
Known detail Head contradicting a captured summary head cannot qualify current-
head freshness/validator/commit. A known metadata head commit must stale and
fence the contradictory explicitly CurrentHead scope; keep its cached historical
entries and private CAS draft, and do not rewrite summary rows or historical
Reviews. Read-only metadata ambiguity does not invent a new effective head.

Add same-head/no-known-detail/history green controls and exact rejected oldhead
receipt/304 proof. Check current-head authority at commit and lease/dispatch where
its explicit policy is known. First unknown provider facet receipt remains
adapter-qualified and commit-fenced; actual future endpoint/head-selection policy
belongs in R118 and must explicitly reconcile authoritative head evidence before
production dispatch. Do not claim this conservative veto chooses a globally
current server head or orders incomparable endpoint clocks. Record any remaining
endpoint rollout boundary honestly; all such production facets remain Unsupported.

## Captured dispatch and terminal intent fences — before correction

Independent root-requested source review finds a bounded continuation dispatch
gap: after a CurrentHead page captures headA/cursorA, accepted summary headB
rotates stored traversal state. The next worker currently reconstructs subjectB
but validates only metadata-versus-summary conflict, so absent Body metadata it
can send cursorA with headB before commit rejects the old run. Add an actual
runtime regression and make the pre-HTTP native validation compare the captured
DetailLease with current installation, authorization view, run, cursor and
explicit head proof in one read transaction. Preserve access/epoch checks and
the conservative known metadata head veto. A stale continuation may not dispatch
merely because a fresh subject can be read.

The same review finds terminal repeated-drift cleanup uses an epoch-only demand
stop. A rejected old response must not clear a newer same-epoch explicit intent
after deselect/reselect or run/view rotation. Reproduce this race through exact
lease state and fence terminal stop to the captured view/run/cursor as well.
No general feed rewrite, wire DTO or production endpoint rollout is authorized.
Keep history controls, qualified full traversal and private CAS assertions.
