# Updater integrity and recovery

This runbook covers the zero-cost controls around Gitru's updater feed. It does
not promote the current Tauri 3/CEF qualification build to production or claim
native installed-update coverage.

## Release invariants

- A stable release uses `MAJOR.MINOR.PATCH`; a beta uses
  `MAJOR.MINOR.PATCH-beta.N`. The manifest version never starts with `v`, while
  its Git tag is exactly `v<manifest-version>`.
- Stable manifests publish only to the `stable` channel and beta manifests only
  to `beta`. A candidate must be newer than every version ever published in the
  channel, not merely newer than its potentially rewound current pointer. A
  version with a durable `<channel>/revocations/<version>.json` marker can never
  be promoted again; recovery always uses a higher version.
- Every updater URL must be an exact HTTPS GitHub release asset URL:

  ```text
  https://github.com/ruru-m07/gitru/releases/download/v<manifest-version>/<asset>
  ```

  Reject a different owner, repository, host, port, tag, or scheme, and reject
  user information, query strings, fragments, encoded traversal, missing
  targets, unknown targets, and target/extension mismatches.
- Verify every downloaded updater artifact against the manifest's embedded
  Tauri minisign signature and the configured updater public key. A valid URL
  or checksum alone is not sufficient. Reject malformed signatures, a wrong
  key, modified artifacts, and signatures copied from another target.
- Keep private signing keys out of the repository and logs. Test fixtures may
  contain a clearly labelled disposable public key, artifact, and signature,
  but never the fixture private key.

Use the repository validator for preflight and readback checks. Consult its
`--help` output for the current command names; the stable invocation shape is:

```bash
cargo run -p updater-release -- <command> ...
```

## Publish a manifest

1. Build and sign all expected updater artifacts. `tauri-action` updates one
   shared GitHub `latest.json` asset, so serialize its platform matrix until
   manifest aggregation is moved into a dedicated job. Generate the final
   manifest only after all platform jobs have completed.
2. Run the updater-release validator against the candidate manifest and its
   downloaded artifacts. Confirm the channel, version/tag match, complete
   target set, exact GitHub asset origins, and embedded minisign signatures.
3. Read the current channel pointer bytes and ETag together. Treat only an
   absent object as an empty channel; authentication, network, and server errors
   must stop publication. Strictly validate a present pointer and include its
   version in the high-water check unless its bytes are exactly the candidate.
   This prevents a lower candidate from overwriting a newer pointer that was
   only partially or manually published without versioned history. A malformed
   current pointer fails normal publication closed.
4. Upload the candidate first to the versioned key:

   ```text
   <channel>/versions/<version>/latest.json
   ```

   Cloudflare R2 does not provide Object Lock, so immutability is enforced by
   the workflow: create the object conditionally with `If-None-Match: *`. If
   that key already exists, continue only when its bytes are identical to the
   candidate. Never overwrite a version with different content.
   Sign those exact manifest bytes with the updater key and store the
   create-only audit sidecar at `latest.json.sig`. Tauri clients verify each
   artifact rather than this sidecar; recovery tooling verifies the sidecar
   before trusting historical manifest metadata.
5. Reject the candidate if its durable revocation marker exists. Read the
   versioned object back and verify its bytes or digest, content type,
   cache policy, and updater-release validation result. Versioned manifests use
   a long-lived immutable cache policy, for example
   `public, max-age=31536000, immutable`.
6. Promote the channel pointer last by uploading the same validated bytes to:

   ```text
   <channel>/latest.json
   ```

   Use compare-and-swap: `If-Match` with the ETag captured before validation, or
   `If-None-Match: *` for an empty channel. The pointer uses a revalidation
   policy such as `no-cache, no-store, must-revalidate`. Read it back through
   both the R2 S3 API and the exact client URL at
   `https://release.gitru.app/<channel>/latest.json`. Retry the public read until
   it serves byte-identical content with JSON and no-store cache headers; a
   successful origin write alone is not publication success.

