# Frozen collaboration schema v8

Numbered SQL files are byte-for-byte copies of applied0001–0008 at participant
PR#162 d2e91957e28de36cc3f7d0ecb2eb6a5d0ea15dcd. Literal seed/ledger/checksums
freeze all historical generic v7 rows plus typed participant.v1 rows/private
six-field clocks, source proof, saved null/false, shared provider identity in two
account partitions, retryable demands and distinct Unicode presentation.
The fixture is independent of current Rust DTOs and contains synthetic data only.

These raw inputs remain immutable. tests/task_migrations.rs qualifies the actual
pinned SQLx/bundled SQLite forward0009 transaction against all25 historical tables,
eight ledger rows, raw JSON/clock bytes, FKs and cold saved reads/draft CAS. Its
fault after the actual old-parent drop proves exact rollback and immediate reopen.
The historical schema rejects Tasks; latest schema admits them while preserving
Body-only metadata, account RESTRICT and child cascades. The synthetic participant
rows qualify storage, not GitHub eligibility or any live-provider behavior.
R106 archive v1/v2 restore policy is not extended by this supported Store bootstrap.
