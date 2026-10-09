# RURU-127 — GitLab todos

## Pre-implementation contract

Base: signed RURU-119 `218c61024cf5c9c557f4642a8a429742e10bc571`.
Worktree: `/Volumes/Lexar/.codex/wt/ruru-127-gitlab-todos/gitru`.
Branch: `ruru/ruru-127-gitlab-todos`. Scope is GitLab.com; account credentials remain independent of Gitru cloud.

GitLab todos are actionable inbox entries. They are not GitHub unread notification threads. Persist native todo identity, source, action, target type and explicit pending/done completion state in the shared item model. Provider unread is absent. Gitru-local snooze, bookmark and disposition remain independent of provider completion. Native todo completion and notification-read delivery remain unsupported in this slice.

The native adapter traverses documented `GET /todos?state=pending` and `state=done` endpoints in that order. After at most five done-history pages, it saves that continuation and revisits the entire pending feed before resuming completed history; no undocumented ordering is assumed. A pending sweep can span native turns and retains the done checkpoint. Thus completed history cannot indefinitely defer new pending sweeps; provider rate limits and the pending collection itself still bound attainable freshness. It uses 50-row pages, bounded response bodies, fixed native routes, strict same-route next-page validation and an account/authorization-epoch bound cursor. A page is committed atomically with its selectors and continuation. Both phases must terminate before the existing two-completed-traversal membership policy observes absence. Missing entries never imply provider completion or canonical resource deletion. Mutable offset traversal is not an atomic snapshot. No undocumented updated-time filter, sort order, `state=all`, or read-on-click request is introduced. The existing native scheduler limits pages per turn, persists checkpoints, honors provider cooldowns and permits offline reads.

Supported merge-request/issue subjects carry same-response immutable project ID, native target ID and scoped IID. Cached routing and derived authorization require all coordinates and the account/instance/epoch to agree. Unknown types, unavailable targets and projectless todos stay visible with an explicit fallback. A provider web URL never becomes HTTP authority. This slice does not introduce GitLab uncached subject discovery or hydrate target body data from a todo response.

Use an optional, serde-default typed inbox field in existing item JSON and optional native selector evidence in existing selector JSON. Existing rows remain decodable; no schema migration is required. GitHub continues its existing read/unread semantics. The UI reuses the current To-dos label and pending/done filters, while rendering native source/completion explicitly and exposing no completion button.

## Validation plan

Finite synthetic transport tests: pending/done handoff; exact next-page route/query validation; forged/cross-account/stale-epoch cursor; malformed/duplicate/oversized rows; unknown and projectless targets; preserved rate-limit evidence and partial failure. Storage/runtime tests: durable phase continuation, no absence after partial traversal, local cold/offline reads, pending versus done semantics, account isolation, exact cached subject identity, identity mismatch/selector replacement and unsupported writes. UI tests verify To-dos wording, provider completion labels, original todo context and cached subject navigation. Run generated IPC via `make typegen`, focused checks, then repository verification. Local results are separate from remote CI and live provider checks. No personal token, provider writes or live user database is used for qualification.

## Primary references

- [GitLab To-Do List API](https://docs.gitlab.com/api/todos/): GET lists pending by default; documented state values are pending and done; targets include MR/issue and other types; completion has separate POST endpoints.
- [GitLab REST pagination](https://docs.gitlab.com/api/rest/#pagination): use bounded pages and server continuation links without expanding the native endpoint's authority.

## Progress

- Pre-code audit: live RURU-127 is Backlog with no existing attachment/comment; prerequisites RURU-111, RURU-79 and RURU-100 are In Review. Implementation base contains their read, subject-routing and provider-gating seams. No duplicate worktree/PR found.


### Implemented behavior

- `NativeInboxState` carries either notification unread evidence or native todo completion, action and target type. Legacy item JSON remains readable with absent source evidence; a todo never becomes unread by inference. The generated runtime codec preserves the tagged payload and rejects unknown completion values.
- GitLab inbox capability is read-only. Each native GET reads at most 50 rows with the existing four-MiB response and request timeout bounds. There is no conditional 304 authority over the two state feeds. Cursors contain only native account/epoch/phase/page counters, reject unknown keys and impossible interleave states, are limited to 1,024 bytes and positive 32-bit page positions, and advance only with an atomic page commit. Same-phase continuation must be exactly the next page; a provider URL is neither persisted nor exposed as a request.
- Five done-history pages trigger a full pending sweep before the saved done continuation resumes. The scheduler retains its ten-page activation budget and provider cooldown. A failed pending page preserves both its position and the done checkpoint. Only the final done page completes the aggregate traversal; neither intermediate pending sweeps nor absence imply completion. Offset pagination remains mutable rather than a point-in-time snapshot.
- Todos for supported MR/issue targets carry immutable target/project IDs plus IID. Cached routing checks these against the exact account/instance/epoch, alias ambiguity and live membership. The derived grant permits point-detail hydration for an unselected repository, while repository feeds still require selection. Selector withdrawal retires queued detail demand and saved access. Unknown, projectless or unavailable targets remain visible with a safe presentation link where the provider supplied one; no uncached GitLab subject discovery is introduced.
- Thin embedded project summaries preserve already known description/default-branch metadata. Existing selected repository state remains independent. Local snooze, dismissal and bookmark state remain Gitru-only; the UI names provider pending/done state and offers no completion action.

### Local qualification

- Focused GitLab native suite: **81 passed**, including actual synthetic HTTP → runtime → SQLite, pending/done pagination, 5-page interleave, failed pending continuation, cold resume at the identical account/epoch checkpoint, quota expiry, ten-page yield, offline reads without vault loads, exact cached subject routing and withdrawal before queued detail dispatch.
- Generated IPC: `make typegen`, **129 commands / 388 schemas**. Typed UI/schema tests cover native source/completion payloads and reject unknown or unread-shaped todo values.
- Full `make verify` passed on the final implementation: **716 frontend tests / 1 platform skip; 1,091 top-level Rust tests plus 2 child helper runs / 5 standalone helper ignores**, and all frontend lint, TypeScript checks, desktop build, Rust format and strict workspace/all-target Clippy. The focused cached-subject suite passes **27 cases**, including refusal of ambiguous locators even when the todo carries a matching target ID.
- No personal credential, live provider, production vault/database, provider mutation or GUI run is represented by these finite local fixtures. Remote CI is running for the draft PR and is recorded separately below.


### Publication

Draft [PR #174](https://github.com/ruru-m07/gitru/pull/174) is stacked on RURU-119 #172. Linear RURU-127 is **In Review** with all three acceptance criteria implemented. Final signed implementation after restacking: `57635247ae07d77e4fa8784b761344d367841d75` on RURU-119 `08a2db3c4d66269ce72bc5a780e9deda78a57f43`. The full local qualification above ran on signed source `2328b760f78a8f1347f81555d23f325daa82d0da`; restacking changes the inherited Git path test fixture for Windows and leaves this implementation unchanged. The inherited correction passes all **13 `pull_file_service` tests** locally after restacking. Exact-head remote CI remains pending and no merge is authorized or performed.