Perform the expensive artifact downloads, signature checks, and candidate
manifest signing in an unlocked preparation job. Transfer the validated
manifest, sidecar, report, validator binary, and exact configuration inputs with
pinned artifact actions. Each preparation attempt has a distinct evidence
artifact; failed-job retries reuse the successful preparation job's exact
artifact instead of rebuilding release bytes. Serialize only the short metadata
revalidation and manifest promotion job per channel, with a bounded timeout and
the full pending queue enabled. Never cancel or displace an in-progress or
already-queued feed operation. Builds and artifact verification do not hold
this lock, so an emergency stop can run while they proceed; the later publisher
then observes the durable revocation marker and fails closed. The ETag
compare-and-swap also prevents a stale release or operator action from
clobbering a newer pointer. Publishing the create-only versioned object first
ensures a failed promotion can be retried without changing release history;
byte-identical retries are safe.

## Stop a rollout and recover

If a release is harmful, first stop future adoption:

1. Identify the last known-good immutable manifest and validate it again. It
   must not have its own durable revocation marker; a revoked version is never
   eligible as a fallback.
2. Verify its versioned `latest.json.sig`, then re-download and verify every
   referenced updater artifact. During the initial rollout of this policy,
   historical manifests may predate the sidecar. That legacy absence requires
   selecting `ALLOW_LEGACY_WITHOUT_SIDECAR` and never skips embedded signature
   verification for every referenced artifact. After those artifacts pass, the
   workflow signs the exact historical manifest, conditionally backfills its
   create-only sidecar, and verifies the sidecar readback before changing the
   pointer. If a sidecar already exists, an invalid sidecar is a hard failure.
3. Before changing the pointer, preserve the exact current pointer bytes at the
   create-only, hash-addressed key
   `<channel>/revocation-evidence/<bad-version>/<sha256>.bin`, then verify its
   bytes, digest, and immutable headers. Create the immutable rollout marker
   `<channel>/revocations/<bad-version>.json` with `If-None-Match: *`. The marker
   records the initiating evidence key and digest and makes the stop sticky:
   delayed or rerun release jobs reject that version. A later out-of-band
   corruption of the same asserted version produces a new hash-addressed
   evidence object and incident summary without weakening the existing marker.
4. Repoint only `<channel>/latest.json` to those exact known-good bytes with
   `If-Match` against the current pointer's previously captured ETag.
5. Read the pointer back through R2 and the exact public client URL. Verify exact
   bytes, JSON/no-store headers, origin rules, and artifact signatures. Record
   the affected and restored versions plus the revocation marker in the
   incident.

If the current pointer itself is malformed, recovery must still be possible.
Capture its bytes and ETag, fully validate the fallback as above, enter the
asserted current version, and use the distinct `STOP_CORRUPT_ROLLOUT`
confirmation. The validator accepts this mode only when strict current-pointer
validation actually fails, checks that the asserted version is canonical for
the selected channel and newer than the fallback, and rejects a different
readable version in the corrupt JSON. Preserve the corrupt bytes themselves
under the hash-addressed evidence key and record its SHA-256/key in the
revocation marker and run summary, tombstone the asserted version, then replace
only the ETag returned with those downloaded bytes. A valid pointer requires
ordinary `STOP_ROLLOUT` and cannot use the corrupt override.

This pointer change protects only clients that have not already installed the
bad version. It does not downgrade or repair clients that already updated.

The real rollback is a forward recovery release: revert to known-good source or
apply the fix, assign a version strictly higher than the bad release, build and
sign fresh artifacts, validate them, and publish them through the normal
immutable-first flow. Never reuse a version, mutate a versioned manifest, or
publish a lower version as a repair.

## Qualification boundary

RURU-93 remains deliberately inert: it uses a disposable application identity,
has updater artifacts and endpoints disabled, and rejects updater commands.
These controls must not be weakened to make this runbook pass. Native
previous-to-candidate installation, relaunch, failed-update recovery, and
architecture-aware delivery remain later promotion gates for RURU-88/RURU-93.

Apple Developer ID signing and notarization, plus Windows Authenticode signing
and SmartScreen reputation, require paid external credentials and remain
unverified. The validation and publishing controls above prove feed and artifact
integrity; they do not substitute for either platform's trust checks.
