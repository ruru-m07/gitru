# RURU-130 — provider inbox actions

Status: implementation contract, 8 October 2026. This isolated external worktree consumes clean R117 and R127 through an explicit integration-only base; no user pull request is merged. Rust owns native operation admission, provider HTTP, delivery, reconciliation and recovery. The root agent owns desktop IPC, generated bindings and selected-item UI.

## Selected scope

Implement GitHub mark-read and GitLab mark-done only. GitHub done remains explicitly unsupported until its acknowledgement and ambiguous-outcome model is implemented. Gitru-local disposition is independent and never implicitly changed. A successful local admission receipt means durable local intent, not provider confirmation. The same command UUID is retained when an IPC receipt is lost.

Official source contracts inspected 8 October 2026:

- https://docs.github.com/en/rest/activity/notifications — exact GET thread and PATCH mark-read (205/304); no per-thread activity compare-and-swap. Classic PAT or imported OAuth notification permissions differ from unsupported fine-grained/App tokens. No account-wide endpoint is used.
- https://docs.gitlab.com/api/todos/ — filtered GET list and POST todo mark_as_done return explicit todo state; no documented GET single todo or conditional activity fence. Bounded filtered lookup cannot infer completion from absence.
- https://docs.rs/reqwest/0.12.28/reqwest/struct.ClientBuilder.html#method.retry — mutation transports disable internal protocol retries, redirects and unbounded bodies.

The caller must explicitly choose BestEffortCurrentItem, disclosed as possibly applying to concurrent provider activity because neither selected endpoint provides server CAS. Native admission and final claim compare the exact account, authorization epoch/view, immutable notification identity and saved activity version. New observed activity before dispatch conflicts; activity arriving after the final local check cannot be fenced remotely and is covered by the explicit best-effort policy. Unknown outcomes never automatically resend.

GitHub read changes only unread=false: its common state still describes the notification subject type. GitLab completion changes todo completion/state to done while unread remains unknown. Effective local rows, native inbox evidence and counts must stay coherent. Unsupported or missing source semantics do not dispatch. Generic resource remote_write capability remains unavailable; this specialized operation descriptor is the authority for these reviewed inbox actions only.

## API and delivery

Local action descriptors bind canonical local notification subject_id (not its PR/issue subject and not a provider ID), account, authorization epoch/view and opaque activity_version. Admission accepts only typed action/policy plus those fences and a stable command UUID; no URL or operation bytes from the renderer. Fixed native provider IDs route requests. Native evidence proves endpoint-specific outcomes; no arbitrary HTTP success or list absence confirms a command.

Shared mutation transport is same-origin, redirect-none, retry-never, timed and byte bounded. Status/body and quota/auth observations stay separate from operation proof. GitLab preflight is a bounded filtered list: missing target remains unavailable, and depleted success quota prevents dispatch. Recovery allows explicit new best-effort intent after review but never bypasses unknown/accepted reconciliation or restored quarantine.

Root owns the common held-feed-response fence fix in R116/R117: confirmed state must not be overwritten by an earlier equal-timestamp feed response. Consume that qualified prerequisite before publication. No new schema is expected. Tests use finite local HTTP and synthetic SQLite only, covering native action differences, activity/epoch races, exact UUID replay, offline/restart, ambiguous response and recovered pending intent. Personal credentials, production databases and live provider mutations are outside validation. Generated IPC is produced by make typegen; local checks and remote CI are recorded separately.
