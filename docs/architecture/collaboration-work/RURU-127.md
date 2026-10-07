# RURU-127 — GitLab todos

## Pre-implementation contract

Base: signed RURU-119 `218c61024cf5c9c557f4642a8a429742e10bc571`.
Worktree: `/Volumes/Lexar/.codex/wt/ruru-127-gitlab-todos/gitru`.
Branch: `ruru/ruru-127-gitlab-todos`. Scope is GitLab.com; account credentials remain independent of Gitru cloud.

GitLab todos are actionable inbox entries. They are not GitHub unread notification threads. Persist native todo identity, source, action, target type and explicit pending/done completion state in the shared item model. Provider unread is absent. Gitru-local snooze, bookmark and disposition remain independent of provider completion. Native todo completion and notification-read delivery remain unsupported in this slice.

The native adapter traverses documented `GET /todos?state=pending` and `state=done` endpoints in that order. It uses 50-row pages, bounded response bodies, fixed native routes, strict same-route next-page validation and an account/authorization-epoch bound cursor. A page is committed atomically with its selectors and continuation. Both phases must terminate before the existing two-completed-traversal membership policy observes absence. Missing entries never imply provider completion or canonical resource deletion. Mutable offset traversal is not an atomic snapshot. No undocumented updated-time filter, sort order, `state=all`, or read-on-click request is introduced. The existing native scheduler limits pages per turn, persists checkpoints, honors provider cooldowns and permits offline reads.

Supported merge-request/issue subjects carry same-response immutable project ID, native target ID and scoped IID. Cached routing and derived authorization require all coordinates and the account/instance/epoch to agree. Unknown types, unavailable targets and projectless todos stay visible with an explicit fallback. A provider web URL never becomes HTTP authority. This slice does not introduce GitLab uncached subject discovery or hydrate target body data from a todo response.

Use an optional, serde-default typed inbox field in existing item JSON and optional native selector evidence in existing selector JSON. Existing rows remain decodable; no schema migration is required. GitHub continues its existing read/unread semantics. The UI reuses the current To-dos label and pending/done filters, while rendering native source/completion explicitly and exposing no completion button.

## Validation plan

Finite synthetic transport tests: pending/done handoff; exact next-page route/query validation; forged/cross-account/stale-epoch cursor; malformed/duplicate/oversized rows; unknown and projectless targets; preserved rate-limit evidence and partial failure. Storage/runtime tests: durable phase continuation, no absence after partial traversal, local cold/offline reads, pending versus done semantics, account isolation, exact cached subject identity, identity mismatch/selector replacement and unsupported writes. UI tests verify To-dos wording, provider completion labels, original todo context and cached subject navigation. Run generated IPC via `make typegen`, focused checks, then repository verification. Local results are separate from remote CI and live provider checks. No personal token, provider writes or live user database is used for qualification.

## Primary references

- [GitLab To-Do List API](https://docs.gitlab.com/api/todos/): GET lists pending by default; documented state values are pending and done; targets include MR/issue and other types; completion has separate POST endpoints.
- [GitLab REST pagination](https://docs.gitlab.com/api/rest/#pagination): use bounded pages and server continuation links without expanding the native endpoint's authority.

## Progress

- Pre-code audit: live RURU-127 is Backlog with no existing attachment/comment; prerequisites RURU-111, RURU-79 and RURU-100 are In Review. Implementation base contains their read, subject-routing and provider-gating seams. No duplicate worktree/PR found.
