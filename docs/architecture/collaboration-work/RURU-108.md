# RURU-108 — Private collaboration data at rest

Status: policy and compatibility spike in progress, 8 October 2026.

Proposed GA policy: encrypt the entire collaboration database and its recoverable
artifacts with an application-managed random database key protected by the OS
credential service; keep provider tokens independent. Current prerelease SQLite
remains plaintext and must not be described as encrypted merely because tokens
use a vault or Unix file permissions are private. This is a reviewable policy
proposal, not a claim of shipped encryption or product-owner acceptance.

Before changing production dependencies, test the exact pinned SQLx 0.9.0 /
libsqlite3-sys 0.37.0 SQLCipher build in a separate Cargo workspace with synthetic
keys and data only. Do not touch application databases, backups, credentials,
root Cargo manifests or lockfiles. Verify reported SQLite/cipher versions, FTS5,
current migrations, keyed reopen, wrong/missing-key refusal, WAL/canary exposure,
transaction rollback and backup/export behavior. Keep the existing production
WAL-reset version gate unchanged. The bundled SQLCipher source currently declares
SQLite 3.50.4, while production permits 3.51.3+, 3.50.7+ or 3.44.6+; this is already
a source-level compatibility blocker, which the spike should confirm at runtime.

The final document will distinguish local synthetic checks from real platform
vault qualification and three-platform packaged builds, and will cover key loss,
rotation, plaintext-to-encrypted migration, backup portability and locked-vault
startup. It must not claim protection from an attacker controlling the unlocked
user session, process memory, renderer access, clipboard, screenshots, swap or
explicit plaintext exports. Independent bounded implementation follow-ups remain
necessary before a GA encryption claim.
