# RURU-108 — Private collaboration data at rest

Status: architecture decision and bounded compatibility evidence for review,
8 October 2026. Production encryption is not implemented by this change.

## Decision and release boundary

The GA collaboration engine should encrypt its complete local database, including
provider cache, search/projections, authored drafts and command history. Use a
random database key protected by the platform credential service, independent of
Gitru cloud and provider tokens. Encrypt recoverable backups by default, with an
explicit portable recovery mechanism. Plaintext exports require an intentional
user action. This is the selected architecture target; the implementation gates
below must pass before the product can claim it.

Current prerelease storage remains ordinary plaintext SQLite. Unix mode 0600/0700
hardening and vault-protected provider tokens do not encrypt database contents.
The existing single-file store mixes public/private provider data and authored
intent, so selectively encrypting a few columns would leave titles, FTS terms,
identities, status metadata and derived projections exposed. A whole-store codec
fits the existing native writer and short read snapshots more directly.

The pinned bundled SQLCipher candidate is **not eligible for production**: the
actual spike reports SQLite 3.50.4, below Gitru's unchanged WAL-reset safety gate.
Do not bypass that gate to get encryption. Retain the current dependency set until
a compatible build and the lifecycle/recovery integration are qualified. This PR
neither changes the main Cargo manifest/lock nor opens a real application database.

## Threat model and exposed artifacts

The intended protection is confidentiality and integrity of copied database files
when the attacker lacks the database key: a lost removable volume, copied app-data
directory, misplaced backup or other user's access to storage bytes. OS full-disk
encryption remains complementary; this policy does not assume it is enabled or
that external volumes and copied backups inherit it.

An attacker controlling the unlocked user account or Gitru process may read the
key, query data, inspect memory or capture the screen. App encryption does not
isolate a compromised renderer/native process, remove clipboard/history exposure,
or establish swap/core-dump/screenshot protection. No OS vault policy tested here
proves resistance to same-user malware. The application must retain its existing
account/view/access fences independently of encryption.

