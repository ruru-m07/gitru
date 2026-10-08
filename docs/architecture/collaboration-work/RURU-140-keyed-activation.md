# RURU-140 next bounded slice: plaintext activation transaction

Pre-code contract recorded on 2026-10-08.

## Scope

Add a native-only transaction that activates a separately-created, fully
verified keyed candidate over a stopped plaintext collaboration database. The
transaction must hold the existing writer lease, reject live WAL/SHM state,
fingerprint both inputs, require an exact one-use confirmation identifier, and
publish a durable same-directory marker before any rename. Activation preserves
the complete plaintext source and its sidecars in a named recovery bundle. A
failure or process death after the marker is published must block normal and
keyed startup until an explicit resume or rollback path handles the marker.

The cipher/export implementation remains owned by the qualified native factory:
this coordinator never receives key bytes and cannot bless a candidate. Its
trusted native verifier must authenticate the candidate, its immutable storage
identity, schema, integrity, authored intent and command evidence before a
preview is returned. Candidate generation, UI/IPC, production startup selection,
vault deletion, key rotation and portable backup remain outside this slice.

## Safety contract

- Preparation is read-only for the active database and never creates key
  metadata beside it.
- Candidate and candidate metadata must be regular private files in the target
  directory; all transient SQLite sidecars must be absent before preview.
- Confirmation rechecks source and candidate fingerprints and consumes the
  exact preview once.
- After the durable marker, every original file is moved into its recovery
  bundle before candidate files can become active.
- Directory sync follows each rename phase. Ambiguous durability preserves the
  marker and refuses Ready publication.
- The marker records hashes and relative filenames only. It never contains key
  bytes or a vault credential reference.
- Rollback and crash-resume qualification are required before this slice can be
  wired into desktop startup.

