# Local result — 8 October 2026

Local macOS synthetic-data run against the checked-in lockfile and schema 20
migration stack. No personal credential, application database or platform vault
was read. `cargo run --locked --offline` exited 0; strict isolated all-target
Clippy and formatting passed.

```text
sqlite=3.50.4; cipher=4.10.0 community; crypto_provider=commoncrypto; fts5=1
production_wal_version_gate=false
fts_query=pass; two_keyed_read_only_pool_connections=pass
migrations_through=20; rollback=pass; db_and_wal_canary_absent=pass
vacuum_backup_keyed_read=true; vacuum_backup_unkeyed_read=false; backup_canary_absent=true
keyed_reopen=pass; wrong_and_missing_key_refusal=pass; synthetic_backup_restore=pass
PROMOTION_BLOCKED: pinned SQLCipher SQLite is below the existing production WAL fix gate
```

This confirms the pinned native-version blocker while exercising basic keyed SQLx
and migration/backup primitives. It does not qualify production Store/recovery
integration, keychain lifecycle, OS crash/suspend behavior, performance, Windows,
Linux or arbitrary temporary-file exposure. The canary checks are finite controls,
not a cryptographic audit. A successful experiment exit is not a release approval.
