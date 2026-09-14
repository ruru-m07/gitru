# Tauri 3 and CEF qualification

> **DO NOT MERGE THIS SPIKE.** This worktree and branch are a disposable Tauri
> 3/CEF qualification environment. The `dev` branch and every production build
> remain on Tauri 2 and Wry. Port only independently reviewed follow-up changes;
> never merge the spike wholesale.

## Current decision

Gitru's production release remains on Tauri 2 and the Wry runtime. Tauri 3 and
its CEF runtime are alpha software, and a successful compile or package job is
not a decision to ship CEF.

The repository supports two mutually exclusive desktop runtime features:

| Build | Cargo feature selection | Configuration | Purpose |
| --- | --- | --- | --- |
| Spike baseline | `qualification,wry` with defaults disabled | `tauri.wry-qualification.conf.json` | Isolated Tauri 3/Wry comparison inside this disposable branch only |
| Qualification | `qualification,cef` with defaults disabled | `tauri.cef.conf.json` | Isolated Tauri 3/CEF evaluation only |

This table describes the Rust/Cargo runtime graph, not a pure-v3 JavaScript
graph. Every published Tauri plugin `3.0.0-alpha.0` used by Gitru currently
declares `@tauri-apps/api: ^2.11.0`, so the frozen lockfile contains eight
plugin-nested `@tauri-apps/api@2.11.1` copies alongside the root
`@tauri-apps/api@3.0.0-alpha.0`. Do not force an override: upstream must publish
v3-compatible plugin metadata, after which the bridge and plugin calls need to
be requalified.

The base configuration and both qualification overlays use the separate
`com.ruru.gitru.cef-qualification` identifier, disable updater checks and
updater artifact creation, and raise only the experimental macOS bundle minimum
to 11.0. The default Cargo feature set is also `qualification,wry`, the base
development command uses Vite's `qualification` mode, and `make dev` and
`make build-tauri` pass the Wry qualification overlay explicitly. A bare Tauri
command in this branch therefore cannot replace or mutate a user's production
Gitru profile. Both runtime-specific overlays select the Cargo
`qualification` feature and the compile-time `qualification` frontend mode. The
feature rejects non-qualification identities and updater commands; the frontend
mode suppresses sidebar update checks and does not mount the PostHog provider.
The v3 alpha updater schema no longer recognizes Tauri 2's
`plugins.updater.active` switch, so the overlays deliberately do not present
that obsolete field as a safety control. The alpha plugin nevertheless requires
a non-null configuration object when it initializes. The base therefore gives
it an empty endpoint list and empty public key; a Rust regression test
deserializes that exact object, checks every dangerous transport option, and
proves it remains inert.
The static frontend bundle can still contain PostHog package strings, so both
overlays also remove PostHog from `connect-src` while preserving the GitHub and
Bitbucket image hosts needed for product-parity avatar rendering. Do not use
production signing, notarization, updater, analytics, or release-upload
credentials in a qualification build.

The production release workflow now fails before verification or signing unless
the checked-out base configuration has the exact `com.ruru.gitru` production
identifier. This branch deliberately does not, so even an accidentally
published tag cannot upload its qualification artifacts. The spike's manifest
and toolchain are not a release migration plan.

## Outcome: NOT YET

The current migration decision is **NOT YET**. The spike can build isolated
Tauri 3 runtime graphs and exercise CEF through CDP, but that is not enough to
approve production migration. Windows sandboxing, a universal macOS delivery
strategy, AppImage sandbox behavior, the mixed Tauri 2/3 JavaScript API graph,
installed and signed update/rollback coverage, complete product-parity evidence,
deterministic command type generation, and approved cost limits remain
unresolved. Any one of those hard gates is sufficient to keep the result at
NOT YET.

## Local build

Install the normal Tauri prerequisites first. CEF additionally needs about 1 GB
of cache space on the first build and needs CMake plus Ninja on Windows and
macOS. Linux builds need GTK4 and `libxdo`; a local Wry comparison still needs
GTK3 and WebKitGTK.

From the repository root, install the pinned dependencies and build the
architecture-native qualification app:

```bash
bun install --frozen-lockfile
bun --cwd=apps/desktop run tauri build --config src-tauri/tauri.cef.conf.json --features qualification,cef -- --no-default-features
```

