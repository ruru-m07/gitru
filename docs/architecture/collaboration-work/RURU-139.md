# RURU-139 — qualify a safe SQLCipher native build

## CI cold-build allowance — 8 October 2026

At signed head `f7b0a7ab6ff212eace1dcd619cbb123ba656298c`, ordinary CI
run `37746049701`, job `113207674587`, hit its 30-minute job limit.
Formatting and the normal workspace Clippy step passed. The retained-harness
Clippy step was still compiling dependencies when the runner cancelled it;
its log contains no lint failure. The outer Rust quality allowance is now
45 minutes. Both strict Clippy commands and all validation steps remain
unchanged. This is a job-budget correction; the replacement head still
requires remote completion before that check can be called green.

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

### Later Windows fixture failure — 8 October 2026

The docs-only head `5016fea9` reran qualification in run `37732404683`.
Windows job `113164424399` passed the actual encrypted probe, then failed the
copied unkeyed regression's
`unknown_write_reconciles_after_cold_restart_with_get_only` control. Its fixture
inherited a nonblocking accepted stream and assumed a single read contained an
entire HTTP request. Windows returned `WouldBlock` (10035); the operation then
correctly remained Unknown rather than falsely confirming. This is separate from
the earlier fully successful run and is not an observed SQLCipher failure.

The qualification runner now applies one explicit test-only repair to the pinned
`f0b8801` source copy. The accepted stream becomes blocking, complete headers/body
are read under one eight-second accept/request deadline and an 8 KiB bound, and
the response write also stays bounded. No provider implementation or operation
assertion changes. In particular, cold reconciliation still must use only GET
and must confirm from the exact provider observation without repeating a write.
The newer application stack already fixed the same socket-inheritance assumption
in `ffc575b6`; this adaptation preserves the deliberately frozen source baseline.

`pins.json` records SHA-256 for the original fixture, standalone replacement helper
and entire patched result. Python controls reject source/helper drift or an
unexpected result before writing. The native helper tests fragmented headers and
bodies, truncated requests, oversized declared bodies, stalled peers and expired
deadlines. The three exact-version fixture substitutions remain separately
enumerated; production source, all migrations and the hard WAL gate are unchanged.

Local repair qualification: **7 Python source-verification controls**, **3
standalone HTTP controls**, and **16 copied-engine inbox tests including those
three controls** all pass. The real previously failing cold-reconciliation case
passes with its original assertions. Strict all-target copied-engine Clippy,
Python compilation, helper Rust formatting, source hash verification and diff
checks pass. Logs: `/tmp/gitru-r139-http-source-tests.log`,
`/tmp/gitru-r139-http-native-tests.log` and `/tmp/gitru-r139-http-clippy.log`.
These are local macOS results; the repaired Windows full matrix and cross-file
reader jobs require a new remote run. No full Windows success is inferred from
this focused local repair.

### Windows helper checkout provenance repair — 8 October 2026

