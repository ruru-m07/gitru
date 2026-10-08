# RURU-120 — local pull request creation UI

## Contract before implementation

This frontend lane builds on the native RURU-120 contract and schema 23. It owns `packages/collaboration-client` and desktop collaboration UI only. The native owners implement storage, recovery, Git observation, provider delivery, IPC, and generated commands. This note supplements `RURU-120.md`; that contract and `remote-collaboration-engine.md` remain authoritative.

Opening, recovering, and saving an authored pull draft reads or writes local storage only. A draft includes title, body, source/base branch names, ready/draft choice, and an explicitly chosen registered clone/link generation. The renderer sends registration IDs and native link versions, never paths or observations. Initial support is same-repository GitHub pull requests; unsupported providers remain explained by native typed reasons.

The user explicitly previews a saved draft. Native code freshly checks the clone, mapping, published branches, permission, and exact tips. Preview grants are process-local and expire; the SDK neither persists nor shares them through the query cache. Editing, a changed draft generation/authorization view, runtime reset, or an expired preview disables submission. A clock timer is solely presentation of native grant expiry and confers no authority.

Submission requires confirmation that GitHub creates against the branches' current tips. No exact-tip atomicity is claimed. A locally lost admission receipt retains the exact command UUID/context for duplicate admission, without creating a fresh request. Unknown delivery is shown through Saved changes and does not automatically resubmit. Strong provider receipts retain the created identity and separately show observed branch drift.

Drafts survive disconnection/recovery. SDK reset redacts provider identity/preview availability synchronously while retaining authored text. Local query invalidation uses `pull_draft:` changes and existing revision/authorization fences. Cross-window changes preserve unsaved text and require an explicit latest-snapshot load before further writes. A paginated global recovery view finds drafts even when their repository is no longer selected or visible.

## Validation plan

- Generated command/wire tests, account/key/view fencing, held-response reset, synchronous authority redaction, and draft preservation.
- UI controls for local-only opening/saving, explicit preview then submit, expired/changed preview, exact UUID retry, receipt/drift display, local clone/link selection, and recovery pagination.
- Run relevant SDK and desktop tests, Biome, TypeScript, and a desktop production build. Native owners qualify schema/recovery/provider tests separately. Do not infer packaged, live credential, or remote CI results from local checks.

## Progress

- Contract recorded before frontend implementation. No frontend verification yet.