The `--` is significant: Tauri 3 alpha has no top-level
`--no-default-features` option, so the flag after it is passed to Cargo. Omitting
it enables the default Wry runtime as well, which intentionally fails Gitru's
mutual-exclusion compile guard.

To make a comparable Wry bundle with the same isolated identity and packaging
settings:

```bash
bun --cwd=apps/desktop run tauri build --config src-tauri/tauri.wry-qualification.conf.json --features qualification,wry -- --no-default-features
```

CEF uses the upstream cache locations by default:

- Linux: `~/.cache/tauri-cef`
- macOS: `~/Library/Caches/tauri-cef`
- Windows: `%LOCALAPPDATA%\tauri-cef`

Do not set `CEF_PATH` to an arbitrary empty directory. The variable can also
mean an already extracted CEF distribution, so a malformed override can bypass
the CLI's normal download/cache resolution.

## Branch qualification CI

On this disposable branch, the normal CI workflow runs the release-mode CEF/CDP E2E
suite as a required Linux, macOS, and Windows parity gate. It caches each
platform's CEF distribution and rejects dependency graphs that contain Tauri 2,
Wry, GTK3, or WebKitGTK. The `dev` branch keeps its existing Tauri 2/Wry E2E
path. The branch qualification workflow adds full installer and cost
comparison; it does not replace the branch's required CEF E2E job. The E2E job launches a macOS
`.app` bundle but raw release binaries on Linux and Windows; it does not cover
installer layout or installed sandbox behavior.

The `CEF qualification` workflow runs only for pushes to the disposable
`ruru/ruru-93-tauri-3-cef-qualification` branch. GitHub does not dispatch a
manual-only workflow that exists only outside the default branch, so this spike
uses the exact branch trigger instead of advertising an inoperable manual path.
The workflow is read-only with respect to the repository and never receives
release secrets. Metrics remain available for 30 days. Unsigned bundles are
uploaded for seven days only after the redistribution notice audit passes.

Each job first proves that its Cargo graph contains exactly one runtime. It then
builds Wry and CEF from the same revision with parallel isolated overlays on:

- Linux x64 deb/RPM and Linux x64 AppImage as independent cells
- macOS arm64 (`.app` and DMG)
- macOS x64 (`.app` and DMG)
- Windows x64 (NSIS)

The macOS targets use architecture-native hosted runners: `macos-26` for arm64
and `macos-26-intel` for x64. This keeps the bundle and DMG comparison from
silently becoming a cross-compilation test.

The jobs cache the platform-specific CEF distribution, record cold/warm cache
state indirectly through the Actions cache result, and attach build duration,
bundle-tree byte counts, tool versions, and CEF cache size. Linux also rejects
deb or RPM output unless each package carries `chrome-sandbox` owned by
`root:root` with mode 4755.
The CEF lanes use Ubuntu 24.04 because its GTK 4.14 packages satisfy the
`gdk4-sys` requirement. Ubuntu 22.04 supplies GTK 4.6 and fails before the
application can compile, so it is not a valid CEF qualification runner.
Wry and CEF compile into separate Cargo target directories so one runtime cannot
reuse the other's compiled artifacts and distort the build-time comparison.
Every CEF installer is unpacked and audited for the CEF license text and the
non-empty `LICENSES.chromium.html` Chromium credits file. That audit fails
closed: until both notices are actually inside every format, the corresponding
qualification job remains failed and the artifact is not eligible for
redistribution.

The CEF overlay is the only configuration that bundles these notices. It ships
the committed upstream CEF BSD `LICENSE.txt`; immediately before bundling, its
hook locates the exact CEF distribution selected by Cargo, copies that
distribution's generated `CREDITS.html` to the ignored
`LICENSES.chromium.html`, and verifies the copy by SHA-256. A missing cache,
unexpected license, truncated credits file, or changed checksum stops the build.

These architecture-specific macOS jobs are intentional. Do not change them to
`universal-apple-darwin` until upstream target resolution accepts a universal
CEF distribution and the resulting framework plus all five helper apps pass
signature and launch checks.

## Non-negotiable platform gates

