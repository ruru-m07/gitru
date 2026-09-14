# Tauri 3 and CEF migration

## Decision

Gitru is moving to Tauri `3.0.0-alpha.0` with CEF as its only desktop runtime.
The desktop manifest, application entry point, default development command,
release build command, E2E harness, and CI runtime assertions all select CEF.
There is no Wry dependency, Cargo feature, configuration overlay, build lane, or
fallback path in this branch.

This branch still uses the disposable `com.ruru.gitru.cef-qualification`
application identity. That boundary keeps migration builds away from a user's
production Gitru profile and prevents the release workflow from signing or
publishing them. Moving to the production identifier is a separate promotion
step after the release gates in this document are satisfied.

Upstream references:

- [Tauri 3 alpha release](https://github.com/tauri-apps/tauri/releases/tag/tauri-v3.0.0-alpha.0)
- [Tauri 3 CEF documentation](https://github.com/tauri-apps/tauri-docs/blob/v3/src/content/docs/develop/cef.mdx)

## Runtime contract

`apps/desktop/src-tauri/Cargo.toml` depends on
`tauri-runtime-cef = 3.0.0-alpha.0` unconditionally. The only application
features are:

| Feature | Purpose |
| --- | --- |
| `qualification` | Requires the disposable qualification identity and disables production side effects |
| `e2e` | Extends `qualification` with reset hooks and a local CDP port for the packaged E2E harness |

The default feature set is `qualification`. `tauri::Builder` always receives a
CEF runtime and the binary entry point always uses CEF's entry-point macro.
Plain `make dev`, `make build-tauri`, and `bun --cwd=apps/desktop run tauri ...`
therefore use CEF without a runtime flag.

CI rejects the branch if the Rust graph contains `tauri-runtime-wry`, Tauri 2,
GTK3, or WebKitGTK. The GTK3/WebKit checks matter on Linux: a passing CEF build
must not quietly retain the old system-webview dependency graph.

The frozen JavaScript graph is not yet pure v3. The published Tauri plugin
`3.0.0-alpha.0` packages used by Gitru currently resolve eight nested
`@tauri-apps/api@2.11.1` copies alongside the root
`@tauri-apps/api@3.0.0-alpha.0`. The qualification safety job counts these
copies so an upstream metadata change cannot pass unnoticed. Remove the check
only after updating the plugins, regenerating the lockfile, and requalifying
the plugin bridge.

## Safety boundary

Migration builds currently have all of the following protections:

- product name `Gitru Runtime Qualification` and identifier
  `com.ruru.gitru.cef-qualification`;
- updater endpoints and public key are empty, updater artifacts are disabled,
  and Rust denies updater commands in qualification builds;
- analytics initialization is omitted and analytics hosts are absent from the
  qualification CSP;
- CEF and Chromium redistribution notices are prepared immediately before
  bundling and are audited inside each installer;
- the release workflow rejects any checkout whose base identifier is not
  exactly `com.ruru.gitru` before verification, signing, notarization, or
  upload.

Tauri's alpha updater plugin requires a non-null configuration object even
when updates are disabled. The intentionally inert object therefore remains in
`tauri.conf.json`; a Rust regression test rejects missing or network-enabled
replacements.

## Local use

CEF needs CMake and Ninja on macOS and Windows. Linux needs GTK4, `libxdo`, and
the normal bundler tools. Its first build downloads a large platform-specific
CEF distribution into:

- Linux: `~/.cache/tauri-cef`
- macOS: `~/Library/Caches/tauri-cef`
- Windows: `%LOCALAPPDATA%\tauri-cef`

Run the native app with:

```bash
bun install --frozen-lockfile
make dev
```

Build the qualification bundles with:

```bash
make build-tauri
```

Do not point `CEF_PATH` at an arbitrary empty directory. Upstream also treats
that variable as an already-extracted distribution and a malformed override
can bypass normal cache resolution.

## CI design

The normal `CI` workflow now treats CEF as the application runtime everywhere:

- frontend tests, lint, type checking, and production frontend build on Linux;
- Rust format and Clippy on Ubuntu 24.04 with GTK4/CEF prerequisites;
- Rust workspace tests on Ubuntu 24.04, macOS arm64, and Windows x64;
- release-mode CEF/CDP desktop E2E on Ubuntu 24.04, macOS arm64, and Windows
  x64.

Ubuntu 24.04 is required because Ubuntu 22.04's GTK4 package is too old for the
current `gdk4-sys` requirement. CEF distributions and Rust artifacts are cached
per operating system and lockfile. The Linux E2E lane verifies that
`chrome-sandbox` is a regular file, assigns it `root:root` ownership with mode
4755, and then runs the app under Xvfb.

The branch-scoped `CEF qualification` workflow builds unsigned installers on:

| Platform | Native runner | Formats |
| --- | --- | --- |
| Linux x64 | Ubuntu 24.04 | deb, RPM, AppImage |
| macOS arm64 | macOS 26 arm64 | app, DMG |
| macOS x64 | macOS 26 Intel | app, DMG |
| Windows x64 | Windows hosted | NSIS |

Each cell asserts the single-runtime dependency graph, records build time,
bundle-tree bytes, CEF cache size, and tool versions, and extracts every output
format to verify both the CEF license and `LICENSES.chromium.html`. The deb/RPM
cell additionally verifies that the packaged sandbox helper is owned by root
with mode 4755. Unsigned bundles are uploaded only after the notice audit
passes; qualification metrics are retained even when a later audit fails.

Tauri alpha.0 downloads `quick-sharun` unconditionally immediately before
running it, so a normal pre-bundle cache override is ineffective. The AppImage
cell accepts only the exact known `locales/` directory failure, verifies the
downloaded helper against a pinned upstream commit and SHA-256, adds a file
guard to its shell-script scan, pins the helper's moved `anylinux.c` dependency
to the same upstream commit, and resumes packaging from Tauri's prepared
AppDir. Any other initial failure, changed helper, failed extraction, or failed
notice audit remains red. This is a qualification workaround, not a permanent
fork of the Tauri bundler. Recovery keeps Tauri's partially prepared AppDir and
supplies its CEF and shared-library directories to the helper's linker scan.

## Evidence as of 2026-09-14

### macOS default-runtime proof

At commit `678bae34a31ad3f47d9e9e8e621eaf44bde9729f`, plain `make dev`
built and launched the qualification `.app` on macOS arm64. The application had
live CEF helper processes, contained the CEF framework and redistribution
notices, used the disposable identifier, and declared macOS 11 as its minimum.
`cargo check -p gitru` passed and `cargo tree` contained
`tauri-runtime-cef 3.0.0-alpha.0` with no Wry runtime.

### Hosted functional proof

Normal CI run
[34847389529](https://github.com/ruru-m07/gitru/actions/runs/34847389529)
passed all eight jobs at the earlier Tauri 3 qualification head. Its
release-mode CEF E2E covered Linux, macOS, and Windows, including repository
import, stage, commit, publish, stash/switch, rebase abort, recovery, and child
tab lifecycle. This is raw-runtime evidence; it does not prove installed,
signed, or updater behavior.

Local macOS and an earlier hosted run also observed nondeterministic managed
child-target/CDP disconnects after different milestones. A later hosted pass
does not close that stability concern. Keep the macOS E2E lane and collect
failure artifacts until repeated packaged runs establish an acceptable rate.

### Hosted packaging proof

Qualification run
[34847384933](https://github.com/ruru-m07/gitru/actions/runs/34847384933)
established the following:

- native macOS arm64 and Intel x64 app/DMG builds succeeded and passed the
  redistribution notice audit;
- the macOS x64 CEF bundle tree was 571,753,199 bytes and its DMG was
  172,566,901 bytes in that run;
- Linux deb and RPM packages contained the sandbox helper with root ownership
  and mode 4755 and included the required notices;
- Linux AppImage packaging failed in Tauri's `quick-sharun` path when it treated
  CEF's `locales` directory as a regular file;
- the Windows alpha runtime remains unsandboxed; later workflow fixes also made
  Windows select the complete Strawberry Perl runtime and parse CRLF lockfiles
  deterministically.

Historical runtime comparisons remain useful for sizing decisions, but the CI
workflow no longer builds a Wry baseline. The product decision is CEF-only and
new evidence should measure CEF against approved budgets rather than keep an
alternate runtime alive.

## Open release gates

CEF is now the development direction, but production publication remains
blocked until the applicable gates are closed or explicitly accepted:

| Gate | Current state | Required closure |
| --- | --- | --- |
| Linux AppImage | A pinned compatibility patch handles CEF's locales directory; hosted proof pending | Successful package, extraction, launch, sandbox, and notices proof |
| Windows sandbox | Unsupported by CEF alpha.0 | Upstream sandbox support or a separately approved security exception |
| macOS stability | Successful hosted runs plus nondeterministic child-target disconnects | Repeated packaged native runs and failure-rate acceptance |
| macOS distribution | Native arm64 and x64 packages work; universal CEF resolution is unsupported | Architecture-aware downloads/updates, or a proven universal packaging strategy |
| JavaScript API graph | Root v3 API plus eight plugin-nested v2 copies | Updated plugin metadata, frozen install, and plugin/API requalification |
| Signing and updates | Qualification bundles are unsigned and updater-disabled | Signed/notarized install, relaunch, update, failed-update recovery, and rollback on every shipped target |
| User profile migration | Qualification identifier is isolated | Tested production-identity transition with existing user state and a recovery plan |
| Cost and performance | Package sizes recorded; limits not approved | Approved installer, installed-size, update, cache, memory, and launch budgets with reproducible measurements |

## Promotion sequence

1. Keep CEF as the only runtime and keep normal CI green on all three operating
   systems.
2. Validate the AppImage compatibility patch and prove the installed Linux sandbox.
3. Resolve or explicitly accept the Windows sandbox limitation.
4. Update the v3 plugin graph and rerun product E2E.
5. Exercise signed/notarized installers, architecture-aware macOS delivery,
   updater failure recovery, and rollback against a staging feed.
6. Approve cost/performance budgets and record measurements.
7. In a dedicated promotion change, restore the production product name,
   identifier, analytics policy, signing, and updater configuration; keep the
   release workflow's identity guard until that review is complete.

Tracking issue: RURU-93.
