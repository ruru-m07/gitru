# Frozen collaboration schema v7

This fixture freezes migrations 0001–0007 from `1766a6e7308375806aa637380ccf7caa2714669b`,
before the participant facet migration. Each numbered SQL file is a byte-for-byte
historical copy. `checksums.sha384` and the literal seven-row SQLx ledger in
`seed.sql` record their applied checksums. Keep these historical inputs immutable;
new storage behavior must arrive through a forward migration.

The seed uses literal historical JSON, including private reconciliation clocks,
without constructing or serializing today's Rust DTOs. It contains all four old
detail observations for two selected actors, Comments/Reviews/Checks entries,
Body metadata, pending and stopped detail demands, private authored drafts and
generations, credentials-reference/cleanup metadata, identities and aliases,
local link intent, partial/denied feeds, and nonzero runtime revision/view/log
state. Body is a singleton value and therefore has no entry rows. Everything is
synthetic; no token or personal content is present.

`tests/participant_migrations.rs` runs the pinned production SQLx/SQLite migrator
against these raw files and ledger. It compares all historical rows and JSON
bytes, verifies foreign keys and new facet constraints, reads through the normal
local Store after immediate reopen, and checks draft generation CAS. A fault
injected after the actual 0008 old-table drop boundary must restore the exact v7
schema, data and ledger. This is supported Store bootstrap qualification only;
it does not extend RURU-106's archive restore allowlist.