| Area | Alpha state | Production gate |
| --- | --- | --- |
| Windows sandbox | CEF is unsandboxed regardless of policy; `Required` refuses to start | Upstream sandbox support, or a separately documented security acceptance, is required |
| macOS architecture | Individual arm64 and x64 targets are available; the embedded CEF resolver rejects `universal-apple-darwin` | Preserve a proven universal artifact or approve and prove architecture-specific update routing |
| macOS support floor | Production Wry config advertises macOS 10.13; the qualification overlay uses 11.0 | Test the oldest declared OS and explicitly approve any support-floor increase |
| Linux deb/RPM sandbox | Packages can install the setuid sandbox helper with mode 4755 | Inspect package metadata and verify installed ownership, mode, and sandbox activation |
| Linux AppImage sandbox | AppImage cannot supply a setuid helper and can fall back to unsandboxed mode when user namespaces are restricted | Fail closed, stop shipping AppImage, or obtain an explicit security acceptance after real-host tests |
| CEF notices | Binary redistribution has CEF and Chromium notice obligations | Required notices and third-party credits must be present in every installer and installed app |
| Tauri JavaScript graph | Root API is v3 alpha, but eight plugin-nested API copies resolve to 2.11.1 because the v3-alpha plugin packages declare `^2.11.0` | Wait for corrected upstream plugin metadata; do not force an override; rerun bridge and plugin coverage on a single v3 graph |
| Command type generation | `make typegen` succeeds and both observed runs expose the same 70 commands and 246 exported type symbols, but consecutive tauri-typegen 0.4.2 runs reorder declarations and change cache hashes | Fix or upgrade the generator before accepting the deterministic-generation gate; do not commit reorder-only output when Rust command signatures are unchanged |
| Qualification network isolation | Both overlays select qualification-only backend and frontend paths; custom updater commands reject the build identity, sidebar checks are inert, the PostHog provider is skipped, and CSP blocks its hosts | Keep the config/feature/CSP assertions and backend identity tests green; prove no updater or analytics requests with traffic capture before wider distribution |

The workflow verifies build-time facts only. Sandbox activation, signatures,
notarization, updater behavior, and application interaction require installed,
production-equivalent artifacts on real hosts.

## Product and runtime evidence matrix

Record one of `pass`, `fail`, or `unknown` for every cell and link the log,
screenshot, trace, or artifact used as evidence. A portable Tauri API working on
Wry is not evidence that it works on CEF.

| Scenario | Linux deb/RPM | Linux AppImage | macOS arm64 | macOS x64 | Windows x64 |
| --- | --- | --- | --- | --- | --- |
| Install, first launch, quit, and relaunch | Unknown | Unknown | Unknown | Unknown | Unknown |
| Repository import and complete Git critical flow | **Pass (raw CEF only; package Unknown)** | **Pass (raw CEF only; package Unknown)** | **Fail (2026-09-14)** | Unknown | **Pass (raw CEF only; package Unknown)** |
| Concurrent child-tab prewarm/show/hide/focus/resize/close | Unknown | Unknown | Unknown | Unknown | Unknown |
| Child-tab failure recovery and repository switching | Unknown | Unknown | **Fail (2026-09-14)** | Unknown | Unknown |
| CSP, IPC, asset URLs, external navigation, and capability scope | Unknown | Unknown | Unknown | Unknown | Unknown |
| Drag region, double-click maximize, traffic lights, and geometry restore | Unknown | Unknown | Unknown | Unknown | Unknown |
| Menus, tray, dialogs, notifications, opener, process, and clipboard | Unknown | Unknown | Unknown | Unknown | Unknown |
| DevTools open/close without IPC loss | Unknown | Unknown | Unknown | Unknown | Unknown |
| Existing-user state/profile migration and Wry rollback | Unknown | Unknown | Unknown | Unknown | Unknown |
| Signed update, relaunch, failed-update recovery, and rollback | Unknown | Unknown | Unknown | Unknown | Unknown |
| Sandbox active under the approved policy | Unknown | Unknown | Unknown | Unknown | Fail in alpha.0 |
| Required CEF/Chromium notices present | Unknown | Unknown | **Pass (unsigned app/DMG only)** | Unknown | Unknown |

### Local macOS arm64 evidence — 2026-09-14

