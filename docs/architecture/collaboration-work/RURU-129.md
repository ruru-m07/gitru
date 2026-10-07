# RURU-129 — GitHub title/body desired-state edits

Status: bounded implementation contract, 8 October 2026. Base: clean R117
`e551eec`. This is the first selected-operation slice of RURU-129; state changes,
labels, GitLab, and Bitbucket edits remain unavailable and keep the issue partial.

## Operation boundary

GitHub.com issues and pull requests accept durable title/body intent from a local
snapshot. Admission has no HTTP dependency. It binds the account, current actor
and authorization epoch/view, immutable native repository and subject identities,
known canonical Body metadata, workflow state and PR head to a versioned native
codec. The renderer supplies only selected text fields, command UUID, and an
opaque native review context. It cannot supply routes, provider payloads, guards,
execution context, evidence, or capability overrides.

Only changed fields are sent. Body edits preserve known null separately from an
omitted field; unknown/omitted/oversized bases cannot authorize editing. Initial
limits are 16 KiB UTF-8 body and 1 KiB/256 characters title. Complete native
preparation and evidence are bounded to the existing 64 KiB command-delivery cap.
No text is silently truncated. An unavailable snapshot or failed submission leaves
entered UI text intact. Cached read access does not prove token write scope;
provider authorization remains explicitly unverified until the mutation response.
Accounts continue to work independently of Gitru cloud.

## Provider semantics and safety

The documented issue and pull update endpoints accept title/body PATCH fields but
provide no documented conditional compare token for these edits. A GET followed
by PATCH is therefore **best effort**: a provider edit can race in the interval.
The product must disclose this limitation. It must not claim optimistic UI or
preflight comparison provides remote compare-and-swap.

Each provider turn obtains fresh native context inside the writer transaction,
then performs a bounded authenticated GET outside it. The response must match the
installation, repository ID, typed subject ID/number, workflow/source, and PR head.
Changed authored fields compare base/remote/desired; independent untouched remote
fields remain untouched. Overlapping edits and unknown guards become explicit
conflict, without dispatch. The writer revalidates the context before committing
an attempt and its execution base. A dispatch requires a durable attempt first.

The shared transport is owned by R130: fixed native route, same API origin,
redirects/retries disabled, bounded request/response, pinned API version and finite
timeout. R129 owns the title/body route, payload, identity and outcome codecs.
Quota/auth observations are independent of operation proof and retain the R115
failure semantics.

Confirmed means the desired fields were verified on the exact canonical resource;
it does not claim causal proof that this command produced them. A successful PATCH
must return validated canonical resource evidence. After transport ambiguity, a
fresh exact-resource GET may prove convergence. A different field value does not
prove non-delivery, and never causes automatic PATCH replay. Unknown outcomes retain
intent for R117 review. Restored quarantine can only reconcile; it never dispatches.

Canonical observations commit through R116's scoped Body finalizer, atomically
with command resolution and optimistic-effect retirement. Its timestamp, identity,
source and active authorization checks remain authoritative. Raw provider facts
and effective pending intent remain separate. Recovery replacement uses R117's
immutable supersession mechanism and native policy choices; no generic payload or
retry bypass is added.

## Native seams and ownership

R129 adds a default-empty `CommandDeliveryPolicy::prepare_context_in` hook. Both
preparation and reconciliation capture <=64 KiB of fresh native context inside the
claim transaction and carry it on the native-only request. A provider policy does
not retain a Store, so backup/restore runtime replacement cannot strand it on a
closed database. `validate_claim` rechecks current authoritative facts after HTTP.
Existing synthetic policies keep their empty context. No migration is required.

Native modules, codec/admission/storage/runtime, provider operation and tests are
owned by the R129 worktree. Root owns later desktop IPC, generated bindings, SDK
and UI integration after DTO coordination. R130 owns `providers/transport.rs` and
`providers/transport/mutations.rs`; consume its signed checkpoint rather than
creating another transport. No production credentials or live mutation validation.

## Qualification gates

- Offline admission, exact retries, stale snapshot/epoch rejection and immediate
  effective list/detail/search projection with preserved authored bytes.
- Fresh HTTP preflight: independent change preserved, overlap conflict, native
  identity/source/head drift prevents dispatch, arbitrary route rejected.
- Durable attempt-before-write and no automatic repeat after timeout/reset/restart.
- Canonical response finalization, GET convergence, absent desired state remains
  unknown, old provider observation cannot replace newer canonical base.
- Auth/quota response handling, bounded context/response/evidence and restore
  quarantine, owned shutdown and account cutover.
- Operation-specific R117 review/replacement; unsupported codecs/providers fail
  explicitly. Current migration/recovery tests remain valid without schema bump.
- Meaningful focused tests, strict Clippy, generated IPC via `make typegen` if
  commands are integrated, then `make verify`. Report remote CI and any real
  provider/platform checks separately; no live write claim from fixture tests.

## Sources and inference

[GitHub issue update](https://docs.github.com/en/rest/issues/issues#update-an-issue)
and [pull request update](https://docs.github.com/en/rest/pulls/pulls#update-a-pull-request)
document selected fields and token permissions. The lack of a documented edit CAS
contract is the reason for the explicit best-effort boundary; this is an inference
from the documented endpoint contract, not a claim that GitHub cannot implement
other internal concurrency mechanisms. Researched 7 October 2026. Existing adapter
version is `2026-03-10`.