Exact head `71c7722206c0a8e94535d88da619d68e9942078a` ran qualification in
[run 37737457140](https://github.com/ruru-m07/gitru/actions/runs/37737457140).
Windows job `113180340692` reports `NATIVE_PROBE_PASS`, including SQLCipher
4.19.0 / SQLite 3.53.4 / OpenSSL 3.6.5, the unchanged WAL gate, keyed readers,
schema22, backup/reopen and page-tamper controls. The later regression preparation
failed before any copied tests because the checkout converted the new helper to
CRLF. Its observed SHA-256 `bd9493613ddce50a723f8df99b01103aa808a4d469ca09ef78111070ea34e62a`
is exactly the CRLF transform of the pinned LF helper hash
`a9504954339b4f45cc80ce8eb5620fa5bb300a25a9958e3c86a9a2b8b6062134`.
This is a rejected source-provenance input, not an encrypted probe failure.

One path-specific Git attribute now pins that helper's checkout to LF. The
runner's strict helper/source/result hashes, all fixture bytes, source pins,
crypto dependencies and native code are unchanged. An actual temporary Git
repository with `core.autocrlf=true` reproduces the failing byte mismatch before
the attribute and passes after it. It verifies that unrelated text still becomes
CRLF, the helper hash and full adapted result stay exact, and a modified helper
still fails before changing the copied fixture. The native platform jobs now
run the source-boundary suite before heavy compilation; `.gitattributes` changes
also trigger qualification.

Local evidence: all **8 Python source-boundary controls** pass, including the
new red-to-green checkout reproduction. Python compilation, workflow YAML parse,
the exact helper Git attribute/hash and `git diff --check` pass. Logs are
`/tmp/gitru-r139-helper-eol-red.log` and
`/tmp/gitru-r139-helper-eol-green.log`. No new full native run is claimed for this
checkout-only correction; its new exact-head remote Windows/regression/portability
results remain pending. The earlier successful `61548a0d` run stays separate.

### Portable archive-path validation — 8 October 2026

The new all-platform source suite on head `df640fcb` exposed a distinct Windows
guard defect in [run 37740320278](https://github.com/ruru-m07/gitru/actions/runs/37740320278),
job `113189715491`. The LF checkout/hash control passed. The archive rejection
test failed because host-native `Path('/absolute').is_absolute()` is false on
Windows: a POSIX tar root was not rejected by the intended pre-extraction guard.
This run stopped before native compilation or probe execution; it supplies no
new cipher result.

Archive validation now uses `PurePosixPath` plus `PureWindowsPath` on every host.
It refuses roots, drive-relative paths, traversal, backslashes, alternate streams,
device names, invalid Windows characters and trailing-dot/space aliases before
writing any member. Extraction then joins the validated POSIX components.
Symlinks, hardlinks and non-regular special members remain rejected. No source
hash, payload, fixture adaptation, native dependency or WAL gate changes.

Local qualification: all **9 Python controls** pass. The expanded 15-case unsafe
archive matrix fails before the repair for nine Windows-specific names on macOS
and passes afterward; a valid archive preserves exact binary file contents.
Each unsafe archive contains a preceding valid member, proving whole-archive
validation happens before any output. All four existing pinned archives
(SQLCipher, libsqlite3-sys, OpenSSL source and openssl-src wrapper) pass their
unchanged SHA-256 guards and extract successfully with the strict portable rules.
Python compilation and diff checks pass. Logs:
`/tmp/gitru-r139-archive-portable-red.log` and
`/tmp/gitru-r139-archive-portable-green.log`. Fresh exact-head native/Windows and
cross-platform-reader CI remains pending; prior successful engine evidence is
retained separately.

### Windows cold-build job allowance — 8 October 2026

Exact head `218723d8` in [run 37740813330](https://github.com/ruru-m07/gitru/actions/runs/37740813330)
passed the source suite and encrypted probes on macOS/Linux. Windows job
`113191158223` also completed the packaged encrypted probe successfully, then
hit the outer 45-minute job limit during the separate copied engine regression.
The log reports **736 library tests passed / 4 ignored**, followed by **24
completed integration suites with zero failures**, before cancellation during
`tests/tasks.rs`. The copied regression was still making progress; this is not
an encrypted-probe, assertion, or source-hash failure. Windows artifact upload
and the dependent cross-platform-reader matrix did not run, so they remain
unqualified at this head. All ordinary PR checks passed.

Only the Windows outer job allowance becomes 75 minutes; other platforms retain
45 minutes. Individual test watchdogs, source pins/hashes, probe assertions,
regression adaptation and the WAL gate are unchanged. This accommodates the
observed cold OpenSSL/cipher build plus the independent full copied-engine build
without weakening a test. Local qualification is limited to workflow YAML parse,
the exact Windows/non-Windows budget expression, and `git diff --check`; no new
native execution is claimed for this workflow-only correction. Fresh remote
completion and cross-platform readers remain pending.

## Packaged keyed/plaintext R125 delta — 8 October 2026

The unchanged R125 packaged protocol now runs against both real storage modes at
signed comparison source `30d410a86cdd5e15fe11efae94c73ac082bd5a4a`, whose direct parent is
final packaged qualification head `942dcd06` and which retains the R140
activation ancestry. The release/no-bundle binaries are distinct and recorded by SHA-256:

- plaintext: `617e6d2451a2d1c5d4e65fc7af2b4e53fa63ed6d27d2521a0c8efc681024ad34`;
- keyed: `3fb4ddde118b980de61b9c2dff8002471aff3364f2522407eaa83a0694e9dcac`.

The earlier `a50724e6` pair remains a valid historical diagnostic, but it is not used
for this exact-head result because the later diagnostics-only Rust change altered the
packaged binary hashes. Both modes were rebuilt and rerun after rebasing.

Both seed and cold-restart WebKit runs pass with the exact 10,000-item fixture,
two accounts, five repositories per account, a main window and concurrent child
view. Provider calls and vault loads remain unchanged, generated IPC validation
passes, and both modes retain two native views. The comparison used a synthetic
fixture key only; no personal credential or platform vault was opened.

| Measurement | Plaintext | Keyed | Keyed delta |
| --- | ---: | ---: | ---: |
| Database | 41,971,712 B | 43,110,400 B | +1,138,688 B (+2.71%) |
| WAL | 4,890,472 B | 4,939,912 B | +49,440 B (+1.01%) |
| Database + WAL + SHM | 46,894,952 B | 48,083,080 B | +1,188,128 B (+2.53%) |
| Rust RSS after seed | 159,449,088 B | 156,401,664 B | -3,047,424 B (-1.91%) |
| Rust RSS after restart | 157,843,456 B | 159,711,232 B | +1,867,776 B (+1.18%) |
| Native runtime open, seed | 266.612 ms | 3,694.436 ms | +3,427.824 ms |
| Native runtime open, restart | 173.477 ms | 762.599 ms | +589.122 ms |

The current keyed connection path has a material latency cost in this run. Native
`items` query p95 increased from 8.772 to 84.582 ms for seed/main, 20.032 to
134.114 ms for seed/child, 20.877 to 108.092 ms for restart/main and 21.429 to
124.861 ms for restart/child. At the React useful-content boundary, list p95 was
41/50/58/89 ms in plaintext and 120/163/140/270 ms keyed for the same four
phase/view combinations. Search p95 was 30/30/30/38 ms plaintext and
90/101/90/100 ms keyed. Detail p95 stayed within 30–39 ms in both modes.

These are paired workstation observations, not universal thresholds. They were
run sequentially on an Apple M4 macOS 27.0 host from an external volume reported
87% full; the device's solid-state classification was unavailable. A single run
cannot separate encryption cost from scheduler, filesystem and thermal effects.
WebKit RSS is unavailable because no process could be proven as a descendant,
and the 10,000-item fixture does not qualify the separate 100,000-summary or
500,000-child memory target. The native-open and repeated list/search deltas are
large enough to require profiling and repeated controlled samples before keyed
storage is treated as performance-qualified.

Retained ignored reports:

- plaintext: `artifacts/collaboration-performance/2026-10-08T13-20-43-661Z-53609/performance-report.json`;
- keyed: `artifacts/collaboration-performance/2026-10-08T13-28-55-246Z-56328/performance-report.json`.

During activation, the packaged harness exposed fresh-root validation happening
after Store creation. Final R140/R213 source now distinguishes a new empty Store
from an interrupted authored fixture by checking retained accounts before
recreating state. The keyed build command also uses `python3`, matching supported
macOS hosts where no `python` alias exists. This completes the local encrypted
R125 delta measurement, while the observed latency regression, repeated-sample
qualification, WebKit memory attribution and large-dataset memory gates remain
open performance work. Remote CI and platform qualification are separate from
this local evidence.
