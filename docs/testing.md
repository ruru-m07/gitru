# Frontend testing

Run every frontend test headlessly from the repository root:

```bash
bun run test
```

The root Vitest configuration defines projects for the desktop app, marketing
site, and the `@gitru/ui`, `@gitru/icon`, `@gitru/mascot`, and
`@gitru/collaboration-client` workspaces. It also
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
- packaged desktop end-to-end smoke tests on Linux, macOS, and Windows.

CI installs Bun from the version in the root `packageManager` field. A local
setup action reads and installs the pinned Rust version from
`rust-toolchain.toml`. CI installs Bun dependencies with
`--frozen-lockfile`, and caches the root Bun and Cargo workspaces. Superseded
runs for the same pull request or branch are canceled.

The release workflow checks out the published tag and runs `make verify`
before any Tauri artifact is built or uploaded. A failing revision therefore
cannot publish application artifacts or the updater manifest.

## Packaged desktop end-to-end tests

Run the same production-equivalent desktop smoke suite used by CI from the
repository root:

```bash
make test-e2e
```

This delegates to `bun --cwd=apps/desktop run e2e`, builds the E2E-enabled
Tauri application, and drives critical workflows across the UI, Tauri, Rust,
and Git boundaries. Every scenario creates its repository fixture beneath the
operating system's temporary directory. Application state uses the separate
`com.ruru.gitru.e2e` Tauri identifier; the harness resets only that namespace's
`repositories.json`, `app-state.json`, and the collaboration database/WAL/SHM
before launch. Collaboration uses an in-memory test vault in E2E builds and
never accesses the developer's native credential store. Tests never reuse or
mutate repositories registered in the developer's Gitru data. Destructive Git
operations are scoped to the disposable fixtures, which are cleaned up after
the run. The fixture uses a local bare origin and local avatar fallbacks, so the
workflow does not depend on credentials or public network access.

The collaboration smoke also reads the real native account and change-log
snapshots, validating them against generated wire schemas. This proves isolated
storage initialization and local IPC without an account. It does not verify a
live provider token or a production OS credential store.

The suite opens `/app/git?embedded=1` in the packaged main webview. This covers
the real Git UI, Tauri commands, Rust services, and Git subprocesses, but
intentionally excludes host-tab and child-Webview lifecycle behavior because
the embedded WebDriver cannot target child webviews. The native folder picker
is replaced with the disposable fixture path; the repository-import commands
that follow it remain real.

Milestone screenshots and frontend, backend, driver, and Git-state logs are
written beneath `artifacts/e2e/` for diagnosis. The directory is ignored by Git,
and CI always attempts to upload its contents as a per-platform artifact. The
suite runs on Ubuntu, macOS, and Windows; Linux runs it inside a 16-bit Xvfb
display for WebKitGTK compatibility.

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
native boundary can be crossed. Packaged Tauri behavior belongs in end-to-end
tests, not jsdom tests.

Collaboration storage/provider/runtime tests run with `cargo test -p collaboration`.
They cover authorization epochs, late responses, pagination continuation,
conditional requests, membership reconciliation, rate cooldowns, drafts,
migrations and writer ownership. Frontend fixtures under `tests/fixtures` are
synthetic IPC responses and must never seed production storage. The read
benchmark under `crates/collaboration/examples` uses a temporary database;
storage latency is distinct from IPC/rendering latency and app memory usage.

GitHub CLI tests use an injected runner or an executable created under an
isolated temporary directory, never the developer's actual `gh` credentials.
They cover metadata-only discovery, explicit account choice, candidate expiry,
credential identity mismatch with no persistence, executable installation and
replacement, environment filtering, bounded output, timeout and child cleanup.
Default standalone runtimes and packaged E2E disable personal CLI discovery;
the collaboration E2E spec asserts the disabled result. Frontend tests exercise
the picker, unavailable accounts, retries, token creation links, manual PAT
fallback and connection without submitting the PAT form. Actual production-vault
and provider authentication need a separate live integration run.

Account-dialog tests cover child-to-host requests, main-only credential controls,
listener/lease cleanup, close/reopen races, delayed native view creation and
visibility changes, and restoration of the current tab. The packaged collaboration
spec navigates into the real host/native-child layout before opening Accounts from
Inbox; the ordinary Git smoke retains its embedded startup. WDIO addresses native
WebviewWindows rather than child Webviews. One async driver call retains the main
executor while children exist, then restores the embedded route before returning.
An E2E-only fixed-action bootstrap hook navigates the host, clicks the child's
actual controls, and reports only route/label/instance and form/button booleans.
It accepts no arbitrary scripts or credentials and is excluded from production
output. Native visibility sequencing is separately covered by controlled
asynchronous surface tests.

Close other E2E-feature builds before starting the packaged suite; the embedded
driver uses port 4445 by default and does not retry a failed bind.

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