Inventory database pages, WAL and rollback journals, FTS and cached file blobs,
SQLite temporary files, backup exports, staging/restore candidates, rollback
bundles and diagnostics. Sensitive metadata such as repository names and account
identity counts as private data too. SQLCipher is a candidate database codec, not
a substitute for checking every artifact produced by our own recovery code.
SQLite documents several temporary and journal file classes, so a database-file
check alone is insufficient. See [SQLite temporary files](https://www.sqlite.org/tempfiles.html).

Source inspection in this stack:
- `crates/collaboration/Cargo.toml` pins SQLx 0.9.0 and libsqlite3-sys 0.37.0 with
  ordinary bundled SQLite.
- `Store::open_owned` in `src/storage.rs` configures WAL/FULL, one native writer,
  three bounded read connections, FTS5 and a fixed-SQLite version gate. It supplies
  no encryption key. Private file/directory modes are Unix-specific hardening;
  Windows ACL qualification remains separate.
- `src/recovery.rs` creates a consistent snapshot, removes credential references,
  uses VACUUM INTO, verifies schema/contents and publishes a separate file. Its
  independent connections currently supply no key. Restores preserve staging and
  rollback bytes intentionally; sanitizing credentials does not sanitize cached
  provider text or authored drafts.

## Key lifecycle and startup

Generate an independent high-entropy database key in native code; never derive it
from a PAT, login, account UUID or cloud token. Keep a non-secret database/key
identity and format generation for lookup. Provider account replacement/disconnect
must not rotate/delete the database key or destroy unrelated recovered drafts.
Use a secret-holding type, scoped lifetimes and audited key application; avoid
putting key material in IPC, renderer state, command arguments, connection URLs,
Debug output, query tracing or diagnostic bundles.

Every writer, reader-pool and backup/recovery connection must be keyed before its
first database operation. SQLCipher documents that ordering requirement, supports
raw key material, and requires export-based conversion for plaintext SQLite;
`rekey` alone cannot encrypt an existing plaintext database. See the
[SQLCipher API](https://www.zetetic.net/sqlcipher/sqlcipher-api/).

A locked vault leaves collaboration unavailable with actionable unlock/retry copy;
the rest of the Git client can remain available. Missing/wrong keys preserve the
original bytes and expose recovery/reset choices. Never fall back to plaintext,
create a replacement key over an existing store, or mistake a key error for an
empty cache. Linux systems without an available credential service require an
explicit supported key-storage policy before enabling this capability.

Rotation must stop new jobs, drain the native writer/readers, preserve an old-key
recovery path, validate the candidate and activate it atomically. Use the existing
writer lease and crash-recovery boundaries. Key loss is potentially permanent
loss of unsent intent; the product must explain that and support portable encrypted
backups. There is no Gitru cloud escrow requirement in this design.

## Migration, backup and retention

Migrate existing plaintext databases to a distinct private encrypted staging file.
Validate schema/foreign keys/integrity, authored draft/command counts and important
content before closing pools and atomically activating it. Low disk space, a wrong
key, interrupted copy, rename failure or crash must preserve a recoverable source.
Carry the restore quarantine/no-automatic-resend policy across encryption changes.

Default backups need an independently reviewed portable encryption/key-wrapping
format and explicit restore credentials; a device-local keychain item does not
travel with a file. Keep backup verification, sanitized credential handling,
version compatibility and rollback paths key-aware. An encrypted output that only
opens on the originating device is not yet a portable recovery feature.

Successful migration cannot promise erasure of historical plaintext backups,
filesystem snapshots, journal remnants or SSD blocks. Keep an explicit policy for
retained rollback bundles and user-approved cleanup; do not silently destroy the
only recoverable authored intent. Avoid custom cryptography and separately qualify
any passphrase/envelope format before selecting it.

## Reproducible pinned-build spike

The isolated workspace at `scripts/spikes/sqlcipher-compat` changes no application
features. Its checked-in lockfile, known synthetic keys and temporary database
under its own ignored target directory make the experiment reproducible without
personal credentials or application data. It enables libsqlite3-sys 0.37.0's
bundled-sqlcipher feature alongside the same pinned SQLx 0.9.0 SQLite interface.
The SQLx upstream package itself includes SQLCipher integration tests; this
experiment additionally exercises our real migration SQL through schema 0020.

Local macOS result: SQLCipher 4.10.0 community / SQLite 3.50.4. The runtime reports CommonCrypto. Read-only-pool, FTS and artifact observations
are recorded in the checked-in [spike results](../../../scripts/spikes/sqlcipher-compat/RESULTS.md).
Keyed creation, FTS query, current migration SQL, rollback, two keyed read-only
pool connections, keyed reopen, wrong/missing-key refusal, encrypted VACUUM INTO
and synthetic backup restore/integrity checks pass. The database, active WAL and
backup did not contain the known canary in this bounded experiment. That scan is
not proof of complete cryptographic or temporary-file coverage.

The runtime SQLite version **fails** the gate in `storage.rs`: at least 3.51.3,
3.50.7 in the 3.50 line, or 3.44.6 in the 3.44 line. Inspection of the pinned package
also shows ordinary bundled SQLite 3.51.3 versus SQLCipher's SQLite 3.50.4. This is a
confirmed pinned-build compatibility blocker, not a speculative release concern.
The standalone experiment intentionally reports it without relaxing production.

This does not run the full application recovery policy under encryption, qualify
real platform vaults, prove OS suspend/restart behavior, test Windows/Linux crypto
linkage, benchmark product latency or establish arbitrary temporary-file secrecy.
Current unkeyed recovery connections would require implementation changes even
after the native version problem is resolved.

SQLCipher's upstream build requires a crypto provider and codec initialization;
its own database-format/major-version compatibility is distinct from SQLite schema
migration. See [SQLCipher source and build guidance](https://github.com/sqlcipher/sqlcipher).
Do not use upstream performance marketing as a Gitru measurement.

## Bounded implementation work

1. [RURU-139](https://linear.app/catra/issue/RURU-139/qualify-a-sqlcipher-build-that-meets-gitrus-sqlite-wal-safety-gate): select a fixed native build, record provenance/linkage/license and qualify all three packaged platforms with our FTS/migration/WAL/recovery and RURU-125 performance fixtures.
2. [RURU-140](https://linear.app/catra/issue/RURU-140/implement-native-database-key-lifecycle-and-recoverable-encrypted): implement database-key startup, locked/missing-key behavior, pool initialization and crash-safe migration/rotation. Depends on the qualified build.
3. [RURU-141](https://linear.app/catra/issue/RURU-141/add-portable-encrypted-collaboration-backups-and-verify-artifact): qualify portable encrypted backups, key-aware restore, artifact exposure and explicit plaintext export. Depends on the build and key lifecycle.

RURU-108 supplies the architecture decision and bounded compatibility evidence.
These child implementations remain Backlog and the GA gate remains open. Local
spike checks are distinct from remote CI and live platform/provider qualification.

## Windows CI watchdog repair — 8 October 2026

Existing PR186 at `486a936e700fe00b863ac26f6f0b526477574a5d` completed 13
checks successfully. The Windows Rust job `113100881716` in run `37712301903`
was cancelled at the configured 45-minute job limit while still compiling the
retained-feature native dependencies. Its log contains no compiler or assertion
failure; unlike PR182's earlier cancellation, this run had not finished all
retained checks.

Apply the already reviewed platform-test watchdog increase from 45 to 60 minutes
to this existing branch. Test commands, assertions, retry policy, dependency
versions and encryption spike behavior remain unchanged. Local validation checks
that the workflow parses and that this timeout is the only workflow change; diff
checks pass. Fresh remote CI must qualify the new head independently. The prior
13 successes remain historical evidence; no Windows success is inferred from the
local YAML check and no encryption implementation is claimed.
