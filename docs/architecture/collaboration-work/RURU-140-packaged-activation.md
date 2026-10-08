# RURU-140 bounded slice: packaged keyed startup activation

Pre-code contract recorded on 2026-10-08. This slice wires an already activated,
verified keyed database into actual desktop startup and lets the retained R125
performance protocol choose a synthetic keyed control before runtime creation.
It does not expose a renderer encryption switch, convert plaintext, rotate keys,
delete vault entries, or make portable backups.

## Startup contract

- Default builds and databases remain plaintext. `native-keyed-storage` is an
  explicit build feature, and ordinary Cargo resolution remains on the existing
  SQLite driver.
- In that build, any key metadata evidence selects the keyed path. Invalid,
  pending, missing-key, wrong-key, identity-mismatched, or corrupt evidence is
  preserved and startup fails closed; it never falls back through `Store::open`.
- Production startup can only load an existing entry in the stable native
  database-key service. It cannot create or replace a key. The key is decoded in
  a zeroizing native buffer, passed into `DatabaseKeySession`, and retained by
  the qualified factory until every SQLite handle closes.
- Provider credentials and database keys keep separate vault traits and service
  namespaces. No database key enters IPC, environment variables, command-line
  arguments, logs, serde values, SQL, or cloneable SQLx options.
- The qualified factory source now lives in `crates/keyed-connections`. The R139
  isolated qualification workspace compiles that exact file, so app wiring does
  not fork the native owner implementation.

## Retained performance selection

`performance-run.ts` accepts only `GITRU_COLLABORATION_STORAGE_MODE=plaintext`
or `keyed`, writes that finite choice into the private launch-owned `run.json`,
and records it in the final report. Rust validates and freezes the marker before
opening `HarnessSession`. The normal IPC protocol cannot change it. A keyed run
uses a generated synthetic key in a private task-owned 0600 file; it never opens
a personal keychain. Both seed and cold restart use the same encrypted Store and
unchanged 10,000-item/native-IPC measurement protocol.

The keyed build wrapper regenerates and verifies the pinned R139 SQLCipher and
four-file SQLx adaptation, injects them with an ephemeral Cargo patch, builds the
actual Tauri harness with both explicit features, then restores the ordinary
workspace lockfile byte-for-byte. This avoids silently changing normal builds or
accepting a stock/system SQLCipher fallback.

## Validation record

Local validation on macOS: patched production desktop compile passed; patched
retained keyed desktop tests passed 16/16; unchanged plaintext retained tests
passed 16/16; startup metadata selection regression passed; Node native-process
ownership suite passed 49 with one existing skip; desktop and E2E TypeScript
typechecks passed; Biome, rustfmt and diff checks passed. The full packaged
seed/restart measurement, cross-platform native builds, and exact-head remote CI
remain separate evidence and are not inferred from these local checks.
