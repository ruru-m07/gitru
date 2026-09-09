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
