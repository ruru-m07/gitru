# R140 next slice: owned keyed connections before activation

Pre-code contract, 8 October 2026. Isolated managed external worktree from exact PR200 de10f464, signed merge of R139 source218723d8. Default application behavior remains unchanged.

## Recommendation

Implement and qualify one native connection factory, with a narrowly pinned
SQLx pre-initialization hook, before adding Store activation or conversion.
The factory must actually key writer, three read-only pooled handles, maintenance
and recovery/verifier handles through the C API. Keep it in an isolated opt-in
cipher workspace until its source, cancellation and leakage tests pass. This is
a useful prerequisite with a smaller review boundary than combining the native
build, vault startup, connection rewrite and two-file activation protocol.

Do not simply enable the stock bundled-sqlcipher feature. Production remains
ordinary bundled SQLite and its existing Store constructor remains unchanged.

## Current evidence and live dependencies

- R140 is In Progress, blocked by R139. R141 is Backlog, blocked by R139 and R140.
  R140 explicitly requires every handle to be keyed before database access,
  byte-preserving startup errors, separately verified conversion and actual OS
  vault qualification. R141 additionally needs portable restore credentials,
  artifact exposure coverage and a reviewed standard encryption envelope.
- PR194 supplies the native key reservation/file-identity/lease component.
  PR200 supplies an optional explicit file-keychain macOS adapter. At exact
  `de10f4641c349e80775645f17964f303c75377ad`, the real disposable macOS keychain
  job passes; Windows Rust and macOS harness remained pending during this audit.
  Neither PR selects an app vault or opens a keyed Store.
- R139's earlier exact `61548a0d` native seven-job matrix passed. Its current
  `218723d8ade902231aea631363bdba826987700c` run37740813330 has passed source
  reproduction and native source-boundary/input-preparation checks on all three
  platforms. Linux/macOS encrypted probes pass; Windows probe and copied-engine
  regression/portability jobs are still pending. Keep those results separate.
- R139 authenticates SQLCipher4.19.0/SQLite3.53.4, libsqlite3-sys0.37.0 and
  OpenSSL3.6.5 on non-Apple platforms; Apple uses CommonCrypto. Its current copied
  application regression baseline is schema22 `f0b8801d`. R120/schema23 and
  R132/schema24 are newer work: a keyed whole-engine acceptance run must pin the
  final qualified combined source, not claim current compatibility from22.

## Concrete compatibility findings

Pinned SQLx0.9.0 `options/connect.rs` calls `SqliteConnection::establish`, then
executes its PRAGMA batch before returning. `SqliteConnectOptions` derives Clone
and Debug and holds ordinary strings for PRAGMA values. The probe's synthetic
`.pragma("key", ...)` is intentionally not a production secret interface.
An ordinary pool `after_connect` hook runs after connection establishment;
using it with Store's WAL/synchronous options cannot guarantee key-before-access.
No public native-handle adoption or before-initialization hook exists in this
version. `lock_handle` safely exposes an already-established handle but does not
repair accesses that have already happened.

The small driver adaptation should introduce an opt-in native initialization
callback immediately after `sqlite3_open_v2`, before extensions, PRAGMA execution
or any schema operation. The callback captures a native redacted key owner and
calls `sqlite3_key` directly. It is fixed by our factory, not supplied through
renderer/configuration input. Options/debug/error formatting never traverses the
callback capture. Retain that owner in the connection worker until actual close.
The default callback is absent, preserving the ordinary driver behavior.

Patch only exact SQLx0.9.0 archive
`488e99c397a62007e4229aec669a179816339afc6d2620ca6fa420dbee2e982c`, with complete
input/output tree hashes and a readable minimal patch. Continue using R139's
exact native/crypto payloads and unchanged WAL gate. Do not fetch or modify an
upstream branch. Qualify this independent driver delta and offer it upstream
separately; upstream publication is not a prerequisite for local implementation.

This is one SQLite linkage per binary. A root Cargo patch/feature switch affects
all SQLx consumers, and the stock0.37 cipher source is not the R139 candidate.
Initially keep the generated verified patches in the isolated test workspace;
a separately reviewed packaging build profile must select them for whole-app
qualification before production selection. No system-library discovery fallback.

## Proposed native API and ownership

```rust
enum KeyedHandleRole { Writer, Reader, Maintenance, Recovery, Verifier }
struct CipherProfile; // fixed validated build identity/settings, no user values
struct KeyedConnectionFactory { /* Arc-owned session/key/lease; private fields */ }
struct KeyedConnections { /* writer + bounded readers + owner; no Serialize */ }

impl KeyedConnectionFactory {
    async fn open_writer(&self, mode: KeyedOpenMode) -> Result<OwnedKeyedConnection>;
    async fn open_reader_pool(&self) -> Result<OwnedKeyedPool>; // max3, read-only
    async fn open_maintenance(&self) -> Result<OwnedKeyedConnection>;
    async fn open_recovery(&self, role: RecoveryRole) -> Result<OwnedKeyedConnection>;
    async fn verify_existing(&self) -> Result<AuthenticatedDatabase>;
}
```

Only a native session supplies canonical path, key and reservation identity.
Explicit new-store authority is required for create flags; all other paths use
existing-only opens. Pool replacements invoke the same hook for every new handle.
No key enters a URL, SQL statement, trace, process argument, environment or IPC.
The 32 random bytes may be supplied as a binary passphrase to sqlite3_key and use
the pinned SQLCipher KDF settings; do not accidentally label that raw-key mode or
change encoding between opens. Any later raw-key mode is a versioned profile.

