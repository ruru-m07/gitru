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
