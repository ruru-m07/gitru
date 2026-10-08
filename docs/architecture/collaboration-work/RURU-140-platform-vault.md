# RURU-140 — bounded macOS database-key vault adapter

Pre-code contract, 8 October 2026. This isolated managed Lexar worktree starts
from published PR194, signed `117bef476929db3171c863fc711340e5dd127286`.
The original architecture/backlog and R140 lifecycle contract remain authoritative.
R140 is still In Progress and depends on R139; this slice does not activate
production database encryption or complete keying/conversion acceptance.

## Live audit and chosen boundary

PR194 has 14 successful checks at its exact source head. R139's seven-job native
SQLCipher qualification run37726932167 succeeded at61548a0d; its later docs-only
head5016fea9 has a fresh, still-in-progress CI run with no observed failure.
Neither result proves keyed application pools, real vault startup, conversion or
portable backups. No live provider credential is required for this work.

The pinned keyring3.6.3 set_secret path is an upsert, including Windows CredWrite.
A load followed by upsert is not atomic create and cannot implement the existing
DatabaseKeyVault::store_new no-overwrite contract. This slice therefore uses
macOS's native SecItemAdd duplicate refusal directly, without any update/delete
fallback. Linux Secret Service duplicate-item semantics and Windows cross-process
no-overwrite ownership require separately reviewed adapters; do not substitute
an upsert or silently lower the common contract.

Implement a native macOS adapter for an explicitly selected file-keychain handle.
It uses the fixed database-only service and DatabaseKeyIdentity's opaque reference,
never a renderer-selected service, arbitrary credential reference or provider PAT
namespace. Load searches only that exact keychain with exact class/service/account;
no broad keychain enumeration or default-search-list fallback. Store performs one
SecItemAdd and returns typed duplicate/uncertain/locked/unavailable outcomes without
replacing or deleting an existing value. A duplicate remains subject to the
lifecycle's constant-time readback: the same key can resume, a different key fails
closed. Exact 32-byte binary keys stay native and are never text/IPC/log/argument
values; owned temporary buffers zeroize and framework-owned copies are released.

## Blocking and platform limitations

Apple's current TN3137 recommends SecItem APIs and distinguishes modern data
protection keychains from the older file-based implementation. The file-keychain
shim has different ACL/prompt behavior; per-query noninteractive attributes cannot
be treated as a complete prompt-suppression guarantee there. Production activation
must therefore separately qualify the app's modern keychain/entitlement and owned
noninteractive execution boundary. The explicit-handle adapter is not selected
by Store::open, app setup or provider credential cleanup in this slice.

Actual fixture operations run in a dedicated subprocess that disables keychain
interaction only inside that child, creates a private temporary keychain with a
synthetic random password via native APIs, and uses only its exact handle. It may
lock/unlock/delete that fixture keychain. It must not change the user's default
keychain, search list, global lock state or any personal entry. Do not rely on a
keychain file being a portable standalone artifact: Apple documents additional
protected entropy dependencies on recent macOS. Cleanup uses the exact owned
fixture handle and temporary directory; never enumerate or manipulate OS-internal
keychain backing files.

## Meaningful qualification

Ordinary tests remain synthetic or pure and never access an OS keychain. Add an
explicit opt-in ignored subprocess fixture that proves real missing/binary read,
create-only duplicate refusal, concurrent competing creates preserve one value,
wrong-size value rejection, locked fixture refusal without UI, unlock/cold child
readback and database lifecycle resume after a completed vault write. Tests use
fresh reservation identities produced by the existing lifecycle, not a public
constructor that can address personal entries. No secret appears in a command line
or environment; cross-process fixture material travels only through a bounded
private file/pipe owned by the test and is removed with its directory.

Map only proved locked statuses to VaultLocked; authentication/permission/general
vault failures must remain conservative typed errors. A failed or cancelled OS call
cannot authorize resetting a database or deleting a key. This adapter never gains
a deletion method; cleanup exists only in the explicit fixture harness.

Record actual local source checks, opt-in macOS fixture outcomes and remote CI
separately. Real data-protection keychain behavior, Windows/Linux OS adapters,
key-before-first-access writer/readers/recovery integration, verified plaintext
staging/rollback, rotation and R141 portable encryption remain open. No fabricated
platform success, new IPC surface, schema migration or production cipher switch.

## Primary sources inspected

- [Apple SecItemAdd](https://developer.apple.com/documentation/security/secitemadd(_:_:)) and [duplicate item](https://developer.apple.com/documentation/security/errsecduplicateitem): native add/uniqueness semantics.
- [Apple TN3137](https://developer.apple.com/documentation/technotes/tn3137-on-mac-keychains), updated24 September2026: explicit keychain/search-list routing, legacy shim and entropy-file distinction.
- [Apple temporary keychain creation](https://developer.apple.com/documentation/security/seckeychaincreate(_:_:_:_:_:_:)): private temporary keychain for noninteractive application use.
- [Microsoft CredWriteW](https://learn.microsoft.com/en-us/windows/win32/api/wincred/nf-wincred-credwritew): replaces existing target/type, so it cannot by itself be called create-only.
- Pinned local keyring3.6.3, security-framework3.7.0 and installed Security.framework headers: concrete native wrapper and prompt behavior, not inferred cross-platform execution.
