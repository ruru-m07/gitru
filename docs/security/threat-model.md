# Desktop threat model

Gitru renders local repository data in a Tauri webview and exposes native Git and
filesystem operations through typed commands. Treat repository names, paths,
commit messages, diffs, remotes, and hosted-service responses as untrusted.

## Assets and trust boundaries

- **Credentials and tokens:** Git credentials remain with Git/credential helpers.
  Hosted-service tokens use the operating-system credential store; they must not
  enter URLs, analytics, or logs. GitHub PAT entry clears the password field
  before awaiting native verification and does not persist tokens in UI state.
  GitHub CLI discovery returns metadata and opaque expiring candidate IDs only.
  Explicit import retrieves the selected credential natively and verifies its
  `/user` identity before writing to the vault or database. CLI subprocesses use
  recognized executable paths, a neutral working directory, bounded output and
  deadlines; token environment overrides, debug logging and prompts are disabled.
  No CLI token is returned to a webview. Gitru never initiates CLI login or
  changes the active CLI account.
- **Repository and filesystem data:** command inputs are scoped to an open
  repository context. File operations must preserve the existing service-layer
  path checks and must not accept arbitrary web content as a path.
- **Native commands:** only local application webviews receive IPC access. The
  main webview owns child-webview, update, and restart capabilities. Embedded tab
  webviews receive read-only app-store access and the directory picker, but no
  process, updater, opener, notification, or webview-management plugin access.
- **Remote content:** external navigation crosses a backend HTTPS-only validator.
  Remote images are limited to the explicitly listed avatar hosts and the CSP
  blocks all other image and network origins.
- **Collaboration data:** private cached observations are partitioned by account
  and authorization epoch. Revocation and scope denial fence native reads and
  delayed frontend responses. Native collaboration management commands require
  the main local webview; tab webviews can read authorized snapshots and save
  private drafts. Provider bodies render as text. SQLite files use private Unix
  permissions but are not encrypted; drafts survive account disconnection.
  Child Accounts buttons send a fixed, payload-free UI hint to the main host;
  this event never supplies credentials, account choices or authorization.
  The host hides native tab surfaces before mounting credential controls and
  restores the selected tab on close. Repository selection is an authorized
  domain preference available in local child tabs.

## Expected attackers

The primary inputs are a malicious repository, crafted Git metadata, a hostile
remote URL, or compromised remote image/analytics infrastructure. The security
boundary must also limit the impact of a frontend injection bug. Gitru does not
claim to defend a user whose operating-system account or Git executable is
already compromised.

## Controls and review requirements

1. Keep the CSP deny-by-default. Add a host only for a documented feature and
   constrain it to the narrowest directive.
2. Add plugin permissions to the webview that uses them, never a wildcard
   window capability. New native commands must validate untrusted strings in
   Rust even if the frontend already validates them.
3. Never log repository paths, remotes, commit data, diffs, file contents,
   credentials, tokens, or raw command errors. Operational logs may include the
   command name, duration, and a non-sensitive error category.
4. Security tests must cover rejected URL schemes/credentials and remote-content
   host allowlists. A packaged build must be exercised when CSP sources change.

## Telemetry

Anonymous usage analytics is off until the user explicitly enables it in the
sidebar. It sends only fixed app-open and presence event names plus basic runtime
metadata such as operating system and screen size. Autocapture, session
recording, surveys, page URLs, titles, referrers, and person profiles are
disabled. The anonymous identifier is memory-only and changes between launches.
Repository paths, code, diffs, remotes, branches, and commit data are never
included. The same sidebar control disables collection immediately.
