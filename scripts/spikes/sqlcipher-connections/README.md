# Owned SQLCipher connections (R140 qualification)

This isolated workspace keys actual SQLCipher handles through the C API before
SQLx executes extensions or its first PRAGMA. It is an opt-in native component;
application Store, app startup, default cipher, recovery and vault selection are
unchanged. It uses synthetic temporary files and an in-memory key vault only.

## Reproduce

From the repository root, prepare the exact R139 native inputs, then the driver:

```sh
python3 scripts/spikes/sqlcipher-qualified/prepare.py
python3 scripts/spikes/sqlcipher-connections/prepare.py
python3 -m unittest discover -s scripts/spikes/sqlcipher-connections -p test_prepare.py -v
python3 scripts/spikes/sqlcipher-connections/run.py
```

Platforms without the pinned amalgamation's generator toolchain can pass the
R139 `--amalgamation-dir` option to its preparation step. The native files must
match the same committed hashes. CI generates them on Linux, then verifies the
bytes again on macOS, Linux and Windows. Preparation must finish before building
in that worktree. Build outputs and verified source copies remain under ignored
`target/`; `Cargo.lock` is committed. The runner rejects the inherited native
library/compiler-flag overrides and an external Cargo target. It verifies one
SQLite linkage owner and the exact driver/native/crypto manifest paths, then runs
formatting, strict Clippy and actual native tests. Its executable linkage and
input/lock/executable hashes are saved with test evidence. No source override,
stock bundled SQLCipher, system SQLite or unkeyed fallback is accepted.

## Driver boundary

SQLx0.9.0's public `after_connect` and `lock_handle` happen after its initialization
PRAGMAs. A secret `.pragma("key", ...)` would put the key into cloneable options
and SQL text. The four-file adaptation adds a native owner hook instead:

- `src/options/mod.rs`: an absent-by-default owner factory and an unsafe native
  initializer/lifecycle contract. Closure captures are never formatted.
- `src/connection/establish.rs`: copies only that redacted factory and adds
  `SQLITE_OPEN_NOFOLLOW` for opted-in opens.
- `src/connection/handle.rs`: reserves a bounded owner **before native open**,
  initializes immediately after a successful `sqlite3_open_v2`, and retains the
  owner through actual `sqlite3_close`, including worker cancellation/failure.
- `src/lib.rs`: exports the native owner contract for this isolated factory.

The original SQLx archive SHA-256, each input/output file hash and every unique
literal substitution are pinned in `driver-adaptation.json`. The readable patch
must reproduce those exact bytes. Preparation reuses R139's host-independent
archive path/type guard, then verifies the complete copied package (including
unmodified files, missing/extra files and symlinks). The readable patch has an
explicit LF checkout attribute. No upstream branch is fetched or patched.

The factory has eight global native reservations, including pool replacements;
each reader pool is independently capped at three. Failed native close marks the
factory permanently faulted before SQLx reports the worker failure. That handle's
owner is deliberately retained until process exit. Later opens, including pool
replacements, cannot accumulate additional failed handles/keys. The isolated
child control actually forces `SQLITE_BUSY` with an unfinalized native statement,
then proves the retained owner, refused later opens and busy OS lease. This is
exceptional fault containment, not successful recovery or key erasure.

## Native API and qualified scope

`KeyedConnectionFactory::new(DatabaseKeySession)` consumes the existing owned
reservation/key/OS lease. `open(HandleRole)` supports Writer, Reader, Maintenance,
Recovery and Verifier; the latter two are existing-file read-only handles here.
`reader_pool()` authenticates every new/replacement read-only handle. Only an
explicit CreateNew session may create a writer file, once per factory; later
writer opens cannot recreate a removed database. Each key call uses the same
32 binary passphrase bytes with the pinned SQLCipher KDF; it is not raw-key mode.
No key is placed in SQL, URLs, trace output, process arguments, environment or
serializable types. The factory keeps no second Rust key copy.

Every returned handle has read the encrypted schema and checked cipher_status,
exact SQLite source/version, SQLCipher/provider identity, FTS5 and fixed
HMAC/header/page/KDF settings. The exact SQLite3.53.4 profile satisfies the
unchanged application WAL safety gate. Wrong-key/plaintext failures retain files;
no attempt publishes Ready. The fixed Rust key zeroizes when the last successful
native owner drops. SQLCipher's internal key-buffer lifetime is its own native
implementation; Rust does not claim to prove erasure of those copies.

Tests exercise all roles, three simultaneous readers and replacement, read-only
write refusal, cold keyed reopen, wrong/no key and plaintext refusal, encrypted
DB/WAL canaries, initialization before first traced SQL, cancelled initialization,
cancelled pool close with a held reader, the eight-handle bound and actual failed
native close. Unix additionally tests a replaced final-path symlink; this is not
a claim of Windows reparse-point qualification. Synthetic callbacks use explicit
worker gates and OS lease observations, not wall-clock ordering as causal proof.

This component does **not** migrate application schemas or establish the missing
encrypted app database identity, own a whole Store shutdown/drain sequence, select
a personal/platform vault, activate plaintext conversion, rotate keys or create
portable encrypted backups. R139's copied schema22 engine tests remain separate;
no keyed current schema24/whole-app/Tauri/performance result is claimed. Those
follow after the final qualified schema and reviewed Store/activation integration.
