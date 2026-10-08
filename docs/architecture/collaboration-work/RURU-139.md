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

The implementation pins the current combined collaboration source at
`f0b8801dec569af24a1b1f67744600f85f4d90ac`, tree
`13a3bd59cbc1f99f048048d4e7c8107bdc71f6bf`, independently of this older R108 PR
base. Git archive extraction and every Git blob are checked; the encrypted probe
uses its **schema 22** migrations. Missing source objects are a hard failure.

macOS 27.0 arm64 / Rust 1.94.1 release probe passes with SQLCipher 4.19.0, SQLite 3.53.4,
CommonCrypto and the unchanged WAL gate. It passes all 22 migrations, STRICT /
JSON / FTS5, three simultaneous keyed read-only connections, rollback,
independent-reader checkpoint/WAL restart, keyed native `sqlite3_backup`, keyed
VACUUM backup, missing/wrong-key refusal, page-tamper refusal and abrupt process
exit/reopen. The executable links Security/CoreFoundation and no dynamic SQLite
or OpenSSL. `cipher_provider_version` returns `unknown` for CommonCrypto; the
actual OS/compiler/framework data is retained in the build report instead of
inventing a crypto version.

Five Python controls qualify hash mismatch, archive traversal/link refusal,
native override refusal, modified FFI build-script/extra-file refusal and pinned
collaboration-source drift. The release probe and strict isolated Clippy/fmt pass. A fresh automated local
source generation also reproduces the pinned C/header hashes before rebuilding
the probe; Linux regeneration remains a separate CI qualification.

The first unchanged engine regression ran 886 passing tests / 5 ignores before
four participant migration cases stopped at their old exact SQLite identity
assertion (`3.51.3`, actual `3.53.4`). These were not failed migration assertions.
Three copied test files contain such engine identity assertions:
`tests/participant_migrations.rs`, `tests/storage.rs`, and
`tests/task_migrations.rs`. The qualification now verifies each original file's
SHA-256 and unique old version literal before replacing only that literal with
exact `3.53.4`. It retains every migration/data/rollback assertion and all
production source bytes. This is the explicit exception to the initial
manifest-only adaptation plan; no tests are skipped and no broad version range
is substituted. The final locked, adapted engine regression passes **1,016 tests / 5 existing
ignored helpers across 27 suites**, with both verified native package paths
checked before execution. This includes frozen schema 1–21 upgrades, schema 22
recovery/proof validation, rollback/fault cases, and the test harness. The
separate regression lock records the current OpenSSL payload; it is not an
unreviewed runtime dependency resolution.

The compatible crates.io OpenSSL wrapper otherwise selects an older crypto
payload. A second isolated, verified patch retains `openssl-src` 300.6.1's build
logic and uses current **OpenSSL 3.6.5** source (commit
`c8bd5a57108599ac650bbae77fcabe3109dab2e8`, tag object
`f93be98fe7c796e92fc999dd1a21a5212d4e4f8e`). Only the native source tree and the
two manifest version metadata fields change; Cargo identifies the payload as
`300.6.1+3.6.5`. Its upstream archive hash, wrapper checksum and license are
preserved. The [29 September security release](https://openssl-library.org/news/timeline/)
motivates this current source selection; no vulnerability in Gitru's specific
CBC/HMAC usage has been demonstrated. Like SQLCipher's tag, GitHub reports this
tag signature as `unknown_key`; source hashes do not imply trusted GPG identity.

The new CI workflow independently regenerates the pinned amalgamation on Linux,
builds/runs the native probe and current engine regression on macOS, Windows and
Linux, and reads each producer's synthetic encrypted fixture on all three
platforms. It preserves source hashes, binary hash, compiler/OS/crypto/linkage
evidence and licenses as artifacts. These remote jobs have **not run yet**.

Still unqualified: keyed whole-app Store/vault startup and conversion (R140),
Tauri packaged encryption, portable user-key wrapping/recovery (R141), and the
R125 encrypted/current native IPC latency/memory comparison. The full regression
uses unkeyed Store against the cipher engine; it must not be described as those
encrypted application checks. R139 remains a partial qualification until its
remaining acceptance evidence exists. Production encryption is unchanged.

## First remote qualification and CRLF repair

PR #193 initial head `6a9bf824` independently reproduces the pinned native
C/header hashes on Linux (run `37726337548`, job `113145248437`). Windows stops
before native compilation because Git archive honors `core.autocrlf=true`,
changing the pinned Cargo.lock bytes. The blob gate correctly refuses them.
A local reproduction shows 8,224 CRLF lines under that setting versus an exact
blob with conversion disabled. Archive extraction now sets `core.autocrlf=false`
and `core.eol=lf` for that command only, preserving every hash/version guard and
the user's Git configuration. A real temporary Git repository with CRLF enabled
reproduces the old behavior and passes the corrected extraction; all six Python
controls pass. Platform/portability results remain pending the new exact head.

## Exact-source remote qualification — 8 October 2026

[Run 37726932167](https://github.com/ruru-m07/gitru/actions/runs/37726932167)
completed successfully for exact signed source
`61548a0d6daa48adf87a31a0e1ca4cc2c475e317`. All seven jobs pass:

- Linux source reproduction verifies the pinned SQLCipher amalgamation hashes.
- Native encrypted probe and copied current-engine regression pass on macOS 26,
  Ubuntu 22.04 and Windows. Platform-specific crypto/linkage reports and source,
  binary and license evidence are retained as workflow artifacts.
- Each platform reads every platform's produced encrypted synthetic fixture;
  all three cross-platform reader jobs pass, including Windows job
  `113158042003` (the final job to finish).

The ordinary PR #193 matrix also passes all 14 reported checks at that source.
These results qualify the isolated native build and synthetic file portability,
not production encrypted app startup. The copied engine regression remains
unkeyed Store against the qualified cipher engine. R140 keyed application/vault
integration and migration, Tauri packaging, R125 encrypted/current native IPC
latency and memory measurements, and R141 portable user-key recovery remain
unqualified. R139 therefore remains In Progress. The production safety gate,
application dependencies and plaintext behavior are unchanged; no merge.