The release-mode CEF/CDP smoke is a repeatable failure on the local macOS arm64
host, so both the complete critical Git flow and child-target recovery cells
above are **Fail**, not Unknown:

- Run `2026-09-14T10-11-52-144Z-7863` completed repository import, stage,
  commit, second-tab lifecycle, publish, stash/switch, and rebase abort. The
  child CDP socket then closed while the target inventory continued to report
  the same target ID (`827EB2E28FFD45928959E70FBDDF314A`) as a stale blank
  target, and the harness timed out waiting to leave detached-rebase state.
- Run `2026-09-14T10-17-37-811Z-10127` completed import, stage, and commit, then
  failed earlier while waiting for Push to Origin. Again the child socket was
  disconnected while the same target ID
  (`C29AB04737F736523E6A799D5BCAFEDA`) remained in the target inventory as a
  stale blank target.
- Run `2026-09-14T13-04-26-487Z-28949`, rebuilt after restoring the alpha
  updater's required inert configuration object, launched without a plugin
  initialization error, reached the ninth rebase-conflict milestone, and
  invoked rebase abort. The active child then disconnected and the harness
  again timed out waiting to leave detached-rebase state.

The local evidence directories under `artifacts/e2e/<run-id>/` contain each
`runner-error.txt`, `targets-on-failure.json`, `failure-1.png`, frontend
diagnostics, CEF/app logs, Git state, and milestone screenshots. These runs prove
that already-completed actions can work; they do not prove the complete flow or
recovery because the child target becomes unusable at nondeterministic stages.

A separate fresh, unsigned macOS arm64 `app` build completed with the
qualification identity. Its main app and all five helper app plists declare
`LSMinimumSystemVersion=11.0`; the embedded CEF framework plist has no such key.
The app contains the committed 1,662-byte CEF license and a byte-identical
19,655,009-byte Chromium credits file (SHA-256
`2e3fdc646f012691a01ac49f07a3e8649f4c852ce22d99a8cf13a96d6e733dc8`).
Because the build intentionally used `--no-sign`, `codesign --verify --deep
--strict` fails; no local DMG or signed/notarized artifact was proven. The
hosted evidence below later covers notices in an unsigned app and DMG, but all
signing-related cells remain Unknown.

A fresh Wry qualification `app` also completed in a separate cold Cargo target
directory. Summing regular file bytes gives 33,014,429 bytes (31.49 MiB) for
Wry and 388,820,746 bytes (370.81 MiB) for CEF, including the 19,656,671 bytes
of bundled CEF and Chromium notices. These unsigned app-tree observations are
not installer, installed-size, updater, or threshold results and therefore do
not change the pending cost table below.

### Hosted branch evidence — 2026-09-14

