# RURU-139 — qualify a safe SQLCipher native build

## Contract before implementation — 8 October 2026

Baseline: signed R108 `486a936e700fe00b863ac26f6f0b526477574a5d`.
This work qualifies a replacement native source independently of production key
lifecycle. It never relaxes `Store::open_owned`'s existing WAL safety gate and
never opens real user databases or reads platform credentials.

The existing SQLx 0.9.0 / libsqlite3-sys 0.37.0 bundled SQLCipher 4.10.0 embeds
SQLite 3.50.4 and remains ineligible. The candidate is stable SQLCipher **4.19.0**,
upstream commit **c4b275a47932888216bade83aff2bbc73df0ff85**, whose `VERSION` is
**3.53.4**. SQLCipher 5 is a prerelease with changed crypto/file format and is
outside this qualification. This selection is based on actual upstream source,
not the SQLCipher version number as a proxy for SQLite safety.

## Reproducible build boundary

- Record upstream tag, exact commit, source archive SHA-256, generated C/header
  hashes, build flags, Rust binding checksum, crypto provider and licenses.
- Keep the existing libsqlite3-sys 0.37.0 build machinery and single Cargo
  `links = sqlite3` owner. A qualification-only local patch replaces that crate's
  bundled SQLCipher amalgamation/header with the verified upstream source. Do
  not change the application's dependency features, lockfile or startup gate.
- Build through a locked isolated Cargo workspace. Fail closed on source hash,
  expected native versions, encryption/crypto availability or WAL-gate mismatch.
  No fallback to a system SQLite or the known-bad bundled cipher is permitted.
- Prefer CommonCrypto/Security on macOS and pinned vendored OpenSSL on Windows
  and Linux, preserving the crate's supported build paths. A platform is only
  qualified after its actual native executable runs; cross-compilation alone
  is insufficient. Record static/dynamic linkage rather than infer it.

## Required evidence

1. Synthetic encrypted database and WAL, three simultaneous keyed read-only
   connections, wrong/missing-key refusal, strict tables, JSON, FTS5, every
   checked-out collaboration migration and rollback/reopen/integrity.
2. WAL checkpoint/restart with independent reader/writer connections; native
   SQLite backup API and VACUUM backup behavior with explicit synthetic keys.
   Preserve the unchanged safety gate even if a stress run happens to pass.
3. Cross-platform produced-file portability and packaged probe execution,
   source/linkage reports and reproducible CI artifacts. Distinguish local macOS
   evidence from pending remote Windows/Linux evidence.
4. Full collaboration compatibility against the qualified engine and measured
   encrypted/plaintext cached-query/memory deltas. Existing R125 native IPC
   measurements require keyed Store integration; do not relabel a standalone
   microbenchmark as that acceptance criterion.

R140 owns real key acquisition, keyed writer/read pool startup and recoverable
plaintext conversion. R141 owns portable backup key wrapping and user recovery.
This work may provide reusable verified build inputs, but does not claim those
features are implemented. Remaining qualification gaps stay explicit in Linear
and this document; an open reviewable PR is not a completed GA encryption gate.

## Primary references and provenance limits

- [SQLCipher 4.19 release](https://github.com/sqlcipher/sqlcipher/releases/tag/v4.19.0)
  includes maintenance/security fixes to `hexkey` and `sqlcipher_export`.
- [Pinned source](https://github.com/sqlcipher/sqlcipher/tree/c4b275a47932888216bade83aff2bbc73df0ff85),
  [build instructions and crypto prerequisites](https://github.com/sqlcipher/sqlcipher/blob/v4.19.0/README.md),
  [license](https://github.com/sqlcipher/sqlcipher/blob/v4.19.0/LICENSE.md).
- [SQLite WAL-reset bug and fixed versions](https://sqlite.org/wal.html#walresetbug).
- Upstream annotated tag object `58beb4a302f0e3c37341d2312cef521f858b1273`
  contains a PGP signature, but GitHub reports `unknown_key`. This qualification
  records HTTPS origin, commit and content hashes; it does not claim independent
  trust verification of the maintainer's signing key.

## Evidence ledger

Design recorded before implementation. No candidate build or platform pass is
claimed yet. Production encryption remains unchanged.
