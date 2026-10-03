# Frozen collaboration schema v1

This fixture records `0001_local_collaboration.sql` from foundation commit
`baafef75`. Keep `schema.sql`, `checksum.sha384`, and the migration-ledger checksum
in `seed.sql` immutable. New production migrations must upgrade this fixture;
editing an applied migration must fail the checksum gate.

The seed uses literal historical JSON rather than the current Rust DTO serializers.
It includes four account identities across active, auth-required, and disconnected
states, overlapping repository/item identifiers,
denied private scope content, a partial sync cursor, authored drafts (including
disconnected subjects), and nonzero revision/epoch/generation metadata. None of
the data contains real credentials or user content.

The fixture was captured before any second production migration existed. Tests
compare the v1 content with the latest embedded production migrations, so they
become a real v1-to-latest upgrade gate as forward migrations are added. Synthetic
failure migrations in `tests/migrations.rs` characterize the pinned SQLx/SQLite
transaction behavior; they are not historical Gitru releases.