The pre-isolation revision, `69053ed6`, ran in normal CI
[34841415287](https://github.com/ruru-m07/gitru/actions/runs/34841415287):

- Ubuntu 24.04, Windows, and macOS arm64 each passed the complete ten-milestone
  CEF/CDP flow using CEF/Chromium 151.0.7922.174. The Linux raw release binary
  ran beneath a 24-bit Xvfb display only after CI verified a regular
  `chrome-sandbox` file and assigned it `root:root` ownership with mode 4755.
  This remains raw-runtime evidence, not proof of installed package behavior.
- The fresh macOS pass followed both a hosted failure and two local failures in
  which the managed child target became unusable at different milestones. It
  therefore confirms nondeterminism; it does not close the macOS stability gate.
- The alpha still runs unsandboxed on Windows, so the functional pass does not
  satisfy the production security gate or prove the NSIS package.

The packaging workflow
[34841411387](https://github.com/ruru-m07/gitru/actions/runs/34841411387)
produced these additional observations:

- macOS arm64 passed both isolated runtime builds and both notice audits. With
  a warm CEF cache, Wry took 125 seconds and 51,803,726 bundle-tree bytes; CEF
  took 178 seconds and 553,880,009 bytes. The CEF cache occupied 488,435,385
  bytes. The unsigned Wry and CEF DMGs were 18,784,933 and 165,056,163 bytes,
  respectively. These artifacts were not installed or signed, and no cost
  limit was approved in advance.
- The isolated Linux deb/RPM cell built both CEF packages. Their extracted
  metadata shows `chrome-sandbox` with mode 4755 and root ownership: RPM emits
  `root root`, while `dpkg-deb --contents` represents root numerically as `0/0`.
  The initial gate accepted only `root/root` and produced a false negative; the
  corrected gate accepts both valid dpkg representations without weakening the
  mode or ownership requirement.
- The independent Linux AppImage cell still fails when Tauri's
  `quick-sharun` path treats CEF's `locales` directory like a file. This remains
  a real distribution blocker and is not bypassed by the workflow.
- The Windows setup validated Strawberry Perl and its required module in
  PowerShell, but the subsequent Git Bash build selected `/usr/bin/perl` and
  stopped in vendored OpenSSL. The corrected workflow records the validated
  directory and forces each isolated build shell to select that exact runtime.
- The architecture-native `macos-26-intel` cell and corrected Windows package
  cell still require fresh evidence after these harness changes.

The first fresh E2E run after the fail-closed base changes exposed another alpha
contract: omitting `plugins.updater` makes plugin initialization deserialize a
`null` value and panic before the host is usable. The base now retains the
minimal inert updater object described above, and the focused regression test
catches a missing, null, malformed, or network-enabled replacement before an
expensive packaged run.

The fail-closed identity changes and workflow corrections require a fresh run;
none weakens a product or security gate. The AppImage packaging failure and the
macOS managed-child instability remain migration blockers even if the corrected
infrastructure lanes turn green.

## Cost measurement record

The decision owner must fill and approve every limit in the third column before
the timed run. Keep the machine type, power state, sample count, repository
fixture, tab count, build-cache state, and exact dependency versions constant.
Use medians and p95 values where applicable; do not compare an unsigned debug
CEF build with a signed production Wry release.

| Metric | Wry baseline | Approved limit set before run | CEF result | Decision/evidence |
| --- | ---: | ---: | ---: | --- |
| Installer bytes, per platform/architecture | Pending | Set before run | Pending | Unknown |
| Installed bytes | Pending | Set before run | Pending | Unknown |
| Full updater payload bytes | Pending | Set before run | Pending | Unknown |
| Clean build download bytes and wall time | Pending | Set before run | Pending | Unknown |
| Warm CI build wall time and cache bytes | Pending | Set before run | Pending | Unknown |
| Cold launch to interactive, median/p95 | Pending | Set before run | Pending | Unknown |
| Warm launch to interactive, median/p95 | Pending | Set before run | Pending | Unknown |
| Idle RSS/process count/CPU/GPU | Pending | Set before run | Pending | Unknown |
| Representative multi-tab RSS/process count/CPU/GPU | Pending | Set before run | Pending | Unknown |
| Update download/install/relaunch wall time | Pending | Set before run | Pending | Unknown |

The CI `metrics.tsv` files seed the bundle, build-time, and cache rows. Installed
size, runtime performance, update cost, and interaction evidence must be
measured separately with signed production-equivalent candidates.

## Decision rule and rollback

CEF is a **go** only when every non-negotiable gate passes, every supported
artifact has complete product evidence, and all approved cost limits pass. A
compile-only result, a single operating system, or an accepted renderer visual
check is insufficient. Any unresolved hard gate makes the decision
**not yet**.

Until a go decision and controlled rollout are approved:

1. keep production on Tauri 2/Wry and treat this entire branch as disposable;
2. keep production identifiers, profiles, updater endpoints, and manifests
   unchanged;
3. never publish qualification artifacts to stable or beta channels;
4. preserve the last signed Wry installer and updater manifest for rollback;
5. re-run update and rollback tests after any Tauri, CEF, Chromium, plugin, or
   target-strategy change.

Primary upstream references:

- [Tauri 3.0.0-alpha.0 release](https://github.com/tauri-apps/tauri/releases/tag/tauri-v3.0.0-alpha.0)
- [Tauri 2 to 3 alpha migration guide](https://github.com/tauri-apps/tauri-docs/blob/v3/src/content/docs/start/migrate/from-tauri-2.mdx)
- [CEF runtime guide](https://github.com/tauri-apps/tauri-docs/blob/v3/src/content/docs/develop/cef.mdx)
- [RURU-93](https://linear.app/catra/issue/RURU-93/qualify-tauri-3-cef-migration-for-gitru-production)
