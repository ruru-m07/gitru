# RURU-141 bounded slice: portable encrypted backup envelope

This slice adds a portable encrypted envelope for an already verified standalone
collaboration SQLite export. It starts from the exact RURU-140 key-lifecycle head
`fb38279d` and the RURU-139 packaged/performance head `7b19aad9`; merge commit
`268197b9` contains both as ancestors.

## Format and credential boundary

The outer file is the interoperable age v1 binary format using age's scrypt
passphrase recipient. Gitru does not define a cipher, KDF, nonce construction or
authentication scheme. A human-provided restore passphrase is required at export
and restore and is not stored, logged, returned through IPC or replaced with the
device database key. The implementation intentionally has no raw-key export API.

The authenticated plaintext begins with a fixed format marker and a bounded JSON
manifest, followed by one closed SQLite snapshot. The manifest binds:

- format and media type;
- exact SQLite byte length and SHA-256 digest;
- reviewed schema version, runtime revision and bounded account/draft/command
  counts.

The SHA-256 is an identity and stale-file check inside age's authenticated
ciphertext, not a replacement for age authentication. Restore checks the age
stream first, enforces size bounds, writes create-only private staging, verifies
the exact digest and summary, and then delegates to the existing schema,
migration, authorization-fence, command-quarantine and explicit-confirmation
recovery transaction. Dropping or failing before confirmation removes staging
and leaves active storage untouched. Final publication is create-new and atomic on
the destination filesystem.

## Exposure inventory and boundaries

The portable artifact contains neither the SQLite header nor synthetic private
canaries in a byte scan. Wrong passphrases, authenticated-stream tampering,
truncation, uncheckpointed WAL evidence and unknown schemas do not modify the
target and leave no named portable staging directory. Recognized historical
schemas are encrypted without mutation and migrate only after decryption into
the existing private recovery staging path. The current recovery transaction
continues to preserve drafts/evidence, remove credential references, quarantine
commands, advance authorization fences, preserve the old database bundle and
fail closed at its existing crash checkpoints.

While a restore is being explicitly inspected, the decrypted SQLite candidate is
a named `0600` file inside a random `0700` directory beside the target because
SQLite schema/migration validation requires a native path. It is removed by RAII
on success, error or cancellation. Host compromise, process memory, filesystem
snapshots and storage remanence remain outside this format's protection. A user
who explicitly requests the existing plaintext export still creates plaintext;
encrypting a later copy cannot erase that export, snapshots or SSD remnants.

The existing keyed Store continues to reject plaintext backup/recovery entry
points. Device-independent restore instead decrypts and validates the portable
artifact into RAII staging, runs the existing migration/fence/quarantine policy,
reserves a fresh RURU-140 next-generation device key, and crosses one trusted
native SQLCipher importer. That importer attaches a create-only candidate, keys
it through `sqlite3_key_v2` before any export, runs `sqlcipher_export`, rebinds
the immutable database identity, authenticates the cipher profile/schema/
integrity/authored summary, checkpoints and closes it. Only then can it publish
matching ready metadata and construct RURU-140's unsafe verified-rotation proof.
Rotation confirmation preserves the prior local keyed database and metadata in
its recovery quarantine. The source device key is neither required nor wrapped.

## Validation

- Portable unit suite: 4 passed. It covers round-trip recovery, explicit old
  schema migration, wrong key, ciphertext tamper, truncation, create-new
  publication, WAL rejection, unknown schema rejection, target preservation,
  artifact canary/header scans and staging cleanup.
- Existing recovery tests remain the authority for migration, durable intent,
  command quarantine, authorization/epoch fences and injected crash boundaries.
- Actual qualified SQLCipher native suite passes 11 tests with one deliberate
  harness helper ignored. Its portable-import test restores the artifact into a fresh
  local key generation, byte-scans artifact/current DB/sidecars, verifies the
  preserved old-keyed quarantine and cold-reopens authored data under the new
  generation. The full native harness and platform CI remain separate evidence.
- The final portable suite passes 4 tests, the qualified native strict Clippy
  run and workspace formatting pass, and the earlier all-feature collaboration
  run passed 937 tests with 7 ignored. The ordinary workspace Clippy command
  cannot compile the opt-in keyed crate without the retained qualified SQLx
  patch; that native seam is checked through its pinned harness configuration.

All fixtures use synthetic accounts, content and passphrases. No personal
credentials, device database or provider mutation is involved.
