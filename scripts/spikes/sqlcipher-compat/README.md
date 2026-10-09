# SQLCipher compatibility spike

Run from the repository root:

```sh
cargo run --locked --manifest-path scripts/spikes/sqlcipher-compat/Cargo.toml
cargo clippy --locked --manifest-path scripts/spikes/sqlcipher-compat/Cargo.toml --all-targets -- -D warnings
cargo fmt --manifest-path scripts/spikes/sqlcipher-compat/Cargo.toml -- --check
```

This is an isolated Cargo workspace. It does not enable SQLCipher in the desktop
application, alter the main lockfile, access the credential vault or open a real
user database. All keys/content are hard-coded synthetic fixtures; temporary
files live inside this workspace's ignored target directory and are removed when
the experiment ends normally. Never replace the fixture constants with real keys.

It reports the actual SQLite/cipher versions, evaluates the current application
WAL-version gate, runs the checked-out collaboration migration SQL, and exercises
FTS, transaction rollback, keyed read-only pooling, active WAL canary scans,
keyed/unkeyed/wrong-key reopen, VACUUM INTO and synthetic restore/integrity.
A successful process exit means the experiment completed, not that the candidate
is eligible for production. `PROMOTION_BLOCKED` is an explicit negative result.

On this macOS run, the pinned build is SQLCipher 4.10.0 / SQLite 3.50.4: it is below
Gitru's unchanged WAL safety gate. Cipher build prerequisites differ by platform;
Windows/Linux linkage and actual vaults are not qualified by this run. Full
application encryption/recovery and portable backup keys remain implementation
work. See [RURU-108](../../../docs/architecture/collaboration-work/RURU-108.md).
