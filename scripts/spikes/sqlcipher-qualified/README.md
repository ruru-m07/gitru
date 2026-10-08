# Qualified-source SQLCipher experiment (RURU-139)

This isolated workspace evaluates SQLCipher **4.19.0 / SQLite 3.53.4** using
SQLx **0.9.0** and the existing **libsqlite3-sys 0.37.0** build machinery. It
does not enable encryption in the desktop app, change its lockfile, weaken its
WAL gate, read credentials or open real user databases. Every key and database is
a disposable synthetic fixture. The earlier `sqlcipher-compat` directory retains
the negative SQLCipher 4.10.0 / SQLite 3.50.4 result.

## Run

Use Python 3.9+, the repository's pinned Rust toolchain, a C compiler and `make`.
Run from the repository root:

```sh
python3 scripts/spikes/sqlcipher-qualified/prepare.py
python3 scripts/spikes/sqlcipher-qualified/run.py
python3 scripts/spikes/sqlcipher-qualified/regression.py
```

`prepare.py` downloads content-hashed upstream and crates.io archives into its
ignored `target`, generates SQLCipher's amalgamation, and checks exact C/header
hashes. It verifies every Rust FFI source file against the pinned crate archive;
only the two bundled SQLCipher native files change. It also extracts the exact
current collaboration commit/tree in `pins.json` with Git and verifies every
file against its Git blob. Fetch that commit if the checkout is shallow; CI uses
`fetch-depth: 0`. Missing objects fail instead of falling back to old migrations.

Windows consumes the same generated C/header artifact with
`prepare.py --amalgamation-dir <directory>`; every consumer verifies the pinned
hashes. The CI source job regenerates these files independently on Linux. MSVC
developer tools and Perl are required for the Windows vendored crypto build.

The native probe requires exactly the selected SQLite/cipher version, crypto
provider, one Cargo SQLite link owner and no dynamic SQLite/OpenSSL linkage.
It asserts the unchanged application WAL gate, cipher format defaults, all 22
current migrations, STRICT/JSON/FTS, three simultaneous keyed read-only
connections, transaction rollback, independent snapshot/checkpoint/WAL restart,
native encrypted backup, VACUUM backup, missing/wrong-key rejection, page-tamper
rejection and abrupt child-process recovery. It exports a synthetic encrypted
file. CI reads all three producer files on each of the three platforms.

The regression command copies byte-identical current collaboration production
sources, tests and migrations to an ignored standalone Cargo workspace. That copy's
manifest enables the cipher and verified patch. Three hash-verified test files
have one intentional adaptation each: an exact `3.51.3` engine-identity assertion
becomes exact `3.53.4`. All data/migration/rollback assertions and the production
WAL gate are unchanged; no test is skipped or changed to a broad version range.
Its separate committed
`regression.lock` keeps the native compatibility run reproducible. It runs the
complete `test-harness` suite. The application's Store remains **unkeyed** in
these tests: this is engine compatibility, not proof of encrypted Store/key-vault
lifecycle. `--freeze-lock` is a maintainer operation requiring lockfile review.

Do not run preparation concurrently with builds in the same worktree. SQLite /
OpenSSL selection overrides, compiler-flag overrides and external `CARGO_TARGET_DIR` are rejected
by the probe runner. Logs, generated sources, fixture files and executables stay
under this isolated directory's ignored `target`.

## Crypto, source and license provenance

macOS uses the platform CommonCrypto implementation through Security and
CoreFoundation; the actual executable's linked frameworks are recorded.
Linux/Windows use statically built **OpenSSL 3.6.5**, not the stale 3.6.3 payload
of the newest compatible `openssl-src` 300 crate. A second content-verified local
patch preserves that crate's build logic, replaces only its `openssl/` source
tree with upstream 3.6.5, and changes the manifest version metadata to
`300.6.1+3.6.5` to identify the real payload. Both probe and regression locks use
this same patch. This is a qualification input, not a published/forked crate.

`pins.json` records source commits, archive SHA-256 checksums and generated-file
checksums. SQLCipher's BSD-style notice and OpenSSL's Apache-2.0 license are
retained in the verified inputs and uploaded with CI evidence; SQLite itself
is public domain. The unchanged Rust wrappers retain their upstream notices.
Annotated upstream tags contain signatures, but GitHub reports `unknown_key`;
this experiment makes no independent maintainer-key trust claim.

Primary sources: [SQLCipher release](https://github.com/sqlcipher/sqlcipher/releases/tag/v4.19.0),
[build requirements](https://github.com/sqlcipher/sqlcipher/blob/v4.19.0/README.md),
[OpenSSL 3.6.5](https://github.com/openssl/openssl/releases/tag/openssl-3.6.5),
[OpenSSL security release timeline](https://openssl-library.org/news/timeline/),
[SQLite WAL-reset fixes](https://sqlite.org/wal.html#walresetbug).

## What a pass means

See [RURU-139 evidence](../../../docs/architecture/collaboration-work/RURU-139.md).
Local macOS results do not imply Windows/Linux CI has passed. A portable synthetic
file is not portable user-key recovery. Packaged native probe execution is not
Tauri packaging qualification. R140 still owns keyed Store/pool initialization,
vault keys and conversion; R141 owns encrypted backup wrapping/recovery. R125's
encrypted-vs-current native IPC latency and memory comparison remains pending
that keyed application integration. No upstream advertised performance estimate
is substituted for a measurement.