Owned startup survives caller cancellation. Failures close the actual SQLite
worker before releasing the session/writer lease, including failed initialization
before a SqliteConnection is returned. Close stops admission, drains readers,
closes writer/transient handles, then releases key/session ownership. Default
Drop cannot unlock while a worker remains alive. This extends the existing
R106 shutdown discipline to pool construction and initializer failures.

Every successful open verifies exact cipher/source/crypto provider profile,
cipher_status=1, HMAC/header/page/KDF settings, unchanged WAL gate and FTS5. Existing
data requires an authenticated schema read; a successful sqlite3_key call alone
is insufficient. Map wrong-key/corruption conservatively without trying plaintext.
Actual app identity/schema/proof validation remains a separate caller step before
Ready: the current application has no encrypted database-identity row. Do not
pretend the synthetic verifier proves that missing integration. The eventual
identity record needs a coordinated schema/recovery policy change, not an
unregistered extra table that strict backup validation would reject.

## Required connection inventory for the later Store slice

- `storage.rs::open_owned`: WAL writer plus reader pool.
- `storage/retention.rs::maintenance_connection`: transient checkpoint/status.
- `recovery.rs::connect`: current database, imported snapshot, scrubbed export,
  candidate, validation and interrupted recovery paths.
- Native backup destination/source handles and any conversion attachment.
- In-memory expected-schema validation is deliberately a separate non-secret
  schema fixture; it must not acquire the app key or accidentally open an app path.

Replace these through one explicit factory in the following slice. The present
`Store::open` also owns a writer lease; keyed session handoff must transfer/share
that lease rather than reacquire it and deadlock. Do not let existing recovery
constructors open encrypted input with their current plaintext options.

## Factory qualification, before Store wiring

Use generated synthetic32-byte keys and disposable paths. Prove native keying
precedes the first SQL statement using an initialization trace; trace/debug/errors
must contain neither key nor its textual encodings. Qualify new encrypted create,
all three concurrent readers, reader pool replacement, native maintenance,
read-only verification, wrong/missing-key refusal and plaintext refusal. Verify
ordinary SQLite linkage/profile mismatch fails closed.

Inject cancellation/failure before open, after native open, after keying, during
first schema read, during second reader creation, and while close is blocked.
Another process must not acquire the session lease until every worker closes.
Test binaries use actual R139 cipher artifacts on macOS/Linux/Windows. Run actual
FTS/WAL/busy/read-only/native backup operations and bounded DB/WAL/journal/temp
canary scans; scan absence is only artifact evidence. No personal vault needed.

## Conversion design after factory and keyed Store qualification

Conversion is a separate explicit operation under drained runtime and the single
writer lease. Reserve the destination key first; preserve a durable non-secret
journal with source fingerprints, target identity/key-generation, candidate and
rollback paths and phases. State transitions are:

1. SourceCaptured: original main/WAL/SHM/journal remain untouched and fingerprinted.
2. CandidateWritten: a distinct private encrypted destination receives an export.
3. CandidateVerified: keyed reopen, cipher/integrity/FK, exact migration/schema and
   authored-command/evidence invariants pass; both candidate and directory sync.
4. SourceArchived: complete rollback bundle is durable; activation marker remains.
5. Activated: same-volume atomic install of candidate and key metadata is recorded.
6. Ready: reopen through the final factory, verify identity, then retire the marker.

DB and sidecar cannot be atomically replaced as one filesystem object. The durable
activation marker must block ordinary startup across every intermediate phase;
reuse R106's preserve-and-inspect approach rather than assuming two renames are
atomic. Cold restart resumes only a fully validated phase or exposes explicit
rollback; never falls back to plaintext. Low disk, failed sync/rename, process
death or cancellation retain original/candidate/key evidence. Windows needs its
own actual replacement/durability qualification. Historical plaintext rollback
and backups are preserved, not claimed erased.

Use `sqlcipher_export`, not in-place rekey, for plaintext conversion. Prefer an
encrypted main destination plus a read-only attached plaintext source; only the
source attachment uses an empty key and it never becomes an application fallback.
Validate export of triggers/FTS/data and explicitly handle user_version and
auto_vacuum, which export does not transfer. A format conversion of the same live
ledger is not automatically a restored backup; define whether quarantine is
required without modifying immutable command receipts. Imported backups retain
the existing mandatory quarantine policy.

## Still-open activation boundaries

The macOS adapter targets an explicit file keychain, not packaged modern data
protection keychain behavior. Linux/Windows create-only adapters remain absent;
keyring upsert is not atomic create. Full Store/IPC/pool performance, current24+
schema migration, recovery UI and final app artifact linkage remain unqualified.
R141 must define portable encryption independently of the device vault key; a
same-device keyed backup is not a portable export. Neither factory nor conversion
should silently make the existing backup button emit a format with no usable
restore credential. Production encryption is not switched by this proposal.

## Primary sources

- SQLCipher API: https://www.zetetic.net/sqlcipher/sqlcipher-api/ (sqlite3_key,
  authenticated read, cipher_status, export and explicit export limitations).
- Exact installed SQLx0.9.0 source: `src/options/connect.rs`, `src/options/mod.rs`,
  `src/connection/establish.rs`, `src/connection/worker.rs`. Registry checksum above.
- R139 verified libsqlite3-sys0.37 build source/bindings: one link owner, bundled
  SQLCipher feature branch, CommonCrypto/OpenSSL selection and sqlite3_key ABI.
- Current R140 lifecycle/platform worknotes and live R140/R141 acceptance records.
