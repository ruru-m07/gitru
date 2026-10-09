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

## Implemented checkpoint — 8 October 2026

The `native-keyed-store` feature now exposes a native-only activation session.
It accepts only an unsafe proof minted after full native candidate verification,
then independently requires same-directory regular files, ready strict key
metadata, closed WAL/SHM/journal state, a free writer lease and exact SHA-256
fingerprints. The preview contains an opaque one-use confirmation identifier and
the recovery bundle path. No key bytes, vault reference or renderer type crosses
this boundary.

Confirmation publishes and syncs the bounded marker before mutation, moves the
plaintext source into a unique recovery directory, installs candidate database
and key metadata, verifies their fingerprints, writes the recovery manifest and
only then removes the marker. Both ordinary Store startup and keyed session
bootstrap refuse a retained marker. An explicit interrupted-activation reader
accepts only the exact bounded marker and can roll back after either the source
move or keyed install; it preserves the candidate and key metadata in the bundle
and never deletes a vault key.

Focused activation tests cover exact confirmation, input drift, malformed and
unready metadata, live sidecars, startup fencing and both rollback phases: **6
passed, 0 failed**. The complete feature-enabled collaboration library run passes
**904 tests, 5 explicit subprocess helpers ignored, 0 failed**. Strict
feature-enabled all-target collaboration Clippy and rustfmt checks pass. These
are local macOS filesystem tests with a synthetic native-verification proof.
Actual plaintext export through SQLCipher, low-disk and hard process-death fault
injection, Windows rename durability, production startup/UI selection, rotation
and reset remain open. Therefore this is an activation coordinator qualification,
not production encryption activation, and RURU-140 remains In Progress.
