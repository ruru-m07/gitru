# RURU-140 — database-key lifecycle and recoverable encryption

## Contract before implementation — 8 October 2026

This bounded first slice starts from combined schema-22 source
`f0b8801dec569af24a1b1f67744600f85f4d90ac`. It implements an independent native
key reservation/lookup component and its failure contract. It does not enable
SQLCipher in production, modify `Store::open` defaults, add renderer commands,
open user databases, or access personal OS credential entries.

The [R108 encrypted-GA decision](https://github.com/ruru-m07/gitru/blob/089ac9f529bfc5f05a007981d17a439a9a022d71/docs/architecture/collaboration-work/RURU-108.md)
remains the product policy. [R139 draft #193](https://github.com/ruru-m07/gitru/pull/193)
qualifies SQLCipher 4.19.0 / SQLite 3.53.4 independently. Its local macOS encrypted
schema-22 probe and 1,016-test unkeyed engine regression pass, and Linux source
reproduction is green. Native Windows/Linux/macOS CI, cross-platform portability,
whole-app keyed pools and encrypted/current IPC performance remain separate
acceptance evidence. Production activation requires that qualification.

## Native boundaries

- A database-specific 32-byte random key uses OS randomness and zeroizes on drop.
  It has no Serialize/Display implementation, redacted Debug, and no conversion
  to provider `SecretToken`. Raw bytes are available only to native vault/codec
  adapters; no connection URL, SQL diagnostic, environment or process argument
  carries them.
- A distinct `DatabaseKeyVault` interface and namespace own database keys.
  Provider connect/disconnect/cleanup has no reference to that interface. Locked,
  unavailable, absent, invalid and ambiguous write outcomes remain distinguishable.
  An ambiguous write is followed by readback, never by deleting/replacing a key.
- A lifecycle session holds the existing database writer lease throughout file
  inspection, metadata changes, vault access and handoff. Competing app startup
  or recovery cannot act concurrently. Blocking vault operations belong on a
  cancellation-owned native blocking task when this component is wired.
- A bounded, private, non-secret sidecar stores version, random database identity,
  generation and phase (`reserved` or `ready`). The vault reference is derived
  from that typed identity in a fixed database-only namespace. Renderer input
  cannot choose a credential service/reference or a path. Unknown versions,
  malformed identity/generation, symlinks, nonregular files, oversized metadata
  and unresolved restore state fail closed.
- Persist and sync a reservation before storing any new secret. Ready publication
  uses a separately synced metadata file and atomic replacement under the lease.
  Retain the previous valid metadata when writing, syncing or rename fails.
  Directory durability on Windows remains a platform qualification, not a claim
  based on Unix fsync tests.

## Startup and crash states

| Saved state | Native outcome / permitted action |
| --- | --- |
| No DB, WAL/SHM/journal, recovery bundle or key metadata | Explicit new-store reservation may generate a new database identity. Reserve durably before calling the vault. |
| Reserved metadata; no DB or sidecars; key absent | Resume the same reservation and provision that key reference. This is the only state permitting secret generation. |
| Reserved metadata; no DB or sidecars; key present | Resume creation with that exact saved key. Never replace it with a new random value. |
| Reserved metadata; DB or durable sidecars present; key present | Interrupted initialization requires keyed read-only verification before promotion to Ready. No automatic reset/recreation. |
| Reserved metadata; DB/sidecars present; key absent | MissingKey; preserve every file and the reservation. |
| Ready metadata; DB present; key present | Supply a native key lease for authenticated open. A successful vault load is not proof that the key opens the database. |
| Ready metadata; DB absent | MissingDatabase; preserve key/metadata and offer future explicit recovery. Never silently create an empty replacement. |
| DB or sidecars exist; metadata absent | PlaintextMigrationRequired only for a positively identified plaintext header; otherwise MissingKeyMetadata. Never create a replacement key or open SQLite to guess. Empty existing files are existing evidence too. |
| Vault locked/unavailable or key readback differs | Return a typed error; preserve metadata/data and do not publish Ready. |
| Keyed verification fails | WrongKeyOrCorrupt; no plaintext fallback or key regeneration. Preserve original bytes for explicit recovery. |

A failed key store can have succeeded externally. Readback of the exact generated
key is sufficient to continue; missing, different or unreadable readback leaves the
reservation intact for a later retry. Crash recovery loads an existing key at that
reference before ever considering generation. The component never removes an
orphan key automatically: absence of a DB can represent loss, an interrupted
write or a user move, and cannot authorize destructive cleanup.

## Keyed Store wiring seam (not activated in this slice)

The eventual native connection factory must apply key material through the codec
before the first SQLite operation, then verify actual encryption, readable schema,
expected database identity and the unchanged SQLite WAL gate. Every writer,
read-only pooled connection and recovery handle must use the same reviewed
factory. The existing cancellation-owned startup/shutdown must retain the writer
lease until all SQLite worker handles actually close. A new-store reservation is
promoted only after verified durable encrypted database creation; a native probe
interface permits finite lifecycle tests without making a fake cipher the
production default.

SQLCipher defers key authentication until a database read; setting a key alone
must never produce Ready. Its native `sqlite3_key` interface avoids SQL text
containing the secret. See the [primary API](https://www.zetetic.net/sqlcipher/sqlcipher-api/).
A renderer-facing error projection can be added when actual startup is wired;
secret/reference types must stay outside generated IPC.

## Plaintext conversion, rotation and recovery follow-up

Conversion is an explicit separate operation: stop admission, drain all native
jobs/readers/writer, hold the writer lease, preserve source and rollback evidence,
export into a distinct keyed private staging database, and validate every current
migration, schema/foreign key/integrity rule and authored draft/command/proof
invariant. Only a verified candidate can activate atomically. Low disk, failed
rename, process death and cancellation must leave a named recoverable source.
`rekey` does not convert plaintext. Rotation follows the same staging discipline
and retains the old key until rollback no longer depends on it. This slice does
not add an in-place rekey shortcut, migration/reset UI, or plaintext fallback.

Account disconnect leaves the shared key and other accounts' drafts untouched.
Explicit database reset, key retirement and reinstall require a separately
reviewed destructive lifecycle; uninstall/reinstall does not imply permission to
delete OS key entries. Loss of all key copies can mean permanent loss of unsent
intent. Portable recovery belongs to R141; a device key alone is not a portable
backup. Existing restored-command quarantine remains mandatory after conversion.

## Qualification and scope ownership

Own new native database-key module/tests, its crate registration/dependencies and
this note. No provider/vault adapter, scheduler, UI, IPC, schema migration or
production cipher feature change is part of the first checkpoint. There is no
file overlap with the active labels, merge or platform-qualification lanes.

Use synthetic keys, a deterministic fault-injecting vault and disposable files.
Exercise reservation before/after each filesystem/vault boundary, ambiguous store
success/failure and readback mismatch, existing DB + lost key, missing DB + Ready
metadata, exact resume after reopen, malformed/symlink/oversized metadata, writer
contention, data/sidecar preservation and redacted errors/debug. Verify failpoints
never rewrite existing DB bytes or disclose key bytes in sidecar/error output.
Actual platform vault behavior and full encrypted migration/backup/application
qualification remain open acceptance; no synthetic result substitutes for them.

## Evidence

Design only at this checkpoint. Implementation and validation results will be
recorded here with exact scope; R140 remains In Progress.
