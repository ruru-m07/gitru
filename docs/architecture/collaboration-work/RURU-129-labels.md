# RURU-129 — GitHub label desired-state intent

Status: bounded pre-code contract, 8 October 2026.

This follow-up adds label membership edits for saved GitHub.com issues and pull
requests. It starts from the qualified RURU-53 integration head `f0b8801d` and
stacks on PR #190. Title/body edits remain PR #181 and close/reopen remains PR
#185. Label creation, rename, deletion, issue-creation metadata, assignees,
milestones, GitLab, Bitbucket, GitHub Enterprise and bulk edits are outside this
slice. Accounts remain independent of Gitru cloud. No live credential or provider
write is used for unattended qualification.

## Provider contract and safety boundary

GitHub exposes labels for pull requests through the Issues label endpoints. The
documented add and remove operations are addressed by label **name**, require
Issues (write) or Pull requests (write), and return the resulting label set on a
successful request. The set-labels endpoint replaces every existing label. None
of these endpoint contracts documents a conditional version or compare-and-swap
token.

Gitru therefore does not use the replace-all endpoint. One immutable command
stores a bounded, disjoint delta of typed label identities: labels to make present
and labels to make absent. A label identity contains the canonical positive
GitHub numeric label ID, exact name and optional color observed from provider
metadata. Name-only labels never authorize a write.

Fresh preflight reads the exact numeric-repository issue resource and validates
the account, repository, subject number/native ID, resource kind and complete
current label set. Unrelated remote label additions and removals are preserved.
A touched label that already has its requested membership is converged. A touched
ID/name reassignment or rename is a conflict. Adds also revalidate the selected
name/ID through a bounded native point read before dispatch. Read access does not
prove write permission; a mutation denial remains explicit provider evidence.

Dispatch uses the fixed numeric repository route. It posts all still-missing adds
once, then removes still-present labels one by one with a correctly encoded path
segment. It never falls back to a mutable owner/name route. Every native write is
preceded by the durable attempt record. After every successful step, the operation
continues only inside the same bounded dispatch and performs a fresh exact resource
read before confirmation. That read must prove every touched membership using the
same IDs and names. It publishes the entire canonical label set, so unrelated
concurrent membership survives.

Any transport ambiguity, partial success, unexpected response, permission loss or
failed readback is outcome-unknown. It never automatically repeats a write or
blindly completes the remaining delta on a later turn. Read-only reconciliation
may confirm when every touched membership is observed. A complete divergent
readback after known successful steps becomes review-required conflict; it is not
silently replaced. The UI discloses that name-addressed provider writes can race
after preflight and that Gitru cannot claim remote compare-and-swap.

Official primary references, checked 8 October 2026:

- https://docs.github.com/en/rest/issues/labels?apiVersion=2026-03-10
- https://docs.github.com/en/rest/issues/issues?apiVersion=2026-03-10

The absence of a documented conditional mutation token is an inference from
those contracts, not a claim about GitHub's internal implementation.

## Local model and IPC

The public native DTOs are frozen before IPC integration:

- `LabelIdentity { provider_id, name, color }`
- `LabelSetContext { account_id, subject_id, authorization_epoch,
  authorization_view, review_token }`
- `LabelSetSnapshot { context, canonical_labels, effective_labels,
  available_labels, catalog_complete, catalog_truncated, availability, reason,
  pending_intent, revision, authorization_view }`
- `LabelSetRequest { context, command_id, add_labels, remove_labels,
  accept_best_effort }`
- `LabelSetReceipt { account_id, command_id, admitted_revision, duplicate }`

The delta is nonempty, disjoint by provider ID and name, deterministically sorted,
and bounded to 32 touched labels. Each set is unique by both provider ID and name.
Names are nonempty, trimmed, control-free UTF-8 up to 1,024 bytes; provider IDs
are canonical positive decimal integers; colors are optional six-digit lowercase
hex. The exact known base and resulting target are each bounded to 100 labels.
The renderer supplies no URL, route, provider payload, native base, proof or
capability override.

`label_set_snapshot` is local-only. It is available only for an active GitHub.com
account whose current Body metadata has a complete known Labels field from the
matching GitHub issue/pull adapter. Its review token binds the current canonical
set, resource/native identities, Body facet revision and authorization view.
`submit_label_set` validates bounds before account or SQLite lookup and preserves
the exact command UUID for a lost local receipt retry.

`available_labels` is a deterministic, bounded union of typed labels already
observed for the same repository and current authorization epoch. It is a local
picker aid, never capability or completeness evidence. `catalog_complete` is
false in this slice; `catalog_truncated` reports the 100-label local display cap.
The UI says that only saved label options are shown. It cannot submit name-only or
unobserved labels.

## Canonical and effective state

Admission records operation `github.edit_labels`, payload version 1. The immutable
codec carries the exact typed delta, exact canonical base, native subject and
repository identities, source, current provider update time, authorization view
and review token. The effective effect stores the calculated target label set and
adds `IntentField::Labels`; canonical provider metadata remains untouched.

Local item/detail reads expose pending state immediately. The selected-resource
header and editor render the effective label set while clearly marking it queued.
Finalization validates exact current account/epoch/resource/Body revision fences,
publishes the full fresh provider observation through the existing scoped Body
finalizer and retires the label effect atomically on confirmation. Conflict
publication updates canonical facts while retaining authored intent for review.

The existing schema-22 command/effect/evidence tables are sufficient, so this
slice adds no migration. The optional label effect field uses explicit serde
default/skip rules: existing version-1 effect JSON re-encodes byte-for-byte and
old operation/proof codecs are not changed. Restore stays at schema policy 22 and
must accept valid new label commands/effects while preserving the same immutable
envelope, attempt and evidence checks. Corrupt, oversized, duplicate or name/ID
reassigned payloads are rejected without altering either backup or target.

## Recovery and UI behavior

Unknown/conflict review exposes one typed Labels field with bounded human-readable
base, remote and intended memberships. Generic renderer text never reconstructs
native label identities. This first slice does not allow generic replacement;
the user may cancel the retained command and open a fresh label editor backed by
current typed identities. Pause/resume/export and exact native reconciliation use
the existing RURU-117 rules. Restored quarantine is reconciliation-only.

The editor uses coss components and generated `@gitru/commands` bindings. It keeps
selection and exact retry identity across local query errors, disables new intent
when context changes, requires explicit best-effort consent, and never performs a
provider request when opened. Account reset synchronously removes provider-derived
context/catalog/canonical labels from the client cache while retaining no separate
authored label draft; durable admitted intent remains under native recovery.

## Qualification gates

- Exact local base, deterministic catalog, bounds, name-only refusal, offline
  admission, immediate effective projection and cold cache reopen.
- Exact UUID retry, epoch/view/resource/facet fences and no cross-account late
  receipt or cache refill.
- Fresh preflight preserving unrelated adds/removes; aligned touched changes;
  rename, deleted/recreated ID and reassigned-name conflicts.
- Add-only, remove-only and mixed deltas; literal Unicode/reserved-character
  names; numeric routes only; write permission/auth/quota failures.
- Durable attempt before the first write; partial success, lost response and
  failed readback never trigger a second write; read-only convergence is exact.
- Atomic canonical publication/effect retirement, held older feed rejection,
  conflict canonical update and restore quarantine.
- Exact old effect JSON compatibility, new effect/command restore controls and
  unchanged schema-22 recovery policy.
- Focused Rust/SDK/UI tests, strict Clippy/format, generated IPC through
  `make typegen`, then `make verify`. Remote CI, live GitHub mutation and packaged
  vault/platform evidence are reported separately.
