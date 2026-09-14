# Frontend testing

Run every frontend test headlessly from the repository root:

```bash
bun run test
```

The root Vitest configuration defines projects for the desktop app, marketing
site, and the `@gitru/ui`, `@gitru/icon`, and `@gitru/mascot` workspaces. It also
discovers unit tests for the generated `@gitru/commands` package without
modifying generated source. A matching `*.test.ts`, `*.test.tsx`, `*.spec.ts`,
or `*.spec.tsx` file in one of those workspaces automatically joins the root
suite, so a package cannot be silently omitted because it lacks a local test
script.

For desktop-only work, use `bun --cwd=apps/desktop run test`; use
`bun --cwd=apps/desktop run test:watch` while iterating. `make test` runs both
the frontend suite and the Rust workspace tests, while `make verify` mirrors the
broader CI verification path. That path also lints and type-checks the shared
test configuration itself.

## Continuous integration

Every pull request, every push to `dev`, and every manual CI dispatch runs
four independent checks:

- frontend tests, lint, type checks, and the desktop frontend build on Linux;
- Rust formatting and Clippy on Linux;
- Rust workspace tests on Linux, macOS, and Windows with matrix fail-fast
  disabled so one platform failure does not hide the others;
- release-mode desktop end-to-end smoke tests on Linux, macOS, and Windows.

CI installs Bun from the version in the root `packageManager` field. A local
setup action reads and installs the pinned Rust version from
`rust-toolchain.toml`. CI installs Bun dependencies with
`--frozen-lockfile`, and caches the root Bun and Cargo workspaces. Superseded
runs for the same pull request or branch are canceled.

The release workflow checks out the published tag, rejects any base Tauri
configuration whose identifier is not exactly `com.ruru.gitru`, and then runs
`make verify` before any Tauri artifact is built or uploaded. A qualification
revision or failing production revision therefore cannot publish application
artifacts or the updater manifest.

On the RURU-93 migration branch, CEF is the only desktop runtime. The
cross-platform release-mode E2E job is therefore a required CEF/CDP gate, and
the Rust quality and test jobs install the same CEF/GTK4 prerequisites. The
branch-scoped `CEF qualification` workflow builds native CEF installers without
signing or updater credentials and audits their sandbox metadata and
redistribution notices. See the
[Tauri 3 and CEF migration runbook](./architecture/tauri-3-cef-qualification.md)
for the remaining production gates and promotion sequence.

## Release-mode desktop end-to-end tests

Run the same release-mode desktop smoke suite used by CI from the repository
root:

```bash
make test-e2e
```

This delegates to `bun --cwd=apps/desktop run e2e`, enables the CEF runtime's
test-only reset and debugging hooks, and connects directly to Chromium's
DevTools Protocol (CDP). macOS
launches an `.app` bundle because CEF resolves its framework and helpers through
that layout; Linux and Windows launch the raw release binary. No Chrome
installation or version-matched WebDriver is required. The CEF runtime exposes
its unauthenticated debugging port only when the Cargo `e2e` feature is compiled;
the runner reserves a loopback port and passes it at launch. Normal development
and release builds keep remote debugging disabled. Installer layout, sandbox
activation, and installed-app behavior belong to the manual qualification
matrix, not this E2E suite.

The upstream WebdriverIO Tauri packages are not used for this path. Their 1.4.0
Rust plugins still depend on Tauri 2 and their platform executors drive the
system WebView2, WKWebView, or WebKitGTK implementations rather than Tauri 3's
CEF runtime. Reintroduce them only after upstream publishes explicit Tauri 3 +
CEF support and demonstrates child-webview coverage.

Every scenario creates its repository fixture beneath the operating system's
temporary directory. Application state uses the separate
`com.ruru.gitru.e2e` Tauri identifier; the harness resets only that namespace's
`repositories.json` and `app-state.json` before launch. CEF's profile and mock
secret store also live under the disposable fixture root. Tests never reuse or
mutate repositories or browser state from a developer's Gitru installation.
Destructive Git operations are scoped to the disposable fixtures, which are
cleaned up after the run. The fixture uses a local bare origin and local avatar
fallbacks, so the workflow does not depend on credentials or public network
access.

The suite opens the real `/app` host shell, which creates the `/app/git` child,
and requires CEF to expose one separate CDP page target for each managed child
webview. It performs the critical repository import, stage, commit, publish,
stash-and-switch, and conflicting rebase/abort flows in the active child target,
crossing the UI, Tauri, Rust, and Git boundaries. It also creates and closes a
second workspace tab and asserts that its CEF target appears and disappears.
This guards the child-webview topology that the previous embedded WebDriver
suite could not observe.

On Linux, the E2E job builds first and then verifies the raw CEF
`chrome-sandbox` helper as a regular file before assigning the required
`root:root` ownership and mode 4755 on the ephemeral runner. Running the raw
binary without this preparation correctly aborts instead of silently disabling
the sandbox. Installer sandbox ownership and activation remain separate package
qualification gates.

The harness fails on errors or uncaught exceptions from every live target. It
checks the temporary second child before closing it, then records teardown-time
messages from that deliberately destroyed target separately in
`closed-target-diagnostics.json`; requests canceled during destruction are not
attributed to the surviving application target.

Milestone screenshots, target inventories, Chromium version metadata,
frontend diagnostics, application/CEF logs, and Git-state logs are written
beneath `artifacts/e2e/` for diagnosis. The directory is ignored by Git, and CI
always attempts to upload its contents as a per-platform artifact. The suite
runs on Ubuntu, macOS, and Windows; Linux uses a 24-bit TrueColor Xvfb display
because the alpha CEF X11 host requires that visual depth when it creates its
child container. Native window-manager behaviors such as title-bar
dragging, maximize gestures, system menus, focus transfer, and installer/update
replacement remain outside CDP's scope and require the platform-specific
packaged/manual checks described by their owning release work.

React workspace tests use Vitest, jsdom, and React Testing Library. Put desktop
cross-cutting suites in `apps/desktop/tests` and small, feature-specific suites
beside their source. Import test APIs from `vitest`. The shared setup provides
jest-dom matchers, isolated browser storage, and deterministic browser API
shims; desktop tests extend it with the native-boundary mock below.

## Native boundary

The desktop setup installs a deterministic `main` Tauri window and an IPC mock
that rejects every unregistered native command. Register only the commands a
test expects:

```ts
import { getStatus } from "@gitru/commands";
import { expect, test } from "vitest";
import { mockTauriCommandResult } from "./mocks/tauri";

test("loads status", async () => {
  mockTauriCommandResult("get_status", { files: [] });

  await expect(getStatus({ contextId: "repo-1" })).resolves.toEqual({
    files: [],
  });
});
```

Use `mockTauriCommand(command, implementation)` when the response depends on
the payload or when the test must assert how a command was called. Never install
an allow-all IPC handler: an unexpected command should fail the test before any
native boundary can be crossed. Release-mode Tauri behavior belongs in end-to-end
tests, not jsdom tests.

## Coverage expectations

- New or changed frontend behavior must include a focused test in the same
  change. Bug fixes should first reproduce the regression.
- Test state and selection rules as unit tests; test user-visible interaction
  with Testing Library queries and `user-event`.
- Reset singleton stores and use explicit fixtures. Do not depend on test order,
  wall-clock time, the developer's storage, network access, or a native Tauri
  process.
- Prefer observable behavior over implementation details and broad snapshots.
  Cover the important success, empty, loading, and failure paths that the change
  introduces.

Coverage is change-based until the existing frontend has a meaningful measured
baseline; a global percentage threshold must not replace the behavioral checks
above.
